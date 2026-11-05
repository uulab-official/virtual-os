# Wire protocol

Transport: newline-delimited JSON-RPC 2.0 over `$VOS_HOME/daemon.sock`.

- One request/response per line, UTF-8.
- Socket mode `0600`; peer UID must match the daemon's UID.
- Maximum line size 4 MiB.

## Request

```json
{"jsonrpc":"2.0","id":1,"method":"vm.list","params":{}}
```

## Response

```json
{"jsonrpc":"2.0","id":1,"result":{"vms":[]}}
```

or

```json
{"jsonrpc":"2.0","id":1,"error":{"code":-32001,"message":"...","data":{"kind":"vm_not_found"}}}
```

## Methods

System: `system.ping`, `system.info`, `system.capabilities`, `system.shutdown`.

VMs: `vm.create`, `vm.list`, `vm.get`, `vm.start`, `vm.stop`, `vm.pause`,
`vm.resume`, `vm.delete`, `vm.clone`, `vm.modify`, `vm.logs`.

Snapshots: `snapshot.list`, `snapshot.create`, `snapshot.restore`, `snapshot.delete`.

Disks: `disk.list`, `disk.add`, `disk.resize`, `disk.remove`, `disk.clone`.

Events: `events.poll`.

See `crates/vmos-client/src/protocol.rs` for the exact parameter/result shapes.
