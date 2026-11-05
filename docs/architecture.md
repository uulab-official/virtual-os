# Architecture

## Store layout

```
$VOS_HOME (default ~/.virtual-os)
├── vms/
│   └── <vm-name>/
│       ├── vm.json          # versioned VM configuration
│       ├── runtime.json     # runtime state (pid, state, last error)
│       ├── nvram/           # EFI variable store
│       ├── disks/           # raw disk images
│       └── snapshots/       # cold snapshots
├── daemon.sock              # JSON-RPC Unix socket
├── daemon.pid
├── daemon.log
└── daemon.lock              # advisory exclusivity lock
```

## Concurrency model

- One daemon per store, enforced by an advisory file lock on `daemon.lock`.
- Lifecycle operations take a single daemon-wide mutex (`ops`), so two
  `vm.start` calls never race on the framework.
- Per-VM file locks in `vmos-core` protect disk/snapshot mutations even when
  the daemon is not running.
- The framework requires all `VZVirtualMachine` calls on a dedicated serial
  dispatch queue; `vmos-vz` owns that queue and exposes only queue-affine
  methods.

## Boot modes

- **Linux** — `VZLinuxBootLoader` with kernel, optional initrd and command line.
- **EFI** — `VZEFIBootLoader` with a persistent variable store (`nvram/efi-vars.fd`).
- **macOS** — `VZMacOSBootLoader` from a hardware model, machine identifier and
  auxiliary storage. Installation from a restore image is modeled in config but
  not yet automated.

## Networking

- NAT via `VZNATNetworkDeviceAttachment`.
- Bridged via `VZBridgedNetworkDeviceAttachment` on a host interface discovered
  through `VZBridgedNetworkInterface.networkInterfaces`.

## Snapshots

Cold snapshots only: the VM must be stopped. A snapshot captures `vm.json`,
all disk images (via `clonefile` on APFS), EFI NVRAM, the machine identifier and
macOS auxiliary storage.

## Runtime state

`runtime.json` is written by the daemon. `effective_runtime()` in `vmos-core`
detects stale records (dead PID) and treats them as `Stopped`, so a crashed
daemon never leaves a VM permanently "running".
