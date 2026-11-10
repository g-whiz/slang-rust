# Agent Instructions

Every coding agent reads this file before it works in this repository. `CLAUDE.md` imports it, so Claude Code loads the same instructions. The repository is meant to hold an idiomatic Rust library for the Slang shader language and runtime. It is at a very early stage and holds standards documents, design documents, repository tooling, the configuration of the Rust checks, and one placeholder crate, but no working Rust code, as Project Status describes. Development happens inside the development container that the sibling repository slang-rs-env defines.

Best Current Practice (BCP) 14 defines the key words in the next paragraph.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

## Hard Rules

1. **Documentation changes with the code.** The agent MUST update all documentation in the same change as the code it describes, because the next agent reads the documentation as true and builds on it. Documentation Maintenance lists what the agent re-reads.
2. **Documentation follows the writing guide.** The agent MUST write every document, and all application programming interface (API) documentation written with rustdoc, in accordance with `docs/writing-documentation.md`, because the agents and users that read a document depend on its conventions to interpret it. This includes the metadata and change log rules for design documents and the API documentation rules of Section 7 of the writing guide. This file is an instruction file under Section 3.4 of that guide, so it has no metadata table, abstract, or change log, and its unnumbered headings are cited by name.
3. **Rust code follows the coding standard.** The agent MUST write all Rust code in accordance with `docs/rust-coding-standards.md`, because a reviewer checks every change against it and rejects code that breaks it. The standard has two kinds of rules. An agent learns the rules that a lint or tool enforces from the failing check, by reading the lint's row in Section 4.5 and following Section 4.4. The judgement rules, listed in Appendix A, are what an agent reads before it writes Rust. A reviewer checks each change against them and cites the rule identifier in each finding. Section 1.2 of the standard says which sections to read for a change.
4. **Deterministic checks belong to tools.** The agent MUST leave every check that a deterministic tool can perform to that tool, and MUST NOT send anything that such a tool can find to an agent or a large language model (LLM) for review, because a reviewer misses findings that a tool reports every time and spends effort that only judgement can use. An agent that checks by hand something a tool could check adds the tool check in the same change, or proposes it in its report when it cannot add it. A review agent does not report a finding that a configured tool already reports. A new rule that a tool can check goes into the tool configuration, not into a list of judgement rules, as Section 19 of the coding standard and Section 3.5 of the writing guide require. A rule that a tool could check but that no configured tool checks yet stays a judgement rule until a tool covers it.
5. **Comments follow the writing guide.** Rust source contains only the comments that Section 7.2 of the writing guide lists, which are API documentation on public items and `# Safety` sections and `// SAFETY:` comments on unsafe code, and never a `TODO` or a `FIXME`. In the `justfile` and the scripts, the only comments are text addressed to a human, such as the help comment of a recipe or the usage text of a script.
6. **Lint exceptions use `#[expect]`.** The agent MUST record every lint exception as `#[expect(lint, reason = "...")]` on the narrowest scope that needs it and MUST NOT use `#[allow]`, because an `#[allow]` hides the lint silently for as long as it stays, while a stale `#[expect]` fails the check. No agent weakens the lint or tool configuration to make a check pass. Rules LINT-1 and LINT-2 in Section 4.4 of the coding standard define the form of an exception and when the configuration may change.
7. **Soundness and security by default.** When two approaches are otherwise comparable, the agent MUST choose the sounder and more secure one and say so in its report, because unsound or insecure code that works is rarely revisited. Unsafe code appears only in `slang-compiler-sys` and `slang-compiler`, and any other crate that needs it records the reason in its design document first, as Section 4.4 of the coding standard states. Generated bindings, vtables, universally unique identifier (UUID) constants, and thunks are never edited by hand, and the agent changes them only by running the binding generator again (FFI-1, COM-9). No file in the repository contains a secret. A build script downloads nothing, installs nothing, and writes only inside `OUT_DIR` (SYS-5). A continuous integration (CI) workflow verifies the SHA-256 checksum of every download whose upstream publishes one.
8. **Every action is a `just` recipe.** Every command, build step, and check is a recipe in the root `justfile`, which is one flat file with `[group]` attributes and no `just` modules, because modules hide their recipes from `just --list`. The agent invokes no script under `scripts/` directly when a recipe wraps it. Every recipe has a help comment, every shell script starts with `#!/usr/bin/env bash` and `set -euo pipefail`, and every shell script and every recipe that contains shell is `shellcheck`-clean. Section 4.2 of `docs/design/tooling.md` lists every recipe, and Validating Changes names the recipes to run for each kind of change.
9. **No attribution.** No output contains a generated-by line, a `Co-Authored-By` trailer, or a link to an agent session, because such a line says nothing about the change and a session link can expose a private session. This covers commits, pull requests, issues, and files. The `commit-msg` hook rejects such lines, and `just merge` strips them.
10. **No extra documentation.** The agent writes no document beyond `README.md`, `AGENTS.md`, `CLAUDE.md`, and the `docs/` tree, with design documents in `docs/design/`, because every extra document is one more place that can diverge from the code.
11. **Work in a worktree.** Files change only in a git worktree under `.worktrees/`, never in the main checkout, because the main checkout is where `just merge` runs. Branches are named `feature/<short-description>`, lowercase and hyphenated, because the repository has no ticket keys. The agent commits without asking, merges with `just merge`, and MAY push `main` after the merge. The agent never force-pushes `main`.
12. **Canadian English and ASCII punctuation.** All prose, including API documentation, commit messages, and recipe help text, uses Canadian English spelling and ASCII punctuation, as Section 8 of the writing guide defines, because one spelling lets an agent find every occurrence of a word with one search.
13. **Consult the references before inventing a pattern.** When an agent is about to use a design pattern, or to make a design choice that the coding standard does not cover, it MUST look the topic up in `docs/rust-references.md`, read the sources starting at the highest tier, and extend the standard in the same change, because a reviewer cannot check code against a rule that does not exist yet. Section 1.1 of the reference list gives the lookup steps, and Section 19 of the coding standard gives the steps to add a rule.

## Project Status

As of 2026-11-10, the repository is at a very early stage. Milestone M0 is under way: steps 1 to 4 of Section 13 of `docs/design/tooling.md` are done, and steps 5 and 6 remain. The agent updates this section, and its date, in the same change that alters what exists.

What exists and works:

- The `justfile` and its recipes, which `just --list` shows, including the recipes that run the Rust checks.
- The scripts under `scripts/`, which the recipes and hooks run.
- The git hooks in `.pre-commit-config.yaml`.
- The editor settings in `.editorconfig`.
- The Claude Code settings in `.claude/settings.json`, which run the prose checker after every edit of a Markdown file.
- `CLAUDE.md`, which imports this file.
- The workspace manifest `Cargo.toml`, with the `[workspace.package]` fields that every member inherits and the workspace lint tables of Section 4.2 of the coding standard.
- The tool configuration: `clippy.toml`, `deny.toml`, `.config/nextest.toml`, `taplo.toml`, and `typos.toml`.
- `crates/slang-compiler-sys`, a placeholder crate with publication metadata and crate-level documentation, and with no API, no build script, and no dependencies. It exists so that every check has a package to run on, and milestone M1 replaces its contents with the raw bindings.
- `LICENSE` and `README.md`.
- The writing guide, the coding standard, and the reference list under `docs/`.
- The architecture design, `docs/design/architecture.md`, which specifies the planned crates, how Slang is obtained and pinned, and the roadmap of milestones M0 to M7.
- The tooling design, `docs/design/tooling.md`, which specifies the toolchain configuration, the recipes, the hooks, the CI workflow, and the requirements on the container image. Milestone M0 implements it, and its Section 1.6 says which parts exist.

`just check` passes. The test recipes pass with a warning that no test ran, because no test exists yet.

What does not exist yet:

- Any crate of the design with an API, and the binding generator.
- The custom checkers of Section 7 of `docs/design/tooling.md`, their recipes, and the extension of `check-prose` to API documentation.
- A CI workflow, and the recipe `check-workflows`.
- Tests and examples.
- The recipes `check-bindings`, `generate-bindings`, and `check-doc-without-slang`, which arrive in M1, and `check-semver`, which arrives with the first release in M7.
- The design document of each crate, which is written as the first task of the milestone that delivers the crate.

The workspace lint tables, `clippy.toml`, and the tools that `just check` runs enforce their rules, so an agent learns those rules from a failing check, as Hard Rule 3 states. The rules that the custom checkers of Section 7 of `docs/design/tooling.md` will check stay judgement rules until those checkers exist, and so does every other rule that no configured tool checks. `check-features` and `check-msrv` run outside `just check`, so until a CI workflow runs them, the rules they check hold only when an agent runs them. The agent MUST NOT assume that a planned crate, recipe, file, or tool exists, because an agent that builds on something absent produces code that cannot work.

## Repository Layout

- `AGENTS.md`: This file.
- `CLAUDE.md`: A one-line file that imports `AGENTS.md` for Claude Code.
- `Cargo.toml`: The workspace manifest, with resolver 3, the `[workspace.package]` fields that every member inherits, such as edition 2024, `rust-version`, and the licence, and the workspace lint tables. Its only member is `crates/slang-compiler-sys`.
- `Cargo.lock`: The lock file of the workspace.
- `LICENSE`: The text of version 3 of the GNU General Public License.
- `README.md`: The README, which is also the readme of every member through the inherited `readme` field.
- `crates/slang-compiler-sys/`: The placeholder crate that Project Status describes, with `Cargo.toml` and `src/lib.rs`.
- `clippy.toml`: The Clippy configuration of Section 4.3 of the coding standard.
- `deny.toml`: The cargo-deny configuration of Section 9 of `docs/design/tooling.md`.
- `.config/nextest.toml`: The nextest profiles `default`, `gpu`, and `hw-gpu`, which implement the test tiers as Section 5 of `docs/design/tooling.md` defines.
- `taplo.toml`: The taplo configuration, with four-space indentation and the exclusion of `target/` and `.worktrees/`.
- `typos.toml`: The typos configuration, which ignores two correct patterns that typos would otherwise report.
- `docs/writing-documentation.md`: The normative writing guide.
- `docs/rust-coding-standards.md`: The normative Rust coding standard, with the specification of the lint and tool configuration.
- `docs/rust-references.md`: The reference list of external sources behind the coding standard, with a topic index.
- `docs/design/architecture.md`: The architecture design, which is authoritative for the crates, their layering, how Slang is obtained and pinned, the examples, the test tiers, platform support, publishing, and the roadmap.
- `docs/design/tooling.md`: The tooling design, which is authoritative for the toolchain configuration, the recipes, the hooks, the CI workflow, the dependency policy, and the requirements on the container image. Milestone M0 implements it, and its Section 1.6 says which parts exist.
- `justfile`: The root `justfile`, with the groups `checks`, `git`, and `rust`.
- `scripts/check-prose`, `scripts/prose-wordlist.txt`: The Markdown prose checker and its list of non-Canadian spellings.
- `scripts/check-shell`, `scripts/justfile-recipes`: The `shellcheck` run over every shell script and every recipe.
- `scripts/check-commit-message`: The commit message checker.
- `scripts/claude-hook-prose`: The Claude Code hook that runs the prose checker after an edit to a Markdown file.
- `scripts/merge-branch`: The squash merge that `just merge` runs.
- `scripts/worktree-prune`: The cleanup of squash-merged branches that `just worktree-prune` and the `post-merge` hook run.
- `.pre-commit-config.yaml`: The `pre-commit`, `commit-msg`, and `post-merge` hooks.
- `.claude/settings.json`: The Claude Code settings for this repository, with the allowed commands, which include `just check` and most of the recipes that it runs, and the hook that runs the prose checker after every edit.
- `.editorconfig`: The character set, line endings, final newline, and indentation for each file type.
- `.gitignore`: The files that git ignores.

`target/`, `.worktrees/`, `.code-review/`, and `.idea/` are local and ignored by git.

These locations are planned and do not exist yet:

- `docs/design/<crate-name>.md`: One design document for each crate, written as the first task of the crate's milestone.
- One directory for each planned crate other than `slang-compiler-sys`, for the binding generator, and for the examples, as Section 3.1 of `docs/design/architecture.md` lists.
- The scripts of the custom checkers under `scripts/`, which Section 7 of `docs/design/tooling.md` names.
- The GitHub Actions workflow `.github/workflows/ci.yml`.

## Technology Decisions

`docs/design/architecture.md` is authoritative for the architecture: the crates and their layering, how Slang is obtained and pinned, the responsibilities of each crate, the examples, the test tiers, platform support, publishing, and the roadmap. This section summarizes it, together with the toolchain and environment decisions, and the architecture design prevails wherever the two disagree. `docs/design/tooling.md` is the design for the toolchain configuration, the recipes, the hooks, the CI workflow, and the requirements on the container image, and it prevails over this section on those topics. Milestone M0 implements it, and its Section 1.6 says which parts exist.

The planned crates are:

- `slang-compiler-sys`: The raw bindings only, generated and checked in, with a build script that links the native library and the manifest key `links = "slang-compiler"`. Until M1, its directory holds the placeholder that Project Status describes.
- `slang-compiler`: Safe, idiomatic wrappers over `slang-compiler-sys`, with no graphics processing unit (GPU) dependency.
- `slango`: A high-level convenience library, with a feature set like SlangPy and design patterns like vulkano. Its first runtime backend is wgpu, and its design does not rule out a slang-rhi backend later. It first offers a runtime call API driven by reflection, and its first milestone covers compute kernels compiled to SPIR-V only. Rendering and a shader target configurable per backend follow.
- `slango-macros`: A later build-time macro that generates typed Rust bindings to Slang code on the same core as the runtime API.
- `slang-slint`: An extension to Slint that provides a view backed by a Slang render pipeline. It uses `slango` and supports wgpu only, permanently.
- The binding generator: A tool in this repository that parses the Slang headers with libclang. It is not published.
- The examples, which also serve as the integration testbed and are not published: a command-line program that compiles and reflects Slang source, a headless compute kernel with a broadcasting case and a differentiable function, a windowed renderer with winit and shader hot reload, a Slint application with a `slang-slint` view, and later a path-tracing demo.

Bindings for slang-rhi, `slang-rhi-sys` and `slang-rhi`, are deferred and not part of the first plan.

The binding generator produces the `#[repr(C)]` vtable structs, UUID constants, and thunks through which Rust calls the Slang interfaces in the style of the Component Object Model (COM). None of them is written by hand. The raw bindings are checked in, and a test regenerates them and fails if the result differs, so users of the crates need no libclang.

The Slang version is always the one that ships in the latest stable LunarG Vulkan software development kit (SDK). The first target is Vulkan SDK 1.4.363.1, whose `slangc` reports `2026.17-25-gc7954ebd1`. That is a commit past the tag `v2026.17`, so the pin is the SDK version together with the Slang commit, not a Slang tag. Slang is obtained in exactly one of two ways. By default, the build uses an installed Vulkan SDK and links Slang dynamically. As an opt-in, a static build builds Slang from source and links it statically. There is no prebuilt download and no other way.

Development uses the latest stable Rust toolchain. The workspace declares the minimum supported Rust version (MSRV) 1.99 in `rust-version`, and `check-msrv` verifies it with cargo-hack. The nightly toolchain serves only checkers such as Miri and never builds the crates. The crates use edition 2024 and Cargo resolver 3. The licence is GPL-3.0-or-later, and `LICENSE` holds its text.

CI is planned on GitHub Actions for Linux with lavapipe on Vulkan, Windows with the Windows Advanced Rasterization Platform (WARP) on Direct3D 12, and macOS with hosted Metal. Tests that need a GPU adapter sit in a module named `gpu`, which runs on any adapter including lavapipe and WARP, or in a module named `hw_gpu`, which needs a hardware GPU and runs locally only. A test that finds no adapter fails and never skips silently.

The published crates are planned for crates.io, including docs.rs builds that work without Slang installed and checks for breaking changes. Nothing is published without the owner's explicit go-ahead.

The development container provides this environment:

- Rust stable 1.99.1, the default toolchain, with Clippy, rustfmt, rust-src, and llvm-tools.
- A nightly toolchain with Miri and rust-src, used only for checkers.
- cargo-nextest, cargo-deny, cargo-insta, cargo-expand, cargo-semver-checks, cargo-hack, cargo-machete, cargo-llvm-cov, the bindgen command-line interface (CLI), typos, taplo, actionlint, `just`, pre-commit, shellcheck, `gh`, CMake, and Ninja.
- GitHub's Secure Shell (SSH) host keys in the known hosts file.
- The LunarG Vulkan SDK 1.4.363.1 at `$VULKAN_SDK`, which ships Slang. Its `slangc` reports `2026.17-25-gc7954ebd1`, the headers are under `$VULKAN_SDK/include/slang`, `libslang-compiler.so` is under `$VULKAN_SDK/lib`, and the pkg-config file is `slang-compiler.pc`. A login shell has `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH`. A process that no login shell started does not, until the next rebuild of the container image applies the fix that slang-rs-env already contains.
- The Slang source tree at `/opt/slang`, at the same commit as the SDK's Slang. It is source only and not built.
- Two Vulkan devices: an NVIDIA RTX 4070 and lavapipe, a software implementation of Vulkan.
- The CUDA 13.5 toolkit.

Recipes run Clippy, rustfmt, cargo-nextest, cargo-deny, cargo-hack, cargo-machete, cargo-llvm-cov, typos, taplo, Miri, and shellcheck. Two planned recipes do not exist yet: `check-workflows`, which runs actionlint and arrives with the first workflow, and `check-semver`, which runs cargo-semver-checks and arrives with the first release. Section 4.8 of the coding standard lists what each tool checks, and its Section 1.1 records the versions that were verified. Section 11 of `docs/design/tooling.md` lists every requirement on the image, which slang-rs-env defines.

## Development Workflow

A change starts with the section of the design document that covers it: `docs/design/architecture.md` for the architecture and the roadmap, and the crate's own design document for the crate. Work on the code of a milestone starts only once the design document that Section 15 of the architecture design requires for it exists. When that document does not exist yet, writing it is the first task of the milestone. Before it writes Rust, the agent reads the sections of the coding standard that Section 1.2 of that standard names. The agent makes the change in a worktree, as Hard Rule 11 requires, runs the checks in Validating Changes, and fixes whatever they report. The agent updates the documentation as Hard Rule 1 requires before it considers the change done.

`just hooks-install` installs the git hooks once per clone. At commit time, the hooks run the prose, shell, and commit message checks, the rustfmt check when a Rust file changes, typos on every commit, the TOML format check when a TOML file changes, and general checks for valid YAML and JSON, file endings, trailing whitespace, merge conflict markers, private keys, and large files. After a merge, they run `just worktree-prune`.

Commits are small, and each holds one group of logically related changes, because a later reader reads history one commit at a time. The subject line is in the imperative mood, such as `Add the safe compiler session`, and the body says why the change was made. `just merge BRANCH` squash-merges the branch onto `main` from the main checkout with the subject and body of every commit in the message, removes the worktree and the branch, and pushes `main`. `just worktree-prune` removes branches that were squash-merged some other way. The agent never force-pushes `main`.

The `review` skill runs the code-review workflow on a branch, writes its report under `.code-review/`, and reads this file as the project rules. The workflow does not find design documents under `docs/design/` by itself, so the agent passes the path of the design document as an explicit argument, such as `docs/design/<name>.md`. A reviewer checks judgement rules only, as Hard Rule 4 requires.

## Validating Changes

The agent runs the checks that match what changed, and `just check` before every merge. Section 4.2 of `docs/design/tooling.md` lists every recipe, and its Section 17.4 says what a failure of each one means and what to do.

To run every check:

```sh
just check
```

`just check` runs `check-prose`, `check-shell`, `check-commit-message-self-test`, `check-typos`, `check-toml`, and `check-rust`, in that order. It needs network access, because `check-deny` fetches the advisory database on every run, and it needs the nightly toolchain, because `check-miri` runs Miri on it. The development container provides the nightly toolchain.

To run only the Rust checks, which are `check-fmt`, `check-unused-deps`, `check-deny`, `check-clippy`, `check-doc`, `check-doctest`, `test-gpu`, and `check-miri`, in that order:

```sh
just check-rust
```

To format every Rust file and every TOML file, which fixes what `check-fmt` and `check-toml` report:

```sh
just fmt
```

To run the lints after a change to Rust code or its API documentation:

```sh
just check-clippy
just check-doc
just check-doctest
```

These commands run the tests of each tier. `test` runs the tests that need no GPU adapter, `test-gpu` adds the tests that need any adapter, including lavapipe, and `test-hw-gpu` runs every test, including those that need a hardware GPU. While no test exists, each passes with a warning that no test ran:

```sh
just test
just test-gpu
just test-hw-gpu
```

To check the dependencies after a change to a manifest:

```sh
just check-deny
just check-unused-deps
```

To run the tests under Miri after a change to unsafe code:

```sh
just check-miri
```

To check the spelling of every file, and the formatting of every TOML file:

```sh
just check-typos
just check-toml
```

To check a Markdown file, run this command. The Claude Code hook also runs it after every edit:

```sh
just check-prose docs/rust-coding-standards.md
```

To check a shell script, or a recipe in the `justfile`:

```sh
just check-shell
```

To check `scripts/check-commit-message`:

```sh
just check-commit-message-self-test
```

Three recipes are not part of `just check`, because the CI workflow is meant to run them, and that workflow does not exist yet. `check-features` compiles the workspace once for each combination of Cargo features. `check-msrv` checks the workspace with the toolchain that `rust-version` names, and its first run installs that toolchain with rustup, which needs network access. `coverage` writes a coverage report to `target/lcov.info`, and it fails with "no coverage data found" while no crate contains a function, which is the case today. An agent runs `check-features` after a change to the Cargo features and `check-msrv` after a change that may need a newer toolchain:

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

The project is developed by its owner with coding agents, under the rules in this file. It has no contribution policy yet. A future contributing policy may require disclosure of artificial intelligence (AI) usage in pull request descriptions.

## References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.
