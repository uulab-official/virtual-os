//! VM store: the filesystem layout and all mutating operations.

use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::path::{Component, Path, PathBuf};

use fs2::FileExt;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::{resolve_vm_path, BootConfig, DiskConfig, NetworkConfig, VmConfig};
use crate::disk::{self, DiskInfo};
use crate::error::{CoreError, Result};
use crate::name::{validate_relative_path, validate_snapshot_name, validate_vm_name};
use crate::runtime::{RunState, RuntimeState};
use crate::snapshot::{Snapshot, SnapshotDisk, SnapshotFile, SnapshotSummary};
use crate::time::Timestamp;
use crate::util;

const VMS_DIR: &str = "vms";
const CONFIG_FILE: &str = "vm.json";
const RUNTIME_FILE: &str = "runtime.json";
const LOCK_FILE: &str = ".lock";
const SNAPSHOTS_DIR: &str = "snapshots";
const SNAPSHOT_FILE: &str = "snapshot.json";

/// New disk request used by `create`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewDisk {
    pub id: String,
    pub size_bytes: u64,
    /// Relative to the VM directory unless absolute.
    pub path: Option<PathBuf>,
}

impl NewDisk {
    pub fn new(id: impl Into<String>, size_bytes: u64) -> Self {
        Self {
            id: id.into(),
            size_bytes,
            path: None,
        }
    }

    pub fn with_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.path = Some(path.into());
        self
    }
}

/// New network request used by `create`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewNetwork {
    pub mode: crate::config::NetworkMode,
    /// Optional MAC address. Generated when absent.
    pub mac: Option<String>,
}

impl NewNetwork {
    pub fn nat() -> Self {
        Self {
            mode: crate::config::NetworkMode::Nat,
            mac: None,
        }
    }

    pub fn bridged(interface: Option<String>) -> Self {
        Self {
            mode: crate::config::NetworkMode::Bridged { interface },
            mac: None,
        }
    }

    pub fn with_mode(mode: crate::config::NetworkMode) -> Self {
        Self { mode, mac: None }
    }

    pub fn with_mac(mut self, mac: impl Into<String>) -> Self {
        self.mac = Some(mac.into());
        self
    }
}

/// Input for creating a VM.
#[derive(Debug, Clone)]
pub struct CreateVmOptions {
    pub name: String,
    pub display_name: Option<String>,
    pub cpus: u32,
    pub memory_mib: u64,
    pub boot: BootConfig,
    pub disks: Vec<NewDisk>,
    pub networks: Vec<NewNetwork>,
    pub shares: Vec<crate::config::ShareConfig>,
    pub serial: crate::config::SerialConfig,
    pub tags: std::collections::BTreeMap<String, String>,
}

impl CreateVmOptions {
    pub fn new(name: impl Into<String>, boot: BootConfig) -> Self {
        Self {
            name: name.into(),
            display_name: None,
            cpus: 2,
            memory_mib: 2048,
            boot,
            disks: Vec::new(),
            networks: vec![NewNetwork::nat()],
            shares: Vec::new(),
            serial: crate::config::SerialConfig::default(),
            tags: Default::default(),
        }
    }

    pub fn with_cpus(mut self, cpus: u32) -> Self {
        self.cpus = cpus;
        self
    }

    pub fn with_memory_mib(mut self, memory_mib: u64) -> Self {
        self.memory_mib = memory_mib;
        self
    }

    pub fn with_display_name(mut self, name: impl Into<String>) -> Self {
        self.display_name = Some(name.into());
        self
    }

    pub fn with_disk(mut self, disk: NewDisk) -> Self {
        self.disks.push(disk);
        self
    }

    pub fn with_network(mut self, mode: crate::config::NetworkMode) -> Self {
        self.networks.push(NewNetwork::with_mode(mode));
        self
    }

    pub fn with_network_spec(mut self, network: NewNetwork) -> Self {
        self.networks.push(network);
        self
    }

    pub fn with_share(mut self, share: crate::config::ShareConfig) -> Self {
        self.shares.push(share);
        self
    }

    pub fn with_serial(mut self, serial: crate::config::SerialConfig) -> Self {
        self.serial = serial;
        self
    }
}

/// A VM loaded from the store.
#[derive(Debug, Clone)]
pub struct Vm {
    pub dir: PathBuf,
    pub config: VmConfig,
}

/// Compact row used by `vm.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmSummary {
    pub name: String,
    pub id: String,
    pub state: RunState,
    pub cpus: u32,
    pub memory_mib: u64,
    pub boot_kind: String,
    pub disk_count: usize,
    pub provisioned_bytes: u64,
    pub created_at: Timestamp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
}

/// Full VM view used by `vm.get` and `vmos show`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VmInfo {
    pub name: String,
    pub id: String,
    pub dir: PathBuf,
    pub state: RunState,
    pub config: VmConfig,
    pub runtime: RuntimeState,
    pub disks: Vec<DiskInfo>,
    pub snapshots: Vec<SnapshotSummary>,
    pub serial_log: PathBuf,
}

/// An exclusive per-VM lock held for the duration of a mutation.
pub struct VmLock {
    _file: File,
}

/// Owner of the `$VOS_HOME` directory.
#[derive(Debug, Clone)]
pub struct VmStore {
    root: PathBuf,
    vms_dir: PathBuf,
}

impl VmStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        let vms_dir = root.join(VMS_DIR);
        Self { root, vms_dir }
    }

    /// Resolve `$VOS_HOME`, falling back to `~/.virtual-os`.
    pub fn default_root() -> PathBuf {
        if let Some(value) = std::env::var_os("VOS_HOME") {
            let path = PathBuf::from(value);
            if !path.as_os_str().is_empty() {
                return path;
            }
        }
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(".virtual-os");
        }
        PathBuf::from(".virtual-os")
    }

    pub fn open_default() -> Self {
        Self::new(Self::default_root())
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn vms_dir(&self) -> &Path {
        &self.vms_dir
    }

    pub fn socket_path(&self) -> PathBuf {
        self.root.join("daemon.sock")
    }

    pub fn pid_path(&self) -> PathBuf {
        self.root.join("daemon.pid")
    }

    pub fn daemon_log_path(&self) -> PathBuf {
        self.root.join("daemon.log")
    }

    pub fn daemon_lock_path(&self) -> PathBuf {
        self.root.join("daemon.lock")
    }

    pub fn is_initialized(&self) -> bool {
        self.vms_dir.is_dir()
    }

    pub fn require_initialized(&self) -> Result<()> {
        if self.is_initialized() {
            Ok(())
        } else {
            Err(CoreError::NotInitialized {
                root: self.root.clone(),
            })
        }
    }

    /// Create the store directory tree. Idempotent.
    pub fn init(&self) -> Result<()> {
        util::create_private_dir_all(&self.root)?;
        util::create_private_dir_all(&self.vms_dir)?;
        Ok(())
    }

    pub fn vm_dir(&self, name: &str) -> Result<PathBuf> {
        validate_vm_name(name)?;
        Ok(self.vms_dir.join(name))
    }

    pub fn open(&self, name: &str) -> Result<Vm> {
        self.require_initialized()?;
        let dir = self.vm_dir(name)?;
        if !dir.is_dir() {
            return Err(CoreError::VmNotFound(name.to_string()));
        }
        let config_path = dir.join(CONFIG_FILE);
        let config: VmConfig = util::read_json(&config_path)?;
        config.validate()?;
        if config.name != name {
            return Err(CoreError::invalid_config(format!(
                "{} declares name {:?}, expected {name:?}",
                config_path.display(),
                config.name
            )));
        }
        Ok(Vm { dir, config })
    }

    pub fn list(&self) -> Result<Vec<VmSummary>> {
        self.require_initialized()?;
        let mut out = Vec::new();
        let entries =
            std::fs::read_dir(&self.vms_dir).map_err(|e| CoreError::io(&self.vms_dir, e))?;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    tracing::warn!(%error, "skipping unreadable store entry");
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || !entry.path().is_dir() {
                continue;
            }
            match self.open(&name) {
                Ok(vm) => out.push(vm.summary()),
                Err(error) => {
                    tracing::warn!(name, %error, "skipping invalid VM directory");
                }
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub fn create(&self, options: CreateVmOptions) -> Result<Vm> {
        self.require_initialized()?;
        validate_vm_name(&options.name)?;
        let final_dir = self.vms_dir.join(&options.name);
        if final_dir.exists() {
            return Err(CoreError::VmExists(options.name));
        }

        let tmp_dir = self.vms_dir.join(format!(
            ".{}.create.{}",
            options.name,
            Uuid::new_v4().simple()
        ));
        util::create_private_dir_all(&tmp_dir)?;

        let mut created_disks: Vec<PathBuf> = Vec::new();
        let result = (|| -> Result<VmConfig> {
            let mut config = VmConfig::new(
                options.name.clone(),
                options.boot.clone(),
                options.cpus,
                options.memory_mib,
            );
            config.display_name = options.display_name.clone();
            config.tags = options.tags.clone();
            config.serial = options.serial.clone();

            // Generic platform identity for Linux and EFI guests, persisted so
            // the guest sees a stable machine identifier across boots.
            if !matches!(config.boot, BootConfig::MacOs { .. }) {
                config.platform.machine_identifier = Some(PathBuf::from("platform/machine-id.bin"));
            }

            if let BootConfig::Efi { variable_store } = &config.boot {
                if variable_store.is_relative() {
                    validate_relative_path(&variable_store.to_string_lossy())?;
                    let path = tmp_dir.join(variable_store);
                    if let Some(parent) = path.parent() {
                        util::create_private_dir_all(parent)?;
                    }
                }
            }

            for (index, new_disk) in options.disks.iter().enumerate() {
                crate::name::validate_disk_id(&new_disk.id)?;
                if new_disk.size_bytes == 0 {
                    return Err(CoreError::invalid_config(format!(
                        "disk {} size must be greater than zero",
                        new_disk.id
                    )));
                }
                if config.disks.iter().any(|d| d.id == new_disk.id) {
                    return Err(CoreError::invalid_config(format!(
                        "duplicate disk id {:?}",
                        new_disk.id
                    )));
                }
                let (config_path, file_path) = match &new_disk.path {
                    Some(path) if path.is_absolute() => (path.clone(), path.clone()),
                    Some(path) => {
                        validate_relative_path(&path.to_string_lossy())?;
                        let file = tmp_dir.join(path);
                        (path.clone(), file)
                    }
                    None => {
                        let rel = PathBuf::from("disks").join(format!("{}.img", new_disk.id));
                        (rel.clone(), tmp_dir.join(rel))
                    }
                };
                disk::create_sparse(&file_path, new_disk.size_bytes)?;
                created_disks.push(file_path.clone());
                config.disks.push(DiskConfig {
                    id: new_disk.id.clone(),
                    path: config_path,
                    size_bytes: new_disk.size_bytes,
                    read_only: false,
                });
                let _ = index;
            }

            for (index, network) in options.networks.iter().enumerate() {
                let mut network_config =
                    NetworkConfig::new(format!("net{index}"), network.mode.clone());
                if let Some(mac) = &network.mac {
                    let normalized = crate::net::normalize_mac(mac).map_err(|reason| {
                        CoreError::invalid_config(format!(
                            "network net{index} has invalid MAC: {reason}"
                        ))
                    })?;
                    network_config.mac = Some(normalized);
                }
                config.networks.push(network_config);
            }

            for share in &options.shares {
                config.shares.push(share.clone());
            }

            config.validate()?;
            util::write_json(&tmp_dir.join(CONFIG_FILE), &config)?;
            util::write_json(&tmp_dir.join(RUNTIME_FILE), &RuntimeState::stopped())?;
            if config.serial.enabled && config.serial.log.is_relative() {
                validate_relative_path(&config.serial.log.to_string_lossy())?;
                let log = tmp_dir.join(&config.serial.log);
                if let Some(parent) = log.parent() {
                    util::create_private_dir_all(parent)?;
                }
                File::create(&log).map_err(|e| CoreError::io(&log, e))?;
                util::set_private_file(&log)?;
            }
            Ok(config)
        })();

        let config = match result {
            Ok(config) => config,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                for path in created_disks {
                    if path.starts_with(&tmp_dir) {
                        continue;
                    }
                    let _ = std::fs::remove_file(path);
                }
                return Err(error);
            }
        };

        if let Err(error) = std::fs::rename(&tmp_dir, &final_dir) {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            for path in created_disks {
                if !path.starts_with(&tmp_dir) {
                    let _ = std::fs::remove_file(path);
                }
            }
            if final_dir.exists() {
                return Err(CoreError::VmExists(options.name));
            }
            return Err(CoreError::io(&final_dir, error));
        }
        util::fsync_dir(&self.vms_dir)?;
        Ok(Vm {
            dir: final_dir,
            config,
        })
    }

    /// Clone a stopped VM into a new VM.
    pub fn clone_vm(&self, source: &str, target: &str) -> Result<Vm> {
        self.require_initialized()?;
        let source_vm = self.open(source)?;
        source_vm.ensure_stopped("clone")?;
        validate_vm_name(target)?;
        if source == target {
            return Err(CoreError::invalid_config(
                "source and target VM names must differ",
            ));
        }
        let final_dir = self.vms_dir.join(target);
        if final_dir.exists() {
            return Err(CoreError::VmExists(target.to_string()));
        }
        if matches!(source_vm.config.boot, BootConfig::MacOs { .. }) {
            return Err(CoreError::Unsupported(
                "cloning macOS guests is not supported yet".into(),
            ));
        }

        let tmp_dir = self
            .vms_dir
            .join(format!(".{target}.clone.{}", Uuid::new_v4().simple()));
        util::create_private_dir_all(&tmp_dir)?;

        let result = (|| -> Result<VmConfig> {
            let mut config = source_vm.config.clone();
            config.id = crate::config::VmId::new();
            config.name = target.to_string();
            config.created_at = Timestamp::now();
            config.updated_at = config.created_at;
            config.platform.machine_identifier = Some(PathBuf::from("platform/machine-id.bin"));
            for network in &mut config.networks {
                network.mac = Some(crate::net::generate_mac());
            }
            config.disks.clear();

            for disk_config in &source_vm.config.disks {
                let src = disk_config.resolve(&source_vm.dir);
                if !src.is_file() {
                    return Err(CoreError::Other(format!(
                        "source disk {} is missing",
                        src.display()
                    )));
                }
                let rel = PathBuf::from("disks").join(format!("{}.img", disk_config.id));
                let dst = tmp_dir.join(&rel);
                disk::clone_file(&src, &dst)?;
                config.disks.push(DiskConfig {
                    id: disk_config.id.clone(),
                    path: rel,
                    size_bytes: disk_config.size_bytes,
                    read_only: disk_config.read_only,
                });
            }

            // Carry firmware state that belongs to the machine definition.
            if let BootConfig::Efi { variable_store } = &source_vm.config.boot {
                let src = resolve_vm_path(&source_vm.dir, variable_store);
                if src.is_file() {
                    let rel = PathBuf::from("nvram/efi-vars.fd");
                    disk::clone_file(&src, &tmp_dir.join(&rel))?;
                    if let BootConfig::Efi { variable_store } = &mut config.boot {
                        *variable_store = rel;
                    }
                }
            }

            config.validate()?;
            util::write_json(&tmp_dir.join(CONFIG_FILE), &config)?;
            util::write_json(&tmp_dir.join(RUNTIME_FILE), &RuntimeState::stopped())?;
            if config.serial.enabled && config.serial.log.is_relative() {
                let log = tmp_dir.join(&config.serial.log);
                if let Some(parent) = log.parent() {
                    util::create_private_dir_all(parent)?;
                }
                File::create(&log).map_err(|e| CoreError::io(&log, e))?;
            }
            Ok(config)
        })();

        let config = match result {
            Ok(config) => config,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                return Err(error);
            }
        };

        if let Err(error) = std::fs::rename(&tmp_dir, &final_dir) {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            if final_dir.exists() {
                return Err(CoreError::VmExists(target.to_string()));
            }
            return Err(CoreError::io(&final_dir, error));
        }
        util::fsync_dir(&self.vms_dir)?;
        Ok(Vm {
            dir: final_dir,
            config,
        })
    }

    /// Remove a VM. Internal disk images are deleted; external images are not.
    pub fn delete(&self, name: &str) -> Result<()> {
        let vm = self.open(name)?;
        let _lock = vm.lock()?;
        vm.ensure_stopped("delete")?;
        let trash = self.root.join("trash");
        util::create_private_dir_all(&trash)?;
        let dest = trash.join(format!("{name}.{}", Uuid::new_v4().simple()));
        std::fs::rename(&vm.dir, &dest).map_err(|e| CoreError::io(&vm.dir, e))?;
        util::fsync_dir(&self.vms_dir)?;
        if let Err(error) = std::fs::remove_dir_all(&dest) {
            tracing::warn!(path = %dest.display(), %error, "failed to clean deleted VM");
        }
        Ok(())
    }
}

impl Vm {
    pub fn name(&self) -> &str {
        &self.config.name
    }

    pub fn config_path(&self) -> PathBuf {
        self.dir.join(CONFIG_FILE)
    }

    pub fn runtime_path(&self) -> PathBuf {
        self.dir.join(RUNTIME_FILE)
    }

    pub fn serial_log_path(&self) -> PathBuf {
        self.config.serial.resolve(&self.dir)
    }

    pub fn snapshots_dir(&self) -> PathBuf {
        self.dir.join(SNAPSHOTS_DIR)
    }

    pub fn snapshot_dir(&self, name: &str) -> Result<PathBuf> {
        validate_snapshot_name(name)?;
        Ok(self.snapshots_dir().join(name))
    }

    pub fn lock(&self) -> Result<VmLock> {
        util::create_private_dir_all(&self.dir)?;
        let path = self.dir.join(LOCK_FILE);
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(&path)
            .map_err(|e| CoreError::io(&path, e))?;
        file.lock_exclusive().map_err(|e| CoreError::io(&path, e))?;
        Ok(VmLock { _file: file })
    }

    pub fn save_config(&self) -> Result<()> {
        self.config.validate()?;
        util::write_json(&self.config_path(), &self.config)
    }

    pub fn load_runtime(&self) -> RuntimeState {
        let path = self.runtime_path();
        match util::read_json::<RuntimeState>(&path) {
            Ok(runtime) => runtime,
            Err(error) => {
                tracing::warn!(path = %path.display(), %error, "using default runtime state");
                RuntimeState::default()
            }
        }
    }

    pub fn effective_runtime(&self) -> RuntimeState {
        self.load_runtime().effective()
    }

    pub fn state(&self) -> RunState {
        self.effective_runtime().state
    }

    pub fn write_runtime(&self, runtime: &RuntimeState) -> Result<()> {
        util::write_json(&self.runtime_path(), runtime)
    }

    pub fn ensure_stopped(&self, action: &'static str) -> Result<()> {
        let runtime = self.effective_runtime();
        if runtime.state.is_active() {
            return Err(CoreError::VmBusy {
                name: self.config.name.clone(),
                state: runtime.state.label().to_string(),
                action,
            });
        }
        Ok(())
    }

    pub fn summary(&self) -> VmSummary {
        let runtime = self.effective_runtime();
        VmSummary {
            name: self.config.name.clone(),
            id: self.config.id.to_string(),
            state: runtime.state,
            cpus: self.config.cpus,
            memory_mib: self.config.memory_mib,
            boot_kind: self.config.boot.kind().to_string(),
            disk_count: self.config.disks.len(),
            provisioned_bytes: self.config.provisioned_bytes(),
            created_at: self.config.created_at,
            last_error: runtime.last_error,
        }
    }

    pub fn disk_infos(&self) -> Vec<DiskInfo> {
        self.config
            .disks
            .iter()
            .map(|disk_config| {
                let path = disk_config.resolve(&self.dir);
                match disk::inspect(
                    &disk_config.id,
                    &path,
                    disk_config.read_only,
                    disk_config.size_bytes,
                ) {
                    Ok(mut info) => {
                        info.size_bytes = info.size_bytes.max(disk_config.size_bytes);
                        info
                    }
                    Err(_) => DiskInfo {
                        id: disk_config.id.clone(),
                        path,
                        format: disk::DiskFormat::Raw,
                        size_bytes: disk_config.size_bytes,
                        allocated_bytes: 0,
                        read_only: disk_config.read_only,
                        present: false,
                    },
                }
            })
            .collect()
    }

    pub fn info(&self) -> Result<VmInfo> {
        let runtime = self.effective_runtime();
        Ok(VmInfo {
            name: self.config.name.clone(),
            id: self.config.id.to_string(),
            dir: self.dir.clone(),
            state: runtime.state,
            config: self.config.clone(),
            runtime,
            disks: self.disk_infos(),
            snapshots: self.list_snapshots()?,
            serial_log: self.serial_log_path(),
        })
    }

    pub fn set_resources(&mut self, cpus: u32, memory_mib: u64) -> Result<()> {
        self.ensure_stopped("resize")?;
        let _lock = self.lock()?;
        let mut updated = self.config.clone();
        updated.cpus = cpus;
        updated.memory_mib = memory_mib;
        updated.touch();
        updated.validate()?;
        util::write_json(&self.config_path(), &updated)?;
        self.config = updated;
        Ok(())
    }

    pub fn add_disk(&mut self, new_disk: NewDisk) -> Result<DiskConfig> {
        self.ensure_stopped("add a disk")?;
        let _lock = self.lock()?;
        crate::name::validate_disk_id(&new_disk.id)?;
        if self.config.disk(&new_disk.id).is_some() {
            return Err(CoreError::DiskExists(new_disk.id));
        }
        if new_disk.size_bytes == 0 {
            return Err(CoreError::invalid_config(
                "disk size must be greater than zero",
            ));
        }
        let config_path = new_disk
            .path
            .clone()
            .unwrap_or_else(|| PathBuf::from("disks").join(format!("{}.img", new_disk.id)));
        if config_path.is_relative() {
            validate_relative_path(&config_path.to_string_lossy())?;
        }
        let file_path = resolve_vm_path(&self.dir, &config_path);
        if file_path.exists() {
            return Err(CoreError::Other(format!(
                "disk image already exists at {}",
                file_path.display()
            )));
        }
        disk::create_sparse(&file_path, new_disk.size_bytes)?;

        let disk_config = DiskConfig {
            id: new_disk.id,
            path: config_path,
            size_bytes: new_disk.size_bytes,
            read_only: false,
        };
        let mut updated = self.config.clone();
        updated.disks.push(disk_config.clone());
        updated.touch();
        if let Err(error) = updated
            .validate()
            .and_then(|_| util::write_json(&self.config_path(), &updated))
        {
            let _ = std::fs::remove_file(&file_path);
            return Err(error);
        }
        self.config = updated;
        Ok(disk_config)
    }

    pub fn resize_disk(&mut self, id: &str, size_bytes: u64) -> Result<DiskConfig> {
        self.ensure_stopped("resize a disk")?;
        let _lock = self.lock()?;
        let existing = self
            .config
            .disk(id)
            .cloned()
            .ok_or_else(|| CoreError::DiskNotFound(id.to_string()))?;
        if size_bytes <= existing.size_bytes {
            return Err(CoreError::invalid_config(format!(
                "new size {} must be larger than current size {}",
                util::format_size(size_bytes),
                util::format_size(existing.size_bytes)
            )));
        }
        let path = existing.resolve(&self.dir);
        let old_len = std::fs::metadata(&path)
            .map_err(|e| CoreError::io(&path, e))?
            .len();
        disk::resize(&path, size_bytes)?;

        let mut updated = self.config.clone();
        if let Some(disk_config) = updated.disk_mut(id) {
            disk_config.size_bytes = size_bytes;
        }
        updated.touch();
        if let Err(error) = updated
            .validate()
            .and_then(|_| util::write_json(&self.config_path(), &updated))
        {
            let _ = disk::resize(&path, old_len);
            return Err(error);
        }
        self.config = updated;
        Ok(self.config.disk(id).expect("disk exists").clone())
    }

    pub fn remove_disk(&mut self, id: &str, delete_file: bool) -> Result<DiskConfig> {
        self.ensure_stopped("remove a disk")?;
        let _lock = self.lock()?;
        let existing = self
            .config
            .disk(id)
            .cloned()
            .ok_or_else(|| CoreError::DiskNotFound(id.to_string()))?;
        let mut updated = self.config.clone();
        updated.disks.retain(|disk| disk.id != id);
        updated.touch();
        updated.validate()?;
        util::write_json(&self.config_path(), &updated)?;
        self.config = updated;
        if delete_file {
            let path = existing.resolve(&self.dir);
            if let Err(error) = std::fs::remove_file(&path) {
                tracing::warn!(path = %path.display(), %error, "disk detached but file removal failed");
            }
        }
        Ok(existing)
    }

    /// Clone one attached disk to a standalone image file.
    pub fn clone_disk(&self, id: &str, target: &Path) -> Result<DiskInfo> {
        self.ensure_stopped("clone a disk")?;
        let _lock = self.lock()?;
        let source = self
            .config
            .disk(id)
            .ok_or_else(|| CoreError::DiskNotFound(id.to_string()))?;
        let source_path = source.resolve(&self.dir);
        if !source_path.is_file() {
            return Err(CoreError::Other(format!(
                "source disk is missing: {}",
                source_path.display()
            )));
        }
        let target_path = if target.is_absolute() {
            target.to_path_buf()
        } else {
            self.dir.join(target)
        };
        if target_path == source_path {
            return Err(CoreError::invalid_config(
                "clone target must differ from the source disk",
            ));
        }
        if target_path.exists() {
            return Err(CoreError::Other(format!(
                "clone target already exists: {}",
                target_path.display()
            )));
        }
        disk::clone_file(&source_path, &target_path)?;
        disk::inspect(id, &target_path, false, source.size_bytes)
    }

    // ------------------------------------------------------------------
    // Snapshots
    // ------------------------------------------------------------------

    pub fn list_snapshots(&self) -> Result<Vec<SnapshotSummary>> {
        let dir = self.snapshots_dir();
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut out = Vec::new();
        let entries = std::fs::read_dir(&dir).map_err(|e| CoreError::io(&dir, e))?;
        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(error) => {
                    tracing::warn!(%error, "skipping unreadable snapshot entry");
                    continue;
                }
            };
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || !entry.path().is_dir() {
                continue;
            }
            match self.load_snapshot(&name) {
                Ok(snapshot) => out.push(snapshot.summary()),
                Err(error) => tracing::warn!(snapshot = name, %error, "skipping invalid snapshot"),
            }
        }
        out.sort_by_key(|snapshot| std::cmp::Reverse(snapshot.created_at));
        Ok(out)
    }

    pub fn load_snapshot(&self, name: &str) -> Result<Snapshot> {
        let dir = self.snapshot_dir(name)?;
        if !dir.is_dir() {
            return Err(CoreError::SnapshotNotFound {
                vm: self.config.name.clone(),
                snapshot: name.to_string(),
            });
        }
        let snapshot: Snapshot = util::read_json(&dir.join(SNAPSHOT_FILE))?;
        if snapshot.schema_version != crate::config::SCHEMA_VERSION {
            return Err(CoreError::invalid_config(format!(
                "snapshot {name:?} has unsupported schema version {}",
                snapshot.schema_version
            )));
        }
        Ok(snapshot)
    }

    /// Capture a cold snapshot of disks, firmware files and configuration.
    pub fn create_snapshot(&self, name: &str, description: Option<String>) -> Result<Snapshot> {
        validate_snapshot_name(name)?;
        self.ensure_stopped("snapshot")?;
        let _lock = self.lock()?;
        let final_dir = self.snapshot_dir(name)?;
        if final_dir.exists() {
            return Err(CoreError::SnapshotExists(name.to_string()));
        }
        let parent = self.snapshots_dir();
        util::create_private_dir_all(&parent)?;
        let tmp_dir = parent.join(format!(".{name}.create.{}", Uuid::new_v4().simple()));
        util::create_private_dir_all(&tmp_dir)?;

        let result = (|| -> Result<Snapshot> {
            let mut disks = Vec::new();
            for disk_config in &self.config.disks {
                let source = disk_config.resolve(&self.dir);
                if !source.is_file() {
                    return Err(CoreError::Other(format!(
                        "cannot snapshot missing disk {}",
                        source.display()
                    )));
                }
                let rel = PathBuf::from("disks").join(format!("{}.img", disk_config.id));
                let target = tmp_dir.join(&rel);
                disk::clone_file(&source, &target)?;
                let size = std::fs::metadata(&target)
                    .map_err(|e| CoreError::io(&target, e))?
                    .len();
                disks.push(SnapshotDisk {
                    id: disk_config.id.clone(),
                    path: disk_config.path.clone(),
                    snapshot_path: rel,
                    size_bytes: size,
                });
            }

            let mut files = Vec::new();
            for (index, source) in self.snapshot_aux_paths().into_iter().enumerate() {
                if !source.is_file() {
                    tracing::debug!(path = %source.display(), "skipping absent snapshot file");
                    continue;
                }
                let file_name = source
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| format!("file-{index}"));
                let safe_name: String = file_name
                    .chars()
                    .map(|c| {
                        if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();
                let rel = PathBuf::from("files").join(format!("{index}-{safe_name}"));
                let target = tmp_dir.join(&rel);
                disk::clone_file(&source, &target)?;
                let size = std::fs::metadata(&target)
                    .map_err(|e| CoreError::io(&target, e))?
                    .len();
                files.push(SnapshotFile {
                    path: source,
                    snapshot_path: rel,
                    size_bytes: size,
                });
            }

            let snapshot = Snapshot::new(
                name.to_string(),
                self.config.clone(),
                disks,
                files,
                description,
            );
            util::write_json(&tmp_dir.join(SNAPSHOT_FILE), &snapshot)?;
            Ok(snapshot)
        })();

        let snapshot = match result {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let _ = std::fs::remove_dir_all(&tmp_dir);
                return Err(error);
            }
        };

        if let Err(error) = std::fs::rename(&tmp_dir, &final_dir) {
            let _ = std::fs::remove_dir_all(&tmp_dir);
            if final_dir.exists() {
                return Err(CoreError::SnapshotExists(name.to_string()));
            }
            return Err(CoreError::io(&final_dir, error));
        }
        util::fsync_dir(&parent)?;
        Ok(snapshot)
    }

    /// Restore a snapshot. The VM must be stopped.
    pub fn restore_snapshot(&mut self, name: &str) -> Result<Snapshot> {
        self.ensure_stopped("restore a snapshot")?;
        let _lock = self.lock()?;
        let snapshot = self.load_snapshot(name)?;
        if snapshot.vm_id != self.config.id.to_string() {
            return Err(CoreError::invalid_config(format!(
                "snapshot {name:?} belongs to VM {}",
                snapshot.vm_id
            )));
        }
        let mut restored = snapshot.config.clone();
        if restored.id != self.config.id || restored.name != self.config.name {
            return Err(CoreError::invalid_config(format!(
                "snapshot {name:?} was taken from a different VM"
            )));
        }
        restored.updated_at = Timestamp::now();
        restored.validate()?;

        let snapshot_root = self.snapshot_dir(name)?;

        for disk in &snapshot.disks {
            let source = safe_join(&snapshot_root, &disk.snapshot_path)?;
            if !source.is_file() {
                return Err(CoreError::Other(format!(
                    "snapshot disk is missing: {}",
                    source.display()
                )));
            }
            let target = resolve_vm_path(&self.dir, &disk.path);
            disk::replace_with_clone(&source, &target)?;
        }

        for file in &snapshot.files {
            let source = safe_join(&snapshot_root, &file.snapshot_path)?;
            if !source.is_file() {
                return Err(CoreError::Other(format!(
                    "snapshot file is missing: {}",
                    source.display()
                )));
            }
            let target = resolve_vm_path(&self.dir, &file.path);
            disk::replace_with_clone(&source, &target)?;
        }

        util::write_json(&self.config_path(), &restored)?;
        self.config = restored;
        Ok(snapshot)
    }

    pub fn delete_snapshot(&self, name: &str) -> Result<Snapshot> {
        let _lock = self.lock()?;
        let snapshot = self.load_snapshot(name)?;
        let dir = self.snapshot_dir(name)?;
        std::fs::remove_dir_all(&dir).map_err(|e| CoreError::io(&dir, e))?;
        util::fsync_dir(&self.snapshots_dir())?;
        Ok(snapshot)
    }

    fn snapshot_aux_paths(&self) -> Vec<PathBuf> {
        let mut paths = BTreeSet::new();
        if let Some(path) = &self.config.platform.machine_identifier {
            paths.insert(resolve_vm_path(&self.dir, path));
        }
        match &self.config.boot {
            BootConfig::Efi { variable_store } => {
                paths.insert(resolve_vm_path(&self.dir, variable_store));
            }
            BootConfig::MacOs {
                hardware_model,
                machine_identifier,
                auxiliary_storage,
                ..
            } => {
                paths.insert(resolve_vm_path(&self.dir, hardware_model));
                paths.insert(resolve_vm_path(&self.dir, machine_identifier));
                paths.insert(resolve_vm_path(&self.dir, auxiliary_storage));
            }
            BootConfig::Linux { .. } => {}
        }
        paths.into_iter().collect()
    }
}

/// Join a snapshot-relative path, rejecting absolute paths and traversal.
fn safe_join(base: &Path, relative: &Path) -> Result<PathBuf> {
    if relative.is_absolute() {
        return Err(CoreError::invalid_config(format!(
            "snapshot path must be relative: {}",
            relative.display()
        )));
    }
    if relative.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    }) {
        return Err(CoreError::invalid_config(format!(
            "snapshot path escapes its directory: {}",
            relative.display()
        )));
    }
    Ok(base.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{NetworkMode, ShareConfig};

    fn store() -> (tempfile::TempDir, VmStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = VmStore::new(dir.path().join("home"));
        store.init().unwrap();
        (dir, store)
    }

    fn linux_boot() -> BootConfig {
        BootConfig::Linux {
            kernel: PathBuf::from("/tmp/vmlinuz"),
            initrd: None,
            command_line: "console=hvc0".into(),
        }
    }

    fn create(store: &VmStore, name: &str) -> Vm {
        store
            .create(
                CreateVmOptions::new(name, linux_boot())
                    .with_cpus(2)
                    .with_memory_mib(1024)
                    .with_disk(NewDisk::new("root", 8 * 1024 * 1024))
                    .with_network(NetworkMode::Nat)
                    .with_share(ShareConfig::new("host", "/tmp", true)),
            )
            .unwrap()
    }

    #[test]
    fn create_list_open_delete() {
        let (_dir, store) = store();
        let vm = create(&store, "dev");
        assert!(vm.dir.join("vm.json").is_file());
        assert!(vm.dir.join("disks/root.img").is_file());
        assert_eq!(vm.config.networks[0].id, "net0");
        assert_eq!(vm.config.disks[0].size_bytes, 8 * 1024 * 1024);

        let list = store.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "dev");
        assert_eq!(list[0].state, RunState::Stopped);

        assert!(matches!(
            store.create(CreateVmOptions::new("dev", linux_boot())),
            Err(CoreError::VmExists(_))
        ));

        store.delete("dev").unwrap();
        assert!(matches!(store.open("dev"), Err(CoreError::VmNotFound(_))));
    }

    #[test]
    fn snapshot_round_trip_restores_disk_and_config() {
        let (_dir, store) = store();
        let mut vm = create(&store, "dev");
        let disk_path = vm.config.disks[0].resolve(&vm.dir);
        std::fs::write(&disk_path, b"before").unwrap();
        let snapshot = vm.create_snapshot("clean", Some("first".into())).unwrap();
        assert_eq!(snapshot.disks.len(), 1);
        assert!(vm
            .snapshot_dir("clean")
            .unwrap()
            .join("snapshot.json")
            .is_file());
        assert_eq!(vm.list_snapshots().unwrap().len(), 1);

        vm.resize_disk("root", 16 * 1024 * 1024).unwrap();
        std::fs::write(&disk_path, b"after").unwrap();
        vm.restore_snapshot("clean").unwrap();
        assert_eq!(std::fs::read(&disk_path).unwrap(), b"before");
        assert_eq!(vm.config.disks[0].size_bytes, 8 * 1024 * 1024);

        vm.delete_snapshot("clean").unwrap();
        assert!(matches!(
            vm.load_snapshot("clean"),
            Err(CoreError::SnapshotNotFound { .. })
        ));
    }

    #[test]
    fn cannot_snapshot_running_vm() {
        let (_dir, store) = store();
        let vm = create(&store, "dev");
        vm.write_runtime(&RuntimeState::running(std::process::id()))
            .unwrap();
        let error = vm.create_snapshot("nope", None).unwrap_err();
        assert!(matches!(error, CoreError::VmBusy { .. }));
    }

    #[test]
    fn stale_runtime_is_treated_as_stopped() {
        let (_dir, store) = store();
        let vm = create(&store, "dev");
        let dead_pid = 999_999_u32;
        vm.write_runtime(&RuntimeState::running(dead_pid)).unwrap();
        assert_eq!(vm.state(), RunState::Stopped);
        assert!(vm.effective_runtime().last_error.is_some());
    }

    #[test]
    fn clone_gets_new_identity_and_copied_disks() {
        let (_dir, store) = store();
        let source = create(&store, "source");
        let disk_path = source.config.disks[0].resolve(&source.dir);
        std::fs::write(&disk_path, b"data").unwrap();
        let clone = store.clone_vm("source", "clone").unwrap();
        assert_ne!(clone.config.id, source.config.id);
        assert_ne!(clone.config.networks[0].mac, source.config.networks[0].mac);
        let cloned_disk = clone.config.disks[0].resolve(&clone.dir);
        assert_eq!(std::fs::read(cloned_disk).unwrap()[..4], *b"data");
    }

    #[test]
    fn disk_add_resize_remove() {
        let (_dir, store) = store();
        let mut vm = create(&store, "dev");
        let added = vm.add_disk(NewDisk::new("data", 4 * 1024 * 1024)).unwrap();
        assert_eq!(added.id, "data");
        assert_eq!(vm.config.disks.len(), 2);
        vm.resize_disk("data", 8 * 1024 * 1024).unwrap();
        assert_eq!(
            std::fs::metadata(vm.dir.join("disks/data.img"))
                .unwrap()
                .len(),
            8 * 1024 * 1024
        );
        vm.remove_disk("data", true).unwrap();
        assert_eq!(vm.config.disks.len(), 1);
        assert!(!vm.dir.join("disks/data.img").exists());
    }

    #[test]
    fn rejects_path_traversal_in_create() {
        let (_dir, store) = store();
        let options = CreateVmOptions::new("dev", linux_boot())
            .with_disk(NewDisk::new("root", 1024).with_path(PathBuf::from("../escape.img")));
        assert!(store.create(options).is_err());
    }
}
