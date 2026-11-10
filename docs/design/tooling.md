# Tooling Design

| Field | Value |
| --- | --- |
| Status | Living document |
| Date | 2026-11-10 |
| Applies to | The repository's configuration, recipes, hooks, and continuous integration |

## Abstract

This document specifies how the deterministic checks of the slang-rust repository are installed and run. It defines what milestone M0 changes in the repository: the removal of an abandoned sketch crate, the one skeleton crate that gives every tool something to check, and the configuration files that are added. It defines the `just` recipes and which of them make up the aggregate check that runs before every merge, the nextest configuration that implements the test tiers, the git hooks, the small custom checkers that move rules of the writing guide from review to a tool, the continuous integration workflow, its security rules, and how it installs the pinned Vulkan SDK on each runner, the dependency policy, the readiness for publishing, and the requirements on the development container image. It gives the order in which M0 makes these changes, so that every step leaves the aggregate check passing, and it lists the questions still open. Milestone M0 is complete: the skeleton crate, the configuration files, the recipes, the nextest profiles, the hooks, the custom checkers, and the continuous integration workflow are implemented.

## 1. Introduction

### 1.1. Problem

The coding standard specifies a strict lint configuration and a set of tools, and the writing guide specifies rules for documentation, but before M0 the repository ran almost none of them, and only the prose, shell, and commit message checks had recipes. Until a tool is installed and run, every rule it would enforce is a judgement rule that a reviewer checks by hand, which the repository's central principle forbids wherever a tool can do the work.

### 1.2. Purpose

This document turns the specifications of the standards documents into installed configuration and runnable checks. It follows the principle that Hard Rule 4 of `AGENTS.md` and Section 3.5 of the writing guide state: anything a deterministic tool can check is checked by a tool and never by a reviewer. Milestone M0 of the roadmap in Section 15.3 of `docs/design/architecture.md` implements this document.

### 1.3. Audience

The primary readers are the coding agents that implement M0 and the agents that later add a recipe, a checker, or a continuous integration (CI) job. The secondary readers are reviewers, and the owner of the repository, who approves its decisions.

### 1.4. Scope

This document covers the files that M0 adds and removes, the workspace manifest fields that are not lints, the recipes, the nextest configuration, the hooks, the custom checkers, the CI workflow, the cargo-deny policy, publishing readiness, and the requirements on the container image.

### 1.5. Non-Goals

Each fact has one owner, as Section 4.5 of the writing guide requires, and this document cites the owner instead of restating the fact.

- Section 4 of `docs/rust-coding-standards.md` owns the lint tables, the contents of `clippy.toml`, the list of tools with what each one checks, and the exact cargo command lines of Section 4.8. This document says which file receives which section's content, which recipe runs which command, and when.
- `docs/design/architecture.md` owns the crates, their layering, the roadmap, the test tiers of its Section 11, the platforms, and the publishing policy. This document defines only the mechanisms that implement them.
- `AGENTS.md` owns the hard rules and the development workflow. Section 12 of this document lists what M0 changes in it.

This document does not design the binding generator, the build script of `slang-compiler-sys`, or the application programming interface (API) of any crate, which the design documents of the crates own. It does not specify the contents of the container image beyond the requirements of Section 11, because the repository slang-rs-env defines the image.

### 1.6. Current State

Every step of Section 13 is done, and M0 is complete. The repository holds:

- The skeleton crate of Section 3.2, in place of the abandoned sketch, which is deleted.
- The workspace manifest of Section 3.3, with the workspace lint table of the coding standard.
- `clippy.toml`, `LICENSE`, `README.md`, `taplo.toml`, `typos.toml`, `.config/nextest.toml`, and `deny.toml`.
- Every recipe of Section 4.2 that arrives in M0.
- The custom checkers of Sections 7.2 to 7.5 and 7.8, the extension of `check-prose` in Section 7.6, the shared lexer `scripts/rust_lexer.py`, the self-test runner `scripts/self-test-checker`, and the fixtures under `scripts/fixtures/` (Section 7.1).
- The hooks of Section 6, together with the hooks and settings that existed before M0.
- The CI workflow `.github/workflows/ci.yml` of Section 8.

`just check` passes in the development container. The workflow passed on its first run on GitHub, on a draft pull request, as Section 8.1 records.

Step 1 of Section 15 of `docs/design/slang-compiler-sys.md`, the first change of M1, added the recipe `pin` (Section 4.2), the binding generator `tools/bindings-generator` as a second workspace member, the nextest profile `default-miri` (Section 5), the licences Apache-2.0 and ISC in `deny.toml` (Section 9), and the installation of libclang in the `check` job (Section 8.1).

Step 2 of Section 15 of `docs/design/slang-compiler-sys.md` added the installation of the pinned Vulkan SDK to every CI job, with a step that reads the pin through `just pin` (Section 8.3). It added the scripts `scripts/install-vulkan-sdk` and `scripts/check-vulkan-sdk`, the recipe `install-vulkan-sdk` in the new recipe group `ci`, the recipe `check-vulkan-sdk` (Section 4.2), and the CI job `coverage` with the upload of its report (Section 8.1). It also rewrote the recipe `pin` so that it runs on the macOS and Windows runners (Section 4.2). Every job of the workflow passed on GitHub with these changes, on a draft pull request.

Step 3 of Section 15 of `docs/design/slang-compiler-sys.md` added the recipes `generate-bindings` and `check-bindings` (Section 4.2), with the amendment of TOOL-7 that permits a recipe that selects named tests to fail on an empty selection, and the regeneration test of the checked-in bindings, which `check` runs on Linux through `test-gpu` (Section 4.3) and which the Linux jobs `check` and `coverage` therefore run (Section 8.1). It added the root `.gitattributes`, which answers TQ-8, and three identifier entries in `typos.toml` (Section 3.4), and it allowed more recipes in `.claude/settings.json` (Section 6). It made a `VULKAN_SDK` that is not a directory a clean failure of `scripts/check-vulkan-sdk` and corrected the usage text of `scripts/install-vulkan-sdk` (Section 8.3). Every job of the workflow passed on GitHub with these changes, on a draft pull request. The recipes `check-doc-without-slang` and `test-static`, which arrive in later steps of M1, and those that arrive in M7, do not exist yet.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that Section 2.2 of `docs/writing-documentation.md` defines, such as agent, reviewer, enforced rule, and judgement rule, the terms that Section 2.2 of `docs/rust-coding-standards.md` defines, such as lint, workspace lint table, and the check, and the terms that Section 2.1 of `docs/design/architecture.md` defines, such as adapter, software adapter, milestone, nextest profile, and pin. This document calls those three files the writing guide, the coding standard, and the architecture design. It adds these terms:

- action: A reusable unit of steps for a GitHub Actions workflow, published in a git repository and referenced from a workflow.
- aggregate recipe: A recipe that only runs other recipes, such as `check`.
- commit SHA: The full 40-character hexadecimal hash that names one git commit.
- container image: The image, defined in the repository slang-rs-env, from which the development container runs.
- custom checker: A script in this repository that checks one rule of the writing guide or the coding standard that no installed tool checks (Section 7).
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
- skeleton crate: A workspace member that has a manifest and crate-level documentation and no API, and that exists so that every check has a package to run on.
- Software Package Data Exchange (SPDX) licence identifier: The standard short name of a licence, such as `GPL-3.0-or-later`, which Cargo's `license` field takes.
- workflow: A YAML Ain't Markup Language (YAML) file under `.github/workflows/` that defines the jobs that GitHub Actions runs.

## 3. Repository Changes in M0

### 3.1. Removal of the Abandoned Sketch

TOOL-1. An agent MUST delete the whole directory `crates/slang-compiler-sys` in the first change of M0, with its manifest, build script, `wrapper.h`, and sources, and MUST NOT reuse any of its files, because its build script clones and builds Slang from source, which SYS-5 and SYS-7 of the coding standard forbid, and its bindings do not compile (Project Status in `AGENTS.md`). The same change removes the sketch's dependencies `bindgen` and `cmake` from `[workspace.dependencies]` and the key `slang-version` from `[workspace.metadata]` in the root `Cargo.toml`, and regenerates `Cargo.lock`.

### 3.2. Workspace Members and the Skeleton Crate

This section answers the questions of the architecture design about the directories of the workspace members and the form of the examples.

The directories that Section 3.1 of the architecture design lists are confirmed: `crates/<crate-name>/` for each library crate, `tools/bindings-generator/` for the binding generator, and `examples/` for the examples.

TOOL-2. The agent that carries out M0 MUST create exactly one skeleton crate, `slang-compiler-sys` at `crates/slang-compiler-sys/`, and MUST NOT create any other library crate, the binding generator, or the examples member, because every check needs only one package with a library target, and a skeleton that waits through several milestones invites code that has no design document, which ARCH-25 of the architecture design forbids. Each other member is created by the milestone that delivers it.

The skeleton is the simplest member that lets every check run. Clippy, the documentation build, the doctests, nextest, cargo-deny, and the other tools of Section 4.8 of the coding standard all run on a package with a library target, and one such package is enough. `slang-compiler-sys` is chosen because M1, the next milestone, builds it, so the skeleton is replaced first.

The skeleton contains a manifest and `src/lib.rs`. The manifest has the publication metadata of Section 10 and inherits the workspace lint table, as Section 4.2 of the coding standard requires. It has no build script and no `links` key, because `links` names the native library that only the build script of M1 links. `src/lib.rs` contains crate-level documentation only. That documentation has the summary sentence and the paragraph of items 1 and 2 of Section 7.4 of the writing guide, says that the crate has no API yet, and has a `# Features` section that says the crate has no features. The example, the platform support, and the description of how the crate finds Slang, which Section 7.4 of the writing guide also requires, arrive with M1, because there is no API to show and no build script to describe before then.

TOOL-3. An agent MUST create the examples as one workspace member, the package `slang-rust-examples` at `examples/`, with `publish = false`, in which each example is a binary target, because one package needs one manifest with one `publish = false`, one integration-test binary under TEST-10 of the coding standard, and one place for shared helpers. M2 creates this member together with example 1. Building one example builds the dependencies of every example, which the examples accept, because ARCH-14 of the architecture design limits the dependencies of `slang-compiler` and no rule limits those of the examples.

### 3.3. Workspace Manifest

The root `Cargo.toml` keeps resolver 3, edition 2024, the version 0.1.0, and the `authors` field that it has today. It receives the workspace lint table of Section 4.2 of the coding standard, and the root `clippy.toml` receives the configuration of Section 4.3 of the coding standard, as Section 4.1 of the coding standard requires.

TOOL-4. The `[workspace.package]` table MUST declare the `license` field as the SPDX licence identifier `GPL-3.0-or-later`, the `rust-version` field as `1.99`, the `repository` field as the address of the repository on GitHub, and the `readme` field as the root `README.md`, and every member MUST inherit these fields from the workspace instead of setting its own, because a member with its own value can disagree with the licence that cargo-deny checks and with the minimum supported Rust version (MSRV) that Clippy reads.

The value `1.99` is the stable toolchain in the development container on 2026-11-10, as the Technology Decisions section of `AGENTS.md` records. The `readme` field names a file outside each member's directory, which Cargo copies into each package (Section 10).

TOOL-5. An agent MUST NOT change `rust-version` except in a change whose stated purpose is to raise it, and that change updates Section 17.3 and `README.md`, because the MSRV is a promise to users (DEP-4 of the coding standard), and a raise hidden in another change breaks their builds without notice.

### 3.4. Files Added and Removed

This table lists every file that M0 adds, rewrites, or removes, and the file `.gitattributes`, which M1 adds, with the section that defines the content of each and the step that makes the change. A number in the last column is a step of Section 13.

| File | Change | Content defined by | Step |
| --- | --- | --- | --- |
| `crates/slang-compiler-sys/` (the sketch) | Removed | Section 3.1 | 1 |
| `Cargo.toml` | Rewritten | Section 3.3 | 1 |
| `Cargo.lock` | Regenerated | Cargo | 1 |
| `crates/slang-compiler-sys/Cargo.toml`, `crates/slang-compiler-sys/src/lib.rs` | Added | Section 3.2 | 1 |
| `clippy.toml` | Added | Section 4.3 of the coding standard | 1 |
| `LICENSE` | Added | The unmodified text of version 3 of the GNU General Public License (GPL) | 1 |
| `README.md` | Added | Section 7.10 of the writing guide | 1 |
| `justfile` | Extended | Section 4 | 2 to 6 |
| `.pre-commit-config.yaml` | Extended | Section 6 | 2, 5, 6 |
| `.claude/settings.json` | Extended with the new recipes in the allowed commands | Section 6 | 5, 6 |
| `typos.toml`, `taplo.toml` | Added | Section 3.4 | 2 |
| `.config/nextest.toml` | Added | Section 5 | 3 |
| `deny.toml` | Added | Section 9 | 4 |
| `scripts/check-rust-comments`, `scripts/check-doc-headings`, `scripts/check-doctest-attributes`, `scripts/check-feature-docs` | Added | Sections 7.2 to 7.5 | 5 |
| `scripts/rust_lexer.py`, `scripts/self-test-checker`, `scripts/fixtures/` | Added | Section 7.1 | 5 |
| `scripts/check-prose` | Extended | Section 7.6 | 5 |
| `.github/workflows/ci.yml` | Added | Section 8 | 6 |
| `scripts/check-workflow-pins` | Added | Section 7.8 | 6 |
| `.gitattributes` | Added | Section 3.4 | Step 3 of Section 15 of `docs/design/slang-compiler-sys.md` |

No `rustfmt.toml` is added, because Section 4.8 of the coding standard specifies the default rustfmt settings.

The root `.gitattributes` holds the single line `* text=auto eol=lf`. git normalizes every file that it detects as text to LF line endings in the repository and checks it out with LF on every platform, whatever the `core.autocrlf` setting of the clone. This answers TQ-8. Before the file existed, the checkout on the hosted Windows runner had CRLF line endings, which broke the recipe `pin` there (Section 4.2). On 2026-11-10, with the file added, renormalization changed no tracked file, and the repository held no binary file. A simulated Windows clone with `core.autocrlf=true` had 249 files with carriage returns without the file and none with it. The self-tests of the custom checkers still pass, because their fixtures create carriage returns from placeholders at run time (Section 7.1). The recipe `pin` and `scripts/check-vulkan-sdk` still accept CRLF input, because a user's own git settings can still produce it.

TOOL-30. `taplo.toml` MUST set `indent_string = "    "` in its `[formatting]` table and `exclude = ["target/**", ".worktrees/**"]`, and `typos.toml` MUST accept each correct word that typos reports in the repository, because `check-toml` and `check-typos` otherwise fail on files that are correct. taplo indents with two spaces by default, which disagrees with the four spaces that `.editorconfig` sets for `*.toml`, and it does not read `.gitignore`, so without the exclusions it also checks the build directory and every worktree.

On 2026-11-10, before the generated bindings were checked in, typos reported four findings in the repository, and each is correct text: a two-letter fragment of the regular expression that matches the shell names in the shebang checks, twice in `scripts/justfile-recipes` and once in `scripts/check-shell`, and the surname of a cited author in `docs/rust-references.md`. `typos.toml` accepts them with two entries in the `extend-ignore-re` list of its `[default]` table: one matches the whole regular expression, and the other matches the author's full name. typos skips only the text that an entry matches, so it still checks the same letters anywhere else, which accepting the words in `[default.extend-words]` would not do. A document that names this configuration describes the two findings instead of spelling them out, because typos reports such a document too. typos reads its configuration from `typos.toml`, `_typos.toml`, or `.typos.toml`, and M0 uses the first name.

Since step 3 of Section 15 of `docs/design/slang-compiler-sys.md`, `typos.toml` also has a `[default.extend-identifiers]` table with three entries. Each entry accepts one whole identifier of the generated bindings that the Slang headers misspell and that the bindings keep, because each generated name has to match the header. Section 5.10 of that document names the three identifiers and explains why entries for whole identifiers are the narrowest exclusion (SCS-14). No file is excluded from typos.

`typos.toml` sets no locale. typos cannot enforce Canadian spelling, because with its default settings it accepts both `colour` and `color`, so `scripts/check-prose` and its word list keep that job (Section 8 of the writing guide).

## 4. Recipes

### 4.1. Rules

The `justfile` stays one flat file with `[group]` attributes, a help comment on every recipe, and shell that is `shellcheck`-clean, as Hard Rule 8 of `AGENTS.md` requires. The `justfile` has the groups `checks`, `ci`, `git`, and `rust`. M0 added the group `rust` for the recipes that change files or run tests and are not checks. Step 2 of Section 15 of `docs/design/slang-compiler-sys.md` added the group `ci` for the recipes that only the CI workflow runs, which is `install-vulkan-sdk`. Every recipe that this document adds takes no parameters, except the custom checkers of Section 7, which take optional paths, and `install-vulkan-sdk`, which takes the SDK version and the expected checksum (Section 17.1).

TOOL-6. An agent MUST add each recipe of Section 4.2 with the name, the group, and the membership of `check` that the table gives, and MUST update the table in the same change when one of them has to differ, because agents, hooks, and CI call recipes by name, and a recipe that differs from this table breaks every caller that follows it.

TOOL-7. A recipe that runs a command of Section 4.8 of the coding standard MUST run that command as Section 4.8 gives it, adding only the option `--profile` with a nextest profile of Section 5, the arguments that Section 4.8 names for the recipe `coverage`, or the environment of Section 10, and an agent that has to change the command changes Section 4.8 in the same change, because two command lines for one check diverge, and an agent that runs the documented command then gets a result different from the recipe's. One departure is permitted: a recipe that selects named tests with a filter expression, and that has to fail when the filter selects no test, MAY add that filter to the nextest command and pass `--no-tests=fail` in place of `--no-tests=warn`, because with `--no-tests=warn` such a recipe passes without running any test when its test is renamed or is not compiled on the platform. On 2026-11-10, `check-bindings` is the only recipe that uses this departure.

TOOL-8. The recipe `check` MUST pass in the development container without a hardware graphics processing unit (GPU) and without a display, and MUST NOT run a recipe that needs either, because an agent runs `check` before every merge in the container, as the Validating Changes section of `AGENTS.md` requires, and CI runs it on a hosted runner that is not required to provide either.

TOOL-9. An agent MUST NOT add a recipe for a tool that the container image does not provide until an image rebuild adds the tool, because the recipe would fail `check` for every agent until then.

### 4.2. Recipe Table

This table lists every recipe, what it runs, whether `check` runs it, and when it arrives. The commands of Section 4.8 of the coding standard are named by their purpose, and Section 4.8 gives each command line.

| Recipe | Group | Runs | In `check` | Arrives |
| --- | --- | --- | --- | --- |
| `default` | none | `just --list --unsorted` | No | Exists |
| `check` | `checks` | The recipes that Section 4.3 lists | Not applicable | Exists |
| `check-prose` | `checks` | `scripts/check-prose`, extended to API documentation by Section 7.6 | Yes | Exists |
| `check-prose-self-test` | `checks` | The self-test of Section 7.1 for `scripts/check-prose` | Yes | Exists |
| `check-shell` | `checks` | `scripts/check-shell` | Yes | Exists |
| `check-commit-message` | `checks` | `scripts/check-commit-message` on one file, for the `commit-msg` hook | No | Exists |
| `check-commit-message-self-test` | `checks` | The self-test of the commit message checker | Yes | Exists |
| `hooks-install` | `checks` | `pre-commit install` | No | Exists |
| `merge` | `git` | `scripts/merge-branch` | No | Exists |
| `worktree-prune` | `git` | `scripts/worktree-prune` | No | Exists |
| `fmt` | `rust` | `cargo fmt` on the whole workspace, which applies the formatting that the formatting command of Section 4.8 checks, and `taplo fmt`, which formats every Tom's Obvious Minimal Language (TOML) file that `taplo.toml` does not exclude | No | Exists |
| `check-rust` | `checks` | The recipes that Section 4.3 lists | Yes | Exists |
| `check-fmt` | `checks` | The formatting command of Section 4.8 | Yes, through `check-rust` | Exists |
| `check-clippy` | `checks` | The Clippy command of Section 4.8 | Yes, through `check-rust` | Exists |
| `check-doc` | `checks` | The documentation command of Section 4.8 | Yes, through `check-rust` | Exists |
| `check-doctest` | `checks` | The doctest command of Section 4.8 | Yes, through `check-rust` | Exists |
| `test` | `rust` | The nextest command of Section 4.8 with the `default` profile | No, because `test-gpu` runs the same tests | Exists |
| `test-gpu` | `rust` | The nextest command of Section 4.8 with the `gpu` profile | Yes, through `check-rust` | Exists |
| `test-hw-gpu` | `rust` | The nextest command of Section 4.8 with the `hw-gpu` profile | No, because it needs a hardware GPU | Exists |
| `check-deny` | `checks` | The cargo-deny command of Section 4.8 | Yes, through `check-rust` | Exists |
| `check-rust-comments` | `checks` | The comment checker of Section 7.2 | Yes, through `check-rust` | Exists |
| `check-doc-headings` | `checks` | The heading checker of Section 7.3 | Yes, through `check-rust` | Exists |
| `check-doctest-attributes` | `checks` | The doctest attribute checker of Section 7.4 | Yes, through `check-rust` | Exists |
| `check-feature-docs` | `checks` | The feature documentation checker of Section 7.5 | Yes, through `check-rust` | Exists |
| `check-rust-comments-self-test`, `check-doc-headings-self-test`, `check-doctest-attributes-self-test`, `check-feature-docs-self-test` | `checks` | The self-test of Section 7.1 for each checker of Sections 7.2 to 7.5 | Yes | Exists |
| `check-workflow-pins` | `checks` | The workflow pin checker of Section 7.8 | Yes | Exists |
| `check-workflow-pins-self-test` | `checks` | The self-test of Section 7.1 for the workflow pin checker | Yes | Exists |
| `pin` | `rust` | Prints the pin of `crates/slang-compiler-sys/pin.rs` as `NAME=value` lines, and fails when a constant is missing (Section 8.1 of `docs/design/slang-compiler-sys.md`). It runs on bash 3.2, which has no associative arrays and was the bash that the recipe got on the hosted macOS runner on 2026-11-10, and it accepts lines that end in CRLF, as in the checkout on the hosted Windows runner on that date. The earlier form of the recipe failed on both runners for these two reasons. The root `.gitattributes` now keeps LF line endings in every checkout (Section 3.4), and the recipe still accepts CRLF, because a user's own git settings can still produce it | No | Exists |
| `install-vulkan-sdk` | `ci` | `scripts/install-vulkan-sdk` with the SDK version and the expected SHA-256 checksum, which downloads, verifies, and installs the Vulkan SDK on a GitHub Actions runner and exports it to the later steps of the job (Section 8.3). Only the CI workflow runs it, and it refuses to run outside a runner | No, because it runs only on a CI runner | Exists |
| `check-vulkan-sdk` | `checks` | `scripts/check-vulkan-sdk`, without arguments, which reads the build tag of the pin with `just pin` and checks that the SDK at `VULKAN_SDK` provides that build of Slang (Section 8.3) | No, every CI job runs it after it installs the SDK | Exists |
| `check-bindings` | `checks` | The nextest command of Section 4.8 with a filter expression that selects only the regeneration test of TEST-8 of the coding standard, and with `--no-tests=fail` in place of `--no-tests=warn`, the one departure that TOOL-7 permits, so that it fails when the filter selects no test, as on Windows and macOS (Section 7.6 of `docs/design/slang-compiler-sys.md`) | No, because `test-gpu` runs the same test on Linux | Exists |
| `generate-bindings` | `rust` | The binary of the binding generator, through `cargo run`, which overwrites `crates/slang-compiler-sys/src/bindings.rs` from the Slang headers of the SDK at `VULKAN_SDK` and needs libclang and rustfmt | No | Exists |
| `check-doc-without-slang` | `checks` | The documentation command of Section 4.8 with `DOCS_RS` set and `VULKAN_SDK` unset (Section 10) | Yes, through `check-rust` | M1 |
| `test-static` | `rust` | A build of the workspace on the static route and the smoke test on it, with the `SLANG_SOURCE_DIR` that the developer supplies, failing with a message that names the variable when it is absent (Section 9.3 of `docs/design/slang-compiler-sys.md`) | No, because it builds Slang from source | M1 |
| `check-typos` | `checks` | The typos command of Section 4.8 on the whole repository, with `typos.toml` (TOOL-30) | Yes | Exists |
| `check-toml` | `checks` | The taplo command of Section 4.8, with `taplo.toml` (TOOL-30) | Yes | Exists |
| `check-workflows` | `checks` | The actionlint command of Section 4.8, which checks every workflow | Yes | Exists |
| `check-unused-deps` | `checks` | The cargo machete command of Section 4.8 | Yes, through `check-rust` | Exists |
| `check-miri` | `checks` | The Miri command of Section 4.8, for the tests that the nextest profile `default-miri` selects | Yes, through `check-rust` | Exists |
| `check-features` | `checks` | The cargo hack command of Section 4.8 for the feature powerset, which includes each single feature | No, the CI job `ci-only` runs it | Exists |
| `check-msrv` | `checks` | The MSRV command of Section 4.8, which checks the workspace with `cargo hack check` and the toolchain that `rust-version` names | No, the CI job `ci-only` runs it, and its first local run downloads that toolchain (Section 17.3) | Exists |
| `check-semver` | `checks` | cargo semver-checks against the last published version of each published crate | No, the planned CI job `semver` runs it | M7, with the first release (Section 10) |
| `coverage` | `rust` | The cargo llvm-cov command of Section 4.8 with the arguments that Section 4.8 names for it, which write the LCOV report `target/lcov.info`. It prints no coverage summary, and it is not a gate | No, the CI job `coverage` runs it (Section 8.1) | Exists |

`check-miri` runs Miri through nextest, for the reason that Section 4.8 of the coding standard gives. Under Miri, nextest uses the profile named `default-miri`, which `.config/nextest.toml` defines since the first change of M1 (Section 5). Miri emulates some functions of libc and libm, but it cannot call an arbitrary native library such as Slang, so `check-miri` covers pure Rust logic, such as marshalling and layout code, and the unsafe code that TEST-7 of the coding standard tests against a fake implementation. A filter in a `[profile.default-miri]` table excludes the tests that call native code: every test whose path contains a module named `ffi`, `gpu`, or `hw_gpu`, selected by whole module path segments as in Section 5. Tests that call Slang or libclang sit in modules named `ffi` (SCS-33 of `docs/design/slang-compiler-sys.md`), and so do the tests that start a process, which Miri cannot do.

No Cargo feature selects the static build of Slang, which the environment variable `SLANG_SOURCE_DIR` selects (Section 9.1 of `docs/design/slang-compiler-sys.md`), so the checks that pass `--all-features` and the feature matrix of `check-features` never build Slang from source.

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
9. `check-workflow-pins-self-test`
10. `check-typos`
11. `check-toml`
12. `check-workflows`
13. `check-workflow-pins`
14. `check-rust`

`check-workflows` can run in `check` only while the repository has a workflow, because actionlint exits with status 3 when it finds none. `check` needs network access (Section 17.6).

`check-rust` runs `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs`, `check-fmt`, `check-unused-deps`, `check-deny`, `check-clippy`, `check-doc`, `check-doctest`, `test-gpu`, and `check-miri`, in that order, so that the checks that do not compile the workspace report first. From M1 it also runs `check-doc-without-slang`.

`check-rust` runs `test-gpu` and not `test`, because the `gpu` profile selects every test that the `default` profile selects (Section 5). `test-gpu` runs in the development container on lavapipe, so `check` meets TOOL-8. On Linux, `test-gpu` also runs the regeneration test of Section 7.6 of `docs/design/slang-compiler-sys.md`, so `check` needs `VULKAN_SDK`, libclang, and rustfmt there, which the development container provides and the Linux jobs of CI install (Section 8.1). A test in a `gpu` module that finds no adapter fails, as ARCH-23 of the architecture design requires, so `check` fails on a machine without any adapter.

## 5. Test Tiers in nextest

The file `.config/nextest.toml` defines three nextest profiles that implement the test tiers of Section 11 of the architecture design. The tier of a test is the name of a module in its path, as ARCH-22 of the architecture design defines. nextest reads that file, and a recipe selects a profile with the option `--profile` (TOOL-7).

TOOL-10. The nextest configuration MUST define the profiles `default`, `gpu`, and `hw-gpu` with the selection below, and each profile other than `default` MUST set its own `default-filter`, because a profile that selects a test of the wrong tier either fails on a runner without the hardware the test needs or leaves a test unrun everywhere. The selections are:

- `default`: every test whose path contains no module named `gpu` or `hw_gpu`.
- `gpu`: every test that `default` selects, and every test whose path contains a module named `gpu` and no module named `hw_gpu`.
- `hw-gpu`: every test.

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

This is the configuration that M0 adds as `.config/nextest.toml`:

```toml
[profile.default]
default-filter = "not test(/(^|::)(gpu|hw_gpu)::/)"
retries = 0

[profile.gpu]
default-filter = "not test(/(^|::)hw_gpu::/)"

[profile.hw-gpu]
default-filter = "all()"
```

The first change of M1 adds the profile `default-miri`, which nextest uses under Miri (Section 4.2), with the filter that SCS-33 of `docs/design/slang-compiler-sys.md` requires. It selects every test that `default` selects, except those whose path contains a module named `ffi`:

```toml
[profile.default-miri]
default-filter = "not test(/(^|::)(ffi|gpu|hw_gpu)::/)"
```

The regular expression matches `gpu` or `hw_gpu` only at the start of the test path or after `::`, and only when `::` follows it. The trailing `::` makes the match a whole module segment. A test function named `gpu` is the last segment of its path and has no `::` after it, and a module named `gpu_buffers` has `_buffers` after `gpu`, so neither matches. The filter of `default-miri` matches `ffi` in the same way. Section 7.5 of `docs/design/slang-compiler-sys.md` records how many tests `check-miri` runs and how many the profile skips. This configuration was verified with nextest 0.9.151, which Section 1.1 of the coding standard records, on a scratch workspace that contains every path of the table in this section, in unit-test modules and in an integration-test binary. A filter that the command line adds with `-E` is intersected with the default filter of the profile, so it narrows the selection of the profile and never widens it. `check-bindings` adds such a filter to the `default` profile to select the regeneration test alone (Section 4.2).

TOOL-11. A profile MUST NOT retry a failed test, because a retry turns an intermittent failure on an adapter into a pass, which hides the failure in the same way that ARCH-23 of the architecture design forbids a test to hide a missing adapter. Zero retries is the default of nextest, and the `default` profile sets `retries = 0` explicitly so that the configuration states this rule instead of relying on the default. The other profiles inherit the value.

TOOL-12. The recipes `test`, `test-gpu`, and `test-hw-gpu` MUST pass when their profile selects no test, because the skeleton crate of M0 has no tests and no `gpu` test exists before M3, so a run that fails on an empty selection would fail `check` until then. nextest 0.9.151 fails such a run with the message "error: no tests to run" and exit status 4. Every nextest command of Section 4.8 of the coding standard therefore passes `--no-tests=warn`, with which an empty run passes and nextest still prints that no test ran, and a recipe keeps that option, except in the one departure that TOOL-7 permits.

nextest does not run doctests, so `check-doctest` runs them, as Section 4.8 of the coding standard states.

## 6. Git Hooks

The hooks in `.pre-commit-config.yaml` call `just` recipes, as the existing local hooks do, because Hard Rule 8 of `AGENTS.md` makes every check a recipe.

TOOL-13. A hook of the `pre-commit` stage MUST run only checks that do not compile the workspace, because a commit hook that compiles the workspace makes every commit wait for a build, and Clippy, the tests, and the documentation build already run in `check` before every merge and in CI.

M0 adds these hooks of the `pre-commit` stage:

- `check-fmt`, which runs when a staged Rust file changes. Step 2 of Section 13 adds it.
- `check-typos`, which runs on every commit. Step 2 adds it.
- `check-toml`, which runs when a staged TOML file changes. Step 2 adds it.
- `check-workflows`, which runs when a staged file under `.github/` changes. Step 6 adds it.
- `check-workflow-pins`, which runs when a staged file under `.github/` changes. Step 6 adds it.

Each of these hooks runs its recipe on the whole repository, not on the staged files only. Since step 5, the existing `check-prose` hook runs on each staged Markdown file and each staged Rust file, and passes their paths to the recipe. The Claude Code hook in `.claude/settings.json` runs `scripts/claude-hook-prose` after every edit, and the script runs `check-prose` on the edited file when the file is inside the repository and its name ends in `.md` or `.rs`. It ignores every other file. When `check-prose` reports findings, the script prints them and exits with status 2, which Claude Code treats as a blocking error and reports with the findings. When `check-prose` fails for another reason, such as a Rust file that cannot be lexed because it is only partly edited (Section 7.1), the script prints the message and exits with status 1, which Claude Code treats as a non-blocking error. The `commit-msg` and `post-merge` hooks do not change.

The allow list of `.claude/settings.json` lets Claude Code run these commands without asking: `just --list`, `just check`, `check-rust` and each check that it runs, the custom checkers and their self-tests, `check-shell`, `check-commit-message` and its self-test, `check-typos`, `check-toml`, `check-workflows`, `check-features`, `check-bindings`, `check-vulkan-sdk`, `pin`, `test`, `test-gpu`, and `test-hw-gpu`, each as `just <recipe>`, and `shellcheck` and `git status`. Step 3 of Section 15 of `docs/design/slang-compiler-sys.md` added `check-bindings`, `check-vulkan-sdk`, `pin`, the three test recipes, and `check-commit-message` with its self-test. Claude Code asks before it runs any other recipe, such as `fmt`, `generate-bindings`, `merge`, `check-msrv`, or `coverage`.

## 7. Custom Checkers

Before M0, the writing guide marked several rules as judgement rules until a tool covered them, and actionlint does not check the pinning rule of Section 8.2. Each of these rules can be checked deterministically, so this section specifies a small custom checker for each. The "Checked by" statements of Section 7 of the writing guide, its Section 7.12, and Section 8.2 of this document now name these checkers.

### 7.1. Conventions

TOOL-14. Each custom checker MUST be a script under `scripts/` with a recipe of the same name in the `checks` group, and MUST follow the conventions that the usage text of `scripts/check-prose` states: one `path:line: message` line for each finding, exit status 0 without findings, 1 with findings, and 2 when a file cannot be read or the setup is broken, after checking every readable file, and usage text on `-h`. With no paths, the checker checks every relevant file in the git repository, tracked or not ignored. A checker that differs breaks the hooks and the recipes, which rely on these exit statuses to tell a finding from a broken setup.

A file that is not valid UTF-8 is a file that cannot be read, so a checker reports it on standard error and exits with status 2. The exception is a Markdown file given to `check-prose`, in which an invalid byte is a finding.

A Rust file that ends inside a string literal, a raw string literal, a block comment, or a character literal also cannot be read. For a checker that reads documentation comments, the same holds for a file that ends inside an attribute, because such a checker skips attributes to group the comments of one item. The checker prints `<checker>: cannot read <file>: unterminated <kind> at line N` on standard error, where N is the line of the opening delimiter, and exits with status 2. Unterminated input is a read error and not a silent end of the file, because a lexer that read the rest of the file as part of the literal would report nothing after it, and the check would pass while it checked nothing there. Other files given in the same run are still checked. While a file is only partly edited, unterminated input is normal, and Section 6 says how the Claude Code hook reports it.

Each checker prints every finding and every error message through the helper `emit` of `scripts/rust_lexer.py`, which escapes every character outside printable ASCII, both in file paths and in file content that a message repeats. A newline, a carriage return, and a tab print as `\n`, `\r`, and `\t`, every other character up to U+00FF prints as `\xNN`, and every character above it prints as `\uNNNN`, or as `\UNNNNNNNN` above U+FFFF. A checked file or a file name therefore cannot send a terminal control sequence to the terminal that shows the output (Section 16). A backslash prints unchanged, so a character that the helper escapes and the same escape written literally in a file print alike.

A checker reports a failed git call and a file that cannot be read in its own words, which do not depend on the version of git or on the operating system, because the self-tests pin the exact text of each error and would otherwise fail on another machine for an unrelated reason. A failed git call reads `not inside a git repository`, `git <subcommand> exited with status N`, or `cannot run git: <cause>`, and never repeats the message of git. A read error names its cause as `is a directory`, `no such file`, `not a directory`, or `permission denied`, and only a cause outside these four uses the description that the operating system gives. When `cargo metadata` fails, `check-feature-docs` still repeats the first error line of Cargo (Section 7.5).

The checkers are written in Python 3 and run with `python3 -I`, as `scripts/check-prose` and `scripts/check-commit-message` are, because the container provides Python 3 and the existing checkers set the pattern. The checkers that read Rust share the lexer `scripts/rust_lexer.py`, which splits Rust source into comments, literals, and other tokens, groups `///` and `//!` comments into the Markdown text that rustdoc reads, and finds fenced code blocks in that text. The lexer skips a shebang line at the start of a file, also when a byte order mark precedes it, and never reads it as a comment. A first line that starts with `#![` is an inner attribute and not a shebang line. `check-workflow-pins` uses only its helpers for finding and reading files.

TOOL-15. Each custom checker MUST have a self-test recipe named after it with the suffix `-self-test`, which `check` runs, and which runs the checker on at least one input that it accepts and one that it rejects for each rule of the checker and fails unless the checker gives exactly the expected result, because a checker that silently stops finding anything passes `check` while it enforces nothing. The inputs MUST be stored under a name that no checker or other tool reads as a real file, because a failing input stored under its real name would be reported by the checker itself.

The self-test recipes of the custom checkers, including `check-prose-self-test`, run `scripts/self-test-checker` with the name of the checker. Each case is a directory `scripts/fixtures/<checker>/<case>/`. Its input files carry the suffix `.fixture`, which the runner removes when it copies them into a new git repository in a temporary directory, so that the checkers, which select files by name, never read a fixture as a real file. The runner replaces placeholders while it copies, so that no committed file holds an unfinished-work marker, a carriage return, an escape character, a bell character, a byte order mark, an invalid byte, or a missing final newline. It runs the checker in that repository with the arguments in the file `arguments` of the case, in which it replaces the same placeholders, or with none. A case that holds a file named `no-git-repository` gets no git repository, and the checker runs in a temporary directory outside any repository, which tests the error for a missing repository. The case passes when the standard output equals the file `expected-output`, the standard error equals the file `expected-errors`, or is empty without that file, and the exit status equals the number in the file `expected-status`, or, without that file, 1 when `expected-output` is not empty and 0 when it is. The usage text of `scripts/self-test-checker` lists the placeholders. `check-commit-message-self-test` keeps its own inline form.

The checkers that read Rust were run over the source of the Rust standard library, 2,976 files, without a crash. On a scratch copy of the repository, each self-test failed when a rule of its checker was disabled.

### 7.2. Comment Checker

This table summarizes the comment checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-rust-comments`, `check-rust-comments` |
| Rule | Section 7.2 of the writing guide, and Hard Rule 5 of `AGENTS.md` |
| Reads | Every `.rs` file |
| Reports | Every comment other than a `///` comment, a `//!` comment, or a safety comment, and every comment of any kind that contains the word `TODO` or `FIXME` |

A safety comment is a run of `//` line comments on consecutive lines, with nothing else between them, whose first line starts with `// SAFETY:`, as the illustration in Section 7.6 of the writing guide shows. A line comment that starts with `SAFETY:` in another case or spacing is reported as a safety comment that does not start with `// SAFETY:`. The checker reports every block comment, including `/** */` and `/*! */`, and every line comment that starts with four slashes. rustdoc and `missing_docs` treat `/** */` and `/*! */` as documentation, but the checker reports them because the writing guide defines API documentation as `///` and `//!` comments only (Section 2.2 of the writing guide). `////` and `/*** */` are not documentation comments, so they are other comments, which Section 7.2 of the writing guide forbids. Whether a safety comment sits directly above an `unsafe` block is checked by the safety comment lints of Section 4.5 of the coding standard, not by this checker.

TOOL-16. The comment checker MUST find comments by reading Rust tokens, so that the characters `//` and `/*` inside a string literal, a raw string literal, or a character literal are not reported, because a checker that reports text inside literals produces findings that no fix can remove.

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

The checker does not report line 4, because the slashes there are inside a string literal. Generated files are checked like any other file. The generated bindings of M1 get no exemption, because the binding generator emits no comments other than one module-level `//!` line (Section 5.10 of `docs/design/slang-compiler-sys.md`).

### 7.3. Heading Checker

This table summarizes the heading checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-doc-headings`, `check-doc-headings` |
| Rule | Section 7.3 of the writing guide |
| Reads | The `///` comments of every `.rs` file. Crate-level and module-level documentation, written with `//!`, is not read, because Section 7.4 of the writing guide gives it its own headings |
| Reports | A heading of any level that is not one of the level-one standard headings of Section 7.3 of the writing guide, including a standard heading with the wrong case or level, a standard heading out of the order that section gives, and a standard heading that appears twice in one item's documentation |

The checker reads both ATX headings, which start with `#`, and setext headings, which are underlined with `=` or `-`. The `///` comments that precede one item, with only attributes and other comments between them, are one item's documentation. The checker ignores lines inside fenced code blocks, because a line that starts with `# ` in a Rust code block is a hidden line of a doctest, not a heading. It does not read documentation written as a `#[doc = ...]` attribute or a `/** */` block comment, which the comment checker of Section 7.2 rejects. Section 7.3 of the writing guide forbids every other heading in the documentation of an item, which is the rule that the checker enforces.

### 7.4. Doctest Attribute Checker

This table summarizes the doctest attribute checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-doctest-attributes`, `check-doctest-attributes` |
| Rule | Section 7.5 of the writing guide |
| Reads | The fenced code blocks in the `///` and `//!` comments of every `.rs` file, and in every file named `README.md`, which a crate includes as doctests (Section 7.10 of the writing guide). A path given on the command line that is not named `README.md` is read as Rust |
| Reports | A code block whose attributes include `ignore` or an attribute that starts with `ignore-`, such as `ignore-windows` |

The checker reports the attributes that start with `ignore-` as well, because Section 7.5 of the writing guide forbids `ignore` and these are variants of it that ignore the example on some targets. The checker splits the info string of a code block into attributes the way rustdoc does: at commas, spaces, and tabs, with a double-quoted attribute read without its quotes, and with the text inside braces, such as `{.ignore}`, read as a class or a key and not as an attribute. Whether a `no_run` example states its reason stays a judgement rule.

### 7.5. Feature Documentation Checker

This table summarizes the feature documentation checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-feature-docs`, `check-feature-docs` |
| Rule | Section 7.8 of the writing guide |
| Reads | For each workspace member with a library target, its features as `cargo metadata --no-deps --offline` reports them, and the `# Features` section of the `//!` crate-level documentation at the top of its library root, before the first item. `cargo metadata` also reports the feature that an optional dependency defines when no feature refers to that dependency with `dep:` |
| Reports | A missing `# Features` section in any library, including a library that has no features, a feature, other than `default`, that the section does not name in a code span, and a code span in the section that looks like a feature name, meaning lowercase letters, digits, `_`, `+`, and `-`, but names no feature of the package |

The checker requires the section in a library that has no features, because item 4 of Section 7.4 of the writing guide requires it in every crate. It does not read crate-level documentation written as a `#![doc = ...]` attribute, including `#![doc = include_str!(...)]`. Unlike the other checkers, it takes the paths of Cargo manifests, and with no paths it checks the workspace of the git repository that contains the current directory. It prints paths relative to the current directory. When `cargo metadata` fails, it exits with status 2.

Whether the section says that each feature is on or off by default, and what it enables, stays a judgement rule, because the wording is free.

### 7.6. Prose in API Documentation

`scripts/check-prose` reads the text of the `///` and `//!` comments of `.rs` files and applies to it the rules that it applies to Markdown, which enforces Section 7.11 of the writing guide for the rules of its Section 8 that the script checks. It reads each run of comments that documents one item as one Markdown document, and it reports each finding at its line in the Rust file. Code spans and fenced code blocks inside the comments stay exempt, as they are in Markdown. It does not read documentation written as a `#[doc = ...]` attribute or a `/** */` block comment. A Rust file that is not valid UTF-8 cannot be read (Section 7.1). With no paths, `check-prose` checks every Markdown file and every Rust file. The recipe keeps its name and parameters, and its self-test is `check-prose-self-test`.

### 7.7. Moving a Rule to Enforced

TOOL-17. The change that lands a custom checker MUST, in the same change, rewrite the "Checked by" statement of the section of the writing guide that the checker enforces and its row in Section 7.12 of the writing guide to name the checker, add the checker to the table of tools in Section 4.8 of the coding standard, and update the Validating Changes section of `AGENTS.md`, as Section 3.5 of the writing guide and Section 19 of the coding standard require for a rule that a tool can check, because a document that still calls an enforced rule a judgement rule sends a reviewer to check by hand what a tool checks, which Hard Rule 4 of `AGENTS.md` forbids. The change that extends `scripts/check-prose` also updates Section 8 of the writing guide, which describes the files that the script and its hook check.

### 7.8. Workflow Pin Checker

This table summarizes the workflow pin checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-workflow-pins`, `check-workflow-pins` |
| Rule | TOOL-20, the `pull_request_target` part of TOOL-22, and the top-level part of TOOL-19 (Section 8.2) |
| Reads | Every `.yml` and `.yaml` file under `.github/workflows/`, and every `action.yml` and `action.yaml` file under `.github/actions/`. A file named `action.yml` or `action.yaml` is read as a composite action, and any other file as a workflow |
| Reports | In every workflow and composite action, a `uses:` whose reference is not a full 40-character lowercase hexadecimal commit SHA. In every workflow, also the `pull_request_target` event under the top-level `on:` key, a workflow without a top-level `permissions:` key or with an empty one, and a top-level permission other than `read` or `none`, such as `write-all` or `contents: write` |

A local action, whose reference starts with `./`, needs no commit SHA. A `docker://` reference passes only when it is pinned by an `@sha256:` digest. The checker does not check the permissions of a job, so whether a job that requests more than read access needs it stays a judgement rule, and so does the part of TOOL-22 about secrets and publishing.

The checker reads each file line by line and does not use a YAML parser, and its usage text states the limits that follow from this. It reads keys only in block style, one per line, or as the value of `permissions:` in a one-line flow mapping such as `{ contents: read }`. It skips the text of a block scalar, such as the script of `run: |`. It does not read a `uses:` inside a flow mapping, or a scalar continued over several lines without a block indicator. It reads the text after a `#` that starts a line or follows a space as a comment, except inside quotes. A workflow written in a form that the checker does not read therefore depends on review.

This illustration shows a workflow step and the finding that the checker prints for it, assuming the step is on line 12 of `.github/workflows/ci.yml`:

```yaml
      - uses: actions/checkout@v4
```

```text
.github/workflows/ci.yml:12: uses: 'actions/checkout@v4' is not pinned to a full 40-character commit SHA
```

## 8. Continuous Integration

### 8.1. Workflow and Jobs

One workflow, `.github/workflows/ci.yml`, runs on every pull request and on every push to `main`. Its `concurrency` group is the workflow and the git reference of the pull request for a pull request, and the run itself for a push, with `cancel-in-progress: true`, so a new push to a pull request cancels the superseded run of that pull request, and no run for `main` is cancelled. Every job has a timeout of 60 minutes, and every `run` step uses Bash, also on Windows. The checkout step sets `persist-credentials: false`, so the token is not left in the git configuration of the runner for later steps.

TOOL-18. A workflow MUST run every check through a `just` recipe and MUST NOT run a check command directly, because CI then runs the same commands as the development container, so a check that fails in one place has the same command to reproduce it in the other. Steps that only prepare a runner, such as installing a tool, are not checks.

This table lists the jobs, with the name that GitHub shows for each and the hosted runner label. `check-workflows` and `check-workflow-pins` check the workflow.

| Job | Name | Runner | Runs | Arrives |
| --- | --- | --- | --- | --- |
| `check` | Run every check on Linux | `ubuntu-latest`, Linux x86_64, with lavapipe as the software adapter from M3, when the first `gpu` test exists | `just check` | Exists |
| `test` | Test on `windows-latest`, Test on `macos-latest` | A matrix of `windows-latest`, Windows x86_64, with the Windows Advanced Rasterization Platform (WARP) as the planned software adapter, and `macos-latest`, macOS arm64, with the hosted Metal device as the planned adapter, with `fail-fast: false` | `just check-clippy`, `just check-doctest`, and `just test-gpu` | Exists |
| `ci-only` | Run `check-features` on Linux, Run `check-msrv` on Linux | `ubuntu-latest`, Linux x86_64, as a matrix of the two recipes, with `fail-fast: false` | `just check-features` and `just check-msrv` | Exists |
| `semver` | Not yet named | Linux x86_64 | `just check-semver` | M7, with the first release (Section 10) |
| `coverage` | Measure test coverage on Linux | `ubuntu-latest`, Linux x86_64 | `just coverage`, then the upload of `target/lcov.info` as the artifact `coverage-lcov` | Exists |

The `test` job runs Clippy and the doctests on Windows and macOS as well as the tests, because a lint exception has to be fulfilled on every target that is checked (Section 4.4 of the coding standard), and code under `#[cfg]` differs between platforms. It compiles the checked-in bindings of `slang-compiler-sys` there and never regenerates them. Whether every platform can run the GPU tier while SPIR-V is the only shader target is open (Section 17, Q-21 of the architecture design). If the answer is that a platform cannot, its entry in the matrix runs `just test` instead of `just test-gpu` until the shader target is configurable. That lavapipe, WARP, and hosted Metal work as adapters on hosted runners is an assumption of the architecture design (Section 16, A-17 and A-18 of the architecture design). No `gpu` test exists yet, so `just test-gpu` has not used an adapter on any runner, and the `check` job does not install lavapipe yet.

No job runs the `hw_gpu` tests, which run only on a developer's hardware GPU through `just test-hw-gpu`. No job builds the static route, because it compiles several hundred C++ files, and a recipe verifies it locally instead (SCS-36 of `docs/design/slang-compiler-sys.md`).

Every job runs its steps in the same order. It checks out the repository, installs the Rust toolchains, installs its tools through the installer action, reads the pin, installs the Vulkan SDK with `just install-vulkan-sdk`, and checks the SDK with `just check-vulkan-sdk` (Section 8.3). Then it runs its own steps. The `check` job installs actionlint and libclang and runs `just check`. The `test` job runs `just check-clippy`, `just check-doctest`, and `just test-gpu`. The `ci-only` job runs the recipe of its matrix entry. The `coverage` job installs libclang, runs `just coverage`, and uploads the report. The workflow defines the steps that check out the repository, read the pin, install and check the SDK, and install libclang once, with YAML anchors, and reuses them in the other jobs, so each of these steps is the same in every job that runs it.

The `coverage` job runs the tests of the `gpu` profile under cargo llvm-cov and keeps the report. The recipe prints no coverage summary in the log of the job. With the LCOV options, cargo llvm-cov ends its output with the line `Finished report saved to target/lcov.info`, as observed on 2026-11-10, so the uploaded report is the only result of the job. The action `actions/upload-artifact`, pinned by commit SHA (TOOL-20), uploads `target/lcov.info` as the artifact `coverage-lcov`, which GitHub keeps for 14 days. The step sets `if-no-files-found: error`, so the job fails when the recipe wrote no report. The job sets no coverage threshold, so the report informs a reviewer and does not gate a merge. The job arrived in step 2 of Section 15 of `docs/design/slang-compiler-sys.md`, because a crate with code and tests then existed to measure.

CI installs the tools on each runner, because no container image is published for CI to run in. Each job installs only the tools that its recipes need:

- The Rust toolchains, through rustup, which the hosted runners provide. Each job installs the stable toolchain with the minimal profile and the components that it needs, which are rustfmt and clippy in the `check` job, clippy in the `test` job, none in the `ci-only` job, and rustfmt and llvm-tools in the `coverage` job. The tests of the binding generator start rustfmt, and cargo llvm-cov needs llvm-tools. The `check` job also installs the nightly toolchain with the components miri and rust-src, and lets rustup choose an older nightly when the latest one lacks a component.
- `just` in every job, cargo-nextest in the `check`, `test`, and `coverage` jobs, shellcheck, typos, taplo, cargo-machete, and cargo-deny in the `check` job, cargo-hack in the `ci-only` job, and cargo-llvm-cov in the `coverage` job, through the action `taiki-e/install-action`, pinned by commit SHA. The action installs prebuilt binaries and verifies the SHA-256 checksum of each one against the manifest of versions and checksums that it carries. The workflow sets `fallback: none`, so the action fails instead of installing a tool by a route that its manifest does not cover.
- actionlint, in the `check` job, through an inline step. It reads the latest release from GitHub, downloads the release archive for Linux x86_64 and the checksums file of the same release, verifies the archive with `sha256sum --check --strict`, and fails on a mismatch before it extracts the archive.
- libclang, in the `check` and `coverage` jobs, which the tests of the binding generator load, including the regeneration test of the checked-in bindings (Sections 7.5 and 7.6 of `docs/design/slang-compiler-sys.md`). The step runs `sudo apt-get update` and `sudo apt-get install --yes --no-install-recommends libclang-dev` before `just check` or `just coverage`. It meets TOOL-21 because apt verifies the signature of the distribution's package index and the SHA-256 checksum of each package against that index, and refuses a package that fails either check. The package is the libclang of the runner's distribution, and its version can differ from the one in the development container (Section 7.6 of `docs/design/slang-compiler-sys.md`). The `test` jobs do not install libclang, because the tests that load it compile only for Linux.
- The pinned Vulkan SDK, in every job, through `just install-vulkan-sdk` (Section 8.3).

Each route except the Vulkan SDK installs the latest version that it provides, which matches the policy of the container image, in which no tool version is pinned. The Vulkan SDK is the version of the pin (Section 8.3). A tool in CI can therefore have a different version from the same tool in the development container, and a check can pass in one and fail in the other until both have the same version. The workflow caches nothing, so no file is restored from a cache.

On its first run on GitHub, on a draft pull request, every job passed: "Run every check on Linux" in 53 seconds, "Test on windows-latest" in 37 seconds, "Test on macos-latest" in 23 seconds, and "Run check-features on Linux" and "Run check-msrv on Linux". These durations are observations of that run, not limits. So `just check` passes on a Linux runner with the tools that the workflow installs, and Clippy, the doctests, and the nextest recipe pass on Windows and macOS. The first run with the binding generator also passed on all five jobs, and the `check` job ran the tests of the generator with the libclang that it installs.

On 2026-11-10, with the Vulkan SDK installed in every job and the `coverage` job added, all six jobs passed on a draft pull request. On that run, "Run every check on Linux" took 4 minutes 28 seconds, "Measure test coverage on Linux" 3 minutes 39 seconds, "Test on macos-latest" 1 minute 44 seconds, "Test on windows-latest" 1 minute 9 seconds, "Run check-msrv on Linux" 31 seconds, and "Run check-features on Linux" 24 seconds. These durations are observations of one run, not limits, and Section 8.3 records how much of them the SDK takes.

Since step 3 of Section 15 of `docs/design/slang-compiler-sys.md`, the `check` and `coverage` jobs also run the regeneration test of the checked-in bindings, through `test-gpu` and `coverage`. The test needs the headers of the installed SDK, libclang, and rustfmt, and both jobs install all three before they run the tests. On 2026-11-10, all six jobs passed on a draft pull request with this change. The checked-in bindings compiled and passed Clippy on Windows and macOS, where the tests for Linux only are not compiled. The regeneration test passed in the `check` and `coverage` jobs. Section 7.5 of `docs/design/slang-compiler-sys.md` records the test counts, and its Section 7.6 records the libclang observation. On that run, "Run every check on Linux" took 1 minute 43 seconds, "Measure test coverage on Linux" 57 seconds, "Test on macos-latest" 1 minute 37 seconds, "Test on windows-latest" 1 minute 21 seconds, "Run check-msrv on Linux" 43 seconds, and "Run check-features on Linux" 39 seconds. These durations are observations of one run, not limits.

### 8.2. Workflow Security

TOOL-19. A workflow MUST grant its token read access to the repository contents only, at the level of the workflow, and a job that needs more MUST request the extra permission on that job alone, because a compromised step can use every permission that the token holds. `check-workflow-pins` reports a workflow without top-level permissions and a top-level permission other than `read` or `none` (Section 7.8). Whether a job that requests more needs it is a judgement rule.

TOOL-20. A workflow MUST reference every action, including the actions that GitHub publishes, by a full commit SHA and never by a tag or a branch, because the owner of an action can move a tag or a branch to different code, which then runs in every later job without any change to this repository. actionlint does not check this, and `check-workflow-pins` enforces it (Section 7.8).

TOOL-21. A workflow step that downloads a file MUST verify it against a SHA-256 checksum that the upstream publishes and MUST stop the job before the file is used when the checksum differs, as Hard Rule 7 of `AGENTS.md` requires. A step that installs tools through an action MUST use an action that verifies each download in this way, pinned by commit SHA (TOOL-20), which also pins the manifest of versions and checksums that the action carries, and MUST configure it to fail instead of installing by a route that it does not verify. A file restored from a cache MUST be verified in the same way as a fresh download. These requirements hold because a tampered file that is not verified runs with the permissions of the job, and a cache can hold a file that an earlier, compromised run stored. When the upstream publishes no checksum, the step SHOULD record in the workflow the checksum of the file that the agent verified when it wrote the step, because a later change in the served file is then detected, even though the first file is trusted on first use.

A checksum that comes from the same host as the file, such as the checksums file of an actionlint release, proves that the download is intact but not that it comes from the publisher, because an attacker who controls the host can replace both. A checksum in the pinned manifest of the installer action comes from the commit of the action that TOOL-20 pins instead. Section 16 states the residual risk. Whether each step meets TOOL-21 is a judgement rule, because no tool can tell from a workflow which step downloads a file.

The action `actions/upload-artifact`, which the `coverage` job uses (Section 8.1), is pinned by commit SHA like every other action, and `check-workflow-pins` checks it. The step that installs the Vulkan SDK meets TOOL-21 with checksums that the workflow records (Section 8.3). The installers of the SDK for macOS and Windows also fetch metadata from LunarG during the installation, which that checksum does not cover, and Section 16 states that residual risk.

TOOL-22. A workflow MUST NOT use the `pull_request_target` event, read a secret, or publish a crate, because the workflows need no secret to check code, and a workflow that holds a secret while it runs the code of a pull request exposes that secret to the author of the pull request. `check-workflow-pins` reports the `pull_request_target` event (Section 7.8). The rest of TOOL-22 is a judgement rule.

### 8.3. The Vulkan SDK on CI Runners

Every job of the workflow installs the pinned Vulkan software development kit (SDK) and checks it before any step that compiles the workspace, because `slang-compiler-sys` links Slang from the step of M1 that adds its build script, and every job compiles that crate. Step 2 of Section 15 of `docs/design/slang-compiler-sys.md` added the installation, and SCS-35 of that document states what it achieves. This section answers TQ-1, which is Q-27 of the architecture design.

TOOL-23. The step that installs the Vulkan SDK on a runner MUST read the SDK version from the one place that records the pin (ARCH-9 of the architecture design) and MUST NOT restate the version, MUST verify the download as TOOL-21 requires, MUST set `VULKAN_SDK` for the later steps, and MUST make the Slang shared library findable at run time on the runner in the way that Section 4.4 of the architecture design describes for the platform, because a second copy of the pin diverges from the first, an unverified installer runs with the job's permissions, and a test binary that cannot find the shared library fails before any test runs.

The step "Read the pin of slang-compiler-sys" reads the version. It runs `just pin`, captures the output, and appends it to the file that `GITHUB_ENV` names, so the later steps of the job have `VULKAN_SDK_VERSION`, `SLANG_BUILD_TAG`, and `SLANG_COMMIT` in their environment. It appends only after the recipe succeeds, so a failing recipe appends nothing and fails the step. The workflow states neither the version nor the build tag, as TOOL-23 and SCS-23 of `docs/design/slang-compiler-sys.md` require.

The step "Install the Vulkan SDK" then runs this command, in which `SDK_SHA256` is the checksum that the workflow records for the platform of the runner:

```sh
just install-vulkan-sdk "${VULKAN_SDK_VERSION}" "${SDK_SHA256}"
```

The recipe runs `scripts/install-vulkan-sdk`, which does these things in this order:

1. It exits with status 2 on a number of arguments other than two, and when `RUNNER_TEMP`, `GITHUB_ENV`, or `GITHUB_PATH` is unset or empty, which GitHub Actions sets on every runner, so it does not run outside CI. It also exits with status 2 on a version that is not four numbers separated by full stops, on a checksum that is not 64 lowercase hexadecimal characters, and on a platform other than Linux x86_64, macOS, and Windows x86_64.
2. It downloads `https://sdk.lunarg.com/sdk/download/<version>/<platform>/<file>`, in which `<platform>` is `linux`, `mac`, or `windows`, with curl. It restricts curl to HTTPS with Transport Layer Security (TLS) 1.2 or later, makes it fail on an HTTP error, and lets it retry three times. When the download fails, the script stops with the exit status of curl.
3. It computes the SHA-256 checksum of the file from standard input, with `sha256sum`, or with `shasum -a 256` where `sha256sum` is absent. When the checksum differs from the expected one, it prints both values and exits with status 1, before it unpacks or runs anything. It hashes standard input because `sha256sum` on the Windows runner prefixes its output with a backslash when the file name contains a backslash.
4. It installs the SDK under `$RUNNER_TEMP/VulkanSDK/<version>`, which this section calls `<root>`, as the first table of this section describes for each platform. When unpacking or running the installer fails, the script stops with the exit status of the failing command. On macOS, it exits with status 1 when the archive does not hold exactly one installer application.
5. It appends `VULKAN_SDK` to the file that `GITHUB_ENV` names on every platform, and also `LD_LIBRARY_PATH` on Linux and `DYLD_LIBRARY_PATH` on macOS, each with the SDK's `lib` directory first. Windows gets no library path variable. It appends the SDK's `bin` directory to the file that `GITHUB_PATH` names on Linux and macOS, and its `Bin` directory on Windows. Every later step of the job has these values.

This table gives the downloaded file, the installation, and the values that the script exports on each platform:

| Platform | Downloaded file | Installation | `VULKAN_SDK` | Library path |
| --- | --- | --- | --- | --- |
| Linux x86_64 | `vulkansdk-linux-x86_64-<version>.tar.xz` | Unpacks the archive into the root with `tar -xJf` and `--strip-components=1` | `<root>/x86_64` | `LD_LIBRARY_PATH` starts with `$VULKAN_SDK/lib`, and `PATH` gets `$VULKAN_SDK/bin` |
| macOS | `vulkansdk-macos-<version>.zip` | Unzips the archive and runs the one installer application in it with `--root <root> --accept-licenses --default-answer --confirm-command install`, without `sudo` | `<root>/macOS` | `DYLD_LIBRARY_PATH` starts with `$VULKAN_SDK/lib`, and `PATH` gets `$VULKAN_SDK/bin` |
| Windows x86_64 | `vulkansdk-windows-X64-<version>.exe` | Runs the installer with the same options and `copy_only=1`, so that it writes no registry entries, layers, shortcuts, or system `PATH` | The root, as a Windows path | `PATH` gets `$VULKAN_SDK\Bin`, which holds both the programs and `slang-compiler.dll` |

The variable that each platform's dynamic loader reads gets the SDK's library directory, and on Windows, `PATH` gets the directory of the DLL, which meets the last part of TOOL-23. The job does not run the setup script of the SDK. On Linux and macOS, the install step sets the loader variables that the setup script would set, as Section 4.4 of the architecture design describes. On 2026-11-10, System Integrity Protection was disabled on the hosted macOS runner, and `DYLD_LIBRARY_PATH` reached every process that a step started. This does not show that the variable reaches a test binary on a developer's Mac, which SA-7 of `docs/design/slang-compiler-sys.md` tracks.

The installers for macOS and Windows are installers of the Qt Installer Framework. On the hosted runners on 2026-11-10, they ran without a graphical session, accepted the licence through `--accept-licenses`, selected only the core component, and returned when the installation was done. The macOS installer needed no `sudo` for a directory that the user owns. Both installers fetched metadata from LunarG's remote repository during the installation, and no supported option to prevent this was found, which Section 16 records as a residual risk.

The step "Check the Slang of the Vulkan SDK" then runs `just check-vulkan-sdk`. The recipe runs `scripts/check-vulkan-sdk`, which reads the build tag of the pin with `just pin` and checks these things. `$VULKAN_SDK/include/slang` holds `slang.h` and `slang-tag-version.h`, and `slang-tag-version.h` has the exact line `#define SLANG_TAG_VERSION "<tag>"`. The Slang library of the platform exists, which is `lib/libslang-compiler.so` on Linux, `lib/libslang-compiler.dylib` on macOS, and `lib/slang-compiler.lib` and `bin/slang-compiler.dll` on Windows, whose file system resolves these names to the directories `Lib` and `Bin`. `$VULKAN_SDK/bin/slangc -version` prints output that contains the tag. The script also lists the files that it finds. It removes carriage returns from `slang-tag-version.h` and from the output of `slangc` before it compares them, so it accepts CRLF line endings. It exits with status 1 when a check fails, including when `VULKAN_SDK` names a path that is not a directory, for which it prints `check-vulkan-sdk: VULKAN_SDK names <path>, which is not a directory` and checks nothing else. It exits with status 2 when it gets an argument, when `VULKAN_SDK` is unset or empty, or when `just pin` fails or prints no `SLANG_BUILD_TAG`. It needs no CI runner, so the same command checks the SDK of the development container.

LunarG publishes a SHA-256 checksum of each SDK download, as 64 hexadecimal characters at a documented address of the form `https://sdk.lunarg.com/sdk/sha/<version>/<platform>/<file>.txt`, which is served from the same host as the download. A checksum fetched from that host at run time detects a corrupted download, but not a download that an attacker at the host replaced together with its checksum. The workflow therefore records the expected values and does not fetch them at run time, which the pinned version of the SDK allows, unlike the latest-version installs of Section 8.1.

The workflow records the three checksums in its top-level `env` as `VULKAN_SDK_SHA256_LINUX`, `VULKAN_SDK_SHA256_MACOS`, and `VULKAN_SDK_SHA256_WINDOWS`, with a comment that ties them to the version that `crates/slang-compiler-sys/pin.rs` pins, and the install step selects the one for `runner.os`. On 2026-11-10, each recorded value was the one that LunarG publishes for the file, and the hash that the runner computed of its download matched it. The checksums are not part of the pin, so they stay in the workflow, which is their only user, and they change in the same change as the pin (Section 8.1 of `docs/design/slang-compiler-sys.md`). LunarG also publishes an installer for Windows on ARM64, which the workflow does not use, because the `windows-latest` runner is x86_64.

The workflow does not cache the SDK. A cache of the installed tree would save at most the installation time in the table below, which was longest on macOS at 43 to 60 seconds, restoring a tree of up to 2.6 GB takes time of its own, and TOOL-21 requires a restored tree to be verified like a fresh download, which would need a recorded manifest of the hash of every file. A cache of the downloaded file alone is easy to verify against the recorded checksum, but it saves only the download, which took at most 17 seconds on every platform on 2026-11-10.

This table records what the step measured on the hosted runners over the runs of the draft pull request of step 2, on 2026-11-10. These are observations, and a later run can take longer.

| Platform | Downloaded size | Download time | Unpacking or installation time | Installed size |
| --- | --- | --- | --- | --- |
| Linux x86_64 | 366,690,104 bytes | 5 to 17 seconds | 7 seconds | 1.9 GB |
| macOS | 397,584,552 bytes | 6 to 7 seconds | 43 to 60 seconds, including the unzip | 2.6 GB |
| Windows x86_64 | 303,066,832 bytes | 4 to 5 seconds | 12 to 13 seconds | 1.8 GB |

On all three platforms, the installed SDK passed `check-vulkan-sdk` with the same Slang build tag. Sections 3.6, 3.7, and 12 of `docs/design/slang-compiler-sys.md` record what each SDK ships.

## 9. Dependency Policy

The file `deny.toml` configures cargo-deny, which Section 4.8 of the coding standard lists with what it owns. Section 4.8 also requires the licence allow-list to include `GPL-3.0-or-later` itself.

TOOL-24. The licence allow-list of `deny.toml` MUST start with `GPL-3.0-or-later` alone, and an agent MUST add any other licence only in the change that adds the first dependency under it, after confirming that the licence is compatible with GPL-3.0-or-later, because a licence that no dependency uses widens what a later dependency can bring in without any check failing.

TOOL-25. Every path dependency between workspace members MUST carry a version, and `deny.toml` MUST set `bans.allow-wildcard-paths = false`, because Cargo refuses to package or publish a crate whose path dependency has no version, so without the check a missing version surfaces only at the release. The value `true` is no alternative (Section 4.8 of the coding standard).

TOOL-26. `deny.toml` MUST make the check fail on every security advisory that applies to a dependency in `Cargo.lock`, on every duplicate version of a crate, and on every crate from a source other than crates.io, and an agent MAY record an exception only for one advisory or one duplicate, with a reason in `deny.toml`, because Section 4.5 of the coding standard leaves the policy on duplicate versions to cargo-deny, and a policy that only warns reports nothing that the check enforces. A duplicate that no change to the workspace can remove is the expected case for an exception. An exception for a duplicate version has the form `skip = [{ crate = "name@version", reason = "..." }]` in the `[bans]` table.

TOOL-31. `deny.toml` MUST set `unused-allowed-license = "deny"` in its `[licenses]` table and `unused-ignored-advisory = "deny"` in its `[advisories]` table, because without them cargo-deny only warns about an unused entry, and the lists that TOOL-24 and TOOL-26 keep minimal then grow without any check failing.

This is the `deny.toml` that M0 adds:

```toml
[graph]
all-features = true

[advisories]
ignore = []
unused-ignored-advisory = "deny"

[licenses]
allow = ["GPL-3.0-or-later"]
confidence-threshold = 0.8
unused-allowed-license = "deny"

[bans]
multiple-versions = "deny"
wildcards = "deny"
allow-wildcard-paths = false

[sources]
unknown-registry = "deny"
unknown-git = "deny"
allow-registry = ["https://github.com/rust-lang/crates.io-index"]
allow-git = []
```

With cargo-deny 0.20.2 on a scratch workspace, this file passes, and it fails as required on a duplicate version, a git source, a path dependency without a version, an advisory, and a licence that is not in the allow-list.

The first change of M1 added the first dependency, the crate `clang` of the binding generator, and with it the two licences that its dependency graph needs, so the allow-list is now:

```toml
allow = ["GPL-3.0-or-later", "Apache-2.0", "ISC"]
```

`Apache-2.0` is there for `clang` and `clang-sys`, and for `glob`, `libc`, `cfg-if`, and `windows-link`, whose licence `MIT OR Apache-2.0` it satisfies, so MIT is not needed. `ISC` is there for `libloading`. Both are compatible with GPL-3.0-or-later, as TOOL-24 requires, and Section 14 of `docs/design/slang-compiler-sys.md` records the licence of each crate.

`check-deny` fetches the advisory database on every run, so it needs network access (Section 17.6).

## 10. Publishing Readiness

The architecture design owns the publishing policy in its Section 13 and the go-ahead rule ARCH-24. This section defines how the repository is kept ready to publish from M0.

TOOL-27. Every published crate MUST carry the metadata fields that the row of `clippy::cargo_common_metadata` in Section 4.5 of the coding standard lists, inheriting those that Section 3.3 places in `[workspace.package]` and setting `description`, `keywords`, and `categories` itself, and every member that is never published MUST set `publish = false`, because Clippy reports a missing field on every member that does not set `publish = false`, so the check fails from M0 onward until the metadata is complete.

Cargo supports this layout. A `readme` that a member inherits from the workspace as `readme = "README.md"` resolves to the root file, and Cargo copies that file into the package. Cargo refuses to package a crate whose path dependency has no version (TOOL-25). Packaging one crate that depends on a member that is not published yet fails, while `cargo package --workspace` succeeds. Cargo does not include the root `LICENSE` in a packaged crate when the manifest sets only the `license` field, so how each published crate carries the licence text is open (Section 15, TQ-6).

The documentation build on docs.rs has to work without Slang, as ARCH-8 of the architecture design requires. From M1, the recipe `check-doc-without-slang` runs the documentation command of Section 4.8 of the coding standard with `DOCS_RS` set and `VULKAN_SDK` unset. It reproduces only the absence of Slang, not the rest of the docs.rs environment, which M7 verifies on docs.rs itself. Each crate's design document defines any docs.rs settings that the crate's manifest needs.

The recipe `check-semver` compares each published crate with its last version on crates.io. It arrives in M7 with the first release, together with the CI job `semver`, because only then does each published crate have a published version to compare with. cargo-semver-checks 0.51.0 fails on a crate that has no published version, with the message "not found in registry" and exit status 101, and has no option to skip such a crate. Its option `--baseline-rev` compares with a git revision instead, but it fails on a second run when the target directory is inside the workspace, and before the first release a comparison with `main` would report every change to the API, which tells a reviewer nothing. M0 still fills in the publication metadata of TOOL-27. How a release is carried out is open (Section 15, TQ-4).

## 11. Container Image Requirements

The repository slang-rs-env defines the container image, so the changes of this section are made there and not in this repository.

TOOL-28. An agent MUST make each change to the container image in the repository slang-rs-env, and the first change in this repository after an image rebuild that adds or removes a tool MUST update the tool list in the Technology Decisions section of `AGENTS.md` and the status of the tool in Section 4.8 of the coding standard, because a document that calls an installed tool missing leaves its rules as judgement rules that a reviewer checks by hand, and a document that calls a missing tool installed leads an agent to build on it.

TOOL-29. The container image MUST provide these, because the recipes of Section 4.2 run them, and the run-time path lets the tests of M1 onward start:

- Every tool that Section 4.8 of the coding standard lists.
- A nightly toolchain with the components miri and rust-src, used only for checkers, as DEP-4 of the coding standard requires, with the stable toolchain as the default.
- The component llvm-tools of the stable toolchain, which cargo llvm-cov needs.
- GitHub's Secure Shell (SSH) host keys in the user's known hosts file, so that git can verify GitHub when it pushes.
- `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH` for every process, whether or not a login shell started it, so that a test binary finds the Slang shared library at run time (Section 4.4 of the architecture design).

The image is not required to provide a toolchain for the declared `rust-version`, because `check-msrv` runs in CI and installs that toolchain on first use (Section 17.3).

On 2026-11-10, after an image rebuild, the image provides every item of this list except the last, at the tool versions that Section 1.1 of the coding standard records. It also provides the component rust-src of the stable toolchain, CMake 3.31.6, Ninja 1.12.1, libclang, which Section 6.2 of the architecture design records, Python 3, which the existing checkers run on, and the tools and files that the Technology Decisions section of `AGENTS.md` lists.

The remaining gap is `LD_LIBRARY_PATH`. A login shell gets both the directory of the Vulkan loader and `$VULKAN_SDK/lib`, but a process that no login shell started gets only the directory of the Vulkan loader, because the image was rebuilt before the change to the `ENV` instruction of its Containerfile was merged. slang-rs-env contains that change now, and it takes effect at the next image rebuild. Until then, a test binary that links Slang fails to start outside a login shell (Section 17.2).

## 12. Documentation Changes in M0

Hard Rule 1 of `AGENTS.md` requires each of these updates in the change that alters what it describes, not at the end of M0:

- `AGENTS.md`: Project Status and Repository Layout for every file of Section 3.4, Validating Changes for every recipe of Section 4.2, the last sentence of Hard Rule 8, which says that the Rust checks have no recipes, and Technology Decisions, which shrinks to a summary that yields to the design documents.
- The coding standard: Section 4.1, which says that none of the configuration exists, Section 1.1 and the last paragraph of Section 4.8, which say that no recipe exists, the status statement of Section 4.8, which says that no recipe runs the tools, a row of Section 4.8 for each custom checker (TOOL-17), and the second part of the residual risk in Section 20.
- The writing guide: the "Checked by" statements and Section 7.12 for each custom checker, and Sections 7.11 and 8 for the extension of `scripts/check-prose` (TOOL-17).
- The architecture design: Section 1.6, and every row of its Sections 16 and 17 that M0 confirms or answers.
- `README.md`, which M0 creates.
- This document: Section 1.6, and the Status field, which changes to "Living document" when M0 is complete.

Every update of this list is made. The change that carried out steps 1 to 4 of Section 13 made the updates that those steps require, and it created `README.md`. The changes of steps 5 and 6 made the rest: the rows and "Checked by" statements of each custom checker (TOOL-17), Sections 7.11 and 8 of the writing guide, the summary in Technology Decisions of `AGENTS.md`, and the Status field of this document. The first CI run confirmed no row of Sections 16 and 17 of the architecture design, because A-17 and A-18 need a `gpu` test, which arrives in M3.

## 13. Implementation Order for M0

M0 is carried out as these changes, in this order. Each change updates the documents that Section 12 assigns to it, and leaves `just check` passing.

1. Delete the sketch (Section 3.1). Rewrite the root manifest with the members, the `[workspace.package]` fields, and the workspace lint table (Section 3.3), and add `clippy.toml`, the skeleton crate with its publication metadata (Sections 3.2 and 10), `LICENSE`, and `README.md`. The licence and the README come in this step, and not later, because Clippy reports a missing `readme` from step 2 onward and cargo-deny reads the licence in step 4.
2. Add `taplo.toml` and `typos.toml` (TOOL-30), and the recipes `fmt`, `check-fmt`, `check-toml`, `check-typos`, `check-clippy`, `check-doc`, `check-doctest`, `check-unused-deps`, `check-features`, `check-msrv`, and `check-rust`. Add `check-toml`, `check-typos`, and `check-rust` to `check`, and add the three hooks of Section 6.
3. Add `.config/nextest.toml` as Section 5 gives it, and add `test`, `test-gpu`, `test-hw-gpu`, `check-miri`, and `coverage`, with `test-gpu` and `check-miri` in `check-rust`.
4. Add `deny.toml` (Section 9) and `check-deny`, with `check-deny` in `check-rust`.
5. Add the custom checkers of Sections 7.2 to 7.5, and the extension of Section 7.6, each with its self-test and the document changes of TOOL-17.
6. Add the workflow of Section 8 with the jobs that arrive in M0, and add the recipe `check-workflows`, with `check-workflows` in `check`, and the workflow pin checker of Section 7.8 with its self-test.

Every step is done, and `just check` passes. Steps 1 to 4 were carried out together in one change. Steps 5 and 6 were carried out on one branch, which `just merge` squash-merges as one change together with their document changes.

## 14. Assumptions to Confirm

Each row is a fact that this document relies on and that has not been checked. A section that relies on one of them cites its identifier. The change that confirms or corrects an assumption states the result in the section that relies on it and removes the row. An identifier is never reused, so the next assumption takes TA-8.

No assumption is open. Every earlier assumption was checked against the tools in the development container and removed. TOOL-12 and Sections 5, 8.3, 9, 10, and 11 state the verified behaviour.

## 15. Open Questions

Each row is a question that this document leaves open. The design document in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row.

| Identifier | Question | Owning design document | Milestone |
| --- | --- | --- | --- |
| TQ-4 | How is a release carried out: which recipe publishes, in which order the crates are published, and where the crates.io token is kept outside the repository? | This document | M7 |
| TQ-6 | How does each published crate carry the text of its licence, which the GPL requires to accompany the work, given that Cargo does not package the root `LICENSE` when a manifest sets only the `license` field (Section 10)? | This document | M7 |

Section 3.4 records the answer to the removed question TQ-8, which is the root `.gitattributes` that keeps LF line endings in every checkout on every platform. Section 8.3 records the answer to the removed question TQ-1, which is Q-27 of the architecture design: each job reads the pin through `just pin`, installs the SDK with `just install-vulkan-sdk` against a checksum that the workflow records, checks it with `just check-vulkan-sdk`, and caches nothing. Section 8.1 records the answer to the removed question TQ-3, which is that CI installs the tools on each runner, and Section 7.8 records the answer to TQ-7, which is the checker `check-workflow-pins`. Section 4.2 records the answers of `docs/design/slang-compiler-sys.md` to the removed questions TQ-2, which is that no Cargo feature selects the static build, so the checks that pass `--all-features` are unaffected, and TQ-5, which is that the profile `default-miri` excludes the modules named `ffi`, `gpu`, and `hw_gpu`. An identifier is never reused, so the next question takes TQ-9.

## 16. Security Considerations

The threat model has six parts. First, a dependency, a build script, or a procedural macro in the dependency graph is malicious or compromised, and it runs with the permissions of the developer or the job whenever a check compiles the workspace, including Clippy, the documentation build, and the tests. Second, an action that a workflow uses is compromised, or its owner moves a tag to different code. Third, an attacker on the network or at a download host tampers with a tool, the Vulkan SDK, or the advisory database that a check or a job downloads. Fourth, the author of a pull request tries to read a secret or to write to the repository through CI. Fifth, a pre-commit hook repository changes the code that its hooks run. Sixth, a file or a file name that a checker reads, such as one in a pull request, holds terminal control sequences, and a checker that printed them unchanged would send them to the terminal of the developer, which is terminal escape injection.

The design addresses each part. For dependencies, cargo-deny rejects crates with known advisories and crates from sources other than crates.io (TOOL-26), the allow-list admits only reviewed licences (TOOL-24), and `Cargo.lock` fixes every version, while the checks run inside the development container and not on the developer's host. For actions, TOOL-20 pins each one to a commit SHA, which `check-workflow-pins` enforces, TOOL-19 limits what a compromised step can do, and the checkout step does not leave the token in the git configuration of the runner (Section 8.1). For downloads, TOOL-21 and TOOL-23 verify every file before use, the workflow restores nothing from a cache, the installer action carries its checksums in a manifest that its commit SHA pins, the expected checksum of the Vulkan SDK is recorded in the workflow and not fetched from the host that serves the SDK, and the download is verified before it is unpacked or run (Section 8.3), and a build script downloads nothing (SYS-5 of the coding standard). For pull requests, TOOL-22 keeps secrets out of every workflow and forbids the event that runs pull request code with secrets, and `check-workflow-pins` reports that event. For hooks, `.pre-commit-config.yaml` already pins the repository `pre-commit-hooks` to a commit SHA, and the local hooks run only recipes of this repository. For checker output, every custom checker escapes its output (Section 7.1), so a checked file cannot send a control sequence to the terminal.

These attacks are out of scope. A compromise of GitHub, crates.io, or LunarG themselves defeats every check here, and no repository configuration can detect it. The build of the container image is defined and secured in the repository slang-rs-env. A compromise of the developer's machine outside the container is a matter of that machine's security, not of this repository. The topics of unsafe code, foreign function interface invariants, untrusted shader source, and file paths do not apply, because this document defines no code that calls Slang or compiles shader source.

The residual risk has seven parts. First, a new dependency's build script and procedural macros run on the first build that includes them, before any reviewer has read them, and cargo-deny finds only vulnerabilities that are already published. Second, a checksum shows only that a file matches what the upstream published, and the first download of a file whose upstream publishes no checksum is trusted on first use. A checksum that comes from the same host as the file, as the checksum of actionlint does, proves integrity but not origin, so an attacker who controls that host can replace the file and its checksum together. Because CI installs the latest version of each tool, a new release of a tool runs in CI without any change to this repository and before anyone has reviewed it. Third, a pinned commit of an action is as trustworthy as the review it received when it was pinned, and the manifest of the installer action is as trustworthy as that commit. Fourth, `check-deny` fetches the advisory database on every run (Section 17.6), so a tampered database could hide an advisory. Fifth, `check-workflow-pins` reads workflows line by line, so a reference written in a form that it does not read (Section 7.8) can pass it, and the permissions of each job, the use of secrets, and whether each download step verifies its file depend on review. Sixth, the escaping covers only the custom checkers of Section 7. `scripts/check-commit-message` and the external tools that the recipes run, such as shellcheck, typos, actionlint, and Cargo, print their output without this escaping, and this document does not specify how they handle control characters. Because a backslash prints unchanged, escaped output can be ambiguous to a reader, which misleads but sends no control sequence. Seventh, the installers of the Vulkan SDK for macOS and Windows fetch metadata from LunarG's remote repository during the installation (Section 8.3), and the recorded checksum does not cover that metadata, so a tampered response is not detected. Two facts limit this risk: on 2026-11-10, the files that the installer installed came from the verified download, and the step selects only the core component. No supported option that installs without this access was found.

## 17. Operational Considerations

### 17.1. Recipes and Parameters

Section 4.2 lists every recipe. This document defines no Cargo feature. This table lists every recipe parameter and its default. Every other recipe takes no parameters.

| Recipe | Parameter | Default |
| --- | --- | --- |
| `check-prose` | Paths of files to check | Every Markdown file and every Rust file in the repository |
| `check-rust-comments`, `check-doc-headings` | Paths of files to check | Every Rust file in the repository (TOOL-14) |
| `check-doctest-attributes` | Paths of files to check | Every Rust file and every `README.md` file in the repository (TOOL-14) |
| `check-feature-docs` | Paths of Cargo manifests to check | The workspace of the repository (Section 7.5) |
| `check-workflow-pins` | Paths of workflow and composite action files to check | Every workflow under `.github/workflows/` and every composite action under `.github/actions/` (Section 7.8) |
| `check-commit-message` | The commit message file | None. The parameter is required. |
| `merge` | The branch, and further arguments for `scripts/merge-branch` | None for the branch, which is required |
| `install-vulkan-sdk` | The SDK version, as four numbers separated by full stops, and the expected SHA-256 checksum of the download for the platform, as 64 lowercase hexadecimal characters (Section 8.3) | None. Both are required. `just` rejects a missing or extra argument before the script runs, and the script exits with status 2 on a malformed one |

### 17.2. Environment Variables

This table lists every environment variable that the recipes, the hooks, or the checks read.

| Variable | Read by | Default and behaviour when unset |
| --- | --- | --- |
| `CARGO_BUILD_WARNINGS` | Cargo, in the commands of Section 4.8 of the coding standard | The recipes set it to `deny` for those commands. Outside them it is unset, and Cargo reports warnings without failing. |
| `VULKAN_SDK` | The build script of `slang-compiler-sys`, from M1, `check-vulkan-sdk`, `generate-bindings`, and the regeneration test that `test-gpu`, `check-bindings`, and `coverage` run on Linux | Section 19.2 of the architecture design. The development container sets it, and on a CI runner `install-vulkan-sdk` sets it for the later steps of the job (Section 8.3). `check-vulkan-sdk` exits with status 2 when it is unset or empty, and with status 1 when it names a path that is not a directory. `generate-bindings` and the regeneration test fail when it is unset, so `check` fails on Linux (Section 19.2 of `docs/design/slang-compiler-sys.md`). |
| `RUNNER_TEMP`, `GITHUB_ENV`, `GITHUB_PATH` | `install-vulkan-sdk` | Set by GitHub Actions on every runner. When one of them is unset or empty, the recipe exits with status 2 and a message that names it and says that this is not a GitHub Actions runner. |
| `SLANG_SOURCE_DIR` | The build script of `slang-compiler-sys`, from M1 | Section 19.2 of the architecture design. The development container does not set it. |
| `LIBCLANG_PATH` | `clang-sys`, when the binding generator or its tests run, from M1 | Section 19.2 of `docs/design/slang-compiler-sys.md` |
| `DOCS_RS` | The build script of `slang-compiler-sys`, from M1 | Unset, so the build proceeds on the selected route. `check-doc-without-slang` sets it. |
| `LD_LIBRARY_PATH` | The dynamic loader on Linux, when a test binary starts | In a login shell, the container includes `$VULKAN_SDK/lib` in it. A process that no login shell started lacks that directory until the next image rebuild (Section 11). Without it, a test binary that links Slang fails to start. On a Linux CI runner, `install-vulkan-sdk` sets it to `$VULKAN_SDK/lib` followed by any earlier value (Section 8.3). |
| `DYLD_LIBRARY_PATH` | The dynamic loader on macOS, when a test binary starts | Unset unless the developer sets it. On a macOS CI runner, `install-vulkan-sdk` sets it to `$VULKAN_SDK/lib` followed by any earlier value (Section 8.3). Whether it reaches a test binary on a developer's Mac is SA-7 of `docs/design/slang-compiler-sys.md`. |
| `PATH` | The dynamic loader on Windows, when a test binary starts, and every shell | On a Windows CI runner, `install-vulkan-sdk` adds `$VULKAN_SDK\Bin`, which holds `slang-compiler.dll`, and on Linux and macOS runners it adds `$VULKAN_SDK/bin` (Section 8.3). |
| `CLAUDE_PROJECT_DIR` | The hook in `.claude/settings.json` | Set by Claude Code. The hook does not run outside it. |

The workflow also sets these variables for its own steps, and no recipe reads them. Its top-level `env` holds `VULKAN_SDK_SHA256_LINUX`, `VULKAN_SDK_SHA256_MACOS`, and `VULKAN_SDK_SHA256_WINDOWS`, and the step that reads the pin writes `VULKAN_SDK_VERSION`, `SLANG_BUILD_TAG`, and `SLANG_COMMIT` to the file that `GITHUB_ENV` names. The step that installs the SDK passes the version and the checksum for its platform to `install-vulkan-sdk` as arguments (Section 8.3).

### 17.3. Platforms and Minimum Supported Rust Version

The checks run in the development container on Linux x86_64, and CI runs them on the platforms of Section 8.1, with tool versions that can differ from those of the container (Section 8.1). The Vulkan SDK in CI is the pinned version on every platform (Section 8.3). The workspace declares `rust-version = "1.99"` (TOOL-4), which only a change of TOOL-5 raises, and `check-msrv` verifies it in the CI job `ci-only`. cargo hack always uses the rustup toolchain named exactly like `rust-version`, which is `1.99`, and installs it with rustup when it is missing, even though the installed stable toolchain is 1.99.1. The first run of `check-msrv` on a machine therefore downloads a toolchain and needs network access. On 2026-11-10, in the development container, that first run installed the toolchain `1.99` in about eleven seconds, and later runs use the installed toolchain.

### 17.4. Failing Checks

This table says what a failure of each check means and what the developer does.

| Recipe | A failure means | What the developer does |
| --- | --- | --- |
| `check-fmt` | A Rust file is not formatted | Runs `just fmt` |
| `check-clippy`, `check-doc`, `check-doctest` | A lint fired | Follows Section 4.4 of the coding standard and the lint's row in Section 4.5 |
| `test`, `test-gpu`, `test-hw-gpu` | A test failed, or a test in a GPU tier found no adapter (ARCH-23 of the architecture design) | Fixes the code or the test, or runs the tier on a machine with an adapter |
| `check-bindings` | The regeneration test failed, with the message for its cause that Section 7.6 of `docs/design/slang-compiler-sys.md` lists, or the filter selected no test, as on Windows and macOS | For a difference, runs `just generate-bindings` and commits the regenerated file with the change that caused it. For a missing `VULKAN_SDK`, header directory, or libclang, installs or sets what the message names. On Windows or macOS, runs the recipe on Linux |
| `generate-bindings` | The binding generator failed, with an error of a kind that Section 7.7 of `docs/design/slang-compiler-sys.md` lists | Follows the recovery for that failure in Section 19.4 of `docs/design/slang-compiler-sys.md` |
| `check-deny` | A licence is not allowed, an advisory applies, a crate is duplicated, or a source is not crates.io. The exit status is the sum of 1 for advisories, 2 for bans, 4 for licences, and 8 for sources | Applies Section 9: replaces or upgrades the dependency, or records an exception that TOOL-24 or TOOL-26 permits |
| `check-prose`, `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs` | Exit status 1: a rule of the writing guide is broken at each reported `path:line`, and the message names the problem, such as "comment other than API documentation or a safety comment" or "feature 'name' is not named in a code span in the '# Features' section". Exit status 2: a file cannot be read, including a file that ends inside an unterminated literal, comment, or attribute, `cargo metadata` failed, the checker ran with no paths outside a git repository, or the setup is broken, as the message on standard error says (Section 7.1) | For status 1, fixes the text as the rule that Section 7 names requires. For status 2, fixes the file or the setup that the message names, such as by closing the unterminated item at the reported line |
| `check-workflow-pins` | Exit status 1: a rule of Section 8.2 is broken at each reported `path:line`, such as "uses: '...' is not pinned to a full 40-character commit SHA", "workflow uses the pull_request_target event", "workflow has no top-level permissions key", or "top-level permissions '...' grant more than read access". Exit status 2: as for the other custom checkers | Pins the action to the full commit SHA of the reviewed release, removes the event, or sets the top-level permissions to `contents: read` and moves any extra permission to the job that needs it (TOOL-19) |
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

No agent weakens a configuration to make a check pass, as Hard Rule 6 of `AGENTS.md` states. Apart from the first run of `check-msrv` (Section 17.3), the CI runs of Section 8.1, and the installation of the SDK in CI (Section 8.3), how long each check takes has not been measured, and this document states no expected duration.

A check that passes in the development container and fails in CI, or the reverse, can come from a difference in tool versions (Section 8.1). The developer compares the version in the log of the job with the version in the container before changing the code.

### 17.5. Logs

The recipes print the output of the tools they run and keep no log. GitHub Actions keeps the log of each job. The recipe `coverage` prints the output of the build and the tests and no coverage summary, and on 2026-11-10 its last line was `Finished report saved to target/lcov.info`. The report is the LCOV file `target/lcov.info`, which the CI job `coverage` uploads as the artifact `coverage-lcov` and GitHub keeps for 14 days (Section 8.1). `install-vulkan-sdk` prints the download address, the size and duration of the download, the verified checksum, the duration of the installation, the value of `VULKAN_SDK`, and the installed size. `check-vulkan-sdk` prints the files that it finds in the SDK, the content of `slang-tag-version.h`, and the output of `slangc -version`.

### 17.6. Network Access

`just check` needs network access. `check-deny` fetches the advisory database from <https://github.com/RustSec/advisory-db> on every run, and fails with exit status 1 when it cannot. With a database already cached, `cargo deny --offline check`, with the global option `--offline` before `check`, works without network access, but `check-deny` runs the command of Section 4.8 of the coding standard without it, so it always checks against the current database. The first run of `check-miri` after an image rebuild also needs network access, because Miri builds a sysroot and fetches the dependencies of the standard library, about 92 MB under `~/.cache/miri`. Later runs of `check-miri` work offline.

Outside `check`, the first run of `check-msrv` on a machine downloads a toolchain (Section 17.3). `install-vulkan-sdk` downloads the SDK from `sdk.lunarg.com`, and on macOS and Windows the installer also contacts LunarG's remote repository (Section 8.3).

## 18. References

### 18.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| TOOL-1 | 3.1 | The first M0 change deletes the sketch and its manifest entries, and reuses nothing. |
| TOOL-2 | 3.2 | M0 creates only the skeleton crate `slang-compiler-sys`. |
| TOOL-3 | 3.2 | The examples are one member, `slang-rust-examples`, with one binary target per example. |
| TOOL-4 | 3.3 | Licence, `rust-version` 1.99, repository, and readme are workspace fields that every member inherits. |
| TOOL-5 | 3.3 | `rust-version` changes only in a change whose purpose is to raise it. |
| TOOL-6 | 4.1 | Recipes have the name, group, and `check` membership of Section 4.2. |
| TOOL-7 | 4.1 | Recipes run the commands of Section 4.8 of the coding standard as written there, adding only a nextest profile, the arguments that Section 4.8 names for `coverage`, or the environment of Section 10, except that a recipe that selects named tests and must fail on an empty selection adds its filter and `--no-tests=fail`. |
| TOOL-8 | 4.1 | `check` passes in the container without a hardware GPU or a display. |
| TOOL-9 | 4.1 | No recipe for a tool before the image provides it. |
| TOOL-10 | 5 | The profiles `default`, `gpu`, and `hw-gpu` select tests by whole module path segments, and each profile other than `default` sets its own filter. |
| TOOL-11 | 5 | No profile retries a failed test. |
| TOOL-12 | 5 | The test recipes pass when their profile selects no test, through `--no-tests=warn`. |
| TOOL-13 | 6 | Commit hooks run only checks that do not compile the workspace. |
| TOOL-14 | 7.1 | Custom checkers follow the output and exit status conventions of `scripts/check-prose`. |
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
| TOOL-29 | 11 | The image provides the tools of Section 4.8, the nightly toolchain with Miri, llvm-tools, GitHub's SSH host keys, and the SDK library path for every process. |
| TOOL-30 | 3.4 | `taplo.toml` sets four-space indentation and excludes `target/` and `.worktrees/`, and `typos.toml` accepts the correct words that typos reports. |
| TOOL-31 | 9 | `deny.toml` denies unused licence allow-list entries and unused advisory ignores. |

## Change Log

- 2026-11-10: Recorded step 3 of M1: the recipes `generate-bindings` and `check-bindings` in Sections 4.2 and 17.4, the amendment of TOOL-7 that permits `--no-tests=fail` for a recipe that selects named tests, with TOOL-12, Section 5, and Appendix A to match, the regeneration test that `check` and the Linux CI jobs run in Sections 4.3 and 8.1, with the libclang versions and the results of the CI run, the root `.gitattributes` and the identifier entries of `typos.toml` in Section 3.4, the allow list of `.claude/settings.json` in Section 6, the clean failure of `scripts/check-vulkan-sdk` on a `VULKAN_SDK` that is not a directory and the exports of `scripts/install-vulkan-sdk` on each platform in Sections 8.3, 17.2, and 17.4, and the state in Section 1.6, and removed TQ-8 as answered by `.gitattributes`.
- 2026-11-10: Recorded step 2 of M1: the installation of the pinned Vulkan SDK in every CI job with its measured times and the decision not to cache it in Section 8.3, the recipes `install-vulkan-sdk` in the new group `ci` and `check-vulkan-sdk`, and the portable recipe `pin` in Sections 4.1 and 4.2, the `coverage` job and its artifact in Section 8.1, the installer metadata as a residual risk in Section 16, and the new parameters, variables, failures, and logs in Sections 17.1 to 17.6, corrected every statement that the recipe `coverage` prints a summary, described the exit statuses of the two scripts as they behave, dated the observations from the runners, said in Section 8.3 that the install step sets the loader variables that the SDK's setup script would set, removed TQ-1 as answered, and added TQ-8.
- 2026-11-10: Recorded the first change of M1 in Sections 1.6, 4.2, 5, 8.1, and 9: the recipe `pin`, the nextest profile `default-miri`, the installation of libclang in the `check` job and how it meets TOOL-21, the first CI run with the binding generator, and the licences Apache-2.0 and ISC in the allow-list, and moved the arrival of the `coverage` job to a later change of M1.
- 2026-11-10: Recorded the answers of `docs/design/slang-compiler-sys.md` to TQ-2 and TQ-5 in Section 4.2 and removed both questions, pointed TQ-1 and Section 8.3 to the requirements of that document, recorded in Section 8.1 that no job builds the static route, updated the environment variables of Section 17.2, added the recipe `test-static` to Section 4.2, and recorded in Section 7.2 that the generated bindings get no exemption from the comment checker.
- 2026-11-10: Recorded the fixes from a code review of the custom checkers: the escaping of their output and the terminal escape injection that it prevents in Sections 7.1 and 16, unterminated input as a read error, stable error wording, the handling of a shebang line, and the new placeholders and marker file of the self-test runner in Section 7.1, the Claude Code hook on Rust files in Section 6, and the new failures in Section 17.4.
- 2026-11-10: Recorded M0 as complete and changed the Status to "Living document", described the custom checkers, their self-test runner, the CI workflow, and the new hooks as implemented, added Section 7.8 for the workflow pin checker, corrected TOOL-15, reworded TOOL-21 to the checksum verification that the workflow performs, and removed TQ-3 and TQ-7 as answered.
- 2026-11-10: Recorded steps 1 to 4 of Section 13 as done in Sections 1.6, 12, and 13, corrected the order of `check-rust`, the commands of `check-features` and `check-msrv`, and the description of `typos.toml` to match the implementation, and recorded the first run of `check-msrv`.
- 2026-11-10: Recorded the tool behaviour verified after the image rebuild, resolved the open assumptions, moved the image-dependent recipes into M0 and `check-semver` to M7, and added TOOL-30, TOOL-31, TQ-6, and TQ-7.
- 2026-11-10: Initial version.
