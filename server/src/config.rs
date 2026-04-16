use std::path::PathBuf;

use config::{Config, ConfigError, Environment, File};
use serde::Deserialize;
use tap::TapFallible;
use v_api::config::{AsymmetricKey, ServerLogFormat, SpecConfig};
use v_api_param::StringParam;

#[derive(Debug, Deserialize)]
pub struct ServerConfig {
    pub log_format: ServerLogFormat,
    pub log_directory: Option<PathBuf>,
    pub param_base_path: Option<PathBuf>,
    pub spec: Option<SpecConfig>,
    pub public_url: String,
    pub database_url: StringParam,
    pub jwt: JwtConfig,
    pub vm_identity: VmIdentityConfig,
    pub oidc: OidcConfig,
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
    pub issuer: String,
    pub audience: String,
    pub token_lifetime: u32,
    pub token_request_duration: u64,
}

#[derive(Debug, Deserialize)]
pub struct BackupConfig {
    pub local_root: PathBuf,
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
