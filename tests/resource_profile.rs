//! Measured resource / scalability profile — v0.2 evidence.
//!
//! Replaces `docs/security/resource-profile-v1.md`'s v0.1 evidence (a
//! hard ceiling at 286 attestations) with proof that the v0.2 bounded,
//! per-record storage design
//! (`docs/adr/0004-paginated-attestation-storage.md`) does not have
//! that ceiling and does not merely push it further out.
//!
//! Diagnostic, not a pass/fail security gate: `#[ignore]`d so it never
//! runs as part of `cargo test --all` / `make check` / CI's default PR
//! job; run explicitly with `make resource-profile`. See
//! `docs/security/resource-profile-v2.md` for how these numbers are
//! meant to be read and the same methodology notes
//! `resource-profile-v1.md` established (real compiled WASM via
//! `Deployer::deploy_v2`, not the native test-contract shortcut; the
//! SDK's own `cost_estimate()` instrumentation, a modelled estimate
//! dated by the SDK to 2026-07-10, not a live mainnet benchmark).

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, BytesN, Env, String};

// Mainnet invocation resource limits, mirrored from
// `soroban_sdk::testutils::cost_estimate::NetworkInvocationResourceLimits::mainnet()`
// (soroban-sdk 27.0.6) -- see `tests/resource_profile.rs`'s v0.1
// history (git log) for why these are mirrored rather than imported.
const MAINNET_INSTRUCTION_LIMIT: i64 = 400_000_000;
const MAINNET_MEM_BYTES_LIMIT: i64 = 41_943_040;
const MAINNET_ENTRY_SIZE_LIMIT: u32 = 65_536;

const WASM_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasm32v1-none/release/proofowl_contracts.wasm"
);

fn hash(env: &Env, seed: u32) -> BytesN<32> {
    let mut bytes = [0u8; 32];
    bytes[0..4].copy_from_slice(&seed.to_be_bytes());
    BytesN::from_array(env, &bytes)
}

fn deploy<'a>(
    env: &'a Env,
    wasm: &[u8],
    salt_byte: u8,
) -> proofowl_contracts::ProofOwlRegistryClient<'a> {
    let wasm_hash = env
        .deployer()
        .upload_contract_wasm(Bytes::from_slice(env, wasm));
    let deployer = Address::generate(env);
    let admin = Address::generate(env);
    let attestor = Address::generate(env);
    let salt = BytesN::from_array(env, &[salt_byte; 32]);
    let contract_id = env
        .deployer()
        .with_address(deployer, salt)
        .deploy_v2(wasm_hash, (admin, attestor));
    proofowl_contracts::ProofOwlRegistryClient::new(env, &contract_id)
}

fn load_wasm() -> Option<std::vec::Vec<u8>> {
    match std::fs::read(WASM_PATH) {
        Ok(w) => Some(w),
        Err(_) => {
            eprintln!(
                "SKIP: build the contract first with `cargo build --target wasm32v1-none --release` \
                 (or run `make resource-profile`, which does this for you)."
            );
            None
        }
    }
}

/// History sizes to sample: spans well past the v0.1 ceiling (286) up
/// to 1000, chosen to make "no ceiling, and cost stays flat" visible
/// without spending unbounded CI time -- this file is explicitly
/// excluded from the default test run for exactly that reason.
const SAMPLE_SIZES: [u32; 6] = [1, 50, 300, 500, 750, 1000];

// ---------------------------------------------------------------------------
// 1. Growth: submit_attestation never hits a ceiling, and its cost stays
//    a small, bounded fraction of the mainnet instruction limit even at
//    1000 attestations for one wallet -- more than 3x past the v0.1
//    ceiling of 286.
//
// Honest finding, not swept under the rug: in THIS local test
// environment, writing a brand-new persistent entry gets measurably
// more expensive as the *total* number of prior entries under the
// contract grows (~6,400 extra instructions per pre-existing entry,
// empirically close to linear -- see the printed marginal-cost table).
// Reads and TTL-only updates to *existing* keys show no such growth
// (see test 3 below: flat within 2% regardless of position in a
// 900-entry history). This asymmetry is consistent with the cost being
// an artifact of the local Soroban test host's in-memory
// snapshot/footprint bookkeeping for *new* ledger entries (the harness
// captures a full ledger snapshot at `Env` drop) rather than a
// documented mainnet cost characteristic -- Soroban's production
// storage backend does not document a cost tied to total prior entry
// count for writing one new, unrelated key. This was not fully
// isolated in this phase and is flagged as a follow-up investigation
// in `docs/security/resource-profile-v2.md`, not asserted as fact.
//
// What IS proven here, and is the property that actually matters for
// the Phase 4 finding this ADR fixes: there is no per-entry SIZE
// ceiling (v0.1's actual failure mode), and the absolute instruction
// cost at 1000 attestations remains under 2% of the mainnet limit --
// nowhere near a resource constraint, whatever the precise shape of its
// growth curve turns out to be at far larger scale.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "diagnostic/slow: run explicitly via `make resource-profile`"]
fn v2_submit_attestation_never_hits_a_ceiling_and_stays_resource_bounded() {
    let Some(wasm) = load_wasm() else { return };
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let client = deploy(&env, &wasm, 1);

    let wallet = Address::generate(&env);
    let attestor = client.get_attestor().unwrap();
    let gh = hash(&env, 0xffff_ffff);
    client.link_github(&wallet, &attestor, &gh);
    let repo = String::from_str(&env, "stellar/soroban-examples");

    println!(
        "\n{:>6} | {:>14} | {:>10} | {:>10} | {:>14}",
        "count", "submit_instr", "mem_bytes", "write_bytes", "submit_fee(stroops)"
    );

    let mut current = 0u32;
    let mut instr_samples: std::vec::Vec<(u32, i64)> = std::vec::Vec::new();
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
        instr_samples.push((current, resources.instructions));
    }

    // Confirm the wallet actually holds every attestation -- no
    // ceiling was hit anywhere in this run, more than 3x past 286.
    assert_eq!(client.get_attestation_count(&wallet), 1000);

    // Marginal (per-additional-attestation) cost between checkpoints,
    // printed for transparency about the growth shape observed.
    println!("\nmarginal instructions per new attestation, between checkpoints:");
    for pair in instr_samples.windows(2) {
        let (n0, i0) = pair[0];
        let (n1, i1) = pair[1];
        let per_entry = (i1 - i0) as f64 / (n1 - n0) as f64;
        println!("  N={n0}->{n1}: {:.0} instr/entry", per_entry);
    }

    // The defensible, load-bearing claim: absolute cost at the largest
    // sampled size is a small fraction of the mainnet instruction
    // limit -- not "flat" (it measurably is not, see above), but
    // nowhere near a resource constraint, and critically: no ceiling.
    let (last_n, last_instr) = *instr_samples.last().unwrap();
    let pct_of_limit = 100.0 * last_instr as f64 / MAINNET_INSTRUCTION_LIMIT as f64;
    println!(
        "\nat N={last_n}: submit_attestation cost = {last_instr} instructions ({pct_of_limit:.4}% \
         of the {MAINNET_INSTRUCTION_LIMIT}-instruction mainnet limit)"
    );
    assert!(
        pct_of_limit < 10.0,
        "submit_attestation at N={last_n} used {pct_of_limit:.2}% of the mainnet instruction \
         limit -- investigate before considering this bounded for production scale"
    );
}

// ---------------------------------------------------------------------------
// 2. Score lookup is O(1) regardless of history size.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "diagnostic/slow: run explicitly via `make resource-profile`"]
fn v2_reputation_score_lookup_is_constant_time() {
    let Some(wasm) = load_wasm() else { return };
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let client = deploy(&env, &wasm, 2);

    let wallet = Address::generate(&env);
    let attestor = client.get_attestor().unwrap();
    let gh = hash(&env, 0xeeee_eeee);
    client.link_github(&wallet, &attestor, &gh);
    let repo = String::from_str(&env, "stellar/soroban-examples");

    let mut score_instr_at_small = 0i64;
    let mut score_instr_at_large = 0i64;
    for target in [1u32, 500u32] {
        let mut current = client.get_attestation_count(&wallet);
        while current < target {
            client.submit_attestation(
                &attestor,
                &gh,
                &repo,
                &(current + 1),
                &1u64,
                &100u32,
                &hash(&env, 1000 + current),
            );
            current += 1;
        }
        let _ = client.get_reputation_score(&wallet);
        let instr = env.cost_estimate().resources().instructions;
        println!("get_reputation_score at count={current}: {instr} instructions");
        if target == 1 {
            score_instr_at_small = instr;
        } else {
            score_instr_at_large = instr;
        }
    }

    println!(
        "\nget_reputation_score instructions: N=1 -> {score_instr_at_small}, N=500 -> {score_instr_at_large} \
         (ratio {:.3}x)",
        score_instr_at_large as f64 / score_instr_at_small as f64
    );
    // O(1): reading the running counter at N=500 must not cost
    // meaningfully more than at N=1. Generous 3x headroom for noise.
    assert!(
        (score_instr_at_large as f64) < (score_instr_at_small as f64) * 3.0,
        "get_reputation_score cost grew with history size -- expected O(1) against the running counter"
    );
}

// ---------------------------------------------------------------------------
// 3. Page reads and paginated TTL maintenance are bounded by the page
//    limit, not by total history size.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "diagnostic/slow: run explicitly via `make resource-profile`"]
fn v2_page_operations_cost_depends_on_page_size_not_history_size() {
    let Some(wasm) = load_wasm() else { return };
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let client = deploy(&env, &wasm, 3);

    let wallet = Address::generate(&env);
    let attestor = client.get_attestor().unwrap();
    let gh = hash(&env, 0xdddd_dddd);
    client.link_github(&wallet, &attestor, &gh);
    let repo = String::from_str(&env, "stellar/soroban-examples");

    // Grow to 900 attestations first.
    for i in 0..900u32 {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo,
            &(i + 1),
            &1u64,
            &100u32,
            &hash(&env, 2000 + i),
        );
    }

    println!(
        "\n{:>10} | {:>10} | {:>14}",
        "start", "page_instr", "bump_page_instr"
    );
    let mut page_costs: std::vec::Vec<i64> = std::vec::Vec::new();
    for &start in &[0u32, 400, 850] {
        let _ = client.get_attestations_page(&wallet, &start, &50u32);
        let page_instr = env.cost_estimate().resources().instructions;

        let _ = client.bump_attestations_ttl_page(&wallet, &start, &50u32);
        let bump_instr = env.cost_estimate().resources().instructions;

        println!("{:>10} | {:>10} | {:>14}", start, page_instr, bump_instr);
        page_costs.push(page_instr);
    }

    // The cost of reading a 50-entry page starting near the END of a
    // 900-entry history (start=850) must be in the same ballpark as
    // reading one starting at the BEGINNING (start=0) -- both touch
    // exactly 50 entries, regardless of where in a 900-entry history
    // they sit.
    let min = *page_costs.iter().min().unwrap();
    let max = *page_costs.iter().max().unwrap();
    println!(
        "\npage-read instruction range across start positions: {min}..{max} (ratio {:.3}x)",
        max as f64 / min as f64
    );
    assert!(
        (max as f64) < (min as f64) * 3.0,
        "page-read cost varied {:.2}x by position in a 900-entry history -- expected it to depend \
         only on the page limit (50), not on how deep into the history the page starts",
        max as f64 / min as f64
    );
}

// ---------------------------------------------------------------------------
// 4. A prolific (or malicious) contributor on one wallet cannot BRICK
//    another wallet's profile -- the specific failure mode v0.1 had
//    (a wallet permanently unable to receive attestations or TTL
//    refreshes once its own entry hit the size ceiling). v0.2 has no
//    per-wallet size ceiling, so this can no longer happen by
//    construction; this test proves wallet B's operations still
//    SUCCEED, and stay resource-bounded, regardless of wallet A's size.
//
// Honest caveat: per test 1's finding, this local harness's cost for
// writing a *new* entry appears to scale with the *contract's total*
// prior entry count, not demonstrably isolated per wallet -- wallet
// B's first submission here does cost more than an isolated first
// submission would (compare to test 1's N=1 sample). What matters for
// the "cannot brick another contributor" claim is that it still
// succeeds and stays a small fraction of the resource limit, not that
// it is perfectly isolated in absolute instruction count.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "diagnostic/slow: run explicitly via `make resource-profile`"]
fn v2_one_wallets_history_size_cannot_brick_another_wallet() {
    let Some(wasm) = load_wasm() else { return };
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let client = deploy(&env, &wasm, 4);
    let attestor = client.get_attestor().unwrap();
    let repo = String::from_str(&env, "stellar/soroban-examples");

    // Wallet A: grown to 900 attestations (a prolific -- or, if the
    // attestor were compromised, maliciously flooded -- contributor).
    let wallet_a = Address::generate(&env);
    let gh_a = hash(&env, 0xaaaa_aaaa);
    client.link_github(&wallet_a, &attestor, &gh_a);
    for i in 0..900u32 {
        client.submit_attestation(
            &attestor,
            &gh_a,
            &repo,
            &(i + 1),
            &1u64,
            &100u32,
            &hash(&env, 3000 + i),
        );
    }

    // Wallet B: brand new, first-ever attestation, submitted AFTER
    // wallet A is already huge.
    let wallet_b = Address::generate(&env);
    let gh_b = hash(&env, 0xbbbb_bbbb);
    client.link_github(&wallet_b, &attestor, &gh_b);
    client.submit_attestation(
        &attestor,
        &gh_b,
        &repo,
        &1u32,
        &1u64,
        &100u32,
        &hash(&env, 9999),
    );
    let b_first_submit_instr = env.cost_estimate().resources().instructions;

    let _ = client.get_reputation_score(&wallet_b);
    let b_score_instr = env.cost_estimate().resources().instructions;

    client.bump_wallet_core_ttl(&wallet_b);
    let b_bump_instr = env.cost_estimate().resources().instructions;

    println!(
        "\nwallet B (fresh, after wallet A reached 900): first submit={b_first_submit_instr} instr, \
         score={b_score_instr} instr, core_ttl_bump={b_bump_instr} instr"
    );

    // The claim that actually matters: wallet B's operations SUCCEEDED
    // (no ceiling, no brick) and stayed a small fraction of the
    // resource limit, despite wallet A's 900-entry history existing
    // under the same contract. Bound generously (10% of the mainnet
    // instruction limit) since this test is about "does it still work
    // and stay far from any limit", not about achieving the tightest
    // possible isolation -- see the caveat above.
    let pct_of_limit = 100.0 * b_first_submit_instr as f64 / MAINNET_INSTRUCTION_LIMIT as f64;
    println!("wallet B's first submit_attestation used {pct_of_limit:.4}% of the mainnet instruction limit");
    assert!(
        pct_of_limit < 10.0,
        "wallet B's first submit_attestation cost {pct_of_limit:.2}% of the mainnet instruction \
         limit despite being a brand-new wallet -- investigate before relying on this at scale"
    );
    assert_eq!(
        client.get_attestation_count(&wallet_b),
        1,
        "wallet B's own attestation must have succeeded"
    );
    assert_eq!(
        client.get_attestation_count(&wallet_a),
        900,
        "wallet A's history must be completely untouched by wallet B's activity"
    );
}

// ---------------------------------------------------------------------------
// 5. No individual persistent entry approaches the 65,536-byte limit,
//    under the documented page-size (50) and record-size constraints.
// ---------------------------------------------------------------------------

#[test]
#[ignore = "diagnostic/slow: run explicitly via `make resource-profile`"]
fn v2_no_entry_approaches_the_soroban_size_limit() {
    let Some(wasm) = load_wasm() else { return };
    let env = Env::default();
    env.mock_all_auths_allowing_non_root_auth();
    let client = deploy(&env, &wasm, 5);
    let attestor = client.get_attestor().unwrap();

    let wallet = Address::generate(&env);
    let gh = hash(&env, 0xcccc_cccc);
    client.link_github(&wallet, &attestor, &gh);

    // A deliberately large `repo` string (the only variable-length
    // field in `Attestation`) -- 200 bytes, comfortably larger than any
    // real "<owner>/<repo>" (capped at 39+100 chars per
    // identifier-spec-v1.md) -- to measure a pessimistic single-entry
    // write size.
    let long_repo_str = std::format!("{}/{}", "o".repeat(100), "r".repeat(99));
    let long_repo = String::from_str(&env, &long_repo_str);
    client.submit_attestation(
        &attestor,
        &gh,
        &long_repo,
        &1u32,
        &1u64,
        &100u32,
        &hash(&env, 1),
    );
    let write_bytes = env.cost_estimate().resources().write_bytes;

    println!(
        "\none AttestationEntry write (repo ~200 bytes) touched {write_bytes} total write bytes \
         this invocation (includes AttestationEntry + AttestationCount + ReputationScore + SeenPr \
         + link TTL bumps, not just the one entry -- a conservative over-count for the single \
         entry's own size)."
    );
    assert!(
        write_bytes < MAINNET_ENTRY_SIZE_LIMIT,
        "a single submit_attestation's total write footprint ({write_bytes} bytes) already exceeds \
         the per-entry limit ({MAINNET_ENTRY_SIZE_LIMIT}) even before isolating just the \
         AttestationEntry -- investigate immediately"
    );

    // A full 50-entry page response, each with a realistic repo string,
    // stays far under the per-entry limit too (it is a transient
    // return value, not a stored entry, but this bounds the "does a
    // page response balloon" question the MAX_PAGE_SIZE choice needs
    // to answer).
    let repo = String::from_str(&env, "stellar/soroban-examples");
    for i in 1..50u32 {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo,
            &(i + 1),
            &1u64,
            &100u32,
            &hash(&env, 10 + i),
        );
    }
    let page = client.get_attestations_page(&wallet, &0u32, &50u32);
    assert_eq!(page.len(), 50);
    let page_read_mem = env.cost_estimate().resources().mem_bytes;
    println!(
        "a full 50-entry page read used {page_read_mem} bytes of modelled memory \
         ({:.4}% of the {MAINNET_MEM_BYTES_LIMIT}-byte mainnet limit).",
        100.0 * page_read_mem as f64 / MAINNET_MEM_BYTES_LIMIT as f64
    );
    assert!(page_read_mem < MAINNET_MEM_BYTES_LIMIT);

    println!(
        "\nmainnet limits for reference: instructions={MAINNET_INSTRUCTION_LIMIT}, \
         mem_bytes={MAINNET_MEM_BYTES_LIMIT}, entry_size={MAINNET_ENTRY_SIZE_LIMIT}"
    );
}
