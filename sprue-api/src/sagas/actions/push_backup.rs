// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::sync::Arc;

use newtype_uuid::TypedUuid;
use serde::{Deserialize, Serialize};
use sprue_model::{
    BlobId, BlobState, BlobUploadState,
    storage::{BlobFilter, StorageError},
};
use steno::{ActionContext, ActionError, ActionRegistry, DagBuilder, Node, SagaDag, SagaName};
use thiserror::Error;
use v_api::response::ResourceError;
use v_model::{Permissions, UserId};

use crate::{
    context::{ApiContext, blob::BlobError},
    permissions::ApiPermissions,
    sagas::{
        SprueSaga,
        actions::{
            BackgroundSaga, BuildDag, GenerateSagaDag, RegisterActions, SagaActionCaller,
            SagaRuntime,
        },
    },
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushBackupParams {
    caller: SagaActionCaller<ApiPermissions>,
    blob: TypedUuid<BlobId>,
}

/// Define the saga actions as statics for easy registration and reference.
mod actions {
    use std::sync::Arc;

    use lazy_static::lazy_static;
    use steno::{Action, ActionFunc};

    use super::*;

    lazy_static! {
        pub static ref CLAIM_BACKUP: Arc<dyn Action<SprueSaga>> = ActionFunc::new_action(
            "claim_transfer",
            saga_claim_transfer,
            saga_claim_transfer_undo
        );
        pub static ref TRANSFER_BLOB: Arc<dyn Action<SprueSaga>> =
            ActionFunc::new_action("transfer_blob", saga_transfer_blob, saga_transfer_blob_undo);
        pub static ref COMPLETE_TRANSFER: Arc<dyn Action<SprueSaga>> =
            steno::new_action_noop_undo("complete_transfer", saga_complete_transfer);
    }
}

async fn saga_claim_transfer(action_context: ActionContext<SprueSaga>) -> Result<(), ActionError> {
    let saga_ctx = action_context.user_data();
    let params = action_context.saga_params::<PushBackupParams>()?;
    let caller = params.caller.into();

    // Attempt to start a blob transfer. State transitions ensure that we only start a transfer if
    // no one else is currently transferring the blob.
    let blob = saga_ctx
        .blob
        .get_blob(&caller, params.blob)
        .await
        .map_err(|err| ActionError::action_failed(err.to_string()))?;
    saga_ctx
        .blob
        .start_blob_transfer(&caller, blob.id)
        .await
        .map_err(|err| ActionError::action_failed(err.to_string()))?;

    Ok(())
}

async fn saga_claim_transfer_undo(
    action_context: ActionContext<SprueSaga>,
) -> Result<(), anyhow::Error> {
    let saga_ctx = action_context.user_data();
    let params = action_context.saga_params::<PushBackupParams>()?;
    let caller = params.caller.into();

    // Attempt to start a blob transfer. State transitions ensure that we only start a transfer if
    // no one else is currently transferring the blob.
    let blob = saga_ctx.blob.get_blob(&caller, params.blob).await?;
    saga_ctx.blob.fail_blob_transfer(&caller, blob.id).await?;

    Ok(())
}

async fn saga_transfer_blob(action_context: ActionContext<SprueSaga>) -> Result<(), ActionError> {
    let saga_ctx = action_context.user_data();
    let params = action_context.saga_params::<PushBackupParams>()?;
    let caller = params.caller.into();

    // Retrieve the writer for this blob
    let writer = saga_ctx
        .blob
        .remote_writer(&caller, params.blob)
        .await
        .map_err(|err| ActionError::action_failed(err.to_string()))?;

    // Stream the locally stored blob to the remote backup storage
    writer
        .write()
        .await
        .map_err(|err| ActionError::action_failed(err.to_string()))?;

    Ok(())
}

async fn saga_transfer_blob_undo(
    action_context: ActionContext<SprueSaga>,
) -> Result<(), anyhow::Error> {
    let saga_ctx = action_context.user_data();
    let params = action_context.saga_params::<PushBackupParams>()?;
    let caller = params.caller.into();

    // Retrieve the writer for this blob
    let writer = saga_ctx.blob.remote_writer(&caller, params.blob).await?;

    // Delete anything that has been previously transferred
    writer.delete().await?;

    Ok(())
}

async fn saga_complete_transfer(
    action_context: ActionContext<SprueSaga>,
) -> Result<(), ActionError> {
    let saga_ctx = action_context.user_data();
    let params = action_context.saga_params::<PushBackupParams>()?;
    let caller = params.caller.into();

    // Attempt to start a blob transfer. State transitions ensure that we only start a transfer if
    // no one else is currently transferring the blob.
    let blob = saga_ctx
        .blob
        .get_blob(&caller, params.blob)
        .await
        .map_err(|err| ActionError::action_failed(err.to_string()))?;
    saga_ctx
        .blob
        .complete_blob_transfer(&caller, blob.id)
        .await
        .map_err(|err| ActionError::action_failed(err.to_string()))?;

    Ok(())
}

#[derive(Clone, Copy)]
pub struct PushBackup;

impl RegisterActions for PushBackup {
    fn register_actions(registry: &mut ActionRegistry<SprueSaga>) {
        registry.register(actions::CLAIM_BACKUP.clone());
        registry.register(actions::TRANSFER_BLOB.clone());
        registry.register(actions::COMPLETE_TRANSFER.clone());
    }
}
impl SagaRuntime for PushBackup {
    fn name(&self) -> SagaName {
        SagaName::new("PushBackup")
    }

    fn system_caller(&self, caller_id: TypedUuid<UserId>) -> SagaActionCaller<ApiPermissions> {
        SagaActionCaller {
            id: caller_id,
            permissions: Permissions::from([
                ApiPermissions::GetBlobsAll,
                ApiPermissions::ManageBlobsAll,
            ]),
        }
    }
}
impl BuildDag for PushBackup {
    fn build_dag(&self, builder: &mut DagBuilder) {
        builder.append(Node::action(
            "claim_transfer",
            "claim_transfer",
            actions::CLAIM_BACKUP.as_ref(),
        ));
        builder.append(Node::action(
            "transfer_blob",
            "transfer_blob",
            actions::TRANSFER_BLOB.as_ref(),
        ));
        builder.append(Node::action(
            "complete_transfer",
            "complete_transfer",
            actions::COMPLETE_TRANSFER.as_ref(),
        ));
    }
}

#[derive(Debug, Error)]
pub enum PushBackupError {
    #[error("Internal context error")]
    Ctx(#[from] ResourceError<BlobError>),
    #[error("Storage error")]
    StorageError(#[from] StorageError),
}

impl BackgroundSaga<PushBackupError> for PushBackup {
    async fn generate_dags(
        &self,
        caller_id: TypedUuid<UserId>,
        ctx: &ApiContext,
    ) -> Result<Vec<Arc<SagaDag>>, PushBackupError> {
        let caller = self.system_caller(caller_id).into();
        let blobs = ctx
            .blob
            .list_blobs(
                &caller,
                &BlobFilter::default().state(BlobState::Uploading(BlobUploadState::Complete)),
            )
            .await?;

        let mut dags = vec![];
        for blob in blobs {
            let params = PushBackupParams {
                caller: self.system_caller(caller_id),
                blob: blob.id,
            };
            dags.push(self.generate_dag(params));
        }
        Ok(dags)
    }
}
