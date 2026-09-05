# ProofOwl event & indexer integration contract v2

Status: **normative** for `v2`. Supersedes
[`event-indexer-v1.md`](./event-indexer-v1.md), kept as the historical
record of the v0.1 event/indexer contract. **No v0.2 instance has been
deployed to any network as of this document** — see
[`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md).

## What changed from v1

- **`AttestationRecorded` gained a `sequence: u32` field** (data map,
  additive — an indexer reading by field name is unaffected by a new
  field appearing). See §1.
- **§6 (TTL maintenance monitoring) is substantially rewritten**: v1's
  single `bump_wallet_ttl` sweep is replaced by a two-part, paginated
  keep-alive obligation (`bump_wallet_core_ttl` +
  `bump_attestations_ttl_page`), because the unbounded call it
  monitored no longer exists (`docs/adr/0004-paginated-attestation-storage.md`).
- **§7 (building a passport)** is rewritten for paginated reads
  (`get_attestation_count` / `get_attestation` /
  `get_attestations_page`) in place of v1's single `get_attestations`
  call.
- Everything else — §0 (authority), §2 (fetching events), §3
  (ordering/idempotency for `GithubLinked` / `GithubUnlinked` /
  `AttestorRotated` / `Initialized`), §4 (reprocessing/replay safety),
  §5 (contract-id/network partitioning), §9 (testnet data labeling),
  §10 (divergence handling) — is **unchanged** in substance; re-read
  those sections in v1, not duplicated verbatim here except where a
  detail needed updating for the new event field.

## 0. Authority (unchanged)

**Contract read methods are authoritative. Events and indexer state are
a convenience cache.** If the indexer's derived state disagrees with
`get_attestation_count` / `get_attestation` / `get_attestations_page` /
`get_reputation_score` / `get_wallet_for_github` /
`get_github_for_wallet` / `get_admin` / `get_attestor` for a given
(contract id, network), the read method wins and the indexer must
reconcile toward it.

## 1. Event catalogue

Unchanged except `AttestationRecorded`:

### `Initialized`, `AttestorRotated`, `GithubLinked`, `GithubUnlinked`

Identical to v1 — topics, data, emitter, and meaning are unchanged. See
[`event-indexer-v1.md`](./event-indexer-v1.md) §1 for the individual
tables.

### `AttestationRecorded` (data map changed)

| | |
|---|---|
| Topics | `["attestation_recorded", wallet: Address]` |
| Data (map) | `repo: string`, `pr_number: u32`, `issue_id: u64`, `complexity: u32`, `pr_hash: BytesN<32>`, `timestamp: u64`, **`sequence: u32`** |
| Emitted by | `submit_attestation` |
| Meaning | one contribution was credited to `wallet`, at zero-based index `sequence` in its history. |

`sequence` is new in v0.2. It is the same value `get_attestation(wallet,
sequence)` addresses this exact attestation by. An indexer can use it
to:

- detect a **gap** (a `sequence` arrives that isn't
  `previous_max_sequence_for_wallet + 1`) — signals a missed event,
  triggering a reconciliation read (§10);
- detect a **replay** (a `sequence` at or below one already recorded
  for that wallet) — should be a no-op given `pr_hash`-based dedupe
  (§3) already handles this, but `sequence` gives a second, independent
  signal;
- build a passport's ordering directly from the event stream without a
  separate `get_attestation_count` round-trip per wallet.

There is still **no `bump_wallet_core_ttl` or `bump_attestations_ttl_page`
event** — TTL maintenance remains silent on-chain in v0.2, same as v1's
`bump_wallet_ttl`. Monitor it via ledger entry TTLs (§6), not events.

## 2. How to fetch events (unchanged)

Same as v1: Soroban RPC `getEvents`, filtered by `contractIds` and
optionally `topics`, with a `startLedger`/cursor for incremental
catch-up. RPC retention is still days, not indefinite; the read methods
(§0) always reflect current state regardless.

## 3. Ordering and idempotency (unchanged, `AttestationRecorded` note added)

Same ordering rule
(`ledgerSequence, transactionIndex, operationIndex, eventIndexInOperation`)
and the same idempotency key
(`contractId, ledgerSequence, txHash, opIndex, eventIndex`). Derived-state
idempotency for `AttestationRecorded` is still "dedupe by `pr_hash`,
globally unique on-chain forever" — `sequence` is additional information
for gap/replay detection (§1), not a replacement for the `pr_hash`-based
dedupe key.

## 4. Reprocessing / replay safety (unchanged)

Same as v1: safe to restart from any earlier cursor, idempotent apply
steps, no direct side effects from the apply loop, full rebuild = drop
derived tables + replay + rederive.

## 5. Contract-id and network partitioning (unchanged)

Same as v1: every row keyed by `(network, contractId)`; a v0.2 instance,
once deployed, is its own partition, distinct from the v0.1 testnet
alpha instance and from any other v0.2 instance on a different network.
See [`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md) —
v0.1 and v0.2 data are never merged or migrated automatically.

## 6. TTL maintenance monitoring (rewritten for bounded, paginated maintenance)

Persistent entries are archived when their TTL runs out; an archived
entry makes the corresponding read method fail until it is restored.
v0.2 splits this obligation into an O(1) piece and a paginated piece
(`docs/adr/0004-paginated-attestation-storage.md`); an indexer or ops
job responsible for keeping passports alive must do **both**, on a
schedule, for every wallet it considers "active":

1. **Core records (O(1)).** Call `bump_wallet_core_ttl(wallet)`. This
   covers `WalletLink`, the `GithubLink` it points to,
   `AttestationCount`, and `ReputationScore`. Cheap enough to run
   frequently for every tracked wallet.
2. **Attestation history (paginated).** Sweep the wallet's **entire**
   history in pages: start at `start = 0`; call
   `bump_attestations_ttl_page(wallet, start, limit)` (a sensible
   `limit`, e.g. 20–50, well within `MAX_PAGE_SIZE`); while the
   returned count equals `limit`, advance `start` by that count and
   call again; stop once it returns less than `limit` (including `0`).
   This refreshes every `AttestationEntry` and the `SeenPr` marker each
   one references.
3. **Re-run the full sweep periodically, not just once.** New
   attestations can be submitted between sweep runs; a wallet's history
   grows, and each sweep only covers what existed at the time it ran.
   Track the last successful sweep's cursor/timestamp per wallet so
   repeated sweeps are efficient (skip wallets swept recently whose
   `AttestationCount` hasn't changed since).
4. There is no event to key a "last swept" record on (TTL maintenance
   is silent, unchanged from v1) — record it from your own job's
   completion, the same as v1 required for `bump_wallet_ttl`.

**What happens if a page or `SeenPr` marker is allowed to archive:**
that specific page's entries (and the `SeenPr` markers they reference)
become unreadable until `RestoreFootprint` runs, on a live network —
localized to that page, not the whole wallet, unlike v0.1's all-or-
nothing single entry. This does not un-spend a `pr_hash` once restored;
see `docs/security/threat-model-v1.md` §9 and
`tests/ttl_replay.rs`'s module doc comment for the full explanation.

**Monitor**, same as v1:

- **Instance TTL** (unchanged — `getLedgerEntries` for the contract
  instance, or `stellar contract info ttl`; any authorized call
  extends it, escalate if it approaches archival).
- **Per-wallet, per-page TTL** for active wallets: for each page of a
  wallet's history, the TTL of its `AttestationEntry` keys (and, by
  construction, the `SeenPr` markers they reference, kept in step by
  the sweep in step 2 above).

The contract's TTL policy (120-day extend target, 90-day bump
threshold) is unchanged from v1 — `SECURITY.md` §5.

## 7. Building a passport (rewritten for pagination)

Given a `wallet`:

1. **Link:** `linked_github_id_hash = get_github_for_wallet(wallet)`
   (unchanged from v1) — `null` ⇒ currently unlinked (history may still
   exist).
2. **Count:** `count = get_attestation_count(wallet)` — O(1).
3. **History:** page through it — `start = 0`; call
   `get_attestations_page(wallet, start, limit)` (`limit` up to
   `MAX_PAGE_SIZE` = 50); append results; advance `start` by the page's
   length; stop when a page returns fewer than `limit` entries (or when
   `start == count`, which returns an empty page immediately). Each
   entry: `{ repo, pr_number, issue_id, complexity, pr_hash, timestamp }`
   — reconstruct the PR URL as `https://github.com/<repo>/pull/<pr_number>`,
   and its `sequence` is its position in the fetched order (`start_of_page
   + index_within_page`).
4. **Verify each entry** (recommended, unchanged from v1): recompute
   `hashGitHubPullRequestV1(owner, repo, pr_number)` and compare to the
   stored `pr_hash`. Flag mismatches; do not silently drop them.
5. **Score:** `get_reputation_score(wallet)` — now **O(1)** on-chain
   (v0.2 reads a running counter instead of re-summing); still
   independently recomputable as
   `Σ (complexity > 0 ? complexity : 50)` with saturating add from the
   paged history, if you want a second check. Both must agree; if not,
   the read method wins.
6. **From events alone** (cache warm-up without a read every request):
   maintain `attestations[wallet]` as an append-only list keyed by
   `pr_hash` from `AttestationRecorded`, ordered by its `sequence`
   field, and `link[wallet]` from `GithubLinked` / `GithubUnlinked`.
   Periodically reconcile against the read methods (§0).

Notes (unchanged from v1):

- `GithubUnlinked` does **not** remove history.
- A `pr_hash` seen for one wallet is spent globally, forever.
- `issue_id == 0` and `complexity == 0` are valid, not errors.

## 8. Handling each event in the apply loop

| Event | Derived-state effect |
|---|---|
| `Initialized` | upsert `instance{network, contractId} = {admin, attestor, createdLedger}` |
| `AttestorRotated` | set `instance.attestor = new_attestor`; optionally record a rotation-history row |
| `GithubLinked` | set `link[wallet] = github_id_hash`; set `reverse[github_id_hash] = wallet`; mark `linkedAtLedger` |
| `GithubUnlinked` | clear `link[wallet]` and `reverse[github_id_hash]` **iff** they currently equal this pair; keep `attestations[wallet]` |
| `AttestationRecorded` | append to `attestations[wallet]` keyed by `pr_hash` (dedupe), recorded at index `sequence`; update `score[wallet]`; index by `pr_hash` and by `repo` |

Apply strictly in event order (§3). After a batch, optionally reconcile
touched wallets against `get_attestation_count` / `get_reputation_score`.

## 9. Testnet data (unchanged, restated)

The v0.1 testnet alpha instance
(`CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6`) remains a
**disposable testnet alpha for v0.1** — it does not speak this v2
contract. No v0.2 instance exists on any network yet. When one is
deployed (under separate approval), it is a fresh
`(network, contractId)` partition with the same disposability caveats
until stated otherwise.

## 10. Divergence handling (unchanged, restated)

When indexer state and a read method disagree:

1. the read method is correct;
2. reconcile the affected `(network, contractId, wallet | github_id_hash)`
   from the read method (using paginated reads for history, per §7);
3. record the divergence for investigation.
