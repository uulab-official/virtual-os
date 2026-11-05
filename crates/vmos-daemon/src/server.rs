//! Unix-socket JSON-RPC server and daemon lifecycle.
//!
//! A store has at most one daemon. Exclusivity is enforced with an advisory
//! file lock on `$VOS_HOME/daemon.lock`; the socket inode is only a rendezvous
//! point and may be stale after a crash.

use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use fs2::FileExt;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use vmos_client::error_codes;
use vmos_client::methods;
use vmos_client::protocol::{
    DiskAddParams, DiskCloneParams, DiskListParams, DiskListResult, DiskRemoveParams,
    DiskResizeParams, DiskResult, Empty, EventPollParams, LogsParams, PingResult, RpcError,
    RpcRequest, RpcResponse, SnapshotCreateParams, SnapshotListParams, SnapshotNameParams,
    VmCloneParams, VmCreateParams, VmDeleteParams, VmModifyParams, VmNameParams, VmRestoreParams,
    VmSaveParams, VmStopParams, MAX_LINE_BYTES, PROTOCOL_VERSION,
};
use vmos_core::{EventKind, VmStore};

use crate::error::DaemonError;
use crate::manager::Daemon;
use crate::{DaemonOptions, VERSION};

/// Set from a signal handler. Only ever flips from false to true.
static SIGNAL_SHUTDOWN: AtomicBool = AtomicBool::new(false);

extern "C" fn handle_signal(_signal: libc::c_int) {
    SIGNAL_SHUTDOWN.store(true, Ordering::Relaxed);
}

fn install_signal_handlers() {
    // `signal` is async-signal-safe and sufficient here: the handler only
    // stores to an atomic and the accept loop polls it.
    unsafe {
        libc::signal(
            libc::SIGINT,
            handle_signal as *const () as libc::sighandler_t,
        );
        libc::signal(
            libc::SIGTERM,
            handle_signal as *const () as libc::sighandler_t,
        );
    }
}

pub fn run(options: DaemonOptions) -> Result<(), DaemonError> {
    let root = options
        .store_root
        .clone()
        .unwrap_or_else(VmStore::default_root);
    let store = VmStore::new(root);
    store.require_initialized()?;

    let _lock = acquire_daemon_lock(&store)?;
    init_logging(&store, options.foreground)?;

    let socket_path = store.socket_path();
    remove_stale_socket(&socket_path)?;
    let listener = bind_socket(&socket_path)?;
    listener
        .set_nonblocking(true)
        .map_err(|error| DaemonError::Socket(format!("cannot configure socket: {error}")))?;

    write_pid(&store)?;
    install_signal_handlers();

    let daemon = Daemon::new(store.clone());
    daemon.recover_stale_runtimes();
    daemon.events.publish(None, EventKind::DaemonStarted);
    daemon.spawn_monitor();

    tracing::info!(
        version = VERSION,
        pid = std::process::id(),
        store = %store.root().display(),
        socket = %socket_path.display(),
        "virtual-os daemon started"
    );

    let result = accept_loop(&listener, &daemon);

    daemon.events.publish(None, EventKind::DaemonStopping);
    let stopped = daemon.stop_all();
    tracing::info!(stopped, "virtual-os daemon stopping");

    drop(listener);
    let _ = fs::remove_file(&socket_path);
    let _ = fs::remove_file(store.pid_path());
    result
}

fn acquire_daemon_lock(store: &VmStore) -> Result<File, DaemonError> {
    let path = store.daemon_lock_path();
    let file = OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| DaemonError::Socket(format!("cannot open {}: {error}", path.display())))?;
    match file.try_lock_exclusive() {
        Ok(()) => {
            let _ = vmos_core::util::set_private_file(&path);
            Ok(file)
        }
        Err(_) => {
            let pid = fs::read_to_string(store.pid_path())
                .ok()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty())
                .unwrap_or_else(|| "unknown".to_string());
            Err(DaemonError::Socket(format!(
                "another daemon already owns this store (pid {pid}); stop it with `vmos daemon stop`"
            )))
        }
    }
}

fn init_logging(store: &VmStore, foreground: bool) -> Result<(), DaemonError> {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_env("VOS_LOG").unwrap_or_else(|_| EnvFilter::new("info"));

    let builder = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(true)
        .with_ansi(foreground);

    let result = if foreground {
        builder.with_writer(std::io::stderr).try_init()
    } else {
        let appender = tracing_appender::rolling::never(store.root(), "daemon.log");
        builder.with_writer(appender).try_init()
    };
    // A host embedding the daemon may have installed a subscriber already.
    if let Err(error) = result {
        eprintln!("virtual-os daemon: tracing already initialized ({error})");
    }
    Ok(())
}

fn remove_stale_socket(path: &Path) -> Result<(), DaemonError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            if !metadata.file_type().is_socket() {
                return Err(DaemonError::Socket(format!(
                    "{} exists and is not a socket; refusing to remove it",
                    path.display()
                )));
            }
            fs::remove_file(path).map_err(|error| {
                DaemonError::Socket(format!("cannot remove stale socket: {error}"))
            })?;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(DaemonError::Socket(format!(
                "cannot inspect {}: {error}",
                path.display()
            )));
        }
    }
    Ok(())
}

fn bind_socket(path: &Path) -> Result<UnixListener, DaemonError> {
    let listener = UnixListener::bind(path).map_err(|error| {
        DaemonError::Socket(format!(
            "cannot bind {}: {error}. Unix socket paths are limited to about 104 bytes; \
             set VOS_HOME to a shorter path if this is ENAMETOOLONG",
            path.display()
        ))
    })?;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|error| {
        DaemonError::Socket(format!("cannot restrict socket permissions: {error}"))
    })?;
    Ok(listener)
}

fn write_pid(store: &VmStore) -> Result<(), DaemonError> {
    let path = store.pid_path();
    let payload = format!("{}\n", std::process::id());
    vmos_core::util::atomic_write(&path, payload.as_bytes())?;
    let _ = vmos_core::util::set_private_file(&path);
    Ok(())
}

fn accept_loop(listener: &UnixListener, daemon: &Arc<Daemon>) -> Result<(), DaemonError> {
    while !daemon.shutdown_requested() && !SIGNAL_SHUTDOWN.load(Ordering::Relaxed) {
        match listener.accept() {
            Ok((stream, _address)) => {
                let _ = stream.set_nonblocking(false);
                let daemon = Arc::clone(daemon);
                let _ = std::thread::Builder::new()
                    .name("vmos-rpc".into())
                    .spawn(move || {
                        if let Err(error) = handle_connection(&daemon, stream) {
                            tracing::debug!(%error, "client connection closed with an error");
                        }
                    });
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => {
                tracing::error!(%error, "accept failed");
                return Err(DaemonError::Socket(format!("accept failed: {error}")));
            }
        }
    }
    Ok(())
}

fn handle_connection(daemon: &Arc<Daemon>, stream: UnixStream) -> std::io::Result<()> {
    if !peer_is_allowed(&stream, daemon)? {
        return Ok(());
    }
    stream.set_read_timeout(Some(Duration::from_secs(300)))?;
    stream.set_write_timeout(Some(Duration::from_secs(60)))?;

    let write_stream = stream.try_clone()?;
    let mut writer = write_stream;
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    loop {
        line.clear();
        let read = reader.read_line(&mut line)?;
        if read == 0 {
            return Ok(());
        }
        if line.len() > MAX_LINE_BYTES {
            write_response(
                &mut writer,
                &RpcResponse::failure(
                    0,
                    RpcError::new(error_codes::INVALID_REQUEST, "request line too large"),
                ),
            )?;
            return Ok(());
        }

        let request = line.trim_end_matches(['\r', '\n']);
        if request.is_empty() {
            continue;
        }

        let parsed: Result<RpcRequest, _> = serde_json::from_str(request);
        let (id, method, params) = match parsed {
            Ok(request) => (request.id, request.method, request.params),
            Err(error) => {
                write_response(
                    &mut writer,
                    &RpcResponse::failure(
                        0,
                        RpcError::new(error_codes::PARSE_ERROR, format!("invalid JSON: {error}")),
                    ),
                )?;
                continue;
            }
        };

        let (response, shutdown) = match dispatch(daemon, &method, params) {
            Ok(Dispatch::Reply(value)) => (RpcResponse::success(id, value), false),
            Ok(Dispatch::Shutdown(value)) => (RpcResponse::success(id, value), true),
            Err(error) => (RpcResponse::failure(id, error), false),
        };
        write_response(&mut writer, &response)?;
        if shutdown {
            daemon.request_shutdown();
            return Ok(());
        }
    }
}

fn write_response(writer: &mut UnixStream, response: &RpcResponse) -> std::io::Result<()> {
    let mut bytes = serde_json::to_vec(response)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    bytes.push(b'\n');
    writer.write_all(&bytes)?;
    writer.flush()
}

/// Reject clients owned by another user, even though the socket is already
/// mode 0600. This also protects stores on shared or unusual filesystems.
fn peer_is_allowed(stream: &UnixStream, daemon: &Arc<Daemon>) -> std::io::Result<bool> {
    let peer = peer_uid(stream)?;
    let ours = unsafe { libc::geteuid() };
    if peer == ours {
        return Ok(true);
    }
    tracing::warn!(peer, ours, "rejected RPC client from another user");
    let _ = daemon;
    Ok(false)
}

#[cfg(target_os = "macos")]
fn peer_uid(stream: &UnixStream) -> std::io::Result<u32> {
    let mut uid: libc::uid_t = 0;
    let mut gid: libc::gid_t = 0;
    let rc = unsafe { libc::getpeereid(stream.as_raw_fd(), &mut uid, &mut gid) };
    if rc != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(uid)
}

#[cfg(not(target_os = "macos"))]
fn peer_uid(stream: &UnixStream) -> std::io::Result<u32> {
    let _ = stream;
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "peer credentials are only implemented on macOS",
    ))
}

enum Dispatch {
    Reply(Value),
    Shutdown(Value),
}

fn parse_params<T: DeserializeOwned>(params: Value) -> Result<T, RpcError> {
    serde_json::from_value(params).map_err(|error| {
        RpcError::new(
            error_codes::INVALID_PARAMS,
            format!("invalid params: {error}"),
        )
    })
}

fn to_value<T: serde::Serialize>(value: T) -> Result<Value, RpcError> {
    serde_json::to_value(value)
        .map_err(|error| RpcError::new(error_codes::INTERNAL_ERROR, error.to_string()))
}

fn dispatch(daemon: &Arc<Daemon>, method: &str, params: Value) -> Result<Dispatch, RpcError> {
    let value = match method {
        methods::SYSTEM_PING => {
            let _: Empty = parse_params(params)?;
            to_value(PingResult {
                version: VERSION.to_string(),
                protocol_version: PROTOCOL_VERSION,
                pid: std::process::id(),
            })?
        }
        methods::SYSTEM_INFO => {
            let _: Empty = parse_params(params)?;
            to_value(daemon.system_info().map_err(|error| error.to_rpc_error())?)?
        }
        methods::SYSTEM_CAPABILITIES => {
            let _: Empty = parse_params(params)?;
            to_value(daemon.capabilities())?
        }
        methods::SYSTEM_SHUTDOWN => {
            let _: Empty = parse_params(params)?;
            return Ok(Dispatch::Shutdown(json!({ "ok": true })));
        }
        methods::VM_CREATE => {
            let params: VmCreateParams = parse_params(params)?;
            to_value(
                daemon
                    .create(params)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::VM_LIST => {
            let _: Empty = parse_params(params)?;
            to_value(daemon.list().map_err(|error| error.to_rpc_error())?)?
        }
        methods::VM_GET => {
            let params: VmNameParams = parse_params(params)?;
            to_value(
                daemon
                    .get(&params.name)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::VM_START => {
            let params: VmNameParams = parse_params(params)?;
            to_value(
                daemon
                    .start(&params.name)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::VM_STOP => {
            let params: VmStopParams = parse_params(params)?;
            to_value(daemon.stop(params).map_err(|error| error.to_rpc_error())?)?
        }
        methods::VM_PAUSE => {
            let params: VmNameParams = parse_params(params)?;
            to_value(
                daemon
                    .pause(&params.name)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::VM_RESUME => {
            let params: VmNameParams = parse_params(params)?;
            to_value(
                daemon
                    .resume(&params.name)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::VM_DELETE => {
            let params: VmDeleteParams = parse_params(params)?;
            daemon
                .delete(&params.name, params.force)
                .map_err(|error| error.to_rpc_error())?;
            json!({ "ok": true, "name": params.name })
        }
        methods::VM_CLONE => {
            let params: VmCloneParams = parse_params(params)?;
            to_value(
                daemon
                    .clone_vm(params)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::VM_MODIFY => {
            let params: VmModifyParams = parse_params(params)?;
            to_value(
                daemon
                    .modify(params)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::VM_LOGS => {
            let params: LogsParams = parse_params(params)?;
            to_value(
                daemon
                    .logs(&params.name, params.lines)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::VM_SAVE => {
            let params: VmSaveParams = parse_params(params)?;
            daemon
                .save_state(&params.name, &params.path)
                .map_err(|error| error.to_rpc_error())?;
            json!({ "ok": true, "name": params.name, "path": params.path })
        }
        methods::VM_RESTORE => {
            let params: VmRestoreParams = parse_params(params)?;
            to_value(
                daemon
                    .restore_state(&params.name, &params.path)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::SNAPSHOT_LIST => {
            let params: SnapshotListParams = parse_params(params)?;
            to_value(
                daemon
                    .snapshot_list(&params.vm)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::SNAPSHOT_CREATE => {
            let params: SnapshotCreateParams = parse_params(params)?;
            to_value(
                daemon
                    .snapshot_create(params)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::SNAPSHOT_RESTORE => {
            let params: SnapshotNameParams = parse_params(params)?;
            to_value(
                daemon
                    .snapshot_restore(&params.vm, &params.name)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::SNAPSHOT_DELETE => {
            let params: SnapshotNameParams = parse_params(params)?;
            to_value(
                daemon
                    .snapshot_delete(&params.vm, &params.name)
                    .map_err(|error| error.to_rpc_error())?,
            )?
        }
        methods::DISK_LIST => {
            let params: DiskListParams = parse_params(params)?;
            to_value(DiskListResult {
                disks: daemon
                    .disk_list(&params.vm)
                    .map_err(|error| error.to_rpc_error())?,
            })?
        }
        methods::DISK_ADD => {
            let params: DiskAddParams = parse_params(params)?;
            to_value(DiskResult {
                disk: daemon
                    .disk_add(params)
                    .map_err(|error| error.to_rpc_error())?,
            })?
        }
        methods::DISK_RESIZE => {
            let params: DiskResizeParams = parse_params(params)?;
            to_value(DiskResult {
                disk: daemon
                    .disk_resize(params)
                    .map_err(|error| error.to_rpc_error())?,
            })?
        }
        methods::DISK_REMOVE => {
            let params: DiskRemoveParams = parse_params(params)?;
            to_value(DiskResult {
                disk: daemon
                    .disk_remove(params)
                    .map_err(|error| error.to_rpc_error())?,
            })?
        }
        methods::DISK_CLONE => {
            let params: DiskCloneParams = parse_params(params)?;
            to_value(DiskResult {
                disk: daemon
                    .disk_clone(params)
                    .map_err(|error| error.to_rpc_error())?,
            })?
        }
        methods::EVENT_POLL => {
            let params: EventPollParams = parse_params(params)?;
            to_value(daemon.poll_events(params.after, params.limit))?
        }
        other => {
            return Err(RpcError::new(
                error_codes::METHOD_NOT_FOUND,
                format!("unknown method {other:?}"),
            ));
        }
    };
    Ok(Dispatch::Reply(value))
}
