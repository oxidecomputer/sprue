// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use futures::future::BoxFuture;
use std::sync::Arc;

use crate::{
    config::BackupStorageConfig,
    context::blob::{BackupStorage, LocalBackupStorage, S3BackupStorage},
};

mod local;
mod s3;

pub type OidcTokenFetcher =
    Arc<dyn Fn(String) -> BoxFuture<'static, anyhow::Result<String>> + Send + Sync>;

pub async fn create_backup_storage(
    config: BackupStorageConfig,
    token_fetcher: OidcTokenFetcher,
) -> BackupStorage {
    match config {
        BackupStorageConfig::Local { root } => BackupStorage::Local(LocalBackupStorage::new(root)),
        BackupStorageConfig::S3 {
            iam_region,
            bucket,
            role,
        } => BackupStorage::S3(S3BackupStorage::new(
            s3::build_s3_client(
                iam_region,
                role,
                "sprue".to_string(),
                std::time::Duration::from_secs(30),
                token_fetcher,
            )
            .await,
            bucket,
        )),
    }
}
