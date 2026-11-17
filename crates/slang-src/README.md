# slang-src

slang-src provides the [Slang](https://shader-slang.org) source tree at a recorded commit to the build scripts of slang-compiler-sys and slang-glslang-sys, which build Slang from source. A program never depends on slang-src directly, and uses [slang-compiler](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler/README.md) or another crate of the project instead.

> [!WARNING]
> This crate is an alpha release, and what it provides can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

A program never depends on slang-src directly, because only the build scripts of other crates read what it provides. To build Slang from source, enable the Cargo feature `vendored` of the crate that the program uses, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/#features) lists.

## Requirements

The build script of slang-src clones Slang from GitHub on the first build in a Cargo target directory, so that build needs network access. What a vendored build needs and costs, and where it keeps the clone, are in the [API documentation](https://g-whiz.github.io/slang-rust/slang_src/).

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/slang_src/) describes the recorded commit, what the build script fetches, and the shared folder. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

slang-src is licensed under `Apache-2.0 WITH LLVM-exception`, the licence of Slang. The text is in [LICENSE-APACHE-LLVM](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE-LLVM).
