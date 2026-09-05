/**
 * READ-ONLY integration check against a live **v0.2** contract
 * instance. Never signs or submits anything even when it runs.
 *
 * v0.2 changed the ABI (`docs/adr/0004-paginated-attestation-storage.md`):
 * `get_attestations` and `bump_wallet_ttl` no longer exist in this
 * client. As of this SDK version **no v0.2 instance has been deployed
 * to any network** (`docs/migrations/v0.1-to-v0.2.md`) — deploying one
 * requires a separate, explicit approval this phase's rules do not
 * grant. The v0.1 testnet alpha instance still exists but does not
 * speak this v0.2 client's ABI, so this file deliberately does NOT
 * hardcode `TESTNET_ALPHA_EXAMPLE` (or any other contract id) the way
 * the v0.1 version of this file did.
 *
 * Skipped unless BOTH `PROOFOWL_INTEGRATION=1` and
 * `PROOFOWL_V2_CONTRACT_ID` are set — i.e. always skipped today, since
 * no v0.2 contract id exists to supply. Once a v0.2 instance is
 * deployed under its own approval, point this at it with:
 *
 *   PROOFOWL_INTEGRATION=1 PROOFOWL_V2_CONTRACT_ID=C... \
 *     [PROOFOWL_V2_RPC_URL=...] [PROOFOWL_V2_NETWORK_PASSPHRASE=...] \
 *     npm run test:integration
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { createReadClient } from "./client.js";
import type { ProofOwlContractConfig } from "./config.js";

const V2_CONTRACT_ID = process.env.PROOFOWL_V2_CONTRACT_ID;
const ENABLED = process.env.PROOFOWL_INTEGRATION === "1" && !!V2_CONTRACT_ID;
const SKIP_REASON =
  "set PROOFOWL_INTEGRATION=1 and PROOFOWL_V2_CONTRACT_ID=C... to run " +
  "(no v0.2 instance is deployed to any network yet -- see docs/migrations/v0.1-to-v0.2.md)";

function configFromEnv(): ProofOwlContractConfig {
  return {
    contractId: V2_CONTRACT_ID as string,
    rpcUrl: process.env.PROOFOWL_V2_RPC_URL ?? "https://soroban-testnet.stellar.org",
    networkPassphrase:
      process.env.PROOFOWL_V2_NETWORK_PASSPHRASE ?? "Test SDF Network ; September 2015",
  };
}

test(
  "v0.2 instance: get_admin / get_attestor resolve to non-null addresses",
  { skip: ENABLED ? false : SKIP_REASON },
  async () => {
    const client = createReadClient(configFromEnv());
    const [admin, attestor] = await Promise.all([client.getAdmin(), client.getAttestor()]);
    assert.equal(typeof admin, "string", "instance must be initialized");
    assert.equal(typeof attestor, "string", "instance must be initialized");
  },
);

test(
  "v0.2 instance: a fresh, never-used address has zero attestations and zero score",
  { skip: ENABLED ? false : SKIP_REASON },
  async () => {
    const client = createReadClient(configFromEnv());
    // The all-zero ed25519 public key: a deterministic, valid strkey
    // that has never interacted with the contract.
    const nobody = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const [count, score] = await Promise.all([
      client.getAttestationCount(nobody),
      client.getReputationScore(nobody),
    ]);
    assert.equal(count, 0);
    assert.equal(score, 0);
  },
);
