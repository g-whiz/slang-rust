# `slang-rhi-sys` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-18 |
| Applies to | `crates/slang-rhi-sys` |

## Abstract

This document specifies `slang-rhi-sys`, the crate of raw Rust bindings to slang-rhi, Slang's render hardware interface, and the build script that builds slang-rhi as static libraries from the Slang source tree that `slang-src` obtains. The crate always links the statically built Slang of the vendored `slang-compiler-sys`, so that Slang and slang-rhi come from one Slang commit. The document defines how the build script builds slang-rhi without network access, what it links, how `slang-bindgen` generates the bindings from the slang-rhi headers with a decision for each construct that `slang.h` does not use, the safety contract of the bindings, the platforms, the tests, and the failure modes. In milestone M9 the crate builds on Linux with the Vulkan backend, and the builds for Windows and macOS wait for a test whose result the owner decides on.

## 1. Introduction

### 1.1. Problem

slang-rhi is a C++ library in the Slang source tree that gives one API over Vulkan, Direct3D 12 (D3D12), Metal, CUDA, the CPU, and WebGPU. Its API consists of COM-style interfaces like those of `slang.h`, with C++ constructs that `slang.h` does not use, such as overloaded virtual methods, classes with virtual methods and no UUID, and records with constructors.

slang-rhi publishes no binaries, and its CMake build downloads a prebuilt Slang and other dependencies when it configures. A build script that ran that build unchanged would reach the network, link a second Slang, and break ARCH-35 of the architecture design.

### 1.2. Purpose

`slang-rhi-sys` builds slang-rhi from the tree of `slang-src` without network access, links it with the Slang of the vendored `slang-compiler-sys`, and exposes the raw bindings that `slang-bindgen` generates from its headers in every build. The safe crate `slang-rhi` builds on it (`docs/design/slang-rhi.md`).

This document implements the part of Section 15.12 of the architecture design that concerns `slang-rhi-sys`. It answers Q-39 of the architecture design and the part of Q-35 that concerns Linux, by the owner's decisions of Section 4.1, and confirms or corrects A-15, A-16, and A-19 to A-21 of that design (Section 3).

### 1.3. Audience

The primary readers are the coding agents that implement the crate, its wrapper build, and the rule set of its bindings. The secondary readers are the reviewers of those changes and the owner. A reader is expected to know Rust, Cargo build scripts, CMake, and C++, but not slang-rhi.

### 1.4. Scope

This document covers the crate and its dependencies, the wrapper build and its options, the libraries that the crate links, the entry of the shared folder, the generation of the bindings with the decision for each kind of declaration, the safety contract, the platforms, the build script, its tests, the crate documentation, and the security and operational considerations.

### 1.5. Non-Goals

This document does not cover:

- The mechanisms of the generator, which Section 7.8 of `docs/design/slang-compiler-sys.md` specifies. This document states the decisions that the rule set of slang-rhi takes with them.
- The safe API, which `docs/design/slang-rhi.md` specifies.
- The builds for Windows with D3D12 and for macOS with Metal, which wait for the test of Section 7.2 and the owner's decision on its result.
- The CUDA, OptiX, and CPU backends, which milestone M15 adds as Cargo features (Section 15.18 of the architecture design).
- The WebGPU backend, whose build downloads a prebuilt library (Section 3.5).
- The header-only helpers of slang-rhi, such as `ShaderCursor` in `include/slang-rhi/shader-cursor.h`, which are inline C++ and not part of the ABI.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that these documents define:

- Section 2.2 of `docs/writing-documentation.md`, such as agent, role, and user.
- Section 2.2 of `docs/rust-coding-standards.md`, such as binding generator, interface, interface pointer, native library, thunk, vtable, and `-sys` crate.
- Section 2.1 of `docs/design/architecture.md`, such as adapter, application binary interface (ABI), CMake, device, global session, lavapipe, slang-rhi, vendored build, and vendored route.
- Section 2.1 of `docs/design/slang-compiler-sys.md`, such as aggregate, compiler probe, default member initializer, inclusion rule, shim header, and translation unit.
- Section 2.1 of `docs/design/slang-src.md`, such as dependent build script, entry, entry key, required submodule, shared folder, source tree, and stamp file.
- Section 2.1 of `docs/design/slang-glslang-sys.md`, such as position-independent code (PIC) and wrapper project.

This document calls those files the writing guide, the coding standard, the architecture design, the sys design, the `slang-src` design, and the `slang-glslang-sys` design. It adds these terms:

- ABI probe: The C++ translation unit that the generator writes for the slang-rhi headers, which asserts the layout of every generated struct and reports the vtable slot of every generated method (SCS-78 of the sys design).
- backend: One graphics or compute API that slang-rhi can open a device on, such as Vulkan or D3D12. slang-rhi calls it a device type.
- FetchContent: The CMake module that downloads a dependency when a project configures. slang-rhi calls it through its macro `FetchPackage`.
- Itanium C++ ABI: The C++ ABI that GCC and Clang use on Linux and macOS, which fixes the layout of vtables.
- Microsoft ABI: The C++ ABI of the Microsoft compiler on Windows.
- placeholder slot: A field of a generated vtable struct that keeps the position of a method that Rust cannot call (SCS-75 of the sys design).
- slang-rhi tree: The directory `external/slang-rhi` of the source tree. Paths in this document without a leading `external/` or `/` are paths in it.
- Xlib: The C client library of the X Window System. Its development headers provide `X11/Xlib.h`.

## 3. Facts

This section is non-normative. It states what the slang-rhi tree of the recorded commit contains, with the file and line of each fact. Section 13 records the facts that only a build or a run can confirm.

### 3.1. The Library and Its Version

slang-rhi is the submodule `external/slang-rhi` of the Slang tree (Slang's `.gitmodules:41-43`), whose gitlink in the recorded commit is `d6d31411a3ab5417945daa6b1d8811cfc340cb66`. Its CMake project declares no version (`CMakeLists.txt:6`), and its README says that it is not ready for general use (`README.md:10-11`). The submodule holds no tag: in a clone of it that is not shallow, `git describe --tags` failed with "No names found" and `git tag` listed nothing. The recorded Slang commit therefore determines which slang-rhi a build gets. This confirms A-19 of the architecture design.

The Vulkan SDK ships no slang-rhi, so every build that uses it builds it from source. Its CMake build needs CMake 3.20 (`CMakeLists.txt:1`) and compiles C++20 (`CMakeLists.txt:1030`), which confirms the build part of A-15. Whether slang-rhi publishes releases elsewhere cannot be read from the tree. The tree itself downloads only a prebuilt Slang from Slang's releases (Section 3.5).

slang-rhi is under `Apache-2.0 WITH LLVM-exception` (`LICENSE:1`), the licence of Slang. The `slang-src` design lists `external/slang-rhi` as a required submodule (Section 3.4 of the `slang-src` design).

### 3.2. The Public Headers

`include/slang-rhi.h` includes `slang-rhi-device.h`, `slang-rhi-config.h`, `slang-com-ptr.h`, `slang.h`, and `slang-rhi/capabilities.h` (`include/slang-rhi.h:3-9`). `slang-com-ptr.h` brings in `<algorithm>`, `<atomic>`, and `<iterator>` through `slang-com-helper.h:9-12`, and `slang-rhi-device.h:6-7` includes `<cstdint>` and `<cstddef>`. The other headers of `include/slang-rhi/` are not included.

`slang-rhi-config.h` comes from the template `include/slang-rhi-config.h.in`, which the configure step writes to `include/slang-rhi-config.h` in the binary directory of the project (`CMakeLists.txt:704-706`). The template defines `SLANG_RHI_SHARED` and one `SLANG_RHI_ENABLE_*` macro for each backend. The only conditionals of `slang-rhi.h` are the export macro under `SLANG_RHI_SHARED` (`include/slang-rhi.h:11-23`) and the layout of `WindowHandle` under `SLANG_WASM` (`:2142-2205`), so the declarations do not depend on which backends a build enables.

The library exports two C functions, `rhiGetInstance`, which returns the process-wide `IRHI`, and `rhiDestroyInstance` (`include/slang-rhi.h:4101`, `:4106`). Every other entry point is a virtual method.

The interfaces derive from `ISlangUnknown` with single inheritance and carry a UUID through `SLANG_COM_INTERFACE`, as those of `slang.h` do, such as `IShaderObject` (`:1741`), `IComputePassEncoder` (`:2523`), `ICommandEncoder` (`:2600`), `ICommandQueue` (`:2855`), and `IDevice` (`:3374`).

Three classes have pure virtual methods and no base, no UUID, and no virtual destructor:

| Class | Line | Methods | Implemented by |
| --- | --- | --- | --- |
| `IAdapter` | 3117 | `getInfo` | slang-rhi |
| `IDebugCallback` | 3212 | `handleMessage` | The caller, which here is Rust |
| `IRHI` | 3984 | 15 methods from `enableDebugLayers` to `initTaskPool`, after one non-virtual inline method | slang-rhi |

The headers use these constructs, which `slang.h` does not use:

- Virtual methods that share a name: `IDevice::hasFeature` (`:3384-3385`), `hasCapability` (`:3392-3393`), `readTexture` (`:3693`, `:3702`), and `readBuffer` (`:3704`, `:3706`), `IQueryPool::reset` (`:2308`, `:2313`), and `bindPipeline` of the three pass encoders (`:2504-2505`, `:2528-2529`, `:2540-2544`).
- Non-virtual inline methods with the name of a virtual method, such as `IDevice::createBuffer` (`:3456`, `:3462`) and `ICommandEncoder::finish` (`:2769-2783`). They are not part of a vtable.
- Virtual methods that take or return an aggregate by value, such as `IRHI::setDebugLayerOptions` and `getDebugLayerOptions` (`:3998`, `:4001`), `IComputePassEncoder::dispatchComputeIndirect` (`:2532`), and `ICommandEncoder::uploadTextureData` (`:2659`). None of them is on the path of a compute dispatch.
- Bit-fields in `FormatInfo` (`:462-471`), in `AccelerationStructureInstanceDescGeneric` (`:1266-1272`), and in the instance descriptions of `slang-rhi-device.h:51-82`.
- Records with constructors, operators, or static member functions, such as `Binding` (`:1702`), whose members are a `BindingType`, two `Slang::ComPtr<IResource>`, and an anonymous union that holds a `BufferRange`, and `NativeHandle` (`:618`) and `ShaderOffset` (`:1655`).
- Default member initializers with values of scoped enumerations, such as `StructType structType = StructType::DeviceDesc` in `DeviceDesc` (`:3293`), with expressions such as `16 * 1024 * 1024` and `uint32_t(-1)`, with `= {}` on fields of struct types, such as `SlangDesc slang = {}`, and with a nested list in braces in `ColorClearValue color = {{0, 0, 0, 0}}` (`:788-790`).
- Constants of struct types, `kEntireBuffer` (`:719`), `kEntireTexture` (`:884`), and `kAllSubresources` (`:1005`), next to the integer constant `kTimeoutInfinite` (`:60`).

Every virtual method is declared `SLANG_NO_THROW` and `SLANG_MCALL`, and the C functions `SLANG_STDCALL`, which are macros of Slang's `include/slang.h`. `SLANG_MCALL` is `SLANG_STDCALL` (`slang.h:225-227`), which is `__stdcall` with the Microsoft compilers and empty elsewhere (`slang.h:218-224`), so these methods have the calling convention of the methods of Slang, which the bindings declare `extern "system"`. `SLANG_NO_THROW` names no calling convention. It is `__declspec(nothrow)` on Windows and empty elsewhere (`slang.h:205-213`).

### 3.3. Objects, Lifetime, and Threading

Reference counts are atomic (`src/core/com-object.h:22-70`), and an object frees itself when its count reaches zero. Every child of a device holds a strong reference to it (`src/device-child.h:24`, `src/reference.h:56-103`), so a child keeps its device alive.

`rhiGetInstance` creates the `IRHI` once under a mutex and never frees it on its own (`src/rhi.cpp:469-479`). `rhiDestroyInstance` frees it, but while a device lives it first fails an assertion that aborts the process in every build type (`src/rhi.cpp:138-144`, `src/core/assert.cpp:21-29`).

Three methods return a pointer that carries no reference:

| Method | Object | Valid until |
| --- | --- | --- |
| `IRHI::getAdapter` | An element of a vector of the backend, filled once (`src/vulkan/vk-backend.cpp:32-35`, `src/backend.h:24-32`) | `rhiDestroyInstance` |
| `ICommandEncoder::beginComputePass` | A member of the command encoder whose `addRef` and `release` do nothing (`src/command-buffer.cpp:468-474`, `src/command-buffer.h:110-111`, `:204`) | The encoder is freed. After `end`, its methods do nothing (`src/command-buffer.cpp:312-320`) |
| `IComputePassEncoder::bindPipeline(IComputePipeline*)` | The root shader object, which the pass encoder holds (`src/command-buffer.cpp:224-235`, `src/command-buffer.h:116`) | The next `bindPipeline` of the pass, or the encoder is freed |

The header states two threading rules: `IDebugCallback::handleMessage` may be called from several threads at once, and its message is valid only during the call (`include/slang-rhi.h:3215`, `docs/error-handling.md:63`), and the methods of `ITaskPool` may be called concurrently (`include/slang-rhi.h:3862-3869`). Neither the headers nor the two documents of `docs/` state whether a device, a queue, an encoder, or a shader object tolerates use from several threads.

The sources hold no `throw` and no `try` (a search of `src/` and `include/`). slang-rhi calls Slang, whose methods can let a C++ exception escape (KB-1 of `docs/design/slang-compiler.md`), and allocations of the standard library can throw.

`IDevice::createShaderProgram` with one global scope and no entry points uses the global scope as the linked program, and takes the Slang session from it (`src/shader.cpp:58-62`). A device creates its own Slang session from the global session of `SlangDesc::slangGlobalSession` (`include/slang-rhi.h:3224-3227`), or creates a global session of its own when that field is null.

### 3.4. Backends

The platform tables of `CMakeLists.txt:116-157` give Windows the backends CPU, D3D11, D3D12, Vulkan, CUDA, and WebGPU, Linux CPU, Vulkan, CUDA, and WebGPU, and macOS CPU, Vulkan, Metal, and WebGPU. Each `SLANG_RHI_ENABLE_*` option defaults to on where the platform has the backend (`CMakeLists.txt:174-183`), and `SLANG_RHI_ENABLE_AFTERMATH` defaults to off (`:184`).

A-16 of the architecture design is confirmed: the CUDA, OptiX, and CPU backends exist. slang-rhi loads the native API of each backend at run time, such as `libvulkan.so.1` with `dlopen` (`src/vulkan/vk-api.cpp:26-45`), so a build links no graphics library.

A-21 of the architecture design is corrected in part. Ray tracing pipelines exist on Vulkan, D3D12, and CUDA (`docs/api.md:35`, `:171`, `:234`). Ray queries exist on D3D12, Metal, and Vulkan, not only on Metal (`src/d3d12/d3d12-device.cpp:988`, `src/metal/metal-device.cpp:240`, `src/vulkan/vk-device.cpp:913-920`). Mesh shading exists on Vulkan and D3D12 (`docs/api.md:219`), and cooperative vectors on CUDA, D3D12, and Vulkan (`docs/api.md:47-49`, `:188`). The headers name no hull, domain, or geometry stage and no feature for them, and whether cooperative vectors on D3D12 need NVAPI is not settled. A-29 of the architecture design records both.

### 3.5. The CMake Build

The default configure step downloads, each with the macro `FetchPackage` over FetchContent (`cmake/FetchPackage.cmake:7-58`):

| Download | Condition | Line | Need of a Linux build with Vulkan only |
| --- | --- | --- | --- |
| A prebuilt Slang 2026.12.2 | `SLANG_RHI_FETCH_SLANG`, on by default | 197, 409 | Turned off. The wrapper supplies the Slang of the vendored build |
| The Vulkan headers | The Vulkan backend | 264-266, 632 | Redirected to `external/vulkan` of the source tree |
| DXC | Windows, D3D12, and `SLANG_RHI_FETCH_DXC` | 539 | None |
| WinPixEventRuntime | Windows and D3D12 | 553 | None |
| The Agility SDK | Windows and `SLANG_RHI_ENABLE_AGILITY_SDK` | 564 | None |
| Aftermath | `SLANG_RHI_ENABLE_AFTERMATH` | 580 | None |
| NVAPI | Windows x86_64 and `SLANG_RHI_ENABLE_NVAPI` | 598 | None |
| The OptiX headers | CUDA and OptiX | 613 | None |
| metal-cpp | Metal | 640 | None |
| Dawn | WebGPU | 648 | None. Its prebuilt library is a download, so the WebGPU backend is off |
| GLFW | Tests with GLFW or examples | 668-680 | None. Both are off outside a top-level build |

This confirms A-20 of the architecture design in kind and extends it by the prebuilt Slang, WinPixEventRuntime, the Vulkan headers, Aftermath, and GLFW.

The macro verifies a download only when its call passes a hash, which must have the form `SHA256=` with 64 hexadecimal digits, and downloads without verification otherwise (`cmake/FetchPackage.cmake:12-22`). Every call passes a hash variable (`CMakeLists.txt:409-648`), the default of the DXC hash is a SHA-256 hash (`:211`), and a user who overrides a URL may leave its hash empty (`:206`). Whether the default of every package on every platform carries a hash is not settled, and no requirement depends on it, because RSYS-4 turns every download off or redirects it to the tree.

The Vulkan headers that slang-rhi pins and those of `external/vulkan` are of the same version, 1.4.347 (`CMakeLists.txt:264-266`, Slang's `external/vulkan/include/vulkan/vulkan_core.h:69`). FetchContent uses the directory that `FETCHCONTENT_SOURCE_DIR_VULKAN_HEADERS` names in place of a download.

The Vulkan backend also needs a header that the source tree does not hold. `src/vulkan/vk-api.h:10-16` defines `VK_USE_PLATFORM_XLIB_KHR` on every Linux target other than Android, so Slang's `external/vulkan/include/vulkan/vulkan.h:58` includes `<X11/Xlib.h>`. A build of the Vulkan backend on Linux therefore needs the development headers of Xlib from the system, such as those of the Debian package `libx11-dev`, and a build with GCC without them stops with `fatal error: X11/Xlib.h: No such file or directory`.

With the headers of `external/vulkan` and those of Xlib, a build on Linux compiles slang-rhi under GCC 14.2.0 with no warning.

With `SLANG_RHI_BUILD_FROM_SLANG_REPO` on, slang-rhi expects a CMake target named `slang` from its parent project and defines none (`CMakeLists.txt:418-480`). It links that target publicly (`:483`) and adds `SLANG_RHI_SLANG_INCLUDE_DIR` to its include directories (`:530`). Without the option, it defines `slang` as an imported shared library, and a static Slang only for Emscripten (`:418-466`).

Slang's own build adds slang-rhi when `SLANG_ENABLE_SLANG_RHI` is on, its default (Slang's `CMakeLists.txt:136`, `external/CMakeLists.txt:403-440`). It then forces the tests of slang-rhi on (`external/CMakeLists.txt:405`) and leaves the CPU, CUDA, OptiX, and Vulkan options at their defaults. The vendored route of `slang-compiler-sys` turns the option off (Section 9.3 of the sys design).

The build produces the static libraries `slang-rhi` (`CMakeLists.txt:314`), `slang-rhi-vma` from the Vulkan Memory Allocator (VMA) when Vulkan is on (`:509-516`), and `slang-rhi-resources` from CMakeRC, which embeds two shader files and is built whatever the backends (`:678-686`). `slang-rhi-d3d12ma` exists only with D3D12 (`:487`). `slang-rhi` compiles as position-independent code (`:1031`).

The INTERFACE target `slang-rhi-warnings-as-errors` adds `-Werror`, or `/WX` with the Microsoft compiler, to the sources of `slang-rhi`, with no option to turn it off (`CMakeLists.txt:774-781`).

The compiled third-party code of a Linux build with Vulkan only is VMA, under the MIT licence (`external/vma/vk_mem_alloc.h:2-18`), and the runtime of CMakeRC, whose file `cmake/CMakeRC.cmake` carries no licence text. The Vulkan headers are compiled in only as declarations, under `Apache-2.0 OR MIT` (Slang's `external/vulkan/LICENSE.md`). stb, doctest, and renderdoc serve only the tests (`CMakeLists.txt:1034-1047`).

## 4. Crate and Dependencies

### 4.1. Owner Decisions

The owner decided on 2026-11-17:

1. `slang-rhi-sys` always uses the vendored Slang. It depends on `slang-compiler-sys` with the feature `vendored`, as `slang-glslang-sys` does, so that Slang and slang-rhi come from the one tree of `slang-src`. The feature `rhi` of `slango` in M10 therefore implies `vendored`. This answers Q-39 of the architecture design.
2. M9 delivers `slang-rhi-sys` on Linux with the Vulkan backend, built with no network access beyond the clone of `slang-src`. For Windows with D3D12 and macOS with Metal, a code step first tests whether slang-rhi builds with its optional downloads turned off, and brings the result to the owner before any download is designed (Section 7.2). This answers the Linux part of Q-35, whose rest RSQ-1 keeps open.
3. The raw bindings cover the whole public API of slang-rhi wherever the generator can produce them soundly, and the safe crate wraps only the compute path in M9 (`docs/design/slang-rhi.md`).

On 2026-11-18, the owner decided that the development headers of Xlib are a stated requirement of the build on Linux, such as those of the Debian package `libx11-dev`, and that the crate bundles no stub of `X11/Xlib.h` (Section 3.5, RSYS-17).

The earlier decisions of Section 15.12 of the architecture design hold: `slang-bindgen` is a build dependency of the crate, the Vulkan, D3D12, and Metal backends are built by default on their platforms, and CUDA with OptiX and the CPU are opt-in features of M15.

### 4.2. The Manifest

RSYS-1. The manifest of `slang-rhi-sys` MUST declare `links = "slang-rhi"` and MUST have no Cargo features. It MUST depend on `slang-src` with its exact version and on `slang-compiler-sys` with the feature `vendored`, both in the table `[target.'cfg(all(target_os = "linux", target_env = "gnu"))'.dependencies]`. It MUST have the build dependency `slang-bindgen` with its exact version from the change that makes the build script generate the bindings (RSYS-9), and MUST NOT depend on `find-msvc-tools` while RSYS-13 refuses every target of the Microsoft toolchain.

The table of the target keeps the dependencies out of every build that RSYS-13 refuses. A build for another target then compiles neither `slang-src` nor `slang-compiler-sys`, so it clones and builds nothing before the build script of this crate stops it.

`links` keeps two builds of slang-rhi out of one build graph (FFI-2). The exact version of `slang-src` keeps the tree of this crate and that of `slang-compiler-sys` one tree (SRC-14 of the `slang-src` design). The dependency on the vendored `slang-compiler-sys` places Slang's libraries after slang-rhi's on the link line, gives this build script the metadata of the Slang build (SCS-54 of the sys design), and starts this build only after the build of Slang, as SGL-1 of the `slang-glslang-sys` design explains for that crate.

The crate takes the version of the workspace and sets its own `license` field to `Apache-2.0 WITH LLVM-exception`, the licence of Slang and of slang-rhi (ARCH-36 of the architecture design, Section 3.1). Its package directory holds the symbolic link `LICENSE` to `LICENSE-APACHE-LLVM` (TOOL-33 of `docs/design/tooling.md`). The package holds no code of slang-rhi or of its third-party files, because the build reads them from the clone, so the licence of the crate covers the whole package.

The crate sets `publish = false` until the change that prepares the second release (TOOL-35 of `docs/design/tooling.md`, Section 13 of the architecture design). The workspace leaves it out of `default-members`, and every recipe that passes `--workspace` excludes it, as for `slang-glslang-sys` (TOOL-32), because its build clones and builds native code.

`find-msvc-tools` serves the lookup of the Microsoft compiler, which no target of M9 needs (Section 7). The build logic has no such lookup while RSYS-13 refuses Windows, because the lookup would be code that never runs and the dependency would be unused, which `check-unused-deps` and `CARGO_BUILD_WARNINGS=deny` reject. The change that designs the build for Windows (RSQ-1) adds both. Until then, `VCINSTALLDIR` and `VCToolsVersion` stay rerun variables (RSYS-8), and `VCToolsVersion` stays an input of the entry key, as SRC-17 of the `slang-src` design lists them.

`slang-bindgen` is unused until the build script runs the generator, for the same reason. The third code step of Section 15.12 of the architecture design adds it with the generation of RSYS-9.

### 4.3. The Crate Root

RSYS-2. `src/lib.rs` MUST hold, in this order and with no other code: the crate-level documentation (Section 10), the opt-out `#![expect(unsafe_code, reason = "...")]` of Section 4.4 of the coding standard, `use slang_compiler_sys as _;` and `use slang_src as _;`, each under `#[cfg(all(target_os = "linux", target_env = "gnu"))]`, the module `bindings` with the lint exceptions of Section 5.10 of the sys design and the body `include!(concat!(env!("OUT_DIR"), "/bindings.rs"));`, and `pub use bindings::*;`. The integration-test binary MUST declare the module `symbols` that includes `symbols.rs` from `OUT_DIR`.

The two `use` declarations name the dependencies whose `links` metadata the build script reads (DEP-2 of the coding standard), and carry the condition of the table of RSYS-1, because on another target the dependencies do not exist. The expectation, the module `bindings`, its re-export, and the module `symbols` arrive with the generated bindings (RSYS-9). Before them, the crate has no unsafe code, and an expectation that nothing fulfils fails the lint `unfulfilled_lint_expectations`.

The crate is the raw layer of FFI-1 for slang-rhi. A hand-written item would be a second record of the headers that diverges from them, which COM-9 and Hard Rule 7 of `AGENTS.md` forbid. The crate keeps no build information of its own, because the Slang of a build is the one that `BUILD_INFO` of `slang-compiler-sys` records and the slang-rhi of a build is the gitlink of that commit.

The generated declarations hold unsafe foreign items, so the crate joins `slang-compiler-sys`, `slang-compiler`, and `slang-glslang-sys` in the list of crates that may hold unsafe code (Section 3.3 of the architecture design).

## 5. The Wrapper Build

### 5.1. The Route

RSYS-3. The build script MUST build slang-rhi through the wrapper project `native/CMakeLists.txt` of the crate, with CMake and the Ninja generator, out of tree in `OUT_DIR/build`, reading the source tree in place from `DEP_SLANG_SRC_ROOT`. It MUST NOT copy the tree and MUST NOT write into it. The wrapper project MUST define an INTERFACE library named `slang` whose include directory is the directory that `DEP_SLANG_COMPILER_INCLUDE` names and which carries the definition `SLANG_STATIC`, and MUST then add `external/slang-rhi` of the tree with `add_subdirectory` and `EXCLUDE_FROM_ALL`.

Two routes exist. The other one turns on `SLANG_ENABLE_SLANG_RHI` in the configure step of Slang that the vendored route of `slang-compiler-sys` runs. This table compares them:

| Concern | Slang's configure step of `slang-compiler-sys` | Wrapper project of `slang-rhi-sys` |
| --- | --- | --- |
| Crate boundaries | `slang-compiler-sys` builds slang-rhi, so its entry and its configure options change for every vendored build, also one that never uses slang-rhi | Only a build that depends on `slang-rhi-sys` builds slang-rhi |
| Rebuild cost | A change of a slang-rhi option changes the entry key of Slang, so Slang builds again | A change rebuilds slang-rhi only, from its own entry |
| Options | Slang's hook forces the tests of slang-rhi on and leaves CPU, CUDA, and OptiX at their defaults (Section 3.5) | The wrapper sets every option (Section 5.2) |
| Entry of the shared folder | One entry holds both | An entry of its own, keyed by its own inputs (RSYS-7) |
| `SLANG_STATIC` on Windows | Not set for the sources of slang-rhi, so `SLANG_API` would be a DLL import (`slang.h:230-256`) | Carried by the wrapper's `slang` target |
| Headers of Slang | Slang's source include directory | The include directory of the vendored entry, which `slang-compiler-sys` exports and from which it generates its own bindings (SCS-54 of the sys design) |

The wrapper project is the sounder choice on every row, and it is the pattern of the `slang-glslang-sys` design (SGL-2). The INTERFACE library `slang` links nothing, because rustc links Slang's static libraries from `slang-compiler-sys`, and a static library needs no link of its own dependencies.

With `SLANG_RHI_BUILD_FROM_SLANG_REPO` on, slang-rhi takes the wrapper's `slang` target and, as `SLANG_RHI_SLANG_INCLUDE_DIR`, the include directory of the entry of `slang-compiler-sys`. The `prelude` directory that is missing next to that include directory does not matter to the Vulkan backend.

The wrapper build writes nothing into the tree, which `git status --short --ignored` shows after a build, in the source tree, in `external/slang-rhi`, and in `external/vulkan`.

A change of `native/CMakeLists.txt` makes Cargo run the build script of this crate again and not that of `slang-compiler-sys`. The build script then builds a new entry of slang-rhi beside the old one, while the shared folder keeps its one entry of Slang.

### 5.2. Configure Options

RSYS-4. The wrapper project MUST set the options of this table before it adds the subdirectory, and the configure step MUST pass `-DFETCHCONTENT_FULLY_DISCONNECTED=ON` and `-DFETCHCONTENT_SOURCE_DIR_VULKAN_HEADERS=<root>/external/vulkan`. With these, the configure step reaches no network, because every download of Section 3.5 is either turned off or redirected to the tree.

| Option | Value | Reason |
| --- | --- | --- |
| `CMAKE_POSITION_INDEPENDENT_CODE` | `ON` | PIC for a Rust executable, also for VMA and the resources |
| `BUILD_SHARED_LIBS` | `OFF` | Static libraries only |
| `SLANG_RHI_BUILD_FROM_SLANG_REPO` | `ON` | slang-rhi takes the wrapper's `slang` target and defines no imported Slang |
| `SLANG_RHI_BUILD_SHARED` | `OFF` | A static library |
| `SLANG_RHI_BUILD_TESTS`, `SLANG_RHI_BUILD_TESTS_WITH_GLFW`, `SLANG_RHI_BUILD_EXAMPLES` | `OFF` | No tests, examples, or GLFW download |
| `SLANG_RHI_INSTALL` | `OFF` | No installation |
| `SLANG_RHI_FETCH_SLANG` | `OFF` | No prebuilt Slang |
| `SLANG_RHI_FETCH_DXC` | `OFF` | No DXC download |
| `SLANG_RHI_SLANG_INCLUDE_DIR` | The value of `DEP_SLANG_COMPILER_INCLUDE` | The headers of the Slang that the build links |
| `SLANG_RHI_ENABLE_VULKAN` | `ON` | The backend of Linux |
| `SLANG_RHI_ENABLE_CPU`, `_D3D11`, `_D3D12`, `_AGILITY_SDK`, `_NVAPI`, `_METAL`, `_CUDA`, `_OPTIX`, `_WGPU`, `_AFTERMATH` | `OFF` | Not built in M9. CUDA, OptiX, and the CPU arrive as features in M15, and WebGPU needs a download |

The configure step is `cmake -S <native> -B <OUT_DIR>/build -G Ninja`, followed by `-DCMAKE_BUILD_TYPE=Release`, the two FetchContent options, `-DSLANG_TREE_DIR=<root>`, `-DSLANG_INCLUDE_DIR=<include>`, and `-DCMAKE_ARCHIVE_OUTPUT_DIRECTORY=<OUT_DIR>/lib`. The build step is `cmake --build <OUT_DIR>/build --target slang-rhi slang-rhi-sys-abi-probe`, followed by `--parallel` and the value of `NUM_JOBS` when it is a whole number. Until the build script writes the ABI probe (RSYS-9), the build step names the target `slang-rhi` alone. The build script runs every CMake command with standard input closed, removes `build/CMakeCache.txt` before every configure step, as SGL-2 does, and appends their output to `OUT_DIR/build.log`.

The options of the table are set as normal variables before `add_subdirectory`, which `option` and `cmake_dependent_option` in a subdirectory respect, as the wrapper of `slang-glslang-sys` does for SPIRV-Tools.

The configure and build steps need no network. With the clone and the build of Slang already in the shared folder, they succeed under `unshare -rn`, which gives a network namespace without any network interface, with `CARGO_NET_OFFLINE=true`, and every compile command includes `external/vulkan/include` of the tree.

The configured header defines `SLANG_RHI_SHARED` as 0 and `SLANG_RHI_ENABLE_VULKAN` as the only enabled backend. The only line of the log about a download is the status message `-- Fetching Vulkan headers ...`, which slang-rhi prints before it calls `FetchPackage`.

RSYS-17. Before it adds the subdirectory, the wrapper project MUST check with `check_include_file_cxx` of the CMake module `CheckIncludeFileCXX` that the C++ compiler finds `X11/Xlib.h`. When the compiler does not find it, the wrapper project MUST stop the configure step with `message(FATAL_ERROR ...)` and a message that names the header, says that the Vulkan backend of slang-rhi includes it on Linux, and names the Debian package `libx11-dev` as an example of the development headers of Xlib. The crate MUST NOT bundle a stub of the header (Section 4.1).

The check stops the build in the configure step with a message that names the cause, where the compiler would otherwise stop the build step with an error inside the Vulkan headers that names no package (Section 3.5). Its result lives only in `CMakeCache.txt` of the build directory, which the build script removes before every configure step, so a build after the installation of the headers checks again.

### 5.3. Warnings as Errors

RSYS-5. The wrapper project MUST clear the property `INTERFACE_COMPILE_OPTIONS` of the target `slang-rhi-warnings-as-errors` after it adds the subdirectory, so that a warning of the compiler does not fail the build.

slang-rhi turns every warning of its own sources into an error with no option to turn it off (Section 3.5). The CI of slang-rhi runs on runner images such as `ubuntu-latest` and sets up its compilers with no pinned version (`.github/workflows/ci.yml:43-63`, `:82-86`), so the tree records no compiler version that upstream tests.

A user's compiler can differ from all of them, and a warning that upstream never saw would stop a user's build in code that the user cannot change. The wrapper of `slang-glslang-sys` sets `SPIRV_WERROR` to `OFF` for the same reason, and the warnings stay in `OUT_DIR/build.log`.

In the build tree of a build on Linux, `build.ninja` holds no `-Werror`, and every compile command of `slang-rhi` carries `-Wall -Wextra -Wpedantic` without it, so clearing the property removes the flag from every compile command.

### 5.4. Libraries and Link Order

RSYS-6. The build script MUST check after the build that each library of this table exists as `lib<name>.a` in `OUT_DIR/lib`, and MUST emit a search path for the directory `lib` of its entry (RSYS-7) and these link instructions, in this order, followed by `stdc++` and then by the system libraries `dl` and `pthread`, each as a `dylib` link instruction:

| Library | Content |
| --- | --- |
| `slang-rhi` | slang-rhi with the Vulkan backend |
| `slang-rhi-vma` | VMA |
| `slang-rhi-resources` | The resources that CMakeRC embeds |

Each library precedes the libraries that it uses, which the static linker of Linux needs, and the dependency on `slang-compiler-sys` places Slang's libraries after these (RSYS-1). slang-rhi uses `dlopen` and starts the worker threads of its task pool with `std::thread` (`src/core/task-pool.cpp:135`, `:172`), which need `dl` and `pthread` on Linux, and it links no graphics library, because it loads `libvulkan.so.1` at run time (Section 3.4). The vendored route of `slang-compiler-sys` also links `dl` and `pthread` (SCS-69 of the sys design), and a repeated system library changes nothing.

The build places the three archives under `OUT_DIR/lib` with these names, and the link order resolves every symbol. Two programs outside the repository checked this:

- A Rust crate that depends on `slang-rhi-sys` by path, with its own declaration of `rhiGetInstance`, linked with the link instructions of this crate and no unresolved symbol, and called `getDeviceTypeName` for the Vulkan device type through the vtable of `IRHI`.
- A C++ program, linked with the three archives in this order, then Slang's six archives, `stdc++`, `dl`, and `pthread`, which created a Vulkan device, created a buffer with initial data and read it back, compiled a Slang compute shader through `IDevice::getSlangSession`, dispatched it, and read the expected results on an NVIDIA adapter and on lavapipe.

The repository holds no such link test, because RSYS-2 and Hard Rule 7 of `AGENTS.md` forbid a hand-written declaration, and the tests of Section 9 link the crate once the bindings exist.

The library `slang-rhi-sys-abi-probe`, which the wrapper project builds from the ABI probe (RSYS-9), is not linked into the crate. The build script emits `cargo::rustc-link-arg-tests` with the path of its archive, so that only the integration-test binary links it (Section 13, RSA-9). It emits the instruction from the change that adds the ABI probe.

### 5.5. The Entry of the Shared Folder

RSYS-7. The build script MUST keep the four libraries of RSYS-6 and RSYS-9, under `lib`, and the configured header `slang-rhi-config.h`, under `include`, in an entry of the directory `slang-rhi-sys` of the shared folder that `DEP_SLANG_SRC_SHARED_DIR` names, with the entry protocol of SRC-16 and the entry key of SRC-17 of the `slang-src` design, and MUST use a complete entry whose inputs equal those of the current build instead of building.

Until the build script writes the ABI probe (RSYS-9), an entry holds the three archives of RSYS-6 under `lib` and the configured header under `include`.

The inputs of the key follow SRC-17, with these values:

- `files` is the FNV-1a hash of the name and content of `native/CMakeLists.txt`, as SGL-6 hashes its native files.
- `configure` holds the lines `-DCMAKE_BUILD_TYPE=Release`, `-DFETCHCONTENT_FULLY_DISCONNECTED=ON`, and `-DFETCHCONTENT_SOURCE_DIR_VULKAN_HEADERS=external/vulkan`, with the path of the Vulkan headers relative to the root of the tree, so that the path of the tree stays out of the key.
- `compiler` is found as SCS-56 of the sys design states.

The ABI probe is a function of the headers, which the commit fixes, of the target, and of the version of `slang-bindgen`, which the exact requirement of RSYS-1 ties to the version of this crate in the input `crate`. The variables through which the compiler reads include and library directories are not inputs of the key, which Section 9.4 of the `slang-src` design states as a limit of every entry of libraries (SRCQ-4 of that design).

An entry is complete only when its file `inputs` equals the inputs of the current build byte for byte, and each of its archives and the configured header exist as files. The build script replaces an entry that fails either check.

### 5.6. Rerun Instructions

RSYS-8. The build script MUST emit `cargo::rerun-if-env-changed` for `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `CMAKE_TOOLCHAIN_FILE`, `MACOSX_DEPLOYMENT_TARGET`, `VCINSTALLDIR`, and `VCToolsVersion`. It MUST emit `cargo::rerun-if-changed` for the stamp file of the entry that it uses (SYS-1). The build logic compiles in `native/CMakeLists.txt` with `include_str!`, so a change of it recompiles and reruns the build script, as SGL-7 explains.

`LIBCLANG_PATH` steers only the generator, so the build script also emits its instruction from the change that makes it run the generator (RSYS-9).

## 6. The Bindings

### 6.1. Generation

The generation of this section arrives in the third code step of Section 15.12 of the architecture design. Sections 4.2, 4.3, 5.2, 5.4, 5.5, 5.6, and 9 state what the crate holds before it.

RSYS-9. On every run, after the configure step or the lookup of a complete entry, the build script MUST call `slang_bindgen::generate_slang_rhi` with `<root>/external/slang-rhi/include`, the `include` directory of the configure step or of the entry, the directory that `DEP_SLANG_COMPILER_INCLUDE` names, and `TARGET`, and MUST write `bindings.rs` and `symbols.rs` into `OUT_DIR`. When it builds, it MUST write the ABI probe into `OUT_DIR/build/abi-probe.cpp` before the build step, from which the wrapper project builds the static library `slang-rhi-sys-abi-probe`. A failure of the generator MUST stop the build with its error, before any link instruction.

The bindings then come from the headers and the configuration that the libraries of the same build were compiled with, as SCS-64 of the sys design requires for Slang. The configured header changes no declaration (Section 3.2), and reading it from the build keeps that true for a later slang-rhi that conditions a declaration on a backend.

The generated items that name Slang types are paths into `slang_compiler_sys` (SCS-71 of the sys design), so a pointer passes between the two crates without a cast.

### 6.2. Decisions of the Rule Set

RSYS-10. The rule set `SLANG_RHI` MUST decide each kind of declaration of the slang-rhi headers as this table states. A declaration that the table does not cover matches no rule, and the generator fails on it (SCS-2 of the sys design).

| Declarations | Decision | Reason | Consequence for the safe crate |
| --- | --- | --- | --- |
| Every declaration of the Slang headers | External (SCS-71) | One Rust type for each native type | None |
| The interfaces with a UUID | Included, with every slot | The API of slang-rhi | None |
| `IAdapter`, `IDebugCallback`, and `IRHI` | Included as interfaces without a UUID (SCS-72) | They have no destructor and no base, so their vtable is the list of their methods | No checked cast. Rust implements `IDebugCallback` |
| The overloads of Section 3.2 | Included with a distinct Rust name for each (SCS-73). The first declared overload keeps the C++ name, and a later one gets the suffix that the rule names: `hasFeature_name`, `hasCapability_name`, `readTexture_blob`, `readBuffer_blob`, `reset_range`, and `bindPipeline_root` | Itanium places them in declaration order, and the ABI probe checks it | None in M9. The Microsoft ABI waits for SA-19 of the sys design |
| The non-virtual methods of interfaces, the inline free functions such as `getRHI`, `destroyRHI`, and `getTextureDimension`, and the operators of `SLANG_RHI_ENUM_CLASS_OPERATORS` | Excluded (SCS-3, SCS-74) | Inline C++, not part of the ABI | The safe crate writes the logic of an inline helper that it needs |
| `Binding`, `NativeHandle`, `WindowHandle`, `AdapterLUID`, `ShaderOffset`, and the other records with members that are not data | Included with those members ignored, and the `Slang::ComPtr<T>` fields of `Binding` as `*mut T` (SCS-74) | Their data layout is plain, and the layout assertions and the ABI probe check it | The caller keeps its own reference to each object that a field names during the call (Section 6.4) |
| `AdapterList` and `BufferOffsetPair` | Excluded | Each holds a `Slang::ComPtr<T>` and is used only by inline helpers or by value | None in M9 |
| The virtual methods that take or return an aggregate by value | Placeholder slots (SCS-75) | The ABI of such a call differs between targets | The safe crate cannot call them. None is on the compute path (Section 3.2, RSQ-2) |
| `FormatInfo`, `AccelerationStructureInstanceDescGeneric`, and the instance descriptions of `slang-rhi-device.h:51-82` | Excluded, and opaque where a pointer names them (SCS-4) | Bit-fields, which a Rust struct cannot express | The safe crate cannot read the information of a format |
| Each record whose default member initializer is a nested list in braces, such as the one that holds `ColorClearValue color` | Excluded, and opaque where a pointer names it (SCS-76) | SCS-9 cannot express the value | None in M9. Render passes need them in M10 (RSQ-3) |
| `kEntireBuffer`, `kEntireTexture`, and `kAllSubresources` | Excluded | Constants of struct types, which no rule evaluates | The safe crate states its own ranges |
| `kTimeoutInfinite` and the other integer constants | Included | Plain constants | None |
| `rhiGetInstance` and `rhiDestroyInstance` | Included in an `unsafe extern "C-unwind"` block | The C functions of the library | The safe crate never calls `rhiDestroyInstance` (Section 6.4) |

The rules name every overload, every record with members that are not data, every placeholder slot, and every excluded record, so a new slang-rhi that adds one fails the generator and becomes a reviewed decision, as SCS-2 intends.

### 6.3. The ABI Probe

RSYS-11. The wrapper project MUST compile the ABI probe of SCS-78 of the sys design with the compiler and the flags of slang-rhi, into the static library `slang-rhi-sys-abi-probe`. A `static_assert` that fails MUST fail the build step, and the integration-test binary MUST compare every offset that the probe reports with the `offset_of!` of the matching field of the generated vtable struct.

The compiler that builds slang-rhi is the only authority on its ABI. The probe checks each struct layout at compile time, so a disagreement stops every build, and checks each vtable slot in a test, because the probe reads it from a value at run time. The order of the overloads, which the generator takes from declaration order, is checked by the same comparison. The `gpu` tests call both overloads of `readBuffer`, and those of the safe crate call `bindPipeline`, which adds a check that depends on no representation of member pointers for the overloads on the compute path (Section 9).

### 6.4. Safety Contract and Unwinding

RSYS-12. The crate-level section `# Safety` MUST state one contract that every generated function and thunk shares, with these conditions:

- Every pointer argument is valid for the access that the C++ signature implies, and every interface pointer points to a live object of that interface.
- A pointer that a method returns without a reference, which are the adapter of `IRHI::getAdapter`, the pass encoder of `beginComputePass` and the other pass beginnings, and the root object of `bindPipeline`, is used only while its owner of Section 3.3 keeps it valid, and is never released.
- An object that a field of type `*mut T` of `Binding` names stays alive through a reference that the caller owns until the call returns, because the callee adds its own references and Rust runs no C++ destructor.
- `rhiDestroyInstance` is never called while a device lives, because it then aborts the process (Section 3.3).
- No object is used from two threads at once unless slang-rhi documents that the object tolerates it, which it documents only for `IDebugCallback` and `ITaskPool` (Section 3.3).
- A program that implements `IDebugCallback` in Rust defines `handleMessage` `extern "system"` and stores it in its `"system-unwind"` field through a conversion of the function pointer, and the implementation tolerates calls from several threads at once.

The generated vtable entries use `"system-unwind"` and the C functions `"C-unwind"`, as SCS-10 and SCS-11 of the sys design require for Slang. slang-rhi itself holds no `throw`, but it calls Slang and the standard library, through which a C++ exception can escape (Section 3.3), and an exception that enters Rust through a non-unwinding ABI is undefined behaviour (FFI-7). A Rust implementation of `IDebugCallback` stays non-unwinding, so that a panic aborts at the boundary instead of unwinding into slang-rhi.

The generated code carries no `Send` or `Sync` implementation, and its raw pointer fields make every interface struct neither (FFI-9).

## 7. Platforms

### 7.1. Linux

RSYS-13. The build script MUST check the target first, after it reads `TARGET` and `HOST` and before it reads any metadata of its dependencies or starts any program. It MUST build on a target whose `CARGO_CFG_TARGET_OS` is `linux` and whose `CARGO_CFG_TARGET_ENV` is `gnu`, and MUST stop the build on every other target with an error that names the target and says that `slang-rhi-sys` builds only for Linux with the GNU C library, because its builds of the D3D12 and Metal backends are not designed. It MUST then stop on a cross-compilation, with the message of SGL-5 of the `slang-glslang-sys` design adapted to slang-rhi.

A build of Windows or macOS with only the Vulkan backend would contradict the owner's decision that D3D12 and Metal are the backends of those platforms (Section 4.1), and a build of D3D12 needs a download that ARCH-35 of the architecture design forbids (Section 3.5). A stop in the build script names the reason before CMake fails in a way that does not. The vendored `slang-compiler-sys` cannot cross-compile, so this crate cannot either.

On a target other than Linux with the GNU C library, the crate has no dependencies (RSYS-1), so Cargo sets none of the variables of their metadata, and a check after the reading of the metadata would report a missing dependency instead of the platform. A cross-compilation from one Linux target with the GNU C library to another keeps the dependencies, so the build script of `slang-src` clones first, and the build script of `slang-compiler-sys` stops with its own error that it cannot cross-compile before this build script runs.

A build on Linux needs everything that a vendored build of `slang-compiler-sys` needs (SRC-20 of the `slang-src` design), and the development headers of Xlib, such as those of the Debian package `libx11-dev` (Section 3.5, RSYS-17).

x86_64 Linux is the tested target. Another architecture of Linux with the GNU C library is allowed, untested, and unsupported, as ARCH-37 of the architecture design requires, and the build script of `slang-compiler-sys` already states it in its warning. On AArch64, the ABI probe relies on SA-21 of the sys design.

At run time, a program needs the Vulkan loader `libvulkan.so.1` and a Vulkan driver, such as lavapipe, to open a device. A program without them builds and links, and fails when it creates a device.

### 7.2. The Test for Windows and macOS

RSYS-14. Before any design of a build for Windows or macOS, a code step MUST run, on the hosted Windows and macOS runners of CI and on a branch whose result does not reach `main`, a configure and build step of the wrapper project with `FETCHCONTENT_FULLY_DISCONNECTED=ON`, with every download of Section 3.5 turned off or redirected, and with only the default backend of the platform. The agent MUST bring the result to the owner, and MUST NOT design or add a download.

On Windows, the step turns on D3D12 and turns off the Agility SDK, NVAPI, the fetch of DXC, and every other backend, and it uses the D3D12 headers of the Windows SDK of the runner. On macOS, it turns on Metal and redirects metal-cpp to `external/metal-cpp` of the source tree, which is a submodule for macOS 14.2 while slang-rhi pins metal-cpp for macOS 15.2 (Slang's `external/CMakeLists.txt:178-181`). The step records for each platform whether the configure step reached the network, whether it failed for the missing WinPixEventRuntime on Windows, whether the sources compiled, and the frameworks or libraries that a link needs.

The result answers RSQ-1, or narrows it for the owner. The step changes no requirement of this document. When the owner decides, the change that follows revises Sections 5 and 7 and adds the submodules that the build reads to Section 3.4 of the `slang-src` design.

## 8. The Build Script

RSYS-15. The build script MUST keep its logic in `build_logic.rs` behind the trait `Host`, and its real host in `system.rs`, which `build.rs` and the tests include, as SYS-8 requires. It MUST check every exit status and return the errors of this table from `main` (SYS-4), and it MUST stop the build only for the cases of this table:

| Failure | The error contains |
| --- | --- |
| `DEP_SLANG_SRC_*` or `DEP_SLANG_COMPILER_*` missing, or `DEP_SLANG_COMPILER_LINKAGE` not `static` | The variable, and that the crate depends on `slang-src` and on the vendored `slang-compiler-sys` directly |
| The target is not Linux with the GNU C library, or differs from the host | RSYS-13 |
| `cmake`, `ninja`, or the compiler probe cannot start | The program, the command, the reason, and that the build needs CMake 3.22 or later, Ninja, a C++20 compiler, and the development headers of Xlib, such as those of the Debian package `libx11-dev` |
| The configure or build step fails, or a `static_assert` of the ABI probe fails | The command, the step, the log `build.log`, and its last 40 lines |
| The C++ compiler does not find `X11/Xlib.h` | The error of a failed configure step, whose last 40 lines of the log hold the message of RSYS-17 |
| A library or the configured header is missing after the build | The path |
| The generator fails | The error of `slang-bindgen` with its sources, and, for an error of kind `Libclang`, that the crate generates its bindings with libclang in every build |
| An operation on the entry fails | The action, the path, the reason, and that removing the entry when no build runs is safe (SGL-6) |

The build logic repeats the parts of `route.rs` and `system.rs` of `slang-compiler-sys` that `build_logic.rs` of `slang-glslang-sys` repeats, as the fourth copy of the entry protocol of SRC-16 (Section 4.1 of the `slang-glslang-sys` design). CMake 3.22 is the minimum of the wrapper project, as for `slang-glslang-sys`, above the 3.20 of slang-rhi.

The build script exports no `links` metadata, because no crate builds native code against slang-rhi.

## 9. Tests

The crate has one integration-test binary, `tests/rhi_sys/`, which carries the `unsafe_code` expectation on its modules `ffi` and `gpu`. It has these modules:

- `build_script`: pure tests of `build_logic.rs` against a fake host. They cover the arguments and the environment of each CMake run, the order of the link instructions, the instruction of the probe archive, the key of the entry and its use and replacement, the stop for each target of RSYS-13 before any metadata is read or any program runs, and every failure of RSYS-15 with its message.
- `build_script::ffi`: tests of `system.rs`, which start processes and write files, as for `slang-glslang-sys`.
- `ffi`: the native-symbol test, which references both C functions of `symbols.rs`, the comparison of every slot offset of the ABI probe with the generated vtable structs (RSYS-11), and a smoke test that calls `rhiGetInstance` and then `IRHI::getDeviceTypeName` for the Vulkan device type through the generated thunk and checks that the result is not null. These tests open no device.
- `gpu`: one test that opens a Vulkan device on the first adapter with a debug callback implemented in Rust as RSYS-12 states, creates a buffer with initial data, reads it back through `readBuffer` and through `readBuffer_blob`, and compares both with the data. It is the call test of one pair of overloads, and it shows that a device opens on lavapipe.

Until the bindings exist (RSYS-9), `main.rs` declares only the module `build_script` and carries no `unsafe_code` expectation. The pure tests then cover no instruction of the probe archive, and the modules `ffi` and `gpu` and the module `symbols` of RSYS-2 arrive with the bindings.

The pure tests of `build_script` could run under Miri, but no recipe runs Miri on the crate, because `check-miri` leaves it out with the other crates of TOOL-32 (SRCQ-3 of the `slang-src` design). The tests of the safe crate exercise the rest of the compute path (`docs/design/slang-rhi.md`).

The recipe `test-rhi` runs these tests with the nextest profile `gpu`, and the CI job `vendored` runs it on Linux, where lavapipe provides the adapter (TOOL-52 and Section 7.4 of `docs/design/tooling.md`).

## 10. Crate Documentation

RSYS-16. The crate-level documentation MUST state what the crate builds and links, that it always builds Slang from source through the vendored `slang-compiler-sys`, the build requirements of Section 7.1 and of a vendored build, the platform rule of RSYS-13, the safety contract of RSYS-12, and that it has no Cargo features, in a `# Features` section and a `# Platform support` section. It MUST state the third-party code that a program built with the crate contains, which is VMA under the MIT licence and the runtime of CMakeRC (Section 3.5), and MUST name the shared folder as SRC-20 of the `slang-src` design lists it.

A program that ships a binary built with the crate carries the notices of the code in it, and the package of the crate holds none of that code, so the documentation is where a user learns of it (RSA-8, RSQ-4).

The safety contract arrives in the documentation with the bindings (RSYS-9), because before them the crate has no unsafe item.

## 11. Security Considerations

The threat model has four parts:

1. The source tree comes from `slang-src`, which fetches it by commit hash (Section 14 of the `slang-src` design), and the wrapper build runs the CMake scripts of slang-rhi on it with the user's permissions.
2. A CMake build of slang-rhi downloads dependencies by default, including a prebuilt Slang and a prebuilt Dawn (Section 3.5).
3. The generated bindings declare several hundred methods whose layout a defect of libclang or of the generator could get wrong without a compile error.
4. A program calls slang-rhi with GPU resources and shader programs, and slang-rhi calls a graphics driver.

Unsafe code and foreign function interface (FFI) invariants. The crate's only unsafe code is the generated declarations, whose shared contract RSYS-12 states. The layout assertions of SCS-12 of the sys design and the ABI probe of RSYS-11 check every struct and every slot against the compiler that built slang-rhi. The crate relies on slang-rhi being built from the same tree and against the same headers as the Slang that it links, which the exact version of `slang-src` and the include directory of RSYS-3 ensure.

Untrusted shader source and file paths. The crate passes nothing to slang-rhi itself. Shader source reaches Slang through `slang-compiler` and slang-rhi, with the considerations of Section 17 of `docs/design/slang-compiler.md`.

Build scripts that run native builds. The build script runs CMake, Ninja, and the compiler, and writes only under `OUT_DIR` and its entries in the shared folder (RSYS-7). The configure step disconnects FetchContent and redirects the Vulkan headers to the tree, so it reaches no network, and RSYS-14 forbids adding a download.

Dependencies. The crate depends on `slang-src` with its exact version and on `slang-compiler-sys`, on Linux with the GNU C library only, and, once it generates the bindings, on `slang-bindgen` with its exact version at build time (RSYS-1). The build also reads the development headers of Xlib from the system, which the user's package manager provides. slang-rhi, VMA, and CMakeRC come from the gitlink of the recorded commit, so the commit hash fixes their content.

These attacks are out of scope:

- A source tree whose content is malicious, which the user chose by patching in a fork of `slang-src`.
- An attacker with write access to the target directory, as Section 14 of the `slang-src` design states.
- Defects of a graphics driver, which run in the program's process.
- Attacks on the repository itself, which are a matter of access control.

The residual risk has four parts:

1. slang-rhi documents no thread safety for its objects, so the safe crate keeps them to one thread (`docs/design/slang-rhi.md`), and a user of the raw bindings carries that duty alone.
2. A slot that the ABI probe does not check, which is every slot on the Microsoft ABI until SA-19 of the sys design is confirmed, rests on the generator alone. M9 builds no such target.
3. The removal of warnings as errors (RSYS-5) lets code that a newer compiler warns about compile, as it does for every other native library of the workspace.
4. slang-rhi aborts the process on a failed assertion in every build type (Section 3.3), so a misuse that its assertions catch stops the program.

## 12. Operational Considerations

### 12.1. Cargo Features

The crate has no Cargo features. The CUDA, OptiX, and CPU backends become features in M15 (Section 15.18 of the architecture design).

### 12.2. Environment Variables

This table lists every environment variable that the build script reads or sets, with its default.

| Variable | Read by | Default and behaviour when absent |
| --- | --- | --- |
| `DEP_SLANG_SRC_ROOT`, `DEP_SLANG_SRC_COMMIT`, `DEP_SLANG_SRC_BUILD_TAG`, `DEP_SLANG_SRC_SHARED_DIR` | The build script | Set by Cargo from the metadata of `slang-src`. Absence stops the build (RSYS-15) |
| `DEP_SLANG_COMPILER_INCLUDE`, `DEP_SLANG_COMPILER_LIB`, `DEP_SLANG_COMPILER_LINKAGE`, `DEP_SLANG_COMPILER_BUILD_TAG` | The build script | Set by Cargo from the metadata of `slang-compiler-sys` (SCS-54 of the sys design). Absence, or a linkage other than `static`, stops the build |
| `OUT_DIR`, `CARGO_MANIFEST_DIR`, `TARGET`, `HOST`, `NUM_JOBS` | The build script | Set by Cargo, as for `slang-glslang-sys` (Section 12.2 of that design) |
| `CARGO_CFG_TARGET_OS`, `CARGO_CFG_TARGET_ENV` | The build script | Set by Cargo. They select the stop of RSYS-13 |
| `LIBCLANG_PATH` | The generator, once the build script runs it (RSYS-8) | As for `slang-compiler-sys` (Section 18.2 of the sys design) |
| `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`, `CMAKE_TOOLCHAIN_FILE`, `MACOSX_DEPLOYMENT_TARGET`, `VCINSTALLDIR`, `VCToolsVersion` | CMake, the compiler probe, and the entry key | Passed through unchanged. The build script reruns when one changes (RSYS-8) |
| `PATH` | The operating system, when the build script starts `cmake`, `ninja`, and the compiler probe | A program that cannot start stops the build (RSYS-15) |

### 12.3. Platforms and Minimum Supported Rust Version

The crate builds on Linux with the GNU C library and is tested on x86_64 Linux in the CI job `vendored` (RSYS-13). It uses the workspace's `rust-version`.

### 12.4. Failure Modes

This table lists the failure modes of the crate, how each is reported, and the recovery.

| Failure | Reported by | Recovery |
| --- | --- | --- |
| A target other than Linux with the GNU C library, or a cross-compilation | The error of the build script (RSYS-13) | Build on Linux. Windows and macOS wait for RSQ-1 |
| A program of the build cannot start, or the configure or build step fails | The error of the build script, with the log (RSYS-15) | Install CMake, Ninja, or a C++20 compiler, or fix the toolchain that the log names |
| The C++ compiler does not find `X11/Xlib.h` | The error of the failed configure step, with the message of RSYS-17 in the log | Install the development headers of Xlib, such as the Debian package `libx11-dev`, and build again |
| The generator refuses a declaration of a slang-rhi other than that of the recorded commit | The error of the generator (SCS-2 of the sys design) | Use the recorded commit, or change the rule set and this design |
| A `static_assert` of the ABI probe fails, or a slot offset differs in the test | The build step, or the `ffi` test | A defect of the generator or of libclang for the target. Do not use the bindings on that target until it is fixed |
| No Vulkan loader or driver at run time | The creation of a device fails, through the safe crate (`docs/design/slang-rhi.md`) | Install the Vulkan loader and a driver, such as lavapipe |
| `rhiDestroyInstance` while a device lives | slang-rhi aborts the process (Section 3.3) | Do not call it while a device lives. The safe crate never calls it |

### 12.5. Logs

The build script writes the output of every CMake step to `OUT_DIR/build.log` and shows its last 40 lines only in the error of a failed step. The crate writes nothing at run time. slang-rhi reports its messages through the debug callback of a device, which answers Q-16 of the architecture design for this crate.

## 13. Assumptions to Confirm

Each row is a fact that this design relies on and that is not verified. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| RSA-7 | A method with an out-parameter of an interface pointer returns it with one reference added, as COM convention requires | RSYS-12, the safe crate | The code step of the safe crate, by a reading of `src/` for each method that it calls, recorded in `docs/design/slang-rhi.md` |
| RSA-8 | `cmake/CMakeRC.cmake` is under the MIT licence of its upstream project, while the file in the tree carries no licence text | RSYS-16 | A reading of the upstream repository of CMakeRC, by the owner or an agent with network access |
| RSA-9 | `cargo::rustc-link-arg-tests` reaches the integration-test binary of the crate and no other target, so only the tests link the probe | RSYS-6 | The code step that adds the ABI probe |

## 14. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row. An identifier is never reused.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| RSQ-1 | How does the build of `slang-rhi-sys` obtain what the D3D12 build on Windows and the Metal build on macOS need, given that WinPixEventRuntime has no option to skip it and the metal-cpp of the tree is older than the pin? This is the rest of Q-35 of the architecture design | The owner, after the test of Section 7.2 | M9 or later |
| RSQ-2 | Should the generator support aggregates passed by value for the tested targets, with the ABI probe extended to calls, so that the methods of the placeholder slots become callable? Rendering in M10 needs some of them, such as `drawIndirect` | This document | M10 |
| RSQ-3 | Should the generator evaluate default member initializers that are nested lists in braces, so that the records of render passes are included? | This document | M10 |
| RSQ-4 | Is the statement of the third-party code in the crate documentation enough, or does the project ship the licence texts of VMA and CMakeRC for the programs that users build? | The owner of the project, by a decision | Before the second release |

## 15. References

### 15.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| RSYS-1 | 4.2 | `links = "slang-rhi"`, no features, the exact `slang-src` and the vendored `slang-compiler-sys` for Linux with the GNU C library only, `slang-bindgen` at build time with the bindings, and no `find-msvc-tools`. |
| RSYS-2 | 4.3 | The crate root holds the documentation, the `unsafe_code` expectation, the two `use` declarations under the condition of the target, the generated module, and its re-export, and nothing hand-written. |
| RSYS-3 | 5.1 | A wrapper project builds slang-rhi out of tree from the tree in place, with an INTERFACE `slang` target over the headers of the vendored Slang and `SLANG_STATIC`. |
| RSYS-4 | 5.2 | The options of the table, FetchContent disconnected, and the Vulkan headers taken from the tree, so the configure step reaches no network. |
| RSYS-5 | 5.3 | The wrapper removes the warnings as errors of slang-rhi. |
| RSYS-6 | 5.4 | Three archives checked and linked in order, then `stdc++`, `dl`, and `pthread`, and the probe archive only for the tests. |
| RSYS-7 | 5.5 | The archives and the configured header live in an entry of the shared folder with the protocol and key of `slang-src`. |
| RSYS-8 | 5.6 | Rerun instructions for the variables of CMake and the lookup of `cl.exe`, for `LIBCLANG_PATH` once the generator runs, and for the stamp file of the used entry. |
| RSYS-9 | 6.1 | Every run generates the bindings from the headers and the configured header of the build, and a build writes the ABI probe before the build step. |
| RSYS-10 | 6.2 | The rule set decides each kind of declaration as the table states. |
| RSYS-11 | 6.3 | The ABI probe compiles with slang-rhi's compiler, and a test compares every slot offset. |
| RSYS-12 | 6.4 | One shared safety contract, unwinding ABIs for calls into slang-rhi, and a non-unwinding Rust debug callback. |
| RSYS-13 | 7.1 | Linux with the GNU C library only, no cross-compilation, and a clear stop on every other target, checked before the metadata of the dependencies is read. |
| RSYS-14 | 7.2 | A test of Windows and macOS with downloads off, whose result goes to the owner before any design of those builds. |
| RSYS-15 | 8 | The logic behind `Host`, every exit status checked, and the failures of the table. |
| RSYS-16 | 10 | The crate documentation states the build, the requirements, the platforms, the contract, the features, and the third-party code in a program. |
| RSYS-17 | 5.2 | The wrapper project checks that the compiler finds `X11/Xlib.h` before it adds slang-rhi, stops the configure step with a message that names the header and `libx11-dev` otherwise, and bundles no stub. |
