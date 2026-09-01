/**
 * Canonical identifier helpers — the reference implementation of
 * `docs/integration/identifier-spec-v1.md`.
 *
 * Every function is pure and has no dependency beyond `node:crypto`.
 * The unit tests in `identifiers.test.ts` pin the exact output vectors;
 * that file and this one must stay in lock-step with the spec document.
 *
 * All hashes are SHA-256 of the UTF-8 bytes of a canonical ASCII string.
 * Output is the raw 32-byte digest; `*Hex` variants give the lowercase
 * 64-char hex form.
 */

import { createHash } from "node:crypto";

// --- low-level -------------------------------------------------------------

function sha256(input: string): Uint8Array {
  return new Uint8Array(createHash("sha256").update(Buffer.from(input, "utf8")).digest());
}

/** Lowercase 64-char hex for a 32-byte value. */
export function bytesToHex(bytes: Uint8Array): string {
  if (!(bytes instanceof Uint8Array)) {
    throw new TypeError("bytesToHex expects a Uint8Array");
  }
  return Buffer.from(bytes).toString("hex");
}

/** Parse a 64-char lowercase-or-uppercase hex string into 32 bytes. */
export function hexToBytes32(hex: string): Uint8Array {
  if (typeof hex !== "string" || !/^[0-9a-fA-F]{64}$/.test(hex)) {
    throw new TypeError("hexToBytes32 expects a 64-character hex string (32 bytes)");
  }
  return new Uint8Array(Buffer.from(hex, "hex"));
}

function assertAsciiPrintable(value: string, label: string): void {
  for (let i = 0; i < value.length; i++) {
    const c = value.charCodeAt(i);
    if (c < 0x20 || c > 0x7e) {
      throw new RangeError(`${label} contains a non-ASCII / control character at index ${i}`);
    }
  }
}

// --- GitHub identity hash (v1) ------------------------------------------

export const GITHUB_USER_ID_PREFIX_V1 = "proofowl:github-user:v1:";

const MAX_SAFE = BigInt(Number.MAX_SAFE_INTEGER); // 2^53 - 1

/**
 * Validate and normalise a GitHub numeric user id to a bigint in
 * `[1, 2^53 - 1]`. Accepts an integer `number`, a `bigint`, or a
 * decimal string with no sign, no leading zeros, and no whitespace.
 */
export function coerceGitHubUserId(githubUserId: number | bigint | string): bigint {
  let n: bigint;
  if (typeof githubUserId === "bigint") {
    n = githubUserId;
  } else if (typeof githubUserId === "number") {
    if (!Number.isInteger(githubUserId)) {
      throw new TypeError("githubUserId number must be an integer");
    }
    n = BigInt(githubUserId);
  } else if (typeof githubUserId === "string") {
    if (!/^[1-9][0-9]*$/.test(githubUserId)) {
      throw new TypeError(
        "githubUserId string must be a positive base-10 integer with no leading zeros",
      );
    }
    n = BigInt(githubUserId);
  } else {
    throw new TypeError("githubUserId must be a number, bigint, or string");
  }
  if (n < 1n) throw new RangeError("githubUserId must be >= 1");
  if (n > MAX_SAFE) {
    throw new RangeError("githubUserId must be <= 2^53 - 1");
  }
  return n;
}

/** The canonical string that gets hashed for a GitHub identity. */
export function canonicalGitHubUserIdStringV1(githubUserId: number | bigint | string): string {
  return GITHUB_USER_ID_PREFIX_V1 + coerceGitHubUserId(githubUserId).toString();
}

/** `github_id_hash` (32 bytes) for a GitHub numeric user id. */
export function hashGitHubUserIdV1(githubUserId: number | bigint | string): Uint8Array {
  return sha256(canonicalGitHubUserIdStringV1(githubUserId));
}

/** `github_id_hash` as lowercase 64-char hex. */
export function hashGitHubUserIdV1Hex(githubUserId: number | bigint | string): string {
  return bytesToHex(hashGitHubUserIdV1(githubUserId));
}

// --- Pull-request hash (v1) -------------------------------------------

const OWNER_RE = /^[a-z0-9](?:[a-z0-9-]{0,37}[a-z0-9])?$/;
const REPO_RE = /^[a-z0-9._-]{1,100}$/;
const U32_MAX = 0xffff_ffff;

export interface NormalizedPullRequest {
  owner: string;
  repo: string;
  /** PR number as a safe integer in `[1, 2^32 - 1]`. */
  number: number;
  /** `github.com/<owner>/<repo>/pull/<number>` — the string that is hashed. */
  canonical: string;
}

function normalizeOwner(owner: string): string {
  if (typeof owner !== "string") throw new TypeError("owner must be a string");
  let o = owner.trim();
  if (o.startsWith("@")) o = o.slice(1);
  assertAsciiPrintable(o, "owner");
  if (/\s/.test(o)) throw new RangeError("owner must not contain whitespace");
  o = o.toLowerCase();
  if (!OWNER_RE.test(o)) {
    throw new RangeError(
      `owner ${JSON.stringify(owner)} is not a valid GitHub login (1-39 chars of [A-Za-z0-9-], no leading/trailing hyphen)`,
    );
  }
  return o;
}

function normalizeRepo(repo: string): string {
  if (typeof repo !== "string") throw new TypeError("repo must be a string");
  let r = repo.trim();
  if (r.toLowerCase().endsWith(".git")) r = r.slice(0, -4);
  assertAsciiPrintable(r, "repo");
  if (/\s/.test(r)) throw new RangeError("repo must not contain whitespace");
  if (r.includes("/")) throw new RangeError("repo must not contain '/'");
  r = r.toLowerCase();
  if (r === "." || r === "..") throw new RangeError("repo must not be '.' or '..'");
  if (!REPO_RE.test(r)) {
    throw new RangeError(
      `repo ${JSON.stringify(repo)} is not a valid GitHub repository name (1-100 chars of [A-Za-z0-9._-])`,
    );
  }
  return r;
}

function normalizePullNumber(pullNumber: number | bigint | string): number {
  let s: string;
  if (typeof pullNumber === "number") {
    if (!Number.isInteger(pullNumber)) throw new TypeError("pullNumber must be an integer");
    s = pullNumber.toString();
  } else if (typeof pullNumber === "bigint") {
    s = pullNumber.toString();
  } else if (typeof pullNumber === "string") {
    let t = pullNumber.trim();
    if (t.startsWith("#")) t = t.slice(1);
    s = t;
  } else {
    throw new TypeError("pullNumber must be a number, bigint, or string");
  }
  if (!/^[1-9][0-9]*$/.test(s)) {
    throw new RangeError("pullNumber must be a positive integer with no leading zeros or sign");
  }
  const n = Number(s);
  if (!Number.isInteger(n) || n < 1 || n > U32_MAX) {
    throw new RangeError("pullNumber must be in [1, 2^32 - 1]");
  }
  return n;
}

/**
 * Normalise `(owner, repo, pullNumber)` to the canonical PR identity.
 * Absorbs a leading `@` on the owner, a trailing `.git` on the repo, a
 * leading `#` on the number, and casing. Rejects anything malformed
 * rather than coercing it.
 */
export function normalizeGitHubPullRequest(
  owner: string,
  repo: string,
  pullNumber: number | bigint | string,
): NormalizedPullRequest {
  const o = normalizeOwner(owner);
  const r = normalizeRepo(repo);
  const n = normalizePullNumber(pullNumber);
  return { owner: o, repo: r, number: n, canonical: `github.com/${o}/${r}/pull/${n}` };
}

/** `pr_hash` (32 bytes) for a pull request. */
export function hashGitHubPullRequestV1(
  owner: string,
  repo: string,
  pullNumber: number | bigint | string,
): Uint8Array {
  return sha256(normalizeGitHubPullRequest(owner, repo, pullNumber).canonical);
}

/** `pr_hash` as lowercase 64-char hex. */
export function hashGitHubPullRequestV1Hex(
  owner: string,
  repo: string,
  pullNumber: number | bigint | string,
): string {
  return bytesToHex(hashGitHubPullRequestV1(owner, repo, pullNumber));
}

/**
 * Recompute a `pr_hash` from an on-chain `Attestation`'s cleartext
 * `repo` (`"<owner>/<repo>"`) and `pr_number`, and compare it to the
 * stored `prHashHex`. Returns `true` on match. An indexer should call
 * this for every attestation (see `identifier-spec-v1.md` §2.6).
 */
export function verifyAttestationPrHash(
  repo: string,
  prNumber: number,
  prHashHex: string,
): boolean {
  const slash = repo.indexOf("/");
  if (slash <= 0 || slash !== repo.lastIndexOf("/")) {
    throw new RangeError('repo must be "<owner>/<repo>"');
  }
  const computed = hashGitHubPullRequestV1Hex(
    repo.slice(0, slash),
    repo.slice(slash + 1),
    prNumber,
  );
  return computed === prHashHex.toLowerCase();
}
