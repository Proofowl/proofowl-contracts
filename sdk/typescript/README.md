# @proofowl/contract-sdk

Typed, read-only client and canonical identifier helpers for the
ProofOwl Soroban contract — **v0.2** (paginated attestation storage).
Written for the future `proofowl-backend` and `proofowl-frontend`
repositories (neither exists yet).

**v0.2 breaking change from v0.1:** `getAttestations` (unbounded — one
call returning a wallet's entire history) and `prepareBumpWalletTtl`
(unbounded TTL refresh) are removed; the contract functions they
wrapped no longer exist, because they had no ceiling on cost or
response size (`docs/adr/0004-paginated-attestation-storage.md`,
`docs/security/resource-profile-v1.md`). Use `getAttestationCount` /
`getAttestation` / `getAttestationsPage`, and
`prepareBumpWalletCoreTtl` / `prepareBumpAttestationsTtlPage`, instead.
This SDK targets **v0.2 only** — see
[`../../docs/migrations/v0.1-to-v0.2.md`](../../docs/migrations/v0.1-to-v0.2.md).
No v0.2 contract has been deployed to any network as of this SDK
version; there is no testnet/mainnet contract ID to configure yet.

**What this SDK does NOT do:** it never signs a transaction, never
submits one, and never touches a local keystore. Mutating calls are
returned as **unsigned** `AssembledTransaction`s for the caller to sign
and submit with its own signer.

The deployed contract WASM / ABI is authoritative. See
[`../../docs/integration/contract-api-v2.md`](../../docs/integration/contract-api-v2.md).

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

- Node **>= 22.6** (uses the built-in test runner). CI uses Node 24;
  last verified locally with Node **24.20.0** / npm **11.19.0** (the
  `packageManager` pin in `package.json`).
- Package manager: **npm** (a `package-lock.json` is committed; `npm ci`
  installs it verbatim).
- For `npm run generate` only: the [Stellar CLI](https://developers.stellar.org/docs/tools/developer-tools/cli/stellar-cli)
  and a Rust toolchain (**1.91+**, see the repo root `CONTRIBUTING.md`)
  to build the WASM.

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
import { createReadClient } from "@proofowl/contract-sdk";

// No v0.2 instance is deployed yet (docs/migrations/v0.1-to-v0.2.md) --
// pass your own { contractId, rpcUrl, networkPassphrase } once one exists.
// `TESTNET_ALPHA_EXAMPLE` still exists but points at the v0.1 instance,
// which does not speak this v0.2 ABI; do not use it with this client.
const client = createReadClient(config);

const admin = await client.getAdmin(); // string | null
const attestor = await client.getAttestor(); // string | null
const score = await client.getReputationScore("G...WALLET"); // number, O(1)
const wallet = await client.getWalletForGithub(githubIdHash); // string | null
```

### Paginated attestation history (v0.2)

```ts
import {
  createReadClient,
  MAX_PAGE_SIZE,
  parseProofOwlError,
  ProofOwlErrorCode,
} from "@proofowl/contract-sdk";

const client = createReadClient(config);

const count = await client.getAttestationCount("G...WALLET"); // number

// One entry by its zero-based sequence.
const first = await client.getAttestation("G...WALLET", 0); // AttestationView

// A bounded page (limit must be 1..=MAX_PAGE_SIZE, enforced client-side
// too, before any round-trip).
const page = await client.getAttestationsPage("G...WALLET", 0, MAX_PAGE_SIZE);

// Sweep the whole history in pages -- the pattern any indexer or
// frontend "load more" button should use instead of one big call.
async function fetchFullHistory(wallet: string) {
  const all = [];
  let start = 0;
  for (;;) {
    const p = await client.getAttestationsPage(wallet, start, MAX_PAGE_SIZE);
    all.push(...p);
    if (p.length < MAX_PAGE_SIZE) break; // reached the end
    start += p.length;
  }
  return all;
}

// `start` beyond the wallet's count throws; parse it like any other
// contract error.
try {
  await client.getAttestation("G...WALLET", 9999);
} catch (err) {
  if (parseProofOwlError(err) === ProofOwlErrorCode.SequenceOutOfRange) {
    // expected: that sequence does not exist for this wallet
  } else {
    throw err;
  }
}
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
//      GitHub OAuth / challenge verification (see attestor-protocol-v2.md);
//   3. whoever holds the fully-signed tx submits it.
// This SDK performs none of steps 1-3.
```

`prepareUnlinkGithub` has the same shape. `prepareSubmitAttestation`,
`prepareBumpWalletCoreTtl`, `prepareBumpAttestationsTtlPage`, and
`prepareSetAttestor` are single-signer and return the unsigned
`AssembledTransaction` directly.

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
`get_attestor`, and a count/score lookup for an address with no
history) against a live **v0.2** instance. No v0.2 instance has been
deployed to any network yet
(`../../docs/migrations/v0.1-to-v0.2.md`), so this test is **always
skipped** until both `PROOFOWL_INTEGRATION=1` and
`PROOFOWL_V2_CONTRACT_ID=C...` are supplied — it deliberately does not
default to the v0.1 testnet alpha instance, which does not speak this
client's ABI:

```
PROOFOWL_INTEGRATION=1 PROOFOWL_V2_CONTRACT_ID=C... npm run test:integration
# or: make sdk-integration-testnet (same env vars)
```

It never signs or submits anything.
