# slang-compiler-sys

slang-compiler-sys provides the raw, unsafe Rust bindings to the [Slang](https://shader-slang.org) shader compiler and links the Slang library, with bindings that its build script generates from the Slang headers in every build. Most programs depend on [slang-compiler](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler/README.md) instead, which wraps these bindings in a safe API, or on [slango](https://github.com/g-whiz/slang-rust/blob/main/crates/slango/README.md), which runs Slang shaders on the GPU.

> [!WARNING]
> This crate is an alpha release, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

```toml
[dependencies]
slang-compiler-sys = "0.2.0-alpha.2"
```

## Requirements

Building needs Slang, from an installed Vulkan SDK or built from source with the feature `vendored`, and libclang, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/#linking-slang) describes.

A program built with this crate contains or loads code of Slang, which is under the licence `Apache-2.0 WITH LLVM-exception`. A build with the feature `vendored` also links code from the Slang source tree that is under its own licences: cmark-gfm, miniz, and lz4.

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/) describes every item and the safety contract of the bindings. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

slang-compiler-sys is licensed under `Apache-2.0 WITH LLVM-exception`, the licence of Slang. The text is in [LICENSE-APACHE-LLVM](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE-LLVM).
