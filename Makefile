# ProofOwl contracts — developer command interface
#
# Thin, predictable wrappers around the same commands CI runs. No target
# here deploys anything, pushes anything, or reads a secret. `make check`
# is the complete local quality gate and must pass before every commit.
#
# Compatible with GNU Make 3.81 (the macOS system make).

# --- configuration ---------------------------------------------------------

CARGO       ?= cargo
WASM_TARGET ?= wasm32v1-none
WASM_CRATE  ?= proofowl_contracts
WASM_OUT    := target/$(WASM_TARGET)/release/$(WASM_CRATE).wasm

# Pinned tool versions for the optional supply-chain targets. CI pins the
# same versions; keep them in sync (see .github/workflows/ci.yml and
# deny.toml).
CARGO_DENY_VERSION  ?= 0.20.2
CARGO_AUDIT_VERSION ?= 0.22.2

# Advisory ignored in deny.toml with a documented rationale; keep the
# `cargo audit` invocation consistent with it.
AUDIT_IGNORE ?= --ignore RUSTSEC-2024-0436

.DEFAULT_GOAL := help

# --- primary targets -----------------------------------------------------

.PHONY: help
help: ## Show this help
	@echo "ProofOwl contracts — make targets"
	@echo
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) \
		| sort \
		| awk 'BEGIN {FS = ":.*?## "} {printf "  \033[36m%-12s\033[0m %s\n", $$1, $$2}'
	@echo
	@echo "Quality gate:   make check"
	@echo "WASM target:    $(WASM_TARGET)  (rustup target add $(WASM_TARGET))"

.PHONY: fmt
fmt: ## Format the workspace in place (rustfmt)
	$(CARGO) fmt --all

.PHONY: fmt-check
fmt-check: ## Check formatting without writing (used by `check` and CI)
	$(CARGO) fmt --all -- --check

.PHONY: lint
lint: ## Clippy with warnings denied, all targets
	$(CARGO) clippy --all-targets -- -D warnings

.PHONY: build
build: ## Build the release WASM for the supported Soroban target
	$(CARGO) build --target $(WASM_TARGET) --release
	@echo "built: $(WASM_OUT)"

.PHONY: test
test: build ## Run the full test suite (build first: an integration test needs the WASM)
	$(CARGO) test --all

.PHONY: check
check: fmt-check lint build test ## Complete local quality gate (matches CI)
	@echo
	@echo "OK — fmt, clippy, wasm build, and tests all passed."

.PHONY: clean
clean: ## Remove build artifacts and regenerated test snapshots
	$(CARGO) clean
	rm -rf test_snapshots

# --- optional: supply-chain checks -------------------------------------
# Not part of `check`: they need network access (advisory DB) and extra
# tools, so they run as their own CI job. Install locally with:
#   cargo install --locked --version $(CARGO_DENY_VERSION) cargo-deny
#   cargo install --locked --version $(CARGO_AUDIT_VERSION) cargo-audit

.PHONY: deny
deny: ## cargo-deny: bans / licenses / sources (deterministic) + advisories
	@command -v cargo-deny >/dev/null 2>&1 || { \
		echo "cargo-deny not installed. Run:"; \
		echo "  cargo install --locked --version $(CARGO_DENY_VERSION) cargo-deny"; \
		exit 1; }
	$(CARGO) deny check

.PHONY: audit
audit: ## cargo-audit: known-vulnerability scan against Cargo.lock
	@command -v cargo-audit >/dev/null 2>&1 || { \
		echo "cargo-audit not installed. Run:"; \
		echo "  cargo install --locked --version $(CARGO_AUDIT_VERSION) cargo-audit"; \
		exit 1; }
	$(CARGO) audit $(AUDIT_IGNORE)

.PHONY: supply-chain
supply-chain: deny audit ## Run every supply-chain check (needs network + tools)

# --- TypeScript integration SDK (sdk/typescript/) ----------------------
# Separate from `make check` (which stays Rust-only). These need Node
# (>= 22.6) and, for `sdk-generate` / `integration-check`, the stellar
# CLI. See sdk/typescript/README.md.

SDK_DIR ?= sdk/typescript

.PHONY: sdk-install
sdk-install: ## Install the TypeScript SDK deps from its committed lockfile
	cd $(SDK_DIR) && npm ci

.PHONY: sdk-generate
sdk-generate: ## Regenerate the contract bindings from the WASM (needs stellar CLI)
	cd $(SDK_DIR) && npm run generate

.PHONY: sdk-test
sdk-test: ## Format-check, lint, type-check, and unit-test the TypeScript SDK
	cd $(SDK_DIR) && npm run check

.PHONY: sdk-drift-check
sdk-drift-check: sdk-generate ## Fail if the generated bindings are stale vs a fresh regeneration
	@git diff --exit-code -- $(SDK_DIR)/src/generated \
		|| { echo "generated bindings are stale — run 'make sdk-generate' and commit"; exit 1; }

.PHONY: integration-check
integration-check: sdk-test sdk-drift-check ## Full TypeScript SDK gate (unit tests + binding drift)
	@echo
	@echo "OK — SDK format, lint, types, unit tests, and binding drift all clean."

.PHONY: sdk-integration-testnet
sdk-integration-testnet: ## Read-only testnet check (get_admin/get_attestor); opt-in, never mutates
	cd $(SDK_DIR) && npm run test:integration
