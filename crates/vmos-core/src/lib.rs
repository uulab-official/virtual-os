//! Core model for virtual-os.
//!
//! This crate is deliberately free of Apple frameworks so that configuration,
//! storage and snapshot logic can be built and tested anywhere. The macOS
//! Virtualization.framework backend lives in `vmos-vz`.
//!
//! The on-disk layout is intentionally simple and inspectable:
//!
//! ```text
//! $VOS_HOME/
//!   daemon.sock
//!   daemon.pid
//!   daemon.log
//!   vms/
//!     <name>/
//!       vm.json          VM definition (versioned JSON)
//!       runtime.json     last observed runtime state
//!       disks/<id>.img   sparse raw disk images
//!       nvram/*.fd       EFI variable stores
//!       snapshots/<name>/
//!         snapshot.json
//!         config.json
//!         disks/<id>.img
//!       serial.log       guest serial console output
//! ```

#![forbid(unsafe_op_in_unsafe_fn)]

pub mod config;
pub mod disk;
pub mod error;
pub mod event;
pub mod name;
pub mod net;
pub mod runtime;
pub mod snapshot;
pub mod store;
pub mod time;
pub mod util;

pub use config::{
    AudioConfig, BalloonConfig, BootConfig, ConsoleConfig, ConsolePortConfig, DevicesConfig,
    DisplayConfig, DiskConfig, GraphicsConfig, KeyboardConfig, KeyboardKind, NetworkConfig, NetworkMode,
    PlatformConfig, PointingConfig, PointingKind, SerialConfig, ShareConfig, SocketConfig,
    UsbConfig, VmConfig, VmId, SCHEMA_VERSION,
};
pub use disk::{DiskFormat, DiskInfo};
pub use error::{CoreError, Result};
pub use event::{Event, EventKind};
pub use name::{validate_disk_id, validate_share_tag, validate_snapshot_name, validate_vm_name};
pub use runtime::{RunState, RuntimeState};
pub use snapshot::{Snapshot, SnapshotDisk, SnapshotFile, SnapshotSummary};
pub use store::{CreateVmOptions, NewDisk, NewNetwork, Vm, VmInfo, VmStore, VmSummary};
pub use time::Timestamp;
