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
# same versions; keep them in sync.
CARGO_DENY_VERSION  ?= 0.16.4
CARGO_AUDIT_VERSION ?= 0.21.2

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
deny: ## cargo-deny: license / bans / sources (deterministic) + advisories
	@command -v cargo-deny >/dev/null 2>&1 || { \
		echo "cargo-deny not installed. Run:"; \
		echo "  cargo install --locked --version $(CARGO_DENY_VERSION) cargo-deny"; \
		exit 1; }
	$(CARGO) deny check bans licenses sources
	$(CARGO) deny check advisories

.PHONY: audit
audit: ## cargo-audit: known-vulnerability scan against Cargo.lock
	@command -v cargo-audit >/dev/null 2>&1 || { \
		echo "cargo-audit not installed. Run:"; \
		echo "  cargo install --locked --version $(CARGO_AUDIT_VERSION) cargo-audit"; \
		exit 1; }
	$(CARGO) audit

.PHONY: supply-chain
supply-chain: deny audit ## Run every supply-chain check (needs network + tools)
