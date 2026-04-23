// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{Context, FormatStyle};

#[derive(Debug, Parser)]
#[clap(name = "config")]
pub struct ConfigCmd {
    #[clap(subcommand)]
    setting: SettingCmd,
}

#[derive(Debug, Subcommand)]
pub enum SettingCmd {
    /// Gets a setting
    #[clap(subcommand, name = "get")]
    Get(GetCmd),
    /// Sets a setting
    #[clap(subcommand, name = "set")]
    Set(SetCmd),
}

#[derive(Debug, Subcommand)]
pub enum GetCmd {
    /// Get the default formatter to use when printing results
    #[clap(name = "format")]
    Format,
    /// Get the configured API host in use
    #[clap(name = "host")]
    Host,
    /// Get the configured access token
    #[clap(name = "token")]
    Token,
    /// Get the configured magic redirect uri
    #[clap(name = "mlink-redirect")]
    MagicLinkRedirectUri,
    /// Get the configured magic link secret
    #[clap(name = "mlink-secret")]
    MagicLinkSecret,
}

#[derive(Debug, Subcommand)]
pub enum SetCmd {
    /// Set the default formatter to use when printing results
    #[clap(name = "format")]
    Format { format: FormatStyle },
    /// Set the configured API host to use
    #[clap(name = "host")]
    Host { host: String },
    /// Set the configured magic redirect uri
    #[clap(name = "mlink-redirect")]
    MagicLinkRedirectUri { redirect: String },
    /// Set the configured magic link secret
    #[clap(name = "mlink-secret")]
    MagicLinkSecret { secret: String },
}

impl ConfigCmd {
    pub async fn run(&self, ctx: &mut Context) -> Result<()> {
        match &self.setting {
            SettingCmd::Get(get) => get.run(ctx).await?,
            SettingCmd::Set(set) => set.run(ctx).await?,
        }

        Ok(())
    }
}

impl GetCmd {
    pub async fn run(&self, ctx: &mut Context) -> Result<()> {
        match &self {
            GetCmd::Format => {
                println!("{}", ctx.config.format_style());
            }
            GetCmd::Host => {
                println!("{}", ctx.config.host().unwrap_or("None"));
            }
            GetCmd::Token => {
                println!("{}", ctx.config.token().unwrap_or("None"));
            }
            GetCmd::MagicLinkRedirectUri => {
                println!("{}", ctx.config.mlink_redirect().unwrap_or("None"));
            }
            GetCmd::MagicLinkSecret => {
                println!("{}", ctx.config.mlink_secret().unwrap_or("None"));
            }
        }

        Ok(())
    }
}

impl SetCmd {
    pub async fn run(&self, ctx: &mut Context) -> Result<()> {
        match &self {
            SetCmd::Format { format } => {
                ctx.config.set_format(format.clone());
                ctx.config.save()?;
            }
            SetCmd::Host { host } => {
                ctx.config.set_host(host.to_string());
                ctx.config.save()?;
            }
            SetCmd::MagicLinkRedirectUri { redirect } => {
                ctx.config.set_mlink_redirect(redirect.to_string());
                ctx.config.save()?;
            }
            SetCmd::MagicLinkSecret { secret } => {
                ctx.config.set_mlink_secret(secret.to_string());
                ctx.config.save()?;
            }
        }

        Ok(())
    }
}
