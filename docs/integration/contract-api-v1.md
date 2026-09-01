# ProofOwl contract API — integration spec v1

Status: **stable for the current contract**. Versioned as `v1`; a new
file (`contract-api-v2.md`) will be added if the contract's public
interface changes (see [`RELEASE_POLICY.md`](../RELEASE_POLICY.md) for
what counts as a breaking change).

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

- Contract crate version: `0.1.0` (`Cargo.toml`)
- soroban-sdk: `27.0.6`
- Reference build WASM SHA-256:
  `d694e0ad3193e3c2782f9c92d9e88ce6a2f4faef545f9df434b01b41ef96dbf1`
- Testnet alpha instance (disposable, may be replaced):
  `CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6`

## Conventions used below

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

Returned by `get_attestations`. Field order in the ABI map is
alphabetical; integrators must key by name, not position.

| Field | Type | Notes |
|---|---|---|
| `repo` | `string` | `"<owner>/<repo>"`, lowercased by the backend before submission by convention (the contract stores it verbatim). |
| `pr_number` | `u32` | GitHub pull-request number. |
| `issue_id` | `u64` | Stellar Wave issue id the contribution resolved. `0` if not applicable. |
| `complexity` | `u32` | One of `0`, `100`, `150`, `200`. `0` = "confirmed, tier unknown". |
| `pr_hash` | `BytesN<32>` | SHA-256 of the canonical PR identifier — see [`identifier-spec-v1.md`](./identifier-spec-v1.md). Global de-dup key. |
| `timestamp` | `u64` | Ledger close time (Unix seconds) when the attestation was recorded. **Set by the contract**, not the caller. |

### `github_id_hash`

`BytesN<32>` — SHA-256 of the canonical GitHub *numeric user id*
identifier. Opaque to the contract. Construction and caveats:
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

Two error surfaces to distinguish when mapping:

- **Contract error** (codes above): `Error(Contract, #N)` — deterministic, business logic.
- **Host auth error**: raised when a required `require_auth()` signature is absent or invalid (`Error(Auth, …)`). Not one of the codes above.

## Two-party authorization

`link_github` and `unlink_github` each call `require_auth()` on **two
independent addresses**: the contributor `wallet` and the trusted
`attestor`. **A single ordinary wallet signature cannot complete these
calls**, and neither can a single attestor signature. Both parties must
authorize the *same* invocation.

On the Stellar CLI (v28), the working form is
`--source <wallet> --auto-sign` (no `--sign-with-key`): `--source` signs
the envelope + the wallet's root auth entry, `--auto-sign` signs the
attestor's non-root Soroban auth entry from the keystore. In a
frontend/backend split:

1. The frontend builds the `AssembledTransaction` and collects the
   contributor's wallet auth-entry signature.
2. The backend adds the attestor auth-entry signature (only after its
   own GitHub OAuth/challenge verification succeeds — see
   [`attestor-protocol-v1.md`](./attestor-protocol-v1.md)).
3. Whoever holds the fully-signed transaction submits it.

The order of the two auth-entry signatures does not matter; both must be
present before submission. See
[`../operations/testnet-deployment.md`](../operations/testnet-deployment.md)
§7 for the exact CLI sequence and the `stellar tx` fallback.

---

## Functions

Each entry lists: signature · caller/auth · mutability · errors · events
· storage/TTL · backend notes · frontend notes.

### `__constructor(admin: Address, attestor: Address)`

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
- **Backend notes:** the attestor address passed here is the key the
  backend must control to co-sign links and submit attestations. Record
  it; it can later change via `set_attestor`.
- **Frontend notes:** none — frontends never deploy.

### `set_attestor(admin: Address, new_attestor: Address) -> Result<()>`

- **Caller / auth:** the stored `admin`. Calls `admin.require_auth()`
  **and** checks `admin == stored Admin`.
- **Mutability:** writes `Attestor` in instance storage.
- **Returns:** `Ok(())`.
- **Errors:** `NotInitialized` (2), `Unauthorized` (3, if the supplied
  `admin` is not the stored admin).
- **Events:** `AttestorRotated` — topics `["attestor_rotated", admin]`,
  data `{ new_attestor }`.
- **Storage / TTL:** extends the instance TTL.
- **Backend notes:** after a rotation the **old** attestor key is
  immediately rejected by `link_github` / `submit_attestation`. Plan a
  cutover: bring the new key online, rotate, then retire the old key.
  Watch for `AttestorRotated` to detect an out-of-band rotation.
- **Frontend notes:** if a link/attestation flow starts failing with
  `Unauthorized`, check whether the attestor rotated.

### `link_github(wallet: Address, attestor: Address, github_id_hash: BytesN<32>) -> Result<()>`

- **Caller / auth:** **two-party** — `wallet.require_auth()` **and**
  `attestor.require_auth()`, plus `attestor == stored Attestor`.
- **Mutability:** writes `WalletLink(wallet) = github_id_hash` and
  `GithubLink(github_id_hash) = wallet` (both directions).
- **Returns:** `Ok(())`.
- **Errors:** `NotInitialized` (2), `Unauthorized` (3, wrong attestor),
  `WalletAlreadyLinked` (4), `GithubAlreadyLinked` (5). A missing
  wallet **or** attestor signature is a host auth error.
- **Events:** `GithubLinked` — topics `["github_linked", wallet]`, data
  `{ github_id_hash }`.
- **Storage / TTL:** creates two persistent entries and extends their
  TTL plus the instance TTL.
- **Backend notes:** only co-sign after the contributor has proven
  control of the GitHub numeric id behind `github_id_hash` via your
  OAuth/challenge flow. The contract does **not** verify this. One
  wallet ↔ one identity, enforced both ways; a re-link needs an
  `unlink_github` first.
- **Frontend notes:** you can *prepare* and collect the wallet's auth
  signature, but the transaction cannot be submitted until the backend
  adds the attestor's signature. Present this as a two-step flow, not a
  single "sign to link" button.

### `unlink_github(wallet: Address, attestor: Address, github_id_hash: BytesN<32>) -> Result<()>`

- **Caller / auth:** **two-party** — the *currently linked* `wallet` and
  the stored `attestor`.
- **Mutability:** removes both `WalletLink(wallet)` and
  `GithubLink(github_id_hash)`.
- **Returns:** `Ok(())`.
- **Errors:** `NotInitialized` (2), `Unauthorized` (3), `LinkNotFound`
  (9, if the pair is not a consistent existing link in both
  directions).
- **Events:** `GithubUnlinked` — topics `["github_unlinked", wallet]`,
  data `{ github_id_hash }`.
- **Storage / TTL:** deletes the two link entries; extends the instance
  TTL. **Does not touch** `Attestations(wallet)` or any `SeenPr`
  marker.
- **Backend notes:** after unlink the wallet keeps its attestation
  history and score; the `github_id_hash` becomes free to link to a
  different wallet (after a fresh OAuth proof). A spent `pr_hash` stays
  spent. There is no admin override — losing the wallet key means the
  link cannot be removed (documented limitation, `SECURITY.md` §4.2).
- **Frontend notes:** same two-step signing shape as `link_github`.

### `submit_attestation(attestor, github_id_hash, repo, pr_number, issue_id, complexity, pr_hash) -> Result<Address>`

- **Signature:** `submit_attestation(attestor: Address, github_id_hash:
  BytesN<32>, repo: string, pr_number: u32, issue_id: u64, complexity:
  u32, pr_hash: BytesN<32>) -> Result<Address>`
- **Caller / auth:** the stored `attestor` only —
  `attestor.require_auth()` and `attestor == stored Attestor`. **Not**
  two-party; the contributor does not sign an attestation.
- **Mutability:** appends an `Attestation` to `Attestations(wallet)` and
  writes `SeenPr(pr_hash)`. The credited `wallet` is resolved from
  `GithubLink(github_id_hash)` — the attestor never names the wallet.
- **Returns:** `Ok(wallet)` — the address credited.
- **Errors:** `NotInitialized` (2), `Unauthorized` (3),
  `InvalidComplexity` (8, checked first), `WalletNotLinked` (7, no link
  for that identity), `DuplicateAttestation` (6, `pr_hash` already
  seen). Evaluation order: auth → complexity → wallet resolution →
  dedup.
- **Events:** `AttestationRecorded` — topics
  `["attestation_recorded", wallet]`, data `{ repo, pr_number,
  issue_id, complexity, pr_hash, timestamp }`.
- **Storage / TTL:** extends the TTL of the history vector, the new
  `SeenPr` marker, the `GithubLink`, the `WalletLink`, and the
  instance.
- **Backend notes:** derive `pr_hash` **exactly** per
  [`identifier-spec-v1.md`](./identifier-spec-v1.md). Treat
  `DuplicateAttestation` as success-equivalent for idempotency (the PR
  is already recorded). If you get `WalletNotLinked`, hold the verified
  fact in your own queue and submit once the contributor links — do not
  invent a placeholder wallet. `repo` + `pr_number` are stored so an
  indexer can rebuild the PR URL; keep them consistent with `pr_hash`.
- **Frontend notes:** read-only consumers never call this. To display
  "pending" contributions, the frontend must ask the backend, not the
  chain.

### `bump_wallet_ttl(wallet: Address) -> ()`

- **Caller / auth:** **anyone**, no `require_auth()`. Permissionless.
- **Mutability:** none to data. Extends TTLs only.
- **Returns:** nothing. Infallible (no `Result`). No-op for an unlinked
  wallet with no history.
- **Errors:** none typed.
- **Events:** none.
- **Storage / TTL:** extends the TTL of `WalletLink(wallet)`, the
  `GithubLink` it points at, `Attestations(wallet)`, **every**
  `SeenPr(pr_hash)` in that history, and the instance. Cost scales with
  the number of attestations for the wallet.
- **Backend notes:** run this on a schedule for active passports (see
  [`event-indexer-v1.md`](./event-indexer-v1.md) "TTL maintenance").
  It is still a state-changing transaction that costs a fee even though
  it changes no data.
- **Frontend notes:** safe to expose as a "keep my passport alive"
  action; it never mutates a user's data and needs only the caller's
  own signature (as tx source), not the wallet owner's.

### `get_attestations(wallet: Address) -> Vec<Attestation>`

- **Caller / auth:** none. Read-only.
- **Returns:** array of [`Attestation`](#attestation), in insertion
  order (oldest first). Empty array if the wallet has none.
- **Errors:** none.
- **Events / storage:** none (a simulation does not extend TTL).
- **Backend / frontend notes:** authoritative history for a wallet.
  Prefer this over indexer state when they disagree. Large histories
  cost more to read (single-vector storage — `SECURITY.md` §7).

### `get_reputation_score(wallet: Address) -> u32`

- **Caller / auth:** none. Read-only.
- **Returns:** `Σ points` over the wallet's attestations, where
  `points = complexity` if `complexity > 0`, else `50` (the
  "unverified tier" base). `saturating_add` — never panics, never
  wraps.
- **Errors:** none.
- **Backend / frontend notes:** cheap headline number. It is fully
  derivable from `get_attestations`; use whichever fits your call
  budget. `0` for an unknown or empty wallet.

### `get_wallet_for_github(github_id_hash: BytesN<32>) -> Option<Address>`

- **Caller / auth:** none. Read-only.
- **Returns:** the linked wallet, or `None` if that identity hash is not
  linked.
- **Backend / frontend notes:** forward resolution — "which wallet owns
  this GitHub identity". This is the exact lookup `submit_attestation`
  does internally.

### `get_github_for_wallet(wallet: Address) -> Option<BytesN<32>>`

- **Caller / auth:** none. Read-only.
- **Returns:** the linked `github_id_hash`, or `None`.
- **Backend / frontend notes:** reverse resolution. Returns the opaque
  hash, not a username — you cannot recover the GitHub id from it.

### `get_admin() -> Option<Address>`

- **Caller / auth:** none. Read-only.
- **Returns:** the stored admin, or `None` if the instance is
  uninitialized/archived.
- **Backend / frontend notes:** use as a lightweight liveness/identity
  check for a contract id + network pair before trusting it.

### `get_attestor() -> Option<Address>`

- **Caller / auth:** none. Read-only.
- **Returns:** the current attestor, or `None`.
- **Backend / frontend notes:** the backend should assert this equals
  the key it holds at startup and after any `AttestorRotated` event.

---

## Quick reference

| Function | Mutating | Auth | Returns | Typed errors |
|---|---|---|---|---|
| `__constructor` | yes (deploy only) | admin | — | — |
| `set_attestor` | yes | admin | `Result<()>` | 2, 3 |
| `link_github` | yes | **wallet + attestor** | `Result<()>` | 2, 3, 4, 5 |
| `unlink_github` | yes | **wallet + attestor** | `Result<()>` | 2, 3, 9 |
| `submit_attestation` | yes | attestor | `Result<Address>` | 2, 3, 6, 7, 8 |
| `bump_wallet_ttl` | yes (TTL only) | none | — | — |
| `get_attestations` | no | none | `Vec<Attestation>` | — |
| `get_reputation_score` | no | none | `u32` | — |
| `get_wallet_for_github` | no | none | `Option<Address>` | — |
| `get_github_for_wallet` | no | none | `Option<BytesN<32>>` | — |
| `get_admin` | no | none | `Option<Address>` | — |
| `get_attestor` | no | none | `Option<Address>` | — |
