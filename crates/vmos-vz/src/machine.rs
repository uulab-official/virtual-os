//! VM lifecycle on a dedicated serial dispatch queue.
//!
//! Apple's documentation requires every call on `VZVirtualMachine` to happen
//! on the queue passed to `initWithConfiguration:queue:`. Completion handlers
//! and delegate callbacks run on that queue too. This module therefore
//! dispatches all framework calls to the queue and bridges completion handlers
//! back to the calling thread through a channel. The handle itself is safe to
//! move between threads because the underlying object is only dereferenced
//! inside queue work items.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::Arc;
use std::time::Duration;

use block2::{DynBlock, RcBlock};
use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::AllocAnyThread;
use objc2_foundation::{NSError, NSString, NSURL};
use objc2_virtualization::{
    VZMacOSInstaller, VZVirtualMachine, VZVirtualMachineConfiguration, VZVirtualMachineState,
};

use crate::config::{self, VzSpec};
use crate::delegate::{StateCell, VmDelegate};
use crate::error::{exception_error, framework_error, VzError};
use crate::state::VmState;

const OPERATION_TIMEOUT: Duration = Duration::from_secs(300);

type ErrorBlock = DynBlock<dyn Fn(*mut NSError)>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostLimits {
    pub min_cpus: usize,
    pub max_cpus: usize,
    pub min_memory_bytes: u64,
    pub max_memory_bytes: u64,
}

struct SendVm(Retained<VZVirtualMachine>);

// SAFETY: VZVirtualMachine is only dereferenced inside work submitted to its
// serial queue. All public methods of VzMachine uphold that invariant.
unsafe impl Send for SendVm {}
unsafe impl Sync for SendVm {}

pub struct VzMachine {
    vm: SendVm,
    queue: DispatchRetained<DispatchQueue>,
    _delegate: Retained<VmDelegate>,
    cell: Arc<StateCell>,
    serial_log: Option<PathBuf>,
}

// SAFETY: access to the underlying VZVirtualMachine is serialized through the
// dispatch queue; the state cell is synchronized.
unsafe impl Send for VzMachine {}
unsafe impl Sync for VzMachine {}

impl VzMachine {
    pub fn new(spec: VzSpec, queue_label: impl AsRef<str>) -> Result<Self, VzError> {
        if !Self::is_supported() {
            return Err(VzError::Unsupported);
        }
        let configuration = config::build_configuration(&spec)?;
        let queue = DispatchQueue::new(queue_label.as_ref(), None);

        let vm = match objc2::exception::catch(std::panic::AssertUnwindSafe(|| unsafe {
            VZVirtualMachine::initWithConfiguration_queue(
                VZVirtualMachine::alloc(),
                &configuration,
                &queue,
            )
        })) {
            Ok(vm) => vm,
            Err(exception) => {
                return Err(exception_error(
                    "creating virtual machine",
                    exception.as_deref(),
                ));
            }
        };

        let cell = Arc::new(StateCell::new());
        let delegate = VmDelegate::new(Arc::clone(&cell));
        // SAFETY: the delegate is retained by this handle for the VM's lifetime.
        unsafe { vm.setDelegate(Some(ProtocolObject::from_ref(&*delegate))) };

        Ok(Self {
            vm: SendVm(vm),
            queue,
            _delegate: delegate,
            cell,
            serial_log: spec.serial.map(|serial| serial.path),
        })
    }

    /// Whether Virtualization.framework can run a VM on this host.
    pub fn is_supported() -> bool {
        // SAFETY: static capability query.
        unsafe { VZVirtualMachine::isSupported() }
    }

    /// Framework-imposed CPU and memory range for this host.
    pub fn host_limits() -> HostLimits {
        // SAFETY: static framework queries.
        unsafe {
            HostLimits {
                min_cpus: VZVirtualMachineConfiguration::minimumAllowedCPUCount(),
                max_cpus: VZVirtualMachineConfiguration::maximumAllowedCPUCount(),
                min_memory_bytes: VZVirtualMachineConfiguration::minimumAllowedMemorySize(),
                max_memory_bytes: VZVirtualMachineConfiguration::maximumAllowedMemorySize(),
            }
        }
    }

    pub fn serial_log(&self) -> Option<&Path> {
        self.serial_log.as_deref()
    }

    pub fn state(&self) -> VmState {
        self.cell.state()
    }

    pub fn last_error(&self) -> Option<String> {
        self.cell.last_error()
    }

    pub fn is_running(&self) -> bool {
        matches!(self.state(), VmState::Running | VmState::Paused)
    }

    pub fn start(&self) -> Result<(), VzError> {
        self.require_state(&[VmState::Stopped, VmState::Error], "start")?;
        self.cell.set(VmState::Starting, None);
        let result = self.run_completion("start", |vm, block| {
            // SAFETY: called on the VM's serial queue.
            unsafe { vm.startWithCompletionHandler(block) };
        });
        match &result {
            Ok(()) => self.cell.set(VmState::Running, None),
            Err(error) => self.cell.fail(error.to_string()),
        }
        result
    }

    /// Hard stop, equivalent to pulling the plug.
    pub fn stop(&self) -> Result<(), VzError> {
        if self.state() == VmState::Stopped {
            return Ok(());
        }
        if !self.state().can_stop() {
            return Err(self.invalid_state("stop"));
        }
        self.cell.set(VmState::Stopping, None);
        let result = self.run_completion("stop", |vm, block| {
            // SAFETY: called on the VM's serial queue.
            unsafe { vm.stopWithCompletionHandler(block) };
        });
        match &result {
            Ok(()) => self.cell.set(VmState::Stopped, None),
            Err(error) => self.cell.fail(error.to_string()),
        }
        result
    }

    /// Ask the guest to shut down through its ACPI power button.
    pub fn request_stop(&self) -> Result<(), VzError> {
        if self.state() != VmState::Running {
            return Err(self.invalid_state("request stop"));
        }
        let (tx, rx) = mpsc::sync_channel(1);
        let vm_addr = self.vm_addr();
        self.queue.exec_async(move || {
            // SAFETY: called on the VM's serial queue; the handle is alive for
            // the duration of this synchronous wait.
            let result = unsafe { (*(vm_addr as *const VZVirtualMachine)).requestStopWithError() }
                .map_err(|error| framework_error("request stop", &error));
            let _ = tx.send(result);
        });
        rx.recv_timeout(Duration::from_secs(30))
            .map_err(|_| VzError::timeout("request stop", Duration::from_secs(30)))?
    }

    pub fn pause(&self) -> Result<(), VzError> {
        self.require_state(&[VmState::Running], "pause")?;
        self.cell.set(VmState::Pausing, None);
        let result = self.run_completion("pause", |vm, block| {
            // SAFETY: called on the VM's serial queue.
            unsafe { vm.pauseWithCompletionHandler(block) };
        });
        match &result {
            Ok(()) => self.cell.set(VmState::Paused, None),
            Err(error) => self.cell.fail(error.to_string()),
        }
        result
    }

    pub fn resume(&self) -> Result<(), VzError> {
        self.require_state(&[VmState::Paused], "resume")?;
        self.cell.set(VmState::Resuming, None);
        let result = self.run_completion("resume", |vm, block| {
            // SAFETY: called on the VM's serial queue.
            unsafe { vm.resumeWithCompletionHandler(block) };
        });
        match &result {
            Ok(()) => self.cell.set(VmState::Running, None),
            Err(error) => self.cell.fail(error.to_string()),
        }
        result
    }

    /// Save a paused VM's memory and device state to disk.
    pub fn save_state(&self, path: &Path) -> Result<(), VzError> {
        self.require_state(&[VmState::Paused], "save state")?;
        self.cell.set(VmState::Saving, None);
        let result = self.run_url_completion("save state", path, |vm, url, block| {
            // SAFETY: called on the VM's serial queue with a valid file URL.
            unsafe { vm.saveMachineStateToURL_completionHandler(url, block) };
        });
        // The VM remains Paused whether saving succeeded or failed.
        match &result {
            Ok(()) => {
                if let Ok(file) = std::fs::File::open(path) {
                    let _ = file.sync_all();
                }
                self.cell.set(VmState::Paused, None);
            }
            Err(error) => self.cell.set(VmState::Paused, Some(error.to_string())),
        }
        result
    }

    /// Restore a stopped VM from a saved state file. Ends in `Paused`.
    pub fn restore_state(&self, path: &Path) -> Result<(), VzError> {
        self.require_state(&[VmState::Stopped], "restore state")?;
        self.cell.set(VmState::Restoring, None);
        let result = self.run_url_completion("restore state", path, |vm, url, block| {
            // SAFETY: called on the VM's serial queue with a valid file URL.
            unsafe { vm.restoreMachineStateFromURL_completionHandler(url, block) };
        });
        match &result {
            Ok(()) => self.cell.set(VmState::Paused, None),
            Err(error) => self.cell.set(VmState::Stopped, Some(error.to_string())),
        }
        result
    }

    /// Graceful shutdown with hard-stop escalation.
    pub fn shutdown(&self, grace: Duration) -> Result<(), VzError> {
        match self.state() {
            VmState::Stopped => Ok(()),
            VmState::Running => {
                if self.request_stop().is_ok() && self.wait_for_stopped(grace) {
                    return Ok(());
                }
                self.stop()
            }
            _ => self.stop(),
        }
    }

    /// Wait until the VM reaches `Stopped` (or `Error`).
    /// Install macOS from a restore image onto a stopped VM.
    pub fn install_macos(&self, restore_image: &Path) -> Result<(), VzError> {
        let restore_path = restore_image.to_path_buf();
        use std::sync::{Condvar, Mutex};
        let result_slot = Arc::new(Mutex::new(None::<Result<(), String>>));
        let condvar = Arc::new(Condvar::new());
        let vm_addr = self.vm_addr();
        let queue = self.queue.clone();
        let slot_clone = Arc::clone(&result_slot);
        let cond_clone = Arc::clone(&condvar);
        queue.exec_async(move || {
            let url = match crate::util::path_to_nsurl(&restore_path) {
                Ok(url) => url,
                Err(error) => {
                    *slot_clone.lock().unwrap() = Some(Err(error.to_string()));
                    cond_clone.notify_one();
                    return;
                }
            };
            // SAFETY: vm_addr is only dereferenced on this queue.
            let vm = unsafe { &*(vm_addr as *const VZVirtualMachine) };
            let installer = unsafe {
                VZMacOSInstaller::initWithVirtualMachine_restoreImageURL(
                    VZMacOSInstaller::alloc(),
                    vm,
                    &url,
                )
            };
            let slot2 = Arc::clone(&slot_clone);
            let cond2 = Arc::clone(&cond_clone);
            let block = RcBlock::new(move |err: *mut NSError| {
                let outcome = if err.is_null() {
                    Ok(())
                } else {
                    Err(unsafe { framework_error("installing macOS", &*err) }.to_string())
                };
                *slot2.lock().unwrap() = Some(outcome);
                cond2.notify_one();
            });
            unsafe { installer.installWithCompletionHandler(&block) };
        });

        let guard = condvar
            .wait_timeout(result_slot.lock().unwrap(), Duration::from_secs(3600))
            .unwrap();
        let _ = guard;
        let mut slot = result_slot.lock().unwrap();
        match slot.take() {
            Some(Ok(())) => Ok(()),
            Some(Err(message)) => Err(VzError::Other(message)),
            None => Err(VzError::timeout(
                "installing macOS",
                Duration::from_secs(3600),
            )),
        }
    }

    pub fn wait_for_stopped(&self, timeout: Duration) -> bool {
        self.cell
            .wait_until(timeout, |state| matches!(state, VmState::Stopped))
    }

    pub fn wait_for_state(&self, target: VmState, timeout: Duration) -> bool {
        self.cell.wait_until(timeout, |state| state == target)
    }

    /// Query the framework state directly (diagnostics only).
    pub fn framework_state(&self) -> Option<VmState> {
        let (tx, rx) = mpsc::sync_channel(1);
        let vm_addr = self.vm_addr();
        self.queue.exec_sync(move || {
            // SAFETY: called on the VM's serial queue.
            let state = unsafe { (*(vm_addr as *const VZVirtualMachine)).state() };
            let _ = tx.send(map_framework_state(state));
        });
        rx.recv_timeout(Duration::from_secs(5)).ok()
    }

    fn require_state(&self, allowed: &[VmState], action: &'static str) -> Result<(), VzError> {
        let state = self.state();
        if allowed.contains(&state) {
            Ok(())
        } else {
            Err(self.invalid_state(action))
        }
    }

    fn invalid_state(&self, action: &'static str) -> VzError {
        VzError::InvalidState {
            state: self.state().label(),
            action,
        }
    }

    fn vm_addr(&self) -> usize {
        &*self.vm.0 as *const VZVirtualMachine as usize
    }

    fn run_completion<F>(&self, operation: &'static str, call: F) -> Result<(), VzError>
    where
        F: FnOnce(&VZVirtualMachine, &ErrorBlock) + Send + 'static,
    {
        let (tx, rx) = mpsc::sync_channel(1);
        let vm_addr = self.vm_addr();
        self.queue.exec_async(move || {
            let block = RcBlock::new(move |error: *mut NSError| {
                let result = if error.is_null() {
                    Ok(())
                } else {
                    // SAFETY: a non-null completion error is valid for the
                    // duration of the callback.
                    Err(framework_error(operation, unsafe { &*error }))
                };
                let _ = tx.send(result);
            });
            // SAFETY: this closure runs on the VM's serial queue.
            let vm = unsafe { &*(vm_addr as *const VZVirtualMachine) };
            call(vm, &block);
        });
        match rx.recv_timeout(OPERATION_TIMEOUT) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err(VzError::timeout(operation, OPERATION_TIMEOUT))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(VzError::other(format!(
                "{operation} completion handler disappeared"
            ))),
        }
    }

    fn run_url_completion(
        &self,
        operation: &'static str,
        path: &Path,
        call: fn(&VZVirtualMachine, &NSURL, &ErrorBlock),
    ) -> Result<(), VzError> {
        let path = path
            .to_str()
            .ok_or_else(|| VzError::invalid_config("state file path is not valid UTF-8"))?
            .to_string();
        self.run_completion(operation, move |vm, block| {
            let ns_path = NSString::from_str(&path);
            // SAFETY: valid path converted to a file URL on the queue thread.
            let url = NSURL::initFileURLWithPath(NSURL::alloc(), &ns_path);
            call(vm, &url, block);
        })
    }
}

fn map_framework_state(state: VZVirtualMachineState) -> VmState {
    if state == VZVirtualMachineState::Stopped {
        VmState::Stopped
    } else if state == VZVirtualMachineState::Starting {
        VmState::Starting
    } else if state == VZVirtualMachineState::Running {
        VmState::Running
    } else if state == VZVirtualMachineState::Pausing {
        VmState::Pausing
    } else if state == VZVirtualMachineState::Paused {
        VmState::Paused
    } else if state == VZVirtualMachineState::Resuming {
        VmState::Resuming
    } else if state == VZVirtualMachineState::Stopping {
        VmState::Stopping
    } else if state == VZVirtualMachineState::Saving {
        VmState::Saving
    } else if state == VZVirtualMachineState::Restoring {
        VmState::Restoring
    } else {
        VmState::Error
    }
}
