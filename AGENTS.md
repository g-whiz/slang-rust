# Agent Instructions

Every coding agent reads this file before it works in this repository. `CLAUDE.md` imports it, so Claude Code loads the same instructions. The repository is meant to hold an idiomatic Rust library for the Slang shader language and runtime. It is at a very early stage. It holds standards documents, design documents, repository tooling, the configuration of the Rust checks, the binding generator, the crate `slang-compiler-sys`, and the first part of the safe crate `slang-compiler`. `slang-compiler-sys` has its checked-in raw bindings, the pinned Slang version, and a build script that links the Slang of an installed Vulkan SDK or builds Slang from a source tree and links it statically. `slang-compiler` can create a global session and a session with its targets and compiler options so far, and cannot compile a shader yet, as Project Status describes. Development happens inside the development container that the sibling repository slang-rs-env defines.

Best Current Practice (BCP) 14 defines the key words in the next paragraph.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

## Hard Rules

1. **Documentation changes with the code.** The agent MUST update all documentation in the same change as the code it describes, because the next agent reads the documentation as true and builds on it. Documentation Maintenance lists what the agent re-reads.
2. **Documentation follows the writing guide.** The agent MUST write every document, and all application programming interface (API) documentation written with rustdoc, in accordance with `docs/writing-documentation.md`, because the agents and users that read a document depend on its conventions to interpret it. This includes the metadata and change log rules for design documents and the API documentation rules of Section 7 of the writing guide. This file is an instruction file under Section 3.4 of that guide, so it has no metadata table, abstract, or change log, and its unnumbered headings are cited by name.
3. **Rust code follows the coding standard.** The agent MUST write all Rust code in accordance with `docs/rust-coding-standards.md`, because a reviewer checks every change against it and rejects code that breaks it. The standard has two kinds of rules. An agent learns the rules that a lint or tool enforces from the failing check, by reading the lint's row in Section 4.5 and following Section 4.4. The judgement rules, listed in Appendix A, are what an agent reads before it writes Rust. A reviewer checks each change against them and cites the rule identifier in each finding. Section 1.2 of the standard says which sections to read for a change.
4. **Deterministic checks belong to tools.** The agent MUST leave every check that a deterministic tool can perform to that tool, and MUST NOT send anything that such a tool can find to an agent or a large language model (LLM) for review, because a reviewer misses findings that a tool reports every time and spends effort that only judgement can use. An agent that checks by hand something a tool could check adds the tool check in the same change, or proposes it in its report when it cannot add it. A review agent does not report a finding that a configured tool already reports. A new rule that a tool can check goes into the tool configuration, not into a list of judgement rules, as Section 19 of the coding standard and Section 3.5 of the writing guide require. A rule that a tool could check but that no configured tool checks yet stays a judgement rule until a tool covers it.
5. **Comments follow the writing guide.** Rust source contains only the comments that Section 7.2 of the writing guide lists, which are API documentation on public items and `# Safety` sections and `// SAFETY:` comments on unsafe code, and never a `TODO` or a `FIXME`. In the `justfile` and the scripts, the only comments are text addressed to a human, such as the help comment of a recipe or the usage text of a script.
6. **Lint exceptions use `#[expect]`.** The agent MUST record every lint exception as `#[expect(lint, reason = "...")]` on the narrowest scope that needs it and MUST NOT use `#[allow]`, because an `#[allow]` hides the lint silently for as long as it stays, while a stale `#[expect]` fails the check. No agent weakens the lint or tool configuration to make a check pass. Rules LINT-1 and LINT-2 in Section 4.4 of the coding standard define the form of an exception and when the configuration may change.
7. **Soundness and security by default.** When two approaches are otherwise comparable, the agent MUST choose the sounder and more secure one and say so in its report, because unsound or insecure code that works is rarely revisited. Unsafe code appears only in `slang-compiler-sys` and `slang-compiler`, and any other crate that needs it records the reason in its design document first, as Section 4.4 of the coding standard states. Generated bindings, vtables, universally unique identifier (UUID) constants, thunks, and the generated list of C functions in `crates/slang-compiler-sys/tests/sys/symbols.rs` are never edited by hand, and the agent changes them only by running the binding generator again (FFI-1, COM-9). No file in the repository contains a secret. A build script downloads nothing, installs nothing, and writes only inside `OUT_DIR` (SYS-5). A continuous integration (CI) workflow verifies the SHA-256 checksum of every download whose upstream publishes one.
8. **Every action is a `just` recipe.** Every command, build step, and check is a recipe in the root `justfile`, which is one flat file with `[group]` attributes and no `just` modules, because modules hide their recipes from `just --list`. The agent invokes no script under `scripts/` directly when a recipe wraps it. Every recipe has a help comment, every shell script starts with `#!/usr/bin/env bash` and `set -euo pipefail`, and every shell script and every recipe that contains shell is `shellcheck`-clean. Section 4.2 of `docs/design/tooling.md` lists every recipe, and Validating Changes names the recipes to run for each kind of change.
9. **No attribution.** No output contains a generated-by line, a `Co-Authored-By` trailer, or a link to an agent session, because such a line says nothing about the change and a session link can expose a private session. This covers commits, pull requests, issues, and files. The `commit-msg` hook rejects such lines, and `just merge` strips them.
10. **No extra documentation.** The agent writes no document beyond `README.md`, `AGENTS.md`, `CLAUDE.md`, and the `docs/` tree, with design documents in `docs/design/`, because every extra document is one more place that can diverge from the code.
11. **Work in a worktree.** Files change only in a git worktree under `.worktrees/`, never in the main checkout, because the main checkout is where `just merge` runs. Branches are named `feature/<short-description>`, lowercase and hyphenated, because the repository has no ticket keys. The agent commits without asking and pushes its branch to run CI, because the project opens no pull requests and a push to a branch that matches `feature/**` is what runs CI on a branch. The agent MUST run `just merge` only after the CI run for the last commit of the branch concluded `success` with every job succeeded, because a merge without that result can put a failing commit on `main`. A fresh branch that holds the tree of a verified branch counts as verified, as Development Workflow describes. The agent MAY push `main` after the merge. Agents open no pull requests and never force-push.
12. **Canadian English and ASCII punctuation.** All prose, including API documentation, commit messages, and recipe help text, uses Canadian English spelling and ASCII punctuation, as Section 8 of the writing guide defines, because one spelling lets an agent find every occurrence of a word with one search.
13. **Consult the references before inventing a pattern.** When an agent is about to use a design pattern, or to make a design choice that the coding standard does not cover, it MUST look the topic up in `docs/rust-references.md`, read the sources starting at the highest tier, and extend the standard in the same change, because a reviewer cannot check code against a rule that does not exist yet. Section 1.1 of the reference list gives the lookup steps, and Section 19 of the coding standard gives the steps to add a rule.

## Project Status

As of 2026-11-11, the repository is at a very early stage. Milestone M0, tooling and skeleton, is complete. Milestone M1, the binding generator and `slang-compiler-sys`, is complete: its design document exists, and all six steps of its Section 15 are done. Step 1 added the pin file, the binding generator, and their tests. Step 2 added the installation of the pinned Vulkan SDK in every CI job and the CI job `coverage`. Step 3 checked in the generated bindings with their crate documentation, and added the regeneration test, the recipes `generate-bindings` and `check-bindings`, and `.gitattributes`. Step 4 added the build script of `slang-compiler-sys`, which checks the Slang of the Vulkan SDK that `VULKAN_SDK` names against the pin and links it. It also added the tests of the script's logic, the smoke test and the native-symbol test, which run on Linux, Windows, and macOS, the generated list of C functions for the native-symbol test, the library path on the command line of the recipes that run tests, and the recipe `check-doc-without-slang`. Step 5 added the static route of the build script, which builds Slang from a clean git checkout of the pinned commit and links it statically, the file `system.rs` with the real implementation of the script's environment trait and its tests, the tests of the static route, and the recipe `test-static`. It also made `fmt` and `check-fmt` format every Rust file that git tracks or does not ignore, including the files that a crate pulls in with `include!`. Step 6 checked the crate-level documentation of `slang-compiler-sys` against its design and completed it, and amended rule SYS-1 of the coding standard so that it covers the git files that the static route watches. The current milestone is M2, the safe crate `slang-compiler` and the compile-and-reflect example. Its design document, `docs/design/slang-compiler.md`, exists as a draft, and steps 1 and 2 of the seven steps of its Section 15 are done, both on 2026-11-11. Step 1 added the crate `crates/slang-compiler` with the owning interface pointer, the error type and its mapping from Slang's failure signals, the Slang call guard with its deferred releases, `GlobalSession` with its builder, the run-time check of the build tag, the rule of one global session for each thread, a fake of the Slang interfaces for the tests under Miri, and their tests. Step 2 added `Session` with `SessionBuilder`, `Target`, and `TargetIndex`, `CompilerOptions` with its typed setters and its `unsafe` raw setters, `OptionName`, six more enumerations, a fake session, and their tests. Steps 3 to 7 remain, and step 3 is next. What stays open after M1 is in the rows of Sections 16 and 17 of `docs/design/slang-compiler-sys.md`. The agent updates this section, and its date, in the same change that alters what exists.

What exists and works:

- The `justfile` and its recipes, which `just --list` shows, including the recipes that run the Rust checks, the recipe `pin`, which prints the pin, the recipes `generate-bindings` and `check-bindings`, which regenerate the two generated files and check them against a regeneration, the recipe `check-doc-without-slang`, which builds the documentation with `DOCS_RS` set and `VULKAN_SDK` removed, the recipe `test-static`, which runs the tests of `slang-compiler-sys` on the static route, the recipes `fmt` and `check-fmt`, which format and check every Rust file that git tracks or does not ignore, and the recipes `install-vulkan-sdk` and `check-vulkan-sdk`, which install the pinned Vulkan SDK on a CI runner and check an installed SDK against the pin. Its variable `slang_library_path` holds the assignment that puts `$VULKAN_SDK/lib` on the library path of Linux or macOS. The recipes that run compiled code which loads Slang put it at the start of their command line (SCS-32 of `docs/design/slang-compiler-sys.md`).
- The scripts under `scripts/`, which the recipes and hooks run, including `install-vulkan-sdk` and `check-vulkan-sdk`, and the custom checkers of Section 7 of `docs/design/tooling.md`: `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs`, and `check-workflow-pins`, the extension of `check-prose` to the documentation comments of Rust files, and a self-test recipe for each.
- The git hooks in `.pre-commit-config.yaml`.
- The CI workflow `.github/workflows/ci.yml`, which runs on every push to `main`, every push to a branch that matches `feature/**`, and every pull request. Every job installs the pinned Vulkan SDK, verified against a SHA-256 checksum that the workflow records, and checks it against the pin, on Linux, Windows, and macOS. The job `coverage` measures test coverage on Linux and keeps the report as an artifact. Section 8 of `docs/design/tooling.md` specifies the jobs. On 2026-11-10, all six jobs, counting each entry of a matrix, passed on GitHub. Afterwards, on 2026-11-10 and 2026-11-11, CI on `main` failed in the Windows job for three commits, from the commit that added the static route of `slang-compiler-sys` to the commit that added the design of `slang-compiler`, because one test of the real host of the build script asserted an error kind that Windows reports differently. The commit "Assert only the variant of a start failure of the real host" fixed it, and run 42833561065 on `main` passed all six jobs. On 2026-11-11, the workflow gained the trigger for pushes to feature branches, and run 42861650459, started by a plain push to a feature branch with no pull request, passed all six jobs.
- The editor settings in `.editorconfig`.
- The Claude Code settings in `.claude/settings.json`, which allow a list of recipes to run without asking and run the prose checker after every edit of a Markdown file or a Rust file.
- The root `.gitattributes`, which keeps LF line endings in every checkout on every platform.
- `CLAUDE.md`, which imports this file.
- The workspace manifest `Cargo.toml`, with the `[workspace.package]` fields that every member inherits and the workspace lint tables of Section 4.2 of the coding standard.
- The tool configuration: `clippy.toml`, `deny.toml`, `.config/nextest.toml`, `taplo.toml`, and `typos.toml`.
- `crates/slang-compiler-sys`, with publication metadata, the raw bindings that the binding generator wrote and that are checked in as `src/bindings.rs`, re-exported at the crate root, and crate-level documentation that states the safety contract that every generated function shares. Its root also includes the three constants of the pin from `crates/slang-compiler-sys/pin.rs`, the only record of the pinned Vulkan SDK version, Slang build tag, and Slang commit. It has no dependencies. Its build script `build.rs`, with its logic in `route.rs` and its real environment in `system.rs`, has two routes. By default, it reads `VULKAN_SDK`, checks the build tag of the Slang headers there against the pin, checks the library directory and the library file, and links the Slang shared library of that SDK dynamically. When `SLANG_SOURCE_DIR` is present, it takes the static route instead: it checks through git that the variable names the top level of a clean git checkout of the pinned commit, copies the tracked files and eight required submodules into `OUT_DIR`, builds a minimal static Slang there with CMake and Ninja, checks the archives and the build tag, and links six static libraries and the C++ standard library. The static route never reads `VULKAN_SDK`, never falls back to the dynamic route, refuses a source archive without git metadata, a changed tree, and cross-compilation, and needs git, CMake, and Ninja. When `DOCS_RS` is present, the build script links nothing. Its integration-test binary `tests/sys/` tests the consistency of the pin constants, tests every route and failure of the build script against a fake environment, tests the real environment of `system.rs`, and holds the smoke test, which creates a global session and checks its build tag, and the native-symbol test, which references every C function of the bindings. The smoke and symbol tests ran and passed on Linux, Windows, and macOS in CI on 2026-11-10, and on the static route through `test-static` in the development container on the same date. The static route has run only on x86_64 Linux. Its code for macOS and for Windows has never run.
- `crates/slang-compiler`, the first part of the safe crate, with publication metadata and `slang-compiler-sys` as its only dependency. Its public API is `GlobalSession` with `new`, `builder`, `build_tag`, `check_target_support`, and the raw escape functions `from_raw`, `into_raw`, and `as_raw`, `GlobalSessionBuilder` with `glsl`, `min_language_version`, and `build`, `Session` with `builder`, `global_session`, `target_count`, `target_index`, and the raw escape functions, `SessionBuilder` with `target`, `search_path`, `define`, `matrix_layout`, `use_up_to_date_binary_module`, `options`, and `build`, `Target` with `new`, `profile`, `capability`, and `options`, `TargetIndex`, `CompilerOptions` with 15 typed setters and the `unsafe` raw setters `set_int` and `set_string`, `OptionName`, the error type `Error` with its alias `Result`, `ErrorKind` with its ten kinds, `Operation`, `Diagnostics`, the record `Compiled`, and the enumerations `TargetFormat`, `Toggle`, `LanguageVersion`, `MatrixLayout`, `Stage`, `OptimizationLevel`, `DebugInfoLevel`, `FloatingPointMode`, and `LineDirectiveMode`. Creating a global session reads the build tag of the loaded library through `spGetBuildTagString` and compares it with the pin before it creates anything, and a second global session on a thread on which one is alive fails with `ErrorKind::AlreadyExists`. `SessionBuilder::build` checks every setting before it calls Slang to create the session. `Session::into_raw` returns the session's `GlobalSession` together with the raw pointer, and `Session::from_raw` takes the target count as a third argument (SLC-4 of the design). Internally, the crate counts every reference through one owning interface pointer, maps every failure signal of Slang to an `ErrorKind` in one function, and keeps a per-thread Slang call guard that panics on a reentrant call and defers releases until the outermost call returns. With it, a user can create a global session, read its build tag, check which target formats Slang reports that it can produce, create a session with targets, search paths, preprocessor macros, a default matrix layout, and compiler options, and select a target by its position. A user cannot yet load a module, link, generate code, reflect, or give a session a custom file system, so no test has yet observed the effect of a compiler option on generated code. Its unit tests run under Miri against a fake of the Slang interfaces, and its integration-test binary `tests/compiler/` holds `ffi` tests that call Slang, including the test of assumption CA-14 of its design, which found no fault but cannot show the absence of one because of the limits that the design lists, so CA-14 stays open and the rule of one global session for each thread stays. On 2026-11-11, all of them passed in CI on Linux, Windows, and macOS, in run 42853770750.
- The binding generator `tools/bindings-generator`, a library with a thin binary, which parses the Slang headers with libclang and writes the raw bindings and the list of their C functions, with its rule set, its shim headers, and its unit tests. Its integration-test binary `tests/generator/` holds the regeneration test, which runs the generator on the headers at `VULKAN_SDK` and fails when either result differs from its checked-in file, `src/bindings.rs` or `tests/sys/symbols.rs` of `slang-compiler-sys`, in any byte.
- `LICENSE` and `README.md`.
- The writing guide, the coding standard, and the reference list under `docs/`.
- The architecture design, `docs/design/architecture.md`, which specifies the planned crates, how Slang is obtained and pinned, and the roadmap of milestones M0 to M7.
- The tooling design, `docs/design/tooling.md`, which specifies the toolchain configuration, the recipes, the hooks, the custom checkers, the CI workflow, and the requirements on the container image. Milestone M0 implemented it, and its Section 1.6 describes the current state.
- The design of `slang-compiler-sys` and the binding generator, `docs/design/slang-compiler-sys.md`, which milestone M1 implemented in the order that its Section 15 gives. Its Section 1.6 describes the current state.
- The design of `slang-compiler` and the compile-and-reflect example, `docs/design/slang-compiler.md`, a draft that milestone M2 implements in the order that its Section 15 gives. Its Section 1.6 says which parts exist.

`just check` passes in the development container and in CI. The tests of the binding generator, including the regeneration test on Linux, and the tests of `slang-compiler-sys` run in the default tier, and no test of a GPU tier exists yet. On 2026-11-11, the workspace has 278 tests on Linux, of which Miri runs 184 and skips 94. `slang-compiler` has 125 of them: 104 unit tests, all of which run under Miri, and 21 `ffi` tests. CI does not build the static route.

Building the workspace now needs the pinned LunarG Vulkan SDK 1.4.363.1 and `VULKAN_SDK` set to its directory, which the development container and every CI job provide. Every command that compiles the workspace, such as `cargo check`, `cargo clippy`, `cargo test`, and every recipe that compiles except `check-doc-without-slang`, runs the build script of `slang-compiler-sys`. Without `VULKAN_SDK`, the build script stops the build with an error that starts "VULKAN_SDK is not set, so slang-compiler-sys cannot find the Slang library to link" and says what to install and set. With an SDK that ships another Slang build, the error names both build tags. When `SLANG_SOURCE_DIR` is set in the environment, every command that compiles the workspace builds Slang from source instead, which takes several minutes for each target directory and profile, so the agent leaves it unset except for `test-static`. A test binary that starts without `$VULKAN_SDK/lib` on the library path stops before any test runs, on Linux with "error while loading shared libraries: libslang-compiler.so.0.2026.17: cannot open shared object file", so the agent runs the tests through their recipes, which supply that path. Section 19.4 of `docs/design/slang-compiler-sys.md` lists every failure and its recovery.

What does not exist yet:

- The parts of `slang-compiler` that steps 3 to 7 of Section 15 of `docs/design/slang-compiler.md` add: modules, entry points, linking, code generation, blobs, and metadata, reflection, the custom file system, and the complete crate documentation. The crate cannot compile a shader yet, and no test has observed the effect of a setting of a session on generated code.
- Any crate of the design above `slang-compiler`.
- A run of the static route on macOS or Windows, and a CI job that builds the static route.
- Examples, including the package `slang-rust-examples`, and tests of the GPU tiers.
- The recipe `check-semver`, which arrives with the first release in M7.
- The CI job `semver`, which arrives in M7.
- The design document of each crate other than `slang-compiler-sys` and `slang-compiler`, which is written as the first task of the milestone that delivers the crate.

The workspace lint tables, `clippy.toml`, the tools that `just check` runs, and the custom checkers enforce their rules, so an agent learns those rules from a failing check, as Hard Rule 3 states. The custom checkers enforce the rules of the writing guide on comments in Rust source, on the headings of item documentation, on `ignore` in doctests, on the `# Features` section, and on ASCII punctuation, semicolons, and listed spellings in API documentation, and the rules of Section 8.2 of `docs/design/tooling.md` on pinned actions, the `pull_request_target` event, and top-level workflow permissions. Section 7.12 of the writing guide and Section 4.8 of the coding standard say which part of each rule a checker enforces. Every rule that no configured tool checks stays a judgement rule. `check-features`, `check-msrv`, `check-vulkan-sdk`, and `coverage` run outside `just check`, and CI runs them in every run of the workflow. The agent MUST NOT assume that a planned crate, recipe, file, or tool exists, because an agent that builds on something absent produces code that cannot work.

## Repository Layout

- `AGENTS.md`: This file.
- `CLAUDE.md`: A one-line file that imports `AGENTS.md` for Claude Code.
- `Cargo.toml`: The workspace manifest, with resolver 3, the `[workspace.package]` fields that every member inherits, such as edition 2024, `rust-version`, and the licence, the dependencies `clang` and `slang-compiler-sys`, by path and version, in `[workspace.dependencies]`, and the workspace lint tables. Its members are `crates/slang-compiler`, `crates/slang-compiler-sys`, and `tools/bindings-generator`.
- `Cargo.lock`: The lock file of the workspace.
- `LICENSE`: The text of version 3 of the GNU General Public License.
- `README.md`: The README, which is also the readme of every member through the inherited `readme` field.
- `crates/slang-compiler-sys/`: The crate of raw bindings that Project Status describes, with `Cargo.toml`, which declares `links = "slang-compiler"` and whose `[package.metadata.docs.rs]` table lists the four supported targets, and `src/lib.rs`, which holds the crate-level documentation, the opt-out of `unsafe_code`, the declaration of the module `bindings` with its lint exception, the re-export of its items, and the inclusion of the pin.
- `crates/slang-compiler-sys/src/bindings.rs`: The generated raw bindings, which `just generate-bindings` writes. The file is generated and never edited by hand (Hard Rule 7), and the regeneration test fails on any difference between it and a fresh regeneration.
- `crates/slang-compiler-sys/pin.rs`: The pin file, the only record of the pinned Vulkan SDK version, Slang build tag, and Slang commit, which the crate root and the binding generator include and the recipe `pin` prints (Section 8.1 of `docs/design/slang-compiler-sys.md`).
- `crates/slang-compiler-sys/build.rs`: The build script, which includes `pin.rs`, `route.rs`, and `system.rs`, runs the logic on the real system, prints the Cargo instructions, and returns the result (Section 9.7 of `docs/design/slang-compiler-sys.md`).
- `crates/slang-compiler-sys/route.rs`: The logic of the build script on both routes, as plain functions behind the trait `Host` for everything it reads, runs, or writes, which the build script and its test both include (SYS-8 of the coding standard). It holds the static route with its git checks, copy, stamp, `cmake` runs, and verification (Section 9.3 of `docs/design/slang-compiler-sys.md`).
- `crates/slang-compiler-sys/system.rs`: The struct `System`, which implements `Host` on the real environment, file system, and processes with the standard library only, and which the build script and the test module `tests/sys/build_script/ffi.rs` both include.
- `crates/slang-compiler-sys/tests/sys/`: The integration-test binary of the crate, with `main.rs` and the modules `pin.rs`, which tests the constants of the pin, `build_script.rs`, which includes `route.rs` and tests every route and failure of the build script against a fake environment, `ffi.rs`, which holds the smoke test and the native-symbol test and carries the only `unsafe_code` exception of the test binary, and `symbols.rs`.
- `crates/slang-compiler-sys/tests/sys/build_script/ffi.rs`: The tests of `System`, which include `system.rs`, start `rustc`, and write files under `CARGO_TARGET_TMPDIR`, so they sit in a module named `ffi`, which Miri skips.
- `crates/slang-compiler-sys/tests/sys/symbols.rs`: The generated list of the C functions of the bindings with their addresses, which `just generate-bindings` writes and the native-symbol test reads. The file is generated and never edited by hand (Hard Rule 7), and the regeneration test fails on any difference between it and a fresh regeneration.
- `crates/slang-compiler/`: The safe crate that Project Status describes, with `Cargo.toml`, which inherits `slang-compiler-sys` from `[workspace.dependencies]` and whose `[package.metadata.docs.rs]` table lists the four supported targets (Section 4.1 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/lib.rs`: The crate-level documentation of the current scope, the opt-out of `unsafe_code`, the declarations of the private modules, and the re-exports of the public items at the crate root.
- `crates/slang-compiler/src/owned.rs`: The owning interface pointer `Owned`, the only code that adds or releases a reference, and the crate-private unsafe trait through which it calls the generated thunks of `addRef` and `release` (SLC-2).
- `crates/slang-compiler/src/guard.rs`: The Slang call guard, its queue of deferred releases, and the per-thread record of live global sessions, each in a `thread_local!` (SLC-28, SLC-29).
- `crates/slang-compiler/src/error.rs`: The public types `Error`, `Result`, `ErrorKind`, `Operation`, `Diagnostics`, and `Compiled` (Sections 6.8 and 6.9 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/mapping.rs`: The one function that maps every failure signal of Slang to an `ErrorKind` (SLC-11).
- `crates/slang-compiler/src/enums.rs`: The enumerations `TargetFormat`, `Toggle`, `LanguageVersion`, `MatrixLayout`, `Stage`, `OptimizationLevel`, `DebugInfoLevel`, `FloatingPointMode`, and `LineDirectiveMode`, with their crate-private conversions to the raw values (SLC-14).
- `crates/slang-compiler/src/options.rs`: `CompilerOptions` with its typed and raw setters, `OptionName`, the list of the options that a target rejects, and the crate-private types that own the C strings of the option entries during a call to Slang (Section 7 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/session.rs`: `Session`, `SessionBuilder`, `Target`, and `TargetIndex`, with the checks of `build`, the building of the session descriptor, and the filling of the fields of each target descriptor (SLC-6, SLC-31).
- `crates/slang-compiler/src/strings.rs`: The one conversion of text that the crate passes to Slang into a C string, which reports a NUL byte as an error (SLC-15).
- `crates/slang-compiler/src/global_session.rs`: `GlobalSession` and `GlobalSessionBuilder`, with the check of the build tag and the order of creation of Section 6.3 of `docs/design/slang-compiler.md`.
- `crates/slang-compiler/src/fake.rs`: The test-only fake of the Slang interfaces built on the generated vtable structs, which the unit tests run under Miri (SLC-25). Its global session records each session descriptor that it receives, and its session writes into its global session after releasing it, as Slang's session does, so that Miri reports a wrong release order.
- `crates/slang-compiler/tests/compiler/`: The integration-test binary `compiler`, with `main.rs`, which carries the `unsafe_code` exception on the module `ffi`, `ffi.rs`, which holds the `ffi` tests of the global session through the public API, `ffi/session.rs`, which holds the `ffi` tests of sessions, targets, and compiler options, and `ffi/two_global_sessions.rs`, which holds the test of CA-14 through the raw bindings.
- `tools/bindings-generator/`: The binding generator, an unpublished workspace member with `Cargo.toml`, the library under `src/` with the modules `parse`, `rules`, `check`, `emit`, `model`, and `error`, the unit tests in `src/tests.rs` and in the modules, and the binary `src/main.rs`.
- `tools/bindings-generator/tests/generator/`: The generator's integration-test binary, with `main.rs` and the module `ffi.rs`, which compiles for Linux only and holds the regeneration test (Section 7.6 of `docs/design/slang-compiler-sys.md`).
- `tools/bindings-generator/src/rules.rs`: The rule set of the binding generator, which decides every declaration and macro of the Slang headers (Section 4 of `docs/design/slang-compiler-sys.md`).
- `tools/bindings-generator/shim/`: The shim headers that replace the system headers when the generator parses for each target.
- `clippy.toml`: The Clippy configuration of Section 4.3 of the coding standard.
- `deny.toml`: The cargo-deny configuration of Section 9 of `docs/design/tooling.md`, whose licence allow-list holds `GPL-3.0-or-later`, `Apache-2.0`, and `ISC`.
- `.config/nextest.toml`: The nextest profiles `default`, `gpu`, and `hw-gpu`, which implement the test tiers as Section 5 of `docs/design/tooling.md` defines, and `default-miri`, which Miri uses and which skips the modules named `ffi`, `gpu`, and `hw_gpu`.
- `taplo.toml`: The taplo configuration, with four-space indentation and the exclusion of `target/` and `.worktrees/`.
- `typos.toml`: The typos configuration, which ignores two correct patterns that typos would otherwise report, and accepts three whole identifiers of the generated bindings that the Slang headers misspell (Section 5.10 of `docs/design/slang-compiler-sys.md`).
- `docs/writing-documentation.md`: The normative writing guide.
- `docs/rust-coding-standards.md`: The normative Rust coding standard, with the specification of the lint and tool configuration.
- `docs/rust-references.md`: The reference list of external sources behind the coding standard, with a topic index.
- `docs/design/architecture.md`: The architecture design, which is authoritative for the crates, their layering, how Slang is obtained and pinned, the examples, the test tiers, platform support, publishing, and the roadmap.
- `docs/design/tooling.md`: The tooling design, which is authoritative for the toolchain configuration, the recipes, the hooks, the custom checkers, the CI workflow, the dependency policy, and the requirements on the container image. Milestone M0 implemented it, and its Section 1.6 describes the current state.
- `docs/design/slang-compiler-sys.md`: The design of `slang-compiler-sys` and the binding generator, which is authoritative for the scope and shape of the generated bindings, the supported targets, the generator, the pin file, the build script on both routes, and the tests of the crate.
- `docs/design/slang-compiler.md`: The design of `slang-compiler` and the compile-and-reflect example, which is authoritative for the facts about Slang that the safe crate relies on, its ownership model, public API, errors, compiler options, file system, reflection, threading rules, soundness invariants, and tests, and for the order of the changes of M2.
- `justfile`: The root `justfile`, with the groups `checks`, `ci`, `git`, and `rust`. The group `ci` holds `install-vulkan-sdk`, which only the CI workflow runs.
- `.github/workflows/ci.yml`: The CI workflow, which Section 8 of `docs/design/tooling.md` specifies. It runs on every push to `main`, every push to a branch that matches `feature/**`, and every pull request. It has the jobs `check`, `test`, `ci-only`, and `coverage`, each of which reads the pin through `just pin` and installs and checks the pinned Vulkan SDK. Its top-level `env` records the SHA-256 checksums of the SDK downloads, and its `check` and `coverage` jobs install libclang for the tests of the binding generator, which include the regeneration test.
- `scripts/install-vulkan-sdk`: The script that downloads the Vulkan SDK on a GitHub Actions runner, verifies it against a checksum, installs it, and exports `VULKAN_SDK`, the library path on Linux and macOS, and the directory of the SDK's programs to the later steps of the job (Section 8.3 of `docs/design/tooling.md`).
- `scripts/check-vulkan-sdk`: The script that checks that the SDK at `VULKAN_SDK` provides the Slang headers, library, and `slangc` of the build tag of the pin.
- `scripts/check-prose`, `scripts/prose-wordlist.txt`: The prose checker for Markdown files and for the documentation comments of Rust files, and its list of non-Canadian spellings.
- `scripts/check-rust-comments`: The checker that Rust source holds only API documentation and safety comments.
- `scripts/check-doc-headings`: The checker of the headings in item documentation.
- `scripts/check-doctest-attributes`: The checker that no documentation code block is marked `ignore`.
- `scripts/check-feature-docs`: The checker that the crate-level documentation of each library names every Cargo feature.
- `scripts/check-workflow-pins`: The checker that every action is pinned by a full commit SHA, and that no workflow uses `pull_request_target` or grants more than read access at its top level.
- `scripts/rust_lexer.py`: The Python module that the custom checkers share, which reads Rust tokens and documentation comments.
- `scripts/self-test-checker`: The runner of the self-test of each custom checker.
- `scripts/fixtures/`: The cases of each self-test, one directory per checker and per case, with input files that end in `.fixture`.
- `scripts/check-shell`, `scripts/justfile-recipes`: The `shellcheck` run over every shell script and every recipe.
- `scripts/check-commit-message`: The commit message checker.
- `scripts/claude-hook-prose`: The Claude Code hook that runs the prose checker after an edit to a Markdown file or a Rust file.
- `scripts/merge-branch`: The squash merge that `just merge` runs.
- `scripts/worktree-prune`: The cleanup of squash-merged branches that `just worktree-prune` and the `post-merge` hook run.
- `.pre-commit-config.yaml`: The `pre-commit`, `commit-msg`, and `post-merge` hooks.
- `.claude/settings.json`: The Claude Code settings for this repository, with the allowed commands and the hook that runs the prose checker after every edit of a Markdown file or a Rust file. The allowed commands include `just check`, every recipe that it runs except `check-doc-without-slang`, `check-bindings`, `check-vulkan-sdk`, `check-features`, `check-commit-message`, `pin`, and the three test recipes of the tiers, and they exclude recipes such as `fmt`, `generate-bindings`, `merge`, `check-msrv`, `coverage`, and `test-static` (Section 6 of `docs/design/tooling.md`).
- `.editorconfig`: The character set, line endings, final newline, and indentation for each file type.
- `.gitattributes`: The single line `* text=auto eol=lf`, which keeps LF line endings in every checkout on every platform (Section 3.4 of `docs/design/tooling.md`).
- `.gitignore`: The files that git ignores.

`target/`, `.worktrees/`, `.code-review/`, and `.idea/` are local and ignored by git. `test-static` builds in `target/static`.

These locations are planned and do not exist yet:

- `docs/design/<crate-name>.md`: One design document for each crate other than `slang-compiler-sys` and `slang-compiler`, written as the first task of the crate's milestone.
- One directory for each planned crate other than `slang-compiler-sys` and `slang-compiler`, and `examples/` for the package `slang-rust-examples`, as Section 3.1 of `docs/design/architecture.md` lists.

## Technology Decisions

`docs/design/architecture.md` is authoritative for the architecture: the crates and their layering, how Slang is obtained and pinned, the responsibilities of each crate, the examples, the test tiers, platform support, publishing, and the roadmap. This section summarizes it, together with the toolchain and environment decisions, and the architecture design prevails wherever the two disagree. `docs/design/tooling.md` is the design for the toolchain configuration, the recipes, the hooks, the CI workflow, and the requirements on the container image, and it prevails over this section on those topics. Milestone M0 implemented it, and its Section 1.6 says which parts exist. `docs/design/slang-compiler-sys.md` is the design of `slang-compiler-sys` and the binding generator, and `docs/design/slang-compiler.md` is the design of `slang-compiler` and the compile-and-reflect example, and each prevails over this section on its topics.

The planned crates are:

- `slang-compiler-sys`: The raw bindings only, generated and checked in, with a build script that links the native library and the manifest key `links = "slang-compiler"`. Its directory holds the checked-in bindings, the pin, and the build script, which links the Slang of an installed Vulkan SDK or builds Slang from source and links it statically, as Project Status describes.
- `slang-compiler`: Safe, idiomatic wrappers over `slang-compiler-sys`, with no graphics processing unit (GPU) dependency. Every wrapper holds the object that Slang created it from, and no wrapper, builder that holds one, reflection handle, or iterator over them is `Send` or `Sync`. A call that returns diagnostics returns its value together with them on success, and every failure is the crate's one `Error` type, whose kind callers match on (Sections 5.2, 6.8, 6.9, and 10 of `docs/design/slang-compiler.md`). Its first part, with the global session, sessions, compiler options, and the error type, exists, as Project Status describes. Two rules bind every change to its compiler options, because Slang reads some options only from fields of the target descriptor and others only from the session. `SessionBuilder::build` fills the floating-point mode, the line directive mode, the scalar layout, and the method of generating SPIR-V of each target descriptor. It takes each from the target's options, otherwise the session's, and reads it exactly as Slang reads it, so that a setting of the session is not silently overwritten (SLC-31 of `docs/design/slang-compiler.md`). `SessionBuilder::build` rejects `LanguageVersion`, `WarningsAsErrors`, `EnableWarning`, and `DisableWarning` in the options of a target with `ErrorKind::InvalidArgument`, because the pinned Slang never reads them there (SLC-6).
- `slango`: A high-level convenience library, with a feature set like SlangPy and design patterns like vulkano. Its first runtime backend is wgpu, and its design does not rule out a slang-rhi backend later. It first offers a runtime call API driven by reflection, and its first milestone covers compute kernels compiled to SPIR-V only. Rendering and a shader target configurable per backend follow.
- `slango-macros`: A later build-time macro that generates typed Rust bindings to Slang code on the same core as the runtime API.
- `slang-slint`: An extension to Slint that provides a view backed by a Slang render pipeline. It uses `slango` and supports wgpu only, permanently.
- The binding generator: A tool in this repository that parses the Slang headers with libclang. It is not published.
- The examples, which also serve as the integration testbed and are not published: a command-line program that compiles and reflects Slang source, a headless compute kernel with a broadcasting case and a differentiable function, a windowed renderer with winit and shader hot reload, a Slint application with a `slang-slint` view, and later a path-tracing demo.

Bindings for slang-rhi, `slang-rhi-sys` and `slang-rhi`, are deferred and not part of the first plan.

The binding generator produces the `#[repr(C)]` vtable structs, UUID constants, and thunks through which Rust calls the Slang interfaces in the style of the Component Object Model (COM). None of them is written by hand. Every named C++ enumeration becomes a `#[repr(transparent)]` integer newtype with associated constants, never a Rust `enum`, and the enumerators of an unnamed enumeration become constants of the integer typedef that a rule assigns. The raw bindings are checked in, and a test regenerates them and fails if the result differs, so users of the crates need no libclang. The generator also writes the list of the C functions of the bindings that the native-symbol test of `slang-compiler-sys` uses, so no list of generated names is kept by hand. The bindings support 64-bit targets only, and `slang-compiler-sys` stops compilation on any other.

The Slang version is always the one that ships in the latest stable LunarG Vulkan software development kit (SDK). The first target is Vulkan SDK 1.4.363.1, whose `slangc` reports `2026.17-25-gc7954ebd1`. That is a commit past the tag `v2026.17`, so the pin is the SDK version together with the Slang commit, not a Slang tag. The pin is recorded only in `crates/slang-compiler-sys/pin.rs`, which every consumer reads. Slang is obtained in exactly one of two ways. By default, the build uses an installed Vulkan SDK and links Slang dynamically. As an opt-in, a static build builds Slang from source and links it statically, selected by setting the environment variable `SLANG_SOURCE_DIR` to the top level of a clean git checkout of the pinned commit with its submodules. The build script implements both ways. The static build copies the tracked files into `OUT_DIR` and builds the copy, because Slang's build writes into its source tree, and it refuses a tree that git cannot verify as the pinned commit, a changed tree, and cross-compilation (Section 9.3 of `docs/design/slang-compiler-sys.md`). No Cargo feature selects the route. There is no prebuilt download and no other way.

Development uses the latest stable Rust toolchain. The workspace declares the minimum supported Rust version (MSRV) 1.99 in `rust-version`, and `check-msrv` verifies it with cargo-hack. The nightly toolchain serves only checkers such as Miri and never builds the crates. The crates use edition 2024 and Cargo resolver 3. The licence is GPL-3.0-or-later, and `LICENSE` holds its text.

CI runs on GitHub Actions on hosted runners for Linux, Windows, and macOS, installs the pinned Vulkan SDK on all three, and runs the default test tier on all three. The GPU tier is planned there, with lavapipe on Vulkan on Linux, the Windows Advanced Rasterization Platform (WARP) on Direct3D 12 on Windows, and hosted Metal on macOS, from the first test that needs an adapter. Tests that need a GPU adapter sit in a module named `gpu`, which runs on any adapter including lavapipe and WARP, or in a module named `hw_gpu`, which needs a hardware GPU and runs locally only. A test that finds no adapter fails and never skips silently.

The published crates are planned for crates.io, including docs.rs builds that work without Slang installed and checks for breaking changes. Nothing is published without the owner's explicit go-ahead.

The development container provides this environment:

- Rust stable 1.99.1, the default toolchain, with Clippy, rustfmt, rust-src, and llvm-tools.
- A nightly toolchain with Miri and rust-src, used only for checkers.
- cargo-nextest, cargo-deny, cargo-insta, cargo-expand, cargo-semver-checks, cargo-hack, cargo-machete, cargo-llvm-cov, the bindgen command-line interface (CLI), typos, taplo, actionlint, `just`, pre-commit, shellcheck, `gh`, CMake, and Ninja.
- GitHub's Secure Shell (SSH) host keys in the known hosts file.
- The LunarG Vulkan SDK 1.4.363.1 at `$VULKAN_SDK`, which ships Slang. Its `slangc` reports `2026.17-25-gc7954ebd1`, the headers are under `$VULKAN_SDK/include/slang`, `libslang-compiler.so` is under `$VULKAN_SDK/lib`, and the pkg-config file is `slang-compiler.pc`. A login shell has `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH`. A process that no login shell started does not, until the next rebuild of the container image applies the fix that slang-rs-env already contains. The recipes that run compiled code which loads Slang supply that path on their own command line, so they do not depend on it.
- The Slang source tree at `/opt/slang`, at the same commit as the SDK's Slang, with every submodule checked out. It is source only and not built in place, and `test-static` accepts it as `SLANG_SOURCE_DIR`.
- Two Vulkan devices: an NVIDIA RTX 4070 and lavapipe, a software implementation of Vulkan.
- The CUDA 13.5 toolkit.

Recipes run Clippy, rustfmt, cargo-nextest, cargo-deny, cargo-hack, cargo-machete, cargo-llvm-cov, typos, taplo, Miri, shellcheck, actionlint, and the custom checkers under `scripts/`. One planned recipe does not exist yet: `check-semver`, which runs cargo-semver-checks and arrives with the first release. CI installs the latest version of each tool on its runners, and the Vulkan SDK of the pin, so a tool version in CI can differ from the one in the container, as Section 8.1 of `docs/design/tooling.md` states. Section 4.8 of the coding standard lists what each tool checks, and its Section 1.1 records the versions that were verified. Section 11 of `docs/design/tooling.md` lists every requirement on the image, which slang-rs-env defines.

## Development Workflow

A change starts with the section of the design document that covers it: `docs/design/architecture.md` for the architecture and the roadmap, and the crate's own design document for the crate. Work on the code of a milestone starts only once the design document that Section 15 of the architecture design requires for it exists. When that document does not exist yet, writing it is the first task of the milestone. Before it writes Rust, the agent reads the sections of the coding standard that Section 1.2 of that standard names. The agent makes the change in a worktree, as Hard Rule 11 requires, runs the checks in Validating Changes, and fixes whatever they report. The agent updates the documentation as Hard Rule 1 requires before it considers the change done.

`just hooks-install` installs the git hooks once per clone. At commit time, the hooks run the prose check on each changed Markdown file and each changed Rust file, the shell and commit message checks, the rustfmt check when a Rust file changes, typos on every commit, the TOML format check when a TOML file changes, `check-workflows` and `check-workflow-pins` when a file under `.github/` changes, and general checks for valid YAML and JSON, file endings, trailing whitespace, merge conflict markers, private keys, and large files. After a merge, they run `just worktree-prune`.

CI runs the workflow `.github/workflows/ci.yml` on every push to `main`, every push to a branch that matches `feature/**`, and every pull request. A new push to the same feature branch or pull request cancels the earlier run, and no run on `main` is cancelled, so every commit on `main` keeps its own result. No real run has shown the cancellation on a feature branch yet. The project opens no pull requests, so a push of a feature branch starts one run. Every job installs and checks the pinned Vulkan SDK before its own steps, and every check in every job is a `just` recipe, so a failure in CI has the same command to reproduce it in the container. Sections 8.1 and 8.3 of `docs/design/tooling.md` specify the jobs, their steps, and the artifact of the job `coverage`.

Commits are small, and each holds one group of logically related changes, because a later reader reads history one commit at a time. The subject line is in the imperative mood, such as `Add the safe compiler session`, and the body says why the change was made. `just merge BRANCH` squash-merges the branch onto `main` from the main checkout with the subject and body of every commit in the message, removes the worktree and the branch, pushes `main`, and deletes the branch on `origin` when its tip there is part of the squash. `just worktree-prune` removes branches that were squash-merged some other way. Agents never force-push.

The agent verifies a branch in CI before it merges the branch, with these steps, which Section 17.7 of `docs/design/tooling.md` specifies:

1. It pushes the branch with a plain push, which starts one run for the last commit of the branch.
2. It reads the run for that exact commit from the run itself, with these commands, in which `<branch>` and `<id>` are placeholders. The first prints the identifier, status, conclusion, and commit of the latest run of the branch, and the second prints each job of the run with its conclusion:

   ```sh
   gh run list --branch <branch> --limit 1 --json databaseId,status,conclusion,headSha
   gh run view <id> --json jobs
   ```

3. It runs `just merge <branch>` only when the run's `headSha` is the last commit of the branch, its conclusion is `success`, and every job succeeded.

The agent reads the run itself and never the exit status of a command whose output is piped into another program, because the exit status of a pipeline is the status of its last program, so it says nothing about the run, which is how the failing Windows job that Project Status records reached `main`. GitHub sometimes cancels a job before any step runs, with the annotation "The job was not acquired by Runner of type hosted even after multiple attempts". That is a problem of runner capacity, not a failure of the change, so the agent reruns the job and waits for the rerun (Section 17.4 of `docs/design/tooling.md`).

`just merge` builds the message of the squashed commit from every commit of the branch. A branch with several work-in-progress commits is therefore merged through a fresh branch. The agent creates a new branch from `main` that holds the identical tree of the verified branch as one commit with the final message, and runs `just merge` on the fresh branch. The fresh branch holds the tree that CI verified only when the verified branch contains the current `main`. No pushed history is rewritten. The branch that was pushed for CI then stays on `origin`, because `just merge` deletes only the merged branch there, and `just worktree-prune` removes its local branch and worktree.

The `review` skill runs the code-review workflow on a branch, writes its report under `.code-review/`, and reads this file as the project rules. The workflow does not find design documents under `docs/design/` by itself, so the agent passes the path of the design document as an explicit argument, such as `docs/design/<name>.md`. A reviewer checks judgement rules only, as Hard Rule 4 requires.

## Validating Changes

The agent runs the checks that match what changed, and `just check` before every merge. Section 4.2 of `docs/design/tooling.md` lists every recipe, and its Section 17.4 says what a failure of each one means and what to do.

To run every check:

```sh
just check
```

`just check` runs `check-prose`, `check-shell`, the self-test of each checker, `check-typos`, `check-toml`, `check-workflows`, `check-workflow-pins`, and `check-rust`, in that order, and Section 4.3 of `docs/design/tooling.md` lists every recipe in it. It needs network access, because `check-deny` fetches the advisory database on every run, the nightly toolchain, because `check-miri` runs Miri on it, and the pinned Vulkan SDK at `VULKAN_SDK`, because the build script of `slang-compiler-sys` links its Slang. The development container provides the nightly toolchain and the SDK.

To run only the Rust checks, which are `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs`, `check-fmt`, `check-unused-deps`, `check-deny`, `check-clippy`, `check-doc`, `check-doc-without-slang`, `check-doctest`, `test-gpu`, and `check-miri`, in that order:

```sh
just check-rust
```

To format every Rust file and every TOML file, which fixes what `check-fmt` and `check-toml` report, run this recipe. `fmt` and `check-fmt` pass every Rust file that git tracks or does not ignore to rustfmt, including `pin.rs`, `route.rs`, and `system.rs`, which `slang-compiler-sys` pulls in with `include!` and which `cargo fmt --all` alone does not read. Both fail with a message that names `git ls-files` when that command fails or lists no Rust file, so they run inside the git checkout:

```sh
just fmt
```

To run the lints after a change to Rust code or its API documentation:

```sh
just check-clippy
just check-doc
just check-doctest
```

To build the documentation as docs.rs does, with `DOCS_RS` set and `VULKAN_SDK` removed, after a change to the build script of `slang-compiler-sys` or to API documentation:

```sh
just check-doc-without-slang
```

These commands run the tests of each tier. `test` runs the tests that need no GPU adapter, `test-gpu` adds the tests that need any adapter, including lavapipe, and `test-hw-gpu` runs every test, including those that need a hardware GPU. No test of a GPU tier exists yet, so all three run the same tests. Each of them, and `check-doctest`, `check-bindings`, and `coverage`, puts `$VULKAN_SDK/lib` on the library path of Linux or macOS on its own command line, so the test binaries find the Slang shared library without any setting of the shell:

```sh
just test
just test-gpu
just test-hw-gpu
```

The tests of the binding generator in modules named `ffi` load libclang or start `rustfmt`, so they need both, which the development container provides. The regeneration test also needs `VULKAN_SDK`, and it fails, never skips, without any of the three. These tests compile for Linux only, so they do not run on Windows or macOS. The tests in the module `ffi` of `slang-compiler-sys`, which are the smoke test and the native-symbol test, call Slang, so they need `VULKAN_SDK` and the library path that the recipes supply, and they run on Linux, Windows, and macOS. The tests in the module `build_script::ffi` start `rustc` and write files, and they also run on all three. The tests in the module `ffi` of the test binary `compiler` of `slang-compiler` call Slang as well, so they also need `VULKAN_SDK` and the library path that the recipes supply, and they run on all three. One of them, the test of CA-14 of `docs/design/slang-compiler.md`, overwrites freed heap blocks only on Linux with the GNU C library, so it is weaker on Windows and macOS.

To run the tests of `slang-compiler-sys` on the static route after a change to the build script, to `route.rs` or `system.rs`, or to the pin, run this recipe with `SLANG_SOURCE_DIR` set to the top level of a clean git checkout of the pinned commit with its submodules, such as `/opt/slang` in the development container. It needs git, CMake, and Ninja, which the container provides, and no network access. It builds in `target/static` with `VULKAN_SDK`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` removed, so the tests pass only when Slang is linked statically. On 2026-11-10, its first build took 3 min 50 s on 16 cores and left 956 MB under `OUT_DIR`, and a rerun of the build script with nothing to rebuild took about 2 s. `just check` and CI do not run it, and it fails with a message that names `SLANG_SOURCE_DIR` when the variable is unset or empty:

```sh
SLANG_SOURCE_DIR=/opt/slang just test-static
```

To regenerate both generated files, `crates/slang-compiler-sys/src/bindings.rs` and `crates/slang-compiler-sys/tests/sys/symbols.rs`, after a change to the binding generator, its rule set, or the pin, run this recipe. It needs `VULKAN_SDK`, libclang, and rustfmt. An agent whose change alters the generator's output MUST commit both regenerated files in the same change, because the regeneration test fails on any difference between a checked-in file and a regeneration, so a change without the regenerated files fails `just check`. The files are never edited by hand (Hard Rule 7):

```sh
just generate-bindings
```

To run the regeneration test alone, run this recipe. It works on Linux only, because the test compiles for Linux only and the recipe fails when it selects no test:

```sh
just check-bindings
```

To print the pin of `slang-compiler-sys`, which is the pinned Vulkan SDK version, Slang build tag, and Slang commit, as `NAME=value` lines:

```sh
just pin
```

To check that the Vulkan SDK at `VULKAN_SDK` provides the Slang headers, library, and `slangc` of the build tag of the pin, which every CI job runs after it installs the SDK and which also works in the development container:

```sh
just check-vulkan-sdk
```

The recipe `install-vulkan-sdk` is for CI only. It refuses to run outside a GitHub Actions runner, and the agent does not run it.

To check the dependencies after a change to a manifest:

```sh
just check-deny
just check-unused-deps
```

To run the tests under Miri after a change to unsafe code, run this recipe. Miri cannot call native code or start a process, so the profile `default-miri` skips every test in a module named `ffi`, `gpu`, or `hw_gpu`, and Miri runs the pure tests of the binding generator, of the pin, and of the logic of the build script, including the static route against its fake environment. It also runs every unit test of `slang-compiler`, which test the owning interface pointer, the Slang call guard, the global session, the session, and the compiler options against the fake of Slang's interfaces in `crates/slang-compiler/src/fake.rs`, which frees each fake object when its count reaches zero, so that Miri detects a use after free. The build script still runs natively, so the recipe needs `VULKAN_SDK`:

```sh
just check-miri
```

To check the spelling of every file, and the formatting of every TOML file:

```sh
just check-typos
just check-toml
```

To check the prose of a Markdown file, or of the documentation comments of a Rust file, run this command. The Claude Code hook also runs it after every edit of a Markdown file or a Rust file:

```sh
just check-prose docs/rust-coding-standards.md
```

To check the comments, the headings of item documentation, the doctest attributes, and the feature documentation after a change to Rust code or its API documentation, run these recipes. Each also takes the paths of files to check, and `check-feature-docs` takes the paths of Cargo manifests:

```sh
just check-rust-comments
just check-doc-headings
just check-doctest-attributes
just check-feature-docs
```

To check a workflow after a change under `.github/`:

```sh
just check-workflows
just check-workflow-pins
```

To run the self-test of a checker after a change to the checker, to `scripts/rust_lexer.py`, or to its cases under `scripts/fixtures/`:

```sh
just check-prose-self-test
just check-rust-comments-self-test
just check-doc-headings-self-test
just check-doctest-attributes-self-test
just check-feature-docs-self-test
just check-workflow-pins-self-test
```

To check a shell script, or a recipe in the `justfile`:

```sh
just check-shell
```

To check `scripts/check-commit-message`:

```sh
just check-commit-message-self-test
```

Three more recipes are not part of `just check`, because they are meant for CI, and `test-static`, above, is not part of it either. CI runs all three in every run of the workflow. `check-features` compiles the workspace once for each combination of Cargo features. `check-msrv` checks the workspace with the toolchain that `rust-version` names, and its first run installs that toolchain with rustup, which needs network access. `coverage` writes a coverage report to `target/lcov.info` and prints no coverage summary, and the CI job `coverage` uploads that report as an artifact. An agent runs `check-features` after a change to the Cargo features and `check-msrv` after a change that may need a newer toolchain:

```sh
just check-features
just check-msrv
just coverage
```

## Documentation Maintenance

Hard Rule 1 is the rule an agent is most likely to skip, because the code works without it. After every change, and before it reports the change complete, the agent re-reads and corrects these documents:

1. The sections of the design documents that describe the changed crates, files, recipes, or behaviour, including the roadmap and the Assumptions to Confirm and Open Questions sections of `docs/design/architecture.md`.
2. `docs/rust-coding-standards.md`, if the change added or changed a rule, a lint, or a tool, including Appendix A.
3. `docs/rust-references.md`, if the change added a source or changed the citations of a rule.
4. `docs/writing-documentation.md`, if the change altered a documentation convention.
5. `README.md`, which is also the crates.io readme of every published member.
6. This file, if the change altered a convention, the repository layout, or the project status.
7. The change log of each document the change touched.

## Contribution Policy

The project is developed by its owner with coding agents, under the rules in this file. It has no contribution policy yet. The project opens no pull requests for now, and a pull request from a fork runs CI with a read-only token and no secrets. A future contributing policy may require disclosure of artificial intelligence (AI) usage in pull request descriptions.

## References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.
