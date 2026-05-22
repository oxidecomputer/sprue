// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![allow(dead_code)]

use sprue_api::permissions::ApiPermissions;
use uuid::Uuid;
use vm_attest::VmInstanceConf;

use crate::common::{SeededContext, nonce_to_data};

mod common;

static USER_SCOPE: &str = "user:info:r service:r service:w";

#[tokio::test]
async fn test_upload_blob() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("upload_blob").await.unwrap();
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
    let mut vm = seed.vm(vm_config);
    let registration = vm
        .client
        .register_server()
        .service(service.id.0)
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

    let qualifying_data = nonce_to_data(registration.nonce.as_ref().unwrap());
    let attestation = vm.rot().attest(&vm.conf(), &qualifying_data).await.unwrap();
    vm.client
        .prove_server()
        .server(registration.id.0)
        .body_map(|body| body.attestation(serde_json::to_value(attestation).unwrap()))
        .send()
        .await
        .unwrap();

    // We should have now successfully registered the server, and the user can now approve it
    user.client
        .accept_server()
        .server(registration.id.0)
        .send()
        .await
        .unwrap();

    // The server now needs a token to interact with the API
    let token_request = vm
        .client
        .register_oidc_token_request()
        .server(registration.id.0)
        .send()
        .await
        .unwrap()
        .into_inner();
    tracing::debug!(?token_request, "Requested token");
    let token_qualifying_data = nonce_to_data(&token_request.nonce.as_ref().unwrap());
    let token_attestation = vm
        .rot()
        .attest(&vm.conf(), &token_qualifying_data)
        .await
        .unwrap();
    tracing::debug!("Generated token attestation");
    let token = vm
        .client
        .prove_oidc_token_request()
        .server(registration.id.0)
        .body_map(|body| {
            body.attestation(serde_json::to_value(token_attestation).unwrap())
                .request(token_request.id.0)
        })
        .send()
        .await
        .unwrap()
        .into_inner()
        .token;
    tracing::debug!(?token, "Received token");
    vm.install_token(&token);

    // Generate a random key for idempotency
    let idempotency_key1 = Uuid::new_v4().to_string();
    let idempotency_key2 = Uuid::new_v4().to_string();

    // The server can now register and upload a blob
    let blob1 = vm
        .client
        .register_blob()
        .server(registration.id.0)
        .body_map(|body| body.idempotency_key(idempotency_key1.clone()).size(1024))
        .send()
        .await
        .unwrap()
        .into_inner()
        .blob;

    // Request a second blob with a new key
    let blob2 = vm
        .client
        .register_blob()
        .server(registration.id.0)
        .body_map(|body| body.idempotency_key(idempotency_key2).size(1024))
        .send()
        .await
        .unwrap()
        .into_inner()
        .blob;

    // Request the blob again with the same key
    let blob3 = vm
        .client
        .register_blob()
        .server(registration.id.0)
        .body_map(|body| body.idempotency_key(idempotency_key1).size(1024))
        .send()
        .await
        .unwrap()
        .into_inner()
        .blob;

    // Ensure blobs with the same key return the same ID
    assert_eq!(blob1.id.0, blob3.id.0);

    // Ensure blobs with different keys return different IDs
    assert_ne!(blob1.id.0, blob2.id.0);
}
