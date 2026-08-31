# Contributing to proofowl-contracts

Thanks for picking this up — whether you found it through a Stellar Wave
issue or on your own.

## Workflow

1. Fork and clone the repo.
2. Install Rust **1.84+** (`rustup`) with the Soroban wasm target:
   `rustup target add wasm32v1-none`. (`soroban-sdk 27` does not build
   against `wasm32-unknown-unknown` on Rust ≥ 1.82.)
3. `cargo test --all` before you touch anything, so you know the baseline
   passes.
4. Make your change. Every new contract function needs at least one
   passing-path test and one failing-path test (see `src/test.rs` for the
   pattern — `try_*` client methods return `Result` instead of panicking,
   which is what you want for asserting on error cases). Anything that
   changes who can call what, or the trust model, must be reflected in
   `SECURITY.md` and, if it's a real decision, a new ADR under
   `docs/adr/`.
5. Before opening a PR, all four must pass locally (CI enforces them):
   ```
   cargo fmt --all -- --check
   cargo clippy --all-targets -- -D warnings
   cargo test --all
   cargo build --target wasm32v1-none --release
   ```
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
