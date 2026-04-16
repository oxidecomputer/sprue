use newtype_uuid::TypedUuid;
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tokio::{
    fs::{File, OpenOptions},
    io::AsyncWrite,
};

use model::{
    Blob, BlobId, BlobState, BlobUploadState, ServerRegistrationId, ServiceId,
    db::NewBlobModel,
    storage::{BlobStorage, StorageError},
};
use v_api::response::{
    OptionalResource, ResourceError, ResourceErrorInner, ResourceResult, resource_restricted,
};

use crate::context::ServerCaller;

#[derive(Debug, Error)]
pub enum BlobError {
    #[error(transparent)]
    File(#[from] std::io::Error),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

#[derive(Clone)]
pub struct BlobContext {
    root: PathBuf,
    storage: Arc<dyn BlobStorage>,
}

impl BlobContext {
    pub fn new(root: PathBuf, storage: Arc<dyn BlobStorage>) -> Self {
        Self { root, storage }
    }

    pub async fn local_writer(
        &self,
        caller: &ServerCaller,
        blob: Blob,
    ) -> ResourceResult<Box<dyn AsyncWrite + Send + Unpin>, BlobError> {
        if caller.id == blob.server_registration_id {
            let mut options = OpenOptions::new();
            options.write(true);
            let resource = self.local_resource(caller, blob, Some(options)).await?;
            Ok(Box::new(resource))
        } else {
            resource_restricted()
        }
    }

    pub async fn local_resource(
        &self,
        caller: &ServerCaller,
        blob: Blob,
        options: Option<OpenOptions>,
    ) -> ResourceResult<File, BlobError> {
        if caller.id == blob.server_registration_id {
            let open = options.unwrap_or_else(|| {
                let mut options = OpenOptions::new();
                options.read(true);
                options
            });
            Ok(open
                .open(self.root.join(blob.id.to_string()))
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?)
        } else {
            resource_restricted()
        }
    }

    pub async fn create_blob(
        &self,
        server: TypedUuid<ServerRegistrationId>,
        service: TypedUuid<ServiceId>,
        size: i64,
    ) -> ResourceResult<Blob, BlobError> {
        let blob = self
            .storage
            .create_blob(&NewBlobModel {
                service_id: service,
                server_registration_id: server,
                total_size: size,
            })
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?
            .into();
        Ok(blob)
    }

    pub async fn get_blob(
        &self,
        caller: &ServerCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<Blob, BlobError> {
        let blob: Blob = self.storage.get_blob(blob).await.optional()?.into();
        if caller.id == blob.server_registration_id {
            Ok(blob)
        } else {
            resource_restricted()
        }
    }

    pub async fn start_blob_upload(
        &self,
        caller: &ServerCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.id == blob.server_registration_id {
            Ok(self
                .storage
                .update_blob_state(
                    blob.id,
                    BlobState::Pending,
                    BlobState::Uploading(BlobUploadState::Started),
                )
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }

    pub async fn complete_blob_upload(
        &self,
        caller: &ServerCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.id == blob.server_registration_id {
            Ok(self
                .storage
                .update_blob_state(
                    blob.id,
                    BlobState::Uploading(BlobUploadState::Started),
                    BlobState::Uploading(BlobUploadState::Complete),
                )
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }

    pub async fn reset_blob_upload(
        &self,
        caller: &ServerCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.id == blob.server_registration_id {
            let mut options = OpenOptions::new();
            options.write(true);
            let resource = self.local_resource(caller, blob, Some(options)).await?;
            resource
                .set_len(0)
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?;
            Ok(())
        } else {
            resource_restricted()
        }
    }

    pub async fn cancel_blob_upload(
        &self,
        caller: &ServerCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.id == blob.server_registration_id {
            Ok(self
                .storage
                .update_blob_state(blob.id, blob.state, BlobState::Cancelled)
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }
}
