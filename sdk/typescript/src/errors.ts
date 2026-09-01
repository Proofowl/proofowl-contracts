/**
 * Typed view of the contract's `#[contracterror]` enum, plus helpers to
 * recognise a ProofOwl contract error thrown by the generated client.
 *
 * See `docs/integration/contract-api-v1.md#errors`. The codes are part
 * of the ABI and are authoritative in the WASM.
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
});

const CONTRACT_ERR_RE = /Error\(Contract,\s*#(\d+)\)/;

/**
 * Best-effort extraction of a ProofOwl contract error code from anything
 * the generated client / stellar-sdk may throw or return.
 *
 * Recognises:
 *  - a plain number that is a valid code;
 *  - `{ message: "...Error(Contract, #N)..." }` shapes;
 *  - an Error whose message contains `Error(Contract, #N)`;
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
    const m = CONTRACT_ERR_RE.exec(input);
    return m ? Number(m[1]) : undefined;
  }

  if (input && typeof input === "object") {
    const obj = input as Record<string, unknown>;
    // Rust Result failure objects from the generated client.
    for (const key of ["error", "value", "cause"]) {
      const nested = extractCode(obj[key]);
      if (nested !== undefined) return nested;
    }
    if (typeof obj.message === "string") {
      const m = CONTRACT_ERR_RE.exec(obj.message);
      if (m) return Number(m[1]);
    }
    if (input instanceof Error) {
      const m = CONTRACT_ERR_RE.exec(input.message);
      if (m) return Number(m[1]);
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
