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

    /// Socket to run on or run against
    #[clap(long)]
    socket: Option<PathBuf>,

    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Parser)]
enum Commands {
    /// Retrieve an OIDC token from the sprue service
    GetToken,
    /// Register and store an arbitrary blob to remote backup storage
    Backup {
        /// Path to file to store
        path: PathBuf,
    },
    /// Register a server instance with the sprue service and prove its identity
    RegisterServer,
    /// Serve the sprue agent as a standalone service
    Serve {
        /// The URL of the Sprue server
        #[clap(short, long, env = "SPRUE_SERVER")]
        server: String,
        /// Name of the service to register as
        #[clap(long)]
        service: String,
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
    let socket = args
        .socket
        .as_ref()
        .unwrap_or(&PathBuf::from(DEFAULT_SPRUE_SOCKET))
        .to_owned();

    let (writer, _guard) = NonBlocking::new(std::io::stdout());
    let _subscriber = tracing_subscriber::fmt()
        .with_file(false)
        .with_line_number(false)
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(writer)
        .json()
        .init();

    match &args.command {
        Commands::GetToken => {
            let client = svc_client(&socket).await?;
            let token = client.get_token(context::current()).await??;
            println!("{}", token);
        }
        Commands::Backup { path } => {
            let client = svc_client(&socket).await?;
            let blob_id = client
                .backup(context::current(), path.to_path_buf())
                .await??;
            println!("Backup completed successfully. Created {}", blob_id);
        }
        Commands::RegisterServer => {
            let client = svc_client(&socket).await?;
            let id = client.register_server(context::current()).await??;
            println!("{}", id);
        }
        Commands::Serve { server, service } => {
            let platform = build_platform(&args)?;
            SprueAgentStarter::new(server, service, socket, Arc::from(platform))
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
