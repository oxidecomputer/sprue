// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use std::path::PathBuf;
use tap::TapFallible;
use v_api::config::{AsymmetricKey, AuthnProviders, ServerLogFormat, SpecConfig};
use v_api_param::StringParam;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub log_format: ServerLogFormat,
    pub log_directory: Option<PathBuf>,
    pub port: Option<u16>,
    pub param_base_path: Option<PathBuf>,
    pub initial_mappers: Option<String>,
    pub spec: Option<SpecConfig>,
    pub public_url: String,
    pub database_url: StringParam,
    pub jwt: JwtConfig,
    pub authn: AuthnProviders,
    pub vm_identity: VmIdentityConfig,
    pub oidc: OidcConfig,
    pub auto_registration_policy: Option<ServerAutoRegistration>,
    pub backup: BackupConfig,
}

#[derive(Debug, Deserialize)]
pub struct JwtConfig {
    pub default_expiration: i64,
    pub keys: Vec<AsymmetricKey>,
}

#[derive(Debug, Deserialize)]
pub struct VmIdentityConfig {
    pub organization: String,
    pub root_cert_chain: String,
    pub measurements: Vec<PathBuf>,
    pub registration_duration: u64,
}

#[derive(Debug, Deserialize)]
pub struct OidcConfig {
    pub token: OidcTokenConfig,
}

#[derive(Debug, Deserialize)]
pub struct OidcTokenConfig {
    pub token_lifetime: u32,
    pub token_request_duration: u64,
}

#[derive(Debug, Deserialize)]
pub struct ServerAutoRegistration {
    pub policy: StringParam,
    pub schema: StringParam,
}

#[derive(Debug, Deserialize)]
pub struct BackupConfig {
    pub local_root: PathBuf,
    pub remote: BackupStorageConfig,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum BackupStorageConfig {
    Local {
        root: PathBuf,
    },
    S3 {
        iam_region: String,
        role: String,
        bucket: String,
    },
}

impl ServerConfig {
    pub fn new(config_sources: Option<Vec<String>>) -> Result<Self, ConfigError> {
        let mut config = Config::builder()
            .add_source(File::with_name("settings.toml").required(false))
            .add_source(File::with_name("sprue-api/settings.toml").required(false));

        for source in config_sources.unwrap_or_default() {
            config = config.add_source(File::with_name(&source).required(false));
        }

        config
            .add_source(Environment::default())
            .build()?
            .try_deserialize()
            .tap_err(|err| println!("Failed to deserialize settings file: {}", err))
    }
}
