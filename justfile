set positional-arguments
set shell := ["bash", "-euo", "pipefail", "-c"]

# List the available recipes
default:
    @just --list --unsorted

# Run every check: prose, shell, the self-tests of the checkers, spelling, TOML formatting and the Rust checks
[group('checks')]
check: check-prose check-shell check-commit-message-self-test check-prose-self-test check-rust-comments-self-test check-doc-headings-self-test check-doctest-attributes-self-test check-feature-docs-self-test check-typos check-toml check-rust

# Check the prose of Markdown files and of Rust documentation comments in the given files, or in every Markdown and Rust file when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-prose *paths:
    @python3 -I "{{ justfile_directory() }}/scripts/check-prose" "$@"

# Check that the prose checker reports exactly the expected findings on its fixtures
[group('checks')]
check-prose-self-test:
    @python3 -I scripts/self-test-checker check-prose

# Run shellcheck on every shell script and on the shell inside every recipe
[group('checks')]
check-shell:
    @scripts/check-shell

# Check a commit message file for attribution, non-ASCII text, and subject format
[group('checks')]
[no-cd]
[no-exit-message]
check-commit-message file:
    @python3 -I "{{ justfile_directory() }}/scripts/check-commit-message" "$1"

# Check that the commit message checker accepts a clean message and rejects attribution
[group('checks')]
check-commit-message-self-test:
    #!/usr/bin/env bash
    set -euo pipefail
    work="$(mktemp -d)"
    trap 'rm -rf -- "${work}"' EXIT
    printf 'Add the safe compiler session\n\nThe body says why.\n' >"${work}/good"
    printf 'Add the safe compiler session\n\nCo-Authored-By: Someone <someone@example.com>\n' >"${work}/bad"
    python3 -I scripts/check-commit-message "${work}/good"
    if python3 -I scripts/check-commit-message "${work}/bad" >/dev/null; then
        printf 'check-commit-message accepted an attribution line\n' >&2
        exit 1
    fi
    printf 'check-commit-message self-test passed\n'

# Check that Rust source holds only API documentation and safety comments, in the given files or in every Rust file when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-rust-comments *paths:
    @python3 -I "{{ justfile_directory() }}/scripts/check-rust-comments" "$@"

# Check that the Rust comment checker reports exactly the expected findings on its fixtures
[group('checks')]
check-rust-comments-self-test:
    @python3 -I scripts/self-test-checker check-rust-comments

# Check the names and order of the headings in item documentation, in the given files or in every Rust file when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-doc-headings *paths:
    @python3 -I "{{ justfile_directory() }}/scripts/check-doc-headings" "$@"

# Check that the heading checker reports exactly the expected findings on its fixtures
[group('checks')]
check-doc-headings-self-test:
    @python3 -I scripts/self-test-checker check-doc-headings

# Check that no documentation code block is marked ignore, in the given files or in every Rust file and README.md when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-doctest-attributes *paths:
    @python3 -I "{{ justfile_directory() }}/scripts/check-doctest-attributes" "$@"

# Check that the doctest attribute checker reports exactly the expected findings on its fixtures
[group('checks')]
check-doctest-attributes-self-test:
    @python3 -I scripts/self-test-checker check-doctest-attributes

# Check that crate-level documentation names every Cargo feature, for the given manifests or for the workspace when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-feature-docs *manifests:
    @python3 -I "{{ justfile_directory() }}/scripts/check-feature-docs" "$@"

# Check that the feature documentation checker reports exactly the expected findings on its fixtures
[group('checks')]
check-feature-docs-self-test:
    @python3 -I scripts/self-test-checker check-feature-docs

# Install the pre-commit, commit-msg and post-merge git hooks
[group('checks')]
hooks-install:
    pre-commit install

# Squash-merge BRANCH onto main from the main checkout, remove its worktree and branch, then push main
[group('git')]
merge BRANCH *ARGS:
    @scripts/merge-branch "$@"

# Remove the worktrees and branches that are already squash-merged into main
[group('git')]
worktree-prune:
    @scripts/worktree-prune

# Format the Rust code of the workspace and every TOML file
[group('rust')]
fmt:
    cargo fmt --all
    taplo fmt

# Run the Rust checks: comments, headings, doctest attributes, feature documentation, formatting, unused and denied dependencies, Clippy, documentation, doctests, tests and Miri
[group('checks')]
check-rust: check-rust-comments check-doc-headings check-doctest-attributes check-feature-docs check-fmt check-unused-deps check-deny check-clippy check-doc check-doctest test-gpu check-miri

# Check that the Rust code of the workspace is formatted
[group('checks')]
check-fmt:
    cargo fmt --all --check

# Run Clippy on every target and feature of the workspace, failing on any warning
[group('checks')]
check-clippy:
    CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features

# Build the API documentation of the workspace, failing on any rustdoc warning
[group('checks')]
check-doc:
    CARGO_BUILD_WARNINGS=deny cargo doc --workspace --no-deps --all-features

# Run the doctests of the workspace
[group('checks')]
check-doctest:
    CARGO_BUILD_WARNINGS=deny cargo test --doc --workspace --all-features

# Run the tests of the default tier, which need no GPU adapter
[group('rust')]
test:
    CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --all-features --profile default --no-tests=warn

# Run the tests of the default and GPU tiers, which need any GPU adapter, including lavapipe
[group('rust')]
test-gpu:
    CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --all-features --profile gpu --no-tests=warn

# Run every test, including the hardware GPU tier, which needs a hardware GPU
[group('rust')]
test-hw-gpu:
    CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --all-features --profile hw-gpu --no-tests=warn

# Check licences, security advisories, duplicate crates and crate sources with cargo-deny, which needs network access
[group('checks')]
check-deny:
    cargo deny check

# Check every file of the repository for spelling mistakes
[group('checks')]
check-typos:
    typos

# Check that every TOML file is formatted
[group('checks')]
check-toml:
    taplo fmt --check

# Check the workspace for unused dependencies
[group('checks')]
check-unused-deps:
    cargo machete

# Run the tests under Miri on the nightly toolchain to find undefined behaviour
[group('checks')]
check-miri:
    cargo +nightly miri nextest run --workspace --no-tests=warn

# Check that every combination of Cargo features compiles, run by CI only
[group('checks')]
check-features:
    CARGO_BUILD_WARNINGS=deny cargo hack check --workspace --all-targets --feature-powerset

# Check that the workspace builds with the toolchain of its declared rust-version, run by CI only
[group('checks')]
check-msrv:
    CARGO_BUILD_WARNINGS=deny cargo hack check --workspace --all-targets --rust-version

# Write a test coverage report to target/lcov.info, run by CI only
[group('rust')]
coverage:
    cargo llvm-cov nextest --workspace --all-features --profile gpu --no-tests=warn --lcov --output-path target/lcov.info
