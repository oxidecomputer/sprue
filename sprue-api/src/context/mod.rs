use derive_builder::Builder;
use dropshot::{HttpError, RequestContext};
use newtype_uuid::TypedUuid;
use sprue_model::{ServerRegistrationId, ServerRegistrationInstanceId, ServiceId};
use std::{collections::HashMap, fmt::Debug, sync::Arc};
use steno::ActionRegistry;
use thiserror::Error;
use v_api::{
    ApiContext as VApiContext, VContext,
    authn::jwt::{Jwt, JwtError},
};
use v_model::{Permissions, permissions::Caller};

use crate::{
    context::{
        blob::BlobContext,
        idempotency::IdempotencyContext,
        oidc::{OidcContext, VmClaims},
        server_identity::ServerIdentityContext,
        service::ServiceContext,
    },
    permissions::ApiPermissions,
    sagas::SprueSaga,
};

pub mod blob;
pub mod idempotency;
pub mod oidc;
pub mod server_identity;
pub mod service;

#[derive(Debug, Error)]
pub enum ApiContextError {
    #[error("Failed to extract JWT")]
    FailedToExtractJwt(#[from] JwtError),
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
    pub server_identity: ServerIdentityContext,
    pub service: ServiceContext,
    pub saga_action_registry: Arc<ActionRegistry<SprueSaga>>,
    v_ctx: Arc<VContext<ApiPermissions>>,
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
            permissions: Permissions::default(),
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

    pub async fn get_server_caller(
        &self,
        rqctx: &RequestContext<ApiContext>,
    ) -> Result<ServerCaller, ApiContextError> {
        let jwt: Jwt<VmClaims> = Jwt::extract(rqctx).await?;
        Ok(ServerCaller {
            id: jwt.claims.sub,
            service: jwt.claims.srv,
            instance: jwt.claims.ox.ins,
        })
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
        HttpError::for_internal_error("Internal server error".to_string())
    }
}
