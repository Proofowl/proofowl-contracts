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

## Storage durability

Soroban archives a persistent entry once its TTL runs out. Every registry
record here — wallet links, GitHub links, PR-dedup markers, attestation
histories, and the contract instance — has its TTL extended on every
write, and anyone can call `bump_wallet_ttl(wallet)` to keep a passport
warm for free. Policy and constants are in
[`SECURITY.md`](./SECURITY.md#storage-durability-ttl-policy).

## Quick start

### Prerequisites

- Rust **1.84+** (stable). `soroban-sdk 27` no longer builds against
  `wasm32-unknown-unknown` on Rust ≥ 1.82; the Soroban wasm target is now
  **`wasm32v1-none`**:
  `rustup target add wasm32v1-none`
- [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli)

### Build and test

```
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --target wasm32v1-none --release
```

### Deploy (testnet)

Deploy and initialize in one step so there is no unowned window — `init`
requires the proposed admin's signature, but atomic deploy+init removes
the race entirely:

```
stellar contract deploy \
  --wasm target/wasm32v1-none/release/proofowl_contracts.wasm \
  --source <admin-testnet-identity> \
  --network testnet \
  -- \
  # (no constructor; run init immediately, from the same identity)
```

```
stellar contract invoke --id <CONTRACT_ID> --network testnet \
  --source <admin-testnet-identity> -- \
  init --admin <ADMIN_ADDRESS> --attestor <ATTESTOR_ADDRESS>
```

See [`SECURITY.md`](./SECURITY.md#first-testnet-deployment-checklist) for
the full first-deployment checklist.

## Contract API

| Function | Caller(s) | Description |
|---|---|---|
| `init(admin, attestor)` | proposed `admin` (signs) | One-time setup; fails if already initialized |
| `set_attestor(admin, new_attestor)` | `admin` | Rotate the attestor key |
| `link_github(wallet, attestor, github_id_hash)` | **both** `wallet` and `attestor` | Two-party wallet ↔ GitHub link |
| `unlink_github(wallet, attestor, github_id_hash)` | **both** linked `wallet` and `attestor` | Clear a link for recovery / relink |
| `submit_attestation(attestor, github_id_hash, repo, pr_number, issue_id, complexity, pr_hash)` | `attestor` | Record a verified contribution; returns the credited wallet |
| `bump_wallet_ttl(wallet)` | anyone | Extend TTL on a wallet's link + history (keep-alive) |
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
| 1 | `AlreadyInitialized` | `init` called twice |
| 2 | `NotInitialized` | called before `init` |
| 3 | `Unauthorized` | caller is not the stored admin / attestor |
| 4 | `WalletAlreadyLinked` | that wallet already has an identity |
| 5 | `GithubAlreadyLinked` | that identity hash is already claimed |
| 6 | `DuplicateAttestation` | that `pr_hash` was already recorded |
| 7 | `WalletNotLinked` | no wallet linked for that identity hash |
| 8 | `InvalidComplexity` | `complexity` not in `{0, 100, 150, 200}` |
| 9 | `LinkNotFound` | `unlink_github` target is not a consistent link |

## Deployed contracts

| Network | Contract ID |
|---|---|
| Testnet | _not yet deployed_ |

## Repositories

- [`proofowl-contracts`](https://github.com/proofowl/proofowl-contracts) — this repo
- [`proofowl-backend`](https://github.com/proofowl/proofowl-backend) — GitHub verification, attestation submission, REST API
- [`proofowl-frontend`](https://github.com/proofowl/proofowl-frontend) — passport pages, leaderboard, wallet linking UI

## Contributing

See [CONTRIBUTING.md](./CONTRIBUTING.md). Please open an issue before large changes.

## License

[MIT](./LICENSE)
