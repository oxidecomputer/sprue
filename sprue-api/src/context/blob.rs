// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use aws_sdk_s3::primitives::ByteStream;
use chrono::{DateTime, Utc};
use newtype_uuid::TypedUuid;
use std::{path::PathBuf, sync::Arc};
use thiserror::Error;
use tokio::{
    fs::{File, OpenOptions},
    io::{AsyncReadExt, AsyncWrite, AsyncWriteExt},
};
use v_model::permissions::Caller;

use sprue_model::{
    Blob, BlobId, BlobState, BlobUploadState, InvalidBlobStateTransition, ServerRegistrationId,
    ServiceId,
    db::NewBlobModel,
    storage::{BlobStorage, StorageError},
};
use v_api::response::{
    OptionalResource, ResourceError, ResourceErrorInner, ResourceResult, resource_restricted,
};

use crate::{context::ServerCaller, permissions::ApiPermissions, sagas::actions::SagaActionCaller};

#[derive(Debug, Error)]
pub enum BlobError {
    #[error(transparent)]
    File(#[from] std::io::Error),
    #[error("File does not have a name")]
    FileDoesNotHaveName,
    #[error("File has an invalid name")]
    FileHasInvalidName,
    #[error("Invalid state")]
    InvalidState(BlobState),
    #[error("Invalid state transition")]
    InvalidStateTransition(#[from] InvalidBlobStateTransition),
    #[error("S3 client error")]
    S3(#[from] aws_sdk_s3::Error),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

pub enum BlobCaller {
    Saga(SagaActionCaller<ApiPermissions>),
    Server(ServerCaller),
    User(Caller<ApiPermissions>),
}
impl From<SagaActionCaller<ApiPermissions>> for BlobCaller {
    fn from(caller: SagaActionCaller<ApiPermissions>) -> Self {
        Self::Saga(caller)
    }
}
impl From<ServerCaller> for BlobCaller {
    fn from(caller: ServerCaller) -> Self {
        Self::Server(caller)
    }
}
impl From<Caller<ApiPermissions>> for BlobCaller {
    fn from(caller: Caller<ApiPermissions>) -> Self {
        Self::User(caller)
    }
}
impl BlobCaller {
    pub fn can_read(&self, blob: &Blob) -> bool {
        match self {
            Self::Saga(saga) => saga.permissions.any(
                [
                    ApiPermissions::GetBlob(blob.id),
                    ApiPermissions::GetBlobsAll,
                ]
                .iter(),
            ),
            Self::Server(server) => server.id == blob.server_registration_id,
            Self::User(user) => user.any(
                [
                    ApiPermissions::GetBlob(blob.id),
                    ApiPermissions::GetBlobsAll,
                ]
                .iter(),
            ),
        }
    }

    pub fn can_manage(&self, blob: &Blob) -> bool {
        match self {
            Self::Saga(_) => false,
            Self::Server(server) => server.id == blob.server_registration_id,
            Self::User(_) => false,
        }
    }
}

#[derive(Clone)]
pub struct BlobContext {
    root: PathBuf,
    storage: Arc<dyn BlobStorage>,
    backup_storage: Arc<BackupStorage>,
}

impl BlobContext {
    pub fn new(
        root: PathBuf,
        storage: Arc<dyn BlobStorage>,
        backup_storage: BackupStorage,
    ) -> Self {
        Self {
            root,
            storage,
            backup_storage: Arc::new(backup_storage),
        }
    }

    pub async fn remote_writer(
        &self,
        caller: &BlobCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<RemoteWriter, BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.can_manage(&blob) {
            Ok(RemoteWriter {
                blob: blob.id,
                path: self.local_path(blob.id),
                backup_storage: self.backup_storage.clone(),
            })
        } else {
            resource_restricted()
        }
    }

    pub async fn local_writer(
        &self,
        caller: &BlobCaller,
        blob: Blob,
    ) -> ResourceResult<Box<dyn AsyncWrite + Send + Unpin>, BlobError> {
        if caller.can_manage(&blob) {
            let mut options = OpenOptions::new();
            options.create(true);
            options.write(true);
            let resource = self.local_resource(caller, blob, Some(options)).await?;
            Ok(Box::new(resource))
        } else {
            resource_restricted()
        }
    }

    pub fn local_path(&self, blob: TypedUuid<BlobId>) -> PathBuf {
        self.root.join(blob.to_string())
    }

    pub async fn local_resource(
        &self,
        caller: &BlobCaller,
        blob: Blob,
        options: Option<OpenOptions>,
    ) -> ResourceResult<File, BlobError> {
        if caller.can_manage(&blob) {
            let open = options.unwrap_or_else(|| {
                let mut options = OpenOptions::new();
                options.read(true);
                options
            });
            Ok(open
                .open(self.local_path(blob.id))
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
        blob_time: Option<DateTime<Utc>>,
    ) -> ResourceResult<Blob, BlobError> {
        let blob = self
            .storage
            .create_blob(&NewBlobModel {
                service_id: service,
                server_registration_id: server,
                blob_time: blob_time.unwrap_or_else(|| Utc::now()),
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
        caller: &BlobCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<Blob, BlobError> {
        let blob: Blob = self.storage.get_blob(blob).await.optional()?.into();
        if caller.can_read(&blob) {
            Ok(blob)
        } else {
            resource_restricted()
        }
    }

    pub async fn list_blobs(&self, caller: &BlobCaller) -> ResourceResult<Vec<Blob>, BlobError> {
        let models = self
            .storage
            .list_blobs()
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?;
        Ok(models
            .into_iter()
            .filter_map(|model| {
                let blob: Blob = model.into();
                if caller.can_read(&blob) {
                    Some(blob)
                } else {
                    None
                }
            })
            .collect())
    }

    pub async fn start_blob_upload(
        &self,
        caller: &BlobCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.can_manage(&blob) {
            Ok(self
                .storage
                .update_blob_state(
                    blob.id,
                    blob.state,
                    blob.state
                        .start_upload()
                        .map_err(ResourceError::InternalError)
                        .inner_err_into()?,
                )
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }

    pub async fn complete_blob_upload(
        &self,
        caller: &BlobCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.can_manage(&blob) {
            Ok(self
                .storage
                .update_blob_state(
                    blob.id,
                    blob.state,
                    blob.state
                        .complete_upload()
                        .map_err(ResourceError::InternalError)
                        .inner_err_into()?,
                )
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }

    pub async fn reset_blob_upload(
        &self,
        caller: &BlobCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.can_manage(&blob) {
            if blob.state == BlobState::Uploading(BlobUploadState::Started) {
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
                tracing::info!(?blob, "Blob can not be truncated in current state");
                Err(ResourceError::InternalError(BlobError::InvalidState(
                    blob.state,
                )))
            }
        } else {
            resource_restricted()
        }
    }

    pub async fn cancel_blob_upload(
        &self,
        caller: &BlobCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.can_manage(&blob) {
            Ok(self
                .storage
                .update_blob_state(
                    blob.id,
                    blob.state,
                    blob.state
                        .cancel()
                        .map_err(ResourceError::InternalError)
                        .inner_err_into()?,
                )
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }

    pub async fn start_blob_transfer(
        &self,
        caller: &BlobCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.can_manage(&blob) {
            Ok(self
                .storage
                .update_blob_state(
                    blob.id,
                    blob.state,
                    blob.state
                        .start_transfer()
                        .map_err(ResourceError::InternalError)
                        .inner_err_into()?,
                )
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }

    pub async fn complete_blob_transfer(
        &self,
        caller: &BlobCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.can_manage(&blob) {
            Ok(self
                .storage
                .update_blob_state(
                    blob.id,
                    blob.state,
                    blob.state
                        .complete_transfer()
                        .map_err(ResourceError::InternalError)
                        .inner_err_into()?,
                )
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }

    pub async fn fail_blob_transfer(
        &self,
        caller: &BlobCaller,
        blob: TypedUuid<BlobId>,
    ) -> ResourceResult<(), BlobError> {
        let blob = self.get_blob(caller, blob).await?;
        if caller.can_manage(&blob) {
            Ok(self
                .storage
                .update_blob_state(
                    blob.id,
                    blob.state,
                    blob.state
                        .fail_transfer()
                        .map_err(ResourceError::InternalError)
                        .inner_err_into()?,
                )
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }
}

pub struct RemoteWriter {
    blob: TypedUuid<BlobId>,
    path: PathBuf,
    backup_storage: Arc<BackupStorage>,
}

impl RemoteWriter {
    pub async fn write(self) -> Result<(), BlobError> {
        match self.backup_storage.as_ref() {
            BackupStorage::Local(local) => local.upload_blob(self.blob, self.path).await,
            BackupStorage::S3(s3) => s3.upload_blob(self.blob, self.path).await,
        }
    }

    pub async fn delete(self) -> Result<(), BlobError> {
        Ok(())
    }
}

pub enum BackupStorage {
    Local(LocalBackupStorage),
    S3(S3BackupStorage),
}

pub struct LocalBackupStorage {
    root: PathBuf,
}
impl LocalBackupStorage {
    pub fn new(root: PathBuf) -> Self {
        Self { root }
    }
}

pub struct S3BackupStorage {
    client: aws_sdk_s3::Client,
    bucket: String,
}
impl S3BackupStorage {
    pub fn new(client: aws_sdk_s3::Client, bucket: String) -> Self {
        Self { client, bucket }
    }
}

trait BackupStorageOps {
    const CHUNK_SIZE: usize = 8 * 1024 * 1024;
    async fn upload_blob(&self, blob: TypedUuid<BlobId>, local: PathBuf) -> Result<(), BlobError>;
}

impl BackupStorageOps for LocalBackupStorage {
    async fn upload_blob(&self, blob: TypedUuid<BlobId>, local: PathBuf) -> Result<(), BlobError> {
        let dst = self.root.join(blob.to_string()).join(
            local
                .file_name()
                .ok_or(BlobError::FileDoesNotHaveName)?
                .to_str()
                .ok_or(BlobError::FileHasInvalidName)?,
        );

        tokio::fs::create_dir_all(&dst).await?;

        let mut src_file = File::open(&local).await?;
        let mut dst_file = File::create(&dst).await?;

        let mut buf = vec![0u8; S3BackupStorage::CHUNK_SIZE];

        loop {
            let n = src_file.read(&mut buf).await?;
            if n == 0 {
                break;
            }
            dst_file.write_all(&buf[..n]).await?;
        }

        dst_file.flush().await?;

        Ok(())
    }
}

impl BackupStorageOps for S3BackupStorage {
    async fn upload_blob(&self, blob: TypedUuid<BlobId>, local: PathBuf) -> Result<(), BlobError> {
        let key = format!(
            "{}/{}",
            blob,
            local
                .file_name()
                .ok_or(BlobError::FileDoesNotHaveName)?
                .to_str()
                .ok_or(BlobError::FileHasInvalidName)?
        );

        let create_resp = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(aws_sdk_s3::Error::from)?;

        tracing::info!(?blob, ?create_resp, "Created multipart upload");

        let upload_id = create_resp
            .upload_id()
            .expect("AWS create must return an upload ID");
        let mut file = File::open(&local).await?;
        let mut part_number = 1;
        let mut completed_parts = vec![];

        loop {
            let mut buf = vec![0u8; S3BackupStorage::CHUNK_SIZE];
            let n = file.read(&mut buf).await?;
            if n == 0 {
                break; // EOF
            }
            buf.truncate(n);

            let part_resp = self
                .client
                .upload_part()
                .bucket(&self.bucket)
                .key(&key)
                .upload_id(upload_id)
                .part_number(part_number)
                .body(ByteStream::from(buf))
                .send()
                .await
                .map_err(aws_sdk_s3::Error::from)?;

            completed_parts.push(
                aws_sdk_s3::types::CompletedPart::builder()
                    .part_number(part_number)
                    .e_tag(part_resp.e_tag().unwrap_or_default())
                    .build(),
            );

            part_number += 1;
        }

        self.client
            .complete_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .multipart_upload(
                aws_sdk_s3::types::CompletedMultipartUpload::builder()
                    .set_parts(Some(completed_parts))
                    .build(),
            )
            .send()
            .await
            .map_err(aws_sdk_s3::Error::from)?;

        Ok(())
    }
}
