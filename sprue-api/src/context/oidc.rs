use chrono::Utc;
use newtype_uuid::TypedUuid;
use serde::{Deserialize, Serialize};
use sprue_model::{
    InvalidTokenRequestStateTransition, ServerRegistration, ServerRegistrationId,
    ServerRegistrationInstanceId, ServiceId, TokenRequest, TokenRequestId,
    db::NewTokenRequestModel,
    storage::{StorageError, TokenRequestStorage},
};
use std::{ops::Add, sync::Arc, time::Duration};
use thiserror::Error;
use uuid::Uuid;
use v_api::response::{OptionalResource, ResourceError, ResourceErrorInner, ResourceResult};
use v_api_param::ParamResolutionError;

use crate::config::OidcConfig;

#[derive(Debug, Error)]
pub enum OidcContextError {
    #[error("Failed to decode JWT")]
    DecodeJwt(#[source] jsonwebtoken::errors::Error),
    #[error("Failed to encode JWT")]
    EncodeJwt(#[source] jsonwebtoken::errors::Error),
    #[error("Failed to decode RSA public key")]
    InvalidKey(#[source] x509_cert::spki::Error),
    #[error("Failed to decode pem")]
    JwtKey(#[source] jsonwebtoken::errors::Error),
    #[error("Failed to resolve parameter")]
    Param(#[from] ParamResolutionError),
    #[error("Invalid state transition for token request")]
    TokenRequestState(#[from] InvalidTokenRequestStateTransition),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error("Vm uuid does not match the expected value")]
    UnexpectedVmId,
}

pub trait OidcContextStorage: TokenRequestStorage {}
impl<T: TokenRequestStorage> OidcContextStorage for T {}

#[derive(Clone)]
pub struct OidcContext {
    storage: Arc<dyn OidcContextStorage>,
    oidc: Arc<OidcConfig>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VmClaims {
    pub iss: String,
    pub aud: String,
    pub sub: TypedUuid<ServerRegistrationId>,
    pub exp: i64,
    pub nbf: i64,
    pub jti: Uuid,
    pub srv: TypedUuid<ServiceId>,
    pub ox: OxideVmClaims,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OxideVmClaims {
    pub ins: TypedUuid<ServerRegistrationInstanceId>,
    pub prj: Option<Uuid>,
    pub slo: Option<Uuid>,
}

impl OidcContext {
    pub fn new(
        oidc: OidcConfig,
        storage: Arc<dyn OidcContextStorage>,
    ) -> Result<Self, OidcContextError> {
        Ok(Self {
            storage,
            oidc: Arc::new(oidc),
        })
    }

    pub async fn register_token_request(
        &self,
        server: ServerRegistration,
        nonce: String,
    ) -> ResourceResult<TokenRequest, OidcContextError> {
        Ok(self
            .storage
            .create_token_request(&NewTokenRequestModel {
                server_registration_id: server.id,
                nonce: Some(nonce),
                expires_at: Some(
                    Utc::now().add(Duration::from_secs(self.oidc.token.token_request_duration)),
                ),
            })
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?
            .into())
    }

    pub async fn get_token_request(
        &self,
        id: TypedUuid<TokenRequestId>,
    ) -> ResourceResult<TokenRequest, OidcContextError> {
        Ok(self.storage.get_token_request(id).await.optional()?.into())
    }

    pub async fn generate_claims(
        &self,
        server: &ServerRegistration,
        token_request: TokenRequest,
    ) -> ResourceResult<VmClaims, OidcContextError> {
        self.storage
            .update_token_request_state(
                token_request.id,
                token_request.state,
                token_request
                    .state
                    .consume()
                    .map_err(ResourceError::InternalError)
                    .inner_err_into()?,
            )
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?;
        Ok(self.create_claims(server))
    }

    fn create_claims(&self, server: &ServerRegistration) -> VmClaims {
        let claims = VmClaims {
            iss: self.oidc.token.issuer.to_string(),
            aud: self.oidc.token.audience.to_string(),
            sub: server.id,
            exp: Utc::now().timestamp() + (self.oidc.token.token_lifetime as i64),
            nbf: Utc::now().timestamp(),
            jti: Uuid::new_v4(),
            srv: server.service_id,
            ox: OxideVmClaims {
                ins: server.instance_id,
                prj: None,
                slo: None,
            },
        };

        claims
    }
}
