# ADR 0001: Attestor resolves the wallet, it does not choose it

## Status
Accepted

## Context
`submit_attestation` needs to know which wallet to credit for a merged
PR. The straightforward design is for the attestor service to pass the
wallet address directly as a parameter — the backend already knows it
from its own database.

## Decision
Instead, `submit_attestation` takes a `github_id_hash` and the contract
resolves the wallet itself via the on-chain `GithubLink` mapping created
by `link_github`. If no wallet is linked for that GitHub identity yet,
the call fails with `WalletNotLinked`.

## Consequences
- A compromised, buggy, or malicious attestor key can still forge *that*
  a contribution happened, or misreport its complexity — that's an
  inherent limit of a single trusted attestor key in v1 (see
  `set_attestor` and the roadmap for the decentralization path).
- What it *cannot* do is redirect an attestation to a wallet the
  GitHub identity hasn't itself linked. The wallet side of the mapping
  always carries the contributor's own signature.
- The tradeoff: attestations can't be submitted for a contributor who
  hasn't linked a wallet yet. The backend is expected to hold verified
  facts in its own queue until linking happens, then submit — not to
  work around this by inventing a placeholder wallet.

## See also

`docs/security/threat-model-v1.md` §3 ("compromised or malicious
attestor") formalizes exactly what this ADR's guarantee does and does
not cover, with severity and test evidence
(`tests/security_matrix.rs`, `tests/state_machine.rs`).

## Update (see ADR 0002)
This ADR originally described the `GithubLink` mapping as created by "the
contributor's own signed transaction, never the attestor's say-so". As of
[ADR 0002](./0002-two-party-github-link.md) the link is created by a
**two-party** call: the wallet signs *and* the trusted attestor
co-signs (after an off-chain GitHub OAuth check). The guarantee in this
ADR is unchanged — the attestor still cannot pick an arbitrary wallet for
an attestation; it can only ever act on a link the wallet also signed.
