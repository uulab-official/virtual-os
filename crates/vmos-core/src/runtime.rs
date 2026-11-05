//! Persisted runtime state (`runtime.json`).
//!
//! The authoritative VM state lives in the daemon, but it is mirrored to the
//! VM directory so that `vmos list` remains useful when the daemon is down and
//! so that stale processes can be detected after a crash.

use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum RunState {
    #[default]
    Stopped,
    Starting,
    Running,
    Paused,
    Stopping,
    Error,
}

impl RunState {
    pub fn label(self) -> &'static str {
        match self {
            Self::Stopped => "stopped",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Stopping => "stopping",
            Self::Error => "error",
        }
    }

    /// States in which the VM occupies resources and mutation must be refused.
    pub fn is_active(self) -> bool {
        matches!(
            self,
            Self::Starting | Self::Running | Self::Paused | Self::Stopping
        )
    }

    pub fn is_transitional(self) -> bool {
        matches!(self, Self::Starting | Self::Stopping)
    }
}

impl std::fmt::Display for RunState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeState {
    pub state: RunState,
    /// PID of the process that owns the Virtualization.framework VM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub started_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Timestamp>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            state: RunState::Stopped,
            pid: None,
            started_at: None,
            updated_at: None,
            last_error: None,
        }
    }
}

impl RuntimeState {
    pub fn running(pid: u32) -> Self {
        let now = Timestamp::now();
        Self {
            state: RunState::Running,
            pid: Some(pid),
            started_at: Some(now),
            updated_at: Some(now),
            last_error: None,
        }
    }

    pub fn stopped() -> Self {
        Self {
            state: RunState::Stopped,
            updated_at: Some(Timestamp::now()),
            ..Self::default()
        }
    }

    pub fn error(message: impl Into<String>) -> Self {
        Self {
            state: RunState::Error,
            pid: None,
            started_at: None,
            updated_at: Some(Timestamp::now()),
            last_error: Some(message.into()),
        }
    }

    /// Resolve a runtime record whose owning process has disappeared.
    ///
    /// A VM is owned by the daemon process. If that PID is gone, no guest can
    /// still be running, so reporting the stale record as active would be a
    /// lie that blocks every subsequent operation.
    pub fn effective(mut self) -> Self {
        if self.state.is_active() {
            match self.pid {
                Some(pid) if crate::util::process_alive(pid) => {}
                Some(_) | None => {
                    self.state = RunState::Stopped;
                    self.pid = None;
                    self.started_at = None;
                    self.updated_at = Some(Timestamp::now());
                    self.last_error
                        .get_or_insert_with(|| "owner process exited".to_string());
                }
            }
        }
        self
    }
}
