// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use newtype_uuid::TypedUuid;
use std::{
    collections::HashMap, error::Error as StdError, future::Future, pin::Pin, sync::Arc,
    time::Duration,
};
use steno::SagaDag;
use tap::TapFallible;
use thiserror::Error;
use tokio::{sync::oneshot, task::JoinHandle};
use v_api::ApiContext as VApiContext;
use v_model::{Permissions, UserId, permissions::Caller, saga::view::SagaExecNodeId};

use crate::{context::ApiContext, permissions::ApiPermissions, sagas::actions::BackgroundSaga};

pub type SagaDagGenerator = Box<
    dyn Fn(
            TypedUuid<UserId>,
            ApiContext,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Vec<Arc<SagaDag>>, Box<dyn StdError + Send + Sync>>>
                    + Send,
            >,
        > + Send
        + Sync,
>;

#[derive(Debug, Clone)]
pub struct SagaBackgroundConfig {
    pub node_id: TypedUuid<SagaExecNodeId>,
    pub interval: Duration,
}

#[derive(Debug, Error)]
pub enum SagaBackgroundSpawnerError {
    #[error("User does not have permissions to create sagas")]
    MissingPermissions,
}

pub struct SagaBackgroundSpawner {
    caller: Caller<ApiPermissions>,
    api_ctx: ApiContext,
    config: SagaBackgroundConfig,
    background_generators: Vec<SagaDagGenerator>,
    shutdown_rx: oneshot::Receiver<()>,
}

impl SagaBackgroundSpawner {
    pub fn new(api_ctx: ApiContext, config: SagaBackgroundConfig) -> (Self, oneshot::Sender<()>) {
        let (shutdown_tx, shutdown_rx) = oneshot::channel();
        (
            Self {
                caller: Caller {
                    id: TypedUuid::new_v4(),
                    permissions: Permissions::default(),
                    extensions: HashMap::default(),
                },
                api_ctx,
                config,
                background_generators: vec![],
                shutdown_rx,
            },
            shutdown_tx,
        )
    }

    pub fn add_background_generator<T, E>(&mut self, generator: T)
    where
        T: BackgroundSaga<E> + 'static,
        E: StdError + Send + Sync + 'static,
    {
        self.background_generators
            .push(Box::new(move |caller_id, ctx| {
                Box::pin(async move {
                    generator
                        .generate_dags(caller_id, &ctx)
                        .await
                        .map_err(|e| Box::new(e) as Box<dyn StdError + Send + Sync>)
                })
            }));
    }

    /// Start the saga initiator.
    ///
    /// Returns a join handle that can be used to wait for completion.
    pub fn start(self) -> JoinHandle<()> {
        tokio::spawn(async move { self.run_loop().await })
    }

    /// Run the polling loop.
    async fn run_loop(mut self) {
        tracing::info!(
            node_id = %self.config.node_id,
            "Starting saga initiator"
        );

        let mut interval = tokio::time::interval(self.config.interval);

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    if let Err(err) = self.spawn_sagas().await {
                        tracing::error!(error = %err, "Failed to spawn sagas");
                    }
                }
                _ = &mut self.shutdown_rx => {
                    tracing::info!("saga initiator received shutdown signal");
                    break;
                }
            }
        }

        tracing::info!("saga initiator stopped");
    }

    async fn spawn_sagas(&self) -> Result<(), SagaBackgroundSpawnerError> {
        let mut to_spawn: Vec<Arc<SagaDag>> = vec![];

        for generator in &self.background_generators {
            match generator(self.caller.id, self.api_ctx.clone()).await {
                Ok(sagas) => to_spawn.extend(sagas),
                Err(err) => tracing::error!(error = ?err, "Failed to generate background sagas"),
            }
        }

        for saga in to_spawn {
            match self
                .api_ctx
                .v_ctx()
                .saga
                .create_saga(
                    &self.caller,
                    saga.clone(),
                    Arc::new(self.api_ctx.clone()),
                    self.api_ctx.saga_action_registry.clone(),
                )
                .map_err(|_| SagaBackgroundSpawnerError::MissingPermissions)?
                .await
            {
                Ok((id, handle)) => {
                    tracing::info!(id = ?id, "Created generated saga");
                    let caller = self.caller.clone();
                    let ctx = self.api_ctx.clone();
                    tokio::spawn(async move {
                        if let Ok(_) =
                            ctx.v_ctx()
                                .saga
                                .start_saga(&caller, id)
                                .await
                                .tap_err(|err| {
                                    tracing::error!(?err, "Failed to start generated saga");
                                })
                        {
                            handle.await;
                        } else {
                            tracing::error!(?id, "Failed to start saga");
                        }
                    });
                }
                Err(err) => tracing::error!(error = ?err, ?saga, "Failed to create generated saga"),
            }
        }

        Ok(())
    }
}
