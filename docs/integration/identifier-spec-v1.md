# ProofOwl canonical identifier spec v1

Status: **normative** for `v1`. The reference implementation is
[`sdk/typescript/src/identifiers.ts`](../../sdk/typescript/src/identifiers.ts);
its unit tests pin every vector in this document. A change to any rule
here is a new version (`identifier-spec-v2.md`) and a new domain/version
prefix — old hashes never silently change meaning.

The contract treats both hashes as **opaque `BytesN<32>`**. It does not
parse, validate, or reverse them. Everything below is a convention that
the backend, the SDK, and any indexer MUST implement identically so that
the same real-world thing always maps to the same 32 bytes.

Common rules for both hashes:

- Hash algorithm: **SHA-256**, one pass, over the UTF-8 bytes of the
  canonical string.
- On-chain value: the raw 32-byte digest as `BytesN<32>`.
- Hex form (CLI, logs, JSON): **lowercase**, exactly 64 hex characters,
  no `0x` prefix.
- Canonical strings are ASCII in practice. A byte outside `0x20–0x7E`
  in any input segment is a **hard rejection**, never transliterated or
  Unicode-normalized.
- No trimming of "meaningful" characters: leading/trailing ASCII
  whitespace in a raw input is stripped once before validation; any
  *interior* whitespace is a rejection.

---

## 1. GitHub identity hash — `github_id_hash`

### 1.1 Input: the immutable numeric user id

Use the GitHub **numeric user id** (the `id` field of the GitHub user
API, e.g. `1024025`), **never** the login/handle.

Rationale: a GitHub *login* can be renamed, and after a rename it can be
claimed by someone else. The numeric id is assigned once and never
reused. Hashing the login would let a reputation record silently change
owner. The backend obtains the numeric id from the authenticated OAuth
session, not from user input.

### 1.2 Canonical string

```
proofowl:github-user:v1:<decimal-id>
```

- Fixed prefix `proofowl:github-user:v1:` (ASCII, exactly as shown,
  including the trailing colon).
- `<decimal-id>`: the numeric id in base 10, digits `0–9` only, **no
  leading zeros** (except the value itself is never `0`), no sign, no
  separators, no whitespace.

Validation (reject on any failure):

- id is an integer with `1 <= id <= 9007199254740991` (`2^53 − 1`, the
  JS safe-integer ceiling; real GitHub ids are far below this);
- the decimal rendering matches `^[1-9][0-9]*$`.

### 1.3 Hash

```
github_id_hash = SHA-256( utf8( "proofowl:github-user:v1:" + decimal_id ) )
```

### 1.4 Vectors

| numeric id | canonical string | `github_id_hash` (hex) |
|---|---|---|
| `1` | `proofowl:github-user:v1:1` | `ad6494a9db671dce66088a82f8446c464e7d425da57d4eca4081b19a74b1e584` |
| `1024025` | `proofowl:github-user:v1:1024025` | `fd608646c4bd0a96553707213c1680c9dfcb0c9ba47f649ccb1c7924125176cb` |

The SDK test (`identifiers.test.ts`) pins these same 64-char outputs;
that file is the authoritative vector set.

### 1.5 This is an identifier, not privacy

`github_id_hash` is **opaque, not secret**. GitHub user ids are small
sequential integers; the entire preimage space is trivially
enumerable, and anyone can compute the hash for any id. The hash hides
**nothing** — it exists only to give the contract a fixed-size,
format-stable key that does not change when a login is renamed. Do not
describe a linked identity as "private" or "anonymous" anywhere in a
product surface.

### 1.6 Who may create a link

Only the trusted **attestor** (operated by `proofowl-backend`) may
co-sign `link_github`, and only **after** an OAuth / challenge flow has
proven that the wallet holder controls the GitHub account with that
exact numeric id. The contract cannot check this. See
[`attestor-protocol-v1.md`](./attestor-protocol-v1.md) §2–3.

---

## 2. Pull-request hash — `pr_hash`

### 2.1 Canonical string

```
github.com/<owner>/<repo>/pull/<number>
```

- no scheme (`https://`), no `www.`, no `github.com` port;
- no trailing slash, no query string, no fragment, no sub-path
  (`/files`, `/commits`, …);
- exactly the four segments shown, separated by single `/`.

### 2.2 Normalization of the parts

**`owner`**

- strip one leading `@` if present (some UIs render `@owner`);
- lowercase (ASCII `A–Z` → `a–z`);
- must match `^[a-z0-9](?:[a-z0-9-]{0,37}[a-z0-9])?$` — GitHub logins are
  1–39 chars of `[A-Za-z0-9-]`, no leading/trailing hyphen, no double
  hyphen is *allowed* by GitHub so we do not forbid it.

**`repo`**

- strip one trailing `.git` if present;
- lowercase;
- must match `^[a-z0-9._-]{1,100}$`;
- reject if it is exactly `.` or `..` (path-traversal guard) or contains
  a `/`.

**`number`** (the pull-request number, **not** the Wave `issue_id`)

- strip one leading `#` if present;
- digits `0–9` only, `^[1-9][0-9]*$` (no leading zeros), value
  `1 <= number <= 4294967295` (`u32` ceiling — the contract stores
  `pr_number` as `u32`).

Any input with interior whitespace, an empty part, a wrong host, a
scheme, a query/fragment, or a non-ASCII byte is **rejected with an
error** — never coerced.

### 2.3 Assembly and hash

```
canonical = "github.com/" + owner + "/" + repo + "/pull/" + number
pr_hash   = SHA-256( utf8(canonical) )
```

### 2.4 Vectors

| owner | repo | number | canonical string | `pr_hash` (hex) |
|---|---|---|---|---|
| `stellar` | `soroban-examples` | `42` | `github.com/stellar/soroban-examples/pull/42` | `1eed82536f9e3a9477916599ab2111d9af634b1270f5d4d1d61ee98bd50d6c0e` |
| `ProofOwl` | `Proofowl-Contracts.git` | `#7` | `github.com/proofowl/proofowl-contracts/pull/7` | `be9b713cbcbacdc44d593cd3e37f8680f6e7e229af9c2182cde3ee05a2bf6cef` |

The second row shows normalization: `@`/case/`.git`/`#` are all
absorbed, so both a browser copy-paste and an API value converge.

Rejected examples (must throw):

- `https://github.com/o/r/pull/1` (scheme present)
- `github.com/o/r/pull/1/files` (sub-path)
- `github.com/o/r/pull/1?diff=split` (query)
- `github.com/o//pull/1` (empty repo)
- `github.com/o/r/issues/1` (issue, not pull)
- `github.com/o/r/pull/0` and `.../pull/01` and `.../pull/-1`
- `github.com/o/r/pull/99999999999` (exceeds `u32`)

### 2.5 Why the hash is globally idempotent

After normalization the canonical string is a pure function of
`(owner, repo, number)`. Every way of referring to the same pull
request — `http` vs `https`, trailing slash, `.git`, casing, the
`/files` tab, an API URL — collapses to the same string and therefore
the same 32 bytes. The contract's `SeenPr(pr_hash)` entry is a
**permanent, global** "this PR has been credited" marker: a second
`submit_attestation` with the same `pr_hash` fails with
`DuplicateAttestation` (6) regardless of which wallet or GitHub identity
it is submitted for. That is the anti-double-credit guarantee, and it
survives `unlink_github` and TTL bumps.

### 2.6 Relationship to `repo` and `pr_number`

Each `Attestation` stores `repo` (`"<owner>/<repo>"`) and `pr_number`
(`u32`) **in the clear** alongside `pr_hash`. This is deliberate:

- an indexer can rebuild `github.com/<repo>/pull/<pr_number>` and link
  straight to the PR;
- an indexer MUST **recompute** `hashGitHubPullRequestV1(owner, repo,
  pr_number)` and compare it to the stored `pr_hash`. A mismatch means
  the attestor submitted inconsistent data (a bug or misbehaviour on
  the backend side) — surface it, do not trust the record.

`pr_hash` alone is not reversible to a URL, but it **is verifiable**
once you have the three parts.
