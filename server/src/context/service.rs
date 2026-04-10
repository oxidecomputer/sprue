use chrono::{DateTime, Utc};
use newtype_uuid::TypedUuid;
use std::sync::Arc;
use thiserror::Error;

use model::{
    HealthCheck, ServerRegistration, ServerRegistrationId, ServerRegistrationInstanceId,
    ServerRegistrationState, Service,
    db::{NewHealthCheckModel, NewServerRegistrationModel, NewServiceModel},
    storage::{HealthCheckStorage, ServerRegistrationStorage, ServiceStorage, StorageError},
};
use v_api::response::{OptionalResource, ResourceError, ResourceErrorInner, ResourceResult};

use crate::endpoints::service::ServiceIdentifier;

#[derive(Debug, Error)]
pub enum ServiceError {
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
}

impl ServiceContext {
    pub fn new(storage: Arc<dyn ServiceContextStorage>) -> Self {
        Self { storage }
    }

    async fn get_service(
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
        service_name: &str,
        instance: TypedUuid<ServerRegistrationInstanceId>,
    ) -> ResourceResult<ServerRegistration, ServiceError> {
        let existing = self
            .storage
            .get_server_registration_by_instance_id(instance)
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?;

        match existing {
            None => {
                let service = self.register_service(service_name).await?;
                Ok(self
                    .storage
                    .create_server_registration(&NewServerRegistrationModel {
                        service_id: service.id,
                        instance_id: instance,
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
        server: TypedUuid<ServerRegistrationId>,
    ) -> ResourceResult<(), ServiceError> {
        Ok(self
            .storage
            .update_server_registration_state(
                server,
                ServerRegistrationState::Pending,
                ServerRegistrationState::Accepted,
            )
            .await
            .optional()?)
    }

    pub async fn reject_server(
        &self,
        server: TypedUuid<ServerRegistrationId>,
    ) -> ResourceResult<(), ServiceError> {
        Ok(self
            .storage
            .update_server_registration_state(
                server,
                ServerRegistrationState::Pending,
                ServerRegistrationState::Rejected,
            )
            .await
            .optional()?)
    }

    pub async fn terminate_server(
        &self,
        server: TypedUuid<ServerRegistrationId>,
    ) -> ResourceResult<(), ServiceError> {
        Ok(self
            .storage
            .update_server_registration_state(
                server,
                ServerRegistrationState::Accepted,
                ServerRegistrationState::Terminated,
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

    // pub async fn create_service(
    //     &self,
    //     service: &NewServiceModel,
    // ) -> ResourceResult<Service, ServiceError> {
    //     Ok(self
    //         .storage
    //         .create_service(service)
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?
    //         .into())
    // }

    // pub async fn get_service(
    //     &self,
    //     name: &str,
    //     key: &str,
    // ) -> ResourceResult<Service, ServiceError> {
    //     let service: Service = self
    //         .storage
    //         .get_service(name)
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?
    //         .into();

    //     // Verify the key is registered for this service and is accepted
    //     let registration = self
    //         .storage
    //         .get_server_registration_by_key(key)
    //         .await
    //         .map_err(|_| ResourceError::DoesNotExist)?;

    //     // Verify the registration is for the correct service and is accepted
    //     if registration.service_id != service.id
    //         || registration.state != ServerRegistrationState::Accepted
    //     {
    //         return Err(ResourceError::DoesNotExist);
    //     }

    //     Ok(service)
    // }

    // pub async fn get_server_registration_by_key(
    //     &self,
    //     key: &str,
    // ) -> ResourceResult<ServerRegistration, ServiceError> {
    //     Ok(self
    //         .storage
    //         .get_server_registration_by_key(key)
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?
    //         .into())
    // }

    // pub async fn create_server_registration(
    //     &self,
    //     registration: &NewServerRegistrationModel,
    // ) -> ResourceResult<ServerRegistration, ServiceError> {
    //     Ok(self
    //         .storage
    //         .create_server_registration(registration)
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?
    //         .into())
    // }

    // pub async fn accept_server_registration(
    //     &self,
    //     registration: &ServerRegistration,
    // ) -> ResourceResult<(), ServiceError> {
    //     self.storage
    //         .update_server_registration_state(
    //             registration.id,
    //             ServerRegistrationState::Pending,
    //             ServerRegistrationState::Accepted,
    //         )
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?;
    //     Ok(())
    // }

    // pub async fn reject_server_registration(
    //     &self,
    //     registration: &ServerRegistration,
    // ) -> ResourceResult<(), ServiceError> {
    //     self.storage
    //         .update_server_registration_state(
    //             registration.id,
    //             ServerRegistrationState::Pending,
    //             ServerRegistrationState::Rejected,
    //         )
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?;
    //     Ok(())
    // }

    // pub async fn terminate_server_registration(
    //     &self,
    //     registration: &ServerRegistration,
    // ) -> ResourceResult<(), ServiceError> {
    //     self.storage
    //         .update_server_registration_state(
    //             registration.id,
    //             ServerRegistrationState::Accepted,
    //             ServerRegistrationState::Terminated,
    //         )
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?;
    //     Ok(())
    // }

    // pub async fn list_server_registrations_by_service(
    //     &self,
    //     service: &Service,
    // ) -> ResourceResult<Vec<ServerRegistration>, ServiceError> {
    //     let registrations = self
    //         .storage
    //         .list_server_registrations_by_service_id(service.id)
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?
    //         .into_iter()
    //         .map(Into::into)
    //         .collect();
    //     Ok(registrations)
    // }

    // pub async fn delete_server_registration(
    //     &self,
    //     instance_id: TypedUuid<ServerRegistrationInstanceId>,
    // ) -> ResourceResult<(), ServiceError> {
    //     self.storage
    //         .delete_server_registration_by_instance_id(instance_id)
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?;
    //     Ok(())
    // }

    // pub async fn log_health(
    //     &self,
    //     registration: &ServerRegistration,
    //     ip_address: String,
    //     checked_in_at: DateTime<Utc>,
    // ) -> ResourceResult<HealthCheck, ServiceError> {
    //     let record = self
    //         .storage
    //         .create_health_check(&NewHealthCheckModel {
    //             server_registration_id: registration.id,
    //             ip_address,
    //             checked_in_at,
    //         })
    //         .await
    //         .map_err(ResourceError::InternalError)
    //         .inner_err_into()?
    //         .into();
    //     Ok(record)
    // }
}
