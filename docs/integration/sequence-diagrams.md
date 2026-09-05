# Integration sequence diagrams

Three flows across the ProofOwl system. **Only the Soroban contract and
the TypeScript SDK exist today** — the backend, indexer, and frontend are
future repositories, drawn here to show where each contract call fits.
Updated for the **v0.2** API (paginated attestation reads and bounded
TTL maintenance) — see
[`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md).

Cross-references:
[`contract-api-v2.md`](./contract-api-v2.md) ·
[`identifier-spec-v1.md`](./identifier-spec-v1.md) (unchanged) ·
[`attestor-protocol-v2.md`](./attestor-protocol-v2.md) ·
[`event-indexer-v2.md`](./event-indexer-v2.md).

## 1. Verified wallet ↔ GitHub linking (two-party)

```mermaid
sequenceDiagram
    autonumber
    actor Dev as Contributor
    participant FE as Frontend (planned)
    participant Wallet as Wallet (Freighter/…)
    participant BE as Backend / attestor (planned)
    participant GH as GitHub OAuth
    participant C as Soroban contract

    Dev->>FE: "Link my GitHub"
    FE->>BE: begin link
    BE->>GH: OAuth Authorization Code + PKCE
    GH-->>BE: session for numeric user id N
    BE-->>FE: challenge = proofowl-link-challenge:v1:N:<wallet>:<nonce>:<exp>
    FE->>Wallet: sign challenge
    Wallet-->>FE: wallet signature over challenge
    FE->>BE: wallet address + signed challenge
    BE->>BE: verify OAuth session (N) AND wallet signature over N
    BE->>BE: github_id_hash = SHA-256("proofowl:github-user:v1:" + N)
    Note over BE: SDK: prepareLinkGithub(config, {wallet, attestor, githubIdHash})
    BE-->>FE: unsigned AssembledTransaction (link_github)
    FE->>Wallet: sign envelope + wallet root auth entry
    Wallet-->>FE: partially-signed tx
    FE->>BE: partially-signed tx
    BE->>BE: verify invocation == link_github(wallet, attestor, github_id_hash)
    BE->>C: submit link_github, adding the attestor auth-entry signature
    C-->>C: require_auth(wallet) AND require_auth(attestor); check attestor
    C-->>C: write WalletLink + GithubLink; extend TTLs
    C-->>BE: Ok(()) ; event GithubLinked{wallet, github_id_hash}
    BE-->>FE: linked
    FE-->>Dev: passport now shows the linked identity
```

Failure branches: `WalletAlreadyLinked` / `GithubAlreadyLinked` →
surface to the user (needs an unlink first); missing wallet **or**
attestor signature → host auth error, the tx is never submitted.

## 2. Verified PR attestation (attestor-only)

```mermaid
sequenceDiagram
    autonumber
    participant Wave as Wave / review source
    participant BE as Backend / attestor (planned)
    participant GHAPI as GitHub REST API
    participant C as Soroban contract
    participant IDX as Indexer (planned)

    Wave-->>BE: "PR #<n> in <owner>/<repo> resolved issue <id>, tier T"
    BE->>GHAPI: GET /repos/<owner>/<repo>/pulls/<n>
    GHAPI-->>BE: merged == true, merged_at, author numeric id
    BE->>BE: author id == identity behind github_id_hash ?
    BE->>BE: <owner>/<repo> on allowed-repo policy ?
    BE->>BE: complexity in {0,100,150,200} ?
    BE->>BE: pr_hash = SHA-256("github.com/<owner>/<repo>/pull/<n>")  (canonical)
    Note over BE: SDK: prepareSubmitAttestation(config, {...})
    BE->>C: submit_attestation(attestor, github_id_hash, repo, pr_number, issue_id, complexity, pr_hash)
    alt happy path
        C-->>C: require_auth(attestor); complexity ok; resolve wallet from GithubLink
        C-->>C: SeenPr(pr_hash) unseen -> append Attestation; write SeenPr; extend TTLs
        C-->>BE: Ok(wallet) ; event AttestationRecorded{wallet, repo, pr_number, issue_id, complexity, pr_hash, timestamp, sequence}
        C-->>IDX: (via getEvents) AttestationRecorded
    else contributor not linked yet
        C-->>BE: Err WalletNotLinked (#7)
        BE->>BE: park job keyed by pr_hash; retry after GithubLinked
    else PR already credited
        C-->>BE: Err DuplicateAttestation (#6)
        BE->>BE: treat as success; reconcile
    end
```

The credited wallet is resolved by the contract from
`GithubLink(github_id_hash)` — the attestor never names a wallet.

## 3. Frontend passport lookup (read-only)

```mermaid
sequenceDiagram
    autonumber
    actor Viewer
    participant FE as Frontend (planned)
    participant SDK as contract-sdk (TypeScript)
    participant RPC as Soroban RPC
    participant C as Soroban contract (view)
    participant IDX as Indexer (planned, optional cache)

    Viewer->>FE: open passport for G...WALLET
    opt fast path (cache)
        FE->>IDX: GET passport(network, contractId, wallet)
        IDX-->>FE: cached history + score
    end
    FE->>SDK: createReadClient({contractId, rpcUrl, networkPassphrase})
    FE->>SDK: getGithubForWallet(wallet)
    SDK->>RPC: simulate get_github_for_wallet
    RPC->>C: (view)
    C-->>SDK: Option<BytesN<32>>
    FE->>SDK: getAttestationCount(wallet) ; getReputationScore(wallet)
    SDK->>RPC: simulate get_attestation_count / get_reputation_score
    RPC-->>SDK: u32 ; u32
    loop until a page returns fewer than MAX_PAGE_SIZE entries
        FE->>SDK: getAttestationsPage(wallet, start, MAX_PAGE_SIZE)
        SDK->>RPC: simulate get_attestations_page
        RPC-->>SDK: Attestation[] (bounded by MAX_PAGE_SIZE)
        SDK-->>FE: typed page
    end
    FE->>FE: for each attestation, verifyAttestationPrHash(repo, prNumber, prHashHex)
    Note over FE,IDX: on disagreement, the contract read wins; reconcile the cache
    FE-->>Viewer: passport (labelled with network + contractId; testnet data marked as such)
```

No signature, no fee, no state change. The read methods are
authoritative; indexer/cache state is a convenience
([`event-indexer-v2.md`](./event-indexer-v2.md) §0).
