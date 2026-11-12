# `slango` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-12 |
| Applies to | `crates/slango`, the compute kernel example, and the windowed renderer example |

## Abstract

This document specifies `slango`, the high-level library for running Slang code on a GPU from Rust, as milestones M3 and M4 define it, with the compute kernel example and the windowed renderer example. `slango` calls the existing compute entry points of a Slang module on wgpu, with bindings built from Slang's reflection alone, and draws with existing vertex and fragment entry points into a texture. The design rests on the facts about Slang's SPIR-V on wgpu's validated shader path, loads a module into an owned layout snapshot that frees every Slang object, and uses no unsafe code.

## 1. Introduction

### 1.1. Problem

`slang-compiler` turns Slang source into SPIR-V and reports the bindings of each parameter, but running that code on a GPU takes much more. A program must open a device, build bind group layouts and a pipeline that match the shader exactly, lay out every plain value by the shader's rules, bind buffers, choose a dispatch size, and copy results back. A mistake in any of these steps fails at run time with a message about descriptor sets and offsets, or computes wrong numbers without any error.

### 1.2. Purpose

`slango` lets a Rust program load a Slang module, look up a compute kernel, set its parameters by name, dispatch it over a number of elements, and read the results, with every binding derived from reflection. For M4, it also lets the program draw with a vertex and a fragment entry point into a texture, with every binding and vertex attribute derived from reflection.

This document implements Section 8 and milestones M3 and M4 (Sections 15.6 and 15.7) of `docs/design/architecture.md`, and meets ARCH-15 to ARCH-19. It answers the questions and assumptions of that design that name this document, as Sections 14 and 15 record.

### 1.3. Audience

The primary readers are the coding agents that change `slango` and the reviewers who check their changes. A reader is expected to know Rust but not GPU programming. Section 2.1 defines the terms that such a reader needs.

### 1.4. Scope

This document covers the compute entry points that a Slang module already declares with `[shader("compute")]`, compiled to SPIR-V and loaded through wgpu's validated path. Their parameters are buffers, constant buffers, parameter blocks, and plain `uniform` values. It covers the public types, the backend boundary, loading, the call, errors, threading, device features, tests, continuous integration (CI), the example, and the dependency changes.

For M4, it also covers render pipelines built from the vertex and fragment entry points that a module already declares with `[shader("vertex")]` and `[shader("fragment")]`, drawing into a texture. Vertex attributes, uniforms, textures, and samplers are bound by name from reflection, as M3 binds the parameters of a kernel. Textures and samplers become parameters of kernels too.

It covers the windowed renderer example as well. The rendering design is in Sections 3.9, 5.3, 5.4, 7.4, 7.5, 8.1, 8.2, 9.5, 13.4, and 13.5, and it extends the other sections where they name M4.

### 1.5. Non-Goals

This document designs nothing beyond M3 and M4. These belong to later milestones: kernels generated around a plain Slang function with a positional call, broadcasting over array arguments, a tensor type with a shape, differentiation driven from Rust, and the unchecked shader passthrough path.

M4 leaves out these features:

- Generated shaders.
- Stages other than vertex, fragment, and compute, such as geometry, tessellation, and mesh stages.
- Multisampling, storage textures, texture arrays, cube maps, and mip-map generation.
- Several draws in one render pass, and render bundles.

`slango` offers no window, no surface configuration, and no frame loop, which belong to the caller (Section 5.3). Hot reload is no API of `slango`, and the windowed renderer example does it with the existing load (Section 13.5).

A shader target configurable per backend is deferred with no milestone assigned. Slang's SPIR-V loads and runs through wgpu's validated path on Vulkan, Direct3D 12, and Metal (Section 3.8), and a second target would need the passthrough path and unsafe code (SLO-2).

### 1.6. Defects of Third-Party Components and Trusted Input

`slango` follows Hard Rule 14 of `AGENTS.md` and Section 1.6 of `docs/design/slang-compiler.md`, and applies the same stance to naga and wgpu. Shader source and file paths are trusted input. When naga rejects a module, `slango` reports naga's own text and does not screen modules for the constructs of Sections 3.5 and 3.9 in advance.

The checks of the arguments of a call stay, because a safe interface needs them (Section 9.3). A defect of Slang that `slango` finds is recorded in Section 3.12 of `docs/design/slang-compiler.md`, not here.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

Section 2.1 of `docs/design/architecture.md` defines adapter, buffer, compute dispatch, compute kernel, device, lavapipe, marshalling, shader passthrough, software adapter, SPIR-V, and wgpu. Section 2.1 of `docs/design/slang-compiler.md` defines descriptor set, layout, parameter category, thread group, type layout, and variable layout. This document adds these terms:

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

## 3. Facts the Design Relies On

This section is non-normative. It states each fact about Slang, naga, wgpu, and the platforms that a later section relies on, once, with its consequence for the design.

### 3.1. Sources and Evidence

Each fact holds for wgpu 30.0.1 with the feature `spirv`, naga 30.0.1, and the pinned Slang, compiled with the session settings of Section 7.1. A fact is known in one of three ways:

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

The extent is the reflected size of the value, or the array stride for a matrix inside an array. It is R times 16 bytes in a uniform buffer and R times 8 or 16 bytes in the immediates, for every R and C from 2 to 4.

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

## 4. Crate Structure and Dependencies

### 4.1. Package and Dependencies

SLO-1. `slango` MUST depend on `slang-compiler` and on wgpu of major version 30 with the feature `spirv`, and on no other crate except `pollster`, which blocks on wgpu's futures. The list stays short because every dependency runs on every user's machine (DEP-2).

`slango` needs `pollster`, because wgpu 30 offers no blocking form of `Instance::request_adapter`, `Instance::enumerate_adapters`, or `Adapter::request_device`, so `slango` waits on them with `pollster::block_on`. Version 30 is the newest wgpu that the current Slint release supports (ARCH-19, Section 3.7).

The implementation re-checks the version at each Slint release, and an upgrade is a breaking release of `slango` (DEP-3). The crate has no Cargo features in M3 and M4.

SLO-2. `slango` MUST contain no unsafe code in M3 and M4, and MUST load every shader through the validated path only.

The validated path runs every kernel that M3 needs and every render shader that M4 needs without unsafe code (Sections 3.2 and 3.9), so passthrough would add unsafe code for no need. This answers Q-19 of the architecture design for `slango` through M4.

### 4.2. Licences and Duplicate Versions

wgpu's dependency tree brings crates under the licences `MIT`, `Unicode-3.0`, and `Zlib`, and duplicate versions of `foldhash`, `hashbrown`, and `syn`. The licence allow-list of `deny.toml` holds the three licences, and its `[bans]` table holds a `skip` entry with a reason for each duplicate version, as TOOL-24 and TOOL-26 require.

The project's licence stays GPL-3.0-or-later. Section 8 of `docs/design/tooling.md` records which crates need each licence, the evidence of its compatibility, and the reason for each skip.

## 5. Public Types and Ownership

### 5.1. Types

SLO-3. `slango` MUST provide the public types `Device`, `Module`, `Kernel`, and `Buffer<T>`, each of which holds its parent by value and is cheap to clone. It MUST create `Device` and the call through structs of options that implement `Default`.

A value that holds its parent can be stored without a lifetime (OWN-3), and options can grow without a breaking change (API-6).

- `Device`: A wgpu device and its queue. It is created from options or, in the module of Section 6, from wgpu objects that the caller already holds.
- `Module`: One loaded Slang module. It holds its `Device` and one `Kernel` for each compute entry point.
- `Kernel`: One compute entry point, with its layout snapshot, its bind group layouts, and its compute pipeline. It holds its `Device`.
- `Buffer<T>`: A storage buffer of elements of type `T`, with its length. It holds its `Device`.

`Device::new` searches the primary backends of wgpu, which are Vulkan, Metal, Direct3D 12, and WebGPU in a browser, and never OpenGL. The field `adapter` of `DeviceOptions` selects the adapter with an `AdapterSelection`:

- `Default`, the default: The first adapter that wgpu finds. `Device::new` sets no power preference, so wgpu does not prefer a hardware GPU.
- `Software`: wgpu's fallback adapter, which is a software adapter such as lavapipe or WARP. Metal has none, so on macOS this selection returns `ErrorKind::NoAdapter` unless a Vulkan driver provides one (Section 3.8).
- `Name`: The first adapter whose name contains the given text, which also makes `ErrorKind::NoAdapter` testable.

`DeviceOptions` and `AdapterSelection` are non-exhaustive, so both can grow without a breaking change. `Device::adapter_name` returns the name of the adapter.

wgpu's types are already reference counted clones (Section 3.2), so `slango` wraps no wgpu object in an `Arc` of its own. It shares its own owned data, such as a layout snapshot, through `Arc`. This follows the vulkano pattern of children that hold their parents (Section 3.7) and adopts nothing of `GpuFuture` (ARCH-16): every wait is a blocking call (CONC-6).

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

## 6. Backend Boundary

SLO-4. Every public item of `slango` that names a wgpu type MUST be in one public module, such as `slango::wgpu`, which holds the constructors from wgpu objects and the accessors that return them. Every other use of wgpu MUST stay private, because a later backend then changes no other part of the public API (ARCH-15).

The layout snapshot is the boundary inside the crate. The reflection walk above it knows Slang and no wgpu type, and the code below it turns the snapshot into wgpu layouts, bind groups, and dispatches.

No backend trait exists (SIMP-2). This answers Q-17 of the architecture design.

## 7. Loading a Module

### 7.1. Compile and Snapshot

SLO-5. `Device::load_module` MUST compile the module on the calling thread and copy into a layout snapshot the facts that the kernels need. It MUST release every Slang object before it returns, because no Slang object may then outlive the call or cross a thread (Section 11).

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

`Device::load_module` and `Device::load_module_from_source` compile and then load. `slango::__private::precompile_file` compiles a file with the same code as `Device::load_module`, and `slango::__private::load_precompiled` loads a snapshot that it decodes (Section 5.2 of `docs/design/slango-macros.md`).

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

Each plain value has its parameter path, offset, size, scalar type, and shape. A shape is a scalar, a vector with its length, a matrix with its rows and columns, or an array with its length, stride, and element shape. The walk flattens a structure into one plain value for each leaf, with a dotted path such as `params.scale`.

A constant buffer or parameter block without plain values gets no uniform buffer.

The walk names parameters and records their usage by these rules:

- A field of a named `cbuffer` gets the path `Name.field`, such as `Arrays.c` for `cbuffer Arrays { float c[5]; }`, although the shader reads it as `c`.
- A parameter of the entry point has a path without a prefix, so `uniform float factor` has the path `factor`, as a global has.
- The unnamed uniform buffer of the global plain values has the empty path.
- The usage of a binding comes from the metadata of the entry point, through `Metadata::is_parameter_location_used` with its set and binding. A plain value has the usage of the uniform buffer that holds it. The immediates carry no usage flag, so a call treats them as used.
- The walk does not record the matrix layout mode. Marshalling needs none, because the session keeps Slang's default row-major layout, whose rows the extent of a matrix places (Section 3.4).

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

SLO-7. `Device::load_module` MUST create its own global session and release it before it returns. `slango` MUST keep no global session in a `static`, a `thread_local!`, or any hidden state, because CONC-4 forbids hidden library state.

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

A failure that belongs to one such entry point becomes the rejection on its record, and the load goes on (SLO-20). The rejection is `ErrorKind::UnsupportedParameter` for a parameter that the walk refuses. It is `ErrorKind::Compile` with Slang's text for a failure of Slang to reflect the entry point, report its metadata, or generate its SPIR-V.

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
- A texture of a colour format has the usages `TEXTURE_BINDING`, `RENDER_ATTACHMENT`, `COPY_SRC`, and `COPY_DST`. A `Depth32Float` texture has `RENDER_ATTACHMENT` only, so it serves as a depth target and is neither sampled nor read.
- A width or height of zero, a size over the device's limit, or a number of pixels other than the width times the height gives `ErrorKind::InvalidArgument`.
- Every texture has one mip level and one sample.
- `Texture::size` and `Texture::format` return the size and the format.
- `Texture::read` returns the pixels of a colour texture as a `Vec<[u8; 4]>` in rows from the top, in the byte order of its format, and blocks as `Buffer::read` does (SLO-11). It gives `ErrorKind::InvalidArgument` for a `Depth32Float` texture, even a wrapped one with `COPY_SRC`, and for a texture without `COPY_SRC`.

These rules settle samplers. `Sampler::new` takes a device and a `SamplerOptions`, whose fields are these:

- `filter`: A `FilterMode`, `Nearest` by default or `Linear`, for magnification and minification.
- `address_mode`: An `AddressMode`, `ClampToEdge` by default, `Repeat`, or `MirrorRepeat`, for every axis.

The other settings of a sampler, such as its mipmap filter, are wgpu's defaults, with no comparison and no anisotropy.

The module `slango::wgpu` has `texture`, which returns the wgpu texture of a `Texture`, and `texture_from_view`, which wraps a wgpu texture view of a device, such as the view of a surface texture, as a `Texture` (SLO-4). No function returns the view of a `Texture`. `texture_from_view` takes the size, format, and usages from the viewed texture through `TextureView::texture` (read, wgpu 30.0.1).

`texture_from_view` gives `ErrorKind::InvalidArgument` for a format that `TextureFormat` lacks. It does the same for a texture that is not two-dimensional with one sample, one mip level, and one array layer, so that `size` and `read` describe what a kernel samples.

It does not check which device the view belongs to. A call, a draw, or a read that needs a usage that such a texture lacks gives `ErrorKind::InvalidArgument`.

### 8.2. Vertex and Index Buffers

SLO-25. `slango` MUST provide a sealed trait `VertexElement`, a supertrait of `Element`, for every `Element` and the packed arrays of 2 and 3 `f32`, `i32`, or `u32`, because a vertex attribute reads packed vectors, such as `Float32x3`.

`VertexElement` holds `f32`, `i32`, and `u32` and the arrays of 2, 3, and 4 of each, with the packed strides 4, 8, 12, and 16 bytes. `Element` keeps its six types and their strides (Section 8).

`Buffer::from_slice`, `Buffer::zeroed`, `Buffer::read`, `Buffer::len`, and `Buffer::is_empty` take the bound `VertexElement`. `Call::bind` keeps the bound `Element`, because only those types have the stride test of SLO-10. Every buffer has the usages `VERTEX` and `INDEX` (Section 8).

The reflected scalar type and component count of an attribute map to its vertex format, such as `float` to `Float32`, `float2` to `Float32x2`, `int3` to `Sint32x3`, and `uint4` to `Uint32x4`. The draw checks the element type of a vertex buffer against its attribute, such as `[f32; 2]` for a `float2` (Section 9.5). A vertex buffer of a generated struct (Section 8) has no type of a vertex attribute, so the draw rejects it with `ErrorKind::InvalidArgument`, because no vertex attribute holds a structure.

An index buffer is a `Buffer<u32>`, read with `IndexFormat::Uint32`.

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
- A buffer of a generated struct (Section 8) binds only to such an element, by its stride, and never to a scalar or vector element. The error then names the struct by its Slang name.

A scalar or vector element matches exactly, and a structure by its stride alone, by the owner's decision. The exact check also covers `bool` and every scalar type that no `Element` has, such as `half`, `double`, and the 8-bit and 16-bit integers, because SLM-5 covers every scalar and vector and the exact check is the sounder choice.

No `Buffer<T>` can therefore be bound to such a parameter, such as a `Buffer<u32>` to a `StructuredBuffer<bool>`. Matrices and arrays keep the check of the stride alone, as structures do, because SLM-5 requires the exact check only for scalars and vectors.

The dispatch runs the checks in this order and returns the first failure, all before any call of wgpu:

1. Each given argument, in the order given: its path, the kind of its parameter, and its value, its buffer, or its texture or sampler. A buffer is checked for its device, then that it is not empty, then its stride, then its element type, so a buffer of another stride gets the error about its stride. A texture is checked for its device, then its format, then its usage.
2. Each used parameter that no argument gives, in the order of the bindings, followed by the immediates.
3. The workgroup count along each axis against the device's limit.

A mistake of the caller in these checks gives `ErrorKind::InvalidArgument`, whose message names the kernel and the parameter. A reflected layout that cannot hold a value gives `ErrorKind::UnsupportedParameter`.

`slango` makes no aliasing check of its own. When one buffer is bound to a read-only and to a read-write parameter of one call, wgpu's validation rejects the dispatch, and the dispatch returns `ErrorKind::Backend` (SLO-15, Section 3.8).

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

`slango` makes no check of its own that one texture is both sampled and a target of one draw. wgpu's validation rejects it (Section 3.9), as it rejects aliased buffers (Section 9.3).

## 10. Errors

SLO-16. `slango` MUST have one public error type as ERR-1 defines, with these kinds of `ErrorKind`, because callers match on the kind (ERR-1):

| Kind | Meaning |
| --- | --- |
| `Compile` | Slang failed to compile or link the module. `source()` returns the error of `slang-compiler` with its diagnostics, except for the rejection of a vertex or fragment entry point, whose text holds that error's text and which has no source error (Section 7.4) |
| `NoAdapter` | wgpu found no adapter that matches the options |
| `DeviceRequest` | The adapter refused the device that the options request |
| `UnsupportedShader` | The backend rejected a kernel at load, or a render pipeline at its creation, as the text after this table states. The source error carries the backend's text |
| `UnsupportedParameter` | A kernel or a render pipeline has a parameter that the walk or SLO-23 rejects, or whose two stages' bindings conflict (Sections 7.1, 7.4, and 7.5), or, at a call or a draw, the reflected layout of a plain value cannot hold the value (Section 9.3) |
| `MissingFeature` | A kernel needs a device feature that the device lacks |
| `InvalidArgument` | A check of Section 9.3 or of SLO-26, including a value that does not match its parameter (Section 9.2), an unknown kernel or entry point name, a wrong option of a render pipeline, a file that cannot be read as text, a buffer larger than the device's limit, whose size overflows, or whose elements have 0 bytes, or a texture whose size or pixels Section 8.1 rejects |
| `Backend` | wgpu reported a validation or out-of-memory error in an error scope at load or at the creation of a render pipeline, as the text after this table states, any error while a buffer, texture, or sampler is created or read, or any error that wgpu reports during a dispatch or a draw (SLO-15) |

The load creates every shader module, bind group layout, pipeline layout, and compute pipeline inside error scopes for validation, internal, and out-of-memory errors, so that no error reaches wgpu's uncaptured-error handler. It maps each error that a scope returns to a kind:

- Any error while the shader module is created gives `UnsupportedShader`.
- An internal error while the layouts or the pipeline are created gives `UnsupportedShader`, because a shader that the backend cannot translate is reported as an internal error.
- A validation or out-of-memory error while the layouts or the pipeline are created gives `Backend`.

The message names the kernel, and the backend's text is returned unchanged as the source error.

`Module::render_pipeline` creates the vertex shader module, the fragment shader module, the layouts, and the render pipeline, each in its own error scope, and maps each error that a scope returns to a kind:

- Any error while a shader module is created gives `UnsupportedShader`.
- An error while the bind group layouts and the pipeline layout are created maps as for a kernel.
- A validation or internal error while the render pipeline itself is created gives `UnsupportedShader`, and an out-of-memory error gives `Backend`. Such a validation error reports a mismatch between the two stages or with the pipeline, such as varyings in another order or of another type (Section 3.9), which only a change of the shader fixes.

The message names both entry points, and the backend's text is returned unchanged as the source error. Every other failure of rendering has the kind that Sections 7.4 to 9.5 state, so `ErrorKind` has eight kinds.

`NoAdapter` answers Q-22 of the architecture design.

## 11. Threading

SLO-17. `Device`, `Module`, `Kernel`, `Buffer<T>`, `RenderPipeline`, `Texture`, `Sampler`, `Error`, and every options type MUST be `Send` and `Sync`, asserted in a test (CONC-1). `slango` MUST implement neither trait by hand, because the types hold only wgpu objects, which are `Send` and `Sync` (Section 3.2), and owned data, so the compiler derives both.

The builders `Call` and `Draw` also hold only such data, and a unit test asserts that each is `Send` and `Sync`. Clones of one kernel may be dispatched from several threads at once, each with its own `Call`, and clones of one render pipeline may be drawn with from several threads, which `gpu` tests check. Each dispatch sees only its own errors, because the error scopes of wgpu 30 are per thread (Section 3.2).

`load_module` uses Slang only on the calling thread, and its Slang objects never leave the call (SLO-5), which keeps every rule of Section 10 of `docs/design/slang-compiler.md`. Several threads that load modules at once each create their own global session, which Slang supports (Section 3.5 of `docs/design/slang-compiler.md`). `slango` starts no thread (CONC-4).

## 12. Device Features

SLO-18. `Device::new` MUST request `Features::IMMEDIATES` with the adapter's maximum immediate size when the adapter offers the feature. The load MUST fail with `ErrorKind::MissingFeature` when a kernel has `uniform` parameters of its entry point and the device lacks the feature or a large enough immediate size.

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

In each frame, the example wraps the view of the surface texture with `slango::wgpu::texture_from_view` and draws one indexed quad into it, with a checkerboard texture that it creates with `Texture::from_pixels` and a `float4x4` transform. The transform rotates the quad by an angle that grows with time and keeps it square at the aspect ratio of the target. The example then presents the surface texture through the wgpu queue that `slango::wgpu::queue` returns, because wgpu 30 presents through the queue.

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

## 14. Assumptions to Confirm

Each row is a statement that this design relies on and that no test or source establishes. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in Section 3 and in the section that relies on it, and removes the row.

An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| SOA-6 | A system value other than `SV_Position`, `SV_VulkanVertexID`, and `SV_VulkanInstanceID` lacks the category `VaryingInput`. An index past the end of a vertex buffer reaches only the backend's own handling | Sections 7.4 and 16 | For another system value, the change that first uses it. For an index past the end, deferred with no change assigned |

Of the assumptions of the architecture design that name this document, Sections 3.3 to 3.5 confirm A-12, Section 3.7 confirms A-13, and Section 3.2 confirms A-11 for M3. M4 does not need A-11, because it keeps the validated path and defers other shader targets (Section 1.5).

Section 3.7 confirms A-10 in part, for the first three patterns of Section 8.3 of that design and the description of `GpuFuture`, with "replacing" read as "building an experimental replacement". A-10 stays for typed shader interfaces generated from reflection. A-9 holds as far as SlangPy's documentation states (Section 3.7), and `slango` does not depend on it.

## 15. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it in the milestone of the last column, and the change that answers it removes the row.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| SOQ-3 | How large must a kernel be before naga's parsing at load or lavapipe's first dispatch takes long enough to matter? | This document | The milestone that first loads kernels larger than those of the example |

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

The residual risk is a defect of naga, wgpu, or a GPU driver that lets a kernel or a draw read or write outside its buffers and textures. A crash that a defect of Slang causes is also residual, and Section 3.12 of `docs/design/slang-compiler.md` records each such defect.

## 17. Operational Considerations

### 17.1. Cargo Features and Environment Variables

`slango` has no Cargo features in M3 and M4. It reads no environment variable itself. Its build needs `VULKAN_SDK`, and its programs need the Slang shared library on the library path, as `slang-compiler-sys` requires (Section 18.4 of `docs/design/slang-compiler-sys.md`).

### 17.2. Platforms and Minimum Supported Rust Version

The crate runs on Linux with Vulkan, on a software and on a hardware adapter. On Windows with Direct3D 12 and on macOS with Metal, it creates a device, loads modules, dispatches kernels, samples textures in them, and draws offscreen in CI, with the same conventions of a draw on all three (Sections 3.8 and 3.9).

wgpu 30.0.1 declares `rust-version` 1.87, below the workspace's minimum supported Rust version (MSRV) of 1.99.

### 17.3. Failure Modes

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
| A draw samples the texture that it draws into | `Backend`, with wgpu's text as the source error | Draw into another texture |
| wgpu reports a validation or out-of-memory error (Section 10), or an error while a buffer, texture, or sampler is created or read | `Backend` | Report a validation error as a defect of `slango` |

### 17.4. Costs

Each load of a module pays for its own global session, well over 100 ms, plus about 10 ms for a small module (Sections 3.6 and 7.2). A program that reloads a shader therefore waits on the order of 150 ms for each reload.

### 17.5. Logs

`slango` emits no logs in M3 and M4 and has no logging dependency, as `slang-compiler` does. This answers Q-16 of the architecture design for `slango` through M4.

## 18. References

### 18.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

### 18.2. Informative References

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
| SLO-14 | 9.3 | Names, presence, values, the strides and element types of buffers, devices, and workgroup limits are checked before dispatch. |
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
