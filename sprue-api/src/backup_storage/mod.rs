// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use futures::future::BoxFuture;
use secrecy::ExposeSecret;
use std::{path::Path, sync::Arc};
use v_api_param::ParamResolutionError;

use crate::{
    config::BackupStorageConfig,
    context::blob::{BackupStorage, LocalBackupStorage, S3BackupStorage},
};

mod local;
mod s3;

pub type OidcTokenFetcher =
    Arc<dyn Fn(String) -> BoxFuture<'static, anyhow::Result<String>> + Send + Sync>;

pub async fn create_backup_storage(
    param_path: Option<&Path>,
    config: BackupStorageConfig,
    token_fetcher: OidcTokenFetcher,
) -> Result<BackupStorage, ParamResolutionError> {
    Ok(match config {
        BackupStorageConfig::Local { root } => BackupStorage::Local(LocalBackupStorage::new(root)),
        BackupStorageConfig::S3 {
            iam_region,
            bucket,
            role,
        } => BackupStorage::S3(S3BackupStorage::new(
            s3::build_s3_client(
                iam_region,
                role.resolve(param_path)?.expose_secret().to_string(),
                "sprue".to_string(),
                std::time::Duration::from_secs(30),
                token_fetcher,
            )
            .await,
            bucket.resolve(param_path)?.expose_secret().to_string(),
        )),
    })
}
