# Changelog

All notable changes to this project are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project aims to follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).
See [`docs/RELEASE_POLICY.md`](docs/RELEASE_POLICY.md) for what counts as a
breaking change for an on-chain contract.

The project is **pre-1.0**: while the major version is `0`, any release
may change contract behaviour, storage layout, or interfaces. Nothing in
this project has been released, tagged, deployed, or audited yet.

## [Unreleased]

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
