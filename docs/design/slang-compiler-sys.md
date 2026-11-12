# `slang-compiler-sys` and Binding Generator Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-11 |
| Applies to | `crates/slang-compiler-sys` and `tools/bindings-generator` |

## Abstract

This document specifies `slang-compiler-sys`, the crate of raw Rust bindings to the native Slang compiler library, and the binding generator that writes those bindings from the Slang headers. It records the facts about the Slang headers, the Vulkan SDK, and the shared library that a survey, a generator spike, and the implementation of the generator verified, and it builds the design on them. It defines which declarations the bindings include, the shape of the generated Rust for types, enumerations, constants, structs, COM-style interfaces, and C functions, and the restriction to 64-bit targets. It specifies how the generator parses the headers for several targets and fails on anything it has no rule for, how the generated bindings and the generated list of their C functions pass the repository's checks, and how a test keeps both checked-in files equal to a fresh regeneration. It defines the file that records the pinned Slang version, the build script with its dynamic route through an installed Vulkan SDK and its static route from a Slang source tree selected by an environment variable, the build for documentation without Slang, the message of each failure, and how the logic of the build script is structured and tested. It covers the tests, Miri, continuous integration, the crate documentation, the dependencies, the order of the changes that implement milestone M1, the assumptions still to confirm, and the open questions.

## 1. Introduction

### 1.1. Problem

Slang exposes its compiler through a C++ header, `slang.h`, whose main API consists of COM-style interfaces, which are C++ classes with only pure virtual methods. Rust cannot call a C++ virtual method directly. It can call one through a `#[repr(C)]` struct of function pointers that has exactly the layout of the C++ vtable. That struct copies a few hundred method signatures, in the right order and with the right calling convention. A single slot out of order calls the wrong function, and no compiler detects it. The headers also declare enumerations whose values the library can extend, structs whose C++ default values matter, and constants that exist only as macros. Hand-written declarations drift from the headers with every Slang release.

The architecture design (`docs/design/architecture.md`) places these declarations in `slang-compiler-sys`, requires that a generator in this repository produce all of them (ARCH-13), and requires this document to exist before any code of milestone M1 (ARCH-3, ARCH-25).

### 1.2. Purpose

`slang-compiler-sys` gives the safe crate `slang-compiler` an exact, complete, and checked set of declarations for the pinned Slang build, and, from step 4 of Section 15, links the native library on the route that the user's environment selects. The binding generator produces those declarations from the headers so that no agent writes a vtable, a UUID, or a thunk by hand (COM-9). This document implements Sections 3 to 6 and milestone M1 (Section 15.4) of the architecture design and answers the questions that its Section 17 assigns to this document.

### 1.3. Audience

The primary readers are the coding agents that implement M1 and the agents that later move the pin or change the generator. The secondary readers are the reviewers who check those changes, and the owner of the repository, who approves the decisions. A reader is expected to know Rust but not the Slang API, C++ application binary interfaces, or libclang. Section 2.1 defines the terms such a reader needs.

### 1.4. Scope

This document covers the declarations that the bindings include, the shape of the generated Rust, the supported targets, the binding generator and its tests, the file that records the pin, the build script of `slang-compiler-sys` on both routes and for documentation builds, how the shared library is found at run time, the tests of the crate, Miri, the CI changes that M1 makes, the crate documentation, the dependencies, and the order of implementation.

### 1.5. Non-Goals

This document does not cover:

- The safe API, which the design document of `slang-compiler` defines in M2, including ownership, errors, and the `Send` and `Sync` decisions of FFI-9 and CONC-1.
- 32-bit targets, which Section 6 excludes. A later change can revisit this decision.
- The deprecated compile-request API of `slang-deprecated.h`, including `slang::ICompileRequest`, which the modern interfaces replace (Section 4).
- `slang-gfx.h`, which declares a separate deprecated library, and the other headers that Section 3.2 lists as out of scope.
- Hand-written declarations of any kind in the generated file, which COM-9 and FFI-1 forbid.
- How continuous integration (CI) installs the Vulkan SDK on each runner, which Section 8.3 of `docs/design/tooling.md` specifies. Section 12 states the requirements that the installation meets.
- The recipes, the nextest profiles, and the CI workflow as a whole, which `docs/design/tooling.md` owns. This document states what M1 adds to them.

### 1.6. Current State

Every step of Section 15 is done, and milestone M1 is complete as of 2026-11-10. These parts of the design exist:

- `crates/slang-compiler-sys/pin.rs`, the pin file of Section 8.1, which the crate root includes and documents.
- The recipe `pin`, which prints the pin and runs on Linux, macOS, and Windows (Section 8.1).
- The installation of the pinned Vulkan SDK in every job of the CI workflow, with its check against the pin, through the recipes `install-vulkan-sdk` and `check-vulkan-sdk` (Section 12).
- `tools/bindings-generator`, the binding generator of Section 7, with its library, its thin binary, its rule set, its shim headers, its unit tests (Section 7.5), and its integration-test binary `tests/generator/`, which holds the regeneration test (Section 7.6).
- `crates/slang-compiler-sys/src/bindings.rs`, the generated bindings, which the generator's binary wrote in the repository, declared in `src/lib.rs` as the module `bindings` with its lint exception and re-exported at the crate root (Sections 5.1 and 5.10).
- The crate-level documentation of Section 13, which step 6 checked against SCS-37 and completed, with the shared safety contract, and the docs.rs targets of Section 9.4 in the manifest of the crate.
- The recipes `generate-bindings` and `check-bindings` (Section 7.6), and the entries of SCS-14 in `typos.toml` (Section 5.10).
- `crates/slang-compiler-sys/tests/sys/symbols.rs`, the second generated file, which lists every C function of the bindings for the native-symbol test (Section 5.1).
- The build script `crates/slang-compiler-sys/build.rs`, with its logic in `crates/slang-compiler-sys/route.rs` and its implementation of the real host in `crates/slang-compiler-sys/system.rs` (Section 9.7). It selects the route, builds the documentation without Slang, links the Slang of the Vulkan SDK on the dynamic route, and builds Slang from a source tree and links it statically on the static route (Sections 9.1 to 9.7). The manifest of the crate declares the key `links = "slang-compiler"` (Section 9.5).
- The integration-test binary `crates/slang-compiler-sys/tests/sys/`, whose module `pin` tests the constants of the pin, whose module `build_script` tests the logic of the build script and, in its module `ffi`, the real host, and whose module `ffi` holds the smoke test and the native-symbol test (Sections 9.7 and 11).
- The library path that the recipes which run compiled code place on the command line of that code (Section 10), the recipe `check-doc-without-slang` (Section 9.4), and the recipe `test-static`, which runs the tests of the crate on the static route (Section 9.3).
- The nextest profile `default-miri` of SCS-33, the licences of Section 14 in `deny.toml`, and the installation of libclang in the Linux jobs of the CI workflow (Section 12).

After M1, on 2026-11-11, step 3 of Section 15 of `docs/design/slang-compiler.md` changed the generated declarations. A test of that step observed a C++ exception that escaped from two methods of Slang, which is undefined behaviour through a non-unwinding ABI. So the generator now declares every vtable entry `extern "system-unwind"` and every C function `extern "C-unwind"`, while the type of every callback stays `extern "C"` (SCS-10, SCS-11). The step regenerated `src/bindings.rs`, added the lint exception for `ffi_unwind_calls` on `mod bindings;` and on the smoke test (Sections 5.10 and 11), and added a paragraph on C++ exceptions to the `# Safety` section of the crate documentation (Section 13).

By default, the crate links the Slang shared library of the Vulkan SDK that `VULKAN_SDK` names, after checking its build tag against the pin. Building the crate therefore needs the pinned SDK and `VULKAN_SDK`, unless `DOCS_RS` is present (Section 9.4) or `SLANG_SOURCE_DIR` names a clean git checkout of the pinned commit (Section 9.3). The static route was built and tested on x86_64 Linux only. Its code for macOS and for Windows with the Microsoft toolchain has never run (Section 17, SQ-3). What stays open after M1 is in the rows of Section 16, SA-7 and SA-8, and of Section 17, SQ-3 to SQ-5.

The facts in Sections 3.1 to 3.10 come from a survey of the Slang headers, the Vulkan SDK, and the Slang source tree in the development container, and from a spike that built a throwaway generator and a miniature of the bindings. Section 3.11 records the facts that the implementation of the generator established. Sections 3.6, 3.7, and 10 also record the facts about the SDKs for Windows and macOS that the CI runs of step 2 established, and Sections 6, 9.2, 10, and 11 record what the CI runs of step 4 established. Section 3.10 records the facts about Slang's build from source that the research for step 5 established, and Sections 9.3, 9.6, and 16 record what the implementation of step 5 observed.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that Section 2.2 of `docs/writing-documentation.md` defines, such as agent, role, and user, the terms that Section 2.2 of `docs/rust-coding-standards.md` defines, such as binding generator, `-sys` crate, safe crate, native library, interface, vtable, thunk, interface pointer, result code, and universally unique identifier (UUID), and the terms that Section 2.1 of `docs/design/architecture.md` defines, such as application binary interface (ABI), COM-style interface, CMake, dynamic route, static route, global session, libclang, Miri, pin, pkg-config, reflection, run-time search path (rpath), smoke test, spike, and Vulkan SDK. This document calls those three files the writing guide, the coding standard, and the architecture design, and calls `docs/design/tooling.md` the tooling design. It adds these terms:

- aggregate: A C++ struct, union, or class value, as opposed to a scalar or a pointer.
- build tag: The version string of one Slang build, such as `2026.17-25-gc7954ebd1`, which is the output of `git describe` on the Slang commit: the nearest tag, the number of commits after it, and `g` followed by the abbreviated commit hash.
- C linkage: The property of a C++ function declared inside `extern "C"`, whose symbol in the shared library is its plain name.
- calling convention: The rules for how a function receives its arguments and returns its result, such as which registers hold them and who removes them from the stack. Rust names a calling convention in the ABI string of a function type, such as `extern "C"` or `extern "system"`.
- canonical type: The type that remains after libclang resolves every typedef, such as `long` behind `int64_t` on Linux.
- clean checkout: A git work tree whose tracked files and submodules match its commit, as `git status` reports it with untracked files left out (Section 9.3).
- default member initializer: A value written in a C++ struct declaration for a field, such as `int size = sizeof(Desc);`, which the field takes when the struct is constructed without that field.
- diagnostic: A message that libclang reports while it parses, with a severity of note, warning, error, or fatal.
- gitlink: The entry of a git index that records a submodule by its commit, with the mode `160000`.
- host: The machine that runs the build script. In Section 9.7, also the private trait `Host` through which the logic of the build script reaches the environment, the file system, and other programs.
- import library: A file on Windows, with the suffix `.lib`, that tells the linker which symbols a dynamic-link library (DLL) exports.
- inclusion rule: An entry of the checked-in rule set of the generator that says whether a declaration, or a group of declarations selected by name or by header, is included in or excluded from the bindings (Section 4.1).
- model: The generator's in-memory description of the declarations that it reads from one parse, such as the interfaces with their methods in order, before it writes any Rust.
- name mangling: The encoding of a C++ function's namespace, class, and parameter types into its symbol name. The encoding differs between compilers.
- opaque handle: A pointer to a type whose contents the header does not declare, so that code can pass the pointer but not read the object.
- shim header: A small header in this repository that replaces a system header, such as `stdint.h`, when the generator parses for a target whose system headers the development container does not have (Section 7.2).
- stamp: The file `slang/stamp` under `OUT_DIR` that records the inputs of the last static build that the build script verified (Section 9.3).
- standard layout: The C++ property of a type whose memory layout follows the same rules as a C struct, which `#[repr(C)]` reproduces in Rust.
- synthetic translation unit: A translation unit that the generator writes in memory and parses to evaluate an expression, such as the value of a macro (Section 7.2).
- target: The platform for which code is compiled, named by a target triple such as `x86_64-pc-windows-msvc`.
- translation unit: One C or C++ source file together with every header that it includes, which a compiler or libclang parses as one unit.
- typedef: A C or C++ declaration that gives an existing type a new name, such as `typedef int32_t SlangResult;`.
- work tree: The directory of files that a git repository checks out. Its top level is the directory that holds `.git`.

## 3. The Slang API

This section is non-normative. It states the facts about the Slang headers, the Vulkan SDK, and the Slang source tree on which the later sections rely, and those sections cite it instead of repeating them.

### 3.1. Sources and Tool Versions

Every fact in this section was verified in the development container on 2026-11-10, with the Vulkan SDK 1.4.363.1, the Slang source tree at `/opt/slang` at commit `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`, clang and libclang 19.1.7, the Rust crate `clang` 2.1.0, CMake 3.31.6, Ninja 1.12.1, and the stable and nightly Rust toolchains that Section 1.1 of the coding standard records. Statements about Slang's documented rules come from the documents in that source tree [SLANG-UG] [SLANG-BUILD]. Section 16 lists what was not verified.

### 3.2. Headers

The SDK directory `$VULKAN_SDK/include/slang` holds 15 headers. Only `slang.h`, of 6,227 lines, declares the host API. It includes `slang-deprecated.h`, of 1,715 lines, and the file `slang-image-format-defs.h`, which expands inside the declaration of `enum SlangImageFormat`. `slang-gfx.h` declares a separate, deprecated library. The prelude headers serve generated shader code, not the host API. The SDK's `slang.h`, `slang-deprecated.h`, and `slang-image-format-defs.h` are byte-identical to those of the source tree. The SDK also has `slang-tag-version.h`, which the source tree has only as the template `slang-tag-version.h.in`.

`slang.h` is C++ only, and a parse in C mode fails. It parses with clang 19 at `-std=c++17` without a diagnostic, and it includes no header of the C++ standard library, only `stdint.h`, `stddef.h`, `inttypes.h`, and `features.h`.

### 3.3. COM-Style Interfaces

The headers declare 28 interfaces with 231 own methods in total: 27 interfaces in `slang.h` and `slang::ICompileRequest`, with 81 methods, in `slang-deprecated.h`. Every interface derives from `ISlangUnknown` through single inheritance. `ISlangUnknown` declares `queryInterface`, `addRef`, and `release`, and it is binary-compatible with the `IUnknown` of COM, though Slang needs no COM runtime [SLANG-UG]. No interface overloads a method name, declares a method that is not pure virtual, declares a virtual destructor, or has a data member. Each vtable is therefore the vtable of the base interface followed by the interface's own methods in declaration order. The largest interfaces of the modern API are `slang::IGlobalSession` with 30 own methods, `slang::ISession` with 21, `slang::IComponentType` with 14, and `slang::IModule` with 13.

The interfaces whose names start with `ISlang` are at global scope inside an `extern "C"` block, and the others are in `namespace slang`. `slang::IBlob` is a typedef of `ISlangBlob`.

Each interface declares its UUID with the macro `SLANG_COM_INTERFACE`, which expands to a `static constexpr` member function `getTypeGuid()` that returns an initializer list of integer literals, so the UUID can be read from the syntax tree that libclang returns. `SlangUUID` is a struct of a `uint32_t`, two `uint16_t`, and an array of eight `uint8_t`. The struct `SlangTerminatedChars` also declares a UUID but has no virtual method and no base, so an interface is a class that has the UUID function and has a virtual method or a base.

Methods use the calling convention macro `SLANG_MCALL`, which is `__stdcall` on the Microsoft compilers and empty elsewhere, and is `extern "system"` in Rust, or `extern "system-unwind"` for a function through which an exception may unwind. `SLANG_NO_THROW` is `__declspec(nothrow)` on Windows and empty elsewhere. It does not keep every exception inside Slang: on 2026-11-11, the tests of `slang-compiler` observed a C++ exception that escaped from `IModule::findAndCheckEntryPoint` and from `IComponentType::getLayout` on Linux, Windows, and macOS (Section 3.6 of `docs/design/slang-compiler.md`). On 2026-11-10, one method lacks `SLANG_MCALL`: `slang::IMetadata::isParameterLocationUsed`, at line 4758 of `slang.h`.

Twenty-two methods declare default arguments, which are not part of the ABI. Some methods take `SlangUUID const&`, which is a pointer in the ABI. Two methods of `slang::IGlobalSession` are marked `[[deprecated]]` and still occupy vtable slots. Slang's user guide states that interfaces whose names end in `_Experimental` may change between releases [SLANG-UG].

No method and no C function passes or returns an aggregate by value. Thirteen parameters are C++ references, including in two C functions, of the types `const SlangUUID&`, `const SessionDesc&`, `bool&`, `SlangInt&`, and `const char*&`. Twenty-five parameters or return values are `bool`. Five parameter types are callbacks, which are function pointers in the plain C calling convention without `SLANG_MCALL`. No function is variadic.

A Rust miniature of the bindings, with `#[repr(C)]` vtable structs in the slot order of the model, `extern "system"` entries, and a build script that emits the library search path and `rustc-link-lib=dylib=slang-compiler`, created a global session and called `getBuildTagString` through vtable slot 8, which is the three methods of `ISlangUnknown` followed by own method index 5, and received `2026.17-25-gc7954ebd1`. `queryInterface` returned the same object for the UUIDs of `ISlangUnknown` and `slang::IGlobalSession`, and `SLANG_E_NO_INTERFACE`, 0x80004002, for that of `ISlangBlob`. The reference count went from 2 to 1 to 0 on release.

### 3.4. C Functions and Reflection

The headers declare 278 functions with C linkage, and `libslang-compiler.so` exports all of them. Of these, 21 are in `slang.h`: 20 named `slang_*` and `spGetBuildTagString`, at line 2092, which needs no session. The other 257 are in `slang-deprecated.h`, and 172 of them are named `spReflection*`. A global session is created with `slang_createGlobalSession2`, at line 5994, which takes a `const SlangGlobalSessionDesc*` and writes a `slang::IGlobalSession*`. `slang_shutdown`, at line 6023, and `slang_getLastInternalErrorMessage`, at line 6078, are also C functions. `SLANG_API_VERSION` is 0.

Two functions have C++ linkage and are exported only under mangled names, `slang_getEmbeddedCoreModule` and `spReflection_GetSession`, and the library also exports one member function defined out of line, `slang::VariableReflection::getDefaultValueBlob`.

Reflection has no COM-style interface. The header declares opaque handles, such as `SlangReflectionType`, at lines 2102 to 2132 of `slang.h`, and the `spReflection*` functions of `slang-deprecated.h` operate on them. The C++ reflection classes of `slang.h`, such as `slang::TypeReflection`, have 201 inline methods, and each casts `this` to an opaque handle and calls an `spReflection*` function. The `spReflection*` functions are therefore the ABI of reflection, although the header that declares them calls itself deprecated. Reflection objects are not reference counted.

### 3.5. Types, Enumerations, and Constants

`SlangResult` is a typedef of `int32_t`, binary-compatible with the `HRESULT` of COM, in which a negative value means failure. The result codes exist only as macros, built with the function-like macro `SLANG_MAKE_ERROR`. `SlangBool` is `bool`. `SlangInt` and `SlangUInt` are 64-bit on 64-bit targets and 32-bit otherwise.

libclang reports 64 enumerations with 703 enumerators, counting the six enumerations nested in records and the enumerators that `slang-image-format-defs.h` adds. Of these, 14 are `enum class`. Most unscoped enumerations use the pattern `typedef int XIntegral; enum X : XIntegral`, and nine have no fixed underlying type. Flag types are integer typedefs whose values are enumerators of the form `1 << n`.

The headers declare 36 structs and 3 unions. None has a user-declared constructor, every named record is trivially copyable and standard layout, and 13 have default member initializers. Several descriptor structs start with a size field whose default is the size of the struct: `structureSize` evaluates to 80 for `SlangGlobalSessionDesc`, 48 for `TargetDesc`, and 96 for `SessionDesc`. A `flags` field evaluates to 1024. The initializers `nullptr` and `{}` cannot be evaluated through libclang, and they mean a null pointer and zero. On x86_64, `SlangUUID` has size 16 and alignment 4, `SlangGlobalSessionDesc` 80 and 4, `TargetDesc` 48 and 8, `SessionDesc` 96 and 8, and `CompilerOptionEntry` 40 and 8. The headers also declare three `inline constexpr` constants, such as `kDefaultTargetFlags`.

The headers define 121 object-like macros. libclang does not evaluate a macro definition, but a synthetic translation unit that includes `slang.h` and declares one `constexpr auto` variable for each macro evaluates them, in about 17 ms. The values include `SLANG_OK` 0, `SLANG_FAIL` 0x80004005, `SLANG_E_NOT_IMPLEMENTED` 0x80004001, `SLANG_E_NO_INTERFACE` 0x80004002, `SLANG_E_INVALID_ARG` 0x80070057, and `SLANG_E_NOT_FOUND` -2113929211. Platform macros, such as `SLANG_LINUX`, take values that depend on the target.

### 3.6. Version Information

`$VULKAN_SDK/include/slang/slang-tag-version.h` defines `SLANG_TAG_VERSION` as `"2026.17-25-gc7954ebd1"` and `SLANG_VERSION_NUMERIC` as `"2026.17"`. The pkg-config file `$VULKAN_SDK/lib/pkgconfig/slang-compiler.pc` gives only `Version: 2026.17` and states that `pkg-config --static` is not supported. At run time, `spGetBuildTagString()` and `slang::IGlobalSession::getBuildTagString()` return `2026.17-25-gc7954ebd1`. The Slang headers do not record the version of the Vulkan SDK, which appears only outside `$VULKAN_SDK`, such as in the SDK's `README.txt` and in the pkg-config file of the Vulkan loader.

On the CI runners, on 2026-11-10, the SDKs 1.4.363.1 for Linux, macOS, and Windows ship the same Slang build. In each, `$VULKAN_SDK/include/slang/slang-tag-version.h` defines `SLANG_TAG_VERSION` as `"2026.17-25-gc7954ebd1"`, and `slangc -version` prints `2026.17-25-gc7954ebd1`, as `check-vulkan-sdk` verified (Section 12).

In the source tree, `HEAD` is `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`, which also carries the tag `vulkan-sdk-1.4.363.0`, and `git describe --tags --match 'v20[2-9][0-9].[0-9]*'` prints `v2026.17-25-gc7954ebd1`. Slang's CMake derives its version only from `git describe`, in `cmake/GitVersion.cmake`, accepts the override `-DSLANG_VERSION_FULL=`, and falls back to `0.0.0-unknown`.

### 3.7. Shared Library

`$VULKAN_SDK/lib/libslang-compiler.so` is a symbolic link to `libslang-compiler.so.0.2026.17`, whose shared object name (SONAME) is the same. It is 34 MB and depends only on libstdc++, libm, libgcc_s, and libc. `libslang.so` is a compatibility link to it. At run time, the library loads `libslang-glslang-2026.17.so` and `libslang-glsl-module-2026.17.so` by name from its own directory. They are not link-time dependencies.

Slang's building documentation states that "the public ABI for Slang libraries in general is not currently stable", which is why the major version of the shared library is 0 [SLANG-BUILD]. This confirms assumption A-1 of the architecture design.

A C++ program linked with `-L$VULKAN_SDK/lib -lslang-compiler` fails to start without `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH`, with the loader's message "cannot open shared object file", and with it prints the build tag and creates and releases a global session. An rpath works as well. `slangc` has the RUNPATH `$ORIGIN/../lib:$ORIGIN`, so a `slangc` that runs does not show that the loader path is set.

The CI runs of step 2 of Section 15 observed these facts about the library of the SDKs for macOS and Windows on the hosted runners on 2026-11-10:

- macOS: `VULKAN_SDK` is the directory `macOS` under the installation root, and it holds `bin`, `Frameworks`, `include`, `lib`, `share`, and `source`. `$VULKAN_SDK/lib/libslang-compiler.dylib` is a symbolic link to `libslang-compiler.0.2026.17.dylib`, a universal binary for x86_64 and arm64, whose install name is `@rpath/libslang-compiler.0.2026.17.dylib`. `slangc` has the rpaths `@loader_path/../lib` and `@loader_path`.
- Windows: `VULKAN_SDK` is the installation root, and its directories are named `Include`, `Lib`, and `Bin`. The file system is case-insensitive, so `include/slang` and `lib` resolve to them. `Lib\slang-compiler.lib` is the import library. `Bin` holds `slang-compiler.dll`, `slang-glslang.dll`, `slang-glsl-module.dll`, `slang-rt.dll`, `slang.dll`, and `slangc.exe`.

### 3.8. Differences Between Targets

libclang returns a translation unit even after a fatal diagnostic, such as a missing system header, and the model is then wrong without any other sign: methods are missing and types degrade to `int`. With `-nostdinc` and shim headers for `stdint.h`, `stddef.h`, `inttypes.h`, `features.h`, and `TargetConditionals.h`, built on the compiler's predefined macros, the parse for Linux is byte-identical to the parse with the system headers, and the parses for `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, and `i686-pc-windows-msvc` report no diagnostic.

On all four targets, the interface names, the bases, and the method order are identical. On x86_64 Windows and on macOS, the canonical type `long` becomes `long long` behind the same typedef names. The nine enumerations without a fixed underlying type are `unsigned int` on Linux and `int` on Windows, with the same size. Every struct layout is identical on the three 64-bit targets. On `i686-pc-windows-msvc`, 230 methods use `__stdcall`, `slang::IMetadata::isParameterLocationUsed` uses `__thiscall` because it lacks `SLANG_MCALL` (Section 3.3), and the layout of every struct that holds a pointer differs.

One parse and the extraction of its model take about 90 ms. A whole run of the generator takes longer (Section 3.11).

### 3.9. Lifetime and Threading Rules

Slang documents these rules, which the safe crate has to respect and which the crate documentation of `slang-compiler-sys` states (Section 13):

- A global session is not thread-safe. A program that compiles on several threads at once uses one global session per thread. Creating a global session and calling `spGetBuildTagString` are thread-safe [SLANG-UG].
- A global session should be released after every object created from it [SLANG-6344].
- Reference counting is not guaranteed to be atomic on every interface, so code does not add or release references to one object from unrelated threads [SLANG-UG].
- A `ProgramLayout` is not reference counted and lives as long as the `slang::IComponentType` that it came from [SLANG-UG].

### 3.10. Source Build

The setting `SLANG_LIB_TYPE=STATIC` produces `libslang-compiler.a`. Slang's building documentation lists the libraries that a consumer of the static library also links: `libcompiler-core.a`, `libcore.a`, `libminiz.a`, and `liblz4.a` [SLANG-BUILD]. This confirms assumption A-3 of the architecture design as far as the documentation goes. The documented list is incomplete.

A build needs CMake 3.22 or newer, or 3.25 for presets, a C++17 compiler, and Ninja. Slang's documentation lists Python 3 only for spirv-tools, which the minimal build of Section 9.3 does not build. In the source tree at `/opt/slang`, all 21 git submodules are checked out, 224 MB under `external/`. The library `slang` has about 388 hand-written translation units and some generated ones.

The research for step 5 of Section 15 built a minimal static Slang from `/opt/slang` in the development container on 2026-11-10, with GCC 14.2.0, CMake 3.31.6, and Ninja 1.12.1 on 16 logical cores, and established these facts:

- A configure and build of the CMake target `slang` with the options of Section 9.3 ran with no network access, in a network namespace without interfaces, and took about 3 min 48 s and 46 min of processor time. No line of its log mentions a download. The build tree took 798 MB.
- With the default options, a configure without network access tries to download a DirectX Shader Compiler (DXC) binary and then to clone DXC. `SLANG_ENABLE_DXIL=OFF` and `SLANG_SLANG_LLVM_FLAVOR=DISABLE` stop every download of the tested configuration. CMake reported `FETCHCONTENT_FULLY_DISCONNECTED=ON` as unused there.
- `-DSLANG_VERSION_FULL=` alone breaks the configure step, with "set_target_properties called with incorrect number of arguments", because Slang's CMake then leaves `SLANG_VERSION_NUMERIC` empty. `-DSLANG_VERSION_NUMERIC=` with the numeric version fixes it. The numeric version is also part of the names of the modules that the library loads at run time.
- Slang's build writes into its source tree. It rewrites one tracked documentation file with the same content, and it creates and removes temporary files next to its `*.meta.slang` inputs in `source/slang`. With the source tree mounted read-only, the build failed at step 189 of 486 with "unable to open file for writing".
- The link needs six archives, in this order: `libslang-compiler.a`, `libcompiler-core.a`, `libcore.a`, `libcmark-gfm.a`, `libminiz.a`, and `liblz4.a`, followed by the C++ standard library. Without `libcmark-gfm.a`, the link fails with 11 undefined references. `-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=` puts all six in one directory.
- `cmake --install` is unusable for the minimal build. It fails, and when forced it installs only `libslang-compiler.a`, which does not link alone, so a consumer links from the build tree.
- The configuration needs 8 of the 21 submodules: `external/cmark`, `external/fast_float`, `external/lua`, `external/lz4`, `external/miniz`, `external/spirv-headers`, `external/unordered_dense`, and `external/vulkan`. A missing one fails at configure time with a CMake error about a missing `CMakeLists.txt`.
- The static library has no slang-glslang, which is always a separate shared module that Slang loads by name. Section 16 records what this changes. The optimization of SPIR-V is the difference that matters by default, because the pinned build validates SPIR-V only when the environment variable `SLANG_RUN_SPIRV_VALIDATION` is `1`, on either route (Section 16). When the dynamic loader finds no `slang-glslang` shared library, the first generation of SPIR-V in each global session succeeds and returns diagnostics that begin `error[E00100]: failed to load downstream compiler 'spirv-opt'`. The later generations in the same global session succeed with no diagnostics. The SPIR-V is not optimized, and it is not validated even when `SLANG_RUN_SPIRV_VALIDATION` is `1`. When the loader finds a `slang-glslang` shared library, the static build behaves as the dynamic route, so the output depends on the environment. A run on 2026-11-11 on x86_64 Linux observed this (Section 16).

### 3.11. Findings of the Generator

The implementation of the generator in step 1 of Section 15 established these facts on 2026-11-10, on the headers of the pin, with the tool versions of Section 3.1.

Four enumerations of `slang.h` have no name. Each one holds the values of a flag type and follows the integer typedef of that type in the header. `SlangDiagnosticFlags` is a typedef of `int`, `SlangCompileFlags` and `SlangTargetFlags` are typedefs of `unsigned int`, and `slang::SessionFlags` is a typedef of `uint32_t`. Together the four enumerations have 13 enumerators. An unnamed enumeration has no name for a newtype, so Section 4.4 lists each one with its typedef.

Two declarations of `slang.h` cannot be expressed as Rust declarations. `slang::VMExecOperand` has bit-fields, and only inline C++ helpers use it. `slang::CompileCoreModuleFlag` is an empty struct, which has size 1 in C++ and would have size 0 as a Rust struct, and it only scopes its nested enumeration `slang::CompileCoreModuleFlag::Enum`. Section 4.3 excludes both and includes the nested enumeration.

libclang reports each `static_assert` of `slang.h` as a declaration, so the rule set has to decide it. It reports the function-like macro `SLANG_FORMAT`, which writes the enumerators of `SlangImageFormat`, as object-like, so the rule set has to decide that macro as well. When it parses for `x86_64-pc-windows-msvc`, clang declares `struct _GUID` implicitly, so the generator ignores forward declarations of records nested in records. `spReflection_GetSession` has C++ linkage (Section 3.4) and matches the pattern `spReflection*`, so its exclusion has to come before that pattern in the rule set.

The different signedness of an enumeration without a fixed underlying type (Section 3.8) is harmless only while every enumerator fits in `int`. An enumerator such as 0x80000000 would be negative on Windows and positive on Linux, and the generator reports it as a difference between targets (Section 7.3). No enumeration of the pinned headers has such a value.

A whole run of the generator, which consists of three parses, three synthetic translation units, the writing of Rust, and rustfmt, takes about 190 ms, against the 90 ms of one parse in Section 3.8.

On the headers of the pin, in a scratch crate outside the repository, the generator wrote a file of 6,299 lines that holds no absolute path, and three runs wrote byte-identical files. The file holds:

- 27 interfaces with 150 vtable slots and 290 thunks for their own and inherited methods, and `slang::ICompileRequest` as an opaque type.
- 57 `#[repr(transparent)]` newtypes, which are 56 enumerations with 632 enumerators as associated constants, and `SlangResult` with 18 result codes.
- The 13 enumerators of the four unnamed enumerations, as free constants.
- 20 structs and 3 unions, with 23 blocks of layout assertions and 13 implementations of `Default`. `SlangGlobalSessionDesc` defaults to `structureSize` 80, `apiVersion` 0, and `minLanguageVersion` 2025.
- 193 C functions, which are the 21 of `slang.h` and the 172 `spReflection*` functions of `slang-deprecated.h`.
- 3 constants from `inline constexpr` declarations, 21 opaque types, and 61 type aliases.

These counts agree with Sections 3.3 to 3.5. The 28 interfaces with 231 own methods of Section 3.3, without `slang::ICompileRequest` and its 81 methods, are the 27 interfaces with 150 slots. The 193 functions are those of Section 3.4 that the rule set includes. The 13 implementations of `Default` are the 13 records with default member initializers of Section 3.5. Of the 64 enumerations with 703 enumerators of Section 3.5, the file holds 60 with 645 enumerators, and the other 4 with 58 enumerators belong to declarations that the rule set excludes. Of the 36 structs of Section 3.5, the other 16 are excluded.

The scratch crate passes Clippy with warnings denied for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, and `aarch64-apple-darwin`, and every layout assertion holds on all three. For `i686-unknown-linux-gnu`, compilation stops with the message of SCS-15. A program that used only the generated bindings created a global session, read `2026.17-25-gc7954ebd1` through the thunk of `slang::IGlobalSession::getBuildTagString` and through `spGetBuildTagString`, received the same object from `queryInterface` for the UUID of `ISlangUnknown` and `SLANG_E_NO_INTERFACE` for that of `ISlangBlob`, and saw the reference count go from 2 to 1 to 0. Section 5.10 lists the lints that fire on the file.

## 4. Scope of the Bindings

### 4.1. Inclusion Rules

SCS-1. The binding generator MUST include every declaration of `slang.h` except the exclusions that Section 4.3 records with their reasons, MUST include from `slang-deprecated.h` only the declarations that match `spReflection*`, except `spReflection_GetSession`, and MUST exclude the rest of `slang-deprecated.h`, including `slang::ICompileRequest`, as well as `slang-gfx.h`, `slang-llvm.h`, and the prelude headers, because the bindings would otherwise miss the reflection ABI (Section 3.4), expose a deprecated API that the safe crate must not use, or declare items that the library does not export or that Rust cannot express. A new exclusion from `slang.h` is a change to Section 4.3 and to the rule set in the same change.

SCS-2. The binding generator MUST decide inclusion and exclusion through a checked-in rule set and MUST fail when a declaration that it reads matches no rule, because otherwise a new Slang header can add or drop API in a regeneration without any reviewer seeing a decision.

SCS-3. The binding generator MUST NOT emit a declaration for a function that the library exports only under a mangled name, which on 2026-11-10 are `slang_getEmbeddedCoreModule`, `spReflection_GetSession`, and `slang::VariableReflection::getDefaultValueBlob`, or for the inline C++ wrapper classes and methods of `slang.h`, because a mangled name differs between compilers and is no stable link name, and an inline wrapper is not part of the ABI, so a declaration of either fails to link or calls nothing.

SCS-4. When an included declaration refers to a type that a rule excludes, such as a parameter of type `slang::ICompileRequest**`, the binding generator MUST emit that type as an opaque type (Section 5.7) and MUST keep every vtable slot of every included interface, including the slots of deprecated methods, because a vtable with a missing slot calls the wrong function for every later method.

SCS-39. The binding generator MUST fail with an error that names each rule and its reason when a rule of the rule set decides no declaration or macro of the headers, including a rule that an earlier rule always shadows and a rule of Section 4.4 that matches no unnamed enumeration, because a rule that decides nothing records a decision about a declaration that the headers no longer have, and a reviewer who reads the rule set then believes that the bindings cover it.

The rule set is the constant `SLANG` in `tools/bindings-generator/src/rules.rs`, a table of Rust values whose types in the same file are the authoritative definition of its format. Unit tests cover it (Section 7.5). It has three lists of rules, and every rule carries a reason that the generator's error messages quote:

- Declaration rules: each names an optional header, a pattern that matches the qualified C++ name of a declaration, such as `slang::TypeReflection::*`, with `*` matching any text, and whether the declaration is included or excluded. A declaration takes the first rule that matches it.
- Macro rules: each names a pattern for an object-like macro and whether the macro becomes a result code, becomes a constant of a given Rust integer type, or is excluded (Section 5.4). A macro takes the first rule that matches it.
- Unnamed enumeration rules (Section 4.4).

The rule set also names the result type `SlangResult`, the UUID type `SlangUUID`, and the typedefs that map to `isize` and `usize` (Section 5.2). An anonymous struct or union nested in a record takes the decision of the outermost record that contains it, and a named type nested in a record matches the rules by its qualified name.

### 4.2. Rationale

This subsection is non-normative. The bindings take all of `slang.h` because the safe crate of M2 decides what it wraps, and a `-sys` crate that omits a declaration forces a regeneration for every later need. The `spReflection*` functions are the only route to reflection (Section 3.4), so they are included despite the name of their header, while the compile-request API in the same header is replaced by the modern interfaces and is left out, as Section 1.5 states. A rule set that fails on an unmatched declaration turns every new header declaration into a reviewed decision. The three symbols with mangled names are left out because Rust would have to hard-code a mangling that only one compiler produces.

A rule set that fails on a rule that decides nothing (SCS-39) keeps the rule set equal to the headers in the other direction. When a pin move removes a declaration, the generator names the rule that still mentions it, so the rule set never records a decision that no longer applies. Two such rules were found by hand during step 1 of Section 15, before SCS-39 existed, and removed.

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

An unnamed enumeration has no name that a rule can match, so the rule set identifies it by the header and one of its enumerators, and assigns it the integer typedef whose values it holds. The generator emits its enumerators as constants of that typedef (SCS-7). An unnamed enumeration that no rule assigns is a declaration that matches no rule (SCS-2). The generator also fails when the assigned typedef is not an included typedef of an integer type, and when an enumerator does not fit that typedef, because the constant would then have a type that cannot hold its value.

This table lists the rules for the four unnamed enumerations of `slang.h` (Section 3.11):

| Enumerator that identifies the enumeration | Typedef of its constants |
| --- | --- |
| `SLANG_DIAGNOSTIC_FLAG_VERBOSE_PATHS` | `SlangDiagnosticFlags` |
| `SLANG_COMPILE_FLAG_NO_MANGLING` | `SlangCompileFlags` |
| `SLANG_TARGET_FLAG_GENERATE_SPIRV_DIRECTLY` | `SlangTargetFlags` |
| `kSessionFlags_None` | `slang::SessionFlags` |

## 5. Generated Rust

### 5.1. File and Module

SCS-5. The binding generator MUST write the bindings into the one file `crates/slang-compiler-sys/src/bindings.rs` and the list of their C functions into the one file `crates/slang-compiler-sys/tests/sys/symbols.rs`. It MUST format both with rustfmt and write them from the same parse. The crate MUST declare the first file as the private module `bindings` and re-export its items at the crate root with `pub use bindings::*;`. The crate MUST declare the second file as the module `symbols` of its integration-test binary. The regeneration check (Section 7.6) compares exactly these two files, and `check-fmt` fails on a checked-in file that rustfmt would change. A list of the C functions written by hand would be a second copy of generated names that diverges at every regeneration.

Each generated item then has exactly one public path, at the crate root, as API-10 requires. The glob re-export is the one that API-10 allows a `-sys` crate for its private generated module, because a hand-written list of every generated name would be a second copy of the generated names that diverges at every regeneration.

The file `crates/slang-compiler-sys/src/lib.rs` holds, in this order, the crate-level documentation (Section 13), the opt-out `#![expect(unsafe_code, reason = "...")]` (Section 5.10), the declaration `mod bindings;` with its lint exception (Section 5.10), the re-export `pub use bindings::*;`, and the inclusion `include!("../pin.rs");` of the pin (Section 8.1). It holds no other code (FFI-1). The check of the pointer width (Section 6) is not in `lib.rs`: the generator writes it at the top of the generated file, directly after the `//!` line, so it travels with the declarations whose layouts it protects.

The checked-in file is the output of the generator's binary on the headers of the pin, 6,299 lines long. On 2026-11-10, two runs of `just generate-bindings` wrote byte-identical files and left the working tree unchanged, and `cargo package --list -p slang-compiler-sys` listed `src/bindings.rs` and `pin.rs`, so the packaged crate carries both.

The second generated file, `crates/slang-compiler-sys/tests/sys/symbols.rs`, holds a one-line `//!` notice that the file is generated and names the recipe `generate-bindings`, and the constant `pub(super) const C_FUNCTIONS: &[(&str, *const ())]`, which pairs the name of every C function that the bindings declare with its address, in declaration order. On 2026-11-10, the file is 783 lines long and lists the 193 C functions of Section 3.11. The native-symbol test reads it (Section 11). The file is never edited by hand, and the regeneration test fails on any difference (Section 7.6). On 2026-11-10, `cargo package --list -p slang-compiler-sys` listed it with `build.rs`, `route.rs`, and the other tests, because the packaged tests need it.

### 5.2. Type Mapping

SCS-6. The binding generator MUST map each type by its typedef name and never by its canonical type, mapping `SlangInt` and `SlangSSizeT` to `isize`, `SlangUInt`, `SlangSizeT`, and `size_t` to `usize`, each `intN_t` to `iN`, each `uintN_t` to `uN`, and the C types `char`, `int`, and the other basic types to the types of `core::ffi`, because the canonical type differs between targets (Section 3.8), and a mapping from it would write a file that is correct on one target only.

Each integer typedef of the headers becomes a Rust type alias, except `SlangResult`, which becomes the newtype of Section 5.4. A C++ reference parameter becomes `*const T` for a `const` reference and `*mut T` otherwise. `bool` maps to `bool`. A data pointer becomes `*const T` or `*mut T` by its `const` qualifier. A callback type becomes `Option<unsafe extern "C" fn(...)>`, because a Rust function pointer cannot be null. A callback type keeps the non-unwinding ABI `"C"`, unlike the vtable entries and the C functions (Sections 5.6 and 5.7), because Slang calls it: a Rust panic in a callback then aborts the process at the boundary instead of unwinding into the C++ frames of Slang (FFI-7).

These are the forms that FFI-3 requires of generated raw bindings. FFI-3 requires `Option<NonNull<T>>` only in hand-written signatures, because the headers do not say which pointers may be null and the generator must not guess. The raw pointers carry the null case and keep the `const` qualifier, and the safe crate converts each one at the boundary (FFI-6).

### 5.3. Enumerations

SCS-7. The binding generator MUST emit each named C++ enumeration as a `#[repr(transparent)]` newtype over its underlying integer type, with one associated constant for each enumerator, MUST emit each enumerator of an unnamed enumeration as a free `pub const` item of the integer typedef that a rule of Section 4.4 assigns to the enumeration, and MUST NOT emit a Rust `enum`, because the library can return a value that the header does not list, and such a value in a Rust `enum` is undefined behaviour (FFI-10), and because a flag constant typed by its flags typedef is passed where the header takes that typedef without a cast.

An enumeration with a fixed underlying type uses that type, mapped by Section 5.2. An enumeration without one becomes a newtype over `u32` when no enumerator is negative, and over `i32` otherwise, because its underlying type differs in signedness between targets but not in size (Section 3.8). The generator rejects an enumeration without a fixed underlying type whose size is not 4 bytes, and an enumerator that does not fit the underlying type. An enumeration nested in a record gets the name that Section 5.11 derives from its qualified C++ name, and the generator fails on any collision of two generated names.

This illustration shows the form of the constants of an unnamed enumeration, with a placeholder enumerator and value:

```rust
pub const SLANG_COMPILE_FLAG_EXAMPLE: SlangCompileFlags = 8;
```

### 5.4. Constants

SCS-8. The binding generator MUST evaluate macro constants through a synthetic translation unit, only for the macros on an explicit allow-list by name pattern, and MUST exclude the platform and compiler detection macros by rule, because a macro definition cannot be read reliably as text, and a platform macro has a value that depends on the target that the generator parses for, so it would make the file differ between targets. The binding generator MUST emit `SlangResult` as a `#[repr(transparent)]` newtype over `i32`, with each result code as an associated constant of that type, because a result code typed as a plain integer can be confused with any other integer, which FFI-10 forbids.

The allow-list is the list of macro rules of the rule set (Section 4.1). It makes these macros constants:

- The result codes `SLANG_OK`, `SLANG_FAIL`, the codes matching `SLANG_E_*`, and the aliases of result codes matching `SLANG_ERROR_*`, each an associated constant of `SlangResult` with the bit pattern that the header defines.
- The facilities matching `SLANG_FACILITY_*`, as free constants of type `i32`, because they are `int` in the header.
- `SLANG_API_VERSION`, as a free constant of type `u32`, the type of the field `SlangGlobalSessionDesc::apiVersion` that takes it.
- The size sentinels `SLANG_UNBOUNDED_SIZE` and `SLANG_UNKNOWN_SIZE`, as free constants of type `usize`.

Every other macro has a rule that excludes it, such as the platform and compiler detection macros, the macros that expand to an attribute or a keyword, and the UUID macros. The generator fails when a value does not fit its type. The three `inline constexpr` constants of `slang.h` become `pub const` items.

`SlangResult` follows FFI-10 like the enumerations of Section 5.3. A `#[repr(transparent)]` newtype has the same ABI as the C typedef of `int32_t`, so every parameter and return value that the header declares as `SlangResult`, in a C function or a vtable entry, uses the newtype. Every `i32` value is a valid `SlangResult`, so an unknown result code causes no undefined behaviour, and COM-7 requires the safe crate to classify every code, including unknown ones, in one function.

### 5.5. Structs, Unions, and Defaults

SCS-9. The binding generator MUST emit each struct as a `#[repr(C)]` struct and each union as a `#[repr(C)]` union, with public fields in declaration order, and MUST emit an `impl Default` that reproduces the C++ default member initializers for each struct that has any, using the evaluated value of each initializer, a null pointer for `nullptr`, and zero for `{}`, because a descriptor whose size field is zero or wrong is rejected or misread by Slang (Section 3.5). A struct without default member initializers gets no `Default`.

A field without an initializer in a struct that has some takes zero in the generated `Default`, which is the value that C++ value-initialization gives it, except a field whose type is a struct with a `Default` of its own, which takes that default. Zero is `false` for `bool`, a null pointer for a pointer, `None` for a callback, and zero in every element of an array.

A floating-point initializer becomes a Rust literal. The generator refuses a value that is not finite, and a `float` value outside the range of `f32`, as a failed evaluation, because Rust has no literal for the first and the second would change its value.

A struct or union may contain anonymous structs and unions, also nested in each other, such as a struct inside an anonymous union. Each one becomes a named `#[repr(C)]` type of its own, with its own layout assertions, and the field that holds it gets a generated name (Section 5.11). The generator refuses a record with a base class, a record without fields, which has size 1 in C++ but size 0 as a Rust struct, a bit-field, and a default member initializer in a union (SCS-19).

### 5.6. Interfaces

SCS-10. For each interface `I`, the binding generator MUST emit the items below, because they are the only path from Rust to the methods, and a wrong order, calling convention, or UUID compiles and fails only at run time (ARCH-13, COM-4). The items are a `#[repr(C)]` struct `I` whose only field is the vtable pointer, and a `#[repr(C)]` vtable struct whose first field is the vtable struct of the base interface. That field is followed by one `unsafe extern "system-unwind" fn` pointer for each own method in declaration order, with the interface pointer as the explicit first parameter. The items also include the UUID of `I`, as an associated constant of a trait that the generator emits and implements for every interface, and one `unsafe` thunk for each own and inherited method, which calls through the vtable. The ABI of each entry is the unwinding form `"system-unwind"`, because some methods of Slang let a C++ exception escape (Section 3.3). An exception that enters Rust through a non-unwinding ABI is undefined behaviour, and through an unwinding ABI it is defined (FFI-7).

A thunk is an associated function whose first parameter is the raw interface pointer, `this: *mut Self`, and not a method with a `&self` receiver. A Rust reference to `I` covers only its vtable pointer, while the native object extends beyond it and changes behind the reference, so a reference would assert an aliasing guarantee that the object does not keep. The raw pointer asserts nothing, which is the sounder choice under Hard Rule 7 of `AGENTS.md`. A thunk for an inherited method casts the pointer to the type of the base interface and calls through the base vtable inside the vtable of `I`.

The trait is `pub unsafe trait Interface { const UUID: SlangUUID; }`, and each interface implements it with `unsafe impl`. It is an unsafe trait because an implementation promises that the type is an interface struct whose vtable has the layout of the native interface with that UUID, which the compiler cannot check, so any other implementation has to state that promise in an `unsafe impl` of its own. The interface struct `I` has the one field `vtable: *const IVtbl`, and the vtable struct is named `IVtbl`. The first field of the vtable struct of a derived interface is `base`, of the type of the base interface's vtable struct, and `ISlangUnknownVtbl` has no `base` field. A thunk for an inherited method calls through `base` once for each level of inheritance, with `this.cast()` as its first argument.

A vtable that Rust implements and passes to Slang, such as a custom file system, uses the same generated vtable struct, so its fields have the type `"system-unwind"`. Its functions are defined `extern "system"`, so that a Rust panic that reaches the boundary aborts the process instead of unwinding into Slang, and each is stored in its field through a conversion of the function pointer to the type of the field, which FFI-7 permits because the caller's ABI is the unwinding form of the callee's. Section 8.4 of `docs/design/slang-compiler.md` applies this rule to the safe crate.

The generator emits no `Send` or `Sync` implementation, and the raw pointer field already makes every interface struct neither (FFI-9). An opaque type of SCS-4 that is an excluded interface is emitted without vtable and thunks. The generator refuses an interface with a data member, a virtual destructor, a method that has the name of another method of the interface or of a base, a base that is not an included interface, and a UUID that it cannot read as eleven integers (SCS-19).

### 5.7. C Functions and Opaque Handles

SCS-11. The binding generator MUST declare every included C function in an `unsafe extern "C-unwind"` block, with no function marked `safe`, because the compiler trusts these declarations without checking them (FFI-3). It MUST emit each opaque handle and each opaque type of SCS-4 as a zero-sized `#[repr(C)]` struct that code outside the crate cannot construct and that is neither `Send`, `Sync`, nor `Unpin`, because a constructible or movable opaque type lets safe code create a value that no native object stands behind. The block uses the unwinding ABI for the reason that SCS-10 gives for the vtable entries, because a C function of Slang is C++ code as well (FFI-7).

An opaque type has the private field `_data: [u8; 0]` and a private `PhantomData` marker of a raw pointer and `PhantomPinned`, which together give these properties, and it derives `Debug`. The generator refuses an included function with C++ linkage, or one that the header defines or declares `inline` (SCS-3), and an excluded type that an included declaration uses by value (SCS-4).

### 5.8. Layout Assertions

SCS-12. The binding generator MUST emit, for every struct and union, compile-time assertions of its size, its alignment, and the offset of each field, with the values that libclang reports, because a binding with a wrong layout otherwise compiles and corrupts memory at run time, while an assertion stops the build. The layouts are identical on the supported targets (Section 3.8), so one set of values holds on all of them.

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

### 5.10. Repository Checks on the Generated File

SCS-13. The binding generator MUST emit no comment and no item documentation, only one module-level `//!` line that says the file is generated and names the recipe `generate-bindings`, because `check-rust-comments` rejects any other comment (Section 7.2 of the writing guide). The crate MUST carry the lint exception of the generated code as one `#[expect(..., reason = "...")]` on the declaration `mod bindings;`. It lists exactly the lints that fire, as determined by running the checks, except `ffi_unwind_calls`, which a second `#[expect]` on the same declaration lists with its own reason. A listed lint that does not fire fails the check through `unfulfilled_lint_expectations` (Section 7.9 of the writing guide), so the list is set by running the checks.

Because the file holds no other comment and no documentation, `check-rust-comments`, `check-doc-headings`, and `check-prose` read it like any other file and need no exemption, as Section 7.2 of the tooling design records. With the file checked in, `check-fmt`, the custom checkers, `check-unused-deps`, `check-deny`, `check-doc`, and `check-miri` pass on 2026-11-10 without any exclusion for it. The only exclusion that the file needs is the one of SCS-14.

On the checked-in file, compiled as the module `bindings` of `slang-compiler-sys` with the workspace lint tables and warnings denied, exactly these lints fire, with the number of findings of each on 2026-11-10:

- `missing_docs`, 1,696.
- `non_snake_case`, 954.
- `non_upper_case_globals`, 303.
- `non_camel_case_types`, 2.
- `clippy::exhaustive_structs`, 131.
- `clippy::missing_safety_doc`, 291.
- `clippy::undocumented_unsafe_blocks`, 317.
- `clippy::multiple_unsafe_ops_per_block`, 290.
- `clippy::similar_names`, 1.

The lint exception on `mod bindings;` lists exactly these nine lints, and its reason says that the generated bindings keep the names of the Slang headers and leave the meaning of each item to Slang's documentation, that their structs mirror C++ structs that Slang fills in, and that the crate documentation states the safety contract that every generated function shares. No expectation of the list is unfulfilled on any of the four supported targets of Section 6. On 2026-11-10, `just check` passed on `x86_64-unknown-linux-gnu`, and Clippy with warnings denied passed on `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, and `x86_64-apple-darwin`, so every layout assertion holds on those targets as well. For `i686-unknown-linux-gnu`, compilation stops with the message of SCS-15. These are the lints that the scratch crate of Section 3.11 found on the same file.

Since 2026-11-11, the declaration `mod bindings;` carries a second lint exception, `#[expect(ffi_unwind_calls, reason = "...")]`, because every thunk calls a vtable entry declared `"system-unwind"` (SCS-10). Its reason says that the thunks call vtable entries declared with the unwinding ABI, because Slang is C++ and some of its calls let an exception escape, which is undefined behaviour through a non-unwinding ABI. It is a separate attribute so that its reason states this cause, which differs from the reason of the nine lints above. `just check` and Clippy with warnings denied on the three other supported targets passed with both exceptions on 2026-11-11, so neither expectation is unfulfilled.

`clippy::exhaustive_enums` does not fire, because the file has no Rust `enum` (SCS-7), and `missing_debug_implementations` does not fire, because every generated type implements `Debug` (Section 5.11). The crate root also carries the opt-out `#![expect(unsafe_code, reason = "...")]` that Section 4.4 of the coding standard defines, because the thunks hold `unsafe` blocks, and its reason says that the generated bindings declare foreign functions and call through vtables, which only unsafe code can do. Original C++ names are kept, which is why the naming lints fire. The thunks are `unsafe fn` items whose bodies are `unsafe` blocks without safety comments, and they have no `# Safety` sections, which is why the three lints of safety documentation fire. Step 3 of Section 15 set the list in the lint exception by running the checks on the checked-in file, as SCS-13 requires, and every later regeneration sets it again in the same way. Section 7.9 of the writing guide and UNS-6 exempt generated raw bindings from `# Safety` sections and safety comments on two conditions, which this design meets: the crate-level documentation states one safety contract that every thunk and every C function shares (Section 13), and the exemption is part of the lint exception on `mod bindings;` (SCS-13).

The second generated file, `tests/sys/symbols.rs` (Section 5.1), needs no lint exception. It holds one `//!` line and one constant, and on 2026-11-10 no lint fired on it with the workspace lint tables and warnings denied on any of the four supported targets, so the declaration `mod symbols;` in `tests/sys/main.rs` carries no `#[expect]`. An expectation there would be unfulfilled and fail the check (SCS-13). The comment checker, the heading checker, the prose checker, and typos read it like any other file.

SCS-14. The agent that first checks in the generated file MUST make `check-typos` pass on it by the narrowest exclusion that works, such as an entry for one identifier, and MUST record the exclusion and its reason in this section, because Slang identifiers are not ordinary words, and a broad exclusion, such as excluding the whole file, would stop typos from checking the generated `//!` line and any later text in the file.

Step 3 of Section 15 made this exclusion. typos reports three identifiers of the generated file: `ISlangClonable`, `ISlangClonableVtbl`, and `SLANG_LANGAUGE_VERSION_DEFAULT`. The Slang headers misspell these names, and the bindings keep them, because each generated name has to match the header (Section 5.11). `typos.toml` accepts each of the three as a whole identifier, with an entry in its `[default.extend-identifiers]` table. Three entries for whole identifiers are narrower than excluding the file, because typos still checks every other identifier and every word of the file, including the `//!` line.

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
| Structs | `Clone`, `Copy`, and `Debug` |
| Unions | `Clone` and `Copy`, with an implementation of `Debug` that prints the type name only |
| Interface structs, vtable structs, and opaque types | `Debug` |

The generated file has no `use` items. It writes every name that is not in the Rust prelude with its full path, such as `::core::ffi::c_int` and `::core::mem::offset_of!`.

## 6. Supported Targets and Platform Independence

SCS-15. The binding generator MUST write, at the top of the generated file, a `compile_error!` under `#[cfg(not(target_pointer_width = "64"))]` that names the supported targets, so that `slang-compiler-sys` stops compilation on every target whose pointer width is not 64, because on a 32-bit target one method has another calling convention and every struct that holds a pointer has another layout (Section 3.8), so the bindings would compile and then corrupt memory.

The supported targets are x86_64 Linux, x86_64 Windows with the Microsoft toolchain, and aarch64 and x86_64 macOS. CI tests x86_64 Linux, x86_64 Windows, and aarch64 macOS (Section 12). On 2026-11-10, the generated bindings linked against the Slang of the SDK, and the smoke test and the native-symbol test passed, on x86_64 Windows and on aarch64 macOS, the `macos-latest` runner, which confirms what was SA-3 for those two targets (Section 11). x86_64 macOS is supported by construction and is not run: on that date, Clippy with warnings denied passed for `x86_64-apple-darwin` in the development container, which type-checks the bindings and their layout assertions for that target but neither links nor runs them. That the bindings link and the tests pass on x86_64 macOS remains unverified, and a run on an x86_64 Mac would confirm it. 32-bit targets are a non-goal that the owner can revisit. The Vulkan SDK installed in the development container provides its libraries for `x86_64` only, and its root is the `x86_64` directory. This design does not rely on whether LunarG ships 32-bit libraries on any platform.

The bindings are generated once, on Linux, and the same file serves every supported target. Everything that depends on the target is expressed in the Rust source by construction: the calling convention through `extern "system-unwind"` and `extern "C-unwind"`, which name the C calling convention on every 64-bit target, the integer typedefs through `isize` and `usize`, and, where nothing else works, `#[cfg]`, which the generator emits only under a rule that names the case. The comparison of Section 7.3 shows that the headers of the pin need no `#[cfg]` other than the one on the `compile_error!` of SCS-15.

This is the check that the generator writes, directly after the `//!` line of Section 5.10:

```rust
#[cfg(not(target_pointer_width = "64"))]
compile_error!("the Slang bindings support only 64-bit targets: x86_64 Linux, x86_64 Windows with the Microsoft toolchain, and aarch64 and x86_64 macOS");
```

The reason for the restriction is cost against benefit. Support for 32-bit targets would need a second set of layout assertions, a per-method calling convention, and a test platform that no CI job provides, for no user that the project knows of. A 64-bit restriction keeps one file correct by construction, and the `compile_error!` turns a silent memory corruption into a build error.

## 7. Binding Generator

### 7.1. Crate and Dependency

SCS-16. The binding generator MUST be the workspace member `tools/bindings-generator`, with `publish = false`, and MUST use the crate `clang` with the features `runtime` and `clang_10_0` and no unsafe code of its own, because with `runtime` the generator loads libclang only when it runs, so the workspace still builds and passes its checks on a machine without libclang, and a generator without unsafe code needs no exception to Hard Rule 7 of `AGENTS.md`.

The crate `clang` 2.1.0 exposes everything the generator needs through its safe API (Section 3.1). It wraps only the libclang API up to clang 10, which is enough. Only one instance of its `Clang` type may exist in a process, so the generator creates one instance per run and passes it to every parse. The crate `clang-sys`, used directly, would need unsafe code. The generator is an application, so ERR-1 and ERR-2 do not apply to it (Section 1.1 of the coding standard), and it opts out of `clippy::print_stderr` at its binary root as Section 4.4 of the coding standard describes.

### 7.2. Parsing

SCS-17. The binding generator MUST parse `slang.h` at `-std=c++17` once for each of the targets `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, and `aarch64-apple-darwin`, each with `-nostdinc` and the shim headers that the generator's crate checks in, and MUST fail when any parse reports a diagnostic of error severity or above, because libclang returns a translation unit even after a fatal diagnostic, and the model of that translation unit is silently wrong (Section 3.8).

The shim headers, under `tools/bindings-generator/shim/`, provide `stdint.h`, `stddef.h`, `inttypes.h`, `features.h`, and `TargetConditionals.h` on top of the compiler's predefined macros. The parse for Linux also uses them, so that no parse depends on the system headers of the machine that runs the generator. Each parse passes libclang the arguments `-x c++ -std=c++17 -target <target> -nostdinc -isystem <shim directory> -I<header directory>`.

The generator evaluates macro constants (Section 5.4) and default member initializers (Section 5.5) through one synthetic translation unit for each target, which includes `slang.h` and declares one `constexpr auto` variable for each value to evaluate. An initializer of `nullptr` or `{}` is not evaluated and maps as Section 5.5 states. Any other initializer that libclang cannot evaluate is a failure.

Before it parses, the generator reads `SLANG_TAG_VERSION` from `slang-tag-version.h` in the same directory and fails when it differs from the build tag of the pin (Section 8.1), because bindings generated from headers that do not match the pin would be checked in under the wrong version. It reads every line that starts with `#define SLANG_TAG_VERSION`, fails when the file cannot be read, when no such line exists, or when the value on such a line is not exactly one string literal, and compares the value of the last such line.

### 7.3. Comparing Targets

SCS-18. The binding generator MUST compare the models of all parses and MUST accept only the differences that it has a rule for, which are a different canonical type behind the same typedef name and a different signedness of an enumeration without a fixed underlying type, and MUST fail on any other difference, because an unexpected difference means that one file cannot be correct on every target.

The comparison works on the models after selection. Both accepted differences disappear from the compared models: the model records a type behind a typedef by the typedef's name (SCS-6), so its canonical type is not compared, and the signedness of an enumeration without a fixed underlying type is ignored, while its enumerator values are compared, which catches the case of Section 3.11.

SCS-19. The binding generator MUST fail, for each supported target, on every included declaration that uses a construct that the generated form of Section 5 cannot express, and on a declaration that matches no rule of Section 4.1, because each such construct has an ABI or a layout that the generated Rust would get wrong without any error. On the Microsoft ABI, for example, a member function returns an aggregate through a hidden pointer, so a vtable entry written as a plain return would read garbage.

These are the constructs that SCS-19 refuses. Section 7.5 describes the tests.

- A parameter or return value that is an aggregate passed by value, an interface or an opaque type used by value, or an array parameter.
- A variadic function.
- A method, a function, or a callback type whose calling convention, as libclang reports it for the target, is not the C calling convention.
- An interface with more than one base, a base that is not an included interface, a data member, or a virtual destructor, a method that is not pure virtual, and a method that has the name of another method of the interface or of a base.
- A class with virtual methods and no UUID, an interface whose UUID the generator cannot read, and an interface when the bindings lack the struct `SlangUUID` with four fields.
- A record with a base class, a record without fields, a bit-field, a default member initializer in a union, a `nullptr` initializer on a field that is not a pointer, and an anonymous record outside a record.
- An enumeration without a fixed underlying type whose size is not 4 bytes, and an enumeration whose underlying type is not an integer type.
- An included function with C++ linkage, or one that the header defines or declares `inline`.
- An excluded type that an included declaration uses by value.
- A type that the generator does not map, such as `void` outside a pointer, a name that is not a type or that the headers do not declare, and a typedef that the rule set maps to `isize` or `usize` but that is not an integer type.
- A pointer or a callback with a value other than `nullptr`, an integer value on a type that is not an integer type, and a floating-point value on an integer type.
- A result code macro when the bindings lack the typedef `SlangResult`, and a `SlangResult` that is not a typedef of a 32-bit signed integer.
- A variable that is not a compile-time constant, and a declaration of any other kind that a rule includes.

The generator reports each one with the declaration, its location, and the reason (Section 7.7). It also fails on values that it cannot evaluate or that do not fit their type (Sections 5.4 and 5.5), and on two items with one Rust name (Section 5.11).

None of these cases occurs among the included declarations of the pin. The method `slang::IMetadata::isParameterLocationUsed` lacks `SLANG_MCALL`, but libclang reports the C calling convention for it on every supported target, as for every other method, so SCS-19 accepts it.

### 7.4. Library and Binary

The generator is a library with a thin binary. The library's public API is:

- The function `generate`, which takes the header directory and returns `Result<Generated>`.
- The struct `Generated`, which is `#[non_exhaustive]` and holds the two generated texts in the public fields `bindings` and `symbols`, both from the same parse. Its method `files` returns each text paired with the path of its file relative to the root of the repository.
- The constants `BINDINGS_PATH`, which is `crates/slang-compiler-sys/src/bindings.rs`, and `SYMBOLS_PATH`, which is `crates/slang-compiler-sys/tests/sys/symbols.rs`.
- The types `Error`, `ErrorKind`, and `Location` with the alias `Result` (Section 7.7).
- The module `pin`, which includes the pin file (Section 8.1).

The binary takes the header directory `$VULKAN_SDK/include/slang`, calls `generate`, and writes each text that `files` returns to its path, relative to the root of the repository.

Inside the library, the module `parse` turns one parse of libclang into a model, `rules` holds the rule set, `check` selects the declarations by the rules, validates the selection against every refusal of SCS-19 for each target, and compares the targets, and `emit` writes the Rust text and formats it with `rustfmt --edition 2024` on standard input. The function of `emit` that runs rustfmt stops it and fails when it does not finish within 60 seconds, so that a stalled rustfmt cannot hang a regeneration. It takes the program, its arguments, and the time limit as parameters only so that the `ffi` tests of Section 7.5 can run a missing program and `sh` in place of rustfmt, to reach the errors of a program that cannot start, of one that fails before it reads its input, and of one that does not finish in time. The module `model` defines the model, and `error` defines the error type.

SCS-20. The binary of the binding generator and the regeneration test of Section 7.6 MUST call the same library function with the same inputs, because the test is a proof that the checked-in file equals the output of the binary only when both take the same path through the code.

### 7.5. Tests

The tests in the generator's `ffi` modules parse small synthetic headers with libclang and check the generated text for the order of vtable slots under single and multi-level inheritance, the extraction of UUIDs, the mapping of types, references, callbacks, and enumerations, the constants of unnamed enumerations, the evaluation of macros and default member initializers, including floating-point values, opaque types, layout assertions, unions and nested anonymous records, the accepted and rejected differences between targets of Section 7.3, determinism, refusals of SCS-19, the failures on a declaration or macro that matches no rule and on a stale rule (SCS-39), the check of the build tag, and the error when a process already holds libclang. The `ffi` modules also hold the tests that start a process, such as `rustfmt` or `sh`, because Miri cannot start a process. The other tests are pure Rust and also run under Miri: they cover the matching of rule patterns, the lookups in the rule set, the reading of `slang-tag-version.h`, the value checks of the validator, the comparison of targets, the formatting of literals, the escaping of names, and the error type.

Step 3 of Section 15 added tests for six refusals of SCS-19 that had none: a data member in an interface, a virtual destructor, a record with a base class, a default member initializer in a union, an array parameter, and `void` behind a typedef.

SCS-21. Every test of the generator that loads libclang MUST sit in a module named `ffi` and MUST be compiled only for Linux targets, because Miri cannot call libclang (Section 11), and the Windows and macOS runners are not required to provide libclang, so such a test would fail there without a defect in the code.

The `ffi` modules are compiled under `#[cfg(target_os = "linux")]`, so they do not run on Windows or macOS at all, and they never report a skipped run as a pass. On Linux, a test that finds no libclang fails. The Linux jobs `check` and `coverage` of the CI workflow provide libclang (Section 12), and the generator's tests passed there on the first run of the workflow with the generator. Only one `Clang` instance may exist per process (Section 7.1), so each test that loads libclang holds a static mutex for its whole run. nextest runs each test in its own process, and a run through `cargo test`, which runs the tests of one binary on several threads in one process, also works.

Step 4 of Section 15 added two tests of the generator in its `ffi` modules, which check the generated list of C functions of Section 5.1.

On 2026-11-10, counting the tests of the generator, the regeneration test of Section 7.6, and the tests of `slang-compiler-sys` in Sections 9.7 and 11, the workspace has 153 tests on Linux. The generator has 94 of them, and `slang-compiler-sys` has 59: 12 for the dynamic route, 33 for the static route, 1 for the numeric version, 8 for the real host in `build_script::ffi`, 2 in `ffi`, and 3 in `pin`. `check-miri` runs 80 of them, and the profile `default-miri` skips the 73 in `ffi` modules. On Windows and macOS, the generator's `ffi` modules are not compiled, and the two tests in the `ffi` module of `slang-compiler-sys` (Section 11) passed there in CI on 2026-11-10. The earlier CI run 42818251079 (Section 12) ran 7 of the 8 tests of the real host, because it predates the eighth. Section 12 records the failure of one test of the real host on Windows that followed. Sections 7.6, 9.7, and 11 cite these counts and do not restate them.

Reversing the order of the thunks in the generator makes a test fail, which shows that the tests of the slot order can fail.

On 2026-11-11, the change of SCS-10 and SCS-11 to the unwinding ABIs changed the texts that the tests of the generator expect, which now hold `"system-unwind"` for each vtable entry, `"C-unwind"` for the block of C functions, and `"C"` for each callback type, and two messages of the module `check` that name the ABI of the bindings. A second run of `just generate-bindings` changed nothing, and `just check-bindings` passed.

### 7.6. Recipes and Regeneration Check

The regeneration check is the test `checked_in_bindings_equal_a_regeneration_from_the_vulkan_sdk`, which step 3 of Section 15 added. It is in the module `ffi` of the generator crate's one integration-test binary, the suite `generator` under `tools/bindings-generator/tests/generator/` (TEST-10), whose root `main.rs` declares that module for Linux targets only (SCS-21). The test calls the library function `generate` on `$VULKAN_SDK/include/slang`, as the binary does (SCS-20), and compares each text that `Generated::files` returns byte for byte with its checked-in file, which are `crates/slang-compiler-sys/src/bindings.rs` and `crates/slang-compiler-sys/tests/sys/symbols.rs` (Section 7.4). It compares both files before it fails, so one failure names every file that differs. It is the first test of TEST-8, and it holds the regenerated text in memory, which TEST-8 allows, because a comparison in memory needs no file.

SCS-22. The regeneration test MUST fail, and MUST NOT skip, when `VULKAN_SDK` is unset, when the headers are missing, or when libclang cannot be loaded, and it is subject to SCS-21, because a check that passes when it cannot run lets bindings that differ from the headers reach the main branch.

The test panics with a message for each cause of failure, and nextest reports the panic as a failed test:

| Cause | The message contains |
| --- | --- |
| `VULKAN_SDK` is unset or empty | "VULKAN_SDK is not set, so the bindings cannot be regenerated", and the pinned SDK version |
| `generate` fails, such as when the header directory is missing or libclang cannot be loaded | "cannot regenerate the bindings:", followed by the error and each of its source errors (Section 7.7) |
| A checked-in file cannot be read | "cannot read", followed by the path of the file and the error |
| A regenerated text differs from its checked-in file | For each file that differs, the path of the file and the header directory, then the number of the first line that differs with that line from both texts, or the line count of each text when one text continues after the other ends, or that the texts differ only in line endings or in the final newline. The message ends with "The generated files are never edited by hand, so run `just generate-bindings` and commit the regenerated files together with the change that caused the difference" |

On 2026-11-10, the test failed as intended with `VULKAN_SDK` unset, with a header directory that does not exist, with libclang that cannot be loaded, and after a one-character edit of the checked-in file.

The test runs in the default test tier, so `test`, `test-gpu`, and therefore `just check` run it on Linux, through `test-gpu` in `check-rust`. A whole run of the generator takes about 190 ms (Section 3.11), so it adds little to the tier. The Windows and macOS runners compile the crate with the checked-in bindings and never regenerate.

On 2026-11-10, the regenerated text was byte-identical with libclang 19.1.7 in the development container and with libclang 21.1.8 on the hosted Linux runner, where the test passed in the `check` and `coverage` jobs. This is an observation for those two versions and not a guarantee for any other. CI installs the libclang package of the runner's distribution (Section 8.1 of the tooling design), and the regeneration test detects a version of libclang that changes the output.

These recipes exist, with the names and groups of Section 4.2 of the tooling design:

- `generate-bindings`, in the group `rust`: Runs the generator's binary, which overwrites both generated files, `crates/slang-compiler-sys/src/bindings.rs` and `crates/slang-compiler-sys/tests/sys/symbols.rs`, from the headers at `VULKAN_SDK`. It needs `VULKAN_SDK`, libclang, and rustfmt, and the binary fails with a message that names `VULKAN_SDK` when the variable is unset or empty (Section 19.2).
- `check-bindings`, in the group `checks`: Runs the regeneration test alone, through the nextest command of Section 4.8 of the coding standard with a filter expression that selects only that test, and with `--no-tests=fail` in place of `--no-tests=warn`.

`check-bindings` passes `--no-tests=fail` under the departure that TOOL-7 of the tooling design permits and explains, so that it never passes without having run the test, as SCS-22 requires. `check` does not run `check-bindings`, because `test-gpu` runs the same test, and no CI job runs it. On Windows and macOS, the recipe fails, because it selects no test there.

This command regenerates the bindings in the development container:

```sh
just generate-bindings
```

This command runs the regeneration test alone:

```sh
just check-bindings
```

A hand edit of either generated file makes the regeneration test fail, which enforces COM-9. An agent that changes the generator, its rule set, or the pin runs `generate-bindings` and commits both regenerated files in the same change.

`check-bindings` starts the test binaries of the workspace through nextest, which runs each binary to list its tests, and the test binary of `slang-compiler-sys` loads Slang when it starts. The recipe therefore carries the library path of Section 10.

### 7.7. Error Reporting

`generate` returns one error type, `Error`, with a message, an optional location in the headers given as the file name without its directory and the line, and an optional source error. Its method `kind` returns an `ErrorKind`, which is `#[non_exhaustive]` and tells the failures apart without the message:

| Kind | Failure |
| --- | --- |
| `Io` | The header directory cannot be read |
| `Libclang` | libclang cannot be loaded or cannot parse a file |
| `Diagnostic` | A parse reports a diagnostic of error severity or above (SCS-17) |
| `Version` | `slang-tag-version.h` is unreadable, lacks the build tag, has a malformed one, or has another one than the pin (Section 7.2) |
| `UnmatchedDeclaration` | Declarations or macros match no rule (SCS-2) |
| `StaleRule` | Rules decide nothing (SCS-39) |
| `Unsupported` | An included declaration uses a construct of SCS-19 |
| `TargetDifference` | The models of two targets differ in a way that no rule accepts (SCS-18) |
| `Evaluation` | A macro or a default member initializer cannot be evaluated or does not fit its type |
| `NameCollision` | Two generated items would have the same Rust name (Section 5.11) |
| `Rustfmt` | rustfmt cannot be started, fails, or does not finish within 60 seconds |

The errors of `UnmatchedDeclaration` and `StaleRule` list every unmatched declaration and every stale rule at once, so that one run shows the whole change to the rule set. When rustfmt fails, the message holds its exit status and its standard error, and when it does not finish in time, the message says that the generator stopped it. When libclang cannot start, the message says which of two cases applies: libclang cannot be loaded, with the reason, and the advice to install libclang or set `LIBCLANG_PATH`, or the process already holds a libclang instance or failed to load libclang before, with the advice to generate the bindings once per process and from one thread. The binary prints the error and each source error, one per line, to standard error and exits with a failure status.

## 8. The Pin

### 8.1. Pin File

SCS-23. The pin MUST be recorded only in `crates/slang-compiler-sys/pin.rs`, as three Rust constants that hold the Vulkan SDK version, `1.4.363.1`, the Slang build tag, `2026.17-25-gc7954ebd1`, and the full Slang commit, `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`, and the build script, the crate, and the generator MUST read it with `include!`, and recipes and the CI workflow MUST read it through a `just` recipe that prints its values, because ARCH-9 forbids a second copy of the pin, and a consumer that restates a value checks against a copy that can diverge.

This answers Q-6 of the architecture design. The file is Rust, so the build script reads it without a parser dependency, and it lies inside the package directory, so it ships in the published crate, where the build script needs it. The constants are `VULKAN_SDK_VERSION`, `SLANG_BUILD_TAG`, and `SLANG_COMMIT`, each a `pub const` of type `&str` with its own documentation. The crate root includes `pin.rs` with `include!("../pin.rs")`, so the constants are public items of `slang-compiler-sys`, and the smoke test and the safe crate read the pin from the crate. These constants are the only way the crate exposes the pin: the generator writes no copy of the pin into `src/bindings.rs`. The generator includes `pin.rs` by a path relative to its own source, as its public module `pin`.

The recipe `pin`, in the group `rust`, prints the three constants as `NAME=value` lines, in the order above, and fails with a message that names the file and the constant when `pin.rs` does not define one of them as a string constant. This command prints the pin:

```sh
just pin
```

On 2026-11-10, it prints:

```text
VULKAN_SDK_VERSION=1.4.363.1
SLANG_BUILD_TAG=2026.17-25-gc7954ebd1
SLANG_COMMIT=c7954ebd1f1984ad2bfde15f76248db41dc34aa3
```

The recipe reads only lines of the exact form `pub const NAME: &str = "value";`, so a change to the form of `pin.rs` makes it fail instead of printing a wrong value.

The recipe runs on every CI runner, because every job reads the pin through it (Section 12). It runs on bash 3.2, which has no associative arrays and was the bash that the recipe got on the hosted macOS runner on 2026-11-10, and it accepts lines that end in CRLF, as they did in the checkout on the hosted Windows runner on that date. Its first form failed on both runners for these two reasons, and step 2 of Section 15 rewrote it without changing its output or its failure message. It also works when `just` runs from a subdirectory of the repository.

The expected SHA-256 checksums of the SDK downloads are not part of the pin and are not in `pin.rs`. They are recorded in the CI workflow file, as Section 8.3 of the tooling design states, and they change in the same change as the pin (ARCH-10).

### 8.2. Updating the Pin

ARCH-10 to ARCH-12 of the architecture design govern a change of the pin. The agent that moves the pin edits `pin.rs`, installs the new SDK, runs `just generate-bindings`, updates the checksums in the workflow, fixes what the regeneration breaks, and runs every check. The generator then fails on any new declaration that its rule set does not cover (SCS-2), on any rule whose declaration the headers no longer have (SCS-39), and on any new construct that Section 5 cannot express (SCS-19), and the layout assertions are regenerated from the new headers.

## 9. Build Script

### 9.1. Route Selection

SCS-24. The build script MUST select its route in this order: when `DOCS_RS` is present in the environment, it returns at once, as Section 9.4 states. Otherwise, when `SLANG_SOURCE_DIR` is present, it takes the static route and does not read `VULKAN_SDK`. Otherwise, it takes the dynamic route. It MUST NOT select a route by any other input, because every additional input is another way to link a Slang that nobody chose (SYS-7).

`slang-compiler-sys` has no Cargo features. This answers Q-7, Q-8, and Q-9 of the architecture design and TQ-2 of the tooling design: the environment variable `SLANG_SOURCE_DIR` selects the static route, its name is confirmed, and the static route does not read `VULKAN_SDK`. The checks that pass `--all-features` and the feature matrix of `check-features` therefore never build Slang from source.

The reasons for selecting the route through the environment are these. Every routine check passes `--all-features`, so a feature for the static route would force a native build of several hundred C++ files into Clippy, the documentation build, the doctests, and the tests. Cargo features must be additive (DEP-1), and how the native library is linked is a choice between options that exclude each other, not an addition. The choice also belongs to whoever builds the final program, who controls the environment, and not to an intermediate dependency that requests a feature in its manifest. The cost is that a crate cannot request static linking in its manifest, though a project can set the variable in the `[env]` table of its `.cargo/config.toml`.

SYS-7 of the coding standard requires this selection: the static route only when `SLANG_SOURCE_DIR` is present, and never through a Cargo feature, for the reasons of this section.

The build script implements this order in the function `configure` of `route.rs` (Section 9.7). It reads `DOCS_RS`, then `SLANG_SOURCE_DIR`, then `VULKAN_SDK`, and it treats a variable as present when the environment contains it, whatever its value, including an empty value.

A present `SLANG_SOURCE_DIR` selects the static route of Section 9.3, whatever its value. The build script never falls back from it to the dynamic route, as ARCH-7 of the architecture design and SCS-27 require, because a user who set the variable asked for a statically linked Slang, and a silent dynamic link would give that user a program that needs the shared library at run time.

This illustration shows the command that builds the workspace on the static route from a clean git checkout of the pinned commit at `/tmp/slang`:

```sh
SLANG_SOURCE_DIR=/tmp/slang cargo build
```

### 9.2. Dynamic Route

SCS-25. On the dynamic route, the build script MUST read `$VULKAN_SDK/include/slang/slang-tag-version.h`, and extract the string that `SLANG_TAG_VERSION` defines. It MUST stop the build with the message of Section 9.6 when the file cannot be read, when the definition is missing or malformed, or when the string differs from the build tag of the pin. It MUST NOT accept any setting that bypasses this check, because a library of another Slang build has layouts and vtables that differ from the checked-in bindings, which is undefined behaviour at run time (SYS-6).

This answers Q-1 of the architecture design. The headers do not record the SDK version (Section 3.6), so the build script cannot check it. The build tag contains the abbreviated Slang commit, and the bindings depend only on Slang, so two SDKs that ship the same Slang build are equivalent for this crate. The safe crate adds a run-time comparison of `spGetBuildTagString()` with the pin in M2, and the smoke test of Section 11 asserts it from M1.

The build script reads the header with the rules that the generator uses (Section 7.2). It reads every line whose first two words are `#define` and `SLANG_TAG_VERSION`, requires the rest of such a line to be exactly one string literal that holds no quotation mark, and compares the value of the last such line. It accepts lines that end in CRLF. A missing definition and a malformed one are separate failures of Section 9.6, and the message of a malformed one names its line.

SCS-40. On the dynamic route, the build script MUST stop the build with the message of Section 9.6 when `VULKAN_SDK` is absent or empty. It MUST also stop the build when the value is not valid UTF-8, contains a line feed or a carriage return, or is not an absolute path. It MUST NOT read any file or emit any instruction that depends on such a value. A line break in the value would end a Cargo instruction early and let the rest of the value inject an instruction of its own. A value that is not UTF-8 cannot be written into an instruction without being altered. A relative path would let the header that the script checks and the library that the linker links resolve against different directories.

SCS-26. On the dynamic route, once the checks of SCS-25, SCS-40, and SCS-41 pass, the build script MUST emit `cargo::rustc-link-search=native=$VULKAN_SDK/lib` and `cargo::rustc-link-lib=dylib=slang-compiler`, and MUST NOT use pkg-config, emit an rpath, or search any other location, because a single mechanism behaves the same on every platform, and finding the library at run time is the job of the environment (Section 10).

SCS-41. On the dynamic route, the build script MUST stop the build with the message of Section 9.6 when `$VULKAN_SDK/lib` cannot be accessed or is not a directory. When the operating system of the target is the operating system of the host, it MUST also stop the build when `$VULKAN_SDK/lib` does not hold the library file of the target as a file. `CARGO_CFG_TARGET_OS` gives the operating system of the target (SYS-3). The library file is `libslang-compiler.so` for Linux, `libslang-compiler.dylib` for macOS, and the import library `slang-compiler.lib` for Windows. The build script MUST NOT look for the library file of a target whose operating system differs from the host's, or of a target for which this list names no file. Without the check of the library file, `cargo check` passes and the failure comes later from the linker with a message that does not name the SDK. The SDK of the host holds only the library of the host, so a check of a foreign target would fail every `cargo check` and Clippy run for that target from Linux. Those runs verify the layout assertions of SCS-12 on the other supported targets (Section 5.10).

The build script asks the file system what a path is through `std::fs::metadata`, which follows symbolic links, so `libslang-compiler.so`, which is a symbolic link in the SDK (Section 3.7), counts as a file. When the file system reports an error, the message gives the reason that the operating system gives, such as "Permission denied (os error 13)". For a foreign target, the build script checks only that `lib` is an accessible directory and then emits the link instructions, so a build for that target that links fails in the linker. The crate documentation states this rule (Section 13).

When every check passes, the build script emits these instructions, in this order, with the rerun instructions of Section 9.5 before them:

```text
cargo::rustc-link-search=native=<VULKAN_SDK>/lib
cargo::rustc-link-lib=dylib=slang-compiler
cargo::metadata=include=<VULKAN_SDK>/include
cargo::metadata=lib=<VULKAN_SDK>/lib
```

In this listing, `<VULKAN_SDK>` stands for the value of the variable. A test of Section 9.7 asserts this exact list.

This answers Q-5 of the architecture design: pkg-config is not used, because the `.pc` file carries only the coarse version (Section 3.6) and Windows has no pkg-config. It also answers Q-4: the build script expects the headers under `include/slang` and the library under `lib` on every platform, with the import library under `lib` on Windows. The CI runs of step 2 of Section 15 confirmed this layout on 2026-11-10, which was assumption SA-1 (Sections 3.6 and 3.7).

The CI runs of step 4 of Section 15 confirmed on 2026-11-10 that a Rust crate links against the import library on Windows, which was assumption SA-2. On the hosted Windows runner, the test binary of `slang-compiler-sys` built, linked, and ran with the search path `$VULKAN_SDK/lib` and `dylib=slang-compiler`. The linker found `Lib\slang-compiler.lib` through the lower-case path on the case-insensitive file system, although the log does not show the exact path, and the program found the DLL through `PATH` (Section 10). The check of SCS-41 for the library file of the host passed on the Windows and macOS runners, which shows that `$VULKAN_SDK/lib` holds `slang-compiler.lib` and `libslang-compiler.dylib` there.

### 9.3. Static Route

Step 5 of Section 15 implemented the static route in the function `static_route` of `route.rs` (Section 9.7). The route never reads `VULKAN_SDK`, and it never falls back to the dynamic route (ARCH-7). It writes only under `OUT_DIR/slang`, which holds these paths:

| Path under `OUT_DIR` | Content |
| --- | --- |
| `slang/source` | The copy of the tracked files of the source tree that the build compiles |
| `slang/build` | The CMake build tree |
| `slang/build/Release/include` | The headers that the build places in its build tree, including the generated `slang-tag-version.h` |
| `slang/lib` | The six static libraries |
| `slang/build.log` | The output of every `cmake` run of the current run of the build script |
| `slang/stamp` | The stamp of the last verified build |

The build script takes these steps, in this order. A step that fails returns the error of Section 9.6 for it, and no later step runs.

1. Validate `SLANG_SOURCE_DIR`. An empty value, a value that is not an absolute path in UTF-8 without line breaks, and a path that is not an accessible directory each stop the build (SCS-42).
2. Check that `OUT_DIR` is an absolute path in UTF-8 without line breaks, and read `TARGET` and `HOST`. A missing `TARGET` or `HOST` stops the build with its own error (SCS-42).
3. Check the target. A target that differs from the host stops the build. So does a target other than Linux with the GNU C library, macOS, or Windows with the Microsoft toolchain (SCS-43).
4. Check the commit. The build script runs `git rev-parse --is-inside-work-tree --show-prefix --git-path HEAD --git-path index --verify HEAD^{commit}` in the source tree (SCS-27, SCS-44). A failure of this command stops the build, which is how a tree without git metadata, such as a source archive, is refused. The command has to print five lines. The build script emits `cargo::rerun-if-changed` for the two paths that git prints for `HEAD` and `index`, resolved against the source tree (Section 9.5). It then stops the build when the directory is not inside a work tree, when the prefix is not empty, which means that the directory lies below the top level of another work tree, and when the commit differs from `SLANG_COMMIT`.
5. Check that the tree is clean. The build script runs `git status --porcelain=v1 --untracked-files=no --ignore-submodules=none` in the source tree and stops the build when it prints any line (SCS-27). This reports a tracked file that differs from the commit, and a submodule at another commit or with changed content. git does not report a submodule that was never initialized.
6. Probe Ninja. The build script runs `ninja --version` and stops the build when Ninja cannot start or fails (SCS-28).
7. Truncate the log `slang/build.log` to empty (SCS-48).
8. Read the stamp. When `slang/stamp` holds exactly the stamp of this run, the build script skips steps 9 to 11 (SCS-46).
9. Remove the old stamp, then list the files to copy. `git ls-files -z --stage` lists the tracked files of the source tree, and the entries with the mode `160000` are its gitlinks. For each of the eight submodules of Section 3.10, the build script requires a gitlink at that path and a `.git` entry, a file or a directory, inside it, and then lists the tracked files of the submodule with the same command. A tracked path that is not made only of normal components, such as one with `..`, stops the build (SCS-45).
10. Copy the listed files into `slang/source`. The build script removes every file of the copy that is not listed, and it writes a listed file only when its bytes differ from the copy (SCS-45).
11. Configure the copy. The build script runs `cmake -S <copy> -B <build> -G Ninja` with the arguments `-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=<lib>`, `-DSLANG_VERSION_FULL=2026.17-25-gc7954ebd1`, `-DSLANG_VERSION_NUMERIC=2026.17`, and `-DFETCHCONTENT_FULLY_DISCONNECTED=ON`, then the 27 options of the constant `SLANG_OPTIONS` of `route.rs`, then the options of the target (SCS-28). The numeric version is the build tag of the pin up to its first hyphen. It removes `SLANG_USE_SCCACHE` from the environment of `cmake` and appends the output to the log.
12. Build. The build script runs `cmake --build <build> --target slang`, followed by `--parallel` and the value of `NUM_JOBS` when that value is a whole number, on every run, also when the stamp matched. It removes `SLANG_USE_SCCACHE` and appends the output to the log. Ninja rebuilds only what changed.
13. Verify the result. Each of the six static libraries has to be a file in `slang/lib`, named `lib<name>.a`, or `<name>.lib` with the Microsoft toolchain. `slang/build/Release/include/slang-tag-version.h` has to define `SLANG_TAG_VERSION` as the build tag of the pin, read with the rules of Section 9.2 (SCS-47).
14. Write the stamp, when step 9 ran (SCS-46).
15. Emit the link instructions and the `links` metadata.

The 27 options of `SLANG_OPTIONS` select a release build, `SLANG_LIB_TYPE=STATIC`, the embedded core module and its source, and `SLANG_SLANG_LLVM_FLAVOR=DISABLE`. They turn off DirectX Intermediate Language (DXIL), the tests, the examples, gfx, slang-rhi, the replayer, slangd, slangc, slangi, slang-rt, slang-glslang, the Slang proxy, CUDA, OptiX, NVAPI, Xlib, Aftermath, the debug information of the release build and its split form, and sccache, and they exclude Dawn and Tint. With the Microsoft toolchain, the build script adds `-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded` when `CARGO_CFG_TARGET_FEATURE` holds `crt-static`, and `-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreadedDLL` otherwise.

When every step passes, the build script emits these instructions, in this order, after the rerun instructions of Section 9.5. In this listing, `<OUT_DIR>` stands for the value of the variable, and the last link instruction depends on the target:

```text
cargo::rustc-link-search=native=<OUT_DIR>/slang/lib
cargo::rustc-link-lib=static=slang-compiler
cargo::rustc-link-lib=static=compiler-core
cargo::rustc-link-lib=static=core
cargo::rustc-link-lib=static=cmark-gfm
cargo::rustc-link-lib=static=miniz
cargo::rustc-link-lib=static=lz4
cargo::rustc-link-lib=dylib=stdc++
cargo::metadata=include=<OUT_DIR>/slang/build/Release/include
cargo::metadata=lib=<OUT_DIR>/slang/lib
```

The C++ standard library is `dylib=stdc++` on Linux and `dylib=c++` on macOS. With the Microsoft toolchain, the build script emits no instruction for it. The link kind is plain `static`, so Cargo copies the archives into the rlib of the crate.

SCS-27. On the static route, the build script MUST verify through git that `SLANG_SOURCE_DIR` names the top level of a git work tree whose `HEAD` is the full commit of the pin. A static library built from another commit does not match the checked-in bindings, which is undefined behaviour at run time. The build script MUST stop the build when `git status` reports any tracked file or submodule that differs from that commit. A changed tree at the right commit would build a library that differs from the one that the bindings match. The build script MUST refuse a directory without git metadata, such as a source archive, because the commit of such a tree cannot be verified. The build script MUST NOT fall back to the dynamic route (ARCH-7), or else a user who asked for a statically linked Slang gets a program that needs the shared library at run time.

This answers Q-2 of the architecture design. The check reads the tree only through git, and untracked and ignored files are neither checked nor copied. A tree with only the eight submodules of Section 3.10 initialized passes, because git does not report a submodule that was never initialized as changed. On 2026-11-10, a fresh clone of the pinned commit with only those eight submodules initialized passed the check. The check also refuses a changed submodule that the build does not use, which is stricter than the build needs.

SCS-28. On the static route, the build script MUST run `cmake` with `std::process::Command` and check every exit status (SYS-4), or else a failed step surfaces later as an obscure link error. It MUST use the Ninja generator and MUST run `ninja --version` before it copies anything. Without the probe, a missing Ninja would stop the build only after a copy of about 156 MB and a configure step. It MUST pass `-DSLANG_VERSION_FULL=` with the build tag of the pin and `-DSLANG_VERSION_NUMERIC=` with its numeric part. Without the first, the library reports `0.0.0-unknown`, which the smoke test rejects, and without the second, the configure step fails (Section 3.10). It MUST disable the optional components and everything that downloads, and pass `-DFETCHCONTENT_FULLY_DISCONNECTED=ON`, because a build script that downloads breaks SYS-5. It MUST link the six static libraries of Section 3.10 and the C++ standard library of the target, or else the link fails with undefined references.

SCS-42. On the static route, the build script MUST stop the build when `SLANG_SOURCE_DIR` is empty, is not valid UTF-8, contains a line feed or a carriage return, is not an absolute path, or does not name an accessible directory. It MUST apply the same checks, except the one for a directory, to `OUT_DIR`, and it MUST stop the build when `TARGET` or `HOST` is absent. A line break in a path would end a Cargo instruction early and let the rest inject an instruction of its own. A relative path would make git, CMake, and the linker resolve it against different directories. A missing variable would make the comparison of SCS-43 meaningless.

SCS-43. On the static route, the build script MUST stop the build when `TARGET` differs from `HOST`, because Slang's build runs programs that it builds for the target, so a cross-compilation fails deep inside CMake or Ninja. It MUST also stop the build for a target other than Linux with the GNU C library, macOS, and Windows with the Microsoft toolchain, because the build script knows the archive names and the C++ standard library of these three only, and a guess would fail in the linker.

SCS-44. The build script MUST run every git command with `GIT_OPTIONAL_LOCKS=0` and with the 15 repository variables of the constant `GIT_REPOSITORY_VARIABLES` of `route.rs` removed from its environment. Those are the variables that `git rev-parse --local-env-vars` lists, such as `GIT_DIR` and `GIT_INDEX_FILE`. A git hook that runs Cargo sets such variables, and git would then answer for another repository than the one that `SLANG_SOURCE_DIR` names. Without `GIT_OPTIONAL_LOCKS=0`, `git status` refreshes the index of the clone and so writes into it, which SYS-5 forbids.

SCS-45. On the static route, the build script MUST build a copy, inside `OUT_DIR`, of the tracked files of the source tree and of the eight submodules of Section 3.10, and MUST NOT run any build step in the source tree. Slang's build writes into its source tree (Section 3.10), which SYS-5 forbids. It MUST stop the build when a required submodule has no gitlink or no `.git` entry, with a message that names the submodule and the command that fetches it, or else CMake fails with an error that does not say what is missing. It MUST refuse a tracked path that is not made only of normal components, because such a path would let the copy write outside `OUT_DIR`. It MUST remove every file of the copy that the listing does not name, or else a file that the commit no longer has stays in the build.

SCS-46. On the static route, the build script MUST copy and configure again whenever the stamp differs from the stamp of the current run, and MUST write the stamp only after a build that SCS-47 verified. The stamp holds the value of `SLANG_SOURCE_DIR`, the commit, the list of required submodules, and the full argument list of the configure step, which carries the pin and the options of the target. A stamp written before the verification would let an interrupted or failed build skip the copy and the configure step on the next run. The build script removes the old stamp before it copies, so a failure in steps 9 to 13 leaves no stamp.

SCS-47. On the static route, after the build, the build script MUST check that each of the six static libraries exists as a file and that the version header of the build tree defines the build tag of the pin. A build that produced another layout or another version would otherwise reach the linker or the program, where the failure no longer names the cause.

SCS-48. On the static route, the build script MUST append the output of every `cmake` run to `slang/build.log` under `OUT_DIR`, truncating the log once at the start of each run. On a failure of the configure or build step, its error MUST name the log and hold the last 40 lines of it. A full build prints hundreds of lines, and an error that holds them all buries the cause, while an error without them leaves the user to find the log.

On 2026-11-10, in the development container, `SLANG_SOURCE_DIR=/opt/slang just test-static` built Slang from source in 3 min 50 s on 16 cores, with `VULKAN_SDK` unset. `OUT_DIR` then took 956 MB: 156 MB for the copy, 703 MB for the build tree, 97 MB for the static libraries, and 60 KB for the log. With the final code of step 5, all 59 tests of `slang-compiler-sys` passed on the static route, including the smoke test and the native-symbol test of Section 11. `readelf -d` on the test binary listed as needed only `libstdc++.so.6`, `libgcc_s.so.1`, `libm.so.6`, `libc.so.6`, and `ld-linux-x86-64.so.2`. A second run with no change was fresh and did not run the build script. A rerun of the build script with a matching stamp took about 2 s, and it ran only the Ninja build, which had nothing to do. After a change of `SLANG_SOURCE_DIR` to a copy of the same tree, a run without network access rewrote no file of the copy, configured again, and rebuilt no library. The log held no download. The checksum of `/opt/slang/.git/index` was the same before and after every run, and no file of `/opt/slang` was newer than a marker created before the first run.

The route was implemented for macOS and for Windows with the Microsoft toolchain from reading Slang's CMake files, and it has never run on either (Section 17, SQ-3). Clippy with warnings denied passed on the crate for `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, and `x86_64-apple-darwin` on 2026-11-10. On Windows, the build with Ninja is expected to need the developer environment of the Microsoft toolchain, which the build script does not set up.

The recipe `test-static`, in the group `rust`, runs the tests of `slang-compiler-sys` on the static route (SCS-49). It fails with a message that names `SLANG_SOURCE_DIR` and says what to set when the variable is unset or empty. Otherwise it runs this command:

```sh
env -u VULKAN_SDK -u LD_LIBRARY_PATH -u DYLD_LIBRARY_PATH CARGO_TARGET_DIR=target/static CARGO_BUILD_WARNINGS=deny cargo nextest run -p slang-compiler-sys --all-features --profile default --no-tests=fail
```

SCS-49. The recipe `test-static` MUST run only the package `slang-compiler-sys`, in the target directory `target/static`, with `VULKAN_SDK`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` removed from the environment. Without `VULKAN_SDK` and the loader path, a test binary that still needed the Slang shared library could not start, so the tests pass only when the link is static. The regeneration test of the generator needs `VULKAN_SDK`, so a run of the whole workspace would fail. A separate target directory keeps the static build from replacing the dynamic build of `target/`, which every other recipe uses. `check` and per-pull-request CI do not run `test-static`, because it builds Slang from source (SCS-36).

The recipe removes no variable that Windows uses to find a DLL, so on Windows it would not prove the static link in the same way. It passes `--no-tests=fail`, so it fails when it selects no test. This command verifies the static route from a clean git checkout of the pinned commit at `/tmp/slang`:

```sh
SLANG_SOURCE_DIR=/tmp/slang just test-static
```

### 9.4. Documentation Builds

SCS-29. When `DOCS_RS` is present, whatever its value, the build script MUST return at once, after the rerun instruction for `DOCS_RS` (SCS-30), without reading any other variable, emitting any link instruction, or emitting `links` metadata, because docs.rs has no Slang, and a build script that fails there leaves the crate without published documentation (ARCH-8).

The checked-in bindings let the crate document itself without Slang. That docs.rs sets `DOCS_RS` is assumption A-7 of the architecture design. The recipe `check-doc-without-slang`, in the group `checks`, runs the documentation command of Section 4.8 of the coding standard with `DOCS_RS` set to `1` and `VULKAN_SDK` removed from the environment (Section 10 of the tooling design), and `check-rust` runs it directly after `check-doc`. On 2026-11-10, with `DOCS_RS=1` and `VULKAN_SDK` unset, `cargo check` and `cargo doc` both succeeded. The recipe imitates the absence of Slang on docs.rs and cannot confirm A-7, which the first docs.rs build confirms in M7. A program built with `DOCS_RS` present does not link, because the build script emits no link instruction. The manifest of `slang-compiler-sys` lists only the four supported targets of Section 6 under `targets` in `[package.metadata.docs.rs]`, which are `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`, `aarch64-apple-darwin`, and `x86_64-apple-darwin`, so that docs.rs does not build for a 32-bit target, on which SCS-15 stops compilation (SA-8). Step 3 of Section 15 added this list.

This command runs the documentation build without Slang:

```sh
just check-doc-without-slang
```

### 9.5. Rerun Instructions and Metadata

SCS-30. The build script MUST emit `cargo::rerun-if-env-changed` for `DOCS_RS` on every run, for `SLANG_SOURCE_DIR` on every run without `DOCS_RS`, and for `VULKAN_SDK` on the dynamic route. It MUST emit `cargo::rerun-if-changed` for `slang-tag-version.h` on the dynamic route, and for the `HEAD` and `index` files of the git checkout on the static route. A build that does not rerun after its input changes links a library that no longer matches the check it passed (SYS-1).

The build script emits each rerun instruction for a variable or a header before it reads it. On the static route, it emits the instructions for the two git files directly after `git rev-parse` reports their paths, and before any later check. `build.rs` prints every instruction that `configure` collected before it returns the result, so the instructions reach Cargo when the build script fails as well. A test of Section 9.7 asserts the rerun instructions on every failure path. On 2026-11-10, a build with no change was fresh, a change of `VULKAN_SDK`, of `slang-tag-version.h`, or of `DOCS_RS` reran the build script, and a change of an unrelated file or variable did not. On the static route on that date, a change of `SLANG_SOURCE_DIR` and a change of the `index` of the clone each reran the build script.

On the static route, the paths are those that `git rev-parse --git-path HEAD --git-path index` prints, resolved against the source tree, which are `<tree>/.git/HEAD` and `<tree>/.git/index` for an ordinary clone. A checkout or a commit rewrites the index in the normal workflow, so these two files stand for the state of the whole tree. The build script reads every tracked file when it copies the tree, but it does not watch those files, because the tree holds too many to watch on every build. SYS-1 of the coding standard permits this for a tree whose commit the build script verifies. On every run, before it compares the stamp, the build script checks through `git rev-parse` and `git status` that the tree is at the pinned commit and unmodified (steps 4 and 5 of Section 9.3). An edit of a tracked file without any git operation afterwards therefore does not rerun the build script, and the library stays the one built from the clean tree that the last run verified. The build script does not watch the reference file behind a symbolic `HEAD` or `packed-refs` either.

`pin.rs`, `route.rs`, and `system.rs` need no instruction, because Cargo rebuilds and reruns the build script when a file that it includes changes. The build script emits no instruction for the variables that Cargo sets for each build, such as `CARGO_CFG_TARGET_OS`, `TARGET`, `HOST`, `OUT_DIR`, and `NUM_JOBS`, none for `$VULKAN_SDK/lib` or the library file, whose kind it checks but whose content it does not read, and none for the files under `OUT_DIR` that it writes itself. CMake reads `CC`, `CXX`, and `CMAKE_TOOLCHAIN_FILE` from the environment, and the build script does not watch them (Section 17, SQ-5).

The manifest declares `links = "slang-compiler"` (FFI-2). Through the `links` metadata, which it emits as `cargo::metadata=include=...` and `cargo::metadata=lib=...`, the build script exposes the directory of the headers as `include` and the directory of the library as `lib`, so a dependent build script reads them as `DEP_SLANG_COMPILER_INCLUDE` and `DEP_SLANG_COMPILER_LIB`. On the dynamic route, they are `$VULKAN_SDK/include`, which holds the headers under `slang`, and `$VULKAN_SDK/lib`. On the static route, they are `slang/build/Release/include` and `slang/lib` under `OUT_DIR`. The first holds the headers and `slang-tag-version.h` directly, without the directory `slang`. It is the include directory of the build tree, because it holds the headers that were built and the version header that the build script verified (SCS-47), while the `include` directory of the source tree lacks `slang-tag-version.h`. Under `DOCS_RS`, neither is set, and a dependent build script that needs them handles their absence.

### 9.6. Failure Messages

SCS-31. On each failure of this table, the build script MUST end by returning an error from its `main` (SYS-4) whose message contains every item of the table's second column, because a build that stops without naming what is missing and how to supply it leaves the user to guess, and a guess often links the wrong Slang.

In this table, the SDK advice is the sentence that tells the user to install the LunarG Vulkan SDK of the pinned version from <https://vulkan.lunarg.com> and to set `VULKAN_SDK` to the absolute path of its directory that holds `include/slang` and `lib`, which is the `x86_64` directory of the SDK on Linux, the `macOS` directory on macOS, and the installation directory on Windows. The source advice is the sentence that tells the user to set `SLANG_SOURCE_DIR` to the absolute path of the top-level directory of a git clone of Slang at the pinned commit with its submodules, gives the three commands `git clone --recursive https://github.com/shader-slang/slang`, `git checkout <commit>`, and `git submodule update --init --recursive`, and offers to unset `SLANG_SOURCE_DIR` to link the Slang of the Vulkan SDK of the pinned version that `VULKAN_SDK` names. Every row is a failure that the build script raises. A function of `route.rs` builds the message of each, and a test of Section 9.7 checks each. The first eight rows belong to the dynamic route, and the others to the static route. A command in a message is the program followed by its arguments, without its environment.

| Failure | The message contains |
| --- | --- |
| `VULKAN_SDK` is absent or empty, and `DOCS_RS` and `SLANG_SOURCE_DIR` are absent | The name `VULKAN_SDK` with "is not set" or "is set but empty", the SDK advice, and, as the other way to obtain Slang, building it from source by setting `SLANG_SOURCE_DIR` to the absolute path of a git checkout of Slang at the pinned commit (ARCH-5) |
| The value of `VULKAN_SDK` is not valid UTF-8, contains a line feed or a carriage return, or is not an absolute path (SCS-40) | The name `VULKAN_SDK` and its value in quotation marks, with every control character escaped and every byte that is not UTF-8 replaced, the statement that the value is not an absolute path in UTF-8 without line breaks, and the SDK advice |
| `$VULKAN_SDK/include/slang/slang-tag-version.h` cannot be read | "cannot read", the full path, the reason that the operating system gives, why the build reads the file, and the SDK advice |
| That file has no line that defines `SLANG_TAG_VERSION` | The full path, the name `SLANG_TAG_VERSION`, the build tag of the pin, and the SDK advice |
| A line of that file defines `SLANG_TAG_VERSION` as anything other than one string literal | The number of the line, the full path, the build tag of the pin, and the SDK advice |
| The build tag differs from the pin | The SDK directory, the build tag that it ships, the path of the file that records it, the build tag of the pin, the pinned SDK version, that the bindings match only that build, and the two remedies: install the pinned SDK and set `VULKAN_SDK` to it, or use the version of the crate that matches the installed SDK (SYS-6) |
| `$VULKAN_SDK/lib` cannot be accessed or is not a directory (SCS-41) | "cannot access" with the path and the reason that the operating system gives, or the path with "is not a directory", then the SDK directory, the library directory, the name of the library file on each of Linux, macOS, and Windows, and the SDK advice |
| The target has the operating system of the host, and the library file of the target in `$VULKAN_SDK/lib` cannot be accessed or is not a file (SCS-41) | "cannot access" with the path of the file and the reason that the operating system gives, or the path with "is not a file", then the same content as the previous row |
| `SLANG_SOURCE_DIR` is empty (SCS-42) | "SLANG_SOURCE_DIR is set but empty", that this selects building Slang from source without naming a source tree, and the source advice (ARCH-7) |
| The value of `SLANG_SOURCE_DIR` is not valid UTF-8, contains a line feed or a carriage return, or is not an absolute path (SCS-42) | The name `SLANG_SOURCE_DIR` and its value in quotation marks, escaped as for `VULKAN_SDK`, the statement that the value is not an absolute path in UTF-8 without line breaks, and the source advice |
| `SLANG_SOURCE_DIR` is not a directory or cannot be accessed (SCS-42) | The name `SLANG_SOURCE_DIR`, the path, "which is not a directory" or "which cannot be accessed" with the reason that the operating system gives, and the source advice (ARCH-7) |
| `OUT_DIR` is absent, or is not an absolute path in UTF-8 without line breaks (SCS-42) | The name `OUT_DIR` with "is not set" or its value in quotation marks, the statement that the build has no directory in which to build Slang, and that Cargo sets `OUT_DIR`, so the crate is built through Cargo with a target directory whose path is UTF-8 |
| `TARGET` or `HOST` is absent (SCS-42) | The name of the variable with "is not set", that the build cannot compare the target with the host, and that Cargo sets the variable |
| The target differs from the host (SCS-43) | Both target triples, that the build from source cannot cross-compile because Slang's build runs programs that it builds for the target, and the two remedies: build on a host of the target, or unset `SLANG_SOURCE_DIR` to link the Slang of the pinned Vulkan SDK |
| The target is not Linux with the GNU C library, macOS, or Windows with the Microsoft toolchain (SCS-43) | The three supported kinds of target, the target triple, and the same two remedies |
| `git`, `ninja`, or `cmake` cannot start (SYS-4) | "cannot start" with the program, the command, and the reason that the operating system gives, that the build from source needs git, CMake 3.22 or later, and Ninja on `PATH`, the path of the source tree for `git` and `ninja`, and the remedies: install the missing program, or unset `SLANG_SOURCE_DIR` |
| `git rev-parse` fails, as in a directory without git metadata (SCS-27) | The command, the source tree, the exit status, and the error output of git, that `SLANG_SOURCE_DIR` has to name the top level of a git checkout because the build verifies the commit through git, that a tree without git metadata, such as a source archive, cannot be verified, and the source advice |
| `git rev-parse` does not print five lines, or `git ls-files` prints an entry that the build cannot read or a path that is not made only of normal components (SCS-45) | The command, the directory, and the output in quotation marks, with every control character escaped, and that it is not the output that the build expects from git |
| The directory is not inside a git work tree, or lies below the top level of another work tree (SCS-27) | The path, "is not inside a git work tree" or "lies at" with the prefix and "inside a git work tree whose top level is another directory", that it is not the top level of a git checkout of Slang, and the source advice |
| `HEAD` differs from the pin (SCS-27) | The path of the tree, its commit, the pinned commit with the build tag and the SDK version, that the bindings match only that commit, the commands `git -C <tree> checkout <commit>` and `git -C <tree> submodule update --init --recursive`, and the other remedy, the version of the crate that matches the tree |
| `git status`, `git ls-files`, or `ninja --version` exits with a failure (SYS-4) | The command, the directory when the command has one, the exit status, or that a signal ended the program, and its error output |
| `git status` reports a change (SCS-27) | The path of the tree, the pinned commit, the command, the first ten lines that it printed, each in quotation marks, and the number of further lines, that the build copies the tracked files and a changed file would build another library than the one that the bindings match, and the remedies: commit, stash, or discard the changes, or point `SLANG_SOURCE_DIR` at an unchanged checkout of the commit |
| A required submodule has no gitlink or no `.git` entry (SCS-45) | The path of the submodule, the path of the tree, that the build needs it, and the command `git -C <tree> submodule update --init <submodule>`, which needs network access |
| A file of the copy cannot be listed or removed, the stamp or the log cannot be written or removed, or the log cannot be opened for a `cmake` run (SCS-45, SCS-46, SCS-48) | "cannot" with the action and the path, the reason that the operating system gives, and that the build keeps the copy, the build tree, and the log under `OUT_DIR` |
| A file cannot be copied (SCS-45) | "cannot copy" with the source path and the destination path, the reason that the operating system gives, and that the build copies the tracked files into `OUT_DIR` because Slang's build writes into its source tree |
| The configure or build step of `cmake` exits with a failure (SCS-48) | The command, the exit status, or that a signal ended the program, the name of the step, the path of the log, and its last 40 lines, or that the log cannot be read with the reason |
| A static library is not a file, or the version header of the build tree cannot be read, lacks `SLANG_TAG_VERSION`, or defines it malformed (SCS-47) | "the build of Slang from source finished, but", the problem with the path, and that the build produced another layout than the one that this version of the crate expects from the pinned commit |
| The version header of the build tree defines another build tag (SCS-47) | The build tag, the path of the header, and the pinned build tag, with the reason that the bindings match only that build |

A variable is present when the environment contains it, whatever its value. An empty `SLANG_SOURCE_DIR` selects the static route and stops the build there, because a user who set the variable asked for that route, and ARCH-7 forbids a fallback. The error type `BuildError` of `route.rs` holds the message, and its `Debug` implementation writes the message unchanged, so the error that `main` returns prints as readable text and not as an escaped string. The build script emits no `cargo::warning`. It writes the output of `cmake` and Ninja to its log, and it shows that output only in the error of a failed configure or build step (SCS-48).

On 2026-11-10, each message of the dynamic route was observed in the development container: with `VULKAN_SDK` unset, with it empty, with a relative path, with a directory that does not exist, with a header that defines another build tag, with a header that lacks the definition and one that defines it malformed, with an SDK without `lib`, with a `lib` directory without the library file, and with a `lib` directory that cannot be read. On the static route on that date, these failures were observed on scratch trees: an empty and a relative `SLANG_SOURCE_DIR`, a directory that does not exist, a tree without `.git`, a directory inside another repository, a clone at another commit, a `PATH` without git, without Ninja, and without CMake, a configure step that failed because `CC` named a compiler that does not exist, a changed tracked file, a changed submodule, and an empty submodule directory. A real failure of the build step could not be produced, so only its test covers it. This example shows the message when `VULKAN_SDK` is unset:

```text
VULKAN_SDK is not set, so slang-compiler-sys cannot find the Slang library to link. Install the LunarG Vulkan SDK 1.4.363.1 from https://vulkan.lunarg.com, and set VULKAN_SDK to the absolute path of its directory that holds include/slang and lib, which is the x86_64 directory of the SDK on Linux, the macOS directory on macOS, and the installation directory on Windows. The other way to obtain Slang is to build it from source: set SLANG_SOURCE_DIR to the absolute path of a git checkout of Slang at the commit c7954ebd1f1984ad2bfde15f76248db41dc34aa3.
```

### 9.7. Structure and Tests of the Build Script

The build script follows SYS-8 of the coding standard. It consists of three files that `build.rs` includes with `include!`, in this order: `pin.rs`, `route.rs`, and `system.rs`.

- `crates/slang-compiler-sys/route.rs` holds the logic, as plain functions without `main`. The test module `tests/sys/build_script.rs` includes it as well.
- `crates/slang-compiler-sys/system.rs` holds the struct `System`, which implements the trait `Host` on the real environment, file system, and processes with the standard library only. The test module `tests/sys/build_script/ffi.rs` includes it as well.
- `build.rs` calls `configure` with `System`, prints each collected instruction, and returns the result from `main`.

The logic reaches the environment, the file system, and other programs only through the private trait `Host`, which has ten methods:

- `var`, which returns the value of an environment variable as an `OsString`, or nothing when the variable is absent.
- `read`, which returns the text of a file.
- `kind`, which says whether a path is a directory, a file, or something else, following symbolic links, or returns the error of the file system.
- `host_os`, which names the operating system of the host in the form of `CARGO_CFG_TARGET_OS`.
- `run`, which runs a program described by the struct `Invocation`, with its arguments, its working directory, and the variables to set and to remove, and returns its exit status and both of its output streams.
- `run_to_log`, which runs a program in the same way, appends both of its output streams to a log file, and returns only its exit status. Its error is the enumeration `LoggedRunError`, whose variant `Log` says that the log file could not be opened or its handle duplicated, and whose variant `Start` says that the program could not be started. The logic reports the first as a failure to open the log and the second as a program that cannot start (Section 9.6).
- `files`, which lists every file under a directory, relative to it.
- `sync_file`, which copies a file, creating the parent directories, and writes it only when its bytes differ from the existing copy.
- `remove_file`, which removes a file.
- `write`, which writes a text to a file, creating the parent directories.

The function `configure` takes a host and a vector of instructions, appends each Cargo instruction to the vector, and returns `Result<(), BuildError>`. `System` implements the methods with `std::env::var_os`, `std::fs`, `std::process::Command`, and `std::env::consts::OS`. It starts each program with standard input closed. `build.rs` has no dependencies and no build dependencies (SCS-38).

The test module `build_script` includes `../../route.rs` and implements `Host` on a fake that holds lists of variables, of file contents, and of path kinds or errors, the name of a host operating system, and a function that answers each program run. It records each variable that the logic reads, each program that it runs, and each file that it copies, removes, or writes. A path that the fake does not list is not found. On 2026-11-10, it holds 46 tests:

- 12 tests of the dynamic route and the documentation build: the exact list of instructions on success, the documentation build, an absent, empty, relative, non-UTF-8, and line-breaking `VULKAN_SDK`, a header that cannot be read, a missing, malformed, and other build tag, the last definition and CRLF lines, each case of the library check for the host's operating system, and a foreign or unknown target.
- 33 tests of the static route: the exact list of instructions without a read of `VULKAN_SDK`, the programs with their arguments and environment, the copy and the stamp written after the build, a matching stamp that skips the copy and the configure step, the removal of stale files, the archives and the C++ standard library of each supported target, every failure of Section 9.6 that the static route raises, the cut-off of the changed files at ten lines, a signal that ends a program, a log that cannot be opened, which is not reported as a program that cannot start, a `cmake` step that cannot start, a log that cannot be read, a missing job count, and a stale stamp that stays removed after a failure.
- 1 test of the numeric version of the build tag.

Each test asserts the content of each message and the instructions that the logic emits, including the rerun instructions on every failure path. Each test of the static route also asserts that `VULKAN_SDK` was not read, and each test of a failure asserts that no stamp was written. These tests are pure Rust, so they run under nextest and under Miri.

The module `build_script::ffi` includes `../../../system.rs` and tests `System` itself with 8 tests. They read a variable, the kinds of paths, and the operating system of the host, list the files of nested directories, write and remove files and create their parents, keep an unchanged file and rewrite a changed one, copy the tracked files and remove stale ones through `sync_copy`, run `rustc` and capture its exit status and output, append the output of a logged run to its log, and tell a log that cannot be opened, which `run_to_log` returns as `LoggedRunError::Log`, from a program that cannot start. They work in `CARGO_TARGET_TMPDIR`. They start a process, so they sit in a module named `ffi`, which Miri skips (SCS-33), and they run on Linux, Windows, and macOS. Section 7.5 gives the test counts of the workspace.

`check-fmt` passes every Rust file that git tracks or does not ignore to rustfmt, including `pin.rs`, `route.rs`, and `system.rs`, which `cargo fmt --all --check` alone does not read, because a crate includes them with `include!` (Section 4.8 of the coding standard). This answers SQ-2.

## 10. Finding the Shared Library at Run Time

On the dynamic route, the dynamic loader of the operating system finds the Slang shared library when the program starts, as Section 4.4 of the architecture design states, and the build script emits no rpath (SCS-26). That the build script of a dependency cannot set the rpath of the final binary is assumption A-6 of the architecture design.

SCS-32. Every recipe of the `justfile` that runs compiled code which loads Slang MUST start the command line of the program that it runs with the assignment that the `justfile` variable `slang_library_path` holds. That assignment puts the library directory of `VULKAN_SDK` first on the library path of the platform, which is `LD_LIBRARY_PATH` on Linux and `DYLD_LIBRARY_PATH` on macOS, followed by any earlier value. The `justfile` MUST NOT export that path. The `justfile` MUST leave the variable empty on Windows and when `VULKAN_SDK` is unset or empty. These requirements hold for three reasons. The tests that call Slang fail before any test runs when the loader cannot find the library. The development container provides the path only to processes that a login shell starts (Section 11 of the tooling design), and macOS with System Integrity Protection enabled is expected to remove an exported `DYLD_*` variable when it starts the protected shell that runs a recipe. Finally, `check-doc-without-slang` and every recipe that does not load Slang have to work without `VULKAN_SDK`.

The `justfile` computes the variable with the `just` functions `os()`, `env()`, and `quote()`, so it holds the assignment as text and no recipe exports anything. This example shows the command that `just test` runs on Linux when `VULKAN_SDK` is `/tmp/vulkan-sdk/x86_64` and `LD_LIBRARY_PATH` is empty:

```sh
LD_LIBRARY_PATH='/tmp/vulkan-sdk/x86_64/lib' CARGO_BUILD_WARNINGS=deny cargo nextest run --workspace --all-features --profile default --no-tests=warn
```

On Windows, and when `VULKAN_SDK` is unset or empty, the variable is empty and the command starts with `CARGO_BUILD_WARNINGS=deny`. The recipes that carry the assignment are `test`, `test-gpu`, `test-hw-gpu`, `check-doctest`, `check-bindings`, and `coverage`. Each runs test binaries or doctests that load Slang, and `check-bindings` carries it because nextest starts every test binary of the workspace to list its tests, including the test binary of `slang-compiler-sys`. `check-miri` does not carry it, because Miri runs no native code, and on 2026-11-10 it passed without it. The recipes that only compile, such as `check-clippy`, `check-doc`, `check-doc-without-slang`, `check-features`, and `check-msrv`, run no compiled code and do not carry it. TOOL-7 of the tooling design permits the assignment in front of a command of Section 4.8 of the coding standard.

On the static route, a program needs no Slang library at run time, because the build links Slang statically (Section 9.3). `test-static` therefore carries no assignment and removes `VULKAN_SDK`, `LD_LIBRARY_PATH`, and `DYLD_LIBRARY_PATH` instead (SCS-49). The statically linked Slang still loads a `slang-glslang` shared module by name when the dynamic loader finds one (Section 16).

Before step 4 of Section 15, this section listed three options, each expected to survive System Integrity Protection on macOS: the assignment on the command line of the program that a recipe runs, a symbolic link to the library inside `OUT_DIR`, which Cargo adds to the dynamic library path of the programs that it runs, and an rpath. Step 4 chose the first. The shell that runs a recipe sets an assignment on its own command line for the program that it starts, so the variable reaches Cargo, which is not protected, and every program that Cargo starts inherits it. An exported variable would pass through the start of the protected shell, which is expected to remove it. The third option is forbidden by SCS-26 and, by A-6 of the architecture design, is assumed to be out of reach of a dependency's build script. The second would make the build script of every user create a link that only the recipes of this repository need.

On every CI runner, the step that installs the SDK also sets the path that the loader of the platform reads (Section 8.3 of the tooling design). On the Linux and macOS runners, the directory therefore appears twice on the library path of a recipe, once from the install step and once from the assignment, which is harmless, because the second entry names a directory that the loader has already searched. On 2026-11-10, on the hosted Windows runner, a program loaded `slang-compiler.dll` by name with this `PATH` when it was started directly, through bash, through `just`, and through `cargo run`. On the hosted Linux runner, `LD_LIBRARY_PATH` reached a program in each of these cases. The test binary of `slang-compiler-sys` then ran on Linux, Windows, and macOS through `test-gpu` (Section 11). The crate documentation tells users what to set on each platform (Section 13).

SA-7 remains open. System Integrity Protection was disabled on the hosted macOS runner on 2026-11-10, and both the install step and the recipe set `DYLD_LIBRARY_PATH` there, so the log of the run cannot show which of the two reached the test binary. On that runner and date, `DYLD_LIBRARY_PATH` set through `GITHUB_ENV` reached every process: one started directly, and one started through bash, `/usr/bin/env`, `just`, or `cargo run`. On a developer's Mac with System Integrity Protection enabled, macOS is expected to remove the `DYLD_*` variables when it starts a protected binary such as `/bin/bash` or `/usr/bin/env`, and the assignment on the command line is expected to survive this, because the protected shell sets it after it has started. Neither expectation was tested. What remains unverified is that `just test` passes on a Mac with System Integrity Protection enabled and without `DYLD_LIBRARY_PATH` in the environment of `just`, which the owner, or a developer with such a Mac, can confirm by running it. Without the variable, loading the library fails, because dyld then searches only system locations and the current directory. `DYLD_FALLBACK_LIBRARY_PATH` also works, but macOS is expected to remove it in the same way, and a symbolic link in `/usr/local/lib` does not work. `cargo run` itself sets `DYLD_FALLBACK_LIBRARY_PATH` to its target directories followed by `~/lib:/usr/local/lib:/usr/lib`.

## 11. Tests and Miri

SCS-33. Every test that calls native code, whether Slang or libclang, MUST sit in a module named `ffi`, and the nextest profile `default-miri` MUST exclude every test whose path contains a module named `ffi`, `gpu`, or `hw_gpu`, because Miri cannot call foreign code, and a test that reaches a foreign call under Miri fails without a defect in the code.

This answers TQ-5 of the tooling design. The selection by module name works like the GPU tiers (Section 5 of the tooling design). The `ffi` tests stay in the default test tier, so `test` and `test-gpu` run them. `check-miri` keeps the nextest form of Section 4.8 of the coding standard, which runs each test in its own process, because `cargo miri test` aborts the whole test binary at the first foreign call. The spike showed that `cargo +nightly miri nextest run` builds without linking the native library and passes when a filter excludes the tests with foreign calls. The build script still runs natively under Miri, so `check-miri` needs `VULKAN_SDK`. On 2026-11-10, Miri built the test binary of `slang-compiler-sys` with the real build script, which emits link instructions, in the development container and in CI, and needed no library path, which confirms what was assumption SA-9.

The crate `slang-compiler-sys` has one integration-test binary (TEST-10), `tests/sys/`, whose root is `tests/sys/main.rs`. It declares four modules:

- `pin`: Three pure tests of the consistency of the pin constants: the commit is a full lowercase hexadecimal hash of 40 characters, the build tag names a release and, when it has a commit count, abbreviates the commit, and the SDK version has four numeric components.
- `build_script`: The pure tests of the logic of the build script, and its module `ffi`, which tests the real host of `system.rs` and starts processes (Section 9.7).
- `ffi`: The smoke test and the native-symbol test, which call Slang. The declaration `mod ffi;` carries `#[expect(unsafe_code, reason = "...")]`, whose reason says that the tests of the module call Slang through the raw bindings, which only unsafe code can do. This is the narrowest scope that needs the exception (LINT-1), and Section 4.4 of the coding standard lists it among the expected kinds of lint exception.
- `symbols`: The generated list of C functions of Section 5.1, without a lint exception (Section 5.10).

The tests of `pin` and of `build_script` outside its module `ffi` call no native code and start no process, so they run under Miri. Section 7.5 gives the test counts.

The smoke test is `global_session_reports_the_pinned_build_tag_and_releases_every_reference`. It creates a global session through `slang_createGlobalSession2` with `SlangGlobalSessionDesc::default()` and asserts the result `SLANG_OK` and a non-null session. It asserts that `spGetBuildTagString` and `IGlobalSession::getBuildTagString` both return the build tag of the pin (SCS-34). It queries the interface `ISlangUnknown` through `queryInterface` with the generated UUID and asserts that the result is the same pointer as the session. It then releases the interface pointer, which leaves a count of 1, and the session, which leaves a count of 0. Since 2026-11-11, the test function carries `#[expect(ffi_unwind_calls, reason = "...")]`, because it calls C functions declared `"C-unwind"` (SCS-11), and its reason says that an exception that escaped one of them would unwind into the test, which is defined and fails it. On that date, `test-static` ran 59 tests of `slang-compiler-sys` and all passed, so the unwinding ABIs change nothing on the static route either.

The native-symbol test is `every_c_function_of_the_bindings_resolves_to_a_native_symbol`, the second test of TEST-8. It asserts that the generated list `C_FUNCTIONS` is not empty and that the address of every function in it is not null, and it reads each address through `std::hint::black_box`, so the compiler keeps every reference. The test binary therefore imports every C function that the bindings declare, 193 on 2026-11-10, and a function that the library does not export fails the link of the test binary with an error such as "undefined symbol". The test does not cover the methods of the interfaces, because a method is a slot of a vtable that Slang fills at run time, not a symbol of the library, so only the smoke test and the layout assertions of SCS-12 exercise them.

On 2026-11-10, both tests passed in the development container and in the Linux, Windows, and macOS jobs of CI (Section 12). On the same date, both passed on the static route through `test-static` in the development container, where the native-symbol test linked against the static libraries (Section 9.3).

When the loader cannot find the shared library, the test binary stops before any test runs. On Linux, on 2026-11-10, a test binary started without the library path printed "error while loading shared libraries: libslang-compiler.so.0.2026.17: cannot open shared object file: No such file or directory" and exited with status 127. The recipes avoid this by supplying the library path (Section 10), and a user who runs a program outside them sets the path as the crate documentation says (Section 13).

SCS-34. The smoke test MUST assert that `spGetBuildTagString()` returns the build tag of the pin, on both routes, because a library that loads but is not the pinned build is the failure that SYS-6 and SCS-25 exist to prevent, and the run-time comparison catches a library that the loader found somewhere other than where the build script checked.

The layout assertions of SCS-12 are compile-time checks and need no test. The tests of the generator are those of Section 7.5.

## 12. Continuous Integration

SCS-35. From the step of Section 15 that first needs it, the Linux, Windows, and macOS jobs of the CI workflow MUST install the pinned Vulkan SDK from LunarG, verify the download against the SHA-256 checksum that LunarG publishes, with the expected value recorded in the workflow (Section 8.3 of the tooling design), set `VULKAN_SDK`, and put the SDK's library directory on the library path of the platform, because every job compiles `slang-compiler-sys` from the step that adds its build script, and every job that runs tests starts binaries that load Slang (TOOL-21, TOOL-23).

Step 2 of Section 15 implemented SCS-35 in every job of the workflow, on Linux, Windows, and macOS. Section 8.3 of the tooling design specifies the mechanism on each platform, including the download address, the installers, the recorded checksums, and the decision not to cache, and it meets TOOL-21, TOOL-23, ARCH-9, and SCS-23: each job reads the pin through `just pin`, so the workflow does not restate the version. After the installation, each job runs `just check-vulkan-sdk`, which checks the headers, the library, and `slangc` of the SDK against the build tag of the pin. The CI runs of that step confirmed SA-1 (Section 9.2), and confirmed the part of SA-2 that concerns the name and location of the import library and the DLL on `PATH` (Section 10). The `check` and `coverage` jobs on Linux also install libclang and the rustfmt component before they run the tests, so that the generator's tests (Section 7.5) and the regeneration test, which also reads the headers of the installed SDK (Section 7.6), find what they need. Section 8.1 of the tooling design specifies that step and states how it meets TOOL-21.

Step 4 of Section 15 needed no change to the workflow, because every job already installed the SDK and set the paths that the build script and the test binaries need. On 2026-11-10, all six jobs passed on a draft pull request with step 4. Every job compiled `slang-compiler-sys` with its build script against the installed SDK. The Windows and macOS jobs built and linked the test binary of `slang-compiler-sys` against the Slang of the SDK and ran the smoke test and the native-symbol test (Section 11), which confirms what were SA-2 and SA-3 (Sections 6 and 9.2). Section 8.1 of the tooling design records the durations of that run.

SCS-36. The CI workflow MUST NOT build the static route on every push or pull request that it runs on, because the route compiles several hundred C++ files, which would multiply the time of every run for a route that changes rarely.

This answers Q-28 of the architecture design. The static route is verified locally through the recipe `test-static` (Section 9.3), which ran in the development container on 2026-11-10. Whether a scheduled CI job builds it is open (Section 17, SQ-4).

Step 5 of Section 15 needed no change to the workflow. On 2026-11-10, all six jobs passed on draft pull request 6, in run 42818251079. The jobs build the dynamic route only, and the tests of the real host of Section 9.7 ran in the Linux, Windows, and macOS jobs. That run predates the eighth of those tests, so it ran 7. On `main`, CI then failed in the Windows job on 2026-11-10 and 2026-11-11 for three commits, from the commit that added the static route to the commit that added `docs/design/slang-compiler.md`, because one test of the real host asserted an error kind that Windows reports differently. The commit "Assert only the variant of a start failure of the real host" fixed the test, and run 42833561065 on `main` passed all six jobs.

## 13. Crate Documentation

SCS-37. The crate-level documentation of `slang-compiler-sys` MUST contain the content of Sections 7.4 and 7.9 of the writing guide and MUST state the Slang rules of Section 3.9, because the generated items have no documentation of their own (SCS-13), and the design document of the safe crate cites these rules for its `Send`, `Sync`, and lifetime decisions. That content includes a `# Features` section that says the crate has no features, and a `# Platform support` section that names the supported targets of Section 6. It also includes how the crate finds or builds Slang on each route, how the library is found at run time on each platform, and the safety contract that every thunk and C function shares, with what happens when a C++ exception escapes a call.

The shared safety contract is the one that Section 7.9 of the writing guide requires, and it is what lets the generated items go without `# Safety` sections and safety comments (Section 5.10). It says that the caller passes only pointers that are valid, correctly typed, live, and obtained from Slang, such as a live interface pointer of the declared interface or a valid handle, that the caller meets the contract that Slang documents for the function, that the caller follows the threading and lifetime rules of Section 3.9, and that a call that breaks any of these is undefined behaviour. The documentation states that the crate promises nothing about threads: every function and thunk is `unsafe`, and no interface struct is `Send` or `Sync`. It links to Slang's documentation for the meaning of each item and points users to `slang-compiler`.

The example of the crate-level documentation creates a global session and is marked `no_run`, because it needs the shared library at run time (Section 7.5 of the writing guide). The documentation of the hand-written public items, which are the constants of the pin, follows Section 7 of the writing guide in full.

Step 3 of Section 15 wrote the crate-level documentation that the lint exception of Section 5.10 requires, step 4 replaced its example and its section on linking, step 5 added the static route to that section, and step 6 checked it against SCS-37 and Section 7 of the writing guide and completed it. On 2026-11-10, it has these parts:

- A summary of what the crate is, with SPIR-V, Vulkan, vtable, and UUID defined on first use, a paragraph that says when to use the crate and that `slang-compiler` wraps the bindings in a safe API that compiles shaders and does not reflect them yet, and that the planned crate `slango` will build on it, a link to Slang's user guide for the meaning of each item, how each kind of C++ declaration appears in Rust, and the constants of the pin with what the pin means for a user: one version of the crate works with exactly one Slang build, and the build script refuses any other.
- `# Examples`: two doctests. The first calls no Slang function and runs. It shows the default values of `SlangGlobalSessionDesc`, an enumeration value that the header does not list, and the sign of the result codes `SLANG_FAIL` and `SLANG_OK`. The second is marked `no_run`. It creates a global session through `slang_createGlobalSession2`, reads its build tag through `IGlobalSession::getBuildTagString`, compares it with `SLANG_BUILD_TAG`, releases the session, and returns an error when the session cannot be created or the tag differs. Its lead-in says that it needs the Slang shared library at run time, which the dynamic loader must find as `# Linking Slang` describes, so it is compiled but not run. Each `unsafe` block in it has a safety comment.
- `# Features`: the crate has no Cargo features.
- `# Platform support`: the four supported targets of Section 6, the stop on a target whose pointers are not 64 bits wide, and what has run on each target: the dynamic route with its tests on x86_64 Linux, x86_64 Windows, and aarch64 macOS, only a type check on x86_64 macOS (Section 6), and the static route on x86_64 Linux only (SQ-3).
- `# Linking Slang`: both routes, for a user. It says that `SLANG_SOURCE_DIR` selects the static route, that no Cargo feature selects a route, and that the build never falls back from one route to the other. For the dynamic route as Section 9.2 specifies it, it names the LunarG Vulkan SDK of `VULKAN_SDK_VERSION` and its download address, says which directory `VULKAN_SDK` names on each platform, and describes the check of the build tag with no setting that skips it, the link from `$VULKAN_SDK/lib`, the failures that stop the build, and the rule of SCS-41 for a target of another operating system. For the static route as Section 9.3 specifies it, it says that `SLANG_SOURCE_DIR` names the top level of a git clone at `SLANG_COMMIT` with its submodules, lists the checks that git performs and the refusal of a source archive, says that the build script never changes the clone, names git, CMake 3.22 or later, Ninja, and a C++ compiler as requirements, and describes the copy, the build, the log, the rebuild after a change, the check of the build tag, the six static libraries with the C++ standard library of each target, the supported targets, and the refusal of cross-compilation. It says that the static build leaves out `slang-glslang`. When the dynamic loader finds no `slang-glslang` shared library, Slang does not optimize SPIR-V and does not validate it even when the environment variable `SLANG_RUN_SPIRV_VALIDATION` is `1`. The pinned Slang validates SPIR-V on neither route unless that variable is `1` (Section 16). The first generation of SPIR-V in each global session then succeeds with diagnostics that begin `error[E00100]: failed to load downstream compiler 'spirv-opt'` and name the libraries that Slang could not load. Later generations in the same global session succeed with no diagnostics. It also says that output as SPIR-V assembly fails, that the statically linked Slang still loads a `slang-glslang` shared library by name when the loader finds one and then behaves as the dynamic route, so that the output depends on the environment, and that the standard modules are not built. It then says that `DOCS_RS` builds the documentation without Slang and gives the `links` metadata of each route. Finally, it says which variable the dynamic loader reads on each platform, that macOS removes the `DYLD_*` variables when it starts a protected program, that an assignment on the command line of Cargo is expected to avoid that and has not been verified (SA-7), and what the loader reports on Linux when it cannot find the library.
- `# Safety`: the shared safety contract, the rules of Section 3.9 with reflection and component type defined, what happens when a C++ exception escapes a call, that no interface struct implements `Send` or `Sync`, and the promise that an implementation of the unsafe trait `Interface` makes. The paragraph on exceptions, which step 3 of Section 15 of `docs/design/slang-compiler.md` added on 2026-11-11, says that Slang raises its internal errors as C++ exceptions and that some calls of the pinned build let one escape for some inputs. It says that every foreign function is declared `"C-unwind"` and every vtable entry `"system-unwind"`, which name the same calling conventions as `"C"` and `"system"`, so an escaping exception is not undefined behaviour. With `panic = "unwind"`, the exception unwinds through the caller's Rust frames and runs their destructors, and the process then aborts at `catch_unwind` or at the root of the thread, or the exception is returned as an opaque error, which Rust leaves unspecified. With `panic = "abort"`, the process aborts as soon as the exception reaches a Rust frame. A caller that must not run its destructors during such an exception stops the unwinding itself, such as with a value whose destructor aborts the process. The type of a callback keeps the ABI `"C"`, so a panic in a callback aborts the process.

## 14. Dependencies and Licences

SCS-38. The published crate `slang-compiler-sys` MUST have no dependencies and no build dependencies, because every dependency of a `-sys` crate runs on every user's machine, and the build script needs only the standard library to read a header, start processes, copy files, and compare strings. The static route runs `cmake` through `std::process::Command` and uses no `cmake` crate.

The generator depends on the crate `clang` 2.1.0, and through it on `clang-sys`, `glob`, `libc`, `libloading`, `cfg-if`, and, on Windows, `windows-link`. These licences were verified with `cargo info` and `cargo tree` on 2026-11-10:

- `clang` and `clang-sys`: Apache-2.0.
- `libloading`: ISC.
- `glob`, `libc`, `cfg-if`, and `windows-link`: MIT OR Apache-2.0.

The licence allow-list of `deny.toml` is `GPL-3.0-or-later`, `Apache-2.0`, and `ISC`, which is exactly what cargo-deny needs for this graph (TOOL-24). Apache-2.0 covers `clang`, `clang-sys`, and the crates under `MIT OR Apache-2.0`, which cargo-deny accepts through either licence, so MIT is not in the list. ISC covers `libloading`. Apache-2.0 and ISC are compatible with GPL-3.0-or-later. The graph has no duplicate version, so `deny.toml` has no exception. The dependency is declared once in `[workspace.dependencies]` as `clang = { version = "2.1.0", features = ["runtime", "clang_10_0"] }` (DEP-2). `check-msrv` verifies that it builds with the workspace's `rust-version`.

## 15. Implementation Order for M1

M1 is carried out as these changes, in this order. Each change updates the documents that Hard Rule 1 of `AGENTS.md` requires, and leaves `just check` passing.

1. Done. This step added `crates/slang-compiler-sys/pin.rs`, its inclusion and documentation in the crate root, and the recipe `pin` (Section 8.1). It added the generator's library, binary, rule set, and shim headers, with the check of the build tag of the headers against the pin (Section 7.2) and the unit tests on synthetic headers (Section 7.5). It also added the integration-test binary of `slang-compiler-sys` with its module `pin` (Section 11), the dependency on `clang` with its licences in `deny.toml` (Section 14), the `[profile.default-miri]` table of SCS-33 in `.config/nextest.toml`, and the installation of libclang in the Linux CI job (Section 12). It confirmed SA-10. In this step, the generator ran on the headers of the pin only in a scratch crate (Section 3.11), and step 3 checked in its output.
2. Done. This step added the installation of the pinned SDK, verified against a checksum that the workflow records, to every job of the CI workflow on Linux, Windows, and macOS (SCS-35), with a step that reads the pin through `just pin`, and the check of the installed SDK against the pin (Section 12). It added the scripts `scripts/install-vulkan-sdk` and `scripts/check-vulkan-sdk` with their recipes, and the CI job `coverage`, and it rewrote the recipe `pin` so that it runs on the macOS and Windows runners (Section 8.1). It answered TQ-1 of the tooling design, confirmed SA-1, confirmed part of SA-2, and recorded why the hosted macOS runner cannot confirm SA-7 (Section 10).
3. Done. This step checked in the generated bindings `crates/slang-compiler-sys/src/bindings.rs`, which the generator's binary wrote in the repository and which carry the `compile_error!` of SCS-15 at their top (Section 5.1). It added the module declaration with its lint exception, verified on the four supported targets (SCS-13 and Section 5.10), the opt-out of `unsafe_code`, the crate-level documentation with the shared safety contract that the exception requires (Section 13), the three entries of SCS-14 in `typos.toml`, and the docs.rs targets of Section 9.4. It added the regeneration test and the recipes `generate-bindings` and `check-bindings` (Section 7.6), and tests for six refusals of SCS-19 (Section 7.5). It also added the root `.gitattributes`, which keeps LF line endings in every checkout, and corrected `scripts/check-vulkan-sdk` and the usage text of `scripts/install-vulkan-sdk` (Sections 3.4 and 8.3 of the tooling design). It confirmed no assumption of Section 16. After this step, the crate still linked nothing, and the native-symbol test of TEST-8 waited for step 4 (Section 11).
4. Done. This step added the build script `build.rs` with its logic in `route.rs` (Section 9.7): route selection, with a stop on `SLANG_SOURCE_DIR` until step 5 (Section 9.1), the dynamic route with the validation of `VULKAN_SDK`, the check of the build tag, and the check of the library directory and file (SCS-25, SCS-26, SCS-40, and SCS-41), the documentation build (Section 9.4), the rerun instructions and `links` metadata (Section 9.5), and the messages of Section 9.6. It added the key `links = "slang-compiler"`, the test module `build_script`, the smoke test and the native-symbol test in the module `ffi` (Section 11), the second generated file `tests/sys/symbols.rs` with the generator's new API and the regeneration of both files (Sections 5.1, 7.4, and 7.6), the library path on the command line of the recipes that run compiled code (SCS-32), and the recipe `check-doc-without-slang` in `check-rust` (Section 9.4). It replaced the example and the section on linking of the crate documentation (Section 13), added `LunarG` to `doc-valid-idents` in `clippy.toml`, and changed the usage text of `scripts/install-vulkan-sdk` and `scripts/check-vulkan-sdk` to say "unset or empty". It confirmed SA-2, SA-3 for x86_64 Windows and aarch64 macOS, and SA-9, and left SA-7 open (Sections 10 and 16). It added SQ-2 (Section 17).
5. Done. This step added the static route in `route.rs` (Section 9.3) with SCS-27 and SCS-28 amended and SCS-42 to SCS-49 added, moved the implementation of `Host` on the real system from `build.rs` into the new file `system.rs` with its tests in `tests/sys/build_script/ffi.rs` (Section 9.7), and grew `Host` to ten methods. It added the 33 tests of the static route, the recipe `test-static` (SCS-49), the static route in `# Linking Slang` of the crate documentation (Section 13), and a new message for an absent `VULKAN_SDK` that names the static route (Section 9.6). It made `fmt` and `check-fmt` pass every Rust file that git tracks or does not ignore to rustfmt, which answers SQ-2 (Section 9.7). It confirmed SA-4 and SA-5, corrected SA-6, answered SQ-1, and added SQ-3 to SQ-5 (Sections 16 and 17).
6. Done. This step checked the crate-level documentation against SCS-37 and Section 7 of the writing guide and completed it (Section 13): it defined the terms that lacked a definition, added when to use the crate, what the pin means for a user, a second example that calls no Slang function, and what has run on each target. It said that the `DYLD_LIBRARY_PATH` assignment on macOS is unverified, and it corrected the statements on the reason in the error for `$VULKAN_SDK/lib` and on atomic reference counting. It checked that `README.md` says how to obtain Slang on both routes, and found that the lints that fire on the generated module match the list of Section 4.4 of the coding standard, so that section did not change. It amended SYS-1 of the coding standard to cover the watched git files of the static route (Section 9.5), and updated `AGENTS.md`, `README.md`, and the architecture design. M1 was complete on 2026-11-10. What stays open is in the rows of Sections 16 and 17.

This order differs from a plain sequence of generator, bindings, build script, and CI for four reasons. The pin file arrives in step 1 because the generator reads it to check the headers before it parses them (Section 7.2). The `default-miri` profile arrives in step 1 because the generator's tests are the first that call native code, and `check-miri` would fail on them without it. The SDK arrives in CI in step 2, on all three platforms at once, because the regeneration test of step 3 needs the headers in the Linux job, and the build script of step 4 needs the SDK in every job that compiles the crate, which includes the Windows and macOS jobs through Clippy and the tests. `check-doc-without-slang` arrives in step 4 and not with the documentation in step 6, because step 4 adds the code path that it tests.

## 16. Assumptions to Confirm

Each row is a fact that this design relies on and that neither the survey nor the spike verified. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SA-7 | On macOS with System Integrity Protection enabled, the library path that a recipe places on the command line of the program it runs reaches the test binary, although macOS is expected to remove the `DYLD_*` variables when it starts a protected system program such as the system shell | SCS-32 | A run of `just test` on a Mac with System Integrity Protection enabled and without `DYLD_LIBRARY_PATH` in the environment of `just`, by the owner or a developer with such a Mac. The hosted macOS runner cannot confirm it, because System Integrity Protection was disabled there on 2026-11-10, and Section 10 records what was observed |
| SA-8 | docs.rs honours `targets` in `[package.metadata.docs.rs]` and builds no other target | Section 9.4 | The first docs.rs build, in M7 |

Step 1 of Section 15 confirmed SA-10, that a preparation step can install libclang on the Linux CI runner where `clang-sys` finds it, and removed its row. Section 7.5 states the result. Step 2 of Section 15 confirmed SA-1, that the Vulkan SDKs for Windows and macOS ship the Slang headers, including `slang-tag-version.h`, under `include/slang`, and the library under `lib`, as on Linux, which is A-8 of the architecture design, and removed its row. Sections 3.6, 3.7, and 9.2 state the result.

Step 4 of Section 15 confirmed three assumptions on 2026-11-10 and removed their rows. SA-2, that a Rust crate links against the import library `slang-compiler.lib` on Windows through `rustc-link-lib=dylib=slang-compiler` with the search path `$VULKAN_SDK/lib`, was confirmed in the Windows job of CI, and Section 9.2 states the result. SA-3, that the generated bindings link and the smoke test passes on Windows and macOS, was confirmed for x86_64 Windows and aarch64 macOS, where both tests of the module `ffi` passed, and Section 6 states the result and what remains for x86_64 macOS. SA-9, that Miri builds the test binary of `slang-compiler-sys` once the real build script emits link instructions, was confirmed by `check-miri` in the development container and in CI, and Section 11 states the result. SA-7 stays open.

Step 5 of Section 15 settled three assumptions in the development container on 2026-11-10 and removed their rows:

- SA-4, that the static route links exactly the libraries that the CMake target of the library links, including those that Slang's building documentation lists, is confirmed. The link needs six archives, which are the four of the documentation, `libslang-compiler.a` itself, and `libcmark-gfm.a`, which the documentation omits, followed by the C++ standard library. Without `libcmark-gfm.a`, the link fails with 11 undefined references. Sections 3.10 and 9.3 state the list and its order, which also answers Q-3 of the architecture design.
- SA-5, that disabling the options of Section 9.3 avoids every download at configure time and at build time, is confirmed for the tested configuration on x86_64 Linux. A configure and build with those options ran in a network namespace without interfaces, and the log of the route held no download. With the default options, an offline configure tries to download and then to clone DXC (Section 3.10). On Windows and macOS, SA-5 was not tested (Section 17, SQ-3).
- SA-6, that a static build without slang-glslang lacks only the GLSL pass-through and the validation of SPIR-V by glslang, is corrected. A C++ program linked against the static libraries of the research build compiled a trivial shader to SPIR-V, GLSL, HLSL, WGSL, and Metal with no file at run time. When the dynamic loader finds no `slang-glslang` shared library, the SPIR-V is not optimized, and the call that generates it still succeeds. The record of 2026-11-10 stated that Slang gave no diagnostic about it, and a run on 2026-11-11 showed the message on the first generation of SPIR-V in each global session. That run used a program on the raw bindings on x86_64 Linux. The program created one global session, one session with a SPIR-V target, and two modules loaded from source and linked. It called `getEntryPointCode` twice for the first program and once for the second. Every step before `getEntryPointCode` returned success with a null diagnostics blob. On the static build with `VULKAN_SDK` unset and no loader path, the first call returned `SLANG_OK` with SPIR-V and a diagnostics blob that holds `error[E00100]: failed to load downstream compiler 'spirv-opt'`, followed by `note[E99996]: failed to load dynamic library 'pthread'` and `note[E99996]: failed to load dynamic library 'slang-glslang-2026.17'`. The second call for the same program and the first call for the second program returned `SLANG_OK` with a null diagnostics blob. This agrees with the following reading of the implementation at the pinned commit. At the default optimization level, the generation of SPIR-V loads the downstream compiler (`source/slang/slang-emit.cpp:3530-3547`). A failed load is reported once as the diagnostic `FailedToLoadDownstreamCompiler`, E00100, and cached for the global session (`source/slang/slang-check.cpp:91-178`, `source/slang/slang-diagnostics.lua:458`). After a failed load, the block that links, validates, and optimizes the SPIR-V is skipped, and the function returns success (`source/slang/slang-emit.cpp:3548-3665`). By default, validation makes no difference from the dynamic route, because the pinned build validates the SPIR-V that it generates directly only when the environment variable `SLANG_RUN_SPIRV_VALIDATION` is `1` (`source/slang/slang-emit.cpp:3409-3431`). The run observed that with that variable set to `1` and no `slang-glslang`, the output was identical: the SPIR-V was not validated, and no further message appeared. Output as SPIR-V assembly fails. The statically linked Slang still loads a `slang-glslang` shared library by name when the dynamic loader finds one. With `$VULKAN_SDK/lib` on `LD_LIBRARY_PATH`, the same static program loaded the SDK's `libslang-glslang-2026.17.so`, and every call returned `SLANG_OK` with a null diagnostics blob, with and without the variable, as a dynamically linked build of the same program did. For a shader with dead code and identity arithmetic, the first call produced 864 bytes of SPIR-V without the library, and 740 bytes with it and on the dynamic route. For a shader with nothing to remove, the SPIR-V was the same 836 bytes in every configuration. The run did not establish whether validation runs when the library is present and the variable is `1`, because it compiled only valid shaders, and it tested no optimization level other than the default and no platform other than x86_64 Linux. The standard modules of Slang are not built. The crate documentation states these limits (Section 13).

The next assumption takes SA-11.

Assumptions A-6 and A-7 of the architecture design, that a dependency's build script cannot set the rpath of the final binary and that docs.rs sets `DOCS_RS`, also apply to this design and stay where the architecture design records them.

## 17. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SQ-3 | Does the static route work on macOS and on Windows with the Microsoft toolchain? Its code for these targets follows a reading of Slang's CMake files and has never run: the archive names and the runtime option of the Microsoft toolchain, the C++ standard library `c++` on macOS, whether macOS needs a framework, Ninja in the developer environment on Windows, and SA-5 on both (Section 9.3) | This document | M7, before the first release |
| SQ-4 | Should a scheduled CI job run `test-static`, on which platforms, and how often? Per-pull-request CI does not build the static route (SCS-36) | This document, with Section 8.1 of the tooling design | M7 |
| SQ-5 | Should the build script watch `CC`, `CXX`, and `CMAKE_TOOLCHAIN_FILE`, which CMake reads from the environment? A change of one of them after the first configure step has no effect until `SLANG_SOURCE_DIR`, the commit, or the configure arguments change (Section 9.5) | This document | M7 |

Step 5 of Section 15 answered two questions and removed their rows. SQ-1 asked whether the static route supports targets other than x86_64 Linux, and cross-compilation. The build script supports Linux with the GNU C library, macOS, and Windows with the Microsoft toolchain, and stops the build for any other target and for every cross-compilation (SCS-43). Only x86_64 Linux was run, which SQ-3 records. SQ-2 asked how the check verifies the formatting of the files that the crate includes with `include!`. `check-fmt` passes every Rust file that git tracks or does not ignore to rustfmt (Section 9.7). The next question takes SQ-6.

## 18. Security Considerations

The threat model has six parts. First, the Slang library and its headers come from the Vulkan SDK or from a source tree that the user names, and a defect in them, or a tampered copy, reaches every program that links the crate. Second, the build script runs on the user's machine with the user's permissions, reads environment variables, and, on the static route, runs `git`, CMake, and a native build of a large C++ code base from the source tree that `SLANG_SOURCE_DIR` names. Third, the binding generator trusts the headers it parses and writes Rust declarations that the compiler trusts without checking them against the library. Fourth, an agent edits the checked-in bindings by hand, or changes the generator so that it writes a wrong declaration. Fifth, the generator's dependencies, such as `clang` and `clang-sys`, run their build scripts on the machines of maintainers. Sixth, a CI job downloads the Vulkan SDK, and an attacker on the network or at the download host tampers with it.

Unsafe code and FFI invariants. The crate relies on the native library being exactly the pinned build, which the tag check of SCS-25, the commit check of SCS-27, and the run-time assertion of SCS-34 secure, and on the declarations matching the headers, which the regeneration check (Section 7.6), the layout assertions (SCS-12), and the failures of SCS-18 and SCS-19 secure. It guarantees its callers nothing beyond exact declarations: every function and thunk is `unsafe`, no interface struct is `Send` or `Sync`, enumerations and `SlangResult` are integer newtypes so that an unknown value is not undefined behaviour (SCS-7, SCS-8), and thunks take raw pointers so that no Rust reference asserts an aliasing guarantee about a native object (SCS-10). Every vtable entry and C function is declared with an unwinding ABI, so a C++ exception that escapes Slang, which the tests of `slang-compiler` observed for two methods on 2026-11-11, is defined behaviour and not undefined behaviour (SCS-10, SCS-11). It unwinds through the caller's Rust frames and runs their destructors, which can call Slang again while its state is inconsistent, so a caller that must not let that happen stops the unwinding itself, as the guard of the safe crate does (Section 11.6 of `docs/design/slang-compiler.md`). The crate documentation states the shared safety contract, Slang's rules, and this behaviour (SCS-37).

Untrusted shader source and file paths. This crate compiles nothing and reads no shader source. The safe crate decides which files Slang can read (Section 8 of `docs/design/slang-compiler.md`).

Build scripts that run native builds. The dynamic route reads one header, asks the file system whether the library directory and the library file exist, and emits instructions, and it searches no location other than `VULKAN_SDK` (SCS-26 and SCS-41). It writes the value of `VULKAN_SDK` into Cargo instructions only after it has rejected a value that is not UTF-8, is not absolute, or contains a line feed or a carriage return (SCS-40). Cargo reads one instruction per line, so without this check a value with a line break could inject an instruction of its own, such as one that links another library or adds a search path. A test of Section 9.7 passes such values and asserts that the build script emits no link instruction and that its message holds no line break. The static route applies the same checks to `SLANG_SOURCE_DIR` and `OUT_DIR` (SCS-42). It refuses a tree at another commit, a changed tree, and a tree without git metadata (SCS-27). It reads the clone only through git, with the repository variables of the environment removed and optional locks off, so git answers for the directory that `SLANG_SOURCE_DIR` names and writes nothing into it (SCS-44). On 2026-11-10, the index of `/opt/slang` was byte-identical before and after every run. The route copies the tracked files into `OUT_DIR` and builds the copy, so Slang's build, which writes into its source tree, writes only under `OUT_DIR` (SCS-45, SYS-5). It refuses a tracked path that would leave the copy, disables every component that downloads (SCS-28), and checks the archives and the build tag that the build produced (SCS-47). The route is selected only by variables that the person who builds sets (SCS-24), and no setting bypasses the version checks.

Dependencies. The published crate has no dependencies (SCS-38). The generator's dependencies are checked by cargo-deny (TOOL-24, TOOL-26), and they run only for maintainers, never on a user's machine (ARCH-2). The native library is fixed to one SDK version and one Slang commit by the pin (SCS-23), and CI verifies every SDK download against a checksum recorded in the workflow (SCS-35).

The checked-in file is a review surface of several thousand lines, which no reviewer reads line by line. The regeneration check makes a hand edit fail `just check` on Linux, so a reviewer reviews the change to the generator, its rule set, and the summary of the regenerated diff instead of the whole file. The rule set records a reason for every decision, and the generator fails on a declaration that no rule decides (SCS-2) and on a rule that decides nothing (SCS-39), so the rule set that the reviewer reads matches the headers in both directions. A change to the generator that writes a wrong declaration and the regenerated file together pass the regeneration check, which is why the generator's unit tests (Section 7.5) and the reviewer's reading of the generator change carry that risk.

These attacks are out of scope. A compromised Vulkan SDK or Slang release from LunarG or the Slang project runs with the full privileges of the program that links it, and no crate can constrain it. A source tree that the user names for the static route is code that the user chose to build, and CMake runs it with the user's permissions. The same holds for its git metadata, which the commit check trusts, and for the programs named `git`, `cmake`, and `ninja` that `PATH` finds. A defect in libclang that silently misreports a declaration without any diagnostic cannot be detected by the generator, though the comparison between targets and the layout assertions narrow it. Attacks on the repository itself are a matter of access control.

The residual risk has eight parts. First, the layout assertions check structs and unions, not function signatures, so a wrong parameter type in a function or a vtable entry that the generator's rules allow compiles and is undefined behaviour, the smoke test exercises only a few functions, and the native-symbol test checks only that each C function exists in the library, not its signature. Second, the static route checks the clone only when the build script runs, which the `HEAD` and `index` files of the clone trigger (SCS-30). A change to a tracked file followed by no git operation does not rerun it, and the library then stays the one built from the tree that the last run verified. A change made by hand to the copy under `OUT_DIR` is not detected while the stamp matches, and Ninja builds it. Third, the build tag check compares an abbreviated commit hash, and a different build with the same tag is not detected by either check, because the run-time assertion of SCS-34 compares the same string. The build script also reads the header and not the library, so a library replaced in an SDK whose header is unchanged passes the build, and only the run-time assertion detects it. Fourth, Slang is a large C++ code base whose defects reach every program that links it, as Section 18 of the architecture design states. Fifth, Miri cannot run any test that calls Slang (SCS-33), so the soundness of the declarations rests on the generator and its tests, not on Miri. Sixth, a program linked on the static route still loads a `slang-glslang` shared library by name when the dynamic loader finds one (Section 16), and that library can come from another Slang build than the pin. The SPIR-V that such a program generates therefore depends on the environment that it runs in: with such a library on the loader path, Slang optimizes the SPIR-V, and without one, it does not. Seventh, the static route has run only on x86_64 Linux, so its checks on macOS and Windows are untested (Section 17, SQ-3). Eighth, a C++ exception that escapes Slang is defined behaviour through the unwinding ABIs, but it runs the destructors of the caller's Rust frames, so a direct user of this crate that does not stop the unwinding can call Slang again while Slang is in the middle of unwinding, and Slang's state can be inconsistent after it.

## 19. Operational Considerations

### 19.1. Cargo Features

`slang-compiler-sys` has no Cargo features, and neither does the binding generator. The environment selects the route of the build script (Section 9.1).

### 19.2. Environment Variables

This table lists every environment variable that the build script or the generator reads, and every variable that the build script sets or removes for the programs that it runs, with its default. The variables that Cargo sets for every build script, such as `OUT_DIR`, `TARGET`, `HOST`, and the `CARGO_CFG_TARGET_*` variables, are not options, and the table lists those that the build script decides on.

| Variable | Read by | Default and behaviour when absent |
| --- | --- | --- |
| `DOCS_RS` | The build script, always | Absent, so the build proceeds on the route that Section 9.1 selects. When present, with any value, the build script returns at once (SCS-29). |
| `SLANG_SOURCE_DIR` | The build script, when `DOCS_RS` is absent | Absent, so the dynamic route applies. When present, with any value, the static route applies (Section 9.3). An empty value, a value that is not an absolute path in UTF-8 without line breaks, and a path that is not an accessible directory stop the build (SCS-42). |
| `OUT_DIR` | The build script on the static route | Set by Cargo. The static route keeps its copy, build tree, libraries, log, and stamp under `OUT_DIR/slang`. Absence, or a value that is not an absolute path in UTF-8 without line breaks, stops the build (SCS-42). |
| `TARGET`, `HOST` | The build script on the static route | Set by Cargo. Absence of either stops the build, and so does a `TARGET` that differs from `HOST` (SCS-42, SCS-43). |
| `CARGO_CFG_TARGET_ENV`, `CARGO_CFG_TARGET_FEATURE` | The build script on the static route | Set by Cargo for the target. The static route requires `gnu` on Linux and `msvc` on Windows (SCS-43). With the Microsoft toolchain, `crt-static` among the target features selects the static runtime library (Section 9.3). |
| `NUM_JOBS` | The build script on the static route | Set by Cargo. When it holds a whole number, the build step passes it to `cmake --build` as `--parallel`. Otherwise the build step passes no job count, and Ninja chooses one. |
| `PATH` | The operating system, when the build script starts `git`, `cmake`, and `ninja` on the static route | The build stops with a message that names the program when one of the three cannot start (Section 9.6). |
| `GIT_OPTIONAL_LOCKS` | Set to `0` by the build script for every git command on the static route | The value of the environment is never used, so `git status` does not refresh the index of the clone (SCS-44). |
| `GIT_DIR`, `GIT_INDEX_FILE`, and the other 13 variables of the constant `GIT_REPOSITORY_VARIABLES` of `route.rs` | Removed by the build script from the environment of every git command on the static route | A value in the environment of the build, such as one that a git hook sets, never reaches git (SCS-44). |
| `SLANG_USE_SCCACHE` | Removed by the build script from the environment of `cmake` on the static route | A value in the environment never reaches Slang's CMake, and the configure step sets `-DSLANG_USE_SCCACHE=OFF`. |
| `CC`, `CXX`, `CMAKE_TOOLCHAIN_FILE` | CMake, when the static route configures, and not the build script | Passed through unchanged. CMake chooses the compilers and the toolchain from them. The build script does not watch them (Section 17, SQ-5). |
| `VULKAN_SDK` | The build script on the dynamic route, the generator, the regeneration test, and the variable `slang_library_path` of the `justfile` | No default. The static route never reads it, and `test-static` removes it (SCS-49). On the dynamic route, absence, an empty value, and a value that is not an absolute path in UTF-8 without line breaks stop the build (SCS-40 and Section 9.6). The generator's binary fails without it, or with it empty, with a message that names `VULKAN_SDK` and the pinned SDK version, and the regeneration test fails without it (SCS-22). Without it, or with it empty, `slang_library_path` is empty, so the recipes add no library path (SCS-32). The development container sets it. `check-doc-without-slang` removes it from the environment of the documentation build. |
| `CARGO_CFG_TARGET_OS` | The build script on both routes | Set by Cargo for the target. On the dynamic route, when it names the operating system of the host, the build script also checks the library file of the target (SCS-41). When it is absent or names another or an unknown operating system, the build script does not look for the library file. On the static route, it selects the C++ standard library and the archive names, and a target other than Linux, macOS, or Windows stops the build (SCS-43). |
| `LIBCLANG_PATH` | `clang-sys`, when the generator or its tests run | Absent, so `clang-sys` searches the locations that its documentation lists. When libclang is not found there, the generator fails with an error of kind `Libclang` that starts with "cannot load libclang" and advises to install libclang or set `LIBCLANG_PATH` (Section 7.7). |

### 19.3. Platforms and Minimum Supported Rust Version

Section 6 lists the supported targets and SCS-15 stops compilation on any other. The binding generator runs on Linux only (Sections 7.5 and 7.6), and it needs libclang and `rustfmt` on the `PATH` at run time. Its crate builds on every platform without libclang. Both crates use the workspace's `rust-version` (TOOL-4), which `check-msrv` verifies.

### 19.4. Failure Modes

This table lists the failure modes that this design defines, how each is reported, and the recovery.

| Failure | Reported by | Recovery |
| --- | --- | --- |
| `VULKAN_SDK` is absent, empty, not an absolute path in UTF-8 without line breaks, or names a directory without the pinned Slang headers, without `lib`, or without the library file of the host's target | The error that the build script returns, with the content of Section 9.6 | Install the LunarG Vulkan SDK of the pin and set `VULKAN_SDK` to the absolute path of its directory that holds `include/slang` and `lib` |
| The SDK at `VULKAN_SDK` ships another Slang build | The error that the build script returns, with both build tags (Section 9.6) | Install the SDK of the pin and set `VULKAN_SDK` to it, or use the version of `slang-compiler-sys` that matches the installed SDK |
| `SLANG_SOURCE_DIR` is empty, not an absolute path in UTF-8 without line breaks, not a directory, not the top level of a git work tree, or a tree without git metadata | The error that the build script returns, with the content of Section 9.6 | Set `SLANG_SOURCE_DIR` to the top level of a git clone of Slang at the pinned commit with its submodules, or unset it to use the dynamic route |
| The source tree is at another commit | The error that the build script returns, with both commits and the commands that check out the pin (Section 9.6) | Check out the pinned commit and update the submodules, or use the version of `slang-compiler-sys` that matches the tree |
| The source tree has a changed tracked file or submodule | The error that the build script returns, with up to ten lines of `git status` (Section 9.6) | Commit, stash, or discard the changes, or point `SLANG_SOURCE_DIR` at an unchanged checkout of the pinned commit |
| A submodule that the build needs is missing | The error that the build script returns, with the command that fetches it (Section 9.6) | Run that command, which needs network access |
| `git`, `cmake`, or `ninja` cannot start, or `git` fails | The error that the build script returns, with the command and its exit status or the reason (Section 9.6) | Install the missing program, or fix the clone that git reports on |
| The target differs from the host, or is not a supported target of the static route | The error that the build script returns, with the triples (Section 9.6) | Build on a host of the target, or unset `SLANG_SOURCE_DIR` to use the dynamic route |
| The configure or build step of `cmake` fails | The error that the build script returns, with the path of `slang/build.log` under `OUT_DIR` and its last 40 lines (Section 9.6) | Read the log, and fix the toolchain or the environment that it names, such as `CC` |
| The static build produced no archive or another build tag | The error that the build script returns, with the path that is missing or the tag (Section 9.6) | Check that the source tree is the pinned commit. The route expects the layout that the build of the pinned commit produces, so another layout needs a change to the build script and this design |
| A file under `OUT_DIR` cannot be copied, listed, opened, removed, or written | The error that the build script returns, with the action, the path, and the reason (Section 9.6) | Free disk space or fix the permissions of the target directory |
| A build that links for a target whose operating system differs from the host's | The linker, because the build script does not look for the library of a foreign target (SCS-41) | Build on a host of the target's operating system, with the SDK for that platform |
| A target whose pointer width is not 64 | The `compile_error!` of SCS-15 | Build for a supported target |
| The shared library is not found at run time | The dynamic loader, which stops the program before `main`. On Linux, it prints "error while loading shared libraries", the file name `libslang-compiler.so.0.2026.17`, and "cannot open shared object file: No such file or directory", and the program exits with status 127 (Section 11) | Run the program through a recipe, which supplies the path, or put the SDK's library directory on the platform's library path as the crate documentation says (Sections 10 and 13) |
| The library at run time is not the pinned build | The smoke test (SCS-34), and from M2 the safe crate | Make the loader find the pinned SDK's library |
| A program linked on the static route finds no `slang-glslang` shared library at run time, so its SPIR-V is not optimized | Slang, only in the diagnostics of the first generation of SPIR-V in each global session, which begin `error[E00100]: failed to load downstream compiler 'spirv-opt'` while the call succeeds (Section 16) | Accept SPIR-V that is not optimized, or put the directory of a `slang-glslang` library of the pinned build on the platform's library path (Section 18) |
| libclang cannot be loaded | The generator or its tests, with an error of kind `Libclang` that gives the reason (Section 7.7) | Install libclang or set `LIBCLANG_PATH` |
| The process already holds a libclang instance | The generator, with an error of kind `Libclang` that says so (Section 7.7) | Generate the bindings once per process and from one thread |
| A parse reports an error diagnostic, a target difference has no rule, a construct of SCS-19 appears, or a declaration matches no rule | The generator, with the diagnostic, the difference, the construct, or every unmatched declaration, each with its location | Fix the shim headers, add a rule to the rule set, or extend the generator and this design |
| A rule decides nothing | The generator, with every stale rule and its reason (SCS-39) | Remove or correct the rule, and update Section 4.3 or 4.4 when it records the rule |
| rustfmt cannot be started, fails, or does not finish within 60 seconds | The generator, with an error of kind `Rustfmt`, which holds rustfmt's exit status and standard error when rustfmt fails (Section 7.7) | Install rustfmt, or fix the generator when the generated text does not parse |
| The headers do not match the pin | The generator, with both build tags (Section 7.2) | Install the pinned SDK, or move the pin (Section 8.2) |
| A checked-in generated file differs from a regeneration | The regeneration test and `check-bindings`, with each file that differs (Section 7.6) | Run `just generate-bindings` and commit both regenerated files with the change that caused the difference |
| A C function of the bindings is not exported by the library | The linker, when it links the test binary of `slang-compiler-sys` for the native-symbol test (Section 11) | Install the SDK of the pin, or move the pin and regenerate (Section 8.2) |

### 19.5. Logs

The build script emits no warnings. Its only output is the Cargo instructions and, on a failure, its error. On the static route, it writes the output of every `cmake` run to `slang/build.log` under `OUT_DIR`, which it truncates at the start of each run, so the log holds the output of the last run only (SCS-48). On 2026-11-10, the log of a full build was 60 KB and held no download. The error of a failed configure or build step names the log and holds its last 40 lines (Section 9.6). The generator prints its errors to standard error. Neither crate emits logs at run time.

## 20. References

### 20.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 20.2. Informative References

- [SLANG-BUILD] The Slang project, "Building Slang From Source", `docs/building.md` at commit `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`, <https://github.com/shader-slang/slang/blob/c7954ebd1f1984ad2bfde15f76248db41dc34aa3/docs/building.md>.
- [SLANG-UG] The Slang project, "Slang User's Guide", `docs/user-guide/` at commit `c7954ebd1f1984ad2bfde15f76248db41dc34aa3`, including the chapter `08-compiling.md`, <https://github.com/shader-slang/slang/tree/c7954ebd1f1984ad2bfde15f76248db41dc34aa3/docs/user-guide>.
- [SLANG-6344] Issue 6344 of the shader-slang/slang repository on GitHub, on the order of releasing the global session, <https://github.com/shader-slang/slang/issues/6344>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| SCS-1 | 4.1 | Include all of `slang.h` except the exclusions of Section 4.3, and only the `spReflection*` part of `slang-deprecated.h` without `spReflection_GetSession`, and exclude the other headers. |
| SCS-2 | 4.1 | A checked-in rule set decides inclusion, and an unmatched declaration fails the generator. |
| SCS-3 | 4.1 | No bindings for symbols exported only under mangled names or for inline C++ wrappers. |
| SCS-4 | 4.1 | Excluded types that included declarations use become opaque types, and every vtable slot stays. |
| SCS-5 | 5.1 | Two rustfmt-formatted files from one parse: `src/bindings.rs`, in a private module re-exported at the crate root, and `tests/sys/symbols.rs`, the list of C functions, as the module `symbols` of the integration-test binary. |
| SCS-6 | 5.2 | Types map by typedef name, never by canonical type. |
| SCS-7 | 5.3 | Named enumerations are `#[repr(transparent)]` integer newtypes, unnamed ones are free constants of their assigned typedef, and none is a Rust enum. |
| SCS-8 | 5.4 | Macro constants come from a synthetic translation unit and an allow-list, without platform macros, and `SlangResult` is an `i32` newtype with the result codes as its constants. |
| SCS-9 | 5.5 | Structs and unions are `#[repr(C)]`, with a `Default` that reproduces C++ default member initializers. |
| SCS-10 | 5.6 | Each interface has a struct, a vtable struct with the base first and `"system-unwind"` entries, a UUID through a trait, and thunks on raw pointers. |
| SCS-11 | 5.7 | C functions are in `unsafe extern "C-unwind"` blocks, and opaque types cannot be constructed. |
| SCS-12 | 5.8 | Compile-time assertions of size, alignment, and field offsets for every struct and union. |
| SCS-13 | 5.10 | No comments or item documentation except one `//!` line, one `#[expect]` on `mod bindings;` with the lints that fire, and a second one for `ffi_unwind_calls`. |
| SCS-14 | 5.10 | `check-typos` passes on the generated file through the narrowest recorded exclusion. |
| SCS-15 | 6 | The generated file starts with a `compile_error!` for every target whose pointer width is not 64. |
| SCS-16 | 7.1 | The generator is `tools/bindings-generator`, unpublished, on the `clang` crate with `runtime`, without unsafe code. |
| SCS-17 | 7.2 | One parse per supported target with `-nostdinc` and shim headers, and no error diagnostic accepted. |
| SCS-18 | 7.3 | Only the two listed differences between targets are accepted. |
| SCS-19 | 7.3 | Every included construct that the generated Rust cannot express, as Section 7.3 lists, and every unmatched declaration fail on every target. |
| SCS-20 | 7.4 | The binary and the regeneration test call the same library function with the same inputs. |
| SCS-21 | 7.5 | Generator tests that load libclang sit in `ffi` modules and compile for Linux only. |
| SCS-22 | 7.6 | The regeneration test fails, never skips, without the SDK, the headers, or libclang. |
| SCS-23 | 8.1 | The pin lives only in `pin.rs`, read through `include!` or a recipe. |
| SCS-24 | 9.1 | `DOCS_RS`, then `SLANG_SOURCE_DIR`, then the dynamic route, and no Cargo feature. |
| SCS-25 | 9.2 | The dynamic route checks `SLANG_TAG_VERSION` against the pin, with no bypass. |
| SCS-26 | 9.2 | After the checks of SCS-25, SCS-40, and SCS-41, the dynamic route links `slang-compiler` from `$VULKAN_SDK/lib`, without pkg-config, rpath, or other locations. |
| SCS-27 | 9.3 | The static route requires the top level of a clean git checkout of the pinned commit, refuses a tree without git metadata, and never falls back. |
| SCS-28 | 9.3 | The static route runs CMake through `Command` with Ninja, probed first, disables downloads, sets the full and numeric version, and links the six archives and the C++ standard library. |
| SCS-29 | 9.4 | With `DOCS_RS` present, the build script returns at once. |
| SCS-30 | 9.5 | Rerun instructions for the route variables, for `slang-tag-version.h` on the dynamic route, and for the `HEAD` and `index` files of the clone on the static route. |
| SCS-31 | 9.6 | Each build failure returns an error with the content of the table. |
| SCS-32 | 10 | Each recipe that runs compiled code which loads Slang starts its command line with the SDK library path assignment of `slang_library_path`, which is never exported and is empty on Windows and without `VULKAN_SDK`. |
| SCS-33 | 11 | Tests that call native code sit in `ffi` modules, which `default-miri` excludes with `gpu` and `hw_gpu`. |
| SCS-34 | 11 | The smoke test asserts the pinned build tag at run time on both routes. |
| SCS-35 | 12 | CI installs and verifies the pinned SDK on all three platforms and sets the paths. |
| SCS-36 | 12 | CI does not build the static route on every push or pull request. |
| SCS-37 | 13 | The crate documentation has the content of the writing guide, the shared safety contract with the behaviour of an escaping C++ exception, and Slang's rules. |
| SCS-38 | 14 | The published crate has no dependencies. |
| SCS-39 | 4.1 | A rule that decides no declaration or macro fails the generator, with each stale rule and its reason. |
| SCS-40 | 9.2 | The dynamic route stops on a `VULKAN_SDK` that is absent, empty, not UTF-8, not absolute, or contains a line break. |
| SCS-41 | 9.2 | The dynamic route stops when `$VULKAN_SDK/lib` is not an accessible directory, and, for a target of the host's operating system only, when it lacks the target's library file. |
| SCS-42 | 9.3 | The static route stops on a `SLANG_SOURCE_DIR` or `OUT_DIR` that is empty, not UTF-8, not absolute, or contains a line break, on a `SLANG_SOURCE_DIR` that is not an accessible directory, and on an absent `TARGET` or `HOST`. |
| SCS-43 | 9.3 | The static route refuses cross-compilation and every target other than Linux with glibc, macOS, and Windows with MSVC. |
| SCS-44 | 9.3 | Every git command runs with `GIT_OPTIONAL_LOCKS=0` and without the 15 repository variables, so git answers for the named tree and writes nothing into it. |
| SCS-45 | 9.3 | The static route builds a copy of the tracked files of the tree and of the eight required submodules inside `OUT_DIR`, refuses a missing submodule and a path that leaves the tree, and removes stale files. |
| SCS-46 | 9.3 | The static route copies and configures again whenever the stamp differs, and writes the stamp only after a verified build. |
| SCS-47 | 9.3 | After the build, the six archives exist and the version header of the build tree defines the pinned build tag. |
| SCS-48 | 9.3 | The output of `cmake` goes to `slang/build.log`, and a failed step's error names the log and holds its last 40 lines. |
| SCS-49 | 9.3 | `test-static` runs only `slang-compiler-sys`, in `target/static`, without `VULKAN_SDK` and the loader path variables. |

## Change Log

- 2026-11-11: Recorded in Sections 1.6 and 3.3 that a C++ exception escaped from two methods of Slang in the tests of `slang-compiler`, amended SCS-10 and SCS-11 so that every vtable entry is `"system-unwind"` and every C function `"C-unwind"`, stated in Section 5.2 that callback types stay `"C"` and in Section 5.6 how a Rust-implemented vtable stores its non-unwinding functions, amended SCS-13 and Section 5.10 for the second lint exception for `ffi_unwind_calls`, updated the illustration of Section 5.9 and the calling convention of Section 6, recorded the changed expected texts of the generator's tests in Section 7.5 and the lint exception of the smoke test and the run of `test-static` in Section 11, amended SCS-37 and Section 13 for the paragraph on exceptions in the crate documentation and for the state of `slang-compiler`, added the unwinding ABIs and an eighth residual risk to Section 18, and updated Appendix A.
- 2026-11-11: Amended SCS-36 in Section 12 and its summary in Appendix A to cover every push and pull request that the CI workflow runs on, now that pushes to feature branches also run it.
- 2026-11-11: Stated in Sections 3.10, 13, and 16 that the pinned Slang validates SPIR-V only when `SLANG_RUN_SPIRV_VALIDATION` is `1`, on either route, so that the missing validation on the static route observed on 2026-11-10 is no difference from the dynamic route, and that the crate documentation names only the missing optimization as a loss of the static route. Corrected the record of 2026-11-10 that Slang gives no diagnostic about SPIR-V that is not optimized on a build without `slang-glslang`: recorded in Sections 3.10, 13, and 16 the run of 2026-11-11 that showed the message on the first generation of SPIR-V in each global session, its effect with `SLANG_RUN_SPIRV_VALIDATION` set to `1`, the sizes of the SPIR-V with and without the library, and what the run did not test, stated in Section 18 that the SPIR-V of a program on the static route depends on the `slang-glslang` library that the loader finds, and added a row for the missing library to Section 19.4.
- 2026-11-11: Narrowed the statement of Section 7.5 on what passed on Windows and macOS on 2026-11-10 to the two tests of the module `ffi`, and recorded in Section 12 that CI on `main` then failed in the Windows job for three commits because of one test of the real host, and which commit and run followed the fix.
- 2026-11-10: Cited Section 8 of `docs/design/slang-compiler.md` in Section 18 for the files that the safe crate lets Slang read, in place of the question Q-13 of the architecture design, which that document answered.
- 2026-11-10: Recorded step 6 of Section 15 as done and M1 as complete in Sections 1.6 and 15, with what stays open in the rows of Sections 16 and 17, described the completed crate documentation in Section 13, and cited the amended SYS-1 in Section 9.5 for the watched git files of the static route.
- 2026-11-10: Recorded step 5 of Section 15 as done and made the design agree with the implementation: the current state and new terms in Sections 1.6 and 2.1, the facts of the source build in Section 3.10, the end of the stop on `SLANG_SOURCE_DIR` in Section 9.1, the static route step by step in Section 9.3 with SCS-27 amended to require a clean checkout and refuse a source archive, SCS-28 amended for Ninja and the numeric version, and SCS-42 to SCS-49 added for the validated variables, the supported targets, the git environment, the copy, the stamp, the verification, the log, and `test-static`, the watched git files and the include directory of the build tree in Section 9.5 with SCS-30 amended, one row for each failure of the static route and the new message for an absent `VULKAN_SDK` in Section 9.6, the ten methods of `Host`, `system.rs`, and the tests of the real host in Section 9.7, the test counts in Section 7.5, the static route in Sections 10 to 14, step 5 in Section 15, the confirmation of SA-4 and SA-5 and the correction of SA-6 with the removal of their rows in Section 16, the answers to SQ-1 and SQ-2 with the removal of their rows and the new questions SQ-3 to SQ-5 in Section 17, the static route in Section 18, the variables, failure modes, and log in Section 19, and Appendix A.
- 2026-11-10: Recorded step 4 of Section 15 as done and made the design agree with the implementation: the current state in Section 1.6, the second generated file `tests/sys/symbols.rs` in Sections 5.1 and 5.10 with SCS-5 amended, the generator's API in Section 7.4, the test counts in Section 7.5, the regeneration of both files in Section 7.6, the route selection with the stop on `SLANG_SOURCE_DIR` until step 5 in Sections 9.1 and 9.3, the dynamic route in Section 9.2 with SCS-26 amended and SCS-40 and SCS-41 added, the documentation build in Section 9.4, the rerun instructions in Section 9.5, one row for each failure in Section 9.6, the new Section 9.7 on the structure and tests of the build script, the library path on the command line of the recipes in Section 10 with SCS-32 amended, the smoke test and the native-symbol test in Section 11, the CI run in Section 12, the crate documentation in Section 13, the confirmation of SA-2, SA-3, and SA-9 with the removal of their rows and the reworded SA-7 in Sections 6 and 16, the new question SQ-2 in Section 17, the validation of `VULKAN_SDK` in Section 18, the environment variables, failure modes, and logs in Section 19, and Appendix A.
- 2026-11-10: Recorded step 3 of Section 15 as done: the current state in Section 1.6, the module wiring and the checked-in file in Section 5.1, the lint list verified on the checked-in file on the four supported targets and the three typos entries of SCS-14 in Section 5.10, the six new refusal tests and the dated test counts in Section 7.5, the regeneration test, its failure messages, the libclang versions observed, and the recipes `generate-bindings` and `check-bindings` with the reason for `--no-tests=fail` in Section 7.6, the docs.rs targets in Section 9.4, why the native-symbol test of TEST-8 waits for step 4 in Section 11, the Linux CI jobs in Section 12, the current crate documentation and what waits for steps 4 and 6 in Section 13, and step 3 in Section 15.
- 2026-11-10: Recorded step 2 of Section 15 as done: the installation of the Vulkan SDK in CI in Sections 1.5, 1.6, and 12, the build tag and the libraries of the SDKs for Windows and macOS in Sections 3.6 and 3.7, the portability of the recipe `pin` in Section 8.1, the confirmed layout in Section 9.2, the library paths on the runners and the macOS observations with the options for SA-7 in Section 10, the confirmation of SA-1 with the removal of its row, the narrowed SA-2 and the updated SA-7 in Section 16, and the dating of the observations from the runners in Sections 3.7, 8.1, and 10.
- 2026-11-10: Recorded step 1 of Section 15 as done and made the design agree with the implementation: the current state in Section 1.6, the findings of the generator in Section 3.11, the rule set, its exclusions, and its unnamed enumeration rules in Sections 4.1, 4.3, and 4.4 with SCS-1 reworded and SCS-39 added, the constants of unnamed enumerations in SCS-7, the names, derives, and paths in Section 5.11, the `compile_error!` at the top of the generated file in Section 5.1, SCS-15, and step 3, the verified lints in Section 5.10, the refusals of SCS-19, the module structure, tests, and error reporting in Sections 7.4, 7.5, and 7.7, the pin constants and the recipe `pin` in Section 8.1, the integration-test binary in Section 11, the confirmation of SA-10, and the corrected nesting rule of Section 4.1, exclusions of Section 4.3, refusals of SCS-19, error kind `Io`, and test counts.
- 2026-11-10: Initial version.
