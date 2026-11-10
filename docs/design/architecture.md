# Architecture Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-10 |
| Applies to | The whole workspace |

## Abstract

This document specifies the architecture of slang-rust, a set of Rust crates for using the Slang shader language and its compiler. It defines the crates and the direction of the dependencies between them, how the native Slang library is obtained and which version is pinned, what each crate is responsible for and what it must not do, the examples that also serve as the integration testbed, the tiers of tests that need a graphics processing unit, the supported platforms, and the plan for publishing. It records each architectural decision with the alternatives considered and the reason. It sets out the roadmap as milestones M0 to M7, each with deliverables, dependencies, exit criteria, and the design document that must exist before work on its code starts. It lists the facts that are still to be confirmed and the open questions that each later design document must answer. No crate of this design is implemented yet.

## 1. Introduction

### 1.1. Problem

Slang is a shader language and compiler with a C++ application programming interface (API) in the style of the Component Object Model (COM). Rust code that uses it today has to call that API through raw pointers and vtables, manage reference counts by hand, and keep each object alive for as long as any object created from it lives. It then has to load the compiled code into a graphics API and pass arguments to it, which takes many more steps. The one existing third-party Rust binding writes its vtables by hand and has a reported soundness issue in which objects crash when they outlive their session [SLANG-RS-26].

### 1.2. Purpose

The workspace provides a layered set of crates. The lowest layer binds the native library exactly and safely enough to build on. The middle layer gives a safe, idiomatic compiler API. The upper layers make calling Slang code from Rust as simple as SlangPy makes it from Python, and let a Slint user interface show content that a Slang render pipeline draws. This document fixes the structure that all of these crates share, so that each crate's own design document can be written against it.

### 1.3. Audience

The primary readers are the coding agents that implement a milestone or write a per-crate design document. The secondary readers are the reviewers who check that a change stays within this architecture, and the owner of the repository, who approves its decisions.

### 1.4. Scope

This document covers the crates and their layering, how Slang is obtained and pinned, the responsibilities and limits of each crate, the examples, the test tiers, platform support, publishing, the roadmap, the assumptions to confirm, and the open questions. It applies to every crate, tool, and example in the workspace.

### 1.5. Non-Goals

This document does not specify the public API of any crate. Each crate's design document does that, as Section 3.4 requires. It does not specify the workspace configuration, the lint configuration, the `just` recipes, the nextest profiles, the continuous integration (CI) workflows, or the requirements on the development container image. The document `docs/design/tooling.md` owns those topics. It does not design a slang-rhi backend or the path-tracing demo, which Section 15.11 lists as later items. It does not restate the rules of `docs/rust-coding-standards.md`, which it cites by rule identifier.

### 1.6. Current State

The repository holds the standards documents and the repository tooling, but no crate of this design. Milestone M0 is complete (Section 15.3), and M1 is next. M0 replaced the abandoned sketch in `crates/slang-compiler-sys` with a placeholder crate that has documentation and no API, and it added the workspace lint configuration, the configuration of the other tools, the recipes that run the Rust checks, the custom checkers of `docs/design/tooling.md`, the CI workflow, the licence file, and `README.md`. The Project Status section of `AGENTS.md` describes the current state in full.

The development container provides the LunarG Vulkan software development kit (SDK) 1.4.363.1, the Slang source tree at the same commit, and two Vulkan devices. The Technology Decisions section of `AGENTS.md` lists the container's contents in full.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that Section 2.2 of `docs/writing-documentation.md` defines, such as agent, role, user, and design document. It also uses the terms that Section 2.2 of `docs/rust-coding-standards.md` defines, such as binding generator, `-sys` crate, safe crate, library crate, native library, interface, vtable, thunk, interface pointer, result code, blob, diagnostics blob, and universally unique identifier (UUID). This document calls those two files the writing guide and the coding standard. It adds these terms:

- adapter: The wgpu name for one implementation of a graphics API that a program can open a device on, such as a hardware GPU or a software implementation such as lavapipe.
- application binary interface (ABI): The layout of types and the calling conventions that compiled code relies on. A library whose ABI is not stable can change these between versions without a change in its source API.
- automatic differentiation: A Slang feature that derives the derivative of a function from its source code.
- broadcasting: Calling a function written for single values over arrays of values, where an argument with fewer dimensions is repeated across the others, as in NumPy.
- buffer: A block of GPU memory that holds data for a shader.
- CMake: A build system generator for C and C++ projects. Slang builds with it.
- COM-style interface: An interface in the sense of Section 2.2 of the coding standard. COM is a binary convention for objects in which every object counts its references through `AddRef` and `Release`, and a caller selects one of the object's interfaces through `QueryInterface` with a UUID.
- component type: Slang's name for a unit of code that can be composed with others and linked, such as a module or an entry point.
- compute dispatch: One GPU command that runs a compute shader over a grid of threads.
- compute kernel: A function that runs on the GPU in many threads at once, started by a compute dispatch.
- crates.io: The public registry of Rust crates.
- CUDA: NVIDIA's platform and API for computing on its GPUs.
- device: An open connection to an adapter, through which a program creates GPU resources and submits work.
- Direct3D 12 (D3D12): The graphics API of Windows.
- docs.rs: The service that builds and hosts the API documentation of every crate published on crates.io.
- dynamic route: The default way of obtaining Slang, which links dynamically to the Slang of an installed Vulkan SDK (Section 4.2).
- entry point: A function in Slang source that a pipeline starts, such as the main function of a compute shader.
- global session: The Slang object that holds the shared state of the compiler, from which sessions are created.
- graphics processing unit (GPU): A processor for parallel graphics and compute work.
- headless: Running without a window or a display.
- hot reload: Recompiling a shader and replacing it in the running program when its source file changes.
- lavapipe: The software implementation of Vulkan in the Mesa project.
- libclang: The C interface to the parser of the Clang compiler, which reads C and C++ headers.
- marshalling: Converting a Rust value into the memory layout that a shader parameter expects, and converting a result back.
- Metal: The graphics API of macOS. Metal Shading Language (MSL) is its shader language.
- milestone: One step of the roadmap in Section 15, identified as M0 to M7.
- Miri: An interpreter for Rust that detects some undefined behaviour in unsafe code. It runs on the nightly toolchain and cannot call native code.
- module: One file of Slang source, like a Rust module.
- nextest profile: A named set of settings in the configuration of cargo-nextest, such as a filter that selects tests.
- OptiX: NVIDIA's ray-tracing API, which runs on CUDA.
- pkg-config: A tool that reports the compiler and linker flags of an installed library from a `.pc` file.
- pin: The one Slang build that the workspace targets, named by a Vulkan SDK version together with a Slang commit (Section 5.1).
- procedural macro: A Rust macro, compiled as a crate of its own, that runs during the compilation of the code that uses it and generates Rust code.
- reflection: Slang's API for querying the parameters, types, layouts, functions, and entry points of compiled Slang code.
- render pipeline: A GPU object that combines the shaders and the fixed settings that draw an image.
- run-time search path (rpath): A list of directories, stored in a binary, where the dynamic loader of the operating system looks for shared libraries.
- session: A Slang object that holds the settings of one compilation context, such as its targets and search paths, and loads modules.
- shader: A program that runs on a GPU.
- shader passthrough: A path in wgpu that passes shader code in the backend's own format to the backend without translating it.
- shader target: The output format that Slang compiles a shader to, such as SPIR-V or MSL.
- Slint: A Rust toolkit for graphical user interfaces.
- slang-rhi: Slang's render hardware interface, a C++ library that gives one API over several graphics and compute APIs.
- SlangPy: A Python library that calls Slang functions on the GPU from Python.
- smoke test: A short test that shows that the basic path works, such as creating one object and reading one value.
- software adapter: An adapter that runs on the central processing unit (CPU), such as lavapipe or WARP.
- SPIR-V: The binary intermediate shader format of Vulkan.
- spike: A short experiment whose result is a recorded finding, not shipped code.
- static route: The opt-in way of obtaining Slang, which builds Slang from source and links it statically (Section 4.3).
- tensor: A multi-dimensional array of elements in GPU memory.
- Vulkan: A cross-platform graphics and compute API.
- Vulkan SDK: The LunarG Vulkan software development kit (SDK), which ships Vulkan tools, headers, and libraries, including Slang.
- vulkano: A Rust library that wraps Vulkan in a safe API. Its companion crate vulkano-shaders generates typed Rust code from shaders at build time.
- Windows Advanced Rasterization Platform (WARP): The software implementation of D3D12 on Windows.
- wgpu: A Rust graphics library with one API over Vulkan, Metal, D3D12, and other backends. wgpu-hal is its lower-level hardware abstraction layer.
- winit: A Rust library that creates windows and handles their events.

## 3. Crates and Layering

### 3.1. Workspace Members

This table lists the planned members of the workspace. The crate names are decided. Section 3.2 of `docs/design/tooling.md` confirms the directories, decides that the examples are one workspace member, and names the skeleton crate that milestone M0 creates.

| Member | Directory | Responsibility | Published | Milestone |
| --- | --- | --- | --- | --- |
| `slang-compiler-sys` | `crates/slang-compiler-sys/` | Raw bindings and the build script that links Slang (Section 6) | Yes | M1 |
| `slang-compiler` | `crates/slang-compiler/` | Safe wrappers over the compile and reflection API (Section 7) | Yes | M2 |
| `slango` | `crates/slango/` | High-level library for calling Slang code on the GPU (Section 8) | Yes | M3, M4 |
| `slang-slint` | `crates/slang-slint/` | A Slint view backed by a Slang render pipeline (Section 9) | Yes | M5 |
| `slango-macros` | `crates/slango-macros/` | Typed Rust bindings to Slang code, generated at build time (Section 8.4) | Yes | M6 |
| Binding generator | `tools/bindings-generator/` | Writes the raw bindings of `slang-compiler-sys` (Section 6.2) | No, `publish = false` | M1 |
| Examples | `examples/` | Example programs that also serve as the integration testbed (Section 10) | No, `publish = false` | M2 to M5 |

### 3.2. Dependency Direction

Dependencies between the library crates point strictly downward, as this diagram shows. An arrow points from a crate to a crate that it depends on. `slango-macros` is a procedural macro crate. `slango` re-exports it behind an optional feature, which is a proposed arrangement that the design document of `slango-macros` confirms (Section 17, Q-26). `slango-macros` uses `slang-compiler` at build time to compile and reflect the `.slang` file that the macro reads. `slango` also depends on `slang-compiler` directly. The examples depend on whichever library crate they demonstrate.

```text
  slang-slint                 Slint view, wgpu only
       |
       v
    slango                    high-level library, wgpu first
       |    \
       |     v
       |    slango-macros     procedural macro, re-exported by slango
       |     |                behind an optional feature (proposed)
       |     |                uses slang-compiler at build time
       v     v
  slang-compiler              safe compile and reflection API
       |
       v
  slang-compiler-sys  ------> native Slang library
                              (Vulkan SDK, or built from source)

  binding generator  -------> writes the checked-in bindings of slang-compiler-sys
                              (no crate depends on it)
```

ARCH-1. A library crate MUST depend only on the library crates below it in the diagram of this section, which are the crates that a path of arrows from it reaches, because an upward or circular dependency forces a lower crate to change and be released whenever a higher one does, and makes the lower crate unusable on its own.

ARCH-2. A library crate MUST NOT depend on the binding generator or on an example, at build time or otherwise, because the generator needs libclang, and the checked-in bindings exist so that users need neither libclang nor the generator (Section 14.2).

### 3.3. Unsafe Code

Unsafe code appears only in `slang-compiler-sys` and `slang-compiler`, as Hard Rule 7 of `AGENTS.md` and Section 4.4 of the coding standard state. `slango` is expected to need unsafe code for wgpu shader passthrough (Section 16, A-11), and possibly for interoperation with Slint or wgpu-hal. Each such need follows the procedure of Section 4.4 of the coding standard, and the design document of `slango` or `slang-slint` decides it (Section 17, Q-19).

### 3.4. Design Documents per Crate

Each crate gets its own design document at `docs/design/<crate-name>.md`, such as `docs/design/slang-compiler.md`. The design document of `slang-compiler-sys` also covers the binding generator, because the two change together. The design document of each crate answers the open questions that Section 17 assigns to it.

ARCH-3. An agent MUST write the design document of a crate as the first task of the milestone that delivers the crate, so that the document exists before work on any code of that milestone starts, because a reviewer checks code against its design document, and code written first fixes decisions that nobody has reviewed. The agent writes it no earlier than that milestone, because a design document written long before its milestone describes a context that has changed by the time the code is written.

## 4. Obtaining Slang

### 4.1. Two Routes

Slang is obtained in exactly one of two ways, as rule SYS-7 of the coding standard requires: the dynamic route by default, and the static route as an opt-in. There is no prebuilt download and no other way. Section 14.6 gives the reason.

| Route | Selected by | Slang comes from | Linking | The user needs |
| --- | --- | --- | --- | --- |
| Dynamic | Default | An installed LunarG Vulkan SDK at `VULKAN_SDK` | Dynamic | The pinned Vulkan SDK at build time and at run time |
| Static | The proposed Cargo feature `static`, a mechanism that is open (Section 17, Q-7) | A Slang source tree at the pinned commit, named by `SLANG_SOURCE_DIR` (proposed) | Static | The source tree, CMake, and a C++ toolchain at build time |

This document calls the proposed selector of the static route the `static` feature. Whether a Cargo feature or the environment selects the static route is open, and so are the names `static` and `SLANG_SOURCE_DIR`. The design document of `slang-compiler-sys` decides them (Section 17, Q-7 and Q-8).

### 4.2. Dynamic Route

ARCH-4. The build script of `slang-compiler-sys` MUST, on the dynamic route, read `VULKAN_SDK` when it runs, find the Slang headers under `$VULKAN_SDK/include/slang` and the library `slang-compiler` under `$VULKAN_SDK/lib`, and link that library dynamically, because the SDK is the only source of Slang that the default route allows (SYS-7). The build script also checks the version that it finds, as SYS-6 requires (Section 17, Q-1).

ARCH-5. The build script of `slang-compiler-sys` MUST stop the build with an error that names `VULKAN_SDK` and the static route when `VULKAN_SDK` is unset, the `static` feature is off, and `DOCS_RS` is unset, and MUST NOT search other locations, because a search outside the two routes breaks SYS-7, and a build that continues without Slang fails later with a link error that does not say what is missing.

In the development container, the Vulkan SDK provides the headers under `$VULKAN_SDK/include/slang`, the shared library `libslang-compiler.so` under `$VULKAN_SDK/lib`, a CMake package configuration, and the pkg-config file `slang-compiler.pc`. These facts were checked in the container. The layouts on Windows and macOS are not known yet (Section 16, A-8, and Section 17, Q-4), and whether the build script uses pkg-config is open (Section 17, Q-5).

### 4.3. Static Route

ARCH-6. The build script of `slang-compiler-sys` MUST, when the `static` feature is on, build Slang with CMake and the setting `SLANG_LIB_TYPE=STATIC` from the source tree that `SLANG_SOURCE_DIR` names, with all build output inside `OUT_DIR`, and link the result statically, because a build script downloads nothing and writes only inside `OUT_DIR` (SYS-5), so the user supplies the source, and a build inside the source tree would change the user's files.

ARCH-7. The build script of `slang-compiler-sys` MUST stop the build with an error that names `SLANG_SOURCE_DIR` when `DOCS_RS` is unset, the `static` feature is on, and the variable is unset or does not name a directory, and MUST NOT fall back to the dynamic route, because a user who asked for a static build and silently gets a dynamic one ships a binary that fails on a machine without the SDK.

A static build of Slang is reported to need Slang's internal static libraries, compiler-core and core, as well as miniz and lz4 (Section 16, A-3). The exact list is open (Section 17, Q-3), and so is how the build script verifies the commit of the source tree (Section 17, Q-2). Whether the static route also reads `VULKAN_SDK` is open (Section 17, Q-9). In the development container, the source tree is at `/opt/slang`, at the pinned commit, and it is source only, not built.

This illustration shows how a user might select the static route. The feature and variable names are proposed and may change:

```sh
SLANG_SOURCE_DIR=/tmp/slang cargo build --features slang-compiler-sys/static
```

### 4.4. Finding the Shared Library at Run Time

On the dynamic route, the operating system's dynamic loader has to find the Slang shared library when the program starts. That is the job of the user's environment: the SDK's setup script on Linux and macOS, and `PATH` on Windows. The build script of a dependency is understood to have no way to set the rpath of the final binary (Section 16, A-6). The crate-level documentation of `slang-compiler-sys` says how the library is found on each platform, as Section 7.4 of the writing guide requires.

In the development container, a login shell has `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH`, and a process that no login shell started does not. The next rebuild of the container image fixes this, and `docs/design/tooling.md` lists the fix among the image requirements.

### 4.5. Documentation Builds Without Slang

ARCH-8. The build script of `slang-compiler-sys` MUST skip finding Slang, building it, and linking it when the environment variable `DOCS_RS` is set, on either route and whatever features are enabled, because docs.rs builds the documentation of a published crate on a machine without Slang, and a build script that fails there leaves the crate without published documentation. ARCH-8 takes precedence over ARCH-4 to ARCH-7, which apply only when `DOCS_RS` is unset. The checked-in bindings let the crate document itself without Slang (Section 14.2). That docs.rs sets `DOCS_RS` is an assumption to confirm (Section 16, A-7).

## 5. Slang Version Policy

### 5.1. Version Target and Pin

The workspace targets the Slang that ships in the latest stable LunarG Vulkan SDK. The first pin is Vulkan SDK 1.4.363.1, whose `slangc -version` prints `2026.17-25-gc7954ebd1`, which is 25 commits past the tag `v2026.17`. The pin is therefore the SDK version together with the Slang commit, not a Slang tag. These facts were checked in the development container. Section 14.5 gives the reason for this target.

In the development container, this command prints the Slang version of the installed SDK, `2026.17-25-gc7954ebd1`:

```sh
"$VULKAN_SDK/bin/slangc" -version
```

ARCH-9. An agent MUST record the pin in exactly one place in the repository, which the binding generator, the build script, the tests, and the CI workflows all read, and MUST NOT restate it in any other file, because two copies of the pin diverge, and a build script that checks one copy while the bindings were generated from another accepts a library that does not match its bindings. Whether that place is a small data file or a key in the manifest metadata is open (Section 17, Q-6).

### 5.2. Updating the Pin

ARCH-10. An agent that moves the pin to a new Vulkan SDK MUST, in the same change, regenerate the bindings, fix what the regeneration breaks, and run every check, because a pin and bindings that disagree on the main branch give mismatched layouts and undefined behaviour at run time, as the reason of SYS-6 states.

ARCH-11. `slang-compiler-sys` MUST support exactly one Slang build in each release, the one in the pinned SDK, because Slang's ABI is reported not to be stable (Section 16, A-1), so a library from another build can disagree with the bindings in ways that no compiler detects.

ARCH-12. An agent that prepares a release of `slang-compiler-sys` after a change of the pin MUST release it as a breaking change unless the regenerated bindings are identical to those of the previous release, because a user who upgrades within a compatible version range expects the same native library to keep working. Slang is reported to release every one to three weeks (Section 16, A-2), which is why the pin follows the SDK and not each Slang release.

## 6. The `slang-compiler-sys` Crate

### 6.1. Contents

`slang-compiler-sys` contains the checked-in generated Rust code, the build script of Section 4, and its crate-level documentation, with no hand-written API, as FFI-1 of the coding standard requires. Its manifest declares `links = "slang-compiler"`, and its build script emits every link directive (FFI-2). The generated code consists of plain C types, constants, enumerations in the form that FFI-10 requires, `extern "C"` functions in the form that FFI-3 requires, and, for each COM-style interface, a `#[repr(C)]` vtable struct, a UUID constant, and thunks. The generated module carries the lint exception that Section 7.9 of the writing guide defines.

### 6.2. Binding Generator

The binding generator is a program in this repository that parses the pinned Slang headers with libclang and writes the bindings file. A `just` recipe runs it, and `docs/design/tooling.md` names that recipe. Only an agent that regenerates the bindings needs libclang, which the development container provides. The generator is an application, so rules ERR-1 and ERR-2 of the coding standard do not apply to it (Section 1.1 of the coding standard).

ARCH-13. The binding generator MUST produce every generated item of Section 6.1 from the pinned headers, including each vtable struct, UUID constant, and thunk, because a hand-written vtable or UUID diverges from the headers without any check failing, and COM-4 and COM-9 forbid writing them by hand.

Two tests cover the bindings, as TEST-8 of the coding standard requires: one regenerates the bindings into a temporary directory and fails if they differ from the checked-in file, and one references the native symbols so that a broken link fails a test.

## 7. The `slang-compiler` Crate

`slang-compiler` wraps the compile and reflection API of Slang: the global session, sessions, module loading, entry points, the composition and linking of component types, target code returned as blobs, diagnostics, and reflection of types, variables, layouts, functions, and entry-point parameters. It follows Section 12 of the coding standard (COM-1 to COM-13). In particular, one owning interface pointer type holds every interface (COM-1), every child holds a reference to its parent so that a session cannot be destroyed under its modules (COM-5), result codes map to the crate's one `Error` type in one place (COM-7, ERR-1), and diagnostics are always collected (COM-8).

ARCH-14. `slang-compiler` MUST NOT depend on wgpu or on any other graphics API crate, because users who only compile and reflect shaders, such as the first example of Section 10, would otherwise build a GPU stack they do not use, and every breaking upgrade of that stack would become a breaking release of the compiler crate (DEP-3).

The design document of `slang-compiler` answers the open questions on thread safety, the file-system and include-path model, compile options, and deprecated API (Section 17, Q-12 to Q-15).

## 8. The `slango` Crate

### 8.1. Purpose and Feature Set

`slango` makes calling Slang code from Rust simple, in the way that SlangPy does for Python. SlangPy's portable core is reported to consist of kernels generated from Slang reflection and cached per call signature, automatic broadcasting of arguments with an explicit `map()`, a unified tensor type, automatic differentiation through Slang's `bwds`, and a device layer (Section 16, A-9). The design document of `slango` confirms this list and decides which parts `slango` provides in each milestone.

### 8.2. Runtime Backend

The runtime backend of `slango` is wgpu first. Its design does not rule out a slang-rhi backend later. Section 14.4 gives the reason.

ARCH-15. `slango` MUST keep its backend-specific types behind a narrow internal boundary and MUST expose wgpu types only in one clearly separated public module, because a later slang-rhi backend then adds code without changing the rest of the public API, while wgpu types that appear throughout the API would make that backend a breaking change everywhere. This constraint does not ask for a backend trait, which SIMP-2 of the coding standard forbids before a second backend exists. The design document of `slango` defines the boundary (Section 17, Q-17).

### 8.3. Patterns From vulkano

`slango` takes these patterns from vulkano, as they are reported (Section 16, A-10): ownership through `Arc` with children holding their parents, structs of creation options, validated calls with clearly named unchecked variants, and typed shader interfaces generated from reflection.

ARCH-16. `slango` MUST NOT adopt the `GpuFuture` synchronization model of vulkano, because its own maintainers are reported to describe it as very hard to use correctly and to be replacing it (Section 16, A-10), and a model that is hard to use correctly in a library leads users to synchronization defects that no test finds.

### 8.4. Call Style

`slango` offers two call styles on one core, the runtime style first. The runtime API is driven by reflection: the user loads a module, looks up a function, passes arguments through a marshalling trait (Section 17, Q-18), and receives errors at run time. Milestone M6 adds the crate `slango-macros`, a build-time macro that reads a `.slang` file and generates typed Rust functions and structs, in the way vulkano-shaders does for vulkano. Section 14.7 gives the reason for the order.

ARCH-17. `slango-macros` MUST generate code that calls the same core of `slango` that the runtime API uses, and MUST NOT generate code that compiles or dispatches shaders by a separate path, because two paths diverge in behaviour, and a fix to one leaves the defect in the other.

This illustration shows the intent of the runtime style only. Every name in it is invented for the illustration, and the design document of `slango` defines the real API:

```rust
let device = slango::Device::new()?;
let module = device.load_module("shapes.slang")?;
let area = module.function("area")?;
let radii = device.buffer_from_slice(&[1.0_f32, 2.0, 3.0])?;
let areas: Vec<f32> = area.call((&radii,))?.read_back()?;
```

### 8.5. Shader Target

In milestone M3, `slango` compiles shaders to SPIR-V only. A later milestone makes the shader target configurable per backend, such as MSL on macOS (Section 17, Q-23). Section 14.9 gives the reason. wgpu is reported to offer a shader passthrough path for some targets and to reject some valid SPIR-V modules in its SPIR-V reader (Section 16, A-11 and A-12).

ARCH-18. An agent MUST carry out a spike as the first task of milestone M3, which compiles a set of test shaders to SPIR-V and loads them through wgpu on lavapipe, and MUST record what fails in the design document of `slango` before the runtime call API is designed, because an API built before the spike can depend on a shader path that wgpu rejects.

### 8.6. wgpu Version

ARCH-19. `slango` MUST use a wgpu major version that the current Slint release supports, because `slang-slint` passes wgpu objects between `slango` and Slint, and two major versions of wgpu in one build have incompatible types. Slint 1.18 is reported to support wgpu 29 and 30 through features named `unstable-wgpu-29` and `unstable-wgpu-30` (Section 16, A-13). The design document of `slango` chooses the version (Section 17, Q-20).

wgpu and the crates around it are below version 1.0, so a breaking upgrade of wgpu is a breaking release of every crate that exposes it, as DEP-3 of the coding standard states.

## 9. The `slang-slint` Crate

`slang-slint` provides a Slint view backed by a Slang render pipeline, built on `slango`. Slint is reported to have no public Vulkan hook, so external GPU content enters through its rendering notifier and its import of wgpu textures, and that API is reported to be unstable and able to change in minor releases (Section 16, A-14). The design document of `slang-slint` is written only as the first task of milestone M5 (ARCH-3).

ARCH-20. `slang-slint` MUST support wgpu only, permanently, and MUST pin a narrow range of Slint versions, because its only entry into Slint is the wgpu texture import, and that API is reported to change in minor releases (Section 16, A-14), which would otherwise break the builds of users. The design document of `slang-slint` chooses the range (Section 17, Q-25).

## 10. Examples

The examples are complete programs that also serve as the integration testbed, as TEST-5 of the coding standard requires. This table lists them with the milestone that delivers each one and its automated check.

| Number | Example | Needs | Milestone | Automated check |
| --- | --- | --- | --- | --- |
| 1 | A command-line program that compiles Slang source and prints its reflection | Slang, no GPU | M2 | Runs headless in CI |
| 2 | A compute kernel, including a broadcasting case and a differentiable function that uses Slang's automatic differentiation | Slang and any adapter | M3 | Runs headless in CI on a software adapter |
| 3 | A windowed renderer with winit and shader hot reload | Slang, a display, and an adapter | M4 | Built in CI, run locally |
| 4 | A Slint application with a `slang-slint` view, such as a shader playground with sliders bound to shader parameters | Slang, a display, and an adapter | M5 | Built in CI, run locally |
| 5 | A path-tracing renderer demo | Not designed | Later | Not designed |

ARCH-21. An agent MUST give each example at least one automated check, as the table of this section states, because an example that no check builds or runs breaks without notice, and the examples are the testbed of the whole workspace. Examples 3 and 4 run only locally until a headless rendering check is designed (Section 17, Q-24).

## 11. Test Tiers

Tests belong to one of three tiers, selected by the name of the module that holds them. The nextest profiles that select each tier are defined in `docs/design/tooling.md`.

| Tier | Module name | Needs | Runs in CI | Runs locally |
| --- | --- | --- | --- | --- |
| Default | Any name other than `gpu` and `hw_gpu` | No adapter. It may need the Slang shared library, which the build needs anyway | Yes, on Linux, Windows, and macOS | Yes |
| GPU | `gpu` | Any adapter, including a software adapter such as lavapipe or WARP | Planned, on lavapipe, WARP, and hosted Metal. CI already runs the recipe of this tier on all three platforms, but no test of this tier exists yet, so no adapter has been used on a runner (Section 16, A-17 and A-18). Whether each platform can run it while SPIR-V is the only shader target is open (Section 17, Q-21) | Yes |
| Hardware GPU | `hw_gpu` | A hardware GPU | No | Yes |

ARCH-22. An agent MUST put every test that needs an adapter in a module named `gpu`, or in a module named `hw_gpu` when the test needs a hardware GPU, because the default tier runs everywhere, and a test that needs an adapter fails there. This is the tagging mechanism that TEST-3 of the coding standard asks the design document to define.

ARCH-23. An agent MUST write every test in a `gpu` or `hw_gpu` module so that it fails when it finds no adapter, and MUST NOT write it to skip itself, because a skipped test reports success for a check that never ran, and a CI runner that loses its adapter would then pass every GPU test.

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
| Linux x86_64 | Vulkan | Primary. Developed and fully tested | The development container, on an NVIDIA RTX 4070 and on lavapipe, and CI, which runs every check and the default test tier, with the GPU tier on lavapipe planned |
| Windows x86_64 | D3D12 through WARP | Supported as far as CI proves | CI only, which runs Clippy, the doctests, and the default test tier, with the GPU tier on WARP planned |
| macOS arm64 | Metal on the hosted runner | Supported as far as CI proves | CI only, which runs Clippy, the doctests, and the default test tier, with the GPU tier on hosted Metal planned |

The two Vulkan devices in the development container were checked in the container. That hosted macOS runners expose a usable Metal device, and that lavapipe, WARP, and hosted Metal form the pattern that wgpu and slang-rhi use in their own CI, are reported (Section 16, A-17 and A-18). The Slang of the Vulkan SDK on Windows and macOS is also still to be confirmed (Section 16, A-8). The GPU tier is planned to run in CI on all three platforms. Whether each platform can run it while SPIR-V is the only shader target is open (Section 17, Q-21).

## 13. Publishing

The workspace plans for publication on crates.io from the start. The published crates are `slang-compiler-sys`, `slang-compiler`, `slango`, `slang-slint`, and `slango-macros`. The binding generator and the examples set `publish = false`. On 2026-11-10, the crates.io API answered HTTP 404 for each of those five names and for `slang-rhi-sys` and `slang-rhi`, so all seven names were available on that date.

Publishing readiness means complete publication metadata, documentation that builds on docs.rs without Slang installed (Section 4.5), and checks for breaking changes with cargo-semver-checks. `docs/design/tooling.md` specifies these. Milestone M0 sets up the metadata and the documentation build so that they are checked from the start, and M7 adds the check for breaking changes with the first release, because cargo-semver-checks needs a published version to compare with. Every crate stays below version 1.0, as DEP-3 of the coding standard requires.

ARCH-24. An agent MUST NOT publish a crate to crates.io without the owner's explicit go-ahead, because a published version cannot be deleted, only yanked, so a mistaken release stays visible to every user.

## 14. Decisions and Rationale

This section is non-normative. It records each decision in which the owner chose between alternatives, the alternatives considered, and the reason.

### 14.1. Generated Vtables

The bindings reach the COM-style interfaces through `#[repr(C)]` vtable structs, UUID constants, and thunks that the binding generator in this repository produces. The alternatives were vtables written by hand, a C shim compiled at build time that flattens the interfaces into plain C functions, and the experimental vtable generation of bindgen. Hand-written vtables have to be checked by hand against every Slang release, and the one existing third-party binding, which writes its vtables by hand, has a reported soundness issue in which objects crash when they outlive their session [SLANG-RS-26]. A C shim would need a C++ toolchain on every user's machine on the default route, which the dynamic route exists to avoid. bindgen's vtable generation is reported to be experimental (Section 16, A-5), and Section 4.2 of `docs/rust-references.md` records the same gap. A generator in this repository, whose output is checked in, gives exact vtables and keeps libclang a tool of the maintainers only.

### 14.2. Checked-In Bindings

The bindings are generated once per pin and checked in, and a test regenerates and compares them (TEST-8). The alternative was to generate them at build time on every user's machine, which needs libclang there and fails on docs.rs, where Slang is not installed. Checked-in bindings need neither, and the comparison test keeps them exact. Section 17 of the coding standard records the same resolution.

### 14.3. Separate Raw and Safe Crates

The raw bindings live in `slang-compiler-sys`, separate from the safe crate `slang-compiler`. The alternative was one crate with private raw bindings. The separate `-sys` crate changes only when Slang changes, it is the one package that carries `links = "slang-compiler"`, which stops two copies of the native library from entering one build (FFI-2), and every decision about ownership, lifetimes, and errors stays in the safe crate (FFI-1). Users who need raw access can also depend on the `-sys` crate without the safe API.

### 14.4. wgpu First, slang-rhi Deferred

`slango` uses wgpu as its first and only backend, and the bindings for slang-rhi are deferred. slang-rhi is reported to publish no releases or prebuilt binaries and to need a C++20 build with CMake (Section 16, A-15), so binding it would put a C++ build on every user's machine, which the default route avoids. Slint is reported to accept wgpu textures directly (Section 16, A-14), so a wgpu backend serves `slang-slint` without any bridge. A slang-rhi backend would add targets that wgpu does not offer, reported to include CUDA, the CPU, and OptiX (Section 16, A-16). That is why ARCH-15 keeps a later slang-rhi backend possible without building an abstraction now.

### 14.5. The Vulkan SDK's Slang as the Version Target

The workspace targets the Slang in the latest stable Vulkan SDK, not the latest Slang release. The alternative was to follow Slang's own releases. Slang's ABI is reported not to be stable and Slang is reported to release every one to three weeks (Section 16, A-1 and A-2), so following each release would make nearly every Slang release a breaking release of `slang-compiler-sys`. The SDK ships Slang with its headers, its shared library, and a pkg-config file, as the development container shows, so the default route needs one installation and nothing else. Because the SDK's Slang is a commit past a tag, the pin names the SDK version and the commit.

### 14.6. Two Routes and No Prebuilt Download

Slang comes from an installed SDK or from a static build from source, and nothing else (SYS-7). The alternatives were a prebuilt download in the build script, a search of system locations, and building from source by default. A download in a build script runs code that nobody reviewed and breaks offline and sandboxed builds, and SYS-5 forbids it. Official release archives are reported to contain no static libraries (Section 16, A-4), so a static build has to come from source in any case. Building from source by default would require a long native build and a C++ toolchain of every user. Two documented routes can each be tested, and a search of other locations would find libraries that do not match the pin.

### 14.7. Runtime Call API Before the Build-Time Macro

`slango` first offers the runtime API driven by reflection, and `slango-macros` follows in M6. The alternative was to start with the macro. The macro generates code that calls the runtime core (ARCH-17), so the core has to exist first, and the examples are the best way to decide its shape. The runtime API also serves shaders known only at run time, such as the shaders of the hot-reload example.

### 14.8. Compute Before Rendering

The first `slango` milestone covers compute kernels, and rendering follows. The alternative was to start with rendering, or to do both at once. Compute kernels run headless, so the milestone is tested on lavapipe in CI, which keeps every milestone up to M3 headless. Compute also needs fewer GPU concepts than a render pipeline, and it is the core of what SlangPy is reported to offer (Section 16, A-9).

### 14.9. SPIR-V First

The first `slango` milestone compiles to SPIR-V only. The alternative was a configurable shader target from the start. Vulkan on Linux is the primary platform, lavapipe runs SPIR-V, and one target keeps the spike of ARCH-18 small. Because wgpu is reported to reject some valid SPIR-V modules (Section 16, A-12), the spike measures that risk before the API depends on it, and the configurable target follows in M4.

## 15. Roadmap

### 15.1. Milestone Rules

ARCH-25. An agent MUST NOT start work on the code of a milestone before the design document that the milestone requires exists, because work without a design cannot be reviewed against one. Writing that document, when it does not exist yet, is the first task of the milestone (ARCH-3). The agent MUST NOT report a milestone complete until its exit criteria hold, because a milestone reported complete with failing checks or stale documents misleads the agent that starts the next one.

The exit criteria of every milestone include every check passing and every affected document updated, including this document. When a milestone's design document confirms an assumption of Section 16 or answers a question of Section 17, the same change removes that row and records the answer in that design document.

### 15.2. Summary

| Milestone | Delivers | Depends on | Design document before any code |
| --- | --- | --- | --- |
| M0 | Tooling and skeleton, complete | The container image, which provides every tool | `docs/design/tooling.md` |
| M1 | Binding generator and `slang-compiler-sys` | M0 | `docs/design/slang-compiler-sys.md` |
| M2 | `slang-compiler` and example 1 | M1 | `docs/design/slang-compiler.md` |
| M3 | `slango` compute kernels and example 2 | M2 | `docs/design/slango.md` |
| M4 | `slango` rendering and example 3 | M3 | The rendering section of `docs/design/slango.md` |
| M5 | `slang-slint` and example 4 | M4 | `docs/design/slang-slint.md` |
| M6 | `slango-macros` | M3 | `docs/design/slango-macros.md` |
| M7 | Publishing | M0 to M6 | `docs/design/tooling.md` |

### 15.3. M0: Tooling and Skeleton

- Deliverables: the contents of `docs/design/tooling.md`, which include deleting the abandoned sketch in `crates/slang-compiler-sys`, an empty and documented skeleton crate so that every tool has something to check (Section 3.2 of `docs/design/tooling.md`), the workspace lint configuration, `clippy.toml`, `deny.toml`, the nextest configuration, the `just` recipes for the Rust checks, the licence file, `README.md`, the CI workflow, and the publication metadata, so that it is checked from the start. The recipes for typos, taplo, cargo machete, Miri, cargo hack, and cargo llvm-cov are part of M0, and the recipe that runs actionlint arrives with the first workflow. The check for breaking changes is not part of M0 and arrives in M7 (Section 15.10).
- Depends on: the container image defined in the repository slang-rs-env, which provides every tool that M0 needs.
- Design document, which exists before any code of the milestone: `docs/design/tooling.md`.
- Exit criteria: `just check` passes with the Rust checks, Miri, and the checks of typos, taplo, actionlint, and cargo machete included, and the CI workflow passes on Linux, Windows, and macOS, including the jobs for the feature combinations and the MSRV. The coverage job arrives with the first crate that contains a function, because the coverage recipe fails while no crate has code. No example exists yet. `AGENTS.md`, Section 4.1 of the coding standard, and `README.md` describe the new state.
- Progress: M0 is complete. Every deliverable exists, including the custom checkers of Section 7 of `docs/design/tooling.md`, the CI workflow, and the recipe that runs actionlint. Every exit criterion holds. `just check` passes with the Rust checks, Miri, typos, taplo, actionlint, cargo machete, and the custom checkers included. On its first run on GitHub, every job of the CI workflow passed: `just check` on Linux, Clippy, the doctests, and the tests on Windows and macOS, and the jobs for the feature combinations and the MSRV. `AGENTS.md`, Section 4.1 of the coding standard, and `README.md` describe the new state. The parts of CI that need an adapter, which are lavapipe on Linux, WARP on Windows, and hosted Metal on macOS, arrive with the first `gpu` test in M3, so A-17 and A-18 of Section 16 remain unconfirmed.

### 15.4. M1: Binding Generator and `slang-compiler-sys`

- Deliverables: the binding generator, the checked-in bindings, the two tests of TEST-8, the build script with the dynamic route first and the static route second, and a smoke test that creates a global session and reads the Slang version string.
- Depends on: M0.
- Design document, which exists before any code of the milestone: `docs/design/slang-compiler-sys.md`, which also covers the binding generator.
- Exit criteria: every check passes, including the two binding tests and the smoke test in the default tier. The smoke test passes on both routes in the development container. Whether CI builds the static route is open (Section 17, Q-28). No example exists yet. `AGENTS.md`, `README.md` with how to obtain Slang, and this document are updated, and Section 4.4 of the coding standard is updated if the lints that fire on the generated module differ from its list.

### 15.5. M2: `slang-compiler`

- Deliverables: the safe API of Section 7 and example 1.
- Depends on: M1.
- Design document, which exists before any code of the milestone: `docs/design/slang-compiler.md`.
- Exit criteria: every check passes, and example 1 runs headless in CI. `AGENTS.md`, `README.md`, and this document are updated.

### 15.6. M3: `slango` Compute Kernels

- Deliverables: the SPIR-V spike of ARCH-18, the runtime call API, buffers and tensors, broadcasting, reading results back, and example 2, including its differentiable function.
- Depends on: M2.
- Design document, which exists before any code of the milestone: `docs/design/slango.md`.
- Exit criteria: every check passes, including the GPU tier in CI on lavapipe, and on WARP and hosted Metal as far as the answer to Section 17, Q-21 allows, and example 2 runs headless on lavapipe in CI. The design document of `slango` records the results of the spike. `AGENTS.md`, `README.md`, and this document are updated.

### 15.7. M4: `slango` Rendering

- Deliverables: render pipelines built from Slang entry points, a shader target configurable per backend, and example 3.
- Depends on: M3.
- Design document, which exists before any code of the milestone: a rendering section in `docs/design/slango.md`.
- Exit criteria: every check passes, example 3 builds in CI, and it runs locally. `AGENTS.md`, `README.md`, and this document are updated.

### 15.8. M5: `slang-slint`

- Deliverables: the Slint view of Section 9 and example 4.
- Depends on: M4.
- Design document, which exists before any code of the milestone: `docs/design/slang-slint.md`.
- Exit criteria: every check passes, example 4 builds in CI, and it runs locally. `AGENTS.md`, `README.md`, and this document are updated.

### 15.9. M6: `slango-macros`

- Deliverables: the build-time macro of Section 8.4, which generates typed Rust functions and structs.
- Depends on: M3, whose runtime core the generated code calls. The roadmap places M6 after M5.
- Design document, which exists before any code of the milestone: `docs/design/slango-macros.md`.
- Exit criteria: every check passes, including the tests that the design document of `slango-macros` names (Section 17, Q-26). `AGENTS.md`, `README.md`, and this document are updated.

### 15.10. M7: Publishing

- Deliverables: a verified documentation build of every published crate on docs.rs, a review of the public API of every published crate against the coding standard, the first release of the published crates after the owner's go-ahead (ARCH-24), and the check for breaking changes with its CI job, which needs the first release as its baseline. The publication metadata is in place from M0 (Section 15.3).
- Depends on: M0 to M6.
- Design document, which exists before any code of the milestone: `docs/design/tooling.md`, whose publishing requirements apply.
- Exit criteria: every check passes, the documentation of every published crate builds on docs.rs, and the owner has approved the release. `README.md` and this document are updated.

### 15.11. Later Items

These items are not scheduled and have no design yet: the path-tracing demo (example 5, Section 17, Q-29), a slang-rhi backend with the crates `slang-rhi-sys` and `slang-rhi`, and further platforms. Each one gets a design document when the owner schedules it.

### 15.12. Order Rationale

This subsection is non-normative. Each milestone is the smallest step that the next one needs. The bindings come before the safe crate that wraps them, the safe crate before the library that compiles through it, and compute before rendering, as Section 14.8 explains. Every milestone up to M3 runs headless, so CI tests each one fully before any work that needs a display begins.

## 16. Assumptions to Confirm

Each row is a fact that comes from research that was not independently confirmed. A section that relies on one of them cites its identifier. The design document or milestone in the last column confirms or corrects it, and the change that does so removes the row.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| A-1 | Slang's ABI is not stable between releases, as Slang's documentation on building states | ARCH-11, Section 14.5 | `docs/design/slang-compiler-sys.md`, M1 |
| A-2 | Slang releases every one to three weeks | ARCH-12, Section 14.5 | `docs/design/slang-compiler-sys.md`, M1 |
| A-3 | A static build of Slang needs its internal static libraries compiler-core and core, as well as miniz and lz4 | Section 4.3 | `docs/design/slang-compiler-sys.md`, M1 |
| A-4 | Official Slang release archives contain no static libraries | Section 14.6 | `docs/design/slang-compiler-sys.md`, M1 |
| A-5 | The vtable generation of bindgen is experimental | Section 14.1 | `docs/design/slang-compiler-sys.md`, M1 |
| A-6 | The build script of a dependency cannot set the rpath of the final binary | Section 4.4 | `docs/design/slang-compiler-sys.md`, M1 |
| A-7 | docs.rs sets the environment variable `DOCS_RS` during its builds | ARCH-8 | `docs/design/slang-compiler-sys.md` at M1, and the first docs.rs build at M7 |
| A-8 | The Vulkan SDK for Windows and macOS ships the Slang headers and shared library | Sections 4.2 and 12 | `docs/design/slang-compiler-sys.md` at M1, and the first CI run on those platforms |
| A-9 | SlangPy's portable core is kernels generated from reflection and cached per call signature, broadcasting with an explicit `map()`, a unified tensor type, automatic differentiation through `bwds`, and a device layer | Sections 8.1 and 14.8 | `docs/design/slango.md`, M3 |
| A-10 | vulkano uses the patterns listed in Section 8.3, and its maintainers describe `GpuFuture` as very hard to use correctly and are replacing it | Section 8.3, ARCH-16 | `docs/design/slango.md`, M3 |
| A-11 | wgpu offers a shader passthrough path for some targets | Sections 3.3 and 8.5 | The spike of ARCH-18 at M3, and the rendering section of `docs/design/slango.md` at M4 |
| A-12 | The SPIR-V reader of wgpu rejects some valid SPIR-V modules | ARCH-18, Section 14.9 | The spike of ARCH-18, recorded in `docs/design/slango.md`, M3 |
| A-13 | Slint 1.18 supports wgpu 29 and 30 through features named `unstable-wgpu-29` and `unstable-wgpu-30` | ARCH-19 | `docs/design/slango.md`, M3 |
| A-14 | Slint has no public Vulkan hook, external GPU content enters through its rendering notifier and wgpu texture import, and that API is unstable and can change in minor releases | ARCH-20, Section 14.4 | `docs/design/slang-slint.md`, M5 |
| A-15 | slang-rhi publishes no releases or prebuilt binaries and needs a C++20 build with CMake | Section 14.4 | The design document of a slang-rhi backend, when the owner schedules one |
| A-16 | A slang-rhi backend would add CUDA, CPU, and OptiX targets | Section 14.4 | The design document of a slang-rhi backend, when the owner schedules one |
| A-17 | Hosted macOS runners on GitHub Actions expose a usable Metal device, which slang-rhi's own CI relies on | Section 12 | `docs/design/tooling.md`, and the first CI run of a `gpu` test at M3 |
| A-18 | lavapipe on Linux, WARP on Windows, and hosted Metal on macOS form the pattern that wgpu and slang-rhi use in their own CI | Section 12 | `docs/design/tooling.md`, and the first CI run of a `gpu` test at M3 |

## 17. Open Questions

Each row is a question that this document leaves open. The design document in the second column answers it before or during the milestone in the third column, and the change that answers it removes the row.

| Identifier | Question | Owning design document | Milestone |
| --- | --- | --- | --- |
| Q-1 | How does the build script read the version of the Slang it finds, and which header or file carries that version? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-2 | How does the static route verify the commit of the source tree? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-3 | Which static libraries exactly does the static route link? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-4 | How do the layouts of the Vulkan SDK on Windows and macOS differ from Linux? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-5 | Does the build script use pkg-config at all? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-6 | Where is the pin recorded, in a small data file or a manifest metadata key, and in what format? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-7 | Is the static route selected by a Cargo feature or by the environment, such as by the presence of the proposed `SLANG_SOURCE_DIR`? A feature is enabled by every check that passes `--all-features`, which are Clippy, the documentation build, and the doctests, so those routine checks would need a Slang source tree and a long native build. If a feature selects the route, how do the `--all-features` checks and the feature matrix treat it, and is the proposed name `static` confirmed? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-8 | Is the proposed name `SLANG_SOURCE_DIR` confirmed for the variable that names the source tree? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-9 | Does the static route read `VULKAN_SDK` as well? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-12 | Which wrappers, such as the session, are `Send` or `Sync`, given Slang's documented threading rules? | `docs/design/slang-compiler.md` | M2 |
| Q-13 | What is the model for the file system, include paths, and module search paths, and which files can Slang read through them? | `docs/design/slang-compiler.md` | M2 |
| Q-14 | How are compile options exposed? | `docs/design/slang-compiler.md` | M2 |
| Q-15 | Which deprecated Slang API does the safe crate leave out? | `docs/design/slang-compiler.md` | M2 |
| Q-16 | Does each crate emit logs, and through which crate? | `docs/design/slang-compiler.md` first, then each later crate's design document | M2 |
| Q-17 | Where is the boundary between backend-specific and portable code in `slango`, and which public module exposes wgpu types? | `docs/design/slango.md` | M3 |
| Q-18 | What is the design of the marshalling trait? | `docs/design/slango.md` | M3 |
| Q-19 | Which unsafe code does `slango` or `slang-slint` need, such as for shader passthrough or for Slint or wgpu-hal interoperation, and why? | `docs/design/slango.md`, `docs/design/slang-slint.md` | M3, M5 |
| Q-20 | Which wgpu major version does `slango` use first? | `docs/design/slango.md` | M3 |
| Q-21 | Which platforms run the GPU tier while SPIR-V is the only shader target? | `docs/design/slango.md` | M3 |
| Q-22 | Which error kind reports that no adapter was found? | `docs/design/slango.md` | M3 |
| Q-23 | How is the shader target configured per backend? | The rendering section of `docs/design/slango.md` | M4 |
| Q-24 | Can the windowed and Slint examples be checked headless, and how? | The rendering section of `docs/design/slango.md` | M4 |
| Q-25 | Which range of Slint versions does `slang-slint` pin? | `docs/design/slang-slint.md` | M5 |
| Q-26 | How does `slango-macros` read a `.slang` file at build time, is the proposed re-export of `slango-macros` by `slango` behind an optional feature confirmed, and which tests or examples exercise the generated code? | `docs/design/slango-macros.md` | M6 |
| Q-27 | How is the pinned Vulkan SDK installed on each CI runner, including its download address, checksum verification, and caching? | `docs/design/tooling.md` | M1, when the first crate needs Slang |
| Q-28 | Does CI build the static route? | `docs/design/slang-compiler-sys.md` | M1 |
| Q-29 | What is the design of the path-tracing demo? | A design document written when the owner schedules the demo | Later |

## 18. Security Considerations

The threat model has five parts. First, an application passes shader source, module names, include paths, and module search paths that an attacker controls to `slang-compiler` or `slango`, and the attacker chooses their content but not the code of the application or of these crates. Second, an agent introduces a defect into unsafe code or into a generated declaration, which turns a malformed input or a misuse into undefined behaviour. Third, the build script of `slang-compiler-sys` runs on the user's machine with the user's permissions, reads environment variables, and, on the static route, runs CMake and a native build on a source tree that the user names. Fourth, a dependency, whether a crate or the native library, contains a defect or a published vulnerability. Fifth, a CI workflow downloads the Vulkan SDK (Section 17, Q-27) and other tools, and an attacker on the network or at the download host tampers with them.

The design addresses each part, mostly through the rules that Section 20 of the coding standard summarizes. For untrusted input, `slang-compiler` returns errors instead of panicking (ERR-5), and its design document states which files Slang can read through include paths and module search paths (Section 17, Q-13). For unsafe code and the invariants of the foreign function interface (FFI), the crates rely on the native library matching the bindings, which ARCH-9 to ARCH-11 and SYS-6 secure, and on the reference counting and lifetime contracts of the Slang API, which Section 12 of the coding standard constrains. Unsafe code stays in two crates (Section 3.3), and generated declarations are never edited by hand (ARCH-13, COM-9). For build scripts, SYS-5 forbids downloads, installs, and writes outside `OUT_DIR`, ARCH-6 keeps the static build inside `OUT_DIR`, and ARCH-5 and ARCH-7 stop the build instead of searching for a library elsewhere. For dependencies, the pin fixes the native library to one SDK version and one commit, and cargo-deny checks the advisories, licences, and sources of every crate. For CI, every download whose upstream publishes a SHA-256 checksum is verified against it, as Hard Rule 7 of `AGENTS.md` requires, and `docs/design/tooling.md` specifies how. No file in the repository contains a secret.

These attacks are out of scope. A malicious or compromised Slang library or Vulkan SDK runs with the full privileges of the application, so no crate here can constrain it. A source tree that the user names for the static route is code that the user chose to build, and CMake runs it with the user's permissions, so the crate cannot defend the user against it. Defects in GPU drivers are outside every crate here. Attacks on the repository itself are a matter of access control.

The residual risk has three parts. First, Slang is a large C++ code base that parses shader source inside the application's process, so a memory-safety defect in Slang can be reached by an attacker's shader source, and shader source can make the compiler use unbounded time or memory unless the application limits it. Second, the soundness of the unsafe code rests on judgement rules and on contracts in the Slang documentation that no tool verifies, and Miri cannot run the native library. Third, the custom checkers of `docs/design/tooling.md` do not read every form of documentation or workflow, and the rules or parts of rules that no tool checks depend on review, as Section 16 of `docs/design/tooling.md` states.

## 19. Operational Considerations

### 19.1. Cargo Features

This table lists every Cargo feature that this document defines. Each crate's design document adds its own.

| Crate | Feature | Default | Enables |
| --- | --- | --- | --- |
| `slang-compiler-sys` | `static` (proposed, and whether a feature selects the route is open, Section 17, Q-7) | Off | The static route of Section 4.3 |

### 19.2. Environment Variables

This table lists every environment variable that the build script of `slang-compiler-sys` reads, with its behaviour when unset.

| Variable | Read when | Behaviour when unset |
| --- | --- | --- |
| `VULKAN_SDK` | On the dynamic route when `DOCS_RS` is unset. Whether the static route reads it is open (Section 17, Q-9). | The build stops with an error that names the variable (ARCH-5) |
| `SLANG_SOURCE_DIR` (proposed) | When the `static` feature is on and `DOCS_RS` is unset | The build stops with an error that names the variable (ARCH-7) |
| `DOCS_RS` | Always | The build proceeds on the selected route. When the variable is set, the build script skips finding, building, and linking Slang on either route, whatever features are enabled (ARCH-8). |

### 19.3. Platforms and Minimum Supported Rust Version

Section 12 lists the supported platforms and graphics APIs. The workspace declares its minimum supported Rust version (MSRV) in `rust-version`, as DEP-4 of the coding standard requires, and `docs/design/tooling.md` sets its value and the check that verifies it.

### 19.4. Failure Modes

This table lists the failure modes that this document defines, how each is reported, and the recovery.

| Failure | Reported by | Recovery |
| --- | --- | --- |
| No Vulkan SDK on the dynamic route | A build script error that names `VULKAN_SDK` (ARCH-5) | Install the pinned SDK and set `VULKAN_SDK`, or enable the static route |
| No source tree on the static route | A build script error that names `SLANG_SOURCE_DIR` (ARCH-7) | Set the variable to a source tree at the pinned commit |
| The native build of Slang fails | A build script error that names the command, its exit status, and its error output (SYS-4) | Fix the toolchain or the source tree that the error names |
| The Slang found is not the pinned version | A build script error that names both versions (SYS-6) | Install the pinned SDK, or check out the pinned commit |
| The shared library is not found at run time | The operating system's dynamic loader, which stops the program before `main` runs | Run the SDK's setup script on Linux or macOS, or add the SDK to `PATH` on Windows (Section 4.4) |
| Slang rejects a shader | The `Error` of `slang-compiler`, with the diagnostic text (ERR-1, COM-8) | Fix the shader source that the diagnostics name |
| No adapter is found | An error of `slango` whose kind is open (Section 17, Q-22). A test in a GPU tier fails (ARCH-23). | Run on a machine with an adapter, such as lavapipe |

### 19.5. Logs

This document defines no logs. Whether the crates emit logs is open (Section 17, Q-16).

## 20. References

### 20.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 20.2. Informative References

- [SLANG-RS-26] "Objects must outlive Session?", issue 26 of the FloatyMonkey/slang-rs repository on GitHub, <https://github.com/FloatyMonkey/slang-rs/issues/26>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| ARCH-1 | 3.2 | Library crates depend only on the crates that a path of arrows in the diagram reaches from them. |
| ARCH-2 | 3.2 | No library crate depends on the binding generator or an example. |
| ARCH-3 | 3.4 | A crate's design document is the first task of its milestone and exists before any code. |
| ARCH-4 | 4.2 | The dynamic route reads `VULKAN_SDK`, finds the headers and library there, and links dynamically. |
| ARCH-5 | 4.2 | With `VULKAN_SDK` unset, the `static` feature off, and `DOCS_RS` unset, the build stops and searches nowhere else. |
| ARCH-6 | 4.3 | The static route builds Slang with CMake from `SLANG_SOURCE_DIR` inside `OUT_DIR` and links statically. |
| ARCH-7 | 4.3 | With `DOCS_RS` unset, the static route stops without a valid source tree and never falls back to the dynamic route. |
| ARCH-8 | 4.5 | With `DOCS_RS` set, the build script skips finding, building, and linking Slang on either route, and this takes precedence over ARCH-4 to ARCH-7. |
| ARCH-9 | 5.1 | The pin is recorded in exactly one place that every consumer reads, and restated nowhere else. |
| ARCH-10 | 5.2 | Moving the pin, regenerating, fixing, and checking happen in one change. |
| ARCH-11 | 5.2 | Each release of `slang-compiler-sys` supports exactly the pinned Slang build. |
| ARCH-12 | 5.2 | A pin change is a breaking release unless the bindings are identical. |
| ARCH-13 | 6.2 | The binding generator produces every generated item, including vtables, UUIDs, and thunks. |
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

## Change Log

- 2026-11-10: Recorded M0 as complete in Sections 1.6, 15.2, and 15.3, recorded in Sections 11 and 12 that CI runs the default test tier on Linux, Windows, and macOS while the GPU tier stays planned, corrected the confirmation point of A-17 and A-18 to the first `gpu` test in CI, and updated the residual risk of Section 18.
- 2026-11-10: Recorded in Sections 1.6 and 15.3 which deliverables of M0 exist, with the placeholder crate in place of the deleted sketch, and narrowed the residual risk of Section 18 to the parts of M0 that remain.
- 2026-11-10: Moved the recipes that waited for the container image into M0 and the check for breaking changes to M7, moved the coverage job to the first crate with code, stated the `LD_LIBRARY_PATH` state of the container, and noted the crate-level documentation of the integration-test root.
- 2026-11-10: Initial version.
