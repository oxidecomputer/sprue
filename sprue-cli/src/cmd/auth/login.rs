// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use std::{io::Write, ops::Add};

use anyhow::Result;
use chrono::{Duration, NaiveDate, TimeDelta, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use oauth2::TokenResponse;
use sprue_sdk::types::{MagicLinkMedium, OAuthProviderName, ApiPermissions};

use crate::{cmd::auth::oauth, Context};

// One week in seconds
static MAGIC_LINK_SESSION_LENGTH: i64 = 604800;

// Authenticates and generates an access token for interacting with the api
#[derive(Parser, Debug, Clone)]
#[clap(name = "login")]
pub struct Login {
    #[command(subcommand)]
    provider: LoginProvider,
    #[arg(short = 'm', default_value = "id")]
    mode: AuthenticationMode,
}

impl Login {
    pub async fn run(&self, ctx: &mut Context) -> Result<()> {
        let access_token = self.provider.run(ctx, &self.mode).await?;

        ctx.config.set_token(access_token);
        ctx.config.save()?;

        Ok(())
    }
}

#[derive(Subcommand, Debug, Clone)]
pub enum LoginProvider {
    #[command(name = "google")]
    /// Login via Google
    Google,
    /// Login via Magic Link
    #[command(name = "mlink")]
    MagicLink {
        /// Email recipient to login via
        email: String,
        /// Optional access scopes to apply to this session
        scope: Option<String>,
    },
}

#[derive(ValueEnum, Debug, Clone, PartialEq)]
pub enum AuthenticationMode {
    /// Retrieve and store an identity token. Identity mode is the default and should be used to
    /// when you do not require extended (multi-day) access
    #[value(name = "id")]
    Identity,
    /// Retrieve and store an api token. Token mode should be used when you want to authenticate
    /// a machine for continued access. This requires the permission to create api tokens
    #[value(name = "token")]
    Token,
}

impl LoginProvider {
    fn as_name(&self) -> Option<OAuthProviderName> {
        match self {
            Self::Google => Some(OAuthProviderName::Google),
            Self::MagicLink { .. } => None,
        }
    }

    pub async fn run(&self, ctx: &mut Context, mode: &AuthenticationMode) -> Result<String> {
        match self {
            Self::Google => {
                self.run_oauth_provider(self.as_name().unwrap(), ctx, mode)
                    .await
            }
            Self::MagicLink { email, scope } => {
                self.run_magic_link(ctx, email, scope.as_deref(), mode)
                    .await
            }
        }
    }

    async fn run_oauth_provider(
        &self,
        name: OAuthProviderName,
        ctx: &mut Context,
        mode: &AuthenticationMode,
    ) -> Result<String> {
        let provider = ctx
            .client()?
            .get_device_provider()
            .provider(name)
            .send()
            .await?;

        let oauth_client = oauth::DeviceOAuth::new(provider.into_inner())?;
        let details = oauth_client.get_device_authorization().await?;

        println!(
            "To complete login visit: {} and enter {}",
            details.verification_uri().as_str(),
            details.user_code().secret()
        );

        let token_response = oauth_client.login(&details).await;

        let identity_token = match token_response {
            Ok(token) => Ok(token.access_token().to_owned()),
            Err(err) => Err(anyhow::anyhow!("Authentication failed: {}", err)),
        }?;

        if mode == &AuthenticationMode::Token {
            let client = ctx.new_client(Some(identity_token.secret()))?;
            let user = client.get_self().send().await?;
            Ok(client
                .create_api_user_token()
                .user_id(user.info.id.clone())
                .body_map(|body| body.expires_at(Utc::now().add(TimeDelta::try_days(365).unwrap())))
                .send()
                .await?
                .key
                .to_string())
        } else {
            Ok(identity_token.secret().to_string())
        }
    }

    async fn run_magic_link(
        &self,
        ctx: &mut Context,
        email: &str,
        scope: Option<&str>,
        mode: &AuthenticationMode,
    ) -> Result<String> {
        let secret = ctx.config.mlink_secret()?.to_string();
        let redirect_uri = ctx.config.mlink_redirect()?.to_string();

        let attempt = ctx
            .client()?
            .magic_link_send()
            .channel("cli")
            .body_map(|body| {
                body.secret(secret.clone())
                    .redirect_uri(redirect_uri)
                    .expires_in(MAGIC_LINK_SESSION_LENGTH)
                    .medium(MagicLinkMedium::Email)
                    .scope(scope.map(&str::to_string))
                    .recipient(email)
            })
            .send()
            .await?
            .into_inner();

        let mut auth_secret = String::new();
        print!("Enter the login token sent to the recipient: ");
        std::io::stdout().flush()?;
        std::io::stdin().read_line(&mut auth_secret)?;

        let token = ctx
            .client()?
            .magic_link_exchange()
            .channel("cli")
            .body_map(|body| {
                body.attempt_id(attempt.attempt_id)
                    .recipient(email)
                    .secret(auth_secret.trim())
            })
            .send()
            .await?
            .into_inner();

        Ok(token.access_token)
    }
}
