use crate::context::{
    blob::BlobContext, idempotency::IdempotencyContext, oidc::OidcContext, service::ServiceContext,
};

pub mod blob;
pub mod idempotency;
pub mod oidc;
pub mod service;

#[derive(Clone)]
pub struct ApiContext {
    pub public_url: String,
    pub blob: BlobContext,
    pub idempotency: IdempotencyContext,
    pub oidc: OidcContext,
    pub service: ServiceContext,
}

impl ApiContext {}
