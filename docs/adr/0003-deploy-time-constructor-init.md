# ADR 0003: Initialization is a deploy-time constructor, not an `init` call

## Status
Accepted. Replaces the `init(admin, attestor)` entrypoint (with
`admin.require_auth()`) that the first hardening pass introduced.

## Context
The registry needs a one-time setup that records the `admin` and
`attestor` addresses. The first design used a public `init` function
guarded by `admin.require_auth()` plus an "already initialized" check.

An independent review found this still allows initialization takeover.
`admin.require_auth()` prevents an attacker from installing an admin
address they do not control, but it does not prevent this:

1. A contract is deployed. Between deployment and the operator's `init`
   call there is a window in which the instance exists but is
   unconfigured.
2. Anyone can call `init` in that window with an `admin` address they
   *do* control and satisfy `admin.require_auth()` themselves.
3. The `already initialized` check now works against the operator: their
   later `init` fails, and the instance is permanently captured.

Telling operators to "deploy and init in the same script run" does not
fix this — a script running two commands is two transactions with a real
gap, not an atomic operation.

## Decision
Remove `init`. Add `pub fn __constructor(env: Env, admin: Address,
attestor: Address)`. Soroban SDK 27 runs `__constructor` as part of the
`CreateContract` host operation, in the same transaction that creates the
instance. The constructor:

- writes `Admin` and `Attestor` to instance storage,
- extends the instance TTL,
- calls `admin.require_auth()` so the deploy transaction must be signed
  by the admin (binding config to a deployer-authorized setup and
  catching a wrong admin address),
- emits `Initialized`.

Deployment via `stellar contract deploy … -- --admin <A> --attestor <B>`
passes these as constructor arguments; there is no follow-up call.

## Consequences
- **No initialization race.** The instance does not exist until the
  transaction that configures it commits. A front-runner who deploys
  their own copy only gets a different contract id.
- **No `AlreadyInitialized` path.** The constructor cannot run twice
  on-chain. The error variant is kept (reserved) only so existing error
  codes don't shift.
- **`register` cannot test constructor-auth rejection.** `Env::register`
  force-mocks constructor authorization (documented SDK behaviour). The
  real deployer-auth path is covered instead by an integration test
  (`tests/constructor_auth.rs`) that uploads the compiled wasm and calls
  `Deployer::deploy_v2` with and without authorization.
- **CI must build the wasm before running tests**, so that integration
  test exercises the real artifact rather than skipping.
- **Deploy tooling change.** Operators pass constructor args to `deploy`
  and no longer run a second `init` invoke. README, SECURITY.md, and the
  deployment checklist are updated.

## Alternatives considered
- **Keep `init`, add a deployer check** (`env.deployer()` / source
  account binding). Soroban does not expose the deploying account to a
  regular contract function after the fact, and re-deriving it is
  fragile. The constructor is the SDK-supported mechanism for exactly
  this.
- **Keep both `init` and a constructor.** Reintroduces the `init` attack
  surface for no benefit.
