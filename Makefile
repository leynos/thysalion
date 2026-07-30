.PHONY: help all clean test build release coverage lint fmt check-fmt markdownlint spelling nixie audit rust-audit demo scenes scenes-check scripts-test

SHELL := bash


TARGET ?= thysalion

USER_WHITAKER := $(HOME)/.local/bin/whitaker
USER_BIN_PATH := $(HOME)/.cargo/bin:$(HOME)/.local/bin:$(HOME)/.bun/bin
CARGO ?= cargo
BUILD_JOBS ?=
RUST_FLAGS ?=
RUST_FLAGS := -D warnings $(RUST_FLAGS)
RUSTDOC_FLAGS ?=
RUSTDOC_FLAGS := -D warnings $(RUSTDOC_FLAGS)
# --workspace is load-bearing: with a root package present, Cargo would
# otherwise default to the root package alone and silently skip members.
CARGO_FLAGS ?= --workspace --all-targets --all-features
DEMO ?= empty
# Windowed harness modules and demo binaries cannot execute in CI, so they
# are excluded from coverage measurement (see docs/adr-005 and the
# developers' guide "Demo harness" section for the boundary).
COVERAGE_IGNORE ?= --ignore-filename-regex 'crates/(demos|harness/src/(overlay|screenshot|camera))'
CLIPPY_FLAGS ?= $(CARGO_FLAGS) -- $(RUST_FLAGS)
TEST_FLAGS ?= $(CARGO_FLAGS)
TEST_CMD := $(if $(shell $(CARGO) nextest --version 2>/dev/null),nextest run,test)
COVERAGE_LINKER_FLAGS ?= -fuse-ld=lld
COVERAGE_RUST_FLAGS ?= $(RUST_FLAGS) -C link-arg=$(COVERAGE_LINKER_FLAGS)
MDLINT ?= markdownlint-cli2
NIXIE ?= nixie
TYPOS_VERSION ?= 1.48.0
TYPOS := uv tool run typos@$(TYPOS_VERSION)
WHITAKER ?= $(or $(shell command -v whitaker 2>/dev/null),$(wildcard $(USER_WHITAKER)),whitaker)
SCENE_BUILDER ?= uv run --script scripts/build_fixture_scenes.py

build: target/debug/$(TARGET) ## Build debug binary
release: target/release/$(TARGET) ## Build release binary

all: check-fmt lint test ## Perform a comprehensive check of code
	+$(MAKE) spelling
	+$(MAKE) scripts-test
	+$(MAKE) scenes-check

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -f .typos-oxendict-base.json .typos-oxendict-base.toml

test: ## Run tests with warnings treated as errors
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) $(TEST_CMD) $(TEST_FLAGS) $(BUILD_JOBS)
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) test --doc --workspace --all-features

target/%/$(TARGET): ## Build binary in debug or release mode
	$(CARGO) build $(BUILD_JOBS) $(if $(findstring release,$(@)),--release) --bin $(TARGET)

coverage: ## Generate lcov coverage with lld for llvm-tools compatibility
	@echo "coverage linker flags: $(COVERAGE_LINKER_FLAGS)"
	CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang \
		RUSTFLAGS="$(COVERAGE_RUST_FLAGS)" \
		CFLAGS="$(COVERAGE_LINKER_FLAGS)" \
		LDFLAGS="$(COVERAGE_LINKER_FLAGS)" \
		$(CARGO) llvm-cov --lcov --output-path lcov.info $(COVERAGE_IGNORE) $(TEST_FLAGS)

lint: ## Run Clippy with warnings denied
	RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --no-deps --workspace
	$(CARGO) clippy $(CLIPPY_FLAGS)
	@echo "Whitaker binary: $(WHITAKER)"
	PATH="$(USER_BIN_PATH):$(PATH)" RUSTFLAGS="$(RUST_FLAGS)" $(WHITAKER) --all -- $(CARGO_FLAGS)

typecheck: ## Type-check without building
	RUSTFLAGS="$(RUST_FLAGS)" $(CARGO) check $(CARGO_FLAGS)

# Supported demos are derived from the demo binaries on disk, so the guard
# below cannot drift from reality. DEMO and the derived list reach the shell
# via the environment, never via make interpolation, so neither can inject
# shell syntax; the guard rejects anything not in the list before Cargo is
# invoked. `$(value DEMO)` captures the caller's raw text without a second
# expansion, so a value like `$$(shell ...)` is inert data rather than a
# Make function call. tests/demo_guard.rs pins this behaviour.
DEMOS := $(patsubst demo-%,%,$(basename $(notdir $(wildcard crates/demos/src/bin/demo-*.rs))))

demo: export DEMO_SLUG = $(value DEMO)
demo: export DEMO_ALLOWED = $(DEMOS)
demo: ## Run a capability demonstration binary (DEMO=empty by default)
	@case " $$DEMO_ALLOWED " in \
		*" $$DEMO_SLUG "*) : ;; \
		*) \
			printf 'DEMO must be one of: %s (got: %s)\n' \
				"$$DEMO_ALLOWED" "$$DEMO_SLUG" >&2; \
			exit 2 ;; \
	esac
	$(CARGO) run -p thysalion-demos --features "demo-$$DEMO_SLUG" \
		--bin "demo-$$DEMO_SLUG"

fmt: ## Format Rust and Markdown sources
	$(CARGO) +nightly fmt --all
	mdformat-all

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check

markdownlint: ## Lint Markdown files
	$(MDLINT) '**/*.md'
	+$(MAKE) spelling
spelling: ## Enforce en-GB-oxendict spelling in Markdown prose
	uv run scripts/generate_typos_config.py
	find . -type f -name '*.md' -not -path './target/*' -print0 | \
		xargs -0 $(TYPOS) --config typos.toml --force-exclude

nixie: ## Validate Mermaid diagrams
	$(NIXIE) --no-sandbox

scenes: ## Compile assets/scenes/src/ into the committed fixture scenes
	$(SCENE_BUILDER)

# Regenerates into a temporary directory and compares. This is what stops a
# hand-edited fixture, or a generator change nobody re-ran, from going
# unnoticed until the authoring sources become decoration.
#
# Not run inside `cargo test`: `make test` is pure Cargo, and a Rust test
# shelling out to `uv run` would break `cargo test --workspace` for any
# contributor without a Python toolchain and would add a subprocess to the
# coverage-measured surface.
scenes-check: ## Verify the committed fixture scenes match their sources
	$(SCENE_BUILDER) --check

scripts-test: ## Run the Python script test suites
	uv run --with pytest --with cyclopts python -m pytest scripts/tests -q

audit: rust-audit ## Audit dependencies for known vulnerabilities

rust-audit: ## Audit the Rust workspace for known vulnerabilities
	set -eo pipefail; \
	manifest_list=$$(mktemp); \
	trap 'rm -f "$$manifest_list"' EXIT; \
	printf "Audit metadata phase: deriving workspace manifests\n"; \
	$(CARGO) metadata --no-deps --format-version 1 | python3 -c 'import json, sys; metadata = json.load(sys.stdin); members = set(metadata["workspace_members"]); print(metadata["workspace_root"]); [print(package["manifest_path"]) for package in metadata["packages"] if package["id"] in members]' > "$$manifest_list"; \
	workspace_root=$$(sed -n '1p' "$$manifest_list"); \
	audit_flags=(); \
	for advisory in $$CARGO_AUDIT_IGNORES; do \
		audit_flags+=(--ignore "$$advisory"); \
	done; \
	printf "Auditing Rust workspace %s\n" "$$workspace_root"; \
	sed -n '2,$$p' "$$manifest_list" | while IFS= read -r manifest; do \
		manifest_dir=$$(dirname "$$manifest"); \
		printf "Workspace Rust manifest %s\n" "$$manifest_dir/Cargo.toml"; \
	done; \
	printf "Audit execution phase: running cargo audit\n"; \
	printf "Audit failures may indicate RustSec advisories, cargo metadata errors, or documented ignores that need CARGO_AUDIT_IGNORES entries.\n"; \
	(cd "$$workspace_root" && $(CARGO) audit "$${audit_flags[@]}")

help: ## Show available targets
	@grep -E '^[a-zA-Z_-]+:.*?##' $(MAKEFILE_LIST) | \
	awk 'BEGIN {FS=":"; printf "Available targets:\n"} {printf "  %-20s %s\n", $$1, $$2}'
