# ProofOwl contract — known risks and accepted limitations v1

Status: **normative**. This is the honest, non-aspirational list of
what is wrong, incomplete, or trust-dependent about this system today,
written for a reader deciding whether to rely on it. Every item below
is either an accepted design trade-off (documented elsewhere, restated
here for completeness) or an open gap that has not been closed. Nothing
in this document should be read as "not a real risk" — it means "we
know about it, and here is exactly what we know."

Cross-references: `docs/security/threat-model-v1.md` (full analysis),
`docs/security/resource-profile-v1.md` (the resource ceiling in detail),
`SECURITY.md` (the original trust-model document these findings extend).

## Risks requiring action before mainnet (ranked by severity)

### R1 — Per-wallet attestation storage has a hard, unrecoverable ceiling

**Severity: was High; now Resolved in the local v0.2 candidate,
pending validation and a new testnet deployment.**

Original finding (Phase 4, kept verbatim — not erased): a single
wallet's attestation history shared one Soroban contract-data entry in
v0.1. Measured that phase: **286 attestations succeed; the 287th fails
outright** (entry size exceeds the 65,536-byte per-entry ceiling), for
that contract's `Attestation` shape and a 25-byte `repo` string. Past
that point, `submit_attestation`, `get_attestations`,
`get_reputation_score`, and `bump_wallet_ttl` all failed for that
wallet, permanently, with no admin/attestor override capable of fixing
it. Full detail: `docs/security/resource-profile-v1.md` (unedited).

**Status:** the v0.2 local candidate
(`docs/adr/0004-paginated-attestation-storage.md`) replaces the
single-entry-per-wallet design with one persistent entry per
attestation. `docs/security/resource-profile-v2.md` measured a wallet
grown to 1000 attestations (more than 3x the v0.1 ceiling) with no
failure and no entry approaching the size limit — the specific failure
mode is resolved by construction (no entry's size depends on history
length any more). "Resolved" describes the storage design and the
tests proving it in this repository today; **it does not mean v0.2 has
been deployed, audited, or exercised against a live network** — none of
those things are true yet (`docs/migrations/v0.1-to-v0.2.md`).
`resource-profile-v2.md` also flags one open item: write cost showed a
small, not-fully-root-caused growth with total contract entry count in
the local test harness — tracked as a follow-up, not a blocker for this
status change (it does not reintroduce a ceiling; see that document §2
for the honest detail).

**Remaining action before mainnet or unbounded production scope:**
deploy v0.2 to testnet under separate approval and re-validate against
a live network; complete the other mainnet blockers unrelated to
storage (independent audit, multisig attestor, hardware-signer admin
custody — unchanged, see `PRODUCTION_READINESS.md` Gate 6).

### R2 — Single trusted attestor key

**Severity: High** (requires compromise of a specific, known,
documented trust anchor — not a design flaw, but a live centralization
point).

The attestor key can fabricate that a contribution happened or
misreport its complexity tier for any already-linked identity. It
cannot redirect credit to an unlinked wallet or steal an identity
(contract-enforced, see `docs/security/threat-model-v1.md` §3). This is
the contract's central, deliberate trust anchor — restated here because
"known" does not mean "resolved."

**Status:** `set_attestor` exists specifically to rotate off a single
key without a migration; rotation to a multisig/threshold scheme has
not happened (`PRODUCTION_READINESS.md` 6.3).

**Action required before mainnet:** attestor must be a multisig or
threshold scheme, with the rotation path exercised at least once.

### R3 — Admin key loss is permanent and total

**Severity: High** (custody risk, not a code defect — but the
consequence of loss is unusually severe by design).

There is no `set_admin`. If the admin key is lost, `set_attestor` can
never be called again, ever — the attestor is permanently frozen at
whatever it currently is. This is a direct, accepted consequence of
removing any re-initialization path (ADR 0003) to close the
initialization-takeover hole; the trade-off was judged worth it, but
the resulting operational risk is real and total, not partial.

**Status:** documented (`docs/security/threat-model-v1.md` §13); no
contract-level mitigation is possible without reintroducing a
privileged override that would undermine the property ADR 0003
established. The only mitigation is custody discipline.

**Action required before mainnet:** admin key in a hardware signer with
a documented, tested custody and recovery policy
(`PRODUCTION_READINESS.md` 6.4).

### R4 — No independent third-party audit

**Severity: High** (process gap — everything in this repository's
security documentation, including this phase's work, is internal).

Every test, every threat-model entry, and every measurement in this
phase was produced without independent adversarial review from a party
that did not write the contract. Internal thoroughness is not a
substitute for that.

**Status:** not scheduled (`PRODUCTION_READINESS.md` 5.6, unchanged by
this phase).

**Action required before mainnet:** independent audit performed,
findings resolved or explicitly accepted in writing
(`PRODUCTION_READINESS.md` 6.2,
`docs/security/security-review-checklist-v1.md` §7).

## Risks acceptable for the current phase, tracked for later

### R5 — Backend, indexer, and frontend do not exist yet

Every mitigation that depends on off-chain behavior (GitHub OAuth
verification before a link, PR-merge verification before an
attestation, indexer reconciliation against read methods, TTL
keep-alive sweeps) is currently a **specification**
(`docs/integration/*-v1.md`), not a running, reviewable system.
"Identity squatting is blocked," for instance, is true of the contract
procedure but unverifiable end-to-end until a real backend exists and
is itself reviewed.

**Status:** explicit, stated non-goal of this phase and this repository
today (`PRODUCTION_READINESS.md` Gate 4: NO-GO). Not a defect to fix
here.

### R6 — GitHub Actions pinned to major-version tags, not commit SHAs

A moved tag (by the action maintainer, or an attacker who compromises
their account) changes what CI runs without changing the pin in this
repository. Carried forward unchanged from Phase 3; not addressed this
phase (needs a maintainer with network access to verify each SHA — an
online, judgment-dependent task, not a deterministic local check).

**Status:** open, tracked (`PRODUCTION_READINESS.md` 2.5).

### R7 — No `npm audit` gate for the TypeScript SDK's dependencies

CI installs the SDK's dependencies with `npm ci --no-audit --no-fund`
(`ci.yml`), deliberately skipping `npm audit`. The Rust side has strong,
deterministic supply-chain gates (`cargo deny`, `cargo audit`, both
enforced in CI); the TypeScript side currently has none beyond a
committed lockfile.

**Status:** open, newly identified and documented this phase (see
`docs/security/threat-model-v1.md` §14). Interim mitigation: manual
`npm audit` review on every dependency change
(`docs/security/security-review-checklist-v1.md` §6).

### R8 — TTL archival's actual failure mode is not locally reproducible

The in-process Soroban test `Env` tracks and reports TTL decay
realistically but does not simulate a live network's archival-on-expiry
behavior (a read against a truly archived entry failing until
`RestoreFootprint` runs). Every TTL test in this repository proves
correct *policy* (what gets extended, by how much, on which calls); none
can prove the archival failure mode itself, because the test harness has
no equivalent to trigger.

**Status:** documented limitation of the test environment, not of the
contract (`docs/security/threat-model-v1.md` §9,
`tests/ttl_replay.rs` module doc). No action item — this is a testing
tool boundary, not a contract gap.

### R9 — Backend obligations for archived state are specification-only

`docs/integration/event-indexer-v1.md` §6 states what an indexer/backend
*should* do about TTL monitoring and keep-alive sweeps. Nothing in the
contract can enforce that a real backend actually does this once built.

**Status:** explicit non-goal of the contract by design (permissionless
`bump_wallet_ttl` exists so *anyone* — including an indexer, a cron job,
or an end user — can perform the keep-alive; the contract cannot force
anyone to call it). Tracked as a backend-build-time requirement, not a
contract defect.

### R10 — Lost wallet key has no recovery path

Documented since Phase 2/3 (`SECURITY.md` §4.2), restated for
completeness: a contributor who loses their wallet key permanently loses
the ability to unlink (and therefore re-link) their GitHub identity.
This is a deliberate trade-off (an override would reintroduce the exact
identity-redirection risk ADR 0002 closed), not an oversight.

**Status:** accepted, unchanged this phase. A future time-locked,
publicly-announced recovery mechanism is noted as possible future work,
not designed or scheduled.

### R11 — Cross-wallet history migration is out of scope

Unlinking and re-linking an identity to a new wallet does not carry the
old wallet's attestation history forward — by design, since "past
reputation stays with the wallet that earned it" is itself an invariant
this phase tested and confirmed
(`tests/security_matrix.rs::reputation_score_matches_independent_recomputation`,
`tests/ttl_replay.rs::bump_wallet_ttl_still_covers_history_after_unlink`).
A contributor who needs to move to a genuinely new wallet starts a new,
separate history there.

**Status:** accepted, unchanged this phase (`SECURITY.md` §4.2).

## What this phase changed about the known-risk picture

- **New finding, not previously quantified:** R1's exact ceiling (286 /
  287) — previously `SECURITY.md` §7 stated the storage design "does
  not scale" without a number; it now has one, and a verdict.
- **New finding, not previously identified:** R7 (no `npm audit` gate).
- **Everything else above was already documented** in `SECURITY.md`,
  the ADRs, or `PRODUCTION_READINESS.md` before this phase; this
  document consolidates it into one place a reviewer or auditor can
  read without cross-referencing five files, and confirms (via the new
  test suites this phase added) that the documented mitigations for R2,
  R3, R10, R11 hold under significantly more adversarial pressure than
  they had previously been tested against.

## Update — v0.2 paginated-storage candidate

A follow-on phase implemented the storage redesign R1 called for
(`docs/adr/0004-paginated-attestation-storage.md`,
`docs/security/resource-profile-v2.md`). R1's status above is updated
to "resolved in the local v0.2 candidate, pending validation and a new
testnet deployment" — the original measurement is kept verbatim, not
erased, as the evidence record that justified the redesign. No other
risk in this document changed status: R2–R11 are all still open or
accepted exactly as stated, and v0.2 has not been deployed, audited, or
exercised against a live network. See
`docs/migrations/v0.1-to-v0.2.md` for the full scope of what changed
and what did not.
