# 🦉 ProofOwl — on-chain contributor reputation registry

[![CI](https://github.com/proofowl/proofowl-contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/proofowl/proofowl-contracts/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](./LICENSE)
[![Built on Stellar](https://img.shields.io/badge/Built%20on-Stellar-blueviolet)](https://stellar.org)

> A contributor's Stellar Wave track record shouldn't live only in one
> platform's private database. ProofOwl anchors verified, merged
> contributions on-chain — portable, checkable by anyone, and outliving
> any single program's backend.

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
histories, and the contract instance — has its TTL extended on every
write. Anyone can call `bump_wallet_ttl(wallet)` to keep a passport warm
for free; it refreshes the wallet link, the GitHub link, the attestation
history, **and every `SeenPr` de-duplication marker in that history**, so
a merged PR can never become re-submittable through TTL expiry. Policy
and constants are in
[`SECURITY.md`](./SECURITY.md#5-storage-durability-ttl-policy).

## Known MVP limitation — attestation storage

A wallet's attestations live in one `Vec<Attestation>` under a single
key. Reads, `bump_wallet_ttl`, and each new `submit_attestation` load or
rewrite the whole vector, so cost grows with a contributor's history.
Fine for MVP volumes; production scale needs paginated / indexed storage
(one entry per attestation + a running score counter). Deliberately
deferred — see [`SECURITY.md`](./SECURITY.md#7-known-mvp-limitations).

## Quick start

### Prerequisites

- Rust **1.84+** (stable). `soroban-sdk 27` no longer builds against
  `wasm32-unknown-unknown` on Rust ≥ 1.82; the Soroban wasm target is now
  **`wasm32v1-none`**:
  `rustup target add wasm32v1-none`
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli)

### Build and test

`make check` runs the complete local quality gate (the same steps CI
runs):

```
make check
# = cargo fmt --all -- --check
#   cargo clippy --all-targets -- -D warnings
#   cargo build --target wasm32v1-none --release
#   cargo test --all
```

`make help` lists every target. Supply-chain checks
(`make deny` / `make audit`) need network access and are a separate CI
job — see [Operations & release](#operations--release).

### Deploy (testnet)

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

## Contract API

| Function | Caller(s) | Description |
|---|---|---|
| `__constructor(admin, attestor)` | the deployer, signed by `admin` | Runs once at deploy; sets admin + attestor. No separate `init`. |
| `set_attestor(admin, new_attestor)` | `admin` | Rotate the attestor key |
| `link_github(wallet, attestor, github_id_hash)` | **both** `wallet` and `attestor` | Two-party wallet ↔ GitHub link |
| `unlink_github(wallet, attestor, github_id_hash)` | **both** linked `wallet` and `attestor` | Clear a link for recovery / relink |
| `submit_attestation(attestor, github_id_hash, repo, pr_number, issue_id, complexity, pr_hash)` | `attestor` | Record a verified contribution; returns the credited wallet |
| `bump_wallet_ttl(wallet)` | anyone | Keep-alive: extends TTL on the wallet link, GitHub link, history, and every `SeenPr` marker in it |
| `get_attestations(wallet)` | anyone (read) | Full attestation history for a wallet |
| `get_reputation_score(wallet)` | anyone (read) | Summed score across all attestations |
| `get_wallet_for_github(github_id_hash)` | anyone (read) | Forward lookup: identity → wallet |
| `get_github_for_wallet(wallet)` | anyone (read) | Reverse lookup: wallet → identity hash |
| `get_admin()` / `get_attestor()` | anyone (read) | Current admin / attestor address |

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

## Deployed contracts

| Network | Contract ID | Source | Notes |
|---|---|---|---|
| **Testnet** | `CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6` | `d030908` | Alpha, 2026-09-01. WASM `d694e0ad…ef96dbf1`. Verified + smoke-tested — [evidence](./docs/testnet/phase2-alpha.md). Disposable; may be replaced. |
| Mainnet | _not deployed — out of scope_ | — | See [`PRODUCTION_READINESS.md`](./PRODUCTION_READINESS.md) |

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

Nothing has been released to a registry, tagged, or audited. One
disposable instance is deployed to **testnet** (see *Deployed contracts*
above); there is no mainnet deployment. The manual
[`testnet-release`](./.github/workflows/testnet-release.yml) workflow
never deploys by default.

## Integration (for the future backend & frontend)

The contract is done and testnet-verified. The **backend, indexer, and
frontend are future repositories that do not exist yet.** These
documents and the SDK are what they will consume:

| Resource | Purpose |
|---|---|
| [`docs/integration/`](./docs/integration/) | Versioned integration contract — [API](./docs/integration/contract-api-v1.md), [identifier spec](./docs/integration/identifier-spec-v1.md), [attestor protocol](./docs/integration/attestor-protocol-v1.md), [event/indexer](./docs/integration/event-indexer-v1.md), [sequence diagrams](./docs/integration/sequence-diagrams.md) |
| [`sdk/typescript/`](./sdk/typescript/) | Typed read-only client + canonical hash helpers + unsigned-transaction preparation. Never signs or submits. Generated bindings are drift-checked in CI. |

The contract ABI in the deployed WASM is authoritative; the docs
describe it. `make integration-check` runs the SDK gate;
`make sdk-integration-testnet` runs the opt-in **read-only** testnet
check.

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md) and run `make check` before you
open a PR. Please open an issue before large changes.

## License

[MIT](./LICENSE)
