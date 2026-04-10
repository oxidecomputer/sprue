use std::{ops::Add, sync::Arc};

use chrono::{DateTime, Duration, Utc};
use dropshot::{ClientErrorStatusCode, HttpError, HttpResponseOk};
use model::{
    IdempotentRequest, IdempotentRequestId, IdempotentRequestState, InvalidValue,
    ServerRegistrationId,
    db::NewIdempotentRequestModel,
    storage::{IdempotentRequestStorage, StorageError},
};
use newtype_uuid::TypedUuid;
use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};
use thiserror::Error;
use uuid::Uuid;
use v_api::response::{OptionalResource, ResourceError, ResourceErrorInner, ResourceResult};

const REQUEST_LIFETIME: Duration = Duration::hours(24);

#[derive(Debug, Error)]
pub enum IdempotencyError {
    #[error("Failed to serialize response")]
    InvalidResponse,
    #[error("Failed to deseriliaze to request")]
    ResponseValue(#[from] InvalidValue),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

pub trait IdempotencyContextStorage: IdempotentRequestStorage {}
impl<T: IdempotentRequestStorage> IdempotencyContextStorage for T {}

pub enum IdempontentRecord<T> {
    New(IdempotentRequest<T>),
    Existing(IdempotentRequest<T>),
}

#[derive(Clone)]
pub struct IdempotencyContext {
    storage: Arc<dyn IdempotencyContextStorage>,
}

impl IdempotencyContext {
    pub fn new(storage: Arc<dyn IdempotencyContextStorage>) -> Self {
        Self { storage }
    }

    pub async fn execute_idempotent_request<T, F, R>(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
        idempotency_key: Option<String>,
        f: F,
    ) -> Result<HttpResponseOk<T>, HttpError>
    where
        T: Clone + DeserializeOwned + JsonSchema + Serialize + Send + Sync + 'static,
        F: FnOnce(IdempotentRequest<T>) -> R,
        R: Future<Output = Result<T, HttpError>>,
    {
        let idempotency_request = self
            .get_request::<T>(
                server_registration_id,
                &idempotency_key.unwrap_or_else(|| Uuid::new_v4().to_string()),
            )
            .await?;

        match idempotency_request {
            IdempontentRecord::New(req) => {
                let id = req.id;
                let response = f(req).await?;

                self.complete_request(id, Some(response.clone())).await?;
                Ok(HttpResponseOk(response))
            }
            IdempontentRecord::Existing(req) => match req.state {
                IdempotentRequestState::Processing => Err(HttpError::for_client_error(
                    None,
                    ClientErrorStatusCode::CONFLICT,
                    "Request in progress".to_string(),
                )),
                IdempotentRequestState::Complete => {
                    Ok(HttpResponseOk(req.response.ok_or_else(|| {
                        HttpError::for_internal_error("Response does not have body".to_string())
                    })?))
                }
            },
        }
    }

    /// Create a new idempotent request with Processing state
    async fn create_request<T>(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
        idempotency_key: &str,
        expires_at: DateTime<Utc>,
    ) -> ResourceResult<IdempotentRequest<T>, IdempotencyError>
    where
        T: DeserializeOwned,
    {
        Ok(self
            .storage
            .create_request(&NewIdempotentRequestModel {
                server_registration_id,
                idempotency_key: idempotency_key.to_string(),
                expires_at,
            })
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?
            .try_into()
            .map_err(ResourceError::InternalError)
            .inner_err_into()?)
    }

    /// Get an idempotent request by server registration ID and idempotency key
    async fn get_request<T>(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
        idempotency_key: &str,
    ) -> ResourceResult<IdempontentRecord<T>, IdempotencyError>
    where
        T: DeserializeOwned,
    {
        let lookup = self
            .storage
            .get_request(server_registration_id, idempotency_key)
            .await
            .optional();
        match lookup {
            Ok(request) => Ok(IdempontentRecord::Existing(
                request
                    .try_into()
                    .map_err(ResourceError::InternalError)
                    .inner_err_into()?,
            )),
            Err(ResourceError::DoesNotExist) => Ok(IdempontentRecord::New(
                self.create_request(
                    server_registration_id,
                    idempotency_key,
                    Utc::now().add(REQUEST_LIFETIME),
                )
                .await?,
            )),
            Err(err) => Err(err)?,
        }
    }

    /// Mark an idempotent request as complete by setting the response.
    /// This atomically transitions the request from Processing to Complete state.
    /// Only succeeds if the request is currently in Processing state.
    async fn complete_request<T>(
        &self,
        id: TypedUuid<IdempotentRequestId>,
        response: Option<T>,
    ) -> ResourceResult<(), IdempotencyError>
    where
        T: Serialize,
    {
        Ok(self
            .storage
            .complete_request(
                id,
                response
                    .map(|r| {
                        serde_json::to_value(r).map_err(|_| {
                            ResourceError::InternalError(IdempotencyError::InvalidResponse)
                        })
                    })
                    .transpose()?,
            )
            .await
            .optional()?)
    }

    /// Delete all expired idempotent requests
    pub async fn cleanup_expired(&self) -> ResourceResult<u64, IdempotencyError> {
        Ok(self
            .storage
            .delete_expired_requests()
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?)
    }
}
