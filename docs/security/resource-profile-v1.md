# ProofOwl contract — resource and scalability profile v1

Status: **normative** for Phase 4. Answers, with measured evidence: how
does cost grow as a wallet's attestation history grows, and where —
concretely, not hypothetically — does it stop working at all.

## Methodology

All numbers below come from `tests/resource_profile.rs`
(`#[ignore]`d; run explicitly with `make resource-profile`, generated
2026-09-05 against `soroban-sdk 27.0.6` / the release WASM built from
this repository's current commit). Two things matter for trusting these
numbers:

1. **Real WASM, not the native test-contract shortcut.** Every other
   test file in this repository uses `Env::register(ProofOwlRegistry,
   …)`, which the SDK's own docs say **skips WASM VM instantiation and
   execution cost** entirely — fine for correctness tests, useless for
   a resource profile. `tests/resource_profile.rs` instead builds the
   release artifact (`cargo build --target wasm32v1-none --release`)
   and deploys it for real via `Deployer::deploy_v2`, the same
   mechanism `tests/constructor_auth.rs` uses to test the real deploy
   path. Every number below reflects an actual WASM invocation.
2. **`env.cost_estimate()` is the SDK's own supported instrumentation**
   (`resources()` for CPU/memory/storage figures, `fee()` for a
   stroop-denominated estimate), not a hand-written approximation. Its
   own documentation is explicit that this is a **model**, based on a
   fee-configuration snapshot the SDK dates 2026-07-10, not a live
   simulation against current mainnet or testnet state. Nothing here is
   a claimed mainnet benchmark — it is what the SDK's own supported
   tooling reports for a real WASM invocation, which is the strongest
   offline evidence available without a live network call (explicitly
   out of scope for this phase).

All measurements are for **one wallet's growing history in isolation**
— no other wallets, no other contract state — which is the worst-case
shape the "single `Vec<Attestation>` per wallet" design
(`SECURITY.md` §7) actually stresses.

## 1. Measured growth: cost vs. history size

| history size (N) | `submit_attestation` instructions | mem (bytes) | write (bytes) | fee estimate (stroops) | `get_attestations` instructions | `get_reputation_score` instructions | `bump_wallet_ttl` instructions |
|---:|---:|---:|---:|---:|---:|---:|---:|
| 1   | 766,593   | 1,253,364 | 568    | 12,134,204 | 436,185   | 456,413   | 607,091    |
| 5   | 832,368   | 1,266,420 | 1,480  | 9,335,029  | 454,206   | 535,074   | 812,101    |
| 10  | 904,721   | 1,284,435 | 2,620  | 9,336,054  | 476,073   | 632,741   | 1,079,195  |
| 25  | 1,113,258 | 1,338,480 | 6,040  | 9,339,123  | 538,872   | 922,940   | 1,923,004  |
| 50  | 1,461,414 | 1,428,555 | 11,740 | 9,344,236  | 644,394   | 1,407,462 | 3,466,133  |
| 100 | 2,136,540 | 1,608,705 | 23,140 | 9,354,450  | 854,076   | 2,375,144 | 7,012,934  |
| 200 | 3,486,343 | 1,969,005 | 45,940 | 9,374,878  | 1,272,123 | 4,309,191 | 15,812,328 |

Mainnet invocation limits (SDK snapshot, same source as above):
instructions = 400,000,000; mem_bytes = 41,943,040; write_bytes =
132,096; **`max_contract_data_entry_size_bytes` = 65,536**.

Observations:

- Every operation grows **roughly linearly** in `N`, exactly as
  expected for "load the whole `Vec<Attestation>`, touch every entry
  (or rewrite it)": `bump_wallet_ttl` (which extends every `SeenPr`
  marker referenced by the history, one storage TTL bump per entry) is
  the fastest-growing — its instruction count roughly doubles for each
  doubling of `N` (3.47M → 7.01M → 15.8M going 50 → 100 → 200), and it
  is already the single most expensive operation measured at every
  history size.
- At `N = 200`, `bump_wallet_ttl` sits at **3.95% of the mainnet
  instruction limit** and 4.82% of the memory limit — nowhere near
  binding on either axis at this size.
- Interestingly, `submit_attestation`'s **fee estimate goes down** from
  N=1 to N=5 before climbing again (12.1M → 9.34M stroops); this is a
  fixed-cost artifact of the very first write to a brand-new persistent
  key (initial rent/allocation cost for the entry) rather than a real
  per-entry saving — the underlying instruction count is monotonically
  increasing throughout, which is what the profile's own regression
  assertion checks (`resources.instructions >= last_submit_instr`).

**CPU and memory are not the binding constraint at any size tested.**
Something else is.

## 2. The hard ceiling — measured, not projected

`tests/resource_profile.rs::find_the_hard_history_size_ceiling`
continues submitting one attestation at a time, past the point where
`measure_attestation_history_growth` stops, until the SDK's own
resource-limit enforcement rejects the call. It does, deterministically,
every run:

> **Wallet history of 286 attestations succeeds. The 287th
> `submit_attestation` call fails outright** — not "costs more," fails
> — because the single `Attestations(wallet)` contract-data entry has
> grown to 65,576 bytes, 40 bytes past the 65,536-byte
> `max_contract_data_entry_size_bytes` ceiling every Soroban contract
> data entry is subject to, on mainnet as much as in this test.

This is measured against exactly this contract's current `Attestation`
shape (`repo: String`, `pr_number: u32`, `issue_id: u64`, `complexity:
u32`, `pr_hash: BytesN<32>`, `timestamp: u64`) with a realistic 25-byte
`repo` string (`"stellar/soroban-examples"`). **The exact number shifts
with `repo` string length** — a project with longer `owner/repo` names
on average would hit the ceiling somewhat sooner; a shorter one,
somewhat later. The order of magnitude (high 200s to low 300s for a
25-character `repo`) is the load-bearing fact, not the single digit.

Past this point, for that one wallet, **forever, with no recovery
path**:

- `submit_attestation` for that identity can never succeed again — not
  "costs too much," **cannot fit in one ledger entry at all**, at any
  fee.
- `get_attestations` and `get_reputation_score` — which load the same
  oversized entry — likely fail identically (not separately measured
  here, since they read the same one entry `submit_attestation` writes;
  the failure mode is the entry itself being unreadable/unwritable in a
  single operation, not a function-specific limit).
- `bump_wallet_ttl` for that wallet fails the same way, meaning **the
  wallet's entire history and its link both become impossible to keep
  alive** — the exact TTL-expiry risk `SECURITY.md` §5 designed
  `bump_wallet_ttl` to prevent stops being preventable past this point.
- There is no admin or attestor override that can split, migrate, or
  truncate an existing wallet's history — no such function exists, by
  design (`SECURITY.md` §1.4, §4.2) — so this is not a recoverable
  operational incident. It is a permanent, structural dead end for that
  one wallet's on-chain reputation.

## 3. Who can hit this, and how

Only the trusted **attestor** can grow any wallet's history
(`submit_attestation` is attestor-only). Two ways the ceiling is
reached:

- **Organically, over the program's real lifetime.** A prolific,
  long-tenured contributor accumulating ~300 merged, Wave-labeled PRs
  across the program's life is not an adversarial scenario — it is the
  literal success case this contract exists to reward. A registry whose
  premise is "portable, long-term contributor reputation" hitting a
  hard, unrecoverable wall for its *most successful* users is a product
  risk, not merely an edge case.
- **Adversarially**, per `docs/security/threat-model-v1.md` §11: a
  compromised or malicious attestor deliberately floods one victim
  wallet to lock it out of ever receiving another attestation or TTL
  refresh — bounded by the attestor's own per-call fee cost, but cheap
  relative to permanently disabling a specific contributor's passport.

## 4. Verdict

**Requires an indexed/paginated storage redesign before mainnet or any
production scope without an enforced per-wallet attestation cap.**

Reasoning:

- CPU, memory, and write-byte costs are all comfortably inside mainnet
  limits even at 200 attestations (§1) — this is *not* a cost problem
  that a bigger fee budget or more patience would solve.
- The actual constraint is structural: one wallet's entire history
  shares one contract-data entry, which has a fixed 65,536-byte ceiling
  regardless of fee. That ceiling is reached at a history size
  (high-200s for this `Attestation` shape) that a genuinely successful,
  long-tenured contributor — the exact user this contract is meant to
  serve — can plausibly reach over the program's real lifetime, not
  just in a contrived stress test.
- Once reached, the wallet is **permanently locked out** of new
  attestations *and* of the TTL keep-alive that protects its existing
  history from archival, with no override or recovery path by design.
  This combination (reachable in normal operation + no recovery) is
  exactly what pushes the verdict past "acceptable for testnet alpha"
  or "acceptable for a bounded v1 production scope."
- It **is** acceptable, unconditionally, for the current **Testnet
  Alpha** (`docs/testnet/phase2-alpha.md`) — a disposable instance with
  no expectation of a multi-year contributor history — and for any
  **explicitly bounded** production pilot that enforces (off-chain, in
  the not-yet-built backend) a hard per-wallet attestation cap set
  comfortably below the measured ceiling (e.g., a policy cap of 150–200
  lifetime attestations per wallet, with headroom for `repo` string
  length variance), until the redesign below ships.

## 5. Minimal migration-safe redesign proposal (not implemented this phase)

Consistent with `SECURITY.md` §7's already-stated direction, described
here in enough detail to scope the work — **not implemented in this
phase**, per the adversarial-testing mandate that a material flaw is
documented, not silently redesigned.

**Shape:**

- Replace `DataKey::Attestations(Address) -> Vec<Attestation>` with:
  - `DataKey::AttestationCount(Address) -> u32` — number of
    attestations for a wallet.
  - `DataKey::AttestationEntry(Address, u32) -> Attestation` — one
    persistent entry per attestation, keyed by `(wallet, seq)`.
  - `DataKey::ReputationScore(Address) -> u32` — a running total
    updated incrementally on each `submit_attestation`, instead of
    re-summed from a full history scan on every
    `get_reputation_score` call.
- `submit_attestation` becomes O(1) in history size: read the count,
  write one new entry at `seq = count`, increment the count, add the
  new points to the running score. No entry ever grows with history
  length again — the *per-entry* size ceiling that causes today's hard
  failure disappears entirely; only the *total number of entries* for
  a wallet grows, and Soroban has no equivalent ceiling on entry count.
- `get_attestations` needs a paginated form (e.g. `get_attestations_page(wallet,
  start, limit) -> Vec<Attestation>`) since returning an unbounded
  number of individually-keyed entries in one call reintroduces an
  unbounded-response-size problem at extreme scale; a bounded default
  page size keeps every call's cost flat regardless of history length.
- `bump_wallet_ttl` extends `AttestationCount`, `ReputationScore`, and
  either every `AttestationEntry` (if kept fully warm) or a
  most-recently-touched window plus the count/score entries (if a
  cheaper partial keep-alive is judged sufficient) — this specific
  trade-off needs its own design pass, not resolved here.

**Migration safety — the good news:** no mainnet instance exists yet
(`README.md` "Deployed contracts", `PRODUCTION_READINESS.md` Gate 6).
Soroban contracts have no upgrade mechanism and this project has
deliberately accepted immutability (`PRODUCTION_READINESS.md` 6.8), so
there is **no live-migration problem to solve** — the correct sequencing
is to ship this storage redesign *before* the first mainnet deployment,
not to migrate an existing one. The only "migration" this repository
will ever need is:

- The **testnet alpha** instance is explicitly disposable
  (`docs/testnet/phase2-alpha.md`, `event-indexer-v1.md` §9) and is
  expected to be replaced by a fresh deployment regardless of this
  change — no data-carrying migration is owed to it.
- If a future pilot's testnet or mainnet instance *does* need to carry
  data forward from an old-shaped instance to a new-shaped one, the old
  contract's data is fully public and read-only forever
  (`get_attestations`, `get_github_for_wallet`, etc.), so a one-time,
  off-chain backfill script (read every wallet's old-shape history via
  the old contract's own read methods, replay it into the new
  contract's `submit_attestation`-equivalent seeding path) is sufficient
  — no contract-level migration primitive is required. This still needs
  its own design (in particular, whether re-emitting historical
  `AttestationRecorded` events on the new contract at backfill time is
  desirable for indexer continuity) — flagged as follow-up work, not
  resolved here.

## 6. What this phase did not attempt

- No gas numbers are claimed as mainnet-accurate; every figure above is
  explicitly the SDK's own modelled estimate, dated and sourced.
- No live testnet or mainnet call was made to validate these numbers
  against a real network (out of scope for this phase, and the read-only
  testnet checks that do exist — `make sdk-integration-testnet` — do not
  exercise cost accounting).
- No storage redesign was implemented — see §5's explicit scope note.
- Event payload growth and indexer-side bottlenecks were not separately
  measured: `AttestationRecorded`'s data size is dominated by the same
  `repo` string and fixed-width fields already profiled here per-entry
  in §1's write-byte column, and no other event in this contract carries
  variable-length data. A dedicated indexer-throughput analysis is out
  of scope until an indexer exists to measure (`docs/architecture.md`).
