// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use dropshot::{ApiDescription, ServerBuilder};
use slog::Logger;
use std::net::SocketAddr;

use crate::{
    context::ApiContext,
    endpoints::{
        blob::{cancel_blob_upload, complete_blob_upload, reset_blob_upload, write_blob_upload},
        oidc::{prove_oidc_token_request, register_oidc_token_request},
        service::{
            accept_server, checkin_server, create_deployment, create_service, delete_deployment,
            get_deployment, get_service, get_service_servers, list_deployments, list_services,
            prove_server, register_blob, register_server, reject_server, terminate_server,
        },
    },
    permissions::ApiPermissions,
};

pub fn create_server(ctx: ApiContext, logger: Logger, port: u16) -> ServerBuilder<ApiContext> {
    let description = describe();

    let server =
        dropshot::ServerBuilder::new(description, ctx, logger).config(dropshot::ConfigDropshot {
            default_request_body_max_bytes: 10 * 1024 * 1024,
            bind_address: SocketAddr::from(([0, 0, 0, 0], port)),
            ..Default::default()
        });
    server
}

v_api::v_system_endpoints!(ApiContext, ApiPermissions);

pub fn describe() -> ApiDescription<ApiContext> {
    let mut description = ApiDescription::new();

    v_api::inject_endpoints!(description);

    description
        .register(register_server)
        .expect("Register endpoint");

    description
        .register(get_service)
        .expect("Register endpoint");
    description
        .register(list_services)
        .expect("Register endpoint");
    description
        .register(get_service_servers)
        .expect("Register endpoint");
    description
        .register(create_service)
        .expect("Register endpoint");
    description
        .register(accept_server)
        .expect("Register endpoint");
    description
        .register(prove_server)
        .expect("Register endpoint");
    description
        .register(reject_server)
        .expect("Register endpoint");
    description
        .register(terminate_server)
        .expect("Register endpoint");
    description
        .register(checkin_server)
        .expect("Register endpoint");
    description
        .register(register_blob)
        .expect("Register endpoint");

    description
        .register(write_blob_upload)
        .expect("Register endpoint");
    description
        .register(reset_blob_upload)
        .expect("Register endpoint");
    description
        .register(complete_blob_upload)
        .expect("Register endpoint");
    description
        .register(cancel_blob_upload)
        .expect("Register endpoint");

    description
        .register(register_oidc_token_request)
        .expect("Register endpoint");
    description
        .register(prove_oidc_token_request)
        .expect("Register endpoint");

    description
        .register(create_deployment)
        .expect("Register endpoint");
    description
        .register(list_deployments)
        .expect("Register endpoint");
    description
        .register(get_deployment)
        .expect("Register endpoint");
    description
        .register(delete_deployment)
        .expect("Register endpoint");

    description
}
