# Tooling Design

| Field | Value |
| --- | --- |
| Status | Living document |
| Date | 2026-11-19 |
| Applies to | The repository's configuration, recipes, hooks, and continuous integration |

## Abstract

This document specifies how the deterministic checks of the slang-rust repository are configured and run. It defines the `just` recipes, the aggregate check that runs before every merge, the nextest profiles for the test tiers, and the git hooks. It also specifies the custom checkers that move rules from review to a tool, the continuous integration workflow that runs all of these checks, and the release procedure with its check for breaking changes and its hosted documentation.

## 1. Introduction

### 1.1. Problem

The coding standard specifies a strict lint configuration and a set of tools, and the writing guide specifies rules for documentation. Until a tool is installed and run, every rule it would enforce is a judgement rule that a reviewer checks by hand. The repository's central principle forbids that wherever a tool can do the work.

### 1.2. Purpose

This document turns the specifications of the standards documents into installed configuration and runnable checks. It follows the principle that Hard Rule 4 of `AGENTS.md` and Section 3.5 of the writing guide state: anything a deterministic tool can check is checked by a tool and never by a reviewer.

### 1.3. Audience

The primary readers are the coding agents that add or change a recipe, a checker, a hook, a tool configuration, or a continuous integration (CI) job. The secondary readers are reviewers, and the owner of the repository, who approves its decisions.

### 1.4. Scope

This document covers the tool configuration files, the workspace manifest fields that are not lints, the recipes, the nextest configuration, the hooks, the custom checkers, the CI workflow and the documentation workflow, the cargo-deny policy, publishing readiness, the versions of a release, the check for breaking changes, the hosted documentation, the review of the public API before a release, the release procedure, and the requirements on the container image.

### 1.5. Non-Goals

Each fact has one owner, as Section 4.5 of the writing guide requires, and this document cites the owner instead of restating the fact.

- Section 4 of `docs/rust-coding-standards.md` owns the lint tables, the contents of `clippy.toml`, the list of tools with what each one checks, and the exact cargo command lines of Section 4.8. This document says which recipe runs which command, and when.
- `docs/design/architecture.md` owns the crates, their layering, the directories of the workspace members, the roadmap, the test tiers of its Section 11, the platforms, and the publishing policy. This document defines only the mechanisms that implement them.
- `AGENTS.md` owns the hard rules and the development workflow.

This document does not design the binding generator `slang-bindgen`, the build scripts of `slang-compiler-sys`, `slang-src`, and `slang-glslang-sys`, or the application programming interface (API) of any crate, which the design documents of the crates own. It does not specify the contents of the container image beyond the requirements of Section 10, because the repository slang-rs-env defines the image.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that these documents define, and calls them the writing guide, the coding standard, and the architecture design:

- Section 2.2 of `docs/writing-documentation.md`, such as agent, reviewer, enforced rule, and judgement rule.
- Section 2.2 of `docs/rust-coding-standards.md`, such as lint, workspace lint table, and the check.
- Section 2.1 of `docs/design/architecture.md`, such as adapter, software adapter, milestone, nextest profile, recorded Slang, and vendored build.

It adds these terms:

- action: A reusable unit of steps for a GitHub Actions workflow, published in a git repository and referenced from a workflow.
- aggregate recipe: A recipe that only runs other recipes, such as `check`.
- commit SHA: The full 40-character hexadecimal hash that names one git commit.
- container image: The image, defined in the repository slang-rs-env, from which the development container runs.
- custom checker: A script in this repository that checks one rule of the writing guide, the coding standard, or a design document that no installed tool checks (Section 6).
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
- Software Package Data Exchange (SPDX) licence identifier: The standard short name of a licence, such as `GPL-3.0-or-later`.
- SPDX licence expression: One or more SPDX licence identifiers combined with `OR`, `AND`, or `WITH` and an exception, such as `MIT OR Apache-2.0` or `Apache-2.0 WITH LLVM-exception`, which Cargo's `license` field takes.
- workflow: A YAML Ain't Markup Language (YAML) file under `.github/workflows/` that defines the jobs that GitHub Actions runs.

## 3. Recipes

### 3.1. Rules

The `justfile` stays one flat file with `[group]` attributes, a help comment on every recipe, and shell that is `shellcheck`-clean, as Hard Rule 8 of `AGENTS.md` requires. It has five groups:

- `checks`: The checks, their self-tests, `check-vulkan-sdk`, and `hooks-install`.
- `rust`: The recipes that change files, print the recorded Slang, or run tests and are not checks.
- `git`: `merge` and `worktree-prune`.
- `ci`: The recipes that only the CI workflow runs, which is `install-vulkan-sdk`.
- `release`: The recipes of the release procedure that build the hosted documentation, publish, tag, or yank, which are `doc-site`, `publish-dry-run`, `publish`, `tag-release`, and `yank` (Section 9.6).

The recipe `default` has no group. Section 14.1 lists every recipe that takes parameters.

Besides the variables of the vendored recipes and `slang_library_path`, the `justfile` holds two variables of the release: `release_packages`, the crates of the next release (TOOL-35), and `documentation_site`, the base address of the hosted documentation (Section 9.4).

TOOL-6. An agent MUST add each recipe of Section 3.2 with the name, the group, and the membership of `check` that the table gives, and MUST update the table in the same change when one of them has to differ. Agents, hooks, and CI call recipes by name, and a recipe that differs from this table breaks every caller that follows it.

TOOL-7. A recipe that runs a command of Section 4.8 of the coding standard MUST run that command as Section 4.8 gives it, and MUST add only these three things:

1. The option `--profile` with a nextest profile of Section 4.
2. The arguments that Section 4.8 names for the recipe `coverage`.
3. In a recipe that runs compiled code which loads Slang, the library path assignment of the `justfile` variable `slang_library_path` at the start of the command line, which SCS-32 of `docs/design/slang-compiler-sys.md` defines.

An agent that has to change the command MUST change Section 4.8 in the same change. Two command lines for one check diverge, and an agent that runs the documented command then gets a result different from the recipe's. The library path assignment changes only where the dynamic loader looks for the Slang shared library, not what the command checks.

TOOL-7 permits one departure, for the two vendored recipes. `test-vendored`, whose purpose is to prove the vendored build (SCS-63 of `docs/design/slang-compiler-sys.md`), MAY run the nextest command and the doctest command, and `check-vendored` MAY run the Clippy command and the documentation command, each with `-p` for every package of the `justfile` variable `vendored_packages` in place of the workspace options, and with `--features` and the value of the variable `vendored_features`.

The departure also covers one more command in each recipe, which checks the feature `vendored` of a crate above `slang-compiler` on its own. `test-vendored` MAY then run the nextest command with `-p slango-macros --features slango-macros/vendored --profile default --no-tests=fail -E 'test(/^(bindings|structs)::/)'`, and `check-vendored` MAY then run the Clippy command with `-p slang-slint --features slang-slint/vendored`.

Both recipes MAY remove `VULKAN_SDK`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` with `env -u` and set `CARGO_TARGET_DIR=target/vendored`, and `test-vendored` MAY pass `--no-tests=fail` to nextest. Without these, a dynamically linked test binary could still find the Slang of the SDK, the vendored build would replace the dynamic build in `target/`, and a selection that lost its tests would pass. The two recipes share one target directory, so the second recipe reuses the build of Slang of the first.

The `justfile` holds the packages once, in `vendored_packages`, the feature once, in `vendored_features`, and the environment once, in `vendored_environment`, and both recipes read them, so their selection, feature, and environment cannot differ. The two recipes are shebang recipes that read `vendored_packages` into a bash array, because a list interpolated into a command line breaks `shellcheck` with SC2086.

`vendored_features` holds `slang-compiler/vendored`, the feature of the highest crate that the main commands select, because that crate forwards it to `slang-compiler-sys` and enables `slang-glslang-sys`. `vendored_packages` holds `slang-src slang-glslang-sys slang-compiler-sys slang-compiler`, the crates that the feature builds with the vendored route and whose tests prove it.

Each of the two extra commands proves the forward of one crate above `slang-compiler`, so it selects only that crate and enables only its feature. Within one invocation, Cargo unifies the features of every selected package, and when the package under test is a procedural macro, it also unifies the features of its dependencies with those of its development dependencies. A feature of one crate would then mask a broken forward of another, so adding these crates to `vendored_packages` would prove nothing for each.

- The command of `test-vendored` enables only `slango-macros/vendored`. When the forward of `slango-macros`, `slango-codegen`, `slango`, or `slang-compiler` is broken, the build of `slang-compiler-sys` takes the dynamic route and stops, because the recipe removes `VULKAN_SDK`. The tests that pass expand the macro with a static Slang and no Vulkan SDK. The filter selects the modules `bindings` and `structs` and leaves out the trybuild case of `ffi`, which cannot run on the vendored route (Section 10 of `docs/design/slango-macros.md`), and the tests of the `gpu` tier.
- The command of `check-vendored` enables only `slang-slint/vendored`, which fails in the same way when the forward of `slang-slint` is broken. A Cargo feature behaves the same on every platform, so the run on Linux in CI suffices.

Neither command proves that Cargo keeps the features of a procedural macro on the host apart from those of the program in a crate that depends on both, and neither covers the forward of the examples package.

TOOL-41 permits one more departure, for `doc-site`, which runs the documentation command on the vendored route for the crates of `release_packages` (Section 9.4).

TOOL-32. The workspace manifest MUST list every member except `slang-src`, `slang-glslang-sys`, `slang-rhi-sys`, and `slang-rhi` in `default-members`, and every command of Section 4.8 of the coding standard that passes `--workspace` MUST also pass `--exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi`. The build scripts of these crates, or of their dependencies, clone Slang and build glslang or Slang and slang-rhi from source, which only a vendored build needs (Section 3.1 of the architecture design). `--workspace` ignores `default-members`, so the exclusions keep them out of the recipes, and `default-members` keeps them out of a plain `cargo build` in the root of the workspace.

`slang-rhi-sys` always builds Slang and slang-rhi from source (Section 4.2 of `docs/design/slang-rhi-sys.md`), and so does `slang-rhi` through it (RHI-1 of `docs/design/slang-rhi.md`). The exclusion of both covers every command of this rule, including that of `check-semver`. Only the recipes of TOOL-52 build them.

TOOL-52. The `justfile` MUST hold the packages of the crates that build slang-rhi once, in the variable `rhi_packages`, and MUST have the recipes `check-rhi` and `test-rhi`, which run with the value of `vendored_environment` in `target/vendored`, select each package of `rhi_packages` with `-p`, and name no feature. `check-rhi` runs the Clippy command with `--all-targets` and then the documentation command with `--no-deps`. `test-rhi` runs the nextest command with the profile `gpu` and `--no-tests=fail`, and then the doctest command. The recipe `check-miri-rhi` runs the Miri command for `slang-rhi` alone, without the loader path variables, in the target directory `target/miri-rhi`. `check-rhi` and `test-rhi` run on Linux and on x86_64 Windows, where the crates build (RSYS-13 of `docs/design/slang-rhi-sys.md`), and on any other host fail with a message that says that the packages build only on Linux and on x86_64 Windows. `check-miri-rhi` runs on Linux only, and on any other host fails with a message that says that `slang-rhi` builds only on Linux.

`rhi_packages` holds `slang-rhi-sys slang-rhi`. The crates need no feature, because `slang-rhi-sys` always enables `vendored` on `slang-compiler-sys` (RSYS-1 of `docs/design/slang-rhi-sys.md`).

The crates do not build on macOS, and they need an adapter for their tests, so they get recipes of their own instead of joining `vendored_packages`, whose recipes run on every platform. On Windows, `test-rhi` needs `dxcompiler.dll` on `PATH`, through which the tests compile their kernels to DXIL (Section 7.3 of `docs/design/slang-rhi-sys.md`), and its help comment says so. `check-rhi` and `test-rhi` share `target/vendored` with the vendored recipes, so a run after `test-vendored` reuses its clone and its build of Slang. This is a departure of TOOL-7 of the same kind as that of the vendored recipes.

`check-miri-rhi` exists because `check-miri` excludes `slang-rhi` (TOOL-32), whose unit tests test its unsafe code against a fake (RHI-16 of `docs/design/slang-rhi.md`). Miri builds into the subdirectory `miri` of the target directory, so in `target/vendored` its build would leave a second clone of Slang under `target/vendored/miri`, which `check-vendored-shared-folder` refuses. The recipe therefore uses a target directory of its own, outside `target/vendored`, with its own clone and native build of Slang. It runs outside `just check`, and CI does not run it (RHQ-9 of `docs/design/slang-rhi.md`).

The three recipes need what a vendored build needs (SRC-20 of `docs/design/slang-src.md`) and, on Linux, the development headers of Xlib, such as those of the Debian package `libx11-dev`, which the Vulkan backend of slang-rhi includes on Linux (Section 7.1 of `docs/design/slang-rhi-sys.md`).

TOOL-53. After the commands of TOOL-52, `check-rhi` and `test-rhi` MUST check the backend `rhi` of `slango` (Section 13.7 of `docs/design/slango.md`) with these commands, each with the value of `vendored_environment`, each selecting one package with `-p`, and each with `--no-default-features` and the feature `rhi` of that package, so that no command builds the feature `wgpu` beside `rhi`:

| Recipe | Package | Command after `vendored_environment` | What it proves | Code step that adds it |
| --- | --- | --- | --- | --- |
| `check-rhi` | `slango` | `cargo clippy -p slango --no-default-features --features rhi --all-targets` | The build with the backend `rhi` | 3 |
| `check-rhi` | `slango` | `cargo doc -p slango --no-deps --no-default-features --features rhi` | The documentation of that build | 3 |
| `check-rhi` | The examples package | `cargo clippy -p slang-rust-examples --no-default-features --features rhi,vendored --all-targets` | The examples, their tests, and the generated code on the backend `rhi`, with the macro's copy of Slang on the vendored route | 4 |
| `test-rhi` | `slango` | `cargo nextest run -p slango --no-default-features --features rhi --profile gpu --no-tests=fail --no-fail-fast` | Every test of `slango` on the backend `rhi`, apart from the skips of SLO-43 | 3 |
| `test-rhi` | `slango` | `cargo test --doc -p slango --no-default-features --features rhi --no-fail-fast` | The doctests, with the `compile_fail` cases of SLO-17 | 3 |
| `test-rhi` | The examples package | `cargo nextest run -p slang-rust-examples --no-default-features --features rhi,vendored --profile gpu --no-tests=fail --no-fail-fast` | `compute-kernel`, `typed-kernel`, the generated code of the macro, and the scenes of `windowed-renderer` and `shader-playground` on the backend `rhi` | 4 |

The column of code steps refers to Section 15.13 of the architecture design. Each command arrives in the step whose work makes it pass. The commands pass no filter expression. The tests of textures, samplers, and rendering, and the scenes of both windowed examples, carry no skip of SLO-43 under the feature `rhi`. The skips that remain name a limit of slang-rhi on one graphics API (Section 13.7 of `docs/design/slango.md`). A skip reason names the limit and no step, so no test source names a step.

The nextest and doctest commands of `test-rhi`, those of TOOL-52 included, pass `--no-fail-fast`, so each runs all its tests after a failure. The recipe runs every command even after one fails, then names each failed command on standard error and exits with status 1, so that one CI run on Windows reports every failure of the recipe.

The environment removes `VULKAN_SDK`, so every command that builds the macro enables `vendored` for the macro's copy of Slang, as `test-vendored` does (TOOL-7). The feature `vendored` of the examples package forwards to `slango-macros`, whose copy of `slango` keeps `wgpu` as a host dependency (Section 4.3 of `docs/design/slango-macros.md`).

No command tests `slango-macros` with the backend `rhi`. When `slango-macros` is the package under test, Cargo unifies the features of the macro's copy of `slango` with those of its development dependency `slango`, so a test build with `slango/rhi` would give one copy both features and stop with the error of SLO-29 of `docs/design/slango.md`. The examples package runs the generated code on the backend `rhi` instead (Section 10 of `docs/design/slango-macros.md`).

These commands are a departure of TOOL-7 of the kind that TOOL-52 takes. Each selects one package with its own features instead of joining `rhi_packages`, because Cargo unifies the features of the packages of one command, so `slango` would be built with `wgpu` beside a package that names it, which SLO-29 stops. The commands run on Linux and on x86_64 Windows only, as the commands of TOOL-52 do, and `just check`, `test-gpu`, and every other recipe keep the default features, so they build no slang-rhi.

TOOL-54. From the code step of M10 that gives `slango` its optional dependency on `slang-rhi` until the release of M11 publishes `slang-rhi`, `slango`, `slango-codegen`, and `slango-macros` MUST set `publish = false`, and `release_packages` MUST leave them out. M11 removes both, in the order of `release_packages`, after `slang-rhi`.

Cargo refuses to package a crate whose dependency, optional or not, names a crate that is not on crates.io, with "no matching package named ... found", for `cargo package --no-verify` and for `cargo publish --dry-run` alike (run in a scratch workspace on 2026-11-18). `slango` depends on the unpublished `slang-rhi` by path and exact version (SLO-33 of `docs/design/slango.md`), `slango-codegen` depends on `slango`, and `slango-macros` on `slango-codegen`, so all three fail to package.

Until M11, `release_packages` holds the five crates `slang-bindgen slang-src slang-compiler-sys slang-glslang-sys slang-compiler`. `check-package` compares `release_packages` with the members without `publish = false`, and `publish-dry-run` packages the crates of `release_packages`. These two changes therefore keep both recipes passing for the other five crates without a change of their scripts, which is the simplest mechanism.

On 2026-11-18, `check-package` packaged the five crates with `slango` set to `publish = false`. Until M11, no alpha of `slango`, `slango-codegen`, or `slango-macros` can be published.

`doc-site` builds the crates of `release_packages`, so it leaves the three out as well. It passes `cargo doc` the feature `vendored` of each selected package that declares one, which it reads from `cargo metadata`, because a fixed list of features failed once `slango-macros` left `release_packages`: Cargo refuses a feature of a package that the command does not select (Section 9.4).

The workflow of Section 7.5 deploys the site that `doc-site` builds as a whole, so a release before M11 would replace the hosted site with one that holds the five crates only, and the pages of `slango`, `slango-codegen`, and `slango-macros` of `0.1.0-alpha.1`, which their `documentation` fields and READMEs name, would no longer answer. `check-release` would then fail for no crate of that release, because it checks the crates of `release_packages` only. TQ-19 asks how a release before M11 keeps those pages.

`check-semver` passes `--workspace` with `--default-features` (TOOL-38), so it never enables `rhi`. cargo-semver-checks skips the members with `publish = false` by itself in that mode: on 2026-11-18, `check-semver` checked only `slang-bindgen`, `slang-compiler`, and `slang-compiler-sys`, so the recipe needs no `--exclude` for the three crates. This confirmed the former assumption TA-26.

The check was meaningless for them anyway while every release is an alpha (Section 9.3), and version 0.2 of `slango` breaks its API on purpose (Section 17.6 of `docs/design/slango.md`). The release of M11 decides how the crates of the second release are checked (Section 15.14 of the architecture design).

`check-msrv` keeps the default features of every package, so it builds no slang-rhi. `check-deny` and `check-unused-deps` need no change:

- `slang-rhi` is a member already, so the optional dependency adds no crate to the graph that `deny.toml` judges.
- cargo-machete finds `slang_rhi` named in the backend module of `slango`, and the optional dependencies of the examples package named in their sources.

A command names only packages and features that the workspace has, because Cargo and cargo-hack warn about or refuse an argument that names a missing package or feature.

No command of Section 4.8 passes `--all-features`, because it would turn on the feature `vendored` and build Slang from source in every check (SCS-50 of `docs/design/slang-compiler-sys.md`). A command names each feature that it enables with `--features`.

The workspace has two Cargo features: `vendored`, off by default in every crate that has it, and `wgpu`, the default feature of `slango` and of the examples package (SLO-28 and Section 13.7 of `docs/design/slango.md`). The commands name no feature and build the default features of each crate, so they build `slango` with `wgpu` and never with `vendored`. The feature powerset of `check-features` leaves `vendored` out with `--exclude-features vendored`. A later feature that the checks have to build is added by name to the commands of Section 4.8.

TOOL-48. `check-features` MUST check every combination of the features of each package, except the combinations of `slango` and of the examples package without `wgpu` and every combination that holds `vendored` or `rhi`. The crate root of `slango` stops the compilation on purpose with one `compile_error!` when neither `wgpu` nor `rhi` is on, and so does any build with both (SLO-29 of `docs/design/slango.md`).

The powerset of cargo-hack includes the empty combination, and from M10 the examples package builds `slango` without default features, so its empty combination stops the same way and the recipe would always fail.

The second command of the recipe, which checks `slango` and the examples package, passes `--exclude-features vendored,rhi`, and the first command passes `--exclude-features vendored` only, because none of its packages has the feature `rhi`. Since the fourth code step of M10, both packages of the second command have the feature `rhi`, and on 2026-11-19 that command checked four combinations of the examples package with `rhi` left out (run). Whether cargo-hack would accept `rhi` on the first command was not tried. The second command leaves `rhi` out for these reasons:

- The feature `rhi` of `slango` and of the examples package builds Slang and slang-rhi from source, which only the recipes of TOOL-52 and TOOL-53 do.
- The CI job `ci-only`, which runs `check-features`, has none of the tools of a vendored build.
- Every combination with `rhi` beside `wgpu` stops with the error of SLO-29.

Leaving `rhi` out of the powerset is therefore enough, and the recipe needs no `--mutually-exclusive-features`. The build with `rhi` alone is what `check-rhi` checks instead (TOOL-53).

Section 4.8 of the coding standard records both commands, as TOOL-7 requires.

The second command checks `slango` and the examples package with `--exclude-no-default-features`, and the first command excludes both. The recipe runs two commands, which Section 4.8 of the coding standard also records:

```sh
CARGO_BUILD_WARNINGS=deny cargo hack check --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --exclude slango --exclude slang-rust-examples --all-targets --feature-powerset --exclude-features vendored
CARGO_BUILD_WARNINGS=deny cargo hack check -p slango -p slang-rust-examples --all-targets --feature-powerset --exclude-features vendored --exclude-no-default-features
```

- The first checks every package of the workspace except `slango` and the examples package. Every other member that depends on `slango` names `wgpu` in that dependency (SLO-30 of `docs/design/slango.md`), so each combination of its own features builds `slango` with the feature.
- The second checks `slango` and the examples package. `--exclude-no-default-features` leaves out exactly the combination with no features, which is the only combination of either package without `wgpu`, because cargo-hack lists `default` as a feature of the powerset and `default` enables `wgpu` (Section 3.10 of `docs/design/slango.md`). For `slango`, it checks `--features wgpu` and `--features default`, each after `--no-default-features`. On 2026-11-18, it checked the examples package with `--no-default-features --features wgpu` and with `--features default`.

The option `--at-least-one-of` of cargo-hack does not serve here. cargo-hack 0.6.46 rejects `--at-least-one-of wgpu`, on the workspace and with `-p slango`, before it checks any package, with "error: --at-least-one-of requires a list of two or more features separated by space or comma". With two features, such as `--at-least-one-of default,wgpu` on the workspace, it exits with status 0 but skips every package that lacks them. With `-p slango`, `--at-least-one-of default,wgpu` gives the same two combinations as `--exclude-no-default-features`. It would also leave out a combination with `rhi` alone, which a later backend adds, so the recipe uses `--exclude-no-default-features`.

Rust 1.100.0 makes Cargo warn about a dependency that a build does not use. Under `CARGO_BUILD_WARNINGS=deny`, `check-features` would then fail on a dependency that one combination of features leaves unused. A run with Rust 1.100.0 found none.

The formatting command of Section 4.8 ends with the list of every Rust file that git tracks or does not ignore. `fmt` and `check-fmt` compute that list with `git ls-files`, and it is part of the command, not an addition.

TOOL-8. The recipe `check` MUST pass in the development container without a hardware graphics processing unit (GPU) and without a display, and MUST NOT run a recipe that needs either. An agent runs `check` before every merge in the container, as the Validating Changes section of `AGENTS.md` requires, and CI runs it on a hosted runner that is not required to provide either.

TOOL-9. An agent MUST NOT add a recipe for a tool that the container image does not provide until an image rebuild adds the tool, because the recipe would fail `check` for every agent until then.

TOOL-30. `taplo.toml` MUST set `indent_string = "    "` in its `[formatting]` table and `exclude = ["target/**", ".worktrees/**"]`. `typos.toml` MUST accept each correct word that typos reports in the repository.

Otherwise `check-toml` and `check-typos` fail on files that are correct. taplo indents with two spaces by default, which disagrees with the four spaces that `.editorconfig` sets for `*.toml`. It does not read `.gitignore`, so without the exclusions it also checks the build directory and every worktree.

typos reports two kinds of correct text in the repository: a two-letter fragment of the regular expression that matches the shell names in the shebang checks of `scripts/justfile-recipes` and `scripts/check-shell`, and the surname of a cited author in `docs/rust-references.md`. `typos.toml` accepts them with two entries in the `extend-ignore-re` list of its `[default]` table. One matches the whole regular expression, and the other matches the author's full name.

typos skips only the text that an entry matches, so it still checks the same letters anywhere else, which accepting the words in `[default.extend-words]` would not do. A document that names this configuration describes the two findings instead of spelling them out, because typos reports such a document too. typos reads its configuration from `typos.toml`, `_typos.toml`, or `.typos.toml`, and the repository uses the first name.

The generated bindings are written under `OUT_DIR`, so typos does not read them. `typos.toml` has an entry in a `[default.extend-identifiers]` table only for a generated identifier that a file of the repository names and that typos reports, because each generated name has to match the header. It holds six such identifiers:

- The three misspelled identifiers of the recorded Slang. Section 5.10 of `docs/design/slang-compiler-sys.md` names them and says why an entry for a whole identifier is the narrowest exclusion.
- The three correct field names `maxLOD`, `minLOD`, and `mipLODBias` of the `SamplerDesc` of slang-rhi, which `slang-rhi` writes (RHI-24 of `docs/design/slang-rhi.md`). typos reports the abbreviation of the level of detail in them as a misspelling.

No file is excluded from typos.

`typos.toml` sets no locale. typos cannot enforce Canadian spelling, because with its default settings it accepts both `colour` and `color`, so `scripts/check-prose` and its word list keep that job (Section 8 of the writing guide).

The repository has no `rustfmt.toml`, because Section 4.8 of the coding standard specifies the default rustfmt settings.

### 3.2. Recipe Table

This table lists every recipe, what it runs, and whether `check` runs it. The commands of Section 4.8 of the coding standard are named by their purpose, and Section 4.8 gives each command line.

| Recipe | Group | Runs | In `check` |
| --- | --- | --- | --- |
| `default` | none | `just --list --unsorted` | No |
| `check` | `checks` | The recipes that Section 3.3 lists | Not applicable |
| `check-prose` | `checks` | `scripts/check-prose`, extended to API documentation by Section 6.6 | Yes |
| `check-prose-self-test` | `checks` | The self-test of Section 6.1 for `scripts/check-prose` | Yes |
| `check-shell` | `checks` | `scripts/check-shell` | Yes |
| `check-commit-message` | `checks` | `scripts/check-commit-message` on one file, for the `commit-msg` hook | No |
| `check-commit-message-self-test` | `checks` | The self-test of the commit message checker | Yes |
| `hooks-install` | `checks` | `pre-commit install` | No |
| `merge` | `git` | `scripts/merge-branch`, which squash-merges a branch onto `main` with the subject and body of every commit of the branch in the message, pushes `main`, and deletes the branch on `origin` when its tip there is part of the squash (Section 14.7) | No |
| `worktree-prune` | `git` | `scripts/worktree-prune` | No |
| `fmt` | `rust` | `cargo fmt --all` with the list of every Rust file that git tracks or does not ignore after `--`, which applies the formatting that the formatting command of Section 4.8 checks, also to the files that a crate includes with `include!`, and `taplo fmt`, which formats every Tom's Obvious Minimal Language (TOML) file that `taplo.toml` does not exclude. It lists the files with `git ls-files -z --cached --others --exclude-standard -- '*.rs'` and keeps those that exist. It fails with "fmt: git ls-files failed, so the Rust files that a crate pulls in with include! cannot be listed." when `git ls-files` fails, and with "fmt: git ls-files listed no Rust file, so the Rust files that a crate pulls in with include! cannot be listed." when the list is empty | No |
| `check-rust` | `checks` | The recipes that Section 3.3 lists | Yes |
| `check-fmt` | `checks` | The formatting command of Section 4.8, with the list of files that `fmt` computes in the same way. It fails with the messages of `fmt`, which start with "check-fmt:" instead, when `git ls-files` fails or lists no Rust file | Yes, through `check-rust` |
| `check-clippy` | `checks` | The Clippy command of Section 4.8 | Yes, through `check-rust` |
| `check-doc` | `checks` | The documentation command of Section 4.8 | Yes, through `check-rust` |
| `check-doctest` | `checks` | The doctest command of Section 4.8, after the library path assignment of SCS-32 of `docs/design/slang-compiler-sys.md` | Yes, through `check-rust` |
| `test` | `rust` | The nextest command of Section 4.8 with the `default` profile, after the library path assignment | No, because `test-gpu` runs the same tests |
| `test-gpu` | `rust` | The nextest command of Section 4.8 with the `gpu` profile, after the library path assignment | Yes, through `check-rust` |
| `test-hw-gpu` | `rust` | The nextest command of Section 4.8 with the `hw-gpu` profile, after the library path assignment | No, because it needs a hardware GPU |
| `check-deny` | `checks` | The cargo-deny command of Section 4.8 | Yes, through `check-rust` |
| `check-rust-comments` | `checks` | The comment checker of Section 6.2 | Yes, through `check-rust` |
| `check-doc-headings` | `checks` | The heading checker of Section 6.3 | Yes, through `check-rust` |
| `check-doctest-attributes` | `checks` | The doctest attribute checker of Section 6.4 | Yes, through `check-rust` |
| `check-feature-docs` | `checks` | The feature documentation checker of Section 6.5 | Yes, through `check-rust` |
| `check-rust-comments-self-test`, `check-doc-headings-self-test`, `check-doctest-attributes-self-test`, `check-feature-docs-self-test` | `checks` | The self-test of Section 6.1 for each checker of Sections 6.2 to 6.5 | Yes |
| `check-slang-call-guard` | `checks` | The binary of `slang-bindgen`, through `cargo run --quiet -p slang-bindgen`, on `$VULKAN_SDK/include/slang` for the target of the host into `target/slang-bindgen/`, and then the Slang call guard checker of Section 6.9 (SCS-60 of `docs/design/slang-compiler-sys.md`). It needs `VULKAN_SDK` and libclang | Yes, through `check-rust`, directly after `check-feature-docs` |
| `check-slang-call-guard-self-test` | `checks` | The self-test of Section 6.1 for the Slang call guard checker | Yes |
| `check-workflow-pins` | `checks` | The workflow pin checker of Section 6.8 | Yes |
| `check-workflow-pins-self-test` | `checks` | The self-test of Section 6.1 for the workflow pin checker | Yes |
| `check-doc-references` | `checks` | The document reference checker of Section 6.10 | Yes, directly after `check-workflow-pins` |
| `check-doc-references-self-test` | `checks` | The self-test of Section 6.1 for the document reference checker | Yes |
| `release-scripts-self-test` | `checks` | `scripts/self-test-release`, which runs the scripts of the release recipes and the recipes `publish-dry-run`, `publish`, `tag-release`, and `doc-site` against stub programs (Section 9.9) | Yes, directly after `check-doc-references-self-test` |
| `recorded-slang` | `rust` | Prints `RECORDED_COMMIT`, `RECORDED_BUILD_TAG`, and `RECORDED_CLONE_ENTRY` of `crates/slang-src/recorded.rs` as `NAME=value` lines, in that order, and fails when a constant is missing (SRC-11 and SRC-25 of `docs/design/slang-src.md`). It runs on bash 3.2, which has no associative arrays and is the bash that a recipe gets on the hosted macOS runner. It accepts lines that end in CRLF, because a user's own git settings can produce them (Section 14.3) | No |
| `install-vulkan-sdk` | `ci` | `scripts/install-vulkan-sdk` with the SDK version and the expected SHA-256 checksum, which downloads, verifies, and installs the Vulkan SDK on a GitHub Actions runner and exports it to the later steps of the job (Section 7.3). Only the CI workflow runs it, and it refuses to run outside a runner | No, because it runs only on a CI runner |
| `check-vulkan-sdk` | `checks` | `scripts/check-vulkan-sdk`, without arguments, which reads `RECORDED_BUILD_TAG` with `just recorded-slang` and checks that the SDK at `VULKAN_SDK` provides that build of Slang (Section 7.3) | No, every CI job that installs the SDK runs it |
| `test-vendored` | `rust` | The nextest command of Section 4.8 with the `default` profile and then the doctest command, with the departure that TOOL-7 permits, as the value of `vendored_environment` followed by `cargo nextest run`, `-p` with each package of `vendored_packages`, `--features` with the value of `vendored_features`, and `--profile default --no-tests=fail`, and the same environment, selection, and feature with `cargo test --doc` (SCS-63 of `docs/design/slang-compiler-sys.md`). The first command is `env -u VULKAN_SDK -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH CARGO_TARGET_DIR=target/vendored CARGO_BUILD_WARNINGS=deny cargo nextest run -p slang-src -p slang-glslang-sys -p slang-compiler-sys -p slang-compiler --features slang-compiler/vendored --profile default --no-tests=fail`. A third command, with the same environment, runs `cargo nextest run -p slango-macros --features slango-macros/vendored --profile default --no-tests=fail -E 'test(/^(bindings|structs)::/)'`, which checks the forward of the feature from `slango-macros` down to `slang-compiler` (TOOL-7). Its first run in `target/vendored` clones the recorded commit, which needs network access, and builds Slang and then slang-glslang from source, which takes several minutes. Later runs use the clone and the built libraries in the shared folder `target/vendored/slang-src` without network access while it exists (Section 9 of `docs/design/slang-src.md`). The tests pass only when nothing loads the Slang of the SDK. The doctests of `slang-src` and `slang-glslang-sys` run only here, because every other recipe excludes the two crates (TOOL-32) | No, because it clones and builds Slang from source |
| `check-vendored` | `checks` | The Clippy command and then the documentation command of Section 4.8, with the departure that TOOL-7 permits, for the same packages, feature, environment, and target directory as `test-vendored`, as the value of `vendored_environment` followed by `cargo clippy`, the packages and the feature of `test-vendored`, and `--all-targets`, and the same with `cargo doc` and `--no-deps`. The first command is `env -u VULKAN_SDK -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH CARGO_TARGET_DIR=target/vendored CARGO_BUILD_WARNINGS=deny cargo clippy -p slang-src -p slang-glslang-sys -p slang-compiler-sys -p slang-compiler --features slang-compiler/vendored --all-targets`. A third command, with the same environment, runs `cargo clippy -p slang-slint --features slang-slint/vendored`, which checks the forward of the feature from `slang-slint` down to `slang-compiler` (TOOL-7). It applies the lints and the rustdoc lints to `slang-src` and `slang-glslang-sys`, which every other recipe excludes (TOOL-32), and to the code of the other two crates under the feature `vendored` | No, because it clones and builds Slang from source. It shares the shared folder of `target/vendored` with `test-vendored`, so the first of the two to run needs network access and builds Slang. The `vendored` CI job on Linux runs it (Section 7.4) |
| `check-rhi` | `checks` | The Clippy command and the documentation command for the packages of `rhi_packages` on the vendored route, on Linux and on x86_64 Windows (TOOL-52), and then the commands of `check-rhi` of TOOL-53 for the backend `rhi` of `slango` and Clippy on the examples package with its features `rhi` and `vendored`. Its first run in `target/vendored` clones Slang and builds Slang and slang-rhi from source, and it needs the tools of a vendored build and, on Linux, the development headers of Xlib | No, because it builds Slang and slang-rhi from source. The `vendored` CI job on Linux and on Windows runs it (Section 7.4) |
| `test-rhi` | `rust` | The nextest command with the profile `gpu` and the doctest command for the packages of `rhi_packages` on the vendored route, on Linux and on x86_64 Windows (TOOL-52), and then the commands of `test-rhi` of TOOL-53, which run the tests and doctests of `slango` and the tests of the examples package with its features `rhi` and `vendored`, among them the `gpu` tests of the generated code of `slango-macros` and the scenes of both windowed examples, on the backend `rhi`, with no filter. Each command runs all its tests after a failure, and the recipe runs every command after a failing one and then fails, naming each failed command (TOOL-53). It needs what `check-rhi` needs, and on Windows `dxcompiler.dll` on `PATH` | No, because it builds Slang and slang-rhi from source and needs an adapter. The `vendored` CI job runs it on lavapipe on Linux and on WARP on Windows (Section 7.4) |
| `check-vendored-shared-folder` | `checks` | A shebang recipe that checks what `test-vendored` or `check-vendored` left in `target/vendored`. It reads `RECORDED_CLONE_ENTRY` with `just recorded-slang` and checks that Cargo tagged `target/vendored` with `CACHEDIR.TAG`. A clone entry is a directory that holds `tree/.git` in one of three layouts, all in lowercase hexadecimal digits: `src/<prefix>-<submodules>` of SRC-25 of `docs/design/slang-src.md`, with the first 23 digits of the commit and a hash of 16, and the earlier layouts `src/<commit>-<submodules>` and `src/<commit>`, with a full commit hash of 40. The recipe prints each clone entry of the shared folder `target/vendored/slang-src`, and checks that the folder holds exactly one, `src/<RECORDED_CLONE_ENTRY>`, with its stamp file, and that no other directory named `slang-src` under `target/vendored` holds a clone entry of any of these layouts. A directory named `slang-src` with a `src` directory alone is no clone, because CMake mirrors the absolute path of each source outside the wrapper project of `slang-glslang-sys` under `CMakeFiles/<target>.dir/` of its build tree, so that tree holds such directories. It prints the tag, the absolute path of the shared folder, the clone entry, and the sizes of the folder and of the clone entry. It fails when `recorded-slang` prints no `RECORDED_CLONE_ENTRY`, on a missing tag, on a missing, incomplete, or extra clone entry in the shared folder, which includes a clone of an earlier layout, on a clone entry in another such directory, or on a failure of `find`. It builds nothing and reaches no network. Its check of the location shows on each platform that the build script found the shared folder through the tag of a target directory that Cargo created (Section 3.2 of `docs/design/slang-src.md`). It does not exercise `--target`, a separate build directory, or two build scripts that run at once, which SRCA-9 of that design leaves open | No, because it reads the result of a vendored recipe. The `vendored` CI job runs it after `test-vendored` on each platform (Section 7.4) |
| `check-typos` | `checks` | The typos command of Section 4.8 on the whole repository, with `typos.toml` (TOOL-30) | Yes |
| `check-toml` | `checks` | The taplo command of Section 4.8, with `taplo.toml` (TOOL-30) | Yes |
| `check-workflows` | `checks` | The actionlint command of Section 4.8, which checks every workflow | Yes |
| `check-unused-deps` | `checks` | The cargo machete command of Section 4.8 | Yes, through `check-rust` |
| `check-miri` | `checks` | The Miri command of Section 4.8, for the tests that the nextest profile `default-miri` selects | Yes, through `check-rust` |
| `check-miri-rhi` | `checks` | `env -u VULKAN_SDK -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH CARGO_TARGET_DIR=target/miri-rhi cargo +nightly miri nextest run -p slang-rhi --no-tests=fail`, on Linux only (TOOL-52). Under Miri, nextest selects the profile `default-miri`. The build scripts run natively, so its first run in `target/miri-rhi` clones Slang into `target/miri-rhi/miri/slang-src` and builds Slang, slang-glslang, and slang-rhi from source, and it needs the tools of a vendored build, the development headers of Xlib, and the nightly toolchain | No, because it builds Slang and slang-rhi from source. CI does not run it (RHQ-9 of `docs/design/slang-rhi.md`) |
| `check-features` | `checks` | The two cargo hack commands of Section 4.8 for the feature powerset, which includes each single feature and leaves out `vendored` (TOOL-32, TOOL-48). The first checks every package except `slango` and the examples package, and the second checks those with `--exclude-features vendored,rhi` and `--exclude-no-default-features`, which leaves out their combination without `wgpu` (TOOL-48) | No, the CI job `ci-only` runs it |
| `check-msrv` | `checks` | The MSRV command of Section 4.8, which checks the workspace with `cargo hack check` and the toolchain that `rust-version` names | No, the CI job `ci-only` runs it, and its first local run downloads that toolchain (Section 14.3) |
| `check-semver` | `checks` | cargo semver-checks with `--workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --default-features`, against the last release of each crate on crates.io (TOOL-38). It checks only the crates with a library target and skips the members with `publish = false` by itself, so until M11 it checks `slang-bindgen`, `slang-compiler`, and `slang-compiler-sys` only (TOOL-54), and while the releases are alphas it runs none of the tool's checks of the API (Section 9.3) | No, because it fetches each baseline from crates.io and builds every crate twice. The CI job `semver` runs it |
| `check-package` | `checks` | `scripts/check-package` with `documentation_site` and the crates of `release_packages`, which compares `release_packages` with the members without `publish = false`, checks the `documentation` field of each crate, prints `cargo package --list` and runs `cargo package --no-verify` for each crate of `release_packages`, and checks each `.crate` file (TOOL-45). Available on Linux and macOS only, and only on a committed tree (Section 9.6) | No, the release procedure runs it |
| `check-release` | `checks` | `scripts/check-release` with `documentation_site` and the crates of `release_packages`, which reads each crate from crates.io and checks its version, its `documentation` field, and that the address answers (Section 9.6) | No, it checks a release after the publish |
| `doc-site` | `release` | The documentation command of Section 4.8 on the vendored route for the crates of `release_packages`, with the feature `vendored` of each of them that declares one, which it reads from `cargo metadata`, and then `scripts/check-doc-site`, which checks that each crate has its index page, writes the index page of the site, and checks the site (TOOL-41). Its first run in `target/vendored` clones Slang and builds Slang and slang-glslang | No, the release procedure and the documentation workflow run it |
| `publish-dry-run` | `release` | `scripts/publish-dry-run`, which runs `cargo publish --dry-run` for the given crates, by default those of `release_packages`, and records the verified commit and crates in `target/publish-dry-run.commit` (Section 9.6). Available on Linux and macOS only, and only on a committed tree | No, the release procedure runs it |
| `publish` | `release` | `scripts/publish`, which, after the checks of Section 9.6, asks crates.io for each of the given crates, by default those of `release_packages`, skips every version that crates.io holds, and publishes each other crate with its own run of `cargo publish --no-verify`, in the given order, within the rate limits of crates.io (TOOL-51). Only the owner runs it, with a token (TOOL-46). Available on Linux and macOS only | No, it publishes |
| `tag-release` | `release` | `scripts/tag-release` with the notes file and the crates of `release_packages`, which creates and pushes the tag `v<version>` and creates the GitHub release with the given notes, marked as a pre-release when the version holds a hyphen (Section 9.6). It needs the owner's go-ahead | No, it tags and deploys |
| `yank` | `release` | `scripts/yank`, which runs `cargo yank` of one version of one crate (Section 9.8). Only the owner runs it, with a token (TOOL-46) | No, it changes crates.io |
| `coverage` | `rust` | The cargo llvm-cov command of Section 4.8 with the arguments that Section 4.8 names for it, after the library path assignment, which write the LCOV report `target/lcov.info`. It prints no coverage summary, and it is not a gate | No, the CI job `coverage` runs it (Section 7.1) |

`check-miri` runs Miri through nextest, for the reason that Section 4.8 of the coding standard gives. Under Miri, nextest uses the profile named `default-miri`, which `.config/nextest.toml` defines (Section 4).

Miri emulates some functions of libc and libm, but it cannot call an arbitrary native library such as Slang. `check-miri` therefore covers pure Rust logic, such as marshalling and layout code, and the unsafe code that TEST-7 of the coding standard tests against a fake implementation.

The filter of `default-miri` excludes every test whose path contains a module named `ffi`, `gpu`, or `hw_gpu`, selected by whole module path segments as in Section 4. The tests that call Slang or libclang sit in modules named `ffi` (SCS-33 of `docs/design/slang-compiler-sys.md`), and so do the tests that start a process, which Miri cannot do.

`check-miri` leaves `slang-src`, `slang-glslang-sys`, `slang-rhi-sys`, and `slang-rhi` out (TOOL-32), so no recipe runs the pure tests of the first three under Miri (SRCQ-3 of `docs/design/slang-src.md`). `check-miri-rhi` runs those of `slang-rhi` (TOOL-52).

The Cargo feature `vendored` selects the vendored build of Slang (SCS-50 of `docs/design/slang-compiler-sys.md`). Among the recipes, `test-vendored`, `check-vendored`, and `doc-site` enable it. `publish-dry-run` enables no feature, but it verifies `slang-src` and `slang-glslang-sys`, which depends on `slang-compiler-sys` with the feature, so it also clones and builds Slang (Section 9.6). `check-rhi`, `test-rhi`, and `check-miri-rhi` enable no feature either, and build `slang-rhi-sys`, which enables it, so they clone and build Slang and slang-rhi (TOOL-52). The other recipes never clone or build Slang from source (TOOL-32).

The library path assignment is the value of the `justfile` variable `slang_library_path`, which SCS-32 of `docs/design/slang-compiler-sys.md` defines. It sets `LD_LIBRARY_PATH` on Linux and `DYLD_LIBRARY_PATH` on macOS to `$VULKAN_SDK/lib` followed by any earlier value. It is empty on Windows and when `VULKAN_SDK` is unset or empty.

Section 10 of `docs/design/slang-compiler-sys.md` records why the assignment is on the command line and not exported.

The recipes that run compiled code which loads Slang put the assignment at the start of their command line: `check-doctest`, `test`, `test-gpu`, `test-hw-gpu`, and `coverage`. These recipes do not carry it:

- `check-miri` and `check-miri-rhi`, which run no native code in their tests.
- The recipes that only compile, such as `check-clippy`, `check-doc`, `check-features`, and `check-msrv`.
- `check-slang-call-guard`, whose run of the binary of `slang-bindgen` loads libclang and not Slang.
- `test-vendored` and `check-vendored`, which remove the loader path variables to prove the static link (TOOL-7), and `doc-site`, which runs no compiled code and removes them in the same way (TOOL-41).

### 3.3. Aggregate Recipes

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
11. `check-doc-references-self-test`
12. `release-scripts-self-test`
13. `check-typos`
14. `check-toml`
15. `check-workflows`
16. `check-workflow-pins`
17. `check-doc-references`
18. `check-rust`

`check-workflows` can run in `check` only while the repository has a workflow, because actionlint exits with status 3 when it finds none. `check` needs network access (Section 14.6).

`check-rust` runs `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs`, `check-slang-call-guard`, `check-fmt`, `check-unused-deps`, `check-deny`, `check-clippy`, `check-doc`, `check-doctest`, `test-gpu`, and `check-miri`, in that order, so that the checks that do not compile the workspace report first. `check-slang-call-guard` compiles only `slang-bindgen`. Every recipe of `check-rust` that compiles the workspace, and `check-slang-call-guard`, needs the SDK at `VULKAN_SDK` and libclang, because the build script of `slang-compiler-sys` generates the bindings from the headers of that SDK with libclang and stops the build without either (SCS-64 of `docs/design/slang-compiler-sys.md`).

`check-rust` runs `test-gpu` and not `test`, because the `gpu` profile selects every test that the `default` profile selects (Section 4). `test-gpu` runs in the development container on lavapipe, so `check` meets TOOL-8.

On Linux, `test-gpu` also runs the tests of `slang-bindgen` that load libclang (SCS-21 of `docs/design/slang-compiler-sys.md`). The development container provides `VULKAN_SDK` and libclang, and the CI jobs install them (Section 7.1). A test in a `gpu` module that finds no adapter fails, as ARCH-23 of the architecture design requires, so `check` fails on a machine without any adapter.

## 4. Test Tiers in nextest

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

It also defines the profile `default-miri`, which nextest uses under Miri (Section 3.2), with the filter that SCS-33 of `docs/design/slang-compiler-sys.md` requires. It selects every test that `default` selects, except those whose path contains a module named `ffi`:

```toml
[profile.default-miri]
default-filter = "not test(/(^|::)(ffi|gpu|hw_gpu)::/)"
```

The regular expression matches `gpu` or `hw_gpu` only at the start of the test path or after `::`, and only when `::` follows it. The trailing `::` makes the match a whole module segment.

A test function named `gpu` is the last segment of its path and has no `::` after it, and a module named `gpu_buffers` has `_buffers` after `gpu`, so neither matches. The filter of `default-miri` matches `ffi` in the same way.

A filter that the command line adds with `-E` is intersected with the default filter of the profile, so it narrows the selection of the profile and never widens it. No recipe adds such a filter.

TOOL-11. A profile MUST NOT retry a failed test, because a retry turns an intermittent failure on an adapter into a pass, which hides the failure in the same way that ARCH-23 of the architecture design forbids a test to hide a missing adapter.

Zero retries is the default of nextest, and the `default` profile sets `retries = 0` explicitly so that the configuration states this rule instead of relying on the default. The other profiles inherit the value.

TOOL-12. The recipes `test`, `test-gpu`, and `test-hw-gpu` MUST pass when their profile selects no test. An empty selection is not a test failure, and a tier can hold no test. nextest 0.9.151 fails such a run with the message "error: no tests to run" and exit status 4.

Every nextest command of Section 4.8 of the coding standard therefore passes `--no-tests=warn`, with which an empty run passes and nextest still prints that no test ran. A recipe keeps that option, except in the departure that TOOL-7 permits.

Each `gpu` test of `slango` prints the adapter that it ran on. This override in the `default` profile keeps that output in the log, so that the log of a CI run records the adapter of each runner. It changes no filter:

```toml
[[profile.default.overrides]]
filter = "package(slango) & test(/^gpu::/)"
success-output = "final"
```

nextest does not run doctests, so `check-doctest` runs them, as Section 4.8 of the coding standard states.

## 5. Git Hooks

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
| `check-doc-references` | A staged file that the checker reads changes, which is a `.rs`, `.toml`, `.yml`, `.yaml`, or `.json` file, the `justfile`, or a file under `scripts/`. It also runs when a Markdown file under `docs/` changes, because a document can define a new rule identifier (Section 6.10) |

Each of these hooks except `check-prose` runs its recipe on the whole repository, not on the staged files only. The `pre-commit` stage also runs hooks of the repository `pre-commit-hooks`, pinned to a commit SHA, which check YAML and JSON syntax, file endings, trailing whitespace, merge conflict markers, private keys, and large files. The `commit-msg` hook runs `check-commit-message`, and the `post-merge` hook runs `worktree-prune`.

The Claude Code hook in `.claude/settings.json` runs `scripts/claude-hook-prose` after every edit. The script runs `check-prose` on the edited file when the file is inside the repository and its name ends in `.md` or `.rs`, and ignores every other file. It reports the result in one of two ways:

- When `check-prose` reports findings, the script prints them and exits with status 2, which Claude Code treats as a blocking error and reports with the findings.
- When `check-prose` fails for another reason, such as a Rust file that cannot be lexed because it is only partly edited (Section 6.1), the script prints the message and exits with status 1, which Claude Code treats as a non-blocking error.

The allow list of `.claude/settings.json` lets Claude Code run these commands without asking, each as `just <recipe>` unless shown otherwise:

- `just --list` and `just check`.
- `check-rust` and each recipe that it runs, except `check-slang-call-guard`.
- Each custom checker and its self-test, except `check-slang-call-guard` and `check-slang-call-guard-self-test`.
- `check-shell`, `check-commit-message` and its self-test, `check-typos`, `check-toml`, `check-workflows`, `check-features`, `check-vulkan-sdk`, `recorded-slang`, `test`, `test-gpu`, and `test-hw-gpu`.
- `shellcheck` and `git status`.

Claude Code asks before it runs any other recipe, such as `fmt`, `merge`, `check-msrv`, `coverage`, `test-vendored`, `check-vendored`, `check-vendored-shared-folder`, `check-slang-call-guard`, `check-slang-call-guard-self-test`, or `release-scripts-self-test`. `just check` and `just check-rust` still run those that they include without asking, because the allow list holds those two commands.

TOOL-47. The allow list MUST NOT hold a recipe that publishes, tags, deploys, or yanks, which are `publish`, `tag-release`, and `yank`, or any command that runs one of them, such as `cargo publish` or `gh release`. Each of them acts in public and needs the owner's go-ahead (Section 9.6), and an entry in the allow list would let an agent run it without asking.

The other recipes of the release procedure stay off the allow list as well, by a choice of the design: `check-semver`, `check-package`, `check-release`, `doc-site`, and `publish-dry-run`. Each of them reaches the network or builds Slang from source, as `check-vendored` does, and none of them is part of `check`.

## 6. Custom Checkers

Several rules of the writing guide, the pinning rule of Section 7.2, and SLC-32 of `docs/design/slang-compiler.md` can be checked deterministically, but no installed tool checks them. actionlint, for example, does not check the pinning rule, and no lint checks SLC-32.

This section specifies a small custom checker for each. The "Checked by" statements of Section 7 of the writing guide, its Section 7.12, and Section 7.2 of this document name these checkers.

### 6.1. Conventions

TOOL-14. Each custom checker MUST be a script under `scripts/` with a recipe of the same name in the `checks` group. It MUST follow the conventions that the usage text of `scripts/check-prose` states, which are:

- One `path:line: message` line for each finding.
- Exit status 0 without findings, 1 with findings, and 2 when a file cannot be read or the setup is broken, after checking every readable file.
- Usage text on `-h`.

With no paths, the checker checks every relevant file in the git repository, tracked or not ignored. A checker that differs breaks the hooks and the recipes, which rely on these exit statuses to tell a finding from a broken setup.

The one departure is a checker that needs an input to know what to look for, such as the generated bindings of Section 6.9 or the project documents of Section 6.10. It exits with status 2 before it checks any file when that input is missing or unusable.

A file that is not valid UTF-8 is a file that cannot be read, so a checker reports it on standard error and exits with status 2. The exception is a Markdown file given to `check-prose`, in which an invalid byte is a finding.

A Rust file that ends inside a string literal, a raw string literal, a block comment, or a character literal also cannot be read. For a checker that reads documentation comments, the same holds for a file that ends inside an attribute, because such a checker skips attributes to group the comments of one item. The checker prints `<checker>: cannot read <file>: unterminated <kind> at line N` on standard error, where N is the line of the opening delimiter, and exits with status 2.

Unterminated input is a read error and not a silent end of the file. A lexer that read the rest of the file as part of the literal would report nothing after it, and the check would pass while it checked nothing there.

Other files given in the same run are still checked. While a file is only partly edited, unterminated input is normal, and Section 5 says how the Claude Code hook reports it.

Each checker prints every finding and every error message through the helper `emit` of `scripts/rust_lexer.py`, which escapes every character outside printable ASCII, both in file paths and in file content that a message repeats. The escapes are these:

- A newline, a carriage return, and a tab print as `\n`, `\r`, and `\t`.
- Every other character up to U+00FF prints as `\xNN`.
- Every character above U+00FF prints as `\uNNNN`, or as `\UNNNNNNNN` above U+FFFF.

A checked file or a file name therefore cannot send a terminal control sequence to the terminal that shows the output (Section 13). A backslash prints unchanged, so a character that the helper escapes and the same escape written literally in a file print alike.

A checker reports a failed git call and a file that cannot be read in its own words, which do not depend on the version of git or on the operating system. The self-tests pin the exact text of each error and would otherwise fail on another machine for an unrelated reason.

A failed git call reads `not inside a git repository`, `git <subcommand> exited with status N`, or `cannot run git: <cause>`, and never repeats the message of git. A read error names its cause as `is a directory`, `no such file`, `not a directory`, or `permission denied`, and only a cause outside these four uses the description that the operating system gives. When `cargo metadata` fails, `check-feature-docs` still repeats the first error line of Cargo (Section 6.5).

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

### 6.2. Comment Checker

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

The checker does not report line 4, because the slashes there are inside a string literal. A generated file in the repository is checked like any other file. The generated bindings are written under `OUT_DIR` and contain no comment, so the checker never reads them (Section 5.10 of `docs/design/slang-compiler-sys.md`).

### 6.3. Heading Checker

This table summarizes the heading checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-doc-headings`, `check-doc-headings` |
| Rule | Section 7.3 of the writing guide |
| Reads | The `///` comments of every `.rs` file. Crate-level and module-level documentation, written with `//!`, is not read, because Section 7.4 of the writing guide gives it its own headings |
| Reports | A heading of any level that is not one of the level-one standard headings of Section 7.3 of the writing guide, including a standard heading with the wrong case or level, a standard heading out of the order that section gives, and a standard heading that appears twice in one item's documentation |

The checker reads both ATX headings, which start with `#`, and setext headings, which are underlined with `=` or `-`. The `///` comments that precede one item, with only attributes and other comments between them, are one item's documentation.

The checker ignores lines inside fenced code blocks, because a line that starts with `# ` in a Rust code block is a hidden line of a doctest, not a heading. It does not read documentation written as a `#[doc = ...]` attribute or a `/** */` block comment, which the comment checker of Section 6.2 rejects. Section 7.3 of the writing guide forbids every other heading in the documentation of an item, which is the rule that the checker enforces.

### 6.4. Doctest Attribute Checker

This table summarizes the doctest attribute checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-doctest-attributes`, `check-doctest-attributes` |
| Rule | Section 7.5 of the writing guide |
| Reads | The fenced code blocks in the `///` and `//!` comments of every `.rs` file, and in every file named `README.md`, which a crate includes as doctests (Section 7.2 of `docs/writing-readmes.md`). A path given on the command line that is not named `README.md` is read as Rust |
| Reports | A code block whose attributes include `ignore` or an attribute that starts with `ignore-`, such as `ignore-windows` |

The checker reports the attributes that start with `ignore-` as well, because Section 7.5 of the writing guide forbids `ignore` and these are variants of it that ignore the example on some targets.

The checker splits the info string of a code block into attributes the way rustdoc does. It splits at commas, spaces, and tabs, reads a double-quoted attribute without its quotes, and reads the text inside braces, such as `{.ignore}`, as a class or a key and not as an attribute. Whether a `no_run` example states its reason stays a judgement rule.

### 6.5. Feature Documentation Checker

This table summarizes the feature documentation checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-feature-docs`, `check-feature-docs` |
| Rule | Section 7.8 of the writing guide |
| Reads | For each workspace member with a library target, its features as `cargo metadata --no-deps --offline` reports them, and the `# Features` section of the `//!` crate-level documentation at the top of its library root, before the first item. `cargo metadata` also reports the feature that an optional dependency defines when no feature refers to that dependency with `dep:` |
| Reports | A missing `# Features` section in any library, including a library that has no features, a feature, other than `default`, that the section does not name in a code span, and a code span in the section that looks like a feature name, meaning lowercase letters, digits, `_`, `+`, and `-`, but names no feature of the package |

A crate name such as `slang-compiler` looks like a feature name, so a `# Features` section writes the names of crates as plain text, without a code span.

The checker requires the section in a library that has no features, because item 4 of Section 7.4 of the writing guide requires it in every crate. It does not read crate-level documentation written as a `#![doc = ...]` attribute, including `#![doc = include_str!(...)]`.

Unlike the other checkers, it takes the paths of Cargo manifests, and with no paths it checks the workspace of the git repository that contains the current directory. It prints paths relative to the current directory. When `cargo metadata` fails, it exits with status 2.

Whether the section says that each feature is on or off by default, and what it enables, stays a judgement rule, because the wording is free.

### 6.6. Prose in API Documentation

`scripts/check-prose` reads the text of the `///` and `//!` comments of `.rs` files and applies to it the rules that it applies to Markdown. This enforces Section 7.11 of the writing guide for the rules of its Section 8 that the script checks.

It reads each run of comments that documents one item as one Markdown document, and it reports each finding at its line in the Rust file. Code spans and fenced code blocks inside the comments stay exempt, as they are in Markdown.

It does not read documentation written as a `#[doc = ...]` attribute or a `/** */` block comment. A Rust file that is not valid UTF-8 cannot be read (Section 6.1). With no paths, `check-prose` checks every Markdown file and every Rust file, and its self-test is `check-prose-self-test`.

`scripts/check-prose` reads two data files beside it. `scripts/prose-wordlist.txt` holds the non-Canadian spellings that Section 8 of the writing guide bans. `scripts/prose-phrases.txt` holds the fixed phrases of the phrase check, each group under a line in square brackets that holds the hint printed with each hit. The script itself holds the six patterns of the phrase check, which Section 6 of `docs/avoiding-claudelish.md` specifies, and runs the check on Markdown files and API documentation alike. A data file that cannot be read, or a phrase above the first hint, is a broken setup, so the script exits with status 2.

### 6.7. Moving a Rule to Enforced

TOOL-17. The change that lands a custom checker MUST do all of these in the same change, as Section 3.5 of the writing guide and Section 19 of the coding standard require for a rule that a tool can check:

- Rewrite the "Checked by" statement of the section of the writing guide that the checker enforces, and its row in Section 7.12 of the writing guide, to name the checker.
- Add the checker to the table of tools in Section 4.8 of the coding standard.
- Update the Validating Changes section of `AGENTS.md`.

A document that still calls an enforced rule a judgement rule sends a reviewer to check by hand what a tool checks, which Hard Rule 4 of `AGENTS.md` forbids. The change that extends `scripts/check-prose` also updates Section 8 of the writing guide, which describes the files that the script and its hook check.

### 6.8. Workflow Pin Checker

This table summarizes the workflow pin checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-workflow-pins`, `check-workflow-pins` |
| Rule | TOOL-20, the `pull_request_target` part of TOOL-22, and the top-level part of TOOL-19 (Section 7.2) |
| Reads | Every `.yml` and `.yaml` file under `.github/workflows/`, and every `action.yml` and `action.yaml` file under `.github/actions/`. A file named `action.yml` or `action.yaml` is read as a composite action, and any other file as a workflow |
| Reports | In every workflow and composite action, a `uses:` whose reference is not a full 40-character lowercase hexadecimal commit SHA. In every workflow, also the `pull_request_target` event under the top-level `on:` key, a workflow without a top-level `permissions:` key or with an empty one, and a top-level permission other than `read` or `none`, such as `write-all` or `contents: write` |

A local action, whose reference starts with `./`, needs no commit SHA. A `docker://` reference passes only when it is pinned by an `@sha256:` digest. The checker does not check the permissions of a job, so whether a job that requests more than read access needs it stays a judgement rule, and so does the part of TOOL-22 about secrets, publishing, and `id-token: write`. The documentation workflow of Section 7.5 sets `contents: read` at its top level and requests `pages: write` and `id-token: write` on its deploy job alone, so it passes the checker with no change to the checker.

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

### 6.9. Slang Call Guard Checker

This table summarizes the Slang call guard checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-slang-call-guard`, `check-slang-call-guard` |
| Rule | SLC-32 of `docs/design/slang-compiler.md`, which applies the guard of FFI-7 of the coding standard to `slang-compiler` |
| Reads | The names of the C functions and thunks in `target/slang-bindgen/bindings.rs` under the root of the git repository that contains the current directory, which the recipe writes before it runs the checker, and every `.rs` file under `crates/slang-compiler/src`, at any depth, except every file named `fake.rs`, every inline module marked `#[cfg(test)]`, `use` items, comments, and literals |
| Reports | A call into Slang that is not inside the closure argument of `abort_on_exception`, a call inside a closure or `fn` item nested in that closure, a C function or thunk that is named without being called, a closure argument that makes no call or more than one call into Slang, a second argument of `abort_on_exception` that is not a closure literal, a `move` closure, and a call of `abort_on_exception` without a closure argument |

The checker keeps no list of names by hand. On every run, it reads the generated bindings. No generated file is kept in the repository, so the recipe first runs the binary of `slang-bindgen` on the headers of the SDK at `VULKAN_SDK`, which in CI and in the development container is the recorded Slang, and the result of the checker depends only on those headers (SCS-60 of `docs/design/slang-compiler-sys.md`). The C functions are the functions of their `unsafe extern "C-unwind"` block, and the thunks are the `unsafe fn` methods of each inherent `impl` block, whose type is the interface.

A change of the bindings therefore changes what the checker looks for without a change to the checker. When the bindings declare no C function or no thunk, the checker exits with status 2, so that a change in the layout of the bindings cannot turn the check off silently.

A C function counts as a call into Slang when its name appears as a word, with or without a path, except after `.` or `fn` and before `:`.

A thunk counts when it appears in the path `Q::m` or `<Q>::m`, where `m` is a thunk of the interface `Q`, also when `Q` ends a longer path, such as `slang_compiler_sys::IModule`. When `Q` is `Self`, or a macro metavariable other than `$crate`, such as `$interface`, the name of any thunk counts. A name that a `use` item imports with `as`, or that a type alias names, counts as the name it stands for.

A thunk takes its object as an argument and has no `self` receiver, so code reaches it only through a path through its interface. These forms are therefore not a call into Slang:

- A thunk name after any other qualifier, such as `T::release` or `component::link`.
- A thunk name after `<Q as Trait>::` or after `$crate::`.
- Every method call written with a dot.

This keeps names that Slang shares with other code, such as `release` and `link`, from causing findings.

When guards are nested, a call belongs to the innermost one, so an outer guard whose only call sits in an inner guard is reported as making no call into Slang. A nested closure is a `|` after one of `(`, `[`, `{`, `,`, `=`, `;`, `:`, `>`, `&`, `return`, `break`, and `move`, which includes the closure of a match arm. A call in the body of a macro counts once, not once for each expansion.

The checker follows TOOL-14, with the departure that TOOL-14 allows. It prints each finding as `path:line: message`, and the message names no rule identifier, as the checker of Section 6.10 requires.

With no paths, it checks every `.rs` file under `crates/slang-compiler/src` that git tracks or does not ignore, and prints paths relative to the root of the repository. A file named `fake.rs` is skipped also when it is given as a path.

It exits with status 2 after checking every readable file when a file cannot be read, is not valid UTF-8, ends inside an unterminated literal or comment, or has a bracket without its partner. It reports the last case as `check-slang-call-guard: cannot read <file>: unbalanced '<bracket>' at line N` or `unclosed '<bracket>' at line N`. It also exits with status 2, before it checks any file, when it runs outside a git repository or when the bindings cannot be read or declare no C function or no thunk, because it then has no names to look for.

The cases of its self-test under `scripts/fixtures/check-slang-call-guard/` give the checker small bindings of their own at `target/slang-bindgen/bindings.rs`, and the self-test runs the checker without the binary of `slang-bindgen`. They cover each kind of finding, the forms that it must not report, such as the name collisions above, calls in comments, literals, test modules, and `fake.rs`, and each error of exit status 2.

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
crates/slang-compiler/src/module.rs:489: call of IModule::getName is not inside a closure passed to abort_on_exception
```

### 6.10. Document Reference Checker

This table summarizes the document reference checker.

| Property | Value |
| --- | --- |
| Script and recipe | `scripts/check-doc-references`, `check-doc-references` |
| Rule | Section 7.1 of the writing guide, and Hard Rule 15 of `AGENTS.md`, for the citations that a tool can recognize |
| Reads | The project documents, which are the Markdown files under `docs/` of the git repository that contains the current directory, at any depth, tracked or not ignored. It then reads the whole text of each checked file, including comments, string literals, the help comments of recipes, and usage texts |
| Reports | The path of a project document, its file name alone, a directory under `docs/` that holds one, a section citation, and a rule identifier that a project document defines |

The checker counts these forms as a citation:

- A document path, such as `docs/design/tooling.md`, its file name alone, such as `tooling.md`, and a directory under `docs/` that holds a document, such as `docs/design/`. The directory `docs/` alone is not reported. A match must not continue a longer name, so the character before it is not a letter, a digit, `_`, `.`, or `-`.
- The word "Section" or "Sections" in any case, followed by whitespace and a number such as `3` or `3.1`. The checker reports every section citation, also one of an external document, because it cannot tell which document a number belongs to.
- A rule identifier: a prefix that a project document defines, a hyphen, and a number, such as `TOOL-14`. A match must stand alone, so `XTOOL-14`, `TOOL-14x`, and `docs/TOOL-14` are not reported.

The checker keeps no list of prefixes by hand. On every run, it collects from every project document the identifiers that stand alone in the first cell of a Markdown table row, such as `| TOOL-14 | 6.1 | ... |`.

An identifier is a run of capital letters and digits that starts with a letter, a hyphen, and a number, and the run is its prefix. The summary tables of labelled requirements have that form.

A document that adds a new prefix is therefore checked without a change to the checker. An identifier whose prefix no document defines in that way, such as `UTF-8`, `SHA-256`, or `RUSTSEC-2026-0441`, is not reported, so a fixed list cannot go stale or report a standard name. A label written only in prose, such as `[IR-8]` in a list of references, defines no prefix.

With no paths, the checker checks every `.rs`, `.toml`, `.yml`, `.yaml`, and `.json` file, the `justfile` at the root, and every file under `scripts/`, tracked or not ignored, and prints paths relative to the root of the repository. It skips its own self-test cases under `scripts/fixtures/check-doc-references/`. It reads the cases of the other checkers under `scripts/fixtures/`, which carry the suffix `.fixture`, as files under `scripts/`.

With no paths, the checker selects no Markdown file outside `scripts/`, so the documents may cite each other. It reads no README either, so RDM-31 of `docs/writing-readmes.md`, which keeps citations of documents out of every README, stays a judgement rule. A path given on the command line is checked whatever its name, and is printed as given.

It prints each finding in one of these forms, at the line where the citation starts. A section citation that spans two lines is reported with its whitespace joined into one space:

```text
path:line: cites the project document <path or name>
path:line: cites a section of a project document: '<citation>'
path:line: cites the rule identifier <identifier> of a project document
```

The checker follows TOOL-14, with the departure that TOOL-14 allows. It exits with status 1 when it reports a finding. It exits with status 2 after checking every readable file when a file cannot be read or is not valid UTF-8.

It also exits with status 2 before it checks any file, even when paths are given, in these cases:

- It runs outside a git repository, with `check-doc-references: not inside a git repository`.
- A project document cannot be read or is not valid UTF-8, with `check-doc-references: cannot read <document>: <cause>`.
- The repository has no project document, with `check-doc-references: no project document: the repository has no Markdown file under docs/`.
- No project document defines a rule identifier, with `check-doc-references: no rule identifier: no project document defines one in the first cell of a table row`.

The last two errors keep a change in the layout of the documents from turning the check off silently.

The cases of its self-test under `scripts/fixtures/check-doc-references/` give the checker small documents of their own, which define the prefix `ZZQ`. They cover these behaviours:

- Each form of citation in each kind of file that the checker reads.
- The forms that it must not report, such as `UTF-8`, `SHA-256`, `GPL-3.0-or-later`, a prefix defined only in prose, a prefix without a number, and a web address that contains `docs/`.
- The files that it skips, which are a Markdown file, a file of another type, and its own cases.
- Paths given on the command line.
- The errors of exit status 2 for a missing git repository, a repository without project documents, documents that define no rule identifier, a checked file that cannot be read, and a checked file that is not valid UTF-8.

Wording about the history of the project, such as "this version" or the milestone that added an item, is beyond what the checker reads, so it stays a judgement rule.

This illustration shows a recipe help comment and the findings that the checker prints for it, assuming it is on line 14 of the `justfile`:

```just
# Run the checks that Section 3.3 of docs/design/tooling.md lists
```

```text
justfile:14: cites a section of a project document: 'Section 3.3'
justfile:14: cites the project document docs/design/tooling.md
```

## 7. Continuous Integration

### 7.1. Workflow and Jobs

One workflow, `.github/workflows/ci.yml`, runs on every push to `main`, on every push to a branch that matches `feature/**`, and on every pull request. The project opens no pull requests, so a plain push of a feature branch is how a branch is verified before it merges (Section 14.7), and each such push starts one run. The `pull_request` trigger stays so that a pull request from a fork runs the same jobs, with a read-only token and no secrets (Section 7.2).

A second workflow, `.github/workflows/documentation.yml`, builds and deploys the hosted documentation on a push of a release tag (Section 7.5). The rest of this section concerns `ci.yml`.

The `concurrency` group of a run on `main` is the run itself, and nothing cancels it, so every commit on `main` keeps its own result. For any other git reference, such as a feature branch or a pull request, the group is the workflow and the reference, and `cancel-in-progress` is true, so a new push cancels the earlier run for that reference.

Every job has a timeout of 60 minutes, except the job `vendored`, which has 90 (Section 7.4), and every `run` step uses Bash, also on Windows. The checkout step sets `persist-credentials: false`, so the token is not left in the git configuration of the runner for later steps.

TOOL-18. A workflow MUST run every check through a `just` recipe and MUST NOT run a check command directly. CI then runs the same commands as the development container, so a check that fails in one place has the same command to reproduce it in the other. Steps that only prepare a runner, such as installing a tool, are not checks.

This table lists the jobs, with the name that GitHub shows for each and the hosted runner label. `check-workflows` and `check-workflow-pins` check the workflow.

| Job | Name | Runner | Runs |
| --- | --- | --- | --- |
| `check` | Run every check on Linux | `ubuntu-latest`, Linux x86_64, with lavapipe as the software adapter | `just check` |
| `test` | Test on `windows-latest`, Test on `macos-latest` | A matrix of `windows-latest`, Windows x86_64, with the Windows Advanced Rasterization Platform (WARP) as the software adapter, and `macos-latest`, macOS arm64, with the hosted Metal device as the adapter, with `fail-fast: false` | `just check-clippy`, `just check-doctest`, and `just test-gpu` |
| `ci-only` | Run `check-features` on Linux, Run `check-msrv` on Linux | `ubuntu-latest`, Linux x86_64, as a matrix of the two recipes, with `fail-fast: false` | `just check-features` and `just check-msrv` |
| `semver` | Check for breaking changes on Linux | `ubuntu-latest`, Linux x86_64 | `just check-semver` (TOOL-39) |
| `coverage` | Measure test coverage on Linux | `ubuntu-latest`, Linux x86_64, with lavapipe as the software adapter | `just coverage`, then the upload of `target/lcov.info` as the artifact `coverage-lcov` |
| `vendored` | Test the vendored build on `ubuntu-latest`, on `windows-latest`, and on `macos-latest` | A matrix of `ubuntu-latest`, `windows-latest`, and `macos-latest`, with `fail-fast: false` | `just check-vendored` on Linux, `just check-rhi` and `just test-rhi` on Linux and Windows, and `just test-vendored` and `just check-vendored-shared-folder` on each platform (Section 7.4) |

The `test` job runs Clippy and the doctests on Windows and macOS as well as the tests, because a lint exception has to be fulfilled on every target that is checked (Section 4.4 of the coding standard), and code under `#[cfg]` differs between platforms. Its build of `slang-compiler-sys` generates the bindings from the headers of the SDK on that platform (SCS-64 of `docs/design/slang-compiler-sys.md`). The tests for Linux only are not compiled there.

Every job except `vendored` builds the default, which never enables `vendored` (SCS-62 of `docs/design/slang-compiler-sys.md`). It compiles `slang-compiler-sys` with its build script, which generates the bindings from the headers of the installed SDK with libclang and links its Slang dynamically. The `test` jobs therefore prove more than compilation on Windows and macOS.

`just test-gpu` links the test binary of `slang-compiler-sys` against the Slang of the SDK and runs it. That includes the smoke test, which creates a global session and checks the build tag at run time. It also includes the native-symbol test, which fails to link when the library does not export a C function of the bindings.

The `macos-latest` runner is arm64, so no job runs the tests on x86_64 macOS (Section 6 of `docs/design/slang-compiler-sys.md`). Linux runs the GPU tier on lavapipe, and Windows and macOS run it on WARP and on the hosted Metal device, with SPIR-V as the only shader target of `slango`. Section 3.8 of `docs/design/slango.md` records the adapter of each runner.

Those tests compile for Linux only, so only the `check` and `coverage` jobs run the tests of `slang-bindgen` that load libclang, through `test-gpu` and `coverage` (SCS-21 of `docs/design/slang-compiler-sys.md`).

No job runs the `hw_gpu` tests, which run only on a developer's hardware GPU through `just test-hw-gpu`. The `vendored` jobs build and test the vendored route on each platform in every run (Section 7.4).

Every job except `vendored` runs its steps in the same order. It checks out the repository, installs the Rust toolchains, installs its tools through the installer action, installs libclang, installs the Vulkan SDK with `just install-vulkan-sdk`, and checks the SDK with `just check-vulkan-sdk` (Section 7.3). Then it runs its own steps:

- The `check` job installs actionlint, lavapipe, and fontconfig and runs `just check`.
- The `test` job runs `just check-clippy`, `just check-doctest`, and `just test-gpu`.
- The `ci-only` job installs fontconfig and runs the recipe of its matrix entry.
- The `coverage` job installs lavapipe and fontconfig, runs `just coverage`, and uploads the report.
- The `semver` job runs `just check-semver`. It installs no adapter and no fontconfig, because the check builds only the libraries of the crates of the release, and none of them depends on Slint.

The workflow defines the steps that check out the repository, install libclang, install and check the SDK, install lavapipe, and install fontconfig once, with YAML anchors. It reuses them in the other jobs, so each of these steps is the same in every job that runs it.

The `coverage` job runs the tests of the `gpu` profile under cargo llvm-cov and keeps the report. The recipe prints no coverage summary in the log of the job (Section 14.5), so the uploaded report is the only result of the job.

The action `actions/upload-artifact`, pinned by commit SHA (TOOL-20), uploads `target/lcov.info` as the artifact `coverage-lcov`, which GitHub keeps for 14 days. The step sets `if-no-files-found: error`, so the job fails when the recipe wrote no report. The job sets no coverage threshold, so the report informs a reviewer and does not gate a merge.

CI installs the tools on each runner, because no container image is published for CI to run in. Each job installs only the tools that its recipes need:

- The Rust toolchains, through rustup, which the hosted runners provide. Each job installs the stable toolchain with the minimal profile and the components that it needs, which are rustfmt and clippy in the `check` job, clippy in the `test` job and in the `vendored` job on Linux, none in the `ci-only` job, the `semver` job, and the `vendored` jobs on Windows and macOS, and llvm-tools in the `coverage` job.

  `check-fmt` needs rustfmt, and cargo llvm-cov needs llvm-tools. The `check` job also installs the nightly toolchain with the components miri and rust-src, and lets rustup choose an older nightly when the latest one lacks a component.
- `just` in every job, cargo-nextest in the `check`, `test`, `coverage`, and `vendored` jobs, shellcheck, typos, taplo, cargo-machete, and cargo-deny in the `check` job, cargo-hack in the `ci-only` job, cargo-llvm-cov in the `coverage` job, and cargo-semver-checks in the `semver` job (TA-20), through the action `taiki-e/install-action`, pinned by commit SHA. The action installs prebuilt binaries and verifies the SHA-256 checksum of each one against the manifest of versions and checksums that it carries. The workflow sets `fallback: none`, so the action fails instead of installing a tool by a route that its manifest does not cover.

  The manifest of cargo-semver-checks at the pinned commit of the action, v2.90.3, names version 0.51.0 as the latest, with a SHA-256 checksum of the build for Linux x86_64. That the job installs the tool this way on a hosted runner is TA-20.
- actionlint, in the `check` job, through an inline step. It reads the latest release from GitHub, downloads the release archive for Linux x86_64 and the checksums file of the same release, verifies the archive with `sha256sum --check --strict`, and fails on a mismatch before it extracts the archive.
- libclang, in every job that builds `slang-compiler-sys`, because every build of `slang-compiler-sys` generates its bindings with libclang (SCS-64 of `docs/design/slang-compiler-sys.md`), and the tests of `slang-bindgen` load it on Linux. On Linux, the step runs `sudo apt-get update` and `sudo apt-get install --yes --no-install-recommends libclang-dev`. It meets TOOL-21 because apt verifies the signature of the distribution's package index and the SHA-256 checksum of each package against that index, and refuses a package that fails either check.

  The package is the libclang of the runner's distribution, and its version can differ from the one in the development container. On Windows and macOS, the jobs use the libclang that the runner image provides. The Windows image provides it through its LLVM installation and the macOS image through the toolchain of Xcode, and `clang-sys` loads each through `LIBCLANG_PATH`, as the `test` jobs show. The step looks for `libclang.dll` in `C:\Program Files\LLVM\bin` on Windows. On macOS, it looks for `libclang.dylib` in `usr/lib` of the default toolchain of Xcode, under the path that `xcode-select --print-path` prints, and then in `/Library/Developer/CommandLineTools/usr/lib`.

  The step always sets `LIBCLANG_PATH` to the directory where it found libclang, on Windows and macOS, so that the libclang that loads is the one that it checked. When the file is missing, the step fails with an error annotation that names the path and says that every build of `slang-compiler-sys` needs libclang. The step runs before the installation of the Vulkan SDK, so a missing libclang fails the job before the download.
- lavapipe, in the `check` and `coverage` jobs, as Mesa's package `mesa-vulkan-drivers`, which gives the tests in modules named `gpu` a software Vulkan adapter. The step runs `sudo apt-get update` and `sudo apt-get install --yes --no-install-recommends mesa-vulkan-drivers`, so it meets TOOL-21 in the same way as the libclang step. The `test` jobs install no adapter.
- The development files of fontconfig, in the `check`, `ci-only`, and `coverage` jobs, as the package `libfontconfig-dev`, and in the `vendored` job on Linux when the runner image lacks them (Section 7.4). A Linux build of Slint links fontconfig through the build script of `yeslogic-fontconfig-sys`, which finds it through pkg-config (Section 3.2 of `docs/design/slang-slint.md`).

  The step runs `sudo apt-get update` and `sudo apt-get install --yes --no-install-recommends libfontconfig-dev`, so it meets TOOL-21 in the same way as the libclang step. The `test` jobs install nothing for it, because the builds on Windows and macOS need no fontconfig development files.
- The Vulkan SDK of the version that the workflow records, in every job except `vendored`, through `just install-vulkan-sdk` (Section 7.3).
- CMake, Ninja, a C++ compiler, Python 3, and, on Linux, pkg-config and the development files of fontconfig in the `vendored` jobs, as Section 7.4 states.

Each route except the Vulkan SDK installs the latest version that it provides, which matches the policy of the container image, in which no tool version is pinned. The Vulkan SDK is the version that the workflow records (Section 7.3). A tool in CI can therefore have a different version from the same tool in the development container, and a check can pass in one and fail in the other until both have the same version.

The workflow caches nothing, so no file is restored from a cache.

### 7.2. Workflow Security

TOOL-19. A workflow MUST grant its token read access to the repository contents only, at the level of the workflow, and a job that needs more MUST request the extra permission on that job alone. A compromised step can use every permission that the token holds.

`check-workflow-pins` reports a workflow without top-level permissions and a top-level permission other than `read` or `none` (Section 6.8). Whether a job that requests more needs it is a judgement rule.

TOOL-20. A workflow MUST reference every action, including the actions that GitHub publishes, by a full commit SHA and never by a tag or a branch. The owner of an action can move a tag or a branch to different code, which then runs in every later job without any change to this repository.

actionlint does not check this, and `check-workflow-pins` enforces it (Section 6.8).

TOOL-21. A workflow step that downloads a file MUST verify it against a SHA-256 checksum that the upstream publishes and MUST stop the job before the file is used when the checksum differs, as Hard Rule 7 of `AGENTS.md` requires.

A step that installs tools through an action MUST use an action that verifies each download in this way, pinned by commit SHA (TOOL-20), which also pins the manifest of versions and checksums that the action carries. The step MUST configure the action to fail instead of installing by a route that it does not verify.

A file restored from a cache MUST be verified in the same way as a fresh download.

These requirements hold because a tampered file that is not verified runs with the permissions of the job, and a cache can hold a file that an earlier, compromised run stored.

When the upstream publishes no checksum, the step SHOULD record in the workflow the checksum of the file that the agent verified when it wrote the step. A later change in the served file is then detected, even though the first file is trusted on first use.

A checksum that comes from the same host as the file, such as the checksums file of an actionlint release, proves that the download is intact but not that it comes from the publisher, because an attacker who controls the host can replace both. A checksum in the pinned manifest of the installer action comes from the commit of the action that TOOL-20 pins instead.

Section 13 states the residual risk. Whether each step meets TOOL-21 is a judgement rule, because no tool can tell from a workflow which step downloads a file.

The action `actions/upload-artifact`, which the `coverage` job uses (Section 7.1), is pinned by commit SHA like every other action, and `check-workflow-pins` checks it. The step that installs the Vulkan SDK meets TOOL-21 with checksums that the workflow records (Section 7.3). The installers of the SDK for macOS and Windows also fetch metadata from LunarG during the installation, which that checksum does not cover, and Section 13 states that residual risk.

TOOL-22. A workflow MUST NOT use the `pull_request_target` event, read a secret, or publish a crate, except that the deploy job of TOOL-42 MAY request the OpenID Connect (OIDC) token of its run through `id-token: write`. The workflows need no secret to check code, and a workflow that holds a secret while it runs the code of a pull request exposes that secret to the author of the pull request.

`check-workflow-pins` reports the `pull_request_target` event (Section 6.8). The rest of TOOL-22, including which job requests `id-token: write`, is a judgement rule.

The exception is narrow because the OIDC token is not a stored secret. GitHub issues it to that one run, and only a user with write access to the repository can start the deploy job, by pushing a tag or, in the fallback of Section 7.5, by starting the workflow by hand, while no pull request can. A deployment to GitHub Pages publishes no crate.

The workflow keeps the `pull_request` trigger for pull requests from forks. GitHub gives a run for such a pull request a read-only token and no secrets, so the code of the pull request can neither write to the repository nor read a secret. A push to `main` or to a feature branch needs write access to the repository, so the push triggers add no way for an outside author to run code in CI.

### 7.3. The Vulkan SDK on CI Runners

Every job of the workflow except `vendored` installs the Vulkan software development kit (SDK) of the version that the workflow records and checks it before any step that compiles the workspace. The build script of `slang-compiler-sys` generates the bindings from the headers of that SDK and links its Slang, and every such job compiles that crate. SCS-35 of `docs/design/slang-compiler-sys.md` states what the installation achieves.

TOOL-23. The step that installs the Vulkan SDK on a runner MUST do all of these:

- Read the SDK version from the top-level `env` of the workflow, the one place that records it (ARCH-29 of the architecture design), and not restate the version.
- Verify the download as TOOL-21 requires.
- Set `VULKAN_SDK` for the later steps.
- Make the Slang shared library findable at run time on the runner in the way that Section 4.4 of the architecture design describes for the platform.

A second copy of the version diverges from the first. An unverified installer runs with the job's permissions. A test binary that cannot find the shared library fails before any test runs.

The workflow records the version in its top-level `env` as `VULKAN_SDK_VERSION`, next to the three checksums of its downloads, so that the version and its checksums change together in one place. The recorded commit of `slang-src` is the commit of the Slang in that SDK, and `check-vulkan-sdk` fails when the two differ (ARCH-29). No other file of the repository states the SDK version.

The step "Install the Vulkan SDK" runs this command, in which `SDK_SHA256` is the checksum that the workflow records for the platform of the runner:

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

The macOS installer needs no `sudo` for a directory that the user owns. Both installers fetch metadata from LunarG's remote repository during the installation, and no supported option to prevent this is known, which Section 13 records as a residual risk.

The step "Check the Slang of the Vulkan SDK" then runs `just check-vulkan-sdk`. The recipe runs `scripts/check-vulkan-sdk`, which reads `RECORDED_BUILD_TAG` with `just recorded-slang` (SRC-11 of `docs/design/slang-src.md`) and checks these things for that tag:

- `$VULKAN_SDK/include/slang` holds `slang.h` and `slang-tag-version.h`, and `slang-tag-version.h` has the exact line `#define SLANG_TAG_VERSION "<tag>"`.
- The Slang library of the platform exists, which is `lib/libslang-compiler.so` on Linux, `lib/libslang-compiler.dylib` on macOS, and `lib/slang-compiler.lib` and `bin/slang-compiler.dll` on Windows, whose file system resolves these names to the directories `Lib` and `Bin`.
- `$VULKAN_SDK/bin/slangc -version` prints output that contains the tag.

The script also lists the files that it finds. It removes carriage returns from `slang-tag-version.h` and from the output of `slangc` before it compares them, so it accepts CRLF line endings. It needs no CI runner, so the same command checks the SDK of the development container.

The script exits with status 1 when a check fails, including when `VULKAN_SDK` names a path that is not a directory. For that case, it prints `check-vulkan-sdk: VULKAN_SDK names <path>, which is not a directory` and checks nothing else. It exits with status 2 when it gets an argument, when `VULKAN_SDK` is unset or empty, or when `just recorded-slang` fails or prints no `RECORDED_BUILD_TAG`.

A failure of the build tag means that the SDK of the workflow and the recorded commit of `slang-src` name different Slangs. The change that moves the recorded Slang changes both in one change (ARCH-30 of the architecture design).

LunarG publishes a SHA-256 checksum of each SDK download, as 64 hexadecimal characters at a documented address of the form `https://sdk.lunarg.com/sdk/sha/<version>/<platform>/<file>.txt`, which is served from the same host as the download. A checksum fetched from that host at run time detects a corrupted download, but not a download that an attacker at the host replaced together with its checksum. The workflow therefore records the expected values and does not fetch them at run time, which the fixed version of the SDK that the workflow records allows, unlike the latest-version installs of Section 7.1.

The workflow records the three checksums in its top-level `env` as `VULKAN_SDK_SHA256_LINUX`, `VULKAN_SDK_SHA256_MACOS`, and `VULKAN_SDK_SHA256_WINDOWS`, next to `VULKAN_SDK_VERSION`. The install step selects the one for `runner.os`.

The workflow is the only user of the version and its checksums, so they stay there and change together, in the change that moves the recorded Slang (Section 8.2 of `docs/design/slang-compiler-sys.md`). LunarG also publishes an installer for Windows on ARM64, which the workflow does not use, because the `windows-latest` runner is x86_64.

The workflow does not cache the SDK. A cache of the installed tree would save at most the installation time, and restoring a large tree takes time of its own.

TOOL-21 also requires a restored tree to be verified like a fresh download, which would need a recorded manifest of the hash of every file. A cache of the downloaded file alone is easy to verify against the recorded checksum, but it saves only the download.

Sections 3.6, 3.7, and 12 of `docs/design/slang-compiler-sys.md` record what each SDK ships.

### 7.4. The Vendored Jobs

The job `vendored` runs `just test-vendored` and then `just check-vendored-shared-folder` on Linux, Windows, and macOS in every run of the workflow, as SCS-62 of `docs/design/slang-compiler-sys.md` requires. On Linux it first runs `just check-vendored`, so that Clippy and the rustdoc lints reach `slang-src` and `slang-glslang-sys`. It installs no Vulkan SDK and starts from an empty target directory, so `slang-src` clones the recorded commit from GitHub, and the job tests the clone, the shared folder, the native builds, and the static link together.

Each entry of the matrix runs these steps in this order:

1. It checks out the repository and installs the stable toolchain with the minimal profile, with the component clippy on Linux and Windows.
2. It installs `just` and cargo-nextest through the installer action of Section 7.1.
3. It prints the version of git, which clones Slang, so that the log records the git of each runner. Every hosted runner provides git.
4. It makes libclang available as Section 7.1 states for the platform, with the step that the other jobs share.
5. On Linux, it checks `cmake`, `ninja`, `c++`, `python3`, and `pkg-config` with `command -v`, and installs the packages `cmake`, `ninja-build`, `g++`, `python3`, and `pkg-config` of those that are missing with apt, in the way of the libclang step. It also installs `libfontconfig-dev` when `pkg-config --exists fontconfig` fails, because the Clippy run on `slang-slint` of `check-vendored` builds Slint, which links fontconfig on Linux. It installs nothing when nothing is missing. On Windows and macOS, it uses those that the runner image provides. The macOS image provides CMake, Ninja, Apple clang, and Python 3, and the Windows image provides CMake, Ninja, Python 3, an installation of Visual Studio with its C++ tools that the lookup of SCS-68 of `docs/design/slang-compiler-sys.md` finds, and the Windows SDK with its D3D12 headers and `dxcompiler.dll`, which the build and the tests of slang-rhi need.
6. It prints the versions of CMake and Ninja, and of `c++` on Linux and macOS, so that the log records the tools of each runner. On Windows, the build script finds the Microsoft compiler itself (SCS-68 of `docs/design/slang-compiler-sys.md`).
7. It looks for `python3` and then `python` on `PATH`, takes the first whose major version is 3, and prints its path and version. When it finds none, it fails with an `::error::` annotation that says that the runner image provides no Python 3 as `python3` or `python` on `PATH`, which the build of slang-glslang needs because SPIRV-Tools generates some of its sources with a Python script. The step looks for both names because the Windows image provides Python 3 under the name `python`, and the build of SPIRV-Tools finds it there.
8. On Linux, it checks the packages `libx11-dev`, `mesa-vulkan-drivers`, and `libvulkan1` with `dpkg-query --show --showformat='${db:Status-Status}'`, and installs those that are not installed with apt, in the way of step 5. The development headers of Xlib serve the build of slang-rhi (Section 7.1 of `docs/design/slang-rhi-sys.md`), and lavapipe and the Vulkan loader serve the `gpu` tests of `slang-rhi-sys` and `slang-rhi`.
9. On Linux, it runs `time just check-vendored`.
10. On Linux and Windows, it runs `time just check-rhi`, which on Linux reuses the clone and the build of Slang of the previous step in the shared folder `target/vendored/slang-src`. The recipe also lints `slango` and the examples package with the feature `rhi` (TOOL-53).
11. It runs `time just test-vendored`, which on Linux and Windows reuses the clone and the built libraries of the previous steps.
12. On Windows, it looks for `dxcompiler.dll` in the versioned x64 directories of the Windows SDK, `C:\Program Files (x86)\Windows Kits\10\bin\10.*\x64`, takes the one of the highest version, prints its path, and appends its directory to `GITHUB_PATH`. When it finds none, it fails with an `::error::` annotation that says that the runner image provides no `dxcompiler.dll` in a versioned Windows SDK directory, which the tests of slang-rhi on D3D12 need. The version of DXC that the tests use therefore follows the runner image.
13. On Linux and Windows, it runs `time just test-rhi`. The recipe also runs the tests of `slango` and of the examples package, with the `gpu` tests of the generated code of `slango-macros`, on the backend `rhi`, on lavapipe on Linux and on WARP on Windows (TOOL-53). The commands of the examples build the examples and the macro's copy of `slango` with wgpu in `target/vendored`, and they include the scenes of both windowed examples. The recipe runs every command after a failing one, so the log of one run shows every failing command (TOOL-53).

In CI run 44398417125 of 2026-11-18, with the commands of `slango` of the third code step, the job `vendored` on Windows took 15 minutes in all, far below its timeout. Its `test-rhi` printed "143 tests run: 143 passed, 0 skipped" for the slang-rhi crates and "271 tests run: 271 passed, 66 skipped" for `slango` on the backend `rhi` on WARP.

In CI run 44449874191 of 2026-11-19, with the commands of the examples package of the fourth code step, the job `vendored` on Windows took 13 minutes in all. Its `test-rhi` printed "225 tests run: 225 passed, 0 skipped" for the slang-rhi crates, "304 tests run: 304 passed, 56 skipped" for `slango`, and "34 tests run: 34 passed, 5 skipped" for the examples package, on the backend `rhi` on WARP.

In CI run 44474920321 of 2026-11-19, with the scenes of both windowed examples on the backend `rhi` of the sixth code step, the job `vendored` on Windows took 16 min 38 s in all: `check-rhi` 12 min 30 s, `test-vendored` 1 min 40 s, and `test-rhi` 1 min 45 s. This confirmed the former assumption TA-25, that the timeout of 90 minutes stays enough. Its `test-rhi` printed "226 tests run: 226 passed, 0 skipped" for the slang-rhi crates, "376 tests run: 376 passed, 3 skipped" for `slango`, and "46 tests run: 46 passed, 0 skipped" for the examples package, on the backend `rhi` on WARP.
14. It runs `just check-vendored-shared-folder`, which checks that the shared folder is `target/vendored/slang-src`, that it holds exactly the clone entry that `recorded-slang` names, and that no other directory named `slang-src` holds a clone below `target/vendored`, and so shows on each platform that the lookup of Section 3.2 of `docs/design/slang-src.md` works in a target directory that Cargo created. It also prints the sizes of the folder and of the clone entry.

The steps that run the vendored recipes start them under the shell keyword `time`, so the log records how long the clone and the native builds take on each runner.

No step sets up the developer environment of the Microsoft toolchain or sets `CC` or `CXX`, and no step changes how rustc links. A developer environment exported to the later steps sets `VCINSTALLDIR`, which makes rustc take `link.exe` from `PATH`, and in the Bash of the Windows runner the `link` of Git's tools comes first there. The build script gives the environment of the Microsoft toolchain only to the `cmake` runs (SCS-68).

Steps 4 to 7 serve only the builds of `slang-compiler-sys` and `slang-glslang-sys` and, for fontconfig, the Clippy run on `slang-slint`, so the job has them only while `vendored_packages` or `rhi_packages` holds a crate that builds Slang (Section 3.1). Steps 8 and 12 serve only the crates of `rhi_packages`. Without these steps, the job needs only git, `just`, cargo-nextest, and the toolchain.

The job reaches the network only through the clone of `slang-src`, which fetches what ARCH-35 of the architecture design permits over HTTPS (Section 14 of `docs/design/slang-src.md`), and through the installation of its tools. It caches nothing between runs, so each run clones and builds from empty.

A vendored build takes several minutes on a machine with many cores (Section 3.10 of `docs/design/slang-compiler-sys.md`), and a hosted runner has fewer. The job has a timeout of 90 minutes, because on Windows it builds Slang with DXIL support, slang-glslang, and slang-rhi from an empty target directory. In run 44291885863, `check-rhi` took 15 min 13 s from an empty target directory on Windows, then `test-vendored` 2 min 25 s and `test-rhi` 19.6 s, and on Linux `check-vendored` took 13 min 49 s, `check-rhi` 32 s, `test-vendored` 1 min 5 s, and `test-rhi` 6.8 s.

The job is the only place where the vendored routes run on Windows and macOS, so it also confirms SRCA-3 of `docs/design/slang-src.md` and the assumptions of `docs/design/slang-glslang-sys.md` that name the vendored CI job.

### 7.5. The Documentation Workflow

TOOL-42. The hosted documentation of Section 9.4 MUST be built and deployed by a workflow of its own, `.github/workflows/documentation.yml`, which runs only on a push of a tag that matches `v*`. Its top-level permissions MUST be `contents: read`. Its deploy job MUST request `pages: write` and `id-token: write`, the permissions that a deployment to GitHub Pages needs [GH-PAGES-WORKFLOW], and every other job MUST NOT request more than the top level grants.

`ci.yml` runs the code of every pull request, so a separate workflow keeps the permissions that change the public site out of every run that a pull request or a branch can start. The design chooses a separate file over a job of `ci.yml` for this reason. Only a user with write access to the repository can push a tag, and the release procedure pushes one only after the owner's go-ahead (Section 9.6).

The workflow has two jobs on `ubuntu-latest`, each with the timeout of 60 minutes of every job:

| Job | Name | Permissions | Steps |
| --- | --- | --- | --- |
| `build` | Build the documentation site | The top-level `contents: read` | Checks out the tag with `persist-credentials: false`, installs the stable toolchain with the minimal profile, installs `just` through the installer action of Section 7.1, makes libclang available, installs and prints the vendored tools as steps 5 to 7 of Section 7.4 do on Linux, runs `just doc-site`, and uploads `target/vendored/doc` with `actions/upload-pages-artifact` |
| `deploy` | Deploy the documentation site | `pages: write` and `id-token: write`, and no other | Needs `build`. Runs in the environment `github-pages` with the address `${{ steps.deployment.outputs.page_url }}`, and runs `actions/deploy-pages` in the step with the identifier `deployment` |

Besides the checkout and installer actions that `ci.yml` pins, the workflow uses these actions, each pinned to the commit of its release (TOOL-20):

| Action | Release | Commit SHA |
| --- | --- | --- |
| `actions/upload-pages-artifact` | v5.0.0 | `fc324d3547104276b827a68afc52ff2a11cc49c9` |
| `actions/deploy-pages` | v5.0.1 | `368f82528645a54fb793d4d04e342629a3f51346` |

The workflow does not use `actions/configure-pages`, of which v6.0.0 is commit `45bfe0192ca1faeb007ade9deae92b16b8254a0d`. Its input `enablement` can turn Pages on only with a token other than `GITHUB_TOKEN` [CONFIGURE-PAGES], which TOOL-22 rules out, and rustdoc writes relative links, so the site needs no base address from it. The first run of the workflow confirmed that `actions/deploy-pages` deploys the artifact without it, in a deploy job without `contents: read`. The owner turns Pages on in the settings of the repository instead (Section 9.6).

The `concurrency` group of the workflow is `pages`, with `cancel-in-progress` false, so a deployment that has started finishes. The workflow caches nothing, so each run clones Slang and builds Slang and slang-glslang from an empty target directory, as the `vendored` job does.

GitHub Pages limits a published site to 1 GB and a deployment to 10 minutes [GH-PAGES-LIMITS], and `doc-site` checks the size (TOOL-41). The artifact may hold no symbolic or hard link [UPLOAD-PAGES-ARTIFACT]. The site is public, because the repository is public, as `check-release` confirmed by reading each address of the first release without credentials.

The workflow cannot share the YAML anchors of `ci.yml`, so its steps that install libclang and the vendored tools are copies of those of `ci.yml`. A change to one of these steps changes both workflows in the same change, which is a judgement rule.

The copies are verbatim, by a choice of the design, so that a diff of the two workflows shows any drift. They keep the branches for Windows and macOS and the condition `runner.os == 'Linux'`, which never apply on `ubuntu-latest`, and the installation of the development files of fontconfig, which `doc-site` does not need.

The job `build` runs on a fresh checkout, so `doc-site` writes the site into an empty `target/vendored/doc`, and the artifact holds only the pages of that build (Section 9.4).

To deploy the site of a tag again, such as after a failed deployment, the owner reruns the workflow run of that tag. A rerun changes the public site, so it needs the owner's go-ahead like the push of the tag.

The environment `github-pages` is expected to allow deployments only from the default branch until the owner adds a rule for the tags, which step 6 of Section 9.6 does (TA-21). With that rule, the environment allows the deployment of the tag of a release, as the first run of the workflow confirmed, so the workflow needs no other trigger.

## 8. Dependency Policy

The file `deny.toml` configures cargo-deny, which Section 4.8 of the coding standard lists with what it owns. Section 4.8 also requires `deny.toml` to accept the licences of the workspace's own crates, which ARCH-36 of the architecture design assigns. cargo-deny checks every crate of the graph against the licence policy, the workspace's own included, whether or not the crate is published.

TOOL-24. The licence allow-list of `deny.toml` MUST hold the licences of the workspace's crates under permissive licences, `Apache-2.0`, `MIT`, and `Apache-2.0 WITH LLVM-exception`, and otherwise only licences that a dependency needs. `GPL-3.0-or-later`, the licence of `slang-slint`, MUST NOT be on the allow-list and MUST be accepted only through the exception for `slang-slint` alone. An agent MUST add any other licence only in the change that adds the first dependency under it, after confirming that it is no GPL licence when a crate under a permissive licence depends on it (TOOL-34), and that it is compatible with GPL-3.0-or-later when `slang-slint` or the examples package depends on it. A licence that no dependency uses widens what a later dependency can bring in without any check failing.

TOOL-34. A crate under `MIT OR Apache-2.0` or `Apache-2.0 WITH LLVM-exception`, except the examples package, MUST NOT depend on a crate that the workspace takes under the GNU General Public License (GPL). A GPL licence is accepted only for `slang-slint`, the examples package, and the crates of Slint that they use.
cargo-deny enforces one part of TOOL-34: it accepts `GPL-3.0-or-later` for `slang-slint` alone, so any other crate under that licence fails the check. The rest of TOOL-34 is a judgement rule that a reviewer checks. `GPL-3.0-only` is on the allow-list for every crate, because cargo-deny cannot accept a licence only inside the dependency tree of one crate, and an exception for each crate of Slint would be a long list that grows with Slint. Nothing in the check therefore stops a crate under a permissive licence from gaining a dependency under `GPL-3.0-only`.

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
allow = [
    "Apache-2.0",
    "Apache-2.0 WITH LLVM-exception",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "BSL-1.0",
    "GPL-3.0-only",
    "ISC",
    "MIT",
    "Unicode-3.0",
    "Zlib",
]
confidence-threshold = 0.8
unused-allowed-license = "deny"
exceptions = [{ crate = "slang-slint", allow = ["GPL-3.0-or-later"] }]

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

cargo-deny checks the dependency graph of every target, including WebAssembly, Apple platforms, Android, and Redox, so the lists cover crates that a build on Linux does not compile. `all-features = true` makes it also check the graph of the feature `vendored`, which it reads without building anything. The rule of Section 3.1 that no command passes `--all-features` therefore does not apply to it.

`slang-src` and `slang-glslang-sys` add no dependency from crates.io (SRC-2 of `docs/design/slang-src.md`, SGL-1 of `docs/design/slang-glslang-sys.md`). Every crate that depends on `slang-src` requires its exact version (SRC-14), which meets TOOL-25.

These dependencies shape the lists:

- `clang`, for `slang-bindgen`, which every build of `slang-compiler-sys` compiles on the user's machine (SCS-61 of `docs/design/slang-compiler-sys.md`).
- wgpu 30 and `pollster`, for `slango` and the examples package.
- winit 0.30.13, for the examples package only, with its default features off and the features `rwh_06`, `x11`, `wayland`, and `wayland-dlopen` on.
- `slint` with the version requirement `~1.18` and its default features off. `slang-slint` enables the features that Section 4.1 of `docs/design/slang-slint.md` lists and no windowing backend or renderer of Slint. The examples package enables Slint's windowing backend on winit and its FemtoVG renderer on wgpu (Section 10 of that design).

Slint's winit backend turns on winit's feature `wayland-csd-adwaita` for every example, including `windowed-renderer`, as Section 3.6 of `docs/design/slang-slint.md` records. `sctk-adwaita` and the crates under BSD licences below are therefore in the graph.

The licence allow-list holds `Apache-2.0 WITH LLVM-exception`, the licence of `slang-compiler-sys`, and these licences, each for the crates that need it. `Apache-2.0` and `MIT` also cover the workspace's crates under `MIT OR Apache-2.0`:

| Licence | Crates that need it | How they enter |
| --- | --- | --- |
| `Apache-2.0` | `clang` and `clang-sys`, and `glob`, `libc`, `cfg-if`, and `windows-link`, whose licence `MIT OR Apache-2.0` it satisfies | `slang-bindgen` |
| `BSD-2-Clause` | `arrayref` 0.3.9 | `tiny-skia` 0.11 and 0.12, and `tiny-skia-path` 0.11 |
| `BSD-3-Clause` | `tiny-skia` and `tiny-skia-path` 0.11.4 and 0.12.0, and `bindgen` 0.72.1 | `tiny-skia` 0.11 through `sctk-adwaita` 0.10, `tiny-skia` 0.12 through `resvg` of `i-slint-common`, and `bindgen` as a build dependency of `skia-bindings` of Slint's Skia renderer, which `i-slint-backend-winit` uses only on Apple targets other than macOS |
| `BSL-1.0` | `clipboard-win` 5.4.3 and `error-code` 3.4.0 | `arboard` of `i-slint-backend-winit`, on Windows |
| `GPL-3.0-only` | The seven crates of Slint 1.18.1 in the graph, `slint`, `slint-macros`, `i-slint-backend-selector`, `i-slint-common`, `i-slint-compiler`, `i-slint-core`, and `i-slint-core-macros`, each under `GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0` | `slint`, which only `slang-slint` and the examples package depend on (TOOL-34), on all four tested targets |
| `ISC` | `libloading` | `slang-bindgen` |
| `MIT` | The crates under MIT alone, which are `block2`, `cfg_aliases`, `crunchy`, `dlib`, `libm`, `objc2`, `objc2-encode`, `objc2-foundation`, `ordered-float`, `redox_syscall`, `slab`, `tokio`, which only the WebAssembly target reaches, `wayland-sys`, and `xml-rs`. It also satisfies `termcolor` and `winapi-util`, under `Unlicense OR MIT` | wgpu and winit |
| `Unicode-3.0` | `unicode-ident`, under `(MIT OR Apache-2.0) AND Unicode-3.0` | wgpu |
| `Zlib` | `foldhash` 0.1.5 and 0.2.0, and `slotmap` | wgpu |

Every licence of the table except `GPL-3.0-only` is permissive, as TOOL-34 requires of the dependencies of the crates under permissive licences. Each licence is also compatible with GPL-3.0-or-later, as TOOL-24 requires for the dependencies of `slang-slint` and the examples package. Section 14 of `docs/design/slang-compiler-sys.md` records the licence of each crate of `slang-bindgen`, which is the case for `Apache-2.0` and `ISC`. The licence list of the Free Software Foundation (FSF) gives the evidence for the others [FSF-LICENSES]:

- `MIT`: Its entry for the Expat licence, which is the text that SPDX names `MIT`, calls it a lax, permissive licence compatible with the GNU GPL.
- `Zlib`: Its entry for the licence of zlib calls it a free software licence compatible with the GPL.
- `Unicode-3.0`: Its entry for the Unicode, Inc. License Agreement for Data Files and Software calls it a lax permissive licence compatible with all versions of the GPL, but it does not name version 3 of that licence. The text that `unicode-ident` ships, headed "UNICODE LICENSE V3", grants permission to deal in the data files or software without restriction, on the one condition that the notice appears with all copies, so it is a permissive grant of the same kind.
- `BSD-2-Clause`: Of the FreeBSD license, which it says "is also sometimes called the '2-clause BSD license'", it says: "It is a lax, permissive non-copyleft free software license, compatible with the GNU GPL."
- `BSD-3-Clause`: Of the Modified BSD license, it says: "It is a lax, permissive non-copyleft free software license, compatible with the GNU GPL. This license is sometimes referred to as the 3-clause BSD license."
- `BSL-1.0`: Of the Boost Software License, it says: "This is a lax, permissive non-copyleft free software license, compatible with the GNU GPL."

The owner approved `GPL-3.0-only`, and `slang-slint` and the examples package take the crates of Slint under it, with the consequence that Section 4.2 of `docs/design/slang-slint.md` states. The evidence of compatibility is this:

- The licence text that `slint` 1.18.1 and `i-slint-core` 1.18.1 ship, in `LICENSES/GPL-3.0-only.txt`, is version 3 of the GNU General Public License, dated 29 June 2007. It differs from the repository's `LICENSE-GPL` only in formatting and quotation marks.
- The FSF licence list lists version 3 of the GNU GPL as a free software licence and a copyleft licence, and recommends it for most software [FSF-LICENSES].
- The compatibility matrix of the GPL FAQ permits copying code under GPLv3 into code that is licensed under GPLv3 or later [GPL-FAQ]. The combination is then distributed under version 3 only, which is the consequence that Section 4.2 of that design states.

The owner also approved `BSD-2-Clause`, `BSD-3-Clause`, and `BSL-1.0`.

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

The `ignore` list of the `[advisories]` table holds six advisories by the owner's decisions, each with its reason, as TOOL-26 permits. Each is for a crate that is unmaintained, with no upgrade available and no vulnerability reported. TOOL-31 fails the check once an entry no longer applies, so each entry is removed when its condition holds:

| Advisory | Crate | How it enters | Removed when |
| --- | --- | --- | --- |
| RUSTSEC-2026-0441 | `pin-utils` 0.1.1, whose repository is archived | `spin_on` 0.1.1, which `slint-macros` 1.18.1 requires unconditionally, so no feature set of Slint avoids it. It runs only at build time, inside a procedural macro | A release of Slint no longer depends on `pin-utils` |
| RUSTSEC-2026-0370 | `zvariant` 5.15 | Slint's winit backend, in the unpublished examples package only (Section 3.6 of `docs/design/slang-slint.md`) | `Cargo.lock` no longer holds `zvariant` 5.15, such as after a release of Slint's winit backend that requires another `zbus` |
| RUSTSEC-2026-0371 | `zbus_names` 4.3 | As for RUSTSEC-2026-0370 | `Cargo.lock` no longer holds `zbus_names` 4.3, as for RUSTSEC-2026-0370 |
| RUSTSEC-2026-0372 | `zvariant_derive` 5.15 | As for RUSTSEC-2026-0370 | `Cargo.lock` no longer holds `zvariant_derive` 5.15, as for RUSTSEC-2026-0370 |
| RUSTSEC-2026-0192 | `ttf-parser` 0.25 | Slint's winit backend, through `sctk-adwaita` 0.10, in the unpublished examples package only | `Cargo.lock` no longer holds `ttf-parser` 0.25, such as after a release of winit or Slint without `sctk-adwaita` 0.10 |
| RUSTSEC-2026-0543 | `chrono` 0.4.45. The advisory affects every version of `chrono` | `i-slint-core` 1.18.1, which `slint` 1.18.1 requires in `slang-slint` and the unpublished examples package. No published crate depends on it | A release of Slint no longer depends on `chrono` |

A Linux build of Slint links fontconfig, so the Linux jobs install its development files (Section 7.1). `check-deny` fetches the advisory database on every run, so it needs network access (Section 14.6).

## 9. Publishing and Releases

The architecture design owns the publishing policy in its Section 13 and the go-ahead rule ARCH-24. This section defines how the repository is kept ready to publish, which crates a release publishes and at which versions, the check for breaking changes, the hosted documentation, the review of the public API, and the release procedure.

### 9.1. Metadata and Licence Files

TOOL-4. The `[workspace.package]` table MUST declare the `license` field as the SPDX licence expression `MIT OR Apache-2.0`, the `rust-version` field as `1.99`, and the `repository` field as the address of the repository on GitHub. Every member MUST inherit these fields from the workspace instead of setting its own, because a member with its own value can disagree with the licence that ARCH-36 of the architecture design assigns and with the minimum supported Rust version (MSRV) that Clippy reads.

No member inherits a `readme` field. Each library crate sets `readme = "README.md"` to its own crate README, as RDM-15 of `docs/writing-readmes.md` requires, because crates.io shows that file as the page of the crate. The examples package has no README.

The one exception is the `license` field of a member that ARCH-36 assigns another licence. `slang-compiler-sys` and every later `-sys` or `-src` crate MUST set it to `Apache-2.0 WITH LLVM-exception`, and `slang-slint` MUST set it to `GPL-3.0-or-later`.

TOOL-5. An agent MUST NOT change `rust-version` except in a change whose stated purpose is to raise it, and that change updates Section 14.3 and the section `Requirements` of the root `README.md`, which is the home of the MSRV for users (Section 4.1 of `docs/writing-readmes.md`). The MSRV is a promise to users (DEP-4 of the coding standard), and a raise hidden in another change breaks their builds without notice.

TOOL-27. Every published crate MUST carry the metadata fields that the row of `clippy::cargo_common_metadata` in Section 4.5 of the coding standard lists, inheriting those that TOOL-4 places in `[workspace.package]` and setting `description`, `keywords`, `categories`, and the `documentation` field of TOOL-40 itself. Every member that the next release does not publish MUST set `publish = false`, as TOOL-35 states. Clippy reports a missing field on every member that does not set `publish = false`, so the check fails until the metadata is complete.

TOOL-3. An agent MUST create the examples as one workspace member, the package `slang-rust-examples` at `examples/`, with `publish = false`, in which each example is a binary target. One package needs one manifest with one `publish = false`, one integration-test binary under TEST-10 of the coding standard, and one place for shared helpers.

Building one example builds the dependencies of every example, which the examples accept, because ARCH-14 of the architecture design limits the dependencies of `slang-compiler` and no rule limits those of the examples.

Cargo resolves the `readme` field of a member against its package directory, so `readme = "README.md"` names the crate README, and Cargo packages that file.

Cargo refuses to package a crate whose path dependency has no version (TOOL-25). Packaging one crate that depends on a member that is not published yet fails, while `cargo package --workspace` succeeds.

TOOL-33. The repository root MUST hold the text of each licence of ARCH-36 once: `LICENSE-APACHE` holds the Apache License 2.0, `LICENSE-MIT` the MIT licence, `LICENSE-APACHE-LLVM` the Apache License 2.0 with the LLVM exception, and `LICENSE-GPL` version 3 of the GNU General Public License. Each package directory, including `crates/slang-bindgen/` and `examples/`, MUST hold its own licence under the conventional name as a relative symbolic link to the root file. The names are `LICENSE-APACHE` and `LICENSE-MIT` for a package under `MIT OR Apache-2.0`, and `LICENSE` for a crate under one licence.

Cargo does not include a root licence file in a packaged crate when the manifest sets only the `license` field, while it does package the files of the crate directory. `cargo package` follows a symbolic link, so a packaged crate holds the full licence text as a regular file. On a checkout without symbolic link support, such a file holds only the path of its target, so a release MUST be packaged on Linux or macOS.

The root texts are these:

- `LICENSE-GPL`: The former root `LICENSE`, unchanged.
- `LICENSE-APACHE`: The Apache License 2.0 as the Apache Software Foundation publishes it.
- `LICENSE-MIT`: The SPDX text of the MIT licence, with the line `Copyright (c) 2026 g-whiz`.
- `LICENSE-APACHE-LLVM`: The Apache License 2.0, followed by the SPDX text of the LLVM exception.

The repository has no `NOTICE` file.

`slang-bindgen`, `slang-src`, and `slang-glslang-sys` are published crates (Section 3.1 of the architecture design), so TOOL-27 applies to them. Every release of `slang-src` needs a release of each crate that requires it exactly (SRC-14 of `docs/design/slang-src.md`, ARCH-32 of the architecture design), which TOOL-36 covers.

### 9.2. The Crates of a Release and Their Versions

TOOL-35. The `justfile` MUST hold the packages of the next release once, in the variable `release_packages`, in the order in which they depend on each other, and every recipe of the release procedure MUST read them from there. Every other member MUST set `publish = false` in its manifest. A second list can diverge from the first. Cargo refuses to publish a member with `publish = false` even when a command names it, so the manifest and not care keeps a member out of a release.

For the first release, `release_packages` holds `slang-bindgen slang-src slang-compiler-sys slang-glslang-sys slang-compiler slango slango-codegen slango-macros`, the crates that Section 13 of the architecture design assigns to it. These members set `publish = false`:

- The examples package, which is never published.
- `slang-slint`, which belongs to the second release. The change that prepares the second release removes the field and adds the crate to `release_packages`.
- `slang-rhi-sys` and `slang-rhi`, which M9 adds and which belong to the second release in the same way (Section 13 of the architecture design).

`check-package` fails when the members without `publish = false` differ from `release_packages` (Section 9.6).

TOOL-36. Every release MUST publish every crate of `release_packages` at a version that crates.io does not hold yet. The crates other than `slang-src` MUST share the version of `[workspace.package]`, and every requirement between members MUST name the version of the same release. `slang-src` MUST take the version that SRC-13 of `docs/design/slang-src.md` derives from the version of the release and the recorded Slang.

The reason is the exact requirements of the workspace. Every requirement between members is exact while the releases are alphas (TOOL-37), and these four are exact for reasons of their own, which hold for any release:

| Crate | Requires exactly | Where the requirement is declared |
| --- | --- | --- |
| `slang-compiler-sys` | `slang-bindgen`, as a build dependency, and `slang-src` | `[workspace.dependencies]` |
| `slang-glslang-sys` | `slang-src` | `[workspace.dependencies]` |
| `slango-macros` | `slango-codegen` | `[workspace.dependencies]` |
| `slango-codegen` | `slango` | The manifest of `slango-codegen` (SLM-2 of `docs/design/slango-macros.md`) |

A crate that requires another exactly cannot move to a new version of it without a release of its own. Publishing every crate in every release, at one shared version, is the simplest rule that keeps every exact requirement satisfiable, and the tag of a release then names the version of every crate except `slang-src`. The cost, a new version of a crate that did not change, is a choice of the design.

Every crate stays below version 1.0 (Section 13 of the architecture design). Below 1.0, Cargo treats a change of the minor number as breaking and a change of the patch number as compatible [CARGO-SEMVER].

TOOL-37. Every release MUST be an alpha release, with the version `0.1.0-alpha.<N>`, until the owner decides otherwise. `<N>` starts at 1 and rises by one with each release of the whole set of crates. The public API of an alpha is unstable and may change in any release, so a later version can break a dependent, as the section `Status` of the root `README.md` tells users.

These rules carry out the policy:

- The change that follows a release MUST raise `<N>` by one in the version of `[workspace.package]`, in every requirement between members, and in the version of `slang-src`, whose release identifiers come first and whose Slang identifiers stay last (SRC-13 of `docs/design/slang-src.md`). It also raises each version that the API documentation and each crate README name, as step 1 of Section 9.6 requires. The root README is raised by the change that follows a publication (RDM-33 of `docs/writing-readmes.md`).
- Every requirement of one member on another MUST be exact, written with `=`, such as `"=0.1.0-alpha.1"`. An alpha makes no promise of compatibility, and a caret requirement on a pre-release also accepts every later pre-release of the same version (Section 3.1 of `docs/design/slang-src.md`), which could pair a crate with an incompatible alpha of another.
- A release that fails or turns out wrong is followed by the next alpha. No version is published twice, as Sections 9.7 and 9.8 apply.

The open questions that the review of Section 9.5 defers MUST be decided before the first release that is not an alpha, because that release promises compatibility for its API.

`check-semver` compares each crate with the baseline that Section 9.3 defines. While every release is an alpha, crates.io holds only pre-releases of these crates, so the baseline of each crate is its newest version, which is the previous alpha. Section 9.3 states what the check reports between two alphas.

### 9.3. The Check for Breaking Changes

TOOL-38. The recipe `check-semver` MUST run cargo-semver-checks on every crate of `release_packages` that has a library target, except `slang-src` and `slang-glslang-sys`, against the last release on crates.io, with the default features of each crate and no other feature. Without an explicit choice, the tool enables every feature whose name does not look unstable [CSC-README], which would turn on `vendored` and clone and build Slang from source for each crate and again for each baseline.

The recipe runs this command, which Section 4.8 of the coding standard, the owner of the command lines, also records (TOOL-7):

```sh
cargo semver-checks --workspace --exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi --default-features
```

The command selects the crates in this way:

- In a workspace, the tool skips every member with `publish = false` unless a command names it [CSC-SRC], and every member without a library target. `--workspace` therefore selects the crates of `release_packages` that have a library target (TOOL-35). The exclusions are those of TOOL-32.
- The baseline of each crate is the newest version on crates.io that is neither a pre-release nor yanked, and otherwise the newest version [CSC-SRC].
- With the default features, `slango` builds with its feature `wgpu` (SLO-28 of `docs/design/slango.md`), and no crate enables `vendored`. Every build takes the dynamic route, so the recipe needs `VULKAN_SDK` and libclang for the current crates and for the baselines.

The help text of `cargo semver-checks check-release` of version 0.51.0 lists `--workspace`, `--exclude`, and `--default-features` in the meaning that TOOL-38 gives them, which confirms TA-10. The tool builds the current crates and the baselines with the environment of the recipe, which confirms TA-11. Without `VULKAN_SDK`, a check of `slang-compiler-sys` failed with exit status 101 and the error of its build script that `VULKAN_SDK` is not set.

The exit status is 0 when the tool finds no breaking change, 100 when it finds one, and 101 when the check cannot complete [CSC-README].

While every release is an alpha, the check reports no change of the API. cargo-semver-checks 0.51.0 judges the step from `0.1.0-alpha.1` to `0.1.0-alpha.2` to be a major change, then runs none of its checks and exits with status 0. A method of `slang-bindgen` renamed in a scratch copy of the workspace gave "0 checks: 0 pass, 260 skip" and "no semver update required", as did every crate in the run of the recipe on the unchanged API.

So while the releases are alphas, the recipe fails only in these cases, each with exit status 101:

- A current crate or a baseline does not build with the environment of the recipe, such as without `VULKAN_SDK` or libclang.
- crates.io does not hold a crate that the command selects.
- The machine has no network access.

The tool runs its checks only on a step that it takes to be minor or patch. Its option `--release-type` sets the kind of step instead of deriving it from the versions. With `--release-type minor`, between the same two alphas, it ran its checks, failed the renamed method with the finding `inherent_method_missing` and exit status 100, and passed an added method with exit status 0. The recipe does not pass the option, and whether it should while the releases are alphas is TQ-17.

`slang-src` and `slang-glslang-sys` are left out, because the check cannot help with them. They build only on the vendored route, so checking them would clone and build Slang and slang-glslang for the current crates and again for the baselines in every run. Their Rust API is empty or one foreign function, and their real interface is their `links` metadata and their build behaviour, which the tool cannot see. ARCH-32 of the architecture design and SRC-13 of `docs/design/slang-src.md` govern their versions.

The check also cannot detect a change of the generated bindings that a new recorded Slang causes. The current `slang-compiler-sys` and its baseline both generate their bindings from the headers of the same installed Slang, so both sides show the same bindings. Whether a move of the recorded Slang is breaking stays with ARCH-32 and the change that moves it (Section 8.2 of `docs/design/slang-compiler-sys.md`).

The tool checks only crates with a library target, so it does not check `slango-macros`, a procedural macro crate. Under `--workspace` it skips the crate, and a command that names the crate alone fails with "no crates with library targets selected, nothing to semver-check" and exit status 101. No check therefore sees the macro `shader!`, the grammar of its input, or the shape of the generated code.

The generated code calls the hidden module `__private` of `slango`, which is exempt from semantic versioning (SLM-6 of `docs/design/slango-macros.md`), and the exact requirements of TOOL-36 tie the three crates together. `slango-codegen` is a library, and the tool checks it like any other.

The tool treats an item with `#[doc(hidden)]` as outside the public API, which confirms TA-12. In a scratch copy, with `--release-type minor` so that the tool ran its checks, a renamed function of `slango::__private` passed with exit status 0, while a renamed `Sampler::new` of the same crate failed with exit status 100.

TOOL-39. The CI job `semver` MUST run `just check-semver` in every run of the workflow, so that a breaking change reaches no release unseen.

The recipe and the job could not exist before the first release, because no crate had a version to compare with. cargo-semver-checks fails each such crate with "not found in registry" [CSC-SRC] and has no option to skip such a crate. Its option `--baseline-rev` compares with a git revision instead, but it fails on a second run when the target directory is inside the workspace, and a comparison with `main` reports every change to the API, which tells a reviewer nothing.

`check-semver` stays outside `check`, as `check-features` and `check-msrv` do. It fetches each baseline from crates.io, so it needs network access, and it builds every crate twice, once for the current version and once for the baseline. CI runs it in every run of the workflow.

The tool builds each checked crate and each baseline in a target directory of its own under `target/semver-checks/`. The build script of `slang-compiler-sys` therefore runs once for each side of each checked crate that depends on it.

### 9.4. Hosted Documentation

TOOL-40. The manifest of each crate of `release_packages` MUST set the `documentation` field to the address of its documentation on the hosted site, `https://g-whiz.github.io/slang-rust/<crate>/`, in which `<crate>` is the crate name with each hyphen replaced by an underscore. Without the field, crates.io links each crate to docs.rs [CARGO-MANIFEST], which cannot build these crates.

The addresses for the first release are these:

| Crate | `documentation` |
| --- | --- |
| `slang-bindgen` | `https://g-whiz.github.io/slang-rust/slang_bindgen/` |
| `slang-src` | `https://g-whiz.github.io/slang-rust/slang_src/` |
| `slang-compiler-sys` | `https://g-whiz.github.io/slang-rust/slang_compiler_sys/` |
| `slang-glslang-sys` | `https://g-whiz.github.io/slang-rust/slang_glslang_sys/` |
| `slang-compiler` | `https://g-whiz.github.io/slang-rust/slang_compiler/` |
| `slango` | `https://g-whiz.github.io/slang-rust/slango/` |
| `slango-codegen` | `https://g-whiz.github.io/slang-rust/slango_codegen/` |
| `slango-macros` | `https://g-whiz.github.io/slang-rust/slango_macros/` |

The host is the default address of GitHub Pages for the project site of the repository `g-whiz/slang-rust`. A custom domain would change every field and is not part of this design. `check-package` checks each field before the publish, and `check-release` checks the field that crates.io shows after it (Section 9.6).

The `justfile` holds the base address `https://g-whiz.github.io/slang-rust` once, in the variable `documentation_site`, and passes it to both recipes, by a choice of the design. Each script derives the address of a crate as `<documentation_site>/<name>/`, with each hyphen of the crate name replaced by an underscore, so the two checks cannot use different addresses. The manifests still spell out each address, because Cargo reads no variable of the `justfile`.

The site holds the documentation of the latest release only, built from its tag (Section 7.5). A user who follows the link of the current version reads the API that the version has, while a site built from `main` would describe API that no release has. The link of an older version then shows the documentation of a newer one, which TQ-12 leaves open.

TOOL-41. The recipe `doc-site` MUST build the site with the documentation command of Section 4.8 of the coding standard, under a departure from TOOL-7 like that of the vendored recipes. It runs the command with `-p` for every package of `release_packages` in place of the workspace options, with the feature `vendored` of each of those packages that declares one, and with the value of the `justfile` variable `vendored_environment` before it. It then writes the index page and checks the site.

The recipe reads the features of the selected packages from `cargo metadata --no-deps` with `python3 -I`, and passes no `--features` when none declares `vendored`. A fixed list of features would name a package that `release_packages` no longer holds, which Cargo refuses with "none of the selected packages contains this feature", as the recipe did once `slango-macros` left `release_packages` (TOOL-54). Of the five crates of `release_packages` until M11, `slang-compiler-sys` and `slang-compiler` declare `vendored`, so the command passes those two features. On 2026-11-18, `doc-site` built a site of 45 MB with the five crates.

For the first release, the command was this:

```sh
env -u VULKAN_SDK -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH CARGO_TARGET_DIR=target/vendored CARGO_BUILD_WARNINGS=deny cargo doc --no-deps -p slang-bindgen -p slang-src -p slang-compiler-sys -p slang-glslang-sys -p slang-compiler -p slango -p slango-codegen -p slango-macros --features slang-compiler/vendored,slango-macros/vendored
```

The site is built on the vendored route, by a choice of the design, for these reasons:

- `slang-src` and `slang-glslang-sys` build only on that route (TOOL-32), and one command writes all eight crates into one directory with one search index.
- The dynamic route for six crates and the vendored route for two would need both the Vulkan SDK and the vendored tools, and two runs of rustdoc.
- `check-vendored` already builds the documentation of four of the crates on this route.

The cost is the clone of Slang and the build of Slang and slang-glslang in a run from an empty target directory, which takes about as long as `check-vendored`.

In the development container, the one command documented all eight crates into one site with 0 warnings. From an empty target directory on 16 cores, the recipe took 7 min 46 s, including the clone and both native builds, and the site took 27 MB with no symbolic link. On a hosted Linux runner, the first run of the documentation workflow, which caches nothing, completed its job `build` within the timeout of 60 minutes, which confirms TA-13.

The command enables `slango-macros/vendored` as well, because the copy of Slang that the macro links is a host dependency, which the feature of `slang-compiler` does not reach (Section 4.3 of `docs/design/slango-macros.md`). Without it, and without `VULKAN_SDK`, the build of the macro stops. The public API of each crate is the same on both routes, so the site describes what a user of either route gets.

After the command, the recipe runs `scripts/check-doc-site` with the directory `target/vendored/doc`, the size limit in KiB, and the crates of `release_packages`. The script does the rest of the recipe's work, and `release-scripts-self-test` tests it (Section 9.9).

The script first checks that `target/vendored/doc/<crate>/index.html` exists for each crate, with `<crate>` as in TOOL-40. For each missing page it prints "check-doc-site: cargo doc wrote no page" with the path, and it fails before it writes the index page. A link of the index page or a `documentation` field would otherwise lead to a missing page.

rustdoc's own index page needs unstable options [RUSTDOC-UNSTABLE]. The script therefore writes `target/vendored/doc/index.html`, a plain HTML page with one link to the documentation of each crate of `release_packages`, in that order, and nothing else. The recipe generates the page in every run, and no file of the repository holds it (Hard Rule 10 of `AGENTS.md`).

The script then checks the site against the limits of GitHub Pages:

- It fails when `target/vendored/doc` holds a symbolic link, which the artifact of the deployment may not contain [UPLOAD-PAGES-ARTIFACT].
- It prints the size of the site with `du -sh`, and fails when the site is larger than 1 GB, the limit of a published site [GH-PAGES-LIMITS].

The recipe takes 1 GB as 10^9 bytes, by a choice of the design, and passes the limit 976562 KiB, the largest whole number of KiB below 10^9 bytes. The script measures the disk usage with `du -sk` and fails above the limit. Disk usage counts whole blocks, so it is larger than the sum of the file sizes, and the check errs on the safe side.

`doc-site` builds only, and deploys nothing. The documentation workflow of Section 7.5 runs it and deploys its output.

rustdoc never removes the output of an earlier run, and `doc-site` does not clean `target/vendored/doc` before it builds. The documentation command of `check-vendored` writes into the same directory with another selection of crates and features, so a local site can hold pages that the release build did not produce, such as a second source page of the generated bindings from a build of `slang-compiler-sys` with other features.

The checks of the size and of symbolic links then cover those pages as well. The workflow builds from an empty directory (Section 7.5), so a deployed site holds no such page. A reviewer who reads a local site for Section 9.5 can meet such pages, which describe the same crates.

The site embeds the absolute path of the build directory. rustdoc writes a source page for each file that a crate compiles, under a directory that mirrors the file's path, and `slang-compiler-sys` includes its generated bindings from `OUT_DIR`, which is an absolute path in `target/vendored`.

The source page of the bindings therefore sits under a directory that repeats the absolute `OUT_DIR`, and the "Source" link of every item page of `slang-compiler-sys`, and the implementor files under `type.impl/`, name that path. In the development container, 447 files of the site named it, of which 435 were item pages of `slang-compiler-sys`. No search index file named it.

No stable option of rustdoc 1.99.1 or 1.100.0 removes the path:

| Option | Result |
| --- | --- |
| `--remap-path-prefix` | Stable, and accepted, but the source page and the item pages keep the absolute path. Its help text says that it remaps source names in compiler messages |
| `--remap-path-scope` | Needs `-Z unstable-options` |
| `--html-no-source` | Needs `-Z unstable-options`. It would also remove every source page and every "Source" link of every crate |

`cargo doc` has no option on paths or source pages. A nightly toolchain builds no crate (DEP-4 of the coding standard), so the unstable options are ruled out.

The design accepts the path, by a choice of the design, which is the simplest sound option. A site that the workflow builds names the path of the hosted runner, such as `/home/runner/work/slang-rust/slang-rust/target/vendored/debug/build/slang-compiler-sys-<hash>/out`, which names no person and no machine of a developer. A site that `doc-site` builds on a developer's machine names that machine's path, which can hold a user name, so it is never deployed.

TOOL-49. The hosted site MUST be deployed only by the documentation workflow of Section 7.5, from the site that its own job `build` produced. A site that `doc-site` built on any other machine MUST NOT be deployed, uploaded, or published elsewhere. Such a site names the absolute build path of that machine in its pages, and can hold stale pages of an earlier build.

TQ-13 asks for a stable way to remove the path.

docs.rs builds the documentation of every version published on crates.io, with no network access [DOCSRS-BUILDS], and offers no documented setting that stops the build of a crate. This confirms A-25 of the architecture design. docs.rs is not supported (Section 13 of the architecture design), so a user who opens docs.rs for one of these crates finds a failed build:

- The build of `slang-compiler-sys`, and of every crate that depends on it, stops in its build script on the dynamic route without `VULKAN_SDK`, or without libclang (Section 9.6 of `docs/design/slang-compiler-sys.md`).
- The build of `slang-src`, and of `slang-glslang-sys`, needs the network to clone Slang.

A failed build on docs.rs shows a page that says that docs.rs failed to build the crate, with a link to the log, as an example page of another crate shows. `slang-bindgen` has no build script that needs Slang, so docs.rs may build it, and this design depends on neither outcome. The crates.io page of each crate links to the hosted site through its `documentation` field (TOOL-40), and no manifest has a `[package.metadata.docs.rs]` table or a code path for docs.rs.

### 9.5. Review of the Public API

TOOL-43. Before every release, each crate of `release_packages` MUST have a review of its whole public API against the judgement rules of this section. Each finding of a broken rule MUST be fixed before the release, or deferred by the owner and recorded as an open question in the design document of the crate. A published version locks in its API, and a tool checks only the rules that it implements (Hard Rule 4 of `AGENTS.md`).

The review checks these rules, and no rule that a configured tool already reports:

- The rules of Appendix A of the coding standard that concern the public API: API-1 to API-16, TYPE-1 to TYPE-4, TYPE-8, ERR-1 to ERR-4, CONC-1, CONC-6, CONC-7, DEP-1, DEP-3, SIMP-1, and SIMP-2.
- The rules of Section 7 of the writing guide on API documentation that no checker enforces (Section 7.12 of the writing guide).
- The items of the checklist of the Rust API Guidelines, IR-8 of `docs/rust-references.md` [API-GUIDELINES], that the rules above do not cover and no tool checks, such as C-QUESTION-MARK, C-FAILURE, C-LINK, and C-HIDDEN.

These tools already cover API hygiene, so the review does not repeat them:

| Tool | What it covers |
| --- | --- |
| `clippy::exhaustive_enums`, `clippy::exhaustive_structs` | A decision about `#[non_exhaustive]` on every public enumeration and every struct whose fields are all public (API-9, C-STRUCT-PRIVATE) |
| `clippy::partial_pub_fields` | Structs that mix public and private fields |
| rustc `missing_docs`, `missing_debug_implementations`, `unreachable_pub`, `unnameable_types` | Documentation on every public item, `Debug` on every public type (C-DEBUG), and the visibility of items |
| The pedantic group of Clippy, such as `missing_errors_doc`, `missing_panics_doc`, and `must_use_candidate` | The `# Errors` and `# Panics` sections and `#[must_use]` |
| The rustdoc lints | Broken intra-doc links and other faults of the documentation |
| `clippy::cargo_common_metadata` | The metadata of each manifest (C-METADATA) |
| `check-doc-headings`, `check-feature-docs`, `check-doctest-attributes`, `check-prose` | The headings of item documentation, the `# Features` section, `ignore` in doctests, and the prose of API documentation |
| `check-semver` | Breaking changes against the last release, in the crates with a library target. While the releases are alphas, it runs none of its checks (Section 9.3) |

The agent runs the review once for each crate. The `review` skill reviews the diff of a branch, so it does not fit a review of a whole surface. The agent therefore gives each reviewer the crate's documentation as `doc-site` writes it, the sources of the crate's public items, and the list of rules above, and the reviewer cites the rule identifier or checklist item of each finding, as in a review of a diff.

The review writes no document (Hard Rule 10 of `AGENTS.md`). Its findings go into the agent's report to the owner, each becomes a fix on a feature branch or a deferred open question, and the report under `.code-review/`, when one is written, stays local and ignored by git. A finding of a broken rule blocks the release until it is fixed or the owner defers it, and a finding that breaks no rule, such as a preference of wording, does not.

### 9.6. Release Procedure

TOOL-44. A release MUST follow the steps of this section in their order. An agent MUST NOT perform a step that this section reserves for the owner, or that needs the owner's go-ahead, without the owner's explicit go-ahead in the current session. A published version can never be overwritten or deleted [CARGO-PUBLISHING], so a step out of order can publish a version that stays wrong.

The steps are these, and the third column says who performs each:

| Step | Action | Performed by |
| --- | --- | --- |
| 1 | Check the preconditions below | An agent |
| 2 | Draft the release notes | An agent, through the `writer` agent |
| 3 | Run `just check-package` | An agent |
| 4 | Run `just doc-site` | An agent |
| 5 | Run `just publish-dry-run` | An agent |
| 6 | For the first release only, set the source of GitHub Pages to "GitHub Actions" in the settings of the repository [GH-PAGES-WORKFLOW], and add to the environment `github-pages` a deployment rule that allows tags that match `v*` (TA-21) | The owner, because it changes settings outside the repository's files |
| 7 | Give the go-ahead to publish the commit of steps 3 to 5 (ARCH-24 of the architecture design) | The owner |
| 8 | Run `just publish` with a token (Section 9.7) | The owner |
| 9 | Run `just tag-release <notes>`, which tags the commit, creates the GitHub release, and so deploys the documentation (Section 7.5) | The owner, or an agent after the owner's go-ahead |
| 10 | Run `just check-release` after the documentation workflow of the tag concluded `success` | An agent |
| 11 | Make the change that follows the release: the raise of TOOL-37 and the update of Project Status in `AGENTS.md` | An agent, on a feature branch, as Section 14.7 describes |

The preconditions of step 1 are these:

- `main` holds the change that prepares the release: the versions of TOOL-36, the `documentation` fields of TOOL-40, `release_packages`, and `publish = false` on every other member (TOOL-35).
- The API documentation of the crates names the version of the release in each example that names a version of the crates, such as a dependency or the version in `BUILD_INFO`, and each README names the version that RDM-33 of `docs/writing-readmes.md` requires. No tool checks this, which TQ-18 asks about.
- The review of the public API of Section 9.5 is complete, and every finding that blocks the release is fixed.
- The CI run of the last commit of `main` concluded `success` with every job, as the commands of Section 14.7 show.
- The release steps run in the main checkout at that commit, with no uncommitted change, on Linux or macOS (TOOL-33). The development container meets this.

The raise of TOOL-37 already sets these versions for the next release.

Cargo refuses to package a crate whose packaged files hold an uncommitted change, and no recipe passes `--allow-dirty` (TOOL-45). `check-package` and `publish-dry-run` therefore run only on a committed tree, and fail on a branch before the commit of a change to a manifest or another packaged file. A change that prepares a release, or a fix of path 2 of Section 9.7, is committed before an agent runs them on it, and steps 3 to 5 run on the commit of `main` that the owner publishes.

The release notes of step 2 go into the body of the GitHub release of step 9, from a file outside the repository, and summarize the changes since the previous tag, or the crates and their purpose for the first release. The project keeps no changelog file, because Hard Rule 10 of `AGENTS.md` admits no document outside the root `README.md`, the `README.md` of each library crate, `AGENTS.md`, `CLAUDE.md`, and `docs/`, and the GitHub release of each tag holds its notes.

The manifests already carry the metadata that crates.io shows: `description`, `license`, `repository`, `keywords`, `categories`, `readme`, and `rust-version`, and the change that prepares the first release adds `documentation`. No crate sets `homepage`, because crates.io shows the repository, and the hosted site is the `documentation` link.

These recipes carry out the release. The recipes `check-package`, `publish-dry-run`, and `publish`, which package, are available only on Linux and macOS, through the `[unix]` attribute of `just`, so that packaging follows the symbolic links of TOOL-33:

| Recipe | What it runs |
| --- | --- |
| `check-package` | It reads the members from `cargo metadata --no-deps --format-version 1` and fails when those without `publish = false` differ from `release_packages`, compared as sets. It fails when the `documentation` field of a crate differs from its address under `documentation_site` (TOOL-40, Section 9.4), and stops before packaging on either failure. It prints `cargo package --list` for each crate, runs one `cargo package --no-verify` with `-p` for each crate, and checks each `.crate` file under the `package` directory of the target directory as TOOL-45 requires. It reports every failed condition and then exits with status 1, and exits with status 2 on a usage error or a missing tool |
| `publish-dry-run` | `cargo publish --dry-run` with `-p` for each package that it is given, by default every crate of `release_packages`, which packages and compiles every given crate in the order of their dependencies, as if the others were published [RUST-1.90]. A dependency outside the given list resolves to its version on crates.io (TA-17). It needs the Vulkan SDK and libclang for the dynamic route, and network access and the vendored tools for `slang-src` and `slang-glslang-sys`, which the development container provides. It first removes any earlier record. On success, it writes the record `target/publish-dry-run.commit` with two lines, `commit=<full hash of HEAD>` and `packages=<the given packages, separated by spaces, in the given order>` |
| `publish` | It refuses to run, with exit status 1, in this order: when `GITHUB_ACTIONS` is set, even to an empty value, when `CARGO_REGISTRY_TOKEN` is unset or empty, when `git status --porcelain --untracked-files=normal` fails or reports an uncommitted change or an untracked file, when `git fetch origin main` fails, when `HEAD` differs from `origin/main`, when the record is missing, when the recorded commit differs from `HEAD`, and when the recorded list differs from the packages that it is given, by default every crate of `release_packages`. The lists are compared as exact strings, so the same crates in another order also refuse. It then reads the version of each package from `cargo metadata`, asks crates.io which versions it holds, and publishes the others one at a time, as TOOL-51 of Section 9.7 specifies |
| `tag-release` | It takes the path of the notes file, relative to the directory where `just` runs. It refuses to run, with exit status 1, when the notes file is not a readable, non-empty regular file, before any call of git, so that a missing file cannot stop `gh` after the tag is pushed. It then runs `git fetch origin main` and refuses when the fetch fails or `HEAD` differs from `origin/main`. It reads from `cargo metadata` the one version that every crate of `release_packages` except `slang-src` shares, and refuses when they differ or when a crate is not a member. It creates the annotated tag `v<version>` on `HEAD` with the message `v<version>`, pushes `refs/tags/v<version>` to `origin`, and runs `gh release create v<version> --verify-tag --title v<version> --notes-file <notes>`. When the version holds a hyphen, which marks a SemVer pre-release such as `0.1.0-alpha.1`, it adds `--prerelease`, so that GitHub marks the release as a pre-release |
| `check-release` | For each crate of `release_packages`, it reads the version of the manifest from `cargo metadata` and asks `https://crates.io/api/v1/crates/<crate>/<version>`, then prints `<crate> <version>: on crates.io` or `<crate> <version>: not on crates.io`, so that its output also lists the crates that a failed publish left out (Section 9.7). For a crate that crates.io holds, it reads the `documentation` field from `https://crates.io/api/v1/crates/<crate>` and requests that address. It fails when crates.io lacks the version, when the field differs from the address under `documentation_site` (Section 9.4), or when the address does not answer with HTTP status 200, and it checks every crate before it exits |
| `yank` | It takes exactly two arguments and runs `cargo yank --version <version> <package>`, with the checks of `publish` on `GITHUB_ACTIONS` and `CARGO_REGISTRY_TOKEN` (Section 9.8) |

The choices of the design behind these rows:

- `tag-release` reads the version from the crates and not from `[workspace.package]`, because `cargo metadata` does not show the table. TOOL-36 makes every crate except `slang-src` carry that version, so the shared version is the same value, and a difference shows a broken release.
- `tag-release` fetches `origin/main` before it compares, as `publish` does, because a stale `origin/main` would let it tag a commit that `main` no longer points to.
- `publish` reads "no uncommitted change" as no change and no untracked file, the stricter reading.
- `publish` and `yank` treat `GITHUB_ACTIONS` as set when it holds any value, including the empty string.
- `check-release` and `publish` send the user agent `slang-rust-release-check`, which names the application and no person, and wait one second before each request to the crates.io API. The data access policy of crates.io requires a user agent that identifies the application and at most one request per second, and only suggests contact details. The policy sets no request limit on `static.crates.io` [CRATES-DATA-ACCESS].
- `check-package` decides whether crates.io holds the version of a crate by the HTTP status of the download of `https://static.crates.io/crates/<crate>/<crate>-<version>.crate`. Status 200 means that it holds the version, and the recipe compares the files. Status 404 means that it does not. Any other status fails the check. The recipe uses no request to the API, which the request limit of the data access policy covers. That limit is separate from the publishing rate limits of Section 9.7.

The crates.io API returns the `documentation` field of a crate at `https://crates.io/api/v1/crates/<crate>` as `.crate.documentation`, and a version at `https://crates.io/api/v1/crates/<crate>/<version>` as `.version.num`. crates.io serves a published `.crate` file at `https://static.crates.io/crates/<crate>/<crate>-<version>.crate`. For a crate that does not exist, all three answer with status 404. A published crate of another project and `slang-bindgen`, before its first publish, showed this.

For a published crate, a version that crates.io does not hold also answers with status 404 at `https://crates.io/api/v1/crates/<crate>/<version>`. The two cases differ only in the detail text of the answer, "crate `<crate>` does not exist" or "crate `<crate>` does not have a version `<version>`". `publish` therefore tells them apart with a second request to `https://crates.io/api/v1/crates/<crate>`, and never by the text. The address of a version also resolves the long pre-release of `slang-src`.

`check-package` has not run since the first release, so it has not yet compared a published `.crate` file of this project.

Every packaged crate holds `Cargo.toml`, the original manifest as `Cargo.toml.orig`, `Cargo.lock`, and `.cargo_vcs_info.json`, besides the files of its package directory, which include its crate README `README.md`. The normalized `Cargo.toml` carries the values that the crate inherits from `[workspace.package]`, including `authors`, where crates.io keeps them for good. The owner keeps the `authors` field as it is, which answers TQ-15.

TOOL-45. A release MUST be packaged on Linux or macOS (TOOL-33), and `check-package` MUST fail on a `.crate` file that breaks one of these conditions:

- It is no larger than 10,000,000 bytes, below the limit of 10 MB that crates.io sets on a `.crate` file [CARGO-PUBLISHING].
- It holds no symbolic link.
- It holds each licence file of the crate under its conventional name (TOOL-33), as a regular file whose content equals the root text.
- It holds `README.md` as a regular file.
- It holds no path under a directory named `target` or `.git`.
- When crates.io already holds the version of the crate, its files equal those of the published `.crate` file, which the recipe downloads, except `.cargo_vcs_info.json`, which names the commit.

`check-package` packages every crate of `release_packages`, including those already published, on purpose. The last condition is how the recovery of Section 9.7 shows that a fix left every published crate unchanged.

The conditions keep out what a package must never hold. Cargo flattens a symbolic link to its target [CARGO-PACKAGE], so a link in the package means a checkout without link support, whose file holds only a path. The generated bindings and the clone of Slang live in `OUT_DIR` and the shared folder of the target directory, never in a package directory (Hard Rule 7 of `AGENTS.md`), and a clone would also exceed the size limit. `cargo package` refuses a working tree with uncommitted changes, because no recipe passes `--allow-dirty`.

`check-package` does not show that a package compiles. `publish-dry-run` shows that every package compiles from its own contents against the packages of the others, that the version requirements between them resolve, and that the build of `slang-src` and `slang-glslang-sys` works from the packaged files. It does not show that crates.io accepts the packages, such as their names and metadata, and it uploads nothing.

With all eight crates, `cargo package` and `cargo publish --dry-run` packaged each crate against the packages of the others and verified them in the order of `release_packages`. For the verification of `slang-glslang-sys`, Cargo unpacked `slang-src` from its local registry under `target/package/tmp-registry`, so the exact requirement on the version of `slang-src` selected that pre-release.

The dry run printed "warning: aborting upload due to dry run" for each crate. This confirms the order and the requirements of TA-17.

A dry run of a subset resolves each dependency outside the subset to its version on crates.io: the dry run of `slango-codegen` and `slango-macros` alone passed while crates.io held `slango`. A dry run of a version that crates.io already holds also passes, and Cargo prints the warning "already exists on crates.io index" for it and goes on, so the dry run of the whole list passes during the recovery of a partial release.

Cargo verifies the packages in the workspace target directory, in `target/debug`, and not under `target/package`. The verification builds two units of `slang-src`: the member of the workspace, and the copy that Cargo unpacked for `slang-glslang-sys`. Where they keep the clone of Slang depends on whether Cargo tagged `target/` (SRC-23 of `docs/design/slang-src.md`):

- When `target/` holds Cargo's tag, both units find the shared folder `target/slang-src` and share one clone and one build of each native library. The shared folder took 364 MB.
- When `target/` holds no tag, each unit keeps its own clone in its `OUT_DIR`, so the dry run clones Slang twice. The two clones took 285 MB and 364 MB.

No recipe creates `target/` other than through Cargo, but Cargo itself leaves it untagged in two cases (Section 3.2 of `docs/design/slang-src.md`). When the first build of a fresh checkout is `test-vendored`, `check-vendored`, or `doc-site`, Cargo creates `target/` only as the parent of the tagged `target/vendored`.

When it is `publish-dry-run`, `cargo publish --dry-run` creates `target/` without a tag. When the first build is `just check` or `check-package`, Cargo tags `target/`. A worktree whose first build was a vendored recipe or `doc-site` therefore clones Slang twice in the dry run.

The design accepts the cost, by a choice of the design, and adds no remedy. The release steps run in the main checkout, whose `target/` an earlier build normally tagged, and the cost is about 650 MB of disk space and a second clone, not a wrong result. TQ-14 asks whether the dry run should build in `target/vendored` instead.

### 9.7. Publishing, Authentication, and Partial Failure

TOOL-46. Every publish and every yank MUST be run by the owner, with an API token of crates.io that is passed in `CARGO_REGISTRY_TOKEN` to that one command. The token MUST NOT be stored in the repository, in a secret of a workflow, in Cargo's credentials file, or in the environment of a process that an agent starts or inherits. A holder of the token can publish versions that can never be deleted, and Hard Rule 7 of `AGENTS.md` keeps every secret out of the repository.

The first publish of a new crate needs an API token, because a trusted publisher can be configured only for a crate that already exists [CRATES-TP-2025, RFC3691]. The owner creates a token for the release, limited to publishing new crates and new versions, to the crates of `release_packages`, and to a short expiry date, and revokes it after the release (TA-18).

The owner runs the publish in a terminal of their own in the development container, and reads the token without echo, so that it appears neither on the command line nor in the shell history:

```sh
read -rs token
CARGO_REGISTRY_TOKEN="${token}" just publish
unset token
```

`publish` passes `--no-verify`, by a choice of the design, so that no build script runs while the token is in the environment. A verification build runs the build script of every dependency, and a build script inherits the environment of Cargo. `publish-dry-run` has already verified the same commit and the same crates, which `publish` checks through `target/publish-dry-run.commit`.

The token stays in the environment of `publish` while it waits for the rate limits of TOOL-51, up to 600 s before each new crate past the burst and up to 1205 s before each retry, and no build script runs during the waits. Every program that the script starts inherits its environment, but only Cargo reads the token. The requests that the script sends to the crates.io API carry no token, and the script prints the token nowhere and runs without shell tracing.

Trusted Publishing from a workflow is deferred, with no change for the first release. It would need a workflow that publishes, which TOOL-22 forbids, a trusted publisher configured on crates.io for each crate, and the action `rust-lang/crates-io-auth-action` [CRATES-AUTH-ACTION] with `id-token: write`. TQ-11 asks whether the second release moves to it.

`publish` runs Cargo once for each crate that it publishes, in the order of its arguments, which by default is that of `release_packages`. For the first release, that order is `slang-bindgen`, `slang-src`, `slang-compiler-sys`, `slang-glslang-sys`, `slang-compiler`, `slango`, `slango-codegen`, and `slango-macros`.

Each run of `cargo publish` waits for its crate to appear in the index before it returns [CARGO-PUBLISH, RUST-1.90] (TA-17), so the next run finds it. `publish` does not reorder its arguments, so a list given by hand keeps the order of `release_packages`. Only `publish-dry-run` passes every crate to one run of Cargo, which orders them by their dependencies.

TOOL-51. `publish` MUST stay within the publishing rate limits of crates.io. It MUST publish no version that crates.io already holds, and MUST publish nothing when it cannot learn which versions crates.io holds, so that running it again after a partial release is safe. A refusal for the rate limit otherwise stops a release part way, and a rerun that reached a published version would stop at it.

crates.io limits publishing for each user account with a leaky bucket [CRATES-RATE-LIMITS]. The values in this table are the defaults of crates.io, which its configuration can change and which crates.io can raise for one user:

| Kind | Burst | Then | Message of a refusal |
| --- | --- | --- | --- |
| New crate | 5 crates | One crate every 600 s | "You have published too many new crates in a short period of time" |
| New version of an existing crate | 30 versions | One version every 60 s | "You have published too many updates to existing crates in a short period of time" |

A refusal is an answer with HTTP status 429 and a `Retry-After` header that holds an HTTP date. Its detail text is the message, followed by "Please try again after `<date>` and see https://crates.io/docs/rate-limits for more details.", in which `<date>` has the form `Fri, 15 Jan 2027 08:02:00 GMT`.

After the refusals of Section 9.6, `publish` behaves as this table states, row by row:

| Step | Behaviour |
| --- | --- |
| Versions | It reads the version of each given package from `cargo metadata --no-deps --format-version 1`. A package that is not a member refuses the run with exit status 1 and "`<name>` is not a member of the workspace, so nothing was published". |
| Check before publishing | Before it publishes any package, it asks `https://crates.io/api/v1/crates/<name>/<version>` for each. Status 200 means that crates.io holds the version. Status 404 leads to a second request to `https://crates.io/api/v1/crates/<name>`, where status 200 means a new version and status 404 means a new crate. It prints one line for each package: "is already on crates.io and is skipped", "is a new version of a crate on crates.io", or "is a new crate on crates.io". |
| Refusal of the check | A failed request, or any other status of either request, refuses the run with exit status 1 and a message that ends "so nothing was published". |
| Requests | Each request follows `sleep 1`, sends the user agent `slang-rust-release-check` and no token, and runs curl with `-q`, which skips the user's `.curlrc`, with HTTPS only, TLS 1.2 or later, a connection timeout of 10 s, a total timeout of 30 s, and 3 retries. |
| Skip | It publishes no package that crates.io holds. |
| Publish | It runs `cargo publish --no-verify -p <name>` once for each other package, in the given order. Cargo's standard output goes to standard output, and its standard error goes to standard error and into a temporary log that the script reads. |
| Pacing | It counts the new crates and the new versions that this run publishes. Before a package of a kind whose count has reached the burst of that kind, it prints "waiting `<interval>` s before the `<kind>` ..." and waits the interval of that kind. |
| Confirmation | When Cargo fails, for any reason, it asks crates.io for that version again. When crates.io answers with status 200, the package counts as published in this run, for the pacing as well, and the script goes on with the next package without a wait. |
| Detection of a refusal | Otherwise, a failure counts as a refusal for the rate limit only when Cargo's output matches the extended regular expression `responded with an error \(status 429` (TA-22). Any other failure stops the run at once, with "cargo publish of `<name>` `<version>` failed with status `<N>`", the report, and Cargo's exit status. |
| Wait | After a refusal, it reads the time of "try again after `<date>`" in Cargo's output with GNU `date -d`, or else with BSD `date -j -f` (TA-23), and waits until that time plus 5 s. When it cannot read the time, it waits the interval of the kind. The wait is at least a tenth of the interval. When that wait would exceed twice the interval plus 5 s, which is when the named time is more than twice the interval away, it does not wait, and stops with a message that names the time, the report, and Cargo's exit status. |
| Retry | After the wait, the burst of that kind counts as used up for the rest of the run, and it publishes the same package again. Each package has at most 3 retries, so Cargo runs at most 4 times for it. When the retries run out, it stops with "crates.io still refuses ... after 3 retries, so run this recipe again later", the report, and Cargo's exit status. |
| Report | Whenever a failure or an interrupt stops it after the check before publishing, it prints three lines on standard error, "published in this run:", "already on crates.io before this run:", and "not published:", each followed by the names of the packages, or by "none". |
| Interrupt | After the check before publishing, `INT` and `TERM` stop the run with "interrupted while publishing `<name>` `<version>`, which crates.io may hold now, so the release stopped", or "interrupted, so the release stopped" between packages, and with the report, in which the package in progress counts as not published. A signal during the check before publishing ends the script without the report, because nothing is published then. |
| Success | It prints "crates.io holds every given package: `<N>` published in this run, `<M>` already there", also when it skipped every package and Cargo never ran. |

The bounds of the waits follow from the limits:

| Kind | Burst | Interval | Shortest wait after a refusal | Longest wait after a refusal |
| --- | --- | --- | --- | --- |
| New crate | 5 | 600 s | 60 s | 1205 s |
| New version | 30 | 60 s | 6 s | 125 s |

These are the exit statuses of `publish`:

| Status | Meaning |
| --- | --- |
| 0 | crates.io holds every given package |
| 1 | A refusal of Section 9.6, or of the check before publishing, before anything is published |
| The status of Cargo | A failure of Cargo that crates.io does not confirm, a refusal for the rate limit after 3 retries, or a named time beyond the longest wait |
| 130 | An interrupt, `INT` |
| 143 | A `TERM` signal |
| 2 | A usage error, or a missing tool |

`publish` needs `cargo`, `git`, `curl`, `python3`, `sleep`, `date`, `tee`, `grep`, and `awk` on `PATH`, and exits with status 2 and "`<tool>` not found on PATH" when one is missing.

This illustration, the output of the script against the stub programs of Section 9.9, shows a rerun of the first release after crates.io accepted its first five crates:

```text
publish: slang-bindgen 0.1.0-alpha.1 is already on crates.io and is skipped
publish: slang-src 0.1.0-alpha.1.slang.2026.17.0.25.gc7954ebd1 is already on crates.io and is skipped
publish: slang-compiler-sys 0.1.0-alpha.1 is already on crates.io and is skipped
publish: slang-glslang-sys 0.1.0-alpha.1 is already on crates.io and is skipped
publish: slang-compiler 0.1.0-alpha.1 is already on crates.io and is skipped
publish: slango 0.1.0-alpha.1 is a new crate on crates.io
publish: slango-codegen 0.1.0-alpha.1 is a new crate on crates.io
publish: slango-macros 0.1.0-alpha.1 is a new crate on crates.io
publish: crates.io holds every given package: 3 published in this run, 5 already there
```

The check runs for every package before the first publish, by a choice of the design, because only then can the script refuse before anything is published when crates.io cannot answer. A publish without the check could reach a version that crates.io holds, which Cargo refuses part way through, while a rerun repeats the check at no cost.

Cargo can fail after a successful upload, such as while it waits for the index, and its output can then hold a page with status 429 from the content delivery network of the index. The pattern therefore matches only an error of the API, and the confirmation counts such an upload as published instead of publishing it again.

The 5 s margin covers a difference between the clock of the machine and that of crates.io, and the shortest wait keeps a named time in the past from an immediate retry. A named time further away than the limits explain stops the run, so that the owner decides when to run it again, instead of the script holding the token for that long. After a refusal, the burst of the kind counts as used up, so that the next package of that kind does not meet a refusal of its own and spend a retry.

Publishing several crates is not atomic [RUST-1.90], and a published version can never be overwritten or deleted [CARGO-PUBLISHING]. When one crate fails, the crates before it are public for good. The crates that are not published yet are called the remaining crates in this section.

`publish` skips every version that crates.io holds (TOOL-51), and a dry run of such a version passes (Section 9.6). The recovery therefore runs `publish-dry-run` and `publish` either with the default list or with the remaining crates, as long as both recipes get the same list.

The recovery starts the same way in every case. An agent runs `just check-release`, whose output lists which crates of `release_packages` crates.io holds, because a crate whose upload succeeded can still fail while Cargo waits for the index. The remaining crates are the others, in the order of `release_packages`, and the report of `publish` names them as well. The owner then continues in one of three ways.

1. A transient failure needs no change. Such failures include a lost connection, a timeout while Cargo waits for the index, an interrupt, and a refusal for the rate limit on which `publish` stopped. On the same commit, an agent runs `just publish-dry-run`, and the owner runs `just publish` with the same list, after the time that crates.io named when it refused for the rate limit. When `main` has moved since the failed publish, such as for a change to the release scripts that changes no packaged file, the recovery runs on the new commit of `main` as path 2 does.
2. A failure that a change to a remaining crate fixes, such as metadata that crates.io rejects, needs a new commit. An agent fixes it on a feature branch, which merges as Section 14.7 describes, and the fix MUST leave the packaged content of every published crate unchanged, including what a package takes from the root `Cargo.toml`, `Cargo.lock`, and the root licence texts. `check-package` shows it, as TOOL-45 states, and a fix that cannot avoid such a change takes path 3. Steps 3 and 4, step 5, step 7, and step 8 then run on the new commit, at the versions of the release, with steps 5 and 8 given the same list. The tag of step 9 names the new commit.
3. A failure that needs a change to a published crate cannot reuse that version, so the release starts again from step 1 with the next alpha for every crate, as TOOL-37 requires. The versions already published stay, and the owner yanks one of them only under Section 9.8.

Path 2 keeps the versions of the release, because every requirement of a remaining crate on a published one names the published version. For example, when `slang-glslang-sys`, the fourth crate of the first release, fails:

- `slang-bindgen`, `slang-src`, and `slang-compiler-sys` are on crates.io at `0.1.0-alpha.1` and the version of `slang-src`, published from the first commit.
- The remaining crates are `slang-glslang-sys`, `slang-compiler`, `slango`, `slango-codegen`, and `slango-macros`.
- `slang-glslang-sys` requires `slang-src` and `slang-compiler-sys` exactly, and `slang-compiler` requires `slang-compiler-sys` at `=0.1.0-alpha.1`. The dry run of the remaining crates resolves these requirements to the published versions on crates.io, so it shows that the remaining crates build against them.
- The fix changes no file of the three published crates, so the new commit holds their published contents, and the tag on the new commit still describes every crate of the release.

Path 3 applies instead when the cause lies in one of the three published crates. That crate cannot get a second `0.1.0-alpha.1`, and the exact requirements of TOOL-37 tie the crates together, so every crate moves to `0.1.0-alpha.2`, and `slang-src` to `0.1.0-alpha.2.slang.2026.17.0.25.gc7954ebd1` when its recorded commit is unchanged. `slang-bindgen` `0.1.0-alpha.1` and the other published versions stay on crates.io, and stay usable unless Section 9.8 applies.

crates.io can delete a whole crate within 72 hours of its publication [CRATES-DELETE], under further conditions that the sources state differently. This design does not rely on it.

### 9.8. Yanking

The owner yanks a version with `just yank <package> <version>` only when the version is unsound, fails to build for every user, or was published with wrong contents. When one defect affects several crates of a release, the owner yanks each of them. A yank needs the owner's go-ahead and a token that may yank, under TOOL-46.

A yank only stops new lock files from selecting the version. Existing lock files keep it, and nothing is deleted [CARGO-YANK]. A yank does not remove a secret that a package holds [CARGO-PUBLISHING], so such a secret is revoked where it was issued. A yank is followed by the next alpha release under TOOL-37.

### 9.9. Self-Test of the Release Scripts

The recipes of the release run scripts under `scripts/`: `check-package`, `check-release`, `publish-dry-run`, `publish`, `tag-release`, and `yank` each run the script of the same name, and `doc-site` runs `check-doc-site` after its documentation command. `publish`, `tag-release`, and `yank` act in public and run only in a release, so their refusals would otherwise first run when they matter. The design adds a self-test for the seven scripts, the recipe `release-scripts-self-test`, which `check` runs and which runs `scripts/self-test-release`.

The script runs every case against stub programs, in this way:

- It writes stubs of `cargo`, `git`, `gh`, `curl`, `sleep`, and `date` into a temporary directory. Each stub except that of `date` records its arguments in a log, and the stubs of `cargo`, `git`, and `curl` answer from files that each case writes, such as the output of `cargo metadata`, the commits of `HEAD` and `origin/main`, and the HTTP status and body of each address.
- The stub of `curl` can also fail as for a host that it cannot reach. The stub of `sleep` fails unless it gets exactly one argument that is a positive whole number, so a case sees a wait that the real `sleep` would reject.
- The stub of `date` prints a fixed time for `date -u +%s` when a case sets one, and passes every other call to `/usr/bin/date`.
- The stub of `cargo publish` prints "Uploading" with the name. A case can make it fail with a given status, refuse each package or the first calls for the rate limit with the text of an error of status 429 and an optional time to try again, upload a version and then fail with a page of status 429 of the index or with another error, or send `INT` or `TERM` to the script.
- Before it changes `PATH`, it resolves `just` with `command -v` to an absolute path, and it refuses to start, with exit status 2 and the message "just is not installed", when it finds none. It then calls `just` only by that path, and the directory of `just` never goes onto `PATH`.
- It sets `PATH` to the directory of the stubs followed by `/usr/bin` and `/bin`, so the real Cargo of the toolchain is not on `PATH`.
- It refuses to start, with exit status 2, when one of the programs that the scripts and the cases run is not in `/usr/bin` or `/bin`, with a message that names it. The list is `bash`, `env`, `python3`, `awk`, `cat`, `cmp`, `cp`, `cut`, `diff`, `dirname`, `du`, `find`, `grep`, `gzip`, `head`, `ln`, `mkdir`, `mktemp`, `rm`, `sed`, `sort`, `tail`, `tar`, `tee`, `tr`, and `wc`.
- It refuses to start, with exit status 2, unless `command -v` resolves each of the six names to its stub and no other program named `cargo` is on `PATH`, so no case can reach crates.io or GitHub. It also refuses when `/usr/bin/date` does not exist, because the stub of `date` passes its calls on to it.
- It unsets `GITHUB_ACTIONS`, `CARGO_REGISTRY_TOKEN`, and `CARGO_TARGET_DIR` before the cases, so it runs the same in CI.
- Each case checks the exit status, a message, and the calls in the log. The cases of a refusal also check that no call that would act happened, such as `cargo publish`, `git tag`, `git push`, or `gh`.

TOOL-50. A program that the self-test needs and that may lie outside `/usr/bin` and `/bin`, such as `just`, MUST be resolved to its absolute path before the self-test narrows `PATH`, MUST be called by that path, and its directory MUST NOT go onto `PATH`. A CI runner installs such tools in directories of its own, which the narrowed `PATH` lacks, and such a directory can also hold a real `cargo`, so putting it on `PATH` could let a case reach crates.io.

The cases of `check-package` build real `.crate` archives, one for each condition of TOOL-45 and for the answers of `static.crates.io`. The cases of `check-release` cover a complete release, a partial one, a wrong or missing `documentation` field, a site that does not answer, an error of crates.io, and a crate that is not a member. The cases of `check-doc-site` cover a complete site with its index page, a missing page, a page under the name with hyphens, a symbolic link, a site over the limit, and a limit that is not a number. The cases of `tag-release` include a release with a version without a hyphen and one with a pre-release, and check that `gh release create` gets `--prerelease` for the pre-release only.

The cases of `publish`, besides its refusals of Section 9.6, cover the rules of TOOL-51:

- The check before publishing: two new crates published in order with both requests made, a new version told apart from a new crate, a version that crates.io holds skipped, every version held so that Cargo never runs, and the partial release with five versions held and three published without a wait.
- The pacing: six new crates, with one wait of 600 s, between the fifth and the sixth.
- The refusals: one refusal with a named time, which waits 125 s, retries, and then waits 600 s before the next new crate, a named time in the past, which waits 60 s, a named time beyond the longest wait, which stops without a wait, the retries used up, and two packages refused three times each, which both succeed.
- The confirmation: an upload followed by a failure, with a page of status 429 of the index and with another error, counted as published, and a page of status 429 for a version that crates.io lacks, which stops without a retry.
- The other failures: a failure of Cargo, crates.io that cannot be reached, crates.io that answers with status 500, and a package that is not a member.
- The interrupts: `INT` and `TERM` during the second of three packages, which exit with status 130 and 143 and print the report. These cases start the script through `python3 -I` with both signals reset to their default, because a shell that starts with a signal ignored cannot trap it.
- The temporary directory, which the script removes after a success and after a failure.

The last cases copy the `justfile` and the scripts of `publish-dry-run`, `publish`, and `tag-release` into the temporary directory and run the three recipes through `just`. They check that a recipe without arguments passes every crate of `release_packages`, read with `just --evaluate`, and that `tag-release` resolves a relative path of the notes file. For `publish`, the case checks one run of `cargo publish --no-verify -p <name>` for each crate of `release_packages`.

The case of `doc-site` runs that recipe through `just` against metadata that gives each crate of `release_packages` a feature `vendored` and adds a package outside it with one. It checks that the documentation command passes the feature `vendored` of each crate of `release_packages` and no feature of the other package. The stub of `cargo` refuses, as Cargo does, with exit status 101 and "none of the selected packages contains this feature", a feature of the form `<package>/<feature>` whose package the command does not select with `-p`, and a case checks that refusal itself. The stub of `cargo doc` writes the index page of each selected crate, so `check-doc-site` passes after it.

On 2026-11-18, `release-scripts-self-test` printed "release self-test passed: 80 cases".

The script reports each failed case after "FAIL", exits with status 1 when a case fails, and prints the number of cases when all pass. The cases are inline in the script, and the script has no directory under `scripts/fixtures/`, because `scripts/self-test-checker` runs only the custom checkers of Section 6.

The recipe is not a checker's self-test under TOOL-15. The real documentation command of `doc-site` and the recipes `check-package`, `check-release`, and `yank` themselves are not covered.

## 10. Container Image Requirements

The repository slang-rs-env defines the container image, so the changes of this section are made there and not in this repository.

TOOL-28. An agent MUST make each change to the container image in the repository slang-rs-env. The first change in this repository after an image rebuild that adds or removes a tool MUST update the tool list in the Technology Decisions section of `AGENTS.md`. It MUST also update the status of the tool in Section 4.8 of the coding standard.

A document that calls an installed tool missing leaves its rules as judgement rules that a reviewer checks by hand. A document that calls a missing tool installed leads an agent to build on it.

TOOL-29. The container image MUST provide these, because the recipes of Section 3.2 run them, and the run-time path lets the tests start:

- Every tool that Section 4.8 of the coding standard lists.
- A nightly toolchain with the components miri and rust-src, used only for checkers, as DEP-4 of the coding standard requires, with the stable toolchain as the default.
- The component llvm-tools of the stable toolchain, which cargo llvm-cov needs.
- GitHub's Secure Shell (SSH) host keys in the user's known hosts file, so that git can verify GitHub when it pushes.
- `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH` for every process, whether or not a login shell started it, so that a test binary finds the Slang shared library at run time (Section 4.4 of the architecture design).
- libclang, which every build of `slang-compiler-sys` and the recipe `check-slang-call-guard` load to generate the bindings (SCS-64 of `docs/design/slang-compiler-sys.md`).
- git, CMake 3.22 or later, Ninja, and a C++20 compiler, which `test-vendored` and `check-vendored` need to build Slang and slang-glslang on the vendored route, and Python 3 for the build of slang-glslang (Section 9.3 of `docs/design/slang-compiler-sys.md` and Section 8 of `docs/design/slang-glslang-sys.md`).
- The development headers of Xlib, as the Debian package `libx11-dev`, which `check-rhi` and `test-rhi` need to build slang-rhi (Section 7.1 of `docs/design/slang-rhi-sys.md`).
- The Debian package `xwayland`, which the owner requires in the image.
- `gh`, which `tag-release` runs, and curl, which `check-release`, `check-package`, and `publish` run (Section 9.6).
- Python 3, tar, `cmp`, `diff`, `find`, `du`, awk, `tee`, and `date`, which the scripts of the release run. They read JSON with Python 3 and not with jq, which is no requirement.

The image meets these release requirements: it provides `gh` 2.103.3 and curl 8.14.1 under `/usr/bin`, and the other programs of the release scripts.

The image is not required to provide a toolchain for the declared `rust-version`, because `check-msrv` runs in CI and installs that toolchain on first use (Section 14.3). The Technology Decisions section of `AGENTS.md` lists what the image provides.

The image does not yet provide `libx11-dev` or `xwayland`, so slang-rs-env has to add both to its Containerfile. Until an image rebuild adds them, a development container has them only when they were installed with apt in the running container, and a rebuilt container loses them, after which `check-rhi` and `test-rhi` fail with the message of RSYS-17 of `docs/design/slang-rhi-sys.md`.

The image does not yet meet the item about `LD_LIBRARY_PATH`. A login shell gets both the directory of the Vulkan loader and `$VULKAN_SDK/lib`, but a process that no login shell started gets only the directory of the Vulkan loader.

slang-rs-env contains the fix to the `ENV` instruction of its Containerfile, which takes effect at the next image rebuild.

The gap does not affect the recipes, because each recipe that runs compiled code which loads Slang supplies the library path on its own command line (Section 3.2). Until the rebuild, a test binary that links Slang still fails to start when a process that no login shell started runs it outside a recipe (Section 14.2).

## 11. Assumptions to Confirm

Each row is a fact that this document relies on and that has not been checked. A section that relies on one of them cites its identifier. The change that confirms or corrects an assumption states the result in the section that relies on it and removes the row.

An identifier is never reused, so the next assumption takes TA-27.

TA-24, that Cargo accepts `--features slango/rhi` for `slango-macros`, whose dependency on `slango` is a development dependency, is confirmed by a check of a scratch workspace with Cargo 1.99.1, in which `cargo tree -e features` showed the feature enabled. TOOL-53 no longer uses it, because such a build gives one copy of `slango` both backend features.

TA-26, that cargo-semver-checks with `--workspace` skips a member with `publish = false`, is confirmed: on 2026-11-18, `check-semver` checked only `slang-bindgen`, `slang-compiler`, and `slang-compiler-sys` while `slango`, `slango-codegen`, and `slango-macros` set `publish = false`, and TOOL-54 states the result.

TA-25, that the CI job `vendored` on Windows finishes within its timeout of 90 minutes with the scenes of the examples on the backend `rhi`, is confirmed: in CI run 44474920321 of 2026-11-19, it took 16 min 38 s in all (Section 7.4).

TA-9, that the hosted Windows and macOS runners provide Python 3 that the build of slang-glslang finds, is confirmed: the build of slang-glslang ran on both, and the step of Section 7.4 found Python 3 as `python3` on both.

TA-19, the addresses of the crates.io API and of `static.crates.io` that `check-release` and `check-package` read, is confirmed for a published crate of another project and for an unpublished crate (Section 9.6).

TA-13, that `doc-site` completes within the timeout of 60 minutes on a hosted Linux runner, including the clone and the builds of Slang and slang-glslang, is confirmed in Section 9.4.

TA-15, that `actions/deploy-pages` deploys the artifact of `actions/upload-pages-artifact` without `actions/configure-pages` and that its job needs no `contents: read`, is confirmed in Section 7.5.

TA-16, that a GitHub Pages site of a public repository is public, is confirmed in Section 7.5.

The part of TA-17 that the development container could show is confirmed and stated in Section 9.6, and its row keeps the rest. The part of TA-17 on a dry run of a subset is confirmed in Section 9.6, and `publish` no longer depends on the order in which one run of Cargo publishes several crates, so its row keeps only the wait for the index.

The parts of TA-21 that the settings of the repository and the first run of the documentation workflow could show are confirmed in Section 7.5. With the source of GitHub Pages set to "GitHub Actions", GitHub provides the environment `github-pages`, and the owner could add to it a deployment rule for tags that match `v*`, next to the rule for the branch `main`. The row keeps the rest.

TA-10, that cargo-semver-checks accepts the options of TOOL-38 in their meaning, TA-11, that it builds with the environment of the recipe, and TA-12, that it treats `#[doc(hidden)]` items as outside the public API, are confirmed in Section 9.3.

The part of TA-20 that the manifest of the installer action could show is confirmed in Section 7.1, and its row keeps the installation on a runner.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| TA-17 | A run of `cargo publish` for one crate returns only after the crate appears in the index of crates.io, so the next run of `publish` resolves it as a dependency | Section 9.7 | The next run of `publish` that publishes a crate and then a crate that depends on it |
| TA-18 | crates.io lets an API token be limited to publishing new crates and new versions, to named crates, and to an expiry date | TOOL-46 | The owner, when creating the token of the first release |
| TA-20 | `taiki-e/install-action` installs cargo-semver-checks on the hosted Linux runner from its manifest, verified against its checksum, as TOOL-21 requires | Section 7.1 | The first run of the CI job `semver` on GitHub |
| TA-21 | The deployment branch and tag rules of the environment `github-pages` refuse a deployment from a run that a tag started, unless a rule allows tags that match `v*` | Sections 7.5 and 9.6 | A run that a tag starts while the environment has no rule for `v*`, which no step of the release procedure makes, because step 6 adds the rule before the first tag |
| TA-22 | Cargo prints a refusal of the crates.io API for its rate limit as "the remote server responded with an error (status 429 Too Many Requests):" followed by the detail text of Section 9.7, which holds "try again after `<date>`", so that the pattern `responded with an error \(status 429` of `publish` matches it and the script reads the time. A reviewer read that form in the source of Cargo on its main branch, and read the detail text in the sources of crates.io. The source of the installed Cargo 1.99.1 was not read, and no real refusal was observed through Cargo | TOOL-51 | The first real refusal for the rate limit. If Cargo prints it otherwise, `publish` stops with Cargo's status instead of waiting, and the owner runs it again later |
| TA-23 | On macOS, `date -u -j -f '%a, %d %b %Y %T GMT'` reads the named time, and `publish` runs on the bash that `/usr/bin/env bash` finds there, which can be bash 3.2. Neither was run, because the development container has only GNU `date` and bash 5.2. When the time cannot be read, `publish` waits the interval of the kind, which is slower but within the limits | TOOL-51 | The first run of `publish` on macOS |

## 12. Open Questions

Each row is a question that this document leaves open. The design document in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row.

| Identifier | Question | Owning design document | Milestone |
| --- | --- | --- | --- |
| TQ-11 | Do the releases after the first move from an API token to Trusted Publishing from a workflow, which needs a change of TOOL-22, a trusted publisher on crates.io for each crate, and the action `rust-lang/crates-io-auth-action`, of which v1.0.6 is commit `3268eb413133e2960fa4ae0e963ca76380c54cd9` (Section 9.7)? | This document | M11 |
| TQ-12 | Does the hosted site keep the documentation of each released version in a directory of its own, so that the link of an older version shows that version? The deployment replaces the whole site, so it would need to rebuild or keep every earlier version, and no official guidance on such directories was found (Section 9.4) | This document | M11 |
| TQ-13 | Is there a stable way to keep the absolute build path out of the hosted site, such as an option of rustdoc or Cargo, a short path for the generated bindings, or a pass over the site in `doc-site` (Section 9.4)? A stable option of a later toolchain answers it when a `doc-site` run with it leaves no file of the site naming the build directory, and the source pages and "Source" links of the hand-written crates stay | This document | M11 |
| TQ-14 | Should `publish-dry-run` build with `CARGO_TARGET_DIR=target/vendored`, without the rest of `vendored_environment`, so that it always finds a tagged target directory and reuses the clone and, when the compiler, the variables of CMake, and the commit match, the native builds of the vendored recipes (Section 9.6)? The cost: `target/vendored` would also hold builds on the dynamic route and the packages under `target/vendored/package`, so `check-package` would have to read the same directory, and TOOL-7 and Section 9.6 would gain a departure with its reason. The reuse of the native builds is not measured | This document | M11 |
| TQ-17 | While every release is an alpha (TOOL-37), should `check-semver` pass `--release-type minor`, so that cargo-semver-checks runs its checks between two alphas, together with a rule that a reported breaking change, exit status 100, does not fail the job? Without the option, the check reports no change of the API until the first release that is not an alpha. An alpha may break its API, so a report that failed the job would block an intended change (Section 9.3) | This document, by a decision of the owner of the project | Not assigned |
| TQ-18 | Should `check-package` check that each example of a README and of the API documentation that names a version of the crates names the version that step 1 of Section 9.6 and RDM-33 of `docs/writing-readmes.md` require, which no tool checks? | This document | Not assigned |
| TQ-19 | The documentation workflow deploys the site of `doc-site` as a whole, and until M11 that site holds the five crates of `release_packages` only (TOOL-54). A release before M11 would therefore remove the hosted pages of `slango`, `slango-codegen`, and `slango-macros` at `0.1.0-alpha.1`, which their `documentation` fields, their crate READMEs, and the root `README.md` link. Should such a release also document the three crates from their published versions, keep the older site beside the new one, or make no release before M11? | The owner of the project, by a decision | Before the next release |

TQ-4, how a release is carried out, is answered by Sections 9.2 to 9.9. TQ-15, whether the owner keeps the `authors` field of `[workspace.package]`, is answered in Section 9.6. TQ-16, whether cargo-semver-checks fails a breaking change between two alphas, is answered in Section 9.3, and it does not.

An identifier is never reused, so the next question takes TQ-20.

## 13. Security Considerations

The threat model has eight parts:

1. A dependency, a build script, or a procedural macro in the dependency graph is malicious or compromised. It runs with the permissions of the developer or the job whenever a check compiles the workspace, including Clippy, the documentation build, and the tests.
2. An action that a workflow uses is compromised, or its owner moves a tag to different code.
3. An attacker on the network or at a download host tampers with a tool, the Vulkan SDK, or the advisory database that a check or a job downloads.
4. The author of a pull request tries to read a secret or to write to the repository through CI.
5. A pre-commit hook repository changes the code that its hooks run.
6. A file or a file name that a checker reads, such as one in a pull request, holds terminal control sequences. A checker that printed them unchanged would send them to the terminal of the developer, which is terminal escape injection.
7. An attacker obtains the crates.io token and publishes versions of the crates, which can never be deleted, or a release publishes a package with wrong contents.
8. An attacker uses the deployment of the hosted documentation to change the public site.

The design addresses each part:

1. For dependencies, cargo-deny rejects crates with known advisories and crates from sources other than crates.io (TOOL-26), and the allow-list admits only reviewed licences (TOOL-24). `Cargo.lock` fixes every version, and the checks run inside the development container and not on the developer's host.
2. For actions, TOOL-20 pins each one to a commit SHA, which `check-workflow-pins` enforces, and TOOL-19 limits what a compromised step can do. The checkout step does not leave the token in the git configuration of the runner (Section 7.1).
3. For downloads, TOOL-21 and TOOL-23 verify every file before use, and the workflow restores nothing from a cache. The installer action carries its checksums in a manifest that its commit SHA pins.

   The expected checksum of the Vulkan SDK is recorded in the workflow and not fetched from the host that serves the SDK, and the download is verified before it is unpacked or run (Section 7.3). Only the build script of `slang-src` reaches the network, and only to clone the recorded commit of the Slang tree by its full hash, which git verifies (ARCH-35 of the architecture design, SYS-5 of the coding standard, Section 14 of `docs/design/slang-src.md`).
4. For pull requests, TOOL-22 keeps secrets out of every workflow and forbids the event that runs pull request code with secrets, and `check-workflow-pins` reports that event. A run for a pull request from a fork gets a read-only token and no secrets (Section 7.2).
5. For hooks, `.pre-commit-config.yaml` pins the repository `pre-commit-hooks` to a commit SHA, and the local hooks run only recipes of this repository.
6. For checker output, every custom checker escapes its output (Section 6.1), so a checked file cannot send a control sequence to the terminal.
7. For the token, TOOL-46 keeps it out of the repository, CI, Cargo's credentials file, and every agent's environment, limits its scopes and lifetime, and passes it to one command. `publish` runs no build script while the token is in its environment, because it passes `--no-verify` after the dry run verified the same commit, and its requests to the crates.io API carry no token. TOOL-22 keeps publishing out of every workflow, TOOL-47 keeps the release recipes off the allow list, and `check-package` checks the contents of every package before the publish (TOOL-45).
8. For the site, only the deploy job of a workflow that runs on a tag holds `pages: write` and `id-token: write` (TOOL-42). Pushing a tag needs write access to the repository, and the actions of the workflow are pinned by commit SHA (TOOL-20). TOOL-49 keeps a site built on a developer's machine, which names that machine's build path, off the public site.

These attacks are out of scope:

- A compromise of GitHub, crates.io, or LunarG themselves defeats every check here, and no repository configuration can detect it.
- The build of the container image is defined and secured in the repository slang-rs-env.
- A compromise of the developer's machine outside the container is a matter of that machine's security, not of this repository.

The topics of unsafe code, foreign function interface invariants, untrusted shader source, and file paths do not apply, because this document defines no code that calls Slang or compiles shader source.

The residual risk has nine parts:

1. A new dependency's build script and procedural macros run on the first build that includes them, before any reviewer has read them, and cargo-deny finds only vulnerabilities that are already published.
2. A checksum shows only that a file matches what the upstream published, and the first download of a file whose upstream publishes no checksum is trusted on first use. A checksum that comes from the same host as the file, as the checksum of actionlint does, proves integrity but not origin, so an attacker who controls that host can replace the file and its checksum together. Because CI installs the latest version of each tool, a new release of a tool runs in CI without any change to this repository and before anyone has reviewed it.
3. A pinned commit of an action is as trustworthy as the review it received when it was pinned, and the manifest of the installer action is as trustworthy as that commit.
4. `check-deny` fetches the advisory database on every run (Section 14.6), so a tampered database could hide an advisory.
5. `check-workflow-pins` reads workflows line by line, so a reference written in a form that it does not read (Section 6.8) can pass it. The permissions of each job, the use of secrets, and whether each download step verifies its file depend on review.
6. The escaping covers only the custom checkers of Section 6. `scripts/check-commit-message` and the external tools that the recipes run, such as shellcheck, typos, actionlint, and Cargo, print their output without this escaping, and this document does not specify how they handle control characters. Because a backslash prints unchanged, escaped output can be ambiguous to a reader, which misleads but sends no control sequence.
7. The installers of the Vulkan SDK for macOS and Windows fetch metadata from LunarG's remote repository during the installation (Section 7.3), and the recorded checksum does not cover that metadata, so a tampered response is not detected. Two facts limit this risk: the files that the installer installs come from the verified download, and the step selects only the core component. No supported option that installs without this access is known.
8. During `just publish`, the token is in the environment of `just`, the script, and every program that the script starts in the development container, including while the script waits for the rate limits of TOOL-51, which can add up to tens of minutes. A process in the container that can read that environment, or a compromised Cargo or other program that the script starts, can take it. The limits of TA-18 bound what it can do, and the owner revokes the token after the release.
9. A compromised action of the documentation workflow, or a compromised dependency of a crate whose build script runs in `doc-site`, can change the site of a release. It cannot publish a crate or write to the repository, because the deploy job holds no other permission.

## 14. Operational Considerations

### 14.1. Recipes and Parameters

Section 3.2 lists every recipe. This document defines no Cargo feature.

This table lists every recipe parameter and its default. Every other recipe takes no parameters.

| Recipe | Parameter | Default |
| --- | --- | --- |
| `check-prose` | Paths of files to check | Every Markdown file and every Rust file in the repository |
| `check-rust-comments`, `check-doc-headings` | Paths of files to check | Every Rust file in the repository (TOOL-14) |
| `check-doctest-attributes` | Paths of files to check | Every Rust file and every `README.md` file in the repository (TOOL-14) |
| `check-feature-docs` | Paths of Cargo manifests to check | The workspace of the repository (Section 6.5) |
| `check-slang-call-guard` | Paths of Rust files to check | Every Rust file under `crates/slang-compiler/src` (Section 6.9) |
| `check-workflow-pins` | Paths of workflow and composite action files to check | Every workflow under `.github/workflows/` and every composite action under `.github/actions/` (Section 6.8) |
| `check-doc-references` | Paths of files to check | Every `.rs`, `.toml`, `.yml`, `.yaml`, and `.json` file, the `justfile`, and every file under `scripts/` except the checker's own self-test cases (Section 6.10) |
| `check-commit-message` | The commit message file | None. The parameter is required. |
| `merge` | The branch, and further arguments for `scripts/merge-branch` | None for the branch, which is required |
| `publish-dry-run`, `publish` | The names of the crates to verify or publish, which `publish` compares with the list that the dry run recorded | Every crate of `release_packages`. A list serves only to finish a release that failed part way, which the default list also does, because `publish` skips every version that crates.io holds (Section 9.7) |
| `tag-release` | `NOTES`, the path of the file with the release notes. A relative path is relative to the directory where `just` runs, through the attribute `[no-cd]`, and the recipe then runs the script from the root of the repository | None. The parameter is required |
| `yank` | The crate and the version to yank | None. Both are required |
| `install-vulkan-sdk` | The SDK version, as four numbers separated by full stops, and the expected SHA-256 checksum of the download for the platform, as 64 lowercase hexadecimal characters (Section 7.3) | None. Both are required. `just` rejects a missing or extra argument before the script runs, and the script exits with status 2 on a malformed one |

### 14.2. Environment Variables

This table lists every environment variable that the recipes, the hooks, or the checks read.

| Variable | Read by | Default and behaviour when unset |
| --- | --- | --- |
| `CARGO_BUILD_WARNINGS` | Cargo, in the commands of Section 4.8 of the coding standard | The recipes set it to `deny` for those commands. Outside them it is unset, and Cargo reports warnings without failing. |
| `VULKAN_SDK` | The build script of `slang-compiler-sys` on the dynamic route, `check-vulkan-sdk`, `check-slang-call-guard`, and the `justfile`, which builds the library path assignment from it | Section 19.2 of the architecture design. The development container sets it, and on a CI runner other than a `vendored` one, `install-vulkan-sdk` sets it for the later steps of the job (Section 7.3). Without it, the build script stops every command that compiles the workspace, so every recipe that compiles fails except `test-vendored`, `check-vendored`, and `doc-site`, which remove it (Section 18.2 of `docs/design/slang-compiler-sys.md`). `check-slang-call-guard` fails when it is unset, because the binary of `slang-bindgen` reads the headers under it. `check-vulkan-sdk` exits with status 2 when it is unset or empty, and with status 1 when it names a path that is not a directory. When it is unset or empty, the library path assignment is empty. |
| `RUNNER_TEMP`, `GITHUB_ENV`, `GITHUB_PATH` | `install-vulkan-sdk` | Set by GitHub Actions on every runner. When one of them is unset or empty, the recipe exits with status 2 and a message that names it and says that this is not a GitHub Actions runner. |
| `CARGO_NET_OFFLINE` | The build script of `slang-src`, which only `test-vendored` and `check-vendored` build | Section 15.2 of `docs/design/slang-src.md`. The development container does not set it. When it is `true`, `test-vendored` and `check-vendored` work only while the shared folder of `target/vendored` holds the clone. `slang-src` reads no other variable of its own. |
| `CARGO_TARGET_DIR` | Cargo | Unset, so Cargo builds in `target/`. `test-vendored`, `check-vendored`, and `doc-site` set it to `target/vendored` on their own command lines, so the vendored build does not replace the dynamic build of `target/`, and the shared folder of the vendored build is `target/vendored/slang-src`. When one of them is the first build of a checkout, Cargo creates `target/` without its tag (Section 9.6). |
| `LIBCLANG_PATH` | `clang-sys`, whenever `slang-bindgen` runs, which is every build of `slang-compiler-sys`, `check-slang-call-guard`, and the tests of `slang-bindgen` | Section 18.2 of `docs/design/slang-compiler-sys.md` |
| `LD_LIBRARY_PATH` | The dynamic loader on Linux, when a test binary starts, and the `justfile`, which reads its earlier value | The recipes that run compiled code which loads Slang set it on their own command line to `$VULKAN_SDK/lib` followed by any earlier value, so they do not depend on the path of a login shell (Section 3.2). In a login shell, the container includes `$VULKAN_SDK/lib` in it. A process that no login shell started lacks that directory until the next image rebuild (Section 10), which affects only a test binary run outside a recipe. Without it, a test binary that links Slang fails to start. On a Linux CI runner, `install-vulkan-sdk` sets it to `$VULKAN_SDK/lib` followed by any earlier value (Section 7.3), so the directory appears twice in a recipe there, which is harmless. `test-vendored` and `check-vendored` remove it, and `DYLD_LIBRARY_PATH`, from the environment of their commands. |
| `DYLD_LIBRARY_PATH` | The dynamic loader on macOS, when a test binary starts, and the `justfile`, which reads its earlier value | The recipes that run compiled code which loads Slang set it on their own command line to `$VULKAN_SDK/lib` followed by any earlier value (Section 3.2). Outside a recipe, it is unset unless the developer sets it. On a macOS CI runner, `install-vulkan-sdk` sets it to `$VULKAN_SDK/lib` followed by any earlier value (Section 7.3), so the directory appears twice in a recipe there. Whether the assignment on the command line reaches a test binary on a Mac with System Integrity Protection enabled is SA-7 of `docs/design/slang-compiler-sys.md`. |
| `PATH` | The dynamic loader on Windows, when a test binary starts, and every shell | On a Windows CI runner, `install-vulkan-sdk` adds `$VULKAN_SDK\Bin`, which holds `slang-compiler.dll`, and on Linux and macOS runners it adds `$VULKAN_SDK/bin` (Section 7.3). |
| `CLAUDE_PROJECT_DIR` | The hook in `.claude/settings.json` | Set by Claude Code. The hook does not run outside it. |
| `CARGO_REGISTRY_TOKEN` | Cargo, in `publish` and `yank` | Unset everywhere except on the command line of the owner's one run of `publish` or `yank` (TOOL-46). When it is unset or empty, both recipes refuse to run. It stays set while `publish` waits for the rate limits of crates.io (Section 9.7). |
| `GITHUB_ACTIONS` | `publish` and `yank` | Set to `true` by GitHub Actions on every runner. When it is set, even to an empty value, both recipes refuse to run, because no workflow publishes (TOOL-22). `release-scripts-self-test` unsets it, so it runs in CI. |

The workflow also sets these variables for its own steps, and no recipe reads them. Its top-level `env` holds `VULKAN_SDK_VERSION`, `VULKAN_SDK_SHA256_LINUX`, `VULKAN_SDK_SHA256_MACOS`, and `VULKAN_SDK_SHA256_WINDOWS`. The step that installs the SDK passes the version and the checksum for its platform to `install-vulkan-sdk` as arguments (Section 7.3).

### 14.3. Platforms and Minimum Supported Rust Version

The checks run in the development container on Linux x86_64, and CI runs them on the platforms of Section 7.1, with tool versions that can differ from those of the container (Section 7.1). The Vulkan SDK in CI is the version that the workflow records, on every platform (Section 7.3).

The workspace declares `rust-version = "1.99"` (TOOL-4), which only a change of TOOL-5 raises, and `check-msrv` verifies it in the CI job `ci-only`.

cargo hack always uses the rustup toolchain named exactly like `rust-version`, which is `1.99`. It installs that toolchain with rustup when it is missing, even when the installed stable toolchain is a later patch release such as 1.99.1. The first run of `check-msrv` on a machine therefore downloads a toolchain and needs network access, and later runs use the installed toolchain.

The root `.gitattributes` holds the single line `* text=auto eol=lf`. git normalizes every file that it detects as text to LF line endings in the repository and checks it out with LF on every platform, whatever the `core.autocrlf` setting of the clone. Without it, the checkout on the hosted Windows runner has CRLF line endings, which breaks a recipe that reads a file line by line, such as `recorded-slang`.

The self-tests of the custom checkers do not depend on the line endings of the checkout, because their fixtures create carriage returns from placeholders at run time (Section 6.1). The recipe `recorded-slang` and `scripts/check-vulkan-sdk` still accept CRLF input, because a user's own git settings can still produce it.

### 14.4. Failing Checks

This table says what a failure of each check means and what the developer does.

| Recipe | A failure means | What the developer does |
| --- | --- | --- |
| `check-fmt` | A Rust file is not formatted, including a file that a crate includes with `include!`, or `git ls-files` failed or listed no Rust file, as its message says | Runs `just fmt`. For a failed listing, runs the recipe inside the git checkout of the repository |
| `fmt` | `git ls-files` failed or listed no Rust file, as its message says, or rustfmt cannot parse a file | Runs the recipe inside the git checkout of the repository, or fixes the syntax error that rustfmt reports |
| `test-vendored` | The build script of `slang-src` stopped the build with a message of Section 11 of `docs/design/slang-src.md`, such as for a build without network access, a native build stopped with a message of Section 9.6 of `docs/design/slang-compiler-sys.md` or Section 12.4 of `docs/design/slang-glslang-sys.md`, a test or doctest failed, or the command for `slango-macros` stopped with "VULKAN_SDK is not set", which means that a crate between `slango-macros` and `slang-compiler` does not forward the feature `vendored` | Follows the recovery of Section 15.4 of `docs/design/slang-src.md`, Section 18.4 of `docs/design/slang-compiler-sys.md`, or Section 12.4 of `docs/design/slang-glslang-sys.md`, such as running it once with network access, or fixes the code, the test, or the forward in the manifest |
| `check-vendored` | A build failure as for `test-vendored`, a lint or rustdoc lint fired on the vendored code, or the Clippy command for `slang-slint` stopped with "VULKAN_SDK is not set", which means that a crate between `slang-slint` and `slang-compiler` does not forward the feature `vendored` | For a build failure, as for `test-vendored`. For a lint, follows Section 4.4 of the coding standard and the lint's row in Section 4.5. For a broken forward, fixes the manifest |
| `check-rhi`, `test-rhi` | The recipe ran on a host other than Linux or x86_64 Windows and printed that the packages build only on Linux and on x86_64 Windows, on Windows a `gpu` test failed at the creation of a compute pipeline because `dxcompiler.dll` is not on `PATH`, a build failure as for `test-vendored`, the build script of `slang-rhi-sys` stopped the build with a message of RSYS-15 of `docs/design/slang-rhi-sys.md`, a lint or rustdoc lint fired, or a test failed. Without the development headers of Xlib, the error names the failed configure step of slang-rhi, and the last lines of its log hold "The C++ compiler cannot find the header X11/Xlib.h" | Runs the recipe on Linux or x86_64 Windows. On Windows, puts the directory of `dxcompiler.dll` of the Windows SDK on `PATH`. For a build failure of Slang, as for `test-vendored`. For the Xlib headers, installs them, such as with the Debian package `libx11-dev`, and runs the recipe again, which configures again. Otherwise follows Section 12.4 of `docs/design/slang-rhi-sys.md`, or fixes the code, the lint, or the test as for `check-vendored` |
| Every recipe that compiles the workspace | The build script of `slang-compiler-sys` stopped the build, with a message that Section 9.6 of `docs/design/slang-compiler-sys.md` lists, such as "VULKAN_SDK is not set", a missing libclang, or headers that the rule set of `slang-bindgen` does not cover | Follows the recovery for that failure in Section 18.4 of `docs/design/slang-compiler-sys.md`, such as installing the Vulkan SDK of the recorded Slang and setting `VULKAN_SDK`, or installing libclang |
| `check-clippy`, `check-doc`, `check-doctest` | A lint fired | Follows Section 4.4 of the coding standard and the lint's row in Section 4.5 |
| `test`, `test-gpu`, `test-hw-gpu` | A test failed, a test in a GPU tier found no adapter (ARCH-23 of the architecture design), or a test binary could not start because the loader did not find the Slang shared library, as when `VULKAN_SDK` is unset in the environment of `just` | Fixes the code or the test, runs the tier on a machine with an adapter, or sets `VULKAN_SDK` to the SDK of the build so that the recipe supplies the library path |
| `check-deny` | A licence is not allowed, an advisory applies, a crate is duplicated, or a source is not crates.io. The exit status is the sum of 1 for advisories, 2 for bans, 4 for licences, and 8 for sources | Applies Section 8: replaces or upgrades the dependency, or records an exception that TOOL-24 or TOOL-26 permits |
| `check-prose`, `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs` | Exit status 1: a rule of the writing guide is broken at each reported `path:line`, and the message names the problem, such as "comment other than API documentation or a safety comment" or "feature 'name' is not named in a code span in the '# Features' section". Exit status 2: a file cannot be read, including a file that ends inside an unterminated literal, comment, or attribute, `cargo metadata` failed, the checker ran with no paths outside a git repository, or the setup is broken, as the message on standard error says (Section 6.1) | For status 1, fixes the text as the rule that Section 6 names requires. For status 2, fixes the file or the setup that the message names, such as by closing the unterminated item at the reported line |
| `check-workflow-pins` | Exit status 1: a rule of Section 7.2 is broken at each reported `path:line`, such as "uses: '...' is not pinned to a full 40-character commit SHA", "workflow uses the pull_request_target event", "workflow has no top-level permissions key", or "top-level permissions '...' grant more than read access". Exit status 2: as for the other custom checkers | Pins the action to the full commit SHA of the reviewed release, removes the event, or sets the top-level permissions to `contents: read` and moves any extra permission to the job that needs it (TOOL-19) |
| `check-slang-call-guard` | Exit status 1: a call into Slang in `slang-compiler` breaks SLC-32 of `docs/design/slang-compiler.md` at each reported `path:line`, such as "call of IModule::getName is not inside a closure passed to abort_on_exception". Exit status 2: as for the other custom checkers, and also a file with a bracket without its partner, or bindings that cannot be read or declare no C function or no thunk (Section 6.9) | For status 1, makes the call the only call into Slang in a closure literal passed to `abort_on_exception`, as SLC-32 requires. For status 2, fixes the file that the message names, or, for the bindings, what stops the run of `slang-bindgen` before the checker, such as an unset `VULKAN_SDK` or a missing libclang, as the error of the binary says (Section 7.7 of `docs/design/slang-compiler-sys.md`) |
| `check-doc-references` | Exit status 1: code or configuration cites a project document at each reported `path:line`, with "cites the project document ...", "cites a section of a project document: '...'", or "cites the rule identifier ... of a project document". Exit status 2: as for the other custom checkers, and also a project document that cannot be read, no project document, or no document that defines a rule identifier (Section 6.10) | For status 1, removes the citation and states what the code needs on its own terms, as Section 7.1 of the writing guide requires. For status 2, fixes the file or the document that the message names, or restores the Markdown files under `docs/` and their summary tables |
| `check-vendored-shared-folder` | A message after "check-vendored-shared-folder:" says which check failed: `recorded-slang` printed no `RECORDED_CLONE_ENTRY`, `target/vendored` holds no `CACHEDIR.TAG`, the shared folder does not hold exactly one clone entry, the complete entry `src/<RECORDED_CLONE_ENTRY>`, with the number of entries that it holds, or a directory outside the shared folder holds a clone. Otherwise `find` failed and printed its own error, such as for a directory that it cannot read | For a missing constant, fixes `recorded.rs` as for `recorded-slang`. For a missing tag, removes `target/vendored`, which existed before Cargo first built there, and runs the vendored recipe again, which clones again. For a missing entry, runs `test-vendored` or `check-vendored` first. For an extra entry, such as a clone `src/<commit>` or `src/<commit>-<submodules>` with the full commit hash that an earlier layout of `slang-src` left, removes every entry that the recipe lists other than `src/<RECORDED_CLONE_ENTRY>` when no build runs, which Section 9.3 of `docs/design/slang-src.md` permits. For another clone, finds which build used a base directory other than `target/vendored`, against SRC-23 of `docs/design/slang-src.md`, such as a run of Miri in `target/vendored`, which builds in `target/vendored/miri` (SRCQ-3 of that design). For a failure of `find`, fixes the permissions of the directory that it names |
| A self-test recipe | A case under `scripts/fixtures/<checker>/` gave a different standard output, standard error, or exit status from the expected one, which the recipe prints after "FAIL" with the name of the case. Exit status 2 means that the checker or its cases cannot be read | Fixes the checker, or, when the change to its behaviour is intended, the expected files of the case |
| `check-shell` | shellcheck reported a finding | Fixes the script or recipe |
| `check-typos` | typos found a spelling mistake | Fixes the word, or accepts a correct word in `typos.toml` (TOOL-30) |
| `check-toml` | A TOML file is not formatted | Runs `just fmt` |
| `check-workflows` | actionlint reported a finding in a workflow | Fixes the workflow |
| `check-unused-deps` | A dependency is declared and not used | Removes it |
| `check-miri` | Miri found undefined behaviour | Fixes the unsafe code, following Section 9 of the coding standard |
| `check-miri-rhi` | The recipe ran on a host other than Linux, a build failure as for `check-rhi`, or Miri found undefined behaviour in `slang-rhi` | Runs the recipe on Linux. For a build failure, as for `check-rhi`. Otherwise fixes the unsafe code, following Section 9 of the coding standard |
| `coverage` | A test failed, or cargo llvm-cov did not write the report. In the CI job `coverage`, the upload step also fails when `target/lcov.info` does not exist | Reads the message of cargo llvm-cov, and fixes a failing test as for `test-gpu` |
| `recorded-slang` | Exit status 1: `crates/slang-src/recorded.rs` does not define `RECORDED_COMMIT`, `RECORDED_BUILD_TAG`, or `RECORDED_CLONE_ENTRY` as a string constant on one line, and the message says "crates/slang-src/recorded.rs does not define NAME as a string constant" with the name of the constant | Restores the constant in that form (SRC-11 of `docs/design/slang-src.md`) |
| `install-vulkan-sdk` | Exit status 1: the checksum of the download differs from the expected one, and the message "install-vulkan-sdk: SHA-256 mismatch" gives both values, or the macOS archive does not hold exactly one installer application. Exit status 2: a malformed argument, an unset or empty `RUNNER_TEMP`, `GITHUB_ENV`, or `GITHUB_PATH`, or an unsupported platform. A missing or extra argument fails in `just` before the script runs. A failed download, unpacking, or installation stops the script with the non-zero status of the command that failed | For a mismatch, compares the checksum in the workflow with the one that LunarG publishes for the file (Section 7.3) before changing anything, because a mismatch can mean a tampered download. For a failed download, runs the job again |
| `check-vulkan-sdk` | Exit status 1: `VULKAN_SDK` names a path that is not a directory, with the message "check-vulkan-sdk: VULKAN_SDK names <path>, which is not a directory", or the SDK at `VULKAN_SDK` lacks `slang.h`, `slang-tag-version.h`, or the Slang library of the platform, defines another build tag, or has a `slangc` that fails or prints another tag, as each message after "check-vulkan-sdk:" says. Exit status 2: an argument was given, `VULKAN_SDK` is unset or empty, or `just recorded-slang` failed or printed no `RECORDED_BUILD_TAG` | Installs the SDK version that the workflow records and points `VULKAN_SDK` at it. In CI, a differing tag means that the workflow and `slang-src` record different Slangs, which the change that moves the recorded Slang fixes (ARCH-30 of the architecture design). Fixes `recorded.rs` when `just recorded-slang` fails |
| `check-features`, `check-msrv` | A feature combination does not compile, or the code needs a newer toolchain than `rust-version` | Fixes the code, or raises `rust-version` under TOOL-5 |
| `check-semver` | Exit status 100: a crate has a breaking change against its last release, which the tool names. It does not occur while the releases are alphas, because the tool then runs none of its checks (Section 9.3). Exit status 101: the check could not complete, such as for a failed build of a crate or its baseline, a missing `VULKAN_SDK` or libclang, a crate that crates.io does not hold, or no network access | For status 100, changes the code so that it stays compatible. For status 101, fixes what the message names, as for a recipe that compiles the workspace |
| `check-package` | The members without `publish = false` differ from `release_packages`, a `documentation` field differs from TOOL-40, `cargo package` failed, such as for uncommitted changes, or a `.crate` file breaks TOOL-45, as the message says | Fixes `release_packages`, the manifest, or the package directory. For a symbolic link or a licence file that differs, packages from a checkout on Linux or macOS with symbolic links (TOOL-33) |
| `publish-dry-run` | A package does not compile from its own contents, a version requirement does not resolve, or Cargo refused a packaged file with an uncommitted change, with "contain changes that were not yet committed into git". It writes no record then | Fixes the manifest or the files that the package lacks, and runs `check-package` again. For an uncommitted change, commits it and runs the recipe again (Section 9.6) |
| `tag-release` | Exit status 1: the notes file is not a readable, non-empty file, the fetch failed, `HEAD` differs from `origin/main`, or the crates other than `slang-src` do not share one version, as the message after "tag-release:" says. Otherwise git or `gh` failed with its own message | Fixes the notes file or the versions, or runs it in the main checkout at `origin/main`. When the push or `gh` failed after the tag was created, the owner decides how to continue, because the tag is public once pushed |
| `yank` | It refused, as for `publish`, or crates.io rejected the yank | Runs it as the owner with a token that may yank (TOOL-46) |
| `release-scripts-self-test` | Exit status 1: a case gave another exit status, message, or call of the stubs, which the recipe prints after "FAIL" with the name of the case. Exit status 2: `just` is not installed, a program that the cases need is not in `/usr/bin` or `/bin`, a stub does not resolve, another `cargo` is on `PATH`, or `just` cannot evaluate `release_packages` | Fixes the script, or, when the change to its behaviour is intended, the case in `scripts/self-test-release`. For status 2, installs the program that the message names, or fixes the setup that it names |
| `publish` | Exit status 1: a check of Section 9.6 refused the run, a package is not a member, or crates.io could not answer the check before publishing, and nothing was published. The status of Cargo: Cargo failed and crates.io does not hold the version, crates.io still refused for its rate limit after 3 retries, or it named a time beyond the longest wait, as the message and the report of the published and remaining packages say. Exit status 130 or 143: an interrupt or a `TERM` signal stopped the release. Exit status 2: a usage error or a missing tool | For a refusal of Section 9.6, runs `publish-dry-run` on the current `main` or sets the token. When crates.io could not answer, runs it again later. For the rate limit, runs it again with the same list later, and not before a time that the message names. For an interrupt or another failure of Cargo, follows Section 9.7 |
| `check-release` | crates.io lacks a version, its `documentation` field differs, or the hosted address does not answer | Waits for the documentation workflow of the tag and runs it again. Otherwise reruns that workflow with the owner's go-ahead, or fixes the field in the next release |
| `doc-site` | A build failure as for `check-vendored`, or a rustdoc lint. Otherwise `scripts/check-doc-site` failed with a message after "check-doc-site:": "cargo doc wrote no page" with the path of each missing index page of a crate, "holds symbolic links" with the links, or "takes N KiB, more than the limit of 976562 KiB". It exits with status 1 for these and for a missing site directory, and with status 2 on a usage error or a limit that is not a whole number of KiB | For a build failure or a lint, as for `check-vendored`. For a missing page, checks the name of the crate in `release_packages` and its library target. For a link or the size, finds the file that causes it in `target/vendored/doc`, which can be a stale file of an earlier build (Section 9.4) |

Clippy can report a `clippy::cargo_common_metadata` warning again after the manifest has been fixed, because it replays its cached result until a source file of the crate changes. The developer then updates the modification time of a source file of the crate, or cleans the build output of the crate, and runs the check again.

No agent weakens a configuration to make a check pass, as Hard Rule 6 of `AGENTS.md` states.

This document states no expected duration for a check. The exception is `test-vendored` and `check-vendored`, whose first run in `target/vendored` clones Slang and builds Slang and then slang-glslang from source, which takes several minutes, and `check-rhi` and `test-rhi`, whose first run there builds Slang and then slang-rhi in the same way. `check-miri-rhi` does the same in `target/miri-rhi`. The build of slang-glslang starts only after the build of Slang, because `slang-glslang-sys` depends on `slang-compiler-sys` (SGL-1 of `docs/design/slang-glslang-sys.md`) and Cargo runs a build script only after the build scripts of its dependencies. A later run in the same target directory takes seconds, because the shared folder holds the clone and the built libraries (Section 9 of `docs/design/slang-src.md`).

A check that passes in the development container and fails in CI, or the reverse, can come from a difference in tool versions (Section 7.1). The developer compares the version in the log of the job with the version in the container before changing the code.

A job that GitHub cancelled before any step ran, with the annotation "The job was not acquired by Runner of type hosted even after multiple attempts", did not fail because of the change. No hosted runner picked the job up, which is a problem of runner capacity. The developer reruns that job and judges the change by the result of the rerun.

### 14.5. Logs

The recipes print the output of the tools they run and keep no log. GitHub Actions keeps the log of each job.

The recipe `coverage` prints the output of the build and the tests and no coverage summary. With the LCOV options, cargo llvm-cov ends its output with the line `Finished report saved to target/lcov.info`. The report is the LCOV file `target/lcov.info`, which the CI job `coverage` uploads as the artifact `coverage-lcov` and GitHub keeps for 14 days (Section 7.1).

`install-vulkan-sdk` prints the download address, the size and duration of the download, the verified checksum, the duration of the installation, the value of `VULKAN_SDK`, and the installed size. `check-vulkan-sdk` prints the files that it finds in the SDK, the content of `slang-tag-version.h`, and the output of `slangc -version`.

On the vendored route, the build script of `slang-compiler-sys` writes the output of `cmake` to `slang/build.log` under its `OUT_DIR`, and the build script of `slang-glslang-sys` writes its output to `build.log` under its own `OUT_DIR`, both inside `target/vendored` for `test-vendored` and `check-vendored` (Section 18.5 of `docs/design/slang-compiler-sys.md` and Section 12.5 of `docs/design/slang-glslang-sys.md`). The build script of `slang-rhi-sys` writes its output to `build.log` under its own `OUT_DIR`, inside `target/vendored` for `check-rhi` and `test-rhi` (Section 12.5 of `docs/design/slang-rhi-sys.md`).

### 14.6. Network Access

`just check` needs network access. `check-deny` fetches the advisory database from <https://github.com/RustSec/advisory-db> on every run, and fails with exit status 1 when it cannot.

With a database already cached, `cargo deny --offline check`, with the global option `--offline` before `check`, works without network access. `check-deny` runs the command of Section 4.8 of the coding standard without that option, so it always checks against the current database.

The first run of `check-miri` after an image rebuild also needs network access, because Miri builds a sysroot and fetches the dependencies of the standard library. Later runs of `check-miri` work offline.

Outside `check`, the first run of `check-msrv` on a machine downloads a toolchain (Section 14.3). `test-vendored`, `check-vendored`, `check-rhi`, and `test-rhi` need network access on their first run in `target/vendored`, because `slang-src` clones Slang from GitHub, and none afterwards while the shared folder `target/vendored/slang-src` exists (SRC-9 of `docs/design/slang-src.md`). `check-vendored-shared-folder` reaches no network. `cargo clean` of that target directory removes the folder, and the next run clones again. The vendored CI jobs clone on every run (Section 7.4).

`install-vulkan-sdk` downloads the SDK from `sdk.lunarg.com`. On macOS and Windows the installer also contacts LunarG's remote repository (Section 7.3).

The recipes of the release reach the network as well. `check-semver` fetches each baseline from crates.io. `publish-dry-run` reads the index of crates.io and clones Slang for the verification of `slang-src` and `slang-glslang-sys` in `target/`, once or twice depending on the tag of `target/` (Section 9.6). `check-package` downloads from `static.crates.io` the published `.crate` file of each crate, if one exists. `doc-site` clones Slang on its first run in `target/vendored`, as the vendored recipes do. `publish`, `yank`, and `check-release` reach crates.io, `check-release` also reaches the hosted site, and `tag-release` reaches GitHub.

### 14.7. Verifying and Merging a Branch

This section gives the procedure for verifying a branch in CI and merging it. The project opens no pull requests, so a push of the branch is what runs CI on it (Section 7.1). The agent follows these steps:

1. It pushes the branch with a plain push, which starts one run for the last commit of the branch.
2. It reads the run for that exact commit from the run itself, with these commands, in which `<branch>` and `<id>` are placeholders. The first prints the identifier, status, conclusion, and commit of the latest run of the branch, and the second prints each job of the run with its conclusion:

   ```sh
   gh run list --branch <branch> --limit 1 --json databaseId,status,conclusion,headSha
   gh run view <id> --json jobs
   ```

3. It runs `just merge <branch>` only when the run's `headSha` is the last commit of the branch, its conclusion is `success`, and every job succeeded.

The agent reads the run itself and never the exit status of a command whose output is piped into another program. The exit status of a pipeline is the status of its last program, so it says nothing about the run, and a merge that relies on it can put a failing commit on `main`. A job cancelled for lack of a runner is rerun, as Section 14.4 states, and the merge waits for the rerun.

`just merge` builds the message of the squashed commit from every commit of the branch (Section 3.2). A branch with several work-in-progress commits is therefore merged through a fresh branch.

The agent creates a new branch from `main` that holds the identical tree of the verified branch as one commit with the final message, and runs `just merge` on the fresh branch. The fresh branch holds the tree that CI verified only when the verified branch contains the current `main`. No pushed history is rewritten.

`just merge` deletes the merged branch on `origin` when its tip there is part of the squash. With the fresh-branch procedure, the merged branch is the fresh one, so the branch that was pushed for CI stays on `origin` after the merge. Its local branch and worktree count as squash-merged once `main` holds its tree, so `just worktree-prune` removes them. By the owner's instruction of 2026-11-19, the agent then checks that `main` holds the tree of the branch that was pushed for CI and deletes that branch on `origin` with `git push origin --delete <branch>`.

## 15. References

### 15.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 15.2. Informative References

- [FSF-LICENSES] Free Software Foundation, "Various Licenses and Comments about Them", version dated 2026-04-22, <https://www.gnu.org/licenses/license-list.html>, read on 2026-11-11.
- [GPL-FAQ] Free Software Foundation, "Frequently Asked Questions about the GNU Licenses", the compatibility matrix, <https://www.gnu.org/licenses/gpl-faq.html#AllCompatibility>, read on 2026-11-11.
- [API-GUIDELINES] "Rust API Guidelines", the checklist, <https://rust-lang.github.io/api-guidelines/checklist.html>, read on 2026-11-15.
- [CARGO-MANIFEST] The Cargo Book, "The Manifest Format", the field `documentation`, <https://doc.rust-lang.org/cargo/reference/manifest.html>, read on 2026-11-15.
- [CARGO-PACKAGE] The Cargo Book, "cargo-package", <https://doc.rust-lang.org/cargo/commands/cargo-package.html>, read on 2026-11-15.
- [CARGO-PUBLISH] The Cargo Book, "cargo-publish", <https://doc.rust-lang.org/cargo/commands/cargo-publish.html>, read on 2026-11-15.
- [CARGO-PUBLISHING] The Cargo Book, "Publishing on crates.io", <https://doc.rust-lang.org/cargo/reference/publishing.html>, read on 2026-11-15.
- [CARGO-SEMVER] The Cargo Book, "SemVer Compatibility", the change categories, <https://doc.rust-lang.org/cargo/reference/semver.html>, read on 2026-11-15.
- [CARGO-YANK] The Cargo Book, "cargo-yank", <https://doc.rust-lang.org/cargo/commands/cargo-yank.html>, read on 2026-11-15.
- [CONFIGURE-PAGES] GitHub, the action `actions/configure-pages`, its file `action.yml` on `main`, <https://raw.githubusercontent.com/actions/configure-pages/main/action.yml>, read on 2026-11-15.
- [CRATES-AUTH-ACTION] The Rust Project, the action `rust-lang/crates-io-auth-action`, <https://github.com/rust-lang/crates-io-auth-action>, read on 2026-11-15.
- [CRATES-DATA-ACCESS] crates.io, "Data Access Policy", the source of the page in the repository rust-lang/crates.io, `svelte/src/routes/data-access/+page.svelte`, <https://github.com/rust-lang/crates.io>, read on 2026-11-16.
- [CRATES-DELETE] The Rust Blog, "crates.io: development update", 2025-02-05, <https://blog.rust-lang.org/2025/02/05/crates-io-development-update/>, and RFC 3660, "crates.io crate deletions", <https://rust-lang.github.io/rfcs/3660-crates-io-crate-deletions.html>, both read on 2026-11-15.
- [CRATES-RATE-LIMITS] crates.io, the page on publishing rate limits, <https://crates.io/docs/rate-limits>, which serves its text only through JavaScript, and its source in the repository rust-lang/crates.io at commit `52370836f0b91f5010de43caeaa333212bda0050`, `svelte/src/routes/docs/rate-limits/+page.svelte`, with the defaults in `src/rate_limiter.rs` and the error in `src/util/errors/json.rs`, <https://github.com/rust-lang/crates.io>, read on 2026-11-17.
- [CRATES-TP-2025] The Rust Blog, "crates.io: development update", 2025-07-11, <https://blog.rust-lang.org/2025/07/11/crates-io-development-update-2025-07>, read on 2026-11-15.
- [CSC-README] cargo-semver-checks, its README, <https://github.com/obi1kenobi/cargo-semver-checks>, read on 2026-11-15.
- [CSC-SRC] cargo-semver-checks, its sources `src/lib.rs` and `src/rustdoc_gen.rs` on `main`, <https://raw.githubusercontent.com/obi1kenobi/cargo-semver-checks/main/src/lib.rs> and <https://raw.githubusercontent.com/obi1kenobi/cargo-semver-checks/main/src/rustdoc_gen.rs>, read on 2026-11-15.
- [DOCSRS-BUILDS] docs.rs, "Builds", <https://docs.rs/about/builds>, read on 2026-11-15.
- [GH-PAGES-LIMITS] GitHub Docs, "GitHub Pages limits", <https://docs.github.com/en/pages/getting-started-with-github-pages/github-pages-limits>, read on 2026-11-15.
- [GH-PAGES-WORKFLOW] GitHub Docs, "Using custom workflows with GitHub Pages", <https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages>, read on 2026-11-15.
- [RFC3691] RFC 3691, "Trusted Publishing for crates.io", <https://rust-lang.github.io/rfcs/3691-trusted-publishing-cratesio.html>, read on 2026-11-15.
- [RUST-1.90] The Rust Blog, "Announcing Rust 1.90.0", 2025-09-18, <https://blog.rust-lang.org/2025/09/18/Rust-1.90.0/>, read on 2026-11-15.
- [RUSTDOC-UNSTABLE] The rustdoc book, "Unstable features", the index page, <https://doc.rust-lang.org/rustdoc/unstable-features.html>, read on 2026-11-15.
- [UPLOAD-PAGES-ARTIFACT] GitHub, the action `actions/upload-pages-artifact`, its README, <https://github.com/actions/upload-pages-artifact>, read on 2026-11-15.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary. TOOL-1 and TOOL-2, which governed the one-time creation of the workspace in M0, are retired, and their labels are not reused.

| Label | Section | Summary |
| --- | --- | --- |
| TOOL-3 | 9.1 | The examples are one member, `slang-rust-examples`, with one binary target per example. |
| TOOL-4 | 9.1 | Licence `MIT OR Apache-2.0`, `rust-version` 1.99, and repository are workspace fields that every member inherits, except the `license` field of the `-sys` and `-src` crates and `slang-slint`. |
| TOOL-5 | 9.1 | `rust-version` changes only in a change whose purpose is to raise it. |
| TOOL-6 | 3.1 | Recipes have the name, group, and `check` membership of Section 3.2. |
| TOOL-7 | 3.1 | Recipes run the commands of Section 4.8 of the coding standard as written there, including the file list of the formatting command, adding only a nextest profile, the arguments that Section 4.8 names for `coverage`, or, when they run code that loads Slang, the library path assignment of SCS-32, except that `test-vendored`, with the nextest and doctest commands, and `check-vendored`, with the Clippy and documentation commands, select the packages of `vendored_packages` with `-p`, enable the feature of `vendored_features`, remove `VULKAN_SDK` and the loader path variables, and set `CARGO_TARGET_DIR=target/vendored`, and `test-vendored` passes `--no-tests=fail`. `doc-site` departs in the same way for the crates of `release_packages` (TOOL-41). |
| TOOL-8 | 3.1 | `check` passes in the container without a hardware GPU or a display. |
| TOOL-9 | 3.1 | No recipe for a tool before the image provides it. |
| TOOL-10 | 4 | The profiles `default`, `gpu`, and `hw-gpu` select tests by whole module path segments, and each profile other than `default` sets its own filter. |
| TOOL-11 | 4 | No profile retries a failed test. |
| TOOL-12 | 4 | The test recipes pass when their profile selects no test, through `--no-tests=warn`. |
| TOOL-13 | 5 | Commit hooks run only checks that do not compile the workspace. |
| TOOL-14 | 6.1 | Custom checkers follow the output and exit status conventions of `scripts/check-prose`, except that a checker whose required input is missing exits with status 2 before it checks any file. |
| TOOL-15 | 6.1 | Each custom checker has a self-test recipe in `check` that runs it on accepted and rejected inputs stored under a name that no tool reads. |
| TOOL-16 | 6.2 | The comment checker reads Rust tokens and ignores literals. |
| TOOL-17 | 6.7 | Landing a checker moves its rule to enforced in every document in the same change. |
| TOOL-18 | 7.1 | Workflows run every check through a recipe. |
| TOOL-19 | 7.2 | Workflow tokens are read-only by default, with extra permissions per job. |
| TOOL-20 | 7.2 | Every action is pinned by a full commit SHA. |
| TOOL-21 | 7.2 | Every download, and every file restored from a cache, is verified against a published SHA-256 checksum before use, and installer actions verify each download and do not fall back. |
| TOOL-22 | 7.2 | No workflow uses `pull_request_target`, reads a secret, or publishes, except the OIDC token of the deploy job of TOOL-42. |
| TOOL-23 | 7.3 | The SDK step reads the version from the top-level `env` of the workflow, verifies the download, sets `VULKAN_SDK`, and makes Slang findable. |
| TOOL-24 | 8 | The allow-list holds the permissive licences of the workspace's crates and grows only with a dependency, and `GPL-3.0-or-later` is accepted only through the exception for `slang-slint`. |
| TOOL-25 | 8 | Path dependencies between members carry a version, and `bans.allow-wildcard-paths` is `false`. |
| TOOL-26 | 8 | Advisories, duplicates, and other sources fail cargo-deny, with exceptions only per item and with a reason. |
| TOOL-27 | 9.1 | Published crates carry the full metadata with `documentation`, and members outside the next release set `publish = false`. |
| TOOL-28 | 10 | Image changes happen in slang-rs-env, and the first change after a rebuild that adds or removes a tool updates the documents. |
| TOOL-29 | 10 | The image provides the tools of Section 4.8, the nightly toolchain with Miri, llvm-tools, GitHub's SSH host keys, the SDK library path for every process, libclang, and git, CMake, Ninja, a C++20 compiler, and Python 3 for the build of slang-glslang on the vendored route. |
| TOOL-30 | 3.1 | `taplo.toml` sets four-space indentation and excludes `target/` and `.worktrees/`, and `typos.toml` accepts the correct words that typos reports. |
| TOOL-31 | 8 | `deny.toml` denies unused licence allow-list entries and unused advisory ignores. |
| TOOL-32 | 3.1 | `default-members` and every `--workspace` command leave out `slang-src`, `slang-glslang-sys`, `slang-rhi-sys`, and `slang-rhi`. |
| TOOL-33 | 9.1 | The root holds each licence text once, each package links its own licence under the conventional name, and a release is packaged on Linux or macOS. |
| TOOL-34 | 8 | A GPL licence is accepted only for `slang-slint`, the examples package, and the Slint crates that they use. cargo-deny enforces `GPL-3.0-or-later` for `slang-slint` alone, and a reviewer checks that no permissive crate depends on a `GPL-3.0-only` crate. |
| TOOL-35 | 9.2 | The `justfile` holds the crates of the next release once, in `release_packages`, and every other member sets `publish = false`. |
| TOOL-36 | 9.2 | Every release publishes every crate of `release_packages`, all but `slang-src` at the shared workspace version, and `slang-src` at the version that SRC-13 derives from it. |
| TOOL-37 | 9.2 | Every release is `0.1.0-alpha.<N>` until the owner decides otherwise, `<N>` rises by one after each release, members require each other exactly, and a failed or wrong release is followed by the next alpha. |
| TOOL-38 | 9.3 | `check-semver` checks the crates of the release with a library target, except `slang-src` and `slang-glslang-sys`, against crates.io, with the default features only. |
| TOOL-39 | 9.3 | The CI job `semver` runs `check-semver` in every run of the workflow, outside `check`. |
| TOOL-40 | 9.4 | The `documentation` field of each released crate is its address on the hosted site. |
| TOOL-41 | 9.4 | `doc-site` builds the site on the vendored route for every crate of the release, checks the page of each crate, writes the index page, and checks for links and the size limit. |
| TOOL-42 | 7.5 | A separate workflow on release tags builds and deploys the site, with Pages permissions on its deploy job alone. |
| TOOL-43 | 9.5 | Before every release, a review of each crate's whole public API against the judgement rules, with every finding fixed or deferred by the owner. |
| TOOL-44 | 9.6 | A release follows the steps of Section 9.6 in order, and the owner's steps need the owner or the owner's go-ahead. |
| TOOL-45 | 9.6 | A release is packaged on Linux or macOS, and `check-package` checks the size, links, licence files, readme, and paths of each package, and that an already published version is unchanged. |
| TOOL-46 | 9.7 | Only the owner publishes or yanks, with a token passed to that one command and stored nowhere. |
| TOOL-47 | 5 | No recipe or command that publishes, tags, deploys, or yanks is on the allow list. |
| TOOL-48 | 3.1 | `check-features` skips every combination of `slango` and of the examples package without `wgpu` and every combination with `vendored` or `rhi`, and only its second command names `rhi`. |
| TOOL-49 | 9.4 | Only the documentation workflow deploys the hosted site, from its own build, and no site built on another machine is deployed. |
| TOOL-50 | 9.9 | The self-test of the release scripts resolves a tool that it needs, such as `just`, before it narrows `PATH`, calls it by its absolute path only, and keeps its directory off `PATH`. |
| TOOL-51 | 9.7 | `publish` stays within the publishing rate limits of crates.io, skips every version that crates.io holds, and publishes nothing when it cannot learn which versions crates.io holds. |
| TOOL-52 | 3.1 | `rhi_packages` and the recipes `check-rhi`, `test-rhi`, and `check-miri-rhi` lint, document, and test the crates that build slang-rhi on the vendored route, the first two on Linux and on x86_64 Windows and the third on Linux, outside `just check`. |
| TOOL-53 | 3.1 | `check-rhi` and `test-rhi` also lint and test `slango` and the examples with the feature `rhi` and without `wgpu`, one package in each command, each command arriving in the code step that makes it pass, with no filter, and `test-rhi` runs every command and every test after a failure. |
| TOOL-54 | 3.1 | Until M11 publishes `slang-rhi`, `slango`, `slango-codegen`, and `slango-macros` set `publish = false` and stay out of `release_packages`. |
