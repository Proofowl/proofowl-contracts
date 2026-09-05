# ADR 0004: Paginated per-attestation storage (v0.2)

## Status

Accepted. Supersedes the single-`Vec<Attestation>`-per-wallet storage
design from v0.1 for any deployment beyond the disposable testnet
alpha. Does not change ADR 0001 or ADR 0002's guarantees — see
"Consequences" below.

## Context

v0.1 stored every wallet's full attestation history in one persistent
entry: `Attestations(wallet) -> Vec<Attestation>`. `submit_attestation`
deserialized, appended to, and re-serialized the whole vector on every
call; `get_attestations`, `get_reputation_score`, and `bump_wallet_ttl`
each loaded and iterated the whole thing.

Phase 4's adversarial and resource-testing pass
(`docs/security/resource-profile-v1.md`) measured exactly where this
breaks: **286 attestations succeed for one wallet; the 287th
`submit_attestation` call fails outright**, because the single entry
exceeds Soroban's 65,536-byte per-contract-data-entry ceiling. Past
that point `submit_attestation`, `get_attestations`,
`get_reputation_score`, and `bump_wallet_ttl` all fail for that wallet,
**permanently**, with no admin or attestor override capable of
recovering it — there is no function that can split, migrate, or
truncate an existing wallet's history.

This is not a cost problem (CPU, memory, and write-byte costs were all
comfortably under mainnet limits even at 200 attestations); it is a
structural ceiling that a prolific, long-tenured contributor —
precisely the user this registry exists to reward — can reach in normal
operation, not just adversarially. `docs/security/resource-profile-v1.md`
§4 concluded this "requires an indexed/paginated storage redesign
before mainnet or any production scope without an enforced per-wallet
attestation cap." This ADR is that redesign.

## Decision

Replace the single vector with one persistent entry per attestation,
plus two small, fixed-size counters:

```
DataKey::AttestationEntry(Address, u32)  // one Attestation, keyed by (wallet, sequence)
DataKey::AttestationCount(Address)       // u32: how many attestations this wallet has
DataKey::ReputationScore(Address)        // u32: running total, updated atomically
```

`WalletLink`, `GithubLink`, and `SeenPr` are unchanged — this ADR only
addresses the one storage shape that had a ceiling.

### Sequence numbering: zero-based

`AttestationCount(wallet)` is both "how many attestations exist" and
"the next sequence number to use." The first attestation for a wallet
is stored at `AttestationEntry(wallet, 0)`; the Nth (1-indexed count) is
stored at `AttestationEntry(wallet, N-1)`. `submit_attestation` reads
the current count as `seq`, writes the new entry at that `seq`, then
increments the count. This is the same indexing convention as a
zero-based array/`Vec`, chosen specifically so "count" and "next free
slot" are the same number with no off-by-one translation anywhere in
the contract or in client code that mirrors `Vec` semantics
(`get_attestations_page` returns entries in the same oldest-first order
`get_attestations` used to, just addressed by explicit index instead of
implicit vector position).

### Running score, not re-summed

`ReputationScore(wallet)` is updated once, atomically, in the same
`submit_attestation` call that writes the new entry:
`score = score.saturating_add(points)`. `get_reputation_score` becomes
a single O(1) read. This removes the second O(history-length) hot path
v0.1 had (`get_reputation_score` used to re-fold the entire vector on
every call).

### Bounded reads: `get_attestation`, `get_attestations_page`

- `get_attestation(wallet, sequence) -> Result<Attestation, Error>` —
  a single entry, O(1).
- `get_attestations_page(wallet, start, limit) -> Result<Vec<Attestation>, Error>`
  — bounded by a fixed, on-chain-enforced `MAX_PAGE_SIZE = 50`. Chosen
  as a small, reasoned bound: large enough to be a useful page for a
  UI or an indexer sweep, small enough that a page's total read cost
  and response size stay flat regardless of how large a wallet's total
  history grows — verified in `docs/security/resource-profile-v2.md`.
  There is deliberately no "give me everything" call any more; that is
  exactly the shape that broke in v0.1.

### Bounded TTL maintenance: two functions, not one

v0.1's `bump_wallet_ttl(wallet)` was unbounded — precisely the same
"load the whole history" pattern that caused the size ceiling, just on
the TTL-refresh path instead of the write path. It is **removed**, not
weakened under the same name, because silently changing what an
existing function name refreshes is exactly the kind of ambiguity that
causes a backend to think a wallet is being kept warm when it no longer
fully is. Two new, explicitly-scoped functions replace it:

- `bump_wallet_core_ttl(wallet)` — O(1): the wallet link, the GitHub
  link it points to, the count, and the score. Cheap enough to call
  freely and often.
- `bump_wallet_attestations_ttl_page(wallet, start, limit) -> Result<u32, Error>`
  — O(page): refreshes the `AttestationEntry` and corresponding
  `SeenPr` marker for each entry in `[start, start+limit)`, returns how
  many it actually refreshed. A backend/indexer sweeps a wallet's full
  history by calling this repeatedly with an advancing `start`; a
  return value less than `limit` (including `0`) signals the sweep has
  reached the end. See `docs/security/resource-profile-v2.md` §4 for
  the exact backend scheduling responsibility this implies.

### New error codes, old ones untouched

`Error` gains `InvalidPageLimit = 10`, `PageLimitExceeded = 11`,
`SequenceOutOfRange = 12`, `PageStartOutOfRange = 13`. Codes 1–9 keep
their exact v0.1 meaning and numeric value — nothing is renumbered.
Per `docs/RELEASE_POLICY.md`, appending new `#[contracterror]` variants
without changing existing ones is additive; removing `get_attestations`
/ `bump_wallet_ttl` and changing `AttestationRecorded`'s data shape are
what make this a breaking (major-at-1.x) change regardless.

### Event schema: additive field

`AttestationRecorded` gains a `sequence: u32` field in its data map, so
an indexer building a passport from events alone (rather than paginated
reads) can reconstruct total order and detect gaps without a separate
`get_attestation_count` round-trip per event. This is an additive
change to the data map (indexers reading by field name are unaffected
by a new field appearing); it is still called out as `v2` because it
ships alongside the removal of `get_attestations` /
`bump_wallet_ttl`, which are not additive. See
`docs/integration/event-indexer-v2.md`.

## Consequences

- **The identity/redirection guarantees from ADR 0001 and ADR 0002 are
  unchanged.** `submit_attestation` still takes a `github_id_hash`,
  still resolves the credited wallet from the on-chain `GithubLink`,
  and still never accepts a caller-supplied wallet address. Storage
  layout for *how many* attestations exist and *where* they live has no
  bearing on *who* they can be credited to.
- **`SeenPr` global de-duplication is unchanged in mechanism** — still
  one entry per `pr_hash`, checked before write, set atomically with
  the new `AttestationEntry`. What changes is which TTL-maintenance
  call refreshes it (now `bump_wallet_attestations_ttl_page`, scoped to
  the page containing that attestation, instead of the old
  whole-history `bump_wallet_ttl`).
- **This is a breaking API change.** `get_attestations` and
  `bump_wallet_ttl` no longer exist; any caller (SDK, a future backend)
  using them must migrate to the paginated equivalents. See
  `docs/migrations/v0.1-to-v0.2.md` and `docs/integration/contract-api-v2.md`.
- **No live migration is needed or attempted.** No mainnet instance
  exists; the testnet alpha instance is explicitly disposable
  (`docs/testnet/phase2-alpha.md`) and this ADR does not redeploy over
  it. v0.2 is a new contract, to be deployed at a new contract ID when
  a separate, explicit approval authorizes it.
- **A wallet can now safely accumulate far more than 286 attestations**
  — the per-entry size ceiling that caused the v0.1 failure cannot recur
  by construction, since no single entry's size depends on history
  length any more. The *number* of entries per wallet is now unbounded
  in principle (bounded in practice by the attestor's own fee cost per
  `submit_attestation` call, same as v0.1) but no longer creates an
  unrecoverable failure mode for the wallet that hits a growth
  threshold, because there is no threshold to hit.
- **Operational cost:** a backend/indexer now needs a scheduled,
  paginated keep-alive sweep instead of one call per wallet — more
  moving parts, but each part is cheap, bounded, and independently
  retriable, which a single unbounded call that eventually fails
  outright was not.

## Alternatives considered

- **Keep `Vec<Attestation>` but cap its length on-chain** (e.g. reject
  the 287th `submit_attestation` with a new error). Rejected: this caps
  a contributor's *lifetime* reputation at an arbitrary number for no
  product reason, and does not remove the underlying scaling problem —
  it just converts a rare accidental failure into a certain, permanent
  one for every sufficiently active contributor.
- **Off-chain history, on-chain hash-only commitment** (store only a
  running Merkle root or count on-chain, keep full attestation data in
  the backend/indexer). Rejected: it would make the chain no longer the
  source of truth for individual attestations, contradicting this
  project's stated purpose (`README.md`: "a contributor's whole track
  record... checkable by anyone"), and would need its own proof
  scheme to verify an individual entry against the root.
- **Fixed-size ring buffer per wallet** (keep only the most recent N
  attestations, discard older ones). Rejected: silently destroying
  earned reputation history contradicts the "portable, long-term
  contributor reputation" premise even more directly than a hard
  ceiling does.
