//! End-to-end check that initialization is bound to the deployment
//! transaction and cannot be captured by a non-deployer.
//!
//! `Env::register` force-mocks constructor authorization, so it cannot
//! exercise a *failing* deploy. This test uses the real on-chain deploy
//! path — `Deployer::deploy_v2` against the compiled wasm — where
//! `require_auth` behaves exactly as it does on-chain.
//!
//! It needs the release wasm to exist:
//!   cargo build --target wasm32v1-none --release
//! If the artifact is missing the test skips (so `cargo test` alone does
//! not fail); CI builds the wasm before running tests, so it always runs
//! there.

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Bytes, BytesN, Env};

const WASM_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/target/wasm32v1-none/release/proofowl_contracts.wasm"
);

#[test]
fn initialization_is_bound_to_an_authorized_deployment() {
    let Ok(wasm) = std::fs::read(WASM_PATH) else {
        eprintln!(
            "SKIP initialization_is_bound_to_an_authorized_deployment: \
             build the contract first with \
             `cargo build --target wasm32v1-none --release`"
        );
        return;
    };

    // --- negative: an unauthorized deploy cannot initialize -----------
    // Fresh env, nothing mocked, so auth is enforced. The deploy needs
    // the deployer's authorization AND the constructor needs the admin's;
    // with neither present the deploy fails and no instance is created.
    {
        let env = Env::default();
        let wasm_hash = env
            .deployer()
            .upload_contract_wasm(Bytes::from_slice(&env, &wasm));

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let attestor = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[1u8; 32]);

        // The unauthorized deploy is expected to panic; silence the
        // default panic hook so the (expected) host backtrace does not
        // clutter the test output.
        let prev_hook = std::panic::take_hook();
        std::panic::set_hook(Box::new(|_| {}));
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            env.deployer()
                .with_address(deployer, salt)
                .deploy_v2(wasm_hash, (admin, attestor))
        }));
        std::panic::set_hook(prev_hook);
        assert!(
            result.is_err(),
            "deploy-time initialization must fail without the required authorization"
        );
    }

    // --- positive: an authorized deploy binds config atomically ------
    // The configuration is written by the constructor inside the same
    // operation that creates the instance — there is no separate `init`
    // call, so there is nothing for a front-runner to race.
    {
        let env = Env::default();
        // The constructor's `admin.require_auth()` runs one level below
        // the deployer's root invocation, so non-root auth must be
        // allowed for the mock to cover it.
        env.mock_all_auths_allowing_non_root_auth();
        let wasm_hash = env
            .deployer()
            .upload_contract_wasm(Bytes::from_slice(&env, &wasm));

        let deployer = Address::generate(&env);
        let admin = Address::generate(&env);
        let attestor = Address::generate(&env);
        let salt = BytesN::from_array(&env, &[2u8; 32]);

        let id = env
            .deployer()
            .with_address(deployer, salt)
            .deploy_v2(wasm_hash, (admin.clone(), attestor.clone()));

        let client = proofowl_contracts::ProofOwlRegistryClient::new(&env, &id);
        assert_eq!(client.get_admin(), Some(admin));
        assert_eq!(client.get_attestor(), Some(attestor));
    }
}
