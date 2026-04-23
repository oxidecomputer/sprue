use lib_vsock::VsockStream;
use std::{
    cell::RefCell,
    io::{BufRead, BufReader, Write},
    ops::DerefMut,
};
use vm_attest::{QualifyingData, Request, Response, VmInstanceAttestation, VmInstanceAttester};

#[derive(Debug)]
pub struct VmInstanceRotVsockClient {
    socket: RefCell<VsockStream>,
}

impl VmInstanceRotVsockClient {
    pub fn new(socket: VsockStream) -> Self {
        Self {
            socket: RefCell::new(socket),
        }
    }
}

/// Errors returned when trying to sign an attestation
#[derive(Debug, thiserror::Error)]
pub enum VmInstanceRotVsockClientError {
    #[error("error deserializing a PlatformAttestation from JSON")]
    Deserialize(#[from] serde_json::Error),

    #[error("error from the underlying socket")]
    Socket(#[from] std::io::Error),

    #[error("error from the VmInstanceRoT")]
    VmInstanceRotError(String),
}

impl VmInstanceAttester for VmInstanceRotVsockClient {
    type Error = VmInstanceRotVsockClientError;

    fn attest(
        &self,
        qualifying_data: &QualifyingData,
    ) -> Result<VmInstanceAttestation, Self::Error> {
        let request = Request::Attest(qualifying_data.clone());
        let mut request = serde_json::to_string(&request)?;
        request.push('\n');
        let request = request;

        tracing::debug!("writing request: {request}");
        self.socket.borrow_mut().write_all(request.as_bytes())?;

        let mut socket_mut = self.socket.borrow_mut();
        let mut reader = BufReader::new(socket_mut.deref_mut());

        let mut response = String::new();
        reader.read_line(&mut response)?;

        tracing::debug!("got response: {response}");
        // map response message to Result
        let response: Response = serde_json::from_str(&response)?;
        match response {
            Response::Attest(a) => Ok(a),
            Response::Error(e) => Err(Self::Error::VmInstanceRotError(e)),
        }
    }
}
