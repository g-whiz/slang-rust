# `slang-compiler-sys` and `slang-bindgen` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-16 |
| Applies to | `crates/slang-compiler-sys` and `crates/slang-bindgen` |

## Abstract

This document specifies `slang-compiler-sys`, the crate of raw Rust bindings to the native Slang compiler library, and `slang-bindgen`, the published binding generator that the build script of `slang-compiler-sys` runs in every build to write those bindings from the headers of the Slang that the build links. It defines which declarations the bindings include, the shape of the generated Rust, and how the generator fails on anything it has no rule for. It also specifies the build script, which links the Slang of an installed Vulkan SDK dynamically by default, or, with the Cargo feature `vendored`, builds Slang from the source tree that `slang-src` obtains and links it statically, and the build information that records which Slang a build uses and how.

## 1. Introduction

### 1.1. Problem

Slang exposes its compiler through a C++ header, `slang.h`, whose main API consists of COM-style interfaces, which are C++ classes with only pure virtual methods. Rust cannot call a C++ virtual method directly. It can call one through a `#[repr(C)]` struct of function pointers that has exactly the layout of the C++ vtable.

That struct copies a few hundred method signatures, in the right order and with the right calling convention. A single slot out of order calls the wrong function, and no compiler detects it.

The headers also declare enumerations whose values the library can extend, structs whose C++ default values matter, and constants that exist only as macros. Hand-written declarations drift from the headers with every Slang release, and Slang reuses vtable slots and renumbers enumerations between releases, so declarations generated from one release and linked against another are wrong without any error.

The architecture design (`docs/design/architecture.md`) places these declarations in `slang-compiler-sys` and requires that a generator in this repository produce all of them from the headers of the Slang that the build links (ARCH-33).

### 1.2. Purpose

`slang-compiler-sys` gives the safe crate `slang-compiler` an exact, complete, and checked set of declarations for the Slang that each build links, and links that Slang. `slang-bindgen` produces those declarations from the headers in every build, so that no agent writes a vtable, a UUID, or a thunk by hand (COM-9), and so that the declarations and the library always come from one Slang.

This document implements Sections 3 to 6 and milestone M7 (Section 15.10) of the architecture design for these two crates, and answers the questions that its Section 17 assigns to this document.

### 1.3. Audience

The primary readers are the coding agents that change the crate or the generator, or move the recorded Slang. The secondary readers are the reviewers who check those changes, and the owner of the repository, who approves the decisions.

A reader is expected to know Rust but not the Slang API, C++ application binary interfaces, or libclang. Section 2.1 defines the terms such a reader needs.

### 1.4. Scope

This document covers:

- The declarations that the bindings include, the shape of the generated Rust, and the tested targets and the targets that the build allows.
- `slang-bindgen`, its library and binary, and its tests.
- The build information of `slang-compiler-sys`.
- The build script of `slang-compiler-sys` on the dynamic route and on the vendored route, its use of the shared folder, and how the shared library is found at run time.
- The tests of the crate, Miri, and the requirements on CI.
- The crate documentation and the dependencies.

### 1.5. Non-Goals

This document does not cover:

- The safe API, which the design document of `slang-compiler` defines, including ownership, errors, the `Send` and `Sync` decisions of FFI-9 and CONC-1, and the run-time check of the build tag.
- How the Slang source tree is obtained, the recorded commit, and the scheme of the shared folder, which `docs/design/slang-src.md` specifies.
- The build of slang-glslang and its loader, which `docs/design/slang-glslang-sys.md` specifies.
- 32-bit targets, which Section 6 excludes. A later change can revisit this decision.
- The deprecated compile-request API of `slang-deprecated.h`, including `slang::ICompileRequest`, which the modern interfaces replace (Section 4).
- `slang-gfx.h`, which declares a separate deprecated library, and the other headers that Section 3.2 lists as out of scope.
- Hand-written declarations of any kind in the generated code, which COM-9 and FFI-1 forbid.
- How continuous integration (CI) installs the Vulkan SDK on each runner, which Section 7.3 of `docs/design/tooling.md` specifies. Section 12 states the requirements that the installation meets.
- The recipes, the nextest profiles, and the CI workflow as a whole, which `docs/design/tooling.md` owns. This document states the requirements that this crate and the generator put on them.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that these documents define:

- Section 2.2 of `docs/writing-documentation.md`, such as agent, role, and user.
- Section 2.2 of `docs/rust-coding-standards.md`, such as binding generator, `-sys` crate, safe crate, native library, interface, vtable, thunk, interface pointer, result code, and universally unique identifier (UUID).
- Section 2.1 of `docs/design/architecture.md`, such as application binary interface (ABI), COM-style interface, CMake, dynamic route, global session, libclang, Miri, pkg-config, reflection, run-time search path (rpath), slang-glslang, smoke test, vendored build, vendored route, and Vulkan SDK.
- Section 2.1 of `docs/design/slang-src.md`, such as build metadata, dependent build script, entry, entry key, recorded commit, recorded Slang, required submodule, shared folder, source tree, and stamp file.

This document calls those files the writing guide, the coding standard, the architecture design, and the `slang-src` design, and calls `docs/design/tooling.md` the tooling design. It adds these terms:

- aggregate: A C++ struct, union, or class value, as opposed to a scalar or a pointer.
- build information: The constant `BUILD_INFO` of `slang-compiler-sys`, which records the Slang that a build generated its bindings from and links, and how it links it (Section 8.1).
- build tag: The version string of one Slang build, such as `2026.17-25-gc7954ebd1`, which is the output of `git describe` on the Slang commit: the nearest tag, the number of commits after it, and `g` followed by the abbreviated commit hash.
- C linkage: The property of a C++ function declared inside `extern "C"`, whose symbol in the shared library is its plain name.
- calling convention: The rules for how a function receives its arguments and returns its result, such as which registers hold them and who removes them from the stack. Rust names a calling convention in the ABI string of a function type, such as `extern "C"` or `extern "system"`.
- canonical type: The type that remains after libclang resolves every typedef, such as `long` behind `int64_t` on Linux.
- compiler probe: The run of the C++ compiler that `CXX` names, or of `c++`, with `--version`, whose first line of output identifies the compiler in the entry key of the vendored route outside the Microsoft toolchain (SCS-56).
- default member initializer: A value written in a C++ struct declaration for a field, such as `int size = sizeof(Desc);`, which the field takes when the struct is constructed without that field.
- diagnostic: A message that libclang reports while it parses, with a severity of note, warning, error, or fatal.
- gitlink: The entry of a git index that records a submodule by its commit, with the mode `160000`.
- host: The machine that runs the build script. In Section 9.7, also the private trait `Host` through which the logic of the build script reaches the environment, the file system, the generator, and other programs.
- import library: A file on Windows, with the suffix `.lib`, that tells the linker which symbols a dynamic-link library (DLL) exports.
- inclusion rule: An entry of the rule set of the generator that says whether a declaration, or a group of declarations selected by name or by header, is included in or excluded from the bindings (Section 4.1).
- model: The generator's in-memory description of the declarations that it reads from one parse, such as the interfaces with their methods in order, before it writes any Rust.
- name mangling: The encoding of a C++ function's namespace, class, and parameter types into its symbol name. The encoding differs between compilers.
- opaque handle: A pointer to a type whose contents the header does not declare, so that code can pass the pointer but not read the object.
- shim header: A small header of `slang-bindgen` that replaces a system header, such as `stdint.h`, when the generator parses, so that no parse depends on the system headers of the machine (Section 7.2).
- stamp: The file `slang/stamp` under `OUT_DIR` that records the inputs of the last vendored build that the build script verified (Section 9.3).
- standard layout: The C++ property of a type whose memory layout follows the same rules as a C struct, which `#[repr(C)]` reproduces in Rust.
- synthetic translation unit: A translation unit that the generator writes in memory and parses to evaluate an expression, such as the value of a macro (Section 7.2).
- target: The platform for which code is compiled, named by a target triple such as `x86_64-pc-windows-msvc`.
- translation unit: One C or C++ source file together with every header that it includes, which a compiler or libclang parses as one unit.
- typedef: A C or C++ declaration that gives an existing type a new name, such as `typedef int32_t SlangResult;`.
- work tree: The directory of files that a git repository checks out. Its top level is the directory that holds `.git`.

## 3. The Slang API

This section is non-normative. It states the facts about the Slang headers, the Vulkan SDK, and the Slang source tree on which the later sections rely. Those sections cite it instead of repeating them.

### 3.1. Sources and Tool Versions

The facts in this section hold for the headers, the library, and the source tree of the recorded Slang (Section 7.1 of the `slang-src` design), which is the Slang of the Vulkan SDK 1.4.363.1, at the commit `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`. The generator reads the headers through libclang and the Rust crate `clang` 2.1.0.

Statements about Slang's documented rules come from the documents in the Slang source tree [SLANG-UG] [SLANG-BUILD]. Section 15 lists what is not verified.

### 3.2. Headers

The SDK directory `$VULKAN_SDK/include/slang` holds 15 headers. Only `slang.h` declares the host API. It includes `slang-deprecated.h`, and the file `slang-image-format-defs.h`, which expands inside the declaration of `enum SlangImageFormat`.

`slang-gfx.h` declares a separate, deprecated library. The prelude headers serve generated shader code, not the host API.

The SDK's `slang.h`, `slang-deprecated.h`, and `slang-image-format-defs.h` are byte-identical to those of the source tree. The SDK also has `slang-tag-version.h`, which the source tree has only as the template `slang-tag-version.h.in`, and which a build of the source tree generates in its build tree.

`slang.h` is C++ only, and a parse in C mode fails. It parses with clang at `-std=c++17` without a diagnostic. It includes no header of the C++ standard library, only `stdint.h`, `stddef.h`, `inttypes.h`, and `features.h`.

### 3.3. COM-Style Interfaces

The headers declare 28 interfaces: 27 in `slang.h`, and `slang::ICompileRequest` in `slang-deprecated.h`. Every interface derives from `ISlangUnknown` through single inheritance. `ISlangUnknown` declares `queryInterface`, `addRef`, and `release`, and it is binary-compatible with the `IUnknown` of COM, though Slang needs no COM runtime [SLANG-UG].

No interface overloads a method name, declares a method that is not pure virtual, declares a virtual destructor, or has a data member. Each vtable is therefore the vtable of the base interface followed by the interface's own methods in declaration order.

The interfaces whose names start with `ISlang` are at global scope inside an `extern "C"` block, and the others are in `namespace slang`. `slang::IBlob` is a typedef of `ISlangBlob`.

Each interface declares its UUID with the macro `SLANG_COM_INTERFACE`. It expands to a `static constexpr` member function `getTypeGuid()` that returns an initializer list of integer literals, so the UUID can be read from the syntax tree that libclang returns. `SlangUUID` is a struct of a `uint32_t`, two `uint16_t`, and an array of eight `uint8_t`.

The struct `SlangTerminatedChars` also declares a UUID but has no virtual method and no base. So an interface is a class that has the UUID function and has a virtual method or a base.

Methods use the calling convention macro `SLANG_MCALL`, which is `__stdcall` on the Microsoft compilers and empty elsewhere. In Rust it is `extern "system"`, or `extern "system-unwind"` for a function through which an exception may unwind. One method lacks `SLANG_MCALL`: `slang::IMetadata::isParameterLocationUsed`.

`SLANG_NO_THROW` is `__declspec(nothrow)` on Windows and empty elsewhere. It does not keep every exception inside Slang. Some methods of the recorded Slang let a C++ exception escape for some inputs, such as `IModule::findAndCheckEntryPoint` and `IComponentType::getLayout` (Section 3.6 of `docs/design/slang-compiler.md`).

Some methods declare default arguments, which are not part of the ABI. Some methods take `SlangUUID const&`, which is a pointer in the ABI.

Two methods of `slang::IGlobalSession` are marked `[[deprecated]]` and still occupy vtable slots. Slang's user guide states that interfaces whose names end in `_Experimental` may change between releases [SLANG-UG].

No method and no C function passes or returns an aggregate by value. Some parameters are C++ references, including in two C functions, of the types `const SlangUUID&`, `const SessionDesc&`, `bool&`, `SlangInt&`, and `const char*&`.

Many parameters and return values are `bool`. Five parameter types are callbacks, which are function pointers in the plain C calling convention without `SLANG_MCALL`. No function is variadic.

### 3.4. C Functions and Reflection

The headers declare 278 functions with C linkage, and `libslang-compiler.so` exports all of them. Of these, 21 are in `slang.h`: 20 named `slang_*`, and `spGetBuildTagString`, which needs no session.

The other 257 are in `slang-deprecated.h`, and 172 of them are named `spReflection*`. The bindings include the 21 of `slang.h` and the 172 `spReflection*` functions (Section 4.1).

A global session is created with `slang_createGlobalSession2`, which takes a `const SlangGlobalSessionDesc*` and writes a `slang::IGlobalSession*`. `slang_shutdown` and `slang_getLastInternalErrorMessage` are also C functions. `SLANG_API_VERSION` is 0.

Two functions have C++ linkage and are exported only under mangled names: `slang_getEmbeddedCoreModule` and `spReflection_GetSession`. The library also exports one member function defined out of line, `slang::VariableReflection::getDefaultValueBlob`.

Reflection has no COM-style interface. The header declares opaque handles, such as `SlangReflectionType`, and the `spReflection*` functions of `slang-deprecated.h` operate on them.

The C++ reflection classes of `slang.h`, such as `slang::TypeReflection`, consist of inline methods. Each casts `this` to an opaque handle and calls an `spReflection*` function.

The `spReflection*` functions are therefore the ABI of reflection, although the header that declares them calls itself deprecated. Reflection objects are not reference counted.

### 3.5. Types, Enumerations, and Constants

`SlangResult` is a typedef of `int32_t`, binary-compatible with the `HRESULT` of COM, in which a negative value means failure. The result codes exist only as macros, built with the function-like macro `SLANG_MAKE_ERROR`. `SlangBool` is `bool`.

`SlangInt` and `SlangUInt` are 64-bit on 64-bit targets and 32-bit otherwise.

libclang reports 64 enumerations, counting the six enumerations nested in records and the enumerators that `slang-image-format-defs.h` adds. Of these, 14 are `enum class`.

Most unscoped enumerations use the pattern `typedef int XIntegral; enum X : XIntegral`, and nine have no fixed underlying type. Flag types are integer typedefs whose values are enumerators of the form `1 << n`.

The headers declare 36 structs and 3 unions. None has a user-declared constructor, every named record is trivially copyable and standard layout, and 13 have default member initializers.

Several descriptor structs start with a size field whose default is the size of the struct. `structureSize` evaluates to 80 for `SlangGlobalSessionDesc`, 48 for `TargetDesc`, and 96 for `SessionDesc`.

A `flags` field evaluates to 1024. The initializers `nullptr` and `{}` cannot be evaluated through libclang, and they mean a null pointer and zero.

On x86_64, the layouts of the main descriptor structs are these:

| Struct | Size | Alignment |
| --- | --- | --- |
| `SlangUUID` | 16 | 4 |
| `SlangGlobalSessionDesc` | 80 | 4 |
| `TargetDesc` | 48 | 8 |
| `SessionDesc` | 96 | 8 |
| `CompilerOptionEntry` | 40 | 8 |

The headers also declare three `inline constexpr` constants, such as `kDefaultTargetFlags`.

The headers define 121 object-like macros. libclang does not evaluate a macro definition. A synthetic translation unit that includes `slang.h` and declares one `constexpr auto` variable for each macro evaluates them.

The values include `SLANG_OK` 0, `SLANG_FAIL` 0x80004005, `SLANG_E_NOT_IMPLEMENTED` 0x80004001, `SLANG_E_NO_INTERFACE` 0x80004002, `SLANG_E_INVALID_ARG` 0x80070057, and `SLANG_E_NOT_FOUND` -2113929211. Platform macros, such as `SLANG_LINUX`, take values that depend on the target.

### 3.6. Version Information

`$VULKAN_SDK/include/slang/slang-tag-version.h` defines `SLANG_TAG_VERSION` as `"2026.17-25-gc7954ebd1"` and `SLANG_VERSION_NUMERIC` as `"2026.17"`. The pkg-config file `$VULKAN_SDK/lib/pkgconfig/slang-compiler.pc` gives only `Version: 2026.17` and states that `pkg-config --static` is not supported.

At run time, `spGetBuildTagString()` and `slang::IGlobalSession::getBuildTagString()` return `2026.17-25-gc7954ebd1`. The Slang headers do not record the version of the Vulkan SDK. It appears only outside `$VULKAN_SDK/include`, such as in the SDK's `README.txt` and in the pkg-config file of the Vulkan loader.

The SDKs 1.4.363.1 for Linux, macOS, and Windows ship the same Slang build. In each, `slang-tag-version.h` defines `SLANG_TAG_VERSION` as `"2026.17-25-gc7954ebd1"`, and `slangc -version` prints the same string, which `check-vulkan-sdk` checks in every CI job (Section 12).

In the source tree, the recorded commit also carries the tag `vulkan-sdk-1.4.363.0`, and `git describe --tags --match 'v20[2-9][0-9].[0-9]*'` prints `v2026.17-25-gc7954ebd1`. Slang's CMake derives its version only from `git describe`, in `cmake/GitVersion.cmake`. It accepts the override `-DSLANG_VERSION_FULL=` and falls back to `0.0.0-unknown`, which is what a shallow clone without tags gives.

A statically linked Slang built with the fallback reports, as its build tag, the modification time of the program that contains it, because `spGetBuildTagString` then reads the time of the module that holds Slang. The build tag of a vendored build is therefore only meaningful when the build passes it explicitly (SCS-28).

### 3.7. Shared Library

On Linux, `$VULKAN_SDK/lib/libslang-compiler.so` is a symbolic link to `libslang-compiler.so.0.2026.17`, whose shared object name (SONAME) is the same. It depends only on libstdc++, libm, libgcc_s, and libc. `libslang.so` is a compatibility link to it.

At run time, the library loads `libslang-glslang-2026.17.so` and `libslang-glsl-module-2026.17.so` by name from its own directory. They are not link-time dependencies.

Slang's building documentation states that "the public ABI for Slang libraries in general is not currently stable", which is why the major version of the shared library is 0 [SLANG-BUILD].

A program linked with `-L$VULKAN_SDK/lib -lslang-compiler` needs `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH`, or an rpath, to start. Without it, the loader reports "cannot open shared object file". `slangc` has the RUNPATH `$ORIGIN/../lib:$ORIGIN`, so a `slangc` that runs does not show that the loader path is set.

The SDKs for macOS and Windows have this layout:

- macOS: `VULKAN_SDK` is the directory `macOS` under the installation root, and it holds `bin`, `Frameworks`, `include`, `lib`, `share`, and `source`. `$VULKAN_SDK/lib/libslang-compiler.dylib` is a symbolic link to `libslang-compiler.0.2026.17.dylib`, a universal binary for x86_64 and arm64, whose install name is `@rpath/libslang-compiler.0.2026.17.dylib`. `slangc` has the rpaths `@loader_path/../lib` and `@loader_path`.
- Windows: `VULKAN_SDK` is the installation root, and its directories are named `Include`, `Lib`, and `Bin`. The file system is case-insensitive, so `include/slang` and `lib` resolve to them.

  `Lib\slang-compiler.lib` is the import library. `Bin` holds `slang-compiler.dll`, `slang-glslang.dll`, `slang-glsl-module.dll`, `slang-rt.dll`, `slang.dll`, and `slangc.exe`.

### 3.8. Differences Between Targets

libclang returns a translation unit even after a fatal diagnostic, such as a missing system header. The model is then wrong without any other sign: methods are missing and types degrade to `int`.

The generator therefore parses with `-nostdinc` and shim headers for `stdint.h`, `stddef.h`, `inttypes.h`, `features.h`, and `TargetConditionals.h`, built on the compiler's predefined macros. With them, the parse for Linux is byte-identical to the parse with the system headers. The parses for `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, and `i686-pc-windows-msvc` report no diagnostic.

On all four targets, the interface names, the bases, and the method order are identical. These differences remain:

- On x86_64 Windows and on macOS, the canonical type `long` becomes `long long` behind the same typedef names.
- The nine enumerations without a fixed underlying type are `unsigned int` on Linux and `int` on Windows, with the same size.
- Every struct layout is identical on the three 64-bit targets.
- On `i686-pc-windows-msvc`, every method with `SLANG_MCALL` uses `__stdcall`, `slang::IMetadata::isParameterLocationUsed` uses `__thiscall` because it lacks `SLANG_MCALL` (Section 3.3), and the layout of every struct that holds a pointer differs.

The generator writes the same text for every tested 64-bit target from the headers of the recorded Slang (Section 6).

### 3.9. Lifetime and Threading Rules

Slang documents these rules, which the safe crate has to respect and which the crate documentation of `slang-compiler-sys` states (Section 13):

- A global session is not thread-safe. A program that compiles on several threads at once uses one global session per thread. Creating a global session and calling `spGetBuildTagString` are thread-safe [SLANG-UG].
- A global session should be released after every object created from it [SLANG-6344].
- Reference counting is not guaranteed to be atomic on every interface, so code does not add or release references to one object from unrelated threads [SLANG-UG].
- A `ProgramLayout` is not reference counted and lives as long as the `slang::IComponentType` that it came from [SLANG-UG].

### 3.10. Source Build

The setting `SLANG_LIB_TYPE=STATIC` produces `libslang-compiler.a`. Slang's building documentation lists the libraries that a consumer of the static library also links: `libcompiler-core.a`, `libcore.a`, `libminiz.a`, and `liblz4.a` [SLANG-BUILD]. The documented list is incomplete.

A build needs CMake 3.22 or newer, or 3.25 for presets, a C++17 compiler, and Ninja. Slang's documentation lists Python 3 only for spirv-tools, which the minimal build of Section 9.3 does not build. The source tree has 21 git submodules under `external/`.

These facts about a minimal static build of the CMake target `slang` with the options of Section 9.3 shape the vendored route:

- The build compiles several hundred C++ translation units. It takes several minutes on a machine with many cores and leaves close to 1 GB under `OUT_DIR`, which is why the vendored route runs in its own CI job (SCS-62) and why the build keeps its libraries in the shared folder (SCS-56).
- It runs without network access. With the default options, a configure without network access tries to download a DirectX Shader Compiler (DXC) binary and then to clone DXC. `SLANG_ENABLE_DXIL=OFF` and `SLANG_SLANG_LLVM_FLAVOR=DISABLE` stop every download of the configuration of Section 9.3.
- `-DSLANG_VERSION_FULL=` alone breaks the configure step, with "set_target_properties called with incorrect number of arguments", because Slang's CMake then leaves `SLANG_VERSION_NUMERIC` empty. `-DSLANG_VERSION_NUMERIC=` with the numeric version fixes it. The numeric version is also part of the names of the modules that the library loads at run time.
- Slang's build writes into its source tree. It rewrites one tracked documentation file with the same content, and it creates and removes temporary files next to its `*.meta.slang` inputs in `source/slang`.

  A build in a read-only source tree fails with "unable to open file for writing", and two builds in one tree race on the same temporary files.
- The link needs six archives, in this order: `libslang-compiler.a`, `libcompiler-core.a`, `libcore.a`, `libcmark-gfm.a`, `libminiz.a`, and `liblz4.a`, followed by the C++ standard library. Without `libcmark-gfm.a`, which the documentation omits, the link fails with undefined references. `-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=` puts all six in one directory.
- `cmake --install` is unusable for the minimal build. It fails, and when forced it installs only `libslang-compiler.a`, which does not link alone, so a consumer links from the build tree.
- The configuration needs 8 of the 21 submodules: `external/cmark`, `external/fast_float`, `external/lua`, `external/lz4`, `external/miniz`, `external/spirv-headers`, `external/unordered_dense`, and `external/vulkan`. A missing one fails at configure time with a CMake error about a missing `CMakeLists.txt`.
- The static library compiles a shader to SPIR-V, GLSL, HLSL, WGSL, and Metal without any file at run time. The standard modules of Slang are not built, and output as SPIR-V assembly fails.

The static library has no slang-glslang, which Slang's build produces only as a separate shared module that Slang loads by name. The optimization of SPIR-V is the difference that matters by default.

`slang-glslang-sys` builds slang-glslang as static libraries and gives Slang a loader for them, which `slang-compiler` installs in a vendored build (`docs/design/slang-glslang-sys.md`). The recorded Slang validates the SPIR-V that it generates only when the environment variable `SLANG_RUN_SPIRV_VALIDATION` is `1`, on either route.

The rest of this section applies only to a global session on which no loader is installed. A program has one when it uses `slang-compiler-sys` directly with the feature `vendored` and installs no loader, or when it adopts a global session that it created itself into `slang-compiler` (SLC-34 of `docs/design/slang-compiler.md`).

When no loader is installed and the dynamic loader finds no `slang-glslang` shared library, the static build behaves as follows:

- The first generation of SPIR-V in each global session succeeds and returns diagnostics that begin `error[E00100]: failed to load downstream compiler 'spirv-opt'`, followed by notes that name the libraries that Slang could not load.
- Later generations in the same global session succeed with no diagnostics. Slang caches the failed load for the global session.
- The SPIR-V is not optimized, and it is not validated even when `SLANG_RUN_SPIRV_VALIDATION` is `1`.

When no loader is installed and the dynamic loader finds a `slang-glslang` shared library, the static build loads it and behaves as the dynamic route, so the output depends on the environment. Whether validation runs in that case with `SLANG_RUN_SPIRV_VALIDATION` set to `1` is not established. These facts are established for the default optimization level on x86_64 Linux only.

### 3.11. Findings of the Generator

These facts about the headers of the recorded Slang determine rules of the generator.

Four enumerations of `slang.h` have no name. Each one holds the values of a flag type and follows the integer typedef of that type in the header:

| Typedef | Underlying type |
| --- | --- |
| `SlangDiagnosticFlags` | `int` |
| `SlangCompileFlags` | `unsigned int` |
| `SlangTargetFlags` | `unsigned int` |
| `slang::SessionFlags` | `uint32_t` |

An unnamed enumeration has no name for a newtype, so Section 4.4 lists each one with its typedef.

Two declarations of `slang.h` cannot be expressed as Rust declarations, so Section 4.3 excludes them:

- `slang::VMExecOperand` has bit-fields, and only inline C++ helpers use it.
- `slang::CompileCoreModuleFlag` is an empty struct, which has size 1 in C++ and would have size 0 as a Rust struct. It only scopes its nested enumeration `slang::CompileCoreModuleFlag::Enum`, which is included.

libclang has these properties that the rule set and the generator have to handle:

- It reports each `static_assert` of `slang.h` as a declaration, so the rule set has to decide it.
- It reports the function-like macro `SLANG_FORMAT`, which writes the enumerators of `SlangImageFormat`, as object-like, so the rule set has to decide that macro as well.
- When it parses for `x86_64-pc-windows-msvc`, clang declares `struct _GUID` implicitly, so the generator ignores forward declarations of records nested in records.

`spReflection_GetSession` has C++ linkage (Section 3.4) and matches the pattern `spReflection*`, so its exclusion has to come before that pattern in the rule set.

The different signedness of an enumeration without a fixed underlying type (Section 3.8) is harmless only while every enumerator fits in `int`. An enumerator such as 0x80000000 would be negative on Windows and positive on Linux, so the newtype of Section 5.3 would have another integer type on each, and the generator refuses it (SCS-19). No enumeration of the recorded headers has such a value.

## 4. Scope of the Bindings

### 4.1. Inclusion Rules

SCS-1. The binding generator MUST include every declaration of `slang.h` except the exclusions that Section 4.3 records with their reasons. It MUST include from `slang-deprecated.h` only the declarations that match `spReflection*`, except `spReflection_GetSession`.

It MUST exclude the rest of `slang-deprecated.h`, including `slang::ICompileRequest`, as well as `slang-gfx.h`, `slang-llvm.h`, and the prelude headers.

Otherwise the bindings would miss the reflection ABI (Section 3.4), expose a deprecated API that the safe crate must not use, or declare items that the library does not export or that Rust cannot express. A new exclusion from `slang.h` is a change to Section 4.3 and to the rule set in the same change.

SCS-2. The binding generator MUST decide inclusion and exclusion through the rule set of its crate and MUST fail when a declaration that it reads matches no rule, because otherwise the headers of a new Slang can add or drop API in a build without any reviewer seeing a decision. No setting of the generator or of the build script relaxes this, so the headers of a Slang other than the recorded one can make a build fail (Section 5.3 of the architecture design).

SCS-3. The binding generator MUST NOT emit a declaration for a function that the library exports only under a mangled name, which for the recorded headers are `slang_getEmbeddedCoreModule`, `spReflection_GetSession`, and `slang::VariableReflection::getDefaultValueBlob`. It MUST NOT emit one for the inline C++ wrapper classes and methods of `slang.h` either.

A mangled name differs between compilers and is no stable link name, and an inline wrapper is not part of the ABI, so a declaration of either fails to link or calls nothing.

SCS-4. When an included declaration refers to a type that a rule excludes, such as a parameter of type `slang::ICompileRequest**`, the binding generator MUST emit that type as an opaque type (Section 5.7). It MUST keep every vtable slot of every included interface, including the slots of deprecated methods, because a vtable with a missing slot calls the wrong function for every later method.

SCS-39. The binding generator MUST fail with an error that names each rule and its reason when a rule of the rule set decides no declaration or macro of the headers. This includes a rule that an earlier rule always shadows and a rule of Section 4.4 that matches no unnamed enumeration.

A rule that decides nothing records a decision about a declaration that the headers no longer have, and a reviewer who reads the rule set then believes that the bindings cover it.

The rule set is the constant `SLANG` in `crates/slang-bindgen/src/rules.rs`, a table of Rust values whose types in the same file are the authoritative definition of its format. Unit tests cover it (Section 7.5). It has three lists of rules, and every rule carries a reason that the generator's error messages quote:

- Declaration rules: each names an optional header, a pattern that matches the qualified C++ name of a declaration, such as `slang::TypeReflection::*`, with `*` matching any text, and whether the declaration is included or excluded. A declaration takes the first rule that matches it.
- Macro rules: each names a pattern for an object-like macro and whether the macro becomes a result code, becomes a constant of a given Rust integer type, or is excluded (Section 5.4). A macro takes the first rule that matches it.
- Unnamed enumeration rules (Section 4.4).

The rule set also names the result type `SlangResult`, the UUID type `SlangUUID`, and the typedefs that map to `isize` and `usize` (Section 5.2). An anonymous struct or union nested in a record takes the decision of the outermost record that contains it. A named type nested in a record matches the rules by its qualified name.

### 4.2. Rationale

This subsection is non-normative. The bindings take all of `slang.h` because the safe crate decides what it wraps, and a `-sys` crate that omits a declaration forces a change of the generator for every later need.

The `spReflection*` functions are the only route to reflection (Section 3.4), so they are included despite the name of their header. The compile-request API in the same header is replaced by the modern interfaces and is left out, as Section 1.5 states. The three symbols with mangled names are left out because Rust would have to hard-code a mangling that only one compiler produces.

A rule set that fails on an unmatched declaration turns every new header declaration into a reviewed decision. A rule set that fails on a rule that decides nothing (SCS-39) keeps the rule set equal to the headers in the other direction. When a new recorded Slang removes a declaration, the generator names the rule that still mentions it, so the rule set never records a decision that no longer applies.

The rule set is a list of exclusions followed by broad inclusions, so the headers of a Slang release near the recorded one usually generate without a rule change (Section 15, SA-15). That is what makes another Slang version best effort and not impossible.

### 4.3. Exclusions from `slang.h`

This table lists every declaration of `slang.h` that the rule set excludes, with the reason that the rule records. Every other declaration of `slang.h` is included (SCS-1).

| Declaration | Reason |
| --- | --- |
| `slang_getEmbeddedCoreModule` | Exported only under a mangled C++ name (SCS-3) |
| `slang::createGlobalSession`, `slang::shutdown`, and `slang::getLastInternalErrorMessage` | Inline C++ helpers, which are not part of the ABI (SCS-3) |
| `slang::Attribute`, `slang::UserAttribute`, `slang::TypeReflection`, `slang::TypeLayoutReflection`, `slang::Modifier`, `slang::VariableReflection`, `slang::VariableLayoutReflection`, `slang::FunctionReflection`, `slang::GenericReflection`, `slang::EntryPointReflection`, `slang::TypeParameterReflection`, `slang::ShaderReflection`, `slang::DeclReflection`, and the types nested in `slang::TypeReflection`, `slang::Modifier`, and `slang::DeclReflection` | Inline C++ wrappers over the `spReflection*` functions, which are not part of the ABI (SCS-3). This also excludes `slang::VariableReflection::getDefaultValueBlob` |
| `slang::CompileCoreModuleFlag` | An empty struct that only scopes the enumeration `slang::CompileCoreModuleFlag::Enum`, which is included (Section 3.11) |
| `slang::VMExecOperand` | A struct with bit-fields, which a Rust struct cannot express, used only by inline C++ helpers (Section 3.11) |
| Every `static_assert` | A compile-time assertion of the header, which declares nothing |

The rule set also excludes every macro that does not become a constant (Section 5.4), and `spReflection_GetSession` of `slang-deprecated.h`, which is exported only under a mangled C++ name (SCS-3).

### 4.4. Unnamed Enumerations

An unnamed enumeration has no name that a rule can match. The rule set therefore identifies it by the header and one of its enumerators, and assigns it the integer typedef whose values it holds. The generator emits its enumerators as constants of that typedef (SCS-7).

An unnamed enumeration that no rule assigns is a declaration that matches no rule (SCS-2). The generator also fails when the assigned typedef is not an included typedef of an integer type, and when an enumerator does not fit that typedef, because the constant would then have a type that cannot hold its value.

This table lists the rules for the four unnamed enumerations of `slang.h` (Section 3.11):

| Enumerator that identifies the enumeration | Typedef of its constants |
| --- | --- |
| `SLANG_DIAGNOSTIC_FLAG_VERBOSE_PATHS` | `SlangDiagnosticFlags` |
| `SLANG_COMPILE_FLAG_NO_MANGLING` | `SlangCompileFlags` |
| `SLANG_TARGET_FLAG_GENERATE_SPIRV_DIRECTLY` | `SlangTargetFlags` |
| `kSessionFlags_None` | `slang::SessionFlags` |

## 5. Generated Rust

### 5.1. Files and Modules

SCS-51. The build script of `slang-compiler-sys` MUST write the bindings into the file `bindings.rs` and the list of their C functions into the file `symbols.rs`, both directly under `OUT_DIR`, from one call of the generator, on every run (Section 9.4). The crate MUST declare the module `bindings` with the body `include!(concat!(env!("OUT_DIR"), "/bindings.rs"));` and re-export its items at the crate root with `pub use bindings::*;`, and its integration-test binary MUST declare the module `symbols` with the body that includes `symbols.rs` in the same way. No generated file is in the repository or in the published package.

The bindings then always come from the headers of the Slang that the same build links, which is the only arrangement that is sound when Slang reuses vtable slots and renumbers enumerations between releases (Section 1.1). A file in the repository would be a second copy that one Slang version can make wrong for another.

Each generated item then has exactly one public path, at the crate root, as API-10 requires. The glob re-export is the one that API-10 allows a `-sys` crate for its private generated module. A hand-written list of every generated name, or of the C functions, would be a second copy of the generated names that diverges with every Slang version.

The file `crates/slang-compiler-sys/src/lib.rs` holds these items, in this order, and no other code (FFI-1):

1. The crate-level documentation (Section 13).
2. The opt-out `#![expect(unsafe_code, reason = "...")]` (Section 5.10).
3. The module `bindings` with its lint exceptions (Section 5.10).
4. The re-export `pub use bindings::*;`.
5. The hand-written types `BuildInfo` and `Linkage` and the constant `BUILD_INFO` (Section 8.1).

The check of the pointer width (Section 6) is not in `lib.rs`. The generator writes it at the top of `bindings.rs`, so it travels with the declarations whose layouts it protects.

`symbols.rs` holds the constant `pub(super) const C_FUNCTIONS: &[(&str, *const ())]`, which pairs the name of every C function that the bindings declare with its address, in declaration order. The native-symbol test reads it (Section 11).

### 5.2. Type Mapping

SCS-6. The binding generator MUST map each type by its typedef name and never by its canonical type. It maps `SlangInt` and `SlangSSizeT` to `isize`, `SlangUInt`, `SlangSizeT`, and `size_t` to `usize`, each `intN_t` to `iN`, each `uintN_t` to `uN`, and the C types `char`, `int`, and the other basic types to the types of `core::ffi`.

The canonical type differs between targets (Section 3.8). A mapping by typedef name gives every tested target the same Rust types, so code that compiles against the bindings on one target compiles on every other.

These are the other mappings:

- Each integer typedef of the headers becomes a Rust type alias, except `SlangResult`, which becomes the newtype of Section 5.4.
- A C++ reference parameter becomes `*const T` for a `const` reference and `*mut T` otherwise.
- `bool` maps to `bool`.
- A data pointer becomes `*const T` or `*mut T` by its `const` qualifier.
- A callback type becomes `Option<unsafe extern "C" fn(...)>`, because a Rust function pointer cannot be null.

A callback type keeps the non-unwinding ABI `"C"`, unlike the vtable entries and the C functions (Sections 5.6 and 5.7), because Slang calls it. A Rust panic in a callback then aborts the process at the boundary instead of unwinding into the C++ frames of Slang (FFI-7).

These are the forms that FFI-3 requires of generated raw bindings. FFI-3 requires `Option<NonNull<T>>` only in hand-written signatures, because the headers do not say which pointers may be null and the generator must not guess. The raw pointers carry the null case and keep the `const` qualifier, and the safe crate converts each one at the boundary (FFI-6).

### 5.3. Enumerations

SCS-7. The binding generator MUST emit each named C++ enumeration as a `#[repr(transparent)]` newtype over its underlying integer type, with one associated constant for each enumerator. It MUST emit each enumerator of an unnamed enumeration as a free `pub const` item of the integer typedef that a rule of Section 4.4 assigns to the enumeration. It MUST NOT emit a Rust `enum`.

The library can return a value that the header does not list, and such a value in a Rust `enum` is undefined behaviour (FFI-10). A flag constant typed by its flags typedef is passed where the header takes that typedef without a cast.

An enumeration with a fixed underlying type uses that type, mapped by Section 5.2. An enumeration without one becomes a newtype over `u32` when no enumerator is negative, and over `i32` otherwise, because its underlying type differs in signedness between targets but not in size (Section 3.8).

The generator rejects an enumeration without a fixed underlying type whose size is not 4 bytes, and an enumerator that does not fit the underlying type. An enumeration nested in a record gets the name that Section 5.11 derives from its qualified C++ name, and the generator fails on any collision of two generated names.

This illustration shows the form of the constants of an unnamed enumeration, with a placeholder enumerator and value:

```rust
pub const SLANG_COMPILE_FLAG_EXAMPLE: SlangCompileFlags = 8;
```

### 5.4. Constants

SCS-8. The binding generator MUST evaluate macro constants through a synthetic translation unit, only for the macros on an explicit allow-list by name pattern, and MUST exclude the platform and compiler detection macros by rule.

A macro definition cannot be read reliably as text, and a platform macro has a value that depends on the target that the generator parses for, so it would give the bindings of each target a different constant.

The binding generator MUST emit `SlangResult` as a `#[repr(transparent)]` newtype over `i32`, with each result code as an associated constant of that type, because a result code typed as a plain integer can be confused with any other integer, which FFI-10 forbids.

The allow-list is the list of macro rules of the rule set (Section 4.1). It makes these macros constants:

- The result codes `SLANG_OK`, `SLANG_FAIL`, the codes matching `SLANG_E_*`, and the aliases of result codes matching `SLANG_ERROR_*`, each an associated constant of `SlangResult` with the bit pattern that the header defines.
- The facilities matching `SLANG_FACILITY_*`, as free constants of type `i32`, because they are `int` in the header.
- `SLANG_API_VERSION`, as a free constant of type `u32`, the type of the field `SlangGlobalSessionDesc::apiVersion` that takes it.
- The size sentinels `SLANG_UNBOUNDED_SIZE` and `SLANG_UNKNOWN_SIZE`, as free constants of type `usize`.

Every other macro has a rule that excludes it, such as the platform and compiler detection macros, the macros that expand to an attribute or a keyword, and the UUID macros. The generator fails when a value does not fit its type. The three `inline constexpr` constants of `slang.h` become `pub const` items.

`SlangResult` follows FFI-10 like the enumerations of Section 5.3. A `#[repr(transparent)]` newtype has the same ABI as the C typedef of `int32_t`, so every parameter and return value that the header declares as `SlangResult`, in a C function or a vtable entry, uses the newtype. Every `i32` value is a valid `SlangResult`, so an unknown result code causes no undefined behaviour, and COM-7 requires the safe crate to classify every code, including unknown ones, in one function.

### 5.5. Structs, Unions, and Defaults

SCS-9. The binding generator MUST emit each struct as a `#[repr(C)]` struct and each union as a `#[repr(C)]` union, with public fields in declaration order. It MUST emit an `impl Default` that reproduces the C++ default member initializers for each struct that has any, using the evaluated value of each initializer, a null pointer for `nullptr`, and zero for `{}`.

A descriptor whose size field is zero or wrong is rejected or misread by Slang (Section 3.5). A struct without default member initializers gets no `Default`.

A field without an initializer in a struct that has some takes zero in the generated `Default`, which is the value that C++ value-initialization gives it. The exception is a field whose type is a struct with a `Default` of its own, which takes that default. Zero is `false` for `bool`, a null pointer for a pointer, `None` for a callback, and zero in every element of an array.

A floating-point initializer becomes a Rust literal. The generator refuses a value that is not finite, and a `float` value outside the range of `f32`, as a failed evaluation, because Rust has no literal for the first and the second would change its value.

A struct or union may contain anonymous structs and unions, also nested in each other, such as a struct inside an anonymous union. Each one becomes a named `#[repr(C)]` type of its own, with its own layout assertions, and the field that holds it gets a generated name (Section 5.11).

The generator refuses a record with a base class, a record without fields, a bit-field, and a default member initializer in a union (SCS-19). A record without fields has size 1 in C++ but size 0 as a Rust struct.

### 5.6. Interfaces

SCS-10. For each interface `I`, the binding generator MUST emit the items below, because they are the only path from Rust to the methods, and a wrong order, calling convention, or UUID compiles and fails only at run time (ARCH-33, COM-4):

- A `#[repr(C)]` struct `I` whose only field is the vtable pointer.
- A `#[repr(C)]` vtable struct whose first field is the vtable struct of the base interface. That field is followed by one `unsafe extern "system-unwind" fn` pointer for each own method in declaration order, with the interface pointer as the explicit first parameter.
- The UUID of `I`, as an associated constant of a trait that the generator emits and implements for every interface.
- One `unsafe` thunk for each own and inherited method, which calls through the vtable.

The ABI of each entry is the unwinding form `"system-unwind"`, because some methods of Slang let a C++ exception escape (Section 3.3). An exception that enters Rust through a non-unwinding ABI is undefined behaviour, and through an unwinding ABI it is defined (FFI-7).

A thunk is an associated function whose first parameter is the raw interface pointer, `this: *mut Self`, and not a method with a `&self` receiver. A Rust reference to `I` covers only its vtable pointer, while the native object extends beyond it and changes behind the reference, so it would assert an aliasing guarantee that the object does not keep. The raw pointer asserts nothing, which is the sounder choice under Hard Rule 7 of `AGENTS.md`.

The trait is `pub unsafe trait Interface { const UUID: SlangUUID; }`, and each interface implements it with `unsafe impl`. It is an unsafe trait because an implementation promises that the type is an interface struct whose vtable has the layout of the native interface with that UUID. The compiler cannot check that promise, so any other implementation has to state it in an `unsafe impl` of its own.

The interface struct `I` has the one field `vtable: *const IVtbl`, and the vtable struct is named `IVtbl`. The first field of the vtable struct of a derived interface is `base`, of the type of the base interface's vtable struct, and `ISlangUnknownVtbl` has no `base` field. A thunk for an inherited method calls through `base` once for each level of inheritance, with `this.cast()` as its first argument.

A vtable that Rust implements and passes to Slang, such as a custom file system, uses the same generated vtable struct, so its fields have the type `"system-unwind"`.

Its functions are defined `extern "system"`, so that a Rust panic that reaches the boundary aborts the process instead of unwinding into Slang. Each is stored in its field through a conversion of the function pointer to the type of the field, which FFI-7 permits because the caller's ABI is the unwinding form of the callee's. Section 8.4 of `docs/design/slang-compiler.md` applies this rule to the safe crate, and its fake of the Slang interfaces follows it too. The section `# Safety` of the crate documentation states the rule for every user who implements an interface in Rust (SCS-37).

The generator emits no `Send` or `Sync` implementation, and the raw pointer field already makes every interface struct neither (FFI-9). An opaque type of SCS-4 that is an excluded interface is emitted without vtable and thunks.

The generator refuses an interface in these cases (SCS-19):

- It has a data member or a virtual destructor.
- It has a method that has the name of another method of the interface or of a base.
- Its base is not an included interface.
- Its UUID cannot be read as eleven integers.

### 5.7. C Functions and Opaque Handles

SCS-11. The binding generator MUST declare every included C function in an `unsafe extern "C-unwind"` block, with no function marked `safe`, because the compiler trusts these declarations without checking them (FFI-3). It MUST emit each opaque handle and each opaque type of SCS-4 as a zero-sized `#[repr(C)]` struct that code outside the crate cannot construct and that is neither `Send`, `Sync`, nor `Unpin`.

A constructible or movable opaque type lets safe code create a value that no native object stands behind. The block uses the unwinding ABI for the reason that SCS-10 gives for the vtable entries, because a C function of Slang is C++ code as well (FFI-7).

An opaque type has the private field `_data: [u8; 0]` and a private `PhantomData` marker of a raw pointer and `PhantomPinned`, which together give these properties, and it derives `Debug`.

The generator refuses an included function with C++ linkage, or one that the header defines or declares `inline` (SCS-3). It also refuses an excluded type that an included declaration uses by value (SCS-4).

### 5.8. Layout Assertions

SCS-12. The binding generator MUST emit, for every struct and union, compile-time assertions of its size, its alignment, and the offset of each field, with the values that libclang reports for the target of the parse.

A binding with a wrong layout otherwise compiles and corrupts memory at run time, while an assertion stops the build. The values are the same on every tested target (Section 3.8), and an assertion that fails on one names the struct.

### 5.9. Illustration

This illustration shows the shape of the generated code for the result type with two codes and for an interface with two own methods that derives from `ISlangUnknown`. It omits the thunk of `setName` and the thunks of the inherited methods. The interface `IExample`, its methods, and the zero UUID are placeholders, and the generated file holds the trait `Interface` once, before the first interface:

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangResult(pub i32);

impl SlangResult {
    pub const SLANG_OK: Self = Self(0);
    pub const SLANG_FAIL: Self = Self(-2_147_467_259);
}

#[repr(C)]
#[derive(Debug)]
pub struct IExample {
    pub vtable: *const IExampleVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IExampleVtbl {
    pub base: ISlangUnknownVtbl,
    pub getCount: unsafe extern "system-unwind" fn(this: *mut IExample) -> SlangInt,
    pub setName: unsafe extern "system-unwind" fn(this: *mut IExample, name: *const ::core::ffi::c_char) -> SlangResult,
}

unsafe impl Interface for IExample {
    const UUID: SlangUUID = SlangUUID { data1: 0x0000_0000, data2: 0x0000, data3: 0x0000, data4: [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00] };
}

impl IExample {
    pub unsafe fn getCount(this: *mut Self) -> SlangInt {
        unsafe { ((*(*this).vtable).getCount)(this) }
    }
}
```

### 5.10. Lint Exceptions of the Generated Code

SCS-13. The binding generator MUST emit no comment, no item documentation, and no inner attribute, because `include!` rejects an inner documentation comment and the crate states the meaning of the generated items in its own documentation (Section 13). The crate MUST carry the lint exception of the generated code as one `#[expect(..., reason = "...")]` on the module `bindings`.

It lists exactly the lints that fire, as determined by running the checks, except `ffi_unwind_calls`, which a second `#[expect]` on the same module lists with its own reason. A listed lint that does not fire fails the check through `unfulfilled_lint_expectations` (Section 7.9 of the writing guide), so the list is set by running the checks.

The generated files are under `OUT_DIR`, so the custom checkers, `check-fmt`, typos, and the prose checker do not read them, and they need no exemption from any of these. Cargo caps the lints of a dependency from a registry, so an expectation that a user's build of another Slang version leaves unfulfilled does not reach the user.

On the generated code of the recorded Slang, compiled as the module `bindings` of `slang-compiler-sys` with the workspace lint tables and warnings denied, exactly these lints fire (Section 15, SA-11):

- `missing_docs`.
- `non_snake_case`.
- `non_upper_case_globals`.
- `non_camel_case_types`.
- `clippy::exhaustive_structs`.
- `clippy::missing_safety_doc`.
- `clippy::undocumented_unsafe_blocks`.
- `clippy::multiple_unsafe_ops_per_block`.
- `clippy::similar_names`.

The first lint exception on the module `bindings` lists exactly these nine lints. Its reason says three things:

- The generated bindings keep the names of the Slang headers and leave the meaning of each item to Slang's documentation.
- Their structs mirror C++ structs that Slang fills in.
- The crate documentation states the safety contract that every generated function shares.

No expectation of the list is unfulfilled on any of the four tested targets of Section 6. A check of each target with Clippy and warnings denied also proves that every layout assertion holds there. A change of the generator or of the recorded Slang sets the list again by running the checks, as SCS-13 requires.

The second lint exception is `#[expect(ffi_unwind_calls, reason = "...")]`, because every thunk calls a vtable entry declared `"system-unwind"` (SCS-10). Its reason says that the thunks call vtable entries declared with the unwinding ABI, because Slang is C++ and some of its calls let an exception escape, which is undefined behaviour through a non-unwinding ABI. It is a separate attribute so that its reason states this cause, which differs from the reason of the nine lints.

Some lints do not fire and are not listed:

- `clippy::exhaustive_enums` does not fire, because the generated code has no Rust `enum` (SCS-7).
- `missing_debug_implementations` does not fire, because every generated type implements `Debug` (Section 5.11).

The crate root also carries the opt-out `#![expect(unsafe_code, reason = "...")]` that Section 4.4 of the coding standard defines, because the thunks hold `unsafe` blocks. Its reason says that the generated bindings declare foreign functions and call through vtables, which only unsafe code can do.

Original C++ names are kept, which is why the naming lints fire. The thunks are `unsafe fn` items whose bodies are `unsafe` blocks without safety comments, and they have no `# Safety` sections, which is why the three lints of safety documentation fire.

Section 7.9 of the writing guide and UNS-6 exempt generated raw bindings from `# Safety` sections and safety comments on two conditions, which this design meets. The crate-level documentation states one safety contract that every thunk and every C function shares (Section 13). The exemption is part of the lint exception on the module `bindings` (SCS-13).

The module `symbols` of the integration-test binary (Section 5.1) needs no lint exception. It holds one constant, and no lint fires on it with the workspace lint tables and warnings denied on any of the four tested targets, so the module carries no `#[expect]`, because an expectation there would be unfulfilled and fail the check (SCS-13).

The Slang headers misspell three identifiers that the bindings keep, `ISlangClonable`, `ISlangClonableVtbl`, and `SLANG_LANGAUGE_VERSION_DEFAULT`, because each generated name has to match the header (Section 5.11). typos does not read the generated code, so it needs no entry for them. A file of the repository that names one of them needs the narrowest entry that works.

### 5.11. Names, Derives, and Paths

The generator derives each Rust name from the C++ name. These are its rules:

- A namespace is dropped, so `slang::IGlobalSession` becomes `IGlobalSession`.
- A type nested in a record joins the names of its records and its own name with `_`, so `slang::CompileCoreModuleFlag::Enum` becomes `CompileCoreModuleFlag_Enum`.
- The k-th field of a record whose type is an unnamed struct or union, counted from 0, has the type `Parent_anon{k}`, so an anonymous union inside that type is `Parent_anon{k}_anon{j}`. A named field of such a type keeps its name and its default member initializer. The j-th anonymous member, a field without a name, counted from 0, is the field `anon{j}`, so the two numbers differ only when a named field of an unnamed type precedes an anonymous member.
- A name that is a Rust keyword becomes a raw identifier, such as `r#type`, and `crate`, `self`, `Self`, `super`, and `_` get a trailing underscore, because they cannot be raw identifiers.
- An unnamed parameter at position i becomes `arg{i}`, and a parameter or field name that repeats an earlier name in the same list, or a parameter named `this`, gets trailing underscores until it is unique.

Two items that would get the same Rust name are a failure, not a renaming.

The generated types derive these traits:

| Item | Derives |
| --- | --- |
| Enumeration newtypes and `SlangResult` | `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, and `Hash` |
| `SlangUUID`, the UUID type that the rule set names | `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, and `Hash`, because a UUID exists to be compared, such as in the `queryInterface` of an interface that Rust implements |
| Every other struct | `Clone`, `Copy`, and `Debug` |
| Unions | `Clone` and `Copy`, with an implementation of `Debug` that prints the type name only |
| Interface structs, vtable structs, and opaque types | `Debug` |

The generated code has no `use` items. It writes every name that is not in the Rust prelude with its full path, such as `::core::ffi::c_int` and `::core::mem::offset_of!`. The text is not formatted, because no reader opens it in the repository and the compiler does not need it formatted.

## 6. Targets and Platform Independence

SCS-15. The binding generator MUST write, at the top of the generated bindings, a `compile_error!` under `#[cfg(not(target_pointer_width = "64"))]` that says that the bindings support only 64-bit targets, so that `slang-compiler-sys` stops compilation on every target whose pointer width is not 64. On a 32-bit target, one method has another calling convention and every struct that holds a pointer has another layout (Section 3.8), so the bindings would compile and then corrupt memory.

The tested targets are `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, and `x86_64-apple-darwin`, which the constant `TESTED_TARGETS` of `route.rs` lists. CI links the bindings against the Slang of the SDK and runs the tests that call it on the first three (Section 12).

x86_64 macOS is tested only by a type check. Clippy with warnings denied for `x86_64-apple-darwin` generates the bindings for that target and type-checks them and their layout assertions, but nothing links or runs them there (Section 15, SA-12).

Every other 64-bit target is allowed, untested, and unsupported (ARCH-37 of the architecture design). The build script states this in a warning and in the build information and does not stop the build for it (SCS-65). A stop remains only where a correct build is impossible: a target whose pointer width is not 64 (SCS-66), and on the vendored route a cross-compilation or a target without a link rule for its C++ runtime library (SCS-43).

SCS-65. On a target that is not one of the tested targets, the build script MUST emit one `cargo::warning` that names the target and says that it is untested and unsupported, and MUST set the field `tested_target` of the build information to `false`. It MUST NOT stop the build because the target is untested, on either route.

The warning reaches the user when a build on that target fails, which is when the statement matters (Section 9.2). The field lets a program report the same statement at run time, and the crate documentation lists the tested targets (Section 13).

SCS-66. On both routes, the build script MUST stop the build with the message of Section 9.6 when `CARGO_CFG_TARGET_POINTER_WIDTH` is not `64`, before it generates the bindings. The stop is a matter of correctness and not of support, for the reason of SCS-15, and a stop in the build script names the cause before the generator reports a refused construct of the 32-bit parse. The `compile_error!` of SCS-15 stays as the guard of the generated code itself.

32-bit targets are a non-goal that the owner can revisit.

The bindings are generated in every build, for the target of that build (Section 9.4). Everything that depends on the target is expressed in the Rust source by construction, so the generated text is the same on every tested target:

- The calling convention, through `extern "system-unwind"` and `extern "C-unwind"`, which name the C calling convention on every 64-bit target.
- The integer typedefs, through `isize` and `usize` and the mapping by typedef name (SCS-6).
- Where nothing else works, `#[cfg]`, which the generator emits only under a rule that names the case.

The headers of the recorded Slang need no `#[cfg]` other than the one on the `compile_error!` of SCS-15 (Section 3.8).

This is the check that the generator writes at the top of the generated bindings:

```rust
#[cfg(not(target_pointer_width = "64"))]
compile_error!("the Slang bindings support only 64-bit targets");
```

The reason for the restriction is cost against benefit. Support for 32-bit targets would need a second set of layout assertions, a per-method calling convention, and a test platform that no CI job provides, for no user that the project knows of.

## 7. `slang-bindgen`

### 7.1. Crate and Dependency

SCS-57. The binding generator MUST be the published library crate `slang-bindgen` in `crates/slang-bindgen`, with a thin binary, and MUST use the crate `clang` with the features `runtime` and `clang_10_0`, the crate `clang-sys` of the same version and features that `clang` uses, and no unsafe code of its own. With `runtime`, the generator loads libclang when it runs and not when it is compiled, so every crate that compiles it builds without libclang, and the error of a missing libclang comes from the generator with advice (Section 7.7). A generator without unsafe code needs no exception to Hard Rule 7 of `AGENTS.md`.

`slang-compiler-sys` is published and runs the generator in its build script, so the generator has to be published with a version, and its shim headers have to be part of its package. The generator finds its shim headers at run time through the directory of its package, which it takes from `env!("CARGO_MANIFEST_DIR")` when it is compiled, and which holds the unpacked package of a registry crate.

The crate `clang` 2.1.0 exposes everything the generator needs through its safe API. It wraps only the libclang API up to clang 10, which is enough. It finds libclang through `LIBCLANG_PATH` and then the standard locations of each platform.

Only one instance of its `Clang` type may exist in a process, so the generator creates one instance for each call of `generate`. The generator calls libclang only through `clang`, because the parse functions of `clang-sys`, used directly, would need unsafe code.

SCS-67. The first call of `generate` that loads libclang MUST keep it loaded until the process exits, through a handle of `clang-sys` that the generator holds in a static, on every platform. `clang` unloads libclang when its `Clang` value is dropped, and on Windows a process that unloaded `libclang.dll` after using it crashes with an access violation when it exits, after the build script has printed every instruction. The CI job `test` on Windows shows that a build script that keeps libclang loaded exits without an error. The generator loads its handle with the safe API of `clang-sys`, so it adds no unsafe code.

`generate` may be called again, from the same thread or from another one, once an earlier call has returned. A call while another call runs still fails with an error of kind `Libclang` (Section 7.7). A build script calls `generate` once.

The generator's binary is an application, so ERR-1 and ERR-2 do not apply to it (Section 1.1 of the coding standard). It opts out of `clippy::print_stderr` at its binary root as Section 4.4 of the coding standard describes. The library follows ERR-1 and ERR-2.

### 7.2. Parsing

SCS-58. The binding generator MUST parse `slang.h` at `-std=c++17` once, for the target that its caller names, with `-nostdinc` and the shim headers of its crate, and MUST NOT keep a list of accepted targets. It MUST fail when the parse reports a diagnostic of error severity or above, because libclang returns a translation unit even after a fatal diagnostic, and the model of that translation unit is silently wrong (Section 3.8).

One parse is all that a build needs, because the bindings are generated for the target of the build. Whether a target is tested is a matter of support, which the build script states (SCS-65), and not of correctness. What the generator cannot generate correctly for a target it refuses through its strict rule set (SCS-19), and the layout assertions of SCS-12 compare every struct with what libclang reports for that target.

The shim headers, under `crates/slang-bindgen/shim/`, provide `stdint.h`, `stddef.h`, `inttypes.h`, `features.h`, and `TargetConditionals.h` on top of the compiler's predefined macros. The parse uses them on every target, so that no parse depends on the system headers of the machine that runs the generator. The parse passes libclang the arguments `-x c++ -std=c++17 -target <target> -nostdinc -isystem <shim directory> -I<header directory>`.

The generator evaluates macro constants (Section 5.4) and default member initializers (Section 5.5) through one synthetic translation unit. That unit includes `slang.h` and declares one `constexpr auto` variable for each value to evaluate.

An initializer of `nullptr` or `{}` is not evaluated and maps as Section 5.5 states. Any other initializer that libclang cannot evaluate is a failure.

The generator does not read `slang-tag-version.h` and checks no version. The build script reads the build tag itself (Section 9.2), and the generator generates whatever headers it is given, within its rule set (SCS-2).

### 7.3. Refused Constructs

SCS-19. The binding generator MUST fail, for the target of the parse, on every included declaration that uses a construct that the generated form of Section 5 cannot express, and on a declaration that matches no rule of Section 4.1.

Each such construct has an ABI or a layout that the generated Rust would get wrong without any error. On the Microsoft ABI, for example, a member function returns an aggregate through a hidden pointer, so a vtable entry written as a plain return would read garbage.

These are the constructs that SCS-19 refuses. Section 7.5 describes the tests.

- A parameter or return value that is an aggregate passed by value, an interface or an opaque type used by value, or an array parameter.
- A variadic function.
- A method, a function, or a callback type whose calling convention, as libclang reports it for the target, is not the C calling convention.
- An interface with more than one base, a base that is not an included interface, a data member, or a virtual destructor.
- A method that is not pure virtual, and a method that has the name of another method of the interface or of a base.
- A class with virtual methods and no UUID, an interface whose UUID the generator cannot read, and an interface when the bindings lack the struct `SlangUUID` with four fields.
- A record with a base class, a record without fields, a bit-field, a default member initializer in a union, a `nullptr` initializer on a field that is not a pointer, and an anonymous record outside a record.
- An enumeration without a fixed underlying type whose size is not 4 bytes, an enumeration without a fixed underlying type that has an enumerator outside the range of `int`, and an enumeration whose underlying type is not an integer type.
- An included function with C++ linkage, or one that the header defines or declares `inline`.
- An excluded type that an included declaration uses by value.
- A type that the generator does not map, such as `void` outside a pointer, and a name that is not a type or that the headers do not declare.
- A typedef that the rule set maps to `isize` or `usize` but that is not an integer type.
- A pointer or a callback with a value other than `nullptr`, an integer value on a type that is not an integer type, and a floating-point value on an integer type.
- A result code macro when the bindings lack the typedef `SlangResult`, and a `SlangResult` that is not a typedef of a 32-bit signed integer.
- A variable that is not a compile-time constant, and a declaration of any other kind that a rule includes.

The refusal of an enumerator outside the range of `int` keeps the Rust type of each enumeration the same on every target, which a newtype over `u32` on one target and over `i32` on another would break (Section 3.11).

The generator reports each one with the declaration, its location, and the reason (Section 7.7). It also fails on values that it cannot evaluate or that do not fit their type (Sections 5.4 and 5.5), and on two items with one Rust name (Section 5.11).

None of these cases occurs among the included declarations of the recorded Slang on any tested target. The method `slang::IMetadata::isParameterLocationUsed` lacks `SLANG_MCALL`, but libclang reports the C calling convention for it on every tested target, as for every other method, so SCS-19 accepts it.

### 7.4. Library and Binary

SCS-59. The library of `slang-bindgen` MUST return the two generated texts to its caller and MUST NOT write files, start a process, or format the text. It MUST NOT write any comment or inner attribute into either text (SCS-13). A build script that runs a formatter would add a process, a time limit, and a failure mode to every user's build for text that nobody reads, and an inner documentation comment breaks `include!`.

The library's public API is:

- The function `generate`, which takes the header directory and the target triple and returns `Result<Generated>`.
- The struct `Generated`, which is `#[non_exhaustive]` and holds the two generated texts in the public fields `bindings` and `symbols`, both from the same parse.
- The types `Error`, `ErrorKind`, and `Location` with the alias `Result` (Section 7.7).

The library has these private modules:

- `parse` turns one parse of libclang into a model.
- `rules` holds the rule set.
- `check` selects the declarations by the rules and validates the selection against every refusal of SCS-19.
- `emit` writes the Rust text.
- `model` defines the model.
- `error` defines the error type.

SCS-60. The binary of `slang-bindgen` MUST take a header directory, a target triple, and an output directory, call `generate`, and write the two texts as `bindings.rs` and `symbols.rs` into the output directory, creating it when it is absent. It MUST call the same library function with the same inputs as the build script of `slang-compiler-sys`, so that a file that it writes equals the code that a build of the same headers and target compiles.

The binary exists for the checker `check-slang-call-guard`, which reads the names of Slang's C functions and thunks from the generated bindings (Section 6.9 of the tooling design) and has no file in the repository to read them from any more. The recipe `check-slang-call-guard` runs the binary on `$VULKAN_SDK/include/slang` for the target of the host, into `target/slang-bindgen/`, and passes `target/slang-bindgen/bindings.rs` to the checker. The checker then reads the bindings of the Slang that CI and the development container install, which is the recorded Slang, and its result depends only on those headers.

This illustration shows the command that the recipe runs in the development container, with the path of the SDK as a placeholder:

```sh
cargo run --quiet -p slang-bindgen -- /tmp/vulkan-sdk/x86_64/include/slang x86_64-unknown-linux-gnu target/slang-bindgen
```

### 7.5. Tests

The tests in the generator's `ffi` modules parse small synthetic headers with libclang and check the generated text. They cover:

- The order of vtable slots under single and multi-level inheritance, and the extraction of UUIDs.
- The mapping of types, references, callbacks, and enumerations, with the ABI strings of SCS-10, SCS-11, and Section 5.2, and the constants of unnamed enumerations.
- The evaluation of macros and default member initializers, including floating-point values.
- Opaque types, layout assertions, unions, and nested anonymous records.
- The same text for each of the four tested targets, a parse for another 64-bit target, a parse that follows its target, and determinism.
- A target that libclang refuses, a missing header directory, and a missing main header.
- Every refusal of SCS-19, and the failures on a declaration or macro that matches no rule and on a stale rule (SCS-39).
- The generated list of C functions of Section 5.1, and the absence of any comment or inner attribute in either text.
- The error when a process already holds libclang.
- Three generations in one process, twice on one thread and once on another, which give the same text (SCS-67). No test on Linux can show the crash at the end of a process on Windows.

The integration-test binary `tests/generator/` runs the binary of SCS-60 in its module `ffi`. Its tests cover:

- The binary on `$VULKAN_SDK/include/slang`, writing both files into a new nested directory with the texts that `generate` returns for the same headers and target. This test needs `VULKAN_SDK`.
- An unreadable header directory, reported with its source error and with no output directory written.
- A libclang that cannot be loaded, with `LIBCLANG_PATH` set to a missing directory, reported with the advice of Section 7.7.
- A target triple that is not valid UTF-8, and a wrong number of arguments, each with the usage text and the exit status 2.
- An output directory that the binary cannot create, and a `bindings.rs` that it cannot write, after which it writes no `symbols.rs`.

The success case runs on the real headers, because the binary uses the strict rule set of the recorded Slang, which refuses a synthetic header with errors of kind `StaleRule` and `UnmatchedDeclaration`. The tests of the libclang failure and of the output paths need the generation to get past the headers, so in practice they need `VULKAN_SDK` as well.

The other tests are pure Rust and also run under Miri. They cover the matching of rule patterns, the lookups in the rule set, the value checks of the validator, the formatting of literals, the escaping of names, and the error type.

SCS-21. Every test of the generator that loads libclang MUST sit in a module named `ffi` and MUST be compiled only for Linux targets. Miri cannot call libclang (Section 11), and the text that the tests check does not depend on the host, because every parse uses the shim headers and names its target. The build of `slang-compiler-sys` on each CI platform runs the generator on the real headers there (Section 12).

The `ffi` modules are compiled under `#[cfg(target_os = "linux")]`, so they do not run on Windows or macOS at all, and they never report a skipped run as a pass. On Linux, a test that finds no libclang fails.

Only one `Clang` instance may exist per process (Section 7.1), so each test that loads libclang holds a static mutex for its whole run. nextest runs each test in its own process. A run through `cargo test`, which runs the tests of one binary on several threads in one process, also works.

### 7.6. Why No Regeneration Test

This subsection is non-normative. No test compares generated bindings with a file, because no generated file exists outside `OUT_DIR`. The build itself gives the checks that such a comparison would give:

- Every build of `slang-compiler-sys` generates the bindings, so a defect of the generator that makes the text fail to compile fails the build, the checks, and CI on every platform.
- The layout assertions (SCS-12) compare every struct with what libclang reports for the target.
- The native-symbol test references every C function of the generated list (Section 11), and the smoke test calls Slang through a vtable.
- `slang-compiler` and its tests compile against the generated items, so a renamed or missing item fails them.

A comparison would also give a reviewed diff of the bindings when the recorded Slang moves. A reviewer gets that diff by running the binary of SCS-60 on the old and new headers and comparing the outputs, as Section 8.2 describes.

### 7.7. Error Reporting

`generate` returns one error type, `Error`. It holds a message, an optional location in the headers, and an optional source error.

The location is the file name without its directory and the line. Its method `kind` returns an `ErrorKind`, which is `#[non_exhaustive]` and tells the failures apart without the message:

| Kind | Failure |
| --- | --- |
| `Io` | The header directory, the main header `slang.h` in it, or the shim directory cannot be read |
| `Libclang` | libclang cannot be loaded, or the process already holds a libclang instance |
| `Target` | libclang loaded but refuses to parse `slang.h` for the target |
| `Diagnostic` | The parse reports a diagnostic of error severity or above (SCS-58) |
| `UnmatchedDeclaration` | Declarations or macros match no rule (SCS-2) |
| `StaleRule` | Rules decide nothing (SCS-39) |
| `Unsupported` | An included declaration uses a construct of SCS-19, or a type that libclang does not report |
| `Evaluation` | A macro or a default member initializer cannot be evaluated or does not fit its type |
| `NameCollision` | Two generated items would have the same Rust name (Section 5.11) |

The errors of `UnmatchedDeclaration` and `StaleRule` list every unmatched declaration and every stale rule at once, so that one run shows the whole change to the rule set.

The generator passes the Rust target triple to libclang unchanged and translates no triple. libclang 19 accepts the four tested targets and `aarch64-unknown-linux-gnu` as given, and it refuses some Rust triples, such as `riscv64gc-unknown-linux-gnu`, while it accepts `riscv64-unknown-linux-gnu`. A refused target is an error of kind `Target`, whose message is "libclang cannot parse slang.h for target", the triple, and "which it does when it does not accept the target triple", with the error of libclang as its source.

libclang refuses to parse a missing main header in the same way as a refused target. The generator therefore reads `slang.h` before it starts libclang and reports a missing or unreadable main header as an error of kind `Io`, so that a missing header is never reported as a refused target.

When libclang cannot start, the `Display` text of the error is a phrase that says which of two cases applies, and holds no advice, as ERR-4 requires. Each layer above it states its advice once:

- libclang cannot be loaded. The text is "cannot load libclang:" and the reason that `clang-sys` gives. The binary adds the advice to install libclang or set `LIBCLANG_PATH` to the directory that holds it, and the build script of `slang-compiler-sys` adds its own sentence (Section 9.6).
- The process already holds a libclang instance, such as during a call of the generator on another thread. The text is "cannot use libclang, because this process already holds a libclang instance". The section `# Errors` of `generate` states that only one instance may exist at a time and that a later call works once the earlier one has returned, so a caller calls the generator from one thread at a time (SCS-67). The binary loads libclang once on one thread, so it never meets this case.

The binary prints the error and each source error, one per line, to standard error and exits with a failure status. For an error of kind `Libclang`, it then prints one line of advice: "slang-bindgen generates the bindings with libclang, so install libclang or set LIBCLANG_PATH to the directory that holds it".

Whether the two cases keep one kind is open (SQ-7 of Section 16).

## 8. Build Information

### 8.1. Build Information Constant

SCS-53. `slang-compiler-sys` MUST expose the constant `BUILD_INFO` of the type `BuildInfo`, whose values the build script writes into the file `build_info.rs` under `OUT_DIR` and the crate includes, and MUST NOT derive any of them from `cfg(feature = "vendored")` in the crate. The values describe the Slang that the build generated its bindings from and links, which only the build script knows, and the safe crate compares the Slang loaded at run time with them (Section 6.3 of `docs/design/slang-compiler.md`).

`BuildInfo` is a `#[non_exhaustive]` struct with these public fields, and `Linkage` is a `#[non_exhaustive]` enumeration with the variants `Dynamic` and `Static`. Both derive `Clone`, `Copy`, `Debug`, `PartialEq`, `Eq`, and `Hash`, and `Linkage` implements `Display` as `dynamic` or `static`, the values of `DEP_SLANG_COMPILER_LINKAGE` (API-7). Both are `Send`, `Sync`, and `'static`, which a test asserts (CONC-1). The shape of the fields is open (SQ-6 of Section 16).

| Field | Type | Value |
| --- | --- | --- |
| `slang_build_tag` | `&'static str` | The build tag of the Slang of the build: `SLANG_TAG_VERSION` of the SDK on the dynamic route, and the build tag of `slang-src` on the vendored route |
| `slang_commit` | `Option<&'static str>` | The full commit of the source tree on the vendored route, and `None` on the dynamic route, where the SDK records no full commit |
| `linkage` | `Linkage` | `Dynamic` on the dynamic route and `Static` on the vendored route |
| `tested_target` | `bool` | `true` when the target of the build is one of the tested targets of Section 6, and `false` otherwise (SCS-65) |
| `version` | `&'static str` | The full version of the build, in the form below |

`version` is a Semantic Versioning 2.0.0 version made of the version of `slang-compiler-sys`, a plus sign, and build metadata that names the Slang and the route: `slang`, the Slang identifiers of the build tag, and `dynamic` or `vendored`. The Slang identifiers are the numbers that SRC-13 of the `slang-src` design derives from a build tag, with a missing patch or count written as `0`, followed by `g` and the hash that the build tag carries.

A build tag without a count and a hash, such as `2026.17`, gives the identifiers `2026.17.0.0` with no hash identifier, as in `0.1.0+slang.2026.17.0.0.dynamic`. SRC-13 takes the hash from the recorded commit in that case, but the dynamic route has no commit to take it from.

These are the values for the recorded Slang on each route, with `0.1.0` as an illustration of the crate version:

```text
0.1.0+slang.2026.17.0.25.gc7954ebd1.dynamic
0.1.0+slang.2026.17.0.25.gc7954ebd1.vendored
```

A build tag of an installed SDK that does not have the form of SRC-12 of the `slang-src` design, including one whose hash has fewer than nine digits, such as `2026.17-25-gc7954eb`, gives the build metadata `slang`, the build tag with every character other than an ASCII letter, a digit, or a hyphen replaced by a hyphen, and the route. The build tag itself is in `slang_build_tag` unchanged.

`version` lets a program print one string that says which Slang it uses and how, and the build metadata, which Cargo ignores, is the place in a SemVer version for that information. The word `vendored` in it names the Cargo feature that a user enables to get the route, while `Linkage` names how Slang is linked.

The documentation of these hand-written public items follows Section 7 of the writing guide in full. They are, with the generated items, the whole public API of the crate (FFI-1).

### 8.2. Moving the Recorded Slang

ARCH-30 of the architecture design governs a move of the recorded Slang. The agent that moves it takes these steps in one change:

1. Run the binary of SCS-60 on the headers of the current SDK and keep the output, for the diff of step 6.
2. Change the Vulkan SDK version and its checksums in the CI configuration (Section 7.3 of the tooling design).
3. Change `RECORDED_COMMIT` and `RECORDED_BUILD_TAG` in `crates/slang-src/recorded.rs`, and the manifest version of `slang-src` as SRC-13 of the `slang-src` design defines, together with the exact requirement of every crate that depends on it.
4. Install the new SDK in the development container, or wait for the image that provides it.
5. Run every check, and fix what the new headers break: a declaration that no rule covers (SCS-2), a rule whose declaration the headers no longer have (SCS-39), a construct that Section 5 cannot express (SCS-19), a lint expectation of SCS-13, and the safe crates.
6. Run the binary of SCS-60 on the new headers and review the diff against the output of step 1.
7. Run the vendored CI job on every platform (SCS-62).

## 9. Build Script

### 9.1. Route Selection

SCS-50. `slang-compiler-sys` MUST have exactly one Cargo feature, `vendored`, which is off by default and enables the optional dependency `slang-src`. The build script MUST take the vendored route of Section 9.3 when the feature is on, which it reads from `CARGO_FEATURE_VENDORED`, and the dynamic route of Section 9.2 otherwise. It MUST NOT select a route by any other input, such as an environment variable, because every additional input is another way to link a Slang that nobody chose.

The owner decided that static linking is the opt-in feature `vendored`, so that a default build stays fast and small and a vendored build is visible in the manifest of whoever asks for it. Cargo unifies features, so a crate in the user's graph that enables `vendored` makes the whole build vendored, which matches what linking means: one build graph links one Slang, as `links = "slang-compiler"` requires.

The feature is not additive in the strict sense of DEP-1, because it replaces the dynamic link with a static one. It never removes an item of the API, so code that compiles without it compiles with it, which is the property that unification needs.

Every crate between a user and `slang-compiler-sys` forwards the feature under the same name, so a user enables it on the crate they depend on. `--all-features` enables it, so the recipes of this repository pass an explicit list of features instead (Section 3.2 of the tooling design).

The build script never falls back from one route to the other. A user who enabled `vendored` and silently got a dynamic link would ship a program that needs the shared library at run time, and a user without the feature who silently got a vendored build would pay for a long native build and a network clone.

This illustration shows a dependency of a user's crate on `slang-compiler` with the feature:

```toml
[dependencies]
slang-compiler = { version = "0.1", features = ["vendored"] }
```

### 9.2. Dynamic Route

SCS-52. On the dynamic route, the build script MUST read `$VULKAN_SDK/include/slang/slang-tag-version.h`, extract the string that `SLANG_TAG_VERSION` defines, and record it as the build tag of the build information (SCS-53). It MUST stop the build with the message of Section 9.6 when the file cannot be read or when the definition is missing or malformed. It MUST NOT compare the build tag with any recorded version.

The bindings are generated from the headers of the same SDK, so the declarations match the library whatever its version, and a gate on one version would refuse a Slang that the build can serve. The run-time check of the safe crate compares the library that the program loads with this record, which catches a library that the loader found somewhere other than the SDK of the build (Section 6.3 of `docs/design/slang-compiler.md`). Another Slang version than the recorded one is best effort, as the crate documentation states (Section 5.3 of the architecture design).

The build script reads the header with these rules. It reads every line whose first two words are `#define` and `SLANG_TAG_VERSION`, and takes the value of the last such line. It requires the rest of such a line to be exactly one non-empty string literal that holds no quotation mark and no control character, and it accepts lines that end in CRLF. An empty literal is malformed, so `slang_build_tag` is never empty and `version` never gets an empty identifier.

A missing definition and a malformed one are separate failures of Section 9.6, and the message of a malformed one names its line.

SCS-40. On the dynamic route, the build script MUST stop the build with the message of Section 9.6 when `VULKAN_SDK` is absent or empty. It MUST also stop the build when the value is not valid UTF-8, contains a line feed or a carriage return, or is not an absolute path. It MUST NOT read any file or emit any instruction that depends on such a value.

A line break in the value would end a Cargo instruction early and let the rest of the value inject an instruction of its own. A value that is not UTF-8 cannot be written into an instruction without being altered.

A relative path would let the headers that the generator reads and the library that the linker links resolve against different directories.

SCS-26. On the dynamic route, once the checks of SCS-40, SCS-41, and SCS-52 pass and the bindings are generated, the build script MUST emit `cargo::rustc-link-search=native=$VULKAN_SDK/lib` and `cargo::rustc-link-lib=dylib=slang-compiler`. It MUST NOT use pkg-config, emit an rpath, or search any other location, because a single mechanism behaves the same on every platform, and finding the library at run time is the job of the environment (Section 10).

SCS-41. On the dynamic route, the build script MUST stop the build with the message of Section 9.6 when `$VULKAN_SDK/lib` cannot be accessed or is not a directory. When the operating system of the target is the operating system of the host, it MUST also stop the build when `$VULKAN_SDK/lib` does not hold the library file of the target as a file.

`CARGO_CFG_TARGET_OS` gives the operating system of the target (SYS-3). The library file is `libslang-compiler.so` for Linux, `libslang-compiler.dylib` for macOS, and the import library `slang-compiler.lib` for Windows.

The build script MUST NOT look for the library file of a target whose operating system differs from the host's, or of a target for which this list names no file.

Without the check of the library file, `cargo check` passes and the failure comes later from the linker with a message that does not name the SDK. The SDK of the host holds only the library of the host, so a check of a foreign target would fail every `cargo check` and Clippy run for that target from Linux. Those runs verify the layout assertions of SCS-12 on the other tested targets (Section 5.10).

The build script asks the file system what a path is through `std::fs::metadata`, which follows symbolic links. `libslang-compiler.so`, which is a symbolic link in the SDK (Section 3.7), therefore counts as a file. When the file system reports an error, the message gives the reason that the operating system gives, such as "Permission denied (os error 13)".

For a foreign target, the build script checks only that `lib` is an accessible directory and then emits the link instructions, so a build for that target that links fails in the linker. The crate documentation states this rule (Section 13).

The dynamic route takes these steps, in this order:

1. Validate `VULKAN_SDK` (SCS-40).
2. Read the build tag (SCS-52).
3. Check the library directory and the library file (SCS-41).
4. Read `TARGET`, and check the pointer width of the target (SCS-66).
5. Emit the warning of SCS-65 for an untested target.
6. Read `OUT_DIR`, and generate the bindings from `$VULKAN_SDK/include/slang` (Section 9.4).
7. Write `bindings.rs`, `symbols.rs`, and `build_info.rs` (SCS-53), in that order.
8. Emit the link instructions and the `links` metadata.

On both routes, the warning comes as soon as the target and its pointer width are known, and before every step that can fail for the platform, such as the generation and, on the vendored route, the native build. Cargo shows the warnings of a registry dependency's build script only when the build fails, so the warning cannot serve as general information. It does reach the user exactly when a build on an untested target fails, and only if the build script emitted it before the step that failed.

When every step passes, the build script emits these instructions, in this order, with the rerun instructions of Section 9.5 and, on an untested target, the warning of SCS-65 before them:

```text
cargo::rustc-link-search=native=<VULKAN_SDK>/lib
cargo::rustc-link-lib=dylib=slang-compiler
cargo::metadata=include=<VULKAN_SDK>/include/slang
cargo::metadata=lib=<VULKAN_SDK>/lib
cargo::metadata=linkage=dynamic
cargo::metadata=build_tag=<build tag>
```

In this listing, `<VULKAN_SDK>` stands for the value of the variable and `<build tag>` for the value of SCS-52. A test of Section 9.7 asserts this exact list.

pkg-config is not used, because the `.pc` file carries only the coarse version (Section 3.6) and Windows has no pkg-config. The build script expects the headers under `include/slang` and the library under `lib` on every platform, with the import library under `lib` on Windows, which is the layout of Section 3.7.

On Windows, the linker finds `Lib\slang-compiler.lib` through the lower-case path `lib` on the case-insensitive file system. The program then finds the DLL through `PATH` (Section 10).

### 9.3. Vendored Route

The function `vendored_route` of `route.rs` implements the vendored route (Section 9.7). The route never reads `VULKAN_SDK`, and it never falls back to the dynamic route (SCS-50). It writes only under `OUT_DIR/slang`, the generated files under `OUT_DIR`, and its entries in the shared folder (SCS-56). `OUT_DIR/slang` holds these paths:

| Path under `OUT_DIR` | Content |
| --- | --- |
| `slang/source` | The copy of the tracked files of the source tree that the build compiles |
| `slang/build` | The CMake build tree |
| `slang/build/Release/include` | The headers that the build places in its build tree, including the generated `slang-tag-version.h` |
| `slang/lib` | The six static libraries |
| `slang/build.log` | The output of every `cmake` run of the current run of the build script |
| `slang/stamp` | The stamp of the last verified build |

SCS-55. On the vendored route, the build script MUST read the source tree, its commit, and its build tag only from `DEP_SLANG_SRC_ROOT`, `DEP_SLANG_SRC_COMMIT`, and `DEP_SLANG_SRC_BUILD_TAG`, and the shared folder only from `DEP_SLANG_SRC_SHARED_DIR`. It MUST stop the build when one of the four is absent, when a path is not an absolute path in UTF-8 without line breaks, when the commit is not 40 lowercase hexadecimal digits, or when the build tag does not have the form of SRC-12 of the `slang-src` design. It MUST apply the same path checks to `OUT_DIR` and stop the build when `TARGET` or `HOST` is absent.

`slang-src` is the one place that obtains the tree and finds the shared folder (Section 4 of the `slang-src` design), so a second source in this build script could build another tree than the one that `slang-glslang-sys` builds. The checks guard the paths that the build script writes into Cargo instructions, and a missing variable would make the comparison of SCS-43 meaningless.

The build script takes these steps, in this order. A step that fails returns the error of Section 9.6 for it, and no later step runs.

1. Read and check the metadata of `slang-src`, `OUT_DIR`, `TARGET`, and `HOST` (SCS-55).
2. Check the target. A target that differs from the host stops the build, and so does a target without a link rule for its C++ runtime library (SCS-43). A target whose pointer width is not 64 stops it as well (SCS-66).
3. Emit the warning of SCS-65 for an untested target, at the place in the order that Section 9.2 gives, before the native build and the generation.
4. Probe Ninja. The build script runs `ninja --version` and stops the build when Ninja cannot start or fails (SCS-28).
5. Assemble the arguments of the configure step (SCS-28).
6. Find the C++ compiler. With the Microsoft toolchain, the build script looks up `cl.exe` and its environment and stops the build when it finds none (SCS-68). On every other target, it runs the compiler probe of SCS-56.
7. Compute the entry key, which runs `cmake --version`, lock the entry in the shared folder, and emit the rerun instruction for its stamp file (SCS-56). When the entry is complete, skip to step 16 with the directories of the entry.
8. Truncate the log `slang/build.log` to empty (SCS-48).
9. Read the stamp. When `slang/stamp` holds exactly the stamp of this run, the build script skips steps 10 to 12 (SCS-46).
10. Remove the old stamp and `slang/build/CMakeCache.txt`, then list the files to copy with `git ls-files -z --stage`, whose entries with the mode `160000` are the gitlinks of the source tree. For each of the eight submodules of Section 3.10, the build script lists the tracked files of the submodule with the same command. A tracked path that is not made only of normal components, such as one with `..`, stops the build (SCS-45).
11. Copy the listed files into `slang/source`. The build script removes every file of the copy that is not listed, and it writes a listed file only when its bytes differ from the copy (SCS-45).
12. Configure the copy. The build script runs `cmake -S <copy> -B <build> -G Ninja` with the arguments of step 5. It removes `SLANG_USE_SCCACHE` from the environment of `cmake`, adds the environment of the toolchain of step 6, and appends the output to the log.
13. Build. The build script runs `cmake --build <build> --target slang`, followed by `--parallel` and the value of `NUM_JOBS` when that value is a whole number, with the same environment as step 12, and appends the output to the log. It runs this step on every run that reaches it, also when the stamp matched, and Ninja rebuilds only what changed.
14. Verify the result (SCS-47).
15. Write the stamp, when step 10 ran (SCS-46). Store the six libraries and the include directory of the build tree in the locked entry (SCS-56).
16. Generate the bindings from the include directory of the entry (Section 9.4).
17. Write `build_info.rs` with the build tag and the commit of `slang-src` and the linkage `Static` (SCS-53).
18. Emit the link instructions and the `links` metadata.

The build script removes `CMakeCache.txt` whenever the stamp differs, because CMake keeps the compiler of its first configure step in that cache and ignores a changed `CXX` or toolchain. Without the removal, a build with another compiler would store libraries of the old compiler under the key of the new one.

The arguments of the configure step are `-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=<lib>`, `-DSLANG_VERSION_FULL=<build tag>`, `-DSLANG_VERSION_NUMERIC=<numeric version>`, and `-DFETCHCONTENT_FULLY_DISCONNECTED=ON`, then the 27 options of the constant `SLANG_OPTIONS` of `route.rs`, then the options of the target. The build tag is the value of `DEP_SLANG_SRC_BUILD_TAG`, and the numeric version is its part before the first hyphen.

The 27 options of `SLANG_OPTIONS` select a release build, `SLANG_LIB_TYPE=STATIC`, the embedded core module and its source, and `SLANG_SLANG_LLVM_FLAVOR=DISABLE`. They turn off these components:

- DirectX Intermediate Language (DXIL), the tests, and the examples.
- gfx, slang-rhi, the replayer, slangd, slangc, slangi, slang-rt, slang-glslang, and the Slang proxy.
- CUDA, OptiX, NVAPI, Xlib, and Aftermath.
- The debug information of the release build and its split form, and sccache.

They also exclude Dawn and Tint. With the Microsoft toolchain, the build script adds `-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded` when `CARGO_CFG_TARGET_FEATURE` holds `crt-static`, and `-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL` otherwise, and it adds `-DSLANG_ENABLE_SPIRV_TOOLS_MIMALLOC=OFF`, so that no build needs the submodule `external/mimalloc`, which `slang-src` does not require (Section 3.4 of the `slang-src` design).

When every step passes, the build script emits these instructions, in this order, after the rerun instructions of Section 9.5. The rerun instruction for the stamp file of the entry comes after the warning of SCS-65 on an untested target, because the build script knows the entry only once it has computed the key in step 7. In this listing, `<lib>` and `<include>` stand for the directories `lib` and `include` of the entry in the shared folder, and the link instructions after `lz4` depend on the target. This listing is the one for Linux:

```text
cargo::rustc-link-search=native=<lib>
cargo::rustc-link-lib=static=slang-compiler
cargo::rustc-link-lib=static=compiler-core
cargo::rustc-link-lib=static=core
cargo::rustc-link-lib=static=cmark-gfm
cargo::rustc-link-lib=static=miniz
cargo::rustc-link-lib=static=lz4
cargo::rustc-link-lib=dylib=stdc++
cargo::rustc-link-lib=dylib=pthread
cargo::rustc-link-lib=dylib=dl
cargo::metadata=include=<include>
cargo::metadata=lib=<lib>
cargo::metadata=linkage=static
cargo::metadata=build_tag=<build tag>
```

The C++ standard library is `dylib=stdc++` on Linux and `dylib=c++` on macOS. With the Microsoft toolchain, the build script emits no instruction for it.

SCS-69. On the vendored route, the build script MUST emit, after the C++ standard library, a `dylib` link instruction for each system library of the constant of its kind of target in `route.rs`:

| Kind of target | System libraries | Source of the list |
| --- | --- | --- |
| Linux with the GNU C library | `pthread`, `dl` | `Threads::Threads` and `${CMAKE_DL_LIBS}`, which `source/core/CMakeLists.txt` of the source tree links into `core` |
| macOS | None | Threads are part of libSystem, and `CMAKE_DL_LIBS` is empty on Darwin |
| Windows with the Microsoft toolchain | `kernel32`, `user32`, `gdi32`, `winspool`, `shell32`, `ole32`, `oleaut32`, `uuid`, `comdlg32`, `advapi32` | `CMAKE_CXX_STANDARD_LIBRARIES`, which CMake initializes to these ten for the Microsoft compiler and links into every program and library of Slang's own build |

Slang's CMake declares no Windows system library for `core`, `compiler-core`, or `slang`, and Slang's own DLL gets them only from CMake's default. Its code still calls them, such as `SHFileOperationW` of shell32 in `Path::removeNonEmpty` of `source/core/slang-io.cpp`, and the link line of rustc for a Windows target has no shell32. Without the list, a link on Windows that pulls in `Slang::Path::removeNonEmpty` fails with `LNK2019: unresolved external symbol __imp_SHFileOperationW`. The ten libraries are import libraries of the Windows SDK, which add nothing to a program that calls none of their functions. Whether a program that pulls in other objects of the archives needs a library outside the ten is SA-18.

The link kind is plain `static`, so Cargo copies the archives into the rlib of the crate. A test binary linked on the vendored route depends only on the C++ and C runtime libraries of the system, and an entry of the shared folder that is removed after the build does not break the binaries already built.

SCS-28. On the vendored route, the build script MUST run `cmake` with `std::process::Command` and check every exit status (SYS-4), or else a failed step surfaces later as an obscure link error. It MUST use the Ninja generator and MUST run `ninja --version` before it copies anything.

Without the probe, a missing Ninja would stop the build only after a copy of the source tree and a configure step.

It MUST pass `-DSLANG_VERSION_FULL=` with the build tag of `slang-src` and `-DSLANG_VERSION_NUMERIC=` with its numeric part. Without the first, the library reports the modification time of the program as its build tag (Section 3.6), which the run-time check of the safe crate rejects, and without the second, the configure step fails (Section 3.10).

It MUST disable the optional components and everything that downloads, and pass `-DFETCHCONTENT_FULLY_DISCONNECTED=ON`, because only `slang-src` reaches the network (ARCH-35). It MUST link the six static libraries of Section 3.10, the C++ standard library of the target, and the system libraries of SCS-69, or else the link fails with undefined references.

SCS-43. On the vendored route, the build script MUST stop the build when `TARGET` differs from `HOST`, because Slang's build runs programs that it builds for the target, so a cross-compilation fails deep inside CMake or Ninja.

It MUST also stop the build for a target whose C++ runtime library has no link rule. The rules cover Linux with the GNU C library, which links `stdc++`, macOS, which links `c++`, and Windows with the Microsoft toolchain, which links no extra C++ library, each with its archive names and the system libraries of SCS-69. A guess for another target would fail in the linker, so the build cannot be correct there. These are the only refusals of the vendored route for a target, and the dynamic route refuses no target for its platform (SCS-65).

SCS-44. The build script MUST run every git command with `GIT_OPTIONAL_LOCKS=0` and with the 15 repository variables of the constant `GIT_REPOSITORY_VARIABLES` of `route.rs` removed from its environment. Those are the variables that `git rev-parse --local-env-vars` lists, such as `GIT_DIR` and `GIT_INDEX_FILE`.

A git hook that runs Cargo sets such variables, and git would then answer for another repository than the source tree. Without `GIT_OPTIONAL_LOCKS=0`, a git command can refresh the index of the source tree and so write into a tree that `slang-src` owns, which another build may be reading.

SCS-45. On the vendored route, the build script MUST build a copy, inside `OUT_DIR`, of the tracked files of the source tree and of the eight submodules of Section 3.10, and MUST NOT run any build step in the source tree. It MUST refuse a tracked path that is not made only of normal components. It MUST remove every file of the copy that the listing does not name.

Slang's build writes into its source tree (Section 3.10). The source tree belongs to `slang-src`, it is a clone in the shared folder that other builds read at the same time, and two builds in one tree race on the same temporary files.

A path that is not made only of normal components would let the copy write outside `OUT_DIR`. A file that the commit no longer has would otherwise stay in the build. `slang-src` checks that every required submodule is checked out (SRC-4 of the `slang-src` design), so a missing submodule never reaches this step.

SCS-46. On the vendored route, the build script MUST copy and configure again whenever the stamp differs from the stamp of the current run, and MUST write the stamp only after a build that SCS-47 verified.

The stamp holds the root of the source tree, its commit, its build tag, the list of required submodules, and the full argument list of the configure step, which carries the options of the target, followed by the inputs of the entry key (SCS-56). The inputs carry the compiler and the variables that CMake reads, so a change of compiler also copies and configures again, with a fresh CMake cache (step 10).

A stamp written before the verification would let an interrupted or failed build skip the copy and the configure step on the next run. The build script removes the old stamp before it copies, so a failure in steps 10 to 14 leaves no stamp.

SCS-47. On the vendored route, after the build, the build script MUST check that each of the six static libraries exists as a file, named `lib<name>.a`, or `<name>.lib` with the Microsoft toolchain, and that `slang/build/Release/include/slang-tag-version.h` defines `SLANG_TAG_VERSION` as the build tag of `slang-src`, read with the rules of Section 9.2. A build that produced another layout or another version would otherwise reach the linker or the program, where the failure no longer names the cause.

SCS-48. On the vendored route, the build script MUST append the output of every `cmake` run to `slang/build.log` under `OUT_DIR`, truncating the log once at the start of each run that builds.

On a failure of the configure or build step, its error MUST name the log and hold the last 40 lines of it.

A full build prints hundreds of lines, and an error that holds them all buries the cause, while an error without them leaves the user to find the log.

SCS-56. The build script MUST keep the six static libraries and the include directory of each verified build in an entry of the directory `slang-compiler-sys` of the shared folder, with the entry protocol of SRC-16 and the entry key of SRC-17 of the `slang-src` design, and MUST use a complete entry whose inputs equal those of the current build instead of building.

A vendored build takes minutes, and Cargo gives each profile and each host or target unit its own `OUT_DIR`, so a procedural macro that links Slang and every profile would otherwise build it again (Section 8 of the `slang-src` design). With the shared folder, it runs once for each set of inputs in a target directory.

An entry has the directories `lib`, with the six libraries, and `include`, with every file of the include directory of the build tree, the file `inputs`, and the stamp file. The build script builds in `OUT_DIR`, then copies the two directories into the temporary directory `<key>.tmp-<process id>` of the entry and renames it, so a miss costs one copy of those two directories. Before it copies into the temporary directory, it removes only the temporary directories `<key>.tmp-<number>` of the same key, and it removes no entry of another key.

The inputs of the key follow SRC-17 of the `slang-src` design, with these values:

- `files` is empty, because the build reads no file of this crate.
- `configure` holds one line for each argument of the configure step except `-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=`, whose value is a path under `OUT_DIR`. The arguments `-S` and `-B` of the configure command are paths under `OUT_DIR` as well and are not arguments of the configure step. The stamp of SCS-46 keeps the full list.
- `compiler` is the path of `cl.exe` that SCS-68 finds with the Microsoft toolchain. That path holds the Visual Studio installation and the version directory of the toolset, so it changes with the compiler.
- On every other target, `compiler` is the first line of the output of the compiler probe. The probe reads `CXX` as CMake reads it. A value that is empty or blank probes `c++ --version`, a value that names an existing file is the program as a whole, so a path with spaces works, and otherwise the first word is the program and the other words are its arguments, so `ccache g++` runs `ccache g++ --version`.

A failure of `cmake --version` or of the compiler probe stops the build with the message of Section 9.6, because a key with an empty input could match an entry of another compiler.

An entry is complete only when its file `inputs` equals the inputs of the current build byte for byte, each of the six archives exists as a file in its `lib`, every file of the include directory of the source tree exists as a file in its `include`, and its `slang-tag-version.h` defines the build tag of `slang-src`. The build script replaces an entry that fails one of these checks, also one that was damaged after it was stored, because the generation reads the headers and the link reads the archives. An entry that cannot be inspected, other than one that does not exist, and an include directory of the source tree that cannot be listed stop the build with the message of Section 9.6.

SCS-68. With the Microsoft toolchain, the build script MUST find `cl.exe` for the target with the crate `find-msvc-tools` before it computes the entry key, and MUST give the environment that the lookup returns to every `cmake` run, added to the inherited values. It MUST stop the build with the message of Section 9.6 when the lookup finds no compiler. It MUST NOT require a developer command prompt.

A build from an ordinary shell on Windows has no developer environment, and Slang's build with Ninja then finds no compiler. A developer environment that a CI step or a user exports for the whole build sets `VCINSTALLDIR`, which makes rustc take `link.exe` from `PATH`, where another program of that name can come first, such as the `link` of Git's tools in a Git Bash shell. The lookup gives the environment only to the `cmake` runs, so rustc keeps its own lookup of the linker.

The lookup takes the developer prompt that `VCINSTALLDIR` marks first, then each installation of Visual Studio 2015 or later that the Visual Studio setup configuration or the registry lists, limited to the toolset that `VCToolsVersion` names. CMake's Ninja generator on Windows takes the first compiler on `PATH`, so it finds `cl` in the environment of the lookup without `-DCMAKE_CXX_COMPILER`, and a `CC` or `CXX` of the user still takes precedence. `find-msvc-tools` is the crate that the `cc` crate uses for the same lookup (Section 14).

The vendored CI job builds and links the vendored route on Linux, on macOS, and on Windows with the Microsoft toolchain. On macOS, the archive names `lib<name>.a` and the C++ standard library `c++` without a framework hold. On Windows, the lookup of SCS-68 finds `cl.exe` from a shell without a developer environment, CMake's Ninja generator builds with it, and the archive names `<name>.lib` and the default runtime library `MultiThreadedDLL` link without a mismatch.

The paths under `OUT_DIR` of the copy and the build tree stay within the path length limit of Windows for the target directory of the hosted Windows runner. A deeper target directory is not shown, and SRCQ-1 of the `slang-src` design keeps that question open. The lookup on a machine with another version of Visual Studio or with several installations is SA-17 (Section 15).

SCS-63. The recipe `test-vendored` MUST run the tests of `slang-src`, `slang-glslang-sys`, `slang-compiler-sys`, and `slang-compiler` with the feature `vendored` of `slang-compiler`, in the target directory `target/vendored`, with `VULKAN_SDK`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` removed from the environment, and MUST fail when it selects no test.

The recipe selects the packages and the feature through the `justfile` variables of TOOL-7 of the tooling design, which name these four crates and the feature `slang-compiler/vendored`.

Without `VULKAN_SDK` and the loader path, a test binary that still needed the Slang shared library could not start, and a build script that still read the SDK would fail, so the tests pass only when the link is static. A separate target directory keeps the vendored build from replacing the dynamic build of `target/`, which every other recipe uses.

The recipe needs network access on its first run in `target/vendored`, and none afterwards while the shared folder `target/vendored/slang-src` exists. The tooling design specifies its command:

```sh
just test-vendored
```

The recipe removes no variable that Windows uses to find a DLL, so on Windows it does not prove the static link in the same way.

### 9.4. Generation of the Bindings

SCS-64. On both routes, the build script MUST run `slang_bindgen::generate` on every run, with the header directory of the Slang that the build links and the value of `TARGET`, and MUST write both texts under `OUT_DIR` before it emits any link instruction. It MUST stop the build with the message of Section 9.6 when the generator fails.

The header directory is `$VULKAN_SDK/include/slang` on the dynamic route, and the include directory of the entry in the shared folder on the vendored route, which holds the headers of the built commit and its generated `slang-tag-version.h`. The include directory of the source tree is not used, because it lacks `slang-tag-version.h` and could differ from the copy that was built.

Generation takes well under a second for one target. Running it on every run, without a stamp, keeps the bindings equal to the headers without a second record of what was generated, and the build script runs only when one of its inputs changed (Section 9.5).

A failure of the generator for a Slang other than the recorded one means that its headers hold declarations or constructs that the rule set does not cover. The message says that another Slang is best effort, names the Vulkan SDK and the feature `vendored` as the two ways to the tested Slang, and holds the error of the generator with each of its source errors.

On the dynamic route, `slang-src` is not a dependency, so the build script cannot name the recorded build tag or the version of the SDK that ships it. The message says instead that the crates are tested against the Slang that the version of `slang-src` that `slang-compiler-sys` requires records, and gives the SDK advice of Section 9.6 for the SDK that ships that Slang.

### 9.5. Rerun Instructions and Metadata

SCS-30. The build script MUST emit these rerun instructions, each before it reads the input:

- `cargo::rerun-if-env-changed` for `LIBCLANG_PATH` on every run.
- On the dynamic route, `cargo::rerun-if-env-changed` for `VULKAN_SDK`, and `cargo::rerun-if-changed` for the directory `$VULKAN_SDK/include/slang`.
- On the vendored route, `cargo::rerun-if-env-changed` for `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `CMAKE_TOOLCHAIN_FILE`, `MACOSX_DEPLOYMENT_TARGET`, `VCINSTALLDIR`, and `VCToolsVersion`, in that order, and `cargo::rerun-if-changed` for the stamp file of the entry that it uses.

A build that does not rerun after its input changes links a library or compiles bindings that no longer match it (SYS-1). Cargo scans a directory named by `rerun-if-changed` for changes, and the SDK's header directory holds a handful of files, so the instruction covers every header that the generator reads. The first seven variables of the vendored route are those that CMake reads at configure time, so a change of compiler reruns the configure step through the stamp. `VCINSTALLDIR` and `VCToolsVersion` steer the lookup of SCS-68, and `VCToolsVersion` is also an input of the entry key.

`build.rs` prints every instruction that the logic collected before it returns the result, so the instructions reach Cargo when the build script fails as well. A test of Section 9.7 asserts the rerun instructions on every failure path.

The build script emits no rerun instruction for these inputs:

- `route.rs` and `system.rs`, because Cargo rebuilds and reruns the build script when a file that it includes changes.
- The feature `vendored` and the variables that Cargo sets for each build, such as `CARGO_CFG_TARGET_OS`, `TARGET`, `HOST`, `OUT_DIR`, and `NUM_JOBS`, which Cargo tracks itself.
- The metadata of `slang-src`, because Cargo runs this build script again when the build script of `slang-src` ran again and its metadata changed (SRCA-1 of the `slang-src` design).
- `$VULKAN_SDK/lib` and the library file, whose kind it checks but whose content it does not read.
- The files under `OUT_DIR` that it writes itself.

The build script reads every tracked file of the source tree when it copies the tree, but it does not watch those files, because the tree holds too many to watch on every build, as SYS-1 permits for a tree whose commit is checked. The clone entry that holds the tree is never changed after it is created (SRC-16 of the `slang-src` design).

SCS-54. The manifest MUST declare `links = "slang-compiler"` (FFI-2), and the build script MUST export, with `cargo::metadata`, the keys `include`, `lib`, `linkage`, and `build_tag` on both routes. A later crate that builds against the same Slang, such as `slang-rhi-sys`, needs the headers, the libraries, and the linkage of the Slang that this crate links, and a second search would find another one.

A dependent build script reads them as `DEP_SLANG_COMPILER_INCLUDE`, `DEP_SLANG_COMPILER_LIB`, `DEP_SLANG_COMPILER_LINKAGE`, and `DEP_SLANG_COMPILER_BUILD_TAG`:

| Key | Dynamic route | Vendored route |
| --- | --- | --- |
| `include` | `$VULKAN_SDK/include/slang` | The directory `include` of the entry in the shared folder |
| `lib` | `$VULKAN_SDK/lib` | The directory `lib` of the entry in the shared folder |
| `linkage` | `dynamic` | `static` |
| `build_tag` | The build tag of SCS-52 | The build tag of `slang-src` |

On both routes, `include` is the directory that holds `slang.h` and `slang-tag-version.h` directly, so a dependent finds the headers the same way on each.

### 9.6. Failure Messages

SCS-31. On each failure of this table, the build script MUST end by returning an error from its `main` (SYS-4) whose message contains every item of the table's second column. A build that stops without naming what is missing and how to supply it leaves the user to guess, and a guess often links the wrong Slang.

In this table, the SDK advice is the sentence that tells the user two things. It says to install the LunarG Vulkan SDK from <https://vulkan.lunarg.com>. It says to set `VULKAN_SDK` to the absolute path of its directory that holds `include/slang` and `lib`, which is the `x86_64` directory of the SDK on Linux, the `macOS` directory on macOS, and the installation directory on Windows.

The vendored advice is the sentence that says that enabling the Cargo feature `vendored` builds Slang from source instead, which needs git, CMake, Ninja, a C++ compiler, and network access for the first build in a target directory.

Every row is a failure that the build script raises. A function of `route.rs` builds the message of each, and a test of Section 9.7 checks each.

The first nine rows belong to the dynamic route, the next three to both routes, and the others to the vendored route. A command in a message is the program followed by its arguments, without its environment.

| Failure | The message contains |
| --- | --- |
| `VULKAN_SDK` is absent or empty | The name `VULKAN_SDK` with "is not set" or "is set but empty", that `slang-compiler-sys` cannot find the Slang library to link, the SDK advice, and the vendored advice (ARCH-5) |
| The value of `VULKAN_SDK` is not valid UTF-8, contains a line feed or a carriage return, or is not an absolute path (SCS-40) | The name `VULKAN_SDK` and its value in quotation marks, with every control character escaped and every byte that is not UTF-8 replaced, the statement that the value is not an absolute path in UTF-8 without line breaks, and the SDK advice |
| `$VULKAN_SDK/include/slang/slang-tag-version.h` cannot be read | "cannot read", the full path, the reason that the operating system gives, why the build reads the file, and the SDK advice |
| That file has no line that defines `SLANG_TAG_VERSION` | The full path, the name `SLANG_TAG_VERSION`, and the SDK advice |
| A line of that file defines `SLANG_TAG_VERSION` as anything other than one string literal | The number of the line, the full path, and the SDK advice |
| `$VULKAN_SDK/lib` cannot be accessed or is not a directory (SCS-41) | "cannot access" with the path and the reason that the operating system gives, or the path with "is not a directory", then the SDK directory, the library directory, the name of the library file on each of Linux, macOS, and Windows, and the SDK advice |
| The target has the operating system of the host, and the library file of the target in `$VULKAN_SDK/lib` cannot be accessed or is not a file (SCS-41) | "cannot access" with the path of the file and the reason that the operating system gives, or the path with "is not a file", then the same content as the previous row |
| `TARGET` is absent, empty, or not valid UTF-8, on the dynamic route | The name `TARGET` with "is not set" or its value in quotation marks with "which is not a target triple", that the build cannot generate the Slang bindings for the target of the build, and that Cargo sets `TARGET`, so the crate is built through Cargo |
| `OUT_DIR` is absent, or is not an absolute path in UTF-8 without line breaks, on the dynamic route | The name `OUT_DIR` with "is not set" or its value in quotation marks, the statement that the build has no directory in which to write the generated Slang bindings, and that Cargo sets `OUT_DIR`, so the crate is built through Cargo with a target directory whose path is UTF-8 |
| `CARGO_CFG_TARGET_POINTER_WIDTH` is not `64` (SCS-66) | The target triple, its pointer width, and that the Slang bindings support only 64-bit targets, because a 32-bit target has other struct layouts and calling conventions |
| The generator fails (SCS-64) | "cannot generate the Slang bindings", the header directory, the target, and the error of the generator with each of its source errors. For an error of kind `Libclang`, the advice to install libclang or set `LIBCLANG_PATH`. For an error of kind `Target`, that libclang does not accept the target triple, so the bindings cannot be generated for it, and the tested targets. For an error of kind `UnmatchedDeclaration`, `StaleRule`, or `Unsupported`, the build tag of the headers, that the crates are tested against the Slang that the required version of `slang-src` records and that another Slang is best effort, the SDK advice for the SDK that ships that Slang (Section 9.4), and the vendored advice |
| `bindings.rs`, `symbols.rs`, or `build_info.rs` cannot be written under `OUT_DIR` (SCS-64, SCS-53) | "cannot write" with the path, the reason that the operating system gives, and that the build writes the generated Slang bindings and the build information under `OUT_DIR` |
| A variable of `slang-src` is absent, or a value fails a check of SCS-55 | The name of the variable, its value in quotation marks, escaped as for `VULKAN_SDK`, when it has one, and that the vendored route reads the source tree from `slang-src`, which has to be a direct dependency of the same version |
| `OUT_DIR` is absent, or is not an absolute path in UTF-8 without line breaks (SCS-55) | The name `OUT_DIR` with "is not set" or its value in quotation marks, the statement that the build has no directory in which to build Slang, and that Cargo sets `OUT_DIR`, so the crate is built through Cargo with a target directory whose path is UTF-8 |
| `TARGET` or `HOST` is absent (SCS-55) | The name of the variable with "is not set", that the build cannot compare the target with the host, and that Cargo sets the variable |
| The target differs from the host (SCS-43) | Both target triples, that the build from source cannot cross-compile because Slang's build runs programs that it builds for the target, and the two remedies: build on a host of the target, or build without the feature `vendored` to link the Slang of a Vulkan SDK |
| The target has no link rule for its C++ runtime library (SCS-43) | The target triple, the three kinds of target that have a rule, that the build cannot link Slang's C++ code without one, and the same two remedies |
| `git`, `ninja`, `cmake`, or the program of the compiler probe cannot start (SYS-4, SCS-56) | "cannot start" with the program, the command, and the reason that the operating system gives, that the build from source needs git, CMake 3.22 or later, and Ninja on `PATH`, and a C++ compiler, and the remedies: install the missing program, or build without the feature `vendored` |
| With the Microsoft toolchain, the lookup finds no `cl.exe` (SCS-68) | "cannot find the Microsoft C++ compiler cl.exe" with the target, that the build runs CMake with Ninja, which needs the environment of the Microsoft toolchain, where the build script looked, which is the developer prompt that `VCINSTALLDIR` marks and every installation of Visual Studio 2015 or later that the setup configuration or the registry lists, limited to the toolset that `VCToolsVersion` names, the value of each of the two variables or that it is not set, and the remedies: install Visual Studio or its Build Tools with the workload for desktop development with C++, or build without the feature `vendored` |
| `git ls-files` prints an entry that the build cannot read or a path that is not made only of normal components (SCS-45) | The command, the directory, and the output in quotation marks, with every control character escaped, and that it is not the output that the build expects from git |
| `git ls-files`, `ninja --version`, `cmake --version`, or the compiler probe exits with a failure (SYS-4, SCS-56) | The command, the directory when the command has one, the exit status, or that a signal ended the program, and its error output |
| A file of the copy cannot be listed or removed, the stamp or the log cannot be written or removed, or the log cannot be opened for a `cmake` run (SCS-45, SCS-46, SCS-48) | "cannot" with the action and the path, the reason that the operating system gives, and that the build keeps the copy, the build tree, and the log under `OUT_DIR` |
| A file cannot be copied (SCS-45) | "cannot copy" with the source path and the destination path, the reason that the operating system gives, and that the build copies the tracked files into `OUT_DIR` because Slang's build writes into its source tree |
| The configure or build step of `cmake` exits with a failure (SCS-48) | The command, the exit status, or that a signal ended the program, the name of the step, the path of the log, and its last 40 lines, or that the log cannot be read with the reason |
| A static library is not a file, or the version header of the build tree cannot be read, lacks `SLANG_TAG_VERSION`, or defines it malformed (SCS-47) | "the build of Slang from source finished, but", the problem with the path, and that the build produced another layout than the one that this version of the crate expects |
| The version header of the build tree defines another build tag (SCS-47) | The build tag, the path of the header, and the build tag of `slang-src`, with the reason that the bindings and the build information name that build tag |
| An entry of the shared folder or its lock cannot be created, inspected, listed, locked, written, renamed, or removed, the modification time of the stamp file of a new entry cannot be set, or the include directory of the source tree cannot be listed (SCS-56) | "cannot" with the action and the path, which for the stamp file is "cannot write" when the file cannot be written and "cannot set the modification time of" when only its time cannot be set, the reason that the operating system gives, and that removing the entry, or running `cargo clean`, when no build runs is safe |

A variable is present when the environment contains it, whatever its value.

The message of a libclang that cannot be loaded holds the advice twice, once from the generator (Section 7.7) and once from the build script, because each must make sense without the other. The build script maps the kinds of the generator to the four kinds of `GenerationError` of Section 9.7, and adds advice only for `Libclang`, `Target`, and `Headers`. The kinds `Io`, `Diagnostic`, `Evaluation`, and `NameCollision` map to `Other` and get none.

The error type `BuildError` of `route.rs` holds the message. Its `Debug` implementation writes the message unchanged, so the error that `main` returns prints as readable text and not as an escaped string.

The build script emits no `cargo::warning` other than the statement of SCS-65 on an untested target, because a warning reaches the user only when the build fails (Section 9.2). It writes the output of `cmake` and Ninja to its log, and it shows that output only in the error of a failed configure or build step (SCS-48).

This example shows the message when `VULKAN_SDK` is unset:

```text
VULKAN_SDK is not set, so slang-compiler-sys cannot find the Slang library to link. Install the LunarG Vulkan SDK from https://vulkan.lunarg.com, and set VULKAN_SDK to the absolute path of its directory that holds include/slang and lib, which is the x86_64 directory of the SDK on Linux, the macOS directory on macOS, and the installation directory on Windows. Enabling the Cargo feature `vendored` builds Slang from source instead, which needs git, CMake, Ninja, a C++ compiler, and network access for the first build in a target directory.
```

### 9.7. Structure and Tests of the Build Script

The build script follows SYS-8 of the coding standard. It consists of two files that `build.rs` includes with `include!`, in this order: `route.rs` and `system.rs`.

- `crates/slang-compiler-sys/route.rs` holds the logic, as plain functions without `main`. The test module `tests/sys/build_script.rs` includes it as well.
- `crates/slang-compiler-sys/system.rs` holds the struct `System`, which implements the trait `Host` on the real environment, file system, processes, and generator. The test module `tests/sys/build_script/ffi.rs` includes it as well.
- `build.rs` calls `configure` with `System`, prints each collected instruction, and returns the result from `main`.

The logic reaches the environment, the file system, the generator, and other programs only through the private trait `Host`, which has these methods:

- `var`, which returns the value of an environment variable as an `OsString`, or nothing when the variable is absent.
- `read`, which returns the text of a file, with every byte sequence that is not UTF-8 replaced, because the log of a failed build can hold compiler output in another encoding, and its last lines have to reach the error (SCS-48).
- `kind`, which says whether a path is a directory, a file, or something else, following symbolic links, or returns the error of the file system.
- `host_os`, which names the operating system of the host in the form of `CARGO_CFG_TARGET_OS`.
- `run`, which runs a program described by the struct `Invocation`, with its arguments, its working directory, the variables to set as pairs of `OsString` values, and the variables to remove, and returns its exit status and both of its output streams. The values to set are `OsString` values, because the environment of the Microsoft toolchain holds paths that need not be UTF-8.
- `run_to_log`, which runs a program in the same way, appends both of its output streams to a log file, and returns only its exit status. Its error is the enumeration `LoggedRunError`.

  Its variant `Log` says that the log file could not be opened or its handle duplicated, and its variant `Start` says that the program could not be started. The logic reports the first as a failure to open the log and the second as a program that cannot start (Section 9.6).
- `files`, which lists every file under a directory, relative to it.
- `list`, which returns the names in one directory, so that the logic finds the temporary directories `<key>.tmp-<number>` of an entry (SRC-16 of the `slang-src` design).
- `sync_file`, which copies a file, creating the parent directories, and writes it only when its bytes differ from the existing copy.
- `remove_file`, which removes a file, and `remove_dir_all`, which removes a directory with its content.
- `write`, which writes a text to a file, creating the parent directories.
- `set_modified`, which sets the modification time of a file, for the stamp file of an entry (SRC-16 of the `slang-src` design).
- `rename`, which renames a directory.
- `lock`, which creates the parent directory of a lock file, opens the file, creating it, and takes an exclusive lock on it that lasts until the process exits.
- `process_id`, which returns the identifier of the process, for the name of the temporary directory of an entry.
- `msvc_compiler`, which looks up `cl.exe` for a target with `find-msvc-tools` and returns the struct `MsvcCompiler` with its path and its environment, or nothing (SCS-68). `System` compiles the lookup only with the feature `vendored` and returns nothing without it.
- `generate`, which calls `slang_bindgen::generate` and returns the two texts, or a `GenerationError`. That error holds the message of the generator's error, followed by each of its source errors after ", caused by: ", and one of four kinds. The kind is `Libclang` or `Target` for the generator's kind of the same name, `Headers` for `UnmatchedDeclaration`, `StaleRule`, and `Unsupported`, and `Other` for every other kind.

The dynamic route uses only `var`, `read`, `kind`, `host_os`, `write`, and `generate`. The other methods serve only the vendored route.

The build logic of `slang-glslang-sys` repeats parts of `route.rs` and `system.rs` as copies, for the reason that Section 4.1 of `docs/design/slang-glslang-sys.md` gives, so a change to the shared parts is checked against that copy in the same change.

The function `configure` takes a host and a vector of instructions, appends each Cargo instruction to the vector, and returns `Result<(), BuildError>`. `System` implements the methods with `std::env::var_os`, `std::fs`, `std::process::Command`, `std::env::consts::OS`, `std::process::id`, `find-msvc-tools`, and `slang-bindgen`. It starts each program with standard input closed, and it keeps each lock file open in a field until the process exits.

The logic names no type of `slang-bindgen`, so the test binary compiles `route.rs` without the generator.

The test module `build_script` includes `../../route.rs` and implements `Host` on a fake. The fake holds lists of variables, of file contents, and of path kinds or errors, the name of a host operating system, a function that answers each program run, and a function that answers each generation.

It records each variable that the logic reads, each program that it runs, each generation that it requests, and each file and directory that it copies, removes, renames, locks, or writes. A path that the fake does not list is not found.

The tests of the module `build_script` cover:

- The dynamic route:
  - The exact list of instructions on success, the generation from the header directory of the SDK for the target, and the exact list of the variables that the route reads, which is `CARGO_FEATURE_VENDORED`, `VULKAN_SDK`, `CARGO_CFG_TARGET_OS`, `TARGET`, `CARGO_CFG_TARGET_POINTER_WIDTH`, and `OUT_DIR`. The build script reads `CARGO_FEATURE_VENDORED` first, to select the route (SCS-50).
  - An absent or invalid `TARGET` and `OUT_DIR`, and each generated file that cannot be written.
  - An absent, empty, relative, non-UTF-8, and line-breaking `VULKAN_SDK`.
  - A header that cannot be read, and a missing and malformed build tag, and any build tag accepted without comparison.
  - The last definition and CRLF lines.
  - Each case of the library check for the host's operating system, and a foreign or unknown target.
  - The content of `build_info.rs`, including a build tag that does not have the form of SRC-12.
- Both routes:
  - On each tested target, no warning and `tested_target` set to `true`.
  - On an untested 64-bit target, such as `aarch64-unknown-linux-gnu`, the one warning of SCS-65, `tested_target` set to `false`, and a build that does not stop.
  - The stop on a target whose pointer width is not 64 (SCS-66).
- The vendored route:
  - The exact list of instructions without a read of `VULKAN_SDK`.
  - The programs with their arguments and environment, including the build tag of `slang-src` in the configure step.
  - The copy and the stamp written after the build, a matching stamp that skips the copy and the configure step, and the removal of stale files.
  - The archives, the C++ standard library, and the system libraries of each kind of target that has a link rule, as the full list of link instructions for Linux on x86-64 and on aarch64, macOS, and Windows with and without `crt-static`, and the stop on a target without one (SCS-43, SCS-69).
  - The use of a complete entry, the replacement of an entry whose inputs, archives, or headers differ, the storage of a new entry by rename with its stamp file written last with the old modification time, a failure to write the stamp file and a failure to set its time as two separate messages, the removal of only the temporary directories of the same key, and the rerun instruction for that stamp file.
  - The key on the compiler and the variables that CMake reads, and the forms of `CXX` that the compiler probe reads.
  - With the Microsoft toolchain, the stop when the lookup finds no compiler, with where it looked, and the environment of the toolchain on both `cmake` runs.
  - The removal of `CMakeCache.txt` when the stamp differs.
  - The content of `build_info.rs`.
- The failures of both routes:
  - Every failure of Section 9.6, and a failure of the generator of each kind.
  - A signal that ends a program.
  - A log that cannot be opened, which is not reported as a program that cannot start.
  - A `cmake` step that cannot start, a log that cannot be read, and a missing job count.
  - A stale stamp that stays removed after a failure.

Each test asserts the content of each message and the instructions that the logic emits, including the rerun instructions on every failure path. Each test of the vendored route also asserts that `VULKAN_SDK` was not read. Each test of a failure before the verified build asserts that no stamp and no entry of the shared folder was written. A failure while the build script stores an entry comes after it wrote the stamp and files of the temporary directory, so those tests assert that no rename and no generation took place instead. These tests are pure Rust, so they run under nextest and under Miri.

The module `build_script::ffi` includes `../../../system.rs` and tests `System` itself. Its tests cover:

- Reading a variable, the kinds of paths, and the operating system of the host.
- Listing the files of nested directories and the names in a directory, writing, renaming, and removing files and directories and creating their parents, setting the modification time of a file, and keeping an unchanged file while rewriting a changed one.
- Taking a lock that a second lock on the same file waits for, in a child process. The child writes a marker file and then takes the lock, and once it holds the lock it requires a second marker that the parent writes just before it releases the lock, so a lock that does not block fails the child however late the child starts.
- The lookup of the Microsoft compiler, which equals the result of `find_msvc_tools::find_tool` for the Windows targets of the Microsoft toolchain with the feature `vendored`, and gives nothing for other targets and without the feature.
- Copying the tracked files and removing stale ones through the function `sync_copy` of `route.rs`.
- Running `rustc` and capturing its exit status and output.
- Running `rustc` with a variable set and another removed, and checking through `env!` and `option_env!` in the compiled code that it sees both changes (SCS-44).
- Appending the output of a logged run to its log, and telling a log that cannot be opened, which `run_to_log` returns as `LoggedRunError::Log`, from a program that cannot start.
- The mapping of `generate` for a missing `slang.h`, which gives the kind `Other` with the source errors, a synthetic header, which gives `Headers`, and a triple with an invented architecture, which gives `Target`. The test uses a triple that no libclang accepts, because which real triples a libclang accepts depends on its version.
- A libclang that cannot be loaded, which gives `Libclang`, in a child process of the test binary with `LIBCLANG_PATH` set to a missing directory, because a process can load libclang only once.

These tests work in `CARGO_TARGET_TMPDIR`. They start a process or load libclang, so they sit in a module named `ffi`, which Miri skips (SCS-33), and they run on Linux, Windows, and macOS.

`check-fmt` passes every Rust file that git tracks or does not ignore to rustfmt, including `route.rs` and `system.rs` (Section 4.8 of the coding standard). `cargo fmt --all --check` alone does not read them, because a crate includes them with `include!`.

## 10. Finding the Shared Library at Run Time

On the dynamic route, the dynamic loader of the operating system finds the Slang shared library when the program starts, as Section 4.4 of the architecture design states, and the build script emits no rpath (SCS-26). That the build script of a dependency cannot set the rpath of the final binary is assumption A-6 of the architecture design.

SCS-32. Every recipe of the `justfile` that runs compiled code which loads Slang MUST start the command line of the program that it runs with the assignment that the `justfile` variable `slang_library_path` holds. That assignment puts the library directory of `VULKAN_SDK` first on the library path of the platform, which is `LD_LIBRARY_PATH` on Linux and `DYLD_LIBRARY_PATH` on macOS, followed by any earlier value.

The `justfile` MUST NOT export that path. The `justfile` MUST leave the variable empty on Windows and when `VULKAN_SDK` is unset or empty.

These requirements hold for three reasons:

- The tests that call Slang fail before any test runs when the loader cannot find the library.
- The development container provides the path only to processes that a login shell starts (Section 10 of the tooling design). macOS with System Integrity Protection enabled is expected to remove an exported `DYLD_*` variable when it starts the protected shell that runs a recipe.
- `test-vendored` and every recipe that does not load Slang have to work without `VULKAN_SDK`.

The `justfile` computes the variable with the `just` functions `os()`, `env()`, and `quote()`, so it holds the assignment as text and no recipe exports anything. This example shows the command that `just test` runs on Linux when `VULKAN_SDK` is `/tmp/vulkan-sdk/x86_64` and `LD_LIBRARY_PATH` is empty:

```sh
LD_LIBRARY_PATH='/tmp/vulkan-sdk/x86_64/lib' CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --exclude slang-src --exclude slang-glslang-sys --profile default --no-tests=warn
```

On Windows, and when `VULKAN_SDK` is unset or empty, the variable is empty and the command starts with `CARGO_BUILD_WARNINGS=deny`.

The recipes that carry the assignment are `test`, `test-gpu`, `test-hw-gpu`, `check-doctest`, and `coverage`. Each runs test binaries or doctests that load Slang.

`check-miri` does not carry it, because Miri runs no native code. The recipes that only compile, such as `check-clippy`, `check-doc`, `check-features`, and `check-msrv`, run no compiled code and do not carry it, and neither does `check-slang-call-guard`, whose generator binary loads libclang and not Slang. TOOL-7 of the tooling design permits the assignment in front of a command of Section 4.8 of the coding standard.

On the vendored route, a program needs no Slang library at run time, because the build links Slang statically (Section 9.3). `test-vendored` therefore carries no assignment and removes `VULKAN_SDK`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` instead (SCS-63). A statically linked Slang without the loader of `slang-glslang-sys` still loads a `slang-glslang` shared module by name when the dynamic loader finds one (Section 3.10).

The assignment on the command line survives System Integrity Protection, because the shell that runs a recipe sets it for the program that it starts. The variable then reaches Cargo, which is not protected, and every program that Cargo starts inherits it. An exported variable would pass through the start of the protected shell, which is expected to remove it.

Two alternatives were rejected:

- An rpath, which SCS-26 forbids and which, by A-6 of the architecture design, is assumed to be out of reach of a dependency's build script.
- A symbolic link to the library inside `OUT_DIR`, which Cargo adds to the dynamic library path of the programs that it runs. It would make the build script of every user create a link that only the recipes of this repository need.

On every CI runner, the step that installs the SDK also sets the path that the loader of the platform reads (Section 7.3 of the tooling design). On Windows, that is `PATH`, through which the loader finds `slang-compiler.dll`.

On the Linux and macOS runners, the directory therefore appears twice on the library path of a recipe, which is harmless, because the second entry names a directory that the loader has already searched.

SA-7 (Section 15) covers the expectation that the assignment survives System Integrity Protection. System Integrity Protection is disabled on the hosted macOS runner, and both the install step and the recipe set `DYLD_LIBRARY_PATH` there. A run in CI therefore cannot show which of the two reached the test binary.

Without the variable, loading the library fails, because dyld then searches only system locations and the current directory. These facts limit the alternatives on macOS:

- `DYLD_FALLBACK_LIBRARY_PATH` also works, but macOS is expected to remove it in the same way.
- A symbolic link in `/usr/local/lib` does not work.
- `cargo run` itself sets `DYLD_FALLBACK_LIBRARY_PATH` to its target directories followed by `~/lib:/usr/local/lib:/usr/lib`.

## 11. Tests and Miri

SCS-33. Every test that calls native code, whether Slang or libclang, MUST sit in a module named `ffi`, and the nextest profile `default-miri` MUST exclude every test whose path contains a module named `ffi`, `gpu`, or `hw_gpu`. Miri cannot call foreign code, and a test that reaches a foreign call under Miri fails without a defect in the code.

The selection by module name works like the GPU tiers (Section 4 of the tooling design). The `ffi` tests stay in the default test tier, so `test` and `test-gpu` run them.

`check-miri` keeps the nextest form of Section 4.8 of the coding standard, which runs each test in its own process, because `cargo miri test` aborts the whole test binary at the first foreign call. `cargo +nightly miri nextest run` builds the test binaries without linking the native library and passes when a filter excludes the tests with foreign calls. The build script still runs natively under Miri, so `check-miri` needs `VULKAN_SDK` and libclang, but it needs no library path.

The crate `slang-compiler-sys` has one integration-test binary (TEST-10), `tests/sys/`, whose root is `tests/sys/main.rs`. It declares four modules:

- `build_info`: Pure tests of the build information. They assert that `slang_build_tag` is not empty, and that `version` is the crate version, `+slang.`, the identifiers of Section 8.1, and the route, where the test derives the identifiers from `slang_build_tag` on its own. The derivation assumes the form of SRC-12 of the `slang-src` design, which `check-vulkan-sdk` guarantees for the Slang of every CI job and of the development container.
  - On the dynamic route, they assert `.dynamic`, `Linkage::Dynamic`, and no `slang_commit`. With the feature `vendored`, they assert `.vendored`, `Linkage::Static`, and a full commit of 40 lowercase hexadecimal digits that starts with the hash of the build tag instead, and fail when the build tag has no hash of at least nine digits, so the check cannot pass without comparing anything, and read the feature with `cfg!(feature = "vendored")`, which compares the build script's record with the feature that Cargo passed.
  - They assert `tested_target` in a constant assertion, `const { assert!(..) }`, which Clippy requires for a constant, so the test binary fails to compile on an untested target.
  - They assert that `Linkage` displays as the value of `DEP_SLANG_COMPILER_LINKAGE`, and, through a `const fn` with the bounds `Send + Sync + 'static`, that `BuildInfo` and `Linkage` have the three.
- `build_script`: The pure tests of the logic of the build script, and its module `ffi`, which tests the real host of `system.rs` and starts processes (Section 9.7).
- `ffi`: The smoke test and the native-symbol test, which call Slang. The module carries `#[expect(unsafe_code, reason = "...")]`, whose reason says that the tests of the module call Slang through the raw bindings, which only unsafe code can do. This is the narrowest scope that needs the exception (LINT-1), and Section 4.4 of the coding standard lists it among the expected kinds of lint exception.
- `symbols`: The generated list of C functions of Section 5.1, included from `OUT_DIR`, without a lint exception (Section 5.10).

The tests of `build_info` and of `build_script` outside its module `ffi` call no native code and start no process, so they run under Miri.

The smoke test is `global_session_reports_the_build_tag_of_the_build_and_releases_every_reference`. It takes these steps:

1. Create a global session through `slang_createGlobalSession2` with `SlangGlobalSessionDesc::default()`, and assert the result `SLANG_OK` and a non-null session.
2. Assert that `spGetBuildTagString` and `IGlobalSession::getBuildTagString` both return `BUILD_INFO.slang_build_tag` (SCS-34).
3. Query the interface `ISlangUnknown` through `queryInterface` with the generated UUID, and assert that the result is the same pointer as the session.
4. Release the interface pointer, which leaves a count of 1, and the session, which leaves a count of 0.

The test function carries `#[expect(ffi_unwind_calls, reason = "...")]`, because it calls C functions declared `"C-unwind"` (SCS-11). Its reason says that an exception that escaped one of them would unwind into the test, which is defined and fails it.

The native-symbol test is `every_c_function_of_the_bindings_resolves_to_a_native_symbol`. It asserts that the generated list `C_FUNCTIONS` is not empty and that the address of every function in it is not null. It reads each address through `std::hint::black_box`, so the compiler keeps every reference.

The test binary therefore imports every C function that the bindings declare. A function that the library does not export fails the link of the test binary with an error such as "undefined symbol".

The test does not cover the methods of the interfaces, because a method is a slot of a vtable that Slang fills at run time, not a symbol of the library. Only the smoke test and the layout assertions of SCS-12 exercise them.

Both tests run on both routes: through the test recipes on the dynamic route, and through `test-vendored` on the vendored route, where the native-symbol test links against the static libraries (Section 9.3).

When the loader cannot find the shared library, the test binary stops before any test runs. On Linux, it prints "error while loading shared libraries: libslang-compiler.so.0.2026.17: cannot open shared object file: No such file or directory" and exits with status 127. The recipes avoid this by supplying the library path (Section 10), and a user who runs a program outside them sets the path as the crate documentation says (Section 13).

SCS-34. The smoke test MUST assert that `spGetBuildTagString()` returns `BUILD_INFO.slang_build_tag`, on both routes. A library that loads but is not the Slang that the bindings were generated from has layouts and vtables that differ from them, and the run-time comparison catches a library that the loader found somewhere other than where the build script read the headers.

The layout assertions of SCS-12 are compile-time checks and need no test. The tests of the generator are those of Section 7.5.

## 12. Continuous Integration

SCS-35. The Linux, Windows, and macOS jobs of the CI workflow that build the default MUST install the Vulkan SDK version that the CI configuration records, from LunarG, and verify the download against the SHA-256 checksum that LunarG publishes, with the expected value recorded in the workflow (Section 7.3 of the tooling design). They MUST set `VULKAN_SDK`, put the SDK's library directory on the library path of the platform, and install libclang. On Windows and macOS, the step that makes libclang available checks the libclang of the runner image and always sets `LIBCLANG_PATH` to its directory, so that the libclang that loads is the one that the step checked (Section 7.1 of the tooling design).

Every job compiles `slang-compiler-sys`, which runs its build script and the generator, and every job that runs tests starts binaries that load Slang (TOOL-21, TOOL-23). After the installation, each job runs `just check-vulkan-sdk`, which checks the headers, the library, and `slangc` of the SDK against `RECORDED_BUILD_TAG` of `slang-src`, read through the recipe `recorded-slang`, and fails when they differ, so that every job tests the recorded Slang (SRC-11 of the `slang-src` design).

The Windows and macOS jobs link the test binary of `slang-compiler-sys` against the Slang of the SDK and run the smoke test, the native-symbol test (Section 11), and the tests of the real host of Section 9.7. The `check` and `coverage` jobs on Linux also run the generator's tests (Section 7.5).

SCS-62. The CI workflow MUST have one job on each of Linux, Windows, and macOS that runs `just test-vendored` in every run of the workflow, without the Vulkan SDK and in a target directory that starts empty, so that each run clones and builds. The jobs that build the default never enable `vendored`. One dedicated job per operating system tests the vendored route, because a route that no job builds breaks without notice.

The vendored jobs clone the recorded commit from GitHub, so they also test `slang-src`. A vendored job fits the time limit of a hosted runner on Linux, macOS, and Windows with a wide margin (Section 7.4 of the tooling design). Section 7 of the tooling design specifies the jobs and what each installs, such as libclang, CMake, Ninja, and a C++ compiler. The job on Windows sets up no developer environment of the Microsoft toolchain, because the build script finds the compiler itself (SCS-68).

## 13. Crate Documentation

SCS-37. The crate-level documentation of `slang-compiler-sys` MUST contain the content of Sections 7.4 and 7.9 of the writing guide and MUST state the Slang rules of Section 3.9. It MUST also state what a default build and a vendored build each need, and that another Slang than the recorded one is best effort. The generated items have no documentation of their own (SCS-13), and the design document of the safe crate cites these rules for its `Send`, `Sync`, and lifetime decisions.

That content includes a `# Features` section that names the feature `vendored`, and a `# Platform support` section that names the tested targets of Section 6. It also includes how the crate obtains Slang on each route, how the library is found at run time on each platform, and the safety contract that every thunk and C function shares, with what happens when a C++ exception escapes a call.

The shared safety contract is the one that Section 7.9 of the writing guide requires, and it is what lets the generated items go without `# Safety` sections and safety comments (Section 5.10). It says these things:

- The caller passes only pointers that are valid, correctly typed, live, and obtained from Slang, such as a live interface pointer of the declared interface or a valid handle.
- The caller meets the contract that Slang documents for the function.
- The caller follows the threading and lifetime rules of Section 3.9.
- A call that breaks any of these is undefined behaviour.

The documentation states that the crate promises nothing about threads: every function and thunk is `unsafe`, and no interface struct is `Send` or `Sync`. It links to Slang's documentation for the meaning of each item and points users to `slang-compiler`.

The example of the crate-level documentation that creates a global session is marked `no_run`, because it needs the shared library at run time (Section 7.5 of the writing guide).

The crate-level documentation has these parts:

- A summary of what the crate is, with SPIR-V, Vulkan, vtable, and UUID defined on first use. It says when to use the crate, that `slang-compiler` wraps the bindings in a safe API that compiles shaders and reflects their parameters, and that `slango` builds on that to run compute kernels and draw with render pipelines. It links to Slang's user guide for the meaning of each item, and says how each kind of C++ declaration appears in Rust.

  It says that every build generates the bindings from the headers of the Slang that it links, names `BUILD_INFO`, and says that the crate is written and tested against the Slang that the version of `slang-src` it requires records, and that the generator can refuse the headers of another Slang.
- `# Examples`: two doctests.

  The first calls no Slang function and runs. It shows the default values of `SlangGlobalSessionDesc`, an enumeration value that the header does not list, the sign of the result codes `SLANG_FAIL` and `SLANG_OK`, and the fields of `BUILD_INFO`.

  The second is marked `no_run`. It creates a global session through `slang_createGlobalSession2`, reads its build tag through `IGlobalSession::getBuildTagString`, compares it with `BUILD_INFO.slang_build_tag`, releases the session, and returns an error when the session cannot be created or the tag differs.

  Its lead-in says that it needs the Slang shared library at run time on the dynamic route, which the dynamic loader must find as `# Linking Slang` describes. Each `unsafe` block in it has a safety comment.
- `# Features`: the feature `vendored`, off by default, what it changes, that every crate between a program and this crate forwards it, naming `slang-compiler`, `slango`, `slango-codegen`, `slango-macros`, and `slang-slint`, that the feature of `slang-compiler` also installs the loader of `slang-glslang-sys`, and that `--all-features` enables it.
- `# Platform support`: the four tested targets of Section 6, that every other 64-bit target is allowed, untested, and unsupported, with the warning and the field `tested_target` of SCS-65, and the stops of SCS-66 and SCS-43 with their reasons. It also says that the dynamic route runs with its tests in CI on x86_64 Linux, x86_64 Windows, and aarch64 macOS, the vendored route on the same three, and x86_64 macOS only as a type check (Section 6).
- `# Linking Slang`: both routes, for a user, as the next list describes.
- `# Safety`: the shared safety contract, the rules of Section 3.9 with reflection and component type defined, what happens when a C++ exception escapes a call, the rule of Section 5.6 for a user who implements a Slang interface in Rust, which is to define each function of the vtable `extern "system"` and store it through a conversion of the function pointer, with the reason, that no interface struct implements `Send` or `Sync`, and the promise that an implementation of the unsafe trait `Interface` makes.

The section `# Linking Slang` says these things:

- That the feature `vendored` selects the vendored route, that no environment variable selects a route, and that the build never falls back from one route to the other.
- That every build needs libclang, found through `LIBCLANG_PATH` or the standard locations, with how to install it on each platform.
- For the dynamic route as Section 9.2 specifies it, it names the LunarG Vulkan SDK and its download address, says which directory `VULKAN_SDK` names on each platform, that the build accepts the Slang of any SDK and records its build tag, the link from `$VULKAN_SDK/lib`, the failures that stop the build, and the rule of SCS-41 for a target of another operating system.
- For the vendored route as Section 9.3 specifies it, it says that `slang-src` obtains the source tree and links to its documentation for another Slang commit through `[patch]`. It names git, CMake 3.22 or later, Ninja, a C++ compiler, and network access for the first build in a target directory as requirements, and describes the copy, the build, the log, the six static libraries with the C++ standard library and the system libraries of SCS-69 of each kind of target that has a link rule, and the refusal of cross-compilation. It says that with the Microsoft toolchain the build finds `cl.exe` itself, needs no developer command prompt, stops with an error that says where it looked when it finds none, and honours `VCToolsVersion` (SCS-68).
- The shared folder, in which the built libraries live, with the content that SRC-20 of the `slang-src` design lists. It names no environment variable of `slang-src`, because none exists.
- That the vendored build of this crate alone leaves out slang-glslang, with the consequences of Section 3.10, and that `slang-compiler` with its feature `vendored` installs the loader of `slang-glslang-sys`, which a direct user installs as `slang-glslang-sys` documents. The recorded Slang validates SPIR-V on neither route unless `SLANG_RUN_SPIRV_VALIDATION` is `1`.
- The `links` metadata of each route (SCS-54).
- Which variable the dynamic loader reads on each platform, and what the loader reports on Linux when it cannot find the library.
- That macOS removes the `DYLD_*` variables when it starts a protected program, and that an assignment on the command line of Cargo is expected to avoid that and is not verified (SA-7).

The paragraph on exceptions in `# Safety` says these things:

- Slang raises its internal errors as C++ exceptions, and some calls of the recorded Slang let one escape for some inputs.
- Every foreign function is declared `"C-unwind"` and every vtable entry `"system-unwind"`, which name the same calling conventions as `"C"` and `"system"`, so an escaping exception is not undefined behaviour.
- With `panic = "unwind"`, the exception unwinds through the caller's Rust frames and runs their destructors. The process then aborts at `catch_unwind` or at the root of the thread, or the exception is returned as an opaque error, which Rust leaves unspecified.
- With `panic = "abort"`, the process aborts as soon as the exception reaches a Rust frame.
- A caller that must not run its destructors during such an exception stops the unwinding itself, such as with a value whose destructor aborts the process.
- The type of a callback keeps the ABI `"C"`, so a panic in a callback aborts the process.

## 14. Dependencies and Licences

SCS-61. The published crate `slang-compiler-sys` MUST have exactly these dependencies:

- `slang-bindgen` with an exact version, as a build dependency.
- `find-msvc-tools`, as an optional build dependency that only the feature `vendored` enables (SCS-68).
- `slang-src` with an exact version, as an optional dependency that only the feature `vendored` enables.
- `slang-bindgen` and `find-msvc-tools`, as development dependencies for the tests of `system.rs`.

Every dependency of a `-sys` crate runs on every user's machine, and the exact versions make the generated code and the source tree of a given version of this crate fixed.

The build script needs only the standard library, the generator, and, on the vendored route, `find-msvc-tools` to read a header, generate the bindings, find the Microsoft compiler, start processes, copy files, take locks, and compare strings. The vendored route runs `cmake` through `std::process::Command` and uses no `cmake` crate.

`find-msvc-tools` replaces the discovery of Visual Studio through the Visual Studio setup configuration, which is a COM interface, and through the registry, which the standard library cannot do. It is the crate that `cc::windows_registry::find_tool` wraps, so it is widely used, it has no dependencies, and it is under `MIT OR Apache-2.0`. Depending on it directly avoids the crates `cc` and `shlex`, which this build script does not need. It is declared once in `[workspace.dependencies]` (DEP-2). It is a development dependency without the feature as well, because the test of the real host compares the lookup of `System` with the result of `find_msvc_tools::find_tool`.

`slang-src` provides only `links` metadata to the build script, and no code of the crate uses it. Cargo's manifest lint `unused_dependencies` reports such a dependency, so the crate root names it with `use slang_src as _;` under `#[cfg(feature = "vendored")]`, as DEP-2 of the coding standard describes for a dependency that only provides `links` metadata.

The generator depends on the crate `clang` 2.1.0 and directly on `clang-sys` 1.9.1, the version that `clang` uses, for the handle of SCS-67. Through them it depends on `glob`, `libc`, `libloading`, `cfg-if`, and, on Windows, `windows-link`. Both direct dependencies are workspace dependencies with the features `runtime` and `clang_10_0`. Every build of `slang-compiler-sys` compiles them on the user's machine. These crates have these licences:

- `clang` and `clang-sys`: Apache-2.0.
- `libloading`: ISC.
- `glob`, `libc`, `cfg-if`, and `windows-link`: MIT OR Apache-2.0.

The licence allow-list of `deny.toml` includes `Apache-2.0` and `ISC`, which cover this graph (TOOL-24). None of these crates is under the GPL, as TOOL-34 requires of the dependencies of the two crates. Section 8 of the tooling design lists the whole allow-list.

`slang-compiler-sys` is under the licence of Slang, `Apache-2.0 WITH LLVM-exception`, which its manifest sets in its own `license` field. `slang-bindgen` is under `MIT OR Apache-2.0`, which it inherits from the workspace (ARCH-36 of the architecture design, TOOL-4 of the tooling design).

The dependency is declared once in `[workspace.dependencies]` as `clang = { version = "2.1.0", features = ["runtime", "clang_10_0"] }` (DEP-2). `check-msrv` verifies that it builds with the workspace's `rust-version`.

## 15. Assumptions to Confirm

Each row is a fact that this design relies on and that is not verified. A section that relies on one cites its identifier.

The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SA-7 | On macOS with System Integrity Protection enabled, the library path that a recipe places on the command line of the program it runs reaches the test binary, although macOS is expected to remove the `DYLD_*` variables when it starts a protected system program such as the system shell | SCS-32 | A run of `just test` on a Mac with System Integrity Protection enabled and without `DYLD_LIBRARY_PATH` in the environment of `just`, by the owner or a developer with such a Mac. The hosted macOS runner cannot confirm it, because System Integrity Protection is disabled there (Section 10) |
| SA-11 | Exactly the nine lints of Section 5.10 fire on the generated code of the recorded Slang on each of the four tested targets, as they do on x86_64 Linux | SCS-13 | The CI jobs on Windows and macOS, and a Clippy check for `x86_64-apple-darwin` |
| SA-12 | The parse for `x86_64-apple-darwin` reports no diagnostic and gives the same text as the parse for `aarch64-apple-darwin` | Section 6 | A Clippy check for `x86_64-apple-darwin` from Linux and a comparison of the output of the binary of SCS-60 for both targets |
| SA-14 | With `crt-static` among the target features, `CMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded` reaches every library of the vendored build, so the static runtime matches Rust's choice. The default runtime `MultiThreadedDLL` links without a mismatch (Section 9.3) | Section 9.3 | A vendored build on Windows with `-C target-feature=+crt-static` that links without a runtime mismatch |
| SA-15 | The headers of Slang releases near the recorded one generate without a change of the rule set, so that another Slang version is best effort and not impossible | Section 4.2 | A run of the binary of SCS-60 on the headers of the newest Slang release when the recorded Slang next moves |
| SA-17 | On a machine with another version of Visual Studio than that of the hosted Windows runner, or with several installations, the lookup of SCS-68 finds a compiler that builds Slang, and the path of `cl.exe` in the entry key tells the installations apart | SCS-68, SCS-56 | A vendored build on such a machine, by the owner or a developer with one |
| SA-18 | With the Microsoft toolchain, the ten system libraries of SCS-69 resolve every Windows API symbol that any Rust program can pull out of Slang's archives and those of `slang-glslang-sys`. The test binaries of `test-vendored` link with them on the hosted Windows runner, which confirms the ten for the objects that those binaries pull in, including `SHFileOperationW` of `slang-io.cpp.obj` | SCS-69 | A link on Windows of a program that uses more of Slang than the tests do, such as the examples package with the feature `vendored` |

The next assumption takes SA-19.

Assumption A-6 of the architecture design, that a dependency's build script cannot set the rpath of the final binary, also applies to this design and stays where the architecture design records it.

## 16. Open Questions

Each row is a question that this document leaves open, and the change that answers it removes the row. The next question takes SQ-8.

SQ-6 and SQ-7 come from the review of the public API before the first release (Section 9.5 of `docs/design/tooling.md`). Each is a finding of a broken rule that waits for the decision of the owner of the project and blocks the first release until the owner decides it.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SQ-6 | `BuildInfo` encodes one fact, the route, in three fields: `linkage`, `slang_commit`, which is `Some` exactly for `Static`, and the suffix of `version`. `tested_target` is a `bool` that is `true` for `x86_64-apple-darwin`, which is only type-checked, so it merges two of three states (Section 8.1). Both break TYPE-3, and `tested_target` also TYPE-2, and the public field types lock in at the release. The options are to make the route one value, such as `Linkage::Static { commit }` and `Linkage::Dynamic`, drop or derive `slang_commit`, and replace `tested_target` with a `#[non_exhaustive]` enumeration such as `TargetSupport { Tested, TypeChecked, Untested }` before the release, or to keep the shape and record why | The owner of the project, by a decision | Before the first release, M8 |
| SQ-7 | `slang_bindgen::ErrorKind::Libclang` covers a missing libclang, which the user fixes by installing it, and a libclang instance that the process already holds, which the caller fixes by calling from one thread at a time (Section 7.7). Callers match on the kind, so one kind for both breaks ERR-1, and moving the second case to its own kind later changes what an existing caller sees. The options are to add a kind such as `LibclangInUse` before the release, or to keep one kind and record why | The owner of the project, by a decision | Before the first release, M8 |

## 17. Security Considerations

The threat model has six parts:

1. The Slang library and its headers come from the Vulkan SDK, or from the source tree that `slang-src` clones, and a defect in them, or a tampered copy, reaches every program that links the crate.
2. The build script runs on the user's machine with the user's permissions and reads environment variables. It runs the generator, which loads libclang. On the vendored route, it runs `git`, CMake, and a native build of a large C++ code base from the source tree.
3. The binding generator trusts the headers it parses and writes Rust declarations that the compiler trusts without checking them against the library.
4. An agent changes the generator so that it writes a wrong declaration.
5. The generator's dependencies, such as `clang` and `clang-sys`, run their build scripts on every user's machine.
6. A CI job downloads the Vulkan SDK, and an attacker on the network or at the download host tampers with it.

Unsafe code and FFI invariants. The crate relies on the declarations matching the library, which holds because every build generates them from the headers of the Slang that the same build links (SCS-51, SCS-64), and which the run-time assertion of SCS-34 checks for the library that the loader found. It relies on the declarations matching the headers, which the layout assertions (SCS-12) and the refusals of SCS-19 secure.

It guarantees its callers nothing beyond exact declarations. Every function and thunk is `unsafe`, and no interface struct is `Send` or `Sync`.

Enumerations and `SlangResult` are integer newtypes so that an unknown value is not undefined behaviour (SCS-7, SCS-8). Thunks take raw pointers so that no Rust reference asserts an aliasing guarantee about a native object (SCS-10).

Every vtable entry and C function is declared with an unwinding ABI. A C++ exception that escapes Slang, which some methods of the recorded Slang let happen (Section 3.3), is therefore defined behaviour and not undefined behaviour (SCS-10, SCS-11). It unwinds through the caller's Rust frames and runs their destructors, which can call Slang again while its state is inconsistent.

A caller that must not let that happen stops the unwinding itself, as the guard of the safe crate does (Section 11.6 of `docs/design/slang-compiler.md`). The crate documentation states the shared safety contract, Slang's rules, and this behaviour (SCS-37).

Untrusted shader source and file paths. This crate compiles nothing and reads no shader source. The safe crate decides which files Slang can read (Section 8 of `docs/design/slang-compiler.md`).

Build scripts that run native builds. The dynamic route reads the headers of the SDK, asks the file system whether the library directory and the library file exist, generates the bindings, and emits instructions. It searches no location other than `VULKAN_SDK` (SCS-26 and SCS-41).

The dynamic route writes the value of `VULKAN_SDK` into Cargo instructions only after it has rejected a value that is not UTF-8, is not absolute, or contains a line feed or a carriage return (SCS-40). Cargo reads one instruction per line, so without this check a value with a line break could inject an instruction of its own, such as one that links another library or adds a search path. A test of Section 9.7 passes such values and asserts that the build script emits no link instruction and that its message holds no line break.

The vendored route applies the same checks to the paths that `slang-src` exports and to `OUT_DIR` (SCS-55). It reads the source tree only through git, with the repository variables of the environment removed and optional locks off, so git answers for the source tree and writes nothing into it (SCS-44). It reaches no network, because only `slang-src` does (ARCH-35, SCS-28).

The vendored route copies the tracked files into `OUT_DIR` and builds the copy, so Slang's build, which writes into its source tree, writes only under `OUT_DIR` (SCS-45). It refuses a tracked path that would leave the copy, disables every component that downloads (SCS-28), and checks the archives and the build tag that the build produced (SCS-47). The route is selected only by the feature `vendored`, which the manifest of whoever builds states (SCS-50).

The shared folder holds libraries that the vendored route links without building them again (SCS-56). It lies in the Cargo target directory, and Section 14 of the `slang-src` design states why an attacker who can write there is out of scope.

Dependencies. The published crate depends only on `slang-bindgen` with an exact version and, under `vendored`, on `slang-src` with an exact version and on `find-msvc-tools`, which has no dependencies (SCS-61). The dependencies of the generator run on every user's machine, and cargo-deny checks them (TOOL-24, TOOL-26).

The bindings are not a review surface in the repository. A reviewer reviews the change to the generator and its rule set, and the diff of the generated output that Section 8.2 produces when the recorded Slang moves.

The rule set records a reason for every decision. The generator fails on a declaration that no rule decides (SCS-2) and on a rule that decides nothing (SCS-39), so the rule set that the reviewer reads matches the headers in both directions.

A change to the generator that writes a wrong declaration compiles in every build. The generator's unit tests (Section 7.5), the layout assertions, the tests of the safe crate, and the reviewer's reading of the generator change carry that risk.

These attacks are out of scope:

- A compromised Vulkan SDK or Slang release from LunarG or the Slang project runs with the full privileges of the program that links it, and no crate can constrain it.
- A source tree of a fork of `slang-src` that the user patches in is code that the user chose to build, and CMake runs it with the user's permissions. The same holds for the programs named `git`, `cmake`, and `ninja` that `PATH` finds, for the compiler that `CXX`, `PATH`, or the lookup of the Microsoft toolchain finds in the installations of Visual Studio (SCS-68), and for the libclang that `LIBCLANG_PATH` or the standard locations provide.
- A defect in libclang that silently misreports a declaration without any diagnostic cannot be detected by the generator, though the layout assertions narrow it.
- Attacks on the repository itself are a matter of access control.

The residual risk has seven parts:

1. The layout assertions check structs and unions, not function signatures. A wrong parameter type in a function or a vtable entry that the generator's rules allow compiles and is undefined behaviour. The smoke test and the native-symbol test cover little of this: the first exercises only a few functions, and the second checks only that each C function exists in the library, not its signature.
2. On the dynamic route, the build script reads the headers and not the library. A library replaced in an SDK whose headers are unchanged passes the build, and only the run-time assertion of SCS-34 and the check of the safe crate detect it, by its build tag, which names an abbreviated commit.
3. A Slang other than the recorded one is best effort. Its headers generate bindings that match it, but the safe crates can depend on a behaviour of the recorded Slang that it does not have.
4. Slang is a large C++ code base whose defects reach every program that links it, as Section 18 of the architecture design states.
5. Miri cannot run any test that calls Slang (SCS-33), so the soundness of the declarations rests on the generator and its tests, not on Miri.
6. A program linked on the vendored route without the loader of `slang-glslang-sys` still loads a `slang-glslang` shared library by name when the dynamic loader finds one (Section 3.10), and that library can come from another Slang build. The SPIR-V that such a program generates then depends on the environment that it runs in.
7. A C++ exception that escapes Slang is defined behaviour through the unwinding ABIs, but it runs the destructors of the caller's Rust frames. A direct user of this crate that does not stop the unwinding can call Slang again while Slang is in the middle of unwinding, and Slang's state can be inconsistent after it.

## 18. Operational Considerations

### 18.1. Cargo Features

`slang-compiler-sys` has one Cargo feature, `vendored`, off by default. It enables the optional dependency `slang-src` and selects the vendored route (SCS-50). Without it, the build script takes the dynamic route. `slang-bindgen` has no Cargo features.

### 18.2. Environment Variables

This table lists every environment variable that the build script or the generator reads, and every variable that the build script sets or removes for the programs that it runs, with its default. The variables that Cargo sets for every build script, such as `OUT_DIR`, `TARGET`, `HOST`, and the `CARGO_CFG_TARGET_*` variables, are not options, and the table lists those that the build script decides on. The variables of Cargo and git that the build script of `slang-src` reads or sets are in Section 15.2 of the `slang-src` design.

| Variable | Read by | Default and behaviour when absent |
| --- | --- | --- |
| `CARGO_FEATURE_VENDORED` | The build script, always | Set by Cargo when the feature `vendored` is on. Absent, so the dynamic route applies (SCS-50). |
| `VULKAN_SDK` | The build script on the dynamic route, the recipe `check-slang-call-guard`, and the variable `slang_library_path` of the `justfile` | No default, and the development container sets it. The vendored route never reads it, and `test-vendored` removes it (SCS-63). On the dynamic route, absence, an empty value, and a value that is not an absolute path in UTF-8 without line breaks stop the build (SCS-40 and Section 9.6), and without it or with it empty `slang_library_path` is empty, so the recipes add no library path (SCS-32). |
| `LIBCLANG_PATH` | `clang-sys`, when the generator runs in the build script, in the binary, or in its tests | Absent, so `clang-sys` searches the locations that its documentation lists. When libclang is not found there, the generator fails with an error of kind `Libclang` that advises to install libclang or set `LIBCLANG_PATH` (Section 7.7), and the build stops (Section 9.6). |
| `DEP_SLANG_SRC_ROOT`, `DEP_SLANG_SRC_COMMIT`, `DEP_SLANG_SRC_BUILD_TAG`, `DEP_SLANG_SRC_SHARED_DIR` | The build script on the vendored route | Set by Cargo from the metadata of `slang-src`. Absence, or a value that SCS-55 rejects, stops the build. The last names the shared folder, in which the build script keeps its entries (SCS-56). |
| `OUT_DIR` | The build script on both routes | Set by Cargo. The generated files are directly under it, and the vendored route keeps its copy, build tree, libraries, log, and stamp under `OUT_DIR/slang`. Absence, or a value that is not an absolute path in UTF-8 without line breaks, stops the build on both routes (Section 9.6, SCS-55). |
| `TARGET`, `HOST` | The build script on both routes | Set by Cargo. `TARGET` names the target of the generator on both routes, and a value outside the tested targets gets the warning of SCS-65. On the dynamic route, an absent, empty, or non-UTF-8 `TARGET` stops the build (Section 9.6). On the vendored route, absence of either stops the build, and so does a `TARGET` that differs from `HOST` (SCS-43, SCS-55). |
| `CARGO_CFG_TARGET_POINTER_WIDTH` | The build script on both routes | Set by Cargo for the target. A value other than `64` stops the build (SCS-66). |
| `CARGO_CFG_TARGET_ENV`, `CARGO_CFG_TARGET_FEATURE` | The build script on the vendored route | Set by Cargo for the target. The link rules of the vendored route need `gnu` on Linux and `msvc` on Windows (SCS-43). With the Microsoft toolchain, `crt-static` among the target features selects the static runtime library (Section 9.3). |
| `NUM_JOBS` | The build script on the vendored route | Set by Cargo. When it holds a whole number, the build step passes it to `cmake --build` as `--parallel`. Otherwise the build step passes no job count, and Ninja chooses one. |
| `PATH` | The operating system, when the build script starts `git`, `cmake`, `ninja`, and the program of the compiler probe on the vendored route | The build stops with a message that names the program when one of them cannot start (Section 9.6). With the Microsoft toolchain, the `cmake` runs get the `PATH` of the lookup of SCS-68 before the inherited value. |
| `VCINSTALLDIR` | `find-msvc-tools`, in the lookup of SCS-68, with the Microsoft toolchain on the vendored route | Absent, so the lookup searches the installations of Visual Studio. When set, it marks a developer command prompt, whose compiler the lookup takes first. The build script reruns when it changes (SCS-30), and names its value in the error of a failed lookup. |
| `VCToolsVersion` | `find-msvc-tools`, in the lookup of SCS-68, and the entry key | Absent, so the lookup takes the newest toolset of an installation. When set, the lookup takes only that toolset. The build script reruns when it changes (SCS-30). |
| `GIT_OPTIONAL_LOCKS` | Set to `0` by the build script for every git command on the vendored route | The value of the environment is never used (SCS-44). |
| `GIT_DIR`, `GIT_INDEX_FILE`, and the other 13 variables of the constant `GIT_REPOSITORY_VARIABLES` of `route.rs` | Removed by the build script from the environment of every git command on the vendored route | A value in the environment of the build, such as one that a git hook sets, never reaches git (SCS-44). |
| `SLANG_USE_SCCACHE` | Removed by the build script from the environment of `cmake` on the vendored route | A value in the environment never reaches Slang's CMake, and the configure step sets `-DSLANG_USE_SCCACHE=OFF`. |
| `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `CMAKE_TOOLCHAIN_FILE`, `MACOSX_DEPLOYMENT_TARGET` | CMake, when the vendored route configures, and the entry key. The build script also reads `CXX` for the compiler probe outside the Microsoft toolchain | Passed through unchanged. CMake chooses the compilers, flags, and toolchain from them. An absent or blank `CXX` makes the probe run `c++` (SCS-56). The build script reruns when one changes (SCS-30). |
| `CARGO_CFG_TARGET_OS` | The build script on both routes | Set by Cargo for the target. On the dynamic route, when it names the operating system of the host, the build script also checks the library file of the target, and when it is absent or names another or an unknown operating system, the build script does not look for the library file (SCS-41). On the vendored route, it selects the C++ standard library and the archive names, and a target other than Linux, macOS, or Windows has no link rule and stops the build (SCS-43). |

### 18.3. Platforms and Minimum Supported Rust Version

Section 6 lists the tested targets. Every other 64-bit target is allowed, untested, and unsupported, and gets a warning (SCS-65). The build script stops on a target whose pointer width is not 64 (SCS-66), and SCS-15 stops compilation there as well.

Every build of `slang-compiler-sys` runs the generator, so every user needs libclang at build time on every platform. The tests of the generator run on Linux only (SCS-21). Both crates use the workspace's `rust-version` (TOOL-4), which `check-msrv` verifies.

### 18.4. Failure Modes

This table lists the failure modes that this design defines, how each is reported, and the recovery.

| Failure | Reported by | Recovery |
| --- | --- | --- |
| `VULKAN_SDK` is absent, empty, not an absolute path in UTF-8 without line breaks, or names a directory without the Slang headers, without `lib`, or without the library file of the host's target | The error that the build script returns, with the content of Section 9.6 | Install the LunarG Vulkan SDK and set `VULKAN_SDK` to the absolute path of its directory that holds `include/slang` and `lib`, or enable `vendored` |
| libclang cannot be loaded | The error that the build script returns, with the error of kind `Libclang` (Section 9.6) | Install libclang or set `LIBCLANG_PATH` |
| libclang refuses the target triple | The error that the build script returns, with the error of kind `Target` and the tested targets (Section 9.6) | Build for a target that libclang accepts. The generator translates no triple (Section 7.7) |
| `TARGET` or `OUT_DIR` is absent or invalid on the dynamic route | The error that the build script returns (Section 9.6) | Build the crate through Cargo, with a target directory whose path is UTF-8 |
| The headers of the Slang that the build links hold a declaration that no rule covers, a construct of SCS-19, or no declaration that a rule names | The error that the build script returns, with every unmatched declaration, construct, or stale rule (Section 9.6) | Use the SDK that ships the recorded Slang, or enable `vendored`. An agent extends the rule set and this design for a new recorded Slang |
| The target is not a tested target | A warning of the build script that Cargo shows when the build fails, and `tested_target` set to `false` (SCS-65) | None needed. The build goes on, and a failure on such a target is not supported |
| The metadata of `slang-src` is missing or invalid | The error that the build script returns (Section 9.6) | Depend on the version of `slang-src` that this crate requires, as a direct dependency through the feature `vendored` |
| `git`, `cmake`, `ninja`, or the C++ compiler cannot start, or `git`, `cmake --version`, or the compiler probe fails | The error that the build script returns, with the command and its exit status or the reason (Section 9.6) | Install the missing program, or build without `vendored` |
| With the Microsoft toolchain, no `cl.exe` is found | The error that the build script returns, with where it looked and the values of `VCINSTALLDIR` and `VCToolsVersion` (SCS-68) | Install Visual Studio or its Build Tools with the workload for desktop development with C++, or build without `vendored` |
| The target differs from the host, or has no link rule for its C++ runtime library on the vendored route | The error that the build script returns, with the triples (Section 9.6) | Build on a host of the target, or build without `vendored` |
| The configure or build step of `cmake` fails | The error that the build script returns, with the path of `slang/build.log` under `OUT_DIR` and its last 40 lines (Section 9.6) | Read the log, and fix the toolchain or the environment that it names, such as `CC` |
| The vendored build produced no archive or another build tag | The error that the build script returns, with the path that is missing or the tag (Section 9.6) | Use the recorded commit. The route expects the layout that the build of the recorded commit produces, so another layout needs a change to the build script and this design |
| A file under `OUT_DIR` or the shared folder cannot be copied, listed, opened, locked, renamed, removed, or written | The error that the build script returns, with the action, the path, and the reason (Section 9.6) | Free disk space or fix the permissions of the target directory |
| A build that links for a target whose operating system differs from the host's, on the dynamic route | The linker, because the build script does not look for the library of a foreign target (SCS-41) | Build on a host of the target's operating system, with the SDK for that platform |
| A target whose pointer width is not 64 | The error that the build script returns (SCS-66), and the `compile_error!` of SCS-15 | Build for a 64-bit target |
| The shared library is not found at run time | The dynamic loader, which stops the program before `main`. On Linux, it prints "error while loading shared libraries", the file name `libslang-compiler.so.0.2026.17`, and "cannot open shared object file: No such file or directory", and the program exits with status 127 (Section 11) | Run the program through a recipe, which supplies the path, or put the SDK's library directory on the platform's library path as the crate documentation says (Sections 10 and 13) |
| The library at run time is not the Slang that the bindings were generated from | The smoke test (SCS-34), and the error of kind `VersionMismatch` of the safe crate | Make the loader find the library of the SDK that the build used |
| A program linked on the vendored route has no loader of `slang-glslang-sys` installed and finds no `slang-glslang` shared library at run time, so its SPIR-V is not optimized | Slang, only in the diagnostics of the first generation of SPIR-V in each global session, which begin `error[E00100]: failed to load downstream compiler 'spirv-opt'` while the call succeeds (Section 3.10) | Use `slang-compiler` with its feature `vendored`, which installs the loader, or install it as `slang-glslang-sys` documents |
| The process already holds a libclang instance | The generator, with an error of kind `Libclang` that says so (Section 7.7) | Call `generate` from one thread at a time. A later call works once the earlier one has returned (SCS-67) |
| A C function of the bindings is not exported by the library | The linker, when it links the test binary of `slang-compiler-sys` for the native-symbol test (Section 11) | Make the build use headers and a library of the same Slang |
| With the Microsoft toolchain, the link of a program on the vendored route reports an unresolved external symbol of the Windows API, such as `LNK2019: unresolved external symbol __imp_SHFileOperationW` | The linker of the program | The list of SCS-69 misses the import library that defines the symbol. An agent adds it to the constant of `route.rs`, or of `build_logic.rs` of `slang-glslang-sys` when an object of slang-glslang needs it, and to this design, and corrects SA-18 |

### 18.5. Logs

The build script emits no warning other than the one of SCS-65 on an untested target. Its only other output is the Cargo instructions and, on a failure, its error.

On the vendored route, it writes the output of every `cmake` run to `slang/build.log` under `OUT_DIR`, which it truncates at the start of each run that builds, so the log holds the output of the last such run only (SCS-48). The error of a failed configure or build step names the log and holds its last 40 lines (Section 9.6).

The generator's binary prints its errors to standard error. Neither crate emits logs at run time.

## 19. References

### 19.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 19.2. Informative References

- [SLANG-BUILD] The Slang project, "Building Slang From Source", `docs/building.md` at commit `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`, <https://github.com/shader-slang/slang/blob/c7954ebd1f1984ad2bfde15f76248db41dc34aa3/docs/building.md>.
- [SLANG-UG] The Slang project, "Slang User's Guide", `docs/user-guide/` at commit `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`, including the chapter `08-compiling.md`, <https://github.com/shader-slang/slang/tree/c7954ebd1f1984ad2bfde15f76248db41dc34aa3/docs/user-guide>.
- [SLANG-6344] Issue 6344 of the shader-slang/slang repository on GitHub, on the order of releasing the global session, <https://github.com/shader-slang/slang/issues/6344>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary. The labels SCS-5, SCS-14, SCS-16, SCS-17, SCS-18, SCS-20, SCS-22 to SCS-25, SCS-27, SCS-29, SCS-36, SCS-38, SCS-42, and SCS-49 belonged to requirements that this design removed, and they are not reused.

| Label | Section | Summary |
| --- | --- | --- |
| SCS-1 | 4.1 | Include all of `slang.h` except the exclusions of Section 4.3, and only the `spReflection*` part of `slang-deprecated.h` without `spReflection_GetSession`, and exclude the other headers. |
| SCS-2 | 4.1 | The rule set decides inclusion, an unmatched declaration fails the generator, and no setting relaxes this. |
| SCS-3 | 4.1 | No bindings for symbols exported only under mangled names or for inline C++ wrappers. |
| SCS-4 | 4.1 | Excluded types that included declarations use become opaque types, and every vtable slot stays. |
| SCS-6 | 5.2 | Types map by typedef name, never by canonical type. |
| SCS-7 | 5.3 | Named enumerations are `#[repr(transparent)]` integer newtypes, unnamed ones are free constants of their assigned typedef, and none is a Rust enum. |
| SCS-8 | 5.4 | Macro constants come from a synthetic translation unit and an allow-list, without platform macros, and `SlangResult` is an `i32` newtype with the result codes as its constants. |
| SCS-9 | 5.5 | Structs and unions are `#[repr(C)]`, with a `Default` that reproduces C++ default member initializers. |
| SCS-10 | 5.6 | Each interface has a struct, a vtable struct with the base first and `"system-unwind"` entries, a UUID through a trait, and thunks on raw pointers. |
| SCS-11 | 5.7 | C functions are in `unsafe extern "C-unwind"` blocks, and opaque types cannot be constructed. |
| SCS-12 | 5.8 | Compile-time assertions of size, alignment, and field offsets for every struct and union, for the target of the parse. |
| SCS-13 | 5.10 | No comment, documentation, or inner attribute in the generated code, one `#[expect]` on the module `bindings` with the lints that fire, and a second one for `ffi_unwind_calls`. |
| SCS-15 | 6 | The generated bindings start with a `compile_error!` for every target whose pointer width is not 64, which says that only 64-bit targets are supported. |
| SCS-19 | 7.3 | Every included construct that the generated Rust cannot express, as Section 7.3 lists, and every unmatched declaration fail for the target of the parse. |
| SCS-21 | 7.5 | Generator tests that load libclang sit in `ffi` modules and compile for Linux only. |
| SCS-26 | 9.2 | After its checks and the generation, the dynamic route links `slang-compiler` from `$VULKAN_SDK/lib`, without pkg-config, rpath, or other locations. |
| SCS-28 | 9.3 | The vendored route runs CMake through `Command` with Ninja, probed first, disables downloads, passes the build tag of `slang-src` and its numeric part, and links the six archives and the C++ standard library. |
| SCS-30 | 9.5 | Rerun instructions for `LIBCLANG_PATH`, for `VULKAN_SDK` and its header directory on the dynamic route, and for the variables that CMake reads, `VCINSTALLDIR`, `VCToolsVersion`, and the stamp file of the used entry on the vendored route. |
| SCS-31 | 9.6 | Each build failure returns an error with the content of the table. |
| SCS-32 | 10 | Each recipe that runs compiled code which loads Slang starts its command line with the SDK library path assignment of `slang_library_path`, which is never exported and is empty on Windows and without `VULKAN_SDK`. |
| SCS-33 | 11 | Tests that call native code sit in `ffi` modules, which `default-miri` excludes with `gpu` and `hw_gpu`. |
| SCS-34 | 11 | The smoke test asserts at run time, on both routes, that the loaded Slang reports the build tag of `BUILD_INFO`. |
| SCS-35 | 12 | The default CI jobs install and verify the SDK version that the CI configuration records, set the paths, install libclang, and check the SDK against the recorded Slang. |
| SCS-37 | 13 | The crate documentation has the content of the writing guide, both routes with their requirements, the best effort for another Slang, the shared safety contract with the behaviour of an escaping C++ exception, and Slang's rules. |
| SCS-39 | 4.1 | A rule that decides no declaration or macro fails the generator, with each stale rule and its reason. |
| SCS-40 | 9.2 | The dynamic route stops on a `VULKAN_SDK` that is absent, empty, not UTF-8, not absolute, or contains a line break. |
| SCS-41 | 9.2 | The dynamic route stops when `$VULKAN_SDK/lib` is not an accessible directory, and, for a target of the host's operating system only, when it lacks the target's library file. |
| SCS-43 | 9.3 | The vendored route refuses cross-compilation and every target without a link rule for its C++ runtime library, which exists for Linux with glibc, macOS, and Windows with MSVC. |
| SCS-44 | 9.3 | Every git command runs with `GIT_OPTIONAL_LOCKS=0` and without the 15 repository variables, so git answers for the source tree and writes nothing into it. |
| SCS-45 | 9.3 | The vendored route builds a copy of the tracked files of the source tree and of the eight required submodules inside `OUT_DIR`, refuses a path that leaves the tree, and removes stale files. |
| SCS-46 | 9.3 | The vendored route copies and configures again whenever the stamp differs, and writes the stamp only after a verified build. |
| SCS-47 | 9.3 | After the build, the six archives exist and the version header of the build tree defines the build tag of `slang-src`. |
| SCS-48 | 9.3 | The output of `cmake` goes to `slang/build.log`, and a failed step's error names the log and holds its last 40 lines. |
| SCS-50 | 9.1 | One Cargo feature, `vendored`, off by default, selects the vendored route, and nothing else selects a route or falls back. |
| SCS-51 | 5.1 | The build script writes `bindings.rs` and `symbols.rs` into `OUT_DIR` on every run, the crate includes them, and no generated file is in the repository or the package. |
| SCS-52 | 9.2 | The dynamic route reads `SLANG_TAG_VERSION` of the SDK and records it without comparing it with any version. |
| SCS-53 | 8.1 | `BUILD_INFO` records the build tag, the commit, the linkage, whether the target is tested, and the full version with build metadata, written by the build script and never derived from `cfg`. |
| SCS-54 | 9.5 | `links = "slang-compiler"`, and the metadata keys `include`, `lib`, `linkage`, and `build_tag` on both routes. |
| SCS-55 | 9.3 | The vendored route reads the tree, commit, build tag, and shared folder only from the metadata of `slang-src`, and checks them, `OUT_DIR`, `TARGET`, and `HOST`. |
| SCS-56 | 9.3 | The vendored route keeps and reuses its libraries and headers in entries of the shared folder of `slang-src`, keyed on the compiler and the configure arguments, and uses only a complete entry. |
| SCS-57 | 7.1 | The generator is the published crate `slang-bindgen` with a thin binary, on the `clang` crate with `runtime` and the `clang-sys` that it uses, without unsafe code. |
| SCS-58 | 7.2 | One parse for the target that the caller names, without a list of accepted targets, with `-nostdinc` and shim headers, and no error diagnostic accepted. |
| SCS-59 | 7.4 | The library returns the two texts and writes no file, starts no process, formats nothing, and writes no comment or inner attribute. |
| SCS-60 | 7.4 | The binary writes `bindings.rs` and `symbols.rs` for a header directory and a target through the same library call, for `check-slang-call-guard`. |
| SCS-61 | 14 | The crate depends only on `slang-bindgen` with an exact version and, under `vendored`, on `slang-src` with an exact version and `find-msvc-tools`. |
| SCS-62 | 12 | One CI job per operating system runs `test-vendored` in every run, without the SDK and from an empty target directory, and the default jobs never enable `vendored`. |
| SCS-63 | 9.3 | `test-vendored` runs the tests of the native crates and `slang-compiler` with `vendored` in `target/vendored`, without `VULKAN_SDK` and the loader path variables. |
| SCS-64 | 9.4 | On both routes, every run generates the bindings for `TARGET` from the headers of the linked Slang before any link instruction, and a generator failure stops the build. |
| SCS-65 | 6 | An untested target gets one warning that it is untested and unsupported and `tested_target` set to `false`, and the build does not stop for it. |
| SCS-66 | 6 | On both routes, a target whose pointer width is not 64 stops the build before the generation. |
| SCS-67 | 7.1 | The first call of `generate` keeps libclang loaded until the process exits, through a static handle of `clang-sys`, without unsafe code. |
| SCS-68 | 9.3 | With the Microsoft toolchain, the vendored route finds `cl.exe` with `find-msvc-tools`, gives its environment to the `cmake` runs only, and stops when it finds none. |
| SCS-69 | 9.3 | After the C++ standard library, the vendored route links the system libraries of its kind of target: `pthread` and `dl` on Linux, none on macOS, and CMake's ten default libraries with the Microsoft toolchain. |
