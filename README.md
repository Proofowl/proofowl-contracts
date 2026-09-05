# 🦉 ProofOwl — on-chain contributor reputation registry

[![CI](https://github.com/proofowl/proofowl-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/proofowl/proofowl-contracts/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Built on Stellar](https://img.shields.io/badge/Built%20on-Stellar-blueviolet)](https://stellar.org)

> A contributor's Stellar Wave track record shouldn't live only in one
> platform's private database. ProofOwl anchors verified, merged
> contributions on-chain — portable, checkable by anyone, and outliving
> any single program's backend.

**This repository's `src/` is the v0.2 candidate** (paginated
attestation storage — [ADR 0004](./docs/adr/0004-paginated-attestation-storage.md)).
**No v0.2 instance has been deployed to any network.** The only live
instance is the v0.1 testnet alpha listed under
[Deployed contracts](#deployed-contracts), which speaks the older,
unbounded-history ABI. See
[`docs/migrations/v0.1-to-v0.2.md`](./docs/migrations/v0.1-to-v0.2.md)
before integrating against either.

## The problem

Programs like [Drips Wave](https://drips.network/wave/stellar) track
points, complexity tiers, and contributor reviews entirely in their own
backend. That's fine for running a Wave cycle — but it means a
contributor's whole track record is untransferable and unverifiable by
anyone outside that one platform. If you want to point a grant committee,
a DAO, or another bounty platform at "here's my real, verified OSS
contribution history," there's currently nothing to point them at.

## The solution

This contract is a minimal, on-chain registry of two things:

1. **A two-party wallet ↔ GitHub identity link.** The contributor's
   wallet signs the linking call *and* a trusted attestor co-signs it.
   The wallet signature proves control of the Stellar key; the attestor
   co-signature attests that an off-chain GitHub OAuth / challenge flow
   proved the same person controls the GitHub account. **The contract
   itself cannot and does not verify GitHub** — see
   [Trust boundaries](#trust-boundaries).
2. **Verified attestations** — one entry per confirmed, merged
   contribution to a Stellar Wave-labeled issue, submitted by the
   trusted attestor service after independently checking GitHub's public
   API (see [proofowl-backend](https://github.com/proofowl/proofowl-backend)
   for exactly what gets checked before an attestation is submitted).

Anyone can then query a wallet's full, checkable history — every
attestation carries the `owner/repo` and PR number it came from, so it
links straight back to the merged pull request.

## Trust boundaries

Read this before touching the contract. Full detail in
[`SECURITY.md`](./SECURITY.md).

- **The contract cannot verify GitHub OAuth.** It has no network access.
  What it enforces is *procedure*: a link exists only if **both** the
  wallet and the trusted attestor signed. The attestor is trusted to
  co-sign only after the backend has run a real GitHub OAuth / challenge
  flow. This is a deliberate trust assumption, documented, not a gap we
  forgot to close. See
  [`docs/adr/0002-two-party-github-link.md`](./docs/adr/0002-two-party-github-link.md).
- **The attestor resolves the wallet, it never chooses it.**
  `submit_attestation` takes a hashed GitHub identity, not a wallet
  address — the contract looks up the wallet via the on-chain link. A
  compromised or careless attestor key can misreport *what* happened,
  but it can't redirect credit to a wallet the GitHub identity hasn't
  itself linked. See
  [`docs/adr/0001-attestor-resolves-via-github-link.md`](./docs/adr/0001-attestor-resolves-via-github-link.md).
- **Identity squatting is blocked by the co-signature.** A wallet cannot
  claim `hash("torvalds")` on its own; the attestor will not co-sign a
  link the OAuth flow did not back. Both directions of the link are also
  one-to-one and collision-checked.
- **The attestor key is a known, deliberate centralization point for
  v1.** `set_attestor` (admin-only) exists specifically so it can be
  rotated to a multisig or a threshold scheme later without a contract
  migration.
- **Complexity tiers are best-effort.** `submit_attestation` accepts
  only `0`, `100`, `150`, `200`; anything else is rejected with
  `InvalidComplexity`. `0` means the attestor confirmed the contribution
  happened but not its official Wave tier, and it scores at a flat base
  rate (50) rather than zero.

## Recovery

A mistaken link is **not** permanent. `unlink_github` is a two-party call
(the linked wallet **and** the attestor) that clears both directions of
the link so the identity can be re-linked. Already-earned attestation
history stays attached to the wallet that earned it, and a merged PR
stays globally spent forever. Recovering an identity whose wallet key is
*lost* is deliberately out of scope for the MVP — see `SECURITY.md` for
why a deferred mechanism is safer than a privileged override.

## Initialization

There is **no `init` function**. Configuration is set by the contract's
`__constructor`, which the host runs once, atomically, inside the deploy
operation itself — so there is no separate init call to front-run, and a
race to "initialize first" would only create a *different* contract
instance. The constructor also calls `admin.require_auth()`, so the
deploy transaction must be signed by the admin. See
[`docs/adr/0003-deploy-time-constructor-init.md`](./docs/adr/0003-deploy-time-constructor-init.md).

## Storage durability

Soroban archives a persistent entry once its TTL runs out. Every registry
record here — wallet links, GitHub links, PR-dedup markers, attestation
entries and counters, and the contract instance — has its TTL extended
on every write. Anyone can call `bump_wallet_core_ttl(wallet)` (O(1): the
link, counter, and score) plus `bump_attestations_ttl_page(wallet, start,
limit)` (bounded, one page of history at a time) to keep a passport warm
for free — together they refresh the wallet link, the GitHub link, the
attestation counter and score, every attestation entry, and every
`SeenPr` de-duplication marker, so a merged PR can never become
re-submittable through TTL expiry. Policy and constants are in
[`SECURITY.md`](./SECURITY.md#5-storage-durability-ttl-policy); the
bounded-TTL design is in
[ADR 0004](./docs/adr/0004-paginated-attestation-storage.md).

## Attestation storage — v0.2 (paginated, no ceiling)

v0.1 kept a wallet's attestations in one `Vec<Attestation>` under a
single key, which had a measured hard ceiling: **286 attestations
succeeded for one wallet, the 287th failed outright** (the entry
exceeded Soroban's per-contract-data-entry size limit) — see
[`docs/security/resource-profile-v1.md`](./docs/security/resource-profile-v1.md).
**v0.2 replaces this** with one persistent entry per attestation
(`get_attestation_count`, `get_attestation`, `get_attestations_page`)
plus a running reputation-score counter, so no single entry's size
depends on history length any more. Measured to hold 1000+ attestations
for one wallet with no failure and no entry approaching the size limit
— see [ADR 0004](./docs/adr/0004-paginated-attestation-storage.md) and
[`docs/security/resource-profile-v2.md`](./docs/security/resource-profile-v2.md).
**This is a local candidate; it has not been deployed, audited, or
exercised against a live network** — see
[`docs/migrations/v0.1-to-v0.2.md`](./docs/migrations/v0.1-to-v0.2.md).

## Quick start

### Prerequisites

- Rust **1.91+** (stable). CI pins the exact stable toolchain
  **`1.91.0`** — the verified minimum, driven by `soroban-sdk 27.0.6`'s
  declared `rust-version` (enforced by Cargo at build time). A newer
  stable also works locally; the pin is a floor. The Soroban wasm
  target is **`wasm32v1-none`**: `rustup target add wasm32v1-none`.
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli)
- **Node ≥ 22.6** + npm — only for `sdk/typescript/` work. CI uses
  Node 24 (last verified: Node 24.20.0 / npm 11.19.0).

### Build and test

`make check` runs the complete local quality gate (the same steps CI
runs):

```
make check
# = cargo fmt --all -- --check
#   cargo clippy --locked --all-targets -- -D warnings
#   cargo build --locked --target wasm32v1-none --release
#   cargo test --locked --all
#   scripts/check_bounded_storage.sh
```

Every dependency-resolving command passes `--locked`, so it builds the
exact committed `Cargo.lock` and fails loudly rather than silently
re-resolving. SDK checks (`sdk/typescript/`) are a separate gate:
`make integration-check` (needs Node).

`make help` lists every target. Supply-chain checks
(`make deny` / `make audit`) need network access and are a separate CI
job — see [Operations & release](#operations--release).

### Deploy (testnet)

**No v0.2 instance has been deployed anywhere — deploying this crate's
current `src/` requires a separate, explicit approval this repository
does not currently record.** The steps below describe the deploy
*mechanism* (unchanged since ADR 0003), not an authorization to run it.

Configuration is passed as constructor arguments to `stellar contract
deploy` itself — there is no follow-up `init` call. Sign with the admin
identity so the constructor's `admin.require_auth()` is satisfied:

```
stellar contract deploy \
  --wasm target/wasm32v1-none/release/proofowl_contracts.wasm \
  --source <admin-testnet-identity> \
  --network testnet \
  -- \
  --admin <ADMIN_ADDRESS> \
  --attestor <ATTESTOR_ADDRESS>
```

That single transaction deploys the instance and runs `__constructor`
atomically. Verify with `get_admin` / `get_attestor`. Full checklist in
[`SECURITY.md`](./SECURITY.md#6-first-testnet-deployment-checklist).

## Contract API (v0.2)

Full spec: [`docs/integration/contract-api-v2.md`](./docs/integration/contract-api-v2.md).

| Function | Caller(s) | Description |
|---|---|---|
| `__constructor(admin, attestor)` | the deployer, signed by `admin` | Runs once at deploy; sets admin + attestor. No separate `init`. |
| `set_attestor(admin, new_attestor)` | `admin` | Rotate the attestor key |
| `link_github(wallet, attestor, github_id_hash)` | **both** `wallet` and `attestor` | Two-party wallet ↔ GitHub link |
| `unlink_github(wallet, attestor, github_id_hash)` | **both** linked `wallet` and `attestor` | Clear a link for recovery / relink |
| `submit_attestation(attestor, github_id_hash, repo, pr_number, issue_id, complexity, pr_hash)` | `attestor` | Record a verified contribution; returns the credited wallet |
| `get_attestation_count(wallet)` | anyone (read) | How many attestations a wallet has |
| `get_attestation(wallet, sequence)` | anyone (read) | One attestation by zero-based index |
| `get_attestations_page(wallet, start, limit)` | anyone (read) | Bounded page of history, `limit` up to `MAX_PAGE_SIZE` (50) |
| `get_reputation_score(wallet)` | anyone (read) | Running score total — O(1) |
| `bump_wallet_core_ttl(wallet)` | anyone | O(1) keep-alive: wallet link, GitHub link, counter, score |
| `bump_attestations_ttl_page(wallet, start, limit)` | anyone | Bounded keep-alive for one page of history + its `SeenPr` markers |
| `get_wallet_for_github(github_id_hash)` | anyone (read) | Forward lookup: identity → wallet |
| `get_github_for_wallet(wallet)` | anyone (read) | Reverse lookup: wallet → identity hash |
| `get_admin()` / `get_attestor()` | anyone (read) | Current admin / attestor address |

**Removed in v0.2** (unbounded, replaced by the paginated functions
above): `get_attestations(wallet) -> Vec<Attestation>`,
`bump_wallet_ttl(wallet)`. See
[`docs/migrations/v0.1-to-v0.2.md`](./docs/migrations/v0.1-to-v0.2.md).

### What `pr_hash` is

`pr_hash` is the global duplicate-PR key. The backend MUST derive it
canonically:

```
pr_hash = SHA-256( lowercase("github.com/<owner>/<repo>/pull/<number>") )
```

no scheme, no trailing slash, no query string. `repo` (`<owner>/<repo>`)
and `pr_number` are stored in the clear alongside it so an indexer can
rebuild the URL; `pr_hash` itself is not reversible. The on-chain
`timestamp` on each attestation is the ledger time it was recorded, not a
value the attestor supplies.

## Errors

| Code | Name | Meaning |
|---|---|---|
| 1 | `AlreadyInitialized` | reserved (no `init` entrypoint; kept for numbering stability) |
| 2 | `NotInitialized` | instance config missing (e.g. archived); practically unreachable |
| 3 | `Unauthorized` | caller is not the stored admin / attestor |
| 4 | `WalletAlreadyLinked` | that wallet already has an identity |
| 5 | `GithubAlreadyLinked` | that identity hash is already claimed |
| 6 | `DuplicateAttestation` | that `pr_hash` was already recorded |
| 7 | `WalletNotLinked` | no wallet linked for that identity hash |
| 8 | `InvalidComplexity` | `complexity` not in `{0, 100, 150, 200}` |
| 9 | `LinkNotFound` | `unlink_github` target is not a consistent link |
| 10 | `InvalidPageLimit` | a paginated call's `limit` was `0` (v0.2) |
| 11 | `PageLimitExceeded` | a paginated call's `limit` exceeded `MAX_PAGE_SIZE` (50) (v0.2) |
| 12 | `SequenceOutOfRange` | `get_attestation`'s `sequence` is `>=` the wallet's count (v0.2) |
| 13 | `PageStartOutOfRange` | a paginated call's `start` is `>` the wallet's count (v0.2) |

Codes 1–9 are byte-for-byte unchanged from v0.1; 10–13 are v0.2
additions, appended not renumbered.

## Deployed contracts

| Network | Contract ID | Source | Version | Notes |
|---|---|---|---|---|
| **Testnet** | `CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6` | `d030908` | **v0.1** | Alpha, 2026-09-01. WASM `d694e0ad…ef96dbf1`. Verified + smoke-tested — [evidence](./docs/testnet/phase2-alpha.md). Disposable; may be replaced. Speaks the v0.1 ABI (unbounded `get_attestations` / `bump_wallet_ttl`) — see [`docs/migrations/v0.1-to-v0.2.md`](./docs/migrations/v0.1-to-v0.2.md). |
| Testnet | _not deployed_ | this repo's `src/` | **v0.2** | Local candidate only. Deploying it needs a separate, explicit approval not yet given. |
| Mainnet | _not deployed — out of scope_ | — | — | See [`PRODUCTION_READINESS.md`](./PRODUCTION_READINESS.md) |

## Repositories

- [`proofowl-contracts`](https://github.com/proofowl/proofowl-contracts) — this repo
- [`proofowl-backend`](https://github.com/proofowl/proofowl-backend) — GitHub verification, attestation submission, REST API *(planned; does not exist yet)*
- [`proofowl-frontend`](https://github.com/proofowl/proofowl-frontend) — passport pages, leaderboard, wallet linking UI *(planned; does not exist yet)*

See [`docs/architecture.md`](./docs/architecture.md) for how the pieces
fit together and which ones exist today.

## Operations & release

| Topic | Document |
|---|---|
| Local quality gate | `make check` — see `make help` |
| What "done" means before a tag or deploy | [`docs/RELEASE_CHECKLIST.md`](./docs/RELEASE_CHECKLIST.md) |
| Versioning & what is a breaking change on-chain | [`docs/RELEASE_POLICY.md`](./docs/RELEASE_POLICY.md) |
| Change history | [`CHANGELOG.md`](./CHANGELOG.md) |
| Go/no-go status by area | [`PRODUCTION_READINESS.md`](./PRODUCTION_READINESS.md) |
| Deploying to testnet (guide) | [`docs/operations/testnet-deployment.md`](./docs/operations/testnet-deployment.md) |
| Testnet helper scripts | [`scripts/`](./scripts) · config template [`.env.example`](./.env.example) |
| Maintainer routine tasks | [`docs/MAINTAINERS.md`](./docs/MAINTAINERS.md) |
| Trust model & vulnerability reporting | [`SECURITY.md`](./SECURITY.md) |
| Architecture decisions | [`docs/adr/`](./docs/adr) |
| Threat model, resource profile, security review checklist, known risks | [`docs/security/`](./docs/security/) |
| Version migrations (v0.1 → v0.2, and future) | [`docs/migrations/`](./docs/migrations/) |

Nothing has been released to a registry, tagged, or audited. One
disposable instance is deployed to **testnet** (see *Deployed contracts*
above); there is no mainnet deployment. The manual
[`testnet-release`](./.github/workflows/testnet-release.yml) workflow
never deploys by default.

## Integration (for the future backend & frontend)

The contract source is at v0.2; the **backend, indexer, and frontend
are future repositories that do not exist yet**, and no v0.2 instance
has been deployed anywhere for them to eventually target. These
documents and the SDK describe what they will consume, once one exists:

| Resource | Purpose |
|---|---|
| [`docs/integration/`](./docs/integration/) | Versioned integration contract — current: [API v2](./docs/integration/contract-api-v2.md), [identifier spec v1](./docs/integration/identifier-spec-v1.md) (unchanged), [attestor protocol v2](./docs/integration/attestor-protocol-v2.md), [event/indexer v2](./docs/integration/event-indexer-v2.md), [sequence diagrams](./docs/integration/sequence-diagrams.md). Historical v0.1 docs are linked from [`docs/integration/README.md`](./docs/integration/README.md). |
| [`sdk/typescript/`](./sdk/typescript/) | Typed read-only client with paginated attestation helpers + canonical hash helpers + unsigned-transaction preparation. Never signs or submits. Targets v0.2. Generated bindings are drift-checked in CI. |
| [`docs/migrations/v0.1-to-v0.2.md`](./docs/migrations/v0.1-to-v0.2.md) | What changed, what didn't, and what v0.1's place is going forward. |

The contract ABI in the deployed WASM is authoritative once a v0.2
instance exists; until then, `src/lib.rs` and `docs/integration/*-v2.md`
are. `make integration-check` runs the SDK gate;
`make sdk-integration-testnet` runs the opt-in **read-only** testnet
check (always skipped today — no v0.2 instance to check against).

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) and run `make check` before you
open a PR. Please open an issue before large changes.

## License

[MIT](./LICENSE)
