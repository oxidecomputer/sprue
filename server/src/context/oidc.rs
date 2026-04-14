use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD as URL_SAFE_NO_PAD};
use chrono::Utc;
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation,
    jwk::{
        AlgorithmParameters, CommonParameters, Jwk, JwkSet, KeyAlgorithm, PublicKeyUse,
        RSAKeyParameters, RSAKeyType,
    },
};
use model::{
    InvalidTokenRequestStateTransition, ServerRegistration, ServerRegistrationInstanceId,
    TokenRequest, TokenRequestId,
    db::NewTokenRequestModel,
    storage::{StorageError, TokenRequestStorage},
};
use newtype_uuid::TypedUuid;
use rsa::{RsaPublicKey, pkcs8::DecodePublicKey, traits::PublicKeyParts};
use serde::{Deserialize, Serialize};
use std::{ops::Add, sync::Arc, time::Duration};
use thiserror::Error;
use uuid::Uuid;
use v_api::response::{OptionalResource, ResourceError, ResourceErrorInner, ResourceResult};

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
    jwks: JwkSet,
    signing_key: EncodingKey,
    verifying_key: DecodingKey,
    validation: Validation,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct VmClaims {
    pub iss: String,
    pub aud: String,
    pub sub: TypedUuid<ServerRegistrationInstanceId>,
    pub exp: i64,
    pub nbf: i64,
    pub jti: Uuid,
}

impl OidcContext {
    pub fn new(
        oidc: OidcConfig,
        storage: Arc<dyn OidcContextStorage>,
    ) -> Result<Self, OidcContextError> {
        let jwks = JwkSet {
            keys: vec![Self::jwk(&oidc.kid, &oidc.public)?],
        };
        let mut validation = Validation::new(Algorithm::RS256);
        validation.set_audience(&[&oidc.token.audience]);
        Ok(Self {
            storage,
            jwks,
            signing_key: EncodingKey::from_rsa_pem(oidc.private.as_bytes())
                .map_err(OidcContextError::JwtKey)?,
            verifying_key: DecodingKey::from_rsa_pem(oidc.public.as_bytes())
                .map_err(OidcContextError::JwtKey)?,
            oidc: Arc::new(oidc),
            validation,
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
                expires_at: Some(Utc::now().add(Duration::from_secs(
                    self.oidc.token.max_token_request_duration,
                ))),
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

    pub async fn generate_token(
        &self,
        server: &ServerRegistration,
        token_request: TokenRequest,
    ) -> ResourceResult<String, OidcContextError> {
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
        self.create_jwt(server.instance_id)
    }

    pub fn validate_token(
        &self,
        token: &str,
    ) -> Result<TypedUuid<ServerRegistrationInstanceId>, OidcContextError> {
        let claims = jsonwebtoken::decode::<VmClaims>(token, &self.verifying_key, &self.validation)
            .map_err(|err| OidcContextError::DecodeJwt(err))?;
        Ok(claims.claims.sub)
    }

    fn create_jwt(
        &self,
        server: TypedUuid<ServerRegistrationInstanceId>,
    ) -> ResourceResult<String, OidcContextError> {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.oidc.kid.clone());

        let claims = VmClaims {
            iss: self.oidc.token.issuer.to_string(),
            aud: self.oidc.token.audience.to_string(),
            sub: server,
            exp: Utc::now().timestamp() + (self.oidc.token.token_lifetime as i64),
            nbf: Utc::now().timestamp(),
            jti: Uuid::new_v4(),
        };

        Ok(jsonwebtoken::encode(&header, &claims, &self.signing_key)
            .map_err(OidcContextError::EncodeJwt)
            .map_err(ResourceError::InternalError)?)
    }

    pub fn jwks(&self) -> &JwkSet {
        &self.jwks
    }

    fn jwk(kid: &str, public_key_pem: &str) -> Result<Jwk, OidcContextError> {
        let public_key = RsaPublicKey::from_public_key_pem(public_key_pem)
            .map_err(OidcContextError::InvalidKey)?;

        Ok(Jwk {
            common: CommonParameters {
                public_key_use: Some(PublicKeyUse::Signature),
                key_operations: None,
                key_algorithm: Some(KeyAlgorithm::RS256),
                key_id: Some(kid.to_string()),
                x509_chain: None,
                x509_sha1_fingerprint: None,
                x509_sha256_fingerprint: None,
                x509_url: None,
            },
            algorithm: AlgorithmParameters::RSA(RSAKeyParameters {
                key_type: RSAKeyType::RSA,
                n: URL_SAFE_NO_PAD.encode(public_key.n().to_bytes_be()),
                e: URL_SAFE_NO_PAD.encode(public_key.e().to_bytes_be()),
            }),
        })
    }
}
