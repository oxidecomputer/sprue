use dropshot::{HttpError, HttpResponseOk, RequestContext, endpoint};
use jsonwebtoken::jwk::{AlgorithmParameters, JwkSet, PublicKeyUse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::context::ApiContext;

pub mod blob;
pub mod oidc;
pub mod service;

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct OpenIdConfiguration {
    jwks_uri: String,
}

#[endpoint {
    method = GET,
    path = "/.well-known/openid-configuration",
}]
pub async fn openid_configuration(
    rqctx: RequestContext<ApiContext>,
) -> Result<HttpResponseOk<OpenIdConfiguration>, HttpError> {
    Ok(HttpResponseOk(OpenIdConfiguration {
        jwks_uri: format!("{}/.well-known/jwks.json", rqctx.context().public_url),
    }))
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Jwks {
    keys: Vec<Jwk>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
pub struct Jwk {
    kty: String,
    kid: String,
    #[serde(rename = "use")]
    use_: String,
    n: String,
    e: String,
}

#[endpoint {
    method = GET,
    path = "/.well-known/jwks.json",
}]
pub async fn jwks_json(
    rqctx: RequestContext<ApiContext>,
) -> Result<HttpResponseOk<Jwks>, HttpError> {
    Ok(HttpResponseOk((&rqctx.context().oidc.jwks).into()))
}

impl From<&JwkSet> for Jwks {
    fn from(value: &JwkSet) -> Self {
        Self {
            keys: value
                .keys
                .iter()
                .map(|jwk| {
                    let (algo, n, e) = match &jwk.algorithm {
                        AlgorithmParameters::RSA(params) => {
                            ("RSA".to_string(), params.n.clone(), params.e.clone())
                        }
                        _ => panic!("Unexpected key type"),
                    };

                    Jwk {
                        kty: algo,
                        kid: jwk.common.key_id.as_ref().unwrap().clone(),
                        use_: match jwk.common.public_key_use {
                            Some(PublicKeyUse::Signature) => "sig".to_string(),
                            _ => panic!("Unexpected key use"),
                        },
                        n,
                        e,
                    }
                })
                .collect(),
        }
    }
}
