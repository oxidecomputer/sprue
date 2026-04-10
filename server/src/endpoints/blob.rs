use dropshot::{
    HttpError, HttpResponseUpdatedNoContent, Path, RequestContext, StreamingBody, endpoint,
};
use futures::StreamExt;
use model::BlobId;
use newtype_uuid::TypedUuid;
use schemars::JsonSchema;
use serde::Deserialize;
use tokio::io::AsyncWriteExt;

use crate::context::ApiContext;

#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlobPath {
    blob: TypedUuid<BlobId>,
}

/// Stream data to fill a registered blob. Any data that is streamed is appended to any data that
/// has already been recieved. Concurrent writes to the same blob are not supported.
#[endpoint {
    path = "/blob/{blob}/upload/write",
    method = POST,
}]
pub async fn write_blob_upload(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
    body: StreamingBody,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();

    ctx.blob.start_blob_upload(path.blob).await?;

    let mut writer = ctx.blob.writer(path.blob);
    let stream = body.into_stream();
    tokio::pin!(stream);

    while let Some(res) = stream.next().await {
        let mut data = res?;
        writer
            .write_all_buf(&mut data)
            .await
            .map_err(|error| HttpError::for_unavail(None, format!("write failed: {error}")))?;
    }

    writer
        .flush()
        .await
        .map_err(|error| HttpError::for_unavail(None, format!("flush failed: {error}")))?;

    Ok(HttpResponseUpdatedNoContent {})
}

/// Reset a blob to synchronously remove any data already uploaded to this blob.
#[endpoint {
    path = "/blob/{blob}/upload/reset",
    method = POST,
}]
pub async fn reset_blob_upload(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    let mut blob = ctx.blob.resource(path.blob);
    ctx.blob.reset_blob_upload(&mut blob).await?;
    Ok(HttpResponseUpdatedNoContent())
}

/// Mark a blob as being fully uploaded and ready to be persisted.
#[endpoint {
    path = "/blob/{blob}/upload/complete",
    method = POST,
}]
pub async fn complete_blob_upload(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    ctx.blob.complete_blob_upload(path.blob).await?;
    Ok(HttpResponseUpdatedNoContent())
}

/// Cancels a blob so that it can no longer be written to. Any data already sent will eventually
/// be deleted.
#[endpoint {
    path = "/blob/{blob}/upload/cancel",
    method = POST,
}]
pub async fn cancel_blob_upload(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<HttpResponseUpdatedNoContent, HttpError> {
    let ctx = rqctx.context();
    let path = path.into_inner();
    ctx.blob.cancel_blob_upload(path.blob).await?;
    Ok(HttpResponseUpdatedNoContent())
}
