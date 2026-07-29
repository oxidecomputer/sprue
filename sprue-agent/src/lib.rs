// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
use serde::{Deserialize, Serialize};
use sprue_sdk::types::TypedUuidForServerRegistrationId;
use std::path::PathBuf;
use thiserror::Error;

pub static DEFAULT_SPRUE_SOCKET: &str = "unix:/var/run/sprue.sock";

#[derive(Error, Debug, Deserialize, Serialize)]
pub enum SprueError {
    #[error("Sprue service failure: {0}")]
    Failure(String),
}

#[tarpc::service]
pub trait SprueService {
    async fn checkin() -> Result<(), SprueError>;
    async fn backup(path: PathBuf) -> Result<String, SprueError>;
    async fn get_token() -> Result<String, SprueError>;
    async fn get_registration_id() -> TypedUuidForServerRegistrationId;
    async fn register_server() -> Result<TypedUuidForServerRegistrationId, SprueError>;
}
