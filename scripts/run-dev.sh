#!/usr/bin/env bash
set -euo pipefail

# Build, sign, and run a single `vmos` command with the development entitlements.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "${ROOT}"

PROFILE="${PROFILE:-debug}"
cargo build -p vmos-cli -p vmos-daemon

"${ROOT}/scripts/sign-dev.sh" "${PROFILE}"

exec "${ROOT}/target/${PROFILE}/vmos" "$@"
