# Security

`virtual-os` runs local virtual machines. The daemon trusts the local user
owning the store.

## Reporting a vulnerability

Please do not open public issues for security problems. Email the maintainers
or use GitHub's private vulnerability reporting. We aim to acknowledge within
72 hours.

## Scope notes

- The daemon socket is mode `0600` and peer UID is checked.
- Guests are isolated by `Virtualization.framework`; this project does not
  implement its own hypervisor.
- Ad-hoc signing is for development only. Production use should use a real
  signing identity and hardened entitlements.
- Bridged networking requires `com.apple.vm.networking`, which may need a
  privileged signing identity; treat bridged interfaces as trusted networks.
