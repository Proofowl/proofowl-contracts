# Maintainers' operational checklist

Routine tasks for people with merge rights on this repository. This is an
operational companion to [`RELEASE_POLICY.md`](RELEASE_POLICY.md) and
[`../CONTRIBUTING.md`](../CONTRIBUTING.md).

## On every pull request

- [ ] `make check` is green in CI.
- [ ] `supply-chain` CI job is green, or the PR adds a reviewed
      `deny.toml` exception with a comment and a date.
- [ ] If the PR changes an exported function, an error variant, a stored
      type, the auth model, or scoring: it updates `README.md`,
      `SECURITY.md`, and adds/updates an ADR. Flag it as breaking per
      `RELEASE_POLICY.md`.
- [ ] `CHANGELOG.md` `[Unreleased]` updated for any user-visible change.
- [ ] No secrets, real contract IDs, credentialed URLs, or generated
      artifacts (`target/`, `test_snapshots/`, `.env`) in the diff.
- [ ] Commit messages carry **no** co-author / assisted-by trailer
      (project rule).

## Cutting a release

Follow [`RELEASE_CHECKLIST.md`](RELEASE_CHECKLIST.md) end to end. Tagging
is local; pushing the tag and publishing a GitHub release are separate
deliberate actions.

## Deploying to testnet

Follow [`operations/testnet-deployment.md`](operations/testnet-deployment.md).
The contract is immutable — a bad deploy is replaced, not patched. Record
every instance.

## Rotating the attestor key

1. Prepare the new attestor address / signer.
2. From the **admin** identity: `set_attestor(admin, new_attestor)`.
3. `scripts/verify_config.sh` to confirm the new attestor is live.
4. Hand the new signer to the backend; retire the old one.
5. Note the rotation in the instance log.

The admin key cannot be rotated — it is fixed at deploy. Losing it means
losing the ability to rotate the attestor for that instance.

## Responding to a security report

See [`../SECURITY.md`](../SECURITY.md) for the report intake. On receipt:

1. Acknowledge privately. Do not discuss in public issues/PRs.
2. Reproduce and assess scope against the trust model in `SECURITY.md`.
3. Fix on a private branch; add a regression test.
4. Decide disclosure timing with the reporter.
5. Release per the checklist; credit the reporter if they consent.

There is currently **no bug bounty and no committed response time** —
do not imply otherwise in any reply or document.

## CI toolchain

- **Rust is pinned to an EXACT stable release: `1.91.0`.** Four places
  must stay in lock-step — the four `dtolnay/rust-toolchain@1.91.0` uses
  in `.github/workflows/ci.yml`, the `rust-1.91.0` segment of the
  `supply-chain` job's cache key, `Cargo.toml`'s `rust-version`, and
  `RUST_TOOLCHAIN_MIN` in the `Makefile`.
- **Why 1.91.0 specifically:** it is the *verified minimum*, not the
  latest. The binding constraint is `soroban-sdk 27.0.6` (and its
  `-macros` / `-spec` / `-ledger-snapshot` siblings), which declare
  `rust-version = 1.91.0`; Cargo refuses to build a crate whose
  `rust-version` exceeds the active toolchain. 1.91.0 is also above the
  floor for parsing the edition-2024 transitive manifests in
  `Cargo.lock` (needs Cargo ≥ 1.85) and above `cargo-deny 0.20.2`'s
  and `cargo-audit 0.22.2`'s own MSRV of 1.88.
- **How to bump it** (e.g. when upgrading `soroban-sdk`): pick the new
  verified minimum by checking the max `rust_version` across the locked
  graph —
  `cargo metadata --locked --format-version 1 | jq -r '.packages[] | select(.rust_version) | .rust_version' | sort -V | tail -1` —
  install *that exact* toolchain locally, run `make check`, `make
  security-test`, and a fresh `cargo install --locked` of both pinned
  security tools plus `make deny` / `make audit`, then update all four
  places above together. Do **not** move to a floating `1.x` /
  `stable` pin; the exact pin is deliberate.
- CI Cargo commands pass `--locked` everywhere they resolve
  dependencies (`cargo audit` excepted — it reads `Cargo.lock`
  directly), so a stale lockfile fails CI instead of being silently
  re-resolved.
- **The checked-in TypeScript bindings are toolchain-coupled.**
  `stellar contract bindings typescript` emits the `ContractSpec([…])`
  array in the order the entries appear in the WASM's `contractspecv0`
  custom section, and that order is rustc-version-dependent (entry
  *contents* are stable; their *sequence* is not). So
  `sdk/typescript/src/generated/index.ts` must be regenerated on the
  pinned toolchain — `make sdk-generate` after a
  `cargo build --locked --target wasm32v1-none --release` on 1.91.0 —
  and committed, or the `sdk-bindings-drift` job (which rebuilds the
  WASM on the pin) fails. Bumping the toolchain pin therefore also
  means regenerating and committing this file in the same change.

### Note — the 2026-09 CI-red period (resolved)

Between roughly the Phase 1 commits and 2026-09-05, the `test`,
`supply-chain`, and `sdk-bindings-drift` jobs were red on `main`. Root
cause, from the run logs (not speculation): CI pinned
`dtolnay/rust-toolchain@1.84`, and over time (a) transitive dependencies
in `Cargo.lock` gained edition-2024 manifests that Cargo 1.84 cannot
parse, and (b) the pinned `cargo-deny 0.20.2` raised its own MSRV to
1.88. Local `make check` kept passing because dev machines run a much
newer stable. The fix (this commit's sibling `ci:` commit) was to pin
the exact verified minimum toolchain, `1.91.0`, and add `--locked`; no
check was weakened and no security tool was downgraded. `sdk` was a
separate, later regression — two SDK files not run through Prettier —
fixed by `style: format typescript sdk sources`. Pinning the toolchain
also surfaced a follow-on `sdk-bindings-drift` mismatch: the committed
`src/generated/index.ts` had been generated on a newer stable, whose
`contractspecv0` section orders two entries differently from 1.91.0's;
regenerating on the pinned toolchain (one adjacent-pair swap,
byte-identical contents) cleared it.

## Keeping supply-chain checks healthy

- The `supply-chain` job also runs on a weekly schedule so a newly
  published advisory is surfaced even without a commit.
- The Rust toolchain pin and the `cargo-deny` / `cargo-audit` version
  pins are coupled: both tools must build on the pinned toolchain.
  Re-check this whenever either the toolchain or a tool version moves
  (see "CI toolchain" above).
- When `cargo audit` / `cargo deny advisories` flags a transitive dep:
  prefer bumping `Cargo.lock`; if no fixed version exists, add a
  time-boxed `deny.toml` exception with a tracking note.
- Review GitHub Actions pins periodically. They are currently pinned to
  major version tags; moving to full commit-SHA pins is a hardening task
  that needs a maintainer with network access to verify each SHA against
  the upstream action repository (see `PRODUCTION_READINESS.md`).
- Enabling Dependabot for the `cargo` and `github-actions` ecosystems is
  recommended; it is not configured in-repo so that no automated PRs are
  created without a maintainer opting in.

## Once an instance is live (future phase)

- Monitor that active passports stay above the TTL threshold; run or
  schedule `bump_wallet_core_ttl` + a paginated `bump_attestations_ttl_page`
  sweep for cold records (see `SECURITY.md` §5).
- Watch contract events for anomalies (unexpected `set_attestor`,
  attestation spikes).
