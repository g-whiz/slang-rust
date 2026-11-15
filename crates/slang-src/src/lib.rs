//! The Slang source tree at a recorded commit, for the crates that build
//! Slang from source.
//!
//! [Slang](https://shader-slang.org) is a shader compiler. The crates
//! `slang-compiler-sys` and `slang-glslang-sys` can build Slang and parts of
//! its source tree from source and link them statically, which is the
//! vendored build that the Cargo feature `vendored` of `slang-compiler-sys`
//! selects and that `slang-glslang-sys` exists for. This crate is their
//! dependency in that build. Its build script obtains the source
//! tree that they build from, and the crate builds nothing itself and has no
//! API. A program does not depend on it directly.
//!
//! The build script exports four values as `links` metadata, which Cargo
//! passes to the build script of each crate that depends on this crate
//! directly:
//!
//! - `DEP_SLANG_SRC_ROOT`: the absolute path of the top level of the source
//!   tree.
//! - `DEP_SLANG_SRC_COMMIT`: the full hash of its commit.
//! - `DEP_SLANG_SRC_BUILD_TAG`: its build tag, the version string of that
//!   Slang build, such as `2026.17-25-gc7954ebd1`.
//! - `DEP_SLANG_SRC_SHARED_DIR`: the absolute path of the shared folder,
//!   which the next sections describe.
//!
//! # The recorded commit
//!
//! The source tree is always a shallow clone of one Slang commit, the
//! recorded commit `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`, whose build tag
//! is `2026.17-25-gc7954ebd1`. It is the Slang that ships in the Vulkan
//! software development kit (SDK) that the crates are tested with.
//!
//! The version of a release of this crate names the Slang that it clones.
//! The pre-release identifiers of the version after `slang` are the numbers
//! of the build tag, with a missing patch number or commit count written as
//! `0`, followed by `g` and the first nine digits of the commit hash. The
//! version `0.1.0-slang.2026.17.0.25.gc7954ebd1` therefore names the build tag
//! `2026.17-25-gc7954ebd1`. The identifiers are not a pre-release of `0.1.0`,
//! and they are numeric so that versions of Slang order as numbers.
//!
//! Every crate that depends on this crate requires its exact version, with
//! `=`, because a caret requirement on a pre-release would let
//! `cargo update` move a build to another Slang.
//!
//! # Another Slang commit
//!
//! The crates offer one way to build another Slang commit, which is Cargo's
//! `[patch]` table, and another commit is best effort. The crates are written
//! and tested against the recorded commit, and a commit whose required
//! submodules differ stops the build. To build another commit:
//!
//! 1. Fork this crate.
//! 2. Change `RECORDED_COMMIT` and `RECORDED_BUILD_TAG` in its file
//!    `recorded.rs` to the full commit hash and the build tag of that commit.
//! 3. Keep the version in its manifest, because every crate that depends on
//!    this crate requires that exact version, and Cargo uses a patch only when
//!    its version satisfies every requirement.
//! 4. Name the fork in the `[patch]` table of the root manifest of your
//!    project, such as with this table, in which the address and the branch
//!    are placeholders:
//!
//! ```toml
//! [patch.crates-io]
//! slang-src = { git = "https://example.com/user/slang-src.git", branch = "other-slang" }
//! ```
//!
//! The version of the fork then no longer names the Slang that it clones,
//! while the commit and the build tag that it exports do.
//!
//! # What a vendored build needs
//!
//! A vendored build needs these on the build machine:
//!
//! - The program `git`, on `PATH`.
//! - The program `cmake`, version 3.22 or later.
//! - The program `ninja`.
//! - A C++ compiler.
//! - Python 3 on `PATH`, which the build of slang-glslang needs.
//! - Network access for its first build in a Cargo target directory.
//!
//! A build without the Cargo feature `vendored` links the Slang of an
//! installed Vulkan SDK and needs none of these and no network access.
//!
//! # The shared folder
//!
//! The clone, and the libraries that the crates build from it, are kept in
//! the shared folder, which is the directory `slang-src` in the Cargo target
//! directory, or in the build directory when Cargo uses a separate one:
//!
//! - Every profile of a project, and both its host and its target units, use
//!   the one shared folder, so a procedural macro that links Slang and the
//!   program clone Slang once and build it once.
//! - A first build clones and builds while later builds use what the folder
//!   holds and need no network access.
//! - `cargo clean` removes the folder with the rest of the target directory,
//!   and the next vendored build clones and builds again. Removing the folder,
//!   or any directory in it, is safe whenever no build runs.
//! - Two projects share the folder only when they share a Cargo target
//!   directory, such as through `CARGO_TARGET_DIR`.
//! - Nothing removes old entries of the folder, so it grows with each recorded
//!   commit and target that a project builds with.
//!
//! The build script finds the folder through the file `CACHEDIR.TAG` that
//! Cargo writes into a target directory or build directory that it creates.
//! In these cases Cargo wrote no such file, so each unit clones and builds in
//! its own output directory instead, which is correct but takes the time and
//! the disk space once for each unit:
//!
//! - A target directory or build directory that existed before the first
//!   build, such as one created with `mkdir`, restored from a cache of a
//!   continuous integration service, or mounted as a volume.
//! - A target directory whose `CACHEDIR.TAG` someone deleted.
//! - The temporary target directory of `cargo install` from crates.io or from
//!   git.
//!
//! # What the crate fetches
//!
//! The build script clones the recorded commit, by its full hash, from
//! `https://github.com/shader-slang/slang.git` with a depth of one, and then
//! the commits that the gitlinks of that commit record for the submodules that
//! the vendored builds need, from the addresses that its `.gitmodules` names.
//! It fetches no tags and no history. git verifies the object ID of every
//! object that it receives, so the commit hash identifies the content of the
//! whole tree.
//!
//! Every git command that reaches the network runs with these git settings,
//! so that only HTTPS can carry the clone, git never asks for credentials, and
//! a transfer that stays below 1000 bytes per second for 60 seconds fails
//! instead of hanging:
//!
//! - `protocol.allow=never` and `protocol.https.allow=always`.
//! - `http.lowSpeedLimit=1000` and `http.lowSpeedTime=60`.
//! - `GIT_TERMINAL_PROMPT=0` in its environment.
//!
//! Your own git configuration still applies, such as a proxy or a rule that
//! rewrites addresses. The build script reaches the network only when the
//! shared folder holds no clone of the recorded commit, and never when the
//! environment variable `CARGO_NET_OFFLINE` is `true`, which Cargo's option
//! `--offline` does not set for a build script. It then stops the build with
//! an error, and it never falls back to an installed Slang. It runs git and
//! nothing else, and it runs no code of the tree.
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate builds on every platform that Rust supports, and its build
//! script runs wherever git runs. The vendored builds that use it are tested
//! on `x86_64` Linux, `x86_64` Windows, and aarch64 macOS.
