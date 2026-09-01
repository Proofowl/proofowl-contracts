# ProofOwl integration contract

Everything the future `proofowl-backend` and `proofowl-frontend`
repositories need to consume the Soroban contract without guesswork.
**Only the contract and the TypeScript SDK exist today.**

The deployed WASM / contract ABI is authoritative; these documents
describe it in prose and are versioned (`-v1`). A change to the
contract's public interface adds a `-v2` file rather than editing these.

| Document | What it pins down |
|---|---|
| [`contract-api-v1.md`](./contract-api-v1.md) | Every function: params, returns, auth, mutability, errors, events, TTL effects, backend/frontend notes. The two-party authorization rule. |
| [`identifier-spec-v1.md`](./identifier-spec-v1.md) | Canonical `github_id_hash` (from the immutable numeric user id) and `pr_hash` (`github.com/<owner>/<repo>/pull/<n>`): exact normalization, hashing, encoding, rejection rules, worked vectors. |
| [`attestor-protocol-v1.md`](./attestor-protocol-v1.md) | What the backend must verify before it uses the attestor key: OAuth ownership proof, PR verification, repo policy, complexity tiers, idempotency, key rotation, audit logging, error mapping, trust boundaries. |
| [`event-indexer-v1.md`](./event-indexer-v1.md) | Every event's topics + data, ordering/idempotency, replay safety, `(network, contractId)` partitioning, TTL monitoring, building passports, and the "read methods are authoritative" rule. |
| [`sequence-diagrams.md`](./sequence-diagrams.md) | Mermaid flows for verified linking, verified PR attestation, and read-only passport lookup. |

TypeScript SDK: [`../../sdk/typescript/`](../../sdk/typescript/) — typed
read-only client, unsigned-transaction preparation helpers, and the
canonical hash helpers from the identifier spec. It never signs,
submits, or reads a keystore.

Related existing docs: [`../../SECURITY.md`](../../SECURITY.md) (trust
model), [`../adr/`](../adr/) (why the contract is shaped this way),
[`../operations/testnet-deployment.md`](../operations/testnet-deployment.md)
§7 (two-party signing on the CLI),
[`../testnet/phase2-alpha.md`](../testnet/phase2-alpha.md) (the deployed
testnet instance).
