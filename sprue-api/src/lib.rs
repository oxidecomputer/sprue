use dice_verifier::{Corim, CorimError, ReferenceMeasurementsError};
use dropshot::BuildError as DropshotError;
use newtype_uuid::TypedUuid;
use secrecy::ExposeSecret;
use slog::Logger;
use sprue_model::storage::postgres::PostgresStorage;
use std::{
    path::PathBuf,
    process,
    sync::{Arc, Mutex},
    time::Duration,
};
use steno::ActionRegistry;
use thiserror::Error;
use tokio::select;
use v_api::{
    VContextBuilder, VContextBuilderError,
    endpoints::login::oauth::{OAuthProviderName, google::GoogleOAuthProvider},
};
use v_api_param::ParamResolutionError;
use v_model::saga::view::SagaExecNodeId;
use x509_cert::Certificate;

use crate::{
    backup_storage::{OidcTokenFetcher, create_backup_storage},
    context::{
        ApiContextBuilder, ApiContextBuilderError,
        blob::BlobContext,
        idempotency::IdempotencyContext,
        oidc::{OidcContext, OidcContextError},
        server_identity::ServerIdentityContext,
        service::ServiceContext,
    },
    initial_data::{InitError, InitialData},
    sagas::{
        actions::{load_actions, push_backup::PushBackup},
        background::{SagaBackgroundConfig, SagaBackgroundSpawner},
    },
};

mod backup_storage;
pub mod config;
pub mod context;
mod endpoints;
mod initial_data;
pub mod permissions;
mod sagas;
mod server;

pub use config::ServerConfig;
pub use server::{describe, create_server};

#[derive(Debug, Error)]
pub enum ServerError {
    #[error("Failed to build ApiContext")]
    CtxBuilder(#[from] ApiContextBuilderError),
    #[error("Failed to initialize data")]
    InitError(#[from] InitError),
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
    #[error("Task failed")]
    TaskFailed(String),
    #[error("Failed to build VContext")]
    VBuilder(#[from] VContextBuilderError),
}

pub async fn run_server(
    node_id: TypedUuid<SagaExecNodeId>,
    config: ServerConfig,
    param_path: Option<PathBuf>,
    logger: Logger,
) -> Result<(), ServerError> {
    let database_url_secret = config.database_url.resolve(param_path)?;
    let storage = Arc::new(PostgresStorage::create(database_url_secret.expose_secret()).unwrap());

    let mut v_ctx = VContextBuilder::new()
        .with_public_url(config.public_url.clone())
        .with_jwt_expiration(config.jwt.default_expiration)
        .with_storage_url(database_url_secret.expose_secret().to_string())
        .with_keys(config.jwt.keys)
        .with_saga_backend(node_id, None)
        .build()
        .await?;

    // Install OAuth provider
    if let Some(google) = config.authn.oauth.google {
        let device_secret = google
            .device
            .client_secret
            .resolve(config.param_base_path.clone())?;
        let web_secret = google
            .web
            .client_secret
            .resolve(config.param_base_path.clone())?;
        v_ctx.insert_oauth_provider(
            OAuthProviderName::Google,
            Box::new(move || {
                Box::new(GoogleOAuthProvider::new(
                    google.device.client_id.clone(),
                    device_secret.clone(),
                    google.web.client_id.clone(),
                    web_secret.clone(),
                    None,
                ))
            }),
        );

        tracing::info!("Added Google OAuth provider");
    }

    let init_data = InitialData::new(config.initial_mappers.map(|p| vec![p])).map_err(|err| {
        tracing::error!(?err, "Failed to load initial data from configuration");
        err
    })?;
    init_data.initialize(&v_ctx).await.map_err(|err| {
        tracing::error!(?err, "Failed to install initial data");
        err
    })?;

    let v_ctx = Arc::new(v_ctx);
    let v_ctx_token = v_ctx.clone();
    let token_fetcher: OidcTokenFetcher = Arc::new(move |audience: String| {
        let v_ctx_token = v_ctx_token.clone();
        Box::pin(async move { Ok(v_ctx_token.service_token(&audience).await?) })
    });

    let mut saga_actions = ActionRegistry::new();
    load_actions(&mut saga_actions);

    let ctx = ApiContextBuilder::default()
        .public_url(config.public_url)
        .blob(BlobContext::new(
            config.backup.local_root,
            storage.clone(),
            create_backup_storage(config.backup.remote, token_fetcher).await,
        ))
        .idempotency(IdempotencyContext::new(storage.clone()))
        .oidc(OidcContext::new(v_ctx.issuer(), config.oidc, storage.clone())?)
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
        .saga_action_registry(Arc::new(saga_actions))
        .v_ctx(v_ctx)
        .build()?;

    tracing::info!("Starting background saga spawner");
    let (mut saga_spawner, saga_spawner_shutdown) = SagaBackgroundSpawner::new(
        ctx.clone(),
        SagaBackgroundConfig {
            node_id,
            interval: Duration::from_secs(5),
        },
    );
    saga_spawner.add_background_generator(PushBackup);
    let saga_spawner_task = saga_spawner.start();
    set_ctrlc_handler(move || {
        tracing::info!("Received shutdown signal");
        if let Err(err) = saga_spawner_shutdown.send(()) {
            tracing::error!(?err, "Failed to send shutdown signal to saga spawner");
        }
        0
    })
    .expect("Failed to install ctrl+c handler");

    let starter = create_server(ctx, logger, config.port.unwrap_or(8080)).build_starter()?;
    let server_task = starter.start();

    let error = select! {
        v = server_task => v.map_err(|e| ServerError::TaskFailed(e.to_string())),
        v = saga_spawner_task => v.map_err(|e| ServerError::TaskFailed(e.to_string())),
    };

    error
}

pub fn set_ctrlc_handler<F>(f: F) -> Result<(), ctrlc::Error>
where
    F: FnOnce() -> i32 + Send + 'static,
{
    let f = Mutex::new(Some(f));
    ctrlc::set_handler(move || {
        if let Ok(mut guard) = f.lock() {
            let f = guard.take().expect("f can only be taken once");
            let code = f();
            process::exit(code);
        }
    })
}
