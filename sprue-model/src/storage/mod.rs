use async_trait::async_trait;
use chrono::{DateTime, Utc};
use newtype_uuid::TypedUuid;

use crate::db::{
    BlobModel, HealthCheckModel, IdempotentRequestModel, NewBlobModel, NewHealthCheckModel,
    NewIdempotentRequestModel, NewServerRegistrationModel, NewServiceModel, NewTokenRequestModel,
    ServerRegistrationModel, ServiceModel, TokenRequestModel,
};
use crate::{
    BlobId, BlobState, IdempotentRequestId, IdempotentRequestState, ServerRegistrationId,
    ServerRegistrationInstanceId, ServerRegistrationState, ServiceId, TokenRequestId,
    TokenRequestState,
};

pub mod postgres;

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("Server registration already exists for instance_id: {0}")]
    ServerRegistrationAlreadyExists(TypedUuid<ServerRegistrationInstanceId>),

    #[error("Invalid state transition for blob {blob_id}: expected state {expected:?}")]
    InvalidBlobStateTransition {
        blob_id: TypedUuid<BlobId>,
        expected: BlobState,
    },

    #[error(
        "Invalid state transition for server registration {server_registration_id}: expected state {expected:?}"
    )]
    InvalidServerRegistrationStateTransition {
        server_registration_id: TypedUuid<ServerRegistrationId>,
        expected: ServerRegistrationState,
    },

    #[error("Idempotent request already exists for key: {0}")]
    IdempotentRequestAlreadyExists(String),

    #[error(
        "Invalid state transition for idempotent request {idempotent_request_id}: expected state {expected:?}"
    )]
    InvalidIdempotentRequestStateTransition {
        idempotent_request_id: TypedUuid<IdempotentRequestId>,
        expected: IdempotentRequestState,
    },

    #[error(
        "Invalid state transition for token request {token_request_id}: expected state {expected:?}"
    )]
    InvalidTokenRequestStateTransition {
        token_request_id: TypedUuid<TokenRequestId>,
        expected: TokenRequestState,
    },

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type StorageResult<T> = Result<T, StorageError>;

/// Represents a state transition for a blob
#[derive(Debug, Clone)]
pub struct BlobStateTransition {
    pub state: BlobState,
    pub created_at: DateTime<Utc>,
}

/// Represents a state transition for a server registration
#[derive(Debug, Clone)]
pub struct ServerRegistrationStateTransition {
    pub state: ServerRegistrationState,
    pub created_at: DateTime<Utc>,
}

/// Represents a state transition for an idempotent request
#[derive(Debug, Clone)]
pub struct IdempotentRequestStateTransition {
    pub state: IdempotentRequestState,
    pub created_at: DateTime<Utc>,
}

/// Represents a state transition for a token request
#[derive(Debug, Clone)]
pub struct TokenRequestStateTransition {
    pub state: TokenRequestState,
    pub created_at: DateTime<Utc>,
}

/// Storage interface for service operations
#[async_trait]
pub trait ServiceStorage: Send + Sync {
    /// Create a new service and return the created model
    async fn create_service(&self, service: &NewServiceModel) -> StorageResult<ServiceModel>;

    /// Get a service by id
    /// Returns None if the service does not exist
    async fn get_service_by_id(
        &self,
        id: TypedUuid<ServiceId>,
    ) -> StorageResult<Option<ServiceModel>>;

    /// Get a service by name
    /// Returns None if the service does not exist
    async fn get_service_by_name(&self, name: &str) -> StorageResult<Option<ServiceModel>>;

    /// List all services
    async fn list_services(&self) -> StorageResult<Vec<ServiceModel>>;

    /// Delete a service
    /// Returns None if the service does not exist
    async fn delete_service(&self, name: &str) -> StorageResult<Option<()>>;
}

/// Storage interface for server registration operations
#[async_trait]
pub trait ServerRegistrationStorage: Send + Sync {
    /// Create a new server registration (associates a public key with a service)
    /// The registration is created with Pending state
    async fn create_server_registration(
        &self,
        registration: &NewServerRegistrationModel,
    ) -> StorageResult<ServerRegistrationModel>;

    /// Get a server registration by ID
    /// Returns None if the registration does not exist
    async fn get_server_registration(
        &self,
        id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Option<ServerRegistrationModel>>;

    /// Get a server registration by instance_id
    /// Returns None if the registration does not exist
    async fn get_server_registration_by_instance_id(
        &self,
        instance_id: TypedUuid<ServerRegistrationInstanceId>,
    ) -> StorageResult<Option<ServerRegistrationModel>>;

    /// List all server registrations for a service ID
    async fn list_server_registrations_by_service_id(
        &self,
        service_id: TypedUuid<ServiceId>,
    ) -> StorageResult<Vec<ServerRegistrationModel>>;

    /// Update server registration state (atomic, creates a new state transition record)
    /// Only succeeds if from_state matches the current state
    /// Returns None if the registration does not exist
    async fn update_server_registration_state(
        &self,
        id: TypedUuid<ServerRegistrationId>,
        from_state: ServerRegistrationState,
        to_state: ServerRegistrationState,
    ) -> StorageResult<Option<()>>;

    /// Get all state transitions for a server registration (ordered by creation time)
    /// Returns None if the registration does not exist
    async fn get_server_registration_state_history(
        &self,
        id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Option<Vec<ServerRegistrationStateTransition>>>;

    /// Delete a server registration
    /// Returns None if the registration does not exist
    async fn delete_server_registration(
        &self,
        id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Option<()>>;

    /// Delete a server registration by instance_id
    /// Returns None if the registration does not exist
    async fn delete_server_registration_by_instance_id(
        &self,
        instance_id: TypedUuid<ServerRegistrationInstanceId>,
    ) -> StorageResult<Option<()>>;
}

/// Storage interface for blob operations
#[async_trait]
pub trait BlobStorage: Send + Sync {
    /// Create a new blob (always created with Pending state) and return the created model
    async fn create_blob(&self, blob: &NewBlobModel) -> StorageResult<BlobModel>;

    /// Get a blob by ID
    /// Returns None if the blob does not exist
    async fn get_blob(&self, id: TypedUuid<BlobId>) -> StorageResult<Option<BlobModel>>;

    /// List all blobs
    async fn list_blobs(&self) -> StorageResult<Vec<BlobModel>>;

    /// Update blob size (atomic)
    /// Returns None if the blob does not exist
    async fn update_blob_size(&self, id: TypedUuid<BlobId>, size: i64)
    -> StorageResult<Option<()>>;

    /// Update blob state (atomic, creates a new state transition record)
    /// Only succeeds if from_state matches the current state
    /// Returns None if the blob does not exist
    async fn update_blob_state(
        &self,
        id: TypedUuid<BlobId>,
        from_state: BlobState,
        to_state: BlobState,
    ) -> StorageResult<Option<()>>;

    /// Get all state transitions for a blob (ordered by creation time)
    /// Returns None if the blob does not exist
    async fn get_blob_state_history(
        &self,
        id: TypedUuid<BlobId>,
    ) -> StorageResult<Option<Vec<BlobStateTransition>>>;

    /// List all blobs for a specific service
    async fn list_blobs_by_service(
        &self,
        service_id: TypedUuid<ServiceId>,
    ) -> StorageResult<Vec<BlobModel>>;
}

/// Storage interface for health check operations
#[async_trait]
pub trait HealthCheckStorage: Send + Sync {
    /// Create a new health check record
    async fn create_health_check(
        &self,
        health_check: &NewHealthCheckModel,
    ) -> StorageResult<HealthCheckModel>;

    /// Get the most recent health check for a server registration
    /// Returns None if no health check exists for the registration
    async fn get_latest_health_check(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Option<HealthCheckModel>>;

    /// List all health checks for a server registration (ordered by checked_in_at DESC)
    async fn list_health_checks_by_server_registration(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Vec<HealthCheckModel>>;
}

/// Storage interface for idempotent request operations
#[async_trait]
pub trait IdempotentRequestStorage: Send + Sync {
    /// Create a new idempotent request (created with Processing state)
    async fn create_request(
        &self,
        request: &NewIdempotentRequestModel,
    ) -> StorageResult<IdempotentRequestModel>;

    /// Get an idempotent request by server registration ID and idempotency key
    /// Returns None if the request does not exist
    async fn get_request(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
        idempotency_key: &str,
    ) -> StorageResult<Option<IdempotentRequestModel>>;

    /// Complete an idempotent request by setting the response and atomically
    /// transitioning from Processing to Complete state.
    /// Only succeeds if the request is currently in Processing state.
    /// Returns None if the request does not exist
    async fn complete_request(
        &self,
        id: TypedUuid<IdempotentRequestId>,
        response: Option<serde_json::Value>,
    ) -> StorageResult<Option<()>>;

    /// Delete expired idempotent requests (where expires_at < now)
    async fn delete_expired_requests(&self) -> StorageResult<u64>;
}

/// Storage interface for token request operations
#[async_trait]
pub trait TokenRequestStorage: Send + Sync {
    /// Create a new token request
    /// The request is created with Pending state
    async fn create_token_request(
        &self,
        request: &NewTokenRequestModel,
    ) -> StorageResult<TokenRequestModel>;

    /// Get a token request by ID
    /// Returns None if the request does not exist
    async fn get_token_request(
        &self,
        id: TypedUuid<TokenRequestId>,
    ) -> StorageResult<Option<TokenRequestModel>>;

    /// List all token requests for a server registration
    async fn list_token_requests_by_server_registration(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Vec<TokenRequestModel>>;

    /// Update token request state (atomic, creates a new state transition record)
    /// Only succeeds if from_state matches the current state
    /// Returns None if the request does not exist
    async fn update_token_request_state(
        &self,
        id: TypedUuid<TokenRequestId>,
        from_state: TokenRequestState,
        to_state: TokenRequestState,
    ) -> StorageResult<Option<()>>;

    /// Get all state transitions for a token request (ordered by creation time)
    /// Returns None if the request does not exist
    async fn get_token_request_state_history(
        &self,
        id: TypedUuid<TokenRequestId>,
    ) -> StorageResult<Option<Vec<TokenRequestStateTransition>>>;

    /// Delete a token request
    /// Returns None if the request does not exist
    async fn delete_token_request(
        &self,
        id: TypedUuid<TokenRequestId>,
    ) -> StorageResult<Option<()>>;
}

/// Combined storage interface
#[async_trait]
pub trait Storage:
    ServiceStorage
    + ServerRegistrationStorage
    + BlobStorage
    + HealthCheckStorage
    + IdempotentRequestStorage
    + TokenRequestStorage
    + Send
    + Sync
{
}
