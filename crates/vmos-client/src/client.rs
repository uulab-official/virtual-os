//! Blocking JSON-RPC client for the virtual-os daemon.

use std::io::{BufRead, BufReader, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::protocol::{
    Empty, PingResult, RpcError, RpcRequest, RpcResponse, ShutdownResult, JSONRPC_VERSION,
    MAX_LINE_BYTES,
};

/// Default timeout for metadata operations.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
/// Timeout for operations that may touch large disk images or boot a guest.
pub const LONG_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("cannot connect to daemon at {}: {source}", path.display())]
    Connect {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("daemon I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("daemon closed the connection before replying")]
    Closed,

    #[error("request timed out after {0:?}")]
    Timeout(Duration),

    #[error("protocol error: {0}")]
    Protocol(String),

    #[error("{0}")]
    Rpc(#[from] RpcError),
}

/// A connected daemon client. Cheap to create; one connection per CLI command.
pub struct Client {
    write: UnixStream,
    read: BufReader<UnixStream>,
    next_id: u64,
    socket: PathBuf,
}

impl Client {
    pub fn connect(socket: impl AsRef<Path>) -> Result<Self, ClientError> {
        let socket = socket.as_ref().to_path_buf();
        let stream = UnixStream::connect(&socket).map_err(|source| ClientError::Connect {
            path: socket.clone(),
            source,
        })?;
        let read_stream = stream.try_clone()?;
        Ok(Self {
            write: stream,
            read: BufReader::new(read_stream),
            next_id: 1,
            socket,
        })
    }

    pub fn connect_default() -> Result<Self, ClientError> {
        Self::connect(vmos_core::VmStore::default_root().join("daemon.sock"))
    }

    /// Connect only if the socket accepts a connection.
    pub fn connect_if_running(socket: impl AsRef<Path>) -> Option<Self> {
        let socket = socket.as_ref().to_path_buf();
        match UnixStream::connect(&socket) {
            Ok(stream) => {
                let read_stream = stream.try_clone().ok()?;
                Some(Self {
                    write: stream,
                    read: BufReader::new(read_stream),
                    next_id: 1,
                    socket,
                })
            }
            Err(_) => None,
        }
    }

    pub fn socket_path(&self) -> &Path {
        &self.socket
    }

    pub fn ping(&mut self) -> Result<PingResult, ClientError> {
        self.call(crate::protocol::methods::SYSTEM_PING, &Empty {})
    }

    /// Send a request with the default timeout.
    pub fn call<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &P,
    ) -> Result<R, ClientError> {
        self.call_with_timeout(method, params, DEFAULT_TIMEOUT)
    }

    /// Send a request that may legitimately take a long time.
    pub fn call_long<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &P,
    ) -> Result<R, ClientError> {
        self.call_with_timeout(method, params, LONG_TIMEOUT)
    }

    pub fn call_with_timeout<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: &P,
        timeout: Duration,
    ) -> Result<R, ClientError> {
        let value = self.request(method, params, timeout)?;
        serde_json::from_value(value)
            .map_err(|error| ClientError::Protocol(format!("invalid result: {error}")))
    }

    /// Send a request and return the raw JSON result.
    pub fn request<P: Serialize>(
        &mut self,
        method: &str,
        params: &P,
        timeout: Duration,
    ) -> Result<Value, ClientError> {
        let id = self.next_id;
        self.next_id = self.next_id.wrapping_add(1).max(1);
        let params = serde_json::to_value(params)
            .map_err(|error| ClientError::Protocol(format!("cannot encode params: {error}")))?;
        let request = RpcRequest::new(id, method, params);
        let mut line = serde_json::to_vec(&request)
            .map_err(|error| ClientError::Protocol(format!("cannot encode request: {error}")))?;
        line.push(b'\n');

        self.write.set_write_timeout(Some(timeout))?;
        self.write.write_all(&line)?;
        self.write.flush()?;

        self.read.get_ref().set_read_timeout(Some(timeout))?;
        let mut response_line = String::new();
        let read = self.read.read_line(&mut response_line)?;
        if read == 0 {
            return Err(ClientError::Closed);
        }
        if response_line.len() > MAX_LINE_BYTES {
            return Err(ClientError::Protocol("response line too large".into()));
        }
        let response: RpcResponse = serde_json::from_str(&response_line)
            .map_err(|error| ClientError::Protocol(format!("invalid response: {error}")))?;
        if response.jsonrpc != JSONRPC_VERSION {
            return Err(ClientError::Protocol(format!(
                "unexpected JSON-RPC version {:?}",
                response.jsonrpc
            )));
        }
        if response.id != id {
            return Err(ClientError::Protocol(format!(
                "response id {} does not match request id {id}",
                response.id
            )));
        }
        if let Some(error) = response.error {
            return Err(ClientError::Rpc(error));
        }
        Ok(response.result.unwrap_or(Value::Null))
    }

    pub fn shutdown(&mut self) -> Result<ShutdownResult, ClientError> {
        self.call(crate::protocol::methods::SYSTEM_SHUTDOWN, &Empty {})
    }
}

impl std::fmt::Debug for Client {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Client")
            .field("socket", &self.socket)
            .finish_non_exhaustive()
    }
}
