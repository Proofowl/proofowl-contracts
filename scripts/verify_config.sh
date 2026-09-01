#!/usr/bin/env bash
#
# Read the deployed contract's stored admin and attestor and compare them
# to the addresses you expect. Read-only: this only simulates/reads, it
# submits no state. Exits non-zero on any mismatch.
#
# Required environment (see .env.example):
#   STELLAR_NETWORK=testnet
#   PROOFOWL_CONTRACT_ID        C... address of the deployed instance
#   PROOFOWL_ADMIN_ADDRESS      G... address you expect as admin
#   PROOFOWL_ATTESTOR_ADDRESS   G... address you expect as attestor
# Optional:
#   PROOFOWL_VERIFY_IDENTITY    keystore alias used as the read source
#                               (falls back to PROOFOWL_ADMIN_IDENTITY)
#   PROOFOWL_RPC_URL            override RPC
#
# Usage:  scripts/verify_config.sh

set -euo pipefail
_here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
. "${_here}/lib.sh"

step "Verify deployed configuration — Stellar testnet"

require_cmd stellar
require_testnet
require_env PROOFOWL_CONTRACT_ID      "C... address of the deployed instance"
require_env PROOFOWL_ADMIN_ADDRESS    "G... address you expect as admin"
require_env PROOFOWL_ATTESTOR_ADDRESS "G... address you expect as attestor"

_source="${PROOFOWL_VERIFY_IDENTITY:-${PROOFOWL_ADMIN_IDENTITY:-}}"
[ -n "${_source}" ] || \
  die "set PROOFOWL_VERIFY_IDENTITY (or PROOFOWL_ADMIN_IDENTITY) to a keystore alias to read from"

_rpc_args=""
[ -n "${PROOFOWL_RPC_URL:-}" ] && _rpc_args="--rpc-url ${PROOFOWL_RPC_URL}"

read_fn() {
  # shellcheck disable=SC2086
  stellar contract invoke \
    --id "${PROOFOWL_CONTRACT_ID}" \
    --source "${_source}" \
    --network testnet ${_rpc_args} \
    --send=no \
    -- "$1" \
    | tr -d '"[:space:]'
}

info "contract: ${PROOFOWL_CONTRACT_ID}"
_got_admin="$(read_fn get_admin)"
_got_attestor="$(read_fn get_attestor)"

_fail=0
if [ "${_got_admin}" = "${PROOFOWL_ADMIN_ADDRESS}" ]; then
  info "admin     OK    ${_got_admin}"
else
  printf '  admin     MISMATCH   on-chain=%s  expected=%s\n' \
    "${_got_admin:-<none>}" "${PROOFOWL_ADMIN_ADDRESS}" >&2
  _fail=1
fi

if [ "${_got_attestor}" = "${PROOFOWL_ATTESTOR_ADDRESS}" ]; then
  info "attestor  OK    ${_got_attestor}"
else
  printf '  attestor  MISMATCH   on-chain=%s  expected=%s\n' \
    "${_got_attestor:-<none>}" "${PROOFOWL_ATTESTOR_ADDRESS}" >&2
  _fail=1
fi

[ "${_fail}" -eq 0 ] || die "on-chain configuration does not match expected values"
step "configuration verified"
