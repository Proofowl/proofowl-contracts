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

# require_testnet — hard guardrail: these scripts operate on testnet only.
require_testnet() {
  require_env STELLAR_NETWORK "must be 'testnet'"
  [ "${STELLAR_NETWORK}" = "testnet" ] || \
    die "these scripts only operate on testnet (STELLAR_NETWORK='${STELLAR_NETWORK}')"
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
