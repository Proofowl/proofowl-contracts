# Phase 2 (Testnet Alpha) — retrospective

Companion to the evidence record in
[`phase2-alpha.md`](./phase2-alpha.md). Testnet only; not an audit.

**This record describes the v0.1 contract.** A later phase measured a
hard attestation-storage ceiling this retrospective could not have
known about (nothing here predicted it), then fixed it in a local v0.2
candidate not yet deployed anywhere — see
[`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md) and
[`../security/resource-profile-v2.md`](../security/resource-profile-v2.md).
This document is kept unedited as the accurate record of what was true
when it was written.

## What succeeded

- **The exact committed WASM runs on testnet.** The on-chain code hash
  (`d694e0ad…ef96dbf1`) is byte-identical to the local
  `cargo build --target wasm32v1-none --release` output, deployed with
  `--optimize=false`.
- **The deploy-time constructor works as designed.** `get_admin` /
  `get_attestor` return the constructor arguments; there is no separate
  `init` step and nothing to front-run.
- **The full contributor-reputation lifecycle works end to end:**
  two-party `link_github`, `submit_attestation`, all read paths,
  two-party `unlink_github`, and — importantly — reputation stays
  attached to the wallet after the link is removed.
- **The negative paths fail correctly on-chain:** an out-of-set
  `complexity` is rejected with `InvalidComplexity` (#8) and a re-used
  `pr_hash` with `DuplicateAttestation` (#6), both during simulation, so
  no bad transaction is ever submitted.
- **The hardened guardrails held.** Every script verified the network by
  a live `getNetwork` call before acting; the fail-closed checks
  (unset / mainnet / testnet-name-but-mainnet-RPC) all refused.

## What was difficult

- **Two-party Soroban auth on the CLI.** The Phase 1 script guessed
  `--source <wallet> --sign-with-key <attestor> --auto-sign`. The first
  live call failed with `TxBadAuth`: on Stellar CLI 28 `--sign-with-key`
  *replaces* the envelope signer, so the wallet never signed the
  envelope. The working form is `--source <wallet> --auto-sign` with no
  `--sign-with-key` — `--auto-sign` signs the non-root auth entry by
  matching its address to a keystore identity. This cost one round of
  live debugging and two throwaway transactions (documented in
  `phase2-alpha.md` Part D).
- **`stellar contract deploy` optimizes by default**, which would have
  put different bytes on-chain than the recorded hash. Caught by a
  `--build-only` dry run before the real deploy; fixed with
  `--optimize=false`.
- **Nothing about the CLI multi-auth behaviour is obvious from `--help`.**
  It had to be confirmed by decoding built transactions and inspecting
  the auth-entry credential types.

## Operational gaps found (and their status)

| Gap | Status |
|---|---|
| Scripts only string-matched `STELLAR_NETWORK=testnet`; no real network check | **Fixed** — `require_verified_testnet()` calls `getNetwork`, refuses mainnet, fails closed |
| Deploy could ship optimized (non-matching) bytes | **Fixed** — `--optimize=false` in `deploy_testnet.sh` |
| Two-party signing command was wrong | **Fixed** — script + ops guide §7 corrected and verified live |
| Smoke test covered only the happy path | **Fixed** — now 7 steps incl. both expected-failure paths and post-unlink retention |
| Stellar CLI not present in the environment | Installed locally (28.0.0); still a per-operator setup step, documented |
| GitHub Actions pinned to major tags, not SHAs | **Open** — carried from Phase 1 (`PRODUCTION_READINESS.md` gate 2.5) |
| No standing testnet instance / monitoring / TTL keep-alive job | **Open** — out of scope for an alpha; needed before Phase 3 |
| `set_attestor` rotation not exercised on-chain | **Open** — not part of the Phase 2 smoke test |

## Criteria to advance to Phase 3

Phase 3 (integration alpha with a real backend/attestor) may begin only
when **all** of the following are true:

1. **Backend exists.** `proofowl-backend` performs a real GitHub OAuth /
   challenge flow and holds the attestor signing key on separate
   infrastructure (not an operator laptop keystore).
2. **Canonical `pr_hash` derivation is implemented and tested** in the
   backend exactly as specified in `README.md` / `SECURITY.md`
   (`SHA-256` of the normalized `github.com/<owner>/<repo>/pull/<n>`),
   with a fixture test.
3. **`set_attestor` rotation is exercised on testnet** end to end
   (admin rotates the attestor key; old key rejected, new key accepted)
   and added to the smoke test.
4. **A standing testnet instance** is deployed from a tagged commit,
   recorded in `docs/testnet/`, with a scheduled `bump_wallet_ttl`
   keep-alive and event monitoring in place.
5. **An event indexer** (or a documented stand-in) consumes
   `GithubLinked` / `AttestationRecorded` / `GithubUnlinked` and can
   reconstruct a passport, proving the stored data is sufficient.
6. **Internal security review** of the contract + backend trust boundary
   is written up (still short of a third-party audit, which remains a
   mainnet prerequisite).
7. **CI hardening follow-ups closed:** GitHub Actions pinned to commit
   SHAs; `deny.toml` advisory exceptions reviewed and still minimal.
8. `make check` green and a fresh `cargo build --target wasm32v1-none
   --release` reproduces the deployed WASM hash for the Phase 3 commit.

Phase 3 does **not** include mainnet. Mainnet criteria are in
`PRODUCTION_READINESS.md` Gate 6 and are unchanged.
