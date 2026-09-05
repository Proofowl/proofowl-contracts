//! Authorization security matrix.
//!
//! Table-driven adversarial coverage of every `require_auth()` boundary
//! in the contract, plus two structural invariants that don't fit a
//! single-call test: admin immutability (nothing except deployment can
//! ever change `Admin`) and error-code discriminant stability (the
//! numeric codes the TypeScript SDK's `ProofOwlErrorCode` enum depends
//! on can't silently renumber).
//!
//! This file is an integration test (`tests/`), so it only sees the
//! crate's public API — same boundary a real external caller sees. It
//! deliberately does not re-derive `SECURITY.md`'s TTL constants; those
//! are exercised in `tests/ttl_replay.rs` against the crate's own
//! internal test module instead.
//!
//! Everything here runs offline against the in-process Soroban `Env`.
//! No network call, no real key, no testnet transaction.

use proofowl_contracts::{Attestation, Error, ProofOwlRegistry, ProofOwlRegistryClient};
use soroban_sdk::testutils::{Address as _, Ledger as _, MockAuth, MockAuthInvoke};
use soroban_sdk::{Address, BytesN, Env, IntoVal, String};

const TS: u64 = 1_700_000_000;

fn hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn repo(env: &Env) -> String {
    String::from_str(env, "stellar/soroban-examples")
}

/// Fully-mocked environment (every `require_auth()` passes). Used for
/// tests that are about *state* (e.g. "does a wrong-but-signed attestor
/// get rejected"), not about *missing signatures*.
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

/// Auth *enforced* after deployment (constructor auth is still
/// force-mocked by `Env::register` — documented SDK behaviour, also
/// covered end-to-end in `tests/constructor_auth.rs`).
fn setup_enforced() -> (Env, ProofOwlRegistryClient<'static>, Address, Address) {
    let env = Env::default();
    env.ledger().set_timestamp(TS);
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let contract_id = env.register(ProofOwlRegistry, (admin.clone(), attestor.clone()));
    let client = ProofOwlRegistryClient::new(&env, &contract_id);
    (env, client, admin, attestor)
}

/// Install a mock-auth set for a named function call, signed by exactly
/// the addresses in `signers` (in order). Any `require_auth()` the call
/// makes for an address NOT in this list will fail.
fn mock_only(
    env: &Env,
    client: &ProofOwlRegistryClient,
    fn_name: &'static str,
    args: soroban_sdk::Vec<soroban_sdk::Val>,
    signers: &[&Address],
) {
    let invoke = MockAuthInvoke {
        contract: &client.address,
        fn_name,
        args: args.into_val(env),
        sub_invokes: &[],
    };
    let auths: std::vec::Vec<MockAuth> = signers
        .iter()
        .map(|addr| MockAuth {
            address: addr,
            invoke: &invoke,
        })
        .collect();
    env.mock_auths(&auths);
}

// ---------------------------------------------------------------------------
// 1. `link_github` — two-party matrix
// ---------------------------------------------------------------------------

#[test]
fn link_github_auth_matrix() {
    // (signers present, expect success)
    struct Case {
        name: &'static str,
        sign_wallet: bool,
        sign_attestor: bool,
        expect_ok: bool,
    }
    let cases = [
        Case {
            name: "both sign -> ok",
            sign_wallet: true,
            sign_attestor: true,
            expect_ok: true,
        },
        Case {
            name: "wallet only -> rejected",
            sign_wallet: true,
            sign_attestor: false,
            expect_ok: false,
        },
        Case {
            name: "attestor only -> rejected",
            sign_wallet: false,
            sign_attestor: true,
            expect_ok: false,
        },
        Case {
            name: "neither -> rejected",
            sign_wallet: false,
            sign_attestor: false,
            expect_ok: false,
        },
    ];

    for case in cases {
        let (env, client, _admin, attestor) = setup_enforced();
        let wallet = Address::generate(&env);
        let gh = hash(&env, 1);

        let mut signers: std::vec::Vec<&Address> = std::vec::Vec::new();
        if case.sign_wallet {
            signers.push(&wallet);
        }
        if case.sign_attestor {
            signers.push(&attestor);
        }
        let args = (wallet.clone(), attestor.clone(), gh.clone()).into_val(&env);
        mock_only(&env, &client, "link_github", args, &signers);

        let result = client.try_link_github(&wallet, &attestor, &gh);
        assert_eq!(
            result.is_ok(),
            case.expect_ok,
            "case {:?}: expected ok={} got {:?}",
            case.name,
            case.expect_ok,
            result
        );
        if !case.expect_ok {
            assert_eq!(
                client.get_wallet_for_github(&gh),
                None,
                "case {:?}",
                case.name
            );
        }
    }
}

#[test]
fn link_github_rejects_a_fully_signed_wrong_attestor() {
    // Every signature present and valid; the stored-attestor identity
    // check must still be the deciding factor, not mere signature
    // presence.
    let (env, client, ..) = setup();
    let wallet = Address::generate(&env);
    let impostor_attestor = Address::generate(&env);
    let gh = hash(&env, 2);

    assert_eq!(
        client.try_link_github(&wallet, &impostor_attestor, &gh),
        Err(Ok(Error::Unauthorized))
    );
    assert_eq!(client.get_wallet_for_github(&gh), None);
}

// ---------------------------------------------------------------------------
// 2. `unlink_github` — two-party matrix, mirrors link_github
// ---------------------------------------------------------------------------

#[test]
fn unlink_github_auth_matrix() {
    struct Case {
        name: &'static str,
        sign_wallet: bool,
        sign_attestor: bool,
        expect_ok: bool,
    }
    let cases = [
        Case {
            name: "both sign -> ok",
            sign_wallet: true,
            sign_attestor: true,
            expect_ok: true,
        },
        Case {
            name: "wallet only -> rejected",
            sign_wallet: true,
            sign_attestor: false,
            expect_ok: false,
        },
        Case {
            name: "attestor only -> rejected",
            sign_wallet: false,
            sign_attestor: true,
            expect_ok: false,
        },
    ];

    for case in cases {
        let (env, client, _admin, attestor) = setup();
        let wallet = Address::generate(&env);
        let gh = hash(&env, 3);
        client.link_github(&wallet, &attestor, &gh);

        // Switch to enforced auth for the unlink attempt itself.
        let mut signers: std::vec::Vec<&Address> = std::vec::Vec::new();
        if case.sign_wallet {
            signers.push(&wallet);
        }
        if case.sign_attestor {
            signers.push(&attestor);
        }
        let args = (wallet.clone(), attestor.clone(), gh.clone()).into_val(&env);
        mock_only(&env, &client, "unlink_github", args, &signers);

        let result = client.try_unlink_github(&wallet, &attestor, &gh);
        assert_eq!(
            result.is_ok(),
            case.expect_ok,
            "case {:?}: expected ok={} got {:?}",
            case.name,
            case.expect_ok,
            result
        );
        if !case.expect_ok {
            // Link must remain intact after a rejected unlink attempt.
            assert_eq!(
                client.get_wallet_for_github(&gh),
                Some(wallet),
                "case {:?}",
                case.name
            );
        }
    }
}

// ---------------------------------------------------------------------------
// 3. `submit_attestation` — attestor-only, both directions
// ---------------------------------------------------------------------------

#[test]
fn submit_attestation_requires_attestor_signature_present() {
    let (env, client, _admin, attestor) = setup_enforced();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 4);

    // Link first, fully mocked.
    env.mock_all_auths();
    client.link_github(&wallet, &attestor, &gh);

    // No auth entries at all for the attestation call.
    env.mock_auths(&[]);
    let result = client.try_submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &1u32,
        &1u64,
        &100u32,
        &hash(&env, 40),
    );
    assert!(result.is_err(), "unsigned attestor call must be rejected");
    assert!(client.get_attestations(&wallet).is_empty());
}

#[test]
fn submit_attestation_rejects_a_signed_but_wrong_attestor() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 5);
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
            &hash(&env, 50)
        ),
        Err(Ok(Error::Unauthorized))
    );
    assert!(client.get_attestations(&wallet).is_empty());
}

// ---------------------------------------------------------------------------
// 4. `set_attestor` — admin-only, both directions
// ---------------------------------------------------------------------------

#[test]
fn set_attestor_requires_admin_signature_present() {
    let (env, client, admin, attestor) = setup_enforced();
    let new_attestor = Address::generate(&env);

    // A stranger signs instead of the admin.
    let stranger = Address::generate(&env);
    let args = (admin.clone(), new_attestor.clone()).into_val(&env);
    mock_only(&env, &client, "set_attestor", args, &[&stranger]);

    assert!(client.try_set_attestor(&admin, &new_attestor).is_err());
    assert_eq!(client.get_attestor(), Some(attestor));
}

#[test]
fn set_attestor_rejects_a_signed_but_wrong_admin() {
    let (env, client, _admin, attestor) = setup();
    let impostor_admin = Address::generate(&env);
    let new_attestor = Address::generate(&env);

    assert_eq!(
        client.try_set_attestor(&impostor_admin, &new_attestor),
        Err(Ok(Error::Unauthorized))
    );
    // Attestor untouched by the rejected call.
    assert_eq!(client.get_attestor(), Some(attestor));
}

// ---------------------------------------------------------------------------
// 5. Admin immutability — nothing but deployment can ever change Admin
// ---------------------------------------------------------------------------

#[test]
fn admin_is_immutable_across_every_mutating_call() {
    let (env, client, admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 6);

    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Exercise every mutating entry point at least once, including
    // hostile attempts, and check `get_admin()` after each one. There
    // is no `set_admin` function to call; this loop exists to make the
    // absence operationally verified, not merely asserted by silence.
    client.link_github(&wallet, &attestor, &gh);
    assert_eq!(client.get_admin(), Some(admin.clone()));

    let new_attestor = Address::generate(&env);
    client.set_attestor(&admin, &new_attestor);
    assert_eq!(client.get_admin(), Some(admin.clone()));

    let pr = hash(&env, 60);
    client.submit_attestation(&new_attestor, &gh, &repo(&env), &1u32, &1u64, &100u32, &pr);
    assert_eq!(client.get_admin(), Some(admin.clone()));

    client.bump_wallet_ttl(&wallet);
    assert_eq!(client.get_admin(), Some(admin.clone()));

    client.unlink_github(&wallet, &new_attestor, &gh);
    assert_eq!(client.get_admin(), Some(admin.clone()));

    // Hostile attempts (wrong signer, bad args) must not touch Admin
    // either -- they should fail before reaching any storage write.
    let stranger = Address::generate(&env);
    let _ = client.try_set_attestor(&stranger, &Address::generate(&env));
    assert_eq!(client.get_admin(), Some(admin.clone()));
    let _ = client.try_link_github(&wallet, &Address::generate(&env), &hash(&env, 61));
    assert_eq!(client.get_admin(), Some(admin));
}

// ---------------------------------------------------------------------------
// 6. Error-code discriminant stability
//
// The TypeScript SDK's `ProofOwlErrorCode` enum (sdk/typescript/src/errors.ts)
// and the generated bindings both hardcode these numbers. If the Rust
// `#[contracterror] enum Error` is ever reordered, this is the test that
// catches the silent renumbering before it reaches a release.
// ---------------------------------------------------------------------------

#[test]
fn error_code_discriminants_match_the_published_abi() {
    assert_eq!(Error::AlreadyInitialized as u32, 1);
    assert_eq!(Error::NotInitialized as u32, 2);
    assert_eq!(Error::Unauthorized as u32, 3);
    assert_eq!(Error::WalletAlreadyLinked as u32, 4);
    assert_eq!(Error::GithubAlreadyLinked as u32, 5);
    assert_eq!(Error::DuplicateAttestation as u32, 6);
    assert_eq!(Error::WalletNotLinked as u32, 7);
    assert_eq!(Error::InvalidComplexity as u32, 8);
    assert_eq!(Error::LinkNotFound as u32, 9);
}

// ---------------------------------------------------------------------------
// 7. `NotInitialized` (error 2) — practically unreachable via normal
// calls (there's no way to un-set the instance short of archival), so
// this test reaches into storage directly to prove the code path
// actually exists and behaves as documented, rather than trusting that
// it's merely "never hit in practice".
// ---------------------------------------------------------------------------

#[test]
fn not_initialized_is_reachable_and_rejects_every_gated_call() {
    use proofowl_contracts::DataKey;

    let (env, client, admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 7);
    client.link_github(&wallet, &attestor, &gh);

    // Simulate an archived / never-configured instance by removing the
    // instance-storage config directly. This is the same effective
    // state `check_attestor` and `set_attestor` treat as
    // `NotInitialized` -- it cannot be produced by any public call, only
    // by real archival (out of scope for a local unit test) or, as
    // here, by directly manipulating storage to exercise the path.
    env.as_contract(&client.address, || {
        env.storage().instance().remove(&DataKey::Attestor);
    });

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
        Err(Ok(Error::NotInitialized))
    );
    assert_eq!(
        client.try_link_github(&Address::generate(&env), &attestor, &hash(&env, 71)),
        Err(Ok(Error::NotInitialized))
    );
    assert_eq!(
        client.try_unlink_github(&wallet, &attestor, &gh),
        Err(Ok(Error::NotInitialized))
    );

    env.as_contract(&client.address, || {
        env.storage().instance().remove(&DataKey::Admin);
    });
    assert_eq!(
        client.try_set_attestor(&admin, &Address::generate(&env)),
        Err(Ok(Error::NotInitialized))
    );

    // Read methods are documented as "practically unreachable" for
    // NotInitialized because they don't check the instance at all --
    // confirm they degrade gracefully (empty/None) rather than panic.
    // No attestation was ever accepted (every gated call above was
    // rejected), so the wallet's history is still empty.
    assert!(client.get_attestations(&wallet).is_empty());
    assert_eq!(client.get_admin(), None);
}

// ---------------------------------------------------------------------------
// 8. Repeated / cross rotation attempts
// ---------------------------------------------------------------------------

#[test]
fn repeated_attestor_rotation_only_the_latest_key_ever_works() {
    let (env, client, admin, a0) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 8);
    client.link_github(&wallet, &a0, &gh);

    let mut previous = std::vec![a0.clone()];
    let mut current = a0;
    for i in 0..5u8 {
        let next = Address::generate(&env);
        client.set_attestor(&admin, &next);
        assert_eq!(client.get_attestor(), Some(next.clone()));

        // Every previously valid attestor key, including the one
        // rotated out just now, is immediately rejected.
        previous.push(current.clone());
        for old in &previous {
            assert_eq!(
                client.try_submit_attestation(
                    old,
                    &gh,
                    &repo(&env),
                    &1u32,
                    &1u64,
                    &100u32,
                    &hash(&env, 80 + i)
                ),
                Err(Ok(Error::Unauthorized)),
                "stale attestor {old:?} must be rejected after rotation {i}"
            );
        }
        // The new key works.
        client.submit_attestation(
            &next,
            &gh,
            &repo(&env),
            &1u32,
            &1u64,
            &100u32,
            &hash(&env, 90 + i),
        );
        current = next;
    }
}

// ---------------------------------------------------------------------------
// 9. Cross-wallet / cross-GitHub mismatch attempts
// ---------------------------------------------------------------------------

#[test]
fn unlink_rejects_every_mismatched_pairing() {
    let (env, client, _admin, attestor) = setup();
    let wallet_a = Address::generate(&env);
    let wallet_b = Address::generate(&env);
    let gh_a = hash(&env, 9);
    let gh_b = hash(&env, 10);
    client.link_github(&wallet_a, &attestor, &gh_a);
    client.link_github(&wallet_b, &attestor, &gh_b);

    // wallet_a's real hash, but claimed by wallet_b.
    assert_eq!(
        client.try_unlink_github(&wallet_b, &attestor, &gh_a),
        Err(Ok(Error::LinkNotFound))
    );
    // wallet_a claiming wallet_b's hash.
    assert_eq!(
        client.try_unlink_github(&wallet_a, &attestor, &gh_b),
        Err(Ok(Error::LinkNotFound))
    );
    // Both links must survive both rejected attempts untouched.
    assert_eq!(client.get_wallet_for_github(&gh_a), Some(wallet_a));
    assert_eq!(client.get_wallet_for_github(&gh_b), Some(wallet_b));
}

#[test]
fn submit_attestation_for_unlinked_identity_never_credits_an_unrelated_wallet() {
    let (env, client, _admin, attestor) = setup();
    let linked_wallet = Address::generate(&env);
    let linked_gh = hash(&env, 11);
    client.link_github(&linked_wallet, &attestor, &linked_gh);

    // A different, never-linked identity hash must resolve to nobody,
    // never to `linked_wallet` or any other existing linked wallet.
    let stray_gh = hash(&env, 12);
    assert_eq!(
        client.try_submit_attestation(
            &attestor,
            &stray_gh,
            &repo(&env),
            &1u32,
            &1u64,
            &100u32,
            &hash(&env, 120)
        ),
        Err(Ok(Error::WalletNotLinked))
    );
    assert!(client.get_attestations(&linked_wallet).is_empty());
}

// ---------------------------------------------------------------------------
// 10. Duplicate link / unlink attempts
// ---------------------------------------------------------------------------

#[test]
fn duplicate_link_and_unlink_attempts_are_rejected_and_atomic() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 13);
    client.link_github(&wallet, &attestor, &gh);

    // Re-linking the same wallet to a different identity fails without
    // disturbing the existing link.
    assert_eq!(
        client.try_link_github(&wallet, &attestor, &hash(&env, 14)),
        Err(Ok(Error::WalletAlreadyLinked))
    );
    assert_eq!(client.get_github_for_wallet(&wallet), Some(gh.clone()));

    client.unlink_github(&wallet, &attestor, &gh);

    // Unlinking again (already gone) fails cleanly.
    assert_eq!(
        client.try_unlink_github(&wallet, &attestor, &gh),
        Err(Ok(Error::LinkNotFound))
    );
    assert_eq!(client.get_github_for_wallet(&wallet), None);
    assert_eq!(client.get_wallet_for_github(&gh), None);
}

// ---------------------------------------------------------------------------
// 11. Invalid complexity never mutates state (atomicity)
// ---------------------------------------------------------------------------

#[test]
fn invalid_complexity_leaves_no_partial_record_anywhere() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 15);
    client.link_github(&wallet, &attestor, &gh);

    for bad in [1u32, 2, 99, 101, 149, 151, 199, 201, u32::MAX] {
        let pr = hash(&env, (bad % 251) as u8);
        assert_eq!(
            client.try_submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &bad, &pr),
            Err(Ok(Error::InvalidComplexity))
        );
    }

    // No attestation, no score, and — critically — the pr_hash values
    // used in the rejected attempts must NOT be marked seen (otherwise
    // a legitimate future submission with the same pr_hash would be
    // wrongly rejected as a duplicate).
    assert!(client.get_attestations(&wallet).is_empty());
    assert_eq!(client.get_reputation_score(&wallet), 0);
    let reused_pr = hash(&env, 1u8);
    client.submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &1u32,
        &1u64,
        &100u32,
        &reused_pr,
    );
    assert_eq!(client.get_attestations(&wallet).len(), 1);
}

// ---------------------------------------------------------------------------
// 12. Reputation score equals accepted attestations under the
// documented scoring rule (complexity, or 50 if complexity == 0),
// cross-checked by independent recomputation for every case.
// ---------------------------------------------------------------------------

#[test]
fn reputation_score_matches_independent_recomputation() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 16);
    client.link_github(&wallet, &attestor, &gh);

    let tiers = [0u32, 100, 0, 150, 200, 0, 100];
    for (i, c) in tiers.iter().enumerate() {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &(i as u32),
            &(i as u64),
            c,
            &hash(&env, 160 + i as u8),
        );
    }

    let list: soroban_sdk::Vec<Attestation> = client.get_attestations(&wallet);
    let recomputed: u32 = list
        .iter()
        .map(|a| if a.complexity == 0 { 50 } else { a.complexity })
        .sum();

    assert_eq!(client.get_reputation_score(&wallet), recomputed);
    // 0->50, 100, 0->50, 150, 200, 0->50, 100 = 50+100+50+150+200+50+100
    assert_eq!(recomputed, 700);
}
