# ProofOwl contracts — security & design notes

This document is the single place that spells out the trust model, the
things the contract deliberately does **not** do, the storage-lifetime
policy, and the known MVP limitations. Read it with `src/lib.rs` open.

## 1. Trust boundaries

### 1.1 The contract cannot verify GitHub

The registry runs inside Soroban. It has no network access, cannot call
GitHub's API, and cannot inspect an OAuth token. A raw wallet signature
proves only that someone controls a Stellar key — **it says nothing about
who controls a GitHub account.**

So the contract does not try to prove GitHub ownership. It enforces a
*procedure* instead:

> A wallet ↔ GitHub link is created only if **both** the wallet **and**
> the trusted attestor authorize the same `link_github` call.

The attestor (operated by the future `proofowl-backend`) is trusted to
co-sign **only after** it has run a real off-chain GitHub OAuth /
challenge flow that proves the wallet holder controls the GitHub identity
behind `github_id_hash`. That off-chain step is where GitHub ownership is
actually established; the co-signature is its on-chain receipt.

This is a deliberate, documented trust assumption. It is not a check we
forgot. Removing the attestor from the loop would require either on-chain
oracles for GitHub or zero-knowledge proofs of an OAuth exchange, both
out of scope for v1.

### 1.2 The attestor resolves the wallet, it never chooses it

`submit_attestation` takes a `github_id_hash`, not a wallet address. The
contract looks up the wallet through the on-chain `GithubLink` mapping,
which only the contributor's own signature (plus the attestor
co-signature) can create.

Consequence: a compromised, buggy, or malicious attestor key can forge
*that* a contribution happened or misreport its complexity tier — that is
the inherent limit of a single trusted attestor in v1 — but it **cannot**
redirect credit to a wallet the GitHub identity has not itself linked.
See `docs/adr/0001-attestor-resolves-via-github-link.md`.

### 1.3 Identity squatting

Because a link needs the attestor co-signature, a wallet cannot claim
`hash("famous-maintainer")` on its own. On top of that:

- `WalletLink` and `GithubLink` are both one-to-one and checked in both
  directions (`WalletAlreadyLinked`, `GithubAlreadyLinked`).
- `submit_attestation` for an unlinked identity fails with
  `WalletNotLinked` — the backend is expected to hold verified facts in
  its own queue until the contributor links, then submit.

### 1.4 Admin powers

The admin can do exactly two things: `init` once, and `set_attestor`.
There is **no** admin function that can create, move, or delete a
wallet ↔ GitHub link, edit an attestation, or change a score. This is
intentional (see §4).

## 2. Initialization

`init` requires `admin.require_auth()` before it writes anything, so a
bystander cannot seize a deployed-but-uninitialized contract with an
admin address they do not control — the worst they can do is install an
admin address they *do* control, which is no better for them than
deploying their own copy of the contract.

**Deploy and `init` in the same transaction or script run.** That closes
the window between deployment and initialization entirely. The contract
keeps a plain `init` (rather than a constructor) to match the documented
Stellar CLI deploy flow; the atomic-deploy guidance is the mitigation for
the residual race.

## 3. Scoring integrity

- `submit_attestation` accepts `complexity ∈ {0, 100, 150, 200}` only.
  Anything else → `InvalidComplexity`, nothing is stored.
- `0` is the "confirmed, tier unknown" sentinel and scores at
  `UNVERIFIED_COMPLEXITY_SCORE = 50`.
- `get_reputation_score` folds with `saturating_add`, so the result is
  deterministic and cannot panic. With the accepted tier values the
  `u32::MAX` ceiling is unreachable in practice (it would take tens of
  millions of attestations).
- The release profile sets `overflow-checks = true`.

## 4. Recovery & lifecycle

### 4.1 What is recoverable

`unlink_github(wallet, attestor, github_id_hash)` is a **two-party** call
(the currently linked wallet **and** the attestor). It removes both
directions of the link. Use it to:

- fix a link made against the wrong `github_id_hash`;
- release an identity so its owner can re-link it to a different wallet
  after re-running the off-chain GitHub verification.

After an unlink:

- the wallet's `Attestations(wallet)` history is **left intact** —
  reputation already earned stays with the wallet that earned it;
- the global `SeenPr` markers are **left intact** — a merged PR stays
  spent forever and cannot be re-attested onto a new wallet.

### 4.2 What is not recoverable in the MVP, and why

- **Lost wallet key.** If the contributor no longer controls the linked
  wallet, they cannot satisfy the wallet half of `unlink_github`, so the
  identity stays linked to the dead wallet. We deliberately do **not**
  add an attestor-only or admin-only override to move a link, because
  that same override would let a compromised attestor/admin silently
  steal a contributor's identity and reputation — exactly the property
  §1.2 exists to protect. A future version can add a safer
  time-locked, publicly-announced recovery (e.g. "new wallet + attestor,
  effective after N ledgers unless the old wallet objects"); until then,
  deferring is the safer choice.
- **History migration.** `unlink` + re-link moves the *identity* but not
  past attestations. Carrying a history to a fresh wallet is future work;
  it needs its own dedup/consistency design.

## 5. Storage durability (TTL policy)

Soroban archives a persistent entry once its TTL (ledgers-remaining)
reaches zero; reading an archived entry fails until someone restores it.
Every record in this registry is meant to live indefinitely.

**Policy:** on every mutating call, the contract extends the TTL of every
persistent entry it touches **and** the instance entry. A permissionless
`bump_wallet_ttl(wallet)` lets a frontend or cron job refresh a wallet's
link, the GitHub link it points at, and its attestation history without
changing any data.

**Constants** (`src/lib.rs`), at the ~5s mainnet ledger cadence
(1 day ≈ 17 280 ledgers):

| Constant | Ledgers | ≈ | Meaning |
|---|---|---|---|
| `REGISTRY_TTL_EXTEND_TO` | 2 073 600 | 120 days | TTL is bumped up to this |
| `REGISTRY_TTL_THRESHOLD` | 1 555 200 | 90 days | only bump if under this |

Both are clamped to `env.storage().max_ttl()` defensively. 120 days sits
comfortably under the mainnet/testnet persistent-entry cap (~180 days) so
there is no silent clamp in normal operation. Records touched at least
once every ~90 days never come close to archival; genuinely cold records
(a contributor who links and then disappears) can be revived by anyone
via `bump_wallet_ttl` before the 120-day horizon, or restored with
`RestoreFootprint` afterwards.

Entries covered: `WalletLink`, `GithubLink`, `SeenPr`, `Attestations`,
and the instance (`Admin` / `Attestor`).

## 6. First testnet deployment checklist

1. `cargo test --all` and `cargo build --target wasm32v1-none --release`
   both green on the commit you intend to ship.
2. Optionally `stellar contract optimize` the wasm.
3. Create/fund two **testnet** identities: `admin` and `attestor`
   (distinct keys). Never reuse a mainnet key.
4. `stellar contract deploy` the wasm from the `admin` identity; capture
   the contract id.
5. **Immediately**, from the same `admin` identity and ideally batched
   with step 4:
   `stellar contract invoke --id <ID> -- init --admin <ADMIN> --attestor <ATTESTOR>`.
6. Verify: `get_admin` and `get_attestor` return the expected addresses;
   a second `init` fails with `AlreadyInitialized`.
7. Smoke-test one full path on testnet: `link_github` (co-signed by a
   throwaway wallet + attestor) → `submit_attestation` → check
   `get_attestations` / `get_reputation_score` → `unlink_github`.
8. Record the contract id in `README.md` under *Deployed contracts* and
   tag the release.
9. Hand the `attestor` address to the backend team; keep the `admin` key
   offline / in a hardware signer. Plan the `set_attestor` rotation to a
   multisig before mainnet.
