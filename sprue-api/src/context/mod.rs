// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use derive_builder::Builder;
use dropshot::{HttpError, RequestContext};
use newtype_uuid::TypedUuid;
use sprue_model::{
    ServerRegistrationId, ServerRegistrationInstanceId, ServerRegistrationState, ServiceId,
};
use sprue_svc::SprueServiceClient;
use std::{collections::HashMap, fmt::Debug, path::PathBuf, sync::Arc};
use steno::ActionRegistry;
use tarpc::tokio_serde::formats::Json;
use thiserror::Error;
use tokio::sync::OnceCell;
use v_api::response::ResourceError;
use v_api::{
    ApiContext as VApiContext, VContext,
    authn::jwt::{Jwt, JwtError},
};
use v_model::permissions::Caller;

use crate::{
    context::{
        blob::BlobContext,
        idempotency::IdempotencyContext,
        oidc::{OidcContext, VmClaims},
        policy::PolicyEngine,
        server_identity::ServerIdentityContext,
        service::ServiceContext,
    },
    permissions::ApiPermissions,
    sagas::SprueSaga,
};

pub mod blob;
pub mod idempotency;
pub mod oidc;
pub mod policy;
pub mod server_identity;
pub mod service;

#[derive(Debug, Error)]
pub enum ApiContextError {
    #[error("Failed to extract JWT")]
    FailedToExtractJwt(#[from] JwtError),
    #[error("Failed to look up the server registration")]
    RegistrationLookup,
    #[error("Server registration is not accepted")]
    RegistrationNotAccepted,
}

#[derive(Builder, Clone)]
#[builder(pattern = "owned")]
pub struct ApiContext {
    #[builder(default = "self.default_system_caller()")]
    system_caller: Caller<ApiPermissions>,
    pub public_url: String,
    pub blob: BlobContext,
    pub idempotency: IdempotencyContext,
    pub oidc: OidcContext,
    #[builder(default)]
    pub policy: Option<PolicyEngine>,
    pub server_identity: ServerIdentityContext,
    pub service: ServiceContext,
    pub saga_action_registry: Arc<ActionRegistry<SprueSaga>>,
    v_ctx: Arc<VContext<ApiPermissions>>,
    #[builder(default)]
    sprue: Option<LazySprueClient>,
}

impl Debug for ApiContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ApiContext")
            .field("public_url", &self.public_url)
            .finish()
    }
}

impl ApiContextBuilder {
    fn default_system_caller(&self) -> Caller<ApiPermissions> {
        Caller {
            id: TypedUuid::new_v4(),
            permissions: vec![
                ApiPermissions::GetApiUsersAll,
                ApiPermissions::ManageApiUsersAll,
                ApiPermissions::GetServicesAll,
                ApiPermissions::ManageServicesAll,
            ]
            .into(),
            extensions: HashMap::default(),
        }
    }
}

pub struct ServerCaller {
    pub id: TypedUuid<ServerRegistrationId>,
    pub service: TypedUuid<ServiceId>,
    pub instance: TypedUuid<ServerRegistrationInstanceId>,
}

impl ApiContext {
    pub fn system_caller(&self) -> &Caller<ApiPermissions> {
        &self.system_caller
    }

    pub async fn sprue_client(&self) -> Option<&SprueServiceClient> {
        match &self.sprue {
            Some(lazy) => lazy.get().await.ok(),
            None => None,
        }
    }

    pub async fn get_server_caller(
        &self,
        rqctx: &RequestContext<ApiContext>,
    ) -> Result<ServerCaller, ApiContextError> {
        let jwt: Jwt<VmClaims> = Jwt::extract(rqctx).await?;

        // A token is only honored while the registration it was issued for is
        // still accepted. A terminated or deleted server loses access at once
        // instead of when its token expires.
        match self
            .service
            .get_server(&self.system_caller, jwt.claims.sub)
            .await
        {
            Ok(server) if server.state == ServerRegistrationState::Accepted => Ok(ServerCaller {
                id: jwt.claims.sub,
                service: jwt.claims.svc,
                instance: jwt.claims.oxc.ins,
            }),
            Ok(_) | Err(ResourceError::DoesNotExist) => {
                Err(ApiContextError::RegistrationNotAccepted)
            }
            Err(err) => {
                tracing::error!(?err, "Failed to look up server registration for token");
                Err(ApiContextError::RegistrationLookup)
            }
        }
    }
}

impl VApiContext for ApiContext {
    type AppPermissions = ApiPermissions;
    fn v_ctx(&self) -> &VContext<Self::AppPermissions> {
        &self.v_ctx
    }
}

impl From<ApiContextError> for HttpError {
    fn from(err: ApiContextError) -> Self {
        tracing::warn!(?err, "Api context error");
        match err {
            ApiContextError::RegistrationNotAccepted => HttpError::for_client_error(
                None,
                dropshot::ClientErrorStatusCode::FORBIDDEN,
                "Server registration is not accepted".to_string(),
            ),
            // A broken signing key is a fault in sprue. Every other token
            // error means the caller sent a token that sprue does not accept.
            ApiContextError::FailedToExtractJwt(JwtError::InvalidJwk(_))
            | ApiContextError::RegistrationLookup => {
                HttpError::for_internal_error("Internal server error".to_string())
            }
            ApiContextError::FailedToExtractJwt(_) => HttpError::for_client_error(
                None,
                dropshot::ClientErrorStatusCode::UNAUTHORIZED,
                "Invalid token".to_string(),
            ),
        }
    }
}

/// A lazily-initialized connection to the sprue-agent unix socket.
///
/// The sprue-agent may not be running when sprue-api starts, so the tarpc
/// transport is established on first use rather than at startup.
#[derive(Clone)]
pub struct LazySprueClient {
    socket_path: PathBuf,
    client: Arc<OnceCell<SprueServiceClient>>,
}

impl LazySprueClient {
    pub fn new(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            client: Arc::new(OnceCell::new()),
        }
    }

    pub async fn get(&self) -> Result<&SprueServiceClient, anyhow::Error> {
        self.client
            .get_or_try_init(|| async {
                tracing::info!(path = ?self.socket_path, "Connecting to sprue-agent");
                let transport =
                    tarpc::serde_transport::unix::connect(&self.socket_path, Json::default).await?;
                let client =
                    SprueServiceClient::new(tarpc::client::Config::default(), transport).spawn();
                Ok(client)
            })
            .await
    }
}
