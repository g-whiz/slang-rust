# Agent Instructions

Every coding agent reads this file before it works in this repository. `CLAUDE.md` imports it, so Claude Code loads the same instructions.

The repository is meant to hold an idiomatic Rust library for the Slang shader language and runtime. It is at a very early stage. It holds these parts:

- Standards documents, design documents, repository tooling, and the configuration of the Rust checks.
- The binding generator, the publishable crate `slang-bindgen`.
- The crate `slang-compiler-sys`, whose build script generates the raw bindings from the headers of the Slang that it links and records that Slang in the constant `BUILD_INFO`. By default it links the Slang of an installed Vulkan SDK dynamically, and with its Cargo feature `vendored` it builds Slang from source and links it statically.
- The crate `slang-src`, which clones the recorded Slang commit for a vendored build, and which `slang-compiler-sys` depends on with the feature `vendored` and `slang-glslang-sys` always depends on.
- The crate `slang-glslang-sys`, which builds slang-glslang from the same tree for a vendored build and holds the in-process loader through which a statically linked Slang optimizes SPIR-V.
- The compile and reflection API of the safe crate `slang-compiler`, which can compile a shader and reflect its parameters, as Project Status describes. With its feature `vendored`, it installs the loader of `slang-glslang-sys` on every global session that it creates.
- The crate `slango`, which runs compute kernels and draws with render pipelines through wgpu by default, or through slang-rhi with its Cargo feature `rhi`.
- The crate `slang-slint`, which shows what `slango` draws in a Slint user interface.
- The procedural macro crate `slango-macros` with its generator crate `slango-codegen`, which generate typed bindings to the compute kernels of a Slang file.
- The crate `slang-rhi-sys`, which builds slang-rhi, the render hardware interface of Slang, from the same tree and generates its raw bindings, and the safe crate `slang-rhi`, which runs compute kernels and draws with render pipelines through slang-rhi with Vulkan on Linux and Direct3D 12 on x86_64 Windows.
- Five examples.

Development happens inside the development container that the sibling repository slang-rs-env defines.

Best Current Practice (BCP) 14 defines the key words in the next paragraph.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

## Hard Rules

1. **Documentation changes with the code, once.** In the same change as the code, the agent MUST update the design document of each crate or tool that the change alters, and every statement in another document that the change would otherwise make false. The next agent reads the documentation as true and builds on it, so a false statement misleads it.

   `AGENTS.md`, the architecture design, the tooling design, and `README.md` change only for such a statement, or when the change alters a rule, the repository layout, or what a user can do.

   The agent MUST NOT mirror a fact into several documents, because every copy can diverge. It states the fact once and cites that place. Documentation Maintenance lists what the agent re-reads.
2. **Documentation follows the writing guide.** The agent MUST write every document, and all application programming interface (API) documentation written with rustdoc, in accordance with `docs/writing-documentation.md`, because the agents and users that read a document depend on its conventions to interpret it. This includes the metadata rules for design documents and the API documentation rules of Section 7 of the writing guide. This file is an instruction file under Section 3.4 of that guide, so it has no metadata table or abstract, and its unnumbered headings are cited by name. Every document and all API documentation also follow `docs/avoiding-claudelish.md`, and every README also follows `docs/writing-readmes.md`.
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

   Unsafe code appears only in `slang-compiler-sys`, `slang-compiler`, and `slang-glslang-sys`, which declares the one foreign function that returns its loader (SGL-13 of `docs/design/slang-glslang-sys.md`), and in the crates `slang-rhi-sys` and `slang-rhi` of M9, whose design documents record the reason (RSYS-2 of `docs/design/slang-rhi-sys.md`, RHI-2 of `docs/design/slang-rhi.md`). `slang-rhi-sys` holds only its generated bindings, and `slang-rhi` holds its unsafe code in four private modules and in the module of each wrapper, as RHI-2 states. Any other crate that needs it records the reason in its design document first, as Section 4.4 of the coding standard states.

   Generated bindings, vtables, universally unique identifier (UUID) constants, thunks, and the generated list of C functions are never written by hand and never checked in. The build scripts of `slang-compiler-sys` and of `slang-rhi-sys` generate them into `OUT_DIR` in every build, for Slang and for slang-rhi, so the agent changes them only by changing the binding generator or its rule sets (FFI-1, COM-9, ARCH-33 of `docs/design/architecture.md`).

   No file in the repository contains a secret. A build script installs nothing, writes only inside `OUT_DIR` or the shared folder in the Cargo target directory of the build, and reaches no network, except the build script of `slang-src`, which clones the recorded commit of the Slang tree by its full hash (SYS-5). A continuous integration (CI) workflow verifies the SHA-256 checksum of every download whose upstream publishes one.
8. **Every action is a `just` recipe.** Every command, build step, and check is a recipe in the root `justfile`, which is one flat file with `[group]` attributes and no `just` modules, because modules hide their recipes from `just --list`. The agent invokes no script under `scripts/` directly when a recipe wraps it.

   Every recipe has a help comment, and every shell script starts with `#!/usr/bin/env bash` and `set -euo pipefail`. Every shell script and every recipe that contains shell is `shellcheck`-clean. Section 3.2 of `docs/design/tooling.md` lists every recipe, and Validating Changes names the recipes to run for each kind of change.
9. **No attribution.** No output contains a generated-by line, a `Co-Authored-By` trailer, or a link to an agent session, because such a line says nothing about the change and a session link can expose a private session. This covers commits, pull requests, issues, and files. The `commit-msg` hook rejects such lines, and `just merge` strips them.
10. **No extra documentation.** The agent writes no document beyond the root `README.md`, the `README.md` of each library crate, `AGENTS.md`, `CLAUDE.md`, and the `docs/` tree, with design documents in `docs/design/`, because every extra document is one more place that can diverge from the code.
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

   The agent MUST record each defect once, in the list of known upstream bugs in Section 3.12 of `docs/design/slang-compiler.md`, or in Section 3.1 of `docs/design/slang-rhi.md` for a defect of slang-rhi, with the smallest known input, the call, the observed outcome, and the source location if already known, so that a later investigation can start from it. The agent MUST NOT investigate a defect further unless it blocks the work.

   The agent MUST add an entry for each defect that should be reported to the to-do list of upstream reports, `docs/upstream-reports.md`, in the same change that records the defect, because that list is the one place where the owner finds the reports to make. Section 2 of that list gives the rules for an entry.

   The agent MAY draft a bug report in that list as low-priority work and MUST NOT post it without the owner's explicit go-ahead in the current session, because a report speaks for the project in a public repository.
15. **Code cites no project document.** The agent MUST NOT cite, in a code file or a comment, a project document, which is a Markdown file under `docs/`, a section of one, or a rule identifier that a project document defines, such as `SLC-32` or `FFI-7`. A reader of the code or of a published crate sees it without the repository's documents, and a citation breaks when a document changes.

   The rule covers API documentation, ordinary comments, the `reason` strings of lint exceptions, error and panic messages, recipe help comments, script usage text, and comments in configuration files. Code also MUST NOT hold project history wording, such as "this version", a milestone, or a step.

   Where a citation would have carried a reason, the code MUST state the reason itself in plain words. The documents cite the code, never the reverse.

   The checker `check-doc-references` reports the path or file name of a project document, a section cited by number, and a rule identifier that a project document defines. `just check` and the `pre-commit` hook run it. No tool checks history wording, so that part stays a judgement rule.

## Project Status

As of 2026-11-22, the repository is at a very early stage. The agent updates this section, and its date, in the same change that alters what exists. Section 15 of `docs/design/architecture.md` defines each milestone and its exit criteria.

- M0, tooling and skeleton, is complete.
- M1, the binding generator and `slang-compiler-sys`, is complete. It delivered the pin file, the binding generator, the checked-in bindings, the build script with its dynamic and static routes, the installation of the pinned Vulkan SDK in every CI job, and the CI job `coverage`. What stays open after M1 is in the rows of Sections 15 and 16 of `docs/design/slang-compiler-sys.md`.
- M2, the safe crate `slang-compiler` and the compile-and-reflect example, is complete on 2026-11-11. A user can compile a shader, reflect its parameters with their types, layouts, and bindings, and run the example `compile-and-reflect`. The custom file system is deferred with no milestone assigned, as What does not exist yet states.
- M3, the crate `slango` and the compute kernel example, is complete on 2026-11-11. A user can load a Slang module, run a compute kernel with its parameters set by name, read its results, and run the example `compute-kernel`.
- M4, the rendering of `slango` and the windowed renderer example, is complete on 2026-11-11. Its design is also in `docs/design/slango.md`. A user can also draw with a vertex and a fragment entry point into a texture or a window, and run the example `windowed-renderer`.
- M5, `slang-slint` and the Slint example, is complete on 2026-11-11. A Slint program can make Slint create a wgpu device that suits `slango`, turn that device into a `slango::Device`, and show what a `slango` render pipeline draws as a Slint image. A user can run the Slint example `shader-playground`.
- M6, `slango-macros` and its generator `slango-codegen`, is complete on 2026-11-13. `slango` checks the element type of a bound buffer (Section 9.3 of `docs/design/slango.md`). A crate can call `slango_macros::shader!` on a Slang file to get a typed kernel for each compute entry point and a Rust struct for each Slang structure that the kernels take, and a user can run the example `typed-kernel`.
- M7, the rework of the native crates, is complete on 2026-11-15. Every build generates the Slang bindings with libclang and records the Slang that it links in `BUILD_INFO`. A program can enable the Cargo feature `vendored` on `slang-compiler`, `slango`, or `slang-slint`, and on `slango-macros` when it calls the macro, to build Slang and its SPIR-V optimizer from source and link them statically without a Vulkan SDK, as the crate-level documentation of `slang-compiler-sys` describes.
- M8, the forward-compatibility pass of `slango` and the first release, is complete on 2026-11-17. The wgpu backend of `slango` sits behind the default Cargo feature `wgpu`, and the eight crates of the first release are on crates.io at `0.1.0-alpha.1`, with their documentation on GitHub Pages. The recipe `check-semver` and the CI job `semver` check the crates against that release.
- M9, `slang-rhi-sys`, `slang-rhi`, and additive reflection, is complete on 2026-11-18. A program on Linux with the GNU C library or on x86_64 Windows can run a compute kernel through slang-rhi with the safe crate `slang-rhi`, reach the rest of slang-rhi through the raw bindings of `slang-rhi-sys`, and reflect functions, attributes, modifiers, and generics with `slang-compiler`. Both new crates belong to the second release.
- M10, the opt-in backend `rhi` of `slango`, is complete on 2026-11-19. A program can turn off the default features of `slango` and enable `rhi` to run compute kernels, call `shader!`, and draw with render pipelines through slang-rhi on Vulkan on Linux and on D3D12 on x86_64 Windows, with the public API of the backend `wgpu` apart from `slango::wgpu`, and the crate-level documentation of `slango` tells a user how to choose a backend and how to move from the published release `0.1.0-alpha.1`.

On 2026-11-15, the owner replaced the roadmap after M6 with milestones M7 to M18, which Section 15 of `docs/design/architecture.md` defines. The design of M7 is in `docs/design/slang-compiler-sys.md`, `docs/design/slang-src.md`, `docs/design/slang-glslang-sys.md`, and the documents that they changed. Five steps delivered M7, all on 2026-11-15, and What exists and works describes what they delivered:

- The first added the crate `slang-src`, its recipes, and the CI job `vendored`.
- The second moved the binding generator to the publishable crate `slang-bindgen` and made the build script of `slang-compiler-sys` generate the bindings and the list of C functions into `OUT_DIR` in every build and record the build tag of the Slang that it links in `BUILD_INFO`. It removed the pin file, the checked-in generated files, the static route with `SLANG_SOURCE_DIR`, every `DOCS_RS` code path, every `[package.metadata.docs.rs]` table, the recipes `pin`, `generate-bindings`, `check-bindings`, `test-static`, and `check-doc-without-slang`, and `--all-features` from the recipes.
- The third added the Cargo feature `vendored` of `slang-compiler-sys`, with the vendored route of its build script, which builds a minimal static Slang from the tree of `slang-src`, keeps the built libraries in the shared folder, finds the Microsoft C++ compiler itself on Windows, and records the static link in `BUILD_INFO`. It also made the vendored recipes and the CI job `vendored` build and test that route.
- The fourth added the crate `slang-glslang-sys`, which builds slang-glslang as static libraries from the tree of `slang-src` and holds the in-process loader, and the feature `vendored` of `slang-compiler`, which installs that loader on every global session that the crate creates. It made `slango`, `slango-codegen`, `slango-macros`, `slang-slint`, and the examples package forward the feature. It moved the vendored recipes to `slang-compiler/vendored` with two extra commands that check the forwards, added the Python 3 step to the CI job `vendored`, and made the vendored route of both native crates link the system libraries that Slang's archives and slang-glslang need.
- The fifth completed the user guide to the vendored build, which the root `README.md` held until 2026-11-17, and the crate-level documentation. It covers which crate takes the feature and why `slango-macros` needs it as well, what the build needs and costs, the shared folder, which Slang a build gets and how `BUILD_INFO` shows it, the platforms, and the common failures. It closed the milestone.

The design of M8, the forward-compatibility pass of `slango` and the first release (Section 15.11 of `docs/design/architecture.md`), is in these documents:

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
- Every member got the version `0.1.0-alpha.1`, and `slang-src` got `0.1.0-alpha.1.slang.2026.17.0.25.gc7954ebd1`, whose test now requires its pre-release to end with the Slang identifiers (SRC-13 of `docs/design/slang-src.md`). Every requirement between members is exact.
- `tag-release` passes `--prerelease` to `gh release create` when the version holds a hyphen, so the tag of the first release is `v0.1.0-alpha.1` and its GitHub release is a pre-release.
- The section `Status` of the root `README.md` states that the API is unstable and that a dependent names the full pre-release version.

On 2026-11-17, the owner set the source of GitHub Pages to "GitHub Actions" and added the tag rule `v*` to the environment `github-pages`, which is step 6 of Section 9.6 of `docs/design/tooling.md`. The GitHub API then reported the build type `workflow`, the address `https://g-whiz.github.io/slang-rust/`, and two deployment policies, the branch `main` and the tag `v*`. At that point, nothing was deployed, published, or tagged.

On 2026-11-17 in the development container, these checks passed on the code of the step with every requirement between members exact, before the last edits of its documents:

- The recipes `check-rust`, which printed "966 tests run: 966 passed, 0 skipped" and, under Miri, "557 tests run: 557 passed (32 slow), 409 skipped", and `release-scripts-self-test`, which printed "release self-test passed: 58 cases", together with `check-prose`, `check-shell`, `check-typos`, `check-toml`, `check-workflows`, `check-workflow-pins`, and `check-doc-references`. The self-tests of the checkers did not run again, so this is not the whole `just check`.
- `check-features`, `check-msrv`, `check-vendored`, and `check-clippy` with Rust 1.100.0.
- `test-vendored`, which printed "422 tests run: 422 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".

`check-package` and `publish-dry-run` did not run, because Cargo packages only a committed tree.

On 2026-11-17, the owner ran `just publish` for the eight crates at the commit "Release every crate as 0.1.0-alpha.1 with exact requirements", with the script as it was before the fifth code step. crates.io refused the sixth new crate for its rate limit, which allows a burst of five new crates and then one every ten minutes (Section 9.7 of `docs/design/tooling.md`).

The crates.io API afterwards showed these crates at their alpha versions:

- `slang-bindgen`, `slang-src`, `slang-compiler-sys`, `slang-glslang-sys`, and `slang-compiler`, which that run published.
- `slango`, which the owner published later the same day.

At that point, `slango-codegen` and `slango-macros` were not published, no tag existed, and nothing was deployed.

On `main` before the fifth code step, `just publish-dry-run` with the eight crates passed while six of the versions were on crates.io. Cargo printed the warning "already exists on crates.io index" for each of the six and went on. `just publish-dry-run slango-codegen slango-macros` passed as well.

The fifth code step of M8, done on 2026-11-17 at the owner's request, made `publish` comply with the rate limits of crates.io and safe to run again after a partial release (TOOL-51 of `docs/design/tooling.md`). It changed `scripts/publish`, its cases in `scripts/self-test-release`, and the help comment of the recipe:

- `publish` asks crates.io for each version before it publishes anything, skips every version that crates.io holds, and publishes the others with one run of Cargo each, in the given order.
- It waits once this run has used up the burst of a kind, waits and retries after a refusal for the rate limit, and confirms every failure of Cargo against crates.io.
- When it stops, it names the crates published in this run, those already on crates.io, and those not published.

On 2026-11-17 in the development container, `just check` passed. It printed "release self-test passed: 78 cases", against 58 before, "966 tests run: 966 passed, 0 skipped", and, under Miri, "557 tests run: 557 passed (15 slow), 409 skipped".

An adversarial review ran the script against stub programs and found no leak of the token and no way to publish a version twice. It found defects, which the step fixed:

- The match of a refusal also matched a page of status 429 of the index that Cargo can print after an upload, so the match now accepts only an error of the API.
- A failure of Cargo after an upload counted as a failure, so every failure is now confirmed against crates.io.
- The wait had no upper bound and a lower bound of 1 s, curl had no timeouts, an interrupt printed no report, and the expansion of an empty array was not safe under `set -u` on bash older than 4.4.

Not verified are a real refusal for the rate limit through the real Cargo, whose error the script matches by a form that a reviewer read in the source of Cargo on its main branch and not in the installed Cargo 1.99.1 (TA-22 of `docs/design/tooling.md`), and the script on bash older than 4.4 and with the `date` of macOS (TA-23).

Later on 2026-11-17, the owner published `slango-codegen` and `slango-macros`, also with the script as it was before the fifth code step. The reworked script of the fifth code step published nothing in this release.

The crates.io API then showed all eight crates of the first release, none yanked. `slang-bindgen`, `slang-compiler-sys`, `slang-glslang-sys`, `slang-compiler`, `slango`, `slango-codegen`, and `slango-macros` are at `0.1.0-alpha.1`, and `slang-src` is at `0.1.0-alpha.1.slang.2026.17.0.25.gc7954ebd1`.

With the owner's go-ahead, `just tag-release` then tagged the published commit of `main` as `v0.1.0-alpha.1`, pushed the tag, and created the GitHub release, which GitHub reports as a pre-release. The push of the tag started the documentation workflow for the first time, and its run 44024668235 concluded `success` with both jobs, "Build the documentation site" and "Deploy the documentation site", succeeded.

`just check-release` then exited with status 0. It printed, for each of the eight crates, that its version is on crates.io and that its documentation address answers, and ended with "check-release: crates.io holds every package, and each documentation address answers". The site is at `https://g-whiz.github.io/slang-rust/`.

The sixth code step of M8, done on 2026-11-17, is the change that follows the release (step 11 of Section 9.6 of `docs/design/tooling.md`). It closed the milestone:

- Every member except `slang-src` has the version `0.1.0-alpha.2`, and `slang-src` has `0.1.0-alpha.2.slang.2026.17.0.25.gc7954ebd1`. Every requirement between members is exact at the new version, and each example of the API documentation of `slang-src`, `slang-compiler-sys`, and `slango-macros` that named `0.1.0-alpha.1` now names the new version (TOOL-37). The published version stays `0.1.0-alpha.1`, which the root `README.md` keeps naming until the next release is published (Section 9.6 of `docs/design/tooling.md`).
- The recipe `check-semver` in the group `checks`, outside `just check`, and the CI job `semver`, which runs it on Linux in every run of the workflow (TOOL-38 and TOOL-39 of `docs/design/tooling.md`).
- A precondition of the release procedure on the version that each example of the crates names (Section 9.6 of `docs/design/tooling.md`), and TQ-18 of that design, which asks whether `check-package` should check it.

The step answered TQ-16 of `docs/design/tooling.md` by experiment, in Section 9.3 of that design. cargo-semver-checks 0.51.0 takes the step from one alpha to the next as a major change and runs none of its checks, so `check-semver` reports no change of the API while every release is an alpha. It also skips every crate without a library target, so it does not check `slango-macros`.

With `--release-type minor`, the tool does run its checks between two alphas and fails a breaking change with exit status 100. The recipe does not pass the option. Whether it should, with a rule that such a report does not fail the job, is TQ-17 of `docs/design/tooling.md`, which the owner has not decided.

On 2026-11-17 in the development container, with Rust 1.99.1 and cargo-semver-checks 0.51.0, these checks passed on the code of the step:

- `just check`, which printed "release self-test passed: 78 cases", "966 tests run: 966 passed (1 slow), 0 skipped", and, under Miri, "557 tests run: 557 passed (27 slow), 409 skipped".
- `check-features`, `check-msrv`, `check-vendored`, `check-workflows`, and `check-workflow-pins`.
- `test-vendored`, which printed "422 tests run: 422 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".
- `check-semver`, which took 105 s and printed "0 checks: 0 pass, 260 skip" and "no semver update required" for each of `slang-bindgen`, `slang-compiler`, `slang-compiler-sys`, `slango`, and `slango-codegen`.

The help comment of `check-semver` changed after `just check` ran, and `check-shell`, `check-doc-references`, `check-typos`, and `check-fmt` then passed again. `check-package` and `publish-dry-run` did not run, because Cargo packages only a committed tree.

When this was written, the CI job `semver` had not run on GitHub, so its installation of cargo-semver-checks rests on the manifest of the installer action alone (TA-20 of `docs/design/tooling.md`). Hard Rule 11 lets the step reach `main` only after its CI run succeeds.

Four open questions stay open after M8 with no milestone assigned: TQ-17 and TQ-18 of `docs/design/tooling.md`, SLMQ-2 of `docs/design/slango-macros.md`, which the first release did not decide, and CQ-5 of `docs/design/slang-compiler.md`, which waits for an answer from Slang. The questions of the review of the public API wait for the first release that is not an alpha, as the third code step states.

On 2026-11-17, after the sixth code step of M8 and before the publication of `0.1.0-alpha.2`, the owner set aside the earlier rules on READMEs. A change of that day added the guides `docs/writing-readmes.md` and `docs/avoiding-claudelish.md`, a README for each library crate, and the phrase check of `check-prose` with its list `scripts/prose-phrases.txt`. It also rewrote the root `README.md` and moved the user guide to the vendored build into the crate-level documentation of `slang-src` and `slang-compiler-sys`.

The change alters the packaged files of every crate of the release. At `0.1.0-alpha.1`, `check-package` failed on all eight with "crates.io holds 0.1.0-alpha.1, and the package differs from the published .crate file". The version `0.1.0-alpha.2` of the sixth code step carries the change. Each crate README names the version of its package, `0.1.0-alpha.2`, and the root `README.md` names the latest published version, `0.1.0-alpha.1`, as RDM-33 of `docs/writing-readmes.md` states. The change restated the precondition of the release procedure and TQ-18 of `docs/design/tooling.md` for this layout.

Three open questions that the designs assign to M7 stay open, because no exit criterion of M7 requires them: SRCQ-1 of `docs/design/slang-src.md`, SGLQ-2 of `docs/design/slang-glslang-sys.md`, and CQ-7 of `docs/design/slang-compiler.md`. The owner has not assigned them another milestone yet.

M9, `slang-rhi-sys`, `slang-rhi`, and additive reflection (Section 15.12 of `docs/design/architecture.md`), is complete, and its design exists. It consists of the new documents `docs/design/slang-rhi-sys.md` and `docs/design/slang-rhi.md`, Section 7.8 of `docs/design/slang-compiler-sys.md` for the generator, Sections 6.7, 9.6, and 9.7 of `docs/design/slang-compiler.md` for reflection, SRC-25 and the new required submodule of `docs/design/slang-src.md`, and TOOL-52 of `docs/design/tooling.md`, within the owner's decisions of 2026-11-17 (Section 4.1 of `docs/design/slang-rhi-sys.md`). Section 15.12 of the architecture design orders its seven code steps, and all seven are done. The open questions and unverified assumptions that stay open after M9 are listed after the seventh code step below.

The fourth code step of M9, done on 2026-11-18, added the additive reflection and the two accessors of Sections 9.6 and 9.7 of `docs/design/slang-compiler.md` to `slang-compiler`, and changed no other crate. Each `apply_specializations` returns `Result<Option<_>>` instead of the `Option` that the design first gave, so that a generic of another session fails with the kind `InvalidArgument`, and SLC-37 now states this.

The step confirmed two assumptions of that design, one by test and one by a reading of Slang's sources, and removed their rows. Section 3.9 of `docs/design/slang-compiler.md` records both results and the M9 part of CQ-13. The step met no defect of Slang. It raised CQ-14 of that design, on the panic message of the Slang call guard, which waits for the owner.

On 2026-11-18 in the development container, with Rust 1.99.1, `just check` passed. It printed "988 tests run: 988 passed (1 slow), 0 skipped" and, under Miri, "568 tests run: 568 passed (48 slow), 420 skipped".

`check-features`, `check-msrv`, `check-vendored`, `check-semver`, and `check-clippy` with Rust 1.100.0 passed. `test-vendored` printed "444 tests run: 444 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".

cargo-semver-checks with `--release-type minor`, the strict run that TQ-17 of `docs/design/tooling.md` describes, ran on `slang-compiler` against its last release on crates.io. It printed "202 checks: 202 pass, 58 skip" and found no breaking change. `check-package` and `publish-dry-run` did not run, because Cargo packages only a committed tree.

The first code step of M9, done on 2026-11-18, added the build of slang-rhi:

- The crate `slang-rhi-sys` builds slang-rhi with its Vulkan backend as static libraries from the tree of `slang-src`, offline, on Linux with the GNU C library only. It has no bindings, no Rust API, and no unsafe code yet, and it sets `publish = false`. It declares its two dependencies for that target only, so it refuses every other target at once and clones and builds nothing there (Section 7.1 of `docs/design/slang-rhi-sys.md`).
- `slang-src` clones eleven required submodules, with `external/slang-rhi` new, into the clone entry `src/<first 23 digits of the commit>-<hash of the submodule list>`, whose name of 40 characters the constant `RECORDED_CLONE_ENTRY` records (SRC-25 and SRC-11 of `docs/design/slang-src.md`).
- The `justfile` gained the variable `rhi_packages`, the recipes `check-rhi` and `test-rhi`, which run on Linux only and outside `just check` (TOOL-52 of `docs/design/tooling.md`), a third line of `recorded-slang`, and a stricter `check-vendored-shared-folder`. The CI job `vendored` gained their steps on Linux (Section 7.4 of that design).

The Vulkan backend of slang-rhi includes `X11/Xlib.h` on Linux, and the development container had no Xlib headers. On 2026-11-18, the owner decided that the Xlib development headers, such as those of the Debian package `libx11-dev`, are a stated requirement of the build, with no stub header (Section 4.1 of `docs/design/slang-rhi-sys.md`).

The same day, at the owner's request, `libx11-dev` and `xwayland` were installed with apt in the running development container, which Technology Decisions describes.

The step confirmed the assumptions of `docs/design/slang-rhi-sys.md` about the build, with one correction, the Xlib headers. Sections 3.5 and 5.1 to 5.4 of that design state the results, and Validating Changes records the build times and sizes.

On 2026-11-18 in the development container, with Rust 1.99.1, these recipes passed on the step before its branch was rebased onto the fourth code step:

- `check-rhi`, `check-vendored-shared-folder`, `check-vendored`, `check-features`, `check-msrv`, `check-deny`, and `check-unused-deps`.
- `test-rhi`, which printed "30 tests run: 30 passed, 0 skipped".
- `test-vendored`, which printed "427 tests run: 427 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".
- `just check`, which printed "966 tests run: 966 passed, 0 skipped" and, under Miri, "557 tests run: 557 passed (23 slow), 409 skipped".

On 2026-11-18, after the rebase onto the fourth code step, `check-rust` printed "988 tests run: 988 passed, 0 skipped" and, under Miri, "568 tests run: 568 passed (15 slow), 420 skipped", `test-vendored` printed "449 tests run: 449 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped", `test-rhi` printed "30 tests run: 30 passed, 0 skipped", and `check-vendored`, `check-rhi`, `check-vendored-shared-folder`, `check-features`, `check-msrv`, and `check-semver` passed.

An adversarial review found the build script, its entry protocol, and its offline guarantee sound. The step fixed its three findings:

- The wrapper project stops the configure step with a message when the compiler cannot find `X11/Xlib.h` (RSYS-17 of `docs/design/slang-rhi-sys.md`).
- The crate refuses another target before anything is cloned.
- `check-vendored-shared-folder` checks the shared folder more strictly.

Run 44220859704, for the first commit of the step, passed nine of its ten jobs, including the job `vendored` on Linux with the new steps `check-rhi` and `test-rhi`. It failed in "Test the vendored build on windows-latest", where the build script of `slang-compiler-sys` could not copy a file of the Slang tree, with "The system cannot find the file specified. (os error 2)". The clone entry of that commit had 57 characters, the full commit hash with the hash of the submodule list, so the longest paths of the tree exceeded the limit of a Windows path during the checkout and git left those files out (Section 3.5 of `docs/design/slang-src.md`).

The step fixed it with these changes:

- The name of the clone entry is now 40 characters, the first 23 hexadecimal digits of the commit, a hyphen, and the hash of the submodule list, and a test pins it at 40 characters at most. The build still verifies the full recorded commit.
- After the clone, the build script runs `git ls-files -z --deleted` in the tree and in each required submodule, and stops the build with a message that names the longest missing path, its length, and the shared folder, and advises a shorter `CARGO_TARGET_DIR` (SRC-26 of `docs/design/slang-src.md`). It does not set `core.longpaths`.
- `check-vendored-shared-folder` recognizes three layouts of clone entries and requires exactly the recorded one.

On 2026-11-18 in the development container, after the fix, these recipes passed:

- `test-vendored`, which cloned again and printed "453 tests run: 453 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".
- `test-rhi`, which printed "30 tests run: 30 passed, 0 skipped".
- `just check`, which printed "988 tests run: 988 passed, 0 skipped" and, under Miri, "568 tests run: 568 passed (15 slow), 420 skipped".
- `check-vendored`, `check-rhi`, `check-vendored-shared-folder`, `check-shell`, and `check-features`.

Run 44228807448, for the last commit of the step, passed all ten jobs, including the job `vendored` on Windows. So the clone entry name of 40 characters works on Windows, and the check of the work tree ran there without reporting a missing file.

The second code step of M9, done on 2026-11-18, ran the test of Windows and macOS of Section 7.2 of `docs/design/slang-rhi-sys.md` on the branch `feature/m9-rhi-platform-test`. That branch replaced the CI workflow with one job for the experiment and added a hook to `route.rs`, so it was never merged, and at the owner's request it was deleted on `origin` on 2026-11-19. Its runs 44234022601, 44235360178, 44236723057, and 44238304191 gave the result that Sections 7.2 to 7.4 of that design state. The D3D12 backend builds offline on Windows, and the Metal backend does not compile on macOS.

On 2026-11-18, the owner decided on the result (Section 4.1 of `docs/design/slang-rhi-sys.md`). slang-rhi is supported on Windows with D3D12, which the sixth code step of M9 implements, with the vendored Slang built with DXIL support (SCS-82 of `docs/design/slang-compiler-sys.md`). It stays unsupported on macOS, and the project asks Slang to update its submodule `external/metal-cpp`, which is the entry REQ-1 of `docs/upstream-reports.md`.

The third code step of M9, done on 2026-11-18, added the bindings of slang-rhi:

- `slang-bindgen` gained `generate_slang_rhi`, the field `Generated::abi_probe`, the rule set `SLANG_RHI`, the module `probe`, and the C++ shim headers, while `generate` and the rule set `SLANG` stayed unchanged (Section 7.8 of `docs/design/slang-compiler-sys.md`).
- The build script of `slang-rhi-sys` generates the bindings of slang-rhi in every build and builds the ABI probe, which the tests compare with the bindings (Sections 5 and 6 of `docs/design/slang-rhi-sys.md`).
- The crate has its `symbols`, ABI probe, smoke, and `defaults` tests, and two `gpu` tests, of which one runs a whole compute dispatch through the raw bindings on a Vulkan adapter.

The step departed from the first design where the headers required it, and the designs now state each departure with its reason. The main ones are the structs without `Default` of SCS-79, the pointer constant `kInvalidCUDAStream` of SCS-80, the callbacks that Rust calls with `"C-unwind"` of SCS-81, which FFI-7 of the coding standard now covers, and the Microsoft ABI decided from the triple that libclang normalizes (SCS-73). It confirmed SA-20 of `docs/design/slang-compiler-sys.md` and RSA-9 of `docs/design/slang-rhi-sys.md`.

The output of the binary of `slang-bindgen` for the Slang headers stayed byte for byte the same before and after the step, for the four tested targets, `aarch64-unknown-linux-gnu`, and five other spellings of Windows targets. An adversarial review compared the emitted bindings of slang-rhi with the real compiler, independently of the probe, on x86_64 Linux: 176 callable thunks, 626 field types, 808 enumerator values, and the generated `Default` values. It found them correct, and the step fixed its findings, which were the decision of the Microsoft ABI, the ABI of the callbacks of `ITaskPool::submitTask`, the pointer to the debug callback in the `gpu` tests, and the defaults of `SubmitDesc` and `HeapAllocDesc`.

On 2026-11-18 in the development container, with Rust 1.99.1, these checks passed on the step:

- `just check`, which printed "1032 tests run: 1032 passed, 0 skipped" and, under Miri, "580 tests run: 580 passed (15 slow), 452 skipped".
- `test-rhi`, which printed "45 tests run: 45 passed, 0 skipped" on the NVIDIA adapter and again with lavapipe.
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".
- `check-rhi`, `check-features`, `check-msrv`, `check-semver`, and `check-clippy` with Rust 1.100.0.
- cargo-semver-checks with `--release-type minor` on `slang-bindgen`, which printed "202 checks: 202 pass, 58 skip" and found no breaking change.

These are not verified:

- The ABI probe and the bindings of slang-rhi on AArch64, which were generated but never compiled or run (SA-21 of `docs/design/slang-compiler-sys.md`), and the probe compiled with Clang.
- The Microsoft ABI, for which the generator refused the overloads of slang-rhi until the sixth code step settled the former assumption SA-19 of that design.
- A Clang that gives a UEFI target the Microsoft ABI, which the generator would not detect (SA-22 of that design).

The fifth code step of M9, done on 2026-11-18, added the safe crate `slang-rhi` for the compute path (`docs/design/slang-rhi.md`):

- The crate `crates/slang-rhi`, with `publish = false`, its unit tests against a fake of five interfaces, and its `gpu` tests. A program can list the Vulkan adapters, open a device with a global session of `slang-compiler`, create a shader program and a compute pipeline, create and read buffers, set parameters by name, dispatch, and submit.
- The rule CONC-8 of the coding standard, which permits the one lock of the crate around the calls through the process-wide instance of slang-rhi.
- The workspace member `crates/slang-rhi` outside `default-members`, `bitflags` and `slang-rhi-sys` in `[workspace.dependencies]`, `slang-rhi` in `rhi_packages`, `--exclude slang-rhi` in every recipe that passes `--workspace`, and the recipe `check-miri-rhi` (TOOL-32 and TOOL-52 of `docs/design/tooling.md`).

The step departed from the first design where slang-rhi required it, and the design now states each departure with its reason and its source location in slang-rhi. The main ones are the per-thread guard of RHI-18, the debug callback object that every device gets and the collector of diagnostics of RHI-19, the end of a compute pass on a panic (RHI-9), the refusal of a compute pipeline without exactly one compute entry point (RHI-7), and unsafe code in the module of each wrapper (RHI-2).

The step confirmed RHA-1 to RHA-4 of `docs/design/slang-rhi.md` by reading and by test, and RSA-7 of `docs/design/slang-rhi-sys.md` by reading, and removed their rows. Sections 3, 6.4, 6.5, 6.7, and 7 of the first design and Section 3.3 of the second state the results. The step met no defect of slang-rhi.

An adversarial review found the lifetimes, the reference counting, the bounds checks, the instance lock, the per-thread guard, the callbacks, and the thread traits sound. It confirmed in the source of slang-rhi the three facts behind the main departures, and probes on lavapipe on 2026-11-18 verified its findings, which the step fixed:

- A submitted command buffer stayed usable although slang-rhi recycles the native object, so `Queue::submit` now takes its command buffers (RHI-21).
- A buffer of size 0 crashed the process. The crate accepted typed buffer ranges, although it creates no buffer with a format, and checked neither the usage of a buffer against its use nor a dispatch against the limit of the device. The crate now checks all four (RHI-8, RHI-11).
- The documentation of the option `validation` claimed more than the debug device of slang-rhi does.
- Smaller items were the bound flag after a failed bind, the panic text of a reentrant call, the offsets of parameters without uniform data, which the private fields of `ShaderOffset` now rule out, the explicit serial compilation of pipelines, and the refusal of programs with a specialization parameter left.

The review established that slang-rhi checks nothing unless its debug device is on, so the checks that keep the safe interface within valid use of the graphics API belong to the crate and are not guards against defects (RHI-20 of `docs/design/slang-rhi.md`). The step raised RHQ-5 to RHQ-9 of that design, which wait for the owner.

On 2026-11-18 in the development container, with Rust 1.99.1, these checks passed on the step:

- `just check`, which printed "release self-test passed: 78 cases", "1032 tests run: 1032 passed, 0 skipped", and, under Miri, "580 tests run: 580 passed (32 slow), 452 skipped". The counts did not change, because every recipe of `just check` excludes `slang-rhi`.
- `test-rhi`, which printed "113 tests run: 113 passed, 0 skipped" and then "test result: ok. 15 passed", "test result: ok. 22 passed", and "test result: ok. 0 passed" for the doctests, on the NVIDIA adapter and again with lavapipe.
- `check-miri-rhi`, which printed "50 tests run: 50 passed, 18 skipped" and took 8 min 33 s from an empty `target/miri-rhi`.
- `check-rhi` and `check-vendored-shared-folder`.
- With Rust 1.100.0, `check-clippy`, and Clippy on `slang-rhi` in the vendored environment with warnings denied.

Before the fixes of the review, `check-vendored`, `check-features`, `check-msrv`, `check-unused-deps`, `check-deny`, and `check-semver` passed, and `test-vendored` printed "453 tests run: 453 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".

What the tests of `slang-rhi` do not verify is in Section 10.4 of `docs/design/slang-rhi.md`: the NVIDIA adapter under the Khronos validation layer, a device without `nullDescriptor`, the path of a failed rebind, which was only read, and a message of the debug callback from a thread of the driver. When this was written, CI had not run the step.

The sixth code step of M9, done on 2026-11-18 on the branch `feature/m9-rhi-windows`, added the build for x86_64 Windows with D3D12 of Section 7.3 of `docs/design/slang-rhi-sys.md`:

- `slang-rhi-sys` and `slang-rhi` build on x86_64 Windows with the Microsoft toolchain and the D3D12 backend, offline, beside Linux with the GNU C library and Vulkan (RSYS-13). `slang-rhi-sys` finds `cl.exe` with `find-msvc-tools` and redirects WinPixEventRuntime to an empty directory (RSYS-18).
- The vendored Slang builds with DXIL support with the Microsoft toolchain only, and a program on Windows provides `dxcompiler.dll`, by the owner's decision of 2026-11-18 (SCS-82 of `docs/design/slang-compiler-sys.md`).
- `slang-bindgen` writes the bindings of slang-rhi for the Microsoft ABI with each run of adjacent overloads in reverse order, and the ABI probe reports every slot there (SCS-73 and SCS-78 of that design). This settled the former assumption SA-19.
- `slang-rhi` refuses `UNORDERED_ACCESS` in `Upload` and `ReadBack` memory and initial data for `ReadBack` memory on every backend (RHI-8 of `docs/design/slang-rhi.md`).
- `check-rhi` and `test-rhi` run on Linux and on x86_64 Windows, and the CI job `vendored` runs them on Windows as well, with the `dxcompiler.dll` of the runner's Windows SDK on `PATH` and a timeout of 90 minutes (TOOL-52 and Section 7.4 of `docs/design/tooling.md`).

The step departed from the first design in two points, which the designs now state. The DXIL option stays in `SLANG_OPTIONS` and is replaced in place with the Microsoft toolchain, so the entry keys of Slang on Linux and macOS do not change, and the generator writes the Microsoft order only for runs of adjacent overloads and refuses the rest.

The last commit of the step, `7c57f78`, fixed the findings of an adversarial review of the commit `6d29f8c`. The review verified these:

- The struct layouts and the `static_assert`s of the probe hold under Clang's Microsoft and Itanium layouts, and it derived all 200 slots of the slang-rhi headers independently with Clang for `x86_64-pc-windows-msvc` with no mismatch.
- The bindings for Linux and for the Microsoft ABI differ only in the order of the 8 pairs of overloads, and every method that passes or returns an aggregate by value is a placeholder on every ABI.
- The output of `slang-bindgen` for the Slang headers stayed byte for byte the same for five targets.
- The decoder of the probe decodes all 200 thunks of Clang's object and fails with the largest value on an unknown form.
- The Windows build can download nothing, and it links no Windows system library for D3D12, which slang-rhi loads at run time.

The step met three defects of slang-rhi, RKB-2 to RKB-4 in Section 3.1 of `docs/design/slang-rhi.md`, with their entries in `docs/upstream-reports.md`. RKB-2 was observed in CI, and the other two were read in the source. It raised RHQ-10 of `docs/design/slang-rhi.md`, SA-24 and SA-25 of `docs/design/slang-compiler-sys.md`, and RSQ-5 of `docs/design/slang-rhi-sys.md`, which wait for the owner or for evidence.

Run 44291885863, for the commit `6d29f8c` before the fixes of the review, passed all ten jobs, so the run concluded `success`. On the Windows runner, `check-rhi` took 15 min 13 s from an empty target directory, `test-vendored` printed "451 tests run: 451 passed, 0 skipped" and then "5 tests run: 5 passed, 9 skipped", and `test-rhi` printed "122 tests run: 122 passed, 0 skipped" with 15, 22, and 0 doctests, on the adapter "Microsoft Basic Render Driver", which is WARP. The shared folder held 785 MB, with a clone entry of 288 MB and no second clone.

On 2026-11-18 in the development container, with Rust 1.99.1, these checks passed on `7c57f78`:

- `just check`, which printed "release self-test passed: 78 cases", "1038 tests run: 1038 passed, 0 skipped", and, under Miri, "581 tests run: 581 passed (13 slow), 457 skipped".
- `test-rhi`, which printed "122 tests run: 122 passed, 0 skipped" and then 15, 22, and 0 passed doctests.
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".
- `check-miri-rhi`, which printed "52 tests run: 52 passed, 19 skipped".
- `check-rhi`, `check-vendored`, `check-vendored-shared-folder`, `check-features`, `check-workflows`, and `check-workflow-pins`, and `check-clippy` with Rust 1.100.0.

`check-msrv` and `check-semver` passed on `6d29f8c`. When this was written, the commit `7c57f78` and the documents of the step had no CI run, and Hard Rule 11 lets the step reach `main` only after its CI run succeeds.

These are not verified: a hardware D3D12 adapter, ARM64 Windows, another version of Visual Studio, the D3D12 debug layer, the probe compiled with clang-cl (SA-25), the order of the Microsoft compiler beyond the current headers (SA-24), what a program observes without `dxcompiler.dll`, which was only traced in the source, and whether DXC needs `dxil.dll` on a hardware adapter (Section 7.3 of `docs/design/slang-rhi-sys.md`).

The seventh code step of M9, done on 2026-11-18, wrote the documentation that the exit criteria of M9 name and closed the milestone:

- The crate READMEs `crates/slang-rhi/README.md` and `crates/slang-rhi-sys/README.md`, which state that neither crate is published on crates.io. The manifest of each crate names its README, and `slang-rhi` compiles the example of its README, which opens a device and prints its API and adapter, as a doctest.
- The crate-level documentation of `slang-rhi` gained a section on debug messages and a section on what the crate does not cover, and that of `slang-rhi-sys` now points to `slang-rhi` and to the documentation of slang-rhi. Item documentation of `slang-rhi` that named Vulkan only now names Direct3D 12 as well.
- The root `README.md` lists both crates in its table of crates and its licence table and states that they build on Linux and x86_64 Windows only. `docs/writing-readmes.md` names both crates in RDM-8 and in Tables 3 and 4.
- The `gpu` test `a_device_of_the_default_type_opens_on_the_api_of_the_platform` of `slang-rhi`, which checks that a device of the type `Default` opens on the platform's graphics API.

Run 44305866228, for the last commit of the sixth code step, passed all ten jobs after one rerun of the job "Test the vendored build on macos-latest", which GitHub had cancelled before any step for lack of a runner. That run is the evidence of the exit criterion on lavapipe and WARP for the code of the sixth code step.

On 2026-11-18 in the development container, with Rust 1.99.1, these checks passed on the code of the seventh code step:

- `just check`, which printed "release self-test passed: 78 cases", "1038 tests run: 1038 passed (1 slow), 0 skipped", and, under Miri, "581 tests run: 581 passed (15 slow), 457 skipped".
- `test-rhi`, which printed "123 tests run: 123 passed, 0 skipped" and then 16, 22, and 0 passed doctests, on the NVIDIA adapter and again on lavapipe.
- `check-miri-rhi`, which printed "52 tests run: 52 passed, 20 skipped".
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "5 tests run: 5 passed, 10 skipped".
- `check-rhi`, which took 12 min 12 s from an empty `target/vendored` and built the documentation of both crates with warnings denied.
- `check-vendored`, `check-vendored-shared-folder`, `check-features`, `check-msrv`, `check-semver`, `check-deny`, and `check-unused-deps`, and `check-clippy` with Rust 1.100.0.

`check-package` did not run, because Cargo packages only a committed tree. When this was written, the new test had not run on Windows, and Hard Rule 11 lets the step reach `main` only after its CI run succeeds.

These open questions and unverified assumptions stay open after M9, with no milestone assigned unless one is named:

- RHQ-1 to RHQ-10 of `docs/design/slang-rhi.md`. RHQ-1 was assigned to M9, which did not decide it, so it has no milestone now. RHQ-2 and RHQ-4 are assigned to M10.
- RSQ-2 and RSQ-3 of `docs/design/slang-rhi-sys.md`, assigned to M10, RSQ-4, which the owner decides before the second release, RSQ-5, and the assumption RSA-8 of that design.
- SA-21, SA-22, SA-24, and SA-25 of `docs/design/slang-compiler-sys.md`.
- CQ-14 of `docs/design/slang-compiler.md`, and CQ-13 of that design, assigned to M17.
- SRCQ-4 of `docs/design/slang-src.md`, which also concerns the entry key of `slang-rhi-sys`.
- What the tests of `slang-rhi` do not verify, which Section 10.4 of `docs/design/slang-rhi.md` lists.

The design of M10, the opt-in backend `rhi` of `slango` (Section 15.13 of `docs/design/architecture.md`), exists since 2026-11-18, and all seven of its code steps are done. It consists of these parts, within the owner's decisions of 2026-11-18 and 2026-11-19, which Section 4.6 of `docs/design/slango.md` records:

- `docs/design/slango.md`, Sections 3.11, 4.6, 4.7, 6.1 to 6.4, 7.6, 7.7, 8.3, 9.6, 9.7, 13.7, and 17.6, with SLO-33, SLO-34, SLO-36 to SLO-46, and the changed SLO-17, SLO-28, SLO-29, and SLO-30. The features `wgpu` and `rhi` of `slango` exclude each other, so each build has one backend, every type that holds a GPU object is neither `Send` nor `Sync` in version 0.2, and each load on the backend `rhi` creates its own Slang session.
- `docs/design/slango-macros.md`, SLM-24 to SLM-26 and the changed SLM-5, SLM-8, SLM-11, SLM-15, and SLM-21, for the hidden helper macros of `slango` that keep the loading path and the struct layouts of the program's backend only, and the embedded sources that the backend `rhi` compiles.
- `docs/design/slang-slint.md`, the changed SLS-14 and Section 10, because the crate stays on wgpu and cannot be built with the backend `rhi`.
- `docs/design/slang-rhi.md`, Section 11 with RHI-22 to RHI-30, among them the sessions of RHI-30 that reproduce the device session, and `docs/design/slang-rhi-sys.md`, Section 6.2, which answers RSQ-2 and RSQ-3 with no change of the generator.
- `docs/design/tooling.md`, TOOL-53, TOOL-54, and the changed TOOL-48, for the recipes `check-rhi`, `test-rhi`, and `check-features`, and for `publish = false` on `slango`, `slango-codegen`, and `slango-macros` until M11.

Section 15.13 of the architecture design orders its seven code steps, the compute phase before the rendering phase. The design answers SOQ-4 and SOQ-9 of `docs/design/slango.md`, RHQ-4 of `docs/design/slang-rhi.md`, and Q-36 of the architecture design, and the owner's decision 5 answers the layout of structures on the backend `rhi`. RHQ-2 of `docs/design/slang-rhi.md` is no longer needed for M10 and has no milestone.

The fourth code step added the rule SIMP-5 of the coding standard for the helper macros of SLM-25 of `docs/design/slango-macros.md`. From the third code step until M11, no alpha of `slango`, `slango-codegen`, or `slango-macros` can be published (TOOL-54 of `docs/design/tooling.md`).

The first code step of M10, done on 2026-11-18 on the branch `feature/m10-boundary`, built the boundary of version 0.2 of `slango` on wgpu alone, with no feature `rhi` and no slang-rhi:

- The shared modules of `slango` hold no wgpu type, and the private backend module `wgpu_backend`, imported as `backend`, holds the objects and calls of wgpu (SLO-34 of `docs/design/slango.md`). The rule SIMP-2 of the coding standard now requires this structure and names its exceptions: the crate root, `slango::wgpu`, and tests.
- `Device`, `Module`, `Kernel`, `Buffer<T>`, `RenderPipeline`, `Texture`, `Sampler`, `Call`, and `Draw` of `slango` and `View` of `slang-slint` are neither `Send` nor `Sync`, through the marker field of the new rule CONC-9 of the coding standard, and CONC-1 and Section 17 of that standard name this exception (SLO-17). `compile_fail` doctests assert it for these types, and compile-time assertions in the module `bindings` of the test binary `macros` of `slango-macros` assert it for the generated `Shader`, kernel struct, and arguments struct (SLM-21 of `docs/design/slango-macros.md`). The `gpu` tests that used several threads are removed, or keep their part on one thread (Section 13.7 of `docs/design/slango.md`).
- A call and a draw refuse one buffer bound to a read-only and to a read-write used parameter, and a draw refuses a vertex or index buffer that is also bound to a read-write parameter, with `ErrorKind::InvalidArgument` instead of the former `Backend` (SLO-44). Each `Buffer<T>` keeps an identity that its clones share.
- The walk records the row stride of each matrix that reflection reports, marshalling writes each row at it, and `ENCODING_VERSION` is 3 (SLO-46). `slango-codegen` and its snapshots assert version 3.
- The examples package has the features `wgpu`, its default, and `vendored`, takes `slango` without its default features, and builds its windowed programs only with `wgpu`. `check-features` checks it with `slango` in its second command (TOOL-48 of `docs/design/tooling.md`).

The step departed from the plan of Section 15.13 of the architecture design in two points, which that section and Section 4.3 of `docs/design/slango.md` now state. The crate root keeps the gate on `wgpu` and the message of M8, because a `cfg` of the missing feature `rhi` trips the lint `unexpected_cfgs`, and the backend `wgpu` has no crate-private `Module`. The third code step adds both. `check-features` excludes only `vendored` until that step adds `rhi`.

The step confirmed SOA-11 of `docs/design/slango.md` with no change of `slang-compiler`, and removed its row. Section 3.11 of that design states the row strides that reflection reports, and Section 3.5 states that naga rejects a matrix whose rows have two components in a uniform buffer. The step raised the assumption SOA-12 of that design and met no defect of Slang.

An adversarial review of the step found that every wgpu descriptor, label, error text, order of checks, and order of drops stayed the same through the restructure, by a reading of the old code against the new. It also found that the nine GPU types of `slango` and `View` are neither `Send` nor `Sync`, and that the errors, options types, and enumerations are both. Matrices of several shapes in constant buffers, nested structures, and global arrays read back exactly on the NVIDIA adapter and on lavapipe. Its findings led to the rules of SLO-44 above and to changes of CONC-1, CONC-9, and SIMP-2. The review also asked that each of the two trybuild cases of `slango-macros` that then asserted the missing traits assert one type only. The second commit of the step replaced those cases with the compile-time assertions, as the paragraphs on CI below state.

On 2026-11-18 in the development container, with Rust 1.99.1, these checks passed on the step after the fixes of the review:

- `just check`, which printed "release self-test passed: 78 cases", "1053 tests run: 1053 passed, 0 skipped", and, under Miri, "595 tests run: 595 passed (24 slow), 458 skipped".
- `test-gpu` with lavapipe, which printed "1053 tests run: 1053 passed, 0 skipped".

These checks passed on the step before the fixes of the review and did not run again after them:

- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 10 skipped". `slango-macros` has one test fewer, the removed assertion of `Send` and `Sync`.
- `test-rhi`, which printed "123 tests run: 123 passed, 0 skipped".
- `check-features`, `check-msrv`, `check-semver`, `check-vendored`, `check-rhi`, `check-deny`, and `check-unused-deps`, and `check-clippy` with Rust 1.100.0.
- cargo-semver-checks with `--release-type minor` on `slango` reported one failed check, `auto_trait_impl_removed`, for the 18 removed implementations of `Send` and `Sync` of the nine types, as intended, and on `slang-compiler` it found no breaking change.

These are not verified: the row strides of DXIL, which were measured only through reflection, and an encoding of version 2 fed to `slango` at run time.

CI run 44370707422, for the first commit of the step, concluded `failure`. Six of its ten jobs passed. The jobs "Run every check on Linux", "Test on macos-latest", "Test on windows-latest", and "Measure test coverage on Linux" failed in the one test `ffi::the_macro_compiles_a_shader_and_reports_each_failure_at_the_path` of `slango-macros`.

Its trybuild cases `not_send` and `not_sync` reported a mismatch, because their `.stderr` files held the notes of error E0277 as rustc 1.99.1 words them, and the newer stable rustc of the runners words them differently. The failure reproduced in the development container with Rust 1.100.0.

The second commit of the step removed the two cases and their `.stderr` files. The module `bindings` of the test binary `macros` of `slango-macros` now asserts at compile time, by trait ambiguity, that the generated `saxpy::Shader`, `saxpy::Saxpy`, and `saxpy::SaxpyArgs<'static>` each implement neither `Send` nor `Sync` (SLM-21 of `docs/design/slango-macros.md`). The assertions depend on no compiler message, and `test-vendored` checks them as well, because it compiles the whole test binary.

CONC-9 of the coding standard now names the two accepted tests of a missing auto trait and forbids a test that compares diagnostic text, and the reference list gained the source of the technique, the crate static_assertions.

On 2026-11-18 in the development container, these checks passed after the fix, with Rust 1.99.1 unless stated. The test counts did not change.

- `test-gpu`, which printed "1053 tests run: 1053 passed, 0 skipped" with Rust 1.99.1 and the same with Rust 1.100.0, including the seven remaining trybuild cases.
- `check-doctest` with Rust 1.100.0, so the `compile_fail,E0277` doctests of `slango` and `slang-slint`, which name only the error code, hold there.
- `check-miri`, which printed "595 tests run: 595 passed (33 slow), 458 skipped".
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 10 skipped".
- `check-fmt`, `check-clippy` with both toolchains, `check-rust-comments`, and `check-doc-references`.

When this was written, CI had not run the second commit, and Hard Rule 11 lets the step reach `main` only after the CI run of its last commit succeeds.

The second code step of M10, done on 2026-11-18 on the branch `feature/m10-rhi-compute`, added the compute phase of Section 11.1 of `docs/design/slang-rhi.md` to `slang-rhi`, and changed no other crate:

- `Device::create_slang_session` returns a `CompileSession`, a Slang session of its own that reproduces the settings of the device session, whose `create_shader_program` accepts the programs of that session (RHI-30). A `ShaderProgram` of a `CompileSession` holds its session.
- `ShaderObject::object` returns the object of a `ConstantBuffer`, `cbuffer`, or `ParameterBlock` parameter, whose fields are then set by name, nested to any depth (RHI-22).
- `DeviceInfo` offers five limits of the device (RHI-29), and `Operation` gained `GetObject` and `CreateSlangSession`.
- `set_buffer` and `object` refuse a parameter that is a structure of several resources with `ErrorKind::InvalidArgument`, where `set_buffer` used its first binding range before.

The step confirmed RHA-5 of `docs/design/slang-rhi.md` and removed its row, and confirmed RHA-8 on Vulkan. CI later confirmed RHA-8 with DXIL on WARP, as the last paragraph of this step states, and the third code step removed its row. Sections 3 and 11.1 of that design state the results. It corrected RHI-29 and RHI-30 where slang-rhi differs from the design: the largest buffer size is the 2 GiB that slang-rhi assumes on both backends, and slang-rhi forces the scalar layout on every target, DXIL included.

An adversarial review found no unsoundness that safe code reaches in `object` or `CompileSession`. It confirmed by reading that slang-rhi keeps the Slang session of a shader program alive natively, and compared the layouts and code of 13 further shaders in both sessions with no difference. Its fixes are the refusal of structures of several resources, the documentation of the limits and of `object` under the debug device of slang-rhi, and the split of the nested `gpu` test of objects, because its form with a uniform field has the shape of RKB-2 one level down.

The review found two defects of slang-rhi on Vulkan, RKB-5 and RKB-6 in Section 3.1 of `docs/design/slang-rhi.md`, with their entries in `docs/upstream-reports.md`. An array of `ConstantBuffer` parameters is bound as if it had one element, and a kernel with a push-constant buffer aborts the process at its dispatch. The crate does not guard against either, as Hard Rule 14 requires.

On 2026-11-18 in the development container, with Rust 1.99.1, these checks passed after the fixes of the review:

- `test-rhi`, which printed "143 tests run: 143 passed, 0 skipped" and then 18, 24, and 0 passed doctests, on the NVIDIA adapter and again on lavapipe.
- `check-miri-rhi`, which printed "60 tests run: 60 passed, 32 skipped".
- `check-rhi`, `check-fmt`, `check-rust-comments`, `check-doc-headings`, and `check-doc-references`, and Clippy and the doctests of `slang-rhi` with Rust 1.100.0 as well.

These checks passed before the fixes of the review and did not run again after them:

- `just check`, which printed "release self-test passed: 78 cases", "1053 tests run: 1053 passed, 0 skipped", and, under Miri, "595 tests run: 595 passed (20 slow), 458 skipped".
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 10 skipped".
- `check-vendored`, `check-vendored-shared-folder`, and `check-semver`.

Run 44388142812, for the commit of the step, passed all ten jobs, and its `test-rhi` printed "143 tests run: 143 passed, 0 skipped" on WARP. So the D3D12 items of the step are verified there: the target and profile of `create_slang_session`, RHA-8 with DXIL, the tests of objects and of the limits, and the nested test with a uniform field, which passed on D3D12 (Section 10.4 of `docs/design/slang-rhi.md`). Arrays of `ParameterBlock` parameters are not tested.

The third code step of M10, done on 2026-11-18 on the branch `feature/m10-slango-rhi-compute`, added compute on the backend `rhi` of `slango` (Sections 6, 7.6, and 9.6 of `docs/design/slango.md`):

- `slango` has the feature `rhi`, off by default, which enables the optional dependency `slang-rhi` and `vendored`. Its crate root compiles exactly one backend and stops with the one error of SLO-29 for neither feature or both, and the private module `rhi_backend` holds the device, the load in a Slang session of its own, the kernels, the buffers, the dispatch, and the mapping of the errors of `slang-rhi` (SLO-28, SLO-33, SLO-37 to SLO-40, and SLO-42).
- A program that turns off the default features of `slango` and enables `rhi` runs compute kernels through slang-rhi on Vulkan on Linux and on D3D12 on x86_64 Windows, with the same public API apart from the module `slango::wgpu`. `Texture::new`, `Texture::from_pixels`, `Sampler::new`, `Module::render_pipeline`, and `__private::load_precompiled` return the new `ErrorKind::Unsupported` on that backend, after their shared checks (Section 6.4).
- The shared code calls each backend through crate-private types of the same names, with a neutral plan of a call that the backend `wgpu` groups into bind groups itself and the backend `rhi` writes by parameter path. The backend `wgpu` gained a unit-struct `Module`, and the backend `rhi` holds `Texture`, `Sampler`, and `RenderPipeline` as uninhabited types, which SIMP-2 of the coding standard now covers, with the source `[S-13]` of the reference list (Section 6.1 of `docs/design/slango.md`).
- The walk reads the usage of a binding from its register class on DXIL and takes the outer block of the entry point as the immediates there. It also records, beside the snapshot and outside its encoding, the place of each binding, so that the backend `rhi` sets resources and blocks among the parameters of the entry point. `ENCODING_VERSION` stays 3 (Section 7.1 of `docs/design/slango.md`).
- `slango`, `slango-codegen`, and `slango-macros` set `publish = false`, and `release_packages` holds the five crates `slang-bindgen`, `slang-src`, `slang-compiler-sys`, `slang-glslang-sys`, and `slang-compiler` (TOOL-54 of `docs/design/tooling.md`). Their crate READMEs say that this version is not published on crates.io. `doc-site` derives the features `vendored` of the crates that it selects from `cargo metadata`, because its fixed list failed once `slango-macros` left `release_packages` (TOOL-41).
- `check-rhi` and `test-rhi` also lint, document, and test `slango` with `--no-default-features --features rhi`, with no filter of nextest, and the second command of `check-features` passes `--exclude-features vendored,rhi` (TOOL-48 and TOOL-53).
- `slang-rhi` refuses in `create_compute_pipeline` a kernel whose thread group exceeds the limits of the device, with `InvalidArgument` before any call into slang-rhi, and `DeviceInfo` offers seven limits, with `max_compute_thread_group_size` and `max_compute_threads_per_group` (RHI-7 and RHI-29 of `docs/design/slang-rhi.md`).

The stated differences between the backends are in Section 6.5 of `docs/design/slango.md`. The backend `rhi` runs some kernels that wgpu rejects, such as those with 64-bit integers, half-precision floats, or wave operations, because it hands the code of Slang to the driver without the validation of wgpu. It refuses at load a used resource inside a structure that is not the element of a constant buffer or a parameter block, and a used constant buffer whose element is not a structure, which the backend `wgpu` runs.

The step raised SOQ-11 of `docs/design/slango.md` and RHQ-11 of `docs/design/slang-rhi.md` for them, and TQ-19 of `docs/design/tooling.md` for the hosted pages of the three crates in a release before M11. It met no defect of Slang or slang-rhi.

The step confirmed SOA-10 and SOA-12 of `docs/design/slango.md` for compute, on Vulkan and on D3D12, and narrowed both rows to rendering (Section 3.11 of that design). It confirmed TA-26 of `docs/design/tooling.md`, because `check-semver` skips the members with `publish = false` by itself, and removed its row.

TA-25 holds for the commands of `slango`: the job `vendored` on Windows took 15 minutes in all in run 44398417125, and the row stays for the commands of the examples package of the fourth and sixth code steps.

An adversarial review of the first commit of the step verified these:

- The behaviour of the backend `wgpu` is unchanged: the same bind groups and order, no test lost, and no layout snapshot changed.
- Marshalling on the backend `rhi` equals that of wgpu on the NVIDIA adapter and on lavapipe for scalars, vectors, every shape of matrix, arrays, nested structures, constant buffers, parameter blocks, nested blocks, the `uniform` parameters of the entry point, several kernels, and structured buffers.
- The error kinds and texts for bad input are equal on both backends, and the adapter selection picks the same adapters.
- Memory does not grow over 20000 calls or 300 loads.
- Neither or both backend features give exactly one compile error.

The step fixed the findings of the review in five later commits:

- Parameters of the entry point that are resources or blocks could not be called on the backend `rhi`. The places of the bindings fix it, and three `gpu` tests in `gpu/call/entry_point.rs` run on both backends.
- A kernel whose thread group exceeded the device loaded and dispatched on the backend `rhi` and then broke the device, so that every later read failed. `slang-rhi` now refuses it. In `slango`, such a load fails with `ErrorKind::Backend` on the backend `wgpu` and on the backend `rhi` on Vulkan, and with `ErrorKind::Compile` on the backend `rhi` on D3D12, where the DXIL compiler refuses the kernel first (Section 7.6 of `docs/design/slango.md`).
- `just doc-site` failed after `slango-macros` left `release_packages`. The recipe now derives its features, the stub `cargo` of the release self-test refuses a feature of a package that a command does not select, and the self-test runs `doc-site`, with 80 cases.
- The backend `rhi` addressed some parameters that slang-rhi cannot reach. Its load now refuses them with `UnsupportedParameter`, and three mismatches between a call plan and the bindings of a kernel are errors of kind `Backend` instead of skips.
- `test-rhi` had a nextest filter. It has none now: 60 tests of textures, samplers, render pipelines, and precompiled loads carry an `ignore` attribute under the feature `rhi` with a reason, five tests whose checks run before `Unsupported` run on the backend `rhi`, and `gpu/unsupported.rs` asserts `Unsupported` for the five operations above.
- The shared API documentation of `slango` describes both backends, and the text of a failed buffer read says "the backend read" where it said "wgpu mapped" (Section 17.6 of `docs/design/slango.md`).

On 2026-11-18 in the development container, with Rust 1.99.1 unless stated, these checks passed on the last commit of the step:

- `just check`, which printed "release self-test passed: 80 cases", "1068 tests run: 1068 passed, 0 skipped", and, under Miri, "597 tests run: 597 passed (25 slow), 471 skipped".
- `test-rhi`, on the NVIDIA adapter and again on lavapipe, which printed "148 tests run: 148 passed, 0 skipped" with 18, 24, and 0 passed doctests for the slang-rhi crates, and then "292 tests run: 292 passed, 60 skipped" with 22 and 18 passed doctests for `slango` on the backend `rhi`.
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 10 skipped".
- `check-features`, `check-unused-deps`, `check-vendored`, `check-rhi`, and `doc-site`, which built a site of 45 MB.
- `check-semver`, which checked `slang-bindgen`, `slang-compiler`, and `slang-compiler-sys` only.
- Clippy with Rust 1.100.0 on the workspace, on the build of `slango` with `rhi`, and on the slang-rhi crates.

`check-miri-rhi` printed "64 tests run: 64 passed, 33 skipped" before the last commits, which changed only test files of `slango`. `check-msrv`, `check-deny`, and `check-vendored-shared-folder` passed on the first commit of the step, and `check-package` on that commit packaged the five crates of `release_packages`. The first build of `slango` with `rhi` from an empty `target/vendored` took 10 min 36 s.

Run 44398417125, for the first commit of the step, passed nine of its ten jobs. GitHub cancelled "Test on macos-latest" before any step for lack of a runner. Its Windows job `vendored` printed "143 tests run: 143 passed, 0 skipped" for the slang-rhi crates and "271 tests run: 271 passed, 66 skipped" for `slango` on the backend `rhi` on WARP, where the tests that combine a constant buffer or `cbuffer` with the `uniform` parameters of the entry point passed.

Run 44407500494, for the commit that held the code of the step with the fixes of the review and before its documents, passed nine of its ten jobs, including "Test on macos-latest". On WARP, the slang-rhi crates printed "148 tests run: 148 passed, 0 skipped", with the `gpu` test of the thread group check of `slang-rhi`, and `slango` on the backend `rhi` printed "274/290 tests run: 273 passed, 1 failed, 61 skipped". So every other test of the fixes of the review passed on D3D12, among them the three tests of the resources and blocks among the parameters of the entry point and the tests of the two refusals at load.

The job "Test the vendored build on windows-latest" failed in `gpu::load::a_kernel_whose_thread_group_exceeds_the_device_fails_to_load_and_the_device_keeps_working` of `slango`, which expected the kind `Backend` and got `Compile`, with "cannot compile a Slang module". On D3D12, the load generates DXIL, and the DXIL compiler refuses `[numthreads(2048, 1, 1)]`, over its limit of 1024 threads along an axis, before the check of `slang-rhi` is reached.

On Vulkan and on wgpu, the same kernel fails with `Backend`. That is a difference of the graphics APIs, not a defect. The fix changes only the test, which now expects `Compile` on the backend `rhi` on Windows and `Backend` elsewhere.

Run 44414032569, for the last commit of the step, passed all ten jobs. On WARP, the slang-rhi crates printed "148 tests run: 148 passed, 0 skipped", and `slango` on the backend `rhi` printed "290 tests run: 290 passed, 61 skipped", so the fixed test of the thread group passed on D3D12. The mappings of the kinds `Compile`, `Unsupported`, and `NoDevice` of `slang-rhi` are not verified, because no input produces them in a test.

The fifth code step of M10, done on 2026-11-18 and 2026-11-19 on the branch `feature/m10-rhi-rendering`, added the rendering phase of Section 11.2 of `docs/design/slang-rhi.md` to `slang-rhi`, which the rendering step of the backend `rhi` of `slango` builds on. It changed no other crate, and outside the crate only `typos.toml`. This step came before the fourth code step, because the first attempt at the fourth step stopped before any change when an automated safeguard halted a helper agent, and the owner approved its retry on 2026-11-19.

- A program can create two-dimensional textures of four colour formats, with or without initial data, and of `D32Float`, read a colour texture back without the padding of its rows, create samplers, and set textures and samplers by name with `set_texture` and `set_sampler` (RHI-23 to RHI-25, RHI-28).
- It can create a render pipeline from a vertex and a fragment entry point with vertex streams, a blend, a cull mode, and an optional depth target, and draw with and without indices in a render pass that a closure scopes and that ends on every path, a panic included (RHI-26, RHI-27).
- `Operation` gained eleven operations, and a failed creation of a render pipeline has the kind `Compile`.
- `typos.toml` accepts the three generated field names `maxLOD`, `minLOD`, and `mipLODBias` of slang-rhi (Section 3.1 of `docs/design/tooling.md`).

The crate refuses, with `InvalidArgument` and before the native call, what the graphics API makes invalid and the crate can check (RHI-20). The main checks are these:

- `create_render_pipeline` requires a stream at the location of every vertex input that the vertex entry point declares, even one that the shader does not read, of the numeric kind and the semantic of that input, and a stride that is a multiple of 4. On D3D12, DXIL keeps a declared input that the shader does not read in its input signature, so the pipeline failed without a stream for it, while Slang removes such an input from SPIR-V. The rule is the same on both backends, and on D3D12 the input layout holds no filler element (RHI-26).
- `set_texture` refuses a texture parameter that is not a plain two-dimensional texture or whose elements are not floating-point numbers, and `set_sampler` refuses a comparison sampler, in compute passes too (RHI-25).
- A render pass checks its targets, each draw checks the bound pipeline and the vertex and index buffers against its counts, and a draw refuses a texture that its pass both samples and draws into (RHI-27).

The step departed from the first design where slang-rhi or the graphics APIs required it, and Section 11.2 of `docs/design/slang-rhi.md` states each departure with its reason. The main ones are the vertex input rule, which replaced the former assumption RHA-6, the query of format support for `D32Float` on Vulkan only, the exact bound of a vertex buffer, and the references that a render pass holds. The step confirmed RHA-7 by reading and by test and removed both rows. Section 3 of that design states the facts and the conventions of a draw, which the `gpu` tests confirmed on Vulkan and on D3D12, so `docs/design/slango.md` retired SOA-9.

Depth clamping is the one difference between the backends that comes from slang-rhi. On Vulkan, slang-rhi always clamps, so a primitive outside the depth range 0 to 1 is drawn with its depth clamped, while D3D12 clips it, and a `gpu` test observed both. The step met three defects of slang-rhi, RKB-7 to RKB-9 in Section 3.1 of `docs/design/slang-rhi.md`, with their entries in `docs/upstream-reports.md`. RKB-7 and RKB-8 concern `getFormatSupport` and were read in the source, and RKB-9 is the depth clamping, observed on both backends. The crate does not guard against them, as Hard Rule 14 requires.

The step raised RHQ-12 of `docs/design/slang-rhi.md`, on the feature `depthClamp` of Vulkan, and SOQ-12 of `docs/design/slango.md`, on whether the backend `rhi` of `slango` supplies a stream for a vertex attribute that the shader does not use, which the backend `wgpu` does not need.

An adversarial review on 2026-11-18 and 2026-11-19 ran the render tests under the Khronos validation layer on lavapipe and on the NVIDIA adapter. It found these sound:

- The lifetimes of render passes and of their shader objects, whose attempts to escape fail to compile, the references that a pass keeps, and the orders of drops.
- The reads and the order of passes under synchronization validation, and the four colour formats with their expected bytes.
- That `robustBufferAccess` is enabled, the guards on every native call, and the attribution of the three defects.

The step fixed its findings in four commits:

- A stride that is not a multiple of 4 is refused, and so are vertex inputs without a matching stream (RHI-26).
- Texture parameters of other shapes or of integer elements and comparison samplers are refused (RHI-25).
- The documentation states the depth clamping, the undefined contents of a texture that was never written, and that a buffer that a draw reads as a vertex or index buffer and writes through a read-write parameter is not refused, which `slango` refuses by SLO-44 of `docs/design/slango.md`.
- The fake render pass shares its log with its root objects and creates a new root on each bind, and a unit test under Miri covers the record of sampled textures.

After the fixes, the 25 render tests gave no validation error on either adapter, and the one message was the performance warning `WARNING-Shader-OutputNotConsumed`.

In the development container, with Rust 1.99.1 unless stated, these checks passed on the last code commit of the step:

- `test-rhi`, on the NVIDIA adapter and again on lavapipe, which printed "225 tests run: 225 passed, 0 skipped" with 27, 33, and 0 passed doctests for the slang-rhi crates, and then "292 tests run: 292 passed, 60 skipped" with 22 and 18 passed doctests for `slango` on the backend `rhi`.
- `check-miri-rhi`, which printed "116 tests run: 116 passed, 58 skipped".
- `just check`, which printed "release self-test passed: 80 cases", "1068 tests run: 1068 passed, 0 skipped", and, under Miri, "597 tests run: 597 passed (36 slow), 471 skipped". The counts did not change, because every recipe of `just check` excludes `slang-rhi`.
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 10 skipped".
- `check-rhi`, `check-vendored`, and `check-semver`, and Clippy, the tests, and the doctests of `slang-rhi` with Rust 1.100.0.

Run 44423418363, for the first commit of the step, failed in two jobs. "Run every check on Linux" failed in the download of the Vulkan SDK, with curl exit status 92, before any code of the project ran. The Windows job `vendored` failed in the `gpu` test of an input that the shader declares and does not read, after 153 tests had passed, which led to the vertex input rule above.

Run 44431645707, for the commit with the fixes of the review and before the documents, passed all ten jobs. On WARP, it printed "225 tests run: 225 passed, 0 skipped" for the slang-rhi crates and "290 tests run: 290 passed, 61 skipped" for `slango` on the backend `rhi`. So every render test passes on D3D12, including the conventions, the counts of components, the stream at a location that the shader does not declare, the read-back with the row pitch of D3D12, and the clipping of a triangle outside the depth range.

What the tests of `slang-rhi` do not verify is in Section 10.4 of `docs/design/slang-rhi.md`, among them a Vulkan device without `depthClamp`, draws into BGRA and sRGB targets, and linear filters. When this was written, the documents of the step had no CI run, and Hard Rule 11 lets the step reach `main` only after the CI run of its last commit succeeds.

The fourth code step of M10, done on 2026-11-19 on the branch `feature/m10-macro-rhi`, brought the macro `shader!` of `slango-macros` to the backend `rhi` of `slango` (SLM-24 to SLM-26 of `docs/design/slango-macros.md`). Its first attempt stopped before any change, when an automated safeguard halted a helper agent, and the owner approved the retry on 2026-11-19.

- A program whose `slango` has the feature `rhi` can call `shader!`, with no feature on `slango-macros`. The generated code wraps the items of each backend in the hidden helper macros `wgpu_only!` and `rhi_only!` of `slango`, which keep only those of the backend of the program's `slango` (SLM-25 and SIMP-5 of the coding standard).
- On the backend `wgpu`, the generated code embeds the encoded snapshot as before. On the backend `rhi`, it embeds the Slang source of the shader and of every module that it imports, in the order of their imports, and `__private::load_embedded` compiles them in a session of their own when the module loads. `load_embedded` replaces `__private::load_precompiled` on both backends.
- Each generated struct of an element structure has the std430 layout on `wgpu` and the packed layout of the scalar layout on `rhi`, which a second compile in each expansion reads. At a dispatch or a draw, a buffer of a generated struct binds only to a structure whose fields match it in path, offset, and type, on both backends (Section 9.3 of `docs/design/slango.md`).
- The examples package has the feature `rhi`, and its module `gpu/generated.rs` runs the generated code on the build's backend. `check-rhi` and `test-rhi` lint and test the examples package on the backend `rhi` (TOOL-53 of `docs/design/tooling.md`).
- The 26 tests of `slango-codegen` that format generated code skip Miri with a reason, because each took minutes there and the crate has no unsafe code, which the new rule TEST-11 of the coding standard permits. They run natively.

The step departed from the design where it was silent or where a review found a gap, and Sections 5.2, 7.1, 7.3, and 8 of `docs/design/slango-macros.md` state each choice. The main ones are these: `slango-codegen` orders the modules in a pure function, a cycle and a description without a module are errors of kind `Compile`, `StructElement::FIELDS` lists nested fields by their paths, and a layout without padding gets a private field of zero size, so a value is created with `new` on both backends.

The step confirmed the former assumption SLMA-6 of `docs/design/slango-macros.md` by test and removed its row, and narrowed SLMA-7 to structures beyond those tested (Sections 3.5 and 3.9 of that design). It raised SLMQ-5 of that design, on a buffer of a plain element type, which still binds to a structure by its stride alone, and met no defect of Slang.

An adversarial review on 2026-11-19 verified these:

- The generated struct layouts equal Slang's reflection on both backends for twelve structures, with round trips on `wgpu` and `rhi` on the NVIDIA adapter and on lavapipe. By reflection, the layout of DXIL equals the scalar layout of SPIR-V for every tested type, including types that the macro does not support.
- Embedded imports work for a diamond, a chain four deep, two files of the same stem in two directories, and a module whose declared name differs from its file name, also after the shader directory is moved. A changed imported file triggers a rebuild, and the second compile adds about 0.2 s to each call of the macro.
- The output on `wgpu` changed only by the wrapping, `FIELDS`, the tracking, and `EMBEDDED`.

The step fixed its findings:

- The check at bind time compared field paths and offsets only, so a struct with a `float3` field bound to a kernel that takes `int3`, on both backends. It now compares the type of each field as well, and a generated struct no longer binds to an element that is not a structure. `ENCODING_VERSION` stays 3.
- The formatting tests of `slango-codegen` took up to about 14 min each under Miri. With the skip, the Miri time of the crate fell from 3898.9 s, summed over its tests, to 173.0 s, and `just check` from 26 min 11 s to 7 min 22 s in the development container.
- An element struct without padding could be built with a struct expression on one backend only.
- The documentation of `slango-macros` and `slango-codegen` states two limits: a program on `rhi` can use `slango-codegen` only as a build dependency or through `slango-macros`, and the second compile does not define `__VULKAN__` or `__D3D12__`.

In the development container, with Rust 1.99.1 unless stated, these checks passed on the last code commit of the step:

- `just check`, in 7 min 22 s, which printed "release self-test passed: 80 cases", "1101 tests run: 1101 passed, 0 skipped", and, under Miri, "589 tests run: 589 passed (6 slow), 512 skipped".
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 12 skipped".
- `test-rhi`, on the NVIDIA adapter and again on lavapipe, which printed "225 tests run: 225 passed, 0 skipped" with 27, 33, and 0 passed doctests for the slang-rhi crates, "311 tests run: 311 passed, 55 skipped" with 22 and 18 passed doctests for `slango` on the backend `rhi`, and "37 tests run: 37 passed, 5 skipped" for the examples package on that backend.
- `check-miri-rhi`, which printed "116 tests run: 116 passed, 58 skipped", and `check-features`, `check-vendored`, and `check-rhi`.
- With Rust 1.100.0, `check-clippy`, `test-gpu`, which printed "1101 tests run: 1101 passed, 0 skipped", and Clippy on the builds of `slango` and of the examples package with `rhi`.

`check-msrv`, `check-semver`, `check-deny`, `check-unused-deps`, `check-vendored-shared-folder`, and `doc-site` passed on the first commit of the step. `check-package` and `publish-dry-run` did not run, because Cargo packages only a committed tree.

A program outside the workspace with `slango` on the backend `rhi` and `slango-macros` with `vendored` built without a Vulkan SDK and ran a typed kernel with an imported module on the NVIDIA adapter and on lavapipe. Its generated struct is 28 bytes, against 32 on the backend `wgpu` (Section 4.3 of `docs/design/slango-macros.md`).

Run 44449874191, for the first commit of the step, passed all ten jobs. On WARP, it printed "225 tests run: 225 passed, 0 skipped" for the slang-rhi crates, "304 tests run: 304 passed, 56 skipped" for `slango` on the backend `rhi`, and "34 tests run: 34 passed, 5 skipped" for the examples package on that backend, which holds the `gpu` tests of the generated structs. So the packed layout holds with DXIL for the tested structures, and embedded loads work with DXIL. The Windows job `vendored` took 13 minutes in all (Section 7.4 of `docs/design/tooling.md`).

Run 44456053283, for the last commit of the step, with the fixes of the review, passed all ten jobs.

The sixth code step of M10, done on 2026-11-19 on the branch `feature/m10-slango-rhi-rendering`, added rendering to the backend `rhi` of `slango` (SLO-41, SLO-45, and Sections 6, 7.6, 7.7, 8.3, and 9.7 of `docs/design/slango.md`):

- On the backend `rhi`, `Texture::new`, `Texture::from_pixels`, `Texture::read`, `Sampler::new`, `Module::render_pipeline`, and the draw work through `slang-rhi`, so the public API works on both backends apart from `slango::wgpu`, and no check of `slango` returns `ErrorKind::Unsupported`. A new depth texture is cleared to 0 by a render pass before `Texture::new` returns (SLO-45), and a compute kernel can sample a texture on that backend too.
- By the owner's decision of 2026-11-19 on SOQ-12, which is decision 7 of Section 4.6 of `docs/design/slango.md`, `slango` supplies a stream itself for a vertex attribute that the shader declares and does not use. The stream has the attribute's format and semantic and the stride 0, over a buffer of 16 zero bytes that the device creates once, so a draw needs the same buffers on both backends. `slang-rhi` now accepts the stride 0 and has the public constant `MAX_VERTEX_STREAMS` (RHI-26 of `docs/design/slang-rhi.md`).
- The merge of the bindings of the two stages takes its key from the backend: the set and the binding on the backend `wgpu`, unchanged with its message, and on the backend `rhi` the set and the binding with the scope and the parameter path, because on DXIL every binding reports the set 0 and the binding 0. On D3D12, a load compiles the vertex entry points with attributes once more to SPIR-V, in the private module `rhi_backend::varying`, to read which vertex inputs are used, because the metadata of DXIL reports every declared input as used (Section 7.6 of that design). On the backend `rhi`, a vertex attribute without a semantic makes its vertex entry point a rejection at load, before any code generation, which `render_pipeline` reports with `UnsupportedParameter`.
- The shaders of `shader-playground` and of one `gpu` render test take their positions from a vertex buffer, and the scene of `shader-playground` draws into a texture of its own on the backend `rhi`. Every skip of the backend `rhi` for textures, samplers, render pipelines, and scenes is gone, and `gpu/unsupported.rs` is deleted.
- `test-rhi` passes `--no-fail-fast` to each command, runs every command after a failing one, names each failed command, and fails at the end (TOOL-53 of `docs/design/tooling.md`).

Section 6.5 of `docs/design/slango.md` states the differences that remain: a semantic on every vertex attribute on `rhi`, the limits of vertex streams over every declared attribute on `rhi`, the two shapes of parameter that slang-rhi cannot address, now for render pipelines too, the depth clamping of Vulkan, and the instance index as `SV_InstanceID`, which naga rejects and the backend `rhi` draws.

The step met no new defect. RKB-2 of `docs/design/slang-rhi.md`, a constant buffer beside a global uniform on D3D12, also fails render pipelines, with the kind `Compile`, and the step extended its row and its entry in `docs/upstream-reports.md`.

The crate does not guard against it, as Hard Rule 14 requires. Two render tests skip the backend `rhi` on Windows, and a new test shares a constant buffer, a parameter block, a texture, and a sampler between the stages without a global uniform.

The step raised SOA-13 of `docs/design/slango.md`, on the second compile to SPIR-V, which does not reproduce the capability options of the device session, and confirmed SOA-10 and SOA-12 of that design for rendering and TA-25 of `docs/design/tooling.md`, and removed their rows.

An adversarial review of the first commit of the step ran about 75 scenarios on the backend `wgpu` and on the backend `rhi` on Vulkan, on the NVIDIA adapter and on lavapipe. The pixels were identical on both backends, the error kinds for bad input were equal, and the Khronos validation layer reported no error, also with synchronization validation. The one difference in behaviour that it found is the instance index above. It found the stride 0 valid in core Vulkan, the bound check of `slang-rhi` right for it, and a buffer that a draw gives for an unused attribute ignored on both backends.

On D3D12, the step fixed three findings after the first commit: the key of the merge, the second compile to SPIR-V, and the check of semantics before any code generation. A test compiled only with `wgpu` covers the vertex and instance indices among the vertex inputs, and the shared API documentation and messages no longer name wgpu where both backends apply.

In the development container, with Rust 1.99.1 unless stated, these checks passed on the last code commit of the step:

- `test-gpu`, which printed "1110 tests run: 1110 passed, 0 skipped".
- `test-rhi`, on the NVIDIA adapter and again on lavapipe, which printed "226 tests run: 226 passed, 0 skipped" with 27, 33, and 0 passed doctests for the slang-rhi crates, "379 tests run: 379 passed, 1 skipped" with 22 and 18 passed doctests for `slango` on the backend `rhi`, and "47 tests run: 47 passed, 0 skipped" for the examples package on that backend.
- `check-rhi`, and with Rust 1.100.0 `check-clippy` and Clippy on the builds with `rhi`.

On the commit before the last three code commits, `just check` printed "release self-test passed: 80 cases", "1109 tests run: 1109 passed, 0 skipped", and, under Miri, "589 tests run: 589 passed (14 slow), 520 skipped". `check-features`, `check-vendored`, and `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 12 skipped", passed there, and `check-miri-rhi` printed "116 tests run: 116 passed, 59 skipped". `check-msrv`, `check-semver`, `check-deny`, `check-unused-deps`, and `check-vendored-shared-folder` passed on the first commit.

On the NVIDIA adapter, the scenes of both windowed examples drawn on the backend `rhi` were byte for byte the same as on the backend `wgpu`: the checkerboard quad of `windowed-renderer` and the colour gradient of `shader-playground`.

Run 44464198896, for the first commit of the step, failed on WARP in the test of a texture that both stages sample, after 307 passed tests, with `UnsupportedParameter` and "... at set 0, binding 0 with different layouts", which led to the key of the merge. Run 44470355051, after the key and the second compile, printed "377 tests run: 374 passed, 3 failed, 1 skipped" for `slango` on the backend `rhi` on WARP: two render tests of RKB-2 and one attribute without a semantic, while the tests of unused attributes passed.

Run 44474920321, for the last code commit and before the documents, passed all ten jobs. On WARP, it printed "226 tests run: 226 passed, 0 skipped" for the slang-rhi crates, "376 tests run: 376 passed, 3 skipped" for `slango` on the backend `rhi`, and "46 tests run: 46 passed, 0 skipped" for the examples package on that backend, and the job `vendored` on Windows took 16 min 38 s. So the conventions, the stream of stride 0, the second compile, the scopes, both scenes, and the test of depth clipping hold on D3D12.

`check-package` and `publish-dry-run` did not run, because Cargo packages only a committed tree. Run 44479647162, for the last commit of the step, with its documents, passed all ten jobs.

The seventh code step of M10, done on 2026-11-19 on the branch `feature/m10-close`, wrote the documentation that the exit criteria of M10 name and closed the milestone. It changed documentation comments, READMEs, and documents, and three texts outside them that had become false:

- The crate-level documentation of `slango` gained the sections "Choosing a backend" and "Changes from 0.1.0-alpha.1", whose content Section 4.7 of `docs/design/slango.md` states, and its `# Features` section points to the first. The item documentation of `Device`, `Device::new`, `Device::load_module`, `Module::render_pipeline`, `RenderPipeline`, `ErrorKind`, and the accessors of `slango::wgpu`, which described wgpu alone or a behaviour of the published release `0.1.0-alpha.1`, now states each backend.
- The crate documentation of `slango-macros` states that the generated `Shader`, kernel structs, and arguments structs are neither `Send` nor `Sync`, where the generated code builds on the backend `rhi`, and the dependencies of a program on that backend without a Vulkan SDK. That of `slango-codegen` states what the generated code embeds on each backend, and that of `slang-slint` that the crate cannot sit in a program on the backend `rhi`.
- The crate documentation of `slang-rhi` points most programs to `slango` and states two more defects of slang-rhi that a program can meet on D3D12, RKB-2 and RKB-3 of `docs/design/slang-rhi.md`.
- The READMEs of `slango`, `slango-macros`, `slango-codegen`, `slang-slint`, and `slang-rhi`, and the root `README.md`, name both backends of `slango`, and Table 4 and Section 5.2 of `docs/writing-readmes.md` describe `slango` and `slang-rhi` in the same way.
- Section 13.7 of `docs/design/slango.md` states the tests of the backend `rhi` by subject, and Sections 3.11, 6.4, and 10 of that design and Section 4.2 of `docs/design/slango-macros.md` no longer hold the history of the code steps.
- The `reason` strings of three lint exceptions in `crates/slango/src/wgpu.rs`, the `description` of `slango` in its manifest, which now says that the crate runs kernels and draws "through wgpu or slang-rhi", and the names of four steps of the CI job `vendored`, which now name what the steps run.

On 2026-11-19 in the development container, with Rust 1.99.1 unless stated, these checks passed on the final code of the step:

- `just check`, which printed "release self-test passed: 80 cases", "1110 tests run: 1110 passed (1 slow), 0 skipped", and, under Miri, "589 tests run: 589 passed (14 slow), 521 skipped".
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 12 skipped".
- `test-rhi`, on the NVIDIA adapter and again on lavapipe, which printed "226 tests run: 226 passed, 0 skipped" with 27, 33, and 0 passed doctests for the slang-rhi crates, "379 tests run: 379 passed, 1 skipped" with 22 and 18 passed doctests for `slango` on the backend `rhi`, and "47 tests run: 47 passed, 0 skipped" for the examples package on that backend.
- `check-miri-rhi`, which printed "116 tests run: 116 passed, 59 skipped".
- `check-doc`, `check-doctest`, `check-rhi`, `check-features`, `check-msrv`, `check-semver`, `check-deny`, `check-unused-deps`, `check-vendored`, `check-vendored-shared-folder`, `check-workflows`, `check-workflow-pins`, and `doc-site`, whose site is 46 MB.
- With Rust 1.100.0, `check-clippy`, `check-doctest`, and Clippy on the builds with `rhi` of `slango` and the examples package.

The documentation of every changed crate built with warnings denied on both backends, so the new intra-doc links resolve. `check-package` and `publish-dry-run` did not run, because Cargo packages only a committed tree. When this was written, the step had no CI run, and Hard Rule 11 lets it reach `main` only after its CI run succeeds.

These open questions, unverified assumptions, and recorded upstream defects stay open after M10, with no milestone assigned unless one is named:

- SOQ-11 and the assumptions SOA-6 and SOA-13 of `docs/design/slango.md`, and SOQ-3 of that design, which waits for larger kernels. SOQ-5 to SOQ-8 of that design wait for the first release that is not an alpha.
- SLMQ-5 and the assumptions SLMA-3, SLMA-5, and SLMA-7 of `docs/design/slango-macros.md`, and SLMQ-1 and SLMQ-2 of that design. SLMQ-3 and SLMQ-4 wait for the first release that is not an alpha.
- RHQ-1 to RHQ-3 and RHQ-5 to RHQ-12 of `docs/design/slang-rhi.md`, and what the tests of `slang-rhi` do not verify, which Section 10.4 of that design lists.
- RSQ-4 of `docs/design/slang-rhi-sys.md`, which the owner decides before the second release, RSQ-5, and the assumption RSA-8 of that design.
- TQ-19 of `docs/design/tooling.md`, which the owner decides before the next release, TQ-11 to TQ-14 of that design, assigned to M11, TQ-17 and TQ-18, and the assumptions TA-17, TA-18, and TA-20 to TA-23 of that design.
- The assumption SLSA-3 of `docs/design/slang-slint.md`.
- SA-21, SA-22, SA-24, and SA-25 of `docs/design/slang-compiler-sys.md`, and CQ-14 of `docs/design/slang-compiler.md`, with CQ-13 of that design assigned to M17.
- SRCQ-4 of `docs/design/slang-src.md`, beside the questions of M7 that the paragraph after M8 names.
- The defects RKB-1 to RKB-9 of `docs/design/slang-rhi.md` and KB-2 to KB-7 of `docs/design/slang-compiler.md`, and the request REQ-1, none of which is reported upstream. Of the defects of Slang, only KB-1 is reported. Each has its entry in `docs/upstream-reports.md`.

On 2026-11-19, the owner decided that Miri runs only on the crates that hold unsafe code that it can execute, which TEST-12 of the coding standard now states. The workspace lint table denies `unsafe_code`, so in every other crate Miri found only what the native run already exercises, and those tests took most of its time. The change of that day did these things:

- `check-miri` runs only the packages of the new `justfile` variable `miri_packages`, which is `slang-compiler`, and fails when a package has no test that Miri runs. `check-miri-rhi` still runs `slang-rhi`.
- Miri no longer runs the tests of `slang-bindgen`, `slang-compiler-sys`, `slango`, `slango-codegen`, `slango-macros`, `slang-slint`, and the examples package. They all still run natively.
- The new checker `check-miri-packages`, which `just check` runs with its self-test, fails when a crate that opts out of `unsafe_code` is in no Miri recipe and is not exempt by name, and when a Miri package is not a workspace member or holds no unsafe code (Section 6.11 of `docs/design/tooling.md`).
- The 26 `#[cfg_attr(miri, ignore)]` attributes of `slango-codegen` are gone, and no other attribute for Miri alone exists in the workspace.

On 2026-11-19 in the development container, `just check` passed in 4 min 47 s, against 7 min 22 s after the fourth code step of M10. It printed "check-miri-packages self-test passed: 5 cases", "release self-test passed: 80 cases", "1110 tests run: 1110 passed, 0 skipped", and, under Miri, "203 tests run: 203 passed, 95 skipped", all of them tests of `slang-compiler`, in about 39 s. Before the change, Miri ran 589 of the 1110 tests and skipped 521. `check-miri-rhi` printed "116 tests run: 116 passed, 59 skipped", and `test-gpu`, `check-shell`, and `check-clippy` with Rust 1.100.0 passed. The decision answers SRCQ-3 of `docs/design/slang-src.md`, which asked which recipe runs the pure tests of `slang-src` and `slang-glslang-sys` under Miri, with none. When this was written, the change had no CI run, and Hard Rule 11 lets it reach `main` only after its CI run succeeds.

The design of M11, the second release (Section 15.14 of `docs/design/architecture.md`), exists since 2026-11-21, and the first two of its four code steps are done. It consists of these parts, within the owner's decisions of 2026-11-19, which Section 9.10.1 of `docs/design/tooling.md` records:

- `docs/design/tooling.md`, Section 9.10, with the changed TOOL-35, TOOL-37, TOOL-38, and TOOL-54, and the new TOOL-56. The release publishes eleven crates at `0.2.0-alpha.1`, with `slang-src` at `0.2.0-alpha.1.slang.2026.17.0.25.gc7954ebd1`, and `0.1.0-alpha.2` is skipped and never published.
- `docs/design/slang-rhi-sys.md`, the changed RSYS-16, for the statement of the third-party code of the crate with its licences.
- `docs/design/architecture.md`, Sections 13 and 15.14.

Section 9.10.7 of `docs/design/tooling.md` orders the four code steps and says who performs each. The design answers TQ-19 of `docs/design/tooling.md` and RSQ-4 of `docs/design/slang-rhi-sys.md`. It extends the review of the public API to the additions of M9 to `slang-compiler` and `slang-bindgen`, which answers TQ-20 of `docs/design/tooling.md`, and raises the assumptions TA-27 to TA-30 of that design. TQ-11 to TQ-14 of that design no longer have a milestone.

The third code step, the release itself, and the fourth, the change that follows it, are not done. Nothing of the second release is published, tagged, or deployed.

The first code step of M11, done on 2026-11-21 on the branch `feature/m11-api-review`, ran the review of the public API of decision 4 of Section 9.10.1 of `docs/design/tooling.md`, with the additions of M9, and fixed the findings that need no decision of the owner:

- The review covered `slang-rhi-sys`, `slang-rhi`, and `slang-slint`, the changes of M10 to `slango`, `slango-codegen`, and `slango-macros`, and the additions of M9 to `slang-compiler` and `slang-bindgen`, none of which a review had covered. It had 27 findings, 11 that need no decision of the owner and 16 that do.
- In `slang-rhi`, `Buffer`, `Texture`, `Sampler`, `ShaderProgram`, `ComputePipeline`, `RenderPipeline`, `Queue`, and `CompileSession` implement `Clone`, which adds a reference to the same object, `Device::info` returns `&DeviceInfo`, `Buffer::usage` is public, `Operation::BindPipeline` is `Operation::BindComputePipeline`, `BlendMode` is `Blend`, and `Device::create_slang_session` takes any sequence of paths (Sections 6.4, 6.6, 6.8, and 11 of `docs/design/slang-rhi.md`). The crate is not published, so these changes break no user.
- The backend `rhi` of `slango` holds the handles of `slang-rhi` directly instead of in an `Rc`, and reads the `DeviceInfo` of its device instead of a copy (Section 6.1 of `docs/design/slango.md`). The documentation of `ErrorKind::Unsupported` and its bullet in the section "Changes from 0.1.0-alpha.1" say that only a build with `rhi` returns the kind, when slang-rhi or the device does not support what a call asked (SLO-42). The public API of `slango` did not change.
- The crate documentation of `slang-slint` gained the section "The version of Slint", and its README states the requirement of a program on `slint` (Section 4.1 of `docs/design/slang-slint.md`). The crate documentation and the README of `slang-rhi-sys` state that `slang-compiler-sys` is a public dependency and how a program depends on it (Section 10 of `docs/design/slang-rhi-sys.md`).
- DEP-1 of the coding standard permits backend features that exclude each other, one of them on by default, within five bounds, which records the owner's decision 5 of Section 4.6 of `docs/design/slango.md`. Before the step, its exception covered only a crate whose one backend sits behind a default feature, which the two backend features of M10 contradicted.
- The eleventh finding that needs no decision, the release metadata of the six crates that then set `publish = false`, belongs to the second code step (Section 9.10.2 of `docs/design/tooling.md`).
- `slang-rhi` gained a unit test of the counting of a clone, which Miri runs, and two `gpu` tests in which clones of the handles run a dispatch and a draw after their originals are dropped (Section 10 of `docs/design/slang-rhi.md`).

The 16 findings that need the owner's decision are RHQ-13 to RHQ-20 of `docs/design/slang-rhi.md`, RSQ-6 and RSQ-7 of `docs/design/slang-rhi-sys.md`, SLSQ-1 and SLSQ-2 of `docs/design/slang-slint.md`, SLMQ-6 of `docs/design/slango-macros.md`, CQ-15 of `docs/design/slang-compiler.md`, and SQ-8 and SQ-9 of `docs/design/slang-compiler-sys.md`, which Section 9.10.1 of `docs/design/tooling.md` lists by crate. By decision 4, they do not block the second release, an alpha, and wait for the first release that is not an alpha.

Two of them matter most to a user of `slang-rhi`, and on 2026-11-21 they were put to the owner, who has not answered yet. RHQ-13 is that the closures of `compute_pass` and `render_pass` must return the crate's `Result`, whose `Error` has no public constructor, so a caller cannot use `?` on its own errors inside a pass. RHQ-14 is that `bind_pipeline` returns a root object that borrows the pass mutably, so the caller drops it before `dispatch` or `draw`.

The review ran cargo-semver-checks with `--release-type minor` against crates.io. It printed "202 checks: 202 pass, 58 skip" for each of `slang-bindgen`, `slang-compiler`, and `slango-codegen`. For `slango`, one check failed, `auto_trait_impl_removed`, for exactly the nine types that lost `Send` and `Sync` by the owner's decision of 2026-11-18 (SLO-17 of `docs/design/slango.md`), which is not a finding.

On 2026-11-21 in the development container, with Rust 1.99.1 unless stated, these checks passed on the step:

- `just check`, which printed "release self-test passed: 80 cases", "1110 tests run: 1110 passed, 0 skipped", and, under Miri, "203 tests run: 203 passed, 95 skipped". The counts did not change, because every recipe of `just check` excludes `slang-rhi`.
- `test-rhi`, on the NVIDIA adapter and again on lavapipe, which printed "229 tests run: 229 passed, 0 skipped" with 27, 33, and 0 passed doctests for the slang-rhi crates, "379 tests run: 379 passed, 1 skipped" with 22 and 18 passed doctests for `slango` on the backend `rhi`, and "47 tests run: 47 passed, 0 skipped" for the examples package on that backend.
- `check-miri-rhi`, which printed "117 tests run: 117 passed, 61 skipped".
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 12 skipped".
- `check-semver`, `check-rhi`, `check-vendored`, and `check-features`.
- With Rust 1.100.0, `check-clippy`, Clippy on the builds with `rhi`, `test-rhi`, and `check-doctest`.

These are not verified: any build or test on Windows, and the counting of the derived `Clone` of each handle type under Miri, which a unit test checks at `Owned` and the two `gpu` tests check natively. `check-package` and `publish-dry-run` did not run, because Cargo packages only a committed tree. When this was written, the step had no CI run, and Hard Rule 11 lets it reach `main` only after the CI run of its last commit succeeds.

The second code step of M11, done on 2026-11-21 and 2026-11-22 on the branch `feature/m11-release-prep`, prepared the second release of Sections 9.10.2 to 9.10.6 of `docs/design/tooling.md`, and published, tagged, and deployed nothing:

- Every member has the version `0.2.0-alpha.1`, and `slang-src` has `0.2.0-alpha.1.slang.2026.17.0.25.gc7954ebd1`. Every requirement between members is exact at the new version, and `Cargo.lock`, the examples of versions in the API documentation of `slang-src`, `slang-compiler-sys`, `slango`, and `slango-macros`, each crate README, and the illustrations of the design documents name it. The root `README.md` keeps naming the published `0.1.0-alpha.1` (RDM-33 of `docs/writing-readmes.md`).
- `slang-rhi-sys`, `slang-rhi`, `slango`, `slango-codegen`, `slango-macros`, and `slang-slint` no longer set `publish = false`, so the examples package is the only member that sets it. `slang-rhi-sys`, `slang-rhi`, and `slang-slint` gained the `documentation` field, and the READMEs of the six crates take the form of a published crate, with dependencies from crates.io and links to the hosted pages.
- `release_packages` holds the eleven crates in the order of Section 9.10.2 of `docs/design/tooling.md`, and `check-semver` passes `--exclude slang-slint`, because crates.io holds no version of it until the release (Section 9.10.5 of that design). The rule TOOL-54 of that design is retired.
- The job `build` of the documentation workflow installs the Xlib headers, lavapipe, and the Vulkan loader with a verbatim copy of step 8 of the job `vendored`, whose condition in both workflows is now `runner.os == 'Linux'` (TOOL-56).
- The crate documentation of `slang-rhi-sys` names the third-party code that a program built with it contains, VMA or the D3D12 Memory Allocator, CMakeRC, and OffsetAllocator, each under the MIT licence with its copyright notice and what a distributor carries (RSYS-16 of `docs/design/slang-rhi-sys.md`). OffsetAllocator was not in the first design, and the step found it compiled into slang-rhi on both platforms.

The step confirmed RSA-8 of `docs/design/slang-rhi-sys.md` and TA-27 to TA-30 of `docs/design/tooling.md`, and removed their rows. Section 3.5 of the first design states what the clone of slang-rhi holds, with each version, licence, and notice, and Sections 9.2, 9.10.2, 9.10.4, and 9.10.5 of the second state the results of the other four. The step raised RSQ-8 of `docs/design/slang-rhi-sys.md`, on `src/core/stable_vector.h` of slang-rhi, which is compiled on Windows, says that it is based on another project, and carries no licence notice. It waits for the owner. Section 3.5 of that design also records that the REUSE file `.reuse/dep5` of slang-rhi gives VMA, the D3D12 Memory Allocator, and CMakeRC the licence of slang-rhi.

Run 44961161884, on the branch `feature/m11-documentation-test` from the last code commit of the step, ran the job `build` of the documentation workflow up to `just doc-site` and concluded `success` in 13 min 24 s, against its timeout of 60 minutes, with a site of 47 MB (Section 9.10.4 of `docs/design/tooling.md`). That branch was deleted on `origin` afterwards.

In the development container, with Rust 1.99.1 unless stated, these checks passed on the committed code of the step:

- `just check`, which printed "release self-test passed: 80 cases", "check-miri-packages self-test passed: 5 cases", "1110 tests run: 1110 passed (1 slow), 0 skipped", and, under Miri, "203 tests run: 203 passed, 95 skipped".
- `test-rhi`, on the NVIDIA adapter and again on lavapipe, which printed "229 tests run: 229 passed, 0 skipped" with 27, 33, and 0 passed doctests for the slang-rhi crates, "379 tests run: 379 passed, 1 skipped" with 22 and 18 passed doctests for `slango` on the backend `rhi`, and "47 tests run: 47 passed, 0 skipped" for the examples package on that backend.
- `check-miri-rhi`, which printed "117 tests run: 117 passed, 61 skipped".
- `test-vendored`, which printed "453 tests run: 453 passed, 0 skipped" and then "4 tests run: 4 passed, 12 skipped".
- `check-semver`, which took 114 s and printed "0 checks: 0 pass, 260 skip" and "no semver update required" for each of `slang-bindgen`, `slang-compiler`, `slang-compiler-sys`, `slango`, and `slango-codegen`.
- `check-package`, which printed "check-package: every package passed" and, for each of the eleven crates, that crates.io does not hold its version `0.2.0-alpha.1`.
- `publish-dry-run`, which took 1166 s with a fresh build of Slang, slang-glslang, and slang-rhi, printed "Uploading" and "warning: aborting upload due to dry run" for each of the eleven crates and no other warning, and recorded the last code commit and the eleven names.
- `doc-site`, which built a site of 47 MB with no warning, whose index page links all eleven crates.
- `check-features`, `check-msrv`, `check-deny`, `check-unused-deps`, `check-vendored`, `check-rhi`, `check-vendored-shared-folder`, `check-workflows`, `check-workflow-pins`, `check-shell`, and `check-toml`, and `check-clippy` with Rust 1.100.0.

`check-release` exited with status 1 and reported that crates.io holds none of the eleven crates at `0.2.0-alpha.1`, which is the state before the release. The order of the publish is `slang-bindgen slang-src slang-compiler-sys slang-glslang-sys slang-compiler slang-rhi-sys slang-rhi slango slango-codegen slango-macros slang-slint`, and Section 9.10.2 of `docs/design/tooling.md` states how the dry run checked it.

Of the preconditions of Section 9.10.7 of `docs/design/tooling.md`, these are met: RSA-8 with the crate documentation, the review of the first code step, the answer 404 of the crates.io API for `slang-rhi-sys`, `slang-rhi`, and `slang-slint`, the Xlib headers in the development container, and the run of TOOL-56. The CI job `semver` of the last commit of `main` has not run on the new versions, because that waits for the push and the merge of the step.

These are not verified: any build or test on Windows, where the facts of the D3D12 Memory Allocator come from a reading of the tree, and the CI job `semver` and the rest of CI on the commit of the step. When this was written, the step had no CI run, and Hard Rule 11 lets it reach `main` only after the CI run of its last commit succeeds.

The next step, the third, is the release itself, steps 1 to 10 of Section 9.6 of `docs/design/tooling.md` on the commit of `main` that holds this step. It waits for the owner's go-ahead, and only the owner publishes (TOOL-46 of that design).

On 2026-11-17, the advisory database gained RUSTSEC-2026-0543, which reports `chrono` as unmaintained, with no upgrade available. In CI run 44090478850, on a branch that changed no dependency, `check-deny` failed with it in the job "Run every check on Linux", and `check-deny` failed the same way on `main`. The owner decided that day to ignore the advisory in `deny.toml`, and Section 8 of `docs/design/tooling.md` records the reason.

What exists and works:

- The `justfile` and its recipes, which `just --list` shows. They include these recipes:
  - The recipes that run the Rust checks, which pass no `--all-features`.
  - `check-slang-call-guard` generates the bindings with the binary of `slang-bindgen` into `target/slang-bindgen/` and then runs the checker on them.
  - `fmt` and `check-fmt` format and check every Rust file that git tracks or does not ignore.
  - `install-vulkan-sdk` installs the Vulkan SDK on a CI runner, and `check-vulkan-sdk` checks an installed SDK against the build tag that `recorded-slang` prints.
  - `recorded-slang` prints the recorded commit, the build tag, and the name of the clone entry of `slang-src`.
  - `test-vendored` and `check-vendored` test, lint, and document `slang-src`, `slang-glslang-sys`, `slang-compiler-sys`, and `slang-compiler` with the feature `slang-compiler/vendored` in `target/vendored`, so their first run there clones Slang and builds Slang and slang-glslang from source. `test-vendored` then runs the tests of the modules `bindings` and `structs` of `slango-macros` with only `slango-macros/vendored`, and `check-vendored` runs Clippy on `slang-slint` with only `slang-slint/vendored`, which check the forward of the feature by each of the two crates (TOOL-7 of `docs/design/tooling.md`).
  - `check-rhi` and `test-rhi` lint, document, and test the packages of `rhi_packages`, `slang-rhi-sys` and `slang-rhi`, then `slango` with `--no-default-features --features rhi`, and then the examples package with `--no-default-features --features rhi,vendored`, with no filter of nextest, on the vendored route in `target/vendored`, on Linux and on x86_64 Windows, where `test-rhi` needs `dxcompiler.dll` on `PATH` (TOOL-52 and TOOL-53 of `docs/design/tooling.md`).
  - `check-miri` runs under Miri the tests of the packages of the `justfile` variable `miri_packages`, now only `slang-compiler`, and fails when a package has no test that Miri runs (TEST-12 of the coding standard).
  - `check-miri-rhi` runs the 117 unit tests of `slang-rhi` under Miri in `target/miri-rhi`, with its own clone and native build of Slang, on Linux only, outside `just check` and CI (TOOL-52).
  - `check-vendored-shared-folder` checks that the shared folder holds exactly one clone, the complete entry that `recorded-slang` names, and that no other directory under `target/vendored` holds a clone. It recognizes a clone in the layout of SRC-25 of `docs/design/slang-src.md` and in the two earlier layouts with the full commit hash, `src/<commit>-<submodules>` and `src/<commit>`. It prints the sizes of the folder and of the clone (Section 3.2 of `docs/design/tooling.md`).
  - The recipes of the release, which Section 9.6 of `docs/design/tooling.md` specifies. `check-package` checks the members without `publish = false`, the `documentation` fields, and each packaged `.crate` file. `check-release` checks a release on crates.io. `doc-site` builds the hosted documentation of the crates of `release_packages` on the vendored route into `target/vendored/doc`, with the features `vendored` that it reads from `cargo metadata` (TOOL-41). `publish-dry-run` runs Cargo's dry run and records the commit and the crates. `publish`, `tag-release`, and `yank` publish, tag, and yank. `check-package`, `publish-dry-run`, and `publish` exist on Linux and macOS only.
  - `check-semver` runs cargo-semver-checks on each crate of the release with a library target, except `slang-src` and `slang-glslang-sys`, against its last release on crates.io, and leaves out `slang-slint`, of which crates.io holds no version until the second release is published, so it checks `slang-bindgen`, `slang-compiler`, `slang-compiler-sys`, `slango`, and `slango-codegen`. While the releases are alphas, it runs none of the tool's checks of the API (Sections 9.3 and 9.10.5 of `docs/design/tooling.md`).
  - `release-scripts-self-test` runs the scripts of the release, and four of its recipes, in 80 cases against stub programs of `cargo`, `git`, `gh`, `curl`, `sleep`, and `date`, so it reaches neither crates.io nor GitHub (Section 9.9 of `docs/design/tooling.md`).

  The variables `release_packages` and `documentation_site` of the `justfile` hold the crates of the next release and the base address of the hosted documentation, which every recipe of the release reads (TOOL-35 and Section 9.4 of `docs/design/tooling.md`). `release_packages` holds the eleven crates of the second release in the order of Section 9.10.2 of that design.

  The variables `vendored_packages` and `vendored_features` of the `justfile` hold the packages and the feature of the vendored recipes (TOOL-7). Every recipe that passes `--workspace` also passes `--exclude slang-src --exclude slang-glslang-sys --exclude slang-rhi-sys --exclude slang-rhi`. `check-features` passes `--exclude-features vendored`, and checks `slango` and the examples package in a second command with `--exclude-features vendored,rhi` and `--exclude-no-default-features` (TOOL-48 of `docs/design/tooling.md`).

  Its variable `slang_library_path` holds the assignment that puts `$VULKAN_SDK/lib` on the library path of Linux or macOS. The recipes that run compiled code which loads Slang put it at the start of their command line (SCS-32 of `docs/design/slang-compiler-sys.md`).
- The scripts under `scripts/`, which the recipes and hooks run, including `install-vulkan-sdk` and `check-vulkan-sdk`, and the custom checkers of Section 6 of `docs/design/tooling.md`. These are `check-rust-comments`, `check-doc-headings`, `check-doctest-attributes`, `check-feature-docs`, `check-slang-call-guard`, `check-workflow-pins`, `check-doc-references`, and `check-miri-packages`, the extension of `check-prose` to the documentation comments of Rust files, and a self-test recipe for each. They also include the scripts of the release, `check-package`, `check-release`, `check-doc-site`, `publish-dry-run`, `publish`, `tag-release`, and `yank`, and their self-test `self-test-release`.
- The git hooks in `.pre-commit-config.yaml`.
- The documentation workflow `.github/workflows/documentation.yml`, which runs only on a push of a tag that matches `v*`. Its job `build` installs the Xlib headers, lavapipe, and the Vulkan loader where the runner lacks them (TOOL-56), runs `just doc-site` on a fresh checkout, and uploads the site, and its job `deploy`, the only job with `pages: write` and `id-token: write`, deploys it to GitHub Pages (Section 7.5 of `docs/design/tooling.md`). Its first run, for the tag `v0.1.0-alpha.1`, deployed the site of the first release, as the paragraphs on the publish above state.
- The CI workflow `.github/workflows/ci.yml`, which runs on every push to `main`, every push to a branch that matches `feature/**`, and every pull request.
  - Every job makes libclang available, and every job except `vendored` installs the Vulkan SDK of the version that the workflow records, verified against a SHA-256 checksum that the workflow records, and checks it with `check-vulkan-sdk`, on Linux, Windows, and macOS. The job `coverage` measures test coverage on Linux and keeps the report as an artifact. Section 7 of `docs/design/tooling.md` specifies the jobs.
  - The job `semver`, added on 2026-11-17, runs `check-semver` on Linux in every run of the workflow, with the Vulkan SDK and libclang and without an adapter or fontconfig.
  - The job `vendored`, added on 2026-11-15, installs no Vulkan SDK and sets up no developer environment of the Microsoft toolchain.
    - It makes libclang available, installs CMake, Ninja, `g++`, `python3`, `pkg-config`, and `libfontconfig-dev` with apt on Linux where the runner lacks them, and prints the versions of CMake, Ninja, and, outside Windows, the C++ compiler. The development files of fontconfig serve the Clippy run on `slang-slint` of `check-vendored`, which builds Slint.
    - On Linux, Windows, and macOS, it prints the Python 3 that it finds as `python3` or `python` on `PATH`, and fails with an annotation when it finds none.
    - It runs `check-vendored` on Linux, and `test-vendored` and `check-vendored-shared-folder` on Linux, Windows, and macOS, and times both vendored recipes (Section 7.4 of `docs/design/tooling.md`).
    - On Linux, it also installs `libx11-dev`, `mesa-vulkan-drivers`, and `libvulkan1` where the runner lacks them. On Linux and Windows, it installs Clippy and runs `check-rhi` before `test-vendored` and `test-rhi` after it, under `time`, so it also lints and tests `slango` on the backend `rhi`. The names of those steps name what they run. In run 44398417125, the job on Windows took 15 minutes in all.
    - On Windows, before `test-rhi`, it puts the directory of the `dxcompiler.dll` of the highest versioned Windows SDK of the runner on `PATH`, prints its path, and fails with an annotation when it finds none, so the version of DXC follows the runner image. The job has a timeout of 90 minutes.
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
- The workspace manifest `Cargo.toml`, with `default-members`, which leaves out `slang-src`, `slang-glslang-sys`, `slang-rhi-sys`, and `slang-rhi`, the `[workspace.package]` fields that the members inherit, and the workspace lint tables of Section 4.2 of the coding standard.
- The manifests of the eleven crates of the second release, each with the `documentation` field of TOOL-40 of `docs/design/tooling.md`, which names its page on the hosted site, and none with `publish = false`. The pages of `slang-rhi-sys`, `slang-rhi`, and `slang-slint` exist only once the site of the second release is deployed.
- The tool configuration: `clippy.toml`, `deny.toml`, `.config/nextest.toml`, `taplo.toml`, and `typos.toml`.
- `crates/slang-compiler-sys`, with publication metadata and crate-level documentation that states the safety contract that every generated function shares, both routes, the need for libclang, and the common build errors. It also states that every crate between a program and it forwards the feature `vendored`, that the feature of `slang-compiler` installs the loader of `slang-glslang-sys`, and how a direct user installs that loader (SCS-37 of `docs/design/slang-compiler-sys.md`).
  - Its one Cargo feature, `vendored`, off by default, enables the optional dependency `slang-src` and the optional build dependency `find-msvc-tools`. `slang-bindgen` is its build dependency, and `slang-bindgen` and `find-msvc-tools` are its development dependencies (SCS-61 of `docs/design/slang-compiler-sys.md`). The crate root names `slang-src` with `use slang_src as _;`, because the build script needs only its `links` metadata (DEP-2 of the coding standard).
  - Its root includes the raw bindings that the build script generated into `OUT_DIR` and re-exports them. It also holds the hand-written `BuildInfo`, `Linkage`, and the constant `BUILD_INFO`, whose values the build script writes into `OUT_DIR/build_info.rs`.
  - Its build script `build.rs`, with its logic in `route.rs` and its real environment in `system.rs`, takes the vendored route with the feature and the dynamic route without it.
  - The dynamic route reads `VULKAN_SDK`, reads the build tag of the Slang headers there and compares it with nothing, and checks the library directory and the library file. It then checks that the target is 64-bit, warns on a target that is not one of the four tested targets, generates the bindings with `slang-bindgen` for `TARGET`, writes `bindings.rs`, `symbols.rs`, and `build_info.rs` into `OUT_DIR`, and links the Slang shared library of that SDK dynamically.
  - The vendored route never reads `VULKAN_SDK`. It reads the source tree, its commit, its build tag, and the shared folder from the `links` metadata of `slang-src`, refuses cross-compilation and a target without a link rule for its C++ runtime library, and probes Ninja. With the Microsoft toolchain it finds `cl.exe` with `find-msvc-tools`, and elsewhere it probes the compiler that `CXX` names, or `c++` when `CXX` is unset or empty, by running it with `--version`.
  - It then uses a complete entry of the shared folder whose inputs match, or copies the tracked files of the tree into `OUT_DIR`, builds a minimal static Slang with CMake and Ninja, verifies it, and stores it as a new entry. It generates the bindings from the headers of the entry, links six static libraries, the C++ standard library, and the system libraries of the target, and records `Linkage::Static` and the commit in `BUILD_INFO` (Section 9.3 of `docs/design/slang-compiler-sys.md`).
  - Its integration-test binary `tests/sys/` tests the build information of the route that the feature selects, tests every step and failure of both routes of the build script against a fake environment, and tests the real environment of `system.rs`, including the lock, the lookup of the Microsoft compiler, and the mapping of each kind of generator failure. It also holds the smoke test, which creates a global session and checks its build tag against `BUILD_INFO`, and the native-symbol test, which references every C function of the generated list.
  - It has 75 tests, and Miri runs none of them, because only the tests in modules named `ffi` execute its unsafe code (TEST-12 of the coding standard).
  - Its crate-level section `# Safety` states, besides the shared contract, the rule for a program that implements a Slang interface in Rust: each function of the vtable is defined `extern "system"` and stored in its `"system-unwind"` field through a conversion of the function pointer, so that a panic aborts at the boundary instead of unwinding into Slang.
  - On the dynamic route, the smoke and symbol tests have passed with generated bindings on Linux, Windows, and macOS, the last two in run 43719234533.
  - On the vendored route, the crate's tests and doctests have passed through `test-vendored` on x86_64 Linux in the development container and, in run 43727977282, before it linked system libraries, on the Linux, Windows, and macOS runners. A program outside the workspace built with the feature in the development container on 2026-11-15 ran without `VULKAN_SDK` and without a library path, and its release binary linked only the C++ and C runtime libraries of the system.
- `crates/slang-src`, the crate that obtains the Slang source tree for a vendored build, with publication metadata, its own licence `Apache-2.0 WITH LLVM-exception`, no dependencies, no API, and crate-level documentation that states what a vendored build needs and costs and where the shared folder is.
  - Its build script `build.rs`, with its logic in `source.rs` and its real host in `system.rs`, shallow-clones the recorded commit of `recorded.rs` with its eleven required submodules into the clone entry `src/<prefix>-<submodules>` of the shared folder `slang-src` of the Cargo target directory, once for each target directory, with the first 23 digits of the commit as `<prefix>`. The constant `RECORDED_CLONE_ENTRY` records the name of the entry, and the build fails when it differs from the name that the build computes (SRC-25 and SRC-11 of `docs/design/slang-src.md`). After the clone, it stops the build when git left a tracked file out of the work tree (SRC-26). It exports the path of the tree, the commit, the build tag, and the shared folder as `links` metadata (SRC-1 of `docs/design/slang-src.md`).
  - `slang-compiler-sys` depends on it with the feature `vendored`, `slang-glslang-sys` depends on it always, and `slang-rhi-sys` depends on it on Linux with the GNU C library and on x86_64 Windows with the Microsoft toolchain. An agent can build, lint, and test the crate through the vendored recipes and print the recorded Slang with `recorded-slang`, as Validating Changes states.
  - Its integration-test binary `tests/slang_src/` holds pure tests against a fake host and `ffi` tests of the real host, one of which checks the clone entry that the build script of the same build wrote (Section 12 of `docs/design/slang-src.md`).
  - On 2026-11-15 in the development container, before `slang-compiler-sys` depended on it, the first run of `check-vendored` in an empty `target/vendored`, which cloned, took 39 s, and the clone occupied 285 MB.
  - Its tests have passed on Linux, Windows, and macOS in the CI job `vendored`.
- `crates/slang-glslang-sys`, the crate that builds slang-glslang for a vendored build, with publication metadata, the licence `Apache-2.0 WITH LLVM-exception`, no features, `slang-compiler-sys` with the feature `vendored` and `slang-src` as its dependencies, and `find-msvc-tools` as its build dependency and development dependency (SGL-1 of `docs/design/slang-glslang-sys.md`).
  - Its build script `build.rs`, with its logic in `build_logic.rs` and its real host in `system.rs`, builds the wrapper project `native/CMakeLists.txt` with CMake and Ninja out of tree in `OUT_DIR`, reading the tree of `slang-src` in place. It builds glslang, SPIRV-Tools, Slang's glue code, and the loader shim `native/loader.cpp` as five static libraries, keeps them in an entry of the shared folder, and links them, the C++ standard library, and the system libraries of the target.
  - Its only API is the foreign function `slang_glslang_sys_loader`, which returns the static loader. It is declared `safe fn`, so a call needs no `unsafe` block (SGL-11 of `docs/design/slang-glslang-sys.md`). The loader answers every request for a library whose file name starts with `slang-glslang` with the nine linked functions of the glue code and passes every other request to Slang's default loader.
  - Its integration-test binary `tests/glslang/` holds 23 pure tests of the build logic against a fake host, 6 `ffi` tests of the real host, and 4 `ffi` tests of the loader. Miri runs none of them (TEST-12 of the coding standard).
  - Its tests have passed through `test-vendored` in the development container on x86_64 Linux, in run 43737935692 on the Linux and macOS runners, in run 43742353739 on the Windows and macOS runners, and in run 43746178292 on all three runners.
- `crates/slang-rhi-sys`, the crate of raw bindings to slang-rhi for M9, with `links = "slang-rhi"`, publication metadata, the licence `Apache-2.0 WITH LLVM-exception`, no features, `slang-compiler-sys` with the feature `vendored` and `slang-src` as its dependencies on Linux with the GNU C library and on x86_64 Windows with the Microsoft toolchain only, and `slang-bindgen` and `find-msvc-tools` as its build dependencies and development dependencies (Section 4.2 of `docs/design/slang-rhi-sys.md`).
  - Its build script `build.rs`, with its logic in `build_logic.rs` and its real host in `system.rs`, refuses every other target first. It builds the Vulkan backend on Linux and the D3D12 backend on Windows, where it finds `cl.exe` with `find-msvc-tools` (Section 7.3 of that design). It configures the wrapper project `native/CMakeLists.txt` with CMake and Ninja in `OUT_DIR` with every download turned off or redirected to the tree, and generates the bindings and the ABI probe with `generate_slang_rhi` in every build. It then keeps the three static libraries of slang-rhi, the archive of the ABI probe, and the configured header in an entry of the shared folder, whose key also holds the hashes of the probe and the header, and links the three libraries, and the probe into the tests only (Sections 5 and 6 of `docs/design/slang-rhi-sys.md`).
  - Its root opts out of `unsafe_code`, includes the bindings from `OUT_DIR` in the module `bindings` with two lint exceptions, and re-exports them. It has no hand-written Rust API. Its crate-level documentation states the shape of the bindings, the build, the shared folder, the failures, the third-party code in a program with its licences and notices, and the shared safety contract with the rules for a program that implements `IDebugCallback` or `ITaskPool` in Rust. Its crate README points most programs to `slang-rhi`.
  - The interfaces of slang-rhi with a UUID implement `slang_compiler_sys::Interface`, every Slang type is a path into `slang_compiler_sys`, 24 methods are placeholder slots without a thunk, and 5 structs have no `Default` (RSYS-10 of `docs/design/slang-rhi-sys.md`).
  - Its integration-test binary `tests/rhi_sys/` holds 51 tests: 34 pure tests of the build logic, 9 tests of the real host in `build_script::ffi`, 3 pure tests of the defaults in `defaults`, the symbol, ABI probe, and smoke tests in `ffi`, and 2 tests in `gpu`, of which one runs a whole compute dispatch through the raw bindings on a Vulkan adapter on Linux and a D3D12 adapter on Windows. Only `test-rhi` runs them, on Linux and on Windows.
- `crates/slang-rhi`, the safe crate over `slang-rhi-sys` for the compute path of M9 and the draws of M10, with publication metadata, the licence `MIT OR Apache-2.0`, no features, `slang-rhi-sys` and `bitflags` as its dependencies, and `slang-compiler` with the feature `vendored` and `slang-compiler-sys` as its dependencies on Linux with the GNU C library and on x86_64 Windows with the Microsoft toolchain only (RHI-1 of `docs/design/slang-rhi.md`). It runs on Vulkan on Linux and on D3D12 on Windows, where a program provides `dxcompiler.dll`. Its crate README holds an example that opens a device.
  - Its public API covers `adapters` and `Adapter`, `DeviceOptions`, `Device`, `DeviceInfo`, the debug callback with `DebugMessage`, `ShaderProgram`, `ComputePipeline`, `CompileSession`, `BufferDesc`, `BufferUsage`, `MemoryType`, `Buffer`, `Queue`, `CommandEncoder`, `ComputePass`, `ShaderObject`, `ShaderOffset`, `CommandBuffer`, and the error type `Error` with its `ErrorKind`. Since the fifth code step of M10, it also covers `Texture`, `Sampler`, `RenderPipeline`, and `RenderPass` with their descriptors and enumerations (Section 6.1 of `docs/design/slang-rhi.md`). A device compiles with the global session that the caller passes, so the loader of `slang-glslang-sys` serves it. The crate's API documentation is the detailed record.
  - Since the second code step of M10, `Device::create_slang_session` creates a `CompileSession` whose session reproduces the device session (RHI-30), `ShaderObject::object` reaches the fields of constant buffers and parameter blocks (RHI-22), and `DeviceInfo` offers the limits of the device (RHI-29). Since the third, `DeviceInfo` offers seven limits, and `create_compute_pipeline` refuses a thread group over the limits of the device (RHI-7). Since the fifth, a program can create and read back textures, create samplers, set both by name, create render pipelines, and draw with and without indices in a render pass (RHI-23 to RHI-28). Since the sixth, a vertex stream may have the stride 0, which gives every vertex or instance the one attribute at the start of its buffer, and the public constant `MAX_VERTEX_STREAMS` holds the 16 streams that slang-rhi allows at most (RHI-26).
  - Since the first code step of M11, the handles `Buffer`, `Texture`, `Sampler`, `ShaderProgram`, `ComputePipeline`, `RenderPipeline`, `Queue`, and `CompileSession` are `Clone`, like `Device`, and a clone adds a reference to the same object. `Device::info` returns `&DeviceInfo`, `Buffer::usage` is public, the blend of a render pipeline is `Blend`, the bind of a compute pipeline reports `Operation::BindComputePipeline`, and `create_slang_session` takes any sequence of paths (Sections 6.4, 6.6, 6.8, and 11 of `docs/design/slang-rhi.md`).
  - Its root opts out of `unsafe_code`. Each call into slang-rhi or into Slang's reflection runs inside a per-thread guard, which panics on a reentrant call, the Slang call guard of `slang-compiler`, and the crate's own abort guard. The calls through the process-wide instance of slang-rhi hold one crate-private lock (CONC-8 of the coding standard). The crate never frees the debug callback object of a device.
  - Before each call into slang-rhi, it checks the conditions of valid use of the graphics API that it can check, such as the bounds of parameters, the usage of a buffer, the limit of a dispatch, a stream for each declared vertex input, and the shape of a texture parameter (RHI-20). No type that holds a native object is `Send` or `Sync`.
  - On Vulkan, slang-rhi clamps the depth of a primitive outside the range 0 to 1, while D3D12 clips it (RKB-9 of `docs/design/slang-rhi.md`), and the crate documentation states this difference.
  - Its unit tests, 117 of them, run against a fake of seven interfaces of slang-rhi, and `check-miri-rhi` runs them under Miri. Its integration-test binary `tests/rhi/` holds 61 tests in `gpu`, 27 of them render tests, which `test-rhi` runs with its 60 doctests, of which 33 are `compile_fail` cases.
- `crates/slang-compiler`, the safe crate without its custom file system, with publication metadata, `slang-compiler-sys` and the optional `slang-glslang-sys` as its dependencies, and `insta` as its only development dependency.
  - Its one Cargo feature, `vendored`, off by default, enables the feature `vendored` of `slang-compiler-sys` and the dependency `slang-glslang-sys`. With it, the crate installs the loader of `slang-glslang-sys` on every global session that it creates, before it returns it, through the guarded thunk of `setSharedLibraryLoader` (SLC-33, SLC-34 of `docs/design/slang-compiler.md`). A global session that `from_raw` adopts gets no loader.
  - Its test module `ffi::vendored`, compiled only with the feature, shows that a global session of the crate returns optimized SPIR-V and that one created through the raw bindings without the loader does not. On x86_64 Linux in the development container, the SPIR-V of the test shader was 1292 bytes with one function and no call with the loader, and 1452 bytes with two functions, two calls, and the diagnostic `failed to load downstream compiler 'spirv-opt'` without it. Both tests passed on the Linux and macOS runners in run 43737935692, on the Windows and macOS runners in run 43742353739, and on all three runners in run 43746178292.
  - A release program outside the workspace with the feature, built in the development container on 2026-11-15, ran without `VULKAN_SDK` and without a library path, and linked only the C++ and C runtime libraries of the system.
  - Its public API covers the global session, sessions with targets and compiler options, modules, entry points, composition, specialization, linking, code generation, blobs, and metadata, and every `layout` method. It also covers the public module `reflection`, with the layouts, reflection handles, and records of reflection, and the error type `Error` with its `ErrorKind` of eight kinds, which has no kind `FileSystem` while the custom file system is deferred. The crate's API documentation is the detailed record.
  - Its reflection also finds functions by name, lists overloads, reads the user attributes and modifiers of functions and the modifiers of variables, walks generics with their parameters, constraints, and concrete arguments, and specializes a function with argument types (Section 9.6 of `docs/design/slang-compiler.md`). `Metadata::cooperative_types` reads the cooperative types that code generation used (SLC-39), each wrapper of a program offers `session`, and `with_slang_call_guard` raises the Slang call guard for a call into Slang through another native library (Section 9.7).
  - Every wrapper holds the object that Slang created it from, and every reflection handle borrows the wrapper it came from.
  - Internally, the crate counts every reference through one owning interface pointer and maps every failure signal of Slang to an `ErrorKind` in one function. It also keeps a per-thread Slang call guard that panics on a reentrant call and defers releases until the outermost call returns.
  - Every call of the crate into Slang runs inside the crate-private function `abort_on_exception`, which aborts the process with one line on standard error that names the call when a C++ exception escapes Slang. The custom checker `check-slang-call-guard` checks where each such call sits (SLC-32).
  - A user cannot give a session a custom file system, which is deferred until a concrete need appears.
  - Some inputs crash or abort the process through known defects of Slang, such as a shader whose types nest past Slang's limit of 128 levels when an entry point of it is looked up with a stage or its layout is requested. Section 3.12 of `docs/design/slang-compiler.md` lists them (Hard Rule 14).
  - Its unit tests run under Miri against a fake of the Slang interfaces, and its integration-test binary `tests/compiler/` holds `ffi` tests that call Slang. On 2026-11-11, all of them passed in CI on Linux, Windows, and macOS, in run 42923671966.
- `crates/slango`, the high-level crate for compute kernels, which also draws with render pipelines into a texture, with publication metadata, `slang-compiler` as its dependency, `wgpu`, `pollster`, and `slang-rhi` as its optional dependencies, and three Cargo features (SLO-27 and SLO-33 of `docs/design/slango.md`):
  - `wgpu`, on by default, enables `wgpu` and `pollster` and selects the backend `wgpu`.
  - `rhi`, off by default, enables `slang-rhi` and `vendored` and selects the backend `rhi`, which runs compute kernels, also from the code that `slango-macros` generates, and creates textures and samplers and draws with render pipelines through slang-rhi on Vulkan on Linux and on D3D12 on x86_64 Windows. The public API works the same on both backends apart from `slango::wgpu`, and Section 6.5 of `docs/design/slango.md` states the differences between the backends, among them a semantic on every vertex attribute on `rhi` and the depth clamping of Vulkan.
  - On the backend `rhi`, the device feeds a vertex attribute that the shader declares and does not use from a stream of stride 0 over its own buffer of 16 zero bytes, so a draw needs the same buffers on both backends. The merge of the bindings of a render pipeline takes its key from the backend, and on D3D12 a load compiles its vertex entry points once more to SPIR-V to read which vertex inputs are used (Sections 6.1, 7.6, and 7.7 of that design).
  - With neither `wgpu` nor `rhi`, or with both, the crate stops compiling with one `compile_error!` that says so (SLO-28, SLO-29).
  - `vendored`, off by default, only forwards to `slang-compiler`.
  - Its public API covers `Device` with `DeviceOptions` and `AdapterSelection`, `Module`, `Kernel`, `Call`, `Buffer`, `Texture`, `Sampler`, the sealed traits `Element`, `VertexElement`, and `Value`, the error type `Error` with its `ErrorKind` of nine kinds, among them `Unsupported`, which no check of the crate returns since the sixth code step of M10 (Section 10 of `docs/design/slango.md`), and, in the build with `wgpu` only, the public module `wgpu`, whose accessors `device`, `queue`, and `texture` return an `Option` (SLO-31).
  - Its crate-level documentation has the section "Choosing a backend", which states how a program enables `rhi`, what that build needs and where it builds, the consequences of the exclusion of the two features, and each difference of Section 6.5 of `docs/design/slango.md`, and the section "Changes from 0.1.0-alpha.1", which states each change of Section 17.6 of that design.
  - It also has the hidden module `__private` for the code that `slango-macros` generates, which is exempt from semantic versioning (SLM-6 of `docs/design/slango-macros.md`). It holds `precompile_file`, which also reads the packed layout of each element structure, `load_embedded` with the `Embedded` of the build's backend, the helper macros `wgpu_only!` and `rhi_only!`, and `StructElement` with its `FIELDS` (Section 5.2 of that design). Its encoding has `ENCODING_VERSION` 3, which holds the row stride of each matrix (SLO-46). The crate's API documentation is the detailed record.
  - A buffer of a generated struct binds only to a structure whose fields match it in path, offset, and type, on both backends, while a buffer of a built-in element binds to a structure by its stride alone (Section 9.3 of `docs/design/slango.md`, SLMQ-5 of `docs/design/slango-macros.md`).
  - Its shared modules hold no wgpu item, and the private backend module `wgpu_backend`, which the crate root imports as `backend`, holds every object and call of wgpu outside `slango::wgpu`, and the private backend module `rhi_backend` holds every object and call of `slang-rhi`. The crate root imports the one of the build as `backend` (SLO-34 of `docs/design/slango.md`, SIMP-2 of the coding standard).
  - `Device`, `Module`, `Kernel`, `Buffer<T>`, `RenderPipeline`, `Texture`, `Sampler`, `Call`, and `Draw` are neither `Send` nor `Sync`, each through a marker field with two `compile_fail` doctests, while the errors, the options types, and the enumerations are both (SLO-17).
  - A call and a draw refuse one buffer bound to a read-only and to a read-write parameter with `ErrorKind::InvalidArgument` (SLO-44), and marshalling writes each row of a matrix at the row stride that reflection reports (SLO-46).
  - Its pure unit tests run natively, and the `gpu` tests of its integration-test binary `tests/slango/` passed in CI on lavapipe, WARP, and hosted Metal, most recently on the backend `wgpu` in run 44398417125, apart from macOS, whose job GitHub cancelled. `test-rhi` runs them on the backend `rhi`, where one test skips on Vulkan and three skip on D3D12, each for a limit of slang-rhi (Section 13.7 of `docs/design/slango.md`), and they passed there on lavapipe and on WARP in run 44474920321.
- `crates/slang-slint`, the crate that shares Slint's wgpu device with `slango` and shows what `slango` draws in a Slint user interface, with publication metadata, `slango` and `slint` as its dependencies, and the feature `vendored`, which only forwards to `slango` (SLS-17 of `docs/design/slang-slint.md`).
  - Its public API covers these items:
    - `wgpu_configuration` returns the configuration with which Slint creates a device that suits `slango`.
    - `device` turns the wgpu 30 device and queue that Slint passes to a rendering notifier into a `slango::Device`.
    - `View` draws with a `slango` render pipeline into a texture that Slint shows as an image.
    - The error type `Error` with its `ErrorKind`.
  - It has no unsafe code. `View` is neither `Send` nor `Sync`, which two `compile_fail` doctests assert (SLS-14). The crate's API documentation is the detailed record.
  - Its pure tests run natively, and the `gpu` tests of its integration-test binary `tests/slang_slint/` passed locally on 2026-11-11 (Section 9 of `docs/design/slang-slint.md`).
- `crates/slango-codegen`, the library crate that holds the generator of `slango-macros`, with publication metadata, `slango` with an exact version, `proc-macro2`, and `quote` as its dependencies, `insta`, `prettyplease`, and `syn` as its development dependencies, and the feature `vendored`, which only forwards to `slango` (SLM-23 of `docs/design/slango-macros.md`).
  - Its public API covers `generate`, which resolves a relative path against the working directory, and the error type `Error` with its `ErrorKind` of three kinds and its method `report`. The function `generate_precompiled` is also `pub`, for the crate's integration test, but `#[doc(hidden)]` and exempt from semantic versioning, like the hidden type of `slango` that it takes (Section 4.2 of `docs/design/slango-macros.md`). It has no unsafe code. Its crate-level documentation says when to use it instead of the macro and why it requires `slango` with an exact version. The crate's API documentation is the detailed record.
  - It always enables `wgpu` on its own `slango`, so a program whose `slango` has `rhi` can use it only as a build dependency or through `slango-macros` (Section 4.3 of `docs/design/slango-macros.md`).
  - Its pure unit tests run natively. The `ffi` tests of its integration-test binary `tests/codegen/` compare the generated code with `insta` snapshots.
- `crates/slango-macros`, the procedural macro crate, with publication metadata, `slango-codegen` with an exact version and `syn` as its dependencies, `slango` and `trybuild` as its development dependencies, and the feature `vendored`, which only forwards to `slango-codegen` (SLM-23).
  - With `vendored` on `slango-macros`, the macro's copy of Slang is built from source, and a program without a Vulkan SDK enables `vendored` on `slango` as well, because Cargo resolves the features of the macro's host dependencies apart (Section 4.3 of `docs/design/slango-macros.md`). On 2026-11-15 in the development container, a release program outside the workspace with `vendored` on both crates built from an empty target directory without a Vulkan SDK in 6 min 18 s, built Slang and slang-glslang once each for both copies, and ran the generated saxpy kernel. With the feature on only one of the two crates, its build stopped with the error that `VULKAN_SDK` is not set. That was on x86_64 Linux only.
  - Its one macro, `shader!`, compiles a Slang file with the Slang of its own copy of `slang-compiler`, by default that of the Vulkan SDK at `VULKAN_SDK`, while the crate builds and generates a typed kernel for each compute entry point and a Rust struct for each Slang structure that the kernels take (Sections 6 and 7 of `docs/design/slango-macros.md`).
  - The generated code follows the backend of the program's `slango`, so the crate has no backend feature. On the backend `wgpu`, it embeds the encoded snapshot and std430 structs. On the backend `rhi`, it embeds the Slang source of the shader and of the modules that it imports with their absolute paths, compiles them when the module loads, and has packed structs (SLM-24 to SLM-26). On that backend, a file that the shader includes with `#include` or `__include` is read from its path on the build machine when the module loads.
  - On 2026-11-19 in the development container, a program outside the workspace with `slango` on the backend `rhi` and `slango-macros` with `vendored` built without a Vulkan SDK and ran a typed kernel on the NVIDIA adapter and on lavapipe, on x86_64 Linux.
  - Its build script gives the macro library an rpath to `$VULKAN_SDK/lib` on Linux and macOS (Section 6.3 of that design). A documentation build expands the macro with Slang like any other build, and the macro has no stub.
  - Its crate-level documentation states what building a crate that calls the macro needs, how failures appear, the exact versions that tie it to `slango`, the lint exceptions that a calling module may need, and the known limits.
  - Its integration-test binary `tests/macros/` holds pure tests, `ffi` tests, and `gpu` tests that passed locally on 2026-11-11 (Section 10 of that design). Its module `bindings` asserts SLM-21 at compile time for the generated `Shader`, kernel struct, and arguments struct, on the default route and on the vendored route.
  - In CI run 43300696682 on `main`, for commit `376f10a`, the `gpu` test `gpu::typed_kernel_advances_the_particles_and_prints_the_checked_results` of the examples passed on lavapipe on Linux, WARP on Windows, and Metal on macOS.
- `examples/`, the unpublished package `slang-rust-examples`, with these examples and their tests:
  - `compile-and-reflect` compiles a Slang file and prints its entry points and parameters.
  - `compute-kernel` runs a compute kernel with `slango` and checks its results.
  - `windowed-renderer` draws a textured quad in a window and reloads its shader.
  - `shader-playground` shows a shader in a Slint window with two sliders and reloads it.
  - `typed-kernel` runs a compute kernel through the typed bindings that `slango_macros::shader!` generates and checks its results.

  The package has the features `wgpu`, its default, `rhi`, and `vendored`. With `rhi`, only `compile-and-reflect`, `compute-kernel`, and `typed-kernel` build, and `test-rhi` runs them, the `gpu` tests of the generated code of `slango-macros`, and the scenes of `windowed-renderer` and `shader-playground` drawn offscreen on the backend `rhi`, where the scene of `shader-playground` draws into a texture of its own (Section 13.7 of `docs/design/slango.md`). The shader of `shader-playground` takes the corners of its triangle from a vertex buffer.

  The tests are described in Section 13.6 of `docs/design/slang-compiler.md`, Sections 13.3, 13.5, and 13.7 of `docs/design/slango.md`, Section 10 of `docs/design/slang-slint.md`, and Section 10 of `docs/design/slango-macros.md`. No recipe runs an example.
- `crates/slang-bindgen`, the binding generator, a library crate with publication metadata, `clang` and `clang-sys` as its dependencies, no features, and a thin binary. It parses the Slang headers, and for `slang-rhi-sys` the slang-rhi headers, with libclang for the target that its caller names and returns the raw bindings and the list of their C functions as unformatted text, with its two rule sets `SLANG` and `SLANG_RHI` and its shim headers, which include the C++ headers that the slang-rhi headers reach.
  - The first call of `generate` keeps libclang loaded until the process exits, through a static handle of `clang-sys`, without unsafe code. A later call works on any thread once the earlier one has returned, and a call during another call fails with `ErrorKind::Libclang` (SCS-67 of `docs/design/slang-compiler-sys.md`).
  - Its public API covers `generate`, the second entry point `generate_slang_rhi`, which also returns the ABI probe of slang-rhi, the struct `Generated` with its field `abi_probe`, and the error type `Error` with its `ErrorKind` of nine kinds, `Location`, and `Result`. The crate's API documentation is the detailed record.
  - For slang-rhi, it writes every Slang type as a path into `slang_compiler_sys`, names overloads by rule, in declaration order on the Itanium C++ ABI and with each run of adjacent overloads reversed on the Microsoft ABI, keeps placeholder slots, leaves out the `Default` of the structs that its rules name, reads pointer constants, declares the callbacks that Rust calls `"C-unwind"`, and writes the ABI probe (Section 7.8 of `docs/design/slang-compiler-sys.md`). It has 151 tests, of which 36 cover the slang-rhi headers on synthetic headers.
  - Its binary `slang-bindgen` takes a header directory, a target triple, and an output directory, and writes `bindings.rs` and `symbols.rs` there. Only the recipe `check-slang-call-guard` runs it.
  - Its unit tests in `src/tests.rs` parse synthetic headers and compile for Linux only. Its integration-test binary `tests/generator/` runs the binary on the headers at `VULKAN_SDK` and on failing inputs, and compiles for Linux only as well (Section 7.5 of `docs/design/slang-compiler-sys.md`).
  - The binary's output for the headers of the Vulkan SDK in the development container was the same text for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, `x86_64-apple-darwin`, and `aarch64-unknown-linux-gnu` on 2026-11-15. In run 43717303370, the build script generated the bindings on Windows and macOS as well, and the tests of `slang-bindgen`, which compile for Linux only, ran on Linux only.
- The four licence texts at the root, `LICENSE-APACHE`, `LICENSE-MIT`, `LICENSE-APACHE-LLVM`, and `LICENSE-GPL`, a symbolic link to the text of its own licence in each package directory, the root `README.md`, which is the project page on GitHub, and the `README.md` of each library crate, which crates.io shows as the page of the crate once it is published.
- The writing guide, the guide to READMEs `docs/writing-readmes.md`, the guide against the prose habits of generated text `docs/avoiding-claudelish.md`, the coding standard, the reference list, and the list of upstream reports under `docs/`.
- The architecture design, `docs/design/architecture.md`, which specifies the planned crates, how Slang is obtained and which Slang is recorded, and the roadmap of milestones M0 to M18.
- The tooling design, `docs/design/tooling.md`, which specifies the toolchain configuration, the recipes, the hooks, the custom checkers, the CI workflow, and the requirements on the container image, which milestone M0 implemented, and the release procedure, the check for breaking changes, and the hosted documentation, which M8 designed and implemented, the recipes, scripts, and documentation workflow in its second code step and `check-semver` with the CI job `semver` in its sixth.
- The design of `slang-compiler-sys` and the binding generator `slang-bindgen`, `docs/design/slang-compiler-sys.md`, a draft for M7, whose dynamic route, generator, and build information the second code step of M7 implemented, whose vendored route the third implemented, and whose system libraries the fourth added.
- The design of `slang-src`, `docs/design/slang-src.md`, a draft for M7 that the first code step implemented.
- The design of `slang-glslang-sys`, `docs/design/slang-glslang-sys.md`, a draft for M7 that the fourth code step implemented.
- The design of `slang-compiler` and the compile-and-reflect example, `docs/design/slang-compiler.md`, a draft that milestone M2 implemented except for the custom file system.
- The design of `slango` and the compute kernel example, `docs/design/slango.md`, a draft that milestone M3 implemented, and that also holds the design of M4, rendering, which M4 implemented, and the design of M10, whose boundary the first code step of M10 implemented, whose compute on the backend `rhi` the third implemented, whose rendering on that backend the sixth implemented, and whose crate documentation and migration notes the seventh completed.
- The design of `slang-slint` and the Slint example, `docs/design/slang-slint.md`, a draft that milestone M5 implemented.
- The design of `slango-macros` and its generator `slango-codegen`, `docs/design/slango-macros.md`, a draft that milestone M6 implemented, and whose design of M10, the macro on the backend `rhi`, the fourth code step of M10 implemented.
- The designs of `slang-rhi-sys` and `slang-rhi`, `docs/design/slang-rhi-sys.md` and `docs/design/slang-rhi.md`, drafts for M9, whose build of slang-rhi the first code step implemented, whose bindings the third implemented, whose safe crate the fifth implemented, whose build for Windows with D3D12 of Section 7.3 of `docs/design/slang-rhi-sys.md` the sixth implemented, and whose crate documentation and crate READMEs the seventh completed. The second code step of M10 implemented the compute phase of Section 11.1 of `docs/design/slang-rhi.md`, and the fifth its rendering phase of Section 11.2. Section 7.4 of that design states why macOS has no slang-rhi.

`just check` passes in the development container, and it passed in CI in run 43749637842. The tests of `slang-bindgen` and of `slang-compiler-sys` run in the default tier.

The tests of the `gpu` tier are those of `slango`, of `slang-slint`, of `slango-macros`, of the examples `compute-kernel` and `typed-kernel`, and of the scenes of the examples `windowed-renderer` and `shader-playground`. On 2026-11-21, after the first code step of M11, the test tiers have 1110 tests on Linux. `check-miri` runs 203 tests of `slang-compiler` under Miri and skips 95, and on 2026-11-19 `just check` took 4 min 47 s in the development container. The tests of `slang-bindgen` grew to 151 in the sixth code step of M9. `test-vendored` runs 453 tests, the 45 of `slang-src`, the 33 of `slang-glslang-sys`, the 75 of `slang-compiler-sys`, and the 300 of `slang-compiler`, then 4 tests of `slango-macros`, with 12 skipped by the filter, and 243 doctests, the 238 of `slang-compiler`, the 4 of `slang-compiler-sys`, and the 1 of `slang-glslang-sys`. Only `test-rhi` runs the tests of `slang-rhi-sys` and `slang-rhi`, with the profile `gpu`: 229 tests, the 51 of `slang-rhi-sys` and the 117 unit tests and 61 `gpu` tests of `slang-rhi`, and then the 60 doctests of `slang-rhi`. It then runs the tests of `slango` on the backend `rhi`, 379 that run and 1 that skips on Linux, and 40 doctests of that build, and then the 47 tests of the examples package on that backend, with no skip. `check-miri-rhi` runs the 117 unit tests of `slang-rhi` under Miri. No other crate's tests run under Miri (TEST-12 of the coding standard).

Building the workspace needs a LunarG Vulkan SDK with `VULKAN_SDK` set to its directory, and libclang, on every platform, which the development container and every CI job except `vendored` provide. The crates are tested with the Slang of SDK 1.4.363.1, and the build accepts the Slang of any SDK. Every command that compiles the workspace, such as `cargo check`, `cargo clippy`, `cargo test`, and every recipe that compiles, runs the build script of `slang-compiler-sys`, which generates the bindings with libclang. None of them enables the feature `vendored` or builds `slang-src`, `slang-glslang-sys`, `slang-rhi-sys`, or `slang-rhi`, which `default-members` and the `--exclude` options of the recipes leave out, so none clones Slang or builds Slang, slang-glslang, or slang-rhi from source.

A vendored build needs no Vulkan SDK. It needs libclang, git and network access for the first build in a target directory, CMake 3.22 or later, Ninja, a C++20 compiler, which on Windows is an installation of Visual Studio with its C++ tools, and, for slang-glslang, Python 3 on `PATH` as `python3` or `python`. A build that includes slang-rhi, which builds on Linux and on x86_64 Windows, also needs on Linux the development headers of Xlib, such as those of the Debian package `libx11-dev` (Section 7.1 of `docs/design/slang-rhi-sys.md`), and on Windows the Windows SDK of the Microsoft toolchain, whose `dxcompiler.dll` a program that creates a pipeline needs on its `PATH` at run time (Section 7.3 of that design). The development container provides all that Linux needs, the Xlib headers only until it is rebuilt, as Technology Decisions states.

Without `VULKAN_SDK`, the build script stops the build with an error that starts "VULKAN_SDK is not set, so slang-compiler-sys cannot find the Slang library to link", says what to install and set, and ends with the advice that enabling the Cargo feature `vendored` builds Slang from source instead. Without libclang, it stops the build with an error that starts "cannot generate the Slang bindings from", names the header directory and the target, holds "cannot load libclang:" with the reason, and ends with "slang-compiler-sys generates its bindings with libclang in every build, so install libclang or set LIBCLANG_PATH to the directory that holds it."

On the vendored route, a missing program stops the build with an error that starts "cannot start" and names git, CMake 3.22 or later, Ninja, and a C++ compiler. On Windows, a missing compiler stops it with an error that starts "cannot find the Microsoft C++ compiler cl.exe" and says where the build script looked. A failed configure or build step names the log `slang/build.log` under `OUT_DIR` and holds its last 40 lines. Section 9.6 of `docs/design/slang-compiler-sys.md` lists every message. The build script of `slang-glslang-sys` names CMake 3.22 or later, Ninja, Python 3 on `PATH`, and a C++20 compiler when a program cannot start, and its log is `build.log` under its own `OUT_DIR` (Section 12.4 of `docs/design/slang-glslang-sys.md`).

On Linux, building the workspace also needs the development files of fontconfig, which Slint links and the development container provides (Section 3.2 of `docs/design/slang-slint.md`).

A test binary that starts without `$VULKAN_SDK/lib` on the library path stops before any test runs, on Linux with "error while loading shared libraries: libslang-compiler.so.0.2026.17: cannot open shared object file". The agent therefore runs the tests through their recipes, which supply that path. Section 18.4 of `docs/design/slang-compiler-sys.md` lists every failure and its recovery.

What does not exist yet:

- The custom file system of `slang-compiler`, which is deferred with no milestone assigned (Section 15.22 of `docs/design/architecture.md`). It is the trait `FileSystem` and the setter `SessionBuilder::file_system` of Section 8 of `docs/design/slang-compiler.md`, with the error kind `FileSystem`, the Rust-owned blob, the panic handling of SLC-18, and the Slang call guard raised around the user's code of every vtable function (SLC-28). A user who needs one reaches Slang's file system interface through `slang-compiler-sys`.
- The path-tracing demo of Section 10 of `docs/design/architecture.md`, the broadcasting case and the differentiable function of `compute-kernel`, and tests of the `hw_gpu` tier.
- The embedding of the files that a shader includes with `#include` or `__include` on the backend `rhi`, which the design records as a limit (Section 14.6 of `docs/design/slango-macros.md`).
- On the backend `rhi`, a resource inside a structure that is not the element of a constant buffer or a parameter block, and a constant buffer whose element is not a structure, which wait for SOQ-11 of `docs/design/slango.md` and RHQ-11 of `docs/design/slang-rhi.md`.
- slang-rhi on ARM64 Windows, which `slang-rhi-sys` refuses (RSYS-13 of `docs/design/slang-rhi-sys.md`).
- slang-rhi on macOS, which stays unsupported until Slang's tree holds a newer metal-cpp (Section 7.4 of `docs/design/slang-rhi-sys.md`).
- The design document of each crate other than `slang-compiler-sys`, `slang-compiler`, `slango`, `slang-slint`, `slango-macros`, `slango-codegen`, `slang-src`, `slang-glslang-sys`, `slang-rhi-sys`, and `slang-rhi`, which is written as the first task of the milestone that delivers the crate.
- A recipe that builds the examples package with its feature `vendored`, and a `gpu` test on the vendored route.

The workspace lint tables, `clippy.toml`, the tools that `just check` runs, and the custom checkers enforce their rules, so an agent learns those rules from a failing check, as Hard Rule 3 states.

The custom checkers enforce these rules:

- The rules of the writing guide on comments in Rust source, on the headings of item documentation, on `ignore` in doctests, and on the `# Features` section.
- The rules of the writing guide on ASCII punctuation, semicolons, and listed spellings in API documentation.
- The phrase check of Section 6 of `docs/avoiding-claudelish.md`, in Markdown files and in API documentation.
- The part of rule SLC-32 of `docs/design/slang-compiler.md` that places every call of `slang-compiler` into Slang alone inside the closure of `abort_on_exception`.
- The rules of Section 7.2 of `docs/design/tooling.md` on pinned actions, the `pull_request_target` event, and top-level workflow permissions.
- The part of Hard Rule 15 that forbids citations of project documents, sections, and rule identifiers in code and configuration.
- The part of rule TEST-12 of the coding standard that requires a Miri recipe for every workspace member that opts out of `unsafe_code`, apart from the members that the checker exempts by name, and no Miri recipe for any other.

Section 7.12 of the writing guide and Section 4.8 of the coding standard say which part of each rule a checker enforces. Every rule that no configured tool checks stays a judgement rule.

`check-features`, `check-msrv`, `check-semver`, `check-vulkan-sdk`, and `coverage` run outside `just check`, and CI runs them in every run of the workflow. The agent MUST NOT assume that a planned crate, recipe, file, or tool exists, because an agent that builds on something absent produces code that cannot work.

## Repository Layout

- `AGENTS.md`: This file.
- `CLAUDE.md`: A one-line file that imports `AGENTS.md` for Claude Code.
- `Cargo.toml`: The workspace manifest. It holds these parts:
  - Resolver 3.
  - The `[workspace.package]` fields that the members inherit, such as edition 2024 and `rust-version`, and the licence `MIT OR Apache-2.0`, which every member except `slang-compiler-sys`, `slang-glslang-sys`, `slang-rhi-sys`, `slang-slint`, and `slang-src` inherits (TOOL-4 of `docs/design/tooling.md`).
  - The version `0.2.0-alpha.1` in `[workspace.package]`, which every member except `slang-src` inherits, the version of the second release, whose minor number rises because M10 breaks the API of `slango` (TOOL-37 and Section 9.10.3 of `docs/design/tooling.md`).
  - The `authors` field in `[workspace.package]`, which every member inherits and Cargo copies into every packaged manifest, and which the owner keeps as it is (Section 9.6 of `docs/design/tooling.md`).
  - In `[workspace.dependencies]`, the dependencies `bitflags`, `clang`, `clang-sys`, `find-msvc-tools`, `insta`, `pollster`, `prettyplease`, `proc-macro2`, `quote`, `slang-bindgen`, `slang-compiler`, `slang-compiler-sys`, `slang-glslang-sys`, `slang-rhi`, `slang-rhi-sys`, `slang-slint`, `slang-src`, `slango`, `slango-codegen`, `slango-macros`, `slint`, `syn`, `trybuild`, `wgpu`, and `winit`, with `slang-bindgen`, `slang-compiler`, `slang-compiler-sys`, `slang-glslang-sys`, `slang-rhi`, `slang-rhi-sys`, `slang-slint`, `slang-src`, `slango`, `slango-codegen`, and `slango-macros` by path and exact version.
  - The workspace lint tables.

  Its members are `crates/slang-bindgen`, `crates/slang-compiler`, `crates/slang-compiler-sys`, `crates/slang-glslang-sys`, `crates/slang-rhi`, `crates/slang-rhi-sys`, `crates/slang-slint`, `crates/slang-src`, `crates/slango`, `crates/slango-codegen`, `crates/slango-macros`, and `examples`. Its `default-members` are every member except `crates/slang-src`, `crates/slang-glslang-sys`, `crates/slang-rhi-sys`, and `crates/slang-rhi`, so a plain `cargo build` in the root of the workspace does not clone Slang or build slang-glslang or slang-rhi (TOOL-32 of `docs/design/tooling.md`).
- `Cargo.lock`: The lock file of the workspace.
- `LICENSE-APACHE`, `LICENSE-MIT`, `LICENSE-APACHE-LLVM`, `LICENSE-GPL`: The texts of the Apache License 2.0, the MIT licence, the Apache License 2.0 with the LLVM exception, and version 3 of the GNU General Public License, each held once (TOOL-33 of `docs/design/tooling.md`).
- `LICENSE`, `LICENSE-APACHE`, and `LICENSE-MIT` in a package directory: Relative symbolic links to the root texts of the package's own licence, under the conventional name, which is `LICENSE` in `crates/slang-compiler-sys/`, `crates/slang-glslang-sys/`, `crates/slang-rhi-sys/`, `crates/slang-slint/`, and `crates/slang-src/`, and `LICENSE-APACHE` with `LICENSE-MIT` in every other package directory, including `crates/slang-bindgen/` and `examples/`. `cargo package` follows the links, but a checkout without symbolic link support holds only the target path, so a release is packaged on Linux or macOS (TOOL-33 of `docs/design/tooling.md`).
- `README.md`: The root README, which is the project page on GitHub. No member inherits it (TOOL-4 of `docs/design/tooling.md`).
- `README.md` in the package directory of each library crate: The crate README, which the crate's `readme` field names and crates.io shows as the page of the crate. The examples package has none (Section 6 of `docs/writing-readmes.md`).
- `crates/slang-bindgen/`: The binding generator that Project Status describes. It has these parts:
  - `Cargo.toml`, which inherits `clang` and `clang-sys` from `[workspace.dependencies]`.
  - The library under `src/` with the private modules `parse`, `rules`, `check`, `emit`, `probe`, `model`, and `error`. The module `emit` writes the unwinding ABIs of SCS-10 and SCS-11 of `docs/design/slang-compiler-sys.md`, and `probe` writes the ABI probe of slang-rhi (SCS-78).
  - `src/rules.rs`, the two rule sets: `SLANG`, which decides every declaration and macro of the Slang headers (Section 4 of `docs/design/slang-compiler-sys.md`), and `SLANG_RHI`, which decides those of the slang-rhi headers (RSYS-10 of `docs/design/slang-rhi-sys.md`).
  - The unit tests in `src/tests.rs`, which compile for Linux only, its module `src/tests/ffi/slang_rhi.rs` with the tests of the slang-rhi headers on synthetic headers, and the unit tests in the modules.
  - The binary `src/main.rs`.
  - The integration-test binary `tests/generator/`, with `main.rs` and the module `ffi.rs`, which compiles for Linux only and runs the binary (Section 7.5 of `docs/design/slang-compiler-sys.md`).
  - `shim/`, the shim headers that replace the system headers when the generator parses for a target: `stdint.h`, `stddef.h`, `inttypes.h`, `features.h`, and `TargetConditionals.h`, and, for the slang-rhi headers, `algorithm`, `atomic`, `iterator`, `cstddef`, `cstdint`, and `assert.h` (SCS-77).
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
- `crates/slang-src/`: The crate that Project Status describes, with `Cargo.toml`, which declares `links = "slang-src"`, the version `0.2.0-alpha.1.slang.2026.17.0.25.gc7954ebd1`, which encodes the release and the recorded build tag (SRC-13 of `docs/design/slang-src.md`), its own licence, no dependencies, and no features. It also has `src/lib.rs`, which holds only the crate-level documentation.
- `crates/slang-src/recorded.rs`: The constants `RECORDED_COMMIT`, `RECORDED_BUILD_TAG`, and `RECORDED_CLONE_ENTRY`, which the build script and the test include and the recipe `recorded-slang` prints (SRC-11 of `docs/design/slang-src.md`).
- `crates/slang-src/build.rs`: The build script, which includes `recorded.rs`, `source.rs`, and `system.rs`, runs the logic on the real host, prints the Cargo instructions, and returns the result.
- `crates/slang-src/source.rs`: The logic of the build script, as plain functions behind the trait `Host` for everything it reads, runs, or writes (SYS-8 of the coding standard). It holds the required submodules, the name of the clone entry, the git settings, the repository variables, the lookup of the shared folder, the entry protocol, and every failure message.
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
- `crates/slang-rhi-sys/`: The crate that Project Status describes, with `Cargo.toml`, which declares `links = "slang-rhi"`, the `documentation` field, its own licence, no features, `slang-compiler-sys` with the feature `vendored` and `slang-src` as dependencies on Linux with the GNU C library and on x86_64 Windows with the Microsoft toolchain only, and `slang-bindgen` and `find-msvc-tools` as build dependencies and development dependencies, and the symbolic link `LICENSE`. It also has `src/lib.rs`, which holds the crate-level documentation, the opt-out of `unsafe_code`, on those targets `use slang_compiler_sys as _;` and `use slang_src as _;`, the module `bindings` with its two lint exceptions, which includes `bindings.rs` from `OUT_DIR`, and the re-export of its items.
- `crates/slang-rhi-sys/native/CMakeLists.txt`: The wrapper project, which turns on the D3D12 backend on Windows and otherwise the Vulkan backend with a check that the compiler finds `X11/Xlib.h`, adds the submodule `external/slang-rhi` of the source tree with the options of Section 5.2 of `docs/design/slang-rhi-sys.md`, and defines the static library `slang-rhi-sys-abi-probe` from the ABI probe that the build script writes to `abi-probe.cpp` in the build directory.
- `crates/slang-rhi-sys/build.rs`: The build script, which includes `build_logic.rs` and `system.rs`, runs the logic on the real host, prints the Cargo instructions, and returns the result.
- `crates/slang-rhi-sys/build_logic.rs`: The logic of the build script, as plain functions behind the trait `Host` (SYS-8 of the coding standard). It holds the platform check with the archives, libraries, and configure options of each platform, the lookup of `cl.exe` on Windows, the empty directory for WinPixEventRuntime, the order of the steps with the generation before the lock, the entry protocol of the shared folder with the entry key, the link instructions, and every failure message.
- `crates/slang-rhi-sys/system.rs`: The struct `System`, which implements `Host` on the real environment, file system, and processes, on the generator with `slang_bindgen::generate_slang_rhi`, and on the lookup of the Microsoft compiler with `find-msvc-tools`, and which the build script and the test module `tests/rhi_sys/build_script/ffi.rs` both include.
- `crates/slang-rhi-sys/tests/rhi_sys/`: The integration-test binary of the crate, with `main.rs`, which carries the `unsafe_code` exceptions on the modules `ffi`, `gpu`, and `symbols`, and these modules:
  - `build_script.rs` includes `build_logic.rs` and tests the logic against a fake host.
  - `build_script/ffi.rs` includes `system.rs` and tests the real host, so Miri skips it.
  - `defaults.rs` tests `kInvalidCUDAStream` and the `Default` of `SubmitDesc` and `HeapAllocDesc`.
  - `ffi.rs` holds the native-symbol test, the comparison of the slots of the ABI probe with `VTABLE_SLOTS`, and the smoke test.
  - `gpu.rs` holds the two tests on a Vulkan adapter on Linux and a D3D12 adapter on Windows, with a debug callback implemented in Rust.
  - `symbols`, declared in `main.rs`, includes the generated list of C functions, the declaration of the slot function of the ABI probe, and `VTABLE_SLOTS` from `OUT_DIR`.
- `crates/slang-rhi/`: The safe crate that Project Status describes, with `Cargo.toml`, which sets the `documentation` field, declares no features, inherits `bitflags` and `slang-rhi-sys` from `[workspace.dependencies]`, and inherits `slang-compiler` with the feature `vendored` and `slang-compiler-sys` on Linux with the GNU C library and on x86_64 Windows with the Microsoft toolchain only (RHI-1 of `docs/design/slang-rhi.md`), and the symbolic links `LICENSE-APACHE` and `LICENSE-MIT`. Its `src/lib.rs` holds the crate-level documentation with the example of the compute path and the path of a draw, the opt-out of `unsafe_code`, the declarations of the private modules, the re-exports of the public items at the crate root, and the item `ReadmeDoctests` that compiles the crate README as doctests. Its private modules are these:
  - `owned` holds the owning interface pointer, the only code that adds or releases a reference (RHI-3).
  - `guard` holds the per-thread guard, the collector of the messages of Slang, each in a `thread_local!`, and the abort guard (Section 8 of `docs/design/slang-rhi.md`).
  - `instance` holds the one lock around the calls through the process-wide instance of slang-rhi (RHI-14).
  - `callback` holds the debug callback object, which the crate never frees, `DebugMessage`, and its enumerations (RHI-6).
  - `lookup` holds the lookup of shader offsets through Slang's reflection (RHI-10).
  - `device`, `buffer`, `program`, `command`, and `shader_object` hold the wrappers, each with the unsafe calls of its own wrapper, and the checks before each call (RHI-2).
  - `compile_session` holds `CompileSession`, `Device::create_slang_session`, and the choice of the target, profile, and macro of the session of each device type (RHI-30).
  - `texture` holds `Format`, `TextureUsage`, `TextureDesc`, `Texture`, `create_texture`, and `read_texture` with their checks (RHI-23, RHI-28).
  - `sampler` holds `FilterMode`, `AddressMode`, `SamplerDesc`, `Sampler`, and `create_sampler` (RHI-24).
  - `render_pipeline` holds `VertexFormat`, `StepMode`, `VertexStream`, `Blend`, `CullMode`, `RenderPipelineDesc`, `RenderPipeline`, and `create_render_pipeline`, with the checks of the vertex streams and inputs and the plan of the input layout of each backend (RHI-26).
  - `render_pass` holds `ColourLoad`, `DepthLoad`, `RenderPassDesc`, `RenderPass`, and `CommandEncoder::render_pass`, with the checks of the targets and of each draw (RHI-27).
  - `error` holds `Error`, `ErrorKind`, `Operation`, and the one mapping of result codes (RHI-12).
  - `fake`, compiled only for the unit tests, holds the fake of `IBuffer`, `IComputePassEncoder`, `IRenderPassEncoder`, `IRenderPipeline`, `IShaderObject` with its `getObject`, `ICommandBuffer`, and `ICommandQueue`, which Miri runs the unit tests against (RHI-16).
- `crates/slang-rhi/tests/rhi/`: The integration-test binary of the crate, with `main.rs`, the helpers in `common.rs`, and the module `gpu`, which holds the tests on a Vulkan adapter on Linux and a D3D12 adapter on Windows (Section 10.3 of `docs/design/slang-rhi.md`). Its file `gpu.rs` declares the submodules `gpu/objects.rs`, with the tests of `ShaderObject::object`, `gpu/compile_session.rs`, with the tests of `CompileSession`, and `gpu/render.rs`, with the tests of textures, samplers, render pipelines, and draws.
- `crates/slang-compiler/`: The safe crate that Project Status describes, with `Cargo.toml`, which declares the feature `vendored` and inherits `slang-compiler-sys`, the optional `slang-glslang-sys`, and the development dependency `insta` from `[workspace.dependencies]` (Section 4.1 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/lib.rs`: The crate-level documentation of the current scope, the opt-out of `unsafe_code`, the declarations of the private modules, the re-exports of the public items at the crate root, and the item `ReadmeDoctests` that compiles the crate README as doctests.
- `crates/slang-compiler/src/owned.rs`: The owning interface pointer `Owned`, the only code that adds or releases a reference, with the checked cast through `queryInterface`, and the crate-private unsafe trait through which it calls the generated thunks of `addRef`, `release`, and `queryInterface` (SLC-2, Section 5.4 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/guard.rs`: The Slang call guard and its queue of deferred releases, each in a `thread_local!` (SLC-28), the public `with_slang_call_guard`, which raises that guard around a closure (SLC-41), and `abort_on_exception`, which aborts the process when a C++ exception escapes the one call into Slang that it wraps (SLC-32).
- `crates/slang-compiler/src/error.rs`: The public types `Error`, `Result`, `ErrorKind`, `Operation`, `Diagnostics`, and `Compiled` (Sections 6.8 and 6.9 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/module.rs`: The module loaders and `loaded_modules` of `Session`, `Module`, `EntryPoint`, and the iterators `LoadedModules`, `EntryPoints`, and `DependencyFilePaths` (Section 6.5).
- `crates/slang-compiler/src/component.rs`: `Component`, `SpecializationArg`, `Session::compose`, `Session::type_conformance`, `ComponentType`, `TypeConformance`, and `LinkedProgram`, with linking, code generation, and the tracked entry point count (Sections 6.6 and 6.7).
- `crates/slang-compiler/src/blob.rs`: `Blob`, `Metadata`, `BindingSpace`, `BindingIndex`, and the crate-private reading of diagnostics blobs (Section 6.7).
- `crates/slang-compiler/src/cooperative.rs`: `CooperativeTypes`, which `Metadata::cooperative_types` returns, the records `CooperativeMatrixType`, `CooperativeMatrixCombination`, `CooperativeVectorTypeUsageInfo`, and `CooperativeVectorCombination`, and the enumerations `Scope` and `CooperativeMatrixUse` (SLC-39).
- `crates/slang-compiler/src/reflection.rs`: The public module `reflection`, which re-exports the items of its private modules and holds their shared helpers (Section 9 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/reflection/`: The private modules `binding`, `generic`, `kinds`, `program`, `type_layout`, `types`, and `variable`, which hold the reflection handles, records, iterators, and value types. `generic` holds `Generic` and its iterators (SLC-37).
- `crates/slang-compiler/src/mapping.rs`: The one function that maps every failure signal of Slang to an `ErrorKind` (SLC-11).
- `crates/slang-compiler/src/enums.rs`: The enumerations `TargetFormat`, `Toggle`, `LanguageVersion`, `MatrixLayout`, `Stage`, `OptimizationLevel`, `DebugInfoLevel`, `FloatingPointMode`, `LineDirectiveMode`, and `ParameterCategory`, with their crate-private conversions to the raw values (SLC-14).
- `crates/slang-compiler/src/options.rs`: `CompilerOptions` with its typed and raw setters, `OptionName`, the list of the options that a target rejects, and the crate-private types that own the C strings of the option entries during a call to Slang (Section 7 of `docs/design/slang-compiler.md`).
- `crates/slang-compiler/src/session.rs`: `Session`, `SessionBuilder`, `Target`, and `TargetIndex`, with the checks of `build`, the building of the session descriptor, and the filling of the fields of each target descriptor (SLC-6, SLC-31).
- `crates/slang-compiler/src/strings.rs`: The one conversion of text that the crate passes to Slang into a C string, which reports a NUL byte as an error (SLC-15). It also holds the shared reading of a string that Slang returns and the one constructor of the error of kind `InvalidUtf8`.
- `crates/slang-compiler/src/global_session.rs`: `GlobalSession` and `GlobalSessionBuilder`, with the check of the build tag of the loaded Slang against `slang_compiler_sys::BUILD_INFO`, the installation of the loader of `slang-glslang-sys` under the feature `vendored`, and the order of creation of Section 6.3 of `docs/design/slang-compiler.md`.
- `crates/slang-compiler/src/fake.rs`: The test-only fake of the Slang interfaces built on the generated vtable structs, which the unit tests run under Miri (SLC-25). Its vtable functions are defined `extern "system"` and stored in their fields through a conversion of the function pointer (Section 13.4 of `docs/design/slang-compiler.md`). Its global session records each session descriptor and each shared library loader that it receives, and its session writes into its global session after releasing it, as Slang's session does, so that Miri reports a wrong release order. Its modules, entry points, component types, and metadata model the counting behaviours of Section 5.3 of `docs/design/slang-compiler.md`. A fake metadata object created with records of cooperative types answers `castAs` with a cooperative-types interface that lives inside it and has no reference count of its own (Section 13.4 of `docs/design/slang-compiler.md`).
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
  - `Cargo.toml`, which declares the features `default`, `wgpu`, `rhi`, and `vendored`, and inherits `slang-compiler`, the optional `pollster`, `slang-rhi`, and `wgpu`, and the development dependency `insta` from `[workspace.dependencies]`.
  - `src/` with the shared private modules `buffer`, `call`, `device`, `draw`, `encoding`, `error`, `kernel`, `layout`, `marshal`, `module`, `render`, `sampler`, and `texture`, which name no backend item and hold no backend `cfg`. Their unit tests sit under `src/buffer/`, `src/call/`, `src/draw/`, `src/encoding/`, `src/kernel/`, `src/marshal/`, `src/render/`, `src/sampler/`, `src/texture/`, and, with their `insta` snapshots, `src/layout/`. `src/layout/tests/ffi/targets.rs` walks the test shaders on SPIR-V with the scalar layout and on DXIL, with its snapshots in `src/layout/tests/ffi/snapshots/`.
  - The private backend module `wgpu_backend` in `src/wgpu_backend.rs`, which the crate root imports as `backend` in the build with `wgpu` (SLO-34 of `docs/design/slango.md`), with its modules `device`, `buffer`, `module`, `texture`, `sampler`, `kernel`, `render`, and `submit` under `src/wgpu_backend/` and the unit tests of `device`, `texture`, `sampler`, `kernel`, `render`, and `submit` in a `tests.rs` under the directory of each. It holds every use of wgpu and `pollster` outside the public module `wgpu`.
  - The private backend module `rhi_backend` in `src/rhi_backend.rs`, which the crate root imports as `backend` in the build with `rhi`, with its modules `device`, `module`, `kernel`, `buffer`, `error`, `texture`, `sampler`, `render`, and `varying` under `src/rhi_backend/`, the unit tests of `kernel` and `render` in a `tests.rs` under the directory of each, and the unit tests of `varying` in `src/rhi_backend/varying/tests.rs` with its `ffi` test in `src/rhi_backend/varying/tests/ffi.rs`. It holds every use of `slang-rhi`. `render` holds the key of the merge of the bindings of the backend, the streams with the stride 0 of an unused attribute, and the checks of Section 7.7 of `docs/design/slango.md`, and `varying` holds the second compile to SPIR-V that reads the usage of the vertex inputs on D3D12 (Section 7.6 of that design).
  - The public module `wgpu`.
  - The hidden public module `__private`, with the types of the interface description in `src/__private/interface.rs` and its unit tests under `src/__private/`.
  - The integration-test binary `tests/slango/`, whose module `gpu` holds the tests of the `gpu` tier. They include these tests:
    - Loading, in `gpu/load.rs`.
    - The embedded load of `__private::load_embedded` on both backends, in `gpu/precompiled.rs`.
    - The call, in `gpu/call.rs` and `gpu/call/`, with the matrices of every shape in `gpu/call/matrices.rs` and the resources and blocks among the parameters of the entry point in `gpu/call/entry_point.rs`.
    - Textures and sampling, in `gpu/texture.rs` and `gpu/sample.rs`.
    - Render pipelines and draws, in `gpu/render.rs` and `gpu/render/`, with the vertex streams of unused attributes, a new depth target, resources among the parameters of a render entry point, and the refusals of the backend `rhi` in `gpu/render/streams.rs`.
  - The shaders that the tests load, under `tests/slango/shaders/`.
- `crates/slang-slint/`: The crate `slang-slint` that Project Status describes. It has these parts:
  - `Cargo.toml`, which sets the licence `GPL-3.0-or-later` and the `documentation` field, declares the feature `vendored`, inherits `slango` with its feature `wgpu` named and `slint` from `[workspace.dependencies]`, and enables the Slint features of SLS-1 of `docs/design/slang-slint.md`.
  - `src/lib.rs` with the crate-level documentation, the re-exports, and `ReadmeDoctests`.
  - The private modules:
    - `device` holds `wgpu_configuration` and `device`.
    - `error` holds the error type and its unit tests.
    - `view` holds `View` with its two `compile_fail` doctests.
  - The integration-test binary `tests/slang_slint/`, with these modules:
    - `configuration` and `device` hold the pure tests.
    - `gpu` holds the tests of the `gpu` tier.
    - `common` holds their helpers.
- `crates/slango-codegen/`: The generator crate that Project Status describes. It has these parts:
  - `Cargo.toml`, which declares the feature `vendored`, inherits `proc-macro2`, `quote`, and the development dependencies `insta`, `prettyplease`, and `syn` from `[workspace.dependencies]` and declares `slango` with an exact version and its feature `wgpu` itself (SLM-2 of `docs/design/slango-macros.md`).
  - `src/lib.rs` with the crate-level documentation, `generate`, `generate_precompiled`, and `ReadmeDoctests`.
  - The private modules:
    - `emit` writes the tokens, wraps the items of each backend in the helper macros of `slango`, orders the embedded modules in `in_import_order`, and has its unit tests in `src/emit/tests.rs`.
    - `emit::structs` in `src/emit/structs.rs` writes the structs for Slang structures and has its unit tests in `src/emit/structs/tests.rs`.
    - `emit::pretty` in `src/emit/pretty.rs`, compiled only for tests, formats generated tokens with the items inside each helper macro formatted as items, and the integration test includes it with `#[path]`.
    - `emit::types` in `src/emit/types.rs` holds the Rust types of Slang values, the Rust type of a generated struct, the operation constant `GENERATE` of the errors of unsupported input, and the error of an unsupported parameter.
    - `names` holds the naming rules and the search for conflicts.
    - `error` holds the error type.
  - The integration-test binary `tests/codegen/`, whose module `ffi` holds the tests that compile the shaders under `tests/codegen/shaders/`, with their `insta` snapshots under `tests/codegen/snapshots/`.
- `crates/slango-macros/`: The procedural macro crate that Project Status describes. It has these parts:
  - `Cargo.toml`, which declares the feature `vendored`, and inherits `slango-codegen`, `syn`, and the development dependencies `slango`, with its feature `wgpu` named, and `trybuild` from `[workspace.dependencies]`.
  - `src/lib.rs` with the crate-level documentation and `shader!`.
  - The build script `build.rs`, which includes the logic in `instructions.rs` (SYS-8 of the coding standard).
  - The integration-test binary `tests/macros/`, with these modules:
    - `bindings` calls the macro and asserts at compile time that the generated types are neither `Send` nor `Sync`.
    - `build_script` includes `instructions.rs`.
    - `ffi` runs the trybuild cases under `tests/macros/ui/` and compares generated structs with Slang's reflection.
    - `gpu` runs the generated saxpy kernel and a kernel over a buffer of a generated struct, and checks that a struct binds only where each field has the same type.
    - `structs` checks the byte conversion of generated structs.
  - The shaders under `tests/macros/shaders/`, which the examples package also compiles. Among them are `imports.slang`, which imports `my_helpers` from `my-helpers.slang`, which imports `base_math` from `base-math.slang`, `layouts.slang`, whose structures have different layouts on the two backends, and `float-fields.slang`, `int-fields.slang`, `narrow-vector.slang`, and `wide-vector.slang`, whose structures differ in the type of a field.
- `examples/Cargo.toml`: The manifest of the unpublished package `slang-rust-examples`, which declares the features `default = ["wgpu"]`, `wgpu`, which enables `slango/wgpu` and the optional dependencies `pollster`, `slang-slint`, `slint`, `wgpu`, and `winit`, `rhi`, which enables `slango/rhi`, and `vendored`, forwarded to `slang-compiler`, `slango`, `slango-macros`, and, when it is on, `slang-slint` (Section 13.7 of `docs/design/slango.md`). It inherits `slango` without its default features, the optional dependencies above, `slang-compiler`, `slango-macros`, and the development dependency `insta` from `[workspace.dependencies]`, and enables Slint's windowing backend on winit and its FemtoVG renderer on wgpu (Section 10 of `docs/design/slang-slint.md`). Its `[[bin]]` entries give `shader-playground` and `windowed-renderer` `required-features = ["wgpu"]`.
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
  - `gpu/generated.rs` calls `slango_macros::shader!` on the shaders of `slango-macros` under `crates/slango-macros/tests/macros/shaders/` by a relative path and runs the generated code on the build's backend (Section 10 of `docs/design/slango-macros.md`).
  - `colour_format.rs` checks the choice of colour format of `windowed-renderer`.
  - `gpu/windowed_renderer.rs` and `gpu/shader_playground.rs` check the scene of each windowed example drawn offscreen and its reload.
  - `colour_format` and the modules of both windowed examples under `ffi` compile only with the feature `wgpu` of the package. `gpu/windowed_renderer.rs` and `gpu/shader_playground.rs` compile and run on both backends, and the helper `scene_texture` of the second returns the texture of the view on the backend `wgpu` and the texture of the scene on the backend `rhi`.
  - The `insta` snapshots are under `snapshots/`.
- `clippy.toml`: The Clippy configuration of Section 4.3 of the coding standard.
- `deny.toml`: The cargo-deny configuration of Section 8 of `docs/design/tooling.md`, whose `[advisories]` table ignores six advisories by the owner's decisions and whose `[bans]` table skips 31 duplicate versions. Its `[licenses]` table accepts `GPL-3.0-or-later` for `slang-slint` alone, through an exception. It accepts `GPL-3.0-only` for every crate, so a reviewer checks that no crate under a permissive licence depends on a crate under the GPL (TOOL-34). Section 8 of `docs/design/tooling.md` lists the licence allow-list, the advisories, and the skips with the reason for each.
- `.config/nextest.toml`: The nextest profiles `default`, `gpu`, and `hw-gpu`, which implement the test tiers as Section 4 of `docs/design/tooling.md` defines. It also holds `default-miri`, which `check-miri` and `check-miri-rhi` use under Miri and which skips the modules named `ffi`, `gpu`, and `hw_gpu`, and an override that keeps the output of the `gpu` tests of `slango`, which name their adapter.
- `taplo.toml`: The taplo configuration, with four-space indentation and the exclusion of `target/` and `.worktrees/`.
- `typos.toml`: The typos configuration, which ignores two correct patterns that typos would otherwise report, and accepts six whole generated identifiers that typos reports: three that the Slang headers misspell and three correct field names of the sampler descriptor of slang-rhi (Section 3.1 of `docs/design/tooling.md`).
- `docs/writing-documentation.md`: The normative writing guide.
- `docs/writing-readmes.md`: The normative guide to the root README and the crate READMEs, which says where each fact for users lives.
- `docs/avoiding-claudelish.md`: The normative guide against the prose habits of text written by large language models, with the specification of the phrase check of `check-prose`.
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
- `docs/design/slang-rhi-sys.md`: The design of `slang-rhi-sys` for M9, which is authoritative for the facts about slang-rhi, the wrapper build, the decision for each kind of declaration of the slang-rhi headers, the ABI probe, the safety contract, the platforms, and the tests of the crate.
- `docs/design/slang-rhi.md`: The design of `slang-rhi` for M9, which is authoritative for its ownership model, its public API of the compute path and of rendering, errors, threading, the guards around native calls, and the tests.
- `justfile`: The root `justfile`, with the groups `checks`, `ci`, `git`, `release`, and `rust`. The group `ci` holds `install-vulkan-sdk`, which only the CI workflow runs, and the group `release` holds `doc-site`, `publish-dry-run`, `publish`, `tag-release`, and `yank`. Its variables `release_packages` and `documentation_site` hold the crates of the next release, in the order of their dependencies, which are the eleven crates of the second release in the order of Section 9.10.2 of `docs/design/tooling.md`, and the base address of the hosted documentation. Its variables `vendored_packages`, `vendored_features`, and `vendored_environment` hold the packages, the feature, and the environment of `test-vendored` and `check-vendored`, which are `slang-src slang-glslang-sys slang-compiler-sys slang-compiler` and `slang-compiler/vendored`. Its variable `rhi_packages` holds the packages of `check-rhi` and `test-rhi`, which are `slang-rhi-sys slang-rhi` (TOOL-52 of `docs/design/tooling.md`). Its variable `miri_packages` holds the packages of `check-miri`, which is `slang-compiler` (TOOL-55 of that design).
- `.github/workflows/ci.yml`: The CI workflow, which Section 7 of `docs/design/tooling.md` specifies. It runs on every push to `main`, every push to a branch that matches `feature/**`, and every pull request.
  - It has the jobs `check`, `test`, `ci-only`, `semver`, `coverage`, and `vendored`. Every job makes libclang available, and each job except `vendored` then installs the Vulkan SDK and checks it with `check-vulkan-sdk`.
  - Its job `vendored` is a matrix of Linux, Windows, and macOS that installs the stable toolchain, `just`, and cargo-nextest, prints the version of git, makes libclang available, installs the missing ones of CMake, Ninja, `g++`, `python3`, `pkg-config`, and `libfontconfig-dev` on Linux, prints the versions of the build tools, prints the Python 3 that it finds or fails, and runs the vendored recipes under `time`. On Linux, it also installs the missing ones of `libx11-dev`, `mesa-vulkan-drivers`, and `libvulkan1`. On Linux and Windows, it runs `check-rhi` and `test-rhi` under `time`, and on Windows it first puts the directory of the `dxcompiler.dll` of the newest Windows SDK on `PATH`. Its timeout is 90 minutes (Section 7.4 of `docs/design/tooling.md`).
  - Its top-level `env` records the Vulkan SDK version `VULKAN_SDK_VERSION` and the SHA-256 checksums of the SDK downloads.
  - Its libclang step installs `libclang-dev` on Linux, and on Windows and macOS checks the libclang of the runner image and sets `LIBCLANG_PATH` to its directory (Section 7.1 of `docs/design/tooling.md`).
  - Its `check` and `coverage` jobs install lavapipe from `mesa-vulkan-drivers` for the tests of the `gpu` tier.
  - Its `check`, `ci-only`, and `coverage` jobs install `libfontconfig-dev`, and its job `vendored` installs it on Linux where it is missing, because a Linux build of Slint links fontconfig.
- `.github/workflows/documentation.yml`: The workflow that builds the hosted documentation with `just doc-site` and deploys it to GitHub Pages on a push of a tag that matches `v*`, which Section 7.5 of `docs/design/tooling.md` specifies.
- `scripts/check-package`: The script of `check-package`, which checks the members, the `documentation` fields, and each packaged `.crate` file.
- `scripts/check-release`: The script of `check-release`, which checks the versions, the `documentation` fields, and the hosted pages of a release on crates.io.
- `scripts/check-doc-site`: The script that `doc-site` runs after `cargo doc`, which checks the page of each crate, writes the index page, and checks the site for symbolic links and its size.
- `scripts/publish-dry-run`: The script of `publish-dry-run`, which runs `cargo publish --dry-run` and writes the record `target/publish-dry-run.commit`.
- `scripts/publish`: The script of `publish`, which refuses unless the checks of Section 9.6 of `docs/design/tooling.md` pass, skips every version that crates.io holds, and publishes the other crates with one run of `cargo publish --no-verify` each, within the rate limits of crates.io (TOOL-51 of `docs/design/tooling.md`).
- `scripts/tag-release`: The script of `tag-release`, which tags `HEAD`, pushes the tag, and creates the GitHub release.
- `scripts/yank`: The script of `yank`, which runs `cargo yank` and refuses in CI and without a token.
- `scripts/self-test-release`: The self-test of the release scripts, which `release-scripts-self-test` runs against stub programs (Section 9.9 of `docs/design/tooling.md`).
- `scripts/install-vulkan-sdk`: The script that downloads the Vulkan SDK on a GitHub Actions runner, verifies it against a checksum, and installs it. It exports `VULKAN_SDK`, the library path on Linux and macOS, and the directory of the SDK's programs to the later steps of the job (Section 7.3 of `docs/design/tooling.md`).
- `scripts/check-vulkan-sdk`: The script that checks that the SDK at `VULKAN_SDK` provides the Slang headers, library, and `slangc` of the build tag that `just recorded-slang` prints.
- `scripts/check-prose`, `scripts/prose-wordlist.txt`, `scripts/prose-phrases.txt`: The prose checker for Markdown files and for the documentation comments of Rust files, its list of non-Canadian spellings, and its list of the phrases of the phrase check, grouped under the hint that it prints for each.
- `scripts/check-rust-comments`: The checker that Rust source holds only API documentation and safety comments.
- `scripts/check-doc-headings`: The checker of the headings in item documentation.
- `scripts/check-doctest-attributes`: The checker that no documentation code block is marked `ignore`.
- `scripts/check-feature-docs`: The checker that the crate-level documentation of each library names every Cargo feature.
- `scripts/check-slang-call-guard`: The checker that every call of `slang-compiler` into Slang runs alone inside the closure of `abort_on_exception`, which reads the names of Slang's functions from `target/slang-bindgen/bindings.rs`, the bindings that its recipe generates first (Section 6.9 of `docs/design/tooling.md`).
- `scripts/check-workflow-pins`: The checker that every action is pinned by a full commit SHA, and that no workflow uses `pull_request_target` or grants more than read access at its top level.
- `scripts/check-doc-references`: The checker that code and configuration cite no project document, section, or rule identifier (Hard Rule 15). It reads the rule identifiers from the tables of the documents under `docs/` on every run.
- `scripts/check-miri-packages`: The checker that the Miri recipes run exactly the workspace members that opt out of `unsafe_code`, apart from the members that it exempts by name (TEST-12 of the coding standard, Section 6.11 of `docs/design/tooling.md`).
- `scripts/rust_lexer.py`: The Python module that the custom checkers share, which reads Rust tokens and documentation comments.
- `scripts/self-test-checker`: The runner of the self-test of each custom checker.
- `scripts/fixtures/`: The cases of each self-test, one directory per checker and per case, with input files that end in `.fixture`. Each case of `check-slang-call-guard` that needs bindings holds them at `target/slang-bindgen/bindings.rs.fixture`, so the self-test runs without `slang-bindgen`.
- `scripts/check-shell`, `scripts/justfile-recipes`: The `shellcheck` run over every shell script and every recipe.
- `scripts/check-commit-message`: The commit message checker.
- `scripts/claude-hook-prose`: The Claude Code hook that runs the prose checker after an edit to a Markdown file or a Rust file.
- `scripts/merge-branch`: The squash merge that `just merge` runs.
- `scripts/worktree-prune`: The cleanup of squash-merged branches that `just worktree-prune` and the `post-merge` hook run.
- `.pre-commit-config.yaml`: The `pre-commit`, `commit-msg`, and `post-merge` hooks.
- `.claude/settings.json`: The Claude Code settings for this repository, with the allowed commands and the hook that runs the prose checker after every edit of a Markdown file or a Rust file. The allowed commands include `just check`, `check-vulkan-sdk`, `check-features`, `check-commit-message`, `recorded-slang`, the three test recipes of the tiers, and every recipe that `just check` runs, apart from `check-slang-call-guard` and its self-test. They exclude recipes such as `fmt`, `merge`, `check-msrv`, `coverage`, `test-vendored`, `check-vendored`, `check-vendored-shared-folder`, `release-scripts-self-test`, and every recipe of the release procedure (Section 5 of `docs/design/tooling.md`).
- `.editorconfig`: The character set, line endings, final newline, and indentation for each file type.
- `.gitattributes`: The single line `* text=auto eol=lf`, which keeps LF line endings in every checkout on every platform (Section 14.3 of `docs/design/tooling.md`).
- `.gitignore`: The files that git ignores. Its last line keeps the directories `target/` of the cases of `check-slang-call-guard` under `scripts/fixtures/`, which the pattern `target` would otherwise ignore.

`target/`, `.worktrees/`, `.code-review/`, and `.idea/` are local and ignored by git. `check-slang-call-guard` writes the generated bindings into `target/slang-bindgen/`, and the vendored recipes build in `target/vendored`. There the build script of `slang-src` keeps its clone in the shared folder `target/vendored/slang-src`, the build script of `slang-compiler-sys` keeps its built libraries and headers in the entries of `target/vendored/slang-src/slang-compiler-sys/`, the build script of `slang-glslang-sys` keeps its built libraries in the entries of `target/vendored/slang-src/slang-glslang-sys/`, and the build script of `slang-rhi-sys` keeps its built libraries and configured header in the entries of `target/vendored/slang-src/slang-rhi-sys/`. The clone itself is the entry `target/vendored/slang-src/src/<prefix>-<submodules>` of SRC-25 of `docs/design/slang-src.md`, whose name `recorded-slang` prints as `RECORDED_CLONE_ENTRY`. `doc-site` writes the hosted documentation into `target/vendored/doc`. `check-miri-rhi` builds in `target/miri-rhi`, where Miri keeps its own clone of Slang in the shared folder `target/miri-rhi/miri/slang-src`. `check-semver` builds each crate and its baseline in a directory of its own under `target/semver-checks/`. `check-package` writes the packages into `target/package`, and `publish-dry-run` writes its record `target/publish-dry-run.commit` and verifies the packages in `target/debug`.

These locations are planned and do not exist yet:

- `docs/design/<crate-name>.md`: One design document for each crate other than `slang-compiler-sys`, `slang-compiler`, `slango`, `slang-slint`, `slango-macros`, `slango-codegen`, `slang-src`, `slang-glslang-sys`, `slang-rhi-sys`, and `slang-rhi`, written as the first task of the crate's milestone.
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
- The user guide to the vendored build in the crate-level documentation of `slang-compiler-sys` and `slang-src`, which documents the shared folder.

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
- `slango`: A high-level convenience library, with a feature set like SlangPy and design patterns like vulkano. It has two runtime backends: wgpu, behind the default Cargo feature `wgpu`, and slang-rhi, behind the opt-in Cargo feature `rhi`, which M10 adds. The two features exclude each other, so each build has exactly one backend, and with neither or both the crate does not compile (SLO-28 and SLO-29 of `docs/design/slango.md`). Since the third code step of M10, the backend `rhi` runs compute kernels, since the fourth also those of the code that `slango-macros` generates, and since the sixth it creates textures and samplers and draws with render pipelines, so the public API works on both backends apart from `slango::wgpu`, with the differences that Section 6.5 of `docs/design/slango.md` states. The accessors of `slango::wgpu` that return a wgpu object return an `Option`, so that a later backend is an additive change (SLO-31). Later milestones add pipeline kinds on the slang-rhi backend only (Sections 15.13 to 15.21 of `docs/design/architecture.md`). It first offers a runtime call API driven by reflection, and its first milestone, M3, covers compute kernels compiled to SPIR-V only.

  In M3 it calls existing compute entry points only, which are functions marked `[shader("compute")]`, with bindings built from reflection. Generated kernels around plain Slang functions, broadcasting over array arguments, and differentiable functions belong to later milestones (Section 8.1 of the architecture design).

  In M3, `slango` uses wgpu 30 and loads every shader through wgpu's validated SPIR-V path, so it has no unsafe code. A load copies what the kernels need into a private, owned layout snapshot and releases every Slang object before it returns, so no Slang object outlives a load. Since the first code step of M10, every type that holds a GPU object, together with `Call` and `Draw`, is neither `Send` nor `Sync`, through a marker field (SLO-17 of `docs/design/slango.md`, CONC-9 of the coding standard). On the backend `rhi`, a device keeps its global session, each load creates a Slang session of its own, and a module keeps its Slang objects (SLO-17 and SLO-37 of `docs/design/slango.md`). A call sets each parameter by name (SLO-1, SLO-2, SLO-5, SLO-6, and Section 9.1 of `docs/design/slango.md`).

  Since M4, it also draws into a texture with render pipelines built from existing vertex and fragment entry points, still with SPIR-V only and no unsafe code. A shader target configurable per backend is deferred with no milestone assigned (Sections 1.4 and 1.5 of `docs/design/slango.md`).
- `slango-macros`: A function-like procedural macro, `shader!`, of M6, that compiles a `.slang` file with Slang during compilation and generates typed Rust bindings to its compute kernels. On the backend `wgpu`, the generated code embeds the precompiled SPIR-V and layout snapshot, and on both backends it calls the same core of `slango` as the runtime API through a hidden module of `slango`. The generator is in the library crate `slango-codegen` (SLM-1, SLM-6, and Section 7 of `docs/design/slango-macros.md`). Since the fourth code step of M10, the generated code wraps the items of each backend in hidden helper macros of `slango`, so it keeps only those of the backend of the program's `slango`. On the backend `rhi`, that is the Slang source of the shader and of every module that it imports, which compiles when the module loads, and structs in the packed layout (SLM-24 to SLM-26 of that design). Every crate that uses it needs Slang at build time, and `slango` does not re-export it in M6 (Section 4.1 of that design). The feature `vendored` of `slango` does not reach the copy of Slang that the macro links, so a program without a Vulkan SDK at build time enables `vendored` on `slango-macros` as well (Section 4.3 of that design).
- `slango-codegen`: The library crate of M6, published in the first release, that holds the generator of `slango-macros`, so that a build script and the crate's own tests can call it (SLM-1 of `docs/design/slango-macros.md`).
- `slang-slint`: An extension to Slint that provides a view backed by a Slang render pipeline, built on `slango`, with wgpu only, permanently. In M5 it is a minimal view that hands a texture that `slango` draws to an ordinary Slint `Image` element, on a narrow range of Slint versions with wgpu 30 only. Slint creates the device from a configuration that `slang-slint` supplies (SLS-1, SLS-6, and SLS-9 of `docs/design/slang-slint.md`).

  The licence consequence of depending on Slint is in Section 4.2 of `docs/design/slang-slint.md`.
- `slang-bindgen`: The binding generator, a published library crate with a thin binary, which parses the Slang headers and the slang-rhi headers with libclang and is a build dependency of `slang-compiler-sys` and of `slang-rhi-sys` (Section 6.2 of `docs/design/architecture.md`).
- The examples, which also serve as the integration testbed and are not published:
  - A command-line program that compiles and reflects Slang source.
  - A headless compute kernel, which later gains a broadcasting case and a differentiable function.
  - A windowed renderer with winit and shader hot reload.
  - A Slint application with a `slang-slint` view.
  - A headless compute kernel called through the typed bindings of `slango-macros`.
  - Planned examples, which Section 10 of `docs/design/architecture.md` lists with their milestones, including the path-tracing demo.

Bindings for slang-rhi, the crates `slang-rhi-sys` and `slang-rhi`, belong to M9 (Section 15.12 of `docs/design/architecture.md`), and `docs/design/slang-rhi-sys.md` and `docs/design/slang-rhi.md` design them. `slang-rhi-sys` builds slang-rhi and generates its raw bindings, and `slang-rhi` wraps them in a safe API for the compute path and, from M10, for the draws that `slango` needs. The crates build on Linux with the GNU C library with the Vulkan backend and, by the owner's decisions of 2026-11-18, on x86_64 Windows with the Microsoft toolchain with the D3D12 backend, offline and with a Slang built with DXIL support. A program on Windows provides `dxcompiler.dll` at run time, and the crates stay unsupported on macOS (Section 4.1 of `docs/design/slang-rhi-sys.md`).

The binding generator produces the `#[repr(C)]` vtable structs, UUID constants, and thunks through which Rust calls the Slang interfaces in the style of the Component Object Model (COM). None of them is written by hand.

Every named C++ enumeration becomes a `#[repr(transparent)]` integer newtype with associated constants, never a Rust `enum`. The enumerators of an unnamed enumeration become constants of the integer typedef that a rule assigns.

No generated binding is kept in the repository or a package. Every build generates the bindings and the list of their C functions, which the native-symbol test of `slang-compiler-sys` uses, from the headers of the Slang that it links, so every build needs libclang and no list of generated names is kept by hand (ARCH-33). The bindings support 64-bit targets only, and `slang-compiler-sys` stops compilation on any other.

The generated declarations of Slang's functions use unwinding ABIs, `"system-unwind"` for vtable entries and `"C-unwind"` for C functions. A callback type and every function that Rust implements for a vtable stay non-unwinding, `"C"` or `"system"`, so that a Rust panic never unwinds into Slang (FFI-7 of the coding standard). The one exception is a function pointer that the library hands to Rust for Rust to call, which a rule names and which is `"C-unwind"`, such as `func` and `payloadDeleter` of `ITaskPool::submitTask` of slang-rhi (SCS-81 of `docs/design/slang-compiler-sys.md`).

The crates target the Slang that ships in the latest stable LunarG Vulkan software development kit (SDK). The recorded Slang is that of Vulkan SDK 1.4.363.1, whose `slangc` reports `2026.17-25-gc7954ebd1`, a commit past the tag `v2026.17`. `slang-src` records its commit and build tag, and the CI configuration records the SDK version (Section 5 of `docs/design/architecture.md`).

No recorded version gates a build. The build records the build tag of the Slang that it links, `slang-compiler` compares the loaded Slang with that record at run time (ARCH-34), and the crates are written and tested against the recorded Slang only, so another Slang is best effort (ARCH-31).

Slang is obtained in exactly one of two ways (Section 4 of `docs/design/architecture.md`):

- By default, the dynamic route links the Slang of an installed Vulkan SDK at `VULKAN_SDK`.
- The opt-in Cargo feature `vendored`, which `slang-compiler-sys` has and which the crates between a user and it forward, selects the vendored route. It builds Slang from the source tree that `slang-src` clones and links it statically. `slang-src` has no environment variable of its own, and the clone and the built libraries live in the shared folder in the Cargo target directory, which every profile and every host and target unit of a project share (Section 4.3 of `docs/design/architecture.md`).

No environment variable selects the route, the build never falls back from one route to the other, and there is no prebuilt download. The recipes pass an explicit list of features and never `--all-features`. Only `test-vendored`, `check-vendored`, `check-rhi`, `test-rhi`, `doc-site`, the vendored CI jobs, and the documentation workflow build the vendored route, and `publish-dry-run` builds it for `slang-glslang-sys`, which always enables the feature on `slang-compiler-sys` (Section 3 of `docs/design/tooling.md`).

Development uses the latest stable Rust toolchain. The workspace declares the minimum supported Rust version (MSRV) 1.99 in `rust-version`, and `check-msrv` verifies it with cargo-hack. The nightly toolchain serves only checkers such as Miri and never builds the crates.

The crates use edition 2024 and Cargo resolver 3. Each crate has the licence that Section 13 of `docs/design/architecture.md` assigns to its kind: the licence of Slang for the `-sys` and `-src` crates, `GPL-3.0-or-later` for `slang-slint`, and `MIT OR Apache-2.0` for every other crate, including the examples package.

CI runs on GitHub Actions on hosted runners for Linux, Windows, and macOS, installs the Vulkan SDK version that the CI configuration records on all three, and runs the default test tier on all three. One job on each platform also builds and tests the vendored route (Section 7.4 of `docs/design/tooling.md`). It also runs the GPU tier on all three, with lavapipe on Vulkan on Linux, the Windows Advanced Rasterization Platform (WARP) on Direct3D 12 on Windows, and hosted Metal on macOS.

Tests that need a GPU adapter sit in a module named `gpu`, which runs on any adapter including lavapipe and WARP, or in a module named `hw_gpu`, which needs a hardware GPU and runs locally only. A test that finds no adapter fails and never skips silently.

The published crates reach crates.io in two releases, in M8 and M11 (Section 13 of `docs/design/architecture.md`). The first release, of eight alpha crates, is on crates.io, and the second, which adds `slang-slint`, `slang-rhi-sys`, and `slang-rhi` and publishes all eleven crates at `0.2.0-alpha.1`, is prepared and not published yet (Section 9.10 of `docs/design/tooling.md`). Every member except the examples package is in `release_packages`, at `0.2.0-alpha.1`. Nothing is published without the owner's explicit go-ahead.

docs.rs is not supported, because it builds without network access and the crates need libclang and either a Vulkan SDK or a clone of Slang. The workflow `.github/workflows/documentation.yml` builds the documentation of each release from its tag on the vendored route and hosts it on GitHub Pages, and the `documentation` field of each crate points there. Only that workflow deploys the site, because a site built elsewhere names the absolute build path of its machine (TOOL-49 of `docs/design/tooling.md`). Section 9 of that design defines the release procedure, in which only the owner publishes, with a token that never enters the repository, CI, or an agent's environment.

The development container provides this environment:

- Rust stable 1.99.1, the default toolchain, with Clippy, rustfmt, rust-src, and llvm-tools.
- A nightly toolchain with Miri and rust-src, used only for checkers.
- cargo-nextest, cargo-deny, cargo-insta, cargo-expand, cargo-semver-checks, cargo-hack, cargo-machete, cargo-llvm-cov, the bindgen command-line interface (CLI), typos, taplo, actionlint, `just`, pre-commit, shellcheck, `gh`, CMake, Ninja, and libclang.
- GitHub's Secure Shell (SSH) host keys in the known hosts file.
- The LunarG Vulkan SDK 1.4.363.1 at `$VULKAN_SDK`, which ships Slang. Its `slangc` reports `2026.17-25-gc7954ebd1`, the headers are under `$VULKAN_SDK/include/slang`, `libslang-compiler.so` is under `$VULKAN_SDK/lib`, and the pkg-config file is `slang-compiler.pc`. A login shell has `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH`.

  A process that no login shell started does not, until the next rebuild of the container image applies the fix that slang-rs-env already contains. The recipes that run compiled code which loads Slang supply that path on their own command line, so they do not depend on it.
- The Slang source tree at `/opt/slang`, at the same commit as the SDK's Slang, with every submodule checked out. It is source only and not built in place, and no recipe reads it.
- The development headers of Xlib from the Debian package `libx11-dev`, and Xwayland from `xwayland`, installed with apt in the running container on 2026-11-18. The image of slang-rs-env does not provide them yet, so a rebuilt container lacks them until the image gains them (Section 10 of `docs/design/tooling.md`).
- Two Vulkan devices: an NVIDIA RTX 4070 and lavapipe, a software implementation of Vulkan.
- The CUDA 13.5 toolkit.

Recipes run Clippy, rustfmt, cargo-nextest, cargo-deny, cargo-hack, cargo-machete, cargo-llvm-cov, cargo-semver-checks, typos, taplo, Miri, shellcheck, actionlint, and the custom checkers under `scripts/`. The recipes of the release also run `cargo package`, `cargo publish`, git, `gh`, and curl through their scripts, and `publish` runs curl to ask crates.io which versions it holds. The recipe `check-semver` runs cargo-semver-checks (TOOL-38 of `docs/design/tooling.md`).

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

No pushed history is rewritten. `just merge` deletes only the merged branch on `origin`, so the branch that was pushed for CI stays there after the merge, and `just worktree-prune` removes its local branch and worktree. By the owner's instruction of 2026-11-19, the agent then checks that `main` holds the tree of that branch and deletes the branch on `origin` (Section 14.7 of `docs/design/tooling.md`).

The `review` skill runs the code-review workflow on a branch, writes its report under `.code-review/`, and reads this file as the project rules. The workflow does not find design documents under `docs/design/` by itself, so the agent passes the path of the design document as an explicit argument, such as `docs/design/<name>.md`.

A reviewer checks judgement rules only, as Hard Rule 4 requires. The agent MUST fix every finding labelled Important before the merge, because such a finding is a defect that would reach `main`. The agent defers the nits and lists them in its report.

## Validating Changes

The agent runs the checks that match what changed, and `just check` before every merge. Section 3.2 of `docs/design/tooling.md` lists every recipe, and its Section 14.4 says what a failure of each one means and what to do.

To run every check:

```sh
just check
```

`just check` runs `check-prose`, `check-shell`, the self-test of each checker, the self-test of the release scripts, `check-typos`, `check-toml`, `check-workflows`, `check-workflow-pins`, `check-doc-references`, `check-miri-packages`, and `check-rust`, in that order, and Section 3.3 of `docs/design/tooling.md` lists every recipe in it. It needs network access, because `check-deny` fetches the advisory database on every run, the nightly toolchain, because `check-miri` runs Miri on it, and a Vulkan SDK at `VULKAN_SDK` and libclang, because the build script of `slang-compiler-sys` generates the bindings from the SDK's headers with libclang and links its Slang. The development container provides the nightly toolchain, the SDK, and libclang.

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

To lint, document, and test the build and the bindings of slang-rhi, the safe crate over them, the backend `rhi` of `slango`, and the generated code of `slango-macros` on that backend after a change to `slang-rhi-sys`, to `slang-rhi`, to `slango`, to `slango-codegen`, to `slango-macros`, to the examples package, to `slang-bindgen` or its rule set `SLANG_RHI`, to the required submodules of `slang-src`, or to the rhi recipes, run these recipes. They build the packages of the `justfile` variable `rhi_packages` on the vendored route in `target/vendored`, then `slango` with `--no-default-features --features rhi` there, and then the examples package with `--no-default-features --features rhi,vendored`, run on Linux and on x86_64 Windows, and are not part of `just check` (TOOL-52 and TOOL-53 of `docs/design/tooling.md`). A change to the shared modules of `slango` therefore needs `test-rhi` as well as `just check`, because each build compiles one backend. They need what the vendored recipes need, and on Linux the development headers of Xlib, without which the configure step of slang-rhi stops with a message that names `X11/Xlib.h` and `libx11-dev` (RSYS-17 of `docs/design/slang-rhi-sys.md`).

`test-rhi` runs 229 tests with the profile `gpu` and then the doctests of the slang-rhi crates, and then the tests and doctests of `slango` on the backend `rhi`. The 51 tests of `slang-rhi-sys` are the tests of the build script, the `defaults`, the symbol, ABI probe, and smoke tests, and two `gpu` tests. The 178 tests of `slang-rhi` are 117 unit tests and 61 `gpu` tests, 27 of them render tests, and its doctests are 27 examples and 33 `compile_fail` cases. On the backend `rhi` on Linux, `slango` runs 379 tests and skips 1, the test of depth clipping, which Vulkan clamps, and its two doctest binaries pass 22 and 18 doctests. On Windows, it skips 3 other tests instead, those of RKB-2 of `docs/design/slang-rhi.md` (Section 13.7 of `docs/design/slango.md`). The examples package then runs 47 tests on that backend, among them the `gpu` tests of the generated code of `slango-macros` and the scenes of both windowed examples. Each command runs all its tests after a failure, and the recipe runs every command after a failing one, then names each failed command and fails (TOOL-53 of `docs/design/tooling.md`). The `gpu` tests open a device on a Vulkan adapter on Linux and a D3D12 adapter on Windows, and they fail, and never skip, without one, so on Linux the recipe needs the Vulkan loader and a driver, such as lavapipe, and on Windows `dxcompiler.dll` on `PATH`. With `VK_DRIVER_FILES` set to the file `lvp_icd.json` of lavapipe, they run on lavapipe instead of the NVIDIA adapter of the development container. The agent runs `test-rhi` after every change to `slang-bindgen`, because both rule sets share one generator.

On 2026-11-18 on 16 cores in the development container, while another agent's checks loaded the machine, `check-rhi` from an empty `target/vendored` took 9 min 37 s and `test-rhi` from an empty `target/vendored` took 16 min 23 s. A repeat of either took 1 s. The build of slang-rhi alone from an empty entry took 23.7 s, its entry in the shared folder is about 3.0 MB, and the clone with `external/slang-rhi` is 296 MB by `du -sh`. On 2026-11-18, the first build of `slango` with the feature `rhi` in an empty `target/vendored`, which cloned and built Slang, slang-glslang, and slang-rhi, took 10 min 36 s:

```sh
just check-rhi
just test-rhi
```

To run the unit tests of `slang-rhi` under Miri after a change to its unsafe code, run this recipe. `check-miri` does not run the crate, because its build scripts build Slang and slang-rhi from source (TOOL-32 of `docs/design/tooling.md`), so this recipe runs them in a target directory of its own, `target/miri-rhi`, on Linux only and outside `just check` and CI (TOOL-52). It needs the nightly toolchain, and its first run needs what `check-rhi` needs, clones Slang again into `target/miri-rhi/miri/slang-src`, and builds Slang, slang-glslang, and slang-rhi natively for the build scripts. On 2026-11-18 in the development container, its first run took 8 min 33 s, and on 2026-11-21, after the first code step of M11, it took 15 min 52 s from an empty `target/miri-rhi` and printed "117 tests run: 117 passed, 61 skipped":

```sh
just check-miri-rhi
```

To check, after any of these recipes, that the shared folder `target/vendored/slang-src` holds exactly one clone, the complete entry that `recorded-slang` names, and that no other directory under `target/vendored` holds a clone, and to print each entry and the sizes of the folder and the clone, run this recipe. A clone of an earlier layout, `src/<commit>` or `src/<commit>-<submodules>` with the full commit hash, that an earlier build left in the shared folder fails it until it is removed. It builds nothing and needs no network access, and `just check` does not run it:

```sh
just check-vendored-shared-folder
```

To print the recorded Slang commit, its build tag, and the name of its clone entry of `crates/slang-src/recorded.rs` as `NAME=value` lines:

```sh
just recorded-slang
```

To check the dependencies after a change to a manifest:

```sh
just check-deny
just check-unused-deps
```

To run the tests under Miri after a change to unsafe code, run this recipe. It runs only the packages of the `justfile` variable `miri_packages`, which is `slang-compiler`, because Miri finds undefined behaviour only in unsafe code (TEST-12 of the coding standard). It fails when a package has no test that Miri runs. Miri cannot call native code or start a process, so the profile `default-miri` skips every test in a module named `ffi`, `gpu`, or `hw_gpu`.

Miri runs every unit test of `slang-compiler` outside the module `guard::tests::ffi`, whose tests start a child process. These test the owning interface pointer, the Slang call guard, the global session, the session, the compiler options, modules, component types, blobs, and metadata against the fake of Slang's interfaces in `crates/slang-compiler/src/fake.rs`.

The fake frees each object when its count reaches zero, so that Miri detects a use after free. The build script still runs natively, so the recipe needs `VULKAN_SDK` and libclang:

```sh
just check-miri
```

To check the spelling of every file, and the formatting of every TOML file:

```sh
just check-typos
just check-toml
```

To check the prose of a Markdown file, or of the documentation comments of a Rust file, against Section 8 of the writing guide and the phrase check of Section 6 of `docs/avoiding-claudelish.md`, run this command. The Claude Code hook also runs it after every edit of a Markdown file or a Rust file:

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

To check that the Miri recipes run exactly the workspace members that opt out of `unsafe_code`, apart from the three `-sys` crates that the checker exempts by name, run this recipe after a change to a lint exception for `unsafe_code`, to `miri_packages`, to a Miri recipe, or to the workspace members (Section 6.11 of `docs/design/tooling.md`):

```sh
just check-miri-packages
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
just check-miri-packages-self-test
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

- `check-package` after a change to a manifest, to `release_packages` or `documentation_site`, to a licence file, or to the README of a crate. Cargo refuses a packaged file with an uncommitted change, so it runs only after the commit. It runs on Linux and macOS only and reaches `static.crates.io`.
- `publish-dry-run` after the same changes, on a committed tree as well, on Linux and macOS only. It needs `VULKAN_SDK`, libclang, network access, and the tools of the vendored build, clones and builds Slang in `target/`, took about 12 minutes in the development container, and writes the record that `publish` checks.
- `doc-site` after a change to the API documentation of a crate of the release, or to the recipe. It needs what `check-vendored` needs, builds in `target/vendored`, and passes `cargo doc` only the features `vendored` of the packages that it selects, which it reads from `cargo metadata`. Its site holds the pages of the eleven crates of `release_packages` (Section 9.10.4 of `docs/design/tooling.md`). Its site is for reading only, and an agent never deploys it (TOOL-49 of `docs/design/tooling.md`).
- `check-release` after a release, to check it. It reaches crates.io and the hosted site, and fails when crates.io lacks a version of the release, when a `documentation` field differs, or when a documentation address does not answer.

```sh
just check-package
just publish-dry-run
just doc-site
just check-release
```

An agent MUST NOT run `publish` or `yank`, which only the owner runs, with a token (TOOL-46 of `docs/design/tooling.md`), and MUST NOT run `tag-release` without the owner's explicit go-ahead in the current session (TOOL-44). Each of them acts in public, and a published version can never be overwritten or deleted.

Four more recipes are not part of `just check`, because they are meant for CI. CI runs all four in every run of the workflow.

- `check-features` compiles the workspace once for each combination of Cargo features, without the feature `vendored`. It checks `slango` and the examples package in a second command, which also leaves out the feature `rhi` and their builds with no features, because `rhi` builds Slang from source and the build of `slango` with no features stops on purpose with a compile error (TOOL-48 of `docs/design/tooling.md`). `test-rhi` and `check-rhi` build `slango` with `rhi` instead.
- `check-msrv` checks the workspace with the toolchain that `rust-version` names, and its first run installs that toolchain with rustup, which needs network access.
- `check-semver` runs cargo-semver-checks against the last release on crates.io (TOOL-38 of `docs/design/tooling.md`). It needs `VULKAN_SDK`, libclang, and network access, and builds each crate and its baseline in `target/semver-checks/`. While the releases are alphas, it reports no change of the API. It checks `slang-bindgen`, `slang-compiler`, `slang-compiler-sys`, `slango`, and `slango-codegen`. `slango-macros` has no library target, the slang-rhi crates build only on the vendored route, and `slang-slint` stays excluded until crates.io holds a version of it (Sections 9.3 and 9.10.5 of `docs/design/tooling.md`).
- `coverage` writes a coverage report to `target/lcov.info` and prints no coverage summary, and the CI job `coverage` uploads that report as an artifact.

An agent runs `check-features` after a change to the Cargo features, `check-msrv` after a change that may need a newer toolchain, and `check-semver` after a change to the public API of a crate of the release:

```sh
just check-features
just check-msrv
just check-semver
just coverage
```

## Documentation Maintenance

Hard Rule 1 is the rule an agent is most likely to skip, because the code works without it. After every change, and before it reports the change complete, the agent MUST re-read and correct these documents, once each, because a stale document misleads the next agent:

1. The design document of each crate or tool that the change alters, including its design content, its assumptions and open questions, and, for `slang-compiler` and `slang-rhi`, its list of known upstream bugs, together with the entry in `docs/upstream-reports.md` of each defect in it that should be reported. Progress and status are recorded only in Project Status of this file, never in a design document (Section 3.6 of the writing guide).
2. Every statement in another document that the change makes false, which the agent finds by searching for the names that the change touches. `README.md`, the architecture design, the tooling design, and this file change only as Hard Rule 1 states. This includes the README of each crate that the change alters, which is the crate's page on crates.io, and every pointer in a README that the change makes false, as RDM-33 of `docs/writing-readmes.md` requires.
3. `docs/rust-coding-standards.md` with its Appendix A, `docs/rust-references.md`, `docs/writing-documentation.md`, `docs/writing-readmes.md`, or `docs/avoiding-claudelish.md`, only when the change alters a rule, a lint, a tool, a source, or a documentation convention.
4. The Date field of the metadata table of each document the change touched.

## Contribution Policy

The project is developed by its owner with coding agents, under the rules in this file. It has no contribution policy yet.

The project opens no pull requests for now, and a pull request from a fork runs CI with a read-only token and no secrets. A future contributing policy may require disclosure of artificial intelligence (AI) usage in pull request descriptions.

## References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.
