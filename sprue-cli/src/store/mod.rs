// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{
    fs::{File as StdFile, OpenOptions, create_dir_all},
    io::Write,
    path::PathBuf,
};

use anyhow::{Result, anyhow};
use config::{Config, Environment, File};
use serde::{Deserialize, Serialize};

use crate::FormatStyle;

#[derive(Debug, Deserialize, Serialize)]
pub struct CliConfig {
    host: Option<String>,
    token: Option<String>,
    default_format: Option<FormatStyle>,
    mlink_redirect: Option<String>,
    mlink_secret: Option<String>,
}

impl CliConfig {
    pub fn new() -> Result<Self> {
        let (path, _) = Self::file(false)?;
        let config = Config::builder()
            .add_source(File::from(path))
            .add_source(Environment::default())
            .build()?;

        Ok(config.try_deserialize()?)
    }

    fn file(clear: bool) -> Result<(PathBuf, StdFile)> {
        let mut path = dirs::config_dir().expect("Failed to determine configs path");
        path.push("sprue");
        create_dir_all(&path).expect("Failed to create configs path");

        path.push("config.toml");
        let file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(clear)
            .open(&path)?;

        Ok((path, file))
    }

    pub fn host(&self) -> Result<&str> {
        self.host.as_deref().ok_or_else(|| {
            anyhow!("Host must either be configured via a configuration file or the environment")
        })
    }

    pub fn set_host(&mut self, host: String) {
        self.host = Some(host);
    }

    pub fn token(&self) -> Result<&str> {
        self.token.as_deref().ok_or_else(|| {
            anyhow!("Token must either be configured via a configuration file or the environment")
        })
    }

    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    pub fn format_style(&self) -> FormatStyle {
        self.default_format
            .as_ref()
            .cloned()
            .unwrap_or(FormatStyle::Json)
    }

    pub fn set_format(&mut self, format: FormatStyle) {
        self.default_format = Some(format);
    }

    pub fn mlink_redirect(&self) -> Result<&str> {
        self.mlink_redirect.as_deref().ok_or_else(|| {
            anyhow!("Magic link redirect uri must either be configured via a configuration file or the environment")
        })
    }

    pub fn set_mlink_redirect(&mut self, mlink_redirect: String) {
        self.mlink_redirect = Some(mlink_redirect);
    }

    pub fn mlink_secret(&self) -> Result<&str> {
        self.mlink_secret.as_deref().ok_or_else(|| {
            anyhow!("Magic link secret must either be configured via a configuration file or the environment")
        })
    }

    pub fn set_mlink_secret(&mut self, mlink_secret: String) {
        self.mlink_secret = Some(mlink_secret);
    }

    pub fn save(&self) -> Result<()> {
        let (filename, mut file) = Self::file(true)?;
        file.write_all(toml::to_string(&self)?.as_bytes())?;

        println!("Configuration updated. Wrote to: {}", filename.display());
        Ok(())
    }
}
