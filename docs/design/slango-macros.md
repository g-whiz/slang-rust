# `slango-macros` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-12 |
| Applies to | `crates/slango-macros`, `crates/slango-codegen`, and the hidden module `__private` of `crates/slango` |

## Abstract

This document specifies milestone M6: the crate `slango-macros`, the library crate `slango-codegen`, and the changes to `slango` that both need. The procedural macro `shader!` of `slango-macros` compiles a Slang file during the compilation of a Rust crate and generates typed Rust bindings to its compute kernels, and `slango-codegen` holds the generator. The document starts from the facts about compiling Slang inside a procedural macro and, on those facts, defines the generated items and the hidden module of `slango` through which they reach the runtime core.

## 1. Introduction

### 1.1. Problem

The runtime API of `slango` sets every parameter by name and checks it at the dispatch (Section 9 of `docs/design/slango.md`). A misspelled name, a value of the wrong type, or a buffer of the wrong element type fails only when the program runs. A user also writes by hand every Rust struct that mirrors a Slang structure in a buffer, with its padding, and nothing checks it against Slang's layout.

A check of the stride alone of a bound buffer lets a `Buffer<u32>` bind to a `StructuredBuffer<float>` and read wrong values without an error.

### 1.2. Purpose

`slango-macros` turns these run-time mistakes into compile errors. The macro reads a `.slang` file, compiles it with the pinned Slang, and generates one typed kernel for each compute entry point and one Rust struct for each Slang structure that the kernels take, with layout assertions. The generated code calls the same core as the runtime API (ARCH-17).

This document implements Section 8.4 and milestone M6 (Section 15.9) of `docs/design/architecture.md`. It answers Q-26 and confirms A-10 of that design, as Sections 4.1 and 3.9 record.

### 1.3. Audience

The primary readers are the coding agents that maintain `slango-macros` and `slango-codegen` and the reviewers who check their changes. A reader is expected to know Rust procedural macros and the public API and layout snapshot of `slango` (Sections 5.1, 7.1, and 9 of `docs/design/slango.md`).

### 1.4. Scope

This document covers the macro, the generator crate, the hidden module of `slango`, the element type check of `bind`, the generated code for compute kernels, errors, threading, tests, continuous integration (CI), and one example.

### 1.5. Non-Goals

These are not part of M6:

- Typed render pipelines, typed draws, and vertex layouts, which are deferred with no milestone assigned.
- An entry point for a build script that writes the generated code into `OUT_DIR`. It needs Slang in the build script and its own tracking, so it is not free, and it is deferred with no milestone assigned.
- A mode that embeds the Slang source and compiles it at run time, because it cannot resolve an `import` (Section 3.5).
- A program that runs without the Slang library (Section 13, SLMQ-1).
- A re-export of the macro by `slango` (Section 13, SLMQ-2).
- A cache of the generated code that lets a crate build without Slang, as `sqlx` keeps one (Section 13, SLMQ-3).

### 1.7. Defects of Third-Party Components and Trusted Input

`slango-macros` follows Hard Rule 14 of `AGENTS.md` and Section 1.7 of `docs/design/slango.md`. The `.slang` file, every file it imports, and the path in the macro call are trusted input from the crate that calls the macro. The macro does not guard against a defect of Slang, and Section 15.6 lists the properties of compiling Slang inside the compiler, which are limits and not defects.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

Section 2.1 of `docs/design/architecture.md` and Section 2.1 of `docs/design/slango.md` define the terms of GPU programming and of `slango` that this document uses, such as procedural macro, compute kernel, layout snapshot, and structured buffer. This document adds these terms:

- encoded snapshot: The layout snapshot of a module, with the SPIR-V of each kernel, written as a private, versioned byte string that the generated code embeds.
- expansion: One run of a procedural macro, during which it returns the tokens that replace its call.
- generated item: A Rust item that the macro emits, such as `Shader`, a kernel struct, an arguments struct, or a struct for a Slang structure.
- host dependency: A dependency that Cargo builds for the machine that runs the compiler, such as a procedural macro and everything it depends on.
- interface description: The plain, public description of a compiled module that the generator reads: its kernels, their parameters with Rust-relevant types, and the files that Slang read.
- invoking file: The Rust source file that contains the call of the macro.
- rpath: A list of directories, stored in a shared library on Linux and macOS, in which the dynamic loader looks for the libraries it needs.
- trybuild: A crate that compiles small Rust programs in a test and compares the compiler's messages with an expected text, used for compile-fail tests.

## 3. Facts the Design Relies On

This section is non-normative. It states each fact about Slang, Cargo, rustc, and the platform that a later section relies on, once, with its consequence for the design.

### 3.1. Sources and Evidence

Each fact carries one of these marks:

- (run): Observed by running code with Rust 1.99.1 and the pinned Vulkan SDK, on x86_64 Linux unless the fact names another platform. A GPU result holds on a hardware NVIDIA GPU and on lavapipe.
- (read): Read in the named source.
- (inferred): Concluded from other facts and not verified. Section 12 lists the assumptions on which a requirement depends.

### 3.2. Mechanism

Three mechanisms can generate typed bindings: a function-like procedural macro that compiles during expansion, a library for build scripts that writes Rust into `OUT_DIR`, and a macro that only declares the interface and checks it at run time.

- The procedural macro generates a typed kernel for `examples/shaders/saxpy.slang` and a `#[repr(C)]` struct for a structured buffer of a structure, and both give the exact expected results (run). The generated code needs only `Kernel::call`, `set`, `bind`, `dispatch`, and one load path for a precompiled snapshot (run).
- The build-script library gives clearer errors without a library path, but its diagnostics carry no span (run).
- The declared macro needs no Slang at build time, but a field of the wrong type fails only at the first dispatch (run).

vulkano-shaders 0.35.0 generates `#[repr(C)]` structs with padding and load functions from reflection. inline-spirv and include-wgsl-oil are procedural macros of the same kind, and wgsl_to_wgpu and spirv-builder are build-script generators (read, the source or documentation of each crate).

### 3.3. Loading the Macro Library

The macro library links `libslang-compiler.so.0.2026.17` dynamically and has no rpath of its own (run). Without `$VULKAN_SDK/lib` on the library path of rustc, `cargo build`, `cargo check`, and `cargo doc` all fail with `error[E0463]: can't find crate for 'slango_macros'`, and nothing names the missing library (run). A doctest goes through rustdoc, which loads the macro the same way (inferred).

With `cargo::rustc-link-arg=-Wl,-rpath,$VULKAN_SDK/lib` printed by the build script of the macro crate, the library gets that directory as its `RUNPATH`, and all three commands pass with `LD_LIBRARY_PATH` unset (run). A link argument of `slang-compiler-sys` would not reach the macro, because `rustc-link-arg` applies only to the targets of the package that prints it (read, the Cargo reference). Cargo removes library paths outside `target/` from every process it starts (read, the Cargo reference).

rust-analyzer loads macro libraries in its own process (read), so it needs the rpath or an environment of its own (inferred).

On Windows, an rpath does not exist, and the macro library finds `slang-compiler.dll` through `PATH`, which the CI job sets (run, on the hosted Windows runner). On the hosted macOS runner, the macro library loads in a job that sets both the rpath of SLM-12 and `DYLD_LIBRARY_PATH` (run). So whether the rpath alone suffices on macOS is not established (SLMA-5).

### 3.4. Paths and Rebuilds

A path resolved from `CARGO_MANIFEST_DIR` fails under trybuild, which compiles each case in a package of its own (run). A path resolved relative to the invoking file, through `proc_macro::Span::call_site().local_file()`, works in the workspace and under trybuild (run). `local_file` is stable since Rust 1.88.0 (read, the documentation of `proc_macro`), and its result is relative to the working directory of rustc, so it must be made absolute against that directory (run).

The documentation of `local_file` says that its result should not be embedded in the output of a macro. vulkano-shaders and include-wgsl-oil embed such paths anyway (read).

An expansion that emits `const _: &[u8] = include_bytes!("<absolute path>");` for the file and for every file of `Module::dependency_file_paths` makes Cargo rebuild the crate when any of them changes. After a change of a file that the shader imports, Cargo reports the file as changed and recompiles, and the next build is fresh (run).

`proc_macro::tracked::path` does not compile on stable Rust, and its tracking issue is open (run, read). Neither mechanism notices a new file that would change how an `import` resolves (inferred).

### 3.5. Embedding and Loading

A load from an embedded, encoded snapshot takes about a millisecond, while a load that compiles embedded source takes about 150 ms (run). The program still has `libslang-compiler.so.0.2026.17` as a needed library in both cases, and fails to start without the library path, because `slango` depends on `slang-compiler` unconditionally (run).

An embedded source that imports another file fails with `cannot open file 'common.slang'`, because `load_module_from_source` sets no search path (run). The precompiled snapshot resolves the import at build time (run).

### 3.6. The Static Route

With `SLANG_SOURCE_DIR` set and no SDK, the macro library and the program have no Slang library as a needed library (run). The saxpy kernels run. A kernel with a structure fails at load with `UnsupportedShader` and naga's `UnsupportedInstruction(Function, CopyLogical)`, because Slang skips `spirv-opt` without `slang-glslang`, which the static build lacks (run, and read, `docs/design/slang-compiler-sys.md`).

So the SPIR-V that the macro embeds depends on the Slang of the machine that expanded the macro (inferred).

### 3.7. Diagnostics, Aborts, and Cross-Compilation

A Slang error, returned as `syn::Error` at the span of the string literal, appears as a compile error with `slango`'s text and its causes, which contain Slang's diagnostics with their own layout (run). A missing file gives `cannot read a Slang file` with the operating system's error as the cause (run). `proc_macro::Diagnostic` is unstable, so the macro cannot point into the `.slang` file (read).

A panic in the macro becomes `error: proc macro panicked` at the call (run). An abort, as `abort_on_exception` performs, kills rustc with `SIGABRT` and no diagnostic of its own (run).

`cargo check --target x86_64-pc-windows-msvc` from Linux passes and runs the macro with the host's Slang (run). The generated SPIR-V and snapshot do not depend on the target (inferred).

### 3.8. docs.rs

With `DOCS_RS` set and no SDK, `cargo doc` fails with `E0463`, because the macro library is linked with Slang's symbols unresolved (run). With a `cfg` that the macro's build script sets when `DOCS_RS` is present, under which the macro emits a stub and calls no generator, the library has no undefined Slang symbol (run). `cargo doc` then passes, without the generated items in the documentation (run).

Under the stub, a crate that names a generated item other than `Shader` builds its documentation only in some positions (run, with `DOCS_RS` set and `VULKAN_SDK` removed):

- A `use` declaration of a generated arguments struct or a generated structure fails with `error[E0432]: unresolved imports`, at the top of a file and inside the body of a function.
- A function signature that names such an item fails, because the compiler cannot find the type, for private functions at the top level and for functions nested inside the body of another function.
- A reference inside the body of a function passes, in an expression, such as a call of `particles::Particle::new` and a struct literal of `particles::AdvanceArgs`, and in the type annotation of a `let`.

So rustdoc resolves every path to a generated item outside a function body, public or private, and the stub leaves such a path unresolved. A struct field, a type alias, or a constant that names a generated item fails the same way (inferred). A private signature in a library documented without its private items is not established.

### 3.9. Generated Code

For `struct Particle { float3 position; float mass; float2 velocity; }` in a `RWStructuredBuffer`, Slang reports offsets 0, 12, and 16 and a stride of 32 (run). A generated `#[repr(C)]` struct with a private tail padding of 8 bytes, a `const fn new`, and `size_of` and `offset_of!` assertions matches it and reads back correctly (run). Private padding fields rule out a struct literal in user code (run).

A structure in a uniform buffer is flattened into one plain value for each leaf with a dotted path such as `params.scale` (read, Section 7.1 of `docs/design/slango.md`). The reflection walk of `slango` records the Slang name of each structure among the plain values with its parameter path (run). This holds for a global `uniform` structure, a structure nested in it, a structure inside a `ConstantBuffer<Params>`, and a `uniform` structure of the entry point.

Slang reports the size of a nested structure rounded up to its alignment, and the offsets of the outer structure follow that size (run). For example, `Motion { float2 velocity; float drag; }` has a size of 16 bytes, with its fields at 0 and 8 and 4 bytes of tail padding. `Body { float3 position; Motion motion; uint id; }` then has its fields at 0, 16, and 32 and a size of 48 bytes.

Unit tests of the tokens generated from a hand-made interface description, formatted with `prettyplease`, run natively and under Miri in a library crate (run). Every insta snapshot test of the repository sits in a module that Miri skips (read).

This confirms A-10 of the architecture design: vulkano generates typed shader interfaces from reflection.

### 3.10. Costs and Dependencies

One expansion costs about as much as one runtime load, about 150 ms (run). A procedural macro runs again on every compilation of the crate that calls it, so each call adds that cost to every rebuild (run).

A release build compiles `slango`, `wgpu`, `naga`, and `slang-compiler` a second time as host dependencies of the macro (run, inferred for the cause).

The crate trybuild builds the macro and its dependencies in a package of its own, so its first run in a clean target directory takes minutes (run).

`syn`, `quote`, `proc-macro2`, `prettyplease`, `trybuild`, and `target-tuple`, which trybuild brings, are `MIT OR Apache-2.0`, which the allow-list admits. Each declares a `rust-version` of 1.88 or less (run).

`syn` 2, `syn` 3, and `prettyplease` 0.2 are in the lock file already, and `deny.toml` skips the duplicate `syn` 2 (read, `Cargo.lock` and `deny.toml`). `proc-macro-error` and `proc-macro-error2` are unmaintained (read, RUSTSEC-2024-0370 and RUSTSEC-2026-0173).

## 4. Crate Structure and Dependencies

### 4.1. Decisions

The owner decided:

1. A function-like procedural macro in `slango-macros` that runs Slang during compilation, so every crate that uses it needs the pinned Slang at build time (Section 6).
2. The generated code embeds the encoded snapshot and loads it without a call into Slang, and the program still links Slang in M6 (Section 5).
3. Compute kernels only, with generated structs for Slang structures (Section 7).
4. The element type check of `bind` in `slango` (Section 5.1).
5. `slango` does not re-export the macro in M6. A user depends on `slango-macros` directly. This answers Q-26 of the architecture design together with Sections 6.1 and 10.
6. The tests of Section 10.
7. The dependencies of Section 4.2, with no new licence and no new advisory exception.
8. The known limits of Section 15.6, which are recorded and not guarded against.
9. The separate published library crate `slango-codegen` for the generator (SLM-1).
10. The element type check of `bind` is exact for a plain element and keeps the stride check for a structure (SLM-5).

### 4.2. Packages and Dependencies

SLM-1. The generator MUST live in the library crate `slango-codegen`, and `slango-macros` MUST hold only the macro entry point, its path resolution, its build script, and the conversion of errors to compile errors. A procedural macro crate exports nothing but macros, and Miri runs the pure tests of the generator in a library crate (Section 3.9), so a generator inside the macro crate could leave its pure tests without Miri.

The owner approved `slango-codegen` as a separate published crate, and Sections 3.1, 3.2, and 13 of `docs/design/architecture.md` list it. Its crates.io name is checked before the first release (SLMA-8).

`slango-codegen` has two public functions, and both return a `proc_macro2::TokenStream`:

- `generate` takes the path of a Slang file, calls `precompile_file`, and so Slang, and generates from its result.
- `generate_precompiled` takes the result of `precompile_file` and is pure.

Only the tests format the tokens with `prettyplease`.

`slango-codegen` depends on `slango`, `proc-macro2`, and `quote`. For its tests only, it depends on `syn` with the features `full`, `parsing`, and `printing`, on `prettyplease`, and on `insta`.

`slango-macros` depends on `slango-codegen` and on `syn` with the features `parsing`, `printing`, and `proc-macro`. For its tests only, it depends on `slango` and `trybuild`. It needs no `proc-macro2` of its own.

Both crates are published, because a published crate cannot depend on an unpublished one.

SLM-2. `slango-codegen` MUST require `slango` with an exact version, `=x.y.z`, and `slango-macros` MUST require `slango-codegen` with an exact version, because the encoded snapshot and the hidden module of `slango` have no stability across versions (SLM-6). A mismatch would fail at load or not compile.

Each requirement also carries the path, as TOOL-25 of `docs/design/tooling.md` requires. The exact requirement on `slango-codegen` is its entry in `[workspace.dependencies]`, because only `slango-macros` inherits it.

The requirement on `slango` cannot come from `[workspace.dependencies]`, because `slang-slint` and the examples package inherit that entry as `0.1.0`. So `slango-codegen` declares `slango = { path = "../slango", version = "=0.1.0" }` in its own manifest, with a comment that cites SLM-2. This is the exception to DEP-2 that the coding standard states.

With an exact requirement, Cargo resolves the `slango` of the user's crate to the same version, because two semver-compatible versions of a crate cannot coexist in one build graph (inferred, SLMA-3). In the workspace, `cargo tree -i slango -e normal,dev` shows one node `slango v0.1.0` that serves every member (run). That covers path dependencies with one version only, so the case of a registry with several versions stays open.

SLM-3. The crates MUST use `syn` 2 and `prettyplease` 0.2, the versions already in the lock file, and MUST NOT use `proc-macro-error` or `proc-macro-error2`.

TOOL-26 of `docs/design/tooling.md` admits an exception only for a duplicate that no change to the workspace can remove. These versions add none, while `prettyplease` 0.3 would add a skip. The two crates of error reporting are unmaintained (Section 3.10).

With these versions, the two crates add no licence, no skip, and no advisory exception to `deny.toml`. A change that needs a new licence or a new advisory exception stops, and the agent asks the owner. Every other dependency of both crates is declared once in `[workspace.dependencies]` (DEP-2).

SLM-4. `slango-macros` and `slango-codegen` MUST contain no unsafe code and MUST emit no logs, because neither needs either: they call only safe `slango` code and report every failure as a compile error or an error value. This answers Q-16 of the architecture design for both crates.

SIMP-4 of the coding standard prefers functions to macros. The macro is the deliverable of M6 that Section 8.4 of the architecture design decides, and the generator behind it is plain functions and types.

## 5. Changes to `slango`

### 5.1. The Element Type Check

This check changes the behaviour of `slango` for every user, macro or not.

SLM-5. The layout snapshot MUST record the element type of each structured buffer in its storage buffer record. When the recorded element is a scalar or a vector, `Call::bind` and `Draw::bind` MUST report `ErrorKind::InvalidArgument` naming the parameter unless the Rust element type has the same scalar and component count. When the recorded element is a structure, they MUST keep the check of the stride alone.

A check of the stride alone reads wrong values without an error (Section 1.1), while a buffer of structures must stay usable without the macro.

The full element type has the four forms that Section 7.1 of `docs/design/slango.md` defines:

- A scalar or vector of `bool`, `int`, `uint`, or `float` with its component count.
- A scalar or vector of another scalar type with its Slang name.
- A structure with its Slang name, size, and fields.
- A composite, which is a matrix, an array, or another type.

A field of a structure can be a plain value, a nested structure, or another type with its Slang name, such as `half` or `Inner[2]`. The Slang name, size, and fields of a structure are for the generator, and the check does not read them. The generator also reads the recorded element type to choose the Rust type of each buffer field (Section 7.2).

The check runs at the dispatch or the draw with the other checks of SLO-14 and SLO-26 of `docs/design/slango.md`. A buffer of a built-in `Element`, such as `[f32; 4]`, still binds to a structured buffer of a structure of the same stride, so a user of the runtime API alone keeps the stride check for structures. Generated bindings are exact in both cases, because the element type that the generator emits for a structure is that structure's own struct (Section 7.3).

Section 9.3 of `docs/design/slango.md` states the rules of the check, the choices of the implementation for `bool`, other scalar types, matrices, and arrays, and the behaviour change.

### 5.2. The Hidden Module

SLM-6. `slango` MUST provide the module `#[doc(hidden)] pub mod __private`, through which generated code and `slango-codegen` reach the core of the runtime API, and MUST NOT promise any stability for it. Its contents change with any version of `slango`, and SLM-2 matches the versions exactly, so no user can depend on it.

The module holds:

- `precompile_file`, which compiles a file exactly as `Device::load_module` does, with the same session, search path, and reflection walk (SLO-5 and SLO-7 of `docs/design/slango.md`). It returns a `Precompiled`, which holds the interface description and the encoded snapshot.
- The types of the interface description: kernels with their name, thread group size, and parameters, each parameter with its path, kind, and type, including the Slang name of a structure and the element type of a structured buffer, and the files that Slang read.
- `load_precompiled`, which decodes an encoded snapshot and loads it with no call into Slang.
- `ENCODING_VERSION`, the version of the encoding (Section 5.3).
- The trait for generated structs (Section 5.4).
- `slang_type`, which names the Slang type of a scalar or a vector, such as `float3`. `slango` uses it for the same type in its own errors, so the generator names a type exactly as the check of a bound buffer does.

Behind the module, loading is split into `layout::compile` and a crate-private load from the snapshot, which Section 7.1 of `docs/design/slango.md` describes. `Device::load_module` and `precompile_file` share the crate-private `module::compile_file`, which reads the file. The encoder and the decoder of the snapshot are crate-private.

Generated code calls `slango` only through its public API and this module, so it compiles and dispatches through the same core as the runtime API (ARCH-17).

The interface description has these properties:

- Its types are public types of `__private` that mirror the crate-private layout types, because the layout types stay crate-private (Section 5.3). They are not `#[non_exhaustive]`, so that tests can build them with literals and match them exhaustively.
- It describes each compute entry point and leaves out the vertex and fragment entry points, for which the generated code offers nothing (Section 7.5).
- The parameters of a kernel follow the order of its bindings, by set and binding. The plain values of each uniform buffer stand at the place of that buffer, and the `uniform` parameters of the entry point come last. The parameters that the compiled code does not use are listed too (Section 7.1).
- A plain value inside a structure is listed under its dotted path. Each structure among the plain values is listed once more with its path and Slang name, so the generator rebuilds the nesting from the paths (Section 3.9).
- The files that Slang read are part of the description and not of the encoded snapshot, so the encoded bytes embed no path of the build machine.

### 5.3. The Encoding

SLM-7. The encoding MUST be private to `slango`, little-endian, and versioned by a magic header with a version number, and `load_precompiled` MUST reject a byte string of another version or with trailing bytes with `ErrorKind::InvalidArgument`.

An encoding that decodes wrongly would load a kernel with wrong bindings.

The layout types stay crate-private. The generated code also asserts at compile time that `slango::__private::ENCODING_VERSION` equals the version it was generated for, so a mismatch that SLM-2 does not prevent fails to compile instead of at load.

The encoding has these properties:

- The bytes start with an 8-byte magic header, followed by `ENCODING_VERSION`, which is 1, as a little-endian `u32`. The version changes whenever the bytes of the encoding, or the meaning of any of them, change.
- `load_precompiled` also rejects truncated or malformed bytes with `ErrorKind::InvalidArgument`. The decoder does not panic and bounds every count by the bytes left before it allocates.
- The decoder accepts at most 256 levels of nested arrays or structures, and only bindings in descriptor sets up to 255, so that a load creates a bounded number of bind group layouts.
- The encoder returns `ErrorKind::UnsupportedParameter` for a snapshot that the decoder would reject, such as one with a count above `u32::MAX` or a binding in a descriptor set above 255. So it never writes bytes that the decoder refuses.

Section 10 lists the tests of the encoding.

### 5.4. Generated Structs and the Seal

SLM-8. `slango` MUST provide in `__private` the trait `StructElement`, with the Slang name, the stride, and the conversion to and from little-endian bytes at the reflected offsets, and MUST implement `Element` and `Value` for every type that implements it. `VertexElement` MUST reject such a type at the draw.

Generated structs need the sealed traits, and the hidden trait is the only way in that does not unseal them for users.

The seal stays sound. `slango` has no unsafe code (SLO-2 of `docs/design/slango.md`), so a wrong implementation of `StructElement`, written by hand against the documentation, can produce only wrong bytes or an error, never undefined behaviour. A user who implements a hidden trait is outside the API, as with any `__private` item.

Because `VertexElement` is a supertrait of `Element`, the blanket implementations cover it too, and coherence accepts them beside the built-in implementations. Sections 8, 8.2, and 9.2 of `docs/design/slango.md` state the public behaviour that follows:

- A draw rejects a vertex buffer of such a type (SLM-8).
- A dispatch or a draw rejects a value of one given to `set`.
- The creation of a buffer rejects an element of 0 bytes.

## 6. The Macro

### 6.1. Invocation and Paths

SLM-9. `slango-macros` MUST provide one function-like procedural macro, `shader!`, which takes exactly one string literal, the path of a `.slang` file, and MUST report any other input as a compile error. The generated items appear where the macro is called, so a user calls it inside a module of its own, once per module.

SLM-10. The macro MUST resolve a relative path against the directory of the invoking file, as `include_str!` does, through `Span::local_file`, made absolute against the working directory of rustc. When `local_file` returns nothing, the macro MUST report a compile error and MUST NOT fall back to `CARGO_MANIFEST_DIR`, because a silent fallback resolves the same literal to another file (Section 3.4).

### 6.2. Rebuild Tracking

SLM-11. The expansion MUST contain `const _: &[u8] = include_bytes!("<absolute path>");` for the file and for every file that Slang read, so that Cargo rebuilds the crate when any of them changes (Section 3.4).

The generated code therefore contains absolute paths of the build machine. When `proc_macro::tracked::path` is stable, a change replaces the trick with it. A new file that changes how an `import` resolves does not trigger a rebuild (Section 15.6).

### 6.3. The Build Script

SLM-12. The build script of `slango-macros` MUST print `cargo::rustc-link-arg=-Wl,-rpath,<VULKAN_SDK>/lib` when `VULKAN_SDK` is set and not empty and the macro is built for Linux or macOS, `cargo::rerun-if-env-changed` for `VULKAN_SDK` and `DOCS_RS`, and the `cfg` of SLM-13 with its `cargo::rustc-check-cfg` declaration, and nothing else. Without the rpath, every build of a crate that uses the macro fails with a misleading `E0463` unless the user puts the library on the path of rustc (Section 3.3).

The build script reads two variables and prints Cargo instructions. It writes no file, downloads nothing, and installs nothing, so it meets Hard Rule 7 of `AGENTS.md` and SYS-5 of the coding standard. The rpath is the build machine's own directory, stored in the macro library, which only the compiler on that machine loads.

On the static route, `VULKAN_SDK` is unset or irrelevant and the macro library needs no Slang library (Section 3.6). On Windows the build script prints no link argument, and the macro library finds `slang-compiler.dll` through `PATH` (Section 3.3). Reading the directory from the `links` metadata of `slang-compiler-sys` would need a direct dependency on it and a change to its build script, so this design keeps the simpler variable.

The build script includes `instructions.rs`, which holds the logic as a plain function behind a trait for the environment, so its tests run under Miri (SYS-8 of the coding standard). The rpath and the `cfg` of SLM-13 are independent of each other.

When the script would print the rpath, a `VULKAN_SDK` that is not valid UTF-8 stops the build of `slango-macros`. So does one that contains a comma, which the linker would read as the end of the `-Wl` argument. The message says to install the SDK in another directory.

### 6.4. docs.rs

SLM-13. When `DOCS_RS` is present, the build script MUST set the `cfg` `slango_docs_rs`, and under it the macro MUST emit a documented stub `pub struct Shader;` and MUST NOT call the generator. Otherwise the macro library links Slang's symbols unresolved and rustdoc cannot load it (Section 3.8).

The stub's documentation says that the bindings are generated only in a build with Slang. The macro emits the stub without resolving or reading the path, so a wrong path does not fail the documentation build. The other generated items are absent from the documentation of the user's crate.

A crate builds its documentation without Slang only when it names every generated item other than `Shader` inside function bodies alone, in expressions and in the type annotations of `let` (Section 3.8). Section 15.6 states the consequence.

### 6.5. Diagnostics

SLM-14. The macro MUST report every failure as a compile error at the span of the string literal, with the text of the error and each of its causes, and MUST NOT panic for a failure it can report.

A panic shows as `proc macro panicked` without the structure of the error (Section 3.7).

## 7. Generated Code

### 7.1. The Shader and Its Kernels

SLM-15. The macro MUST generate a struct `Shader` with `Shader::load(&slango::Device) -> slango::Result<Shader>`, which loads the embedded snapshot with `load_precompiled` and looks up every kernel, a public field and a kernel struct for each compute entry point, and `Shader::module`, which returns the `slango::Module`. A lookup at load means no later call fails for a missing kernel.

SLM-16. Each kernel struct MUST have the constant `THREAD_GROUP_SIZE: [u32; 3]`, a method `dispatch(&self, args: &XxxArgs<'_>, elements: [u32; 3]) -> slango::Result<()>`, a method `dispatch_groups` with the same arguments and workgroup counts, and `kernel`, which returns the `slango::Kernel`. `dispatch` and `dispatch_groups` MUST create the call with `Kernel::call`, set every argument with the public setter and its literal path, and dispatch, so every runtime check of `slango` still runs (ARCH-17).

The arguments struct `XxxArgs<'a>` has one public field for every parameter of the kernel, including a parameter that the compiled code does not use, so the generated API does not change when an optimization drops a parameter. It has the lifetime only when it holds a reference.

### 7.2. Parameters

SLM-17. The generator MUST map each parameter to a field as this table shows, and MUST report every other parameter as a compile error (Section 7.5):

| Slang parameter | Field type | Set with |
| --- | --- | --- |
| `bool`, `int`, `uint`, or `float` | `bool`, `i32`, `u32`, or `f32` | `set` |
| A vector of `N` of these | `[T; N]` | `set` |
| A matrix of `R` rows and `C` columns | `[[T; C]; R]` | `set` |
| An array of any of these | `[U; N]`, nested from the outermost length | `set` |
| A structure | The generated struct (Section 7.3) | `set` of each leaf by its dotted path |
| A structured buffer of an element with a built-in `Element` | `&'a slango::Buffer<T>` | `bind` |
| A structured buffer of a structure | `&'a slango::Buffer<S>`, with `S` the generated struct | `bind` |
| A texture | `&'a slango::Texture` | `texture` |
| A sampler | `&'a slango::Sampler` | `sampler` |

A field of a named constant buffer has the path `Name.field` (Section 7.1 of `docs/design/slango.md`) and becomes its own field. The documentation of each buffer field says whether the kernel reads it or reads and writes it.

### 7.3. Structures

SLM-18. For a Slang structure that is the element of a structured buffer, the generator MUST emit a `#[repr(C)]` struct with a public field at each reflected offset and private padding fields for every gap and the tail. It MUST also emit a `const fn new` that takes every public field, an assertion of its size and of every field's offset, and an implementation of `StructElement`.

The assertions use `size_of` and `offset_of!` in a `const` block. A layout that Rust and Slang see differently then fails to compile instead of reading wrong values.

A field is `f32`, `i32`, `u32`, a vector of these, an array of these whose size is its length times the element size, or another generated struct. For a structure that appears only as a uniform value, the generator emits a struct with public fields, `new`, and no padding or assertions, because `slango` writes each leaf at its reflected offset (SLO-13 of `docs/design/slango.md`).

Every generated struct derives `Clone`, `Copy`, `Debug`, and `PartialEq`. A structure is emitted once, under its Slang name, however many parameters use it.

The element form is built as follows:

- The struct is `#[repr(C)]`, with its public fields in the order of their offsets. A private array of `u8` fills each gap and the tail, named `_padding0`, `_padding1`, and so on in the order of the offsets.
- `new` is a `const fn` marked `#[must_use]`. It takes the public fields in their order and sets every byte of padding to zero.
- A `const` block asserts that `size_of` equals the stride that Slang gives the structure and that `offset_of!` of each field equals its reflected offset. Each assertion has a message that names the struct, and the field for an offset.
- The implementation of `StructElement` gives the Slang name and the stride. `write_bytes` appends each scalar as little-endian bytes in the order of the offsets, and zero for each padding. `read_bytes` returns `None` for bytes of another length, and otherwise reads each scalar at its offset with checked slicing, so it cannot panic.
- A nested structure of an element is an element struct of its own, emitted before the struct that holds it. The outer struct holds it as a field at its reflected offset, and writes and reads it through its own implementation of `StructElement`.

The uniform form is a plain struct with public fields, `new`, and the same derives, and with no `#[repr(C)]`, padding, or assertions. Its fields are the leaves and the nested structures whose path is the path of the structure followed by one name, in the order of their first leaf among the parameters. A leaf may be any plain value of SLM-17, such as a `bool`, a matrix, or an array.

The arguments struct has one field for each outermost uniform structure, at the place of its first leaf, and the dispatch sets every leaf by its dotted path, such as `params.light.colour`. A structure inside a named constant buffer gets the field of its whole path, such as `settings_light` for `settings.light`. A uniform structure with no leaves still gets its field, after the other parameters.

A structure that is both an element and a uniform value is emitted once in the element form, which serves both, whichever use comes first. Every use of one structure name must give the same field names and Rust types, and two uses as an element must also give the same layout. Otherwise the generator reports a name conflict (Section 7.4).

A generated `new` with more than seven arguments carries `#[expect(clippy::too_many_arguments, reason = "...")]`, whose reason says that `new` takes one argument for each field of the Slang structure. Without it, Clippy's default limit of seven arguments fires in the crate that calls the macro.

The cost is that a crate that raises `too-many-arguments-threshold` in its Clippy configuration sees `unfulfilled_lint_expectations` from Clippy for a generated `new` whose argument count is above seven and within that threshold. `#[allow]` is ruled out by Hard Rule 6 of `AGENTS.md`. Section 4.4 of the coding standard lists this exception.

### 7.4. Names

SLM-19. The generator MUST derive each Rust name from its Slang name by fixed rules and MUST report a compile error that names both Slang names when two generated items or fields get the same Rust name.

A silent rename would make the generated API depend on the order of declarations.

The rules are these:

- A field name of an arguments struct is the path in snake case, with `.` replaced by `_` and `_` inserted before an uppercase letter that follows a lowercase letter or a digit.
- A kernel struct name is the entry point name in upper camel case, and its arguments struct adds `Args`.
- A struct keeps its Slang name. The Rust name of a field of a generated struct is the Slang field name in snake case, not its path.
- A strict or reserved keyword of edition 2024 becomes a raw identifier, in a struct and in the dotted access of the dispatch, such as `args.filter.r#type`.
- The generated code names every item of `core` and `slango` by its full path, such as `::core::result::Result::Ok`, so an item of the user's module cannot shadow it.

These are compile errors of the kind `NameConflict`:

- A name that is not a Rust identifier: `self`, `Self`, `super`, `crate`, `_`, an empty name, a name that starts with a digit, a structure name such as `Pair<float>`, and a name with a character that is not ASCII, which is rejected and never mapped.
- A reserved name: `Shader` among the type names, and `load` and `module` among the fields of `Shader`.
- Two items with one Rust name. The name of each arguments struct takes part in the search, so a kernel `scale` beside a kernel `scaleArgs` is a conflict.
- A struct name that equals another type name in the same scope as `Shader`, the kernel structs, and the arguments structs, such as a structure `Step` beside a kernel `step`.
- A Slang field named like a padding field, such as `_padding0`, because the padding fields take their names after the public fields.
- Two different structures with one Slang name, such as `Item` in two namespaces.

### 7.5. Not Supported in M6

The generator reports each of these as an error of the kind `Unsupported` (SLM-20), which the macro reports as a compile error:

- A structured buffer whose element is not `i32`, `u32`, `f32`, or an array of four of them, which are the built-in `Element` types. Examples are a vector of 2 or 3 components such as `float3`, `bool`, another plain type such as `half` or `vector<double,2>`, and a matrix, an array, or another composite type.
- A parameter that `precompile_file` of `slango` rejects with `UnsupportedParameter`, such as a 64-bit or 16-bit scalar value. The error of `slango` is the source.
- A field of an element structure, at any depth of nesting, of a type that Section 7.3 does not list. Examples are a `bool`, a vector or an array of `bool`, a matrix, and a field of another type, such as `half` or `double`. An array whose stride differs from the size of its element, such as `float3 a[2]`, is also unsupported.
- A structured buffer of a structure of 0 bytes, because `StructElement` needs a stride above zero.
- An element structure with a field that starts inside the field before it, with a size that ends before its last field, or with an offset or array size that overflows the computation of its layout. These checks do not guard against Slang. The interface description is public and can be built by hand, and the generator computes the padding from it with checked arithmetic.
- A resource inside a uniform structure, such as a texture.
- The path of a file that Slang read that is not valid UTF-8, which the tracking of SLM-11 cannot write.

A module with render entry points compiles, and the generated code offers nothing for them. A user reaches them through `Shader::module`.

The message names the parameter by its path and the kernel, and gives the reason. For an unsupported field, it also names the field and the structure that holds the field, which is the nested structure for a field of one. The `Display` text of the error puts `cannot generate the bindings: ` before it:

```text
the parameter `cells` of the kernel `grow` is not supported, because the field `alive` of the structure `Cell` is a `bool`, which a generated struct does not support
```

### 7.6. Usage Illustration

This illustration shows the form of the generated API for `saxpy.slang` and has not been compiled. A line marked `// ...` is elided:

```rust
mod saxpy {
    slango_macros::shader!("../shaders/saxpy.slang");
}

fn run(device: &slango::Device) -> slango::Result<Vec<f32>> {
    // ... create the buffers x, y, and result as slango::Buffer<f32>
    let shader = saxpy::Shader::load(device)?;
    let args = saxpy::SaxpyArgs { a: 2.0, count: 4, x: &x, y: &y, result: &result };
    shader.saxpy.dispatch(&args, [4, 1, 1])?;
    result.read()
}
```

## 8. Errors

`slango-macros` has no error type, because a procedural macro crate exports only macros. It turns every failure into a compile error (SLM-14).

SLM-20. `slango-codegen` MUST have one public error type `Error` with the alias `Result`, as ERR-1 defines, with the kinds `Compile`, whose source is the error of `slango`, `Unsupported`, for every case of Section 7.5, and `NameConflict`, for SLM-19, because the macro's tests match on the kind.

`ErrorKind` is `#[non_exhaustive]`, and its kinds cover these cases:

- `Compile` covers every error of `slango` other than `UnsupportedParameter`, such as a file that cannot be read or does not compile.
- `Unsupported` also covers an error of `slango` of the kind `UnsupportedParameter`, with that error as its source (Section 7.5).
- `NameConflict` also covers a Slang name that gives no Rust identifier (Section 7.4).

`Error::report` returns the text of the error followed by one line `caused by:` for each cause, and the macro reports that text (SLM-14).

At run time, generated code returns `slango::Error`. A load of an encoded snapshot of another version gives `InvalidArgument` (SLM-7), and every mistake at a dispatch gives the error that `slango` gives for the same call by name.

## 9. Threading

SLM-21. `Shader` and every kernel struct MUST be `Send` and `Sync` without a manual implementation, asserted in a test of the generated code (CONC-1), because they hold only `slango` types, which are both (SLO-17 of `docs/design/slango.md`).

An arguments struct holds references to buffers, textures, and samplers, which are `Sync`, so it is `Send` and `Sync` too. The macro starts no thread.

## 10. Tests

`slango-codegen` has pure unit tests in `src/emit/tests.rs`, in `src/emit/structs/tests.rs`, and in the modules `names` and `error`. They build interface descriptions by hand and check the tokens of the pure step with plain assertions, and Miri runs them. They cover:

- Each row of SLM-17, the documentation of read and read-write buffers, a kernel with no parameters, the tracking of every file read, and a path that is not UTF-8.
- The element form of SLM-18 with its padding, assertions, and `new`, the exact code of `write_bytes` and `read_bytes`, a gap between fields, an array read at its stride, and a nested structure.
- A structure emitted once for two kernels and a nested use, the uniform form with its leaves set by path, inside a constant buffer, and without leaves, and an element that is also a uniform value in either order.
- Raw keywords, each conflict of Section 7.4, each unsupported case of Section 7.5, and the lint exception of a `new` with more than seven arguments.

The module `ffi` of its integration-test binary `tests/codegen/` (TEST-10) compiles the test shaders under `tests/codegen/shaders/` with Slang. It compares the formatted expansion with `insta` snapshots of these shaders:

- saxpy.
- A shader with a named constant buffer, a texture, a sampler, a uniform array, a `bool`, `int4` and `float4` buffers, a matrix, and two kernels.
- A structure element with padding.
- A nested structure that two kernels use.
- A uniform structure beside an element that also serves as a uniform value.

Before formatting, the tests replace each path of a file read with `<shaders>/` and the file name, and the encoded snapshot with `b"<encoded snapshot>"`. So the snapshots do not depend on the machine, the checkout, or the SPIR-V of the pin.

Further `ffi` tests of `slango-codegen` check:

- The absolute paths and the embedded encoding that `generate` emits, and the files read for a shader with an `import`.
- A Slang error with Slang's message among the causes, and a missing file.
- A `float3` buffer and a `double` uniform.
- A field of each unsupported kind that a shader can declare, `bool`, a matrix, `float3 a[2]`, `half`, and `double`, with each message.
- Two structures of one name in two namespaces, with the conflict.

`slango-macros` has one integration-test binary, `tests/macros/`:

- The module `bindings`, which Miri runs, asserts SLM-21. It resolves `shaders/saxpy.slang` from `tests/macros/bindings.rs` and `../shaders/scale.slang` from `tests/macros/bindings/nested.rs` (SLM-10).
- The module `build_script` tests the logic of the build script under Miri (Section 6.3).
- The module `structs` sits in no `ffi` module, so Miri runs it, although it calls the macro. It checks the round trip of `write_bytes` and `read_bytes` for generated structs over many seeds, the rejection of bytes of another length, and that `write_bytes` puts each scalar at the `offset_of!` of its field, nested fields included, and zero in the padding.
- The module `ffi` runs trybuild with one passing case and with compile-fail cases for a Slang compile error, an unsupported parameter, an unsupported `bool` field, a name conflict, input that is not one string literal, and a string literal with a suffix. The expected text of a Slang error contains Slang's messages, so a new pin can change it (SLM-22).
- The module `ffi` also compares the size, stride, and field offsets of generated structs with what `precompile_file` reflects. They cover tail padding, a gap between fields, arrays, nested structures, and a `new` of eight arguments.
- The module `gpu` runs saxpy through `dispatch` and through `dispatch_groups` with a count that stops inside the last group, against values computed on the CPU. It also runs a kernel over a buffer of particles with a nested structure, a uniform structure `Step` that nests the structure `Wind`, and a uniform `Motion` that the element form of `Motion` serves. It compares the whole buffer exactly with values computed on the CPU.

A missing file is no trybuild case, because its expected text would hold an error message of the operating system that differs on Windows, and the `ffi` test of `slango-codegen` covers it. The comparison with `precompile_file` uses the description of `slango`, so a wrong description that is consistent with itself passes it. The `gpu` test is the independent check of the stride, through the stride check of `bind`.

The inputs of the `gpu` tests are dyadic rationals, so an adapter with or without fused multiply-add gives the same bits.

`slango` has these tests of the changes of Section 5:

- Unit tests and `gpu` tests of SLM-5 for each kind of mismatch of a scalar or vector element, and for a built-in element of the same stride bound to a structure.
- Pure tests of the encoding, which Miri runs. They decode every record and variant of the snapshot back to itself, including uniform buffers with arrays, immediates, textures, samplers, and render records, and reject every truncation of an encoding.
- A pure test that sets each byte of an encoding to 0 or 255 and checks that it either fails to decode or decodes to a snapshot that encodes to the same bytes.
- A test that pins the bytes of the encoding to `ENCODING_VERSION`, which guards the version rule of Section 5.3.
- An `ffi` test that compiles each test shader of `slango` with `precompile_file` and checks that the decoded snapshot equals the snapshot that `Device::load_module` builds, render records included.
- `gpu` tests that run kernels from a precompiled load and give the same results as a load from source. No test draws from a precompiled module.

The pure tests of the encoding use generated variants and exhaustive sweeps, not property tests with regression files as TEST-9 of the coding standard asks, because they need no new dependency.

The example `typed-kernel` of the examples package, at `examples/src/bin/typed-kernel.rs`, calls the macro on its own shader `examples/shaders/particles.slang`. The shader has one kernel, `advance`, which takes a structured buffer of the structure `Particle`, a structured buffer of `float`, and three plain values. The stride of `Particle` is 32 bytes, which leaves 4 bytes of tail padding.

The macro embeds the shader when the example builds, so the example takes no file argument and rejects every argument other than `-h` or `--help`. It dispatches through the generated arguments struct, reads both buffers back, and prints them.

It exits with a failure status when a result differs from the values that it computes on the CPU. Every input is dyadic, so every result is exact on every adapter.

The example names the generated items other than `Shader` only inside function bodies, so that `check-doc-without-slang` documents it without Slang (Section 6.4), and its crate-level documentation says why. Its tests in `examples/tests/examples/` are:

- In `gpu.rs`, a `gpu` test that runs the example, checks that it succeeds and that standard error starts with `running on `, and compares standard output with the `insta` snapshot `examples__gpu__particles.snap`.
- In `ffi/typed_kernel.rs`, `ffi` tests that check the usage text for `-h` and `--help` and the failure for an argument.

No automated test covers the rebuild after a change of an imported file, because it needs a nested Cargo build. Section 3.4 states the behaviour.

The crate-level example of `slango-macros` is a `text` block. In a doctest, the macro receives the documented source file, such as `crates/slango-macros/src/lib.rs`, and resolves the path against its directory by SLM-10.

So the path of the example does not resolve there, as Section 7.5 of `docs/writing-documentation.md` permits.

The `gpu` tests of saxpy in `tests/macros/gpu.rs` of `slango-macros` run the bindings that the module `bindings` generates with the same call on the same shader.

So they are the compiled example of the same use that the writing guide asks for. The example `typed-kernel` is a further compiled use, with another shader.

SLM-22. An agent that changes the pin MUST run the compile-fail tests and update their expected text in the same change, because that text contains Slang's messages.

A new Slang can therefore fail the tests with no change to the macro.

## 12. Assumptions to Confirm

Each row is a statement that this design relies on and that is not yet established. A section that relies on one cites its identifier.

The change that confirms or corrects it states the result in Section 3 and in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SLMA-3 | An exact version requirement on `slango` makes Cargo resolve the user's `slango` to the same version | SLM-2 | The first release, in M7 |
| SLMA-5 | The rpath alone, without `DYLD_LIBRARY_PATH`, lets the macro library load on macOS | SLM-12 on macOS | A macOS build with no library path set, which CI does not run |
| SLMA-8 | The crates.io name `slango-codegen` is available | SLM-1 | The first release, in M7 |

## 13. Open Questions

| Identifier | Question | Answered by |
| --- | --- | --- |
| SLMQ-1 | Should a Cargo feature of `slango` remove `slang-compiler` at run time, so that a program with only precompiled modules needs no Slang library, and what does `load_module` do without it? | A later milestone, not assigned |
| SLMQ-2 | Should `slango` re-export `shader!` behind a feature? | M7, at publishing |
| SLMQ-3 | How does a crate that names generated items outside function bodies get its documentation on docs.rs, where the stub breaks its documentation build (Section 15.6)? The candidates are a checked-in cache of the generated interface that the macro reads when Slang is absent, in the style of the offline mode of `sqlx`, which would also show the generated items, and documentation built elsewhere than on docs.rs. | M7, at publishing |

An identifier is never reused.

## 14. Security Considerations

The threat model treats the `.slang` file, the files it imports, and the path in the macro call as trusted input from the crate that calls the macro, as Hard Rule 14 of `AGENTS.md` states. The macro runs Slang inside rustc with the privileges of the build.

Neither crate has unsafe code (SLM-4), and `slango` keeps none (SLO-2 of `docs/design/slango.md`). So a wrong `StructElement` implementation or a corrupt encoded snapshot can produce wrong bytes or an error, never undefined behaviour (Section 5.4).

The build script of `slango-macros` writes nothing and downloads nothing, and adds the SDK's library directory to the macro library's rpath (SLM-12). The new dependencies are widely used crates under licences that the allow-list admits, fixed by `Cargo.lock` and checked by `cargo-deny` (SLM-3).

These attacks are out of scope, because the user chose the input: a malicious shader, a shader that reaches a defect of Slang, and a `.slang` file outside the crate that a relative path names.

The residual risk is a defect in Slang that corrupts memory inside rustc during an expansion. Another is a library planted in the SDK's library directory, which the rpath lets rustc load, as the library path of the recipes already does.

## 15. Operational Considerations

### 15.1. Cargo Features and Environment Variables

Neither crate has Cargo features. The build script of `slango-macros` reads `VULKAN_SDK`, which sets the rpath, and `DOCS_RS`, which selects the stub (SLM-12, SLM-13). Building a crate that uses the macro needs what building `slango` needs: the pinned Slang through `VULKAN_SDK` or `SLANG_SOURCE_DIR` (Section 9 of `docs/design/slang-compiler-sys.md`).

### 15.2. Platforms and Minimum Supported Rust Version

CI builds and tests both crates on Linux, Windows, and macOS. On macOS, the CI job also sets `DYLD_LIBRARY_PATH`, so it does not show that the rpath alone suffices (Section 3.3, SLMA-5).

`Span::local_file` needs Rust 1.88.0, and every new dependency declares 1.88 or less, below the workspace's minimum supported Rust version (MSRV) of 1.99.

### 15.3. Failure Modes

| Failure | Reported as | Recovery |
| --- | --- | --- |
| The Slang library is not on rustc's library path, such as on Windows without the SDK on `PATH` | `E0463: can't find crate for 'slango_macros'` | Set `VULKAN_SDK` before the first build on Linux and macOS, or put the SDK's library directory on the path |
| `VULKAN_SDK` is unset and `SLANG_SOURCE_DIR` is unset | The build script of `slang-compiler-sys` fails | Install the pinned SDK and set `VULKAN_SDK` |
| The `.slang` file does not compile or cannot be read | A compile error at the path literal with Slang's text | Fix the shader or the path |
| A parameter or field is not supported | A compile error at the path literal that names it (Section 7.5) | Change the shader, or use the runtime API |
| Slang aborts during an expansion | rustc exits with `SIGABRT` after one line on standard error | Report it as a defect of Slang (Hard Rule 14) |
| naga rejects the embedded SPIR-V | `UnsupportedShader` at `Shader::load` | Build with the Slang of the Vulkan SDK (Section 3.6) |
| The encoded snapshot is of another version | A compile error from the version assertion, or `InvalidArgument` at load | Use the matching versions (SLM-2) |

### 15.4. Logs

Neither crate emits logs (SLM-4).

### 15.5. Costs

Section 3.10 states the build-time costs of the macro: each call runs on every compilation of the crate that contains it, a release build compiles the runtime dependencies twice, and the first trybuild run is slow.

### 15.6. Known Limits

These are properties of the design, recorded and not guarded against:

- An abort inside Slang during an expansion kills rustc (Section 3.7).
- The embedded SPIR-V depends on the Slang of the build machine (Section 3.6).
- A release build compiles `slango`, `wgpu`, and `naga` twice (Section 3.10).
- A new file that changes how an `import` resolves does not trigger a rebuild (Section 3.4).
- The generated items are absent from the documentation on docs.rs (Section 6.4).
- A crate that names a generated item other than `Shader` in a `use` declaration, an item signature, or a struct field does not build its documentation on docs.rs in M6. That is the ordinary way to use typed bindings, and the owner accepts the limit for M6 (Sections 3.8 and 6.4, SLMQ-3).

These are not determined: the macro in rust-analyzer and in a cross build that links, the rpath alone on macOS without `DYLD_LIBRARY_PATH` (SLMA-5), and the behaviour of cargo-semver-checks on hidden items.

## 16. References

### 16.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| SLM-1 | 4.2 | The generator lives in the library crate `slango-codegen`, and the macro crate holds only the entry point. |
| SLM-2 | 4.2 | `slango-codegen` requires `slango`, and `slango-macros` requires `slango-codegen`, with exact versions. |
| SLM-3 | 4.2 | Use `syn` 2 and `prettyplease` 0.2, and no `proc-macro-error`. |
| SLM-4 | 4.2 | No unsafe code and no logs in either crate. |
| SLM-5 | 5.1 | The snapshot records each structured buffer's element type, and `bind` rejects a mismatched scalar or vector element and keeps the stride check for a structure. |
| SLM-6 | 5.2 | `slango` provides the hidden, unstable module `__private` for generated code. |
| SLM-7 | 5.3 | The encoding is private, little-endian, and versioned, and a wrong version fails the load. |
| SLM-8 | 5.4 | The hidden trait `StructElement` gives generated structs `Element` and `Value`. |
| SLM-9 | 6.1 | One macro, `shader!`, which takes one string literal. |
| SLM-10 | 6.1 | Paths resolve against the invoking file, with no fallback. |
| SLM-11 | 6.2 | The expansion includes every file that Slang read with `include_bytes!`. |
| SLM-12 | 6.3 | The build script prints only the rpath on Linux and macOS, its rerun instructions, and the `cfg` of SLM-13. |
| SLM-13 | 6.4 | With `DOCS_RS`, the macro emits a stub and calls no generator. |
| SLM-14 | 6.5 | Every failure is a compile error at the literal, never a panic. |
| SLM-15 | 7.1 | `Shader` loads the embedded snapshot and looks up every kernel. |
| SLM-16 | 7.1 | Each kernel struct dispatches through the public call API with literal paths. |
| SLM-17 | 7.2 | Parameters map to field types by the table of Section 7.2. |
| SLM-18 | 7.3 | Element structs are `#[repr(C)]` with padding, `new`, layout assertions, and `StructElement`. |
| SLM-19 | 7.4 | Names follow fixed rules, and a conflict is a compile error. |
| SLM-20 | 8 | `slango-codegen` has one `Error` type with three kinds. |
| SLM-21 | 9 | `Shader` and kernel structs are `Send` and `Sync`, asserted in a test. |
| SLM-22 | 10 | A change of the pin updates the expected text of the compile-fail tests. |

## Change Log

- 2026-11-12: Audited the document as a design: removed Section 1.6 "Current State" and Section 11 "Implementation Order for M6", restated the facts of Section 3 without the record of the spike, its runs, and its measurements, removed the history of steps from Sections 4 to 10 and 12 to 15, merged the lists of unsupported cases in Section 7.5 and of tests in Section 10, corrected Section 5.2 to the crate-private encoder and decoder and Section 7.5 to the source of the prefix of a message, and split long paragraphs.
- 2026-11-12: Recorded in Section 5.3 that the decoder accepts only bindings in descriptor sets up to 255 and that the encoder refuses a larger set, added the compile-fail case of a string literal with a suffix to Section 10, and corrected in Section 10 the reason why the crate-level example is a `text` block and the statement that the `gpu` tests of saxpy run the bindings of the module `bindings`.
- 2026-11-11: Recorded step 5 of Section 11, the example `typed-kernel`, as done in Sections 1.6 and 11 with its check in CI still open, described the example and its tests as built in Section 10, corrected the statement that the example runs `saxpy.slang` and is the compiled form of the crate-level example, which the `gpu` tests of saxpy are, confirmed SLMA-2 in a narrower form with its evidence in Section 3.8 and the condition in Section 6.4, removed its row from Section 12, added the known limit of Section 15.6 for a crate that names generated items outside function bodies, and widened SLMQ-3 to that crate's documentation on docs.rs, answered by M7.
- 2026-11-11: Recorded step 4 of Section 11, the generated structs for Slang structures, as done in Sections 1.6 and 11, recorded the element form, the uniform form, the sharing of one structure name, and the lint exception of a generated `new` as built in Section 7.3, the names of structures and their fields in Section 7.4, the unsupported cases of structures with the form of their message in Section 7.5, and the tests as built in Section 10, corrected the array case of Section 7.5 to a stride that differs from the element size, confirmed SLMA-7 in Section 3.9, confirmed SLMA-6 of step 3 from its CI run in Section 3.3 and narrowed SLMA-5 without confirming it, because the macOS job also sets `DYLD_LIBRARY_PATH`, with the time of the trybuild test on Windows in Section 3.10 and the matching updates in Sections 1.6, 11, 12, 15.2, and 15.6, and removed the rows of SLMA-6 and SLMA-7 from Section 12 and kept the narrowed row of SLMA-5.
- 2026-11-11: Recorded step 3 of Section 11, the crates `slango-codegen` and `slango-macros`, as done in Sections 1.6 and 11, recorded its costs in Section 3.10, the generator's functions, the dependencies of both crates, and the exception to DEP-2 for the exact requirement on `slango` in Section 4.2, the checks of `VULKAN_SDK` in Section 6.3, the stub without a path check in Section 6.4, the settled naming rules in Section 7.4, the unsupported cases as built in Section 7.5, the error kinds as built in Section 8, and the tests as built with the manual check of the rebuild in Section 10, confirmed SLMA-1 and removed its row from Section 12, and added the evidence for SLMA-3 to Section 4.2.
- 2026-11-11: Recorded step 2 of Section 11, the hidden module `slango::__private`, as done in Sections 1.6 and 11, corrected Sections 5.1 and 5.4 to the element types and the rejection of a vertex buffer that the implementation has, recorded how step 2 settled the interface description in Section 5.2 and the encoding with its tests in Section 5.3, closed the gap of Section 3.5, named the test that pins the bytes of the encoding in Section 10, and confirmed SLMA-4 in Section 3.9 and removed its row from Section 12.
- 2026-11-11: Recorded step 1 of Section 11, the check of the element type of a bound buffer in `slango`, as done in Sections 1.6, 5.1, and 11, put the problem of the stride-only check in the past in Section 1.1, and narrowed SLMA-4 to the Slang name of a uniform structure in Section 12.
- 2026-11-11: First draft, written after the spike of M6 as the first task of M6.
