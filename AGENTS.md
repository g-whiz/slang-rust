# Agent Instructions

Every coding agent reads this file before it works in this repository. `CLAUDE.md` imports it, so Claude Code loads the same instructions. The repository is meant to hold an idiomatic Rust library for the Slang shader language and runtime. It is at a very early stage and holds standards documents and repository tooling, but no working Rust code, as Project Status describes. Development happens inside the development container that the sibling repository slang-rs-env defines.

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
8. **Every action is a `just` recipe.** Every command, build step, and check is a recipe in the root `justfile`, which is one flat file with `[group]` attributes and no `just` modules, because modules hide their recipes from `just --list`. The agent invokes no script under `scripts/` directly when a recipe wraps it. Every recipe has a help comment, every shell script starts with `#!/usr/bin/env bash` and `set -euo pipefail`, and every shell script and every recipe that contains shell is `shellcheck`-clean. The Rust checks have no recipes yet, as Validating Changes explains.
9. **No attribution.** No output contains a generated-by line, a `Co-Authored-By` trailer, or a link to an agent session, because such a line says nothing about the change and a session link can expose a private session. This covers commits, pull requests, issues, and files. The `commit-msg` hook rejects such lines, and `just merge` strips them.
10. **No extra documentation.** The agent writes no document beyond `README.md`, `AGENTS.md`, `CLAUDE.md`, and the `docs/` tree, with design documents in `docs/design/`, because every extra document is one more place that can diverge from the code. `PLANS.md` is a legacy file that predates the decisions in Technology Decisions. No agent adds to it, and a design document will replace it.
11. **Work in a worktree.** Files change only in a git worktree under `.worktrees/`, never in the main checkout, because the main checkout is where `just merge` runs. Branches are named `feature/<short-description>`, lowercase and hyphenated, because the repository has no ticket keys. The agent commits without asking, merges with `just merge`, and MAY push `main` after the merge. The agent never force-pushes `main`.
12. **Canadian English and ASCII punctuation.** All prose, including API documentation, commit messages, and recipe help text, uses Canadian English spelling and ASCII punctuation, as Section 8 of the writing guide defines, because one spelling lets an agent find every occurrence of a word with one search.
13. **Consult the references before inventing a pattern.** When an agent is about to use a design pattern, or to make a design choice that the coding standard does not cover, it MUST look the topic up in `docs/rust-references.md`, read the sources starting at the highest tier, and extend the standard in the same change, because a reviewer cannot check code against a rule that does not exist yet. Section 1.1 of the reference list gives the lookup steps, and Section 19 of the coding standard gives the steps to add a rule.

## Project Status

As of 2026-11-10, the repository is at a very early stage. The agent updates this section, and its date, in the same change that alters what exists.

What exists and works:

- The `justfile` and its recipes, which `just --list` shows.
- The scripts under `scripts/`, which the recipes and hooks run.
- The git hooks in `.pre-commit-config.yaml`.
- The editor settings in `.editorconfig`.
- The Claude Code settings in `.claude/settings.json`, which run the prose checker after every edit of a Markdown file.
- `CLAUDE.md`, which imports this file.
- The writing guide, the coding standard, and the reference list under `docs/`.

What exists but does not work: `crates/slang-compiler-sys` is an abandoned sketch. It does not compile, with 36 errors in its generated bindings, and its build script clones and builds Slang from source, which SYS-5 and SYS-7 forbid. It will be rewritten. The agent MUST NOT build on it, because its design contradicts the decisions in Technology Decisions. The agent SHOULD NOT run `cargo check`, `cargo build`, `cargo clippy`, or any other command that compiles the workspace unless the task needs it, because each one runs that build script, which downloads and compiles Slang for several minutes. The `bindgen` and `cmake` dependencies and the `slang-version` key in `[workspace.metadata]` of the root `Cargo.toml` belong to the sketch.

What does not exist yet:

- Any crate of the design, and the binding generator.
- The workspace lint tables, `rust-version`, and the licence field in `Cargo.toml`.
- `clippy.toml`, `deny.toml`, and `rustfmt.toml`.
- Recipes for the Rust checks.
- Tests and examples.
- A CI workflow.
- A licence file and `README.md`.
- Any design document, and the directory `docs/design/`.

The agent MUST NOT assume that a rule of the coding standard is enforced by a tool, because none of the configuration of Section 4 of that standard exists yet, as its Section 4.1 states. Until it does, every rule there is a judgement rule. The agent also MUST NOT assume that a planned crate, recipe, file, or tool exists, or that `PLANS.md` is current, because an agent that builds on something absent produces code that cannot work.

## Repository Layout

- `AGENTS.md`: This file.
- `CLAUDE.md`: A one-line file that imports `AGENTS.md` for Claude Code.
- `PLANS.md`: The owner's original to-do sketch. It predates the decisions in Technology Decisions and is partly superseded, such as in its crate list and in how Slang is obtained. It stays until a design document replaces it.
- `Cargo.toml`: The workspace manifest, with resolver 3, edition 2024, and the sketch as its only member.
- `Cargo.lock`: The lock file of the workspace.
- `crates/slang-compiler-sys/`: The abandoned sketch that Project Status describes, with `Cargo.toml`, `build.rs`, `wrapper.h`, and `src/`.
- `docs/writing-documentation.md`: The normative writing guide.
- `docs/rust-coding-standards.md`: The normative Rust coding standard, with the specification of the lint and tool configuration.
- `docs/rust-references.md`: The reference list of external sources behind the coding standard, with a topic index.
- `justfile`: The root `justfile`, with the groups `checks` and `git`.
- `scripts/check-prose`, `scripts/prose-wordlist.txt`: The Markdown prose checker and its list of non-Canadian spellings.
- `scripts/check-shell`, `scripts/justfile-recipes`: The `shellcheck` run over every shell script and every recipe.
- `scripts/check-commit-message`: The commit message checker.
- `scripts/claude-hook-prose`: The Claude Code hook that runs the prose checker after an edit to a Markdown file.
- `scripts/merge-branch`: The squash merge that `just merge` runs.
- `scripts/worktree-prune`: The cleanup of squash-merged branches that `just worktree-prune` and the `post-merge` hook run.
- `.pre-commit-config.yaml`: The `pre-commit`, `commit-msg`, and `post-merge` hooks.
- `.claude/settings.json`: The Claude Code settings for this repository, with the allowed commands and the hook that runs the prose checker after every edit.
- `.editorconfig`: The character set, line endings, final newline, and indentation for each file type.
- `.gitignore`: The files that git ignores.

`target/`, `.worktrees/`, `.code-review/`, and `.idea/` are local and ignored by git.

These locations are planned and do not exist yet:

- `docs/design/`: The design documents.
- One directory under `crates/` for each planned crate.
- `README.md`, a licence file, `clippy.toml`, and `deny.toml`.
- The GitHub Actions workflows.

## Technology Decisions

These are decisions that await a design document. Once a design document covers a decision, the design document is authoritative. This section then becomes a summary, and the design document prevails wherever they disagree.

The planned crates are:

- `slang-compiler-sys`: The raw bindings only, generated and checked in, with a build script that links the native library and the manifest key `links = "slang-compiler"`.
- `slang-compiler`: Safe, idiomatic wrappers over `slang-compiler-sys`.
- `slango`: A high-level convenience library, with a feature set like SlangPy and design patterns like vulkano. Its first runtime backend is wgpu, and its design does not rule out a slang-rhi backend later.
- `slang-slint`: An extension to Slint that provides a view backed by a Slang render pipeline. It uses `slango` and supports wgpu only.
- The binding generator: A tool in this repository that parses the Slang headers with libclang.
- Examples of varying complexity, which double as a testbed.

Bindings for slang-rhi, `slang-rhi-sys` and `slang-rhi`, are deferred and not part of the first plan.

The binding generator produces the `#[repr(C)]` vtable structs, UUID constants, and thunks through which Rust calls the Slang interfaces in the style of the Component Object Model (COM). None of them is written by hand. The raw bindings are checked in, and a test regenerates them and fails if the result differs, so users of the crates need no libclang.

The Slang version is always the one that ships in the latest stable LunarG Vulkan software development kit (SDK). The first target is Vulkan SDK 1.4.363.1, whose `slangc` reports `2026.17-25-gc7954ebd1`. That is a commit past the tag `v2026.17`, so the pin is the SDK version together with the Slang commit, not a Slang tag. Slang is obtained in exactly one of two ways. By default, the build uses an installed Vulkan SDK and links Slang dynamically. As an opt-in, a static build builds Slang from source and links it statically. There is no prebuilt download and no other way.

Development uses the latest stable Rust toolchain. The workspace declares a minimum supported Rust version (MSRV) in `rust-version` and verifies it with a tool. The nightly toolchain serves only checkers such as Miri and never builds the crates. The crates use edition 2024 and Cargo resolver 3. The licence is GPL-3.0-or-later.

CI is planned on GitHub Actions for Linux with lavapipe on Vulkan, Windows with the Windows Advanced Rasterization Platform (WARP) on Direct3D 12, and macOS with hosted Metal. Tests that need a hardware graphics processing unit (GPU) are tagged and run locally only.

The development container provides this environment:

- Rust stable 1.99.1 with Clippy and rustfmt. No nightly toolchain and no Miri yet.
- cargo-nextest, cargo-deny, cargo-insta, cargo-expand, the bindgen command-line interface (CLI), `just`, pre-commit, shellcheck, and `gh`.
- The LunarG Vulkan SDK 1.4.363.1 at `$VULKAN_SDK`, which ships Slang. Its `slangc` reports `2026.17-25-gc7954ebd1`, the headers are under `$VULKAN_SDK/include/slang`, `libslang-compiler.so` is under `$VULKAN_SDK/lib`, and the pkg-config file is `slang-compiler.pc`. `$VULKAN_SDK/lib` is not on `LD_LIBRARY_PATH`.
- The Slang source tree at `/opt/slang`, at the same commit as the SDK's Slang. It is source only and not built.
- Two Vulkan devices: an NVIDIA RTX 4070 and lavapipe, a software implementation of Vulkan.
- The CUDA 13.5 toolkit.

These tools are not installed yet and are planned additions to the container image, which slang-rs-env defines: cargo-semver-checks, cargo-hack, cargo-machete, cargo-llvm-cov, typos, taplo, actionlint, and the nightly toolchain with Miri. Section 4.8 of the coding standard lists what each one will check.

## Development Workflow

A change starts with the section of the design document that covers it. Until a design document exists, it starts with Technology Decisions and the standards documents. Before it writes Rust, the agent reads the sections of the coding standard that Section 1.2 of that standard names. The agent makes the change in a worktree, as Hard Rule 11 requires, runs the checks in Validating Changes, and fixes whatever they report. The agent updates the documentation as Hard Rule 1 requires before it considers the change done.

`just hooks-install` installs the git hooks once per clone. At commit time, the hooks run the prose, shell, and commit message checks, and general checks for valid YAML and JSON, file endings, trailing whitespace, merge conflict markers, private keys, and large files. After a merge, they run `just worktree-prune`.

Commits are small, and each holds one group of logically related changes, because a later reader reads history one commit at a time. The subject line is in the imperative mood, such as `Add the safe compiler session`, and the body says why the change was made. `just merge BRANCH` squash-merges the branch onto `main` from the main checkout with the subject and body of every commit in the message, removes the worktree and the branch, and pushes `main`. `just worktree-prune` removes branches that were squash-merged some other way. The agent never force-pushes `main`.

The `review` skill runs the code-review workflow on a branch, writes its report under `.code-review/`, and reads this file as the project rules. The workflow does not find design documents under `docs/design/` by itself, so the agent passes the path of the design document as an explicit argument, such as `docs/design/<name>.md`. A reviewer checks judgement rules only, as Hard Rule 4 requires.

## Validating Changes

The agent runs the checks that match what changed, and `just check` before every merge.

To run every repository check, meaning prose, shell, and the commit message checker self-test:

```sh
just check
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

No recipe runs the Rust checks yet. Section 4.8 of the coding standard specifies them: formatting, Clippy, tests, doctests, the documentation build, and the dependency checks. The change that adds the workspace lint configuration adds recipes for these checks and updates this section. Until then, an agent that writes Rust runs the commands from Section 4.8 of the coding standard directly and reports their results.

## Documentation Maintenance

Hard Rule 1 is the rule an agent is most likely to skip, because the code works without it. After every change, and before it reports the change complete, the agent re-reads and corrects these documents:

1. The sections of the design document that describe the changed crates, files, recipes, or behaviour, once a design document exists.
2. `docs/rust-coding-standards.md`, if the change added or changed a rule, a lint, or a tool, including Appendix A.
3. `docs/rust-references.md`, if the change added a source or changed the citations of a rule.
4. `docs/writing-documentation.md`, if the change altered a documentation convention.
5. `README.md`, once it exists.
6. This file, if the change altered a convention, the repository layout, or the project status.
7. The change log of each document the change touched.

## Contribution Policy

The project is developed by its owner with coding agents, under the rules in this file. It has no contribution policy yet. A future contributing policy may require disclosure of artificial intelligence (AI) usage in pull request descriptions.

## References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.
