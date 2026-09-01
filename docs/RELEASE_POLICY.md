# Release & versioning policy

## Scope

This policy covers the `proofowl-contracts` crate and the WASM it
produces. It does **not** govern deployed contract instances: deploying,
or upgrading to, a new version is always a separate, deliberate act (see
[`operations/testnet-deployment.md`](operations/testnet-deployment.md)).

## Versioning

The crate follows [Semantic Versioning 2.0.0](https://semver.org). The
version lives in `Cargo.toml` (`package.version`) and is mirrored in
`CHANGELOG.md`.

### Pre-1.0

While the version is `0.y.z`, the project is explicitly unstable: **any
release may change contract behaviour, storage layout, error codes, or
interfaces.** `0.y+1.0` is used for changes that would be breaking at
1.x; `0.y.z+1` for backwards-compatible fixes and additions.

### What is a breaking change for an on-chain contract

At 1.x, a **major** bump is required for any of:

- a change to an exported function's name, parameters, or return type;
- a change to `#[contracterror]` variants or their numeric codes;
- a change to a stored type's layout (`Attestation`, `DataKey`) or to a
  storage key's shape;
- a change to who must authorize a call (the auth model);
- a change to scoring semantics or the accepted `complexity` set;
- removing or repurposing an event.

A **minor** bump covers backwards-compatible additions: a new read
function, a new event, a new optional maintenance entrypoint.

A **patch** bump covers changes with no observable interface or
behaviour difference: docs, tests, comments, internal refactors,
dependency bumps that do not change generated WASM behaviour.

Because deployed instances are immutable (no upgrade mechanism), a major
bump in practice means "a new deployment at a new contract ID", not an
in-place upgrade.

## Release process

Cutting a release is a maintainer action. It never happens automatically
and never from CI.

1. Confirm the [release checklist](RELEASE_CHECKLIST.md) is fully green.
2. Bump `package.version` in `Cargo.toml`; run `cargo update -p
   proofowl-contracts` so `Cargo.lock` matches.
3. Move the `CHANGELOG.md` `[Unreleased]` entries under a new
   `## [x.y.z] - YYYY-MM-DD` heading; add a fresh empty `[Unreleased]`;
   update the link references at the bottom.
4. Commit: `chore(release): x.y.z`.
5. Tag locally: `git tag -a vx.y.z -m "proofowl-contracts x.y.z"`.
   Pushing the tag and publishing a GitHub release are separate manual
   steps a maintainer takes when ready — they are out of scope for
   automated tooling in this repo.
6. If this release is going to a network, follow the operations guide;
   record the instance (commit SHA, WASM sha256, contract ID, addresses).

## Relationship to networks

- A version tag does **not** imply a deployment.
- A deployment does **not** require a fresh tag, but SHOULD be made from a
  tagged commit so the on-chain WASM is traceable.
- `README.md` lists deployed contract IDs per network; it is updated only
  when an instance is intended to be kept.

## Supply chain

Dependency changes (`Cargo.toml`, `Cargo.lock`) go through the
`supply-chain` CI job (`cargo deny` + `cargo audit`). A release must not
carry an unresolved `deny`/`audit` finding without an explicit,
documented exception in `deny.toml`.
