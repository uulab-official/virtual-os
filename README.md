# virtual-os

A small, open-source virtual machine manager for macOS, written in Rust and built
on Apple's `Virtualization.framework`.

`virtual-os` (`vmos`) gives you a daemon, a CLI, a JSON-RPC protocol and a C SDK
for creating and running Linux (direct kernel boot), EFI/UEFI and macOS guests on
Apple Silicon and Intel Macs. It is intentionally compact and auditable: the
whole control plane is a few thousand lines of Rust with no hidden hypervisor.

## Status

Early but functional. The daemon, CLI, disk/snapshot management, networking,
shared folders and serial consoles are implemented. macOS guest *installation*
is modeled but not yet automated; Linux and EFI guests boot today.

Runtime VM operations require a binary signed with the
`com.apple.security.virtualization` entitlement (see [Development](#development)).

## Architecture

```
┌────────────┐     JSON-RPC 2.0     ┌──────────────┐
│  vmos CLI  │ ───────────────────► │ vmos-daemon  │
└────────────┘   Unix socket         └──────┬───────┘
                                           │
┌────────────┐                              │  Virtualization.framework
│  vmos-sdk  │ ─────────────────────────────┤
└────────────┘                              ▼
                                     ┌──────────────┐
                                     │  vmos-vz     │
                                     └──────────────┘
┌──────────────┐
│  vmos-core   │  config, store, disks, snapshots, net
└──────────────┘
```

Crates:

- `vmos-core` — VM configuration, on-disk store, raw disks, snapshots, networking.
- `vmos-vz` — `Virtualization.framework` backend (macOS only).
- `vmos-client` — wire protocol and blocking JSON-RPC client.
- `vmos-daemon` — one daemon per store, owns all running VMs.
- `vmos-cli` — the `vmos` command.
- `vmos-gui` — a desktop GUI (eframe/egui) for managing VMs.
- `vmos-sdk` — C ABI (`libvmos_sdk`).

## Quick start

```bash
# 1. Build
cargo build --workspace

# 2. Sign with the virtualization entitlement (required for vm.start)
./scripts/sign-dev.sh debug

# 3. Initialize a store
./target/debug/vmos init

# 4. Create a Linux VM (direct boot)
./target/debug/vmos create my-linux \
    --boot linux \
    --kernel /path/to/vmlinuz \
    --initrd /path/to/initrd.img \
    --cmdline "console=hvc0 root=/dev/vda" \
    --disk root:20G

# 5. Start it
./target/debug/vmos start my-linux
```

Or use the helper:

```bash
./scripts/run-dev.sh create my-linux --boot linux --kernel ...
```

## Documentation

- [Architecture](docs/architecture.md)
- [CLI reference](docs/cli.md)
- [Wire protocol](docs/protocol.md)
- [Development](docs/development.md)
- [Roadmap](docs/roadmap.md)
- [Parallels Desktop analysis](docs/analysis/overview.md)

## License

MIT — see [LICENSE](LICENSE).
