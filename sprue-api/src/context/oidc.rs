// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use chrono::Utc;
use newtype_uuid::TypedUuid;
use serde::{Deserialize, Serialize};
use sprue_model::{
    InvalidTokenRequestStateTransition, ProjectId, ServerRegistration, ServerRegistrationId,
    ServerRegistrationInstanceId, Service, ServiceId, SiloId, TokenRequest, TokenRequestId,
    db::NewTokenRequestModel,
    storage::{StorageError, TokenRequestStorage},
};
use std::{ops::Add, sync::Arc, time::Duration};
use thiserror::Error;
use uuid::Uuid;
use v_api::response::{OptionalResource, ResourceError, ResourceErrorInner, ResourceResult};
use v_api_param::ParamResolutionError;

use crate::{config::OidcConfig, schema::Audience};

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
    issuer: String,
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
    pub svc: TypedUuid<ServiceId>,
    /// The name of the service that the server is registered to.
    pub svn: String,
    pub oxc: OxideVmClaims,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct OxideVmClaims {
    pub ins: TypedUuid<ServerRegistrationInstanceId>,
    pub prj: TypedUuid<ProjectId>,
    pub slo: TypedUuid<SiloId>,
}

impl OidcContext {
    pub fn new(
        issuer: String,
        oidc: OidcConfig,
        storage: Arc<dyn OidcContextStorage>,
    ) -> Result<Self, OidcContextError> {
        Ok(Self {
            issuer,
            storage,
            oidc: Arc::new(oidc),
        })
    }

    /// Register a token request. The audience of the token is recorded on the
    /// request and defaults to this issuer when the server does not ask for
    /// one.
    pub async fn register_token_request(
        &self,
        server: ServerRegistration,
        nonce: String,
        audience: Option<Audience>,
    ) -> ResourceResult<TokenRequest, OidcContextError> {
        Ok(self
            .storage
            .create_token_request(&NewTokenRequestModel {
                server_registration_id: server.id,
                nonce: Some(nonce),
                expires_at: Some(
                    Utc::now().add(Duration::from_secs(self.oidc.token.token_request_duration)),
                ),
                audience: Some(
                    audience
                        .map(String::from)
                        .unwrap_or_else(|| self.issuer.clone()),
                ),
            })
            .await
            .map_err(ResourceError::InternalError)
            .inner_err_into()?
            .into())
    }

    /// Get a token request that was registered by the given server. A request
    /// that belongs to a different server is treated as missing.
    pub async fn get_token_request(
        &self,
        server: TypedUuid<ServerRegistrationId>,
        id: TypedUuid<TokenRequestId>,
    ) -> ResourceResult<TokenRequest, OidcContextError> {
        Ok(self
            .storage
            .get_token_request(id)
            .await
            .map(|request| request.filter(|r| r.server_registration_id == server))
            .optional()?
            .into())
    }

    /// Consume the token request and build the claims for the server. The
    /// token carries the audience that was recorded on the request.
    pub async fn generate_claims(
        &self,
        server: &ServerRegistration,
        service: &Service,
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
        Ok(self.create_claims(server, service, token_request.audience))
    }

    // A request without a recorded audience was made before audiences were
    // recorded, and those requests were always issued for this issuer.
    fn create_claims(
        &self,
        server: &ServerRegistration,
        service: &Service,
        audience: Option<String>,
    ) -> VmClaims {
        VmClaims {
            iss: self.issuer.to_string(),
            aud: audience.unwrap_or_else(|| self.issuer.clone()),
            sub: server.id,
            exp: Utc::now().timestamp() + (self.oidc.token.token_lifetime as i64),
            nbf: Utc::now().timestamp(),
            jti: Uuid::new_v4(),
            svc: server.service_id,
            svn: service.name.clone(),
            oxc: OxideVmClaims {
                ins: server.instance_id,
                prj: server.project_id,
                slo: server.silo_id,
            },
        }
    }
}
