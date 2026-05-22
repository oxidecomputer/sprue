// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![allow(unused)]

use anyhow::{Result, anyhow};
use clap::{Arg, ArgAction, Command, CommandFactory, FromArgMatches, ValueEnum, value_parser};
use clap_complete::{Shell, generate};
use generated::cli::{CliConfig as ProgenitorCliConfig, *};
use owo_colors::OwoColorize;
use owo_colors::colors::xterm::DarkGreen;
use reqwest::header::{AUTHORIZATION, HeaderMap, HeaderValue};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sprue_sdk::Client;
use std::fmt::{Debug, Display};
use std::time::Duration;
use std::{collections::HashMap, error::Error};
use v_cli_sdk::{
    printer::Printer,
    {FormatStyle, VCliConfig, VCliContext, VerbosityLevel},
};

use crate::auth::LoginProvider;
use crate::context::Context;

mod auth;
mod config;
mod context;
mod generated;

#[derive(Debug, Default)]
struct Tree<'a> {
    children: HashMap<&'a str, Tree<'a>>,
    cmd: Option<CliCommand>,
}

impl<'a> Tree<'a> {
    fn cmd(&self, name: &str) -> Command {
        let mut cmd = if let Some(op) = self.cmd {
            // Command node
            Cli::<Context>::get_command(op).name(name.to_owned())
        } else {
            // Internal node
            Command::new(name.to_owned()).subcommand_required(true)
        };

        for (sub_name, sub_tree) in self.children.iter() {
            cmd = cmd.subcommand(sub_tree.cmd(sub_name));
        }

        cmd
    }
}

fn cmd_path<'a>(cmd: &CliCommand) -> Option<&'a str> {
    match cmd {
        CliCommand::CreateService => Some("service create"),
        CliCommand::GetService => Some("service get"),
        CliCommand::GetServiceServers => Some("service server list"),

        CliCommand::CreateDeployment => Some("deployment create"),
        CliCommand::GetDeployment => Some("deployment get"),
        CliCommand::ListDeployments => Some("deployment list"),
        CliCommand::DeleteDeployment => Some("deployment delete"),

        CliCommand::RegisterServer => Some("server register"),
        CliCommand::AcceptServer => Some("server accept"),
        CliCommand::ProveServer => Some("server prove"),
        CliCommand::RejectServer => Some("server reject"),
        CliCommand::TerminateServer => Some("server terminate"),
        CliCommand::CheckinServer => Some("server checkin"),

        CliCommand::RegisterOidcTokenRequest => Some("oidc token register"),
        CliCommand::ProveOidcTokenRequest => Some("oidc token prove"),

        CliCommand::RegisterBlob => Some("blob register"),
        CliCommand::WriteBlobUpload => Some("blob upload"),
        CliCommand::ResetBlobUpload => Some("blob reset"),
        CliCommand::CancelBlobUpload => Some("blob cancel"),
        CliCommand::CompleteBlobUpload => Some("blob complete"),

        // User commands
        CliCommand::CreateApiUser => Some("sys user create"),
        CliCommand::CreateApiUserToken => Some("sys user token create"),
        CliCommand::DeleteApiUserToken => Some("sys user token delete"),
        CliCommand::GetApiUser => Some("sys user get"),
        CliCommand::ListApiUsers => Some("sys user list"),
        CliCommand::GetApiUserToken => Some("sys user token get"),
        CliCommand::ListApiUserTokens => Some("sys user token list"),
        CliCommand::UpdateApiUser => Some("sys user update"),
        CliCommand::GetSelf => Some("self"),
        CliCommand::SetApiUserContactEmail => Some("sys user contact email set"),
        CliCommand::AddApiUserPermission => Some("sys user permission add"),
        CliCommand::RemoveApiUserPermission => Some("sys user permission remove"),

        // Link commands are handled separately
        CliCommand::CreateLinkToken => None,
        CliCommand::LinkProvider => None,

        // Group commands
        CliCommand::GetGroups => Some("sys group list"),
        CliCommand::CreateGroup => Some("sys group create"),
        CliCommand::UpdateGroup => Some("sys group update"),
        CliCommand::DeleteGroup => Some("sys group delete"),
        CliCommand::GetGroupMembers => Some("sys group membership list"),

        // User admin commands
        CliCommand::AddApiUserToGroup => Some("sys group membership add"),
        CliCommand::RemoveApiUserFromGroup => Some("sys group membership remove"),

        // Mapper commands
        CliCommand::GetMappers => Some("sys mapper list"),
        CliCommand::CreateMapper => Some("sys mapper create"),
        CliCommand::DeleteMapper => Some("sys mapper delete"),

        // OAuth client commands
        CliCommand::ListOauthClients => Some("sys oauth list"),
        CliCommand::CreateOauthClient => Some("sys oauth create"),
        CliCommand::GetOauthClient => Some("sys oauth get"),
        CliCommand::CreateOauthClientRedirectUri => Some("sys oauth redirect create"),
        CliCommand::DeleteOauthClientRedirectUri => Some("sys oauth redirect delete"),
        CliCommand::CreateOauthClientSecret => Some("sys oauth secret create"),
        CliCommand::DeleteOauthClientSecret => Some("sys oauth secret delete"),

        // Magic link client commands
        CliCommand::ListMagicLinks => Some("sys mlink list"),
        CliCommand::CreateMagicLink => Some("sys mlink create"),
        CliCommand::GetMagicLink => Some("sys mlink get"),
        CliCommand::CreateMagicLinkRedirectUri => Some("sys mlink redirect create"),
        CliCommand::DeleteMagicLinkRedirectUri => Some("sys mlink redirect delete"),
        CliCommand::CreateMagicLinkSecret => Some("sys mlink secret create"),
        CliCommand::DeleteMagicLinkSecret => Some("sys mlink secret delete"),

        // Authentication is handled separately
        CliCommand::ExchangeDeviceToken => None,
        CliCommand::GetDeviceProvider => None,
        CliCommand::MagicLinkSend => None,
        CliCommand::MagicLinkExchange => None,

        // Unsupported commands
        CliCommand::AuthzCodeRedirect => None,
        CliCommand::AuthzCodeCallback => None,
        CliCommand::AuthzCodeExchange => None,
        CliCommand::OpenidConfiguration => None,
        CliCommand::DeviceAuthz => None,
        CliCommand::ExchangeDeviceToken => None,
        CliCommand::GetDeviceProvider => None,
        CliCommand::GetWebPkceProvider => None,
        CliCommand::JwksJson => None,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut root = Tree::default();

    for cmd in CliCommand::iter() {
        if let Some(path) = cmd_path(&cmd) {
            let mut node = &mut root;

            let mut parts = path.split(' ').peekable();
            while let Some(ss) = parts.next() {
                if parts.peek().is_some() {
                    node = node.children.entry(ss).or_default();
                } else {
                    assert!(
                        !node.children.contains_key(ss),
                        "two identical subcommands {}",
                        path,
                    );
                    node.children.insert(
                        ss,
                        Tree {
                            cmd: Some(cmd),
                            ..Default::default()
                        },
                    );
                }
            }
        }
    }

    let mut cmd = root.cmd("sprue");
    cmd = cmd
        .bin_name("sprue")
        .arg(
            Arg::new("debug")
                .long("debug")
                .short('d')
                .help("Enable more verbose errors")
                .global(true)
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("format")
                .long("format")
                .short('f')
                .help("Specify the output format to display results in")
                .global(true)
                .value_parser(value_parser!(FormatStyle))
                .action(ArgAction::Set),
        );

    cmd = cmd.subcommand(v_cli_sdk::cmd::auth::Auth::<LoginProvider>::command());
    cmd = cmd.subcommand(v_cli_sdk::cmd::config::ConfigCmd::command());
    cmd = cmd.subcommand(
        Command::new("completion")
            .about("Generate shell completions")
            .arg(
                Arg::new("shell")
                    .short('s')
                    .long("shell")
                    .required(true)
                    .value_parser(value_parser!(Shell)),
            ),
    );
    cmd =
        cmd.subcommand(Command::new("version").about("Prints version information about the CLI."));

    let mut ctx = Context::new()?;

    let matches = cmd.clone().get_matches();

    if matches.try_get_one::<bool>("debug").ok().is_some() {
        ctx.set_verbosity(VerbosityLevel::All);
    }

    let format = matches
        .try_get_one::<FormatStyle>("format")
        .unwrap()
        .cloned()
        .unwrap_or_else(|| ctx.config().default_format());
    ctx.set_printer(Some(match format {
        FormatStyle::Json => Printer::Json,
        FormatStyle::Tab => Printer::Tab,
    }));

    let mut node = &root;
    let mut sm = &matches;

    match matches.subcommand() {
        Some(("version", _)) => {
            println!("{}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some(("completion", sub_matches)) => {
            let shell = sub_matches.get_one::<Shell>("shell").copied().unwrap();
            generate(shell, &mut cmd, "sprue-cli", &mut std::io::stdout());
            return Ok(());
        }
        Some(("auth", sub_matches)) => {
            v_cli_sdk::cmd::auth::Auth::<LoginProvider>::from_arg_matches(sub_matches)
                .unwrap()
                .run(&mut ctx)
                .await?;
        }
        Some(("config", sub_matches)) => {
            v_cli_sdk::cmd::config::ConfigCmd::from_arg_matches(sub_matches)
                .unwrap()
                .run(&mut ctx)
                .await?;
        }
        _ => {
            while let Some((sub_name, sub_matches)) = sm.subcommand() {
                node = node.children.get(sub_name).unwrap();
                sm = sub_matches;
            }

            let client = ctx.client();
            if client.is_none() {
                println!(
                    "A host must be configured. Run `sprue-cli config set host <HOST>` to configure a host."
                );
                std::process::exit(1);
            }
            let cli = Cli::new(ctx.client().expect("Client has been created"), ctx);
            if cli.execute(node.cmd.unwrap(), sm).await.is_err() {
                std::process::exit(1);
            }
        }
    };

    Ok(())
}

pub fn reserialize<T, U>(value: &T) -> U
where
    T: Serialize + Debug,
    U: DeserializeOwned,
{
    serde_json::from_str::<U>(&serde_json::to_string::<T>(value).unwrap()).unwrap()
}

impl ProgenitorCliConfig for Context {
    fn success_item<T>(&self, value: &progenitor_client::ResponseValue<T>)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug,
    {
        self.list_item(value.as_ref())
    }

    fn success_no_item(&self, value: &sprue_sdk::ResponseValue<()>) {
        self.list_item(value.as_ref())
    }

    fn error<T>(&self, value: &progenitor_client::Error<T>)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug,
    {
        self.printer().unwrap().print_error_response::<T>(value)
    }

    fn list_start<T>(&self)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug,
    {
    }

    fn list_item<T>(&self, value: &T)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug,
    {
        self.printer().unwrap().print_response(value)
    }

    fn list_end_success<T>(&self)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug,
    {
    }

    fn list_end_error<T>(&self, value: &progenitor_client::Error<T>)
    where
        T: schemars::JsonSchema + serde::Serialize + std::fmt::Debug,
    {
    }
}
