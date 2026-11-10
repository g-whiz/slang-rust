# `slang-compiler-sys` and Binding Generator Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-10 |
| Applies to | `crates/slang-compiler-sys` and `tools/bindings-generator` |

## Abstract

This document specifies `slang-compiler-sys`, the crate of raw Rust bindings to the native Slang compiler library, and the binding generator that writes those bindings from the Slang headers. It records the facts about the Slang headers, the Vulkan SDK, and the shared library that a survey, a generator spike, and the implementation of the generator verified, and it builds the design on them. It defines which declarations the bindings include, the shape of the generated Rust for types, enumerations, constants, structs, COM-style interfaces, and C functions, and the restriction to 64-bit targets. It specifies how the generator parses the headers for several targets and fails on anything it has no rule for, how the generated file passes the repository's checks, and how a test keeps the checked-in file equal to a fresh regeneration. It defines the file that records the pinned Slang version, the build script with its dynamic route through an installed Vulkan SDK and its static route from a Slang source tree selected by an environment variable, the build for documentation without Slang, and the message of each failure. It covers the tests, Miri, continuous integration, the crate documentation, the dependencies, the order of the changes that implement milestone M1, the assumptions still to confirm, and the open questions.

## 1. Introduction

### 1.1. Problem

Slang exposes its compiler through a C++ header, `slang.h`, whose main API consists of COM-style interfaces, which are C++ classes with only pure virtual methods. Rust cannot call a C++ virtual method directly. It can call one through a `#[repr(C)]` struct of function pointers that has exactly the layout of the C++ vtable. That struct copies a few hundred method signatures, in the right order and with the right calling convention. A single slot out of order calls the wrong function, and no compiler detects it. The headers also declare enumerations whose values the library can extend, structs whose C++ default values matter, and constants that exist only as macros. Hand-written declarations drift from the headers with every Slang release.

The architecture design (`docs/design/architecture.md`) places these declarations in `slang-compiler-sys`, requires that a generator in this repository produce all of them (ARCH-13), and requires this document to exist before any code of milestone M1 (ARCH-3, ARCH-25).

### 1.2. Purpose

`slang-compiler-sys` gives the safe crate `slang-compiler` an exact, complete, and checked set of declarations for the pinned Slang build, and links the native library on the route that the user's environment selects. The binding generator produces those declarations from the headers so that no agent writes a vtable, a UUID, or a thunk by hand (COM-9). This document implements Sections 3 to 6 and milestone M1 (Section 15.4) of the architecture design and answers the questions that its Section 17 assigns to this document.

### 1.3. Audience

The primary readers are the coding agents that implement M1 and the agents that later move the pin or change the generator. The secondary readers are the reviewers who check those changes, and the owner of the repository, who approves the decisions. A reader is expected to know Rust but not the Slang API, C++ application binary interfaces, or libclang. Section 2.1 defines the terms such a reader needs.

### 1.4. Scope

This document covers the declarations that the bindings include, the shape of the generated Rust, the supported targets, the binding generator and its tests, the file that records the pin, the build script of `slang-compiler-sys` on both routes and for documentation builds, how the shared library is found at run time, the tests of the crate, Miri, the CI changes that M1 makes, the crate documentation, the dependencies, and the order of implementation.

### 1.5. Non-Goals

This document does not cover:

- The safe API, which the design document of `slang-compiler` defines in M2, including ownership, errors, and the `Send` and `Sync` decisions of FFI-9 and CONC-1.
- 32-bit targets, which Section 6 excludes. The owner can revisit this decision.
- The deprecated compile-request API of `slang-deprecated.h`, including `slang::ICompileRequest`, which the modern interfaces replace (Section 4).
- `slang-gfx.h`, which declares a separate deprecated library, and the other headers that Section 3.2 lists as out of scope.
- Hand-written declarations of any kind in the generated file, which COM-9 and FFI-1 forbid.
- How continuous integration (CI) installs the Vulkan SDK on each runner, which Section 8.3 of `docs/design/tooling.md` specifies. Section 12 states the requirements that the installation meets.
- The recipes, the nextest profiles, and the CI workflow as a whole, which `docs/design/tooling.md` owns. This document states what M1 adds to them.

### 1.6. Current State

Steps 1 and 2 of Section 15 are done. These parts of the design exist:

- `crates/slang-compiler-sys/pin.rs`, the pin file of Section 8.1, which the crate root includes and documents.
- The recipe `pin`, which prints the pin and runs on Linux, macOS, and Windows (Section 8.1).
- The installation of the pinned Vulkan SDK in every job of the CI workflow, with its check against the pin, through the recipes `install-vulkan-sdk` and `check-vulkan-sdk` (Section 12).
- `tools/bindings-generator`, the binding generator of Section 7, with its library, its thin binary, its rule set, its shim headers, and its unit tests (Section 7.5).
- The integration-test binary `crates/slang-compiler-sys/tests/sys/`, whose module `pin` tests the constants of the pin (Section 11).
- The nextest profile `default-miri` of SCS-33, the licences of Section 14 in `deny.toml`, and the installation of libclang in the Linux job of the CI workflow (Section 12).

`crates/slang-compiler-sys` is otherwise still the placeholder crate that milestone M0 created, with crate-level documentation that says it holds no bindings yet, and with no build script, no `links` key, and no dependencies. These parts do not exist yet: the checked-in bindings `crates/slang-compiler-sys/src/bindings.rs`, the regeneration test, the recipes `generate-bindings`, `check-bindings`, `check-doc-without-slang`, and `test-static`, the build script, the smoke test, and the library path export of the `justfile`. The generator's binary has not been run in the repository, because its output arrives in step 3.

The facts in Sections 3.1 to 3.10 come from a survey of the Slang headers, the Vulkan SDK, and the Slang source tree in the development container, and from a spike that built a throwaway generator and a miniature of the bindings. Section 3.11 records the facts that the implementation of the generator established. Sections 3.6, 3.7, and 10 also record the facts about the SDKs for Windows and macOS that the CI runs of step 2 established.

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
- default member initializer: A value written in a C++ struct declaration for a field, such as `int size = sizeof(Desc);`, which the field takes when the struct is constructed without that field.
- diagnostic: A message that libclang reports while it parses, with a severity of note, warning, error, or fatal.
- import library: A file on Windows, with the suffix `.lib`, that tells the linker which symbols a dynamic-link library (DLL) exports.
- inclusion rule: An entry of the checked-in rule set of the generator that says whether a declaration, or a group of declarations selected by name or by header, is included in or excluded from the bindings (Section 4.1).
- model: The generator's in-memory description of the declarations that it reads from one parse, such as the interfaces with their methods in order, before it writes any Rust.
- name mangling: The encoding of a C++ function's namespace, class, and parameter types into its symbol name. The encoding differs between compilers.
- opaque handle: A pointer to a type whose contents the header does not declare, so that code can pass the pointer but not read the object.
- shim header: A small header in this repository that replaces a system header, such as `stdint.h`, when the generator parses for a target whose system headers the development container does not have (Section 7.2).
- standard layout: The C++ property of a type whose memory layout follows the same rules as a C struct, which `#[repr(C)]` reproduces in Rust.
- synthetic translation unit: A translation unit that the generator writes in memory and parses to evaluate an expression, such as the value of a macro (Section 7.2).
- target: The platform for which code is compiled, named by a target triple such as `x86_64-pc-windows-msvc`.
- translation unit: One C or C++ source file together with every header that it includes, which a compiler or libclang parses as one unit.
- typedef: A C or C++ declaration that gives an existing type a new name, such as `typedef int32_t SlangResult;`.

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

Methods use the calling convention macro `SLANG_MCALL`, which is `__stdcall` on the Microsoft compilers and empty elsewhere, and is `extern "system"` in Rust. `SLANG_NO_THROW` is `__declspec(nothrow)` on Windows and empty elsewhere. On 2026-11-10, one method lacks `SLANG_MCALL`: `slang::IMetadata::isParameterLocationUsed`, at line 4758 of `slang.h`.

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

The setting `SLANG_LIB_TYPE=STATIC` produces `libslang-compiler.a`. Slang's building documentation lists the libraries that a consumer of the static library also links: `libcompiler-core.a`, `libcore.a`, `libminiz.a`, and `liblz4.a` [SLANG-BUILD]. This confirms assumption A-3 of the architecture design as far as the documentation goes. The CMake target of the library also links helper libraries for capabilities and lookup tables, and cmark, so the documented list is incomplete, and the complete list needs a build (Section 16, SA-4).

A build needs CMake 3.22 or newer, or 3.25 for presets, a C++17 compiler, Ninja, and Python 3. In the source tree at `/opt/slang`, all 21 git submodules are checked out, 224 MB under `external/`. The library `slang` has about 388 hand-written translation units and some generated ones. These options reduce the build: `SLANG_ENABLE_TESTS`, `SLANG_ENABLE_EXAMPLES`, `SLANG_ENABLE_GFX`, `SLANG_ENABLE_SLANG_RHI`, `SLANG_ENABLE_REPLAYER`, `SLANG_ENABLE_SLANGD`, `SLANG_ENABLE_SLANGC`, `SLANG_ENABLE_SLANGI`, `SLANG_ENABLE_SLANGRT`, `SLANG_ENABLE_SLANG_GLSLANG`, `SLANG_ENABLE_DXIL`, and `SLANG_SLANG_LLVM_FLAVOR=DISABLE`. The CMake files suggest that the default LLVM flavour and the fetching of DirectX Shader Compiler (DXC) download files at configure time unless they are disabled, which was not tested (Section 16, SA-5).

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

SCS-5. The binding generator MUST write all generated code into the one file `crates/slang-compiler-sys/src/bindings.rs`, formatted with rustfmt, and the crate MUST declare it as the private module `bindings` and re-export its items at the crate root with `pub use bindings::*;`, because the regeneration check (Section 7.6) compares one file, and `check-fmt` fails on a checked-in file that rustfmt would change.

Each generated item then has exactly one public path, at the crate root, as API-10 requires. The glob re-export is the one that API-10 allows a `-sys` crate for its private generated module, because a hand-written list of every generated name would be a second copy of the generated names that diverges at every regeneration.

The file `crates/slang-compiler-sys/src/lib.rs` holds the crate-level documentation (Section 13), the opt-out of `unsafe_code` (Section 5.10), the declaration of the module with its lint exception (Section 5.10), the re-export, and the inclusion of the pin (Section 8.1). It holds no other code (FFI-1). The check of the pointer width (Section 6) is not in `lib.rs`: the generator writes it at the top of the generated file, directly after the `//!` line, so it travels with the declarations whose layouts it protects.

### 5.2. Type Mapping

SCS-6. The binding generator MUST map each type by its typedef name and never by its canonical type, mapping `SlangInt` and `SlangSSizeT` to `isize`, `SlangUInt`, `SlangSizeT`, and `size_t` to `usize`, each `intN_t` to `iN`, each `uintN_t` to `uN`, and the C types `char`, `int`, and the other basic types to the types of `core::ffi`, because the canonical type differs between targets (Section 3.8), and a mapping from it would write a file that is correct on one target only.

Each integer typedef of the headers becomes a Rust type alias, except `SlangResult`, which becomes the newtype of Section 5.4. A C++ reference parameter becomes `*const T` for a `const` reference and `*mut T` otherwise. `bool` maps to `bool`. A data pointer becomes `*const T` or `*mut T` by its `const` qualifier. A callback type becomes `Option<unsafe extern "C" fn(...)>`, because a Rust function pointer cannot be null.

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

SCS-10. For each interface `I`, the binding generator MUST emit a `#[repr(C)]` struct `I` whose only field is the vtable pointer, a `#[repr(C)]` vtable struct whose first field is the vtable struct of the base interface, followed by one `unsafe extern "system" fn` pointer for each own method in declaration order, with the interface pointer as the explicit first parameter, the UUID of `I` as an associated constant of a trait that the generator emits and implements for every interface, and one `unsafe` thunk for each own and inherited method that calls through the vtable, because these items are the only path from Rust to the methods, and a wrong order, calling convention, or UUID compiles and fails only at run time (ARCH-13, COM-4).

A thunk is an associated function whose first parameter is the raw interface pointer, `this: *mut Self`, and not a method with a `&self` receiver. A Rust reference to `I` covers only its vtable pointer, while the native object extends beyond it and changes behind the reference, so a reference would assert an aliasing guarantee that the object does not keep. The raw pointer asserts nothing, which is the sounder choice under Hard Rule 7 of `AGENTS.md`. A thunk for an inherited method casts the pointer to the type of the base interface and calls through the base vtable inside the vtable of `I`.

The trait is `pub unsafe trait Interface { const UUID: SlangUUID; }`, and each interface implements it with `unsafe impl`. It is an unsafe trait because an implementation promises that the type is an interface struct whose vtable has the layout of the native interface with that UUID, which the compiler cannot check, so any other implementation has to state that promise in an `unsafe impl` of its own. The interface struct `I` has the one field `vtable: *const IVtbl`, and the vtable struct is named `IVtbl`. The first field of the vtable struct of a derived interface is `base`, of the type of the base interface's vtable struct, and `ISlangUnknownVtbl` has no `base` field. A thunk for an inherited method calls through `base` once for each level of inheritance, with `this.cast()` as its first argument.

The generator emits no `Send` or `Sync` implementation, and the raw pointer field already makes every interface struct neither (FFI-9). An opaque type of SCS-4 that is an excluded interface is emitted without vtable and thunks. The generator refuses an interface with a data member, a virtual destructor, a method that has the name of another method of the interface or of a base, a base that is not an included interface, and a UUID that it cannot read as eleven integers (SCS-19).

### 5.7. C Functions and Opaque Handles

SCS-11. The binding generator MUST declare every included C function in an `unsafe extern "C"` block, with no function marked `safe`, and MUST emit each opaque handle and each opaque type of SCS-4 as a zero-sized `#[repr(C)]` struct that code outside the crate cannot construct and that is neither `Send`, `Sync`, nor `Unpin`, because the compiler trusts these declarations without checking them (FFI-3), and a constructible or movable opaque type lets safe code create a value that no native object stands behind.

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
    pub getCount: unsafe extern "system" fn(this: *mut IExample) -> SlangInt,
    pub setName: unsafe extern "system" fn(this: *mut IExample, name: *const ::core::ffi::c_char) -> SlangResult,
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

SCS-13. The binding generator MUST emit no comment and no item documentation, only one module-level `//!` line that says the file is generated and names the recipe `generate-bindings`, and the crate MUST carry the lint exception of the generated code as one `#[expect(..., reason = "...")]` on the declaration `mod bindings;`, listing exactly the lints that fire, as determined by running the checks, because `check-rust-comments` rejects any other comment (Section 7.2 of the writing guide), and an `#[expect]` that lists a lint that does not fire fails the check through `unfulfilled_lint_expectations` (Section 7.9 of the writing guide).

Because the file holds no other comment and no documentation, `check-rust-comments`, `check-doc-headings`, and `check-prose` read it like any other file and need no exemption, as Section 7.2 of the tooling design records.

On the file that the generator writes from the headers of the pin, compiled in the scratch crate of Section 3.11 with the workspace lint tables and warnings denied, exactly these lints fire, the same on the three targets of that section, with the number of findings of each on 2026-11-10:

- `missing_docs`, 1,696.
- `non_snake_case`, 954.
- `non_upper_case_globals`, 303.
- `non_camel_case_types`, 2.
- `clippy::exhaustive_structs`, 131.
- `clippy::missing_safety_doc`, 291.
- `clippy::undocumented_unsafe_blocks`, 317.
- `clippy::multiple_unsafe_ops_per_block`, 290.
- `clippy::similar_names`, 1.

`clippy::exhaustive_enums` does not fire, because the file has no Rust `enum` (SCS-7), and `missing_debug_implementations` does not fire, because every generated type implements `Debug` (Section 5.11). The crate root also needs `#![expect(unsafe_code, reason = "...")]`, because the thunks hold `unsafe` blocks. Original C++ names are kept, which is why the naming lints fire. The thunks are `unsafe fn` items whose bodies are `unsafe` blocks without safety comments, and they have no `# Safety` sections, which is why the three lints of safety documentation fire. Step 3 of Section 15 sets the list in the lint exception by running the checks on the checked-in file, as SCS-13 requires. Section 7.9 of the writing guide and UNS-6 exempt generated raw bindings from `# Safety` sections and safety comments on two conditions, which this design meets: the crate-level documentation states one safety contract that every thunk and every C function shares (Section 13), and the exemption is part of the lint exception on `mod bindings;` (SCS-13). The crate root also carries the opt-out of `unsafe_code` that Section 4.4 of the coding standard defines.

SCS-14. The agent that first checks in the generated file MUST make `check-typos` pass on it by the narrowest exclusion that works, such as an entry for one identifier, and MUST record the exclusion and its reason in this section, because Slang identifiers are not ordinary words, and a broad exclusion, such as excluding the whole file, would stop typos from checking the generated `//!` line and any later text in the file.

### 5.11. Names, Derives, and Paths

The generator derives each Rust name from the C++ name. These are its rules:

- A namespace is dropped, so `slang::IGlobalSession` becomes `IGlobalSession`.
- A type nested in a record joins the names of its records and its own name with `_`, so `slang::CompileCoreModuleFlag::Enum` becomes `CompileCoreModuleFlag_Enum`.
- The k-th anonymous struct or union inside a record, counted from 0, is the type `Parent_anon{k}`, held by the field `anon{k}`, so an anonymous union inside it is `Parent_anon{k}_anon{j}`.
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

The supported targets are x86_64 Linux, x86_64 Windows with the Microsoft toolchain, and aarch64 and x86_64 macOS. CI tests x86_64 Linux, x86_64 Windows, and aarch64 macOS (Section 12 and SA-3). x86_64 macOS is supported by construction and is not tested. 32-bit targets are a non-goal that the owner can revisit. The Vulkan SDK installed in the development container provides its libraries for `x86_64` only, and its root is the `x86_64` directory. This design does not rely on whether LunarG ships 32-bit libraries on any platform.

The bindings are generated once, on Linux, and the same file serves every supported target. Everything that depends on the target is expressed in the Rust source by construction: the calling convention through `extern "system"`, which is the C calling convention on every 64-bit target, the integer typedefs through `isize` and `usize`, and, where nothing else works, `#[cfg]`, which the generator emits only under a rule that names the case. The comparison of Section 7.3 shows that the headers of the pin need no `#[cfg]` other than the one on the `compile_error!` of SCS-15.

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

The generator is a library with a thin binary. The library's public API is the function `generate`, which takes the header directory and returns the generated text, the types `Error`, `ErrorKind`, and `Location` with the alias `Result` (Section 7.7), and the module `pin`, which includes the pin file (Section 8.1). The binary takes the header directory `$VULKAN_SDK/include/slang`, calls `generate`, and writes the text to `crates/slang-compiler-sys/src/bindings.rs`, by a path relative to the generator's manifest directory.

Inside the library, the module `parse` turns one parse of libclang into a model, `rules` holds the rule set, `check` selects the declarations by the rules, validates the selection against every refusal of SCS-19 for each target, and compares the targets, and `emit` writes the Rust text and formats it with `rustfmt --edition 2024` on standard input. The module `model` defines the model, and `error` defines the error type.

SCS-20. The binary of the binding generator and the regeneration test of Section 7.6 MUST call the same library function with the same inputs, because the test is a proof that the checked-in file equals the output of the binary only when both take the same path through the code.

### 7.5. Tests

The tests in the generator's `ffi` modules parse small synthetic headers with libclang and check the generated text for the order of vtable slots under single and multi-level inheritance, the extraction of UUIDs, the mapping of types, references, callbacks, and enumerations, the constants of unnamed enumerations, the evaluation of macros and default member initializers, including floating-point values, opaque types, layout assertions, unions and nested anonymous records, the accepted and rejected differences between targets of Section 7.3, determinism, several refusals of SCS-19, the failures on a declaration or macro that matches no rule and on a stale rule (SCS-39), the check of the build tag, and the error when a process already holds libclang. The `ffi` modules also hold the tests that start a process, such as `rustfmt` or `sh`, because Miri cannot start a process. The other tests are pure Rust and also run under Miri: they cover the matching of rule patterns, the lookups in the rule set, the reading of `slang-tag-version.h`, the value checks of the validator, the comparison of targets, the formatting of literals, the escaping of names, and the error type.

SCS-21. Every test of the generator that loads libclang MUST sit in a module named `ffi` and MUST be compiled only for Linux targets, because Miri cannot call libclang (Section 11), and the Windows and macOS runners are not required to provide libclang, so such a test would fail there without a defect in the code.

The `ffi` modules are compiled under `#[cfg(target_os = "linux")]`, so they do not run on Windows or macOS at all, and they never report a skipped run as a pass. On Linux, a test that finds no libclang fails. The Linux job of the CI workflow provides libclang (Section 12), and the generator's tests passed there on the first run of the workflow with the generator. Only one `Clang` instance may exist per process (Section 7.1), so each test that loads libclang holds a static mutex for its whole run. nextest runs each test in its own process, and a run through `cargo test`, which runs the tests of one binary on several threads in one process, also works. With the tests of Section 11, the workspace has 88 tests on 2026-11-10. `check-miri` runs 34 of them, and the profile `default-miri` skips the 54 in `ffi` modules.

Reversing the order of the thunks in the generator makes a test fail, which shows that the tests of the slot order can fail.

### 7.6. Recipes and Regeneration Check

The regeneration check, which step 3 of Section 15 adds, is an integration test of the generator's crate, in its one integration-test binary (TEST-10). It runs the generator in memory on `$VULKAN_SDK/include/slang` and compares the result byte for byte with `crates/slang-compiler-sys/src/bindings.rs`. It is the first test of TEST-8, and it holds the regenerated text in memory, which TEST-8 allows, because a comparison in memory needs no file.

SCS-22. The regeneration test MUST fail, and MUST NOT skip, when `VULKAN_SDK` is unset, when the headers are missing, or when libclang cannot be loaded, and it is subject to SCS-21, because a check that passes when it cannot run lets bindings that differ from the headers reach the main branch.

The test runs in the default test tier, so `test`, `test-gpu`, and therefore `just check` run it on Linux. A whole run of the generator takes about 190 ms (Section 3.11), so it adds little to the tier. Windows and macOS runners do not regenerate.

These recipes arrive in M1, with the names and groups of Section 4.2 of the tooling design:

- `generate-bindings`: Runs the generator's binary, which overwrites `crates/slang-compiler-sys/src/bindings.rs`.
- `check-bindings`: Runs the regeneration test alone, through the nextest command of Section 4.8 of the coding standard with a filter for that test.

This command regenerates the bindings in the development container:

```sh
just generate-bindings
```

A hand edit of the generated file makes the regeneration test fail, which enforces COM-9. An agent that changes the generator runs `generate-bindings` and commits the generator and the file together.

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
| `Rustfmt` | rustfmt cannot be started or fails |

The errors of `UnmatchedDeclaration` and `StaleRule` list every unmatched declaration and every stale rule at once, so that one run shows the whole change to the rule set. When rustfmt fails, the message holds its exit status and its standard error. When libclang cannot start, the message says which of two cases applies: libclang cannot be loaded, with the reason, and the advice to install libclang or set `LIBCLANG_PATH`, or the process already holds a libclang instance or failed to load libclang before, with the advice to generate the bindings once per process and from one thread. The binary prints the error and each source error, one per line, to standard error and exits with a failure status.

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

This command builds the workspace on the static route from a source tree at the pinned commit:

```sh
SLANG_SOURCE_DIR=/tmp/slang cargo build
```

### 9.2. Dynamic Route

SCS-25. On the dynamic route, the build script MUST read `$VULKAN_SDK/include/slang/slang-tag-version.h`, extract the string that `SLANG_TAG_VERSION` defines, and stop the build with the message of Section 9.6 when the file is missing, when the definition is missing or malformed, or when the string differs from the build tag of the pin, and MUST NOT accept any setting that bypasses this check, because a library of another Slang build has layouts and vtables that differ from the checked-in bindings, which is undefined behaviour at run time (SYS-6).

This answers Q-1 of the architecture design. The headers do not record the SDK version (Section 3.6), so the build script cannot check it. The build tag contains the abbreviated Slang commit, and the bindings depend only on Slang, so two SDKs that ship the same Slang build are equivalent for this crate. The safe crate adds a run-time comparison of `spGetBuildTagString()` with the pin in M2, and the smoke test of Section 11 asserts it from M1.

SCS-26. On the dynamic route, the build script MUST emit the search path `$VULKAN_SDK/lib` and `cargo::rustc-link-lib=dylib=slang-compiler`, and MUST NOT use pkg-config, emit an rpath, or search any other location, because a single mechanism behaves the same on every platform, and finding the library at run time is the job of the environment (Section 10).

This answers Q-5 of the architecture design: pkg-config is not used, because the `.pc` file carries only the coarse version (Section 3.6) and Windows has no pkg-config. It also answers Q-4: the build script expects the headers under `include/slang` and the library under `lib` on every platform, with the import library under `lib` on Windows. The CI runs of step 2 of Section 15 confirmed this layout on 2026-11-10, which was assumption SA-1 (Sections 3.6 and 3.7). The import library on Windows has the name that `cargo::rustc-link-lib=dylib=slang-compiler` looks for in the search path `$VULKAN_SDK/lib`. That a Rust crate links against it is still SA-2 of Section 16.

### 9.3. Static Route

SCS-27. On the static route, the build script MUST stop the build when `SLANG_SOURCE_DIR` does not name a directory, and MUST verify that the directory is a git checkout whose `HEAD`, as `git rev-parse` reports it, equals the full commit of the pin, stopping the build otherwise, and MUST NOT fall back to the dynamic route (ARCH-7), because a static library built from another commit does not match the checked-in bindings.

This answers Q-2 of the architecture design. The check sees the commit and not the working tree, so local changes to a tree at the right commit pass it (Section 18).

SCS-28. On the static route, the build script MUST run `cmake` and the build tool with `std::process::Command`, check every exit status (SYS-4), write every output under `OUT_DIR` (SYS-5), disable the optional components and everything that downloads, pass `-DSLANG_VERSION_FULL=` with the build tag of the pin, and link the resulting static libraries and the C++ standard library of the target, because a native build that writes outside `OUT_DIR` or downloads breaks SYS-5, and a library built without the version override reports `0.0.0-unknown`, which the smoke test rejects.

The options to disable are those of Section 3.10: the tests, the examples, gfx, slang-rhi, the replayer, slangd, slangc, slangi, slang-rt, slang-glslang, DirectX Intermediate Language (DXIL), and LLVM through `SLANG_SLANG_LLVM_FLAVOR=DISABLE`, together with `SLANG_LIB_TYPE=STATIC`. These options avoid every download only if SA-5 holds. The build script uses no `cmake` crate, so the published crate has no build dependencies (Section 14). It chooses the C++ standard library from the `CARGO_CFG_TARGET_*` variables (SYS-3).

The exact list of static libraries, which is Q-3 of the architecture design, is determined by building the route, and the change that implements it records the list in this section and confirms SA-4. That change also records what the static build loses by disabling slang-glslang, which is expected to be the OpenGL Shading Language (GLSL) pass-through and the validation of SPIR-V by glslang (SA-6). The static route is implemented after the dynamic route, in step 5 of Section 15.

The recipe `test-static`, which arrives in step 5 of Section 15, builds the workspace on the static route and runs the smoke test of Section 11 on it. It sets no environment variable itself. The developer supplies `SLANG_SOURCE_DIR`, and the recipe fails with a message that names `SLANG_SOURCE_DIR` when the variable is absent. `check` does not run it, because it builds Slang from source (SCS-36).

This command verifies the static route from a source tree at the pinned commit:

```sh
SLANG_SOURCE_DIR=/tmp/slang just test-static
```

### 9.4. Documentation Builds

SCS-29. When `DOCS_RS` is present, whatever its value, the build script MUST return at once, after the rerun instruction for `DOCS_RS` (SCS-30), without reading any other variable, emitting any link instruction, or emitting `links` metadata, because docs.rs has no Slang, and a build script that fails there leaves the crate without published documentation (ARCH-8).

The checked-in bindings let the crate document itself without Slang. That docs.rs sets `DOCS_RS` is assumption A-7 of the architecture design. The recipe `check-doc-without-slang`, which arrives in M1, runs the documentation command of Section 4.8 of the coding standard with `DOCS_RS` set and `VULKAN_SDK` unset (Section 10 of the tooling design). The manifest of `slang-compiler-sys` lists only the supported targets of Section 6 under `targets` in `[package.metadata.docs.rs]`, so that docs.rs does not build for a 32-bit target, on which SCS-15 stops compilation (SA-8).

### 9.5. Rerun Instructions and Metadata

SCS-30. The build script MUST emit `cargo::rerun-if-env-changed` for `DOCS_RS` on every run, for `SLANG_SOURCE_DIR` on every run without `DOCS_RS`, for `VULKAN_SDK` on the dynamic route, and `cargo::rerun-if-changed` for every file that it reads, including `slang-tag-version.h` on the dynamic route and the state of the git checkout that it verifies on the static route, because a build that does not rerun after its input changes links a library that no longer matches the check it passed (SYS-1).

`pin.rs` needs no instruction, because Cargo rebuilds and reruns the build script when a file that it includes changes. On the static route, the build script does not watch every file of the source tree, which is too large to scan on every build. It watches what determines the commit, and the change that implements the route records the exact paths in this section.

The manifest declares `links = "slang-compiler"` (FFI-2). Through the `links` metadata, the build script exposes the directory of the headers as `include` and the directory of the library as `lib`, so a dependent build script reads them as `DEP_SLANG_COMPILER_INCLUDE` and `DEP_SLANG_COMPILER_LIB`. On the dynamic route, they are `$VULKAN_SDK/include` and `$VULKAN_SDK/lib`. On the static route, they are the `include` directory of the source tree and the directory under `OUT_DIR` that holds the static libraries. Under `DOCS_RS`, neither is set, and a dependent build script that needs them handles their absence.

### 9.6. Failure Messages

SCS-31. On each failure of this table, the build script MUST end by returning an error from its `main` (SYS-4) whose message contains every item of the table's second column, because a build that stops without naming what is missing and how to supply it leaves the user to guess, and a guess often links the wrong Slang.

| Failure | The message contains |
| --- | --- |
| `VULKAN_SDK`, `SLANG_SOURCE_DIR`, and `DOCS_RS` are all absent, or `VULKAN_SDK` is empty | The name `VULKAN_SDK`, the pinned SDK version, and the name `SLANG_SOURCE_DIR` as the way to the static route (ARCH-5) |
| `$VULKAN_SDK/include/slang/slang-tag-version.h` is missing | The full path that was read, and the pinned SDK version |
| `SLANG_TAG_VERSION` is missing or malformed in that file | The full path, and the build tag of the pin |
| The build tag differs from the pin | Both build tags, the pinned SDK version, and the path that was read (SYS-6) |
| `SLANG_SOURCE_DIR` is empty or names no directory | The name `SLANG_SOURCE_DIR` and its value (ARCH-7) |
| `git` cannot run, or fails, in the source tree | The command, its exit status or the reason it could not start, and its error output (SYS-4) |
| `HEAD` of the source tree differs from the pin | Both commits, and the path of the tree |
| `cmake` or the build tool cannot run, or fails | The command, its exit status or the reason it could not start, and its error output (SYS-4) |

A variable is present when the environment contains it, whatever its value. An empty `SLANG_SOURCE_DIR` still selects the static route and fails there, because a user who set the variable asked for that route, and ARCH-7 forbids a fallback. The build script emits no `cargo::warning` and prints the output of CMake and the build tool only in the error of a failure.

## 10. Finding the Shared Library at Run Time

On the dynamic route, the dynamic loader of the operating system finds the Slang shared library when the program starts, as Section 4.4 of the architecture design states, and the build script emits no rpath (SCS-26). That the build script of a dependency cannot set the rpath of the final binary is assumption A-6 of the architecture design.

SCS-32. The `justfile` MUST export the library directory of `VULKAN_SDK` on the library path of the platform, `LD_LIBRARY_PATH` on Linux and `DYLD_LIBRARY_PATH` on macOS, for every recipe, and MUST keep working when `VULKAN_SDK` is unset, because the tests that call Slang fail before any test runs when the loader cannot find the library, and the development container provides the path only to processes that a login shell starts (Section 11 of the tooling design), while `check-doc-without-slang` runs without `VULKAN_SDK`.

On every CI runner, the step that installs the SDK sets the path that the loader of the platform reads (Section 8.3 of the tooling design). On 2026-11-10, on the hosted Windows runner, a program loaded `slang-compiler.dll` by name with this `PATH` when it was started directly, through bash, through `just`, and through `cargo run`. On the hosted Linux runner, `LD_LIBRARY_PATH` reached a program in each of these cases. That a Rust crate links against the import library on Windows is still SA-2. The crate documentation tells users what to set on each platform (Section 13). A library path set by the `justfile` reaches the test binary on macOS only if no protected system program in between removes it (SA-7).

SA-7 could not be confirmed on the hosted macOS runner, because System Integrity Protection was disabled there on 2026-11-10. On that runner and date, `DYLD_LIBRARY_PATH` set through `GITHUB_ENV` reached every process: one started directly, and one started through bash, `/usr/bin/env`, `just`, or `cargo run`. On a developer's Mac with System Integrity Protection enabled, macOS is expected to remove the `DYLD_*` variables when it starts a protected binary such as `/bin/bash` or `/usr/bin/env`, so a variable that the `justfile` exports would not reach a test binary that such a binary starts. This was not tested. Without the variable, loading the library fails, because dyld then searches only system locations and the current directory. `DYLD_FALLBACK_LIBRARY_PATH` also works, but macOS is expected to remove it in the same way, and a symbolic link in `/usr/local/lib` does not work. `cargo run` itself sets `DYLD_FALLBACK_LIBRARY_PATH` to its target directories followed by `~/lib:/usr/local/lib:/usr/lib`.

These options are expected to survive System Integrity Protection, and none was tested with it enabled. Step 4 of Section 15, which implements the build script and the library path export, chooses among them and confirms or corrects SA-7:

- The recipe puts the assignment on the command line of the program that it runs, as in `DYLD_LIBRARY_PATH="$VULKAN_SDK/lib" cargo nextest run`, so that the protected shell sets the variable for Cargo, which is not protected.
- The build script places a symbolic link to the library inside `OUT_DIR`, which Cargo adds to the dynamic library path of the programs that it runs.
- An rpath, which SCS-26 forbids the build script to emit, and which, by A-6 of the architecture design, the build script of a dependency is assumed unable to set on the final binary.

## 11. Tests and Miri

SCS-33. Every test that calls native code, whether Slang or libclang, MUST sit in a module named `ffi`, and the nextest profile `default-miri` MUST exclude every test whose path contains a module named `ffi`, `gpu`, or `hw_gpu`, because Miri cannot call foreign code, and a test that reaches a foreign call under Miri fails without a defect in the code.

This answers TQ-5 of the tooling design. The selection by module name works like the GPU tiers (Section 5 of the tooling design). The `ffi` tests stay in the default test tier, so `test` and `test-gpu` run them. `check-miri` keeps the nextest form of Section 4.8 of the coding standard, which runs each test in its own process, because `cargo miri test` aborts the whole test binary at the first foreign call. The spike showed that `cargo +nightly miri nextest run` builds without linking the native library and passes when a filter excludes the tests with foreign calls. The build script still runs natively under Miri, so `check-miri` needs `VULKAN_SDK`, and whether Miri builds the test binary of the real crate is SA-9.

The crate `slang-compiler-sys` has one integration-test binary (TEST-10), `tests/sys/`, whose root is `tests/sys/main.rs`. Its module `pin`, which exists, holds three pure tests of the consistency of the pin constants: the commit is a full lowercase hexadecimal hash of 40 characters, the build tag names a release and, when it has a commit count, abbreviates the commit, and the SDK version has four numeric components. These tests call no native code, so they run under Miri. Its planned module `ffi` holds the smoke test, which is also the second test of TEST-8, because it references native symbols, so a broken link fails it. The smoke test creates a global session through `slang_createGlobalSession2`, checks that the build tag equals the build tag of the pin, queries the interface `ISlangUnknown` through `queryInterface` with the generated UUID, and releases every reference it holds.

SCS-34. The smoke test MUST assert that `spGetBuildTagString()` returns the build tag of the pin, on both routes, because a library that loads but is not the pinned build is the failure that SYS-6 and SCS-25 exist to prevent, and the run-time comparison catches a library that the loader found somewhere other than where the build script checked.

The layout assertions of SCS-12 are compile-time checks and need no test. The tests of the generator are those of Section 7.5.

## 12. Continuous Integration

SCS-35. From the step of Section 15 that first needs it, the Linux, Windows, and macOS jobs of the CI workflow MUST install the pinned Vulkan SDK from LunarG, verify the download against the SHA-256 checksum that LunarG publishes, with the expected value recorded in the workflow (Section 8.3 of the tooling design), set `VULKAN_SDK`, and put the SDK's library directory on the library path of the platform, because every job compiles `slang-compiler-sys` from the step that adds its build script, and every job that runs tests starts binaries that load Slang (TOOL-21, TOOL-23).

Step 2 of Section 15 implemented SCS-35 in every job of the workflow, on Linux, Windows, and macOS. Section 8.3 of the tooling design specifies the mechanism on each platform, including the download address, the installers, the recorded checksums, and the decision not to cache, and it meets TOOL-21, TOOL-23, ARCH-9, and SCS-23: each job reads the pin through `just pin`, so the workflow does not restate the version. After the installation, each job runs `just check-vulkan-sdk`, which checks the headers, the library, and `slangc` of the SDK against the build tag of the pin. The CI runs of that step confirmed SA-1 (Section 9.2), and confirmed the part of SA-2 that concerns the name and location of the import library and the DLL on `PATH` (Section 10). The `check` and `coverage` jobs on Linux also install libclang, so that the generator's tests find it (Section 7.5). Section 8.1 of the tooling design specifies that step and states how it meets TOOL-21.

SCS-36. The CI workflow MUST NOT build the static route on every pull request, because the route compiles several hundred C++ files, which would multiply the time of every run for a route that changes rarely.

This answers Q-28 of the architecture design. In M1, the static route is verified locally through the recipe `test-static` (Section 9.3 and step 5 of Section 15). Whether a scheduled CI job builds it is reconsidered in M7.

## 13. Crate Documentation

SCS-37. The crate-level documentation of `slang-compiler-sys` MUST contain the content of Sections 7.4 and 7.9 of the writing guide, including a `# Features` section that says the crate has no features, a `# Platform support` section that names the supported targets of Section 6, how the crate finds or builds Slang on each route and how the library is found at run time on each platform, and the safety contract that every thunk and C function shares, and MUST state the Slang rules of Section 3.9, because the generated items have no documentation of their own (SCS-13), and the design document of the safe crate cites these rules for its `Send`, `Sync`, and lifetime decisions.

The shared safety contract is the one that Section 7.9 of the writing guide requires, and it is what lets the generated items go without `# Safety` sections and safety comments (Section 5.10). It says that the caller passes only pointers that are valid, correctly typed, live, and obtained from Slang, such as a live interface pointer of the declared interface or a valid handle, that the caller meets the contract that Slang documents for the function, that the caller follows the threading and lifetime rules of Section 3.9, and that a call that breaks any of these is undefined behaviour. The documentation states that the crate promises nothing about threads: every function and thunk is `unsafe`, and no interface struct is `Send` or `Sync`. It links to Slang's documentation for the meaning of each item and points users to `slang-compiler`.

The example of the crate-level documentation creates a global session, and it is marked `no_run` because it needs the shared library at run time (Section 7.5 of the writing guide). The documentation of the hand-written public items, which are the constants of the pin, follows Section 7 of the writing guide in full.

## 14. Dependencies and Licences

SCS-38. The published crate `slang-compiler-sys` MUST have no dependencies and no build dependencies, because every dependency of a `-sys` crate runs on every user's machine, and the build script needs only the standard library to read a header, start processes, and compare strings.

The generator depends on the crate `clang` 2.1.0, and through it on `clang-sys`, `glob`, `libc`, `libloading`, `cfg-if`, and, on Windows, `windows-link`. These licences were verified with `cargo info` and `cargo tree` on 2026-11-10:

- `clang` and `clang-sys`: Apache-2.0.
- `libloading`: ISC.
- `glob`, `libc`, `cfg-if`, and `windows-link`: MIT OR Apache-2.0.

The licence allow-list of `deny.toml` is `GPL-3.0-or-later`, `Apache-2.0`, and `ISC`, which is exactly what cargo-deny needs for this graph (TOOL-24). Apache-2.0 covers `clang`, `clang-sys`, and the crates under `MIT OR Apache-2.0`, which cargo-deny accepts through either licence, so MIT is not in the list. ISC covers `libloading`. Apache-2.0 and ISC are compatible with GPL-3.0-or-later. The graph has no duplicate version, so `deny.toml` has no exception. The dependency is declared once in `[workspace.dependencies]` as `clang = { version = "2.1.0", features = ["runtime", "clang_10_0"] }` (DEP-2). `check-msrv` verifies that it builds with the workspace's `rust-version`.

## 15. Implementation Order for M1

M1 is carried out as these changes, in this order. Each change updates the documents that Hard Rule 1 of `AGENTS.md` requires, and leaves `just check` passing.

1. Done. This step added `crates/slang-compiler-sys/pin.rs`, its inclusion and documentation in the crate root, and the recipe `pin` (Section 8.1). It added the generator's library, binary, rule set, and shim headers, with the check of the build tag of the headers against the pin (Section 7.2) and the unit tests on synthetic headers (Section 7.5). It also added the integration-test binary of `slang-compiler-sys` with its module `pin` (Section 11), the dependency on `clang` with its licences in `deny.toml` (Section 14), the `[profile.default-miri]` table of SCS-33 in `.config/nextest.toml`, and the installation of libclang in the Linux CI job (Section 12). It confirmed SA-10. The generator has run on the headers of the pin only in a scratch crate (Section 3.11), because step 3 checks in its output.
2. Done. This step added the installation of the pinned SDK, verified against a checksum that the workflow records, to every job of the CI workflow on Linux, Windows, and macOS (SCS-35), with a step that reads the pin through `just pin`, and the check of the installed SDK against the pin (Section 12). It added the scripts `scripts/install-vulkan-sdk` and `scripts/check-vulkan-sdk` with their recipes, and the CI job `coverage`, and it rewrote the recipe `pin` so that it runs on the macOS and Windows runners (Section 8.1). It answered TQ-1 of the tooling design, confirmed SA-1, confirmed part of SA-2, and recorded why the hosted macOS runner cannot confirm SA-7 (Section 10).
3. Check in the generated bindings, which carry the `compile_error!` of SCS-15 at their top, with the module declaration and its lint exception (SCS-13), the shared safety contract in the crate-level documentation that the exception requires (Section 5.10), the opt-out of `unsafe_code`, the typos exclusion (SCS-14), the docs.rs targets of Section 9.4, the regeneration test, and the recipes `generate-bindings` and `check-bindings`.
4. Add the build script with route selection, the documentation build, and the dynamic route (Sections 9.1, 9.2, and 9.4 to 9.6), the `links` key, the smoke test (Section 11), the library path export of the `justfile` (SCS-32), and the recipe `check-doc-without-slang` in `check-rust`. This change confirms or corrects SA-2, SA-3, SA-7, and SA-9.
5. Add the static route (Section 9.3) and the recipe `test-static`, which builds the route and runs the smoke test on it, with the static library list recorded in Section 9.3. This change confirms or corrects SA-4, SA-5, and SA-6, and answers SQ-1.
6. Complete the crate-level documentation (Section 13), update `README.md` with how to obtain Slang, update `AGENTS.md` and the architecture design, and update Section 4.4 of the coding standard if the lints that fire on the generated module differ from its list.

This order differs from a plain sequence of generator, bindings, build script, and CI for four reasons. The pin file arrives in step 1 because the generator reads it to check the headers before it parses them (Section 7.2). The `default-miri` profile arrives in step 1 because the generator's tests are the first that call native code, and `check-miri` would fail on them without it. The SDK arrives in CI in step 2, on all three platforms at once, because the regeneration test of step 3 needs the headers in the Linux job, and the build script of step 4 needs the SDK in every job that compiles the crate, which includes the Windows and macOS jobs through Clippy and the tests. `check-doc-without-slang` arrives in step 4 and not with the documentation in step 6, because step 4 adds the code path that it tests.

## 16. Assumptions to Confirm

Each row is a fact that this design relies on and that neither the survey nor the spike verified. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SA-2 | On Windows, a Rust crate links against the import library `slang-compiler.lib` through `rustc-link-lib=dylib=slang-compiler` with the search path `$VULKAN_SDK/lib`. Step 2 of Section 15 confirmed the name and location of the import library, and that a program loads the DLL with `$VULKAN_SDK\Bin` on `PATH` (Sections 3.7 and 10) | SCS-26, Section 10 | Step 4 of Section 15, in the Windows job |
| SA-3 | The generated bindings link and the smoke test passes on Windows and macOS | Section 6 | Step 4 of Section 15, in the Windows and macOS jobs |
| SA-4 | The static route links exactly the libraries that the CMake target of the library links, which include those that Slang's building documentation lists (Section 3.10) | Section 9.3 | Step 5 of Section 15, by building the route |
| SA-5 | Disabling the options of Section 9.3 avoids every download at configure time and at build time | SCS-28 | Step 5 of Section 15, by configuring and building without network access |
| SA-6 | A static build without slang-glslang lacks the GLSL pass-through and the validation of SPIR-V by glslang, and nothing else that the safe crate needs | Section 9.3 | Step 5 of Section 15 |
| SA-7 | A library path that the `justfile` exports on macOS reaches the test binary, although macOS removes the `DYLD_*` variables when it starts a protected system program such as the system shell | SCS-32 | Step 4 of Section 15. The hosted macOS runner cannot confirm it, because System Integrity Protection was disabled there on 2026-11-10, and Section 10 records what was observed and the options for step 4 |
| SA-8 | docs.rs honours `targets` in `[package.metadata.docs.rs]` and builds no other target | Section 9.4 | The first docs.rs build, in M7 |
| SA-9 | Miri builds the test binary of `slang-compiler-sys` once the real build script emits link instructions | SCS-33 | Step 4 of Section 15, by running `check-miri` |

Step 1 of Section 15 confirmed SA-10, that a preparation step can install libclang on the Linux CI runner where `clang-sys` finds it, and removed its row. Section 7.5 states the result. Step 2 of Section 15 confirmed SA-1, that the Vulkan SDKs for Windows and macOS ship the Slang headers, including `slang-tag-version.h`, under `include/slang`, and the library under `lib`, as on Linux, which is A-8 of the architecture design, and removed its row. Sections 3.6, 3.7, and 9.2 state the result. The next assumption takes SA-11.

Assumptions A-6 and A-7 of the architecture design, that a dependency's build script cannot set the rpath of the final binary and that docs.rs sets `DOCS_RS`, also apply to this design and stay where the architecture design records them.

## 17. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SQ-1 | Does the static route support targets other than x86_64 Linux, and cross-compilation, or does the build script stop with an error on them? | This document | M1, step 5 of Section 15 |

## 18. Security Considerations

The threat model has six parts. First, the Slang library and its headers come from the Vulkan SDK or from a source tree that the user names, and a defect in them, or a tampered copy, reaches every program that links the crate. Second, the build script runs on the user's machine with the user's permissions, reads environment variables, and, on the static route, runs `git`, CMake, and a native build of a large C++ code base from the source tree that `SLANG_SOURCE_DIR` names. Third, the binding generator trusts the headers it parses and writes Rust declarations that the compiler trusts without checking them against the library. Fourth, an agent edits the checked-in bindings by hand, or changes the generator so that it writes a wrong declaration. Fifth, the generator's dependencies, such as `clang` and `clang-sys`, run their build scripts on the machines of maintainers. Sixth, a CI job downloads the Vulkan SDK, and an attacker on the network or at the download host tampers with it.

Unsafe code and FFI invariants. The crate relies on the native library being exactly the pinned build, which the tag check of SCS-25, the commit check of SCS-27, and the run-time assertion of SCS-34 secure, and on the declarations matching the headers, which the regeneration check (Section 7.6), the layout assertions (SCS-12), and the failures of SCS-18 and SCS-19 secure. It guarantees its callers nothing beyond exact declarations: every function and thunk is `unsafe`, no interface struct is `Send` or `Sync`, enumerations and `SlangResult` are integer newtypes so that an unknown value is not undefined behaviour (SCS-7, SCS-8), and thunks take raw pointers so that no Rust reference asserts an aliasing guarantee about a native object (SCS-10). The crate documentation states the shared safety contract and Slang's rules (SCS-37).

Untrusted shader source and file paths. This crate compiles nothing and reads no shader source. The safe crate decides which files Slang can read, as Q-13 of the architecture design assigns.

Build scripts that run native builds. The dynamic route reads one header and emits instructions, and it searches no location other than `VULKAN_SDK` (SCS-26). The static route runs CMake on a source tree that the user chose, writes only under `OUT_DIR` (SYS-5), disables every component that downloads (SCS-28), and refuses a tree at another commit (SCS-27). The route is selected only by variables that the person who builds sets (SCS-24), and no setting bypasses the version checks.

Dependencies. The published crate has no dependencies (SCS-38). The generator's dependencies are checked by cargo-deny (TOOL-24, TOOL-26), and they run only for maintainers, never on a user's machine (ARCH-2). The native library is fixed to one SDK version and one Slang commit by the pin (SCS-23), and CI verifies every SDK download against a checksum recorded in the workflow (SCS-35).

The checked-in file is a review surface of several thousand lines, which no reviewer reads line by line. The regeneration check makes a hand edit fail `just check` on Linux, so a reviewer reviews the change to the generator, its rule set, and the summary of the regenerated diff instead of the whole file. The rule set records a reason for every decision, and the generator fails on a declaration that no rule decides (SCS-2) and on a rule that decides nothing (SCS-39), so the rule set that the reviewer reads matches the headers in both directions. A change to the generator that writes a wrong declaration and the regenerated file together pass the regeneration check, which is why the generator's unit tests (Section 7.5) and the reviewer's reading of the generator change carry that risk.

These attacks are out of scope. A compromised Vulkan SDK or Slang release from LunarG or the Slang project runs with the full privileges of the program that links it, and no crate can constrain it. A source tree that the user names for the static route is code that the user chose to build, and CMake runs it with the user's permissions. A defect in libclang that silently misreports a declaration without any diagnostic cannot be detected by the generator, though the comparison between targets and the layout assertions narrow it. Attacks on the repository itself are a matter of access control.

The residual risk has five parts. First, the layout assertions check structs and unions, not function signatures, so a wrong parameter type in a function or a vtable entry that the generator's rules allow compiles and is undefined behaviour, and the smoke test exercises only a few functions. Second, the commit check of the static route reads `HEAD`, so a modified working tree at the pinned commit builds a library that differs from the headers. Third, the build tag check compares an abbreviated commit hash, and a different build with the same tag is not detected by either check, because the run-time assertion of SCS-34 compares the same string. Fourth, Slang is a large C++ code base whose defects reach every program that links it, as Section 18 of the architecture design states. Fifth, Miri cannot run any test that calls Slang (SCS-33), so the soundness of the declarations rests on the generator and its tests, not on Miri.

## 19. Operational Considerations

### 19.1. Cargo Features

`slang-compiler-sys` has no Cargo features, and neither does the binding generator. The environment selects the route of the build script (Section 9.1).

### 19.2. Environment Variables

This table lists every environment variable that the build script or the generator reads, with its default. The `CARGO_CFG_TARGET_*` variables and `OUT_DIR`, which Cargo always sets, are not options.

| Variable | Read by | Default and behaviour when absent |
| --- | --- | --- |
| `DOCS_RS` | The build script, always | Absent, so the build proceeds on the route that Section 9.1 selects. When present, with any value, the build script returns at once (SCS-29). |
| `SLANG_SOURCE_DIR` | The build script, when `DOCS_RS` is absent | Absent, so the dynamic route applies. When present, the static route applies, and an empty value or a path that is not a directory stops the build (Section 9.6). |
| `VULKAN_SDK` | The build script on the dynamic route, the generator, the regeneration test, and the library path export of the `justfile` | No default. On the dynamic route, absence stops the build (Section 9.6). The generator's binary fails without it, or with it empty, with a message that names `VULKAN_SDK` and the pinned SDK version, and the regeneration test fails without it (SCS-22). The `justfile` exports no library path without it (SCS-32). The development container sets it. |
| `LIBCLANG_PATH` | `clang-sys`, when the generator or its tests run | Absent, so `clang-sys` searches the locations that its documentation lists. When libclang is not found there, the generator fails with an error of kind `Libclang` that starts with "cannot load libclang" and advises to install libclang or set `LIBCLANG_PATH` (Section 7.7). |

### 19.3. Platforms and Minimum Supported Rust Version

Section 6 lists the supported targets and SCS-15 stops compilation on any other. The binding generator runs on Linux only (Sections 7.5 and 7.6), and it needs libclang and `rustfmt` on the `PATH` at run time. Its crate builds on every platform without libclang. Both crates use the workspace's `rust-version` (TOOL-4), which `check-msrv` verifies.

### 19.4. Failure Modes

This table lists the failure modes that this design defines, how each is reported, and the recovery.

| Failure | Reported by | Recovery |
| --- | --- | --- |
| A failure of the build script in Section 9.6 | The error that the build script returns, with the content of Section 9.6 | Install the pinned SDK and set `VULKAN_SDK`, check out the pinned commit, or fix the toolchain that the message names |
| A target whose pointer width is not 64 | The `compile_error!` of SCS-15 | Build for a supported target |
| The shared library is not found at run time | The dynamic loader, which stops the program before `main` | Put the SDK's library directory on the platform's library path (Section 10) |
| The library at run time is not the pinned build | The smoke test (SCS-34), and from M2 the safe crate | Make the loader find the pinned SDK's library |
| libclang cannot be loaded | The generator or its tests, with an error of kind `Libclang` that gives the reason (Section 7.7) | Install libclang or set `LIBCLANG_PATH` |
| The process already holds a libclang instance | The generator, with an error of kind `Libclang` that says so (Section 7.7) | Generate the bindings once per process and from one thread |
| A parse reports an error diagnostic, a target difference has no rule, a construct of SCS-19 appears, or a declaration matches no rule | The generator, with the diagnostic, the difference, the construct, or every unmatched declaration, each with its location | Fix the shim headers, add a rule to the rule set, or extend the generator and this design |
| A rule decides nothing | The generator, with every stale rule and its reason (SCS-39) | Remove or correct the rule, and update Section 4.3 or 4.4 when it records the rule |
| rustfmt cannot be started or fails | The generator, with an error of kind `Rustfmt`, which holds rustfmt's exit status and standard error when rustfmt fails (Section 7.7) | Install rustfmt, or fix the generator when the generated text does not parse |
| The headers do not match the pin | The generator, with both build tags (Section 7.2) | Install the pinned SDK, or move the pin (Section 8.2) |
| The checked-in bindings differ from a regeneration | The regeneration test and `check-bindings` | Run `just generate-bindings` and commit the result with the change that caused it |

### 19.5. Logs

The build script emits no warnings and no log, and it prints the output of a native build only in the error of a failure (Section 9.6). The generator prints its errors to standard error. Neither crate emits logs at run time.

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
| SCS-5 | 5.1 | One rustfmt-formatted file, `src/bindings.rs`, in a private module re-exported at the crate root. |
| SCS-6 | 5.2 | Types map by typedef name, never by canonical type. |
| SCS-7 | 5.3 | Named enumerations are `#[repr(transparent)]` integer newtypes, unnamed ones are free constants of their assigned typedef, and none is a Rust enum. |
| SCS-8 | 5.4 | Macro constants come from a synthetic translation unit and an allow-list, without platform macros, and `SlangResult` is an `i32` newtype with the result codes as its constants. |
| SCS-9 | 5.5 | Structs and unions are `#[repr(C)]`, with a `Default` that reproduces C++ default member initializers. |
| SCS-10 | 5.6 | Each interface has a struct, a vtable struct with the base first, a UUID through a trait, and thunks on raw pointers. |
| SCS-11 | 5.7 | C functions are in `unsafe extern "C"` blocks, and opaque types cannot be constructed. |
| SCS-12 | 5.8 | Compile-time assertions of size, alignment, and field offsets for every struct and union. |
| SCS-13 | 5.10 | No comments or item documentation except one `//!` line, and one `#[expect]` on `mod bindings;` with the lints that fire. |
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
| SCS-26 | 9.2 | The dynamic route links `slang-compiler` from `$VULKAN_SDK/lib`, without pkg-config, rpath, or other locations. |
| SCS-27 | 9.3 | The static route requires a git checkout whose `HEAD` is the pinned commit, with no fallback. |
| SCS-28 | 9.3 | The static route runs CMake through `Command` inside `OUT_DIR`, disables downloads, sets the version, and links statically. |
| SCS-29 | 9.4 | With `DOCS_RS` present, the build script returns at once. |
| SCS-30 | 9.5 | Rerun instructions for every variable and file that the build script reads. |
| SCS-31 | 9.6 | Each build failure returns an error with the content of the table. |
| SCS-32 | 10 | The `justfile` exports the SDK library path on Linux and macOS and works without `VULKAN_SDK`. |
| SCS-33 | 11 | Tests that call native code sit in `ffi` modules, which `default-miri` excludes with `gpu` and `hw_gpu`. |
| SCS-34 | 11 | The smoke test asserts the pinned build tag at run time on both routes. |
| SCS-35 | 12 | CI installs and verifies the pinned SDK on all three platforms and sets the paths. |
| SCS-36 | 12 | CI does not build the static route on every pull request. |
| SCS-37 | 13 | The crate documentation has the content of the writing guide, the shared safety contract, and Slang's rules. |
| SCS-38 | 14 | The published crate has no dependencies. |
| SCS-39 | 4.1 | A rule that decides no declaration or macro fails the generator, with each stale rule and its reason. |

## Change Log

- 2026-11-10: Recorded step 2 of Section 15 as done: the installation of the Vulkan SDK in CI in Sections 1.5, 1.6, and 12, the build tag and the libraries of the SDKs for Windows and macOS in Sections 3.6 and 3.7, the portability of the recipe `pin` in Section 8.1, the confirmed layout in Section 9.2, the library paths on the runners and the macOS observations with the options for SA-7 in Section 10, the confirmation of SA-1 with the removal of its row, the narrowed SA-2 and the updated SA-7 in Section 16, and the dating of the observations from the runners in Sections 3.7, 8.1, and 10.
- 2026-11-10: Recorded step 1 of Section 15 as done and made the design agree with the implementation: the current state in Section 1.6, the findings of the generator in Section 3.11, the rule set, its exclusions, and its unnamed enumeration rules in Sections 4.1, 4.3, and 4.4 with SCS-1 reworded and SCS-39 added, the constants of unnamed enumerations in SCS-7, the names, derives, and paths in Section 5.11, the `compile_error!` at the top of the generated file in Section 5.1, SCS-15, and step 3, the verified lints in Section 5.10, the refusals of SCS-19, the module structure, tests, and error reporting in Sections 7.4, 7.5, and 7.7, the pin constants and the recipe `pin` in Section 8.1, the integration-test binary in Section 11, the confirmation of SA-10, and the corrected nesting rule of Section 4.1, exclusions of Section 4.3, refusals of SCS-19, error kind `Io`, and test counts.
- 2026-11-10: Initial version.
