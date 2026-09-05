//! Cross-language canonical hash vector verification.
//!
//! The contract never computes `github_id_hash` or `pr_hash` itself —
//! both are opaque `BytesN<32>` values it just stores
//! (`docs/integration/identifier-spec-v1.md`). The canonicalization and
//! hashing rules are specified once and implemented independently in
//! two places: `sdk/typescript/src/identifiers.ts` (Node's
//! `node:crypto`) and, here, the Soroban host's own `sha256` (via
//! `env.crypto().sha256`, the same primitive
//! `CustomAccountInterface::__check_auth` and other host crypto paths
//! rely on).
//!
//! This file recomputes the exact vectors pinned in
//! `sdk/typescript/src/identifiers.test.ts` (which are themselves
//! copied from `identifier-spec-v1.md` §1.4 / §2.4) using the Rust/host
//! hasher, and asserts byte-for-byte agreement. This is the specific
//! gap closed this phase: "the spec says these bytes" and "the
//! TypeScript SDK produces these bytes" were both already true; this
//! proves a *third*, independent implementation (the on-chain host
//! primitive) agrees too, so a canonicalization bug can't hide behind
//! two implementations that happen to share a mistake.
//!
//! Fully offline: `env.crypto().sha256` runs inside the local Soroban
//! test host, no network call.

use soroban_sdk::{Bytes, Env};

fn sha256_hex(env: &Env, canonical: &str) -> std::string::String {
    let bytes = Bytes::from_slice(env, canonical.as_bytes());
    let digest = env.crypto().sha256(&bytes);
    digest
        .to_array()
        .iter()
        .map(|b| std::format!("{b:02x}"))
        .collect()
}

// ---------------------------------------------------------------------------
// github_id_hash vectors -- identifier-spec-v1.md §1.4,
// identifiers.test.ts `GH_USER_VECTORS`.
// ---------------------------------------------------------------------------

#[test]
fn github_user_id_hash_vectors_match_the_typescript_sdk() {
    let env = Env::default();
    let vectors: [(&str, &str); 3] = [
        (
            "proofowl:github-user:v1:1",
            "ad6494a9db671dce66088a82f8446c464e7d425da57d4eca4081b19a74b1e584",
        ),
        (
            "proofowl:github-user:v1:1024025",
            "fd608646c4bd0a96553707213c1680c9dfcb0c9ba47f649ccb1c7924125176cb",
        ),
        (
            "proofowl:github-user:v1:9007199254740991",
            "1e7fa4a5295f32689530d00860728b707d60f73de136143ee122575b46604e9e",
        ),
    ];

    for (canonical, expected_hex) in vectors {
        assert_eq!(
            sha256_hex(&env, canonical),
            expected_hex,
            "canonical string {canonical:?} must hash identically on-chain and in the TS SDK"
        );
    }
}

// ---------------------------------------------------------------------------
// pr_hash vectors -- identifier-spec-v1.md §2.4,
// identifiers.test.ts `PR_VECTORS`. The canonical strings here are
// already fully normalized (lowercase, no @/#/`.git`) -- normalization
// itself is an SDK-only concern per the spec (the contract accepts
// whatever `pr_hash` bytes the attestor supplies), so this file checks
// only the hash step, not the normalization step.
// ---------------------------------------------------------------------------

#[test]
fn pull_request_hash_vectors_match_the_typescript_sdk() {
    let env = Env::default();
    let vectors: [(&str, &str); 3] = [
        (
            "github.com/stellar/soroban-examples/pull/42",
            "1eed82536f9e3a9477916599ab2111d9af634b1270f5d4d1d61ee98bd50d6c0e",
        ),
        (
            "github.com/proofowl/proofowl-contracts/pull/7",
            "be9b713cbcbacdc44d593cd3e37f8680f6e7e229af9c2182cde3ee05a2bf6cef",
        ),
        (
            "github.com/a/b/pull/1",
            "74b8b07fec5539a632c2df4ecd2aafaadfe0df40f9941fba6c11bfa7039c4c93",
        ),
    ];

    for (canonical, expected_hex) in vectors {
        assert_eq!(
            sha256_hex(&env, canonical),
            expected_hex,
            "canonical string {canonical:?} must hash identically on-chain and in the TS SDK"
        );
    }
}

// ---------------------------------------------------------------------------
// Round-trip: a hash computed here, fed straight into the contract as a
// BytesN<32>, behaves exactly as `submit_attestation` documents --
// closing the loop from "the SDK computed this pr_hash" to "the
// contract accepted exactly this 32-byte value".
// ---------------------------------------------------------------------------

#[test]
fn a_host_computed_hash_round_trips_through_submit_attestation() {
    use proofowl_contracts::{ProofOwlRegistry, ProofOwlRegistryClient};
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::{Address, String};

    let env = Env::default();
    env.mock_all_auths();
    let admin = Address::generate(&env);
    let attestor = Address::generate(&env);
    let contract_id = env.register(ProofOwlRegistry, (admin, attestor.clone()));
    let client = ProofOwlRegistryClient::new(&env, &contract_id);

    let wallet = Address::generate(&env);
    let gh_canonical = "proofowl:github-user:v1:1024025";
    let gh_bytes = Bytes::from_slice(&env, gh_canonical.as_bytes());
    let gh_hash = env.crypto().sha256(&gh_bytes).to_bytes();
    client.link_github(&wallet, &attestor, &gh_hash);

    let pr_canonical = "github.com/stellar/soroban-examples/pull/42";
    let pr_bytes = Bytes::from_slice(&env, pr_canonical.as_bytes());
    let pr_hash = env.crypto().sha256(&pr_bytes).to_bytes();

    let repo = String::from_str(&env, "stellar/soroban-examples");
    let credited =
        client.submit_attestation(&attestor, &gh_hash, &repo, &42u32, &1u64, &100u32, &pr_hash);
    assert_eq!(credited, wallet);

    let stored = client.get_attestations(&wallet);
    assert_eq!(stored.get(0).unwrap().pr_hash, pr_hash);

    // The same pr_hash, recomputed independently a second time from the
    // same canonical string, is byte-identical and therefore correctly
    // rejected as a duplicate -- hashing is deterministic across calls,
    // not merely within one.
    let pr_hash_again = env.crypto().sha256(&pr_bytes).to_bytes();
    assert_eq!(pr_hash, pr_hash_again);
    assert!(client
        .try_submit_attestation(
            &attestor,
            &gh_hash,
            &repo,
            &42u32,
            &1u64,
            &100u32,
            &pr_hash_again
        )
        .is_err());
}
