use std::path::{Path, PathBuf};

/// Errors produced by the core model.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("invalid VM name {name:?}: {reason}")]
    InvalidName { name: String, reason: &'static str },

    #[error("invalid identifier {value:?}: {reason}")]
    InvalidId { value: String, reason: &'static str },

    #[error("VM {0:?} not found")]
    VmNotFound(String),

    #[error("VM {0:?} already exists")]
    VmExists(String),

    #[error("VM {name:?} is {state}; {action} requires the VM to be stopped")]
    VmBusy {
        name: String,
        state: String,
        action: &'static str,
    },

    #[error("VM configuration is invalid: {0}")]
    InvalidConfig(String),

    #[error("snapshot {snapshot:?} not found for VM {vm:?}")]
    SnapshotNotFound { vm: String, snapshot: String },

    #[error("snapshot {0:?} already exists")]
    SnapshotExists(String),

    #[error("disk {0:?} not found")]
    DiskNotFound(String),

    #[error("disk {0:?} already exists")]
    DiskExists(String),

    #[error("disk {0:?} is attached to VM {1:?} and cannot be removed")]
    DiskInUse(String, String),

    #[error("operation is not supported: {0}")]
    Unsupported(String),

    #[error("virtual-os is not initialized at {}; run `vmos init`", .root.display())]
    NotInitialized { root: PathBuf },

    #[error("I/O error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("failed to parse {}: {source}", path.display())]
    Json {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("{0}")]
    Other(String),
}

impl CoreError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn json(path: impl AsRef<Path>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }

    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::InvalidConfig(message.into())
    }
}

pub type Result<T> = std::result::Result<T, CoreError>;
