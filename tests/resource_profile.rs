//! Measured resource / scalability profile.
//!
//! This is diagnostic, not a pass/fail security gate: it exists to
//! *produce evidence* for `docs/security/resource-profile-v1.md`, not to
//! assert a specific ceiling. It is `#[ignore]`d so it never runs as
//! part of `cargo test --all` / `make check` / CI's default PR job;
//! run it explicitly with `make resource-profile`.
//!
//! ## Methodology (read before trusting a number from this file)
//!
//! - Deploys the **real compiled release WASM** (via `Deployer::deploy_v2`,
//!   the same technique `tests/constructor_auth.rs` uses) rather than the
//!   native "test contract" shortcut `Env::register` normally uses for
//!   other tests in this suite. This matters: per the SDK's own
//!   documentation on `CostEstimate::resources`, a native test-contract
//!   invocation *skips* WASM VM instantiation and execution costs
//!   entirely, which would make every number in this file meaningless
//!   for a WASM-deployed contract. Deploying the actual artifact avoids
//!   that gap.
//! - Needs the release WASM to exist first:
//!   `cargo build --target wasm32v1-none --release` (the `resource-profile`
//!   Makefile target depends on `build`, which does this).
//! - Uses `env.cost_estimate().resources()` /`.fee()` — the SDK's own
//!   supported instrumentation, not a hand-rolled estimate. Per that
//!   API's own documentation, these are **modelled** resource/fee
//!   figures ("take the return value with a grain of salt"), based on a
//!   fee-configuration snapshot the SDK dates 2026-07-10, not a live
//!   simulation against current mainnet/testnet state. Nothing in this
//!   file is a claimed mainnet benchmark — see
//!   `docs/security/resource-profile-v1.md` for how these numbers are
//!   meant to be read.
//! - All measurements are for a **single wallet's** growing history in
//!   isolation (no other wallets, no other contract state) — the
//!   worst-case-shaped scenario the storage design in `SECURITY.md` §7
//!   already flags, not an average-case simulation.

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, BytesN, Env, String};

// Mainnet invocation resource limits, mirrored from
// `soroban_sdk::testutils::cost_estimate::NetworkInvocationResourceLimits::mainnet()`
// (soroban-sdk 27.0.6, `src/testutils/cost_estimate.rs`). That type is
// not itself re-exported for external naming (it comes from the
// `soroban-env-host` crate, a transitive, not direct, dependency), so
// the three figures this file actually compares against are mirrored
// here verbatim rather than pulling in a new direct dependency just to
// name a type. The SDK's own doc comment on that method dates this
// snapshot to 2026-07-10; re-check it on every soroban-sdk bump the
// same way `docs/security/resource-profile-v1.md` says to.
const MAINNET_INSTRUCTION_LIMIT: i64 = 400_000_000;
const MAINNET_MEM_BYTES_LIMIT: i64 = 41_943_040;
const MAINNET_WRITE_BYTES_LIMIT: u32 = 132_096;

const WASM_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasm32v1-none/release/proofowl_contracts.wasm"
);

/// History sizes to sample. Chosen to span "typical MVP contributor"
/// (single digits) through "very active contributor" (low hundreds)
/// without spending unbounded CI time — this file is explicitly
/// excluded from the default test run for exactly that reason. Kept
/// safely below the hard per-entry-size ceiling found and reported by
/// `find_the_hard_history_size_ceiling` below (measured well under 300
/// for this contract's `Attestation` shape).
const SAMPLE_SIZES: [u32; 7] = [1, 5, 10, 25, 50, 100, 200];

fn hash(env: &Env, seed: u32) -> BytesN<32> {
    let mut bytes = [0u8; 32];
    bytes[0..4].copy_from_slice(&seed.to_be_bytes());
    BytesN::from_array(env, &bytes)
}

#[test]
#[ignore = "diagnostic/slow: run explicitly via `make resource-profile`"]
fn measure_attestation_history_growth() {
    let Ok(wasm) = std::fs::read(WASM_PATH) else {
        eprintln!(
            "SKIP measure_attestation_history_growth: build the contract first with \
             `cargo build --target wasm32v1-none --release` (or run `make resource-profile`, \
             which does this for you)."
        );
        return;
    };

    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let wasm_hash = env
        .deployer()
        .upload_contract_wasm(Bytes::from_slice(&env, &wasm));
    let deployer = Address::generate(&env);
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let salt = BytesN::from_array(&env, &[9u8; 32]);
    let contract_id = env
        .deployer()
        .with_address(deployer, salt)
        .deploy_v2(wasm_hash, (admin, attestor.clone()));
    let client = proofowl_contracts::ProofOwlRegistryClient::new(&env, &contract_id);

    let wallet = Address::generate(&env);
    let gh = hash(&env, 0xffff_ffff);
    client.link_github(&wallet, &attestor, &gh);
    let repo = String::from_str(&env, "stellar/soroban-examples");

    println!(
        "\n{:>6} | {:>14} | {:>10} | {:>10} | {:>14}",
        "count", "submit_instr", "mem_bytes", "write_bytes", "submit_fee(stroops)"
    );

    let mut last_submit_instr = 0i64;
    let mut current = 0u32;
    for &target in SAMPLE_SIZES.iter() {
        while current < target {
            let pr = hash(&env, current + 1);
            client.submit_attestation(&attestor, &gh, &repo, &(current + 1), &1u64, &100u32, &pr);
            current += 1;
        }
        let resources = env.cost_estimate().resources();
        let fee = env.cost_estimate().fee();
        println!(
            "{:>6} | {:>14} | {:>10} | {:>10} | {:>14}",
            current, resources.instructions, resources.mem_bytes, resources.write_bytes, fee.total
        );
        assert!(
            resources.instructions >= last_submit_instr,
            "submit_attestation cost must not decrease as history grows (count={current})"
        );
        last_submit_instr = resources.instructions;

        // get_attestations / get_reputation_score / bump_wallet_ttl at
        // this same history size.
        let _ = client.get_attestations(&wallet);
        let read_resources = env.cost_estimate().resources();
        let _ = client.get_reputation_score(&wallet);
        let score_resources = env.cost_estimate().resources();
        client.bump_wallet_ttl(&wallet);
        let bump_resources = env.cost_estimate().resources();
        println!(
            "{:>6} | get_attestations: {} instr, {} mem | get_reputation_score: {} instr | bump_wallet_ttl: {} instr",
            current,
            read_resources.instructions,
            read_resources.mem_bytes,
            score_resources.instructions,
            bump_resources.instructions
        );
    }

    println!(
        "\nmainnet invocation limits (SDK snapshot, see the constants at the top of this file): \
         instructions={MAINNET_INSTRUCTION_LIMIT}, mem_bytes={MAINNET_MEM_BYTES_LIMIT}, \
         write_bytes={MAINNET_WRITE_BYTES_LIMIT}"
    );
    // The very last call made above was bump_wallet_ttl at the largest
    // sampled history size -- labelled explicitly here since
    // `cost_estimate().resources()` always reflects only the single
    // most recent top-level invocation, not a cumulative total.
    let final_bump_resources = env.cost_estimate().resources();
    println!(
        "at count={current}, bump_wallet_ttl (the single most expensive operation measured): \
         instructions={} ({:.4}% of limit), mem_bytes={} ({:.4}% of limit), write_bytes={} ({:.4}% of limit)",
        final_bump_resources.instructions,
        100.0 * final_bump_resources.instructions as f64 / MAINNET_INSTRUCTION_LIMIT as f64,
        final_bump_resources.mem_bytes,
        100.0 * final_bump_resources.mem_bytes as f64 / MAINNET_MEM_BYTES_LIMIT as f64,
        final_bump_resources.write_bytes,
        100.0 * final_bump_resources.write_bytes as f64 / MAINNET_WRITE_BYTES_LIMIT as f64,
    );
}

/// Finds the exact history size at which a single wallet's
/// `Attestations(wallet)` entry stops fitting in one contract-data
/// entry at all — not "gets expensive", but **fails outright**: past
/// this point, `submit_attestation` (and, by the same mechanism,
/// `get_attestations` / `bump_wallet_ttl`, which load and rewrite the
/// same one entry) cannot succeed on a real network no matter how much
/// fee is paid, because the entry itself exceeds
/// `max_contract_data_entry_size_bytes` (65536 bytes on the SDK's
/// mainnet snapshot). This was discovered empirically while extending
/// `measure_attestation_history_growth`'s sample range past 200 — the
/// SDK's own resource-limit enforcement (on by default for a
/// WASM-deployed test invocation) panics with `Budget: ExceededLimit`
/// and names the exact oversized entry, which is exactly the signal
/// this test captures and reports instead of letting it abort the
/// process.
///
/// The panic hook is suppressed only around the expected failing call,
/// mirroring the same pattern `tests/constructor_auth.rs` uses for its
/// expected-panic negative case.
#[test]
#[ignore = "diagnostic/slow: run explicitly via `make resource-profile`"]
fn find_the_hard_history_size_ceiling() {
    let Ok(wasm) = std::fs::read(WASM_PATH) else {
        eprintln!(
            "SKIP find_the_hard_history_size_ceiling: build the contract first with \
             `cargo build --target wasm32v1-none --release` (or run `make resource-profile`)."
        );
        return;
    };

    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let wasm_hash = env
        .deployer()
        .upload_contract_wasm(Bytes::from_slice(&env, &wasm));
    let deployer = Address::generate(&env);
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let salt = BytesN::from_array(&env, &[10u8; 32]);
    let contract_id = env
        .deployer()
        .with_address(deployer, salt)
        .deploy_v2(wasm_hash, (admin, attestor.clone()));
    let client = proofowl_contracts::ProofOwlRegistryClient::new(&env, &contract_id);

    let wallet = Address::generate(&env);
    let gh = hash(&env, 0xeeee_eeee);
    client.link_github(&wallet, &attestor, &gh);
    // A realistic, fixed-length repo string: "<owner>/<repo>" at 25
    // ASCII bytes ("stellar/soroban-examples"). The exact ceiling is a
    // function of `Attestation`'s total encoded size, so it shifts if
    // this string's length changes -- that sensitivity is itself part
    // of the finding (see docs/security/resource-profile-v1.md).
    let repo = String::from_str(&env, "stellar/soroban-examples");

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(std::boxed::Box::new(|_| {}));

    let mut last_success = 0u32;
    let mut ceiling: Option<u32> = None;
    // 1000 is comfortably above where the ceiling was observed (low
    // 300s) while keeping worst-case runtime bounded.
    for n in 1..=1000u32 {
        let pr = hash(&env, n);
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            client.submit_attestation(&attestor, &gh, &repo, &n, &1u64, &100u32, &pr);
        }));
        match outcome {
            Ok(()) => last_success = n,
            Err(_) => {
                ceiling = Some(n);
                break;
            }
        }
    }
    std::panic::set_hook(prev_hook);

    let ceiling = ceiling.expect(
        "expected the per-entry size limit to be hit within 1000 attestations; if this \
         no longer happens the contract's storage layout likely changed favourably -- \
         update this test's range and docs/security/resource-profile-v1.md accordingly",
    );

    println!(
        "\nHARD CEILING: wallet history of {last_success} attestations succeeds; the \
         {ceiling}th submit_attestation call fails outright (the Attestations(wallet) \
         entry exceeds the max per-contract-data-entry size limit of 65536 bytes), \
         regardless of fee paid. See docs/security/resource-profile-v1.md."
    );

    // Loose bound: catches a catastrophic regression (e.g. the ceiling
    // suddenly dropping to single digits) without being brittle to a
    // small, expected shift from an encoding or SDK-limit change.
    assert!(
        (50..=1000).contains(&ceiling),
        "hard ceiling {ceiling} is outside the sane range -- re-derive \
         docs/security/resource-profile-v1.md's numbers"
    );
}
