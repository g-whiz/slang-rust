# Tooling Design

| Field | Value |
| --- | --- |
| Status | Living document |
| Date | 2026-11-12 |
| Applies to | The repository's configuration, recipes, hooks, and continuous integration |

## Abstract

This document specifies how the deterministic checks of the slang-rust repository are configured and run. It defines the `just` recipes, the aggregate check that runs before every merge, the nextest profiles for the test tiers, and the git hooks. It also specifies the custom checkers that move rules from review to a tool and the continuous integration workflow that runs all of these checks.

## 1. Introduction

### 1.1. Problem

The coding standard specifies a strict lint configuration and a set of tools, and the writing guide specifies rules for documentation. Until a tool is installed and run, every rule it would enforce is a judgement rule that a reviewer checks by hand. The repository's central principle forbids that wherever a tool can do the work.

### 1.2. Purpose

This document turns the specifications of the standards documents into installed configuration and runnable checks. It follows the principle that Hard Rule 4 of `AGENTS.md` and Section 3.5 of the writing guide state: anything a deterministic tool can check is checked by a tool and never by a reviewer.

### 1.3. Audience

The primary readers are the coding agents that add or change a recipe, a checker, a hook, a tool configuration, or a continuous integration (CI) job. The secondary readers are reviewers, and the owner of the repository, who approves its decisions.

### 1.4. Scope

This document covers the tool configuration files, the workspace manifest fields that are not lints, the recipes, the nextest configuration, the hooks, the custom checkers, the CI workflow, the cargo-deny policy, publishing readiness, and the requirements on the container image.

### 1.5. Non-Goals

Each fact has one owner, as Section 4.5 of the writing guide requires, and this document cites the owner instead of restating the fact.

- Section 4 of `docs/rust-coding-standards.md` owns the lint tables, the contents of `clippy.toml`, the list of tools with what each one checks, and the exact cargo command lines of Section 4.8. This document says which recipe runs which command, and when.
- `docs/design/architecture.md` owns the crates, their layering, the directories of the workspace members, the roadmap, the test tiers of its Section 11, the platforms, and the publishing policy. This document defines only the mechanisms that implement them.
- `AGENTS.md` owns the hard rules and the development workflow.

This document does not design the binding generator, the build script of `slang-compiler-sys`, or the application programming interface (API) of any crate, which the design documents of the crates own. It does not specify the contents of the container image beyond the requirements of Section 11, because the repository slang-rs-env defines the image.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that these documents define, and calls them the writing guide, the coding standard, and the architecture design:

- Section 2.2 of `docs/writing-documentation.md`, such as agent, reviewer, enforced rule, and judgement rule.
- Section 2.2 of `docs/rust-coding-standards.md`, such as lint, workspace lint table, and the check.
- Section 2.1 of `docs/design/architecture.md`, such as adapter, software adapter, milestone, nextest profile, and pin.

It adds these terms:

- action: A reusable unit of steps for a GitHub Actions workflow, published in a git repository and referenced from a workflow.
- aggregate recipe: A recipe that only runs other recipes, such as `check`.
- commit SHA: The full 40-character hexadecimal hash that names one git commit.
- container image: The image, defined in the repository slang-rs-env, from which the development container runs.
- custom checker: A script in this repository that checks one rule of the writing guide, the coding standard, or a design document that no installed tool checks (Section 7).
- development container: The container in which agents work on this repository, run from the container image.
- GitHub Actions: The CI service of GitHub.
- hook: A git hook that pre-commit installs from `.pre-commit-config.yaml` and runs at a git event, such as a commit.
- hosted runner: A runner that GitHub provides.
- image rebuild: A rebuild of the container image from its definition in the repository slang-rs-env.
- job: A set of steps in a workflow that runs on one runner.
- matrix: A job definition that runs once for each value of a list, such as each platform.
- recipe: A named command in the root `justfile`, run with `just <recipe>`.
- recipe group: The group that the `[group]` attribute gives a recipe, under which `just --list` shows it.
- runner: The machine that runs a job.
- Software Package Data Exchange (SPDX) licence identifier: The standard short name of a licence, such as `GPL-3.0-or-later`, which Cargo's `license` field takes.
- workflow: A YAML Ain't Markup Language (YAML) file under `.github/workflows/` that defines the jobs that GitHub Actions runs.

## 4. Recipes

### 4.1. Rules

The `justfile` stays one flat file with `[group]` attributes, a help comment on every recipe, and shell that is `shellcheck`-clean, as Hard Rule 8 of `AGENTS.md` requires. It has four groups:

- `checks`: The checks, their self-tests, `check-vulkan-sdk`, and `hooks-install`.
- `rust`: The recipes that change files, print the pin, or run tests and are not checks.
- `git`: `merge` and `worktree-prune`.
- `ci`: The recipes that only the CI workflow runs, which is `install-vulkan-sdk`.

The recipe `default` has no group. Section 17.1 lists every recipe that takes parameters.

TOOL-6. An agent MUST add each recipe of Section 4.2 with the name, the group, and the membership of `check` that the table gives, and MUST update the table in the same change when one of them has to differ. Agents, hooks, and CI call recipes by name, and a recipe that differs from this table breaks every caller that follows it.

TOOL-7. A recipe that runs a command of Section 4.8 of the coding standard MUST run that command as Section 4.8 gives it, and MUST add only these four things:

1. The option `--profile` with a nextest profile of Section 5.
2. The arguments that Section 4.8 names for the recipe `coverage`.
3. The environment of Section 10.
4. In a recipe that runs compiled code which loads Slang, the library path assignment of the `justfile` variable `slang_library_path` at the start of the command line, which SCS-32 of `docs/design/slang-compiler-sys.md` defines.

An agent that has to change the command MUST change Section 4.8 in the same change. Two command lines for one check diverge, and an agent that runs the documented command then gets a result different from the recipe's. The library path assignment changes only where the dynamic loader looks for the Slang shared library, not what the command checks.

TOOL-7 permits two departures:

- A recipe that selects named tests with a filter expression, and that has to fail when the filter selects no test, MAY add that filter to the nextest command and pass `--no-tests=fail` in place of `--no-tests=warn`. With `--no-tests=warn`, such a recipe passes without running any test when its test is renamed or is not compiled on the platform. `check-bindings` is the only recipe that uses this departure.
- `test-static`, whose purpose is to prove the static link of Slang (SCS-49 of `docs/design/slang-compiler-sys.md`), MAY run the nextest command for the one package `slang-compiler-sys` with `-p`. It MAY remove `VULKAN_SDK`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` with `env -u`, set `CARGO_TARGET_DIR=target/static`, and pass `--no-tests=fail` without a filter. Without these, the regeneration test would fail for lack of `VULKAN_SDK`, a dynamically linked test binary could still find the Slang of the SDK, and the static build would replace the dynamic build in `target/`.

The formatting command of Section 4.8 ends with the list of every Rust file that git tracks or does not ignore. `fmt` and `check-fmt` compute that list with `git ls-files`, and it is part of the command, not an addition.

TOOL-8. The recipe `check` MUST pass in the development container without a hardware graphics processing unit (GPU) and without a display, and MUST NOT run a recipe that needs either. An agent runs `check` before every merge in the container, as the Validating Changes section of `AGENTS.md` requires, and CI runs it on a hosted runner that is not required to provide either.

TOOL-9. An agent MUST NOT add a recipe for a tool that the container image does not provide until an image rebuild adds the tool, because the recipe would fail `check` for every agent until then.

TOOL-30. `taplo.toml` MUST set `indent_string = "    "` in its `[formatting]` table and `exclude = ["target/**", ".worktrees/**"]`. `typos.toml` MUST accept each correct word that typos reports in the repository.

Otherwise `check-toml` and `check-typos` fail on files that are correct. taplo indents with two spaces by default, which disagrees with the four spaces that `.editorconfig` sets for `*.toml`. It does not read `.gitignore`, so without the exclusions it also checks the build directory and every worktree.

typos reports two kinds of correct text in the repository: a two-letter fragment of the regular expression that matches the shell names in the shebang checks of `scripts/justfile-recipes` and `scripts/check-shell`, and the surname of a cited author in `docs/rust-references.md`. `typos.toml` accepts them with two entries in the `extend-ignore-re` list of its `[default]` table. One matches the whole regular expression, and the other matches the author's full name.

typos skips only the text that an entry matches, so it still checks the same letters anywhere else, which accepting the words in `[default.extend-words]` would not do. A document that names this configuration describes the two findings instead of spelling them out, because typos reports such a document too. typos reads its configuration from `typos.toml`, `_typos.toml`, or `.typos.toml`, and the repository uses the first name.

`typos.toml` also has a `[default.extend-identifiers]` table with three entries. Each entry accepts one whole identifier of the generated bindings that the Slang headers misspell and that the bindings keep, because each generated name has to match the header. Section 5.10 of `docs/design/slang-compiler-sys.md` names the three identifiers and explains why entries for whole identifiers are the narrowest exclusion (SCS-14).

No file is excluded from typos.

`typos.toml` sets no locale. typos cannot enforce Canadian spelling, because with its default settings it accepts both `colour` and `color`, so `scripts/check-prose` and its word list keep that job (Section 8 of the writing guide).

The repository has no `rustfmt.toml`, because Section 4.8 of the coding standard specifies the default rustfmt settings.

### 4.2. Recipe Table

This table lists every recipe, what it runs, and whether `check` runs it. The commands of Section 4.8 of the coding standard are named by their purpose, and Section 4.8 gives each command line.

| Recipe | Group | Runs | In `check` |
| --- | --- | --- | --- |
| `default` | none | `just --list --unsorted` | No |
| `check` | `checks` | The recipes that Section 4.3 lists | Not applicable |
| `check-prose` | `checks` | `scripts/check-prose`, extended to API documentation by Section 7.6 | Yes |
| `check-prose-self-test` | `checks` | The self-test of Section 7.1 for `scripts/check-prose` | Yes |
| `check-shell` | `checks` | `scripts/check-shell` | Yes |
| `check-commit-message` | `checks` | `scripts/check-commit-message` on one file, for the `commit-msg` hook | No |
| `check-commit-message-self-test` | `checks` | The self-test of the commit message checker | Yes |
| `hooks-install` | `checks` | `pre-commit install` | No |
| `merge` | `git` | `scripts/merge-branch`, which squash-merges a branch onto `main` with the subject and body of every commit of the branch in the message, pushes `main`, and deletes the branch on `origin` when its tip there is part of the squash (Section 17.7) | No |
| `worktree-prune` | `git` | `scripts/worktree-prune` | No |
| `fmt` | `rust` | `cargo fmt --all` with the list of every Rust file that git tracks or does not ignore after `--`, which applies the formatting that the formatting command of Section 4.8 checks, also to the files that a crate includes with `include!`, and `taplo fmt`, which formats every Tom's Obvious Minimal Language (TOML) file that `taplo.toml` does not exclude. It lists the files with `git ls-files -z --cached --others --exclude-standard -- '*.rs'` and keeps those that exist. It fails with "fmt: git ls-files failed, so the Rust files that a crate pulls in with include! cannot be listed." when `git ls-files` fails, and with "fmt: git ls-files listed no Rust file, so the Rust files that a crate pulls in with include! cannot be listed." when the list is empty | No |
| `check-rust` | `checks` | The recipes that Section 4.3 lists | Yes |
| `check-fmt` | `checks` | The formatting command of Section 4.8, with the list of files that `fmt` computes in the same way. It fails with the messages of `fmt`, which start with "check-fmt:" instead, when `git ls-files` fails or lists no Rust file | Yes, through `check-rust` |
| `check-clippy` | `checks` | The Clippy command of Section 4.8 | Yes, through `check-rust` |
| `check-doc` | `checks` | The documentation command of Section 4.8 | Yes, through `check-rust` |
| `check-doctest` | `checks` | The doctest command of Section 4.8, after the library path assignment of SCS-32 of `docs/design/slang-compiler-sys.md` | Yes, through `check-rust` |
| `test` | `rust` | The nextest command of Section 4.8 with the `default` profile, after the library path assignment | No, because `test-gpu` runs the same tests |
| `test-gpu` | `rust` | The nextest command of Section 4.8 with the `gpu` profile, after the library path assignment | Yes, through `check-rust` |
| `test-hw-gpu` | `rust` | The nextest command of Section 4.8 with the `hw-gpu` profile, after the library path assignment | No, because it needs a hardware GPU |
| `check-deny` | `checks` | The cargo-deny command of Section 4.8 | Yes, through `check-rust` |
| `check-rust-comments` | `checks` | The comment checker of Section 7.2 | Yes, through `check-rust` |
| `check-doc-headings` | `checks` | The heading checker of Section 7.3 | Yes, through `check-rust` |
| `check-doctest-attributes` | `checks` | The doctest attribute checker of Section 7.4 | Yes, through `check-rust` |
| `check-feature-docs` | `checks` | The feature documentation checker of Section 7.5 | Yes, through `check-rust` |
| `check-rust-comments-self-test`, `check-doc-headings-self-test`, `check-doctest-attributes-self-test`, `check-feature-docs-self-test` | `checks` | The self-test of Section 7.1 for each checker of Sections 7.2 to 7.5 | Yes |
| `check-slang-call-guard` | `checks` | The Slang call guard checker of Section 7.9 | Yes, through `check-rust`, directly after `check-feature-docs` |
| `check-slang-call-guard-self-test` | `checks` | The self-test of Section 7.1 for the Slang call guard checker | Yes |
| `check-workflow-pins` | `checks` | The workflow pin checker of Section 7.8 | Yes |
| `check-workflow-pins-self-test` | `checks` | The self-test of Section 7.1 for the workflow pin checker | Yes |
| `pin` | `rust` | Prints the pin of `crates/slang-compiler-sys/pin.rs` as `NAME=value` lines, and fails when a constant is missing (Section 8.1 of `docs/design/slang-compiler-sys.md`). It runs on bash 3.2, which has no associative arrays and is the bash that a recipe gets on the hosted macOS runner. It accepts lines that end in CRLF, because a user's own git settings can produce them (Section 17.3) | No |
| `install-vulkan-sdk` | `ci` | `scripts/install-vulkan-sdk` with the SDK version and the expected SHA-256 checksum, which downloads, verifies, and installs the Vulkan SDK on a GitHub Actions runner and exports it to the later steps of the job (Section 8.3). Only the CI workflow runs it, and it refuses to run outside a runner | No, because it runs only on a CI runner |
| `check-vulkan-sdk` | `checks` | `scripts/check-vulkan-sdk`, without arguments, which reads the build tag of the pin with `just pin` and checks that the SDK at `VULKAN_SDK` provides that build of Slang (Section 8.3) | No, every CI job runs it after it installs the SDK |
| `check-bindings` | `checks` | The nextest command of Section 4.8, after the library path assignment, with a filter expression that selects only the regeneration test of TEST-8 of the coding standard, and with `--no-tests=fail` in place of `--no-tests=warn`, the first departure that TOOL-7 permits, so that it fails when the filter selects no test, as on Windows and macOS (Section 7.6 of `docs/design/slang-compiler-sys.md`). It carries the assignment because nextest starts every test binary to list its tests, including the binary of `slang-compiler-sys`, which loads Slang | No, because `test-gpu` runs the same test on Linux |
| `generate-bindings` | `rust` | The binary of the binding generator, through `cargo run`, which overwrites both generated files, `crates/slang-compiler-sys/src/bindings.rs` and `crates/slang-compiler-sys/tests/sys/symbols.rs`, from the Slang headers of the SDK at `VULKAN_SDK` and needs libclang and rustfmt | No |
| `check-doc-without-slang` | `checks` | The documentation command of Section 4.8 with `DOCS_RS` set to `1` and `VULKAN_SDK` removed from the environment, as `env -u VULKAN_SDK DOCS_RS=1 CARGO_BUILD_WARNINGS=deny cargo doc --workspace --no-deps --all-features` (Section 10) | Yes, through `check-rust`, directly after `check-doc` |
| `test-static` | `rust` | The nextest command of Section 4.8 with the `default` profile for the package `slang-compiler-sys` only, with `VULKAN_SDK`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` removed, `CARGO_TARGET_DIR=target/static`, and `--no-tests=fail`, the second departure that TOOL-7 permits, as `env -u VULKAN_SDK -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH CARGO_TARGET_DIR=target/static CARGO_BUILD_WARNINGS=deny cargo nextest run -p slang-compiler-sys --all-features --profile default --no-tests=fail`. The build script then builds Slang from the source tree that the developer's `SLANG_SOURCE_DIR` names and links it statically, so the tests pass only when nothing loads the Slang of the SDK. When `SLANG_SOURCE_DIR` is unset or empty, it fails with "test-static: SLANG_SOURCE_DIR is not set or empty. Set it to the absolute path of a git checkout of Slang at the commit SLANG_COMMIT that just pin prints, with its submodules." (Section 9.3 of `docs/design/slang-compiler-sys.md`) | No, because it builds Slang from source |
| `check-typos` | `checks` | The typos command of Section 4.8 on the whole repository, with `typos.toml` (TOOL-30) | Yes |
| `check-toml` | `checks` | The taplo command of Section 4.8, with `taplo.toml` (TOOL-30) | Yes |
| `check-workflows` | `checks` | The actionlint command of Section 4.8, which checks every workflow | Yes |
| `check-unused-deps` | `checks` | The cargo machete command of Section 4.8 | Yes, through `check-rust` |
| `check-miri` | `checks` | The Miri command of Section 4.8, for the tests that the nextest profile `default-miri` selects | Yes, through `check-rust` |
| `check-features` | `checks` | The cargo hack command of Section 4.8 for the feature powerset, which includes each single feature | No, the CI job `ci-only` runs it |
| `check-msrv` | `checks` | The MSRV command of Section 4.8, which checks the workspace with `cargo hack check` and the toolchain that `rust-version` names | No, the CI job `ci-only` runs it, and its first local run downloads that toolchain (Section 17.3) |
| `check-semver` | `checks` | Planned for M7, with the first release, and not yet in the `justfile`: cargo semver-checks against the last published version of each published crate (Section 10) | No, the planned CI job `semver` runs it |
| `coverage` | `rust` | The cargo llvm-cov command of Section 4.8 with the arguments that Section 4.8 names for it, after the library path assignment, which write the LCOV report `target/lcov.info`. It prints no coverage summary, and it is not a gate | No, the CI job `coverage` runs it (Section 8.1) |

`check-miri` runs Miri through nextest, for the reason that Section 4.8 of the coding standard gives. Under Miri, nextest uses the profile named `default-miri`, which `.config/nextest.toml` defines (Section 5).

Miri emulates some functions of libc and libm, but it cannot call an arbitrary native library such as Slang. `check-miri` therefore covers pure Rust logic, such as marshalling and layout code, and the unsafe code that TEST-7 of the coding standard tests against a fake implementation.

The filter of `default-miri` excludes every test whose path contains a module named `ffi`, `gpu`, or `hw_gpu`, selected by whole module path segments as in Section 5. The tests that call Slang or libclang sit in modules named `ffi` (SCS-33 of `docs/design/slang-compiler-sys.md`), and so do the tests that start a process, which Miri cannot do.

No Cargo feature selects the static build of Slang, which the environment variable `SLANG_SOURCE_DIR` selects (Section 9.1 of `docs/design/slang-compiler-sys.md`). The checks that pass `--all-features` and the feature matrix of `check-features` therefore never build Slang from source.

The library path assignment is the value of the `justfile` variable `slang_library_path`, which SCS-32 of `docs/design/slang-compiler-sys.md` defines. It sets `LD_LIBRARY_PATH` on Linux and `DYLD_LIBRARY_PATH` on macOS to `$VULKAN_SDK/lib` followed by any earlier value. It is empty on Windows and when `VULKAN_SDK` is unset or empty.

Section 10 of `docs/design/slang-compiler-sys.md` records why the assignment is on the command line and not exported.

The recipes that run compiled code which loads Slang put the assignment at the start of their command line: `check-doctest`, `test`, `test-gpu`, `test-hw-gpu`, `check-bindings`, and `coverage`. These recipes do not carry it:

- `check-miri`, which runs no native code.
- The recipes that only compile, such as `check-clippy`, `check-doc`, `check-doc-without-slang`, `check-features`, and `check-msrv`.
- `test-static`, which removes the loader path variables to prove the static link (TOOL-7).

### 4.3. Aggregate Recipes

`check` runs these recipes, in this order:

1. `check-prose`
2. `check-shell`
3. `check-commit-message-self-test`
4. `check-prose-self-test`
5. `check-rust-comments-self-test`
6. `check-doc-headings-self-test`
7. `check-doctest-attributes-self-test`
8. `check-feature-docs-self-test`
9. `check-slang-call-guard-self-test`
10. `check-workflow-pins-self-test`
11. `check-typos`
12. `check-toml`
13. `check-workflows`
14. `check-workflow-pins`
15. `check-rust`

`check-workflows` can run in `check` only while the repository has a workflow, because actionlint exits with status 3 when it finds none. `check` needs network access (Section 17.6).

`check-rust` runs `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs`, `check-slang-call-guard`, `check-fmt`, `check-unused-deps`, `check-deny`, `check-clippy`, `check-doc`, `check-doc-without-slang`, `check-doctest`, `test-gpu`, and `check-miri`, in that order, so that the checks that do not compile the workspace report first. Every recipe of `check-rust` that compiles the workspace needs the pinned SDK at `VULKAN_SDK`, because the build script of `slang-compiler-sys` stops the build without it. The exception is `check-doc-without-slang`, which removes `VULKAN_SDK` and sets `DOCS_RS`.

`check-rust` runs `test-gpu` and not `test`, because the `gpu` profile selects every test that the `default` profile selects (Section 5). `test-gpu` runs in the development container on lavapipe, so `check` meets TOOL-8.

On Linux, `test-gpu` also runs the regeneration test of Section 7.6 of `docs/design/slang-compiler-sys.md`, so `check` needs `VULKAN_SDK`, libclang, and rustfmt there. The development container provides them, and the Linux jobs of CI install them (Section 8.1). A test in a `gpu` module that finds no adapter fails, as ARCH-23 of the architecture design requires, so `check` fails on a machine without any adapter.

## 5. Test Tiers in nextest

The file `.config/nextest.toml` defines three nextest profiles that implement the test tiers of Section 11 of the architecture design. The tier of a test is the name of a module in its path, as ARCH-22 of the architecture design defines. nextest reads that file, and a recipe selects a profile with the option `--profile` (TOOL-7).

TOOL-10. The nextest configuration MUST define the profiles `default`, `gpu`, and `hw-gpu` with these selections:

- `default`: every test whose path contains no module named `gpu` or `hw_gpu`.
- `gpu`: every test that `default` selects, and every test whose path contains a module named `gpu` and no module named `hw_gpu`.
- `hw-gpu`: every test.

Each profile other than `default` MUST set its own `default-filter`, because a profile that selects a test of the wrong tier either fails on a runner without the hardware the test needs or leaves a test unrun everywhere.

A profile inherits `default-filter` from `default` when it sets none, so an `hw-gpu` profile without the filter `all()` would run the default tier only.

A module matches only when a whole segment of the test's module path equals `gpu` or `hw_gpu`. The name of the test function itself is not a module and does not match. This table shows how the profiles treat normal and abnormal test paths, written as the path of the test inside its test binary:

| Test path | Tier | `default` | `gpu` | `hw-gpu` |
| --- | --- | --- | --- | --- |
| `session::create_reads_version` | Default | Yes | Yes | Yes |
| `gpu::dispatch_writes_buffer` | GPU | No | Yes | Yes |
| `hw_gpu::renders_on_hardware` | Hardware GPU | No | No | Yes |
| `reflection::gpu::binds_buffer` | GPU | No | Yes | Yes |
| `gpu_buffers::layout_is_packed` | Default, because `gpu_buffers` is not `gpu` | Yes | Yes | Yes |
| `tests::gpu` | Default, because `gpu` is the test function | Yes | Yes | Yes |

`.config/nextest.toml` defines the three profiles as follows:

```toml
[profile.default]
default-filter = "not test(/(^|::)(gpu|hw_gpu)::/)"
retries = 0

[profile.gpu]
default-filter = "not test(/(^|::)hw_gpu::/)"

[profile.hw-gpu]
default-filter = "all()"
```

It also defines the profile `default-miri`, which nextest uses under Miri (Section 4.2), with the filter that SCS-33 of `docs/design/slang-compiler-sys.md` requires. It selects every test that `default` selects, except those whose path contains a module named `ffi`:

```toml
[profile.default-miri]
default-filter = "not test(/(^|::)(ffi|gpu|hw_gpu)::/)"
```

The regular expression matches `gpu` or `hw_gpu` only at the start of the test path or after `::`, and only when `::` follows it. The trailing `::` makes the match a whole module segment.

A test function named `gpu` is the last segment of its path and has no `::` after it, and a module named `gpu_buffers` has `_buffers` after `gpu`, so neither matches. The filter of `default-miri` matches `ffi` in the same way.

A filter that the command line adds with `-E` is intersected with the default filter of the profile, so it narrows the selection of the profile and never widens it. `check-bindings` adds such a filter to the `default` profile to select the regeneration test alone (Section 4.2).

TOOL-11. A profile MUST NOT retry a failed test, because a retry turns an intermittent failure on an adapter into a pass, which hides the failure in the same way that ARCH-23 of the architecture design forbids a test to hide a missing adapter.

Zero retries is the default of nextest, and the `default` profile sets `retries = 0` explicitly so that the configuration states this rule instead of relying on the default. The other profiles inherit the value.

TOOL-12. The recipes `test`, `test-gpu`, and `test-hw-gpu` MUST pass when their profile selects no test. An empty selection is not a test failure, and a tier can hold no test. nextest 0.9.151 fails such a run with the message "error: no tests to run" and exit status 4.

Every nextest command of Section 4.8 of the coding standard therefore passes `--no-tests=warn`, with which an empty run passes and nextest still prints that no test ran. A recipe keeps that option, except in the two departures that TOOL-7 permits.

Each `gpu` test of `slango` prints the adapter that it ran on. This override in the `default` profile keeps that output in the log, so that the log of a CI run records the adapter of each runner. It changes no filter:

```toml
[[profile.default.overrides]]
filter = "package(slango) & test(/^gpu::/)"
success-output = "final"
```

nextest does not run doctests, so `check-doctest` runs them, as Section 4.8 of the coding standard states.

## 6. Git Hooks

The hooks in `.pre-commit-config.yaml` call `just` recipes, because Hard Rule 8 of `AGENTS.md` makes every check a recipe.

TOOL-13. A hook of the `pre-commit` stage MUST run only checks that do not compile the workspace. A commit hook that compiles the workspace makes every commit wait for a build, and Clippy, the tests, and the documentation build already run in `check` before every merge and in CI.

These local hooks run in the `pre-commit` stage:

| Hook | Runs when |
| --- | --- |
| `check-prose` | A staged Markdown or Rust file changes. It passes the paths of those files to the recipe |
| `check-shell` | The `justfile` or a file under `scripts/` changes |
| `check-fmt` | A staged Rust file changes |
| `check-typos` | Every commit |
| `check-toml` | A staged TOML file changes |
| `check-workflows` | A staged file under `.github/` changes |
| `check-workflow-pins` | A staged file under `.github/` changes |

Each of these hooks except `check-prose` runs its recipe on the whole repository, not on the staged files only. The `pre-commit` stage also runs hooks of the repository `pre-commit-hooks`, pinned to a commit SHA, which check YAML and JSON syntax, file endings, trailing whitespace, merge conflict markers, private keys, and large files. The `commit-msg` hook runs `check-commit-message`, and the `post-merge` hook runs `worktree-prune`.

The Claude Code hook in `.claude/settings.json` runs `scripts/claude-hook-prose` after every edit. The script runs `check-prose` on the edited file when the file is inside the repository and its name ends in `.md` or `.rs`, and ignores every other file. It reports the result in one of two ways:

- When `check-prose` reports findings, the script prints them and exits with status 2, which Claude Code treats as a blocking error and reports with the findings.
- When `check-prose` fails for another reason, such as a Rust file that cannot be lexed because it is only partly edited (Section 7.1), the script prints the message and exits with status 1, which Claude Code treats as a non-blocking error.

The allow list of `.claude/settings.json` lets Claude Code run these commands without asking, each as `just <recipe>` unless shown otherwise:

- `just --list` and `just check`.
- `check-rust` and each recipe that it runs, except `check-doc-without-slang` and `check-slang-call-guard`.
- Each custom checker and its self-test, except `check-slang-call-guard` and `check-slang-call-guard-self-test`.
- `check-shell`, `check-commit-message` and its self-test, `check-typos`, `check-toml`, `check-workflows`, `check-features`, `check-bindings`, `check-vulkan-sdk`, `pin`, `test`, `test-gpu`, and `test-hw-gpu`.
- `shellcheck` and `git status`.

Claude Code asks before it runs any other recipe, such as `fmt`, `generate-bindings`, `merge`, `check-msrv`, `coverage`, `test-static`, `check-doc-without-slang`, `check-slang-call-guard`, or `check-slang-call-guard-self-test`. `just check` and `just check-rust` still run those that they include without asking, because the allow list holds those two commands.

## 7. Custom Checkers

Several rules of the writing guide, the pinning rule of Section 8.2, and SLC-32 of `docs/design/slang-compiler.md` can be checked deterministically, but no installed tool checks them. actionlint, for example, does not check the pinning rule, and no lint checks SLC-32.

This section specifies a small custom checker for each. The "Checked by" statements of Section 7 of the writing guide, its Section 7.12, and Section 8.2 of this document name these checkers.

### 7.1. Conventions

TOOL-14. Each custom checker MUST be a script under `scripts/` with a recipe of the same name in the `checks` group. It MUST follow the conventions that the usage text of `scripts/check-prose` states, which are:

- One `path:line: message` line for each finding.
- Exit status 0 without findings, 1 with findings, and 2 when a file cannot be read or the setup is broken, after checking every readable file.
- Usage text on `-h`.

With no paths, the checker checks every relevant file in the git repository, tracked or not ignored. A checker that differs breaks the hooks and the recipes, which rely on these exit statuses to tell a finding from a broken setup.

The one departure is a checker that needs an input to know what to look for, such as the generated bindings of Section 7.9. It exits with status 2 before it checks any file when that input is missing or unusable.

A file that is not valid UTF-8 is a file that cannot be read, so a checker reports it on standard error and exits with status 2. The exception is a Markdown file given to `check-prose`, in which an invalid byte is a finding.

A Rust file that ends inside a string literal, a raw string literal, a block comment, or a character literal also cannot be read. For a checker that reads documentation comments, the same holds for a file that ends inside an attribute, because such a checker skips attributes to group the comments of one item. The checker prints `<checker>: cannot read <file>: unterminated <kind> at line N` on standard error, where N is the line of the opening delimiter, and exits with status 2.

Unterminated input is a read error and not a silent end of the file. A lexer that read the rest of the file as part of the literal would report nothing after it, and the check would pass while it checked nothing there.

Other files given in the same run are still checked. While a file is only partly edited, unterminated input is normal, and Section 6 says how the Claude Code hook reports it.

Each checker prints every finding and every error message through the helper `emit` of `scripts/rust_lexer.py`, which escapes every character outside printable ASCII, both in file paths and in file content that a message repeats. The escapes are these:

- A newline, a carriage return, and a tab print as `\n`, `\r`, and `\t`.
- Every other character up to U+00FF prints as `\xNN`.
- Every character above U+00FF prints as `\uNNNN`, or as `\UNNNNNNNN` above U+FFFF.

A checked file or a file name therefore cannot send a terminal control sequence to the terminal that shows the output (Section 16). A backslash prints unchanged, so a character that the helper escapes and the same escape written literally in a file print alike.

A checker reports a failed git call and a file that cannot be read in its own words, which do not depend on the version of git or on the operating system. The self-tests pin the exact text of each error and would otherwise fail on another machine for an unrelated reason.

A failed git call reads `not inside a git repository`, `git <subcommand> exited with status N`, or `cannot run git: <cause>`, and never repeats the message of git. A read error names its cause as `is a directory`, `no such file`, `not a directory`, or `permission denied`, and only a cause outside these four uses the description that the operating system gives. When `cargo metadata` fails, `check-feature-docs` still repeats the first error line of Cargo (Section 7.5).

The checkers are written in Python 3 and run with `python3 -I`, as `scripts/check-prose` and `scripts/check-commit-message` are, because the container provides Python 3 and those checkers set the pattern.

The checkers that read Rust share the lexer `scripts/rust_lexer.py`. It splits Rust source into comments, literals, and other tokens, groups `///` and `//!` comments into the Markdown text that rustdoc reads, and finds fenced code blocks in that text.

The lexer skips a shebang line at the start of a file, also when a byte order mark precedes it, and never reads it as a comment. A first line that starts with `#![` is an inner attribute and not a shebang line. `check-workflow-pins` uses only the helpers of the lexer module for finding and reading files.

TOOL-15. Each custom checker MUST have a self-test recipe named after it with the suffix `-self-test`, which `check` runs. The recipe MUST run the checker on at least one input that it accepts and one that it rejects for each rule of the checker, and MUST fail unless the checker gives exactly the expected result.

A checker that silently stops finding anything passes `check` while it enforces nothing. The inputs MUST be stored under a name that no checker or other tool reads as a real file, because a failing input stored under its real name would be reported by the checker itself.

The self-test recipes of the custom checkers, including `check-prose-self-test`, run `scripts/self-test-checker` with the name of the checker. Each case is a directory `scripts/fixtures/<checker>/<case>/`. Its input files carry the suffix `.fixture`, which the runner removes when it copies them into a new git repository in a temporary directory, so that the checkers, which select files by name, never read a fixture as a real file.

The runner replaces placeholders while it copies, so that no committed file holds an unfinished-work marker, a carriage return, an escape character, a bell character, a byte order mark, an invalid byte, or a missing final newline. It runs the checker in that repository with the arguments in the file `arguments` of the case, in which it replaces the same placeholders, or with none. A case that holds a file named `no-git-repository` gets no git repository, and the checker runs in a temporary directory outside any repository, which tests the error for a missing repository.

A case passes when all three of these hold:

- The standard output equals the file `expected-output`.
- The standard error equals the file `expected-errors`, or is empty when the case has no such file.
- The exit status equals the number in the file `expected-status`. Without that file, the expected status is 1 when `expected-output` is not empty and 0 when it is.

The usage text of `scripts/self-test-checker` lists the placeholders. `check-commit-message-self-test` keeps its own inline form.

### 7.2. Comment Checker

This table summarizes the comment checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-rust-comments`, `check-rust-comments` |
| Rule | Section 7.2 of the writing guide, and Hard Rule 5 of `AGENTS.md` |
| Reads | Every `.rs` file |
| Reports | Every comment other than a `///` comment, a `//!` comment, or a safety comment, and every comment of any kind that contains the word `TODO` or `FIXME` |

A safety comment is a run of `//` line comments on consecutive lines, with nothing else between them, whose first line starts with `// SAFETY:`, as the illustration in Section 7.6 of the writing guide shows. A line comment that starts with `SAFETY:` in another case or spacing is reported as a safety comment that does not start with `// SAFETY:`.

The checker reports every block comment, including `/** */` and `/*! */`, and every line comment that starts with four slashes. rustdoc and `missing_docs` treat `/** */` and `/*! */` as documentation, but the writing guide defines API documentation as `///` and `//!` comments only (Section 2.2 of the writing guide).

`////` and `/*** */` are not documentation comments, so they are other comments, which Section 7.2 of the writing guide forbids. Whether a safety comment sits directly above an `unsafe` block is checked by the safety comment lints of Section 4.5 of the coding standard, not by this checker.

TOOL-16. The comment checker MUST find comments by reading Rust tokens. The characters `//` and `/*` inside a string literal, a raw string literal, or a character literal are then not reported.

A checker that reports text inside literals produces findings that no fix can remove.

The lexer also treats byte string and C string literals as literals. Documentation written as a `#[doc = ...]` attribute is a literal, not a comment, so the checker does not read it.

This illustration shows an input and the findings that the checker prints for it, assuming the file is `src/lib.rs`:

```rust
/// Returns the scale factor.
pub fn scale() -> f32 {
    // Half of the unit size.
    let url = "https://shader-slang.org";
    0.5
}
```

```text
src/lib.rs:3: comment other than API documentation or a safety comment
```

The checker does not report line 4, because the slashes there are inside a string literal. Generated files are checked like any other file. The generated bindings get no exemption, because the binding generator emits no comments other than one module-level `//!` line (Section 5.10 of `docs/design/slang-compiler-sys.md`).

### 7.3. Heading Checker

This table summarizes the heading checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-doc-headings`, `check-doc-headings` |
| Rule | Section 7.3 of the writing guide |
| Reads | The `///` comments of every `.rs` file. Crate-level and module-level documentation, written with `//!`, is not read, because Section 7.4 of the writing guide gives it its own headings |
| Reports | A heading of any level that is not one of the level-one standard headings of Section 7.3 of the writing guide, including a standard heading with the wrong case or level, a standard heading out of the order that section gives, and a standard heading that appears twice in one item's documentation |

The checker reads both ATX headings, which start with `#`, and setext headings, which are underlined with `=` or `-`. The `///` comments that precede one item, with only attributes and other comments between them, are one item's documentation.

The checker ignores lines inside fenced code blocks, because a line that starts with `# ` in a Rust code block is a hidden line of a doctest, not a heading. It does not read documentation written as a `#[doc = ...]` attribute or a `/** */` block comment, which the comment checker of Section 7.2 rejects. Section 7.3 of the writing guide forbids every other heading in the documentation of an item, which is the rule that the checker enforces.

### 7.4. Doctest Attribute Checker

This table summarizes the doctest attribute checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-doctest-attributes`, `check-doctest-attributes` |
| Rule | Section 7.5 of the writing guide |
| Reads | The fenced code blocks in the `///` and `//!` comments of every `.rs` file, and in every file named `README.md`, which a crate includes as doctests (Section 7.10 of the writing guide). A path given on the command line that is not named `README.md` is read as Rust |
| Reports | A code block whose attributes include `ignore` or an attribute that starts with `ignore-`, such as `ignore-windows` |

The checker reports the attributes that start with `ignore-` as well, because Section 7.5 of the writing guide forbids `ignore` and these are variants of it that ignore the example on some targets.

The checker splits the info string of a code block into attributes the way rustdoc does. It splits at commas, spaces, and tabs, reads a double-quoted attribute without its quotes, and reads the text inside braces, such as `{.ignore}`, as a class or a key and not as an attribute. Whether a `no_run` example states its reason stays a judgement rule.

### 7.5. Feature Documentation Checker

This table summarizes the feature documentation checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-feature-docs`, `check-feature-docs` |
| Rule | Section 7.8 of the writing guide |
| Reads | For each workspace member with a library target, its features as `cargo metadata --no-deps --offline` reports them, and the `# Features` section of the `//!` crate-level documentation at the top of its library root, before the first item. `cargo metadata` also reports the feature that an optional dependency defines when no feature refers to that dependency with `dep:` |
| Reports | A missing `# Features` section in any library, including a library that has no features, a feature, other than `default`, that the section does not name in a code span, and a code span in the section that looks like a feature name, meaning lowercase letters, digits, `_`, `+`, and `-`, but names no feature of the package |

The checker requires the section in a library that has no features, because item 4 of Section 7.4 of the writing guide requires it in every crate. It does not read crate-level documentation written as a `#![doc = ...]` attribute, including `#![doc = include_str!(...)]`.

Unlike the other checkers, it takes the paths of Cargo manifests, and with no paths it checks the workspace of the git repository that contains the current directory. It prints paths relative to the current directory. When `cargo metadata` fails, it exits with status 2.

Whether the section says that each feature is on or off by default, and what it enables, stays a judgement rule, because the wording is free.

### 7.6. Prose in API Documentation

`scripts/check-prose` reads the text of the `///` and `//!` comments of `.rs` files and applies to it the rules that it applies to Markdown. This enforces Section 7.11 of the writing guide for the rules of its Section 8 that the script checks.

It reads each run of comments that documents one item as one Markdown document, and it reports each finding at its line in the Rust file. Code spans and fenced code blocks inside the comments stay exempt, as they are in Markdown.

It does not read documentation written as a `#[doc = ...]` attribute or a `/** */` block comment. A Rust file that is not valid UTF-8 cannot be read (Section 7.1). With no paths, `check-prose` checks every Markdown file and every Rust file, and its self-test is `check-prose-self-test`.

### 7.7. Moving a Rule to Enforced

TOOL-17. The change that lands a custom checker MUST do all of these in the same change, as Section 3.5 of the writing guide and Section 19 of the coding standard require for a rule that a tool can check:

- Rewrite the "Checked by" statement of the section of the writing guide that the checker enforces, and its row in Section 7.12 of the writing guide, to name the checker.
- Add the checker to the table of tools in Section 4.8 of the coding standard.
- Update the Validating Changes section of `AGENTS.md`.

A document that still calls an enforced rule a judgement rule sends a reviewer to check by hand what a tool checks, which Hard Rule 4 of `AGENTS.md` forbids. The change that extends `scripts/check-prose` also updates Section 8 of the writing guide, which describes the files that the script and its hook check.

### 7.8. Workflow Pin Checker

This table summarizes the workflow pin checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-workflow-pins`, `check-workflow-pins` |
| Rule | TOOL-20, the `pull_request_target` part of TOOL-22, and the top-level part of TOOL-19 (Section 8.2) |
| Reads | Every `.yml` and `.yaml` file under `.github/workflows/`, and every `action.yml` and `action.yaml` file under `.github/actions/`. A file named `action.yml` or `action.yaml` is read as a composite action, and any other file as a workflow |
| Reports | In every workflow and composite action, a `uses:` whose reference is not a full 40-character lowercase hexadecimal commit SHA. In every workflow, also the `pull_request_target` event under the top-level `on:` key, a workflow without a top-level `permissions:` key or with an empty one, and a top-level permission other than `read` or `none`, such as `write-all` or `contents: write` |

A local action, whose reference starts with `./`, needs no commit SHA. A `docker://` reference passes only when it is pinned by an `@sha256:` digest. The checker does not check the permissions of a job, so whether a job that requests more than read access needs it stays a judgement rule, and so does the part of TOOL-22 about secrets and publishing.

The checker reads each file line by line and does not use a YAML parser, and its usage text states the limits that follow from this:

- It reads keys only in block style, one per line, or as the value of `permissions:` in a one-line flow mapping such as `{ contents: read }`.
- It skips the text of a block scalar, such as the script of `run: |`.
- It does not read a `uses:` inside a flow mapping, or a scalar continued over several lines without a block indicator.
- It reads the text after a `#` that starts a line or follows a space as a comment, except inside quotes.

A workflow written in a form that the checker does not read therefore depends on review.

This illustration shows a workflow step and the finding that the checker prints for it, assuming the step is on line 12 of `.github/workflows/ci.yml`:

```yaml
      - uses: actions/checkout@v4
```

```text
.github/workflows/ci.yml:12: uses: 'actions/checkout@v4' is not pinned to a full 40-character commit SHA
```

### 7.9. Slang Call Guard Checker

This table summarizes the Slang call guard checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-slang-call-guard`, `check-slang-call-guard` |
| Rule | SLC-32 of `docs/design/slang-compiler.md`, which applies the guard of FFI-7 of the coding standard to `slang-compiler` |
| Reads | The names of the C functions and thunks in `crates/slang-compiler-sys/src/bindings.rs` of the git repository that contains the current directory, and every `.rs` file under `crates/slang-compiler/src`, at any depth, except every file named `fake.rs`, every inline module marked `#[cfg(test)]`, `use` items, comments, and literals |
| Reports | A call into Slang that is not inside the closure argument of `abort_on_exception`, a call inside a closure or `fn` item nested in that closure, a C function or thunk that is named without being called, a closure argument that makes no call or more than one call into Slang, a second argument of `abort_on_exception` that is not a closure literal, a `move` closure, and a call of `abort_on_exception` without a closure argument |

The checker keeps no list of names by hand. On every run, it reads the generated bindings. The C functions are the functions of their `unsafe extern "C-unwind"` block, and the thunks are the `unsafe fn` methods of each inherent `impl` block, whose type is the interface.

A change of the bindings therefore changes what the checker looks for without a change to the checker. When the bindings declare no C function or no thunk, the checker exits with status 2, so that a change in the layout of the bindings cannot turn the check off silently.

A C function counts as a call into Slang when its name appears as a word, with or without a path, except after `.` or `fn` and before `:`.

A thunk counts when it appears in the path `Q::m` or `<Q>::m`, where `m` is a thunk of the interface `Q`, also when `Q` ends a longer path, such as `slang_compiler_sys::IModule`. When `Q` is `Self`, or a macro metavariable other than `$crate`, such as `$interface`, the name of any thunk counts. A name that a `use` item imports with `as`, or that a type alias names, counts as the name it stands for.

A thunk takes its object as an argument and has no `self` receiver, so code reaches it only through a path through its interface. These forms are therefore not a call into Slang:

- A thunk name after any other qualifier, such as `T::release` or `component::link`.
- A thunk name after `<Q as Trait>::` or after `$crate::`.
- Every method call written with a dot.

This keeps names that Slang shares with other code, such as `release` and `link`, from causing findings.

When guards are nested, a call belongs to the innermost one, so an outer guard whose only call sits in an inner guard is reported as making no call into Slang. A nested closure is a `|` after one of `(`, `[`, `{`, `,`, `=`, `;`, `:`, `>`, `&`, `return`, `break`, and `move`, which includes the closure of a match arm. A call in the body of a macro counts once, not once for each expansion.

The checker follows TOOL-14, with the departure that TOOL-14 allows. It prints each finding as `path:line: message (SLC-32)`.

With no paths, it checks every `.rs` file under `crates/slang-compiler/src` that git tracks or does not ignore, and prints paths relative to the root of the repository. A file named `fake.rs` is skipped also when it is given as a path.

It exits with status 2 after checking every readable file when a file cannot be read, is not valid UTF-8, ends inside an unterminated literal or comment, or has a bracket without its partner. It reports the last case as `check-slang-call-guard: cannot read <file>: unbalanced '<bracket>' at line N` or `unclosed '<bracket>' at line N`. It also exits with status 2, before it checks any file, when it runs outside a git repository or when the bindings cannot be read or declare no C function or no thunk, because it then has no names to look for.

The cases of its self-test under `scripts/fixtures/check-slang-call-guard/` give the checker small bindings of their own. They cover each kind of finding, the forms that it must not report, such as the name collisions above, calls in comments, literals, test modules, and `fake.rs`, and each error of exit status 2.

These parts of SLC-32 are beyond what the checker reads, so they stay a judgement rule:

- Whether the closure owns a value with a destructor. The checker reports only the `move` form, and not a value that a closure without `move` captures by value through the way it uses it.
- A call that a macro defined outside the checked files makes.
- A call through a function pointer that the checker does not see named, such as an entry read from a vtable, as in `((*(*p).vtable).m)(p)`.
- An interface reached through a name that neither `use ... as` nor a type alias gives, such as a glob re-export under another name.
- A function named `abort_on_exception` that is not the guard. Any path that ends in `abort_on_exception(` counts as the guard.
- Whether the name of the callee in the first argument of `abort_on_exception` names the call that the closure makes. A tool could check this, and no tool does yet.

A `#[cfg(test)]` on an out-of-line module, such as `mod tests;`, and a condition such as `cfg(all(test, ...))` do not exclude a module, so such a module is checked. The risk there is a finding on test code, not a missed call.

This illustration shows the finding for a guard in `crates/slang-compiler/src/module.rs` replaced with the bare call, assuming the call is on line 489. The checker exits with status 1:

```rust
IModule::getName(self.raw.as_ptr())
```

```text
crates/slang-compiler/src/module.rs:489: call of IModule::getName is not inside a closure passed to abort_on_exception (SLC-32)
```

## 8. Continuous Integration

### 8.1. Workflow and Jobs

One workflow, `.github/workflows/ci.yml`, runs on every push to `main`, on every push to a branch that matches `feature/**`, and on every pull request. The project opens no pull requests, so a plain push of a feature branch is how a branch is verified before it merges (Section 17.7), and each such push starts one run. The `pull_request` trigger stays so that a pull request from a fork runs the same jobs, with a read-only token and no secrets (Section 8.2).

The `concurrency` group of a run on `main` is the run itself, and nothing cancels it, so every commit on `main` keeps its own result. For any other git reference, such as a feature branch or a pull request, the group is the workflow and the reference, and `cancel-in-progress` is true, so a new push cancels the earlier run for that reference.

Every job has a timeout of 60 minutes, and every `run` step uses Bash, also on Windows. The checkout step sets `persist-credentials: false`, so the token is not left in the git configuration of the runner for later steps.

TOOL-18. A workflow MUST run every check through a `just` recipe and MUST NOT run a check command directly. CI then runs the same commands as the development container, so a check that fails in one place has the same command to reproduce it in the other. Steps that only prepare a runner, such as installing a tool, are not checks.

This table lists the jobs, with the name that GitHub shows for each and the hosted runner label. `check-workflows` and `check-workflow-pins` check the workflow.

| Job | Name | Runner | Runs |
| --- | --- | --- | --- |
| `check` | Run every check on Linux | `ubuntu-latest`, Linux x86_64, with lavapipe as the software adapter | `just check` |
| `test` | Test on `windows-latest`, Test on `macos-latest` | A matrix of `windows-latest`, Windows x86_64, with the Windows Advanced Rasterization Platform (WARP) as the software adapter, and `macos-latest`, macOS arm64, with the hosted Metal device as the adapter, with `fail-fast: false` | `just check-clippy`, `just check-doctest`, and `just test-gpu` |
| `ci-only` | Run `check-features` on Linux, Run `check-msrv` on Linux | `ubuntu-latest`, Linux x86_64, as a matrix of the two recipes, with `fail-fast: false` | `just check-features` and `just check-msrv` |
| `semver` | Not yet named | Linux x86_64 | `just check-semver`. Planned for M7, with the first release, and not yet in the workflow (Section 10) |
| `coverage` | Measure test coverage on Linux | `ubuntu-latest`, Linux x86_64, with lavapipe as the software adapter | `just coverage`, then the upload of `target/lcov.info` as the artifact `coverage-lcov` |

The `test` job runs Clippy and the doctests on Windows and macOS as well as the tests, because a lint exception has to be fulfilled on every target that is checked (Section 4.4 of the coding standard), and code under `#[cfg]` differs between platforms. It compiles the checked-in bindings of `slang-compiler-sys` there and never regenerates them. The tests for Linux only are not compiled there.

Every job compiles `slang-compiler-sys` with its build script, which checks the installed SDK against the pin and links its Slang. The `test` jobs therefore prove more than compilation on Windows and macOS.

`just test-gpu` links the test binary of `slang-compiler-sys` against the Slang of the SDK and runs it. That includes the smoke test, which creates a global session and checks the build tag at run time. It also includes the native-symbol test, which fails to link when the library does not export a C function of the bindings.

The `macos-latest` runner is arm64, so no job runs the tests on x86_64 macOS (Section 6 of `docs/design/slang-compiler-sys.md`). Linux runs the GPU tier on lavapipe, and Windows and macOS run it on WARP and on the hosted Metal device, with SPIR-V as the only shader target of `slango`. Section 3.8 of `docs/design/slango.md` records the adapter of each runner.

The `check` and `coverage` jobs run the regeneration test of the checked-in bindings, through `test-gpu` and `coverage`. The test needs the headers of the installed SDK, libclang, and rustfmt, and both jobs install all three before they run the tests.

No job runs the `hw_gpu` tests, which run only on a developer's hardware GPU through `just test-hw-gpu`. No job builds the static route, because it compiles several hundred C++ files and takes several minutes, so the recipe `test-static` verifies it locally instead (SCS-36 of `docs/design/slang-compiler-sys.md`). Whether a scheduled job runs it is SQ-4 of that document.

Every job runs its steps in the same order. It checks out the repository, installs the Rust toolchains, installs its tools through the installer action, reads the pin, installs the Vulkan SDK with `just install-vulkan-sdk`, and checks the SDK with `just check-vulkan-sdk` (Section 8.3). Then it runs its own steps:

- The `check` job installs actionlint, libclang, lavapipe, and fontconfig and runs `just check`.
- The `test` job runs `just check-clippy`, `just check-doctest`, and `just test-gpu`.
- The `ci-only` job installs fontconfig and runs the recipe of its matrix entry.
- The `coverage` job installs libclang, lavapipe, and fontconfig, runs `just coverage`, and uploads the report.

The workflow defines the steps that check out the repository, read the pin, install and check the SDK, install libclang, install lavapipe, and install fontconfig once, with YAML anchors. It reuses them in the other jobs, so each of these steps is the same in every job that runs it.

The `coverage` job runs the tests of the `gpu` profile under cargo llvm-cov and keeps the report. The recipe prints no coverage summary in the log of the job (Section 17.5), so the uploaded report is the only result of the job.

The action `actions/upload-artifact`, pinned by commit SHA (TOOL-20), uploads `target/lcov.info` as the artifact `coverage-lcov`, which GitHub keeps for 14 days. The step sets `if-no-files-found: error`, so the job fails when the recipe wrote no report. The job sets no coverage threshold, so the report informs a reviewer and does not gate a merge.

CI installs the tools on each runner, because no container image is published for CI to run in. Each job installs only the tools that its recipes need:

- The Rust toolchains, through rustup, which the hosted runners provide. Each job installs the stable toolchain with the minimal profile and the components that it needs, which are rustfmt and clippy in the `check` job, clippy in the `test` job, none in the `ci-only` job, and rustfmt and llvm-tools in the `coverage` job.

  The tests of the binding generator start rustfmt, and cargo llvm-cov needs llvm-tools. The `check` job also installs the nightly toolchain with the components miri and rust-src, and lets rustup choose an older nightly when the latest one lacks a component.
- `just` in every job, cargo-nextest in the `check`, `test`, and `coverage` jobs, shellcheck, typos, taplo, cargo-machete, and cargo-deny in the `check` job, cargo-hack in the `ci-only` job, and cargo-llvm-cov in the `coverage` job, through the action `taiki-e/install-action`, pinned by commit SHA. The action installs prebuilt binaries and verifies the SHA-256 checksum of each one against the manifest of versions and checksums that it carries. The workflow sets `fallback: none`, so the action fails instead of installing a tool by a route that its manifest does not cover.
- actionlint, in the `check` job, through an inline step. It reads the latest release from GitHub, downloads the release archive for Linux x86_64 and the checksums file of the same release, verifies the archive with `sha256sum --check --strict`, and fails on a mismatch before it extracts the archive.
- libclang, in the `check` and `coverage` jobs, which the tests of the binding generator load, including the regeneration test (Sections 7.5 and 7.6 of `docs/design/slang-compiler-sys.md`). The step runs `sudo apt-get update` and `sudo apt-get install --yes --no-install-recommends libclang-dev`. It meets TOOL-21 because apt verifies the signature of the distribution's package index and the SHA-256 checksum of each package against that index, and refuses a package that fails either check.

  The package is the libclang of the runner's distribution, and its version can differ from the one in the development container (Section 7.6 of `docs/design/slang-compiler-sys.md`). The `test` jobs do not install libclang, because the tests that load it compile only for Linux.
- lavapipe, in the `check` and `coverage` jobs, as Mesa's package `mesa-vulkan-drivers`, which gives the tests in modules named `gpu` a software Vulkan adapter. The step runs `sudo apt-get update` and `sudo apt-get install --yes --no-install-recommends mesa-vulkan-drivers`, so it meets TOOL-21 in the same way as the libclang step. The `test` jobs install no adapter.
- The development files of fontconfig, in the `check`, `ci-only`, and `coverage` jobs, as the package `libfontconfig-dev`. A Linux build of Slint links fontconfig through the build script of `yeslogic-fontconfig-sys`, which finds it through pkg-config (Section 3.2 of `docs/design/slang-slint.md`).

  The step runs `sudo apt-get update` and `sudo apt-get install --yes --no-install-recommends libfontconfig-dev`, so it meets TOOL-21 in the same way as the libclang step. The `test` jobs install nothing for it, because the builds on Windows and macOS need no fontconfig development files.
- The pinned Vulkan SDK, in every job, through `just install-vulkan-sdk` (Section 8.3).

Each route except the Vulkan SDK installs the latest version that it provides, which matches the policy of the container image, in which no tool version is pinned. The Vulkan SDK is the version of the pin (Section 8.3). A tool in CI can therefore have a different version from the same tool in the development container, and a check can pass in one and fail in the other until both have the same version.

The workflow caches nothing, so no file is restored from a cache.

### 8.2. Workflow Security

TOOL-19. A workflow MUST grant its token read access to the repository contents only, at the level of the workflow, and a job that needs more MUST request the extra permission on that job alone. A compromised step can use every permission that the token holds.

`check-workflow-pins` reports a workflow without top-level permissions and a top-level permission other than `read` or `none` (Section 7.8). Whether a job that requests more needs it is a judgement rule.

TOOL-20. A workflow MUST reference every action, including the actions that GitHub publishes, by a full commit SHA and never by a tag or a branch. The owner of an action can move a tag or a branch to different code, which then runs in every later job without any change to this repository.

actionlint does not check this, and `check-workflow-pins` enforces it (Section 7.8).

TOOL-21. A workflow step that downloads a file MUST verify it against a SHA-256 checksum that the upstream publishes and MUST stop the job before the file is used when the checksum differs, as Hard Rule 7 of `AGENTS.md` requires.

A step that installs tools through an action MUST use an action that verifies each download in this way, pinned by commit SHA (TOOL-20), which also pins the manifest of versions and checksums that the action carries. The step MUST configure the action to fail instead of installing by a route that it does not verify.

A file restored from a cache MUST be verified in the same way as a fresh download.

These requirements hold because a tampered file that is not verified runs with the permissions of the job, and a cache can hold a file that an earlier, compromised run stored.

When the upstream publishes no checksum, the step SHOULD record in the workflow the checksum of the file that the agent verified when it wrote the step. A later change in the served file is then detected, even though the first file is trusted on first use.

A checksum that comes from the same host as the file, such as the checksums file of an actionlint release, proves that the download is intact but not that it comes from the publisher, because an attacker who controls the host can replace both. A checksum in the pinned manifest of the installer action comes from the commit of the action that TOOL-20 pins instead.

Section 16 states the residual risk. Whether each step meets TOOL-21 is a judgement rule, because no tool can tell from a workflow which step downloads a file.

The action `actions/upload-artifact`, which the `coverage` job uses (Section 8.1), is pinned by commit SHA like every other action, and `check-workflow-pins` checks it. The step that installs the Vulkan SDK meets TOOL-21 with checksums that the workflow records (Section 8.3). The installers of the SDK for macOS and Windows also fetch metadata from LunarG during the installation, which that checksum does not cover, and Section 16 states that residual risk.

TOOL-22. A workflow MUST NOT use the `pull_request_target` event, read a secret, or publish a crate. The workflows need no secret to check code, and a workflow that holds a secret while it runs the code of a pull request exposes that secret to the author of the pull request.

`check-workflow-pins` reports the `pull_request_target` event (Section 7.8). The rest of TOOL-22 is a judgement rule.

The workflow keeps the `pull_request` trigger for pull requests from forks. GitHub gives a run for such a pull request a read-only token and no secrets, so the code of the pull request can neither write to the repository nor read a secret. A push to `main` or to a feature branch needs write access to the repository, so the push triggers add no way for an outside author to run code in CI.

### 8.3. The Vulkan SDK on CI Runners

Every job of the workflow installs the pinned Vulkan software development kit (SDK) and checks it before any step that compiles the workspace. The build script of `slang-compiler-sys` links Slang, and every job compiles that crate. SCS-35 of `docs/design/slang-compiler-sys.md` states what the installation achieves.

TOOL-23. The step that installs the Vulkan SDK on a runner MUST do all of these:

- Read the SDK version from the one place that records the pin (ARCH-9 of the architecture design) and not restate the version.
- Verify the download as TOOL-21 requires.
- Set `VULKAN_SDK` for the later steps.
- Make the Slang shared library findable at run time on the runner in the way that Section 4.4 of the architecture design describes for the platform.

A second copy of the pin diverges from the first. An unverified installer runs with the job's permissions. A test binary that cannot find the shared library fails before any test runs.

The step "Read the pin of slang-compiler-sys" reads the version. It runs `just pin`, captures the output, and appends it to the file that `GITHUB_ENV` names, so the later steps of the job have `VULKAN_SDK_VERSION`, `SLANG_BUILD_TAG`, and `SLANG_COMMIT` in their environment.

It appends only after the recipe succeeds, so a failing recipe appends nothing and fails the step. The workflow states neither the version nor the build tag, as TOOL-23 and SCS-23 of `docs/design/slang-compiler-sys.md` require.

The step "Install the Vulkan SDK" then runs this command, in which `SDK_SHA256` is the checksum that the workflow records for the platform of the runner:

```sh
just install-vulkan-sdk "${VULKAN_SDK_VERSION}" "${SDK_SHA256}"
```

The recipe runs `scripts/install-vulkan-sdk`, which does these things in this order:

1. It exits with status 2 on a number of arguments other than two, and when `RUNNER_TEMP`, `GITHUB_ENV`, or `GITHUB_PATH` is unset or empty, which GitHub Actions sets on every runner, so it does not run outside CI. It also exits with status 2 on a version that is not four numbers separated by full stops, on a checksum that is not 64 lowercase hexadecimal characters, and on a platform other than Linux x86_64, macOS, and Windows x86_64.
2. It downloads `https://sdk.lunarg.com/sdk/download/<version>/<platform>/<file>`, in which `<platform>` is `linux`, `mac`, or `windows`, with curl. It restricts curl to HTTPS with Transport Layer Security (TLS) 1.2 or later, makes it fail on an HTTP error, and lets it retry three times. When the download fails, the script stops with the exit status of curl.
3. It computes the SHA-256 checksum of the file from standard input, with `sha256sum`, or with `shasum -a 256` where `sha256sum` is absent. When the checksum differs from the expected one, it prints both values and exits with status 1, before it unpacks or runs anything. It hashes standard input because `sha256sum` on the Windows runner prefixes its output with a backslash when the file name contains a backslash.
4. It installs the SDK under `$RUNNER_TEMP/VulkanSDK/<version>`, which this section calls `<root>`, as the table below describes for each platform. When unpacking or running the installer fails, the script stops with the exit status of the failing command. On macOS, it exits with status 1 when the archive does not hold exactly one installer application.
5. It appends `VULKAN_SDK` to the file that `GITHUB_ENV` names on every platform, and also `LD_LIBRARY_PATH` on Linux and `DYLD_LIBRARY_PATH` on macOS, each with the SDK's `lib` directory first. Windows gets no library path variable.

   It appends the SDK's `bin` directory to the file that `GITHUB_PATH` names on Linux and macOS, and its `Bin` directory on Windows. Every later step of the job has these values.
6. It removes the directory that holds the downloaded file on a best-effort basis. When the removal fails, such as on Windows while the installer still holds the file locked, it prints one warning to standard error and still exits with status 0, because the runner discards its temporary directory.

This table gives the downloaded file, the installation, and the values that the script exports on each platform:

| Platform | Downloaded file | Installation | `VULKAN_SDK` | Library path |
| --- | --- | --- | --- | --- |
| Linux x86_64 | `vulkansdk-linux-x86_64-<version>.tar.xz` | Unpacks the archive into the root with `tar -xJf` and `--strip-components=1` | `<root>/x86_64` | `LD_LIBRARY_PATH` starts with `$VULKAN_SDK/lib`, and `PATH` gets `$VULKAN_SDK/bin` |
| macOS | `vulkansdk-macos-<version>.zip` | Unzips the archive and runs the one installer application in it with `--root <root> --accept-licenses --default-answer --confirm-command install`, without `sudo` | `<root>/macOS` | `DYLD_LIBRARY_PATH` starts with `$VULKAN_SDK/lib`, and `PATH` gets `$VULKAN_SDK/bin` |
| Windows x86_64 | `vulkansdk-windows-X64-<version>.exe` | Runs the installer with the same options and `copy_only=1`, so that it writes no registry entries, layers, shortcuts, or system `PATH` | The root, as a Windows path | `PATH` gets `$VULKAN_SDK\Bin`, which holds both the programs and `slang-compiler.dll` |

The variable that each platform's dynamic loader reads gets the SDK's library directory, and on Windows, `PATH` gets the directory of the DLL, which meets the last part of TOOL-23. The job does not run the setup script of the SDK. On Linux and macOS, the install step sets the loader variables that the setup script would set, as Section 4.4 of the architecture design describes.

System Integrity Protection is disabled on the hosted macOS runner, so `DYLD_LIBRARY_PATH` reaches every process that a step starts. This does not show that the variable reaches a test binary on a developer's Mac, which SA-7 of `docs/design/slang-compiler-sys.md` tracks.

The installers for macOS and Windows are installers of the Qt Installer Framework. On the hosted runners, they run without a graphical session, accept the licence through `--accept-licenses`, select only the core component, and return when the installation is done.

The macOS installer needs no `sudo` for a directory that the user owns. Both installers fetch metadata from LunarG's remote repository during the installation, and no supported option to prevent this is known, which Section 16 records as a residual risk.

The step "Check the Slang of the Vulkan SDK" then runs `just check-vulkan-sdk`. The recipe runs `scripts/check-vulkan-sdk`, which reads the build tag of the pin with `just pin` and checks these things:

- `$VULKAN_SDK/include/slang` holds `slang.h` and `slang-tag-version.h`, and `slang-tag-version.h` has the exact line `#define SLANG_TAG_VERSION "<tag>"`.
- The Slang library of the platform exists, which is `lib/libslang-compiler.so` on Linux, `lib/libslang-compiler.dylib` on macOS, and `lib/slang-compiler.lib` and `bin/slang-compiler.dll` on Windows, whose file system resolves these names to the directories `Lib` and `Bin`.
- `$VULKAN_SDK/bin/slangc -version` prints output that contains the tag.

The script also lists the files that it finds. It removes carriage returns from `slang-tag-version.h` and from the output of `slangc` before it compares them, so it accepts CRLF line endings. It needs no CI runner, so the same command checks the SDK of the development container.

The script exits with status 1 when a check fails, including when `VULKAN_SDK` names a path that is not a directory. For that case, it prints `check-vulkan-sdk: VULKAN_SDK names <path>, which is not a directory` and checks nothing else. It exits with status 2 when it gets an argument, when `VULKAN_SDK` is unset or empty, or when `just pin` fails or prints no `SLANG_BUILD_TAG`.

LunarG publishes a SHA-256 checksum of each SDK download, as 64 hexadecimal characters at a documented address of the form `https://sdk.lunarg.com/sdk/sha/<version>/<platform>/<file>.txt`, which is served from the same host as the download. A checksum fetched from that host at run time detects a corrupted download, but not a download that an attacker at the host replaced together with its checksum. The workflow therefore records the expected values and does not fetch them at run time, which the pinned version of the SDK allows, unlike the latest-version installs of Section 8.1.

The workflow records the three checksums in its top-level `env` as `VULKAN_SDK_SHA256_LINUX`, `VULKAN_SDK_SHA256_MACOS`, and `VULKAN_SDK_SHA256_WINDOWS`, with a comment that ties them to the version that `crates/slang-compiler-sys/pin.rs` pins. The install step selects the one for `runner.os`.

The checksums are not part of the pin, so they stay in the workflow, which is their only user, and they change in the same change as the pin (Section 8.1 of `docs/design/slang-compiler-sys.md`). LunarG also publishes an installer for Windows on ARM64, which the workflow does not use, because the `windows-latest` runner is x86_64.

The workflow does not cache the SDK. A cache of the installed tree would save at most the installation time, and restoring a large tree takes time of its own.

TOOL-21 also requires a restored tree to be verified like a fresh download, which would need a recorded manifest of the hash of every file. A cache of the downloaded file alone is easy to verify against the recorded checksum, but it saves only the download.

Sections 3.6, 3.7, and 12 of `docs/design/slang-compiler-sys.md` record what each SDK ships.

## 9. Dependency Policy

The file `deny.toml` configures cargo-deny, which Section 4.8 of the coding standard lists with what it owns. Section 4.8 also requires the licence allow-list to include `GPL-3.0-or-later` itself.

TOOL-24. The licence allow-list of `deny.toml` MUST start with `GPL-3.0-or-later` alone. An agent MUST add any other licence only in the change that adds the first dependency under it, after confirming that the licence is compatible with GPL-3.0-or-later. A licence that no dependency uses widens what a later dependency can bring in without any check failing.

TOOL-25. Every path dependency between workspace members MUST carry a version, and `deny.toml` MUST set `bans.allow-wildcard-paths = false`, because Cargo refuses to package or publish a crate whose path dependency has no version, so without the check a missing version surfaces only at the release. The value `true` is no alternative (Section 4.8 of the coding standard).

TOOL-26. `deny.toml` MUST make the check fail on every security advisory that applies to a dependency in `Cargo.lock`, on every duplicate version of a crate, and on every crate from a source other than crates.io. An agent MAY record an exception only for one advisory or one duplicate, with a reason in `deny.toml`.

Section 4.5 of the coding standard leaves the policy on duplicate versions to cargo-deny, and a policy that only warns reports nothing that the check enforces. A duplicate that no change to the workspace can remove is the expected case for an exception. An exception for a duplicate version has the form `skip = [{ crate = "name@version", reason = "..." }]` in the `[bans]` table.

TOOL-31. `deny.toml` MUST set `unused-allowed-license = "deny"` in its `[licenses]` table and `unused-ignored-advisory = "deny"` in its `[advisories]` table, because without them cargo-deny only warns about an unused entry, and the lists that TOOL-24 and TOOL-26 keep minimal then grow without any check failing.

`deny.toml` has these settings. Its `ignore`, `allow`, and `skip` lists hold one entry per line, and the tables below give their entries:

```toml
[graph]
all-features = true

[advisories]
ignore = [] # The advisories of the advisory table, each with its reason.
unused-ignored-advisory = "deny"

[licenses]
allow = ["GPL-3.0-or-later"] # Followed by the licences of the licence table.
confidence-threshold = 0.8
unused-allowed-license = "deny"

[bans]
multiple-versions = "deny"
wildcards = "deny"
allow-wildcard-paths = false
skip = [] # The entries of the skip table, each with its reason.

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

cargo-deny checks the dependency graph of every target, including WebAssembly, Apple platforms, Android, and Redox, so the lists cover crates that a build on Linux does not compile.

These dependencies shape the lists:

- `clang`, for the binding generator.
- wgpu 30 and `pollster`, for `slango` and the examples package.
- winit 0.30.13, for the examples package only, with its default features off and the features `rwh_06`, `x11`, `wayland`, and `wayland-dlopen` on.
- `slint` with the version requirement `~1.18` and its default features off. `slang-slint` enables the features that Section 4.1 of `docs/design/slang-slint.md` lists and no windowing backend or renderer of Slint. The examples package enables Slint's windowing backend on winit and its FemtoVG renderer on wgpu (Section 10 of that design).

Slint's winit backend turns on winit's feature `wayland-csd-adwaita` for every example, including `windowed-renderer`, as Section 3.6 of `docs/design/slang-slint.md` records. `sctk-adwaita` and the crates under BSD licences below are therefore in the graph.

The licence allow-list holds `GPL-3.0-or-later` and these licences, each for the crates that need it:

| Licence | Crates that need it | How they enter |
| --- | --- | --- |
| `Apache-2.0` | `clang` and `clang-sys`, and `glob`, `libc`, `cfg-if`, and `windows-link`, whose licence `MIT OR Apache-2.0` it satisfies | The binding generator |
| `BSD-2-Clause` | `arrayref` 0.3.9 | `tiny-skia` 0.11 and 0.12, and `tiny-skia-path` 0.11 |
| `BSD-3-Clause` | `tiny-skia` and `tiny-skia-path` 0.11.4 and 0.12.0, and `bindgen` 0.72.1 | `tiny-skia` 0.11 through `sctk-adwaita` 0.10, `tiny-skia` 0.12 through `resvg` of `i-slint-common`, and `bindgen` as a build dependency of `skia-bindings` of Slint's Skia renderer, which `i-slint-backend-winit` uses only on Apple targets other than macOS |
| `BSL-1.0` | `clipboard-win` 5.4.3 and `error-code` 3.4.0 | `arboard` of `i-slint-backend-winit`, on Windows |
| `GPL-3.0-only` | The seven crates of Slint 1.18.1 in the graph, `slint`, `slint-macros`, `i-slint-backend-selector`, `i-slint-common`, `i-slint-compiler`, `i-slint-core`, and `i-slint-core-macros`, each under `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` | `slint`, on all four supported targets |
| `ISC` | `libloading` | The binding generator |
| `MIT` | The crates under MIT alone, which are `block2`, `cfg_aliases`, `crunchy`, `dlib`, `libm`, `objc2`, `objc2-encode`, `objc2-foundation`, `ordered-float`, `redox_syscall`, `slab`, `tokio`, which only the WebAssembly target reaches, `wayland-sys`, and `xml-rs`. It also satisfies `termcolor` and `winapi-util`, under `Unlicense OR MIT` | wgpu and winit |
| `Unicode-3.0` | `unicode-ident`, under `(MIT OR Apache-2.0) AND Unicode-3.0` | wgpu |
| `Zlib` | `foldhash` 0.1.5 and 0.2.0, and `slotmap` | wgpu |

Each licence is compatible with GPL-3.0-or-later, as TOOL-24 requires. Section 14 of `docs/design/slang-compiler-sys.md` records the licence of each crate of the binding generator, which is the case for `Apache-2.0` and `ISC`. The licence list of the Free Software Foundation (FSF) gives the evidence for the others [FSF-LICENSES]:

- `MIT`: Its entry for the Expat licence, which is the text that SPDX names `MIT`, calls it a lax, permissive licence compatible with the GNU GPL.
- `Zlib`: Its entry for the licence of zlib calls it a free software licence compatible with the GPL.
- `Unicode-3.0`: Its entry for the Unicode, Inc. License Agreement for Data Files and Software calls it a lax permissive licence compatible with all versions of the GPL, but it does not name version 3 of that licence. The text that `unicode-ident` ships, headed "UNICODE LICENSE V3", grants permission to deal in the data files or software without restriction, on the one condition that the notice appears with all copies, so it is a permissive grant of the same kind.
- `BSD-2-Clause`: Of the FreeBSD license, which it says "is also sometimes called the '2-clause BSD license'", it says: "It is a lax, permissive non-copyleft free software license, compatible with the GNU GPL."
- `BSD-3-Clause`: Of the Modified BSD license, it says: "It is a lax, permissive non-copyleft free software license, compatible with the GNU GPL. This license is sometimes referred to as the 3-clause BSD license."
- `BSL-1.0`: Of the Boost Software License, it says: "This is a lax, permissive non-copyleft free software license, compatible with the GNU GPL."

The owner approved `GPL-3.0-only`, and `slang-slint` takes the crates of Slint under it, with the consequence that Section 4.2 of `docs/design/slang-slint.md` states. The evidence of compatibility is this:

- The licence text that `slint` 1.18.1 and `i-slint-core` 1.18.1 ship, in `LICENSES/GPL-3.0-only.txt`, is version 3 of the GNU General Public License, dated 29 June 2007. It differs from the repository's `LICENSE` only in formatting and quotation marks.
- The FSF licence list lists version 3 of the GNU GPL as a free software licence and a copyleft licence, and recommends it for most software [FSF-LICENSES].
- The compatibility matrix of the GPL FAQ permits copying code under GPLv3 into code that is licensed under GPLv3 or later [GPL-FAQ]. The combination is then distributed under version 3 only, which is the consequence that Section 4.2 of that design states.

The owner also approved `BSD-2-Clause`, `BSD-3-Clause`, and `BSL-1.0`. The project's licence stays GPL-3.0-or-later.

The dependencies bring duplicate versions that no change to the workspace can remove, because the lock file already resolves each crate to its newest compatible version. Each older version has its own `skip` entry, with its reason in `deny.toml`, under TOOL-26.

No exception covers a subtree such as that of winit. Such an exception would stop the check for about 120 crates under winit, most of which are not duplicated, while the single entries admit exactly the duplicated versions. `deny.toml` holds these 31 entries:

| Skip | Needed by | Other versions in the tree |
| --- | --- | --- |
| `bitflags@1.3.2` | `core-graphics` 0.23 in the macOS backend and `redox_syscall` 0.4 in the Redox backend of winit | 2, of wgpu 30 and winit |
| `block2@0.5.1` | `objc2-foundation` 0.2 and the macOS backend of winit | 0.6, of `wgpu-hal` 30 |
| `foldhash@0.1.5` | `hashbrown` 0.15.5 of `petgraph` 0.8, which naga 30 requires | 0.2.0, of `hashbrown` 0.16.1 |
| `getrandom@0.3.4` | `ahash` 0.8 of the Wayland backend of winit | 0.4, of `jobserver` |
| `glow@0.17.0` | `wgpu-hal` 30 | 0.18, of `femtovg` 0.27 and `i-slint-renderer-femtovg` 1.18 |
| `hashbrown@0.14.5` | `rowan` 0.17 of `i-slint-compiler` 1.18 of `slint-macros` | 0.15.5 and 0.16.1, which have their own entries, and 0.17.1, of `fontique` and `indexmap` 2 |
| `hashbrown@0.15.5` | `petgraph` 0.8 of naga 30 | 0.16.1 and 0.17.1 |
| `hashbrown@0.16.1` | `gpu-allocator` 0.28 of `wgpu-hal` 30 | 0.17.1, of `indexmap` 2 of naga |
| `itertools@0.13.0` | `bindgen` 0.72, a build dependency of `skia-bindings` on Apple targets other than macOS | 0.15, of `femtovg` 0.27 and `i-slint-compiler` 1.18 |
| `jni-sys@0.3.1` | `ndk-sys` 0.6 of `wgpu-hal` and `ndk` 0.9 of winit, as a shim over 0.4 | 0.4, of `jni` 0.22 of `android-activity` in the Android backend of winit |
| `linux-raw-sys@0.4.15` | `rustix` 0.38 of winit and its Wayland backend | 0.12, of `rustix` 1 |
| `miniz_oxide@0.8.9` | `png` 0.18 of `image` 0.25 of `i-slint-core`, directly | 0.9.1, of the compression crate of the same `png` |
| `nom@7.1.3` | `cexpr` 0.6 of `bindgen` 0.72 | 8, of `tree_magic_mini` 3.2 of `wl-clipboard-rs` of `arboard` |
| `objc2@0.5.3` | The macOS backend of winit, with `objc2-app-kit` and `objc2-foundation` 0.2 | 0.6, of `wgpu-hal` 30 |
| `objc2-app-kit@0.2.2` | The macOS backend of winit 0.30 of the examples package | 0.3.2, of `i-slint-core` on macOS |
| `objc2-foundation@0.2.2` | The macOS backend of winit | 0.3, of `objc2-metal` 0.3 of `wgpu-hal` 30 |
| `objc2-ui-kit@0.2.2` | The iOS backend of winit 0.30 | 0.3, of `i-slint-backend-winit` 1.18 |
| `r-efi@5.3.0` | `getrandom` 0.3 of `ahash` of winit | 6, of `getrandom` 0.4 |
| `redox_syscall@0.4.1` | The Redox backend of winit | 0.5, of `parking_lot_core` of wgpu, and 0.9, of `libredox` of `orbclient` |
| `redox_syscall@0.5.18` | `parking_lot_core` 0.9 of wgpu 30 | 0.9, of `libredox` 0.1 of `orbclient` in the Redox backend of winit |
| `rustc-hash@1.1.0` | naga 30, `naga-types` 30, and `wgpu-core` 30, and `rowan` 0.17 of `i-slint-compiler` | 2, of `bindgen` 0.72 |
| `rustix@0.38.44` | winit, `calloop` 0.13, and `smithay-client-toolkit` 0.19 | 1, of `wayland-backend`, `x11rb`, and `polling` |
| `shlex@1.3.0` | `bindgen` 0.72 | 2, of `cc` |
| `smol_str@0.2.2` | winit 0.30 of the examples package | 0.3.7, of `i-slint-compiler` |
| `syn@2.0.119` | `jni-sys-macros`, `windows-implement`, `windows-interface`, and `zerocopy-derive`, and `slango-codegen`, `slango-macros`, and `prettyplease` 0.2 (SLM-3 of `docs/design/slango-macros.md`) | 3.0.10, of `bytemuck_derive`, `thiserror-impl`, and `wasm-bindgen-macro-support` |
| `thiserror@1.0.69` | `calloop` 0.13, `ndk` 0.9, and `smithay-client-toolkit` 0.19 of winit | 2, of wgpu 30 |
| `thiserror-impl@1.0.69` | `thiserror` 1 | 2, of `thiserror` 2 |
| `tiny-skia@0.11.4` | `sctk-adwaita` 0.10 | 0.12, of `resvg` 0.48 of `i-slint-common` |
| `tiny-skia-path@0.11.4` | `tiny-skia` 0.11 | 0.12, of `tiny-skia` 0.12 and `usvg` 0.48 |
| `windows-sys@0.52.0` | The Windows backend of winit | 0.59.0, and 0.61, of `rustix` 1, `polling`, and `errno` |
| `windows-sys@0.59.0` | `rustix` 0.38 of winit | 0.52.0, and 0.61, of `rustix` 1, `polling`, and `errno` |

The `ignore` list of the `[advisories]` table holds five advisories by the owner's decision, each with its reason, as TOOL-26 permits. Each is for a crate that is unmaintained, with no upgrade available and no vulnerability reported. TOOL-31 fails the check once an entry no longer applies, so each entry is removed when its condition holds:

| Advisory | Crate | How it enters | Removed when |
| --- | --- | --- | --- |
| RUSTSEC-2026-0441 | `pin-utils` 0.1.1, whose repository is archived | `spin_on` 0.1.1, which `slint-macros` 1.18.1 requires unconditionally, so no feature set of Slint avoids it. It runs only at build time, inside a procedural macro | A release of Slint no longer depends on `pin-utils` |
| RUSTSEC-2026-0370 | `zvariant` 5.15 | Slint's winit backend, in the unpublished examples package only (Section 3.6 of `docs/design/slang-slint.md`) | `Cargo.lock` no longer holds `zvariant` 5.15, such as after a release of Slint's winit backend that requires another `zbus` |
| RUSTSEC-2026-0371 | `zbus_names` 4.3 | As for RUSTSEC-2026-0370 | `Cargo.lock` no longer holds `zbus_names` 4.3, as for RUSTSEC-2026-0370 |
| RUSTSEC-2026-0372 | `zvariant_derive` 5.15 | As for RUSTSEC-2026-0370 | `Cargo.lock` no longer holds `zvariant_derive` 5.15, as for RUSTSEC-2026-0370 |
| RUSTSEC-2026-0192 | `ttf-parser` 0.25 | Slint's winit backend, through `sctk-adwaita` 0.10, in the unpublished examples package only | `Cargo.lock` no longer holds `ttf-parser` 0.25, such as after a release of winit or Slint without `sctk-adwaita` 0.10 |

A Linux build of Slint links fontconfig, so the Linux jobs install its development files (Section 8.1). `check-deny` fetches the advisory database on every run, so it needs network access (Section 17.6).

## 10. Publishing Readiness

The architecture design owns the publishing policy in its Section 13 and the go-ahead rule ARCH-24. This section defines how the repository is kept ready to publish.

TOOL-4. The `[workspace.package]` table MUST declare the `license` field as the SPDX licence identifier `GPL-3.0-or-later`, the `rust-version` field as `1.99`, the `repository` field as the address of the repository on GitHub, and the `readme` field as the root `README.md`. Every member MUST inherit these fields from the workspace instead of setting its own, because a member with its own value can disagree with the licence that cargo-deny checks and with the minimum supported Rust version (MSRV) that Clippy reads.

TOOL-5. An agent MUST NOT change `rust-version` except in a change whose stated purpose is to raise it, and that change updates Section 17.3 and `README.md`. The MSRV is a promise to users (DEP-4 of the coding standard), and a raise hidden in another change breaks their builds without notice.

TOOL-27. Every published crate MUST carry the metadata fields that the row of `clippy::cargo_common_metadata` in Section 4.5 of the coding standard lists, inheriting those that TOOL-4 places in `[workspace.package]` and setting `description`, `keywords`, and `categories` itself. Every member that is never published MUST set `publish = false`, because Clippy reports a missing field on every member that does not set `publish = false`, so the check fails until the metadata is complete.

TOOL-3. An agent MUST create the examples as one workspace member, the package `slang-rust-examples` at `examples/`, with `publish = false`, in which each example is a binary target. One package needs one manifest with one `publish = false`, one integration-test binary under TEST-10 of the coding standard, and one place for shared helpers.

Building one example builds the dependencies of every example, which the examples accept, because ARCH-14 of the architecture design limits the dependencies of `slang-compiler` and no rule limits those of the examples.

Cargo supports this layout. A `readme` that a member inherits from the workspace as `readme = "README.md"` resolves to the root file, and Cargo copies that file into the package.

Cargo refuses to package a crate whose path dependency has no version (TOOL-25). Packaging one crate that depends on a member that is not published yet fails, while `cargo package --workspace` succeeds.

Cargo does not include the root `LICENSE` in a packaged crate when the manifest sets only the `license` field, so how each published crate carries the licence text is open (Section 15, TQ-6).

The documentation build on docs.rs has to work without Slang, as ARCH-8 of the architecture design requires. The environment of this section, which TOOL-7 permits a recipe to add, is `DOCS_RS` set to `1` and `VULKAN_SDK` removed. The recipe `check-doc-without-slang` runs the documentation command of Section 4.8 of the coding standard in that environment.

`check-rust` runs it directly after `check-doc`. This is its command:

```sh
env -u VULKAN_SDK DOCS_RS=1 CARGO_BUILD_WARNINGS=deny cargo doc --workspace --no-deps --all-features
```

With `DOCS_RS` present, the build script of `slang-compiler-sys` reads nothing else and links nothing (Section 9.4 of `docs/design/slang-compiler-sys.md`), so the recipe passes on a machine without the Vulkan SDK. It reproduces only the absence of Slang, not the rest of the docs.rs environment, which M7 verifies on docs.rs itself. Each crate's design document defines any docs.rs settings that the crate's manifest needs.

The command documents every member of the workspace, so a new member needs no change to the recipe. `slang-compiler` has no build script, so its documentation builds without Slang because the build script of `slang-compiler-sys` links nothing when `DOCS_RS` is set (Section 13.7 of `docs/design/slang-compiler.md`). The examples package holds `typed-kernel`, which calls the macro `shader!`, so the recipe also builds the documentation of a caller of the macro in its stub form (Section 6.4 of `docs/design/slango-macros.md`).

The recipe `check-semver` compares each published crate with its last version on crates.io. It arrives in M7 with the first release, together with the CI job `semver`, because only then does each published crate have a published version to compare with.

cargo-semver-checks 0.51.0 fails on a crate that has no published version, with the message "not found in registry" and exit status 101, and has no option to skip such a crate. Its option `--baseline-rev` compares with a git revision instead, but it fails on a second run when the target directory is inside the workspace. Before the first release, a comparison with `main` would report every change to the API, which tells a reviewer nothing.

How a release is carried out is open (Section 15, TQ-4).

## 11. Container Image Requirements

The repository slang-rs-env defines the container image, so the changes of this section are made there and not in this repository.

TOOL-28. An agent MUST make each change to the container image in the repository slang-rs-env. The first change in this repository after an image rebuild that adds or removes a tool MUST update the tool list in the Technology Decisions section of `AGENTS.md`. It MUST also update the status of the tool in Section 4.8 of the coding standard.

A document that calls an installed tool missing leaves its rules as judgement rules that a reviewer checks by hand. A document that calls a missing tool installed leads an agent to build on it.

TOOL-29. The container image MUST provide these, because the recipes of Section 4.2 run them, and the run-time path lets the tests start:

- Every tool that Section 4.8 of the coding standard lists.
- A nightly toolchain with the components miri and rust-src, used only for checkers, as DEP-4 of the coding standard requires, with the stable toolchain as the default.
- The component llvm-tools of the stable toolchain, which cargo llvm-cov needs.
- GitHub's Secure Shell (SSH) host keys in the user's known hosts file, so that git can verify GitHub when it pushes.
- `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH` for every process, whether or not a login shell started it, so that a test binary finds the Slang shared library at run time (Section 4.4 of the architecture design).
- git, CMake 3.22 or later, Ninja, and a C++17 compiler, which `test-static` needs to build Slang on the static route (Section 9.3 of `docs/design/slang-compiler-sys.md`).

The image is not required to provide a toolchain for the declared `rust-version`, because `check-msrv` runs in CI and installs that toolchain on first use (Section 17.3). The Technology Decisions section of `AGENTS.md` lists what the image provides.

The image does not yet meet the item about `LD_LIBRARY_PATH`. A login shell gets both the directory of the Vulkan loader and `$VULKAN_SDK/lib`, but a process that no login shell started gets only the directory of the Vulkan loader.

slang-rs-env contains the fix to the `ENV` instruction of its Containerfile, which takes effect at the next image rebuild.

The gap does not affect the recipes, because each recipe that runs compiled code which loads Slang supplies the library path on its own command line (Section 4.2). Until the rebuild, a test binary that links Slang still fails to start when a process that no login shell started runs it outside a recipe (Section 17.2).

## 14. Assumptions to Confirm

Each row is a fact that this document relies on and that has not been checked. A section that relies on one of them cites its identifier. The change that confirms or corrects an assumption states the result in the section that relies on it and removes the row.

An identifier is never reused, so the next assumption takes TA-8.

No assumption is open.

## 15. Open Questions

Each row is a question that this document leaves open. The design document in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row.

| Identifier | Question | Owning design document | Milestone |
| --- | --- | --- | --- |
| TQ-4 | How is a release carried out: which recipe publishes, in which order the crates are published, and where the crates.io token is kept outside the repository? | This document | M7 |
| TQ-6 | How does each published crate carry the text of its licence, which the GPL requires to accompany the work, given that Cargo does not package the root `LICENSE` when a manifest sets only the `license` field (Section 10)? | This document | M7 |

An identifier is never reused, so the next question takes TQ-10.

## 16. Security Considerations

The threat model has six parts:

1. A dependency, a build script, or a procedural macro in the dependency graph is malicious or compromised. It runs with the permissions of the developer or the job whenever a check compiles the workspace, including Clippy, the documentation build, and the tests.
2. An action that a workflow uses is compromised, or its owner moves a tag to different code.
3. An attacker on the network or at a download host tampers with a tool, the Vulkan SDK, or the advisory database that a check or a job downloads.
4. The author of a pull request tries to read a secret or to write to the repository through CI.
5. A pre-commit hook repository changes the code that its hooks run.
6. A file or a file name that a checker reads, such as one in a pull request, holds terminal control sequences. A checker that printed them unchanged would send them to the terminal of the developer, which is terminal escape injection.

The design addresses each part:

1. For dependencies, cargo-deny rejects crates with known advisories and crates from sources other than crates.io (TOOL-26), and the allow-list admits only reviewed licences (TOOL-24). `Cargo.lock` fixes every version, and the checks run inside the development container and not on the developer's host.
2. For actions, TOOL-20 pins each one to a commit SHA, which `check-workflow-pins` enforces, and TOOL-19 limits what a compromised step can do. The checkout step does not leave the token in the git configuration of the runner (Section 8.1).
3. For downloads, TOOL-21 and TOOL-23 verify every file before use, and the workflow restores nothing from a cache. The installer action carries its checksums in a manifest that its commit SHA pins.

   The expected checksum of the Vulkan SDK is recorded in the workflow and not fetched from the host that serves the SDK, and the download is verified before it is unpacked or run (Section 8.3). A build script downloads nothing (SYS-5 of the coding standard).
4. For pull requests, TOOL-22 keeps secrets out of every workflow and forbids the event that runs pull request code with secrets, and `check-workflow-pins` reports that event. A run for a pull request from a fork gets a read-only token and no secrets (Section 8.2).
5. For hooks, `.pre-commit-config.yaml` pins the repository `pre-commit-hooks` to a commit SHA, and the local hooks run only recipes of this repository.
6. For checker output, every custom checker escapes its output (Section 7.1), so a checked file cannot send a control sequence to the terminal.

These attacks are out of scope:

- A compromise of GitHub, crates.io, or LunarG themselves defeats every check here, and no repository configuration can detect it.
- The build of the container image is defined and secured in the repository slang-rs-env.
- A compromise of the developer's machine outside the container is a matter of that machine's security, not of this repository.

The topics of unsafe code, foreign function interface invariants, untrusted shader source, and file paths do not apply, because this document defines no code that calls Slang or compiles shader source.

The residual risk has seven parts:

1. A new dependency's build script and procedural macros run on the first build that includes them, before any reviewer has read them, and cargo-deny finds only vulnerabilities that are already published.
2. A checksum shows only that a file matches what the upstream published, and the first download of a file whose upstream publishes no checksum is trusted on first use. A checksum that comes from the same host as the file, as the checksum of actionlint does, proves integrity but not origin, so an attacker who controls that host can replace the file and its checksum together. Because CI installs the latest version of each tool, a new release of a tool runs in CI without any change to this repository and before anyone has reviewed it.
3. A pinned commit of an action is as trustworthy as the review it received when it was pinned, and the manifest of the installer action is as trustworthy as that commit.
4. `check-deny` fetches the advisory database on every run (Section 17.6), so a tampered database could hide an advisory.
5. `check-workflow-pins` reads workflows line by line, so a reference written in a form that it does not read (Section 7.8) can pass it. The permissions of each job, the use of secrets, and whether each download step verifies its file depend on review.
6. The escaping covers only the custom checkers of Section 7. `scripts/check-commit-message` and the external tools that the recipes run, such as shellcheck, typos, actionlint, and Cargo, print their output without this escaping, and this document does not specify how they handle control characters. Because a backslash prints unchanged, escaped output can be ambiguous to a reader, which misleads but sends no control sequence.
7. The installers of the Vulkan SDK for macOS and Windows fetch metadata from LunarG's remote repository during the installation (Section 8.3), and the recorded checksum does not cover that metadata, so a tampered response is not detected. Two facts limit this risk: the files that the installer installs come from the verified download, and the step selects only the core component. No supported option that installs without this access is known.

## 17. Operational Considerations

### 17.1. Recipes and Parameters

Section 4.2 lists every recipe. This document defines no Cargo feature.

This table lists every recipe parameter and its default. Every other recipe takes no parameters.

| Recipe | Parameter | Default |
| --- | --- | --- |
| `check-prose` | Paths of files to check | Every Markdown file and every Rust file in the repository |
| `check-rust-comments`, `check-doc-headings` | Paths of files to check | Every Rust file in the repository (TOOL-14) |
| `check-doctest-attributes` | Paths of files to check | Every Rust file and every `README.md` file in the repository (TOOL-14) |
| `check-feature-docs` | Paths of Cargo manifests to check | The workspace of the repository (Section 7.5) |
| `check-slang-call-guard` | Paths of Rust files to check | Every Rust file under `crates/slang-compiler/src` (Section 7.9) |
| `check-workflow-pins` | Paths of workflow and composite action files to check | Every workflow under `.github/workflows/` and every composite action under `.github/actions/` (Section 7.8) |
| `check-commit-message` | The commit message file | None. The parameter is required. |
| `merge` | The branch, and further arguments for `scripts/merge-branch` | None for the branch, which is required |
| `install-vulkan-sdk` | The SDK version, as four numbers separated by full stops, and the expected SHA-256 checksum of the download for the platform, as 64 lowercase hexadecimal characters (Section 8.3) | None. Both are required. `just` rejects a missing or extra argument before the script runs, and the script exits with status 2 on a malformed one |

### 17.2. Environment Variables

This table lists every environment variable that the recipes, the hooks, or the checks read.

| Variable | Read by | Default and behaviour when unset |
| --- | --- | --- |
| `CARGO_BUILD_WARNINGS` | Cargo, in the commands of Section 4.8 of the coding standard | The recipes set it to `deny` for those commands. Outside them it is unset, and Cargo reports warnings without failing. |
| `VULKAN_SDK` | The build script of `slang-compiler-sys`, `check-vulkan-sdk`, `generate-bindings`, the regeneration test that `test-gpu`, `check-bindings`, and `coverage` run on Linux, and the `justfile`, which builds the library path assignment from it | Section 19.2 of the architecture design. The development container sets it, and on a CI runner `install-vulkan-sdk` sets it for the later steps of the job (Section 8.3). Without it, the build script stops every command that compiles the workspace, so every recipe that compiles fails except `check-doc-without-slang`, which removes it (Section 19.2 of `docs/design/slang-compiler-sys.md`). `check-vulkan-sdk` exits with status 2 when it is unset or empty, and with status 1 when it names a path that is not a directory. `generate-bindings` and the regeneration test fail when it is unset. When it is unset or empty, the library path assignment is empty. `test-static` removes it from the environment of its command. |
| `RUNNER_TEMP`, `GITHUB_ENV`, `GITHUB_PATH` | `install-vulkan-sdk` | Set by GitHub Actions on every runner. When one of them is unset or empty, the recipe exits with status 2 and a message that names it and says that this is not a GitHub Actions runner. |
| `SLANG_SOURCE_DIR` | The build script of `slang-compiler-sys`, and `test-static` | Section 19.2 of the architecture design. The development container does not set it. A present value selects the static route (Section 9.3 of `docs/design/slang-compiler-sys.md`), so every recipe that compiles the workspace then builds Slang from source in the `OUT_DIR` of its build, once for each target directory and profile. `test-static` fails with a message that names it when it is unset or empty. In the container, `/opt/slang` is a source tree that the route accepts. |
| `CARGO_TARGET_DIR` | Cargo | Unset, so Cargo builds in `target/`. `test-static` sets it to `target/static` on its own command line, so the static build does not replace the dynamic build of `target/`. |
| `LIBCLANG_PATH` | `clang-sys`, when the binding generator or its tests run | Section 19.2 of `docs/design/slang-compiler-sys.md` |
| `DOCS_RS` | The build script of `slang-compiler-sys` | Unset, so the build proceeds on the selected route. `check-doc-without-slang` sets it to `1`. |
| `LD_LIBRARY_PATH` | The dynamic loader on Linux, when a test binary starts, and the `justfile`, which reads its earlier value | The recipes that run compiled code which loads Slang set it on their own command line to `$VULKAN_SDK/lib` followed by any earlier value, so they do not depend on the path of a login shell (Section 4.2). In a login shell, the container includes `$VULKAN_SDK/lib` in it. A process that no login shell started lacks that directory until the next image rebuild (Section 11), which affects only a test binary run outside a recipe. Without it, a test binary that links Slang fails to start. On a Linux CI runner, `install-vulkan-sdk` sets it to `$VULKAN_SDK/lib` followed by any earlier value (Section 8.3), so the directory appears twice in a recipe there, which is harmless. `test-static` removes it, and `DYLD_LIBRARY_PATH`, from the environment of its command. |
| `DYLD_LIBRARY_PATH` | The dynamic loader on macOS, when a test binary starts, and the `justfile`, which reads its earlier value | The recipes that run compiled code which loads Slang set it on their own command line to `$VULKAN_SDK/lib` followed by any earlier value (Section 4.2). Outside a recipe, it is unset unless the developer sets it. On a macOS CI runner, `install-vulkan-sdk` sets it to `$VULKAN_SDK/lib` followed by any earlier value (Section 8.3), so the directory appears twice in a recipe there. Whether the assignment on the command line reaches a test binary on a Mac with System Integrity Protection enabled is SA-7 of `docs/design/slang-compiler-sys.md`. |
| `PATH` | The dynamic loader on Windows, when a test binary starts, and every shell | On a Windows CI runner, `install-vulkan-sdk` adds `$VULKAN_SDK\Bin`, which holds `slang-compiler.dll`, and on Linux and macOS runners it adds `$VULKAN_SDK/bin` (Section 8.3). |
| `CLAUDE_PROJECT_DIR` | The hook in `.claude/settings.json` | Set by Claude Code. The hook does not run outside it. |

The workflow also sets these variables for its own steps, and no recipe reads them. Its top-level `env` holds `VULKAN_SDK_SHA256_LINUX`, `VULKAN_SDK_SHA256_MACOS`, and `VULKAN_SDK_SHA256_WINDOWS`.

The step that reads the pin writes `VULKAN_SDK_VERSION`, `SLANG_BUILD_TAG`, and `SLANG_COMMIT` to the file that `GITHUB_ENV` names. The step that installs the SDK passes the version and the checksum for its platform to `install-vulkan-sdk` as arguments (Section 8.3).

### 17.3. Platforms and Minimum Supported Rust Version

The checks run in the development container on Linux x86_64, and CI runs them on the platforms of Section 8.1, with tool versions that can differ from those of the container (Section 8.1). The Vulkan SDK in CI is the pinned version on every platform (Section 8.3).

The workspace declares `rust-version = "1.99"` (TOOL-4), which only a change of TOOL-5 raises, and `check-msrv` verifies it in the CI job `ci-only`.

cargo hack always uses the rustup toolchain named exactly like `rust-version`, which is `1.99`. It installs that toolchain with rustup when it is missing, even when the installed stable toolchain is a later patch release such as 1.99.1. The first run of `check-msrv` on a machine therefore downloads a toolchain and needs network access, and later runs use the installed toolchain.

The root `.gitattributes` holds the single line `* text=auto eol=lf`. git normalizes every file that it detects as text to LF line endings in the repository and checks it out with LF on every platform, whatever the `core.autocrlf` setting of the clone. Without it, the checkout on the hosted Windows runner has CRLF line endings, which breaks a recipe that reads a file line by line, such as `pin`.

The self-tests of the custom checkers do not depend on the line endings of the checkout, because their fixtures create carriage returns from placeholders at run time (Section 7.1). The recipe `pin` and `scripts/check-vulkan-sdk` still accept CRLF input, because a user's own git settings can still produce it.

### 17.4. Failing Checks

This table says what a failure of each check means and what the developer does.

| Recipe | A failure means | What the developer does |
| --- | --- | --- |
| `check-fmt` | A Rust file is not formatted, including a file that a crate includes with `include!`, or `git ls-files` failed or listed no Rust file, as its message says | Runs `just fmt`. For a failed listing, runs the recipe inside the git checkout of the repository |
| `fmt` | `git ls-files` failed or listed no Rust file, as its message says, or rustfmt cannot parse a file | Runs the recipe inside the git checkout of the repository, or fixes the syntax error that rustfmt reports |
| `test-static` | `SLANG_SOURCE_DIR` is unset or empty, the build script stopped the static route with a message of Section 9.6 of `docs/design/slang-compiler-sys.md`, or a test failed | Sets `SLANG_SOURCE_DIR` to a clean git checkout of the pinned commit, follows the recovery of Section 19.4 of that document, or fixes the code or the test |
| Every recipe that compiles the workspace, except `check-doc-without-slang` | The build script of `slang-compiler-sys` stopped the build, with a message that Section 9.6 of `docs/design/slang-compiler-sys.md` lists, such as "VULKAN_SDK is not set" or another build tag | Follows the recovery for that failure in Section 19.4 of `docs/design/slang-compiler-sys.md`, such as installing the pinned SDK and setting `VULKAN_SDK`, or unsetting `SLANG_SOURCE_DIR` |
| `check-clippy`, `check-doc`, `check-doc-without-slang`, `check-doctest` | A lint fired | Follows Section 4.4 of the coding standard and the lint's row in Section 4.5 |
| `test`, `test-gpu`, `test-hw-gpu` | A test failed, a test in a GPU tier found no adapter (ARCH-23 of the architecture design), or a test binary could not start because the loader did not find the Slang shared library, as when `VULKAN_SDK` is unset in the environment of `just` | Fixes the code or the test, runs the tier on a machine with an adapter, or sets `VULKAN_SDK` to the pinned SDK so that the recipe supplies the library path |
| `check-bindings` | The regeneration test failed, with the message for its cause that Section 7.6 of `docs/design/slang-compiler-sys.md` lists, or the filter selected no test, as on Windows and macOS | For a difference, runs `just generate-bindings` and commits both regenerated files with the change that caused it. For a missing `VULKAN_SDK`, header directory, or libclang, installs or sets what the message names. On Windows or macOS, runs the recipe on Linux |
| `generate-bindings` | The binding generator failed, with an error of a kind that Section 7.7 of `docs/design/slang-compiler-sys.md` lists | Follows the recovery for that failure in Section 19.4 of `docs/design/slang-compiler-sys.md` |
| `check-deny` | A licence is not allowed, an advisory applies, a crate is duplicated, or a source is not crates.io. The exit status is the sum of 1 for advisories, 2 for bans, 4 for licences, and 8 for sources | Applies Section 9: replaces or upgrades the dependency, or records an exception that TOOL-24 or TOOL-26 permits |
| `check-prose`, `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs` | Exit status 1: a rule of the writing guide is broken at each reported `path:line`, and the message names the problem, such as "comment other than API documentation or a safety comment" or "feature 'name' is not named in a code span in the '# Features' section". Exit status 2: a file cannot be read, including a file that ends inside an unterminated literal, comment, or attribute, `cargo metadata` failed, the checker ran with no paths outside a git repository, or the setup is broken, as the message on standard error says (Section 7.1) | For status 1, fixes the text as the rule that Section 7 names requires. For status 2, fixes the file or the setup that the message names, such as by closing the unterminated item at the reported line |
| `check-workflow-pins` | Exit status 1: a rule of Section 8.2 is broken at each reported `path:line`, such as "uses: '...' is not pinned to a full 40-character commit SHA", "workflow uses the pull_request_target event", "workflow has no top-level permissions key", or "top-level permissions '...' grant more than read access". Exit status 2: as for the other custom checkers | Pins the action to the full commit SHA of the reviewed release, removes the event, or sets the top-level permissions to `contents: read` and moves any extra permission to the job that needs it (TOOL-19) |
| `check-slang-call-guard` | Exit status 1: a call into Slang in `slang-compiler` breaks SLC-32 of `docs/design/slang-compiler.md` at each reported `path:line`, such as "call of IModule::getName is not inside a closure passed to abort_on_exception (SLC-32)". Exit status 2: as for the other custom checkers, and also a file with a bracket without its partner, or bindings that cannot be read or declare no C function or no thunk (Section 7.9) | For status 1, makes the call the only call into Slang in a closure literal passed to `abort_on_exception`, as SLC-32 requires. For status 2, fixes the file or the bindings that the message names, and regenerates the bindings with `just generate-bindings` instead of editing them |
| A self-test recipe | A case under `scripts/fixtures/<checker>/` gave a different standard output, standard error, or exit status from the expected one, which the recipe prints after "FAIL" with the name of the case. Exit status 2 means that the checker or its cases cannot be read | Fixes the checker, or, when the change to its behaviour is intended, the expected files of the case |
| `check-shell` | shellcheck reported a finding | Fixes the script or recipe |
| `check-typos` | typos found a spelling mistake | Fixes the word, or accepts a correct word in `typos.toml` (TOOL-30) |
| `check-toml` | A TOML file is not formatted | Runs `just fmt` |
| `check-workflows` | actionlint reported a finding in a workflow | Fixes the workflow |
| `check-unused-deps` | A dependency is declared and not used | Removes it |
| `check-miri` | Miri found undefined behaviour | Fixes the unsafe code, following Section 9 of the coding standard |
| `coverage` | A test failed, or cargo llvm-cov did not write the report. In the CI job `coverage`, the upload step also fails when `target/lcov.info` does not exist | Reads the message of cargo llvm-cov, and fixes a failing test as for `test-gpu` |
| `pin` | Exit status 1: `crates/slang-compiler-sys/pin.rs` does not define one of the three constants in the form `pub const NAME: &str = "value";`, and the message says "crates/slang-compiler-sys/pin.rs does not define NAME as a string constant" with the name of the constant | Restores the constant in that form (Section 8.1 of `docs/design/slang-compiler-sys.md`) |
| `install-vulkan-sdk` | Exit status 1: the checksum of the download differs from the expected one, and the message "install-vulkan-sdk: SHA-256 mismatch" gives both values, or the macOS archive does not hold exactly one installer application. Exit status 2: a malformed argument, an unset or empty `RUNNER_TEMP`, `GITHUB_ENV`, or `GITHUB_PATH`, or an unsupported platform. A missing or extra argument fails in `just` before the script runs. A failed download, unpacking, or installation stops the script with the non-zero status of the command that failed | For a mismatch, compares the checksum in the workflow with the one that LunarG publishes for the file (Section 8.3) before changing anything, because a mismatch can mean a tampered download. For a failed download, runs the job again |
| `check-vulkan-sdk` | Exit status 1: `VULKAN_SDK` names a path that is not a directory, with the message "check-vulkan-sdk: VULKAN_SDK names <path>, which is not a directory", or the SDK at `VULKAN_SDK` lacks `slang.h`, `slang-tag-version.h`, or the Slang library of the platform, defines another build tag, or has a `slangc` that fails or prints another tag, as each message after "check-vulkan-sdk:" says. Exit status 2: an argument was given, `VULKAN_SDK` is unset or empty, or `just pin` failed or printed no `SLANG_BUILD_TAG` | Installs the SDK of the pin and points `VULKAN_SDK` at it, or fixes `pin.rs` when `just pin` fails |
| `check-features`, `check-msrv`, `check-semver` | A feature combination does not compile, the code needs a newer toolchain than `rust-version`, or the API has a breaking change | Fixes the code, raises `rust-version` under TOOL-5, or releases the change as breaking |

Clippy can report a `clippy::cargo_common_metadata` warning again after the manifest has been fixed, because it replays its cached result until a source file of the crate changes. The developer then updates the modification time of a source file of the crate, or cleans the build output of the crate, and runs the check again.

No agent weakens a configuration to make a check pass, as Hard Rule 6 of `AGENTS.md` states.

This document states no expected duration for a check. The exception is `test-static`, whose first run builds Slang from source, which takes several minutes. A later run whose build script finds a matching stamp takes seconds.

A check that passes in the development container and fails in CI, or the reverse, can come from a difference in tool versions (Section 8.1). The developer compares the version in the log of the job with the version in the container before changing the code.

A job that GitHub cancelled before any step ran, with the annotation "The job was not acquired by Runner of type hosted even after multiple attempts", did not fail because of the change. No hosted runner picked the job up, which is a problem of runner capacity. The developer reruns that job and judges the change by the result of the rerun.

### 17.5. Logs

The recipes print the output of the tools they run and keep no log. GitHub Actions keeps the log of each job.

The recipe `coverage` prints the output of the build and the tests and no coverage summary. With the LCOV options, cargo llvm-cov ends its output with the line `Finished report saved to target/lcov.info`. The report is the LCOV file `target/lcov.info`, which the CI job `coverage` uploads as the artifact `coverage-lcov` and GitHub keeps for 14 days (Section 8.1).

`install-vulkan-sdk` prints the download address, the size and duration of the download, the verified checksum, the duration of the installation, the value of `VULKAN_SDK`, and the installed size. `check-vulkan-sdk` prints the files that it finds in the SDK, the content of `slang-tag-version.h`, and the output of `slangc -version`.

On the static route, the build script of `slang-compiler-sys` writes the output of `cmake` and Ninja to `slang/build.log` under its `OUT_DIR`, which is inside `target/static` for `test-static` (Section 19.5 of `docs/design/slang-compiler-sys.md`).

### 17.6. Network Access

`just check` needs network access. `check-deny` fetches the advisory database from <https://github.com/RustSec/advisory-db> on every run, and fails with exit status 1 when it cannot.

With a database already cached, `cargo deny --offline check`, with the global option `--offline` before `check`, works without network access. `check-deny` runs the command of Section 4.8 of the coding standard without that option, so it always checks against the current database.

The first run of `check-miri` after an image rebuild also needs network access, because Miri builds a sysroot and fetches the dependencies of the standard library. Later runs of `check-miri` work offline.

Outside `check`, the first run of `check-msrv` on a machine downloads a toolchain (Section 17.3). `test-static` needs no network access once its Cargo dependencies are fetched, because the static route downloads nothing (SCS-28 of `docs/design/slang-compiler-sys.md`).

`install-vulkan-sdk` downloads the SDK from `sdk.lunarg.com`. On macOS and Windows the installer also contacts LunarG's remote repository (Section 8.3).

### 17.7. Verifying and Merging a Branch

This section gives the procedure for verifying a branch in CI and merging it. The project opens no pull requests, so a push of the branch is what runs CI on it (Section 8.1). The agent follows these steps:

1. It pushes the branch with a plain push, which starts one run for the last commit of the branch.
2. It reads the run for that exact commit from the run itself, with these commands, in which `<branch>` and `<id>` are placeholders. The first prints the identifier, status, conclusion, and commit of the latest run of the branch, and the second prints each job of the run with its conclusion:

   ```sh
   gh run list --branch <branch> --limit 1 --json databaseId,status,conclusion,headSha
   gh run view <id> --json jobs
   ```

3. It runs `just merge <branch>` only when the run's `headSha` is the last commit of the branch, its conclusion is `success`, and every job succeeded.

The agent reads the run itself and never the exit status of a command whose output is piped into another program. The exit status of a pipeline is the status of its last program, so it says nothing about the run, and a merge gated on it can put a failing commit on `main`. A job cancelled for lack of a runner is rerun, as Section 17.4 states, and the merge waits for the rerun.

`just merge` builds the message of the squashed commit from every commit of the branch (Section 4.2). A branch with several work-in-progress commits is therefore merged through a fresh branch.

The agent creates a new branch from `main` that holds the identical tree of the verified branch as one commit with the final message, and runs `just merge` on the fresh branch. The fresh branch holds the tree that CI verified only when the verified branch contains the current `main`. No pushed history is rewritten.

`just merge` deletes the merged branch on `origin` when its tip there is part of the squash. With the fresh-branch procedure, the merged branch is the fresh one, so the branch that was pushed for CI stays on `origin`. Its local branch and worktree count as squash-merged once `main` holds its tree, so `just worktree-prune` removes them.

## 18. References

### 18.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 18.2. Informative References

- [FSF-LICENSES] Free Software Foundation, "Various Licenses and Comments about Them", version dated 2026-04-22, <https://www.gnu.org/licenses/license-list.html>, read on 2026-11-11.
- [GPL-FAQ] Free Software Foundation, "Frequently Asked Questions about the GNU Licenses", the compatibility matrix, <https://www.gnu.org/licenses/gpl-faq.html#AllCompatibility>, read on 2026-11-11.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary. TOOL-1 and TOOL-2, which governed the one-time creation of the workspace in M0, are retired, and their labels are not reused.

| Label | Section | Summary |
| --- | --- | --- |
| TOOL-3 | 10 | The examples are one member, `slang-rust-examples`, with one binary target per example. |
| TOOL-4 | 10 | Licence, `rust-version` 1.99, repository, and readme are workspace fields that every member inherits. |
| TOOL-5 | 10 | `rust-version` changes only in a change whose purpose is to raise it. |
| TOOL-6 | 4.1 | Recipes have the name, group, and `check` membership of Section 4.2. |
| TOOL-7 | 4.1 | Recipes run the commands of Section 4.8 of the coding standard as written there, including the file list of the formatting command, adding only a nextest profile, the arguments that Section 4.8 names for `coverage`, the environment of Section 10, or, when they run code that loads Slang, the library path assignment of SCS-32, except that a recipe that selects named tests and must fail on an empty selection adds its filter and `--no-tests=fail`, and `test-static` adds `-p slang-compiler-sys`, removes `VULKAN_SDK` and the loader path variables, sets `CARGO_TARGET_DIR=target/static`, and passes `--no-tests=fail`. |
| TOOL-8 | 4.1 | `check` passes in the container without a hardware GPU or a display. |
| TOOL-9 | 4.1 | No recipe for a tool before the image provides it. |
| TOOL-10 | 5 | The profiles `default`, `gpu`, and `hw-gpu` select tests by whole module path segments, and each profile other than `default` sets its own filter. |
| TOOL-11 | 5 | No profile retries a failed test. |
| TOOL-12 | 5 | The test recipes pass when their profile selects no test, through `--no-tests=warn`. |
| TOOL-13 | 6 | Commit hooks run only checks that do not compile the workspace. |
| TOOL-14 | 7.1 | Custom checkers follow the output and exit status conventions of `scripts/check-prose`, except that a checker whose required input is missing exits with status 2 before it checks any file. |
| TOOL-15 | 7.1 | Each custom checker has a self-test recipe in `check` that runs it on accepted and rejected inputs stored under a name that no tool reads. |
| TOOL-16 | 7.2 | The comment checker reads Rust tokens and ignores literals. |
| TOOL-17 | 7.7 | Landing a checker moves its rule to enforced in every document in the same change. |
| TOOL-18 | 8.1 | Workflows run every check through a recipe. |
| TOOL-19 | 8.2 | Workflow tokens are read-only by default, with extra permissions per job. |
| TOOL-20 | 8.2 | Every action is pinned by a full commit SHA. |
| TOOL-21 | 8.2 | Every download, and every file restored from a cache, is verified against a published SHA-256 checksum before use, and installer actions verify each download and do not fall back. |
| TOOL-22 | 8.2 | No workflow uses `pull_request_target`, reads a secret, or publishes. |
| TOOL-23 | 8.3 | The SDK step reads the pin, verifies the download, sets `VULKAN_SDK`, and makes Slang findable. |
| TOOL-24 | 9 | The licence allow-list starts with `GPL-3.0-or-later` and grows only with a dependency. |
| TOOL-25 | 9 | Path dependencies between members carry a version, and `bans.allow-wildcard-paths` is `false`. |
| TOOL-26 | 9 | Advisories, duplicates, and other sources fail cargo-deny, with exceptions only per item and with a reason. |
| TOOL-27 | 10 | Published crates carry the full metadata, and unpublished members set `publish = false`. |
| TOOL-28 | 11 | Image changes happen in slang-rs-env, and the first change after a rebuild that adds or removes a tool updates the documents. |
| TOOL-29 | 11 | The image provides the tools of Section 4.8, the nightly toolchain with Miri, llvm-tools, GitHub's SSH host keys, the SDK library path for every process, and git, CMake, Ninja, and a C++17 compiler for the static route. |
| TOOL-30 | 4.1 | `taplo.toml` sets four-space indentation and excludes `target/` and `.worktrees/`, and `typos.toml` accepts the correct words that typos reports. |
| TOOL-31 | 9 | `deny.toml` denies unused licence allow-list entries and unused advisory ignores. |

## Change Log

- 2026-11-12: Audited the document against the repository: removed the current state, the M0 repository changes, documentation changes, and implementation order, the dated run records, and the measurements, moved TOOL-3, TOOL-4, and TOOL-5 to Section 10, TOOL-30 with the typos and taplo configuration to Section 4.1, and the purpose of `.gitattributes` to Section 17.3, retired TOOL-1 and TOOL-2, merged the licence, skip, and advisory lists of Section 9 into one table each, and split long paragraphs.
- 2026-11-11: Recorded step 5 of Section 11 of `docs/design/slango-macros.md` in Sections 1.6, 3.2, 3.3, and 10: the binary target `typed-kernel`, the workspace dependency `slango-macros`, and the documentation of a caller of the macro by `check-doc-without-slang`.
- 2026-11-11: Recorded step 3 of Section 11 of `docs/design/slango-macros.md` in Sections 1.6, 3.2, 3.3, and 9: the members `crates/slango-codegen` and `crates/slango-macros`, the workspace dependencies `prettyplease`, `proc-macro2`, `quote`, `slango-codegen`, `syn`, and `trybuild`, and the crates of M6 that also need the skipped `syn` 2.
- 2026-11-11: Recorded step 3 of Section 11 of `docs/design/slang-slint.md`, the example `shader-playground`, in Sections 1.6, 3.2, 3.3, and 9, with the licences `BSD-2-Clause`, `BSD-3-Clause`, and `BSL-1.0` and their evidence, eight `skip` entries, and the owner's exceptions for four advisories with their removal conditions, and corrected the statements that winit's `wayland-csd-adwaita` is not built and that Slint turns on no feature of the examples' winit.
- 2026-11-11: Recorded step 1 of Section 11 of `docs/design/slang-slint.md` in Sections 1.6, 3.2, 3.3, 8.1, and 9. These are the member `crates/slang-slint` with the workspace dependency `slint`, the licence `GPL-3.0-only` with its evidence, four `skip` entries, the owner's exception for advisory RUSTSEC-2026-0441, and fontconfig in the Linux jobs. The BSD and BSL licences and eight planned `skip` entries wait for step 3. Added the reference [GPL-FAQ].
- 2026-11-11: Recorded in Section 9, as planned, the dependency Slint 1.18 that the design of M5 in `docs/design/slang-slint.md` adds, with the licences `GPL-3.0-only`, `BSD-2-Clause`, `BSD-3-Clause`, and `BSL-1.0`, twelve `skip` entries, the installation of fontconfig in the Linux jobs, and the correction of the description of winit's features that its step 1 settles.
- 2026-11-11: Recorded in Section 9 that step 11 of `docs/design/slango.md` added winit 0.30.13 to the examples package with the features that leave out the crates under BSD licences, and fifteen single `skip` entries for the duplicate versions that it brings, with TOOL-26 unchanged. Recorded in Sections 3.2 and 3.3 the binary target `windowed-renderer` and `winit` in `[workspace.dependencies]`.
- 2026-11-11: Recorded in Section 9, as planned, the dependency winit 0.30 that the design of M4 in `docs/design/slango.md` adds to the examples package, with the licence and duplicate-version findings that its step 11 settles.
- 2026-11-11: Recorded in Sections 3.2 and 3.3 that step 6 of `docs/design/slango.md` added the binary target `compute-kernel` and `slango` in `[workspace.dependencies]`.
- 2026-11-11: Recorded in Section 8.1 that `slango` dispatches kernels on WARP and hosted Metal, and removed the fallback to `just test` that a negative answer would have needed.
- 2026-11-11: Recorded in Section 8.1 that `slango` loads modules on WARP and hosted Metal, so only dispatch stays open on Windows and macOS.
- 2026-11-11: Recorded step 1 of Section 14 of `docs/design/slango.md` in Sections 1.6, 3.2, 3.3, 5, 8.1, and 9: the new member `crates/slango` with its workspace dependencies, the nextest override that keeps the output of its `gpu` tests, lavapipe in the `check` and `coverage` jobs, the first runs of the `gpu` tier on all three platforms, and the licences `MIT`, `Unicode-3.0`, and `Zlib` with the evidence of their compatibility and the four duplicate-version exceptions in `deny.toml`. Added Section 18.2 with the licence list of the FSF.
- 2026-11-11: Recorded as planned for step 1 of Section 14 of `docs/design/slango.md`, in Sections 1.6, 5, 8.1, and 9, the licences `MIT`, `Zlib`, and `Unicode-3.0` and the duplicate-version exceptions that wgpu needs, and lavapipe in the `check` job, and narrowed the open part of Q-21 of the architecture design to Windows and macOS in Section 8.1.
- 2026-11-11: Recorded in Section 8.3 that `scripts/install-vulkan-sdk` removes its download on a best-effort basis and warns when the removal fails, and in Section 1.6 that M2 is complete.
- 2026-11-11: Recorded in Sections 1.6, 3.2, and 3.3 that step 5 of `docs/design/slang-compiler.md` added the examples package and `slang-compiler` in `[workspace.dependencies]`, and that no recipe runs the example.
- 2026-11-11: Recorded in Sections 1.6, 3.3, and 9 that step 4 of `docs/design/slang-compiler.md` added `insta` as a development dependency with no new licence, and in Section 7.9 the 129 sites that the checker of SLC-32 recognizes after that step.
- 2026-11-11: Added Section 7.9 for the custom checker `check-slang-call-guard`, which enforces SLC-32 of `docs/design/slang-compiler.md` and lists what stays a judgement rule, named it in the introduction of Section 7, added its recipe and its self-test recipe to Sections 4.2, 4.3, and 17.1 and its failures to Section 17.4, recorded in Section 6 that `.claude/settings.json` has no entry for either recipe, in Section 7.1 its run over the Rust standard library, and in Section 1.6 the state and CI run 42907583067, and removed TQ-9 as answered by the checker. Amended TOOL-14 and its summary in Appendix A to allow the exit before any check when a required input is missing.
- 2026-11-11: Recorded in Section 1.6 that steps 2 and 3 of `docs/design/slang-compiler.md` changed no tooling, that step 3 passed all six CI jobs in run 42898756060, and that M2 needs one custom checker, and added TQ-9 to Section 15 for the checker that every call of `slang-compiler` into Slang runs inside its abort guard.
- 2026-11-11: Recorded in Sections 1.6, 8.1, and 8.2 that the workflow also runs on every push to a branch that matches `feature/**`, with a concurrency group that never cancels a run on `main`, the `pull_request` trigger kept for pull requests from forks, which Section 16 also cites, and the run that verified the change, described the `merge` recipe in Section 4.2, added to Section 17.4 how to treat a job that no hosted runner picked up, and added Section 17.7 with the procedure that verifies a branch in CI and merges it, including the fresh-branch merge.
- 2026-11-11: Recorded in Sections 1.6, 3.2, and 3.3 that step 1 of M2 added the member `crates/slang-compiler` and `slang-compiler-sys` in `[workspace.dependencies]` with no change to the tooling, while the examples package stays planned, cited in Section 5 the Miri counts of `slang-compiler`, and recorded in Section 1.6 the failure of the Windows job on `main` for three commits and the run that passed after the fix.
- 2026-11-10: Recorded, as planned, what the design of M2, `docs/design/slang-compiler.md`, needs from tooling: the members `slang-compiler` and `slang-rust-examples` with the binary target `compile-and-reflect` in Section 3.2, the development dependency `insta` with its licences under TOOL-24 and assumption CA-12 in Section 9, and the coverage of the new members by `check-doc-without-slang` in Section 10, and stated in Section 1.6 that it needs no new recipe, hook, checker, or CI job.
- 2026-11-10: Recorded in Section 1.6 that step 6 of M1 changed no tooling and that M1 is complete.
- 2026-11-10: Recorded step 5 of M1: the state in Section 1.6, the amendment of TOOL-7 and its summary in Appendix A that permit the form of `test-static` and state that the file list of the formatting command is part of it, with TOOL-12 to match, the recipe `test-static` as existing and the file list and failure messages of `fmt` and `check-fmt` in Section 4.2, the absence of `test-static` from the allow list of `.claude/settings.json` in Section 6, the local verification of the static route in Section 8.1, git, CMake, Ninja, and a C++17 compiler in TOOL-29 and its summary in Section 11, and the variables, failures, durations, logs, and network access of the new recipe in Section 17.
- 2026-11-10: Recorded step 4 of M1: the amendment of TOOL-7 and its summary in Appendix A that permit the library path assignment in front of a command that runs code which loads Slang, the recipes that carry it, `check-doc-without-slang` as existing, and `generate-bindings` writing both generated files in Section 4.2, the order of `check-rust` and its need for `VULKAN_SDK` in Section 4.3, the allow list of `.claude/settings.json`, which lacks `check-doc-without-slang`, in Section 6, what the Windows and macOS jobs now prove and the CI run in Section 8.1, the environment and command of `check-doc-without-slang` in Section 10, that the gap in the container image no longer affects the recipes in Section 11, the variables `VULKAN_SDK`, `SLANG_SOURCE_DIR`, `DOCS_RS`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` in Section 17.2, the failures of the build script and of a test binary that cannot start in Section 17.4, and the state in Section 1.6.
- 2026-11-10: Recorded step 3 of M1: the recipes `generate-bindings` and `check-bindings` in Sections 4.2 and 17.4, the amendment of TOOL-7 that permits `--no-tests=fail` for a recipe that selects named tests, with TOOL-12, Section 5, and Appendix A to match, the regeneration test that `check` and the Linux CI jobs run in Sections 4.3 and 8.1, with the libclang versions and the results of the CI run, the root `.gitattributes` and the identifier entries of `typos.toml` in Section 3.4, the allow list of `.claude/settings.json` in Section 6, the clean failure of `scripts/check-vulkan-sdk` on a `VULKAN_SDK` that is not a directory and the exports of `scripts/install-vulkan-sdk` on each platform in Sections 8.3, 17.2, and 17.4, and the state in Section 1.6, and removed TQ-8 as answered by `.gitattributes`.
- 2026-11-10: Recorded step 2 of M1: the installation of the pinned Vulkan SDK in every CI job with its measured times and the decision not to cache it in Section 8.3, the recipes `install-vulkan-sdk` in the new group `ci` and `check-vulkan-sdk`, and the portable recipe `pin` in Sections 4.1 and 4.2, the `coverage` job and its artifact in Section 8.1, the installer metadata as a residual risk in Section 16, and the new parameters, variables, failures, and logs in Sections 17.1 to 17.6, corrected every statement that the recipe `coverage` prints a summary, described the exit statuses of the two scripts as they behave, dated the observations from the runners, said in Section 8.3 that the install step sets the loader variables that the SDK's setup script would set, removed TQ-1 as answered, and added TQ-8.
- 2026-11-10: Recorded the first change of M1 in Sections 1.6, 4.2, 5, 8.1, and 9: the recipe `pin`, the nextest profile `default-miri`, the installation of libclang in the `check` job and how it meets TOOL-21, the first CI run with the binding generator, and the licences Apache-2.0 and ISC in the allow-list, and moved the arrival of the `coverage` job to a later change of M1.
- 2026-11-10: Recorded the answers of `docs/design/slang-compiler-sys.md` to TQ-2 and TQ-5 in Section 4.2 and removed both questions, pointed TQ-1 and Section 8.3 to the requirements of that document, recorded in Section 8.1 that no job builds the static route, updated the environment variables of Section 17.2, added the recipe `test-static` to Section 4.2, and recorded in Section 7.2 that the generated bindings get no exemption from the comment checker.
- 2026-11-10: Recorded the fixes from a code review of the custom checkers: the escaping of their output and the terminal escape injection that it prevents in Sections 7.1 and 16, unterminated input as a read error, stable error wording, the handling of a shebang line, and the new placeholders and marker file of the self-test runner in Section 7.1, the Claude Code hook on Rust files in Section 6, and the new failures in Section 17.4.
- 2026-11-10: Recorded M0 as complete and changed the Status to "Living document", described the custom checkers, their self-test runner, the CI workflow, and the new hooks as implemented, added Section 7.8 for the workflow pin checker, corrected TOOL-15, reworded TOOL-21 to the checksum verification that the workflow performs, and removed TQ-3 and TQ-7 as answered.
- 2026-11-10: Recorded steps 1 to 4 of Section 13 as done in Sections 1.6, 12, and 13, corrected the order of `check-rust`, the commands of `check-features` and `check-msrv`, and the description of `typos.toml` to match the implementation, and recorded the first run of `check-msrv`.
- 2026-11-10: Recorded the tool behaviour verified after the image rebuild, resolved the open assumptions, moved the image-dependent recipes into M0 and `check-semver` to M7, and added TOOL-30, TOOL-31, TQ-6, and TQ-7.
- 2026-11-10: Initial version.
