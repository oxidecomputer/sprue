use aws_credential_types::{Credentials, provider::ProvideCredentials};
use std::fmt::Debug;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use thiserror::Error;
use tokio::sync::RwLock;

use crate::backup_storage::OidcTokenFetcher;

#[derive(Debug, Error)]
pub enum WebIdentityCredentialProviderError {
    #[error("Invalid expiration time")]
    Expiry(#[from] aws_smithy_types::date_time::ConversionError),
    #[error("Failed to fetch internal token")]
    InternalToken,
    #[error("No token returned by AWS")]
    NoTokenReturned,
    #[error("AWS STS error")]
    Sts(#[from] aws_sdk_sts::Error),
}

#[derive(Clone)]
struct AwsCachedCredentials {
    credentials: Credentials,
    expiry: SystemTime,
}

#[derive(Clone)]
pub struct WebIdentityCredentialProvider {
    role_arn: String,
    session_name: String,
    token_fetcher: OidcTokenFetcher,
    cache: Arc<RwLock<Option<AwsCachedCredentials>>>,
    refresh_buffer: Duration,
}

impl Debug for WebIdentityCredentialProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebIdentityCredentialProvider")
            .field("role_arn", &self.role_arn)
            .field("session_name", &self.session_name)
            .field("refresh_buffer", &self.refresh_buffer)
            .finish()
    }
}

impl WebIdentityCredentialProvider {
    pub fn new(
        role_arn: String,
        session_name: String,
        refresh_buffer: Duration,
        token_fetcher: OidcTokenFetcher,
    ) -> Self {
        Self {
            role_arn: role_arn.into(),
            session_name: session_name.into(),
            token_fetcher,
            cache: Arc::new(RwLock::new(None)),
            refresh_buffer: refresh_buffer,
        }
    }

    async fn fetch_fresh_credentials(
        &self,
    ) -> Result<AwsCachedCredentials, WebIdentityCredentialProviderError> {
        let token = (self.token_fetcher)("sts.amazonaws.com".to_string())
            .await
            .map_err(|err| {
                tracing::error!(?err, "AWS client failed to retrieve internal service token");
                WebIdentityCredentialProviderError::InternalToken
            })?;
        let sts_config = aws_config::from_env().no_credentials().load().await;
        let sts_client = aws_sdk_sts::Client::new(&sts_config);

        let resp = sts_client
            .assume_role_with_web_identity()
            .role_arn(&self.role_arn)
            .role_session_name(&self.session_name)
            .web_identity_token(token)
            .send()
            .await
            .map_err(|err| aws_sdk_sts::Error::from(err))?;

        let creds = resp
            .credentials()
            .ok_or(WebIdentityCredentialProviderError::NoTokenReturned)?;
        let expiry: SystemTime = creds.expiration().clone().try_into()?;

        let credentials = Credentials::new(
            creds.access_key_id(),
            creds.secret_access_key(),
            Some(creds.session_token().to_string()),
            Some(expiry),
            env!("CARGO_PKG_NAME"),
        );

        Ok(AwsCachedCredentials {
            credentials,
            expiry,
        })
    }

    fn is_expired(cached: &AwsCachedCredentials, buffer: Duration) -> bool {
        let refresh_at = cached.expiry - buffer;
        SystemTime::now() >= refresh_at
    }
}

impl ProvideCredentials for WebIdentityCredentialProvider {
    fn provide_credentials<'a>(
        &'a self,
    ) -> aws_credential_types::provider::future::ProvideCredentials<'a>
    where
        Self: 'a,
    {
        aws_credential_types::provider::future::ProvideCredentials::new(async move {
            {
                let cache = self.cache.read().await;
                if let Some(cached) = cache.as_ref() {
                    if !Self::is_expired(cached, self.refresh_buffer) {
                        return Ok(cached.credentials.clone());
                    }
                }
            }

            let fresh = self.fetch_fresh_credentials().await.map_err(|e| {
                aws_credential_types::provider::error::CredentialsError::provider_error(e)
            })?;

            let credentials = fresh.credentials.clone();
            *self.cache.write().await = Some(fresh);

            Ok(credentials)
        })
    }
}

pub async fn build_s3_client(
    region: String,
    role_arn: String,
    session_name: String,
    refresh_buffer: Duration,
    token_fetcher: OidcTokenFetcher,
) -> aws_sdk_s3::Client {
    let provider =
        WebIdentityCredentialProvider::new(role_arn, session_name, refresh_buffer, token_fetcher);

    let config = aws_config::from_env()
        .region(aws_config::Region::new(region.to_string()))
        .credentials_provider(provider)
        .load()
        .await;

    aws_sdk_s3::Client::new(&config)
}
