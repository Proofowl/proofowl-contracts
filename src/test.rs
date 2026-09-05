#![cfg(test)]

use super::*;
use soroban_sdk::testutils::storage::{Instance as _, Persistent as _};
use soroban_sdk::testutils::{Address as _, Ledger as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Env, IntoVal};

const TS: u64 = 1_700_000_000;

/// Fully-authorized environment: `mock_all_auths` on, contract deployed
/// (constructor runs at `register`), ledger clock set. Most tests want
/// this.
fn setup() -> (Env, ProofOwlRegistryClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(TS);
    env.ledger().set_sequence_number(100);

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let contract_id = env.register(ProofOwlRegistry, (admin.clone(), attestor.clone()));
    let client = ProofOwlRegistryClient::new(&env, &contract_id);

    (env, client, admin, attestor)
}

/// Like `setup`, but with auth *enforced* after deployment so a test can
/// install a precise `mock_auths` list. `Env::register` always runs the
/// constructor with auth mocked (documented SDK behaviour), so the
/// contract is deployed and configured regardless; only calls made
/// afterwards are subject to the enforced auth.
fn setup_enforced_auth() -> (Env, ProofOwlRegistryClient<'static>, Address, Address) {
    let env = Env::default();
    env.ledger().set_timestamp(TS);
    env.ledger().set_sequence_number(100);

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let contract_id = env.register(ProofOwlRegistry, (admin.clone(), attestor.clone()));
    let client = ProofOwlRegistryClient::new(&env, &contract_id);

    (env, client, admin, attestor)
}

fn hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn repo(env: &Env) -> String {
    String::from_str(env, "stellar/soroban-examples")
}

// ---------------------------------------------------------------------------
// 1. Initialization (deploy-time constructor, no `init` entrypoint)
//
// The on-chain guarantee that a non-deployer cannot capture
// initialization is exercised end-to-end, against the real wasm and the
// real deployer auth path, in `tests/constructor_auth.rs`. `Env::register`
// force-mocks constructor auth, so it cannot reject here; what it *can*
// show is that the constructor bound the config at deploy time and that
// it demanded the admin's authorization.
// ---------------------------------------------------------------------------

#[test]
fn constructor_binds_config_at_deploy() {
    let (_env, client, admin, attestor) = setup();
    // No init call was made; config is already present from `register`.
    assert_eq!(client.get_admin(), Some(admin));
    assert_eq!(client.get_attestor(), Some(attestor));
}

#[test]
fn constructor_requires_admin_authorization() {
    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);

    let _id = env.register(ProofOwlRegistry, (admin.clone(), attestor.clone()));

    // The constructor's `admin.require_auth()` is recorded during
    // registration. On-chain this means the deploy transaction must be
    // signed by `admin` — a party who is not `admin` (and cannot obtain
    // that signature) cannot run this setup at all.
    let auths = env.auths();
    assert!(
        auths.iter().any(|(addr, _)| addr == &admin),
        "constructor must require admin authorization; auths = {auths:?}"
    );
}

#[test]
fn there_is_no_reinitialization_entrypoint() {
    // Compile-time guarantee, asserted for the reader: the generated
    // client exposes no `init` / `try_init`. The only setup path is the
    // constructor, which the host runs exactly once at deploy.
    let (_env, client, admin, attestor) = setup();
    assert_eq!(client.get_admin(), Some(admin));
    assert_eq!(client.get_attestor(), Some(attestor));
}

// ---------------------------------------------------------------------------
// 2. Two-party GitHub link
// ---------------------------------------------------------------------------

#[test]
fn link_github_sets_both_directions() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 1);

    client.link_github(&wallet, &attestor, &gh);

    assert_eq!(client.get_wallet_for_github(&gh), Some(wallet.clone()));
    assert_eq!(client.get_github_for_wallet(&wallet), Some(gh));
}

#[test]
fn link_github_requires_wallet_authorization() {
    let (env, client, _admin, attestor) = setup_enforced_auth();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 1);

    // Only the attestor signs; the wallet signature is missing.
    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name: "link_github",
        args: (wallet.clone(), attestor.clone(), gh.clone()).into_val(&env),
        sub_invokes: &[],
    };
    env.mock_auths(&[MockAuth {
        address: &attestor,
        invoke: &invoke,
    }]);

    assert!(client.try_link_github(&wallet, &attestor, &gh).is_err());
    assert_eq!(client.get_wallet_for_github(&gh), None);
}

#[test]
fn link_github_requires_attestor_authorization() {
    let (env, client, _admin, attestor) = setup_enforced_auth();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 1);

    // Only the wallet signs; the attestor co-signature is missing.
    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name: "link_github",
        args: (wallet.clone(), attestor.clone(), gh.clone()).into_val(&env),
        sub_invokes: &[],
    };
    env.mock_auths(&[MockAuth {
        address: &wallet,
        invoke: &invoke,
    }]);

    assert!(client.try_link_github(&wallet, &attestor, &gh).is_err());
    assert_eq!(client.get_wallet_for_github(&gh), None);
}

#[test]
fn link_github_rejects_wrong_attestor_even_when_signed() {
    let (env, client, ..) = setup();
    let wallet = Address::generate(&env);
    let impostor = Address::generate(&env);
    let gh = hash(&env, 1);

    // mock_all_auths means the impostor's signature is present; the
    // stored-attestor check must still reject it.
    assert_eq!(
        client.try_link_github(&wallet, &impostor, &gh),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn link_github_one_wallet_cannot_hold_two_identities() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);

    client.link_github(&wallet, &attestor, &hash(&env, 1));
    assert_eq!(
        client.try_link_github(&wallet, &attestor, &hash(&env, 2)),
        Err(Ok(Error::WalletAlreadyLinked))
    );
}

#[test]
fn link_github_identity_squat_is_blocked() {
    let (env, client, _admin, attestor) = setup();
    let real_owner = Address::generate(&env);
    let squatter = Address::generate(&env);
    let famous_identity = hash(&env, 42);

    client.link_github(&real_owner, &attestor, &famous_identity);

    // Someone else trying to claim the same GitHub identity hash is
    // refused even with a valid attestor co-signature present.
    assert_eq!(
        client.try_link_github(&squatter, &attestor, &famous_identity),
        Err(Ok(Error::GithubAlreadyLinked))
    );
    assert_eq!(
        client.get_wallet_for_github(&famous_identity),
        Some(real_owner)
    );
}

// ---------------------------------------------------------------------------
// 3. Recovery / unlink
// ---------------------------------------------------------------------------

#[test]
fn unlink_github_clears_the_link_and_allows_relink() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 7);

    client.link_github(&wallet, &attestor, &gh);
    client.unlink_github(&wallet, &attestor, &gh);

    assert_eq!(client.get_wallet_for_github(&gh), None);
    assert_eq!(client.get_github_for_wallet(&wallet), None);

    // The identity can now be linked to a fresh wallet.
    let new_wallet = Address::generate(&env);
    client.link_github(&new_wallet, &attestor, &gh);
    assert_eq!(client.get_wallet_for_github(&gh), Some(new_wallet));
}

#[test]
fn unlink_requires_wallet_authorization() {
    let (env, client, _admin, attestor) = setup_enforced_auth();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 7);
    mock_pair(&env, &client, "link_github", &wallet, &attestor, &gh);
    client.link_github(&wallet, &attestor, &gh);

    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name: "unlink_github",
        args: (wallet.clone(), attestor.clone(), gh.clone()).into_val(&env),
        sub_invokes: &[],
    };
    env.mock_auths(&[MockAuth {
        address: &attestor,
        invoke: &invoke,
    }]);
    assert!(client.try_unlink_github(&wallet, &attestor, &gh).is_err());
    assert_eq!(client.get_wallet_for_github(&gh), Some(wallet));
}

#[test]
fn unlink_requires_attestor_authorization() {
    let (env, client, _admin, attestor) = setup_enforced_auth();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 7);
    mock_pair(&env, &client, "link_github", &wallet, &attestor, &gh);
    client.link_github(&wallet, &attestor, &gh);

    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name: "unlink_github",
        args: (wallet.clone(), attestor.clone(), gh.clone()).into_val(&env),
        sub_invokes: &[],
    };
    env.mock_auths(&[MockAuth {
        address: &wallet,
        invoke: &invoke,
    }]);
    assert!(client.try_unlink_github(&wallet, &attestor, &gh).is_err());
    assert_eq!(client.get_wallet_for_github(&gh), Some(wallet));
}

#[test]
fn unlink_rejects_wrong_attestor() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 7);
    client.link_github(&wallet, &attestor, &gh);

    let impostor = Address::generate(&env);
    assert_eq!(
        client.try_unlink_github(&wallet, &impostor, &gh),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn unlink_unknown_link_fails() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    assert_eq!(
        client.try_unlink_github(&wallet, &attestor, &hash(&env, 99)),
        Err(Ok(Error::LinkNotFound))
    );
}

#[test]
fn unlink_preserves_history_and_global_pr_dedup() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 7);
    client.link_github(&wallet, &attestor, &gh);

    let spent_pr = hash(&env, 200);
    client.submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &10u32,
        &1u64,
        &150u32,
        &spent_pr,
    );

    client.unlink_github(&wallet, &attestor, &gh);

    // Reputation earned stays with the wallet that earned it.
    assert_eq!(client.get_attestation_count(&wallet), 1);
    assert_eq!(client.get_reputation_score(&wallet), 150);

    // Re-link the identity to a new wallet.
    let new_wallet = Address::generate(&env);
    client.link_github(&new_wallet, &attestor, &gh);

    // The already-spent PR cannot be re-attested onto the new wallet.
    assert_eq!(
        client.try_submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &10u32,
            &1u64,
            &150u32,
            &spent_pr
        ),
        Err(Ok(Error::DuplicateAttestation))
    );
    // A different PR still works.
    client.submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &11u32,
        &2u64,
        &150u32,
        &hash(&env, 201),
    );
    assert_eq!(client.get_reputation_score(&new_wallet), 150);
    assert_eq!(client.get_attestation_count(&wallet), 1);
}

// ---------------------------------------------------------------------------
// 4. Attestations
// ---------------------------------------------------------------------------

#[test]
fn submit_attestation_resolves_wallet_and_records_indexer_fields() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 4);
    client.link_github(&wallet, &attestor, &gh);

    let pr = hash(&env, 40);
    let returned =
        client.submit_attestation(&attestor, &gh, &repo(&env), &123u32, &101u64, &150u32, &pr);
    assert_eq!(returned, wallet);

    assert_eq!(client.get_attestation_count(&wallet), 1);
    let a = client.get_attestation(&wallet, &0);
    assert_eq!(
        a,
        Attestation {
            repo: repo(&env),
            pr_number: 123,
            issue_id: 101,
            complexity: 150,
            pr_hash: pr,
            timestamp: TS, // ledger time, not caller-supplied
        }
    );
    assert_eq!(client.get_reputation_score(&wallet), 150);
}

#[test]
fn submit_attestation_rejects_invalid_complexity() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 5);
    client.link_github(&wallet, &attestor, &gh);

    for bad in [1u32, 50, 99, 101, 175, 250, 999_999] {
        assert_eq!(
            client.try_submit_attestation(
                &attestor,
                &gh,
                &repo(&env),
                &1u32,
                &1u64,
                &bad,
                &hash(&env, bad as u8 ^ 0x5a)
            ),
            Err(Ok(Error::InvalidComplexity)),
            "complexity {bad} should be rejected"
        );
    }
    assert_eq!(client.get_attestation_count(&wallet), 0);
}

#[test]
fn submit_attestation_accepts_every_valid_complexity_tier() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 6);
    client.link_github(&wallet, &attestor, &gh);

    for (i, c) in [0u32, 100, 150, 200].into_iter().enumerate() {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &(i as u32),
            &(i as u64),
            &c,
            &hash(&env, 60 + i as u8),
        );
    }
    // 50 (unverified) + 100 + 150 + 200
    assert_eq!(client.get_reputation_score(&wallet), 500);
    assert_eq!(client.get_attestation_count(&wallet), 4);
}

#[test]
fn unverified_complexity_scores_at_base_rate() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 5);
    client.link_github(&wallet, &attestor, &gh);

    client.submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &1u32,
        &1u64,
        &0u32,
        &hash(&env, 50),
    );
    assert_eq!(
        client.get_reputation_score(&wallet),
        UNVERIFIED_COMPLEXITY_SCORE
    );
}

#[test]
fn duplicate_pr_hash_rejected_globally() {
    let (env, client, _admin, attestor) = setup();
    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);
    let gh_a = hash(&env, 10);
    let gh_b = hash(&env, 11);
    client.link_github(&wallet_a, &attestor, &gh_a);
    client.link_github(&wallet_b, &attestor, &gh_b);

    let pr = hash(&env, 60);
    client.submit_attestation(&attestor, &gh_a, &repo(&env), &1u32, &1u64, &100u32, &pr);

    // Same PR, different GitHub identity / wallet: still rejected.
    assert_eq!(
        client.try_submit_attestation(&attestor, &gh_b, &repo(&env), &1u32, &1u64, &100u32, &pr),
        Err(Ok(Error::DuplicateAttestation))
    );
}

#[test]
fn attesting_unlinked_github_id_fails() {
    let (env, client, _admin, attestor) = setup();
    let gh = hash(&env, 7);
    assert_eq!(
        client.try_submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &1u32,
            &1u64,
            &100u32,
            &hash(&env, 70)
        ),
        Err(Ok(Error::WalletNotLinked))
    );
}

#[test]
fn attestation_from_wrong_attestor_fails() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 8);
    client.link_github(&wallet, &attestor, &gh);

    let impostor = Address::generate(&env);
    assert_eq!(
        client.try_submit_attestation(
            &impostor,
            &gh,
            &repo(&env),
            &1u32,
            &1u64,
            &100u32,
            &hash(&env, 80)
        ),
        Err(Ok(Error::Unauthorized))
    );
}

#[test]
fn attestation_requires_attestor_authorization() {
    let (env, client, _admin, attestor) = setup_enforced_auth();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 8);
    mock_pair(&env, &client, "link_github", &wallet, &attestor, &gh);
    client.link_github(&wallet, &attestor, &gh);

    // No auth entries at all for the attestation call.
    env.mock_auths(&[]);
    let res = client.try_submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &1u32,
        &1u64,
        &100u32,
        &hash(&env, 81),
    );
    assert!(res.is_err());
}

// ---------------------------------------------------------------------------
// 5. Attestor rotation
// ---------------------------------------------------------------------------

#[test]
fn set_attestor_rotates_the_signing_key() {
    let (env, client, admin, old_attestor) = setup();
    let new_attestor = Address::generate(&env);
    let wallet = Address::generate(&env);
    let gh = hash(&env, 9);
    client.link_github(&wallet, &old_attestor, &gh);

    client.set_attestor(&admin, &new_attestor);
    assert_eq!(client.get_attestor(), Some(new_attestor.clone()));

    // Old key is now rejected...
    assert_eq!(
        client.try_submit_attestation(
            &old_attestor,
            &gh,
            &repo(&env),
            &1u32,
            &1u64,
            &100u32,
            &hash(&env, 91)
        ),
        Err(Ok(Error::Unauthorized))
    );
    // ...new key works.
    let w = client.submit_attestation(
        &new_attestor,
        &gh,
        &repo(&env),
        &1u32,
        &1u64,
        &100u32,
        &hash(&env, 92),
    );
    assert_eq!(w, wallet);
}

#[test]
fn set_attestor_requires_admin_authorization() {
    let (env, client, admin, _attestor) = setup_enforced_auth();
    let new_attestor = Address::generate(&env);

    // Someone other than admin signs.
    let stranger = Address::generate(&env);
    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name: "set_attestor",
        args: (admin.clone(), new_attestor.clone()).into_val(&env),
        sub_invokes: &[],
    };
    env.mock_auths(&[MockAuth {
        address: &stranger,
        invoke: &invoke,
    }]);
    assert!(client.try_set_attestor(&admin, &new_attestor).is_err());
}

#[test]
fn set_attestor_rejects_non_admin_address() {
    let (env, client, ..) = setup();
    let impostor = Address::generate(&env);
    let new_attestor = Address::generate(&env);
    assert_eq!(
        client.try_set_attestor(&impostor, &new_attestor),
        Err(Ok(Error::Unauthorized))
    );
}

// ---------------------------------------------------------------------------
// 6. Reputation scoring
// ---------------------------------------------------------------------------

#[test]
fn reputation_score_sums_multiple_attestations() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 12);
    client.link_github(&wallet, &attestor, &gh);

    client.submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &1u32,
        &1u64,
        &100u32,
        &hash(&env, 101),
    );
    client.submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &2u32,
        &2u64,
        &150u32,
        &hash(&env, 102),
    );
    client.submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &3u32,
        &3u64,
        &200u32,
        &hash(&env, 103),
    );

    assert_eq!(client.get_reputation_score(&wallet), 450);
    assert_eq!(client.get_attestation_count(&wallet), 3);
}

#[test]
fn reputation_score_is_zero_for_unknown_wallet() {
    let (env, client, ..) = setup();
    let nobody = Address::generate(&env);
    assert_eq!(client.get_reputation_score(&nobody), 0);
    assert_eq!(client.get_attestation_count(&nobody), 0);
}

// ---------------------------------------------------------------------------
// 7. Paginated attestation queries (v0.2)
// ---------------------------------------------------------------------------

#[test]
fn get_attestation_by_sequence_matches_submission_order() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 13);
    client.link_github(&wallet, &attestor, &gh);

    for i in 0..5u32 {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &i,
            &(i as u64),
            &100u32,
            &hash(&env, 110 + i as u8),
        );
    }

    for i in 0..5u32 {
        let a = client.get_attestation(&wallet, &i);
        assert_eq!(a.pr_number, i, "sequence {i} must match submission order");
    }
    assert_eq!(
        client.try_get_attestation(&wallet, &5u32),
        Err(Ok(Error::SequenceOutOfRange))
    );
}

#[test]
fn get_attestations_page_boundaries() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 14);
    client.link_github(&wallet, &attestor, &gh);

    for i in 0..10u32 {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &i,
            &(i as u64),
            &100u32,
            &hash(&env, 120 + i as u8),
        );
    }

    // A full page in the middle.
    let page = client.get_attestations_page(&wallet, &2u32, &3u32);
    assert_eq!(page.len(), 3);
    assert_eq!(page.get(0).unwrap().pr_number, 2);
    assert_eq!(page.get(2).unwrap().pr_number, 4);

    // A page that runs past the end is truncated, not an error.
    let tail = client.get_attestations_page(&wallet, &8u32, &50u32);
    assert_eq!(tail.len(), 2);

    // start == count is a valid, empty page (end-of-pagination signal).
    let empty = client.get_attestations_page(&wallet, &10u32, &10u32);
    assert!(empty.is_empty());

    // start > count is an error.
    assert_eq!(
        client.try_get_attestations_page(&wallet, &11u32, &10u32),
        Err(Ok(Error::PageStartOutOfRange))
    );

    // limit == 0 is an error.
    assert_eq!(
        client.try_get_attestations_page(&wallet, &0u32, &0u32),
        Err(Ok(Error::InvalidPageLimit))
    );

    // limit above MAX_PAGE_SIZE is an error.
    assert_eq!(
        client.try_get_attestations_page(&wallet, &0u32, &(MAX_PAGE_SIZE + 1)),
        Err(Ok(Error::PageLimitExceeded))
    );

    // limit == MAX_PAGE_SIZE is accepted.
    let max_page = client.get_attestations_page(&wallet, &0u32, &MAX_PAGE_SIZE);
    assert_eq!(max_page.len(), 10);
}

#[test]
fn paginated_reads_reconstruct_the_same_history_as_sequential_gets() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 15);
    client.link_github(&wallet, &attestor, &gh);

    let tiers = [0u32, 100, 150, 200, 0, 100, 150];
    for (i, c) in tiers.iter().enumerate() {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &(i as u32),
            &(i as u64),
            c,
            &hash(&env, 130 + i as u8),
        );
    }

    let count = client.get_attestation_count(&wallet);
    assert_eq!(count, tiers.len() as u32);

    // Reconstruct via get_attestation, one at a time.
    let mut via_get: Vec<Attestation> = Vec::new(&env);
    for seq in 0..count {
        via_get.push_back(client.get_attestation(&wallet, &seq));
    }

    // Reconstruct via a small page size, several pages.
    let mut via_pages: Vec<Attestation> = Vec::new(&env);
    let mut start = 0u32;
    loop {
        let page = client.get_attestations_page(&wallet, &start, &2u32);
        let n = page.len();
        for a in page.iter() {
            via_pages.push_back(a);
        }
        if n < 2 {
            break;
        }
        start += n;
    }

    assert_eq!(via_get, via_pages);

    // And the running score matches an independent recompute from
    // either reconstruction.
    let recomputed: u32 = via_get
        .iter()
        .map(|a| if a.complexity == 0 { 50 } else { a.complexity })
        .fold(0u32, |acc, p| acc.saturating_add(p));
    assert_eq!(client.get_reputation_score(&wallet), recomputed);
}

// ---------------------------------------------------------------------------
// 8. Storage durability (TTL) — baseline. Extended coverage lives in
// `tests/ttl_replay.rs`.
// ---------------------------------------------------------------------------

#[test]
fn linking_extends_ttl_on_all_link_records() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 1);
    client.link_github(&wallet, &attestor, &gh);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::WalletLink(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::GithubLink(gh.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(env.storage().instance().get_ttl() >= REGISTRY_TTL_THRESHOLD);
    });
}

#[test]
fn attesting_extends_ttl_on_entry_count_score_and_pr_record() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 2);
    client.link_github(&wallet, &attestor, &gh);
    let pr = hash(&env, 55);
    client.submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &100u32, &pr);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), 0)) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::AttestationCount(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::ReputationScore(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::SeenPr(pr.clone())) >= REGISTRY_TTL_THRESHOLD);
    });
}

#[test]
fn bump_wallet_core_ttl_refreshes_the_o1_records_and_is_a_noop_for_unlinked_wallet() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 3);
    client.link_github(&wallet, &attestor, &gh);
    client.submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &1u32,
        &1u64,
        &100u32,
        &hash(&env, 56),
    );

    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 10);
    client.bump_wallet_core_ttl(&wallet);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::WalletLink(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::GithubLink(gh.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::AttestationCount(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::ReputationScore(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
    });

    // No-op (does not panic, changes nothing) for a wallet with no link
    // and no history.
    let nobody = Address::generate(&env);
    client.bump_wallet_core_ttl(&nobody);
    assert_eq!(client.get_github_for_wallet(&nobody), None);
}

#[test]
fn bump_attestations_ttl_page_refreshes_only_its_page() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 4);
    client.link_github(&wallet, &attestor, &gh);
    for i in 0..4u32 {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &i,
            &(i as u64),
            &100u32,
            &hash(&env, 60 + i as u8),
        );
    }

    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 10);

    // Bump only the first page (entries 0..2).
    let refreshed = client.bump_attestations_ttl_page(&wallet, &0u32, &2u32);
    assert_eq!(refreshed, 2);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), 0)) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), 1)) >= REGISTRY_TTL_THRESHOLD);
        // Entries 2 and 3 are untouched by this page's bump.
        assert!(p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), 2)) < REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), 3)) < REGISTRY_TTL_THRESHOLD);
    });

    // A page starting at the end refreshes nothing and says so.
    let refreshed_at_end = client.bump_attestations_ttl_page(&wallet, &4u32, &2u32);
    assert_eq!(refreshed_at_end, 0);
}

// ---------------------------------------------------------------------------
// helpers
// ---------------------------------------------------------------------------

/// Install a two-signature mock (`wallet` + `attestor`) for a 3-arg
/// `(Address, Address, BytesN<32>)` contract function.
fn mock_pair(
    env: &Env,
    client: &ProofOwlRegistryClient,
    fn_name: &str,
    wallet: &Address,
    attestor: &Address,
    gh: &BytesN<32>,
) {
    let args = (wallet.clone(), attestor.clone(), gh.clone()).into_val(env);
    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name,
        args,
        sub_invokes: &[],
    };
    env.mock_auths(&[
        MockAuth {
            address: wallet,
            invoke: &invoke,
        },
        MockAuth {
            address: attestor,
            invoke: &invoke,
        },
    ]);
}
