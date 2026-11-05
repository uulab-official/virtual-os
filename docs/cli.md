# CLI reference

All commands accept `--store <PATH>` (or `VOS_HOME`) to override the store root.

## Store

```
vmos init
```

## Virtual machines

```
vmos create NAME [--cpus N] [--memory MiB] [--boot linux|efi|macos] ...
vmos list [--json]
vmos show NAME [--json]
vmos start NAME
vmos stop NAME [--force] [--timeout SECONDS]
vmos pause NAME
vmos resume NAME
vmos delete NAME [--force]
vmos clone SOURCE TARGET
vmos modify NAME [--cpus N] [--memory MiB]
vmos logs NAME [--lines N]
vmos save NAME PATH
vmos restore NAME PATH
vmos install NAME RESTORE_IMAGE
```

### `create` options

- `--boot linux --kernel PATH [--initrd PATH] [--cmdline STR]`
- `--boot efi`
- `--boot macos --hardware-model PATH --machine-identifier PATH --auxiliary-storage PATH [--restore-image PATH]`
- `--disk ID:SIZE[:PATH]` (repeatable; SIZE accepts `20G`, `4096M`, …)
- `--network nat|bridged[:IFACE]` (repeatable)
- `--share TAG:HOSTPATH[:ro]` (repeatable)
- `--serial PATH`
- `--display-name STR`
- `--tag KEY=VALUE` (repeatable)

## Snapshots

```
vmos snapshot list VM
vmos snapshot create VM NAME [--description TEXT]
vmos snapshot restore VM NAME
vmos snapshot delete VM NAME
```

## Disks

```
vmos disk list VM
vmos disk add VM ID SIZE [--path PATH]
vmos disk resize VM DISK SIZE
vmos disk remove VM DISK [--delete]
vmos disk clone VM DISK TARGET
vmos disk compact VM DISK
```

## Daemon

```
vmos daemon start [--foreground]
vmos daemon stop
vmos daemon status
vmos daemon run [--foreground]   # internal entrypoint
```

## Diagnostics

```
vmos doctor [--json]
vmos events [--after SEQ] [--limit N]
vmos version
```
