// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use cedar_policy::{
    EntityAttrEvaluationError, EntityId, EntityTypeName, EntityUid, ExpressionConstructionError,
};
use newtype_uuid::TypedUuid;
use sprue_model::{ProjectId, ServerRegistrationInstanceId, ServiceId, SiloId};
use std::{str::FromStr, sync::LazyLock};
use thiserror::Error;

pub mod action;
pub mod principal;
pub mod resource;

#[derive(Debug, Error)]
pub enum PolicyError {
    #[error("Failed to construct entity")]
    EntityConstruction(#[from] EntityAttrEvaluationError),
    #[error("Failed to construct expression")]
    ExpressionConstruction(#[from] ExpressionConstructionError),
}

macro_rules! cedar_entity {
    ($static_name:ident, $type_name:literal, $fn_name:ident, $id_type:ty) => {
        static $static_name: LazyLock<EntityTypeName> = LazyLock::new(|| {
            EntityTypeName::from_str($type_name)
                .expect(concat!("invalid entity type name: ", $type_name))
        });

        pub fn $fn_name(id: $id_type) -> EntityUid {
            EntityUid::from_type_name_and_id(
                $static_name.clone(),
                EntityId::new(id.to_string().as_str()),
            )
        }
    };
}

cedar_entity!(
    SERVICE_ENTITY_TYPE,
    "Sprue::Service",
    service_entity_uid,
    TypedUuid<ServiceId>
);
cedar_entity!(
    INSTANCE_ENTITY_TYPE,
    "Sprue::Instance",
    instance_entity_uid,
    TypedUuid<ServerRegistrationInstanceId>
);
cedar_entity!(
    PROJECT_ENTITY_TYPE,
    "Sprue::Project",
    project_entity_uid,
    TypedUuid<ProjectId>
);
cedar_entity!(
    SILO_ENTITY_TYPE,
    "Sprue::Silo",
    silo_entity_uid,
    TypedUuid<SiloId>
);
