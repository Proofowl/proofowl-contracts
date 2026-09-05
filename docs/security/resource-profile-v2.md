# ProofOwl contract — resource and scalability profile v2

Status: **normative** for the local v0.2 candidate. Supersedes the
verdict of `docs/security/resource-profile-v1.md` (v0.1's measured
286-attestation hard ceiling) with evidence for the paginated,
per-record storage design (`docs/adr/0004-paginated-attestation-storage.md`).
**v1's numbers are kept unedited** as the evidence record that
justified this redesign — read this document alongside it, not instead
of it.

## Methodology (unchanged from v1, restated)

All numbers below come from `tests/resource_profile.rs` (`#[ignore]`d;
run explicitly with `make resource-profile`), generated 2026-09-05
against `soroban-sdk 27.0.6` and the release WASM built from this
repository's current v0.2 commit. As in v1:

1. **Real WASM, not the native test-contract shortcut** — every
   measurement deploys the actual compiled artifact via
   `Deployer::deploy_v2`, so every number reflects a genuine WASM
   invocation, not the native-contract path that skips VM
   instantiation cost.
2. **`env.cost_estimate()` is the SDK's own supported instrumentation**
   — a *modelled* estimate, dated by the SDK to 2026-07-10, not a live
   simulation against current mainnet or testnet state. Nothing here is
   a claimed mainnet benchmark.

All measurements are for wallets in isolation or in pairs, at the sizes
stated per test — chosen to comfortably exceed v0.1's 286-attestation
ceiling and to stress the specific properties the redesign claims.

## 1. No ceiling, at more than 3x the v0.1 failure point

`v2_submit_attestation_never_hits_a_ceiling_and_stays_resource_bounded`
grew one wallet to **1000 attestations** (v0.1 failed outright at 287).
Every write succeeded; `get_attestation_count` confirmed all 1000 are
present. This is the headline fix: the per-wallet, per-entry-size
ceiling that made v0.1 permanently unusable past 286 attestations
**cannot recur by construction** in v0.2 — no single entry's size grows
with history length, ever (ADR 0004).

| history size (N) | `submit_attestation` instructions | mem (bytes) | write (bytes) | fee estimate (stroops) |
|---:|---:|---:|---:|---:|
| 1    | 956,232   | 1,265,325 | 848 | 17,747,901 |
| 50   | 1,329,230 | 1,371,964 | 848 | 12,143,080 |
| 300  | 2,944,700 | 1,932,964 | 848 | 12,144,211 |
| 500  | 4,228,086 | 2,381,764 | 848 | 12,145,109 |
| 750  | 5,823,917 | 2,942,764 | 848 | 12,146,226 |
| 1000 | 7,428,949 | 3,503,764 | 848 | 12,147,350 |

At N=1000, `submit_attestation` costs **1.86% of the mainnet
instruction limit** (400,000,000) — nowhere near a resource constraint.
`write_bytes` is flat at 848 for every sample (N≥50): each call always
writes exactly the same handful of small, fixed-size records (one new
`AttestationEntry`, `AttestationCount`, `ReputationScore`, `SeenPr`,
plus TTL bumps on the links) — never a growing blob.

## 2. Honest finding: write cost is not perfectly flat — read cost is

This is stated plainly rather than smoothed over, because Phase 4's own
standard was to report evidence honestly, not to declare victory.

**The marginal cost of writing a new attestation grows with the total
number of prior entries under the contract**, roughly linearly, at
about **6,400 additional instructions per pre-existing entry** once
past the first few dozen (the N=1→50 segment runs slightly hotter at
~7,600/entry, plausibly one-time link/setup amortization):

| segment | instructions per new attestation |
|---|---:|
| N=1 → 50   | 7,612 |
| N=50 → 300 | 6,462 |
| N=300 → 500 | 6,417 |
| N=500 → 750 | 6,383 |
| N=750 → 1000 | 6,420 |

This is why N=1000's absolute cost (7.43M instructions) is ~7.8x N=1's
(956K) rather than ~1x — a large, constant base-invocation cost (WASM
instantiation etc., independent of history) plus a small, roughly
linear per-entry term.

**Reads and TTL-only updates to *existing* keys show no such growth.**
`v2_page_operations_cost_depends_on_page_size_not_history_size` read
50-entry pages, and separately ran a 50-entry paginated TTL bump,
starting at position 0, 400, and 850 within a 900-entry history:

| page start | `get_attestations_page` instructions | `bump_attestations_ttl_page` instructions |
|---:|---:|---:|
| 0   | 4,684,450 | 8,762,247 |
| 400 | 4,775,962 | 8,936,623 |
| 850 | 4,762,456 | 8,913,321 |

Range: 1.02x across the whole 900-entry history — genuinely flat,
regardless of how deep into the history the page starts.
`v2_reputation_score_lookup_is_constant_time` confirms the same for the
running score counter: 427,537 instructions at N=1 vs. 460,742 at N=500
— a 1.08x ratio, consistent with a true O(1) single-key read (the small
residual difference is noise, not growth).

**Interpretation, stated as a hypothesis, not a fact:** the asymmetry
(new-entry writes scale with total prior entries; reads and
existing-key updates do not) is consistent with the *local Soroban test
host's* in-memory ledger-snapshot/footprint bookkeeping for newly
created entries (this harness captures a full ledger snapshot at `Env`
drop — visible as the `test_snapshots/*.json` files each test run
writes) rather than a documented mainnet cost characteristic. Soroban's
production storage backend does not document a cost tied to total
prior entry count for writing one new, unrelated key. **This was not
conclusively isolated in this phase.** It is flagged here as a
follow-up investigation — ideally by comparing against a real testnet
simulation once a v0.2 instance is deployed under separate approval —
not asserted as either "a real mainnet cost" or "definitely just a test
artifact."

**What this does and does not change about the verdict:** even taking
the observed linear-with-small-slope growth at face value and
extrapolating it, the practical implication is bounded, not
catastrophic. At the small ~6,400/entry slope, reaching even 10% of the
mainnet instruction limit (40,000,000) purely from this per-entry term
would take roughly 6,000 total entries under the contract — and unlike
v0.1, there is no hard wall at any point; cost degrades gracefully
rather than failing outright. This is categorically different from
v0.1's finding, where the failure was a fixed, unrecoverable ceiling at
a specific, low count.

## 3. Cannot brick another contributor's profile

`v2_one_wallets_history_size_cannot_brick_another_wallet` grew wallet A
to 900 attestations, then had wallet B — brand new — submit its first
ever attestation, read its score, and run its core TTL bump, all
**after** A was already large:

- Wallet B's first `submit_attestation`: 9,058,969 instructions
  (2.26% of the mainnet instruction limit) — elevated relative to an
  isolated first attestation (956,232 at N=1 in test 1, consistent with
  §2's total-prior-entries effect), but **it succeeded**, at a small
  fraction of any resource limit.
- Wallet B's `get_reputation_score`: 483,562 instructions — in the same
  band as test 1's O(1) score reads.
- Wallet B's `bump_wallet_core_ttl`: 692,213 instructions.
- Wallet A's history was completely unaffected: `get_attestation_count`
  still returned exactly 900 afterward.

The specific v0.1 failure mode this test rules out — one wallet's
growth permanently disabling *another* wallet's ability to receive
attestations or TTL refreshes — **cannot happen in v0.2**: wallet B's
call did not fail, did not approach any resource limit, and did not
touch wallet A's data. The §2 caveat about total-contract-size affecting
absolute cost applies here too (wallet B's cost is not perfectly
isolated in absolute terms in this harness) — but "somewhat more
expensive, still far under any limit" is not "bricked," which is the
property that matters.

## 4. No entry approaches the 65,536-byte per-entry limit

`v2_no_entry_approaches_the_soroban_size_limit` used a deliberately
oversized 200-byte `repo` string (far larger than any real
`<owner>/<repo>`, capped at 39+100 characters per
`identifier-spec-v1.md`). The **entire invocation's** write footprint —
`AttestationEntry` + `AttestationCount` + `ReputationScore` + `SeenPr` +
link TTL bumps, a conservative over-count of the one entry's own size —
totaled **1,024 bytes**, 1.6% of the 65,536-byte limit. A full 50-entry
`get_attestations_page` response used 1,406,507 bytes of modelled
memory, 3.35% of the 41,943,040-byte mainnet memory limit. Neither
comes close to any ceiling. This directly answers the question the
`MAX_PAGE_SIZE = 50` choice needed to survive: even a worst-case page of
worst-case-sized records is nowhere near a limit.

## 5. Verdict

**Acceptable for a bounded v1 production scope, and a substantial,
measured improvement over v0.1 — with one open item to track, not a
blocker.**

- The specific Phase 4 finding (a hard, unrecoverable per-wallet
  ceiling at 286 attestations) is **resolved**: no entry's size grows
  with history length, verified to 1000 attestations with no failure
  and no sign of an approaching wall.
- Reads, score lookups, and TTL maintenance are genuinely bounded —
  flat cost regardless of position in a large history or total history
  size.
- Write cost has a small, empirically linear component with total
  contract entry count whose root cause (local test-harness artifact
  vs. a real characteristic) was not conclusively determined this
  phase. Recommended before treating this as fully closed for
  unbounded/mainnet scale: re-measure against a real testnet deployment
  of v0.2 (which needs the separate approval this phase's rules
  require before any deployment) to see whether the same growth
  appears against a live RPC-backed ledger, not just the local
  in-process snapshot host.
- No individual entry or page response approaches Soroban's size
  limits under the documented `MAX_PAGE_SIZE = 50` constraint.

This does **not** require the storage redesign v1 called for — that
redesign is what this document is evidence *for*, already implemented
locally. It does not yet warrant an unconditional "acceptable for
unbounded mainnet scale" verdict, pending the real-network
re-measurement noted above; that distinction is why this is phrased as
"a bounded v1 production scope" rather than "any production scope."

## 6. What this phase did not attempt

- No gas numbers are claimed as mainnet-accurate; every figure above is
  explicitly the SDK's own modelled estimate, dated and sourced, exactly
  as in v1.
- No live testnet or mainnet call was made — this phase's rules
  explicitly prohibit any testnet write, and a v0.2 deployment requires
  separate approval not yet given.
- The total-prior-entry-count cost effect (§2) was measured and
  reported, not root-caused. Investigating the local Soroban test
  host's snapshot/footprint implementation in detail, or reproducing
  the same measurement against a real RPC-backed network, is explicitly
  deferred.
- Event payload growth was not separately re-measured; `AttestationRecorded`
  gained one additional fixed-width `u32` field (`sequence`) since v1,
  a negligible addition to its already-measured size profile.
