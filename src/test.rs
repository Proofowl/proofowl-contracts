#![cfg(test)]

use super::*;
use soroban_sdk::testutils::storage::{Instance as _, Persistent as _};
use soroban_sdk::testutils::{Address as _, Ledger as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Env, IntoVal};

const TS: u64 = 1_700_000_000;

/// Fully-authorized environment: `mock_all_auths` on, contract
/// initialized, ledger clock set. Most tests want this.
fn setup() -> (Env, ProofOwlRegistryClient<'static>, Address, Address) {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(TS);
    env.ledger().set_sequence_number(100);

    let contract_id = env.register(ProofOwlRegistry, ());
    let client = ProofOwlRegistryClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    client.init(&admin, &attestor);

    (env, client, admin, attestor)
}

/// Bare environment: no auth mocking, so `require_auth` is enforced
/// against whatever `mock_auths` list a test installs.
fn bare() -> (Env, ProofOwlRegistryClient<'static>) {
    let env = Env::default();
    env.ledger().set_timestamp(TS);
    env.ledger().set_sequence_number(100);
    let contract_id = env.register(ProofOwlRegistry, ());
    let client = ProofOwlRegistryClient::new(&env, &contract_id);
    (env, client)
}

fn hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn repo(env: &Env) -> String {
    String::from_str(env, "stellar/soroban-examples")
}

// ---------------------------------------------------------------------------
// 1. Initialization
// ---------------------------------------------------------------------------

#[test]
fn init_requires_admin_authorization() {
    let (env, client) = bare();
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);

    // No auth provided at all -> init must fail rather than store config.
    let res = client.try_init(&admin, &attestor);
    assert!(res.is_err());
    assert_eq!(client.try_get_admin(), Ok(Ok(None)));

    // With the proposed admin's signature it succeeds.
    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name: "init",
        args: (admin.clone(), attestor.clone()).into_val(&env),
        sub_invokes: &[],
    };
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &invoke,
    }]);
    client.init(&admin, &attestor);
    assert_eq!(client.get_admin(), Some(admin));
    assert_eq!(client.get_attestor(), Some(attestor));
}

#[test]
fn init_rejects_caller_choosing_an_admin_they_do_not_control() {
    let (env, client) = bare();
    let attacker = Address::generate(&env);
    let victim_admin = Address::generate(&env);
    let attestor = Address::generate(&env);

    // Attacker can only sign as themselves; they try to install
    // `victim_admin` (or, equivalently, an address they picked but whose
    // key they lack) as admin.
    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name: "init",
        args: (victim_admin.clone(), attestor.clone()).into_val(&env),
        sub_invokes: &[],
    };
    env.mock_auths(&[MockAuth {
        address: &attacker,
        invoke: &invoke,
    }]);

    let res = client.try_init(&victim_admin, &attestor);
    assert!(res.is_err());
    assert_eq!(client.try_get_admin(), Ok(Ok(None)));
}

#[test]
fn double_init_fails() {
    let (_env, client, admin, attestor) = setup();
    assert_eq!(
        client.try_init(&admin, &attestor),
        Err(Ok(Error::AlreadyInitialized))
    );
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
    let (env, client, _admin, attestor) = setup_bare_initialized();
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
    let (env, client, _admin, attestor) = setup_bare_initialized();
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
    let (env, client, _admin, attestor) = setup_bare_initialized();
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
    let (env, client, _admin, attestor) = setup_bare_initialized();
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
    assert_eq!(client.get_attestations(&wallet).len(), 1);
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

    let list = client.get_attestations(&wallet);
    assert_eq!(list.len(), 1);
    let a = list.get(0).unwrap();
    assert_eq!(a.repo, repo(&env));
    assert_eq!(a.pr_number, 123);
    assert_eq!(a.issue_id, 101);
    assert_eq!(a.complexity, 150);
    assert_eq!(a.pr_hash, pr);
    assert_eq!(a.timestamp, TS); // ledger time, not caller-supplied
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
    assert!(client.get_attestations(&wallet).is_empty());
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
    assert_eq!(client.get_attestations(&wallet).len(), 4);
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
    let (env, client, ..) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 8);
    client.link_github(&wallet, &_attestor_of(&client), &gh);

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
    let (env, client, _admin, attestor) = setup_bare_initialized();
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
    let (env, client, admin, _attestor) = setup_bare_initialized();
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
    assert_eq!(client.get_attestations(&wallet).len(), 3);
}

#[test]
fn reputation_score_is_zero_for_unknown_wallet() {
    let (env, client, ..) = setup();
    let nobody = Address::generate(&env);
    assert_eq!(client.get_reputation_score(&nobody), 0);
    assert!(client.get_attestations(&nobody).is_empty());
}

// ---------------------------------------------------------------------------
// 7. Storage durability (TTL)
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
fn attesting_extends_ttl_on_history_and_pr_records() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 2);
    client.link_github(&wallet, &attestor, &gh);
    let pr = hash(&env, 55);
    client.submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &100u32, &pr);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::Attestations(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::SeenPr(pr.clone())) >= REGISTRY_TTL_THRESHOLD);
    });
}

#[test]
fn bump_wallet_ttl_refreshes_cold_records() {
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

    // Advance the ledger clock so the entries have decayed a bit.
    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 10);

    client.bump_wallet_ttl(&wallet);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::WalletLink(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::GithubLink(gh.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::Attestations(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
    });
}

#[test]
fn bump_wallet_ttl_is_a_noop_for_unlinked_wallet() {
    let (env, client, ..) = setup();
    let nobody = Address::generate(&env);
    client.bump_wallet_ttl(&nobody); // must not panic
}

// ---------------------------------------------------------------------------
// helpers that need an initialized-but-not-fully-mocked env
// ---------------------------------------------------------------------------

/// Like `setup`, but drops back to enforced auth after `init` so tests
/// can install precise `mock_auths` lists.
fn setup_bare_initialized() -> (Env, ProofOwlRegistryClient<'static>, Address, Address) {
    let (env, client) = bare();
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);

    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name: "init",
        args: (admin.clone(), attestor.clone()).into_val(&env),
        sub_invokes: &[],
    };
    env.mock_auths(&[MockAuth {
        address: &admin,
        invoke: &invoke,
    }]);
    client.init(&admin, &attestor);
    (env, client, admin, attestor)
}

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

fn _attestor_of(client: &ProofOwlRegistryClient) -> Address {
    client.get_attestor().unwrap()
}
