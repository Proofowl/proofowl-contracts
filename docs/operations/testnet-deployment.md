# Testnet deployment — operations guide

This guide covers deploying the ProofOwl registry contract to the
**Stellar testnet only**. Mainnet is out of scope for the current phase
(see [`../../PRODUCTION_READINESS.md`](../../PRODUCTION_READINESS.md)).

Nothing in this repository deploys automatically. Every command below is
run by a human operator, deliberately, from a machine that holds the
right keys.

---

## 1. Prerequisites

| Requirement | Notes |
|---|---|
| Rust 1.84+ with `wasm32v1-none` | `rustup target add wasm32v1-none` |
| [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli) | `stellar --version` ≥ 22 (validated on 28.0.0); provides `contract deploy/invoke` and the `tx` subcommands used for multi-signature flows |
| `curl` | the scripts use it to verify the RPC's network passphrase |
| A funded **testnet** deployer account | Fund via <https://friendbot.stellar.org> or `stellar keys fund`. The helper scripts **do not** fund anything. |
| `make check` passing on the exact commit you intend to ship | The lockfile (`Cargo.lock`) is committed, so the WASM is reproducible. |

Configuration is supplied through environment variables. Copy
[`../../.env.example`](../../.env.example) to `.env`, fill it in, and
`source .env` (or `set -a; . ./.env; set +a`) before running a script.
`.env` is git-ignored — never commit a filled-in copy.

---

## 2. Key separation

Use **three distinct keys**. Do not collapse them.

| Role | Holds | Where it should live |
|---|---|---|
| **Deployer** | pays the deploy transaction fee; signs `CreateContract` | an operator laptop / CI runner identity, testnet-only |
| **Admin** | the contract's `admin`; the *only* post-deploy privileged action is `set_attestor` | offline / hardware signer even on testnet, so the habit is right for mainnet |
| **Attestor** | the contract's `attestor`; co-signs identity links and submits attestations | the backend service's signing key (a separate machine/HSM), rotated with `set_attestor` |

The deploy transaction must be signed by **both** the deployer (fee +
create) and the admin (the constructor calls `admin.require_auth()`).
With the Stellar CLI the simplest path is to run `deploy` from the admin
identity so it is both source and authorizer; if you want a separate
fee-payer, use `--source <deployer>` and add the admin as an additional
signer.

Never reuse a mainnet key for testnet. Never paste a secret key on a
command line or into a script — the Stellar CLI keystore holds keys by
*alias*; the scripts and `.env.example` only ever reference aliases and
public `G...` addresses.

---

## 3. Testnet-only guardrails

- **Two-layer network check.** The scripts refuse to run unless
  `STELLAR_NETWORK=testnet` (declared intent) **and** a live
  `getNetwork` call to the RPC they will use reports the exact Stellar
  testnet passphrase, `Test SDF Network ; September 2015`. The mainnet
  passphrase is positively refused. Any error — unreachable RPC, blank
  or unexpected passphrase — aborts. The RPC URL must be `https://`.
- The scripts pass `--rpc-url` and `--network-passphrase` explicitly
  (both taken from that verified `getNetwork` response); they do not
  depend on a named `--network testnet` CLI config existing.
- The scripts never call friendbot, never `stellar keys generate`, and
  never deploy as a side effect of being `source`d — deployment happens
  only when you explicitly run `scripts/deploy_testnet.sh`.
- No secret value is ever printed. Scripts echo aliases, addresses, and
  contract IDs only.
- There is no CI job that deploys. The manual
  `.github/workflows/testnet-release.yml` workflow requires an explicit
  `workflow_dispatch`, defaults `deploy` to `false`, and is gated on a
  protected `testnet` GitHub Environment that a maintainer must configure
  with its own secrets.

---

## 4. Rollback reality — the contract is immutable

This contract has **no upgrade mechanism and no admin kill-switch**. Once
an instance is deployed:

- Its WASM cannot be replaced. There is no `update_current_contract_wasm`
  call and no admin function that swaps code.
- Its stored `admin` cannot be changed. `attestor` can be rotated
  (`set_attestor`), nothing else.
- "Rolling back" a bad deployment means **deploying a fresh instance** at
  a new contract ID and re-pointing every off-chain consumer (backend,
  indexer, frontend, docs) at the new ID. On-chain data in the old
  instance stays where it is; there is no migration path in this phase
  (see `SECURITY.md` §7).

Practical consequences:

- Treat the first real testnet deploy as disposable: expect to throw it
  away at least once while the backend and frontend stabilise.
- Do not advertise a contract ID until you have run the smoke test
  against it and are willing to keep it.
- Keep a short record (date, commit SHA, WASM sha256, contract ID,
  admin/attestor addresses) for every instance you deploy, so a stale ID
  can always be traced back to a commit.

---

## 5. Exact command sequence

All commands assume `.env` is sourced and `STELLAR_NETWORK=testnet`.

### 5.1 Build the release WASM

```
scripts/build_wasm.sh
# equivalent to:
#   cargo build --target wasm32v1-none --release
# prints the artifact path and its sha256
```

### 5.2 Deploy + initialize (one transaction)

```
scripts/deploy_testnet.sh
# equivalent to:
#   stellar contract deploy \
#     --wasm target/wasm32v1-none/release/proofowl_contracts.wasm \
#     --source "$PROOFOWL_ADMIN_IDENTITY" \
#     --rpc-url <verified testnet RPC> \
#     --network-passphrase "Test SDF Network ; September 2015" \
#     -- \
#     --admin "$PROOFOWL_ADMIN_ADDRESS" \
#     --attestor "$PROOFOWL_ATTESTOR_ADDRESS"
```

The constructor runs inside this transaction. There is no second `init`
step. The script prints the new contract ID; record it as
`PROOFOWL_CONTRACT_ID` in your `.env` and in your instance log.

### 5.3 Verify the on-chain configuration

```
scripts/verify_config.sh
# reads get_admin / get_attestor from the deployed contract and
# compares them to PROOFOWL_ADMIN_ADDRESS / PROOFOWL_ATTESTOR_ADDRESS.
# Non-zero exit on any mismatch.
```

### 5.4 Smoke test (writes disposable state to the testnet instance)

```
scripts/smoke_test.sh
# link_github (wallet + attestor co-sign) with a throwaway identity and
# obviously-fake hashes -> submit_attestation -> read get_attestations
# and get_reputation_score -> unlink_github to leave no dangling link.
```

The smoke test needs a disposable, already-funded testnet identity you
created yourself (`PROOFOWL_SMOKE_WALLET_IDENTITY`). It does not create or
fund it.

### 5.5 Record and hand off

- Add the contract ID to your instance log and to `README.md` under
  *Deployed contracts* only once you intend to keep it.
- Give the backend team the `attestor` address and confirm the key
  separation above is real on their side.
- Keep the `admin` key offline. Plan the `set_attestor` rotation to a
  multisig before any mainnet consideration.

---

## 6. If something goes wrong

| Symptom | Likely cause | Action |
|---|---|---|
| `deploy` fails with an auth error | admin identity did not sign / wrong `--source` | re-run with the admin identity as source, or add it as a signer |
| `verify_config.sh` reports a mismatch | deployed against the wrong addresses | the instance is unusable — deploy a fresh one (§4) |
| smoke test `link_github` fails | attestor alias/address mismatch, or the wallet identity is unfunded | check `PROOFOWL_ATTESTOR_*`, fund the smoke wallet |
| you deployed from the wrong commit | — | deploy a fresh instance from the right commit; abandon the old ID |

---

## 7. Two-party authorization on the Stellar CLI

`link_github` and `unlink_github` each call `require_auth()` on **two
independent addresses** — the contributor `wallet` and the `attestor`.
One CLI `--source` argument cannot authorize both: `--source` signs the
transaction envelope and the address's *root* Soroban auth entry only.
The second address's `require_auth()` produces a *non-root* auth entry
that needs its own signature.

**Approach used by `scripts/smoke_test.sh` (single supported CLI
command):**

```
stellar contract invoke --id <C...> \
  --rpc-url <verified testnet RPC> \
  --network-passphrase "Test SDF Network ; September 2015" \
  --source        <wallet-identity>     \   # signs tx + wallet's root auth entry
  --sign-with-key <attestor-identity>   \   # signs the attestor's non-root auth entry
  --auto-sign                           \   # don't prompt for the non-root entry
  -- link_github \
     --wallet <G...wallet> --attestor <G...attestor> \
     --github_id_hash <64-hex>
```

Both identities live in the local Stellar CLI keystore; no secret key is
ever placed on the command line or in a file. `submit_attestation` needs
only the attestor, so it is a plain single-`--source` invoke.

**Fallback (explicit build → sign → send), if a CLI version does not
assemble the non-root entry in one step:**

```
stellar contract invoke ... --source <wallet> --build-only -- link_github ...  > tx.xdr
stellar tx sign --sign-with-key <wallet>              tx.xdr                    > tx1.xdr
stellar tx sign --sign-with-key <attestor> --auto-sign tx1.xdr                 > tx2.xdr
stellar tx send tx2.xdr
```

This uses only `stellar` subcommands — no extra client, no new
dependency, and nothing added to the contract's public API. The
`.xdr` files are transient and must never be committed (they are
covered by `.gitignore`).
