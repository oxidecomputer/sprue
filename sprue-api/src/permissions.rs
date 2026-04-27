// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use newtype_uuid::TypedUuid;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sprue_model::{BlobId, ServiceId};
use std::collections::BTreeSet;
use v_api::permissions::VPermission;
use v_api_permission_derive::v_api;

#[v_api(From(VPermission))]
#[derive(Debug, Clone, Hash, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
pub enum ApiPermissions {
    #[v_api(
        contract(kind = append, variant = GetServices),
        scope(to = "service:r")
    )]
    GetService(TypedUuid<ServiceId>),
    #[v_api(
        contract(kind = extend, variant = GetServices),
        expand(kind = iter, variant = GetService),
        scope(to = "service:r")
    )]
    GetServices(BTreeSet<TypedUuid<ServiceId>>),
    #[v_api(
        expand(kind = alias, variant = GetService, source = actor),
        scope(to = "service:r", from = "service:r")
    )]
    GetServicesAssigned,
    #[v_api(scope(to = "service:r", from = "service:r"))]
    GetServicesAll,

    #[v_api(scope(to = "service:w", from = "service:w"))]
    CreateService,
    #[v_api(
        contract(kind = append, variant = ManageServices),
        scope(to = "service:w")
    )]
    ManageService(TypedUuid<ServiceId>),
    #[v_api(
        contract(kind = extend, variant = ManageServices),
        expand(kind = iter, variant = ManageService),
        scope(to = "service:w")
    )]
    ManageServices(BTreeSet<TypedUuid<ServiceId>>),
    #[v_api(
        expand(kind = alias, variant = ManageService, source = actor),
        scope(to = "service:w", from = "service:w")
    )]
    ManageServicesAssigned,
    #[v_api(scope(to = "service:w", from = "service:w"))]
    ManageServicesAll,

    #[v_api(
        contract(kind = append, variant = GetBlobs),
        scope(to = "blob:r")
    )]
    GetBlob(TypedUuid<BlobId>),
    #[v_api(
        contract(kind = extend, variant = GetBlobs),
        expand(kind = iter, variant = GetBlob),
        scope(to = "blob:r")
    )]
    GetBlobs(BTreeSet<TypedUuid<BlobId>>),
    #[v_api(
        expand(kind = alias, variant = GetBlob, source = actor),
        scope(to = "blob:r", from = "blob:r")
    )]
    GetBlobsAssigned,
    #[v_api(scope(to = "blob:r", from = "blob:r"))]
    GetBlobsAll,
}
