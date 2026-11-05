//! Cold snapshots: configuration + disk images captured while stopped.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::config::{VmConfig, SCHEMA_VERSION};
use crate::time::Timestamp;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotDisk {
    pub id: String,
    /// Original path recorded relative to the VM directory (or absolute).
    pub path: PathBuf,
    /// Path inside the snapshot directory.
    pub snapshot_path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotFile {
    /// Original path relative to the VM directory (or absolute).
    pub path: PathBuf,
    /// Path inside the snapshot directory.
    pub snapshot_path: PathBuf,
    pub size_bytes: u64,
}

/// Snapshot manifest stored as `snapshot.json`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema_version: u32,
    pub name: String,
    pub vm_id: String,
    pub created_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Complete VM definition at capture time.
    pub config: VmConfig,
    pub disks: Vec<SnapshotDisk>,
    /// Firmware and platform files (EFI vars, machine identifier, aux storage).
    #[serde(default)]
    pub files: Vec<SnapshotFile>,
}

impl Snapshot {
    pub fn new(
        name: impl Into<String>,
        config: VmConfig,
        disks: Vec<SnapshotDisk>,
        files: Vec<SnapshotFile>,
        description: Option<String>,
    ) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            name: name.into(),
            vm_id: config.id.to_string(),
            created_at: Timestamp::now(),
            description,
            config,
            disks,
            files,
        }
    }

    pub fn total_bytes(&self) -> u64 {
        self.disks
            .iter()
            .map(|disk| disk.size_bytes)
            .chain(self.files.iter().map(|file| file.size_bytes))
            .sum()
    }

    pub fn summary(&self) -> SnapshotSummary {
        SnapshotSummary {
            name: self.name.clone(),
            created_at: self.created_at,
            description: self.description.clone(),
            disk_count: self.disks.len(),
            total_bytes: self.total_bytes(),
            state: self.config.boot.kind().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotSummary {
    pub name: String,
    pub created_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub disk_count: usize,
    pub total_bytes: u64,
    /// Boot mode at capture time (`linux`, `efi`, `macos`).
    pub state: String,
}
