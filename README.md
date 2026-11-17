# slang-rust

slang-rust is a set of Rust crates for [Slang](https://shader-slang.org), a language and compiler for writing shaders. A shader is a small program that runs on a graphics processing unit (GPU), the chip that draws images and runs many computations at once.

The crates let a Rust program compile Slang code, read the parameters it declares, and run it on a GPU. The source code and the documents that this README names are in the repository at <https://github.com/g-whiz/slang-rust>.

## Stability

Every release of these crates is an alpha release, with a version such as `0.1.0-alpha.1`. The API is unstable and may change in any release, including from one alpha to the next.

Cargo matches a pre-release version only for a requirement that names a pre-release, so a requirement such as `"0.1"` matches no alpha release. A dependent names the full pre-release version, as in this example:

```toml
[dependencies]
slango = "0.1.0-alpha.1"
```

Such a requirement also accepts a later alpha release, which may break the dependent. A dependent that needs one fixed API writes the exact requirement `"=0.1.0-alpha.1"`.

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

The build of `slang-compiler-sys` stops with a message that says what to install and set when `VULKAN_SDK` is missing or libclang cannot be loaded. A program that uses the crates can instead build Slang from source and then needs no Vulkan SDK, as [Building Slang From Source](#building-slang-from-source) describes.

A crate that calls the macro `shader!` needs the same SDK when it builds, because the macro compiles its Slang file then, unless it enables the feature `vendored` on `slango-macros`. On Windows, the compiler then also needs the SDK's `Bin` directory on `PATH`. The crate-level documentation of `slango-macros` says how the compiler finds the Slang library on each platform.

On Linux, `VULKAN_SDK` names the `x86_64` directory of the SDK. These commands set it and build the workspace, with a placeholder path:

```sh
export VULKAN_SDK=/tmp/vulkan-sdk/1.4.363.1/x86_64
cargo build --workspace
```

## Building Slang From Source

The crates obtain Slang in one of two ways, and the Cargo feature `vendored` chooses between them:

| | Default | With the feature `vendored` |
| --- | --- | --- |
| Which Slang | The Slang shared library of an installed LunarG Vulkan SDK, at `VULKAN_SDK` | A Slang that the build compiles from source |
| How it is linked | Dynamically | Statically |
| Needs at build time | The Vulkan SDK and libclang | The tools that [What the Build Needs](#what-the-build-needs) lists, and no Vulkan SDK |
| Needs at run time | The Slang shared library on the library path | No Slang library |
| First build | As fast as any crate | Minutes, as [What It Costs](#what-it-costs) shows |

Only the feature selects the way, and no environment variable does. The build never falls back from one way to the other, so a build with the feature never links the Slang of a Vulkan SDK, and a build without it never builds Slang.

### Enabling the Feature

Enable `vendored` on the crate that your program depends on: `slang-compiler`, `slango`, or `slang-slint`. Each forwards the feature down to `slang-compiler-sys`, which builds Slang, and the feature of `slang-compiler` also adds `slang-glslang-sys`, which builds the SPIR-V optimizer of Slang. This example is for a program that uses `slango`:

```toml
[dependencies]
slango = { version = "0.1.0-alpha.1", features = ["vendored"] }
```

A program that calls the macro `shader!` enables the feature on `slango-macros` as well:

```toml
[dependencies]
slango = { version = "0.1.0-alpha.1", features = ["vendored"] }
slango-macros = { version = "0.1.0-alpha.1", features = ["vendored"] }
```

The macro runs on the build machine, and Cargo resolves the features of a procedural macro apart from those of the program. The macro and the program therefore each link their own copy of Slang, and each copy needs the feature. On a machine without `VULKAN_SDK`, a build with the feature on only one of the two crates stops with the error that starts "VULKAN_SDK is not set". Both copies are built from one source tree and share one build, as [The Shared Folder](#the-shared-folder) describes.

A program that uses `slang-compiler-sys` directly and enables `vendored` only there gets SPIR-V that Slang does not optimize. It has to install the loader of `slang-glslang-sys` on each global session itself, as the crate documentation of `slang-glslang-sys` shows. `slang-compiler` installs that loader on every global session that it creates, so a program that uses `slang-compiler` or a crate above it gets optimized SPIR-V, as with the Slang of a Vulkan SDK.

### What the Build Needs

A build with the feature needs these on the build machine:

- git, and network access for the first build in a Cargo target directory, which clones Slang.
- CMake 3.22 or later, and Ninja.
- A C++20 compiler. On Windows, that is an installation of Visual Studio or its Build Tools with the C++ tools. The build finds the Microsoft C++ compiler `cl.exe` by itself and needs no developer command prompt.
- Python 3 on `PATH`, as `python3` or `python`, for the build of the SPIR-V optimizer.
- libclang, which every build needs, with or without the feature, as [Requirements](#requirements) describes.

It does not need a Vulkan SDK or `VULKAN_SDK` at build time, and the program does not need a Slang library or a library path at run time.

### What It Costs

The first build in a Cargo target directory clones Slang and builds Slang and its SPIR-V optimizer from source. Later builds in that target directory reuse the clone and the built libraries and need no network access. These figures were measured and serve only as examples:

- First build: about 6 minutes for a small program with `slango` and `slango-macros` on a 16-core x86_64 Linux machine. Between about 4 and 16 minutes on GitHub's hosted runners, depending on the platform.
- Disk space of the shared folder: about 350 MB on Linux and macOS, and about 760 MB on Windows.
- Size of a release binary: tens of megabytes more than with the dynamic link. The release binary of a small program with `slango` and the macro was 43 MB on x86_64 Linux.

### The Shared Folder

The clone and the built libraries are kept in the shared folder, the directory `slang-src` in the Cargo target directory, or in the build directory when Cargo uses a separate one:

- Every profile of a project, and both its host units and its target units, use the one shared folder. A procedural macro and the program therefore clone Slang once and build it once.
- `cargo clean` removes the folder, and the next build clones and builds again. Removing the folder is safe whenever no build runs.
- Two projects share the folder only when they share a Cargo target directory, such as through `CARGO_TARGET_DIR`.
- Nothing removes old content of the folder, so it grows with each Slang commit and target that a project builds with.

The build finds the folder through the file `CACHEDIR.TAG` that Cargo writes into a target directory that it creates. It takes the nearest directory above its output directory that holds that file. When that directory is named after the target triple, it takes the parent directory instead if that holds the file too. When it finds no such directory, it uses its output directory, which is correct and loses only the sharing: each unit then clones and builds on its own. Cargo writes no such file in these cases:

- A target directory that existed before the first build, such as one created with `mkdir`, restored from a cache of a continuous integration service, or mounted as a volume.
- A target directory whose `CACHEDIR.TAG` was deleted.
- The temporary target directory of `cargo install`.

The crate-level documentation of `slang-src` describes the folder in full.

### Which Slang

A build with the feature builds one recorded Slang commit, `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`, whose build tag is `2026.17-25-gc7954ebd1`. It is the Slang of Vulkan SDK 1.4.363.1, which the crates are tested with. The version of `slang-src` names it, such as `0.1.0-alpha.1.slang.2026.17.0.25.gc7954ebd1`.

The only way to build another Slang commit is Cargo's `[patch]` table with a fork of `slang-src` that records that commit. This is best effort and unsupported. The crate-level documentation of `slang-src` gives the steps.

The constant `slang_compiler_sys::BUILD_INFO` tells a program which Slang its build got:

- `slang_build_tag`: the build tag of the Slang that the build links.
- `slang_commit`: the full commit hash with the feature, and `None` with the Slang of a Vulkan SDK.
- `linkage`: `Linkage::Static` with the feature, and `Linkage::Dynamic` without it.
- `tested_target`: whether the target is one of four that the build script lists as tested, which are `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, and `x86_64-apple-darwin`.
- `version`: one string that names all of it, such as `0.1.0-alpha.1+slang.2026.17.0.25.gc7954ebd1.vendored`.

### Platforms

A build with the feature is tested on x86_64 Linux, on x86_64 Windows with the Microsoft toolchain, and on arm64 macOS, on GitHub's hosted runners. On Windows, it is tested only with the Visual Studio of the runner and in a target directory of usual depth. A program outside this repository that enables the feature on `slango` and `slango-macros` has been built and run on x86_64 Linux only.

The build stops in these cases, because a correct build is not possible:

- The target differs from the host. Slang's build runs programs that it builds for the target, so it cannot cross-compile.
- The target has no rule to link its C++ runtime library. Rules exist for Linux with the GNU C library, macOS, and Windows with the Microsoft toolchain.
- The pointers of the target are not 64 bits wide.

Every other target builds with a warning that it is untested and unsupported.

### Common Failures

| The error starts with | Cause | What to do |
| --- | --- | --- |
| "VULKAN_SDK is not set" | A crate of the build links the Slang of a Vulkan SDK, because the feature is off, or on only one of `slango` and `slango-macros` | Enable the feature as [Enabling the Feature](#enabling-the-feature) shows, or install a Vulkan SDK and set `VULKAN_SDK` |
| "cannot start" | A program that the build runs is missing: git, CMake, Ninja, or the C++ compiler | Install the program that the message names and put it on `PATH` |
| "cannot find the Microsoft C++ compiler cl.exe" | No installation of Visual Studio with the C++ tools was found | Install Visual Studio or its Build Tools with the workload for desktop development with C++ |
| The failed command, followed by "failed with" and the step that failed | CMake or Ninja failed to configure or build Slang or its SPIR-V optimizer | Read the log `build.log` that the message names. The message also holds its last 40 lines |
| A git command, followed by "ended with" and "while cloning Slang" | The first build in a target directory cannot reach the network | Build once with network access, or build without the feature |

With `CARGO_NET_OFFLINE` set to `true`, a first build stops with an error that says that it needs to clone Slang. A build with an existing clone in the shared folder needs no network access.

In this repository, this command builds `slang-compiler` with the feature:

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
