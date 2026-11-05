//! virtual-os daemon.
//!
//! One daemon per user store owns every running Virtualization.framework VM.
//! Clients speak newline-delimited JSON-RPC 2.0 over `$VOS_HOME/daemon.sock`.
//! The daemon serializes lifecycle operations, mirrors state into each VM's
//! `runtime.json`, and records a bounded event history.

mod error;
mod events;
mod manager;
mod server;

use std::path::{Path, PathBuf};

pub use error::DaemonError;
pub use events::EventBus;
pub use manager::Daemon;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, Clone, Default)]
pub struct DaemonOptions {
    /// Override `$VOS_HOME`.
    pub store_root: Option<PathBuf>,
    /// Keep the daemon attached to the terminal for debugging.
    pub foreground: bool,
}

/// Run the daemon until `system.shutdown` is received.
pub fn run(options: DaemonOptions) -> Result<(), DaemonError> {
    server::run(options)
}

/// Whether a store has been initialized.
pub fn store_initialized(root: &Path) -> bool {
    root.join("vms").is_dir()
}
