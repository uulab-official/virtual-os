use std::path::PathBuf;
use std::time::Duration;

use objc2_foundation::NSError;

use crate::util::describe_ns_error;

#[derive(Debug, thiserror::Error)]
pub enum VzError {
    #[error("Virtualization.framework is not supported on this Mac")]
    Unsupported,

    #[error(
        "missing the com.apple.security.virtualization entitlement: {details}. \
         Sign the binary with `scripts/sign-dev.sh` (ad-hoc signing is sufficient for local use)"
    )]
    EntitlementMissing { details: String },

    #[error("invalid VM configuration: {0}")]
    InvalidConfig(String),

    #[error("Objective-C exception while talking to Virtualization.framework: {0}")]
    Exception(String),

    #[error("{operation} failed: {message}")]
    Framework {
        operation: &'static str,
        message: String,
    },

    #[error("{operation} timed out after {seconds}s")]
    Timeout {
        operation: &'static str,
        seconds: u64,
    },

    #[error("VM is {state}; cannot {action}")]
    InvalidState {
        state: &'static str,
        action: &'static str,
    },

    #[error("I/O error at {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{0}")]
    Other(String),
}

impl VzError {
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }

    pub fn other(message: impl Into<String>) -> Self {
        Self::Other(message.into())
    }

    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::InvalidConfig(message.into())
    }

    pub fn timeout(operation: &'static str, timeout: Duration) -> Self {
        Self::Timeout {
            operation,
            seconds: timeout.as_secs().max(1),
        }
    }
}

pub(crate) fn framework_error(operation: &'static str, error: &NSError) -> VzError {
    let message = describe_ns_error(error);
    if message.to_ascii_lowercase().contains("entitlement") {
        VzError::EntitlementMissing { details: message }
    } else {
        VzError::Framework { operation, message }
    }
}

pub(crate) fn exception_error(
    stage: &str,
    exception: Option<&objc2::exception::Exception>,
) -> VzError {
    let details = exception
        .map(|exception| format!("{exception:?}"))
        .unwrap_or_else(|| "unknown Objective-C exception".to_string());
    if details.to_ascii_lowercase().contains("entitlement") {
        VzError::EntitlementMissing { details }
    } else {
        VzError::Exception(format!("{stage}: {details}"))
    }
}
