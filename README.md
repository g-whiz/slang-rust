# slang-rust

slang-rust is a set of Rust crates for [Slang](https://shader-slang.org), a language and compiler for writing shaders. A shader is a small program that runs on a graphics processing unit (GPU), the chip that draws images and runs many computations at once.

The crates let a Rust program compile Slang code, read the parameters it declares, and run it on a GPU. The source code and the documents that this README names are in the repository at <https://github.com/g-whiz/slang-rust>.

## Crates

The workspace holds nine crates. No crate is published yet.

| Crate | Purpose |
| --- | --- |
| `slang-compiler-sys` | Raw bindings to the native Slang library, which its build script generates from the headers of the Slang that it links. Bindings are Rust declarations that let Rust code call a library written in another language. |
| `slang-bindgen` | The binding generator, which reads the Slang headers with libclang and writes the raw bindings for `slang-compiler-sys`. |
| `slang-src` | The crate that obtains the Slang source tree for a build of Slang from source, which `slang-compiler-sys` uses with its feature `vendored`. |
| `slang-glslang-sys` | The crate that builds slang-glslang, the part of Slang that optimizes SPIR-V, from the same source tree, and gives it to a Slang built from source. |
| `slang-compiler` | Safe, idiomatic wrappers over `slang-compiler-sys`, with no GPU dependency. It compiles Slang code to output formats such as SPIR-V, the binary shader format of Vulkan, and reflects the parameters of compiled code with their types, memory layouts, and bindings. |
| `slango` | A high-level library that runs Slang code on the GPU through wgpu, a Rust graphics library. It loads a Slang module, runs a compute kernel with its parameters set by name, reads the results back, and draws with a vertex and a fragment entry point into a texture. |
| `slango-macros` | The macro `shader!`, which compiles a Slang file while the calling crate builds and generates a typed Rust struct for each compute kernel and for each Slang structure that a kernel takes. A misspelled parameter or a value of the wrong type is then a compile error. |
| `slango-codegen` | The generator behind `slango-macros`, as a library. |
| `slang-slint` | A view for Slint, a Rust toolkit for graphical user interfaces. It shares Slint's GPU device with `slango` and shows what `slango` draws as an image in a Slint window. |

The repository also holds the examples, which are not published.

## Requirements

Building the workspace needs these tools and libraries:

- Rust 1.99 or later, as the `rust-version` field of the workspace manifest declares.
- A LunarG Vulkan software development kit (SDK), from <https://vulkan.lunarg.com>, with the environment variable `VULKAN_SDK` set to its directory. The SDK ships the Slang library that the crates link dynamically. The crates are tested with the Slang of SDK 1.4.363.1, and the Slang of another SDK is best effort.
- libclang, the C and C++ parser library of the LLVM project, because every build of `slang-compiler-sys` generates the raw bindings with it. On Linux, a package such as `libclang-dev` provides it. On macOS, the Xcode command line tools provide it, and on Windows, an installation of LLVM does. The environment variable `LIBCLANG_PATH` can name the directory that holds it.
- On Linux, the development files of fontconfig, such as the package `libfontconfig-dev`, because Slint links fontconfig.

The build of `slang-compiler-sys` stops with a message that says what to install and set when `VULKAN_SDK` is missing or libclang cannot be loaded.

A crate that calls the macro `shader!` needs the same SDK when it builds, because the macro compiles its Slang file then. On Windows, the compiler then also needs the SDK's `Bin` directory on `PATH`. The crate-level documentation of `slango-macros` says how the compiler finds the Slang library on each platform.

On Linux, `VULKAN_SDK` names the `x86_64` directory of the SDK. These commands set it and build the workspace, with a placeholder path:

```sh
export VULKAN_SDK=/tmp/vulkan-sdk/1.4.363.1/x86_64
cargo build --workspace
```

## Building Slang From Source

With the Cargo feature `vendored`, Slang is built from source and linked statically. The build then needs no Vulkan SDK, and the program needs no Slang shared library at run time. `slang-compiler`, `slango`, `slango-macros`, and `slang-slint` each have the feature and forward it down to `slang-compiler-sys`. The feature of `slang-compiler` also adds `slang-glslang-sys`, so a vendored build optimizes SPIR-V.

A program that calls the macro `shader!` enables `vendored` on both `slango` and `slango-macros`, because the macro's copy of Slang and the program's copy are built apart.

A build with the feature needs these tools:

- git and network access for the first build in a Cargo target directory, which clones the recorded Slang commit through `slang-src`.
- CMake 3.22 or later, Ninja, and a C++20 compiler. On Windows, the build finds the Microsoft C++ compiler of an installation of Visual Studio itself and needs no developer command prompt.
- Python 3 on `PATH`, as `python3` or `python`, for the build of slang-glslang.
- libclang, as for every build.

The first build takes several minutes. The clone and the built libraries are kept in the folder `slang-src` of the Cargo target directory, which every build of the project shares, so later builds take seconds. `cargo clean` removes the folder.

These limits hold:

- A program that enables `vendored` only on `slang-compiler-sys` gets SPIR-V that Slang does not optimize, unless it installs the loader of `slang-glslang-sys` itself.
- The vendored build with the optimizer is tested on the GitHub Actions runners for Linux, macOS, and Windows with the Microsoft toolchain. On Windows, it is tested only with the Visual Studio of the runner and in a target directory of usual depth.

The crate-level documentation of `slang-compiler-sys`, `slang-src`, and `slang-glslang-sys` says more. This command builds `slang-compiler` with the feature:

```sh
cargo build -p slang-compiler --features vendored
```

## Examples

The examples are binaries of the unpublished package `slang-rust-examples`, and they also serve as integration tests:

- `compile-and-reflect` compiles a Slang file and prints its entry points and parameters.
- `compute-kernel` runs a compute kernel on the GPU with `slango` and checks its results.
- `windowed-renderer` draws a rotating, textured quad in a window and reloads its shader whenever the file changes.
- `shader-playground` shows what a shader draws in a Slint window, with two sliders that set its parameters, and reloads the shader whenever the file changes.
- `typed-kernel` runs a compute kernel through the typed bindings that the macro `shader!` generates and checks its results.

Every example needs the Slang library of the Vulkan SDK on the library path, which is `LD_LIBRARY_PATH` on Linux and `DYLD_LIBRARY_PATH` on macOS. Every example except `compile-and-reflect` also needs a GPU driver or a software driver, such as lavapipe on Linux. `windowed-renderer` and `shader-playground` need a display, and `shader-playground` on a software driver needs the environment variable `SLINT_WGPU_CPU` set to `1`.

These commands run the examples from the workspace root. The first four take the path of a sample shader. `typed-kernel` takes no argument, because the macro embeds its shader when it builds:

```sh
cargo run -p slang-rust-examples --bin compile-and-reflect -- examples/shaders/scale.slang
cargo run -p slang-rust-examples --bin compute-kernel -- examples/shaders/saxpy.slang
cargo run -p slang-rust-examples --bin windowed-renderer -- examples/shaders/textured-quad.slang
cargo run -p slang-rust-examples --bin shader-playground -- examples/shaders/playground.slang
cargo run -p slang-rust-examples --bin typed-kernel
```

## Development

Development happens in the development container that the sibling repository slang-rs-env defines. The container provides the Rust toolchains, the check tools, and the Vulkan SDK.

Every command is a recipe of `just`, a command runner. This command lists the recipes:

```sh
just --list
```

This command runs every check:

```sh
just check
```

[`AGENTS.md`](https://github.com/g-whiz/slang-rust/blob/main/AGENTS.md) holds the rules for working in the repository and the recipe to run for each kind of change. These documents are under `docs/`:

- [Architecture design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/architecture.md): The crates, how Slang is obtained and which Slang is recorded, and the roadmap.
- [Tooling design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/tooling.md): The tool configuration, the recipes, the hooks, the custom checkers, and the continuous integration.
- [Design of `slang-compiler-sys`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler-sys.md): The raw bindings, the binding generator `slang-bindgen`, the build information, and the build script.
- [Design of `slang-compiler`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler.md): The safe compile and reflection API and the example `compile-and-reflect`.
- [Design of `slango`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slango.md): Compute kernels and render pipelines on wgpu, and the examples `compute-kernel` and `windowed-renderer`.
- [Design of `slang-slint`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-slint.md): The Slint view and the example `shader-playground`.
- [Design of `slango-macros`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slango-macros.md): The macro `shader!`, its generator `slango-codegen`, and the example `typed-kernel`.
- [Rust coding standards](https://github.com/g-whiz/slang-rust/blob/main/docs/rust-coding-standards.md): The rules for Rust code.
- [Writing guide](https://github.com/g-whiz/slang-rust/blob/main/docs/writing-documentation.md): The rules for documentation.
- [Reference list](https://github.com/g-whiz/slang-rust/blob/main/docs/rust-references.md): The external sources behind the coding standards.

## Licence

Each crate has its own licence, given here as an SPDX licence expression. Each crate's package holds the text of its licence.

| Crate | Licence | Licence text |
| --- | --- | --- |
| `slang-compiler-sys`, `slang-src`, and `slang-glslang-sys` | `Apache-2.0 WITH LLVM-exception`, the licence of Slang | [`LICENSE-APACHE-LLVM`](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE-LLVM) |
| `slang-bindgen`, `slang-compiler`, `slango`, `slango-macros`, and `slango-codegen` | `MIT OR Apache-2.0`, at your option | [`LICENSE-MIT`](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-MIT) and [`LICENSE-APACHE`](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-APACHE) |
| `slang-slint` | `GPL-3.0-or-later` | [`LICENSE-GPL`](https://github.com/g-whiz/slang-rust/blob/main/LICENSE-GPL) |

The source of the examples is also under `MIT OR Apache-2.0`.

`slang-slint` depends on Slint, which is available under several licences, and takes it under version 3 of the GNU General Public License only, with the SPDX identifier `GPL-3.0-only`. A program that uses `slang-slint` is therefore distributed under version 3 of the GNU General Public License only. The example `shader-playground` uses Slint too, so the program built from it is under version 3 of the GNU General Public License only. The other crates do not depend on Slint, so this does not apply to them.
