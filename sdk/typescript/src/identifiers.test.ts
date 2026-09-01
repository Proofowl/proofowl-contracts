import { test } from "node:test";
import assert from "node:assert/strict";

import {
  bytesToHex,
  canonicalGitHubUserIdStringV1,
  coerceGitHubUserId,
  GITHUB_USER_ID_PREFIX_V1,
  hashGitHubPullRequestV1,
  hashGitHubPullRequestV1Hex,
  hashGitHubUserIdV1,
  hashGitHubUserIdV1Hex,
  hexToBytes32,
  normalizeGitHubPullRequest,
  verifyAttestationPrHash,
} from "./identifiers.js";

// ---------------------------------------------------------------------------
// Pinned vectors — these are the authoritative outputs of
// docs/integration/identifier-spec-v1.md. Do not change them without
// bumping the spec version.
// ---------------------------------------------------------------------------

const GH_USER_VECTORS: ReadonlyArray<readonly [string | number | bigint, string, string]> = [
  [
    1,
    "proofowl:github-user:v1:1",
    "ad6494a9db671dce66088a82f8446c464e7d425da57d4eca4081b19a74b1e584",
  ],
  [
    1024025,
    "proofowl:github-user:v1:1024025",
    "fd608646c4bd0a96553707213c1680c9dfcb0c9ba47f649ccb1c7924125176cb",
  ],
  [
    9007199254740991n,
    "proofowl:github-user:v1:9007199254740991",
    "1e7fa4a5295f32689530d00860728b707d60f73de136143ee122575b46604e9e",
  ],
];

const PR_VECTORS: ReadonlyArray<readonly [string, string, number | string, string, string]> = [
  [
    "stellar",
    "soroban-examples",
    42,
    "github.com/stellar/soroban-examples/pull/42",
    "1eed82536f9e3a9477916599ab2111d9af634b1270f5d4d1d61ee98bd50d6c0e",
  ],
  // normalization: @owner, mixed case, trailing .git, #number
  [
    "@ProofOwl",
    "Proofowl-Contracts.git",
    "#7",
    "github.com/proofowl/proofowl-contracts/pull/7",
    "be9b713cbcbacdc44d593cd3e37f8680f6e7e229af9c2182cde3ee05a2bf6cef",
  ],
  [
    "a",
    "b",
    1,
    "github.com/a/b/pull/1",
    "74b8b07fec5539a632c2df4ecd2aafaadfe0df40f9941fba6c11bfa7039c4c93",
  ],
];

test("hashGitHubUserIdV1 matches the pinned vectors", () => {
  for (const [input, canonical, hex] of GH_USER_VECTORS) {
    assert.equal(canonicalGitHubUserIdStringV1(input), canonical);
    assert.equal(hashGitHubUserIdV1Hex(input), hex);
    assert.equal(bytesToHex(hashGitHubUserIdV1(input)), hex);
    assert.equal(hashGitHubUserIdV1(input).length, 32);
  }
});

test("GitHub user id string forms agree with numeric forms", () => {
  assert.equal(hashGitHubUserIdV1Hex("1024025"), hashGitHubUserIdV1Hex(1024025));
  assert.equal(hashGitHubUserIdV1Hex(1n), hashGitHubUserIdV1Hex(1));
});

test("coerceGitHubUserId rejects malformed ids", () => {
  for (const bad of [0, -1, 1.5, "0", "01", "+1", " 1", "1 ", "", "1e5", "0x1", "abc", null]) {
    assert.throws(
      () => coerceGitHubUserId(bad as never),
      `${JSON.stringify(bad)} should be rejected`,
    );
  }
  assert.throws(() => coerceGitHubUserId(9007199254740992n), /2\^53/);
});

test("the GitHub user prefix is exactly as specified", () => {
  assert.equal(GITHUB_USER_ID_PREFIX_V1, "proofowl:github-user:v1:");
});

test("hashGitHubPullRequestV1 matches the pinned vectors, with normalization", () => {
  for (const [owner, repo, num, canonical, hex] of PR_VECTORS) {
    const norm = normalizeGitHubPullRequest(owner, repo, num);
    assert.equal(norm.canonical, canonical);
    assert.equal(hashGitHubPullRequestV1Hex(owner, repo, num), hex);
    assert.equal(bytesToHex(hashGitHubPullRequestV1(owner, repo, num)), hex);
    assert.equal(hashGitHubPullRequestV1(owner, repo, num).length, 32);
  }
});

test("normalizeGitHubPullRequest absorbs cosmetic variation", () => {
  const base = normalizeGitHubPullRequest("Stellar", "Soroban-Examples", 42).canonical;
  assert.equal(normalizeGitHubPullRequest("stellar", "soroban-examples", "42").canonical, base);
  assert.equal(
    normalizeGitHubPullRequest("@stellar", "soroban-examples.git", "#42").canonical,
    base,
  );
  assert.equal(normalizeGitHubPullRequest("  stellar  ", " soroban-examples ", 42).canonical, base);
});

test("normalizeGitHubPullRequest rejects malformed parts", () => {
  const bad: ReadonlyArray<readonly [string, string, unknown]> = [
    ["", "r", 1], // empty owner
    ["o", "", 1], // empty repo
    ["o/x", "r", 1], // slash in owner
    ["o", "r/x", 1], // slash in repo
    ["o", ".", 1], // path traversal
    ["o", "..", 1], // path traversal
    ["-o", "r", 1], // leading hyphen owner
    ["o-", "r", 1], // trailing hyphen owner
    ["thisownernameiswaytoolongtobeavalidgithublogin", "r", 1],
    ["oñ", "r", 1], // non-ascii
    ["o", "r", 0], // number < 1
    ["o", "r", "01"], // leading zero
    ["o", "r", -1], // negative
    ["o", "r", "1.0"], // not an integer
    ["o", "r", 5_000_000_000], // exceeds u32
    ["o", "r", "https://x"], // clearly not a number
    ["https://github.com/o/r/pull/1", "r", 1], // URL as owner
  ];
  for (const [owner, repo, num] of bad) {
    assert.throws(
      () => normalizeGitHubPullRequest(owner, repo, num as never),
      `${JSON.stringify([owner, repo, num])} should be rejected`,
    );
  }
});

test("hexToBytes32 / bytesToHex round-trip", () => {
  const hex = hashGitHubUserIdV1Hex(1); // a known 64-char sha256 hex
  assert.equal(hex.length, 64);
  const bytes = hexToBytes32(hex);
  assert.equal(bytes.length, 32);
  assert.equal(bytesToHex(bytes), hex);
  assert.equal(bytesToHex(hexToBytes32(hex.toUpperCase())), hex, "uppercase input normalises");
  assert.throws(() => hexToBytes32("abc"), /64-character hex/);
  assert.throws(() => hexToBytes32("g".repeat(64)), /64-character hex/);
});

test("verifyAttestationPrHash confirms a consistent record and flags a bad one", () => {
  const hex = hashGitHubPullRequestV1Hex("stellar", "soroban-examples", 42);
  assert.equal(verifyAttestationPrHash("stellar/soroban-examples", 42, hex), true);
  assert.equal(verifyAttestationPrHash("stellar/soroban-examples", 43, hex), false);
  assert.equal(verifyAttestationPrHash("other/repo", 42, hex), false);
  assert.throws(() => verifyAttestationPrHash("no-slash", 1, hex), /<owner>\/<repo>/);
});
