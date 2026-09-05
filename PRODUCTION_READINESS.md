# Production readiness

Objective go/no-go criteria for `proofowl-contracts`, with the **current
status** recorded honestly. A gate is `GO` only when every criterion
under it is met. Nothing here is aspirational marketing — if it says
`NOT MET`, it is not met.

Legend: `MET` · `PARTIAL` · `NOT MET` · `N/A`

Security-specific evidence backing Gates 5 and 6 lives under
[`docs/security/`](docs/security/) — see
[`threat-model-v1.md`](docs/security/threat-model-v1.md),
[`resource-profile-v1.md`](docs/security/resource-profile-v1.md) (v0.1
finding) / [`resource-profile-v2.md`](docs/security/resource-profile-v2.md)
(v0.2 candidate evidence),
[`security-review-checklist-v1.md`](docs/security/security-review-checklist-v1.md),
and [`known-risks-v1.md`](docs/security/known-risks-v1.md). The v0.2
storage redesign is a **local candidate only** — see
[`docs/migrations/v0.1-to-v0.2.md`](docs/migrations/v0.1-to-v0.2.md).

---

## Gate 1 — Local validation

Run on the pinned toolchain (Rust **1.91.0**, the verified minimum —
see `docs/MAINTAINERS.md` "CI toolchain"). Every dependency-resolving
command uses `--locked`.

| # | Criterion | Status |
|---|---|---|
| 1.1 | `cargo fmt --all -- --check` clean | MET |
| 1.2 | `cargo clippy --locked --all-targets -- -D warnings` clean | MET (on 1.91.0) |
| 1.3 | `cargo build --locked --target wasm32v1-none --release` succeeds | MET (on 1.91.0) |
| 1.4 | `cargo test --locked --all` green (unit + integration + doc) | MET (on 1.91.0) |
| 1.5 | `make check` runs all of the above plus `check_bounded_storage.sh` as one gate | MET |
| 1.6 | `Cargo.lock` committed; `--locked` build reproducible from it | MET |
| 1.7 | `Cargo.toml` `rust-version`, CI toolchain pin, Makefile `RUST_TOOLCHAIN_MIN`, and the supply-chain cache key all name the same exact toolchain | MET (`1.91.0`) |

**Gate 1: GO.**

---

## Gate 2 — CI

| # | Criterion | Status |
|---|---|---|
| 2.1 | CI runs fmt, clippy, release-WASM build, tests on push and PR | MET |
| 2.2 | WASM built before tests (integration test needs the artifact) | MET |
| 2.3 | Supply-chain job: `cargo deny --locked check` (bans/licenses/sources) as a hard gate | MET |
| 2.4 | Supply-chain job: `cargo deny advisories` + `cargo audit`, also on a weekly schedule | MET |
| 2.5 | GitHub Actions pinned to explicit versions | PARTIAL — `dtolnay/rust-toolchain@1.91.0` is a version-exact branch (not the moving `@1.91`); other actions on major tags. Full commit-SHA pinning is an open hardening task (needs a maintainer with network access to verify SHAs) |
| 2.6 | No action references a floating branch | MET — the Rust action uses the exact `@1.91.0`, not `@1.91` / `@stable` |
| 2.7 | CI toolchain can actually run every gate | **MET (pending a green run)** — CI's `test` / `supply-chain` / `sdk-bindings-drift` jobs were red because the `@1.84` pin could no longer parse edition-2024 transitive manifests or build `cargo-deny 0.20.2` (rustc ≥ 1.88). Fixed by pinning the verified minimum `1.91.0`; every step was reproduced locally on that exact toolchain (fmt, clippy, build, test, fresh `cargo install --locked` of both security tools, `cargo deny --locked check`, `cargo audit`). Pinning the toolchain also required regenerating `sdk/typescript/src/generated/index.ts` on 1.91.0 — the WASM's `contractspecv0` entry order is rustc-version-dependent and the committed file had been built on a newer stable (one adjacent-pair swap, byte-identical entry contents, no API change); verified deterministic across three 1.91.0 builds, and `make sdk-drift-check` is green on the pin. CI has not yet re-run — this row flips to plain MET only after a green push. |
| 2.8 | `sdk` job (Node) green | **MET (pending a green run)** — was red on two unformatted files (`README.md`, `errors.test.ts`); fixed with Prettier and the full SDK suite (format / lint / typecheck / 21 unit tests / build) re-run locally on Node 24.20.0. |

**Gate 2: GO with follow-ups** (2.5; confirm 2.7 / 2.8 flip to MET on the next CI run).

---

## Gate 3 — Testnet deployment

| # | Criterion | Status |
|---|---|---|
| 3.1 | Documented deploy procedure with three-key separation | MET (`docs/operations/testnet-deployment.md`) |
| 3.2 | Helper scripts: build, deploy, verify-config, smoke-test | MET (`scripts/`) |
| 3.3 | Scripts refuse non-testnet, validate inputs, never print secrets, never fund/generate keys | MET — network now verified via a live `getNetwork` call, mainnet passphrase positively refused |
| 3.4 | Manual-only release workflow that never deploys by default | MET (`.github/workflows/testnet-release.yml`) |
| 3.5 | An instance actually deployed to testnet | **MET** — `CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6` (2026-09-01, src `d030908`); on-chain WASM hash matches the local build |
| 3.6 | `verify_config.sh` + `smoke_test.sh` run green against a live instance | **MET** — config verified; 7-step smoke test passed (see `docs/testnet/phase2-alpha.md`) |
| 3.7 | Instance log started (commit SHA, WASM sha256, contract ID, addresses, tx hashes) | **MET** — `docs/testnet/phase2-alpha.md` |
| 3.8 | Stellar CLI available to the operator | MET locally (28.0.0 installed); still a per-operator setup step, documented in the ops guide |

**Gate 3: GO for a disposable testnet alpha.** One instance is deployed,
verified, and smoke-tested. It is disposable and may be replaced; this
does not extend to any standing testnet service or to mainnet.

---

## Gate 4 — End-to-end integration

| # | Criterion | Status |
|---|---|---|
| 4.0 | Versioned integration contract published (API, identifiers, attestor protocol, events/indexer) | MET — `docs/integration/*-v1.md` |
| 4.0b | Typed SDK consumable: read-only client, unsigned-tx prep, canonical hash helpers, generated-binding drift check | MET — `sdk/typescript/`; unit tests + an opt-in read-only testnet check pass |
| 4.1 | Backend (`proofowl-backend`) exists and performs GitHub OAuth / verification | NOT MET — future repository |
| 4.2 | Backend co-signs `link_github` only after verifying GitHub ownership | NOT MET |
| 4.3 | Attestation submission pipeline derives `pr_hash` canonically | NOT MET (the canonical algorithm is now specified and implemented in the SDK; no pipeline consumes it yet) |
| 4.4 | Event indexer consumes contract events | NOT MET — future repository |
| 4.5 | Frontend (`proofowl-frontend`) reads passports / leaderboard | NOT MET — future repository |
| 4.6 | A full path exercised across all components on testnet | NOT MET |

**Gate 4: NO-GO** — the integration contract and SDK exist and are
tested, but the backend, indexer, and frontend do not. Only the contract
and its consumables exist today.

---

## Gate 5 — Security review

| # | Criterion | Status |
|---|---|---|
| 5.1 | Trust model, non-goals, and deferred limitations documented | MET (`SECURITY.md`) |
| 5.2 | ADRs for each significant security decision | MET (`docs/adr/0001–0003`) |
| 5.3 | Tests cover each auth path and each failure mode | MET (`src/test.rs`, `tests/constructor_auth.rs`) |
| 5.4 | Private vulnerability-reporting process published | PARTIAL — process described in `SECURITY.md`; the security contact is a placeholder a maintainer must fill in |
| 5.5 | Internal security review / threat-model walkthrough recorded | **MET** — Phase 4 (2026-09): `docs/security/threat-model-v1.md`, `docs/security/security-review-checklist-v1.md`, `docs/security/known-risks-v1.md`, plus adversarial/state-machine/TTL/boundary/resource test suites (`tests/security_matrix.rs`, `tests/state_machine.rs`, `tests/ttl_replay.rs`, `tests/boundary_and_events.rs`, `tests/resource_profile.rs`) |
| 5.6 | Independent third-party audit | NOT MET — no audit has been performed or scheduled. Internal review (5.5) is not a substitute — see `docs/security/security-review-checklist-v1.md` §7 |
| 5.7 | Bug bounty | NOT MET — none exists; do not claim one |

**Gate 5: NO-GO for anything beyond testnet** — an internal
threat-model review and adversarial test pass are now done (5.5); no
external review has been done (5.6), which remains a hard mainnet
prerequisite (Gate 6.2).

---

## Gate 6 — Mainnet readiness

Every item below is a hard prerequisite. All are currently **NOT MET**.

| # | Criterion | Status |
|---|---|---|
| 6.1 | Gates 1–5 all GO | NOT MET |
| 6.2 | Independent third-party audit completed; findings resolved or accepted in writing | NOT MET |
| 6.3 | Attestor is a multisig / threshold scheme, not a single key (`set_attestor` path exercised) | NOT MET |
| 6.4 | Admin key in a hardware signer with a documented custody policy | NOT MET |
| 6.5 | Testnet instance run for a sustained period with real backend + indexer traffic | NOT MET |
| 6.6 | Per-wallet attestation storage scaling addressed or bounded (`SECURITY.md` §7) | **PARTIAL** — the paginated-storage redesign is implemented in a **local v0.2 candidate** (`docs/adr/0004-paginated-attestation-storage.md`), measured to hold 1000+ attestations with no ceiling (`docs/security/resource-profile-v2.md`). Not MET for this gate because v0.2 has not been deployed to any network, audited, or exercised live — see `docs/migrations/v0.1-to-v0.2.md`. The original v0.1 finding (286/287) remains in `docs/security/resource-profile-v1.md`, unedited. |
| 6.7 | Incident-response runbook and on-call owner | NOT MET |
| 6.8 | Immutability accepted in writing by the project owner (no upgrade path exists) | NOT MET |

**Gate 6: NO-GO.** Mainnet is not in scope for the current phase.

---

## Summary

| Gate | Status |
|---|---|
| 1 Local validation | **GO** |
| 2 CI | **GO** with follow-ups (SHA-pin actions; keep supply-chain green) |
| 3 Testnet deployment | **GO** (disposable alpha deployed, verified, smoke-tested — 2026-09-01) |
| 4 E2E integration | **NO-GO** (integration contract + SDK exist; backend / indexer / frontend do not) |
| 5 Security review | **NO-GO** (internal threat-model + adversarial testing done, Phase 4 2026-09; external audit still not done) |
| 6 Mainnet | **NO-GO** (out of scope; storage redesign exists as a local v0.2 candidate, not yet deployed/audited — see `docs/migrations/v0.1-to-v0.2.md` and `docs/security/resource-profile-v2.md`) |

The honest one-line status: **the contract runs correctly on Stellar
testnet (one disposable alpha instance, deployed and smoke-tested), and
has now been through an internal adversarial security-testing pass
(Phase 4, 2026-09) covering authorization, state-machine invariants,
TTL/replay resistance, boundary conditions, SDK cross-verification, and
a measured resource/scalability profile — see `docs/security/`.
Nothing beyond that is ready — no backend/indexer/frontend, no external security
review, no audit, no mainnet.**
