import { test } from "node:test";
import assert from "node:assert/strict";

import {
  assertConfig,
  isMainnet,
  MAINNET_PASSPHRASE,
  TESTNET_ALPHA_EXAMPLE,
  TESTNET_PASSPHRASE,
  type ProofOwlContractConfig,
} from "./config.js";

const OK: ProofOwlContractConfig = {
  contractId: "CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6",
  rpcUrl: "https://soroban-testnet.stellar.org",
  networkPassphrase: TESTNET_PASSPHRASE,
};

test("assertConfig accepts a well-formed testnet config", () => {
  assert.doesNotThrow(() => assertConfig(OK));
});

test("assertConfig rejects a non-contract id", () => {
  assert.throws(() => assertConfig({ ...OK, contractId: "GABC" }), /contract strkey/);
  assert.throws(
    () => assertConfig({ ...OK, contractId: "CCJ7DVU2" }),
    /contract strkey/,
    "too short",
  );
});

test("assertConfig rejects http:// unless allowHttp", () => {
  assert.throws(() => assertConfig({ ...OK, rpcUrl: "http://localhost:8000" }), /https:\/\//);
  assert.doesNotThrow(() =>
    assertConfig({ ...OK, rpcUrl: "http://localhost:8000", allowHttp: true }),
  );
});

test("assertConfig rejects an empty passphrase", () => {
  assert.throws(() => assertConfig({ ...OK, networkPassphrase: "" }), /networkPassphrase/);
});

test("isMainnet is true only for the mainnet passphrase", () => {
  assert.equal(isMainnet(OK), false);
  assert.equal(isMainnet({ ...OK, networkPassphrase: MAINNET_PASSPHRASE }), true);
});

test("the testnet example config is valid and frozen", () => {
  assert.doesNotThrow(() => assertConfig(TESTNET_ALPHA_EXAMPLE));
  assert.equal(Object.isFrozen(TESTNET_ALPHA_EXAMPLE), true);
  assert.equal(
    TESTNET_ALPHA_EXAMPLE.contractId,
    "CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6",
  );
  assert.equal(TESTNET_ALPHA_EXAMPLE.networkPassphrase, TESTNET_PASSPHRASE);
});
