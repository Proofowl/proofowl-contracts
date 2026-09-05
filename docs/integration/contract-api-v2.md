# ProofOwl contract API — integration spec v2

Status: **stable for the current contract (local v0.2 candidate)**.
Supersedes [`contract-api-v1.md`](./contract-api-v1.md), which describes
the v0.1 ABI and is kept as the historical record of that version — it
no longer matches this crate's `src/lib.rs`. **No v0.2 instance has been
deployed to any network as of this document** — see
[`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md).

## What changed from v1

Full rationale: [`../adr/0004-paginated-attestation-storage.md`](../adr/0004-paginated-attestation-storage.md).
Summary:

- **Removed**: `get_attestations(wallet) -> Vec<Attestation>` (unbounded)
  and `bump_wallet_ttl(wallet)` (unbounded). Both had no ceiling on cost
  or response size — the root cause of the ceiling
  `docs/security/resource-profile-v1.md` measured at 286 attestations.
- **Added**: `get_attestation_count`, `get_attestation`,
  `get_attestations_page` (bounded reads); `bump_wallet_core_ttl`,
  `bump_attestations_ttl_page` (bounded TTL maintenance).
- **Added error codes** 10–13 (`InvalidPageLimit`, `PageLimitExceeded`,
  `SequenceOutOfRange`, `PageStartOutOfRange`). Codes 1–9 are byte-for-byte
  unchanged from v1 — nothing was renumbered.
- **`AttestationRecorded` gained a `sequence: u32` field** (additive to
  the data map).
- Everything else — `__constructor`, `set_attestor`, `link_github`,
  `unlink_github`, `submit_attestation`'s signature and auth rules,
  `get_wallet_for_github`, `get_github_for_wallet`, `get_admin`,
  `get_attestor` — is **unchanged**.

## Source of truth

**The deployed WASM and its embedded contract spec (ABI) are
authoritative.** This document describes that ABI in prose for
integrators; if the two ever disagree, the on-chain spec wins and this
file is the bug. Regenerate the machine-readable view any time with:

```
stellar contract bindings typescript \
  --wasm target/wasm32v1-none/release/proofowl_contracts.wasm \
  --output-dir sdk/typescript/src/generated --overwrite
```

or, for the raw XDR spec:

```
stellar contract inspect --wasm target/wasm32v1-none/release/proofowl_contracts.wasm
```

- Contract crate version: `0.2.0` (`Cargo.toml`)
- soroban-sdk: `27.0.6`
- No reference build WASM SHA-256 is recorded here yet — none has been
  published as a release artifact from this candidate.
- No testnet or mainnet instance exists for v0.2.

## Conventions used below

Unchanged from v1:

| Term | Meaning |
|---|---|
| `Address` | Stellar strkey — `G…` account or `C…` contract. In JSON/TS it is a string. |
| `BytesN<32>` | exactly 32 bytes. In TS it is a `Buffer`/`Uint8Array` of length 32; on the CLI a 64-char lowercase hex string. |
| `Result<T>` | the call can return a typed contract `Error` (see [Errors](#errors)). A failing `Result` surfaces during **simulation** — a rejected write never becomes a transaction. |
| `Option<T>` | `T` or absent (`null` / `undefined` / `None`). |
| "mutating" | changes ledger state; must be signed and submitted. |
| "read-only" | a simulation is sufficient; no signature, no fee, no state change. |
| "two-party" | the call has **two** independent `require_auth()` addresses; see [Two-party authorization](#two-party-authorization). |

Ledger types map as: `u32` → number, `u64` → bigint (JS), `string` →
UTF-8 string, `Vec<T>` → array.

## Data types

### `Attestation`

Returned by `get_attestation` / `get_attestations_page`. Field order in
the ABI map is alphabetical; integrators must key by name, not
position. **Unchanged from v1** — the type itself did not change; only
how a caller reaches an instance of it did (by explicit sequence /
page, not an unbounded vector).

| Field | Type | Notes |
|---|---|---|
| `repo` | `string` | `"<owner>/<repo>"`, lowercased by the backend before submission by convention (the contract stores it verbatim). |
| `pr_number` | `u32` | GitHub pull-request number. |
| `issue_id` | `u64` | Stellar Wave issue id the contribution resolved. `0` if not applicable. |
| `complexity` | `u32` | One of `0`, `100`, `150`, `200`. `0` = "confirmed, tier unknown". |
| `pr_hash` | `BytesN<32>` | SHA-256 of the canonical PR identifier — see [`identifier-spec-v1.md`](./identifier-spec-v1.md) (unchanged in v0.2). Global de-dup key. |
| `timestamp` | `u64` | Ledger close time (Unix seconds) when the attestation was recorded. **Set by the contract**, not the caller. |

Note: `Attestation` itself carries no `sequence` field — the sequence
is the *address* you use to fetch it (`get_attestation`'s argument, or
a page's `start + index`), not a stored property of the value. The
`AttestationRecorded` **event** does carry a `sequence` field (see
below) since an event has no other way to convey it.

### `github_id_hash`

Unchanged from v1: `BytesN<32>` — SHA-256 of the canonical GitHub
*numeric user id* identifier. Construction and caveats:
[`identifier-spec-v1.md`](./identifier-spec-v1.md).

## Errors

`#[contracterror]`, `#[repr(u32)]`. A `Result` call fails with one of:

| Code | Name | Raised by | Meaning |
|---|---|---|---|
| 1 | `AlreadyInitialized` | — | Reserved for numbering stability. Unreachable (constructor runs once, no `init`). |
| 2 | `NotInitialized` | `set_attestor`, `link_github`, `unlink_github`, `submit_attestation` | Instance config missing (e.g. archived). Practically unreachable while the instance entry is alive. |
| 3 | `Unauthorized` | `set_attestor`, `link_github`, `unlink_github`, `submit_attestation` | Caller-supplied `admin`/`attestor` is not the stored one. (A *missing* signature fails earlier, as a host auth error, not this.) |
| 4 | `WalletAlreadyLinked` | `link_github` | That wallet already has a GitHub link. |
| 5 | `GithubAlreadyLinked` | `link_github` | That `github_id_hash` is already linked to some wallet. |
| 6 | `DuplicateAttestation` | `submit_attestation` | That `pr_hash` was already recorded (globally, forever). |
| 7 | `WalletNotLinked` | `submit_attestation` | No wallet is linked for that `github_id_hash`. |
| 8 | `InvalidComplexity` | `submit_attestation` | `complexity` not in `{0, 100, 150, 200}`. |
| 9 | `LinkNotFound` | `unlink_github` | `(wallet, github_id_hash)` is not an existing, consistent link in both directions. |
| 10 | `InvalidPageLimit` | `get_attestations_page`, `bump_attestations_ttl_page` | `limit` was `0`. **v0.2.** |
| 11 | `PageLimitExceeded` | `get_attestations_page`, `bump_attestations_ttl_page` | `limit` exceeded `MAX_PAGE_SIZE` (50). **v0.2.** |
| 12 | `SequenceOutOfRange` | `get_attestation` | `sequence >= get_attestation_count(wallet)`. **v0.2.** |
| 13 | `PageStartOutOfRange` | `get_attestations_page`, `bump_attestations_ttl_page` | `start > get_attestation_count(wallet)`. `start == count` is **not** an error (yields an empty page / zero refreshed). **v0.2.** |

Two error surfaces to distinguish when mapping:

- **Contract error** (codes above): `Error(Contract, #N)` — deterministic, business logic.
- **Host auth error**: raised when a required `require_auth()` signature is absent or invalid (`Error(Auth, …)`). Not one of the codes above.

## Two-party authorization

Unchanged from v1. `link_github` and `unlink_github` each call
`require_auth()` on **two independent addresses**: the contributor
`wallet` and the trusted `attestor`. **A single ordinary wallet
signature cannot complete these calls**, and neither can a single
attestor signature. Both parties must authorize the *same* invocation.

On the Stellar CLI (v28), the working form is
`--source <wallet> --auto-sign` (no `--sign-with-key`): `--source` signs
the envelope + the wallet's root auth entry, `--auto-sign` signs the
attestor's non-root Soroban auth entry from the keystore. In a
frontend/backend split:

1. The frontend builds the `AssembledTransaction` and collects the
   contributor's wallet auth-entry signature.
2. The backend adds the attestor auth-entry signature (only after its
   own GitHub OAuth/challenge verification succeeds — see
   [`attestor-protocol-v2.md`](./attestor-protocol-v2.md)).
3. Whoever holds the fully-signed transaction submits it.

The order of the two auth-entry signatures does not matter; both must be
present before submission.

---

## Functions

Each entry lists: signature · caller/auth · mutability · errors · events
· storage/TTL · backend notes · frontend notes. Functions unchanged from
v1 are marked **(unchanged)**; v0.2 additions are marked **(v0.2)**.

### `__constructor(admin: Address, attestor: Address)` (unchanged)

- **Signature:** `__constructor(admin, attestor) -> ()`
- **Caller / auth:** the deployer, in the `CreateContract` operation.
  Calls `admin.require_auth()`, so the deploy transaction must be signed
  by `admin`. **Cannot be invoked after deployment** — there is no
  `init` entrypoint and the host runs `__constructor` exactly once.
- **Mutability:** writes instance storage (`Admin`, `Attestor`).
- **Returns:** nothing.
- **Errors:** none typed. A missing `admin` signature fails the deploy
  as a host auth error.
- **Events:** `Initialized` — topics `["initialized", admin]`, data
  `{ attestor }`.
- **Storage / TTL:** sets `Admin` and `Attestor`; extends the instance
  TTL.

### `set_attestor(admin: Address, new_attestor: Address) -> Result<()>` (unchanged)

- **Caller / auth:** the stored `admin`. Calls `admin.require_auth()`
  **and** checks `admin == stored Admin`.
- **Mutability:** writes `Attestor` in instance storage.
- **Errors:** `NotInitialized` (2), `Unauthorized` (3).
- **Events:** `AttestorRotated` — topics `["attestor_rotated", admin]`,
  data `{ new_attestor }`.
- **Storage / TTL:** extends the instance TTL.
- **Backend notes:** after a rotation the **old** attestor key is
  immediately rejected by `link_github` / `submit_attestation`.

### `link_github(wallet: Address, attestor: Address, github_id_hash: BytesN<32>) -> Result<()>` (unchanged)

- **Caller / auth:** **two-party** — `wallet.require_auth()` **and**
  `attestor.require_auth()`, plus `attestor == stored Attestor`.
- **Mutability:** writes `WalletLink(wallet) = github_id_hash` and
  `GithubLink(github_id_hash) = wallet` (both directions).
- **Errors:** `NotInitialized` (2), `Unauthorized` (3, wrong attestor),
  `WalletAlreadyLinked` (4), `GithubAlreadyLinked` (5).
- **Events:** `GithubLinked` — topics `["github_linked", wallet]`, data
  `{ github_id_hash }`.
- **Storage / TTL:** creates two persistent entries and extends their
  TTL plus the instance TTL.

### `unlink_github(wallet: Address, attestor: Address, github_id_hash: BytesN<32>) -> Result<()>` (unchanged)

- **Caller / auth:** **two-party** — the *currently linked* `wallet` and
  the stored `attestor`.
- **Mutability:** removes both `WalletLink(wallet)` and
  `GithubLink(github_id_hash)`.
- **Errors:** `NotInitialized` (2), `Unauthorized` (3), `LinkNotFound`
  (9).
- **Events:** `GithubUnlinked` — topics `["github_unlinked", wallet]`,
  data `{ github_id_hash }`.
- **Storage / TTL:** deletes the two link entries; extends the instance
  TTL. **Does not touch** any `AttestationEntry`, `AttestationCount`,
  `ReputationScore`, or `SeenPr` marker.

### `submit_attestation(attestor, github_id_hash, repo, pr_number, issue_id, complexity, pr_hash) -> Result<Address>` (storage changed, signature unchanged)

- **Signature:** unchanged from v1.
- **Caller / auth:** the stored `attestor` only — unchanged.
- **Mutability (v0.2):** writes one new `AttestationEntry(wallet, seq)`
  (`seq` = the wallet's current `AttestationCount`, zero-based),
  increments `AttestationCount(wallet)`, updates
  `ReputationScore(wallet)` atomically (`+= complexity`, or `+= 50` if
  `complexity == 0`), and writes `SeenPr(pr_hash)`. Replaces v1's
  "append to `Attestations(wallet)`'s `Vec`."
- **Returns:** `Ok(wallet)` — the address credited, exactly as v1.
- **Errors:** `NotInitialized` (2), `Unauthorized` (3),
  `InvalidComplexity` (8, checked first), `WalletNotLinked` (7),
  `DuplicateAttestation` (6). Evaluation order unchanged: auth →
  complexity → wallet resolution → dedup.
- **Events:** `AttestationRecorded` — topics
  `["attestation_recorded", wallet]`, data `{ repo, pr_number,
  issue_id, complexity, pr_hash, timestamp, sequence }`. **`sequence`
  is new in v0.2** (additive field) — the zero-based index this
  attestation occupies in `wallet`'s history, the same value
  `get_attestation` addresses it by.
- **Storage / TTL:** extends the new `AttestationEntry`, the
  `AttestationCount`, the `ReputationScore`, the new `SeenPr` marker,
  both link entries, and the instance.
- **Backend notes:** unchanged from v1 otherwise — derive `pr_hash`
  exactly per `identifier-spec-v1.md`; treat `DuplicateAttestation` as
  success-equivalent; on `WalletNotLinked`, hold the verified fact in
  your own queue.

### `get_attestation_count(wallet: Address) -> u32` (v0.2)

- **Caller / auth:** none. Read-only.
- **Returns:** how many attestations `wallet` has. `0` for an unknown
  or never-attested wallet. Also the next `sequence`
  `get_attestation` will resolve once one more attestation is
  submitted.
- **Errors:** none.
- **Events / storage:** none (a simulation does not extend TTL).
- **Backend / frontend notes:** the cheap "does this wallet have a
  history at all, and how big" call — O(1), a single-key read.

### `get_attestation(wallet: Address, sequence: u32) -> Result<Attestation>` (v0.2)

- **Caller / auth:** none. Read-only.
- **Returns:** the attestation at zero-based index `sequence` in
  `wallet`'s history (`0` = the first ever recorded for that wallet).
- **Errors:** `SequenceOutOfRange` (12) if `sequence >=
  get_attestation_count(wallet)`.
- **Events / storage:** none.
- **Backend / frontend notes:** O(1), a single-key read. Prefer
  `get_attestations_page` when you need more than one entry — looping
  this call per index also works but a page fetches a whole range in
  one round-trip.

### `get_attestations_page(wallet: Address, start: u32, limit: u32) -> Result<Vec<Attestation>>` (v0.2, replaces v1's `get_attestations`)

- **Caller / auth:** none. Read-only.
- **Returns:** up to `limit` attestations starting at zero-based index
  `start`, oldest first — `wallet`'s history sliced
  `[start, start + limit)`, truncated to however many actually exist.
  `start == get_attestation_count(wallet)` returns an **empty array**
  (not an error) — the "no more pages" signal for a caller iterating to
  the end.
- **Errors:** `InvalidPageLimit` (10, `limit == 0`),
  `PageLimitExceeded` (11, `limit >` `MAX_PAGE_SIZE` = 50),
  `PageStartOutOfRange` (13, `start >` the wallet's count — note this
  is strictly greater-than; `start == count` is valid, see above).
- **Events / storage:** none.
- **Backend / frontend notes:** this is the paginated replacement for
  v1's unbounded `get_attestations`. Cost and response size are bounded
  by `limit` regardless of how large `wallet`'s total history is —
  see `docs/security/resource-profile-v2.md`. To fetch a whole history,
  loop with an advancing `start` until a page returns fewer than
  `limit` entries.

### `bump_wallet_core_ttl(wallet: Address) -> ()` (v0.2, replaces part of v1's `bump_wallet_ttl`)

- **Caller / auth:** **anyone**, no `require_auth()`. Permissionless.
- **Mutability:** none to data. Extends TTLs only, and only for the
  wallet's **O(1) records**: `WalletLink(wallet)`, the `GithubLink` it
  points to, `AttestationCount(wallet)`, `ReputationScore(wallet)`, and
  the instance. **Does not touch** any `AttestationEntry` or `SeenPr`
  marker — use `bump_attestations_ttl_page` for those.
- **Returns:** nothing. Infallible. No-op for a wallet with no link and
  no history.
- **Errors:** none typed.
- **Events:** none.
- **Backend notes:** cheap and safe to call often; does not, by itself,
  keep a wallet's attestation history warm — pair with a scheduled
  `bump_attestations_ttl_page` sweep. See
  `docs/integration/event-indexer-v2.md` §6 and
  `docs/security/resource-profile-v2.md`.
- **Frontend notes:** safe to expose as part of a "keep my passport
  alive" action; needs only the caller's own signature as tx source.

### `bump_attestations_ttl_page(wallet: Address, start: u32, limit: u32) -> Result<u32>` (v0.2, replaces part of v1's `bump_wallet_ttl`)

- **Caller / auth:** **anyone**, no `require_auth()`. Permissionless.
- **Mutability:** none to data. Extends the TTL of each
  `AttestationEntry(wallet, seq)` in `[start, start + limit)` and the
  `SeenPr(pr_hash)` marker each one references — so a merged PR can
  never become re-submittable just because its marker was allowed to
  expire, regardless of which page it is on.
- **Returns:** `Ok(refreshed)` — the number of entries actually
  refreshed in this call (`<= limit`). A return value less than
  `limit` (including `0`) means the sweep has reached the end of the
  wallet's history as of this call.
- **Errors:** same rules as `get_attestations_page`:
  `InvalidPageLimit` (10), `PageLimitExceeded` (11),
  `PageStartOutOfRange` (13).
- **Events:** none.
- **Backend notes:** a full-history keep-alive sweep calls this
  repeatedly with an advancing `start` until the return value is less
  than `limit`; re-run periodically since new attestations can be
  submitted between sweeps. See
  `docs/integration/event-indexer-v2.md` §6.

### `get_wallet_for_github`, `get_github_for_wallet`, `get_admin`, `get_attestor` (all unchanged)

Identical to v1 — see [`contract-api-v1.md`](./contract-api-v1.md) if
you need the individual descriptions; nothing about their signature,
auth, errors, or behavior changed in v0.2.

---

## Quick reference

| Function | Mutating | Auth | Returns | Typed errors | Version |
|---|---|---|---|---|---|
| `__constructor` | yes (deploy only) | admin | — | — | unchanged |
| `set_attestor` | yes | admin | `Result<()>` | 2, 3 | unchanged |
| `link_github` | yes | **wallet + attestor** | `Result<()>` | 2, 3, 4, 5 | unchanged |
| `unlink_github` | yes | **wallet + attestor** | `Result<()>` | 2, 3, 9 | unchanged |
| `submit_attestation` | yes | attestor | `Result<Address>` | 2, 3, 6, 7, 8 | storage changed |
| `get_attestation_count` | no | none | `u32` | — | **v0.2** |
| `get_attestation` | no | none | `Result<Attestation>` | 12 | **v0.2** |
| `get_attestations_page` | no | none | `Result<Vec<Attestation>>` | 10, 11, 13 | **v0.2** |
| `bump_wallet_core_ttl` | yes (TTL only) | none | — | — | **v0.2** |
| `bump_attestations_ttl_page` | yes (TTL only) | none | `Result<u32>` | 10, 11, 13 | **v0.2** |
| `get_wallet_for_github` | no | none | `Option<Address>` | — | unchanged |
| `get_github_for_wallet` | no | none | `Option<BytesN<32>>` | — | unchanged |
| `get_admin` | no | none | `Option<Address>` | — | unchanged |
| `get_attestor` | no | none | `Option<Address>` | — | unchanged |

**Removed in v0.2** (do not use — the functions no longer exist):
`get_attestations(wallet) -> Vec<Attestation>`, `bump_wallet_ttl(wallet)`.
