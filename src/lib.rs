#![no_std]

//! ProofOwl registry contract.
//!
//! Two jobs, on-chain:
//!
//! 1. Hold a **two-party** link between a Stellar wallet and a GitHub
//!    identity. The wallet proves it controls the key by signing the
//!    linking call, and the trusted attestor co-signs to assert that an
//!    off-chain GitHub OAuth / challenge flow proved the same caller
//!    controls that GitHub account. Neither signature alone creates a
//!    link.
//! 2. Hold verified attestations submitted by the trusted attestor
//!    service, each one representing one confirmed, merged contribution
//!    to a Stellar Wave-labeled issue in an approved repo.
//!
//! ## Trust boundaries (read before changing anything)
//!
//! * **The contract cannot verify GitHub OAuth itself.** It has no
//!   network access and no way to check a GitHub token. What it enforces
//!   is *procedure*: a link exists only if the wallet signed **and** the
//!   attestor signed. The attestor is trusted to co-sign only after the
//!   future backend has run a real GitHub OAuth / challenge flow proving
//!   the wallet holder controls the GitHub identity behind
//!   `github_id_hash`. See `docs/adr/0002-two-party-github-link.md`.
//! * **The attestor resolves the wallet, it never chooses it.**
//!   `submit_attestation` takes a `github_id_hash`, not a wallet address;
//!   the contract looks up the wallet via the on-chain link. A
//!   compromised attestor key can forge *what* happened or misreport
//!   complexity, but cannot redirect credit to a wallet the GitHub
//!   identity has not itself linked. See
//!   `docs/adr/0001-attestor-resolves-via-github-link.md`.
//! * **Identity squatting is blocked by the attestor co-signature.** A
//!   wallet cannot claim `hash("torvalds")` on its own — the attestor
//!   will not co-sign a link the OAuth flow did not back.
//!
//! ## Initialization
//!
//! There is **no `init` entrypoint**. Configuration is set by
//! [`ProofOwlRegistry::__constructor`], which the host runs exactly once,
//! atomically, inside the `CreateContract` operation that deploys the
//! instance. There is therefore nothing to front-run: a race to
//! "initialize first" would only ever create a *different* contract
//! instance. The constructor additionally calls `admin.require_auth()`,
//! so the deploy transaction must carry the admin's signature.
//!
//! ## Storage durability
//!
//! Every long-lived record (wallet links, GitHub links, PR-dedup
//! markers, attestation entries, and the instance itself) has its TTL
//! extended on every write and on every read-write path that touches it.
//! See `SECURITY.md` for the exact policy and its rationale, and
//! `docs/adr/0004-paginated-attestation-storage.md` §"Bounded TTL
//! maintenance" for how a wallet's growing history is kept alive without
//! ever loading it all in one call.
//!
//! ## v0.2: bounded, per-record attestation storage
//!
//! v0.1 stored a wallet's entire attestation history in one
//! `Vec<Attestation>` under a single persistent entry. That entry hit
//! Soroban's 65,536-byte per-entry limit at ~286 attestations, after
//! which the wallet could never receive another attestation or TTL
//! refresh — see `docs/security/resource-profile-v1.md` and
//! `docs/adr/0004-paginated-attestation-storage.md`.
//!
//! v0.2 replaces that with one persistent entry per attestation,
//! addressed by a zero-based sequence number, plus a small counter:
//!
//! * [`DataKey::AttestationEntry`]`(wallet, seq)` — one [`Attestation`],
//!   `seq` in `0..count`.
//! * [`DataKey::AttestationCount`]`(wallet)` — how many attestations
//!   this wallet has, and the next `seq` to use.
//!
//! No single entry's size depends on history length any more, so the
//! v0.1 ceiling cannot recur by construction. Reads are bounded and
//! paginated ([`ProofOwlRegistry::get_attestation_count`],
//! [`ProofOwlRegistry::get_attestation`],
//! [`ProofOwlRegistry::get_attestations_page`]) in place of v0.1's
//! unbounded `get_attestations`, [`ProofOwlRegistry::get_reputation_score`]
//! is O(1) against a running [`DataKey::ReputationScore`] counter that
//! `submit_attestation` updates atomically, and TTL maintenance is split
//! into an O(1) call for a wallet's small fixed-size records
//! ([`ProofOwlRegistry::bump_wallet_core_ttl`]) and a bounded, paginated
//! sweep for its attestation entries
//! ([`ProofOwlRegistry::bump_attestations_ttl_page`]) in place of
//! v0.1's unbounded `bump_wallet_ttl`. See
//! `docs/adr/0004-paginated-attestation-storage.md` and
//! `docs/migrations/v0.1-to-v0.2.md`.

use soroban_sdk::{
    contract, contracterror, contractevent, contractimpl, contracttype, Address, BytesN, Env,
    IntoVal, String, Val, Vec,
};

/// One recorded contribution.
///
/// `pr_hash` is the global de-duplication key and is treated as an
/// opaque 32-byte value. The backend MUST derive it canonically as:
///
/// ```text
/// pr_hash = SHA-256(  lowercase("github.com/<owner>/<repo>/pull/<number>")  )
/// ```
///
/// with no scheme, no trailing slash, and no query string, so that the
/// same PR always hashes to the same value regardless of how the URL was
/// captured. `repo` (`"<owner>/<repo>"`) and `pr_number` are stored in
/// the clear so an indexer or frontend can reconstruct that URL and link
/// straight to the pull request; `pr_hash` alone is not reversible.
#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attestation {
    /// `"<owner>/<repo>"`, e.g. `"stellar/soroban-examples"`.
    pub repo: String,
    /// GitHub pull-request number.
    pub pr_number: u32,
    /// Stellar Wave issue id this contribution resolved.
    pub issue_id: u64,
    /// Wave complexity tier in points. One of `0`, `100`, `150`, `200`.
    /// `0` means the attestor confirmed the contribution happened but
    /// could not confirm its official tier — see
    /// [`ProofOwlRegistry::get_reputation_score`] for how that is scored.
    pub complexity: u32,
    /// SHA-256 of the normalized PR URL — see the type docs.
    pub pr_hash: BytesN<32>,
    /// Ledger timestamp (Unix seconds) at which the attestation was
    /// recorded on-chain. Set by the contract, not the caller.
    pub timestamp: u64,
}

#[contracttype]
pub enum DataKey {
    /// Admin address (instance storage).
    Admin,
    /// Attestor address (instance storage).
    Attestor,
    /// `wallet -> github_id_hash`.
    WalletLink(Address),
    /// `github_id_hash -> wallet`.
    GithubLink(BytesN<32>),
    /// `(wallet, sequence) -> Attestation`. One persistent entry per
    /// attestation, `sequence` zero-based — see
    /// `docs/adr/0004-paginated-attestation-storage.md`.
    AttestationEntry(Address, u32),
    /// `wallet -> u32`: how many attestations this wallet has, and the
    /// next `sequence` to write at.
    AttestationCount(Address),
    /// `wallet -> u32`: running reputation score, updated atomically by
    /// `submit_attestation`. Never re-derived from a full scan — see
    /// `docs/adr/0004-paginated-attestation-storage.md`.
    ReputationScore(Address),
    /// `pr_hash -> ()` global duplicate-PR guard.
    SeenPr(BytesN<32>),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    /// Reserved. Kept for numbering stability; unreachable now that
    /// setup is a one-shot constructor with no `init` entrypoint.
    AlreadyInitialized = 1,
    /// The instance config is missing (e.g. archived). Practically
    /// unreachable while the instance entry is alive — see the TTL
    /// policy in `SECURITY.md`.
    NotInitialized = 2,
    Unauthorized = 3,
    WalletAlreadyLinked = 4,
    GithubAlreadyLinked = 5,
    DuplicateAttestation = 6,
    WalletNotLinked = 7,
    /// `complexity` was not one of the accepted tier values.
    InvalidComplexity = 8,
    /// `unlink_github` was called for a (wallet, github_id_hash) pair
    /// that is not an existing, consistent link.
    LinkNotFound = 9,
    /// A paginated call's `limit` was `0`. v0.2.
    InvalidPageLimit = 10,
    /// A paginated call's `limit` exceeded [`MAX_PAGE_SIZE`]. v0.2.
    PageLimitExceeded = 11,
    /// [`ProofOwlRegistry::get_attestation`]'s `sequence` was `>=` the
    /// wallet's attestation count. v0.2.
    SequenceOutOfRange = 12,
    /// A paginated call's `start` was `>` the wallet's attestation
    /// count. `start == count` is valid (yields an empty page) — see
    /// [`ProofOwlRegistry::get_attestations_page`]. v0.2.
    PageStartOutOfRange = 13,
}

/// Score credited for an attestation whose complexity tier the attestor
/// could not independently confirm (`complexity == 0`). A confirmed
/// contribution with an unknown tier still counts for something, but
/// less than the lowest real tier.
const UNVERIFIED_COMPLEXITY_SCORE: u32 = 50;

/// The only `complexity` values `submit_attestation` accepts. `0` is the
/// "confirmed, tier unknown" sentinel; the rest are the Stellar Wave
/// point tiers.
const ALLOWED_COMPLEXITY: [u32; 4] = [0, 100, 150, 200];

/// The largest `limit` accepted by any paginated call
/// (`get_attestations_page`, `bump_attestations_ttl_page`).
///
/// Chosen as a small, fixed bound so a page's read/write cost and
/// response size stay flat regardless of how large a wallet's total
/// history grows — the exact property v0.1's unbounded
/// `get_attestations` / `bump_wallet_ttl` did not have. 50 is large
/// enough to be a useful page for a UI or an indexer sweep and small
/// enough that even a full page of the largest realistic `Attestation`
/// (long `repo` string) stays a tiny fraction of any per-call resource
/// limit — measured in `docs/security/resource-profile-v2.md`. See
/// `docs/adr/0004-paginated-attestation-storage.md`.
const MAX_PAGE_SIZE: u32 = 50;

// --- TTL policy -------------------------------------------------------------
//
// Soroban archives a persistent entry once its TTL (ledgers remaining)
// hits zero; reading an archived entry fails until it is restored. Every
// registry record here is meant to live indefinitely, so every write and
// every read-write path re-extends the entries it touches, and anyone
// can call a keep-alive to warm a wallet's records for free.
//
// At the ~5s mainnet ledger cadence: 1 day ~= 17_280 ledgers.
//   * extend target : 120 days  (well under the ~180-day mainnet cap, so
//                                no silent clamp on mainnet or testnet)
//   * threshold      : 90 days   (entries with >90 days left are left
//                                alone, so bumps stay cheap)
// Both are clamped to `env.storage().max_ttl()` defensively.

const LEDGERS_PER_DAY: u32 = 17_280;
const REGISTRY_TTL_EXTEND_TO: u32 = 120 * LEDGERS_PER_DAY;
const REGISTRY_TTL_THRESHOLD: u32 = 90 * LEDGERS_PER_DAY;

fn extend_persistent<K: IntoVal<Env, Val>>(env: &Env, key: &K) {
    let max = env.storage().max_ttl();
    let extend_to = REGISTRY_TTL_EXTEND_TO.min(max);
    let threshold = REGISTRY_TTL_THRESHOLD.min(extend_to);
    env.storage()
        .persistent()
        .extend_ttl(key, threshold, extend_to);
}

fn extend_instance(env: &Env) {
    let max = env.storage().max_ttl();
    let extend_to = REGISTRY_TTL_EXTEND_TO.min(max);
    let threshold = REGISTRY_TTL_THRESHOLD.min(extend_to);
    env.storage().instance().extend_ttl(threshold, extend_to);
}

/// Read `AttestationCount(wallet)`, defaulting to `0` for a wallet with
/// no attestations yet.
fn attestation_count(env: &Env, wallet: &Address) -> u32 {
    env.storage()
        .persistent()
        .get(&DataKey::AttestationCount(wallet.clone()))
        .unwrap_or(0)
}

// --- Events ---------------------------------------------------------------

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Initialized {
    #[topic]
    pub admin: Address,
    pub attestor: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestorRotated {
    #[topic]
    pub admin: Address,
    pub new_attestor: Address,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GithubLinked {
    #[topic]
    pub wallet: Address,
    pub github_id_hash: BytesN<32>,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GithubUnlinked {
    #[topic]
    pub wallet: Address,
    pub github_id_hash: BytesN<32>,
}

#[contractevent]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AttestationRecorded {
    #[topic]
    pub wallet: Address,
    pub repo: String,
    pub pr_number: u32,
    pub issue_id: u64,
    pub complexity: u32,
    pub pr_hash: BytesN<32>,
    pub timestamp: u64,
    /// Zero-based index of this attestation in `wallet`'s history —
    /// the same `sequence` [`ProofOwlRegistry::get_attestation`] and
    /// [`ProofOwlRegistry::get_attestations_page`] address it by. New
    /// in v0.2; an indexer building a passport from events alone can
    /// use it to detect gaps or reorderings without a separate
    /// `get_attestation_count` round-trip. See
    /// `docs/integration/event-indexer-v2.md`.
    pub sequence: u32,
}

#[contract]
pub struct ProofOwlRegistry;

#[contractimpl]
impl ProofOwlRegistry {
    /// Deploy-time setup. The host calls this exactly once, atomically,
    /// as part of the `CreateContract` operation that creates the
    /// instance — there is no separate `init` call and therefore no
    /// initialization race to front-run.
    ///
    /// `admin.require_auth()` means the deploy transaction must carry the
    /// admin's signature, binding the configuration to a
    /// deployer-authorized setup rather than to whoever calls first.
    ///
    /// `attestor` is the key allowed to submit attestations and to
    /// co-sign identity links; rotate it later with [`Self::set_attestor`]
    /// without redeploying.
    pub fn __constructor(env: Env, admin: Address, attestor: Address) {
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Attestor, &attestor);
        extend_instance(&env);

        Initialized { admin, attestor }.publish(&env);
    }

    /// Rotate the attestor key. Admin-only. The intended path to
    /// decentralize off a single trusted key later (multisig or a
    /// threshold attestor contract) without a migration.
    pub fn set_attestor(env: Env, admin: Address, new_attestor: Address) -> Result<(), Error> {
        admin.require_auth();
        let stored_admin: Address = env
            .storage()
            .instance()
            .get(&DataKey::Admin)
            .ok_or(Error::NotInitialized)?;
        if admin != stored_admin {
            return Err(Error::Unauthorized);
        }

        env.storage()
            .instance()
            .set(&DataKey::Attestor, &new_attestor);
        extend_instance(&env);

        AttestorRotated {
            admin,
            new_attestor,
        }
        .publish(&env);

        Ok(())
    }

    /// Link a wallet to a hashed GitHub identity. **Two-party**: requires
    /// the signatures of both `wallet` and the trusted `attestor`.
    ///
    /// * The wallet signature proves control of the Stellar key.
    /// * The attestor signature attests that the off-chain GitHub OAuth /
    ///   challenge flow proved the same person controls the GitHub
    ///   account behind `github_id_hash`. The contract cannot and does
    ///   not verify GitHub itself.
    ///
    /// One wallet <-> one GitHub identity, enforced in both directions. A
    /// mistaken link is undone with [`Self::unlink_github`] (also
    /// two-party); there is intentionally no admin override that could
    /// silently move a link.
    pub fn link_github(
        env: Env,
        wallet: Address,
        attestor: Address,
        github_id_hash: BytesN<32>,
    ) -> Result<(), Error> {
        wallet.require_auth();
        attestor.require_auth();
        Self::check_attestor(&env, &attestor)?;

        let wallet_key = DataKey::WalletLink(wallet.clone());
        let github_key = DataKey::GithubLink(github_id_hash.clone());

        if env.storage().persistent().has(&wallet_key) {
            return Err(Error::WalletAlreadyLinked);
        }
        if env.storage().persistent().has(&github_key) {
            return Err(Error::GithubAlreadyLinked);
        }

        env.storage().persistent().set(&wallet_key, &github_id_hash);
        env.storage().persistent().set(&github_key, &wallet);
        extend_persistent(&env, &wallet_key);
        extend_persistent(&env, &github_key);
        extend_instance(&env);

        GithubLinked {
            wallet,
            github_id_hash,
        }
        .publish(&env);

        Ok(())
    }

    /// Undo a link. **Two-party**: requires the signatures of both the
    /// currently linked `wallet` and the trusted `attestor`. Used to fix
    /// a mistaken link (wrong GitHub identity hash) or to release an
    /// identity so it can be re-linked to a different wallet after the
    /// owner re-runs the off-chain GitHub verification.
    ///
    /// Both link records are removed. The wallet's attestation history
    /// and the global PR-dedup markers are left untouched: a merged PR
    /// stays spent forever, and reputation already earned stays attached
    /// to the wallet that earned it. Migrating a history to a fresh
    /// wallet is out of scope — see `SECURITY.md`.
    pub fn unlink_github(
        env: Env,
        wallet: Address,
        attestor: Address,
        github_id_hash: BytesN<32>,
    ) -> Result<(), Error> {
        wallet.require_auth();
        attestor.require_auth();
        Self::check_attestor(&env, &attestor)?;

        let wallet_key = DataKey::WalletLink(wallet.clone());
        let github_key = DataKey::GithubLink(github_id_hash.clone());

        // Both directions must exist and agree, otherwise this is not a
        // real link and we refuse rather than delete something adjacent.
        let linked_hash: Option<BytesN<32>> = env.storage().persistent().get(&wallet_key);
        let linked_wallet: Option<Address> = env.storage().persistent().get(&github_key);
        if linked_hash.as_ref() != Some(&github_id_hash) || linked_wallet.as_ref() != Some(&wallet)
        {
            return Err(Error::LinkNotFound);
        }

        env.storage().persistent().remove(&wallet_key);
        env.storage().persistent().remove(&github_key);
        extend_instance(&env);

        GithubUnlinked {
            wallet,
            github_id_hash,
        }
        .publish(&env);

        Ok(())
    }

    /// Record a verified contribution. Attestor-only. Resolves the wallet
    /// from the on-chain GitHub link rather than trusting the caller to
    /// supply one — see module docs and ADR 0001.
    ///
    /// `complexity` must be one of `0`, `100`, `150`, `200`
    /// ([`Error::InvalidComplexity`] otherwise). `pr_hash` is the global
    /// duplicate guard and must be derived canonically — see
    /// [`Attestation`]. The on-chain `timestamp` is the ledger time, not
    /// a caller-supplied value.
    ///
    /// Stored as one new persistent entry
    /// (`AttestationEntry(wallet, seq)`) rather than appended to a
    /// growing vector — see
    /// `docs/adr/0004-paginated-attestation-storage.md`.
    // Each argument is a distinct on-chain fact the backend must pass
    // explicitly; a wrapper struct would only rename the same fields.
    #[allow(clippy::too_many_arguments)]
    pub fn submit_attestation(
        env: Env,
        attestor: Address,
        github_id_hash: BytesN<32>,
        repo: String,
        pr_number: u32,
        issue_id: u64,
        complexity: u32,
        pr_hash: BytesN<32>,
    ) -> Result<Address, Error> {
        attestor.require_auth();
        Self::check_attestor(&env, &attestor)?;

        if !ALLOWED_COMPLEXITY.contains(&complexity) {
            return Err(Error::InvalidComplexity);
        }

        let github_key = DataKey::GithubLink(github_id_hash);
        let wallet: Address = env
            .storage()
            .persistent()
            .get(&github_key)
            .ok_or(Error::WalletNotLinked)?;

        let pr_key = DataKey::SeenPr(pr_hash.clone());
        if env.storage().persistent().has(&pr_key) {
            return Err(Error::DuplicateAttestation);
        }

        let timestamp = env.ledger().timestamp();
        let attestation = Attestation {
            repo: repo.clone(),
            pr_number,
            issue_id,
            complexity,
            pr_hash: pr_hash.clone(),
            timestamp,
        };

        let seq = attestation_count(&env, &wallet);
        let entry_key = DataKey::AttestationEntry(wallet.clone(), seq);
        let count_key = DataKey::AttestationCount(wallet.clone());
        let score_key = DataKey::ReputationScore(wallet.clone());

        let points = if complexity == 0 {
            UNVERIFIED_COMPLEXITY_SCORE
        } else {
            complexity
        };
        let current_score: u32 = env.storage().persistent().get(&score_key).unwrap_or(0);
        let new_score = current_score.saturating_add(points);

        env.storage().persistent().set(&entry_key, &attestation);
        env.storage().persistent().set(&count_key, &(seq + 1));
        env.storage().persistent().set(&score_key, &new_score);
        env.storage().persistent().set(&pr_key, &());

        // Keep every record this contribution depends on warm.
        extend_persistent(&env, &entry_key);
        extend_persistent(&env, &count_key);
        extend_persistent(&env, &score_key);
        extend_persistent(&env, &pr_key);
        extend_persistent(&env, &github_key);
        extend_persistent(&env, &DataKey::WalletLink(wallet.clone()));
        extend_instance(&env);

        AttestationRecorded {
            wallet: wallet.clone(),
            repo,
            pr_number,
            issue_id,
            complexity,
            pr_hash,
            timestamp,
            sequence: seq,
        }
        .publish(&env);

        Ok(wallet)
    }

    /// Permissionless keep-alive for the O(1) records tied to a wallet:
    /// the wallet link, the GitHub link it points at, the attestation
    /// counter, and the reputation score. Does **not** touch individual
    /// attestation entries or their `SeenPr` markers — those are swept
    /// separately, in bounded pages, by
    /// [`Self::bump_attestations_ttl_page`].
    ///
    /// Anyone can call it (a frontend "keep my passport alive" button, a
    /// cron job). It only pushes out archival and never changes data.
    /// No-op for a wallet with no link and no history. Cost is constant
    /// regardless of history size — see
    /// `docs/adr/0004-paginated-attestation-storage.md`.
    pub fn bump_wallet_core_ttl(env: Env, wallet: Address) {
        let wallet_key = DataKey::WalletLink(wallet.clone());
        if let Some(github_id_hash) = env.storage().persistent().get::<_, BytesN<32>>(&wallet_key) {
            extend_persistent(&env, &wallet_key);
            extend_persistent(&env, &DataKey::GithubLink(github_id_hash));
        }

        let count_key = DataKey::AttestationCount(wallet.clone());
        if env.storage().persistent().has(&count_key) {
            extend_persistent(&env, &count_key);
        }

        let score_key = DataKey::ReputationScore(wallet);
        if env.storage().persistent().has(&score_key) {
            extend_persistent(&env, &score_key);
        }

        extend_instance(&env);
    }

    /// Permissionless, bounded keep-alive for one page of a wallet's
    /// attestation history: extends the TTL of each
    /// `AttestationEntry(wallet, seq)` in `[start, start+limit)` and the
    /// `SeenPr` marker its `pr_hash` points at, so a merged PR can never
    /// become re-submittable just because its marker was allowed to
    /// expire — regardless of which page it happens to be on.
    ///
    /// Same `limit`/`start` rules as
    /// [`Self::get_attestations_page`] ([`Error::InvalidPageLimit`],
    /// [`Error::PageLimitExceeded`], [`Error::PageStartOutOfRange`]).
    /// Returns the number of entries actually refreshed: a caller
    /// sweeping a wallet's full history calls this repeatedly with an
    /// advancing `start`, and a return value less than `limit`
    /// (including `0`) signals the sweep has reached the end. Changes no
    /// data — see `docs/security/resource-profile-v2.md` for the backend
    /// scheduling responsibility this implies, and
    /// `docs/adr/0004-paginated-attestation-storage.md` for why this is
    /// a separate call from [`Self::bump_wallet_core_ttl`].
    pub fn bump_attestations_ttl_page(
        env: Env,
        wallet: Address,
        start: u32,
        limit: u32,
    ) -> Result<u32, Error> {
        Self::check_page_limit(limit)?;
        let count = attestation_count(&env, &wallet);
        if start > count {
            return Err(Error::PageStartOutOfRange);
        }

        let end = start.saturating_add(limit).min(count);
        let mut refreshed = 0u32;
        for seq in start..end {
            let entry_key = DataKey::AttestationEntry(wallet.clone(), seq);
            if let Some(entry) = env.storage().persistent().get::<_, Attestation>(&entry_key) {
                extend_persistent(&env, &entry_key);
                extend_persistent(&env, &DataKey::SeenPr(entry.pr_hash));
                refreshed += 1;
            }
        }
        extend_instance(&env);
        Ok(refreshed)
    }

    /// How many attestations `wallet` has. `0` for an unknown or
    /// never-attested wallet. Also the next `sequence`
    /// [`Self::get_attestation`] will return once one more attestation
    /// is submitted.
    pub fn get_attestation_count(env: Env, wallet: Address) -> u32 {
        attestation_count(&env, &wallet)
    }

    /// A single attestation by its zero-based `sequence` (`0` = the
    /// first ever recorded for `wallet`). [`Error::SequenceOutOfRange`]
    /// if `sequence >= get_attestation_count(wallet)`.
    pub fn get_attestation(env: Env, wallet: Address, sequence: u32) -> Result<Attestation, Error> {
        env.storage()
            .persistent()
            .get(&DataKey::AttestationEntry(wallet, sequence))
            .ok_or(Error::SequenceOutOfRange)
    }

    /// A bounded page of `wallet`'s attestation history, oldest first,
    /// starting at zero-based index `start`.
    ///
    /// `limit` must be in `1..=`[`MAX_PAGE_SIZE`]
    /// ([`Error::InvalidPageLimit`] for `0`, [`Error::PageLimitExceeded`]
    /// above the maximum). `start` must be `<=` the wallet's attestation
    /// count ([`Error::PageStartOutOfRange`] otherwise) — `start` equal
    /// to the count is valid and returns an empty page, the normal
    /// "no more pages" signal for a caller iterating to the end.
    ///
    /// Replaces v0.1's unbounded `get_attestations`: this call's cost
    /// and response size are bounded by `limit` regardless of how large
    /// `wallet`'s total history is — see
    /// `docs/adr/0004-paginated-attestation-storage.md`.
    pub fn get_attestations_page(
        env: Env,
        wallet: Address,
        start: u32,
        limit: u32,
    ) -> Result<Vec<Attestation>, Error> {
        Self::check_page_limit(limit)?;
        let count = attestation_count(&env, &wallet);
        if start > count {
            return Err(Error::PageStartOutOfRange);
        }

        let end = start.saturating_add(limit).min(count);
        let mut out = Vec::new(&env);
        for seq in start..end {
            if let Some(entry) = env
                .storage()
                .persistent()
                .get::<_, Attestation>(&DataKey::AttestationEntry(wallet.clone(), seq))
            {
                out.push_back(entry);
            }
        }
        Ok(out)
    }

    /// Sum of complexity points across all attestations for a wallet.
    /// Attestations with an unverified tier (`complexity == 0`) count at
    /// [`UNVERIFIED_COMPLEXITY_SCORE`] rather than zero. The sum
    /// saturates at `u32::MAX`; with the accepted tier values that ceiling
    /// is unreachable in practice, and the result is fully deterministic.
    ///
    /// O(1): reads the running counter `submit_attestation` maintains
    /// atomically, never re-derived from a scan of the history — see
    /// `docs/adr/0004-paginated-attestation-storage.md`.
    pub fn get_reputation_score(env: Env, wallet: Address) -> u32 {
        env.storage()
            .persistent()
            .get(&DataKey::ReputationScore(wallet))
            .unwrap_or(0)
    }

    pub fn get_wallet_for_github(env: Env, github_id_hash: BytesN<32>) -> Option<Address> {
        env.storage()
            .persistent()
            .get(&DataKey::GithubLink(github_id_hash))
    }

    pub fn get_github_for_wallet(env: Env, wallet: Address) -> Option<BytesN<32>> {
        env.storage().persistent().get(&DataKey::WalletLink(wallet))
    }

    pub fn get_admin(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Admin)
    }

    pub fn get_attestor(env: Env) -> Option<Address> {
        env.storage().instance().get(&DataKey::Attestor)
    }

    /// Load the stored attestor and reject any caller that is not it.
    fn check_attestor(env: &Env, caller: &Address) -> Result<(), Error> {
        let stored: Address = env
            .storage()
            .instance()
            .get(&DataKey::Attestor)
            .ok_or(Error::NotInitialized)?;
        if caller != &stored {
            return Err(Error::Unauthorized);
        }
        Ok(())
    }

    /// Shared bound check for every paginated call's `limit`.
    fn check_page_limit(limit: u32) -> Result<(), Error> {
        if limit == 0 {
            return Err(Error::InvalidPageLimit);
        }
        if limit > MAX_PAGE_SIZE {
            return Err(Error::PageLimitExceeded);
        }
        Ok(())
    }
}

mod test;
