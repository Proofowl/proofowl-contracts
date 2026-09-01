# ProofOwl event & indexer integration contract v1

Status: **normative** for `v1`. Describes every contract event and how an
indexer (or the backend) should consume them to build passport history
and scores without ever diverging dangerously from on-chain truth.

## 0. Authority

**Contract read methods are authoritative. Events and indexer state are
a convenience cache.** If the indexer's derived state disagrees with
`get_attestations` / `get_reputation_score` / `get_wallet_for_github` /
`get_github_for_wallet` / `get_admin` / `get_attestor` for a given
(contract id, network), the read method wins and the indexer must
reconcile toward it. Never serve a write, a payout, or a
security-relevant decision from indexer state without a read-method
confirmation.

## 1. Event catalogue

Every event uses `data_format = map` (fields addressed by name, not
position). The first topic is a fixed string equal to the event's
snake_case name; the second topic is the indexed address.

### `Initialized`

| | |
|---|---|
| Topics | `["initialized", admin: Address]` |
| Data (map) | `attestor: Address` |
| Emitted by | `__constructor` (deploy time, exactly once) |
| Meaning | this contract instance was created with `admin` and `attestor`. |

### `AttestorRotated`

| | |
|---|---|
| Topics | `["attestor_rotated", admin: Address]` |
| Data (map) | `new_attestor: Address` |
| Emitted by | `set_attestor` |
| Meaning | the attestor key changed. Records that will now verify against a different key. |

### `GithubLinked`

| | |
|---|---|
| Topics | `["github_linked", wallet: Address]` |
| Data (map) | `github_id_hash: BytesN<32>` |
| Emitted by | `link_github` |
| Meaning | `wallet` and `github_id_hash` are now linked (both directions). |

### `GithubUnlinked`

| | |
|---|---|
| Topics | `["github_unlinked", wallet: Address]` |
| Data (map) | `github_id_hash: BytesN<32>` |
| Emitted by | `unlink_github` |
| Meaning | the link between `wallet` and `github_id_hash` was removed. Attestation history and score for `wallet` are **unchanged**. |

### `AttestationRecorded`

| | |
|---|---|
| Topics | `["attestation_recorded", wallet: Address]` |
| Data (map) | `repo: string`, `pr_number: u32`, `issue_id: u64`, `complexity: u32`, `pr_hash: BytesN<32>`, `timestamp: u64` |
| Emitted by | `submit_attestation` |
| Meaning | one contribution was credited to `wallet`. `timestamp` is the ledger close time the contract set. |

There is **no `bump_wallet_ttl` event** — TTL maintenance is silent
on-chain. Monitor it via ledger entry TTLs, not events (§6).

## 2. How to fetch events

Use the Soroban RPC `getEvents` method, filtered by:

- `contractIds`: the exact contract id you are indexing;
- `topics`: optionally the first topic (`"initialized"`,
  `"github_linked"`, …) to narrow;
- a `startLedger` / cursor for incremental catch-up.

RPC serves events only within its retention window (days). For a full
history from genesis of the contract you must either index continuously
from deployment or backfill from an archival source. For state you
cannot reconstruct from the retained window, fall back to the read
methods (§0), which always reflect current state.

## 3. Ordering and idempotency

- **Order events by `(ledgerSequence, transactionIndex, operationIndex,
  eventIndexInOperation)`** — the RPC returns them in this order within
  a page; preserve it across pages.
- Two events in the **same** transaction are totally ordered by the
  indices above. This contract emits at most one event per successful
  call, so intra-transaction ordering rarely matters, but honour it.
- **Idempotency key** for an event row:
  `(contractId, ledgerSequence, txHash, opIndex, eventIndex)`. Upsert on
  this key; re-ingesting the same event is a no-op.
- **Derived-state idempotency:**
  - `GithubLinked` / `GithubUnlinked`: apply as "set link" / "clear
    link" — replaying them in order converges. If you see
    `GithubLinked(w, h)` while your state already has `w` linked to a
    *different* hash, your state is stale — reconcile from
    `get_github_for_wallet(w)`.
  - `AttestationRecorded`: dedupe by `pr_hash` (globally unique on-chain
    forever). A replay with a `pr_hash` you already have is a no-op.
  - `AttestorRotated` / `Initialized`: last-write-wins on
    `(contractId)`.

## 4. Reprocessing / replay safety

- The indexer must be safe to **restart from any earlier cursor** and
  replay to head. Achieve this by making every apply step idempotent
  (§3) and never doing side effects (emails, payouts, further chain
  writes) directly from the apply loop — emit domain events that a
  separate, also-idempotent consumer handles.
- A full rebuild = drop derived tables, replay the raw event log (or
  re-fetch within the RPC window + read-method reconciliation for the
  gap), rederive.
- Never trust event data over a read method during reconciliation.

## 5. Contract-id and network partitioning

- **Every stored row is keyed by `(network, contractId)`.** Testnet and
  any future mainnet instance are separate universes; the testnet alpha
  instance is disposable and may be replaced by a new contract id.
- `network` = the network passphrase (authoritative), e.g.
  `Test SDF Network ; September 2015`. Do not key by a human name like
  "testnet".
- Config: the indexer takes `(rpcUrl, networkPassphrase, contractId)`
  as explicit input — no hard-coded defaults. Verify at startup that
  `getNetwork.passphrase` matches the configured passphrase and that
  `get_admin()` / `get_attestor()` return non-null for the contract id.
- Cross-instance queries (e.g. "did this GitHub id ever link on any
  instance") must be explicit unions over partitions, never an
  accidental merge.

## 6. TTL maintenance monitoring

Persistent entries are archived when their TTL runs out; an archived
entry makes the corresponding read method fail until it is restored.
The indexer / an ops job should watch:

- **Instance TTL** of the contract (`getLedgerEntries` for the contract
  instance; or `stellar contract info ttl`). If it approaches the
  archival threshold, any authorized caller can extend it by making any
  contract call; escalate to ops.
- **Per-wallet passport TTL**: for active wallets, monitor the TTL of
  `WalletLink(wallet)` / `Attestations(wallet)` / each `SeenPr` marker.
  When any nears expiry, call `bump_wallet_ttl(wallet)` (permissionless;
  it costs a fee, changes no data, and refreshes all of them). A daily
  or weekly sweep over "wallets with events in the last N days" is the
  intended pattern.
- Track the last `bump_wallet_ttl` transaction per wallet so sweeps are
  not redundant. There is no event to key on — record it from your own
  submission.

The contract's TTL policy (120-day extend target, 90-day bump
threshold) is in `SECURITY.md` §5.

## 7. Building a passport

Given a `wallet`:

1. **Link:** `linked_github_id_hash = get_github_for_wallet(wallet)`
   (or derive from the latest `GithubLinked` / `GithubUnlinked` for that
   wallet, then confirm with the read). `null` ⇒ currently unlinked
   (but history may still exist).
2. **History:** `get_attestations(wallet)` — ordered oldest→newest.
   Each entry: `{ repo, pr_number, issue_id, complexity, pr_hash,
   timestamp }`. Reconstruct the PR URL as
   `https://github.com/<repo>/pull/<pr_number>`.
3. **Verify each entry** (recommended): recompute
   `hashGitHubPullRequestV1(owner, repo, pr_number)` and compare to the
   stored `pr_hash` (see
   [`identifier-spec-v1.md`](./identifier-spec-v1.md) §2.6). Flag
   mismatches; do not silently drop them.
4. **Score:** `get_reputation_score(wallet)` — or recompute:
   `Σ (complexity > 0 ? complexity : 50)` with saturating add. Both
   must agree; if not, the read method wins.
5. **From events alone** (cache warm-up without a read every request):
   maintain `attestations[wallet]` as an append-only list keyed by
   `pr_hash` from `AttestationRecorded`, and `link[wallet]` from
   `GithubLinked` / `GithubUnlinked`. Periodically reconcile against the
   read methods.

Notes:

- `GithubUnlinked` does **not** remove history — keep the attestations
  and score attached to `wallet`.
- A `pr_hash` seen for one wallet is spent globally; you will never see
  it credited to another wallet (the contract enforces this).
- `issue_id == 0` and `complexity == 0` are valid ("not applicable" /
  "tier unknown"), not errors.

## 8. Handling each event in the apply loop

| Event | Derived-state effect |
|---|---|
| `Initialized` | upsert `instance{network, contractId} = {admin, attestor, createdLedger}` |
| `AttestorRotated` | set `instance.attestor = new_attestor`; optionally record a rotation-history row |
| `GithubLinked` | set `link[wallet] = github_id_hash`; set `reverse[github_id_hash] = wallet`; mark `linkedAtLedger` |
| `GithubUnlinked` | clear `link[wallet]` and `reverse[github_id_hash]` **iff** they currently equal this pair; keep `attestations[wallet]` |
| `AttestationRecorded` | append to `attestations[wallet]` keyed by `pr_hash` (dedupe); update `score[wallet]`; index by `pr_hash` and by `repo` |

Apply strictly in event order (§3). After a batch, optionally reconcile
touched wallets against `get_attestations` / `get_reputation_score`.

## 9. Testnet data

- The current instance
  (`CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6`) is a
  **disposable testnet alpha**. Data may be wiped by a redeploy at a new
  contract id at any time.
- Backend/frontend MUST label anything sourced from it as testnet, MUST
  NOT present testnet reputation as real, and MUST make the
  `(network, contractId)` it is showing explicit in the UI/API.
- A new deployment ⇒ a new `(network, contractId)` partition and a fresh
  index; do not migrate testnet rows forward automatically.

## 10. Divergence handling (restated)

When indexer state and a read method disagree:

1. the read method is correct;
2. reconcile the affected `(network, contractId, wallet | github_id_hash)`
   from the read method;
3. record the divergence for investigation (it usually means a missed
   event, an RPC retention gap, or an ordering bug — occasionally an
   attestor-submitted inconsistency worth escalating).
