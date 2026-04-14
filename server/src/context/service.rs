use chrono::{DateTime, Utc};
use newtype_uuid::TypedUuid;
use std::{ops::Add, sync::Arc, time::Duration};
use thiserror::Error;

use model::{
    HealthCheck, InvalidStateTransition, ServerRegistration, ServerRegistrationId,
    ServerRegistrationInstanceId, ServerRegistrationState, Service,
    db::{NewHealthCheckModel, NewServerRegistrationModel, NewServiceModel},
    storage::{HealthCheckStorage, ServerRegistrationStorage, ServiceStorage, StorageError},
};
use v_api::response::{OptionalResource, ResourceError, ResourceErrorInner, ResourceResult};

use crate::endpoints::service::ServiceIdentifier;

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Invalid state transition for registration")]
    RegistrationState(#[from] InvalidStateTransition),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

pub trait ServiceContextStorage:
    ServiceStorage + ServerRegistrationStorage + HealthCheckStorage
{
}
impl<T: ServiceStorage + ServerRegistrationStorage + HealthCheckStorage> ServiceContextStorage
    for T
{
}

#[derive(Clone)]
pub struct ServiceContext {
    storage: Arc<dyn ServiceContextStorage>,
    max_server_registration_age: Duration,
}

impl ServiceContext {
    pub fn new(
        storage: Arc<dyn ServiceContextStorage>,
        max_server_registration_age: Duration,
    ) -> Self {
        Self {
            storage,
            max_server_registration_age,
        }
    }

    pub async fn get_service(
        &self,
        service: &ServiceIdentifier,
    ) -> ResourceResult<Service, ServiceError> {
        Ok(match service {
            ServiceIdentifier::Id(id) => self.storage.get_service_by_id(*id).await,
            ServiceIdentifier::Name(name) => self.storage.get_service_by_name(name).await,
        }
        .optional()?
        .into())
    }

    pub async fn create_service(&self, name: &str) -> ResourceResult<Service, ServiceError> {
        Ok(self
            .storage
            .create_service(&NewServiceModel {
                name: name.to_string(),
            })
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?
            .into())
    }

    pub async fn get_service_servers(
        &self,
        service: &ServiceIdentifier,
    ) -> ResourceResult<Vec<ServerRegistration>, ServiceError> {
        let service: Service = match service {
            ServiceIdentifier::Id(id) => self.storage.get_service_by_id(*id).await,
            ServiceIdentifier::Name(name) => self.storage.get_service_by_name(name).await,
        }
        .optional()?
        .into();

        let registrations = self
            .storage
            .list_server_registrations_by_service_id(service.id)
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?
            .into_iter()
            .map(Into::into)
            .collect();
        Ok(registrations)
    }

    pub async fn get_server(
        &self,
        server: TypedUuid<ServerRegistrationId>,
    ) -> ResourceResult<ServerRegistration, ServiceError> {
        Ok(self
            .storage
            .get_server_registration(server)
            .await
            .optional()?
            .into())
    }

    pub async fn register_service(
        &self,
        service_name: &str,
    ) -> ResourceResult<Service, ServiceError> {
        let service = self
            .get_service(&ServiceIdentifier::Name(service_name.to_string()))
            .await;

        match service {
            Err(ResourceError::DoesNotExist) => Ok(self
                .storage
                .create_service(&NewServiceModel {
                    name: service_name.to_string(),
                })
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?
                .into()),
            other => other,
        }
    }

    pub async fn register_server(
        &self,
        service: &ServiceIdentifier,
        instance: TypedUuid<ServerRegistrationInstanceId>,
        nonce: String,
    ) -> ResourceResult<ServerRegistration, ServiceError> {
        let existing = self
            .storage
            .get_server_registration_by_instance_id(instance)
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?;

        match existing {
            None => {
                let service = self.get_service(service).await?;
                Ok(self
                    .storage
                    .create_server_registration(&NewServerRegistrationModel {
                        service_id: service.id,
                        instance_id: instance,
                        nonce: Some(nonce),
                        expires_at: Some(Utc::now().add(self.max_server_registration_age)),
                    })
                    .await
                    .map_err(ResourceError::InternalError)
                    .inner_err_into()?
                    .into())
            }
            Some(model) => Ok(model.into()),
        }
    }

    pub async fn accept_server(
        &self,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        Ok(self
            .storage
            .update_server_registration_state(
                server.id,
                ServerRegistrationState::Pending,
                server
                    .state
                    .accept()
                    .map_err(ResourceError::InternalError)
                    .inner_err_into()?,
            )
            .await
            .optional()?)
    }

    pub async fn prove_server(
        &self,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        Ok(self
            .storage
            .update_server_registration_state(
                server.id,
                ServerRegistrationState::Pending,
                server
                    .state
                    .prove()
                    .map_err(ResourceError::InternalError)
                    .inner_err_into()?,
            )
            .await
            .optional()?)
    }

    pub async fn reject_server(
        &self,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        Ok(self
            .storage
            .update_server_registration_state(
                server.id,
                ServerRegistrationState::Pending,
                server
                    .state
                    .reject()
                    .map_err(ResourceError::InternalError)
                    .inner_err_into()?,
            )
            .await
            .optional()?)
    }

    pub async fn terminate_server(
        &self,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        Ok(self
            .storage
            .update_server_registration_state(
                server.id,
                ServerRegistrationState::Accepted,
                server
                    .state
                    .terminate()
                    .map_err(ResourceError::InternalError)
                    .inner_err_into()?,
            )
            .await
            .optional()?)
    }

    pub async fn checkin(
        &self,
        server: TypedUuid<ServerRegistrationId>,
        checked_in_at: DateTime<Utc>,
    ) -> ResourceResult<HealthCheck, ServiceError> {
        let registration = self.get_server(server).await?;
        let record = self
            .storage
            .create_health_check(&NewHealthCheckModel {
                server_registration_id: registration.id,
                checked_in_at,
            })
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?
            .into();
        Ok(record)
    }
}
