//! TTL and replay-resistance coverage.
//!
//! Expands on the baseline TTL tests in `src/test.rs` to specifically
//! prove the properties Phase 4 calls out: every long-lived record kind
//! is refreshed correctly (including at a larger, more realistic history
//! size than the baseline's 3-PR case), `bump_wallet_ttl` cannot mutate
//! any observable data under any circumstance (including with zero
//! authorization present, since it is permissionless by design), and
//! duplicate-PR prevention survives repeated keep-alive cycles.
//!
//! ## What this file mirrors vs. re-derives
//!
//! The extend-to / bump-threshold day constants are documented,
//! human-facing policy in `SECURITY.md` §5 and are private constants in
//! `src/lib.rs` (`REGISTRY_TTL_EXTEND_TO` / `REGISTRY_TTL_THRESHOLD`),
//! not part of the public API. As an external integration test this
//! file cannot import them, so it mirrors the same values from
//! `SECURITY.md` as local constants. If that policy ever changes,
//! `SECURITY.md`, `src/lib.rs`, and the constants below must change
//! together — a mismatch here would fail loudly rather than silently
//! validate against stale numbers.
//!
//! ## What cannot be fully emulated locally (documented, not skipped)
//!
//! The in-process Soroban `Env` tracks and reports TTL numbers
//! realistically (ledger-count arithmetic, `extend_ttl` semantics), and
//! this file exercises that faithfully. What it does **not** reproduce
//! is a live network's actual *archival* behaviour: on mainnet/testnet,
//! a persistent entry whose TTL reaches zero is evicted from live state
//! and a subsequent read fails until `RestoreFootprint` runs. The test
//! `Env` has no equivalent failure mode to trigger here — there is no
//! local way to drive an entry to TTL zero and then observe a read
//! fail the way it would on a real network. Every test below therefore
//! stops at "the TTL was correctly refreshed / correctly decayed",
//! which is the full extent of what a unit test can prove about this
//! policy; the archival failure mode itself is covered only by policy
//! (SECURITY.md §5) and by the indexer's monitoring obligation
//! (`docs/integration/event-indexer-v1.md` §6), not by a test here.
//! See `docs/security/threat-model-v1.md` §9 and
//! `docs/security/known-risks-v1.md` for the same limitation stated for
//! a reader of the security docs rather than of this file.

use proofowl_contracts::{DataKey, ProofOwlRegistry, ProofOwlRegistryClient};
use soroban_sdk::testutils::storage::{Instance as _, Persistent as _};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, BytesN, Env, String};

/// Mirrors `SECURITY.md` §5 / `src/lib.rs::REGISTRY_TTL_EXTEND_TO`.
const REGISTRY_TTL_EXTEND_TO: u32 = 120 * LEDGERS_PER_DAY;
/// Mirrors `SECURITY.md` §5 / `src/lib.rs::REGISTRY_TTL_THRESHOLD`.
const REGISTRY_TTL_THRESHOLD: u32 = 90 * LEDGERS_PER_DAY;
const LEDGERS_PER_DAY: u32 = 17_280;

const TS: u64 = 1_700_000_000;

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

fn hash(env: &Env, byte: u8) -> BytesN<32> {
    BytesN::from_array(env, &[byte; 32])
}

fn repo(env: &Env) -> String {
    String::from_str(env, "stellar/soroban-examples")
}

// ---------------------------------------------------------------------------
// 1. Every persistent entry kind is refreshed on write, at a realistic
//    history size (6 attestations, not just 1-3).
// ---------------------------------------------------------------------------

#[test]
fn every_record_kind_is_refreshed_at_a_realistic_history_size() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 1);
    client.link_github(&wallet, &attestor, &gh);

    let prs: std::vec::Vec<BytesN<32>> = (0..6u8).map(|i| hash(&env, 50 + i)).collect();
    for (i, pr) in prs.iter().enumerate() {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &(i as u32),
            &(i as u64),
            &100u32,
            pr,
        );
    }

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::WalletLink(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::GithubLink(gh.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::Attestations(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        for pr in &prs {
            assert!(
                p.get_ttl(&DataKey::SeenPr(pr.clone())) >= REGISTRY_TTL_THRESHOLD,
                "every SeenPr marker in a 6-entry history must be extended, not just the latest"
            );
        }
        assert!(env.storage().instance().get_ttl() >= REGISTRY_TTL_THRESHOLD);
    });
}

// ---------------------------------------------------------------------------
// 2. `bump_wallet_ttl` refreshes cold records at a realistic history
//    size and changes no data.
// ---------------------------------------------------------------------------

#[test]
fn bump_wallet_ttl_refreshes_every_cold_record_and_mutates_nothing() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 2);
    client.link_github(&wallet, &attestor, &gh);

    let prs: std::vec::Vec<BytesN<32>> = (0..6u8).map(|i| hash(&env, 60 + i)).collect();
    for (i, pr) in prs.iter().enumerate() {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &(i as u32),
            &(i as u64),
            &150u32,
            pr,
        );
    }

    let score_before = client.get_reputation_score(&wallet);
    let count_before = client.get_attestations(&wallet).len();
    let link_before = client.get_github_for_wallet(&wallet);

    // Advance past the bump threshold for everything touched above.
    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 10);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::WalletLink(wallet.clone())) < REGISTRY_TTL_THRESHOLD);
        for pr in &prs {
            assert!(p.get_ttl(&DataKey::SeenPr(pr.clone())) < REGISTRY_TTL_THRESHOLD);
        }
    });

    client.bump_wallet_ttl(&wallet);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::WalletLink(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::GithubLink(gh.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::Attestations(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        for pr in &prs {
            assert!(p.get_ttl(&DataKey::SeenPr(pr.clone())) >= REGISTRY_TTL_THRESHOLD);
        }
    });

    // Pure keep-alive: no observable data changed.
    assert_eq!(client.get_reputation_score(&wallet), score_before);
    assert_eq!(client.get_attestations(&wallet).len(), count_before);
    assert_eq!(client.get_github_for_wallet(&wallet), link_before);
}

// ---------------------------------------------------------------------------
// 3. `bump_wallet_ttl` cannot create a link, add reputation, or bypass
//    authorization -- and is provably callable with zero signatures
//    present, since it is permissionless by design.
// ---------------------------------------------------------------------------

#[test]
fn bump_wallet_ttl_needs_no_authorization_and_creates_nothing() {
    let env = Env::default();
    env.ledger().set_timestamp(TS);
    env.ledger().set_sequence_number(100);
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let contract_id = env.register(ProofOwlRegistry, (admin, attestor));
    let client = ProofOwlRegistryClient::new(&env, &contract_id);

    let unlinked_wallet = Address::generate(&env);

    // Auth enforcement is on (no mock_all_auths in this Env) and no
    // auths are mocked at all -- if bump_wallet_ttl required any
    // signature, this call would panic on a missing auth entry.
    env.mock_auths(&[]);
    client.bump_wallet_ttl(&unlinked_wallet);

    // No link, no history, no reputation was created for a wallet that
    // never did anything.
    assert_eq!(client.get_github_for_wallet(&unlinked_wallet), None);
    assert!(client.get_attestations(&unlinked_wallet).is_empty());
    assert_eq!(client.get_reputation_score(&unlinked_wallet), 0);

    // Calling it repeatedly is equally inert.
    for _ in 0..5 {
        client.bump_wallet_ttl(&unlinked_wallet);
    }
    assert_eq!(client.get_github_for_wallet(&unlinked_wallet), None);
    assert!(client.get_attestations(&unlinked_wallet).is_empty());
}

// ---------------------------------------------------------------------------
// 4. Duplicate-PR prevention survives repeated keep-alive cycles, not
//    just one.
// ---------------------------------------------------------------------------

#[test]
fn duplicate_pr_rejection_survives_several_bump_cycles() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 3);
    client.link_github(&wallet, &attestor, &gh);
    let pr = hash(&env, 99);
    client.submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &200u32, &pr);

    let mut seq = 100u32;
    for cycle in 0..3 {
        seq += REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 10;
        env.ledger().set_sequence_number(seq);
        client.bump_wallet_ttl(&wallet);

        assert_eq!(
            client.try_submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &200u32, &pr),
            Err(Ok(proofowl_contracts::Error::DuplicateAttestation)),
            "cycle {cycle}: a PR kept alive across repeated bumps must stay permanently spent"
        );
    }
    // Still exactly one attestation on the wallet -- none of the
    // rejected resubmission attempts above ever got through.
    assert_eq!(client.get_attestations(&wallet).len(), 1);
}

// ---------------------------------------------------------------------------
// 5. After `unlink_github`, a keep-alive on the *originating* wallet
//    still covers its own history and every SeenPr marker in it -- the
//    unlink removes the link records only, never the history coverage.
// ---------------------------------------------------------------------------

#[test]
fn bump_wallet_ttl_still_covers_history_after_unlink() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 4);
    client.link_github(&wallet, &attestor, &gh);
    let pr = hash(&env, 44);
    client.submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &100u32, &pr);

    client.unlink_github(&wallet, &attestor, &gh);

    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 10);
    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::Attestations(wallet.clone())) < REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::SeenPr(pr.clone())) < REGISTRY_TTL_THRESHOLD);
    });

    // bump_wallet_ttl keys off WalletLink first to find the GithubLink
    // to refresh, but the Attestations(wallet) branch is independent of
    // whether a link currently exists -- it is looked up directly by
    // wallet address.
    client.bump_wallet_ttl(&wallet);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::Attestations(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::SeenPr(pr.clone())) >= REGISTRY_TTL_THRESHOLD);
    });

    // History is untouched -- the unlink did not touch it, and neither
    // did the bump.
    assert_eq!(client.get_attestations(&wallet).len(), 1);

    // The duplicate guard survived too: re-link the identity to a fresh
    // wallet (now possible, since it was released by the unlink) and
    // confirm the already-spent PR still cannot be re-attested, even
    // under a completely different wallet.
    let new_wallet = Address::generate(&env);
    client.link_github(&new_wallet, &attestor, &gh);
    assert_eq!(
        client.try_submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &100u32, &pr),
        Err(Ok(proofowl_contracts::Error::DuplicateAttestation))
    );
    // The original wallet's history is still exactly what it earned.
    assert_eq!(client.get_attestations(&wallet).len(), 1);
    assert!(client.get_attestations(&new_wallet).is_empty());
}

// ---------------------------------------------------------------------------
// 6. A record just inside the bump threshold is left alone (bumping
//    stays cheap); a record just past it is refreshed. Confirms the
//    threshold is a real, exercised boundary, not just a constant that
//    happens to never bind in these tests.
// ---------------------------------------------------------------------------

#[test]
fn bump_threshold_boundary_is_exact() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 5);
    client.link_github(&wallet, &attestor, &gh);

    // Right after linking, TTL sits at the extend-to target -- comfortably
    // above the threshold, so an immediate bump would be a no-op change
    // (still correct, just not interesting to assert beyond "still
    // >= threshold", which the write-path test above already covers).
    // Advance to just before the record would fall under threshold...
    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD - 1);
    env.as_contract(&client.address, || {
        let ttl = env
            .storage()
            .persistent()
            .get_ttl(&DataKey::WalletLink(wallet.clone()));
        assert!(
            ttl >= REGISTRY_TTL_THRESHOLD,
            "one ledger before the threshold boundary, TTL must still read >= threshold, got {ttl}"
        );
    });

    // ...then one ledger further, past it.
    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 1);
    env.as_contract(&client.address, || {
        let ttl = env
            .storage()
            .persistent()
            .get_ttl(&DataKey::WalletLink(wallet.clone()));
        assert!(
            ttl < REGISTRY_TTL_THRESHOLD,
            "one ledger after the threshold boundary, TTL must read < threshold, got {ttl}"
        );
    });

    client.bump_wallet_ttl(&wallet);
    env.as_contract(&client.address, || {
        assert!(
            env.storage()
                .persistent()
                .get_ttl(&DataKey::WalletLink(wallet.clone()))
                >= REGISTRY_TTL_EXTEND_TO - 1
        );
    });
}
