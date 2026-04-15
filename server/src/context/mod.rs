use std::sync::Arc;

use derive_builder::Builder;
use v_api::{ApiContext as VApiContext, VContext};

use crate::{
    context::{
        blob::BlobContext, idempotency::IdempotencyContext, oidc::OidcContext,
        server_identity::ServerIdentityContext, service::ServiceContext,
    },
    permissions::ApiPermissions,
};

pub mod blob;
pub mod idempotency;
pub mod oidc;
pub mod server_identity;
pub mod service;

#[derive(Builder, Clone)]
#[builder(pattern = "owned")]
pub struct ApiContext {
    pub public_url: String,
    pub blob: BlobContext,
    pub idempotency: IdempotencyContext,
    pub oidc: OidcContext,
    pub server_identity: ServerIdentityContext,
    pub service: ServiceContext,
    v_ctx: Arc<VContext<ApiPermissions>>,
}

impl ApiContext {}

impl VApiContext for ApiContext {
    type AppPermissions = ApiPermissions;
    fn v_ctx(&self) -> &VContext<Self::AppPermissions> {
        &self.v_ctx
    }
}
