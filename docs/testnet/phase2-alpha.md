# Testnet Alpha — evidence record (Phase 2)

**Scope:** prove that the exact committed WASM runs correctly on Stellar
**testnet** and that a full contributor-reputation lifecycle works with
throwaway identities.

**This is testnet only.** It is not a security audit, not a mainnet
deployment, and not a mainnet-readiness claim. All accounts are
disposable and friendbot-funded. No secret keys, seed phrases, `.env`
values, CLI keystore files, or authorization-entry material appear in
this document — every value below is public on-chain data.

**This record describes the v0.1 contract**, which is what the instance
below actually runs. v0.1's attestation-storage design has since been
superseded by a local v0.2 candidate (not deployed anywhere) — see
[`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md). This
document is kept unedited as the accurate record of this deployment.

---

## Part A — build & environment

| Field | Value |
|---|---|
| Date (UTC) | 2026-09-01 |
| Git commit SHA (build) | `d030908407deb4acba6cda0a3207b9643e06f11a` |
| Git commit SHA (deploy) | `83e2a12fe5db588aefb9f58508ab1e2e3e170b6c` — contract source (`src/`, `Cargo.toml`, `Cargo.lock`) byte-identical to `d030908`; only `scripts/` and `docs/` differ. A clean rebuild from either commit reproduces the WASM hash below. |
| Working tree | clean at deploy time |
| WASM path | `target/wasm32v1-none/release/proofowl_contracts.wasm` |
| WASM SHA-256 | `d694e0ad3193e3c2782f9c92d9e88ce6a2f4faef545f9df434b01b41ef96dbf1` |
| WASM size | 28124 bytes |
| Build | `cargo build --target wasm32v1-none --release` from a clean tree (`cargo clean` first) |
| rustc | `1.98.0 (88d9e12ae 2026-08-18)` |
| cargo | `1.98.0 (797e8a9bc 2026-08-05)` |
| soroban-sdk | `27.0.6` (from `Cargo.lock`) |
| Stellar CLI | `28.0.0 (300aaf69ab100536678bdb641428b06f06b318ea)` |
| `make check` | PASS (fmt-check, clippy `-D warnings`, wasm release build, `cargo test --all` — 33 unit + 1 integration) |

### Network target (verified via `getNetwork`, not assumed)

| Field | Value |
|---|---|
| Network name | testnet |
| RPC URL | `https://soroban-testnet.stellar.org` |
| Network passphrase | `Test SDF Network ; September 2015` |
| Testnet protocol version at run time | 28 |
| Friendbot | `https://friendbot.stellar.org/` |

The helper scripts refuse to run unless a live `getNetwork` call returns
exactly this passphrase; the mainnet passphrase is positively refused.
Fail-closed behaviour was tested locally (network unset → refuse;
`=mainnet` → refuse; `=testnet` but RPC pointed at a real mainnet Soroban
RPC → refuse, exit 1).

---

## Part B — deployment

| Field | Value |
|---|---|
| Date deployed (UTC) | 2026-09-01T10:40:07Z (ledger close of the create transaction) |
| Deploy command | `stellar contract deploy --wasm <path> --source <admin> --rpc-url https://soroban-testnet.stellar.org --network-passphrase "Test SDF Network ; September 2015" --optimize=false -- --admin <G...> --attestor <G...>` |
| `--optimize=false` | deliberate — deploys the exact bytes whose sha256 is recorded above |
| Admin address (`G...`) — also deployer / fee payer | `GDHGAVUNEGGKBL5Z6PIDK3KXQO42J7SHFIHYYT22W5YCV5UQ6DQV5CY6` |
| Attestor address (`G...`) | `GD4AV554CBCMUXSVKSJG35J6OHJMCYAP56VZEBVBC5YFYPMB7ZSNC3VW` |
| Test wallet address (`G...`) — smoke test only | `GCNHX5ORRQLJOFQELVAXZ3PQMIAQ3B3QLKZQRV6FXGICZEMQRWY3TRKG` |
| WASM upload transaction | `027d42a38976ed2405ad2835b681f2b3cfee6cd9351397b1feba05ac54a67a1d` — status SUCCESS |
| Create + constructor transaction | `8c3fc5e65ff58328f4bef878481b898d73b176466882ad57643099f834b700e2` — status SUCCESS |
| **Contract ID (`C...`)** | **`CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6`** |
| On-chain WASM hash reported by the deploy | `d694e0ad3193e3c2782f9c92d9e88ce6a2f4faef545f9df434b01b41ef96dbf1` — matches the local build |
| `get_admin()` returned | `GDHGAVUNEGGKBL5Z6PIDK3KXQO42J7SHFIHYYT22W5YCV5UQ6DQV5CY6` — matches expected admin |
| `get_attestor()` returned | `GD4AV554CBCMUXSVKSJG35J6OHJMCYAP56VZEBVBC5YFYPMB7ZSNC3VW` — matches expected attestor |

Config verification was done with `scripts/verify_config.sh`, which
re-verifies the network via `getNetwork` before reading.

---

## Part C — end-to-end smoke test

Run via `scripts/smoke_test.sh` on 2026-09-01 (~10:45Z), input namespace
tag `phase2-alpha-1`, exit code 0, all seven steps passed.

### Test inputs (deterministic; each is SHA-256 of a documented public string — no secrets)

| Input | Preimage | Value |
|---|---|---|
| `github_id_hash` | `proofowl:testnet:phase2-alpha-1:github-identity` | `a529febd8d107bb5eebe800683bced0314f3f019f9e5725eb8f36a3fb3f4a78d` |
| `pr_hash` | `github.com/proofowl/testnet-smoke/pull/1\|phase2-alpha-1` | `74a961928a1e62fa4c891459d2c56b75b3488ca11c82209a684de3eb624eaec3` |
| `pr_hash` (invalid-complexity step) | `github.com/proofowl/testnet-smoke/pull/2\|phase2-alpha-1` | `17fccc43b0c805ebcb6f93d94824d34683d3554827e31e0228fc2567313fc536` |
| `repo` | — | `proofowl/testnet-smoke` |

### Results

| # | Step | Expected | Result | Transaction hash |
|---|---|---|---|---|
| 1 | `link_github(wallet, attestor, github_id_hash)` — two-party (`--source <wallet> --auto-sign`) | success; `GithubLinked` event | PASS — event emitted | `15d96990d815a8512e5db184ca34823e813029d9309eb75d43af515658bab531` (SUCCESS) |
| 2 | `submit_attestation(attestor, github_id_hash, "proofowl/testnet-smoke", pr_number=1, issue_id=1, complexity=100, pr_hash)` | success; returns the credited wallet | PASS — returned `GCNHX5OR…RWY3TRKG` | `d4dd54dc916f203d6ecbafdaa792e05bcf6152fae131c2f5ca090d08751fa395` (SUCCESS) |
| 3 | reads | 1 attestation, score 100, both lookups resolve | PASS — `get_attestations` → `[{complexity:100, issue_id:1, pr_hash:74a961…, pr_number:1, repo:"proofowl/testnet-smoke", timestamp:1788259522}]`; `get_reputation_score` → `100`; `get_wallet_for_github` → test wallet; `get_github_for_wallet` → `a529febd…` | — (simulated reads) |
| 4 | `submit_attestation` with `complexity=175` | rejected — `InvalidComplexity` (contract error #8) | PASS — `HostError: Error(Contract, #8)` at simulation; no transaction submitted | — (rejected pre-submit) |
| 5 | `submit_attestation` re-using `pr_hash` (`pr_number=1, issue_id=9, complexity=150`) | rejected — `DuplicateAttestation` (contract error #6) | PASS — `HostError: Error(Contract, #6)` at simulation; no transaction submitted | — (rejected pre-submit) |
| 6 | `unlink_github(wallet, attestor, github_id_hash)` — two-party | success; `GithubUnlinked` event | PASS — event emitted | `1a038c9756644331caa2184e3f5b1a7ebbcd41c56df26b022293e6e3992ec4df` (SUCCESS) |
| 7 | post-unlink reads | link gone (`None`); reputation still 100 on the wallet | PASS — `get_wallet_for_github` → `null`; `get_github_for_wallet` → `null`; `get_reputation_score` → `100` (retained); attestation history still present | — (simulated reads) |

All submitted transactions were confirmed `SUCCESS` via the RPC
`getTransaction` method. Steps 4 and 5 produce no transaction hash by
design: the CLI simulates first, the contract error is raised during
simulation, and a rejected write never becomes a transaction — the
evidence is the `Error(Contract, #8 / #6)` diagnostic event.

### Events observed

- `link_github` → `GithubLinked` topic, data `{ wallet: GCNHX5OR…, github_id_hash: a529febd… }`
- `submit_attestation` → returned the credited wallet address; attestation persisted with `timestamp = 1788259522` (ledger time, contract-set, ≈ 2026-09-01T10:45:22Z)
- `unlink_github` → `GithubUnlinked` topic, data `{ wallet: GCNHX5OR…, github_id_hash: a529febd… }`

---

## Part D — known limitations and issues found

1. **Two-party signing (fixed).** The Phase 1 `smoke_test.sh` used
   `--source <wallet> --sign-with-key <attestor> --auto-sign`; the first
   live run failed with `TxBadAuth` because on Stellar CLI 28
   `--sign-with-key` replaces the envelope signer. The working form is
   `--source <wallet> --auto-sign` (no `--sign-with-key`); `--auto-sign`
   signs the attestor's non-root Soroban auth entry from the keystore.
   Fixed in this repo; see `docs/operations/testnet-deployment.md` §7.
2. **Default WASM optimization (fixed pre-deploy).** `stellar contract
   deploy` optimizes the WASM by default, which would put different bytes
   on-chain than the recorded hash. The deploy script now passes
   `--optimize=false`.
3. **Manual multi-auth research transactions.** While finding the correct
   signing form, one throwaway `link_github` (`c9e064c05988728f7a1aa6592867fae0339c0464ef8b83355bcdf3128e756ad3`)
   and its `unlink_github`
   (`9732d681c5eaa07fd0e6b36ac622ac8dbf238b2260ec7ea173aedc82bea930c3`)
   were submitted with the same `phase2-alpha-1` `github_id_hash` and
   then reverted, leaving no link. These are testnet-only public
   transactions, listed here for completeness; they are not part of the
   scripted evidence run in Part C.
4. **Single trusted attestor / no upgrade path / lost-key recovery
   deferred.** Unchanged from `SECURITY.md` §7 — none of these were
   addressed in Phase 2 and none are mainnet-ready.
5. **Environment.** The Stellar CLI was not preinstalled; the `28.0.0`
   prebuilt release binary was installed locally. Three throwaway
   identities were generated and friendbot-funded; their secret keys
   live only in the local Stellar CLI keystore and are not in this repo.

---

## Part E — statement

- This exercise was performed **only on Stellar testnet** using
  disposable, friendbot-funded identities, after explicit approval.
- It is **not** a security audit and makes **no** claim about mainnet
  readiness. Mainnet remains out of scope (see `PRODUCTION_READINESS.md`).
- No secret keys, seed phrases, environment values, CLI keystore files,
  or authorization entries were recorded here or committed to Git.
