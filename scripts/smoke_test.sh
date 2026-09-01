#!/usr/bin/env bash
#
# Harmless end-to-end smoke test against a DEPLOYED TESTNET instance.
#
# It exercises one full path with a throwaway identity and obviously-fake
# hashes:
#   link_github (wallet + attestor co-sign)
#     -> submit_attestation (attestor)
#       -> read get_attestations / get_reputation_score
#         -> unlink_github (wallet + attestor co-sign)
#
# What it writes to the testnet instance:
#   - one wallet <-> github link, then removes it again;
#   - one attestation for a unique fake pr_hash. That pr_hash's
#     de-duplication marker is permanent by design (a merged PR stays
#     spent forever) — this is expected and harmless on testnet.
#
# It NEVER creates or funds an identity. PROOFOWL_SMOKE_WALLET_IDENTITY
# must already exist in your keystore and be funded on testnet.
#
# Required environment (see .env.example):
#   STELLAR_NETWORK=testnet
#   PROOFOWL_CONTRACT_ID             C... address of the deployed instance
#   PROOFOWL_ATTESTOR_IDENTITY       keystore alias that signs as attestor
#   PROOFOWL_ATTESTOR_ADDRESS        G... address of the attestor
#   PROOFOWL_SMOKE_WALLET_IDENTITY   disposable funded testnet identity
# Optional:
#   PROOFOWL_RPC_URL                 override RPC
#   PROOFOWL_ASSUME_YES=1            skip the confirmation prompt
#
# Usage:  scripts/smoke_test.sh

set -euo pipefail
_here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
. "${_here}/lib.sh"

step "Testnet smoke test — writes disposable state"

require_cmd stellar
require_testnet
require_env PROOFOWL_CONTRACT_ID           "C... address of the deployed instance"
require_env PROOFOWL_ATTESTOR_IDENTITY     "keystore alias that signs as attestor"
require_env PROOFOWL_ATTESTOR_ADDRESS      "G... address of the attestor"
require_env PROOFOWL_SMOKE_WALLET_IDENTITY "disposable funded testnet identity"

sha256_hex() {
  if command -v shasum >/dev/null 2>&1; then
    printf '%s' "$1" | shasum -a 256 | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    printf '%s' "$1" | sha256sum | awk '{print $1}'
  elif command -v openssl >/dev/null 2>&1; then
    printf '%s' "$1" | openssl dgst -sha256 | awk '{print $NF}'
  else
    die "need one of: shasum, sha256sum, openssl"
  fi
}

_rpc_args=""
[ -n "${PROOFOWL_RPC_URL:-}" ] && _rpc_args="--rpc-url ${PROOFOWL_RPC_URL}"

# Resolve the throwaway wallet's public address from its alias.
_wallet_addr="$(stellar keys address "${PROOFOWL_SMOKE_WALLET_IDENTITY}")"
[ -n "${_wallet_addr}" ] || die "could not resolve address for ${PROOFOWL_SMOKE_WALLET_IDENTITY}"

# Unique per run so repeated runs never collide on an existing link or a
# spent pr_hash.
_nonce="$(date +%s)"
_gh_hash="$(sha256_hex "proofowl-smoke:github:${_wallet_addr}:${_nonce}")"
_pr_hash="$(sha256_hex "github.com/proofowl/smoke-test/pull/${_nonce}")"

info "contract:      ${PROOFOWL_CONTRACT_ID}"
info "smoke wallet:  $(safe_addr "${_wallet_addr}")  (alias ${PROOFOWL_SMOKE_WALLET_IDENTITY})"
info "attestor:      $(safe_addr "${PROOFOWL_ATTESTOR_ADDRESS}")  (alias ${PROOFOWL_ATTESTOR_IDENTITY})"
info "fake gh hash:  ${_gh_hash}"
info "fake pr hash:  ${_pr_hash}  (its dedup marker will persist — expected)"

confirm $'\nRun the smoke test against this instance?' smoke

# stellar contract invoke: --source is the tx source + a signer;
# --sign-with-key adds another auth-entry signer. Flag names can vary by
# CLI version — adjust for yours if needed.
invoke() {
  # shellcheck disable=SC2086
  stellar contract invoke \
    --id "${PROOFOWL_CONTRACT_ID}" \
    --network testnet ${_rpc_args} \
    "$@"
}

step "1/5  link_github  (wallet + attestor)"
invoke --source "${PROOFOWL_SMOKE_WALLET_IDENTITY}" \
       --sign-with-key "${PROOFOWL_ATTESTOR_IDENTITY}" \
       -- link_github \
       --wallet "${_wallet_addr}" \
       --attestor "${PROOFOWL_ATTESTOR_ADDRESS}" \
       --github_id_hash "${_gh_hash}"

step "2/5  submit_attestation  (attestor)"
invoke --source "${PROOFOWL_ATTESTOR_IDENTITY}" \
       -- submit_attestation \
       --attestor "${PROOFOWL_ATTESTOR_ADDRESS}" \
       --github_id_hash "${_gh_hash}" \
       --repo "proofowl/smoke-test" \
       --pr_number 1 \
       --issue_id 1 \
       --complexity 100 \
       --pr_hash "${_pr_hash}"

step "3/5  get_attestations"
invoke --source "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no \
       -- get_attestations --wallet "${_wallet_addr}"

step "4/5  get_reputation_score  (expect 100)"
invoke --source "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no \
       -- get_reputation_score --wallet "${_wallet_addr}"

step "5/5  unlink_github  (wallet + attestor) — clean up the link"
invoke --source "${PROOFOWL_SMOKE_WALLET_IDENTITY}" \
       --sign-with-key "${PROOFOWL_ATTESTOR_IDENTITY}" \
       -- unlink_github \
       --wallet "${_wallet_addr}" \
       --attestor "${PROOFOWL_ATTESTOR_ADDRESS}" \
       --github_id_hash "${_gh_hash}"

step "smoke test complete"
info "the link was removed; the attestation and its pr_hash marker remain"
info "on the instance by design."
