#!/usr/bin/env bash
#
# Structural regression guard for the v0.2 paginated-storage redesign
# (docs/adr/0004-paginated-attestation-storage.md).
#
# Fails if `src/lib.rs` reintroduces v0.1's unbounded per-wallet
# attestation storage or drops the bounded page-size guard that fixed
# it (docs/security/resource-profile-v1.md's measured 286-attestation
# ceiling). This is a fast, deterministic, fully offline STATIC check —
# it does not replace the runtime proof in `tests/state_machine.rs`,
# `tests/security_matrix.rs`, or `tests/resource_profile.rs`; it exists
# so a refactor that keeps every existing test passing by accident (for
# example, one that restores an unbounded scan a small test suite never
# exercises at a large N) is still caught immediately, at the source
# level, before those tests are even compiled.
#
# Run via `make check-bounded-storage` (part of `make check`) or
# directly: scripts/check_bounded_storage.sh
#
# Portable to bash 3.2 (the macOS system bash), no network access.

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
lib_rs="${repo_root}/src/lib.rs"

[ -f "$lib_rs" ] || {
  echo "error: $lib_rs not found" >&2
  exit 1
}

fail=0

ok()   { printf 'ok: %s\n' "$1"; }
bad()  { printf 'FAIL: %s\n' "$1" >&2; fail=1; }

# --- 1. The v0.1 unbounded storage key and functions must never return.
if grep -Eq '^\s*Attestations\(Address\)' "$lib_rs"; then
  bad "the v0.1 unbounded 'Attestations(Address)' storage key has returned"
else
  ok "no v0.1 unbounded 'Attestations(Address)' storage key"
fi

if grep -Eq 'pub fn get_attestations\(' "$lib_rs"; then
  bad "the v0.1 unbounded 'get_attestations(' getter has returned"
else
  ok "no v0.1 unbounded 'get_attestations(' getter"
fi

if grep -Eq 'pub fn bump_wallet_ttl\(' "$lib_rs"; then
  bad "the v0.1 unbounded 'bump_wallet_ttl(' function has returned"
else
  ok "no v0.1 unbounded 'bump_wallet_ttl(' function"
fi

# --- 2. The bounded v0.2 replacements must be present.
for fn in get_attestation_count get_attestation get_attestations_page \
          bump_wallet_core_ttl bump_attestations_ttl_page; do
  if grep -Eq "pub fn ${fn}\\(" "$lib_rs"; then
    ok "bounded v0.2 function '${fn}(' is present"
  else
    bad "bounded v0.2 function '${fn}(' is missing"
  fi
done

# --- 3. A page-size ceiling must exist and stay a small, sane number.
# 500 is a generous sanity ceiling, not the current value (50) — this
# check exists to catch the constant being deleted or silently raised
# to something that stops being a meaningful bound, not to pin the
# exact number (docs/adr/0004-paginated-attestation-storage.md documents
# why 50 was chosen; re-measure via `make resource-profile` /
# docs/security/resource-profile-v2.md before ever raising it).
max_page_line="$(grep -E '^const MAX_PAGE_SIZE: u32 = [0-9]+;' "$lib_rs" || true)"
if [ -z "$max_page_line" ]; then
  bad "MAX_PAGE_SIZE constant not found in src/lib.rs"
else
  max_page_value="$(printf '%s\n' "$max_page_line" | sed -E 's/^const MAX_PAGE_SIZE: u32 = ([0-9]+);$/\1/')"
  if [ "$max_page_value" -lt 1 ] || [ "$max_page_value" -gt 500 ]; then
    bad "MAX_PAGE_SIZE is ${max_page_value}, outside the sane [1, 500] range"
  else
    ok "MAX_PAGE_SIZE = ${max_page_value} (within the sane [1, 500] range)"
  fi
fi

# --- 4. Both paginated functions must actually call the shared limit
# check -- guards against a new paginated function forgetting the bound,
# or the existing ones having it silently removed.
if grep -Eq 'fn check_page_limit\(' "$lib_rs"; then
  ok "check_page_limit helper is present"
else
  bad "check_page_limit helper is missing"
fi
limit_check_calls="$(grep -c 'Self::check_page_limit(limit)?;' "$lib_rs" || true)"
if [ "${limit_check_calls:-0}" -ge 2 ]; then
  ok "check_page_limit is called by at least 2 functions (found ${limit_check_calls})"
else
  bad "check_page_limit is called by fewer than 2 functions (found ${limit_check_calls:-0}) -- \
get_attestations_page and bump_attestations_ttl_page must both validate limit"
fi

if [ "$fail" -ne 0 ]; then
  echo >&2
  echo "One or more bounded-storage invariants failed." >&2
  echo "See docs/adr/0004-paginated-attestation-storage.md." >&2
  exit 1
fi

echo
echo "OK — bounded attestation storage guarantees intact."
