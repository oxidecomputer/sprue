use std::{
    collections::HashMap,
    str::Utf8Error,
    sync::{Arc, RwLock},
};

use attest_data::{Attestation, Log};
use base64::{Engine, prelude::BASE64_URL_SAFE_NO_PAD as URL_SAFE_NO_PAD};
use chrono::Utc;
use dice_verifier::{
    MeasurementSet, MeasurementSetError, Nonce, PkiPathSignatureVerifierError,
    ReferenceMeasurements, VerifyAttestationError, VerifyMeasurementsError,
};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header,
    jwk::{
        AlgorithmParameters, CommonParameters, Jwk, JwkSet, KeyAlgorithm, PublicKeyUse,
        RSAKeyParameters, RSAKeyType,
    },
};
use model::{ServerRegistrationId, ServerRegistrationInstanceId};
use newtype_uuid::{GenericUuid, TypedUuid};
use rsa::{RsaPublicKey, pkcs8::DecodePublicKey, traits::PublicKeyParts};
use serde::Serialize;
use sha2::{Digest, Sha256};
use tap::TapFallible;
use thiserror::Error;
use uuid::Uuid;
use vm_attest::{QualifyingData, RotType, VmInstanceAttestation, VmInstanceConf};
use x509_cert::{
    Certificate,
    der::{Decode, asn1::Utf8StringRef},
};

#[derive(Debug, Error)]
pub enum OidcContextError {
    #[error("Failed to decode JWT")]
    DecodeJwt(#[source] jsonwebtoken::errors::Error),
    #[error("Failed to encode JWT")]
    EncodeJwt(#[source] jsonwebtoken::errors::Error),
    #[error("Failed to deserialize")]
    Hubpack(#[from] hubpack::Error),
    #[error("Failed to decode RSA public key")]
    InvalidKey(#[source] x509_cert::spki::Error),
    #[error("Failed to verify measurements")]
    InvalidMeasurements(#[from] VerifyMeasurementsError),
    #[error("Invalid measurement set")]
    InvalidMeasurementSet(#[from] MeasurementSetError),
    #[error("Failed to decode pem")]
    JwtKey(#[source] jsonwebtoken::errors::Error),
    #[error("Certificate chain missing common name")]
    MissingCommonName,
    #[error("RoT measurement missing")]
    MissingRotMeasurement,
    #[error("No request found for registration id")]
    NoRequest,
    #[error("Failed to parse certificate from attestation chain")]
    ParseCertificate,
    #[error("Failed to deserialize VM data")]
    ParseVmData(#[source] serde_json::Error),
    #[error("Vm uuid does not match the expected value")]
    UnexpectedVmId,
    #[error("Failed to verify attestation")]
    VerifyAttestation(#[from] VerifyAttestationError),
    #[error("Failed to verify certificate chain")]
    VerifyChain(#[from] PkiPathSignatureVerifierError),
    #[error("Malformed VM data")]
    VmData(#[source] Utf8Error),
    #[error("Wrong instance id")]
    WrongInstanceId,
}

#[derive(Clone)]
pub struct OidcContext {
    root_certs: Vec<Certificate>,
    ref_measurements: Arc<ReferenceMeasurements>,
    requests: Arc<RwLock<HashMap<TypedUuid<ServerRegistrationId>, (TypedUuid<ServerRegistrationInstanceId>, QualifyingData)>>>,
    jwt: Arc<OidcJwtContext>,
    pub jwks: JwkSet,
    signing_key: EncodingKey,
    verifying_key: DecodingKey,
}

#[derive(Clone)]
pub struct OidcJwtContext {
    pub kid: String,
    pub public: String,
    pub private: String,
}

#[derive(Debug, Serialize)]
pub struct VmClaims {
    pub iss: String,
    pub aud: String,
    pub sub: Uuid,
    pub exp: i64,
    pub nbf: i64,
    pub jti: Uuid,
}

// utility function to get common name from cert subject
fn get_cert_cn(cert: &Certificate) -> Option<Utf8StringRef<'_>> {
    use const_oid::db::rfc4519::COMMON_NAME;

    for elm in cert.tbs_certificate.subject.0.iter() {
        for atav in elm.0.iter() {
            if atav.oid == COMMON_NAME {
                return Some(
                    Utf8StringRef::try_from(&atav.value)
                        .expect("Decode name attribute value to UTF8 string"),
                );
            }
        }
    }

    None
}

impl OidcContext {
    pub fn new(
        root_certs: Vec<Certificate>,
        ref_measurements: ReferenceMeasurements,
        jwt: OidcJwtContext,
    ) -> Result<Self, OidcContextError> {
        let jwks = JwkSet {
            keys: vec![Self::jwk(&jwt.kid, &jwt.public)?],
        };
        Ok(Self {
            root_certs,
            ref_measurements: Arc::new(ref_measurements),
            requests: Arc::new(RwLock::new(HashMap::new())),
            jwks,
            signing_key: EncodingKey::from_rsa_pem(jwt.private.as_bytes())
                .map_err(OidcContextError::JwtKey)?,
            verifying_key: DecodingKey::from_rsa_pem(jwt.public.as_bytes())
                .map_err(OidcContextError::JwtKey)?,
            jwt: Arc::new(jwt),
        })
    }

    pub fn store_nonce(
        &self,
        registration_id: TypedUuid<ServerRegistrationId>,
        instance_id: TypedUuid<ServerRegistrationInstanceId>,
        nonce: QualifyingData,
    ) {
        let mut requests = self.requests.write().unwrap();
        requests.insert(registration_id, (instance_id, nonce));
    }

    pub fn exchange_platform_attestation(
        &self,
        attestation: &VmInstanceAttestation,
        registration_id: TypedUuid<ServerRegistrationId>,
    ) -> Result<Option<String>, OidcContextError> {
        let qualifying_data = self.requests.write().unwrap().remove(&registration_id);
        if qualifying_data.is_none() {
            return Err(OidcContextError::NoRequest);
        }
        let (instance_id, qualifying_data) = qualifying_data.unwrap();

        tracing::info!(?instance_id, ?qualifying_data, "Retrieved instance id and qualifying data for registration");
        tracing::info!(?attestation, "Testing attestation");

        let mut cert_chain_pem = Vec::new();
        for cert in &attestation.cert_chain {
            cert_chain_pem.push(Certificate::from_der(cert).map_err(|err| {
                tracing::info!(?err, "Failed to parse attestation certificate");
                OidcContextError::ParseCertificate
            })?);
        }
        let cert_chain_pem = cert_chain_pem;
        let verified_root =
            dice_verifier::verify_cert_chain(&cert_chain_pem, Some(&self.root_certs))?;

        tracing::info!(?verified_root, "Verified cert chain");

        let common_name = get_cert_cn(verified_root).ok_or(OidcContextError::MissingCommonName)?;
        tracing::info!(?common_name, "Verified cert chain");

        // The qualifying data provided to this function must be the qualifying
        // data passed from the vm instance down to the vm instance RoT. This means
        // the nonce generated by the challenger / appraiser has already been
        // combined with the data produced by the vm instance.
        //
        // So we must calculate the qualifying data produced by the vm instance
        // RoT by combining the provided qualifying data w/ the serialized log
        // for the vm instance:
        let mut qdata = Sha256::new();
        for log in &attestation.measurement_logs {
            match log.rot {
                RotType::OxideInstance => qdata.update(&log.data),
                _ => continue,
            }
        }
        qdata.update(qualifying_data);

        // smuggle this data into the `verify_attestation` function in the
        // `attest_data::Nonce` type
        let qualifying_data = Nonce::N32(attest_data::Array(qdata.finalize().into()));

        // get the log from the Oxide platform RoT
        let oxlog = attestation
            .measurement_logs
            .iter()
            .find(|&log| log.rot == RotType::OxidePlatform);

        tracing::info!(?oxlog, "Found Oxide platform log");

        // put log in the form expected by the `verify_attestation` function
        let (log, _): (Log, _) = if let Some(oxlog) = oxlog {
            hubpack::deserialize(&oxlog.data)
                .tap_err(|err| tracing::error!(?err, "Failed to deserialize RoT measurement"))?
        } else {
            return Err(OidcContextError::MissingRotMeasurement);
        };

        tracing::info!(?log, "Deserialized log");

        let (ox_attest, _): (Attestation, _) = hubpack::deserialize(&attestation.attestation)?;

        tracing::info!(?ox_attest, "Deserialized attestation");

        dice_verifier::verify_attestation(&cert_chain_pem[0], &ox_attest, &log, &qualifying_data)?;

        tracing::info!("Verified attestation");

        let mut vm_uuid = None;

        // appraise logs
        for log in &attestation.measurement_logs {
            match log.rot {
                RotType::OxidePlatform => {
                    // use dice-verifier crate to use the RIMs to appraise the
                    // log from the OxidePlatform RoT
                    let (log, _): (Log, _) = hubpack::deserialize(&log.data)?;
                    let measurements = MeasurementSet::from_artifacts(&cert_chain_pem, &log)?;

                    dice_verifier::verify_measurements(&measurements, &self.ref_measurements)?;

                    tracing::info!(?log, ?measurements, "Verified Oxide platform measurements");
                }
                RotType::OxideInstance => {
                    // compare log / config description from the OxideInstance
                    // RoT to the reference from the config reference
                    let instance_cfg =
                        str::from_utf8(&log.data).map_err(OidcContextError::VmData)?;
                    let instance_cfg: VmInstanceConf = serde_json::from_str(instance_cfg)
                        .map_err(OidcContextError::ParseVmData)?;

                    tracing::info!(?instance_cfg, "Validating instance cfg");

                    // Verify that the uuid of the instance in the attestation matches the uuid of
                    // the instance requesting the token
                    if instance_cfg.uuid != instance_id.into_untyped_uuid() {
                        return Err(OidcContextError::WrongInstanceId);
                    }

                    tracing::info!(instance_id = ?instance_id.into_untyped_uuid(), ?instance_cfg.uuid, "Verified instance id in vm instance config matches registered server");
                    vm_uuid = Some(instance_cfg.uuid);
                }
            }
        }

        Ok(vm_uuid.map(|id| self.create_jwt(id)).transpose()?)
    }

    fn create_jwt(&self, vm: Uuid) -> Result<String, OidcContextError> {
        let mut header = Header::new(Algorithm::RS256);
        header.kid = Some(self.jwt.kid.clone());

        let claims = VmClaims {
            iss: "vm-attest-oidc".to_string(),
            aud: format!("vm-attest-oidc/{}", vm),
            sub: vm,
            exp: Utc::now().timestamp() + 3600,
            nbf: Utc::now().timestamp(),
            jti: Uuid::new_v4(),
        };

        Ok(jsonwebtoken::encode(&header, &claims, &self.signing_key)
            .map_err(OidcContextError::EncodeJwt)?)
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
