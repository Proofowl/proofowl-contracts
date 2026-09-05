# ProofOwl attestor integration protocol v2

Status: **normative** for the `proofowl-backend` service, once it
targets a v0.2 contract instance. Supersedes
[`attestor-protocol-v1.md`](./attestor-protocol-v1.md), kept as the
historical record of the v0.1 protocol. **No v0.2 instance has been
deployed to any network as of this document** — see
[`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md). The
`proofowl-backend` repository does **not exist yet** either way.

## What changed from v1

Everything in v1 §0–§7 and §9–§12 (trust boundaries, key custody,
OAuth/challenge proof, the identity-hash derivation, two-party link
submission, PR verification, the allowed-repository policy, canonical
PR hash derivation, attestor-key rotation, unlink assistance, rate
limiting, audit logging) is **unchanged** — re-read those sections
there; they are not duplicated here. Only two things changed, both
downstream of `docs/adr/0004-paginated-attestation-storage.md`:

- **§8 (idempotency and retries)**: no change to the retry logic itself,
  but a backend reconciling its own queue against on-chain state after a
  submission now uses the v0.2 paginated read functions
  (`get_attestation_count` / `get_attestation` /
  `get_attestations_page`), not the removed `get_attestations`.
- **§13 (failure-mode → error mapping)**: four new codes
  (`InvalidPageLimit`, `PageLimitExceeded`, `SequenceOutOfRange`,
  `PageStartOutOfRange`) exist, but **none of them can ever be raised by
  `link_github`, `unlink_github`, or `submit_attestation`** — they only
  apply to the read/TTL-maintenance functions the attestor key does not
  call. A correctly-implemented attestor service submitting attestations
  will never see these four codes from its own writes; they matter only
  if the same service also exposes read/keep-alive functionality (a
  `bump_attestations_ttl_page` sweep, or serving `get_attestations_page`
  to a frontend/indexer) using the same SDK.

## 8. Idempotency and retries (revised for v0.2 reconciliation)

- **Key every attestation job by `pr_hash`.** Unchanged.
- On submit, map results — unchanged from v1 (`Ok(wallet)` → success;
  `DuplicateAttestation` (6) → success-equivalent; `WalletNotLinked`
  (7) → park and retry after `GithubLinked`; `InvalidComplexity` (8) →
  backend bug; `Unauthorized` (3) → check `get_attestor()`; network
  errors → retry with backoff).
- **Reconciling a job against on-chain state** (e.g. "did this
  `pr_hash` actually land, and at what sequence") now uses
  `get_attestation_count(wallet)` plus `get_attestation(wallet, seq)` or
  `get_attestations_page(wallet, start, limit)` to walk the wallet's
  history and match on `pr_hash` — there is no longer a single call
  that returns everything at once. For a wallet with a large history,
  prefer paging from the **end** backward if you expect the job you're
  reconciling to be recent (most attestor writes are appends), though
  the contract does not provide a reverse-order read — you still walk
  forward in pages; there is no on-chain way to fetch "the last N"
  directly. If this becomes a real bottleneck for an active backend, the
  practical fix is not to reconcile via full history walks in the first
  place: track your own job's assigned sequence from the `Ok(wallet)`
  return combined with `AttestationRecorded`'s new `sequence` event
  field (§ below), and target `get_attestation(wallet, that_sequence)`
  directly — O(1), no walk needed.
- Two backend instances racing on the same `pr_hash`: unchanged — at
  most one `Ok`, the other gets `DuplicateAttestation`; both are
  terminal-success for the job.

### New: use `AttestationRecorded`'s `sequence` field to avoid a reconciliation walk entirely

`submit_attestation`'s `Ok(wallet)` return doesn't include the
sequence it was assigned, but the `AttestationRecorded` event emitted
by the same call does (`docs/integration/event-indexer-v2.md`). A
backend that already tails events (as `event-indexer-v1.md` /
`-v2.md` recommend for any component consuming this contract) can
record `(pr_hash) -> sequence` directly from the event stream and
never needs to page through a wallet's history to find a specific job's
result. This is a **recommended pattern**, not a requirement — reconciling
via `get_attestation_count` + paging still works, just costs more for a
large history.

## 9. Attestor-key rotation

Unchanged from v1 — `set_attestor(admin, new_attestor)` is still an
admin action the backend cooperates with but does not perform, the
`AttestorRotated` event is unchanged, and the cutover procedure (bring
the new key online, rotate, retire the old key) is identical.

## 10. Unlink assistance

Unchanged from v1, with one clarification: after unlink, `wallet` keeps
its attestation history and score exactly as before — this was already
true in v1 and remains true in v0.2, now backed by
`AttestationEntry`/`AttestationCount`/`ReputationScore` staying
untouched by `unlink_github` rather than a `Vec` staying untouched.
Verify a link before co-signing an unlink the same way as v1:
`get_wallet_for_github` / `get_github_for_wallet` (both unchanged).

## 11. Rate limiting

Unchanged from v1.

## 12. Audit logging

Unchanged from v1. One addition worth logging if convenient (not
required): the `sequence` a `submit_attestation` call was assigned,
from either the on-chain read immediately after submission or the
`AttestationRecorded` event — useful for correlating an audit-log entry
directly to a specific `get_attestation(wallet, sequence)` lookup
later, without needing to search.

## 13. Failure modes → error mapping (v0.2 additions)

The v1 table (`Unauthorized`, `WalletNotLinked`, `DuplicateAttestation`,
`InvalidComplexity`, `WalletAlreadyLinked`, `GithubAlreadyLinked`,
`LinkNotFound`, `NotInitialized`, missing signature) is unchanged and
still the complete list of what `link_github`, `unlink_github`, and
`submit_attestation` can return. These four are new and apply **only**
to the v0.2 read/TTL functions, never to an attestor-signed write:

| Situation | Contract signal | Backend action |
|---|---|---|
| paginated call's `limit` was `0` | `InvalidPageLimit` (10) | backend bug in a read/keep-alive path; fix before retrying |
| paginated call's `limit` exceeded 50 | `PageLimitExceeded` (11) | backend bug; clamp to `MAX_PAGE_SIZE` before calling |
| `get_attestation`'s `sequence` was out of range | `SequenceOutOfRange` (12) | re-check `get_attestation_count(wallet)` first; the wallet may have fewer attestations than assumed |
| paginated call's `start` was beyond the wallet's count | `PageStartOutOfRange` (13) | re-check `get_attestation_count(wallet)`; `start == count` is valid and means "no more pages," `start > count` means a stale count was used |

## 14. Explicit non-goals of this protocol (unchanged)

Identical to v1 §14 — this protocol does not make the contract trust
GitHub, does not remove the single-trusted-attestor limitation, and
does not provide anonymity for linked identities.
