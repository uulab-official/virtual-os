# Contributing

Thanks for considering a contribution.

## Getting started

1. Fork and clone.
2. `cargo build --workspace`
3. `./scripts/sign-dev.sh debug`
4. `cargo test --workspace`

## Guidelines

- Keep changes small and focused; one concern per PR.
- Run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`
  and `cargo test --workspace` before submitting.
- Prefer adding tests alongside behavior changes.
- Document public APIs and protocol changes.
- macOS-only code must be gated with `#[cfg(target_os = "macos")]` and must
  not break Linux builds of `vmos-core` and `vmos-client`.

## Commit style

Conventional-ish: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `ci:`.

## Reporting bugs

Use the issue tracker. Include `vmos version`, macOS version, chip, and the
contents of `daemon.log` when relevant.
