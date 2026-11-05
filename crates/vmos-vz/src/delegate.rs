//! Delegate and shared lifecycle state.
//!
//! All delegate callbacks arrive on the VM's serial dispatch queue. They only
//! update a small mutex-protected state cell and signal a condition variable,
//! so they never block the framework.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use objc2::rc::Retained;
use objc2::{define_class, msg_send, AllocAnyThread, DefinedClass};
use objc2_foundation::{NSError, NSObject, NSObjectProtocol};
use objc2_virtualization::{VZNetworkDevice, VZVirtualMachine, VZVirtualMachineDelegate};

use crate::state::VmState;
use crate::util::describe_ns_error;

#[derive(Debug, Clone)]
struct StateInner {
    state: VmState,
    last_error: Option<String>,
}

pub(crate) struct StateCell {
    inner: Mutex<StateInner>,
    changed: Condvar,
}

impl StateCell {
    pub(crate) fn new() -> Self {
        Self {
            inner: Mutex::new(StateInner {
                state: VmState::Stopped,
                last_error: None,
            }),
            changed: Condvar::new(),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, StateInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    pub(crate) fn state(&self) -> VmState {
        self.lock().state
    }

    pub(crate) fn last_error(&self) -> Option<String> {
        self.lock().last_error.clone()
    }

    pub(crate) fn set(&self, state: VmState, error: Option<String>) {
        let mut guard = self.lock();
        guard.state = state;
        guard.last_error = error;
        drop(guard);
        self.changed.notify_all();
    }

    pub(crate) fn fail(&self, message: impl Into<String>) {
        self.set(VmState::Error, Some(message.into()));
    }

    /// Wait until `predicate` is true, or the timeout expires.
    pub(crate) fn wait_until<F>(&self, timeout: Duration, predicate: F) -> bool
    where
        F: Fn(VmState) -> bool,
    {
        let deadline = Instant::now() + timeout;
        let mut guard = self.lock();
        loop {
            if predicate(guard.state) {
                return true;
            }
            let now = Instant::now();
            if now >= deadline {
                return false;
            }
            let remaining = deadline.saturating_duration_since(now);
            let (next, timeout_result) = self
                .changed
                .wait_timeout(guard, remaining)
                .unwrap_or_else(|poison| poison.into_inner());
            guard = next;
            if timeout_result.timed_out() && !predicate(guard.state) {
                return false;
            }
        }
    }
}

pub(crate) struct DelegateIvars {
    cell: Arc<StateCell>,
}

define_class!(
    // SAFETY:
    // - NSObject has no subclassing requirements.
    // - The ivars only contain an Arc; no manual Drop is required.
    // - Delegate methods only touch the shared state cell.
    #[unsafe(super(NSObject))]
    #[name = "VmosVirtualMachineDelegate"]
    #[ivars = DelegateIvars]
    pub(crate) struct VmDelegate;

    unsafe impl NSObjectProtocol for VmDelegate {}

    unsafe impl VZVirtualMachineDelegate for VmDelegate {
        #[unsafe(method(guestDidStopVirtualMachine:))]
        fn guest_did_stop(&self, _virtual_machine: &VZVirtualMachine) {
            tracing::info!("guest requested virtual machine stop");
            self.ivars().cell.set(VmState::Stopped, None);
        }

        #[unsafe(method(virtualMachine:didStopWithError:))]
        fn did_stop_with_error(&self, _virtual_machine: &VZVirtualMachine, error: &NSError) {
            let message = describe_ns_error(error);
            tracing::error!(error = %message, "virtual machine stopped with error");
            self.ivars().cell.fail(message);
        }

        #[unsafe(method(virtualMachine:networkDevice:attachmentWasDisconnectedWithError:))]
        fn network_disconnected(
            &self,
            _virtual_machine: &VZVirtualMachine,
            _network_device: &VZNetworkDevice,
            error: &NSError,
        ) {
            let message = describe_ns_error(error);
            tracing::warn!(error = %message, "network attachment disconnected");
            // This can happen repeatedly and is recoverable; do not transition
            // the VM to Error. Keep the reason for diagnostics.
            let mut guard = self.ivars().cell.lock();
            guard.last_error = Some(format!("network attachment disconnected: {message}"));
        }
    }
);

impl VmDelegate {
    pub(crate) fn new(cell: Arc<StateCell>) -> Retained<Self> {
        let this = Self::alloc().set_ivars(DelegateIvars { cell });
        // SAFETY: NSObject's init is always valid.
        unsafe { msg_send![super(this), init] }
    }
}
