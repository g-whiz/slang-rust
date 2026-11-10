# slang-rust

slang-rust is a planned set of Rust crates for [Slang](https://shader-slang.org), a language and compiler for writing shaders. A shader is a small program that runs on a graphics processing unit (GPU), the chip that draws images and runs many computations at once. The crates are planned in three layers:

- Raw bindings to the native Slang library. Bindings are Rust declarations that let Rust code call a library written in another language.
- A safe Rust API for compiling Slang code and for reading information about it, such as the types and parameters it declares.
- A high-level library for running Slang code on a GPU from Rust.

The source code and the documents that this README names are in the repository at <https://github.com/g-whiz/slang-rust>.

## Status

The project is at a very early stage, and no crate is usable yet. The workspace holds one crate, `slang-compiler-sys`. It is a placeholder with documentation and no API. It marks the place of the raw bindings, which milestone M1 of the roadmap adds. The repository also holds the design documents, the coding and writing standards, and the tooling that checks every change.

## Planned Crates

Every crate in this table is planned. Each crate gets a link to its documentation when it has an API.

| Crate | State | Purpose |
| --- | --- | --- |
| `slang-compiler-sys` | Planned, placeholder present | Raw bindings to the native Slang library, generated from the Slang headers |
| `slang-compiler` | Planned | Safe, idiomatic wrappers over `slang-compiler-sys`, with no GPU dependency |
| `slango` | Planned | A high-level library for calling Slang code on the GPU, with wgpu, a Rust graphics library, as its first backend |
| `slango-macros` | Planned | A build-time macro that generates typed Rust bindings to Slang code |
| `slang-slint` | Planned | A view for Slint, a Rust toolkit for graphical user interfaces, that a Slang render pipeline draws, built on `slango` and wgpu |

The repository will also hold a binding generator, which writes the raw bindings, and examples that also serve as integration tests. Neither will be published.

## Obtaining Slang

The crates will obtain the native Slang library in one of two ways. By default, the build will use the Slang that ships in an installed LunarG Vulkan software development kit (SDK) and link it dynamically. Vulkan is a graphics programming interface, and the SDK is a package of Vulkan tools and libraries. As an opt-in, the build will compile Slang from a source tree and link it statically. There will be no prebuilt download. Sections 4 and 5 of the [architecture design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/architecture.md) define both routes and the pinned Slang version.

## Requirements

The workspace needs Rust 1.99 or later, as the `rust-version` field of its manifest declares.

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

[`AGENTS.md`](https://github.com/g-whiz/slang-rust/blob/main/AGENTS.md) holds the rules for working in the repository, the project status, and the recipe to run for each kind of change. These documents are under `docs/`:

- [Architecture design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/architecture.md): The crates, how Slang is obtained and pinned, and the roadmap.
- [Tooling design](https://github.com/g-whiz/slang-rust/blob/main/docs/design/tooling.md): The tool configuration, the recipes, the hooks, the custom checkers, and the continuous integration.
- [Rust coding standards](https://github.com/g-whiz/slang-rust/blob/main/docs/rust-coding-standards.md): The rules for Rust code.
- [Writing guide](https://github.com/g-whiz/slang-rust/blob/main/docs/writing-documentation.md): The rules for documentation.
- [Reference list](https://github.com/g-whiz/slang-rust/blob/main/docs/rust-references.md): The external sources behind the coding standards.

## Licence

slang-rust is licensed under the GNU General Public License, version 3 or later, with the SPDX identifier `GPL-3.0-or-later`. The file [`LICENSE`](https://github.com/g-whiz/slang-rust/blob/main/LICENSE) in the repository holds the text of the licence.
