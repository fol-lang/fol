PROJECT_NAME := $(shell awk -F'"' '/^\[package\]/{package=1; next} package && /^name = /{print $$2; exit}' Cargo.toml)
PROJECT_CAP  := $(shell echo $(PROJECT_NAME) | tr '[:lower:]' '[:upper:]')
CURRENT_VERSION := $(shell awk -F'"' '/^\[package\]/{package=1; next} package && /^version = /{print $$2; exit}' Cargo.toml)
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

.PHONY: build b compile c fmt f fmt-changed fmt-check lint run r test t tree tree-test interop-check interop-locked test-interop verify help h clean docs release

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
	@cargo run -- tool tree generate "$(TREE_DIR)"

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

test-interop: interop-locked
	@set -eu; \
		test "$$(uname -s)" = Linux || { echo "H7 interop requires Linux" >&2; exit 1; }; \
		gcc="$$(command -v gcc || true)"; \
		test -n "$$gcc" || { echo "H7 interop requires GCC" >&2; exit 1; }; \
		command -v realpath >/dev/null 2>&1 || { echo "H7 interop requires realpath" >&2; exit 1; }; \
		gcc="$$(realpath "$$gcc")"; \
		FOL_H7_REQUIRED=1 FOL_H7_GCC="$$gcc" cargo test -p fol-frontend --test interop_h7 -- --nocapture


TEST_ARGS ?=

test:
	@cargo test --workspace $(TEST_ARGS)
	@cargo test -- $(TEST_ARGS) --ignored

t: test

verify: fmt-check lint test interop-check test-interop

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
	@echo "  verify       Run the complete non-mutating repository gate"
	@echo "  docs         Build documentation in target/book (TYPE=mdbook|doxygen)"
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
else ifeq ($(TYPE),doxygen)
	@command -v doxygen >/dev/null 2>&1 || { echo "doxygen is not installed. Please install it first."; exit 1; }
else
	$(error Invalid documentation type. Use 'make docs TYPE=mdbook' or 'make docs TYPE=doxygen')
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
