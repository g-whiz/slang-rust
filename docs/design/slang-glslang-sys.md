# `slang-glslang-sys` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-19 |
| Applies to | `crates/slang-glslang-sys` |

## Abstract

This document specifies `slang-glslang-sys`, the crate that builds slang-glslang, which is glslang, SPIRV-Tools, and Slang's glue code, as static libraries from the Slang source tree that `slang-src` obtains, and that holds an in-process shared library loader through which a statically linked Slang uses them. Without it, a vendored build of Slang finds no slang-glslang and emits SPIR-V that is not optimized. The document defines the wrapper build, what the crate links, the loader shim and how it reaches Slang's default loader, the one function that the crate exports, who installs the loader, the unsafe code of the crate, the build requirements, the failure modes, and the tests.

## 1. Introduction

### 1.1. Problem

Slang optimizes the SPIR-V that it generates through slang-glslang, a library that Slang's CMake build produces only as a module that Slang loads by name at run time (Section 3.10 of `docs/design/slang-compiler-sys.md`). A statically linked Slang finds no such module unless one lies on the loader path of the program, so a vendored build emits SPIR-V that is not optimized and reports a diagnostic on the first generation in each global session. When a module of another Slang build happens to lie on the loader path, the output depends on the machine that runs the program.

Slang loads every library through the shared library loader of its global session, and `IGlobalSession::setSharedLibraryLoader` replaces that loader. A loader that answers the request for slang-glslang with functions linked into the program gives a statically linked Slang its optimizer without any file at run time.

### 1.2. Purpose

`slang-glslang-sys` builds glslang, SPIRV-Tools, Slang's glue code, and a loader shim as static libraries, links them into the program, and exports one function that returns the loader. `slang-compiler` installs that loader on every global session that it creates in a vendored build, so a vendored build emits optimized SPIR-V.

This document implements the part of Section 15.10 of `docs/design/architecture.md` that concerns slang-glslang. It owned Q-32 of the architecture design, which the tests of Section 9 answer (Section 14).

### 1.3. Audience

The primary readers are the coding agents that change the crate, its wrapper build, its shim, or the code of `slang-compiler` that installs the loader. The secondary readers are the reviewers of those changes and the owner. A reader is expected to know Rust, Cargo build scripts, and C++, but not the internals of Slang.

### 1.4. Scope

This document covers the wrapper build and its options, what the crate links, the loader shim, the exported function and its Rust declaration, who installs the loader, the build script, its tests, the crate documentation, and the security and operational considerations.

### 1.5. Non-Goals

This document does not cover:

- The build of Slang itself, which `docs/design/slang-compiler-sys.md` specifies.
- How the tree is obtained and the scheme of the shared folder, which `docs/design/slang-src.md` specifies.
- The code of `slang-compiler` that installs the loader, which `docs/design/slang-compiler.md` specifies. This document states the contract that the code meets.
- Downstream compilers other than slang-glslang, such as the DirectX Shader Compiler (DXC), NVRTC, or the LLVM host target, which the shim passes to Slang's default loader unchanged.
- A loader for the dynamic route, where the Vulkan SDK ships slang-glslang next to the Slang shared library.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that these documents define:

- Section 2.2 of `docs/writing-documentation.md`, such as agent, role, and user.
- Section 2.2 of `docs/rust-coding-standards.md`, such as interface, interface pointer, native library, and `-sys` crate.
- Section 2.1 of `docs/design/architecture.md`, such as CMake, global session, slang-glslang, SPIR-V, vendored build, and vendored route.
- Section 2.1 of `docs/design/slang-compiler-sys.md`, such as build tag and host.
- Section 2.1 of `docs/design/slang-src.md`, such as dependent build script, entry, entry key, recorded Slang, shared folder, source tree, and stamp file.

This document calls those files the writing guide, the coding standard, the architecture design, the sys design, and the `slang-src` design. It adds these terms:

- default loader: Slang's own shared library loader, a static object of the class `Slang::DefaultSharedLibraryLoader`, which loads a library from the file system with the platform's dynamic loader.
- glslang: The reference compiler of the OpenGL Shading Language (GLSL), which Slang uses to link and validate SPIR-V.
- loader shim: The C++ code of this crate, `native/loader.cpp`, that implements Slang's interface `ISlangSharedLibraryLoader` over the linked functions (Section 5).
- position-independent code (PIC): Machine code that runs at any address, which a Rust executable needs from every static library that it links when it is built as a position-independent executable.
- shared library loader: An object that implements Slang's interface `ISlangSharedLibraryLoader`, through which a global session loads every library that it needs at run time.
- SPIRV-Tools: The library of tools for SPIR-V, including the optimizer that Slang runs through slang-glslang.
- wrapper project: The CMake project of this crate, `native/CMakeLists.txt`, which builds glslang, SPIRV-Tools, the glue code, and the loader shim from the source tree (Section 4).

## 3. Facts

This section is non-normative. It states the facts about Slang on which the later sections rely. Section 13 records which of them are assumptions to confirm.

The glue code of slang-glslang is the file `source/slang-glslang/slang-glslang.cpp` of the source tree. It includes `slang.h` and no generated header of Slang, so it compiles without a configure step of Slang. It exports nine C functions, which the export list of the module, `slang-glslang.version-script`, names. Their types are those of `source/slang-glslang/slang-glslang.h`:

| Function | Returns | Parameters |
| --- | --- | --- |
| `glslang_compile` | `int` | A `glslang_CompileRequest_1_0` pointer |
| `glslang_compile_1_1`, `glslang_compile_1_2`, `glslang_compile_1_3` | `int` | A pointer to the request struct of the same version |
| `glslang_disassembleSPIRV`, `glslang_validateSPIRV` | `bool` | The SPIR-V words and their count |
| `glslang_disassembleSPIRVWithResult` | `bool` | The SPIR-V words, their count, and an out-pointer to the text |
| `glslang_freeDisassembly` | `void` | The text that the previous function returned |
| `glslang_linkSPIRV` | `int` | A `glslang_LinkRequest` pointer |

The glue code marks the nine functions `__declspec(dllexport)` when the Microsoft compiler compiles it. Linked statically into a Rust executable, they make `link.exe` export them from the executable, and it writes an import library and an export file next to the executable. The exports change nothing at run time, because the shim returns the addresses from its own table (Section 5).

Slang looks for slang-glslang through the loader of the global session, by the name `slang-glslang-<numeric version>`, such as `slang-glslang-2026.17`. On Unix, it first asks the same loader for `pthread` and then `libpthread.so.0`, and ignores the result.

`IGlobalSession::getSharedLibraryLoader` returns null while the default loader is installed, so a loader cannot reach the default loader through it (Section 13, SGLA-1). The default loader is reachable through the static member function `Slang::DefaultSharedLibraryLoader::getSingleton()`, declared in the internal header `source/core/slang-shared-library.h`, which needs the include directory of the submodule `external/unordered_dense` and C++20.

`IGlobalSession::setSharedLibraryLoader` returns `void` in the recorded Slang, so installing a loader reports no result and cannot fail.

Slang's static libraries contain no code of glslang or SPIRV-Tools, so linking both sets of archives into one program duplicates no symbol of either.

The wrapper build takes a few minutes, and its five archives occupy about 23 MB. It runs only after the build of Slang (SGL-1), so the first vendored build in a target directory takes the time of both builds in sequence.

CMake mirrors the absolute path of each source file outside the wrapper project under `CMakeFiles/<target>.dir/` of the build tree. The build tree therefore holds deep paths that repeat the path of the source tree, including directories named `slang-src` (SRCQ-1 of the `slang-src` design).

## 4. The Wrapper Build

SGL-1. The manifest of `slang-glslang-sys` MUST declare `links = "slang-glslang"`, and the crate MUST depend on `slang-src` with its exact version and on `slang-compiler-sys` with the feature `vendored`. It MUST have no Cargo features. The crate exists only for a vendored build, and the dependency on `slang-compiler-sys` makes rustc place this crate's libraries before Slang's on the link line, which the loader shim needs to resolve the default loader from Slang's library `core` (Section 6).

The exact version of `slang-src` keeps the tree of this crate and the tree of `slang-compiler-sys` the same tree (SRC-14 of the `slang-src` design), and `links` keeps two builds of slang-glslang out of one graph.

Cargo runs a build script only after the build scripts of its dependencies, so the dependency on `slang-compiler-sys` also makes the wrapper build start only after the build of Slang has finished. Without the dependency, the two builds would run side by side and the first vendored build would take less time. The order of the libraries on the link line would then depend on the order that rustc gives to siblings, so the crate keeps the dependency.

The crate takes the version of the workspace, and the workspace requires it with a plain version, not an exact one, as it requires `slang-compiler-sys`. Its own dependencies fix the tree that it builds.

The crate is created under the licence of Slang, `Apache-2.0 WITH LLVM-exception`, which its manifest sets in its own `license` field instead of inheriting the licence of the workspace (ARCH-36 of the architecture design, TOOL-4 of `docs/design/tooling.md`).

SGL-2. The build script MUST build the wrapper project with CMake and the Ninja generator out of tree, in `OUT_DIR/build`, reading the source tree in place from `DEP_SLANG_SRC_ROOT`. It MUST NOT copy the tree and MUST NOT write into it. It MUST run every CMake command with `PYTHONDONTWRITEBYTECODE=1` in its environment.

glslang and SPIRV-Tools write nothing into their source directories except the Python bytecode of the grammar table generator of SPIRV-Tools, which `PYTHONDONTWRITEBYTECODE=1` suppresses (Section 13, SGLA-2). A build in place then needs no copy, and two builds can read one tree at once, such as the builds of two profiles that read the one clone in the shared folder. A write into the tree would break SYS-5 and race with another build that reads the clone. On Linux, `git status --short --ignored` in the tree and in the submodules `external/spirv-tools`, `external/glslang`, and `external/spirv-headers` prints nothing after the build.

The wrapper project adds the submodules `external/spirv-headers`, `external/spirv-tools`, and `external/glslang` of the source tree with `add_subdirectory` and `EXCLUDE_FROM_ALL`, in that order, and defines the static library `slang-glslang-static` from the glue code and the loader shim. The build script builds the one target `slang-glslang-static`, and CMake builds every library that it links first. The wrapper project stops the configure step with a fatal error when `SLANG_TREE_DIR` is not a directory.

`OUT_DIR` holds the build tree `build`, the directory `lib` of the five archives, and the log `build.log`. The build script keeps no stamp in `OUT_DIR`, so it configures and builds on every run that finds no complete entry (SGL-6), and Ninja keeps the object files of the previous run. Before every configure step, it removes `build/CMakeCache.txt`, because CMake keeps the compiler of its first configure step in that cache and ignores a changed `CXX` or toolchain. Without the removal, a build with another compiler would store archives of the old compiler under the key of the new one.

SGL-3. The wrapper project MUST set the options of glslang and SPIRV-Tools that Slang's own build sets, MUST build every library as a static library with position-independent code, and MUST turn off the tests, the executables, the installation, and the use of mimalloc. It MUST compile the glue code and the shim as C++20 with hidden visibility, also for inline functions, and on Windows with the definitions `NOMINMAX`, `WIN32_LEAN_AND_MEAN`, `VC_EXTRALEAN`, `UNICODE`, and `_UNICODE`, as Slang's build compiles the glue code.

The archives then hold the same code that Slang's own build of slang-glslang compiles (Section 13, SGLA-3), and a Rust executable can link them. Without the definitions, `<windows.h>` defines macros that break the glue code. mimalloc would add a submodule and a library to link on Windows, where SPIRV-Tools uses it by default, for no benefit inside Slang.

These are the options that the wrapper project sets before it adds the submodules:

| Option | Value | Reason |
| --- | --- | --- |
| `CMAKE_POSITION_INDEPENDENT_CODE` | `ON` | PIC for a Rust executable |
| `BUILD_SHARED_LIBS` | `OFF` | Static libraries only |
| `BUILD_TESTING` | `OFF` | No tests of the submodules |
| `SPIRV_TOOLS_BUILD_STATIC` | `ON` | As Slang's build |
| `SPIRV_WERROR` | `OFF` | As Slang's build |
| `SPIRV_SKIP_TESTS` | `ON` | As Slang's build |
| `SPIRV_SKIP_EXECUTABLES` | `ON` | No command-line tools |
| `SKIP_SPIRV_TOOLS_INSTALL` | `ON` | As Slang's build |
| `SPIRV_TOOLS_USE_MIMALLOC` | `OFF` | No mimalloc |
| `SPIRV_TOOLS_USE_MIMALLOC_IN_STATIC_BUILD` | `OFF` | No mimalloc |
| `SKIP_GLSLANG_INSTALL` | `ON` | As Slang's build |
| `ENABLE_OPT` | `ON` | As Slang's build, the optimizer |
| `ENABLE_PCH` | `OFF` | As Slang's build |
| `ALLOW_EXTERNAL_SPIRV_TOOLS` | `OFF` | As Slang's build |
| `GLSLANG_ENABLE_INSTALL` | `OFF` | No installation |
| `GLSLANG_TESTS` | `OFF` | No tests |
| `ENABLE_GLSLANG_BINARIES` | `OFF` | No command-line tools |
| `BUILD_EXTERNAL` | `OFF` | No further downloads |

The library `slang-glslang-static` reads the include directories `include`, `source`, and `external/unordered_dense/include` of the source tree.

The configure step is `cmake -S <native> -B <OUT_DIR>/build -G Ninja`, followed by `-DCMAKE_BUILD_TYPE=Release` and `-DFETCHCONTENT_FULLY_DISCONNECTED=ON`, the runtime library option of the target, `-DSLANG_TREE_DIR=<root>`, and `-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=<OUT_DIR>/lib`. `<native>` is the directory `native` of the crate. With the Microsoft toolchain, the runtime library option is the one that the vendored route of `slang-compiler-sys` passes, so that both builds use one runtime (Section 9.3 of the sys design). The build step is `cmake --build <OUT_DIR>/build --target slang-glslang-static`, followed by `--parallel` and the value of `NUM_JOBS` when that value is a whole number.

SGL-4. The build script MUST check after the build that each of the five libraries of this table exists as a file in `OUT_DIR/lib`, named `lib<name>.a`, or `<name>.lib` with the Microsoft toolchain. It MUST emit a search path for the directory `lib` of its entry in the shared folder (SGL-6) and these link instructions, in this order. They are followed by the C++ standard library of the target as the vendored route of `slang-compiler-sys` emits it, and then by the system libraries of the target:

| Library | Content |
| --- | --- |
| `slang-glslang-static` | The glue code and the loader shim |
| `glslang` | glslang, including its SPIR-V back end |
| `SPIRV-Tools-opt` | The SPIR-V optimizer |
| `SPIRV-Tools-link` | The SPIR-V linker |
| `SPIRV-Tools` | The core of SPIRV-Tools |

A library that the build did not produce would otherwise fail in the linker of the user's program with a message that does not name the cause. Each library precedes the libraries that it uses, which the static linkers of Linux need. The library `SPIRV` that glslang also builds holds no code and is not linked.

The system libraries are `pthread` on Linux, which glslang declares through `Threads::Threads` in `external/glslang/glslang/CMakeLists.txt`, none on macOS, and, with the Microsoft toolchain, the ten libraries that CMake links into every program and library by default, which SCS-69 of the sys design lists. Slang's own build of the slang-glslang module gets those ten from CMake as well. SPIRV-Tools declares no system library apart from mimalloc, which is off. Each system library is a `dylib` link instruction, after the C++ standard library.

SGL-5. The build script MUST stop the build only where a correct build is impossible: when `TARGET` differs from `HOST`, because its dependency, the vendored `slang-compiler-sys`, cannot cross-compile Slang, and for a target whose C++ runtime library has no link rule of SCS-43 of the sys design, because the link of its C++ libraries would fail. It MUST NOT stop the build because a target is not one of the tested targets, which the build script of `slang-compiler-sys` states in its warning (SCS-65 of the sys design).

The link rules of the two crates are the same, so this crate links on every target on which the vendored `slang-compiler-sys`, its dependency, links. A build of slang-glslang for a target that Slang is not built for has no use.

The message of a cross-compilation names both triples and says that the build of slang-glslang cannot cross-compile, because the Slang that it serves is built from source for the host only. The message of a target without a link rule names the triple and the three kinds of target that have a rule. Both end with the two remedies: build on a host of the target, or build without the Cargo feature `vendored` of the crate that enables `slang-glslang-sys`, such as `slang-compiler`, to link the Slang of a Vulkan SDK.

SGL-6. The build script MUST keep the five libraries of each verified build in an entry of the directory `slang-glslang-sys` of the shared folder that `DEP_SLANG_SRC_SHARED_DIR` names, with the entry protocol of SRC-16 and the entry key of SRC-17 of the `slang-src` design, and MUST use a complete entry whose inputs equal those of the current build instead of building. An entry holds the five libraries under `lib`, the file `inputs`, and the stamp file.

The build takes minutes, and Cargo gives each profile and each host or target unit its own `OUT_DIR`, so the build would otherwise repeat for every profile and for a procedural macro that links Slang (Section 8 of the `slang-src` design). With the shared folder, it runs once for each set of inputs in a target directory.

The inputs of the key follow SRC-17 of the `slang-src` design, with these values:

- `files` is the FNV-1a hash of the bytes `CMakeLists.txt`, a NUL byte, the content of `native/CMakeLists.txt`, a NUL byte, `loader.cpp`, a NUL byte, the content of `native/loader.cpp`, and a NUL byte. The build logic compiles both files in with `include_str!`, so the hash is the hash of the files that the build script was compiled with.
- `configure` holds the lines `-DCMAKE_BUILD_TYPE=Release` and `-DFETCHCONTENT_FULLY_DISCONNECTED=ON`, and, with the Microsoft toolchain, the runtime library option. `-S`, `-B`, `-DSLANG_TREE_DIR=`, and `-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=` name paths, and `-G Ninja` never changes, so they are left out.
- `compiler` is found as SCS-56 and SCS-68 of the sys design state for `slang-compiler-sys`.

The variables through which the compiler reads include and library directories are not inputs of the key, which Section 9.4 of the `slang-src` design states as a limit of every entry of libraries.

An entry is complete only when its file `inputs` equals the inputs of the current build byte for byte and each of the five archives exists as a file in its `lib`. The build script replaces an entry that fails either check.

The build script writes the stamp file of a new entry and then sets its modification time, as two steps. A failed write stops the build with "cannot write" and the path, and a failed setting of the time stops it with "cannot set the modification time of" and the path. Each message ends with the reason that the operating system gives and the advice that removing the entry, or running `cargo clean`, when no build runs is safe.

SGL-7. The build script MUST emit `cargo::rerun-if-env-changed` for `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `CMAKE_TOOLCHAIN_FILE`, and `MACOSX_DEPLOYMENT_TARGET`, which CMake reads, and for `VCINSTALLDIR` and `VCToolsVersion`, which steer the lookup of `cl.exe`, in that order. It MUST emit `cargo::rerun-if-changed` for the stamp file of the entry that it uses. A change of these inputs changes the libraries, and a build that does not rerun links libraries that no longer match them (SYS-1).

Once a build script emits a rerun instruction, Cargo no longer reruns it when another file of the package changes. The build logic compiles in `native/CMakeLists.txt` and `native/loader.cpp` with `include_str!`, so a change of either recompiles the build script, which then runs again, and the same text feeds the `files` input of SGL-6. Cargo also reruns the build script when the metadata of `slang-src` changes (Section 16, SRCA-1, of the `slang-src` design).

The build script follows SYS-4 and SYS-8: it runs `ninja --version`, `cmake --version`, the compiler probe, and the two CMake steps with `std::process::Command`, checks every exit status, appends the output of the CMake steps to `OUT_DIR/build.log`, which it truncates at the start of each build, and on a failure returns an error that names the command, the log, and its last 40 lines. Its logic is in `build_logic.rs`, behind the trait `Host`, which the build script and the tests both include, and the real host is in `system.rs`.

### 4.1. Code Shared by Copy

The build logic of this crate repeats parts of `route.rs` and `system.rs` of `slang-compiler-sys`, with the parts that concern Slang removed:

- The types `Invocation`, `Exit`, `Finished`, and `BuildError`.
- The selection of the platform and the toolchain, the compiler probe, and the error of a failed lookup of `cl.exe`.
- The FNV-1a hash, the entry protocol, the logged runs, and the checks of exit status.
- The real `System`.

The copies of the entry protocol in `slang-src`, `slang-compiler-sys`, and this crate are the three copies of SRC-16 of the `slang-src` design.

The crates keep copies, not shared code, because no shared place fits. A build script cannot `include!` a file of another published package, `slang-src` has no API (SRC-2 of the `slang-src` design), and a crate of build helpers would be one more published crate that every vendored build compiles. A change to one copy is checked against the others in the same change.

## 5. The Loader Shim

SGL-8. The loader shim MUST implement `ISlangSharedLibraryLoader` so that a request whose file name, after any directory and an optional `lib` prefix, starts with `slang-glslang` returns a library whose `findSymbolAddressByName` returns the address of each of the nine functions of Section 3 by its name and null for any other name. It MUST pass every other request to the default loader unchanged and return its result.

The file name is the part of the path after the last `/` or `\`, so both separators strip a directory on every platform, and a directory whose name starts with `slang-glslang` does not match.

A match on the prefix also answers a Slang whose numeric version differs from the recorded one, such as the commit of a fork of `slang-src` patched in through `[patch]` (SRC-22 of the `slang-src` design), whose module name carries its own version. Every other library goes to the default loader, so DXC, NVRTC, and the LLVM host target load from the file system as they do without the shim (Section 13, SGLA-4). A shim that refused every other name would disable those downstream compilers in a vendored build.

The shim declares the nine functions in an `extern "C"` block with the types of `slang-glslang.h` (Section 3) and keeps their addresses in a table, so it needs no symbol of the program to be exported. The archive of the glue code therefore needs no export list on any platform, and hidden visibility does not affect the lookup.

SGL-9. The loader shim MUST reach the default loader through `Slang::DefaultSharedLibraryLoader::getSingleton()` from the internal header of the source tree that it compiles against. `getSharedLibraryLoader` returns null while the default loader is installed (Section 3), and a reimplementation of the default loader in the shim would drift from Slang's. The shim compiles against the header of the same tree that `slang-compiler-sys` builds, so a change of that internal name fails the build of this crate instead of misbehaving at run time.

SGL-10. The loader and the library that the shim returns MUST be objects with static storage duration whose `addRef` and `release` return 1, change no state, and never free anything. Their interface queries MUST answer only their own interfaces:

| Object | Interfaces that `queryInterface` answers | Interfaces that `castAs` answers |
| --- | --- | --- |
| The loader | `ISlangUnknown`, `ISlangSharedLibraryLoader` | Not applicable, the interface has no `castAs` |
| The library | `ISlangUnknown`, `ISlangCastable`, `ISlangSharedLibrary` | The same three |

For any other UUID, `queryInterface` returns `SLANG_E_NO_INTERFACE` and writes null, and `castAs` returns null. `ISlangSharedLibrary` derives from `ISlangCastable`, so Slang can ask the library for either.

The objects hold no mutable state, so a global session on any thread can use them at any time, and a reference that Slang keeps after a global session ends never dangles.

The shim calls no function of Slang other than `getSingleton` and the methods of the default loader, and it never installs itself. It catches no exception. Each of its methods only compares a string, returns a pointer, or forwards a call to the default loader. It consists of the two classes `LinkedLibrary` and `Loader` and the exported function of SGL-11.

## 6. The Exported Function and Its Installation

SGL-11. The loader shim MUST export exactly one function, `extern "C" ISlangSharedLibraryLoader* slang_glslang_sys_loader(void)`, which returns the static loader of SGL-10 and does nothing else. The crate MUST declare it in Rust as `pub safe fn slang_glslang_sys_loader() -> *mut slang_compiler_sys::ISlangSharedLibraryLoader` in an `unsafe extern "C"` block, and MUST NOT declare any other foreign item.

One function that returns an object and calls nothing keeps the shim out of every call into Slang. The installation then is an ordinary call of a generated thunk, which the frozen abort guard of `slang-compiler` and its checker already cover, so neither changes.

The declaration uses the interface type of the generated bindings, so a caller passes the result to `setSharedLibraryLoader` without a cast. The ABI is `"C"`, because the function cannot throw, and a non-unwinding ABI asserts that.

The function is marked `safe`, because it takes no argument and has no precondition: the shim only returns the address of a static object (`native/loader.cpp`), so every call is sound, as FFI-3 and UNS-3 of the coding standard require for a `safe fn`. A call therefore needs no `unsafe` block and no safety comment. The condition on the pointer, that it is passed only to `setSharedLibraryLoader` of the vendored Slang, is a condition of that later call, which is unsafe under the contract of `slang-compiler-sys`. Calling the function is not a call into Slang, so `slang-compiler` calls it outside its guard `abort_on_exception`.

SGL-12. `slang-compiler`, with its feature `vendored`, MUST install the loader on every global session that it creates, through the generated thunk of `IGlobalSession::setSharedLibraryLoader`, inside its guard `abort_on_exception` like every other call into Slang, before it returns the global session to its caller. `docs/design/slang-compiler.md` specifies where (SLC-34). A global session that compiles before the installation has already reported the missing module and cached the failure for its life (Section 3.10 of the sys design). The installation cannot fail, because the method returns nothing (Section 3).

A user of `slang-compiler-sys` alone, with its feature `vendored`, gets SPIR-V that is not optimized unless the user depends on this crate and installs the loader the same way. The crate documentation of both crates states this.

This illustration shows the installation by a direct user of the raw bindings:

```rust
let loader = slang_glslang_sys::slang_glslang_sys_loader();
// SAFETY: `global_session` is a live global session of the vendored Slang
// that this program links, only this thread uses it, and the loader is a
// static object.
unsafe {
    slang_compiler_sys::IGlobalSession::setSharedLibraryLoader(global_session, loader);
}
```

## 7. Unsafe Code

SGL-13. The crate root MUST carry the opt-out `#![expect(unsafe_code, reason = "...")]` that Section 4.4 of the coding standard defines, whose reason says that the crate declares the foreign function that returns the loader. The `unsafe extern "C"` block is the only unsafe code of the crate.

The `unsafe_code` lint reports an `unsafe extern` block, so the expectation of the crate root is fulfilled by that block alone. This crate therefore joins `slang-compiler-sys` and `slang-compiler` in the list of crates that may hold unsafe code (Section 3.3 of the architecture design). The opt-out is a crate-level expectation, so a lint that does not fire fails the check and shows that the exception is no longer needed.

The declaration has no `# Safety` section, because the function is safe. The body of its documentation says that the returned pointer is never null and stays valid for the life of the process, and that a caller passes it only to `setSharedLibraryLoader` of a global session of the vendored Slang that the program links, a condition that the caller of that unsafe call upholds. A Slang that another copy of the library loaded has another default loader, so the shim would answer it from the wrong Slang.

The integration-test binary carries its own `unsafe_code` expectation on the module `ffi`, whose tests call the loader and the libraries that it returns through the raw bindings.

## 8. Build Requirements

A build of this crate needs, at build time:

- git, CMake 3.22 or later, Ninja, a C++20 compiler, and Python 3 on `PATH`, because the grammar table generator of SPIRV-Tools is a Python script. CMake starts Python, under the name `python3` or `python`, and the build script does not.
- On Windows, an installation of Visual Studio or its Build Tools with the C++ tools. The build script finds `cl.exe` and its environment with the build dependency `find-msvc-tools` and gives that environment to its CMake runs, as SCS-68 of `docs/design/slang-compiler-sys.md` specifies for `slang-compiler-sys`, so the build needs no developer command prompt. A build from an ordinary shell has no developer environment, in which CMake's Ninja generator finds no compiler, and a developer environment exported to the whole build changes how rustc finds its linker.
- The source tree of `slang-src`, which needs network access for the first build in a target directory.

At run time, the program needs no file of slang-glslang. The crate documentation states these requirements.

## 9. Tests

The crate has one integration-test binary, `tests/glslang/`, with 33 tests in these modules:

- `build_script`: 23 pure tests of `build_logic.rs` against a fake host. They cover these parts:
  - The published test values of FNV-1a, and the `files` input over both native files and their names.
  - The arguments of each CMake run, the environment with `PYTHONDONTWRITEBYTECODE`, the probes of Ninja and the compiler, and the forms of `CXX` that the probe reads.
  - The link instructions and their order for Linux, macOS, and the Microsoft toolchain, with the runtime library option and the environment of the found compiler.
  - Every failure of the build script with its message, including a program that cannot start, a probe that fails, a failed step with the tail of its log, a signal, a log that cannot be opened or read, a library that the build did not produce, and each operation on the shared folder that fails.
  - The refusal of cross-compilation and of a target without a link rule, and no refusal and no warning for an untested target that has one.
  - The key on every variable that CMake reads, the use and the replacement of an entry, its storage with the stamp file, and the removal of only the numbered temporary directories of its key.
- `build_script::ffi`: 6 tests of `system.rs`, which read variables and files, run programs and append their output to a log, list, copy, write, stamp, rename, and remove files, hold a lock until it is dropped, and find the Microsoft compiler as `find-msvc-tools` does. They start processes and write files, so they sit in a module named `ffi`, which Miri skips.
- `ffi`: 4 tests of the loader that the crate links:
  - It loads `slang-glslang-<numeric version>`, also with the prefix `lib`, a directory with `/`, a directory with `\`, mixed separators, and another version, gets the same static library each time, and finds each of the nine functions and no unknown name.
  - It passes every other name to the default loader. It loads a library that the platform always has, `libc.so.6` with `malloc` on Linux, `System` with `malloc` on macOS, and `kernel32` with `GetProcAddress` on Windows, and finds that function but not a function of slang-glslang. Loading a missing name fails, also when a directory whose name starts with `slang-glslang` precedes it with either separator.
  - The loader answers `queryInterface` only for its own interfaces (SGL-10), and starts the out-pointer from a dangling value that is not null, so a missing write shows.
  - The library answers `queryInterface` and `castAs` only for its own interfaces, in the same way.

No recipe runs Miri on the crate. Its unsafe code is the one foreign declaration of SGL-11, which only the tests in `ffi` execute, so the pure tests of `build_script` give Miri no unsafe code to check (TEST-12 of the coding standard).

The tests that show the effect, that a vendored global session with the loader installed emits optimized SPIR-V without a diagnostic and one without it does not, belong to `slang-compiler`, which installs the loader (Section 13 of `docs/design/slang-compiler.md`). `test-vendored` runs them with the tests of this crate, and the vendored CI job runs that recipe on Linux, Windows, and macOS (Section 7.4 of `docs/design/tooling.md`).

## 10. Crate Documentation

SGL-14. The crate-level documentation MUST state what the crate builds and why, the requirements of Section 8, the five libraries, the C++ standard library, and the system libraries that it links, that `slang-compiler` with its feature `vendored` installs the loader, how a direct user of `slang-compiler-sys` installs it, the condition on the pointer of SGL-13, and the risk of Section 11 for a program that links another copy of glslang or SPIRV-Tools. It MUST also state the shared folder as SRC-20 of the `slang-src` design lists it, and name no environment variable of `slang-src`, because none exists. It MUST contain a `# Features` section that says the crate has no Cargo features and a `# Platform support` section with the rule of SGL-5. A user who links Slang statically without the loader gets SPIR-V that is not optimized and no build error, so the documentation is the only place that tells the user.

## 11. Security Considerations

The threat model has four parts:

1. The source tree comes from `slang-src`, which fetches it by commit hash (Section 14 of the `slang-src` design), and the wrapper build runs its CMake scripts and its Python script with the user's permissions.
2. Slang calls the loader for every library that a global session loads, including the name of a downstream compiler that a shader's options select.
3. The program that links this crate can link another copy of glslang or SPIRV-Tools, such as through a crate that builds shaderc.
4. An attacker who can place a library on the loader path of the program, or in a directory that Slang searches, can have the default loader load it.

Unsafe code and foreign function interface (FFI) invariants. The crate's only unsafe code is the declaration of SGL-11. The shim guarantees a non-null pointer to a static loader whose methods have no preconditions beyond those of `ISlangSharedLibraryLoader`, and it relies on being linked with the Slang of the same source tree, which the dependency of SGL-1 ensures. The shim holds no mutable state (SGL-10), so concurrent global sessions cannot race on it.

Untrusted shader source and file paths. The shim answers the name of slang-glslang from linked code and so removes one library that the default loader would otherwise search for on the file system. Every other name goes to the default loader unchanged, so the shim neither adds nor removes any other path that Slang can load from.

Build scripts that run native builds. The build script runs CMake, Ninja, and the compiler, and CMake runs the Python script of SPIRV-Tools on the source tree. The build script writes only under `OUT_DIR` and its entries in the shared folder of the Cargo target directory (SGL-6), and writes nothing into the tree (SGL-2). The wrapper project turns off every component that downloads, and the configure step disconnects `FetchContent` (SGL-3).

Dependencies. The crate depends on `slang-src` with its exact version and on `slang-compiler-sys`, and on `find-msvc-tools` as a build dependency and a development dependency. glslang and SPIRV-Tools come from the gitlinks of the recorded commit, so the commit hash fixes their content.

These attacks are out of scope:

- A source tree whose content is malicious, which the user chose by patching in a fork of `slang-src`.
- An attacker with write access to the target directory, who can place any library in an entry, as Section 14 of the `slang-src` design states.
- A library that an attacker places where the default loader finds it, which is the same exposure that a program without this crate has.
- Attacks on the repository itself, which are a matter of access control.

The residual risk has four parts:

1. A program that links another static copy of glslang or SPIRV-Tools can fail to link with duplicate symbols, or, when the linker accepts both, can call code of the other version through a symbol that both define (Section 13, SGLA-7).
2. The shim depends on an internal name of Slang (SGL-9). A change of its behaviour that keeps the name compiles and is detected only by the tests of Section 9.
3. The glue code and the optimizer run inside the program's process, so a defect in them, reached through a shader, affects the program as a defect of Slang does.
4. With the Microsoft toolchain, an executable that links this crate exports the nine functions of the glue code (Section 3), so another module of the process can find them by name (SGLQ-2).

## 12. Operational Considerations

### 12.1. Cargo Features

The crate has no Cargo features. `slang-compiler` enables it through its own feature `vendored` (Section 19.1 of the architecture design).

### 12.2. Environment Variables

This table lists every environment variable that the build script reads or sets, with its default. The variables that `slang-src` exports are described in Section 4 of the `slang-src` design.

| Variable | Read by | Default and behaviour when absent |
| --- | --- | --- |
| `DEP_SLANG_SRC_ROOT`, `DEP_SLANG_SRC_COMMIT`, `DEP_SLANG_SRC_BUILD_TAG`, `DEP_SLANG_SRC_SHARED_DIR` | The build script | Set by Cargo from the metadata of `slang-src`. Absence stops the build, with a message that the crate has to depend on `slang-src` directly. The last names the shared folder, in which the build script keeps its entries (SGL-6). |
| `OUT_DIR`, `CARGO_MANIFEST_DIR`, `TARGET`, `HOST` | The build script | Set by Cargo. Absence, or an `OUT_DIR` or `CARGO_MANIFEST_DIR` that is not an absolute path in UTF-8 without line breaks, stops the build. |
| `CARGO_CFG_TARGET_OS`, `CARGO_CFG_TARGET_ENV`, `CARGO_CFG_TARGET_FEATURE` | The build script | Set by Cargo for the target. They select the link rule of SGL-4 and SGL-5, and with the Microsoft toolchain `crt-static` among the target features selects the static runtime library. |
| `NUM_JOBS` | The build script | Set by Cargo. A whole number becomes `--parallel` of `cmake --build`. |
| `PYTHONDONTWRITEBYTECODE` | Set to `1` by the build script for every CMake command | The value of the environment is never used (SGL-2). |
| `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `CMAKE_TOOLCHAIN_FILE`, `MACOSX_DEPLOYMENT_TARGET` | CMake and the entry key. The build script also reads `CXX` for the compiler probe outside the Microsoft toolchain | Passed through unchanged. The build script reruns when one changes (SGL-7). |
| `VCINSTALLDIR`, `VCToolsVersion` | `find-msvc-tools`, in the lookup of `cl.exe` with the Microsoft toolchain, and, for `VCToolsVersion`, the entry key | As for `slang-compiler-sys` (Section 18.2 of the sys design). The build script reruns when one changes (SGL-7). |
| `PATH` | The operating system, when the build script starts `cmake`, `ninja`, and the compiler probe, and CMake, when it starts Python | A program that the build script cannot start stops the build with a message that names it. A missing Python fails the configure or build step, whose error holds the tail of the log. With the Microsoft toolchain, the CMake runs get the `PATH` of the lookup of `cl.exe` before the inherited value. |

### 12.3. Platforms and Minimum Supported Rust Version

The crate links on every target for which a link rule exists, which is Linux with the GNU C library, macOS, and Windows with the Microsoft toolchain, without cross-compilation (SGL-5). It is tested on the targets of the vendored CI jobs, and every other target on which it links is untested and unsupported (Section 6 of the sys design). It uses the workspace's `rust-version`.

### 12.4. Failure Modes

This table lists the failure modes of the crate, how each is reported, and the recovery.

| Failure | Reported by | Recovery |
| --- | --- | --- |
| `cmake`, `ninja`, or the program of the compiler probe cannot start | The error of the build script, with the program, the command, the reason, and that the build needs CMake 3.22 or later, Ninja, and Python 3 on `PATH`, and a C++20 compiler | Install it, or build without `vendored` |
| With the Microsoft toolchain, no `cl.exe` is found | The error of the build script, with where it looked and the values of `VCINSTALLDIR` and `VCToolsVersion` | Install Visual Studio or its Build Tools with the workload for desktop development with C++, or build without `vendored` |
| Python 3 is missing, or the configure or build step fails for another reason | The error of the build script, with the command, the log path, and its last 40 lines | Read the log and install Python 3 or fix the toolchain that it names |
| A library of SGL-4 is missing after the build | The error of the build script, with the path | Check the source tree. Another layout of glslang needs a change of the wrapper project and of this design |
| The target differs from the host, or has no link rule for its C++ runtime library | The error of the build script, with the triples and the remedies (SGL-5) | Build on a host of the target, or build without `vendored` |
| An entry of the shared folder cannot be inspected, listed, locked, written, renamed, or removed, or the time of its stamp file cannot be set | The error of the build script, with the action, the path, and the reason (SGL-6) | Free disk space or fix the permissions of the target directory. Removing the entry when no build runs is safe |
| The internal name of the default loader changed in the source tree | The compiler, while it builds the shim | Change the shim and this design for the new Slang, or use the recorded commit |
| Duplicate symbols with another copy of glslang or SPIRV-Tools | The linker of the user's program | Remove the other copy, or build without `vendored`, where Slang loads slang-glslang as a separate module |
| With the Microsoft toolchain, an unresolved external symbol of the Windows API in the link of a program | The linker of the user's program | The system libraries of SGL-4 miss a library. An agent adds it to the constant of `build_logic.rs`, or of `route.rs` of `slang-compiler-sys` when an object of Slang needs it (SA-18 of the sys design) |
| The loader is not installed on a global session of a vendored Slang | Slang, in the diagnostics of the first generation of SPIR-V, while the call succeeds (Section 3.10 of the sys design) | Install the loader as Section 6 states |

### 12.5. Logs

The build script emits no warnings. It writes the output of every CMake step to `OUT_DIR/build.log` and shows its last 40 lines only in the error of a failed step. Cargo shows the output of a build script only when it fails, so the log of the CI job holds no output of a successful wrapper build. The shim writes nothing at run time.

## 13. Assumptions to Confirm

Each row is a fact that this design relies on and that is not verified. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SGLA-1 | `IGlobalSession::getSharedLibraryLoader` returns null while the default loader is installed, and `Slang::DefaultSharedLibraryLoader::getSingleton()` returns the default loader, in the recorded Slang | SGL-9 | A test that asks a global session for its loader before the installation. The `ffi` test of Section 9 shows only that the shim reaches a working default loader |
| SGLA-2 | The wrapper build writes nothing into the source tree on Windows and macOS when `PYTHONDONTWRITEBYTECODE` is `1`, as it writes nothing on Linux (SGL-2) | SGL-2 | The vendored CI job, by a `git status --ignored` of the tree after the build, on Windows and macOS |
| SGLA-3 | The compile flags of glslang and SPIRV-Tools in the wrapper build equal those of Slang's own build of slang-glslang, apart from flags that change no generated code | SGL-3 | A comparison of the compile commands of both builds on Linux, Windows, and macOS |
| SGLA-4 | Slang requests downstream compilers other than slang-glslang, such as DXC, through the loader of the global session, so the shim passes them to the default loader | SGL-8 | A vendored test that requests such a compiler on a platform where it is installed |
| SGLA-7 | A program that links another static copy of glslang or SPIRV-Tools next to this crate fails to link or calls the wrong copy | Section 11 | A test program that also links a crate that builds shaderc |

The labels SGLA-5 and SGLA-6 belonged to assumptions that are confirmed or removed. The next assumption takes SGLA-8.

## 14. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row.

Q-32 of the architecture design, whether the loader gives a vendored build optimized SPIR-V on Linux, Windows, and macOS, is answered. The tests of Section 9 and the two tests of `ffi::vendored` of `slang-compiler` pass under `test-vendored` on the hosted runners of all three platforms, so the loader links, answers its requests, and makes Slang optimize SPIR-V there. They do not cover a deeper target directory or another installation of Visual Studio on Windows (SRCQ-1 of the `slang-src` design, SA-17 of the sys design).

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SGLQ-2 | Should the wrapper project keep the Microsoft compiler from marking the nine functions of the glue code `__declspec(dllexport)`, so that an executable that links this crate exports nothing and `link.exe` writes no import library and export file next to it (Section 3)? | This document | M7 |

SGLQ-1, whether the archive of the glue code needs the export list of the module on macOS, is answered: it does not, because the shim takes the addresses from its own table (Section 5). The next question takes SGLQ-3.

## 15. References

### 15.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| SGL-1 | 4 | `links = "slang-glslang"`, the exact version of `slang-src`, the vendored `slang-compiler-sys`, and no features. |
| SGL-2 | 4 | The wrapper project builds out of tree in `OUT_DIR`, reads the source tree in place, never writes into it, and runs with `PYTHONDONTWRITEBYTECODE=1`. |
| SGL-3 | 4 | Slang's options for glslang and SPIRV-Tools, static PIC libraries, no tests, executables, installation, or mimalloc, and C++20 with the Windows definitions. |
| SGL-4 | 4 | The five libraries exist after the build and are linked in order from the entry, followed by the C++ standard library and the system libraries of the target. |
| SGL-5 | 4 | A stop only for cross-compilation and for a target without a link rule for its C++ runtime library, never for an untested target. |
| SGL-6 | 4 | The libraries live in an entry of the shared folder, with the entry protocol and key of `slang-src`. |
| SGL-7 | 4 | Rerun instructions for the variables that CMake and the lookup of `cl.exe` read and for the stamp file of the used entry. |
| SGL-8 | 5 | The shim answers names that start with `slang-glslang` with the nine linked functions and passes every other name to the default loader. |
| SGL-9 | 5 | The shim reaches the default loader through Slang's internal singleton, compiled against the same tree. |
| SGL-10 | 5 | The loader and library are static, stateless objects whose reference counting frees nothing and whose queries answer only their own interfaces. |
| SGL-11 | 6 | One exported `extern "C"` function returns the loader, declared in Rust with the generated interface type. |
| SGL-12 | 6 | `slang-compiler` with `vendored` installs the loader on every global session through the guarded thunk before returning it. |
| SGL-13 | 7 | The crate-level `unsafe_code` expectation covers the one `unsafe extern "C"` block, whose one function is a `safe fn` that states its condition on the pointer in its body. |
| SGL-14 | 10 | The crate documentation states the build, the requirements, the linked libraries, the installation, the safety contract, and the risk of another glslang. |
