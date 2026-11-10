set positional-arguments
set shell := ["bash", "-euo", "pipefail", "-c"]

# The assignment that puts the library directory of the Vulkan SDK that VULKAN_SDK names ahead of the library path of the dynamic loader, on Linux and macOS, so that the programs that a recipe runs find the Slang shared library. It is empty on Windows, where PATH finds the DLL, and when VULKAN_SDK is unset or empty. A recipe puts it on the command line of the program it runs, because macOS removes the DYLD_ variables when it starts a protected program such as /bin/bash.
library_path_variable := if os() == "linux" { "LD_LIBRARY_PATH" } else if os() == "macos" { "DYLD_LIBRARY_PATH" } else { "" }
vulkan_sdk := env("VULKAN_SDK", "")
existing_library_path := if library_path_variable == "" { "" } else { env(library_path_variable, "") }
slang_library_path := if library_path_variable == "" { "" } else if vulkan_sdk == "" { "" } else { library_path_variable + "=" + quote(vulkan_sdk / "lib" + if existing_library_path == "" { "" } else { ":" + existing_library_path }) }

# List the available recipes
default:
    @just --list --unsorted

# Run every check: prose, shell, the self-tests of the checkers, spelling, TOML formatting, workflows, and the Rust checks
[group('checks')]
check: check-prose check-shell check-commit-message-self-test check-prose-self-test check-rust-comments-self-test check-doc-headings-self-test check-doctest-attributes-self-test check-feature-docs-self-test check-slang-call-guard-self-test check-workflow-pins-self-test check-typos check-toml check-workflows check-workflow-pins check-rust

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

# Check that every call of slang-compiler into Slang runs inside the closure of abort_on_exception, in the given files or in every Rust file of crates/slang-compiler/src when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-slang-call-guard *paths:
    @python3 -I "{{ justfile_directory() }}/scripts/check-slang-call-guard" "$@"

# Check that the Slang call guard checker reports exactly the expected findings on its fixtures
[group('checks')]
check-slang-call-guard-self-test:
    @python3 -I scripts/self-test-checker check-slang-call-guard

# Check every GitHub Actions workflow with actionlint
[group('checks')]
check-workflows:
    actionlint

# Check that every workflow and composite action pins each action to a full commit SHA, and that every workflow grants read access only and avoids pull_request_target, in the given files or in every one when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-workflow-pins *paths:
    @python3 -I "{{ justfile_directory() }}/scripts/check-workflow-pins" "$@"

# Check that the workflow pin checker reports exactly the expected findings on its fixtures
[group('checks')]
check-workflow-pins-self-test:
    @python3 -I scripts/self-test-checker check-workflow-pins

# Download the Vulkan SDK VERSION, verify it against SHA256, install it, and export it to later steps, run by CI only
[group('ci')]
install-vulkan-sdk VERSION SHA256:
    @scripts/install-vulkan-sdk "$@"

# Check that the Vulkan SDK at VULKAN_SDK provides the Slang headers, library, and slangc of the build tag of the pin
[group('checks')]
check-vulkan-sdk:
    @scripts/check-vulkan-sdk

# Install the pre-commit, commit-msg, and post-merge git hooks
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

# Format the Rust code of the workspace, every Rust file that git tracks or does not ignore, which includes the files that a crate pulls in with include!, and every TOML file
[group('rust')]
fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    listing="$(mktemp)"
    trap 'rm -f "${listing}"' EXIT
    if ! git ls-files -z --cached --others --exclude-standard -- '*.rs' >"${listing}"; then
        printf '%s\n' "fmt: git ls-files failed, so the Rust files that a crate pulls in with include! cannot be listed." >&2
        exit 1
    fi
    files=()
    while IFS= read -r -d '' file; do
        if [[ -f "${file}" ]]; then
            files+=("${file}")
        fi
    done <"${listing}"
    if [[ "${#files[@]}" -eq 0 ]]; then
        printf '%s\n' "fmt: git ls-files listed no Rust file, so the Rust files that a crate pulls in with include! cannot be listed." >&2
        exit 1
    fi
    cargo fmt --all -- "${files[@]}"
    taplo fmt

# Print the pin of slang-compiler-sys as NAME=value lines: the Vulkan SDK version, the Slang build tag, and the Slang commit
[group('rust')]
pin:
    #!/usr/bin/env bash
    set -euo pipefail
    pin_file=crates/slang-compiler-sys/pin.rs
    pattern='^pub const ([A-Z_]+): &str = "([^"]+)";$'
    for name in VULKAN_SDK_VERSION SLANG_BUILD_TAG SLANG_COMMIT; do
        value=""
        while IFS= read -r line; do
            line="${line%$'\r'}"
            if [[ "${line}" =~ ${pattern} && "${BASH_REMATCH[1]}" == "${name}" ]]; then
                value="${BASH_REMATCH[2]}"
            fi
        done <"${pin_file}"
        if [[ -z "${value}" ]]; then
            printf '%s does not define %s as a string constant\n' "${pin_file}" "${name}" >&2
            exit 1
        fi
        printf '%s=%s\n' "${name}" "${value}"
    done

# Regenerate crates/slang-compiler-sys/src/bindings.rs and the list of its C functions, crates/slang-compiler-sys/tests/sys/symbols.rs, from the Slang headers of the Vulkan SDK that VULKAN_SDK names, which needs libclang and rustfmt
[group('rust')]
generate-bindings:
    cargo run --quiet -p bindings-generator

# Check that the checked-in bindings equal a regeneration from the Slang headers of the Vulkan SDK that VULKAN_SDK names, failing when the test cannot run
[group('checks')]
check-bindings:
    {{ slang_library_path }} CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --all-features --no-tests=fail -E 'package(bindings-generator) & binary(generator) & test(=ffi::checked_in_bindings_equal_a_regeneration_from_the_vulkan_sdk)'

# Run the Rust checks: comments, headings, doctest attributes, feature documentation, the Slang call guard, formatting, unused and denied dependencies, Clippy, documentation with and without Slang, doctests, tests, and Miri
[group('checks')]
check-rust: check-rust-comments check-doc-headings check-doctest-attributes check-feature-docs check-slang-call-guard check-fmt check-unused-deps check-deny check-clippy check-doc check-doc-without-slang check-doctest test-gpu check-miri

# Check that the Rust code of the workspace and every Rust file that git tracks or does not ignore, which includes the files that a crate pulls in with include!, is formatted
[group('checks')]
check-fmt:
    #!/usr/bin/env bash
    set -euo pipefail
    listing="$(mktemp)"
    trap 'rm -f "${listing}"' EXIT
    if ! git ls-files -z --cached --others --exclude-standard -- '*.rs' >"${listing}"; then
        printf '%s\n' "check-fmt: git ls-files failed, so the Rust files that a crate pulls in with include! cannot be listed." >&2
        exit 1
    fi
    files=()
    while IFS= read -r -d '' file; do
        if [[ -f "${file}" ]]; then
            files+=("${file}")
        fi
    done <"${listing}"
    if [[ "${#files[@]}" -eq 0 ]]; then
        printf '%s\n' "check-fmt: git ls-files listed no Rust file, so the Rust files that a crate pulls in with include! cannot be listed." >&2
        exit 1
    fi
    cargo fmt --all --check -- "${files[@]}"

# Run Clippy on every target and feature of the workspace, failing on any warning
[group('checks')]
check-clippy:
    CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --all-targets --all-features

# Build the API documentation of the workspace, failing on any rustdoc warning
[group('checks')]
check-doc:
    CARGO_BUILD_WARNINGS=deny cargo doc --workspace --no-deps --all-features

# Build the API documentation of the workspace as docs.rs does, with DOCS_RS set and VULKAN_SDK unset, failing on any rustdoc warning
[group('checks')]
check-doc-without-slang:
    env -u VULKAN_SDK DOCS_RS=1 CARGO_BUILD_WARNINGS=deny cargo doc --workspace --no-deps --all-features

# Run the doctests of the workspace
[group('checks')]
check-doctest:
    {{ slang_library_path }} CARGO_BUILD_WARNINGS=deny cargo test --doc --workspace --all-features

# Run the tests of the default tier, which need no GPU adapter
[group('rust')]
test:
    {{ slang_library_path }} CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --all-features --profile default --no-tests=warn

# Run the tests of the default and GPU tiers, which need any GPU adapter, including lavapipe
[group('rust')]
test-gpu:
    {{ slang_library_path }} CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --all-features --profile gpu --no-tests=warn

# Run every test, including the hardware GPU tier, which needs a hardware GPU
[group('rust')]
test-hw-gpu:
    {{ slang_library_path }} CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --all-features --profile hw-gpu --no-tests=warn

# Run the tests of slang-compiler-sys on the static route, which builds Slang from the source tree that SLANG_SOURCE_DIR names and links it statically, in the target directory target/static, without VULKAN_SDK and without a library path, so the tests pass only when nothing loads the Slang of the Vulkan SDK
[group('rust')]
test-static:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ -z "${SLANG_SOURCE_DIR:-}" ]]; then
        printf '%s\n' "test-static: SLANG_SOURCE_DIR is not set or empty. Set it to the absolute path of a git checkout of Slang at the commit SLANG_COMMIT that just pin prints, with its submodules." >&2
        exit 1
    fi
    env -u VULKAN_SDK -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH CARGO_TARGET_DIR=target/static CARGO_BUILD_WARNINGS=deny cargo nextest run -p slang-compiler-sys --all-features --profile default --no-tests=fail

# Check licences, security advisories, duplicate crates, and crate sources with cargo-deny, which needs network access
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
    {{ slang_library_path }} cargo llvm-cov nextest --workspace --all-features --profile gpu --no-tests=warn --lcov --output-path target/lcov.info
