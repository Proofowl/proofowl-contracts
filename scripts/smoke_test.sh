#!/usr/bin/env bash
#
# End-to-end smoke test against a DEPLOYED TESTNET instance.
#
# Exercises the full contributor-reputation lifecycle with throwaway
# testnet identities and deterministic dummy inputs:
#
#   1. two-party link_github (wallet + attestor)
#   2. submit_attestation (attestor)
#   3. reads: get_attestations / get_reputation_score / reverse lookups
#   4. invalid complexity is rejected            (expected failure)
#   5. duplicate pr_hash is rejected             (expected failure)
#   6. two-party unlink_github (wallet + attestor)
#   7. link is gone, but recorded reputation still attached to the wallet
#
# Guardrails: the RPC is queried with getNetwork and must report the
# Stellar testnet passphrase (mainnet is positively refused). Never
# creates or funds an identity. Never prints key material.
#
# Two-party calls: link_github / unlink_github each require TWO
# independent require_auth() addresses. This uses the Stellar CLI's
# supported multi-signature invoke: --source signs the transaction
# envelope and the source address's root auth entry; --auto-sign then
# signs every remaining non-root Soroban auth entry by matching the
# entry's address to an identity in the local keystore (here, the
# attestor). Do NOT add --sign-with-key for this: it replaces the
# envelope signer and yields TxBadAuth. See
# docs/operations/testnet-deployment.md section 7.
#
# Required environment (see .env.example):
#   STELLAR_NETWORK=testnet
#   PROOFOWL_CONTRACT_ID             C... address of the deployed instance
#   PROOFOWL_ATTESTOR_IDENTITY       keystore alias that signs as attestor
#   PROOFOWL_ATTESTOR_ADDRESS        G... address of the attestor
#   PROOFOWL_SMOKE_WALLET_IDENTITY   disposable funded testnet identity
# Optional:
#   PROOFOWL_RPC_URL                 RPC override (default: public testnet RPC)
#   PROOFOWL_SMOKE_TAG              deterministic-input namespace
#                                   (default: phase2-alpha-1)
#   PROOFOWL_ASSUME_YES=1            skip the confirmation prompt
#
# Usage:  scripts/smoke_test.sh

set -euo pipefail
_here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# shellcheck source=scripts/lib.sh
. "${_here}/lib.sh"

step "Testnet end-to-end smoke test"

require_cmd stellar
require_verified_testnet
require_env PROOFOWL_CONTRACT_ID           "C... address of the deployed instance"
require_env PROOFOWL_ATTESTOR_IDENTITY     "keystore alias that signs as attestor"
require_env PROOFOWL_ATTESTOR_ADDRESS      "G... address of the attestor"
require_env PROOFOWL_SMOKE_WALLET_IDENTITY "disposable funded testnet identity"

CID="${PROOFOWL_CONTRACT_ID}"
RPC="${PROOFOWL_VERIFIED_RPC}"
PASS="${PROOFOWL_VERIFIED_PASSPHRASE}"
TAG="${PROOFOWL_SMOKE_TAG:-phase2-alpha-1}"

sha256_hex() {
  if command -v shasum >/dev/null 2>&1; then
    printf '%s' "$1" | shasum -a 256 | awk '{print $1}'
  elif command -v sha256sum >/dev/null 2>&1; then
    printf '%s' "$1" | sha256sum | awk '{print $1}'
  else
    printf '%s' "$1" | openssl dgst -sha256 | awk '{print $NF}'
  fi
}

WALLET_ADDR="$(stellar keys address "${PROOFOWL_SMOKE_WALLET_IDENTITY}")"
[ -n "${WALLET_ADDR}" ] || die "cannot resolve address for ${PROOFOWL_SMOKE_WALLET_IDENTITY}"

# Deterministic dummy inputs, namespaced by TAG (documented, no secrets).
GH_HASH="$(sha256_hex "proofowl:testnet:${TAG}:github-identity")"
PR_HASH="$(sha256_hex "github.com/proofowl/testnet-smoke/pull/1|${TAG}")"
PR_HASH_BAD="$(sha256_hex "github.com/proofowl/testnet-smoke/pull/2|${TAG}")"
REPO="proofowl/testnet-smoke"

info "contract:     ${CID}"
info "wallet:       $(safe_addr "${WALLET_ADDR}")  (alias ${PROOFOWL_SMOKE_WALLET_IDENTITY})"
info "attestor:     $(safe_addr "${PROOFOWL_ATTESTOR_ADDRESS}")  (alias ${PROOFOWL_ATTESTOR_IDENTITY})"
info "input tag:    ${TAG}"
info "github_id_hash: ${GH_HASH}"
info "pr_hash:        ${PR_HASH}"
info "pr_hash (bad):  ${PR_HASH_BAD}"

confirm $'\nRun the end-to-end smoke test against this instance?' smoke

_errf="$(mktemp)"
trap 'rm -f "${_errf}"' EXIT

# invoke_1 <source-ident> -- fn args...      (single-signer write / read)
invoke_1() {
  local src="$1"; shift
  stellar contract invoke --id "${CID}" \
    --rpc-url "${RPC}" --network-passphrase "${PASS}" \
    --source "${src}" "$@"
}

# invoke_2 <source-ident> -- fn args...   (two-party write)
# --source signs the envelope + root auth entry; --auto-sign signs the
# other require_auth address's non-root entry from the keystore. The
# co-signer identity must simply be present in the keystore (it is
# resolved by the entry's address, not passed here).
invoke_2() {
  local src="$1"; shift
  stellar contract invoke --id "${CID}" \
    --rpc-url "${RPC}" --network-passphrase "${PASS}" \
    --source "${src}" --auto-sign "$@"
}

# tx_hash_from_stderr — pull the 64-hex transaction hash out of CLI logs.
tx_hash_from_stderr() {
  grep -oE '[0-9a-f]{64}' "${_errf}" | head -1
}

evidence() { printf 'EVIDENCE %s\n' "$*"; }

# ---------------------------------------------------------------------------
step "1/7  link_github  (wallet + attestor, two-party)"
out="$(invoke_2 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" \
        -- link_github \
        --wallet "${WALLET_ADDR}" \
        --attestor "${PROOFOWL_ATTESTOR_ADDRESS}" \
        --github_id_hash "${GH_HASH}" 2>"${_errf}")" || { cat "${_errf}" >&2; die "link_github failed"; }
evidence "step=1 name=link_github status=ok tx=$(tx_hash_from_stderr)"

# ---------------------------------------------------------------------------
step "2/7  submit_attestation  (attestor, complexity=100)"
out="$(invoke_1 "${PROOFOWL_ATTESTOR_IDENTITY}" \
        -- submit_attestation \
        --attestor "${PROOFOWL_ATTESTOR_ADDRESS}" \
        --github_id_hash "${GH_HASH}" \
        --repo "${REPO}" \
        --pr_number 1 \
        --issue_id 1 \
        --complexity 100 \
        --pr_hash "${PR_HASH}" 2>"${_errf}")" || { cat "${_errf}" >&2; die "submit_attestation failed"; }
evidence "step=2 name=submit_attestation status=ok tx=$(tx_hash_from_stderr) returned=${out}"

# ---------------------------------------------------------------------------
step "3/7  reads  (get_attestations / get_reputation_score / reverse lookups)"
atts="$(invoke_1 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no -- get_attestations --wallet "${WALLET_ADDR}" 2>/dev/null)"
score="$(invoke_1 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no -- get_reputation_score --wallet "${WALLET_ADDR}" 2>/dev/null | tr -d '"[:space:]')"
w4g="$(invoke_1 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no -- get_wallet_for_github --github_id_hash "${GH_HASH}" 2>/dev/null | tr -d '"[:space:]')"
g4w="$(invoke_1 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no -- get_github_for_wallet --wallet "${WALLET_ADDR}" 2>/dev/null | tr -d '"[:space:]')"
info "get_attestations:        ${atts}"
info "get_reputation_score:    ${score}   (expect 100)"
info "get_wallet_for_github:   ${w4g}   (expect ${WALLET_ADDR})"
info "get_github_for_wallet:   ${g4w}   (expect ${GH_HASH})"
[ "${score}" = "100" ]              || die "expected score 100, got '${score}'"
[ "${w4g}" = "${WALLET_ADDR}" ]     || die "reverse lookup mismatch: '${w4g}'"
[ "${g4w}" = "${GH_HASH}" ]         || die "forward hash lookup mismatch: '${g4w}'"
case "${atts}" in *"${PR_HASH}"*) : ;; *) die "attestation list missing pr_hash" ;; esac
evidence "step=3 name=reads status=ok score=${score} wallet_for_github=${w4g} github_for_wallet=${g4w}"

# ---------------------------------------------------------------------------
step "4/7  invalid complexity is rejected  (expected failure, complexity=175)"
set +e
inv_out="$(invoke_1 "${PROOFOWL_ATTESTOR_IDENTITY}" \
            -- submit_attestation \
            --attestor "${PROOFOWL_ATTESTOR_ADDRESS}" \
            --github_id_hash "${GH_HASH}" \
            --repo "${REPO}" \
            --pr_number 2 \
            --issue_id 2 \
            --complexity 175 \
            --pr_hash "${PR_HASH_BAD}" 2>&1)"
inv_rc=$?
set -e
info "exit=${inv_rc}"
printf '%s\n' "${inv_out}" | sed 's/^/    /'
[ "${inv_rc}" -ne 0 ] || die "invalid complexity was NOT rejected"
case "${inv_out}" in
  *"#8"*|*InvalidComplexity*) : ;;
  *) die "rejected, but not with the InvalidComplexity (#8) error" ;;
esac
evidence "step=4 name=invalid_complexity status=rejected_as_expected error=InvalidComplexity(#8)"

# ---------------------------------------------------------------------------
step "5/7  duplicate pr_hash is rejected  (expected failure, reuse pr_hash)"
set +e
dup_out="$(invoke_1 "${PROOFOWL_ATTESTOR_IDENTITY}" \
            -- submit_attestation \
            --attestor "${PROOFOWL_ATTESTOR_ADDRESS}" \
            --github_id_hash "${GH_HASH}" \
            --repo "${REPO}" \
            --pr_number 1 \
            --issue_id 9 \
            --complexity 150 \
            --pr_hash "${PR_HASH}" 2>&1)"
dup_rc=$?
set -e
info "exit=${dup_rc}"
printf '%s\n' "${dup_out}" | sed 's/^/    /'
[ "${dup_rc}" -ne 0 ] || die "duplicate pr_hash was NOT rejected"
case "${dup_out}" in
  *"#6"*|*DuplicateAttestation*) : ;;
  *) die "rejected, but not with the DuplicateAttestation (#6) error" ;;
esac
evidence "step=5 name=duplicate_pr status=rejected_as_expected error=DuplicateAttestation(#6)"

# ---------------------------------------------------------------------------
step "6/7  unlink_github  (wallet + attestor, two-party)"
out="$(invoke_2 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" \
        -- unlink_github \
        --wallet "${WALLET_ADDR}" \
        --attestor "${PROOFOWL_ATTESTOR_ADDRESS}" \
        --github_id_hash "${GH_HASH}" 2>"${_errf}")" || { cat "${_errf}" >&2; die "unlink_github failed"; }
evidence "step=6 name=unlink_github status=ok tx=$(tx_hash_from_stderr)"

# ---------------------------------------------------------------------------
step "7/7  link removed, reputation retained"
w4g2="$(invoke_1 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no -- get_wallet_for_github --github_id_hash "${GH_HASH}" 2>/dev/null | tr -d '"[:space:]')"
g4w2="$(invoke_1 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no -- get_github_for_wallet --wallet "${WALLET_ADDR}" 2>/dev/null | tr -d '"[:space:]')"
score2="$(invoke_1 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no -- get_reputation_score --wallet "${WALLET_ADDR}" 2>/dev/null | tr -d '"[:space:]')"
atts2="$(invoke_1 "${PROOFOWL_SMOKE_WALLET_IDENTITY}" --send=no -- get_attestations --wallet "${WALLET_ADDR}" 2>/dev/null)"
info "get_wallet_for_github:  ${w4g2}   (expect null / None)"
info "get_github_for_wallet:  ${g4w2}   (expect null / None)"
info "get_reputation_score:   ${score2}   (expect 100 — retained)"
case "${w4g2}" in ""|null|None|"()") : ;; *) die "link still present (wallet_for_github=${w4g2})" ;; esac
case "${g4w2}" in ""|null|None|"()") : ;; *) die "link still present (github_for_wallet=${g4w2})" ;; esac
[ "${score2}" = "100" ] || die "reputation not retained after unlink (got '${score2}')"
case "${atts2}" in *"${PR_HASH}"*) : ;; *) die "attestation history lost after unlink" ;; esac
evidence "step=7 name=post_unlink status=ok link_removed=true score_retained=${score2}"

step "smoke test complete — all 7 steps passed"
info "expected failures (steps 4 and 5) were correctly rejected."
info "the wallet<->github link was removed; the attestation and its"
info "pr_hash dedup marker remain on the instance by design."
