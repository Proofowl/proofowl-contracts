# ProofOwl contract — security review checklist v1

Status: **normative** for Phase 4 and beyond. This is the line-item
checklist a reviewer (internal or external) works through before
signing off on any release beyond the current disposable testnet alpha.
It complements, and cross-references, `docs/security/threat-model-v1.md`
and `docs/security/resource-profile-v1.md` rather than repeating them.

## How to use this document

- Tick items off against a specific commit SHA, not "the repo in
  general." Record the SHA at the top of the review record.
- A `NOT MET` or `PARTIAL` item is not automatically a blocker — check
  it against §2 (release-blocker definition) below.
- This checklist reviews the **contract, its tests, and its published
  integration surface** (`src/`, `tests/`, `sdk/typescript/`,
  `docs/integration/`, `docs/security/`). It does not and cannot review
  the backend, indexer, or frontend, none of which exist yet
  (`docs/architecture.md`).

## 1. Line-item review checklist

### 1.1 Initialization and admin

- [ ] No entrypoint other than `__constructor` can set `Admin` or
      `Attestor` for the first time.
- [ ] `__constructor` requires `admin.require_auth()`.
- [ ] No function can reassign `Admin` post-deployment (grep `src/lib.rs`
      for `DataKey::Admin` writes — there must be exactly one, in
      `__constructor`).
- [ ] `set_attestor` requires `admin.require_auth()` **and** an explicit
      check that the caller-supplied `admin` equals the stored one.
- [ ] Evidence: `tests/constructor_auth.rs`,
      `tests/security_matrix.rs::admin_is_immutable_across_every_mutating_call`,
      `tests/security_matrix.rs::set_attestor_*`.

### 1.2 Two-party linking

- [ ] `link_github` requires both `wallet.require_auth()` and
      `attestor.require_auth()`.
- [ ] `link_github` checks the caller-supplied `attestor` against the
      stored attestor (a valid signature from the wrong address must
      still fail).
- [ ] `unlink_github` has the same two-party requirement, checked
      against the *currently linked* wallet, not any wallet.
- [ ] Both directions of the link (`WalletLink`, `GithubLink`) are
      written together in `link_github` and removed together in
      `unlink_github` — no code path writes one without the other.
- [ ] `WalletAlreadyLinked` / `GithubAlreadyLinked` are checked before
      any write in `link_github`.
- [ ] `LinkNotFound` in `unlink_github` requires **both** directions to
      independently agree with the supplied pair, not just one.
- [ ] Evidence: `src/test.rs`, `tests/security_matrix.rs` §1–2, §9–10,
      `tests/state_machine.rs`.

### 1.3 Attestation submission

- [ ] `submit_attestation` requires `attestor.require_auth()` and checks
      it against the stored attestor.
- [ ] `complexity` is validated against the exact allowed set before any
      storage read or write.
- [ ] The credited wallet is resolved from `GithubLink`, never accepted
      as a caller-supplied parameter (ADR 0001).
- [ ] `SeenPr` is checked before write and set atomically with the new
      attestation.
- [ ] `timestamp` is set from `env.ledger().timestamp()`, never from a
      caller-supplied value.
- [ ] Evidence: `src/test.rs`, `tests/security_matrix.rs` §3, §6, §11–12,
      `tests/boundary_and_events.rs`, `tests/state_machine.rs`.

### 1.4 Atomicity and error handling

- [ ] Every fallible mutating function returns `Result<_, Error>` and
      every error variant is reachable and tested (§ below).
- [ ] A rejected call leaves no partial write anywhere — not just at the
      call's own target key.
- [ ] `get_reputation_score` cannot panic (uses `saturating_add`).
- [ ] The release profile sets `overflow-checks = true` (`Cargo.toml`).
- [ ] Evidence: `tests/security_matrix.rs::invalid_complexity_leaves_no_partial_record_anywhere`,
      `tests/state_machine.rs` (every rejected-action branch asserts
      `assert_model_unchanged`).

### 1.5 TTL / storage durability

- [ ] Every mutating call extends the TTL of every persistent entry it
      touches, plus the instance.
- [ ] `bump_wallet_ttl` refreshes the wallet link, the GitHub link, the
      history vector, **and every `SeenPr` marker referenced by that
      history** — not just the vector itself.
- [ ] `bump_wallet_ttl` requires no authorization and changes no
      observable data.
- [ ] TTL extend-to / threshold constants are clamped to
      `env.storage().max_ttl()`.
- [ ] Evidence: `src/test.rs` §7, `tests/ttl_replay.rs` (all).

### 1.6 Events

- [ ] Every successful mutating call emits exactly the event
      `docs/integration/event-indexer-v1.md` documents for it, with the
      documented topics and data fields.
- [ ] No event is emitted on a failed call.
- [ ] `bump_wallet_ttl` emits no event (documented as silent).
- [ ] Evidence: `tests/boundary_and_events.rs` §5–6.

### 1.7 Identifiers and cross-implementation consistency

- [ ] `github_id_hash` and `pr_hash` are treated as fully opaque
      `BytesN<32>` everywhere in the contract — no parsing, no implicit
      format assumption.
- [ ] The pinned vectors in `identifier-spec-v1.md` /
      `sdk/typescript/src/identifiers.test.ts` are reproduced
      byte-for-byte by the Soroban host's own `sha256` primitive.
- [ ] The TypeScript SDK's `ProofOwlErrorCode` enum values match the
      Rust `#[contracterror] enum Error` discriminants, and both match
      the WASM-generated bindings (`errors.test.ts`, the
      `sdk-bindings-drift` CI job).
- [ ] Evidence: `tests/sdk_vectors.rs`, `sdk/typescript/src/errors.test.ts`,
      `tests/security_matrix.rs::error_code_discriminants_match_the_published_abi`.

### 1.8 Resource / scalability

- [ ] The per-wallet history storage design's growth characteristics are
      measured, not assumed, and the exact failure mode (not just
      "gets expensive") is identified.
- [ ] The measured hard ceiling and its implications are documented with
      an explicit verdict.
- [ ] Evidence: `docs/security/resource-profile-v1.md`,
      `tests/resource_profile.rs`.

### 1.9 Supply chain

- [ ] `cargo deny check` (bans/licenses/sources) passes with only
      dated, reasoned, tracked exceptions.
- [ ] `cargo deny check advisories` / `cargo audit` pass with only
      dated, reasoned, tracked exceptions (see §5, dependency-review
      procedure).
- [ ] `Cargo.lock` is committed and the release build is reproducible
      from it.
- [ ] `sdk/typescript` dependencies install from a committed lockfile
      (`npm ci`) in CI.
- [ ] Evidence: `deny.toml`, `.github/workflows/ci.yml` `supply-chain`
      job, this phase's `make audit-ready` run (recorded in the Phase 4
      completion report).

### 1.10 Documentation accuracy

- [ ] `README.md`'s API table, error table, and trust-boundary summary
      match `src/lib.rs`.
- [ ] `SECURITY.md` reflects the current auth model, TTL policy, and
      known limitations.
- [ ] `PRODUCTION_READINESS.md` gate statuses are current and every
      `NOT MET` is honestly still not met (no aspirational statuses).
- [ ] No document claims an audit, bug bounty, response SLA, or
      production usage that does not exist.
- [ ] Every ADR that a later document supersedes or extends says so.

## 2. Release-blocker definition

An item is a **hard release blocker** for a given target (testnet
alpha / bounded v1 production / mainnet — see
`PRODUCTION_READINESS.md`'s gates) if and only if it is:

- a **Critical** or **High** severity finding (per the rubric in §3)
  that is reachable **without** assuming a compromised attestor or
  admin, for that target; or
- any finding, at any severity, that contradicts a claim this
  repository makes in `README.md`, `SECURITY.md`, or
  `PRODUCTION_READINESS.md` (a false claim is always a blocker,
  regardless of the underlying severity, because it is also a trust
  violation); or
- a missing or failing item in §1.1–§1.6 (core authorization, atomicity,
  TTL, and event-correctness invariants) — these are the properties the
  rest of the documented trust model is built on; or
- for the **mainnet** target specifically, any `NOT MET` row under
  `PRODUCTION_READINESS.md` Gate 6.

A finding that requires a compromised or complicit attestor/admin to
reach (High severity per the threat model's own rubric, since that is a
documented, accepted trust boundary) is **not** automatically a release
blocker for testnet alpha or a bounded v1 pilot, but MUST be listed in
`docs/security/known-risks-v1.md` and MUST be a hard blocker for
mainnet (`PRODUCTION_READINESS.md` 6.3, 6.4).

## 3. Severity rubric

Shared verbatim with `docs/security/threat-model-v1.md`:

| Severity | Meaning |
|---|---|
| **Critical** | Direct loss/theft of funds or reputation, or total bypass of a stated invariant, reachable by an unprivileged attacker. |
| **High** | Same impact, but requires a privileged party (attestor/admin) to be compromised or complicit, or requires an off-chain component that doesn't exist yet to be built wrong. |
| **Medium** | Degrades availability, increases cost, or corrupts derived (off-chain) state without corrupting on-chain truth. |
| **Low** | Theoretical, requires an already-broken assumption elsewhere (host bug, hash collision), or is cosmetic. |
| **Accepted** | A known, documented design trade-off. Not a bug; tracked so it isn't rediscovered and "fixed" into a worse design. |

## 4. Evidence required before mainnet

Beyond `PRODUCTION_READINESS.md` Gate 6 in full, specifically from a
security-testing standpoint:

- [ ] This checklist completed against the exact commit being
      considered for mainnet, by someone who did not write the change
      being reviewed.
- [ ] `docs/security/resource-profile-v1.md`'s verdict is either
      "acceptable" for the intended production scope with an enforced
      bound, or the storage redesign it describes has shipped.
- [ ] An independent third-party audit has been performed and its
      findings resolved or explicitly accepted in writing by the
      project owner (`PRODUCTION_READINESS.md` 6.2) — internal review,
      however thorough, is not a substitute.
- [ ] The attestor is a multisig/threshold scheme, exercised via
      `set_attestor`, not a single key (`PRODUCTION_READINESS.md` 6.3).
- [ ] The admin key is in a hardware signer with a documented custody
      policy (`PRODUCTION_READINESS.md` 6.4) — required because admin
      key loss is **permanent and irreversible** by design (no
      `set_admin`; `docs/security/threat-model-v1.md` §13).
- [ ] Immutability (no upgrade path) is accepted in writing by the
      project owner (`PRODUCTION_READINESS.md` 6.8).

## 5. Test coverage map

| Area | Primary test file(s) |
|---|---|
| Constructor / deploy-time auth | `tests/constructor_auth.rs` |
| Baseline auth, linking, attestation, TTL | `src/test.rs` |
| Authorization matrix, admin immutability, error-code stability | `tests/security_matrix.rs` |
| Long-sequence state-machine invariants | `tests/state_machine.rs` |
| TTL refresh correctness, replay resistance | `tests/ttl_replay.rs` |
| Boundary values, event emission | `tests/boundary_and_events.rs` |
| Cross-language hash vector agreement | `tests/sdk_vectors.rs` |
| Resource growth and hard ceiling | `tests/resource_profile.rs` (diagnostic; `make resource-profile`) |
| SDK identifier helpers | `sdk/typescript/src/identifiers.test.ts` |
| SDK error-code sync with generated bindings | `sdk/typescript/src/errors.test.ts` |
| Generated-binding drift vs. built WASM | `.github/workflows/ci.yml` `sdk-bindings-drift` job |

## 6. Dependency-review procedure

Run on every dependency bump (`Cargo.toml`, `Cargo.lock`,
`sdk/typescript/package.json` / `package-lock.json`) and at minimum
monthly otherwise:

1. `cargo deny check` — must pass with zero new, undocumented findings.
   A new finding is either fixed (upgrade/replace the dependency) or
   added to `deny.toml`'s `ignore` list with a comment naming the
   advisory/lint id, the reason, and the date — mirroring the existing
   `RUSTSEC-2024-0436` entry's format exactly.
2. `cargo audit --ignore RUSTSEC-2024-0436` (or the current exception
   list) — same treatment for any new advisory.
3. For the TypeScript SDK: review `npm ls` for unexpected new
   transitive dependencies after any `package-lock.json` change; there
   is currently no `npm audit` gate in CI (`ci.yml` installs with
   `--no-audit`) — this is a tracked, open gap (see
   `docs/security/known-risks-v1.md`), so a manual `npm audit` review is
   the interim mitigation until that gap is closed.
4. Re-verify the pinned tool versions (`cargo-deny`, `cargo-audit`,
   Stellar CLI) in `Makefile` and `.github/workflows/ci.yml` are still
   the versions actually validated locally — they must never drift
   apart silently.
5. Record the date and outcome of this review in the release record
   (`docs/RELEASE_CHECKLIST.md`).

## 7. External-audit handoff checklist

Before sending this repository to a third-party auditor:

- [ ] This checklist and `docs/security/known-risks-v1.md` are current
      and handed over as-is — do not curate away known limitations; an
      auditor working from an incomplete picture wastes time
      rediscovering documented trade-offs instead of finding new ones.
- [ ] `docs/security/threat-model-v1.md` and
      `docs/security/resource-profile-v1.md` are handed over — an
      auditor should spend their time on what these documents could not
      cover (cryptographic review of the SDK's identifier
      implementation independent of the contract, deeper economic/game-
      theoretic analysis of attestor incentives, a security review of
      the backend once it exists), not re-deriving what a targeted
      internal pass already found.
- [ ] The exact commit SHA, `Cargo.lock`, and built WASM sha256 being
      audited are recorded and immutable for the audit's duration — no
      moving target.
- [ ] `PRODUCTION_READINESS.md`'s current gate statuses are shared
      unedited, including every `NOT MET`.
- [ ] Scope is stated explicitly: `src/`, `tests/`, `sdk/typescript/`,
      and the CI/release workflows are in scope; the backend, indexer,
      and frontend are out of scope because they do not exist yet
      (matches `SECURITY.md`'s stated vulnerability-reporting scope).
- [ ] A named point of contact and expected response time for
      audit-in-progress questions is agreed before the audit starts.
- [ ] Post-audit: every finding gets a severity (this document's rubric,
      for consistency), a fix-or-accept decision recorded in writing,
      and, if fixed, a regression test added under `tests/`.
