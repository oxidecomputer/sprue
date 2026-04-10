use dice_verifier::{Corim, CorimError, ReferenceMeasurementsError};
use dropshot::{BuildError, HttpServerStarter};
use model::storage::postgres::PostgresStorage;
use secrecy::ExposeSecret;
use slog::Logger;
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use v_api_param::ParamResolutionError;
use x509_cert::Certificate;

use crate::context::{
    ApiContext,
    blob::BlobContext,
    idempotency::IdempotencyContext,
    oidc::{OidcContext, OidcContextError, OidcJwtContext},
    service::ServiceContext,
};

mod config;
pub mod context;
mod endpoints;
mod server;

pub use config::ServerConfig;
pub use server::describe;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Failed to create server")]
    FailedToCreate(#[from] BuildError),
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
}

pub fn create_server(
    config: ServerConfig,
    param_path: Option<PathBuf>,
    logger: Logger,
) -> Result<HttpServerStarter<ApiContext>, ServerError> {
    let storage = Arc::new(
        PostgresStorage::create(config.database_url.resolve(param_path)?.expose_secret()).unwrap(),
    );
    let ctx = ApiContext {
        public_url: config.public_url,
        blob: BlobContext::new(storage.clone()),
        idempotency: IdempotencyContext::new(storage.clone()),
        oidc: OidcContext::new(
            Certificate::load_pem_chain(config.root_cert_chain.as_bytes())?,
            TryFrom::<&[Corim]>::try_from(
                &config
                    .measurements
                    .into_iter()
                    .map(|p| {
                        let data = std::fs::read(p)?;
                        Ok::<_, ServerError>(Corim::from_bytes(&data)?)
                    })
                    .collect::<Result<Vec<_>, _>>()?,
            )?,
            OidcJwtContext {
                kid: config.kid,
                public: config.public,
                private: config.private,
            },
        )?,
        service: ServiceContext::new(storage),
    };
    let starter = server::server(ctx, logger);
    Ok(starter?)
}
