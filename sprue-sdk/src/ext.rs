use vm_attest::{MeasurementLog, VmInstanceAttestation};

use crate::types::{Attestation, AttestationMeasurementLogsItem};

impl From<VmInstanceAttestation> for Attestation {
    fn from(value: VmInstanceAttestation) -> Self {
        Self {
            attestation: value.attestation,
            cert_chain: value.cert_chain,
            measurement_logs: value
                .measurement_logs
                .into_iter()
                .map(|log| log.into())
                .collect::<Vec<_>>(),
        }
    }
}

impl From<MeasurementLog> for AttestationMeasurementLogsItem {
    fn from(value: MeasurementLog) -> Self {
        Self {
            data: value.data,
            rot: serde_json::to_value(&value.rot)
                .expect("RotType is directly serializable")
                .as_str()
                .expect("RotType serializes as a string")
                .to_string(),
        }
    }
}
