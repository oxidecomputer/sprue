// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use clap::Parser;
use sprue_svc::{DEFAULT_SPRUE_SOCKET, SprueServiceClient};
use std::path::Path;
use std::{path::PathBuf, sync::Arc};
use tarpc::context;
use tarpc::tokio_serde::formats::Json;
use tracing_appender::non_blocking::NonBlocking;
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

use crate::platform::OxidePlatform;
use crate::server::SprueAgentStarter;

mod cmd;
mod oidc;
mod platform;
mod server;
mod vsock;

static VM_ATTESTATION_PORT: u32 = 605;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Path to a directory containing mock attestation fixtures for local
    /// development (cert-chain.pem, log.bin, alias.key, vm.json).
    ///
    /// Only available when built with the `local-dev` feature.
    #[cfg(feature = "local-dev")]
    #[clap(long)]
    mock_dir: Option<PathBuf>,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Parser)]
enum Commands {
    /// Retrieve an OIDC token from the sprue service
    GetToken {
        #[clap(short, long)]
        id: Uuid,
    },
    /// Register and store an arbitrary blob to remote backup storage
    Backup {
        /// Path to file to store
        path: PathBuf,
        /// Socket the Sprue agent is listening on
        #[clap(long)]
        socket: Option<PathBuf>,
    },
    /// Register a server instance with the sprue service and prove its identity
    ///
    /// The instance, project, and silo identifiers are discovered automatically
    /// via a platform attestation over vsock.  The only input the calling
    /// application needs to supply is the service name it belongs to.
    RegisterServer {
        /// Service to register server for
        #[clap(short, long)]
        service: String,
    },
    /// Serve the sprue agent as a standalone service
    Serve {
        /// The URL of the Sprue server
        #[clap(short, long)]
        server: String,
        /// Name of the service to register as
        #[clap(long)]
        service: String,
        /// Socket to run on
        #[clap(long)]
        socket: Option<PathBuf>,
    },
}

fn build_platform(args: &Args) -> anyhow::Result<Box<dyn platform::Platform + Sync>> {
    #[cfg(feature = "local-dev")]
    if let Some(ref dir) = args.mock_dir {
        tracing::info!(
            ?dir,
            "Using mock attestation platform for local development"
        );
        return Ok(Box::new(platform::mock::MockPlatform::from_test_data(dir)?));
    }

    let _ = args; // suppress unused warning when local-dev is off
    Ok(Box::new(OxidePlatform))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let platform = build_platform(&args)?;

    match args.command {
        Commands::GetToken { id } => {
            // let token = cmd::get_token(&client, id, platform.as_ref()).await?;
            // println!("{token}");
        }
        Commands::Backup { path, socket } => {
            let client = svc_client(&socket.unwrap_or(PathBuf::from(DEFAULT_SPRUE_SOCKET))).await?;
            let blob_id = client.backup(context::current(), path).await??;
            println!("Backup completed successfully. Created {}", blob_id);
        }
        Commands::RegisterServer { service } => {
            // cmd::register_server(&client, &service, platform.as_ref()).await?;
            // println!("Server registered successfully");
        }
        Commands::Serve {
            server,
            socket,
            service,
        } => {
            let (writer, _guard) = NonBlocking::new(std::io::stdout());
            let _subscriber = tracing_subscriber::fmt()
                .with_file(false)
                .with_line_number(false)
                .with_env_filter(EnvFilter::from_default_env())
                .with_writer(writer)
                .json()
                .init();

            SprueAgentStarter::new(
                server,
                service,
                socket.unwrap_or(PathBuf::from(DEFAULT_SPRUE_SOCKET)),
                Arc::from(platform),
            )
            .serve()
            .await?;
        }
    }

    Ok(())
}

async fn svc_client(socket: &Path) -> anyhow::Result<SprueServiceClient> {
    let transport = tarpc::serde_transport::unix::connect(&socket, Json::default).await?;
    let client = SprueServiceClient::new(tarpc::client::Config::default(), transport).spawn();
    Ok(client)
}
