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
