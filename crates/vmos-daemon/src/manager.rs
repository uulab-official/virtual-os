use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use vmos_client::{
    BridgedInterfaceInfo, Capabilities, DiskAddParams, DiskCloneParams, DiskRemoveParams,
    DiskResizeParams, EventPollResult, LogsResult, ShutdownResult, SnapshotCreateParams,
    SnapshotListResult, SnapshotResult, SystemInfo, VmCloneParams, VmCreateParams, VmGetResult,
    VmListResult, VmModifyParams, VmStopParams, PROTOCOL_VERSION,
};
use vmos_core::{
    CreateVmOptions, DiskInfo, EventKind, NewDisk, NewNetwork, RunState, SnapshotSummary,
    Timestamp, Vm, VmStore, VmSummary,
};
use vmos_vz::{
    VmState as VzState, VzBoot, VzDisk, VzGraphics, VzMachine, VzNetwork, VzNetworkMode, VzSerial,
    VzShare, VzSpec,
};

use crate::error::DaemonError;
use crate::events::EventBus;
use crate::VERSION;

pub struct Daemon {
    pub store: Arc<VmStore>,
    pub started: Instant,
    pub events: EventBus,
    pub shutdown: AtomicBool,
    ops: Mutex<()>,
    machines: Mutex<HashMap<String, Arc<VzMachine>>>,
}

impl Daemon {
    pub fn new(store: VmStore) -> Arc<Self> {
        Arc::new(Self {
            store: Arc::new(store),
            started: Instant::now(),
            events: EventBus::default(),
            shutdown: AtomicBool::new(false),
            ops: Mutex::new(()),
            machines: Mutex::new(HashMap::new()),
        })
    }

    fn lock_ops(&self) -> std::sync::MutexGuard<'_, ()> {
        self.ops.lock().unwrap_or_else(|poison| poison.into_inner())
    }

    fn machines(&self) -> std::sync::MutexGuard<'_, HashMap<String, Arc<VzMachine>>> {
        self.machines
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn machine(&self, name: &str) -> Option<Arc<VzMachine>> {
        self.machines().get(name).cloned()
    }

    pub fn shutdown_requested(&self) -> bool {
        self.shutdown.load(Ordering::Relaxed)
    }

    pub fn request_shutdown(&self) -> ShutdownResult {
        self.shutdown.store(true, Ordering::Relaxed);
        ShutdownResult { ok: true }
    }

    pub fn list(&self) -> Result<VmListResult, DaemonError> {
        Ok(VmListResult {
            vms: self.store.list()?,
        })
    }

    pub fn get(&self, name: &str) -> Result<VmGetResult, DaemonError> {
        let vm = self.store.open(name)?;
        Ok(VmGetResult { vm: vm.info()? })
    }

    pub fn summary(&self, name: &str) -> Result<VmSummary, DaemonError> {
        Ok(self.store.open(name)?.summary())
    }

    pub fn system_info(&self) -> Result<SystemInfo, DaemonError> {
        Ok(SystemInfo {
            version: VERSION.to_string(),
            protocol_version: PROTOCOL_VERSION,
            pid: std::process::id(),
            uptime_secs: self.started.elapsed().as_secs(),
            store_root: self.store.root().to_path_buf(),
            vm_count: self.store.list()?.len(),
            vz_supported: VzMachine::is_supported(),
            features: features(),
        })
    }

    pub fn capabilities(&self) -> Capabilities {
        let supported = VzMachine::is_supported();
        let limits = VzMachine::host_limits();
        Capabilities {
            vz_supported: supported,
            min_cpus: limits.min_cpus,
            max_cpus: limits.max_cpus,
            min_memory_bytes: limits.min_memory_bytes,
            max_memory_bytes: limits.max_memory_bytes,
            bridged_interfaces: vmos_vz::bridged_interfaces()
                .into_iter()
                .map(|interface| BridgedInterfaceInfo {
                    identifier: interface.identifier,
                    display_name: interface.display_name,
                })
                .collect(),
            default_store_root: VmStore::default_root(),
            daemon_socket: self.store.socket_path(),
            os: std::env::consts::OS.to_string(),
            arch: std::env::consts::ARCH.to_string(),
        }
    }

    // ------------------------------------------------------------------
    // VM lifecycle
    // ------------------------------------------------------------------

    pub fn create(&self, params: VmCreateParams) -> Result<VmGetResult, DaemonError> {
        let _ops = self.lock_ops();
        let mut boot = params.boot;
        if let vmos_core::BootConfig::Efi { variable_store } = &mut boot {
            if variable_store.as_os_str().is_empty() {
                *variable_store = PathBuf::from("nvram/efi-vars.fd");
            }
        }

        let mut options = CreateVmOptions::new(params.name.clone(), boot)
            .with_cpus(params.cpus)
            .with_memory_mib(params.memory_mib)
            .with_serial(params.serial);
        if let Some(display_name) = params.display_name {
            options = options.with_display_name(display_name);
        }
        options.networks.clear();
        for network in &params.networks {
            options.networks.push(NewNetwork {
                mode: network.mode.clone(),
                mac: network.mac.clone(),
            });
        }
        for disk in &params.disks {
            let mut new_disk = NewDisk::new(disk.id.clone(), disk.size_bytes);
            if let Some(path) = &disk.path {
                new_disk = new_disk.with_path(path.clone());
            }
            options.disks.push(new_disk);
        }
        options.shares = params.shares.clone();
        options.tags = params.tags.clone();

        let vm = self.store.create(options)?;
        self.events
            .publish(Some(&vm.config.name), EventKind::VmCreated);
        tracing::info!(vm = %vm.config.name, "created VM");
        Ok(VmGetResult { vm: vm.info()? })
    }

    pub fn start(&self, name: &str) -> Result<VmGetResult, DaemonError> {
        let _ops = self.lock_ops();
        let vm = self.store.open(name)?;

        if let Some(machine) = self.machine(name) {
            match machine.state() {
                VzState::Running | VzState::Paused => {
                    return self.get(name);
                }
                _ => {
                    self.machines().remove(name);
                }
            }
        }

        // Reconcile a runtime record left behind by a previous daemon or a
        // failed lifecycle operation before claiming the VM again.
        let runtime = vm.effective_runtime();
        if runtime.state.is_active() {
            tracing::warn!(vm = name, state = %runtime.state, "clearing stale active runtime state");
            self.write_runtime(&vm, RunState::Stopped, None)?;
        }

        self.write_runtime(&vm, RunState::Starting, None)?;
        let spec = spec_for(&vm)?;
        let machine = match VzMachine::new(spec, format!("com.uulab.vmos.vm.{name}")) {
            Ok(machine) => Arc::new(machine),
            Err(error) => {
                let message = error.to_string();
                self.write_runtime(&vm, RunState::Error, Some(message.clone()))?;
                self.events.publish(
                    Some(name),
                    EventKind::VmError {
                        message: message.clone(),
                    },
                );
                return Err(error.into());
            }
        };

        if let Err(error) = machine.start() {
            let message = error.to_string();
            self.write_runtime(&vm, RunState::Error, Some(message.clone()))?;
            self.events.publish(
                Some(name),
                EventKind::VmError {
                    message: message.clone(),
                },
            );
            return Err(error.into());
        }

        self.machines().insert(name.to_string(), machine);
        self.write_runtime(&vm, RunState::Running, None)?;
        self.events.publish(Some(name), EventKind::VmStarted);
        tracing::info!(vm = name, "started VM");
        self.get(name)
    }

    pub fn stop(&self, params: VmStopParams) -> Result<VmGetResult, DaemonError> {
        let _ops = self.lock_ops();
        let vm = self.store.open(&params.name)?;
        let machine = self.machines().remove(&params.name);
        let forced = params.force || params.timeout_secs == 0;

        if let Some(machine) = machine {
            let state = machine.state();
            if !matches!(state, VzState::Stopped | VzState::Error) {
                let result = if forced {
                    machine.stop()
                } else {
                    machine.shutdown(Duration::from_secs(params.timeout_secs.max(1)))
                };
                if let Err(error) = result {
                    if machine.state().is_active() {
                        self.machines().insert(params.name.clone(), machine);
                    }
                    let message = error.to_string();
                    self.write_runtime(&vm, RunState::Error, Some(message))?;
                    return Err(error.into());
                }
            }
        }

        self.write_runtime(&vm, RunState::Stopped, None)?;
        self.events
            .publish(Some(&params.name), EventKind::VmStopped { forced });
        tracing::info!(vm = %params.name, forced, "stopped VM");
        self.get(&params.name)
    }

    pub fn pause(&self, name: &str) -> Result<VmGetResult, DaemonError> {
        let _ops = self.lock_ops();
        let vm = self.store.open(name)?;
        let machine = self
            .machine(name)
            .ok_or_else(|| DaemonError::NotRunning(name.to_string()))?;
        if machine.state() != VzState::Paused {
            machine.pause()?;
        }
        self.write_runtime(&vm, RunState::Paused, None)?;
        self.events.publish(Some(name), EventKind::VmPaused);
        self.get(name)
    }

    pub fn resume(&self, name: &str) -> Result<VmGetResult, DaemonError> {
        let _ops = self.lock_ops();
        let vm = self.store.open(name)?;
        let machine = self
            .machine(name)
            .ok_or_else(|| DaemonError::NotRunning(name.to_string()))?;
        if machine.state() != VzState::Running {
            machine.resume()?;
        }
        self.write_runtime(&vm, RunState::Running, None)?;
        self.events.publish(Some(name), EventKind::VmResumed);
        self.get(name)
    }

    pub fn delete(&self, name: &str, force: bool) -> Result<(), DaemonError> {
        let _ops = self.lock_ops();
        let vm = self.store.open(name)?;
        if let Some(machine) = self.machine(name) {
            if !force {
                return Err(DaemonError::Core(vmos_core::CoreError::VmBusy {
                    name: name.to_string(),
                    state: machine.state().label().to_string(),
                    action: "delete",
                }));
            }
            let _ = machine.stop();
            self.machines().remove(name);
        }
        self.store.delete(name)?;
        let _ = vm;
        self.events.publish(Some(name), EventKind::VmDeleted);
        tracing::info!(vm = name, "deleted VM");
        Ok(())
    }

    pub fn clone_vm(&self, params: VmCloneParams) -> Result<VmGetResult, DaemonError> {
        let _ops = self.lock_ops();
        let vm = self.store.clone_vm(&params.source, &params.target)?;
        self.events.publish(
            Some(&params.target),
            EventKind::VmCloned {
                source: params.source,
            },
        );
        Ok(VmGetResult { vm: vm.info()? })
    }

    pub fn modify(&self, params: VmModifyParams) -> Result<VmGetResult, DaemonError> {
        let _ops = self.lock_ops();
        let mut vm = self.store.open(&params.name)?;
        let cpus = params.cpus.unwrap_or(vm.config.cpus);
        let memory = params.memory_mib.unwrap_or(vm.config.memory_mib);
        vm.set_resources(cpus, memory)?;
        Ok(VmGetResult { vm: vm.info()? })
    }

    pub fn logs(&self, name: &str, lines: usize) -> Result<LogsResult, DaemonError> {
        let vm = self.store.open(name)?;
        let path = vm.serial_log_path();
        let lines = lines.clamp(1, 10_000);
        let content = if path.is_file() {
            vmos_core::util::read_last_lines(&path, lines)?
        } else {
            Vec::new()
        };
        Ok(LogsResult {
            name: name.to_string(),
            path,
            lines: content,
        })
    }

    // ------------------------------------------------------------------
    // Snapshots
    // ------------------------------------------------------------------

    pub fn snapshot_list(&self, vm: &str) -> Result<SnapshotListResult, DaemonError> {
        let vm = self.store.open(vm)?;
        Ok(SnapshotListResult {
            snapshots: vm.list_snapshots()?,
        })
    }

    pub fn snapshot_create(
        &self,
        params: SnapshotCreateParams,
    ) -> Result<SnapshotResult, DaemonError> {
        let _ops = self.lock_ops();
        let vm = self.store.open(&params.vm)?;
        let snapshot = vm.create_snapshot(&params.name, params.description)?;
        self.events.publish(
            Some(&params.vm),
            EventKind::SnapshotCreated {
                snapshot: params.name,
            },
        );
        Ok(SnapshotResult { snapshot })
    }

    pub fn snapshot_restore(&self, vm: &str, name: &str) -> Result<SnapshotResult, DaemonError> {
        let _ops = self.lock_ops();
        let mut vm = self.store.open(vm)?;
        let snapshot = vm.restore_snapshot(name)?;
        self.events.publish(
            Some(&vm.config.name),
            EventKind::SnapshotRestored {
                snapshot: name.to_string(),
            },
        );
        Ok(SnapshotResult { snapshot })
    }

    pub fn snapshot_delete(&self, vm: &str, name: &str) -> Result<SnapshotSummary, DaemonError> {
        let _ops = self.lock_ops();
        let vm = self.store.open(vm)?;
        let snapshot = vm.delete_snapshot(name)?;
        self.events.publish(
            Some(&vm.config.name),
            EventKind::SnapshotDeleted {
                snapshot: name.to_string(),
            },
        );
        Ok(snapshot.summary())
    }

    // ------------------------------------------------------------------
    // Disks
    // ------------------------------------------------------------------

    pub fn disk_list(&self, vm: &str) -> Result<Vec<DiskInfo>, DaemonError> {
        Ok(self.store.open(vm)?.disk_infos())
    }

    pub fn disk_add(&self, params: DiskAddParams) -> Result<DiskInfo, DaemonError> {
        let _ops = self.lock_ops();
        let mut vm = self.store.open(&params.vm)?;
        let mut new_disk = NewDisk::new(params.id.clone(), params.size_bytes);
        if let Some(path) = params.path {
            new_disk = new_disk.with_path(path);
        }
        let disk = vm.add_disk(new_disk)?;
        self.events.publish(
            Some(&params.vm),
            EventKind::DiskCreated {
                disk: params.id,
                size_bytes: disk.size_bytes,
            },
        );
        vm.disk_infos()
            .into_iter()
            .find(|info| info.id == disk.id)
            .ok_or_else(|| DaemonError::message("disk was added but cannot be inspected"))
    }

    pub fn disk_resize(&self, params: DiskResizeParams) -> Result<DiskInfo, DaemonError> {
        let _ops = self.lock_ops();
        let mut vm = self.store.open(&params.vm)?;
        let disk = vm.resize_disk(&params.disk, params.size_bytes)?;
        self.events.publish(
            Some(&params.vm),
            EventKind::DiskResized {
                disk: params.disk,
                size_bytes: disk.size_bytes,
            },
        );
        vm.disk_infos()
            .into_iter()
            .find(|info| info.id == disk.id)
            .ok_or_else(|| DaemonError::message("disk was resized but cannot be inspected"))
    }

    pub fn disk_remove(&self, params: DiskRemoveParams) -> Result<DiskInfo, DaemonError> {
        let _ops = self.lock_ops();
        let mut vm = self.store.open(&params.vm)?;
        let disk = vm.remove_disk(&params.disk, params.delete)?;
        let path = disk.resolve(&vm.dir);
        Ok(DiskInfo {
            id: disk.id,
            path,
            format: vmos_core::DiskFormat::Raw,
            size_bytes: disk.size_bytes,
            allocated_bytes: 0,
            read_only: disk.read_only,
            present: false,
        })
    }

    pub fn disk_clone(&self, params: DiskCloneParams) -> Result<DiskInfo, DaemonError> {
        let _ops = self.lock_ops();
        let vm = self.store.open(&params.vm)?;
        let info = vm.clone_disk(&params.disk, &params.target)?;
        self.events.publish(
            Some(&params.vm),
            EventKind::DiskCloned {
                disk: params.disk,
                target: params.target.display().to_string(),
            },
        );
        Ok(info)
    }

    // ------------------------------------------------------------------
    // Events and recovery
    // ------------------------------------------------------------------

    pub fn poll_events(&self, after: Option<u64>, limit: Option<usize>) -> EventPollResult {
        let (events, next) = self.events.poll(after, limit.unwrap_or(100));
        EventPollResult { events, next }
    }

    /// Stop every machine the daemon owns. Used during daemon shutdown so
    /// guests receive a graceful power button before the process exits.
    pub fn stop_all(&self) -> usize {
        let _ops = self.lock_ops();
        let machines: Vec<(String, Arc<VzMachine>)> = {
            let mut guard = self.machines();
            guard.drain().collect()
        };
        let count = machines.len();
        for (name, machine) in machines {
            if machine.state().is_active() {
                if let Err(error) = machine.shutdown(Duration::from_secs(5)) {
                    tracing::warn!(vm = %name, %error, "graceful stop failed during shutdown");
                }
                if machine.state().is_active() {
                    let _ = machine.stop();
                }
            }
            if let Ok(vm) = self.store.open(&name) {
                let _ = self.write_runtime(&vm, RunState::Stopped, None);
            }
            self.events
                .publish(Some(&name), EventKind::VmStopped { forced: true });
            tracing::info!(vm = %name, "stopped VM for daemon shutdown");
        }
        count
    }

    /// Mark runtime records left active by a previous daemon as stopped.
    pub fn recover_stale_runtimes(&self) {
        let vms = match self.store.list() {
            Ok(vms) => vms,
            Err(error) => {
                tracing::warn!(%error, "cannot scan store during recovery");
                return;
            }
        };
        for summary in vms {
            let Ok(vm) = self.store.open(&summary.name) else {
                continue;
            };
            let raw = vm.load_runtime();
            if raw.state.is_active() && raw.effective().state == RunState::Stopped {
                let message = "daemon restarted while the VM was active".to_string();
                if let Err(error) = self.write_runtime(&vm, RunState::Stopped, Some(message)) {
                    tracing::warn!(vm = %summary.name, %error, "failed to recover runtime state");
                } else {
                    self.events
                        .publish(Some(&summary.name), EventKind::VmStopped { forced: true });
                    tracing::warn!(vm = %summary.name, "recovered stale active VM state");
                }
            }
        }
    }

    /// Watch framework state so guest-initiated shutdowns reach the store.
    pub fn spawn_monitor(self: &Arc<Self>) {
        let daemon = Arc::clone(self);
        let result = std::thread::Builder::new()
            .name("vmos-monitor".into())
            .spawn(move || {
                while !daemon.shutdown_requested() {
                    daemon.reconcile();
                    for _ in 0..10 {
                        if daemon.shutdown_requested() {
                            break;
                        }
                        std::thread::sleep(Duration::from_millis(100));
                    }
                }
            });
        if let Err(error) = result {
            tracing::error!(%error, "failed to start VM monitor thread");
        }
    }

    fn reconcile(&self) {
        let machines: Vec<(String, Arc<VzMachine>)> = self
            .machines()
            .iter()
            .map(|(name, machine)| (name.clone(), Arc::clone(machine)))
            .collect();

        for (name, machine) in machines {
            let observed = machine.state();
            if matches!(observed, VzState::Running | VzState::Paused) {
                let state = if observed == VzState::Paused {
                    RunState::Paused
                } else {
                    RunState::Running
                };
                if let Ok(vm) = self.store.open(&name) {
                    if vm.effective_runtime().state != state {
                        let _ = self.write_runtime(&vm, state, None);
                    }
                }
                continue;
            }
            if !matches!(observed, VzState::Stopped | VzState::Error) {
                continue;
            }

            self.machines().remove(&name);
            let Ok(vm) = self.store.open(&name) else {
                continue;
            };
            if observed == VzState::Error {
                let message = machine
                    .last_error()
                    .unwrap_or_else(|| "virtual machine stopped with an error".to_string());
                let _ = self.write_runtime(&vm, RunState::Error, Some(message.clone()));
                self.events
                    .publish(Some(&name), EventKind::VmError { message });
                tracing::error!(vm = %name, "guest stopped with an error");
            } else {
                let _ = self.write_runtime(&vm, RunState::Stopped, None);
                self.events
                    .publish(Some(&name), EventKind::VmStopped { forced: false });
                tracing::info!(vm = %name, "guest stopped");
            }
        }
    }

    fn write_runtime(
        &self,
        vm: &Vm,
        state: RunState,
        error: Option<String>,
    ) -> Result<(), DaemonError> {
        let mut runtime = vm.load_runtime();
        let now = Timestamp::now();
        runtime.state = state;
        runtime.updated_at = Some(now);
        runtime.last_error = error;
        match state {
            RunState::Starting => {
                runtime.pid = Some(std::process::id());
                runtime.started_at = Some(now);
            }
            RunState::Running | RunState::Paused | RunState::Stopping => {
                runtime.pid = Some(std::process::id());
                runtime.started_at.get_or_insert(now);
            }
            RunState::Stopped | RunState::Error => {
                runtime.pid = None;
                runtime.started_at = None;
            }
        }
        vm.write_runtime(&runtime)?;
        Ok(())
    }
}

fn features() -> Vec<String> {
    [
        "linux-direct-boot",
        "efi-boot",
        "nat-network",
        "bridged-network",
        "virtiofs",
        "serial-console",
        "cold-snapshots",
        "vm-clone",
        "disk-management",
        "json-rpc",
    ]
    .into_iter()
    .map(str::to_string)
    .collect()
}

fn spec_for(vm: &Vm) -> Result<VzSpec, DaemonError> {
    let config = &vm.config;
    let boot = match &config.boot {
        vmos_core::BootConfig::Linux {
            kernel,
            initrd,
            command_line,
        } => VzBoot::Linux {
            kernel: vmos_core::config::resolve_vm_path(&vm.dir, kernel),
            initrd: initrd
                .as_ref()
                .map(|path| vmos_core::config::resolve_vm_path(&vm.dir, path)),
            command_line: command_line.clone(),
        },
        vmos_core::BootConfig::Efi { variable_store } => VzBoot::Efi {
            variable_store: vmos_core::config::resolve_vm_path(&vm.dir, variable_store),
        },
        vmos_core::BootConfig::MacOs {
            hardware_model,
            machine_identifier,
            auxiliary_storage,
            ..
        } => VzBoot::MacOs {
            hardware_model: vmos_core::config::resolve_vm_path(&vm.dir, hardware_model),
            machine_identifier: vmos_core::config::resolve_vm_path(&vm.dir, machine_identifier),
            auxiliary_storage: vmos_core::config::resolve_vm_path(&vm.dir, auxiliary_storage),
        },
    };

    let disks = config
        .disks
        .iter()
        .map(|disk| VzDisk {
            path: disk.resolve(&vm.dir),
            read_only: disk.read_only,
        })
        .collect();

    let networks = config
        .networks
        .iter()
        .map(|network| VzNetwork {
            mac: network.mac.clone(),
            mode: match &network.mode {
                vmos_core::NetworkMode::Nat => VzNetworkMode::Nat,
                vmos_core::NetworkMode::Bridged { interface } => VzNetworkMode::Bridged {
                    interface: interface.clone(),
                },
            },
        })
        .collect();

    let shares = config
        .shares
        .iter()
        .map(|share| VzShare {
            tag: share.tag.clone(),
            host_path: vmos_core::config::resolve_vm_path(&vm.dir, &share.host_path),
            read_only: share.read_only,
        })
        .collect();

    let serial = if config.serial.enabled {
        Some(VzSerial {
            path: config.serial.resolve(&vm.dir),
            append: config.serial.append,
        })
    } else {
        None
    };

    let platform_identifier = config
        .platform
        .machine_identifier
        .as_ref()
        .map(|path| vmos_core::config::resolve_vm_path(&vm.dir, path));

    let graphics = if matches!(config.boot, vmos_core::BootConfig::MacOs { .. }) {
        Some(VzGraphics::default())
    } else {
        None
    };

    Ok(VzSpec {
        cpus: config.cpus,
        memory_mib: config.memory_mib,
        boot,
        disks,
        networks,
        shares,
        serial,
        platform_identifier,
        graphics,
    })
}
