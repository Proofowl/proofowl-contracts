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
  // 1-9: v0.1, unchanged; 10-13: v0.2 additions, appended not renumbered
  // (docs/adr/0004-paginated-attestation-storage.md).
  for (let code = 1; code <= 13; code++) {
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

test("parseProofOwlError recognises v0.2 codes via the Error(Contract, #N) form", () => {
  assert.equal(
    parseProofOwlError("HostError: Error(Contract, #10)"),
    ProofOwlErrorCode.InvalidPageLimit,
  );
  assert.equal(
    parseProofOwlError("HostError: Error(Contract, #13)"),
    ProofOwlErrorCode.PageStartOutOfRange,
  );
});

test("parseProofOwlError recognises the bare name a Result-returning read call throws", () => {
  // `AssembledTransaction.result.unwrap()` for get_attestation /
  // get_attestations_page throws `new Error(<bare variant name>)`,
  // NOT the `Error(Contract, #N)` string a submitted mutating call's
  // rejection carries -- see the doc comment on NAME_TO_CODE in
  // errors.ts for why these differ.
  assert.equal(
    parseProofOwlError(new Error("SequenceOutOfRange")),
    ProofOwlErrorCode.SequenceOutOfRange,
  );
  assert.equal(parseProofOwlError("PageStartOutOfRange"), ProofOwlErrorCode.PageStartOutOfRange);
  // The shape `Result.unwrapErr()` returns directly (an ErrorMessage
  // object), before any Error is even constructed.
  assert.equal(
    parseProofOwlError({ message: "PageLimitExceeded" }),
    ProofOwlErrorCode.PageLimitExceeded,
  );
  // Not a real error name -> undefined, not a false positive.
  assert.equal(parseProofOwlError("NotARealErrorName"), undefined);
  assert.equal(parseProofOwlError(new Error("NotARealErrorName")), undefined);
});

test("idempotency helpers", () => {
  assert.equal(isDuplicateAttestation("Error(Contract, #6)"), true);
  assert.equal(isDuplicateAttestation("Error(Contract, #7)"), false);
  assert.equal(isWalletNotLinked("Error(Contract, #7)"), true);
  assert.equal(isWalletNotLinked("Error(Contract, #6)"), false);
});
