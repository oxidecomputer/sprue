// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use async_trait::async_trait;
use dice_verifier::{Corim, ReferenceMeasurements};
use dropshot::ServerBuilder;
use newtype_uuid::TypedUuid;
use rand::RngExt;
use slog::{Discard, Logger, o};
use sprue_api::{
    config::{OidcConfig, OidcTokenConfig},
    context::{
        ApiContext, ApiContextBuilder,
        blob::{BackupStorage, BlobContext, LocalBackupStorage},
        idempotency::IdempotencyContext,
        oidc::OidcContext,
        policy::PolicyEngine,
        server_identity::ServerIdentityContext,
        service::ServiceContext,
    },
    create_server,
    permissions::ApiPermissions,
};
use sprue_model::{storage::postgres::PostgresStorage, test_util::TestDb};
use std::{
    collections::HashMap,
    fs,
    path::{PathBuf, absolute},
    sync::Arc,
    time::Duration,
};
use steno::ActionRegistry;
use strum::IntoEnumIterator;
use tokio::task::JoinHandle;
use v_api::{
    ApiContext as VApiContext, MagicLinkMessage, MagicLinkTarget, VContextBuilder,
    config::AsymmetricKey,
    messenger::{Message, Messenger, MessengerError},
};
use v_model::{
    AccessGroupId, MagicLink, NewAccessGroup, permissions::Caller, schema_ext::MagicLinkMedium,
};
use vm_attest::{QualifyingData, VmInstanceConf};
use x509_cert::Certificate;

use crate::common::actors::{MockUser, MockVm};

pub mod actors;

pub struct SeededContext {
    pub system_caller: Caller<ApiPermissions>,
    pub ctx: ApiContext,
    pub port: u16,
    pub service: String,
    pub magic_link: SeededMagicLink,
    #[allow(dead_code)]
    db: TestDb,
    pub test_storage: Vec<PathBuf>,
}

pub struct SeededMagicLink {
    pub client: MagicLink,
    pub redirect_uri: String,
    #[allow(dead_code)]
    pub secret: String,
}

struct Noop {}

impl MagicLinkMessage for Noop {
    fn create_message(
        &self,
        _recipient: &str,
        _token: &str,
        _url: &reqwest::Url,
    ) -> Option<Message> {
        Some(Message {
            recipient: String::new(),
            subject: None,
            text: String::new(),
            html: None,
        })
    }
}

#[async_trait]
impl Messenger for Noop {
    async fn send(&self, _message: Message) -> Result<(), MessengerError> {
        Ok(())
    }
}

fn url(port: u16, path: &str) -> String {
    format!("http://0.0.0.0:{}/{}", port, path)
        .trim_end_matches('/')
        .to_string()
}

impl SeededContext {
    pub async fn create(test_name: &str) -> anyhow::Result<SeededContext> {
        Self::create_with_policy(test_name, None).await
    }

    pub async fn create_with_policy(
        test_name: &str,
        policy: Option<PolicyEngine>,
    ) -> anyhow::Result<SeededContext> {
        let test_id = TypedUuid::new_v4();
        let db: TestDb = TestDb::new(test_name).await;

        let mut rng = rand::rng();
        let port = rng.random_range(8000..8888);
        let service = url(port, "");

        let mut v_context = VContextBuilder::<ApiPermissions>::new()
            .with_public_url(url(port, ""))
            .with_storage_url(db.url())
            .with_jwt_expiration(24 * 60 * 60)
            .with_keys(vec![
                AsymmetricKey::LocalVerifier {
                    kid: "test-key".to_string(),
                    public: include_str!("../../test-data/api/cert.pem")
                        .to_string()
                        .into(),
                },
                AsymmetricKey::LocalSigner {
                    kid: "test-key".to_string(),
                    private: include_str!("../../test-data/api/key.pem")
                        .to_string()
                        .into(),
                },
            ])
            .with_saga_backend(test_id, None)
            .with_additional_builtin_permissions(ApiPermissions::iter().collect())
            .build()
            .await?;

        let caller = Caller {
            id: TypedUuid::new_v4(),
            permissions: ApiPermissions::iter().collect(),
            extensions: HashMap::new(),
        };

        let magic_link = v_context.magic_link.create_magic_link(&caller).await?;
        v_context
            .magic_link
            .add_magic_link_secret(&caller, &TypedUuid::new_v4(), &magic_link.id, "1234")
            .await?;
        v_context
            .magic_link
            .add_magic_link_redirect_uri(&caller, &magic_link.id, &url(port, "mlink-return"))
            .await?;
        let target = MagicLinkTarget {
            medium: MagicLinkMedium::Email,
            channel: "all".to_string(),
        };
        v_context
            .magic_link
            .set_message_builder(target.clone(), Noop {});
        v_context.magic_link.set_messenger(target, Noop {});

        let v_ctx = Arc::new(v_context);
        let storage = Arc::new(PostgresStorage::create(&db.url()).unwrap());

        let path_id = format!("{}-{}", test_name, test_id);
        let local_storage = PathBuf::from(format!("test-data/storage/local/{}", path_id));
        fs::create_dir_all(absolute(&local_storage)?)?;
        let remote_storage = PathBuf::from(format!("test-data/storage/remote/{}", path_id));
        fs::create_dir_all(absolute(&remote_storage)?)?;
        let test_storage = vec![local_storage.clone(), remote_storage.clone()];

        let context = ApiContextBuilder::default()
            .public_url(url(port, ""))
            .blob(BlobContext::new(
                local_storage,
                storage.clone(),
                BackupStorage::Local(LocalBackupStorage::new(remote_storage)),
            ))
            .idempotency(IdempotencyContext::new(storage.clone()))
            .oidc(
                OidcContext::new(
                    v_ctx.issuer(),
                    OidcConfig {
                        token: OidcTokenConfig {
                            audience: v_ctx.issuer(),
                            token_lifetime: 30,
                            token_request_duration: 10,
                        },
                    },
                    storage.clone(),
                )
                .unwrap(),
            )
            .server_identity(ServerIdentityContext::new(
                "Oxide Computer Company".to_string(),
                Certificate::load_pem_chain(include_bytes!(
                    "../../test-data/attestation/root.crt"
                ))?,
                Arc::new(ReferenceMeasurements::try_from(std::slice::from_ref(
                    &Corim::from_bytes(include_bytes!("../../test-data/attestation/corim.cbor"))?,
                ))?),
            ))
            .service(ServiceContext::new(storage, Duration::from_secs(10)))
            .saga_action_registry(Arc::new(ActionRegistry::new()))
            .policy(policy)
            .v_ctx(v_ctx)
            .build()?;

        Ok(SeededContext {
            system_caller: caller,
            ctx: context,
            port,
            service,
            magic_link: SeededMagicLink {
                client: magic_link,
                redirect_uri: url(port, "mlink-return"),
                secret: "1234".to_string(),
            },
            db,
            test_storage,
        })
    }

    pub async fn group(&self, permissions: Vec<ApiPermissions>) -> TypedUuid<AccessGroupId> {
        let group_id = TypedUuid::new_v4();
        let group = self
            .ctx
            .v_ctx()
            .group
            .create_group(
                &self.system_caller,
                NewAccessGroup {
                    id: group_id,
                    name: group_id.to_string(),
                    permissions: permissions.into(),
                },
            )
            .await
            .unwrap();
        group.id
    }

    pub async fn user(&self, scope: &str, groups: Vec<TypedUuid<AccessGroupId>>) -> MockUser {
        MockUser::create(&self.service, groups, scope, &self.magic_link, &self.ctx)
            .await
            .unwrap()
    }

    pub fn vm(&self, conf: VmInstanceConf) -> MockVm {
        MockVm::create(&self.service, conf)
    }

    pub fn server(&self) -> MockServer {
        MockServer {
            inner: create_server(self.ctx.clone(), Logger::root(Discard, o! {}), self.port),
        }
    }
}

impl Drop for SeededContext {
    fn drop(&mut self) {
        for path in &self.test_storage {
            fs::remove_dir_all(path).unwrap();
        }
    }
}

pub struct MockServer {
    inner: ServerBuilder<ApiContext>,
}

impl MockServer {
    pub fn run(self) -> JoinHandle<()> {
        tokio::spawn(async move {
            tracing::info!("Starting test server");
            self.inner.start().unwrap().await.unwrap();
        })
    }
}

pub fn nonce_to_data(nonce: &str) -> QualifyingData {
    let nonce: [u8; 32] = hex::decode(nonce).unwrap().try_into().unwrap();
    QualifyingData::from(nonce)
}
