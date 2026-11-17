# Agent Instructions

Every coding agent reads this file before it works in this repository. `CLAUDE.md` imports it, so Claude Code loads the same instructions.

The repository is meant to hold an idiomatic Rust library for the Slang shader language and runtime. It is at a very early stage. It holds these parts:

- Standards documents, design documents, repository tooling, and the configuration of the Rust checks.
- The binding generator, the publishable crate `slang-bindgen`.
- The crate `slang-compiler-sys`, whose build script generates the raw bindings from the headers of the Slang that it links and records that Slang in the constant `BUILD_INFO`. By default it links the Slang of an installed Vulkan SDK dynamically, and with its Cargo feature `vendored` it builds Slang from source and links it statically.
- The crate `slang-src`, which clones the recorded Slang commit for a vendored build, and which `slang-compiler-sys` depends on with the feature `vendored` and `slang-glslang-sys` always depends on.
- The crate `slang-glslang-sys`, which builds slang-glslang from the same tree for a vendored build and holds the in-process loader through which a statically linked Slang optimizes SPIR-V.
- The compile and reflection API of the safe crate `slang-compiler`, which can compile a shader and reflect its parameters, as Project Status describes. With its feature `vendored`, it installs the loader of `slang-glslang-sys` on every global session that it creates.
- The crate `slango`, which runs compute kernels and draws with render pipelines.
- The crate `slang-slint`, which shows what `slango` draws in a Slint user interface.
- The procedural macro crate `slango-macros` with its generator crate `slango-codegen`, which generate typed bindings to the compute kernels of a Slang file.
- Five examples.

Development happens inside the development container that the sibling repository slang-rs-env defines.

Best Current Practice (BCP) 14 defines the key words in the next paragraph.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

## Hard Rules

1. **Documentation changes with the code, once.** In the same change as the code, the agent MUST update the design document of each crate or tool that the change alters, and every statement in another document that the change would otherwise make false. The next agent reads the documentation as true and builds on it, so a false statement misleads it.

   `AGENTS.md`, the architecture design, the tooling design, and `README.md` change only for such a statement, or when the change alters a rule, the repository layout, or what a user can do.

   The agent MUST NOT mirror a fact into several documents, because every copy can diverge. It states the fact once and cites that place. Documentation Maintenance lists what the agent re-reads.
2. **Documentation follows the writing guide.** The agent MUST write every document, and all application programming interface (API) documentation written with rustdoc, in accordance with `docs/writing-documentation.md`, because the agents and users that read a document depend on its conventions to interpret it. This includes the metadata rules for design documents and the API documentation rules of Section 7 of the writing guide. This file is an instruction file under Section 3.4 of that guide, so it has no metadata table or abstract, and its unnumbered headings are cited by name.
3. **Rust code follows the coding standard.** The agent MUST write all Rust code in accordance with `docs/rust-coding-standards.md`, because a reviewer checks every change against it and rejects code that breaks it. The standard has two kinds of rules.

   An agent learns the rules that a lint or tool enforces from the failing check, by reading the lint's row in Section 4.5 and following Section 4.4. The judgement rules, listed in Appendix A, are what an agent reads before it writes Rust.

   A reviewer checks each change against them and cites the rule identifier in each finding. Section 1.2 of the standard says which sections to read for a change.
4. **Deterministic checks belong to tools.** The agent MUST leave every check that a deterministic tool can perform to that tool, and MUST NOT send anything that such a tool can find to an agent or a large language model (LLM) for review. A reviewer misses findings that a tool reports every time and spends effort that only judgement can use.

   An agent that checks by hand something a tool could check adds the tool check in the same change, or proposes it in its report when it cannot add it. A review agent does not report a finding that a configured tool already reports.

   A new rule that a tool can check goes into the tool configuration, not into a list of judgement rules, as Section 19 of the coding standard and Section 3.5 of the writing guide require. A rule that a tool could check but that no configured tool checks yet stays a judgement rule until a tool covers it.
5. **Comments follow the writing guide.** Rust source contains only the comments that Section 7.2 of the writing guide lists, which are API documentation on public items and `# Safety` sections and `// SAFETY:` comments on unsafe code, and never a `TODO` or a `FIXME`. In the `justfile` and the scripts, the only comments are text addressed to a human, such as the help comment of a recipe or the usage text of a script. No comment cites a project document, as Hard Rule 15 states.
6. **Lint exceptions use `#[expect]`.** The agent MUST record every lint exception as `#[expect(lint, reason = "...")]` on the narrowest scope that needs it and MUST NOT use `#[allow]`, because an `#[allow]` hides the lint silently for as long as it stays, while a stale `#[expect]` fails the check.

   No agent weakens the lint or tool configuration to make a check pass. Rules LINT-1 and LINT-2 in Section 4.4 of the coding standard define the form of an exception and when the configuration may change. The `reason` states the reason in plain words and names no rule identifier, as Hard Rule 15 requires.
7. **Soundness and security by default.** When two approaches are otherwise comparable, the agent MUST choose the sounder and more secure one and say so in its report, because unsound or insecure code that works is rarely revisited.

   Unsafe code appears only in `slang-compiler-sys`, `slang-compiler`, and `slang-glslang-sys`, which declares the one foreign function that returns its loader (SGL-13 of `docs/design/slang-glslang-sys.md`). Any other crate that needs it records the reason in its design document first, as Section 4.4 of the coding standard states.

   Generated bindings, vtables, universally unique identifier (UUID) constants, thunks, and the generated list of C functions are never written by hand and never checked in. The build script of `slang-compiler-sys` generates them into `OUT_DIR` in every build, so the agent changes them only by changing the binding generator or its rule set (FFI-1, COM-9, ARCH-33 of `docs/design/architecture.md`).

   No file in the repository contains a secret. A build script installs nothing, writes only inside `OUT_DIR` or the shared folder in the Cargo target directory of the build, and reaches no network, except the build script of `slang-src`, which clones the recorded commit of the Slang tree by its full hash (SYS-5). A continuous integration (CI) workflow verifies the SHA-256 checksum of every download whose upstream publishes one.
8. **Every action is a `just` recipe.** Every command, build step, and check is a recipe in the root `justfile`, which is one flat file with `[group]` attributes and no `just` modules, because modules hide their recipes from `just --list`. The agent invokes no script under `scripts/` directly when a recipe wraps it.

   Every recipe has a help comment, and every shell script starts with `#!/usr/bin/env bash` and `set -euo pipefail`. Every shell script and every recipe that contains shell is `shellcheck`-clean. Section 3.2 of `docs/design/tooling.md` lists every recipe, and Validating Changes names the recipes to run for each kind of change.
9. **No attribution.** No output contains a generated-by line, a `Co-Authored-By` trailer, or a link to an agent session, because such a line says nothing about the change and a session link can expose a private session. This covers commits, pull requests, issues, and files. The `commit-msg` hook rejects such lines, and `just merge` strips them.
10. **No extra documentation.** The agent writes no document beyond `README.md`, `AGENTS.md`, `CLAUDE.md`, and the `docs/` tree, with design documents in `docs/design/`, because every extra document is one more place that can diverge from the code.
11. **Work in a worktree.** Files change only in a git worktree under `.worktrees/`, never in the main checkout, because the main checkout is where `just merge` runs. Branches are named `feature/<short-description>`, lowercase and hyphenated, because the repository has no ticket keys.

   The agent commits without asking and pushes its branch to run CI, because the project opens no pull requests and a push to a branch that matches `feature/**` is what runs CI on a branch.

   The agent MUST run `just merge` only after the CI run for the last commit of the branch concluded `success` with every job succeeded, because a merge without that result can put a failing commit on `main`. A fresh branch that holds the tree of a verified branch counts as verified, as Development Workflow describes.

   The agent MAY push `main` after the merge. Agents open no pull requests and never force-push.
12. **Canadian English and ASCII punctuation.** All prose, including API documentation, commit messages, and recipe help text, uses Canadian English spelling and ASCII punctuation, as Section 8 of the writing guide defines. One spelling lets an agent find every occurrence of a word with one search.
13. **Consult the references before inventing a pattern.** When an agent is about to use a design pattern, or to make a design choice that the coding standard does not cover, it MUST look the topic up in `docs/rust-references.md` and read the sources starting at the highest tier. It MUST then extend the standard in the same change.

   A reviewer cannot check code against a rule that does not exist yet. Section 1.1 of the reference list gives the lookup steps, and Section 19 of the coding standard gives the steps to add a rule.
14. **Defects of Slang are not guarded against.** Where Slang has a genuine defect, the agent MUST NOT guard against it, screen input for it, or write a new test for it, and MUST implement the crates as if the defect did not exist.

   The crates provide a safe interface on the explicit assumption that Slang behaves as documented. Shader source, type names, and other text passed to Slang are trusted input. The crates are sound for well-formed use, and a crash or an abort that a defect causes on pathological input is accepted and documented, not prevented.

   The checks that a safe interface needs stay whatever Slang's defects are: the bounds of indexes, null returns, the conversion of strings, objects of another session, and reentrant calls, which Slang documents as unsupported.

   The agent MUST record each defect once, in the list of known upstream bugs in Section 3.12 of `docs/design/slang-compiler.md`, with the smallest known input, the call, the observed outcome, and the source location if already known, so that a later investigation can start from it. The agent MUST NOT investigate a defect further unless it blocks the work.

   The agent MUST add an entry for each defect that should be reported to the to-do list of upstream reports, `docs/upstream-reports.md`, in the same change that records the defect, because that list is the one place where the owner finds the reports to make. Section 2 of that list gives the rules for an entry.

   The agent MAY draft a bug report in that list as low-priority work and MUST NOT post it without the owner's explicit go-ahead in the current session, because a report speaks for the project in a public repository.
15. **Code cites no project document.** The agent MUST NOT cite, in a code file or a comment, a project document, which is a Markdown file under `docs/`, a section of one, or a rule identifier that a project document defines, such as `SLC-32` or `FFI-7`. A reader of the code or of a published crate sees it without the repository's documents, and a citation breaks when a document changes.

   The rule covers API documentation, ordinary comments, the `reason` strings of lint exceptions, error and panic messages, recipe help comments, script usage text, and comments in configuration files. Code also MUST NOT hold project history wording, such as "this version", a milestone, or a step.

   Where a citation would have carried a reason, the code MUST state the reason itself in plain words. The documents cite the code, never the reverse.

   The checker `check-doc-references` reports the path or file name of a project document, a section cited by number, and a rule identifier that a project document defines. `just check` and the `pre-commit` hook run it. No tool checks history wording, so that part stays a judgement rule.

## Project Status

As of 2026-11-17, the repository is at a very early stage. The agent updates this section, and its date, in the same change that alters what exists. Section 15 of `docs/design/architecture.md` defines each milestone and its exit criteria.

- M0, tooling and skeleton, is complete.
- M1, the binding generator and `slang-compiler-sys`, is complete. It delivered the pin file, the binding generator, the checked-in bindings, the build script with its dynamic and static routes, the installation of the pinned Vulkan SDK in every CI job, and the CI job `coverage`. What stays open after M1 is in the rows of Sections 15 and 16 of `docs/design/slang-compiler-sys.md`.
- M2, the safe crate `slang-compiler` and the compile-and-reflect example, is complete on 2026-11-11. A user can compile a shader, reflect its parameters with their types, layouts, and bindings, and run the example `compile-and-reflect`. The custom file system is deferred with no milestone assigned, as What does not exist yet states.
- M3, the crate `slango` and the compute kernel example, is complete on 2026-11-11. A user can load a Slang module, run a compute kernel with its parameters set by name, read its results, and run the example `compute-kernel`.
- M4, the rendering of `slango` and the windowed renderer example, is complete on 2026-11-11. Its design is also in `docs/design/slango.md`. A user can also draw with a vertex and a fragment entry point into a texture or a window, and run the example `windowed-renderer`.
- M5, `slang-slint` and the Slint example, is complete on 2026-11-11. A Slint program can make Slint create a wgpu device that suits `slango`, turn that device into a `slango::Device`, and show what a `slango` render pipeline draws as a Slint image. A user can run the Slint example `shader-playground`.
- M6, `slango-macros` and its generator `slango-codegen`, is complete on 2026-11-13. `slango` checks the element type of a bound buffer (Section 9.3 of `docs/design/slango.md`). A crate can call `slango_macros::shader!` on a Slang file to get a typed kernel for each compute entry point and a Rust struct for each Slang structure that the kernels take, and a user can run the example `typed-kernel`.
- M7, the rework of the native crates, is complete on 2026-11-15. Every build generates the Slang bindings with libclang and records the Slang that it links in `BUILD_INFO`. A program can enable the Cargo feature `vendored` on `slang-compiler`, `slango`, or `slang-slint`, and on `slango-macros` when it calls the macro, to build Slang and its SPIR-V optimizer from source and link them statically without a Vulkan SDK, as the section "Building Slang From Source" of `README.md` describes.

On 2026-11-15, the owner replaced the roadmap after M6 with milestones M7 to M18, which Section 15 of `docs/design/architecture.md` defines. The design of M7 is in `docs/design/slang-compiler-sys.md`, `docs/design/slang-src.md`, `docs/design/slang-glslang-sys.md`, and the documents that they changed. Five steps delivered M7, all on 2026-11-15, and What exists and works describes what they delivered:

- The first added the crate `slang-src`, its recipes, and the CI job `vendored`.
- The second moved the binding generator to the publishable crate `slang-bindgen` and made the build script of `slang-compiler-sys` generate the bindings and the list of C functions into `OUT_DIR` in every build and record the build tag of the Slang that it links in `BUILD_INFO`. It removed the pin file, the checked-in generated files, the static route with `SLANG_SOURCE_DIR`, every `DOCS_RS` code path, every `[package.metadata.docs.rs]` table, the recipes `pin`, `generate-bindings`, `check-bindings`, `test-static`, and `check-doc-without-slang`, and `--all-features` from the recipes.
- The third added the Cargo feature `vendored` of `slang-compiler-sys`, with the vendored route of its build script, which builds a minimal static Slang from the tree of `slang-src`, keeps the built libraries in the shared folder, finds the Microsoft C++ compiler itself on Windows, and records the static link in `BUILD_INFO`. It also made the vendored recipes and the CI job `vendored` build and test that route.
- The fourth added the crate `slang-glslang-sys`, which builds slang-glslang as static libraries from the tree of `slang-src` and holds the in-process loader, and the feature `vendored` of `slang-compiler`, which installs that loader on every global session that the crate creates. It made `slango`, `slango-codegen`, `slango-macros`, `slang-slint`, and the examples package forward the feature. It moved the vendored recipes to `slang-compiler/vendored` with two extra commands that check the forwards, added the Python 3 step to the CI job `vendored`, and made the vendored route of both native crates link the system libraries that Slang's archives and slang-glslang need.
- The fifth completed the user guide to the vendored build in `README.md` and the crate-level documentation. It covers which crate takes the feature and why `slango-macros` needs it as well, what the build needs and costs, the shared folder, which Slang a build gets and how `BUILD_INFO` shows it, the platforms, and the common failures. It closed the milestone.

M8, the forward-compatibility pass of `slango` and the first release (Section 15.11 of `docs/design/architecture.md`), is in progress. Its design exists:

- `docs/design/slango.md`, Sections 4.3 to 4.5, 5.5, and 6, designs the forward-compatibility pass. It puts the wgpu backend behind the default Cargo feature `wgpu`, without which the crate stops compiling with one `compile_error!` (SLO-28 to SLO-30), and makes the three accessors of `slango::wgpu` return an `Option` (SLO-31). It also finds that the Clippy lints of the workspace already enforce `#[non_exhaustive]` on every public type that needs it (SLO-32).
- `docs/design/tooling.md`, Sections 3.1, 5, 7.5, and 9, designs the publishing prerequisites and the release. It covers the crates and versions of a release, the check for breaking changes, the hosted documentation on GitHub Pages, the review of the public API, the release procedure with the steps reserved for the owner, and the change of `check-features` for the feature `wgpu` (TOOL-35 to TOOL-50).

The first code step of M8, done on 2026-11-15, implemented the forward-compatibility pass and the change of `check-features`:

- `slango` has the Cargo feature `wgpu`, on by default, which enables its dependencies `wgpu` and `pollster` and without which the crate stops compiling with one `compile_error!` that names the feature. A dependent that turns off the default features of `slango` names `wgpu`, as every member of the workspace that depends on `slango` does.
- `slango::wgpu::device`, `queue`, and `texture` return an `Option`, which is always `Some` while wgpu is the only backend, so a caller handles the `Option`. `View::to_image` of `slang-slint` turns `None` into an error of kind `TextureImport`, and the example `windowed-renderer` keeps its own clones of the wgpu device and queue and calls none of the three.
- `check-features` checks `slango` in a second command that leaves out its build with no features (TOOL-48 of `docs/design/tooling.md`).

On 2026-11-15 in the development container, every check passed with Rust 1.99.1 and with Rust 1.100.0. A code review then found that no test covered the mapping of `None` in `View::to_image`, and a unit test of `slang-slint` that Miri runs now checks it, which adds one test to the tiers. No test produces the `None` case of the three accessors themselves, and no automated check runs the build of `slango` without the feature. Section 3.10 of `docs/design/slango.md` records what a run by hand of that build reported.

The second code step of M8, done on 2026-11-16, added the tools of the release that Sections 7.5 and 9 of `docs/design/tooling.md` specify, and published, tagged, and deployed nothing:

- The `justfile` variables `release_packages`, the eight crates of the first release, and `documentation_site`, the base address of the hosted documentation.
- The recipes `check-package` and `check-release` in the group `checks`, `release-scripts-self-test` in the group `checks` and in `just check`, and `doc-site`, `publish-dry-run`, `publish`, `tag-release`, and `yank` in the new group `release`, with eight scripts under `scripts/`.
- The `documentation` field of each of the eight crates, which names its page on the hosted site, and `publish = false` on `slang-slint`.
- The workflow `.github/workflows/documentation.yml`, which builds the site and deploys it to GitHub Pages on a push of a tag that matches `v*`.

An agent can now check the packages, run Cargo's dry run of the publish, build the site, and check a release on crates.io. The owner can publish, tag, and yank, each only through its recipe, and an agent runs `tag-release` only after the owner's go-ahead, as Validating Changes states.

On 2026-11-15 and 2026-11-16 in the development container, before the first commit of the step, these runs gave its evidence:

- `just check` passed, with the self-test of the release scripts, and the test counts did not change. `check-features`, `check-msrv`, `check-vendored`, and `test-vendored` passed, and so did the formatting, Clippy, documentation, and feature checks with Rust 1.100.0.
- Cargo refused to package the uncommitted manifests, so the real `check-package` and `publish-dry-run` stopped. Copies of their scripts that pass `--allow-dirty` then passed: every one of the eight packages met the conditions of TOOL-45, the largest `.crate` file was that of `slang-compiler` with 194085 bytes, and the dry run packaged and verified the eight crates in the order of `release_packages` in 12 min 13 s and uploaded nothing. It cloned Slang twice, because Cargo had not tagged `target/` (Section 9.6 of `docs/design/tooling.md`).
- `doc-site` built a site of 27 MB from an empty target directory in 7 min 46 s, with no warning and no symbolic link. The site names the absolute build path in its pages of `slang-compiler-sys`, which the design accepts for a site that the workflow builds (TOOL-49 of `docs/design/tooling.md`).
- `check-release` reported that crates.io holds none of the eight crates.

A code review of the first commit of the step found two gaps in the tests. The checks inside the recipe `doc-site` had none, and neither did the logic in the recipes themselves, which is the relative notes path of `tag-release` and the default list of `publish` and `publish-dry-run`.

The step closed both. The checks of `doc-site` moved to `scripts/check-doc-site`, and the self-test now tests that script and runs the three recipes through `just` on a copy of the `justfile` (Section 9.9 of `docs/design/tooling.md`). On 2026-11-16, `just release-scripts-self-test` printed "release self-test passed: 57 cases", against 48 for the first commit, and `just check` passed.

On 2026-11-16 in the development container, the real recipes ran for the first time, on the committed tree of the first commit of the step. `just check-package` exited with status 0 and printed "check-package: every package passed". `just publish-dry-run` exited with status 0, printed "Uploading" and "warning: aborting upload due to dry run" for each of the eight crates in the order of `release_packages`, and wrote a record with that commit and the eight names.

The dry run took 4.7 s on a target directory that an earlier build had filled, which says nothing about a first run. Nothing was uploaded, and no token of crates.io exists on the machine.

In run 43780389601, for the first commit of the second code step of M8, all nine jobs passed, so the run concluded `success`. Its `just check` on the Linux runner included the self-test with 48 cases. It did not include the fixes of the review or the documents of the step.

Run 43783694243, for the single commit that held the whole step, its code, the fixes of the review, and the documents, concluded `failure`. Eight of the nine jobs passed. The job "Run every check on Linux" failed in `just check` at `release-scripts-self-test`, with "just: command not found". The self-test had narrowed `PATH` to its stubs, `/usr/bin`, and `/bin`, while the runner installs `just` outside `/usr/bin`, and the cases that run recipes through `just` came with the fixes of the review, after run 43780389601.

The self-test now resolves `just` to its absolute path before it narrows `PATH`, calls it only by that path, and stops with status 2 when `just` is not installed or a program that it needs is not in `/usr/bin` or `/bin` (TOOL-50 of `docs/design/tooling.md`). On 2026-11-16 in the development container, in a private mount namespace in which `/usr/bin/just` failed loudly and a copy of `just` sat beside a decoy `cargo` at the front of `PATH`, the fixed self-test passed and ran neither, and the self-test of that commit failed as CI did. `just check` then passed with 57 cases.

On 2026-11-16, GitHub Pages was not turned on for the repository, whose Pages endpoint answered with status 404, the remote held no tag, and the push of the branch started only the workflow `CI`. The `target/` directory of the main checkout holds Cargo's tag.

The packaged manifest of each of the eight crates carries the `authors` value of `[workspace.package]`, which Cargo copies into it. On 2026-11-17, the owner decided to keep the field as it is, which answers TQ-15 of `docs/design/tooling.md`.

The third code step of M8, done on 2026-11-16, ran the review of the public API of Section 9.5 of `docs/design/tooling.md` over the eight crates of the first release and fixed the findings that need no decision of the owner:

- The review had 45 findings. The step fixed the findings in code and in API documentation, and recorded every other as an open question for the owner in the design document of its crate, as TOOL-43 requires. These questions are CQ-8 to CQ-12 of `docs/design/slang-compiler.md`, SOQ-5 to SOQ-9 of `docs/design/slango.md`, SLMQ-3 and SLMQ-4 of `docs/design/slango-macros.md`, and SQ-6 and SQ-7 of `docs/design/slang-compiler-sys.md`. The owner has not decided them. Since the owner's decision of 2026-11-17, which the fourth code step records, they no longer block the first release.
- In `slang-compiler`, the module loaders take the path as `impl AsRef<Path>`, `type_conformance` takes its identifier as `Option<u32>`, `OptionName` implements `TryFrom<i32>`, `ErrorKind` lost the kind `FileSystem`, which no call returned, and the ordered enumerations, `TargetIndex`, and the settings types gained their comparison traits. In its module `reflection`, `explicit_counter_binding_range_offset` returns `None` for no counter, and the new `TypeLayout::field_layout` returns the field at a position. The documentation of `Component` and `SpecializationArg` states that neither is `Send` or `Sync`.
- In `slango`, an error of kind `Compile` from `render_pipeline` returns Slang's text as its `source()` and no longer in its `Display` text, so the hidden `ENCODING_VERSION` is 2. In `slango-codegen`, `generate` resolves a relative path, and `generate_precompiled` is hidden.
- In the native crates, `SlangUUID` derives `PartialEq`, `Eq`, and `Hash`, `BuildInfo` and `Linkage` derive `Hash`, `Linkage` implements `Display`, the error of `slang-bindgen` for libclang holds no advice, and `slang_glslang_sys_loader` is a `safe fn`. The fake of `slang-compiler` defines its vtable functions `extern "system"`, as FFI-7 requires.
- The crate documentation of `slang-compiler-sys` states how a program that implements a Slang interface in Rust keeps a panic out of Slang, and that of `slang-src` states what a dependent build script relies on. Two defects of Slang that the review found are KB-6 and KB-7 of Section 3.12 of `docs/design/slang-compiler.md`, and DEP-1 of the coding standard records the exception that `slango` needs for its build without the feature `wgpu`.

On 2026-11-16 in the development container, `just check`, `check-vendored`, `test-vendored`, `check-features`, `check-msrv`, `check-deny`, and `check-unused-deps` passed with Rust 1.99.1, and `check-fmt`, `check-clippy`, `check-doc`, `check-doctest`, `check-features`, and `test-gpu` passed with Rust 1.100.0. These runs were on the code of the step before its documentation changes.

The fourth code step of M8, done on 2026-11-17, carried out the owner's decisions of that day on the versions of the release:

- The owner decided that the public API ships as it is on `main`, and that every published release is an alpha until the owner decides otherwise. The open questions of the review stay open and are decided before the first release that is not an alpha (TOOL-37 of `docs/design/tooling.md`).
- Every member has the version `0.1.0-alpha.1`, and `slang-src` has `0.1.0-alpha.1.slang.2026.17.0.25.gc7954ebd1`, whose test now requires its pre-release to end with the Slang identifiers (SRC-13 of `docs/design/slang-src.md`). Every requirement between members is exact.
- `tag-release` passes `--prerelease` to `gh release create` when the version holds a hyphen, so the tag of the first release is `v0.1.0-alpha.1` and its GitHub release is a pre-release.
- `README.md` states that the API is unstable and that a dependent names the full pre-release version.

On 2026-11-17, the owner set the source of GitHub Pages to "GitHub Actions" and added the tag rule `v*` to the environment `github-pages`, which is step 6 of Section 9.6 of `docs/design/tooling.md`. The GitHub API then reported the build type `workflow`, the address `https://g-whiz.github.io/slang-rust/`, and two deployment policies, the branch `main` and the tag `v*`. Nothing is deployed, published, or tagged yet.

On 2026-11-17 in the development container, these checks passed on the code of the step with every requirement between members exact, before the last edits of its documents:

- The recipes `check-rust`, which printed "966 tests run: 966 passed, 0 skipped" and, under Miri, "557 tests run: 557 passed (32 slow), 409 skipped", and `release-scripts-self-test`, which printed "release self-test passed: 58 cases", together with `check-prose`, `check-shell`, `check-typos`, `check-toml`, `check-workflows`, `check-workflow-pins`, and `check-doc-references`. The self-tests of the checkers did not run again, so this is not the whole `just check`.
- `check-features`, `check-msrv`, `check-vendored`, and `check-clippy` with Rust 1.100.0.
- `test-vendored`, which printed "422 tests run: 422 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".

`check-package` and `publish-dry-run` did not run, because Cargo packages only a committed tree.

M8 still needs steps 3 to 5 of Section 9.6 of `docs/design/tooling.md`, which are `check-package`, `doc-site`, and `publish-dry-run`, on the merged commit of `main`. The owner then gives the go-ahead and publishes, and `tag-release` runs only with the owner's go-ahead. The change that follows the release then adds `check-semver` and the CI job `semver`, once TQ-16 of `docs/design/tooling.md` is answered.

Four open questions that the designs assign to M7 stay open, because no exit criterion of M7 requires them: SRCQ-1 and SRCQ-3 of `docs/design/slang-src.md`, SGLQ-2 of `docs/design/slang-glslang-sys.md`, and CQ-7 of `docs/design/slang-compiler.md`. The owner has not assigned them another milestone yet.

What exists and works:

- The `justfile` and its recipes, which `just --list` shows. They include these recipes:
  - The recipes that run the Rust checks, which pass no `--all-features`.
  - `check-slang-call-guard` generates the bindings with the binary of `slang-bindgen` into `target/slang-bindgen/` and then runs the checker on them.
  - `fmt` and `check-fmt` format and check every Rust file that git tracks or does not ignore.
  - `install-vulkan-sdk` installs the Vulkan SDK on a CI runner, and `check-vulkan-sdk` checks an installed SDK against the build tag that `recorded-slang` prints.
  - `recorded-slang` prints the recorded commit and build tag of `slang-src`.
  - `test-vendored` and `check-vendored` test, lint, and document `slang-src`, `slang-glslang-sys`, `slang-compiler-sys`, and `slang-compiler` with the feature `slang-compiler/vendored` in `target/vendored`, so their first run there clones Slang and builds Slang and slang-glslang from source. `test-vendored` then runs the tests of the modules `bindings` and `structs` of `slango-macros` with only `slango-macros/vendored`, and `check-vendored` runs Clippy on `slang-slint` with only `slang-slint/vendored`, which check the forward of the feature by each of the two crates (TOOL-7 of `docs/design/tooling.md`).
  - `check-vendored-shared-folder` checks where the clone is, recognizing a clone by its work tree `src/<commit>/tree/.git`, and prints the size of the shared folder.
  - The recipes of the release, which Section 9.6 of `docs/design/tooling.md` specifies. `check-package` checks the members without `publish = false`, the `documentation` fields, and each packaged `.crate` file. `check-release` checks a release on crates.io. `doc-site` builds the hosted documentation on the vendored route into `target/vendored/doc`. `publish-dry-run` runs Cargo's dry run and records the commit and the crates. `publish`, `tag-release`, and `yank` publish, tag, and yank. `check-package`, `publish-dry-run`, and `publish` exist on Linux and macOS only.
  - `release-scripts-self-test` runs the scripts of the release, and three of its recipes, against stub programs of `cargo`, `git`, `gh`, `curl`, and `sleep`, so it reaches neither crates.io nor GitHub (Section 9.9 of `docs/design/tooling.md`).

  The variables `release_packages` and `documentation_site` of the `justfile` hold the crates of the next release and the base address of the hosted documentation, which every recipe of the release reads (TOOL-35 and Section 9.4 of `docs/design/tooling.md`).

  The variables `vendored_packages` and `vendored_features` of the `justfile` hold the packages and the feature of the vendored recipes (TOOL-7). Every recipe that passes `--workspace` also passes `--exclude slang-src --exclude slang-glslang-sys`. `check-features` passes `--exclude-features vendored`, and checks `slango` in a second command with `--exclude-no-default-features` (TOOL-48 of `docs/design/tooling.md`).

  Its variable `slang_library_path` holds the assignment that puts `$VULKAN_SDK/lib` on the library path of Linux or macOS. The recipes that run compiled code which loads Slang put it at the start of their command line (SCS-32 of `docs/design/slang-compiler-sys.md`).
- The scripts under `scripts/`, which the recipes and hooks run, including `install-vulkan-sdk` and `check-vulkan-sdk`, and the custom checkers of Section 6 of `docs/design/tooling.md`. These are `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs`, `check-slang-call-guard`, `check-workflow-pins`, and `check-doc-references`, the extension of `check-prose` to the documentation comments of Rust files, and a self-test recipe for each. They also include the scripts of the release, `check-package`, `check-release`, `check-doc-site`, `publish-dry-run`, `publish`, `tag-release`, and `yank`, and their self-test `self-test-release`.
- The git hooks in `.pre-commit-config.yaml`.
- The documentation workflow `.github/workflows/documentation.yml`, which runs only on a push of a tag that matches `v*`. Its job `build` runs `just doc-site` on a fresh checkout and uploads the site, and its job `deploy`, the only job with `pages: write` and `id-token: write`, deploys it to GitHub Pages (Section 7.5 of `docs/design/tooling.md`). It has never run, because no such tag exists.
- The CI workflow `.github/workflows/ci.yml`, which runs on every push to `main`, every push to a branch that matches `feature/**`, and every pull request.
  - Every job makes libclang available, and every job except `vendored` installs the Vulkan SDK of the version that the workflow records, verified against a SHA-256 checksum that the workflow records, and checks it with `check-vulkan-sdk`, on Linux, Windows, and macOS. The job `coverage` measures test coverage on Linux and keeps the report as an artifact. Section 7 of `docs/design/tooling.md` specifies the jobs.
  - The job `vendored`, added on 2026-11-15, installs no Vulkan SDK and sets up no developer environment of the Microsoft toolchain.
    - It makes libclang available, installs CMake, Ninja, `g++`, `python3`, `pkg-config`, and `libfontconfig-dev` with apt on Linux where the runner lacks them, and prints the versions of CMake, Ninja, and, outside Windows, the C++ compiler. The development files of fontconfig serve the Clippy run on `slang-slint` of `check-vendored`, which builds Slint.
    - On Linux, Windows, and macOS, it prints the Python 3 that it finds as `python3` or `python` on `PATH`, and fails with an annotation when it finds none.
    - It runs `check-vendored` on Linux, and `test-vendored` and `check-vendored-shared-folder` on Linux, Windows, and macOS, and times both vendored recipes (Section 7.4 of `docs/design/tooling.md`).
  - In run 43710947459, for the commit that completed the first code step of M7, the job `vendored` passed on Linux, Windows, and macOS. Each ran 36 of 36 tests, found Cargo's tag in `target/vendored`, kept the clone in the shared folder `target/vendored/slang-src`, and made no second clone. That was the first run of the clone and of the lookup of the shared folder on Windows and macOS.
  - In run 43717303370, the first for the second code step of M7, the build script of `slang-compiler-sys` on Windows died with an access violation, exit code 0xc0000005, after it had printed every Cargo instruction. `slang-bindgen` then kept libclang loaded until the process exits (SCS-67 of `docs/design/slang-compiler-sys.md`). The run was cancelled when the next commit was pushed to the same feature branch, the first observed cancellation of an earlier run of a feature branch.
  - In run 43719234533, for the last commit of the second code step of M7, the build script of `slang-compiler-sys` completed on Windows with libclang at `C:\Program Files\LLVM\bin\libclang.dll`, and 817 tests passed with the generated bindings on WARP. On macOS, with libclang from the Xcode toolchain, 821 tests passed on Metal, and on Linux `just check` passed. Keeping libclang loaded therefore cured the crash, and the generated bindings and the libclang step have run on all three platforms.
  - In run 43725883567, for the first commit of the third code step of M7, the job `vendored` passed on Linux and macOS and failed on Windows.
    - On the macOS arm64 runner, with CMake 4.4.5, Ninja 1.13.2, and Apple clang 21.0.0, `test-vendored` from an empty target directory cloned and built Slang in 5 min 47 s, and 104 of 104 tests and 4 doctests passed. The shared folder was 330 MB at `target/vendored/slang-src`, in a target directory with Cargo's tag, with no second clone. That was the first build of Slang from source on macOS in this project.
    - On the Linux runner, with CMake 4.4.5 and Ninja 1.13.2, `check-vendored` took 6 min 30 s and `test-vendored` after it 8.6 s, 104 of 104 tests passed, and the shared folder was 349 MB.
    - On Windows, the job failed before any build of Slang. A workflow step exported the developer environment of the Microsoft toolchain, and rustc then took `link.exe` from `PATH`, where it found the `link` of Git's coreutils. The second commit of the step removed that step and made the build script find the compiler itself (SCS-68 of `docs/design/slang-compiler-sys.md`).
  - In run 43727977282, for the second commit of the third code step of M7, the job `vendored` passed on Linux, Windows, and macOS, and the default test job on macOS passed.
    - On Windows, with libclang at `C:\Program Files\LLVM\bin\libclang.dll`, CMake 4.4.5, Ninja 1.13.2, and no developer environment set up by the workflow, `test-vendored` from an empty target directory cloned and built Slang in 8 min 34 s. That was the first build of Slang from source on Windows in this project.
    - 109 of 109 tests, the 36 of `slang-src` and the 73 of `slang-compiler-sys`, and 4 doctests passed. Cargo tagged `target/vendored`, the shared folder was `target/vendored/slang-src` with 559 MB, and no second clone existed.
    - The run shows that the build script finds the Microsoft compiler on the hosted Windows runner from a shell without a developer environment, that the minimal static Slang builds and links there with the Ninja generator, and that the paths under the runner's target directory are short enough. It does not show a deeper target directory, another version of Visual Studio, or a machine with several installations (SA-17 of `docs/design/slang-compiler-sys.md`, SRCQ-1 of `docs/design/slang-src.md`).
  - In run 43737935692, for commit `f85d9e1`, the first commit of the fourth code step of M7, the job `vendored` passed on Linux and macOS and failed on Windows.
    - On Linux and macOS, it was the first build of `slang-glslang-sys` in CI, and on macOS the first run of the loader.
    - On Windows, the job found Python 3.12.10 and failed after 15 min 6 s. The clone, the build of Slang, and the build of slang-glslang with the Microsoft toolchain completed, and the link of the test binary of `slang-glslang-sys` then failed with `LNK2019: unresolved external symbol __imp_SHFileOperationW`, referenced by `Slang::Path::removeNonEmpty`. `link.exe` also wrote an import library and an export file for that test binary (Section 3 of `docs/design/slang-glslang-sys.md`).
    - The cause, which a reading of Slang's sources established, is that Slang's CMake declares no Windows system library for its archives, so rustc linked no shell32. The second commit of the step added the system libraries of SCS-69 of `docs/design/slang-compiler-sys.md`, and run 43742353739 confirmed the fix.
    - The default test jobs on Windows and macOS and the jobs `coverage`, `check-features`, and `check-msrv` passed.
  - In run 43742353739, for commit `5f5d454`, the second commit of the fourth code step of M7, eight of the nine jobs passed and the job `vendored` on Linux failed, so the run concluded `failure`.
    - On Windows, the job `vendored` passed. `test-vendored` from an empty target directory took 15 min 11 s. 410 of 410 tests passed with 202 doctests, one fewer than the 411 on macOS, because `session::tests::build_rejects_a_search_path_that_is_not_utf8` of `slang-compiler` does not run on Windows, and then 5 of 5 tests of `slango-macros` with 9 skipped by the filter. Both tests of `ffi::vendored` of `slang-compiler` and the four loader tests of `slang-glslang-sys` passed. The shared folder was `target/vendored/slang-src` with 759 MB, and no other directory held a clone.
    - So the test binary of `slang-glslang-sys` links on Windows with the ten system libraries, the loader has run there, and a vendored build optimizes SPIR-V there. The run does not show that no other program needs a library outside the ten (SA-18 of `docs/design/slang-compiler-sys.md`), a deeper target directory, or another installation of Visual Studio (SRCQ-1 of `docs/design/slang-src.md`, SA-17).
    - On macOS arm64, with CMake 4.4.5, Apple clang 21.0.0, and Python 3.15.2 at `/opt/homebrew/bin/python3`, the job `vendored` passed. `test-vendored` took 11 min 14 s from an empty target directory, 411 of 411 tests passed, then 5 of 5 tests of `slango-macros` with 10 skipped by the filter, both tests of `ffi::vendored` passed, and the shared folder was 346 MB.
    - It was the first run of the macro against a static Slang on macOS and on Windows.
    - On Linux, the job `vendored` failed after 9 min 35 s in the step that runs `check-vendored`, with `error: failed to run custom build command for yeslogic-fontconfig-sys v6.0.1`. Its new Clippy run on `slang-slint` builds Slint, which links fontconfig on Linux, and the job did not install its development files. `test-vendored` therefore did not run on Linux in this run. The third commit of the step made the job install `pkg-config` and `libfontconfig-dev` where missing.
    - The job "Run every check on Linux" passed, so `just check` passed in CI for `5f5d454`. The default test jobs on Windows and macOS and the jobs `coverage`, `check-features`, and `check-msrv` passed.
  - In run 43746178292, for commit `3362b7d`, the third and last commit of the fourth code step of M7, all nine jobs passed, so the run concluded `success`.
    - On Linux, with Python 3.14.4, the job `vendored` passed. `check-vendored` from an empty target directory took 14 min 53 s, including the Clippy run on `slang-slint`, which was the first run of the step that installs fontconfig and of that Clippy run on a Linux runner. `test-vendored` after it took 1 min 6 s: 411 of 411 tests passed, then 5 of 5 tests of `slango-macros` with 10 skipped by the filter, both tests of `ffi::vendored` passed, and the shared folder was 372 MB. That was the first run on a Linux runner of `test-vendored` with the final code of `slang-glslang-sys` and of the macro tests against a static Slang.
    - On Windows, `test-vendored` from an empty target directory took 16 min 30 s, 410 of 410 tests passed, then 5 of 5 tests of `slango-macros` with 9 skipped, and the shared folder was 759 MB.
    - On macOS, `test-vendored` from an empty target directory took 4 min 18 s, against 11 min 14 s in run 43742353739 on the same kind of runner. 411 of 411 tests passed, then 5 of 5 tests of `slango-macros` with 10 skipped, and the shared folder was 346 MB.
    - The job "Run every check on Linux", the default test jobs on Windows and macOS, and the jobs `coverage`, `check-features`, and `check-msrv` passed.
  - In run 43768611209, for the first commit of the first code step of M8, all nine jobs passed, so the run concluded `success`. They include `check-features` with its two commands, the job `vendored` on Linux, Windows, and macOS, and the default test jobs on Windows and macOS. The run did not include the unit test that the review added or the documents of the step.
  - Run 43749637842, for the merged tree of the fourth code step of M7 on `main`, passed all nine jobs after one rerun of the default test job on Windows. The hosted runner of its first attempt lost communication with the server before any build step ran.
  - On 2026-11-10, all six jobs, counting each entry of a matrix, passed on GitHub.
  - Afterwards, on 2026-11-10 and 2026-11-11, CI on `main` failed in the Windows job for three commits, from the commit that added the static route of `slang-compiler-sys` to the commit that added the design of `slang-compiler`. One test of the real host of the build script asserted an error kind that Windows reports differently. The commit "Assert only the variant of a start failure of the real host" fixed it, and run 42833561065 on `main` passed all six jobs.
  - On 2026-11-11, the workflow gained the trigger for pushes to feature branches. Run 42861650459, started by a plain push to a feature branch with no pull request, passed all six jobs.
  - Run 42898756060 on 2026-11-11, for the last commit of the branch that added modules, linking, and code generation to `slang-compiler`, passed all six jobs.
  - Run 42907583067 on 2026-11-11, for the commit that added `check-slang-call-guard`, passed all six jobs, and so did run 42923671966, for the last commit of the branch that added reflection to `slang-compiler`.
- The editor settings in `.editorconfig`.
- The Claude Code settings in `.claude/settings.json`, which allow a list of recipes to run without asking and run the prose checker after every edit of a Markdown file or a Rust file.
- The root `.gitattributes`, which keeps LF line endings in every checkout on every platform.
- `CLAUDE.md`, which imports this file.
- The workspace manifest `Cargo.toml`, with `default-members`, which leaves out `slang-src` and `slang-glslang-sys`, the `[workspace.package]` fields that the members inherit, and the workspace lint tables of Section 4.2 of the coding standard.
- The manifests of the eight crates of the first release, each with the `documentation` field of TOOL-40 of `docs/design/tooling.md`, which names its page on the hosted site.
- The tool configuration: `clippy.toml`, `deny.toml`, `.config/nextest.toml`, `taplo.toml`, and `typos.toml`.
- `crates/slang-compiler-sys`, with publication metadata and crate-level documentation that states the safety contract that every generated function shares and both routes. It also states that every crate between a program and it forwards the feature `vendored`, that the feature of `slang-compiler` installs the loader of `slang-glslang-sys`, and how a direct user installs that loader (SCS-37 of `docs/design/slang-compiler-sys.md`).
  - Its one Cargo feature, `vendored`, off by default, enables the optional dependency `slang-src` and the optional build dependency `find-msvc-tools`. `slang-bindgen` is its build dependency, and `slang-bindgen` and `find-msvc-tools` are its development dependencies (SCS-61 of `docs/design/slang-compiler-sys.md`). The crate root names `slang-src` with `use slang_src as _;`, because the build script needs only its `links` metadata (DEP-2 of the coding standard).
  - Its root includes the raw bindings that the build script generated into `OUT_DIR` and re-exports them. It also holds the hand-written `BuildInfo`, `Linkage`, and the constant `BUILD_INFO`, whose values the build script writes into `OUT_DIR/build_info.rs`.
  - Its build script `build.rs`, with its logic in `route.rs` and its real environment in `system.rs`, takes the vendored route with the feature and the dynamic route without it.
  - The dynamic route reads `VULKAN_SDK`, reads the build tag of the Slang headers there and compares it with nothing, and checks the library directory and the library file. It then checks that the target is 64-bit, warns on a target that is not one of the four tested targets, generates the bindings with `slang-bindgen` for `TARGET`, writes `bindings.rs`, `symbols.rs`, and `build_info.rs` into `OUT_DIR`, and links the Slang shared library of that SDK dynamically.
  - The vendored route never reads `VULKAN_SDK`. It reads the source tree, its commit, its build tag, and the shared folder from the `links` metadata of `slang-src`, refuses cross-compilation and a target without a link rule for its C++ runtime library, and probes Ninja. With the Microsoft toolchain it finds `cl.exe` with `find-msvc-tools`, and elsewhere it probes the compiler that `CXX` names, or `c++` when `CXX` is unset or empty, by running it with `--version`.
  - It then uses a complete entry of the shared folder whose inputs match, or copies the tracked files of the tree into `OUT_DIR`, builds a minimal static Slang with CMake and Ninja, verifies it, and stores it as a new entry. It generates the bindings from the headers of the entry, links six static libraries, the C++ standard library, and the system libraries of the target, and records `Linkage::Static` and the commit in `BUILD_INFO` (Section 9.3 of `docs/design/slang-compiler-sys.md`).
  - Its integration-test binary `tests/sys/` tests the build information of the route that the feature selects, tests every step and failure of both routes of the build script against a fake environment, and tests the real environment of `system.rs`, including the lock, the lookup of the Microsoft compiler, and the mapping of each kind of generator failure. It also holds the smoke test, which creates a global session and checks its build tag against `BUILD_INFO`, and the native-symbol test, which references every C function of the generated list.
  - It has 75 tests. Miri runs the tests of the build information and of the logic of the build script.
  - Its crate-level section `# Safety` states, besides the shared contract, the rule for a program that implements a Slang interface in Rust: each function of the vtable is defined `extern "system"` and stored in its `"system-unwind"` field through a conversion of the function pointer, so that a panic aborts at the boundary instead of unwinding into Slang.
  - On the dynamic route, the smoke and symbol tests have passed with generated bindings on Linux, Windows, and macOS, the last two in run 43719234533.
  - On the vendored route, the crate's tests and doctests have passed through `test-vendored` on x86_64 Linux in the development container and, in run 43727977282, before it linked system libraries, on the Linux, Windows, and macOS runners. A program outside the workspace built with the feature in the development container on 2026-11-15 ran without `VULKAN_SDK` and without a library path, and its release binary linked only the C++ and C runtime libraries of the system.
- `crates/slang-src`, the crate that obtains the Slang source tree for a vendored build, with publication metadata, its own licence `Apache-2.0 WITH LLVM-exception`, no dependencies, and crate-level documentation and no API.
  - Its build script `build.rs`, with its logic in `source.rs` and its real host in `system.rs`, shallow-clones the recorded commit of `recorded.rs` with its ten required submodules into the shared folder `slang-src` of the Cargo target directory, once for each target directory. It exports the path of the tree, the commit, the build tag, and the shared folder as `links` metadata (SRC-1 of `docs/design/slang-src.md`).
  - `slang-compiler-sys` depends on it with the feature `vendored`, and `slang-glslang-sys` depends on it always. An agent can build, lint, and test the crate through the vendored recipes and print the recorded Slang with `recorded-slang`, as Validating Changes states.
  - Its integration-test binary `tests/slang_src/` holds pure tests against a fake host and `ffi` tests of the real host, one of which checks the clone entry that the build script of the same build wrote (Section 12 of `docs/design/slang-src.md`).
  - On 2026-11-15 in the development container, before `slang-compiler-sys` depended on it, the first run of `check-vendored` in an empty `target/vendored`, which cloned, took 39 s, and the clone occupied 285 MB.
  - Its tests have passed on Linux, Windows, and macOS in the CI job `vendored`.
- `crates/slang-glslang-sys`, the crate that builds slang-glslang for a vendored build, with publication metadata, the licence `Apache-2.0 WITH LLVM-exception`, no features, `slang-compiler-sys` with the feature `vendored` and `slang-src` as its dependencies, and `find-msvc-tools` as its build dependency and development dependency (SGL-1 of `docs/design/slang-glslang-sys.md`).
  - Its build script `build.rs`, with its logic in `build_logic.rs` and its real host in `system.rs`, builds the wrapper project `native/CMakeLists.txt` with CMake and Ninja out of tree in `OUT_DIR`, reading the tree of `slang-src` in place. It builds glslang, SPIRV-Tools, Slang's glue code, and the loader shim `native/loader.cpp` as five static libraries, keeps them in an entry of the shared folder, and links them, the C++ standard library, and the system libraries of the target.
  - Its only API is the foreign function `slang_glslang_sys_loader`, which returns the static loader. It is declared `safe fn`, so a call needs no `unsafe` block (SGL-11 of `docs/design/slang-glslang-sys.md`). The loader answers every request for a library whose file name starts with `slang-glslang` with the nine linked functions of the glue code and passes every other request to Slang's default loader.
  - Its integration-test binary `tests/glslang/` holds 23 pure tests of the build logic against a fake host, 6 `ffi` tests of the real host, and 4 `ffi` tests of the loader. No recipe runs the pure tests under Miri (SRCQ-3 of `docs/design/slang-src.md`).
  - Its tests have passed through `test-vendored` in the development container on x86_64 Linux, in run 43737935692 on the Linux and macOS runners, in run 43742353739 on the Windows and macOS runners, and in run 43746178292 on all three runners.
- `crates/slang-compiler`, the safe crate without its custom file system, with publication metadata, `slang-compiler-sys` and the optional `slang-glslang-sys` as its dependencies, and `insta` as its only development dependency.
  - Its one Cargo feature, `vendored`, off by default, enables the feature `vendored` of `slang-compiler-sys` and the dependency `slang-glslang-sys`. With it, the crate installs the loader of `slang-glslang-sys` on every global session that it creates, before it returns it, through the guarded thunk of `setSharedLibraryLoader` (SLC-33, SLC-34 of `docs/design/slang-compiler.md`). A global session that `from_raw` adopts gets no loader.
  - Its test module `ffi::vendored`, compiled only with the feature, shows that a global session of the crate returns optimized SPIR-V and that one created through the raw bindings without the loader does not. On x86_64 Linux in the development container, the SPIR-V of the test shader was 1292 bytes with one function and no call with the loader, and 1452 bytes with two functions, two calls, and the diagnostic `failed to load downstream compiler 'spirv-opt'` without it. Both tests passed on the Linux and macOS runners in run 43737935692, on the Windows and macOS runners in run 43742353739, and on all three runners in run 43746178292.
  - A release program outside the workspace with the feature, built in the development container on 2026-11-15, ran without `VULKAN_SDK` and without a library path, and linked only the C++ and C runtime libraries of the system.
  - Its public API covers the global session, sessions with targets and compiler options, modules, entry points, composition, specialization, linking, code generation, blobs, and metadata, and every `layout` method. It also covers the public module `reflection`, with the layouts, reflection handles, and records of reflection, and the error type `Error` with its `ErrorKind` of eight kinds, which has no kind `FileSystem` while the custom file system is deferred. The crate's API documentation is the detailed record.
  - Every wrapper holds the object that Slang created it from, and every reflection handle borrows the wrapper it came from.
  - Internally, the crate counts every reference through one owning interface pointer and maps every failure signal of Slang to an `ErrorKind` in one function. It also keeps a per-thread Slang call guard that panics on a reentrant call and defers releases until the outermost call returns.
  - Every call of the crate into Slang runs inside the crate-private function `abort_on_exception`, which aborts the process with one line on standard error that names the call when a C++ exception escapes Slang. The custom checker `check-slang-call-guard` checks where each such call sits (SLC-32).
  - A user cannot give a session a custom file system, which is deferred until a concrete need appears.
  - Some inputs crash or abort the process through known defects of Slang, such as a shader whose types nest past Slang's limit of 128 levels when an entry point of it is looked up with a stage or its layout is requested. Section 3.12 of `docs/design/slang-compiler.md` lists them (Hard Rule 14).
  - Its unit tests run under Miri against a fake of the Slang interfaces, and its integration-test binary `tests/compiler/` holds `ffi` tests that call Slang. On 2026-11-11, all of them passed in CI on Linux, Windows, and macOS, in run 42923671966.
- `crates/slango`, the high-level crate for compute kernels, which also draws with render pipelines into a texture, with publication metadata, `slang-compiler` as its dependency, `wgpu` and `pollster` as its optional dependencies, and two Cargo features (SLO-27 of `docs/design/slango.md`):
  - `wgpu`, on by default, enables `wgpu` and `pollster` and gates every module and re-export of the crate root. Without it, the crate stops compiling with one `compile_error!` that names the feature (SLO-28, SLO-29).
  - `vendored`, off by default, only forwards to `slang-compiler`.
  - Its public API covers `Device` with `DeviceOptions` and `AdapterSelection`, `Module`, `Kernel`, `Call`, `Buffer`, `Texture`, `Sampler`, the sealed traits `Element`, `VertexElement`, and `Value`, the error type `Error` with its `ErrorKind`, and the public module `wgpu`, whose accessors `device`, `queue`, and `texture` return an `Option` (SLO-31).
  - It also has the hidden module `__private` for the code that `slango-macros` generates, which is exempt from semantic versioning (SLM-6 of `docs/design/slango-macros.md`). The crate's API documentation is the detailed record.
  - Its pure unit tests run under Miri, and the `gpu` tests of its integration-test binary `tests/slango/` passed in CI on lavapipe, WARP, and hosted Metal.
- `crates/slang-slint`, the crate that shares Slint's wgpu device with `slango` and shows what `slango` draws in a Slint user interface, with publication metadata, `publish = false`, because it belongs to the second release, `slango` and `slint` as its dependencies, and the feature `vendored`, which only forwards to `slango` (SLS-17 of `docs/design/slang-slint.md`).
  - Its public API covers these items:
    - `wgpu_configuration` returns the configuration with which Slint creates a device that suits `slango`.
    - `device` turns the wgpu 30 device and queue that Slint passes to a rendering notifier into a `slango::Device`.
    - `View` draws with a `slango` render pipeline into a texture that Slint shows as an image.
    - The error type `Error` with its `ErrorKind`.
  - It has no unsafe code. The crate's API documentation is the detailed record.
  - Its pure tests run under Miri, and the `gpu` tests of its integration-test binary `tests/slang_slint/` passed locally on 2026-11-11 (Section 9 of `docs/design/slang-slint.md`).
- `crates/slango-codegen`, the library crate that holds the generator of `slango-macros`, with publication metadata, `slango` with an exact version, `proc-macro2`, and `quote` as its dependencies, `insta`, `prettyplease`, and `syn` as its development dependencies, and the feature `vendored`, which only forwards to `slango` (SLM-23 of `docs/design/slango-macros.md`).
  - Its public API covers `generate`, which resolves a relative path against the working directory, and the error type `Error` with its `ErrorKind` of three kinds and its method `report`. The function `generate_precompiled` is also `pub`, for the crate's integration test, but `#[doc(hidden)]` and exempt from semantic versioning, like the hidden type of `slango` that it takes (Section 4.2 of `docs/design/slango-macros.md`). It has no unsafe code. Its crate-level documentation says when to use it instead of the macro and why it requires `slango` with an exact version. The crate's API documentation is the detailed record.
  - Its pure unit tests run under Miri, and the `ffi` tests of its integration-test binary `tests/codegen/` compare the generated code with `insta` snapshots.
- `crates/slango-macros`, the procedural macro crate, with publication metadata, `slango-codegen` with an exact version and `syn` as its dependencies, `slango` and `trybuild` as its development dependencies, and the feature `vendored`, which only forwards to `slango-codegen` (SLM-23).
  - With `vendored` on `slango-macros`, the macro's copy of Slang is built from source, and a program without a Vulkan SDK enables `vendored` on `slango` as well, because Cargo resolves the features of the macro's host dependencies apart (Section 4.3 of `docs/design/slango-macros.md`). On 2026-11-15 in the development container, a release program outside the workspace with `vendored` on both crates built from an empty target directory without a Vulkan SDK in 6 min 18 s, built Slang and slang-glslang once each for both copies, and ran the generated saxpy kernel. With the feature on only one of the two crates, its build stopped with the error that `VULKAN_SDK` is not set. That was on x86_64 Linux only.
  - Its one macro, `shader!`, compiles a Slang file with the Slang of its own copy of `slang-compiler`, by default that of the Vulkan SDK at `VULKAN_SDK`, while the crate builds and generates a typed kernel for each compute entry point and a Rust struct for each Slang structure that the kernels take (Sections 6 and 7 of `docs/design/slango-macros.md`).
  - Its build script gives the macro library an rpath to `$VULKAN_SDK/lib` on Linux and macOS (Section 6.3 of that design). A documentation build expands the macro with Slang like any other build, and the macro has no stub.
  - Its crate-level documentation states what building a crate that calls the macro needs, how failures appear, the exact versions that tie it to `slango`, the lint exceptions that a calling module may need, and the known limits.
  - Its integration-test binary `tests/macros/` holds pure tests that Miri runs, `ffi` tests, and `gpu` tests that passed locally on 2026-11-11 (Section 10 of that design).
  - In CI run 43300696682 on `main`, for commit `376f10a`, the `gpu` test `gpu::typed_kernel_advances_the_particles_and_prints_the_checked_results` of the examples passed on lavapipe on Linux, WARP on Windows, and Metal on macOS.
- `examples/`, the unpublished package `slang-rust-examples`, with these examples and their tests:
  - `compile-and-reflect` compiles a Slang file and prints its entry points and parameters.
  - `compute-kernel` runs a compute kernel with `slango` and checks its results.
  - `windowed-renderer` draws a textured quad in a window and reloads its shader.
  - `shader-playground` shows a shader in a Slint window with two sliders and reloads it.
  - `typed-kernel` runs a compute kernel through the typed bindings that `slango_macros::shader!` generates and checks its results.

  The tests are described in Section 13.6 of `docs/design/slang-compiler.md`, Sections 13.3 and 13.5 of `docs/design/slango.md`, Section 10 of `docs/design/slang-slint.md`, and Section 10 of `docs/design/slango-macros.md`. No recipe runs an example.
- `crates/slang-bindgen`, the binding generator, a library crate with publication metadata, `clang` and `clang-sys` as its dependencies, no features, and a thin binary. It parses the Slang headers with libclang for the target that its caller names and returns the raw bindings and the list of their C functions as unformatted text, with its rule set and its shim headers.
  - The first call of `generate` keeps libclang loaded until the process exits, through a static handle of `clang-sys`, without unsafe code. A later call works on any thread once the earlier one has returned, and a call during another call fails with `ErrorKind::Libclang` (SCS-67 of `docs/design/slang-compiler-sys.md`).
  - Its public API covers `generate`, the struct `Generated`, and the error type `Error` with its `ErrorKind` of nine kinds, `Location`, and `Result`. The crate's API documentation is the detailed record.
  - Its binary `slang-bindgen` takes a header directory, a target triple, and an output directory, and writes `bindings.rs` and `symbols.rs` there. Only the recipe `check-slang-call-guard` runs it.
  - Its unit tests in `src/tests.rs` parse synthetic headers and compile for Linux only. Its integration-test binary `tests/generator/` runs the binary on the headers at `VULKAN_SDK` and on failing inputs, and compiles for Linux only as well (Section 7.5 of `docs/design/slang-compiler-sys.md`).
  - The binary's output for the headers of the Vulkan SDK in the development container was the same text for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, and `aarch64-unknown-linux-gnu` on 2026-11-15. In run 43717303370, the build script generated the bindings on Windows and macOS as well, and the tests of `slang-bindgen`, which compile for Linux only, ran on Linux only.
- The four licence texts at the root, `LICENSE-APACHE`, `LICENSE-MIT`, `LICENSE-APACHE-LLVM`, and `LICENSE-GPL`, a symbolic link to the text of its own licence in each package directory, and `README.md`, whose section "Building Slang From Source" is the user guide to the vendored build.
- The writing guide, the coding standard, the reference list, and the list of upstream reports under `docs/`.
- The architecture design, `docs/design/architecture.md`, which specifies the planned crates, how Slang is obtained and which Slang is recorded, and the roadmap of milestones M0 to M18.
- The tooling design, `docs/design/tooling.md`, which specifies the toolchain configuration, the recipes, the hooks, the custom checkers, the CI workflow, and the requirements on the container image, which milestone M0 implemented, and the release procedure, the check for breaking changes, and the hosted documentation, which M8 designed and whose recipes, scripts, and workflow, all but `check-semver` and the CI job `semver`, the second code step of M8 implemented.
- The design of `slang-compiler-sys` and the binding generator `slang-bindgen`, `docs/design/slang-compiler-sys.md`, a draft for M7, whose dynamic route, generator, and build information the second code step of M7 implemented, whose vendored route the third implemented, and whose system libraries the fourth added.
- The design of `slang-src`, `docs/design/slang-src.md`, a draft for M7 that the first code step implemented.
- The design of `slang-glslang-sys`, `docs/design/slang-glslang-sys.md`, a draft for M7 that the fourth code step implemented.
- The design of `slang-compiler` and the compile-and-reflect example, `docs/design/slang-compiler.md`, a draft that milestone M2 implemented except for the custom file system.
- The design of `slango` and the compute kernel example, `docs/design/slango.md`, a draft that milestone M3 implemented, and that also holds the design of M4, rendering, which M4 implemented.
- The design of `slang-slint` and the Slint example, `docs/design/slang-slint.md`, a draft that milestone M5 implemented.
- The design of `slango-macros` and its generator `slango-codegen`, `docs/design/slango-macros.md`, a draft that milestone M6 implemented.

`just check` passes in the development container, and it passed in CI in run 43749637842. The tests of `slang-bindgen` and of `slang-compiler-sys` run in the default tier.

The tests of the `gpu` tier are those of `slango`, of `slang-slint`, of `slango-macros`, of the examples `compute-kernel` and `typed-kernel`, and of the scenes of the examples `windowed-renderer` and `shader-playground`. On 2026-11-16, after the third code step of M8, the test tiers have 966 tests on Linux, of which Miri runs 557 and skips 409. `test-vendored` runs 422 tests, the 36 of `slang-src`, the 33 of `slang-glslang-sys`, the 75 of `slang-compiler-sys`, and the 278 of `slang-compiler`, then 5 tests of `slango-macros`, and 203 doctests, the 198 of `slang-compiler`, the 4 of `slang-compiler-sys`, and the 1 of `slang-glslang-sys`. No recipe runs the tests of `slang-src` or `slang-glslang-sys` under Miri, and a run by hand of those of `slang-src` on 2026-11-15 ran 28 and skipped 8 (SRCQ-3 of `docs/design/slang-src.md`).

Building the workspace needs a LunarG Vulkan SDK with `VULKAN_SDK` set to its directory, and libclang, on every platform, which the development container and every CI job except `vendored` provide. The crates are tested with the Slang of SDK 1.4.363.1, and the build accepts the Slang of any SDK. Every command that compiles the workspace, such as `cargo check`, `cargo clippy`, `cargo test`, and every recipe that compiles, runs the build script of `slang-compiler-sys`, which generates the bindings with libclang. None of them enables the feature `vendored` or builds `slang-src` or `slang-glslang-sys`, which `default-members` and the `--exclude` options of the recipes leave out, so none clones Slang or builds Slang or slang-glslang from source.

A vendored build needs no Vulkan SDK. It needs libclang, git and network access for the first build in a target directory, CMake 3.22 or later, Ninja, a C++20 compiler, which on Windows is an installation of Visual Studio with its C++ tools, and, for slang-glslang, Python 3 on `PATH` as `python3` or `python`. The development container provides all of them.

Without `VULKAN_SDK`, the build script stops the build with an error that starts "VULKAN_SDK is not set, so slang-compiler-sys cannot find the Slang library to link", says what to install and set, and ends with the advice that enabling the Cargo feature `vendored` builds Slang from source instead. Without libclang, it stops the build with an error that starts "cannot generate the Slang bindings from", names the header directory and the target, holds "cannot load libclang:" with the reason, and ends with "slang-compiler-sys generates its bindings with libclang in every build, so install libclang or set LIBCLANG_PATH to the directory that holds it."

On the vendored route, a missing program stops the build with an error that starts "cannot start" and names git, CMake 3.22 or later, Ninja, and a C++ compiler. On Windows, a missing compiler stops it with an error that starts "cannot find the Microsoft C++ compiler cl.exe" and says where the build script looked. A failed configure or build step names the log `slang/build.log` under `OUT_DIR` and holds its last 40 lines. Section 9.6 of `docs/design/slang-compiler-sys.md` lists every message. The build script of `slang-glslang-sys` names CMake 3.22 or later, Ninja, Python 3 on `PATH`, and a C++20 compiler when a program cannot start, and its log is `build.log` under its own `OUT_DIR` (Section 12.4 of `docs/design/slang-glslang-sys.md`).

On Linux, building the workspace also needs the development files of fontconfig, which Slint links and the development container provides (Section 3.2 of `docs/design/slang-slint.md`).

A test binary that starts without `$VULKAN_SDK/lib` on the library path stops before any test runs, on Linux with "error while loading shared libraries: libslang-compiler.so.0.2026.17: cannot open shared object file". The agent therefore runs the tests through their recipes, which supply that path. Section 18.4 of `docs/design/slang-compiler-sys.md` lists every failure and its recovery.

What does not exist yet:

- The custom file system of `slang-compiler`, which is deferred with no milestone assigned (Section 15.22 of `docs/design/architecture.md`). It is the trait `FileSystem` and the setter `SessionBuilder::file_system` of Section 8 of `docs/design/slang-compiler.md`, with the error kind `FileSystem`, the Rust-owned blob, the panic handling of SLC-18, and the Slang call guard raised around the user's code of every vtable function (SLC-28). A user who needs one reaches Slang's file system interface through `slang-compiler-sys`.
- The path-tracing demo of Section 10 of `docs/design/architecture.md`, the broadcasting case and the differentiable function of `compute-kernel`, and tests of the `hw_gpu` tier.
- The recipe `check-semver` and the CI job `semver`, which the change that follows the first release adds (TOOL-39 of `docs/design/tooling.md`).
- The first release: no crate is published, no tag `v<version>` and no GitHub release exist, and the hosted documentation does not exist, so the `documentation` field of each crate names a page that does not answer yet.
- The design document of each crate other than `slang-compiler-sys`, `slang-compiler`, `slango`, `slang-slint`, `slango-macros`, `slango-codegen`, `slang-src`, and `slang-glslang-sys`, which is written as the first task of the milestone that delivers the crate.
- A recipe that builds the examples package with its feature `vendored`, and a `gpu` test on the vendored route.

The workspace lint tables, `clippy.toml`, the tools that `just check` runs, and the custom checkers enforce their rules, so an agent learns those rules from a failing check, as Hard Rule 3 states.

The custom checkers enforce these rules:

- The rules of the writing guide on comments in Rust source, on the headings of item documentation, on `ignore` in doctests, and on the `# Features` section.
- The rules of the writing guide on ASCII punctuation, semicolons, and listed spellings in API documentation.
- The part of rule SLC-32 of `docs/design/slang-compiler.md` that places every call of `slang-compiler` into Slang alone inside the closure of `abort_on_exception`.
- The rules of Section 7.2 of `docs/design/tooling.md` on pinned actions, the `pull_request_target` event, and top-level workflow permissions.
- The part of Hard Rule 15 that forbids citations of project documents, sections, and rule identifiers in code and configuration.

Section 7.12 of the writing guide and Section 4.8 of the coding standard say which part of each rule a checker enforces. Every rule that no configured tool checks stays a judgement rule.

`check-features`, `check-msrv`, `check-vulkan-sdk`, and `coverage` run outside `just check`, and CI runs them in every run of the workflow. The agent MUST NOT assume that a planned crate, recipe, file, or tool exists, because an agent that builds on something absent produces code that cannot work.

## Repository Layout

- `AGENTS.md`: This file.
- `CLAUDE.md`: A one-line file that imports `AGENTS.md` for Claude Code.
- `Cargo.toml`: The workspace manifest. It holds these parts:
  - Resolver 3.
  - The `[workspace.package]` fields that the members inherit, such as edition 2024 and `rust-version`, and the licence `MIT OR Apache-2.0`, which every member except `slang-compiler-sys`, `slang-glslang-sys`, `slang-slint`, and `slang-src` inherits (TOOL-4 of `docs/design/tooling.md`).
  - The version `0.1.0-alpha.1` in `[workspace.package]`, which every member except `slang-src` inherits (TOOL-37 of `docs/design/tooling.md`).
  - The `authors` field in `[workspace.package]`, which every member inherits and Cargo copies into every packaged manifest, and which the owner keeps as it is (Section 9.6 of `docs/design/tooling.md`).
  - In `[workspace.dependencies]`, the dependencies `clang`, `clang-sys`, `find-msvc-tools`, `insta`, `pollster`, `prettyplease`, `proc-macro2`, `quote`, `slang-bindgen`, `slang-compiler`, `slang-compiler-sys`, `slang-glslang-sys`, `slang-slint`, `slang-src`, `slango`, `slango-codegen`, `slango-macros`, `slint`, `syn`, `trybuild`, `wgpu`, and `winit`, with `slang-bindgen`, `slang-compiler`, `slang-compiler-sys`, `slang-glslang-sys`, `slang-slint`, `slang-src`, `slango`, `slango-codegen`, and `slango-macros` by path and exact version.
  - The workspace lint tables.

  Its members are `crates/slang-bindgen`, `crates/slang-compiler`, `crates/slang-compiler-sys`, `crates/slang-glslang-sys`, `crates/slang-slint`, `crates/slang-src`, `crates/slango`, `crates/slango-codegen`, `crates/slango-macros`, and `examples`. Its `default-members` are every member except `crates/slang-src` and `crates/slang-glslang-sys`, so a plain `cargo build` in the root of the workspace does not clone Slang or build slang-glslang (TOOL-32 of `docs/design/tooling.md`).
- `Cargo.lock`: The lock file of the workspace.
- `LICENSE-APACHE`, `LICENSE-MIT`, `LICENSE-APACHE-LLVM`, `LICENSE-GPL`: The texts of the Apache License 2.0, the MIT licence, the Apache License 2.0 with the LLVM exception, and version 3 of the GNU General Public License, each held once (TOOL-33 of `docs/design/tooling.md`).
- `LICENSE`, `LICENSE-APACHE`, and `LICENSE-MIT` in a package directory: Relative symbolic links to the root texts of the package's own licence, under the conventional name, which is `LICENSE` in `crates/slang-compiler-sys/`, `crates/slang-glslang-sys/`, `crates/slang-slint/`, and `crates/slang-src/`, and `LICENSE-APACHE` with `LICENSE-MIT` in every other package directory, including `crates/slang-bindgen/` and `examples/`. `cargo package` follows the links, but a checkout without symbolic link support holds only the target path, so a release is packaged on Linux or macOS (TOOL-33 of `docs/design/tooling.md`).
- `README.md`: The README, which is also the readme of every member through the inherited `readme` field.
- `crates/slang-bindgen/`: The binding generator that Project Status describes. It has these parts:
  - `Cargo.toml`, which inherits `clang` and `clang-sys` from `[workspace.dependencies]`.
  - The library under `src/` with the private modules `parse`, `rules`, `check`, `emit`, `model`, and `error`. The module `emit` writes the unwinding ABIs of SCS-10 and SCS-11 of `docs/design/slang-compiler-sys.md`.
  - `src/rules.rs`, the rule set, which decides every declaration and macro of the Slang headers (Section 4 of `docs/design/slang-compiler-sys.md`).
  - The unit tests in `src/tests.rs`, which compile for Linux only, and in the modules.
  - The binary `src/main.rs`.
  - The integration-test binary `tests/generator/`, with `main.rs` and the module `ffi.rs`, which compiles for Linux only and runs the binary (Section 7.5 of `docs/design/slang-compiler-sys.md`).
  - `shim/`, the shim headers that replace the system headers when the generator parses for a target.
- `crates/slang-compiler-sys/`: The crate of raw bindings that Project Status describes, with `Cargo.toml`, which declares `links = "slang-compiler"` and the feature `vendored`, inherits `slang-bindgen` as a build dependency and a development dependency, `find-msvc-tools` as an optional build dependency and a development dependency, and `slang-src` as an optional dependency. It also has `src/lib.rs`, which holds these parts:
  - The crate-level documentation, with both routes.
  - The opt-out of `unsafe_code`.
  - The module `bindings` with its two lint exceptions, which includes `bindings.rs` from `OUT_DIR`, and the re-export of its items.
  - `use slang_src as _;` under the feature `vendored`.
  - `BuildInfo`, `Linkage`, and `BUILD_INFO`, whose value it includes from `build_info.rs` in `OUT_DIR`.
- `crates/slang-compiler-sys/build.rs`: The build script, which includes `route.rs` and `system.rs`, runs the logic on the real system, prints the Cargo instructions, and returns the result (Section 9.7 of `docs/design/slang-compiler-sys.md`).
- `crates/slang-compiler-sys/route.rs`: The logic of both routes of the build script, as plain functions behind the trait `Host` for everything it reads, runs, generates, or writes, which the build script and its test both include (SYS-8 of the coding standard). It holds the tested targets, the reading of the build tag, the derivation of `version`, the options of the vendored build of Slang, the entry protocol of the shared folder with the entry key, and every failure message (Sections 9.2 and 9.3 of `docs/design/slang-compiler-sys.md`).
- `crates/slang-compiler-sys/system.rs`: The struct `System`, which implements `Host` on the real environment, file system, and processes with the standard library, on the generator with `slang-bindgen`, and on the lookup of the Microsoft compiler with `find-msvc-tools` under the feature `vendored`. It holds every lock until the build script exits and maps each kind of generator error to a kind of the route. The build script and the test module `tests/sys/build_script/ffi.rs` both include it.
- `crates/slang-compiler-sys/tests/sys/`: The integration-test binary of the crate, with `main.rs` and these modules:
  - `build_info.rs` tests `BUILD_INFO` for the route that the feature `vendored` selects.
  - `build_script.rs` includes `route.rs` and tests every step and failure of both routes of the build script against a fake environment.
  - `ffi.rs` holds the smoke test and the native-symbol test and carries the only `unsafe_code` exception of the test binary.
  - `symbols`, declared in `main.rs`, includes the generated list of C functions from `OUT_DIR`.
- `crates/slang-compiler-sys/tests/sys/build_script/ffi.rs`: The tests of `System`, which include `system.rs`, load libclang, start child processes of the test binary, such as for the lock, compare the lookup of the Microsoft compiler with `find-msvc-tools`, and write files under `CARGO_TARGET_TMPDIR`, so they sit in a module named `ffi`, which Miri skips.
- `crates/slang-src/`: The crate that Project Status describes, with `Cargo.toml`, which declares `links = "slang-src"`, the version `0.1.0-alpha.1.slang.2026.17.0.25.gc7954ebd1`, which encodes the release and the recorded build tag (SRC-13 of `docs/design/slang-src.md`), its own licence, no dependencies, and no features. It also has `src/lib.rs`, which holds only the crate-level documentation.
- `crates/slang-src/recorded.rs`: The constants `RECORDED_COMMIT` and `RECORDED_BUILD_TAG`, which the build script and the test include and the recipe `recorded-slang` prints (SRC-11 of `docs/design/slang-src.md`).
- `crates/slang-src/build.rs`: The build script, which includes `recorded.rs`, `source.rs`, and `system.rs`, runs the logic on the real host, prints the Cargo instructions, and returns the result.
- `crates/slang-src/source.rs`: The logic of the build script, as plain functions behind the trait `Host` for everything it reads, runs, or writes (SYS-8 of the coding standard). It holds the required submodules, the git settings, the repository variables, the lookup of the shared folder, the entry protocol, and every failure message.
- `crates/slang-src/system.rs`: The struct `System`, which implements `Host` with the standard library only and holds every lock until the build script exits, and which the build script and the test module `tests/slang_src/build_script/ffi.rs` both include.
- `crates/slang-src/tests/slang_src/`: The integration-test binary of the crate, with `main.rs` and these modules:
  - `build_script.rs` includes `recorded.rs` and `source.rs` and tests the logic against a fake host.
  - `build_script/ffi.rs` includes `system.rs`, tests the real host, starts git, and writes files under `CARGO_TARGET_TMPDIR`, so Miri skips it.
- `crates/slang-glslang-sys/`: The crate that Project Status describes, with `Cargo.toml`, which declares `links = "slang-glslang"`, its own licence, no features, `slang-compiler-sys` with the feature `vendored` and `slang-src` as dependencies, and `find-msvc-tools` as a build dependency and a development dependency. It also has `src/lib.rs`, which holds the crate-level documentation, the opt-out of `unsafe_code`, `use slang_src as _;`, and the one `unsafe extern "C"` block that declares `slang_glslang_sys_loader`.
- `crates/slang-glslang-sys/native/CMakeLists.txt`: The wrapper project, which adds the submodules spirv-headers, spirv-tools, and glslang of the source tree with the options of SGL-3 of `docs/design/slang-glslang-sys.md` and defines the static library `slang-glslang-static` from Slang's glue code and `loader.cpp`.
- `crates/slang-glslang-sys/native/loader.cpp`: The loader shim, with the static library of the nine linked functions, the static loader, which passes every other name to Slang's default loader, and the exported function `slang_glslang_sys_loader` (Section 5 of `docs/design/slang-glslang-sys.md`).
- `crates/slang-glslang-sys/build.rs`: The build script, which includes `build_logic.rs` and `system.rs`, runs the logic on the real host, prints the Cargo instructions, and returns the result.
- `crates/slang-glslang-sys/build_logic.rs`: The logic of the build script, as plain functions behind the trait `Host` (SYS-8 of the coding standard). It holds the link rules and system libraries of each kind of target, the configure options, the entry protocol of the shared folder with the entry key, which compiles in both native files with `include_str!`, and every failure message. It repeats parts of `route.rs` of `slang-compiler-sys` by copy (Section 4.1 of `docs/design/slang-glslang-sys.md`).
- `crates/slang-glslang-sys/system.rs`: The struct `System`, which implements `Host` on the real environment, file system, and processes, and on the lookup of the Microsoft compiler with `find-msvc-tools`, and which the build script and the test module `tests/glslang/build_script/ffi.rs` both include.
- `crates/slang-glslang-sys/tests/glslang/`: The integration-test binary of the crate, with `main.rs`, which carries the `unsafe_code` exception on the module `ffi`, and these modules:
  - `build_script.rs` includes `build_logic.rs` and tests the logic against a fake host.
  - `build_script/ffi.rs` includes `system.rs` and tests the real host, so Miri skips it.
  - `ffi.rs` tests the loader that the crate links.
- `crates/slang-compiler/`: The safe crate that Project Status describes, with `Cargo.toml`, which declares the feature `vendored` and inherits `slang-compiler-sys`, the optional `slang-glslang-sys`, and the development dependency `insta` from `[workspace.dependencies]` (Section 4.1 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/lib.rs`: The crate-level documentation of the current scope, the opt-out of `unsafe_code`, the declarations of the private modules, and the re-exports of the public items at the crate root.
- `crates/slang-compiler/src/owned.rs`: The owning interface pointer `Owned`, the only code that adds or releases a reference, with the checked cast through `queryInterface`, and the crate-private unsafe trait through which it calls the generated thunks of `addRef`, `release`, and `queryInterface` (SLC-2, Section 5.4 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/guard.rs`: The Slang call guard and its queue of deferred releases, each in a `thread_local!` (SLC-28), and `abort_on_exception`, which aborts the process when a C++ exception escapes the one call into Slang that it wraps (SLC-32).
- `crates/slang-compiler/src/error.rs`: The public types `Error`, `Result`, `ErrorKind`, `Operation`, `Diagnostics`, and `Compiled` (Sections 6.8 and 6.9 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/module.rs`: The module loaders and `loaded_modules` of `Session`, `Module`, `EntryPoint`, and the iterators `LoadedModules`, `EntryPoints`, and `DependencyFilePaths` (Section 6.5).
- `crates/slang-compiler/src/component.rs`: `Component`, `SpecializationArg`, `Session::compose`, `Session::type_conformance`, `ComponentType`, `TypeConformance`, and `LinkedProgram`, with linking, code generation, and the tracked entry point count (Sections 6.6 and 6.7).
- `crates/slang-compiler/src/blob.rs`: `Blob`, `Metadata`, `BindingSpace`, `BindingIndex`, and the crate-private reading of diagnostics blobs (Section 6.7).
- `crates/slang-compiler/src/reflection.rs`: The public module `reflection`, which re-exports the items of its private modules and holds their shared helpers (Section 9 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/reflection/`: The private modules `binding`, `kinds`, `program`, `type_layout`, `types`, and `variable`, which hold the reflection handles, records, iterators, and value types.
- `crates/slang-compiler/src/mapping.rs`: The one function that maps every failure signal of Slang to an `ErrorKind` (SLC-11).
- `crates/slang-compiler/src/enums.rs`: The enumerations `TargetFormat`, `Toggle`, `LanguageVersion`, `MatrixLayout`, `Stage`, `OptimizationLevel`, `DebugInfoLevel`, `FloatingPointMode`, `LineDirectiveMode`, and `ParameterCategory`, with their crate-private conversions to the raw values (SLC-14).
- `crates/slang-compiler/src/options.rs`: `CompilerOptions` with its typed and raw setters, `OptionName`, the list of the options that a target rejects, and the crate-private types that own the C strings of the option entries during a call to Slang (Section 7 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/session.rs`: `Session`, `SessionBuilder`, `Target`, and `TargetIndex`, with the checks of `build`, the building of the session descriptor, and the filling of the fields of each target descriptor (SLC-6, SLC-31).
- `crates/slang-compiler/src/strings.rs`: The one conversion of text that the crate passes to Slang into a C string, which reports a NUL byte as an error (SLC-15). It also holds the shared reading of a string that Slang returns and the one constructor of the error of kind `InvalidUtf8`.
- `crates/slang-compiler/src/global_session.rs`: `GlobalSession` and `GlobalSessionBuilder`, with the check of the build tag of the loaded Slang against `slang_compiler_sys::BUILD_INFO`, the installation of the loader of `slang-glslang-sys` under the feature `vendored`, and the order of creation of Section 6.3 of `docs/design/slang-compiler.md`.
- `crates/slang-compiler/src/fake.rs`: The test-only fake of the Slang interfaces built on the generated vtable structs, which the unit tests run under Miri (SLC-25). Its vtable functions are defined `extern "system"` and stored in their fields through a conversion of the function pointer (Section 13.4 of `docs/design/slang-compiler.md`). Its global session records each session descriptor and each shared library loader that it receives, and its session writes into its global session after releasing it, as Slang's session does, so that Miri reports a wrong release order. Its modules, entry points, component types, and metadata model the counting behaviours of Section 5.3 of `docs/design/slang-compiler.md`.
- `crates/slang-compiler/tests/compiler/`: The integration-test binary `compiler`, with these files:
  - `main.rs` carries the `unsafe_code` exception on the module `ffi`.
  - `ffi.rs` holds the `ffi` tests of the global session through the public API.
  - `ffi/session.rs` holds the `ffi` tests of sessions, targets, and compiler options.
  - `ffi/compile.rs` holds the `ffi` tests of modules, linking, code generation, and metadata.
  - `ffi/settings.rs` holds the tests of the effect of each compiler option on generated code.
  - `ffi/exceptions.rs` holds the tests of the inputs that let a C++ exception escape Slang, some run in a child process of the test binary.
  - `ffi/paths.rs` holds the tests of relative paths.
  - `ffi/reflection.rs` holds the tests of reflection with its `insta` snapshots under `ffi/snapshots/`.
  - `ffi/vendored.rs`, compiled only with the feature `vendored`, holds the two tests of the loader of `slang-glslang-sys`.
  - `ffi/support.rs` holds their helpers.
- `crates/slango/`: The crate `slango` that Project Status describes. It has these parts:
  - `Cargo.toml`, which declares the features `default`, `wgpu`, and `vendored` and inherits `slang-compiler`, the optional `pollster` and `wgpu`, and the development dependency `insta` from `[workspace.dependencies]`.
  - `src/` with the private modules `buffer`, `call`, `device`, `draw`, `encoding`, `error`, `kernel`, `layout`, `marshal`, `module`, `render`, `sampler`, and `texture`. Their unit tests sit under `src/buffer/`, `src/call/`, `src/draw/`, `src/encoding/`, `src/kernel/`, `src/marshal/`, `src/render/`, `src/sampler/`, `src/texture/`, and, with their `insta` snapshots, `src/layout/`.
  - The public module `wgpu`.
  - The hidden public module `__private`, with the types of the interface description in `src/__private/interface.rs` and its unit tests under `src/__private/`.
  - The integration-test binary `tests/slango/`, whose module `gpu` holds the tests of the `gpu` tier. They include these tests:
    - Loading, in `gpu/load.rs`.
    - The precompiled load, in `gpu/precompiled.rs`.
    - The call, in `gpu/call.rs` and `gpu/call/`.
    - Textures and sampling, in `gpu/texture.rs` and `gpu/sample.rs`.
    - Render pipelines and draws, in `gpu/render.rs` and `gpu/render/`.
  - The shaders that the tests load, under `tests/slango/shaders/`.
- `crates/slang-slint/`: The crate `slang-slint` that Project Status describes. It has these parts:
  - `Cargo.toml`, which sets `publish = false` (TOOL-35 of `docs/design/tooling.md`), declares the feature `vendored`, inherits `slango` with its feature `wgpu` named and `slint` from `[workspace.dependencies]`, and enables the Slint features of SLS-1 of `docs/design/slang-slint.md`.
  - `src/lib.rs` with the crate-level documentation and the re-exports.
  - The private modules:
    - `device` holds `wgpu_configuration` and `device`.
    - `error` holds the error type and its unit tests.
    - `view` holds `View`.
  - The integration-test binary `tests/slang_slint/`, with these modules:
    - `configuration`, `device`, and `view` hold the pure tests.
    - `gpu` holds the tests of the `gpu` tier.
    - `common` holds their helpers.
- `crates/slango-codegen/`: The generator crate that Project Status describes. It has these parts:
  - `Cargo.toml`, which declares the feature `vendored`, inherits `proc-macro2`, `quote`, and the development dependencies `insta`, `prettyplease`, and `syn` from `[workspace.dependencies]` and declares `slango` with an exact version and its feature `wgpu` itself (SLM-2 of `docs/design/slango-macros.md`).
  - `src/lib.rs` with the crate-level documentation, `generate`, and `generate_precompiled`.
  - The private modules:
    - `emit` writes the tokens and has its unit tests in `src/emit/tests.rs`.
    - `emit::structs` in `src/emit/structs.rs` writes the structs for Slang structures and has its unit tests in `src/emit/structs/tests.rs`.
    - `emit::types` in `src/emit/types.rs` holds the Rust types of Slang values, the Rust type of a generated struct, the operation constant `GENERATE` of the errors of unsupported input, and the error of an unsupported parameter.
    - `names` holds the naming rules and the search for conflicts.
    - `error` holds the error type.
  - The integration-test binary `tests/codegen/`, whose module `ffi` holds the tests that compile the shaders under `tests/codegen/shaders/`, with their `insta` snapshots under `tests/codegen/snapshots/`.
- `crates/slango-macros/`: The procedural macro crate that Project Status describes. It has these parts:
  - `Cargo.toml`, which declares the feature `vendored` and inherits `slango-codegen`, `syn`, and the development dependencies `slango`, with its feature `wgpu` named, and `trybuild` from `[workspace.dependencies]`.
  - `src/lib.rs` with the crate-level documentation and `shader!`.
  - The build script `build.rs`, which includes the logic in `instructions.rs` (SYS-8 of the coding standard).
  - The integration-test binary `tests/macros/`, with these modules:
    - `bindings` calls the macro.
    - `build_script` includes `instructions.rs`.
    - `ffi` runs the trybuild cases under `tests/macros/ui/` and compares generated structs with Slang's reflection.
    - `gpu` runs the generated saxpy kernel and a kernel over a buffer of a generated struct.
    - `structs` checks the byte conversion of generated structs.
  - The shaders under `tests/macros/shaders/`.
- `examples/Cargo.toml`: The manifest of the unpublished package `slang-rust-examples`, which declares the feature `vendored`, forwarded to `slang-compiler`, `slang-slint`, `slango`, and `slango-macros`, and inherits `pollster`, `slang-compiler`, `slang-slint`, `slango`, with its feature `wgpu` named, `slango-macros`, `slint`, `wgpu`, `winit`, and the development dependency `insta` from `[workspace.dependencies]`, and enables Slint's windowing backend on winit and its FemtoVG renderer on wgpu (Section 10 of `docs/design/slang-slint.md`).
- `examples/src/bin/compile-and-reflect.rs`: The example `compile-and-reflect`, which a user runs from the workspace root with `cargo run -p slang-rust-examples --bin compile-and-reflect -- examples/shaders/scale.slang`, with the Slang library on the library path.
- `examples/src/bin/compute-kernel.rs`: The example `compute-kernel`, which a user runs from the workspace root with `cargo run -p slang-rust-examples --bin compute-kernel -- examples/shaders/saxpy.slang`, with the Slang library on the library path and a GPU driver or a software driver.
- `examples/src/shared/`: The modules that several binaries of the examples include with a `#[path]` attribute, because the package has no library crate:
  - `report.rs` prints an error and its causes on standard error. All five binaries include it.
  - `frames.rs` reads the command line of both windowed examples and handles their option `--frames`.
  - `watched.rs` checks whether the shader of a windowed example changed so that it reloads. Both windowed binaries and their scenes use it.
- `examples/src/bin/windowed-renderer/`: The example `windowed-renderer`, with `main.rs`, the modules `scene.rs`, `format.rs`, and `retry.rs`, which the tests also compile, and the shared modules `frames.rs`, `report.rs`, and `watched.rs`. `retry.rs` holds the redraw retry and the wake time of the event loop. A user runs it from the workspace root with `cargo run -p slang-rust-examples --bin windowed-renderer -- examples/shaders/textured-quad.slang` (Section 13.5 of `docs/design/slango.md`).
- `examples/src/bin/shader-playground/`: The example `shader-playground`, with `main.rs`, the modules `ui.rs`, which declares the Slint window, and `scene.rs`, which the tests also compile, and the shared modules `frames.rs`, `report.rs`, and `watched.rs`. A user runs it from the workspace root with `cargo run -p slang-rust-examples --bin shader-playground -- examples/shaders/playground.slang`, with `SLINT_WGPU_CPU=1` on a software adapter (Section 10 of `docs/design/slang-slint.md`).
- `examples/src/bin/typed-kernel.rs`: The example `typed-kernel`, which calls `slango_macros::shader!` on `examples/shaders/particles.slang`. A user runs it from the workspace root with `cargo run -p slang-rust-examples --bin typed-kernel`, with the Slang library on the library path and a GPU driver or a software driver. It takes no file argument, because the macro embeds the shader when the example builds (Section 10 of `docs/design/slango-macros.md`).
- `examples/shaders/`: The shaders of the examples and their tests, `scale.slang`, which compiles, `broken.slang`, which does not, `saxpy.slang`, the kernel of `compute-kernel`, `textured-quad.slang`, the shader of `windowed-renderer`, `playground.slang`, the shader of `shader-playground`, and `particles.slang`, the shader of `typed-kernel`.
- `examples/tests/examples/`: The integration-test binary of the examples, with `main.rs`, which also includes the shared modules `frames.rs` and `watched.rs`. Its parts are these:
  - `common.rs` holds the helpers that run an example binary and check its output. They check its usage text, the options that both windowed examples share in that text, and its rejection of bad arguments. It also holds the helpers that create a device and that copy and rewrite a shader in a scratch directory.
  - `unattended.rs` tests the reading of `--frames`, the frame limit, and the watchdog of an unattended run.
  - `redraw_retry.rs` tests the redraw retry and the wake time of `windowed-renderer`.
  - The module `ffi.rs` runs `compile-and-reflect` on `scale.slang` and `broken.slang`, with each of its options, with bad arguments, and on a file that it cannot read.
  - `ffi/watched.rs` tests `watched.rs` on scratch files.
  - `ffi/compute_kernel.rs`, `ffi/typed_kernel.rs`, `ffi/windowed_renderer.rs`, and `ffi/shader_playground.rs` test the usage text and the arguments of the other four examples.
  - The module `gpu.rs` runs `compute-kernel` on `saxpy.slang`, on `broken.slang`, and on a missing file, and runs `typed-kernel`.
  - `colour_format.rs` checks the choice of colour format of `windowed-renderer`.
  - `gpu/windowed_renderer.rs` and `gpu/shader_playground.rs` check the scene of each windowed example drawn offscreen and its reload.
  - The `insta` snapshots are under `snapshots/`.
- `clippy.toml`: The Clippy configuration of Section 4.3 of the coding standard.
- `deny.toml`: The cargo-deny configuration of Section 8 of `docs/design/tooling.md`, whose `[advisories]` table ignores five advisories by the owner's decisions and whose `[bans]` table skips 31 duplicate versions. Its `[licenses]` table accepts `GPL-3.0-or-later` for `slang-slint` alone, through an exception. It accepts `GPL-3.0-only` for every crate, so a reviewer checks that no crate under a permissive licence depends on a crate under the GPL (TOOL-34). Section 8 of `docs/design/tooling.md` lists the licence allow-list, the advisories, and the skips with the reason for each.
- `.config/nextest.toml`: The nextest profiles `default`, `gpu`, and `hw-gpu`, which implement the test tiers as Section 4 of `docs/design/tooling.md` defines. It also holds `default-miri`, which Miri uses and which skips the modules named `ffi`, `gpu`, and `hw_gpu`, and an override that keeps the output of the `gpu` tests of `slango`, which name their adapter.
- `taplo.toml`: The taplo configuration, with four-space indentation and the exclusion of `target/` and `.worktrees/`.
- `typos.toml`: The typos configuration, which ignores two correct patterns that typos would otherwise report, and accepts three whole identifiers that the Slang headers misspell and that the design documents name (Section 5.10 of `docs/design/slang-compiler-sys.md`).
- `docs/writing-documentation.md`: The normative writing guide.
- `docs/rust-coding-standards.md`: The normative Rust coding standard, with the specification of the lint and tool configuration.
- `docs/rust-references.md`: The reference list of external sources behind the coding standard, with a topic index.
- `docs/upstream-reports.md`: The one to-do list of bug reports to make to upstream projects, with an entry for each defect that should be reported and the drafts written so far (Hard Rule 14).
- `docs/design/architecture.md`: The architecture design, which is authoritative for the crates, their layering, how Slang is obtained and which Slang is recorded, the examples, the test tiers, platform support, publishing, and the roadmap.
- `docs/design/tooling.md`: The tooling design, which is authoritative for the toolchain configuration, the recipes, the hooks, the custom checkers, the CI workflow, the dependency policy, the release procedure with the hosted documentation, and the requirements on the container image.
- `docs/design/slang-compiler-sys.md`: The design of `slang-compiler-sys` and the binding generator `slang-bindgen`, which is authoritative for the scope and shape of the generated bindings, the tested targets, the generator, the build information, the build script on both routes, and the tests of the crate.
- `docs/design/slang-compiler.md`: The design of `slang-compiler` and the compile-and-reflect example, which is authoritative for these topics:
  - The facts about Slang that the safe crate relies on.
  - Its ownership model, public API, errors, and compiler options.
  - Its file system and reflection.
  - Its threading rules, soundness invariants, and tests.
- `docs/design/slango.md`: The design of `slango` with the compute kernel example for M3 and rendering with the windowed renderer example for M4, which is authoritative for these topics:
  - The facts that the design relies on.
  - The public types and the backend boundary.
  - Loading, the call, and the draw.
  - Errors, threading, and device features.
  - The tests and the dependency changes.
- `docs/design/slang-slint.md`: The design of `slang-slint` and the Slint example for M5, which is authoritative for these topics:
  - The facts that the design relies on.
  - The dependency on Slint and its licence consequence.
  - The sharing of the device and the view.
  - Errors, threading, and the tests.
- `docs/design/slango-macros.md`: The design of `slango-macros` and its generator `slango-codegen` for M6, which is authoritative for these topics:
  - The facts that the design relies on.
  - The hidden module and the element type check of `slango`.
  - The macro and the generated code.
  - Errors, threading, and the tests.
- `justfile`: The root `justfile`, with the groups `checks`, `ci`, `git`, `release`, and `rust`. The group `ci` holds `install-vulkan-sdk`, which only the CI workflow runs, and the group `release` holds `doc-site`, `publish-dry-run`, `publish`, `tag-release`, and `yank`. Its variables `release_packages` and `documentation_site` hold the crates of the next release, in the order of their dependencies, and the base address of the hosted documentation. Its variables `vendored_packages`, `vendored_features`, and `vendored_environment` hold the packages, the feature, and the environment of `test-vendored` and `check-vendored`, which are `slang-src slang-glslang-sys slang-compiler-sys slang-compiler` and `slang-compiler/vendored`.
- `.github/workflows/ci.yml`: The CI workflow, which Section 7 of `docs/design/tooling.md` specifies. It runs on every push to `main`, every push to a branch that matches `feature/**`, and every pull request.
  - It has the jobs `check`, `test`, `ci-only`, `coverage`, and `vendored`. Every job makes libclang available, and each job except `vendored` then installs the Vulkan SDK and checks it with `check-vulkan-sdk`.
  - Its job `vendored` is a matrix of Linux, Windows, and macOS that installs the stable toolchain, `just`, and cargo-nextest, prints the version of git, makes libclang available, installs the missing ones of CMake, Ninja, `g++`, `python3`, `pkg-config`, and `libfontconfig-dev` on Linux, prints the versions of the build tools, prints the Python 3 that it finds or fails, and runs the vendored recipes under `time` (Section 7.4 of `docs/design/tooling.md`).
  - Its top-level `env` records the Vulkan SDK version `VULKAN_SDK_VERSION` and the SHA-256 checksums of the SDK downloads.
  - Its libclang step installs `libclang-dev` on Linux, and on Windows and macOS checks the libclang of the runner image and sets `LIBCLANG_PATH` to its directory (Section 7.1 of `docs/design/tooling.md`).
  - Its `check` and `coverage` jobs install lavapipe from `mesa-vulkan-drivers` for the tests of the `gpu` tier.
  - Its `check`, `ci-only`, and `coverage` jobs install `libfontconfig-dev`, and its job `vendored` installs it on Linux where it is missing, because a Linux build of Slint links fontconfig.
- `.github/workflows/documentation.yml`: The workflow that builds the hosted documentation with `just doc-site` and deploys it to GitHub Pages on a push of a tag that matches `v*`, which Section 7.5 of `docs/design/tooling.md` specifies.
- `scripts/check-package`: The script of `check-package`, which checks the members, the `documentation` fields, and each packaged `.crate` file.
- `scripts/check-release`: The script of `check-release`, which checks the versions, the `documentation` fields, and the hosted pages of a release on crates.io.
- `scripts/check-doc-site`: The script that `doc-site` runs after `cargo doc`, which checks the page of each crate, writes the index page, and checks the site for symbolic links and its size.
- `scripts/publish-dry-run`: The script of `publish-dry-run`, which runs `cargo publish --dry-run` and writes the record `target/publish-dry-run.commit`.
- `scripts/publish`: The script of `publish`, which refuses unless the checks of Section 9.6 of `docs/design/tooling.md` pass and then runs `cargo publish --no-verify`.
- `scripts/tag-release`: The script of `tag-release`, which tags `HEAD`, pushes the tag, and creates the GitHub release.
- `scripts/yank`: The script of `yank`, which runs `cargo yank` and refuses in CI and without a token.
- `scripts/self-test-release`: The self-test of the release scripts, which `release-scripts-self-test` runs against stub programs (Section 9.9 of `docs/design/tooling.md`).
- `scripts/install-vulkan-sdk`: The script that downloads the Vulkan SDK on a GitHub Actions runner, verifies it against a checksum, and installs it. It exports `VULKAN_SDK`, the library path on Linux and macOS, and the directory of the SDK's programs to the later steps of the job (Section 7.3 of `docs/design/tooling.md`).
- `scripts/check-vulkan-sdk`: The script that checks that the SDK at `VULKAN_SDK` provides the Slang headers, library, and `slangc` of the build tag that `just recorded-slang` prints.
- `scripts/check-prose`, `scripts/prose-wordlist.txt`: The prose checker for Markdown files and for the documentation comments of Rust files, and its list of non-Canadian spellings.
- `scripts/check-rust-comments`: The checker that Rust source holds only API documentation and safety comments.
- `scripts/check-doc-headings`: The checker of the headings in item documentation.
- `scripts/check-doctest-attributes`: The checker that no documentation code block is marked `ignore`.
- `scripts/check-feature-docs`: The checker that the crate-level documentation of each library names every Cargo feature.
- `scripts/check-slang-call-guard`: The checker that every call of `slang-compiler` into Slang runs alone inside the closure of `abort_on_exception`, which reads the names of Slang's functions from `target/slang-bindgen/bindings.rs`, the bindings that its recipe generates first (Section 6.9 of `docs/design/tooling.md`).
- `scripts/check-workflow-pins`: The checker that every action is pinned by a full commit SHA, and that no workflow uses `pull_request_target` or grants more than read access at its top level.
- `scripts/check-doc-references`: The checker that code and configuration cite no project document, section, or rule identifier (Hard Rule 15). It reads the rule identifiers from the tables of the documents under `docs/` on every run.
- `scripts/rust_lexer.py`: The Python module that the custom checkers share, which reads Rust tokens and documentation comments.
- `scripts/self-test-checker`: The runner of the self-test of each custom checker.
- `scripts/fixtures/`: The cases of each self-test, one directory per checker and per case, with input files that end in `.fixture`. Each case of `check-slang-call-guard` that needs bindings holds them at `target/slang-bindgen/bindings.rs.fixture`, so the self-test runs without `slang-bindgen`.
- `scripts/check-shell`, `scripts/justfile-recipes`: The `shellcheck` run over every shell script and every recipe.
- `scripts/check-commit-message`: The commit message checker.
- `scripts/claude-hook-prose`: The Claude Code hook that runs the prose checker after an edit to a Markdown file or a Rust file.
- `scripts/merge-branch`: The squash merge that `just merge` runs.
- `scripts/worktree-prune`: The cleanup of squash-merged branches that `just worktree-prune` and the `post-merge` hook run.
- `.pre-commit-config.yaml`: The `pre-commit`, `commit-msg`, and `post-merge` hooks.
- `.claude/settings.json`: The Claude Code settings for this repository, with the allowed commands and the hook that runs the prose checker after every edit of a Markdown file or a Rust file. The allowed commands include `just check`, every recipe that it runs except `check-slang-call-guard` and `check-slang-call-guard-self-test`, `check-vulkan-sdk`, `check-features`, `check-commit-message`, `recorded-slang`, and the three test recipes of the tiers. They exclude recipes such as `fmt`, `merge`, `check-msrv`, `coverage`, `test-vendored`, `check-vendored`, `check-vendored-shared-folder`, `release-scripts-self-test`, and every recipe of the release procedure (Section 5 of `docs/design/tooling.md`).
- `.editorconfig`: The character set, line endings, final newline, and indentation for each file type.
- `.gitattributes`: The single line `* text=auto eol=lf`, which keeps LF line endings in every checkout on every platform (Section 14.3 of `docs/design/tooling.md`).
- `.gitignore`: The files that git ignores. Its last line keeps the directories `target/` of the cases of `check-slang-call-guard` under `scripts/fixtures/`, which the pattern `target` would otherwise ignore.

`target/`, `.worktrees/`, `.code-review/`, and `.idea/` are local and ignored by git. `check-slang-call-guard` writes the generated bindings into `target/slang-bindgen/`, and the vendored recipes build in `target/vendored`. There the build script of `slang-src` keeps its clone in the shared folder `target/vendored/slang-src`, the build script of `slang-compiler-sys` keeps its built libraries and headers in the entries of `target/vendored/slang-src/slang-compiler-sys/`, and the build script of `slang-glslang-sys` keeps its built libraries in the entries of `target/vendored/slang-src/slang-glslang-sys/`. `doc-site` writes the hosted documentation into `target/vendored/doc`. `check-package` writes the packages into `target/package`, and `publish-dry-run` writes its record `target/publish-dry-run.commit` and verifies the packages in `target/debug`.

These locations are planned and do not exist yet:

- `docs/design/<crate-name>.md`: One design document for each crate other than `slang-compiler-sys`, `slang-compiler`, `slango`, `slang-slint`, `slango-macros`, `slango-codegen`, `slang-src`, and `slang-glslang-sys`, written as the first task of the crate's milestone.
## Technology Decisions

`docs/design/architecture.md` is authoritative for the architecture: the crates and their layering, how Slang is obtained and which Slang is recorded, the responsibilities of each crate, the examples, the test tiers, platform support, publishing, and the roadmap. This section summarizes it, together with the toolchain and environment decisions, and the architecture design prevails wherever the two disagree.

`docs/design/tooling.md` is the design for the toolchain configuration, the recipes, the hooks, the CI workflow, and the requirements on the container image, and it prevails over this section on those topics.

`docs/design/slang-compiler-sys.md` is the design of `slang-compiler-sys` and the binding generator `slang-bindgen`, `docs/design/slang-src.md` is the design of `slang-src`, and `docs/design/slang-glslang-sys.md` is the design of `slang-glslang-sys`. `docs/design/slang-compiler.md` is the design of `slang-compiler` and the compile-and-reflect example. Each prevails over this section on its topics.

This section summarizes the target design, which includes the rework of the native crates that M7 designed. The code implements the whole M7 design:

- The crate `slang-bindgen`, which the build script of `slang-compiler-sys` runs in every build.
- The dynamic route of `slang-compiler-sys`, with the bindings and the list of C functions generated into `OUT_DIR`, the build information `BUILD_INFO`, and the warning on an untested target.
- The run-time check of `slang-compiler` against `BUILD_INFO`.
- The crate `slang-src`.
- The Cargo feature `vendored` of `slang-compiler-sys` and its vendored route, which builds Slang from the tree of `slang-src`, keeps the built libraries in the shared folder, finds the Microsoft compiler on Windows, and links Slang statically.
- The crate `slang-glslang-sys` with its loader, which `slang-compiler` installs under its feature `vendored`, and its exclusion in the recipes.
- The forwarding of the feature `vendored` by every crate between a user and `slang-compiler-sys`.
- The vendored recipes and the vendored CI job, which build `slang-src`, `slang-glslang-sys`, `slang-compiler-sys`, and `slang-compiler` with the feature of `slang-compiler` and check the forwards of `slango-macros` and `slang-slint`.
- Recipes that pass no `--all-features`, and the removal of the pin, the checked-in generated files, the static route, and every `DOCS_RS` code path.
- The user guide to the vendored build in `README.md` and the crate-level documentation, which documents the shared folder.

Project Status says what exists, and Validating Changes names the recipes that exist today.

The planned crates are:

- `slang-compiler-sys`: The raw bindings, which its build script generates with `slang-bindgen` in every build from the headers of the Slang that it links, the build information `BUILD_INFO`, and the linking of Slang, with the manifest key `links = "slang-compiler"` (Section 6 of `docs/design/architecture.md`).
- `slang-src`: The crate whose build script obtains the Slang source tree for a vendored build and exports its path. It is the only code of the workspace that reaches the network during a build (ARCH-35, `docs/design/slang-src.md`).
- `slang-glslang-sys`: The crate that builds slang-glslang from the same tree and holds the in-process loader through which a statically linked Slang optimizes SPIR-V (`docs/design/slang-glslang-sys.md`).
- `slang-compiler`: Safe, idiomatic wrappers over `slang-compiler-sys`, with no graphics processing unit (GPU) dependency. Its compile and reflection API exists, as Project Status describes. With its feature `vendored`, it installs the loader of `slang-glslang-sys` on every global session that it creates, and it checks when it creates a global session that the loaded Slang reports the build tag of `BUILD_INFO` (SLC-5, SLC-34 of `docs/design/slang-compiler.md`).

  Every wrapper holds the object that Slang created it from, and no wrapper, builder that holds one, reflection handle, or iterator over them is `Send` or `Sync`. A call that returns diagnostics returns its value together with them on success. Every failure is the crate's one `Error` type, whose kind callers match on (Sections 5.2, 6.8, 6.9, and 10 of `docs/design/slang-compiler.md`).

  Every call of the crate into Slang, which is every call of a generated thunk or C function, MUST run inside the guard `guard::abort_on_exception` as the innermost frame around that one call (SLC-32 of `docs/design/slang-compiler.md`). A C++ exception that escapes Slang must abort the process before any destructor of the crate runs. A new call into Slang therefore still goes through the guard, because the checker `check-slang-call-guard` requires it.

  The abort guard is frozen. It consists of the unwinding ABIs of the generated declarations, `abort_on_exception`, and the checker. The agent MUST NOT extend it with audits of which functions of Slang throw, new tests of an abort, or new "Aborts" paragraphs.

  The agent MUST remove the mechanism instead of modifying it when a change would have to modify it, by returning the declarations to non-unwinding ABIs and making the calls direct. The mechanism exists only to give a defect of Slang a defined outcome (Hard Rule 14, Section 11.6 of `docs/design/slang-compiler.md`).

  The checker enforces where each call sits. Three cases stay a judgement rule, together with the other cases that Section 6.9 of `docs/design/tooling.md` lists:

  - What the guard's closure captures by value without `move`.
  - Calls that a macro defined outside the crate's sources makes.
  - Calls through a function pointer read from a vtable.

  Two rules bind every change to its compiler options, because Slang reads some options only from fields of the target descriptor and others only from the session:

  - `SessionBuilder::build` fills the floating-point mode, the line directive mode, the scalar layout, and the method of generating SPIR-V of each target descriptor. It takes each from the target's options, otherwise the session's, and reads it exactly as Slang reads it, so that a setting of the session is not silently overwritten (SLC-31 of `docs/design/slang-compiler.md`).
  - `SessionBuilder::build` rejects `LanguageVersion`, `WarningsAsErrors`, `EnableWarning`, and `DisableWarning` in the options of a target with `ErrorKind::InvalidArgument`, because the recorded Slang never reads them there (SLC-6).
- `slango`: A high-level convenience library, with a feature set like SlangPy and design patterns like vulkano. Its only runtime backend today is wgpu, behind the default Cargo feature `wgpu`, without which the crate does not compile (SLO-28 and SLO-29 of `docs/design/slango.md`). The accessors of `slango::wgpu` that return a wgpu object return an `Option`, so that a later backend is an additive change (SLO-31). M10 plans slang-rhi as an opt-in backend behind the Cargo feature `rhi`, with wgpu kept as the default backend, and later milestones add pipeline kinds on the slang-rhi backend only (Sections 15.13 to 15.21 of `docs/design/architecture.md`). It first offers a runtime call API driven by reflection, and its first milestone, M3, covers compute kernels compiled to SPIR-V only.

  In M3 it calls existing compute entry points only, which are functions marked `[shader("compute")]`, with bindings built from reflection. Generated kernels around plain Slang functions, broadcasting over array arguments, and differentiable functions belong to later milestones (Section 8.1 of the architecture design).

  In M3, `slango` uses wgpu 30 and loads every shader through wgpu's validated SPIR-V path, so it has no unsafe code. A load copies what the kernels need into a private, owned layout snapshot and releases every Slang object before it returns, so no Slang object outlives a load and the public types are `Send` and `Sync`. A call sets each parameter by name (SLO-1, SLO-2, SLO-5, SLO-6, and Section 9.1 of `docs/design/slango.md`).

  Since M4, it also draws into a texture with render pipelines built from existing vertex and fragment entry points, still with SPIR-V only and no unsafe code. A shader target configurable per backend is deferred with no milestone assigned (Sections 1.4 and 1.5 of `docs/design/slango.md`).
- `slango-macros`: A function-like procedural macro, `shader!`, of M6, that compiles a `.slang` file with Slang during compilation and generates typed Rust bindings to its compute kernels. The generated code embeds the precompiled SPIR-V and layout snapshot and calls the same core of `slango` as the runtime API through a hidden module of `slango`, and the generator is in the library crate `slango-codegen` (SLM-1, SLM-6, and Section 7 of `docs/design/slango-macros.md`). Every crate that uses it needs Slang at build time, and `slango` does not re-export it in M6 (Section 4.1 of that design). The feature `vendored` of `slango` does not reach the copy of Slang that the macro links, so a program without a Vulkan SDK at build time enables `vendored` on `slango-macros` as well (Section 4.3 of that design).
- `slango-codegen`: The library crate of M6, to be published, that holds the generator of `slango-macros`, so that its pure tests run under Miri (SLM-1 of `docs/design/slango-macros.md`).
- `slang-slint`: An extension to Slint that provides a view backed by a Slang render pipeline, built on `slango`, with wgpu only, permanently. In M5 it is a minimal view that hands a texture that `slango` draws to an ordinary Slint `Image` element, on a narrow range of Slint versions with wgpu 30 only. Slint creates the device from a configuration that `slang-slint` supplies (SLS-1, SLS-6, and SLS-9 of `docs/design/slang-slint.md`).

  The licence consequence of depending on Slint is in Section 4.2 of `docs/design/slang-slint.md`.
- `slang-bindgen`: The binding generator, a published library crate with a thin binary, which parses the Slang headers with libclang and is a build dependency of `slang-compiler-sys` (Section 6.2 of `docs/design/architecture.md`).
- The examples, which also serve as the integration testbed and are not published:
  - A command-line program that compiles and reflects Slang source.
  - A headless compute kernel, which later gains a broadcasting case and a differentiable function.
  - A windowed renderer with winit and shader hot reload.
  - A Slint application with a `slang-slint` view.
  - A headless compute kernel called through the typed bindings of `slango-macros`.
  - Planned examples, which Section 10 of `docs/design/architecture.md` lists with their milestones, including the path-tracing demo.

Bindings for slang-rhi, the crates `slang-rhi-sys` and `slang-rhi`, are planned for M9 (Section 15.12 of `docs/design/architecture.md`).

The binding generator produces the `#[repr(C)]` vtable structs, UUID constants, and thunks through which Rust calls the Slang interfaces in the style of the Component Object Model (COM). None of them is written by hand.

Every named C++ enumeration becomes a `#[repr(transparent)]` integer newtype with associated constants, never a Rust `enum`. The enumerators of an unnamed enumeration become constants of the integer typedef that a rule assigns.

No generated binding is kept in the repository or a package. Every build generates the bindings and the list of their C functions, which the native-symbol test of `slang-compiler-sys` uses, from the headers of the Slang that it links, so every build needs libclang and no list of generated names is kept by hand (ARCH-33). The bindings support 64-bit targets only, and `slang-compiler-sys` stops compilation on any other.

The generated declarations of Slang's functions use unwinding ABIs, `"system-unwind"` for vtable entries and `"C-unwind"` for C functions. A callback type and every function that Rust implements for a vtable stay non-unwinding, `"C"` or `"system"`, so that a Rust panic never unwinds into Slang (FFI-7 of the coding standard).

The crates target the Slang that ships in the latest stable LunarG Vulkan software development kit (SDK). The recorded Slang is that of Vulkan SDK 1.4.363.1, whose `slangc` reports `2026.17-25-gc7954ebd1`, a commit past the tag `v2026.17`. `slang-src` records its commit and build tag, and the CI configuration records the SDK version (Section 5 of `docs/design/architecture.md`).

No recorded version gates a build. The build records the build tag of the Slang that it links, `slang-compiler` compares the loaded Slang with that record at run time (ARCH-34), and the crates are written and tested against the recorded Slang only, so another Slang is best effort (ARCH-31).

Slang is obtained in exactly one of two ways (Section 4 of `docs/design/architecture.md`):

- By default, the dynamic route links the Slang of an installed Vulkan SDK at `VULKAN_SDK`.
- The opt-in Cargo feature `vendored`, which `slang-compiler-sys` has and which the crates between a user and it forward, selects the vendored route. It builds Slang from the source tree that `slang-src` clones and links it statically. `slang-src` has no environment variable of its own, and the clone and the built libraries live in the shared folder in the Cargo target directory, which every profile and every host and target unit of a project share (Section 4.3 of `docs/design/architecture.md`).

No environment variable selects the route, the build never falls back from one route to the other, and there is no prebuilt download. The recipes pass an explicit list of features and never `--all-features`. Only `test-vendored`, `check-vendored`, `doc-site`, the vendored CI jobs, and the documentation workflow build the vendored route, and `publish-dry-run` builds it for `slang-glslang-sys`, which always enables the feature on `slang-compiler-sys` (Section 3 of `docs/design/tooling.md`).

Development uses the latest stable Rust toolchain. The workspace declares the minimum supported Rust version (MSRV) 1.99 in `rust-version`, and `check-msrv` verifies it with cargo-hack. The nightly toolchain serves only checkers such as Miri and never builds the crates.

The crates use edition 2024 and Cargo resolver 3. Each crate has the licence that Section 13 of `docs/design/architecture.md` assigns to its kind: the licence of Slang for the `-sys` and `-src` crates, `GPL-3.0-or-later` for `slang-slint`, and `MIT OR Apache-2.0` for every other crate, including the examples package.

CI runs on GitHub Actions on hosted runners for Linux, Windows, and macOS, installs the Vulkan SDK version that the CI configuration records on all three, and runs the default test tier on all three. One job on each platform also builds and tests the vendored route (Section 7.4 of `docs/design/tooling.md`). It also runs the GPU tier on all three, with lavapipe on Vulkan on Linux, the Windows Advanced Rasterization Platform (WARP) on Direct3D 12 on Windows, and hosted Metal on macOS.

Tests that need a GPU adapter sit in a module named `gpu`, which runs on any adapter including lavapipe and WARP, or in a module named `hw_gpu`, which needs a hardware GPU and runs locally only. A test that finds no adapter fails and never skips silently.

The published crates are planned for crates.io in two releases, in M8 and M11 (Section 13 of `docs/design/architecture.md`). Nothing is published without the owner's explicit go-ahead.

docs.rs is not supported, because it builds without network access and the crates need libclang and either a Vulkan SDK or a clone of Slang. The workflow `.github/workflows/documentation.yml` builds the documentation of each release from its tag on the vendored route and hosts it on GitHub Pages, and the `documentation` field of each crate points there. Only that workflow deploys the site, because a site built elsewhere names the absolute build path of its machine (TOOL-49 of `docs/design/tooling.md`). Section 9 of that design defines the release procedure, in which only the owner publishes, with a token that never enters the repository, CI, or an agent's environment.

The development container provides this environment:

- Rust stable 1.99.1, the default toolchain, with Clippy, rustfmt, rust-src, and llvm-tools.
- A nightly toolchain with Miri and rust-src, used only for checkers.
- cargo-nextest, cargo-deny, cargo-insta, cargo-expand, cargo-semver-checks, cargo-hack, cargo-machete, cargo-llvm-cov, the bindgen command-line interface (CLI), typos, taplo, actionlint, `just`, pre-commit, shellcheck, `gh`, CMake, Ninja, and libclang.
- GitHub's Secure Shell (SSH) host keys in the known hosts file.
- The LunarG Vulkan SDK 1.4.363.1 at `$VULKAN_SDK`, which ships Slang. Its `slangc` reports `2026.17-25-gc7954ebd1`, the headers are under `$VULKAN_SDK/include/slang`, `libslang-compiler.so` is under `$VULKAN_SDK/lib`, and the pkg-config file is `slang-compiler.pc`. A login shell has `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH`.

  A process that no login shell started does not, until the next rebuild of the container image applies the fix that slang-rs-env already contains. The recipes that run compiled code which loads Slang supply that path on their own command line, so they do not depend on it.
- The Slang source tree at `/opt/slang`, at the same commit as the SDK's Slang, with every submodule checked out. It is source only and not built in place, and no recipe reads it.
- Two Vulkan devices: an NVIDIA RTX 4070 and lavapipe, a software implementation of Vulkan.
- The CUDA 13.5 toolkit.

Recipes run Clippy, rustfmt, cargo-nextest, cargo-deny, cargo-hack, cargo-machete, cargo-llvm-cov, typos, taplo, Miri, shellcheck, actionlint, and the custom checkers under `scripts/`. The recipes of the release also run `cargo package`, `cargo publish`, git, `gh`, and curl through their scripts. The recipe `check-semver`, which runs cargo-semver-checks, does not exist yet. The change that follows the first release adds it (TOOL-39 of `docs/design/tooling.md`).

CI installs the latest version of each tool on its runners, and the Vulkan SDK version that the CI configuration records. A tool version in CI can therefore differ from the one in the container, as Section 7.1 of `docs/design/tooling.md` states.

Section 4.8 of the coding standard lists what each tool checks, and its Section 1.1 records the versions that were verified. Section 10 of `docs/design/tooling.md` lists every requirement on the image, which slang-rs-env defines.

## Development Workflow

A change starts with the section of the design document that covers it: `docs/design/architecture.md` for the architecture and the roadmap, and the crate's own design document for the crate. Work on the code of a milestone starts only once the design document that Section 15 of the architecture design requires for it exists. When that document does not exist yet, writing it is the first task of the milestone.

Before it writes Rust, the agent reads the sections of the coding standard that Section 1.2 of that standard names. The agent makes the change in a worktree, as Hard Rule 11 requires, runs the checks in Validating Changes, and fixes whatever they report. The agent updates the documentation as Hard Rule 1 requires before it considers the change done.

`just hooks-install` installs the git hooks once per clone. At commit time, the hooks run these checks:

- The prose check on each changed Markdown file and each changed Rust file.
- The shell and commit message checks.
- The rustfmt check when a Rust file changes.
- Typos on every commit.
- The TOML format check when a TOML file changes.
- `check-workflows` and `check-workflow-pins` when a file under `.github/` changes.
- `check-doc-references` when a Rust, TOML, YAML, or JSON file, the `justfile`, a file under `scripts/`, or a Markdown file under `docs/` changes.
- General checks for valid YAML and JSON, file endings, trailing whitespace, merge conflict markers, private keys, and large files.

After a merge, the hooks run `just worktree-prune`.

CI runs the workflow `.github/workflows/ci.yml` on every push to `main`, every push to a branch that matches `feature/**`, and every pull request. A new push to the same feature branch or pull request cancels the earlier run, and no run on `main` is cancelled, so every commit on `main` keeps its own result. Run 43717303370 on 2026-11-15 showed the cancellation on a feature branch, so the agent reads the run of the last commit, never an earlier one.

The second workflow, `.github/workflows/documentation.yml`, runs only on a push of a tag that matches `v*`, which only step 9 of the release procedure pushes (Section 9.6 of `docs/design/tooling.md`), so a push of a branch never starts it.

The project opens no pull requests, so a push of a feature branch starts one run. Every job makes libclang available, every job except `vendored` installs and checks the Vulkan SDK before its own steps, and every check in every job is a `just` recipe, so a failure in CI has the same command to reproduce it in the container. Sections 7.1, 7.3, and 7.4 of `docs/design/tooling.md` specify the jobs, their steps, and the artifact of the job `coverage`.

Commits are small, and each holds one group of logically related changes, because a later reader reads history one commit at a time. The subject line is in the imperative mood, such as `Add the safe compiler session`, and the body says why the change was made.

`just merge BRANCH` squash-merges the branch onto `main` from the main checkout with the subject and body of every commit in the message. It then removes the worktree and the branch, pushes `main`, and deletes the branch on `origin` when its tip there is part of the squash.

`just worktree-prune` removes branches that were squash-merged some other way. Agents never force-push.

The agent verifies a branch in CI before it merges the branch, with these steps, which Section 14.7 of `docs/design/tooling.md` specifies:

1. It pushes the branch with a plain push, which starts one run for the last commit of the branch.
2. It reads the run for that exact commit from the run itself, with these commands, in which `<branch>` and `<id>` are placeholders. The first prints the identifier, status, conclusion, and commit of the latest run of the branch, and the second prints each job of the run with its conclusion:

   ```sh
   gh run list --branch <branch> --limit 1 --json databaseId,status,conclusion,headSha
   gh run view <id> --json jobs
   ```

3. It runs `just merge <branch>` only when the run's `headSha` is the last commit of the branch, its conclusion is `success`, and every job succeeded.

The agent reads the run itself and never the exit status of a command whose output is piped into another program. The exit status of a pipeline is the status of its last program, so it says nothing about the run, which is how the failing Windows job that Project Status records reached `main`.

GitHub sometimes cancels a job before any step runs, with the annotation "The job was not acquired by Runner of type hosted even after multiple attempts". That is a problem of runner capacity, not a failure of the change, so the agent reruns the job and waits for the rerun (Section 14.4 of `docs/design/tooling.md`).

`just merge` builds the message of the squashed commit from every commit of the branch. A branch with several work-in-progress commits is therefore merged through a fresh branch.

The agent creates a new branch from `main` that holds the identical tree of the verified branch as one commit with the final message, and runs `just merge` on the fresh branch. The fresh branch holds the tree that CI verified only when the verified branch contains the current `main`.

No pushed history is rewritten. The branch that was pushed for CI then stays on `origin`, because `just merge` deletes only the merged branch there, and `just worktree-prune` removes its local branch and worktree.

The `review` skill runs the code-review workflow on a branch, writes its report under `.code-review/`, and reads this file as the project rules. The workflow does not find design documents under `docs/design/` by itself, so the agent passes the path of the design document as an explicit argument, such as `docs/design/<name>.md`.

A reviewer checks judgement rules only, as Hard Rule 4 requires. The agent MUST fix every finding labelled Important before the merge, because such a finding is a defect that would reach `main`. The agent defers the nits and lists them in its report.

## Validating Changes

The agent runs the checks that match what changed, and `just check` before every merge. Section 3.2 of `docs/design/tooling.md` lists every recipe, and its Section 14.4 says what a failure of each one means and what to do.

To run every check:

```sh
just check
```

`just check` runs `check-prose`, `check-shell`, the self-test of each checker, the self-test of the release scripts, `check-typos`, `check-toml`, `check-workflows`, `check-workflow-pins`, `check-doc-references`, and `check-rust`, in that order, and Section 3.3 of `docs/design/tooling.md` lists every recipe in it. It needs network access, because `check-deny` fetches the advisory database on every run, the nightly toolchain, because `check-miri` runs Miri on it, and a Vulkan SDK at `VULKAN_SDK` and libclang, because the build script of `slang-compiler-sys` generates the bindings from the SDK's headers with libclang and links its Slang. The development container provides the nightly toolchain, the SDK, and libclang.

To run only the Rust checks, which are `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs`, `check-slang-call-guard`, `check-fmt`, `check-unused-deps`, `check-deny`, `check-clippy`, `check-doc`, `check-doctest`, `test-gpu`, and `check-miri`, in that order:

```sh
just check-rust
```

To format every Rust file and every TOML file, which fixes what `check-fmt` and `check-toml` report, run this recipe. `fmt` and `check-fmt` pass every Rust file that git tracks or does not ignore to rustfmt, including `route.rs` and `system.rs`, which `slang-compiler-sys` pulls in with `include!` and which `cargo fmt --all` alone does not read. Both fail with a message that names `git ls-files` when that command fails or lists no Rust file, so they run inside the git checkout:

```sh
just fmt
```

To run the lints after a change to Rust code or its API documentation:

```sh
just check-clippy
just check-doc
just check-doctest
```

These commands run the tests of each tier:

- `test` runs the tests that need no GPU adapter.
- `test-gpu` adds the tests that need any adapter, including lavapipe.
- `test-hw-gpu` runs every test, including those that need a hardware GPU.

No test of the `hw_gpu` tier exists yet, so `test-gpu` and `test-hw-gpu` run the same tests. Each of them, and `check-doctest` and `coverage`, puts `$VULKAN_SDK/lib` on the library path of Linux or macOS on its own command line, so the test binaries find the Slang shared library without any setting of the shell:

```sh
just test
just test-gpu
just test-hw-gpu
```

The tests of `slang-bindgen` in modules named `ffi` load libclang, which the development container provides, and the tests of its binary also need `VULKAN_SDK`. They fail, never skip, without them. These tests compile for Linux only, so they do not run on Windows or macOS.

The tests in the module `ffi` of `slang-compiler-sys`, which are the smoke test and the native-symbol test, call Slang, so they need `VULKAN_SDK` and the library path that the recipes supply, and they run on Linux, Windows, and macOS. The tests in the module `build_script::ffi` load libclang, start child processes, and write files, and they also run on all three. Their test of the lookup of the Microsoft compiler finds a compiler only on Windows with the feature `vendored`, so only a vendored run on Windows exercises the lookup itself.

The tests in the module `ffi` of the test binary `compiler` of `slang-compiler` call Slang as well, so they also need `VULKAN_SDK` and the library path that the recipes supply, and they run on all three. One of them, the test that blobs and metadata outlive every other object, overwrites freed heap blocks only on Linux with the GNU C library, so it is weaker on Windows and macOS.

No recipe regenerates or checks generated files, because none is checked in. A change to `slang-bindgen` or its rule set takes effect in the next build of `slang-compiler-sys`, and every check that compiles the workspace compiles the new output. To read the bindings that a build compiles, run `check-slang-call-guard`, which writes them to `target/slang-bindgen/bindings.rs` for the host target (SCS-60 and Section 7.6 of `docs/design/slang-compiler-sys.md`).

To check that the Vulkan SDK at `VULKAN_SDK` provides the Slang headers, library, and `slangc` of the build tag that `recorded-slang` prints, which every CI job except `vendored` runs after it installs the SDK and which also works in the development container:

```sh
just check-vulkan-sdk
```

The recipe `install-vulkan-sdk` is for CI only. It refuses to run outside a GitHub Actions runner, and the agent does not run it.

To lint, document, and test the vendored build after a change to `slang-src`, to its recorded commit, to the build script of `slang-compiler-sys` or `slang-glslang-sys`, to the loader, to the code of `slang-compiler` under its feature `vendored`, to a forward of the feature, or to the vendored recipes, run these recipes. They build `slang-src`, `slang-glslang-sys`, `slang-compiler-sys`, and `slang-compiler` with the feature `slang-compiler/vendored`, which the `justfile` variables `vendored_packages` and `vendored_features` name. `check-vendored` runs Clippy and the documentation build, and then Clippy on `slang-slint` with only `slang-slint/vendored`. `test-vendored` runs the tests and the doctests, and then the tests of the modules `bindings` and `structs` of `slango-macros` with only `slango-macros/vendored`. Each forward gets its own command, because Cargo unifies the features of the packages of one command (TOOL-7 of `docs/design/tooling.md`). Both recipes build in `target/vendored` with `VULKAN_SDK`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` removed, and fail on any warning.

Their first run in `target/vendored` clones the recorded Slang commit and builds a minimal static Slang and then slang-glslang from source, so it needs git, network access, libclang, CMake, Ninja, a C++20 compiler, and Python 3, which the development container provides. On 2026-11-15 on 16 cores in the development container, after the fourth code step of M7, the first `check-vendored` in an empty `target/vendored` took 6 min 33 s, `test-vendored` right after it took 31.9 s, and repeats took 0.45 s and 5.3 s. In an earlier measured run of the same step, the clone took 38 s, the build of Slang 3 min 48 s, and the build of slang-glslang 1 min 42 s after it. The shared folder then held 364 MB: the clone of 285 MB, the entry of `slang-compiler-sys` of 57 MB, and the entry of `slang-glslang-sys` of 23 MB.

Later runs use the clone and the entries in the shared folder `target/vendored/slang-src` without network access until `target/vendored` is removed. A change of the compiler, of a variable that CMake reads, or of the recorded commit builds Slang and slang-glslang again, and a change of the wrapper project or the loader shim builds slang-glslang again. `just check` runs neither recipe, and no recipe that it runs enables the feature `vendored` or builds `slang-src` or `slang-glslang-sys`, so `just check` never clones and never builds Slang or slang-glslang:

```sh
just check-vendored
just test-vendored
```

To check, after either recipe, that the clone sits in the shared folder `target/vendored/slang-src` and in no other directory under `target/vendored`, and to print where it is and its size, run this recipe. It builds nothing and needs no network access, and `just check` does not run it:

```sh
just check-vendored-shared-folder
```

To print the recorded Slang commit and build tag of `crates/slang-src/recorded.rs` as `NAME=value` lines:

```sh
just recorded-slang
```

To check the dependencies after a change to a manifest:

```sh
just check-deny
just check-unused-deps
```

To run the tests under Miri after a change to unsafe code, run this recipe. Miri cannot call native code or start a process, so the profile `default-miri` skips every test in a module named `ffi`, `gpu`, or `hw_gpu`. Miri runs the pure tests of `slang-bindgen`, of the build information, and of the logic of both routes of the build script against its fake environment.

Miri also runs every unit test of `slang-compiler` outside the module `guard::tests::ffi`, whose tests start a child process. These test the owning interface pointer, the Slang call guard, the global session, the session, the compiler options, modules, component types, blobs, and metadata against the fake of Slang's interfaces in `crates/slang-compiler/src/fake.rs`.

The fake frees each object when its count reaches zero, so that Miri detects a use after free. The build script still runs natively, so the recipe needs `VULKAN_SDK` and libclang:

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

To check the comments, the headings of item documentation, the doctest attributes, the feature documentation, and the guard around every call of `slang-compiler` into Slang after a change to Rust code or its API documentation, run these recipes. Each also takes the paths of files to check, and `check-feature-docs` takes the paths of Cargo manifests. With no paths, `check-slang-call-guard` checks every Rust file under `crates/slang-compiler/src`. It first runs the binary of `slang-bindgen` on `$VULKAN_SDK/include/slang` for the host target into `target/slang-bindgen/`, so it needs `VULKAN_SDK` and libclang, and it stops with exit status 2 and a message when `VULKAN_SDK` is unset or empty. The checker then reads the names of Slang's functions from `target/slang-bindgen/bindings.rs`:

```sh
just check-rust-comments
just check-doc-headings
just check-doctest-attributes
just check-feature-docs
just check-slang-call-guard
```

To check a workflow after a change under `.github/`:

```sh
just check-workflows
just check-workflow-pins
```

To check that code and configuration cite no project document, section, or rule identifier, run this recipe after a change to code, configuration, a script, the `justfile`, or a rule table of a document. It also takes the paths of files to check. With no paths, it checks every Rust, TOML, YAML, and JSON file, the `justfile`, and every file under `scripts/`:

```sh
just check-doc-references
```

To run the self-test of a checker after a change to the checker, to `scripts/rust_lexer.py`, or to its cases under `scripts/fixtures/`:

```sh
just check-prose-self-test
just check-rust-comments-self-test
just check-doc-headings-self-test
just check-doctest-attributes-self-test
just check-feature-docs-self-test
just check-slang-call-guard-self-test
just check-workflow-pins-self-test
just check-doc-references-self-test
```

To check a shell script, or a recipe in the `justfile`:

```sh
just check-shell
```

To check `scripts/check-commit-message`:

```sh
just check-commit-message-self-test
```

To run the self-test of the release scripts after a change to a script of the release, to `scripts/self-test-release`, or to a recipe of the release, run this recipe. It runs against stub programs, needs no network access, and `just check` runs it. It needs `just` installed anywhere on `PATH`, and the programs that Section 9.9 of `docs/design/tooling.md` lists in `/usr/bin` or `/bin`:

```sh
just release-scripts-self-test
```

The recipes of the release procedure, which Section 9.6 of `docs/design/tooling.md` orders, are outside `just check`. An agent runs them in a release, and after a change that they check:

- `check-package` after a change to a manifest, to `release_packages` or `documentation_site`, to a licence file, or to `README.md`. Cargo refuses a packaged file with an uncommitted change, so it runs only after the commit. It runs on Linux and macOS only and reaches `static.crates.io`.
- `publish-dry-run` after the same changes, on a committed tree as well, on Linux and macOS only. It needs `VULKAN_SDK`, libclang, network access, and the tools of the vendored build, clones and builds Slang in `target/`, took about 12 minutes in the development container, and writes the record that `publish` checks.
- `doc-site` after a change to the API documentation of a crate of the release, or to the recipe. It needs what `check-vendored` needs and builds in `target/vendored`. Its site is for reading only, and an agent never deploys it (TOOL-49 of `docs/design/tooling.md`).
- `check-release` after a release, to check it. It reaches crates.io and the hosted site, and fails before the first release, because crates.io holds none of the crates.

```sh
just check-package
just publish-dry-run
just doc-site
just check-release
```

An agent MUST NOT run `publish` or `yank`, which only the owner runs, with a token (TOOL-46 of `docs/design/tooling.md`), and MUST NOT run `tag-release` without the owner's explicit go-ahead in the current session (TOOL-44). Each of them acts in public, and a published version can never be overwritten or deleted.

Three more recipes are not part of `just check`, because they are meant for CI. CI runs all three in every run of the workflow.

- `check-features` compiles the workspace once for each combination of Cargo features, without the feature `vendored` and without the build of `slango` with no features, which stops on purpose with a compile error (TOOL-48 of `docs/design/tooling.md`).
- `check-msrv` checks the workspace with the toolchain that `rust-version` names, and its first run installs that toolchain with rustup, which needs network access.
- `coverage` writes a coverage report to `target/lcov.info` and prints no coverage summary, and the CI job `coverage` uploads that report as an artifact.

An agent runs `check-features` after a change to the Cargo features and `check-msrv` after a change that may need a newer toolchain:

```sh
just check-features
just check-msrv
just coverage
```

## Documentation Maintenance

Hard Rule 1 is the rule an agent is most likely to skip, because the code works without it. After every change, and before it reports the change complete, the agent MUST re-read and correct these documents, once each, because a stale document misleads the next agent:

1. The design document of each crate or tool that the change alters, including its design content, its assumptions and open questions, and, for `slang-compiler`, its list of known upstream bugs, together with the entry in `docs/upstream-reports.md` of each defect in it that should be reported. Progress and status are recorded only in Project Status of this file, never in a design document (Section 3.6 of the writing guide).
2. Every statement in another document that the change makes false, which the agent finds by searching for the names that the change touches. `README.md`, the architecture design, the tooling design, and this file change only as Hard Rule 1 states. `README.md` is also the crates.io readme of every published member.
3. `docs/rust-coding-standards.md` with its Appendix A, `docs/rust-references.md`, or `docs/writing-documentation.md`, only when the change alters a rule, a lint, a tool, a source, or a documentation convention.
4. The Date field of the metadata table of each document the change touched.

## Contribution Policy

The project is developed by its owner with coding agents, under the rules in this file. It has no contribution policy yet.

The project opens no pull requests for now, and a pull request from a fork runs CI with a read-only token and no secrets. A future contributing policy may require disclosure of artificial intelligence (AI) usage in pull request descriptions.

## References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.
