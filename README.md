# slang-rust

slang-rust is a planned set of Rust crates for [Slang](https://shader-slang.org), a language and compiler for writing shaders. A shader is a small program that runs on a graphics processing unit (GPU), the chip that draws images and runs many computations at once. The crates are planned in three layers:

- Raw bindings to the native Slang library. Bindings are Rust declarations that let Rust code call a library written in another language.
- A safe Rust API for compiling Slang code and for reading information about it, such as the types and parameters it declares.
- A high-level library for running Slang code on a GPU from Rust.

The source code and the documents that this README names are in the repository at <https://github.com/g-whiz/slang-rust>.

## Status

The project is at a very early stage. No crate is published. The workspace holds seven crates and the examples. `slang-compiler` is the safe API, and its compile and reflection API exists. It creates a global session, the root object of every Slang compilation, and checks that the loaded Slang library is the pinned version. It creates a session, the context of a compilation, with its output formats, search paths, preprocessor macros, and compiler options. It loads Slang modules, finds their entry points, which are the functions where a shader starts, combines and links them, and generates code for each of its nine output formats, such as SPIR-V, the binary shader format of Vulkan. Its reflection lists the parameters of compiled code with their types, memory layouts, and bindings, the numbered slots through which a shader reaches its resources. Slang is written in C++, and a shader whose types nest more than 128 levels deep can make Slang throw a C++ exception that it does not catch, so `slang-compiler` then stops the program instead of returning an error, and prints a message first unless the program is built with `panic = "abort"`. Section 11.6 of the [design of `slang-compiler`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler.md) explains why. `slang-compiler-sys` holds the raw bindings, which the binding generator writes from the Slang headers, and records the pinned version of Slang. Its build script links the Slang library of an installed Vulkan SDK, after checking that it is the pinned version, or builds Slang from source and links it statically. The binding generator is the third crate, and a test checks that the bindings in the repository equal what it writes. The fourth crate, `slango`, is the high-level library. It can create a GPU device, load a Slang module, create buffers on the device, run a compute kernel with its parameters set by name, and read the results back. It can also create textures and samplers, and draw with a vertex and a fragment entry point of a module into a texture, with the parameters set by name. The fifth crate, `slang-slint`, connects `slango` to Slint. It makes Slint, a Rust toolkit for graphical user interfaces, create a GPU device that `slango` can use, and gives `slango` that device. Its view draws with `slango` into a texture that Slint shows as an image inside a Slint window. The sixth crate, `slango-macros`, holds the macro `shader!`, which compiles a Slang file while the crate that calls it builds and generates a typed Rust struct for each compute kernel, with one field of the matching Rust type for each parameter, so a misspelled parameter or a value of the wrong type is a compile error. It supports plain values, buffers of the built-in element types of `slango`, textures, and samplers. It also generates a Rust struct for each Slang structure that a kernel takes, with the memory layout that Slang gives it. Building a crate that calls it needs the pinned Slang. The seventh crate, `slango-codegen`, is the generator behind the macro. The repository also holds the design documents, the coding and writing standards, and the tooling that checks every change.

## Planned Crates

No crate in this table is published. Each crate gets a link to its documentation when it is published.

| Crate | State | Purpose |
| --- | --- | --- |
| `slang-compiler-sys` | Built and not published, with the raw bindings, the pinned Slang version, the linking of the Slang of an installed Vulkan SDK, and the static build of Slang from source | Raw bindings to the native Slang library, generated from the Slang headers |
| `slang-compiler` | Built and not published: it compiles shaders, with sessions, compiler options, modules, linking, and code generation, and reflects their parameters. A custom file system is deferred until a concrete need appears | Safe, idiomatic wrappers over `slang-compiler-sys`, with no GPU dependency |
| `slango` | Built for compute kernels and drawing into a texture, and not published: it can create a GPU device, load a Slang module with its kernels, create and read buffers and textures, run a kernel, and draw with a render pipeline | A high-level library for calling Slang code on the GPU, with wgpu, a Rust graphics library, as its first backend |
| `slango-macros` | Built for compute kernels and the Slang structures they take, and not published, with the macro `shader!` | A build-time macro that generates typed Rust bindings to Slang code |
| `slango-codegen` | Built and not published, with the generator that `slango-macros` calls | The generator of `slango-macros`, as a library |
| `slang-slint` | Built and not published: it shares Slint's GPU device with `slango` and shows what `slango` draws as an image in a Slint window | A view for Slint, a Rust toolkit for graphical user interfaces, that a Slang render pipeline draws, built on `slango` and wgpu |

The repository also holds the binding generator, which writes the raw bindings, and the examples, which also serve as integration tests. The first example, `compile-and-reflect`, compiles a Slang file and prints its entry points and parameters. The second, `compute-kernel`, runs a compute kernel on the GPU with `slango` and checks its results. The third, `windowed-renderer`, draws a rotating, textured quad in a window and reloads its shader whenever the file changes. The fourth, `shader-playground`, shows what a shader draws in a Slint window with two sliders that set its parameters, and also reloads the shader whenever the file changes. The fifth, `typed-kernel`, runs a compute kernel on the GPU through the typed bindings that the macro `shader!` generates, including a struct for a Slang structure, and checks its results. Neither the generator nor the examples are published.

## Obtaining Slang

The crates obtain the native Slang library in one of two ways. By default, the build uses the Slang that ships in an installed LunarG Vulkan software development kit (SDK) and links it dynamically. Vulkan is a graphics programming interface, and the SDK is a package of Vulkan tools and libraries. As an opt-in, the build compiles Slang from source and links it statically, so the program needs no Slang library at run time. There is no prebuilt download.

To select the static build, set the environment variable `SLANG_SOURCE_DIR` to the absolute path of the top-level directory of a git clone of Slang at the pinned commit, with its submodules and without local changes to its tracked files. The build checks the commit and the changes with git, and it refuses a source archive without git metadata. It needs git, CMake 3.22 or later, Ninja, and a C++ compiler, it downloads nothing, and it leaves the clone unchanged. The first build takes several minutes. The static build has been tested on x86_64 Linux only. Section 9.3 of the [design of `slang-compiler-sys`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler-sys.md) gives the details, including the supported targets and what the static build leaves out. Sections 4 and 5 of the [architecture design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/architecture.md) define both routes and the pinned Slang version.

## Requirements

The workspace needs Rust 1.99 or later, as the `rust-version` field of its manifest declares.

Building the workspace also needs the LunarG Vulkan SDK 1.4.363.1, from <https://vulkan.lunarg.com>, with the environment variable `VULKAN_SDK` set to its directory, unless `SLANG_SOURCE_DIR` selects the static build. The build of `slang-compiler-sys` stops with a message that says what to install and set when the variable is missing or the SDK ships another Slang build. Sections 9 and 10 of the [design of `slang-compiler-sys`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler-sys.md) give the details, including how a program finds the Slang shared library when it runs.

On Linux, building the workspace also needs the development files of fontconfig, such as the package `libfontconfig-dev`, because Slint links fontconfig.

## Development

Development happens in the development container that the sibling repository slang-rs-env defines. The container provides the Rust toolchains, the check tools, and the Vulkan SDK. Every command is a recipe of `just`, a command runner.

This command lists the recipes:

```sh
just --list
```

This command runs every check, as is required before every merge:

```sh
just check
```

These commands, run from the workspace root with the Slang library of the Vulkan SDK on the library path, run the five examples. The first four run a sample shader that their argument names, and the fifth runs a shader that the macro `shader!` embeds when it builds, so it takes no argument. Every example except the first also needs a GPU driver or a software driver, such as lavapipe on Linux, and the third and fourth need a display. On a software driver, the fourth also needs the environment variable `SLINT_WGPU_CPU` set to `1`:

```sh
cargo run -p slang-rust-examples --bin compile-and-reflect -- examples/shaders/scale.slang
cargo run -p slang-rust-examples --bin compute-kernel -- examples/shaders/saxpy.slang
cargo run -p slang-rust-examples --bin windowed-renderer -- examples/shaders/textured-quad.slang
cargo run -p slang-rust-examples --bin shader-playground -- examples/shaders/playground.slang
cargo run -p slang-rust-examples --bin typed-kernel
```

[`AGENTS.md`](https://github.com/g-whiz/slang-rust/blob/main/AGENTS.md) holds the rules for working in the repository, the project status, and the recipe to run for each kind of change. These documents are under `docs/`:

- [Architecture design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/architecture.md): The crates, how Slang is obtained and pinned, and the roadmap.
- [Tooling design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/tooling.md): The tool configuration, the recipes, the hooks, the custom checkers, and the continuous integration.
- [Design of `slang-compiler-sys`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler-sys.md): The raw bindings, the binding generator, the pin, and the build script.
- [Design of `slang-compiler`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-compiler.md): The safe compile and reflection API and the compile-and-reflect example, without the deferred custom file system.
- [Design of `slango`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slango.md): `slango`, which runs the compute entry points of a Slang module on wgpu and draws with its vertex and fragment entry points, and the compute kernel example.
- [Design of `slang-slint`](https://github.com/g-whiz/slang-rust/blob/main/docs/design/slang-slint.md): The view that shows what a `slango` render pipeline draws in a Slint user interface, and the Slint example.
- [Rust coding standards](https://github.com/g-whiz/slang-rust/blob/main/docs/rust-coding-standards.md): The rules for Rust code.
- [Writing guide](https://github.com/g-whiz/slang-rust/blob/main/docs/writing-documentation.md): The rules for documentation.
- [Reference list](https://github.com/g-whiz/slang-rust/blob/main/docs/rust-references.md): The external sources behind the coding standards.

## Licence

slang-rust is licensed under the GNU General Public License, version 3 or later, with the SPDX identifier `GPL-3.0-or-later`. The file [`LICENSE`](https://github.com/g-whiz/slang-rust/blob/main/LICENSE) in the repository holds the text of the licence.

`slang-slint` depends on Slint, which is available under several licences, and takes it under version 3 of the GNU General Public License only, with the SPDX identifier `GPL-3.0-only`. A program that uses `slang-slint` is therefore distributed under version 3 of the GNU General Public License only. `slang-compiler-sys`, `slang-compiler`, and `slango` do not depend on Slint, so this does not apply to them.
