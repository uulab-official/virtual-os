#!/usr/bin/env bash
set -euo pipefail

# Ad-hoc sign the built vmos binaries with the Virtualization entitlement.
# Ad-hoc signing is enough for local development on the same machine; it is
# NOT sufficient for distribution or for bridged networking in some cases.

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PROFILE="${1:-debug}"
TARGET_DIR="${ROOT}/target/${PROFILE}"

if [[ ! -d "${TARGET_DIR}" ]]; then
    echo "build directory not found: ${TARGET_DIR}" >&2
    echo "run 'cargo build' first" >&2
    exit 1
fi

ENTITLEMENTS="${ROOT}/entitlements/virtualization.entitlements"
for binary in vmos vmos-daemon; do
    path="${TARGET_DIR}/${binary}"
    if [[ -f "${path}" ]]; then
        codesign --force --sign - --entitlements "${ENTITLEMENTS}" "${path}"
        echo "signed ${path}"
    fi
done
