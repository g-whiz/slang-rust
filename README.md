# slang-rust

[![CI](https://github.com/g-whiz/slang-rust/actions/workflows/ci.yml/badge.svg)](https://github.com/g-whiz/slang-rust/actions/workflows/ci.yml)

slang-rust is a set of Rust crates for the [Slang](https://shader-slang.org) shader language. They compile and reflect Slang shaders, run compute kernels and render pipelines on the graphics processing unit (GPU) through [wgpu](https://wgpu.rs), generate typed Rust bindings to compute kernels while a crate builds, and show what a shader draws in a [Slint](https://slint.dev) user interface. They also run compute kernels through [slang-rhi](https://github.com/shader-slang/slang-rhi), the render hardware interface of Slang.

## Status

> [!WARNING]
> Every release of these crates is an alpha release, and the API can change in any release, including from one alpha release to the next. Cargo matches a pre-release version only from a requirement that names a pre-release, so a dependent names the full version, such as `"0.1.0-alpha.1"`, and a requirement such as `"0.1"` fails to resolve. The requirement `"0.1.0-alpha.1"` also accepts a later alpha release, so a dependent that needs one fixed API writes `"=0.1.0-alpha.1"`.

## Crates

| Crate | Use it to | API documentation |
| --- | --- | --- |
| [slango](https://github.com/g-whiz/slang-rust/blob/main/crates/slango/README.md) | Run Slang compute kernels and draw with Slang render pipelines on the GPU through wgpu. | [slango](https://g-whiz.github.io/slang-rust/slango/) |
| [slango-macros](https://github.com/g-whiz/slang-rust/blob/main/crates/slango-macros/README.md) | Compile a Slang file while your crate builds and call its compute kernels through typed Rust bindings. | [slango_macros](https://g-whiz.github.io/slang-rust/slango_macros/) |
| [slang-compiler](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler/README.md) | Compile Slang shaders and reflect their parameters, with no GPU. | [slang_compiler](https://g-whiz.github.io/slang-rust/slang_compiler/) |
| [slang-slint](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-slint/README.md) | Show what a render pipeline of slango draws in a Slint user interface. | [Source](https://github.com/g-whiz/slang-rust/tree/main/crates/slang-slint) |
| [slang-rhi](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi/README.md) | Run Slang compute kernels on the GPU through slang-rhi, with Vulkan on Linux and Direct3D 12 on Windows. | [Source](https://github.com/g-whiz/slang-rust/tree/main/crates/slang-rhi) |
| [slango-codegen](https://github.com/g-whiz/slang-rust/blob/main/crates/slango-codegen/README.md) | Generate the typed bindings of slango-macros outside a macro call, such as to print them. | [slango_codegen](https://g-whiz.github.io/slang-rust/slango_codegen/) |
| [slang-compiler-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler-sys/README.md) | Call the raw Slang API where slang-compiler has no safe wrapper. | [slang_compiler_sys](https://g-whiz.github.io/slang-rust/slang_compiler_sys/) |
| [slang-rhi-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi-sys/README.md) | Call the raw slang-rhi API where slang-rhi has no safe wrapper. | [Source](https://github.com/g-whiz/slang-rust/tree/main/crates/slang-rhi-sys) |
| [slang-glslang-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-glslang-sys/README.md) | Give a Slang built from source its SPIR-V optimizer when you use slang-compiler-sys directly. | [slang_glslang_sys](https://g-whiz.github.io/slang-rust/slang_glslang_sys/) |
| [slang-src](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-src/README.md) | Provide the Slang source tree to the build scripts that build Slang from source, which a program never depends on directly. | [slang_src](https://g-whiz.github.io/slang-rust/slang_src/) |
| [slang-bindgen](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-bindgen/README.md) | Generate the raw Rust bindings of slang-compiler-sys from the Slang headers with libclang. | [slang_bindgen](https://g-whiz.github.io/slang-rust/slang_bindgen/) |

## Requirements

The crates need Rust 1.99 or later.

The crates support 64-bit targets only. These are the supported targets:

- Linux on x86-64, `x86_64-unknown-linux-gnu`.
- Windows on x86-64 with the Microsoft toolchain, `x86_64-pc-windows-msvc`.
- macOS on Apple silicon, `aarch64-apple-darwin`.
- macOS on x86-64, `x86_64-apple-darwin`.

Another 64-bit target is untested and unsupported, and the build warns about it.

slang-rhi and slang-rhi-sys build only on Linux with the GNU C library and on Windows on x86-64 with the Microsoft toolchain, as the [slang-rhi-sys documentation](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi-sys/src/lib.rs) describes.

Building needs Slang, from an installed Vulkan SDK or built from source with the feature `vendored`, and libclang, as the [slang-compiler-sys documentation](https://g-whiz.github.io/slang-rust/slang_compiler_sys/) describes.

## Getting Started

Run this command in the directory of your Cargo package to add slango:

```sh
cargo add slango@0.1.0-alpha.1
```

The [slango README](https://github.com/g-whiz/slang-rust/blob/main/crates/slango/README.md) shows a first program that runs a compute kernel.

## Examples

Run each command from the root of a clone of the repository. With the Slang of a Vulkan SDK, each example also needs the Slang shared library on the library path when it starts, as [the Slang of a Vulkan SDK](https://g-whiz.github.io/slang-rust/slang_compiler_sys/#the-slang-of-a-vulkan-sdk) in the slang-compiler-sys documentation describes.

```sh
# Compile a Slang file and print its entry points and parameters.
cargo run -p slang-rust-examples --bin compile-and-reflect -- examples/shaders/scale.slang
# Run a compute kernel with slango and check its results.
cargo run -p slang-rust-examples --bin compute-kernel -- examples/shaders/saxpy.slang
# Draw a textured quad in a window and reload the shader when its file changes.
cargo run -p slang-rust-examples --bin windowed-renderer -- examples/shaders/textured-quad.slang
# Show a shader in a Slint window with two sliders and reload it when its file changes.
cargo run -p slang-rust-examples --bin shader-playground -- examples/shaders/playground.slang
# Run a compute kernel through the typed bindings of slango-macros and check its results.
cargo run -p slang-rust-examples --bin typed-kernel
```

## Documentation

The [hosted API documentation](https://g-whiz.github.io/slang-rust/) holds the documentation of each published crate, built from each release tag. Contributors read [AGENTS.md](https://github.com/g-whiz/slang-rust/blob/main/AGENTS.md), which holds the rules for working in the repository.

## Getting Help

Report a bug or ask a question in the [issues of the repository](https://github.com/g-whiz/slang-rust/issues).

## Licence

Each crate has its own licence, which its README states:

| Crates | Licence |
| --- | --- |
| [slango](https://github.com/g-whiz/slang-rust/blob/main/crates/slango/README.md), [slango-macros](https://github.com/g-whiz/slang-rust/blob/main/crates/slango-macros/README.md), [slango-codegen](https://github.com/g-whiz/slang-rust/blob/main/crates/slango-codegen/README.md), [slang-compiler](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler/README.md), [slang-rhi](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi/README.md), and [slang-bindgen](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-bindgen/README.md) | `MIT OR Apache-2.0` |
| [slang-compiler-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-compiler-sys/README.md), [slang-rhi-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-rhi-sys/README.md), [slang-glslang-sys](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-glslang-sys/README.md), and [slang-src](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-src/README.md) | `Apache-2.0 WITH LLVM-exception`, the licence of Slang |
| [slang-slint](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-slint/README.md) | `GPL-3.0-or-later` |

slang-slint depends on Slint, and the [slang-slint README](https://github.com/g-whiz/slang-rust/blob/main/crates/slang-slint/README.md) states what that means for the licence of a program that uses it.
