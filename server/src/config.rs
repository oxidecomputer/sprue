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

    // VM Attestation
    pub root_cert_chain: String,
    pub measurements: Vec<PathBuf>,

    // Oidc keys
    pub kid: String,
    pub public: String,
    pub private: String,
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
