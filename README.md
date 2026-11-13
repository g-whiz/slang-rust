# slang-rust

slang-rust is a set of Rust crates for [Slang](https://shader-slang.org), a language and compiler for writing shaders. A shader is a small program that runs on a graphics processing unit (GPU), the chip that draws images and runs many computations at once.

The crates let a Rust program compile Slang code, read the parameters it declares, and run it on a GPU. The source code and the documents that this README names are in the repository at <https://github.com/g-whiz/slang-rust>.

## Crates

The workspace holds six crates. No crate is published yet.

| Crate | Purpose |
| --- | --- |
| `slang-compiler-sys` | Raw bindings to the native Slang library, generated from the Slang headers. Bindings are Rust declarations that let Rust code call a library written in another language. |
| `slang-compiler` | Safe, idiomatic wrappers over `slang-compiler-sys`, with no GPU dependency. It compiles Slang code to output formats such as SPIR-V, the binary shader format of Vulkan, and reflects the parameters of compiled code with their types, memory layouts, and bindings. |
| `slango` | A high-level library that runs Slang code on the GPU through wgpu, a Rust graphics library. It loads a Slang module, runs a compute kernel with its parameters set by name, reads the results back, and draws with a vertex and a fragment entry point into a texture. |
| `slango-macros` | The macro `shader!`, which compiles a Slang file while the calling crate builds and generates a typed Rust struct for each compute kernel and for each Slang structure that a kernel takes. A misspelled parameter or a value of the wrong type is then a compile error. |
| `slango-codegen` | The generator behind `slango-macros`, as a library. |
| `slang-slint` | A view for Slint, a Rust toolkit for graphical user interfaces. It shares Slint's GPU device with `slango` and shows what `slango` draws as an image in a Slint window. |

The repository also holds the binding generator, which writes the raw bindings, and the examples. Neither is published.

## Requirements

Building the workspace needs these tools and libraries:

- Rust 1.99 or later, as the `rust-version` field of the workspace manifest declares.
- The LunarG Vulkan software development kit (SDK) 1.4.363.1, from <https://vulkan.lunarg.com>, with the environment variable `VULKAN_SDK` set to its directory. The SDK ships the Slang library that the crates link dynamically.
- On Linux, the development files of fontconfig, such as the package `libfontconfig-dev`, because Slint links fontconfig.

The build of `slang-compiler-sys` stops with a message that says what to install and set when `VULKAN_SDK` is missing or the SDK ships another Slang build.

A crate that calls the macro `shader!` needs the same SDK when it builds, because the macro compiles its Slang file then. On Windows, the compiler then also needs the SDK's `Bin` directory on `PATH`. The crate-level documentation of `slango-macros` says how the compiler finds the Slang library on each platform, and what the macro generates when the documentation builds without Slang, as on docs.rs.

On Linux, `VULKAN_SDK` names the `x86_64` directory of the SDK. These commands set it and build the workspace, with a placeholder path:

```sh
export VULKAN_SDK=/tmp/vulkan-sdk/1.4.363.1/x86_64
cargo build --workspace
```

## Building Slang from Source

As an alternative to the Vulkan SDK, the build can compile Slang from source and link it statically, so the program needs no Slang library at run time. There is no prebuilt download.

To select this build, set the environment variable `SLANG_SOURCE_DIR` to the absolute path of a git clone of Slang at the pinned commit. The clone needs its submodules and no local changes to its tracked files, and the build refuses a source archive without git metadata.

This build needs git, CMake 3.22 or later, Ninja, and a C++ compiler. It downloads nothing and leaves the clone unchanged, and the first build takes several minutes. Section 9.3 of the [design of `slang-compiler-sys`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler-sys.md) gives the supported targets and what the build leaves out.

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

- [Architecture design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/architecture.md): The crates, how Slang is obtained and pinned, and the roadmap.
- [Tooling design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/tooling.md): The tool configuration, the recipes, the hooks, the custom checkers, and the continuous integration.
- [Design of `slang-compiler-sys`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler-sys.md): The raw bindings, the binding generator, the pin, and the build script.
- [Design of `slang-compiler`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler.md): The safe compile and reflection API and the example `compile-and-reflect`.
- [Design of `slango`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slango.md): Compute kernels and render pipelines on wgpu, and the examples `compute-kernel` and `windowed-renderer`.
- [Design of `slang-slint`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-slint.md): The Slint view and the example `shader-playground`.
- [Design of `slango-macros`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slango-macros.md): The macro `shader!`, its generator `slango-codegen`, and the example `typed-kernel`.
- [Rust coding standards](https://github.com/g-whiz/slang-rust/blob/main/docs/rust-coding-standards.md): The rules for Rust code.
- [Writing guide](https://github.com/g-whiz/slang-rust/blob/main/docs/writing-documentation.md): The rules for documentation.
- [Reference list](https://github.com/g-whiz/slang-rust/blob/main/docs/rust-references.md): The external sources behind the coding standards.

## Licence

slang-rust is licensed under the GNU General Public License, version 3 or later, with the SPDX identifier `GPL-3.0-or-later`. The file [`LICENSE`](https://github.com/g-whiz/slang-rust/blob/main/LICENSE) in the repository holds the text of the licence.

`slang-slint` depends on Slint, which is available under several licences, and takes it under version 3 of the GNU General Public License only, with the SPDX identifier `GPL-3.0-only`. A program that uses `slang-slint` is therefore distributed under version 3 of the GNU General Public License only. The other five crates do not depend on Slint, so this does not apply to them.
