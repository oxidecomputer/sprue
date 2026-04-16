use chrono::{DateTime, Utc};
use newtype_uuid::TypedUuid;
use std::{ops::Add, sync::Arc, time::Duration};
use thiserror::Error;
use v_model::permissions::Caller;

use model::{
    HealthCheck, InvalidStateTransition, ServerRegistration, ServerRegistrationId,
    ServerRegistrationInstanceId, ServerRegistrationState, Service, ServiceId,
    db::{NewHealthCheckModel, NewServerRegistrationModel, NewServiceModel},
    storage::{HealthCheckStorage, ServerRegistrationStorage, ServiceStorage, StorageError},
};
use v_api::response::{
    OptionalResource, ResourceError, ResourceErrorInner, ResourceResult, resource_restricted,
};

use crate::{context::ServerCaller, permissions::ApiPermissions};

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
        caller: &Caller<ApiPermissions>,
        service: TypedUuid<ServiceId>,
    ) -> ResourceResult<Service, ServiceError> {
        if caller.any(
            [
                ApiPermissions::GetService(service),
                ApiPermissions::GetServicesAll,
            ]
            .iter(),
        ) {
            Ok(self
                .storage
                .get_service_by_id(service)
                .await
                .optional()?
                .into())
        } else {
            resource_restricted()
        }
    }

    pub async fn create_service(
        &self,
        caller: &Caller<ApiPermissions>,
        name: &str,
    ) -> ResourceResult<Service, ServiceError> {
        if caller.can(&ApiPermissions::CreateService) {
            Ok(self
                .storage
                .create_service(&NewServiceModel {
                    name: name.to_string(),
                })
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?
                .into())
        } else {
            resource_restricted()
        }
    }

    pub async fn get_service_servers(
        &self,
        caller: &Caller<ApiPermissions>,
        service: TypedUuid<ServiceId>,
    ) -> ResourceResult<Vec<ServerRegistration>, ServiceError> {
        if caller.any(
            [
                ApiPermissions::GetService(service),
                ApiPermissions::GetServicesAll,
            ]
            .iter(),
        ) {
            let registrations = self
                .storage
                .list_server_registrations_by_service_id(service)
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?
                .into_iter()
                .map(Into::into)
                .collect();

            Ok(registrations)
        } else {
            resource_restricted()
        }
    }

    pub async fn get_server(
        &self,
        caller: &Caller<ApiPermissions>,
        server: TypedUuid<ServerRegistrationId>,
    ) -> ResourceResult<ServerRegistration, ServiceError> {
        let server: ServerRegistration = self
            .storage
            .get_server_registration(server)
            .await
            .optional()?
            .into();

        if caller.any(
            [
                ApiPermissions::GetService(server.service_id),
                ApiPermissions::GetServicesAll,
            ]
            .iter(),
        ) {
            Ok(server)
        } else {
            resource_restricted()
        }
    }

    pub async fn register_server(
        &self,
        caller: &Caller<ApiPermissions>,
        service: TypedUuid<ServiceId>,
        instance: TypedUuid<ServerRegistrationInstanceId>,
        nonce: String,
    ) -> ResourceResult<ServerRegistration, ServiceError> {
        let service = self.get_service(caller, service).await?;
        if caller.any(
            [
                ApiPermissions::ManageService(service.id),
                ApiPermissions::ManageServersAll,
            ]
            .iter(),
        ) {
            let existing = self
                .storage
                .get_server_registration_by_instance_id(instance)
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?;

            match existing {
                None => Ok(self
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
                    .into()),
                Some(model) => Ok(model.into()),
            }
        } else {
            resource_restricted()
        }
    }

    pub async fn accept_server(
        &self,
        caller: &Caller<ApiPermissions>,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        if caller.any(
            [
                ApiPermissions::ManageService(server.service_id),
                ApiPermissions::ManageServersAll,
            ]
            .iter(),
        ) {
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
        } else {
            resource_restricted()
        }
    }

    pub async fn prove_server(
        &self,
        caller: &Caller<ApiPermissions>,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        if caller.any(
            [
                ApiPermissions::ManageService(server.service_id),
                ApiPermissions::ManageServersAll,
            ]
            .iter(),
        ) {
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
        } else {
            resource_restricted()
        }
    }

    pub async fn reject_server(
        &self,
        caller: &Caller<ApiPermissions>,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        if caller.any(
            [
                ApiPermissions::ManageService(server.service_id),
                ApiPermissions::ManageServersAll,
            ]
            .iter(),
        ) {
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
        } else {
            resource_restricted()
        }
    }

    pub async fn terminate_server(
        &self,
        caller: &Caller<ApiPermissions>,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        if caller.any(
            [
                ApiPermissions::ManageService(server.service_id),
                ApiPermissions::ManageServersAll,
            ]
            .iter(),
        ) {
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
        } else {
            resource_restricted()
        }
    }

    pub async fn checkin(
        &self,
        caller: &ServerCaller,
        server: TypedUuid<ServerRegistrationId>,
        checked_in_at: DateTime<Utc>,
    ) -> ResourceResult<HealthCheck, ServiceError> {
        if caller.id == server {
            let record = self
                .storage
                .create_health_check(&NewHealthCheckModel {
                    server_registration_id: server,
                    checked_in_at,
                })
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?
                .into();
            Ok(record)
        } else {
            resource_restricted()
        }
    }
}
