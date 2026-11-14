// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![allow(dead_code)]

use sprue_api::{context::policy::PolicyEngine, permissions::ApiPermissions};
use sprue_sdk::types::ServerRegistrationState;
use uuid::Uuid;
use vm_attest::VmInstanceConf;

use crate::common::SeededContext;

mod common;

static USER_SCOPE: &str = "user:info:r service:r service:w";

/// Permit registration when the server's project/silo pair matches a known
/// deployment on the service.
const POLICY: &str = r#"
    permit(
        principal,
        action == Sprue::Action::"registerServer",
        resource
    ) when {
        resource.deployments.contains({
            "project": principal.project,
            "silo": principal.silo
        })
    };
"#;

#[tokio::test]
async fn test_server_auto_registration() {
    let _ = tracing_subscriber::fmt::try_init();

    let policy = PolicyEngine::new(POLICY).unwrap();
    let seed = SeededContext::create_with_policy("server_auto_registration", Some(policy))
        .await
        .unwrap();
    let _handle = seed.server().run();

    let user_group = seed
        .group(vec![
            ApiPermissions::GetApiUserSelf,
            ApiPermissions::CreateService,
        ])
        .await;
    let user = seed.user(USER_SCOPE, vec![user_group]).await;
    tracing::info!(?user, "Created test user");

    let service = user
        .client
        .create_service()
        .body_map(|body| body.name("test"))
        .send()
        .await
        .unwrap()
        .into_inner();

    let vm_config: VmInstanceConf =
        serde_json::from_slice(include_bytes!("../test-data/attestation/vm.json")).unwrap();
    let vm = seed.vm(vm_config);

    // Create a deployment matching the VM's project/silo so the policy will
    // accept the registration automatically.
    user.client
        .create_deployment()
        .service(service.id.to_string())
        .body_map(|body| body.project(vm.conf().project).silo(vm.conf().silo))
        .send()
        .await
        .unwrap();

    let registration = vm.register(&service.id.to_string()).await;

    // No manual accept_server call — the policy engine should have done it.
    // Verify the server is in Accepted state.
    let servers = user
        .client
        .get_service_servers()
        .service(service.id.to_string())
        .send()
        .await
        .unwrap()
        .into_inner();

    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].id.0, registration.id.0);
    assert_eq!(servers[0].state, ServerRegistrationState::Accepted);
}

/// A VM that runs in a different project than any deployment of the service is
/// not accepted, because the policy sees the attested project rather than one
/// the VM reports about itself.
#[tokio::test]
async fn test_server_auto_registration_uses_attested_project() {
    let _ = tracing_subscriber::fmt::try_init();

    let policy = PolicyEngine::new(POLICY).unwrap();
    let seed = SeededContext::create_with_policy("server_auto_registration_attested", Some(policy))
        .await
        .unwrap();
    let _handle = seed.server().run();

    let user_group = seed
        .group(vec![
            ApiPermissions::GetApiUserSelf,
            ApiPermissions::CreateService,
        ])
        .await;
    let user = seed.user(USER_SCOPE, vec![user_group]).await;
    let service = user
        .client
        .create_service()
        .body_map(|body| body.name("test"))
        .send()
        .await
        .unwrap()
        .into_inner();

    let deployed: VmInstanceConf =
        serde_json::from_slice(include_bytes!("../test-data/attestation/vm.json")).unwrap();
    user.client
        .create_deployment()
        .service(service.id.to_string())
        .body_map(|body| body.project(deployed.project).silo(deployed.silo))
        .send()
        .await
        .unwrap();

    let mut elsewhere = deployed.clone();
    elsewhere.project = Uuid::new_v4();
    let vm = seed.vm(elsewhere);
    let registration = vm.register(&service.id.to_string()).await;

    assert_eq!(registration.state, ServerRegistrationState::Proven);
}

/// A server that is waiting for approval is accepted once a matching
/// deployment is created, without the VM doing anything further.
#[tokio::test]
async fn test_new_deployment_accepts_waiting_server() {
    let _ = tracing_subscriber::fmt::try_init();

    let policy = PolicyEngine::new(POLICY).unwrap();
    let seed =
        SeededContext::create_with_policy("server_auto_registration_deployment", Some(policy))
            .await
            .unwrap();
    let _handle = seed.server().run();

    let user_group = seed
        .group(vec![
            ApiPermissions::GetApiUserSelf,
            ApiPermissions::CreateService,
        ])
        .await;
    let user = seed.user(USER_SCOPE, vec![user_group]).await;
    let service = user
        .client
        .create_service()
        .body_map(|body| body.name("test"))
        .send()
        .await
        .unwrap()
        .into_inner();

    let vm_config: VmInstanceConf =
        serde_json::from_slice(include_bytes!("../test-data/attestation/vm.json")).unwrap();
    let vm = seed.vm(vm_config);
    let registration = vm.register(&service.id.to_string()).await;
    assert_eq!(registration.state, ServerRegistrationState::Proven);

    user.client
        .create_deployment()
        .service(service.id.to_string())
        .body_map(|body| body.project(vm.conf().project).silo(vm.conf().silo))
        .send()
        .await
        .unwrap();

    let servers = user
        .client
        .get_service_servers()
        .service(service.id.to_string())
        .send()
        .await
        .unwrap()
        .into_inner();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].state, ServerRegistrationState::Accepted);
}
