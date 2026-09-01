# Testnet Alpha — evidence record (Phase 2)

**Scope:** prove that the exact committed WASM runs correctly on Stellar
**testnet** and that a full contributor-reputation lifecycle works with
throwaway identities.

**This is testnet only.** It is not an audit, not a mainnet deployment,
and not a mainnet-readiness claim. All accounts are disposable and
friendbot-funded. No secret material appears in this document.

---

## Part A — build & environment (pre-deployment)

| Field | Value |
|---|---|
| Date started (UTC) | 2026-09-01 |
| Git commit SHA | `d030908407deb4acba6cda0a3207b9643e06f11a` |
| Git commit (short) | `d030908` |
| Working tree | clean (`git status --porcelain` empty) |
| WASM path | `target/wasm32v1-none/release/proofowl_contracts.wasm` |
| WASM SHA-256 | `d694e0ad3193e3c2782f9c92d9e88ce6a2f4faef545f9df434b01b41ef96dbf1` |
| WASM size | 28124 bytes |
| Build | `cargo build --target wasm32v1-none --release` from a clean tree (`cargo clean` first) |
| rustc | `1.98.0 (88d9e12ae 2026-08-18)` |
| cargo | `1.98.0 (797e8a9bc 2026-08-05)` |
| soroban-sdk | `27.0.6` (from `Cargo.lock`) |
| Stellar CLI | `28.0.0 (300aaf69ab100536678bdb641428b06f06b318ea)` |
| `make check` | PASS (fmt-check, clippy `-D warnings`, wasm release build, `cargo test --all` — 33 unit + 1 integration) |

### Network target (verified, not assumed)

| Field | Value |
|---|---|
| Network name | testnet |
| RPC URL | `https://soroban-testnet.stellar.org` |
| Network passphrase | `Test SDF Network ; September 2015` |
| Verified via | JSON-RPC `getNetwork` on the RPC above |
| Testnet protocol version (at time of run) | 28 |
| Friendbot | `https://friendbot.stellar.org/` |

The helper scripts refuse to run unless a live `getNetwork` call returns
exactly this passphrase; the mainnet passphrase is positively refused.

---

## Part B — deployment

_Pending explicit deployment approval. Filled in after the deploy
command runs._

| Field | Value |
|---|---|
| Date deployed (UTC) | _pending_ |
| Deployer / admin address (`G...`) | _pending_ |
| Attestor address (`G...`) | _pending_ |
| Test wallet address (`G...`) | _pending_ |
| Deploy transaction hash | _pending_ |
| Contract ID (`C...`) | _pending_ |
| `get_admin()` returned | _pending_ (expected: the admin address above) |
| `get_attestor()` returned | _pending_ (expected: the attestor address above) |

---

## Part C — end-to-end smoke test

_Pending deployment._ Deterministic dummy inputs (namespace tag
`phase2-alpha-1`), all derived by SHA-256 of a documented public string —
no secrets:

- `github_id_hash = SHA-256("proofowl:testnet:phase2-alpha-1:github-identity")`
- `pr_hash        = SHA-256("github.com/proofowl/testnet-smoke/pull/1|phase2-alpha-1")`
- `pr_hash (bad)  = SHA-256("github.com/proofowl/testnet-smoke/pull/2|phase2-alpha-1")`
- `repo = "proofowl/testnet-smoke"`

| # | Step | Expected | Result | Tx hash |
|---|---|---|---|---|
| 1 | `link_github` (wallet + attestor, two-party) | success | _pending_ | _pending_ |
| 2 | `submit_attestation` (attestor, complexity 100) | success | _pending_ | _pending_ |
| 3 | `get_attestations` / `get_reputation_score` / `get_wallet_for_github` / `get_github_for_wallet` | 1 entry, score 100, reverse+forward lookups resolve | _pending_ | (reads) |
| 4 | `submit_attestation` with complexity 175 | rejected — `InvalidComplexity` (#8) | _pending_ | _pending_ |
| 5 | `submit_attestation` re-using `pr_hash` | rejected — `DuplicateAttestation` (#6) | _pending_ | _pending_ |
| 6 | `unlink_github` (wallet + attestor, two-party) | success | _pending_ | _pending_ |
| 7 | `get_wallet_for_github` / `get_github_for_wallet` / `get_reputation_score` | link gone (None), score still 100 | _pending_ | (reads) |

### Events observed

_pending_

---

## Part D — statement

- This exercise was performed **only on Stellar testnet** using
  disposable, friendbot-funded identities.
- It is **not** a security audit and makes **no** claim about mainnet
  readiness. Mainnet is out of scope (see `PRODUCTION_READINESS.md`).
- No secret keys, seed phrases, environment values, or authorization
  entries were recorded here or committed to Git.
