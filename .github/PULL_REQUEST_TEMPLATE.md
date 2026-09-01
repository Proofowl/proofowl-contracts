<!--
  Thanks for contributing. Keep PRs scoped to one change. Describe WHY,
  not just WHAT — this registry's value is being trustworthy, so contract
  changes are read closely.
-->

## Summary

<!-- What does this change and why. Link the issue it closes. -->

Closes #

## Type of change

- [ ] Bug fix (no interface change)
- [ ] New feature / behaviour
- [ ] Docs / tooling / CI only
- [ ] Dependency update

## Trust-model impact

<!-- See SECURITY.md. Be explicit — reviewers will not assume "none". -->

- [ ] No change to who must authorize a call, to stored types / storage
      keys, to error variants/codes, or to scoring
- [ ] This PR **does** change one of the above, and it is described here
      and in an ADR / `SECURITY.md` update:

<!-- describe the impact -->

## Checklist

- [ ] `make check` passes locally (fmt-check, clippy `-D warnings`,
      `wasm32v1-none` release build, `cargo test --all`)
- [ ] New behaviour has both a passing-path and a failing-path test
- [ ] `CHANGELOG.md` `[Unreleased]` updated for any user-visible change
- [ ] `README.md` / `SECURITY.md` / `docs/adr/` updated if this changes
      the API, errors, trust model, or scoring
- [ ] `Cargo.lock` updated in this PR if dependencies changed;
      `supply-chain` CI job passes
- [ ] No secrets, real contract IDs, credentialed URLs, or generated
      artifacts (`target/`, `test_snapshots/`, `.env`) in the diff
- [ ] Commit messages contain **no** co-author / assisted-by trailer
      (project rule)

## Notes for reviewers

<!-- Anything that needs a closer look, or that you're unsure about. -->
