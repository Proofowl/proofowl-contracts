/**
 * READ-ONLY integration check against the public Phase 2 testnet alpha
 * instance. It calls only view methods (`get_admin`, `get_attestor`,
 * `get_reputation_score` for an address expected to have no history) and
 * NEVER signs or submits anything.
 *
 * Skipped unless `PROOFOWL_INTEGRATION=1` is set, so the default
 * `npm test` stays fully offline. Run it with `npm run test:integration`
 * (or `make sdk-integration-testnet`).
 */

import { test } from "node:test";
import assert from "node:assert/strict";

import { createReadClient } from "./client.js";
import { TESTNET_ALPHA_EXAMPLE } from "./config.js";

const ENABLED = process.env.PROOFOWL_INTEGRATION === "1";

// Public Phase 2 testnet alpha values (docs/testnet/phase2-alpha.md).
const EXPECTED_ADMIN = "GDHGAVUNEGGKBL5Z6PIDK3KXQO42J7SHFIHYYT22W5YCV5UQ6DQV5CY6";
const EXPECTED_ATTESTOR = "GD4AV554CBCMUXSVKSJG35J6OHJMCYAP56VZEBVBC5YFYPMB7ZSNC3VW";

test(
  "testnet alpha: get_admin / get_attestor match the documented addresses",
  { skip: ENABLED ? false : "set PROOFOWL_INTEGRATION=1 to run" },
  async () => {
    const client = createReadClient(TESTNET_ALPHA_EXAMPLE);
    const [admin, attestor] = await Promise.all([client.getAdmin(), client.getAttestor()]);
    assert.equal(admin, EXPECTED_ADMIN, "on-chain admin");
    assert.equal(attestor, EXPECTED_ATTESTOR, "on-chain attestor");
  },
);

test(
  "testnet alpha: a fresh address has an empty history and zero score",
  { skip: ENABLED ? false : "set PROOFOWL_INTEGRATION=1 to run" },
  async () => {
    const client = createReadClient(TESTNET_ALPHA_EXAMPLE);
    // The all-zero ed25519 public key: a deterministic, valid strkey
    // that has never interacted with the contract.
    const nobody = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
    const [history, score] = await Promise.all([
      client.getAttestations(nobody),
      client.getReputationScore(nobody),
    ]);
    assert.deepEqual(history, []);
    assert.equal(score, 0);
  },
);
