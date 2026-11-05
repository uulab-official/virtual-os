use std::fs::OpenOptions;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use clap::{Parser, Subcommand};
use vmos_client::{
    Client, DiskAddParams, DiskCloneParams, DiskRemoveParams, DiskResizeParams, LogsParams,
    VmCreateParams, VmDeleteParams, VmModifyParams, VmStopParams,
};
use vmos_core::{BootConfig, NetworkMode, ShareConfig, VmStore};

#[derive(Parser)]
#[command(
    name = "vmos",
    version,
    about = "Manage local virtual machines on macOS"
)]
struct Cli {
    /// Override the store root (defaults to $VOS_HOME or ~/.virtual-os).
    #[arg(long, global = true, env = "VOS_HOME")]
    store: Option<PathBuf>,

    #[command(subcommand)]
    command: Commands,
}

#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum Commands {
    /// Initialize a new store.
    Init,
    /// Create a virtual machine.
    Create {
        name: String,
        #[arg(long, default_value_t = 2)]
        cpus: u32,
        #[arg(long, default_value_t = 2048)]
        memory: u64,
        #[arg(long, value_name = "KIND", default_value = "efi")]
        boot: String,
        #[arg(long)]
        kernel: Option<PathBuf>,
        #[arg(long)]
        initrd: Option<PathBuf>,
        #[arg(long, default_value = "")]
        cmdline: String,
        #[arg(long, value_name = "ID:SIZE[:PATH]", action = clap::ArgAction::Append)]
        disk: Vec<String>,
        #[arg(long, value_name = "MODE", action = clap::ArgAction::Append, default_value = "nat")]
        network: Vec<String>,
        #[arg(long, value_name = "TAG:HOSTPATH[:ro]", action = clap::ArgAction::Append)]
        share: Vec<String>,
        #[arg(long)]
        serial: Option<PathBuf>,
        #[arg(long)]
        display_name: Option<String>,
        #[arg(long, value_name = "KEY=VALUE", action = clap::ArgAction::Append)]
        tag: Vec<String>,
        // macOS guest boot helpers
        #[arg(long)]
        hardware_model: Option<PathBuf>,
        #[arg(long)]
        machine_identifier: Option<PathBuf>,
        #[arg(long)]
        auxiliary_storage: Option<PathBuf>,
        #[arg(long)]
        restore_image: Option<PathBuf>,
    },
    /// List virtual machines.
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show one virtual machine.
    Show {
        name: String,
        #[arg(long)]
        json: bool,
    },
    /// Start a virtual machine.
    Start {
        name: String,
    },
    /// Stop a virtual machine.
    Stop {
        name: String,
        #[arg(long)]
        force: bool,
        #[arg(long, default_value_t = 30)]
        timeout: u64,
    },
    /// Pause a virtual machine.
    Pause {
        name: String,
    },
    /// Resume a virtual machine.
    Resume {
        name: String,
    },
    /// Delete a virtual machine.
    Delete {
        name: String,
        #[arg(long)]
        force: bool,
    },
    /// Clone a virtual machine.
    Clone {
        source: String,
        target: String,
    },
    /// Modify CPU/memory of a stopped VM.
    Modify {
        name: String,
        #[arg(long)]
        cpus: Option<u32>,
        #[arg(long)]
        memory: Option<u64>,
    },
    /// Print serial console output.
    Logs {
        name: String,
        #[arg(long, default_value_t = 200)]
        lines: usize,
    },
    Install {
        name: String,
        restore_image: PathBuf,
    },
    Save {
        name: String,
        path: PathBuf,
    },
    Restore {
        name: String,
        path: PathBuf,
    },
    Backup {
        name: String,
        path: PathBuf,
    },
    RestoreBackup {
        path: PathBuf,
        #[arg(long)]
        name: Option<String>,
    },
    /// Snapshot management.
    Snapshot {
        #[command(subcommand)]
        command: SnapshotCommands,
    },
    /// Disk management.
    Disk {
        #[command(subcommand)]
        command: DiskCommands,
    },
    /// Daemon management.
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },
    /// Show diagnostics and host capabilities.
    Doctor {
        #[arg(long)]
        json: bool,
    },
    /// Poll daemon events.
    Events {
        #[arg(long)]
        after: Option<u64>,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Print the CLI version.
    Version,
}

#[derive(Subcommand)]
enum SnapshotCommands {
    List {
        vm: String,
    },
    Create {
        vm: String,
        name: String,
        #[arg(long)]
        description: Option<String>,
    },
    Restore {
        vm: String,
        name: String,
    },
    Delete {
        vm: String,
        name: String,
    },
}

#[derive(Subcommand)]
enum DiskCommands {
    List {
        vm: String,
    },
    Add {
        vm: String,
        id: String,
        size: String,
        #[arg(long)]
        path: Option<PathBuf>,
    },
    Resize {
        vm: String,
        disk: String,
        size: String,
    },
    Remove {
        vm: String,
        disk: String,
        #[arg(long)]
        delete: bool,
    },
    Clone {
        vm: String,
        disk: String,
        target: PathBuf,
    },
    Compact {
        vm: String,
        disk: String,
    },
}

#[derive(Subcommand)]
enum DaemonCommands {
    /// Start the daemon in the background.
    Start {
        #[arg(long)]
        foreground: bool,
    },
    /// Stop the running daemon.
    Stop,
    /// Show daemon status.
    Status,
    /// Run the daemon in the foreground (used by `daemon start`).
    #[command(hide = true)]
    Run {
        #[arg(long)]
        foreground: bool,
    },
}

fn store_root(cli: &Cli) -> PathBuf {
    cli.store.clone().unwrap_or_else(VmStore::default_root)
}

fn store(cli: &Cli) -> VmStore {
    VmStore::new(store_root(cli))
}

fn daemon_socket(cli: &Cli) -> PathBuf {
    store_root(cli).join("daemon.sock")
}

fn ensure_daemon(cli: &Cli) -> Result<Client, Box<dyn std::error::Error>> {
    let socket = daemon_socket(cli);
    if let Some(client) = Client::connect_if_running(&socket) {
        return Ok(client);
    }
    spawn_daemon(cli)?;
    for _ in 0..100 {
        std::thread::sleep(Duration::from_millis(100));
        if let Some(client) = Client::connect_if_running(&socket) {
            return Ok(client);
        }
    }
    Err("daemon did not become ready within 10 seconds".into())
}

fn spawn_daemon(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let exe = std::env::current_exe()?;
    let root = store_root(cli);
    let log_path = root.join("daemon.log");
    let _ = std::fs::create_dir_all(&root);
    let log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    let log_clone = log.try_clone()?;
    let mut cmd = Command::new(exe);
    cmd.arg("daemon").arg("run");
    if let Some(store) = &cli.store {
        cmd.arg("--store").arg(store);
    }
    cmd.stdin(Stdio::null());
    cmd.stdout(log);
    cmd.stderr(log_clone);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }
    cmd.spawn()?;
    Ok(())
}

fn stop_daemon_process(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let _ = client.shutdown();
    for _ in 0..50 {
        std::thread::sleep(Duration::from_millis(100));
        if Client::connect_if_running(daemon_socket(cli)).is_none() {
            return Ok(());
        }
    }
    Err("daemon did not stop within 5 seconds".into())
}

fn print_json(value: &serde_json::Value) {
    println!(
        "{}",
        serde_json::to_string_pretty(value).unwrap_or_default()
    );
}

fn cmd_init(cli: &Cli) -> Result<(), Box<dyn std::error::Error>> {
    let store = store(cli);
    store.init()?;
    println!("initialized store at {}", store.root().display());
    Ok(())
}

fn parse_size(value: &str) -> Result<u64, Box<dyn std::error::Error>> {
    vmos_core::util::parse_size(value).map_err(|reason| reason.into())
}

fn cmd_create(cli: &Cli, args: &Commands) -> Result<(), Box<dyn std::error::Error>> {
    // Extract fields manually by matching.
    let Commands::Create {
        name,
        cpus,
        memory,
        boot,
        kernel,
        initrd,
        cmdline,
        disk,
        network,
        share,
        serial,
        display_name,
        tag,
        hardware_model,
        machine_identifier,
        auxiliary_storage,
        restore_image,
    } = args
    else {
        return Err("internal error".into());
    };

    let boot_config = match boot.as_str() {
        "linux" => {
            let kernel = kernel
                .clone()
                .ok_or("--kernel is required for Linux boot")?;
            BootConfig::Linux {
                kernel,
                initrd: initrd.clone(),
                command_line: cmdline.clone(),
            }
        }
        "efi" => {
            let variable_store = PathBuf::from("nvram/efi-vars.fd");
            BootConfig::Efi { variable_store }
        }
        "macos" => {
            let hardware_model = hardware_model
                .clone()
                .ok_or("--hardware-model is required for macOS boot")?;
            let machine_identifier = machine_identifier
                .clone()
                .ok_or("--machine-identifier is required for macOS boot")?;
            let auxiliary_storage = auxiliary_storage
                .clone()
                .ok_or("--auxiliary-storage is required for macOS boot")?;
            BootConfig::MacOs {
                restore_image: restore_image.clone(),
                hardware_model,
                machine_identifier,
                auxiliary_storage,
            }
        }
        other => return Err(format!("unknown boot kind {other:?}").into()),
    };

    let mut params = VmCreateParams::new(name.clone(), boot_config, *cpus, *memory);
    if let Some(display_name) = display_name {
        params.display_name = Some(display_name.clone());
    }

    for spec in disk {
        let parts: Vec<&str> = spec.splitn(3, ':').collect();
        if parts.len() < 2 {
            return Err(format!("invalid --disk {spec:?}; expected ID:SIZE[:PATH]").into());
        }
        let id = parts[0].to_string();
        let size = parse_size(parts[1])?;
        let mut disk_spec = vmos_client::DiskSpec {
            id,
            size_bytes: size,
            path: None,
        };
        if parts.len() == 3 {
            disk_spec.path = Some(PathBuf::from(parts[2]));
        }
        params.disks.push(disk_spec);
    }

    params.networks.clear();
    for spec in network {
        let (mode, mac) = if let Some(rest) = spec.strip_prefix("bridged") {
            let iface = rest.trim_start_matches(':');
            let iface = if iface.is_empty() {
                None
            } else {
                Some(iface.to_string())
            };
            (NetworkMode::Bridged { interface: iface }, None)
        } else if spec == "vmnet" {
            (NetworkMode::Vmnet, None)
        } else {
            (NetworkMode::Nat, None)
        };
        let net = vmos_client::NetworkSpec { mac, mode };
        // allow mac via embedded? no
        params.networks.push(net);
    }
    if params.networks.is_empty() {
        params.networks.push(vmos_client::NetworkSpec::nat());
    }

    for spec in share {
        let parts: Vec<&str> = spec.splitn(3, ':').collect();
        if parts.len() < 2 {
            return Err(format!("invalid --share {spec:?}; expected TAG:HOSTPATH[:ro]").into());
        }
        let tag = parts[0].to_string();
        let host_path = PathBuf::from(parts[1]);
        let read_only = parts.get(2).map(|value| *value == "ro").unwrap_or(false);
        params.shares.push(ShareConfig {
            tag,
            host_path,
            read_only,
        });
    }

    if let Some(serial) = serial {
        params.serial.enabled = true;
        params.serial.log = serial.clone();
        params.serial.append = true;
    }

    for entry in tag {
        let (key, value) = entry
            .split_once('=')
            .ok_or_else(|| format!("invalid --tag {entry:?}; expected KEY=VALUE"))?;
        params.tags.insert(key.to_string(), value.to_string());
    }

    let mut client = ensure_daemon(cli)?;
    let result: vmos_client::VmGetResult = client.call("vm.create", &params)?;
    println!(
        "created {} ({})",
        result.vm.name,
        vmos_core::util::format_size(result.vm.config.memory_mib * 1024 * 1024)
    );
    println!("  state: {}", result.vm.state);
    println!("  dir:   {}", result.vm.dir.display());
    Ok(())
}

fn main() {
    let cli = Cli::parse();
    let result = run(cli);
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    match &cli.command {
        Commands::Init => cmd_init(&cli),
        Commands::Create { .. } => cmd_create(&cli, &cli.command),
        Commands::List { json } => cmd_list(&cli, *json),
        Commands::Show { name, json } => cmd_show(&cli, name, *json),
        Commands::Start { name } => cmd_start(&cli, name),
        Commands::Stop {
            name,
            force,
            timeout,
        } => cmd_stop(&cli, name, *force, *timeout),
        Commands::Pause { name } => cmd_pause(&cli, name),
        Commands::Resume { name } => cmd_resume(&cli, name),
        Commands::Delete { name, force } => cmd_delete(&cli, name, *force),
        Commands::Clone { source, target } => cmd_clone(&cli, source, target),
        Commands::Modify { name, cpus, memory } => cmd_modify(&cli, name, *cpus, *memory),
        Commands::Logs { name, lines } => cmd_logs(&cli, name, *lines),
        Commands::Save { name, path } => cmd_save(&cli, name, path),
        Commands::Restore { name, path } => cmd_restore(&cli, name, path),
        Commands::Backup { name, path } => cmd_backup(&cli, name, path),
        Commands::RestoreBackup { path, name } => cmd_restore_backup(&cli, path, name.as_deref()),
        Commands::Install {
            name,
            restore_image,
        } => cmd_install(&cli, name, restore_image),
        Commands::Snapshot { .. } => cmd_snapshot(&cli, &cli.command),
        Commands::Disk { .. } => cmd_disk(&cli, &cli.command),
        Commands::Daemon { .. } => cmd_daemon(&cli, &cli.command),
        Commands::Doctor { json } => cmd_doctor(&cli, *json),
        Commands::Events { after, limit } => cmd_events(&cli, *after, *limit),
        Commands::Version => {
            println!("vmos {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
    }
}

fn cmd_list(cli: &Cli, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let result: vmos_client::VmListResult = client.call("vm.list", &vmos_client::Empty {})?;
    if json {
        print_json(&serde_json::to_value(&result)?);
        return Ok(());
    }
    if result.vms.is_empty() {
        println!("no virtual machines");
        return Ok(());
    }
    println!(
        "{:<20} {:<10} {:<8} {:<10} BOOT",
        "NAME", "STATE", "CPUS", "MEMORY"
    );
    for vm in result.vms {
        println!(
            "{:<20} {:<10} {:<8} {:<10} {}",
            vm.name,
            vm.state,
            vm.cpus,
            vmos_core::util::format_size(vm.memory_mib * 1024 * 1024),
            vm.boot_kind
        );
    }
    Ok(())
}

fn cmd_show(cli: &Cli, name: &str, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let result: vmos_client::VmGetResult = client.call(
        "vm.get",
        &vmos_client::VmNameParams {
            name: name.to_string(),
        },
    )?;
    if json {
        print_json(&serde_json::to_value(&result)?);
        return Ok(());
    }
    let vm = result.vm;
    println!("name:        {}", vm.name);
    println!("id:          {}", vm.id);
    println!("state:       {}", vm.state);
    println!("cpus:        {}", vm.config.cpus);
    println!(
        "memory:      {}",
        vmos_core::util::format_size(vm.config.memory_mib * 1024 * 1024)
    );
    println!("boot:        {:?}", vm.config.boot);
    println!("dir:         {}", vm.dir.display());
    println!("serial log:  {}", vm.serial_log.display());
    println!("disks:");
    for disk in vm.disks {
        println!(
            "  {}  {}  {}",
            disk.id,
            vmos_core::util::format_size(disk.size_bytes),
            disk.path.display()
        );
    }
    println!("snapshots:");
    for snapshot in vm.snapshots {
        println!("  {}  {}", snapshot.name, snapshot.created_at);
    }
    Ok(())
}

fn cmd_start(cli: &Cli, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let result: vmos_client::VmGetResult = client.call_long(
        "vm.start",
        &vmos_client::VmNameParams {
            name: name.to_string(),
        },
    )?;
    println!("{}: {}", result.vm.name, result.vm.state);
    Ok(())
}

fn cmd_stop(
    cli: &Cli,
    name: &str,
    force: bool,
    timeout: u64,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let result: vmos_client::VmGetResult = client.call_long(
        "vm.stop",
        &VmStopParams {
            name: name.to_string(),
            force,
            timeout_secs: timeout,
        },
    )?;
    println!("{}: {}", result.vm.name, result.vm.state);
    Ok(())
}

fn cmd_pause(cli: &Cli, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let result: vmos_client::VmGetResult = client.call(
        "vm.pause",
        &vmos_client::VmNameParams {
            name: name.to_string(),
        },
    )?;
    println!("{}: {}", result.vm.name, result.vm.state);
    Ok(())
}

fn cmd_resume(cli: &Cli, name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let result: vmos_client::VmGetResult = client.call(
        "vm.resume",
        &vmos_client::VmNameParams {
            name: name.to_string(),
        },
    )?;
    println!("{}: {}", result.vm.name, result.vm.state);
    Ok(())
}

fn cmd_delete(cli: &Cli, name: &str, force: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let _: serde_json::Value = client.call(
        "vm.delete",
        &VmDeleteParams {
            name: name.to_string(),
            force,
        },
    )?;
    println!("deleted {name}");
    Ok(())
}

fn cmd_clone(cli: &Cli, source: &str, target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let params = vmos_client::VmCloneParams {
        source: source.to_string(),
        target: target.to_string(),
    };
    let result: vmos_client::VmGetResult = client.call_long("vm.clone", &params)?;
    println!("cloned {} -> {}", source, result.vm.name);
    Ok(())
}

fn cmd_modify(
    cli: &Cli,
    name: &str,
    cpus: Option<u32>,
    memory: Option<u64>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let params = VmModifyParams {
        name: name.to_string(),
        cpus,
        memory_mib: memory,
    };
    let result: vmos_client::VmGetResult = client.call("vm.modify", &params)?;
    println!(
        "{}: {} cpus, {}",
        result.vm.name,
        result.vm.config.cpus,
        vmos_core::util::format_size(result.vm.config.memory_mib * 1024 * 1024)
    );
    Ok(())
}

fn cmd_save(cli: &Cli, name: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let params = vmos_client::VmSaveParams {
        name: name.to_string(),
        path: path.to_path_buf(),
    };
    let _: serde_json::Value = client.call_long("vm.save", &params)?;
    println!("saved {name} to {}", path.display());
    Ok(())
}

fn cmd_restore(cli: &Cli, name: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let params = vmos_client::VmRestoreParams {
        name: name.to_string(),
        path: path.to_path_buf(),
    };
    let result: vmos_client::VmGetResult = client.call_long("vm.restore", &params)?;
    println!("{}: {}", result.vm.name, result.vm.state);
    Ok(())
}

fn cmd_install(
    cli: &Cli,
    name: &str,
    restore_image: &Path,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let params = vmos_client::VmInstallParams {
        name: name.to_string(),
        restore_image: restore_image.to_path_buf(),
    };
    let result: vmos_client::VmGetResult = client.call_long("vm.install", &params)?;
    println!("{}: {}", result.vm.name, result.vm.state);
    Ok(())
}

fn cmd_backup(cli: &Cli, name: &str, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let root = store_root(cli);
    let vms_dir = root.join("vms");
    let status = std::process::Command::new("tar")
        .arg("czf")
        .arg(path)
        .arg("-C")
        .arg(&vms_dir)
        .arg(name)
        .status()?;
    if !status.success() {
        return Err("backup failed".into());
    }
    println!("backed up {name} to {}", path.display());
    Ok(())
}

fn cmd_restore_backup(cli: &Cli, path: &Path, name: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    let root = store_root(cli);
    let vms_dir = root.join("vms");
    std::fs::create_dir_all(&vms_dir)?;
    let status = std::process::Command::new("tar")
        .arg("xzf")
        .arg(path)
        .arg("-C")
        .arg(&vms_dir)
        .status()?;
    if !status.success() {
        return Err("restore failed".into());
    }
    if let Some(name) = name {
        println!("restored backup to {name}");
    } else {
        println!("restored backup to {}", vms_dir.display());
    }
    Ok(())
}

fn cmd_logs(cli: &Cli, name: &str, lines: usize) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let result: vmos_client::LogsResult = client.call(
        "vm.logs",
        &LogsParams {
            name: name.to_string(),
            lines,
        },
    )?;
    for line in result.lines {
        println!("{line}");
    }
    Ok(())
}

fn cmd_snapshot(cli: &Cli, command: &Commands) -> Result<(), Box<dyn std::error::Error>> {
    let Commands::Snapshot { command } = command else {
        return Err("internal error".into());
    };
    match command {
        SnapshotCommands::List { vm } => {
            let mut client = ensure_daemon(cli)?;
            let result: vmos_client::SnapshotListResult = client.call(
                "snapshot.list",
                &vmos_client::SnapshotListParams { vm: vm.clone() },
            )?;
            if result.snapshots.is_empty() {
                println!("no snapshots for {vm}");
            } else {
                for snapshot in result.snapshots {
                    println!(
                        "{}  {}  {}",
                        snapshot.name,
                        snapshot.created_at,
                        snapshot.description.unwrap_or_default()
                    );
                }
            }
        }
        SnapshotCommands::Create {
            vm,
            name,
            description,
        } => {
            let mut client = ensure_daemon(cli)?;
            let params = vmos_client::SnapshotCreateParams {
                vm: vm.clone(),
                name: name.clone(),
                description: description.clone(),
            };
            let result: vmos_client::SnapshotResult =
                client.call_long("snapshot.create", &params)?;
            println!("created snapshot {} for {}", result.snapshot.name, vm);
        }
        SnapshotCommands::Restore { vm, name } => {
            let mut client = ensure_daemon(cli)?;
            let params = vmos_client::SnapshotNameParams {
                vm: vm.clone(),
                name: name.clone(),
            };
            let result: vmos_client::SnapshotResult =
                client.call_long("snapshot.restore", &params)?;
            println!("restored snapshot {} for {}", result.snapshot.name, vm);
        }
        SnapshotCommands::Delete { vm, name } => {
            let mut client = ensure_daemon(cli)?;
            let params = vmos_client::SnapshotNameParams {
                vm: vm.clone(),
                name: name.clone(),
            };
            let _: serde_json::Value = client.call("snapshot.delete", &params)?;
            println!("deleted snapshot {name} from {vm}");
        }
    }
    Ok(())
}

fn cmd_disk(cli: &Cli, command: &Commands) -> Result<(), Box<dyn std::error::Error>> {
    let Commands::Disk { command } = command else {
        return Err("internal error".into());
    };
    match command {
        DiskCommands::List { vm } => {
            let mut client = ensure_daemon(cli)?;
            let result: vmos_client::DiskListResult =
                client.call("disk.list", &vmos_client::DiskListParams { vm: vm.clone() })?;
            for disk in result.disks {
                println!(
                    "{}  {}  {}",
                    disk.id,
                    vmos_core::util::format_size(disk.size_bytes),
                    disk.path.display()
                );
            }
        }
        DiskCommands::Add { vm, id, size, path } => {
            let mut client = ensure_daemon(cli)?;
            let params = DiskAddParams {
                vm: vm.clone(),
                id: id.clone(),
                size_bytes: parse_size(size)?,
                path: path.clone(),
            };
            let result: vmos_client::DiskResult = client.call_long("disk.add", &params)?;
            println!(
                "added disk {} ({})",
                result.disk.id,
                vmos_core::util::format_size(result.disk.size_bytes)
            );
        }
        DiskCommands::Resize { vm, disk, size } => {
            let mut client = ensure_daemon(cli)?;
            let params = DiskResizeParams {
                vm: vm.clone(),
                disk: disk.clone(),
                size_bytes: parse_size(size)?,
            };
            let result: vmos_client::DiskResult = client.call_long("disk.resize", &params)?;
            println!(
                "resized disk {} to {}",
                result.disk.id,
                vmos_core::util::format_size(result.disk.size_bytes)
            );
        }
        DiskCommands::Remove { vm, disk, delete } => {
            let mut client = ensure_daemon(cli)?;
            let params = DiskRemoveParams {
                vm: vm.clone(),
                disk: disk.clone(),
                delete: *delete,
            };
            let result: vmos_client::DiskResult = client.call("disk.remove", &params)?;
            println!("removed disk {}", result.disk.id);
        }
        DiskCommands::Clone { vm, disk, target } => {
            let mut client = ensure_daemon(cli)?;
            let params = DiskCloneParams {
                vm: vm.clone(),
                disk: disk.clone(),
                target: target.clone(),
            };
            let result: vmos_client::DiskResult = client.call_long("disk.clone", &params)?;
            println!("cloned disk {} to {}", result.disk.id, target.display());
        }
        DiskCommands::Compact { vm, disk } => {
            let mut client = ensure_daemon(cli)?;
            let params = vmos_client::DiskCompactParams {
                vm: vm.clone(),
                disk: disk.clone(),
            };
            let result: vmos_client::DiskResult = client.call_long("disk.compact", &params)?;
            println!("compacted disk {}", result.disk.id);
        }
    }
    Ok(())
}

fn cmd_daemon(cli: &Cli, command: &Commands) -> Result<(), Box<dyn std::error::Error>> {
    let Commands::Daemon { command } = command else {
        return Err("internal error".into());
    };
    match command {
        DaemonCommands::Start { foreground } => {
            if *foreground {
                #[cfg(target_os = "macos")]
                {
                    let root = store_root(cli);
                    let store = VmStore::new(root.clone());
                    store.require_initialized()?;
                    let options = vmos_daemon::DaemonOptions {
                        store_root: Some(root),
                        foreground: true,
                    };
                    vmos_daemon::run(options)?;
                    return Ok(());
                }
                #[cfg(not(target_os = "macos"))]
                {
                    return Err("daemon is only supported on macOS".into());
                }
            }
            spawn_daemon(cli)?;
            // wait for readiness
            for _ in 0..100 {
                std::thread::sleep(Duration::from_millis(100));
                if Client::connect_if_running(daemon_socket(cli)).is_some() {
                    println!("daemon started");
                    return Ok(());
                }
            }
            Err("daemon did not become ready".into())
        }
        DaemonCommands::Stop => {
            stop_daemon_process(cli)?;
            println!("daemon stopped");
            Ok(())
        }
        DaemonCommands::Status => {
            match Client::connect_if_running(daemon_socket(cli)) {
                Some(mut client) => {
                    let ping = client.ping()?;
                    println!("running (pid {}, version {})", ping.pid, ping.version);
                    let info = client.call::<_, vmos_client::SystemInfo>(
                        "system.info",
                        &vmos_client::Empty {},
                    )?;
                    println!("store: {}", info.store_root.display());
                    println!("vms:   {}", info.vm_count);
                }
                None => println!("not running"),
            }
            Ok(())
        }
        DaemonCommands::Run { foreground } => {
            #[cfg(target_os = "macos")]
            {
                let root = store_root(cli);
                let store = VmStore::new(root.clone());
                store.require_initialized()?;
                let options = vmos_daemon::DaemonOptions {
                    store_root: Some(root),
                    foreground: *foreground,
                };
                vmos_daemon::run(options)?;
                Ok(())
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = foreground;
                Err("daemon is only supported on macOS".into())
            }
        }
    }
}

fn cmd_doctor(cli: &Cli, json: bool) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = match ensure_daemon(cli) {
        Ok(client) => client,
        Err(_) => {
            // Try capabilities without daemon by direct vz? Fall back to spawning.
            spawn_daemon(cli)?;
            for _ in 0..100 {
                std::thread::sleep(Duration::from_millis(100));
                if Client::connect_if_running(daemon_socket(cli)).is_some() {
                    break;
                }
            }
            ensure_daemon(cli)?
        }
    };
    let caps = client
        .call::<_, vmos_client::Capabilities>("system.capabilities", &vmos_client::Empty {})?;
    if json {
        print_json(&serde_json::to_value(&caps)?);
        return Ok(());
    }
    println!("vz supported:        {}", caps.vz_supported);
    println!("cpus:                {}..{}", caps.min_cpus, caps.max_cpus);
    println!(
        "memory:              {}..{}",
        vmos_core::util::format_size(caps.min_memory_bytes),
        vmos_core::util::format_size(caps.max_memory_bytes)
    );
    println!("store:               {}", caps.default_store_root.display());
    println!("socket:              {}", caps.daemon_socket.display());
    println!("os/arch:             {}/{}", caps.os, caps.arch);
    println!("bridged interfaces:");
    for iface in caps.bridged_interfaces {
        println!(
            "  {}  {}",
            iface.identifier,
            iface.display_name.unwrap_or_default()
        );
    }
    Ok(())
}

fn cmd_events(
    cli: &Cli,
    after: Option<u64>,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut client = ensure_daemon(cli)?;
    let result = client.call::<_, vmos_client::EventPollResult>(
        "events.poll",
        &vmos_client::EventPollParams {
            after,
            limit: Some(limit),
        },
    )?;
    for event in result.events {
        println!(
            "{}  {}  {}",
            event.seq,
            event.at,
            serde_json::to_string(&event.kind).unwrap_or_default()
        );
    }
    println!("next: {}", result.next);
    Ok(())
}
