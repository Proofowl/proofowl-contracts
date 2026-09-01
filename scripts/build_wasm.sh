#!/usr/bin/env bash
#
# Build the release WASM for the supported Soroban target and print its
# path, size, and sha256. No network, no deploy, no secrets.
#
# Usage:  scripts/build_wasm.sh

set -euo pipefail
_here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
. "${_here}/lib.sh"

step "Build release WASM (${PROOFOWL_WASM_TARGET})"
require_cmd cargo
require_cmd rustup

if ! rustup target list --installed | grep -qx "${PROOFOWL_WASM_TARGET}"; then
  die "rust target ${PROOFOWL_WASM_TARGET} not installed — run: rustup target add ${PROOFOWL_WASM_TARGET}"
fi

( cd "${PROOFOWL_REPO_ROOT}" && cargo build --target "${PROOFOWL_WASM_TARGET}" --release )

[ -f "${PROOFOWL_WASM_PATH}" ] || die "expected artifact not found: ${PROOFOWL_WASM_PATH}"

_sha=""
if command -v shasum >/dev/null 2>&1; then
  _sha="$(shasum -a 256 "${PROOFOWL_WASM_PATH}" | awk '{print $1}')"
elif command -v sha256sum >/dev/null 2>&1; then
  _sha="$(sha256sum "${PROOFOWL_WASM_PATH}" | awk '{print $1}')"
fi

step "Artifact"
info "path:   ${PROOFOWL_WASM_PATH}"
info "size:   $(wc -c < "${PROOFOWL_WASM_PATH}" | tr -d ' ') bytes"
[ -n "${_sha}" ] && info "sha256: ${_sha}"
info "record this sha256 in your deployment log before deploying."
