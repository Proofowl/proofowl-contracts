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
//! ## Storage durability
//!
//! Every long-lived record (wallet links, GitHub links, PR-dedup
//! markers, attestation histories, and the instance itself) has its TTL
//! extended on every write and on every read-write path that touches it,
//! plus a permissionless [`ProofOwlRegistry::bump_wallet_ttl`] keep-alive.
//! See `SECURITY.md` for the exact policy and its rationale.

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
    /// `wallet -> Vec<Attestation>`.
    Attestations(Address),
    /// `pr_hash -> ()` global duplicate-PR guard.
    SeenPr(BytesN<32>),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum Error {
    AlreadyInitialized = 1,
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

// --- TTL policy -------------------------------------------------------------
//
// Soroban archives a persistent entry once its TTL (ledgers remaining)
// hits zero; reading an archived entry fails until it is restored. Every
// registry record here is meant to live indefinitely, so every write and
// every read-write path re-extends the entries it touches, and anyone
// can call `bump_wallet_ttl` to keep a passport warm for free.
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
}

#[contract]
pub struct ProofOwlRegistry;

#[contractimpl]
impl ProofOwlRegistry {
    /// One-time setup. Must be authorized by `admin` — the proposed admin
    /// signs the initializing transaction, so a bystander cannot seize a
    /// deployed-but-uninitialized contract with an admin address they do
    /// not control.
    ///
    /// `attestor` is the key allowed to submit attestations and to
    /// co-sign identity links; rotate it later with [`Self::set_attestor`]
    /// without redeploying.
    ///
    /// For maximum safety, deploy and `init` in the same transaction (or
    /// the same script run) so there is no window at all — see
    /// `SECURITY.md`.
    pub fn init(env: Env, admin: Address, attestor: Address) -> Result<(), Error> {
        if env.storage().instance().has(&DataKey::Admin) {
            return Err(Error::AlreadyInitialized);
        }
        admin.require_auth();

        env.storage().instance().set(&DataKey::Admin, &admin);
        env.storage().instance().set(&DataKey::Attestor, &attestor);
        extend_instance(&env);

        Initialized {
            admin: admin.clone(),
            attestor: attestor.clone(),
        }
        .publish(&env);

        Ok(())
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
    /// (`Attestations(wallet)`) and the global PR-dedup markers are left
    /// untouched: a merged PR stays spent forever, and reputation already
    /// earned stays attached to the wallet that earned it. Migrating a
    /// history to a fresh wallet is out of scope for the MVP — see
    /// `SECURITY.md`.
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

        let attestations_key = DataKey::Attestations(wallet.clone());
        let mut list: Vec<Attestation> = env
            .storage()
            .persistent()
            .get(&attestations_key)
            .unwrap_or(Vec::new(&env));
        list.push_back(attestation);

        env.storage().persistent().set(&attestations_key, &list);
        env.storage().persistent().set(&pr_key, &());

        // Keep every record this contribution depends on warm.
        extend_persistent(&env, &attestations_key);
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
        }
        .publish(&env);

        Ok(wallet)
    }

    /// Permissionless keep-alive. Extends the TTL of a wallet's link, the
    /// GitHub link it points at, and its attestation history. Anyone can
    /// call it (a frontend "keep my passport alive" button, a cron job);
    /// it only pushes out archival and never changes data. No-op for an
    /// unlinked wallet.
    pub fn bump_wallet_ttl(env: Env, wallet: Address) {
        let wallet_key = DataKey::WalletLink(wallet.clone());
        if let Some(github_id_hash) = env.storage().persistent().get::<_, BytesN<32>>(&wallet_key) {
            extend_persistent(&env, &wallet_key);
            extend_persistent(&env, &DataKey::GithubLink(github_id_hash));
        }
        let attestations_key = DataKey::Attestations(wallet);
        if env.storage().persistent().has(&attestations_key) {
            extend_persistent(&env, &attestations_key);
        }
        extend_instance(&env);
    }

    pub fn get_attestations(env: Env, wallet: Address) -> Vec<Attestation> {
        env.storage()
            .persistent()
            .get(&DataKey::Attestations(wallet))
            .unwrap_or(Vec::new(&env))
    }

    /// Sum of complexity points across all attestations for a wallet.
    /// Attestations with an unverified tier (`complexity == 0`) count at
    /// [`UNVERIFIED_COMPLEXITY_SCORE`] rather than zero. The sum
    /// saturates at `u32::MAX`; with the accepted tier values that ceiling
    /// is unreachable in practice, and the result is fully deterministic.
    pub fn get_reputation_score(env: Env, wallet: Address) -> u32 {
        let list: Vec<Attestation> = env
            .storage()
            .persistent()
            .get(&DataKey::Attestations(wallet))
            .unwrap_or(Vec::new(&env));

        list.iter().fold(0u32, |acc, a| {
            let points = if a.complexity == 0 {
                UNVERIFIED_COMPLEXITY_SCORE
            } else {
                a.complexity
            };
            acc.saturating_add(points)
        })
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
}

mod test;
