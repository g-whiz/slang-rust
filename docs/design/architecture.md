# Architecture Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-15 |
| Applies to | The whole workspace |

## Abstract

This document specifies the architecture of slang-rust, a set of Rust crates for using the Slang shader language and its compiler. It defines the crates, the direction of the dependencies between them, and what each crate is responsible for. It also fixes how the native Slang library is obtained, which Slang version the workspace records and tests, and sets out the roadmap of milestones M0 to M18.

## 1. Introduction

### 1.1. Problem

Slang is a shader language and compiler with a C++ application programming interface (API) in the style of the Component Object Model (COM). Rust code that uses it has to call that API through raw pointers and vtables, manage reference counts by hand, and keep each object alive for as long as any object created from it lives.

It then has to load the compiled code into a graphics API and pass arguments to it, which takes many more steps. The one existing third-party Rust binding writes its vtables by hand and has a reported soundness issue in which objects crash when they outlive their session [SLANG-RS-26].

### 1.2. Purpose

The workspace provides a layered set of crates. The lowest layer binds the native library exactly and safely enough to build on. The middle layer gives a safe, idiomatic compiler API.

The upper layers make calling Slang code from Rust as simple as SlangPy makes it from Python, and let a Slint user interface show content that a Slang render pipeline draws. This document fixes the structure that all of these crates share, so that each crate's own design document can be written against it.

### 1.3. Audience

The primary readers are the coding agents that implement a milestone or write a per-crate design document. The secondary readers are the reviewers who check that a change stays within this architecture, and the owner of the repository, who approves its decisions.

### 1.4. Scope

This document covers the crates and their layering, how Slang is obtained and which version is recorded, the responsibilities and limits of each crate, the examples, the test tiers, platform support, publishing, the roadmap, the assumptions to confirm, and the open questions. It applies to every crate, tool, and example in the workspace.

### 1.5. Non-Goals

This document does not specify the public API of any crate. Each crate's design document does that, as Section 3.4 requires.

It does not specify the workspace configuration, the lint configuration, the `just` recipes, the nextest profiles, the continuous integration (CI) workflows, or the requirements on the development container image. The document `docs/design/tooling.md` owns those topics.

It does not design the slang-rhi crates, the slang-rhi backend of `slango`, the new pipeline kinds, or the path-tracing demo. The milestone of Section 15 that delivers each one designs it. It does not restate the rules of `docs/rust-coding-standards.md`, which it cites by rule identifier.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that Section 2.2 of `docs/writing-documentation.md` defines, such as agent, role, user, and design document.

It also uses the terms that Section 2.2 of `docs/rust-coding-standards.md` defines, such as binding generator, `-sys` crate, safe crate, library crate, native library, interface, vtable, thunk, interface pointer, result code, blob, diagnostics blob, and universally unique identifier (UUID). This document calls those two files the writing guide and the coding standard. It adds these terms:

- acceleration structure: A GPU data structure over geometry that ray tracing traverses to find the geometry that a ray hits.
- adapter: The wgpu name for one implementation of a graphics API that a program can open a device on, such as a hardware GPU or a software implementation such as lavapipe.
- application binary interface (ABI): The layout of types and the calling conventions that compiled code relies on. A library whose ABI is not stable can change these between versions without a change in its source API.
- automatic differentiation: A Slang feature that derives the derivative of a function from its source code.
- best effort: The support that the crates give a Slang version other than the recorded Slang: a build may work, but nothing tests it, and the crate documentation states it as unsupported (Section 5.3).
- broadcasting: Calling a function written for single values over arrays of values, where an argument with fewer dimensions is repeated across the others, as in NumPy.
- buffer: A block of GPU memory that holds data for a shader.
- build tag: The version string of one Slang build, such as `2026.17-25-gc7954ebd1`, which is the output of `git describe` on the Slang commit: the nearest tag, the number of commits after it, and `g` followed by the abbreviated commit hash.
- build information: The constant of `slang-compiler-sys` that records the build tag of the Slang that a build generated its bindings from and links, and whether it links it dynamically or statically (Section 6.1).
- CMake: A build system generator for C and C++ projects. Slang builds with it.
- COM-style interface: An interface in the sense of Section 2.2 of the coding standard. COM is a binary convention for objects in which every object counts its references through `AddRef` and `Release`, and a caller selects one of the object's interfaces through `QueryInterface` with a UUID.
- component type: Slang's name for a unit of code that can be composed with others and linked, such as a module or an entry point.
- compute dispatch: One GPU command that runs a compute shader over a grid of threads.
- compute kernel: A function that runs on the GPU in many threads at once, started by a compute dispatch.
- cooperative vector: A vector whose operations, such as a matrix-vector product, the threads of a group carry out together on hardware for machine learning. Slang names its type `CoopVec`.
- crates.io: The public registry of Rust crates.
- CUDA: NVIDIA's platform and API for computing on its GPUs.
- device: An open connection to an adapter, through which a program creates GPU resources and submits work.
- Direct3D 12 (D3D12): The graphics API of Windows.
- docs.rs: The service that builds and hosts the API documentation of every crate published on crates.io.
- dynamic route: The default way of obtaining Slang, which generates the bindings from the headers of an installed Vulkan SDK and links its Slang dynamically (Section 4.2).
- entry point: A function in Slang source that a pipeline starts, such as the main function of a compute shader.
- geometry stage: The optional stage of a render pipeline that runs once per primitive and can emit new primitives.
- GitHub Pages: GitHub's service that hosts static web pages from a repository.
- global session: The Slang object that holds the shared state of the compiler, from which sessions are created.
- graphics processing unit (GPU): A processor for parallel graphics and compute work.
- headless: Running without a window or a display.
- hit group: The set of shaders, such as a closest-hit shader and an any-hit shader, that a ray tracing pipeline runs when a ray hits one kind of geometry.
- hot reload: Recompiling a shader and replacing it in the running program when its source file changes.
- hull and domain stages: The stages of a render pipeline that control and evaluate tessellation, which subdivides primitives on the GPU. Vulkan calls them the tessellation control and tessellation evaluation stages.
- lavapipe: The software implementation of Vulkan in the Mesa project.
- libclang: The C interface to the parser of the Clang compiler, which reads C and C++ headers.
- marshalling: Converting a Rust value into the memory layout that a shader parameter expects, and converting a result back.
- mesh shading: A render pipeline in which a task stage and a mesh stage, which run like compute shaders, produce the primitives that the fragment stage draws, in place of the vertex stage.
- Metal: The graphics API of macOS. Metal Shading Language (MSL) is its shader language.
- milestone: One step of the roadmap in Section 15, identified as M0 to M18.
- Miri: An interpreter for Rust that detects some undefined behaviour in unsafe code. It runs on the nightly toolchain and cannot call native code.
- module: One file of Slang source, like a Rust module.
- nextest profile: A named set of settings in the configuration of cargo-nextest, such as a filter that selects tests.
- OptiX: NVIDIA's ray-tracing API, which runs on CUDA.
- pkg-config: A tool that reports the compiler and linker flags of an installed library from a `.pc` file.
- pre-release identifier: The part of a Semantic Versioning (SemVer) 2.0.0 version after the first hyphen and before any plus sign, such as `slang.2026.17.0.25.gc7954ebd1` in `0.1.0-slang.2026.17.0.25.gc7954ebd1`. It consists of identifiers separated by dots, and SemVer compares identifiers that consist of digits only numerically.
- procedural macro: A Rust macro, compiled as a crate of its own, that runs during the compilation of the code that uses it and generates Rust code.
- ray query: A query that traces a ray through an acceleration structure from inside an ordinary shader, without a ray tracing pipeline.
- ray tracing pipeline: A GPU pipeline whose stages generate rays, run a hit group on each hit, and run a miss shader on each miss.
- recorded commit: The Slang commit that `slang-src` records and clones by default, which is the commit of the Slang in the Vulkan SDK that CI installs (Section 5.1).
- recorded Slang: The Slang build at the recorded commit, against which the crates are written and tested (Section 5.1).
- reflection: Slang's API for querying the parameters, types, layouts, functions, and entry points of compiled Slang code.
- render pipeline: A GPU object that combines the shaders and the fixed settings that draw an image.
- run-time search path (rpath): A list of directories, stored in a binary, where the dynamic loader of the operating system looks for shared libraries.
- session: A Slang object that holds the settings of one compilation context, such as its targets and search paths, and loads modules.
- shader: A program that runs on a GPU.
- shader passthrough: A path in wgpu that passes shader code in the backend's own format to the backend without translating it.
- shader table: The table in GPU memory through which a ray tracing pipeline finds the ray generation shader, the miss shaders, and the hit group for each ray.
- shader target: The output format that Slang compiles a shader to, such as SPIR-V or MSL.
- shallow clone: A git clone that fetches only the named commit, without the history before it.
- shared folder: The directory `slang-src` in the Cargo target directory, or in the build directory when Cargo uses a separate one, in which `slang-src` keeps its clone and the crates that build Slang keep their built libraries for every profile and every host and target unit of a project (Section 4.3).
- Slint: A Rust toolkit for graphical user interfaces.
- slang-glslang: The library of the Slang tree that wraps glslang and the SPIR-V optimizer. The Vulkan SDK's Slang ships it, and Slang uses it to optimize the SPIR-V it emits.
- slang-rhi: Slang's render hardware interface, a C++ library that gives one API over several graphics and compute APIs.
- SlangPy: A Python library that calls Slang functions on the GPU from Python.
- smoke test: A short test that shows that the basic path works, such as creating one object and reading one value.
- software adapter: An adapter that runs on the central processing unit (CPU), such as lavapipe or WARP.
- SPIR-V: The binary intermediate shader format of Vulkan.
- spike: A short experiment whose result is a recorded finding, not shipped code.
- tensor: A multi-dimensional array of elements in GPU memory.
- trainable primitive: A Slang function with parameters that a training loop adjusts from the gradients of a loss, in the style of SlangPy.
- vendored build: A build in which `slang-compiler-sys` has its opt-in Cargo feature `vendored`, so that it takes the vendored route.
- vendored route: The opt-in way of obtaining Slang, which builds Slang from the source tree that `slang-src` obtains and links it statically (Section 4.3).
- Vulkan: A cross-platform graphics and compute API.
- Vulkan SDK: The LunarG Vulkan software development kit (SDK), which ships Vulkan tools, headers, and libraries, including Slang.
- vulkano: A Rust library that wraps Vulkan in a safe API. Its companion crate vulkano-shaders generates typed Rust code from shaders at build time.
- Windows Advanced Rasterization Platform (WARP): The software implementation of D3D12 on Windows.
- wgpu: A Rust graphics library with one API over Vulkan, Metal, D3D12, and other backends. wgpu-hal is its lower-level hardware abstraction layer.
- winit: A Rust library that creates windows and handles their events.

## 3. Crates and Layering

### 3.1. Workspace Members

This table lists the members of the workspace. The examples form one workspace member, the unpublished package `slang-rust-examples`. The rows of `slang-rhi-sys` and `slang-rhi` are planned crates, whose directories the design of M9 chooses (Section 15.12).

`slang-src` and `slang-glslang-sys` exist only for a vendored build, and their build scripts clone and build native code. The workspace leaves them out of its default members, and every recipe that builds the default excludes them, as `docs/design/tooling.md` specifies.

| Member | Directory | Responsibility | Published | Milestone |
| --- | --- | --- | --- | --- |
| `slang-compiler-sys` | `crates/slang-compiler-sys/` | Raw bindings that its build script generates, the build information, and the linking of Slang (Section 6) | Yes | M1, M7 |
| `slang-compiler` | `crates/slang-compiler/` | Safe wrappers over the compile and reflection API (Section 7) | Yes | M2 |
| `slango` | `crates/slango/` | High-level library for calling Slang code on the GPU (Section 8) | Yes | M3, M4 |
| `slang-slint` | `crates/slang-slint/` | A Slint view backed by a Slang render pipeline (Section 9) | Yes | M5 |
| `slango-macros` | `crates/slango-macros/` | Typed Rust bindings to Slang code, generated at build time (Section 8.4) | Yes | M6 |
| `slango-codegen` | `crates/slango-codegen/` | The generator of `slango-macros`, as a library (Section 4.2 of `docs/design/slango-macros.md`) | Yes | M6 |
| `slang-src` | `crates/slang-src/` | Obtains the Slang source tree and exports its path to the build scripts of the crates that build Slang (`docs/design/slang-src.md`) | Planned for the first release (Section 13) | M7 |
| `slang-glslang-sys` | `crates/slang-glslang-sys/` | Builds slang-glslang and holds the in-process loader that hands it to a statically linked Slang (`docs/design/slang-glslang-sys.md`) | Planned for the first release (Section 13) | M7 |
| `slang-bindgen` | `crates/slang-bindgen/` | The binding generator, which the build script of `slang-compiler-sys` runs to generate the raw bindings (Section 6.2) | Planned for the first release (Section 13) | M1, M7 |
| `slang-rhi-sys` | Planned | Raw bindings of slang-rhi and the build script that builds it | Planned for the second release (Section 13) | M9 |
| `slang-rhi` | Planned | Safe wrappers over `slang-rhi-sys` | Planned for the second release (Section 13) | M9 |
| Examples | `examples/` | Example programs that also serve as the integration testbed (Section 10) | No, `publish = false` | M2 to M6 |

### 3.2. Dependency Direction

Dependencies between the library crates point strictly downward, as this diagram shows. An arrow points from a crate to a crate that it depends on.

`slango-macros` is a procedural macro crate. It depends on `slango` through its generator, the library crate `slango-codegen`, and runs the compile and reflection of `slango` at build time on the `.slang` file that the macro reads. `slango` does not re-export it (Sections 4.1 and 4.2 of `docs/design/slango-macros.md`).

`slango` depends on `slang-compiler` directly. The examples depend on whichever library crate they demonstrate.

`slang-compiler-sys` depends on `slang-src` only under its feature `vendored`, and `slang-compiler` depends on `slang-glslang-sys` only under its own feature `vendored`, which also enables that of `slang-compiler-sys`. `slang-glslang-sys` depends on `slang-src` and on the vendored `slang-compiler-sys`, whose Slang its loader serves (SGL-1 of `docs/design/slang-glslang-sys.md`). `slang-bindgen` is a build dependency of `slang-compiler-sys`, drawn with a dotted arrow.

```text
  slang-slint                     Slint view, wgpu only
       |
       |    slango-macros         procedural macro, runs the compile and
       |     |                    reflection of slango at build time
       |     v
       |    slango-codegen        generator of slango-macros
       |     |
       v     v
    slango                        high-level library, wgpu first
       |
       v
  slang-compiler ----------+      safe compile and reflection API
       |                   |  vendored
       |                   v
       |            slang-glslang-sys        slang-glslang and its loader
       |               |          |
       v               |          |
  slang-compiler-sys <-+          |          raw bindings, links Slang
       |   :   |                  |
       |   :   +-- vendored --+   |
       |   :                  v   v
       |   :               slang-src         obtains the Slang source tree
       |   :
       |   :..> slang-bindgen                generates the raw bindings
       v
  native Slang library
  (Vulkan SDK, or built from the tree of slang-src)
```

ARCH-1. A library crate MUST depend only on the library crates below it in the diagram of this section, which are the crates that a path of arrows from it reaches. An upward or circular dependency forces a lower crate to change and be released whenever a higher one does, and makes the lower crate unusable on its own.

ARCH-26. A library crate MUST NOT depend on an example. A crate other than a `-sys` crate MUST NOT depend on `slang-bindgen`, and a `-sys` crate MUST depend on it only as a build dependency or a development dependency, because the generator runs while the crate builds, and a normal dependency would link libclang's crates into the user's program for no use.

M9 adds `slang-rhi-sys` and `slang-rhi` to the diagram, with `slang-rhi-sys` below `slang-src` and `slang-bindgen` like `slang-compiler-sys`. M10 makes `slang-rhi` a dependency of `slango` behind the opt-in feature `rhi`, while wgpu stays the default backend (Sections 15.12 and 15.13).

### 3.3. Unsafe Code

Unsafe code appears only in `slang-compiler-sys`, `slang-compiler`, and `slang-glslang-sys`, as Hard Rule 7 of `AGENTS.md` and Section 4.4 of the coding standard state. `slang-glslang-sys` declares the one foreign function that returns its loader, in an `unsafe extern "C"` block, and has no other unsafe code (SGL-13 of `docs/design/slang-glslang-sys.md`). The other crates have none:

- `slang-src` runs git and `slang-bindgen` parses headers, both through safe APIs (SRC-2 of `docs/design/slang-src.md` and SCS-57 of `docs/design/slang-compiler-sys.md`).
- `slango` loads every shader through wgpu's validated path (SLO-2 of `docs/design/slango.md`).
- `slang-slint` shares Slint's device and imports textures through safe APIs (SLS-4 of `docs/design/slang-slint.md`).
- `slango-codegen` and `slango-macros` generate and compile Rust tokens only.

A later feature of `slango` or `slang-slint` may need unsafe code, such as for shader passthrough or for interoperation with Slint or wgpu-hal. Each such need follows the procedure of Section 4.4 of the coding standard, and the design document of that crate decides it (Section 17, Q-19).

The planned crates `slang-rhi-sys` and `slang-rhi` of M9 will hold unsafe code, as `slang-compiler-sys` and `slang-compiler` do. The design of M9 records the reason before any of their code exists, and the same change widens this section and Hard Rule 7 of `AGENTS.md` (Section 15.12).

### 3.4. Design Documents per Crate

Each crate gets its own design document at `docs/design/<crate-name>.md`, such as `docs/design/slang-compiler.md`. The design document of `slang-compiler-sys` also covers the binding generator `slang-bindgen`, because the two change together. `slang-src` and `slang-glslang-sys` have their own design documents, `docs/design/slang-src.md` and `docs/design/slang-glslang-sys.md`. The design document of `slango-macros` also covers `slango-codegen`.

The design document of each crate answers the open questions that Section 17 assigns to it.

ARCH-3. An agent MUST write the design document of a crate as the first task of the milestone that delivers the crate. The document then exists before work on any code of that milestone starts.

A reviewer checks code against its design document, and code written first fixes decisions that nobody has reviewed. The agent writes it no earlier than that milestone, because a design document written long before its milestone describes a context that has changed by the time the code is written.

## 4. Obtaining Slang

### 4.1. Two Routes

Slang is obtained in exactly one of two ways: the dynamic route by default, and the vendored route when the Cargo feature `vendored` of `slang-compiler-sys` is on. There is no prebuilt download and no other way. Section 14.6 gives the reason.

| Route | Selected by | Slang comes from | Linking | The user needs |
| --- | --- | --- | --- | --- |
| Dynamic | Default | An installed LunarG Vulkan SDK at `VULKAN_SDK` | Dynamic | libclang and a Vulkan SDK at build time, and the Slang shared library on the library path at run time |
| Vendored | The Cargo feature `vendored` | The source tree that `slang-src` obtains, which is always a shallow clone of the recorded commit | Static | libclang, git, CMake, Ninja, a C++ compiler, network access for the first build in a target directory, and Python 3 when the build includes slang-glslang. No Slang library at run time |

No environment variable selects a route. Every crate between a user and `slang-compiler-sys` forwards the feature under the same name (Section 19.1). On both routes, the build script generates the bindings from the headers of the Slang that it links (Section 6). SCS-50 of `docs/design/slang-compiler-sys.md` decides the selection and gives the reasons.

### 4.2. Dynamic Route

ARCH-4. The build script of `slang-compiler-sys` MUST, on the dynamic route, read `VULKAN_SDK` when it runs, generate the bindings from the Slang headers under `$VULKAN_SDK/include/slang`, record the build tag of those headers in its build information, and link the library `slang-compiler` under `$VULKAN_SDK/lib` dynamically. The SDK is the only source of Slang that the default route allows, and headers and library from one SDK are one Slang, so the bindings match the library whatever its version (Section 9.2 of `docs/design/slang-compiler-sys.md`).

ARCH-5. The build script of `slang-compiler-sys` MUST stop the build with an error that names `VULKAN_SDK` and the feature `vendored` when it takes the dynamic route and `VULKAN_SDK` is unset. It MUST NOT search other locations, because a search outside the two routes finds a library that nobody chose. A build that continues without Slang fails later with a link error that does not say what is missing.

The Vulkan SDK provides the headers under `$VULKAN_SDK/include/slang`, the shared library `libslang-compiler.so` under `$VULKAN_SDK/lib`, a CMake package configuration, and the pkg-config file `slang-compiler.pc`. The SDKs for Windows and macOS have the same layout and ship the same Slang build (Sections 3.6, 3.7, and 9.2 of `docs/design/slang-compiler-sys.md`). The build script expects that layout on every platform and does not use pkg-config.

### 4.3. Vendored Route

ARCH-27. With the feature `vendored`, the build script of `slang-compiler-sys` MUST build Slang with CMake and the setting `SLANG_LIB_TYPE=STATIC` from a copy of the source tree that `slang-src` exports, MUST keep all build output inside `OUT_DIR` or the shared folder, MUST generate the bindings from the headers of that build, and MUST link the result statically. One tree then serves `slang-compiler-sys`, `slang-glslang-sys`, and later `slang-rhi-sys`, so all the native code of a build comes from one Slang commit, and a copy keeps Slang's build from writing into a tree that other builds read.

ARCH-28. On the vendored route, a build script MUST NOT fall back to the dynamic route, and the build script of `slang-compiler-sys` MUST NOT read `VULKAN_SDK`. A user who asked for a static build and silently gets a dynamic one ships a binary that fails on a machine without the SDK. When `slang-src` cannot obtain the tree, such as without network access and without an earlier clone in the shared folder, it stops the build with an error that says that a build without the feature `vendored` links an installed Vulkan SDK and needs no network access (SRC-9 of `docs/design/slang-src.md`).

ARCH-35. The build script of `slang-src` MUST be the only code of the workspace that reaches the network while a crate builds, and only to clone the recorded commit by its full hash with its required submodules (Section 6.1 of `docs/design/slang-src.md`). A build script MUST NOT write outside `OUT_DIR`, except into the shared folder in the Cargo target directory of the build. A download in a build script breaks offline and sandboxed builds and runs where no reviewer looks, so one crate that fetches by commit hash over HTTPS keeps the network access in one reviewable place. The build script honours `CARGO_NET_OFFLINE` and needs the network only for the first build in a target directory.

`slang-src` has no environment variable of its own and always clones the recorded commit (SRC-21 of `docs/design/slang-src.md`). A user who needs another Slang commit names a fork of `slang-src` that records it in Cargo's `[patch]` table, which is best effort (SRC-22 of `docs/design/slang-src.md`, Section 5.3 of this document).

The vendored route of `slang-compiler-sys` links six static libraries, followed by the C++ standard library of the target. The six are the four that Slang's building documentation lists, `slang-compiler` itself, and `cmark-gfm`. The route refuses cross-compilation (Section 9.3 of `docs/design/slang-compiler-sys.md`).

Slang's build produces slang-glslang, which Slang uses to optimize SPIR-V, only as a module that it loads at run time, so a statically linked Slang alone emits SPIR-V that is not optimized. `slang-glslang-sys` builds slang-glslang as static libraries from the same tree, and `slang-compiler` with its feature `vendored` installs its in-process loader on every global session, so a vendored build of `slang-compiler` emits optimized SPIR-V (SGL-12 of `docs/design/slang-glslang-sys.md`).

By default, the clone and the built libraries live in the shared folder, which every profile and every host and target unit of a project use, so a procedural macro that links Slang, such as `slango-macros`, and the program clone and build Slang once for each target directory. `cargo clean` removes the folder, and projects share it only by sharing a target directory. Where the build script finds no tag of Cargo above `OUT_DIR`, each unit builds in its own `OUT_DIR` instead (Sections 8 and 9 of `docs/design/slang-src.md`). The lookup works on x86_64 Linux, Windows, and macOS in a target directory that Cargo creates without `--target`, where the build script clones once. That the lookup holds under `--target` and with a separate build directory, and that every unit shares one native build, are assumptions (Section 16, A-28).

The recipe `test-vendored` runs the tests of the native crates and `slang-compiler` with the feature, without the SDK (SCS-63 of `docs/design/slang-compiler-sys.md`).

This command builds a crate on the vendored route from a clone of the recorded commit, which needs network access for the first build in a target directory:

```sh
cargo build --features vendored
```

### 4.4. Finding the Shared Library at Run Time

On the dynamic route, the operating system's dynamic loader has to find the Slang shared library when the program starts. That is the job of the user's environment: the SDK's setup script on Linux and macOS, and `PATH` on Windows. The build script of a dependency is understood to have no way to set the rpath of the final binary (Section 16, A-6).

The crate-level documentation of `slang-compiler-sys` says how the library is found on each platform, as Section 7.4 of the writing guide requires. The CI workflow does not run the setup script. Its install step sets the same loader variables directly, and `PATH` on Windows (Section 7.3 of `docs/design/tooling.md`).

In this repository, each recipe that runs compiled code which loads Slang puts the library path assignment on the command line of the program that it runs, without exporting it (SCS-32 of `docs/design/slang-compiler-sys.md`). A program run outside the recipes needs the path in its environment, which Section 10 of `docs/design/tooling.md` requires of the container image.

### 4.5. Documentation Builds

A documentation build of a crate that depends on `slang-compiler-sys` runs its build script like any other build, so it generates the bindings with libclang and needs a Vulkan SDK or a vendored build. No build script reads `DOCS_RS` or has a route for a build without Slang.

docs.rs is not supported, so no build has to work without Slang. CI builds the documentation and hosts it outside docs.rs (Section 13).

## 5. Slang Version Policy

### 5.1. Version Target and Recorded Commit

The workspace targets the Slang that ships in the latest stable LunarG Vulkan SDK. Section 14.5 gives the reason for this target.

The recorded Slang is the Slang of Vulkan SDK 1.4.363.1, whose `slangc -version` prints `2026.17-25-gc7954ebd1`, which is 25 commits past the tag `v2026.17`. The recorded commit is therefore a commit, not a Slang tag, and `slang-src` records it with its build tag (Section 7.1 of `docs/design/slang-src.md`).

This command prints the Slang version of the installed SDK:

```sh
"$VULKAN_SDK/bin/slangc" -version
```

ARCH-29. An agent MUST record the recorded commit and its build tag only in `slang-src` (SRC-11 of `docs/design/slang-src.md`), and the Vulkan SDK version that CI installs, with its checksums, only in the CI configuration. The check of the installed SDK in CI MUST fail when the Slang of that SDK differs from the recorded Slang. Two copies of one value diverge, and CI tests the dynamic route and the vendored route against one Slang only when the two records name the same one.

No recorded version gates a build. A dynamic build accepts the Slang of any installed SDK, generates the bindings from its headers, and records its build tag in the build information (ARCH-4). A vendored build records the build tag that `slang-src` exports.

ARCH-34. `slang-compiler` MUST check, when it creates a global session, that the build tag of the Slang that the program loaded at run time equals the build tag of the build information of `slang-compiler-sys`, and MUST return an error otherwise. A library of another Slang has layouts and vtables that differ from the bindings, and the dynamic loader can find a library other than the one whose headers the build read. `docs/design/slang-compiler.md` specifies the check.

### 5.2. Moving the Recorded Slang

ARCH-30. An agent that moves the recorded Slang to the Slang of a new Vulkan SDK MUST, in one change, update the SDK version and its checksums in the CI configuration, the recorded commit and build tag of `slang-src` with its manifest version and every exact requirement of it, fix what the new headers break, and run every check, including the vendored CI jobs. Records that disagree on the main branch make CI test two different Slangs or fail on every run (Section 8.2 of `docs/design/slang-compiler-sys.md`).

ARCH-32. An agent that prepares a release after a move of the recorded Slang MUST release `slang-src` with a new minor number (SRC-13 of `docs/design/slang-src.md`) and release every crate that requires it exactly. It MUST release `slang-compiler-sys` as a breaking change, except when the generated bindings of the old and the new recorded Slang are identical. A user who upgrades within a compatible version range expects code that compiled against the bindings to keep compiling.

Slang is reported to release every one to three weeks. That is background to why the recorded Slang follows the SDK and not each Slang release.

### 5.3. Another Slang Version

ARCH-31. The crates MUST be written and tested against the recorded Slang only. An agent MUST NOT add code that exists only to support another Slang version, and MUST NOT add a setting that relaxes the rule set of the binding generator or the check of ARCH-34. Slang's documentation states that its ABI is not stable (Section 3.7 of `docs/design/slang-compiler-sys.md`), so code for several versions needs tests that no CI job runs, and a relaxed check links declarations that do not match the library.

Another Slang version, from another SDK or from a fork of `slang-src` that the user names in Cargo's `[patch]` table (SRC-22 of `docs/design/slang-src.md`), is best effort. The generator refuses headers that its rule set does not cover (SCS-2 of `docs/design/slang-compiler-sys.md`), and the safe crates can name items that an older Slang lacks. The crate documentation of `slang-compiler-sys` and of `slang-src` states another version as unsupported.

## 6. The `slang-compiler-sys` Crate

### 6.1. Contents

`slang-compiler-sys` contains the build script of Section 4, which generates the bindings into `OUT_DIR` in every build, the inclusion of those bindings, the build information, and its crate-level documentation, with no other hand-written API, as FFI-1 of the coding standard requires. No generated code is in the repository or the published package. Its manifest declares `links = "slang-compiler"` and the one feature `vendored`, and its build script emits every link directive (FFI-2).

The build information `BUILD_INFO` records the build tag of the Slang of the build, its full commit on the vendored route, its linkage, and a full version whose build metadata names both, such as `0.1.0+slang.2026.17.0.25.gc7954ebd1.vendored` (SCS-53 of `docs/design/slang-compiler-sys.md`). The build script also exports the include directory, the library directory, the linkage, and the build tag as `links` metadata, which a later `-sys` crate that builds against the same Slang reads (SCS-54).

The generated code consists of these items:

- Plain C types and constants.
- Enumerations in the form that FFI-10 requires.
- `extern "C-unwind"` functions in the form that FFI-3 and FFI-7 require.
- For each COM-style interface, a `#[repr(C)]` vtable struct, a UUID constant, and thunks.

The generated module carries the lint exception that Section 7.9 of the writing guide defines.

### 6.2. Binding Generator

The binding generator is the published library crate `slang-bindgen`, which parses the Slang headers with libclang for one target and returns the bindings and the list of their C functions as text. The build script of `slang-compiler-sys` runs it in every build, so every build needs libclang, and later the build script of `slang-rhi-sys` runs it too. A thin binary of the crate writes the same text into files for the checker of the guard around calls into Slang (SCS-60 of `docs/design/slang-compiler-sys.md`).

The library follows ERR-1 and ERR-2 of the coding standard. Its binary is an application, so those rules do not apply to the binary (Section 1.1 of the coding standard).

ARCH-33. The binding generator MUST produce every generated item of Section 6.1, including each vtable struct, UUID constant, and thunk, from the headers of the Slang that the same build links, because a hand-written vtable or UUID diverges from the headers without any check failing, and Slang reuses vtable slots and renumbers enumerations between releases. COM-4 and COM-9 forbid writing them by hand.

No test compares the bindings with a file, because none is kept. Every build compiles the generated code and its layout assertions, a test references every native symbol of the generated list so that a broken link fails a test, and the safe crate compiles and runs against the generated items (Section 7.6 of `docs/design/slang-compiler-sys.md`).

`docs/design/slang-compiler-sys.md` specifies the generator, the generated code, and the tests.

## 7. The `slang-compiler` Crate

`slang-compiler` wraps the compile and reflection API of Slang: the global session, sessions, module loading, entry points, the composition and linking of component types, target code returned as blobs, diagnostics, and reflection of types, variables, layouts, functions, and entry-point parameters. It follows Section 12 of the coding standard (COM-1 to COM-13).

In particular, one owning interface pointer type holds every interface (COM-1), and every child holds a reference to its parent so that a session cannot be destroyed under its modules (COM-5). Result codes map to the crate's one `Error` type in one place (COM-7, ERR-1), and diagnostics are always collected (COM-8).

ARCH-14. `slang-compiler` MUST NOT depend on wgpu or on any other graphics API crate, because users who only compile and reflect shaders, such as the first example of Section 10, would otherwise build a GPU stack they do not use. Every breaking upgrade of that stack would also become a breaking release of the compiler crate (DEP-3).

`docs/design/slang-compiler.md` specifies the crate and example 1. It answers the questions on thread safety, the file-system and include-path model, compile options, deprecated API, and, for this crate, logs. These decisions of that document shape the crates above it:

- Every wrapper holds the object that Slang created it from, so a module or a linked program keeps its session and global session alive (Section 5.2 of that document).
- No wrapper, builder that holds one, reflection handle, or iterator over them is `Send` or `Sync`, so a program that compiles on several threads creates a global session on each (Section 10 of that document).
- A reentrant call into Slang panics, and a release during a call into Slang waits until the call returns (SLC-28 of that document).
- A call that returns diagnostics returns them with its value on success and in the error on failure (Section 6.8 of that document).
- Compiler options have typed setters for the options of its Section 7.2 and `unsafe` raw setters for every other option. A setting in the options of a session reaches every target, and an option that Slang reads only from the session is an error in the options of a target (Section 7 of that document).
- A loaded module stays loaded for the life of its session, so hot reload creates a new session from the same global session and loads the module again (Section 8.5 of that document).
- A user cannot give a session a custom file system. It is deferred with no milestone assigned (Section 15.22).

When a C++ exception escapes Slang, which the recorded Slang does for types nested past its limit of 128 levels, `slang-compiler` aborts the process instead of returning an error. It writes a message on standard error unless the program builds with `panic = "abort"`. Rust offers no defined way to turn a foreign exception into an error, and a C++ shim that could would make every build need a C++ compiler (Section 11.6 of that document).

A crate above `slang-compiler` that compiles untrusted source and must survive such a shader compiles in a separate process.

## 8. The `slango` Crate

### 8.1. Purpose and Feature Set

`slango` makes calling Slang code from Rust simple, in the way that SlangPy does for Python. SlangPy's portable core is reported to consist of kernels generated from Slang reflection and cached per call signature, automatic broadcasting of arguments with an explicit `map()`, a unified tensor type, automatic differentiation through Slang's `bwds`, and a device layer (Section 16, A-9).

`slango` calls existing compute entry points only, which are functions marked `[shader("compute")]`, on wgpu, with bindings built from reflection. It does not generate a kernel around a plain Slang function. Generated kernels, broadcasting over array arguments, and differentiable functions belong to later milestones, whose designs decide which parts of SlangPy's core `slango` provides.

The wrappers of `slang-compiler` stay on the thread that created them, and its reflection handles borrow the program they came from (Section 7). A load of `slango` therefore compiles on the calling thread, copies what the kernels need into a private, owned layout snapshot, and releases every Slang object before it returns.

The public types of `slango` therefore hold only wgpu objects and owned data, and they are `Send` and `Sync`. A call sets each parameter by name (Sections 4.1, 7.1, 9.1, and 11 of `docs/design/slango.md`).

### 8.2. Runtime Backend

The runtime backend of `slango` is wgpu first. Its design does not rule out a slang-rhi backend later. Section 14.4 gives the reason.

ARCH-15. `slango` MUST keep its backend-specific types behind a narrow internal boundary and MUST expose wgpu types only in one clearly separated public module. A later slang-rhi backend then adds code without changing the rest of the public API, while wgpu types that appear throughout the API would make that backend a breaking change everywhere.

This constraint does not ask for a backend trait, which SIMP-2 of the coding standard forbids before a second backend exists. Section 6 of `docs/design/slango.md` defines the boundary.

### 8.3. Patterns From vulkano

`slango` takes these patterns from vulkano:

- Ownership through `Arc`, with children holding their parents.
- Structs of creation options.
- Validated calls with clearly named unchecked variants.
- Typed shader interfaces generated from reflection.

Section 3.7 of `docs/design/slango.md` confirms the first three, and Section 3.2 of `docs/design/slango-macros.md` confirms the fourth from vulkano-shaders 0.35.0.

ARCH-16. `slango` MUST NOT adopt the `GpuFuture` synchronization model of vulkano, because a maintainer describes it as hard to chain correctly while building an experimental replacement (Section 3.7 of `docs/design/slango.md`). A model that is hard to use correctly in a library leads users to synchronization defects that no test finds.

### 8.4. Call Style

`slango` offers two call styles on one core. The runtime API is driven by reflection. The user loads a module, looks up a compute entry point, sets its arguments by name through a sealed marshalling trait, dispatches it, reads the results back, and receives errors at run time.

Section 5.2 of `docs/design/slango.md` illustrates it.

The crate `slango-macros` provides the second style, a build-time macro that reads a `.slang` file and generates typed Rust functions and structs, in the way vulkano-shaders does for vulkano. Section 14.7 gives the reason for building the runtime style first.

ARCH-17. `slango-macros` MUST generate code that calls the same core of `slango` that the runtime API uses, and MUST NOT generate code that compiles or dispatches shaders by a separate path, because two paths diverge in behaviour. A fix to one leaves the defect in the other.

### 8.5. Shader Target

`slango` compiles shaders to SPIR-V only, for compute and for rendering. Section 14.9 gives the reason for SPIR-V first. A shader target configurable per backend, such as MSL on macOS, is deferred with no milestone assigned (Section 15.22, Section 17, Q-23), for the reason that Section 1.5 of `docs/design/slango.md` states.

wgpu's SPIR-V reader rejects some valid SPIR-V modules (Section 3.5 of `docs/design/slango.md`). wgpu offers a shader passthrough path, which runs SPIR-V (Section 3.2 of that document) and is reported to cover other targets (Section 16, A-11).

ARCH-18. An agent MUST carry out a spike as the first task of milestone M3. The spike compiles a set of test shaders to SPIR-V and loads them through wgpu on lavapipe.

The agent MUST record what fails in the design document of `slango` before the runtime call API is designed. An API built before the spike can depend on a shader path that wgpu rejects.

### 8.6. wgpu Version

ARCH-19. `slango` MUST use a wgpu major version that the current Slint release supports, because `slang-slint` passes wgpu objects between `slango` and Slint, and two major versions of wgpu in one build have incompatible types.

Slint 1.18 supports wgpu 29 and 30 through features named `unstable-wgpu-29` and `unstable-wgpu-30` (Section 3.7 of `docs/design/slango.md`). `slango` uses wgpu 30 (SLO-1 of that document).

wgpu and the crates around it are below version 1.0, so a breaking upgrade of wgpu is a breaking release of every crate that exposes it, as DEP-3 of the coding standard states.

## 9. The `slang-slint` Crate

`slang-slint` provides a Slint view backed by a Slang render pipeline, built on `slango`. External GPU content enters Slint through its rendering notifier and its import of wgpu textures, and Slint names the features of that API unstable (Section 3 of `docs/design/slang-slint.md`). Slint is reported to have no public Vulkan hook, and that API is reported to be able to change in minor releases (Section 16, A-14).

`docs/design/slang-slint.md` settles these points:

- A minimal view that hands a texture that `slango` draws to an ordinary Slint `Image` element.
- A device that Slint creates from a configuration that `slang-slint` supplies.
- The licence consequence of depending on Slint (Section 4.2 of that document).

ARCH-20. `slang-slint` MUST support wgpu only, permanently, and MUST pin a narrow range of Slint versions, because its only entry into Slint is the wgpu texture import, and that API is marked unstable, which would otherwise let a release of Slint break the builds of users. The design document of `slang-slint` chooses the range (SLS-1 of that document).

## 10. Examples

The examples are complete programs that also serve as the integration testbed, as TEST-5 of the coding standard requires. This table lists them with the milestone that delivers each one and its automated check.

| Number | Example | Needs | Milestone | Automated check |
| --- | --- | --- | --- | --- |
| 1 | `compile-and-reflect`, a command-line program that compiles Slang source and prints its reflection | Slang, no GPU | M2 | Runs headless in CI |
| 2 | `compute-kernel`, a compute kernel. A broadcasting case and a differentiable function that uses Slang's automatic differentiation arrive with those features in M16 (Sections 8.1 and 15.19) | Slang and any adapter | M3 | Runs headless in CI on a software adapter |
| 3 | `windowed-renderer`, a windowed renderer with winit and shader hot reload | Slang, a display, and an adapter | M4 | Built in CI on the three platforms and run locally, with its scene drawn offscreen by tests in CI (Section 13.5 of `docs/design/slango.md`) |
| 4 | `shader-playground`, a Slint application with a `slang-slint` view and sliders bound to shader parameters | Slang, a display, and an adapter | M5 | Built in CI and run locally, with its scene drawn offscreen by tests (Section 10 of `docs/design/slang-slint.md`) |
| 5 | `typed-kernel`, a headless compute kernel called through the typed bindings of `slango-macros`, with a generated struct for a Slang structure | Slang at build time and any adapter | M6 | Runs headless in the GPU tier on a software adapter (Section 10 of `docs/design/slango-macros.md`) |
| 6 | A path-tracing renderer demo, which shows the ray tracing of `slango` | Not designed | M14 | Not designed |
| 7 | A renderer with hull, domain, and geometry stages | Not designed | M12 | Not designed |
| 8 | A renderer with a mesh shading pipeline | Not designed | M13 | Not designed |
| 9 | A training example built on the neural primitives of `slango` | Not designed | M17 | Not designed |

The shader hot reload of example 3 loads the module again through `slango` for each change of a source file, because a session never reloads a module (Section 7). Each load creates a new global session, and Section 7.2 of `docs/design/slango.md` states why that cost is acceptable.

ARCH-21. An agent MUST give each example at least one automated check, as the table of this section states, because an example that no check builds or runs breaks without notice, and the examples are the testbed of the whole workspace. The windows of examples 3 and 4 run only locally until a headless check of a window is designed (Section 17, Q-24).

## 11. Test Tiers

Tests belong to one of three tiers, selected by the name of the module that holds them. The nextest profiles that select each tier are defined in `docs/design/tooling.md`.

| Tier | Module name | Needs | Runs in CI | Runs locally |
| --- | --- | --- | --- | --- |
| Default | Any name other than `gpu` and `hw_gpu` | No adapter. It may need the Slang shared library, which the build needs anyway | Yes, on Linux, Windows, and macOS | Yes |
| GPU | `gpu` | Any adapter, including a software adapter such as lavapipe or WARP | Yes, on lavapipe, WARP, and hosted Metal (Section 3.8 of `docs/design/slango.md`) | Yes |
| Hardware GPU | `hw_gpu` | A hardware GPU | No | Yes |

ARCH-22. An agent MUST put every test that needs an adapter in a module named `gpu`, or in a module named `hw_gpu` when the test needs a hardware GPU, because the default tier runs everywhere, and a test that needs an adapter fails there. This is the tagging mechanism that TEST-3 of the coding standard asks the design document to define.

ARCH-23. An agent MUST write every test in a `gpu` or `hw_gpu` module so that it fails when it finds no adapter, and MUST NOT write it to skip itself, because a skipped test reports success for a check that never ran. A CI runner that loses its adapter would then pass every GPU test.

Each crate has one integration-test binary, as TEST-10 of the coding standard requires. This illustration shows where the tiers sit in that layout. The suite name is a placeholder:

```text
crates/slango/tests/<suite>/main.rs      declares the modules
crates/slango/tests/<suite>/gpu.rs       tests that need any adapter
crates/slango/tests/<suite>/hw_gpu.rs    tests that need a hardware GPU
```

The root `main.rs` also starts with crate-level documentation, as the row of `missing_docs` in Section 4.5 of the coding standard requires.

## 12. Platform Support

This table states the support tier of each platform.

| Platform | Graphics API in tests | Tier | Tested where |
| --- | --- | --- | --- |
| Linux x86_64 | Vulkan | Primary. Developed and fully tested | The development container, on an NVIDIA RTX 4070 and on lavapipe, and CI, which runs every check, the default test tier, and the GPU tier on lavapipe |
| Windows x86_64 | D3D12 through WARP | Supported as far as CI proves | CI only, which runs Clippy, the doctests, the default test tier, and the GPU tier on WARP |
| macOS arm64 | Metal on the hosted runner | Supported as far as CI proves | CI only, which runs Clippy, the doctests, the default test tier, and the GPU tier on hosted Metal |
| Every other 64-bit target | None | Allowed, untested, and unsupported | Nowhere |

CI installs the Vulkan SDK version that its configuration records on all three platforms, and its Slang on Windows and macOS is the same build as on Linux (Section 3.6 of `docs/design/slang-compiler-sys.md`). One CI job on each platform also builds and tests the vendored route (SCS-62 of that document). On each runner, the GPU tier uses SPIR-V as the only shader target (Section 3.8 of `docs/design/slango.md`).

The tested targets are x86_64 Linux, x86_64 Windows with the Microsoft toolchain, and aarch64 and x86_64 macOS, of which CI only type-checks x86_64 macOS with Clippy (Section 6 of `docs/design/slang-compiler-sys.md`).

ARCH-37. A build MUST NOT stop because its target is not one of the tested targets. The build script of `slang-compiler-sys` MUST state an untested target as untested and unsupported, in a warning and in its build information (SCS-65 of `docs/design/slang-compiler-sys.md`). A build MUST stop for its target only where a correct build is impossible, and MUST name the reason: a target whose pointer width is not 64, and on the vendored route a cross-compilation or a target without a link rule for its C++ runtime library (SCS-66 and SCS-43 of that document). The owner decided that untested platforms are allowed, because a refusal blocks a user whose build would work, while the statement keeps the support honest.

`slang-compiler-sys` stops a target whose pointer width is not 64, because on 32-bit Windows the Slang headers give one method another calling convention and every struct that holds a pointer another layout. Every crate that depends on it inherits that stop. The dynamic route is never refused for the platform.

## 13. Publishing

The workspace plans for publication on crates.io in two releases. The examples set `publish = false` and are never published.

| Release | Milestone | Crates | Condition |
| --- | --- | --- | --- |
| First | M8 | `slang-bindgen`, `slang-src`, `slang-glslang-sys`, `slang-compiler-sys`, `slang-compiler`, `slango`, `slango-codegen`, and `slango-macros` | The rework of M7, the forward-compatibility pass of `slango`, and the publishing prerequisites of M8 are done |
| Second | M11 | `slang-rhi-sys`, `slang-rhi`, `slang-slint`, and the versions of `slango`, `slango-codegen`, and `slango-macros` that carry the feature `rhi` | M10 has added the opt-in `rhi` backend to `slango` |

The first release holds the crates whose shape the rework of M7 settles, together with `slango` and its two macro crates, whose default backend stays wgpu. `slang-slint` is not in it. A published version locks in its public API and its build behaviour, so every change that would break them comes before the release that publishes them (Section 15.23).

The owner accepts one exception to that order, a breaking version 0.2 of `slango` if the `rhi` backend of M10 needs one (Section 15.11).

On 2026-11-15, every planned crate name above was free on crates.io in both its hyphen and its underscore form.

Each crate of the workspace has the licence of its kind, as this table shows. The licences are SPDX licence expressions.

| Kind | Crates | Licence |
| --- | --- | --- |
| A `-sys` or `-src` crate, which binds or builds Slang or a part of the Slang tree | `slang-compiler-sys`, `slang-src`, `slang-glslang-sys`, and the planned `slang-rhi-sys` | `Apache-2.0 WITH LLVM-exception` |
| The Slint view | `slang-slint` | `GPL-3.0-or-later` |
| Every other crate | `slang-compiler`, `slango`, `slango-codegen`, `slango-macros`, the binding generator `slang-bindgen`, the planned `slang-rhi`, and the unpublished examples package | `MIT OR Apache-2.0` |

ARCH-36. Every crate MUST carry the licence that the table assigns to its kind, and a new crate MUST take that licence when it is created.

The owner wants the crates usable under permissive licences before they are published. A `-sys` or `-src` crate takes the licence of Slang, whose code it binds or builds, and every other crate takes the dual licence that is the convention of the Rust ecosystem.

`slang-slint` stays under the GNU General Public License (GPL), because it depends on Slint under `GPL-3.0-only`. The example `shader-playground` links Slint as well, so the program built from it is under version 3 of the GPL only, whatever the licence of the examples' source (Section 4.2 of `docs/design/slang-slint.md`). `docs/design/tooling.md` defines where each licence text is kept and the licence policy for dependencies.

Publishing readiness means complete publication metadata, the text of its licence in each published package, a release procedure, hosted documentation, and checks for breaking changes with cargo-semver-checks. `docs/design/tooling.md` specifies these, and M8 adds the parts that do not exist yet.

docs.rs is not supported. CI builds the documentation of every published crate and hosts it outside docs.rs, such as on GitHub Pages, and the documentation link of each crate's manifest points there. The reason is that every build of `slang-compiler-sys` after M7 generates its bindings with libclang and either clones Slang over the network or links an installed Slang, and docs.rs is reported to build without network access (Section 16, A-25).

Milestone M0 set up the metadata so that it is checked from the start. M8 adds the check for breaking changes with the first release, because cargo-semver-checks needs a published version to compare with. Every crate stays below version 1.0, as DEP-3 of the coding standard requires.

ARCH-24. An agent MUST NOT publish a crate to crates.io without the owner's explicit go-ahead, because a published version cannot be deleted, only yanked, so a mistaken release stays visible to every user.

## 14. Decisions and Rationale

This section is non-normative. It records each decision between alternatives, the alternatives considered, and the reason.

### 14.1. Generated Vtables

The bindings reach the COM-style interfaces through `#[repr(C)]` vtable structs, UUID constants, and thunks that the binding generator in this repository produces. The alternatives were vtables written by hand, a C shim compiled at build time that flattens the interfaces into plain C functions, and the experimental vtable generation of bindgen.

Hand-written vtables have to be checked by hand against every Slang release, and the one existing third-party binding, which writes its vtables by hand, has a reported soundness issue in which objects crash when they outlive their session [SLANG-RS-26]. A C shim would need a C++ toolchain on every user's machine on the default route, which the dynamic route exists to avoid.

bindgen's vtable generation was reported to be experimental when this decision was made, and Section 4.2 of `docs/rust-references.md` records the same gap. A generator in this repository gives exact vtables, and its rule set turns every declaration of a new Slang into a reviewed decision.

### 14.2. Bindings Generated in Every Build

The build script of `slang-compiler-sys` generates the bindings in every build, from the headers of the Slang that the build links, and no generated file is kept in the repository or the package. The alternatives were bindings generated once for one Slang and checked in, which was the earlier decision, and bindings generated at release time and shipped only in the published package.

The owner decided that a build accepts the Slang of any installed SDK and that a user can select another Slang commit. Slang reuses vtable slots and renumbers enumerations between releases, so bindings of one Slang compiled against the library of another are wrong without any error. Only bindings generated from the headers of the linked Slang stay sound when the Slang varies, and they make the run-time check of ARCH-34 the only version check that a build needs.

The cost is that every user needs libclang at build time, the generator's dependencies compile on every user's machine, and docs.rs, which has neither Slang nor libclang, cannot build the documentation. docs.rs is not supported for that reason (Section 13). The earlier decision avoided these costs and paid with a gate that refused every Slang but one.

### 14.3. Separate Raw and Safe Crates

The raw bindings live in `slang-compiler-sys`, separate from the safe crate `slang-compiler`. The alternative was one crate with private raw bindings.

The separate `-sys` crate changes only when Slang changes. It is the one package that carries `links = "slang-compiler"`, which stops two copies of the native library from entering one build (FFI-2). Every decision about ownership, lifetimes, and errors stays in the safe crate (FFI-1), and users who need raw access can depend on the `-sys` crate without the safe API.

### 14.4. wgpu as the Default Backend, slang-rhi as an Opt-In Backend

`slango` uses wgpu as its first and only backend. slang-rhi is reported to publish no releases or prebuilt binaries and to need a C++20 build with CMake (Section 16, A-15). When M3 began, binding it would have put a C++ build on every user's machine, which the default route avoided.

Slint accepts wgpu textures directly (Section 3.4 of `docs/design/slang-slint.md`), so a wgpu backend serves `slang-slint` without any bridge. A slang-rhi backend would add targets that wgpu does not offer, reported to include CUDA, the CPU, and OptiX (Section 16, A-16). That is why ARCH-15 keeps a later slang-rhi backend possible without building an abstraction now.

The owner decided to add slang-rhi as a second backend of `slango` behind the opt-in Cargo feature `rhi`, with wgpu kept as the default backend (Section 15.13). The owner's goals include ray tracing, render pipelines with hull, domain, and geometry stages, mesh shading, and cooperative vectors. wgpu 30 is reported to offer no tessellation, geometry stage, or cooperative vectors, and to offer ray tracing pipelines and mesh shaders only as experimental features (Section 16, A-22).

slang-rhi is reported to cover each of these features on at least one of its device backends (Section 16, A-21). It is also reported to be a submodule of the Slang tree that the Vulkan SDK does not ship, so every build that uses it builds slang-rhi from source with a C++ toolchain (Section 16, A-19).

wgpu stays the default so that default builds stay fast and small unless the user opts in. A default build links the Slang of an installed Vulkan SDK and builds neither Slang nor slang-rhi from source (Section 15.10), while a user who needs the new pipeline kinds opts into the native build of slang-rhi. Because `rhi` adds a backend and leaves the default as it is, `slango` can ship in the first release before the slang-rhi crates exist (Section 15.23).

wgpu also stays because `slang-slint` needs it and a wgpu backend keeps compute kernels and vertex plus fragment pipelines free of unsafe code. The new pipeline kinds exist on the slang-rhi backend only, and the wgpu backend reports them as unsupported (Section 15.13).

### 14.5. The Vulkan SDK's Slang as the Version Target

The workspace targets the Slang in the latest stable Vulkan SDK, not the latest Slang release. The alternative was to follow Slang's own releases.

Slang's ABI is not stable, as its documentation states (Section 3.7 of `docs/design/slang-compiler-sys.md`), and Slang is reported to release every one to three weeks. Following each release would make nearly every Slang release a breaking release of `slang-compiler-sys`.

The SDK ships Slang with its headers, its shared library, and a pkg-config file, so the default route needs one installation besides libclang. Because the SDK's Slang is a commit past a tag, `slang-src` records the commit and its build tag, and the CI configuration records the SDK version.

### 14.6. Two Routes, a Cloned Source Tree, and No Prebuilt Download

Slang comes from an installed SDK, linked dynamically by default, or from a static build of a source tree that `slang-src` obtains, selected by the Cargo feature `vendored`, and nothing else. The alternatives were a prebuilt download in the build script, a search of system locations, building from source by default, and a source tree that the user always supplies.

A prebuilt download runs a binary that nobody built from reviewed source, and official release archives are reported to contain no static libraries, which rules out a prebuilt static download in any case. Building from source by default would require a long native build and a C++ toolchain of every user, so the owner made it opt-in, with the dynamic route as the fast and small default. A search of other locations would find libraries that nobody chose.

A vendored build obtains its source by itself, so that it is visible as a feature in a manifest and needs no source tree from the user. `slang-src` clones by full commit hash, which identifies the content that git verifies, over HTTPS only, in one crate that is the only network access of the workspace (ARCH-35). It has no environment variable of its own, and a user who needs another commit patches in a fork of `slang-src` (Section 4.3). The clone lives in the shared folder, so every later build in the same target directory needs no network access.

### 14.7. Runtime Call API Before the Build-Time Macro

`slango` first offers the runtime API driven by reflection, and `slango-macros` follows in M6. The alternative was to start with the macro.

The macro generates code that calls the runtime core (ARCH-17), so the core has to exist first, and the examples are the best way to decide its shape. The runtime API also serves shaders known only at run time, such as the shaders of the hot-reload example.

### 14.8. Compute Before Rendering

The first `slango` milestone covers compute kernels, and rendering follows. The alternative was to start with rendering, or to do both at once.

Compute kernels run headless, so the milestone is tested on lavapipe in CI, which keeps every milestone up to M3 headless. Compute also needs fewer GPU concepts than a render pipeline, and it is the core of what SlangPy is reported to offer (Section 16, A-9).

### 14.9. SPIR-V First

The first `slango` milestone compiles to SPIR-V only. The alternative was a configurable shader target from the start.

Vulkan on Linux is the primary platform, lavapipe runs SPIR-V, and one target keeps the spike of ARCH-18 small. wgpu rejects some valid SPIR-V modules, so the spike measures that risk before the API depends on it. Section 3.5 of `docs/design/slango.md` lists what it rejects. Rendering keeps SPIR-V as the only target, and the configurable target is deferred (Section 8.5).

## 15. Roadmap

### 15.1. Milestone Rules

ARCH-25. An agent MUST NOT start work on the code of a milestone before the design document that the milestone requires exists, because work without a design cannot be reviewed against one. Writing that document, when it does not exist yet, is the first task of the milestone (ARCH-3).

The agent MUST NOT report a milestone complete until its exit criteria hold, because a milestone reported complete with failing checks or stale documents misleads the agent that starts the next one.

The exit criteria of every milestone include every check passing and every affected document updated, including this document. When a milestone's design document confirms an assumption of Section 16 or answers a question of Section 17, the same change removes that row and records the answer in that design document.

From M7 on, each milestone opens with its own design round, which writes or revises the design documents that the milestone names. The owner's decisions that bind a milestone are stated in its subsection, and its design round works within them.

Several milestones replace requirements of this document that the code satisfies today. Those requirements stay in force until the milestone that names them, and the first task of that milestone rewrites them together with the sections that hold them.

### 15.2. Summary

| Milestone | Delivers | Depends on | Design document before any code |
| --- | --- | --- | --- |
| M0 | Tooling and skeleton | The container image, which provides every tool | `docs/design/tooling.md` |
| M1 | Binding generator and `slang-compiler-sys` | M0 | `docs/design/slang-compiler-sys.md` |
| M2 | `slang-compiler` and example 1, without the custom file system | M1 | `docs/design/slang-compiler.md` |
| M3 | `slango` compute kernels and example 2 | M2 | `docs/design/slango.md` |
| M4 | `slango` rendering and example 3, without the configurable shader target | M3 | The rendering design in `docs/design/slango.md` |
| M5 | `slang-slint` and example 4 | M4 | `docs/design/slang-slint.md` |
| M6 | `slango-macros`, `slango-codegen`, and example 5 | M3 | `docs/design/slango-macros.md` |
| M7 | Rework of the native crates: `slang-bindgen`, `slang-src`, `slang-glslang-sys`, and `slang-compiler-sys` with the opt-in feature `vendored` | M6 | `docs/design/slang-compiler-sys.md`, revised, and `docs/design/slang-src.md` and `docs/design/slang-glslang-sys.md`, new |
| M8 | Forward-compatibility pass of `slango` and the first release | M7 | The publishing sections of `docs/design/tooling.md` and `docs/design/slango.md`, revised |
| M9 | `slang-rhi-sys`, `slang-rhi`, and additive reflection in `slang-compiler` | M7 | `docs/design/slang-rhi-sys.md` and `docs/design/slang-rhi.md`, new, and `docs/design/slang-compiler.md`, revised |
| M10 | The opt-in `rhi` backend of `slango`, with wgpu kept as the default backend | M9 | `docs/design/slango.md`, `docs/design/slango-macros.md`, and `docs/design/slang-slint.md`, revised |
| M11 | The second release | M10 | The release procedure in `docs/design/tooling.md` |
| M12 | Render pipelines with hull, domain, and geometry stages, and example 7 | M10 | A design in `docs/design/slango.md` |
| M13 | Mesh shading pipelines and example 8 | M10 | A design in `docs/design/slango.md` |
| M14 | Ray tracing and example 6 | M9, M10 | A design in `docs/design/slango.md` |
| M15 | The opt-in slang-rhi backends CUDA with OptiX, and CPU | M14 | Designs in `docs/design/slang-rhi-sys.md` and `docs/design/slango.md` |
| M16 | Generated kernels with broadcasting, automatic differentiation, and tensors, and the new cases of example 2 | M9, M10 | A design in `docs/design/slango.md` |
| M17 | Neural primitives and example 9 | M16 | A design in `docs/design/slango.md` |
| M18 | Typed bindings of `slango-macros` for the new pipeline kinds | M12, M13, M14 | `docs/design/slango-macros.md`, revised |

### 15.3. M0: Tooling and Skeleton

- Deliverables: the contents of `docs/design/tooling.md`. They include the deletion of the abandoned sketch in `crates/slang-compiler-sys` and an empty and documented skeleton crate so that every tool has something to check. They also include the workspace lint configuration, `clippy.toml`, `deny.toml`, the nextest configuration, the `just` recipes for the Rust checks, the licence file, `README.md`, the CI workflow, and the publication metadata, so that it is checked from the start.

  The recipes for typos, taplo, cargo machete, Miri, cargo hack, and cargo llvm-cov are part of M0, and the recipe that runs actionlint arrives with the first workflow. The check for breaking changes is not part of M0 and arrives in M8 (Section 15.11).
- Depends on: the container image defined in the repository slang-rs-env, which provides every tool that M0 needs.
- Design document, which exists before any code of the milestone: `docs/design/tooling.md`.
- Exit criteria: `just check` passes with the Rust checks, Miri, and the checks of typos, taplo, actionlint, and cargo machete included, and the CI workflow passes on Linux, Windows, and macOS, including the jobs for the feature combinations and the MSRV. The coverage job is part of M1. No example exists yet.

  `AGENTS.md`, Section 4.1 of the coding standard, and `README.md` describe the new state.

### 15.4. M1: Binding Generator and `slang-compiler-sys`

- Deliverables: the binding generator, the checked-in bindings, the two tests of TEST-8, and the build script with the dynamic route first and the static route second. They also include a smoke test that creates a global session and reads the Slang version string, the installation of the pinned Vulkan SDK in every CI job, and the CI job that measures test coverage.
- Depends on: M0.
- Design document, which exists before any code of the milestone: `docs/design/slang-compiler-sys.md`, which also covers the binding generator.
- Exit criteria: every check passes, including the two binding tests and the smoke test in the default tier. The smoke test passes on both routes in the development container.

  CI does not build the static route on every pull request, and a recipe verifies it locally (Section 12 of `docs/design/slang-compiler-sys.md`). No example exists yet.

  `AGENTS.md`, `README.md` with how to obtain Slang, and this document are updated. Section 4.4 of the coding standard is updated if the lints that fire on the generated module differ from its list.

### 15.5. M2: `slang-compiler`

- Deliverables: the safe API of Section 7 and example 1. The custom file system of Section 8 of `docs/design/slang-compiler.md` is not a deliverable (Section 15.22).
- Depends on: M1.
- Design document, which exists before any code of the milestone: `docs/design/slang-compiler.md`.
- Exit criteria: every check passes, and example 1 runs headless in CI. `AGENTS.md`, `README.md`, and this document are updated.

### 15.6. M3: `slango` Compute Kernels

- Deliverables: the SPIR-V spike of ARCH-18, the runtime call API for existing compute entry points (Section 8.1), buffers, reading results back, and example 2 without its broadcasting case and differentiable function, which arrive with those features in a later milestone.
- Depends on: M2.
- Design document, which exists before any code of the milestone: `docs/design/slango.md`.
- Exit criteria: every check passes, including the GPU tier in CI on lavapipe, WARP, and hosted Metal, and example 2 runs headless on lavapipe in CI. The design document of `slango` records the results of the spike. `AGENTS.md`, `README.md`, and this document are updated.

### 15.7. M4: `slango` Rendering

- Deliverables: render pipelines built from Slang entry points and example 3. A shader target configurable per backend is not a deliverable (Section 8.5).
- Depends on: M3.
- Design document, which exists before any code of the milestone: the rendering design in `docs/design/slango.md`, whose Section 1.4 lists its sections.
- Exit criteria: every check passes, example 3 builds in CI, and it runs locally. `AGENTS.md`, `README.md`, and this document are updated.

### 15.8. M5: `slang-slint`

- Deliverables: the Slint view of Section 9 and example 4.
- Depends on: M4.
- Design document, which exists before any code of the milestone: `docs/design/slang-slint.md`.
- Exit criteria: every check passes, example 4 builds in CI, and it runs locally. `AGENTS.md`, `README.md`, and this document are updated.

### 15.9. M6: `slango-macros`

- Deliverables: the build-time macro of Section 8.4, which generates typed Rust functions and structs, its generator crate `slango-codegen`, and example 5.
- Depends on: M3, whose runtime core the generated code calls. The roadmap places M6 after M5.
- Design document, which exists before any code of the milestone: `docs/design/slango-macros.md`.
- Exit criteria: every check passes, including the tests that Section 10 of `docs/design/slango-macros.md` names. `AGENTS.md`, `README.md`, and this document are updated.

### 15.10. M7: Rework of the Native Crates

- Deliverables: these changes to `slang-compiler-sys`, the binding generator, and `slang-compiler`, and two new crates:
  - The binding generator as the published library crate `slang-bindgen` and a build dependency of `slang-compiler-sys`.
  - Bindings that the build script of `slang-compiler-sys` generates with libclang in every build. The checked-in bindings and the checked-in list of C functions leave the repository.
  - The new crate `slang-src`, which obtains the Slang source tree with a shallow clone of the recorded Slang commit and exports its path to the build scripts of the crates that depend on it.
  - The new crate `slang-glslang-sys`, which builds slang-glslang and holds the in-process loader that hands it to Slang.
  - The opt-in Cargo feature `vendored` of `slang-compiler-sys`, off by default, which every crate between a user and `slang-compiler-sys` forwards. With it, the build script builds Slang from the tree of `slang-src` and links it statically, and `slang-compiler` installs the loader of `slang-glslang-sys` when it creates a global session. Without it, the build script links the Slang of an installed Vulkan SDK dynamically.
  - The constant of `slang-compiler-sys` with the build information, and the run-time check that the loaded Slang is the Slang of the build's own record.
  - The shared folder in the Cargo target directory, which keeps the clone and the built libraries for every profile and every host and target unit of a project, and its documentation in `README.md` and in the crate-level documentation.
  - A warning and a field of the build information on a target that is not tested, instead of a refusal.
  - Recipes that pass an explicit list of Cargo features instead of `--all-features`, which would force a vendored build.
  - One CI job per operating system that builds and tests `vendored`.

  Slang's CMake build builds slang-glslang only as a module that Slang loads at run time. A C++ test linked it into a statically linked Slang through `IGlobalSession::setSharedLibraryLoader`, and that Slang emitted optimized SPIR-V with no diagnostic, so the design starts from such a loader. The Rust side of the loader is a deliverable of M7, and its tests answer Q-32 (Section 17).
- Owner decisions that bind it:
  - Every build generates the bindings with libclang in the build script, from the headers of the Slang that it links, and no package contains pregenerated bindings.
  - The binding generator is published as its own crate, `slang-bindgen`, a build dependency of `slang-compiler-sys` and later of `slang-rhi-sys`.
  - `slang-src` only obtains the Slang source tree and exports its path. It is the one place for network access, and `slang-compiler-sys`, `slang-glslang-sys`, and later `slang-rhi-sys` build from that one tree.
  - `slang-src` always clones, and it has no environment variable of its own. There is no local source directory and no variable that selects a commit.
  - `slang-glslang-sys` builds slang-glslang, which consists of glslang, SPIRV-Tools, and Slang's glue code. A statically built `slang-compiler` depends on it and installs its loader when it creates a global session, so a vendored build emits optimized SPIR-V.
  - `vendored` is an opt-in Cargo feature, off by default, and no environment variable selects the linkage. Dynamic linking keeps default builds fast and small.
  - No pin gates a build. A dynamic build accepts the Slang of any installed Vulkan SDK, generates the bindings from its headers, and records the real version. This is best effort, because the safe crates are written and tested against one Slang version and document any other version as unsupported.
  - The recorded Slang commit lives only in `slang-src`. The manifest version of `slang-src` carries the Slang version as a pre-release identifier, by deliberate abuse of the notation, for example `0.1.0-slang.2026.17.0.25.gc7954ebd1`. The design fixes the spelling of the identifiers (SRC-13 of `docs/design/slang-src.md`).
  - `slang-compiler-sys` has a plain manifest version. It exposes a constant with the build information, which names the real Slang version or commit and whether Slang is linked statically or dynamically.
  - The Slang loaded at run time must be the Slang that the bindings were generated from, and the check compares it with the build's own record.
  - The dynamic CI jobs install a pinned Vulkan SDK version, recorded in the CI configuration. The commit of `slang-src` is the commit of the Slang in that SDK, and a check fails when the two differ, so that the crates are tested against one Slang.
  - A user who wants another Slang commit uses Cargo's `[patch]` table with a fork of `slang-src` that records it. The crates offer no other mechanism, and another commit is best effort.
  - By default, `slang-src` keeps the clone, and the native crates keep their built libraries, in one shared folder inside the Cargo target directory of the build, so that the host and target units, such as a procedural macro and the program, and every profile of one project clone once and build Slang once. `cargo clean` removes it. `README.md` and the crate-level documentation MUST document it.
  - Untested and unsupported platforms are allowed. The build states that the platform is untested and unsupported and does not stop for it.
  - The recipes of this repository and most CI jobs build the default, which links dynamically. One dedicated CI job per operating system builds and tests `vendored`.
- Choices of the design within these decisions:
  - Without network access and without a clone in the shared folder, a vendored build fails with an error that says that a build without `vendored` links an installed Vulkan SDK and needs no network access. It honours `CARGO_NET_OFFLINE` and never falls back silently, and git stops a transfer that stays below 1000 bytes per second for 60 seconds (SRC-8 of `docs/design/slang-src.md`).
  - The build tag of the tree is always the recorded build tag, so the clone fetches no tags.
  - The build script finds the shared folder from the nearest ancestor of `OUT_DIR` that holds Cargo's file `CACHEDIR.TAG`, with a rule for the directory of the target triple under `--target`, and uses `OUT_DIR` when it finds none, which is correct and loses only the sharing (SRC-23 of `docs/design/slang-src.md`).
  - Each entry of the shared folder is created once under a file lock and renamed into place when complete, and a stamp file with an old modification time makes Cargo rebuild an entry that was removed.
  - The statement on an untested target is a warning of the build script, which Cargo shows when the build fails, and a field of the build information. A build stops only where a correct build is impossible: a target whose pointer width is not 64, and on the vendored route a cross-compilation or a target without a link rule for its C++ runtime library (ARCH-37).
  - The pre-release identifier of `slang-src` uses numeric identifiers separated by dots, so that versions order correctly, and the crates that depend on `slang-src` require an exact version.
- Replaces: the requirements on the binding generator, the static route, the pin, `DOCS_RS`, and the supported targets with ARCH-26 to ARCH-35 and ARCH-37, together with Sections 4, 5, 6, 12, 14.2, 14.6, 18, 19.1, and 19.2 of this document, and ARCH-1 with the diagram of Section 3.2, which gains `slang-bindgen`, `slang-src`, and `slang-glslang-sys`. The design also rewrites ARCH-4 and ARCH-5 where the feature changes how Slang is obtained. Outside this document, it replaces SYS-5, SYS-6, SYS-7, TEST-8, and COM-9 of the coding standard, SCS-5, SCS-14, SCS-16 to SCS-18, SCS-20, SCS-22 to SCS-25, SCS-27, SCS-29, SCS-36, SCS-38, SCS-42, and SCS-49 of `docs/design/slang-compiler-sys.md`, and Hard Rule 7 of `AGENTS.md`.
- Depends on: M6.
- Design document, which exists before any code of the milestone: `docs/design/slang-compiler-sys.md`, revised, which also covers `slang-bindgen`, and `docs/design/slang-src.md` and `docs/design/slang-glslang-sys.md`, new (ARCH-3). The CI job is specified in `docs/design/tooling.md`.
- Exit criteria: every check passes, including the vendored CI job on Linux, Windows, and macOS. Git tracks no generated bindings, the recipes still link the Slang of the Vulkan SDK, and `README.md` and the crate-level documentation document the shared folder. `AGENTS.md`, `README.md`, the coding standard, `docs/design/tooling.md`, and this document are updated.

### 15.11. M8: Forward Compatibility of `slango` and the First Release

- Deliverables:
  - The forward-compatibility pass of `slango` that the owner decided, as listed below.
  - The publishing prerequisites: a release procedure, the recipe `check-semver` with its CI job, and a review of the public API of each crate of the release against the coding standard.
  - The hosted documentation of each crate of the release, built in CI.
  - The first release, after the owner's go-ahead (ARCH-24).
- Owner decisions that bind it:
  - The first release holds `slang-bindgen`, `slang-src`, `slang-glslang-sys`, `slang-compiler-sys`, `slang-compiler`, `slango`, `slango-codegen`, and `slango-macros`, and not `slang-slint`. It follows as soon as the rework of M7 and the publishing prerequisites are done.
  - Before the release, `slango` gets a small forward-compatibility pass, so that the feature `rhi` of M10 is additive where that is cheap. The wgpu backend moves behind a default Cargo feature `wgpu`, the accessors in `slango::wgpu` that return a raw wgpu object become fallible, and a check confirms that the option structs and enumerations are `#[non_exhaustive]`.
  - No slang-rhi spike precedes the release. The owner accepts a breaking version 0.2 of `slango` if the `rhi` backend later needs one, for example if slang-rhi objects are not thread-safe while the public types of `slango` are `Send` and `Sync`.
  - docs.rs is not supported. CI builds the documentation and hosts it elsewhere, such as on GitHub Pages, and the documentation link of each crate points there.
  - Nothing is published without the owner's explicit go-ahead.
  - The crates of the release are published under the licences that Section 13 assigns (ARCH-36).
- Replaces: the docs.rs requirements of `docs/design/tooling.md`, and the statement of `docs/design/slango.md` that the crate has no Cargo features.
- Depends on: M7.
- Design document, which exists before any code of the milestone: the publishing sections of `docs/design/tooling.md`, revised, and `docs/design/slango.md`, revised for the forward-compatibility pass.
- Exit criteria: every check passes, including the check for breaking changes against the release. The documentation link of each released crate leads to its hosted documentation, and the owner has approved the release. `AGENTS.md`, `README.md`, and this document are updated.

### 15.12. M9: `slang-rhi-sys`, `slang-rhi`, and Additive Reflection

- Deliverables:
  - The crate `slang-rhi-sys`, with raw bindings that `slang-bindgen`, extended to the slang-rhi headers, generates in its build script, and a build script that builds slang-rhi from the Slang tree that `slang-src` obtains.
  - The crate `slang-rhi`, with safe wrappers over `slang-rhi-sys`.
  - Additive reflection in `slang-compiler`: functions found by name, function modifiers such as `Differentiable`, function attributes, specialization with argument types, generics, and the metadata of cooperative types.

  The CMake build of slang-rhi is reported to download its dependencies when it configures, so how the build script of `slang-rhi-sys` obtains them is open (Section 16, A-20, and Section 17, Q-35). Whether the feature `rhi` of `slango` implies `vendored`, so that Slang and slang-rhi always come from one Slang tree, is also open (Section 17, Q-39).
- Owner decisions that bind it:
  - `slang-bindgen` is a build dependency of `slang-rhi-sys`, as it is of `slang-compiler-sys`, and `slang-rhi-sys` builds slang-rhi from the one Slang tree of `slang-src`.
  - The Vulkan, D3D12, and Metal device backends of slang-rhi are built by default on their platforms. CUDA with OptiX, and the CPU, are opt-in Cargo features, which M15 delivers.
- Replaces: ARCH-1 and the diagram of Section 3.2, which gain the two crates, and Section 3.3 with the list of crates in Hard Rule 7 of `AGENTS.md` that may hold unsafe code. It confirms or corrects A-15, A-16, and A-19 to A-21 (Section 16).
- Depends on: M7, whose `slang-bindgen` and `slang-src` it builds on. The roadmap places M9 after M8.
- Design document, which exists before any code of the milestone: `docs/design/slang-rhi-sys.md` and `docs/design/slang-rhi.md`, new, `docs/design/slang-compiler-sys.md`, revised for the generator, and `docs/design/slang-compiler.md`, revised for reflection.
- Exit criteria: every check passes, including the tests of both new crates in the GPU tier on the adapters that the design names. `AGENTS.md`, `README.md`, and this document are updated.

### 15.13. M10: The Opt-In slang-rhi Backend of `slango`

- Deliverables:
  - The slang-rhi backend of `slango` behind the opt-in Cargo feature `rhi`, at parity with the wgpu backend for compute kernels and vertex plus fragment pipelines.
  - `slango-macros` and `slango-codegen` reworked for the `rhi` backend.
  - The five existing examples on both backends.

  Which shader target the slang-rhi backend compiles to for each device backend is open (Section 17, Q-36).
- Owner decisions that bind it:
  - wgpu stays the default backend, behind the default feature `wgpu` of M8, and slang-rhi is opt-in through the feature `rhi`. M10 adds a backend and does not switch the default.
  - The wgpu backend keeps compute kernels and vertex plus fragment pipelines, stays free of unsafe code, and reports an unsupported error for every other pipeline kind.
  - `slang-slint` stays on wgpu. A Slint view shows compute output and vertex plus fragment output only, so ARCH-20 stays.
  - A breaking version 0.2 of `slango` is accepted if the `rhi` backend needs one (Section 15.11).
- Replaces: ARCH-15 with Section 8.2, and the parts of Sections 3.3, 8.5, and 8.6 that assume wgpu is the only backend. ARCH-19 then binds the wgpu feature.
- Depends on: M9.
- Design document, which exists before any code of the milestone: `docs/design/slango.md`, `docs/design/slango-macros.md`, and `docs/design/slang-slint.md`, revised.
- Exit criteria: every check passes, including the GPU tier on both backends on the adapters that the design names, and the five examples run on both backends. A build of `slango` with its default features builds no slang-rhi. `AGENTS.md`, `README.md`, and this document are updated.

### 15.14. M11: The Second Release

- Deliverables: the second release, of `slang-rhi-sys`, `slang-rhi`, `slang-slint`, and the versions of `slango`, `slango-codegen`, and `slango-macros` that carry the feature `rhi`, with their hosted documentation and the check for breaking changes extended to the new crates, after the owner's go-ahead (ARCH-24).
- Owner decisions that bind it: the second release follows once M10 has added the opt-in `rhi` backend to `slango`, and nothing is published without the owner's explicit go-ahead.
- Depends on: M10.
- Design document, which exists before any code of the milestone: the release procedure in `docs/design/tooling.md`, extended to these crates.
- Exit criteria: every check passes, the documentation link of each released crate leads to its hosted documentation, and the owner has approved the release. `AGENTS.md`, `README.md`, and this document are updated.

### 15.15. M12: Hull, Domain, and Geometry Stages

- Deliverables: render pipelines with hull, domain, and geometry stages on the slang-rhi backend, and example 7.
- Owner decisions that bind it: the new stages exist on the slang-rhi backend only, and the wgpu backend reports them as unsupported. A Slint view does not show them.
- Depends on: M10. The roadmap places M12 after M11.
- Design document, which exists before any code of the milestone: a design in `docs/design/slango.md`.
- Exit criteria: every check passes, including the tests of the new stages on the adapters that the design names and the automated check of example 7 (ARCH-21). `AGENTS.md`, `README.md`, and this document are updated.

### 15.16. M13: Mesh Shading

- Deliverables: mesh shading pipelines on the slang-rhi backend, and example 8.
- Owner decisions that bind it: mesh shading exists on the slang-rhi backend only, and the wgpu backend reports it as unsupported. A Slint view does not show it.
- Depends on: M10. The roadmap places M13 after M12.
- Design document, which exists before any code of the milestone: a design in `docs/design/slango.md`.
- Exit criteria: every check passes, including the tests of mesh shading on the adapters that the design names and the automated check of example 8 (ARCH-21). `AGENTS.md`, `README.md`, and this document are updated.

### 15.17. M14: Ray Tracing

- Deliverables: acceleration structures, ray tracing pipelines with shader tables and hit groups, and ray queries on the slang-rhi backend, with any support for hardware ray tracing that `slang-compiler-sys` and `slang-compiler` still lack. Example 6, the path-tracing demo, shows them.

  Slang is reported to offer no reflection of hit groups, so the design decides how a user describes them (Section 16, A-23).
- Owner decisions that bind it: ray tracing exists on the slang-rhi backend only, and the wgpu backend reports it as unsupported. A Slint view does not show it.
- Depends on: M9 and M10. The roadmap places M14 after M13.
- Design document, which exists before any code of the milestone: a design in `docs/design/slango.md`, which also designs example 6 and answers Q-29.
- Exit criteria: every check passes, including the tests of ray tracing on the adapters that the design names and the automated check of example 6 (ARCH-21). `AGENTS.md`, `README.md`, and this document are updated.

### 15.18. M15: The CUDA and CPU Backends

- Deliverables: the opt-in device backends of slang-rhi for CUDA with OptiX and for the CPU, as Cargo features of `slang-rhi-sys`, `slang-rhi`, and `slango`. OptiX runs the ray tracing interface of M14.
- Owner decisions that bind it: CUDA with OptiX, and the CPU, are opt-in Cargo features, while Vulkan, D3D12, and Metal stay the device backends built by default on their platforms when the feature `rhi` is on.
- Depends on: M14.
- Design document, which exists before any code of the milestone: designs in `docs/design/slang-rhi-sys.md` and `docs/design/slango.md`.
- Exit criteria: every check passes, including the tests of each new backend in the tier that the design names. `AGENTS.md`, `README.md`, and this document are updated.

### 15.19. M16: Generated Kernels, Automatic Differentiation, and Tensors

- Deliverables: kernels that `slango` generates around plain Slang functions, with broadcasting over array arguments, automatic differentiation, and tensors. Example 2 gains its broadcasting case and its differentiable function (Section 10).

  Slang is reported to offer no API that returns a derivative, so a derivative comes from compiling `bwd_diff(f)` or `fwd_diff(f)` in Slang source (Section 16, A-24). The design uses the reflection of functions that M9 adds.
- Owner decisions that bind it: none specific to this milestone. Which backends offer the feature is open (Section 17, Q-37).
- Depends on: M9 and M10. The roadmap places M16 after M15.
- Design document, which exists before any code of the milestone: a design in `docs/design/slango.md`, which confirms or corrects A-9.
- Exit criteria: every check passes, including the GPU tier tests of the new cases of example 2. `AGENTS.md`, `README.md`, and this document are updated.

### 15.20. M17: Neural Primitives

- Deliverables: cooperative vectors, the `slang.neural` module of Slang, and trainable primitives, with example 9, a training example. wgpu 30 is reported to offer no cooperative vectors (Section 16, A-22), and which backends offer the rest is open (Section 17, Q-37).
- Owner decisions that bind it: the neural scope is cooperative vectors (Slang's `CoopVec`), trainable primitives with a SlangPy-style interface of tensors, gradients of Slang functions, and an optimizer loop, and Slang's own `slang.neural` module. The recorded Slang marks `slang.neural` experimental (Section 16, A-26).
- Depends on: M16, and M9 for the metadata of cooperative types.
- Design document, which exists before any code of the milestone: a design in `docs/design/slango.md`.
- Exit criteria: every check passes, including the automated check of example 9 (ARCH-21). `AGENTS.md`, `README.md`, and this document are updated.

### 15.21. M18: Typed Bindings for the New Pipeline Kinds

- Deliverables: typed bindings of `slango-macros` for render pipelines with hull, domain, and geometry stages, for mesh shading pipelines, and for ray tracing pipelines.
- Owner decisions that bind it: the new pipeline kinds exist on the slang-rhi backend only, so their typed bindings do too.
- Depends on: M12, M13, and M14.
- Design document, which exists before any code of the milestone: `docs/design/slango-macros.md`, revised.
- Exit criteria: every check passes, including the tests that the design names for each pipeline kind. `AGENTS.md`, `README.md`, and this document are updated.

### 15.22. Later Items

These items are not scheduled and have no design yet:

- A shader target of `slango` configurable per backend, deferred out of M4 (Section 8.5, Section 17, Q-23).
- The custom file system of `slang-compiler`, deferred out of M2 because it serves hosts that keep shaders in a virtual file system and nothing on the roadmap is such a host (Section 8 of `docs/design/slang-compiler.md`).
- Further platforms.

Each one gets a design document when the owner schedules it.

### 15.23. Order Rationale

This subsection is non-normative. Up to M6, each milestone is the smallest step that the next one needs.

The bindings come before the safe crate that wraps them, the safe crate before the library that compiles through it, and compute before rendering, as Section 14.8 explains. Every milestone up to M3 runs headless, so CI tests each one fully before any work that needs a display begins.

From M7 on, compatibility impact sets the order. What a published version locks in comes before the release that publishes it, because a published version cannot be deleted and a later change to it is a breaking release. Additive work follows the releases.

Every user of the first release depends on how M7 makes `slang-compiler-sys` build, link, and record its Slang, and on the crates that M7 adds for a vendored build, so M7 precedes the first release in M8. The forward-compatibility pass of M8 comes before that release, so that the feature `rhi` of M10 can arrive in `slango` without a breaking change where that is cheap.

wgpu staying the default backend is what lets `slango` and its macro crates ship in the first release. The `rhi` backend of M10 adds a feature and leaves the default build as it is, so the first release does not wait for slang-rhi.

M9 and M10 add slang-rhi and the feature `rhi` of `slango` and its macro crates, so they precede the second release in M11. The reflection of M9 is additive, but M16 and M17 depend on it.

M12 to M18 add pipeline kinds, backends, and features without breaking the released API. M15 follows M14 because OptiX runs the ray tracing interface of M14, and comes before M16 because training workloads commonly run on CUDA. M18 comes last, because its typed bindings wrap the pipeline kinds of M12 to M14.

## 16. Assumptions to Confirm

Each row is a fact that comes from research that was not independently confirmed. A section that relies on one of them cites its identifier.

The design document or milestone in the last column confirms or corrects it, and the change that does so removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| A-6 | The build script of a dependency cannot set the rpath of the final binary | Section 4.4 | Section 10 of `docs/design/slang-compiler-sys.md`, if a change of it proposes an rpath. The recipes put the library path on the command line instead (SCS-32), which does not rely on A-6 |
| A-9 | SlangPy's portable core is kernels generated from reflection and cached per call signature, broadcasting with an explicit `map()`, a unified tensor type, automatic differentiation through `bwds`, and a device layer | Sections 8.1 and 14.8 | The design of M16, which adds generated kernels. Section 3.7 of `docs/design/slango.md` finds it consistent with SlangPy's documentation |
| A-11 | wgpu offers a shader passthrough path for targets other than SPIR-V | Section 8.5 | The design of the configurable shader target. Section 3.2 of `docs/design/slango.md` confirms the path for SPIR-V |
| A-14 | Slint has no public Vulkan hook, and its wgpu API can change in minor releases | Section 9 | The design of a change that widens the Slint range of SLS-1 of `docs/design/slang-slint.md` or needs a Vulkan hook |
| A-15 | slang-rhi publishes no releases or prebuilt binaries and needs a C++20 build with CMake | Section 14.4 | The design of M9 |
| A-16 | A slang-rhi backend would add CUDA, CPU, and OptiX targets | Sections 14.4 and 15.18 | The design of M9 |
| A-19 | slang-rhi is the submodule `external/slang-rhi` of the Slang tree, so the Slang commit determines it, and the Vulkan SDK does not ship it, so it is always built from source | Section 14.4 | The design of M9 |
| A-20 | The CMake build of slang-rhi downloads dependencies when it configures, each verified with a SHA-256 checksum: DXC, the Agility SDK, and NVAPI on Windows, metal-cpp on macOS, the OptiX headers for CUDA, and Dawn for its WebGPU backend | Section 15.12 | The design of M9 |
| A-21 | slang-rhi offers ray tracing pipelines on Vulkan, D3D12, and CUDA through OptiX, and ray queries only on Metal. It offers hull, domain, and geometry stages and mesh shading on Vulkan and D3D12, and cooperative vectors on Vulkan, CUDA, and D3D12 through NVAPI | Section 14.4 | The design of M9 |
| A-22 | wgpu 30 offers no tessellation, geometry stage, or cooperative vectors, and offers ray tracing pipelines and mesh shaders only as experimental features | Sections 14.4 and 15.20 | The design of M10 |
| A-23 | Slang offers no reflection of hit groups | Section 15.17 | The design of M14 |
| A-24 | Slang offers no API that returns a derivative, so a derivative comes from compiling `bwd_diff(f)` or `fwd_diff(f)` in Slang source | Section 15.19 | The design of M16 |
| A-25 | docs.rs builds crates without network access | Section 13 | The design of M8 |
| A-26 | The recorded Slang holds the module `slang.neural` under `source/standard-modules/neural`, marks it experimental, and needs the experimental feature flag to use it | Section 15.20 | The design of M17 |
| A-28 | On Windows and macOS, the build script of `slang-src` also finds the shared folder under `--target` and with a separate build directory, and every host and target unit of a project share one native build | Section 4.3 | A vendored build on Windows and macOS with `--target` and with a native build (SRCA-9 of `docs/design/slang-src.md`) |

## 17. Open Questions

Each row is a question that this document leaves open. The design document in the third column answers it before or during the milestone in the fourth column, and the change that answers it removes the row. An identifier is never reused.

| Identifier | Question | Owning design document | Milestone |
| --- | --- | --- | --- |
| Q-16 | Does each crate other than `slang-compiler`, `slango`, `slang-slint`, `slango-macros`, and `slango-codegen` emit logs, and through which crate? | Each later crate's design document | The milestone of each later crate |
| Q-19 | Which unsafe code would `slango` need for shader passthrough or another later feature, and why? | The design of the configurable shader target | The milestone of that target |
| Q-23 | How is the shader target configured per backend? | The design of the configurable shader target | Deferred with no milestone assigned (Section 15.22) |
| Q-24 | Can the windows of examples 3 and 4 be checked headless, and how? | A design document, written when a CI runner can show a window | Not scheduled |
| Q-29 | What is the design of the path-tracing demo? | The design of M14 | M14 |
| Q-35 | How does the build script of `slang-rhi-sys` obtain the dependencies that the CMake build of slang-rhi downloads, and how does an offline or sandboxed build get them? | `docs/design/slang-rhi-sys.md` | M9 |
| Q-36 | Which shader target does the slang-rhi backend of `slango` compile to for each device backend, and does the answer settle Q-23? | `docs/design/slango.md` | M10 |
| Q-37 | Which backends of `slango` offer generated kernels, automatic differentiation, tensors, and the neural primitives? | `docs/design/slango.md` | M16 and M17 |
| Q-39 | Does the feature `rhi` of `slango` imply `vendored`, so that Slang and slang-rhi always come from one Slang tree? | `docs/design/slang-rhi-sys.md` | M9 |

Q-32, whether the in-process loader of `slang-glslang-sys` gives a vendored build optimized SPIR-V on Linux, Windows, and macOS, is answered yes for the hosted runners of the three platforms by the vendored tests of `slang-compiler` and `slang-glslang-sys` (Section 9 of `docs/design/slang-glslang-sys.md`).

## 18. Security Considerations

The threat model has five parts:

1. An application passes shader source, module names, include paths, and module search paths that an attacker controls to `slang-compiler` or `slango`. The attacker chooses their content but not the code of the application or of these crates.
2. An agent introduces a defect into unsafe code or into a generated declaration, which turns a malformed input or a misuse into undefined behaviour.
3. The build scripts of `slang-compiler-sys`, `slang-src`, and `slang-glslang-sys` run on the user's machine with the user's permissions and read environment variables. Every build runs the binding generator with libclang. On the vendored route, `slang-src` clones Slang over the network, and the other two run CMake and native builds on the source tree.
4. A dependency, whether a crate or the native library, contains a defect or a published vulnerability.
5. A CI workflow downloads the Vulkan SDK (Section 7.3 of `docs/design/tooling.md`) and other tools, and an attacker on the network or at the download host tampers with them.

The design addresses each part, mostly through the rules that Section 20 of the coding standard summarizes:

- Untrusted input: `slang-compiler` returns errors instead of panicking (ERR-5), and its design document states which files Slang can read through include paths and module search paths. The crates treat text passed to Slang as trusted input and do not guard against defects of Slang (Section 1.6 of `docs/design/slang-compiler.md`).

  Slang reads standard modules, the files of some options, and downstream compilers directly from the operating system. An application that compiles untrusted source and needs isolation therefore runs the compiler in a separate process (Sections 8.6 and 17 of `docs/design/slang-compiler.md`).
- Unsafe code: the crates rely on the native library matching the bindings, which the generation from the headers of the linked Slang (ARCH-33) and the run-time check (ARCH-34) secure, and on the reference counting and lifetime contracts of the Slang API, which Section 12 of the coding standard constrains. Unsafe code stays in three crates (Section 3.3), and generated declarations are never written by hand (ARCH-33, COM-9).
- Build scripts: only `slang-src` reaches the network, to clone the recorded commit by its full hash over HTTPS only, without tags or history, and no build script writes outside `OUT_DIR` except into the shared folder in the Cargo target directory (ARCH-35, Section 14 of `docs/design/slang-src.md`). ARCH-27 keeps the vendored build out of the source tree, and ARCH-5 and ARCH-28 stop the build instead of searching for a library elsewhere or falling back.
- Dependencies: the exact version of `slang-src` fixes the recorded commit for each version of the crates that depend on it, the dynamic route links whichever SDK the user installed, and cargo-deny checks the advisories, licences, and sources of every crate, including the dependencies of the generator, which now compile on every user's machine.
- CI: every download whose upstream publishes a SHA-256 checksum is verified against it, as Hard Rule 7 of `AGENTS.md` requires, and `docs/design/tooling.md` specifies how. No file in the repository contains a secret.

These attacks are out of scope:

- A malicious or compromised Slang library or Vulkan SDK runs with the full privileges of the application, so no crate here can constrain it.
- A commit that the user records in a fork of `slang-src` and patches in through `[patch]` is code that the user chose to build, and CMake runs it with the user's permissions, so the crates cannot defend the user against it.
- An attacker who can write into the Cargo target directory can place any library in the shared folder, and can already replace any artifact that Cargo builds there.
- Defects in GPU drivers are outside every crate here.
- Attacks on the repository itself are a matter of access control.

The residual risk has five parts:

1. Slang is a large C++ code base that parses shader source inside the application's process. A memory-safety defect in Slang can be reached by an attacker's shader source, and shader source can make the compiler use unbounded time or memory unless the application limits it.
2. The soundness of the unsafe code rests on judgement rules and on contracts in the Slang documentation that no tool verifies, and Miri cannot run the native library.
3. The custom checkers of `docs/design/tooling.md` do not read every form of documentation or workflow. The rules or parts of rules that no tool checks depend on review, as Section 13 of `docs/design/tooling.md` states.
4. A shader from an untrusted source can abort the process. When its types nest past Slang's limit of 128 levels, a C++ exception escapes Slang, and `slang-compiler` aborts the process on purpose instead of returning an error (Section 11.6 of `docs/design/slang-compiler.md`). An application that must survive such a shader compiles in a separate process.
5. A vendored build trusts its clone by a git commit hash, which is a SHA-1 hash with detection of the known collision attacks (Section 14 of `docs/design/slang-src.md`), and a Slang other than the recorded one is best effort, so its bindings match it but the safe crates are not tested against it (Section 5.3).

## 19. Operational Considerations

### 19.1. Cargo Features

This table summarizes the Cargo feature `vendored`, which selects the vendored route (Section 4.1). It is off by default in every crate that has it. Each crate's design document defines its own features, and the rows cite them.

| Crate | Feature `vendored` |
| --- | --- |
| `slang-compiler-sys` | Enables the optional dependency `slang-src` and selects the vendored route (SCS-50 of `docs/design/slang-compiler-sys.md`) |
| `slang-compiler` | Enables the feature of `slang-compiler-sys` and the dependency `slang-glslang-sys`, and installs its loader on every global session (SGL-12 of `docs/design/slang-glslang-sys.md`) |
| `slango`, `slango-codegen`, and `slang-slint` | Forwards the feature to the crate below |
| `slango-macros` | Forwards the feature to the copy of Slang that the macro links on the host, whose features Cargo resolves apart from the program's copy, so a program without a Vulkan SDK at build time enables it on both `slango` and `slango-macros`. Both copies share the shared folder (Section 8 of `docs/design/slang-src.md`) |

`slang-src`, `slang-glslang-sys`, and `slang-bindgen` have no Cargo features. `--all-features` enables `vendored`, so the recipes pass an explicit list of features (`docs/design/tooling.md`).

### 19.2. Environment Variables

This table lists every environment variable that a user sets to control how Slang is obtained, with its behaviour when unset. `slang-src` has no environment variable of its own (SRC-21 of `docs/design/slang-src.md`). The design document of each crate lists every variable that its build script reads.

| Variable | Read by | Behaviour when unset |
| --- | --- | --- |
| `VULKAN_SDK` | The build script of `slang-compiler-sys` on the dynamic route. The vendored route does not read it. | The build stops with an error that names the variable and the feature `vendored` (ARCH-5) |
| `LIBCLANG_PATH` | The binding generator, in every build | The generator searches the standard locations of libclang, and the build stops with advice when it finds none (Section 7.7 of `docs/design/slang-compiler-sys.md`) |
| `CARGO_NET_OFFLINE` | The build script of `slang-src` | `slang-src` may clone. When `true`, it reaches no network and stops a build that would need to clone (SRC-9 of `docs/design/slang-src.md`) |
| `CARGO_TARGET_DIR` | Cargo, which places the shared folder in the target directory | The target directory of the workspace. Two projects that set it to one directory share one shared folder (Section 9.1 of `docs/design/slang-src.md`) |

### 19.3. Platforms and Minimum Supported Rust Version

Section 12 lists the tested platforms and graphics APIs, and the tier of every other 64-bit target. The workspace declares its minimum supported Rust version (MSRV) in `rust-version`, as DEP-4 of the coding standard requires, and `docs/design/tooling.md` sets its value and the check that verifies it.

### 19.4. Failure Modes

This table lists the failure modes that this document defines, how each is reported, and the recovery.

| Failure | Reported by | Recovery |
| --- | --- | --- |
| No Vulkan SDK on the dynamic route | A build script error that names `VULKAN_SDK` and the feature `vendored` (ARCH-5) | Install the Vulkan SDK and set `VULKAN_SDK`, or enable `vendored` |
| No libclang | A build script error of the generator that names `LIBCLANG_PATH` (Section 9.6 of `docs/design/slang-compiler-sys.md`) | Install libclang or set `LIBCLANG_PATH` |
| The headers of another Slang hold declarations or constructs that the generator's rule set does not cover | A build script error that names each one (Section 9.6 of `docs/design/slang-compiler-sys.md`) | Use the SDK that ships the recorded Slang, or enable `vendored` without a fork of `slang-src` (Section 5.3) |
| No source tree on the vendored route, such as without network access and without a clone in the shared folder | A build script error of `slang-src` that says that a build without `vendored` needs no network access (ARCH-28) | Build once with network access in the same target directory, or build without `vendored` |
| The target differs from the host on the vendored route, or has no link rule for its C++ runtime library | A build script error that names the target triples (Section 9.6 of `docs/design/slang-compiler-sys.md`) | Build on a host of the target, or build without `vendored` |
| The target is not a tested target | A warning of the build script of `slang-compiler-sys`, which Cargo shows when the build fails, and the build information (ARCH-37) | None needed. The build goes on, unsupported |
| The native build of Slang or slang-glslang fails | A build script error that names the command, its exit status, and its log (SYS-4) | Fix the toolchain that the error names |
| The Slang loaded at run time is not the Slang of the build information | An error of `slang-compiler` when it creates a global session (ARCH-34) | Make the dynamic loader find the library of the SDK that the build used |
| The shared library is not found at run time | The operating system's dynamic loader, which stops the program before `main` runs | Run the SDK's setup script on Linux or macOS, or add the SDK to `PATH` on Windows (Section 4.4) |
| Slang rejects a shader | The `Error` of `slang-compiler`, with the diagnostic text (ERR-1, COM-8) | Fix the shader source that the diagnostics name |
| No adapter is found | `ErrorKind::NoAdapter` of `slango` (Section 10 of `docs/design/slango.md`). A test in a GPU tier fails (ARCH-23). | Run on a machine with an adapter, such as lavapipe |

### 19.5. Logs

This document defines no logs. No crate of the workspace emits logs or has a logging dependency (Section 18.5 of `docs/design/slang-compiler.md`, Section 17.5 of `docs/design/slango.md`, SLS-4 of `docs/design/slang-slint.md`, and SLM-4 of `docs/design/slango-macros.md`). Whether each later crate emits logs is open (Section 17, Q-16).

## 20. References

### 20.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 20.2. Informative References

- [SLANG-RS-26] "Objects must outlive Session?", issue 26 of the FloatyMonkey/slang-rs repository on GitHub, <https://github.com/FloatyMonkey/slang-rs/issues/26>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary. The labels ARCH-2 and ARCH-6 to ARCH-13 belonged to requirements that the design of M7 replaced, and they are not reused.

| Label | Section | Summary |
| --- | --- | --- |
| ARCH-1 | 3.2 | Library crates depend only on the crates that a path of arrows in the diagram reaches from them. |
| ARCH-3 | 3.4 | A crate's design document is the first task of its milestone and exists before any code. |
| ARCH-4 | 4.2 | The dynamic route reads `VULKAN_SDK`, generates the bindings from its headers, records their build tag, and links its library dynamically. |
| ARCH-5 | 4.2 | On the dynamic route without `VULKAN_SDK`, the build stops with an error that names it and `vendored`, and searches nowhere else. |
| ARCH-14 | 7 | `slang-compiler` depends on no graphics API crate. |
| ARCH-15 | 8.2 | `slango` keeps backend types internal and wgpu types in one separate public module. |
| ARCH-16 | 8.3 | `slango` does not adopt vulkano's `GpuFuture` model. |
| ARCH-17 | 8.4 | `slango-macros` generates code that calls the same core as the runtime API. |
| ARCH-18 | 8.5 | M3 starts with a SPIR-V spike on lavapipe, recorded before the API is designed. |
| ARCH-19 | 8.6 | `slango` uses a wgpu major version that the current Slint release supports. |
| ARCH-20 | 9 | `slang-slint` supports wgpu only and pins a narrow range of Slint versions. |
| ARCH-21 | 10 | Each example has at least one automated check. |
| ARCH-22 | 11 | Tests that need an adapter sit in a `gpu` or `hw_gpu` module. |
| ARCH-23 | 11 | A test in a `gpu` or `hw_gpu` module fails, and never skips, when it finds no adapter. |
| ARCH-24 | 13 | No crate is published without the owner's explicit go-ahead. |
| ARCH-25 | 15.1 | Work on a milestone's code starts only once its design document exists, and the milestone ends only when its exit criteria hold. |
| ARCH-26 | 3.2 | No library crate depends on an example, and only a `-sys` crate depends on `slang-bindgen`, as a build or development dependency. |
| ARCH-27 | 4.3 | With `vendored`, Slang is built with CMake from a copy of the tree of `slang-src`, inside `OUT_DIR` or the shared folder, with bindings from that build, and linked statically. |
| ARCH-28 | 4.3 | The vendored route never falls back to the dynamic route and never reads `VULKAN_SDK`. |
| ARCH-29 | 5.1 | The recorded commit lives only in `slang-src`, the SDK version of CI only in the CI configuration, and CI fails when their Slang differs. |
| ARCH-30 | 5.2 | Moving the recorded Slang updates the CI SDK, `slang-src`, and its exact requirements, fixes the breakage, and runs every check in one change. |
| ARCH-31 | 5.3 | The crates target only the recorded Slang, and nothing relaxes the generator or the run-time check for another version. |
| ARCH-32 | 5.2 | A release after a move of the recorded Slang is a new minor of `slang-src`, a release of its dependents, and a breaking release of `slang-compiler-sys` unless the bindings are identical. |
| ARCH-33 | 6.2 | The binding generator produces every generated item from the headers of the Slang that the same build links. |
| ARCH-34 | 5.1 | `slang-compiler` checks that the loaded Slang reports the build tag of the build information. |
| ARCH-35 | 4.3 | Only `slang-src` reaches the network during a build, only to clone the recorded commit by its hash, and no build script writes outside `OUT_DIR` except into the shared folder in the Cargo target directory. |
| ARCH-36 | 13 | Every crate carries the licence of its kind from its creation. |
| ARCH-37 | 12 | An untested target never stops a build and gets a statement that it is untested and unsupported, and a build stops for its target only where a correct build is impossible. |
