# ProofOwl integration contract

Everything the future `proofowl-backend` and `proofowl-frontend`
repositories need to consume the Soroban contract without guesswork.
**Only the contract and the TypeScript SDK exist today.**

The deployed WASM / contract ABI is authoritative; these documents
describe it in prose and are versioned (`-v1`, `-v2`, …). A change to
the contract's public interface adds a new version file rather than
editing an existing one — see
[`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md) for
the current v0.1 → v0.2 transition. **No v0.2 instance has been
deployed to any network as of this document** — v0.2 is a local
candidate only.

## Current (v2 — target this for any new work)

| Document | What it pins down |
|---|---|
| [`contract-api-v2.md`](./contract-api-v2.md) | Every function: params, returns, auth, mutability, errors, events, TTL effects. Paginated attestation reads/TTL maintenance; error codes 1–13. |
| [`attestor-protocol-v2.md`](./attestor-protocol-v2.md) | What the backend must verify before it uses the attestor key. Unchanged from v1 except idempotency/reconciliation (§8) and the error table (§13); read v1 alongside it. |
| [`event-indexer-v2.md`](./event-indexer-v2.md) | Every event's topics + data (`AttestationRecorded` gained `sequence`), paginated TTL monitoring, paginated passport-building. |
| [`identifier-spec-v1.md`](./identifier-spec-v1.md) | Canonical `github_id_hash` / `pr_hash` derivation — **unchanged for v0.2**, still normative, no `-v2` exists because nothing here changed. |
| [`sequence-diagrams.md`](./sequence-diagrams.md) | Mermaid flows, updated for the v0.2 API (paginated reads, bounded TTL calls). |

## Historical (v1 — describes the deployed testnet alpha instance only)

| Document | Superseded by |
|---|---|
| [`contract-api-v1.md`](./contract-api-v1.md) | [`contract-api-v2.md`](./contract-api-v2.md) |
| [`attestor-protocol-v1.md`](./attestor-protocol-v1.md) | [`attestor-protocol-v2.md`](./attestor-protocol-v2.md) |
| [`event-indexer-v1.md`](./event-indexer-v1.md) | [`event-indexer-v2.md`](./event-indexer-v2.md) |

These are kept unedited as the accurate description of what the v0.1
testnet alpha instance
(`CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6`) actually
runs. Do not build a new integration against v0.1 — see
[`../migrations/v0.1-to-v0.2.md`](../migrations/v0.1-to-v0.2.md) for why
and what v0.1's place is going forward.

## SDK

TypeScript SDK: [`../../sdk/typescript/`](../../sdk/typescript/) — typed
read-only client with paginated attestation helpers, unsigned-transaction
preparation helpers, and the canonical hash helpers from the identifier
spec. Targets the v0.2 ABI; it never signs, submits, or reads a
keystore.

Related existing docs: [`../../SECURITY.md`](../../SECURITY.md) (trust
model), [`../adr/`](../adr/) (why the contract is shaped this way,
including [ADR 0004](../adr/0004-paginated-attestation-storage.md) for
the v0.2 storage redesign), [`../security/`](../security/) (threat
model, resource profiles v1/v2, known risks, security review
checklist), [`../operations/testnet-deployment.md`](../operations/testnet-deployment.md)
§7 (two-party signing on the CLI — still applicable to v0.2),
[`../testnet/phase2-alpha.md`](../testnet/phase2-alpha.md) (the v0.1
testnet instance).
