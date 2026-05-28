// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use chrono::{Duration, Utc};
use dice_verifier::AttestMock;
use http::{HeaderValue, header::AUTHORIZATION};
use newtype_uuid::{GenericUuid, TypedUuid};
use sprue_api::{context::ApiContext, permissions::ApiPermissions};
use sprue_sdk::Client as SprueClient;
use std::{
    ops::Add,
    path::{PathBuf, absolute},
};
use uuid::Uuid;
use v_api::{ApiContext as VApiContext, authn::key::RawKey};
use v_model::{
    AccessGroupId, ApiUserInfo,
    schema_ext::MagicLinkMedium as Medium,
    storage::{ApiUserFilter, ListPagination},
};
use vm_attest::{VmInstanceConf, VmInstanceRot};

use super::SeededMagicLink;

#[derive(Debug)]
pub struct MockUser {
    pub client: SprueClient,
    pub email: String,
    pub user: ApiUserInfo<ApiPermissions>,
}

impl MockUser {
    pub async fn create(
        server: &str,
        groups: Vec<TypedUuid<AccessGroupId>>,
        scope: &str,
        magic_link: &SeededMagicLink,
        ctx: &ApiContext,
    ) -> anyhow::Result<MockUser> {
        let http = reqwest::Client::new();
        let client = SprueClient::new_with_client(server, http);

        // Generate a user email
        let email = format!("user-{}@localhost", Uuid::new_v4());

        // Use the context to register the applicant so that we can control
        // the secret that is signed
        let key = RawKey::generate::<8>(&Uuid::new_v4());
        let exchange_secret = hex::encode(key.expose_secret());
        let redirect_uri =
            serde_json::from_str::<Vec<_>>(&format!("[\"{}\"]", magic_link.redirect_uri))?
                .pop()
                .unwrap();

        // Emulate sending a login magic link
        let attempt = ctx
            .v_ctx()
            .magic_link
            .send_login_attempt(
                key,
                ctx.v_ctx().signer(),
                magic_link.client.id,
                &redirect_uri,
                Medium::Email,
                "all",
                scope,
                Utc::now().add(Duration::seconds(60)),
                &email,
            )
            .await?;

        tracing::trace!(?exchange_secret, "Attempt to exchange magic link secret");

        // Emulate accepting the magic link login
        let token = client
            .magic_link_exchange()
            .channel("all")
            .body_map(|body| {
                body.attempt_id(attempt.id.into_untyped_uuid())
                    .recipient(email.clone())
                    .secret(exchange_secret)
            })
            .send()
            .await?
            .into_inner()
            .access_token;

        tracing::info!(?token, "Retrieved access token");

        // Redefine the client now that we have an access token to act on behalf of the user
        let http = reqwest::Client::builder()
            .default_headers(
                [(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", token))?,
                )]
                .into_iter()
                .collect(),
            )
            .build()?;

        let client = SprueClient::new_with_client(server, http);
        let mut filter = ApiUserFilter::default();
        filter.email = Some(vec![email.clone()]);
        tracing::info!(?filter, "Looking up user");
        let user = ctx
            .v_ctx()
            .user
            .list_api_user(
                &ctx.v_ctx().builtin_registration_user(),
                filter,
                &ListPagination::latest(),
            )
            .await?
            .pop()
            .unwrap();

        for group in groups {
            ctx.v_ctx()
                .add_api_user_to_group(
                    &ctx.v_ctx().builtin_registration_user(),
                    &user.user.id,
                    &group,
                )
                .await?;
        }

        // Look up the user again post group membership update
        let user = ctx
            .v_ctx()
            .user
            .get_api_user(&ctx.v_ctx().builtin_registration_user(), &user.user.id)
            .await?;

        Ok(MockUser {
            client,
            email,
            user,
        })
    }
}

#[derive(Debug)]
pub struct MockVm {
    server: String,
    pub conf: VmInstanceConf,
    pub client: SprueClient,
}

impl MockVm {
    pub fn create(server: &str, conf: VmInstanceConf) -> Self {
        Self {
            server: server.to_string(),
            conf: conf.clone(),
            client: SprueClient::new(server),
        }
    }

    pub fn install_token(&mut self, token: &str) {
        let http = reqwest::Client::builder()
            .default_headers(
                [(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {}", token)).unwrap(),
                )]
                .into_iter()
                .collect(),
            )
            .build()
            .unwrap();

        self.client = SprueClient::new_with_client(&self.server, http);
    }

    pub fn conf(&self) -> &VmInstanceConf {
        &self.conf
    }

    pub fn rot(&self) -> VmInstanceRot {
        let attest = Box::new(
            AttestMock::load(
                absolute(PathBuf::from("test-data/attestation/cert-chain.pem")).unwrap(),
                absolute(PathBuf::from("test-data/attestation/log.bin")).unwrap(),
                absolute(PathBuf::from("test-data/attestation/alias.key")).unwrap(),
            )
            .unwrap(),
        );
        VmInstanceRot::new(attest)
    }
}
