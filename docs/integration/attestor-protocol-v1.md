# ProofOwl attestor integration protocol v1

Status: **superseded for a v0.2 target — historical record of the v0.1
protocol.** See [`attestor-protocol-v2.md`](./attestor-protocol-v2.md)
for what changed (mainly §8 idempotency/reconciliation and §13's error
table) and [`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md).
Most of this document (trust boundaries, OAuth proof, PR verification,
rotation, unlink, rate limiting, audit logging) is unchanged in
substance and is not duplicated in v2 — read it here.

---

Status (original, v0.1): **normative** for the `proofowl-backend`
service. Versioned as `v1`. This document defines what the backend MUST
do **before** it uses the attestor key to:

- co-sign a wallet ↔ GitHub identity link (`link_github`);
- submit a contribution attestation (`submit_attestation`);
- assist with an unlink (`unlink_github`).

The `proofowl-backend` repository does **not exist yet**. This is the
contract it must satisfy when it is built.

## 0. Trust boundaries (read first)

The smart contract:

- **does not** verify GitHub OAuth, GitHub API responses, PR state, repo
  ownership, or complexity tiers;
- **does** enforce procedure: a link needs both the wallet and the
  stored attestor to sign; an attestation needs the stored attestor to
  sign; the credited wallet is always resolved from the on-chain link,
  never named by the attestor.

Therefore every fact the contract records is only as trustworthy as this
backend. A compromised attestor key can forge *that* a contribution
happened or misreport its tier, but **cannot** bind a GitHub identity to
a wallet the identity's owner did not sign for, and **cannot** redirect
credit to an arbitrary wallet. Nothing in this protocol may weaken those
two properties. See `SECURITY.md` and `docs/adr/0001`, `0002`.

## 1. Attestor key custody

- The attestor key is a Stellar account key. It MUST live in a signer
  the application process cannot exfiltrate (HSM, KMS, cloud signer),
  not in a plaintext keystore on an app server.
- Startup check: call `get_attestor()` on the configured contract id +
  network and assert it equals the address of the key the backend
  holds. Refuse to start on mismatch.
- The key is single-purpose. Do not reuse it for fee payment, deploys,
  or any other contract.

## 2. GitHub ownership proof (OAuth / challenge)

Before a link, the backend MUST establish that **the person who controls
the wallet** also controls a specific GitHub **numeric user id**.

1. **OAuth Authorization Code + PKCE** with GitHub as the identity
   provider. Request the minimal scope needed to read the authenticated
   user's numeric id (`read:user` / no scope for the public id).
2. From the OAuth-authenticated session, read the user's **numeric id**
   (`GET /user` → `id`). Never take the numeric id, login, or email from
   client-supplied input.
3. **Bind the GitHub session to the wallet.** The wallet must sign a
   backend-issued, single-use challenge that names the GitHub numeric
   id and a short expiry, e.g. a Stellar
   `manageData` / auth-entry style message or an off-chain signed blob:
   `proofowl-link-challenge:v1:<github_numeric_id>:<wallet_address>:<nonce>:<expiry_unix>`.
   Verify the signature against `<wallet_address>`.
4. Only when both hold — a live GitHub OAuth session for id `N` **and** a
   valid wallet signature over a challenge naming `N` and that wallet —
   is the pair considered proven.

Session, nonce, and challenge storage MUST be server-side, single-use,
and expiring (minutes, not hours).

## 3. From proven ownership to the identity hash

- Compute `github_id_hash` per
  [`identifier-spec-v1.md`](./identifier-spec-v1.md) §1 from the
  **numeric id `N`** obtained in step 2.3 — never from a login.
- Do not accept a client-supplied `github_id_hash`. Recompute it.
- Persist the mapping `N ↔ github_id_hash ↔ wallet` in the backend's
  own store with the proof artifacts (OAuth timestamp, challenge nonce,
  wallet signature) for audit (§12).

## 4. Two-party link submission

`link_github(wallet, attestor, github_id_hash)` needs **both**
signatures on the same invocation (see
[`contract-api-v1.md`](./contract-api-v1.md#two-party-authorization)).

1. Build the `AssembledTransaction` for `link_github` with `wallet` as
   the invoker/source and `attestor == get_attestor()`.
2. Obtain the **wallet's** auth-entry signature. Two shapes:
   - *frontend-initiated*: the frontend collected it and posts the
     partially-signed transaction to the backend;
   - *backend-initiated*: the backend returns the unsigned transaction /
     auth payload to the client, which signs and returns it.
3. The backend verifies: the invocation is exactly `link_github` on the
   configured contract id; the args are `(wallet, attestor,
   github_id_hash)` matching the proven pair from §2–3; the wallet
   auth entry is present and valid.
4. The backend adds the **attestor** auth-entry signature.
5. Submit (or hand back for submission). Handle the result per §7.

The backend MUST NOT co-sign if any of: the GitHub proof is missing or
expired; the `github_id_hash` does not match the proven numeric id; the
`wallet` in the call differs from the wallet that signed the challenge;
`get_attestor()` no longer equals the backend's key.

## 5. PR verification (before `submit_attestation`)

The backend MUST independently verify, via the GitHub REST/GraphQL API
(authenticated app token, not user input):

| Check | Requirement |
|---|---|
| PR exists | `GET /repos/{owner}/{repo}/pulls/{number}` returns 200 |
| PR is **merged** | `merged == true` and `merged_at` is set (an open or closed-unmerged PR is not eligible) |
| Author identity | the PR author's **numeric id** equals the numeric id behind the `github_id_hash` being credited |
| Repository is allowed | `owner/repo` is on the **allowed-repository policy** list (see §6) at the PR's merge time |
| Contribution window | `merged_at` is within the program window the backend enforces (if any) |
| Not a bot / not self-merge-only | apply program rules (e.g. reject PRs authored by a bot account; reject trivial automated PRs) per policy |

Complexity tier:

- `complexity` MUST be one of `0`, `100`, `150`, `200` (the contract
  rejects anything else with `InvalidComplexity`).
- `100 / 150 / 200` are the Stellar Wave point tiers; the backend
  assigns them from the Wave issue / review outcome it has verified.
- `0` means "confirmed merged, tier not independently determined". Use
  it only when the backend truly cannot resolve a tier; it scores at a
  flat base rate on-chain.

`issue_id` is the Stellar Wave issue id the contribution resolved, or
`0` if not applicable.

## 6. Allowed-repository policy

- Maintain an explicit allow-list of `owner/repo` values (lowercased).
  An attestation for a repo not on the list at merge time MUST be
  refused by the backend (the contract does not know or check this).
- The list is configuration, versioned and audit-logged on change.
- Removing a repo from the list does not retract past attestations
  (on-chain history is immutable); it only stops new ones.

## 7. Canonical PR hash & submission

- `repo` argument: `"<owner>/<repo>"`, lowercased.
- `pr_number` argument: the PR number as `u32`.
- `pr_hash` argument: `hashGitHubPullRequestV1(owner, repo, pr_number)`
  exactly per [`identifier-spec-v1.md`](./identifier-spec-v1.md) §2.
  The backend computes it; it is never client-supplied.
- `github_id_hash` argument: the verified author's identity hash (§3).
- Call `submit_attestation(attestor, github_id_hash, repo, pr_number,
  issue_id, complexity, pr_hash)`, signed by the attestor key only.

## 8. Idempotency and retries

- **Key every attestation job by `pr_hash`.** The backend's own store
  MUST treat `pr_hash` as unique.
- On submit, map results:
  - `Ok(wallet)` → record success with the returned wallet and the
    transaction hash.
  - `DuplicateAttestation` (6) → **treat as success**. The PR is
    already credited on-chain (possibly by a previous attempt or
    another backend instance). Reconcile local state; do not retry.
  - `WalletNotLinked` (7) → the contributor has not linked yet. Park
    the verified job in a "pending link" queue and retry after the
    matching `GithubLinked` event or on a schedule. **Never** invent a
    placeholder wallet or link on the contributor's behalf without the
    §2 proof.
  - `InvalidComplexity` (8) → backend bug; fix the tier, do not retry
    blindly.
  - `Unauthorized` (3) / auth error → check `get_attestor()`; a
    rotation may have happened (§9). Halt attestations until the key
    matches.
  - Network / simulation transient errors → retry with exponential
    backoff and a capped attempt count; the `pr_hash` key makes retries
    safe.
- Two backend instances racing on the same `pr_hash`: at most one
  `Ok`, the other gets `DuplicateAttestation`; both outcomes are
  terminal-success for the job.

## 9. Attestor-key rotation

- Rotation is an **admin** action: `set_attestor(admin, new_attestor)`.
  The backend does not perform it, but MUST cooperate.
- Procedure:
  1. Provision the new signer; a standby backend instance loads it.
  2. Admin calls `set_attestor`. An `AttestorRotated` event is emitted.
  3. Backend instances watch for `AttestorRotated`; on seeing it, each
     instance re-checks `get_attestor()` and switches to the new key,
     or halts if it does not hold the new key.
  4. Retire the old key.
- Between step 2 and every instance switching, calls signed by the old
  key fail with `Unauthorized`. Design the queue to pause and resume,
  not to drop jobs.

## 10. Unlink assistance

- `unlink_github(wallet, attestor, github_id_hash)` is two-party like
  `link_github`. The backend co-signs only when it is satisfied the
  request is legitimate — e.g. the contributor re-authenticates via §2
  and asks to release the identity, or an operator ticket with a clear
  audit trail.
- The backend MUST verify the `(wallet, github_id_hash)` pair is an
  actual current link (`get_wallet_for_github` / `get_github_for_wallet`)
  before co-signing, to avoid a wasted `LinkNotFound`.
- After unlink: the wallet keeps its attestation history and score; the
  `github_id_hash` is free to be linked to a **different** wallet after
  a fresh §2 proof; spent `pr_hash` markers remain spent. There is no
  key-loss recovery (`SECURITY.md` §4.2) — document this to users.

## 11. Rate limiting

- Per-wallet and per-GitHub-id limits on link attempts and challenge
  issuance (e.g. small N per hour) to blunt griefing and enumeration.
- Global cap on `submit_attestation` throughput to bound fee spend and
  ledger write pressure; a queue with backpressure, not unbounded
  concurrency.
- GitHub API calls: respect GitHub's rate limits with a shared token
  bucket; never proxy raw client requests to GitHub.

## 12. Audit logging

For every attestor-key use, record (append-only, tamper-evident where
possible):

- the action (`link` / `unlink` / `attest`) and all on-chain arguments;
- the resolved / expected wallet and `github_id_hash`;
- for links: OAuth session id/time, challenge nonce, wallet-signature
  digest (not the raw key material);
- for attestations: the GitHub API evidence (PR id, `merged_at`, author
  numeric id, repo-policy version);
- the submitted transaction hash and the contract result (`Ok` /
  error code);
- operator identity for any manual action.

Never log secret keys, seed phrases, raw OAuth tokens, or full
authorization-entry secret material.

## 13. Failure modes → error mapping (summary)

| Situation | Contract signal | Backend action |
|---|---|---|
| wrong / rotated attestor key | `Unauthorized` (3) or auth error | halt attest/link; reconcile with `get_attestor()`; §9 |
| contributor not linked yet | `WalletNotLinked` (7) | park job, retry after `GithubLinked`; §8 |
| PR already credited | `DuplicateAttestation` (6) | success-equivalent; reconcile; §8 |
| bad tier value | `InvalidComplexity` (8) | backend bug; fix tier; §5 |
| wallet already has an identity | `WalletAlreadyLinked` (4) | surface to user; needs unlink first |
| identity already linked elsewhere | `GithubAlreadyLinked` (5) | surface to user; possible squat or stale link |
| unlink target not a real link | `LinkNotFound` (9) | re-check with getters before co-signing; §10 |
| instance archived | `NotInitialized` (2) | alert; the contract instance needs restore |
| missing wallet or attestor signature | host auth error | build/collect both signatures; §4 |

## 14. Explicit non-goals of this protocol

- It does not make the contract trust GitHub. It makes the **backend**
  trustworthy enough that the contract's procedural guarantees are
  meaningful.
- It does not remove the single-trusted-attestor limitation
  (`SECURITY.md` §7). Moving to a multisig / threshold attestor is
  future work and is a `set_attestor` target.
- It does not provide anonymity or privacy for linked identities (see
  [`identifier-spec-v1.md`](./identifier-spec-v1.md) §1.5).
