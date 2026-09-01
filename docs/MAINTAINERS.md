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

## Keeping supply-chain checks healthy

- The `supply-chain` job also runs on a weekly schedule so a newly
  published advisory is surfaced even without a commit.
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
  schedule `bump_wallet_ttl` for cold records (see `SECURITY.md` §5).
- Watch contract events for anomalies (unexpected `set_attestor`,
  attestation spikes).
