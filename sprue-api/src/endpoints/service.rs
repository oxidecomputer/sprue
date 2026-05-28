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
use sprue_model::{
    Blob, Deployment, DeploymentId, HealthCheck, ProjectId, ServerRegistration,
    ServerRegistrationId, ServerRegistrationInstanceId, Service, ServiceIdentifier, SiloId,
};
use v_api::ApiContext as VApiContext;

use crate::{
    context::{ApiContext, policy::PolicyDecision},
    permissions::ApiPermissions,
    schema::Attestation,
};

/// Path extractor for endpoints under `/service/{service}`.
///
/// The `service` segment accepts either a service UUID or a service name.
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServicePath {
    service: ServiceIdentifier,
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
    let service = ctx.service.resolve_service(&caller, &path.service).await?;

    Ok(HttpResponseOk(service))
}

/// List services
#[endpoint {
    method = GET,
    path = "/service",
}]
pub async fn list_services(
    rqctx: RequestContext<ApiContext>,
) -> Result<HttpResponseOk<Vec<Service>>, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let services = ctx.service.list_services(&caller).await?;

    Ok(HttpResponseOk(services))
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

    // Once the service has been created, grant the caller permission to manage it
    let manage_service_permissions = vec![
        ApiPermissions::GetService(service.id.clone()),
        ApiPermissions::ManageService(service.id.clone()),
    ];
    ctx.v_ctx().user.add_permissions_to_user(ctx.system_caller(), &caller.id, manage_service_permissions.into()).await.map_err(|err| {
      tracing::error!(?err, system = ?ctx.system_caller(), ?caller, "Failed to assign manage service permission");
      HttpError::for_internal_error("Failed to assign permission".to_string())
    })?;

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
    let service = ctx.service.resolve_service(&caller, &path.service).await?;
    let servers = ctx.service.get_service_servers(&caller, service.id).await?;

    Ok(HttpResponseOk(servers))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct RegisterServerBody {
    instance: TypedUuid<ServerRegistrationInstanceId>,
    project_id: TypedUuid<ProjectId>,
    silo_id: TypedUuid<SiloId>,
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
    let service = ctx
        .service
        .resolve_service(&ctx.system_caller(), &path.service)
        .await?;
    let nonce = ctx.server_identity.generate_nonce()?;
    let registration = ctx
        .service
        .register_server(
            ctx.system_caller(),
            service.id,
            body.instance,
            body.project_id,
            body.silo_id,
            nonce,
        )
        .await?;

    Ok(HttpResponseOk(RegisterServerResponse { registration }))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServerPath {
    pub server: TypedUuid<ServerRegistrationId>,
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct ServerAttestation {
    attestation: Attestation,
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
    let attestation = body.into_inner().attestation;
    let server = ctx
        .service
        .get_server(ctx.system_caller(), path.server)
        .await?;

    // Verify the attestation
    ctx.server_identity
        .verify_instance_attestation(
            server.instance_id,
            &server.nonce.as_deref().ok_or_else(|| {
                HttpError::for_bad_request(
                    None,
                    "Registration is not in a state that can be proven".to_string(),
                )
            })?,
            &attestation,
        )
        .map_err(|err| {
            tracing::info!(?err, "Failed to verify attestation");
            HttpError::for_bad_request(None, "Invalid attestation".to_string())
        })?;

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

    // If a registration policy engine is configured, evaluate the policy to
    // automatically accept or reject the server now that its identity is proven.
    if let Some(ref policy) = ctx.policy {
        // Retrieve the service and all known deployments for the service that this instance is
        // attempting to register against.
        let service = ctx
            .service
            .get_service(ctx.system_caller(), server.service_id)
            .await?;
        let deployments = ctx
            .service
            .list_deployments(ctx.system_caller(), server.service_id)
            .await?;

        match policy.evaluate_server_auto_registration(&server, &service, deployments.iter()) {
            Ok(PolicyDecision::Accept) => {
                tracing::info!(
                    server = ?server.id,
                    "Policy engine auto-accepting server registration"
                );

                // We need to refetch the server with its updated state
                let server = ctx
                    .service
                    .get_server(ctx.system_caller(), server.id)
                    .await?;
                ctx.service
                    .accept_server(ctx.system_caller(), &server)
                    .await
                    .map_err(|err| {
                        tracing::error!(?err, "Failed to auto-accept server via policy");
                        HttpError::for_internal_error(
                            "Failed to apply registration policy".to_string(),
                        )
                    })?;
            }
            Ok(PolicyDecision::Reject) => {
                tracing::info!(
                    server = ?server.id,
                    "Policy engine did not allow auto-accepting server registration; server remains in proven state"
                );
            }
            Err(err) => {
                tracing::error!(
                    ?err,
                    server = ?server.id,
                    "Policy evaluation failed; server remains in proven state"
                );
            }
        }
    }

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

/// Delete a server registration
#[endpoint {
    method = DELETE,
    path = "/server/{server}",
}]
pub async fn delete_server(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServerPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let server = ctx.service.get_server(&caller, path.server).await?;
    ctx.service.delete_server(&caller, &server).await?;
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
    blob_time: Option<DateTime<Utc>>,
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
                .create_blob(caller.id, caller.service, body.size, body.blob_time)
                .await?;
            let response = RegisterBlobResponse { blob };
            Ok(response)
        })
        .await
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct CreateDeploymentBody {
    project_id: TypedUuid<ProjectId>,
    silo_id: TypedUuid<SiloId>,
}

/// Create a new deployment for a service
///
/// A deployment represents a project/silo pair where the service is deployed.
#[endpoint {
    method = POST,
    path = "/service/{service}/deployment",
}]
pub async fn create_deployment(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServicePath>,
    body: TypedBody<CreateDeploymentBody>,
) -> Result<HttpResponseOk<Deployment>, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let body = body.into_inner();
    let service = ctx.service.resolve_service(&caller, &path.service).await?;
    let deployment = ctx
        .service
        .create_deployment(&caller, service.id, body.project_id, body.silo_id)
        .await?;

    Ok(HttpResponseOk(deployment))
}

/// List all deployments for a service
#[endpoint {
    method = GET,
    path = "/service/{service}/deployment",
}]
pub async fn list_deployments(
    rqctx: RequestContext<ApiContext>,
    path: Path<ServicePath>,
) -> Result<HttpResponseOk<Vec<Deployment>>, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let service = ctx.service.resolve_service(&caller, &path.service).await?;
    let deployments = ctx.service.list_deployments(&caller, service.id).await?;

    Ok(HttpResponseOk(deployments))
}

#[derive(Debug, Deserialize, JsonSchema)]
pub struct DeploymentPath {
    #[allow(dead_code)]
    service: ServiceIdentifier,
    deployment: TypedUuid<DeploymentId>,
}

/// Get a deployment by its id
#[endpoint {
    method = GET,
    path = "/service/{service}/deployment/{deployment}",
}]
pub async fn get_deployment(
    rqctx: RequestContext<ApiContext>,
    path: Path<DeploymentPath>,
) -> Result<HttpResponseOk<Deployment>, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let deployment = ctx.service.get_deployment(&caller, path.deployment).await?;

    Ok(HttpResponseOk(deployment))
}

/// Delete a deployment from a service
#[endpoint {
    method = DELETE,
    path = "/service/{service}/deployment/{deployment}",
}]
pub async fn delete_deployment(
    rqctx: RequestContext<ApiContext>,
    path: Path<DeploymentPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let caller = rqctx.v_ctx().get_caller(&rqctx).await?;
    let path = path.into_inner();
    let deployment = ctx.service.get_deployment(&caller, path.deployment).await?;
    ctx.service.delete_deployment(&caller, &deployment).await?;
    Ok(HttpResponseUpdatedNoContent())
}
