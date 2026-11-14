// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

#![allow(dead_code)]

use newtype_uuid::GenericUuid;
use sprue_sdk::ClientInfo;
use reqwest::StatusCode;
use sprue_api::{context::oidc::VmClaims, permissions::ApiPermissions};
use sprue_sdk::types::{ServerRegistration, ServerRegistrationState, Service};
use uuid::Uuid;
use vm_attest::VmInstanceConf;

use crate::common::{SeededContext, actors::MockUser};

mod common;

static USER_SCOPE: &str = "user:info:r service:r service:w";

fn vm_config() -> VmInstanceConf {
    serde_json::from_slice(include_bytes!("../test-data/attestation/vm.json")).unwrap()
}

async fn user_with_services(seed: &SeededContext, names: &[&str]) -> (MockUser, Vec<Service>) {
    let user_group = seed
        .group(vec![
            ApiPermissions::GetApiUserSelf,
            ApiPermissions::CreateService,
        ])
        .await;
    let user = seed.user(USER_SCOPE, vec![user_group]).await;
    let mut services = Vec::new();
    for name in names {
        services.push(
            user.client
                .create_service()
                .body_map(|body| body.name(*name))
                .send()
                .await
                .unwrap()
                .into_inner(),
        );
    }
    (user, services)
}

#[tokio::test]
async fn test_server_registration() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("server_registration").await.unwrap();
    let _handle = seed.server().run();
    let (user, services) = user_with_services(&seed, &["test"]).await;
    let service = &services[0];

    let vm = seed.vm(vm_config());
    let registration = vm.register(&service.id.to_string()).await;
    assert_eq!(registration.state, ServerRegistrationState::Proven);

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
        .service(service.id.to_string())
        .send()
        .await
        .unwrap()
        .into_inner();

    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].id.0, registration.id.0);
    assert_eq!(servers[0].state, ServerRegistrationState::Accepted);

    // Registering again returns the same registration in its current state
    let again = vm.register(&service.id.to_string()).await;
    assert_eq!(again.id.0, registration.id.0);
    assert_eq!(again.state, ServerRegistrationState::Accepted);
}

/// The instance, project and silo of a registration come from the attestation.
#[tokio::test]
async fn test_registration_uses_attested_identity() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("registration_attested_identity")
        .await
        .unwrap();
    let _handle = seed.server().run();
    let (_user, services) = user_with_services(&seed, &["test"]).await;

    let mut conf = vm_config();
    conf.project = Uuid::new_v4();
    conf.silo = Uuid::new_v4();
    let vm = seed.vm(conf.clone());
    let registration = vm.register(&services[0].id.to_string()).await;

    assert_eq!(registration.service_id.0, services[0].id.0);
    assert_eq!(registration.instance_id.0, conf.uuid);
    assert_eq!(registration.project_id.0, conf.project);
    assert_eq!(registration.silo_id.0, conf.silo);
}

/// A challenge can only be answered once.
#[tokio::test]
async fn test_registration_challenge_is_single_use() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("registration_single_use")
        .await
        .unwrap();
    let _handle = seed.server().run();
    let (_user, services) = user_with_services(&seed, &["test"]).await;

    let vm = seed.vm(vm_config());
    let challenge = vm.challenge(&services[0].id.to_string()).await;
    let attestation = vm.attest(&challenge).await;
    vm.prove(&challenge, attestation).await.unwrap();

    let attestation = vm.attest(&challenge).await;
    let err = vm.prove(&challenge, attestation).await.unwrap_err();
    assert_eq!(err.status(), Some(StatusCode::NOT_FOUND));
}

/// An attestation without an instance log does not prove any identity.
#[tokio::test]
async fn test_registration_requires_instance_log() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("registration_instance_log")
        .await
        .unwrap();
    let _handle = seed.server().run();
    let (_user, services) = user_with_services(&seed, &["test"]).await;

    let vm = seed.vm(vm_config());
    let challenge = vm.challenge(&services[0].id.to_string()).await;
    let mut attestation = vm.attest(&challenge).await;
    attestation
        .measurement_logs
        .retain(|log| log.rot != "OxideInstance");
    let err = vm.prove(&challenge, attestation).await.unwrap_err();
    assert_eq!(err.status(), Some(StatusCode::BAD_REQUEST));
}

/// An instance that is registered to one service can not be moved to another
/// service by answering a challenge for it.
#[tokio::test]
async fn test_registration_conflicts_with_other_service() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("registration_conflict")
        .await
        .unwrap();
    let _handle = seed.server().run();
    let (_user, services) = user_with_services(&seed, &["first", "second"]).await;

    let vm = seed.vm(vm_config());
    vm.register(&services[0].id.to_string()).await;

    let challenge = vm.challenge(&services[1].id.to_string()).await;
    let attestation = vm.attest(&challenge).await;
    let err = vm.prove(&challenge, attestation).await.unwrap_err();
    assert_eq!(err.status(), Some(StatusCode::CONFLICT));
}

async fn accepted_vm(
    seed: &SeededContext,
    user: &MockUser,
    service: &Service,
) -> (common::actors::MockVm, ServerRegistration) {
    let mut vm = seed.vm(vm_config());
    let registration = vm.register(&service.id.to_string()).await;
    user.client
        .accept_server()
        .server(registration.id.clone())
        .send()
        .await
        .unwrap();
    vm.authenticate(&registration.id).await;
    vm.client
        .checkin_server()
        .server(registration.id.clone())
        .body_map(|body| body.checked_in_at(chrono::Utc::now()))
        .send()
        .await
        .unwrap();
    (vm, registration)
}

/// A terminated server loses access with its existing token, and terminated
/// is final so registering again returns the terminated registration.
#[tokio::test]
async fn test_terminated_server_loses_access() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("registration_terminated")
        .await
        .unwrap();
    let _handle = seed.server().run();
    let (user, services) = user_with_services(&seed, &["test"]).await;
    let (vm, registration) = accepted_vm(&seed, &user, &services[0]).await;

    user.client
        .terminate_server()
        .server(registration.id.clone())
        .send()
        .await
        .unwrap();

    let err = vm
        .client
        .checkin_server()
        .server(registration.id.clone())
        .body_map(|body| body.checked_in_at(chrono::Utc::now()))
        .send()
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(StatusCode::FORBIDDEN));

    let again = vm.register(&services[0].id.to_string()).await;
    assert_eq!(again.id.0, registration.id.0);
    assert_eq!(again.state, ServerRegistrationState::Terminated);

    // Deleting the terminated registration lets the instance register again
    user.client
        .delete_server()
        .server(registration.id.clone())
        .send()
        .await
        .unwrap();
    let fresh = vm.register(&services[0].id.to_string()).await;
    assert_ne!(fresh.id.0, registration.id.0);
    assert_eq!(fresh.state, ServerRegistrationState::Proven);
}

/// A deleted server loses access with its existing token and can register
/// again as a new registration.
#[tokio::test]
async fn test_deleted_server_loses_access() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("registration_deleted").await.unwrap();
    let _handle = seed.server().run();
    let (user, services) = user_with_services(&seed, &["test"]).await;
    let service = services[0].id.to_string();
    let (vm, registration) = accepted_vm(&seed, &user, &services[0]).await;

    user.client
        .delete_server()
        .server(registration.id.clone())
        .send()
        .await
        .unwrap();

    let err = vm
        .client
        .checkin_server()
        .server(registration.id.clone())
        .body_map(|body| body.checked_in_at(chrono::Utc::now()))
        .send()
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(StatusCode::FORBIDDEN));

    let again = vm.register(&service).await;
    assert_ne!(again.id.0, registration.id.0);
    assert_eq!(again.state, ServerRegistrationState::Proven);
}

/// A service can only be deleted once all of its servers are terminated.
#[tokio::test]
async fn test_delete_service_requires_terminated_servers() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("delete_service").await.unwrap();
    let _handle = seed.server().run();
    let (user, services) = user_with_services(&seed, &["test"]).await;
    let service = services[0].id.to_string();

    let vm = seed.vm(vm_config());
    let registration = vm.register(&service).await;

    let err = user
        .client
        .delete_service()
        .service(service.clone())
        .send()
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(StatusCode::BAD_REQUEST));

    user.client
        .terminate_server()
        .server(registration.id.clone())
        .send()
        .await
        .unwrap();
    user.client
        .delete_service()
        .service(service.clone())
        .send()
        .await
        .unwrap();

    let err = user
        .client
        .get_service()
        .service(service)
        .send()
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(StatusCode::NOT_FOUND));
}

fn claims(token: &str) -> VmClaims {
    jsonwebtoken::dangerous::insecure_decode::<VmClaims>(token)
        .unwrap()
        .claims
}

/// A token carries the name of its service and is issued for sprue itself
/// unless the server requests another audience.
#[tokio::test]
async fn test_token_audience_and_service_name() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("token_audience").await.unwrap();
    let _handle = seed.server().run();
    let (user, services) = user_with_services(&seed, &["test"]).await;
    let (mut vm, registration) = accepted_vm(&seed, &user, &services[0]).await;

    let default = claims(&vm.token(&registration.id, None).await);
    assert_eq!(default.aud, default.iss);
    assert_eq!(default.svn, "test");
    assert_eq!(*default.svc.as_untyped_uuid(), services[0].id.0);

    let audience = "https://relying-party.example.com";
    let token = vm.token(&registration.id, Some(audience)).await;
    let custom = claims(&token);
    assert_eq!(custom.aud, audience);
    assert_eq!(custom.svn, "test");

    // Sprue only accepts tokens that were issued for sprue
    vm.install_token(&token);
    let err = vm
        .client
        .checkin_server()
        .server(registration.id.clone())
        .body_map(|body| body.checked_in_at(chrono::Utc::now()))
        .send()
        .await
        .unwrap_err();
    assert_eq!(err.status(), Some(StatusCode::UNAUTHORIZED));
}

/// An audience that is empty or holds whitespace is refused.
#[tokio::test]
async fn test_token_audience_format() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("token_audience_format").await.unwrap();
    let _handle = seed.server().run();
    let (user, services) = user_with_services(&seed, &["test"]).await;
    let (vm, registration) = accepted_vm(&seed, &user, &services[0]).await;

    for audience in ["", "has space", "line\nbreak"] {
        let response = reqwest::Client::new()
            .post(format!(
                "{}/server/{}/oidc/token",
                vm.client.baseurl(),
                registration.id.0
            ))
            .json(&serde_json::json!({ "audience": audience }))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{audience:?}");
    }
}

/// The audience of a token is recorded on its request.
#[tokio::test]
async fn test_token_request_records_audience() {
    let _ = tracing_subscriber::fmt::try_init();
    let seed = SeededContext::create("token_audience_recorded").await.unwrap();
    let _handle = seed.server().run();
    let (user, services) = user_with_services(&seed, &["test"]).await;
    let (vm, registration) = accepted_vm(&seed, &user, &services[0]).await;

    let audience = "sts.amazonaws.com";
    let request = vm
        .client
        .register_oidc_token_request()
        .server(registration.id.clone())
        .body_map(|body| body.audience(Some(audience.try_into().unwrap())))
        .send()
        .await
        .unwrap()
        .into_inner();
    assert_eq!(request.audience.as_deref(), Some(audience));

    let default = vm
        .client
        .register_oidc_token_request()
        .server(registration.id.clone())
        .body_map(|body| body.audience(None))
        .send()
        .await
        .unwrap()
        .into_inner();
    let token = vm.token(&registration.id, None).await;
    assert_eq!(default.audience, Some(claims(&token).iss));
}
