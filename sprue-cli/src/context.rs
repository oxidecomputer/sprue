// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use oauth2::http::{HeaderMap, HeaderValue};
use reqwest::header::AUTHORIZATION;
use sprue_sdk::Client;
use std::{sync::Arc, time::Duration};
use thiserror::Error;
use v_cli_sdk::{
    VCliConfig, VCliContext, VerbosityLevel,
    cmd::auth::{login::CliMagicLinkAdapter, oauth::CliOAuthAdapter},
    printer::Printer,
};

use crate::{
    auth::{AdapterToken, MagicLinkAdapter, OAuthAdapter},
    config::Config,
    config::missing_value,
    generated::cli::CliConfig,
};

#[derive(Debug, Error)]
pub enum ContextError {
    #[error("Client error")]
    Client(#[from] sprue_sdk::Error<sprue_sdk::types::Error>),
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error(transparent)]
    Hyper(#[from] hyper::Error),
    #[error("No client configured. Run `support-cli config set host <HOST>` to configure a host.")]
    NoClient,
    #[error("No host configured. Run `support-cli config set host <HOST>` to configure a host.")]
    NoHost,
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
    #[error("SDK error: {0}")]
    Sdk(String),
    #[error("Unsupported OAuth provider")]
    UnsupportedOAuthProvider,
}

#[derive(Debug, Clone)]
pub struct Context {
    config: Config,
    printer: Option<Printer>,
    verbosity: VerbosityLevel,
}

impl Context {
    pub fn new() -> anyhow::Result<Self> {
        let config = Config::load()?;

        Ok(Self {
            config,
            printer: None,
            verbosity: VerbosityLevel::None,
        })
    }

    pub fn new_client(host: &str, token: Option<&str>) -> anyhow::Result<Client> {
        let mut default_headers = HeaderMap::new();

        if let Some(token) = token {
            let mut auth_header = HeaderValue::from_str(&format!("Bearer {}", token))?;
            auth_header.set_sensitive(true);
            default_headers.insert(AUTHORIZATION, auth_header);
        }

        let http_client = reqwest::Client::builder()
            .default_headers(default_headers)
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(10))
            .build()?;

        Ok(Client::new_with_client(host, http_client))
    }

    pub fn set_printer(&mut self, printer: Option<Printer>) {
        self.printer = printer;
    }

    pub fn set_verbosity(&mut self, verbosity: VerbosityLevel) {
        self.verbosity = verbosity;
    }
}

impl VCliContext<sprue_sdk::Client, Printer> for Context {
    type ShortToken = AdapterToken;
    type LongToken = AdapterToken;
    type Error = ContextError;

    fn config(&self) -> &impl VCliConfig {
        &self.config
    }
    fn config_mut(&mut self) -> &mut impl VCliConfig {
        &mut self.config
    }
    fn client(&self) -> Option<sprue_sdk::Client> {
        let client = Self::new_client(
            self.config.host().ok_or(missing_value("host")).ok()?,
            self.config.token(),
        )
        .ok()?;
        Some(client)
    }
    fn printer(&self) -> Option<&Printer> {
        self.printer.as_ref()
    }
    fn verbosity(&self) -> VerbosityLevel {
        self.verbosity
    }

    fn oauth_adapter(
        &self,
    ) -> impl CliOAuthAdapter<
        ShortToken = Self::ShortToken,
        LongToken = Self::LongToken,
        Error = Self::Error,
    > + Send
    + Sync
    + 'static {
        OAuthAdapter::new(self.clone())
    }
    fn mlink_adapter(
        &self,
    ) -> impl CliMagicLinkAdapter<Token = Self::LongToken, Error = Self::Error> + Send + Sync + 'static
    {
        MagicLinkAdapter {}
    }
}
