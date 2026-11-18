# `slang-src` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-18 |
| Applies to | `crates/slang-src` |

## Abstract

This document specifies `slang-src`, the crate that obtains the Slang source tree for the crates that build Slang from source and exports the location of that tree to their build scripts, and it builds nothing itself. It defines the one source of the tree, which is a shallow clone of a recorded Slang commit, the version scheme that carries that commit in the crate's own version, the metadata that the crate exports, and the shared folder in the Cargo target directory that keeps the clone and the built libraries for every build of a project. It also defines how a user builds another Slang commit through Cargo's `[patch]` table and the failure modes. The crate reads no environment variable of its own. It is the one crate of the workspace that reaches the network, so it also states what it fetches, from where, and why a commit hash identifies what it fetched.

## 1. Introduction

### 1.1. Problem

A vendored build of `slang-compiler-sys` builds Slang from source and links it statically, and `slang-glslang-sys` builds glslang and SPIRV-Tools from the same source tree (Section 4 of `docs/design/architecture.md`). Both need the same tree at the same commit, and so does `slang-rhi-sys` later. A build script that clones the tree in each of these crates would clone it several times, could clone different commits, and would put network access into every crate that builds native code.

The tree is large, and a full clone with every submodule is much larger than what the builds read. Cargo also gives each profile and each host or target unit its own `OUT_DIR`, so a clone and a build kept only in `OUT_DIR` would repeat for every profile of a project and for a procedural macro that links Slang.

### 1.2. Purpose

`slang-src` is the one place that obtains the tree. It holds the recorded Slang commit, clones it, and exports the path, the commit, and the build tag of the tree to the build scripts of the crates that depend on it. It also finds the shared folder in the Cargo target directory and exports it, so that every crate that builds Slang keeps its built libraries there.

This document implements the parts of Sections 4, 5, and 15.10 of the architecture design that concern the source tree.

### 1.3. Audience

The primary readers are the coding agents that change `slang-src`, the build scripts that depend on it, or the recorded commit. The secondary readers are the reviewers of those changes and the owner of the repository, who approves the decisions. A reader is expected to know Cargo build scripts and git, but not the layout of the Slang tree.

### 1.4. Scope

This document covers:

- The source of the tree, the clone, its submodules, and its git settings.
- The recorded commit, its build tag, the version scheme of the crate, and another commit through `[patch]`.
- The metadata that the build script exports.
- The shared folder, how the build script finds it, its layout, its keys, and its locking, which every crate that builds Slang from the tree follows.
- How a procedural macro that links Slang shares the folder with the program.
- The tests, the crate documentation, the security considerations, and the operational considerations.

### 1.5. Non-Goals

This document does not cover:

- The native builds of Slang and of slang-glslang, which `docs/design/slang-compiler-sys.md` and `docs/design/slang-glslang-sys.md` specify. `slang-src` builds nothing and runs no code of the tree.
- The dynamic route, which never depends on `slang-src`.
- The recipes and the CI jobs, which `docs/design/tooling.md` specifies. This document states the requirements that the crate puts on them.
- An offline mirror, a proxy, or a download of anything other than the Slang repository and its submodules.
- An eviction of old entries of the shared folder, which `cargo clean` removes with the rest of the target directory (Section 9.3).

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

- base directory: The directory that holds the shared folder, which is the Cargo target directory or the build directory of the build, or `OUT_DIR` when the build script finds neither (Section 9.1).
- build directory: The directory in which Cargo keeps intermediate build artifacts such as `OUT_DIR`, which is the target directory unless the Cargo setting `build-dir` names another one.
- build metadata: The part of a Semantic Versioning (SemVer) 2.0.0 version [SEMVER] after a plus sign, which Cargo ignores when it compares versions.
- build unit: One compilation that Cargo plans, such as the build script run of a package for the host or for the target in one profile.
- clone entry: The entry of the shared folder that holds one clone, its build tag file, and its stamp file (Section 6.1).
- dependent build script: The build script of a crate that depends directly on `slang-src` and reads its metadata, such as the build scripts of `slang-compiler-sys` and `slang-glslang-sys`.
- entry: One directory of the shared folder that holds one clone or the libraries of one native build, complete or absent, never partial (Section 9.2).
- entry key: The name of an entry of built libraries, derived from every input that changes those libraries (Section 9.4).
- object ID: The name of a git object, which is the SHA-1 hash of its content. A commit hash is the object ID of a commit.
- recorded commit: The Slang commit that `slang-src` clones, which is the commit of the Slang in the Vulkan SDK that CI installs (Section 7).
- recorded Slang: The Slang build at the recorded commit, named by its build tag.
- required submodule: One of the submodules of the Slang tree that the dependent build scripts read (Section 3.4).
- shared folder: The directory `slang-src` in the base directory, in which `slang-src` keeps its clone entries and each dependent build script keeps its entries of built libraries (Section 9).
- source tree: The Slang work tree that `slang-src` exports, with its required submodules checked out.
- stamp file: The file `stamp` of an entry, written last and with an old modification time, which the build scripts that use the entry name in `cargo::rerun-if-changed` (SRC-16).

## 3. Facts

This section is non-normative. It states the facts on which the later sections rely.

### 3.1. Cargo

A build script exports `links` metadata with `cargo::metadata=KEY=VALUE`. Cargo passes it to the build script of each package that depends directly on the package, as the environment variable `DEP_<LINKS>_<KEY>`, with the `links` name in upper case and each hyphen replaced by an underscore. A package that depends on it only indirectly does not see it.

The `links` key also stops two versions of the package from entering one build graph.

A build script cannot see the command-line option `--offline`. Cargo passes the environment variable `CARGO_NET_OFFLINE` to a build script only when the user sets it in the environment.

Each target directory, profile, and host or target unit has its own `OUT_DIR` for a build script, so a build script that writes only into `OUT_DIR` repeats its work in each of them. Cargo gives a build script no variable that names the target directory or the build directory.

A build script sets an environment variable for the compilation of its own package with `cargo::rustc-env=NAME=VALUE`, which the code of every target of the package, including its tests, reads with `env!`.

A pre-release identifier made only of digits compares numerically, and any other identifier compares as ASCII text and ranks above every numeric identifier. A numeric identifier with a leading zero is not valid. A version with a pre-release orders below the same version without one.

A caret requirement on a pre-release version, such as `"0.1.0-alpha.1.slang.2026.17.0.25.gc7954ebd1"`, matches every later pre-release of `0.1.0`, so `cargo update` moves it to a version that carries another Slang. Only the exact requirement `"=0.1.0-alpha.1.slang.2026.17.0.25.gc7954ebd1"` holds one version. A requirement such as `"0.1"` matches no pre-release at all.

Cargo's `[patch]` table replaces a package of a registry with another source, such as a git repository, in the whole build graph of a project. Cargo uses a patch only when its version satisfies the requirements of the packages that depend on it (Section 16, SRCA-8).

### 3.2. The Target Directory

These facts hold for Cargo 1.99.1 on x86_64 Linux. On Windows and macOS, the vendored CI jobs confirm the first fact for a target directory that Cargo creates without `--target`. In such a directory, the build script finds the shared folder through the tag, takes the lock, writes the stamp file, and clones once. Section 16, SRCA-5 and SRCA-9, records the rest as assumptions on Windows and macOS.

- Cargo writes a file named `CACHEDIR.TAG` into a target directory or build directory when it creates that directory. A build with `--target` also gets a tagged directory named after the target triple below it, which holds the units of the target, while the units of the host stay in the profile directories at the top.
- Cargo writes no tag into a directory that existed before the first build, such as one created by `mkdir`, and it does not write the tag again after someone deleted it.
- Cargo tags only the directory that it creates as the target directory. When `CARGO_TARGET_DIR` names a directory below one that does not exist yet, such as `target/vendored`, Cargo also creates the parent `target/` and leaves it without a tag, so a later build in `target/` finds no tag there.
- `cargo publish --dry-run`, run first in a target directory that does not exist yet, creates it without a tag. `cargo check`, `cargo build`, and `cargo package --list` create it with the tag. `cargo metadata`, `cargo clean`, `cargo fetch`, and `cargo tree` do not create it.
- `cargo install` from crates.io or from git builds in a temporary target directory without a tag. `cargo install --path`, `cargo package`, and `cargo publish` build in the target directory of the workspace.
- With a separate build directory, set by `build-dir` in the Cargo configuration or by `CARGO_BUILD_BUILD_DIR`, `OUT_DIR` is inside the build directory, which has its own tag.
- A newer directory layout of Cargo places `OUT_DIR` one level deeper below the profile directory. A rule that counts levels up from `OUT_DIR` breaks on such a change, while the nearest tagged ancestor stays the same directory, which is why SRC-23 searches for the tag and counts no levels.
- Cargo does not serialize two builds with different profiles in one target directory, so their build scripts run at the same time.
- `cargo clean` without options removes the whole target directory. `cargo clean -p <package>` and `cargo clean --release` remove only files of their packages or profile.
- Cargo runs a build script again when a file that its `cargo::rerun-if-changed` names is missing, or has a modification time later than the start of the last run of the build script.
- `cargo miri` builds in the directory `miri` inside the target directory, which carries its own tag.

### 3.3. git

`git clone --depth 1` cannot fetch a commit by its hash. `git init` followed by `git fetch --depth 1 origin <commit>` can, and GitHub serves any commit that a branch or tag of the repository reaches. `git submodule update --init --depth 1 -- <paths>` fetches the commit that each gitlink records, also when the default branch of the submodule does not contain it.

`git submodule update` stops with a pathspec error when one of its paths has no gitlink in the commit, before it reports anything about the other paths. `git ls-tree HEAD -- <paths>` lists each path of the commit with its mode and type, which are `160000` and `commit` for a gitlink, and reaches no network.

`git ls-files -z --deleted` lists each tracked file that is missing from the work tree, which includes a file that the checkout never created. It reads no file content, so line endings do not change its answer, and it reaches no network. In a superproject, it lists no file of a submodule, so each submodule needs its own run.

A shallow clone holds no tags, so `git describe` fails in it. The build tag of the clone therefore comes from `recorded.rs` (Section 7.1).

git verifies the object ID of every object it receives, so a fetched commit, its trees, and its files are the content that the hash names. Every gitlink records the commit of its submodule by object ID, so the commit of the superproject determines the content of every submodule. git uses a variant of SHA-1 that detects the known collision attacks.

The git settings `http.lowSpeedLimit` and `http.lowSpeedTime` stop a transfer that stays below a rate for a time. git has no setting that bounds the time to connect, and the operating system then bounds it (Section 16, SRCA-2).

The environment variable `GIT_ALLOW_PROTOCOL`, when set, overrides the settings `protocol.allow` and `protocol.<name>.allow`, including those given with `-c`.

`git rev-parse --local-env-vars` lists the environment variables that name or change the repository that git works on, such as `GIT_DIR`. git 2.47.3 lists 15 of them.

`git init` copies the git template directory into the new repository. git's default template holds only hooks whose names end in `.sample`, which git never runs. The setting `init.templateDir` of the user's git configuration can name another template, whose hooks git runs, such as a hook that runs after a checkout.

### 3.4. The Slang Tree

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
| `external/vulkan` | `slang-compiler-sys` and `slang-rhi-sys` |
| `external/glslang` | `slang-glslang-sys` |
| `external/spirv-tools` | `slang-glslang-sys` |
| `external/slang-rhi` | `slang-rhi-sys` |

`external/slang-rhi` joins the list for milestone M9, and `slang-rhi-sys` also reads `external/vulkan` for the Vulkan headers that its build takes from the tree (RSYS-4 of `docs/design/slang-rhi-sys.md`). The submodule holds no submodules of its own, and the builds of `slang-compiler-sys` and `slang-glslang-sys` do not read it, because the vendored route of `slang-compiler-sys` copies only its own list of submodules (Section 9.3 of the sys design). Its addition therefore changes neither build and only what a clone entry holds, which SRC-25 handles.

On x86_64 Windows, the builds read no further submodule. The build of slang-rhi with D3D12 reads `external/slang-rhi`, with the D3D12 Memory Allocator in its directory `external/d3d12ma`, and takes the D3D12 headers from the Windows SDK, and the build of Slang with DXIL support reads the header `external/dxc/dxcapi.h`, which lies in a directory of the Slang repository and not in a submodule (Section 7.3 of `docs/design/slang-rhi-sys.md`, SCS-82 of the sys design). The build of slang-rhi passes the redirection of the Vulkan headers to `external/vulkan` on Windows as well, where it does not use them. The CI job `vendored` built both from this list of submodules on the hosted Windows runner.

`external/mimalloc` is not required, because the vendored build of `slang-compiler-sys` passes `-DSLANG_ENABLE_SPIRV_TOOLS_MIMALLOC=OFF` with the Microsoft toolchain (Section 9.3 of the sys design) and the wrapper project of `slang-glslang-sys` sets both mimalloc options of SPIRV-Tools to `OFF` on every target (SGL-3 of `docs/design/slang-glslang-sys.md`). The vendored CI jobs on macOS and Windows show that the build of Slang of `slang-compiler-sys` needs no other submodule there. Section 16, SRCA-3, records that the build of slang-glslang needs no other submodule on Windows and macOS.

A shallow clone of the recorded commit with these submodules takes about half a minute on a fast connection and occupies under 300 MB.

### 3.5. Path Length on Windows

A Windows program that neither writes the prefix `\\?\` before a path nor opts into long paths is limited to `MAX_PATH`, 260 characters including the terminating NUL character, so to 259 characters for the path itself. The standard library of Rust adds the prefix to a long path itself. git creates a longer path only with its setting `core.longpaths`, and the compilers and CMake that read the tree in place have the same limit.

The checkout writes the tree into the temporary directory of SRC-16, whose name is that of the clone entry followed by `.tmp-` and the process ID, so it is 9 or 10 characters longer than the entry for a process ID of 4 or 5 digits. The longest tracked path of the recorded commit has 141 characters, the same with every required submodule as without them (`git ls-files --recurse-submodules`).

On the hosted Windows runner, the target directory of the vendored CI job is `D:\a\slang-rust\slang-rust\target/vendored`. With a clone entry of 40 characters, that longest path then has 244 characters in the entry and 254 in a temporary directory with a process ID of 5 digits, which leaves 5 characters below the limit.

Clone entries of 40 characters, which were then the full commit hash, have passed the vendored CI job on that runner. A clone entry of 57 characters, the full commit hash followed by a hyphen and the hash of SRC-25, did not: tracked files were absent from its work tree, and the build script of `slang-compiler-sys` failed when it copied one of them with "The system cannot find the file specified. (os error 2)".

## 4. Crate and Metadata

SRC-1. The manifest of `slang-src` MUST declare `links = "slang-src"`. Its build script MUST export, with `cargo::metadata`, the keys `root`, `commit`, `build_tag`, and `shared_dir` on every successful run. Every crate that reads them MUST depend on `slang-src` directly, because Cargo passes `links` metadata only to direct dependents (Section 3.1), and `links` keeps one version of the tree in each build graph.

A dependent build script reads them as `DEP_SLANG_SRC_ROOT`, `DEP_SLANG_SRC_COMMIT`, `DEP_SLANG_SRC_BUILD_TAG`, and `DEP_SLANG_SRC_SHARED_DIR`:

| Key | Value |
| --- | --- |
| `root` | The absolute path of the top level of the source tree, in UTF-8 without line breaks |
| `commit` | The full commit hash of the source tree, 40 lowercase hexadecimal digits |
| `build_tag` | The build tag of the source tree, in the form that SRC-12 defines, such as `2026.17-25-gc7954ebd1` |
| `shared_dir` | The absolute path of the shared folder (Section 9.1), in which each dependent build script keeps its own entries |

The build tag is the value that a dependent build script passes to Slang's CMake as `SLANG_VERSION_FULL`, and the part before its first hyphen is the numeric version (Section 9.3 of the sys design).

SRC-2. `slang-src` MUST have no unsafe code, no dependencies, and no build dependencies. Its build script MUST keep its logic in a file that the build script and a test both include, behind a trait for everything that it reads, runs, or writes (SYS-8). Every dependency of a crate that every vendored build compiles runs on every user's machine, and the standard library reads the environment, starts git, and locks and renames files without help.

The library of the crate holds the crate-level documentation and no public item. The crate exists for its build script and its metadata, and a public item would be an API that nothing needs and that every release would have to keep.

The licence of the crate is the one that ARCH-36 of the architecture design assigns to it.

The crate has these files:

- `build.rs` includes `recorded.rs`, `source.rs`, and `system.rs`, calls the logic with the real host, prints every instruction, and returns the result from `main`.
- `recorded.rs` holds the recorded commit, its build tag, and the name of its clone entry (Section 7.1, SRC-25).
- `source.rs` holds the logic, as plain functions behind the trait `Host`.
- `system.rs` holds the struct `System`, which implements `Host` with the standard library.
- `src/lib.rs` holds the crate-level documentation (Section 13).
- `tests/slang_src/` is the integration-test binary (Section 12).

## 5. The Source

SRC-21. The source tree MUST always be a clone of the recorded commit (Section 6.1). The build script MUST NOT read an environment variable of its own, and MUST NOT take the tree, the commit, or the shared folder from any input other than `recorded.rs` and the variables of Section 15.2. A user who needs another commit uses Cargo's `[patch]` table (Section 7.3).

One source keeps every build of one version of the crate on one Slang, and a crate without settings of its own has nothing that a user has to learn or that a build can set by accident.

SRC-4. The build script MUST check, directly after the checkout and before it fetches any submodule, that each required submodule of Section 3.4 has a gitlink in the commit, through `git ls-tree`. After the submodule update, it MUST check that each required submodule has a `.git` entry, a file or a directory, inside its directory. It MUST stop the build when either check fails, with the message of Section 11. A dependent build that reaches a missing submodule fails inside CMake with an error that does not say what is missing.

The gitlinks are checked before the update because `git submodule update` stops with a pathspec error at a path without a gitlink (Section 3.3), which does not say that the commit lacks a required submodule.

The list of required submodules is a constant of `source.rs`. A change of the native builds that reads another submodule changes this list and Section 3.4 in the same change. A missing gitlink happens only with a commit whose layout differs from the recorded one, such as the commit of a fork patched in through `[patch]`.

SRC-26. After the checks of SRC-4, the build script MUST run `git ls-files -z --deleted` in the work tree and in each required submodule, and MUST stop the build with the message of Section 11, before it writes the file `build-tag`, when any of these commands lists a path. The message MUST name the longest listed path, the first of them in byte order when several are equally long, with its path in the clone entry and the length of that path in characters. The build script MUST NOT set the git setting `core.longpaths`.

git leaves out a file that it cannot create, most often because its path is longer than the limit of Windows (Section 3.5). A dependent build that reaches such a file fails far from the cause, with an error that names only the file, while this check stops the clone with the length of the path and the advice to use a target directory with a shorter path. The check runs one git command for the tree and one for each required submodule, and none of them reaches the network (Section 3.3).

With `core.longpaths`, git would create the long paths, and the builds that read the tree in place through CMake and the compilers would fail later on the same paths (Section 3.5). A clone that stops at once with the cause costs the user less than a native build that fails after minutes.

## 6. Obtaining the Tree

### 6.1. The Clone

SRC-7. The build script MUST clone with these commands, in this order, in an empty temporary directory inside the shared folder, and MUST NOT run a git command that fetches anything else:

1. `git init` in the temporary directory, followed by `git remote add origin https://github.com/shader-slang/slang.git`.
2. `git fetch --depth 1 origin <commit>`, with the full recorded commit hash.
3. `git checkout --detach FETCH_HEAD`.
4. `git ls-tree -z HEAD -- <path>...`, with the paths of the required submodules of Section 3.4, for the check of the gitlinks of SRC-4.
5. `git submodule update --init --depth 1 -- <path>...`, with the same paths.

It MUST then check the `.git` entry of each required submodule (SRC-4) and the work tree (SRC-26), write the recorded build tag into the file `build-tag` and then the stamp file, and rename the temporary directory to the clone entry only after every step succeeded, with the entry protocol of SRC-16. A clone entry that exists is therefore complete, and an interrupted clone leaves only a temporary directory, which the next run removes before it clones again.

SRC-25. The clone entry MUST be `src/<prefix>-<submodules>` in the shared folder, where `<prefix>` is the first 23 hexadecimal digits of the recorded commit and `<submodules>` is the 16 lowercase hexadecimal digits of the 64-bit FNV-1a hash of the paths of the required submodules of Section 3.4, each followed by a NUL byte, in the order of the constant of `source.rs`. The name MUST NOT be longer than 40 characters. Its lock is `src/<prefix>-<submodules>.lock`, and its temporary directories are `src/<prefix>-<submodules>.tmp-<process id>`.

A clone entry is never changed after it is complete, and the reuse of an entry checks only its `tree` and its commit. Without the second component, a target directory that holds the clone entry of an earlier version of the crate, made without `external/slang-rhi`, would give a later version a tree without the submodule that it requires, and the build of `slang-rhi-sys` would then fail inside CMake with an error that names no cause. With it, a change of the list makes a new clone entry next to the old one, which costs one clone and keeps every rule of SRC-16.

The old entry stays in the shared folder until `cargo clean` or a removal by hand, which Section 9.3 permits when no build runs. The hash is that of SRC-17, so the crate needs no second hash function.

Every path of the tree, of its temporary directory during the checkout, and of the lock grows with the name. The limit of 40 characters is the length of the clone entries that have passed on the hosted Windows runner, where it leaves 5 characters below the limit of Windows (Section 3.5). A longer name leaves tracked files out of the work tree there, which SRC-26 reports.

The prefix of 92 bits only selects the directory and weakens no check of the commit. The clone fetches the full recorded commit (SRC-7), and the reuse of an entry compares the full commit that git reports with `RECORDED_COMMIT`. The hash keeps its 64 bits, so the name still changes with the list of required submodules.

`recorded.rs` MUST hold the name of the clone entry, `<prefix>-<submodules>`, as the constant `RECORDED_CLONE_ENTRY`. The build script MUST compute the name from `RECORDED_COMMIT` and the list of required submodules, and MUST stop the build with the message of Section 11, before it reads any variable or touches the file system, when the constant differs from the computed name.

The constant lets the recipes find the clone entry through `recorded-slang` without computing the hash themselves (SRC-11). The check makes a change of the list of required submodules fail the first build until the constant names the new entry, so the constant and the build cannot name two entries.

Entries named `src/<commit>` or `src/<commit>-<submodules>`, where `<commit>` is the full commit hash, are entries of earlier layouts of the crate, which the build script neither reads nor removes. The recipe `check-vendored-shared-folder` recognizes clone entries of all three layouts and fails while the shared folder holds any entry other than `RECORDED_CLONE_ENTRY`, so a developer removes such an entry when no build runs (Section 3.2 of `docs/design/tooling.md`).

The clone entry has this layout:

| Path in the clone entry | Content |
| --- | --- |
| `tree` | The work tree with its `.git` directory and the required submodules |
| `build-tag` | The build tag of the tree, as one line |
| `stamp` | The stamp file (SRC-16) |

When a clone entry exists, the build script checks that its `tree` exists and that it reports the recorded commit through `git rev-parse --verify HEAD^{commit}`, and then uses it without any network access. A clone entry without its `tree`, one that reports another commit, or one that git cannot read stops the build with the error of Section 11 that names the entry, because another build may be reading it.

The build script checks that `tree` exists before it starts git there. git cannot start in a directory that does not exist, and the error for a program that cannot start would advise installing git, which does not help.

SRC-8. The build script MUST run every git command that reaches the network with the settings `protocol.allow=never`, `protocol.https.allow=always`, `http.lowSpeedLimit=1000`, and `http.lowSpeedTime=60`, passed with `-c`, with `GIT_TERMINAL_PROMPT=0` in its environment, and with `GIT_ALLOW_PROTOCOL` removed from its environment. It MUST run every git command with `GIT_OPTIONAL_LOCKS=0` and with the repository variables removed from its environment.

The repository variables are a constant list of `source.rs`, which holds the variables that `git rev-parse --local-env-vars` lists. A test of the real host MUST fail when the installed git lists a variable that the constant lacks.

Only HTTPS can then carry the clone, so no URL in `.gitmodules` can make git start an SSH client or read a local repository. `GIT_ALLOW_PROTOCOL` would override the protocol settings (Section 3.3), so a value in the environment of the build could allow another protocol. git fails instead of asking for credentials, which it does when a repository or a commit does not exist, and a build would otherwise wait for input that never comes. A transfer that stalls below 1000 bytes per second for 60 seconds fails instead of hanging.

A git hook that runs Cargo sets repository variables such as `GIT_DIR`, and git would then answer for another repository. Without `GIT_OPTIONAL_LOCKS=0`, `git status` and other read commands refresh the index of a tree that another build may be reading.

A constant list keeps the git commands and the tests against a fake host independent of the installed git, while the test of the real host reports a git that adds a variable. Asking git for the list on every run would add a process to every build and a failure mode to the build script.

The build script runs every git command with `std::process::Command`, checks every exit status (SYS-4), and starts each with standard input closed. The user's git configuration still applies, such as a proxy or a rewrite of URLs, which Section 14 discusses.

### 6.2. Network Access

SRC-9. The build script MUST NOT reach the network when `CARGO_NET_OFFLINE` is `true`, and MUST then stop the build with the error of Section 11 for a build without network access when no clone entry of the recorded commit exists. When a clone fails, it MUST stop the build with that error, holding the command and the error output of git. It MUST NOT fall back to the dynamic route, because a user who enabled `vendored` and silently gets a dynamic link ships a program that needs the Slang shared library at run time.

The error says that a build without the Cargo feature `vendored` links the Slang of an installed Vulkan SDK and needs no network access. A vendored build reaches the network only for the first build in a target directory, and every later build uses the clone entry while the shared folder exists (Section 9).

## 7. The Recorded Commit and the Version

### 7.1. Recording

SRC-11. The recorded commit and its build tag MUST be recorded in `crates/slang-src/recorded.rs`, as the two constants `RECORDED_COMMIT` and `RECORDED_BUILD_TAG`, in the manifest version of `slang-src`, which encodes the build tag as Section 7.2 defines, and in the crate-level documentation, which states the commit, the build tag, and the version (SRC-20). Tests MUST fail when the constants disagree with the manifest version or when the crate documentation does not state all three. Recipes and scripts that need the recorded commit, the build tag, or the name of the clone entry MUST read them through the recipe `recorded-slang`, which prints `RECORDED_COMMIT`, `RECORDED_BUILD_TAG`, and `RECORDED_CLONE_ENTRY` of `recorded.rs` (SRC-25) as `NAME=value` lines.

Records that can diverge are only safe when a test compares them. Cargo reads the version only from the manifest, the build script needs the full commit, which the version abbreviates, and a reader of the crate documentation needs the commit as text. The recipe keeps every other consumer, such as the check of the Vulkan SDK in CI, from restating either value.

These are the values of the recorded Slang:

| Constant | Value |
| --- | --- |
| `RECORDED_COMMIT` | `c7954ebd1f1984ad2bfde15f76248db41dc34aa3` |
| `RECORDED_BUILD_TAG` | `2026.17-25-gc7954ebd1` |
| `RECORDED_CLONE_ENTRY` | `c7954ebd1f1984ad2bfde15-076690edef70bf9e` |

The recorded commit is the commit of the Slang in the Vulkan SDK version that CI installs, which the CI configuration records (Section 5.1 of the architecture design). The CI check of the installed SDK compares its build tag with `RECORDED_BUILD_TAG` and fails when they differ, so that every job tests one Slang (Section 7.3 of `docs/design/tooling.md`).

SRC-24. The build script MUST stop the build, before it reads any variable or touches the file system, when `RECORDED_COMMIT` is not 40 lowercase hexadecimal digits. The commit becomes a component of the paths of the clone entry and its lock and part of a Cargo instruction, so a value of a fork with a path separator, `..`, or a line break could reach outside the shared folder or add an instruction.

### 7.2. The Version Scheme

SRC-12. A build tag that `slang-src` exports MUST have the form `<major>.<minor>`, optionally followed by `.<patch>`, optionally followed by `-<count>-g<hash>`, in which each of `<major>`, `<minor>`, `<patch>`, and `<count>` is a decimal number without a leading zero, or `0`, and `<hash>` is at least nine lowercase hexadecimal digits. The build script MUST stop the build on any other value, because a dependent passes the part before the first hyphen to CMake as a dotted number (Section 3.10 of the sys design) and the manifest version below needs these parts.

SRC-13. The manifest version of `slang-src` MUST be the version of the release that it belongs to, with the Slang identifiers `slang.<major>.<minor>.<patch>.<count>.g<hash>` added as the last pre-release identifiers. The Slang identifiers are made from `RECORDED_BUILD_TAG` with a missing patch written as `0`, a missing count written as `0`, and the hash abbreviated to nine digits, taken from `RECORDED_COMMIT` when the build tag has none. The version of the release follows TOOL-37 of `docs/design/tooling.md`, so a new recorded commit and a change of only the crate both come with the next release of the whole set of crates.

The release `0.1.0-alpha.2` and the recorded build tag `2026.17-25-gc7954ebd1` give the version `0.1.0-alpha.2.slang.2026.17.0.25.gc7954ebd1`. A release version without a pre-release, such as `0.1.0`, would give `0.1.0-slang.2026.17.0.25.gc7954ebd1`. The test of the version requires the pre-release to end with the Slang identifiers, after nothing or after identifiers that end with a dot, so it needs no change for the next release (Section 12).

The release identifiers make the version a pre-release in the meaning of SemVer. The Slang identifiers are a deliberate abuse of the notation, because they mark no further stage of the release. In a release, they name the Slang that the crate clones. In a fork that is patched in, they do not (SRC-22).

Numeric identifiers make Slang versions order as numbers, so `2026.17.0.25` orders below `2026.22.0.3`. A build tag written with hyphens, such as `2026.17-25-g...`, would be one identifier compared as text, so `17-25` would order below `17-3`. The patch number is always written, so that a build tag of a patch release, such as `2026.17.1-3-g...`, maps to five numbers like any other.

The release identifiers come first, so versions of `slang-src` order by release before Slang. The order never selects a version, because every dependent requires an exact version (SRC-14), and the Slang identifiers tell a reader which Slang a release clones.

SRC-14. Every crate that depends on `slang-src` MUST require its exact version with `=`, because a caret requirement on a pre-release lets `cargo update` move a build to another Slang (Section 3.1). Every release of `slang-src` therefore needs a release of each crate that depends on it, which the release procedure has to cover (TQ-4 of `docs/design/tooling.md`). A fork that is patched in keeps the manifest version for the same reason (SRC-22).

This illustration shows the requirement in the manifest of a dependent:

```toml
[dependencies]
slang-src = { version = "=0.1.0-alpha.2.slang.2026.17.0.25.gc7954ebd1", path = "../slang-src" }
```

### 7.3. Another Slang Commit

SRC-22. The crates MUST offer no way to build another Slang commit other than Cargo's `[patch]` table. A user who needs another commit forks `slang-src`, changes `RECORDED_COMMIT` and `RECORDED_BUILD_TAG` in `recorded.rs`, keeps the manifest version, and names the fork in the `[patch]` table of the root manifest of the project. The crate documentation states these steps and that another commit is best effort.

The fork keeps the manifest version because every dependent requires that exact version (SRC-14), and Cargo uses a patch only when its version satisfies every requirement (Section 3.1, Section 16, SRCA-8). The version of the fork therefore does not name the Slang that it clones. The tests of SRC-11 fail in the fork, which marks it as a build for one project and never a release.

The commit and the build tag that the fork exports still name its Slang, so the build information of `slang-compiler-sys` reports the Slang that was built (Section 8.1 of the sys design).

Another commit is best effort. The crates are written and tested against the recorded Slang, the binding generator refuses headers that its rule set does not cover (SCS-2 of the sys design), and a commit whose submodules differ stops the build (SRC-4).

This illustration shows the table in the root manifest of a user's project, with a placeholder URL and branch:

```toml
[patch.crates-io]
slang-src = { git = "https://example.com/user/slang-src.git", branch = "other-slang" }
```

## 8. A Procedural Macro That Links Slang

This section is non-normative. `docs/design/slango-macros.md` states what a user of `slango-macros` enables.

`slango-macros` links Slang into the procedural macro that the compiler runs on the host. Cargo builds a dependency of a procedural macro as a host unit, with the profile settings of `build-override`, and merges a host unit with the target unit of the same package only when their settings and target agree. Section 16, SRCA-4, records that these counts come from stub crates in the shape of the workspace and are an assumption for the real crates.

A vendored build of a crate that calls `slango-macros` therefore runs the build scripts of `slang-src` and of the crates that build Slang once for each unit in these cases:

| Command | Runs each build script |
| --- | --- |
| `cargo build` and `cargo test` in the dev profile without `--target` | Once |
| `cargo build --release` | Twice, because the release profile optimizes while `build-override` does not by default |
| Any build with `--target` | Twice, because host units and target units then have separate directories |
| `cargo check` and `cargo clippy` | Twice for the crates that build Slang, for a reason in Cargo's unit graph that is not established |

Every run finds the same shared folder (Section 9.1), so the second run waits for the lock of the first and uses its entries. The clone and each native build then happen once for each target directory, whatever the number of units and profiles. Only where the lookup of SRC-23 finds no tag does each unit keep its own folder under its `OUT_DIR` and build again.

With resolver 3, Cargo resolves the features of host units apart from those of target units, so the feature `vendored` of `slango` does not reach the copy of Slang that `slango-macros` links. A program that needs no Vulkan SDK at build time enables `vendored` on both, as `docs/design/slango-macros.md` states.

## 9. The Shared Folder

### 9.1. Location

SRC-23. The build script MUST find the base directory with these steps, and the shared folder MUST be the directory `slang-src` in it:

1. Take the nearest directory `D` among `OUT_DIR` and its ancestors, starting with `OUT_DIR` itself, that holds a file named `CACHEDIR.TAG`.
2. When the last component of `D` equals the value of `TARGET` and the parent of `D` also holds a file named `CACHEDIR.TAG`, the base directory is the parent of `D`. Otherwise it is `D`.
3. When no such `D` exists, the base directory is `OUT_DIR`.

The build script MUST create the shared folder when it does not exist and MUST export its path as `shared_dir`. It MUST NOT find the folder by any other input, such as a count of levels above `OUT_DIR` or an environment variable.

Step 1 finds the target directory, or the build directory when Cargo uses a separate one, because Cargo tags the directory that it creates and `OUT_DIR` lies below it (Section 3.2). Step 2 keeps the target units of a build with `--target` in the same folder as its host units, because their nearest tag is the directory of the triple. A count of levels breaks when Cargo changes its layout, while the tag does not move.

An error while the build script looks for a tag counts as no tag, so the search moves on to the next ancestor and, at the end, to step 3. The lookup then loses only the sharing, as step 3 does.

Step 3 keeps the build correct when Cargo wrote no tag, and only the sharing is lost: each unit then keeps its clone and its libraries below its own `OUT_DIR`, as a build script without the folder would. These cases have no tag, and the crate documentation lists them as known limits:

- A target directory or build directory that existed before the first build, such as one created with `mkdir`, and by the same mechanism a directory restored from a cache of a CI service or mounted as a volume. This includes a directory that Cargo created only as the parent of another target directory, such as `target/` after a first build with `CARGO_TARGET_DIR=target/vendored`, and a target directory that `cargo publish --dry-run` created (Section 3.2). Section 9.6 of `docs/design/tooling.md` states what this costs the dry run of a release.
- A tag that someone deleted.
- The temporary target directory of `cargo install` from crates.io or from git.

A target directory created before the first build inside another tagged directory resolves to the outer directory, so its builds share the folder of the outer directory, which is still correct. A custom target given as a JSON file is not verified. When Cargo names its directory differently from `TARGET`, step 2 does not apply, and the target units use a folder of their own, which is correct and loses only the sharing.

The shared folder belongs to the target directory, so every build unit and profile of a project shares it, and two projects share it only when they share a target directory, such as through `CARGO_TARGET_DIR`.

### 9.2. Layout and Entries

The shared folder has this layout. Each dependent build script owns the directory named after its crate:

| Path | Content | Written by |
| --- | --- | --- |
| `src/<prefix>-<submodules>/` | The clone entry of one commit and one list of required submodules, with `tree`, `build-tag`, and `stamp` (Section 6.1) | `slang-src` |
| `src/<prefix>-<submodules>.lock` | The lock of that clone entry | `slang-src` |
| `slang-compiler-sys/<key>/` | The libraries and headers of one build of Slang | `slang-compiler-sys` |
| `slang-compiler-sys/<key>.lock` | The lock of that entry | `slang-compiler-sys` |
| `slang-glslang-sys/<key>/` | The libraries of one build of slang-glslang | `slang-glslang-sys` |
| `slang-glslang-sys/<key>.lock` | The lock of that entry | `slang-glslang-sys` |
| `slang-rhi-sys/<key>/` | The libraries and the configured header of one build of slang-rhi | `slang-rhi-sys` |
| `slang-rhi-sys/<key>.lock` | The lock of that entry | `slang-rhi-sys` |

`<prefix>` is the first 23 hexadecimal digits of the commit hash, `<submodules>` is the hash of SRC-25, and `<key>` is the entry key of Section 9.4.

SRC-16. Every build script that writes an entry, which is the build script of `slang-src` for a clone entry and each dependent build script for an entry of libraries, MUST follow these steps:

1. Open the lock file of the entry, creating it when it is absent, and take an exclusive lock on it with `std::fs::File::lock`, which blocks while another process holds it.
2. When the entry exists, use it. For an entry of libraries, its file `inputs` has to equal the inputs of the current build byte for byte (Section 9.4), and every file that the build links, and every header that the dependent build script reads from it, such as for the generation of bindings, has to exist in it.
3. Otherwise, remove every directory named `<entry>.tmp-<number>`, build into `<entry>.tmp-<process id>`, write the file `inputs` for an entry of libraries, write the stamp file last with its modification time set to 2000-01-01T00:00:00Z, remove an entry that failed step 2, and rename the temporary directory to the entry.
4. Keep the lock until the build script exits, when the operating system releases it, also after a crash.

Cargo does not serialize builds of different profiles (Section 3.2), so the lock is what makes two build scripts that need the same entry at the same time build it once, while the second waits and uses it. An entry is renamed into place only when complete, so a crash leaves a temporary directory, never a partial entry, and a temporary directory can be removed safely while the lock is held.

A build script that uses an entry MUST emit `cargo::rerun-if-changed` for its stamp file. Cargo then runs the build script again when the entry was removed (Section 3.2, Section 16, SRCA-5). The old modification time keeps an entry that another unit created after the start of this run from causing a needless run, because Cargo reruns a build script only for a file newer than its last run.

An entry is never changed after the rename. An entry that has to be built again is removed, never edited in place, because no build script reruns for a change inside it.

### 9.3. Removal

No build script removes a complete entry, except an entry of libraries that failed step 2 of SRC-16, and nothing evicts old entries. `cargo clean` without options removes the shared folder with the rest of the target directory, and `cargo clean -p` and `cargo clean --release` keep it (Section 3.2). Removing the folder, or any entry of it, is safe whenever no build runs, because rustc copies the static libraries into the rlib of the crate that links them, and a build script whose entry is gone builds it again (SRC-16).

The folder grows with each recorded commit and target that a project builds with.

### 9.4. Keys of Built Libraries

SRC-17. A dependent build script MUST name each entry of libraries by an entry key that is the 16 lowercase hexadecimal digits of the 64-bit FNV-1a hash of its inputs, and MUST write those inputs into the file `inputs` of the entry. The inputs are a text of `NAME=value` lines, in this order:

1. `crate`: the name and version of the crate that builds.
2. `files`: the FNV-1a hash of the content of every file of the crate that the native build reads, such as the wrapper project of `slang-glslang-sys`, in a fixed order.
3. `commit` and `build_tag`: the values of `slang-src`.
4. `target`: the value of `TARGET`.
5. `cmake`: the first line of the output of `cmake --version`.
6. `configure`: every argument of the configure step that is not a path under `OUT_DIR`, in order, one line each.
7. `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `CMAKE_TOOLCHAIN_FILE`, `MACOSX_DEPLOYMENT_TARGET`, and `VCToolsVersion`: the value of each variable, or an empty value when it is absent.
8. `compiler`: the identity of the C++ compiler. With the Microsoft toolchain, it is the full path of the `cl.exe` that the build script finds, and on every other target, the first line of the output of `<compiler> --version`, in which `<compiler>` is read from `CXX` as CMake reads it, or is `c++` when `CXX` is absent or blank. Section 9.3 of `docs/design/slang-compiler-sys.md` gives the exact rules.

Every input that changes the built libraries changes the key, so an entry is never used for another build. The file `inputs` makes a collision of two keys harmless, because step 2 of SRC-16 compares it in full.

A hash without cryptographic strength is enough, because the comparison of the inputs and not the hash decides a hit, and an attacker who can write into the target directory can write any entry directly (Section 14). The hash needs no dependency.

The Cargo profile is not an input, because the native builds always use the CMake build type `Release`, so every profile shares an entry. The paths of `OUT_DIR` and of the source tree are not inputs, so that the host and target units, whose `OUT_DIR` differ, share an entry. An argument of the configure step that names a directory under `OUT_DIR`, such as `-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=`, is left out for the same reason.

The path of `cl.exe` identifies the Microsoft compiler, because it holds the Visual Studio installation and the version directory of the toolset. `cl.exe` has no `--version` option.

The inputs leave out the variables through which GCC and Clang read include and library directories, `CPLUS_INCLUDE_PATH`, `CPATH`, `C_INCLUDE_PATH`, and `LIBRARY_PATH`, and no dependent build script emits a rerun instruction for them. This is a known limit of the entries of `slang-compiler-sys`, `slang-glslang-sys`, and `slang-rhi-sys`: after a change of one of these variables, a build reuses an entry that was built with other headers or libraries. The user removes the entry, which Section 9.3 permits when no build runs, so that the next build builds it with the new paths (Section 17, SRCQ-4).

## 10. Rerun Instructions

SRC-18. The build script MUST emit `cargo::rerun-if-changed` for the stamp file of the clone entry that it exports, and no other rerun instruction. After the metadata of SRC-1, it MUST emit `cargo::rustc-env=SLANG_SRC_SHARED_DIR=<shared folder>`, with the shared folder that it found. It MUST emit no other instruction.

The build script has no input of its own other than `recorded.rs`, which it compiles in, so a change of the recorded commit rebuilds and reruns it. The stamp file makes Cargo rerun it when the clone entry was removed (SRC-16). A dependent build script emits its own rerun instruction for the stamp file of each entry that it uses, and Section 16, SRCA-1, records that it runs again whenever the metadata of `slang-src` changes, such as after a change of `recorded.rs` in a fork patched in through `[patch]`.

`SLANG_SRC_SHARED_DIR` is neither a rerun instruction nor `links` metadata, and it reaches only the compilation of `slang-src` itself (Section 3.1), where the test of the real host reads the shared folder that the build script of the same build used (Section 12). A test cannot find that folder again from its own `OUT_DIR`, because it does not know the `TARGET` of the build. The build script never reads the variable, so SRC-21 holds, and only the crate's tests read it.

## 11. Failure Messages

SRC-19. On each failure of this table, the build script MUST end by returning an error from its `main` (SYS-4) whose message contains every item of the second column. A build that stops without naming what is missing and how to supply it leaves the user to guess.

In this table, the offline advice is the sentence that says two things: a vendored build needs network access for its first build in a target directory, and a build without the Cargo feature `vendored` links the Slang of an installed Vulkan SDK and needs no network access.

| Failure | The message contains |
| --- | --- |
| `RECORDED_COMMIT` is not 40 lowercase hexadecimal digits (SRC-24) | The value in quotation marks, escaped as for `OUT_DIR`, and that the recorded commit has to be a full commit hash of 40 lowercase hexadecimal digits |
| `RECORDED_BUILD_TAG` has a form that SRC-12 rejects | The value in quotation marks, escaped as for `OUT_DIR`, and the form that a build tag needs |
| `RECORDED_CLONE_ENTRY` differs from the name that the build script computes (SRC-25) | Both names in quotation marks, and that the computed name is to be recorded as `RECORDED_CLONE_ENTRY` in `recorded.rs`, where the recipes read it |
| `OUT_DIR` is not an absolute path in UTF-8 without line breaks | The value in quotation marks, with every control character escaped and every byte that is not UTF-8 replaced, and that Cargo sets it |
| `git` cannot start | The command and the reason that the operating system gives. Only when the operating system does not find the program, it adds that a vendored build needs git on `PATH`, because that advice misleads for any other reason |
| `CARGO_NET_OFFLINE` is `true` and no clone entry of the recorded commit exists | That the build needs to clone Slang, the commit, and the offline advice |
| A git command that reaches the network fails | The command, its exit status, its error output, the commit, and the offline advice |
| Another git command fails | The command, the directory, its exit status, and its error output |
| A required submodule has no gitlink or is not checked out (SRC-4) | The path of the submodule, the commit, and that the native builds need it, which a commit patched in through `[patch]` may lack |
| `git ls-files -z --deleted` lists a tracked file after the clone (SRC-26) | The commit, the number of missing files, the temporary work tree, the longest missing path with its path in the clone entry and the length of that path in characters, that git leaves out a file that it cannot create, most often because its path exceeds the limit of 260 characters of a Windows path, which the compilers and CMake that read the tree observe as well, the shared folder, and the advice to set `CARGO_TARGET_DIR` to a directory with a shorter path and build again |
| A clone entry has no work tree, reports another commit, or git cannot read it | The path of the entry, and the missing work tree, both commits, or the command with its exit status and error output. It also says that another build may be reading the entry, and that removing the entry or running `cargo clean` when no build runs fixes it |
| A file or directory under `OUT_DIR` or the shared folder cannot be inspected, listed, created, locked, renamed, removed, or written | The action, the path, and the reason that the operating system gives |

The build script emits no `cargo::warning`. Cargo shows the warnings of a registry dependency's build script only when the build fails, so a warning would not reach the user who needs it, and an error does.

## 12. Tests

The crate has one integration-test binary, `tests/slang_src/`, whose root `main.rs` declares the module `build_script`. No test clones Slang. The build script that builds the test binary performs the real clone, and a test of the real host checks what it left.

- `build_script`: The pure tests of `source.rs`, which the module includes with `recorded.rs`, against a fake host. They run under Miri, and they cover these parts:
  - The git commands of the clone in order, with their settings and environment, and the network settings only on the commands that fetch (SRC-7, SRC-8).
  - The use of an existing clone entry without the network, and the variables that the build script reads (SRC-21).
  - `CARGO_NET_OFFLINE`, which stops a clone only when it is exactly `true` (SRC-9).
  - The removal of only the numbered temporary directories of the recorded commit (SRC-16).
  - Every failure of Section 11 with its message, including each file operation that fails and an `OUT_DIR` that is not UTF-8, on Unix and on Windows.
  - The lookup of the base directory and the shared folder that the build script exports (SRC-23).
  - The forms of the recorded commit and the build tag, and the version identifiers that a build tag gives (SRC-12, SRC-13, SRC-24).
  - The agreement of the constants with the manifest version and with the crate documentation, and of the crate documentation with the list of required submodules (SRC-11, SRC-20).
  - The name of the clone entry: the values of the hash, the form of the name and its dependence on the list of required submodules, its length of at most 40 characters and its characters, the equality of `RECORDED_CLONE_ENTRY` with the computed name and the failure for a stale name, and the entries `src/<commit>` and `src/<commit>-<submodules>` of the earlier layouts, which the build script neither reads nor removes (SRC-25).
  - The check of the work tree: the message for files missing from the work tree, a missing file of a submodule, which the message names with the path of the submodule, and a failure of `git ls-files` (SRC-26).
- `build_script::ffi`: The tests of `system.rs` on the real host. They start git and write files under `CARGO_TARGET_TMPDIR`, so they sit in a module named `ffi`, which Miri skips. They cover these parts:
  - The file operations, the stamp time, and the lock, which `System` holds until it is dropped.
  - The environment and the closed standard input of git, and the list of repository variables against the one that the installed git prints (SRC-8).
  - The lookup of SRC-23 on a real file system.
  - The rejection of a clone entry without its work tree.
  - The reuse of the clone entry that the build script of the same build wrote.

The reuse test reads the shared folder from `SLANG_SRC_SHARED_DIR` (SRC-18) and runs the logic on the real host with `CARGO_NET_OFFLINE=true`, so it uses that clone entry and cannot clone. It checks the exported instructions, the work tree with every required submodule, the file `build-tag`, the time of the stamp file, and that no temporary directory is left. It fails, and does not skip, when the entry is missing.

The test binary builds only through `test-vendored` and `check-vendored`, because every other recipe leaves `slang-src` out (TOOL-32 of `docs/design/tooling.md`). The vendored CI job runs `test-vendored` on Linux, Windows, and macOS, so the clone runs on each platform. A second clone inside a test would cost as much time and disk space as the first and prove nothing more.

No recipe runs the pure tests under Miri, which is a known gap. Every recipe that runs Miri leaves `slang-src` out, and `cargo miri` builds in a tagged directory of its own inside the target directory (Section 3.2), where the lookup of SRC-23 finds another shared folder, so the build script would clone a second time (Section 17, SRCQ-3).

## 13. Crate Documentation

SRC-20. The crate-level documentation of `slang-src` MUST state what a vendored build needs, because it is the home of these facts for users, and every README and the crate-level documentation of `slang-compiler-sys` only point to it (Table 1 of `docs/writing-readmes.md`):

- git, and network access for the first build in a target directory.
- CMake 3.22 or later, and Ninja.
- A C++20 compiler, which with the Microsoft toolchain the build finds without a developer command prompt.
- Python 3 on `PATH` for the build of slang-glslang, under the name `python3` or `python`, because an installation of Python 3 on Windows may provide only `python`.
- libclang, which every build of `slang-compiler-sys` needs.

It MUST also state that a vendored build needs no Vulkan SDK, and it MUST state the shared folder:

- That it is the directory `slang-src` in the Cargo target directory, or in the build directory when Cargo uses a separate one.
- That every profile, and both the host and target units, of a project use it, so a procedural macro and the program clone and build Slang once.
- That `cargo clean` removes it, and that removing it when no build runs is safe.
- That projects share it only by sharing a Cargo target directory, such as through `CARGO_TARGET_DIR`.
- How the build script finds it, as SRC-23 specifies, and the known limits of Section 9.1, in which each unit builds in its own `OUT_DIR`.

It MUST NOT name an environment variable of `slang-src`, because none exists (SRC-21). A user who does not know where the folder is or when it is lost cannot reason about the time and disk space that a vendored build takes.

The crate-level documentation also states:

- That the crate builds nothing and is a dependency of `slang-compiler-sys` under its feature `vendored` and of `slang-glslang-sys`, which exists only for that build and has no feature.
- The recorded commit, its build tag, and the version of the crate, and that the version of a release names the commit (Section 7.2). The tests of SRC-11 keep them equal to `recorded.rs` and the manifest.
- How to build another commit through `[patch]`, and that it is best effort (SRC-22).
- The error for a work tree that git left incomplete, with its advice of a target directory with a shorter path (SRC-26).
- What the crate fetches and from where, the git settings of SRC-8, that it honours `CARGO_NET_OFFLINE`, and that it runs nothing from the tree (Section 14).
- What a dependent build script relies on, in a section of its own: that the tree at `root` is shared and read-only, the list of required submodules of SRC-4, that `src/` of the shared folder belongs to `slang-src` and each dependent owns the directory named after its crate (Section 9.2), the entry protocol of SRC-16 in short, and the rerun instruction on the stamp file. A third-party crate that depends on a published `slang-src` sees only the crate documentation, and a change of any of these is a breaking change that no signature shows.
- `# Features`: the crate has no Cargo features.
- `# Platform support`: the crate runs wherever git runs, and the vendored builds that use it are tested on the platforms of the vendored CI jobs (Section 15.3).

## 14. Security Considerations

The threat model has five parts:

1. An attacker on the network between the build and GitHub, or in control of a DNS resolver or proxy, tampers with what git receives.
2. An attacker controls the Slang repository on GitHub or the repository of a submodule, and changes what a branch or a tag points to.
3. An attacker with write access to the Cargo target directory places or changes an entry of the shared folder.
4. The environment of the build, such as a git hook or a parent build, sets git variables or Cargo variables that the build script reads.
5. The user patches in a fork of `slang-src` whose commit is malicious.

Untrusted shader source and file paths. The crate reads no shader source and gives no path to Slang. It reads only the variables of Section 15.2 and git's answers.

Unsafe code and foreign function interface (FFI) invariants. The crate has no unsafe code and declares no foreign function (SRC-2).

Build scripts that run native builds. The build script runs git and nothing else. It runs no CMake, no compiler, and no script of the tree. A fetch never brings hooks, and `git init` installs only the hooks of the user's git template, which in git's default template are inactive samples (Section 3.3). It fetches only the recorded commit from `https://github.com/shader-slang/slang.git` and the commits of the required submodules from the HTTPS URLs that `.gitmodules` of that commit names, and SRC-8 forbids every other protocol and every prompt for credentials. It fetches no tags and no history.

It fetches by full commit hash, and git verifies the object ID of every object it receives (Section 3.3). The commit hash therefore identifies the content of the superproject, and its gitlinks identify the content of each submodule, so an attacker on the network or in control of a branch or tag cannot substitute other content for the recorded commit. HTTPS adds confidentiality.

The build script writes only into `OUT_DIR` and into the shared folder in the base directory (SRC-23), which Cargo created for this build. It writes nothing outside them.

Every path that it writes into a Cargo instruction derives from `OUT_DIR`, which is checked for line breaks first (Section 11), and from the recorded commit, which has to be 40 lowercase hexadecimal digits (SRC-24). A value therefore cannot add an instruction or name a path outside the shared folder. It removes the repository variables and `GIT_ALLOW_PROTOCOL` from the environment of git (SRC-8), so git answers for the tree it was given and keeps the protocol settings.

Dependencies. The crate has no dependencies (SRC-2). Every crate that depends on it requires its exact version (SRC-14), which fixes the recorded commit for a given version of each dependent.

These attacks are out of scope:

- A malicious commit in a fork that the user patches in is code that the user chose to build. The dependent builds run its CMake scripts and compile it with the user's permissions.
- An attacker with write access to the target directory can place any library in an entry, and the dependent builds link it. Such an attacker can already replace any artifact that Cargo builds there, so the shared folder adds no new exposure.
- The user's git configuration applies to every git command, because the user controls it and needs it on some networks. It includes a proxy, a rule that rewrites URLs, and a template of `init.templateDir` whose active hooks git runs in the clone, such as a hook that runs after the checkout. The build script does not guard against it.
- Attacks on the repository of this workspace are a matter of access control.

The residual risk has three parts:

1. git's object IDs are SHA-1 hashes. git's variant detects the known collision attacks, but an attacker who could produce a new SHA-1 collision could substitute content for a commit.
2. A clone entry in the shared folder is trusted once it exists, and the build script does not verify its content again.
3. A network that stalls a connection without transferring anything can hold a build until the operating system gives up the connection, which can take minutes (Section 16, SRCA-2).

## 15. Operational Considerations

### 15.1. Cargo Features

`slang-src` has no Cargo features. It is a dependency of the crates that build Slang only under their feature `vendored`, so a default build never compiles it.

### 15.2. Environment Variables

This table lists every environment variable that the build script reads or sets for git, with its default. The build script reads no environment variable of its own (SRC-21).

| Variable | Read by | Default and behaviour when absent |
| --- | --- | --- |
| `CARGO_NET_OFFLINE` | The build script, before it clones | Absent or any value other than `true`, so the build script may clone. When `true`, it does not reach the network (SRC-9). |
| `OUT_DIR` | The build script | Set by Cargo. The start of the search for the base directory (SRC-23). A value that is not an absolute path in UTF-8 without line breaks stops the build. |
| `TARGET` | The build script | Set by Cargo. Compared with the name of the nearest tagged directory (SRC-23). When it is absent or not UTF-8, step 2 of SRC-23 does not apply. |
| `PATH` | The operating system, when the build script starts git | git has to be on it, or the build stops (Section 11). |
| `GIT_TERMINAL_PROMPT` | Set to `0` by the build script for every git command that reaches the network | The value of the environment is never used, so git never asks for credentials (SRC-8). |
| `GIT_ALLOW_PROTOCOL` | Removed by the build script from the environment of every git command that reaches the network | A value in the environment of the build never overrides the protocol settings (SRC-8). |
| `GIT_OPTIONAL_LOCKS` | Set to `0` by the build script for every git command | The value of the environment is never used (SRC-8). |
| `GIT_DIR`, `GIT_INDEX_FILE`, and the other repository variables | Removed by the build script from the environment of every git command | A value in the environment of the build never reaches git (SRC-8). |

### 15.3. Platforms and Minimum Supported Rust Version

The crate builds on every platform that Rust supports, and its build script runs where git runs. The vendored builds that use it are tested on the targets of the vendored CI jobs, and allowed on other 64-bit targets with the limits of Section 6 of the sys design. The crate uses the workspace's `rust-version`, and `std::fs::File::lock` is stable at that version.

### 15.4. Failure Modes

This table lists the failure modes of the crate, how each is reported, and the recovery. Every error is the error that the build script returns, with the content of Section 11.

| Failure | Recovery |
| --- | --- |
| No network access, a blocked host, or `CARGO_NET_OFFLINE` set to `true`, and no clone in the shared folder | Build once with network access in the same target directory, or build without `vendored` |
| A required submodule is missing from a commit patched in through `[patch]` | Use the recorded commit, or a commit with the submodules of Section 3.4 |
| The recorded commit or build tag of a fork patched in through `[patch]` has the wrong form | Write the full commit hash and the build tag of that commit into `recorded.rs` (SRC-12, SRC-24) |
| git is missing | Install git |
| A clone entry has no work tree, is at another commit, or cannot be read | Remove the entry, or run `cargo clean`, when no build runs |
| git left tracked files out of the work tree of a new clone, most often because a path exceeds the limit of Windows (SRC-26) | Set `CARGO_TARGET_DIR` to a directory with a shorter path and build again |
| The shared folder or `OUT_DIR` cannot be written | Free disk space or fix the permissions of the target directory |
| A commit patched in through `[patch]` builds a Slang that the binding generator or the safe crates do not support | Use the recorded commit, which the crates are tested against |

### 15.5. Logs

The build script emits no warnings and writes no log. Its only output is the instructions of SRC-1 and SRC-18 and, on a failure, its error. The crate emits nothing at run time.

## 16. Assumptions to Confirm

Each row is a fact that this design relies on and that is not verified. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SRCA-1 | Cargo runs a dependent build script again whenever the build script of `slang-src` runs again and its metadata changes | Section 10 | A build in the M7 code steps that changes `RECORDED_COMMIT` between two builds in one target directory |
| SRCA-2 | The low-speed settings of SRC-8 stop a transfer that stalls after it connected, and the operating system stops a connection attempt to an unreachable address after about two minutes | SRC-8, Section 14 | A run of the clone against an unroutable proxy, on Linux, Windows, and macOS |
| SRCA-3 | The required submodules of Section 3.4 are all that the build of slang-glslang reads on Windows and macOS, with mimalloc turned off | Section 3.4 | The vendored CI jobs on Windows and macOS |
| SRCA-4 | The counts of Section 8, and the single clone and native build for each target directory, which come from stub crates in the shape of the workspace on x86_64 Linux, hold for the real crates | Section 8 | A vendored build of the example that calls `slango-macros`, with each command of the table |
| SRCA-5 | On Windows and macOS, as on x86_64 Linux, Cargo runs a build script again when the stamp file that it names is missing, and not for a stamp file whose modification time is 2000-01-01 | SRC-16, Section 9.3 | The vendored CI jobs on Windows and macOS, with a test that removes the shared folder between two builds |
| SRCA-8 | A `[patch]` entry that names a fork of `slang-src` with the same manifest version and another recorded commit replaces `slang-src` for every dependent, and the build exports the commit of the fork | SRC-22 | A build in the M7 code steps of a scratch project that patches `slang-src` with a local copy that records another commit |
| SRCA-9 | On Windows and macOS, as on x86_64 Linux, Cargo tags the build directory that it creates and the directory of the triple under `--target`, names that directory exactly after `TARGET`, and `std::fs::File::lock` serializes two build scripts that run at once, so that SRC-23 and SRC-16 give one folder and one creation of each entry | SRC-16, SRC-23, Section 8 | A vendored build on Windows and macOS with `--target`, with a separate build directory, and with two units that clone at once |

The next assumption takes SRCA-10.

## 17. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SRCQ-1 | Do the clone, the copy and build tree of `slang-compiler-sys`, and the build tree of `slang-glslang-sys` under `OUT_DIR` fit within the path length limit of Windows when the target directory is deeper than that of the hosted Windows runner, or when a later recorded commit tracks a longer path, or does a build script need a shorter layout on Windows? On that runner, the clone leaves 5 characters below the limit, and SRC-26 stops a clone that exceeds it with the length of the path (Section 3.5). The build tree of `slang-glslang-sys` is the deepest, because CMake mirrors the absolute path of each source outside the wrapper project under `CMakeFiles/<target>.dir/` (Section 4 of `docs/design/slang-glslang-sys.md`). | This document, with `docs/design/slang-compiler-sys.md` and `docs/design/slang-glslang-sys.md` | M7 |
| SRCQ-3 | Which recipe runs the pure tests of `slang-src` and of the build script of `slang-glslang-sys` under Miri, and how does it keep the build scripts from cloning and building a second time in the target directory of Miri (Section 12, Section 9 of `docs/design/slang-glslang-sys.md`)? | `docs/design/tooling.md`, with this document | M7 |
| SRCQ-4 | Should SRC-17 add `CPLUS_INCLUDE_PATH`, `CPATH`, `C_INCLUDE_PATH`, and `LIBRARY_PATH` to the inputs of the entry key, and should each dependent build script emit a rerun instruction for them, so that a changed include or library path no longer reuses an entry built with other headers (Section 9.4)? | This document, with `docs/design/slang-compiler-sys.md`, `docs/design/slang-glslang-sys.md`, and `docs/design/slang-rhi-sys.md` | Not assigned |

The next question takes SRCQ-5.

## 18. References

### 18.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 18.2. Informative References

- [SEMVER] Preston-Werner, T., "Semantic Versioning 2.0.0", <https://semver.org/spec/v2.0.0.html>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary. The labels SRC-3, SRC-5, SRC-6, SRC-10, and SRC-15 belonged to requirements that this design removed, and they are not reused. The next requirement takes SRC-27.

| Label | Section | Summary |
| --- | --- | --- |
| SRC-1 | 4 | `links = "slang-src"`, the metadata keys `root`, `commit`, `build_tag`, and `shared_dir`, and direct dependents only. |
| SRC-2 | 4 | No unsafe code, no dependencies, and the logic behind a host trait in an included file. |
| SRC-4 | 5 | The required submodules have gitlinks, checked after the checkout, and `.git` entries, checked after the update, or the build stops with the message of Section 11. |
| SRC-7 | 6.1 | The clone is an init, a depth-1 fetch of the recorded commit, a checkout, a local check of the gitlinks, a depth-1 update of the required submodules, and the checks of SRC-4 and SRC-26, renamed into place in the shared folder when complete. |
| SRC-8 | 6.1 | git runs with HTTPS only, no prompt, no `GIT_ALLOW_PROTOCOL`, a low-speed limit, no optional locks, and no repository variables, which a constant lists and a test checks against the installed git. |
| SRC-9 | 6.2 | No network access with `CARGO_NET_OFFLINE=true`, and a failed or impossible clone stops the build with the offline advice and never falls back. |
| SRC-11 | 7.1 | The recorded commit and build tag live in `recorded.rs`, the manifest version, and the crate documentation, tests compare them, and scripts read them and the name of the clone entry through `recorded-slang`. |
| SRC-12 | 7.2 | A build tag has the form `<major>.<minor>[.<patch>][-<count>-g<hash>]`, or the build stops. |
| SRC-13 | 7.2 | The version is the version of the release with `slang.<major>.<minor>.<patch>.<count>.g<hash>` as its last pre-release identifiers, such as `0.1.0-alpha.2.slang.2026.17.0.25.gc7954ebd1`. |
| SRC-14 | 7.2 | Every dependent requires the exact version with `=`. |
| SRC-16 | 9.2 | Every entry is built under an exclusive lock in a temporary directory, gets a stamp file with an old time, is renamed into place when complete, and is watched through its stamp file. |
| SRC-17 | 9.4 | An entry of libraries is named by the FNV-1a hash of its listed inputs, which the entry holds and a hit compares in full. |
| SRC-18 | 10 | The only rerun instruction is the stamp file of the clone entry, and the only other instruction after the metadata is `cargo::rustc-env=SLANG_SRC_SHARED_DIR` for the crate's own tests. |
| SRC-19 | 11 | Each failure returns an error with the content of the table. |
| SRC-20 | 13 | The crate documentation states the requirements, that no Vulkan SDK is needed, and the shared folder with its lookup and its limits, and name no environment variable of the crate. |
| SRC-21 | 5 | The tree is always a clone of the recorded commit, and the build script reads no environment variable of its own. |
| SRC-22 | 7.3 | Another commit only through `[patch]` with a fork that keeps the manifest version, best effort. |
| SRC-23 | 9.1 | The shared folder is `slang-src` in the nearest tagged ancestor of `OUT_DIR`, its parent when that ancestor is the directory of `TARGET` under another tag, or `OUT_DIR` without a tag. |
| SRC-24 | 7.1 | A recorded commit that is not 40 lowercase hexadecimal digits stops the build before anything else. |
| SRC-25 | 6.1 | The clone entry is named by the first 23 digits of the commit and the hash of the list of required submodules, at most 40 characters, so a change of the list makes a new entry, and `RECORDED_CLONE_ENTRY` records the name, which the build checks before anything else. |
| SRC-26 | 5 | After the clone, `git ls-files -z --deleted` in the tree and in each required submodule lists nothing, or the build stops before the rename with the longest missing path and its length, and `core.longpaths` stays unset. |
