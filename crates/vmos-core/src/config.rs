//! Versioned VM definition persisted as `vm.json`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{CoreError, Result};
use crate::name::{validate_disk_id, validate_share_tag, validate_vm_name};
use crate::time::Timestamp;

/// Current `vm.json` schema version.
pub const SCHEMA_VERSION: u32 = 1;

/// Maximum number of virtual CPUs accepted by the product surface.
///
/// The Virtualization.framework enforces the tighter hardware-specific limit
/// (`VZVirtualMachineConfiguration.maximumAllowedCPUCount`) at start time.
pub const MAX_CPUS: u32 = 128;

/// Maximum guest memory accepted by the product surface (1 TiB).
pub const MAX_MEMORY_MIB: u64 = 1024 * 1024;

/// Minimum guest memory accepted by the product surface.
pub const MIN_MEMORY_MIB: u64 = 128;

/// Stable VM identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct VmId(String);

impl VmId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(value: impl Into<String>) -> Result<Self> {
        let value = value.into();
        Uuid::parse_str(&value).map_err(|_| CoreError::InvalidId {
            value: value.clone(),
            reason: "expected a UUID",
        })?;
        Ok(Self(value))
    }
}

impl Default for VmId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for VmId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Where the guest gets its initial code from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum BootConfig {
    /// Direct Linux boot using `VZLinuxBootLoader`.
    Linux {
        kernel: PathBuf,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        initrd: Option<PathBuf>,
        #[serde(default)]
        command_line: String,
    },
    /// UEFI boot using `VZEFIBootLoader` and a persistent variable store.
    Efi {
        /// Relative to the VM directory unless absolute.
        variable_store: PathBuf,
    },
    /// Boot an installed macOS guest through `VZMacOSBootLoader`.
    MacOs {
        /// Restore image used for installation (`VZMacOSRestoreImage`).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        restore_image: Option<PathBuf>,
        /// Path to a serialized `VZMacHardwareModel`.
        hardware_model: PathBuf,
        /// Path to a serialized `VZMacMachineIdentifier`.
        machine_identifier: PathBuf,
        /// Path to the macOS auxiliary storage image.
        auxiliary_storage: PathBuf,
    },
}

impl BootConfig {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Linux { .. } => "linux",
            Self::Efi { .. } => "efi",
            Self::MacOs { .. } => "macos",
        }
    }

    /// Human-readable primary boot source.
    pub fn source(&self) -> String {
        match self {
            Self::Linux { kernel, .. } => kernel.display().to_string(),
            Self::Efi { variable_store } => variable_store.display().to_string(),
            Self::MacOs { .. } => "macOS".to_string(),
        }
    }
}

/// One block device exposed to the guest over virtio-blk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiskConfig {
    pub id: String,
    /// Relative to the VM directory unless absolute.
    pub path: PathBuf,
    pub size_bytes: u64,
    #[serde(default)]
    pub read_only: bool,
}

impl DiskConfig {
    pub fn new(id: impl Into<String>, path: impl Into<PathBuf>, size_bytes: u64) -> Self {
        Self {
            id: id.into(),
            path: path.into(),
            size_bytes,
            read_only: false,
        }
    }

    pub fn resolve(&self, vm_dir: &Path) -> PathBuf {
        resolve_vm_path(vm_dir, &self.path)
    }

    pub fn validate(&self) -> Result<()> {
        validate_disk_id(&self.id)?;
        if self.path.as_os_str().is_empty() {
            return Err(CoreError::invalid_config(format!(
                "disk {} has an empty path",
                self.id
            )));
        }
        if self.path.is_relative() {
            crate::name::validate_relative_path(&self.path.to_string_lossy())?;
        }
        if self.size_bytes == 0 {
            return Err(CoreError::invalid_config(format!(
                "disk {} has zero size",
                self.id
            )));
        }
        Ok(())
    }
}

/// How a virtual NIC reaches the outside world.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NetworkMode {
    /// Shared NAT provided by Virtualization.framework.
    #[default]
    Nat,
    /// Layer-2 bridge to a host interface. Requires the
    /// `com.apple.vm.networking` entitlement.
    Bridged {
        /// Host interface name such as `en0`. `None` lets the framework choose.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        interface: Option<String>,
    },
    /// Logical vmnet network (shared/isolated topology managed by vmnet).
    Vmnet,
}

impl NetworkMode {
    pub fn label(&self) -> String {
        match self {
            Self::Nat => "nat".to_string(),
            Self::Bridged { interface } => match interface {
                Some(name) => format!("bridged:{name}"),
                None => "bridged".to_string(),
            },
            Self::Vmnet => "vmnet".to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
    #[serde(flatten)]
    pub mode: NetworkMode,
}

impl NetworkConfig {
    pub fn new(id: impl Into<String>, mode: NetworkMode) -> Self {
        Self {
            id: id.into(),
            mac: Some(crate::net::generate_mac()),
            mode,
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_disk_id(&self.id)?;
        if let Some(mac) = &self.mac {
            crate::net::normalize_mac(mac).map_err(|reason| {
                CoreError::invalid_config(format!("network {} has invalid MAC: {reason}", self.id))
            })?;
        }
        if let NetworkMode::Bridged {
            interface: Some(interface),
        } = &self.mode
        {
            if interface.trim().is_empty() {
                return Err(CoreError::invalid_config(format!(
                    "network {} has an empty bridge interface",
                    self.id
                )));
            }
        }
        Ok(())
    }
}

/// A virtiofs share. The guest mounts it with `mount -t virtiofs <tag> ...`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShareConfig {
    pub tag: String,
    pub host_path: PathBuf,
    #[serde(default)]
    pub read_only: bool,
}

impl ShareConfig {
    pub fn new(tag: impl Into<String>, host_path: impl Into<PathBuf>, read_only: bool) -> Self {
        Self {
            tag: tag.into(),
            host_path: host_path.into(),
            read_only,
        }
    }

    pub fn validate(&self) -> Result<()> {
        validate_share_tag(&self.tag)?;
        if self.host_path.as_os_str().is_empty() {
            return Err(CoreError::invalid_config(format!(
                "share {:?} has an empty host path",
                self.tag
            )));
        }
        Ok(())
    }
}

/// Guest serial console configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SerialConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Relative to the VM directory unless absolute.
    #[serde(default = "default_serial_log")]
    pub log: PathBuf,
    #[serde(default = "default_true")]
    pub append: bool,
}

fn default_true() -> bool {
    true
}

fn default_serial_log() -> PathBuf {
    PathBuf::from("serial.log")
}

impl Default for SerialConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            log: default_serial_log(),
            append: true,
        }
    }
}

impl SerialConfig {
    pub fn resolve(&self, vm_dir: &Path) -> PathBuf {
        resolve_vm_path(vm_dir, &self.log)
    }
}

/// Graphics device configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GraphicsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_graphics_width")]
    pub width: u32,
    #[serde(default = "default_graphics_height")]
    pub height: u32,
    #[serde(default = "default_graphics_ppi")]
    pub pixels_per_inch: u32,
}

impl Default for GraphicsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            width: 1920,
            height: 1080,
            pixels_per_inch: 80,
        }
    }
}

/// Audio device configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AudioConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub input: bool,
    #[serde(default = "default_true")]
    pub output: bool,
}

impl Default for AudioConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            input: true,
            output: true,
        }
    }
}

/// USB controller configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UsbConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_true")]
    pub autoconnect: bool,
}

impl Default for UsbConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            autoconnect: true,
        }
    }
}

/// Pointing device kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum PointingKind {
    #[default]
    Usb,
    Trackpad,
    None,
}

/// Pointing device configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PointingConfig {
    #[serde(default)]
    pub kind: PointingKind,
}

impl Default for PointingConfig {
    fn default() -> Self {
        Self {
            kind: PointingKind::Usb,
        }
    }
}

/// Keyboard device kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum KeyboardKind {
    #[default]
    Usb,
    Mac,
    None,
}

/// Keyboard device configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KeyboardConfig {
    #[serde(default)]
    pub kind: KeyboardKind,
}

impl Default for KeyboardConfig {
    fn default() -> Self {
        Self {
            kind: KeyboardKind::Usb,
        }
    }
}

/// A single virtio console port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConsolePortConfig {
    pub name: String,
    /// Relative to the VM directory unless absolute.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub log: Option<PathBuf>,
}

/// Virtio console configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct ConsoleConfig {
    #[serde(default)]
    pub ports: Vec<ConsolePortConfig>,
}

/// Host-guest socket device (vsock) configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SocketConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for SocketConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Memory balloon device configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BalloonConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for BalloonConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Aggregate device configuration for a VM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct DevicesConfig {
    #[serde(default)]
    pub graphics: GraphicsConfig,
    #[serde(default)]
    pub audio: AudioConfig,
    #[serde(default)]
    pub usb: UsbConfig,
    #[serde(default)]
    pub pointing: PointingConfig,
    #[serde(default)]
    pub keyboard: KeyboardConfig,
    #[serde(default)]
    pub console: ConsoleConfig,
    #[serde(default)]
    pub socket: SocketConfig,
    #[serde(default)]
    pub balloon: BalloonConfig,
    #[serde(default = "default_true")]
    pub entropy: bool,
}

fn default_graphics_width() -> u32 {
    1920
}
fn default_graphics_height() -> u32 {
    1080
}
fn default_graphics_ppi() -> u32 {
    80
}

/// Complete VM definition. This is the source of truth on disk.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlatformConfig {
    /// Generic machine identifier used by Linux and EFI guests.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_identifier: Option<PathBuf>,
}

/// Complete VM definition. This is the source of truth on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmConfig {
    pub schema_version: u32,
    pub id: VmId,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    pub created_at: Timestamp,
    pub updated_at: Timestamp,
    pub cpus: u32,
    pub memory_mib: u64,
    pub boot: BootConfig,
    #[serde(default)]
    pub disks: Vec<DiskConfig>,
    #[serde(default)]
    pub networks: Vec<NetworkConfig>,
    #[serde(default)]
    pub shares: Vec<ShareConfig>,
    #[serde(default)]
    pub serial: SerialConfig,
    #[serde(default)]
    pub devices: DevicesConfig,
    #[serde(default)]
    pub platform: PlatformConfig,
    #[serde(default)]
    pub tags: BTreeMap<String, String>,
}

impl VmConfig {
    /// Construct a configuration with sensible defaults for a new VM.
    pub fn new(name: impl Into<String>, boot: BootConfig, cpus: u32, memory_mib: u64) -> Self {
        let now = Timestamp::now();
        Self {
            schema_version: SCHEMA_VERSION,
            id: VmId::new(),
            name: name.into(),
            display_name: None,
            created_at: now,
            updated_at: now,
            cpus,
            memory_mib,
            boot,
            disks: Vec::new(),
            networks: Vec::new(),
            shares: Vec::new(),
            serial: SerialConfig::default(),
            devices: DevicesConfig::default(),
            platform: PlatformConfig::default(),
            tags: BTreeMap::new(),
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.schema_version != SCHEMA_VERSION {
            return Err(CoreError::invalid_config(format!(
                "unsupported schema version {} (this build supports {})",
                self.schema_version, SCHEMA_VERSION
            )));
        }
        validate_vm_name(&self.name)?;
        if self.cpus == 0 {
            return Err(CoreError::invalid_config("cpus must be at least 1"));
        }
        if self.cpus > MAX_CPUS {
            return Err(CoreError::invalid_config(format!(
                "cpus must be at most {MAX_CPUS}"
            )));
        }
        if self.memory_mib < MIN_MEMORY_MIB {
            return Err(CoreError::invalid_config(format!(
                "memory must be at least {MIN_MEMORY_MIB} MiB"
            )));
        }
        if self.memory_mib > MAX_MEMORY_MIB {
            return Err(CoreError::invalid_config(format!(
                "memory must be at most {MAX_MEMORY_MIB} MiB"
            )));
        }

        match &self.boot {
            BootConfig::Linux { kernel, .. } => {
                if kernel.as_os_str().is_empty() {
                    return Err(CoreError::invalid_config("kernel path is empty"));
                }
            }
            BootConfig::Efi { variable_store } => {
                if variable_store.as_os_str().is_empty() {
                    return Err(CoreError::invalid_config(
                        "EFI variable store path is empty",
                    ));
                }
                if variable_store.is_relative() {
                    crate::name::validate_relative_path(&variable_store.to_string_lossy())?;
                }
            }
            BootConfig::MacOs {
                hardware_model,
                machine_identifier,
                auxiliary_storage,
                ..
            } => {
                for (label, path) in [
                    ("hardware model", hardware_model),
                    ("machine identifier", machine_identifier),
                    ("auxiliary storage", auxiliary_storage),
                ] {
                    if path.as_os_str().is_empty() {
                        return Err(CoreError::invalid_config(format!(
                            "macOS {label} path is empty"
                        )));
                    }
                    if path.is_relative() {
                        crate::name::validate_relative_path(&path.to_string_lossy())?;
                    }
                }
            }
        }

        let mut disk_ids = std::collections::BTreeSet::new();
        let mut disk_paths = std::collections::BTreeSet::new();
        for disk in &self.disks {
            disk.validate()?;
            if !disk_ids.insert(&disk.id) {
                return Err(CoreError::invalid_config(format!(
                    "duplicate disk id {:?}",
                    disk.id
                )));
            }
            if !disk_paths.insert(&disk.path) {
                return Err(CoreError::invalid_config(format!(
                    "duplicate disk path {}",
                    disk.path.display()
                )));
            }
        }

        let mut network_ids = std::collections::BTreeSet::new();
        for network in &self.networks {
            network.validate()?;
            if !network_ids.insert(&network.id) {
                return Err(CoreError::invalid_config(format!(
                    "duplicate network id {:?}",
                    network.id
                )));
            }
        }

        let mut share_tags = std::collections::BTreeSet::new();
        for share in &self.shares {
            share.validate()?;
            if !share_tags.insert(&share.tag) {
                return Err(CoreError::invalid_config(format!(
                    "duplicate shared directory tag {:?}",
                    share.tag
                )));
            }
        }

        if self.serial.enabled {
            if self.serial.log.as_os_str().is_empty() {
                return Err(CoreError::invalid_config("serial log path is empty"));
            }
            if self.serial.log.is_relative() {
                crate::name::validate_relative_path(&self.serial.log.to_string_lossy())?;
            }
        }

        if let Some(path) = &self.platform.machine_identifier {
            if path.is_relative() {
                crate::name::validate_relative_path(&path.to_string_lossy())?;
            }
        }

        Ok(())
    }

    pub fn touch(&mut self) {
        self.updated_at = Timestamp::now();
    }

    pub fn disk(&self, id: &str) -> Option<&DiskConfig> {
        self.disks.iter().find(|disk| disk.id == id)
    }

    pub fn disk_mut(&mut self, id: &str) -> Option<&mut DiskConfig> {
        self.disks.iter_mut().find(|disk| disk.id == id)
    }

    /// Total provisioned disk capacity.
    pub fn provisioned_bytes(&self) -> u64 {
        self.disks.iter().map(|disk| disk.size_bytes).sum()
    }
}

/// Resolve a config path against the VM directory. Absolute paths win.
pub fn resolve_vm_path(vm_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        vm_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn linux_config() -> VmConfig {
        VmConfig::new(
            "test",
            BootConfig::Linux {
                kernel: PathBuf::from("/vmlinuz"),
                initrd: None,
                command_line: "console=hvc0".into(),
            },
            2,
            1024,
        )
    }

    #[test]
    fn valid_config_passes() {
        linux_config().validate().unwrap();
    }

    #[test]
    fn json_round_trip_is_stable() {
        let mut config = linux_config();
        config
            .disks
            .push(DiskConfig::new("root", "disks/root.img", 20 << 30));
        config
            .networks
            .push(NetworkConfig::new("net0", NetworkMode::Nat));
        config
            .shares
            .push(ShareConfig::new("workspace", "/tmp", true));
        let json = serde_json::to_string_pretty(&config).unwrap();
        let parsed: VmConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, config);
        assert!(json.contains("\"mode\": \"nat\""));
    }

    #[test]
    fn rejects_bad_memory_and_cpu() {
        let mut config = linux_config();
        config.cpus = 0;
        assert!(config.validate().is_err());
        config.cpus = 1;
        config.memory_mib = 1;
        assert!(config.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_disk_ids() {
        let mut config = linux_config();
        config
            .disks
            .push(DiskConfig::new("root", "disks/a.img", 1024));
        config
            .disks
            .push(DiskConfig::new("root", "disks/b.img", 1024));
        assert!(config
            .validate()
            .unwrap_err()
            .to_string()
            .contains("duplicate"));
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let config = linux_config();
        let mut value = serde_json::to_value(&config).unwrap();
        value["mystery"] = serde_json::json!(true);
        assert!(serde_json::from_value::<VmConfig>(value).is_err());
    }
}
