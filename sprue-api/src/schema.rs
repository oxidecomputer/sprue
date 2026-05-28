// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use vm_attest::VmInstanceAttestation;

#[derive(Debug, Deserialize, Serialize)]
#[serde(transparent)]
pub struct Attestation(VmInstanceAttestation);

impl Attestation {
    pub fn into_inner(self) -> VmInstanceAttestation {
        self.0
    }
}

impl Deref for Attestation {
    type Target = VmInstanceAttestation;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl JsonSchema for Attestation {
    fn schema_name() -> String {
        "Attestation".to_string()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        use schemars::schema::{InstanceType, ObjectValidation, Schema, SchemaObject, SingleOrVec};

        let bytes_schema = generator.subschema_for::<Vec<u8>>();
        let nested_bytes_schema = generator.subschema_for::<Vec<Vec<u8>>>();

        // MeasurementLog: { rot: string, data: [u8] }
        let measurement_log_schema = SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
            object: Some(Box::new(ObjectValidation {
                properties: [
                    (
                        "rot".to_string(),
                        Schema::Object(SchemaObject {
                            instance_type: Some(SingleOrVec::Single(Box::new(
                                InstanceType::String,
                            ))),
                            ..Default::default()
                        }),
                    ),
                    ("data".to_string(), bytes_schema.clone()),
                ]
                .into_iter()
                .collect(),
                required: ["rot".to_string(), "data".to_string()]
                    .into_iter()
                    .collect(),
                ..Default::default()
            })),
            ..Default::default()
        };

        let measurement_logs_schema = SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Array))),
            array: Some(Box::new(schemars::schema::ArrayValidation {
                items: Some(SingleOrVec::Single(Box::new(Schema::Object(
                    measurement_log_schema,
                )))),
                ..Default::default()
            })),
            ..Default::default()
        };

        Schema::Object(SchemaObject {
            instance_type: Some(SingleOrVec::Single(Box::new(InstanceType::Object))),
            object: Some(Box::new(ObjectValidation {
                properties: [
                    ("attestation".to_string(), bytes_schema),
                    ("cert_chain".to_string(), nested_bytes_schema),
                    (
                        "measurement_logs".to_string(),
                        Schema::Object(measurement_logs_schema),
                    ),
                ]
                .into_iter()
                .collect(),
                required: [
                    "attestation".to_string(),
                    "cert_chain".to_string(),
                    "measurement_logs".to_string(),
                ]
                .into_iter()
                .collect(),
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}
