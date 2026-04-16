use dice_verifier::{Corim, CorimError, ReferenceMeasurementsError};
use dropshot::{BuildError as DropshotError, HttpServerStarter};
use model::storage::postgres::PostgresStorage;
use secrecy::ExposeSecret;
use slog::Logger;
use std::{path::PathBuf, sync::Arc, time::Duration};
use thiserror::Error;
use v_api::{VContextBuilder, VContextBuilderError};
use v_api_param::ParamResolutionError;
use x509_cert::Certificate;

use crate::context::{
    ApiContext, ApiContextBuilder, ApiContextBuilderError,
    blob::BlobContext,
    idempotency::IdempotencyContext,
    oidc::{OidcContext, OidcContextError},
    server_identity::ServerIdentityContext,
    service::ServiceContext,
};

mod config;
pub mod context;
mod endpoints;
mod permissions;
mod server;

pub use config::ServerConfig;
pub use server::describe;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Failed to build ApiContext")]
    CtxBuilder(#[from] ApiContextBuilderError),
    #[error("Failed to create server")]
    FailedToCreate(#[from] DropshotError),
    #[error("Failed to parse measurement")]
    Measurement(#[from] CorimError),
    #[error("Failed to create OidcContext")]
    Oidc(#[from] OidcContextError),
    #[error("Failed to resolve paramater")]
    ParamResolution(#[from] ParamResolutionError),
    #[error("Failed to read measurement file")]
    Read(#[from] std::io::Error),
    #[error("Failed to construct reference measurement")]
    ReferenceMeasurement(#[from] ReferenceMeasurementsError),
    #[error("Failed to load root cert chain")]
    RootCertChain(#[from] x509_cert::der::Error),
    #[error("Failed to build VContext")]
    VBuilder(#[from] VContextBuilderError),
}

pub async fn create_server(
    config: ServerConfig,
    param_path: Option<PathBuf>,
    logger: Logger,
) -> Result<HttpServerStarter<ApiContext>, ServerError> {
    let database_url_secret = config.database_url.resolve(param_path)?;
    let storage = Arc::new(PostgresStorage::create(database_url_secret.expose_secret()).unwrap());

    let v_ctx = VContextBuilder::new()
        .with_public_url(config.public_url.clone())
        .with_jwt_expiration(config.jwt.default_expiration)
        .with_storage_url(database_url_secret.expose_secret().to_string())
        .with_keys(config.jwt.keys)
        .build()
        .await?;

    let ctx = ApiContextBuilder::default()
        .public_url(config.public_url)
        .blob(BlobContext::new(config.backup.local_root, storage.clone()))
        .idempotency(IdempotencyContext::new(storage.clone()))
        .oidc(OidcContext::new(config.oidc, storage.clone())?)
        .server_identity(ServerIdentityContext::new(
            config.vm_identity.organization,
            Certificate::load_pem_chain(config.vm_identity.root_cert_chain.as_bytes())?,
            Arc::new(TryFrom::<&[Corim]>::try_from(
                &config
                    .vm_identity
                    .measurements
                    .into_iter()
                    .map(|p| {
                        let data = std::fs::read(p)?;
                        Ok::<_, ServerError>(Corim::from_bytes(&data)?)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )?),
        ))
        .service(ServiceContext::new(
            storage,
            Duration::from_secs(config.vm_identity.registration_duration),
        ))
        .v_ctx(Arc::new(v_ctx))
        .build()?;

    let starter = server::server(ctx, logger);
    Ok(starter?)
}
