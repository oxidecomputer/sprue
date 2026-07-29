// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

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
