use serde::{Deserialize, Serialize};

/// Lifecycle state observed by the host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum VmState {
    #[default]
    Stopped,
    Starting,
    Running,
    Pausing,
    Paused,
    Resuming,
    Stopping,
    Saving,
    Restoring,
    Error,
}

impl VmState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Pausing => "pausing",
            Self::Paused => "paused",
            Self::Resuming => "resuming",
            Self::Stopping => "stopping",
            Self::Saving => "saving",
            Self::Restoring => "restoring",
            Self::Error => "error",
        }
    }

    pub fn is_active(self) -> bool {
        !matches!(self, Self::Stopped | Self::Error)
    }

    pub fn can_start(self) -> bool {
        matches!(self, Self::Stopped | Self::Error)
    }

    pub fn can_stop(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Running | Self::Pausing | Self::Paused | Self::Resuming
        )
    }
}

impl std::fmt::Display for VmState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
