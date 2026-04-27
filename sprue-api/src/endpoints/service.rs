// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use chrono::{DateTime, Utc};
use dropshot::{
    HttpError, HttpResponseOk, HttpResponseUpdatedNoContent, Path, RequestContext, TypedBody,
    endpoint,
};
use newtype_uuid::TypedUuid;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sprue_model::{
    Blob, HealthCheck, ServerRegistration, ServerRegistrationId, ServerRegistrationInstanceId,
    Service, ServiceId,
};
use v_api::ApiContext as VApiContext;
use vm_attest::VmInstanceAttestation;

use crate::context::ApiContext;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServicePath {
    service: TypedUuid<ServiceId>,
}

/// Get a service by its id
#[endpoint {
    method = GET,
    path = "/service/{service}",
}]
pub async fn get_service(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServicePath>,
) -> Result<HttpResponseOk<Service>, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let service = ctx.service.get_service(&caller, path.service).await?;

    Ok(HttpResponseOk(service))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateService {
    name: String,
}

/// Create a new service
#[endpoint {
    method = POST,
    path = "/service",
}]
pub async fn create_service(
    rqctx: RequestContext<ApiContext>,
    path: TypedBody<CreateService>,
) -> Result<HttpResponseOk<Service>, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let service = ctx.service.create_service(&caller, &path.name).await?;

    Ok(HttpResponseOk(service))
}

/// Get all servers registered for a service
#[endpoint {
    method = GET,
    path = "/service/{service}/server",
}]
pub async fn get_service_servers(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServicePath>,
) -> Result<HttpResponseOk<Vec<ServerRegistration>>, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let servers = ctx
        .service
        .get_service_servers(&caller, path.service)
        .await?;

    Ok(HttpResponseOk(servers))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegisterServerBody {
    instance: TypedUuid<ServerRegistrationInstanceId>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RegisterServerResponse {
    registration: ServerRegistration,
}

/// Request a server be registered as a representative instance of a service
///
/// The server will need to prove its identity via an attestation. Once its identity is verified
/// the server will need to either be accepted by policy or manual intervention.
#[endpoint {
    method = POST,
    path = "/service/{service}/register",
}]
pub async fn register_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServicePath>,
    body: TypedBody<RegisterServerBody>,
) -> Result<HttpResponseOk<RegisterServerResponse>, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    let body = body.into_inner();
    let nonce = ctx.server_identity.generate_nonce()?;
    let registration = ctx
        .service
        .register_server(ctx.system_caller(), path.service, body.instance, nonce)
        .await?;

    Ok(HttpResponseOk(RegisterServerResponse { registration }))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServerPath {
    pub server: TypedUuid<ServerRegistrationId>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServerAttestation {
    attestation: Value,
}

/// Prove the identity of a server
#[endpoint {
    method = POST,
    path = "/server/{server}/prove",
}]
pub async fn prove_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
    body: TypedBody<ServerAttestation>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
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

    // The server has proven its identity and we can mark it as proven
    ctx.service
        .prove_server(ctx.system_caller(), &server)
        .await
        .map_err(|err| {
            // TODO: Currently all failures are emited as bad request errors. This is not strictly
            // correct as we may have a misconfigured server. From the stance of the server though this
            // request does not match what it is expecting
            tracing::info!(?err, "Failed to prove server");
            HttpError::for_bad_request(None, "Failed to prove server".to_string())
        })?;

    Ok(HttpResponseUpdatedNoContent())
}

/// Accept a server's request to be added as a representative instance of a service
#[endpoint {
    method = POST,
    path = "/server/{server}/accept",
}]
pub async fn accept_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let server = ctx.service.get_server(&caller, path.server).await?;
    ctx.service.accept_server(&caller, &server).await?;
    Ok(HttpResponseUpdatedNoContent())
}

/// Reject a server's request to be added as a representative instance of a service
#[endpoint {
    method = POST,
    path = "/server/{server}/reject",
}]
pub async fn reject_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let server = ctx.service.get_server(&caller, path.server).await?;
    ctx.service.reject_server(&caller, &server).await?;
    Ok(HttpResponseUpdatedNoContent())
}

/// Remove a server from the pool of representative instances of a service
#[endpoint {
    method = POST,
    path = "/server/{server}/terminate",
}]
pub async fn terminate_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let server = ctx.service.get_server(&caller, path.server).await?;
    ctx.service.terminate_server(&caller, &server).await?;
    Ok(HttpResponseUpdatedNoContent())
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CheckinBody {
    checked_in_at: DateTime<Utc>,
}

/// Report a check in of a server for aliveness checks
#[endpoint {
    method = POST,
    path = "/server/{server}/checkin",
}]
pub async fn checkin_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
    body: TypedBody<CheckinBody>,
) -> Result<HttpResponseOk<HealthCheck>, HttpError> {
    let ctx = rqctx.context();
    let caller = ctx.get_server_caller(&rqctx).await?;
    let path = path.into_inner();
    let body = body.into_inner();
    let record = ctx
        .service
        .checkin(&caller, path.server, body.checked_in_at)
        .await?;
    Ok(HttpResponseOk(record))
}

#[derive(Debug, Deserialize, JsonSchema)]
struct RegisterBlobBody {
    size: i64,
    idempotency_key: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
struct RegisterBlobResponse {
    blob: Blob,
}

/// Register a new blob request to upload a blob to
///
/// Returns a blob instance that the requesting server is authorized to upload to.
#[endpoint {
    method = POST,
    path = "/server/{server}/blob/register",
}]
pub async fn register_blob(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
    body: TypedBody<RegisterBlobBody>,
) -> Result<HttpResponseOk<RegisterBlobResponse>, HttpError> {
    let ctx = rqctx.context();
    let caller = ctx.get_server_caller(&rqctx).await?;
    let path = path.into_inner();
    let body = body.into_inner();

    ctx.idempotency
        .execute_idempotent_request(path.server, body.idempotency_key, |_| async move {
            let blob = ctx
                .blob
                .create_blob(caller.id, caller.service, body.size)
                .await?;
            let response = RegisterBlobResponse { blob };
            Ok(response)
        })
        .await
}
