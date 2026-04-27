// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::Context;
use clap::Parser;
use lib_vsock::{VMADDR_CID_HOST, VsockAddr, VsockStream};
use uuid::Uuid;
use vm_attest::{QualifyingData, VmInstanceAttester};

use crate::oidc::validate_jwt;
use crate::vsock::VmInstanceRotVsockClient;

mod oidc;
mod vsock;

static VM_ATTESTATION_PORT: u32 = 605;

#[derive(Debug, Parser)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// The URL of the sprue server
    #[clap(short, long)]
    server: String,
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Parser)]
enum Commands {
    /// Retrieve an OIDC token from the sprue service
    GetToken {
        /// Registraiton id
        id: Uuid,
    },
    /// Register a server instance with the sprue service
    RegisterServer {
        /// Instance id
        id: Uuid,
        /// Service to register server for
        service: String,
    },
    /// Accept a server registration with the sprue service
    AcceptServer {
        /// Registration id
        id: Uuid,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let client = sprue_sdk::Client::new(&args.server);

    match args.command {
        Commands::GetToken { id } => {
            // Send our id to the server to register a token flow and receive back a nonce to prove
            // ownership of this challenge
            let response = client
                .register_oidc_token_request()
                .server(id)
                .send()
                .await?;

            // Convert the nonce into a 32 byte array which is used by the attestation API
            let server_nonce: [u8; 32] = response
                .nonce
                .as_ref()
                .map(hex::decode)
                .ok_or_else(|| anyhow::anyhow!("No nonce returned"))??
                .try_into()
                .map_err(|v: Vec<u8>| anyhow::anyhow!("expected 32 bytes, got {}", v.len()))?;
            let qualifying_data = QualifyingData::from(server_nonce);

            // Communicate over the known VM attestation port to retrieve a platform attestation
            let addr = VsockAddr::new(VMADDR_CID_HOST, VM_ATTESTATION_PORT);
            let stream = VsockStream::connect(&addr).context("vsock stream connect")?;
            let vm_instance_rot = VmInstanceRotVsockClient::new(stream);
            let attestation = vm_instance_rot.attest(&qualifying_data)?;
            let serialized = serde_json::to_value(attestation)?;

            // Send the attestation back to the server to complete the challenge. The server will
            // verify that the id of the vm in the attestation matches the id of the vm we sent.
            let token_response = client
                .prove_oidc_token_request()
                .server(id)
                .body_map(|body| body.attestation(serialized))
                .send()
                .await?;

            let token = token_response.into_inner().token;

            // Fetch the JWKS from the server to validate the token
            let jwks_response = client.jwks_json().send().await?;
            let jwks = jwks_response.into_inner();

            // Validate the JWT against the JWKS
            let claims = validate_jwt(&token, &jwks)?;

            println!("Token validated successfully!");
            println!();
            println!("  Subject (VM ID): {}", claims.sub);
            println!("  Issuer: {}", claims.iss);
            println!("  Audience: {}", claims.aud);
            println!();
            println!("  Token: {}", token);
        }
        Commands::RegisterServer { id, service } => {
            let response = client
                .register_server()
                .service(service)
                .body_map(|body| body.instance(id))
                .send()
                .await?;
            println!("{:?}", response);
        }
        Commands::AcceptServer { id } => {
            let response = client.accept_server().server(id).send().await?;
            println!("{:?}", response);
        }
    }

    Ok(())
}
