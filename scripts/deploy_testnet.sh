#!/usr/bin/env bash
#
# Deploy the ProofOwl registry to the Stellar TESTNET and run its
# constructor in the same transaction.
#
# - Refuses to run unless STELLAR_NETWORK=testnet.
# - Never funds or generates a key. The admin identity must already exist
#   in your Stellar CLI keystore and be funded on testnet.
# - Never prints key material. Only aliases, public addresses, and the
#   resulting contract ID are shown.
# - Prompts for confirmation unless PROOFOWL_ASSUME_YES=1.
#
# Required environment (see .env.example):
#   STELLAR_NETWORK=testnet
#   PROOFOWL_ADMIN_IDENTITY     Stellar CLI identity alias that signs as admin
#   PROOFOWL_ADMIN_ADDRESS      G... address of the admin (constructor arg)
#   PROOFOWL_ATTESTOR_ADDRESS   G... address of the attestor (constructor arg)
# Optional:
#   PROOFOWL_RPC_URL            override RPC (else the CLI's testnet default)
#   PROOFOWL_ASSUME_YES=1       skip the confirmation prompt
#
# Usage:  scripts/build_wasm.sh && scripts/deploy_testnet.sh

set -euo pipefail
_here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
. "${_here}/lib.sh"

step "Deploy ProofOwl registry — Stellar testnet"

require_cmd stellar
require_testnet
require_env PROOFOWL_ADMIN_IDENTITY   "Stellar CLI identity alias that signs as admin"
require_env PROOFOWL_ADMIN_ADDRESS    "G... address of the admin"
require_env PROOFOWL_ATTESTOR_ADDRESS "G... address of the attestor"

[ -f "${PROOFOWL_WASM_PATH}" ] || \
  die "WASM not built at ${PROOFOWL_WASM_PATH} — run scripts/build_wasm.sh first"

_rpc_args=""
if [ -n "${PROOFOWL_RPC_URL:-}" ]; then
  _rpc_args="--rpc-url ${PROOFOWL_RPC_URL}"
fi

info "network:  testnet"
info "wasm:     ${PROOFOWL_WASM_PATH}"
info "admin:    $(safe_addr "${PROOFOWL_ADMIN_ADDRESS}")"
info "attestor: $(safe_addr "${PROOFOWL_ATTESTOR_ADDRESS}")"
info "signer:   keystore alias '${PROOFOWL_ADMIN_IDENTITY}' (no key material shown)"
info "note:     the deploy tx is authorized by this identity; the"
info "          constructor also calls admin.require_auth()."

confirm $'\nProceed with deployment?' deploy

step "stellar contract deploy"
# The constructor arguments follow the bare '--'. There is no separate
# init call. Word-splitting on ${_rpc_args} is intentional.
# shellcheck disable=SC2086
_contract_id="$(
  stellar contract deploy \
    --wasm "${PROOFOWL_WASM_PATH}" \
    --source "${PROOFOWL_ADMIN_IDENTITY}" \
    --network testnet ${_rpc_args} \
    -- \
    --admin "${PROOFOWL_ADMIN_ADDRESS}" \
    --attestor "${PROOFOWL_ATTESTOR_ADDRESS}"
)"

[ -n "${_contract_id}" ] || die "deploy produced no contract id"

step "Deployed"
info "contract id: ${_contract_id}"
info "next steps:"
info "  1. add  PROOFOWL_CONTRACT_ID=${_contract_id}  to your .env"
info "  2. run  scripts/verify_config.sh"
info "  3. run  scripts/smoke_test.sh"
info "  4. record date, commit SHA, wasm sha256, contract id, admin/attestor"
