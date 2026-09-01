# Production readiness

Objective go/no-go criteria for `proofowl-contracts`, with the **current
status** recorded honestly. A gate is `GO` only when every criterion
under it is met. Nothing here is aspirational marketing — if it says
`NOT MET`, it is not met.

Legend: `MET` · `PARTIAL` · `NOT MET` · `N/A`

---

## Gate 1 — Local validation

| # | Criterion | Status |
|---|---|---|
| 1.1 | `cargo fmt --all -- --check` clean | MET |
| 1.2 | `cargo clippy --all-targets -- -D warnings` clean | MET |
| 1.3 | `cargo build --target wasm32v1-none --release` succeeds | MET |
| 1.4 | `cargo test --all` green (unit + integration + doc) | MET |
| 1.5 | `make check` runs all of the above as one gate | MET |
| 1.6 | `Cargo.lock` committed; build reproducible from it | MET |

**Gate 1: GO.**

---

## Gate 2 — CI

| # | Criterion | Status |
|---|---|---|
| 2.1 | CI runs fmt, clippy, release-WASM build, tests on push and PR | MET |
| 2.2 | WASM built before tests (integration test needs the artifact) | MET |
| 2.3 | Supply-chain job: `cargo deny` (bans/licenses/sources) as a hard gate | MET |
| 2.4 | Supply-chain job: `cargo deny advisories` + `cargo audit`, also on a weekly schedule | MET |
| 2.5 | GitHub Actions pinned to explicit versions | PARTIAL — pinned to major version tags; full commit-SHA pinning is an open hardening task (needs a maintainer with network access to verify SHAs) |
| 2.6 | No action references a floating branch | MET |
| 2.7 | Supply-chain checks validated in this environment | PARTIAL — `cargo deny` / `cargo audit` were run locally against `deny.toml`; see the report in the PR. CI installs the same pinned versions. |

**Gate 2: GO with follow-ups** (2.5, and keep 2.7 green in CI).

---

## Gate 3 — Testnet deployment

| # | Criterion | Status |
|---|---|---|
| 3.1 | Documented deploy procedure with three-key separation | MET (`docs/operations/testnet-deployment.md`) |
| 3.2 | Helper scripts: build, deploy, verify-config, smoke-test | MET (`scripts/`) |
| 3.3 | Scripts refuse non-testnet, validate inputs, never print secrets, never fund/generate keys | MET |
| 3.4 | Manual-only release workflow that never deploys by default | MET (`.github/workflows/testnet-release.yml`) |
| 3.5 | An instance actually deployed to testnet | NOT MET — nothing has been deployed |
| 3.6 | `verify_config.sh` + `smoke_test.sh` run green against a live instance | NOT MET |
| 3.7 | Instance log started (commit SHA, WASM sha256, contract ID, addresses) | NOT MET |
| 3.8 | Stellar CLI available to the operator | NOT MET in this environment — operator must install it |

**Gate 3: NO-GO** — procedure and tooling are ready; no deployment has happened.

---

## Gate 4 — End-to-end integration

| # | Criterion | Status |
|---|---|---|
| 4.1 | Backend (`proofowl-backend`) exists and performs GitHub OAuth / verification | NOT MET — future repository |
| 4.2 | Backend co-signs `link_github` only after verifying GitHub ownership | NOT MET |
| 4.3 | Attestation submission pipeline derives `pr_hash` canonically | NOT MET |
| 4.4 | Event indexer consumes contract events | NOT MET — future repository |
| 4.5 | Frontend (`proofowl-frontend`) reads passports / leaderboard | NOT MET — future repository |
| 4.6 | A full path exercised across all components on testnet | NOT MET |

**Gate 4: NO-GO** — only the contract exists today.

---

## Gate 5 — Security review

| # | Criterion | Status |
|---|---|---|
| 5.1 | Trust model, non-goals, and deferred limitations documented | MET (`SECURITY.md`) |
| 5.2 | ADRs for each significant security decision | MET (`docs/adr/0001–0003`) |
| 5.3 | Tests cover each auth path and each failure mode | MET (`src/test.rs`, `tests/constructor_auth.rs`) |
| 5.4 | Private vulnerability-reporting process published | PARTIAL — process described in `SECURITY.md`; the security contact is a placeholder a maintainer must fill in |
| 5.5 | Internal security review / threat-model walkthrough recorded | NOT MET |
| 5.6 | Independent third-party audit | NOT MET — no audit has been performed or scheduled |
| 5.7 | Bug bounty | NOT MET — none exists; do not claim one |

**Gate 5: NO-GO for anything beyond testnet** — documentation and tests
are solid; no internal or external review has been done.

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
| 6.6 | Per-wallet attestation storage scaling addressed or bounded (`SECURITY.md` §7) | NOT MET |
| 6.7 | Incident-response runbook and on-call owner | NOT MET |
| 6.8 | Immutability accepted in writing by the project owner (no upgrade path exists) | NOT MET |

**Gate 6: NO-GO.** Mainnet is not in scope for the current phase.

---

## Summary

| Gate | Status |
|---|---|
| 1 Local validation | **GO** |
| 2 CI | **GO** with follow-ups (SHA-pin actions; keep supply-chain green) |
| 3 Testnet deployment | **NO-GO** (ready to attempt; not yet done) |
| 4 E2E integration | **NO-GO** (dependent repositories do not exist) |
| 5 Security review | **NO-GO** (no internal or external review) |
| 6 Mainnet | **NO-GO** (out of scope) |

The honest one-line status: **the contract and its release engineering
are ready for a first testnet deployment; nothing beyond that is ready,
and nothing has been deployed or audited.**
