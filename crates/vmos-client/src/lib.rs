//! JSON-RPC client and wire protocol for virtual-os.

mod client;
pub mod protocol;

pub use client::{Client, ClientError, DEFAULT_TIMEOUT, LONG_TIMEOUT};
pub use protocol::*;
