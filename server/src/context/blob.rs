use newtype_uuid::TypedUuid;
use std::{fs::File, sync::Arc};
use thiserror::Error;
use tokio::io::AsyncWrite;

use model::{
    Blob, BlobId, BlobState, BlobUploadState, ServerRegistration,
    db::NewBlobModel,
    storage::{BlobStorage, StorageError},
};
use v_api::response::{OptionalResource, ResourceError, ResourceErrorInner, ResourceResult};

#[derive(Debug, Error)]
pub enum BlobError {
    #[error(transparent)]
    File(#[from] std::io::Error),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Clone)]
pub struct BlobContext {
    storage: Arc<dyn BlobStorage>,
}

impl BlobContext {
    pub fn new(storage: Arc<dyn BlobStorage>) -> Self {
        Self { storage }
    }

    pub fn writer(&self, _backup: TypedUuid<BlobId>) -> Box<dyn AsyncWrite + Send + Unpin> {
        unimplemented!()
    }

    pub fn resource(&self, _backup: TypedUuid<BlobId>) -> File {
        unimplemented!()
    }

    pub async fn create_blob(
        &self,
        server: &ServerRegistration,
        size: i64,
    ) -> ResourceResult<Blob, BlobError> {
        let blob = self
            .storage
            .create_blob(&NewBlobModel {
                service_id: server.service_id,
                total_size: size,
            })
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?
            .into();
        Ok(blob)
    }

    pub async fn get_blob(&self, blob: TypedUuid<BlobId>) -> ResourceResult<Blob, BlobError> {
        Ok(self.storage.get_blob(blob).await.optional()?.into())
    }

    pub async fn start_blob_upload(
        &self,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        Ok(self
            .storage
            .update_blob_state(
                blob,
                BlobState::Pending,
                BlobState::Uploading(BlobUploadState::Started),
            )
            .await
            .optional()?)
    }

    pub async fn complete_blob_upload(
        &self,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        Ok(self
            .storage
            .update_blob_state(
                blob,
                BlobState::Uploading(BlobUploadState::Started),
                BlobState::Uploading(BlobUploadState::Complete),
            )
            .await
            .optional()?)
    }

    pub async fn reset_blob_upload(&self, blob: &mut File) -> ResourceResult<(), BlobError> {
        blob.set_len(0)
            .map_err(ResourceError::InternalError)
            .inner_err_into()?;
        Ok(())
    }

    pub async fn cancel_blob_upload(
        &self,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(blob).await?;
        Ok(self
            .storage
            .update_blob_state(blob.id, blob.state, BlobState::Cancelled)
            .await
            .optional()?)
    }
}
