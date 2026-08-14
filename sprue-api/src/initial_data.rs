// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use config::{Config, ConfigError, Environment, File};
use newtype_uuid::TypedUuid;
use serde::Deserialize;
use thiserror::Error;
use tracing::Instrument;
use v_api::{VContext, response::ResourceError};
use v_model::{
    NewAccessGroup, Permissions,
    storage::{AccessGroupFilter, StoreError},
};

use crate::permissions::ApiPermissions;

#[derive(Debug, Deserialize)]
pub struct InitialData {
    pub groups: Vec<InitialGroup>,
}

#[derive(Debug, Deserialize)]
pub struct InitialGroup {
    pub name: String,
    pub permissions: Permissions<ApiPermissions>,
}

#[derive(Debug, Error)]
pub enum InitError {
    #[error("Failed to parse configuration file for initial data: {0}")]
    Config(#[from] ConfigError),
    #[error("Resource operation failed")]
    Resource(#[from] ResourceError<StoreError>),
    #[error("Failed to serialize rule for storage: {0}")]
    Rule(#[from] serde_json::Error),
    #[error("Failed to store initial rule: {0}")]
    Storage(#[from] StoreError),
}

impl InitialData {
    pub fn new(config_sources: Option<Vec<String>>) -> Result<Self, InitError> {
        let mut config = Config::builder()
            .add_source(File::with_name("mappers.toml").required(false))
            .add_source(File::with_name("sprue-api/mappers.toml").required(false));

        for source in config_sources.unwrap_or_default() {
            config = config.add_source(File::with_name(&source).required(false));
        }

        Ok(config
            .add_source(Environment::default())
            .build()?
            .try_deserialize()?)
    }

    pub async fn initialize(self, ctx: &VContext<ApiPermissions>) -> Result<(), InitError> {
        let existing_groups = ctx
            .group
            .list_groups(
                &ctx.builtin_registration_user(),
                AccessGroupFilter::default(),
            )
            .await?;

        for group in self.groups {
            let span = tracing::info_span!("Initializing group", group = ?group);

            async {
                let id = existing_groups
                    .iter()
                    .find(|g| g.name == group.name)
                    .map(|g| g.id)
                    .unwrap_or_else(TypedUuid::new_v4);

                ctx.group
                    .create_group(
                        &ctx.builtin_registration_user(),
                        NewAccessGroup {
                            id,
                            name: group.name,
                            permissions: group.permissions,
                        },
                    )
                    .await
                    .map(|_| ())
                    .or_else(handle_unique_violation_error)
            }
            .instrument(span)
            .await?
        }

        Ok(())
    }
}

fn handle_unique_violation_error(
    err: ResourceError<StoreError>,
) -> Result<(), ResourceError<StoreError>> {
    match err {
        ResourceError::Conflict => {
            tracing::info!("Record already exists. Skipping.");
            Ok(())
        }
        err => {
            tracing::error!(?err, "Failed to store record");
            Err(err)
        }
    }
}
