use chrono::{DateTime, Utc};
use newtype_uuid::TypedUuid;
use partial_struct::partial;

use crate::{
    BlobId, BlobState, HealthCheckId, IdempotentRequestId, IdempotentRequestState,
    ServerRegistrationId, ServerRegistrationInstanceId, ServerRegistrationState, ServiceId,
    TokenRequestId, TokenRequestState,
};

#[partial(NewServiceModel)]
pub struct ServiceModel {
    #[partial(NewServiceModel(skip))]
    pub id: TypedUuid<ServiceId>,
    pub name: String,
    #[partial(NewServiceModel(skip))]
    pub created_at: DateTime<Utc>,
}

#[partial(NewServerRegistrationModel)]
pub struct ServerRegistrationModel {
    #[partial(NewServerRegistrationModel(skip))]
    pub id: TypedUuid<ServerRegistrationId>,
    pub service_id: TypedUuid<ServiceId>,
    pub instance_id: TypedUuid<ServerRegistrationInstanceId>,
    pub nonce: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    #[partial(NewServerRegistrationModel(skip))]
    pub state: ServerRegistrationState,
    #[partial(NewServerRegistrationModel(skip))]
    pub created_at: DateTime<Utc>,
    #[partial(NewServerRegistrationModel(skip))]
    pub updated_at: DateTime<Utc>,
}

#[partial(NewTokenRequestModel)]
pub struct TokenRequestModel {
    #[partial(NewTokenRequestModel(skip))]
    pub id: TypedUuid<TokenRequestId>,
    pub server_registration_id: TypedUuid<ServerRegistrationId>,
    pub nonce: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
    #[partial(NewTokenRequestModel(skip))]
    pub state: TokenRequestState,
    #[partial(NewTokenRequestModel(skip))]
    pub created_at: DateTime<Utc>,
    #[partial(NewTokenRequestModel(skip))]
    pub updated_at: DateTime<Utc>,
}

#[partial(NewBlobModel)]
pub struct BlobModel {
    #[partial(NewBlobModel(skip))]
    pub id: TypedUuid<BlobId>,
    pub service_id: TypedUuid<ServiceId>,
    pub server_registration_id: TypedUuid<ServerRegistrationId>,
    pub blob_time: DateTime<Utc>,
    #[partial(NewBlobModel(skip))]
    pub size: i64,
    pub total_size: i64,
    #[partial(NewBlobModel(skip))]
    pub state: BlobState,
    #[partial(NewBlobModel(skip))]
    pub created_at: DateTime<Utc>,
    #[partial(NewBlobModel(skip))]
    pub updated_at: DateTime<Utc>,
}

#[partial(NewHealthCheckModel)]
pub struct HealthCheckModel {
    #[partial(NewHealthCheckModel(skip))]
    pub id: TypedUuid<HealthCheckId>,
    pub server_registration_id: TypedUuid<ServerRegistrationId>,
    pub checked_in_at: DateTime<Utc>,
    #[partial(NewHealthCheckModel(skip))]
    pub created_at: DateTime<Utc>,
}

#[partial(NewIdempotentRequestModel)]
pub struct IdempotentRequestModel {
    #[partial(NewIdempotentRequestModel(skip))]
    pub id: TypedUuid<IdempotentRequestId>,
    pub server_registration_id: TypedUuid<ServerRegistrationId>,
    pub idempotency_key: String,
    #[partial(NewIdempotentRequestModel(skip))]
    pub response: Option<serde_json::Value>,
    #[partial(NewIdempotentRequestModel(skip))]
    pub state: IdempotentRequestState,
    pub expires_at: DateTime<Utc>,
    #[partial(NewIdempotentRequestModel(skip))]
    pub created_at: DateTime<Utc>,
    #[partial(NewIdempotentRequestModel(skip))]
    pub updated_at: DateTime<Utc>,
}
