//! Lifecycle events published by the daemon.

use serde::{Deserialize, Serialize};

use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventKind {
    VmCreated,
    VmDeleted,
    VmCloned { source: String },
    VmStarted,
    VmStopped { forced: bool },
    VmPaused,
    VmResumed,
    VmError { message: String },
    SnapshotCreated { snapshot: String },
    SnapshotRestored { snapshot: String },
    SnapshotDeleted { snapshot: String },
    DiskCreated { disk: String, size_bytes: u64 },
    DiskResized { disk: String, size_bytes: u64 },
    DiskCloned { disk: String, target: String },
    DaemonStarted,
    DaemonStopping,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub seq: u64,
    pub at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vm: Option<String>,
    pub kind: EventKind,
}
