// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::anyhow;
use sprue_sdk::Client;
use vm_attest::QualifyingData;

use crate::{platform::Platform, registration::Standing};

/// Answer a registration challenge for the named service and return the
/// standing that sprue reports. Sprue builds the registration from the
/// attested identity, so this VM never reports its own identity. An instance
/// that is already registered receives its existing registration, and an
/// instance without one is registered.
pub async fn register_server(
    client: &Client,
    service: &str,
    platform: &dyn Platform,
) -> anyhow::Result<Standing> {
    let challenge = client
        .register_server()
        .service(service.to_string())
        .send()
        .await?
        .into_inner();

    let bytes: [u8; 32] = hex::decode(&challenge.nonce)?
        .try_into()
        .map_err(|v: Vec<u8>| anyhow!("expected 32 byte nonce, got {}", v.len()))?;
    let attestation = platform.attest(&QualifyingData::from(bytes))?;

    client
        .prove_registration()
        .challenge(challenge.id)
        .body_map(|body| body.attestation(attestation))
        .send()
        .await?
        .into_inner()
        .try_into()
}
