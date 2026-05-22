// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![allow(dead_code)]

use sprue_api::{context::policy::PolicyEngine, permissions::ApiPermissions};
use sprue_sdk::types::ServerRegistrationState;
use vm_attest::VmInstanceConf;

use crate::common::{SeededContext, nonce_to_data};

mod common;

static USER_SCOPE: &str = "user:info:r service:r service:w";

const SCHEMA: &str = r#"
    namespace Sprue {
        entity Project;
        entity Silo;
        entity Instance {
            project: Project,
            silo: Silo,
        };
        entity Service {
            deployments: Set<{
                "project": Project,
                "silo": Silo,
            }>,
        };
        action registerServer
            appliesTo { principal: [Instance], resource: [Service] };
    }
"#;

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

    let policy = PolicyEngine::new(POLICY, SCHEMA).unwrap();
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
        .service(service.id.clone())
        .body_map(|body| body.project_id(vm.conf().project).silo_id(vm.conf().silo))
        .send()
        .await
        .unwrap();

    // Register the server — same flow as the manual test
    let registration = vm
        .client
        .register_server()
        .service(service.id.clone())
        .body_map(|body| {
            body.instance(vm.conf.uuid)
                .project_id(vm.conf().project)
                .silo_id(vm.conf().silo)
        })
        .send()
        .await
        .unwrap()
        .into_inner()
        .registration;

    // Transform the nonce into qualifying data
    let qualifying_data = nonce_to_data(registration.nonce.as_ref().unwrap());

    // Generate an attestation for the VM from our mock RoT to provide the VM's identity
    let attestation = vm.rot().attest(&vm.conf(), &qualifying_data).await.unwrap();

    // Prove identity — the policy engine should auto-accept the server
    vm.client
        .prove_server()
        .server(registration.id.clone())
        .body_map(|body| body.attestation(serde_json::to_value(attestation).unwrap()))
        .send()
        .await
        .unwrap();

    // No manual accept_server call — the policy engine should have done it.
    // Verify the server is in Accepted state.
    let servers = user
        .client
        .get_service_servers()
        .service(service.id)
        .send()
        .await
        .unwrap()
        .into_inner();

    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].id.0, registration.id.0);
    assert_eq!(servers[0].state, ServerRegistrationState::Accepted);
}
