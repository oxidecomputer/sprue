// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD};
use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};
use serde::Deserialize;
use sprue_sdk::types::{Jwk, Jwks};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum JwtValidationError {
    #[error("Failed to decode JWT header")]
    DecodeHeader(#[source] jsonwebtoken::errors::Error),
    #[error("Failed to decode JWT")]
    DecodeJwt(#[source] jsonwebtoken::errors::Error),
    #[error("JWT header missing key id (kid)")]
    MissingKid,
    #[error("No matching key found in JWKS for kid: {0}")]
    NoMatchingKey(String),
    #[error("Invalid key type: expected RSA, got {0}")]
    InvalidKeyType(String),
    #[error("Failed to decode base64 modulus (n)")]
    DecodeModulus(#[source] base64::DecodeError),
    #[error("Failed to decode base64 exponent (e)")]
    DecodeExponent(#[source] base64::DecodeError),
}

/// Claims expected in the VM attestation JWT
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct VmClaims {
    pub iss: String,
    pub aud: String,
    pub sub: Uuid,
    pub exp: i64,
    pub nbf: i64,
    pub jti: Uuid,
}

/// Find a JWK in the JWKS that matches the given key id
fn find_key_by_kid<'a>(jwks: &'a Jwks, kid: &str) -> Option<&'a Jwk> {
    jwks.keys.iter().find(|key| key.kid == kid)
}

/// Construct a DecodingKey from a JWK
fn decoding_key_from_jwk(jwk: &Jwk) -> Result<DecodingKey, JwtValidationError> {
    // Verify the key type is RSA
    if jwk.kty != "RSA" {
        return Err(JwtValidationError::InvalidKeyType(jwk.kty.clone()));
    }

    // Decode the modulus and exponent from base64url
    let n = BASE64_URL_SAFE_NO_PAD
        .decode(&jwk.n)
        .map_err(JwtValidationError::DecodeModulus)?;
    let e = BASE64_URL_SAFE_NO_PAD
        .decode(&jwk.e)
        .map_err(JwtValidationError::DecodeExponent)?;

    // Construct the decoding key from RSA components
    Ok(DecodingKey::from_rsa_raw_components(&n, &e))
}

/// Validate a JWT against a JWKS
///
/// This function:
/// 1. Extracts the key id (kid) from the JWT header
/// 2. Finds the matching key in the JWKS
/// 3. Validates the JWT signature and claims using that key
///
/// Returns the validated claims on success
pub fn validate_jwt(
    issuer: &str,
    token: &str,
    jwks: &Jwks,
) -> Result<VmClaims, JwtValidationError> {
    // Decode the JWT header to get the key id
    let header = decode_header(token).map_err(JwtValidationError::DecodeHeader)?;

    let kid = header.kid.ok_or(JwtValidationError::MissingKid)?;

    // Find the matching key in the JWKS
    let jwk = find_key_by_kid(jwks, &kid)
        .ok_or_else(|| JwtValidationError::NoMatchingKey(kid.clone()))?;

    // Construct the decoding key from the JWK
    let decoding_key = decoding_key_from_jwk(jwk)?;

    // Set up validation parameters
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);

    // We expect the audience to match the issuer
    validation.set_audience(&[issuer]);
    validation.validate_aud = true;

    // Decode and validate the JWT
    let token_data = decode::<VmClaims>(token, &decoding_key, &validation)
        .map_err(JwtValidationError::DecodeJwt)?;

    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_key_by_kid() {
        let jwks = Jwks {
            keys: vec![
                Jwk {
                    kid: "key1".to_string(),
                    kty: "RSA".to_string(),
                    n: "test_n".to_string(),
                    e: "AQAB".to_string(),
                    use_: "sig".to_string(),
                },
                Jwk {
                    kid: "key2".to_string(),
                    kty: "RSA".to_string(),
                    n: "test_n2".to_string(),
                    e: "AQAB".to_string(),
                    use_: "sig".to_string(),
                },
            ],
        };

        assert!(find_key_by_kid(&jwks, "key1").is_some());
        assert!(find_key_by_kid(&jwks, "key2").is_some());
        assert!(find_key_by_kid(&jwks, "key3").is_none());
    }
}
