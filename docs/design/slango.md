# `slango` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-18 |
| Applies to | `crates/slango`, the compute kernel example, and the windowed renderer example |

## Abstract

This document specifies `slango`, the high-level library for running Slang code on a GPU from Rust, as milestones M3 and M4 define it, with the compute kernel example and the windowed renderer example. `slango` calls the existing compute entry points of a Slang module on wgpu, with bindings built from Slang's reflection alone, and draws with existing vertex and fragment entry points into a texture. The design rests on the facts about Slang's SPIR-V on wgpu's validated shader path, loads a module into an owned layout snapshot that frees every Slang object, and uses no unsafe code. For the first release, it also specifies a small forward-compatibility pass: wgpu moves behind a default Cargo feature, the functions that return a raw wgpu object return an option, and every public option struct and enumeration stays non-exhaustive, so that a second backend can be added later without a breaking change where that is cheap. For M10, it specifies that second backend: slang-rhi behind the opt-in Cargo feature `rhi`, which excludes the feature `wgpu`, so that each build has exactly one backend, at parity with wgpu for compute kernels and for vertex and fragment pipelines, with every public type that holds a GPU object neither `Send` nor `Sync` in version 0.2.

## 1. Introduction

### 1.1. Problem

`slang-compiler` turns Slang source into SPIR-V and reports the bindings of each parameter, but running that code on a GPU takes much more. A program must open a device, build bind group layouts and a pipeline that match the shader exactly, lay out every plain value by the shader's rules, bind buffers, choose a dispatch size, and copy results back. A mistake in any of these steps fails at run time with a message about descriptor sets and offsets, or computes wrong numbers without any error.

### 1.2. Purpose

`slango` lets a Rust program load a Slang module, look up a compute kernel, set its parameters by name, dispatch it over a number of elements, and read the results, with every binding derived from reflection. For M4, it also lets the program draw with a vertex and a fragment entry point into a texture, with every binding and vertex attribute derived from reflection.

For M8, it settles the forward-compatibility pass that Section 15.11 of the architecture design records as the owner's decision, so that the opt-in backend `rhi` of M10 is an additive change where that is cheap (Sections 4.3, 5.5, and 6).

For M10, it lets a program run the same kernels and draws on slang-rhi, which M12 to M18 extend with pipeline kinds that wgpu does not offer (Section 14.4 of the architecture design).

This document implements Section 8 and milestones M3 and M4 (Sections 15.6 and 15.7) of `docs/design/architecture.md`, the pass of `slango` in milestone M8 (Section 15.11), and the backend `rhi` of milestone M10 (Section 15.13), and meets ARCH-15 to ARCH-19. It answers the questions and assumptions of that design that name this document, as Sections 14 and 15 record.

### 1.3. Audience

The primary readers are the coding agents that change `slango` and the reviewers who check their changes. A reader is expected to know Rust but not GPU programming. Section 2.1 defines the terms that such a reader needs.

### 1.4. Scope

This document covers the compute entry points that a Slang module already declares with `[shader("compute")]`, compiled to SPIR-V and loaded through wgpu's validated path. Their parameters are buffers, constant buffers, parameter blocks, and plain `uniform` values. It covers the public types, the backend boundary, loading, the call, errors, threading, device features, tests, continuous integration (CI), the example, and the dependency changes.

For M4, it also covers render pipelines built from the vertex and fragment entry points that a module already declares with `[shader("vertex")]` and `[shader("fragment")]`, drawing into a texture. Vertex attributes, uniforms, textures, and samplers are bound by name from reflection, as M3 binds the parameters of a kernel. Textures and samplers become parameters of kernels too.

It covers the windowed renderer example as well. The rendering design is in Sections 3.9, 5.3, 5.4, 7.4, 7.5, 8.1, 8.2, 9.5, 13.4, and 13.5, and it extends the other sections where they name M4.

For M8, it covers the Cargo feature `wgpu`, the accessors of the module `slango::wgpu`, and the exhaustiveness of the public types, in Sections 3.10, 4.3 to 4.5, 5.5, 6, and 13.6.

For M10, it covers the backend `rhi`: the facts it relies on, the owner's decisions, the feature, the structure of a crate with one backend for each build, loading, buffers, textures, the call, the draw, errors, threading, the tests, and the migration to version 0.2. The design of M10 is in Sections 3.11, 4.6, 4.7, 6.1 to 6.4, 7.6, 7.7, 8.3, 9.6, 9.7, 13.7, and 17.6, and it changes the other sections where they name M10. The additions to `slang-rhi` that the backend needs are in Section 11 of `docs/design/slang-rhi.md`.

### 1.5. Non-Goals

This document designs nothing beyond M3, M4, and the pass of M8. For M8, it designs no `rhi` backend, no backend trait, and no other abstraction over backends, which M10 designs (Section 4.3). These belong to later milestones: kernels generated around a plain Slang function with a positional call, broadcasting over array arguments, a tensor type with a shape, differentiation driven from Rust, and the unchecked shader passthrough path.

M4 leaves out these features:

- Generated shaders.
- Stages other than vertex, fragment, and compute, such as geometry, tessellation, and mesh stages.
- Multisampling, storage textures, texture arrays, cube maps, and mip-map generation.
- Several draws in one render pass, and render bundles.

`slango` offers no window, no surface configuration, and no frame loop, which belong to the caller (Section 5.3). Hot reload is no API of `slango`, and the windowed renderer example does it with the existing load (Section 13.5).

M10 leaves out these features:

- A surface or a window on the backend `rhi`. The windowed examples present through wgpu only (Section 4.6).
- A build of `slango` with both backends, and a choice of backend at run time (Section 4.7).
- A public module `slango::rhi` that returns the objects of slang-rhi (SLO-36).
- Pipeline kinds other than compute and vertex plus fragment, which M12 to M14 add on the backend `rhi` only.
- A device backend of slang-rhi other than Vulkan on Linux and D3D12 on x86_64 Windows (Section 7 of `docs/design/slang-rhi-sys.md`).

A shader target configurable per backend of wgpu is deferred with no milestone assigned. The backend `rhi` compiles to the target that slang-rhi chooses for its device (SLO-37). Slang's SPIR-V loads and runs through wgpu's validated path on Vulkan, Direct3D 12, and Metal (Section 3.8), and a second target would need the passthrough path and unsafe code (SLO-2).

### 1.6. Defects of Third-Party Components and Trusted Input

`slango` follows Hard Rule 14 of `AGENTS.md` and Section 1.6 of `docs/design/slang-compiler.md`, and applies the same stance to naga and wgpu. Shader source and file paths are trusted input. When naga rejects a module, `slango` reports naga's own text and does not screen modules for the constructs of Sections 3.5 and 3.9 in advance.

The checks of the arguments of a call stay, because a safe interface needs them (Section 9.3). A defect of Slang that `slango` finds is recorded in Section 3.12 of `docs/design/slang-compiler.md`, not here.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

Section 2.1 of `docs/design/architecture.md` defines adapter, buffer, compute dispatch, compute kernel, device, lavapipe, marshalling, shader passthrough, software adapter, SPIR-V, and wgpu. Section 2.1 of `docs/design/slang-compiler.md` defines descriptor set, layout, parameter category, thread group, type layout, and variable layout. This document adds these terms:

- backend: The GPU library through which a device of `slango` runs its kernels and draws, which is wgpu or slang-rhi. The backend `wgpu` is the one that the feature `wgpu` builds, and the backend `rhi` is the one that the feature `rhi` builds.
- bind group: The wgpu name for the set of bindings that a dispatch uses together. One bind group fills one descriptor set, and its bind group layout states the kind of each binding.
- binding: A numbered slot in a descriptor set through which a kernel reaches a buffer.
- colour target: The texture that a render pass draws its colour output into.
- device session: The Slang session of a device of slang-rhi, which `slang_rhi::Device::slang_session` returns, with the one target of that device (Section 3.11). The backend `rhi` does not compile in it (SLO-37).
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

## 3. Facts the Design Relies On

This section is non-normative. It states each fact about Slang, naga, wgpu, and the platforms that a later section relies on, once, with its consequence for the design.

### 3.1. Sources and Evidence

Each fact holds for wgpu 30.0.1 with the feature `spirv`, naga 30.0.1, and the recorded Slang, compiled with the session settings of Section 7.1. A fact is known in one of three ways:

- Run: A program or a test observed it. Such a fact holds on Vulkan with lavapipe and with a hardware GPU, and on the three adapters of Section 3.8 where it says so. A fact without a mark is of this kind.
- Read: A source states it, and the fact names the source with the mark (read).
- Inferred: It is concluded from other facts and not verified. Every inferred fact that the design depends on is a row of Section 14.

### 3.2. Facts About wgpu and naga

Slang's SPIR-V of compute kernels loads and runs through the validated path, with every bind group layout and pipeline built from reflection alone. This holds for read-only and read-write structured buffers, global `uniform` values, a `ConstantBuffer`, parameter blocks, `uniform` parameters of the entry point, and parameters that the code never uses.

naga also accepts these constructs on the validated path:

- Control flow of every kind, and generics with interface constraints.
- `[Differentiable]` functions with `bwd_diff` and `fwd_diff`.
- Atomics, matrices, uniform arrays, and `static const` tables.
- `inout` and `out` parameters, `bool` fields, `cbuffer`, and nested parameter blocks.

None of this needs unsafe code. The passthrough path also runs every such kernel, but it needs unsafe code and `Features::PASSTHROUGH_SHADERS`, so `slango` does not use it (SLO-2). Section 3.5 lists what the validated path rejects.

These facts about wgpu shape the ownership, the identity of a device, and the threading of `slango` (Sections 5.1, 9.3, and 11):

- `Device`, `Queue`, `Buffer`, `ComputePipeline`, and `BindGroup` are `Send` and `Sync`.
- All wgpu types are `Clone` through internal reference counting since wgpu 24 [WGPU-CHANGELOG] (read).
- Two devices from two device requests compare equal with `==`, so `==` cannot tell two wgpu devices apart.
- Error scopes are per thread, because `Device::push_error_scope` pushes onto the device's thread-local stack of error scopes (read, source of wgpu 30.0.1).
- `Device::create_shader_module` passes `ShaderRuntimeChecks::checked()`, whose `bounds_checks` is `true`. The documentation of that field promises that a shader can only read or write the accessible region of the buffers that a bind group binds, through checks that wgpu injects where the platform cannot enforce them itself (read, source of wgpu 30.0.1 and wgpu-types 30.0.1). `slango` creates every shader module through that method.

By default, Slang renames every SPIR-V entry point to `main`. With `vulkan_use_entry_point_name(Toggle::On)`, the SPIR-V entry point keeps the Slang name, as Slang's documentation also states [SLANG-SPIRV] (read).

### 3.3. Mapping From Reflection to wgpu

This table is the core of the crate. "DTS" is `ParameterCategory::DescriptorTableSlot`. A walk starts at `ProgramLayout::global_params_var_layout` and at `EntryPointLayout::var_layout`. Outside a parameter block, the binding of a variable is the sum of `offset(DTS)` along its path, and its set is the sum of `binding_space(DTS)` along its path.

| Slang declaration | What reflection reports | wgpu layout entry |
| --- | --- | --- |
| `RWStructuredBuffer<T>` | `TypeKind::Resource`, access `ReadWrite`, category DTS, binding type `MutableRawBuffer`. Element stride from `element_type_layout().stride(Uniform)`: 4 for `float`, 16 for `float4`, 32 for `{float3, float, float2, uint}` | Storage buffer, not read-only, with `min_binding_size` equal to the stride |
| `StructuredBuffer<T>` | The same, with access `Read` and binding type `RawBuffer` | Read-only storage buffer |
| `ConstantBuffer<S>` or `cbuffer` | `TypeKind::ConstantBuffer`, category DTS. Binding from `container_var_layout()`, size from `element_var_layout().type_layout().size(Uniform)`, field offsets from `offset(Uniform)` | Uniform buffer with `min_binding_size` equal to the size |
| Global `uniform` plain value | The global scope becomes an unnamed `TypeKind::ConstantBuffer` at DTS 0, and every other global binding moves up by one. Fields at `offset(Uniform)` with std140 rounding | One uniform buffer at set 0, binding 0, that `slango` owns and fills |
| `uniform` parameter of the entry point | `TypeKind::ConstantBuffer` with category `PushConstantBuffer` and binding type `PushConstant`, std430 offsets | No binding. The pipeline layout's immediate size, then the immediates of the compute pass. Needs `Features::IMMEDIATES` (Section 12) |
| `ParameterBlock<S>` | `TypeKind::ParameterBlock`, category `SubElementRegisterSpace`. Its set is the sum of `offset(SubElementRegisterSpace)` along the path, including each element variable. Bindings restart at 0, with an implicit uniform buffer first when `S` has plain values. The parent's `descriptor_sets()` leaves the block's set out | One more bind group layout at that set |
| `ParameterBlock` inside a `ParameterBlock` | The inner set is the outer set plus 1, by the same sum | Two bind group layouts |
| System values such as `SV_DispatchThreadID` | The type layout of `uint3 id : SV_DispatchThreadID` has a uniform size of 0 | Nothing. The walk skips every plain value of uniform size 0 |
| `[numthreads(x, y, z)]` | `compute_thread_group_size()` returns `[x, y, z]` | Workgroup counts derived from element counts (Section 9.4) |
| A parameter that the code never uses | Still listed with its binding | The SPIR-V drops it. The layout is accepted with or without its entry |
| `Texture2D`, `SamplerState`, `RWTexture2D` | Binding types `Texture`, `Sampler`, `MutableTexture` | `slango` builds the first two (Sections 3.9 and 8.1), and the walk rejects the third (Section 7.1) |

For each element type of `Buffer<T>`, the reflected stride of a structured buffer of the matching Slang type equals the stride that `slango` uses for `T`. The size of every buffer, its length times that stride, is therefore a multiple of the stride.

### 3.4. Layout of Plain Values

Slang's default layout of plain values differs from the layout of any Rust struct with the same fields, so `slango` writes every plain value at its reflected offset and never relies on the layout of a Rust struct (SLO-13). These examples show the difference:

- `struct Params { float scale; float3 offset; uint count; float2 pair; }` in a constant buffer has its fields at offsets 0, 16, 28, and 32, with size 48.
- `float c[5]` in a constant buffer takes 80 bytes, with a stride of 16.
- `float4x4`, `float3x3`, `float2x3`, and `float3` in sequence take offsets 0, 64, 112, and 144, with size 160.
- A `bool` takes 4 bytes.
- As `uniform` parameters of an entry point, `float`, `float3`, `float4x4`, a structure of a `float` and a `uint`, and `bool` take offsets 0, 16, 32, 96, 100, and 104 in the immediates. The last value ends at 108, and the block has size 112, because std430 rounds the size of the block up to its alignment of 16.

Slang's default matrix layout is row-major, as `defaultMatrixLayoutMode` in `slang.h` states, and `slango` sets no matrix layout. In that layout, a matrix of R rows and C columns is R rows, each starting at a multiple of the matrix stride. Row r starts at the value's offset plus r times its extent divided by R, and column c of a row follows at 4 bytes times c.

The extent is the reflected size of the value, or the array stride for a matrix inside an array. It is R times 16 bytes in a uniform buffer and R times 8 or 16 bytes in the immediates, for every R and C from 2 to 4. This rule holds for Slang's SPIR-V and not for DXIL, so from M10 the walk records the row stride that reflection reports, and marshalling uses it on both backends (SLO-46, Section 3.11).

Kernels that read a `float2x3` and a `float3x2` in the immediates and a `float2x3` in a constant buffer, element by element, read them in row order on every adapter of Section 3.8. The `slangc` command line defaults to the other layout, so an experiment with it must pass `-matrix-layout-row-major` to match what `slango` compiles.

### 3.5. Known Limits of the Validated Path

naga rejects these valid Slang constructs, with the same text on every adapter. `slango` does not screen for them (Section 1.6).

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

`double`, `int64_t`, and `half` load only on a device with the matching feature. naga otherwise names the missing capability, such as `Using 'f64' values requires the 'naga::valid::Capabilities::FLOAT64' flag`.

16-bit integers belong with them. A kernel with a local `int16_t` on a device without the 16-bit integer feature fails when its shader module is created. The validation error names the capability `SHADER_INT16`.

### 3.6. Costs

The global session dominates the cost of a load, which is why Section 7.2 bounds how often a program can afford to load a module. Each figure is an order of magnitude:

- Creating a global session takes about 100 to 135 ms, and releasing it about 25 ms.
- A session, load, link, code generation, and reflection of a small module take about 10 ms after that.
- A naga shader module, the layouts, and a pipeline each take under 1 ms, and so does a dispatch with its wait. The first dispatch of a pipeline on lavapipe is the exception, at 15 to 20 ms.
- A reload of a module with a vertex and a fragment entry point, global uniforms, a constant buffer, and a parameter block takes about 155 ms with a new global session and about 12 ms with the global session kept. The work of wgpu in it takes under 1 ms.

### 3.7. Versions and Prior Art

Slint 1.18.1 declares the features `unstable-wgpu-29` and `unstable-wgpu-30`, and wgpu 30.0.1 declares `rust-version` 1.87 (read, `cargo info`). SlangPy's documentation shows kernels generated around plain functions, broadcasting with `map()`, tensors, and differentiation through `bwds` [SLANGPY] (read). Its kernel cache and device layer were not read in its source.

vulkano uses `Arc` ownership with children that hold their parents, structs of creation options, and validated calls with named unchecked variants. A maintainer describes `GpuFuture` as hard to chain correctly while building an experimental replacement [VULKANO-2540] (read).

### 3.8. Adapters of the CI Runners

The `gpu` tests run on one adapter on each hosted runner, and each adapter offers `Features::IMMEDIATES`:

| Runner | Adapter | Backend | Device type | `max_immediate_size` |
| --- | --- | --- | --- | --- |
| `ubuntu-latest`, with `mesa-vulkan-drivers` | lavapipe, named `llvmpipe (...)` | Vulkan | `Cpu` | 256 bytes |
| `windows-latest` | WARP, named `Microsoft Basic Render Driver` | Direct3D 12 | `Cpu` | 128 bytes |
| `macos-latest` | `Apple Paravirtual device` | Metal | `IntegratedGpu` | 256 bytes |

lavapipe is the only adapter of the Linux runner, so the default selection also finds it there. Metal offers no software adapter, so `AdapterSelection::Software` returns `ErrorKind::NoAdapter` on the macOS runner (Sections 5.1 and 13.1).

On all three adapters, modules load, and kernels dispatch with correct results for every kind of parameter of Section 7.1 and every element type of `Buffer<T>`. A kernel that calls `frexp` fails with the same backend text on all three. One buffer bound to a read-only and to a read-write parameter of one call fails wgpu's validation on every adapter (Section 9.3).

### 3.9. Facts for Rendering

This section is non-normative. A fact that names the adapters of Section 3.8 holds on them as well as on the two Vulkan adapters of Section 3.1.

A vertex and a fragment entry point, compiled as Section 7.1 compiles a kernel, into one SPIR-V module for each entry point with `vulkan_use_entry_point_name(Toggle::On)`, load through the validated path and draw the expected pixels into an `Rgba8Unorm` texture. No unsafe code and no other session or target option is needed. `EntryPointLayout::stage` returns `Stage::Vertex` and `Stage::Fragment`, and every fact that rendering needs comes through the safe API of `slang-compiler`.

This table states what reflection reports for the inputs and resources of render entry points, and what wgpu needs for each:

| Slang declaration | What reflection reports | wgpu |
| --- | --- | --- |
| A field of a structure parameter of the vertex entry point, such as `float3 position : POSITION` in `VIn input` | Through `EntryPointLayout::parameter_layouts` and the fields of the parameter's type: the name, the semantic, the scalar type and element count, and the location as the sum of `offset(VaryingInput)` along the path. The locations equal those in the SPIR-V and from `slangc` | A vertex attribute at that location, with the format of the scalar type and count, such as `Float32x3`. Reflection gives no stride, offset, or step mode |
| A vertex attribute that the code never reads | Still listed at its location. `Metadata::is_parameter_location_used` with `ParameterCategory::VaryingInput`, space 0, and the location returns `false`. The SPIR-V drops it and renumbers nothing | Accepted with or without the attribute |
| Attributes that step per instance, such as `float2 offset : INSTANCE_OFFSET`, in the same input structure | Listed like the other attributes, so reflection does not say which attributes step per instance | Correct from a second buffer with `VertexStepMode::Instance`. The step mode is part of the render pipeline (read, `VertexBufferLayout::step_mode` in wgpu 30.0.1) |
| `SV_Position` as a vertex output or a fragment input | No category and no location | Nothing |
| Varyings | Locations in declaration order, within each entry point's own structure | Matched by location. When two structures declare the same varyings in another order, `create_render_pipeline` fails with a validation error that starts `Error matching ShaderStages(FRAGMENT) shader requirements against the pipeline` |
| A global `uniform`, a `ConstantBuffer`, and a `ParameterBlock`, used by two entry points | The bindings of Section 3.3, unchanged. The metadata of each entry point reports which bindings that stage uses | One layout entry for each binding, visible to the stages that use it. A narrower visibility fails `create_render_pipeline` with `Visibility flags don't include the shader stage`, and a wider one is accepted |
| `uniform` parameters of the vertex and of the fragment entry point | One push constant block for each entry point, each at byte offset 0 | The two blocks alias: the fragment stage reads the bytes written for the vertex stage. `RenderPass::set_immediates` takes no stage (read, wgpu 30.0.1) |
| `Texture2D<float4>` | `TypeKind::Resource`, base shape `Texture2d`, not combined, access `Read`, result type `vector<float,4>`, category DTS | A texture entry with `sample_type: Float { filterable: true }` and `view_dimension: D2` |
| `SamplerState` | `TypeKind::SamplerState`, category DTS, at its own binding | A `Filtering` sampler entry |
| `Sampler2D` | `TypeKind::Resource`, combined, with one binding | Rejected by naga, as the last table of this section shows |

The conventions of a draw are the same on every adapter of Section 3.8, so neither Slang nor `slango` flips anything:

- Neither axis is flipped. The y axis of the vertex position points up, so a triangle in the upper left of clip space lands in the upper-left rows of the target.
- Counter-clockwise triangles are front faces, so `CullMode::Back` discards a clockwise triangle.
- The depth range is 0 to 1. A `Depth32Float` depth target with the comparison `Less` keeps the nearer, smaller depth in either order of drawing, and geometry with a depth outside the range is clipped.
- `SV_Position` in the fragment stage has pixel centres at 0.5, with y pointing down from the top row.
- The texture coordinates (0, 0) sample the top-left pixel of a texture in a draw.
- A transform written as rows in Rust translates by the expected pixels with `mul(transform, position)`, as the matrix rule below states.
- `int` and `uint` vertex attributes of one to four components arrive unchanged through `nointerpolation` varyings, including values near the 32-bit limits.
- wgpu rejects one texture that is both sampled and the target of one draw with a validation error about conflicting usages, and the draw returns `ErrorKind::Backend`.

Compute kernels that sample textures with `SampleLevel` behave alike on every adapter of Section 3.8:

- The texture coordinates (0, 0) sample the top-left pixel, with no flipped axis.
- A kernel reads a pixel of `Bgra8Unorm` or `Bgra8UnormSrgb` as red, green, blue, and alpha.
- A channel of 128 in an sRGB format samples to within 1/255 of the 0.2159 that the sRGB formula gives, and to 128/255 in the matching non-sRGB format.
- Linear filtering gives the interpolated value within a tolerance of 0.01.
- `ClampToEdge`, `Repeat`, and `MirrorRepeat` give the expected pixels at coordinates outside the range 0 to 1.
- All five formats of Section 8.1 are created with their usages without a device feature.

The matrix rule of Section 3.4 holds for a `float4x4` global uniform. The Rust value `[[1, 0, 0, tx], [0, 1, 0, ty], [0, 0, 1, 0], [0, 0, 0, 1]]`, written row by row, translates by (tx, ty) with `mul(transform, v)` and does not translate with `mul(v, transform)`. A math type that stores columns, such as the arrays that glam's `Mat4::to_cols_array_2d` returns, therefore needs `mul(v, transform)` or a transpose.

naga rejects these render constructs, with the same text on every adapter, in the form of Section 3.5:

| Construct | naga's error text |
| --- | --- |
| `SV_VertexID` or `SV_InstanceID` in the vertex stage | `Shader '' parsing error: UnsupportedCapability(DrawParameters)` |
| `Sampler2D` | `Shader '' parsing error: InvalidId(12)` |
| An integer varying without `nointerpolation` | `Shader validation error: Entry point vsMain at Vertex is invalid`, followed by `@interpolate(flat) must be explicitly specified for integer I/O` |

`SV_VulkanVertexID` and `SV_VulkanInstanceID` load instead, and the instance number includes the first instance of the draw. A `nointerpolation` integer varying loads. Two colour targets, `discard`, `ddx` and `ddy`, `SV_Position` read in the fragment stage, and attributes that step per instance also work.

`SV_VulkanVertexID` and `SV_VulkanInstanceID`, in a structure or as separate parameters, have no category `VaryingInput` and take no location. The attributes around them therefore keep consecutive locations, and `slangc` decorates them as built-ins without a location. An `int`, `uint`, or `uint2` attribute reflects as a 32-bit signed or unsigned integer with its component count.

Render pipelines are created from Slang's vertex and fragment SPIR-V on every adapter of Section 3.8, including pipelines whose vertex entry point has `int`, `uint`, `uint2`, and `int3` attributes. Varyings of another type at one location fail at `create_render_pipeline` with the text that starts `Error matching ShaderStages(FRAGMENT) shader requirements against the pipeline`. `SV_VertexID` fails at `create_shader_module` with `UnsupportedCapability(DrawParameters)`.

Both texts are the same on every adapter.

### 3.10. Facts for the Backend Feature

This section is non-normative. Unless a fact names another state or toolchain, its facts hold for the code of the crate before the pass of M8, for Cargo and Clippy of Rust 1.99.1, and for the configuration of the workspace. They shape Sections 4.3, 5.5, and 6.

These facts concern the code:

- Five public functions name a wgpu type in their signatures, and all five are in `src/wgpu.rs`: `from_device_and_queue`, `device`, `queue`, `texture`, and `texture_from_view` (read). No other public item names a wgpu type. The kind `ErrorKind::Backend` carries an error of wgpu only as a source error of type `dyn Error`.
- `Device`, `Buffer<T>`, `Texture`, `Sampler`, `Kernel`, and `RenderPipeline` hold wgpu objects, `Module` holds kernels, and `Call` and `Draw` record wgpu commands (read). The items that hold no wgpu object, which are `Error`, `ErrorKind`, the options types, the sealed traits, and the crate-private layout snapshot, create nothing on a GPU.
- `pollster` is used only in `src/device.rs`, to wait on wgpu's futures (read).
- Of the dependents in the workspace, `slang-slint` calls `slango::wgpu::texture` in `View::to_image` and `slango::wgpu::from_device_and_queue` in `device`. The example `windowed-renderer` calls `from_device_and_queue`, `device`, `queue`, and `texture_from_view`. The code that `slango-codegen` generates names no wgpu item (read).

These facts concern the tools:

- A feature that names an optional dependency only as `dep:name` leaves that dependency without an implicit feature of its own name, so a feature `wgpu = ["dep:wgpu", "dep:pollster"]` is valid (read, the Cargo reference, "Optional dependencies"). Section 4.3 of `docs/design/slango-macros.md` states how Cargo resolves the features of host and target dependencies.
- `compile_error!` stops the compilation with its message (read, the documentation of `std::compile_error`).
- The workspace lint table sets `clippy::exhaustive_enums` and `clippy::exhaustive_structs` to `warn`, every member inherits it, and `check-clippy` turns every warning into an error (read, `Cargo.toml` and the `justfile`). In `slango`, only the items of `__private` carry an exception to either lint, so the lints do not fire on a struct whose fields are all private (inferred from the passing check).
- `clippy::unnecessary_wraps` reports a public function that always returns `Some`, because the workspace sets `avoid-breaking-exported-api = false` (read, Section 4.5 of the coding standard).
- `check-features` runs cargo-hack with `--feature-powerset`, which also checks each package with none of its features. For `slango` with the feature `wgpu` and with `vendored` excluded, cargo-hack 0.6.46 lists three combinations: `--no-default-features`, `--features wgpu`, and `--features default`, each after `--no-default-features`. It treats `default` as a feature of the powerset (read, by `--print-command-list`). TOOL-48 of `docs/design/tooling.md` states which option of cargo-hack leaves out the first combination.
- After the pass, `cargo check -p slango --no-default-features` exits with status 101, and the error of SLO-29 is its only error, with Rust 1.99.1 and with Rust 1.100.0. With 1.99.1, it prints no warning. With 1.100.0, Cargo prints the warning "unused dependency `slang-compiler`" after the error. The build compiles `slang-compiler` and runs the build script of `slang-compiler-sys` before it reaches `slango`, so it needs `VULKAN_SDK` and libclang.

### 3.11. Facts for the Backend `rhi`

This section is non-normative. It states the facts about `slang-rhi`, slang-rhi, and Slang that the backend `rhi` relies on, with the place that establishes each. A fact marked (read) was read in the source of slang-rhi at the recorded commit, under `external/slang-rhi` of the Slang tree, or in the Slang source of the recorded commit under `source/slang`. A fact marked (run with `slangc`) was observed with the `slangc` of the recorded Slang. Sections 3.3, 6.1 to 6.4, 7.6, 7.7, 8.3, 9.6, and 9.7 rely on them.

These facts concern the device and the compile:

- `slang-rhi` wraps the compute path of slang-rhi in M9, and no type of it that holds a native object is `Send` or `Sync`, because slang-rhi documents no threading rule (RHI-13 of `docs/design/slang-rhi.md`).
- A device of `slang-rhi` compiles with the global session that its caller passes (RHI-5), and its device session has exactly one target, the compile target of the device: SPIR-V on Vulkan and DXIL on D3D12. slang-rhi creates that session itself with `forceGLSLScalarBufferLayout` set (`src/slang-context.h:87`, read) and the row-major matrix layout (`include/slang-rhi.h:3229`, read). The crate sets no target (Section 3 of `docs/design/slang-rhi.md`).
- slang-rhi takes the Slang session from the linked program that a shader program receives, and generates the code for target 0 of that session (`src/shader.cpp:58`, read). Only the crate rule RHI-7 of `docs/design/slang-rhi.md` limits a shader program to a program of the device session, and RHI-30 of that design adds sessions that reproduce the settings of the device session, whose programs become shader programs as well. slang-rhi generates the code of a program when the pipeline is created.
- In one session, a second load of source under a module name that the session holds returns the cached module when the source is the same, and fails with "a module named '...' is already loaded from different source contents in this session" otherwise (`slang-session.cpp:297-317`, read). An `import` resolves first to a module that the session holds under the imported name (`slang-session.cpp:1543`, read). So one session for all loads of a device would fail a reload of a changed file and two files of the same stem, and would reuse a stale imported module, which is why each load creates its own session (owner decision 6 of Section 4.6).
- The metadata of an entry point, from which the walk reads the usage of each parameter, comes from `getEntryPointMetadata`, which generates the code of the entry point (`slang-linkable.cpp:342`, read). So a load generates code for the target of its session, and on DXIL that needs `dxcompiler` (Section 7.6).
- A compute pipeline of a kernel with a `ConstantBuffer` and a global `uniform` fails on D3D12 (RKB-2 of `docs/design/slang-rhi.md`).
- On Windows, a program that creates a pipeline needs `dxcompiler.dll` on its `PATH` (Section 7.3 of `docs/design/slang-rhi-sys.md`). On Linux, the only `libdxcompiler.so` in reach is the one in `$VULKAN_SDK/lib`, and the vendored Slang of Linux has no DXIL support (SCS-82 of `docs/design/slang-compiler-sys.md`).
- `slang-rhi-sys` builds on Linux with the GNU C library and on x86_64 Windows with the Microsoft toolchain, and its build script stops the build on every other target before anything is cloned (RSYS-13 of that design).

These facts concern layouts, each from `slangc -reflection-json` with `-matrix-layout-row-major` on three targets: SPIR-V with Slang's default layout, SPIR-V with `-force-glsl-scalar-layout`, and DXIL with the profile `sm_6_0` (run with `slangc`):

- The scalar layout of SPIR-V and the layout of DXIL agree for every structure that was measured, in size and in every field offset. They differ from the default layout of SPIR-V, the std430 layout that the backend `wgpu` uses, as this table shows:

  | Structure | std430 size | Scalar SPIR-V and DXIL size | Field offsets that differ |
  | --- | --- | --- | --- |
  | `Particle` of the example `typed-kernel` | 32 | 28 | None, the stride only |
  | `Particle` of the tests of `slango-macros` | 48 | 32 | `id`, 32 against 28 |
  | `Tail` | 16 | 12 | None, the stride only |
  | `Gap` | 32 | 20 | `colour`, 16 against 4 |
  | `Grid` | 48 | 40 | `mask`, 32 against 24 |
  | `Nested` | 80 | 48 | Every field after `position` |
  | `Wide` | 32 | 32 | None |

- For a structure whose fields are 32-bit scalars, vectors, arrays, and such structures, the std430 size equals the packed size exactly when std430 adds no padding, and then every offset is the same. A structure of the wrong layout therefore always has the wrong stride (inferred from the rules of both layouts).
- On DXIL, the rows of a matrix in a uniform buffer are 16 bytes apart and the last row is not padded, so a global `float3x3` has a size of 44, a `float2x3` of 28, and a `float3x2` of 40. The rule of Section 3.4, that row `r` starts at the offset plus `r` times the extent divided by the number of rows, gives other offsets on DXIL. The scalar layout of SPIR-V meets the rule, with 36, 24, and 24.
- `Element` holds only 4-byte scalars and vectors of them (Section 8), whose stride is the same in every layout.

These facts concern the vertex stage on D3D12:

- `slangc` with the target `dxil` and the profile `sm_6_0` rejects `SV_VulkanVertexID` with "invalid semantic 'SV_VulkanVertexID' for vs 6.0" (run with `slangc`), and naga rejects `SV_VertexID` (Section 3.9). So no shader that reads the vertex or instance index runs on both the backend `wgpu` and the backend `rhi` on D3D12.
- A Slang varying without a semantic does not compile to DXIL, with "Semantic must be defined for all outputs" (run with `slangc`), and the D3D12 backend of slang-rhi copies the semantic name of an input element as it is given, including a null name (`src/d3d12/d3d12-device.cpp:1699-1709`, read).

These facts concern the parameters:

- `ShaderObject::offset_of` finds the top-level parameters of an object by name, and `set_data` writes the uniform data of one such parameter. The root object of a program holds the global parameters of the program as one structure, unwrapped from its container, and the entry point object holds the `uniform` parameters of the entry point (RHI-10, Section 3 of `docs/design/slang-rhi.md`).
- In M9, the fields of a `ConstantBuffer` or `ParameterBlock` parameter cannot be set, and textures and samplers cannot be bound (Section 11 of `docs/design/slang-rhi.md`).
- `IShaderObject::getObject` returns the object of such a parameter, by pointer and reference only (`include/slang-rhi.h`, read).

These facts concern rendering, all read in the headers and the backends of slang-rhi:

- Creating a texture, a texture view, a sampler, an input layout, and a render pipeline, beginning a render pass, binding a render pipeline, setting the render state, drawing with and without indices, and reading a texture back all take their descriptors by pointer or reference. None of these methods is a placeholder slot of `slang-rhi-sys` (RSYS-10 of `docs/design/slang-rhi-sys.md`). Initial texture data reaches `createTexture` through its argument `initData`, so no command of the placeholder slots, such as `uploadTextureData`, is needed.
- The Vulkan backend sets every viewport with a negative height, from `originY + extentY` (`src/vulkan/vk-command.cpp:845-851`), so clip space points y up and the framebuffer starts at the top row on Vulkan as on D3D12.
- `FrontFaceMode::CounterClockwise` is the default and maps directly on both backends (`src/vulkan/vk-utils.cpp:815-825`, `src/d3d12/d3d12-pipeline.cpp:270`). A viewport passes its depth range of 0 to 1 unchanged.
- The Vulkan backend always enables depth clamping and ignores `RasterizerDesc::depthClipEnable` (`src/vulkan/vk-pipeline.cpp:633`), while D3D12 honours it. Geometry with a depth outside the range 0 to 1 is therefore clamped on Vulkan instead of clipped.
- `RenderState` holds at most 16 vertex buffers, 16 viewports, and 16 scissor rectangles in fixed arrays (`include/slang-rhi.h:2331`).
- The Vulkan backend does not clear a texture that `createTexture` creates without initial data (`src/vulkan/vk-texture.cpp:290-470`), while wgpu zeroes every new texture. `slang-rhi` refuses initial data for a depth format (RHI-23 of `docs/design/slang-rhi.md`).
- slang-rhi keeps a reference to each view of a render pass and to each vertex and index buffer of a draw until the work completes (`src/command-list.cpp:84-101`, `src/command-buffer.cpp:72-73`), so a draw that returns without waiting is sound.

The headers of slang-rhi document none of these conventions, so the conventions of a draw on the backend `rhi` are assumed until the `gpu` tests of Section 13.7 run them (SOA-9).

## 4. Crate Structure and Dependencies

### 4.1. Package and Dependencies

SLO-1. `slango` MUST depend on `slang-compiler`, and, as optional dependencies that only the feature `wgpu` enables (SLO-28), on wgpu of major version 30 with the feature `spirv` and on `pollster`, which blocks on wgpu's futures. As an optional dependency that only the feature `rhi` enables, it MUST depend on `slang-rhi` (SLO-33). It MUST depend on no other crate. The list stays short because every dependency runs on every user's machine (DEP-2).

`slango` needs `pollster`, because wgpu 30 offers no blocking form of `Instance::request_adapter`, `Instance::enumerate_adapters`, or `Adapter::request_device`, so `slango` waits on them with `pollster::block_on`. Version 30 is the newest wgpu that the current Slint release supports (ARCH-19, Section 3.7).

The implementation re-checks the version at each Slint release, and an upgrade is a breaking release of `slango` (DEP-3).

SLO-27. `slango` MUST have exactly three Cargo features, `wgpu` of SLO-28, `rhi` of SLO-33, and `vendored = ["slang-compiler/vendored"]`, the last two off by default, and MUST NOT read `vendored` in its own code. The feature `vendored` only forwards the choice of the vendored route to `slang-compiler`, so that a user who depends on `slango` alone can link a statically built Slang (Section 19.1 of the architecture design, SLC-33 of `docs/design/slang-compiler.md`). No `gpu` test of `slango` runs on the vendored route. The forward is checked through the commands of `test-vendored` and `check-vendored` for `slango-macros` and `slang-slint`, which both reach `slang-compiler` through `slango` (TOOL-7 of `docs/design/tooling.md`).

SLO-2. `slango` MUST contain no unsafe code, and the backend `wgpu` MUST load every shader through the validated path only.

The validated path runs every kernel that M3 needs and every render shader that M4 needs without unsafe code (Sections 3.2 and 3.9), so passthrough would add unsafe code for no need. This answers Q-19 of the architecture design for `slango` through M4.

The backend `rhi` calls only the safe API of `slang-rhi`, and Section 11 of `docs/design/slang-rhi.md` adds to that crate what the backend needs. So M10 keeps `slango` free of unsafe code, and the unsafe code of the backend stays in `slang-rhi`, whose design document records its reason (RHI-2 of that design).

### 4.2. Licences and Duplicate Versions

wgpu's dependency tree brings crates under the licences `MIT`, `Unicode-3.0`, and `Zlib`, and duplicate versions of `foldhash`, `hashbrown`, and `syn`. The licence allow-list of `deny.toml` holds the three licences, and its `[bans]` table holds a `skip` entry with a reason for each duplicate version, as TOOL-24 and TOOL-26 require.

`slango` is under `MIT OR Apache-2.0`, which it inherits from the workspace (ARCH-36 of the architecture design). None of these crates is under the GPL, as TOOL-34 of `docs/design/tooling.md` requires of the dependencies of `slango`. Section 8 of `docs/design/tooling.md` records which crates need each licence, the evidence for it, and the reason for each skip.

### 4.3. The Backend Feature

SLO-28. The manifest of `slango` MUST declare `default = ["wgpu"]` and `wgpu = ["dep:wgpu", "dep:pollster"]`, with `wgpu` and `pollster` as optional dependencies. The crate root MUST put every module declaration and every re-export, including the hidden module `__private`, behind the condition that exactly one backend feature is on, `#[cfg(any(all(feature = "wgpu", not(feature = "rhi")), all(feature = "rhi", not(feature = "wgpu"))))]`, and the public module `wgpu` and the private backend module of wgpu behind `#[cfg(all(feature = "wgpu", not(feature = "rhi")))]` (SLO-34). Otherwise a program that enables only the backend `rhi` still builds wgpu, and a user cannot leave it out without a breaking release.

The feature gates the whole crate and not only the module `slango::wgpu`, because every type that runs on a GPU holds the objects of its backend (Section 3.10). The five functions of `slango::wgpu`, which are `from_device_and_queue`, `device`, `queue`, `texture`, and `texture_from_view`, are the only public items that name a wgpu type, and SLO-4 keeps it so. Three of them, `device`, `queue`, and `texture`, are the accessors of SLO-31.

SLO-29. When the features `wgpu` and `rhi` are both off or both on, the crate root of `slango` MUST stop the compilation with one `compile_error!` and exactly this message: "slango needs exactly one backend: keep its default Cargo feature `wgpu`, or turn the default features off and enable its Cargo feature `rhi`, but not both". With SLO-28, it is then the only error of the build of `slango` (Section 3.10). Otherwise a build with `default-features = false` fails with hundreds of unresolved names, and a build with both features fails with conflicting definitions, neither of which says what to change. The second exception of DEP-1 of the coding standard permits this one stop.

The two features are mutually exclusive by the owner's decision 5 of Section 4.6. With both on, Cargo still builds wgpu and, through `rhi`, Slang and slang-rhi from source before it reaches the error, because the features enable those dependencies. The design accepts that cost of a build that cannot succeed.

This is a choice of the design. The other option, a crate that compiles without a backend to its errors, options, and loading, was rejected for two reasons:

- It would need a `cfg` in each module that holds a wgpu object, which is most of the crate (Section 3.10).
- The part that remains could create no device, so it could run nothing.

A compile error is honest about a crate without a backend. M10 changes the condition from "no `wgpu`" to "not exactly one of `wgpu` and `rhi`", which breaks no build that compiled before, because no build could enable `rhi` before M10.

`cargo check -p slango --no-default-features` therefore fails with that one error. With Rust 1.100.0, Cargo also warns that `slang-compiler` is unused (Section 3.10). The design accepts that warning, because the build fails with the error anyway, and keeps `slang-compiler` a required dependency.

`check-features` checks every combination of the features of each package (Section 3.10), so the recipe leaves out the one combination of `slango` without `wgpu`, the one with no features, and checks every combination that contains `wgpu`. It also leaves out `rhi`, which builds Slang and slang-rhi from source and cannot be combined with `wgpu`, and the recipes of the backend `rhi` check the build with `rhi` alone instead (Section 4.7). TOOL-48 of `docs/design/tooling.md` specifies the recipe.

SLO-30. Every member of the workspace that depends on `slango` MUST name the backend feature of `slango` that it builds with, and MUST NOT rely on the default feature alone. Otherwise a later change that turns the default features off, in the entry of `[workspace.dependencies]` or elsewhere, removes the backend from that member without an error at the place of the change. The members write these entries:

| Member | Entry |
| --- | --- |
| `slang-slint` | `slango = { workspace = true, features = ["wgpu"] }` |
| `slango-codegen` | The entry of SLM-2 of `docs/design/slango-macros.md`, with `features = ["wgpu"]` added |
| `slango-macros`, as a development dependency | `slango = { workspace = true, features = ["wgpu"] }` |
| The examples package | `slango = { workspace = true, default-features = false }`, with its own features `wgpu = ["slango/wgpu", ...]`, the default, and `rhi = ["slango/rhi"]` (Section 13.7) |

The entry of `slango` in `[workspace.dependencies]` keeps the default features. `slang-slint` needs wgpu for good, because it passes wgpu objects between `slango` and Slint, so it cannot be built with the backend `rhi` (owner decision 5 of Section 4.6). The windowed programs of the examples need wgpu for the same reason, and Section 13.7 states how the examples package builds with `rhi` without them.

`slango-codegen` needs a backend only so that its copy of `slango` compiles, because `precompile_file` creates no device. That copy is a host dependency of the macro, so its feature does not reach the `slango` of the user's program (Section 4.3 of `docs/design/slango-macros.md`).

The `# Features` section of the crate-level documentation of `slango` names all three features, as Section 7.8 of the writing guide requires and `check-feature-docs` checks. It describes `wgpu` with these points:

- It is on by default.
- It runs the kernels and draws of every device on wgpu 30, and it provides the module `slango::wgpu`.
- The crate needs exactly one backend, so a build with neither `wgpu` nor `rhi`, or with both, stops with the error of SLO-29.

It describes `rhi` with the points of Section 4.7.

The module `slango::wgpu` says in its documentation that it is available only with the feature `wgpu`. Every other public item exists in both builds, so the other items do not repeat that sentence. This is a choice of the design. Where an item behaves differently on the two backends, such as `AdapterSelection::Software`, its documentation states the behaviour of each backend.

With this feature, the backend `rhi` of M10 is additive in these respects:

- A program that writes `default-features = false, features = ["rhi"]` builds neither wgpu nor `pollster`, because only the feature `wgpu` enables them.
- The compile error of SLO-29 can be relaxed without breaking a build.
- `DeviceOptions`, `AdapterSelection`, `ErrorKind`, and the other non-exhaustive types of Section 5.5 can gain a field or a variant, such as a kind for an unsupported pipeline.
- An accessor of SLO-31 can return `None` for an object of the other backend without a change of its signature.
- `texture_from_view` already returns a `Result`, so a device of the other backend can be rejected with an error kind.

The pass cannot promise the rest, and the owner accepts a breaking version 0.2 of `slango` if the backend `rhi` needs one (Section 15.11 of the architecture design). In particular, SLO-17 makes every public type `Send` and `Sync`, which a backend whose objects are not thread-safe cannot keep. The owner also decided against a spike of slang-rhi before the release, so no signature of this document had been tried against slang-rhi. Section 4.6 records how M10 settles this.

With one backend for each build (Section 4.7), M10 relies on the first point and on the growth of `ErrorKind`. No object of a build lacks the objects of its backend, so the accessors of SLO-31 and `texture_from_view` keep their behaviour (SLO-36).

### 4.4. Semantic Versioning of the Pass

Each change of the pass would break users after a release, which is why it comes before the first release:

- Putting existing items behind a new default feature removes every item from a dependent that already builds `slango` with `default-features = false`.
- Changing the return type of `device`, `queue`, and `texture` from a reference to an option breaks every caller.
- Adding `#[non_exhaustive]` to an existing type breaks every struct literal of it and every `match` on it without a wildcard arm. Every type that needs it has it already (Section 5.5), so the pass adds none.

Making `wgpu` and `pollster` optional changes nothing for a dependent that keeps the default features.

### 4.5. What the Pass Does Not Do

The pass leaves these out on purpose, because each is a decision of M10 that the backend `rhi` informs:

- A backend trait, an enumeration of backends, or a field of `DeviceOptions` that chooses a backend (SIMP-2).
- A change to `from_device_and_queue` or `texture_from_view`. Both take wgpu objects, so both stay specific to wgpu, and a device that `from_device_and_queue` returns is always backed by wgpu.
- A `cfg` on any item inside a module of the crate.
- A new error kind, a change to SLO-17, or a change to the generated code of `slango-macros`.

M10 makes each of these decisions in Sections 4.6, 4.7, and 6.1 to 6.4.

### 4.6. Owner Decisions for M10

The owner decided on 2026-11-18:

1. Every public type of `slango` that holds a GPU object loses `Send` and `Sync`, on both backends, in the breaking version 0.2 that Section 15.11 of the architecture design accepts, and `slango` implements neither trait by hand (SLO-17). This answers SOQ-4.
2. On the backend `wgpu`, the code that `slango-macros` generates loads the embedded, precompiled SPIR-V as before. On the backend `rhi`, it compiles the embedded Slang source when the module loads. The macro still compiles the shader at build time, to check it and to generate the typed bindings (SLM-24 of `docs/design/slango-macros.md`).
3. The backend `rhi` reaches parity with the backend `wgpu` for compute kernels first, and then for rendering, which is textures, samplers, and vertex and fragment pipelines. Both phases belong to M10, and each code step merges when it is verified (Section 15.13 of the architecture design).
4. The scenes of the examples `windowed-renderer` and `shader-playground` draw offscreen on the backend `rhi` in the `gpu` tests. The windowed programs stay on wgpu, and `slang-rhi` gets no surface in M10. `slang-slint` stays on wgpu only (ARCH-20 of the architecture design).
5. The Cargo features `wgpu` and `rhi` of `slango` are mutually exclusive. A build with both, or with neither, stops with one `compile_error!` that says so (SLO-29). This replaces a choice of backend for each device at run time.
6. Each load of a module on the backend `rhi` creates its own Slang session, configured like the session of the slang-rhi device, as each load on the backend `wgpu` creates its own session. Reloads and files of the same name then behave as on the backend `wgpu` (SLO-37).

The owner made decisions 1 to 4 and, later the same day, decisions 5 and 6. Other documents cite these decisions here and do not repeat them.

The design reads decision 1 as covering only the types that hold a GPU object, together with `Call` and `Draw`, which record the commands of their backend. `Error`, `ErrorKind`, the options types, and the enumerations hold no GPU object and stay `Send` and `Sync` (SLO-17). A type that callers send between threads to report a failure loses nothing by staying `Send`, and ERR-1 of the coding standard models the error type on `std::io::Error`, which is both. The owner was told of this reading and did not object.

### 4.7. The Feature `rhi`

SLO-33. The manifest of `slango` MUST declare `rhi = ["dep:slang-rhi", "vendored"]`, off by default, with `slang-rhi` as an optional dependency for every target, inherited from `[workspace.dependencies]` by path and exact version. The crate MUST NOT check the target itself.

`slang-rhi-sys` always enables `vendored` on `slang-compiler-sys`, so the feature `rhi` builds the vendored Slang in any case (Section 4.1 of `docs/design/slang-rhi-sys.md`). Naming `vendored` in the feature also makes `slang-compiler` install the loader of `slang-glslang-sys` on the global sessions of the backend `wgpu`, which Cargo would otherwise do only through the unification of the features of `slang-rhi`.

The exact version and the path let M11 publish `slango` after `slang-rhi`, in the order of `release_packages` (TOOL-35 of `docs/design/tooling.md`). `slang-rhi` sets `publish = false` until then, so `slango` with this dependency is not published in M10.

The build script of `slang-rhi-sys` stops the build on every target other than Linux with the GNU C library and x86_64 Windows with the Microsoft toolchain, with a message that names the supported targets (RSYS-13 of `docs/design/slang-rhi-sys.md`). A second check in `slango` would repeat that message in a second place that can diverge, so the crate adds none, by a choice of the design. A program on macOS that enables `rhi` therefore gets the error of `slang-rhi-sys`.

The features combine as this table states:

| Features | Backend of every device | Builds Slang and slang-rhi from source |
| --- | --- | --- |
| `wgpu`, the default | `wgpu` | No, unless `vendored` is on |
| `rhi`, with `default-features = false` | `rhi` | Yes |
| `wgpu` and `rhi` | None. The build stops with the error of SLO-29 | Yes, before the error |
| Neither | None. The build stops with the error of SLO-29 | Not reached |

A build of `slango` with its default features builds no slang-rhi, as the exit criteria of M10 require, because only `rhi` enables `slang-rhi`.

A program that wants the backend `rhi` therefore writes `default-features = false` and `features = ["rhi"]` in its dependency on `slango`, because `wgpu` stays the default feature.

The exclusion has these consequences, which the owner accepts with decision 5 of Section 4.6:

- Cargo unifies the features of one package across a build graph. Two crates of one graph that ask `slango` for different backends, or one that keeps the default features beside one that asks for `rhi`, give `slango` both features, and the build stops with the error of SLO-29. A library that depends on `slango` and works on either backend turns the default features off and leaves the choice to the program.
- `slang-slint` names `wgpu` (SLO-30), so it cannot sit in a program that uses the backend `rhi`.
- The procedural macro of `slango-macros` builds its own copy of `slango` as a host dependency, whose features Cargo resolves apart from those of the program's copy (Section 4.3 of `docs/design/slango-macros.md`). The macro's copy keeps `wgpu` while the program's copy has `rhi`, so the exclusion does not reach the macro.

The `# Features` section describes `rhi` with these points:

- It is off by default, it excludes `wgpu`, so a program turns the default features off to enable it, and it implies `vendored`.
- It runs the kernels and draws of every device on slang-rhi, with Vulkan on Linux and D3D12 on x86_64 Windows.
- It builds Slang and slang-rhi from source, with the tools that `slang-rhi-sys` names, and the build stops on every other target.
- A program on Windows needs `dxcompiler.dll` at run time.
- `slango::wgpu` does not exist in a build with `rhi`.

## 5. Public Types and Ownership

### 5.1. Types

SLO-3. `slango` MUST provide the public types `Device`, `Module`, `Kernel`, and `Buffer<T>`, each of which holds its parent by value and is cheap to clone. It MUST create `Device` and the call through structs of options that implement `Default`.

A value that holds its parent can be stored without a lifetime (OWN-3), and options can grow without a breaking change (API-6).

- `Device`: A wgpu device and its queue, or, on the backend `rhi`, a device of `slang-rhi`, its queue, and its global session (SLO-37). It is created from options or, on the backend `wgpu` in the module of Section 6, from wgpu objects that the caller already holds.
- `Module`: One loaded Slang module. It holds its `Device` and one `Kernel` for each compute entry point.
- `Kernel`: One compute entry point, with its layout snapshot, its bind group layouts, and its compute pipeline. It holds its `Device`.
- `Buffer<T>`: A storage buffer of elements of type `T`, with its length. It holds its `Device`.

On the backend `wgpu`, `Device::new` searches the primary backends of wgpu, which are Vulkan, Metal, Direct3D 12, and WebGPU in a browser, and never OpenGL. Section 6.2 states the search on the backend `rhi`, where `Software` selects a software adapter of slang-rhi instead of wgpu's fallback adapter. The field `adapter` of `DeviceOptions` selects the adapter with an `AdapterSelection`:

- `Default`, the default: The first adapter that wgpu finds. `Device::new` sets no power preference, so wgpu does not prefer a hardware GPU.
- `Software`: wgpu's fallback adapter, which is a software adapter such as lavapipe or WARP. Metal has none, so on macOS this selection returns `ErrorKind::NoAdapter` unless a Vulkan driver provides one (Section 3.8).
- `Name`: The first adapter whose name contains the given text, which also makes `ErrorKind::NoAdapter` testable.

`DeviceOptions` and `AdapterSelection` are non-exhaustive, so both can grow without a breaking change. `Device::adapter_name` returns the name of the adapter.

wgpu's types are already reference counted clones (Section 3.2), so `slango` wraps no wgpu object in an `Arc` of its own. The backend `rhi` shares the objects of `slang-rhi` that are not `Clone` through `Rc`, because they are neither `Send` nor `Sync` (Section 6.1). It shares its own owned data, such as a layout snapshot, through `Arc`. This follows the vulkano pattern of children that hold their parents (Section 3.7) and adopts nothing of `GpuFuture` (ARCH-16): every wait is a blocking call (CONC-6).

### 5.2. Usage Illustration

This illustration shows the whole flow of a compute kernel. It shows the form only. The kernel `scale` in `scale.slang` multiplies each element of a buffer by a factor:

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

SLO-19. `slango` MUST provide the public types `RenderPipeline`, `Texture`, and `Sampler`, each cheap to clone and holding its `Device` and no Slang object. It MUST create `RenderPipeline` and `Sampler` through options structs that implement `Default`.

As for SLO-3, a value without its parent would need a lifetime, and options without `Default` could not grow without a breaking change.

These are the public items of rendering:

- `RenderPipeline`: A render pipeline built from a vertex and a fragment entry point of a `Module`, with the merged layout snapshot of both, its bind group layouts, and its wgpu render pipeline (Section 7.5).
- `RenderPipelineOptions`, `Blend`, and `CullMode`: The options of a render pipeline (Section 7.5).
- `Texture` and `TextureFormat`: A two-dimensional texture with its size and format (Section 8.1).
- `Sampler`, `SamplerOptions`, `FilterMode`, and `AddressMode`: A sampler and its options (Section 8.1).
- `Draw`: The builder of one draw, which `RenderPipeline::draw` returns (Section 9.5).
- `VertexElement`: The sealed trait of the element types that a vertex attribute and a buffer accept (Section 8.2).

Every options type and enumeration is non-exhaustive, as `DeviceOptions` is (Section 5.1). `slango` owns no window, surface, or frame loop, and depends on no windowing crate (SLO-1).

A caller that renders to a window configures the surface with wgpu and wraps the view of each frame's surface texture as a `Texture` through `slango::wgpu::texture_from_view` (Section 8.1). It then draws into it and presents the frame itself.

### 5.4. Usage Illustration for Rendering

This illustration shows the form of a draw. A `gpu` test runs the program with the path of its test shader. The file `quad.slang` samples a texture onto a quad and transforms its vertices by a uniform matrix:

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

### 5.5. Exhaustiveness of the Public Types

SLO-32. Every public enumeration, and every public struct whose fields are all public, of `slango` outside `__private` MUST be `#[non_exhaustive]`, or else adding the option or variant that a later backend or Slang needs is a breaking release. Every such type of `__private` MUST instead carry an exception to `clippy::exhaustive_enums` or `clippy::exhaustive_structs`, or else `slango-codegen`, which builds them with struct literals and matches them without a wildcard arm, does not compile. A struct with a private field needs neither, because no other crate can write a literal of it.

The rule follows API-9 of the coding standard, whose third case covers the hidden module, which is exempt from semantic versioning (SLM-6 of `docs/design/slango-macros.md`). Every type already follows it, so the pass of M8 changes no type. This table lists each public type that the rule concerns, as the code has it:

| Type | File | Kind | `#[non_exhaustive]` |
| --- | --- | --- | --- |
| `DeviceOptions` | `src/device.rs` | Options struct with a public field | Yes |
| `AdapterSelection` | `src/device.rs` | Enumeration | Yes |
| `RenderPipelineOptions` | `src/render.rs` | Options struct with public fields | Yes |
| `Blend`, `CullMode` | `src/render.rs` | Enumerations | Yes |
| `SamplerOptions` | `src/sampler.rs` | Options struct with public fields | Yes |
| `FilterMode`, `AddressMode` | `src/sampler.rs` | Enumerations | Yes |
| `TextureFormat` | `src/texture.rs` | Enumeration | Yes |
| `ErrorKind` | `src/error.rs` | Enumeration of error kinds (ERR-1) | Yes |
| `Precompiled` | `src/__private.rs` | Struct with public fields | No, with an exception |
| `Interface`, `KernelInterface`, `Parameter`, `ValueType`, `Structure`, `Field`, `UniformStructure` | `src/__private/interface.rs` | Structs with public fields | No, with an exception |
| `ParameterKind`, `Scalar`, `Shape`, `Access`, `ElementType`, `FieldType` | `src/__private/interface.rs` | Enumerations | No, with an exception |

`Device`, `Module`, `Kernel`, `Buffer<T>`, `RenderPipeline`, `Texture`, `Sampler`, `Call`, `Draw`, and `Error` have only private fields. The enumeration `ValueType` of the private module `marshal::sealed` is reachable by no other crate, so neither lint reports it.

The check is a tool check already, as Hard Rule 4 of `AGENTS.md` requires. `clippy::exhaustive_enums` and `clippy::exhaustive_structs` report every public enumeration and every public struct with only public fields that has neither the attribute nor an exception, in every crate of the workspace, and `check-clippy` fails on them (Section 3.10). A new public type therefore cannot reach `main` without a decision, and the pass adds no lint configuration.

Clippy cannot judge which of the two a type needs, so that choice stays the judgement rule API-9, which the review of the public API of each crate of the release in M8 applies (Section 15.11 of the architecture design). The lints cover the other crates of the release in the same way, so this document asks nothing of them.

## 6. Backend Boundary

SLO-4. Every public item of `slango` that names a wgpu type MUST be in the public module `slango::wgpu`, which exists only with the feature `wgpu` (SLO-28) and holds the constructors from wgpu objects and the accessors that return them. Every other use of wgpu MUST stay private, because a later backend then changes no other part of the public API (ARCH-15).

The layout snapshot is the boundary inside the crate. The reflection walk above it knows Slang and no wgpu type, and the code below it turns the snapshot into wgpu layouts, bind groups, and dispatches, or, on the backend `rhi`, into the shader objects and pipelines of `slang-rhi` (Sections 7.6 and 9.6).

No public backend trait exists. The two backends share the code above the boundary, and each build compiles the private module of its one backend below it (SLO-34). This answers Q-17 of the architecture design.

SLO-31. Each function of `slango::wgpu` that returns a wgpu object of a `slango` object MUST return it as an `Option` of a reference, and `None` when the object has no such wgpu object. Otherwise a later backend, whose objects hold no wgpu object, could only panic in these functions or change their signatures in a breaking release. The three functions change as this table states:

| Function | Former signature | Signature with SLO-31 |
| --- | --- | --- |
| `device` | `fn device(device: &Device) -> &wgpu::Device` | `fn device(device: &Device) -> Option<&wgpu::Device>` |
| `queue` | `fn queue(device: &Device) -> &wgpu::Queue` | `fn queue(device: &Device) -> Option<&wgpu::Queue>` |
| `texture` | `fn texture(texture: &Texture) -> &wgpu::Texture` | `fn texture(texture: &Texture) -> Option<&wgpu::Texture>` |

Each keeps `#[must_use]`. No other public function returns a wgpu object (Section 3.10).

The return type is an `Option` and not a `Result` with a kind of `ErrorKind`, by a choice of the design. A missing wgpu object is a property of the object that the caller asks about, not a failure of an operation (ERR-5), and the caller knows which backend made the object.

A new kind would also be one that no code path of the crate can return while wgpu is its only backend. A caller that needs an error maps the `None` to an error of its own, as `slang-slint` does.

wgpu is the only backend, so every `Device` and every `Texture` holds a wgpu object, and the three functions always return `Some`. Each implementation returns `Some` of the reference that the crate-private accessor gives. Each function carries `#[expect(clippy::unnecessary_wraps, reason = "...")]`, whose reason says in plain words that an object of a backend other than wgpu has no wgpu object and that wgpu is the only backend of the crate.

The exception turns stale, and fails `check-clippy`, as soon as a function can return `None`, so a change that lets one return `None` has to change it (Hard Rule 6 of `AGENTS.md`). Section 6.3 states why M10 keeps it. The documentation of each function states that it returns `None` for an object that has no wgpu object, and that every object has one while wgpu is the only backend of the crate. Its example handles the `Option`, without `unwrap`.

The two constructors stay as they are:

- `from_device_and_queue(device: wgpu::Device, queue: wgpu::Queue) -> Device` cannot fail, because it always makes a device that wgpu backs.
- `texture_from_view(device: &Device, view: &wgpu::TextureView) -> Result<Texture>` already returns a `Result`, so a later backend can reject a device that wgpu does not back with an error kind, which is no breaking change.

The callers in the workspace change in these ways:

- `View::to_image` of `slang-slint` maps `None` to an error of its kind `TextureImport` (SLS-11 of `docs/design/slang-slint.md`).
- The example `windowed-renderer` keeps clones of the wgpu device and queue that it creates before it hands them to `from_device_and_queue`, and configures the surface and presents each frame through those clones. It then calls none of the three functions. wgpu objects are reference counted, so a clone is cheap (Section 3.2).
- The tests of `slango`, of `slang-slint`, and of the examples that call the three functions unwrap the `Option` with `expect` and a message that names the object, such as "the wgpu device of a device".
- The helpers `write_pixels` and `wgpu_texture` of `tests/slango/gpu/texture.rs` call them too. They are not test functions, so `allow-expect-in-tests` of `clippy.toml` does not cover them, and each carries `#[expect(clippy::expect_used, reason = "...")]`, whose reason says that the helper fails the test when the object has no wgpu object. Making them fallible would change every caller.
- The code that `slango-codegen` generates names no wgpu item (Section 3.10), so it, its snapshots, and `slango-macros` do not change.

### 6.1. One Backend for Each Build

SLO-34. `slango` MUST hold the objects and calls of each backend in one private backend module, compiled only under the `cfg` of its feature, and MUST compile the shared modules in every build. Each public type that holds a GPU object, which are `Device`, `Module`, `Kernel`, `Buffer<T>`, `Texture`, `Sampler`, and `RenderPipeline`, MUST hold the objects of the build's backend through a crate-private type of that backend module with the same name in each module. `slango` MUST NOT add a public trait or a private enumeration over the backends. `Call` and `Draw` MUST keep the arguments that a caller gives them in the backend-neutral form of Section 9.1 and pass them to the backend only when they submit.

A build has exactly one backend (SLO-29), so nothing chooses between backends while the program runs, and neither a trait nor an enumeration has a second case to serve. The public API stays one set of concrete types, so a program written for one backend compiles on the other, apart from the module `slango::wgpu`.

A public trait would freeze the shape of the boundary and invite a third implementation outside the crate, which nothing needs (SIMP-1 of the coding standard). The code step that adds the backend module changes SIMP-2 of the coding standard, whose condition, a second backend, then holds, to require this structure.

The crate root declares the backend module of the enabled feature under one name, so the shared modules name `backend::Device` and the like without a `cfg` of their own. Only the crate root, the two backend modules, and the tests hold a `cfg` of a backend feature. The boundary has these parts:

| Part | Shared by both backends | Backend `wgpu` | Backend `rhi` |
| --- | --- | --- | --- |
| Reflection walk, `layout` | Paths, kinds, element types, strides, plain values with the row stride of each matrix, vertex attributes, and usage (Section 7.1) | Reads set and binding for its bind group layouts | Ignores set and binding (Section 7.6) |
| Marshalling, `marshal` | `Value`, `Element`, `VertexElement`, and the writing of plain values at reflected offsets (Section 9.2) | Writes into the uniform buffers and the immediates of a call | Writes into the uniform data of shader objects (Section 9.6) |
| Checks of a call and a draw | Every check of SLO-14, SLO-26, and SLO-44, before any work of the backend | The workgroup and buffer limits of wgpu | The thread group and buffer limits of `slang-rhi` |
| Encoding, `encoding` | The encoded snapshot of `slango-macros`, version 3 (Section 7.1) | Decodes it for `load_embedded` | Does not read it (Section 7.6) |
| Hidden module, `__private` | `precompile_file`, the interface description, `StructElement`, and the helper macros of SLM-25 of `docs/design/slango-macros.md` | `Embedded` holds the encoded snapshot, and `load_embedded` decodes and loads it | `Embedded` holds the embedded sources, and `load_embedded` compiles them (SLM-24 of `docs/design/slango-macros.md`) |
| Errors, `error` | `Error` and `ErrorKind` (Section 10) | Maps the errors of wgpu | Maps the errors of `slang-rhi` (SLO-42) |

Each public type holds these objects of the build's backend:

| Type | Backend `wgpu` | Backend `rhi` |
| --- | --- | --- |
| `Device` | `wgpu::Device` and `wgpu::Queue` | `slang_rhi::Device`, its `slang_rhi::Queue`, and the `slang_compiler::GlobalSession` of SLO-37, in an `Rc` |
| `Module` | The kernels and the render records of the snapshot | The kernels, the render records, the `slang_rhi::CompileSession` of its load, and the `slang_compiler::Module` of that session with the entry points of its render records (SLO-37) |
| `Kernel` | Its layout snapshot, bind group layouts, and compute pipeline | Its layout snapshot and its `slang_rhi::ComputePipeline` |
| `Buffer<T>` | `wgpu::Buffer` | `slang_rhi::Buffer`, or none for a buffer of length zero (SLO-40) |
| `Texture` | `wgpu::Texture` and its view | The texture and its view of `slang-rhi` (Section 8.3) |
| `Sampler` | `wgpu::Sampler` | The sampler of `slang-rhi` |
| `RenderPipeline` | Its merged snapshot, bind group layouts, and render pipeline | Its merged snapshot and the render pipeline of `slang-rhi` (Section 7.7) |

Every type also keeps the identity of its device, which its clones share (Section 9.3), on both backends.

### 6.2. The Device on the Backend `rhi`

`DeviceOptions` has no choice of backend, because the build fixes it (SLO-29). On the backend `rhi`, `Device::new` creates the global session and then the device of `slang-rhi` of the device type of the platform, which is Vulkan on Linux and D3D12 on Windows. The device type is chosen by `cfg(target_os = "windows")` in the private module of the backend, because `AdapterSelection::Name` and `Software` need the adapters of one device type. The field `adapter` selects the adapter in this way:

- `Default`: The adapter that slang-rhi chooses, when the options name none.
- `Software`: The first adapter of `slang_rhi::adapters` whose adapter type is a software adapter, such as lavapipe or WARP, or `ErrorKind::NoAdapter` when there is none.
- `Name`: The first adapter of `slang_rhi::adapters` whose name contains the given text, or `ErrorKind::NoAdapter`.

A failure of `slang_rhi::Device::new` of the kind `NoDevice` gives `ErrorKind::NoAdapter`, and every other failure gives `ErrorKind::DeviceRequest` with the error of `slang-rhi` as its source. `Device::adapter_name` returns the adapter name of `DeviceInfo`. The device keeps the debug device of slang-rhi off and passes no closure of its own, so `slang-rhi` keeps only the diagnostics of Slang that it collects and drops every other message (RHI-6).

`slango::wgpu::from_device_and_queue` exists only in a build with the feature `wgpu`, so it always creates a device of the backend `wgpu`, as before, and stays infallible.

### 6.3. The Module `slango::wgpu` and the Backend `rhi`

SLO-36. `slango` MUST NOT add a public module that returns the objects of `slang-rhi`, or any other public item that exists only with the feature `rhi`, in M10. The module `slango::wgpu` MUST keep its five functions, their signatures, and the exceptions to `clippy::unnecessary_wraps` of SLO-31 unchanged.

A module `slango::rhi` would put `slang-rhi`, which sets `publish = false` until M11, into the public API, and no program needs its objects in M10: no window and no Slint view runs on the backend `rhi` (Section 4.6). It can be added later without a breaking change, so the design leaves it out (SIMP-1). A build with `rhi` therefore has exactly the public items of a build with `wgpu`, without the module `slango::wgpu`.

The module `slango::wgpu` exists only in a build whose one backend is wgpu (SLO-28), so every object that its accessors receive holds a wgpu object, every accessor still returns `Some`, and the lint still fires. The exceptions therefore stay, with their reason, and `texture_from_view` never meets a device of another backend. The documentation of each accessor changes only where it speaks of another backend, to say that every object of a build with the feature `wgpu` has a wgpu object.

### 6.4. What the Backend `rhi` Does Not Offer

The backend `rhi` offers every public item of Sections 5 and 7 to 9, except these:

- `slango::wgpu` and its five functions, which a build with `rhi` does not have (SLO-36).
- The immediates feature of wgpu, which the backend does not need. A `uniform` parameter of an entry point reaches the entry point object instead (Section 9.6), so SLO-18 and `ErrorKind::MissingFeature` concern the backend `wgpu` only.
- The validated path of naga. The backend loads what slang-rhi generates, so `ErrorKind::UnsupportedShader` concerns the backend `wgpu` only.
- A kernel with a `ConstantBuffer` and a global `uniform` on D3D12, which RKB-2 of `docs/design/slang-rhi.md` makes fail at load with `ErrorKind::Backend`. `slango` does not guard against it (Section 1.6).

These differences of shaders between the backends stay, and the crate documentation states them:

- A shader that reads the vertex index cannot run on both the backend `wgpu` and the backend `rhi` on D3D12: DXIL rejects `SV_VulkanVertexID`, and naga rejects `SV_VertexID` (Section 3.11). The design expects the same of the instance index with `SV_VulkanInstanceID` and `SV_InstanceID`, which was not measured. A shader that both backends run on every platform takes such values from a vertex buffer instead, as the shader of the example `shader-playground` does from M10 (Section 13.7).
- On the backend `rhi`, every vertex attribute and every varying needs a semantic, which D3D12 matches by name and DXIL requires (Section 3.11). The backend `wgpu` matches by location and needs none.

Until the code step of M10 that adds rendering to the backend, `Texture::new`, `Texture::from_pixels`, `Sampler::new`, and `Module::render_pipeline` on the backend `rhi` fail with `ErrorKind::Unsupported` (SLO-42), so each code step of Section 15.13 of the architecture design merges a crate that compiles and reports what it lacks.

## 7. Loading a Module

### 7.1. Compile and Snapshot

SLO-5. `Device::load_module` MUST compile the module on the calling thread and copy into a layout snapshot the facts that the kernels need. On the backend `wgpu`, it MUST release every Slang object before it returns, because no Slang object then outlives the call or ties the module to the global session of the load (Section 11). The backend `rhi` keeps the objects that SLO-37 names instead.

The load reads the file as text and compiles it with `Session::load_module_from_source`, with the path of the file as the path of the module and the directory of the file as a search path. A variant takes the source text and a module name instead of a path.

The session targets `TargetFormat::Spirv` with Slang's default layout, and sets `vulkan_use_entry_point_name(Toggle::On)` so that every message of wgpu names the Slang function (Section 3.2). The load generates one SPIR-V module for each entry point through `LinkedProgram::entry_point_code`.

The load follows these rules:

- `Device::load_module` takes the file stem as the module name, and the directory of the file, or `.` when the path has none, as the search path.
- `Device::load_module_from_source` takes the module name and the source text and sets no search path, so an `import` finds only what Slang finds by default.
- A path that is not valid UTF-8, which Slang needs, gives `ErrorKind::InvalidArgument`. So does a file that cannot be read as text, with the I/O error as the source error.

Slang resolves an `import` of a sibling module relative to the path of the importing file, so the search path is not what makes such an `import` resolve. The search path is one more directory in which Slang looks for an imported module.

The load has two parts, so that a module compiled ahead of time loads through the same code (SLM-6 of `docs/design/slango-macros.md`):

- The compile reads the file, when there is one, compiles the module, and walks it into the layout snapshot with the SPIR-V of each entry point. It also returns the paths of the files that Slang read, which the load does not need. It releases every Slang object before it returns, so SLO-5 holds for it.
- The load creates the module on the device from the snapshot, with no call into Slang.

`Device::load_module` and `Device::load_module_from_source` compile and then load. `slango::__private::precompile_file` compiles a file with the same code as `Device::load_module`, and `slango::__private::load_embedded` loads a snapshot that it decodes on the backend `wgpu`, or compiles the embedded sources and loads them through the load of Section 7.6 on the backend `rhi` (Section 5.2 of `docs/design/slango-macros.md`).

SLO-6. The layout snapshot MUST hold only owned data and MUST be private to `slango`.

The data is the thread group size, the immediate size, the structures among the plain values, and, for each parameter, its parameter path, its kind, its set and binding, and whether the code uses it. Each parameter also holds either its element stride and element type, with the layout of a structure element, or the scalar type, shape, offset, and size of each plain value. The snapshot is the backend-neutral boundary of Section 6, and a public snapshot would freeze its shape.

The walk MUST follow the rules of Section 3.3, including the shift of global bindings for global plain values and the sum of `SubElementRegisterSpace` offsets for parameter blocks.

This answers Q-31 of the architecture design, and CQ-4 of `docs/design/slang-compiler.md`, for M3. It also makes Q-30 of the architecture design, and CQ-1 of `docs/design/slang-compiler.md`, unnecessary for M3, because no Slang object leaves `load_module`.

The snapshot holds one record for each compute entry point, and one for each vertex and fragment entry point (Section 7.4). The record of a kernel holds:

- The name of the entry point and its thread group size.
- Its bindings, sorted by set and binding, each with its parameter path and a usage flag. A binding is either a storage buffer with its access, read-only or read-write, its element stride, and its element type, or a uniform buffer with its size and its plain values.
- An optional block of immediates with its size and its plain values.
- The structures among its plain values, in the order of the walk, each with its parameter path and Slang name, such as `params` with `Params` and `params.inner` with `Inner`. The structure that is the element of a `ConstantBuffer<S>` or a `ParameterBlock<S>` is not one of them, because its fields are parameters of their own. These records serve the generator of `slango-macros`, and the runtime does not read them.

The walk reads the element type of a storage buffer from the element type layout from which it reads the stride, and the check of Section 9.3 reads it (SLM-5 of `docs/design/slango-macros.md`). An element type is one of these:

- A scalar or a vector of `bool`, `int`, `uint`, or `float`, with its number of components, 1 for a scalar.
- A scalar or a vector of another scalar type, such as `half2`, `double`, or `uint8_t4`, with the name that Slang gives it, such as `vector<half,2>`.
- A structure, with its Slang name, its size, and its fields, each with its name, its byte offset, and its type. A field type is a scalar, vector, matrix, or array of `bool`, `int`, `uint`, or `float`, a nested structure, or any other type with the name that Slang gives it, such as `half` or `Inner[2]`. A structure with a field of another type still loads.
- A composite, which is a matrix, an array, or any other type that is neither a scalar, a vector, nor a structure. The snapshot records no name, size, or fields for it.

When Slang reports no type for the elements of a scalar or vector element, the load gives `ErrorKind::UnsupportedParameter`, as the walk's other checks for a missing answer of Slang do.

Each plain value has its parameter path, offset, size, scalar type, and shape. A shape is a scalar, a vector with its length, a matrix with its rows, columns, and row stride (SLO-46), or an array with its length, stride, and element shape. The walk flattens a structure into one plain value for each leaf, with a dotted path such as `params.scale`.

A constant buffer or parameter block without plain values gets no uniform buffer.

The walk names parameters and records their usage by these rules:

- A field of a named `cbuffer` gets the path `Name.field`, such as `Arrays.c` for `cbuffer Arrays { float c[5]; }`, although the shader reads it as `c`.
- A parameter of the entry point has a path without a prefix, so `uniform float factor` has the path `factor`, as a global has.
- The unnamed uniform buffer of the global plain values has the empty path.
- The usage of a binding comes from the metadata of the entry point, through `Metadata::is_parameter_location_used` with its set and binding. A plain value has the usage of the uniform buffer that holds it. The immediates carry no usage flag, so a call treats them as used.
- The walk does not record the matrix layout mode, because every session of `slango` and of slang-rhi keeps the row-major layout (Sections 3.4 and 3.11). It records the row stride of each matrix as SLO-46 states.

SLO-46. The walk MUST record, for each matrix among the plain values, the stride between its rows that reflection reports for the target of the session, and marshalling MUST write row `r` of the matrix at its offset plus `r` times that stride, on both backends. The rule of Section 3.4, which derives the row stride from the extent of the matrix, holds for Slang's SPIR-V but not for DXIL, where the last row is not padded (Section 3.11). The shape of a matrix in the snapshot therefore holds its row stride, which changes the bytes of the encoding, so `ENCODING_VERSION` becomes 3 (Section 5.3 of `docs/design/slango-macros.md`).

The row stride comes from the type layout of the matrix through `slang-compiler`. Whether that crate offers it already is not verified, and the code step that adds SLO-46 adds an accessor to `slang-compiler` when it lacks one, which is an additive change (SOA-11).

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
| `Texture2D` with a `float` result, and `SamplerState` | A sampled texture or a sampler (Section 8.1) |

Among the rejected parameters are every other resource, such as a storage texture, `Sampler2D`, or `ByteAddressBuffer`, a plain value of another scalar type, such as `double`, an array of structures or of resources, and an array without a fixed length. A failure of Slang to compile, link, or reflect the module is `ErrorKind::Compile`.

### 7.2. The Global Session for Each Load

SLO-7. On the backend `wgpu`, `Device::load_module` MUST create its own global session and release it before it returns. `slango` MUST keep no global session in a `static`, a `thread_local!`, or any hidden state, because CONC-4 forbids hidden library state. A device of the backend `rhi` holds its own global session as a field, which is no hidden state, and each load on it creates its own session from that global session (SLO-37).

The simplest form also has no state to invalidate or to clean up at thread exit.

The global session makes up most of the cost of each load, well over 100 ms (Section 3.6). That is acceptable for a program that loads a few modules once, and for the hot reload of a development tool after each save of a file, such as the example of Section 13.5.

SLO-7 therefore stays through M4. A caller-held compiler or a cache of global sessions becomes an open question again only when a later need, such as loading many modules, makes the cost matter.

### 7.3. Kernels and Pipelines

SLO-8. `Device::load_module` MUST create the shader module, the bind group layouts, and the compute pipeline of every compute entry point before it returns. It MUST fail as a whole when any of them fails, because every error of a module then surfaces at one call.

Each creation costs under 1 ms (Section 3.6).

SLO-9. `slango` MUST create each shader module inside an error scope and MUST return a rejection by naga as `ErrorKind::UnsupportedShader`, with naga's text returned unchanged as the source error (Section 10), because the text names the construct that the user must change (Section 3.5). An error that wgpu reports to its uncaptured-error handler is lost to the caller.

The bind group layouts leave out every binding that the code does not use, which wgpu accepts (Section 3.3), so that the caller binds only what the kernel reads or writes. A kernel has one bind group layout for each set from 0 to the last set that holds a used binding, and a set between them with no used binding gets an empty layout. A parameter that the walk rejects fails the load with `ErrorKind::UnsupportedParameter` (Section 7.1).

The load checks SLO-18 before it creates the shader module of a kernel. It fails with `ErrorKind::MissingFeature` when the kernel has immediates and the device lacks `Features::IMMEDIATES`, or when the reflected size of the immediates exceeds the device's `max_immediate_size`. Section 10 states how the load maps the errors of wgpu.

`Module::kernel` looks a kernel up by its Slang name and returns a reference to it. For a name that is no compute entry point of the module, it returns `ErrorKind::InvalidArgument` with a text that lists no names. No method lists the kernels of a module.

### 7.4. Vertex and Fragment Entry Points

SLO-20. `Device::load_module` MUST also record the layout snapshot and the SPIR-V of every vertex and fragment entry point, and MUST NOT create their shader modules or fail on their parameters. Otherwise a render entry point that `slango` rejects would make the kernels of its module unusable.

The load compiles every entry point as Section 7.1 states, with one SPIR-V module for each (Section 3.9), and ignores the entry points of other stages. The record of a vertex or fragment entry point holds its name, its stage, and its bindings and plain values as Section 7.1 defines them, with the usage of each binding from that entry point's metadata. The record of a vertex entry point also holds its vertex attributes.

A failure that belongs to one such entry point becomes the rejection on its record, and the load goes on (SLO-20). The rejection is `ErrorKind::UnsupportedParameter` for a parameter that the walk refuses. It is `ErrorKind::Compile` for a failure of Slang to reflect the entry point, report its metadata, or generate its SPIR-V. A `Compile` rejection keeps the text of the error of `slang-compiler`, with Slang's diagnostics, apart from its detail. The detail is the detail of that error, or "Slang failed on the {role} `{name}`" when it has none, and the text becomes the source error when `render_pipeline` returns the rejection.

The rejection keeps the text and not the error of `slang-compiler`, because the record is returned from every call of `render_pipeline` and is decoded from the bytes of a precompiled module, so its source is a crate-private error type that holds the text and implements `Display` and `std::error::Error`. The documentation promises no downcast of the source. The encoding of a rejection record holds a tag byte, 0 or 1, and the text of the source after the detail, which is why `ENCODING_VERSION` is 2 (Section 5.3 of `docs/design/slango-macros.md`).

A rejected record keeps no SPIR-V, and `Module::render_pipeline` returns its rejection.

The load skips an entry point whose name Slang cannot report. It fails as a whole only for a failure of the whole compilation or of a kernel, and compute kernels keep their pipelines at load (SLO-8).

SLO-21. The walk MUST record each vertex attribute of a vertex entry point with its parameter path, location, scalar type, component count, and usage, because the render pipeline builds its vertex layout from reflection alone and the draw checks each buffer against it.

The walk records the vertex attributes by these rules:

- It visits the parameters that `EntryPointLayout::parameter_layouts` returns with the category `VaryingInput`, and the fields of a structure, recursively.
- The parameter path is the dotted path from the parameter's name, such as `input.position` for the field `position` of the parameter `input`. A parameter that is not a structure has its own name as its path.
- The location is the sum of `offset(VaryingInput)` along the path (Section 3.9).
- A field without the category `VaryingInput`, such as `SV_Position`, `SV_VulkanVertexID`, or `SV_VulkanInstanceID`, is skipped (Section 3.9). That every other system value lacks the category is assumed (SOA-6).
- A `float`, `int`, or `uint` scalar, or a vector of 1 to 4 of them, is supported, and a vector of one component is an attribute like a scalar. Every other type, such as a matrix, an array, or a `bool`, is a rejection of the walk.
- The usage comes from `Metadata::is_parameter_location_used` with `ParameterCategory::VaryingInput`, space 0, and the location.

The walk records no varying and no output of the fragment entry point, because wgpu matches the varyings by location (Section 3.9). The walk accepts a sampler only when its type is `SamplerState`, so it rejects `SamplerComparisonState`, which reflects with the same kind, under SLO-24.

### 7.5. Render Pipelines

`Module::render_pipeline` takes the name of a vertex entry point, the name of a fragment entry point, and a `RenderPipelineOptions`, and returns a `RenderPipeline`. A name that is no entry point of that stage gives `ErrorKind::InvalidArgument`, as `Module::kernel` does (Section 7.3). Each call creates a new pipeline, and `slango` keeps no cache, because a cache is hidden state (CONC-4).

The fixed-function state of a render pipeline is this:

- The topology is always a list of triangles, and front faces are counter-clockwise, which is wgpu's default.
- `Blend::Replace` sets no blend state, and every colour channel is written.
- The depth test has wgpu's default stencil and bias state.
- The other primitive fields and the multisample state, with one sample, are wgpu's defaults.

These are the fields of `RenderPipelineOptions`, each with its default:

| Field | Type | Default | Meaning |
| --- | --- | --- | --- |
| `colour_format` | `TextureFormat` | `Rgba8Unorm` | The format of the colour target. `Depth32Float` gives `ErrorKind::InvalidArgument` |
| `depth_format` | `Option<TextureFormat>` | `None` | `Some(TextureFormat::Depth32Float)` adds a depth test with the comparison `Less` and depth writes. Any other format gives `ErrorKind::InvalidArgument` |
| `blend` | `Blend` | `Blend::Replace` | `Replace` writes the colour of the fragment entry point, and `Alpha` blends it over the target with wgpu's `BlendState::ALPHA_BLENDING` |
| `cull_mode` | `CullMode` | `CullMode::None` | `Back` discards the triangles that face away |
| `instance_attributes` | `Vec<String>` | Empty | The parameter paths of the vertex attributes that step per instance. Every other attribute steps per vertex. A path that names no vertex attribute gives `ErrorKind::InvalidArgument`, and a path of an attribute that the vertex entry point does not use is accepted and has no effect |

The step mode is an option of the pipeline, not of the draw, because wgpu fixes it when it creates the pipeline (Section 3.9).

SLO-22. `Module::render_pipeline` MUST create both shader modules, the bind group layouts, the pipeline layout, and the render pipeline inside error scopes. It MUST fail as a whole, so that every error of a pipeline surfaces at one call, as SLO-8 requires for kernels.

`Module::render_pipeline` runs these checks in this order, before any call of wgpu, and returns the first failure with a message that names both entry points:

1. The vertex name, then the fragment name, names an entry point of that stage, otherwise `ErrorKind::InvalidArgument`.
2. The record of the vertex, then of the fragment entry point, holds no rejection (Section 7.4), otherwise the kind of the rejection, `UnsupportedParameter` or `Compile`.
3. The vertex, then the fragment entry point, has no `uniform` parameters of its own, otherwise `ErrorKind::UnsupportedParameter` (SLO-23).
4. `colour_format` is a colour format, then `depth_format` is `None` or `Depth32Float`, otherwise `ErrorKind::InvalidArgument`.
5. Each path of `instance_attributes` names a vertex attribute of the vertex entry point, otherwise `ErrorKind::InvalidArgument`.
6. Each vertex attribute has a vertex format (Section 8.2), otherwise `ErrorKind::UnsupportedParameter`. The walk records only attributes that have one.
7. The bindings of the two entry points merge, otherwise `ErrorKind::UnsupportedParameter`.

The pipeline merges the bindings of its two entry points, which share every global binding (Section 3.9), by set and binding. A set and binding that both entry points report must describe the same parameter path and the same resource, or the merge fails. Each merged binding is visible to exactly the stages whose metadata reports it used.

A binding that neither stage uses gets no bind group layout entry, as Section 7.3 states for kernels. It stays known to the pipeline, so that a draw can accept and ignore an argument for it (Section 9.5). The bind group layouts cover the sets as Section 7.3 states for kernels.

The vertex state has one vertex buffer for each used attribute, in slots numbered from 0 in the order of locations. Each attribute sits at offset 0, with a stride equal to the size of its format, and the step mode `Instance` when `instance_attributes` names its path, otherwise `Vertex`. An unused attribute gets no vertex buffer (Section 3.9).

Because each used attribute takes its own vertex buffer, the device's limit on vertex buffers bounds the attributes that a vertex entry point may use, and creation fails when it uses more. Section 10 states how the errors of creation map to kinds.

SLO-23. `Module::render_pipeline` MUST return `ErrorKind::UnsupportedParameter` when the vertex or the fragment entry point has `uniform` parameters of its own, because their push constant blocks alias at offset 0 and wgpu sets immediates without a stage (Section 3.9).

The uniforms of a render pipeline are therefore global uniforms, constant buffers, and parameter blocks, and its pipeline layout has no immediates.

### 7.6. Loading on the Backend `rhi`

SLO-37. A device of the backend `rhi` MUST create one `slang_compiler::GlobalSession` and pass it to `slang_rhi::Device::new`, and MUST hold it for its whole life. Each load on that device MUST create a session of its own with `slang_rhi::Device::create_slang_session` (RHI-30 of `docs/design/slang-rhi.md`), with the directory of the file as its search path when the load has a file, MUST compile the module in that session, MUST walk the reflection of the program linked in that session into the layout snapshot, and MUST keep the `slang_rhi::CompileSession`, the `slang_compiler::Module`, and the entry points of its vertex and fragment entry points in the `Module` for the whole life of the `Module`. It MUST create every shader program through that `CompileSession`, and MUST set no target and no layout option itself.

This implements the owner's decision 6 of Section 4.6. A session of its own for each load makes a reload of a changed file, two files of the same stem on one device, and a changed imported file behave as on the backend `wgpu`, which one session for all loads of a device cannot do (Section 3.11). `create_slang_session` reproduces the target, the profile, the scalar layout, the row-major matrix layout, and the other settings with which slang-rhi creates its device session, so the code that a shader program gets is what the device session would give, and `slang-rhi` accepts a program of such a session through `CompileSession::create_shader_program` (RHI-30 of `docs/design/slang-rhi.md`). That the reproduced settings are complete is RHA-8 of that design. The target is the one that the device runs, SPIR-V on Vulkan and DXIL on D3D12 (Section 3.11). This answers Q-36 of the architecture design: the backend `rhi` compiles to the target that slang-rhi chooses for each device type, and that answer leaves Q-23, the shader target of the backend `wgpu`, deferred as it was.

The Slang objects of a load have these lifetimes:

- The `Device` holds the global session, and every `Module` holds its `Device`, so the global session outlives every session made from it.
- The `Module` holds the `CompileSession` of its load, which holds the session, so a `Module` keeps the session of its load alive, with the entry points of its render records, because a render pipeline is created after the load from a vertex and a fragment entry point (Section 7.7).
- Each shader program of `slang-rhi` holds its linked program, so a `Kernel` or `RenderPipeline` keeps what it was created from even after its `Module` is dropped.
- When the last `Module`, `Kernel`, and `RenderPipeline` of a load are dropped, the session of the load is released. A program that reloads a shader therefore does not grow the memory of the device with each load.

One global session for each device replaces the global session for each load of SLO-7, so a load on the backend `rhi` creates a session but no global session, which costs less (Section 3.6).

The load on the backend `rhi` follows Section 7.1 with these differences:

- `Device::load_module` reads the file and loads it with `Session::load_module_from_source` in the session of the load, with the path of the file and the file stem as the module name, and the directory of the file as the search path, as on the backend `wgpu`. `Device::load_module_from_source` sets no search path. Slang resolves an `import` of a sibling file relative to the path of the importing file (Section 7.1), so that `import` resolves on both backends.
- The walk runs on the `ProgramLayout`, the `EntryPointLayout`, and the metadata of target 0 of the program linked in the session of the load, so every offset, size, and stride in the snapshot is that of the device's target. The walk is a function of these reflection handles alone, so the backend reuses it, with the one change of the usage flag in the last item of this list.
- The snapshot holds no SPIR-V. The walk reads the metadata of each entry point, which makes Slang generate the code of the entry point for the target (Section 3.11), so the load generates code. On D3D12, a load therefore needs `dxcompiler.dll` as the creation of a pipeline does. Whether Slang reuses that code when slang-rhi creates the pipeline, or generates it a second time, is not verified, and only the cost depends on it.
- The usage flag of a binding comes from `Metadata::is_parameter_location_used` with the parameter category that the target reports for the resource. On DXIL, a resource reports the category of its register class instead of `DescriptorTableSlot` (SOA-10). The backend `rhi` reads no set and no binding of the snapshot.

SLO-38. For each compute entry point, the load on the backend `rhi` MUST compose the module with that entry point alone, link it in the session of the load, and create its shader program and its compute pipeline before it returns. It MUST fail as a whole when any of them fails, as SLO-8 requires of the backend `wgpu`.

slang-rhi needs a program with exactly one compute entry point for each compute pipeline (Section 3.11), and creating every pipeline at load makes every error of a module surface at one call, as on the backend `wgpu`. The load maps the errors of `slang-rhi` as SLO-42 states.

The snapshot of a module from `slango::__private::load_embedded` on the backend `rhi` is the one that this load builds from the embedded sources (SLM-24 of `docs/design/slango-macros.md`). A build with `rhi` embeds no encoded snapshot.

### 7.7. Render Pipelines on the Backend `rhi`

SLO-41. On the backend `rhi`, `Module::render_pipeline` MUST run the checks of Section 7.5 in their order, and then the checks of this section, before any call into `slang-rhi`, MUST compose the module with the vertex and the fragment entry point, link it in the session of the load of the module, and create a shader program and a render pipeline of `slang-rhi` from it. The render pipeline MUST have the fixed-function state of Section 7.5, one vertex stream for each used attribute in the order of locations, and the options of `RenderPipelineOptions` mapped as this section states. It MUST fail as a whole, as SLO-22 requires.

The backend reuses the merge of the bindings and every check of Section 7.5, which read only the snapshot. The options map to the render pipeline of `slang-rhi` (RHI-26 of `docs/design/slang-rhi.md`) in this way:

| Option or state | Render pipeline of `slang-rhi` |
| --- | --- |
| Topology | A list of triangles |
| Front face | Counter-clockwise |
| `colour_format` | The format of the one colour target, by the mapping of Section 8.3 |
| `depth_format` | `Some(Depth32Float)` gives a depth test with the comparison `Less` and depth writes, and `None` gives no depth test |
| `blend` | `Replace` turns blending off and writes every channel. `Alpha` blends the colour with the source alpha and its inverse, and the alpha with one and the inverse source alpha, as wgpu's `BlendState::ALPHA_BLENDING` does |
| `cull_mode` | `None` or `Back`, with the same meaning |
| `instance_attributes` | The step of each vertex stream, per instance for a named attribute and per vertex otherwise |

Each used attribute sits at offset 0 of its own vertex stream, with the stride of its format, as on the backend `wgpu` (Section 7.5). `slang-rhi` also needs the semantic name and index of each attribute, by which D3D12 matches it (RHA-6 of `docs/design/slang-rhi.md`). The load on the backend `rhi` reads them from reflection into a record of the module outside the layout snapshot, which the backend `rhi` never encodes (Section 7.6).

After the checks of Section 7.5, `render_pipeline` on the backend `rhi` runs these checks and returns the first failure with a message that names both entry points:

1. Each vertex attribute has a semantic, otherwise `ErrorKind::UnsupportedParameter` that names the attribute, because D3D12 matches attributes by semantic and DXIL needs one (Section 6.4). The check runs on Vulkan too, so that one shader behaves alike on both platforms of the backend.
2. The used attributes number at most the limit of vertex streams of the device and at most 16, and the input elements, one for each location from 0 to the highest used location, number at most the limit of vertex input elements of the device (RHI-26, RHI-29 of `docs/design/slang-rhi.md`), otherwise `ErrorKind::InvalidArgument`.

`slango` runs these checks before the call, so that `slang-rhi`, which refuses the same cases with its own kind `InvalidArgument`, is not reached with them, and the kind is not the `Backend` of SLO-42.

`uniform` parameters of a render entry point stay rejected by SLO-23 on both backends, so that one shader behaves alike on both, although the backend `rhi` could set them through the entry point objects.

## 8. Buffers

SLO-10. A call MUST accept as the element type `T` of a bound `Buffer<T>` only the types of a sealed trait whose byte representation equals the std430 layout of the matching Slang element type without padding. Each such type MUST have a test that checks its size against the stride that reflection reports, because a Rust type with another layout would place every element after the first at a wrong offset (Section 3.4).

The sealed trait is `Element`, and it holds exactly these types, whose strides the test of SLO-10 checks against reflection (Section 3.3):

| Rust type | Slang type | Stride in bytes |
| --- | --- | --- |
| `f32`, `i32`, `u32` | `float`, `int`, `uint` | 4 |
| `[f32; 4]`, `[i32; 4]`, `[u32; 4]` | `float4`, `int4`, `uint4` | 16 |

A further type joins `Element` only with the test of SLO-10. A call or a draw compares `T` with the element of its parameter as Section 9.3 states.

`Element`, `VertexElement`, and `Value` have blanket implementations for every type that implements the hidden trait `slango::__private::StructElement`, which the structs that `slango-macros` generates implement (SLM-8 of `docs/design/slango-macros.md`). The traits stay sealed for users (API-8), because the hidden trait is not part of the API.

Such a type has no test of SLO-10. The generated code asserts its size and field offsets against Slang's layout instead (SLM-18 of that design).

A buffer of such a type binds only to a structure or a composite element (Section 9.3). A dispatch or a draw rejects a value of it given to `set` (Section 9.2), and a draw rejects a vertex buffer of it (Section 8.2).

The size of a buffer is its length times the stride. Elements and the scalars of plain values (Section 9.2) are written little-endian, and elements are read the same way. These rules settle the creation of a buffer and its contents:

- `Buffer::from_slice` creates a buffer that holds a copy of a slice, and `Buffer::zeroed` creates one of a given length whose bytes are zero.
- A buffer has the usages `STORAGE`, `COPY_SRC`, `COPY_DST`, `VERTEX`, and `INDEX`, the last two for the draw (Section 8.2).
- A buffer of length zero is allowed, and `read` returns it as an empty vector without work on the GPU. A call rejects it as an argument (Section 9.3).
- An element of 0 bytes gives `ErrorKind::InvalidArgument` from `Buffer::from_slice` and `Buffer::zeroed`. No built-in element has 0 bytes, but a wrong implementation of `StructElement` with a stride of 0 would otherwise make `Buffer::read` panic.
- No method writes new contents into an existing buffer.

SLO-11. `Buffer::read` MUST block until every dispatch submitted before it has finished and MUST return the elements as a `Vec<T>`, because a caller must not observe a partial result.

A blocking read is the only synchronization that `slango` offers. It copies the buffer to a staging buffer, maps it, and blocks the calling thread until the device has finished.

### 8.1. Textures and Samplers

SLO-24. The walk MUST accept a `Texture2D` with a `float` result and a `SamplerState` in every stage. It MUST reject every other texture or sampler kind, including `Sampler2D`, with `ErrorKind::UnsupportedParameter`, because only these two are in the supported set of parameters (Section 7.1).

This is the walk's own check of that set and no screen for a defect of naga (Section 1.6).

The snapshot records a texture as a sampled texture and a sampler as a sampler, each with its parameter path, set, binding, and usage. The backend builds the layout entries of Section 3.9, visible to the compute stage in a kernel and as Section 7.5 states in a render pipeline. The setters `texture` and `sampler` are in Sections 9.1 and 9.5.

Storage textures, texture arrays, cube maps, and textures of other shapes or result types stay rejected (Section 1.5). A subscript of a texture fails as Section 3.5 states.

These rules settle textures:

- `Texture::from_pixels` takes a device, a size `[width, height]`, and a slice of `[u8; 4]` pixels in rows from the top, and creates an `Rgba8Unorm` texture. It takes no format. `Texture::new` takes a device, a size, and a `TextureFormat`, and creates a texture whose bytes are zero.
- `TextureFormat` has the colour formats `Rgba8Unorm`, `Rgba8UnormSrgb`, `Bgra8Unorm`, and `Bgra8UnormSrgb`, and the depth format `Depth32Float`.
- A texture of a colour format has the usages `TEXTURE_BINDING`, `RENDER_ATTACHMENT`, `COPY_SRC`, and `COPY_DST`. A `Depth32Float` texture has `RENDER_ATTACHMENT` only, so it is used as a depth target and is neither sampled nor read.
- A width or height of zero, a size over the device's limit, or a number of pixels other than the width times the height gives `ErrorKind::InvalidArgument`.
- Every texture has one mip level and one sample.
- `Texture::size` and `Texture::format` return the size and the format.
- `Texture::read` returns the pixels of a colour texture as a `Vec<[u8; 4]>` in rows from the top, in the byte order of its format, and blocks as `Buffer::read` does (SLO-11). It gives `ErrorKind::InvalidArgument` for a `Depth32Float` texture, even a wrapped one with `COPY_SRC`, and for a texture without `COPY_SRC`.

These rules settle samplers. `Sampler::new` takes a device and a `SamplerOptions`, whose fields are these:

- `filter`: A `FilterMode`, `Nearest` by default or `Linear`, for magnification and minification.
- `address_mode`: An `AddressMode`, `ClampToEdge` by default, `Repeat`, or `MirrorRepeat`, for every axis.

The other settings of a sampler, such as its mipmap filter, are wgpu's defaults, with no comparison and no anisotropy.

The module `slango::wgpu` has `texture`, which returns the wgpu texture of a `Texture` as an `Option` (SLO-31), and `texture_from_view`, which wraps a wgpu texture view of a device, such as the view of a surface texture, as a `Texture` (SLO-4). No function returns the view of a `Texture`. `texture_from_view` takes the size, format, and usages from the viewed texture through `TextureView::texture` (read, wgpu 30.0.1).

`texture_from_view` gives `ErrorKind::InvalidArgument` for a format that `TextureFormat` lacks. It does the same for a texture that is not two-dimensional with one sample, one mip level, and one array layer, so that `size` and `read` describe what a kernel samples.

It does not check which device the view belongs to. A call, a draw, or a read that needs a usage that such a texture lacks gives `ErrorKind::InvalidArgument`.

### 8.2. Vertex and Index Buffers

SLO-25. `slango` MUST provide a sealed trait `VertexElement`, a supertrait of `Element`, for every `Element` and the packed arrays of 2 and 3 `f32`, `i32`, or `u32`, because a vertex attribute reads packed vectors, such as `Float32x3`.

`VertexElement` holds `f32`, `i32`, and `u32` and the arrays of 2, 3, and 4 of each, with the packed strides 4, 8, 12, and 16 bytes. `Element` keeps its six types and their strides (Section 8).

`Buffer::from_slice`, `Buffer::zeroed`, `Buffer::read`, `Buffer::len`, and `Buffer::is_empty` take the bound `VertexElement`. `Call::bind` keeps the bound `Element`, because only those types have the stride test of SLO-10. Every buffer has the usages `VERTEX` and `INDEX` (Section 8).

The reflected scalar type and component count of an attribute map to its vertex format, such as `float` to `Float32`, `float2` to `Float32x2`, `int3` to `Sint32x3`, and `uint4` to `Uint32x4`. The draw checks the element type of a vertex buffer against its attribute, such as `[f32; 2]` for a `float2` (Section 9.5). A vertex buffer of a generated struct (Section 8) has no type of a vertex attribute, so the draw rejects it with `ErrorKind::InvalidArgument`, because no vertex attribute holds a structure.

An index buffer is a `Buffer<u32>`, read with `IndexFormat::Uint32`.

### 8.3. Buffers, Textures, and Samplers on the Backend `rhi`

SLO-40. On the backend `rhi`, a `Buffer<T>` of a length above zero MUST be one buffer of `slang-rhi` in `DeviceLocal` memory with the size of Section 8, the element size of `T`, and the usages `SHADER_RESOURCE`, `UNORDERED_ACCESS`, `VERTEX_BUFFER`, `INDEX_BUFFER`, `COPY_SOURCE`, and `COPY_DESTINATION`. A buffer of length zero MUST hold no buffer of `slang-rhi`. `Buffer::read` MUST wait on the device's queue with `Queue::wait_on_host` and then read the whole buffer with `Device::read_buffer`.

These usages give a buffer every use that a call and a draw make of it on the backend `wgpu` (Section 8), and `slang-rhi` checks each use against them (RHI-11). `slang-rhi` refuses a buffer of size 0 (RHI-8), while `slango` allows one that no call binds (Section 8), so the backend creates nothing for it and `read` returns an empty vector without work on the GPU, as on the backend `wgpu`. The wait on the host makes `read` see every dispatch and draw submitted before it, which SLO-11 requires.

`Buffer::from_slice` passes the bytes of the slice as initial data, and `Buffer::zeroed` passes as many zero bytes. The creation checks the size against the largest buffer that the device allows, from the limits of `slang-rhi` (RHI-29 of `docs/design/slang-rhi.md`), and fails with `ErrorKind::InvalidArgument` as on the backend `wgpu`. Every other rule of Section 8 holds on both backends.

Every element type of `Element` has the same stride under the layouts of the backend `rhi`, because a scalar of 4 bytes and a vector of four of them are packed in every layout of Section 3.11. The check of SLO-10 against reflection runs on both backends, so a difference shows as a failed test (Section 13.7).

A structure has the layout of the build's backend: std430 on the backend `wgpu`, and the packed layout that the scalar layout of SPIR-V and the layout of DXIL share on the backend `rhi` (Section 3.11). A build has one backend (SLO-29), so every structure in it has one layout. This rule answers the question of the layout of structures on the backend `rhi`, by the owner's decision 5 of Section 4.6:

- The offsets and the stride of a structure in a buffer always come from the reflection of the program on the build's target, and never from a layout that `slango` assumes.
- A generated struct of `slango-macros` has the layout of the build's backend, because the macro generates one form of the struct for each backend and the build compiles one (SLM-26 of `docs/design/slango-macros.md`).
- A buffer of a generated struct whose stride or field offsets differ from the reflected structure fails the check of its element at the call or the draw with `ErrorKind::InvalidArgument` (SLM-26 of that design). A buffer of a built-in `Element` bound to a structure keeps the check of its stride, and a structure of the wrong layout always has the wrong stride (Section 3.11). So a mismatch fails the check and never corrupts the elements.
- A test that writes the bytes of a structure by hand, such as with a hand-written stride, writes them for the build's backend, under the `cfg` of the backend feature.

SLO-45. On the backend `rhi`, `Texture::new` MUST create a texture of a colour format with initial data of zero bytes, and a texture of `Depth32Float` followed by one render pass that clears it to the depth 0 and draws nothing, before it returns. Every new texture then holds only zero bytes on both backends, as the documentation of `Texture::new` promises and as wgpu does, while the Vulkan backend of slang-rhi leaves a texture without initial data undefined and `slang-rhi` refuses initial data for a depth format (Section 3.11). A draw that loads the depth of a new depth target without `clear_depth` therefore starts from 0 on both backends.

On the backend `rhi`, a `Texture` is a two-dimensional texture of `slang-rhi` with one mip level, one sample, and one layer, with its default view, and a `Sampler` is a sampler of `slang-rhi` (RHI-23, RHI-24 of `docs/design/slang-rhi.md`). They follow every rule of Section 8.1, with these mappings:

| `slango` | `slang-rhi` |
| --- | --- |
| `TextureFormat::Rgba8Unorm`, `Rgba8UnormSrgb`, `Bgra8Unorm`, `Bgra8UnormSrgb` | The formats of the same names |
| `TextureFormat::Depth32Float` | `D32Float` |
| The usages of a colour texture | `SHADER_RESOURCE`, `RENDER_TARGET`, `COPY_SOURCE`, and `COPY_DESTINATION` |
| The usage of a depth texture | `DEPTH_STENCIL` only |
| `FilterMode::Nearest`, `Linear` | `Point` or `Linear` for minification and magnification, and `Point` for the mip filter, as wgpu's default |
| `AddressMode::ClampToEdge`, `Repeat`, `MirrorRepeat` | `ClampToEdge`, `Wrap`, or `MirrorRepeat` on every axis |

`Texture::from_pixels` passes its pixels to `slang-rhi` as initial data, `Texture::new` creates its texture as SLO-45 states, and `Texture::read` reads the texture back through `slang-rhi` after a wait on the host, with the rows of the result from the top, as Section 8.1 states (RHI-28). Every other setting of a sampler is the default of wgpu that Section 8.1 names, and `slang-rhi` writes it explicitly, because the defaults of slang-rhi differ: its address mode is `Wrap` and its filters are `Linear` by default (Section 11 of `docs/design/slang-rhi.md`).

## 9. The Call

### 9.1. Arguments by Name

A kernel's parameters are named globals and named parameters of the entry point, with no positional order, so the call addresses each one by its parameter path. `Kernel::call` returns a builder.

Its setter `set` records a plain value at a parameter path, its setter `bind` records a buffer for a parameter path, and both are infallible (API-6). This answers Q-18 of the architecture design for M3.

The builder `Call` follows these rules:

- It borrows the kernel, the parameter paths, and the bound buffers, and clones none of them.
- `set` and `bind` take `&mut self`, and `dispatch` and `dispatch_groups` take `&self`. A dispatch therefore does not consume the call, and the same call can dispatch again with the same arguments.
- A later `set` or `bind` for a path replaces the earlier argument for that path, whichever setter gave it.
- Every mistake in the arguments is reported by the dispatch, never by a setter (Section 9.3).

The setters `texture` and `sampler` record a `Texture` or a `Sampler` for a parameter path under the same rules, and are infallible too.

### 9.2. Marshalling

SLO-12. The trait through which `set` takes a value MUST be sealed (API-8). It MUST check the value against the reflected scalar type, shape, and size of the parameter before it writes any byte, because a wrong value otherwise reaches the GPU as wrong numbers with no error.

SLO-13. `slango` MUST write every plain value at its reflected offset, element by element for vectors, matrices, and arrays, and MUST NOT copy the bytes of a Rust struct as a whole, because the reflected layout differs from every Rust layout (Section 3.4).

The sealed trait is `Value`, with `Copy` as a supertrait, so that `set` takes the value itself. It holds `bool`, `i32`, `u32`, and `f32`, which stand for `bool`, `int`, `uint`, and `float`, and arrays `[T; N]` of a value type, nested to any depth. A Rust array stands for any Slang type of the same nesting:

- `[f32; 3]` stands for a vector `float3` or an array `float[3]`.
- `[[f32; 3]; 2]` stands for a matrix `float2x3`, whose first index is the row, an array of vectors `float3[2]`, or an array of arrays `float[2][3]`.

A value matches its parameter when the scalar types are equal and the lengths of the nesting are equal, from the outermost to the innermost. The reflected size enters only when the value is written. A value that does not match gives `ErrorKind::InvalidArgument`, whose message names the parameter path, the Slang type, and the Rust type, and no byte is written.

A value of a generated struct (Section 8), or an array of them, never matches, and the message names the struct by its Slang name, because a structure among the plain values is set leaf by leaf (Section 7.1).

Each scalar is written at its reflected offset, and a `bool` as a 32-bit 0 or 1. A matrix is written row by row, as Section 3.4 states. Global plain values and plain values in constant buffers and parameter blocks go into uniform buffers that the call owns, and plain values of the entry point go into the immediates of the dispatch.

### 9.3. Validation Before Dispatch

SLO-14. The dispatch MUST check, before it submits any work, that every parameter path names a parameter of the kernel and that every used parameter is set. It MUST check that every value matches its parameter (SLO-12) and that the element stride of every bound buffer equals the reflected stride.

It MUST run the check of the element type of every bound buffer that SLM-5 of `docs/design/slango-macros.md` requires. It MUST check that every buffer belongs to the kernel's device and that every workgroup count is within the device's limit. It MUST return `ErrorKind::InvalidArgument` naming the parameter when a check fails, because wgpu reports these mistakes late, with less context, or not at all.

Each `Device` carries an identity that its clones share, because `==` cannot tell two wgpu devices apart (Section 3.2). A buffer belongs to the device whose identity it holds. Two `Device` values built separately from the same wgpu device therefore count as different devices.

The checks of SLO-14 follow these rules:

- A parameter that the kernel does not use may be omitted. An argument given for one gets every check and is then ignored.
- Every plain value of a used uniform buffer, and every plain value of the immediates, must be set. The immediates count as used (Section 7.1).
- A bound buffer must not be empty.
- An argument of one kind given for a parameter of another kind, among plain values, buffers, textures, and samplers, is rejected.

Textures and samplers follow these further rules:

- A texture or a sampler must belong to the kernel's device, as a buffer does.
- A bound texture must have a colour format and the usage `TEXTURE_BINDING`. Of the colour textures, only one from `slango::wgpu::texture_from_view` can lack that usage.
- Every texture and sampler that the kernel uses must be given.

The check of the element type of a bound buffer (SLM-5 of `docs/design/slango-macros.md`) reads the element type that Section 7.1 records, with these rules:

- When the element of the parameter is a scalar or a vector of `bool`, `int`, `uint`, or `float`, the Rust element type of the buffer must have the same scalar type and number of components, such as `f32` for `float` or `[u32; 4]` for `uint4`. Otherwise the error names the parameter, the Slang type, such as `float`, and the Rust type, such as `u32`.
- When the element of the parameter is a scalar or a vector of another scalar type, no Rust element type matches it, and the error names it with the name that Slang gives it.
- When the element of the parameter is a structure, a matrix, or an array, the stride alone is checked, so a `Buffer<[f32; 4]>` binds to a `StructuredBuffer<float2x2>` and to a buffer of a structure of 16 bytes.
- A buffer of a generated struct (Section 8) binds only to such an element, by its stride and, from M10, by the offset of each field (SLM-26 of `docs/design/slango-macros.md`), and never to a scalar or vector element. The error then names the struct by its Slang name.

A scalar or vector element matches exactly, and a structure by its stride alone, by the owner's decision. The exact check also covers `bool` and every scalar type that no `Element` has, such as `half`, `double`, and the 8-bit and 16-bit integers, because SLM-5 covers every scalar and vector and the exact check is the sounder choice.

No `Buffer<T>` can therefore be bound to such a parameter, such as a `Buffer<u32>` to a `StructuredBuffer<bool>`. Matrices and arrays keep the check of the stride alone, as structures do, because SLM-5 requires the exact check only for scalars and vectors.

The dispatch runs the checks in this order and returns the first failure, all before any call of wgpu:

1. Each given argument, in the order given: its path, the kind of its parameter, and its value, its buffer, or its texture or sampler. A buffer is checked for its device, then that it is not empty, then its stride, then its element type, so a buffer of another stride gets the error about its stride. A texture is checked for its device, then its format, then its usage.
2. Each used parameter that no argument gives, in the order of the bindings, followed by the immediates.
3. One buffer bound to a read-only and to a read-write parameter (SLO-44).
4. The workgroup count along each axis against the device's limit.

A mistake of the caller in these checks gives `ErrorKind::InvalidArgument`, whose message names the kernel and the parameter. A reflected layout that cannot hold a value gives `ErrorKind::UnsupportedParameter`.

SLO-44. A dispatch and a draw MUST fail with `ErrorKind::InvalidArgument`, naming both parameters, when one buffer is bound to a read-only and to a read-write parameter of the same call or draw, on both backends, in the shared checks. Until M10, wgpu's validation rejected such a dispatch with `ErrorKind::Backend` (Section 3.8). On D3D12, the read-only binding needs a shader-resource state and the read-write binding the state of unordered access, which one buffer cannot hold at once, and slang-rhi keeps one required state for each resource, so the later binding would win without an error (inferred, not run). One check before both backends gives the same kind and text on both and closes that gap of RHI-20 of `docs/design/slang-rhi.md`. Two parameters that are both read-only, or a buffer bound to one read-write parameter, stay valid. The change of the kind on the backend `wgpu` is part of the migration to version 0.2 (Section 17.6).

SLO-15. The dispatch MUST open an error scope around the work it records and submits, and MUST return each validation error that remains as `ErrorKind::Backend`, because an error that reaches wgpu's uncaptured-error handler is lost to the caller.

### 9.4. Dispatch and Results

`dispatch` takes the element counts of three axes and derives each workgroup count by rounding the element count divided by the thread group size up. The kernel therefore guards the threads past the end itself, as the illustration of Section 5.2 does.

`dispatch_groups` takes the workgroup counts directly. A workgroup count over the device's limit is an error (Section 9.3).

A count of zero along any axis runs no thread, and the dispatch succeeds once the arguments pass the checks. A dispatch returns once its work is submitted, without waiting for it. Its results come back through `Buffer::read` on a buffer that the caller bound (SLO-11), which sees them because it submits its copy on the same queue after the dispatch.

### 9.5. The Draw

`RenderPipeline::draw` returns the builder `Draw`, which follows the rules of Section 9.1 for `Call`. Its setters `set`, `bind`, `texture`, and `sampler` record parameters as those of a call do (Sections 8.1 and 9.1).

A matrix is set row by row (Section 9.2), so a transform written in Rust as rows works with `mul(transform, vector)` in Slang (Section 3.9). These setters record the rest of a draw:

- `vertex`: The buffer of a vertex attribute, named by its parameter path (Section 7.4).
- `target`: The colour target, a `Texture`, which every draw needs.
- `clear`: A colour as `[f32; 4]`, to which the draw clears the colour target first. Without it, the draw keeps the target's contents.
- `depth`: The depth target, a `Texture`, which a pipeline with a depth format needs and every other pipeline rejects.
- `clear_depth`: A depth as `f32`, to which the draw clears the depth target first. Without it, the draw keeps its contents.

The arguments of parameters and the vertex buffers are separate namespaces. A later `vertex` for a path replaces an earlier `vertex` for that path, and the setters of parameters replace each other as those of a call do.

The draw ends with `submit`, which takes a count of vertices and a count of instances, or with `submit_indexed`, which takes a `Buffer<u32>` of indices, a count of indices, and a count of instances. Each records one render pass with one draw, submits it, and returns once the work is submitted, without waiting, as a dispatch does (Section 9.4). A count of zero is a successful draw once the arguments pass the checks, and it still clears the targets that the draw clears.

SLO-26. The draw MUST run the checks of SLO-14 on its parameters and the checks below before it submits any work. It MUST return `ErrorKind::InvalidArgument` naming the pipeline's entry points and the parameter or target, because wgpu reports these mistakes late, with less context, or not at all.

The draw shares the checks of its parameters with a call, so every rule of Section 9.3 for a parameter holds for a draw. It runs the checks in this order and returns the first failure, all before any call of wgpu:

1. The given arguments: each argument of a parameter, in the order given, as Section 9.3 states, then each vertex buffer, in the order given. A vertex buffer names a vertex attribute, belongs to the device, is not empty, and has the attribute's scalar type and component count.
2. The missing arguments: each used parameter that no argument gives, as Section 9.3 states, and then each used vertex attribute without a buffer, in the order of locations. An unused parameter or attribute may be omitted. An argument or a vertex buffer given for one gets the checks of step 1 and is then ignored, so its length does not count in step 4.
3. The targets. A colour target is required, and it belongs to the device, has the pipeline's colour format, and has the usage `RENDER_ATTACHMENT`.

   A depth target is required exactly when the pipeline has a depth format. It is checked as the colour target is, against the depth format, and must have the size of the colour target.

   A depth given to `clear_depth` must be from 0.0 to 1.0, otherwise `ErrorKind::InvalidArgument`. This check runs even when the draw has no depth target, and such a draw ignores a valid depth.
4. The counts. The used vertex buffers that step per vertex must have one length, and so must those that step per instance.

   With `submit`, the count of vertices is at most the length of the buffers per vertex. With `submit_indexed`, the index buffer belongs to the device and is not empty, and the count of indices is at most its length and is not bounded by the vertex buffers. The count of instances is at most the length of the buffers per instance, when there are any.

The draw does not read the values of an index buffer (Section 16). It records and submits its work inside an error scope, and returns each error that remains as `ErrorKind::Backend`, as SLO-15 states for a dispatch.

`slango` makes no check of its own that one texture is both sampled and a target of one draw. wgpu's validation rejects it (Section 3.9), and on the backend `rhi`, `slang-rhi` refuses it (Section 9.7). A draw checks one buffer bound to a read-only and to a read-write parameter as a dispatch does (SLO-44).

### 9.6. The Call on the Backend `rhi`

SLO-39. On the backend `rhi`, a dispatch MUST run every check of SLO-14 in the order of Section 9.3 before any call into `slang-rhi`, with the limit of thread groups that `slang-rhi` reports for the device (RHI-29 of `docs/design/slang-rhi.md`). It MUST then build the same byte image of each uniform block and of the immediates that the backend `wgpu` builds (Section 9.2), and write each used argument into the shader objects of the kernel's pipeline by the rules of this section, all inside one compute pass of one command encoder, before it submits the command buffer.

The checks and the marshalling read only the snapshot and the arguments, so both backends share them, and a mistake gets the same kind and text on both.

The backend reaches each parameter through the objects of `slang-rhi` in this way:

- The root object of the bound pipeline holds the global parameters. The object of the entry point, `ShaderObject::entry_point(0)`, holds the `uniform` parameters of the entry point, which the backend `wgpu` passes as immediates.
- A parameter path is resolved one segment at a time from the root object. A segment that names a constant buffer, a named `cbuffer`, or a parameter block descends into its object with `ShaderObject::object` (RHI-22 of `docs/design/slang-rhi.md`). The segment that names a buffer, a texture, a sampler, or the outermost structure or value of a plain value ends the descent, and `offset_of` gives its offset in the object that the descent reached.
- A plain value is written as part of its top-level field. For each top-level field of an object that holds plain values, the backend writes the bytes of the byte image from the field's uniform offset for its uniform size with one `set_data`. The offsets of the snapshot and of the shader object come from the same reflection of the same program, so the slice holds exactly the leaves of that field (SOA-10). A matrix among them is written row by row at its reflected row stride (SLO-46).
- A buffer is written with `set_buffer`, a texture with `set_texture`, and a sampler with `set_sampler`, each at array index 0 (RHI-25).
- A parameter that the kernel does not use gets the checks of SLO-14 and is then not written, as on the backend `wgpu`.

A lookup that finds no parameter for a path of the snapshot would be a defect of `slango`, because both come from one program. The dispatch then fails with `ErrorKind::Backend` and a message that names the path, and it submits nothing.

The dispatch derives its thread group counts as Section 9.4 states and calls `ComputePass::dispatch`, finishes the encoder, and submits the command buffer to the queue of the device. It returns once the work is submitted, without waiting, as on the backend `wgpu`. `Buffer::read` waits on the host (SLO-40).

One buffer bound to a read-only and to a read-write parameter of one call fails the shared check of SLO-44 before any call into `slang-rhi`, as on the backend `wgpu`.

### 9.7. The Draw on the Backend `rhi`

SLO-41 creates the render pipeline. A draw on the backend `rhi` runs every check of SLO-26 in the order of Section 9.5 before any call into `slang-rhi`, and then records one render pass with `CommandEncoder::render_pass` (RHI-27 of `docs/design/slang-rhi.md`) with these parts:

- The colour attachment is the view of the colour target. It is cleared to the colour of `clear` when the draw has one, and otherwise keeps its contents.
- The depth attachment, when the pipeline has a depth format, is the view of the depth target. It is cleared to the depth of `clear_depth` when the draw has one, and otherwise keeps its contents.
- The render state sets one viewport from the origin over the whole colour target with the depth range 0 to 1, one scissor rectangle over the whole target, the vertex buffer of each used attribute in its stream, and, for `submit_indexed`, the index buffer with 32-bit indices.
- The parameters are written into the root object of the bound pipeline as Section 9.6 states.
- `submit` draws the count of vertices and the count of instances, and `submit_indexed` draws the count of indices and the count of instances.

The draw then finishes the encoder and submits the command buffer, and returns without waiting, as on the backend `wgpu`.

The conventions of a draw of Section 3.9 are conventions of `slango` on both backends, by the decision of the design that answers SOQ-9 (Section 15). On the backend `rhi`, slang-rhi meets them through the negative viewport height of its Vulkan backend and the native conventions of D3D12 (Section 3.11), and the `gpu` tests of Section 13.7 check each convention on both backends (SOA-9). One convention differs: on Vulkan, slang-rhi clamps a depth outside the range 0 to 1 instead of clipping the geometry (Section 3.11). The crate documentation states that difference, and no test of `slango` depends on it.

`slang-rhi` refuses a texture that the pass both samples and draws into, because that breaks valid use of the graphics API (RHI-27). The draw then fails with `ErrorKind::Backend`, as on the backend `wgpu` (SLO-42).

## 10. Errors

SLO-16. `slango` MUST have one public error type as ERR-1 defines, with these kinds of `ErrorKind`, because callers match on the kind (ERR-1):

| Kind | Meaning |
| --- | --- |
| `Compile` | Slang failed to compile or link the module. `source()` returns the error of `slang-compiler` with its diagnostics. For the rejection of a vertex or fragment entry point, `source()` returns an error whose text is the text of that error, and the `Display` text of the error does not hold it (Section 7.4) |
| `NoAdapter` | The backend found no adapter that matches the options |
| `DeviceRequest` | The adapter refused the device that the options request |
| `UnsupportedShader` | The backend rejected a kernel at load, or a render pipeline at its creation, as the text after this table states. The source error carries the backend's text |
| `UnsupportedParameter` | A kernel or a render pipeline has a parameter that the walk or SLO-23 rejects, or whose two stages' bindings conflict (Sections 7.1, 7.4, and 7.5), or, at a call or a draw, the reflected layout of a plain value cannot hold the value (Section 9.3) |
| `MissingFeature` | A kernel needs a device feature that the device lacks |
| `InvalidArgument` | A check of Section 9.3 or of SLO-26, including a value that does not match its parameter (Section 9.2), an unknown kernel or entry point name, a wrong option of a render pipeline, a file that cannot be read as text, a buffer larger than the device's limit, whose size overflows, or whose elements have 0 bytes, or a texture whose size or pixels Section 8.1 rejects |
| `Backend` | wgpu reported a validation or out-of-memory error in an error scope at load or at the creation of a render pipeline, as the text after this table states, any error while a buffer, texture, or sampler is created or read, or any error that wgpu reports during a dispatch or a draw (SLO-15). On the backend `rhi`, `slang-rhi` failed as SLO-42 states |
| `Unsupported` | The backend of the build does not offer the operation, such as a pipeline kind that the backend lacks (SLO-42) |

The load creates every shader module, bind group layout, pipeline layout, and compute pipeline inside error scopes for validation, internal, and out-of-memory errors, so that no error reaches wgpu's uncaptured-error handler. It maps each error that a scope returns to a kind:

- Any error while the shader module is created gives `UnsupportedShader`.
- An internal error while the layouts or the pipeline are created gives `UnsupportedShader`, because a shader that the backend cannot translate is reported as an internal error.
- A validation or out-of-memory error while the layouts or the pipeline are created gives `Backend`.

The message names the kernel, and the backend's text is returned unchanged as the source error.

`Module::render_pipeline` creates the vertex shader module, the fragment shader module, the layouts, and the render pipeline, each in its own error scope, and maps each error that a scope returns to a kind:

- Any error while a shader module is created gives `UnsupportedShader`.
- An error while the bind group layouts and the pipeline layout are created maps as for a kernel.
- A validation or internal error while the render pipeline itself is created gives `UnsupportedShader`, and an out-of-memory error gives `Backend`. Such a validation error reports a mismatch between the two stages or with the pipeline, such as varyings in another order or of another type (Section 3.9), which only a change of the shader fixes.

The message names both entry points, and the backend's text is returned unchanged as the source error. Every other failure of rendering has the kind that Sections 7.4 to 9.5 state. With the kind `Unsupported` of M10, `ErrorKind` has nine kinds.

`NoAdapter` answers Q-22 of the architecture design.

SLO-42. `ErrorKind` MUST gain the kind `Unsupported` for every operation that the backend of the build does not offer. On the backend `rhi`, `slango` MUST map each error of `slang-rhi` to a kind by this table, in one crate-private function, with the error of `slang-rhi` as the source error:

| Kind of `slang-rhi` | Where | Kind of `slango` |
| --- | --- | --- |
| `NoDevice` | `Device::new` | `NoAdapter` |
| Any other kind | `Device::new` | `DeviceRequest` |
| `Compile` | The creation of a shader program or a pipeline | `Compile`, with the diagnostics of Slang in the source error |
| `Slang` | Any call | `Compile` |
| `Unsupported` | Any call | `Unsupported` |
| `InvalidArgument`, `OutOfMemory`, `Failed`, and `Unrecognized` | Any other call | `Backend` |

A code generation that fails for the target of the device is the failure of Slang to compile that `Compile` already names on the backend `wgpu`, so a shader with an error gets one kind on both backends. `slango` checks every argument before it calls `slang-rhi`, so an `InvalidArgument` of `slang-rhi` reports a condition of valid use that only `slang-rhi` checks, such as a texture both sampled and drawn into (Section 9.7), and it is `Backend` as wgpu's validation errors are. The kinds `UnsupportedShader` and `MissingFeature` stay those of the backend `wgpu` (Section 6.4).

`ErrorKind` is `#[non_exhaustive]`, so the new kind is no breaking change for a caller who matches with a wildcard arm (SLO-32). With one backend for each build, no operation of M10 meets a backend whose feature is off, so the kind serves two needs:

- Within M10, the code steps before the rendering step return it from the operations of rendering on the backend `rhi` (Section 6.4). After the rendering step, no operation of M10 returns it.
- M12 to M14 and M18 add pipeline kinds on the backend `rhi` only and report them with this kind on the backend `wgpu` (Section 15.13 of the architecture design).

The kind arrives with the first need, in the step that adds compute to the backend `rhi`, so that those steps merge a crate that reports what it lacks, and it stays for the later milestones.

`slango` returns no warning of Slang on success, on either backend, so the backend `rhi` needs none from `slang-rhi` (RHQ-4 of `docs/design/slang-rhi.md`, which Section 6.5 of that design answers).

## 11. Threading

SLO-17. `Device`, `Module`, `Kernel`, `Buffer<T>`, `RenderPipeline`, `Texture`, `Sampler`, `Call`, and `Draw` MUST be neither `Send` nor `Sync` in the build of either backend, and each MUST hold a private marker field of the type `PhantomData<*const ()>` that makes it so. `Error`, `ErrorKind`, every options type, and every enumeration MUST be `Send` and `Sync`. `slango` MUST implement neither trait by hand. Each auto trait that a type implements gets a compile-time assertion test, and each that it lacks gets a `compile_fail` doctest (CONC-1).

This is the owner's decision 1 of Section 4.6, with the types that it covers as the design reads it there. The objects of `slang-rhi` are neither `Send` nor `Sync` (Section 3.11), and FFI-9 of the coding standard forbids an `unsafe impl` without a threading rule of slang-rhi. A type whose auto traits followed its backend would be `Send` in a build with `wgpu` and not in a build with `rhi`, so a program that compiles on one backend could fail to compile on the other. The marker makes the traits one fact of the API in both builds.

The marker is a project-defined pattern, because the coding standard has no rule that makes a type without a foreign handle neither `Send` nor `Sync`. The sources behind it are S-1 of `docs/rust-references.md`, the Rustonomicon on `Send`, `Sync`, and `PhantomData`, which states that a raw pointer is neither and that negative implementations are unstable.

The first code step of M10 adds the rule CONC-9 to the coding standard, as Hard Rule 13 of `AGENTS.md` and Section 19 of the coding standard require in the change that first relies on it: a type that MUST be neither `Send` nor `Sync` whatever it holds carries a private field of the type `PhantomData<*const ()>`, and states why in its documentation. The same step changes SIMP-2, as Section 6.1 states.

A program that used `slango` on several threads moves each device and everything created from it to one thread, and sends results as plain data, such as the `Vec<T>` that `Buffer::read` returns. Section 17.6 states the migration. `Error` stays `Send` and `Sync`, so an error can still cross a thread.

`load_module` uses Slang only on the calling thread. On the backend `wgpu`, its Slang objects never leave the call (SLO-5). On the backend `rhi`, the device and its modules hold Slang objects, and the types that hold them are not `Send`, so they stay on the thread that created them, which keeps every rule of Section 10 of `docs/design/slang-compiler.md`. Several threads that load modules at once on devices of their own each use their own global session, which Slang supports (Section 3.5 of `docs/design/slang-compiler.md`), and the sessions of the loads of one device all stay on its thread. `slango` starts no thread (CONC-4).

## 12. Device Features

SLO-18. On the backend `wgpu`, `Device::new` MUST request `Features::IMMEDIATES` with the adapter's maximum immediate size when the adapter offers the feature. The load MUST fail with `ErrorKind::MissingFeature` when a kernel has `uniform` parameters of its entry point and the device lacks the feature or a large enough immediate size.

Without the feature, naga rejects the module with a message about `entryPointParams` that does not name the cause, `Capability Capabilities(IMMEDIATES) is not supported`.

The default immediate size of wgpu is 0, and the adapters of the CI runners offer 128 or 256 bytes (Section 3.8). `Device::new` requests no other optional feature, such as the ones that `double`, `int64_t`, and `half` need (Section 3.5). `DeviceOptions` has no field for them, because a field of type `wgpu::Features` would name a wgpu type outside the module of SLO-4.

A caller who needs another feature creates the wgpu device with it and passes the device and its queue to `slango::wgpu::from_device_and_queue`. A `Device` built that way uses the features that its device has. Rendering needs no optional feature, because a render pipeline has no immediates (SLO-23) and the formats of Section 8.1 need none (Section 3.9).

## 13. Tests, CI, and the Example

### 13.1. Tests

`slango` has one integration-test binary (TEST-10). Every test that needs an adapter sits in a module named `gpu`, selects any adapter, and fails when it finds none (ARCH-22, ARCH-23).

The tests of the reflection walk call Slang and need no adapter, so they sit in the module `layout::tests::ffi` in the default tier and check their snapshots with `insta`. They are unit tests inside the crate, not tests of the integration-test binary, because the snapshot is private and TEST-1 limits integration tests to the public API.

The tests of marshalling, of the checks of a call, and of the workgroup counts are pure and run under Miri.

The tests of loading and of the call are `gpu` tests of the integration-test binary, in the modules `gpu::load` and `gpu::call`, with the shader files that they load under `tests/slango/shaders/`. The tests of the call dispatch kernels and assert every result. They also assert the kind and text of each error of Section 9.3, and that a rejected call leaves its output buffer unchanged.

The test of `AdapterSelection::Software` is compiled out on macOS, because Metal has no software adapter (Section 3.8), and every other `gpu` test runs on all three platforms.

### 13.2. Continuous Integration

The `check` and `coverage` jobs on Linux install lavapipe from Mesa's package `mesa-vulkan-drivers` and run the `gpu` tier. The jobs on Windows and macOS run `just test-gpu`, so they run the same tests on WARP and on hosted Metal.

The tier creates devices, loads modules, dispatches kernels, samples textures, creates render pipelines, and draws offscreen on all three platforms (Sections 3.8 and 3.9). This answers Q-21 of the architecture design.

### 13.3. The Example

Example 2 of the architecture design is the headless binary target `compute-kernel` of the package `slang-rust-examples`, with its source at `examples/src/bin/compute-kernel.rs`. It takes one argument, the path of a Slang file, and runs the kernel `saxpy` of that file. Its shader, `examples/shaders/saxpy.slang`, computes `result = a * x + y` for each element of two input buffers, with the global `uniform` values `a` and `count`.

The example prints the name of the adapter on standard error, and the inputs and results on standard output. It compares the results exactly with the values that it computes on the CPU. That is valid because the inputs are small whole numbers and `a` is 2.5, so every result is exact.

It exits with a failure status on a mismatch or an error, and on an error it prints the error and each of its source errors on standard error. A user runs it from the workspace root, with the Slang library on the library path as the example's documentation describes:

```sh
cargo run -p slang-rust-examples --bin compute-kernel -- examples/shaders/saxpy.slang
```

Its `gpu` tests sit in the integration-test binary `examples/tests/examples/`, so they run on any adapter, on lavapipe in CI (ARCH-21). One compares standard output with an `insta` snapshot, and the others check that a compile error and a missing file fail with the error on standard error. The error tests need an adapter too, because the example creates the device before it loads the file.

The tests in `ffi/compute_kernel.rs` check `--help` and a wrong number of arguments, which fail before the example creates the device, so they need no adapter. The example needs no differentiable function, and the broadcasting case of example 2 waits for the milestone that adds broadcasting.

### 13.4. Tests of Rendering

The tests of the walk for vertex and fragment entry points, vertex attributes, textures, and samplers are `ffi` unit tests in `layout::tests::ffi` with `insta` snapshots, as Section 13.1 states for the walk. The merging of the bindings of two stages and the checks of SLO-26 are pure unit tests that Miri runs, in `render::tests` and `draw::tests`.

The `gpu` tests of rendering sit in the module `gpu::render` of the integration-test binary. They create render pipelines, render offscreen into a `Texture`, read its pixels back, and assert them, on lavapipe, WARP, and hosted Metal in CI (Section 13.2). Their shaders, as source text in the tests or under `tests/slango/shaders/`, cover these cases:

- The conventions of Section 3.9 and a matrix transform.
- Uniforms, a constant buffer, and a parameter block used by one or both stages.
- A sampled texture, a depth target, and an indexed draw.
- Attributes per instance, integer attributes, alpha blending, and back-face culling.

Further `gpu` tests assert the kind and text of each error of SLO-23 and SLO-26, and that a rejected draw leaves its target unchanged. The tests of textures and samplers sit in `gpu::texture`, and `gpu::sample` checks that a compute kernel reads a texture through a sampler.

These tests are the headless check of rendering that Q-24 of the architecture design asks for, as far as `slango` goes. Q-24 stays open for the windowed example itself, which no CI runner can show (Section 13.5).

### 13.5. The Windowed Renderer Example

Example 3 of the architecture design is the binary target `windowed-renderer` of the package `slang-rust-examples`. Its source is the directory `examples/src/bin/windowed-renderer/`, with these files, of which the tests also compile the three modules:

- `main.rs`: The window and the frame loop.
- `scene.rs`: The scene and its reload.
- `format.rs`: The choice of the surface's colour format, as a pure function.
- `retry.rs`: The retry of a frame for which the surface gave no texture, and the time at which the event loop wakes.

`main.rs` also includes two shared modules with a `#[path]` attribute, which example 4 shares (Section 10 of `docs/design/slang-slint.md`). `examples/src/shared/frames.rs` parses the arguments and counts the frames of `--frames`. `examples/src/shared/watched.rs` checks the modification time of the shader.

Its shader is `examples/shaders/textured-quad.slang`, with the vertex entry point `vsMain` and the fragment entry point `fsMain`. The example takes the path of a Slang file and the option `--frames N`.

With `--frames`, it closes the window after `N` drawn frames, prints one line with the number of frames drawn on standard output, and exits, so that the example runs without a person. It then also fails with a message on standard error and a non-zero exit status when it draws no frame for 10 seconds, so that an unattended run cannot wait forever. A user runs it from the workspace root, with the Slang library on the library path and a display:

```sh
cargo run -p slang-rust-examples --bin windowed-renderer -- examples/shaders/textured-quad.slang
```

The example creates the window with winit, and the wgpu instance, surface, adapter, and device itself. It prints the adapter's name on standard error and hands the device and its queue to `slango` with `slango::wgpu::from_device_and_queue`. It configures the surface with the first format, in the surface's own order of preference, that is one of the four colour formats of `TextureFormat` (Section 8.1).

When the surface offers none of them, the example exits with an error that lists the formats that the surface offers.

In each frame, the example wraps the view of the surface texture with `slango::wgpu::texture_from_view` and draws one indexed quad into it, with a checkerboard texture that it creates with `Texture::from_pixels` and a `float4x4` transform. The transform rotates the quad by an angle that grows with time and keeps it square at the aspect ratio of the target. The example keeps clones of the wgpu device and queue that it hands to `slango`, configures the surface through its device, and presents the surface texture through its queue, because wgpu 30 presents through the queue (SLO-31).

The example reloads its shader without a file-watching dependency. Before each frame, it compares the modification time of the file with the time of the last load. When the time changed, it records the new time, loads the module, and creates the pipeline again.

When either fails, the example prints the error and each error that caused it on standard error and keeps drawing with the old pipeline. It does not load the file again until its time changes once more.

A file whose modification time cannot be read counts as unchanged, as it can be while an editor replaces it. Section 7.2 states why the cost of each reload is acceptable.

The window loop and the integration-test binary `examples/tests/examples/` both use `scene.rs`, so that the tests run the code that the window runs (TEST-1 of the coding standard). The tests cover the example in these modules:

- `gpu::windowed_renderer`: Draws the scene offscreen with the example's own shader on any adapter. It checks exact texels, a rotation that is not symmetric, and a quad that stays square in pixels on a wide and a tall target. It also checks a failed reload that keeps the old pipeline and is not tried again, and a successful reload.
- `ffi::windowed_renderer` and `colour_format`: Start the binary to check `--help` and each argument error, which need no window, and check the choice of the colour format.
- `unattended`: Checks the parsing of the arguments, the frame count, and the 10-second bound of `frames.rs` with given times and no window.
- `redraw_retry`: Checks the retry of `retry.rs` and the time at which the event loop wakes, in the same way.
- `ffi::watched`: Checks `watched.rs` on a file in a scratch directory, including a file that is removed and written again.

No test opens a window, because the Linux runner has no display, so example 3 runs with a window locally only (ARCH-21 of the architecture design). CI builds the example, because `check-clippy` compiles every target of the workspace, and runs its scene on the three adapters of Section 3.8. The example has these known limits with a window:

- On the Wayland socket of the development container, with the software adapter, it draws a few frames and then stops, so `--frames` ends with the error of the 10-second bound. The cause is unknown. Under a nested Wayland compositor, it draws, reloads, and exits as this section states.
- On X11, it needs the Xlib library, which the development container lacks.

winit 0.30.13, the version line that the examples of wgpu 30 use, is a dependency of the examples package only. It builds on all three CI runners with no package added to the Linux runners. Section 8 of `docs/design/tooling.md` records its features and the duplicate versions that it brings.

### 13.6. Tests of the Forward-Compatibility Pass

The pass adds no test module to `slango`. Its parts are proved in these ways:

- The `Some` path of SLO-31: four `gpu` assertions compare what an accessor returns for an object of `from_device_and_queue` or `texture_from_view` with `Some` of the wgpu object that it wraps. `tests/slango/gpu.rs` asserts it for `device` and `queue`, and `tests/slango/gpu/texture.rs` and `examples/tests/examples/gpu/shader_playground.rs` assert it for `texture`. They prove the `Some` path on lavapipe, WARP, and hosted Metal (Section 13.2).
- That every accessor always returns `Some` in a build with the feature `wgpu` alone: the exception to `clippy::unnecessary_wraps` on each accessor, which `check-clippy` proves on every run (SLO-36).
- The build with the feature `wgpu`: every recipe that compiles the workspace, which builds `slango` with its default features, and `check-features` for every combination that contains `wgpu` (SLO-29).
- The feature documentation: `check-feature-docs`, which fails when the `# Features` section does not name a feature.
- The exhaustiveness of SLO-32: `check-clippy`. No trybuild case is added, because the compiler itself rejects a struct literal of a `#[non_exhaustive]` type in another crate.

These stay unproven by any automated check:

- The `None` path of SLO-31, because no object of a build with `wgpu` lacks a wgpu object, and `slango::wgpu` does not exist in a build with `rhi` (SLO-36). A unit test of `slang-slint` checks that its mapping of `None` gives the error of SLS-11 (Section 9 of `docs/design/slang-slint.md`).
- The single error of SLO-29 without a backend or with both, because `check-features` skips both combinations. Section 3.10 records what one run by hand of `cargo check -p slango --no-default-features` reported before M10.
- SLO-30. No check fails when a dependent drops `features = ["wgpu"]`, because the entry of `slango` in `[workspace.dependencies]` keeps the default features, so `wgpu` stays on for that dependent. `check-features` runs cargo-hack on each package, but `--no-default-features` there turns off only the features of that package and not the default features of its dependency `slango`. A whole-workspace build would hide the omission anyway, because Cargo unifies the features that the other members request. Review of the four manifests is the check.

SLO-32 needs no change of the code, because every type already follows it.

### 13.7. Tests of the Backend `rhi`

SLO-43. Every `gpu` test of `slango` and of the examples MUST run its assertions on a device of the build's one backend, created through one helper of each test binary, and every failure MUST name that backend. A test that the backend `rhi` cannot run MUST carry a skip under the `cfg` of the feature `rhi`, with a reason that names the limit, and MUST still run on the backend `wgpu`.

One test source then checks whichever backend the build has, and the build with the default features runs exactly the tests of today. The helper creates the device with `AdapterSelection::Default`, and the name of the backend that failures show comes from a constant of the test binary under the `cfg` of each feature. A test that calls `slango::wgpu`, or that holds a hand-written stride or offset of a structure, is compiled under the `cfg` of its backend (Section 8.3).

A skip reason states the limit in plain words, such as the defect of slang-rhi on D3D12 below, and never a milestone or a step, which Hard Rule 15 of `AGENTS.md` forbids in code.

The recipes run the tests of the backend `rhi` (TOOL-53 of `docs/design/tooling.md`):

- `test-rhi` runs the `gpu` tests and the doctests of `slango` with `--no-default-features --features rhi`, and the `gpu` tests of the examples with the features `rhi` and `vendored` of the examples package and without its default features.
- `check-rhi` runs Clippy and the documentation build on `slango` with `--no-default-features --features rhi`, and Clippy on the examples package with the same features as `test-rhi`.
- `just check`, `test-gpu`, and every other recipe keep the default features, so they build no slang-rhi.

A code step that adds part of the backend `rhi` adds the commands of TOOL-53 that its own work makes pass, and a filter of nextest leaves out the test modules of what a later step adds, such as the test modules of rendering before the rendering step. The filter is in the recipe, so no test source names a step. The step that adds the missing part removes its filter.

The CI job `vendored` runs both recipes on Linux, where lavapipe provides a Vulkan adapter, and on Windows, where WARP provides a D3D12 adapter (Section 7.4 of `docs/design/tooling.md`). These are the adapters that the exit criteria of M10 name.

The tests of the backend `rhi` cover these cases:

- Every `gpu` test of Sections 13.1 and 13.4 on the backend `rhi`, which includes each kind of parameter of Section 7.1, every element type, every check of SLO-14, SLO-26, and SLO-44 with its kind and text, the conventions of Section 3.9, and the reads of buffers and textures.
- Each mapping of SLO-42 that a test can produce, such as a shader that fails code generation for the target.
- A reload of a changed file under the same module name on one device, and two files of the same stem on one device, which SLO-37 makes work.
- The marshalling of matrices of every shape in a constant buffer, in a parameter block, and as `uniform` parameters of the entry point, at the reflected row stride (SLO-46).
- A new colour texture and a new depth texture that read as zero, and a draw that loads the depth of a new depth target without `clear_depth` (SLO-45).
- A vertex attribute without a semantic, which the backend `rhi` rejects (Section 7.7).

The tests of the backend `wgpu` gain the cases of SLO-44, SLO-45, and SLO-46 on that backend.

A test whose kernel combines a `ConstantBuffer` and a global `uniform` fails on D3D12 through RKB-2 of `docs/design/slang-rhi.md`. Such a test skips the backend `rhi` on Windows, with a reason that says so, while it runs on that backend on Linux. Hard Rule 14 of `AGENTS.md` forbids a new test of the defect, so no test asserts the failure.

The shader `examples/shaders/playground.slang` of the example `shader-playground` and the `gpu` test of `slango` that draws with `SV_VulkanVertexID` (`tests/slango/gpu/render.rs`) take the positions of their vertices from a vertex buffer instead of the vertex index, because no shader that reads the vertex index runs on both backends on D3D12 (Section 6.4). The program `shader-playground` creates that vertex buffer as its scene does.

The examples run on both backends in this way, which meets the exit criterion of M10 under the owner's decisions 4 and 5 of Section 4.6:

- The examples package turns the default features of `slango` off and gains its own features: `wgpu`, its default, which enables `slango/wgpu` and the optional dependencies `slang-slint`, `slint`, `wgpu`, and `winit`, and `rhi`, which enables `slango/rhi`. Its feature `vendored` forwards to `slang-slint` only when that dependency is on. A build of the examples package with both `wgpu` and `rhi` stops with the error of SLO-29.
- The binaries `windowed-renderer` and `shader-playground` declare `required-features = ["wgpu"]`, so a build with `rhi` leaves them out, and their windows stay on wgpu.
- `compute-kernel` and `typed-kernel` build with either feature and run on the build's backend. They take no new option, and their `gpu` tests run them as before.
- The scenes of `windowed-renderer` and `shader-playground` draw offscreen on the build's backend in their `gpu` tests (SLO-43). The scene of `shader-playground` draws into a `slang_slint::View` with the feature `wgpu`, and into a `slango::Texture` that it creates and resizes itself under the `cfg` of the feature `rhi`, because `slang-slint` cannot be built with `rhi`. Both offer the texture of the scene to the tests, which compare its pixels.
- The tests that compile the modules `format.rs` and `retry.rs` of `windowed-renderer`, which name wgpu or winit, and the assertions that call `slango::wgpu`, compile only with the feature `wgpu`.
- The examples test binary gains a module of `gpu` tests that call `slango_macros::shader!` on the test shaders of `slango-macros`, including a shader with two levels of `import` and the structures with padding, so that the generated code runs on the backend `rhi` (Section 10 of `docs/design/slango-macros.md`).
- `compile-and-reflect` creates no device, so it runs unchanged on either build.

The tests of the walk on the layouts of the backend `rhi` are `ffi` unit tests in `layout::tests::ffi` with `insta` snapshots. They compile the test shaders of Section 13.1 in a session of `slang-compiler` with the SPIR-V target and the scalar layout, and in one with the DXIL target, and walk them. The walk reads the metadata of each entry point, which generates code (Section 3.11), so the DXIL cases need `dxcompiler`:

- The cases of the scalar layout of SPIR-V run in the default tier on every platform, because they need no slang-rhi and no `dxcompiler`.
- The DXIL cases compile only for Linux in a build with the feature `wgpu`. There the recipes put `$VULKAN_SDK/lib`, which holds `libdxcompiler.so`, on the library path, so they run in the default tier of `just check` and of the CI job on Linux. Whether the Vulkan SDKs of Windows and macOS put `dxcompiler` where a test finds it is not verified, so the DXIL cases do not compile there. They do not compile in a build with `rhi` either, because `test-rhi` runs without the SDK, and the vendored Slang of Linux has no DXIL support (Section 3.11).

These tests check SOA-10 for the layouts, and the `gpu` tests on WARP check it for D3D12.

The pure tests of the marker of SLO-17 are `compile_fail` doctests, one for `Send` and one for `Sync` of each type, as `slang-rhi` has them (RHI-13 of `docs/design/slang-rhi.md`). The `gpu` tests that dispatch clones of one kernel and draw with clones of one pipeline from several threads are removed, because the types are no longer `Send`. These are the tests in `gpu/call.rs` and `gpu/render.rs` that use several threads, and the module `gpu/render/sharing.rs`.

## 14. Assumptions to Confirm

Each row is a statement that this design relies on and that no test or source establishes. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in Section 3 and in the section that relies on it, and removes the row.

An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SOA-6 | A system value other than `SV_Position`, `SV_VulkanVertexID`, and `SV_VulkanInstanceID` lacks the category `VaryingInput`. An index past the end of a vertex buffer reaches only the backend's own handling | Sections 7.4 and 16 | For another system value, the change that first uses it. For an index past the end, deferred with no change assigned |
| SOA-9 | On the backend `rhi`, the conventions of a draw of Section 3.9 hold on Vulkan and D3D12, apart from the depth clamping of Section 3.11, through the negative viewport height of the Vulkan backend of slang-rhi | Section 9.7 | The `gpu` tests of the conventions on the backend `rhi` (Section 13.7), in the code step that adds rendering to that backend |
| SOA-10 | For a program linked in the session of a load on the backend `rhi`, the walk of Section 7.1 records the same paths, kinds, and usage as for the session of the backend `wgpu` when the usage reads the category of each resource on DXIL, and the offset and size of each top-level field of a shader object equal those of the walk | Sections 7.6 and 9.6 | The `ffi` tests of the walk on the SPIR-V target with the scalar layout and on the DXIL target, and the `gpu` tests of every kind of parameter on the backend `rhi` (Section 13.7) |
| SOA-11 | `slang-compiler` either offers the reflected row stride of a matrix in its type layout already, or can gain an accessor for it as an additive change | SLO-46 and Section 7.1 | The code step that adds SLO-46 |

SOA-7 and SOA-8 are retired. The owner's decision 6 of Section 4.6 removed the loads in one device session that SOA-7 concerned, and a reflection of DXIL refuted SOA-8, which SLO-46 replaces (Section 3.11).

Of the assumptions of the architecture design that name this document, Sections 3.3 to 3.5 confirm A-12, Section 3.7 confirms A-13, and Section 3.2 confirms A-11 for M3. M4 does not need A-11, because it keeps the validated path and defers other shader targets (Section 1.5).

Section 3.7 confirms A-10 in part, for the first three patterns of Section 8.3 of that design and the description of `GpuFuture`, with "replacing" read as "building an experimental replacement". A-10 stays for typed shader interfaces generated from reflection. A-9 holds as far as SlangPy's documentation states (Section 3.7), and `slango` does not depend on it.

## 15. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it in the milestone of the last column, and the change that answers it removes the row.

The owner's decision 1 of Section 4.6 answers SOQ-4: the backend `rhi` cannot keep `Send` and `Sync` of the types that hold GPU objects, so version 0.2 removes them on both backends (SLO-17), while `from_device_and_queue` stays infallible and `Device::new` serves the backend of the build (SLO-29).

The owner's decision 5 of Section 4.6 answers SOQ-10, on the layout of structures on the backend `rhi`, whose row is removed: a build has one backend, so a structure has the one layout of that backend, a generated struct has the encoding of that layout, and a mismatch fails the check of its element (Section 8.3).

The design of M10 answers SOQ-9, as far as the backend `rhi` needs it, by these choices:

- The conventions of a draw of Section 3.9 become conventions of `slango` that every backend keeps, because a shader written for one backend then draws the same pixels on the other, and the `gpu` tests check them on both (Section 9.7). The depth clamping of slang-rhi on Vulkan is the one stated difference.
- The statements about naga, the validated path, the immediates of wgpu, and the adapters of wgpu become statements about the backend `wgpu`, because the backend `rhi` has none of them (Section 6.4).
- The statement that every type is `Send` and `Sync` follows SLO-17.

SOQ-5 to SOQ-8 come from the review of the public API before the first release (Section 9.5 of `docs/design/tooling.md`). Each is a finding that waits for the decision of the owner of the project. The owner let the first release, an alpha, ship without deciding them, and each is decided before the first release that is not an alpha (TOOL-37 of `docs/design/tooling.md`).

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SOQ-3 | How large must a kernel be before naga's parsing at load or lavapipe's first dispatch takes long enough to matter? | This document | The milestone that first loads kernels larger than those of the example |
| SOQ-5 | `RenderPipelineOptions::colour_format` and `depth_format` take any `TextureFormat`, so a depth format as the colour format and a colour format as the depth format compile and fail only in `render_pipeline` with `InvalidArgument` (Section 7.5), against TYPE-3. The options are to add `#[non_exhaustive]` enumerations, such as `DepthFormat` with `Depth32Float` and `ColourFormat` with the four colour formats, each with a lossless conversion into `TextureFormat`, before the release, to narrow only `depth_format`, or to keep the run-time check and record why | The owner of the project, by a decision | Before the first release that is not an alpha |
| SOQ-6 | The setters `set`, `bind`, `texture`, and `sampler` of `Call`, and those and `vertex` of `Draw`, take `path: &'a str` and store it for the life of the builder, so a path built at run time must outlive the builder (Section 9.1), against API-13. The options are to take `impl Into<Cow<'a, str>>` before the release, which keeps every literal free, or to keep the borrow and record the zero-copy choice as an exception to API-13 | The owner of the project, by a decision | Before the first release that is not an alpha |
| SOQ-7 | `Buffer::zeroed` and `Texture::new` both create a resource whose bytes are zero, under two names, against API-3 (Section 8). The options are to rename `Texture::new` to `Texture::zeroed`, to rename `Buffer::zeroed` to `Buffer::new`, or to keep both names and record why | The owner of the project, by a decision | Before the first release that is not an alpha |
| SOQ-8 | `Device` has an identity that its clones share and that the crate compares for every check of another device, but implements no `PartialEq`, `Eq`, or `Hash`, which API-7 asks for (Section 9.3). The options are to implement them by hand through the identity before the release, documenting that two `Device` values built separately from one wgpu device are unequal, or to defer them, which is additive | The owner of the project, by a decision | Before the first release that is not an alpha |

Of the questions of the architecture design that name this document, these sections answer or defer each:

- Section 17.5 answers Q-16 for `slango` through M4.
- Section 6 answers Q-17.
- Section 9.1 answers Q-18 for M3.
- SLO-2 answers Q-19 for `slango` through M4.
- SLO-1 answers Q-20.
- Section 13.2 answers Q-21.
- Section 10 answers Q-22.
- Section 1.5 defers Q-23 with the configurable shader target.
- Section 13.4 answers Q-24 for `slango`.
- Section 7.1 answers Q-31 and makes Q-30 unnecessary for M3.

## 16. Security Considerations

The threat model treats the shader source, the file paths, and the options that a program passes as trusted input from the program itself, as Hard Rule 14 of `AGENTS.md` states. It covers mistakes in the arguments of a call, which Section 9.3 checks.

`slango` has no unsafe code (SLO-2) and no build script. Its memory safety rests on the safe APIs of wgpu and `slang-compiler`.

The safety of the GPU's memory accesses rests on naga's validation and on the bounds checks that wgpu promises on the validated path, which it injects where the platform cannot enforce them itself (Section 3.2). Slang is linked as `docs/design/slang-compiler-sys.md` specifies, and `Cargo.lock` and `cargo-deny` fix and check the versions of wgpu's dependency tree (Section 4.2).

These attacks are out of scope, because a program that loads them has already chosen to run them: a malicious shader, a shader that reaches a defect of Slang, naga, wgpu, or a driver, and a kernel that never finishes, which blocks `Buffer::read`.

Rendering adds textures, samplers, and vertex and index buffers, whose sizes and formats the draw checks (SLO-26). The draw does not read the values of an index buffer, so an index past the end of a vertex buffer reaches the backend, and what a vertex fetch then reads rests on wgpu and the driver (SOA-6).

On the backend `rhi`, no naga validation and no bounds checks of wgpu run, and slang-rhi checks nothing unless its debug device is on (Section 3 of `docs/design/slang-rhi.md`). `slango` keeps every check of SLO-14 and SLO-26 on that backend, and `slang-rhi` checks every condition of valid use of the graphics API that it can check before each native call (RHI-20 of that design). The memory accesses of a shader then rest on the driver, so a kernel that indexes past the end of a buffer on the backend `rhi` reads or writes what the driver lets it, which the shader's author chose to run.

The residual risk is a defect of naga, wgpu, slang-rhi, or a GPU driver that lets a kernel or a draw read or write outside its buffers and textures, and on the backend `rhi` a shader that indexes out of bounds. A crash that a defect of Slang causes is also residual, and Section 3.12 of `docs/design/slang-compiler.md` records each such defect.

## 17. Operational Considerations

### 17.1. Cargo Features and Environment Variables

`slango` has three Cargo features (SLO-27). `wgpu`, on by default, builds the backend `wgpu`, and `rhi`, off by default, builds the backend `rhi` and implies `vendored`. Exactly one of the two must be on, or the crate does not compile (SLO-28, SLO-29). `vendored`, off by default, only forwards to `slang-compiler`. Section 4.7 states how the features combine. The crate reads no environment variable itself. Without the feature, its build needs `VULKAN_SDK` and libclang, and its programs need the Slang shared library on the library path. With it, its build clones and builds Slang through `slang-src` and its programs need no Slang library (Section 4.1 of the architecture design, Section 18.4 of `docs/design/slang-compiler-sys.md`).

### 17.2. Platforms and Minimum Supported Rust Version

The crate runs on Linux with Vulkan, on a software and on a hardware adapter. On Windows with Direct3D 12 and on macOS with Metal, it creates a device, loads modules, dispatches kernels, samples textures in them, and draws offscreen in CI, with the same conventions of a draw on all three (Sections 3.8 and 3.9).

The backend `rhi` builds and runs on Linux with the GNU C library with Vulkan and on x86_64 Windows with D3D12, where a program needs `dxcompiler.dll` at run time, and on no other platform (Section 3.11). CI runs its `gpu` tests on lavapipe and on WARP (Section 13.7).

wgpu 30.0.1 declares `rust-version` 1.87, below the workspace's minimum supported Rust version (MSRV) of 1.99.

### 17.3. Failure Modes

| Failure | Reported as | Recovery |
| --- | --- | --- |
| `slango` is built with neither the feature `wgpu` nor `rhi` | The compile error of SLO-29 | Keep the default features, or turn them off and enable `rhi` |
| `slango` is built with both features, such as when two crates of one build graph ask for different backends | The compile error of SLO-29 | Make every crate of the graph ask for the same backend, and turn the default features off wherever `rhi` is wanted |
| `slango` is built with the feature `rhi` on a target other than Linux with the GNU C library or x86_64 Windows | The error of the build script of `slang-rhi-sys` (RSYS-13 of `docs/design/slang-rhi-sys.md`) | Use the backend `wgpu` on that target |
| A load or a pipeline on the backend `rhi` on Windows finds no `dxcompiler.dll` | `Compile`, with the diagnostics of Slang (SLO-42) | Put the directory of `dxcompiler.dll` of the Windows SDK on `PATH` |
| A kernel with a `ConstantBuffer` and a global `uniform` on the backend `rhi` on D3D12 | `Backend` at load (RKB-2 of `docs/design/slang-rhi.md`) | Use a `ParameterBlock`, or no global `uniform` beside the constant buffer |
| A buffer of a generated struct whose layout differs from the reflected structure | `InvalidArgument` at the call, which names the stride or the field (Section 8.3) | Rebuild with matching versions of `slango-macros` and `slango` |
| A vertex attribute without a semantic on the backend `rhi` | `UnsupportedParameter` from `render_pipeline` (Section 7.7) | Give every vertex attribute a semantic |
| A shader reads `SV_VulkanVertexID` on the backend `rhi` on D3D12 | `Compile` (Section 6.4) | Take the value from a vertex buffer |
| No adapter exists, such as on a machine without a GPU driver or software adapter | `NoAdapter` | Install a driver or lavapipe |
| The Slang shared library is not on the library path | The program stops before `main`, as `slang-compiler-sys` describes | Run through the recipes, or set the library path |
| The module does not compile | `Compile`, with Slang's diagnostics | Fix the shader |
| naga or the backend rejects a kernel | `UnsupportedShader`, with the backend's text as the source error (Section 10) | Avoid the construct (Section 3.5) |
| A kernel has a parameter that the walk rejects (Section 7.1) | `UnsupportedParameter` | Wait for the milestone that supports it |
| A kernel needs a feature the device lacks | `MissingFeature` | Create the wgpu device with the feature and pass it to `slango::wgpu::from_device_and_queue` (Section 12), or use another adapter |
| A call has a wrong name, value, buffer, texture, or sampler, or more workgroups than the device allows (Section 9.3) | `InvalidArgument`, naming the parameter | Fix the call |
| A call or a draw binds one buffer to a read-only and to a read-write parameter | `InvalidArgument`, naming both parameters (SLO-44) | Bind separate buffers |
| A buffer would be larger than the device allows | `InvalidArgument` | Create a smaller buffer |
| A render entry point has a parameter that the walk rejects, or `uniform` parameters of its own (Sections 7.4 and 7.5) | `UnsupportedParameter` | Use global uniforms, constant buffers, or parameter blocks, or wait for the milestone that supports the parameter |
| naga or the backend rejects a render pipeline, such as for `SV_VertexID` or varyings in another order | `UnsupportedShader`, with the backend's text as the source error (Section 10) | Avoid the construct (Section 3.9), or declare the varyings in one order |
| A draw has a wrong name, value, buffer, texture, target, or count (SLO-26) | `InvalidArgument`, naming the entry points and the parameter or target | Fix the draw |
| A draw samples the texture that it draws into | `Backend`, with wgpu's text as the source error | Draw into another texture |
| wgpu reports a validation or out-of-memory error (Section 10), or an error while a buffer, texture, or sampler is created or read | `Backend` | Report a validation error as a defect of `slango` |

### 17.4. Costs

Each load of a module pays for its own global session, well over 100 ms, plus about 10 ms for a small module (Sections 3.6 and 7.2). A program that reloads a shader therefore waits on the order of 150 ms for each reload.

On the backend `rhi`, a device creates its global session once, and each load costs a new session and the compile, code generation, and reflection of the module without a new global session. The cost of a load on the backend `rhi` has not been measured. Each load keeps its session until its module, kernels, and render pipelines are dropped (Section 7.6).

### 17.5. Logs

`slango` emits no logs in M3 and M4 and has no logging dependency, as `slang-compiler` does. This answers Q-16 of the architecture design for `slango` through M4. The backend `rhi` emits none either, and the messages of slang-rhi other than the diagnostics of Slang are dropped (Section 6.2).

### 17.6. Migration to Version 0.2

This section is non-normative. It lists the changes of M10 that break a program written for version 0.1, so that the crate documentation and the notes of the release that M11 publishes can state them. The version number that carries them is chosen by the release procedure of M11 (TOOL-37 of `docs/design/tooling.md`).

| Change | What a program does |
| --- | --- |
| `Device`, `Module`, `Kernel`, `Buffer<T>`, `Texture`, `Sampler`, `RenderPipeline`, `Call`, and `Draw` are neither `Send` nor `Sync` (SLO-17) | Keeps each device and everything created from it on one thread, and moves results between threads as plain data |
| `ErrorKind` has the kind `Unsupported` (SLO-42) | Nothing, when it matches with a wildcard arm, as `#[non_exhaustive]` requires |
| One buffer bound to a read-only and to a read-write parameter gives `InvalidArgument` instead of `Backend` (SLO-44) | Matches the new kind, if it matched `Backend` for this case |
| A program that sets `default-features = false` and names neither `wgpu` nor `rhi`, or a build graph that enables both | Names exactly one of `wgpu` and `rhi` (SLO-29) |
| The generated code of `slango-macros` loads through `load_embedded`, carries the loading path and the struct layouts of the backend of the program's `slango` only, and embeds the Slang source on the backend `rhi` (SLM-24 to SLM-26 of `docs/design/slango-macros.md`) | Rebuilds with the matching versions of `slango-macros` and `slango`, which SLM-2 requires anyway. `Shader` and the kernel structs are no longer `Send` or `Sync` |
| `slang_slint::View` is no longer `Send` or `Sync` (SLS-14 of `docs/design/slang-slint.md`) | Keeps the view on the thread of the user interface, where Slint already requires its image |

A program that only uses `slango` on one thread with its default features needs no change of its code.

## 18. References

### 18.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 18.2. Informative References

- [SLANG-SPIRV] The Slang project, "SPIR-V specific functionalities", `docs/user-guide/a2-01-spirv-target-specific.md` in Slang's source tree at the recorded commit, read on 2026-11-11.
- [SLANGPY] The Slang project, "SlangPy documentation", <https://slangpy.shader-slang.org/en/latest/>, read on 2026-11-11.
- [VULKANO-2540] Pull request 2540 of the vulkano-rs/vulkano repository on GitHub, <https://github.com/vulkano-rs/vulkano/pull/2540>, read on 2026-11-11.
- [WGPU-CHANGELOG] The wgpu project, "Changelog", <https://raw.githubusercontent.com/gfx-rs/wgpu/trunk/CHANGELOG.md>, read on 2026-11-11.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary. SLO-35, a choice of backend for each device that the owner's decision 5 of Section 4.6 replaced, is retired, and its identifier is not reused.

| Label | Section | Summary |
| --- | --- | --- |
| SLO-1 | 4.1 | Depend on `slang-compiler`, under the feature `wgpu` on wgpu 30 with `spirv` and `pollster`, under the feature `rhi` on `slang-rhi`, and on no other crate. |
| SLO-2 | 4.1 | No unsafe code, and every shader of the backend `wgpu` through the validated path. |
| SLO-3 | 5.1 | `Device`, `Module`, `Kernel`, and `Buffer<T>`, cheap to clone, holding their parents, created through options. |
| SLO-4 | 6 | wgpu types appear only in the public module `slango::wgpu`, which exists only with the feature `wgpu`. |
| SLO-5 | 7.1 | `load_module` compiles on the calling thread, and on the backend `wgpu` releases every Slang object before it returns. |
| SLO-6 | 7.1 | The private layout snapshot holds only owned data and follows the walk rules of Section 3.3. |
| SLO-7 | 7.2 | A new global session for each load on the backend `wgpu`, and no hidden Slang state. |
| SLO-8 | 7.3 | Every shader module, layout, and pipeline is created at load, and any failure of a kernel fails the load. |
| SLO-9 | 7.3 | A naga rejection is caught in an error scope and returned as `UnsupportedShader` with naga's text. |
| SLO-10 | 8 | The element types of a buffer bound to a kernel come from a sealed trait, each with a test of its stride. |
| SLO-11 | 8 | `Buffer::read` blocks until earlier dispatches finish and returns the elements. |
| SLO-12 | 9.2 | The value trait is sealed and checks each value against its reflected type before writing. |
| SLO-13 | 9.2 | Plain values are written at reflected offsets, never as Rust struct bytes. |
| SLO-14 | 9.3 | Names, presence, values, the strides and element types of buffers, devices, and workgroup limits are checked before dispatch. |
| SLO-15 | 9.3 | Remaining wgpu validation errors are caught in an error scope and returned as `Backend`. |
| SLO-16 | 10 | One `Error` type with the nine kinds of Section 10. |
| SLO-17 | 11 | The types that hold GPU objects, `Call`, and `Draw` are neither `Send` nor `Sync` on every backend through a marker field, the other public types are both, and each trait is asserted. |
| SLO-18 | 12 | On the backend `wgpu`, request immediates when offered, and fail the load with `MissingFeature` when a kernel needs them. |
| SLO-19 | 5.3 | `RenderPipeline`, `Texture`, and `Sampler`, cheap to clone, holding their device, with options for pipelines and samplers. |
| SLO-20 | 7.4 | The load records vertex and fragment entry points without creating their shader modules or failing on them. |
| SLO-21 | 7.4 | The walk records each vertex attribute with its path, location, type, and usage. |
| SLO-22 | 7.5 | A render pipeline is created inside error scopes and fails as a whole. |
| SLO-23 | 7.5 | `uniform` parameters of a render entry point give `UnsupportedParameter`. |
| SLO-24 | 8.1 | `Texture2D` with a `float` result and `SamplerState` are parameters of every stage, and `Sampler2D` is rejected. |
| SLO-25 | 8.2 | The sealed trait `VertexElement`, a supertrait of `Element`, adds packed vectors of 2 and 3 components. |
| SLO-26 | 9.5 | A draw checks its parameters, attributes, targets, and counts before any work and reports `InvalidArgument`. |
| SLO-27 | 4.1 | Three Cargo features, `wgpu`, `rhi`, and `vendored`, of which the last two are off by default and `vendored` only forwards to `slang-compiler`. |
| SLO-28 | 4.3 | The default feature `wgpu` enables wgpu and `pollster`, exactly one backend feature gates the declarations of the crate root, and `wgpu` alone gates the module `slango::wgpu`. |
| SLO-29 | 4.3 | With neither `wgpu` nor `rhi`, or with both, the crate stops compiling with one `compile_error!` that names both features. |
| SLO-30 | 4.3 | Every member of the workspace that depends on `slango` names the backend feature that it builds with. |
| SLO-31 | 6 | `slango::wgpu::device`, `queue`, and `texture` return an `Option`, under an exception to `clippy::unnecessary_wraps`. |
| SLO-32 | 5.5 | Public enumerations and structs with only public fields are `#[non_exhaustive]`, except those of `__private`, which carry a lint exception. |
| SLO-33 | 4.7 | The feature `rhi` enables the optional `slang-rhi`, by path and exact version, and `vendored`, and the crate does not check the target. |
| SLO-34 | 6.1 | Each backend lives in one private module under the `cfg` of its feature, the public types hold the objects of the build's backend, and no trait or enumeration over backends exists. |
| SLO-36 | 6.3 | No public module `slango::rhi` or other item of `rhi` alone, and `slango::wgpu` keeps its functions and lint exceptions unchanged. |
| SLO-37 | 7.6 | A device of the backend `rhi` owns its global session, and each load creates its own session like the device session, walks its program, and keeps its Slang objects. |
| SLO-38 | 7.6 | The load on the backend `rhi` creates a shader program and a compute pipeline for each compute entry point and fails as a whole. |
| SLO-39 | 9.6 | A dispatch on the backend `rhi` runs the checks of SLO-14 first and writes the byte images and arguments into the shader objects by parameter path. |
| SLO-40 | 8.3 | A buffer of the backend `rhi` is one buffer of `slang-rhi` with every usage of a call and a draw, none for length zero, and `read` waits on the host. |
| SLO-41 | 7.7 | A render pipeline of the backend `rhi` runs the checks of Section 7.5 and of its semantics and streams first, has the fixed-function state and options of `slango`, and fails as a whole. |
| SLO-42 | 10 | The kind `Unsupported`, and one mapping of the errors of `slang-rhi` to the kinds of `slango`. |
| SLO-43 | 13.7 | Each `gpu` test runs on the build's backend through one helper, names the backend in a failure, and skips only under the `cfg` of `rhi` with a reason. |
| SLO-44 | 9.3 | One buffer bound to a read-only and to a read-write parameter gives `InvalidArgument` on both backends. |
| SLO-45 | 8.3 | On the backend `rhi`, `Texture::new` creates a colour texture from zero bytes and clears a depth texture to 0. |
| SLO-46 | 7.1 | The walk records the reflected row stride of each matrix, and marshalling writes each row at it on both backends. |
