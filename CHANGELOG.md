# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
See [`docs/RELEASE_POLICY.md`](docs/RELEASE_POLICY.md) for what counts as a
breaking change for an on-chain contract.

The project is **pre-1.0**: while the major version is `0`, any release
may change contract behaviour, storage layout, or interfaces. Nothing in
this project has been released to a package registry, tagged, or audited.
A first instance has been deployed to **Stellar testnet** (see the
Testnet section below); no mainnet deployment exists.

## [Unreleased]

### v0.2 paginated-storage redesign — local candidate, not deployed
- **Breaking storage/API redesign**
  (`docs/adr/0004-paginated-attestation-storage.md`) fixing Phase 4's
  measured hard ceiling (v0.1: 286 attestations succeed, the 287th
  fails outright — `docs/security/resource-profile-v1.md`, kept
  unedited). Replaces the single `Vec<Attestation>`-per-wallet entry
  with one persistent entry per attestation
  (`AttestationEntry(wallet, seq)`, zero-based), an `AttestationCount`
  counter, and a running `ReputationScore` counter updated atomically.
- **Removed**: `get_attestations(wallet) -> Vec<Attestation>`
  (unbounded) and `bump_wallet_ttl(wallet)` (unbounded).
- **Added**: `get_attestation_count`, `get_attestation`,
  `get_attestations_page` (bounded reads, `MAX_PAGE_SIZE = 50`);
  `bump_wallet_core_ttl` (O(1)), `bump_attestations_ttl_page` (bounded
  paginated TTL sweep).
- **New error codes** `InvalidPageLimit` (10), `PageLimitExceeded` (11),
  `SequenceOutOfRange` (12), `PageStartOutOfRange` (13) — appended,
  codes 1–9 unchanged.
- **`AttestationRecorded` event** gains a `sequence: u32` field
  (additive).
- **Measured evidence**: `docs/security/resource-profile-v2.md` — a
  wallet grown to 1000 attestations (3.5x the v0.1 ceiling) with no
  failure and no entry approaching Soroban's 65,536-byte per-entry
  limit; reads and TTL maintenance are genuinely flat regardless of
  history size; one open item (a small, not-fully-root-caused write-cost
  growth with total contract entry count in the local test harness) is
  documented, not hidden.
- **New CI/tooling gate**: `scripts/check_bounded_storage.sh` /
  `make check-bounded-storage` (part of `make check`) — a static,
  deterministic guard that fails if the v0.1 unbounded pattern or its
  function names ever reappear in `src/lib.rs`.
- **TypeScript SDK updated for v0.2**: bindings regenerated from the
  v0.2 WASM; `client.ts` gains `getAttestationCount` / `getAttestation`
  / `getAttestationsPage` and `MAX_PAGE_SIZE`, drops `getAttestations`;
  `prepareBumpWalletTtl` replaced by `prepareBumpWalletCoreTtl` /
  `prepareBumpAttestationsTtlPage`; `errors.ts` recognizes the bare
  error-name form a `Result`-returning read call throws (new for the
  two new Result-returning reads) in addition to the existing
  `Error(Contract, #N)` form. The opt-in testnet integration test no
  longer defaults to the v0.1 testnet alpha instance — it requires an
  explicit v0.2 contract id (none exists yet) and is otherwise always
  skipped.
- **New/updated integration docs**: `contract-api-v2.md`,
  `attestor-protocol-v2.md`, `event-indexer-v2.md`; v0.1 equivalents
  marked superseded and kept as the historical record of the deployed
  testnet alpha instance; `identifier-spec-v1.md` unchanged (hash
  derivation did not change) with a note confirming it still applies.
- **New**: `docs/migrations/v0.1-to-v0.2.md` — states plainly that
  v0.1 testnet alpha data does not migrate, no mainnet state exists to
  migrate, v0.2 needs a new contract ID when eventually deployed, and
  v0.1 remains historical test evidence, not an integration target.
- **`docs/security/known-risks-v1.md` R1** updated to "resolved in the
  local v0.2 candidate, pending validation and a new testnet
  deployment" — the original v0.1 measurement is kept, not erased.
- Crate version bumped to `0.2.0` per `docs/RELEASE_POLICY.md`'s
  breaking-change rule (no tag, no release cut — see that policy for
  what a real release requires).
- **No v0.2 instance has been deployed to any network.** This is a
  local candidate only; deploying it requires a separate, explicit
  approval not given as part of this change. No push, tag, release,
  deployment, or testnet transaction occurred.

### Adversarial and security testing (Phase 4)
- **Formal threat model**
  (`docs/security/threat-model-v1.md`) covering all 16 required threat
  categories (malicious wallet holder, identity squatting, compromised
  attestor/admin, hostile backend input, replayed attestation, bad
  indexer, RPC/stale-event risk, TTL expiration, front-running, DoS /
  resource exhaustion, hash-collision / canonicalization disagreement,
  key loss, supply-chain compromise, protocol downgrade), each with
  attacker capability, mitigation, residual risk, and severity. States
  plainly that the contract is not trustless — the attestor is a
  documented, load-bearing trust anchor.
- **New adversarial test suites** (`tests/`, all deterministic and
  offline): `security_matrix.rs` (full auth-failure matrix, admin
  immutability, error-code discriminant stability), `state_machine.rs`
  (fixed-seed, model-based long-sequence invariant testing),
  `ttl_replay.rs` (TTL refresh correctness at realistic scale,
  permissionless-keep-alive purity, duplicate-PR survival across
  repeated keep-alive cycles), `boundary_and_events.rs` (boundary/
  malformed inputs, exact event-emission correctness),
  `sdk_vectors.rs` (the Soroban host's own SHA-256 reproduces the
  TypeScript SDK's pinned canonical-hash vectors byte-for-byte).
- **Measured resource/scalability profile**
  (`docs/security/resource-profile-v1.md`,
  `tests/resource_profile.rs`): deploys the real release WASM and uses
  the SDK's own `cost_estimate()` instrumentation. Finding: a single
  wallet's attestation history has a **hard, measured ceiling** — 286
  attestations succeed, the 287th `submit_attestation` call fails
  outright (the entry exceeds Soroban's 65,536-byte per-contract-data-
  entry limit), after which that wallet can never receive another
  attestation or TTL refresh. Verdict: acceptable for testnet alpha and
  an explicitly bounded pilot; **requires the scoped (not implemented)
  paginated-storage redesign before mainnet or unbounded production
  scope.**
- **Security review package**: `docs/security/security-review-checklist-v1.md`
  (line-item checklist, severity rubric, release-blocker definition,
  dependency-review procedure, audit-handoff checklist) and
  `docs/security/known-risks-v1.md` (ranked, honest list of open risks
  and accepted limitations). `SECURITY.md`, `PRODUCTION_READINESS.md`,
  `README.md`, and ADRs 0001–0003 now link this material.
- **CI/tooling**: `make security-test`, `make resource-profile`,
  `make audit-ready`; CI gains an explicit "Adversarial and security
  test suite" step and a scheduled/manual `resource-profile` job. No
  existing check was weakened.
- The contract itself was **not modified** in this phase — testing
  found one material scalability finding (above), documented per the
  phase's rules of engagement rather than fixed reactively.

### Integration contract (Phase 3)
- **Versioned integration spec** under `docs/integration/`:
  `contract-api-v1.md` (every function — params, auth, errors, events,
  TTL effects; the two-party authorization rule), `identifier-spec-v1.md`
  (canonical `github_id_hash` from the immutable numeric user id, and
  `pr_hash`, with normalization / rejection rules and pinned vectors),
  `attestor-protocol-v1.md` (what the backend must verify before using
  the attestor key), `event-indexer-v1.md` (event topics/payloads,
  ordering, idempotency, `(network, contractId)` partitioning, TTL
  monitoring), and `sequence-diagrams.md`.
- **TypeScript SDK** at `sdk/typescript/` (`@proofowl/contract-sdk`):
  generated bindings from `stellar contract bindings typescript` (kept
  in `src/generated/`, regenerated via `npm run generate`, drift-checked
  in CI); a read-only `createReadClient` built with no signer;
  `prepare*` helpers returning **unsigned** transactions (two-party
  helpers report which addresses still must sign); canonical
  `hashGitHubUserIdV1` / `normalizeGitHubPullRequest` /
  `hashGitHubPullRequestV1` with unit-pinned vectors; and an opt-in
  read-only testnet integration test. The SDK never signs, submits, or
  reads a keystore.
- **Tooling**: `make sdk-install` / `sdk-generate` / `sdk-test` /
  `integration-check` / `sdk-integration-testnet`; CI gains a Node-only
  `sdk` job and a `sdk-bindings-drift` job. `make check` stays
  Rust-only; the Rust / WASM / supply-chain / testnet gates are
  unchanged.
- The contract itself was **not modified** in this phase.

### Testnet
- **Testnet alpha deployed** (2026-09-01, contract source `d030908`).
  Contract ID `CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6`
  on Stellar testnet. On-chain WASM hash
  `d694e0ad3193e3c2782f9c92d9e88ce6a2f4faef545f9df434b01b41ef96dbf1`
  matches the local release build. A seven-step end-to-end smoke test
  (two-party link, attestation, reads, invalid-complexity rejection,
  duplicate-PR rejection, two-party unlink, reputation retained after
  unlink) passed against the live instance. Full public evidence:
  [`docs/testnet/phase2-alpha.md`](docs/testnet/phase2-alpha.md).
  This is testnet only — not an audit, not a mainnet-readiness claim.
- Testnet helper scripts now verify the network via a live `getNetwork`
  call (mainnet passphrase positively refused) and deploy with
  `--optimize=false` so on-chain bytes match the recorded hash.
- Corrected the two-party CLI signing flow for `link_github` /
  `unlink_github` (`--source <wallet> --auto-sign`, no `--sign-with-key`)
  after a live `TxBadAuth`; documented in the operations guide §7.

### Added
- Deploy-time `__constructor(admin, attestor)` — configuration is bound
  to the deployment transaction; there is no `init` entrypoint.
- Two-party `link_github(wallet, attestor, github_id_hash)` and
  `unlink_github(...)` — both the wallet and the trusted attestor must
  authorize. `unlink_github` gives a two-party recovery / relink path.
- `submit_attestation` stores `repo` and `pr_number` so an indexer can
  reconstruct the pull-request URL; the on-chain `timestamp` is the
  ledger time, not a caller-supplied value.
- Permissionless `bump_wallet_ttl(wallet)` keep-alive that also refreshes
  every `SeenPr` de-duplication marker in a wallet's history.
- Read endpoints: `get_attestations`, `get_reputation_score`,
  `get_wallet_for_github`, `get_github_for_wallet`, `get_admin`,
  `get_attestor`.
- ADRs 0001–0003; `SECURITY.md` trust-model and TTL documentation.
- Integration test that drives the real deployer/auth path against the
  compiled WASM.
- Release engineering: `Makefile` quality gate, `scripts/` testnet
  helpers, `.env.example`, operations guide, issue/PR templates,
  `CODEOWNERS` placeholder, supply-chain CI (cargo-deny / cargo-audit),
  a manual-only testnet release workflow skeleton, this changelog,
  `PRODUCTION_READINESS.md`, and `docs/architecture.md`.

### Changed
- `complexity` is validated to `{0, 100, 150, 200}`; anything else is
  rejected with `InvalidComplexity`.
- Reputation scoring uses `saturating_add` and treats `complexity == 0`
  as a flat base score (50).
- Every persistent record and the instance have their TTL extended on
  every mutating call; policy documented in `SECURITY.md` §5.
- Build target is `wasm32v1-none` (Rust 1.84+); `wasm32-unknown-unknown`
  is no longer supported by `soroban-sdk` 27 on Rust ≥ 1.82.
- Events use the `#[contractevent]` macro.
- `Cargo.lock` is committed for reproducible WASM builds.

### Security
- Global, permanent PR de-duplication via `SeenPr(pr_hash)`; the
  keep-alive covers every marker so a spent PR can never become
  re-submittable through TTL expiry.
- No admin or attestor function can create, move, or delete a
  wallet ↔ GitHub link, edit an attestation, or change a score.
- Known deferred limitations (single trusted attestor, lost-key
  recovery, history migration, per-wallet attestation vector scaling)
  are enumerated in `SECURITY.md` §7.

[Unreleased]: https://github.com/Proofowl/proofowl-contracts/commits/main
