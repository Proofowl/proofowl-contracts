# @proofowl/contract-sdk

Typed, read-only client and canonical identifier helpers for the
ProofOwl Soroban contract. Written for the future `proofowl-backend` and
`proofowl-frontend` repositories (neither exists yet).

**What this SDK does NOT do:** it never signs a transaction, never
submits one, and never touches a local keystore. Mutating calls are
returned as **unsigned** `AssembledTransaction`s for the caller to sign
and submit with its own signer.

The deployed contract WASM / ABI is authoritative. See
[`../../docs/integration/contract-api-v1.md`](../../docs/integration/contract-api-v1.md).

## Layout

```
src/
  generated/index.ts   verbatim output of `stellar contract bindings typescript`
                       (DO NOT EDIT — regenerated from the WASM, drift-checked in CI)
  config.ts            ProofOwlContractConfig + the testnet-alpha EXAMPLE config
  errors.ts            typed error codes + parseProofOwlError()
  client.ts            createReadClient() + prepare*() unsigned-tx helpers
  identifiers.ts       hashGitHubUserIdV1 / normalizeGitHubPullRequest / hashGitHubPullRequestV1
  index.ts             public entry
```

## Requirements

- Node **>= 22.6** (uses the built-in test runner; CI uses Node 24).
- Package manager: **npm** (a `package-lock.json` is committed).
- For `npm run generate` only: the [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli)
  and a Rust toolchain to build the WASM.

## Install & build

```
npm ci            # from the committed lockfile
npm run build     # -> dist/  (tsc)
```

## Checks

```
npm run check     # format:check + lint + typecheck + unit tests (offline)
```

Individually: `npm run format:check`, `npm run lint`, `npm run typecheck`,
`npm test`. From the repo root: `make sdk-test`, `make integration-check`.

## Regenerate the bindings

```
npm run generate  # rebuilds src/generated/index.ts from the contract WASM
```

CI (`sdk-bindings-drift` job) regenerates and fails if the committed file
differs.

## Examples

### Read-only lookup

```ts
import { createReadClient, TESTNET_ALPHA_EXAMPLE } from "@proofowl/contract-sdk";

// TESTNET_ALPHA_EXAMPLE is the disposable Phase 2 instance — an example,
// not a production default. Pass your own { contractId, rpcUrl,
// networkPassphrase } in real code.
const client = createReadClient(TESTNET_ALPHA_EXAMPLE);

const admin = await client.getAdmin(); // string | null
const attestor = await client.getAttestor(); // string | null
const score = await client.getReputationScore("G...WALLET"); // number
const history = await client.getAttestations("G...WALLET"); // AttestationView[]
const wallet = await client.getWalletForGithub(githubIdHash); // string | null
```

### Prepare a two-party link (the SDK does not sign it)

```ts
import { prepareLinkGithub, hashGitHubUserIdV1 } from "@proofowl/contract-sdk";

const githubIdHash = hashGitHubUserIdV1(1024025); // GitHub numeric user id

const { transaction, needsSignatureFrom, invoker } = await prepareLinkGithub(config, {
  wallet: "G...CONTRIBUTOR",
  attestor: "G...ATTESTOR", // must equal the on-chain attestor
  githubIdHash,
});

// `transaction` is UNSIGNED. `needsSignatureFrom` is ["G...ATTESTOR"].
//   1. the contributor signs the envelope + their root auth entry
//      (e.g. via a wallet like Freighter);
//   2. the backend signs the attestor's auth entry — ONLY after its own
//      GitHub OAuth / challenge verification (see attestor-protocol-v1.md);
//   3. whoever holds the fully-signed tx submits it.
// This SDK performs none of steps 1-3.
```

`prepareUnlinkGithub` has the same shape. `prepareSubmitAttestation`,
`prepareBumpWalletTtl`, and `prepareSetAttestor` are single-signer and
return the unsigned `AssembledTransaction` directly.

### Canonical hashes

```ts
import {
  hashGitHubUserIdV1Hex,
  normalizeGitHubPullRequest,
  hashGitHubPullRequestV1Hex,
  verifyAttestationPrHash,
} from "@proofowl/contract-sdk";

hashGitHubUserIdV1Hex(1);
// "ad6494a9db671dce66088a82f8446c464e7d425da57d4eca4081b19a74b1e584"

normalizeGitHubPullRequest("@Stellar", "Soroban-Examples.git", "#42").canonical;
// "github.com/stellar/soroban-examples/pull/42"

hashGitHubPullRequestV1Hex("stellar", "soroban-examples", 42);
// "1eed82536f9e3a9477916599ab2111d9af634b1270f5d4d1d61ee98bd50d6c0e"

// indexer sanity check: recompute pr_hash from an attestation's cleartext fields
verifyAttestationPrHash(att.repo, att.prNumber, att.prHashHex); // boolean
```

Full rules and rejection cases:
[`../../docs/integration/identifier-spec-v1.md`](../../docs/integration/identifier-spec-v1.md).

## Read-only testnet integration test

`src/testnet.integration.test.ts` calls only view methods (`get_admin`,
`get_attestor`, and a history/score lookup for an address with no
history) against the public Phase 2 alpha instance. It is **skipped**
unless `PROOFOWL_INTEGRATION=1`:

```
npm run test:integration      # or: make sdk-integration-testnet
```

It never signs or submits anything.
