use std::time::Duration;

use anyhow::anyhow;
use sprue_sdk::{
    Client,
    types::{ServerRegistrationState, TypedUuidForServerRegistrationId},
};
use tokio::time::interval;
use vm_attest::QualifyingData;

use crate::platform::Platform;

pub async fn register_server(
    client: &Client,
    service: &str,
    platform: &dyn Platform,
) -> anyhow::Result<TypedUuidForServerRegistrationId> {
    // Perform an attestation to learn the VM's identity
    let (_, vm_conf) = platform.discover_identity()?;

    let mut tick = interval(Duration::from_secs(5));
    loop {
        // Register the server with the sprue service
        let response = client
            .register_server()
            .service(service.to_string())
            .body_map(|body| {
                body.instance(vm_conf.uuid)
                    .project_id(vm_conf.project)
                    .silo_id(vm_conf.silo)
            })
            .send()
            .await?
            .into_inner();

        // If the server is already accepted then nothing else needs to be done. Otherwise we need to
        // perform an attestation flow to prove the server's identity.
        match response.registration.state {
            ServerRegistrationState::Accepted => return Ok(response.registration.id),
            ServerRegistrationState::Rejected => anyhow::bail!("Server registration rejected"),
            ServerRegistrationState::Proven => {
                // We need to wait for the registration to be approved as we were not automatically
                // approved based on identiy
            }
            ServerRegistrationState::Pending => {
                let registration = response.registration;

                // Perform an attestation with the server's challenge to prove the server's identity
                let nonce = registration
                    .nonce
                    .ok_or_else(|| anyhow!("No nonce returned"))?;
                let bytes: [u8; 32] = hex::decode(&nonce)?
                    .try_into()
                    .map_err(|v: Vec<u8>| anyhow!("expected 32 byte nonce, got {}", v.len()))?;
                let attestation = platform.attest(&QualifyingData::from(bytes))?;

                // Prove the server's identity to the sprue service
                client
                    .prove_server()
                    .server(registration.id.0)
                    .body_map(|body| body.attestation(attestation))
                    .send()
                    .await?;

                return Ok(registration.id);
            }
            ServerRegistrationState::Expired => {
                // This server registration expired. We need to wait for the next tick to retry.
            }
            ServerRegistrationState::Terminated => {
                anyhow::bail!(
                    "Server registration terminated. This server can no longer be registered."
                );
            }
        }

        tick.tick().await;
    }
}
