use dropshot::{HttpError, HttpResponseOk, Path, RequestContext, TypedBody, endpoint};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use vm_attest::{QualifyingData, VmInstanceAttestation};

use crate::{context::ApiContext, endpoints::service::ServerPath};

#[derive(Debug, Serialize, JsonSchema)]
pub struct OidcServerTokenNonce {
    nonce: String,
}

#[endpoint {
    method = POST,
    path = "/server/{server}/oidc/token",
}]
pub async fn register_oidc_token_request(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
) -> Result<HttpResponseOk<OidcServerTokenNonce>, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    let server = ctx.service.get_server(path.server).await?;
    let nonce = QualifyingData::from_platform_rng()
        .map_err(|_| HttpError::for_internal_error("Failed to generate nonce".to_string()))?;
    ctx.oidc.store_nonce(server.id, server.instance_id, nonce.clone());
    Ok(HttpResponseOk(OidcServerTokenNonce {
        nonce: hex::encode(nonce.into_inner()),
    }))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServerAttestation {
    attestation: Value,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct OidcServerToken {
    token: String,
}

#[endpoint {
    method = POST,
    path = "/server/{server}/oidc/token/prove",
}]
pub async fn prove_oidc_token_request(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
    body: TypedBody<ServerAttestation>,
) -> Result<HttpResponseOk<OidcServerToken>, HttpError> {
    let path = path.into_inner();
    let body = body.into_inner();
    let attestation: VmInstanceAttestation = serde_json::from_value(body.attestation).map_err(|err| {
        tracing::info!(?err, "Unable to deserialize attestation");
        HttpError::for_bad_request(None, "Failed to deserialize attestation".to_string())
    })?;
    let token = rqctx
        .context()
        .oidc
        .exchange_platform_attestation(&attestation, path.server)
        .map_err(|_| HttpError::for_internal_error("Failed to verify attestation".to_string()))?;
    match token {
        Some(token) => Ok(HttpResponseOk(OidcServerToken { token })),
        None => Err(HttpError::for_bad_request(
            None,
            "Malformed attestation".to_string(),
        )),
    }
}
