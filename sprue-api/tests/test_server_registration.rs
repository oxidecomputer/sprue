// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![allow(dead_code)]

use sprue_api::permissions::ApiPermissions;
use sprue_sdk::types::ServerRegistrationState;
use vm_attest::VmInstanceConf;

use crate::common::{SeededContext, nonce_to_data};

mod common;

static USER_SCOPE: &str = "user:info:r service:r service:w";

#[tokio::test]
async fn test_server_registration() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("server_registration").await.unwrap();
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

    // Create a registration requet that we can prove against
    let registration = vm
        .client
        .register_server()
        .service(service.id.clone())
        .body_map(|body| body.instance(vm.conf.uuid))
        .send()
        .await
        .unwrap()
        .into_inner()
        .registration;

    // Transform the nonce into qualifying data
    let qualifying_data = nonce_to_data(registration.nonce.as_ref().unwrap());

    // Generate an attestation for the VM from our mock RoT to provide the VM's identity
    let attestation = vm.rot().attest(&vm.conf(), &qualifying_data).await.unwrap();

    // Now that we have an attestation, we can prove our identity to the server
    vm.client
        .prove_server()
        .server(registration.id.clone())
        .body_map(|body| body.attestation(serde_json::to_value(attestation).unwrap()))
        .send()
        .await
        .unwrap();

    // We should have now successfully registered the server, and the user can now approve it
    user.client
        .accept_server()
        .server(registration.id.clone())
        .send()
        .await
        .unwrap();

    // The server should now be approved and visible to the user
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
