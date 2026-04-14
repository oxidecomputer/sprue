use crate::context::{
    blob::BlobContext, idempotency::IdempotencyContext, oidc::OidcContext,
    server_identity::ServerIdentityContext, service::ServiceContext,
};

pub mod blob;
pub mod idempotency;
pub mod oidc;
pub mod server_identity;
pub mod service;

#[derive(Clone)]
pub struct ApiContext {
    pub public_url: String,
    pub blob: BlobContext,
    pub idempotency: IdempotencyContext,
    pub oidc: OidcContext,
    pub server_identity: ServerIdentityContext,
    pub service: ServiceContext,
}

impl ApiContext {}
