/**
 * Typed view of the contract's `#[contracterror]` enum, plus helpers to
 * recognise a ProofOwl contract error thrown by the generated client.
 *
 * See `docs/integration/contract-api-v2.md#errors`. The codes are part
 * of the ABI and are authoritative in the WASM. Codes 1-9 are v0.1,
 * unchanged; 10-13 are v0.2 additions, appended not renumbered
 * (`docs/adr/0004-paginated-attestation-storage.md`).
 */

import { Errors as GeneratedErrors } from "./generated/index.js";

/** Re-export the generated `{ [code]: { message } }` map verbatim. */
export { GeneratedErrors };

export enum ProofOwlErrorCode {
  /** Reserved; unreachable (constructor runs once, no `init`). */
  AlreadyInitialized = 1,
  /** Instance config missing (e.g. archived). Practically unreachable. */
  NotInitialized = 2,
  /** Caller-supplied admin/attestor is not the stored one. */
  Unauthorized = 3,
  /** That wallet already has a GitHub link. */
  WalletAlreadyLinked = 4,
  /** That github_id_hash is already linked to some wallet. */
  GithubAlreadyLinked = 5,
  /** That pr_hash was already recorded (globally, forever). */
  DuplicateAttestation = 6,
  /** No wallet is linked for that github_id_hash. */
  WalletNotLinked = 7,
  /** complexity not in {0, 100, 150, 200}. */
  InvalidComplexity = 8,
  /** unlink_github target is not a consistent existing link. */
  LinkNotFound = 9,
  /** A paginated call's `limit` was `0`. v0.2. */
  InvalidPageLimit = 10,
  /** A paginated call's `limit` exceeded `MAX_PAGE_SIZE`. v0.2. */
  PageLimitExceeded = 11,
  /** `get_attestation`'s `sequence` was `>=` the wallet's attestation count. v0.2. */
  SequenceOutOfRange = 12,
  /** A paginated call's `start` was `>` the wallet's attestation count. v0.2. */
  PageStartOutOfRange = 13,
}

export const PROOFOWL_ERROR_NAME: Readonly<Record<ProofOwlErrorCode, string>> = Object.freeze({
  [ProofOwlErrorCode.AlreadyInitialized]: "AlreadyInitialized",
  [ProofOwlErrorCode.NotInitialized]: "NotInitialized",
  [ProofOwlErrorCode.Unauthorized]: "Unauthorized",
  [ProofOwlErrorCode.WalletAlreadyLinked]: "WalletAlreadyLinked",
  [ProofOwlErrorCode.GithubAlreadyLinked]: "GithubAlreadyLinked",
  [ProofOwlErrorCode.DuplicateAttestation]: "DuplicateAttestation",
  [ProofOwlErrorCode.WalletNotLinked]: "WalletNotLinked",
  [ProofOwlErrorCode.InvalidComplexity]: "InvalidComplexity",
  [ProofOwlErrorCode.LinkNotFound]: "LinkNotFound",
  // v0.2 additions -- appended, not renumbered (docs/adr/0004-paginated-attestation-storage.md).
  [ProofOwlErrorCode.InvalidPageLimit]: "InvalidPageLimit",
  [ProofOwlErrorCode.PageLimitExceeded]: "PageLimitExceeded",
  [ProofOwlErrorCode.SequenceOutOfRange]: "SequenceOutOfRange",
  [ProofOwlErrorCode.PageStartOutOfRange]: "PageStartOutOfRange",
});

/**
 * Reverse lookup, name -> code. Needed because of how
 * `@stellar/stellar-sdk/contract`'s `Result<T, Error>` wrapper surfaces
 * a contract-level `Err` for a READ (simulated, not submitted) call:
 * `AssembledTransaction.result` for such a call resolves to a
 * `rust_result.Ok` / `rust_result.Err` object directly (the Result is
 * decoded from the successful simulation's return value, not thrown as
 * a host error), and `Err.unwrap()` throws a plain `Error` whose
 * `.message` is exactly the bare variant name (e.g.
 * `"SequenceOutOfRange"`) from the ABI's `Errors` map -- NOT the
 * `Error(Contract, #N)` string a submitted, rejected mutating
 * transaction's host error carries. `get_attestation` and
 * `get_attestations_page` (v0.2) are the first read-only calls in this
 * SDK that return a `Result`, so this case did not previously arise.
 */
const NAME_TO_CODE: ReadonlyMap<string, ProofOwlErrorCode> = new Map(
  Object.entries(PROOFOWL_ERROR_NAME).map(([code, name]) => [
    name,
    Number(code) as ProofOwlErrorCode,
  ]),
);

const CONTRACT_ERR_RE = /Error\(Contract,\s*#(\d+)\)/;

/** Try the `Error(Contract, #N)` pattern first, then a bare error name. */
function extractCodeFromString(s: string): number | undefined {
  const match = CONTRACT_ERR_RE.exec(s);
  if (match) return Number(match[1]);
  const code = NAME_TO_CODE.get(s.trim());
  return code;
}

/**
 * Best-effort extraction of a ProofOwl contract error code from anything
 * the generated client / stellar-sdk may throw or return.
 *
 * Recognises:
 *  - a plain number that is a valid code;
 *  - `{ message: "...Error(Contract, #N)..." }` shapes (a submitted,
 *    rejected mutating call's host error);
 *  - `{ message: "SequenceOutOfRange" }` (or any other bare error
 *    name) shapes -- what a Result-returning READ call's
 *    `.result.unwrap()` throws (v0.2: `get_attestation`,
 *    `get_attestations_page`), since that Result is decoded straight
 *    from a successful simulation, not surfaced as a host error string;
 *  - an Error whose message contains either of the above;
 *  - a Rust-`Result` failure object exposing `.error` / `.value`.
 *
 * Returns `undefined` if it is not a recognised ProofOwl contract error
 * (e.g. a host auth error, an RPC/network error).
 */
export function parseProofOwlError(input: unknown): ProofOwlErrorCode | undefined {
  const code = extractCode(input);
  if (code === undefined) return undefined;
  return code in PROOFOWL_ERROR_NAME ? (code as ProofOwlErrorCode) : undefined;
}

function extractCode(input: unknown): number | undefined {
  if (typeof input === "number" && Number.isInteger(input)) return input;

  if (typeof input === "string") {
    return extractCodeFromString(input);
  }

  if (input && typeof input === "object") {
    const obj = input as Record<string, unknown>;
    // Rust Result failure objects from the generated client.
    for (const key of ["error", "value", "cause"]) {
      const nested = extractCode(obj[key]);
      if (nested !== undefined) return nested;
    }
    if (typeof obj.message === "string") {
      const code = extractCodeFromString(obj.message);
      if (code !== undefined) return code;
    }
    if (input instanceof Error) {
      const code = extractCodeFromString(input.message);
      if (code !== undefined) return code;
    }
  }
  return undefined;
}

/** True if `err` is `DuplicateAttestation` — treat as success-equivalent for idempotency. */
export function isDuplicateAttestation(err: unknown): boolean {
  return parseProofOwlError(err) === ProofOwlErrorCode.DuplicateAttestation;
}

/** True if `err` is `WalletNotLinked` — the contributor has not linked yet. */
export function isWalletNotLinked(err: unknown): boolean {
  return parseProofOwlError(err) === ProofOwlErrorCode.WalletNotLinked;
}
