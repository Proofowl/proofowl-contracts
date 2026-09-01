# Release checklist

Everything here must be true **before** a version tag is created or a
deployment is made. Copy this list into the release PR / issue and tick
it off. If an item cannot be satisfied, the release does not go out.

## A. Repository hygiene

- [ ] Working tree clean; release is cut from `main` (or an agreed
      release branch) at a known commit SHA.
- [ ] `CHANGELOG.md` `[Unreleased]` section reflects every user-visible
      change since the last release.
- [ ] `Cargo.toml` `package.version` bumped per
      [`RELEASE_POLICY.md`](RELEASE_POLICY.md); `Cargo.lock` regenerated
      and committed.
- [ ] No `TODO` / `FIXME` / `XXX` in `src/` that blocks the release
      (record any that remain in the CHANGELOG or an issue).

## B. Local quality gate

- [ ] `make check` passes (fmt-check, clippy `-D warnings`, `wasm32v1-none`
      release build, `cargo test --all`).
- [ ] `make deny` passes, or every finding has a reviewed exception in
      `deny.toml` with a comment and a date.
- [ ] `make audit` passes, or every advisory has a documented,
      time-boxed exception.

## C. CI

- [ ] CI green on the exact release commit: `test` job and `supply-chain`
      job both pass.
- [ ] No GitHub Actions pinned to a floating `main`/`master` ref.

## D. Documentation accuracy

- [ ] `README.md` API table, error table, and trust boundaries match
      `src/lib.rs`.
- [ ] `SECURITY.md` reflects the current auth model and TTL policy.
- [ ] Any behaviour change since the last release has an ADR under
      `docs/adr/` if it was a real design decision.
- [ ] No document claims an audit, deployment, live integration, bug
      bounty, response SLA, or production usage that does not exist.

## E. Artifact

- [ ] Release WASM built from the tagged commit; its sha256 recorded in
      the release notes.
- [ ] (If deploying) `stellar contract optimize` output sha256 also
      recorded.

## F. Deployment (only if this release is going to a network)

- [ ] Target network confirmed; for this phase that is **testnet only**.
- [ ] Three distinct, correctly funded keys ready (deployer / admin /
      attestor); admin key handled offline.
- [ ] `scripts/deploy_testnet.sh` run; contract ID captured.
- [ ] `scripts/verify_config.sh` passes against the new instance.
- [ ] `scripts/smoke_test.sh` passes against the new instance.
- [ ] Instance recorded (date, commit SHA, WASM sha256, contract ID,
      admin/attestor addresses) and `README.md` "Deployed contracts"
      updated only if the instance is being kept.

## G. Tag

- [ ] `git tag -a vX.Y.Z -m "proofowl-contracts X.Y.Z"` created locally.
- [ ] Pushing the tag / publishing a GitHub release is a deliberate,
      separate maintainer action — not automated, not part of this
      checklist's completion.
