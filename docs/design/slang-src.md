# `slang-src` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-15 |
| Applies to | `crates/slang-src` |

## Abstract

This document specifies `slang-src`, the crate that obtains the Slang source tree for the crates that build Slang from source and exports the location of that tree to their build scripts, and it builds nothing itself. It defines the three sources of the tree, the shallow clone of a recorded Slang commit, the version scheme that carries that commit in the crate's own version, the metadata that the crate exports, the opt-in cache directory that the crates that build Slang share, the environment variables, and the failure modes. It is the one crate of the workspace that reaches the network, so it also states what it fetches, from where, and why a commit hash identifies what it fetched.

## 1. Introduction

### 1.1. Problem

A vendored build of `slang-compiler-sys` builds Slang from source and links it statically, and `slang-glslang-sys` builds glslang and SPIRV-Tools from the same source tree (Section 4 of `docs/design/architecture.md`). Both need the same tree at the same commit, and so does `slang-rhi-sys` later. A build script that clones the tree in each of these crates would clone it several times, could clone different commits, and would put network access into every crate that builds native code.

The tree is large, and a full clone with every submodule is much larger than what the builds read. A user can also need another Slang commit than the one the workspace records, or a build without network access.

### 1.2. Purpose

`slang-src` is the one place that obtains the tree. It holds the recorded Slang commit, clones that commit or another commit that the user selects, or takes a local checkout that the user names, and exports the path, the commit, and the build tag of the tree to the build scripts of the crates that depend on it. It also names the cache directory that those crates share.

This document implements the parts of Sections 4, 5, and 15.10 of the architecture design that concern the source tree.

### 1.3. Audience

The primary readers are the coding agents that change `slang-src`, the build scripts that depend on it, or the recorded commit. The secondary readers are the reviewers of those changes and the owner of the repository, who approves the decisions. A reader is expected to know Cargo build scripts and git, but not the layout of the Slang tree.

### 1.4. Scope

This document covers:

- The sources of the tree and the order in which the build script chooses one.
- The clone, its submodules, and its git settings.
- The recorded commit, its build tag, and the version scheme of the crate.
- The metadata that the build script exports.
- The cache directory, its layout, its keys, and its locking, which every crate that builds Slang from the tree follows.
- The cost of building twice when a procedural macro links Slang.
- The crate documentation, the security considerations, and the operational considerations.

### 1.5. Non-Goals

This document does not cover:

- The native builds of Slang and of slang-glslang, which `docs/design/slang-compiler-sys.md` and `docs/design/slang-glslang-sys.md` specify. `slang-src` builds nothing and runs no code of the tree.
- The dynamic route, which never depends on `slang-src`.
- The recipes and the CI jobs, which `docs/design/tooling.md` specifies. This document states the requirements that the crate puts on them.
- An offline mirror, a proxy, or a download of anything other than the Slang repository and its submodules.
- A cleanup or eviction of the cache directory, which the user removes by hand (Section 9.3).

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that these documents define:

- Section 2.2 of `docs/writing-documentation.md`, such as agent, role, and user.
- Section 2.2 of `docs/rust-coding-standards.md`, such as native library and `-sys` crate.
- Section 2.1 of `docs/design/architecture.md`, such as CMake, pre-release identifier, procedural macro, shallow clone, slang-glslang, vendored build, and vendored route.
- Section 2.1 of `docs/design/slang-compiler-sys.md`, such as build tag, gitlink, host, and work tree.

This document calls those files the writing guide, the coding standard, the architecture design, and the sys design. It adds these terms:

- build metadata: The part of a Semantic Versioning (SemVer) 2.0.0 version [SEMVER] after a plus sign, which Cargo ignores when it compares versions.
- cache directory: The directory that `SLANG_SRC_CACHE_DIR` names, which keeps clones and built libraries across target directories, profiles, and projects (Section 9).
- cache entry: One directory of the cache directory that holds one clone or the libraries of one native build, complete or absent, never partial (Section 9.2).
- cache key: The name of a cache entry of built libraries, derived from every input that changes those libraries (Section 9.4).
- clone entry: The directory that holds one clone, its build tag file, and nothing else, either under `OUT_DIR` or in the cache directory (Section 6.3).
- dependent build script: The build script of a crate that depends directly on `slang-src` and reads its metadata, such as the build scripts of `slang-compiler-sys` and `slang-glslang-sys`.
- object ID: The name of a git object, which is the SHA-1 hash of its content. A commit hash is the object ID of a commit.
- recorded commit: The Slang commit that `slang-src` clones when the user selects no other source, which is the commit of the Slang in the Vulkan SDK that CI installs (Section 7).
- recorded Slang: The Slang build at the recorded commit, named by its build tag.
- required submodule: One of the submodules of the Slang tree that the dependent build scripts read (Section 5).
- source tree: The Slang work tree that `slang-src` exports, with its required submodules checked out.

## 3. Facts

This section is non-normative. It states the facts on which the later sections rely.

### 3.1. Cargo

A build script exports `links` metadata with `cargo::metadata=KEY=VALUE`. Cargo passes it to the build script of each package that depends directly on the package, as the environment variable `DEP_<LINKS>_<KEY>`, with the `links` name in upper case and each hyphen replaced by an underscore. A package that depends on it only indirectly does not see it.

The `links` key also stops two versions of the package from entering one build graph.

A build script cannot see the command-line option `--offline`. Cargo passes the environment variable `CARGO_NET_OFFLINE` to a build script only when the user sets it in the environment.

Each target directory and each profile has its own `OUT_DIR` for a build script, so a build script that writes only into `OUT_DIR` repeats its work in every target directory and profile.

A pre-release identifier made only of digits compares numerically, and any other identifier compares as ASCII text and ranks above every numeric identifier. A numeric identifier with a leading zero is not valid. A version with a pre-release orders below the same version without one.

A caret requirement on a pre-release version, such as `"0.1.0-slang.2026.17.0.25.gc7954ebd1"`, matches every later pre-release of `0.1.0`, so `cargo update` moves it to a version that carries another Slang. Only the exact requirement `"=0.1.0-slang.2026.17.0.25.gc7954ebd1"` holds one version. A requirement such as `"0.1"` matches no pre-release at all.

### 3.2. git

`git clone --depth 1` cannot fetch a commit by its hash. `git init` followed by `git fetch --depth 1 origin <commit>` can, and GitHub serves any commit that a branch or tag of the repository reaches. `git submodule update --init --depth 1 -- <paths>` fetches the commit that each gitlink records, also when the default branch of the submodule does not contain it.

A shallow clone holds no tags, so `git describe` fails in it. `git describe --tags` on a full history prints the nearest tag, the count of commits after it, and `g` followed by the commit hash abbreviated to a length that depends on the size of the repository. `--abbrev=9` fixes that length at nine, which is the length in the build tags of the Vulkan SDK.

git verifies the object ID of every object it receives, so a fetched commit, its trees, and its files are the content that the hash names. Every gitlink records the commit of its submodule by object ID, so the commit of the superproject determines the content of every submodule. git uses a variant of SHA-1 that detects the known collision attacks.

The git settings `http.lowSpeedLimit` and `http.lowSpeedTime` stop a transfer that stays below a rate for a time. git has no setting that bounds the time to connect, and the operating system then bounds it (Section 15, SRCA-2).

### 3.3. The Slang Tree

The Slang repository is `https://github.com/shader-slang/slang.git`, and its submodules are under `external/`. Slang's CMake derives its version from `git describe` with tags that match `v20[2-9][0-9].[0-9]*`, and it falls back to `0.0.0-unknown` without one (Section 3.6 of the sys design).

These are the required submodules of the recorded Slang:

| Submodule | Read by |
| --- | --- |
| `external/cmark` | `slang-compiler-sys` |
| `external/fast_float` | `slang-compiler-sys` |
| `external/lua` | `slang-compiler-sys` |
| `external/lz4` | `slang-compiler-sys` |
| `external/miniz` | `slang-compiler-sys` |
| `external/spirv-headers` | `slang-compiler-sys` and `slang-glslang-sys` |
| `external/unordered_dense` | `slang-compiler-sys` and `slang-glslang-sys` |
| `external/vulkan` | `slang-compiler-sys` |
| `external/glslang` | `slang-glslang-sys` |
| `external/spirv-tools` | `slang-glslang-sys` |

`external/mimalloc` is not required, because the vendored build of `slang-compiler-sys` passes `-DSLANG_ENABLE_SPIRV_TOOLS_MIMALLOC=OFF` with the Microsoft toolchain (Section 9.3 of the sys design) and the wrapper project of `slang-glslang-sys` sets both mimalloc options of SPIRV-Tools to `OFF` on every target (SGL-3 of `docs/design/slang-glslang-sys.md`). Section 15, SRCA-3, records that no other submodule is needed on Windows or macOS.

A shallow clone of the recorded commit with these submodules takes about half a minute on a fast connection and occupies under 300 MB.

## 4. Crate and Metadata

SRC-1. The manifest of `slang-src` MUST declare `links = "slang-src"`. Its build script MUST export, with `cargo::metadata`, the keys `root`, `commit`, and `build_tag` on every successful run, and the key `cache_dir` only when Section 9.1 permits the dependent build scripts to use the cache. Every crate that reads them MUST depend on `slang-src` directly, because Cargo passes `links` metadata only to direct dependents (Section 3.1), and `links` keeps one version of the tree in each build graph.

A dependent build script reads them as `DEP_SLANG_SRC_ROOT`, `DEP_SLANG_SRC_COMMIT`, `DEP_SLANG_SRC_BUILD_TAG`, and `DEP_SLANG_SRC_CACHE_DIR`:

| Key | Value |
| --- | --- |
| `root` | The absolute path of the top level of the source tree, in UTF-8 without line breaks |
| `commit` | The full commit hash of the source tree, 40 lowercase hexadecimal digits |
| `build_tag` | The build tag of the source tree, in the form that SRC-12 defines, such as `2026.17-25-gc7954ebd1` |
| `cache_dir` | The absolute path of the cache directory, present only when SRC-15 permits its use |

The build tag is the value that a dependent build script passes to Slang's CMake as `SLANG_VERSION_FULL`, and the part before its first hyphen is the numeric version (Section 9.3 of the sys design).

SRC-2. `slang-src` MUST have no unsafe code, no dependencies, and no build dependencies. Its build script MUST keep its logic in a file that the build script and a test both include, behind a trait for everything that it reads, runs, or writes (SYS-8). Every dependency of a crate that every vendored build compiles runs on every user's machine, and the standard library reads the environment, starts git, and locks and renames files without help.

The library of the crate holds the crate-level documentation and no public item. The crate exists for its build script and its metadata, and a public item would be an API that nothing needs and that every release would have to keep.

The crate is created under the licence of Slang, `Apache-2.0 WITH LLVM-exception`, which its manifest sets in its own `license` field instead of inheriting the licence of the workspace (ARCH-36 of the architecture design, TOOL-4 of `docs/design/tooling.md`).

The crate has these files:

- `build.rs` includes `recorded.rs`, `source.rs`, and `system.rs`, calls the logic with the real host, prints every instruction, and returns the result from `main`.
- `recorded.rs` holds the recorded commit and its build tag (Section 7.1).
- `source.rs` holds the logic, as plain functions behind the trait `Host`.
- `system.rs` holds the struct `System`, which implements `Host` with the standard library.
- `src/lib.rs` holds the crate-level documentation (Section 12).

## 5. Selecting the Source

SRC-3. The build script MUST choose the source of the tree in this order, and MUST NOT choose it by any other input:

1. When `SLANG_SOURCE_DIR` is present, the local checkout that it names (Section 6.1).
2. Otherwise, when `SLANG_SRC_COMMIT` is present, a clone of that commit (Section 6.4).
3. Otherwise, a clone of the recorded commit (Section 6.2).

Every other input would be another way to build a Slang that nobody chose. A variable is present when the environment contains it, whatever its value. A present variable with an empty value stops the build with the error of Section 11 that names it, because a user who set a variable asked for its source, and a silent fall-through would build another one.

When `SLANG_SOURCE_DIR` and `SLANG_SRC_COMMIT` are both present, `SLANG_SOURCE_DIR` takes effect and the build script does not read `SLANG_SRC_COMMIT`. The crate documentation states this order.

SRC-4. With every source, the build script MUST check that each required submodule of Section 3.3 has a gitlink in the tree and a `.git` entry, a file or a directory, inside its directory. It MUST stop the build when one does not, with a message that names the submodule and the command `git -C <tree> submodule update --init --depth 1 -- <submodule>`. A dependent build that reaches a missing submodule fails inside CMake with an error that does not say what is missing.

The list of required submodules is a constant of `source.rs`. A change of the native builds that reads another submodule changes this list and Section 3.3 in the same change.

## 6. Obtaining the Tree

### 6.1. A Local Checkout

SRC-5. When `SLANG_SOURCE_DIR` is present, the build script MUST stop the build when the value is empty, is not valid UTF-8, contains a line feed or a carriage return, is not an absolute path, or does not name an accessible directory. It MUST then require, through `git rev-parse`, that the directory is the top level of a git work tree, and MUST read its commit with `git rev-parse --verify HEAD^{commit}`.

A line break in the value would end a Cargo instruction early and let the rest inject an instruction of its own. A relative path would let git and the dependent builds resolve it against different directories. A directory below the top level of a work tree is not a Slang checkout, and a tree without git metadata, such as a source archive, has no commit to record.

SRC-6. The build script MUST use the checkout of `SLANG_SOURCE_DIR` as it is, and MUST NOT write into it, clone into it, or check whether its files differ from its commit. The dependent build scripts read it and never write into it either (Section 9.3 of the sys design and Section 4 of `docs/design/slang-glslang-sys.md`).

The bindings are generated from the headers of the tree that is built, so a checkout with local changes builds a Slang whose bindings match it. A user who names a checkout chose its content, and a build that refused a changed tree would stop the work of a user who tests a change to Slang.

The build tag of a local checkout comes from `git describe --tags --abbrev=9 --match 'v20[2-9][0-9].[0-9]*' HEAD` in the checkout, without its leading `v`, checked as SRC-12 requires. A checkout whose history holds no such tag, such as a shallow clone, stops the build with the error of Section 11, which names the command `git -C <tree> fetch --tags`.

This command builds a crate with a vendored Slang from a local checkout at `/tmp/slang`, as an illustration:

```sh
SLANG_SOURCE_DIR=/tmp/slang cargo build --features vendored
```

### 6.2. The Recorded Commit

When neither `SLANG_SOURCE_DIR` nor `SLANG_SRC_COMMIT` is present, the build script clones the recorded commit with the steps of Section 6.3 and exports the build tag that `recorded.rs` records. It runs no `git describe` for the recorded commit, because the build tag is known and a describe needs a second fetch.

### 6.3. The Clone

SRC-7. The build script MUST clone with these commands, in this order, in an empty temporary directory, and MUST NOT run a git command that fetches anything else:

1. `git init` in the temporary directory, followed by `git remote add origin https://github.com/shader-slang/slang.git`.
2. `git fetch --depth 1 origin <commit>`, with the full commit hash.
3. `git checkout --detach FETCH_HEAD`.
4. `git submodule update --init --depth 1 -- <path>...`, with the paths of the required submodules of Section 3.3.

It MUST then check the required submodules (SRC-4), write the build tag into the file `build-tag` of the clone entry, and rename the temporary directory to the clone entry only after every step succeeded. A clone entry that exists is therefore complete, and an interrupted clone leaves only a temporary directory, which the next run removes before it clones again.

The clone entry has this layout:

| Path in the clone entry | Content |
| --- | --- |
| `tree` | The work tree with its `.git` directory and the required submodules |
| `build-tag` | The build tag of the tree, as one line |

Without the cache directory, the clone entry is `OUT_DIR/clone` and its temporary directory is `OUT_DIR/clone.tmp`. With it, the clone entry is in the cache directory (Section 9.2).

On a later run of the build script, a clone entry whose `tree` reports the expected commit through `git rev-parse --verify HEAD^{commit}` is used without any network access. A clone entry that reports another commit, or that git cannot read, is removed and cloned again, except in the cache directory, where the build script stops the build with the error of Section 11 that names the entry, because another build may be reading it.

SRC-8. The build script MUST run every git command that reaches the network with the settings `protocol.allow=never`, `protocol.https.allow=always`, `http.lowSpeedLimit=1000`, and `http.lowSpeedTime=60`, passed with `-c`, and with `GIT_TERMINAL_PROMPT=0` in its environment. It MUST run every git command with `GIT_OPTIONAL_LOCKS=0` and with the repository variables that `git rev-parse --local-env-vars` lists removed from its environment.

Only HTTPS can then carry the clone, so no URL in `.gitmodules` can make git start an SSH client or read a local repository. git fails instead of asking for credentials, which it does when a repository or a commit does not exist, and a build would otherwise wait for input that never comes. A transfer that stalls below 1000 bytes per second for 60 seconds fails instead of hanging.

A git hook that runs Cargo sets repository variables such as `GIT_DIR`, and git would then answer for another repository. Without `GIT_OPTIONAL_LOCKS=0`, `git status` and other read commands refresh the index of a tree that another build may be reading.

The build script runs every git command with `std::process::Command`, checks every exit status (SYS-4), and starts each with standard input closed. The user's git configuration still applies, such as a proxy or a rewrite of URLs, which Section 13 discusses.

SRC-9. The build script MUST NOT reach the network when `CARGO_NET_OFFLINE` is `true`, and MUST then stop the build before it clones with the error of Section 11 for a build without network access. When a clone fails, it MUST stop the build with that error, holding the command and the error output of git. It MUST NOT fall back to another source or to the dynamic route, because a user who enabled `vendored` and silently gets a dynamic link ships a program that needs the Slang shared library at run time.

The error names `SLANG_SOURCE_DIR` as the way to build without network access, and says that a build without the Cargo feature `vendored` links the Slang of an installed Vulkan SDK instead.

### 6.4. A Selected Commit

SRC-10. When `SLANG_SRC_COMMIT` is present, the build script MUST stop the build unless the value is exactly 40 lowercase hexadecimal digits. It MUST clone that commit as Section 6.3 states and MUST derive its build tag with a second fetch, into the bare repository `describe.git` next to the temporary directory, of `git fetch --filter=tree:0 --tags origin <commit>`, followed by `git describe --tags --abbrev=9 --match 'v20[2-9][0-9].[0-9]*' <commit>` in that repository.

A full hash names exactly one commit, while an abbreviated hash or a branch name names whatever the repository holds at the time of the build. The second fetch transfers the commit history and the tags without trees or files, so `git describe` finds the nearest tag, and the clone itself stays shallow.

The build script removes `describe.git` after it read the build tag. A describe that fails or prints a value that SRC-12 rejects stops the build with the error of Section 11.

A selected commit is best effort. The crates are written and tested against the recorded Slang, and the binding generator refuses headers that its rule set does not cover (SCS-2 of the sys design), so another commit can fail to build. The crate documentation states this.

This command builds a crate with a vendored Slang at another commit, as an illustration with a placeholder hash:

```sh
SLANG_SRC_COMMIT=0123456789abcdef0123456789abcdef01234567 cargo build --features vendored
```

## 7. The Recorded Commit and the Version

### 7.1. Recording

SRC-11. The recorded commit and its build tag MUST be recorded only in `crates/slang-src/recorded.rs`, as the two constants `RECORDED_COMMIT` and `RECORDED_BUILD_TAG`, and in the manifest version of `slang-src`, which encodes the build tag as Section 7.2 defines. A test MUST fail when the two disagree. Recipes and scripts that need the recorded commit or build tag MUST read them through the recipe `recorded-slang`, which prints both as `NAME=value` lines.

Two records that can diverge are only safe when a test compares them, and Cargo reads the version only from the manifest, while the build script needs the full commit, which the version abbreviates. The recipe keeps every other consumer, such as the check of the Vulkan SDK in CI, from restating either value.

These are the values of the recorded Slang:

| Constant | Value |
| --- | --- |
| `RECORDED_COMMIT` | `c7954ebd1f1984ad2bfde15f76248db41dc34aa3` |
| `RECORDED_BUILD_TAG` | `2026.17-25-gc7954ebd1` |

The recorded commit is the commit of the Slang in the Vulkan SDK version that CI installs, which the CI configuration records (Section 5.1 of the architecture design). The CI check of the installed SDK compares its build tag with `RECORDED_BUILD_TAG` and fails when they differ, so that every job tests one Slang (Section 7.3 of `docs/design/tooling.md`).

### 7.2. The Version Scheme

SRC-12. A build tag that `slang-src` exports MUST have the form `<major>.<minor>`, optionally followed by `.<patch>`, optionally followed by `-<count>-g<hash>`, in which each of `<major>`, `<minor>`, `<patch>`, and `<count>` is a decimal number without a leading zero, or `0`, and `<hash>` is at least nine lowercase hexadecimal digits. The build script MUST stop the build on any other value, because a dependent passes the part before the first hyphen to CMake as a dotted number (Section 3.10 of the sys design) and the manifest version below needs these parts.

SRC-13. The manifest version of `slang-src` MUST be `<crate version>-slang.<major>.<minor>.<patch>.<count>.g<hash>`, made from `RECORDED_BUILD_TAG` with a missing patch written as `0`, a missing count written as `0`, and the hash abbreviated to nine digits, taken from `RECORDED_COMMIT` when the build tag has none. A release with a new recorded commit MUST raise the minor number of the crate version, and a release that changes only the crate MUST raise its patch number.

The recorded build tag `2026.17-25-gc7954ebd1` gives the version `0.1.0-slang.2026.17.0.25.gc7954ebd1`. This use of the pre-release identifier is a deliberate abuse of the notation: the version is not a pre-release of `0.1.0`, and the identifiers name the Slang that the crate clones.

Numeric identifiers make Slang versions order as numbers, so `2026.17.0.25` orders below `2026.22.0.3`. A build tag written with hyphens, such as `2026.17-25-g...`, would be one identifier compared as text, so `17-25` would order below `17-3`. The patch number is always written, so that a build tag of a patch release, such as `2026.17.1-3-g...`, maps to five numbers like any other.

A new recorded commit raises the minor number because every dependent requires an exact version (SRC-14), so the order between crate versions never selects a version. The minor number then tells a reader that the Slang changed, and the patch number that only the crate did.

SRC-14. Every crate that depends on `slang-src` MUST require its exact version with `=`, because a caret requirement on a pre-release lets `cargo update` move a build to another Slang (Section 3.1). Every release of `slang-src` therefore needs a release of each crate that depends on it, which the release procedure has to cover (TQ-4 of `docs/design/tooling.md`).

This illustration shows the requirement in the manifest of a dependent:

```toml
[dependencies]
slang-src = { version = "=0.1.0-slang.2026.17.0.25.gc7954ebd1", path = "../slang-src" }
```

## 8. Building Twice With a Procedural Macro

This section is non-normative. `docs/design/slango-macros.md` states what a user of `slango-macros` enables.

`slango-macros` links Slang into the procedural macro that the compiler runs on the host. Cargo builds a dependency of a procedural macro as a host unit, with the profile settings of `build-override`, and merges a host unit with the target unit of the same package only when their settings and target agree. Section 15, SRCA-4, records that these facts were measured with stub crates in the shape of the workspace and not with the real crates.

A vendored build of a crate that calls `slango-macros` therefore runs the build scripts of `slang-src` and of the crates that build Slang twice in these cases, once for the host and once for the target:

| Command | Runs each build script |
| --- | --- |
| `cargo build` and `cargo test` in the dev profile without `--target` | Once |
| `cargo build --release` | Twice, because the release profile optimizes while `build-override` does not by default |
| Any build with `--target` | Twice, because host units and target units then have separate directories |
| `cargo check` and `cargo clippy` | Twice for the crates that build Slang, for a reason in Cargo's unit graph that is not established |

Without the cache directory, each run clones and builds in its own `OUT_DIR`, so the cost of the vendored build doubles. Every profile and target directory also has its own `OUT_DIR`, so `cargo build` followed by `cargo build --release` builds Slang three times in total.

The remedies are these two:

- The cache directory (Section 9). The second run waits for the lock of the first run and then uses its entry, and every later target directory, profile, and project uses the same entry.
- For release builds, a `[profile.release.build-override]` table in the user's root manifest whose settings match the release profile, which makes Cargo merge the host and target units. Its cost is that every build script and procedural macro of the user's graph is compiled with optimization.

This illustration shows the table in the root manifest of a user's project:

```toml
[profile.release.build-override]
opt-level = 3
codegen-units = 16
```

The crate documentation of `slang-src` and `README.md` state both remedies. With resolver 3, Cargo resolves the features of host units apart from those of target units, so the feature `vendored` of `slango` does not reach the copy of Slang that `slango-macros` links. A program that needs no Vulkan SDK at build time enables `vendored` on both, as `docs/design/slango-macros.md` states.

## 9. The Cache Directory

### 9.1. Selection

SRC-15. The build script MUST read `SLANG_SRC_CACHE_DIR` and MUST write nothing outside `OUT_DIR` when it is absent. When it is present, the build script MUST stop the build when the value is empty, is not valid UTF-8, contains a line feed or a carriage return, or is not an absolute path, and MUST otherwise create the directory when it does not exist and keep its clone entries there. It MUST export the key `cache_dir` only when the tree is a clone, never for a checkout of `SLANG_SOURCE_DIR`.

Writing outside `OUT_DIR` is the departure from SYS-5 that the owner decided for the cache, and it happens only when the user asks for it. A checkout of `SLANG_SOURCE_DIR` can hold local changes that its commit does not record, so neither it nor a library built from it may enter an entry that a key based on the commit names. The dependent build scripts read the cache directory only from the key `cache_dir`, so they use the cache exactly when `slang-src` permits it.

### 9.2. Layout and Entries

The cache directory has this layout. Each dependent build script owns the directory named after its crate:

| Path | Content | Written by |
| --- | --- | --- |
| `src/<commit>/` | The clone entry of one commit, with `tree` and `build-tag` (Section 6.3) | `slang-src` |
| `src/<commit>.lock` | The lock of that clone entry | `slang-src` |
| `slang-compiler-sys/<key>/` | The libraries and headers of one build of Slang | `slang-compiler-sys` |
| `slang-compiler-sys/<key>.lock` | The lock of that entry | `slang-compiler-sys` |
| `slang-glslang-sys/<key>/` | The libraries of one build of slang-glslang | `slang-glslang-sys` |
| `slang-glslang-sys/<key>.lock` | The lock of that entry | `slang-glslang-sys` |

`<commit>` is the full commit hash, and `<key>` is the cache key of Section 9.4.

SRC-16. Every build script that writes a cache entry, which is the build script of `slang-src` for a clone entry and each dependent build script for an entry of libraries, MUST follow these steps:

1. Open the lock file of the entry, creating it when it is absent, and take an exclusive lock on it with `std::fs::File::lock`, which blocks while another process holds it.
2. When the entry exists, use it. For an entry of libraries, its file `inputs` has to equal the inputs of the current build byte for byte (Section 9.4), and every file that the build links has to exist in it.
3. Otherwise, remove every directory named `<entry>.tmp-<number>`, build into `<entry>.tmp-<process id>`, write the file `inputs` last for an entry of libraries, remove an entry that failed step 2, and rename the temporary directory to the entry.
4. Keep the lock until the build script exits, when the operating system releases it, also after a crash.

Two build scripts that need the same entry at the same time then build it once, and the second waits and uses it. An entry is renamed into place only when complete, so a crash leaves a temporary directory, never a partial entry, and a temporary directory can be removed safely while the lock is held.

A build script that uses an entry emits `cargo::rerun-if-changed` for a file of the entry, `build-tag` for a clone entry and `inputs` for an entry of libraries, so that Cargo runs it again when the user removed the entry (Section 15, SRCA-5).

### 9.3. Removal

No build script removes a complete entry, except an entry of libraries that failed step 2 of SRC-16, and nothing evicts old entries. The user removes the cache directory, or any entry of it, by hand. Removing it is safe whenever no build runs, because rustc copies the static libraries into the rlib of the crate that links them, and a build script whose entry is gone builds it again (Section 15, SRCA-5).

The crate documentation and `README.md` say that the directory grows with each commit, target, and toolchain that builds with it, and that removing it is the way to free its space.

### 9.4. Keys of Built Libraries

SRC-17. A dependent build script MUST name each entry of libraries by a cache key that is the 16 lowercase hexadecimal digits of the 64-bit FNV-1a hash of its inputs, and MUST write those inputs into the file `inputs` of the entry. The inputs are a text of `NAME=value` lines, in this order:

1. `crate`: the name and version of the crate that builds.
2. `files`: the FNV-1a hash of the content of every file of the crate that the native build reads, such as the wrapper project of `slang-glslang-sys`, in a fixed order.
3. `commit` and `build_tag`: the values of `slang-src`.
4. `target`: the value of `TARGET`.
5. `cmake`: the first line of the output of `cmake --version`.
6. `configure`: every argument of the configure step, in order, one line each.
7. `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `CMAKE_TOOLCHAIN_FILE`, `MACOSX_DEPLOYMENT_TARGET`, and `VCToolsVersion`: the value of each variable, or an empty value when it is absent.
8. `compiler`: the first line of the output of `<compiler> --version`, in which `<compiler>` is the value of `CXX`, or `c++` when it is absent, on every target except Windows with the Microsoft toolchain, where it is empty.

Every input that changes the built libraries changes the key, so an entry is never used for another build. The file `inputs` makes a collision of two keys harmless, because step 2 of SRC-16 compares it in full.

A hash without cryptographic strength is enough, because the comparison of the inputs and not the hash decides a hit, and an attacker who can write into the cache directory can write any entry directly (Section 13). The hash needs no dependency.

The Cargo profile is not an input, because the native builds always use the CMake build type `Release`. The paths of `OUT_DIR` and of the source tree are not inputs, so that every target directory and project shares an entry. Section 15, SRCA-6, records that `VCToolsVersion` identifies the Microsoft compiler well enough.

## 10. Rerun Instructions

SRC-18. The build script MUST emit `cargo::rerun-if-env-changed` for `SLANG_SOURCE_DIR`, `SLANG_SRC_COMMIT`, `SLANG_SRC_CACHE_DIR`, and `CARGO_NET_OFFLINE` on every run, before it reads them. It MUST emit `cargo::rerun-if-changed` for the `HEAD` and `index` files of the git directory of the source tree, as `git rev-parse --git-path HEAD --git-path index` prints them, as soon as it knows them.

A build that does not rerun after its input changes exports a tree that no longer matches its metadata (SYS-1). The `HEAD` and `index` files change with every checkout and commit, so they stand for the state of a local checkout without a watch on each of its files, as SYS-1 permits for a tree whose commit the script reads on every run. They also make Cargo rerun the build script when a clone entry was removed.

An edit of a file of a local checkout without a git operation afterwards does not rerun the build script, and the dependent builds keep the library built before the edit. The crate documentation states this. Section 15, SRCA-1, records that a dependent build script reruns whenever the metadata of `slang-src` changes.

## 11. Failure Messages

SRC-19. On each failure of this table, the build script MUST end by returning an error from its `main` (SYS-4) whose message contains every item of the second column. A build that stops without naming what is missing and how to supply it leaves the user to guess.

In this table, the offline advice is the sentence that says three things: to build without network access, set `SLANG_SOURCE_DIR` to the top level of a git checkout of Slang with its submodules, as the clone command of the message shows, and a build without the Cargo feature `vendored` links the Slang of an installed Vulkan SDK instead.

| Failure | The message contains |
| --- | --- |
| A variable of Section 14.2 is present but empty | The name of the variable with "is set but empty", and what the variable selects |
| The value of `SLANG_SOURCE_DIR` or `SLANG_SRC_CACHE_DIR` is not valid UTF-8, contains a line feed or a carriage return, or is not an absolute path | The name of the variable, its value in quotation marks with every control character escaped and every byte that is not UTF-8 replaced, and that the value is not an absolute path in UTF-8 without line breaks |
| `SLANG_SOURCE_DIR` is not an accessible directory, or not the top level of a git work tree | The path, the reason, that the variable names the top level of a git checkout of Slang, and that a tree without git metadata, such as a source archive, has no commit to record |
| `SLANG_SRC_COMMIT` is not 40 lowercase hexadecimal digits | The value in quotation marks, escaped as above, and that the variable takes a full commit hash |
| `git` cannot start | The command, the reason that the operating system gives, and that a vendored build needs git on `PATH` |
| `CARGO_NET_OFFLINE` is `true` and the tree is not a local checkout or an existing clone entry | That the build needs to clone Slang, the commit, and the offline advice |
| A git command that reaches the network fails | The command, its exit status, its error output, the commit, and the offline advice |
| Another git command fails | The command, the directory, its exit status, and its error output |
| A required submodule is missing (SRC-4) | The path of the submodule, the path of the tree, and the command that fetches it |
| `git describe` fails or prints a value that SRC-12 rejects | The command, the directory, the output or the error output, that the build needs a tag of the form `v<major>.<minor>` in the history of the commit, and the command `git -C <tree> fetch --tags` for a local checkout |
| A clone entry in the cache directory reports another commit | The path of the entry, both commits, and that removing the entry when no build runs fixes it |
| A file or directory under `OUT_DIR` or the cache directory cannot be created, locked, renamed, removed, or written | The action, the path, and the reason that the operating system gives |

The build script emits no `cargo::warning`. Cargo shows the warnings of a registry dependency's build script only when the build fails, so a warning would not reach the user who needs it, and an error does.

## 12. Crate Documentation

SRC-20. The crate-level documentation of `slang-src` and `README.md` MUST state what a vendored build needs, which is git, CMake 3.22 or later, Ninja, a C++ compiler, Python 3, and network access or a local checkout, and every environment variable of Section 14.2 with its default. They MUST also state the cache directory, its growth, that removing it is safe when no build runs, and the two remedies of Section 8. The owner decided that the cache directory is documented in both places, and a user who does not know the variables cannot build without network access or avoid the double build.

The crate-level documentation also states:

- That the crate builds nothing and is a dependency of `slang-compiler-sys` and `slang-glslang-sys` under their vendored builds.
- The recorded commit, and that the version of the crate names it (Section 7.2).
- The order of the sources (SRC-3), that a selected commit is best effort, and that a local checkout is used as it is.
- What the crate fetches and from where, the git settings of SRC-8, and that it runs nothing from the tree (Section 13).
- That an edit of a local checkout without a git operation does not trigger a rebuild (Section 10).
- `# Features`: the crate has no Cargo features.
- `# Platform support`: Linux, Windows, and macOS, the platforms of the vendored CI job.

## 13. Security Considerations

The threat model has five parts:

1. An attacker on the network between the build and GitHub, or in control of a DNS resolver or proxy, tampers with what git receives.
2. An attacker controls the Slang repository on GitHub or the repository of a submodule, and changes what a branch or a tag points to.
3. An attacker with write access to the cache directory places or changes an entry.
4. The environment of the build, such as a git hook or a parent build, sets git variables or Cargo variables that the build script reads.
5. The user selects a commit or a local checkout whose content is malicious.

Untrusted shader source and file paths. The crate reads no shader source and gives no path to Slang. It reads only the variables of Section 14.2 and git's answers.

Unsafe code and foreign function interface (FFI) invariants. The crate has no unsafe code and declares no foreign function (SRC-2).

Build scripts that run native builds. The build script runs git and nothing else. It runs no CMake, no compiler, no script of the tree, and no hook, because `git init` creates an empty hooks directory and a fetch never brings hooks. It fetches only from `https://github.com/shader-slang/slang.git` and from the HTTPS URLs that `.gitmodules` of the fetched commit names for the required submodules, and SRC-8 forbids every other protocol and every prompt for credentials.

It fetches by full commit hash, and git verifies the object ID of every object it receives (Section 3.2). The commit hash therefore identifies the content of the superproject, and its gitlinks identify the content of each submodule, so an attacker on the network or in control of a branch or tag cannot substitute other content for the recorded commit or for a commit that the user selected by hash. HTTPS adds confidentiality and protects the build tag that `git describe` derives for a selected commit, which tags determine.

The build script writes only into `OUT_DIR`, and into the cache directory when the user names one (SRC-15). It writes nothing into a local checkout (SRC-6).

Every path that it writes into a Cargo instruction is checked for line breaks first (SRC-5, SRC-15), so a value cannot inject an instruction. It removes the repository variables from the environment of git (SRC-8), so git answers for the tree it was given.

Dependencies. The crate has no dependencies (SRC-2). Every crate that depends on it requires its exact version (SRC-14), which fixes the recorded commit for a given version of each dependent.

These attacks are out of scope:

- A malicious commit that the user selects with `SLANG_SRC_COMMIT`, or a checkout that the user names with `SLANG_SOURCE_DIR`, is code that the user chose to build. The dependent builds run its CMake scripts and compile it with the user's permissions.
- An attacker with write access to the cache directory can place any library in an entry, and the dependent builds link it. The cache directory belongs to the user, who chooses where it is and who can write there, as the crate documentation says.
- The user's git configuration, such as a proxy or a rule that rewrites URLs, applies to every git command, because the user controls it and needs it on some networks.
- Attacks on the repository of this workspace are a matter of access control.

The residual risk has four parts:

1. git's object IDs are SHA-1 hashes. git's variant detects the known collision attacks, but an attacker who could produce a new SHA-1 collision could substitute content for a commit.
2. The build tag of a selected commit comes from the tags of the remote at the time of the build, so a tag added or moved later can change the build tag, though not the content.
3. A clone entry in the cache directory is trusted once it exists, and the build script does not verify its content again.
4. A network that stalls a connection without transferring anything can hold a build until the operating system gives up the connection, which can take minutes (Section 15, SRCA-2).

## 14. Operational Considerations

### 14.1. Cargo Features

`slang-src` has no Cargo features. It is a dependency of the crates that build Slang only under their feature `vendored`, so a default build never compiles it.

### 14.2. Environment Variables

This table lists every environment variable that the build script reads or sets for git, with its default.

| Variable | Read by | Default and behaviour when absent |
| --- | --- | --- |
| `SLANG_SOURCE_DIR` | The build script, always | Absent, so the build script clones (Section 6). When present, the local checkout it names is the source tree, and an empty value, a value that SRC-5 rejects, or a directory that is not the top level of a git work tree stops the build. |
| `SLANG_SRC_COMMIT` | The build script, when `SLANG_SOURCE_DIR` is absent | Absent, so the build script clones the recorded commit. When present, it clones that commit, and a value that is not a full commit hash stops the build (SRC-10). |
| `SLANG_SRC_CACHE_DIR` | The build script, always | Absent, so nothing is written outside `OUT_DIR` and no dependent uses a cache. When present, it names the cache directory (SRC-15). |
| `CARGO_NET_OFFLINE` | The build script, before it clones | Absent or any value other than `true`, so the build script may clone. When `true`, it does not reach the network (SRC-9). |
| `OUT_DIR` | The build script | Set by Cargo. A value that is not an absolute path in UTF-8 without line breaks stops the build. |
| `PATH` | The operating system, when the build script starts git | git has to be on it, or the build stops (Section 11). |
| `GIT_TERMINAL_PROMPT` | Set to `0` by the build script for every git command that reaches the network | The value of the environment is never used, so git never asks for credentials (SRC-8). |
| `GIT_OPTIONAL_LOCKS` | Set to `0` by the build script for every git command | The value of the environment is never used (SRC-8). |
| `GIT_DIR`, `GIT_INDEX_FILE`, and the other repository variables | Removed by the build script from the environment of every git command | A value in the environment of the build never reaches git (SRC-8). |

### 14.3. Platforms and Minimum Supported Rust Version

The crate builds on every platform that Rust supports, and its build script runs where git runs. The vendored builds that use it are supported on x86_64 Linux, x86_64 Windows with the Microsoft toolchain, and aarch64 and x86_64 macOS (Section 6 of the sys design). The crate uses the workspace's `rust-version`, and `std::fs::File::lock` is stable at that version.

### 14.4. Failure Modes

This table lists the failure modes of the crate, how each is reported, and the recovery. Every error is the error that the build script returns, with the content of Section 11.

| Failure | Recovery |
| --- | --- |
| No network access, a blocked host, or `CARGO_NET_OFFLINE` set to `true` | Set `SLANG_SOURCE_DIR` to a local checkout, set `SLANG_SRC_CACHE_DIR` to a cache that holds the clone, or build without `vendored` |
| A selected commit does not exist on GitHub | Correct `SLANG_SRC_COMMIT`, or unset it to use the recorded commit |
| A selected commit or a local checkout has no reachable tag | Fetch the tags of the checkout, or select another commit |
| A required submodule is missing from a local checkout | Run the command that the message names, which needs network access |
| git is missing | Install git |
| A clone entry of the cache directory is at another commit | Remove the entry when no build runs |
| The cache directory or `OUT_DIR` cannot be written | Free disk space or fix the permissions |
| A selected commit or a local checkout builds a Slang that the binding generator or the safe crates do not support | Use the recorded commit, which the crates are tested against |

### 14.5. Logs

The build script emits no warnings and writes no log. Its only output is the Cargo instructions and, on a failure, its error. The crate emits nothing at run time.

## 15. Assumptions to Confirm

Each row is a fact that this design relies on and that is not verified. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SRCA-1 | Cargo runs a dependent build script again whenever the build script of `slang-src` runs again and its metadata changes | Section 10 | A test of the vendored CI job that changes `SLANG_SRC_COMMIT` between two builds in one target directory |
| SRCA-2 | The low-speed settings of SRC-8 stop a transfer that stalls after it connected, and the operating system stops a connection attempt to an unreachable address after about two minutes | SRC-8, Section 13 | A run of the clone against an unroutable proxy, on Linux, Windows, and macOS |
| SRCA-3 | The ten required submodules of Section 3.3 are all that the native builds read on Windows and macOS, with mimalloc turned off | Section 3.3 | The vendored CI jobs on Windows and macOS |
| SRCA-4 | The counts of Section 8, which come from stub crates in the shape of the workspace, hold for the real crates | Section 8 | A vendored build of the example that calls `slango-macros`, with each command of the table |
| SRCA-5 | Cargo runs a build script again when a file named by its `cargo::rerun-if-changed` no longer exists | Sections 9.2 and 9.3 | A test that removes a cache entry between two builds |
| SRCA-6 | `VCToolsVersion` changes whenever the Microsoft compiler that CMake finds changes | SRC-17 | The vendored CI job on Windows, by a review of the developer environment it runs in |

The next assumption takes SRCA-7.

## 16. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SRCQ-1 | Does the clone fit within the path length limit of Windows when `OUT_DIR` is deep, or does the build script need a shorter directory, such as the cache directory, on Windows? | This document, with `docs/design/slang-compiler-sys.md` | M7 |
| SRCQ-2 | Should a build tag file or the cache directory record when an entry was last used, so that a later tool can evict entries? | This document | Not scheduled |

The next question takes SRCQ-3.

## 17. References

### 17.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 17.2. Informative References

- [SEMVER] Preston-Werner, T., "Semantic Versioning 2.0.0", <https://semver.org/spec/v2.0.0.html>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| SRC-1 | 4 | `links = "slang-src"`, the metadata keys `root`, `commit`, `build_tag`, and `cache_dir`, and direct dependents only. |
| SRC-2 | 4 | No unsafe code, no dependencies, and the logic behind a host trait in an included file. |
| SRC-3 | 5 | The source is `SLANG_SOURCE_DIR`, then `SLANG_SRC_COMMIT`, then the recorded commit, and an empty variable stops the build. |
| SRC-4 | 5 | Every source has the ten required submodules, or the build stops with the command that fetches the missing one. |
| SRC-5 | 6.1 | `SLANG_SOURCE_DIR` is an absolute UTF-8 path without line breaks to the top level of a git work tree. |
| SRC-6 | 6.1 | A local checkout is used as it is and never written. |
| SRC-7 | 6.3 | The clone is an init, a depth-1 fetch of the commit, a checkout, and a depth-1 update of the required submodules, renamed into place when complete. |
| SRC-8 | 6.3 | git runs with HTTPS only, no prompt, a low-speed limit, no optional locks, and no repository variables. |
| SRC-9 | 6.3 | No network access with `CARGO_NET_OFFLINE=true`, and a failed clone stops the build with the offline advice and never falls back. |
| SRC-10 | 6.4 | `SLANG_SRC_COMMIT` is a full hash, and its build tag comes from a treeless fetch of tags and `git describe --abbrev=9`. |
| SRC-11 | 7.1 | The recorded commit and build tag live only in `recorded.rs` and the manifest version, a test compares them, and scripts read them through `recorded-slang`. |
| SRC-12 | 7.2 | A build tag has the form `<major>.<minor>[.<patch>][-<count>-g<hash>]`, or the build stops. |
| SRC-13 | 7.2 | The version is `<crate version>-slang.<major>.<minor>.<patch>.<count>.g<hash>`, with a minor bump for a new commit and a patch bump for a crate fix. |
| SRC-14 | 7.2 | Every dependent requires the exact version with `=`. |
| SRC-15 | 9.1 | Nothing outside `OUT_DIR` without `SLANG_SRC_CACHE_DIR`, and no cache for a local checkout. |
| SRC-16 | 9.2 | Every cache entry is built under an exclusive lock in a temporary directory and renamed into place when complete. |
| SRC-17 | 9.4 | An entry of libraries is named by the FNV-1a hash of its listed inputs, which the entry holds and a hit compares in full. |
| SRC-18 | 10 | Rerun instructions for the four variables and the `HEAD` and `index` files of the tree. |
| SRC-19 | 11 | Each failure returns an error with the content of the table. |
| SRC-20 | 12 | The crate documentation and `README.md` state the requirements, the variables, the cache directory, and the remedies of the double build. |
