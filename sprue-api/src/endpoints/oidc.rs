// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use dropshot::{HttpError, HttpResponseOk, Path, RequestContext, TypedBody, endpoint};
use newtype_uuid::TypedUuid;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sprue_model::{TokenRequest, TokenRequestId};
use v_api::ApiContext as VApiContext;
use vm_attest::VmInstanceAttestation;

use crate::{context::ApiContext, endpoints::service::ServerPath};

/// Register a request for a server OIDC token.
///
/// The server will be issued a challenge that it must sign and return to prove its identity.
#[endpoint {
    method = POST,
    path = "/server/{server}/oidc/token",
}]
pub async fn register_oidc_token_request(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
) -> Result<HttpResponseOk<TokenRequest>, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    let server = ctx
        .service
        .get_server(ctx.system_caller(), path.server)
        .await?;
    let nonce = ctx.server_identity.generate_nonce()?;

    let token_request = ctx.oidc.register_token_request(server, nonce).await?;
    Ok(HttpResponseOk(token_request))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServerAttestation {
    request: TypedUuid<TokenRequestId>,
    attestation: Value,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OidcServerToken {
    token: String,
}

/// Complete a server OIDC token request
///
/// Server must provide an attestation that proves the server's identity along with the challenge
/// that was issued in the initial request.
#[endpoint {
    method = POST,
    path = "/server/{server}/oidc/token/prove",
}]
pub async fn prove_oidc_token_request(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
    body: TypedBody<ServerAttestation>,
) -> Result<HttpResponseOk<OidcServerToken>, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    let body = body.into_inner();
    let server = ctx
        .service
        .get_server(ctx.system_caller(), path.server)
        .await?;
    let attestation: VmInstanceAttestation =
        serde_json::from_value(body.attestation).map_err(|err| {
            tracing::info!(?err, "Unable to deserialize attestation");
            HttpError::for_bad_request(None, "Failed to deserialize attestation".to_string())
        })?;

    // Verify the attestation
    ctx.server_identity
        .verify_attestation(&server, &attestation)
        .map_err(|_| HttpError::for_internal_error("Failed to verify attestation".to_string()))?;

    let token_request = ctx.oidc.get_token_request(body.request).await?;
    let claims = ctx.oidc.generate_claims(&server, token_request).await?;
    let token = ctx.v_ctx().sign_jwt(&claims).await.map_err(|err| {
        tracing::error!(?err, "Unable to sign claims");
        HttpError::for_internal_error("Failed to sign claims".to_string())
    })?;

    Ok(HttpResponseOk(OidcServerToken { token }))
}
