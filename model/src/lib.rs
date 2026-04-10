use chrono::{DateTime, Utc};
use newtype_uuid::{TypedUuid, TypedUuidKind, TypedUuidTag};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::db::{
    BlobModel, HealthCheckModel, IdempotentRequestModel, ServerRegistrationModel, ServiceModel,
};

pub mod db;
pub mod storage;

#[derive(JsonSchema)]
pub enum ServiceId {}
impl TypedUuidKind for ServiceId {
    fn tag() -> TypedUuidTag {
        const TAG: TypedUuidTag = TypedUuidTag::new("service");
        TAG
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct Service {
    pub id: TypedUuid<ServiceId>,
    pub name: String,
    pub created_at: DateTime<Utc>,
}

impl From<ServiceModel> for Service {
    fn from(model: ServiceModel) -> Self {
        Self {
            id: model.id,
            name: model.name,
            created_at: model.created_at,
        }
    }
}

#[derive(JsonSchema)]
pub enum ServerRegistrationId {}
impl TypedUuidKind for ServerRegistrationId {
    fn tag() -> TypedUuidTag {
        const TAG: TypedUuidTag = TypedUuidTag::new("server_registration");
        TAG
    }
}

#[derive(JsonSchema)]
pub enum ServerRegistrationInstanceId {}
impl TypedUuidKind for ServerRegistrationInstanceId {
    fn tag() -> TypedUuidTag {
        const TAG: TypedUuidTag = TypedUuidTag::new("server_registration_instance");
        TAG
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum ServerRegistrationState {
    Pending,
    Accepted,
    Rejected,
    Terminated,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ServerRegistration {
    pub id: TypedUuid<ServerRegistrationId>,
    pub service_id: TypedUuid<ServiceId>,
    pub instance_id: TypedUuid<ServerRegistrationInstanceId>,
    pub state: ServerRegistrationState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ServerRegistrationModel> for ServerRegistration {
    fn from(model: ServerRegistrationModel) -> Self {
        Self {
            id: model.id,
            service_id: model.service_id,
            instance_id: model.instance_id,
            state: model.state,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(JsonSchema)]
pub enum BlobId {}
impl TypedUuidKind for BlobId {
    fn tag() -> TypedUuidTag {
        const TAG: TypedUuidTag = TypedUuidTag::new("blob");
        TAG
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
pub struct Blob {
    pub id: TypedUuid<BlobId>,
    pub service_id: TypedUuid<ServiceId>,
    pub size: i64,
    pub total_size: i64,
    pub state: BlobState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum BlobState {
    Pending,
    Uploading(BlobUploadState),
    Transferring(BlobTransferState),
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum BlobUploadState {
    Started,
    Complete,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum BlobTransferState {
    Started,
    Complete,
}

impl From<BlobModel> for Blob {
    fn from(model: BlobModel) -> Self {
        Self {
            id: model.id,
            service_id: model.service_id,
            size: model.size,
            total_size: model.total_size,
            state: model.state,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(JsonSchema)]
pub enum HealthCheckId {}
impl TypedUuidKind for HealthCheckId {
    fn tag() -> TypedUuidTag {
        const TAG: TypedUuidTag = TypedUuidTag::new("healthcheck");
        TAG
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HealthCheck {
    pub id: TypedUuid<HealthCheckId>,
    pub server_registration_id: TypedUuid<ServerRegistrationId>,
    pub checked_in_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

impl From<HealthCheckModel> for HealthCheck {
    fn from(model: HealthCheckModel) -> Self {
        Self {
            id: model.id,
            server_registration_id: model.server_registration_id,
            checked_in_at: model.checked_in_at,
            created_at: model.created_at,
        }
    }
}

#[derive(JsonSchema)]
pub enum IdempotentRequestId {}
impl TypedUuidKind for IdempotentRequestId {
    fn tag() -> TypedUuidTag {
        const TAG: TypedUuidTag = TypedUuidTag::new("idempotent_request");
        TAG
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum IdempotentRequestState {
    Processing,
    Complete,
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct IdempotentRequest<T> {
    pub id: TypedUuid<IdempotentRequestId>,
    pub server_registration_id: TypedUuid<ServerRegistrationId>,
    pub idempotency_key: String,
    pub response: Option<T>,
    pub state: IdempotentRequestState,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Error)]
#[error("Can not deserialize value")]
pub struct InvalidValue;

impl<T> TryFrom<IdempotentRequestModel> for IdempotentRequest<T>
where
    T: DeserializeOwned,
{
    type Error = InvalidValue;
    fn try_from(model: IdempotentRequestModel) -> Result<Self, Self::Error> {
        Ok(Self {
            id: model.id,
            server_registration_id: model.server_registration_id,
            idempotency_key: model.idempotency_key,
            response: model
                .response
                .map(|r| serde_json::from_value(r))
                .transpose()
                .map_err(|_| InvalidValue)?,
            state: model.state,
            expires_at: model.expires_at,
            created_at: model.created_at,
            updated_at: model.updated_at,
        })
    }
}
