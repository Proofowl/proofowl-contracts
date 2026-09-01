#!/usr/bin/env bash
#
# Regenerate sdk/typescript/src/generated/index.ts from the committed
# contract WASM, using the official `stellar contract bindings
# typescript` command. The output is written verbatim under a fixed
# DO-NOT-EDIT banner so that CI can detect drift with `git diff`.
#
# Requires: stellar CLI on PATH, and a Rust toolchain to build the WASM
# if it is not already present.

set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
sdk_root="$(cd "${here}/.." && pwd)"
repo_root="$(cd "${sdk_root}/../.." && pwd)"

wasm="${repo_root}/target/wasm32v1-none/release/proofowl_contracts.wasm"
out_dir="${sdk_root}/src/generated"
out_file="${out_dir}/index.ts"

command -v stellar >/dev/null 2>&1 || {
  echo "error: the 'stellar' CLI is required (see docs/operations/testnet-deployment.md)" >&2
  exit 1
}

if [ ! -f "${wasm}" ]; then
  echo "building contract WASM..."
  ( cd "${repo_root}" && cargo build --target wasm32v1-none --release )
fi

tmp="$(mktemp -d)"
trap 'rm -rf "${tmp}"' EXIT

stellar contract bindings typescript \
  --wasm "${wasm}" \
  --output-dir "${tmp}/gen" \
  --overwrite >/dev/null

mkdir -p "${out_dir}"
{
  echo "// GENERATED FILE — DO NOT EDIT BY HAND."
  echo "// Source: \`stellar contract bindings typescript\` run against the committed"
  echo "// contract WASM (target/wasm32v1-none/release/proofowl_contracts.wasm)."
  echo "// Regenerate with:  npm run generate   (or:  make sdk-generate)"
  echo "// CI fails if this file drifts from a fresh regeneration."
  echo ""
  cat "${tmp}/gen/src/index.ts"
} > "${out_file}"

echo "wrote ${out_file}"
