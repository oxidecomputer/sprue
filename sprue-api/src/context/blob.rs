// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use aws_sdk_s3::primitives::ByteStream;
use chrono::{DateTime, Utc};
use newtype_uuid::TypedUuid;
use std::{path::PathBuf, sync::Arc};
use tap::TapFallible;
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
    #[error("S3 upload part response missing ETag")]
    S3MissingETag,
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
            Self::Saga(saga) => saga.permissions.can(&ApiPermissions::GetBlob(blob.id)),
            Self::Server(server) => server.id == blob.server_registration_id,
            Self::User(user) => user.can(&ApiPermissions::GetBlob(blob.id)),
        }
    }

    pub fn can_manage(&self, blob: &Blob) -> bool {
        match self {
            Self::Saga(saga) => saga.permissions.can(&ApiPermissions::ManageBlobsAll),
            Self::Server(server) => server.id == blob.server_registration_id,
            Self::User(user) => user.can(&ApiPermissions::ManageBlobsAll),
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
    const MAX_S3_PARTS: u64 = 10_000;
    const DEFAULT_CHUNK_SIZE: u64 = 8 * 1024 * 1024;

    pub fn new(client: aws_sdk_s3::Client, bucket: String) -> Self {
        Self { client, bucket }
    }

    /// Choose a chunk size that keeps the upload within S3's 10,000-part limit.
    fn chunk_size(file_size: u64) -> usize {
        let min_chunk = file_size.div_ceil(Self::MAX_S3_PARTS);
        std::cmp::max(Self::DEFAULT_CHUNK_SIZE, min_chunk) as usize
    }

    async fn put_object(&self, key: &str) -> Result<(), BlobError> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .body(ByteStream::from(vec![]))
            .send()
            .await
            .map_err(aws_sdk_s3::Error::from)?;
        Ok(())
    }

    async fn multipart_upload(
        &self,
        blob: TypedUuid<BlobId>,
        key: &str,
        local: &PathBuf,
        file_size: u64,
    ) -> Result<(), BlobError> {
        let create_resp = self
            .client
            .create_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(aws_sdk_s3::Error::from)?;

        tracing::info!(?blob, ?create_resp, "Created multipart upload");

        let upload_id = create_resp
            .upload_id()
            .expect("AWS create must return an upload ID");

        let result = self.upload_parts(key, upload_id, local, file_size).await;

        if result.is_err() {
            self.abort_upload(key, upload_id).await;
        }

        result
    }

    async fn upload_parts(
        &self,
        key: &str,
        upload_id: &str,
        local: &PathBuf,
        file_size: u64,
    ) -> Result<(), BlobError> {
        let chunk_size = Self::chunk_size(file_size);
        let mut file = File::open(local).await?;
        let mut part_number: i32 = 1;
        let mut completed_parts = vec![];

        loop {
            let mut buf = Vec::with_capacity(chunk_size);
            let n = (&mut file)
                .take(chunk_size as u64)
                .read_to_end(&mut buf)
                .await?;
            if n == 0 {
                break;
            }

            let response = self
                .client
                .upload_part()
                .bucket(&self.bucket)
                .key(key)
                .upload_id(upload_id)
                .part_number(part_number)
                .body(ByteStream::from(buf))
                .send()
                .await
                .map_err(aws_sdk_s3::Error::from)?;

            completed_parts.push(
                aws_sdk_s3::types::CompletedPart::builder()
                    .part_number(part_number)
                    .e_tag(response.e_tag().ok_or(BlobError::S3MissingETag)?)
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

    async fn abort_upload(&self, key: &str, upload_id: &str) {
        let _ = self
            .client
            .abort_multipart_upload()
            .bucket(&self.bucket)
            .key(key)
            .upload_id(upload_id)
            .send()
            .await
            .tap_err(|err| {
                tracing::error!(?err, "Failed to abort multipart upload");
            });
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

        tokio::fs::create_dir_all(dst.parent().ok_or(BlobError::FileHasInvalidName)?).await?;

        let mut src_file = File::open(&local).await?;
        let mut dst_file = File::create(&dst).await?;

        let mut buf = vec![0u8; Self::CHUNK_SIZE];

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

        let metadata = tokio::fs::metadata(&local).await?;

        // Empty files cannot use multipart upload (S3 requires at least one part).
        if metadata.len() == 0 {
            self.put_object(&key).await
        } else {
            self.multipart_upload(blob, &key, &local, metadata.len())
                .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aws_sdk_s3::Client;
    use aws_sdk_s3::operation::abort_multipart_upload::AbortMultipartUploadOutput;
    use aws_sdk_s3::operation::complete_multipart_upload::CompleteMultipartUploadOutput;
    use aws_sdk_s3::operation::create_multipart_upload::CreateMultipartUploadOutput;
    use aws_sdk_s3::operation::put_object::PutObjectOutput;
    use aws_sdk_s3::operation::upload_part::UploadPartOutput;
    use aws_smithy_mocks::{RuleMode, mock, mock_client};

    /// Wrapper around a temporary file that cleans up on drop.
    struct TempFile(PathBuf);

    impl TempFile {
        fn new(name: &str, content: &[u8]) -> Self {
            let dir = std::env::temp_dir().join("sprue-blob-tests");
            std::fs::create_dir_all(&dir).unwrap();
            let id = TypedUuid::<BlobId>::new_v4();
            let path = dir.join(format!("{}-{}", name, id));
            std::fs::write(&path, content).unwrap();
            Self(path)
        }

        fn path(&self) -> PathBuf {
            self.0.clone()
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[tokio::test]
    async fn test_s3_multipart_upload() {
        let create_rule = mock!(Client::create_multipart_upload).then_output(|| {
            CreateMultipartUploadOutput::builder()
                .upload_id("test-upload-id")
                .build()
        });

        let upload_rule = mock!(Client::upload_part)
            .then_output(|| UploadPartOutput::builder().e_tag("\"test-etag\"").build());

        let complete_rule = mock!(Client::complete_multipart_upload)
            .then_output(|| CompleteMultipartUploadOutput::builder().build());

        let client = mock_client!(
            aws_sdk_s3,
            RuleMode::MatchAny,
            [&create_rule, &upload_rule, &complete_rule]
        );

        let storage = S3BackupStorage::new(client, "test-bucket".into());
        let file = TempFile::new("multipart", &[0u8; 1024]);

        storage
            .upload_blob(TypedUuid::new_v4(), file.path())
            .await
            .expect("multipart upload should succeed");

        assert_eq!(create_rule.num_calls(), 1);
        assert_eq!(upload_rule.num_calls(), 1);
        assert_eq!(complete_rule.num_calls(), 1);
    }

    #[tokio::test]
    async fn test_s3_empty_file_uses_put_object() {
        let put_rule = mock!(Client::put_object).then_output(|| PutObjectOutput::builder().build());

        let client = mock_client!(aws_sdk_s3, [&put_rule]);

        let storage = S3BackupStorage::new(client, "test-bucket".into());
        let file = TempFile::new("empty", &[]);

        storage
            .upload_blob(TypedUuid::new_v4(), file.path())
            .await
            .expect("empty file upload should succeed");

        assert_eq!(put_rule.num_calls(), 1);
    }

    #[tokio::test]
    async fn test_s3_upload_part_failure_triggers_abort() {
        let create_rule = mock!(Client::create_multipart_upload).then_output(|| {
            CreateMultipartUploadOutput::builder()
                .upload_id("test-upload-id")
                .build()
        });

        let upload_rule = mock!(Client::upload_part)
            .sequence()
            .http_status(400, None)
            .build();

        let abort_rule = mock!(Client::abort_multipart_upload)
            .then_output(|| AbortMultipartUploadOutput::builder().build());

        let client = mock_client!(
            aws_sdk_s3,
            RuleMode::MatchAny,
            [&create_rule, &upload_rule, &abort_rule],
            |conf| conf.retry_config(aws_sdk_s3::config::retry::RetryConfig::disabled())
        );

        let storage = S3BackupStorage::new(client, "test-bucket".into());
        let file = TempFile::new("fail-part", &[0u8; 1024]);

        let result = storage.upload_blob(TypedUuid::new_v4(), file.path()).await;

        assert!(result.is_err());
        assert_eq!(create_rule.num_calls(), 1);
        assert_eq!(abort_rule.num_calls(), 1);
    }

    #[tokio::test]
    async fn test_s3_complete_failure_triggers_abort() {
        let create_rule = mock!(Client::create_multipart_upload).then_output(|| {
            CreateMultipartUploadOutput::builder()
                .upload_id("test-upload-id")
                .build()
        });

        let upload_rule = mock!(Client::upload_part)
            .then_output(|| UploadPartOutput::builder().e_tag("\"test-etag\"").build());

        let complete_rule = mock!(Client::complete_multipart_upload)
            .sequence()
            .http_status(400, None)
            .build();

        let abort_rule = mock!(Client::abort_multipart_upload)
            .then_output(|| AbortMultipartUploadOutput::builder().build());

        let client = mock_client!(
            aws_sdk_s3,
            RuleMode::MatchAny,
            [&create_rule, &upload_rule, &complete_rule, &abort_rule],
            |conf| conf.retry_config(aws_sdk_s3::config::retry::RetryConfig::disabled())
        );

        let storage = S3BackupStorage::new(client, "test-bucket".into());
        let file = TempFile::new("fail-complete", &[0u8; 1024]);

        let result = storage.upload_blob(TypedUuid::new_v4(), file.path()).await;

        assert!(result.is_err());
        assert_eq!(upload_rule.num_calls(), 1);
        assert_eq!(abort_rule.num_calls(), 1);
    }

    #[tokio::test]
    async fn test_s3_missing_etag_returns_error_and_aborts() {
        let create_rule = mock!(Client::create_multipart_upload).then_output(|| {
            CreateMultipartUploadOutput::builder()
                .upload_id("test-upload-id")
                .build()
        });

        // Return a successful response but with no e_tag set
        let upload_rule =
            mock!(Client::upload_part).then_output(|| UploadPartOutput::builder().build());

        let abort_rule = mock!(Client::abort_multipart_upload)
            .then_output(|| AbortMultipartUploadOutput::builder().build());

        let client = mock_client!(
            aws_sdk_s3,
            RuleMode::MatchAny,
            [&create_rule, &upload_rule, &abort_rule]
        );

        let storage = S3BackupStorage::new(client, "test-bucket".into());
        let file = TempFile::new("missing-etag", &[0u8; 1024]);

        let result = storage.upload_blob(TypedUuid::new_v4(), file.path()).await;

        assert!(matches!(result, Err(BlobError::S3MissingETag)));
        assert_eq!(abort_rule.num_calls(), 1);
    }
}
