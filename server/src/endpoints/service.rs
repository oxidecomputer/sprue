use chrono::{DateTime, Utc};
use dropshot::{
    HttpError, HttpResponseOk, HttpResponseUpdatedNoContent, Path, RequestContext, TypedBody,
    endpoint,
};
use model::{
    Blob, HealthCheck, ServerRegistration, ServerRegistrationId, ServerRegistrationInstanceId,
    ServiceId,
};
use newtype_uuid::TypedUuid;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::ApiContext;

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ServiceIdentifier {
    Id(TypedUuid<ServiceId>),
    Name(String),
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegisterServerPath {
    service: String,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegisterServerBody {
    instance: TypedUuid<ServerRegistrationInstanceId>,
}

#[derive(Debug, Serialize, JsonSchema)]
pub struct RegisterServerResponse {
    registration: ServerRegistration,
}

/// Request a server be registered as a representative instance of a service. The registration
/// will need to be accepted before the server can begin check ins or blobs.
#[endpoint {
    method = POST,
    path = "/service/{service}/register",
}]
pub async fn register_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<RegisterServerPath>,
    body: TypedBody<RegisterServerBody>,
) -> Result<HttpResponseOk<RegisterServerResponse>, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    let body = body.into_inner();
    let registration = ctx
        .service
        .register_server(&path.service, body.instance)
        .await?;
    Ok(HttpResponseOk(RegisterServerResponse { registration }))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServerPath {
    pub server: TypedUuid<ServerRegistrationId>,
}

/// Accept a server's request to be added as a representative instance of a service.
#[endpoint {
    method = POST,
    path = "/server/{server}/accept",
}]
pub async fn accept_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    ctx.service.accept_server(path.server).await?;
    Ok(HttpResponseUpdatedNoContent())
}

/// Reject a server's request to be added as a representative instance of a service.
#[endpoint {
    method = POST,
    path = "/server/{server}/reject",
}]
pub async fn reject_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    ctx.service.reject_server(path.server).await?;
    Ok(HttpResponseUpdatedNoContent())
}

/// Remove a server from the pool of representative instances of a service.
#[endpoint {
    method = POST,
    path = "/server/{server}/terminate",
}]
pub async fn terminate_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    ctx.service.terminate_server(path.server).await?;
    Ok(HttpResponseUpdatedNoContent())
}

#[derive(Debug, Deserialize, JsonSchema)]
struct CheckinBody {
    checked_in_at: DateTime<Utc>,
}

/// Report a check in of a server for aliveness checks.
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
    let path = path.into_inner();
    let body = body.into_inner();
    let record = ctx.service.checkin(path.server, body.checked_in_at).await?;
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

/// Register a new blob request to upload a blob to. Returns a blob instance that the
/// requesting server is authorized to upload to.
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
    let path = path.into_inner();
    let body = body.into_inner();

    ctx.idempotency
        .execute_idempotent_request(path.server, body.idempotency_key, |_| async move {
            let server = ctx.service.get_server(path.server).await?;
            let blob = ctx.blob.create_blob(&server, body.size).await?;
            let response = RegisterBlobResponse { blob };
            Ok(response)
        })
        .await
}
