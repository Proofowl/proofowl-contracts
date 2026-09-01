# shellcheck shell=bash
#
# Shared helpers for the ProofOwl testnet operations scripts.
#
# This file ONLY defines functions and variables. It performs no action
# when sourced, and it refuses to be executed directly. Nothing here
# deploys, funds, generates keys, pushes, or prints secret material.
#
# Portable to bash 3.2 (the macOS system bash).

# Refuse direct execution: this is a library.
if [ "${BASH_SOURCE[0]}" = "${0}" ]; then
  echo "scripts/lib.sh is a library; source it from another script, do not run it." >&2
  exit 1
fi

set -euo pipefail

# Repository root, resolved from this file's location.
PROOFOWL_REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROOFOWL_WASM_TARGET="wasm32v1-none"
PROOFOWL_WASM_PATH="${PROOFOWL_REPO_ROOT}/target/${PROOFOWL_WASM_TARGET}/release/proofowl_contracts.wasm"

# --- network constants: hard-coded so they cannot be misconfigured ------
# The testnet passphrase is a well-known public constant. The mainnet
# passphrase is listed only so the scripts can positively refuse it.
PROOFOWL_TESTNET_PASSPHRASE='Test SDF Network ; September 2015'
PROOFOWL_MAINNET_PASSPHRASE='Public Global Stellar Network ; September 2015'
PROOFOWL_DEFAULT_TESTNET_RPC='https://soroban-testnet.stellar.org'

# Filled in by require_verified_testnet; scripts pass these to the CLI as
# --rpc-url / --network-passphrase instead of a named network config.
PROOFOWL_VERIFIED_RPC=''
PROOFOWL_VERIFIED_PASSPHRASE=''

die()  { printf 'error: %s\n' "$*" >&2; exit 1; }
info() { printf '  %s\n' "$*"; }
step() { printf '\n==> %s\n' "$*"; }

# require_cmd <name> — abort if a command is not on PATH.
require_cmd() {
  command -v "$1" >/dev/null 2>&1 || die "required command not found: $1"
}

# require_env <VAR> [description] — abort if an env var is empty/unset.
require_env() {
  eval "_pf_val=\${$1:-}"
  if [ -z "${_pf_val}" ]; then
    if [ -n "${2:-}" ]; then
      die "environment variable $1 is not set ($2)"
    fi
    die "environment variable $1 is not set"
  fi
  unset _pf_val
}

# require_testnet — cheap first gate: the operator's declared intent.
require_testnet() {
  require_env STELLAR_NETWORK "must be 'testnet'"
  [ "${STELLAR_NETWORK}" = "testnet" ] || \
    die "these scripts only operate on testnet (STELLAR_NETWORK='${STELLAR_NETWORK}')"
}

# proofowl_rpc_url — the RPC these scripts will actually use: the
# operator's override, or the well-known public testnet RPC.
proofowl_rpc_url() {
  printf '%s' "${PROOFOWL_RPC_URL:-$PROOFOWL_DEFAULT_TESTNET_RPC}"
}

# require_verified_testnet — the authoritative guardrail. It does not
# trust STELLAR_NETWORK or the RPC hostname: it asks the RPC itself which
# network it serves (getNetwork) and refuses to continue unless the
# passphrase is exactly the Stellar testnet passphrase. It fails closed
# on any error (unreachable RPC, missing/blank passphrase, mainnet
# passphrase, anything unexpected). Sets PROOFOWL_VERIFIED_RPC and
# PROOFOWL_VERIFIED_PASSPHRASE for callers to hand to the CLI.
require_verified_testnet() {
  require_testnet
  require_cmd curl

  local rpc body pass
  rpc="$(proofowl_rpc_url)"

  case "$rpc" in
    https://*) : ;;
    *) die "RPC URL must be https:// — refusing '${rpc}'" ;;
  esac

  body="$(curl -fsS --max-time 20 -X POST "$rpc" \
            -H 'Content-Type: application/json' \
            -d '{"jsonrpc":"2.0","id":1,"method":"getNetwork"}' 2>/dev/null)" \
    || die "cannot reach RPC to verify the network: ${rpc}"

  # Extract "passphrase":"..." without assuming jq is present.
  pass="$(printf '%s' "$body" | sed -n 's/.*"passphrase"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p')"

  [ -n "$pass" ] || die "RPC ${rpc} returned no network passphrase (response: ${body})"

  if [ "$pass" = "$PROOFOWL_MAINNET_PASSPHRASE" ]; then
    die "REFUSING: RPC ${rpc} serves Stellar MAINNET. This tooling is testnet-only."
  fi
  if [ "$pass" != "$PROOFOWL_TESTNET_PASSPHRASE" ]; then
    die "RPC ${rpc} is not Stellar testnet — passphrase is: ${pass}"
  fi

  # If the operator also pinned a passphrase in the environment, it must
  # agree with what the RPC reports.
  if [ -n "${STELLAR_NETWORK_PASSPHRASE:-}" ] && \
     [ "${STELLAR_NETWORK_PASSPHRASE}" != "$PROOFOWL_TESTNET_PASSPHRASE" ]; then
    die "STELLAR_NETWORK_PASSPHRASE is set to a non-testnet value"
  fi

  PROOFOWL_VERIFIED_RPC="$rpc"
  PROOFOWL_VERIFIED_PASSPHRASE="$pass"

  info "network verified via getNetwork:"
  info "  rpc:        ${rpc}"
  info "  passphrase: ${pass}"
}

# safe_addr <value> — echo a public address, but refuse to echo anything
# that looks like a Stellar secret seed (starts with 'S'), so a
# misconfigured env cannot leak a key into logs.
safe_addr() {
  case "$1" in
    S*[A-Z2-7]) die "refusing to print a value that looks like a secret key" ;;
    *) printf '%s' "$1" ;;
  esac
}

# confirm <prompt> <expected-word> — interactive gate. Skipped only when
# PROOFOWL_ASSUME_YES=1 is set explicitly by the operator.
confirm() {
  if [ "${PROOFOWL_ASSUME_YES:-0}" = "1" ]; then
    info "PROOFOWL_ASSUME_YES=1 — skipping confirmation prompt"
    return 0
  fi
  printf '%s type "%s" to continue: ' "$1" "$2"
  read -r _pf_reply || true
  [ "${_pf_reply:-}" = "$2" ] || die "aborted by operator"
  unset _pf_reply
}
