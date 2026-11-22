# slang-glslang-sys

slang-glslang-sys builds glslang and SPIRV-Tools as static libraries for a Slang built from source, and holds the loader through which that Slang optimizes SPIR-V, the binary shader format of Vulkan. A Slang built from source with the feature `vendored` needs it to optimize SPIR-V, and most programs depend on [slang-compiler](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler/README.md) instead, which installs this loader when its own feature `vendored` is on.

> [!WARNING]
> This crate is an alpha release, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

A program that uses the raw bindings of [slang-compiler-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler-sys/README.md) with the feature `vendored` adds this crate to install the loader itself:

```toml
[dependencies]
slang-glslang-sys = "0.2.0-alpha.1"
```

## Requirements

The crate always builds Slang from source, from the source tree that slang-src clones, so building it needs the tools that the [slang-src documentation](https://g-whiz.github.io/slang-rust/slang_src/#what-a-vendored-build-needs) lists.

The static libraries that the crate links hold code of glslang and SPIRV-Tools from the Slang source tree, which is under their own licences, and glue code of Slang, which is under the licence `Apache-2.0 WITH LLVM-exception`.

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/slang_glslang_sys/) shows how to install the loader on a global session. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

slang-glslang-sys is licensed under `Apache-2.0 WITH LLVM-exception`, the licence of Slang. The text is in [LICENSE-APACHE-LLVM](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE-LLVM).
