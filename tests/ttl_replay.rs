//! TTL and replay-resistance coverage — v0.2 bounded TTL maintenance.
//!
//! v0.1 had one permissionless keep-alive, `bump_wallet_ttl`, that
//! loaded and refreshed a wallet's entire history in a single call —
//! itself an unbounded operation with the same growth problem that
//! caused the storage ceiling (`docs/security/resource-profile-v1.md`).
//! v0.2 replaces it with two bounded calls
//! (`docs/adr/0004-paginated-attestation-storage.md`):
//!
//! - `bump_wallet_core_ttl(wallet)` — O(1): the wallet link, the GitHub
//!   link it points to, the attestation counter, and the reputation
//!   score.
//! - `bump_attestations_ttl_page(wallet, start, limit) -> Result<u32, Error>`
//!   — O(page): one page of attestation entries and the `SeenPr`
//!   markers they reference.
//!
//! This file proves: every record kind is refreshed correctly by the
//! call responsible for it; a full-history sweep across multiple pages
//! covers every entry, including old ones; duplicate-PR prevention
//! survives that sweep; and TTL maintenance — core or paginated — never
//! changes a link, a count, a score, or any attestation's content.
//!
//! ## Backend/indexer scheduling responsibility (documented here, not
//! enforced by the contract — it cannot force anyone to call these)
//!
//! A service responsible for keeping passports alive must, for every
//! wallet it considers "active" (per `docs/integration/event-indexer-v2.md`
//! §6):
//!
//! 1. Call `bump_wallet_core_ttl(wallet)` — cheap, O(1), safe to do
//!    often.
//! 2. Sweep the wallet's **entire** attestation history in pages:
//!    `start = 0`; call `bump_attestations_ttl_page(wallet, start,
//!    limit)`; while the returned count equals `limit`, advance
//!    `start` by that count and call again; stop once it returns less
//!    than `limit` (including `0`), which means every entry up to the
//!    current `get_attestation_count(wallet)` has been reached.
//! 3. Because new attestations can be submitted between sweep runs, a
//!    full sweep must be re-run periodically (not just once ever) to
//!    also cover entries added after the last sweep — the same
//!    "sweep on a schedule" obligation v0.1 had, just now correctly
//!    bounded per call instead of unbounded per wallet.
//!
//! ## What happens if a page or a `SeenPr` marker is allowed to archive
//!
//! If a page of `AttestationEntry` records (and the `SeenPr` markers
//! they reference) is never refreshed and its TTL reaches zero on a
//! live network, that page's entries are archived: reads that need
//! them (`get_attestation`, `get_attestations_page` for a range
//! overlapping that page) fail until `RestoreFootprint` runs, exactly
//! as for any other archived Soroban entry. Because each page is now
//! independent storage, archival is **partial and localized** — unlike
//! v0.1, where the entire history lived in one entry and archival was
//! all-or-nothing. A `SeenPr` marker archiving does **not** un-spend the
//! PR it guards once restored (`SECURITY.md` §5); it only means a
//! duplicate-submission check for that specific `pr_hash` would fail to
//! read until restored, which is a read-availability gap, not a
//! dedup-safety gap — restoring the entry returns exactly the same
//! "already seen" marker it always held.
//!
//! ## What cannot be fully emulated locally (documented, not skipped)
//!
//! As in Phase 4: the in-process Soroban `Env` tracks and reports TTL
//! decay realistically but does not reproduce a live network's
//! archival-on-expiry read failure. Every test below proves correct
//! *policy*; the archival failure mode itself is documented above and
//! in `docs/security/threat-model-v1.md` §9, not reproduced in a unit
//! test.

use proofowl_contracts::{DataKey, Error, ProofOwlRegistry, ProofOwlRegistryClient};
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

fn advance_past_threshold(env: &Env) {
    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 10);
}

// ---------------------------------------------------------------------------
// 1. Every record kind is refreshed by the call responsible for it, at
//    a realistic history size.
// ---------------------------------------------------------------------------

#[test]
fn submit_attestation_extends_every_record_it_writes() {
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
        assert!(p.get_ttl(&DataKey::AttestationCount(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::ReputationScore(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        for (i, pr) in prs.iter().enumerate() {
            assert!(
                p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), i as u32))
                    >= REGISTRY_TTL_THRESHOLD,
                "entry {i} must be extended on write"
            );
            assert!(
                p.get_ttl(&DataKey::SeenPr(pr.clone())) >= REGISTRY_TTL_THRESHOLD,
                "SeenPr for entry {i} must be extended on write"
            );
        }
        assert!(env.storage().instance().get_ttl() >= REGISTRY_TTL_THRESHOLD);
    });
}

// ---------------------------------------------------------------------------
// 2. `bump_wallet_core_ttl` refreshes exactly the O(1) records, and only
//    those.
// ---------------------------------------------------------------------------

#[test]
fn bump_wallet_core_ttl_refreshes_o1_records_but_not_attestation_entries() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 2);
    client.link_github(&wallet, &attestor, &gh);
    let pr = hash(&env, 60);
    client.submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &100u32, &pr);

    advance_past_threshold(&env);
    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::WalletLink(wallet.clone())) < REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), 0)) < REGISTRY_TTL_THRESHOLD);
    });

    client.bump_wallet_core_ttl(&wallet);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::WalletLink(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::GithubLink(gh.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::AttestationCount(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::ReputationScore(wallet.clone())) >= REGISTRY_TTL_THRESHOLD);
        // The attestation entry and its SeenPr marker are NOT this
        // call's responsibility -- still cold, exactly as documented.
        assert!(
            p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), 0)) < REGISTRY_TTL_THRESHOLD,
            "bump_wallet_core_ttl must not touch attestation entries"
        );
        assert!(
            p.get_ttl(&DataKey::SeenPr(pr.clone())) < REGISTRY_TTL_THRESHOLD,
            "bump_wallet_core_ttl must not touch SeenPr markers"
        );
    });
}

#[test]
fn bump_wallet_core_ttl_needs_no_authorization_and_is_a_noop_for_a_fresh_wallet() {
    let env = Env::default();
    env.ledger().set_timestamp(TS);
    env.ledger().set_sequence_number(100);
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let contract_id = env.register(ProofOwlRegistry, (admin, attestor));
    let client = ProofOwlRegistryClient::new(&env, &contract_id);

    let unlinked_wallet = Address::generate(&env);
    env.mock_auths(&[]); // no signatures at all -- permissionless
    client.bump_wallet_core_ttl(&unlinked_wallet);

    assert_eq!(client.get_github_for_wallet(&unlinked_wallet), None);
    assert_eq!(client.get_attestation_count(&unlinked_wallet), 0);
    assert_eq!(client.get_reputation_score(&unlinked_wallet), 0);
}

// ---------------------------------------------------------------------------
// 3. `bump_attestations_ttl_page` refreshes exactly its page, across
//    multiple pages, including old ones a sweep only reaches later.
// ---------------------------------------------------------------------------

#[test]
fn bump_attestations_ttl_page_covers_multiple_pages_including_old_ones() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 3);
    client.link_github(&wallet, &attestor, &gh);

    let prs: std::vec::Vec<BytesN<32>> = (0..7u8).map(|i| hash(&env, 70 + i)).collect();
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

    advance_past_threshold(&env);
    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        for i in 0..7u32 {
            assert!(
                p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), i)) < REGISTRY_TTL_THRESHOLD,
                "entry {i} must have decayed before any sweep"
            );
        }
    });

    // Sweep in pages of 3: [0,3), [3,6), [6,9) -- the last page is
    // short (only entry 6 exists).
    let r0 = client.bump_attestations_ttl_page(&wallet, &0u32, &3u32);
    assert_eq!(r0, 3);
    let r1 = client.bump_attestations_ttl_page(&wallet, &3u32, &3u32);
    assert_eq!(r1, 3);
    let r2 = client.bump_attestations_ttl_page(&wallet, &6u32, &3u32);
    assert_eq!(
        r2, 1,
        "short last page must report exactly what it refreshed"
    );

    // Every entry, including the ones from the FIRST page (the
    // "old" page by the time the sweep finished), is now warm.
    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        for (i, pr) in prs.iter().enumerate() {
            assert!(
                p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), i as u32))
                    >= REGISTRY_TTL_THRESHOLD,
                "entry {i} must be refreshed after the full sweep"
            );
            assert!(
                p.get_ttl(&DataKey::SeenPr(pr.clone())) >= REGISTRY_TTL_THRESHOLD,
                "SeenPr for entry {i} must be refreshed after the full sweep"
            );
        }
    });

    // A page starting exactly at the count refreshes nothing and says
    // so -- the sweep's own "I've reached the end" signal.
    assert_eq!(client.bump_attestations_ttl_page(&wallet, &7u32, &3u32), 0);
}

#[test]
fn bump_attestations_ttl_page_rejects_bad_limits_and_out_of_range_start() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 4);
    client.link_github(&wallet, &attestor, &gh);
    client.submit_attestation(
        &attestor,
        &gh,
        &repo(&env),
        &1u32,
        &1u64,
        &100u32,
        &hash(&env, 80),
    );

    assert_eq!(
        client.try_bump_attestations_ttl_page(&wallet, &0u32, &0u32),
        Err(Ok(Error::InvalidPageLimit))
    );
    assert_eq!(
        client.try_bump_attestations_ttl_page(&wallet, &0u32, &51u32),
        Err(Ok(Error::PageLimitExceeded))
    );
    assert_eq!(
        client.try_bump_attestations_ttl_page(&wallet, &2u32, &10u32),
        Err(Ok(Error::PageStartOutOfRange))
    );
    // start == count (1) is valid, refreshes nothing.
    assert_eq!(client.bump_attestations_ttl_page(&wallet, &1u32, &10u32), 0);
}

// ---------------------------------------------------------------------------
// 4. Duplicate-PR prevention survives a bounded, paginated sweep,
//    across several sweep cycles.
// ---------------------------------------------------------------------------

#[test]
fn duplicate_pr_rejection_survives_paginated_sweeps_across_cycles() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 5);
    client.link_github(&wallet, &attestor, &gh);

    let prs: std::vec::Vec<BytesN<32>> = (0..5u8).map(|i| hash(&env, 90 + i)).collect();
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

    let mut seq = 100u32;
    for cycle in 0..3 {
        seq += REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 10;
        env.ledger().set_sequence_number(seq);

        // A full bounded sweep, small pages.
        let mut start = 0u32;
        loop {
            let refreshed = client.bump_attestations_ttl_page(&wallet, &start, &2u32);
            if refreshed < 2 {
                break;
            }
            start += refreshed;
        }

        for (i, pr) in prs.iter().enumerate() {
            assert_eq!(
                client.try_submit_attestation(
                    &attestor,
                    &gh,
                    &repo(&env),
                    &(i as u32),
                    &(i as u64),
                    &150u32,
                    pr
                ),
                Err(Ok(Error::DuplicateAttestation)),
                "cycle {cycle}, pr {i}: kept-alive PR must stay permanently spent"
            );
        }
    }
    // No rejected resubmission attempt above ever got through.
    assert_eq!(client.get_attestation_count(&wallet), 5);
}

// ---------------------------------------------------------------------------
// 5. After `unlink_github`, the originating wallet's history is still
//    fully coverable by a paginated sweep -- unlink removes the link
//    records only, never history coverage.
// ---------------------------------------------------------------------------

#[test]
fn paginated_sweep_still_covers_history_after_unlink() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 6);
    client.link_github(&wallet, &attestor, &gh);
    let pr = hash(&env, 44);
    client.submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &100u32, &pr);

    client.unlink_github(&wallet, &attestor, &gh);

    advance_past_threshold(&env);
    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), 0)) < REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::SeenPr(pr.clone())) < REGISTRY_TTL_THRESHOLD);
    });

    // bump_attestations_ttl_page keys directly off the wallet address
    // (not off any current link), so it works identically whether or
    // not the wallet is currently linked.
    let refreshed = client.bump_attestations_ttl_page(&wallet, &0u32, &10u32);
    assert_eq!(refreshed, 1);

    env.as_contract(&client.address, || {
        let p = env.storage().persistent();
        assert!(p.get_ttl(&DataKey::AttestationEntry(wallet.clone(), 0)) >= REGISTRY_TTL_THRESHOLD);
        assert!(p.get_ttl(&DataKey::SeenPr(pr.clone())) >= REGISTRY_TTL_THRESHOLD);
    });

    assert_eq!(client.get_attestation_count(&wallet), 1);

    // Re-link the identity to a fresh wallet and confirm the spent PR
    // stays spent regardless of which wallet attempts it.
    let new_wallet = Address::generate(&env);
    client.link_github(&new_wallet, &attestor, &gh);
    assert_eq!(
        client.try_submit_attestation(&attestor, &gh, &repo(&env), &1u32, &1u64, &100u32, &pr),
        Err(Ok(Error::DuplicateAttestation))
    );
    assert!(client.get_attestation_count(&new_wallet) == 0);
}

// ---------------------------------------------------------------------------
// 6. TTL maintenance -- core or paginated -- never changes links, count,
//    score, or any attestation's content. (Complements the same
//    assertion made inline in `tests/state_machine.rs` across a long
//    hostile sequence; this file checks it in isolation with precise
//    before/after snapshots.)
// ---------------------------------------------------------------------------

#[test]
fn ttl_maintenance_changes_no_data_of_any_kind() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 8);
    client.link_github(&wallet, &attestor, &gh);
    for i in 0..5u32 {
        client.submit_attestation(
            &attestor,
            &gh,
            &repo(&env),
            &i,
            &(i as u64),
            &100u32,
            &hash(&env, 200 + i as u8),
        );
    }

    let link_before = client.get_github_for_wallet(&wallet);
    let count_before = client.get_attestation_count(&wallet);
    let score_before = client.get_reputation_score(&wallet);
    let entries_before: std::vec::Vec<_> = (0..count_before)
        .map(|seq| client.get_attestation(&wallet, &seq))
        .collect();

    advance_past_threshold(&env);
    client.bump_wallet_core_ttl(&wallet);
    client.bump_attestations_ttl_page(&wallet, &0u32, &3u32);
    client.bump_attestations_ttl_page(&wallet, &3u32, &3u32);

    assert_eq!(client.get_github_for_wallet(&wallet), link_before);
    assert_eq!(client.get_attestation_count(&wallet), count_before);
    assert_eq!(client.get_reputation_score(&wallet), score_before);
    for seq in 0..count_before {
        assert_eq!(
            client.get_attestation(&wallet, &seq),
            entries_before[seq as usize]
        );
    }
}

// ---------------------------------------------------------------------------
// 7. Bump threshold boundary is exact (mirrors the v0.1 baseline check,
//    now against the v0.2 keys).
// ---------------------------------------------------------------------------

#[test]
fn bump_threshold_boundary_is_exact_for_core_ttl() {
    let (env, client, _admin, attestor) = setup();
    let wallet = Address::generate(&env);
    let gh = hash(&env, 9);
    client.link_github(&wallet, &attestor, &gh);

    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD - 1);
    env.as_contract(&client.address, || {
        let ttl = env
            .storage()
            .persistent()
            .get_ttl(&DataKey::WalletLink(wallet.clone()));
        assert!(
            ttl >= REGISTRY_TTL_THRESHOLD,
            "one ledger before threshold, got {ttl}"
        );
    });

    env.ledger()
        .set_sequence_number(100 + REGISTRY_TTL_EXTEND_TO - REGISTRY_TTL_THRESHOLD + 1);
    env.as_contract(&client.address, || {
        let ttl = env
            .storage()
            .persistent()
            .get_ttl(&DataKey::WalletLink(wallet.clone()));
        assert!(
            ttl < REGISTRY_TTL_THRESHOLD,
            "one ledger after threshold, got {ttl}"
        );
    });

    client.bump_wallet_core_ttl(&wallet);
    env.as_contract(&client.address, || {
        assert!(
            env.storage()
                .persistent()
                .get_ttl(&DataKey::WalletLink(wallet.clone()))
                >= REGISTRY_TTL_EXTEND_TO - 1
        );
    });
}
