use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use sprue_sdk::types::TypedUuidForServerRegistrationId;
use thiserror::Error;

pub static DEFAULT_SPRUE_SOCKET: &str = "unix:/var/run/sprue.sock";

#[derive(Error, Debug, Deserialize, Serialize)]
pub enum SprueError {
    #[error("Sprue service failure: {0}")]
    Failure(String),
}

#[tarpc::service]
pub trait SprueService {
    async fn backup(path: PathBuf) -> Result<String, SprueError>;
    async fn get_token() -> Result<String, SprueError>;
    async fn get_registration_id() -> TypedUuidForServerRegistrationId;
    async fn register_server() -> Result<TypedUuidForServerRegistrationId, SprueError>;
}
