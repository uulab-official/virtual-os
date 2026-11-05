use vmos_client::RpcError;
use vmos_core::CoreError;
use vmos_vz::VzError;

use vmos_client::error_codes;

#[derive(Debug, thiserror::Error)]
pub enum DaemonError {
    #[error(transparent)]
    Core(#[from] CoreError),

    #[error(transparent)]
    Vz(#[from] VzError),

    #[error("VM {0:?} is not running")]
    NotRunning(String),

    #[error("{0}")]
    Message(String),

    #[error("cannot bind daemon socket: {0}")]
    Socket(String),
}

impl DaemonError {
    pub fn message(value: impl Into<String>) -> Self {
        Self::Message(value.into())
    }

    pub fn to_rpc_error(&self) -> RpcError {
        let (code, kind) = match self {
            Self::Core(CoreError::VmNotFound(_)) => (error_codes::VM_NOT_FOUND, "vm_not_found"),
            Self::Core(CoreError::SnapshotNotFound { .. }) => {
                (error_codes::VM_NOT_FOUND, "snapshot_not_found")
            }
            Self::Core(CoreError::DiskNotFound(_)) => (error_codes::VM_NOT_FOUND, "disk_not_found"),
            Self::Core(CoreError::VmExists(_))
            | Self::Core(CoreError::SnapshotExists(_))
            | Self::Core(CoreError::DiskExists(_)) => {
                (error_codes::ALREADY_EXISTS, "already_exists")
            }
            Self::Core(CoreError::VmBusy { .. }) => (error_codes::VM_BUSY, "vm_busy"),
            Self::Core(CoreError::InvalidName { .. })
            | Self::Core(CoreError::InvalidId { .. })
            | Self::Core(CoreError::InvalidConfig(_)) => {
                (error_codes::INVALID_PARAMS, "invalid_params")
            }
            Self::Core(CoreError::NotInitialized { .. }) => {
                (error_codes::NOT_INITIALIZED, "not_initialized")
            }
            Self::Core(CoreError::Unsupported(_)) => (error_codes::UNSUPPORTED, "unsupported"),
            Self::Vz(VzError::EntitlementMissing { .. }) => {
                (error_codes::ENTITLEMENT_MISSING, "entitlement_missing")
            }
            Self::Vz(VzError::Unsupported) => (error_codes::UNSUPPORTED, "unsupported"),
            Self::Vz(VzError::InvalidConfig(_)) => (error_codes::INVALID_PARAMS, "invalid_params"),
            Self::Vz(VzError::InvalidState { .. }) => (error_codes::VM_BUSY, "invalid_state"),
            Self::Vz(VzError::Timeout { .. }) => (error_codes::TIMEOUT, "timeout"),
            Self::NotRunning(_) => (error_codes::VM_BUSY, "not_running"),
            Self::Core(CoreError::Io { .. }) | Self::Core(CoreError::Json { .. }) => {
                (error_codes::DAEMON_ERROR, "io")
            }
            _ => (error_codes::DAEMON_ERROR, "daemon_error"),
        };
        RpcError::new(code, self.to_string()).with_data(serde_json::json!({ "kind": kind }))
    }
}
