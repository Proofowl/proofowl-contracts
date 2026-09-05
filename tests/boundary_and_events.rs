//! Boundary, negative, and event-emission coverage.
//!
//! Covers: empty/long/malformed `repo` strings; zero and extreme values
//! for `pr_number`, `issue_id`, `complexity`, and ledger `timestamp`;
//! and that every successful mutating call emits exactly the documented
//! event with the documented fields, while every rejected call emits
//! none.
//!
//! Every custom error code (`Error::AlreadyInitialized` through
//! `Error::LinkNotFound`) is exercised somewhere in this test suite:
//! `AlreadyInitialized` is unreachable by construction (documented,
//! not tested — there is no code path that can produce it);
//! `NotInitialized` in `tests/security_matrix.rs`
//! (`not_initialized_is_reachable_and_rejects_every_gated_call`);
//! `Unauthorized`, `WalletAlreadyLinked`, `GithubAlreadyLinked`,
//! `WalletNotLinked`, `LinkNotFound` across `tests/security_matrix.rs`
//! and `src/test.rs`; `DuplicateAttestation` and `InvalidComplexity`
//! in both those files and again here, in the context of boundary
//! values specifically.
//!
//! ## Reasoned bounds, not resource exhaustion
//!
//! The "very long" `repo` string used below is ~800 ASCII bytes —
//! comfortably larger than any real GitHub `owner/repo` string (GitHub
//! logins and repo names are capped at 39 / 100 characters respectively
//! per `identifier-spec-v1.md`), enough to prove the contract does not
//! special-case string length, without attempting to approach the
//! Soroban host's own per-entry size ceiling (`docs/security/resource-profile-v1.md`
//! measures that ceiling properly; this file is about boundary
//! *correctness*, not about finding the resource limit).

use proofowl_contracts::{
    AttestationRecorded, AttestorRotated, Error, GithubLinked, GithubUnlinked, Initialized,
    ProofOwlRegistry, ProofOwlRegistryClient,
};
use soroban_sdk::testutils::{Address as _, Events as _, Ledger as _};
use soroban_sdk::{Address, BytesN, Env, Event as _, String};

const TS: u64 = 1_700_000_000;

fn setup() -> (Env, ProofOwlRegistryClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(TS);
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let contract_id = env.register(ProofOwlRegistry, (admin.clone(), attestor.clone()));
    let client = ProofOwlRegistryClient::new(&env, &contract_id);
    (env, client, admin, attestor)
}

fn hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

// ---------------------------------------------------------------------------
// 1. `repo` string boundaries
// ---------------------------------------------------------------------------

#[test]
fn repo_string_boundaries_are_accepted_and_returned_verbatim() {
    let (env, client, _admin, attestor) = setup();

    let long_repo: std::string::String = "o".repeat(390) + "/" + &"r".repeat(400); // ~791 bytes
    let cases: [(&str, std::string::String); 4] = [
        ("empty", std::string::String::new()),
        ("realistic", "stellar/soroban-examples".to_string()),
        ("unusually long (~791 bytes)", long_repo),
        (
            "control characters and non-ASCII",
            "weird/repo\nname\twith\u{0}nulls-and-\u{00e9}accents".to_string(),
        ),
    ];

    for (i, (label, repo_str)) in cases.into_iter().enumerate() {
        let wallet = Address::generate(&env);
        let gh_hash = hash(&env, 10 + i as u8 * 2);
        client.link_github(&wallet, &attestor, &gh_hash);

        let repo = String::from_str(&env, &repo_str);
        let pr = hash(&env, 11 + i as u8 * 2);
        client.submit_attestation(&attestor, &gh_hash, &repo, &1u32, &1u64, &100u32, &pr);

        assert_eq!(client.get_attestation_count(&wallet), 1, "case {label:?}");
        assert_eq!(
            client.get_attestation(&wallet, &0u32).repo,
            repo,
            "case {label:?}: stored repo must match exactly"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. `pr_number` / `issue_id` extremes
// ---------------------------------------------------------------------------

#[test]
fn pr_number_and_issue_id_extremes_are_stored_exactly() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 1);
    client.link_github(&wallet, &attestor, &gh);

    let repo = String::from_str(&env, "stellar/soroban-examples");
    let cases: [(u32, u64); 4] = [(0, 0), (0, u64::MAX), (u32::MAX, 0), (u32::MAX, u64::MAX)];
    for (i, (pr_number, issue_id)) in cases.iter().enumerate() {
        let pr = hash(&env, 10 + i as u8);
        client.submit_attestation(&attestor, &gh, &repo, pr_number, issue_id, &100u32, &pr);
    }

    assert_eq!(client.get_attestation_count(&wallet), 4);
    for (i, (pr_number, issue_id)) in cases.iter().enumerate() {
        let a = client.get_attestation(&wallet, &(i as u32));
        assert_eq!(a.pr_number, *pr_number, "case {i}");
        assert_eq!(a.issue_id, *issue_id, "case {i}");
    }
}

// ---------------------------------------------------------------------------
// 3. Complexity boundary sweep -- exact accept/reject pattern around
//    every allowed tier.
// ---------------------------------------------------------------------------

#[test]
fn complexity_boundary_sweep_matches_the_exact_accept_reject_pattern() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 2);
    client.link_github(&wallet, &attestor, &gh);
    let repo = String::from_str(&env, "stellar/soroban-examples");

    let cases: [(u32, bool); 12] = [
        (0, true),
        (1, false),
        (99, false),
        (100, true),
        (101, false),
        (149, false),
        (150, true),
        (151, false),
        (199, false),
        (200, true),
        (201, false),
        (u32::MAX, false),
    ];

    let mut accepted = 0u32;
    for (i, (complexity, expect_ok)) in cases.iter().enumerate() {
        let pr = hash(&env, 20 + i as u8);
        let result = client.try_submit_attestation(
            &attestor,
            &gh,
            &repo,
            &(i as u32),
            &1u64,
            complexity,
            &pr,
        );
        assert_eq!(
            result.is_ok(),
            *expect_ok,
            "complexity {complexity} expected ok={expect_ok}, got {result:?}"
        );
        if *expect_ok {
            accepted += 1;
        } else {
            assert_eq!(result, Err(Ok(Error::InvalidComplexity)));
        }
    }
    assert_eq!(client.get_attestation_count(&wallet), accepted);
}

// ---------------------------------------------------------------------------
// 4. Ledger timestamp extremes are reflected verbatim in the recorded
//    attestation.
// ---------------------------------------------------------------------------

#[test]
fn attestation_timestamp_reflects_ledger_time_at_extremes() {
    for ts in [0u64, u64::MAX] {
        let env = Env::default();
        env.mock_all_auths();
        env.ledger().set_timestamp(ts);
        let admin = Address::generate(&env);
        let attestor = Address::generate(&env);
        let contract_id = env.register(ProofOwlRegistry, (admin, attestor.clone()));
        let client = ProofOwlRegistryClient::new(&env, &contract_id);

        let wallet = Address::generate(&env);
        let gh = hash(&env, 3);
        client.link_github(&wallet, &attestor, &gh);
        let repo = String::from_str(&env, "stellar/soroban-examples");
        client.submit_attestation(
            &attestor,
            &gh,
            &repo,
            &1u32,
            &1u64,
            &100u32,
            &hash(&env, 30),
        );

        assert_eq!(
            client.get_attestation(&wallet, &0u32).timestamp,
            ts,
            "ledger timestamp {ts}"
        );
    }
}

// ---------------------------------------------------------------------------
// 5. Every successful mutating call emits exactly its documented event.
//
// `env.events().all()` (per its documented contract) returns only the
// events published by the *last* contract invocation, and none at all
// if that invocation failed -- so no manual before/after diffing is
// needed here.
// ---------------------------------------------------------------------------

#[test]
fn constructor_emits_initialized_exactly_once() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let contract_id = env.register(ProofOwlRegistry, (admin.clone(), attestor.clone()));

    let events = env.events().all().filter_by_contract(&contract_id);
    assert_eq!(events.events().len(), 1);
    let expected = Initialized { admin, attestor }.to_xdr(&env, &contract_id);
    assert_eq!(events.events()[0], expected);
}

#[test]
fn link_github_emits_exactly_one_github_linked_event() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 4);

    client.link_github(&wallet, &attestor, &gh);

    let events = env.events().all().filter_by_contract(&client.address);
    assert_eq!(events.events().len(), 1);
    let expected = GithubLinked {
        wallet,
        github_id_hash: gh,
    }
    .to_xdr(&env, &client.address);
    assert_eq!(events.events()[0], expected);
}

#[test]
fn unlink_github_emits_exactly_one_github_unlinked_event() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 5);
    client.link_github(&wallet, &attestor, &gh);

    client.unlink_github(&wallet, &attestor, &gh);

    let events = env.events().all().filter_by_contract(&client.address);
    assert_eq!(events.events().len(), 1);
    let expected = GithubUnlinked {
        wallet,
        github_id_hash: gh,
    }
    .to_xdr(&env, &client.address);
    assert_eq!(events.events()[0], expected);
}

#[test]
fn submit_attestation_emits_exactly_one_attestation_recorded_event() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 6);
    client.link_github(&wallet, &attestor, &gh);
    let repo = String::from_str(&env, "stellar/soroban-examples");
    let pr = hash(&env, 60);

    client.submit_attestation(&attestor, &gh, &repo, &7u32, &8u64, &150u32, &pr);

    let events = env.events().all().filter_by_contract(&client.address);
    assert_eq!(events.events().len(), 1);
    let expected = AttestationRecorded {
        wallet,
        repo,
        pr_number: 7,
        issue_id: 8,
        complexity: 150,
        pr_hash: pr,
        timestamp: TS,
        sequence: 0, // v0.2: zero-based, first attestation for this wallet
    }
    .to_xdr(&env, &client.address);
    assert_eq!(events.events()[0], expected);
}

#[test]
fn attestation_recorded_sequence_field_matches_the_wallets_growing_history() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 61);
    client.link_github(&wallet, &attestor, &gh);
    let repo = String::from_str(&env, "stellar/soroban-examples");

    // A second attestation for the same wallet must carry sequence 1,
    // not 0 -- the event's new v0.2 field tracks the same zero-based
    // index `get_attestation` addresses it by.
    client.submit_attestation(
        &attestor,
        &gh,
        &repo,
        &1u32,
        &1u64,
        &100u32,
        &hash(&env, 62),
    );
    client.submit_attestation(
        &attestor,
        &gh,
        &repo,
        &2u32,
        &2u64,
        &100u32,
        &hash(&env, 63),
    );

    let events = env.events().all().filter_by_contract(&client.address);
    assert_eq!(
        events.events().len(),
        1,
        "only the last call's event is visible here"
    );
    let expected = AttestationRecorded {
        wallet: wallet.clone(),
        repo,
        pr_number: 2,
        issue_id: 2,
        complexity: 100,
        pr_hash: hash(&env, 63),
        timestamp: TS,
        sequence: 1,
    }
    .to_xdr(&env, &client.address);
    assert_eq!(events.events()[0], expected);
    assert_eq!(client.get_attestation_count(&wallet), 2);
}

#[test]
fn set_attestor_emits_exactly_one_attestor_rotated_event() {
    let (env, client, admin, _attestor) = setup();
    let new_attestor = Address::generate(&env);

    client.set_attestor(&admin, &new_attestor);

    let events = env.events().all().filter_by_contract(&client.address);
    assert_eq!(events.events().len(), 1);
    let expected = AttestorRotated {
        admin,
        new_attestor,
    }
    .to_xdr(&env, &client.address);
    assert_eq!(events.events()[0], expected);
}

// ---------------------------------------------------------------------------
// 6. Every rejected call emits no events at all.
// ---------------------------------------------------------------------------

#[test]
fn rejected_calls_emit_no_events() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 7);
    client.link_github(&wallet, &attestor, &gh);
    let repo = String::from_str(&env, "stellar/soroban-examples");

    // Identity squat attempt.
    let squatter = Address::generate(&env);
    let _ = client.try_link_github(&squatter, &attestor, &gh);
    assert!(env
        .events()
        .all()
        .filter_by_contract(&client.address)
        .events()
        .is_empty());

    // Invalid complexity.
    let _ =
        client.try_submit_attestation(&attestor, &gh, &repo, &1u32, &1u64, &42u32, &hash(&env, 70));
    assert!(env
        .events()
        .all()
        .filter_by_contract(&client.address)
        .events()
        .is_empty());

    // Duplicate PR.
    let pr = hash(&env, 71);
    client.submit_attestation(&attestor, &gh, &repo, &1u32, &1u64, &100u32, &pr);
    let _ = client.try_submit_attestation(&attestor, &gh, &repo, &1u32, &1u64, &100u32, &pr);
    assert!(env
        .events()
        .all()
        .filter_by_contract(&client.address)
        .events()
        .is_empty());

    // Wrong admin rotating the attestor.
    let impostor = Address::generate(&env);
    let _ = client.try_set_attestor(&impostor, &Address::generate(&env));
    assert!(env
        .events()
        .all()
        .filter_by_contract(&client.address)
        .events()
        .is_empty());

    // Mismatched unlink.
    let other_wallet = Address::generate(&env);
    let _ = client.try_unlink_github(&other_wallet, &attestor, &gh);
    assert!(env
        .events()
        .all()
        .filter_by_contract(&client.address)
        .events()
        .is_empty());
}
