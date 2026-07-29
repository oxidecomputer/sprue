// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use futures::future;
use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use sprue_sdk::{Client, types::TypedUuidForServerRegistrationId};
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
    token: String,
    client: Client,
    expires_at: Instant,
}

/// A shared, lazily-authenticated client that caches its bearer token and
/// rebuilds the underlying `sprue_sdk::Client` when the token expires.
///
/// Unauthenticated operations (e.g. `register_server`) use a plain `Client`
/// without a bearer token.
pub struct AuthenticatedClient {
    server: String,
    registration_id: TypedUuidForServerRegistrationId,
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
        registration_id: TypedUuidForServerRegistrationId,
        platform: Arc<dyn Platform + Sync>,
    ) -> Self {
        Self {
            server,
            registration_id,
            platform,
            cache: RwLock::new(None),
            refresh_semaphore: Semaphore::new(1),
        }
    }

    /// Return a plain, unauthenticated client.
    fn unauthenticated(&self) -> Client {
        Client::new(&self.server)
    }

    /// Read the cached client if the token is still valid.
    fn cached_client(&self) -> Option<Client> {
        let guard = self.cache.read().unwrap_or_else(|e| e.into_inner());
        match &*guard {
            Some(cached) if Instant::now() < cached.expires_at => Some(cached.client.clone()),
            _ => None,
        }
    }

    /// Read the cached token if it is still valid.
    fn cached_token(&self) -> Option<String> {
        let guard = self.cache.read().unwrap_or_else(|e| e.into_inner());
        match &*guard {
            Some(cached) if Instant::now() < cached.expires_at => Some(cached.token.clone()),
            _ => None,
        }
    }

    /// Return a `Client` with a valid bearer token, refreshing if necessary.
    async fn client(&self) -> Result<Client, SprueError> {
        // Fast path: no locking beyond the std RwLock (never held across await).
        if let Some(client) = self.cached_client() {
            return Ok(client);
        }

        // Slow path: acquire the single refresh permit.
        let _permit = self
            .refresh_semaphore
            .acquire()
            .await
            .map_err(|_| SprueError::Failure("Refresh semaphore closed".to_string()))?;

        // Re-check — another task may have refreshed while we waited.
        if let Some(client) = self.cached_client() {
            return Ok(client);
        }

        tracing::info!("Refreshing authentication token");

        let plain = self.unauthenticated();
        let token = cmd::get_token(TokenRequest {
            client: &plain,
            registration_id: self.registration_id.clone(),
            platform: self.platform.as_ref(),
            issuer: &self.server,
        })
        .await
        .map_err(|err| {
            tracing::error!(?err, "Failed to refresh token");
            SprueError::Failure(err.to_string())
        })?;

        let expires_at = token_expiry(&token)?;
        let client = build_client(&self.server, &token)?;

        // Write lock is held only for the assignment — no await.
        {
            let mut guard = self.cache.write().unwrap_or_else(|e| e.into_inner());
            *guard = Some(CachedToken {
                token,
                client: client.clone(),
                expires_at,
            });
        }

        Ok(client)
    }

    /// Return the raw token string, refreshing if necessary.
    async fn token(&self) -> Result<String, SprueError> {
        self.client().await?;
        Ok(self.cached_token().expect("just refreshed"))
    }
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
    service: String,
    auth: Arc<AuthenticatedClient>,
}

impl SprueService for SprueAgent {
    fn checkin(
        self,
        _context: tarpc::context::Context,
    ) -> impl Future<Output = Result<(), SprueError>> {
        async move {
            let client = self.auth.client().await?;
            cmd::checkin(CheckinRequest {
                client: &client,
                registration_id: self.auth.registration_id.clone(),
            })
            .await
            .map_err(|err| {
                tracing::error!(?err, "Failed to checkin");
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
            let client = self.auth.client().await?;
            Ok(cmd::backup(BackupRequest {
                client: &client,
                registration_id: self.auth.registration_id.clone(),
                path: &path,
            })
            .await
            .map_err(|err| {
                tracing::error!(?err, "Failed to backup");
                SprueError::Failure(err.to_string())
            })?
            .to_string())
        }
    }

    fn get_token(
        self,
        _context: tarpc::context::Context,
    ) -> impl Future<Output = Result<String, SprueError>> {
        async move {
            tracing::info!(
                registration = ?self.auth.registration_id.0,
                "Retrieving token for registration id"
            );
            self.auth.token().await
        }
    }

    fn register_server(
        self,
        _context: tarpc::context::Context,
    ) -> impl Future<Output = Result<TypedUuidForServerRegistrationId, SprueError>> {
        async move {
            let client = self.auth.unauthenticated();
            cmd::register_server(&client, &self.service, self.auth.platform.as_ref())
                .await
                .map_err(|err| {
                    tracing::error!(?err, "Failed to register server");
                    SprueError::Failure(err.to_string())
                })
        }
    }

    fn get_registration_id(
        self,
        _context: tarpc::context::Context,
    ) -> impl Future<Output = TypedUuidForServerRegistrationId> {
        async move { self.auth.registration_id.clone() }
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

    const INITIAL_RETRY_INTERVAL: Duration = Duration::from_secs(1);
    const MAX_RETRY_INTERVAL: Duration = Duration::from_secs(60);

    /// Attempt to register the server, retrying with exponential backoff if
    /// the remote sprue server is not reachable yet.
    async fn register_with_retry(
        client: &Client,
        service: &str,
        platform: &dyn Platform,
    ) -> anyhow::Result<TypedUuidForServerRegistrationId> {
        let mut interval = Self::INITIAL_RETRY_INTERVAL;

        loop {
            match cmd::register_server(client, service, platform).await {
                Ok(id) => return Ok(id),
                Err(err) => {
                    tracing::warn!(
                        ?err,
                        retry_in = ?interval,
                        "Failed to register with sprue server, retrying"
                    );
                    tokio::time::sleep(interval).await;
                    interval = (interval * 2).min(Self::MAX_RETRY_INTERVAL);
                }
            }
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

        let client = Client::new(&self.server);

        // Wait for the remote sprue server to become available and register.
        // The server may not be running yet at boot time so we retry with
        // exponential backoff rather than failing immediately.
        let registration_id =
            Self::register_with_retry(&client, &self.service, self.platform.as_ref()).await?;

        let auth = Arc::new(AuthenticatedClient::new(
            self.server.clone(),
            registration_id.clone(),
            self.platform.clone(),
        ));

        let listener = tarpc::serde_transport::unix::listen(&self.socket, Json::default).await?;

        // Guard ensures the socket file is cleaned up on drop
        let _guard = SocketGuard::new(self.socket.clone());

        tracing::info!(path = ?self.socket, "Listening on socket");

        let accept_loop = listener
            .filter_map(|r| future::ready(r.ok()))
            .map(server::BaseChannel::with_defaults)
            .map(|channel| {
                let agent = SprueAgent {
                    service: self.service.clone(),
                    auth: auth.clone(),
                };
                async { channel.execute(agent.serve()).for_each(spawn).await }
            })
            .buffer_unordered(10)
            .for_each(|_| async {});

        tracing::info!("Accepting connections");

        // Start a checkin loop that periodically checks in with the server
        let checkin_agent = SprueAgent {
            service: self.service.clone(),
            auth: auth.clone(),
        };
        let checkin_loop = async {
            loop {
                match checkin_agent.clone().checkin(context::current()).await {
                    Ok(_) => {
                        tracing::trace!("Checkin successful");
                    }
                    Err(err) => {
                        tracing::error!(?err, "Failed to checkin");
                    }
                };

                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        };

        // Run the accept loop until a shutdown signal is received
        tokio::select! {
            _ = accept_loop => {},
            _ = checkin_loop => {},
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
