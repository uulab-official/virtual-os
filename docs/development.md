# Development

## Prerequisites

- Rust 1.89+ (see `rust-version` in `Cargo.toml`).
- macOS (Virtualization.framework). Linux can build and test `vmos-core` only.
- Xcode command line tools for `codesign`.

## Build

```bash
cargo build --workspace
```

## Sign (required for `vm.start`)

```bash
./scripts/sign-dev.sh debug
```

Ad-hoc signing is sufficient for local development on the same Mac. Bridged
networking and distribution require a real signing identity and additional
entitlements.

## Test

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Run

```bash
./scripts/run-dev.sh init
./scripts/run-dev.sh daemon start --foreground   # in one terminal
./scripts/run-dev.sh list                         # in another
```

## Entitlements

`entitlements/virtualization.entitlements` declares
`com.apple.security.virtualization`. Add `com.apple.vm.networking` for bridged
networking when using a real signing identity.
