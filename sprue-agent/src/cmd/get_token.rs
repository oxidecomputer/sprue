use anyhow::anyhow;
use sprue_sdk::{Client, types::TypedUuidForServerRegistrationId};
use vm_attest::QualifyingData;

use crate::{oidc::validate_jwt, platform::Platform};

pub struct TokenRequest<'a> {
    pub client: &'a Client,
    pub registration_id: TypedUuidForServerRegistrationId,
    pub platform: &'a dyn Platform,
    pub issuer: &'a str,
}

pub async fn get_token<'a>(request: TokenRequest<'a>) -> anyhow::Result<String> {
    let TokenRequest {
        client,
        registration_id,
        platform,
        issuer,
    } = request;

    // Send our id to the server to register a token flow and receive back a nonce to prove
    // ownership of this challenge
    let response = client
        .register_oidc_token_request()
        .server(registration_id.clone())
        .send()
        .await?
        .into_inner();

    let nonce = response.nonce.as_ref().ok_or_else(|| {
        tracing::error!("Token request did not return a challenge nonce");
        anyhow!("No nonce returned")
    })?;
    let bytes: [u8; 32] = hex::decode(nonce)?.try_into().map_err(|v: Vec<u8>| {
        tracing::error!(len = v.len(), "Expected 32 byte nonce");
        anyhow!("Expected 32 byte nonce, got {}", v.len())
    })?;
    let attestation = platform
        .attest(&QualifyingData::from(bytes))
        .map_err(|err| {
            tracing::error!(?err, "Failed to construct attestation");
            err
        })?;

    // Send the attestation back to the server to complete the challenge. The server will
    // verify that the id of the vm in the attestation matches the id of the vm we sent.
    let token_response = client
        .prove_oidc_token_request()
        .server(registration_id)
        .body_map(|body| body.request(response.id).attestation(attestation))
        .send()
        .await
        .map_err(|err| {
            tracing::error!(?err, "Failed to exchange attestation for token");
            err
        })?;

    let token = token_response.into_inner().token;

    // Fetch the JWKS from the server to validate the token
    let jwks_response = client.jwks_json().send().await.map_err(|err| {
        tracing::error!(?err, "Failed to retrieve JWKS to validate token");
        err
    })?;
    let jwks = jwks_response.into_inner();

    // Validate the JWT against the JWKS
    validate_jwt(issuer, &token, &jwks).map_err(|err| {
        tracing::error!(?err, "Failed to validate token against JWKS");
        err
    })?;

    Ok(token.to_string())
}
