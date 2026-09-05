# ProofOwl contract — formal threat model v1

Status: **normative** for Phase 4 (adversarial and security testing).
Scope: `src/lib.rs` (the deployed Soroban registry contract) and the
protocols it depends on (`docs/adr/`, `docs/integration/*-v1.md`,
`SECURITY.md`). Out of scope: the not-yet-built backend, indexer, and
frontend implementations themselves — this document states what they
are *required* to do and what happens if they don't, but it cannot test
code that doesn't exist yet.

**This contract is not trustless.** It removes trust from everywhere it
can (initialization, wallet custody, wallet↔identity redirection,
double-crediting) and concentrates what remains in one place: the
attestor key. Read every item below with that in mind. Anywhere this
document could be read as "the contract prevents X," check whether X is
actually "the contract prevents X *given an honest attestor*" — if so,
that qualifier is load-bearing, not decorative.

## How to read this document

Each entry gives: **attacker capability**, the **asset** at risk, the
**trust boundary** crossed, the **mitigation** (contract-enforced,
protocol-enforced, or accepted), the **residual risk**, and a
**severity** rating.

Severity rubric (shared with
`docs/security/security-review-checklist-v1.md`):

| Severity | Meaning |
|---|---|
| **Critical** | Direct loss/theft of funds or reputation, or total bypass of a stated invariant, reachable by an unprivileged attacker. |
| **High** | Same impact, but requires a privileged party (attestor/admin) to be compromised or complicit, or requires an off-chain component that doesn't exist yet to be built wrong. |
| **Medium** | Degrades availability, increases cost, or corrupts derived (off-chain) state without corrupting on-chain truth. |
| **Low** | Theoretical, requires an already-broken assumption elsewhere (host bug, hash collision), or is cosmetic. |
| **Accepted** | A known, documented design trade-off. Not a bug; tracked so it isn't rediscovered and "fixed" into a worse design. |

---

## 1. Malicious wallet holder

**Capability:** controls a Stellar keypair; can sign and submit
arbitrary transactions; can call every public contract function with
any arguments; can pay fees to spam calls.

**Assets at risk:** their own link slot; other wallets' link slots
(attempted); PR-dedup markers (attempted); contract read availability.

**Trust boundary:** wallet signature ↔ contract state. A wallet
signature proves control of a Stellar key and nothing else.

**Mitigations (contract-enforced):**
- Cannot link a `github_id_hash` without the attestor's co-signature
  (`link_github` requires both `wallet.require_auth()` and
  `attestor.require_auth()` with the caller checked against the stored
  attestor) — ADR 0002.
- Cannot hold two identities (`WalletAlreadyLinked`) or steal an
  identity another wallet holds (`GithubAlreadyLinked`).
- Cannot call `submit_attestation` at all (attestor-only), so cannot
  self-credit reputation regardless of link state.
- Cannot re-submit a PR someone else's wallet already claimed
  (`SeenPr` is global, keyed by `pr_hash`, independent of caller).
- Cannot unlink someone else's link (`unlink_github` needs the
  *currently linked* wallet's signature) or forge `LinkNotFound`-passing
  state.

**Residual risk:** a wallet holder can grief `bump_wallet_ttl` calls
against *any* wallet (it is permissionless by design) — this costs the
caller a fee and changes no data, so it is a cost-only nuisance, not a
state attack. A wallet holder who also controls a second, colluding
wallet can attempt self-referential test sequences (covered by
`tests/state_machine.rs`); none were found to break an invariant.

**Severity:** Accepted (griefing via `bump_wallet_ttl` fee cost) /
mitigated to no observed break for state manipulation. See
`tests/security_matrix.rs`, `tests/state_machine.rs`.

---

## 2. GitHub identity-squatting attempts

**Capability:** knows or can compute `github_id_hash` for any GitHub
numeric user id (the hash is **opaque, not secret** —
`identifier-spec-v1.md` §1.5, small integer preimage space, trivially
enumerable). Attempts to link a victim's identity to their own wallet.

**Asset at risk:** a victim's ability to later link their true identity;
reputation being misattributed to a squatter.

**Trust boundary:** off-chain GitHub OAuth proof → on-chain co-signature.

**Mitigation:** `link_github` cannot complete on a wallet signature
alone; it requires the attestor's co-signature, and the attestor
protocol (`attestor-protocol-v1.md` §2–4) requires it to run an OAuth +
wallet-challenge binding *before* co-signing. The contract cannot verify
this itself — it enforces the *procedure*, not the *proof*. A squatter
who cannot pass the backend's OAuth check cannot obtain the
co-signature, and without it `link_github` fails with a host auth error
before any contract logic runs.

**Residual risk:** this entire mitigation lives in a backend that
**does not exist yet** (Gate 4, `PRODUCTION_READINESS.md`). Until it is
built and its OAuth/challenge flow is itself audited, "identity
squatting is blocked" is a property of the *intended* system, not a
property you can observe on-chain today. If the backend ever co-signs
without a real OAuth check (a backend bug, not a contract bug), the
contract has no way to detect or reject that — see §3 (compromised
attestor).

**Severity:** Accepted at the contract layer (correctly delegates to
procedure); **High** at the system layer until `proofowl-backend` exists
and its OAuth binding is reviewed.

---

## 3. Compromised or malicious attestor

**Capability:** holds the attestor private key (via key theft, insider
misuse, or a bug in the not-yet-built backend that lets an attacker
drive attestor-signed calls). Can call `link_github` (co-sign only),
`unlink_github` (co-sign only), and `submit_attestation` freely against
the stored attestor value.

**Assets at risk:** the integrity of *what happened* (fabricated
contributions), complexity/tier accuracy, unlink availability for
existing links.

**Trust boundary:** this is the contract's central, deliberate trust
anchor (`SECURITY.md` §1.2, ADR 0001).

**What a compromised attestor CAN do:**
- Fabricate a `submit_attestation` for a PR that was never merged, or
  never existed, crediting a real linked wallet with false reputation.
- Misreport `complexity` (within the 4 allowed values) for a real
  contribution.
- Co-sign a link or unlink for a wallet that also signs — i.e. it can
  act as a *willing accomplice* to any wallet holder, including one it
  controls itself.
- Halt all linking/attestation by refusing to co-sign (availability,
  not integrity, impact) — indistinguishable on-chain from a
  disconnected backend.

**What a compromised attestor CANNOT do (contract-enforced):**
- Redirect an attestation to a wallet the GitHub identity has not
  itself linked (ADR 0001) — it must operate through a `GithubLink`
  entry the wallet's own key created.
- Move or delete an existing link unilaterally — `unlink_github` still
  requires the linked wallet's signature.
- Create a `pr_hash` credit twice, or exceed the 4 allowed complexity
  values.
- Change `Admin`, or read/tamper with another instance's storage.

**Mitigation:** `set_attestor` (admin-only) allows rotation without a
migration once compromise is detected; `AttestorRotated` is emitted so
watchers (indexer, backend instances) can react (attestor-protocol-v1.md
§9). Rotation is immediate and total — the old key is rejected on the
very next call (`tests/state_machine.rs`,
`set_attestor_rotates_the_signing_key`).

**Residual risk:** detection is entirely off-chain (no on-chain
anomaly detection exists or is planned for v1); the window between
compromise and rotation is exactly as long as it takes a human to
notice. A single key is a single point of failure by design
(`SECURITY.md` §7, `README.md`) — `set_attestor` to a multisig/threshold
scheme is a stated mainnet prerequisite (`PRODUCTION_READINESS.md` 6.3),
not yet done.

**Severity:** **High** (bounded blast radius — cannot steal identity or
redirect credit — but full write access to "what happened" within that
bound) until multisig rotation lands; the redirection/theft-of-identity
paths are **Accepted-by-design-mitigated** (contract-enforced, verified
in `tests/security_matrix.rs`).

---

## 4. Compromised admin

**Capability:** holds the admin private key. The admin's *only*
post-deploy power is `set_attestor(admin, new_attestor)`.

**Assets at risk:** the attestor key itself (indirectly — a compromised
admin can install an attacker-controlled attestor).

**What a compromised admin CAN do:** replace the attestor with an
address the attacker controls, at which point §3's "compromised
attestor" capabilities apply going forward (not retroactively — past
attestations and links are untouched).

**What a compromised admin CANNOT do (contract-enforced):** create,
move, or delete a wallet↔GitHub link; edit or delete an attestation;
change a reputation score; replace itself; read another instance's
storage. There is no admin override for *any* of these — deliberately
(`SECURITY.md` §1.4, §4.2). This is verified operationally in
`tests/security_matrix.rs::admin_powers_are_limited_to_attestor_rotation`
— every other mutating path is exercised with the admin key attempting
it and rejected.

**Mitigation:** the admin key cannot be rotated post-deploy at all —
there is no `set_admin`. This bounds admin compromise to "can swap the
attestor," which is itself detectable (`AttestorRotated` event) and
reversible (rotate again). It also means **admin key loss is
permanent and unrecoverable** — see §13.

**Residual risk:** an admin compromise combined with slow detection has
the same blast radius as an attestor compromise (§3), just one hop
removed. `PRODUCTION_READINESS.md` 6.4 requires a hardware-signer
custody policy for the admin key before mainnet; not yet done.

**Severity:** **High** until hardware-signer custody is documented and
enforced operationally (this is a key-custody control, not something
the contract can enforce).

---

## 5. Malformed or hostile backend input

**Capability:** the future backend (or a bug in it) passes unexpected
values to `submit_attestation` / `link_github` — empty/very long
`repo` strings, `pr_number` or `issue_id` at `0` or `u32::MAX`/`u64::MAX`,
non-canonical `pr_hash` values, `complexity` outside the allowed set.

**Asset at risk:** on-chain data integrity and contract availability
(panic risk).

**Mitigation (contract-enforced):**
- `complexity` is checked against `ALLOWED_COMPLEXITY` **before** any
  storage read or write — `InvalidComplexity` on any other value,
  atomically (nothing is persisted).
- `repo` is an opaque `String` the contract never parses, indexes, or
  branches on — there is no length limit, control-character check, or
  charset restriction *in the contract*, and none is needed for
  correctness or safety: the string is stored and returned verbatim,
  never used as a storage key, never concatenated, never used in a
  size-sensitive loop. A pathological `repo` value degrades read/display
  quality off-chain (indexer/frontend problem) and marginally increases
  this attestation's own storage cost — it cannot corrupt other wallets'
  data or panic the contract. Exercised in `tests/boundary_and_events.rs`
  (empty, ~800-byte, and control-character/non-ASCII repo strings).
- `pr_number` / `issue_id` have no contract-side minimum — `0` is
  accepted for both (documented as valid for `issue_id`,
  "not applicable"; `pr_number == 0` is likewise accepted on-chain even
  though the *canonical hash spec* the backend must follow requires
  `>= 1`). This is intentional: the contract does not re-derive or
  validate `pr_hash` from `repo`/`pr_number` — see §11. A backend that
  submits `pr_number: 0` produces a record whose clear-text field
  disagrees with what a correctly-derived `pr_hash` would encode; the
  indexer's mandated recompute-and-compare step
  (`identifier-spec-v1.md` §2.6) is exactly the check that catches this.
  It is a backend-input-quality problem, not a contract vulnerability.
- Arithmetic that combines untrusted input (`get_reputation_score`'s
  fold) uses `saturating_add`, never panics.
- No caller-controlled value is ever used as a loop bound tied to
  another wallet's data, so one hostile submission cannot inflate the
  cost of another wallet's reads.

**Residual risk:** a backend that submits well-formed-but-wrong data
(wrong complexity tier, wrong repo string, non-canonical `pr_hash`) is
indistinguishable on-chain from a correct submission — see §3. Storage
growth from an oversized `repo` string is bounded per-call by the
Soroban host's own transaction/entry size limits (`max_contract_data_entry_size_bytes`
in the mainnet resource limits reported by `env.cost_estimate()` —
`docs/security/resource-profile-v1.md`), not by anything this contract
adds.

**Severity:** Low (contract cannot be panicked or corrupted by malformed
input; the interesting risk is data-quality, covered under §3/§11).

---

## 6. Duplicate / replayed PR attestation

**Capability:** the attestor (accidentally via a retry, or maliciously)
submits the same `pr_hash` more than once, from the same or a different
`github_id_hash`/wallet.

**Asset at risk:** double-crediting reputation for one real-world
contribution.

**Mitigation (contract-enforced):** `SeenPr(pr_hash)` is a global,
permanent marker checked before any state is written and set atomically
with the attestation itself. It is keyed purely by `pr_hash` —
independent of wallet, GitHub identity, or attestor — so a duplicate is
rejected regardless of which identity it is resubmitted under, even
across an intervening `unlink_github` + re-link to a different wallet
(`unlink_preserves_history_and_global_pr_dedup`,
`tests/ttl_replay.rs`). The attestor-protocol explicitly directs the
backend to treat `DuplicateAttestation` as success-equivalent for
idempotent retries (§8), so this is also the *intended* mechanism for
safe retry, not merely a defense.

**Residual risk:** none identified at the contract level. The
`SeenPr` marker itself is subject to the same TTL policy as everything
else (§9); if it were ever allowed to archive, the duplicate guard could
theoretically be defeated for that one PR after restoration wiped it —
this is exactly why `bump_wallet_ttl` refreshes every `SeenPr` marker
referenced by a wallet's history, not just the history vector
(`SECURITY.md` §5, verified in `tests/ttl_replay.rs`). A `SeenPr` marker
for a wallet that has since been fully unlinked (no wallet history
references it any more, because history stays with the *original*
wallet — see §11) is **not** covered by any `bump_wallet_ttl` call
after that unlink, since no wallet's history array still points at it
unless the original wallet's own history is bumped. It stays covered
as long as the *original* crediting wallet is kept warm.

**Severity:** Accepted (fully mitigated for the documented lifetime;
depends on the original wallet's TTL being maintained — see §9).

---

## 7. Bad or inconsistent indexer

**Capability:** an indexer (not yet built) has a bug, an ordering error,
a missed event, or diverges from on-chain truth through any other
failure.

**Asset at risk:** anyone who trusts indexer-served data (a leaderboard,
a passport page) for a decision.

**Mitigation:** `event-indexer-v1.md` §0 states, and this contract's
design supports, that **read methods are authoritative and events are a
convenience cache**. Every derived fact (`link[wallet]`,
`attestations[wallet]`, `score[wallet]`) is independently recomputable
from `get_github_for_wallet` / `get_wallet_for_github` /
`get_attestations` / `get_reputation_score` at any time. `pr_hash` is
independently re-verifiable against the clear-text `repo`/`pr_number`
(§2.6). Nothing in the contract requires trusting event ordering for
correctness — only for indexer efficiency.

**Residual risk:** if a product surface serves indexer state *without*
periodic reconciliation against the read methods (which the spec
mandates but cannot enforce, since the indexer doesn't exist), a user
can be shown stale or wrong data. This is entirely a future-component
risk; the contract has no mechanism to detect or prevent it, by design
— it just never depends on the indexer being right.

**Severity:** Accepted at the contract layer / **Medium** at the system
layer until an indexer exists and its reconciliation loop is verified.

---

## 8. RPC failure or stale event history

**Capability:** Soroban RPC is down, lagging, or has rotated past its
event-retention window (`event-indexer-v1.md` §2 — "days," not
indefinite) when a consumer needs history.

**Asset at risk:** availability of a complete audit trail from genesis.

**Mitigation:** the read methods (`get_attestations`,
`get_reputation_score`, link getters) always reflect *current* state
regardless of RPC event retention — they read live ledger state, not
the event log. A consumer that lost the ability to replay history from
genesis can still recover full current state for any wallet or
identity via the read methods; what it permanently loses is only the
*timeline* of intermediate events for a gap outside both the RPC
retention window and its own indexed history — the current facts are
never lost.

**Residual risk:** an indexer that both (a) never captured events in
the gap and (b) has no archival event source falls back to read methods
per the spec, but this yields *current state*, not *point-in-time
history* for that gap — e.g. it cannot tell you exactly when within a
missed window a link was created if it was later unlinked and relinked
within that same window (only the current link, if any, is
reconstructable; intermediate transitions are lost). This is a data
completeness limitation for the future indexer, not a contract
vulnerability — the contract itself has no history requirement beyond
"reads reflect the current, correct state."

**Severity:** Low / Accepted.

---

## 9. Storage TTL expiration

**Capability:** no attacker action required — this is a systemic risk
from neglect (no one calls `bump_wallet_ttl` for a dormant wallet for
long enough).

**Asset at risk:** availability of a wallet's link/history (reads fail
against an archived entry until restored); in the specific case of a
`SeenPr` marker, the *duplicate-PR guard*.

**Mitigation (contract-enforced):** every mutating call extends the TTL
of every entry it touches (`extend_persistent`/`extend_instance`); the
permissionless `bump_wallet_ttl` refreshes the wallet link, the GitHub
link, the full history vector, and **every** `SeenPr` marker referenced
by that history in one call (`SECURITY.md` §5). 120-day extend target,
90-day bump threshold, both defensively clamped to
`env.storage().max_ttl()`.

**What this contract does NOT do:** it cannot make anyone actually
*call* `bump_wallet_ttl`. A wallet that links, gets zero attestations,
and is never touched again for >120 days (mainnet) will have its
`WalletLink`/`GithubLink` entries archive. This is availability
degradation, not data loss — `RestoreFootprint` can revive an archived
persistent entry, and the underlying value is unchanged by archival.

**Residual risk / what cannot be fully emulated locally:** the
in-process Soroban test `Env` extends and reports TTLs realistically,
but does **not** enforce read failures once a TTL reaches zero the way
a live network's archival does — see
`docs/security/known-risks-v1.md` and the notes in
`tests/ttl_replay.rs` for exactly what was and wasn't exercised
locally. The *policy* (extend-to / threshold values, which entries are
covered) is fully tested; the *archival failure mode itself* is
documented, not reproduced, in a unit test.

**Severity:** Accepted (documented, permissionless remedy exists) for
normal dormancy; **Medium** operational risk if the future
backend/indexer does not implement the mandated keep-alive sweep
(`event-indexer-v1.md` §6) — this is a backend obligation, not a
contract gap.

---

## 10. Front-running / transaction ordering

**Capability:** an attacker observes a pending transaction (e.g. in the
mempool / during simulation) and tries to submit a competing transaction
first to capture a resource.

**Assets that were historically at risk:** contract initialization
(ADR 0003); a `github_id_hash` slot between two competing `link_github`
calls.

**Mitigation (contract-enforced):**
- **Initialization:** there is no window to front-run. `__constructor`
  runs inside the same `CreateContract` operation that creates the
  instance; a racer who deploys first only creates a *different*
  contract id (ADR 0003, `tests/constructor_auth.rs`). Not applicable
  to an already-deployed instance.
- **Linking races:** two different wallets racing to link the *same*
  `github_id_hash` both require the attestor's co-signature on that
  exact call. The attestor is expected to co-sign at most one candidate
  per identity (having verified GitHub ownership first), so this reduces
  to "can the attestor be tricked into co-signing two competing claims
  for the same identity" — a backend-implementation question (§2), not
  a contract race. At the contract level, whichever of the two
  attestor-co-signed transactions lands first wins
  `GithubAlreadyLinked` protection for the second regardless of
  submission order chosen by the front-runner — the check is
  atomic within the transaction (`link_github_identity_squat_is_blocked`,
  extended in `tests/state_machine.rs` with racing-order permutations).
- **`submit_attestation` / `pr_hash` races:** two `submit_attestation`
  calls for the same `pr_hash` (e.g. two backend instances retrying
  the same job — attestor-protocol-v1.md §8) resolve deterministically:
  exactly one lands `Ok`, the other gets `DuplicateAttestation`,
  regardless of ordering. Verified in `tests/state_machine.rs` and
  `duplicate_pr_hash_rejected_globally`.

**Residual risk:** none identified that isn't already reduced to "trust
the attestor's off-chain decision" (§2, §3). There is no MEV-style value
extractable from reordering these calls — outcomes are the same
regardless of which of two conflicting transactions the network orders
first; only *which* of two symmetric candidates wins is order-dependent,
never *whether* the invariant holds.

**Severity:** Accepted (structurally prevented, not merely mitigated).

---

## 11. Denial-of-service / resource exhaustion

**Capability:** the attestor (only the attestor can call
`submit_attestation`) submits a very large number of attestations to one
wallet, growing that wallet's `Vec<Attestation>` without bound.

**Asset at risk:** the cost of `get_attestations`, `get_reputation_score`,
and `bump_wallet_ttl` for that one wallet; in the extreme, the ability to
call `bump_wallet_ttl` for that wallet within a single transaction's
resource budget at all.

**Mitigation:** none at the contract level today — this is the
documented MVP scalability limitation (`SECURITY.md` §7, module doc
comment in `src/lib.rs`). It is bounded by two things that are *not*
contract guarantees: (1) only the trusted attestor can grow any
wallet's vector, so this is a privileged-party cost-griefing vector, not
an open one; (2) each `submit_attestation` itself costs the attestor a
fee that grows with history size (it must deserialize, append, and
re-serialize the whole vector), so the attestor bears an increasing
cost to keep attacking its own victim.

**Measured, not invented:** `docs/security/resource-profile-v1.md`
gives instrumented (not hand-estimated) CPU/memory/fee figures from
`env.cost_estimate()` at several history sizes, with an explicit verdict
on where this stops being safe for a single transaction.

**Residual risk:** a wallet with a sufficiently large history could
reach a point where `bump_wallet_ttl` (or even `get_attestations`) no
longer fits the mainnet per-transaction resource limits reported by
`InvocationResourceLimits::mainnet()`, permanently locking that wallet's
TTL maintenance out of a single transaction (there is no pagination to
fall back to). This is the central finding of the resource-profile
review — see that document for the exact threshold and the verdict.

**Severity:** **Medium** today (requires attestor privilege or
complicity, bounded by attestor's own fee cost, and not reachable at
current expected MVP volumes); becomes the **primary blocker** for
unbounded production/mainnet scope without the storage redesign already
flagged in `SECURITY.md` §7 — see the resource profile's verdict.

---

## 12. Identifier-hash collisions or canonicalization disagreement

**Capability:** a SHA-256 collision (computationally infeasible,
included for completeness), or — the practically relevant case — the
backend/SDK/indexer disagree about how to canonicalize a GitHub user id
or PR reference before hashing it.

**Asset at risk:** wrong wallet credited (user-id canonicalization
disagreement); a PR silently treated as a "new" one when it is really a
duplicate under a different canonicalization, or vice versa (PR
canonicalization disagreement).

**Mitigation:**
- The contract itself never computes or checks either hash — both are
  opaque `BytesN<32>` to it (`identifier-spec-v1.md` intro). This means
  the contract *cannot* be attacked via a canonicalization bug in its
  own code — there isn't any. The entire risk lives in whichever
  off-chain component computes the hash.
- `identifier-spec-v1.md` is normative and versioned (`v1`); its
  reference implementation is `sdk/typescript/src/identifiers.ts`, with
  pinned output vectors in `identifiers.test.ts`. Any change to a
  canonicalization rule is a new version with a new domain/version
  prefix (`proofowl:github-user:v1:` → `v2:`) precisely so old and new
  hashes never collide or get silently reinterpreted.
- `tests/sdk_vectors.rs` (added this phase) recomputes the **same**
  pinned vectors using the contract's own host `env.crypto().sha256()`
  and asserts byte-for-byte agreement with the TypeScript SDK's output —
  proof that "SHA-256 of this exact canonical string" means the same 32
  bytes in both languages, closing the gap between "the spec says so"
  and "it was actually verified across the two implementations that
  matter."
- `verifyAttestationPrHash` (SDK) / `identifier-spec-v1.md` §2.6 lets
  any consumer recompute `pr_hash` from the clear-text `repo`/`pr_number`
  the contract stores and flag a mismatch — this is the safety net for
  a canonicalization *bug* in whatever produced the stored `pr_hash`
  (the contract does not do this check itself; see §5).

**Residual risk:** a *second* independent implementation of the spec
(e.g. inside the future Go/Python backend, if it doesn't literally reuse
`sdk/typescript`) could still diverge from the TypeScript reference if
it's hand-rewritten rather than vetted against the pinned vectors. This
is a process risk for whoever builds the backend, not something this
repository can close today.

**Severity:** Low for the contract itself (no canonicalization logic to
break); **Medium** process risk for a future independent
re-implementation that isn't vector-tested against the same pins.

---

## 13. Key loss and recovery limitations

**Capability:** the contributor loses their wallet key; or the admin
loses the admin key.

**Asset at risk:** the contributor's ability to unlink/relink or prove
new attestations under a fresh wallet without losing history; the
project's ability to ever rotate the attestor again.

**Wallet key loss (documented, `SECURITY.md` §4.2):**
`unlink_github` requires the *linked wallet's* signature — there is
deliberately **no** attestor-only or admin-only override to move a link
on a lost key's behalf, because that same override would let a
compromised attestor/admin silently reassign a contributor's identity
and reputation (exactly what §2/§3's mitigations exist to prevent). The
accepted consequence: a lost wallet key means that GitHub identity is
**permanently stuck** linked to a dead wallet, with no on-chain recovery
path in v1. A future time-locked, publicly-announced recovery mechanism
is noted as future work, not built.

**Admin key loss:** there is no `set_admin`. If the admin key is lost,
`set_attestor` can never be called again — the attestor is permanently
fixed at whatever it was rotated to last. This is a direct, unavoidable
consequence of ADR 0003's "no re-initialization, ever" property: the
same design that makes takeover impossible also makes admin-key loss
unrecoverable by construction. There is no contract-level mitigation
possible for this without reintroducing a privileged override.

**Mitigation (operational, not contractual):** `PRODUCTION_READINESS.md`
6.4 requires a hardware-signer custody policy for the admin key before
mainnet. This is the only real mitigation for admin key loss — custody
practice, not code.

**Severity:** Accepted (wallet key loss — documented, deliberate
trade-off) / **High** operational risk (admin key loss — total,
permanent, and irreversible attestor-rotation lockout) until a custody
policy is in force.

---

## 14. Dependency / supply-chain compromise

**Capability:** a malicious or compromised transitive dependency in the
`soroban-sdk` tree, in the TypeScript SDK's `node_modules` tree, or in a
pinned CI tool (`cargo-deny`, `cargo-audit`, the Stellar CLI, a GitHub
Action) executes attacker code at build or CI time, or ships a
vulnerable primitive into the shipped WASM.

**Asset at risk:** the integrity of the built artifact; CI secrets (none
are used by this repo's CI beyond the default `GITHUB_TOKEN`); developer
machines running `make check`.

**Mitigation (already in place, verified this phase, unchanged):**
- `deny.toml`: `unknown-registry = "deny"`, `unknown-git = "deny"`,
  `allow-registry` limited to crates.io only, `wildcards = "deny"` — no
  dependency can silently float to an unreviewed version or come from an
  unexpected source.
- `cargo deny check` (bans/licenses/sources — deterministic from
  `Cargo.lock`) and `cargo deny check advisories` / `cargo audit`
  (RustSec DB, network-dependent) both run in CI, the latter also on a
  weekly schedule so a newly published advisory is caught without a
  commit (`ci.yml`).
- One time-boxed, dated, reasoned exception exists
  (`RUSTSEC-2024-0436`, an unmaintained-not-vulnerable transitive
  compile-time dependency with no available upgrade) — re-assessed on
  every `soroban-sdk` bump.
- `Cargo.lock` is committed; the release build is reproducible from it.
- CI tool versions (`cargo-deny`, `cargo-audit`, the Stellar CLI) are
  pinned to exact versions in both the Makefile and `ci.yml`, kept in
  sync by convention (checked in this phase's `make audit-ready` run —
  see the completion report).
- TypeScript SDK dependencies are installed from a committed lockfile
  (`npm ci`) in CI; the SDK has one runtime dependency
  (`@stellar/stellar-sdk`).

**Residual risk:** GitHub Actions are pinned to **major version tags**,
not commit SHAs (`PRODUCTION_READINESS.md` 2.5, `ci.yml` header comment)
— a tag can be moved by the action's maintainer (or, if their account is
compromised, by an attacker) to point at different code without the pin
changing. This is a known, tracked, **not yet fixed** hardening gap,
carried forward unchanged from Phase 3 — Phase 4 does not fix it (it
needs a maintainer with network access to verify each SHA, which is
outside a deterministic/offline testing phase). The npm dependency tree
(`sdk/typescript/node_modules`) is not run through an equivalent
deny/audit-style gate in this repository today — `npm ci --no-audit
--no-fund` in CI explicitly skips `npm audit`; this is a gap, not a
mitigated risk. See `docs/security/known-risks-v1.md`.

**Severity:** Medium (deterministic Rust supply-chain gates are strong
and enforced; the two gaps above — floating Action tags, no `npm audit`
gate — are real and open, not currently exploited to our knowledge).

---

## 15. Protocol / API downgrade or incompatible SDK usage

**Capability:** a consumer (backend, indexer, frontend, or a
third-party integrator) uses an older or hand-rolled client against a
newer contract instance, or vice versa — e.g. assumes contract
error-code `6` still means what it meant in a prior version, or submits
arguments in an outdated shape.

**Asset at risk:** correctness of error handling and data
interpretation in the consumer; potentially wrong idempotency decisions
(e.g. failing to treat `DuplicateAttestation` as success-equivalent
because a stale client doesn't recognize code `6`).

**Mitigation:**
- The contract's error codes are a `#[repr(u32)]` enum; this phase adds
  a Rust test (`tests/security_matrix.rs`) pinning each variant's
  numeric discriminant, so a future refactor that reorders the enum and
  silently renumbers an error is caught in CI before it ships — this is
  the property the TypeScript SDK's `errors.ts` / `errors.test.ts`
  already depend on (`ProofOwlErrorCode` enum values must match
  `GeneratedErrors` from the WASM-derived bindings; drift is caught by
  `errors.test.ts` and the separate `sdk-bindings-drift` CI job that
  regenerates bindings from the built WASM and diffs them).
- `docs/RELEASE_POLICY.md` defines what counts as a breaking on-chain
  change; `contract-api-v1.md` / `identifier-spec-v1.md` /
  `attestor-protocol-v1.md` / `event-indexer-v1.md` are explicitly
  versioned (`v1`), with the stated rule that a breaking change gets a
  new version file rather than a silent edit.
- "The deployed WASM and its embedded contract spec (ABI) are
  authoritative" (`contract-api-v1.md`) — a consumer can always inspect
  the live ABI (`stellar contract inspect`) rather than trust a
  possibly-stale document.

**Residual risk:** nothing on-chain can force a consumer to actually
check the ABI version or to upgrade. A third-party integrator who
hand-rolls a client against `contract-api-v1.md` without the SDK's
drift protection, and never re-checks it against a new contract
instance, can silently misinterpret a future breaking change. This is
inherent to publishing a versioned spec for external consumption — the
mitigation is "version and document rigorously," not "prevent
misuse," and that's what exists today.

**Severity:** Low for consumers using `sdk/typescript` (drift-checked in
CI); Medium for any future independent client implementation that
doesn't adopt the same drift-check discipline.

---

## 16. Scoring-integrity edge cases (supplementary)

Not in the required list, but adjacent to §5/§11 and worth stating
explicitly since it was directly tested this phase: `complexity == 0`
("confirmed, tier unknown") scores at a flat
`UNVERIFIED_COMPLEXITY_SCORE = 50` rather than `0`. This is a documented
design choice (a confirmed-but-untiered contribution still counts for
something), not a bug — but it means **`get_reputation_score` is not a
pure function of `Σ complexity`**; a naive off-chain recomputation that
forgets the `0 → 50` substitution will disagree with the contract by
`50 × (number of untiered attestations)`. `event-indexer-v1.md` §7
states the correct recompute formula explicitly for this reason.
`get_reputation_score`'s `saturating_add` means the theoretical
`u32::MAX` ceiling is unreachable at any realistic attestation count
(tens of millions of max-tier attestations would be required for one
wallet) — treated as a non-issue, not a mitigation that was engineered.

**Severity:** Accepted (documented, correctly specified for
off-chain recomputation).

---

## Summary table

| # | Threat | Severity | Contract-level status |
|---|---|---|---|
| 1 | Malicious wallet holder | Accepted | Mitigated, tested |
| 2 | GitHub identity squatting | Accepted (contract) / High (system, backend TBD) | Delegated to procedure |
| 3 | Compromised/malicious attestor | High | Bounded, not eliminated (by design) |
| 4 | Compromised admin | High | Bounded to attestor rotation only |
| 5 | Malformed/hostile backend input | Low | Rejected or harmless |
| 6 | Duplicate/replayed PR attestation | Accepted | Fully mitigated |
| 7 | Bad/inconsistent indexer | Accepted (contract) / Medium (system, TBD) | Read methods authoritative |
| 8 | RPC failure / stale event history | Low/Accepted | Read methods unaffected |
| 9 | Storage TTL expiration | Accepted / Medium (ops) | Policy tested; archival itself not locally reproducible |
| 10 | Front-running / tx ordering | Accepted | Structurally prevented |
| 11 | DoS / resource exhaustion | Medium → blocker at scale | Measured this phase, see resource profile |
| 12 | Hash collision / canonicalization disagreement | Low / Medium (process) | Cross-verified this phase |
| 13 | Key loss and recovery | Accepted (wallet) / High (admin) | Deliberate, irreversible by design |
| 14 | Dependency/supply-chain compromise | Medium | Strong Rust gates; two open gaps |
| 15 | Protocol/API downgrade | Low / Medium | Versioned + drift-checked |
| 16 | Scoring-integrity edge cases | Accepted | Documented, tested |

## Evidence

The tests supporting the mitigations above live in:

- `src/test.rs` (existing, Phase 2/3 baseline)
- `tests/constructor_auth.rs` (existing, real deploy-auth path)
- `tests/security_matrix.rs` — §1, §3, §4, §16 (new, Phase 4)
- `tests/state_machine.rs` — §1, §6, §10, §16 (new, Phase 4)
- `tests/ttl_replay.rs` — §6, §9 (new, Phase 4)
- `tests/boundary_and_events.rs` — §5, §15 (new, Phase 4)
- `tests/sdk_vectors.rs` — §12 (new, Phase 4)
- `tests/resource_profile.rs` + `docs/security/resource-profile-v1.md` — §11 (new, Phase 4)

No test in this phase submitted a testnet transaction, used a real
wallet/attestor/admin key, or made a network call. All of the above runs
fully offline against the in-process Soroban test `Env`.
