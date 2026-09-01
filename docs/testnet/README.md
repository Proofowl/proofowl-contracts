# Testnet evidence records

Public-safe records of ProofOwl contract deployments to **Stellar
testnet** and the end-to-end smoke tests run against them.

Each record contains only public data: commit SHA, WASM hash, tool
versions, the public RPC URL and network passphrase, public account
addresses (`G...`), the public contract ID (`C...`), and public
transaction hashes. **No secret keys, seed phrases, `.env` values, or
authorization-entry material appears here.**

None of this is an audit, a mainnet deployment, or a mainnet-readiness
claim. Testnet accounts are disposable and funded by friendbot.

| Record | Date (UTC) | Commit (src) | Contract ID |
|---|---|---|---|
| [`phase2-alpha.md`](./phase2-alpha.md) | 2026-09-01 | `d030908` | `CCJ7DVU2XYVFNZMHN4VPCYSPJ7HW4RPI544XG5TG42ZX7TDSUIL3SKP6` |

Related: [`phase2-retrospective.md`](./phase2-retrospective.md) — what
worked, what was hard, gaps found, and the criteria to advance to
Phase 3.
