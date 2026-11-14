// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use futures::future;
use futures_util::StreamExt;
use reqwest::{
    StatusCode,
    header::{AUTHORIZATION, HeaderMap, HeaderValue},
};
use sprue_sdk::{
    Client,
    types::{Audience, TypedUuidForServerRegistrationId},
};
use sprue_svc::{SprueError, SprueService};
use std::{
    path::PathBuf,
    sync::{Arc, RwLock},
    time::Duration,
};
use tarpc::{
    context,
    server::{self, Channel},
    tokio_serde::formats::Json,
};
use tokio::sync::Semaphore;
use tokio::time::Instant;

use crate::{
    cmd::{self, BackupRequest, CheckinRequest, TokenRequest},
    platform::Platform,
    registration::{Registration, Standing},
};

/// Guard that removes the unix socket file when dropped.
struct SocketGuard {
    path: PathBuf,
}

impl SocketGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for SocketGuard {
    fn drop(&mut self) {
        if self.path.exists() {
            if let Err(err) = std::fs::remove_file(&self.path) {
                tracing::warn!(?err, path = ?self.path, "Failed to remove socket file");
            } else {
                tracing::info!(path = ?self.path, "Removed socket file");
            }
        }
    }
}

/// Buffer subtracted from the JWT `exp` to refresh before actual expiry.
const TOKEN_EXPIRY_BUFFER: Duration = Duration::from_secs(30);

struct CachedToken {
    registration_id: TypedUuidForServerRegistrationId,
    token: String,
    client: Client,
    expires_at: Instant,
}

/// A shared, lazily-authenticated client that caches its bearer token and
/// rebuilds the underlying `sprue_sdk::Client` when the token expires. Tokens
/// are only requested while the registration is accepted.
///
/// Unauthenticated operations (e.g. registration) use a plain `Client`
/// without a bearer token.
pub struct AuthenticatedClient {
    server: String,
    registration: Arc<Registration>,
    platform: Arc<dyn Platform + Sync>,
    /// Cached token and pre-built client. Reads are cheap (`std::sync::RwLock`
    /// never crosses an await point). The semaphore below gates the async
    /// refresh so we never hold this lock across an await.
    cache: RwLock<Option<CachedToken>>,
    /// A single-permit semaphore ensures only one task runs the (expensive)
    /// attestation + token exchange at a time.
    refresh_semaphore: Semaphore,
}

impl AuthenticatedClient {
    fn new(
        server: String,
        registration: Arc<Registration>,
        platform: Arc<dyn Platform + Sync>,
    ) -> Self {
        Self {
            server,
            registration,
            platform,
            cache: RwLock::new(None),
            refresh_semaphore: Semaphore::new(1),
        }
    }

    /// Return a plain, unauthenticated client.
    fn unauthenticated(&self) -> Client {
        Client::new(&self.server)
    }

    /// Return the id of the registration when it is accepted. Any other
    /// standing means that the agent must not request a token.
    fn accepted_id(&self) -> Result<TypedUuidForServerRegistrationId, SprueError> {
        match self.registration.standing() {
            Standing::Accepted(id) => Ok(id),
            standing => Err(SprueError::NotAccepted(standing.to_string())),
        }
    }

    /// Read the cached token and client if they belong to the given
    /// registration and the token is still valid.
    fn cached(&self, id: &TypedUuidForServerRegistrationId) -> Option<(String, Client)> {
        let guard = self.cache.read().unwrap_or_else(|e| e.into_inner());
        match &*guard {
            Some(cached)
                if cached.registration_id.0 == id.0 && Instant::now() < cached.expires_at =>
            {
                Some((cached.token.clone(), cached.client.clone()))
            }
            _ => None,
        }
    }

    /// Return a token and a `Client` that carries it, refreshing if necessary.
    async fn authenticated(
        &self,
    ) -> Result<(TypedUuidForServerRegistrationId, String, Client), SprueError> {
        let id = self.accepted_id()?;
        let (token, client) = match self.cached(&id) {
            Some(cached) => cached,
            None => self.refresh(&id).await?,
        };
        Ok((id, token, client))
    }

    /// Exchange an attestation for a new token. Only one refresh runs at a
    /// time, and a token cached by another task while waiting is reused.
    async fn refresh(
        &self,
        id: &TypedUuidForServerRegistrationId,
    ) -> Result<(String, Client), SprueError> {
        let _permit = self
            .refresh_semaphore
            .acquire()
            .await
            .map_err(|_| SprueError::Failure("Refresh semaphore closed".to_string()))?;

        match self.cached(id) {
            Some(cached) => Ok(cached),
            None => {
                tracing::info!("Refreshing authentication token");

                let plain = self.unauthenticated();
                let token = cmd::get_token(TokenRequest {
                    client: &plain,
                    registration_id: id.clone(),
                    platform: self.platform.as_ref(),
                    issuer: &self.server,
                    audience: None,
                })
                .await
                .map_err(|err| {
                    tracing::error!(?err, "Failed to refresh token");

                    // Sprue refuses to issue a token to a registration that
                    // is not accepted or no longer exists.
                    if status_of(&err).is_some_and(|status| status.is_client_error()) {
                        self.registration.recheck();
                    }
                    SprueError::Failure(err.to_string())
                })?;

                let expires_at = token_expiry(&token)?;
                let client = build_client(&self.server, &token)?;

                // Write lock is held only for the assignment — no await.
                {
                    let mut guard = self.cache.write().unwrap_or_else(|e| e.into_inner());
                    *guard = Some(CachedToken {
                        registration_id: id.clone(),
                        token: token.clone(),
                        client: client.clone(),
                        expires_at,
                    });
                }

                Ok((token, client))
            }
        }
    }

    /// Request a token for another audience. These tokens are not used to
    /// talk to sprue, so they are not cached and a new one is requested on
    /// every call.
    async fn token_for(&self, audience: &Audience) -> Result<String, SprueError> {
        let id = self.accepted_id()?;
        let plain = self.unauthenticated();
        cmd::get_token(TokenRequest {
            client: &plain,
            registration_id: id,
            platform: self.platform.as_ref(),
            issuer: &self.server,
            audience: Some(audience),
        })
        .await
        .map_err(|err| {
            tracing::error!(?err, audience = %audience.as_str(), "Failed to get token");
            if status_of(&err).is_some_and(|status| status.is_client_error()) {
                self.registration.recheck();
            }
            SprueError::Failure(err.to_string())
        })
    }

    /// React to a failed request that carried this agent's token. Sprue
    /// refuses such requests once the registration is no longer accepted, so
    /// the cached token is dropped and the standing is checked again.
    fn observe(&self, err: &anyhow::Error) {
        match status_of(err) {
            Some(StatusCode::FORBIDDEN) => {
                tracing::info!("Sprue refused the agent token");
                *self.cache.write().unwrap_or_else(|e| e.into_inner()) = None;
                self.registration.recheck();
            }
            // Sprue did not accept the token itself, so a new one is
            // requested on the next call.
            Some(StatusCode::UNAUTHORIZED) => {
                tracing::info!("Sprue did not accept the agent token");
                *self.cache.write().unwrap_or_else(|e| e.into_inner()) = None;
            }
            _ => {}
        }
    }
}

/// Parse an audience that a local caller asked for. The sprue server is
/// refused as an audience because a token for sprue lets the caller act as
/// this server towards sprue.
fn external_audience(server: &str, audience: String) -> Result<Audience, SprueError> {
    match audience.trim_end_matches('/') == server.trim_end_matches('/') {
        true => Err(SprueError::Failure(
            "Tokens for sprue are not handed out by the agent".to_string(),
        )),
        false => Audience::try_from(audience)
            .map_err(|err| SprueError::Failure(format!("Invalid audience: {err}"))),
    }
}

/// The HTTP status of a failed sprue request, when the error came from one.
fn status_of(err: &anyhow::Error) -> Option<StatusCode> {
    err.downcast_ref::<sprue_sdk::Error<sprue_sdk::types::Error>>()
        .and_then(|err| err.status())
}

/// Decode the JWT `exp` claim and convert to a tokio `Instant`, applying a
/// safety buffer so we refresh before the token actually expires.
fn token_expiry(token: &str) -> Result<Instant, SprueError> {
    // Decode header+payload without signature verification (we already
    // validated in get_token).
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3 {
        return Err(SprueError::Failure("Malformed JWT".to_string()));
    }

    use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
    let payload = BASE64_URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|e| SprueError::Failure(format!("Failed to decode JWT payload: {}", e)))?;

    #[derive(serde::Deserialize)]
    struct Exp {
        exp: i64,
    }

    let claims: Exp = serde_json::from_slice(&payload)
        .map_err(|e| SprueError::Failure(format!("Failed to parse JWT exp claim: {}", e)))?;

    let now_unix = chrono::Utc::now().timestamp();
    let remaining = Duration::from_secs((claims.exp - now_unix).max(0) as u64);
    let expires_at = Instant::now() + remaining.saturating_sub(TOKEN_EXPIRY_BUFFER);

    Ok(expires_at)
}

/// Build a `sprue_sdk::Client` with a bearer token baked into default headers.
fn build_client(server: &str, token: &str) -> Result<Client, SprueError> {
    let mut auth_header = HeaderValue::from_str(&format!("Bearer {}", token)).map_err(|err| {
        tracing::error!(?err, "Failed to construct auth header");
        SprueError::Failure(err.to_string())
    })?;
    auth_header.set_sensitive(true);

    let mut headers = HeaderMap::new();
    headers.insert(AUTHORIZATION, auth_header);

    let http = reqwest::Client::builder()
        .default_headers(headers)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| {
            tracing::error!(?err, "Failed to build HTTP client");
            SprueError::Failure(err.to_string())
        })?;

    Ok(Client::new_with_client(server, http))
}

#[derive(Clone)]
pub struct SprueAgent {
    auth: Arc<AuthenticatedClient>,
}

impl SprueService for SprueAgent {
    fn checkin(
        self,
        _context: tarpc::context::Context,
    ) -> impl Future<Output = Result<(), SprueError>> {
        async move {
            let (registration_id, _, client) = self.auth.authenticated().await?;
            cmd::checkin(CheckinRequest {
                client: &client,
                registration_id,
            })
            .await
            .map_err(|err| {
                tracing::error!(?err, "Failed to checkin");
                self.auth.observe(&err);
                SprueError::Failure(err.to_string())
            })
        }
    }

    fn backup(
        self,
        _context: tarpc::context::Context,
        path: PathBuf,
    ) -> impl Future<Output = Result<String, SprueError>> {
        async move {
            let (registration_id, _, client) = self.auth.authenticated().await?;
            Ok(cmd::backup(BackupRequest {
                client: &client,
                registration_id,
                path: &path,
            })
            .await
            .map_err(|err| {
                tracing::error!(?err, "Failed to backup");
                self.auth.observe(&err);
                SprueError::Failure(err.to_string())
            })?
            .to_string())
        }
    }

    fn get_token(
        self,
        _context: tarpc::context::Context,
        audience: String,
    ) -> impl Future<Output = Result<String, SprueError>> {
        async move {
            let audience = external_audience(&self.auth.server, audience)?;
            self.auth.token_for(&audience).await
        }
    }

    fn register_server(
        self,
        _context: tarpc::context::Context,
    ) -> impl Future<Output = Result<TypedUuidForServerRegistrationId, SprueError>> {
        async move {
            // Registration is driven in the background. Wait until it has
            // produced a registration and report it.
            let mut standing = self.auth.registration.subscribe();
            let standing = standing
                .wait_for(|standing| *standing != Standing::Unregistered)
                .await
                .map_err(|_| SprueError::Failure("Registration task stopped".to_string()))?
                .clone();
            match standing {
                Standing::Waiting(id) | Standing::Accepted(id) => Ok(id),
                standing => Err(SprueError::NotAccepted(standing.to_string())),
            }
        }
    }

    fn get_registration_id(
        self,
        _context: tarpc::context::Context,
    ) -> impl Future<Output = Result<TypedUuidForServerRegistrationId, SprueError>> {
        async move {
            let standing = self.auth.registration.standing();
            standing
                .registration_id()
                .ok_or_else(|| SprueError::NotAccepted(standing.to_string()))
        }
    }
}

pub struct SprueAgentStarter {
    server: String,
    service: String,
    socket: PathBuf,
    platform: Arc<dyn Platform + Sync>,
}

impl SprueAgentStarter {
    pub fn new<T, S>(
        server: T,
        service: S,
        socket: PathBuf,
        platform: Arc<dyn Platform + Sync>,
    ) -> Self
    where
        T: ToString,
        S: ToString,
    {
        Self {
            server: server.to_string(),
            service: service.to_string(),
            socket,
            platform,
        }
    }

    pub async fn serve(&self) -> anyhow::Result<()> {
        // Verify that the socket parent directory exists
        if self.socket.parent().is_none() || !self.socket.parent().unwrap().exists() {
            anyhow::bail!(
                "Socket path does not exist: {:?}",
                self.socket.parent().map(|p| p.display())
            );
        }

        // Remove a stale socket from a previous run
        if self.socket.exists() {
            std::fs::remove_file(&self.socket)?;
        }

        // Registration is driven in the background so that the socket is
        // available while the agent is not yet accepted.
        let registration = Arc::new(Registration::new());
        let auth = Arc::new(AuthenticatedClient::new(
            self.server.clone(),
            registration.clone(),
            self.platform.clone(),
        ));
        let client = Client::new(&self.server);
        let registration_loop = registration.run(&client, &self.service, self.platform.as_ref());

        let listener = tarpc::serde_transport::unix::listen(&self.socket, Json::default).await?;

        // Guard ensures the socket file is cleaned up on drop
        let _guard = SocketGuard::new(self.socket.clone());

        tracing::info!(path = ?self.socket, "Listening on socket");

        let accept_loop = listener
            .filter_map(|r| future::ready(r.ok()))
            .map(server::BaseChannel::with_defaults)
            .map(|channel| {
                let agent = SprueAgent { auth: auth.clone() };
                async { channel.execute(agent.serve()).for_each(spawn).await }
            })
            .buffer_unordered(10)
            .for_each(|_| async {});

        tracing::info!("Accepting connections");

        // Check in periodically while the registration is accepted
        let checkin_agent = SprueAgent { auth: auth.clone() };
        let mut standing = registration.subscribe();
        let checkin_loop = async {
            loop {
                let accepted = standing
                    .wait_for(|standing| matches!(standing, Standing::Accepted(_)))
                    .await
                    .is_ok();
                if accepted {
                    match checkin_agent.clone().checkin(context::current()).await {
                        Ok(_) => tracing::trace!("Checkin successful"),
                        Err(err) => tracing::error!(?err, "Failed to checkin"),
                    };
                }

                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        };

        // Run the accept loop until a shutdown signal is received
        tokio::select! {
            _ = accept_loop => {},
            _ = checkin_loop => {},
            _ = registration_loop => {},
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received shutdown signal");
            }
        }

        // _guard drops here, removing the socket file
        Ok(())
    }
}

async fn spawn(fut: impl Future<Output = ()> + Send + 'static) {
    tokio::spawn(fut);
}

#[cfg(test)]
mod tests {
    use super::external_audience;

    #[test]
    fn test_external_audience_refuses_sprue() {
        let server = "https://sprue.example.com";
        assert!(external_audience(server, server.to_string()).is_err());
        assert!(external_audience(server, format!("{server}/")).is_err());
        assert!(external_audience(server, "".to_string()).is_err());
        assert!(external_audience(server, "sts.amazonaws.com".to_string()).is_ok());
    }
}
