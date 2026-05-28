use anyhow::{Context, Result};
use vm_attest::{MeasurementLog, QualifyingData, RotType, VmInstanceAttestation, VmInstanceConf};

/// Extract the `VmInstanceConf` from attestation measurement logs.
///
/// The platform embeds the VM instance configuration as a JSON-serialized
/// `OxideInstance` measurement log entry.
pub fn extract_vm_conf(logs: &[MeasurementLog]) -> Result<VmInstanceConf> {
    for log in logs {
        if log.rot == RotType::OxideInstance {
            let raw = std::str::from_utf8(&log.data)
                .context("OxideInstance measurement log is not valid UTF-8")?;
            let conf: VmInstanceConf =
                serde_json::from_str(raw).context("Failed to parse VmInstanceConf from log")?;
            return Ok(conf);
        }
    }
    anyhow::bail!("No OxideInstance measurement log found in attestation")
}

/// Abstraction over the platform attestation mechanism.
pub trait Platform: Send + Sync {
    fn attest(&self, data: &QualifyingData) -> Result<VmInstanceAttestation>;

    /// Perform a discovery attestation with random qualifying data and extract the VM config.
    fn discover_identity(&self) -> Result<(VmInstanceAttestation, VmInstanceConf)> {
        let data = QualifyingData::from_platform_rng()
            .map_err(|err| anyhow::anyhow!("Failed to generate random qualifying data: {}", err))?;
        let attestation = self.attest(&data)?;
        let conf = extract_vm_conf(&attestation.measurement_logs)?;
        Ok((attestation, conf))
    }
}

// Real Oxide platform – vsock to the RoT
pub struct OxidePlatform;

impl Platform for OxidePlatform {
    fn attest(&self, data: &QualifyingData) -> Result<VmInstanceAttestation> {
        use lib_vsock::{VMADDR_CID_HOST, VsockAddr, VsockStream};
        use vm_attest::VmInstanceAttester;

        use crate::{VM_ATTESTATION_PORT, vsock::VmInstanceRotVsockClient};

        let addr = VsockAddr::new(VMADDR_CID_HOST, VM_ATTESTATION_PORT);
        let stream = VsockStream::connect(&addr).context("vsock connect")?;
        let rot = VmInstanceRotVsockClient::new(stream);
        Ok(rot.attest(data)?)
    }
}

// Mock platform – for local development without Oxide hardware
#[cfg(feature = "local-dev")]
pub mod mock {
    use super::*;
    use dice_verifier::AttestMock;
    use std::path::{Path, PathBuf};
    use vm_attest::VmInstanceRot;

    /// A mock platform that uses `AttestMock` + `VmInstanceRot` to produce
    /// attestations from on-disk test fixtures, without any vsock dependency.
    pub struct MockPlatform {
        vm_conf: VmInstanceConf,
        cert_chain: PathBuf,
        log: PathBuf,
        alias_key: PathBuf,
    }

    impl MockPlatform {
        /// Create a mock platform from explicit file paths.
        #[allow(dead_code)]
        pub fn new(
            vm_conf: VmInstanceConf,
            cert_chain: PathBuf,
            log: PathBuf,
            alias_key: PathBuf,
        ) -> Self {
            Self {
                vm_conf,
                cert_chain,
                log,
                alias_key,
            }
        }

        /// Create a mock platform using the standard test-data directory.
        ///
        /// `test_data_dir` should point to a directory containing:
        ///   - `cert-chain.pem`
        ///   - `log.bin`
        ///   - `alias.key`
        ///   - `vm.json`
        pub fn from_test_data(test_data_dir: &Path) -> Result<Self> {
            let vm_json = std::fs::read_to_string(test_data_dir.join("vm.json"))
                .context("Failed to read vm.json")?;
            let vm_conf: VmInstanceConf =
                serde_json::from_str(&vm_json).context("Failed to parse vm.json")?;

            Ok(Self {
                vm_conf,
                cert_chain: test_data_dir.join("cert-chain.pem"),
                log: test_data_dir.join("log.bin"),
                alias_key: test_data_dir.join("alias.key"),
            })
        }

        fn rot(&self) -> Result<VmInstanceRot> {
            let attest = Box::new(
                AttestMock::load(&self.cert_chain, &self.log, &self.alias_key).map_err(|err| {
                    anyhow::anyhow!("Failed to load mock attestation data: {}", err)
                })?,
            );
            Ok(VmInstanceRot::new(attest))
        }
    }

    impl Platform for MockPlatform {
        fn attest(&self, data: &QualifyingData) -> Result<VmInstanceAttestation> {
            let rot = self.rot()?;
            // VmInstanceRot::attest is async but AttestMock doesn't actually
            // block, so running it synchronously here is fine.
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(rot.attest(&self.vm_conf, data))
            })
            .map_err(|err| anyhow::anyhow!("Mock attestation failed: {}", err))
        }
    }
}
