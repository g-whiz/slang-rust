# slang-rhi-sys

slang-rhi-sys provides the raw, unsafe Rust bindings to [slang-rhi](https://github.com/shader-slang/slang-rhi), the render hardware interface of the [Slang](https://shader-slang.org) shader compiler. Its build script generates the bindings from the slang-rhi headers in every build, and builds slang-rhi from source and links it statically. Most programs depend on [slang-rhi](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi/README.md) instead, which wraps these bindings in a safe API for compute dispatches and draws.

> [!WARNING]
> This crate is an alpha release, and its API can change in any release. Read the [project status](https://github.com/g-whiz/slang-rust#status) before you depend on it.

## Installation

```toml
[dependencies]
slang-rhi-sys = "0.2.0-alpha.1"
```

A program that names an item of the Slang headers in the bindings, such as `ISlangUnknown` or a global session, also depends on slang-compiler-sys at the same version as this crate:

```toml
[dependencies]
slang-compiler-sys = "0.2.0-alpha.1"
```

## Requirements

The crate always builds Slang and slang-rhi from one Slang source tree, which slang-src clones, and never uses the Slang of a Vulkan SDK. Building it needs the tools that the [slang-src documentation](https://g-whiz.github.io/slang-rust/slang_src/#what-a-vendored-build-needs) lists, and on Linux also the development headers of Xlib, as the [crate documentation](https://g-whiz.github.io/slang-rust/slang_rhi_sys/) describes.

The crate builds on Linux with the GNU C library, with the Vulkan backend of slang-rhi, and on x86_64 Windows with the Microsoft toolchain, with its Direct3D 12 backend. It does not build on macOS. On Windows, a program that creates a pipeline needs `dxcompiler.dll` when it runs, as the [crate documentation](https://g-whiz.github.io/slang-rust/slang_rhi_sys/) describes.

A program built with this crate contains code of slang-rhi and Slang, which is under the licence `Apache-2.0 WITH LLVM-exception`, and third-party code under its own licences, which the [crate documentation](https://g-whiz.github.io/slang-rust/slang_rhi_sys/) lists with the notices that a distributor of a binary carries.

## Documentation

The [API documentation](https://g-whiz.github.io/slang-rust/slang_rhi_sys/) describes the shape of the bindings, the build, and the safety contract that every binding shares. The [project README](https://github.com/g-whiz/slang-rust) lists the other crates.

## Licence

slang-rhi-sys is licensed under `Apache-2.0 WITH LLVM-exception`, the licence of Slang. The text is in [LICENSE-APACHE-LLVM](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE-LLVM).
