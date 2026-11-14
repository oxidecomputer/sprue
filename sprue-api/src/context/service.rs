// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use chrono::{DateTime, Utc};
use newtype_uuid::TypedUuid;
use std::{ops::Add, sync::Arc, time::Duration};
use thiserror::Error;
use v_model::permissions::Caller;

use sprue_model::{
    Deployment, DeploymentId, HealthCheck, InvalidStateTransition, ProjectId,
    RegistrationChallenge, RegistrationChallengeId, ServerRegistration, ServerRegistrationId,
    Service, ServiceId, ServiceIdentifier, SiloId,
    db::{
        NewDeploymentModel, NewHealthCheckModel, NewRegistrationChallengeModel,
        NewServerRegistrationModel, NewServiceModel,
    },
    storage::{
        DeploymentStorage, HealthCheckStorage, Paginated, RegistrationChallengeStorage,
        ServerRegistrationStorage, ServiceStorage, StorageError,
    },
};
use v_api::response::{
    OptionalResource, ResourceError, ResourceErrorInner, ResourceResult, resource_restricted,
};

use crate::{
    context::{ServerCaller, server_identity::VerifiedInstance},
    permissions::ApiPermissions,
};

#[derive(Debug, Error)]
pub enum ServiceError {
    #[error("Invalid state transition for registration")]
    RegistrationState(#[from] InvalidStateTransition),
    #[error(transparent)]
    Storage(#[from] StorageError),
}

pub trait ServiceContextStorage:
    ServiceStorage
    + DeploymentStorage
    + RegistrationChallengeStorage
    + ServerRegistrationStorage
    + HealthCheckStorage
{
}
impl<
    T: ServiceStorage
        + DeploymentStorage
        + RegistrationChallengeStorage
        + ServerRegistrationStorage
        + HealthCheckStorage,
> ServiceContextStorage for T
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

    pub async fn resolve_service(
        &self,
        caller: &Caller<ApiPermissions>,
        identifier: &ServiceIdentifier,
    ) -> ResourceResult<Service, ServiceError> {
        match identifier {
            ServiceIdentifier::Id(id) => self.get_service(caller, *id).await,
            ServiceIdentifier::Name(name) => {
                let service: Service = self
                    .storage
                    .get_service_by_name(name)
                    .await
                    .optional()?
                    .into();
                if caller.can(&ApiPermissions::GetService(service.id)) {
                    Ok(service)
                } else {
                    resource_restricted()
                }
            }
        }
    }

    pub async fn get_service(
        &self,
        caller: &Caller<ApiPermissions>,
        service: TypedUuid<ServiceId>,
    ) -> ResourceResult<Service, ServiceError> {
        if caller.can(&ApiPermissions::GetService(service)) {
            Ok(self.storage.get_service(service).await.optional()?.into())
        } else {
            resource_restricted()
        }
    }

    pub async fn list_services(
        &self,
        caller: &Caller<ApiPermissions>,
    ) -> ResourceResult<Vec<Service>, ServiceError> {
        let services = self
            .storage
            .list_services()
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?;
        Ok(services
            .into_iter()
            .filter_map(|s| {
                if caller.can(&ApiPermissions::GetService(s.id)) {
                    Some(s.into())
                } else {
                    None
                }
            })
            .collect())
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

    pub async fn delete_service(
        &self,
        caller: &Caller<ApiPermissions>,
        service: TypedUuid<ServiceId>,
    ) -> ResourceResult<(), ServiceError> {
        if caller.can(&ApiPermissions::ManageService(service)) {
            Ok(self.storage.delete_service(service).await.optional()?)
        } else {
            resource_restricted()
        }
    }

    pub async fn get_service_servers(
        &self,
        caller: &Caller<ApiPermissions>,
        service: TypedUuid<ServiceId>,
    ) -> ResourceResult<Vec<ServerRegistration>, ServiceError> {
        if caller.can(&ApiPermissions::GetService(service)) {
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

        if caller.can(&ApiPermissions::GetService(server.service_id)) {
            Ok(server)
        } else {
            resource_restricted()
        }
    }

    /// Issue a challenge to a VM that wants to join the given service.
    pub async fn create_registration_challenge(
        &self,
        caller: &Caller<ApiPermissions>,
        service: TypedUuid<ServiceId>,
        nonce: String,
    ) -> ResourceResult<RegistrationChallenge, ServiceError> {
        if caller.can(&ApiPermissions::ManageService(service)) {
            Ok(self
                .storage
                .create_registration_challenge(&NewRegistrationChallengeModel {
                    service_id: service,
                    nonce,
                    expires_at: Utc::now().add(self.max_server_registration_age),
                })
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?
                .into())
        } else {
            resource_restricted()
        }
    }

    /// Get a registration challenge that can still be answered.
    pub async fn get_registration_challenge(
        &self,
        caller: &Caller<ApiPermissions>,
        id: TypedUuid<RegistrationChallengeId>,
    ) -> ResourceResult<RegistrationChallenge, ServiceError> {
        let challenge: RegistrationChallenge = self
            .storage
            .get_active_registration_challenge(id)
            .await
            .optional()?
            .into();
        if caller.can(&ApiPermissions::ManageService(challenge.service_id)) {
            Ok(challenge)
        } else {
            resource_restricted()
        }
    }

    /// Answer a registration challenge with an instance identity that has been
    /// proven by attestation. The challenge is consumed so that it can not be
    /// replayed. An instance that is already registered gets its existing
    /// registration back, but only when that registration agrees with the
    /// attested identity and the challenged service. Otherwise a new
    /// registration is created.
    pub async fn complete_registration_challenge(
        &self,
        caller: &Caller<ApiPermissions>,
        challenge: &RegistrationChallenge,
        instance: VerifiedInstance,
    ) -> ResourceResult<ServerRegistration, ServiceError> {
        if caller.can(&ApiPermissions::ManageService(challenge.service_id)) {
            self.storage
                .consume_registration_challenge(challenge.id)
                .await
                .optional()?;

            let existing = self
                .storage
                .get_server_registration_by_instance_id(instance.instance_id())
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?;

            match existing {
                None => Ok(self
                    .storage
                    .create_server_registration(&NewServerRegistrationModel {
                        service_id: challenge.service_id,
                        instance_id: instance.instance_id(),
                        project_id: instance.project_id(),
                        silo_id: instance.silo_id(),
                    })
                    .await
                    .map_err(ResourceError::InternalError)
                    .inner_err_into()?
                    .into()),
                Some(model)
                    if model.service_id == challenge.service_id
                        && model.project_id == instance.project_id()
                        && model.silo_id == instance.silo_id() =>
                {
                    Ok(model.into())
                }
                Some(model) => {
                    tracing::warn!(
                        registration = ?model.id,
                        instance = ?instance.instance_id(),
                        "Existing registration does not match the attested identity or the challenged service"
                    );
                    Err(ResourceError::Conflict)
                }
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
        if caller.can(&ApiPermissions::ManageService(server.service_id)) {
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

    pub async fn reject_server(
        &self,
        caller: &Caller<ApiPermissions>,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        if caller.can(&ApiPermissions::ManageService(server.service_id)) {
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

    pub async fn delete_server(
        &self,
        caller: &Caller<ApiPermissions>,
        server: &ServerRegistration,
    ) -> ResourceResult<(), ServiceError> {
        if caller.can(&ApiPermissions::ManageService(server.service_id)) {
            Ok(self
                .storage
                .delete_server_registration(server.id)
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
        if caller.can(&ApiPermissions::ManageService(server.service_id)) {
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

    pub async fn list_health_checks(
        &self,
        caller: &Caller<ApiPermissions>,
        server: &ServerRegistration,
    ) -> ResourceResult<Vec<HealthCheck>, ServiceError> {
        if caller.any(
            [
                ApiPermissions::GetService(server.service_id),
                ApiPermissions::GetServicesAll,
            ]
            .iter(),
        ) {
            let checks = self
                .storage
                .list_health_checks_by_server_registration(server.id)
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?;
            let checks = checks.into_iter().map(Into::into).collect();
            Ok(checks)
        } else {
            resource_restricted()
        }
    }

    pub async fn list_health_checks_paginated(
        &self,
        caller: &Caller<ApiPermissions>,
        server: &ServerRegistration,
        page: &Paginated,
    ) -> ResourceResult<Vec<HealthCheck>, ServiceError> {
        if caller.any(
            [
                ApiPermissions::GetService(server.service_id),
                ApiPermissions::GetServicesAll,
            ]
            .iter(),
        ) {
            let checks = self
                .storage
                .list_health_checks_paginated(server.id, page)
                .await
                .map_err(ResourceError::InternalError)
                .inner_err_into()?
                .into_iter()
                .map(Into::into)
                .collect();
            Ok(checks)
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
        if caller.can(&ApiPermissions::ManageService(service_id)) {
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

        if caller.can(&ApiPermissions::GetService(deployment.service_id)) {
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
        if caller.can(&ApiPermissions::GetService(service_id)) {
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
        if caller.can(&ApiPermissions::ManageService(deployment.service_id)) {
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
