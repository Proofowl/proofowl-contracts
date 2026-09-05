# Contributing to proofowl-contracts

Thanks for picking this up — whether you found it through a Stellar Wave
issue or on your own.

## Prerequisites

- **Rust 1.91+** (`rustup`) with the Soroban wasm target:
  `rustup target add wasm32v1-none`. CI pins the exact stable toolchain
  **1.91.0** (`.github/workflows/ci.yml`, `Cargo.toml`'s `rust-version`);
  this is the verified minimum, driven by `soroban-sdk 27.0.6`'s own
  declared `rust-version = 1.91.0`, which Cargo enforces at build time.
  A newer stable toolchain also works locally — the pin is a floor.
  (Historically `soroban-sdk 27` also dropped `wasm32-unknown-unknown`
  on Rust ≥ 1.82; `wasm32v1-none` is the only Soroban wasm target now.)
- **Node ≥ 22.6** with **npm** — only if you touch `sdk/typescript/`.
  CI uses Node 24; this repo was last verified with Node 24.20.0 /
  npm 11.19.0. `sdk/typescript/README.md` has the SDK-specific steps.

## Workflow

1. Fork and clone the repo.
2. Install the toolchain above.
3. `cargo test --locked --all` before you touch anything, so you know
   the baseline passes (`--locked` uses the committed `Cargo.lock`, as
   CI does).
4. Make your change. Every new contract function needs at least one
   passing-path test and one failing-path test (see `src/test.rs` for the
   pattern — `try_*` client methods return `Result` instead of panicking,
   which is what you want for asserting on error cases). Anything that
   changes who can call what, or the trust model, must be reflected in
   `SECURITY.md` and, if it's a real decision, a new ADR under
   `docs/adr/`.
5. Before opening a PR, run the local quality gate — CI enforces the
   same steps:
   ```
   make check
   ```
   which is `cargo fmt --all -- --check`, `cargo clippy --locked
   --all-targets -- -D warnings`, `cargo build --locked --target
   wasm32v1-none --release` (before the tests — `tests/constructor_auth.rs`
   loads the compiled artifact and skips itself if it is missing),
   `cargo test --locked --all`, then `scripts/check_bounded_storage.sh`.
   Every dependency-resolving command passes `--locked` so it builds the
   exact committed `Cargo.lock` and fails loudly on drift.
   `make help` lists every target. `Cargo.lock` is committed — update it
   in the same PR when you change dependencies, and the `supply-chain`
   CI job (`cargo deny` / `cargo audit`) must stay green. If you touch
   `sdk/typescript/`, also run `make integration-check` (needs Node).
6. Open a PR against `main`. Please describe *why*, not just *what* — this
   registry's whole value is being trustworthy, so reviewers read contract
   changes closely.

## Scope note for Wave contributors

If you picked this up from a Stellar Wave issue: please keep the PR scoped
to exactly what the issue describes. This contract intentionally keeps a
narrow trust surface (see the module docs at the top of `src/lib.rs`) —
if your change would expand what the attestor key can do, or change who
can call what, flag that in the PR description explicitly rather than
folding it in quietly.

## Code of conduct

Be respectful, be patient with review turnaround, and assume good faith.
