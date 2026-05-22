use cedar_policy::{Entity, RestrictedExpression};
use newtype_uuid::TypedUuid;
use sprue_model::{Deployment, ProjectId, Service, ServiceId, SiloId};
use std::collections::{HashMap, HashSet};

use crate::policy::{PolicyError, project_entity_uid, service_entity_uid, silo_entity_uid};

pub struct ServiceResource {
    id: TypedUuid<ServiceId>,
    deployments: Vec<(TypedUuid<ProjectId>, TypedUuid<SiloId>)>,
}
impl ServiceResource {
    pub fn new<'a>(
        service: &Service,
        deployments: impl IntoIterator<Item = &'a Deployment>,
    ) -> Self {
        Self {
            id: service.id,
            deployments: deployments
                .into_iter()
                .filter_map(|d| {
                    if service.id == d.service_id {
                        Some((d.project_id, d.silo_id))
                    } else {
                        None
                    }
                })
                .collect(),
        }
    }
}
impl TryFrom<ServiceResource> for Entity {
    type Error = PolicyError;
    fn try_from(resource: ServiceResource) -> Result<Self, Self::Error> {
        let deployment_records = resource
            .deployments
            .into_iter()
            .map(|(project_id, silo_id)| {
                Ok::<RestrictedExpression, Self::Error>(RestrictedExpression::new_record(
                    HashMap::from([
                        (
                            "project".to_string(),
                            RestrictedExpression::new_entity_uid(project_entity_uid(project_id)),
                        ),
                        (
                            "silo".to_string(),
                            RestrictedExpression::new_entity_uid(silo_entity_uid(silo_id)),
                        ),
                    ]),
                )?)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Entity::new(
            service_entity_uid(resource.id),
            HashMap::from([(
                "deployments".to_string(),
                RestrictedExpression::new_set(deployment_records),
            )]),
            HashSet::default(),
        )?)
    }
}
