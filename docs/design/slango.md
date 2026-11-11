# `slango` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-11 |
| Applies to | `crates/slango` and the compute kernel example |

## Abstract

This document specifies the first milestone of `slango`, the high-level library for running Slang code on a GPU from Rust, and the compute kernel example that milestone M3 delivers with it. In M3, `slango` calls existing compute entry points of a Slang module on wgpu, with bindings built from Slang's reflection alone. The document starts from the facts that a spike established about Slang's SPIR-V on wgpu's validated shader path, including the mapping from reflection to wgpu bindings and the constructs that the path rejects. On those facts it defines four public types, the one public module that exposes wgpu, the loading of a module into an owned layout snapshot that frees every Slang object, the call by parameter name with its checks before dispatch, the errors, the threading, and the device features. It also specifies the tests, the example, the dependency changes, and the order of the changes that implement M3. It ends with the assumptions still to confirm and the open questions.

## 1. Introduction

### 1.1. Problem

`slang-compiler` turns Slang source into SPIR-V and reports the bindings of each parameter, but running that code on a GPU takes much more. A program must open a device, build bind group layouts and a pipeline that match the shader exactly, lay out every plain value by the shader's rules, bind buffers, choose a dispatch size, and copy results back. A mistake in any of these steps fails at run time with a message about descriptor sets and offsets, or computes wrong numbers without any error.

### 1.2. Purpose

`slango` lets a Rust program load a Slang module, look up a compute kernel, set its parameters by name, dispatch it over a number of elements, and read the results, with every binding derived from reflection. This document implements Section 8 and milestone M3 (Section 15.6) of `docs/design/architecture.md`, meets ARCH-15 to ARCH-19, and answers the questions and assumptions of that design that name this document, as Section 15 and Section 16 of this document record.

### 1.3. Audience

The primary readers are the coding agents that implement M3 and the reviewers who check their changes. A reader is expected to know Rust but not GPU programming. Section 2.1 defines the terms that such a reader needs.

### 1.4. Scope

This document covers the compute entry points that a Slang module already declares with `[shader("compute")]`, compiled to SPIR-V and loaded through wgpu's validated path, with buffers, constant buffers, parameter blocks, and plain `uniform` values as parameters. It covers the public types, the backend boundary, loading, the call, errors, threading, device features, tests, continuous integration (CI), the example, and the dependency changes.

### 1.5. Non-Goals

This document designs nothing beyond M3. These belong to later milestones: kernels generated around a plain Slang function with a positional call, broadcasting over array arguments, a tensor type with a shape, differentiation driven from Rust, textures and samplers as arguments, the unchecked shader passthrough path, shader targets other than SPIR-V, and rendering.

### 1.6. Current State

Step 1 of Section 14 is done, on 2026-11-11. `crates/slango` holds `Device` with `DeviceOptions` and `AdapterSelection` (Section 5.1), the error type `Error` with `ErrorKind` and `Result` (Section 10), and the public module `slango::wgpu` (Section 6), which builds a `Device` from a wgpu device and queue and returns them. Its first tests of the `gpu` tier create devices on every CI runner (Sections 3.8 and 13). Step 2 is done, on 2026-11-11. The private module `layout` holds the layout snapshot and the walk of Section 7.1, and a private helper that compiles a module from source text into one snapshot for each compute entry point, with unit tests that Miri runs and `ffi` unit tests that call Slang (Section 13.1). Until step 3, only its tests called it. Step 3 is done, on 2026-11-11. `Device::load_module` loads a Slang file and `Device::load_module_from_source` loads source text, each into a `Module` with one `Kernel` for each compute entry point, and the load creates the compute pipeline of each kernel (Sections 7.1 and 7.3). `Module::kernel` looks a kernel up by name, and `Kernel::name` and `Kernel::thread_group_size` return its name and the size of its thread group. Step 4 is done, on 2026-11-11. `Buffer::from_slice` and `Buffer::zeroed` create a `Buffer<T>` on a device, `Buffer::read` copies its elements back, and `Buffer::len` and `Buffer::is_empty` return its length (Section 8). The sealed traits `Element` and `Value` list the element types of a buffer and the Rust types of plain values (Sections 8 and 9.2). Until step 5, `Value` has no public consumer and `Element` has none besides `Buffer`. Steps 5 and 6 do not exist yet, so a program can create a device, load a module, look up kernels, and create and read back buffers, but cannot call a kernel. The spike that ARCH-18 requires ran on 2026-11-11 as a scratch program that the repository does not keep, and Section 3 records what it established. Section 14 gives the order of the changes that implement M3.

### 1.7. Defects of Third-Party Components and Trusted Input

`slango` follows Hard Rule 14 of `AGENTS.md` and Section 1.7 of `docs/design/slang-compiler.md`, and applies the same stance to naga and wgpu. Shader source and file paths are trusted input. When naga rejects a module, `slango` reports naga's own text and does not screen modules for the constructs of Section 3.5 in advance. The checks of the arguments of a call stay, because a safe interface needs them (Section 9.3). A defect of Slang that M3 finds is recorded in Section 3.12 of `docs/design/slang-compiler.md`, not here.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

Section 2.1 of `docs/design/architecture.md` defines adapter, buffer, compute dispatch, compute kernel, device, lavapipe, marshalling, shader passthrough, software adapter, spike, SPIR-V, and wgpu. Section 2.1 of `docs/design/slang-compiler.md` defines descriptor set, layout, parameter category, thread group, type layout, and variable layout. This document adds these terms:

- bind group: The wgpu name for the set of bindings that a dispatch uses together. One bind group fills one descriptor set, and its bind group layout states the kind of each binding.
- binding: A numbered slot in a descriptor set through which a kernel reaches a buffer.
- element count: The number of threads a caller asks a dispatch to run along each of three axes.
- error scope: A wgpu mechanism that collects the validation errors of the calls made while it is open and returns them as values.
- immediate: The wgpu 30 name for a push constant.
- kernel: In this document, one compute entry point of a loaded module, ready to dispatch.
- layout snapshot: The owned Rust data that `slango` copies from Slang's reflection at load time and from which it builds every wgpu layout (Section 7.1).
- naga: The shader translator inside wgpu, which parses SPIR-V, validates it, and translates it for the device's graphics programming interface.
- parameter path: The name of a plain value or a buffer as the call sets it, with a full stop between the name of a constant buffer or parameter block and the name of a field, such as `params.scale`.
- plain value: A scalar, vector, matrix, array, or structure value that a kernel reads from a uniform buffer or a push constant, as opposed to a buffer that it reaches through a binding.
- push constant: A small block of plain values passed with a dispatch instead of through a buffer.
- std140, std430: The two memory layout rules of Vulkan that Slang's SPIR-V uses by default, std140 for uniform buffers and std430 for storage buffers and push constants.
- storage buffer: A buffer that a kernel reads and, unless it is read-only, writes.
- uniform buffer: A read-only buffer of plain values. Slang calls it a constant buffer.
- validated path: The loading of SPIR-V through `ShaderSource::SpirV`, in which naga parses and validates the module before the device sees it.
- workgroup: The wgpu name for a thread group.

## 3. Facts From the Spike

This section is non-normative. It states each fact that a later section relies on, once, and says how it is known.

### 3.1. Sources and Evidence

Each fact carries one of these marks:

- (run): Verified on 2026-11-11 by a scratch program on x86_64 Linux, built in debug mode, with `slang-compiler` of the repository at commit `f4011b7`, wgpu 30.0.1 with the feature `spirv`, and naga 30.0.1. It ran on two Vulkan adapters, lavapipe of Mesa 25.0.7 and an NVIDIA GeForce RTX 4070 Laptop GPU, with identical results on both. In Section 3.8 alone, it means verified on 2026-11-11 by the `gpu` tests of step 1 of Section 14 on GitHub's hosted runners. In a fact that names step 2, it means verified on 2026-11-11 by the `ffi` tests of step 2 of Section 14 on x86_64 Linux. In a fact that names step 3, it means verified on 2026-11-11 by the work of step 3 of Section 14, in CI run 42952435667 or, where the fact says so, locally on the two Vulkan adapters of the development machine and not in CI. In a fact that names step 4, it means verified on 2026-11-11 by the work of step 4 of Section 14 on x86_64 Linux: a fact about layout through Slang's reflection and the SPIR-V decorations that the `slangc` of the pinned Vulkan SDK printed, and not on a GPU, and a fact about wgpu by creating devices on the development machine.
- (read): Read in a source on 2026-11-11, which the fact names.
- (inferred): Concluded from other facts and not verified. Every inferred fact that the design depends on is also a row of Section 15.

### 3.2. What Ran

Slang compiled three kernels to SPIR-V with `slang-compiler`, the scratch program built every bind group layout and pipeline from reflection alone, dispatched each kernel, read the results back, and every value matched (run). The kernels covered a read-write structured buffer with a global `uniform`, a read-only and a read-write structured buffer with a `ConstantBuffer`, a `ParameterBlock` holding a buffer, and two entry-point `uniform` parameters, and a kernel with an unused buffer and an unused uniform (run). Through the validated path, naga also accepted control flow of every kind, generics with interface constraints, `[Differentiable]` functions with `bwd_diff` and `fwd_diff`, atomics, matrices, uniform arrays, `static const` tables, `inout` and `out` parameters, `bool` fields, `cbuffer`, and nested parameter blocks (run). No unsafe code was needed (run). The passthrough path also ran every kernel on both adapters, and it needs unsafe code and `Features::PASSTHROUGH_SHADERS` (run). wgpu 30's `Device`, `Queue`, `Buffer`, `ComputePipeline`, and `BindGroup` are `Send` and `Sync` (run), and all wgpu types are `Clone` through internal reference counting since wgpu 24 [WGPU-CHANGELOG] (read). In wgpu 30.0.1, two devices from two device requests compared equal with `==` (run on the development machine on 2026-11-11, step 4, with no test in the repository).

By default, Slang renames every SPIR-V entry point to `main`, and with `vulkan_use_entry_point_name(Toggle::On)` the SPIR-V entry point keeps the Slang name (run). Slang's documentation states the same default [SLANG-SPIRV] (read).

### 3.3. Mapping From Reflection to wgpu

This table is the core of the crate. "DTS" is `ParameterCategory::DescriptorTableSlot`. A walk starts at `ProgramLayout::global_params_var_layout` and at `EntryPointLayout::var_layout`. Outside a parameter block, the binding of a variable is the sum of `offset(DTS)` along its path, and its set is the sum of `binding_space(DTS)` along its path. Every row is (run) unless it says otherwise.

| Slang declaration | What reflection reports | wgpu layout entry |
| --- | --- | --- |
| `RWStructuredBuffer<T>` | `TypeKind::Resource`, access `ReadWrite`, category DTS, binding type `MutableRawBuffer`. Element stride from `element_type_layout().stride(Uniform)`: 4 for `float`, 16 for `float4`, 32 for `{float3, float, float2, uint}` | Storage buffer, not read-only, with `min_binding_size` equal to the stride |
| `StructuredBuffer<T>` | The same, with access `Read` and binding type `RawBuffer` | Read-only storage buffer |
| `ConstantBuffer<S>` or `cbuffer` | `TypeKind::ConstantBuffer`, category DTS. Binding from `container_var_layout()`, size from `element_var_layout().type_layout().size(Uniform)`, field offsets from `offset(Uniform)` | Uniform buffer with `min_binding_size` equal to the size |
| Global `uniform` plain value | The global scope becomes an unnamed `TypeKind::ConstantBuffer` at DTS 0, and every other global binding moves up by one. Fields at `offset(Uniform)` with std140 rounding | One uniform buffer at set 0, binding 0, that `slango` owns and fills |
| `uniform` parameter of the entry point | `TypeKind::ConstantBuffer` with category `PushConstantBuffer` and binding type `PushConstant`, std430 offsets | No binding. The pipeline layout's immediate size, then the immediates of the compute pass. Needs `Features::IMMEDIATES` (Section 12) |
| `ParameterBlock<S>` | `TypeKind::ParameterBlock`, category `SubElementRegisterSpace`. Its set is the sum of `offset(SubElementRegisterSpace)` along the path, including each element variable. Bindings restart at 0, with an implicit uniform buffer first when `S` has plain values. The parent's `descriptor_sets()` leaves the block's set out | One more bind group layout at that set |
| `ParameterBlock` inside a `ParameterBlock` | The inner set is the outer set plus 1, by the same sum | Two bind group layouts |
| System values such as `SV_DispatchThreadID` | The type layout of `uint3 id : SV_DispatchThreadID` has a uniform size of 0 (run, step 2) | Nothing. The walk skips every plain value of uniform size 0 |
| `[numthreads(x, y, z)]` | `compute_thread_group_size()` returns `[x, y, z]` | Workgroup counts derived from element counts (Section 9.4) |
| A parameter that the code never uses | Still listed with its binding | The SPIR-V drops it. The layout is accepted with or without its entry |
| `Texture2D`, `SamplerState`, `RWTexture2D` | Binding types `Texture`, `Sampler`, `MutableTexture` | Not built. The walk rejects them (Section 7.1) |

For each element type of `Buffer<T>`, the reflected stride of a structured buffer of the matching Slang type equals the stride that `slango` uses for `T`, so the size of every buffer, its length times that stride, is a multiple of the stride (run, step 4).

### 3.4. Layout of Plain Values

Slang's default layout puts the fields of `struct Params { float scale; float3 offset; uint count; float2 pair; }` in a constant buffer at offsets 0, 16, 28, and 32, with size 48 (run). `float c[5]` in a constant buffer takes 80 bytes, with a stride of 16 (run). `float4x4`, `float3x3`, `float2x3`, and `float3` in sequence take offsets 0, 64, 112, and 144, with size 160 (run). A `bool` takes 4 bytes (run). As `uniform` parameters of an entry point, `float`, `float3`, `float4x4`, a structure of a `float` and a `uint`, and `bool` take offsets 0, 16, 32, 96, 100, and 104 in the immediates. The last value ends at 108, and the block has size 112, because std430 rounds the size of the block up to its alignment of 16 (run, step 2). These offsets differ from the layout of any Rust struct with the same fields, so `slango` writes every plain value at its reflected offset and never relies on the layout of a Rust struct (SLO-13).

Slang's default matrix layout is row-major, which `defaultMatrixLayoutMode` in `slang.h` states, and `slango` sets no matrix layout (run, step 4). In that layout, a matrix of R rows and C columns is R rows, each starting at a multiple of the matrix stride. Row r starts at the value's offset plus r times its extent divided by R, and column c of a row follows at 4 bytes times c (run, step 4). The extent is the reflected size of the value, or the array stride for a matrix inside an array. It is R times 16 bytes in a uniform buffer and R times 8 or 16 bytes in the immediates, for every R and C from 2 to 4 (run, step 4). The `slangc` command line defaults to the other layout, so an experiment with it must pass `-matrix-layout-row-major` to match what `slango` compiles (run, step 4). No GPU has checked this rule yet, and the dispatch tests of step 5 will.

### 3.5. Known Limits of the Validated Path

naga rejects these valid Slang constructs, with the same text on both adapters (run). `slango` does not screen for them (Section 1.7).

| Construct | naga's error text |
| --- | --- |
| Scalar layout (`glsl_force_scalar_layout`) | `The struct member[1] offset 4 is not a multiple of the required alignment 16` |
| `uint8_t` | `parsing error: UnsupportedCapability(UniformAndStorageBuffer8BitAccess)` |
| `frexp` | `parsing error: UnsupportedExtInst(51)` |
| `ByteAddressBuffer` read as both `uint` and `float` | `Bindings for [2] conflict with other resource` |
| An array of buffers, such as `StructuredBuffer<float> inputs[2]` | `Base type [9] for the array is invalid` |
| An array of textures, sized or unsized | `parsing error: InvalidImage([12])` |
| `RWTexture2D<float4>` without a `[format(...)]` | `parsing error: UnsupportedCapability(StorageImageReadWithoutFormat)` |
| A subscript of a `Texture2D` | `Cannot textureLoad from a mipmapped image without specifying a level.` |

`double`, `int64_t`, and `half` load only on a device with the matching feature, and naga otherwise names the missing capability, such as `Using 'f64' values requires the 'naga::valid::Capabilities::FLOAT64' flag` (run). 16-bit integers belong with them. On the two Vulkan adapters of the development machine, a kernel with a local `int16_t` on a device without the 16-bit integer feature failed when its shader module was created, with a validation error that names the capability `SHADER_INT16`. This was run locally on 2026-11-11 and not in CI. It corrects the spike's record of `parsing error: InvalidTypeWidth(2)` for `int16_t`. `uint16_t` was not run.

### 3.6. Costs

`GlobalSession::new` took about 135 ms (134 ms and 145 ms), and a session, load, link, code generation, and reflection of a small kernel took 8 to 11 ms after it (run). A naga shader module took under 0.4 ms, layouts and a pipeline under 1 ms, and a dispatch with its wait under 1 ms, except the first dispatch of a pipeline on lavapipe, which took 15 to 20 ms (run). Each number is a single sample from a debug build and shows an order of magnitude only.

### 3.7. Versions and Prior Art

Slint 1.18.1 declares the features `unstable-wgpu-29` and `unstable-wgpu-30`, and wgpu 30.0.1 is the latest stable wgpu, with `rust-version` 1.87 (read, `cargo info` on 2026-11-11). SlangPy's documentation shows kernels generated around plain functions, broadcasting with `map()`, tensors, and differentiation through `bwds`, but its kernel cache and device layer were not read in its source [SLANGPY] (read). vulkano uses `Arc` ownership with children that hold their parents, structs of creation options, and validated calls with named unchecked variants, and a maintainer describes `GpuFuture` as hard to chain correctly while building an experimental replacement [VULKANO-2540] (read).

### 3.8. Adapters of the CI Runners

In CI run 42943216478, in which all six jobs passed, the `gpu` tests created a device with `Features::IMMEDIATES` on each hosted runner, through wgpu 30.0.1 (run). The operating system, driver, and adapter names are read from the log of that run:

| Runner | Adapter | Backend | Device type | `max_immediate_size` |
| --- | --- | --- | --- | --- |
| `ubuntu-latest`, Ubuntu 26.04, with `mesa-vulkan-drivers` 26.0.8 | lavapipe, named `llvmpipe (LLVM 21.1.8, 256 bits)` | Vulkan | `Cpu` | 256 bytes |
| `windows-latest` | WARP, named `Microsoft Basic Render Driver` | Direct3D 12 | `Cpu` | 128 bytes |
| `macos-latest` | `Apple Paravirtual device` | Metal | `IntegratedGpu` | 256 bytes |

lavapipe is the only adapter of the Linux runner, so the default selection also finds it there (run). Metal offers no software adapter: on the macOS runner, `AdapterSelection::Software` returned `ErrorKind::NoAdapter` in the earlier run 42941845864 (run). In CI run 42952435667, in which all six jobs passed, the `gpu` tests loaded modules on all three adapters (run, step 3). Kernels with immediates, a global uniform, a constant buffer, a parameter block, an import, and two kernels in one module reached the creation of their compute pipelines, and a kernel that calls `frexp` failed with the same backend text on all three. No runner has dispatched a kernel yet, so dispatch stays open (SOA-2).

## 4. Crate Structure and Dependencies

### 4.1. Package and Dependencies

SLO-1. `slango` MUST depend on `slang-compiler` and on wgpu of major version 30 with the feature `spirv`, and on no other crate except `pollster`, which blocks on wgpu's futures (Section 16). The list stays short because every dependency runs on every user's machine (DEP-2). Version 30 is the newest wgpu that the current Slint release supports (ARCH-19, Section 3.7). The implementation re-checks the version at each Slint release, and an upgrade is a breaking release of `slango` (DEP-3). The crate has no Cargo features in M3.

SLO-2. `slango` MUST contain no unsafe code in M3, and MUST load every shader through the validated path only. The validated path ran every kernel that M3 needs without unsafe code (Section 3.2), so passthrough would add unsafe code for no need. This answers Q-19 of the architecture design for `slango` in M3.

### 4.2. Licences and Duplicate Versions

wgpu's dependency tree brings crates under licences that `deny.toml` did not allow, and duplicate versions of `foldhash`, `hashbrown`, and `syn`. Step 1 of Section 14 added `MIT`, `Unicode-3.0`, and `Zlib` to the licence allow-list of `deny.toml`, and a `skip` entry with a reason for each of four duplicate versions, as TOOL-24 and TOOL-26 require. The project's licence stays GPL-3.0-or-later. Section 9 of `docs/design/tooling.md` records which crates need each licence, the evidence of its compatibility, and the reason for each skip.

## 5. Public Types and Ownership

### 5.1. Types

SLO-3. `slango` MUST provide the public types `Device`, `Module`, `Kernel`, and `Buffer<T>`, each of which holds its parent by value and is cheap to clone. It MUST create `Device` and the call through structs of options that implement `Default`. A value that holds its parent can be stored without a lifetime (OWN-3), and options can grow without a breaking change (API-6).

- `Device`: A wgpu device and its queue. It is created from options or, in the module of Section 6, from wgpu objects that the caller already holds.
- `Module`: One loaded Slang module. It holds its `Device` and one `Kernel` for each compute entry point.
- `Kernel`: One compute entry point, with its layout snapshot, its bind group layouts, and its compute pipeline. It holds its `Device`.
- `Buffer<T>`: A storage buffer of elements of type `T`, with its length. It holds its `Device`.

`Device::new` searches the primary backends of wgpu, which are Vulkan, Metal, Direct3D 12, and WebGPU in a browser, and never OpenGL. The field `adapter` of `DeviceOptions` selects the adapter with an `AdapterSelection`:

- `Default`, the default: The first adapter that wgpu finds. `Device::new` sets no power preference, so wgpu does not prefer a hardware GPU.
- `Software`: wgpu's fallback adapter, which is a software adapter such as lavapipe or WARP. Metal has none, so on macOS this selection returns `ErrorKind::NoAdapter` unless a Vulkan driver provides one (Section 3.8).
- `Name`: The first adapter whose name contains the given text, which also makes `ErrorKind::NoAdapter` testable.

`DeviceOptions` and `AdapterSelection` are non-exhaustive, so both can grow without a breaking change. `Device::adapter_name` returns the name of the adapter.

wgpu's types are already reference counted clones (Section 3.2), so `slango` wraps no wgpu object in an `Arc` of its own. It shares its own owned data, such as a layout snapshot, through `Arc`. This follows the vulkano pattern of children that hold their parents (Section 3.7) and adopts nothing of `GpuFuture` (ARCH-16): every wait in M3 is a blocking call (CONC-6).

### 5.2. Usage Illustration

This illustration shows the whole flow of M3. It shows the form only, and the implementation settles the exact names within the contracts of Sections 5 to 12. The kernel `scale` in `scale.slang` multiplies each element of a buffer by a factor:

```slang
RWStructuredBuffer<float> data;
uniform float factor;
uniform uint count;

[shader("compute")]
[numthreads(64, 1, 1)]
void scale(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count) { data[id.x] *= factor; }
}
```

The Rust program creates a device, loads the module, gets the kernel, creates a buffer, sets the uniforms, binds the buffer, dispatches, and reads the result back:

```rust
use slango::{Buffer, Device, DeviceOptions};

fn main() -> slango::Result<()> {
    let device = Device::new(&DeviceOptions::default())?;
    let module = device.load_module("scale.slang")?;
    let scale = module.kernel("scale")?;
    let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
    scale
        .call()
        .set("factor", 2.0_f32)
        .set("count", 4_u32)
        .bind("data", &data)
        .dispatch([4, 1, 1])?;
    let result: Vec<f32> = data.read()?;
    assert_eq!(result, [2.0, 4.0, 6.0, 8.0]);
    Ok(())
}
```

## 6. Backend Boundary

SLO-4. Every public item of `slango` that names a wgpu type MUST be in one public module, such as `slango::wgpu`, which holds the constructors from wgpu objects and the accessors that return them. Every other use of wgpu MUST stay private, because a later backend then changes no other part of the public API (ARCH-15).

The layout snapshot is the boundary inside the crate. The reflection walk above it knows Slang and no wgpu type, and the code below it turns the snapshot into wgpu layouts, bind groups, and dispatches. No backend trait exists (SIMP-2). This answers Q-17 of the architecture design.

## 7. Loading a Module

### 7.1. Compile and Snapshot

SLO-5. `Device::load_module` MUST compile the module on the calling thread and copy into a layout snapshot the facts that the kernels need. It MUST release every Slang object before it returns, because no Slang object may then outlive the call or cross a thread (Section 11).

The load reads the file as text and compiles it with `Session::load_module_from_source`, with the directory of the file as a search path so that an `import` of a sibling module resolves. A variant takes the source text and a module name instead of a path. The session targets `TargetFormat::Spirv` with Slang's default layout, and sets `vulkan_use_entry_point_name(Toggle::On)` so that every message of wgpu names the Slang function (Section 3.2). The load generates one SPIR-V module for each compute entry point through `LinkedProgram::entry_point_code`.

Step 3 settled these choices, which the design left open:

- `Device::load_module` takes the file stem as the module name, and the directory of the file, or `.` when the path has none, as the search path.
- `Device::load_module_from_source` takes the module name and the source text and sets no search path, so an `import` finds only what Slang finds by default.
- A path that is not valid UTF-8, which Slang needs, gives `ErrorKind::InvalidArgument`. So does a file that cannot be read as text, with the I/O error as the source error.

SLO-6. The layout snapshot MUST hold only owned data and MUST be private to `slango`. The data is the thread group size, the immediate size, and for each parameter its parameter path, its kind, its set and binding, whether the code uses it, and either its element stride or the scalar type, shape, offset, and size of each plain value. The snapshot is the backend-neutral boundary of Section 6, and a public snapshot would freeze its shape. The walk MUST follow the rules of Section 3.3, including the shift of global bindings for global plain values and the sum of `SubElementRegisterSpace` offsets for parameter blocks.

This answers Q-31 of the architecture design, and CQ-4 of `docs/design/slang-compiler.md`, for M3. It also makes Q-30 of the architecture design, and CQ-1 of `docs/design/slang-compiler.md`, unnecessary for M3, because no Slang object leaves `load_module`.

Step 2 built the snapshot as one record for each compute entry point, and the walk ignores entry points of other stages. Each record holds:

- The name of the entry point and its thread group size.
- Its bindings, sorted by set and binding, each with its parameter path and a usage flag. A binding is either a storage buffer with its access, read-only or read-write, and its element stride, or a uniform buffer with its size and its plain values.
- An optional block of immediates with its size and its plain values.

Each plain value has its parameter path, offset, size, scalar type, and shape. A shape is a scalar, a vector with its length, a matrix with its rows and columns, or an array with its length, stride, and element shape. The walk flattens a structure into one plain value for each leaf, with a dotted path such as `params.scale`. A constant buffer or parameter block without plain values gets no uniform buffer.

The design left these choices open, and step 2 settled them:

- A field of a named `cbuffer` gets the path `Name.field`, such as `Arrays.c` for `cbuffer Arrays { float c[5]; }`, although the shader reads it as `c`.
- A parameter of the entry point has a path without a prefix, so `uniform float factor` has the path `factor`, as a global has.
- The unnamed uniform buffer of the global plain values has the empty path.
- The usage of a binding comes from the metadata of the entry point, through `Metadata::is_parameter_location_used` with its set and binding. A plain value has the usage of the uniform buffer that holds it. The immediates carry no usage flag, so step 5 treats them as used.
- The walk does not record the matrix layout mode. Step 4 settled that marshalling needs none, because the session keeps Slang's default row-major layout, whose rows the extent of a matrix places (Section 3.4).

The walk supports these parameters and rejects every other with `ErrorKind::UnsupportedParameter`, whose text names the parameter and the kernel:

| Parameter | Snapshot |
| --- | --- |
| `StructuredBuffer<T>`, `RWStructuredBuffer<T>` | A read-only or read-write storage buffer |
| `ConstantBuffer<S>`, `cbuffer`, global `uniform` plain values | A uniform buffer |
| `ParameterBlock<S>`, nested to any depth | Its bindings in its own set |
| `uniform` parameters of the entry point | The immediates |
| `bool`, `int`, `uint`, and `float` as scalars, vectors, matrices, and arrays of fixed length, including arrays of arrays | Plain values |
| A structure of supported values | One plain value for each leaf |
| A system value | Nothing (Section 3.3) |

Among the rejected parameters are every resource other than a structured buffer, such as a texture, a storage texture, or `ByteAddressBuffer`, a sampler, a plain value of another scalar type, such as `double`, an array of structures or of resources, and an array without a fixed length. A failure of Slang to compile, link, or reflect the module is `ErrorKind::Compile`.

### 7.2. The Global Session for Each Load

SLO-7. `Device::load_module` MUST create its own global session and release it before it returns. `slango` MUST keep no global session in a `static`, a `thread_local!`, or any hidden state, because CONC-4 forbids hidden library state. The simplest form also has no state to invalidate or to clean up at thread exit.

The cost is about 135 ms for each load (Section 3.6). That is acceptable for a program that loads a few modules once, and it matters for a program that loads many modules or reloads them often. Section 16, SOQ-1, keeps the remedy open.

### 7.3. Kernels and Pipelines

SLO-8. `Device::load_module` MUST create the shader module, the bind group layouts, and the compute pipeline of every compute entry point before it returns. It MUST fail as a whole when any of them fails, because every error of a module then surfaces at one call. Each creation costs under 1 ms (Section 3.6).

SLO-9. `slango` MUST create each shader module inside an error scope and MUST return a rejection by naga as `ErrorKind::UnsupportedShader`, with naga's text returned unchanged as the source error (Section 10), because the text names the construct that the user must change (Section 3.5). An error that wgpu reports to its uncaptured-error handler is lost to the caller.

The bind group layouts leave out every binding that the code does not use, which wgpu accepts (Section 3.3), so that the caller binds only what the kernel reads or writes. A kernel has one bind group layout for each set from 0 to the last set that holds a used binding, and a set between them with no used binding gets an empty layout. A parameter that the walk rejects fails the load with `ErrorKind::UnsupportedParameter` (Section 7.1). The load checks SLO-18 before it creates the shader module of a kernel, so it fails with `ErrorKind::MissingFeature` when the kernel has immediates and the device lacks `Features::IMMEDIATES`, or when the reflected size of the immediates exceeds the device's `max_immediate_size`. Section 10 states how the load maps the errors of wgpu.

`Module::kernel` looks a kernel up by its Slang name and returns a reference to it. For a name that is no compute entry point of the module, it returns `ErrorKind::InvalidArgument` with a text that lists no names. No method lists the kernels of a module.

## 8. Buffers

SLO-10. `Buffer<T>` MUST accept as `T` only the types of a sealed trait whose byte representation equals the std430 layout of the matching Slang element type without padding. Each such type MUST have a test that checks its size against the stride that reflection reports, because a Rust type with another layout would place every element after the first at a wrong offset (Section 3.4).

The sealed trait is `Element`. Step 4 implemented it for exactly these types, whose strides the test of SLO-10 checks against reflection (Section 3.3):

| Rust type | Slang type | Stride in bytes |
| --- | --- | --- |
| `f32`, `i32`, `u32` | `float`, `int`, `uint` | 4 |
| `[f32; 4]`, `[i32; 4]`, `[u32; 4]` | `float4`, `int4`, `uint4` | 16 |

The implementation adds further types only with the test of SLO-10. The size of a buffer is its length times the stride. Elements and the scalars of plain values (Section 9.2) are written little-endian, and elements are read the same way.

These rules settle the creation of a buffer and its contents:

- `Buffer::from_slice` creates a buffer that holds a copy of a slice, and `Buffer::zeroed` creates one of a given length whose bytes are zero.
- A buffer has the usages `STORAGE`, `COPY_SRC`, and `COPY_DST`.
- A buffer of length zero is allowed, and `read` returns it as an empty vector without work on the GPU. Step 5 decides what binding one means.
- No method writes new contents into an existing buffer.

SLO-11. `Buffer::read` MUST block until every dispatch submitted before it has finished and MUST return the elements as a `Vec<T>`, because a caller must not observe a partial result. A blocking read is the only synchronization that M3 offers. It copies the buffer to a staging buffer, maps it, and blocks the calling thread until the device has finished.

## 9. The Call

### 9.1. Arguments by Name

A kernel's parameters are named globals and named parameters of the entry point, with no positional order, so the call addresses each one by its parameter path. `Kernel::call` returns a builder. Its setter `set` records a plain value at a parameter path, its setter `bind` records a buffer for a parameter path, and both are infallible (API-6). This answers Q-18 of the architecture design for M3.

### 9.2. Marshalling

SLO-12. The trait through which `set` takes a value MUST be sealed (API-8). It MUST check the value against the reflected scalar type, shape, and size of the parameter before it writes any byte, because a wrong value otherwise reaches the GPU as wrong numbers with no error.

SLO-13. `slango` MUST write every plain value at its reflected offset, element by element for vectors, matrices, and arrays, and MUST NOT copy the bytes of a Rust struct as a whole, because the reflected layout differs from every Rust layout (Section 3.4).

The sealed trait is `Value`. Step 4 implemented it for `bool`, `i32`, `u32`, and `f32`, which stand for `bool`, `int`, `uint`, and `float`, and for arrays `[T; N]` of a value type, nested to any depth. A Rust array stands for any Slang structure of the same nesting:

- `[f32; 3]` stands for a vector `float3` or an array `float[3]`.
- `[[f32; 3]; 2]` stands for a matrix `float2x3`, whose first index is the row, an array of vectors `float3[2]`, or an array of arrays `float[2][3]`.

A value matches its parameter when the scalar types are equal and the lengths of the nesting are equal, from the outermost to the innermost, and the reflected size enters only when the value is written. A value that does not match gives `ErrorKind::InvalidArgument`, whose message names the parameter path, the Slang type, and the Rust type, and no byte is written. Each scalar is written at its reflected offset, and a `bool` as a 32-bit 0 or 1. A matrix is written row by row, as Section 3.4 states. Global plain values and plain values in constant buffers and parameter blocks go into uniform buffers that the call owns, and plain values of the entry point go into the immediates of the dispatch.

### 9.3. Validation Before Dispatch

SLO-14. The dispatch MUST check, before it submits any work, that every parameter path names a parameter of the kernel and that every used parameter is set. It MUST check that every value matches its parameter (SLO-12) and that the element stride of every bound buffer equals the reflected stride. It MUST check that every buffer belongs to the kernel's device and that every workgroup count is within the device's limit. It MUST return `ErrorKind::InvalidArgument` naming the parameter when a check fails, because wgpu reports these mistakes late, with less context, or not at all.

Each `Device` carries an identity that its clones share, and a buffer belongs to the device whose identity it holds. Two `Device` values built separately from the same wgpu device therefore count as different devices.

SLO-15. The dispatch MUST open an error scope around the work it records and submits, and MUST return each validation error that remains as `ErrorKind::Backend`, because an error that reaches wgpu's uncaptured-error handler is lost to the caller.

### 9.4. Dispatch and Results

`dispatch` takes the element counts of three axes and derives each workgroup count by rounding the element count divided by the thread group size up, so the kernel guards the threads past the end itself, as the illustration of Section 5.2 does. `dispatch_groups` takes the workgroup counts directly. A dispatch submits its work and returns. Its results come back through `Buffer::read` on a buffer that the caller bound (SLO-11).

## 10. Errors

SLO-16. `slango` MUST have one public error type as ERR-1 defines, with these kinds of `ErrorKind`, because callers match on the kind (ERR-1):

| Kind | Meaning |
| --- | --- |
| `Compile` | Slang failed to compile or link the module. `source()` returns the error of `slang-compiler` with its diagnostics |
| `NoAdapter` | wgpu found no adapter that matches the options |
| `DeviceRequest` | The adapter refused the device that the options request |
| `UnsupportedShader` | The backend rejected a kernel at load, as the list below states. The source error carries the backend's text |
| `UnsupportedParameter` | A kernel has a parameter that the walk rejects (Section 7.1) |
| `MissingFeature` | A kernel needs a device feature that the device lacks |
| `InvalidArgument` | A check of Section 9.3, including a value that does not match its parameter (Section 9.2), an unknown kernel name, a file that cannot be read as text, or a buffer larger than the device's limit or whose size overflows |
| `Backend` | wgpu reported a validation or out-of-memory error in an error scope at load, as the list below states, any error while a buffer is created or read, or a validation error at dispatch (SLO-15) |

The load creates every shader module, bind group layout, pipeline layout, and compute pipeline inside error scopes for validation, internal, and out-of-memory errors, so that no error reaches wgpu's uncaptured-error handler. It maps each error that a scope returns to a kind:

- Any error while the shader module is created gives `UnsupportedShader`.
- An internal error while the layouts or the pipeline are created gives `UnsupportedShader`, because a shader that the backend cannot translate is reported as an internal error.
- A validation or out-of-memory error while the layouts or the pipeline are created gives `Backend`.

The message names the kernel, and the backend's text is returned unchanged as the source error.

`NoAdapter` answers Q-22 of the architecture design.

## 11. Threading

SLO-17. `Device`, `Module`, `Kernel`, `Buffer<T>`, `Error`, and every options type MUST be `Send` and `Sync`, asserted in a test (CONC-1). `slango` MUST implement neither trait by hand, because the types hold only wgpu objects, which are `Send` and `Sync` (Section 3.2), and owned data, so the compiler derives both.

The call builder also holds only such data. `load_module` uses Slang only on the calling thread, and its Slang objects never leave the call (SLO-5), which keeps every rule of Section 10 of `docs/design/slang-compiler.md`. Several threads that load modules at once each create their own global session, which Slang supports (Section 3.5 of `docs/design/slang-compiler.md`). `slango` starts no thread (CONC-4).

## 12. Device Features

SLO-18. `Device::new` MUST request `Features::IMMEDIATES` with the adapter's maximum immediate size when the adapter offers the feature. The load MUST fail with `ErrorKind::MissingFeature` when a kernel has `uniform` parameters of its entry point and the device lacks the feature or a large enough immediate size, because without the feature naga rejects the module with a message about `entryPointParams` that does not name the cause (run, `Capability Capabilities(IMMEDIATES) is not supported`).

The default immediate size of wgpu is 0. The adapters of the spike and of the CI runners offer 128 or 256 bytes (run, Section 3.8). `Device::new` requests no other optional feature, such as the ones that `double`, `int64_t`, and `half` need (Section 3.5). `DeviceOptions` has no field for them, because a field of type `wgpu::Features` would name a wgpu type outside the module of SLO-4. A caller who needs another feature creates the wgpu device with it and passes the device and its queue to `slango::wgpu::from_device_and_queue`. A `Device` built that way uses the features that its device has.

## 13. Tests, CI, and the Example

### 13.1. Tests

`slango` has one integration-test binary (TEST-10). Every test that needs an adapter sits in a module named `gpu`, selects any adapter, and fails when it finds none (ARCH-22, ARCH-23). The tests of the reflection walk call Slang and need no adapter, so they sit in the module `layout::tests::ffi` in the default tier and check their snapshots with `insta`. They are unit tests inside the crate, not tests of the integration-test binary, because the snapshot is private and TEST-1 limits integration tests to the public API. The tests of marshalling and of the workgroup counts are pure and run under Miri. The tests of loading are `gpu` tests of the integration-test binary, in the module `gpu::load`, with the shader files that they load under `tests/slango/shaders/`. The test of `AdapterSelection::Software` is compiled out on macOS, because Metal has no software adapter (Section 3.8), and every other `gpu` test runs on all three platforms.

### 13.2. Continuous Integration

The `check` and `coverage` jobs on Linux install lavapipe from Mesa's package `mesa-vulkan-drivers` and run the `gpu` tier. The jobs on Windows and macOS run `just test-gpu`, so they run the same tests on WARP and on hosted Metal. The tier passed on all three platforms with the tests of steps 1 and 3, which create devices and load modules (Section 3.8). Whether dispatching a kernel passes on Windows and macOS stays open until step 5 of Section 14 (SOA-2). If a platform cannot run the tier, its job runs `just test` instead, as Section 8.1 of `docs/design/tooling.md` allows, and Q-21 of the architecture design is answered for that platform.

### 13.3. The Example

The example is a headless program in `examples/` that loads one Slang file with compute entry points, runs one kernel over a buffer with uniforms, prints the results, and checks them. Its test runs it in a `gpu` module, on lavapipe in CI (ARCH-21). The example needs no differentiable function, and the broadcasting case of example 2 waits for the milestone that adds broadcasting.

## 14. Implementation Order for M3

Each step leaves `just check` passing.

1. Add the crate skeleton, `Device` with `DeviceOptions`, adapter selection, the features of Section 12, `Error` and `ErrorKind`, the changes of Section 4.2, and the installation of lavapipe in the `check` job. Tests: the `Send` and `Sync` assertions of SLO-17, the `Display` text of each error kind, and a `gpu` test that creates a device and checks that it has immediates when the adapter offers them. Done on 2026-11-11, with lavapipe also installed in the `coverage` job, and all six CI jobs passed with it (Section 3.8).
2. Add the layout snapshot and the walk of Section 3.3. Tests: `ffi` snapshot tests for global plain values, constant buffers, parameter blocks, nested parameter blocks, parameters of the entry point, system values, unused parameters, and a texture that the walk reports as unsupported. Done on 2026-11-11, with the `ffi` tests as unit tests (Section 13.1) and the snapshot as Section 7.1 describes.
3. Add `Module`, `Kernel`, and `load_module` with the SPIR-V of each entry point, the error scope of SLO-9, and the layouts and pipelines. Tests: `gpu` tests that load the kernels of Section 3.2, a module that naga rejects, which returns `UnsupportedShader` with naga's text (TEST-6), and a kernel with uniforms of its entry point on a device without immediates, which returns `MissingFeature`. Done on 2026-11-11, with the choices of Sections 7.1 and 7.3 and the mapping of errors of Section 10. The rejected module calls `frexp`, because a 16-bit integer loads on a device with the matching feature (Section 3.5), and the tests passed on all three CI runners (Section 3.8).
4. Add `Buffer<T>` with upload and `read`, and the marshalling of plain values. Tests: the stride test of SLO-10, pure tests that write the values of Section 3.4 at their reflected offsets, and a `gpu` round trip of a buffer. Done on 2026-11-11, with the element and value types and the rules of Sections 8 and 9.2, the identity of a device of Section 9.3, and the matrix rule of Section 3.4. The `gpu` tests passed on all three CI runners, and the writing of plain values is tested against reflected offsets only, until step 5 dispatches.
5. Add the call builder, the checks of Section 9.3, and the dispatch. Tests: one test of each check of SLO-14, pure tests of the workgroup counts, and `gpu` tests that dispatch the kernels of Section 3.2 and assert every value.
6. Add the example of Section 13.3 with its test and the crate documentation, and update the documents that M3 makes false. Tests: the example's `gpu` test.

## 15. Assumptions to Confirm

Each row is a statement that this design relies on and that the spike did not establish. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in Section 3 and in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SOA-2 | SPIR-V through naga, with immediates, dispatches on Direct3D 12 with WARP on the Windows runner and on Metal on the macOS runner | Section 13.2, architecture A-17, A-18, and Q-21 | Step 5 of Section 14, by the first CI run that dispatches |
| SOA-3 | On the validated path, wgpu adds bounds checks to every buffer access of a kernel | Section 17 | Step 5 of Section 14, by reading the bounds-check policy of the shader translation of wgpu 30 in its source |
Of the assumptions of the architecture design that name this document, Section 3.3 to Section 3.5 confirm A-12, Section 3.7 confirms A-13, and Section 3.2 confirms A-11 for M3. Section 3.7 confirms A-10 in part, which covers the first three patterns of Section 8.3 of that design and the description of `GpuFuture`, with "replacing" read as "building an experimental replacement". It did not cover typed shader interfaces generated from reflection, so A-10 stays for that pattern. A-9 holds as far as SlangPy's documentation was read (Section 3.7), and M3 does not depend on it. SOA-2 carries what stays open of A-17 and A-18, which is dispatch only, because WARP and Metal each create a device with immediates and load modules (Section 3.8).

## 16. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it in the milestone of the last column, and the change that answers it removes the row.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SOQ-1 | Should a caller-held compiler object or a cache of global sessions replace the global session for each load of SLO-7, once loading many modules or hot reload makes its cost of about 135 ms matter? | This document | The milestone that measures the cost in a real use, at the latest the hot reload of M4 |
| SOQ-3 | How large must a kernel be before naga's parsing at load or lavapipe's first dispatch takes long enough to matter? | This document | The milestone that first loads kernels larger than those of the example |

Of the questions of the architecture design that name this document, Section 18.5 answers Q-16 for `slango`, Section 6 answers Q-17, Section 9.1 answers Q-18 for M3, SLO-2 answers Q-19 for `slango` in M3, SLO-1 answers Q-20, Section 10 answers Q-22, and Section 7.1 answers Q-31 and makes Q-30 unnecessary for M3. Q-21 is answered for Linux. On Windows and macOS, a device with immediates is created and modules load (Section 3.8), and SOA-2 carries the rest.

`slango` waits on wgpu's futures with `pollster::block_on`, because wgpu 30 offers no blocking form of `Instance::request_adapter`, `Instance::enumerate_adapters`, or `Adapter::request_device`.

## 17. Security Considerations

The threat model treats the shader source, the file paths, and the options that a program passes as trusted input from the program itself, as Hard Rule 14 of `AGENTS.md` states, and covers mistakes in the arguments of a call, which Section 9.3 checks. `slango` has no unsafe code (SLO-2) and no build script. Its memory safety rests on the safe APIs of wgpu and `slang-compiler`, and the safety of the GPU's memory accesses rests on naga's validation and on wgpu's bounds checks on the validated path (SOA-3). Slang is linked as `docs/design/slang-compiler-sys.md` specifies, and `Cargo.lock` and `cargo-deny` fix and check the versions of wgpu's dependency tree (Section 4.2).

These attacks are out of scope, because a program that loads them has already chosen to run them: a malicious shader, a shader that reaches a defect of Slang, naga, wgpu, or a driver, and a kernel that never finishes, which blocks `Buffer::read`.

The residual risk is a defect of naga, wgpu, or a GPU driver that lets a kernel read or write outside its buffers, and a crash that a defect of Slang causes, which Section 3.12 of `docs/design/slang-compiler.md` records.

## 18. Operational Considerations

### 18.1. Cargo Features and Environment Variables

`slango` has no Cargo features in M3. It reads no environment variable itself. Its build needs `VULKAN_SDK`, and its programs need the Slang shared library on the library path, as `slang-compiler-sys` requires (Section 19.4 of `docs/design/slang-compiler-sys.md`).

### 18.2. Platforms and Minimum Supported Rust Version

The crate runs on x86_64 Linux with Vulkan, which the spike verified on a software and a hardware adapter. On Windows with Direct3D 12 and on macOS with Metal, it creates a device and loads modules in CI (Section 3.8), and dispatching stays unverified until SOA-2 is confirmed. wgpu 30.0.1 declares `rust-version` 1.87, below the workspace's minimum supported Rust version (MSRV) of 1.99.

### 18.3. Failure Modes

| Failure | Reported as | Recovery |
| --- | --- | --- |
| No adapter exists, such as on a machine without a GPU driver or software adapter | `NoAdapter` | Install a driver or lavapipe |
| The Slang shared library is not on the library path | The program stops before `main`, as `slang-compiler-sys` describes | Run through the recipes, or set the library path |
| The module does not compile | `Compile`, with Slang's diagnostics | Fix the shader |
| naga or the backend rejects a kernel | `UnsupportedShader`, with the backend's text as the source error (Section 10) | Avoid the construct (Section 3.5) |
| A kernel has a parameter that the walk rejects (Section 7.1) | `UnsupportedParameter` | Wait for the milestone that supports it |
| A kernel needs a feature the device lacks | `MissingFeature` | Create the wgpu device with the feature and pass it to `slango::wgpu::from_device_and_queue` (Section 12), or use another adapter |
| A call has a wrong name, value, or buffer | `InvalidArgument`, naming the parameter | Fix the call |
| A buffer would be larger than the device allows | `InvalidArgument` | Create a smaller buffer |
| wgpu reports a validation or out-of-memory error (Section 10), or an error while a buffer is created or read | `Backend` | Report a validation error as a defect of `slango` |

### 18.4. Costs

Each load of a module costs about 135 ms for its global session, plus 8 to 11 ms for a small module (Section 3.6, SOQ-1).

### 18.5. Logs

`slango` emits no logs in M3 and has no logging dependency, as `slang-compiler` does. This answers Q-16 of the architecture design for `slango`.

## 19. References

### 19.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 19.2. Informative References

- [SLANG-SPIRV] The Slang project, "SPIR-V specific functionalities", `docs/user-guide/a2-01-spirv-target-specific.md` in Slang's source tree at the pinned commit, read on 2026-11-11.
- [SLANGPY] The Slang project, "SlangPy documentation", <https://slangpy.shader-slang.org/en/latest/>, read on 2026-11-11.
- [VULKANO-2540] Pull request 2540 of the vulkano-rs/vulkano repository on GitHub, <https://github.com/vulkano-rs/vulkano/pull/2540>, read on 2026-11-11.
- [WGPU-CHANGELOG] The wgpu project, "Changelog", <https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/CHANGELOG.md>, read on 2026-11-11.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| SLO-1 | 4.1 | Depend on `slang-compiler`, wgpu 30 with `spirv`, and `pollster`, and on no other crate. |
| SLO-2 | 4.1 | No unsafe code in M3, and every shader through the validated path. |
| SLO-3 | 5.1 | `Device`, `Module`, `Kernel`, and `Buffer<T>`, cheap to clone, holding their parents, created through options. |
| SLO-4 | 6 | wgpu types appear only in one public module. |
| SLO-5 | 7.1 | `load_module` compiles on the calling thread and releases every Slang object before it returns. |
| SLO-6 | 7.1 | The private layout snapshot holds only owned data and follows the walk rules of Section 3.3. |
| SLO-7 | 7.2 | A new global session for each load, and no hidden Slang state. |
| SLO-8 | 7.3 | Every shader module, layout, and pipeline is created at load, and any failure fails the load. |
| SLO-9 | 7.3 | A naga rejection is caught in an error scope and returned as `UnsupportedShader` with naga's text. |
| SLO-10 | 8 | Buffer element types come from a sealed trait, each with a test of its stride. |
| SLO-11 | 8 | `Buffer::read` blocks until earlier dispatches finish and returns the elements. |
| SLO-12 | 9.2 | The value trait is sealed and checks each value against its reflected type before writing. |
| SLO-13 | 9.2 | Plain values are written at reflected offsets, never as Rust struct bytes. |
| SLO-14 | 9.3 | Names, presence, values, strides, devices, and workgroup limits are checked before dispatch. |
| SLO-15 | 9.3 | Remaining wgpu validation errors are caught in an error scope and returned as `Backend`. |
| SLO-16 | 10 | One `Error` type with the eight kinds of Section 10. |
| SLO-17 | 11 | Every public type is `Send` and `Sync` without a manual implementation, asserted in a test. |
| SLO-18 | 12 | Request immediates when offered, and fail the load with `MissingFeature` when a kernel needs them. |

## Change Log

- 2026-11-11: Recorded step 4 of Section 14, which added `Buffer<T>` and the marshalling of plain values, in Sections 1.6 and 14, with the element types and the rules of buffers in Section 8, the value types and their check in Section 9.2, the identity of a device in Section 9.3, and the errors of buffers and values in Sections 10 and 18.3. Recorded in Section 3.4 the row-major matrix layout and the offsets of its rows, settled in Section 7.1 that marshalling needs no matrix layout mode, recorded in Section 3.2 that wgpu 30.0.1 compares devices of two requests as equal, confirmed SOA-4 in Section 3.3 and removed its row from Section 15, and defined the mark (run) for step 4 in Section 3.1.
- 2026-11-11: Recorded step 3 of Section 14, which added `Module`, `Kernel`, and the load of a module from a file or from source text with a compute pipeline for each kernel, in Sections 1.6, 7.1, 7.3, 13.1, and 14, with the choices that the design left open in Sections 7.1 and 7.3 and the mapping of wgpu's errors at load in Sections 10 and 18.3. Recorded in Section 3.8 that modules load on all three CI runners, narrowed SOA-2 to dispatch in Sections 13.2, 15, 16, and 18.2, moved 16-bit integers in Section 3.5 to the constructs that need a device feature, and defined the mark (run) for step 3 in Section 3.1.
- 2026-11-11: Recorded step 2 of Section 14, which added the private layout snapshot and its walk, in Sections 1.6, 7.1, 13.1, and 14, with the choices that the design left open and the supported and rejected parameters in Section 7.1. Recorded in Sections 3.3 and 3.4 that a system value has a uniform size of 0 and that the immediates round their size up to their alignment, and defined the mark (run) for step 2 in Section 3.1. Made the rows of Sections 10 and 18.3 for `UnsupportedParameter` cite Section 7.1.
- 2026-11-11: Recorded step 1 of Section 14, which added `Device`, `DeviceOptions`, `AdapterSelection`, the error type, and the module `slango::wgpu`, in Sections 1.6, 5.1, 13.1, and 14, added Section 3.8 with the adapters of the CI runners, recorded the licence changes in Section 4.2, corrected in Sections 12 and 18.3 how a caller requests another device feature, updated Sections 13.2 and 18.2 for the `gpu` tier in CI, narrowed SOA-2 to loading and dispatch, and answered SOQ-2 through `pollster` in SLO-1 and Appendix A.
- 2026-11-11: First draft, written after the spike of ARCH-18 as the first task of M3.
