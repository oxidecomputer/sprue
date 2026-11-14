// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use attest_data::{Attestation, Log, Nonce};
use dice_verifier::{
    MeasurementSet, MeasurementSetError, PkiPathSignatureVerifierError, ReferenceMeasurements,
    VerifyAttestationError, VerifyMeasurementsError,
};
use newtype_uuid::{GenericUuid, TypedUuid};
use sha2::{Digest, Sha256};
use sprue_model::{ProjectId, ServerRegistrationInstanceId, SiloId};
use std::{future::Future, str::Utf8Error, sync::Arc};
use tap::TapFallible;
use thiserror::Error;
use tokio::{sync::Mutex, task::JoinHandle};
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
    #[error("Certificate chain has the wrong organization")]
    IncorrectOrganization,
    #[error("Failed to verify measurements")]
    InvalidMeasurements(#[from] VerifyMeasurementsError),
    #[error("Invalid measurement set")]
    InvalidMeasurementSet(#[from] MeasurementSetError),
    #[error("Certificate chain missing organization")]
    MissingOrganization,
    #[error("Instance log missing from attestation")]
    MissingInstanceData,
    #[error("RoT measurement missing")]
    MissingRotMeasurement,
    #[error("Attestation contains more than one instance log")]
    MultipleInstanceData,
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
    VerifyAttestation(VerifyAttestationError),
    #[error("Failed to verify certificate chain")]
    VerifyChain(PkiPathSignatureVerifierError),
    #[error("Malformed VM data")]
    VmData(#[source] Utf8Error),
}

#[derive(Clone)]
pub struct ServerIdentityContext {
    organization: String,
    root_certs: Vec<Certificate>,
    ref_measurements_loader: Arc<Mutex<Option<JoinHandle<Arc<ReferenceMeasurements>>>>>,
    ref_measurements: Arc<Mutex<Option<Arc<ReferenceMeasurements>>>>,
}

impl ServerIdentityContext {
    pub fn new<F, Fut>(organization: String, root_certs: Vec<Certificate>, loader: F) -> Self
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Arc<ReferenceMeasurements>> + Send + 'static,
    {
        // Load reference measurements in the background so a potentially slow
        // repository download does not block server startup.
        let handle = tokio::spawn(loader());
        Self {
            organization,
            root_certs,
            ref_measurements_loader: Arc::new(Mutex::new(Some(handle))),
            ref_measurements: Arc::new(Mutex::new(None)),
        }
    }

    /// Resolves the reference measurements, awaiting the background loader task
    /// on first access and caching the result for subsequent calls.
    async fn reference_measurements(&self) -> Arc<ReferenceMeasurements> {
        let mut cached = self.ref_measurements.lock().await;
        if let Some(measurements) = cached.as_ref() {
            return measurements.clone();
        }

        let handle = self
            .ref_measurements_loader
            .lock()
            .await
            .take()
            .expect("reference measurement loader is consumed exactly once");
        let measurements = handle
            .await
            .expect("reference measurement loader task should not panic");
        *cached = Some(measurements.clone());
        measurements
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

    /// Verify an attestation produced over the given nonce. On success this
    /// returns the instance identity that the attestation proves. The identity
    /// is read from the attested instance log.
    #[instrument(skip(self, nonce, attestation))]
    pub async fn verify_instance_attestation(
        &self,
        nonce: &str,
        attestation: &VmInstanceAttestation,
    ) -> Result<VerifiedInstance, ServerIdentityError> {
        let nonce: [u8; 32] = hex::decode(nonce)
            .map_err(ServerIdentityError::NonceFormat)?
            .try_into()
            .map_err(|_| ServerIdentityError::NonceInvalid)?;
        let qualifying_data = QualifyingData::from(nonce);
        tracing::info!("Reconstructed qualifying data from model");

        // The attestation must carry exactly one instance log. It is the only
        // source of the instance identity, so a missing log is an error.
        let instance_logs = attestation
            .measurement_logs
            .iter()
            .filter(|log| log.rot == RotType::OxideInstance)
            .collect::<Vec<_>>();
        let instance_log = match instance_logs.as_slice() {
            [log] => Ok(*log),
            [] => Err(ServerIdentityError::MissingInstanceData),
            _ => Err(ServerIdentityError::MultipleInstanceData),
        }?;
        let instance_cfg: VmInstanceConf = serde_json::from_str(
            str::from_utf8(&instance_log.data).map_err(ServerIdentityError::VmData)?,
        )
        .map_err(ServerIdentityError::ParseVmData)?;

        let mut cert_chain_pem = Vec::new();
        for cert in &attestation.cert_chain {
            cert_chain_pem.push(Certificate::from_der(cert).map_err(|err| {
                tracing::info!(?err, "Failed to parse attestation certificate");
                ServerIdentityError::ParseCertificate
            })?);
        }

        // Verify certificate chain against known root certificates
        tracing::debug!(?cert_chain_pem, roots = ?self.root_certs, "Verifying certificate chain against known root certificates");
        let verified_root =
            dice_verifier::verify_cert_chain(&cert_chain_pem, Some(&self.root_certs))
                .map_err(ServerIdentityError::VerifyChain)?;

        let organization =
            get_cert_organization(verified_root).ok_or(ServerIdentityError::MissingOrganization)?;
        tracing::info!(?organization, "Verified cert chain");

        if organization.as_str() != self.organization {
            return Err(ServerIdentityError::IncorrectOrganization);
        }

        // The qualifying data that reached the platform RoT is the digest of the
        // instance log followed by the challenge nonce. Rebuilding it here binds
        // the instance log to the signed attestation.
        let mut qdata = Sha256::new();
        qdata.update(&instance_log.data);
        qdata.update(qualifying_data);
        let qualifying_data = Nonce::N32(attest_data::Array(qdata.finalize().into()));

        // Get the log from the Oxide platform RoT
        let oxlog = attestation
            .measurement_logs
            .iter()
            .find(|&log| log.rot == RotType::OxidePlatform)
            .ok_or(ServerIdentityError::MissingRotMeasurement)?;
        let (log, _): (Log, _) = hubpack::deserialize(&oxlog.data)
            .tap_err(|err| tracing::error!(?err, "Failed to deserialize RoT measurement"))?;
        tracing::info!("Found Oxide platform log");

        // Extract the actual attestation
        let (ox_attest, _): (Attestation, _) = hubpack::deserialize(&attestation.attestation)?;
        tracing::info!("Deserialized attestation");

        dice_verifier::verify_attestation(&cert_chain_pem[0], &ox_attest, &log, &qualifying_data)
            .map_err(ServerIdentityError::VerifyAttestation)?;
        tracing::info!("Verified attestation");

        // Appraise the platform log against the reference measurements
        let reference_measurements = self.reference_measurements().await;
        let measurements = MeasurementSet::from_artifacts(&cert_chain_pem, &log)?;
        dice_verifier::verify_measurements(&measurements, reference_measurements.as_ref())
            .tap_err(|err| {
                tracing::info!(?err, "Failed to verify RoT measurements");
            })?;
        tracing::info!(instance = ?instance_cfg.uuid, "Verified Oxide platform measurements");

        Ok(VerifiedInstance {
            instance_id: TypedUuid::from_untyped_uuid(instance_cfg.uuid),
            project_id: TypedUuid::from_untyped_uuid(instance_cfg.project),
            silo_id: TypedUuid::from_untyped_uuid(instance_cfg.silo),
        })
    }
}

/// The identity of a VM instance as proven by a verified attestation. A value
/// of this type can only be produced by `verify_instance_attestation`, so
/// holding one means that every field came from attested data.
#[derive(Debug, Clone, Copy)]
pub struct VerifiedInstance {
    instance_id: TypedUuid<ServerRegistrationInstanceId>,
    project_id: TypedUuid<ProjectId>,
    silo_id: TypedUuid<SiloId>,
}

impl VerifiedInstance {
    pub fn instance_id(&self) -> TypedUuid<ServerRegistrationInstanceId> {
        self.instance_id
    }

    pub fn project_id(&self) -> TypedUuid<ProjectId> {
        self.project_id
    }

    pub fn silo_id(&self) -> TypedUuid<SiloId> {
        self.silo_id
    }
}

// utility function to get common name from cert subject
fn get_cert_organization(cert: &Certificate) -> Option<Utf8StringRef<'_>> {
    use const_oid::db::rfc4519::ORGANIZATION_NAME;

    for elm in cert.tbs_certificate.issuer.0.iter() {
        tracing::info!(?elm, "Issuer element");
        for atav in elm.0.iter() {
            if atav.oid == ORGANIZATION_NAME {
                return Some(
                    Utf8StringRef::try_from(&atav.value)
                        .expect("Decode name attribute value to UTF8 string"),
                );
            }
        }
    }

    None
}
