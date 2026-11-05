# Roadmap

## Done

- VM config, store, runtime state, per-VM locks.
- Device model: graphics, audio, USB, pointing, keyboard, balloon, vsock, entropy.
- Saved state (save/restore) and macOS installer hook.
- Disk compaction.
- Raw disk create/resize/remove/clone.
- Cold snapshots (config + disks + NVRAM + identifiers).
- Linux direct boot, EFI boot, macOS boot model.
- NAT and bridged networking, virtiofs shares, serial logs.
- Daemon with JSON-RPC, event ring buffer, stale-state recovery, monitor thread.
- CLI covering create/start/stop/pause/resume/delete/clone/modify/logs/snapshot/disk/daemon/doctor/events.
- C SDK.
- Dev signing scripts, Makefile, CI workflows.

## Next

- macOS guest installation flow from a restore image.
- Saved-state (hibernation) wiring end to end.
- Hot-plug-ish disk operations with running VMs where the framework allows.
- Authenticated remote control and multi-user stores.
- Packaging (`.app`, Homebrew formula, signed/notarized releases).
- Deeper Parallels-level analysis via Ghidra/Hopper to close remaining unknowns.
