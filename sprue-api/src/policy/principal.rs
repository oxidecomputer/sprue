// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use cedar_policy::{Entity, RestrictedExpression};
use newtype_uuid::TypedUuid;
use sprue_model::{ProjectId, ServerRegistration, ServerRegistrationInstanceId, SiloId};
use std::collections::{HashMap, HashSet};

use crate::policy::{PolicyError, instance_entity_uid, project_entity_uid, silo_entity_uid};

pub struct ServerPrincipal {
    instance_id: TypedUuid<ServerRegistrationInstanceId>,
    project: TypedUuid<ProjectId>,
    silo: TypedUuid<SiloId>,
}
impl From<&ServerRegistration> for ServerPrincipal {
    fn from(server: &ServerRegistration) -> Self {
        Self {
            instance_id: server.instance_id,
            project: server.project_id,
            silo: server.silo_id,
        }
    }
}
impl TryFrom<ServerPrincipal> for Entity {
    type Error = PolicyError;

    fn try_from(principal: ServerPrincipal) -> Result<Self, Self::Error> {
        Ok(Entity::new(
            instance_entity_uid(principal.instance_id),
            HashMap::from([
                (
                    "project".to_string(),
                    RestrictedExpression::new_entity_uid(project_entity_uid(principal.project)),
                ),
                (
                    "silo".to_string(),
                    RestrictedExpression::new_entity_uid(silo_entity_uid(principal.silo)),
                ),
            ]),
            HashSet::default(),
        )?)
    }
}
