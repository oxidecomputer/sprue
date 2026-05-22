// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use chrono::{DateTime, Utc};
use newtype_uuid::TypedUuid;
use std::{ops::Add, sync::Arc, time::Duration};
use thiserror::Error;
use v_model::permissions::Caller;

use sprue_model::{
    Deployment, DeploymentId, HealthCheck, InvalidStateTransition, ProjectId, ServerRegistration,
    ServerRegistrationId, ServerRegistrationInstanceId, Service, ServiceId, SiloId,
    db::{NewDeploymentModel, NewHealthCheckModel, NewServerRegistrationModel, NewServiceModel},
    storage::{
        DeploymentStorage, HealthCheckStorage, ServerRegistrationStorage, ServiceStorage,
        StorageError,
    },
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
    ServiceStorage + DeploymentStorage + ServerRegistrationStorage + HealthCheckStorage
{
}
impl<T: ServiceStorage + DeploymentStorage + ServerRegistrationStorage + HealthCheckStorage>
    ServiceContextStorage for T
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
        project_id: TypedUuid<ProjectId>,
        silo_id: TypedUuid<SiloId>,
        nonce: String,
    ) -> ResourceResult<ServerRegistration, ServiceError> {
        let service = self.get_service(caller, service).await?;
        if caller.any(
            [
                ApiPermissions::ManageService(service.id),
                ApiPermissions::ManageServicesAll,
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
                        project_id,
                        silo_id,
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
                ApiPermissions::ManageServicesAll,
            ]
            .iter(),
        ) {
            Ok(self
                .storage
                .update_server_registration_state(
                    server.id,
                    server.state,
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
                ApiPermissions::ManageServicesAll,
            ]
            .iter(),
        ) {
            Ok(self
                .storage
                .update_server_registration_state(
                    server.id,
                    server.state,
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
                ApiPermissions::ManageServicesAll,
            ]
            .iter(),
        ) {
            Ok(self
                .storage
                .update_server_registration_state(
                    server.id,
                    server.state,
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
                ApiPermissions::ManageServicesAll,
            ]
            .iter(),
        ) {
            Ok(self
                .storage
                .update_server_registration_state(
                    server.id,
                    server.state,
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

    pub async fn create_deployment(
        &self,
        caller: &Caller<ApiPermissions>,
        service_id: TypedUuid<ServiceId>,
        project_id: TypedUuid<ProjectId>,
        silo_id: TypedUuid<SiloId>,
    ) -> ResourceResult<Deployment, ServiceError> {
        if caller.any(
            [
                ApiPermissions::ManageService(service_id),
                ApiPermissions::ManageServicesAll,
            ]
            .iter(),
        ) {
            Ok(self
                .storage
                .create_deployment(&NewDeploymentModel {
                    service_id,
                    project_id,
                    silo_id,
                })
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?
                .into())
        } else {
            resource_restricted()
        }
    }

    pub async fn get_deployment(
        &self,
        caller: &Caller<ApiPermissions>,
        deployment_id: TypedUuid<DeploymentId>,
    ) -> ResourceResult<Deployment, ServiceError> {
        let deployment: Deployment = self
            .storage
            .get_deployment(deployment_id)
            .await
            .optional()?
            .into();

        if caller.any(
            [
                ApiPermissions::GetService(deployment.service_id),
                ApiPermissions::GetServicesAll,
            ]
            .iter(),
        ) {
            Ok(deployment)
        } else {
            resource_restricted()
        }
    }

    pub async fn list_deployments(
        &self,
        caller: &Caller<ApiPermissions>,
        service_id: TypedUuid<ServiceId>,
    ) -> ResourceResult<Vec<Deployment>, ServiceError> {
        if caller.any(
            [
                ApiPermissions::GetService(service_id),
                ApiPermissions::GetServicesAll,
            ]
            .iter(),
        ) {
            let deployments = self
                .storage
                .list_deployments_by_service_id(service_id)
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?
                .into_iter()
                .map(Into::into)
                .collect();

            Ok(deployments)
        } else {
            resource_restricted()
        }
    }

    pub async fn delete_deployment(
        &self,
        caller: &Caller<ApiPermissions>,
        deployment: &Deployment,
    ) -> ResourceResult<(), ServiceError> {
        if caller.any(
            [
                ApiPermissions::ManageService(deployment.service_id),
                ApiPermissions::ManageServicesAll,
            ]
            .iter(),
        ) {
            Ok(self
                .storage
                .delete_deployment(deployment.id)
                .await
                .optional()?)
        } else {
            resource_restricted()
        }
    }
}
