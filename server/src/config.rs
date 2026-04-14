use std::path::PathBuf;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use tap::TapFallible;
use v_api::config::{ServerLogFormat, SpecConfig};
use v_api_param::StringParam;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub log_format: ServerLogFormat,
    pub log_directory: Option<PathBuf>,
    pub spec: Option<SpecConfig>,
    pub database_url: StringParam,
    pub public_url: String,
    pub vm_identity: VmIdentityConfig,
    pub oidc: OidcConfig,
}

#[derive(Debug, Deserialize)]
pub struct VmIdentityConfig {
    pub common_name: String,
    pub root_cert_chain: String,
    pub measurements: Vec<PathBuf>,
    pub max_registration_duration: u64,
}

#[derive(Debug, Deserialize)]
pub struct OidcConfig {
    pub kid: String,
    pub public: String,
    pub private: String,
    pub token: OidcTokenConfig,
}

#[derive(Debug, Deserialize)]
pub struct OidcTokenConfig {
    pub issuer: String,
    pub audience: String,
    pub token_lifetime: u32,
    pub max_token_request_duration: u64,
}

impl ServerConfig {
    pub fn new(config_sources: Option<Vec<String>>) -> Result<Self, ConfigError> {
        let mut config = Config::builder()
            .add_source(File::with_name("settings.toml").required(false))
            .add_source(File::with_name("server/settings.toml").required(false));

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
