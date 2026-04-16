use chrono::{DateTime, Utc};
use newtype_uuid::{TypedUuid, TypedUuidKind, TypedUuidTag};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use thiserror::Error;

use crate::db::{
    BlobModel, HealthCheckModel, IdempotentRequestModel, ServerRegistrationModel, ServiceModel,
    TokenRequestModel,
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

#[derive(JsonSchema)]
pub enum TokenRequestId {}
impl TypedUuidKind for TokenRequestId {
    fn tag() -> TypedUuidTag {
        const TAG: TypedUuidTag = TypedUuidTag::new("token_request");
        TAG
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum ServerRegistrationState {
    Pending,
    Proven,
    Accepted,
    Rejected,
    Terminated,
    Expired,
}

#[derive(Debug, Error)]
#[error("invalid state transition")]
pub struct InvalidStateTransition {
    pub from: ServerRegistrationState,
    pub to: ServerRegistrationState,
}

impl ServerRegistrationState {
    pub fn prove(&self) -> Result<Self, InvalidStateTransition> {
        match self {
            Self::Pending => Ok(Self::Proven),
            Self::Proven => Err(InvalidStateTransition {
                from: *self,
                to: Self::Proven,
            }),
            Self::Accepted => Err(InvalidStateTransition {
                from: *self,
                to: Self::Proven,
            }),
            Self::Rejected => Err(InvalidStateTransition {
                from: *self,
                to: Self::Proven,
            }),
            Self::Terminated => Err(InvalidStateTransition {
                from: *self,
                to: Self::Proven,
            }),
            Self::Expired => Err(InvalidStateTransition {
                from: *self,
                to: Self::Proven,
            }),
        }
    }
    pub fn accept(&self) -> Result<Self, InvalidStateTransition> {
        match self {
            Self::Pending => Err(InvalidStateTransition {
                from: *self,
                to: Self::Accepted,
            }),
            Self::Proven => Ok(Self::Accepted),
            Self::Accepted => Err(InvalidStateTransition {
                from: *self,
                to: Self::Accepted,
            }),
            Self::Rejected => Err(InvalidStateTransition {
                from: *self,
                to: Self::Accepted,
            }),
            Self::Terminated => Err(InvalidStateTransition {
                from: *self,
                to: Self::Accepted,
            }),
            Self::Expired => Err(InvalidStateTransition {
                from: *self,
                to: Self::Accepted,
            }),
        }
    }
    pub fn reject(&self) -> Result<Self, InvalidStateTransition> {
        match self {
            Self::Pending => Ok(Self::Rejected),
            Self::Proven => Ok(Self::Rejected),
            Self::Accepted => Err(InvalidStateTransition {
                from: *self,
                to: Self::Rejected,
            }),
            Self::Rejected => Err(InvalidStateTransition {
                from: *self,
                to: Self::Rejected,
            }),
            Self::Terminated => Err(InvalidStateTransition {
                from: *self,
                to: Self::Rejected,
            }),
            Self::Expired => Err(InvalidStateTransition {
                from: *self,
                to: Self::Rejected,
            }),
        }
    }
    pub fn terminate(&self) -> Result<Self, InvalidStateTransition> {
        match self {
            Self::Pending => Ok(Self::Terminated),
            Self::Proven => Ok(Self::Terminated),
            Self::Accepted => Ok(Self::Terminated),
            Self::Rejected => Ok(Self::Terminated),
            Self::Terminated => Err(InvalidStateTransition {
                from: *self,
                to: Self::Terminated,
            }),
            Self::Expired => Ok(Self::Terminated),
        }
    }
    pub fn expire(&self) -> Result<Self, InvalidStateTransition> {
        match self {
            Self::Pending => Ok(Self::Expired),
            Self::Proven => Err(InvalidStateTransition {
                from: *self,
                to: Self::Expired,
            }),
            Self::Accepted => Err(InvalidStateTransition {
                from: *self,
                to: Self::Expired,
            }),
            Self::Rejected => Err(InvalidStateTransition {
                from: *self,
                to: Self::Expired,
            }),
            Self::Terminated => Err(InvalidStateTransition {
                from: *self,
                to: Self::Expired,
            }),
            Self::Expired => Err(InvalidStateTransition {
                from: *self,
                to: Self::Expired,
            }),
        }
    }
}

#[derive(Debug, Copy, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub enum TokenRequestState {
    Pending,
    Consume,
    Terminated,
    Expired,
}

#[derive(Debug, Error)]
#[error("invalid token request state transition")]
pub struct InvalidTokenRequestStateTransition {
    pub from: TokenRequestState,
    pub to: TokenRequestState,
}

impl TokenRequestState {
    pub fn consume(&self) -> Result<Self, InvalidTokenRequestStateTransition> {
        match self {
            Self::Pending => Ok(Self::Consume),
            _ => Err(InvalidTokenRequestStateTransition {
                from: *self,
                to: Self::Consume,
            }),
        }
    }
    pub fn terminate(&self) -> Result<Self, InvalidTokenRequestStateTransition> {
        match self {
            Self::Terminated => Err(InvalidTokenRequestStateTransition {
                from: *self,
                to: Self::Terminated,
            }),
            _ => Ok(Self::Terminated),
        }
    }
    pub fn expire(&self) -> Result<Self, InvalidTokenRequestStateTransition> {
        match self {
            Self::Pending => Ok(Self::Expired),
            Self::Consume => Err(InvalidTokenRequestStateTransition {
                from: *self,
                to: Self::Expired,
            }),
            Self::Terminated => Err(InvalidTokenRequestStateTransition {
                from: *self,
                to: Self::Expired,
            }),
            Self::Expired => Err(InvalidTokenRequestStateTransition {
                from: *self,
                to: Self::Expired,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct TokenRequest {
    pub id: TypedUuid<TokenRequestId>,
    pub server_registration_id: TypedUuid<ServerRegistrationId>,
    pub nonce: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    pub state: TokenRequestState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<TokenRequestModel> for TokenRequest {
    fn from(model: TokenRequestModel) -> Self {
        Self {
            id: model.id,
            server_registration_id: model.server_registration_id,
            nonce: model.nonce,
            expires_at: model.expires_at,
            state: model.state,
            created_at: model.created_at,
            updated_at: model.updated_at,
        }
    }
}

#[derive(Debug, Clone, Serialize, JsonSchema)]
pub struct ServerRegistration {
    pub id: TypedUuid<ServerRegistrationId>,
    pub service_id: TypedUuid<ServiceId>,
    pub instance_id: TypedUuid<ServerRegistrationInstanceId>,
    pub nonce: Option<String>,
    pub state: ServerRegistrationState,
    pub expires_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<ServerRegistrationModel> for ServerRegistration {
    fn from(model: ServerRegistrationModel) -> Self {
        Self {
            id: model.id,
            service_id: model.service_id,
            instance_id: model.instance_id,
            nonce: model.nonce,
            state: model.state,
            expires_at: model.expires_at,
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
    pub server_registration_id: TypedUuid<ServerRegistrationId>,
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
            server_registration_id: model.server_registration_id,
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
