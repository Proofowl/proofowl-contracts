//! Registry state-machine invariants.
//!
//! A deterministic, model-based adversarial test: a fixed-seed PRNG (no
//! external crate — a small inline splitmix64, seeded with a constant)
//! drives a long sequence of valid and hostile actions against the real
//! contract, while a plain-Rust shadow "model" independently predicts
//! what should happen to each action using the same rules
//! `src/lib.rs` documents. After every single step, the model's
//! predicted outcome and post-state are compared against the contract's
//! actual outcome and post-state (via its own read methods). Any
//! divergence is a contract bug (or a model bug — the point of running
//! this for hundreds of steps against a small, closed universe of
//! wallets/identities/attestors is that both are exercised heavily
//! enough that they'd have to agree by construction, not by luck).
//!
//! Fully offline, fully deterministic: the same seed always produces
//! the same sequence of actions, so a failure is always reproducible by
//! re-running this file. No network call, no real key, no testnet
//! transaction.

use proofowl_contracts::{Error, ProofOwlRegistry, ProofOwlRegistryClient};
use soroban_sdk::testutils::{Address as _, Ledger as _};
use soroban_sdk::{Address, BytesN, Env, String};
use std::collections::HashSet;

// --- deterministic PRNG (splitmix64) ---------------------------------------

struct Rng(u64);

impl Rng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Fixed seed: any test failure below is reproducible by re-running this
/// file unchanged. Chosen arbitrarily; do not "reroll" it to make a
/// failing case disappear — a failure here means the model and the
/// contract disagree, and changing the seed only hides which action
/// sequence exposed it.
const SEED: u64 = 0x50524f4f464f574c; // ASCII "PROOFOWL" reinterpreted as u64

// --- universe ---------------------------------------------------------------

const N_WALLETS: usize = 4;
const N_GH: usize = 4;
const N_ATTESTORS: usize = 3;
const N_PR_IDS: u8 = 20;
const STEPS: usize = 400;

/// Valid tiers plus a spread of invalid values, so both branches of
/// `InvalidComplexity` get exercised throughout the run.
const COMPLEXITY_POOL: [u32; 9] = [0, 100, 150, 200, 1, 50, 99, 201, 999_999];

fn gh(env: &Env, idx: usize) -> BytesN<32> {
    BytesN::from_array(env, &[(idx + 1) as u8; 32])
}

fn pr_hash(env: &Env, id: u8) -> BytesN<32> {
    // Distinguish from `gh` byte patterns by offsetting into a disjoint
    // range; collision would only matter if the contract ever compared
    // a GithubLink key against a SeenPr key, which it never does (they
    // are different DataKey variants), but keeping them visually
    // distinct keeps failure output readable.
    BytesN::from_array(env, &[100u8.wrapping_add(id); 32])
}

fn repo(env: &Env) -> String {
    String::from_str(env, "stellar/soroban-examples")
}

fn score_for(complexity: u32) -> u32 {
    if complexity == 0 {
        50
    } else {
        complexity
    }
}

// --- shadow model ------------------------------------------------------------

#[derive(Clone)]
struct Model {
    /// wallet index -> linked gh index
    link: [Option<usize>; N_WALLETS],
    /// gh index -> linked wallet index
    reverse: [Option<usize>; N_GH],
    /// wallet index -> number of accepted attestations
    count: [u32; N_WALLETS],
    /// wallet index -> reputation score under the documented scoring rule
    score: [u32; N_WALLETS],
    /// pr ids already credited, globally, forever (never removed, even
    /// across an unlink -- mirrors `SeenPr`'s lifetime).
    seen_pr: HashSet<u8>,
    /// index into the attestor address pool currently stored on-chain
    attestor: usize,
}

impl Model {
    fn new() -> Self {
        Model {
            link: [None; N_WALLETS],
            reverse: [None; N_GH],
            count: [0; N_WALLETS],
            score: [0; N_WALLETS],
            seen_pr: HashSet::new(),
            attestor: 0,
        }
    }
}

// --- driver ------------------------------------------------------------------

#[test]
fn registry_state_machine_holds_its_invariants_over_a_long_hostile_sequence() {
    let env = Env::default();
    env.mock_all_auths();
    env.ledger().set_timestamp(1_700_000_000);

    let admin = Address::generate(&env);
    let attestors: std::vec::Vec<Address> =
        (0..N_ATTESTORS).map(|_| Address::generate(&env)).collect();
    let wallets: std::vec::Vec<Address> = (0..N_WALLETS).map(|_| Address::generate(&env)).collect();
    let ghs: std::vec::Vec<BytesN<32>> = (0..N_GH).map(|i| gh(&env, i)).collect();

    let contract_id = env.register(ProofOwlRegistry, (admin.clone(), attestors[0].clone()));
    let client = ProofOwlRegistryClient::new(&env, &contract_id);

    let mut model = Model::new();
    let mut rng = Rng(SEED);

    for step in 0..STEPS {
        // Invariant, checked every step regardless of which action ran:
        // link/reverse consistency, in both directions, for the whole
        // universe -- "one wallet maps to at most one GitHub identity
        // and vice versa" and "unlink removes both directions
        // atomically" are exactly the claim that these two maps agree
        // with the contract's own getters at all times, not just right
        // after a link/unlink call.
        assert_admin_never_changed(&client, &admin, step);
        assert_link_consistency(&client, &model, &wallets, &ghs, step);
        assert_scores_match(&client, &model, &wallets, step);

        match rng.below(7) {
            0 => do_link(&client, &mut model, &mut rng, &wallets, &ghs, step),
            1 => do_unlink(&client, &mut model, &mut rng, &wallets, &ghs, step),
            2 => do_submit_valid_attestor(
                &env, &client, &mut model, &mut rng, &wallets, &ghs, &attestors, step,
            ),
            3 => do_submit_wrong_attestor(&env, &client, &model, &mut rng, &ghs, &attestors, step),
            4 => do_rotate_by_admin(&env, &client, &mut model, &mut rng, &attestors, step),
            5 => do_rotate_by_impostor(&client, &model, &mut rng, &wallets, &attestors, step),
            _ => do_bump_ttl(&client, &mut rng, &wallets, step),
        }
    }

    // Final full-state cross-check, independent of the per-step checks
    // above: recompute every wallet's expected attestation count and
    // score from the model and compare against both `get_attestations`
    // and `get_reputation_score`.
    for (i, w) in wallets.iter().enumerate() {
        let list = client.get_attestations(w);
        assert_eq!(
            list.len(),
            model.count[i],
            "final attestation count mismatch for wallet {i}"
        );
        let recomputed: u32 = list.iter().map(|a| score_for(a.complexity)).sum();
        assert_eq!(
            recomputed, model.score[i],
            "final recompute mismatch for wallet {i}"
        );
        assert_eq!(
            client.get_reputation_score(w),
            model.score[i],
            "final get_reputation_score mismatch for wallet {i}"
        );
    }
}

// --- action implementations --------------------------------------------------

fn do_link(
    client: &ProofOwlRegistryClient,
    model: &mut Model,
    rng: &mut Rng,
    wallets: &[Address],
    ghs: &[BytesN<32>],
    step: usize,
) {
    let wi = rng.below(N_WALLETS);
    let gi = rng.below(N_GH);
    // Sometimes deliberately re-link an already-linked wallet or an
    // already-claimed identity (identity-squat / re-link attempt).
    let attestor = client.get_attestor().unwrap();

    let expect_ok = model.link[wi].is_none() && model.reverse[gi].is_none();
    let before = model.clone();

    let result = client.try_link_github(&wallets[wi], &attestor, &ghs[gi]);

    if expect_ok {
        assert!(
            result.is_ok(),
            "step {step}: expected link_github to succeed, got {result:?}"
        );
        model.link[wi] = Some(gi);
        model.reverse[gi] = Some(wi);
    } else {
        let expected_err = if model.link[wi].is_some() {
            Error::WalletAlreadyLinked
        } else {
            Error::GithubAlreadyLinked
        };
        assert_eq!(
            result,
            Err(Ok(expected_err)),
            "step {step}: hostile link_github(wallet={wi}, gh={gi})"
        );
        assert_model_unchanged(&before, model, step, "rejected link_github");
    }
}

fn do_unlink(
    client: &ProofOwlRegistryClient,
    model: &mut Model,
    rng: &mut Rng,
    wallets: &[Address],
    ghs: &[BytesN<32>],
    step: usize,
) {
    let wi = rng.below(N_WALLETS);
    // Half the time attempt the wallet's real linked identity (if any);
    // half the time attempt a random one, which is a cross-identity
    // mismatch attempt whenever it doesn't happen to be the real one.
    let gi = if rng.below(2) == 0 {
        model.link[wi].unwrap_or_else(|| rng.below(N_GH))
    } else {
        rng.below(N_GH)
    };
    let attestor = client.get_attestor().unwrap();
    let expect_ok = model.link[wi] == Some(gi);
    let before = model.clone();

    let result = client.try_unlink_github(&wallets[wi], &attestor, &ghs[gi]);

    if expect_ok {
        assert!(
            result.is_ok(),
            "step {step}: expected unlink_github to succeed, got {result:?}"
        );
        model.link[wi] = None;
        model.reverse[gi] = None;
        // History and score are untouched -- verified by
        // assert_scores_match on the next loop iteration finding no
        // change, since nothing here mutates model.count/model.score.
    } else {
        assert_eq!(
            result,
            Err(Ok(Error::LinkNotFound)),
            "step {step}: hostile unlink_github(wallet={wi}, gh={gi})"
        );
        assert_model_unchanged(&before, model, step, "rejected unlink_github");
    }
}

#[allow(clippy::too_many_arguments)]
fn do_submit_valid_attestor(
    env: &Env,
    client: &ProofOwlRegistryClient,
    model: &mut Model,
    rng: &mut Rng,
    wallets: &[Address],
    ghs: &[BytesN<32>],
    attestors: &[Address],
    step: usize,
) {
    let gi = rng.below(N_GH);
    let pr_id = rng.below(N_PR_IDS as usize) as u8;
    let complexity = COMPLEXITY_POOL[rng.below(COMPLEXITY_POOL.len())];
    let valid_complexity = matches!(complexity, 0 | 100 | 150 | 200);
    let already_seen = model.seen_pr.contains(&pr_id);
    let linked_wallet = model.reverse[gi];

    let attestor = attestors[model.attestor].clone();
    let before = model.clone();

    let result = client.try_submit_attestation(
        &attestor,
        &ghs[gi],
        &repo(env),
        &(pr_id as u32),
        &(pr_id as u64),
        &complexity,
        &pr_hash(env, pr_id),
    );

    // Evaluation order documented in contract-api-v1.md: auth -> complexity
    // -> wallet resolution -> dedup. Model that exact order.
    if !valid_complexity {
        assert_eq!(
            result,
            Err(Ok(Error::InvalidComplexity)),
            "step {step}: submit with invalid complexity {complexity}"
        );
        assert_model_unchanged(&before, model, step, "invalid complexity");
        return;
    }
    if linked_wallet.is_none() {
        assert_eq!(
            result,
            Err(Ok(Error::WalletNotLinked)),
            "step {step}: submit for unlinked gh {gi}"
        );
        assert_model_unchanged(&before, model, step, "unlinked gh");
        return;
    }
    if already_seen {
        assert_eq!(
            result,
            Err(Ok(Error::DuplicateAttestation)),
            "step {step}: submit with already-seen pr id {pr_id}"
        );
        assert_model_unchanged(&before, model, step, "duplicate pr");
        return;
    }

    // All conditions satisfied: must succeed and credit exactly the
    // linked wallet, never any other.
    let wi = linked_wallet.unwrap();
    assert_eq!(
        result,
        Ok(Ok(wallets[wi].clone())),
        "step {step}: expected submit_attestation to credit wallet {wi}"
    );
    model.count[wi] += 1;
    model.score[wi] = model.score[wi].saturating_add(score_for(complexity));
    model.seen_pr.insert(pr_id);
}

fn do_submit_wrong_attestor(
    env: &Env,
    client: &ProofOwlRegistryClient,
    model: &Model,
    rng: &mut Rng,
    ghs: &[BytesN<32>],
    attestors: &[Address],
    step: usize,
) {
    // Pick any attestor pool entry that is NOT the currently stored one
    // -- simulates a stale key post-rotation, or an attacker who
    // controls a never-installed attestor address.
    let wrong_idx = (0..N_ATTESTORS)
        .find(|&i| i != model.attestor)
        .unwrap_or((model.attestor + 1) % N_ATTESTORS);
    let gi = rng.below(N_GH);
    let pr_id = rng.below(N_PR_IDS as usize) as u8;

    let result = client.try_submit_attestation(
        &attestors[wrong_idx],
        &ghs[gi],
        &repo(env),
        &(pr_id as u32),
        &(pr_id as u64),
        &100u32,
        &pr_hash(env, pr_id),
    );
    assert_eq!(
        result,
        Err(Ok(Error::Unauthorized)),
        "step {step}: submit_attestation signed by non-stored attestor must always fail"
    );
    // `model` is passed by shared reference and never mutated here: this
    // action has no valid path to change any state, so the next loop
    // iteration's `assert_scores_match` / `assert_link_consistency`
    // (comparing the unchanged model against the live contract)
    // independently confirms the rejected call credited nobody.
}

fn do_rotate_by_admin(
    env: &Env,
    client: &ProofOwlRegistryClient,
    model: &mut Model,
    rng: &mut Rng,
    attestors: &[Address],
    step: usize,
) {
    let new_idx = rng.below(N_ATTESTORS);
    let admin = client.get_admin().unwrap();
    client.set_attestor(&admin, &attestors[new_idx]);
    assert_eq!(
        client.get_attestor(),
        Some(attestors[new_idx].clone()),
        "step {step}: rotation must take effect immediately"
    );
    let old_idx = model.attestor;
    model.attestor = new_idx;

    // The old attestor, if different, must be rejected on the very
    // next call -- immediate revocation, not eventual.
    if old_idx != new_idx {
        let result = client.try_submit_attestation(
            &attestors[old_idx],
            &gh(env, 0),
            &repo(env),
            &0u32,
            &0u64,
            &100u32,
            &pr_hash(env, 255),
        );
        assert_eq!(
            result,
            Err(Ok(Error::Unauthorized)),
            "step {step}: old attestor must be rejected the instant after rotation"
        );
    }
}

fn do_rotate_by_impostor(
    client: &ProofOwlRegistryClient,
    model: &Model,
    rng: &mut Rng,
    wallets: &[Address],
    attestors: &[Address],
    step: usize,
) {
    // Anyone who isn't the admin -- reuse a wallet address as the
    // impostor "admin" candidate.
    let impostor = &wallets[rng.below(N_WALLETS)];
    let new_idx = rng.below(N_ATTESTORS);
    let before_attestor = client.get_attestor();

    let result = client.try_set_attestor(impostor, &attestors[new_idx]);
    assert_eq!(
        result,
        Err(Ok(Error::Unauthorized)),
        "step {step}: non-admin set_attestor must always fail"
    );
    assert_eq!(
        client.get_attestor(),
        before_attestor,
        "step {step}: rejected rotation must not change the stored attestor"
    );
    assert_eq!(
        client.get_attestor(),
        Some(attestors[model.attestor].clone())
    );
}

fn do_bump_ttl(client: &ProofOwlRegistryClient, rng: &mut Rng, wallets: &[Address], step: usize) {
    let wi = rng.below(N_WALLETS);
    let before_gh = client.get_github_for_wallet(&wallets[wi]);
    let before_score = client.get_reputation_score(&wallets[wi]);
    let before_count = client.get_attestations(&wallets[wi]).len();

    client.bump_wallet_ttl(&wallets[wi]);

    // Permissionless keep-alive: must never create a link, never add
    // reputation, never bypass any authorization -- it changes no
    // observable data at all.
    assert_eq!(
        client.get_github_for_wallet(&wallets[wi]),
        before_gh,
        "step {step}: bump_wallet_ttl changed the link"
    );
    assert_eq!(
        client.get_reputation_score(&wallets[wi]),
        before_score,
        "step {step}: bump_wallet_ttl changed the score"
    );
    assert_eq!(
        client.get_attestations(&wallets[wi]).len(),
        before_count,
        "step {step}: bump_wallet_ttl changed the history length"
    );
}

// --- shared assertions ---------------------------------------------------

fn assert_admin_never_changed(client: &ProofOwlRegistryClient, admin: &Address, step: usize) {
    assert_eq!(
        client.get_admin(),
        Some(admin.clone()),
        "step {step}: admin must never change post-deployment"
    );
}

fn assert_link_consistency(
    client: &ProofOwlRegistryClient,
    model: &Model,
    wallets: &[Address],
    ghs: &[BytesN<32>],
    step: usize,
) {
    for (wi, w) in wallets.iter().enumerate() {
        let expected = model.link[wi].map(|gi| ghs[gi].clone());
        assert_eq!(
            client.get_github_for_wallet(w),
            expected,
            "step {step}: wallet {wi} link mismatch"
        );
    }
    for (gi, g) in ghs.iter().enumerate() {
        let expected = model.reverse[gi].map(|wi| wallets[wi].clone());
        assert_eq!(
            client.get_wallet_for_github(g),
            expected,
            "step {step}: gh {gi} reverse-link mismatch"
        );
    }
}

fn assert_scores_match(
    client: &ProofOwlRegistryClient,
    model: &Model,
    wallets: &[Address],
    step: usize,
) {
    for (wi, w) in wallets.iter().enumerate() {
        assert_eq!(
            client.get_attestations(w).len(),
            model.count[wi],
            "step {step}: wallet {wi} attestation count mismatch"
        );
        assert_eq!(
            client.get_reputation_score(w),
            model.score[wi],
            "step {step}: wallet {wi} score mismatch"
        );
    }
}

fn assert_model_unchanged(before: &Model, after: &Model, step: usize, what: &str) {
    assert_eq!(
        before.link, after.link,
        "step {step}: {what} must not change links"
    );
    assert_eq!(
        before.reverse, after.reverse,
        "step {step}: {what} must not change reverse links"
    );
    assert_eq!(
        before.count, after.count,
        "step {step}: {what} must not change attestation counts"
    );
    assert_eq!(
        before.score, after.score,
        "step {step}: {what} must not change scores"
    );
    assert_eq!(
        before.seen_pr, after.seen_pr,
        "step {step}: {what} must not change seen_pr"
    );
    assert_eq!(
        before.attestor, after.attestor,
        "step {step}: {what} must not change the attestor"
    );
}
