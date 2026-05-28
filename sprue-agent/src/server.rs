use std::{path::PathBuf, sync::Arc, time::Duration};

use futures::future;
use futures_util::StreamExt;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use sprue_sdk::{Client, types::TypedUuidForServerRegistrationId};
use sprue_svc::{SprueError, SprueService};
use tarpc::{
    server::{self, Channel},
    tokio_serde::formats::Json,
};

use crate::{
    cmd::{self, BackupRequest, TokenRequest},
    platform::Platform,
};

/// Guard that removes the unix socket file when dropped.
///
/// This ensures cleanup on normal exit, early return via `?`, and panic
/// unwinding. For SIGINT/SIGTERM the serve loop installs a signal handler
/// that breaks out of the loop so that this drop runs normally.
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

#[derive(Clone)]
pub struct SprueAgent {
    server: String,
    registration_id: TypedUuidForServerRegistrationId,
    platform: Arc<dyn Platform + Sync>,
}

impl SprueService for SprueAgent {
    fn backup(
        self,
        context: tarpc::context::Context,
        path: PathBuf,
    ) -> impl Future<Output = Result<String, SprueError>> {
        let server = self.server.clone();
        let registration_id = self.registration_id.clone();
        async move {
            let token = self.get_token(context).await?;
            let client = new_client(&server, Some(&token))?;
            tracing::info!(?client, "Created authenticated client");
            Ok(cmd::backup(BackupRequest {
                client: &client,
                registration_id,
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
        let client = Client::new(&self.server);
        async move {
            tracing::info!(registration = ?self.registration_id.0, "Retrieving token for registration id");
            cmd::get_token(TokenRequest {
                client: &client,
                registration_id: self.registration_id.clone(),
                platform: self.platform.as_ref(),
                issuer: &self.server,
            })
            .await
            .map_err(|err| {
                tracing::error!(?err, "Failed to retrieve token");
                SprueError::Failure(err.to_string())
            })
        }
    }
    fn get_registration_id(
        self,
        _context: tarpc::context::Context,
    ) -> impl Future<Output = TypedUuidForServerRegistrationId> {
        async move { self.registration_id }
    }
}

pub struct SprueAgentStarter {
    server: String,
    service: String,
    socket: PathBuf,
    platform: Arc<dyn Platform + Sync>,
}

impl SprueAgentStarter {
    pub fn new(
        server: String,
        service: String,
        socket: PathBuf,
        platform: Arc<dyn Platform + Sync>,
    ) -> Self {
        Self {
            server,
            service,
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

        let client = Client::new(&self.server);

        // Start by registering the server
        let registration_id =
            cmd::register_server(&client, &self.service, self.platform.as_ref()).await?;

        let listener = tarpc::serde_transport::unix::listen(&self.socket, Json::default).await?;

        // Guard ensures the socket file is cleaned up on drop
        let _guard = SocketGuard::new(self.socket.clone());

        tracing::info!(path = ?self.socket, "Listening on socket");

        let accept_loop = listener
            .filter_map(|r| future::ready(r.ok()))
            .map(server::BaseChannel::with_defaults)
            .map(|channel| {
                let agent = SprueAgent {
                    server: self.server.clone(),
                    registration_id: registration_id.clone(),
                    platform: self.platform.clone(),
                };
                async { channel.execute(agent.serve()).for_each(spawn).await }
            })
            .buffer_unordered(10)
            .for_each(|_| async {});

        tracing::info!("Accepting connections");

        // Run the accept loop until a shutdown signal is received
        tokio::select! {
            _ = accept_loop => {},
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

fn new_client(host: &str, token: Option<&str>) -> Result<Client, SprueError> {
    let mut default_headers = HeaderMap::new();

    if let Some(token) = token {
        let mut auth_header =
            HeaderValue::from_str(&format!("Bearer {}", token)).map_err(|err| {
                tracing::error!(?err, "Failed to parse auth header");
                SprueError::Failure(err.to_string())
            })?;
        auth_header.set_sensitive(true);
        default_headers.insert(AUTHORIZATION, auth_header);
    }

    let http_client = reqwest::Client::builder()
        .default_headers(default_headers)
        .connect_timeout(Duration::from_secs(5))
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|err| {
            tracing::error!(?err, "Failed to build HTTP client");
            SprueError::Failure(err.to_string())
        })?;

    Ok(Client::new_with_client(host, http_client))
}
