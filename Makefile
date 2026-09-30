.PHONY: help all clean test build release coverage lint lint-clippy lint-whitaker \
	typecheck fmt check-fmt markdownlint spelling nixie audit rust-audit demo \
	scenes scenes-check scripts-test install-build-tools check-build-tools \
	install-markdownlint

SHELL := bash
.NOTPARALLEL: lint


TARGET ?= thysalion

CARGO ?= cargo
RUSTC ?= rustc
BUILD_TOOLS_PREFIX ?= $(HOME)/.local
MOLD_VERSION_FILE ?= tools/mold/VERSION
MOLD_SHA256SUMS_FILE ?= tools/mold/SHA256SUMS
export BUILD_TOOLS_PREFIX MOLD_VERSION_FILE MOLD_SHA256SUMS_FILE
export PATH := $(BUILD_TOOLS_PREFIX)/bin:$(PATH)
BUILD_JOBS ?=
RUST_FLAGS ?=
RUST_FLAGS := -D warnings $(RUST_FLAGS)
# The build standard: every `rustflags` source in `.cargo/config.toml` carries
# the parallel frontend, and the Linux source adds mold. Assigning `RUSTFLAGS`
# replaces those sources outright, so the gate targets restate the flags here.
# Coverage and release builds deliberately take neither.
STANDARD_THREADS_FLAG ?= -Zthreads=8
STANDARD_MOLD_FLAG ?= -Clink-arg=-fuse-ld=mold
BUILD_HOST_OS := $(shell uname -s)
STANDARD_RUSTFLAGS = $(STANDARD_THREADS_FLAG)$(if $(filter Linux,$(BUILD_HOST_OS)), $(STANDARD_MOLD_FLAG))
# Keep inherited backend selectors out of each built-in Cargo profile. Test
# inherits dev and bench inherits release, but either may be overridden by an
# exported profile setting; build scripts and proc macros use build-override.
BACKEND_PROFILE_ENV_UNSETS = \
	-u CARGO_PROFILE_DEV_CODEGEN_BACKEND \
	-u CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND \
	-u CARGO_PROFILE_TEST_CODEGEN_BACKEND \
	-u CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND \
	-u CARGO_PROFILE_RELEASE_CODEGEN_BACKEND \
	-u CARGO_PROFILE_RELEASE_BUILD_OVERRIDE_CODEGEN_BACKEND \
	-u CARGO_PROFILE_BENCH_CODEGEN_BACKEND \
	-u CARGO_PROFILE_BENCH_BUILD_OVERRIDE_CODEGEN_BACKEND
# Keep development on Cargo's LLVM default even when callers export a
# different backend through their shell environment.
DEVELOPMENT_CARGO_ENV = env -u CARGO_ENCODED_RUSTFLAGS $(BACKEND_PROFILE_ENV_UNSETS) -u CARGO_UNSTABLE_CODEGEN_BACKEND
# Release builds take neither development flag. Cross runs stable Cargo inside
# the checkout, so the committed config must omit codegen-backend settings.
# The host invocation uses an external directory and explicit manifest path.
RELEASE_RUSTFLAGS = env -u CARGO_ENCODED_RUSTFLAGS $(BACKEND_PROFILE_ENV_UNSETS) -u CARGO_UNSTABLE_CODEGEN_BACKEND RUSTFLAGS=""
RUSTDOC_FLAGS ?= --cfg docsrs -D warnings
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
# Coverage explicitly selects LLVM even though it is the development default,
# so inherited backend overrides cannot change the instrumentation route.
# `COVERAGE_RUST_FLAGS` displaces the development mold flag, because RUSTFLAGS
# replaces Cargo's configured target flags wholesale. The coverage linker is
# lld; a host with a different supported toolchain may override these values.
COVERAGE_CODEGEN_BACKEND ?= llvm
# `-fuse-ld=lld` needs an `ld.lld` on PATH, which a host may not have even
# with clang installed. The rustup toolchain always ships one, so fall back to
# it rather than requiring a system lld just to run the gate locally.
COVERAGE_LLD_DIR ?= $(shell $(RUSTC) --print sysroot)/lib/rustlib/$(shell $(RUSTC) -vV | sed -n 's/^host: //p')/bin/gcc-ld
BUN ?= bun
MDLINT ?= markdownlint-cli2
# `make fmt` and `make check-fmt` call mdtablefix directly. `--git` selects the
# Markdown files Git tracks and `--include-untracked` adds the untracked files
# Git does not ignore, so a new document is formatted before it is staged.
# Both modes need mdtablefix 0.6.0 or later; CI pins the version at the
# install-mdtablefix step.
MDTABLEFIX ?= mdtablefix
MDTABLEFIX_SELECT = --git --include-untracked
MDTABLEFIX_RULES = --wrap --renumber --breaks --ellipsis --fences
NIXIE ?= nixie
TYPOS_CONFIG_BUILDER_VERSION ?= v0.1.3
TYPOS_CONFIG_BUILDER = uv tool run --from \
	"git+https://github.com/leynos/typos-config-builder.git@$(TYPOS_CONFIG_BUILDER_VERSION)" \
	typos-config-builder
WHITAKER ?= whitaker
SCENE_BUILDER ?= uv run --script scripts/build_fixture_scenes.py

build: check-build-tools ## Build debug binary
	$(DEVELOPMENT_CARGO_ENV) RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)" $(CARGO) build $(BUILD_JOBS) --bin $(TARGET)
release: ## Build release binary with production Cargo routing
	(cd / && $(RELEASE_RUSTFLAGS) $(CARGO) +stable build --manifest-path "$(abspath Cargo.toml)" $(BUILD_JOBS) --release --bin $(TARGET))

all: ## Perform a comprehensive check of code, sequentially
	+$(MAKE) check-fmt
	+$(MAKE) lint
	+$(MAKE) test
	+$(MAKE) spelling
	+$(MAKE) scripts-test
	+$(MAKE) scenes-check

clean: ## Remove build artifacts
	$(CARGO) clean
	rm -f .typos-oxendict-base.json .typos-oxendict-base.toml

test: check-build-tools ## Run tests with warnings treated as errors
	$(DEVELOPMENT_CARGO_ENV) RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)" $(CARGO) $(TEST_CMD) $(TEST_FLAGS) $(BUILD_JOBS)
	$(DEVELOPMENT_CARGO_ENV) RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)" $(CARGO) test --doc --workspace --all-features $(BUILD_JOBS)

coverage: check-build-tools ## Generate lcov coverage with lld for llvm-tools compatibility
	@echo "coverage linker flags: $(COVERAGE_LINKER_FLAGS)"
	env -u CARGO_ENCODED_RUSTFLAGS PATH="$(COVERAGE_LLD_DIR):$$PATH" \
		CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=clang \
		CARGO_UNSTABLE_CODEGEN_BACKEND=true \
		CARGO_PROFILE_DEV_CODEGEN_BACKEND=$(COVERAGE_CODEGEN_BACKEND) \
		CARGO_PROFILE_TEST_CODEGEN_BACKEND=$(COVERAGE_CODEGEN_BACKEND) \
		CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND=$(COVERAGE_CODEGEN_BACKEND) \
		CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND=$(COVERAGE_CODEGEN_BACKEND) \
		RUSTFLAGS="$(COVERAGE_RUST_FLAGS)" \
		CFLAGS="$(COVERAGE_LINKER_FLAGS)" \
		LDFLAGS="$(COVERAGE_LINKER_FLAGS)" \
		$(CARGO) llvm-cov --lcov --output-path lcov.info $(COVERAGE_IGNORE) $(TEST_FLAGS)

lint: lint-clippy lint-whitaker ## Run the Rust lint gates in order

lint-clippy: check-build-tools ## Run rustdoc and Clippy with warnings denied
	$(DEVELOPMENT_CARGO_ENV) RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)" RUSTDOCFLAGS="$(RUSTDOC_FLAGS)" $(CARGO) doc --no-deps --workspace
	$(DEVELOPMENT_CARGO_ENV) RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)" $(CARGO) clippy $(CLIPPY_FLAGS)

# Make 4.4 serializes these prerequisites even under `make -j lint`. Keep the
# Whitaker leaf independent so its failure path can be checked without Cargo.
lint-whitaker: ## Run the installer-managed rolling Whitaker suite
	@command -v "$(WHITAKER)" >/dev/null 2>&1 || { \
		printf 'Whitaker is unavailable; install it with the shared installer before make lint.\n' >&2; \
		exit 1; \
	}
	@echo "Whitaker binary: $(WHITAKER)"
	env -u CARGO_ENCODED_RUSTFLAGS $(BACKEND_PROFILE_ENV_UNSETS) RUSTFLAGS="" \
		CARGO_UNSTABLE_CODEGEN_BACKEND=true \
		CARGO_PROFILE_DEV_CODEGEN_BACKEND=llvm \
		CARGO_PROFILE_TEST_CODEGEN_BACKEND=llvm \
		CARGO_PROFILE_DEV_BUILD_OVERRIDE_CODEGEN_BACKEND=llvm \
		CARGO_PROFILE_TEST_BUILD_OVERRIDE_CODEGEN_BACKEND=llvm \
		$(WHITAKER) --all -- $(CARGO_FLAGS)

typecheck: check-build-tools ## Type-check without building
	$(DEVELOPMENT_CARGO_ENV) RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)" $(CARGO) check $(CARGO_FLAGS)

install-build-tools: ## Install the pinned development linker and toolchain
	@scripts/install-build-tools.sh

check-build-tools: ## Check development linker and toolchain prerequisites
	@scripts/check-build-tools.sh

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
demo: check-build-tools ## Run a capability demonstration binary (DEMO=empty by default)
	@case " $$DEMO_ALLOWED " in \
		*" $$DEMO_SLUG "*) : ;; \
		*) \
			printf 'DEMO must be one of: %s (got: %s)\n' \
				"$$DEMO_ALLOWED" "$$DEMO_SLUG" >&2; \
			exit 2 ;; \
	esac
	$(DEVELOPMENT_CARGO_ENV) RUSTFLAGS="$(RUST_FLAGS) $(STANDARD_RUSTFLAGS)" $(CARGO) run -p thysalion-demos --features "demo-$$DEMO_SLUG" \
		--bin "demo-$$DEMO_SLUG"

fmt: ## Format Rust and Markdown sources
	$(CARGO) fmt --all
	$(MDTABLEFIX) --in-place $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)
	@unset FORCE_COLOR; $(MDLINT) --fix "**/*.md"

check-fmt: ## Verify formatting
	$(CARGO) fmt --all -- --check
	$(MDTABLEFIX) --check $(MDTABLEFIX_SELECT) $(MDTABLEFIX_RULES)

markdownlint: ## Lint Markdown files
	$(MDLINT) '**/*.md'
	+$(MAKE) spelling

install-markdownlint: ## Install the CI-matched Markdown linter locally
	@if ! command -v "$(BUN)" >/dev/null 2>&1; then \
		printf 'Bun is required; install Bun, then rerun make install-markdownlint.\n' >&2; \
		exit 127; \
	fi
	$(BUN) add --global --exact markdownlint-cli2@0.22.1

spelling: ## Enforce en-GB-oxendict spelling in Markdown prose
	$(TYPOS_CONFIG_BUILDER) gate --repository .

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
