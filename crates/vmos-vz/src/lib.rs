//! Apple Virtualization.framework backend.
//!
//! The backend is intentionally a thin, explicit layer: the daemon owns
//! policy, the store owns state, and this crate owns the Objective-C objects
//! and their one required serial dispatch queue.
//!
//! ```no_run
//! use vmos_vz::{VzBoot, VzDisk, VzMachine, VzNetwork, VzNetworkMode, VzSerial, VzSpec};
//! use std::path::PathBuf;
//!
//! # fn main() -> Result<(), vmos_vz::VzError> {
//! let spec = VzSpec {
//!     cpus: 4,
//!     memory_mib: 4096,
//!     boot: VzBoot::Linux {
//!         kernel: PathBuf::from("/tmp/vmlinuz"),
//!         initrd: None,
//!         command_line: "console=hvc0".into(),
//!     },
//!     disks: vec![VzDisk { path: PathBuf::from("/tmp/root.img"), read_only: false }],
//!     networks: vec![VzNetwork { mac: None, mode: VzNetworkMode::Nat }],
//!     shares: vec![],
//!     serial: Some(VzSerial { path: PathBuf::from("/tmp/serial.log"), append: true }),
//!     platform_identifier: None,
//!     graphics: None,
//! };
//! let machine = VzMachine::new(spec, "com.example.vmos")?;
//! machine.start()?;
//! machine.shutdown(std::time::Duration::from_secs(30))?;
//! # Ok(())
//! # }
//! ```

#![cfg(target_os = "macos")]

mod config;
mod delegate;
mod error;
mod machine;
mod state;
mod util;

pub use config::{
    bridged_interfaces, validate_share_tag, BridgedInterface, VzBoot, VzDisk, VzGraphics,
    VzNetwork, VzNetworkMode, VzSerial, VzShare, VzSpec,
};
pub use error::VzError;
pub use machine::{HostLimits, VzMachine};
pub use state::VmState;
