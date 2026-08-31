# ADR 0002: Wallet ↔ GitHub links require wallet **and** attestor authorization

## Status
Accepted. Supersedes the "wallet-signature-only" linking flow described
in the original `link_github` and in ADR 0001's consequences.

## Context
The first design made `link_github` a single-signature call: the wallet
signed, and the link was created. The stated rationale was that this made
the link "self-sovereign rather than backend-asserted."

The problem: **a wallet signature proves control of a Stellar key, not
control of a GitHub account.** Under the single-signature flow, anyone
could call `link_github(their_wallet, sha256("torvalds"))` and, from that
point on, every attestation the backend later submitted for that GitHub
identity would resolve — via the exact mechanism ADR 0001 introduced — to
the squatter's wallet. The contributor-resolution guarantee was intact;
it was just pointing at the attacker. Whoever linked a `github_id_hash`
first owned it, with no check that they were the real owner.

The contract cannot fix this itself: it has no way to verify GitHub.

## Decision
`link_github` and the new `unlink_github` are **two-party** calls. Both
require:

- `wallet.require_auth()` — proves control of the Stellar key, and
- `attestor.require_auth()` + a check that the caller-supplied attestor
  equals the stored attestor — the trusted verifier's co-signature.

The attestor (the future `proofowl-backend`) co-signs a link **only
after** running an off-chain GitHub OAuth / challenge flow that proves
the wallet holder controls the GitHub identity behind `github_id_hash`.
The co-signature is the on-chain receipt of that off-chain proof.

Links remain one-to-one in both directions, still collision-checked
(`WalletAlreadyLinked`, `GithubAlreadyLinked`).

`unlink_github` (same two signatures) clears both directions so a
mistaken link can be fixed and an identity can be re-linked to a new
wallet. It intentionally leaves attestation history and global PR-dedup
markers untouched.

## Consequences
- **Squatting is blocked.** A wallet cannot self-assign someone else's
  GitHub identity; the attestor will not co-sign a link the OAuth flow
  did not back.
- **The link is no longer purely self-sovereign.** It now depends on the
  trusted attestor, which is the same trust anchor `submit_attestation`
  already relies on — not a new one. This trade is documented in
  `SECURITY.md §1`.
- **The core ADR 0001 rule still holds.** The attestor still cannot pick
  an arbitrary wallet for `submit_attestation`; it can only co-sign a
  link the wallet *also* signed. A compromised attestor that also
  controls a wallet could link *that* wallet to an identity, but it
  cannot bind an identity to a wallet whose key it does not hold.
- **UX cost.** Linking now needs the contributor and the backend to
  co-sign one transaction (the frontend builds the wallet's auth entry,
  the backend adds the attestor's after OAuth succeeds, then submits).
- **Recovery from a lost wallet key is out of scope for the MVP.** See
  `SECURITY.md §4.2` — an attestor-only override was rejected because it
  would reintroduce exactly the redirect capability this ADR removes.

## Alternatives considered
- **Propose/confirm in two transactions** (wallet proposes, attestor
  confirms later). Rejected for the MVP: it needs a pending-request
  record with its own TTL and its own anti-squatting rules for the
  pending slot. The atomic two-signature call has none of that state and
  no partial-link window.
- **On-chain signature of a GitHub-issued nonce.** No standard, and still
  needs an oracle to vouch that the nonce came from GitHub.
