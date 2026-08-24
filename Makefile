PROJECT_NAME := $(shell awk -F'"' '/^\[package\]/{package=1; next} package && /^name = /{print $$2; exit}' Cargo.toml)
PROJECT_CAP  := $(shell echo $(PROJECT_NAME) | tr '[:lower:]' '[:upper:]')
CURRENT_VERSION := $(shell awk -F'"' '/^\[workspace\.package\]/{w=1} w && /^version[[:space:]]*=/{print $$2; exit}' Cargo.toml)
LATEST_TAG   ?= $(shell git describe --tags --abbrev=0 2>/dev/null)
TOP_DIR      := $(CURDIR)
BUILD_DIR    := $(TOP_DIR)/target
DOCS_BUILD_DIR ?= $(BUILD_DIR)/book
# The repository predates an enforced workspace-wide rustfmt baseline.  Keep
# legacy untouched files stable while requiring every Rust file changed after
# this audited commit (including untracked additions) to be formatted.
RUSTFMT_BASELINE := d24f2bba44b0b6bd230d5c6f04f68f37cb506be6

ifeq ($(PROJECT_NAME),)
$(error Error: project name not found in Cargo.toml)
endif

$(info ------------------------------------------)
$(info Project: $(PROJECT_NAME))
$(info Version: $(CURRENT_VERSION))
$(info ------------------------------------------)

.PHONY: build b compile c fmt f fmt-changed fmt-check lint run r test t test-network print-version tree tree-test interop-check interop-locked test-interop test-build-actions test-native abi-check test-v4-c test-v4-c-import test-v4-c-handle test-v4-c-callback test-v4-linear test-v4-sanitize verify verify-all help h clean docs release

SHELL := /bin/bash


build:
	@cargo build --release

b: build

compile:
	@cargo clean
	@make build

c: compile

fmt:
	@cargo fmt --all

f: fmt

fmt-changed:
	@set -eu; \
		git cat-file -e "$(RUSTFMT_BASELINE)^{commit}"; \
		mapfile -t files < <( \
			{ git diff --name-only --diff-filter=ACMR "$(RUSTFMT_BASELINE)" -- '*.rs'; \
			  git ls-files --others --exclude-standard -- '*.rs'; } | sort -u \
		); \
		if (($${#files[@]})); then \
			rustfmt --edition 2021 --config skip_children=true "$${files[@]}"; \
		fi

fmt-check:
	@set -eu; \
		git cat-file -e "$(RUSTFMT_BASELINE)^{commit}"; \
		mapfile -t files < <( \
			{ git diff --name-only --diff-filter=ACMR "$(RUSTFMT_BASELINE)" -- '*.rs'; \
			  git ls-files --others --exclude-standard -- '*.rs'; } | sort -u \
		); \
		if (($${#files[@]})); then \
			rustfmt --edition 2021 --check --config skip_children=true "$${files[@]}"; \
		fi

lint:
	@cargo clippy --all-targets --all-features -- -D warnings

ARGS ?=
DIR ?= $(TOP_DIR)

run:
	@cd $(DIR) && cargo run --manifest-path $(TOP_DIR)/Cargo.toml -- $(ARGS)

r: run

TREE_DIR ?= $(TOP_DIR)/lang/tooling/fol-editor/tree-sitter

tree:
	@cargo run --bin folc -- tool tree generate "$(TREE_DIR)"

tree-test: tree
	@set -eu; \
		cache="$$(mktemp -d "$${TMPDIR:-/tmp}/fol-tree-sitter-test.XXXXXX")"; \
		trap 'rm -rf "$$cache"' EXIT; \
		output="$$(cd "$(TREE_DIR)" && XDG_CACHE_HOME="$$cache" tree-sitter test)"; \
		printf '%s\n' "$$output"; \
		printf '%s\n' "$$output" | grep -Eq 'Total parses: [1-9][0-9]*; successful parses: [1-9][0-9]*; failed parses: 0;'

interop-check:
	@bash tools/verify-interop-lock.sh
	@cargo test -p fol-interop -p fol-frontend --no-run

interop-locked:
	@bash tools/verify-interop-lock.sh --locked

# Every promoted lane is required, so a missing compiler fails the gate rather
# than quietly proving only the one that happens to be installed. The dev shell
# exports all three.
test-interop: interop-locked
	@set -eu; \
		test "$$(uname -s)" = Linux || { echo "H7 interop requires Linux" >&2; exit 1; }; \
		command -v realpath >/dev/null 2>&1 || { echo "H7 interop requires realpath" >&2; exit 1; }; \
		gcc="$${FOL_H7_GCC:-$$(command -v gcc || true)}"; \
		test -n "$$gcc" || { echo "H7 interop requires GCC; run inside 'nix develop'" >&2; exit 1; }; \
		clang="$${FOL_H7_CLANG:-$$(command -v clang || true)}"; \
		test -n "$$clang" || { echo "H7 interop requires clang; run inside 'nix develop'" >&2; exit 1; }; \
		musl="$${FOL_H7_MUSL_CC:-}"; \
		test -n "$$musl" || { echo "H7 interop requires FOL_H7_MUSL_CC; run inside 'nix develop'" >&2; exit 1; }; \
		FOL_H7_REQUIRED=1 \
		FOL_H7_GCC="$$(realpath "$$gcc")" \
		FOL_H7_CLANG="$$(realpath "$$clang")" \
		FOL_H7_MUSL_CC="$$(realpath "$$musl")" \
		cargo test -p fol-frontend --test interop_h7 -- --nocapture


TEST_ARGS ?=

test:
	@cargo test --workspace $(TEST_ARGS)

# The action graph and materializer gate. Separate from `test` because these
# exercise filesystem publication, process locks, and tool execution: a failure
# here means a build could publish a partial tree or run an untrusted binary,
# which is worth naming rather than burying in a workspace-wide run.
test-build-actions:
	@set -eu; \
		for module in action_graph action_trust materialize plan; do \
			cargo test -p fol-build "$$module::" || exit 1; \
		done

# The M3 native-product gate: builds real static and shared libraries and links
# a C program against them. Separate from `test` because a failure here means a
# library FOL claims to produce is not one a C consumer can link -- worth naming
# rather than burying in a workspace-wide run.
test-native:
	@cargo test -p fol --test native -- --nocapture

# The M5 C export gate: a real C program links the installed FOL library and
# calls every scalar export through the generated header. A failure here means
# the FOL -> C path is broken somewhere a unit test cannot see.
test-v4-c:
	@cargo test -p fol --test v4_c_export -- --nocapture

# M6's C import slice: bind a real header against a real archive, build a FOL
# program that calls it, and run it. Needs FOL_INTEROP_GCC/FOL_INTEROP_TEMP;
# skips without them unless FOL_H7_REQUIRED is set.
test-v4-c-import:
	@cargo test -p fol --test v4_c_import -- --nocapture

# The ASan/UBSan boundary lane. Compiles each checked-in C consumer with
# sanitizers on and runs it, plus a deliberate-overflow control so the lane
# cannot pass by not running. Skips with a message when no sanitizer-capable
# compiler is present.
test-v4-sanitize:
	@cargo test -p fol --test v4_c_sanitize -- --nocapture

# M7.4's linear resource gate: a handle released two ways, and the three
# refusals (leak, report-while-holding, one-sided branch) with their reasons.
test-v4-linear:
	@cargo test -p fol --test v4_linear -- --nocapture

# M7.4 opaque C handles: a real provider, a real bind, and the four misuses
# C cannot catch at all. Skips without a C toolchain unless FOL_H7_REQUIRED.
test-v4-c-handle:
	@cargo test -p fol --test v4_c_handle -- --nocapture

# M7.5 synchronous callbacks: C invoking a FOL closure, the trampoline's
# containment, and the one canonical shape that is imported.
test-v4-c-callback:
	@cargo test -p fol --test v4_c_callback -- --nocapture

# The ABI model gate: the canonical type vocabulary, the classifier's required
# negative cases, the verifier, the manifest encoding, and the two fingerprints.
# Separate from `test` because a failure here means the compiler's idea of the C
# boundary moved, which every later milestone builds on.
abi-check:
	@set -eu; \
		cargo test -p fol-abi; \
		cargo test -p fol-backend "abi::"

# The only #[ignore]d tests in the tree fetch real repositories over the
# network, so they stay out of `verify` and run on demand (and nightly in CI).
test-network:
	@cargo test -p fol --test integration -- $(TEST_ARGS) --ignored

print-version:
	@echo $(CURRENT_VERSION)

t: test

verify: fmt-check lint test test-build-actions test-native abi-check test-v4-c test-v4-c-import test-v4-c-handle test-v4-c-callback test-v4-linear test-v4-sanitize interop-check test-interop

verify-all: verify test-network

help:
	@echo
	@echo "Usage: make [target]"
	@echo
	@echo "Available targets:"
	@echo "  build        Build project"
	@echo "  compile      Configure and generate build files"
	@echo "  fmt          Format the Rust workspace"
	@echo "  fmt-changed  Format Rust files changed after the audited baseline"
	@echo "  fmt-check    Check the incremental Rust formatting baseline"
	@echo "  lint         Run Clippy for all targets and features"
	@echo "  run          Run the main executable"
	@echo "  tree         Regenerate the checked-in tree-sitter bundle"
	@echo "  tree-test    Regenerate and run non-empty tree-sitter corpus tests"
	@echo "  interop-check Verify the sibling lock and compile the H7 integration"
	@echo "  interop-locked Require exact clean sibling revisions and remotes"
	@echo "  test-interop Run the required Linux/GCC H7 link-and-run smoke"
	@echo "  test         Run tests"
	@echo "  test-network Run the network-dependent ignored tests"
	@echo "  verify       Run the complete non-mutating repository gate"
	@echo "  verify-all   Run verify plus the network-dependent tests"
	@echo "  docs         Build documentation in target/book (TYPE=mdbook|rustdoc)"
	@echo "  release      Create a new release (TYPE=patch|minor|major)"
	@echo

h : help

clean:
	@echo "Cleaning build directory..."
	@rm -rf $(BUILD_DIR)
	@echo "Build directory cleaned."

docs:
ifeq ($(TYPE),mdbook)
	@command -v mdbook >/dev/null 2>&1 || { echo "mdbook is not installed. Please install it first."; exit 1; }
	@mdbook build $(TOP_DIR)/book --dest-dir $(DOCS_BUILD_DIR)
	@echo "Documentation written to $(DOCS_BUILD_DIR)"
else ifeq ($(TYPE),rustdoc)
	@cargo doc --workspace --no-deps
	@echo "API documentation written to target/doc"
else
	$(error Invalid documentation type. Use 'make docs TYPE=mdbook' or 'make docs TYPE=rustdoc')
endif

TYPE ?= patch
HAS_REL := $(shell command -v git-rel 2>/dev/null)

release:
	@if [ -z "$(HAS_REL)" ]; then \
		echo "git-rel is not installed. Please install it first."; \
		exit 1; \
	fi
	@if [ -z "$(TYPE)" ]; then \
		echo "Release type not specified. Use 'make release TYPE=[patch|minor|major|m.m.p]'"; \
		exit 1; \
	fi
	@git rel $(TYPE)
