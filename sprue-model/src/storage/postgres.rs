// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use async_trait::async_trait;
use chrono::Utc;
use newtype_uuid::{GenericUuid, TypedUuid};
use sqlx::{PgPool, Row};

use super::{
    BlobFilter, BlobStateTransition, BlobStorage, DeploymentStorage, HealthCheckStorage,
    IdempotentRequestStorage, Paginated, ServerRegistrationStateTransition,
    ServerRegistrationStorage, ServiceStorage, Storage, StorageError, StorageResult,
    TokenRequestStateTransition, TokenRequestStorage,
};
use crate::db::{
    BlobModel, DeploymentModel, HealthCheckModel, IdempotentRequestModel, NewBlobModel,
    NewDeploymentModel, NewHealthCheckModel, NewIdempotentRequestModel, NewServerRegistrationModel,
    NewServiceModel, NewTokenRequestModel, ServerRegistrationModel, ServiceModel,
    TokenRequestModel,
};
use crate::{
    BlobId, BlobState, DeploymentId, IdempotentRequestId, IdempotentRequestState,
    ServerRegistrationId, ServerRegistrationInstanceId, ServerRegistrationState, ServiceId,
    TokenRequestId, TokenRequestState,
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

    async fn get_service(&self, id: TypedUuid<ServiceId>) -> StorageResult<Option<ServiceModel>> {
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

    async fn delete_service(&self, id: TypedUuid<ServiceId>) -> StorageResult<Option<()>> {
        let result = sqlx::query(
            r#"
            DELETE FROM service
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
}

#[async_trait]
impl DeploymentStorage for PostgresStorage {
    async fn create_deployment(
        &self,
        deployment: &NewDeploymentModel,
    ) -> StorageResult<DeploymentModel> {
        let now = Utc::now();
        let row = sqlx::query(
            r#"
            INSERT INTO deployment (service_id, project_id, silo_id, created_at)
            VALUES ($1, $2, $3, $4)
            RETURNING id, service_id, project_id, silo_id, created_at
            "#,
        )
        .bind(deployment.service_id.as_untyped_uuid())
        .bind(deployment.project_id.as_untyped_uuid())
        .bind(deployment.silo_id.as_untyped_uuid())
        .bind(now)
        .fetch_one(&self.pool)
        .await?;

        let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
        let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
        Ok(DeploymentModel {
            id: TypedUuid::from_untyped_uuid(id_uuid),
            service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
            project_id: {
                let v: sqlx::types::Uuid = row.try_get("project_id")?;
                TypedUuid::from_untyped_uuid(v)
            },
            silo_id: {
                let v: sqlx::types::Uuid = row.try_get("silo_id")?;
                TypedUuid::from_untyped_uuid(v)
            },
            created_at: row.try_get("created_at")?,
        })
    }

    async fn get_deployment(
        &self,
        id: TypedUuid<DeploymentId>,
    ) -> StorageResult<Option<DeploymentModel>> {
        let row = sqlx::query(
            r#"
            SELECT id, service_id, project_id, silo_id, created_at
            FROM deployment
            WHERE id = $1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                Ok(Some(DeploymentModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    project_id: {
                        let v: sqlx::types::Uuid = row.try_get("project_id")?;
                        TypedUuid::from_untyped_uuid(v)
                    },
                    silo_id: {
                        let v: sqlx::types::Uuid = row.try_get("silo_id")?;
                        TypedUuid::from_untyped_uuid(v)
                    },
                    created_at: row.try_get("created_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_deployments_by_service_id(
        &self,
        service_id: TypedUuid<ServiceId>,
    ) -> StorageResult<Vec<DeploymentModel>> {
        let rows = sqlx::query(
            r#"
            SELECT id, service_id, project_id, silo_id, created_at
            FROM deployment
            WHERE service_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(service_id.as_untyped_uuid())
        .fetch_all(&self.pool)
        .await?;

        let mut deployments = Vec::with_capacity(rows.len());
        for row in rows {
            let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
            let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
            deployments.push(DeploymentModel {
                id: TypedUuid::from_untyped_uuid(id_uuid),
                service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                project_id: {
                    let v: sqlx::types::Uuid = row.try_get("project_id")?;
                    TypedUuid::from_untyped_uuid(v)
                },
                silo_id: {
                    let v: sqlx::types::Uuid = row.try_get("silo_id")?;
                    TypedUuid::from_untyped_uuid(v)
                },
                created_at: row.try_get("created_at")?,
            });
        }

        Ok(deployments)
    }

    async fn delete_deployment(&self, id: TypedUuid<DeploymentId>) -> StorageResult<Option<()>> {
        let result = sqlx::query(
            r#"
            DELETE FROM deployment
            WHERE id = $1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .execute(&self.pool)
        .await?;

        if result.rows_affected() > 0 {
            Ok(Some(()))
        } else {
            Ok(None)
        }
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

        let mut tx = self.pool.begin().await?;

        // Insert the server registration record
        let row = sqlx::query(
            r#"
            INSERT INTO server_registration (service_id, instance_id, project_id, silo_id, nonce, expires_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            RETURNING id, service_id, instance_id, project_id, silo_id, nonce, expires_at, created_at, updated_at
            "#,
        )
        .bind(registration.service_id.as_untyped_uuid())
        .bind(registration.instance_id.as_untyped_uuid())
        .bind(registration.project_id.as_untyped_uuid())
        .bind(registration.silo_id.as_untyped_uuid())
        .bind(registration.nonce.as_deref().unwrap_or_default())
        .bind(registration.expires_at)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            // Check for unique constraint violation on instance_id
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.constraint() == Some("server_registration_instance_id_key") {
                    return StorageError::ServerRegistrationAlreadyExists(registration.instance_id);
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
        .bind(pending_state)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
        let instance_id_uuid: sqlx::types::Uuid = row.try_get("instance_id")?;
        let project_id_uuid: sqlx::types::Uuid = row.try_get("project_id")?;
        let silo_id_uuid: sqlx::types::Uuid = row.try_get("silo_id")?;
        Ok(ServerRegistrationModel {
            id: registration_id,
            service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
            instance_id: TypedUuid::from_untyped_uuid(instance_id_uuid),
            project_id: TypedUuid::from_untyped_uuid(project_id_uuid),
            silo_id: TypedUuid::from_untyped_uuid(silo_id_uuid),
            nonce: row.try_get("nonce")?,
            expires_at: row.try_get("expires_at")?,
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
            SELECT sr.id, sr.service_id, sr.instance_id, sr.project_id, sr.silo_id, sr.nonce, sr.expires_at, sr.created_at, sr.updated_at, srs.state
            FROM server_registration sr
            JOIN server_registration_state srs ON srs.server_registration_id = sr.id
            WHERE sr.id = $1 AND srs.state != $2
            ORDER BY srs.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .bind(ServerRegistrationState::Expired)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let instance_id_uuid: sqlx::types::Uuid = row.try_get("instance_id")?;
                let project_id_uuid: sqlx::types::Uuid = row.try_get("project_id")?;
                let silo_id_uuid: sqlx::types::Uuid = row.try_get("silo_id")?;
                let state: ServerRegistrationState = row.try_get("state")?;

                Ok(Some(ServerRegistrationModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    instance_id: TypedUuid::from_untyped_uuid(instance_id_uuid),
                    project_id: TypedUuid::from_untyped_uuid(project_id_uuid),
                    silo_id: TypedUuid::from_untyped_uuid(silo_id_uuid),
                    nonce: row.try_get("nonce")?,
                    expires_at: row.try_get("expires_at")?,
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
            SELECT sr.id, sr.service_id, sr.instance_id, sr.project_id, sr.silo_id, sr.nonce, sr.expires_at, sr.created_at, sr.updated_at, srs.state
            FROM server_registration sr
            JOIN server_registration_state srs ON srs.server_registration_id = sr.id
            WHERE sr.instance_id = $1 AND srs.state != $2
            ORDER BY srs.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(instance_id.as_untyped_uuid())
        .bind(ServerRegistrationState::Expired)
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let instance_id_uuid: sqlx::types::Uuid = row.try_get("instance_id")?;
                let project_id_uuid: sqlx::types::Uuid = row.try_get("project_id")?;
                let silo_id_uuid: sqlx::types::Uuid = row.try_get("silo_id")?;
                let state: ServerRegistrationState = row.try_get("state")?;

                Ok(Some(ServerRegistrationModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    instance_id: TypedUuid::from_untyped_uuid(instance_id_uuid),
                    project_id: TypedUuid::from_untyped_uuid(project_id_uuid),
                    silo_id: TypedUuid::from_untyped_uuid(silo_id_uuid),
                    nonce: row.try_get("nonce")?,
                    expires_at: row.try_get("expires_at")?,
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
            SELECT DISTINCT ON (sr.id) sr.id, sr.service_id, sr.instance_id, sr.project_id, sr.silo_id, sr.nonce, sr.expires_at, sr.created_at, sr.updated_at, srs.state
            FROM server_registration sr
            JOIN server_registration_state srs ON srs.server_registration_id = sr.id
            WHERE sr.service_id = $1 AND srs.state != $2
            ORDER BY sr.id, srs.created_at DESC
            "#,
        )
        .bind(service_id.as_untyped_uuid())
        .bind(ServerRegistrationState::Expired)
        .fetch_all(&self.pool)
        .await?;

        let registrations = rows
            .iter()
            .map(|row| {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let instance_id_uuid: sqlx::types::Uuid = row.try_get("instance_id")?;
                let project_id_uuid: sqlx::types::Uuid = row.try_get("project_id")?;
                let silo_id_uuid: sqlx::types::Uuid = row.try_get("silo_id")?;
                let state: ServerRegistrationState = row.try_get("state")?;

                Ok(ServerRegistrationModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    instance_id: TypedUuid::from_untyped_uuid(instance_id_uuid),
                    project_id: TypedUuid::from_untyped_uuid(project_id_uuid),
                    silo_id: TypedUuid::from_untyped_uuid(silo_id_uuid),
                    nonce: row.try_get("nonce")?,
                    expires_at: row.try_get("expires_at")?,
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

        // Use a CTE to atomically check the current state, insert the new state
        // transition, and update the registration in a single query.
        //
        // Any update to a server registration will mean blanking out the nonce. The only cases
        // where a value will be present are when the previous state is Pending and we are
        // transitioning to Proven, Rejected, or Terminated. In each of those cases we no longer
        // want to be storing the nonce. Once a request has been proven, rejected, or terminated,
        // the request is no longer active and we can also clear out the expiration time.
        let row = sqlx::query(
            r#"
            WITH current_state AS (
                SELECT state
                FROM server_registration_state
                WHERE server_registration_id = $1
                ORDER BY created_at DESC
                LIMIT 1
            ),
            insert_new_state AS (
                INSERT INTO server_registration_state (server_registration_id, state, created_at)
                SELECT $1, $2, $3
                FROM current_state
                WHERE current_state.state = $4
                RETURNING server_registration_id
            ),
            update_registration AS (
                UPDATE server_registration
                SET nonce = null, expires_at = null, updated_at = $3
                WHERE id = $1
                AND EXISTS (SELECT 1 FROM insert_new_state)
                RETURNING id
            )
            SELECT
                EXISTS (SELECT 1 FROM current_state) AS has_current_state,
                EXISTS (SELECT 1 FROM insert_new_state) AS was_inserted,
                (SELECT state FROM current_state) AS actual_state
            "#,
        )
        .bind(id.as_untyped_uuid())
        .bind(to_state)
        .bind(now)
        .bind(from_state)
        .fetch_one(&self.pool)
        .await?;

        let has_current_state: bool = row.try_get("has_current_state")?;
        let was_inserted: bool = row.try_get("was_inserted")?;

        if !has_current_state {
            return Ok(None);
        }

        if !was_inserted {
            let actual: ServerRegistrationState = row.try_get("actual_state")?;
            return Err(StorageError::InvalidServerRegistrationStateTransition {
                server_registration_id: id,
                expected: from_state,
                actual,
                to: to_state,
            });
        }

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
                let state: ServerRegistrationState = row.try_get("state")?;
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

        let mut tx = self.pool.begin().await?;

        // Insert the blob record
        let row = sqlx::query(
            r#"
            INSERT INTO blob (service_id, server_registration_id, blob_time, size, total_size, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING id, service_id, server_registration_id, blob_time, size, total_size, created_at, updated_at
            "#,
        )
        .bind(blob.service_id.as_untyped_uuid())
        .bind(blob.server_registration_id.as_untyped_uuid())
        .bind(blob.blob_time)
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
        .bind(pending_state)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
        let server_registration_id_uuid: sqlx::types::Uuid =
            row.try_get("server_registration_id")?;
        Ok(BlobModel {
            id: blob_id,
            service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
            server_registration_id: TypedUuid::from_untyped_uuid(server_registration_id_uuid),
            blob_time: row.try_get("blob_time")?,
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
            SELECT b.id, b.service_id, b.server_registration_id, b.blob_time, b.size, b.total_size, b.created_at, b.updated_at,
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
                let server_registration_id_uuid: sqlx::types::Uuid =
                    row.try_get("server_registration_id")?;
                let state: BlobState = row.try_get("state")?;

                Ok(Some(BlobModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    server_registration_id: TypedUuid::from_untyped_uuid(
                        server_registration_id_uuid,
                    ),
                    blob_time: row.try_get("blob_time")?,
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

    async fn list_blobs(&self, filter: &BlobFilter) -> StorageResult<Vec<BlobModel>> {
        // Build the query dynamically based on which filter fields are set.
        // We use a CTE to get the latest state per blob first, then filter.
        let mut sql = String::from(
            "SELECT q.id, q.service_id, q.server_registration_id, q.blob_time, \
             q.size, q.total_size, q.created_at, q.updated_at, q.state \
             FROM ( \
               SELECT DISTINCT ON (b.id) b.id, b.service_id, b.server_registration_id, \
                      b.blob_time, b.size, b.total_size, b.created_at, b.updated_at, bs.state \
               FROM blob b \
               JOIN blob_state bs ON bs.blob_id = b.id \
               ORDER BY b.id, bs.created_at DESC \
             ) q",
        );

        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx: usize = 1;

        if filter.service_id.is_some() {
            conditions.push(format!("q.service_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.server_registration_id.is_some() {
            conditions.push(format!("q.server_registration_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.state.is_some() {
            conditions.push(format!("q.state = ${}", param_idx));
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        let mut query = sqlx::query(&sql);

        if let Some(ref service_id) = filter.service_id {
            query = query.bind(service_id.as_untyped_uuid());
        }
        if let Some(ref server_registration_id) = filter.server_registration_id {
            query = query.bind(server_registration_id.as_untyped_uuid());
        }
        if let Some(ref state) = filter.state {
            query = query.bind(state);
        }

        let rows = query.fetch_all(&self.pool).await?;

        let blobs = rows
            .iter()
            .map(|row| {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let server_registration_id_uuid: sqlx::types::Uuid =
                    row.try_get("server_registration_id")?;
                let state: BlobState = row.try_get("state")?;

                Ok(BlobModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    server_registration_id: TypedUuid::from_untyped_uuid(
                        server_registration_id_uuid,
                    ),
                    blob_time: row.try_get("blob_time")?,
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

        // Use a CTE to atomically check the current state, insert the new state
        // transition, and update the blob in a single query.
        let row = sqlx::query(
            r#"
            WITH current_state AS (
                SELECT state
                FROM blob_state
                WHERE blob_id = $1
                ORDER BY created_at DESC
                LIMIT 1
            ),
            insert_new_state AS (
                INSERT INTO blob_state (blob_id, state, created_at)
                SELECT $1, $2, $3
                FROM current_state
                WHERE current_state.state = $4
                RETURNING blob_id
            ),
            update_blob AS (
                UPDATE blob
                SET updated_at = $3
                WHERE id = $1
                AND EXISTS (SELECT 1 FROM insert_new_state)
                RETURNING id
            )
            SELECT
                EXISTS (SELECT 1 FROM current_state) AS has_current_state,
                EXISTS (SELECT 1 FROM insert_new_state) AS was_inserted,
                (SELECT state FROM current_state) AS actual_state
            "#,
        )
        .bind(id.as_untyped_uuid())
        .bind(to_state)
        .bind(now)
        .bind(from_state)
        .fetch_one(&self.pool)
        .await?;

        let has_current_state: bool = row.try_get("has_current_state")?;
        let was_inserted: bool = row.try_get("was_inserted")?;

        if !has_current_state {
            return Ok(None);
        }

        if !was_inserted {
            let actual: BlobState = row.try_get("actual_state")?;
            return Err(StorageError::InvalidBlobStateTransition {
                blob_id: id,
                expected: from_state,
                actual,
                to: to_state,
            });
        }

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
                let state: BlobState = row.try_get("state")?;
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
            SELECT DISTINCT ON (b.id) b.id, b.service_id, b.server_registration_id, b.blob_time, b.size, b.total_size,
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
                let server_registration_id_uuid: sqlx::types::Uuid =
                    row.try_get("server_registration_id")?;
                let state: BlobState = row.try_get("state")?;

                Ok(BlobModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    server_registration_id: TypedUuid::from_untyped_uuid(
                        server_registration_id_uuid,
                    ),
                    blob_time: row.try_get("blob_time")?,
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

    async fn list_blobs_paginated(
        &self,
        filter: &BlobFilter,
        page: &Paginated,
    ) -> StorageResult<Vec<BlobModel>> {
        let mut sql = String::from(
            "SELECT q.id, q.service_id, q.server_registration_id, q.blob_time, \
             q.size, q.total_size, q.created_at, q.updated_at, q.state \
             FROM ( \
               SELECT DISTINCT ON (b.id) b.id, b.service_id, b.server_registration_id, \
                      b.blob_time, b.size, b.total_size, b.created_at, b.updated_at, bs.state \
               FROM blob b \
               JOIN blob_state bs ON bs.blob_id = b.id \
               ORDER BY b.id, bs.created_at DESC \
             ) q",
        );

        let mut conditions: Vec<String> = Vec::new();
        let mut param_idx: usize = 1;

        if filter.service_id.is_some() {
            conditions.push(format!("q.service_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.server_registration_id.is_some() {
            conditions.push(format!("q.server_registration_id = ${}", param_idx));
            param_idx += 1;
        }
        if filter.state.is_some() {
            conditions.push(format!("q.state = ${}", param_idx));
            param_idx += 1;
        }
        if page.created_before.is_some() {
            conditions.push(format!("q.created_at < ${}", param_idx));
            param_idx += 1;
        }

        if !conditions.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(&conditions.join(" AND "));
        }

        sql.push_str(" ORDER BY q.created_at DESC");
        sql.push_str(&format!(" LIMIT ${}", param_idx));

        let mut query = sqlx::query(&sql);

        if let Some(ref service_id) = filter.service_id {
            query = query.bind(service_id.as_untyped_uuid());
        }
        if let Some(ref server_registration_id) = filter.server_registration_id {
            query = query.bind(server_registration_id.as_untyped_uuid());
        }
        if let Some(ref state) = filter.state {
            query = query.bind(state);
        }
        if let Some(ref ts) = page.created_before {
            query = query.bind(ts);
        }
        query = query.bind(page.limit as i64);

        let rows = query.fetch_all(&self.pool).await?;

        let blobs = rows
            .iter()
            .map(|row| {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let service_id_uuid: sqlx::types::Uuid = row.try_get("service_id")?;
                let server_registration_id_uuid: sqlx::types::Uuid =
                    row.try_get("server_registration_id")?;
                let state: BlobState = row.try_get("state")?;

                Ok(BlobModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    service_id: TypedUuid::from_untyped_uuid(service_id_uuid),
                    server_registration_id: TypedUuid::from_untyped_uuid(
                        server_registration_id_uuid,
                    ),
                    blob_time: row.try_get("blob_time")?,
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

    async fn list_health_checks_paginated(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
        page: &Paginated,
    ) -> StorageResult<Vec<HealthCheckModel>> {
        let sql = if page.created_before.is_some() {
            r#"
            SELECT id, server_registration_id, checked_in_at
            FROM health_check
            WHERE server_registration_id = $1 AND checked_in_at < $2
            ORDER BY checked_in_at DESC
            LIMIT $3
            "#
        } else {
            r#"
            SELECT id, server_registration_id, checked_in_at
            FROM health_check
            WHERE server_registration_id = $1
            ORDER BY checked_in_at DESC
            LIMIT $2
            "#
        };

        let mut query = sqlx::query(sql).bind(server_registration_id.as_untyped_uuid());

        if let Some(ref ts) = page.created_before {
            query = query.bind(ts);
        }
        query = query.bind(page.limit as i64);

        let rows = query.fetch_all(&self.pool).await?;

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
                    created_at: row.try_get("checked_in_at")?,
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
            if let sqlx::Error::Database(ref db_err) = e
                && db_err.constraint() == Some("idempotent_request_unique_key") {
                    return StorageError::IdempotentRequestAlreadyExists(
                        request.idempotency_key.clone(),
                    );
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
        .bind(&processing_state)
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
                let state: IdempotentRequestState = row.try_get("state")?;
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
        let processing_state = IdempotentRequestState::Processing;
        let complete_state = IdempotentRequestState::Complete;

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
        .bind(processing_state)
        .bind(complete_state)
        .fetch_one(&self.pool)
        .await?;

        let request_exists: bool = row.try_get("request_exists")?;
        let current_state: Option<IdempotentRequestState> = row.try_get("current_state")?;
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

#[async_trait]
impl TokenRequestStorage for PostgresStorage {
    async fn create_token_request(
        &self,
        request: &NewTokenRequestModel,
    ) -> StorageResult<TokenRequestModel> {
        let now = Utc::now();
        let pending_state = TokenRequestState::Pending;

        let mut tx = self.pool.begin().await?;

        // Insert the token request record
        let row = sqlx::query(
            r#"
            INSERT INTO token_request (server_registration_id, nonce, expires_at, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, server_registration_id, nonce, expires_at, created_at, updated_at
            "#,
        )
        .bind(request.server_registration_id.as_untyped_uuid())
        .bind(request.nonce.as_deref().unwrap_or_default())
        .bind(request.expires_at)
        .bind(now)
        .bind(now)
        .fetch_one(&mut *tx)
        .await?;

        let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
        let token_request_id = TypedUuid::from_untyped_uuid(id_uuid);

        // Insert initial Pending state into token_request_state table
        sqlx::query(
            r#"
            INSERT INTO token_request_state (token_request_id, state, created_at)
            VALUES ($1, $2, $3)
            "#,
        )
        .bind(token_request_id.as_untyped_uuid())
        .bind(pending_state)
        .bind(now)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        let server_registration_id_uuid: sqlx::types::Uuid =
            row.try_get("server_registration_id")?;
        Ok(TokenRequestModel {
            id: token_request_id,
            server_registration_id: TypedUuid::from_untyped_uuid(server_registration_id_uuid),
            nonce: row.try_get("nonce")?,
            expires_at: row.try_get("expires_at")?,
            state: pending_state,
            created_at: row.try_get("created_at")?,
            updated_at: row.try_get("updated_at")?,
        })
    }

    async fn get_token_request(
        &self,
        id: TypedUuid<TokenRequestId>,
    ) -> StorageResult<Option<TokenRequestModel>> {
        let row = sqlx::query(
            r#"
            SELECT tr.id, tr.server_registration_id, tr.nonce, tr.expires_at, tr.created_at, tr.updated_at, trs.state
            FROM token_request tr
            JOIN token_request_state trs ON trs.token_request_id = tr.id
            WHERE tr.id = $1
            ORDER BY trs.created_at DESC
            LIMIT 1
            "#,
        )
        .bind(id.as_untyped_uuid())
        .fetch_optional(&self.pool)
        .await?;

        match row {
            Some(row) => {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let server_registration_id_uuid: sqlx::types::Uuid =
                    row.try_get("server_registration_id")?;
                let state: TokenRequestState = row.try_get("state")?;

                Ok(Some(TokenRequestModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    server_registration_id: TypedUuid::from_untyped_uuid(
                        server_registration_id_uuid,
                    ),
                    nonce: row.try_get("nonce")?,
                    expires_at: row.try_get("expires_at")?,
                    state,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn list_token_requests_by_server_registration(
        &self,
        server_registration_id: TypedUuid<ServerRegistrationId>,
    ) -> StorageResult<Vec<TokenRequestModel>> {
        let rows = sqlx::query(
            r#"
            SELECT DISTINCT ON (tr.id) tr.id, tr.server_registration_id, tr.nonce, tr.expires_at, tr.created_at, tr.updated_at, trs.state
            FROM token_request tr
            JOIN token_request_state trs ON trs.token_request_id = tr.id
            WHERE tr.server_registration_id = $1
            ORDER BY tr.id, trs.created_at DESC
            "#,
        )
        .bind(server_registration_id.as_untyped_uuid())
        .fetch_all(&self.pool)
        .await?;

        let requests = rows
            .iter()
            .map(|row| {
                let id_uuid: sqlx::types::Uuid = row.try_get("id")?;
                let server_registration_id_uuid: sqlx::types::Uuid =
                    row.try_get("server_registration_id")?;
                let state: TokenRequestState = row.try_get("state")?;

                Ok(TokenRequestModel {
                    id: TypedUuid::from_untyped_uuid(id_uuid),
                    server_registration_id: TypedUuid::from_untyped_uuid(
                        server_registration_id_uuid,
                    ),
                    nonce: row.try_get("nonce")?,
                    expires_at: row.try_get("expires_at")?,
                    state,
                    created_at: row.try_get("created_at")?,
                    updated_at: row.try_get("updated_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(requests)
    }

    async fn update_token_request_state(
        &self,
        id: TypedUuid<TokenRequestId>,
        from_state: TokenRequestState,
        to_state: TokenRequestState,
    ) -> StorageResult<Option<()>> {
        let now = Utc::now();

        // Use a CTE to atomically check the current state, insert the new state
        // transition, and update the token request in a single query.
        //
        // Any update to a token request will mean blanking out the nonce. The only cases
        // where a value will be present are when the previous state is Pending and we are
        // transitioning to Consumed, or Terminated. In each of those cases we no longer
        // want to be storing the nonce. Once a request has been consumed, terminated, or
        // expired, the request is no longer active and we can also clear out the expiration
        // time.
        let row = sqlx::query(
            r#"
            WITH current_state AS (
                SELECT state
                FROM token_request_state
                WHERE token_request_id = $1
                ORDER BY created_at DESC
                LIMIT 1
            ),
            insert_new_state AS (
                INSERT INTO token_request_state (token_request_id, state, created_at)
                SELECT $1, $2, $3
                FROM current_state
                WHERE current_state.state = $4
                RETURNING token_request_id
            ),
            update_token_request AS (
                UPDATE token_request
                SET nonce = null, expires_at = null, updated_at = $3
                WHERE id = $1
                AND EXISTS (SELECT 1 FROM insert_new_state)
                RETURNING id
            )
            SELECT
                EXISTS (SELECT 1 FROM current_state) AS has_current_state,
                EXISTS (SELECT 1 FROM insert_new_state) AS was_inserted,
                (SELECT state FROM current_state) AS actual_state
            "#,
        )
        .bind(id.as_untyped_uuid())
        .bind(to_state)
        .bind(now)
        .bind(from_state)
        .fetch_one(&self.pool)
        .await?;

        let has_current_state: bool = row.try_get("has_current_state")?;
        let was_inserted: bool = row.try_get("was_inserted")?;

        if !has_current_state {
            return Ok(None);
        }

        if !was_inserted {
            let actual: TokenRequestState = row.try_get("actual_state")?;
            return Err(StorageError::InvalidTokenRequestStateTransition {
                token_request_id: id,
                expected: from_state,
                actual,
                to: to_state,
            });
        }

        Ok(Some(()))
    }

    async fn get_token_request_state_history(
        &self,
        id: TypedUuid<TokenRequestId>,
    ) -> StorageResult<Option<Vec<TokenRequestStateTransition>>> {
        let rows = sqlx::query(
            r#"
            SELECT state, created_at
            FROM token_request_state
            WHERE token_request_id = $1
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
                let state: TokenRequestState = row.try_get("state")?;
                Ok(TokenRequestStateTransition {
                    state,
                    created_at: row.try_get("created_at")?,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()?;

        Ok(Some(transitions))
    }

    async fn delete_token_request(
        &self,
        id: TypedUuid<TokenRequestId>,
    ) -> StorageResult<Option<()>> {
        let result = sqlx::query(
            r#"
            DELETE FROM token_request
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
}

impl Storage for PostgresStorage {}
