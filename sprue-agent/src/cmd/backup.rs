// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use chrono::Utc;
use rand::RngExt;
use reqwest::Body;
use sprue_sdk::Client;
use sprue_sdk::types::{TypedUuidForBlobId, TypedUuidForServerRegistrationId};
use std::path::Path;
use tokio::fs::File;
use tokio_util::io::ReaderStream;

pub struct BackupRequest<'a> {
    pub client: &'a Client,
    pub registration_id: TypedUuidForServerRegistrationId,
    pub path: &'a Path,
}

pub async fn backup(request: BackupRequest<'_>) -> anyhow::Result<TypedUuidForBlobId> {
    let BackupRequest {
        client,
        registration_id,
        path,
    } = request;
    let bytes: [u8; 32] = rand::rng().random();
    let key = hex::encode(bytes);
    let size = path.metadata()?.len();
    let now = Utc::now();
    let blob_id = client
        .register_blob()
        .server(registration_id)
        .body_map(|body| body.blob_time(now).idempotency_key(key).size(size))
        .send()
        .await
        .map_err(|err| {
            tracing::error!(?err, "Failed to register blob");
            err
        })?
        .into_inner()
        .blob
        .id;

    // We loop here to do our best to ensure the file is fully uploaded before returning
    loop {
        let file = File::open(path).await.map_err(|err| {
            tracing::error!(?err, "Failed to open file");
            err
        })?;
        let stream = ReaderStream::new(file);
        let body = Body::wrap_stream(stream);
        match client
            .write_blob_upload()
            .blob(blob_id.clone())
            .body(body)
            .send()
            .await
        {
            Ok(_) => break,
            Err(_err) => {
                continue;
            }
        }
    }

    // With the file uploaded we can now mark the transfer complete
    client
        .complete_blob_upload()
        .blob(blob_id.clone())
        .send()
        .await?;

    Ok(blob_id)
}
