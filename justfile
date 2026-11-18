set positional-arguments
set shell := ["bash", "-euo", "pipefail", "-c"]

# The assignment that puts the library directory of the Vulkan SDK that VULKAN_SDK names ahead of the library path of the dynamic loader, on Linux and macOS, so that the programs that a recipe runs find the Slang shared library. It is empty on Windows, where PATH finds the DLL, and when VULKAN_SDK is unset or empty. A recipe puts it on the command line of the program it runs, because macOS removes the DYLD_ variables when it starts a protected program such as /bin/bash.
library_path_variable := if os() == "linux" { "LD_LIBRARY_PATH" } else if os() == "macos" { "DYLD_LIBRARY_PATH" } else { "" }
vulkan_sdk := env("VULKAN_SDK", "")
existing_library_path := if library_path_variable == "" { "" } else { env(library_path_variable, "") }
slang_library_path := if library_path_variable == "" { "" } else if vulkan_sdk == "" { "" } else { library_path_variable + "=" + quote(vulkan_sdk / "lib" + if existing_library_path == "" { "" } else { ":" + existing_library_path }) }

# The packages that the vendored build compiles, separated by spaces, which test-vendored and check-vendored select with -p
vendored_packages := "slang-src slang-glslang-sys slang-compiler-sys slang-compiler"

# The Cargo feature that selects the vendored build, which test-vendored and check-vendored enable
vendored_features := "slang-compiler/vendored"

# The environment of the vendored recipes: no Vulkan SDK and no loader path, so nothing can load the Slang of an SDK, and their own target directory, so the vendored build does not replace the build of target/
vendored_environment := "env -u VULKAN_SDK -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH CARGO_TARGET_DIR=target/vendored CARGO_BUILD_WARNINGS=deny"

# The packages that build slang-rhi from source, separated by spaces, which check-rhi and test-rhi select with -p
rhi_packages := "slang-rhi-sys slang-rhi"

# The packages of the next release, separated by spaces, in the order in which they depend on each other, which every recipe of the release reads. Every other member sets publish = false
release_packages := "slang-bindgen slang-src slang-compiler-sys slang-glslang-sys slang-compiler slango slango-codegen slango-macros"

# The address of the hosted documentation, which holds the documentation of each released crate in a directory named after the crate with each hyphen replaced by an underscore
documentation_site := "https://g-whiz.github.io/slang-rust"

# List the available recipes
default:
    @just --list --unsorted

# Run every check: prose, shell, the self-tests of the checkers and of the release scripts, spelling, TOML formatting, workflows, references to project documents, and the Rust checks
[group('checks')]
check: check-prose check-shell check-commit-message-self-test check-prose-self-test check-rust-comments-self-test check-doc-headings-self-test check-doctest-attributes-self-test check-feature-docs-self-test check-slang-call-guard-self-test check-workflow-pins-self-test check-doc-references-self-test release-scripts-self-test check-typos check-toml check-workflows check-workflow-pins check-doc-references check-rust

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

# Generate the Slang bindings from the headers of the Vulkan SDK that VULKAN_SDK names into target/slang-bindgen, which needs libclang, then check that every call of slang-compiler into Slang runs inside the closure of abort_on_exception, in the given files or in every Rust file of crates/slang-compiler/src when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-slang-call-guard *paths:
    #!/usr/bin/env bash
    set -euo pipefail
    root={{ quote(justfile_directory()) }}
    if [[ -z "${VULKAN_SDK:-}" ]]; then
        printf '%s\n' "check-slang-call-guard: VULKAN_SDK is not set or empty, so the Slang bindings whose names the checker reads cannot be generated. Set it to the directory of the Vulkan SDK that holds include/slang." >&2
        exit 2
    fi
    host="$(rustc -vV | sed -n 's/^host: //p')"
    if [[ -z "${host}" ]]; then
        printf '%s\n' "check-slang-call-guard: rustc -vV printed no host target, so the Slang bindings cannot be generated for it." >&2
        exit 2
    fi
    (cd "${root}" && cargo run --quiet -p slang-bindgen -- "${VULKAN_SDK}/include/slang" "${host}" target/slang-bindgen)
    python3 -I "${root}/scripts/check-slang-call-guard" "$@"

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

# Check that code and configuration cite no project document, section, or rule identifier, in the given files or in every Rust, TOML, YAML, and JSON file, the justfile, and every file under scripts/ except the cases of this checker when none are given
[group('checks')]
[no-cd]
[no-exit-message]
check-doc-references *paths:
    @python3 -I "{{ justfile_directory() }}/scripts/check-doc-references" "$@"

# Check that the document reference checker reports exactly the expected findings on its fixtures
[group('checks')]
check-doc-references-self-test:
    @python3 -I scripts/self-test-checker check-doc-references

# Check that the scripts of the release refuse, check, and call cargo, git, gh, and curl as expected, run against stubs of those programs so that nothing reaches crates.io or GitHub
[group('checks')]
release-scripts-self-test:
    @scripts/self-test-release

# Download the Vulkan SDK VERSION, verify it against SHA256, install it, and export it to later steps, run by CI only
[group('ci')]
install-vulkan-sdk VERSION SHA256:
    @scripts/install-vulkan-sdk "$@"

# Check that the Vulkan SDK at VULKAN_SDK provides the Slang headers, library, and slangc of the build tag that recorded-slang prints
[group('checks')]
check-vulkan-sdk:
    @scripts/check-vulkan-sdk

# Print the recorded Slang commit, its build tag, and the name of its clone entry in the shared folder of crates/slang-src/recorded.rs as NAME=value lines
[group('rust')]
recorded-slang:
    #!/usr/bin/env bash
    set -euo pipefail
    recorded_file=crates/slang-src/recorded.rs
    pattern='^const ([A-Z_]+): &str = "([^"]+)";$'
    for name in RECORDED_COMMIT RECORDED_BUILD_TAG RECORDED_CLONE_ENTRY; do
        value=""
        while IFS= read -r line || [[ -n "${line}" ]]; do
            line="${line%$'\r'}"
            if [[ "${line}" =~ ${pattern} && "${BASH_REMATCH[1]}" == "${name}" ]]; then
                value="${BASH_REMATCH[2]}"
            fi
        done <"${recorded_file}"
        if [[ -z "${value}" ]]; then
            printf '%s does not define %s as a string constant\n' "${recorded_file}" "${name}" >&2
            exit 1
        fi
        printf '%s=%s\n' "${name}" "${value}"
    done

# Run the tests and doctests of the vendored build in target/vendored, without VULKAN_SDK and without a loader path, and then the tests of slango-macros that expand the macro, with only the feature vendored of slango-macros, which reaches Slang through slango-codegen, slango, and slang-compiler. Its first run clones the recorded Slang commit, which needs network access, and builds Slang and slang-glslang from source, which takes several minutes
[group('rust')]
test-vendored:
    #!/usr/bin/env bash
    set -euo pipefail
    read -r -a names <<<"{{ vendored_packages }}"
    packages=()
    for name in "${names[@]}"; do
        packages+=(-p "${name}")
    done
    {{ vendored_environment }} cargo nextest run "${packages[@]}" --features "{{ vendored_features }}" --profile default --no-tests=fail
    {{ vendored_environment }} cargo test --doc "${packages[@]}" --features "{{ vendored_features }}"
    {{ vendored_environment }} cargo nextest run -p slango-macros --features slango-macros/vendored --profile default --no-tests=fail -E 'test(/^(bindings|structs)::/)'

# Run Clippy and build the API documentation of the vendored build in target/vendored, without VULKAN_SDK and without a loader path, failing on any warning, and then Clippy on slang-slint with only its feature vendored, which reaches Slang through slango. Its first run clones the recorded Slang commit, which needs network access, and builds Slang and slang-glslang from source, which takes several minutes
[group('checks')]
check-vendored:
    #!/usr/bin/env bash
    set -euo pipefail
    read -r -a names <<<"{{ vendored_packages }}"
    packages=()
    for name in "${names[@]}"; do
        packages+=(-p "${name}")
    done
    {{ vendored_environment }} cargo clippy "${packages[@]}" --features "{{ vendored_features }}" --all-targets
    {{ vendored_environment }} cargo doc "${packages[@]}" --features "{{ vendored_features }}" --no-deps
    {{ vendored_environment }} cargo clippy -p slang-slint --features slang-slint/vendored

# Run the tests of the packages of rhi_packages with the gpu profile, and their doctests, in target/vendored, without VULKAN_SDK and without a loader path, on Linux and on x86_64 Windows, where the crates build. On Windows, the tests compile shaders with dxcompiler.dll, which has to be on PATH. Its first run clones the recorded Slang commit, which needs network access, and builds Slang and slang-rhi from source, which takes several minutes
[group('rust')]
test-rhi:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ os() }}-{{ arch() }}" in
        linux-* | windows-x86_64) ;;
        *)
            printf 'test-rhi: the packages %s build only on Linux and on x86_64 Windows, so the recipe runs only there\n' "{{ rhi_packages }}" >&2
            exit 1
            ;;
    esac
    read -r -a names <<<"{{ rhi_packages }}"
    packages=()
    for name in "${names[@]}"; do
        packages+=(-p "${name}")
    done
    {{ vendored_environment }} cargo nextest run "${packages[@]}" --profile gpu --no-tests=fail
    {{ vendored_environment }} cargo test --doc "${packages[@]}"

# Run Clippy and build the API documentation of the packages of rhi_packages in target/vendored, without VULKAN_SDK and without a loader path, failing on any warning, on Linux and on x86_64 Windows, where the crates build. Its first run clones the recorded Slang commit, which needs network access, and builds Slang and slang-rhi from source, which takes several minutes
[group('checks')]
check-rhi:
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ os() }}-{{ arch() }}" in
        linux-* | windows-x86_64) ;;
        *)
            printf 'check-rhi: the packages %s build only on Linux and on x86_64 Windows, so the recipe runs only there\n' "{{ rhi_packages }}" >&2
            exit 1
            ;;
    esac
    read -r -a names <<<"{{ rhi_packages }}"
    packages=()
    for name in "${names[@]}"; do
        packages+=(-p "${name}")
    done
    {{ vendored_environment }} cargo clippy "${packages[@]}" --all-targets
    {{ vendored_environment }} cargo doc "${packages[@]}" --no-deps

# Run the tests of slang-rhi under Miri on the nightly toolchain, without VULKAN_SDK and without a loader path, in target/miri-rhi, on Linux only, where the crate builds. Its first run clones the recorded Slang commit, which needs network access, and builds Slang and slang-rhi from source for the build scripts, which takes several minutes
[group('checks')]
check-miri-rhi:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ "{{ os() }}" != linux ]]; then
        printf 'check-miri-rhi: the package slang-rhi builds only on Linux, so the recipe runs only there\n' >&2
        exit 1
    fi
    env -u VULKAN_SDK -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH CARGO_TARGET_DIR=target/miri-rhi cargo +nightly miri nextest run -p slang-rhi --no-tests=fail

# Check that the shared folder target/vendored/slang-src holds exactly one Slang clone, the directory src/<23 digits of the commit>-<submodules> that recorded-slang prints as RECORDED_CLONE_ENTRY, and no clone in an older layout, src/<commit> or src/<commit>-<submodules> with the full commit, and that no other directory named slang-src under target/vendored holds a clone in any of these layouts, and print where it is, whether Cargo tagged target/vendored, and the sizes of the folder and the clone
[group('checks')]
check-vendored-shared-folder:
    #!/usr/bin/env bash
    set -euo pipefail
    target=target/vendored
    shared="${target}/slang-src"
    expected="$(just recorded-slang | sed -n 's/^RECORDED_CLONE_ENTRY=//p')"
    if [[ -z "${expected}" ]]; then
        printf 'check-vendored-shared-folder: just recorded-slang printed no RECORDED_CLONE_ENTRY\n' >&2
        exit 1
    fi
    if [[ -f "${target}/CACHEDIR.TAG" ]]; then
        printf 'Cargo tagged %s with CACHEDIR.TAG\n' "${target}"
    else
        printf 'check-vendored-shared-folder: %s holds no CACHEDIR.TAG, so each build unit kept its own clone\n' "${target}" >&2
        exit 1
    fi
    shopt -s nullglob
    commit_pattern="$(printf '%.0s[0-9a-f]' {1..40})"
    prefix_pattern="$(printf '%.0s[0-9a-f]' {1..23})"
    hash_pattern="$(printf '%.0s[0-9a-f]' {1..16})"
    clones() {
        local dir="$1" entry
        for entry in "${dir}/src/"${commit_pattern} "${dir}/src/"${commit_pattern}-${hash_pattern} "${dir}/src/"${prefix_pattern}-${hash_pattern}; do
            if [[ -e "${entry}/tree/.git" ]]; then
                printf '%s\0' "${entry}"
            fi
        done
    }
    entries=()
    while IFS= read -r -d '' entry; do
        entries+=("${entry}")
    done < <(clones "${shared}")
    for entry in "${entries[@]}"; do
        printf 'Clone entry: %s\n' "${entry}"
    done
    if [[ "${#entries[@]}" -ne 1 || "${entries[0]}" != "${shared}/src/${expected}" || ! -f "${entries[0]}/stamp" ]]; then
        printf 'check-vendored-shared-folder: %s must hold exactly one clone entry, the complete entry src/%s, and holds %d, so remove every other entry and build again\n' "${shared}" "${expected}" "${#entries[@]}" >&2
        exit 1
    fi
    printf 'The shared folder is %s\n' "$(cd "${shared}" && pwd)"
    printf 'The clone entry is %s\n' "${entries[0]}"
    du -sh "${shared}"
    du -sh "${entries[0]}"
    listing="$(mktemp)"
    trap 'rm -f "${listing}"' EXIT
    find "${target}" -path "${shared}" -prune -o -type d -name slang-src -print0 >"${listing}"
    others=()
    while IFS= read -r -d '' dir; do
        while IFS= read -r -d '' entry; do
            others+=("${entry}")
        done < <(clones "${dir}")
    done <"${listing}"
    if [[ "${#others[@]}" -ne 0 ]]; then
        printf 'check-vendored-shared-folder: a directory outside the shared folder holds a clone: %s\n' "${others[@]}" >&2
        exit 1
    fi
    printf 'No other directory named slang-src under %s holds a clone\n' "${target}"

# Check that the members without publish = false are the release packages and that each sets its address on the hosted documentation, print the files of each package, and check each packaged .crate file: its size, no symbolic link, its README and licence files, no file under target or .git, and the same files as the version that crates.io already holds, if any. Cargo refuses uncommitted changes to a packaged file, and the recipe reaches crates.io
[group('checks')]
[unix]
check-package:
    #!/usr/bin/env bash
    set -euo pipefail
    read -r -a packages <<<"{{ release_packages }}"
    scripts/check-package "{{ documentation_site }}" "${packages[@]}"

# Check on crates.io that it holds the version of each release package, printing the answer for each, that its documentation field names the hosted documentation, and that this address answers
[group('checks')]
check-release:
    #!/usr/bin/env bash
    set -euo pipefail
    read -r -a packages <<<"{{ release_packages }}"
    scripts/check-release "{{ documentation_site }}" "${packages[@]}"

# Build the hosted documentation of the release packages in target/vendored/doc on the vendored build, without VULKAN_SDK and without a loader path, failing on any warning, check that it holds the index page of each of them, write its own index page, and check that it holds no symbolic link and is at most 1 GB. Its first run clones the recorded Slang commit, which needs network access, and builds Slang and slang-glslang from source, which takes several minutes
[group('release')]
doc-site:
    #!/usr/bin/env bash
    set -euo pipefail
    read -r -a names <<<"{{ release_packages }}"
    packages=()
    for name in "${names[@]}"; do
        packages+=(-p "${name}")
    done
    {{ vendored_environment }} cargo doc --no-deps "${packages[@]}" --features slang-compiler/vendored,slango-macros/vendored
    scripts/check-doc-site target/vendored/doc 976562 "${names[@]}"

# Run cargo publish --dry-run for the given packages, or for every release package when none are given, which packages and compiles each against the packages of the others and uploads nothing, then record the commit and the packages in target/publish-dry-run.commit. Cargo refuses uncommitted changes to a packaged file, and the recipe needs VULKAN_SDK, libclang, network access, and the tools of the vendored build
[group('release')]
[unix]
publish-dry-run *packages:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ $# -eq 0 ]]; then
        read -r -a packages <<<"{{ release_packages }}"
        set -- "${packages[@]}"
    fi
    scripts/publish-dry-run "$@"

# Publish the given packages, or every release package when none are given, to crates.io without a verification build, only by the owner with CARGO_REGISTRY_TOKEN set for this one run, and only at the commit of origin/main whose dry run recorded the same packages. It skips every version that crates.io already holds, so a rerun completes a partial publish, publishes one package at a time, and waits as the rate limits of crates.io require, which can take ten minutes for each new crate past the fifth
[group('release')]
[unix]
publish *packages:
    #!/usr/bin/env bash
    set -euo pipefail
    if [[ $# -eq 0 ]]; then
        read -r -a packages <<<"{{ release_packages }}"
        set -- "${packages[@]}"
    fi
    scripts/publish "$@"

# Create and push the tag v<version> on HEAD, which must equal origin/main, and create its GitHub release, marked as a pre-release when the version holds a hyphen, with the release notes in the file NOTES, which deploys the hosted documentation. Only with the owner's go-ahead
[group('release')]
[no-cd]
tag-release NOTES:
    #!/usr/bin/env bash
    set -euo pipefail
    notes="$1"
    if [[ "${notes}" != /* ]]; then
        notes="${PWD}/${notes}"
    fi
    root={{ quote(justfile_directory()) }}
    cd "${root}"
    read -r -a packages <<<"{{ release_packages }}"
    scripts/tag-release "${notes}" "${packages[@]}"

# Yank the version VERSION of the crate PACKAGE on crates.io, only by the owner with CARGO_REGISTRY_TOKEN set for this one run
[group('release')]
yank PACKAGE VERSION:
    @scripts/yank "$@"

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

# Run the Rust checks: comments, headings, doctest attributes, feature documentation, the Slang call guard, formatting, unused and denied dependencies, Clippy, documentation, doctests, tests, and Miri
[group('checks')]
check-rust: check-rust-comments check-doc-headings check-doctest-attributes check-feature-docs check-slang-call-guard check-fmt check-unused-deps check-deny check-clippy check-doc check-doctest test-gpu check-miri

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

# Run Clippy on every target of the workspace, failing on any warning
[group('checks')]
check-clippy:
    CARGO_BUILD_WARNINGS=deny cargo clippy --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --all-targets

# Build the API documentation of the workspace, failing on any rustdoc warning
[group('checks')]
check-doc:
    CARGO_BUILD_WARNINGS=deny cargo doc --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --no-deps

# Run the doctests of the workspace
[group('checks')]
check-doctest:
    {{ slang_library_path }} CARGO_BUILD_WARNINGS=deny cargo test --doc --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi

# Run the tests of the default tier, which need no GPU adapter
[group('rust')]
test:
    {{ slang_library_path }} CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --profile default --no-tests=warn

# Run the tests of the default and GPU tiers, which need any GPU adapter, including lavapipe
[group('rust')]
test-gpu:
    {{ slang_library_path }} CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --profile gpu --no-tests=warn

# Run every test, including the hardware GPU tier, which needs a hardware GPU
[group('rust')]
test-hw-gpu:
    {{ slang_library_path }} CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --profile hw-gpu --no-tests=warn

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
    cargo +nightly miri nextest run --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --no-tests=warn

# Check that every combination of Cargo features compiles, except slango without a backend, run by CI only
[group('checks')]
check-features:
    CARGO_BUILD_WARNINGS=deny cargo hack check --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --exclude slango --all-targets --feature-powerset --exclude-features vendored
    CARGO_BUILD_WARNINGS=deny cargo hack check -p slango --all-targets --feature-powerset --exclude-features vendored --exclude-no-default-features

# Check that the workspace builds with the toolchain of its declared rust-version, run by CI only
[group('checks')]
check-msrv:
    CARGO_BUILD_WARNINGS=deny cargo hack check --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --all-targets --rust-version

# Check the release packages with a library target, other than slang-src and slang-glslang-sys, for breaking changes against their last release on crates.io, with the default features of each, run by CI only. It fetches each baseline from crates.io, which needs network access, and builds each crate twice
[group('checks')]
check-semver:
    cargo semver-checks --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --default-features

# Write a test coverage report to target/lcov.info, run by CI only
[group('rust')]
coverage:
    {{ slang_library_path }} cargo llvm-cov nextest --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --profile gpu --no-tests=warn --lcov --output-path target/lcov.info
