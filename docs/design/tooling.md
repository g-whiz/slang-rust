# Tooling Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-10 |
| Applies to | The repository's configuration, recipes, hooks, and continuous integration |

## Abstract

This document specifies how the deterministic checks of the slang-rust repository are installed and run. It defines what milestone M0 changes in the repository: the removal of an abandoned sketch crate, the one skeleton crate that gives every tool something to check, and the configuration files that are added. It defines the `just` recipes and which of them make up the aggregate check that runs before every merge, the nextest configuration that implements the test tiers, the git hooks, the small custom checkers that move rules of the writing guide from review to a tool, the continuous integration workflow and its security rules, the dependency policy, the readiness for publishing, and the requirements on the development container image. It gives the order in which M0 makes these changes, so that every step leaves the aggregate check passing, and it lists the questions still open. None of this is implemented yet.

## 1. Introduction

### 1.1. Problem

The coding standard specifies a strict lint configuration and a set of tools, and the writing guide specifies rules for documentation, but the repository runs almost none of them. Only the prose, shell, and commit message checks have recipes. Until the rest are installed, every rule they would enforce is a judgement rule that a reviewer checks by hand, which the repository's central principle forbids wherever a tool can do the work.

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

The repository holds the `justfile` with the recipes that the Validating Changes section of `AGENTS.md` lists, the scripts under `scripts/`, the hooks in `.pre-commit-config.yaml`, `.editorconfig`, and `.claude/settings.json`. The workspace has one member, the abandoned sketch `crates/slang-compiler-sys`, which the Project Status section of `AGENTS.md` describes. No lint configuration, `clippy.toml`, `deny.toml`, nextest configuration, Rust check recipe, licence file, `README.md`, or CI workflow exists.

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

This table lists every file that M0 adds, rewrites, or removes, the section that defines its content, and the step of Section 13 that makes the change.

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
| `.pre-commit-config.yaml` | Extended | Section 6 | 2, 5 |
| `typos.toml`, `taplo.toml` | Added | Section 3.4 | 2 |
| `.config/nextest.toml` | Added | Section 5 | 3 |
| `deny.toml` | Added | Section 9 | 4 |
| The scripts of the custom checkers under `scripts/` | Added | Section 7 | 5 |
| `scripts/check-prose` | Extended | Section 7.6 | 5 |
| `.github/workflows/ci.yml` | Added | Section 8 | 6 |

No `rustfmt.toml` is added, because Section 4.8 of the coding standard specifies the default rustfmt settings.

TOOL-30. `taplo.toml` MUST set `indent_string = "    "` in its `[formatting]` table and `exclude = ["target/**", ".worktrees/**"]`, and `typos.toml` MUST accept each correct word that typos reports in the repository, because `check-toml` and `check-typos` otherwise fail on files that are correct. taplo indents with two spaces by default, which disagrees with the four spaces that `.editorconfig` sets for `*.toml`, and it does not read `.gitignore`, so without the exclusions it also checks the build directory and every worktree.

On 2026-11-10, typos reports four findings in the repository, and each is correct text: the string `ba` in the regular expression `(ba)?sh`, twice in `scripts/justfile-recipes` and once in `scripts/check-shell`, and `Ede`, the surname of an author, in `docs/rust-references.md`. `typos.toml` accepts them, for example with entries in its `[default.extend-words]` table. typos reads its configuration from `typos.toml`, `_typos.toml`, or `.typos.toml`, and M0 uses the first name.

`typos.toml` sets no locale. typos cannot enforce Canadian spelling, because with its default settings it accepts both `colour` and `color`, so `scripts/check-prose` and its word list keep that job (Section 8 of the writing guide).

## 4. Recipes

### 4.1. Rules

The `justfile` stays one flat file with `[group]` attributes, a help comment on every recipe, and shell that is `shellcheck`-clean, as Hard Rule 8 of `AGENTS.md` requires. The `checks` and `git` groups exist today. This document adds the group `rust` for the recipes that change files or run tests and are not checks. Every recipe that this document adds takes no parameters, except the custom checkers of Section 7, which take optional paths.

TOOL-6. An agent MUST add each recipe of Section 4.2 with the name, the group, and the membership of `check` that the table gives, and MUST update the table in the same change when one of them has to differ, because agents, hooks, and CI call recipes by name, and a recipe that differs from this table breaks every caller that follows it.

TOOL-7. A recipe that runs a command of Section 4.8 of the coding standard MUST run that command as Section 4.8 gives it, adding only the option `--profile` with a nextest profile of Section 5, the arguments that Section 4.8 names for the recipe `coverage`, or the environment of Section 10, and an agent that has to change the command changes Section 4.8 in the same change, because two command lines for one check diverge, and an agent that runs the documented command then gets a result different from the recipe's.

TOOL-8. The recipe `check` MUST pass in the development container without a hardware graphics processing unit (GPU) and without a display, and MUST NOT run a recipe that needs either, because an agent runs `check` before every merge in the container, as the Validating Changes section of `AGENTS.md` requires, and CI runs it on a hosted runner that is not required to provide either.

TOOL-9. An agent MUST NOT add a recipe for a tool that the container image does not provide until an image rebuild adds the tool, because the recipe would fail `check` for every agent until then.

### 4.2. Recipe Table

This table lists every recipe, what it runs, whether `check` runs it, and when it arrives. The commands of Section 4.8 of the coding standard are named by their purpose, and Section 4.8 gives each command line.

| Recipe | Group | Runs | In `check` | Arrives |
| --- | --- | --- | --- | --- |
| `default` | none | `just --list --unsorted` | No | Exists |
| `check` | `checks` | The recipes that Section 4.3 lists | Not applicable | Exists, extended in M0 |
| `check-prose` | `checks` | `scripts/check-prose`, extended to API documentation by Section 7.6 | Yes | Exists |
| `check-shell` | `checks` | `scripts/check-shell` | Yes | Exists |
| `check-commit-message` | `checks` | `scripts/check-commit-message` on one file, for the `commit-msg` hook | No | Exists |
| `check-commit-message-self-test` | `checks` | The self-test of the commit message checker | Yes | Exists |
| `hooks-install` | `checks` | `pre-commit install` | No | Exists |
| `merge` | `git` | `scripts/merge-branch` | No | Exists |
| `worktree-prune` | `git` | `scripts/worktree-prune` | No | Exists |
| `fmt` | `rust` | `cargo fmt` on the whole workspace, which applies the formatting that the formatting command of Section 4.8 checks, and `taplo fmt`, which formats every Tom's Obvious Minimal Language (TOML) file that `taplo.toml` does not exclude | No | M0 |
| `check-rust` | `checks` | The recipes that Section 4.3 lists | Yes | M0 |
| `check-fmt` | `checks` | The formatting command of Section 4.8 | Yes, through `check-rust` | M0 |
| `check-clippy` | `checks` | The Clippy command of Section 4.8 | Yes, through `check-rust` | M0 |
| `check-doc` | `checks` | The documentation command of Section 4.8 | Yes, through `check-rust` | M0 |
| `check-doctest` | `checks` | The doctest command of Section 4.8 | Yes, through `check-rust` | M0 |
| `test` | `rust` | The nextest command of Section 4.8 with the `default` profile | No, because `test-gpu` runs the same tests | M0 |
| `test-gpu` | `rust` | The nextest command of Section 4.8 with the `gpu` profile | Yes, through `check-rust` | M0 |
| `test-hw-gpu` | `rust` | The nextest command of Section 4.8 with the `hw-gpu` profile | No, because it needs a hardware GPU | M0 |
| `check-deny` | `checks` | The cargo-deny command of Section 4.8 | Yes, through `check-rust` | M0 |
| `check-rust-comments` | `checks` | The comment checker of Section 7.2 | Yes, through `check-rust` | M0 |
| `check-doc-headings` | `checks` | The heading checker of Section 7.3 | Yes, through `check-rust` | M0 |
| `check-doctest-attributes` | `checks` | The doctest attribute checker of Section 7.4 | Yes, through `check-rust` | M0 |
| `check-feature-docs` | `checks` | The feature documentation checker of Section 7.5 | Yes, through `check-rust` | M0 |
| `<checker>-self-test`, one for each custom checker | `checks` | The self-test of Section 7.1 | Yes | M0 |
| `check-bindings` | `checks` | The regeneration test of TEST-8 of the coding standard, alone | No, because `test-gpu` runs the same test | M1 |
| `generate-bindings` | `rust` | The binding generator | No | M1 |
| `check-doc-without-slang` | `checks` | The documentation command of Section 4.8 with `DOCS_RS` set and `VULKAN_SDK` unset (Section 10) | Yes, through `check-rust` | M1 |
| `check-typos` | `checks` | The typos command of Section 4.8 on the whole repository, with `typos.toml` (TOOL-30) | Yes | M0 |
| `check-toml` | `checks` | The taplo command of Section 4.8, with `taplo.toml` (TOOL-30) | Yes | M0 |
| `check-workflows` | `checks` | The actionlint command of Section 4.8, which checks every workflow | Yes | M0, with the first workflow |
| `check-unused-deps` | `checks` | The cargo machete command of Section 4.8 | Yes, through `check-rust` | M0 |
| `check-miri` | `checks` | The Miri command of Section 4.8, for the tests that the nextest profile `default-miri` selects | Yes, through `check-rust` | M0 |
| `check-features` | `checks` | The cargo hack commands of Section 4.8 for each feature and for the feature powerset | No, the CI job `features` runs it | M0 |
| `check-msrv` | `checks` | The MSRV command of Section 4.8, which builds the workspace with the toolchain that `rust-version` names | No, the CI job `msrv` runs it, and its first local run downloads that toolchain (Section 17.3) | M0 |
| `check-semver` | `checks` | cargo semver-checks against the last published version of each published crate | No, the CI job `semver` runs it | M7, with the first release (Section 10) |
| `coverage` | `rust` | The cargo llvm-cov command of Section 4.8 with the arguments that Section 4.8 names for it, which write an LCOV report and print a coverage summary. It is not a gate, and it fails while no crate contains a function | No, the CI job `coverage` runs it | M0 |

`check-miri` runs Miri through nextest, for the reason that Section 4.8 of the coding standard gives. Under Miri, nextest uses the profile named `default-miri`, which inherits from `default` when `.config/nextest.toml` does not define it. M0 does not define it, because the skeleton crate has no tests. Miri emulates some functions of libc and libm, but it cannot call an arbitrary native library such as Slang, so `check-miri` covers pure Rust logic, such as marshalling and layout code, and the unsafe code that TEST-7 of the coding standard tests against a fake implementation. A filter in a `[profile.default-miri]` table excludes the tests that call native code. Which tests it excludes is open (Section 15, TQ-5).

How the checks that pass `--all-features` and the feature matrix of `check-features` treat a feature that selects the static build of Slang is open (Section 15, TQ-2).

### 4.3. Aggregate Recipes

`check` runs `check-prose`, `check-shell`, `check-commit-message-self-test`, the self-test recipe of each custom checker, `check-typos`, `check-toml`, `check-workflows`, and `check-rust`. `check-workflows` joins `check` with the first workflow, in step 6 of Section 13, because actionlint exits with status 3 when the repository has no workflow. `check` needs network access (Section 17.6).

`check-rust` runs the custom checkers of Sections 7.2 to 7.5, `check-fmt`, `check-deny`, `check-unused-deps`, `check-clippy`, `check-doc`, `check-doctest`, `test-gpu`, and `check-miri`, in that order, so that the checks that do not compile the workspace report first. From M1 it also runs `check-doc-without-slang`.

`check-rust` runs `test-gpu` and not `test`, because the `gpu` profile selects every test that the `default` profile selects (Section 5). `test-gpu` runs in the development container on lavapipe, so `check` meets TOOL-8. A test in a `gpu` module that finds no adapter fails, as ARCH-23 of the architecture design requires, so `check` fails on a machine without any adapter.

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

The regular expression matches `gpu` or `hw_gpu` only at the start of the test path or after `::`, and only when `::` follows it. The trailing `::` makes the match a whole module segment. A test function named `gpu` is the last segment of its path and has no `::` after it, and a module named `gpu_buffers` has `_buffers` after `gpu`, so neither matches. This configuration was verified with nextest 0.9.151, which Section 1.1 of the coding standard records, on a scratch workspace that contains every path of the table in this section, in unit-test modules and in an integration-test binary. A filter that the command line adds with `-E` is intersected with the default filter of the profile, so it narrows the selection of the profile and never widens it.

TOOL-11. A profile MUST NOT retry a failed test, because a retry turns an intermittent failure on an adapter into a pass, which hides the failure in the same way that ARCH-23 of the architecture design forbids a test to hide a missing adapter. Zero retries is the default of nextest, and the `default` profile sets `retries = 0` explicitly so that the configuration states this rule instead of relying on the default. The other profiles inherit the value.

TOOL-12. The recipes `test`, `test-gpu`, and `test-hw-gpu` MUST pass when their profile selects no test, because the skeleton crate of M0 has no tests and no `gpu` test exists before M3, so a run that fails on an empty selection would fail `check` until then. nextest 0.9.151 fails such a run with the message "error: no tests to run" and exit status 4. Every nextest command of Section 4.8 of the coding standard therefore passes `--no-tests=warn`, with which an empty run passes and nextest still prints that no test ran, and a recipe keeps that option.

nextest does not run doctests, so `check-doctest` runs them, as Section 4.8 of the coding standard states.

## 6. Git Hooks

The hooks in `.pre-commit-config.yaml` call `just` recipes, as the existing local hooks do, because Hard Rule 8 of `AGENTS.md` makes every check a recipe.

TOOL-13. A hook of the `pre-commit` stage MUST run only checks that do not compile the workspace, because a commit hook that compiles the workspace makes every commit wait for a build, and Clippy, the tests, and the documentation build already run in `check` before every merge and in CI.

M0 adds these hooks of the `pre-commit` stage:

- `check-fmt`, which runs when a staged Rust file changes.
- `check-typos`, which runs on every commit.
- `check-toml`, which runs when a staged TOML file changes.

Step 2 of Section 13 adds all three.

When Section 7.6 extends `check-prose` to Rust files, the existing `check-prose` hook also runs when a staged Rust file changes. The `commit-msg` and `post-merge` hooks do not change.

## 7. Custom Checkers

The writing guide marks several rules as judgement rules until a tool covers them, in the "Checked by" statements of its Section 7 and in its Section 7.12. Each of these rules can be checked deterministically, so this section plans a small custom checker for each.

### 7.1. Conventions

TOOL-14. Each custom checker MUST be a script under `scripts/` with a recipe of the same name in the `checks` group, and MUST follow the conventions that the usage text of `scripts/check-prose` states: one `path:line: message` line for each finding, exit status 0 without findings, 1 with findings, and 2 when a file cannot be read or the setup is broken, after checking every readable file, and usage text on `-h`. With no paths, the checker checks every relevant file in the git repository, tracked or not ignored. A checker that differs breaks the hooks and the recipes, which rely on these exit statuses to tell a finding from a broken setup.

The checkers are written in Python 3 and run with `python3 -I`, as `scripts/check-prose` and `scripts/check-commit-message` are, because the container provides Python 3 and the existing checkers set the pattern.

TOOL-15. Each custom checker MUST have a self-test recipe named after it with the suffix `-self-test`, which `check` runs, and which writes at least one passing input and one failing input for each rule of the checker into a temporary directory and fails unless the checker accepts the first and rejects the second, as `check-commit-message-self-test` does, because a checker that silently stops finding anything passes `check` while it enforces nothing. The inputs live only in the self-test, because a failing input stored in the repository would be reported by the checker itself.

### 7.2. Comment Checker

This table summarizes the comment checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-rust-comments`, `check-rust-comments` |
| Rule | Section 7.2 of the writing guide, and Hard Rule 5 of `AGENTS.md` |
| Reads | Every `.rs` file |
| Reports | Every comment other than a `///` comment, a `//!` comment, or a safety comment, and every comment that contains `TODO` or `FIXME` |

A safety comment is a run of consecutive `//` line comments whose first line starts with `// SAFETY:`, as the illustration in Section 7.6 of the writing guide shows. The checker reports every block comment, including `/** */` and `/*! */`, and every line comment that starts with four slashes. rustdoc and `missing_docs` treat `/** */` and `/*! */` as documentation, but the checker reports them because the writing guide defines API documentation as `///` and `//!` comments only (Section 2.2 of the writing guide). `////` and `/*** */` are not documentation comments, so they are other comments, which Section 7.2 of the writing guide forbids. Whether a safety comment sits directly above an `unsafe` block is checked by the safety comment lints of Section 4.5 of the coding standard, not by this checker.

TOOL-16. The comment checker MUST find comments by reading Rust tokens, so that the characters `//` and `/*` inside a string literal, a raw string literal, or a character literal are not reported, because a checker that reports text inside literals produces findings that no fix can remove.

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

The checker does not report line 4, because the slashes there are inside a string literal. If the binding generator of M1 has to emit other comments, the design document of `slang-compiler-sys` adds an exemption for the generated file to Section 7.2 of the writing guide and to this checker in the same change.

### 7.3. Heading Checker

This table summarizes the heading checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-doc-headings`, `check-doc-headings` |
| Rule | Section 7.3 of the writing guide |
| Reads | The `///` comments of every `.rs` file. Crate-level and module-level documentation, written with `//!`, is not read, because Section 7.4 of the writing guide gives it its own headings |
| Reports | A heading that is not one of the standard headings of Section 7.3 of the writing guide, a standard heading out of the order that section gives, and a standard heading that appears twice in one item's documentation |

The checker ignores lines inside fenced code blocks, because a line that starts with `# ` in a Rust code block is a hidden line of a doctest, not a heading. Section 7.3 of the writing guide discourages other headings. The checker reports every other heading, so the change that lands the checker rewrites that sentence as a rule that the checker enforces.

### 7.4. Doctest Attribute Checker

This table summarizes the doctest attribute checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-doctest-attributes`, `check-doctest-attributes` |
| Rule | Section 7.5 of the writing guide |
| Reads | The code blocks in the `///` and `//!` comments of every `.rs` file, and in `README.md`, which a crate includes as doctests (Section 7.10 of the writing guide) |
| Reports | A code block whose attributes include `ignore` or an attribute that starts with `ignore-` |

The checker reports the attributes that start with `ignore-` as well, because Section 7.5 of the writing guide forbids `ignore` and the checker treats these as variants of it. The agent that writes the checker verifies against rustdoc how rustdoc reads the attributes of a code block from its info string, and which attributes are variants of `ignore`. Whether a `no_run` example states its reason stays a judgement rule.

### 7.5. Feature Documentation Checker

This table summarizes the feature documentation checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-feature-docs`, `check-feature-docs` |
| Rule | Section 7.8 of the writing guide |
| Reads | For each workspace member with a library target, its features as Cargo reports them in the package metadata, and the `# Features` section of its crate-level documentation. The agent that writes the checker verifies whether Cargo reports the features that it derives from optional dependencies |
| Reports | A feature, other than `default`, that the `# Features` section does not name in a code span, a feature that the section names and the package does not have, and a missing `# Features` section in a crate that has a feature |

Whether the section says that each feature is on or off by default, and what it enables, stays a judgement rule, because the wording is free.

### 7.6. Prose in API Documentation

`scripts/check-prose` is extended to read the text of the `///` and `//!` comments of `.rs` files and to apply to it the rules that it applies to Markdown, which enforces Section 7.11 of the writing guide for the rules of its Section 8 that the script checks. Code spans and fenced code blocks inside the comments stay exempt, as they are in Markdown. With no paths, `check-prose` then checks every Markdown file and every Rust file. The recipe keeps its name and parameters.

### 7.7. Moving a Rule to Enforced

TOOL-17. The change that lands a custom checker MUST, in the same change, rewrite the "Checked by" statement of the section of the writing guide that the checker enforces and its row in Section 7.12 of the writing guide to name the checker, add the checker to the table of tools in Section 4.8 of the coding standard, and update the Validating Changes section of `AGENTS.md`, as Section 3.5 of the writing guide and Section 19 of the coding standard require for a rule that a tool can check, because a document that still calls an enforced rule a judgement rule sends a reviewer to check by hand what a tool checks, which Hard Rule 4 of `AGENTS.md` forbids. The change that extends `scripts/check-prose` also updates Section 8 of the writing guide, which describes the files that the script and its hook check.

## 8. Continuous Integration

### 8.1. Workflow and Jobs

One workflow, `.github/workflows/ci.yml`, runs on every pull request and on every push to `main`.

TOOL-18. A workflow MUST run every check through a `just` recipe and MUST NOT run a check command directly, because CI then runs the same commands as the development container, so a check that fails in one place has the same command to reproduce it in the other. Steps that only prepare a runner, such as installing a tool, are not checks.

This table lists the jobs. The runner labels and the setup steps are chosen when the workflow is written, and `check-workflows` checks them.

| Job | Runner | Runs | Arrives |
| --- | --- | --- | --- |
| `check` | Linux x86_64, with lavapipe as the software adapter from M3, when the first `gpu` test exists | `just check` | M0 |
| `test` | A matrix of Windows x86_64, with the Windows Advanced Rasterization Platform (WARP) as the software adapter, and macOS arm64, with the hosted Metal device | `just check-clippy`, `just check-doctest`, and `just test-gpu` | M0 |
| `features` | Linux x86_64 | `just check-features` | M0 |
| `msrv` | Linux x86_64 | `just check-msrv` | M0 |
| `semver` | Linux x86_64 | `just check-semver` | M7, with the first release (Section 10) |
| `coverage` | Linux x86_64 | `just coverage`, with the coverage summary in the log of the job | With the first crate that contains a function, because `just coverage` fails while no crate has code |

The `test` job runs Clippy and the doctests on Windows and macOS as well as the tests, because a lint exception has to be fulfilled on every target that is checked (Section 4.4 of the coding standard), and code under `#[cfg]` differs between platforms. Whether every platform can run the GPU tier while SPIR-V is the only shader target is open (Section 17, Q-21 of the architecture design). If the answer is that a platform cannot, its entry in the matrix runs `just test` instead of `just test-gpu` until the shader target is configurable. That lavapipe, WARP, and hosted Metal work as adapters on hosted runners is an assumption of the architecture design (Section 16, A-17 and A-18 of the architecture design).

No job runs the `hw_gpu` tests, which run only on a developer's hardware GPU through `just test-hw-gpu`. No job builds the static route until the design document of `slang-compiler-sys` decides whether CI builds it (Section 17, Q-28 of the architecture design).

How CI obtains the same versions of the tools as the container image is open (Section 15, TQ-3).

### 8.2. Workflow Security

TOOL-19. A workflow MUST grant its token read access to the repository contents only, at the level of the workflow, and a job that needs more MUST request the extra permission on that job alone, because a compromised step can use every permission that the token holds.

TOOL-20. A workflow MUST reference every action, including the actions that GitHub publishes, by a full commit SHA and never by a tag or a branch, because the owner of an action can move a tag or a branch to different code, which then runs in every later job without any change to this repository. actionlint does not check that an action is pinned by a commit SHA, so TOOL-20 is a judgement rule until a checker covers it (Section 15, TQ-7).

TOOL-21. A workflow step that downloads a file MUST record the expected SHA-256 checksum in the workflow and MUST stop the job before the file is used when the checksum differs, as Hard Rule 7 of `AGENTS.md` requires for every download whose upstream publishes a checksum, and a file restored from a cache MUST be verified in the same way as a fresh download, because a tampered file that is not verified runs with the permissions of the job, and a cache can hold a file that an earlier, compromised run stored. When the upstream publishes no checksum, the step SHOULD record the checksum of the file that the agent verified when it pinned the download, because a later change in the served file is then detected, even though the first file is trusted on first use.

TOOL-22. A workflow MUST NOT use the `pull_request_target` event, read a secret, or publish a crate, because the workflows need no secret to check code, and a workflow that holds a secret while it runs the code of a pull request exposes that secret to the author of the pull request.

### 8.3. The Vulkan SDK on CI Runners

Every job needs the pinned Vulkan software development kit (SDK) for Slang from M1, when `slang-compiler-sys` first links Slang. No job of M0 needs it, because the skeleton crate does not link Slang. How the SDK is installed on each platform, including the download address, checksum verification, and caching, is open (Section 15, TQ-1, which is Q-27 of the architecture design). It is settled when the step that installs the SDK is first written and run in CI.

LunarG publishes a SHA-256 checksum of each SDK download, as 64 hexadecimal characters at a documented address of the form `https://sdk.lunarg.com/sdk/sha/<version>/<platform>/<file>.txt`, which is served from the same host as the download. A checksum fetched from that host at run time detects a corrupted download, but not a download that an attacker at the host replaced together with its checksum. The step therefore records the expected value in the workflow, as TOOL-21 requires, and does not fetch it at run time.

TOOL-23. The step that installs the Vulkan SDK on a runner MUST read the SDK version from the one place that records the pin (ARCH-9 of the architecture design) and MUST NOT restate the version, MUST verify the download as TOOL-21 requires, MUST set `VULKAN_SDK` for the later steps, and MUST make the Slang shared library findable at run time on the runner in the way that Section 4.4 of the architecture design describes for the platform, because a second copy of the pin diverges from the first, an unverified installer runs with the job's permissions, and a test binary that cannot find the shared library fails before any test runs.

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

## 13. Implementation Order for M0

M0 is carried out as these changes, in this order. Each change updates the documents that Section 12 assigns to it, and leaves `just check` passing.

1. Delete the sketch (Section 3.1). Rewrite the root manifest with the members, the `[workspace.package]` fields, and the workspace lint table (Section 3.3), and add `clippy.toml`, the skeleton crate with its publication metadata (Sections 3.2 and 10), `LICENSE`, and `README.md`. The licence and the README come in this step, and not later, because Clippy reports a missing `readme` from step 2 onward and cargo-deny reads the licence in step 4.
2. Add `taplo.toml` and `typos.toml` (TOOL-30), and the recipes `fmt`, `check-fmt`, `check-toml`, `check-typos`, `check-clippy`, `check-doc`, `check-doctest`, `check-unused-deps`, `check-features`, `check-msrv`, and `check-rust`. Add `check-toml`, `check-typos`, and `check-rust` to `check`, and add the three hooks of Section 6.
3. Add `.config/nextest.toml` as Section 5 gives it, and add `test`, `test-gpu`, `test-hw-gpu`, `check-miri`, and `coverage`, with `test-gpu` and `check-miri` in `check-rust`.
4. Add `deny.toml` (Section 9) and `check-deny`, with `check-deny` in `check-rust`.
5. Add the custom checkers of Sections 7.2 to 7.5, and the extension of Section 7.6, one change for each, each with its self-test and the document changes of TOOL-17.
6. Add the workflow of Section 8 with the jobs that arrive in M0, and add the recipe `check-workflows`, with `check-workflows` in `check`.

## 14. Assumptions to Confirm

Each row is a fact that this document relies on and that has not been checked. A section that relies on one of them cites its identifier. The change that confirms or corrects an assumption states the result in the section that relies on it and removes the row. An identifier is never reused, so the next assumption takes TA-8.

No assumption is open. Every earlier assumption was checked against the tools in the development container and removed. TOOL-12 and Sections 5, 8.3, 9, 10, and 11 state the verified behaviour.

## 15. Open Questions

Each row is a question that this document leaves open. The design document in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row.

| Identifier | Question | Owning design document | Milestone |
| --- | --- | --- | --- |
| TQ-1 | How is the pinned Vulkan SDK installed on each CI runner, including its download address, checksum verification, and caching? This is Q-27 of the architecture design, and any answer meets TOOL-21 and TOOL-23. Section 8.3 states where LunarG publishes the checksum of each download. | This document | M1, when the step is first written and run |
| TQ-2 | How do the checks that pass `--all-features`, which are Clippy, the documentation build, the doctests, and the tests, and the feature matrix of `check-features`, treat a feature that selects the static build of Slang, which needs a Slang source tree and a long native build? This is part of Q-7 of the architecture design. | `docs/design/slang-compiler-sys.md` | M1 |
| TQ-3 | How does CI use the same tool versions as the container image: by running the Linux jobs in the container image, or by installing pinned versions on each runner? Any answer meets TOOL-20 and TOOL-21. | This document | M0, when the workflow is first written |
| TQ-4 | How is a release carried out: which recipe publishes, in which order the crates are published, and where the crates.io token is kept outside the repository? | This document | M7 |
| TQ-5 | Which tests does the filter of the nextest profile `default-miri` exclude from `check-miri`, because they call native code? The mechanism is that filter (Section 4.2). | The design document of the first crate whose tests call native code: `docs/design/slang-compiler-sys.md`, whose smoke test of M1 calls Slang, or `docs/design/slang-compiler.md` | M1 or M2 |
| TQ-6 | How does each published crate carry the text of its licence, which the GPL requires to accompany the work, given that Cargo does not package the root `LICENSE` when a manifest sets only the `license` field (Section 10)? | This document | M7 |
| TQ-7 | Does a small custom checker enforce TOOL-20, which actionlint does not check (Section 8.2)? | This document | M0, when the first workflow is written |

## 16. Security Considerations

The threat model has five parts. First, a dependency, a build script, or a procedural macro in the dependency graph is malicious or compromised, and it runs with the permissions of the developer or the job whenever a check compiles the workspace, including Clippy, the documentation build, and the tests. Second, an action that a workflow uses is compromised, or its owner moves a tag to different code. Third, an attacker on the network or at a download host tampers with a tool, the Vulkan SDK, or the advisory database that a check or a job downloads. Fourth, the author of a pull request tries to read a secret or to write to the repository through CI. Fifth, a pre-commit hook repository changes the code that its hooks run.

The design addresses each part. For dependencies, cargo-deny rejects crates with known advisories and crates from sources other than crates.io (TOOL-26), the allow-list admits only reviewed licences (TOOL-24), and `Cargo.lock` fixes every version, while the checks run inside the development container and not on the developer's host. For actions, TOOL-20 pins each one to a commit SHA, and TOOL-19 limits what a compromised step can do. For downloads, TOOL-21 and TOOL-23 verify every file before use, including files restored from a cache, the expected checksum of the Vulkan SDK is recorded in the workflow and not fetched from the host that serves the SDK (Section 8.3), and a build script downloads nothing (SYS-5 of the coding standard). For pull requests, TOOL-22 keeps secrets out of every workflow and forbids the event that runs pull request code with secrets. For hooks, `.pre-commit-config.yaml` already pins the repository `pre-commit-hooks` to a commit SHA, and the local hooks run only recipes of this repository.

These attacks are out of scope. A compromise of GitHub, crates.io, or LunarG themselves defeats every check here, and no repository configuration can detect it. The build of the container image is defined and secured in the repository slang-rs-env. A compromise of the developer's machine outside the container is a matter of that machine's security, not of this repository. The topics of unsafe code, foreign function interface invariants, untrusted shader source, and file paths do not apply, because this document defines no code that calls Slang or compiles shader source.

The residual risk has five parts. First, a new dependency's build script and procedural macros run on the first build that includes them, before any reviewer has read them, and cargo-deny finds only vulnerabilities that are already published. Second, a checksum shows only that a file matches what the upstream published, and the first download of a file whose upstream publishes no checksum is trusted on first use. Third, a pinned commit of an action is as trustworthy as the review it received when it was pinned. Fourth, `check-deny` fetches the advisory database on every run (Section 17.6), so a tampered database could hide an advisory. Fifth, TOOL-20 is a judgement rule until a checker covers it (Section 15, TQ-7), so an action pinned by a tag can pass review.

## 17. Operational Considerations

### 17.1. Recipes and Parameters

Section 4.2 lists every recipe. This document defines no Cargo feature. This table lists every recipe parameter and its default. Every other recipe takes no parameters.

| Recipe | Parameter | Default |
| --- | --- | --- |
| `check-prose` | Paths of files to check | Every Markdown file in the repository, and after Section 7.6 every Rust file as well |
| `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs` | Paths of files to check | Every relevant file in the repository (TOOL-14) |
| `check-commit-message` | The commit message file | None. The parameter is required. |
| `merge` | The branch, and further arguments for `scripts/merge-branch` | None for the branch, which is required |

### 17.2. Environment Variables

This table lists every environment variable that the recipes, the hooks, or the checks read.

| Variable | Read by | Default and behaviour when unset |
| --- | --- | --- |
| `CARGO_BUILD_WARNINGS` | Cargo, in the commands of Section 4.8 of the coding standard | The recipes set it to `deny` for those commands. Outside them it is unset, and Cargo reports warnings without failing. |
| `VULKAN_SDK` | The build script of `slang-compiler-sys`, from M1 | Section 19.2 of the architecture design. The development container sets it. |
| `SLANG_SOURCE_DIR` (proposed) | The build script of `slang-compiler-sys`, from M1 | Section 19.2 of the architecture design |
| `DOCS_RS` | The build script of `slang-compiler-sys`, from M1 | Unset, so the build proceeds on the selected route. `check-doc-without-slang` sets it. |
| `LD_LIBRARY_PATH` | The dynamic loader on Linux, when a test binary starts | In a login shell, the container includes `$VULKAN_SDK/lib` in it. A process that no login shell started lacks that directory until the next image rebuild (Section 11). Without it, a test binary that links Slang fails to start. |
| `CLAUDE_PROJECT_DIR` | The hook in `.claude/settings.json` | Set by Claude Code. The hook does not run outside it. |

### 17.3. Platforms and Minimum Supported Rust Version

The checks run in the development container on Linux x86_64, and CI runs them on the platforms of Section 8.1. The workspace declares `rust-version = "1.99"` (TOOL-4), which only a change of TOOL-5 raises, and `check-msrv` verifies it in the CI job `msrv`. cargo hack always uses the rustup toolchain named exactly like `rust-version`, which is `1.99`, and installs it with rustup when it is missing, even though the installed stable toolchain is 1.99.1. The first run of `check-msrv` on a machine therefore downloads a toolchain and needs network access.

### 17.4. Failing Checks

This table says what a failure of each check means and what the developer does.

| Recipe | A failure means | What the developer does |
| --- | --- | --- |
| `check-fmt` | A Rust file is not formatted | Runs `just fmt` |
| `check-clippy`, `check-doc`, `check-doctest` | A lint fired | Follows Section 4.4 of the coding standard and the lint's row in Section 4.5 |
| `test`, `test-gpu`, `test-hw-gpu` | A test failed, or a test in a GPU tier found no adapter (ARCH-23 of the architecture design) | Fixes the code or the test, or runs the tier on a machine with an adapter |
| `check-deny` | A licence is not allowed, an advisory applies, a crate is duplicated, or a source is not crates.io. The exit status is the sum of 1 for advisories, 2 for bans, 4 for licences, and 8 for sources | Applies Section 9: replaces or upgrades the dependency, or records an exception that TOOL-24 or TOOL-26 permits |
| `check-prose`, the custom checkers | A rule of the writing guide is broken at the reported line | Fixes the text as the rule that Section 7 names requires |
| A self-test recipe | A checker accepts a violation or rejects a valid input | Fixes the checker |
| `check-shell` | shellcheck reported a finding | Fixes the script or recipe |
| `check-typos` | typos found a spelling mistake | Fixes the word, or accepts a correct word in `typos.toml` (TOOL-30) |
| `check-toml` | A TOML file is not formatted | Runs `just fmt` |
| `check-workflows` | actionlint reported a finding in a workflow | Fixes the workflow |
| `check-unused-deps` | A dependency is declared and not used | Removes it |
| `check-miri` | Miri found undefined behaviour | Fixes the unsafe code, following Section 9 of the coding standard |
| `coverage` | No coverage data was found, because no crate contains a function yet | Waits for the first crate that contains a function |
| `check-features`, `check-msrv`, `check-semver` | A feature combination does not compile, the code needs a newer toolchain than `rust-version`, or the API has a breaking change | Fixes the code, raises `rust-version` under TOOL-5, or releases the change as breaking |

Clippy can report a `clippy::cargo_common_metadata` warning again after the manifest has been fixed, because it replays its cached result until a source file of the crate changes. The developer then updates the modification time of a source file of the crate, or cleans the build output of the crate, and runs the check again.

No agent weakens a configuration to make a check pass, as Hard Rule 6 of `AGENTS.md` states. How long each check takes has not been measured, and this document states no expected duration.

### 17.5. Logs

The recipes print the output of the tools they run and keep no log. GitHub Actions keeps the log of each job, and the log of the `coverage` job holds the coverage summary. The recipe `coverage` also writes the LCOV report `target/lcov.info`.

### 17.6. Network Access

`just check` needs network access. `check-deny` fetches the advisory database from <https://github.com/RustSec/advisory-db> on every run, and fails with exit status 1 when it cannot. With a database already cached, `cargo deny --offline check`, with the global option `--offline` before `check`, works without network access, but `check-deny` runs the command of Section 4.8 of the coding standard without it, so it always checks against the current database. The first run of `check-miri` after an image rebuild also needs network access, because Miri builds a sysroot and fetches the dependencies of the standard library, about 92 MB under `~/.cache/miri`. Later runs of `check-miri` work offline.

Outside `check`, the first run of `check-msrv` on a machine downloads a toolchain (Section 17.3).

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
| TOOL-7 | 4.1 | Recipes run the commands of Section 4.8 of the coding standard as written there, adding only a nextest profile, the arguments that Section 4.8 names for `coverage`, or the environment of Section 10. |
| TOOL-8 | 4.1 | `check` passes in the container without a hardware GPU or a display. |
| TOOL-9 | 4.1 | No recipe for a tool before the image provides it. |
| TOOL-10 | 5 | The profiles `default`, `gpu`, and `hw-gpu` select tests by whole module path segments, and each profile other than `default` sets its own filter. |
| TOOL-11 | 5 | No profile retries a failed test. |
| TOOL-12 | 5 | The test recipes pass when their profile selects no test, through `--no-tests=warn`. |
| TOOL-13 | 6 | Commit hooks run only checks that do not compile the workspace. |
| TOOL-14 | 7.1 | Custom checkers follow the output and exit status conventions of `scripts/check-prose`. |
| TOOL-15 | 7.1 | Each custom checker has a self-test recipe in `check`. |
| TOOL-16 | 7.2 | The comment checker reads Rust tokens and ignores literals. |
| TOOL-17 | 7.7 | Landing a checker moves its rule to enforced in every document in the same change. |
| TOOL-18 | 8.1 | Workflows run every check through a recipe. |
| TOOL-19 | 8.2 | Workflow tokens are read-only by default, with extra permissions per job. |
| TOOL-20 | 8.2 | Every action is pinned by a full commit SHA. |
| TOOL-21 | 8.2 | Every download, and every file restored from a cache, is verified by SHA-256 before use. |
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

- 2026-11-10: Recorded the tool behaviour verified after the image rebuild, resolved the open assumptions, moved the image-dependent recipes into M0 and `check-semver` to M7, and added TOOL-30, TOOL-31, TQ-6, and TQ-7.
- 2026-11-10: Initial version.
