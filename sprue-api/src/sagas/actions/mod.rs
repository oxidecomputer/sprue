// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use newtype_uuid::TypedUuid;
use serde::{Deserialize, Serialize};
use std::{error::Error, future::Future, sync::Arc};
use steno::{ActionRegistry, DagBuilder, SagaDag, SagaName};
use v_api::ArcMap;
use v_model::{Permissions, UserId, permissions::Caller};

use crate::{
    context::ApiContext, permissions::ApiPermissions, sagas::actions::push_backup::PushBackup,
};

use super::SprueSaga;

pub mod push_backup;

pub fn load_actions(registry: &mut ActionRegistry<SprueSaga>) {
    PushBackup::register_actions(registry);
}

pub trait RegisterActions {
    fn register_actions(registry: &mut ActionRegistry<SprueSaga>);
}

// A paired down caller that can be deserialized and serialized
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SagaActionCaller<T> {
    pub id: TypedUuid<UserId>,
    pub permissions: Permissions<T>,
}

impl<T> From<SagaActionCaller<T>> for Caller<T> {
    fn from(caller: SagaActionCaller<T>) -> Self {
        Caller {
            id: caller.id,
            permissions: caller.permissions,
            extensions: ArcMap::new(),
        }
    }
}

pub trait BuildDag {
    fn build_dag(&self, builder: &mut DagBuilder);
}

pub trait GenerateSagaDag {
    fn generate_dag<P>(&self, params: P) -> Arc<SagaDag>
    where
        P: Serialize;
}
impl<T> GenerateSagaDag for T
where
    T: BuildDag + SagaRuntime,
{
    fn generate_dag<P>(&self, params: P) -> Arc<SagaDag>
    where
        P: Serialize,
    {
        let mut builder = DagBuilder::new(self.name());
        self.build_dag(&mut builder);

        Arc::new(SagaDag::new(
            builder.build().expect("DAG was unexpectedly invalid"),
            serde_json::to_value(params).expect("Failed to serialize params"),
        ))
    }
}

pub trait SagaRuntime {
    fn name(&self) -> SagaName;
    fn system_caller(&self, caller_id: TypedUuid<UserId>) -> SagaActionCaller<ApiPermissions>;
}

pub trait BackgroundSaga<E: Error>: GenerateSagaDag + Copy + Send + Sync {
    fn generate_dags(
        &self,
        caller_id: TypedUuid<UserId>,
        ctx: &ApiContext,
    ) -> impl Future<Output = Result<Vec<Arc<SagaDag>>, E>> + Send;
}
