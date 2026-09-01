import { test } from "node:test";
import assert from "node:assert/strict";

import {
  GeneratedErrors,
  isDuplicateAttestation,
  isWalletNotLinked,
  parseProofOwlError,
  PROOFOWL_ERROR_NAME,
  ProofOwlErrorCode,
} from "./errors.js";

test("every enum code has a name and matches the generated map", () => {
  const generated = GeneratedErrors as Record<number, { message: string } | undefined>;
  for (let code = 1; code <= 9; code++) {
    const name = PROOFOWL_ERROR_NAME[code as ProofOwlErrorCode];
    assert.equal(typeof name, "string");
    assert.equal(
      name,
      generated[code]?.message,
      `code ${code} name must match the generated bindings`,
    );
  }
});

test("parseProofOwlError extracts a code from an Error(Contract, #N) message", () => {
  assert.equal(
    parseProofOwlError("HostError: Error(Contract, #8)"),
    ProofOwlErrorCode.InvalidComplexity,
  );
  assert.equal(
    parseProofOwlError(new Error("... Error(Contract, #6) ...")),
    ProofOwlErrorCode.DuplicateAttestation,
  );
  assert.equal(
    parseProofOwlError({ message: "Error(Contract, #7)" }),
    ProofOwlErrorCode.WalletNotLinked,
  );
  assert.equal(
    parseProofOwlError({ error: { message: "Error(Contract, #9)" } }),
    ProofOwlErrorCode.LinkNotFound,
  );
  assert.equal(parseProofOwlError(3), ProofOwlErrorCode.Unauthorized);
});

test("parseProofOwlError returns undefined for non-ProofOwl errors", () => {
  assert.equal(parseProofOwlError("Error(Auth, InvalidAction)"), undefined);
  assert.equal(parseProofOwlError(new Error("network timeout")), undefined);
  assert.equal(parseProofOwlError(42), undefined, "42 is not a valid code");
  assert.equal(parseProofOwlError(null), undefined);
  assert.equal(parseProofOwlError(undefined), undefined);
});

test("idempotency helpers", () => {
  assert.equal(isDuplicateAttestation("Error(Contract, #6)"), true);
  assert.equal(isDuplicateAttestation("Error(Contract, #7)"), false);
  assert.equal(isWalletNotLinked("Error(Contract, #7)"), true);
  assert.equal(isWalletNotLinked("Error(Contract, #6)"), false);
});
