# `slango-macros` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-11 |
| Applies to | `crates/slango-macros`, `crates/slango-codegen`, and the hidden module `__private` of `crates/slango` |

## Abstract

This document specifies milestone M6: the crate `slango-macros`, whose procedural macro `shader!` compiles a Slang file during the compilation of a Rust crate and generates typed Rust bindings to its compute kernels, the library crate `slango-codegen`, which holds the generator, and the changes to `slango` that both need. It starts from the facts that a spike established about compiling Slang inside a procedural macro: how the macro library finds Slang, how paths resolve and rebuilds follow the shader, how a precompiled snapshot loads, how docs.rs builds a crate that uses the macro, and what it costs. On those facts it records the owner's decisions, defines the generated items and the hidden module of `slango` through which they reach the runtime core, and states the type check that `slango` gains first. It states the errors, the threading, the tests, the known limits, and the order of the changes that implement M6. It ends with the assumptions to confirm and the open questions.

## 1. Introduction

### 1.1. Problem

The runtime API of `slango` sets every parameter by name and checks it at the dispatch (Section 9 of `docs/design/slango.md`). A misspelled name, a value of the wrong type, or a buffer of the wrong element type fails only when the program runs. A user also writes by hand every Rust struct that mirrors a Slang structure in a buffer, with its padding, and nothing checks it against Slang's layout. Until step 1 of Section 11, `slango` checked only the stride of a bound buffer, so a `Buffer<u32>` bound to a `StructuredBuffer<float>` without an error.

### 1.2. Purpose

`slango-macros` turns these run-time mistakes into compile errors. The macro reads a `.slang` file, compiles it with the pinned Slang, and generates one typed kernel for each compute entry point and one Rust struct for each Slang structure that the kernels take, with layout assertions. The generated code calls the same core as the runtime API (ARCH-17). This document implements Section 8.4 and milestone M6 (Section 15.9) of `docs/design/architecture.md`, and answers Q-26 and confirms A-10 of that design, as Sections 4.1 and 3.9 record.

### 1.3. Audience

The primary readers are the coding agents that implement M6 and the reviewers who check their changes. A reader is expected to know Rust procedural macros and the public API and layout snapshot of `slango` (Sections 5.1, 7.1, and 9 of `docs/design/slango.md`).

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

### 1.6. Current State

This document exists as a draft dated 2026-11-11. The spike of M6 ran on 2026-11-11 on a copy of the repository that the repository does not keep, and Section 3 records what it established. Step 1 of Section 11 is done, on 2026-11-11. It added the check of the element type of a bound buffer of SLM-5 to `slango`, which Section 9.3 of `docs/design/slango.md` describes with its behaviour change. Step 2 is done, on 2026-11-11. It added the hidden module `slango::__private` of Section 5.2, with `precompile_file`, the interface description, the encoding of Section 5.3 with `ENCODING_VERSION`, `load_precompiled`, and the trait `StructElement` of Section 5.4, and split the load of `slango` into a compile and a load, as Section 7.1 of `docs/design/slango.md` describes with the behaviour changes. Where the implementation settled or corrected this design, Section 5 states it. No code of `slango-macros` or `slango-codegen` exists, and the next change is step 3 of Section 11.

### 1.7. Defects of Third-Party Components and Trusted Input

`slango-macros` follows Hard Rule 14 of `AGENTS.md` and Section 1.7 of `docs/design/slango.md`. The `.slang` file, every file it imports, and the path in the macro call are trusted input from the crate that calls the macro. The macro does not guard against a defect of Slang. Section 15.6 lists the properties of compiling Slang inside the compiler, which are limits and not defects.

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

## 3. Facts From the Spike

This section is non-normative. It states each fact that a later section relies on, once, and says how it is known.

### 3.1. Sources and Evidence

Each fact carries one of these marks:

- (run): Verified on 2026-11-11 on x86_64 Linux with Rust 1.99.1 and the pinned Vulkan SDK, on a copy of the repository at commit `aa5c195` with a changed `slango`, a generator crate, a procedural macro crate, and a crate that used the macro. GPU results ran on an NVIDIA GeForce RTX 4070 Laptop GPU and on lavapipe, `llvmpipe (LLVM 19.1.7, 256 bits)`, with the same results.
- (read): Read on 2026-11-11 in the named source.
- (inferred): Concluded from other facts and not verified. Section 12 lists the assumptions on which a requirement depends.

Nothing of the spike ran on Windows or macOS, in rust-analyzer, or as a cross build that links.

### 3.2. Mechanism

Three mechanisms were compared: a function-like procedural macro that compiles during expansion, a library for build scripts that writes Rust into `OUT_DIR`, and a macro that only declares the interface and checks it at run time (run). The procedural macro generated a typed kernel for `examples/shaders/saxpy.slang` and a `#[repr(C)]` struct for a structured buffer of a structure, and both gave the exact expected results on both adapters (run). The generated code called only `Kernel::call`, `set`, `bind`, `dispatch`, and one new load path for a precompiled snapshot (run). The build-script library worked and gave clearer errors without a library path, but its diagnostics carry no span (run). The declared macro needs no Slang at build time, and a field of the wrong type failed only at the first dispatch (run). vulkano-shaders 0.35.0 generates `#[repr(C)]` structs with padding and load functions from reflection, inline-spirv and include-wgsl-oil are procedural macros of the same kind, and wgsl_to_wgpu and spirv-builder are build-script generators (read, the source or documentation of each crate).

### 3.3. Loading the Macro Library

The macro library links `libslang-compiler.so.0.2026.17` dynamically and has no rpath (run). Without `$VULKAN_SDK/lib` on the library path of rustc, `cargo build`, `cargo check`, and `cargo doc` all fail with `error[E0463]: can't find crate for 'slango_macros'`, and nothing names the missing library (run). A doctest goes through rustdoc, which loads the macro the same way (inferred). With `cargo::rustc-link-arg=-Wl,-rpath,$VULKAN_SDK/lib` printed by the build script of the macro crate, the library gets that directory as its `RUNPATH`, and all three commands pass with `LD_LIBRARY_PATH` unset (run). A link argument of `slang-compiler-sys` would not reach the macro, because `rustc-link-arg` applies only to the targets of the package that prints it (read, the Cargo reference). Cargo removes library paths outside `target/` from every process it starts (read, the Cargo reference). rust-analyzer loads macro libraries in its own process (read), so it needs the rpath or an environment of its own (inferred). On Windows the macro library needs `slang-compiler.dll` on `PATH`, and an rpath does not exist there (inferred).

### 3.4. Paths and Rebuilds

A path resolved from `CARGO_MANIFEST_DIR` failed under trybuild, which compiles each case in a package of its own (run). A path resolved relative to the invoking file, through `proc_macro::Span::call_site().local_file()`, stable since Rust 1.88.0 (read, the documentation of `proc_macro`), worked in the workspace and under trybuild, once the result, which is relative to the working directory of rustc, was made absolute against that directory (run). The documentation of `local_file` says that its result should not be embedded in the output of a macro, and vulkano-shaders and include-wgsl-oil embed such paths anyway (read).

An expansion that emits `const _: &[u8] = include_bytes!("<absolute path>");` for the file and for every file of `Module::dependency_file_paths` makes Cargo rebuild the crate when any of them changes: after a change of a file that the shader imports, Cargo reported the file as changed and recompiled, and the next build was fresh (run). `proc_macro::tracked::path` does not compile on stable 1.99.1, and its tracking issue is open (run, read). Neither mechanism notices a new file that would change how an `import` resolves (inferred).

### 3.5. Embedding and Loading

A load from an embedded, encoded snapshot took 0.89 to 0.93 ms, and a load that compiled embedded source took 148 to 151 ms (run). The encoded snapshot of `saxpy.slang` is a byte string of about 1.5 kB (run). The program still has `libslang-compiler.so.0.2026.17` as a needed library in both cases and fails to start without the library path, because `slango` depends on `slang-compiler` unconditionally (run). An embedded source that imports another file fails with `cannot open file 'common.slang'`, because `load_module_from_source` sets no search path, while the precompiled snapshot resolved the import at build time (run).

The spike split `Device::load` into a compile into the layout snapshot and a load from it, added a hidden public module with the precompile function, the interface description, a little-endian encoding with a magic header and version, a loader from the encoding, and a trait for generated structs with blanket implementations of the sealed traits (run). Coherence accepted the blanket implementations beside the existing ones (run). Afterwards, the 185 unit tests of `slango` and all 305 of its `gpu` tests passed (run). The encoding was exercised only for storage buffers and global uniforms. Immediates, textures, samplers, and render records were encoded but never loaded. Step 2 of Section 11 closed this gap, except for a draw from a precompiled module (Section 5.3).

### 3.6. The Static Route

With `SLANG_SOURCE_DIR` set and no SDK, the crate that used the macro built in 5 min 8 s, and the macro library and the program had no Slang library as a needed library (run). The saxpy kernels ran. The kernel with a structure failed at load with `UnsupportedShader` and naga's `UnsupportedInstruction(Function, CopyLogical)`, because Slang skips `spirv-opt` without `slang-glslang`, which the static build lacks (run, and read, `docs/design/slang-compiler-sys.md`). So the SPIR-V that the macro embeds depends on the Slang of the machine that expanded the macro (inferred).

### 3.7. Diagnostics, Aborts, and Cross-Compilation

A Slang error, returned as `syn::Error` at the span of the string literal, appears as a compile error with `slango`'s text and its causes, which contain Slang's diagnostics with their own layout (run). A missing file gives `cannot read a Slang file` with the operating system's error as the cause (run). `proc_macro::Diagnostic` is unstable, so the macro cannot point into the `.slang` file (read). A panic in the macro becomes `error: proc macro panicked` at the call, and an abort, simulated as `abort_on_exception` aborts, kills rustc with `SIGABRT` and no diagnostic of its own (run). No real input made Slang abort: a structure nested 200 deep gave an ordinary compile error (run). `cargo check --target x86_64-pc-windows-msvc` from Linux passed and ran the macro with the host's Slang (run). The generated SPIR-V and snapshot do not depend on the target (inferred).

### 3.8. docs.rs

With `DOCS_RS` set and no SDK, `cargo doc` failed with `E0463`, because the macro library is linked with Slang's symbols unresolved (run). With a `cfg` that the macro's build script sets when `DOCS_RS` is present, under which the macro emits a stub and calls no generator, the library had no undefined Slang symbol and `cargo doc` passed, without the generated items in the documentation (run).

### 3.9. Generated Code

For `struct Particle { float3 position; float mass; float2 velocity; }` in a `RWStructuredBuffer`, Slang reported offsets 0, 12, and 16 and a stride of 32, and the generated `#[repr(C)]` struct with a private tail padding of 8 bytes, a `const fn new`, and `size_of` and `offset_of!` assertions matched it and read back correctly (run). Private padding fields rule out a struct literal in user code (run). The spike did not generate arrays inside structures whose stride exceeds the element size, matrices or `bool` in structures, or nested structures, whose code path is untested. A structure in a uniform buffer is flattened into one plain value for each leaf with a dotted path such as `params.scale` (read, Section 7.1 of `docs/design/slango.md`). A unit test of the tokens generated from a hand-made interface description, formatted with `prettyplease`, passed natively and under Miri (run). Every insta snapshot test of the repository sits in a module that Miri skips (read).

Step 2 of Section 11 confirmed SLMA-4 on 2026-11-11. The walk records the Slang name of each structure among the plain values with its parameter path. The `ffi` test `layout::tests::ffi::each_structure_among_the_plain_values_is_recorded_with_its_path_and_slang_name` of `slango` recorded `params` with `Params`, `params.inner` with `Inner`, `settings.inner` with `Inner`, `run` with `Run`, and `run.inner` with `Inner`. These cover a global `uniform` structure, a structure nested in it, a structure inside a `ConstantBuffer<Params>`, and a `uniform` structure of the entry point. The walk also recorded `Particle` above as a structure element of 32 bytes with its fields at the offsets 0, 12, and 16.

This confirms A-10 of the architecture design: vulkano generates typed shader interfaces from reflection.

### 3.10. Costs and Dependencies

One expansion took 141 to 159 ms when idle and up to 236 ms under load, the cost of one runtime load (run). A procedural macro runs again on every compilation of the crate that calls it, and with three calls a rebuild after a change of the Rust source took about 0.93 s (run). A clean debug build took 18.7 s against 17.3 s for `slango` alone (run). A clean release build took 34.0 s against 25.0 s, 36 % more, because Cargo builds `slango`, `wgpu`, `naga`, and `slang-compiler` a second time as host dependencies (run, inferred for the cause). The debug macro library is 91.7 MB on the dynamic route and 125 MB on the static route (run). The first trybuild run took 169.7 s and a later one 0.64 s (run). The same expected text passed on stable 1.99.1 and nightly 1.102.0 (run).

`syn`, `quote`, `proc-macro2`, `prettyplease`, `trybuild`, and `target-tuple`, which trybuild brings, are `MIT OR Apache-2.0`, which the allow-list admits, and each declares a `rust-version` of 1.88 or less (run). `cargo deny` reported no advisory and one new duplicate, `prettyplease` 0.2.37, which `bindgen` of Slint's iOS-only renderer brings, beside the 0.3.0 of the spike (run). `syn` 2 and `syn` 3 are both in the lock file already, with a skip for `syn` 2 (read, `deny.toml`). `proc-macro-error` and `proc-macro-error2` are unmaintained (read, RUSTSEC-2024-0370 and RUSTSEC-2026-0173).

## 4. Crate Structure and Dependencies

### 4.1. Decisions

The owner decided on 2026-11-11:

1. A function-like procedural macro in `slango-macros` that runs Slang during compilation, so every crate that uses it needs the pinned Slang at build time (Section 6).
2. The generated code embeds the encoded snapshot and loads it without a call into Slang, and the program still links Slang in M6 (Section 5).
3. Compute kernels only, with generated structs for Slang structures (Section 7).
4. The first step of M6 is the element type check of `bind` in `slango` (Section 5.1).
5. `slango` does not re-export the macro in M6. A user depends on `slango-macros` directly. This answers Q-26 of the architecture design together with Sections 6.1 and 10.
6. The tests of Section 10.
7. The dependencies of Section 4.2, with no new licence and no new advisory exception.
8. The known limits of Section 15.6, which are recorded and not guarded against.
9. The order of Section 11.
10. The separate published library crate `slango-codegen` for the generator (SLM-1).
11. The element type check of `bind` is exact for a plain element and keeps the stride check for a structure (SLM-5).

### 4.2. Packages and Dependencies

SLM-1. The generator MUST live in the library crate `slango-codegen`, and `slango-macros` MUST hold only the macro entry point, its path resolution, its build script, and the conversion of errors to compile errors. A procedural macro crate exports nothing but macros, and the spike ran the pure tests of the generator under Miri only in a library crate (Section 3.9), so a generator inside the macro crate could leave its pure tests without Miri.

The owner approved `slango-codegen` as a separate published crate on 2026-11-11, and Sections 3.1, 3.2, and 13 of `docs/design/architecture.md` list it. Its crates.io name is checked before the first release (SLMA-8).

`slango-codegen` has two steps: one from a path to an interface description, which calls `slango` and so Slang, and one from an interface description to tokens, which is pure. It depends on `slango`, `proc-macro2`, and `quote`, and on `syn` with the feature `full` and `prettyplease` for its tests only. `slango-macros` depends on `slango-codegen`, `proc-macro2`, and `syn`. Both are published, because a published crate cannot depend on an unpublished one.

SLM-2. `slango-codegen` MUST require `slango` with an exact version, `=x.y.z`, and `slango-macros` MUST require `slango-codegen` with an exact version, because the encoded snapshot and the hidden module of `slango` have no stability across versions (SLM-6), so a mismatch would fail at load or not compile.

Each requirement also carries the path, as TOOL-25 of `docs/design/tooling.md` requires. With an exact requirement, Cargo resolves the `slango` of the user's crate to the same version, because two semver-compatible versions of a crate cannot coexist in one build graph (inferred, SLMA-3).

SLM-3. The crates MUST use `syn` 2 and `prettyplease` 0.2, the versions already in the lock file, and MUST NOT use `proc-macro-error` or `proc-macro-error2`. TOOL-26 of `docs/design/tooling.md` admits an exception only for a duplicate that no change to the workspace can remove, and these versions add none, while `prettyplease` 0.3 would add a skip. The two crates of error reporting are unmaintained (Section 3.10).

If the generator does not compile against `syn` 2 and `prettyplease` 0.2 (SLMA-1), the step uses `syn` 3 and `prettyplease` 0.3 with one skip for `prettyplease@0.2.37` under TOOL-26. If any step needs a new licence or a new advisory exception, the step stops and the agent asks the owner. `trybuild` is a development dependency of `slango-macros`. Every dependency is declared once in `[workspace.dependencies]` (DEP-2).

SLM-4. `slango-macros` and `slango-codegen` MUST contain no unsafe code and MUST emit no logs, because neither needs either: they call only safe `slango` code and report every failure as a compile error or an error value. This answers Q-16 of the architecture design for both crates.

SIMP-4 of the coding standard prefers functions to macros. The macro is the deliverable of M6 that Section 8.4 of the architecture design decides, and the generator behind it is plain functions and types.

## 5. Changes to `slango`

### 5.1. The Element Type Check

This section changes the behaviour of `slango` for every user, macro or not.

SLM-5. The layout snapshot MUST record the element type of each structured buffer in its storage buffer record. When the recorded element is a scalar or a vector, `Call::bind` and `Draw::bind` MUST report `ErrorKind::InvalidArgument` naming the parameter unless the Rust element type has the same scalar and component count. When the recorded element is a structure, they MUST keep the check of the stride alone. A check of the stride alone lets a `Buffer<u32>` bind to a `StructuredBuffer<float>` and read wrong values without an error (Section 1.1), while a buffer of structures must stay usable without the macro.

The full element type has the four forms that Section 7.1 of `docs/design/slango.md` defines: a scalar or vector of `bool`, `int`, `uint`, or `float` with its component count, a scalar or vector of another scalar type with its Slang name, a structure with its Slang name, size, and fields, and a composite, which is a matrix, an array, or another type. A field of a structure can be a plain value, a nested structure, or another type with its Slang name, such as `half` or `Inner[2]`. The check runs at the dispatch or the draw with the other checks of SLO-14 and SLO-26 of `docs/design/slango.md`. A buffer of a built-in `Element`, such as `[f32; 4]`, still binds to a structured buffer of a structure of the same stride, so a user of the runtime API alone keeps today's behaviour for structures. Generated bindings are exact in both cases, because the element type that the generator emits for a structure is that structure's own struct (Section 7.3). The generator also reads the recorded element type to choose the Rust type of each buffer field (Section 7.2).

Step 1 of Section 11 implemented SLM-5 and confirmed the part of SLMA-4 on structured buffers: the walk records the element type of each structured buffer (Section 7.1 of `docs/design/slango.md`). Step 2 added the Slang name, size, and fields of a structure, which the generator needs and the check does not read. Section 9.3 of `docs/design/slango.md` states the rules of the check, the choices of the implementation for `bool`, other scalar types, matrices, and arrays, and the behaviour change.

### 5.2. The Hidden Module

SLM-6. `slango` MUST provide the module `#[doc(hidden)] pub mod __private`, through which generated code and `slango-codegen` reach the core of the runtime API, and MUST NOT promise any stability for it. Its contents change with any version of `slango`, and SLM-2 matches the versions exactly, so no user can depend on it.

The module holds:

- The split of loading into `layout::compile` and a crate-private load from the snapshot, which Section 7.1 of `docs/design/slango.md` describes. `Device::load_module` and `precompile_file` share the crate-private `module::compile_file`, which reads the file.
- `precompile_file`, which compiles a file exactly as `Device::load_module` does, with the same session, search path, and reflection walk (SLO-5 and SLO-7 of `docs/design/slango.md`), and returns the interface description and the encoded snapshot.
- The interface description: kernels with their name, thread group size, and parameters, each parameter with its path, kind, and type, including the Slang name of a structure and the element type of a structured buffer, and the files that Slang read.
- The encoder and the decoder of the snapshot (Section 5.3).
- `load_precompiled`, which decodes an encoded snapshot and loads it with no call into Slang.
- The trait for generated structs (Section 5.4).

Generated code calls `slango` only through its public API and this module, so it compiles and dispatches through the same core as the runtime API (ARCH-17).

Step 2 settled the interface description as follows:

- Its types are public types of `__private` that mirror the crate-private layout types, because the layout types stay crate-private (Section 5.3). They are not `#[non_exhaustive]`, so that the tests of step 3 can build them with literals and match them exhaustively.
- It describes each compute entry point and leaves out the vertex and fragment entry points, for which the generated code offers nothing (Section 7.5).
- The parameters of a kernel follow the order of its bindings, by set and binding. The plain values of each uniform buffer stand at the place of that buffer, and the `uniform` parameters of the entry point come last. The parameters that the compiled code does not use are listed too (Section 7.1).
- A plain value inside a structure is listed under its dotted path, and each structure among the plain values is listed once more with its path and Slang name, so the generator rebuilds the nesting from the paths. SLMA-4 is confirmed for these names (Section 3.9).
- The files that Slang read are part of the description and not of the encoded snapshot, so the encoded bytes embed no path of the build machine.

### 5.3. The Encoding

SLM-7. The encoding MUST be private to `slango`, little-endian, and versioned by a magic header with a version number, and `load_precompiled` MUST reject a byte string of another version or with trailing bytes with `ErrorKind::InvalidArgument`, because an encoding that decodes wrongly would load a kernel with wrong bindings.

The layout types stay crate-private. The generated code also asserts at compile time that `slango::__private::ENCODING_VERSION` equals the version it was generated for, so a mismatch that SLM-2 does not prevent fails to compile instead of at load. A round-trip test covers every record of the snapshot, including uniform buffers with arrays, immediates, textures, samplers, and render records, which the spike did not load (Section 3.5).

Step 2 implemented the encoding with these properties:

- The bytes start with an 8-byte magic header, followed by `ENCODING_VERSION`, which is 1, as a little-endian `u32`. The version changes whenever the bytes of the encoding, or the meaning of any of them, change.
- `load_precompiled` also rejects truncated or malformed bytes with `ErrorKind::InvalidArgument`. The decoder does not panic, bounds every count by the bytes left before it allocates, and accepts at most 256 levels of nested arrays or structures.
- The encoder returns `ErrorKind::UnsupportedParameter` for a snapshot that the decoder would reject, such as one with a count above `u32::MAX`, so it never writes bytes that the decoder refuses.
- The pure tests decode every record and variant of the snapshot back to itself, and reject every truncation of an encoding. Each byte of an encoding set to 0 or 255 either fails to decode or decodes to a snapshot that encodes to the same bytes. These tests use generated variants and exhaustive sweeps, not property tests with regression files as TEST-9 of the coding standard asks, because the step added no dependency.
- An `ffi` test compiles each test shader of `slango` with `precompile_file` and checks that the decoded snapshot equals the snapshot that `Device::load_module` builds, render records included. The `gpu` tests run kernels from a precompiled load. No draw from a precompiled module has run.

### 5.4. Generated Structs and the Seal

SLM-8. `slango` MUST provide in `__private` the trait `StructElement`, with the Slang name, the stride, and the conversion to and from little-endian bytes at the reflected offsets, and MUST implement `Element` and `Value` for every type that implements it. `VertexElement` MUST reject such a type at the draw. Generated structs need the sealed traits, and the hidden trait is the only way in that does not unseal them for users.

The seal stays sound. `slango` has no unsafe code (SLO-2 of `docs/design/slango.md`), so a wrong implementation of `StructElement`, written by hand against the documentation, can produce only wrong bytes or an error, never undefined behaviour. A user who implements a hidden trait is outside the API, as with any `__private` item. Because `VertexElement` is a supertrait of `Element`, the blanket implementations cover it too, and coherence accepts them beside the built-in implementations. Sections 8, 8.2, and 9.2 of `docs/design/slango.md` state the public behaviour that follows: a draw rejects a vertex buffer of such a type (SLM-8), a dispatch or a draw rejects a value of one given to `set`, and the creation of a buffer rejects an element of 0 bytes.

## 6. The Macro

### 6.1. Invocation and Paths

SLM-9. `slango-macros` MUST provide one function-like procedural macro, `shader!`, which takes exactly one string literal, the path of a `.slang` file, and MUST report any other input as a compile error. The generated items appear where the macro is called, so a user calls it inside a module of its own, once per module.

SLM-10. The macro MUST resolve a relative path against the directory of the invoking file, as `include_str!` does, through `Span::local_file`, made absolute against the working directory of rustc. When `local_file` returns nothing, the macro MUST report a compile error and MUST NOT fall back to `CARGO_MANIFEST_DIR`, because a silent fallback resolves the same literal to another file (Section 3.4).

### 6.2. Rebuild Tracking

SLM-11. The expansion MUST contain `const _: &[u8] = include_bytes!("<absolute path>");` for the file and for every file that Slang read, so that Cargo rebuilds the crate when any of them changes (Section 3.4).

The generated code therefore contains absolute paths of the build machine. When `proc_macro::tracked::path` is stable, a change replaces the trick with it. A new file that changes how an `import` resolves does not trigger a rebuild (Section 15.6).

### 6.3. The Build Script

SLM-12. The build script of `slango-macros` MUST print `cargo::rustc-link-arg=-Wl,-rpath,<VULKAN_SDK>/lib` when `VULKAN_SDK` is set and the macro is built for Linux or macOS, `cargo::rerun-if-env-changed` for `VULKAN_SDK` and `DOCS_RS`, and the `cfg` of SLM-13, and nothing else. Without the rpath, every build of a crate that uses the macro fails with a misleading `E0463` unless the user puts the library on the path of rustc (Section 3.3).

The build script reads two variables and prints Cargo instructions. It writes no file, downloads nothing, and installs nothing, so it meets Hard Rule 7 of `AGENTS.md` and SYS-5 of the coding standard. The rpath is the build machine's own directory, stored in the macro library, which only the compiler on that machine loads. On the static route, `VULKAN_SDK` is unset or irrelevant and the macro library needs no Slang library (Section 3.6). On Windows the build script prints no link argument, and the macro library finds `slang-compiler.dll` through `PATH` (SLMA-6). Reading the directory from the `links` metadata of `slang-compiler-sys` would need a direct dependency on it and a change to its build script, so this design keeps the simpler variable.

### 6.4. docs.rs

SLM-13. When `DOCS_RS` is present, the build script MUST set the `cfg` `slango_docs_rs`, and under it the macro MUST emit a documented stub `pub struct Shader;` and MUST NOT call the generator. Otherwise the macro library links Slang's symbols unresolved and rustdoc cannot load it (Section 3.8).

The stub's documentation says that the bindings are generated only in a build with Slang. The other generated items are absent from the documentation of the user's crate. A crate whose public signatures name a generated item other than `Shader` fails to build its documentation without Slang (SLMA-2).

### 6.5. Diagnostics

SLM-14. The macro MUST report every failure as a compile error at the span of the string literal, with the text of the error and each of its causes, and MUST NOT panic for a failure it can report, because a panic shows as `proc macro panicked` without the structure of the error (Section 3.7).

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

SLM-18. For a Slang structure that is the element of a structured buffer, the generator MUST emit a `#[repr(C)]` struct with a public field at each reflected offset, private padding fields for every gap and the tail, a `const fn new` that takes every public field, an assertion of its size and of every field's offset with `size_of` and `offset_of!` in a `const` block, and an implementation of `StructElement`. A layout that Rust and Slang see differently then fails to compile instead of reading wrong values.

A field is `f32`, `i32`, `u32`, a vector of these, an array of these whose size is its length times the element size, or another generated struct. For a structure that appears only as a uniform value, the generator emits a struct with public fields, `new`, and no padding or assertions, because `slango` writes each leaf at its reflected offset (SLO-13 of `docs/design/slango.md`). Every generated struct derives `Clone`, `Copy`, `Debug`, and `PartialEq`. A structure is emitted once, under its Slang name, however many parameters use it. When a structure is both an element and a uniform value, the element form serves both.

### 7.4. Names

SLM-19. The generator MUST derive each Rust name from its Slang name by fixed rules and MUST report a compile error that names both Slang names when two generated items or fields get the same Rust name, because a silent rename would make the generated API depend on the order of declarations.

A field name is the path in snake case, with `.` replaced by `_` and `_` inserted before an uppercase letter that follows a lowercase letter or a digit. A kernel struct name is the entry point name in upper camel case, and its arguments struct adds `Args`. A struct keeps its Slang name. A Rust keyword becomes a raw identifier. A name that cannot be raw, such as `self`, is a compile error, and so is a name that equals `Shader`, `load`, or `module` where it would collide with a generated item.

### 7.5. Not Supported in M6

The macro reports each of these as a compile error that names the kernel, the parameter, the field when there is one, and the reason:

- A structured buffer whose element is a plain type without a built-in `Element`, such as `float3` or `bool`.
- A field of an element structure that is a `bool`, a matrix, or an array whose stride exceeds its element size, such as `float3 a[2]`.
- A parameter of any other kind, such as a byte address buffer.
- 16-bit and 64-bit scalars.

A module with render entry points compiles, and the generated code offers nothing for them. A user reaches them through `Shader::module`.

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

At run time, generated code returns `slango::Error`. A load of an encoded snapshot of another version gives `InvalidArgument` (SLM-7), and every mistake at a dispatch gives the error that `slango` gives for the same call by name.

## 9. Threading

SLM-21. `Shader` and every kernel struct MUST be `Send` and `Sync` without a manual implementation, asserted in a test of the generated code (CONC-1), because they hold only `slango` types, which are both (SLO-17 of `docs/design/slango.md`).

An arguments struct holds references to buffers, textures, and samplers, which are `Sync`, so it is `Send` and `Sync` too. The macro starts no thread.

## 10. Tests

`slango-codegen` has unit tests in its private modules and one integration-test binary, `tests/codegen/` (TEST-10):

- Pure unit tests build interface descriptions by hand and check the tokens of the pure step, formatted with `prettyplease`, with plain assertions: the mapping of each row of SLM-17, the padding and assertions of SLM-18, the names and conflicts of SLM-19, and each error of Section 7.5. Miri runs them.
- The module `ffi` compiles test shaders with Slang and compares the formatted expansion with `insta` snapshots, for saxpy, a structure element with padding, a nested structure, a uniform structure, and a named constant buffer. It also checks the interface description and the list of files read for a shader with an `import`.

`slango-macros` has one integration-test binary, `tests/macros/`:

- The module `ffi` runs trybuild with one passing case and compile-fail cases for a Slang compile error, an unsupported parameter, a name conflict, and input that is not one string literal. The expected text of a Slang error contains Slang's messages, so a new pin can change it (SLM-22).
- The module `gpu` runs generated bindings: saxpy, and a kernel over a structured buffer of a structure with padding, each checked against values computed on the CPU, and the assertions of SLM-21.

`slango` gains unit tests and `gpu` tests of SLM-5 for each kind of mismatch of a scalar or vector element and for a built-in element of the same stride bound to a structure, a round-trip test of the encoding under Miri (SLM-7), a test that pins the bytes of the encoding to `ENCODING_VERSION`, which guards the version rule of Section 5.3, and `gpu` tests that a precompiled load and a load from source give the same results.

The example `typed-kernel` of the examples package runs `examples/shaders/saxpy.slang` through the macro and checks its results, and a `gpu` test of the examples runs it, so a crate that uses the macro builds in CI and `check-doc-without-slang` documents it without Slang (SLM-13).

SLM-22. An agent that changes the pin MUST run the compile-fail tests and update their expected text in the same change, because that text contains Slang's messages, so a new Slang can fail them with no change to the macro.

## 11. Implementation Order for M6

These steps implement M6. Each step leaves `just check` passing and adds its tests.

1. In `slango`, record the element type of each structured buffer in the snapshot and check it in `Call::bind` and `Draw::bind` (SLM-5). Update `docs/design/slango.md` with the behaviour change. Tests: the unit and `gpu` tests of SLM-5. Done on 2026-11-11 (Section 5.1).
2. In `slango`, split loading into compile and load, and add `__private` with `precompile_file`, the interface description, the encoding, `load_precompiled`, `ENCODING_VERSION`, and `StructElement` with its blanket implementations (Section 5). Confirm SLMA-4. Tests: the round trip under Miri and the `gpu` tests of the precompiled load. Done on 2026-11-11 (Section 5).
3. Add `crates/slango-codegen` and `crates/slango-macros` for compute kernels with plain values, buffers of built-in elements, textures, and samplers, with path resolution, tracking, the build script, and the docs.rs stub (Sections 6, 7.1, 7.2, 7.4, 7.5, 8, and 9), the dependencies in `[workspace.dependencies]`, and any change to `deny.toml` that Section 4.2 allows. Record both crates as existing in Section 1.6 of `docs/design/architecture.md`. Confirm SLMA-1, SLMA-5, and SLMA-6 in CI. Tests: the pure tests, the snapshots, trybuild with the rule SLM-22, and the `gpu` test of saxpy.
4. Add generated structs (Section 7.3) with their tests, including the `gpu` test of a structure element. Confirm SLMA-7.
5. Add the example `typed-kernel` with its `gpu` test, and record it in Section 10 of `docs/design/architecture.md`. Confirm SLMA-2 with `check-doc-without-slang`. Check: CI runs the example's test on lavapipe, WARP, and hosted Metal.
6. Complete the crate-level documentation of both crates, with the need for Slang at build time and the limits of Section 15.6, and update the documents that M6 makes false. With this step, M6 is complete.

## 12. Assumptions to Confirm

Each row is a statement that this design relies on and that the spike did not establish. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in Section 3 and in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SLMA-1 | The generator compiles against `syn` 2 and `prettyplease` 0.2 as it did against `syn` 3 and `prettyplease` 0.3 | SLM-3 | Step 3 |
| SLMA-2 | A crate whose public signatures name only `Shader` of the generated items builds its documentation without Slang | Section 6.4 | Step 5 |
| SLMA-3 | An exact version requirement on `slango` makes Cargo resolve the user's `slango` to the same version | SLM-2 | The first release, in M7 |
| SLMA-5 | `-Wl,-rpath` makes the macro library load on macOS with no library path set | SLM-12 | Step 3, in CI |
| SLMA-6 | On Windows, the macro library finds `slang-compiler.dll` through the `PATH` that the CI job sets | Section 6.3 | Step 3, in CI |
| SLMA-7 | A nested structure as a field generates a correct layout | SLM-18 | Step 4 |
| SLMA-8 | The crates.io name `slango-codegen` is available | SLM-1 | The first release, in M7 |

## 13. Open Questions

| Identifier | Question | Answered by |
| --- | --- | --- |
| SLMQ-1 | Should a Cargo feature of `slango` remove `slang-compiler` at run time, so that a program with only precompiled modules needs no Slang library, and what does `load_module` do without it? | A later milestone, not assigned |
| SLMQ-2 | Should `slango` re-export `shader!` behind a feature? | M7, at publishing |
| SLMQ-3 | Should the macro read a checked-in cache of its output when Slang is absent, so that docs.rs shows the generated items? | Not scheduled |

SLMQ-4 asked whether the runtime API needs a way to bind a buffer of a structure without the macro. The owner answered it on 2026-11-11 with SLM-5, which keeps the stride check for a structure, and the row is removed. An identifier is never reused.

## 14. Security Considerations

The threat model treats the `.slang` file, the files it imports, and the path in the macro call as trusted input from the crate that calls the macro, as Hard Rule 14 of `AGENTS.md` states. The macro runs Slang inside rustc with the privileges of the build. Neither crate has unsafe code (SLM-4), and `slango` keeps none (SLO-2 of `docs/design/slango.md`), so a wrong `StructElement` implementation or a corrupt encoded snapshot can produce wrong bytes or an error, never undefined behaviour (Section 5.4). The build script of `slango-macros` writes nothing and downloads nothing, and adds the SDK's library directory to the macro library's rpath (SLM-12). The new dependencies are widely used crates under licences that the allow-list admits, fixed by `Cargo.lock` and checked by `cargo-deny` (SLM-3).

These attacks are out of scope, because the user chose the input: a malicious shader, a shader that reaches a defect of Slang, and a `.slang` file outside the crate that a relative path names.

The residual risk is a defect in Slang that corrupts memory inside rustc during an expansion, and a library planted in the SDK's library directory, which the rpath lets rustc load, as the library path of the recipes already does.

## 15. Operational Considerations

### 15.1. Cargo Features and Environment Variables

Neither crate has Cargo features. The build script of `slango-macros` reads `VULKAN_SDK`, which sets the rpath, and `DOCS_RS`, which selects the stub (SLM-12, SLM-13). Building a crate that uses the macro needs what building `slango` needs: the pinned Slang through `VULKAN_SDK` or `SLANG_SOURCE_DIR` (Section 9 of `docs/design/slang-compiler-sys.md`).

### 15.2. Platforms and Minimum Supported Rust Version

The spike ran on x86_64 Linux only (Section 3.1). Step 3 of Section 11 builds and tests both crates on Windows and macOS in CI. `Span::local_file` needs Rust 1.88.0, and every new dependency declares 1.88 or less, below the workspace's minimum supported Rust version (MSRV) of 1.99.

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

Each call of the macro costs about 150 ms on every compilation of the crate that contains it, and a release build compiles `slango`, `wgpu`, and `naga` twice (Section 3.10). The first trybuild run of a clean target directory takes about three minutes.

### 15.6. Known Limits

These are properties of the design, recorded and not guarded against:

- An abort inside Slang during an expansion kills rustc (Section 3.7).
- The embedded SPIR-V depends on the Slang of the build machine (Section 3.6).
- A release build compiles `slango`, `wgpu`, and `naga` twice (Section 3.10).
- A new file that changes how an `import` resolves does not trigger a rebuild (Section 3.4).
- The generated items are absent from the documentation on docs.rs (Section 6.4).

These were not determined: the macro on macOS and Windows, in rust-analyzer, and in a cross build that links, and the behaviour of cargo-semver-checks on hidden items.

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

- 2026-11-11: Recorded step 2 of Section 11, the hidden module `slango::__private`, as done in Sections 1.6 and 11, corrected Sections 5.1 and 5.4 to the element types and the rejection of a vertex buffer that the implementation has, recorded how step 2 settled the interface description in Section 5.2 and the encoding with its tests in Section 5.3, closed the gap of Section 3.5, named the test that pins the bytes of the encoding in Section 10, and confirmed SLMA-4 in Section 3.9 and removed its row from Section 12.
- 2026-11-11: Recorded step 1 of Section 11, the check of the element type of a bound buffer in `slango`, as done in Sections 1.6, 5.1, and 11, put the problem of the stride-only check in the past in Section 1.1, and narrowed SLMA-4 to the Slang name of a uniform structure in Section 12.
- 2026-11-11: First draft, written after the spike of M6 as the first task of M6.
