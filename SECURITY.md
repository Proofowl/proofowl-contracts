# ProofOwl contracts — security & design notes

This document is the single place that spells out the trust model, the
things the contract deliberately does **not** do, the storage-lifetime
policy, and the known MVP limitations. Read it with `src/lib.rs` open.

**This crate's `src/` is the v0.2 candidate** (paginated attestation
storage, [ADR 0004](docs/adr/0004-paginated-attestation-storage.md)).
**No v0.2 instance has been deployed to any network** — see
[`docs/migrations/v0.1-to-v0.2.md`](docs/migrations/v0.1-to-v0.2.md).

**Phase 4 adversarial and security testing** (2026-09) produced a
formal threat model, a resource/scalability profile with a measured
hard storage ceiling (v0.1), and a consolidated security-review
package. A follow-on phase implemented the storage redesign that
finding called for. Start with these for anything beyond this
document's design-level summary:

- [`docs/security/threat-model-v1.md`](docs/security/threat-model-v1.md) — attacker capabilities, assets, mitigations, residual risk, severity, per threat category.
- [`docs/security/resource-profile-v1.md`](docs/security/resource-profile-v1.md) — the v0.1 finding: measured cost growth and the exact per-wallet history ceiling (286 attestations succeed, the 287th fails outright).
- [`docs/security/resource-profile-v2.md`](docs/security/resource-profile-v2.md) — the v0.2 candidate's evidence: 1000+ attestations with no ceiling.
- [`docs/security/security-review-checklist-v1.md`](docs/security/security-review-checklist-v1.md) — line-item review checklist, severity rubric, release-blocker definition, audit handoff checklist.
- [`docs/security/known-risks-v1.md`](docs/security/known-risks-v1.md) — the honest, ranked list of open risks and accepted limitations, including the v0.2 status update.

## Reporting a vulnerability

**Do not open a public issue or pull request for a security problem.**

Report it privately, one of two ways:

1. GitHub **private vulnerability reporting** — this repository's
   *Security* tab → *Report a vulnerability*.
2. Email the security contact:
   `SECURITY-CONTACT-PLACEHOLDER` — a maintainer must replace this with a
   monitored address before the repository is made widely public.

What to expect, stated honestly for the current phase:

- There is **no bug bounty** and **no committed response-time SLA**.
- Reports are still handled seriously and privately. A maintainer will
  acknowledge, assess scope against the trust model below, fix on a
  private branch with a regression test, and agree disclosure timing
  with the reporter.
- In scope: the contract in `src/`, the deployment/verification scripts
  in `scripts/`, and the CI/release workflows. Out of scope: the
  not-yet-existing backend, indexer, and frontend repositories, and any
  deployed instance (none exists yet).

The trust *assumptions* the contract makes on purpose (single trusted
attestor, off-chain GitHub verification, immutability, deferred
recovery) are documented below and in `docs/adr/` — those are design
decisions, not vulnerabilities.

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

The admin is fixed at deploy time (constructor) and can do exactly one
thing afterwards: `set_attestor`. There is **no** admin function that can
create, move, or delete a wallet ↔ GitHub link, edit an attestation, or
change a score. This is intentional (see §4).

## 2. Initialization

There is **no `init` entrypoint.** Configuration is written by
`__constructor(admin, attestor)`, which the host runs exactly once,
inside the `CreateContract` host operation that deploys the instance.
Deployment and initialization are the *same* operation in the *same*
transaction — not a script that happens to run two commands in a row.

Why this closes the takeover hole a hardened `init` left open:

- With a separate `init`, a deployed-but-uninitialized instance is a
  shared resource. `admin.require_auth()` stops an attacker installing an
  admin address they don't hold, but it does not stop them calling `init`
  *first* with an address they *do* hold and permanently capturing that
  specific instance (e.g. one a deploy script created but had not yet
  initialized).
- With a constructor there is no uninitialized window and no separate
  call: the instance does not exist until the transaction that also
  configures it commits. A front-runner who deploys their own copy just
  gets a *different* contract id — they cannot touch yours.

The constructor additionally calls `admin.require_auth()`, so the deploy
transaction must carry the admin's signature: configuration is bound to a
deployer-authorized setup, and a mistaken admin address (one whose key
didn't sign) fails the deploy outright.

This is verified end-to-end against the compiled wasm and the real
deployer auth path in `tests/constructor_auth.rs`
(`initialization_is_bound_to_an_authorized_deployment`): an unauthorized
`deploy_v2` fails, an authorized one binds `admin` / `attestor` atomically.
See `docs/adr/0003-deploy-time-constructor-init.md`.

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
persistent entry it touches **and** the instance entry. In particular
`submit_attestation` extends the new `SeenPr` marker, the history vector,
both link records, and the instance.

**Permissionless keep-alive (v0.2 — bounded, two calls):** a frontend or
cron job refreshes a wallet's records with two calls instead of one,
per [ADR 0004](docs/adr/0004-paginated-attestation-storage.md):

- `bump_wallet_core_ttl(wallet)` — O(1): `WalletLink(wallet)`, the
  `GithubLink` it points at, `AttestationCount(wallet)`, and
  `ReputationScore(wallet)`.
- `bump_attestations_ttl_page(wallet, start, limit)` — bounded: extends
  one page of `AttestationEntry(wallet, seq)` records **and the
  `SeenPr(pr_hash)` marker each one references**. A full sweep calls
  this repeatedly with an advancing `start` until the returned count is
  less than `limit`.

`SeenPr` markers are what make duplicate-PR rejection global and
permanent; the paginated sweep is what keeps every one of them warm
regardless of which page it is on — the same property v0.1's single
`bump_wallet_ttl` provided for its one-vector history, now achieved
without loading the whole history in one call. See
`tests/ttl_replay.rs` for the full test coverage of this split.

The cost of `bump_wallet_core_ttl` is constant; the cost of
`bump_attestations_ttl_page` is bounded by `limit`, not by the wallet's
total history size — see `docs/security/resource-profile-v2.md`.

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
via the two keep-alive calls above before the 120-day horizon, or
restored with `RestoreFootprint` afterwards.

**Entries covered:** `WalletLink`, `GithubLink`, `SeenPr`,
`AttestationEntry`, `AttestationCount`, `ReputationScore`, and the
instance (`Admin` / `Attestor`) — every persistent record the contract
writes. No record is left on the default TTL.

## 6. First testnet deployment checklist

1. `cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D
   warnings`, `cargo test --all`, and `cargo build --target
   wasm32v1-none --release` all green on the commit you intend to ship.
   Commit `Cargo.lock` (it is tracked) so the wasm is reproducible.
2. (Optional) `stellar contract optimize --wasm
   target/wasm32v1-none/release/proofowl_contracts.wasm`.
3. Create + fund two **distinct testnet** identities: `admin` and
   `attestor`. Never reuse a mainnet key.
4. Deploy **and initialize in one transaction** by passing the
   constructor args to `deploy`, signed by `admin`:
   ```
   stellar contract deploy --wasm <wasm> --source <admin> --network testnet \
     -- --admin <ADMIN_ADDR> --attestor <ATTESTOR_ADDR>
   ```
   Capture the printed contract id. There is no second `init` step.
5. Verify: `get_admin` and `get_attestor` return the expected addresses.
   (There is no `init` to call twice; re-running the constructor is not
   possible on-chain.)
6. Smoke-test one full path on testnet: `link_github` (co-signed by a
   throwaway wallet + attestor) → `submit_attestation` → check
   `get_attestation_count` / `get_attestations_page` /
   `get_reputation_score` → `unlink_github`.
7. Record the contract id in `README.md` under *Deployed contracts* and
   tag the release.
8. Hand the `attestor` address to the backend team; keep the `admin` key
   offline / in a hardware signer. Plan the `set_attestor` rotation to a
   multisig before mainnet.

## 7. Known limitations

- **Attestation storage scaling — resolved in this v0.2 candidate,
  pending live validation.** v0.1 kept a single `Vec<Attestation>` per
  wallet under `Attestations(wallet)`, which Phase 4's
  `tests/resource_profile.rs` measured to fail outright at 286
  attestations (65,536-byte per-entry ceiling) — see
  [`docs/security/resource-profile-v1.md`](docs/security/resource-profile-v1.md)
  for that finding, kept unedited. This crate's current `src/lib.rs`
  replaces it with one persistent entry per attestation keyed by
  `(wallet, seq)`, a `count`, and a running `score` counter
  ([ADR 0004](docs/adr/0004-paginated-attestation-storage.md)), measured
  to hold 1000+ attestations with no failure
  ([`docs/security/resource-profile-v2.md`](docs/security/resource-profile-v2.md)).
  **This is a local candidate — it has not been deployed to any
  network, audited, or exercised live.** See
  [`docs/migrations/v0.1-to-v0.2.md`](docs/migrations/v0.1-to-v0.2.md).
- **Lost-wallet-key recovery is deferred** (§4.2).
- **Cross-wallet history migration is deferred** (§4.2).
- **Single trusted attestor** (§1.2) — rotation to a multisig/threshold
  scheme via `set_attestor` is expected before mainnet.
