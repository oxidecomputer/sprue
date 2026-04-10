use async_trait::async_trait;
use chrono::Utc;
use newtype_uuid::{GenericUuid, TypedUuid};
use sqlx::{PgPool, Row};

use super::{
    BlobStateTransition, BlobStorage, HealthCheckStorage, IdempotentRequestStorage,
    ServerRegistrationStateTransition, ServerRegistrationStorage, ServiceStorage, Storage,
    StorageError, StorageResult,
};
use crate::db::{
    BlobModel, HealthCheckModel, IdempotentRequestModel, NewBlobModel, NewHealthCheckModel,
    NewIdempotentRequestModel, NewServerRegistrationModel, NewServiceModel,
    ServerRegistrationModel, ServiceModel,
};
use crate::{
    BlobId, BlobState, IdempotentRequestId, IdempotentRequestState, ServerRegistrationId,
    ServerRegistrationInstanceId, ServerRegistrationState, ServiceId,
};

/// PostgreSQL storage implementation
#[derive(Clone)]
pub struct PostgresStorage {
    pool: PgPool,
}

impl PostgresStorage {
    /// Create a new PostgreSQL storage instance
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Create a new PostgreSQL storage instance from a connection URL
    pub fn create(url: &str) -> StorageResult<Self> {
        Ok(Self::new(PgPool::connect_lazy(url)?))
    }

    /// Get a reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait]
impl ServiceStorage for PostgresStorage {
    async fn create_service(&self, service: &NewServiceModel) -> StorageResult<ServiceModel> {
        let now = Utc::now();
        let row = sqlx::query(
            r#"
            INSERT INTO service (name, created_at)
            VALUES ($1, $2)
            RETURNING id, name, created_at
            "#,
        )
        .bind(&service.name)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
        Ok(ServiceModel {
            id: TypedUuid::from_untyped_uuid(id_uuid),
            name: row.try_get("name")?,
            created_at: row.try_get("created_at")?,
        })
    }

    async fn get_service_by_id(
        &self,
        id: TypedUuid<ServiceId>,
    ) -> StorageResult<Option<ServiceModel>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, created_at
            FROM service
            WHERE id = $1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                Ok(Some(ServiceModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    name: row.try_get("name")?,
                    created_at: row.try_get("created_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_service_by_name(&self, name: &str) -> StorageResult<Option<ServiceModel>> {
        let row = sqlx::query(
            r#"
            SELECT id, name, created_at
            FROM service
            WHERE name = $1
            "#,
        )
        .bind(name)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                Ok(Some(ServiceModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    name: row.try_get("name")?,
                    created_at: row.try_get("created_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_services(&self) -> StorageResult<Vec<ServiceModel>> {
        let rows = sqlx::query(
            r#"
            SELECT id, name, created_at
            FROM service
            ORDER BY name
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let services = rows
            .iter()
            .map(|row| {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                Ok(ServiceModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    name: row.try_get("name")?,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(services)
    }

    async fn delete_service(&self, name: &str) -> StorageResult<Option<()>> {
        let result = sqlx::query(
            r#"
            DELETE FROM service
            WHERE name = $1
            "#,
        )
        .bind(name)
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        Ok(Some(()))
    }
}

#[async_trait]
impl ServerRegistrationStorage for PostgresStorage {
    async fn create_server_registration(
        &self,
        registration: &NewServerRegistrationModel,
    ) -> StorageResult<ServerRegistrationModel> {
        let now = Utc::now();
        let pending_state = ServerRegistrationState::Pending;
        let state_json = serde_json::to_value(&pending_state).map_err(|e| {
            StorageError::Internal(format!(
                "Failed to serialize server registration state: {}",
                e
            ))
        })?;

        let mut tx = self.pool.begin().await?;

        // Insert the server registration record
        let row = sqlx::query(
            r#"
            INSERT INTO server_registration (service_id, instance_id, created_at, updated_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id, service_id, instance_id, created_at, updated_at
            "#,
        )
        .bind(registration.service_id.as_untyped_uuid())
        .bind(registration.instance_id.as_untyped_uuid())
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            // Check for unique constraint violation on instance_id
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("server_registration_instance_id_key") {
                    return StorageError::ServerRegistrationAlreadyExists(registration.instance_id);
                }
            }
            StorageError::Database(e)
        })?;

        let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
        let registration_id = TypedUuid::from_untyped_uuid(id_uuid);

        // Insert initial Pending state into server_registration_state table
        sqlx::query(
            r#"
            INSERT INTO server_registration_state (server_registration_id, state, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(registration_id.as_untyped_uuid())
        .bind(&state_json)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
        let instance_id_uuid: sqlx::types::Uuid = row.try_get("instance_id")?;
        Ok(ServerRegistrationModel {
            id: registration_id,
            service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
            instance_id: TypedUuid::from_untyped_uuid(instance_id_uuid),
            state: pending_state,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    async fn get_server_registration(
        &self,
        id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Option<ServerRegistrationModel>> {
        let row = sqlx::query(
            r#"
            SELECT sr.id, sr.service_id, sr.instance_id, sr.created_at, sr.updated_at, srs.state
            FROM server_registration sr
            JOIN server_registration_state srs ON srs.server_registration_id = sr.id
            WHERE sr.id = $1
            ORDER BY srs.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let instance_id_uuid: sqlx::types::Uuid = row.try_get("instance_id")?;
                let state_json: serde_json::Value = row.try_get("state")?;
                let state: ServerRegistrationState =
                    serde_json::from_value(state_json).map_err(|e| {
                        StorageError::Internal(format!(
                            "Failed to deserialize server registration state: {}",
                            e
                        ))
                    })?;

                Ok(Some(ServerRegistrationModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    instance_id: TypedUuid::from_untyped_uuid(instance_id_uuid),
                    state,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn get_server_registration_by_instance_id(
        &self,
        instance_id: TypedUuid<ServerRegistrationInstanceId>,
    ) -> StorageResult<Option<ServerRegistrationModel>> {
        let row = sqlx::query(
            r#"
            SELECT sr.id, sr.service_id, sr.instance_id, sr.created_at, sr.updated_at, srs.state
            FROM server_registration sr
            JOIN server_registration_state srs ON srs.server_registration_id = sr.id
            WHERE sr.instance_id = $1
            ORDER BY srs.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(instance_id.as_untyped_uuid())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let instance_id_uuid: sqlx::types::Uuid = row.try_get("instance_id")?;
                let state_json: serde_json::Value = row.try_get("state")?;
                let state: ServerRegistrationState =
                    serde_json::from_value(state_json).map_err(|e| {
                        StorageError::Internal(format!(
                            "Failed to deserialize server registration state: {}",
                            e
                        ))
                    })?;

                Ok(Some(ServerRegistrationModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    instance_id: TypedUuid::from_untyped_uuid(instance_id_uuid),
                    state,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_server_registrations_by_service_id(
        &self,
        service_id: TypedUuid<ServiceId>,
    ) -> StorageResult<Vec<ServerRegistrationModel>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (sr.id) sr.id, sr.service_id, sr.instance_id, sr.created_at, sr.updated_at, srs.state
            FROM server_registration sr
            JOIN server_registration_state srs ON srs.server_registration_id = sr.id
            WHERE sr.service_id = $1
            ORDER BY sr.id, srs.created_at DESC
            "#,
        )
        .bind(service_id.as_untyped_uuid())
        .fetch_all(&self.pool)
        .await?;

        let registrations = rows
            .iter()
            .map(|row| {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let instance_id_uuid: sqlx::types::Uuid = row.try_get("instance_id")?;
                let state_json: serde_json::Value = row.try_get("state")?;
                let state: ServerRegistrationState =
                    serde_json::from_value(state_json).map_err(|e| {
                        sqlx::Error::Decode(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Failed to deserialize server registration state: {}", e),
                        )))
                    })?;

                Ok(ServerRegistrationModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    instance_id: TypedUuid::from_untyped_uuid(instance_id_uuid),
                    state,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(registrations)
    }

    async fn update_server_registration_state(
        &self,
        id: TypedUuid<ServerRegistrationId>,
        from_state: ServerRegistrationState,
        to_state: ServerRegistrationState,
    ) -> StorageResult<Option<()>> {
        let now = Utc::now();
        let from_state_json = serde_json::to_value(&from_state).map_err(|e| {
            StorageError::Internal(format!("Failed to serialize from_state: {}", e))
        })?;
        let to_state_json = serde_json::to_value(&to_state)
            .map_err(|e| StorageError::Internal(format!("Failed to serialize to_state: {}", e)))?;

        let mut tx = self.pool.begin().await?;

        // Check current state matches from_state
        let current_state_row = sqlx::query(
            r#"
            SELECT state FROM server_registration_state
            WHERE server_registration_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .fetch_optional(&mut *tx)
        .await?;

        match current_state_row {
            Some(row) => {
                let current_state_json: serde_json::Value = row.try_get("state")?;
                if current_state_json != from_state_json {
                    return Err(StorageError::InvalidServerRegistrationStateTransition {
                        server_registration_id: id,
                        expected: from_state,
                    });
                }
            }
            None => return Ok(None),
        }

        // Insert new state
        sqlx::query(
            r#"
            INSERT INTO server_registration_state (server_registration_id, state, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(id.as_untyped_uuid())
        .bind(&to_state_json)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        // Update server_registration updated_at
        sqlx::query(
            r#"
            UPDATE server_registration SET updated_at = $1 WHERE id = $2
            "#,
        )
        .bind(now)
        .bind(id.as_untyped_uuid())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Some(()))
    }

    async fn get_server_registration_state_history(
        &self,
        id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Option<Vec<ServerRegistrationStateTransition>>> {
        let rows = sqlx::query(
            r#"
            SELECT state, created_at
            FROM server_registration_state
            WHERE server_registration_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(id.as_untyped_uuid())
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let transitions = rows
            .iter()
            .map(|row| {
                let state_json: serde_json::Value = row.try_get("state")?;
                let state: ServerRegistrationState =
                    serde_json::from_value(state_json).map_err(|e| {
                        sqlx::Error::Decode(Box::new(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            format!("Failed to deserialize server registration state: {}", e),
                        )))
                    })?;
                Ok(ServerRegistrationStateTransition {
                    state,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(Some(transitions))
    }

    async fn delete_server_registration(
        &self,
        id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Option<()>> {
        let result = sqlx::query(
            r#"
            DELETE FROM server_registration
            WHERE id = $1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        Ok(Some(()))
    }

    async fn delete_server_registration_by_instance_id(
        &self,
        instance_id: TypedUuid<ServerRegistrationInstanceId>,
    ) -> StorageResult<Option<()>> {
        let result = sqlx::query(
            r#"
            DELETE FROM server_registration
            WHERE instance_id = $1
            "#,
        )
        .bind(instance_id.as_untyped_uuid())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        Ok(Some(()))
    }
}

#[async_trait]
impl BlobStorage for PostgresStorage {
    async fn create_blob(&self, blob: &NewBlobModel) -> StorageResult<BlobModel> {
        let now = Utc::now();
        let pending_state = BlobState::Pending;
        let state_json = serde_json::to_value(&pending_state).map_err(|e| {
            StorageError::Internal(format!("Failed to serialize blob state: {}", e))
        })?;

        let mut tx = self.pool.begin().await?;

        // Insert the blob record
        let row = sqlx::query(
            r#"
            INSERT INTO blob (service_id, blob_time, size, total_size, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING id, service_id, blob_time, size, total_size, created_at, updated_at
            "#,
        )
        .bind(blob.service_id.as_untyped_uuid())
        .bind(0i64) // Initial size is 0
        .bind(blob.total_size)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;

        let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
        let blob_id = TypedUuid::from_untyped_uuid(id_uuid);

        // Insert the initial state transition
        sqlx::query(
            r#"
            INSERT INTO blob_state (blob_id, state, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(blob_id.as_untyped_uuid())
        .bind(&state_json)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
        Ok(BlobModel {
            id: blob_id,
            service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
            size: row.try_get("size")?,
            total_size: row.try_get("total_size")?,
            state: pending_state,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    async fn get_blob(&self, id: TypedUuid<BlobId>) -> StorageResult<Option<BlobModel>> {
        let row = sqlx::query(
            r#"
            SELECT b.id, b.service_id, b.blob_time, b.size, b.total_size, b.created_at, b.updated_at,
                   bs.state
            FROM blob b
            JOIN blob_state bs ON bs.blob_id = b.id
            WHERE b.id = $1
            ORDER BY bs.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let state_json: serde_json::Value = row.try_get("state")?;
                let state: BlobState = serde_json::from_value(state_json).map_err(|e| {
                    StorageError::Internal(format!("Failed to deserialize blob state: {}", e))
                })?;

                Ok(Some(BlobModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    size: row.try_get("size")?,
                    total_size: row.try_get("total_size")?,
                    state,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_blobs(&self) -> StorageResult<Vec<BlobModel>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (b.id) b.id, b.service_id, b.blob_time, b.size, b.total_size,
                   b.created_at, b.updated_at, bs.state
            FROM blob b
            JOIN blob_state bs ON bs.blob_id = b.id
            ORDER BY b.id, bs.created_at DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let blobs = rows
            .iter()
            .map(|row| {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let state_json: serde_json::Value = row.try_get("state")?;
                let state: BlobState = serde_json::from_value(state_json).map_err(|e| {
                    sqlx::Error::Decode(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to deserialize blob state: {}", e),
                    )))
                })?;

                Ok(BlobModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    size: row.try_get("size")?,
                    total_size: row.try_get("total_size")?,
                    state,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(blobs)
    }

    async fn update_blob_size(
        &self,
        id: TypedUuid<BlobId>,
        size: i64,
    ) -> StorageResult<Option<()>> {
        let now = Utc::now();
        let result = sqlx::query(
            r#"
            UPDATE blob
            SET size = $1, updated_at = $2
            WHERE id = $3
            "#,
        )
        .bind(size)
        .bind(now)
        .bind(id.as_untyped_uuid())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() == 0 {
            return Ok(None);
        }

        Ok(Some(()))
    }

    async fn update_blob_state(
        &self,
        id: TypedUuid<BlobId>,
        from_state: BlobState,
        to_state: BlobState,
    ) -> StorageResult<Option<()>> {
        let now = Utc::now();
        let from_state_json = serde_json::to_value(&from_state).map_err(|e| {
            StorageError::Internal(format!("Failed to serialize from_state: {}", e))
        })?;
        let to_state_json = serde_json::to_value(&to_state)
            .map_err(|e| StorageError::Internal(format!("Failed to serialize to_state: {}", e)))?;

        let mut tx = self.pool.begin().await?;

        // Check current state matches from_state
        let current_state_row = sqlx::query(
            r#"
            SELECT state FROM blob_state
            WHERE blob_id = $1
            ORDER BY created_at DESC
            LIMIT 1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .fetch_optional(&mut *tx)
        .await?;

        match current_state_row {
            Some(row) => {
                let current_state_json: serde_json::Value = row.try_get("state")?;
                if current_state_json != from_state_json {
                    return Err(StorageError::InvalidBlobStateTransition {
                        blob_id: id,
                        expected: from_state,
                    });
                }
            }
            None => return Ok(None),
        }

        // Insert new state
        sqlx::query(
            r#"
            INSERT INTO blob_state (blob_id, state, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(id.as_untyped_uuid())
        .bind(&to_state_json)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        // Update blob updated_at
        sqlx::query(
            r#"
            UPDATE blob SET updated_at = $1 WHERE id = $2
            "#,
        )
        .bind(now)
        .bind(id.as_untyped_uuid())
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        Ok(Some(()))
    }

    async fn get_blob_state_history(
        &self,
        id: TypedUuid<BlobId>,
    ) -> StorageResult<Option<Vec<BlobStateTransition>>> {
        let rows = sqlx::query(
            r#"
            SELECT state, created_at
            FROM blob_state
            WHERE blob_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(id.as_untyped_uuid())
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok(None);
        }

        let transitions = rows
            .iter()
            .map(|row| {
                let state_json: serde_json::Value = row.try_get("state")?;
                let state: BlobState = serde_json::from_value(state_json).map_err(|e| {
                    sqlx::Error::Decode(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to deserialize blob state: {}", e),
                    )))
                })?;
                Ok(BlobStateTransition {
                    state,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(Some(transitions))
    }

    async fn list_blobs_by_service(
        &self,
        service_id: TypedUuid<ServiceId>,
    ) -> StorageResult<Vec<BlobModel>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (b.id) b.id, b.service_id, b.blob_time, b.size, b.total_size,
                   b.created_at, b.updated_at, bs.state
            FROM blob b
            JOIN blob_state bs ON bs.blob_id = b.id
            WHERE b.service_id = $1
            ORDER BY b.id, bs.created_at DESC
            "#,
        )
        .bind(service_id.as_untyped_uuid())
        .fetch_all(&self.pool)
        .await?;

        let blobs = rows
            .iter()
            .map(|row| {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let state_json: serde_json::Value = row.try_get("state")?;
                let state: BlobState = serde_json::from_value(state_json).map_err(|e| {
                    sqlx::Error::Decode(Box::new(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("Failed to deserialize blob state: {}", e),
                    )))
                })?;

                Ok(BlobModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    size: row.try_get("size")?,
                    total_size: row.try_get("total_size")?,
                    state,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(blobs)
    }
}

#[async_trait]
impl HealthCheckStorage for PostgresStorage {
    async fn create_health_check(
        &self,
        health_check: &NewHealthCheckModel,
    ) -> StorageResult<HealthCheckModel> {
        let now = Utc::now();
        let row = sqlx::query(
            r#"
            INSERT INTO health_check (server_registration_id, checked_in_at)
            VALUES ($1, $2)
            RETURNING id, server_registration_id, checked_in_at
            "#,
        )
        .bind(health_check.server_registration_id.as_untyped_uuid())
        .bind(health_check.checked_in_at)
        .fetch_one(&self.pool)
        .await?;

        let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
        let server_registration_id_uuid: sqlx::types::Uuid =
            row.try_get("server_registration_id")?;
        Ok(HealthCheckModel {
            id: TypedUuid::from_untyped_uuid(id_uuid),
            server_registration_id: TypedUuid::from_untyped_uuid(server_registration_id_uuid),
            checked_in_at: row.try_get("checked_in_at")?,
            created_at: now,
        })
    }

    async fn get_latest_health_check(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Option<HealthCheckModel>> {
        let row = sqlx::query(
            r#"
            SELECT id, server_registration_id, checked_in_at
            FROM health_check
            WHERE server_registration_id = $1
            ORDER BY checked_in_at DESC
            LIMIT 1
            "#,
        )
        .bind(server_registration_id.as_untyped_uuid())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let server_registration_id_uuid: sqlx::types::Uuid =
                    row.try_get("server_registration_id")?;
                Ok(Some(HealthCheckModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    server_registration_id: TypedUuid::from_untyped_uuid(
                        server_registration_id_uuid,
                    ),
                    checked_in_at: row.try_get("checked_in_at")?,
                    created_at: row.try_get("checked_in_at")?, // Using checked_in_at as proxy
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_health_checks_by_server_registration(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Vec<HealthCheckModel>> {
        let rows = sqlx::query(
            r#"
            SELECT id, server_registration_id, checked_in_at
            FROM health_check
            WHERE server_registration_id = $1
            ORDER BY checked_in_at DESC
            "#,
        )
        .bind(server_registration_id.as_untyped_uuid())
        .fetch_all(&self.pool)
        .await?;

        let health_checks = rows
            .iter()
            .map(|row| {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let server_registration_id_uuid: sqlx::types::Uuid =
                    row.try_get("server_registration_id")?;
                Ok(HealthCheckModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    server_registration_id: TypedUuid::from_untyped_uuid(
                        server_registration_id_uuid,
                    ),
                    checked_in_at: row.try_get("checked_in_at")?,
                    created_at: row.try_get("checked_in_at")?, // Using checked_in_at as proxy
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(health_checks)
    }
}

#[async_trait]
impl IdempotentRequestStorage for PostgresStorage {
    async fn create_request(
        &self,
        request: &NewIdempotentRequestModel,
    ) -> StorageResult<IdempotentRequestModel> {
        let now = Utc::now();
        let processing_state = IdempotentRequestState::Processing;
        let state_json = serde_json::to_value(&processing_state).map_err(|e| {
            StorageError::Internal(format!(
                "Failed to serialize idempotent request state: {}",
                e
            ))
        })?;

        let mut tx = self.pool.begin().await?;

        // Insert the idempotent request record
        let row = sqlx::query(
            r#"
            INSERT INTO idempotent_request (server_registration_id, idempotency_key, expires_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, server_registration_id, idempotency_key, response, expires_at, created_at, updated_at
            "#,
        )
        .bind(request.server_registration_id.as_untyped_uuid())
        .bind(&request.idempotency_key)
        .bind(request.expires_at)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            // Check for unique constraint violation on idempotency_key
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint() == Some("idempotent_request_unique_key") {
                    return StorageError::IdempotentRequestAlreadyExists(
                        request.idempotency_key.clone(),
                    );
                }
            }
            StorageError::Database(e)
        })?;

        let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
        let request_id = TypedUuid::from_untyped_uuid(id_uuid);

        // Insert initial Processing state into idempotent_request_state table
        sqlx::query(
            r#"
            INSERT INTO idempotent_request_state (idempotent_request_id, state, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(request_id.as_untyped_uuid())
        .bind(&state_json)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let server_registration_id_uuid: sqlx::types::Uuid =
            row.try_get("server_registration_id")?;
        let response: Option<serde_json::Value> = row.try_get("response")?;

        Ok(IdempotentRequestModel {
            id: request_id,
            server_registration_id: TypedUuid::from_untyped_uuid(server_registration_id_uuid),
            idempotency_key: row.try_get("idempotency_key")?,
            response,
            state: processing_state,
            expires_at: row.try_get("expires_at")?,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    async fn get_request(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
        idempotency_key: &str,
    ) -> StorageResult<Option<IdempotentRequestModel>> {
        let row = sqlx::query(
            r#"
            SELECT ir.id, ir.server_registration_id, ir.idempotency_key, ir.response, ir.expires_at, ir.created_at, ir.updated_at, irs.state
            FROM idempotent_request ir
            JOIN idempotent_request_state irs ON irs.idempotent_request_id = ir.id
            WHERE ir.server_registration_id = $1 AND ir.idempotency_key = $2
            ORDER BY irs.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(server_registration_id.as_untyped_uuid())
        .bind(idempotency_key)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let server_registration_id_uuid: sqlx::types::Uuid =
                    row.try_get("server_registration_id")?;
                let state_json: serde_json::Value = row.try_get("state")?;
                let state: IdempotentRequestState =
                    serde_json::from_value(state_json).map_err(|e| {
                        StorageError::Internal(format!(
                            "Failed to deserialize idempotent request state: {}",
                            e
                        ))
                    })?;
                let response: Option<serde_json::Value> = row.try_get("response")?;

                Ok(Some(IdempotentRequestModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    server_registration_id: TypedUuid::from_untyped_uuid(
                        server_registration_id_uuid,
                    ),
                    idempotency_key: row.try_get("idempotency_key")?,
                    response,
                    state,
                    expires_at: row.try_get("expires_at")?,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn complete_request(
        &self,
        id: TypedUuid<IdempotentRequestId>,
        response: Option<serde_json::Value>,
    ) -> StorageResult<Option<()>> {
        let now = Utc::now();
        let processing_state_json = serde_json::to_value(&IdempotentRequestState::Processing)
            .map_err(|e| {
                StorageError::Internal(format!(
                    "Failed to serialize idempotent request state: {}",
                    e
                ))
            })?;
        let complete_state_json =
            serde_json::to_value(&IdempotentRequestState::Complete).map_err(|e| {
                StorageError::Internal(format!(
                    "Failed to serialize idempotent request state: {}",
                    e
                ))
            })?;

        // Use a CTE to atomically:
        // 1. Check the current state is Processing
        // 2. Update the idempotent_request with the response
        // 3. Insert the Complete state into idempotent_request_state
        let row = sqlx::query(
            r#"
            WITH current_state AS (
                SELECT irs.state
                FROM idempotent_request ir
                JOIN idempotent_request_state irs ON irs.idempotent_request_id = ir.id
                WHERE ir.id = $1
                ORDER BY irs.created_at DESC
                LIMIT 1
            ),
            updated AS (
                UPDATE idempotent_request
                SET response = $2, updated_at = $3
                WHERE id = $1
                AND EXISTS (SELECT 1 FROM current_state WHERE state = $4)
                RETURNING id
            ),
            new_state AS (
                INSERT INTO idempotent_request_state (idempotent_request_id, state, created_at)
                SELECT $1, $5, $3
                FROM updated
                RETURNING idempotent_request_id
            )
            SELECT
                EXISTS (SELECT 1 FROM idempotent_request WHERE id = $1) as request_exists,
                (SELECT state FROM current_state) as current_state,
                EXISTS (SELECT 1 FROM new_state) as completed
            "#,
        )
        .bind(id.as_untyped_uuid())
        .bind(&response)
        .bind(now)
        .bind(&processing_state_json)
        .bind(&complete_state_json)
        .fetch_one(&self.pool)
        .await?;

        let request_exists: bool = row.try_get("request_exists")?;
        let current_state: Option<serde_json::Value> = row.try_get("current_state")?;
        let completed: bool = row.try_get("completed")?;

        if !request_exists {
            return Ok(None);
        }

        if !completed {
            // Request exists but wasn't updated - must be in wrong state
            if current_state.is_none() {
                return Ok(None);
            }
            return Err(StorageError::InvalidIdempotentRequestStateTransition {
                idempotent_request_id: id,
                expected: IdempotentRequestState::Processing,
            });
        }

        Ok(Some(()))
    }

    async fn delete_expired_requests(&self) -> StorageResult<u64> {
        let now = Utc::now();

        let result = sqlx::query(
            r#"
            DELETE FROM idempotent_request
            WHERE expires_at < $1
            "#,
        )
        .bind(now)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected())
    }
}

impl Storage for PostgresStorage {}
