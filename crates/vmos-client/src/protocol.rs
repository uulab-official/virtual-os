//! Wire protocol shared by `vmos-daemon` and `vmos`.
//!
//! The transport is newline-delimited JSON-RPC 2.0 over a Unix domain socket.
//! One request/response pair per line keeps the protocol trivially debuggable
//! with `nc -U` and avoids framing bugs.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use vmos_core::{
    BootConfig, DiskInfo, Event, RunState, SerialConfig, ShareConfig, Snapshot, SnapshotSummary,
    VmConfig, VmInfo, VmSummary,
};

pub const JSONRPC_VERSION: &str = "2.0";
pub const PROTOCOL_VERSION: u32 = 1;
pub const MAX_LINE_BYTES: usize = 4 * 1024 * 1024;

pub mod methods {
    pub const SYSTEM_PING: &str = "system.ping";
    pub const SYSTEM_INFO: &str = "system.info";
    pub const SYSTEM_CAPABILITIES: &str = "system.capabilities";
    pub const SYSTEM_SHUTDOWN: &str = "system.shutdown";

    pub const VM_CREATE: &str = "vm.create";
    pub const VM_LIST: &str = "vm.list";
    pub const VM_GET: &str = "vm.get";
    pub const VM_START: &str = "vm.start";
    pub const VM_STOP: &str = "vm.stop";
    pub const VM_PAUSE: &str = "vm.pause";
    pub const VM_RESUME: &str = "vm.resume";
    pub const VM_DELETE: &str = "vm.delete";
    pub const VM_CLONE: &str = "vm.clone";
    pub const VM_MODIFY: &str = "vm.modify";
    pub const VM_LOGS: &str = "vm.logs";
    pub const VM_STATS: &str = "vm.stats";
    pub const VM_INSTALL: &str = "vm.install";
    pub const VM_SAVE: &str = "vm.save";
    pub const VM_RESTORE: &str = "vm.restore";

    pub const SNAPSHOT_LIST: &str = "snapshot.list";
    pub const SNAPSHOT_CREATE: &str = "snapshot.create";
    pub const SNAPSHOT_RESTORE: &str = "snapshot.restore";
    pub const SNAPSHOT_DELETE: &str = "snapshot.delete";

    pub const DISK_LIST: &str = "disk.list";
    pub const DISK_ADD: &str = "disk.add";
    pub const DISK_RESIZE: &str = "disk.resize";
    pub const DISK_REMOVE: &str = "disk.remove";
    pub const DISK_CLONE: &str = "disk.clone";
    pub const DISK_COMPACT: &str = "disk.compact";

    pub const EVENT_POLL: &str = "events.poll";
}

pub mod error_codes {
    pub const PARSE_ERROR: i64 = -32700;
    pub const INVALID_REQUEST: i64 = -32600;
    pub const METHOD_NOT_FOUND: i64 = -32601;
    pub const INVALID_PARAMS: i64 = -32602;
    pub const INTERNAL_ERROR: i64 = -32603;

    pub const VM_NOT_FOUND: i64 = -32001;
    pub const ALREADY_EXISTS: i64 = -32002;
    pub const VM_BUSY: i64 = -32003;
    pub const UNSUPPORTED: i64 = -32004;
    pub const NOT_INITIALIZED: i64 = -32005;
    pub const ENTITLEMENT_MISSING: i64 = -32006;
    pub const PERMISSION_DENIED: i64 = -32007;
    pub const TIMEOUT: i64 = -32008;
    pub const DAEMON_ERROR: i64 = -32000;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default)]
    pub params: Value,
}

impl RpcRequest {
    pub fn new(id: u64, method: impl Into<String>, params: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.into(),
            params,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<RpcError>,
}

impl RpcResponse {
    pub fn success(id: u64, result: Value) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn failure(id: u64, error: RpcError) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(error),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
#[error("{message} (code {code})")]
pub struct RpcError {
    pub code: i64,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl RpcError {
    pub fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with_data(mut self, data: Value) -> Self {
        self.data = Some(data);
        self
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Empty {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PingResult {
    pub version: String,
    pub protocol_version: u32,
    pub pid: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemInfo {
    pub version: String,
    pub protocol_version: u32,
    pub pid: u32,
    pub uptime_secs: u64,
    pub store_root: PathBuf,
    pub vm_count: usize,
    pub vz_supported: bool,
    pub features: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgedInterfaceInfo {
    pub identifier: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Capabilities {
    pub vz_supported: bool,
    pub min_cpus: usize,
    pub max_cpus: usize,
    pub min_memory_bytes: u64,
    pub max_memory_bytes: u64,
    pub bridged_interfaces: Vec<BridgedInterfaceInfo>,
    pub default_store_root: PathBuf,
    pub daemon_socket: PathBuf,
    pub os: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskSpec {
    pub id: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(flatten)]
    pub mode: vmos_core::NetworkMode,
}

impl NetworkSpec {
    pub fn nat() -> Self {
        Self {
            mac: None,
            mode: vmos_core::NetworkMode::Nat,
        }
    }

    pub fn bridged(interface: Option<String>) -> Self {
        Self {
            mac: None,
            mode: vmos_core::NetworkMode::Bridged { interface },
        }
    }

    pub fn with_mac(mut self, mac: impl Into<String>) -> Self {
        self.mac = Some(mac.into());
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmCreateParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub cpus: u32,
    pub memory_mib: u64,
    pub boot: BootConfig,
    #[serde(default)]
    pub disks: Vec<DiskSpec>,
    #[serde(default)]
    pub networks: Vec<NetworkSpec>,
    #[serde(default)]
    pub shares: Vec<ShareConfig>,
    #[serde(default)]
    pub serial: SerialConfig,
    #[serde(default)]
    pub devices: vmos_core::DevicesConfig,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
}

impl VmCreateParams {
    pub fn new(name: impl Into<String>, boot: BootConfig, cpus: u32, memory_mib: u64) -> Self {
        Self {
            name: name.into(),
            display_name: None,
            cpus,
            memory_mib,
            boot,
            disks: Vec::new(),
            networks: vec![NetworkSpec::nat()],
            shares: Vec::new(),
            serial: SerialConfig::default(),
            devices: vmos_core::DevicesConfig::default(),
            tags: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmNameParams {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmDeleteParams {
    pub name: String,
    #[serde(default)]
    pub force: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmStopParams {
    pub name: String,
    #[serde(default)]
    pub force: bool,
    #[serde(default = "default_shutdown_timeout")]
    pub timeout_secs: u64,
}

fn default_shutdown_timeout() -> u64 {
    30
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmModifyParams {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory_mib: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmCloneParams {
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmListResult {
    pub vms: Vec<VmSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmGetResult {
    pub vm: VmInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmStateResult {
    pub name: String,
    pub state: RunState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmSaveParams {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmRestoreParams {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmInstallParams {
    pub name: String,
    pub restore_image: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmStatsParams {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmStatsResult {
    pub name: String,
    pub state: vmos_core::RunState,
    pub cpus: u32,
    pub memory_mib: u64,
    pub disks: Vec<DiskInfo>,
    pub serial_log_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsParams {
    pub name: String,
    #[serde(default = "default_log_lines")]
    pub lines: usize,
}

fn default_log_lines() -> usize {
    200
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogsResult {
    pub name: String,
    pub path: PathBuf,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotCreateParams {
    pub vm: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotNameParams {
    pub vm: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotListParams {
    pub vm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotListResult {
    pub snapshots: Vec<SnapshotSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotResult {
    pub snapshot: Snapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskAddParams {
    pub vm: String,
    pub id: String,
    pub size_bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskResizeParams {
    pub vm: String,
    pub disk: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskRemoveParams {
    pub vm: String,
    pub disk: String,
    #[serde(default)]
    pub delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskCompactParams {
    pub vm: String,
    pub disk: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskCloneParams {
    pub vm: String,
    pub disk: String,
    pub target: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskListParams {
    pub vm: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskListResult {
    pub disks: Vec<DiskInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskResult {
    pub disk: DiskInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPollParams {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub after: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventPollResult {
    pub events: Vec<Event>,
    pub next: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShutdownResult {
    pub ok: bool,
}

/// Convenience helper for the common "config was captured with the VM" field.
pub fn config_of(vm: &VmInfo) -> &VmConfig {
    &vm.config
}
