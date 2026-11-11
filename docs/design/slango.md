# `slango` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-11 |
| Applies to | `crates/slango`, the compute kernel example, and the windowed renderer example |

## Abstract

This document specifies the first milestone of `slango`, the high-level library for running Slang code on a GPU from Rust, and the compute kernel example that milestone M3 delivers with it. In M3, `slango` calls existing compute entry points of a Slang module on wgpu, with bindings built from Slang's reflection alone. The document starts from the facts that a spike established about Slang's SPIR-V on wgpu's validated shader path, including the mapping from reflection to wgpu bindings and the constructs that the path rejects. On those facts it defines four public types, the one public module that exposes wgpu, the loading of a module into an owned layout snapshot that frees every Slang object, the call by parameter name with its checks before dispatch, the errors, the threading, and the device features. It also specifies the tests, the example, the dependency changes, and the order of the changes that implement M3. It then specifies milestone M4, rendering. From the facts that a second spike established about vertex and fragment entry points on the same path, it defines render pipelines built from existing entry points, textures, samplers, vertex and index buffers, and a draw by parameter name with its checks, all without unsafe code. It also specifies the offscreen tests, the windowed renderer example, and the order of the changes of M4, and it defers a shader target configurable per backend. It ends with the assumptions to confirm and the open questions.

## 1. Introduction

### 1.1. Problem

`slang-compiler` turns Slang source into SPIR-V and reports the bindings of each parameter, but running that code on a GPU takes much more. A program must open a device, build bind group layouts and a pipeline that match the shader exactly, lay out every plain value by the shader's rules, bind buffers, choose a dispatch size, and copy results back. A mistake in any of these steps fails at run time with a message about descriptor sets and offsets, or computes wrong numbers without any error.

### 1.2. Purpose

`slango` lets a Rust program load a Slang module, look up a compute kernel, set its parameters by name, dispatch it over a number of elements, and read the results, with every binding derived from reflection. For M4, it also lets the program draw with a vertex and a fragment entry point into a texture, with every binding and vertex attribute derived from reflection. This document implements Section 8 and milestones M3 and M4 (Sections 15.6 and 15.7) of `docs/design/architecture.md`, meets ARCH-15 to ARCH-19, and answers the questions and assumptions of that design that name this document, as Section 15 and Section 16 of this document record.

### 1.3. Audience

The primary readers are the coding agents that implement M3 and M4 and the reviewers who check their changes. A reader is expected to know Rust but not GPU programming. Section 2.1 defines the terms that such a reader needs.

### 1.4. Scope

This document covers the compute entry points that a Slang module already declares with `[shader("compute")]`, compiled to SPIR-V and loaded through wgpu's validated path, with buffers, constant buffers, parameter blocks, and plain `uniform` values as parameters. It covers the public types, the backend boundary, loading, the call, errors, threading, device features, tests, continuous integration (CI), the example, and the dependency changes.

For M4, it also covers render pipelines built from the vertex and fragment entry points that a module already declares with `[shader("vertex")]` and `[shader("fragment")]`, drawing into a texture. Vertex attributes, uniforms, textures, and samplers are bound by name from reflection, as M3 binds the parameters of a kernel, and textures and samplers become parameters of kernels too. It covers the windowed renderer example as well. The rendering design is in Sections 3.9, 5.3, 5.4, 7.4, 7.5, 8.1, 8.2, 9.5, 13.4, and 13.5, and in steps 7 to 12 of Section 14, and it extends the other sections where they name M4.

### 1.5. Non-Goals

This document designs nothing beyond M3 and M4. These belong to later milestones: kernels generated around a plain Slang function with a positional call, broadcasting over array arguments, a tensor type with a shape, differentiation driven from Rust, and the unchecked shader passthrough path.

M4 leaves out generated shaders, stages other than vertex, fragment, and compute, such as geometry, tessellation, and mesh stages, multisampling, storage textures, texture arrays, cube maps, mip-map generation, several draws in one render pass, and render bundles. `slango` offers no window, no surface configuration, and no frame loop, which belong to the caller (Section 5.3). Hot reload is no API of `slango`, and the windowed renderer example does it with the existing load (Section 13.5).

A shader target configurable per backend is deferred out of M4 with no milestone assigned. Slang's SPIR-V loads and runs through wgpu's validated path on Vulkan, Direct3D 12, and Metal (Section 3.8), and a second target would need the passthrough path and unsafe code (SLO-2).

### 1.6. Current State

Step 1 of Section 14 is done, on 2026-11-11. `crates/slango` holds `Device` with `DeviceOptions` and `AdapterSelection` (Section 5.1), the error type `Error` with `ErrorKind` and `Result` (Section 10), and the public module `slango::wgpu` (Section 6), which builds a `Device` from a wgpu device and queue and returns them. Its first tests of the `gpu` tier create devices on every CI runner (Sections 3.8 and 13). Step 2 is done, on 2026-11-11. The private module `layout` holds the layout snapshot and the walk of Section 7.1, and a private helper that compiles a module from source text into one snapshot for each compute entry point, with unit tests that Miri runs and `ffi` unit tests that call Slang (Section 13.1). Until step 3, only its tests called it. Step 3 is done, on 2026-11-11. `Device::load_module` loads a Slang file and `Device::load_module_from_source` loads source text, each into a `Module` with one `Kernel` for each compute entry point, and the load creates the compute pipeline of each kernel (Sections 7.1 and 7.3). `Module::kernel` looks a kernel up by name, and `Kernel::name` and `Kernel::thread_group_size` return its name and the size of its thread group. Step 4 is done, on 2026-11-11. `Buffer::from_slice` and `Buffer::zeroed` create a `Buffer<T>` on a device, `Buffer::read` copies its elements back, and `Buffer::len` and `Buffer::is_empty` return its length (Section 8). The sealed traits `Element` and `Value` list the element types of a buffer and the Rust types of plain values (Sections 8 and 9.2). Step 5 is done, on 2026-11-11. `Kernel::call` returns a `Call`, on which `Call::set` sets a plain value and `Call::bind` binds a buffer, each by parameter path, and `Call::dispatch` and `Call::dispatch_groups` check every argument and run the kernel (Section 9). Step 6 is done, on 2026-11-11. It added the example `compute-kernel` of Section 13.3 and completed the crate-level documentation. Every step of Section 14 is done, so M3 is complete on 2026-11-11. A user can create a device, load a Slang module from a file or from source text, create buffers, set the parameters of a compute kernel by name, dispatch it, and read its results, and the example shows that whole flow. The crate has no unsafe code, because it loads every shader through wgpu's validated SPIR-V path (SLO-2). The spike that ARCH-18 requires ran on 2026-11-11 as a scratch program that the repository does not keep, and Section 3 records what it established. Section 14 gives the order of the changes that implement M3.

The design of M4, rendering, exists as a draft dated 2026-11-11, in the sections that Section 1.4 lists. A second spike, for rendering, ran on 2026-11-11 as a scratch program that the repository does not keep, and Section 3.9 records what it established. Steps 7 to 12 of Section 14 give the order of the changes of M4. Step 7 is done, on 2026-11-11. The private layout snapshot records each vertex and fragment entry point with its SPIR-V, its bindings, and, for a vertex entry point, its vertex attributes, and the walk accepts textures and samplers (Sections 7.4 and 8.1). No public item changed, and a load shows one difference, in the parameter that its error may name (Section 7.4). Step 8 is done, on 2026-11-11. It added `Texture` with `TextureFormat`, `Sampler` with `SamplerOptions`, `FilterMode`, and `AddressMode`, the sealed trait `VertexElement`, the functions `texture` and `texture_from_view` of `slango::wgpu`, and the setters `Call::texture` and `Call::sampler` (Sections 8.1, 8.2, and 9.1). A user can create textures and samplers, read a texture back, and pass both to a compute kernel. Rendering cannot be used yet. Steps 9 to 12 are not done, and the next change is step 9.

### 1.7. Defects of Third-Party Components and Trusted Input

`slango` follows Hard Rule 14 of `AGENTS.md` and Section 1.7 of `docs/design/slang-compiler.md`, and applies the same stance to naga and wgpu. Shader source and file paths are trusted input. When naga rejects a module, `slango` reports naga's own text and does not screen modules for the constructs of Sections 3.5 and 3.9 in advance. The checks of the arguments of a call stay, because a safe interface needs them (Section 9.3). A defect of Slang that M3 finds is recorded in Section 3.12 of `docs/design/slang-compiler.md`, not here.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

Section 2.1 of `docs/design/architecture.md` defines adapter, buffer, compute dispatch, compute kernel, device, lavapipe, marshalling, shader passthrough, software adapter, spike, SPIR-V, and wgpu. Section 2.1 of `docs/design/slang-compiler.md` defines descriptor set, layout, parameter category, thread group, type layout, and variable layout. This document adds these terms:

- bind group: The wgpu name for the set of bindings that a dispatch uses together. One bind group fills one descriptor set, and its bind group layout states the kind of each binding.
- binding: A numbered slot in a descriptor set through which a kernel reaches a buffer.
- colour target: The texture that a render pass draws its colour output into.
- depth target: A texture of depth values that a render pass tests and writes, so that nearer triangles hide farther ones.
- draw: One GPU command that runs a render pipeline over a number of vertices and instances.
- element count: The number of threads a caller asks a dispatch to run along each of three axes.
- error scope: A wgpu mechanism that collects the validation errors of the calls made while it is open and returns them as values.
- fragment entry point: An entry point marked `[shader("fragment")]`, which computes the colour of each pixel that a triangle covers.
- immediate: The wgpu 30 name for a push constant.
- index buffer: A buffer of vertex numbers that selects the vertices of each triangle, so that triangles can share vertices.
- instance: One repetition of the vertices of a draw, which reads its own values from the vertex attributes that step per instance.
- kernel: In this document, one compute entry point of a loaded module, ready to dispatch.
- layout snapshot: The owned Rust data that `slango` copies from Slang's reflection at load time and from which it builds every wgpu layout (Section 7.1).
- naga: The shader translator inside wgpu, which parses SPIR-V, validates it, and translates it for the device's graphics programming interface.
- parameter path: The name of a plain value or a buffer as the call sets it, with a full stop between the name of a constant buffer or parameter block and the name of a field, such as `params.scale`.
- plain value: A scalar, vector, matrix, array, or structure value that a kernel reads from a uniform buffer or a push constant, as opposed to a buffer that it reaches through a binding.
- push constant: A small block of plain values passed with a dispatch instead of through a buffer.
- render pass: The wgpu recording of draws into a colour target and an optional depth target, with the operations that keep or clear each target first.
- sampler: A GPU object that states how a shader reads a texture between and beyond its pixels, through a filter mode and an address mode.
- std140, std430: The two memory layout rules of Vulkan that Slang's SPIR-V uses by default, std140 for uniform buffers and std430 for storage buffers and push constants.
- step mode: Whether a vertex attribute advances once per vertex or once per instance.
- storage buffer: A buffer that a kernel reads and, unless it is read-only, writes.
- texture: A two-dimensional image in GPU memory that a shader samples or a render pass draws into.
- uniform buffer: A read-only buffer of plain values. Slang calls it a constant buffer.
- validated path: The loading of SPIR-V through `ShaderSource::SpirV`, in which naga parses and validates the module before the device sees it.
- varying: A value that the vertex entry point outputs and the fragment entry point receives, interpolated across each triangle.
- vertex attribute: One input of the vertex entry point that a buffer provides for each vertex or instance, such as a position.
- vertex entry point: An entry point marked `[shader("vertex")]`, which computes the position of each vertex.
- workgroup: The wgpu name for a thread group.

## 3. Facts From the Spike

This section is non-normative. It states each fact that a later section relies on, once, and says how it is known.

### 3.1. Sources and Evidence

Each fact carries one of these marks:

- (run): Verified on 2026-11-11 by a scratch program on x86_64 Linux, built in debug mode, with `slang-compiler` of the repository at commit `f4011b7`, wgpu 30.0.1 with the feature `spirv`, and naga 30.0.1. It ran on two Vulkan adapters, lavapipe of Mesa 25.0.7 and an NVIDIA GeForce RTX 4070 Laptop GPU, with identical results on both. In Section 3.8 alone, it means verified on 2026-11-11 by the `gpu` tests of step 1 of Section 14 on GitHub's hosted runners. In a fact that names step 2, it means verified on 2026-11-11 by the `ffi` tests of step 2 of Section 14 on x86_64 Linux. In a fact that names step 3, it means verified on 2026-11-11 by the work of step 3 of Section 14, in CI run 42952435667 or, where the fact says so, locally on the two Vulkan adapters of the development machine and not in CI. In a fact that names step 4, it means verified on 2026-11-11 by the work of step 4 of Section 14 on x86_64 Linux: a fact about layout through Slang's reflection and the SPIR-V decorations that the `slangc` of the pinned Vulkan SDK printed, and not on a GPU, and a fact about wgpu by creating devices on the development machine. In a fact that names step 5, it means verified on 2026-11-11 by the `gpu` tests of step 5 of Section 14, which dispatch kernels and read their results back, in CI run 42961810362 on the three adapters of Section 3.8 and locally on the two Vulkan adapters of the development machine. In a fact that names step 6, it means verified on 2026-11-11 by the tests of the example of step 6 of Section 14, in CI run 42966911977 on the three adapters of Section 3.8. In a fact that names step 7, it means verified on 2026-11-11 by the work of step 7 of Section 14 on x86_64 Linux, through Slang's reflection and the SPIR-V decorations that the `slangc` of the pinned Vulkan SDK printed, or, for a cost, by one series of measurements of a release build on the development machine. In a fact that names step 8, it means verified on 2026-11-11 by the `gpu` tests of step 8 of Section 14, which create and read textures and run compute kernels that sample them, in CI run 42996353428 on the three adapters of Section 3.8 and locally on the two Vulkan adapters of the development machine. In Section 3.9 alone, except in a fact that names step 7 or step 8, it means verified on 2026-11-11 by the spike of M4, a second scratch program with the same versions of wgpu and naga on the same two Vulkan adapters, built against `slang-compiler` of the repository at commit `e561af6`, with identical results on both adapters.
- (read): Read in a source on 2026-11-11, which the fact names.
- (inferred): Concluded from other facts and not verified. Every inferred fact that the design depends on is also a row of Section 15.

### 3.2. What Ran

Slang compiled three kernels to SPIR-V with `slang-compiler`, the scratch program built every bind group layout and pipeline from reflection alone, dispatched each kernel, read the results back, and every value matched (run). The kernels covered a read-write structured buffer with a global `uniform`, a read-only and a read-write structured buffer with a `ConstantBuffer`, a `ParameterBlock` holding a buffer, and two entry-point `uniform` parameters, and a kernel with an unused buffer and an unused uniform (run). Through the validated path, naga also accepted control flow of every kind, generics with interface constraints, `[Differentiable]` functions with `bwd_diff` and `fwd_diff`, atomics, matrices, uniform arrays, `static const` tables, `inout` and `out` parameters, `bool` fields, `cbuffer`, and nested parameter blocks (run). No unsafe code was needed (run). The passthrough path also ran every kernel on both adapters, and it needs unsafe code and `Features::PASSTHROUGH_SHADERS` (run). wgpu 30's `Device`, `Queue`, `Buffer`, `ComputePipeline`, and `BindGroup` are `Send` and `Sync` (run), and all wgpu types are `Clone` through internal reference counting since wgpu 24 [WGPU-CHANGELOG] (read). In wgpu 30.0.1, two devices from two device requests compared equal with `==` (run on the development machine on 2026-11-11, step 4, with no test in the repository). In wgpu 30.0.1, `Device::create_shader_module` passes `ShaderRuntimeChecks::checked()`, whose `bounds_checks` is `true`, and the documentation of that field promises that a shader can only read or write the accessible region of the buffers that a bind group binds, through checks that wgpu injects when the platform cannot enforce them itself (read, source of wgpu 30.0.1 and wgpu-types 30.0.1 on 2026-11-11). `slango` creates every shader module through that method. Error scopes are per thread in wgpu 30.0.1, because `Device::push_error_scope` pushes onto the device's thread-local stack of error scopes (read, source of wgpu 30.0.1 on 2026-11-11).

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
| `Texture2D`, `SamplerState`, `RWTexture2D` | Binding types `Texture`, `Sampler`, `MutableTexture` | Not built in M3. M4 builds the first two (Sections 3.9 and 8.1), and the walk keeps rejecting the third (Section 7.1) |

For each element type of `Buffer<T>`, the reflected stride of a structured buffer of the matching Slang type equals the stride that `slango` uses for `T`, so the size of every buffer, its length times that stride, is a multiple of the stride (run, step 4).

### 3.4. Layout of Plain Values

Slang's default layout puts the fields of `struct Params { float scale; float3 offset; uint count; float2 pair; }` in a constant buffer at offsets 0, 16, 28, and 32, with size 48 (run). `float c[5]` in a constant buffer takes 80 bytes, with a stride of 16 (run). `float4x4`, `float3x3`, `float2x3`, and `float3` in sequence take offsets 0, 64, 112, and 144, with size 160 (run). A `bool` takes 4 bytes (run). As `uniform` parameters of an entry point, `float`, `float3`, `float4x4`, a structure of a `float` and a `uint`, and `bool` take offsets 0, 16, 32, 96, 100, and 104 in the immediates. The last value ends at 108, and the block has size 112, because std430 rounds the size of the block up to its alignment of 16 (run, step 2). These offsets differ from the layout of any Rust struct with the same fields, so `slango` writes every plain value at its reflected offset and never relies on the layout of a Rust struct (SLO-13).

Slang's default matrix layout is row-major, which `defaultMatrixLayoutMode` in `slang.h` states, and `slango` sets no matrix layout (run, step 4). In that layout, a matrix of R rows and C columns is R rows, each starting at a multiple of the matrix stride. Row r starts at the value's offset plus r times its extent divided by R, and column c of a row follows at 4 bytes times c (run, step 4). The extent is the reflected size of the value, or the array stride for a matrix inside an array. It is R times 16 bytes in a uniform buffer and R times 8 or 16 bytes in the immediates, for every R and C from 2 to 4 (run, step 4). The `slangc` command line defaults to the other layout, so an experiment with it must pass `-matrix-layout-row-major` to match what `slango` compiles (run, step 4). The rule held on every adapter tested: a `float2x3` and a `float3x2` in the immediates and a `float2x3` in a constant buffer, each read element by element by the kernel, came back in row order (run, step 5).

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

lavapipe is the only adapter of the Linux runner, so the default selection also finds it there (run). Metal offers no software adapter: on the macOS runner, `AdapterSelection::Software` returned `ErrorKind::NoAdapter` in the earlier run 42941845864 (run). In CI run 42952435667, in which all six jobs passed, the `gpu` tests loaded modules on all three adapters (run, step 3). Kernels with immediates, a global uniform, a constant buffer, a parameter block, an import, and two kernels in one module reached the creation of their compute pipelines, and a kernel that calls `frexp` failed with the same backend text on all three. In CI run 42961810362, in which all six jobs passed, the `gpu` tests dispatched kernels on all three adapters and every result was correct, with immediates, global uniforms, constant buffers, parameter blocks, and every element type of `Buffer<T>` (run, step 5). One buffer bound to a read-only and to a read-write parameter of one call failed wgpu's validation on every adapter tested (run, step 5). In CI run 42966911977, in which all six jobs passed, the example of Section 13.3 ran and passed its tests on lavapipe, WARP, and Metal (run, step 6).

### 3.9. Facts for Rendering

This section is non-normative. Every fact in it is (run), as Section 3.1 defines the mark for this section, unless it says otherwise.

A vertex and a fragment entry point, compiled as Section 7.1 compiles a kernel, into one SPIR-V module for each entry point with `vulkan_use_entry_point_name(Toggle::On)`, loaded through the validated path and drew the expected pixels into an `Rgba8Unorm` texture that was read back. No unsafe code and no other session or target option was needed. `EntryPointLayout::stage` returns `Stage::Vertex` and `Stage::Fragment`. Every fact that the spike needed came through the safe API of `slang-compiler`.

This table states what reflection reports for the inputs and resources of render entry points, and what wgpu needs for each:

| Slang declaration | What reflection reports | wgpu |
| --- | --- | --- |
| A field of a structure parameter of the vertex entry point, such as `float3 position : POSITION` in `VIn input` | Through `EntryPointLayout::parameter_layouts` and the fields of the parameter's type: the name, the semantic, the scalar type and element count, and the location as the sum of `offset(VaryingInput)` along the path. The locations equal those in the SPIR-V and from `slangc` | A vertex attribute at that location, with the format of the scalar type and count, such as `Float32x3`. Reflection gives no stride, offset, or step mode |
| A vertex attribute that the code never reads | Still listed at its location. `Metadata::is_parameter_location_used` with `ParameterCategory::VaryingInput`, space 0, and the location returns `false`. The SPIR-V drops it and renumbers nothing | Accepted with or without the attribute |
| Attributes that step per instance, such as `float2 offset : INSTANCE_OFFSET`, in the same input structure | Listed like the other attributes, so reflection does not say which attributes step per instance | Correct from a second buffer with `VertexStepMode::Instance`. The step mode is part of the render pipeline (read, `VertexBufferLayout::step_mode` in wgpu 30.0.1) |
| `SV_Position` as a vertex output or a fragment input | No category and no location | Nothing |
| Varyings | Locations in declaration order, within each entry point's own structure | Matched by location. When two structures declare the same varyings in another order, `create_render_pipeline` fails with a validation error that starts `Error matching ShaderStages(FRAGMENT) shader requirements against the pipeline` |
| A global `uniform`, a `ConstantBuffer`, and a `ParameterBlock`, used by two entry points | The bindings of Section 3.3, unchanged. The metadata of each entry point reports which bindings that stage uses | One layout entry for each binding, visible to the stages that use it. A narrower visibility fails `create_render_pipeline` with `Visibility flags don't include the shader stage`, and a wider one is accepted |
| `uniform` parameters of the vertex and of the fragment entry point | One push constant block for each entry point, each at byte offset 0 | The two blocks alias: the fragment stage read the bytes written for the vertex stage. `RenderPass::set_immediates` takes no stage (read, wgpu 30.0.1) |
| `Texture2D<float4>` | `TypeKind::Resource`, base shape `Texture2d`, not combined, access `Read`, result type `vector<float,4>`, category DTS | A texture entry with `sample_type: Float { filterable: true }` and `view_dimension: D2`, which the spike chose and which rendered correctly |
| `SamplerState` | `TypeKind::SamplerState`, category DTS, at its own binding | A `Filtering` sampler entry |
| `Sampler2D` | `TypeKind::Resource`, combined, with one binding | Rejected by naga, as the next table shows |

On Vulkan, the coordinate conventions need no flip from Slang or `slango` (SOA-5 covers drawing on the other backends). The y axis of the vertex position points up, the texture coordinates (0, 0) sample the top-left pixel of a texture, and `SV_Position` in the fragment stage has pixel centres at 0.5 with y pointing down. Face culling and the order of the vertices of a triangle were not tested. A `Depth32Float` depth target cleared to 1.0, with the comparison `Less` and depth writes, hid a farther triangle drawn after a nearer one.

Compute kernels that sample textures with `SampleLevel` behave alike on every adapter that Section 3.1 names for step 8, including WARP on Direct3D 12 and the Apple paravirtual device on Metal (run, step 8):

- The texture coordinates (0, 0) sample the top-left pixel, with no flipped axis.
- A kernel reads a pixel of `Bgra8Unorm` or `Bgra8UnormSrgb` as red, green, blue, and alpha.
- A channel of 128 in an sRGB format sampled to between 0.2158 and 0.2167 on the five adapters, within 1/255 of the 0.2159 that the sRGB formula gives, and to 128/255 in the matching non-sRGB format (run, step 8).
- Linear filtering gives the interpolated value within the tolerance of 0.01 that the test states.
- `ClampToEdge`, `Repeat`, and `MirrorRepeat` give the expected pixels at coordinates outside the range 0 to 1.
- All five formats of Section 8.1 are created with their usages without a device feature.

The matrix rule of Section 3.4 holds for a `float4x4` global uniform. The Rust value `[[1, 0, 0, tx], [0, 1, 0, ty], [0, 0, 1, 0], [0, 0, 0, 1]]`, written row by row, translated by (tx, ty) with `mul(transform, v)` and did not translate with `mul(v, transform)`. A math type that stores columns, such as the arrays that glam's `Mat4::to_cols_array_2d` returns, therefore needs `mul(v, transform)` or a transpose.

naga rejects these render constructs, with the same text on both adapters, in the form of Section 3.5:

| Construct | naga's error text |
| --- | --- |
| `SV_VertexID` or `SV_InstanceID` in the vertex stage | `Shader '' parsing error: UnsupportedCapability(DrawParameters)` |
| `Sampler2D` | `Shader '' parsing error: InvalidId(12)` |
| An integer varying without `nointerpolation` | `Shader validation error: Entry point vsMain at Vertex is invalid`, followed by `@interpolate(flat) must be explicitly specified for integer I/O` |

`SV_VulkanVertexID` and `SV_VulkanInstanceID` load instead, and the instance number includes the first instance of the draw. A `nointerpolation` integer varying loads. Two colour targets, `discard`, `ddx` and `ddy`, `SV_Position` read in the fragment stage, and attributes that step per instance also worked.

`SV_VulkanVertexID` and `SV_VulkanInstanceID`, in a structure or as separate parameters, have no category `VaryingInput` and take no location, so the attributes around them keep consecutive locations, and `slangc` decorates them as built-ins without a location (run, step 7). An `int`, `uint`, or `uint2` attribute reflects as a 32-bit signed or unsigned integer with its component count (run, step 7). In one series of 7 runs of a release build on the development machine, the load of a module with two kernels took a median of about 6 ms longer with a vertex and a fragment entry point added, a difference that also includes parsing the longer source (run, step 7).

A reload of a module with a vertex and a fragment entry point, global uniforms, a constant buffer, and a parameter block took a median of about 155 ms with a new global session and about 11.5 ms with the global session kept, on both adapters, over 7 runs of a release build. The reload covered the session, the load, the link, the SPIR-V of both entry points, reflection, both shader modules, and the render pipeline. Of the 155 ms, creating the global session took about 107 ms and releasing it about 27 ms. The work of wgpu, both shader modules, the layouts, and the render pipeline, took under 1 ms.

## 4. Crate Structure and Dependencies

### 4.1. Package and Dependencies

SLO-1. `slango` MUST depend on `slang-compiler` and on wgpu of major version 30 with the feature `spirv`, and on no other crate except `pollster`, which blocks on wgpu's futures (Section 16). The list stays short because every dependency runs on every user's machine (DEP-2). Version 30 is the newest wgpu that the current Slint release supports (ARCH-19, Section 3.7). The implementation re-checks the version at each Slint release, and an upgrade is a breaking release of `slango` (DEP-3). The crate has no Cargo features in M3 and M4.

SLO-2. `slango` MUST contain no unsafe code in M3 and M4, and MUST load every shader through the validated path only. The validated path ran every kernel that M3 needs and every render shader that M4 needs without unsafe code (Sections 3.2 and 3.9), so passthrough would add unsafe code for no need. This answers Q-19 of the architecture design for `slango` through M4.

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

This illustration shows the whole flow of M3. It shows the form only. The kernel `scale` in `scale.slang` multiplies each element of a buffer by a factor:

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

### 5.3. Types for Rendering

SLO-19. `slango` MUST provide the public types `RenderPipeline`, `Texture`, and `Sampler`, each cheap to clone and holding its `Device` and no Slang object. It MUST create `RenderPipeline` and `Sampler` through options structs that implement `Default`. As for SLO-3, a value without its parent would need a lifetime, and options without `Default` could not grow without a breaking change.

These are the new public items of M4:

- `RenderPipeline`: A render pipeline built from a vertex and a fragment entry point of a `Module`, with the merged layout snapshot of both, its bind group layouts, and its wgpu render pipeline (Section 7.5).
- `RenderPipelineOptions`, `Blend`, and `CullMode`: The options of a render pipeline (Section 7.5).
- `Texture` and `TextureFormat`: A two-dimensional texture with its size and format (Section 8.1).
- `Sampler`, `SamplerOptions`, `FilterMode`, and `AddressMode`: A sampler and its options (Section 8.1).
- `Draw`: The builder of one draw, which `RenderPipeline::draw` returns (Section 9.5).
- `VertexElement`: The sealed trait of the element types that a vertex attribute and a buffer accept (Section 8.2).

Every options type and enumeration is non-exhaustive, as `DeviceOptions` is (Section 5.1). `slango` owns no window, surface, or frame loop, and depends on no windowing crate (SLO-1). A caller that renders to a window configures the surface with wgpu, wraps the view of each frame's surface texture as a `Texture` through `slango::wgpu::texture_from_view` (Section 8.1), draws into it, and presents the frame itself.

### 5.4. Usage Illustration for Rendering

This illustration shows the form of a draw in M4, whose `Module::render_pipeline`, `RenderPipeline`, and `Draw` do not exist yet. The file `quad.slang` samples a texture onto a quad and transforms its vertices by a uniform matrix:

```slang
struct VIn { float2 position : POSITION; float2 uv : TEXCOORD0; };
struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };

uniform float4x4 transform;
Texture2D<float4> image;
SamplerState imageSampler;

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = mul(transform, float4(input.position, 0.0, 1.0));
    output.uv = input.uv;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return image.Sample(imageSampler, input.uv);
}
```

The Rust program creates the pipeline, the buffers, the texture, the sampler, and a target, draws two triangles through an index buffer, and reads the target back:

```rust
use slango::{
    Buffer, Device, DeviceOptions, RenderPipelineOptions, Sampler, SamplerOptions, Texture,
    TextureFormat,
};

fn main() -> slango::Result<()> {
    let device = Device::new(&DeviceOptions::default())?;
    let module = device.load_module("quad.slang")?;
    let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
    let corners = [[-1.0_f32, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    let positions = Buffer::from_slice(&device, &corners)?;
    let uvs = Buffer::from_slice(&device, &[[0.0_f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]])?;
    let indices = Buffer::from_slice(&device, &[0_u32, 1, 2, 0, 2, 3])?;
    let red_and_blue = [[255, 0, 0, 255], [0, 0, 255, 255], [0, 0, 255, 255], [255, 0, 0, 255]];
    let image = Texture::from_pixels(&device, [2, 2], &red_and_blue)?;
    let sampler = Sampler::new(&device, &SamplerOptions::default())?;
    let target = Texture::new(&device, [64, 64], TextureFormat::Rgba8Unorm)?;
    let identity = [
        [1.0_f32, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    pipeline
        .draw()
        .set("transform", identity)
        .texture("image", &image)
        .sampler("imageSampler", &sampler)
        .vertex("input.position", &positions)
        .vertex("input.uv", &uvs)
        .target(&target)
        .clear([0.0, 0.0, 0.0, 1.0])
        .submit_indexed(&indices, 6, 1)?;
    let pixels: Vec<[u8; 4]> = target.read()?;
    assert_eq!(pixels[0], [255, 0, 0, 255]);
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

Step 2 built the snapshot as one record for each compute entry point, and step 7 added the records of vertex and fragment entry points (Section 7.4). The record of a kernel holds:

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

The walk supports these parameters and rejects every other with `ErrorKind::UnsupportedParameter`, whose text names the parameter and the kernel, the vertex entry point, or the fragment entry point:

| Parameter | Snapshot |
| --- | --- |
| `StructuredBuffer<T>`, `RWStructuredBuffer<T>` | A read-only or read-write storage buffer |
| `ConstantBuffer<S>`, `cbuffer`, global `uniform` plain values | A uniform buffer |
| `ParameterBlock<S>`, nested to any depth | Its bindings in its own set |
| `uniform` parameters of the entry point | The immediates |
| `bool`, `int`, `uint`, and `float` as scalars, vectors, matrices, and arrays of fixed length, including arrays of arrays | Plain values |
| A structure of supported values | One plain value for each leaf |
| A system value | Nothing (Section 3.3) |
| `Texture2D` with a `float` result, and `SamplerState`, from M4 | A sampled texture or a sampler (Section 8.1) |

Among the rejected parameters are every other resource, such as a storage texture, `Sampler2D`, or `ByteAddressBuffer`, a plain value of another scalar type, such as `double`, an array of structures or of resources, and an array without a fixed length. A failure of Slang to compile, link, or reflect the module is `ErrorKind::Compile`.

### 7.2. The Global Session for Each Load

SLO-7. `Device::load_module` MUST create its own global session and release it before it returns. `slango` MUST keep no global session in a `static`, a `thread_local!`, or any hidden state, because CONC-4 forbids hidden library state. The simplest form also has no state to invalidate or to clean up at thread exit.

The cost is about 135 ms for each load (Sections 3.6 and 3.9). That is acceptable for a program that loads a few modules once, and for the hot reload of a development tool after each save of a file, such as the example of Section 13.5. SLO-7 therefore stays through M4. A caller-held compiler or a cache of global sessions becomes an open question again only when a later need, such as loading many modules, makes the cost matter.

### 7.3. Kernels and Pipelines

SLO-8. `Device::load_module` MUST create the shader module, the bind group layouts, and the compute pipeline of every compute entry point before it returns. It MUST fail as a whole when any of them fails, because every error of a module then surfaces at one call. Each creation costs under 1 ms (Section 3.6).

SLO-9. `slango` MUST create each shader module inside an error scope and MUST return a rejection by naga as `ErrorKind::UnsupportedShader`, with naga's text returned unchanged as the source error (Section 10), because the text names the construct that the user must change (Section 3.5). An error that wgpu reports to its uncaptured-error handler is lost to the caller.

The bind group layouts leave out every binding that the code does not use, which wgpu accepts (Section 3.3), so that the caller binds only what the kernel reads or writes. A kernel has one bind group layout for each set from 0 to the last set that holds a used binding, and a set between them with no used binding gets an empty layout. A parameter that the walk rejects fails the load with `ErrorKind::UnsupportedParameter` (Section 7.1). The load checks SLO-18 before it creates the shader module of a kernel, so it fails with `ErrorKind::MissingFeature` when the kernel has immediates and the device lacks `Features::IMMEDIATES`, or when the reflected size of the immediates exceeds the device's `max_immediate_size`. Section 10 states how the load maps the errors of wgpu.

`Module::kernel` looks a kernel up by its Slang name and returns a reference to it. For a name that is no compute entry point of the module, it returns `ErrorKind::InvalidArgument` with a text that lists no names. No method lists the kernels of a module.

### 7.4. Vertex and Fragment Entry Points

SLO-20. `Device::load_module` MUST also record the layout snapshot and the SPIR-V of every vertex and fragment entry point, and MUST NOT create their shader modules or fail on their parameters. Otherwise a render entry point that `slango` rejects would make the kernels of its module unusable.

The load compiles every entry point as Section 7.1 states, with one SPIR-V module for each (Section 3.9), and still ignores the entry points of other stages. The record of a vertex or fragment entry point holds its name, its stage, its bindings and plain values as Section 7.1 defines them, with the usage of each binding from that entry point's metadata, and, for a vertex entry point, its vertex attributes. A failure that belongs to one such entry point becomes the rejection on its record, and the load goes on (SLO-20). The rejection is `ErrorKind::UnsupportedParameter` for a parameter that the walk refuses, and `ErrorKind::Compile` with Slang's text for a failure of Slang to reflect the entry point, report its metadata, or generate its SPIR-V. A rejected record keeps no SPIR-V, and `Module::render_pipeline` returns its rejection. The load skips an entry point whose name Slang cannot report, and fails as a whole only for a failure of the whole compilation or of a kernel, as before step 7. Where a module has several parameters that a load refuses, the error may now name a different one than before. Compute kernels keep their pipelines at load (SLO-8).

SLO-21. The walk MUST record each vertex attribute of a vertex entry point with its parameter path, location, scalar type, component count, and usage, because the render pipeline builds its vertex layout from reflection alone and the draw checks each buffer against it.

The walk records the vertex attributes by these rules:

- It visits the parameters that `EntryPointLayout::parameter_layouts` returns with the category `VaryingInput`, and the fields of a structure, recursively.
- The parameter path is the dotted path from the parameter's name, such as `input.position` for the field `position` of the parameter `input`. A parameter that is not a structure has its own name as its path.
- The location is the sum of `offset(VaryingInput)` along the path (Section 3.9).
- A field without the category `VaryingInput`, such as `SV_Position`, `SV_VulkanVertexID`, or `SV_VulkanInstanceID`, is skipped (Section 3.9). That every other system value lacks the category is assumed (SOA-6).
- A `float`, `int`, or `uint` scalar, or a vector of 1 to 4 of them, is supported, and step 7 settled that a vector of one component is an attribute like a scalar. Every other type, such as a matrix, an array, or a `bool`, is a rejection of the walk.
- The usage comes from `Metadata::is_parameter_location_used` with `ParameterCategory::VaryingInput`, space 0, and the location.

The walk records no varying and no output of the fragment entry point, because wgpu matches the varyings by location (Section 3.9).

Step 7 settled that the walk accepts a sampler only when its type is `SamplerState`, so it rejects `SamplerComparisonState`, which reflects with the same kind, under SLO-24.

### 7.5. Render Pipelines

`Module::render_pipeline` takes the name of a vertex entry point, the name of a fragment entry point, and a `RenderPipelineOptions`, and returns a `RenderPipeline`. A name that is no entry point of that stage gives `ErrorKind::InvalidArgument`, as `Module::kernel` does (Section 7.3). Each call creates a new pipeline, and `slango` keeps no cache, because a cache is hidden state (CONC-4). The topology is always a list of triangles, and front faces are counter-clockwise, which is wgpu's default.

These are the fields of `RenderPipelineOptions`, each with its default:

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `colour_format` | `TextureFormat` | `Rgba8Unorm` | The format of the colour target. `Depth32Float` gives `ErrorKind::InvalidArgument` |
| `depth_format` | `Option<TextureFormat>` | `None` | `Some(TextureFormat::Depth32Float)` adds a depth test with the comparison `Less` and depth writes. Any other format gives `ErrorKind::InvalidArgument` |
| `blend` | `Blend` | `Blend::Replace` | `Replace` writes the colour of the fragment entry point, and `Alpha` blends it over the target with wgpu's `BlendState::ALPHA_BLENDING` |
| `cull_mode` | `CullMode` | `CullMode::None` | `Back` discards the triangles that face away |
| `instance_attributes` | `Vec<String>` | Empty | The parameter paths of the vertex attributes that step per instance. Every other attribute steps per vertex. A path that names no vertex attribute gives `ErrorKind::InvalidArgument` |

The step mode is an option of the pipeline, not of the draw, because wgpu fixes it when it creates the pipeline (Section 3.9).

SLO-22. `Module::render_pipeline` MUST create both shader modules, the bind group layouts, the pipeline layout, and the render pipeline inside error scopes. It MUST fail as a whole, so that every error of a pipeline surfaces at one call, as SLO-8 requires for kernels.

The pipeline merges the bindings of its two entry points, which share every global binding (Section 3.9). Each binding that a stage uses gets one bind group layout entry, visible to every stage whose metadata reports it used, and a binding that neither stage uses is left out, as Section 7.3 states for kernels. The vertex state has one vertex buffer for each used attribute, in the order of locations, with the attribute at offset 0 and a stride equal to the size of its format. An unused attribute gets no vertex buffer (Section 3.9). Section 10 states how the errors of creation map to kinds.

SLO-23. `Module::render_pipeline` MUST return `ErrorKind::UnsupportedParameter` when the vertex or the fragment entry point has `uniform` parameters of its own, because their push constant blocks alias at offset 0 and wgpu sets immediates without a stage (Section 3.9).

The uniforms of a render pipeline are therefore global uniforms, constant buffers, and parameter blocks, and its pipeline layout has no immediates.

## 8. Buffers

SLO-10. A call MUST accept as the element type `T` of a bound `Buffer<T>` only the types of a sealed trait whose byte representation equals the std430 layout of the matching Slang element type without padding. Each such type MUST have a test that checks its size against the stride that reflection reports, because a Rust type with another layout would place every element after the first at a wrong offset (Section 3.4).

The sealed trait is `Element`. Step 4 implemented it for exactly these types, whose strides the test of SLO-10 checks against reflection (Section 3.3):

| Rust type | Slang type | Stride in bytes |
| --- | --- | --- |
| `f32`, `i32`, `u32` | `float`, `int`, `uint` | 4 |
| `[f32; 4]`, `[i32; 4]`, `[u32; 4]` | `float4`, `int4`, `uint4` | 16 |

The implementation adds further types only with the test of SLO-10. The size of a buffer is its length times the stride. Elements and the scalars of plain values (Section 9.2) are written little-endian, and elements are read the same way.

These rules settle the creation of a buffer and its contents:

- `Buffer::from_slice` creates a buffer that holds a copy of a slice, and `Buffer::zeroed` creates one of a given length whose bytes are zero.
- A buffer has the usages `STORAGE`, `COPY_SRC`, `COPY_DST`, `VERTEX`, and `INDEX`, the last two for the draw (Section 8.2).
- A buffer of length zero is allowed, and `read` returns it as an empty vector without work on the GPU. A call rejects it as an argument (Section 9.3).
- No method writes new contents into an existing buffer.

SLO-11. `Buffer::read` MUST block until every dispatch submitted before it has finished and MUST return the elements as a `Vec<T>`, because a caller must not observe a partial result. A blocking read is the only synchronization that M3 offers. It copies the buffer to a staging buffer, maps it, and blocks the calling thread until the device has finished.

### 8.1. Textures and Samplers

SLO-24. The walk MUST accept a `Texture2D` with a `float` result and a `SamplerState` in every stage. It MUST reject every other texture or sampler kind, including `Sampler2D`, with `ErrorKind::UnsupportedParameter`, because only these two are in the supported set of parameters (Section 7.1). This is the walk's own check of that set and no screen for a defect of naga (Section 1.7).

The snapshot records a texture as a sampled texture and a sampler as a sampler, each with its parameter path, set, binding, and usage. The backend builds the layout entries of Section 3.9, visible to the compute stage in a kernel and as Section 7.5 states in a render pipeline. The setters `texture` and `sampler` are in Sections 9.1 and 9.5. Storage textures, texture arrays, cube maps, and textures of other shapes or result types stay rejected (Section 1.5). A subscript of a texture fails as Section 3.5 states.

These rules settle textures:

- `Texture::from_pixels` takes a device, a size `[width, height]`, and a slice of `[u8; 4]` pixels in rows from the top, and creates an `Rgba8Unorm` texture. It takes no format. `Texture::new` takes a device, a size, and a `TextureFormat`, and creates a texture whose bytes are zero.
- `TextureFormat` has the colour formats `Rgba8Unorm`, `Rgba8UnormSrgb`, `Bgra8Unorm`, and `Bgra8UnormSrgb`, and the depth format `Depth32Float`.
- A texture of a colour format has the usages `TEXTURE_BINDING`, `RENDER_ATTACHMENT`, `COPY_SRC`, and `COPY_DST`. A `Depth32Float` texture has `RENDER_ATTACHMENT` only, so it serves as a depth target and is neither sampled nor read.
- A width or height of zero, a size over the device's limit, or a number of pixels other than the width times the height gives `ErrorKind::InvalidArgument`.
- Every texture has one mip level and one sample.
- `Texture::size` and `Texture::format` return the size and the format.
- `Texture::read` returns the pixels of a colour texture as a `Vec<[u8; 4]>` in rows from the top, in the byte order of its format, and blocks as `Buffer::read` does (SLO-11). It gives `ErrorKind::InvalidArgument` for a `Depth32Float` texture, even a wrapped one with `COPY_SRC`, and for a texture without `COPY_SRC`.

These rules settle samplers. `Sampler::new` takes a device and a `SamplerOptions`, whose fields are these:

- `filter`: A `FilterMode`, `Nearest` by default or `Linear`, for magnification and minification.
- `address_mode`: An `AddressMode`, `ClampToEdge` by default, `Repeat`, or `MirrorRepeat`, for every axis.

The other settings of a sampler, such as its mipmap filter, are wgpu's defaults, with no comparison and no anisotropy.

The module `slango::wgpu` has `texture`, which returns the wgpu texture of a `Texture`, and `texture_from_view`, which wraps a wgpu texture view of a device, such as the view of a surface texture, as a `Texture` (SLO-4). No function returns the view of a `Texture`. `texture_from_view` takes the size, format, and usages from the viewed texture through `TextureView::texture` (read, wgpu 30.0.1). It gives `ErrorKind::InvalidArgument` for a format that `TextureFormat` lacks and for a texture that is not two-dimensional with one sample, one mip level, and one array layer, so that `size` and `read` describe what a kernel samples. It does not check which device the view belongs to. A call, a draw, or a read that needs a usage that such a texture lacks gives `ErrorKind::InvalidArgument`.

### 8.2. Vertex and Index Buffers

SLO-25. `slango` MUST provide a sealed trait `VertexElement`, a supertrait of `Element`, for every `Element` and the packed arrays of 2 and 3 `f32`, `i32`, or `u32`, because a vertex attribute reads packed vectors, such as `Float32x3`.

Step 8 implemented `VertexElement` for `f32`, `i32`, and `u32` and for the arrays of 2, 3, and 4 of each, with the packed strides 4, 8, 12, and 16 bytes. `Element` has `VertexElement` as a supertrait and keeps its six types and their strides (Section 8). `Buffer::from_slice`, `Buffer::zeroed`, `Buffer::read`, `Buffer::len`, and `Buffer::is_empty` take the bound `VertexElement` instead of `Element`, which accepts every type that they accepted. `Call::bind` keeps the bound `Element`, because only those types have the stride test of SLO-10. Every buffer has the usages `VERTEX` and `INDEX` (Section 8). Step 10 maps each element type to the vertex format of its scalar type and component count, because the draw is the only reader of the mapping: `f32` to `Float32`, `[f32; 2]` to `Float32x2`, `[i32; 3]` to `Sint32x3`, `[u32; 4]` to `Uint32x4`, and so on. An attribute of type `int` or `uint` has been reflected (Section 3.9) but not loaded through naga (SOA-6). An index buffer is a `Buffer<u32>`, read with `IndexFormat::Uint32`.

## 9. The Call

### 9.1. Arguments by Name

A kernel's parameters are named globals and named parameters of the entry point, with no positional order, so the call addresses each one by its parameter path. `Kernel::call` returns a builder. Its setter `set` records a plain value at a parameter path, its setter `bind` records a buffer for a parameter path, and both are infallible (API-6). This answers Q-18 of the architecture design for M3.

Step 5 settled these choices for the builder `Call`:

- It borrows the kernel, the parameter paths, and the bound buffers, and clones none of them.
- `set` and `bind` take `&mut self`, and `dispatch` and `dispatch_groups` take `&self`, so a dispatch does not consume the call, and the same call can dispatch again with the same arguments.
- A later `set` or `bind` for a path replaces the earlier argument for that path, whichever setter gave it.
- Every mistake in the arguments is reported by the dispatch, never by a setter (Section 9.3).

Step 8 added the setters `texture` and `sampler`, which record a `Texture` or a `Sampler` for a parameter path under the same rules and are infallible too.

### 9.2. Marshalling

SLO-12. The trait through which `set` takes a value MUST be sealed (API-8). It MUST check the value against the reflected scalar type, shape, and size of the parameter before it writes any byte, because a wrong value otherwise reaches the GPU as wrong numbers with no error.

SLO-13. `slango` MUST write every plain value at its reflected offset, element by element for vectors, matrices, and arrays, and MUST NOT copy the bytes of a Rust struct as a whole, because the reflected layout differs from every Rust layout (Section 3.4).

The sealed trait is `Value`. Step 4 implemented it for `bool`, `i32`, `u32`, and `f32`, which stand for `bool`, `int`, `uint`, and `float`, and for arrays `[T; N]` of a value type, nested to any depth. Step 5 made `Copy` a supertrait of `Value`, so that `set` takes the value itself. A Rust array stands for any Slang structure of the same nesting:

- `[f32; 3]` stands for a vector `float3` or an array `float[3]`.
- `[[f32; 3]; 2]` stands for a matrix `float2x3`, whose first index is the row, an array of vectors `float3[2]`, or an array of arrays `float[2][3]`.

A value matches its parameter when the scalar types are equal and the lengths of the nesting are equal, from the outermost to the innermost, and the reflected size enters only when the value is written. A value that does not match gives `ErrorKind::InvalidArgument`, whose message names the parameter path, the Slang type, and the Rust type, and no byte is written. Each scalar is written at its reflected offset, and a `bool` as a 32-bit 0 or 1. A matrix is written row by row, as Section 3.4 states. Global plain values and plain values in constant buffers and parameter blocks go into uniform buffers that the call owns, and plain values of the entry point go into the immediates of the dispatch.

### 9.3. Validation Before Dispatch

SLO-14. The dispatch MUST check, before it submits any work, that every parameter path names a parameter of the kernel and that every used parameter is set. It MUST check that every value matches its parameter (SLO-12) and that the element stride of every bound buffer equals the reflected stride. It MUST check that every buffer belongs to the kernel's device and that every workgroup count is within the device's limit. It MUST return `ErrorKind::InvalidArgument` naming the parameter when a check fails, because wgpu reports these mistakes late, with less context, or not at all.

Each `Device` carries an identity that its clones share, and a buffer belongs to the device whose identity it holds. Two `Device` values built separately from the same wgpu device therefore count as different devices.

Step 5 settled the checks of SLO-14 as these rules:

- A parameter that the kernel does not use may be omitted. An argument given for one gets every check and is then ignored.
- Every plain value of a used uniform buffer, and every plain value of the immediates, must be set. The immediates count as used (Section 7.1).
- A bound buffer must not be empty.
- An argument of one kind given for a parameter of another kind, among plain values, buffers, textures, and samplers, is rejected.

Step 8 extended these rules to textures and samplers:

- A texture or a sampler must belong to the kernel's device, as a buffer does.
- A bound texture must have a colour format and the usage `TEXTURE_BINDING`. Of the colour textures, only one from `slango::wgpu::texture_from_view` can lack that usage.
- Every texture and sampler that the kernel uses must be given.

The dispatch runs the checks in this order and returns the first failure, all before any call of wgpu:

1. Each given argument, in the order given: its path, the kind of its parameter, and its value, its buffer, or its texture or sampler. A texture is checked for its device, then its format, then its usage.
2. Each used parameter that no argument gives, in the order of the bindings, followed by the immediates.
3. The workgroup count along each axis against the device's limit.

A mistake of the caller in these checks gives `ErrorKind::InvalidArgument`, whose message names the kernel and the parameter. A reflected layout that cannot hold a value gives `ErrorKind::UnsupportedParameter`.

`slango` makes no aliasing check of its own. When one buffer is bound to a read-only and to a read-write parameter of one call, wgpu's validation rejects the dispatch, and the dispatch returns `ErrorKind::Backend` (SLO-15, Section 3.8).

SLO-15. The dispatch MUST open an error scope around the work it records and submits, and MUST return each validation error that remains as `ErrorKind::Backend`, because an error that reaches wgpu's uncaptured-error handler is lost to the caller.

### 9.4. Dispatch and Results

`dispatch` takes the element counts of three axes and derives each workgroup count by rounding the element count divided by the thread group size up, so the kernel guards the threads past the end itself, as the illustration of Section 5.2 does. `dispatch_groups` takes the workgroup counts directly. A workgroup count over the device's limit is an error (Section 9.3). A count of zero along any axis runs no thread, and the dispatch succeeds once the arguments pass the checks. A dispatch returns once its work is submitted, without waiting for it. Its results come back through `Buffer::read` on a buffer that the caller bound (SLO-11), which sees them because it submits its copy on the same queue after the dispatch.

### 9.5. The Draw

`RenderPipeline::draw` returns the builder `Draw`, which follows the rules of Section 9.1 for `Call`. Its setters `set`, `bind`, `texture`, and `sampler` record parameters as those of a call do (Sections 8.1 and 9.1). A matrix is set row by row (Section 9.2), so a transform written in Rust as rows works with `mul(transform, vector)` in Slang (Section 3.9). These setters record the rest of a draw:

- `vertex`: The buffer of a vertex attribute, named by its parameter path (Section 7.4).
- `target`: The colour target, a `Texture`, which every draw needs.
- `clear`: A colour as `[f32; 4]`, to which the draw clears the colour target first. Without it, the draw keeps the target's contents.
- `depth`: The depth target, a `Texture`, which a pipeline with a depth format needs and every other pipeline rejects.
- `clear_depth`: A depth as `f32`, to which the draw clears the depth target first. Without it, the draw keeps its contents.

The draw ends with `submit`, which takes a count of vertices and a count of instances, or with `submit_indexed`, which takes a `Buffer<u32>` of indices, a count of indices, and a count of instances. Each records one render pass with one draw, submits it, and returns without waiting, as a dispatch does (Section 9.4). A count of zero still clears the targets that the draw clears.

SLO-26. The draw MUST run the checks of SLO-14 on its parameters and the checks below before it submits any work. It MUST return `ErrorKind::InvalidArgument` naming the pipeline's entry points and the parameter or target, because wgpu reports these mistakes late, with less context, or not at all.

The draw runs the checks in this order and returns the first failure:

1. Each given argument, in the order given, as Section 9.3 states. A texture for a parameter has a colour format and `TEXTURE_BINDING`. The buffer of a vertex attribute has the attribute's scalar type and component count and is not empty.
2. Each used parameter and each used vertex attribute that no argument gives. An unused one may be omitted, and an argument given for it gets every check and is then ignored.
3. The targets. The colour target has the pipeline's colour format and `RENDER_ATTACHMENT`. A depth target is given exactly when the pipeline has a depth format, and it has the size of the colour target.
4. The counts. The buffers of the attributes that step per vertex have one length, and so do those that step per instance. The count of vertices, or of indices with `submit_indexed`, is at most the length of those buffers or of the index buffer, and the count of instances is at most the length of the buffers per instance, when there are any.

Each `Texture` and `Sampler` belongs to the device that created it, as a buffer does (Section 9.3). The draw does not read the values of an index buffer (Section 17). It records and submits its work inside an error scope, and returns each error that remains as `ErrorKind::Backend`, as SLO-15 states for a dispatch. `slango` makes no check that one texture is both sampled and a target of one draw, and leaves it to wgpu's validation (SOA-6), as it leaves aliased buffers (Section 9.3).

## 10. Errors

SLO-16. `slango` MUST have one public error type as ERR-1 defines, with these kinds of `ErrorKind`, because callers match on the kind (ERR-1):

| Kind | Meaning |
| --- | --- |
| `Compile` | Slang failed to compile or link the module. `source()` returns the error of `slang-compiler` with its diagnostics |
| `NoAdapter` | wgpu found no adapter that matches the options |
| `DeviceRequest` | The adapter refused the device that the options request |
| `UnsupportedShader` | The backend rejected a kernel at load, or a render pipeline at its creation, as the text after this table states. The source error carries the backend's text |
| `UnsupportedParameter` | A kernel or a render pipeline has a parameter that the walk or SLO-23 rejects (Sections 7.1, 7.4, and 7.5), or, at a call or a draw, the reflected layout of a plain value cannot hold the value (Section 9.3) |
| `MissingFeature` | A kernel needs a device feature that the device lacks |
| `InvalidArgument` | A check of Section 9.3 or of SLO-26, including a value that does not match its parameter (Section 9.2), an unknown kernel or entry point name, a wrong option of a render pipeline, a file that cannot be read as text, a buffer larger than the device's limit or whose size overflows, or a texture whose size or pixels Section 8.1 rejects |
| `Backend` | wgpu reported a validation or out-of-memory error in an error scope at load or at the creation of a render pipeline, as the text after this table states, any error while a buffer, texture, or sampler is created or read, or any error that wgpu reports during a dispatch or a draw (SLO-15) |

The load creates every shader module, bind group layout, pipeline layout, and compute pipeline inside error scopes for validation, internal, and out-of-memory errors, so that no error reaches wgpu's uncaptured-error handler. It maps each error that a scope returns to a kind:

- Any error while the shader module is created gives `UnsupportedShader`.
- An internal error while the layouts or the pipeline are created gives `UnsupportedShader`, because a shader that the backend cannot translate is reported as an internal error.
- A validation or out-of-memory error while the layouts or the pipeline are created gives `Backend`.

The message names the kernel, and the backend's text is returned unchanged as the source error.

`Module::render_pipeline` maps the errors of its scopes in the same way, with one difference: a validation error while the render pipeline itself is created gives `UnsupportedShader`. Such an error reports a mismatch between the two stages or with the pipeline, such as varyings in another order (Section 3.9), which only a change of the shader fixes. The message names both entry points. Every other new failure of M4 has the kind that Sections 7.4 to 9.5 state, so `ErrorKind` keeps its eight kinds.

`NoAdapter` answers Q-22 of the architecture design.

## 11. Threading

SLO-17. `Device`, `Module`, `Kernel`, `Buffer<T>`, `RenderPipeline`, `Texture`, `Sampler`, `Error`, and every options type MUST be `Send` and `Sync`, asserted in a test (CONC-1). `slango` MUST implement neither trait by hand, because the types hold only wgpu objects, which are `Send` and `Sync` (Section 3.2), and owned data, so the compiler derives both.

The builders `Call` and `Draw` also hold only such data, and a unit test asserts that each is `Send` and `Sync`. Clones of one kernel may be dispatched from several threads at once, each with its own `Call`, which a `gpu` test does. Each dispatch sees only its own errors, because the error scopes of wgpu 30 are per thread (Section 3.2). `load_module` uses Slang only on the calling thread, and its Slang objects never leave the call (SLO-5), which keeps every rule of Section 10 of `docs/design/slang-compiler.md`. Several threads that load modules at once each create their own global session, which Slang supports (Section 3.5 of `docs/design/slang-compiler.md`). `slango` starts no thread (CONC-4).

## 12. Device Features

SLO-18. `Device::new` MUST request `Features::IMMEDIATES` with the adapter's maximum immediate size when the adapter offers the feature. The load MUST fail with `ErrorKind::MissingFeature` when a kernel has `uniform` parameters of its entry point and the device lacks the feature or a large enough immediate size, because without the feature naga rejects the module with a message about `entryPointParams` that does not name the cause (run, `Capability Capabilities(IMMEDIATES) is not supported`).

The default immediate size of wgpu is 0. The adapters of the spike and of the CI runners offer 128 or 256 bytes (run, Section 3.8). `Device::new` requests no other optional feature, such as the ones that `double`, `int64_t`, and `half` need (Section 3.5). `DeviceOptions` has no field for them, because a field of type `wgpu::Features` would name a wgpu type outside the module of SLO-4. A caller who needs another feature creates the wgpu device with it and passes the device and its queue to `slango::wgpu::from_device_and_queue`. A `Device` built that way uses the features that its device has. Rendering in M4 needs no optional feature, because a render pipeline has no immediates (SLO-23), and the formats of Section 8.1 need none, as their creation and use in compute kernels on the three adapters of Section 3.8 showed (run, step 8, Section 3.9).

## 13. Tests, CI, and the Example

### 13.1. Tests

`slango` has one integration-test binary (TEST-10). Every test that needs an adapter sits in a module named `gpu`, selects any adapter, and fails when it finds none (ARCH-22, ARCH-23). The tests of the reflection walk call Slang and need no adapter, so they sit in the module `layout::tests::ffi` in the default tier and check their snapshots with `insta`. They are unit tests inside the crate, not tests of the integration-test binary, because the snapshot is private and TEST-1 limits integration tests to the public API. The tests of marshalling, of the checks of a call, and of the workgroup counts are pure and run under Miri. The tests of loading and of the call are `gpu` tests of the integration-test binary, in the modules `gpu::load` and `gpu::call`, with the shader files that they load under `tests/slango/shaders/`. The tests of the call dispatch kernels and assert every result, and assert the kind and text of each error of Section 9.3 and that a rejected call leaves its output buffer unchanged. The test of `AdapterSelection::Software` is compiled out on macOS, because Metal has no software adapter (Section 3.8), and every other `gpu` test runs on all three platforms.

### 13.2. Continuous Integration

The `check` and `coverage` jobs on Linux install lavapipe from Mesa's package `mesa-vulkan-drivers` and run the `gpu` tier. The jobs on Windows and macOS run `just test-gpu`, so they run the same tests on WARP and on hosted Metal. The tier passed on all three platforms with the tests of steps 1, 3, 5, 6, and 8, which create devices, load modules, dispatch kernels, and sample textures (Sections 3.8 and 3.9). This answers Q-21 of the architecture design.

### 13.3. The Example

Example 2 of the architecture design is the headless binary target `compute-kernel` of the package `slang-rust-examples`, with its source at `examples/src/bin/compute-kernel.rs`. It takes one argument, the path of a Slang file, and runs the kernel `saxpy` of that file. Its shader, `examples/shaders/saxpy.slang`, computes `result = a * x + y` for each element of two input buffers, with the global `uniform` values `a` and `count`. The example prints the name of the adapter on standard error, and the inputs and results on standard output. It compares the results exactly with the values that it computes on the CPU, which is valid because the inputs are small whole numbers and `a` is 2.5, so every result is exact. It exits with a failure status on a mismatch or an error, and on an error it prints the error and each of its source errors on standard error. A user runs it from the workspace root, with the Slang library on the library path as the example's documentation describes:

```sh
cargo run -p slang-rust-examples --bin compute-kernel -- examples/shaders/saxpy.slang
```

Its three tests sit in the module `gpu` of the integration-test binary `examples/tests/examples/`, so they run on any adapter, on lavapipe in CI (ARCH-21). The error tests need an adapter too, because the example creates the device before it loads the file. The first test compares standard output with an `insta` snapshot, and the other two check that a compile error and a missing file fail with the error on standard error. The example needs no differentiable function, and the broadcasting case of example 2 waits for the milestone that adds broadcasting.

### 13.4. Tests of Rendering

The tests of the walk for vertex and fragment entry points, vertex attributes, textures, and samplers are `ffi` unit tests in `layout::tests::ffi` with `insta` snapshots, as Section 13.1 states for the walk, and step 7 added them. The merging of the bindings of two stages and the checks of SLO-26 are pure unit tests that Miri runs. The `gpu` tests of rendering sit in the module `gpu::render` of the integration-test binary. They render offscreen into a `Texture`, read its pixels back, and assert them, on lavapipe, WARP, and hosted Metal in CI (Section 13.2). Their shaders under `tests/slango/shaders/` cover a triangle with interpolated colours, uniforms, a constant buffer, and a parameter block used by both stages, a sampled texture, a depth target, an indexed draw, attributes per instance, alpha blending, back-face culling, a matrix transform, and integer attributes. Further `gpu` tests assert the kind and text of each error of SLO-23 and SLO-26, that a rejected draw leaves its target unchanged, and that a compute kernel reads a texture through a sampler, which step 8 added in `gpu::sample` together with the tests of textures and samplers in `gpu::texture`.

These tests are the headless check of rendering that Q-24 of the architecture design asks for, as far as `slango` goes. Q-24 stays open for the windowed example itself, which no CI runner can show (Section 13.5).

### 13.5. The Windowed Renderer Example

Example 3 of the architecture design is the binary target `windowed-renderer` of the package `slang-rust-examples`, with its source at `examples/src/bin/windowed-renderer.rs` and its shader at `examples/shaders/textured-quad.slang`. It takes one argument, the path of a Slang file. It opens a window with winit and draws, in every frame, a quad with an index buffer and a checkerboard texture that it creates with `Texture::from_pixels`, rotated by a `float4x4` transform whose angle grows with time. It draws into the view of the surface texture through `slango::wgpu::texture_from_view`. The window, the surface and its configuration, and the frame loop are the example's own winit and wgpu code.

The example reloads its shader without a file-watching dependency. Before each frame, it compares the modification time of the file with the one it loaded. When the time changed, it loads the module and creates the pipeline again. When either fails, it prints the error and each of its source errors on standard error and keeps drawing with the old pipeline. Section 7.2 states why the cost of each reload is acceptable.

The implementation step adds a way to render a fixed number of frames and exit, such as an option on the command line, when that stays small, so that a person need not close the window to exercise the example. CI builds the example, because `check-clippy` compiles every target of the workspace, and the example runs locally only (ARCH-21). After the step that adds it, a user runs it from the workspace root, with the Slang library on the library path and a display:

```sh
cargo run -p slang-rust-examples --bin windowed-renderer -- examples/shaders/textured-quad.slang
```

winit 0.30, the version that the examples of wgpu 30 use, becomes a dependency of the examples package only. Section 9 of `docs/design/tooling.md` records its features and the licence and duplicate-version changes that the step settles.

## 14. Implementation Order for M3

These steps implement M3. Each step leaves `just check` passing.

1. Add the crate skeleton, `Device` with `DeviceOptions`, adapter selection, the features of Section 12, `Error` and `ErrorKind`, the changes of Section 4.2, and the installation of lavapipe in the `check` job. Tests: the `Send` and `Sync` assertions of SLO-17, the `Display` text of each error kind, and a `gpu` test that creates a device and checks that it has immediates when the adapter offers them. Done on 2026-11-11, with lavapipe also installed in the `coverage` job, and all six CI jobs passed with it (Section 3.8).
2. Add the layout snapshot and the walk of Section 3.3. Tests: `ffi` snapshot tests for global plain values, constant buffers, parameter blocks, nested parameter blocks, parameters of the entry point, system values, unused parameters, and a texture that the walk reports as unsupported. Done on 2026-11-11, with the `ffi` tests as unit tests (Section 13.1) and the snapshot as Section 7.1 describes.
3. Add `Module`, `Kernel`, and `load_module` with the SPIR-V of each entry point, the error scope of SLO-9, and the layouts and pipelines. Tests: `gpu` tests that load the kernels of Section 3.2, a module that naga rejects, which returns `UnsupportedShader` with naga's text (TEST-6), and a kernel with uniforms of its entry point on a device without immediates, which returns `MissingFeature`. Done on 2026-11-11, with the choices of Sections 7.1 and 7.3 and the mapping of errors of Section 10. The rejected module calls `frexp`, because a 16-bit integer loads on a device with the matching feature (Section 3.5), and the tests passed on all three CI runners (Section 3.8).
4. Add `Buffer<T>` with upload and `read`, and the marshalling of plain values. Tests: the stride test of SLO-10, pure tests that write the values of Section 3.4 at their reflected offsets, and a `gpu` round trip of a buffer. Done on 2026-11-11, with the element and value types and the rules of Sections 8 and 9.2, the identity of a device of Section 9.3, and the matrix rule of Section 3.4. The `gpu` tests passed on all three CI runners, and the writing of plain values is tested against reflected offsets only, until step 5 dispatches.
5. Add the call builder, the checks of Section 9.3, and the dispatch. Tests: one test of each check of SLO-14, pure tests of the workgroup counts, and `gpu` tests that dispatch the kernels of Section 3.2 and assert every value. Done on 2026-11-11, with the choices of Sections 9.1, 9.3, and 9.4. The `gpu` tests passed on all three CI runners, and confirmed the matrix rule of Section 3.4 and SOA-2 (Section 3.8).
6. Add the example of Section 13.3 with its test and the crate documentation, and update the documents that M3 makes false. Tests: the example's `gpu` test. Done on 2026-11-11, with three `gpu` tests of the example (Section 13.3), and all six CI jobs passed with it in run 42966911977 (Section 3.8). With this step, M3 is complete.

These steps continue the numbering and implement M4. Each step leaves `just check` passing and adds its tests.

7. Add the records of vertex and fragment entry points with their vertex attributes, and textures and samplers in the walk, with no change of the public API (Sections 7.4 and 8.1). Tests: `ffi` snapshot tests of the shaders of Section 3.9, an unused attribute, a system value, a rejected attribute type, `uniform` parameters of a render entry point, `Sampler2D`, and a module whose kernel loads while its fragment entry point is rejected. Done on 2026-11-11, with the choices of Section 7.4 and no change of public behaviour beyond the one difference of a load that Section 7.4 states, the parameter that its error may name. The tests also cover integer attributes and the bindings that two entry points share, and a `gpu` test runs a kernel of a module with a vertex and a fragment entry point.
8. Add `Texture`, `TextureFormat`, `Sampler` with its options, `VertexElement` and the new buffer usages, the texture functions of `slango::wgpu`, and textures and samplers as arguments of a call (Sections 8.1 and 8.2). Tests: pure tests of the size checks of textures, a `gpu` round trip of `Texture::from_pixels` and `Texture::read`, a kernel that reads a texture through a sampler, and the `Send` and `Sync` assertions of SLO-17. Done on 2026-11-11, with the choices of Sections 8.1, 8.2, 9.1, and 9.3. The tests also cover every format, every sampler option, and each check of a call with a texture or a sampler, and passed on all three CI runners in run 42996353428 (Section 3.1). The mapping of element types to vertex formats moved to step 10 (Section 8.2).
9. Add `Module::render_pipeline` with its options (Section 7.5). Tests: `gpu` tests that create the pipelines of Section 3.9, a vertex entry point that reads `SV_VertexID` and varyings in another order, which return `UnsupportedShader` (TEST-6), `uniform` parameters of a render entry point, which return `UnsupportedParameter`, and unknown names and paths, which return `InvalidArgument`.
10. Add `Draw` with its checks and its submission (Section 9.5), and the mapping of element types to vertex formats (Section 8.2). Tests: one test of each check of SLO-26 and the `gpu` tests of Section 13.4.
11. Add the example of Section 13.5 with winit and the changes to `deny.toml` that Section 9 of `docs/design/tooling.md` plans. Check: `check-clippy` builds the example and `check-deny` passes.
12. Complete the crate documentation, including the matrix convention of Section 9.5 and how a caller draws into a window (Section 5.3), and update the documents that M4 makes false. With this step, M4 is complete.

## 15. Assumptions to Confirm

Each row is a statement that this design relies on and that the spike did not establish. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in Section 3 and in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SOA-5 | Render pipelines through the validated path draw on Direct3D 12 with WARP and on Metal as on Vulkan, with the conventions of a draw of Section 3.9: no flip of the y axis, the same winding of a front face, and the same depth range. The spike drew only on Vulkan | Sections 7.5, 9.5, and 18.2 | The `gpu` tests of step 10 of Section 14 in CI |
| SOA-6 | A vertex attribute of type `int` or `uint`, scalar or vector, loads through naga and arrives unchanged. A system value other than `SV_Position`, `SV_VulkanVertexID`, and `SV_VulkanInstanceID` lacks the category `VaryingInput`. wgpu rejects one texture that is both sampled and a target of one draw. An index past the end of a vertex buffer reaches only the backend's own handling | Sections 7.4, 8.2, 9.5, and 17 | The `gpu` tests of step 10, and for another system value the change that first uses it |
| SOA-7 | winit 0.30 with the features that Section 9 of `docs/design/tooling.md` names builds in every CI job without the system libraries of a display, and `check-deny` reports for the workspace what it reported for a scratch package with the same dependencies | Section 13.5 | Step 11 |

Step 5 of Section 14 removed the last two rows of M3. It confirmed SOA-2, that SPIR-V through naga, with immediates, dispatches on Direct3D 12 with WARP and on Metal, in CI run 42961810362 (Section 3.8). It confirmed SOA-3, that wgpu adds bounds checks to every buffer access of a kernel on the validated path, (read, source of wgpu 30.0.1 on 2026-11-11), with the correction that wgpu injects the checks only where the platform cannot enforce them itself (Section 3.2). No test established it.

Step 7 of Section 14 confirmed SOA-6 in part. It confirmed that `SV_VulkanVertexID` and `SV_VulkanInstanceID` lack the category `VaryingInput` (Section 3.9), and the row keeps the rest.

Step 8 of Section 14 confirmed SOA-5 in part, through compute kernels. It confirmed that the texture coordinates (0, 0) sample the top-left pixel on WARP and on Metal as on Vulkan, and that every format of Section 8.1 is created without a device feature (run, step 8, Section 3.9), and the row keeps the rest.

Of the assumptions of the architecture design that name this document, Section 3.3 to Section 3.5 confirm A-12, Section 3.7 confirms A-13, and Section 3.2 confirms A-11 for M3. M4 does not need A-11, because it keeps the validated path and defers other shader targets (Section 1.5). Section 3.7 confirms A-10 in part, which covers the first three patterns of Section 8.3 of that design and the description of `GpuFuture`, with "replacing" read as "building an experimental replacement". It did not cover typed shader interfaces generated from reflection, so A-10 stays for that pattern. A-9 holds as far as SlangPy's documentation was read (Section 3.7), and M3 does not depend on it. A-17 and A-18 are removed from the architecture design, because the `gpu` tests of this project pass on the three hosted runners (Section 3.8).

## 16. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it in the milestone of the last column, and the change that answers it removes the row.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SOQ-3 | How large must a kernel be before naga's parsing at load or lavapipe's first dispatch takes long enough to matter? | This document | The milestone that first loads kernels larger than those of the example |

Section 7.2 answered SOQ-1 for M4 on 2026-11-11, from the costs that Section 3.9 measured, and its row is removed. The global session for each load of SLO-7 stays.

Of the questions of the architecture design that name this document, Section 18.5 answers Q-16 for `slango` through M4, Section 6 answers Q-17, Section 9.1 answers Q-18 for M3, SLO-2 answers Q-19 for `slango` through M4, SLO-1 answers Q-20, Section 10 answers Q-22, Section 13.2 answers Q-21, Section 13.4 answers Q-24 for `slango`, Section 1.5 defers Q-23 with the configurable shader target, and Section 7.1 answers Q-31 and makes Q-30 unnecessary for M3.

`slango` waits on wgpu's futures with `pollster::block_on`, because wgpu 30 offers no blocking form of `Instance::request_adapter`, `Instance::enumerate_adapters`, or `Adapter::request_device`.

## 17. Security Considerations

The threat model treats the shader source, the file paths, and the options that a program passes as trusted input from the program itself, as Hard Rule 14 of `AGENTS.md` states, and covers mistakes in the arguments of a call, which Section 9.3 checks. `slango` has no unsafe code (SLO-2) and no build script. Its memory safety rests on the safe APIs of wgpu and `slang-compiler`, and the safety of the GPU's memory accesses rests on naga's validation and on the bounds checks that wgpu promises on the validated path, which it injects where the platform cannot enforce them itself (Section 3.2). Slang is linked as `docs/design/slang-compiler-sys.md` specifies, and `Cargo.lock` and `cargo-deny` fix and check the versions of wgpu's dependency tree (Section 4.2).

These attacks are out of scope, because a program that loads them has already chosen to run them: a malicious shader, a shader that reaches a defect of Slang, naga, wgpu, or a driver, and a kernel that never finishes, which blocks `Buffer::read`.

Rendering in M4 adds no unsafe code (SLO-2). It adds textures, samplers, and vertex and index buffers, whose sizes and formats the draw checks (SLO-26). The draw does not read the values of an index buffer, so an index past the end of a vertex buffer reaches the backend, and what a vertex fetch then reads rests on wgpu and the driver (SOA-6).

The residual risk is a defect of naga, wgpu, or a GPU driver that lets a kernel or a draw read or write outside its buffers and textures, and a crash that a defect of Slang causes, which Section 3.12 of `docs/design/slang-compiler.md` records.

## 18. Operational Considerations

### 18.1. Cargo Features and Environment Variables

`slango` has no Cargo features in M3 and M4. It reads no environment variable itself. Its build needs `VULKAN_SDK`, and its programs need the Slang shared library on the library path, as `slang-compiler-sys` requires (Section 19.4 of `docs/design/slang-compiler-sys.md`).

### 18.2. Platforms and Minimum Supported Rust Version

The crate runs on x86_64 Linux with Vulkan, which the spike verified on a software and a hardware adapter. On Windows with Direct3D 12 and on macOS with Metal, it creates a device, loads modules, dispatches kernels, and samples textures in them in CI (Sections 3.8 and 3.9). The spike rendered on the two Vulkan adapters of the development machine only, and no code of `slango` renders yet (Section 3.9, SOA-5). wgpu 30.0.1 declares `rust-version` 1.87, below the workspace's minimum supported Rust version (MSRV) of 1.99.

### 18.3. Failure Modes

| Failure | Reported as | Recovery |
| --- | --- | --- |
| No adapter exists, such as on a machine without a GPU driver or software adapter | `NoAdapter` | Install a driver or lavapipe |
| The Slang shared library is not on the library path | The program stops before `main`, as `slang-compiler-sys` describes | Run through the recipes, or set the library path |
| The module does not compile | `Compile`, with Slang's diagnostics | Fix the shader |
| naga or the backend rejects a kernel | `UnsupportedShader`, with the backend's text as the source error (Section 10) | Avoid the construct (Section 3.5) |
| A kernel has a parameter that the walk rejects (Section 7.1) | `UnsupportedParameter` | Wait for the milestone that supports it |
| A kernel needs a feature the device lacks | `MissingFeature` | Create the wgpu device with the feature and pass it to `slango::wgpu::from_device_and_queue` (Section 12), or use another adapter |
| A call has a wrong name, value, buffer, texture, or sampler, or more workgroups than the device allows (Section 9.3) | `InvalidArgument`, naming the parameter | Fix the call |
| A call binds one buffer to a read-only and to a read-write parameter | `Backend`, with wgpu's text as the source error | Bind separate buffers |
| A buffer would be larger than the device allows | `InvalidArgument` | Create a smaller buffer |
| A render entry point has a parameter that the walk rejects, or `uniform` parameters of its own (Sections 7.4 and 7.5) | `UnsupportedParameter` | Use global uniforms, constant buffers, or parameter blocks, or wait for the milestone that supports the parameter |
| naga or the backend rejects a render pipeline, such as for `SV_VertexID` or varyings in another order | `UnsupportedShader`, with the backend's text as the source error (Section 10) | Avoid the construct (Section 3.9), or declare the varyings in one order |
| A draw has a wrong name, value, buffer, texture, target, or count (SLO-26) | `InvalidArgument`, naming the entry points and the parameter or target | Fix the draw |
| wgpu reports a validation or out-of-memory error (Section 10), or an error while a buffer, texture, or sampler is created or read | `Backend` | Report a validation error as a defect of `slango` |

### 18.4. Costs

Each load of a module costs about 135 ms for its global session, plus 8 to 11 ms for a small module (Sections 3.6 and 7.2).
### 18.5. Logs

`slango` emits no logs in M3 and M4 and has no logging dependency, as `slang-compiler` does. This answers Q-16 of the architecture design for `slango` through M4.

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
| SLO-2 | 4.1 | No unsafe code in M3 and M4, and every shader through the validated path. |
| SLO-3 | 5.1 | `Device`, `Module`, `Kernel`, and `Buffer<T>`, cheap to clone, holding their parents, created through options. |
| SLO-4 | 6 | wgpu types appear only in one public module. |
| SLO-5 | 7.1 | `load_module` compiles on the calling thread and releases every Slang object before it returns. |
| SLO-6 | 7.1 | The private layout snapshot holds only owned data and follows the walk rules of Section 3.3. |
| SLO-7 | 7.2 | A new global session for each load, and no hidden Slang state. |
| SLO-8 | 7.3 | Every shader module, layout, and pipeline is created at load, and any failure of a kernel fails the load. |
| SLO-9 | 7.3 | A naga rejection is caught in an error scope and returned as `UnsupportedShader` with naga's text. |
| SLO-10 | 8 | The element types of a buffer bound to a kernel come from a sealed trait, each with a test of its stride. |
| SLO-11 | 8 | `Buffer::read` blocks until earlier dispatches finish and returns the elements. |
| SLO-12 | 9.2 | The value trait is sealed and checks each value against its reflected type before writing. |
| SLO-13 | 9.2 | Plain values are written at reflected offsets, never as Rust struct bytes. |
| SLO-14 | 9.3 | Names, presence, values, strides, devices, and workgroup limits are checked before dispatch. |
| SLO-15 | 9.3 | Remaining wgpu validation errors are caught in an error scope and returned as `Backend`. |
| SLO-16 | 10 | One `Error` type with the eight kinds of Section 10. |
| SLO-17 | 11 | Every public type is `Send` and `Sync` without a manual implementation, asserted in a test. |
| SLO-18 | 12 | Request immediates when offered, and fail the load with `MissingFeature` when a kernel needs them. |
| SLO-19 | 5.3 | `RenderPipeline`, `Texture`, and `Sampler`, cheap to clone, holding their device, with options for pipelines and samplers. |
| SLO-20 | 7.4 | The load records vertex and fragment entry points without creating their shader modules or failing on them. |
| SLO-21 | 7.4 | The walk records each vertex attribute with its path, location, type, and usage. |
| SLO-22 | 7.5 | A render pipeline is created inside error scopes and fails as a whole. |
| SLO-23 | 7.5 | `uniform` parameters of a render entry point give `UnsupportedParameter`. |
| SLO-24 | 8.1 | `Texture2D` with a `float` result and `SamplerState` are parameters of every stage, and `Sampler2D` is rejected. |
| SLO-25 | 8.2 | The sealed trait `VertexElement`, a supertrait of `Element`, adds packed vectors of 2 and 3 components. |
| SLO-26 | 9.5 | A draw checks its parameters, attributes, targets, and counts before any work and reports `InvalidArgument`. |

## Change Log

- 2026-11-11: Recorded step 8 of Section 14, which added textures, samplers, `VertexElement`, and textures and samplers as arguments of a call, in Sections 1.6, 13.4, and 14, with the choices of Sections 8, 8.1, 8.2, 9.1, and 9.3, and removed the refusal of a kernel with a texture or a sampler from Sections 1.6, 8.1, and 14. Moved the mapping of element types to vertex formats to step 10 in Sections 8.2 and 14. Recorded in Section 3.9 how kernels sample textures on the three CI runners, defined the mark (run) for step 8 in Section 3.1, cited that evidence in Section 12, and narrowed SOA-5 to rendering in Section 15. Updated Sections 13.2, 18.2, and 18.3 for textures.
- 2026-11-11: Recorded step 7 of Section 14, which added the records of vertex and fragment entry points with their vertex attributes, and textures and samplers in the walk, in Sections 1.6, 7.1, 13.4, and 14. Settled in Section 7.4 one-component vector attributes, the rejection of `SamplerComparisonState`, the rejection of a render entry point that cannot be used, and the one difference of a load, the parameter that its error may name, stated in Section 8.1 that the load refuses a kernel with a texture or a sampler until step 8, and corrected Section 8.2. Recorded in Section 3.9 the system values and integer attributes that reflection reports and the cost of render entry points at load, defined the mark (run) for step 7 in Section 3.1, and narrowed SOA-6 in Section 15.
- 2026-11-11: Added the design of M4, rendering, as a draft: the facts of its spike in Section 3.9, the types and usage illustration in Sections 5.3 and 5.4, render entry points and pipelines in Sections 7.4 and 7.5, textures, samplers, and vertex and index buffers in Sections 8.1 and 8.2, the draw in Section 9.5, its tests and the windowed renderer example in Sections 13.4 and 13.5, steps 7 to 12 in Section 14, SLO-19 to SLO-26, and SOA-5 to SOA-7. Narrowed SLO-10 to buffers bound to a kernel, extended SLO-1, SLO-2, SLO-17, and Sections 1, 3.1, 3.3, 7.1, 7.2, 8, 10 to 12, and 15 to 18 to M4, deferred the configurable shader target in Section 1.5, and answered SOQ-1 in Section 7.2 and removed its row.
- 2026-11-11: Recorded step 6 of Section 14, which added the example `compute-kernel` and completed the crate documentation, and M3 as complete, in Sections 1.6 and 14, described the example as built in Section 13.3, recorded in Section 3.8 that it ran on all three CI runners, and defined the mark (run) for step 6 in Section 3.1.
- 2026-11-11: Recorded step 5 of Section 14, which added the call of a kernel, in Sections 1.6 and 14, with the choices of the builder in Section 9.1, `Copy` as a supertrait of `Value` in Section 9.2, the rules and order of the checks in Section 9.3, the dispatch sizes and the return of a dispatch in Section 9.4, the errors of a call in Sections 10 and 18.3, dispatch from several threads in Section 11, and the tests in Section 13.1. Recorded in Section 3.4 that dispatches confirmed the matrix rule, in Section 3.8 that kernels dispatch on all three CI runners and that wgpu rejects one buffer bound as read-only and read-write, and in Section 3.2 the bounds checks and per-thread error scopes of wgpu 30.0.1. Confirmed SOA-2 and, with a correction, SOA-3, and removed their rows from Section 15, answered Q-21 of the architecture design in Section 13.2, updated Sections 16, 17, and 18.2, and defined the mark (run) for step 5 in Section 3.1.
- 2026-11-11: Recorded step 4 of Section 14, which added `Buffer<T>` and the marshalling of plain values, in Sections 1.6 and 14, with the element types and the rules of buffers in Section 8, the value types and their check in Section 9.2, the identity of a device in Section 9.3, and the errors of buffers and values in Sections 10 and 18.3. Recorded in Section 3.4 the row-major matrix layout and the offsets of its rows, settled in Section 7.1 that marshalling needs no matrix layout mode, recorded in Section 3.2 that wgpu 30.0.1 compares devices of two requests as equal, confirmed SOA-4 in Section 3.3 and removed its row from Section 15, and defined the mark (run) for step 4 in Section 3.1.
- 2026-11-11: Recorded step 3 of Section 14, which added `Module`, `Kernel`, and the load of a module from a file or from source text with a compute pipeline for each kernel, in Sections 1.6, 7.1, 7.3, 13.1, and 14, with the choices that the design left open in Sections 7.1 and 7.3 and the mapping of wgpu's errors at load in Sections 10 and 18.3. Recorded in Section 3.8 that modules load on all three CI runners, narrowed SOA-2 to dispatch in Sections 13.2, 15, 16, and 18.2, moved 16-bit integers in Section 3.5 to the constructs that need a device feature, and defined the mark (run) for step 3 in Section 3.1.
- 2026-11-11: Recorded step 2 of Section 14, which added the private layout snapshot and its walk, in Sections 1.6, 7.1, 13.1, and 14, with the choices that the design left open and the supported and rejected parameters in Section 7.1. Recorded in Sections 3.3 and 3.4 that a system value has a uniform size of 0 and that the immediates round their size up to their alignment, and defined the mark (run) for step 2 in Section 3.1. Made the rows of Sections 10 and 18.3 for `UnsupportedParameter` cite Section 7.1.
- 2026-11-11: Recorded step 1 of Section 14, which added `Device`, `DeviceOptions`, `AdapterSelection`, the error type, and the module `slango::wgpu`, in Sections 1.6, 5.1, 13.1, and 14, added Section 3.8 with the adapters of the CI runners, recorded the licence changes in Section 4.2, corrected in Sections 12 and 18.3 how a caller requests another device feature, updated Sections 13.2 and 18.2 for the `gpu` tier in CI, narrowed SOA-2 to loading and dispatch, and answered SOQ-2 through `pollster` in SLO-1 and Appendix A.
- 2026-11-11: First draft, written after the spike of ARCH-18 as the first task of M3.
