use attest_data::{Attestation, Log, Nonce};
use dice_verifier::{
    MeasurementSet, MeasurementSetError, PkiPathSignatureVerifierError, ReferenceMeasurements,
    VerifyAttestationError, VerifyMeasurementsError,
};
use newtype_uuid::GenericUuid;
use sha2::{Digest, Sha256};
use sprue_model::{ServerRegistration, ServerRegistrationState};
use std::{str::Utf8Error, sync::Arc};
use tap::TapFallible;
use thiserror::Error;
use tracing::instrument;
use v_api::response::{ResourceError, ResourceResult};
use vm_attest::{QualifyingData, RotType, VmInstanceAttestation, VmInstanceConf};
use x509_cert::{
    Certificate,
    der::{Decode, asn1::Utf8StringRef},
};

#[derive(Debug, Error)]
pub enum ServerIdentityError {
    #[error("Failed to deserialize")]
    Hubpack(#[from] hubpack::Error),
    #[error("Failed to verify instance data")]
    FailedToVerifyInstanceData,
    #[error("Failed to verify RoT")]
    FailedToVerifyRot,
    #[error("Certificate chain has the wrong organization")]
    IncorrectOrganization,
    #[error("Failed to verify measurements")]
    InvalidMeasurements(#[from] VerifyMeasurementsError),
    #[error("Invalid measurement set")]
    InvalidMeasurementSet(#[from] MeasurementSetError),
    #[error("Certificate chain missing organization")]
    MissingOrganization,
    #[error("RoT measurement missing")]
    MissingRotMeasurement,
    #[error("Failed to generate nonce")]
    Nonce,
    #[error("Nonce format is not hex encoded")]
    NonceFormat(#[from] hex::FromHexError),
    #[error("Nonce can not be converted into QualifyingData")]
    NonceInvalid,
    #[error("Instance registration does not contain a nonce")]
    NoNonce,
    #[error("Server registration is not in pending state")]
    NotPending,
    #[error("Failed to parse certificate from attestation chain")]
    ParseCertificate,
    #[error("Failed to deserialize VM data")]
    ParseVmData(#[source] serde_json::Error),
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
pub struct ServerIdentityContext {
    organization: String,
    root_certs: Vec<Certificate>,
    ref_measurements: Arc<ReferenceMeasurements>,
}

impl ServerIdentityContext {
    pub fn new(
        organization: String,
        root_certs: Vec<Certificate>,
        ref_measurements: Arc<ReferenceMeasurements>,
    ) -> Self {
        Self {
            organization,
            root_certs,
            ref_measurements,
        }
    }

    pub fn generate_nonce(&self) -> ResourceResult<String, ServerIdentityError> {
        let nonce = QualifyingData::from_platform_rng()
            .map_err(|err| {
                tracing::error!(?err, "Failed to generate nonce");
                ServerIdentityError::Nonce
            })
            .map_err(ResourceError::InternalError)?;
        let encoded_nonce = hex::encode(nonce.into_inner());
        Ok(encoded_nonce)
    }

    #[instrument(skip(self), fields(server_id = ?server.id, instance_id = ?server.instance_id))]
    pub fn verify_attestation(
        &self,
        server: &ServerRegistration,
        attestation: &VmInstanceAttestation,
    ) -> Result<(), ServerIdentityError> {
        if server.state != ServerRegistrationState::Pending {
            return Err(ServerIdentityError::NotPending);
        }
        if server.nonce.is_none() {
            return Err(ServerIdentityError::NoNonce);
        }

        let nonce: [u8; 32] = hex::decode(server.nonce.as_ref().unwrap())
            .map_err(ServerIdentityError::NonceFormat)?
            .try_into()
            .map_err(|_| ServerIdentityError::NonceInvalid)?;
        let qualifying_data = QualifyingData::from(nonce);
        tracing::info!("Reconstructed qualifying data from model");

        let mut cert_chain_pem = Vec::new();
        for cert in &attestation.cert_chain {
            cert_chain_pem.push(Certificate::from_der(cert).map_err(|err| {
                tracing::info!(?err, "Failed to parse attestation certificate");
                ServerIdentityError::ParseCertificate
            })?);
        }
        let cert_chain_pem = cert_chain_pem;
        let verified_root =
            dice_verifier::verify_cert_chain(&cert_chain_pem, Some(&self.root_certs))?;

        tracing::info!(?verified_root, "Verified cert chain");

        let organization =
            get_cert_organization(verified_root).ok_or(ServerIdentityError::MissingOrganization)?;
        tracing::info!(?organization, "Verified cert chain");

        if organization.as_str() != self.organization {
            return Err(ServerIdentityError::IncorrectOrganization);
        }

        // From vm-attest:

        // The qualifying data provided to this operation must be the qualifying
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
        tracing::info!("Found Oxide platform log");

        // Put log in the form expected by the `verify_attestation` function
        let (log, _): (Log, _) = if let Some(oxlog) = oxlog {
            hubpack::deserialize(&oxlog.data)
                .tap_err(|err| tracing::error!(?err, "Failed to deserialize RoT measurement"))?
        } else {
            return Err(ServerIdentityError::MissingRotMeasurement);
        };

        // Extract the actual attestation
        let (ox_attest, _): (Attestation, _) = hubpack::deserialize(&attestation.attestation)?;
        tracing::info!("Deserialized attestation");

        dice_verifier::verify_attestation(&cert_chain_pem[0], &ox_attest, &log, &qualifying_data)?;
        tracing::info!("Verified attestation");

        for log in &attestation.measurement_logs {
            match log.rot {
                RotType::OxidePlatform => {
                    // use dice-verifier crate to use the RIMs to appraise the
                    // log from the OxidePlatform RoT
                    let (log, _): (Log, _) = hubpack::deserialize(&log.data)?;
                    let measurements = MeasurementSet::from_artifacts(&cert_chain_pem, &log)?;
                    dice_verifier::verify_measurements(&measurements, &self.ref_measurements)?;
                    return Err(ServerIdentityError::FailedToVerifyRot);
                }
                RotType::OxideInstance => {
                    // Compare the server identity to the instance id config from the attestation
                    let instance_cfg =
                        str::from_utf8(&log.data).map_err(ServerIdentityError::VmData)?;
                    let instance_cfg: VmInstanceConf = serde_json::from_str(instance_cfg)
                        .map_err(ServerIdentityError::ParseVmData)?;

                    // Verify that the uuid of the instance in the attestation matches the uuid of
                    // the instance requesting the token
                    if instance_cfg.uuid != server.instance_id.into_untyped_uuid() {
                        return Err(ServerIdentityError::WrongInstanceId);
                    }

                    tracing::info!(instance_config = ?instance_cfg.uuid, "Verified instance id in vm instance config matches registered server");
                }
            }
        }

        tracing::info!("Verified Oxide platform measurements");

        Ok(())
    }
}

// utility function to get common name from cert subject
fn get_cert_organization(cert: &Certificate) -> Option<Utf8StringRef<'_>> {
    use const_oid::db::rfc4519::ORGANIZATION;

    for elm in cert.tbs_certificate.subject.0.iter() {
        for atav in elm.0.iter() {
            if atav.oid == ORGANIZATION {
                return Some(
                    Utf8StringRef::try_from(&atav.value)
                        .expect("Decode name attribute value to UTF8 string"),
                );
            }
        }
    }

    None
}
