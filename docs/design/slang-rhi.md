# `slang-rhi` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-19 |
| Applies to | `crates/slang-rhi` |

## Abstract

This document specifies `slang-rhi`, the safe Rust crate over the raw bindings of `slang-rhi-sys`, in the scope of milestone M9: the path of one compute dispatch. A program lists the adapters of a backend, opens a device with a global session of `slang-compiler`, creates a shader program from a linked Slang program of the device's session and a compute pipeline from it, creates buffers, encodes a compute pass that binds parameters by name and dispatches, submits the work, and reads a buffer back. The document defines the ownership of every object, the borrows that turn the unowned pointers of slang-rhi into lifetimes, the checks that keep a safe call within valid use of the graphics API, the errors, the threading rules that follow from slang-rhi documenting none, the guards around calls into native code, the tests, and the additions of M10. M10 adds what the backend `rhi` of `slango` needs, in two phases: sessions of their own that reproduce the device session, the objects of constant buffers and parameter blocks, and the limits of a device for compute, and then textures, samplers, render pipelines, render passes, and the reading of textures for rendering.

## 1. Introduction

### 1.1. Problem

The raw bindings of `slang-rhi-sys` require a caller to count references, to keep the owner of every unowned pointer alive, to build parameter bindings whose fields hold references, and to decide which objects may be used from which threads, while slang-rhi documents almost none of these rules (Section 3.3 of `docs/design/slang-rhi-sys.md`). A mistake is undefined behaviour that a test on one driver may not show.

### 1.2. Purpose

`slang-rhi` gives Rust users a safe API for the compute path of slang-rhi, and from M10 for the draws that `slango` needs, built on `slang-compiler` for everything that concerns Slang. The `rhi` backend of `slango` in M10 builds on it (Section 15.13 of the architecture design).

This document implements the part of Section 15.12 of the architecture design that concerns `slang-rhi`, within the owner's decisions of Section 4.1 of `docs/design/slang-rhi-sys.md`. It answers Q-16 of the architecture design for this crate (Section 14.5).

### 1.3. Audience

The primary readers are the coding agents that implement the crate and the `rhi` backend of `slango`. The secondary readers are the reviewers of those changes and the owner. A reader is expected to know Rust and the ownership model of `docs/design/slang-compiler.md`, but not slang-rhi.

### 1.4. Scope

This document covers the crate and its dependencies, the ownership model, the public API of the compute path and of the rendering phase of M10, errors, threading, the guards around calls into native code, the soundness invariants, the tests, the crate documentation, and the security and operational considerations.

### 1.5. Non-Goals

This document does not cover, in M9:

- Rendering, textures, samplers, fences, mapped buffers, copy commands, and every other part of the API of slang-rhi that the compute path does not need, which M10 adds as `slango` needs them (Section 11).
- Backends other than Vulkan on Linux and D3D12 on Windows, which follow the platforms of Section 7 of `docs/design/slang-rhi-sys.md`.
- A version of the API whose types are `Send` or `Sync`, which slang-rhi's missing threading rules forbid (Section 7).
- The raw bindings and the native build, which `docs/design/slang-rhi-sys.md` specifies.

## 2. Requirements Language

Best Current Practice (BCP) 14 defines the key words in this section.

The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "NOT RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in BCP 14 [RFC2119] [RFC8174] when, and only when, they appear in all capitals, as shown here.

### 2.1. Terminology

This document uses the terms that these documents define:

- Section 2.2 of `docs/writing-documentation.md`, such as agent, role, and user.
- Section 2.2 of `docs/rust-coding-standards.md`, such as interface, interface pointer, result code, and safe crate.
- Section 2.1 of `docs/design/architecture.md`, such as adapter, buffer, compute dispatch, compute kernel, device, global session, lavapipe, session, and slang-rhi.
- Section 2.1 of `docs/design/slang-compiler.md`, such as Slang call guard and wrapper.
- Section 2.1 of `docs/design/slang-rhi-sys.md`, such as backend.

This document calls those files the writing guide, the coding standard, the architecture design, the `slang-compiler` design, and the sys design. It adds these terms:

- abort guard: The crate-private function of this crate that runs one call into native code and aborts the process when a C++ exception reaches it, as FFI-7 of the coding standard requires (Section 8.1).
- command buffer: A recorded list of GPU commands that a queue executes.
- command encoder: The object that records commands into a command buffer.
- compute pass: The part of a command encoder in which compute pipelines are bound and dispatched.
- compute pipeline: A GPU object that holds a compiled compute shader.
- debug callback: The object through which slang-rhi reports messages of its debug device, the validation layer, the driver, and Slang (Section 3.3 of the sys design).
- debug device: The layer of slang-rhi that wraps a device when `DeviceDesc::enableValidation` is set and checks each call into slang-rhi against slang-rhi's own rules (`src/rhi.cpp:341-356`). It is not a validation layer of the graphics API.
- per-thread guard: The crate-private mark of this crate that records, for each thread, whether a call of the crate into slang-rhi is in progress on it (Section 8.2).
- queue: The object of a device that executes command buffers.
- root shader object: The shader object that holds the global parameters of the program of a bound pipeline.
- shader object: The slang-rhi object that holds the values and resources of the parameters of a program or of one of its entry points.
- shader offset: The position of a parameter in a shader object, made of a byte offset of uniform data, a binding range index, and an index in the array of that range.
- shader program: The slang-rhi object that holds a linked Slang program for a device.
- valid use: The conditions that the specification of the graphics API places on each call, such as the valid usage rules of Vulkan. A call that breaks one has undefined behaviour by that specification.

## 3. Facts

This section is non-normative. Section 3 of the sys design states the facts about slang-rhi on which this design relies, and this section adds the facts of the compute path and of rendering. Paths are in the slang-rhi tree of the sys design.

The path of one compute dispatch is the one of `tests/test-compute-trivial.cpp:6-45`. It creates a device, takes the Slang session of the device with `IDevice::getSlangSession` (`include/slang-rhi.h:3401`), loads and links a program there, creates a shader program (`:3618`) and a compute pipeline (`:3656`), creates buffers (`:3456`), takes a queue (`:3555`) and a command encoder (`:2862`), begins a compute pass (`:2608`), binds the pipeline, which returns the root shader object (`:2528`), sets the parameters, dispatches (`:2531`), ends the pass (`:2496`), finishes the encoder (`:2769`), submits (`:2886`), waits (`:2902`), and reads the result (`:3704`).

Every descriptor on that path is passed by `const` reference, and no method on it takes an aggregate by value. `ShaderOffset` holds three `uint32_t` (`:1655`). `IShaderObject` sets data by `setData(const ShaderOffset&, const void*, Size)` and a resource by `setBinding(const ShaderOffset&, const Binding&)` (`:1750`, `:1753`), and offers no lookup by name. The helper `ShaderCursor` finds a field by name through the reflection of the element type layout of the object: `findFieldIndexByName`, the field's uniform offset, and `getFieldBindingRangeOffset` (`include/slang-rhi/shader-cursor.h:87`, `:169-194`). When a lookup at the root fails, `ShaderCursor` also searches the entry points (`include/slang-rhi/shader-cursor.h:245-262`).

The element type layout of a root shader object is the struct of the global parameters of the program, unwrapped from its container (`src/shader-object.h:132-166`, `src/vulkan/vk-shader-object-layout.cpp:901-903`). `ShaderObject::setData` clamps a size that runs past the uniform data of the object on purpose (`src/shader-object.cpp:67-72`).

`createShaderProgram` takes an optional out-parameter for diagnostics (`:3618`), which the Vulkan backend never fills (`src/vulkan/vk-device.cpp:2521-2538`). Code generation reports the errors of Slang only through the debug callback (`src/shader.cpp:200-207`). slang-rhi reports other messages through the debug callback of the device, from the calling thread or from a thread of the driver (`docs/error-handling.md:39-63`). A device whose `DeviceDesc::debugCallback` is null gets the static `NullDebugCallback`, which drops every message (`src/device.cpp:493`, `src/device.h:139-158`).

`ShaderProgram::compileShaders` holds a `std::mutex` while it compiles (`src/shader.cpp:113`), and the debug callback runs while that mutex is held. A second lock of a `std::mutex` by the thread that holds it is undefined behaviour.

slang-rhi checks the arguments of a call against the rules of the graphics API only in its debug device. Without the debug device, it passes them to the driver unchecked. The validation layer of Vulkan, which checks valid use, is turned on only through `IRHI::enableDebugLayers` (`src/vulkan/vk-device.cpp:382-449`), which affects the whole process and cannot be turned off again.

The `IRHI` instance is one object of the process, which any thread reaches through `rhiGetInstance`, and slang-rhi documents no threading rule for it (Section 3.3 of the sys design).

A device whose `SlangDesc::slangGlobalSession` is null creates a global session of its own (Section 3.3 of the sys design), and that session has no loader of `slang-glslang-sys`. A C++ program outside the repository, linked against the vendored Slang and slang-rhi, created such a device and compiled a compute shader through `IDevice::getSlangSession`. Its debug callback received `error[E00100]: failed to load downstream compiler 'spirv-opt'` with `note[E99996]: failed to load dynamic library 'slang-glslang-2026.17'`, and the SPIR-V, which was not optimized, still ran and gave the expected results.

The device's session has exactly one target, the device's compile target, with `forceGLSLScalarBufferLayout` set whatever the format of the target, DXIL included (`src/slang-context.h:78-92`, `:87`), and slang-rhi generates the code of a program for the target at index 0 of the program's session (`src/shader.cpp:150-156`).

A native shader program holds the Slang session of its program through its root shader object layout, whose field `ShaderObjectLayout::m_slangSession` (`src/shader-object.h:128`) is set from the program's session (`src/shader-object.cpp:18-25`), and `createShaderProgram` creates that layout at once (`src/vulkan/vk-device.cpp:2520-2534`, `src/d3d12/d3d12-device.cpp:1795-1801`). A pipeline, a shader object, or a deferred compilation therefore keeps the session alive after every Rust value that held it is dropped. A Slang component type holds only a raw pointer to its linkage (`source/slang/slang-linkable.h:416` of the Slang tree), so without that native reference every Rust object made from a program would have to hold the session.

When a root shader object is created, `ShaderObject::init` creates one object for each binding of each range of sub-objects that has a layout, recursively (`src/shader-object.cpp:515-547`). The layouts give a sub-object index to the ranges of the binding types `ConstantBuffer`, `ParameterBlock`, and `ExistentialValue` (`src/vulkan/vk-shader-object-layout.cpp:334-340`, `src/d3d12/d3d12-shader-object-layout.cpp:148-154`), and a range of `ExistentialValue` has no layout and gets no object, so its entry stays null (`src/shader-object.cpp:527-532`). `getObject` indexes the objects with no check of the kind of the range or of the array index (`src/shader-object.cpp:121-130`).

A `[vk::push_constant]` buffer gives a range of the binding type `PushConstant`, which gets no sub-object on Vulkan (`src/vulkan/vk-shader-object-layout.cpp:334-371`). A `cbuffer` and a `ConstantBuffer` both reflect as the binding type `CONSTANT_BUFFER`.

`createDevice` creates the device session from `DeviceDesc::slang`, which the crate leaves at its defaults apart from the global session (`include/slang-rhi.h:3224-3247`), and from the arguments of each backend (`src/slang-context.h:17-94`):

- The matrix layout is row-major, the default of `SlangDesc` (`include/slang-rhi.h:3229`), and the session has no search path.
- The one macro is `__VULKAN__` on Vulkan and `__D3D12__` on D3D12, each with the value `1` (`src/vulkan/vk-device.cpp:1809`, `src/d3d12/d3d12-device.cpp:1239`).
- The session options hold one `Capability` entry for each capability of the device, in ascending order, with the name from `IRHI::getCapabilityName` and the identifier from `IGlobalSession::findCapability` (`src/slang-context.h:52-66`, `src/device.cpp:523-534`, `src/rhi.cpp:271-279`). The capabilities are final before the session is created on both backends (`src/vulkan/vk-device.cpp:1658-1661`, `src/d3d12/d3d12-device.cpp:839-1215`).
- The target is SPIR-V with no profile on Vulkan (`src/vulkan/vk-device.cpp:1804-1810`). On D3D12, it is DXIL with the profile of the highest known shader model of the device from 6.0 up, for which slang-rhi reports a feature `SM_6_0` to `SM_6_10` for each known shader model at or below it, and DXBC below shader model 6.0 (`src/d3d12/d3d12-device.cpp:45-71`, `:1205-1240`).
- The floating-point mode, the line directive mode, and the target flags are the defaults (`src/slang-context.h:84-86`). slang-rhi does not pass `SlangDesc::optimizationLevel` to the session.

The facts that the checks of the buffers and of the dispatch rely on are these:

- `createBuffer` maps each flag of `BufferUsage` to a Vulkan buffer usage bit (`src/vulkan/vk-utils.cpp:479-494`). `ShaderResource` and `UnorderedAccess` both give `STORAGE_BUFFER`, and `ConstantBuffer` gives only `UNIFORM_BUFFER`. `AccelerationStructure`, `AccelerationStructureBuildInput`, and `ShaderTable` map to bits of extensions, and `Shared` to external memory (`src/vulkan/vk-buffer.cpp:424-434`), which a plain compute device need not enable.
- `createBuffer` adds `TRANSFER_DST` itself when initial data is given (`src/vulkan/vk-buffer.cpp:408-411`). The memory types `Upload` and `ReadBack`, and the usage `ConstantBuffer`, select host-visible coherent memory, into which initial data is copied through a mapping, and `DeviceLocal` memory receives initial data through a staging copy (`src/vulkan/vk-buffer.cpp:413-421`, `:443-460`). A memory type that the device lacks makes the call fail.
- `readBuffer` copies with `vkCmdCopyBuffer` for every memory type (`src/vulkan/vk-device.cpp:1869-1918`), which needs `TRANSFER_SRC` on the buffer.
- A command buffer that the queue has run is recycled by slang-rhi for a later encoder (`src/vulkan/vk-command.cpp:1912-1937`), and the queue keeps its own reference to each command buffer in flight (`src/vulkan/vk-command.cpp:1996-2003`).
- `DeviceInfo::limits.maxComputeDispatchThreadGroups` holds the Vulkan limit `maxComputeWorkGroupCount` (`src/vulkan/vk-device.cpp:1613-1615`), and `maxTextureDimension2D`, `maxVertexStreams`, and `maxVertexInputElements` also come from the driver (`:1599-1615`).
- `DeviceInfo::limits.maxBufferSize` is 2 GiB on both backends, a value that slang-rhi assumes and does not ask the device for (`src/vulkan/vk-device.cpp:1597`, `src/d3d12/d3d12-device.cpp:796`).
- With the linking style `SingleProgram` and no separate entry points, `ShaderProgram::_isSpecializable` checks the specialization parameter count of the global scope (`src/shader.h:132-146`). For a specializable program, `createComputePipeline` creates a virtual pipeline whose code slang-rhi generates at the first dispatch (`src/device.cpp:758-764`, `src/pipeline-resolver.cpp:96-106`).
- The Vulkan backend of `createComputePipeline` asserts that the program has an entry point, and the failed assertion aborts the process (`src/vulkan/vk-pipeline.cpp:836`, Section 3.3 of the sys design).

The D3D12 backend differs from the Vulkan backend on the compute path in these facts, which a reading of the D3D12 backend established:

- On the heaps of the memory types `Upload` and `ReadBack`, `createBuffer` clears the resource flags, which drops `ALLOW_UNORDERED_ACCESS` (`src/d3d12/d3d12-device.cpp:1311-1324`). A buffer with `UnorderedAccess` in such memory is then created, and a later unordered access view of it is not valid use of D3D12.
- `createBuffer` asserts that a `ReadBack` buffer gets no initial data, and the assertion is compiled into every build type and aborts the process (`src/d3d12/d3d12-device.cpp:1314`).
- On D3D12, slang-rhi creates a `ReadBack` buffer only with the state of a copy destination, so a `ReadBack` buffer with `CopySource` fails at creation, and `readBuffer` cannot read any `ReadBack` buffer that the crate can create. The call fails with an error.
- `readBuffer` copies a buffer in another memory type into a staging buffer in `ReadBack` memory and needs no usage flag of the buffer (`src/d3d12/d3d12-device.cpp:1739-1765`). It records no transition of the source buffer into the copy source state. Whether that breaks valid use of D3D12, or the implicit promotion of a buffer from the common state covers it, is not verified.
- slang-rhi never transitions a buffer in `Upload` or `ReadBack` memory (`src/core/state-tracking.h:41-46`).
- A buffer parameter without a bound buffer gets a null descriptor, which reads zeros and drops writes (`src/d3d12/d3d12-shader-object.cpp:456-459`).
- `DeviceInfo::limits.maxComputeDispatchThreadGroups` is 65535 along each axis, the limit of D3D12 (`src/d3d12/d3d12-device.cpp:812-814`), and `maxTextureDimension2D`, `maxVertexStreams`, and `maxVertexInputElements` are the constants of the API as well, 16384, 32, and 32 (`:798-814`).
- On a device of shader model 6 or later, which WARP is, the device compiles every program to DXIL through Slang, which loads `dxcompiler.dll` (Section 7.3 of the sys design). The crate forces no target, and the device's session holds the DXIL target that slang-rhi chose.

The raw types of the compute path in `slang-rhi-sys` have these shapes (RSYS-10 of the sys design), and a `gpu` test of that crate runs the whole compute path through them (Section 9 of the sys design):

- `DeviceDesc`, `BufferDesc`, `SubmitDesc`, and the other descriptors of the compute path implement `Default` with the C++ defaults, so the crate writes `..T::default()` for every field that it does not set. The default `cudaStream` of `SubmitDesc` is `kInvalidCUDAStream`.
- `Binding` holds its two resources as the raw pointers `resource` and `resource2` of type `*mut IResource`, and the range in a field of an anonymous union. slang-rhi adds its own references during `setBinding`, so the crate keeps its own reference through the call (Section 6.7).
- The struct constant `kEntireBuffer` has no binding, so the crate writes the range of a whole buffer itself, as an offset of 0 and a size of `u64::MAX`.
- `ShaderOffset` is a plain record of three `u32` fields.
- The methods of the placeholder slots have no thunk, and none of them is on the compute path.

The rendering phase of Section 11.2 relies on these facts of both backends, read in the source unless a test is named:

- `ITexture::getDefaultView` returns its view with a reference added (`src/vulkan/vk-texture.cpp:112-122`, `src/d3d12/d3d12-texture.cpp:91-101`).
- `beginRenderPass` copies the attachments of its descriptor and retains their views (`src/command-list.cpp:84-101`). `IRenderPassEncoder::setRenderState` retains the vertex and index buffers, `bindPipeline` creates a new root shader object on each call, and each draw writes the render state that was set last (`src/command-buffer.cpp:26-97`).
- `uploadTextureData` copies, for each row, the smaller of the row pitch of the source and of the staging memory (`src/command-buffer.cpp:578-660`), and both backends use it for the initial data of `createTexture`.
- `ITexture::getSubresourceLayout` with `kDefaultAlignment` aligns each row to the alignment of the platform, which is 256 bytes on D3D12 (`src/rhi-shared.cpp:197-207`, `src/d3d12/d3d12-device.cpp:1399-1403`). `readTexture` copies `sizeInBytes` of the layout that the caller passes, on a generic path that both backends share (`src/device.cpp:905-945`).
- The Vulkan backend gives each input element the location of its index and the binding of its `bufferSlotIndex` (`src/vulkan/vk-device.cpp:2502-2506`). The D3D12 backend copies the semantic name and index, the `InputSlot` from `bufferSlotIndex`, and the step rate of the stream, and does not check `bufferSlotIndex` against the number of streams (`src/d3d12/d3d12-device.cpp:1699-1714`).
- On D3D12, DXIL keeps a vertex input that the shader declares and does not read in its input signature. A render pipeline whose input layout held no element for it failed to be created, with `SLANG_FAIL` and no diagnostics (CI run 44423418363, on WARP). On SPIR-V, Slang removes such an input. Telling a used input from a declared one would need a code generation pass before slang-rhi compiles the pipeline.
- The Vulkan backend reads every core feature that the device supports with `vkGetPhysicalDeviceFeatures` and enables them all (`src/vulkan/vk-api.cpp:176`, `src/vulkan/vk-device.cpp:603`), so `robustBufferAccess`, which core Vulkan requires every device to support, is enabled, and so is `VK_EXT_vertex_attribute_robustness` where the device has it (`src/vulkan/vk-device.cpp:864-869`). A draw with the index 100000 into a buffer of 3 vertices ran on the NVIDIA RTX 4070 and on lavapipe with no loss of the device and no error of the Khronos validation layer, and a draw after it was exact. This confirmed the former assumption RHA-7.
- The Vulkan backend always enables depth clamping (`src/vulkan/vk-pipeline.cpp:634`), while the D3D12 backend clips by `RasterizerDesc::depthClipEnable`, whose default is on (`src/d3d12/d3d12-pipeline.cpp:274`, `include/slang-rhi.h:1866`). A primitive outside the depth range 0 to 1 is therefore drawn with its depth clamped on Vulkan and not drawn on D3D12, which the `gpu` test `a_triangle_outside_the_depth_range_is_clamped_on_vulkan_and_clipped_on_direct3d_12` observed on both (RKB-9 in Section 3.1).
- `getFormatSupport` is wrong on both backends for some bits (RKB-7 and RKB-8 in Section 3.1). Every colour format and use of a texture that the crate offers is mandatory on Vulkan and on D3D12, and so is a `D32Float` depth target on D3D12, while Vulkan does not guarantee a `D32_SFLOAT` depth attachment.

The `gpu` tests of Section 10.3 confirmed these conventions on Vulkan, on the NVIDIA RTX 4070 and on lavapipe with the debug device of slang-rhi off and on, and on D3D12 on WARP in CI run 44431645707:

- Clip space points y up, and the top row of a target comes first, as the negative viewport height of the Vulkan backend predicts (`src/vulkan/vk-command.cpp:845-851`). `read_texture` returns the rows from the top, also with the row pitch of 256 bytes of D3D12.
- Counter-clockwise triangles face the front, and `CullMode::Back` culls the others.
- The depth range of the viewport is 0 to 1, and the depth test `Less` keeps the nearest fragment, after a clear and after a load.
- `SV_Position` has its origin at the top left with texel centres at 0.5, and initial texture data starts at the top row.
- A pass with only a depth target clears a new depth texture.
- A stream that steps per instance with the step rate 1, and `BlendMode::Alpha`, give the expected values.

### 3.1. Known Upstream Bugs

This is the one list of the known defects of the recorded slang-rhi. It has the columns of Section 3.12 of the `slang-compiler` design and follows the rules stated there, and every path in the column "Source" is in the slang-rhi tree.

The crate does not guard against these defects, as Hard Rule 14 of `AGENTS.md` requires for Slang. The checks of RHI-8, RHI-11, and RHI-20 are not guards, because a safe interface checks the bounds of an index and the valid use of the graphics API whatever slang-rhi does (Section 9.1).

| Identifier | Defect and smallest input | Call | Outcome | Source | Report |
| --- | --- | --- | --- | --- | --- |
| RKB-1 | The range check of `readBuffer` adds `offset` and `size` with no guard against overflow, so a sum that wraps lets a range past the end of the buffer pass. By a reading, the smallest input is a buffer of any size read with `offset` set to `SIZE_MAX` and `size` set to 1, whose sum wraps to 0 | `IDevice::readBuffer` | Not observed, because the defect was read in the source and not reproduced. By the reading, the check passes and the backend reads from `offset` bytes past the start of the buffer, as the CPU backend does with `memcpy` (`src/cpu/cpu-buffer.cpp:61`) | `if (offset + size > bufferImpl->m_desc.size)` in the Vulkan backend (`src/vulkan/vk-device.cpp:1872`), where `Offset` and `Size` are `size_t` (`include/slang-rhi.h:57-58`) and `m_desc` is a `BufferDesc` (`src/rhi-shared.h:95`) whose `size` is `uint64_t` (`include/slang-rhi.h:745`). The CPU, WebGPU, CUDA, Metal, D3D12, and D3D11 backends have the same check (`src/cpu/cpu-buffer.cpp:57`, `src/wgpu/wgpu-device.cpp:470`, `src/cuda/cuda-device.cpp:534`, `src/metal/metal-device.cpp:377`, `src/d3d12/d3d12-device.cpp:1730`, `src/d3d11/d3d11-device.cpp:567`), and so does the debug layer (`src/debug-layer/debug-device.cpp:1221`) | Not reported |
| RKB-2 | A compute or render pipeline of a program with a constant buffer and a global uniform fails on D3D12. The smallest input is a module with `struct Params { float4 value; };`, `ConstantBuffer<Params> constants;`, `uniform float scale;`, and `RWStructuredBuffer<float> values;`, and a compute entry point with `[numthreads(1, 1, 1)]` whose body is `values[0] = constants.value.x + scale;`. The same kernel with `ParameterBlock<Params>` in place of `ConstantBuffer<Params>`, and the kernel without the global uniform, both work on D3D12, and all three work on Vulkan. A render pipeline fails the same way, such as one whose vertex stage reads `uniform float4x4 transform;` and whose fragment stage reads `ConstantBuffer<Frame> frame;` | `IDevice::createComputePipeline`, through `Device::create_compute_pipeline`, and `IDevice::createRenderPipeline`, through `Device::create_render_pipeline`, on a D3D12 device on WARP on x86_64 Windows, with the vendored Slang built with DXIL support and `dxcompiler.dll` 10.0.26100.0 | `SLANG_FAIL` (-2147467259), with no diagnostics and no message of the debug callback, while the debug device of slang-rhi was off. Observed for a compute pipeline in CI run 44282421547, and for two render pipelines of the `gpu` tests of `slango` in CI run 44470355051, where `slango` reported the kind `Compile`. The failing `HRESULT` and the message of the D3D12 debug layer are not captured. Whether the root signature of slang-rhi or the DXIL layout of Slang is at fault is not established. Both kinds of pipeline use the root signature that slang-rhi builds once from the layout of the program. slang-rhi chooses the DXIL target and its options, because the crate accepts only a program of the device's own session (RHI-7) | `DeviceImpl::createComputePipeline2` turns any failing `HRESULT` of `CreateComputePipelineState` into `SLANG_FAIL` with no report (`src/d3d12/d3d12-pipeline.cpp:489`, `:535-536`), and the render path does the same (`:439`, `:446`, `:536`, `:543`). Both use the root signature of the program (`src/d3d12/d3d12-pipeline.cpp:240`, `:498-500`, `src/d3d12/d3d12-shader-object-layout.cpp:1000-1002`) | Not reported |
| RKB-3 | An exhausted GPU descriptor heap on D3D12 gives out descriptor handles built from null. By a reading, the smallest input is about 7,800 finished command buffers that each recorded one dispatch and that the program keeps, or a very large number of dispatches in one pass. The heap holds 1,000,000 descriptors (`src/d3d12/d3d12-device.cpp:696-699`), and the arena of each command buffer takes chunks of 128 (`src/d3d12/d3d12-command.cpp:2272`) | Every call that binds a shader object on D3D12, such as `IComputePassEncoder::dispatchCompute` | Not observed, because the defect was read in the source and not reproduced, which needs Windows. By the reading, `GPUDescriptorHeap::allocate` returns an empty range when the heap is full, `GPUDescriptorArena::allocate` does not check it and hands out handles made of a null base and an offset, the shader object writes descriptors through them, and the reset of the arena then fails the assertion of `GPUDescriptorHeap::free`, which aborts the process | `src/d3d12/d3d12-descriptor-heap.cpp:193-209` for the allocation, `:250-262` for the arena, and `:211-219` for the assertion, and `src/d3d12/d3d12-shader-object.cpp:189` for the write | Not reported |
| RKB-4 | `readBuffer` on D3D12 ignores `offset` for a buffer in `ReadBack` memory. By a reading, the smallest input is a `ReadBack` buffer of 8 bytes read with `offset` 4 and `size` 4, which returns its first 4 bytes | `IDevice::readBuffer` on D3D12 | Not observed, because the defect was read in the source and not reproduced. `slang-rhi` cannot reach it, because a `ReadBack` buffer has no use through the crate (Section 6.6) | For a `ReadBack` buffer, the backend maps the buffer itself and copies from its start, with the range `{0, size}` (`src/d3d12/d3d12-device.cpp:1767-1780`), while the copy for other memory types starts at `offset` (`:1761`) | Not reported |
| RKB-5 | On Vulkan, an array of `ConstantBuffer` parameters is bound as if it had one element. The smallest input is a module with `struct P { float v; };`, `ConstantBuffer<P> cbs[3];`, and `RWStructuredBuffer<float> o;`, and a compute entry point that writes `o[i] = cbs[i].v` for each `i` below 3, with `v` of element `i` set to `(i + 1) * 10` through the object that `ShaderObject::object` returns at array index `i` | The binding of the root shader object at `IComputePassEncoder::dispatchCompute`, through `ComputePass::dispatch`, on Vulkan | `o` held `[30.0, 0.0, 0.0]` on the NVIDIA RTX 4070 and `[30.0, 30.0, 0.0]` on lavapipe. The Khronos validation layer reported `VUID-VkComputePipelineCreateInfo-layout-07991`, with an array size of 3 in the SPIR-V against a `descriptorCount` of 1. Reproduced by a probe on 2026-11-18. Not observed on D3D12, where it did not run | The Vulkan backend steps the offset for each element of the range, while the descriptor set layout holds one descriptor for it (`src/vulkan/vk-shader-object.cpp:608-626`) | Not reported |
| RKB-6 | On Vulkan, a kernel that declares a push-constant buffer aborts the process at its dispatch, whether or not the program sets the buffer. The smallest input is a compute entry point of a module with `struct P { float v; };` and `[vk::push_constant] ConstantBuffer<P> pc;` | `IComputePassEncoder::dispatchCompute`, through `ComputePass::dispatch`, on Vulkan | The process aborts with "Assertion failed: Unsupported binding type: 14". Reproduced by a probe on 2026-11-18. The abort does not depend on `ShaderObject::object`, which refuses a push-constant range (RHI-22) | `src/vulkan/vk-shader-object.cpp:579` | Not reported |
| RKB-7 | On D3D12, `getFormatSupport` reports the support of the shader resource view format of a format, so `D32Float` reports no `DepthStencil`, although D3D12 requires depth-stencil support of `DXGI_FORMAT_D32_FLOAT`. By a reading, the smallest input is `getFormatSupport` with `Format::D32Float` on a D3D12 device | `IDevice::getFormatSupport` on D3D12 | Not observed, because the defect was read in the source and not reproduced. By the reading, the bit `DepthStencil` is missing. The crate asks for the support of a format on Vulkan only, where the format is not guaranteed, so it does not reach the defect (RHI-23) | The loop at `src/d3d12/d3d12-device.cpp:1128-1181` queries `formatMapping.srvFormat`, which is `DXGI_FORMAT_R32_FLOAT` for `D32Float` (`src/d3d/d3d-utils.cpp:89`) | Not reported |
| RKB-8 | On Vulkan, `getFormatSupport` takes the bits `CopySource` and `CopyDestination` from the features of linear tiling, while slang-rhi creates textures with optimal tiling, so both bits can be wrong for a texture. By a reading, the smallest input is `getFormatSupport` for a format whose features of linear and optimal tiling differ on the device | `IDevice::getFormatSupport` on Vulkan | Not observed, because the defect was read in the source and not reproduced. The crate does not read these bits | `src/vulkan/vk-device.cpp:1728-1729` | Not reported |
| RKB-9 | The Vulkan backend always enables depth clamping and ignores `RasterizerDesc::depthClipEnable`, while the D3D12 backend honours it, so the same pipeline draws a primitive outside the depth range 0 to 1 on Vulkan and clips it on D3D12. Depth clamping is also valid use of Vulkan only on a device with the feature `depthClamp`, which slang-rhi enables only where the device supports it (Section 3). The smallest input is the `gpu` test `a_triangle_outside_the_depth_range_is_clamped_on_vulkan_and_clipped_on_direct3d_12`, a triangle whose depth lies outside the range 0 to 1, drawn with its default `RasterizerDesc` | `IDevice::createRenderPipeline` and a draw of it | On Vulkan, on the NVIDIA RTX 4070 and on lavapipe, the triangle is drawn with its depth clamped. On D3D12 on WARP in CI run 44431645707, it is clipped and not drawn. A device without `depthClamp` was not available, so the outcome there, a render pipeline that breaks valid use of Vulkan, is a reading. The crate documentation states the difference, and the crate does not guard against it | `rasterizer.depthClampEnable = VK_TRUE` at `src/vulkan/vk-pipeline.cpp:634`, against `src/d3d12/d3d12-pipeline.cpp:274` and the default of `depthClipEnable` at `include/slang-rhi.h:1866` | Not reported |

## 4. Crate Structure

### 4.1. Package and Dependencies

RHI-1. The crate MUST depend on `slang-rhi-sys`, on `slang-compiler-sys`, on `slang-compiler` with the feature `vendored`, and on `bitflags`, and on no graphics crate. It MUST have no Cargo features.

`slang-rhi-sys` always links the vendored Slang (Section 4.1 of the sys design), and Cargo unifies the feature `vendored` of `slang-compiler-sys` in the whole build graph. `slang-compiler` without its own feature `vendored` would then create global sessions without the loader of `slang-glslang-sys`, and every program would get SPIR-V that is not optimized (SGL-12 of `docs/design/slang-glslang-sys.md`). Enabling the feature here installs the loader on every global session that `slang-compiler` creates.

The crate calls the reflection functions of `slang-compiler-sys` for the lookup of Section 6.7, so it depends on that crate directly. `bitflags` represents the usage flags of a buffer, which a caller combines, as TYPE-8 of the coding standard requires. It is in `[workspace.dependencies]` under DEP-2.

The manifest declares `slang-rhi-sys` and `bitflags` for every target, and `slang-compiler` and `slang-compiler-sys` for Linux with the GNU C library and x86_64 Windows with the Microsoft toolchain only, with the condition of the target table of `slang-rhi-sys` (RSYS-1 of the sys design). On another target, the build script of `slang-rhi-sys` stops the build at once, before anything is cloned, so the crate root carries no `cfg` of its own.

The crate takes the licence `MIT OR Apache-2.0` of the workspace (ARCH-36 of the architecture design), sets `publish = false` until the change that prepares the second release (TOOL-35 of `docs/design/tooling.md`), and stays out of `default-members` and of every recipe that passes `--workspace`, because building it builds Slang and slang-rhi from source (TOOL-32).

### 4.2. Unsafe Code

RHI-2. The crate root MUST carry the opt-out `#![expect(unsafe_code, reason = "...")]` of Section 4.4 of the coding standard. The owning interface pointer, the abort guard, the debug callback, and the lookup of shader offsets MUST each sit in a private module of their own, as UNS-2 requires, and each private module of a wrapper MUST hold the unsafe calls of the generated thunks that its own wrapper makes, each with its safety comment.

The crate calls the generated thunks of `slang-rhi-sys` and the reflection functions of `slang-compiler-sys`, which are unsafe, as `slang-compiler` calls those of Slang. It therefore joins the crates that may hold unsafe code (Section 3.3 of the architecture design).

The private modules are `owned`, `guard`, `callback`, and `lookup` for the four abstractions, and `instance`, `device`, `buffer`, `program`, `command`, and `shader_object` for the wrappers. A thunk call depends on the invariant of the wrapper that makes it, such as the open pass of a `ComputePass`, so it sits with that wrapper, as in `slang-compiler`. Moving every thunk call into one module would separate each call from the fields that its safety rests on.

`ffi_unwind_calls` carries an exception on each direct call of a C function, each inside the abort guard, as in `slang-compiler` (Section 4.4 of the coding standard).

## 5. Ownership

### 5.1. The Owning Interface Pointer

RHI-3. The crate MUST hold every interface of `slang-rhi-sys` with a reference in one crate-private owning interface pointer type, generic over the interface, as COM-1 of the coding standard requires, and MUST call `addRef` and `release` only there.

The type follows the owning interface pointer of `slang-compiler` (SLC-2 of the `slang-compiler` design) without its deferral of releases, because the deferral serves the reentrancy rule of Slang, which Section 8 meets through the guard of `slang-compiler` itself. The crate keeps its own type because the type of `slang-compiler` is private.

The wrappers offer no `from_raw`, `into_raw`, or `as_raw`, which COM-11 of the coding standard asks of every interface wrapper. Whether they get them is RHQ-5.

### 5.2. References Returned by Each Call

This table records, for each call of the compute path and of the rendering phase, whether the returned pointer carries a reference, as COM-2 of the coding standard requires. Section 3.3 of the sys design states where slang-rhi adds the reference of each owned return:

| Call | Returns | Owned or borrowed | Rust type |
| --- | --- | --- | --- |
| `rhiGetInstance` | `IRHI*` | Neither. A process-wide object that is never freed while the crate is in use (RHI-14) | Crate-private |
| `IRHI::getAdapter` | `IAdapter*` | Borrowed from the backend until `rhiDestroyInstance` | None. The crate copies the information into `Adapter` (Section 6.3) |
| `IRHI::createDevice` | `IDevice**` | Owned | `Device` |
| `IDevice::getSlangSession` | `slang::ISession**` | Owned | `slang_compiler::Session`, through `Session::from_raw` with the device's global session |
| `createShaderProgram`, `createComputePipeline`, `createBuffer`, `getQueue`, `createCommandEncoder`, `finish` | An interface through `**` | Owned | `ShaderProgram`, `ComputePipeline`, `Buffer`, `Queue`, `CommandEncoder`, `CommandBuffer` |
| `ICommandEncoder::beginComputePass` | `IComputePassEncoder*` | Borrowed from the encoder | `ComputePass<'e>`, which borrows the encoder mutably |
| `IComputePassEncoder::bindPipeline` | `IShaderObject*` | Borrowed from the pass until its next `bindPipeline` | `ShaderObject<'p>`, which borrows the pass mutably |
| `IShaderObject::getEntryPoint` | `IShaderObject**` | Owned, and valid only while its root object is | `ShaderObject<'p>`, which borrows its root object mutably |
| `IShaderObject::getObject` | `IShaderObject**` | Owned, and valid only while its parent object is | `ShaderObject<'p>`, which borrows its parent object mutably (RHI-22) |
| `IShaderObject::getElementTypeLayout` | `slang::TypeLayoutReflection*` | Borrowed from the program of the bound pipeline | Used only inside one call of the crate (Section 6.7) |
| `IDevice::getCapabilities`, `IDevice::hasFeature` | Values, through a buffer of the caller or as the result | Neither. No pointer is returned | Used only inside `create_slang_session` (RHI-30) |
| `IRHI::getCapabilityName` | `const char*` | Borrowed. A static string | Passed to `IGlobalSession::findCapability` inside `create_slang_session` (RHI-30) |
| `createTexture`, `ITexture::getDefaultView`, `createSampler`, `createInputLayout`, `createRenderPipeline` | An interface through `**` | Owned (Section 3) | `Texture`, which holds its default view, `Sampler`, and `RenderPipeline`, which holds its input layout |
| `ICommandEncoder::beginRenderPass` | `IRenderPassEncoder*` | Borrowed from the encoder | `RenderPass<'e>`, which borrows the encoder mutably |
| `IRenderPassEncoder::bindPipeline` | `IShaderObject*` | Borrowed from the pass until its next `bindPipeline` | `ShaderObject<'p>`, which borrows the pass mutably |
| `IDevice::getFormatSupport`, `ITexture::getSubresourceLayout`, `IDevice::readTexture` | Values, through a pointer of the caller | Neither. No pointer is returned | Used only inside `create_texture` and `read_texture` (RHI-23, RHI-28) |

RHI-4. Every wrapper of an owned interface MUST hold the `Device` that created it, as COM-5 of the coding standard requires, and a `Device` MUST hold the `slang_compiler::GlobalSession` that it was created with and its `slang_compiler::Session`. Every pointer that the table marks borrowed MUST be held only by a value whose lifetime borrows its owner as the table states.

A native child holds a strong reference to its device (Section 3.3 of the sys design), so the Rust parent field is not what keeps the device alive. It keeps the field order of COM-5 and makes the device reachable from every object for the checks of RHI-7 and Section 6.7. The borrows make the compiler enforce the validity rules of Section 3.3 of the sys design at no run-time cost, as SLC-19 of the `slang-compiler` design does for reflection handles.

## 6. Public API

### 6.1. Overview

The public items are at the crate root (API-10):

- The free function `adapters` and the type `Adapter`, with the enumerations `DeviceType` and `AdapterType`.
- `DeviceOptions`, `Device`, `DeviceInfo`, `DebugMessage`, `DebugMessageType`, and `DebugMessageSource`.
- `ShaderProgram`, `ComputePipeline`, and `CompileSession`.
- `BufferDesc`, `MemoryType`, `BufferUsage`, and `Buffer`.
- `Queue`, `CommandEncoder`, `ComputePass`, `ShaderObject`, `ShaderOffset`, and `CommandBuffer`.
- From the rendering phase of Section 11.2, `Format`, `TextureUsage`, `TextureDesc`, and `Texture`, and `FilterMode`, `AddressMode`, `SamplerDesc`, and `Sampler`.
- From the same phase, `VertexFormat`, `StepMode`, `VertexStream`, `BlendMode`, `CullMode`, `RenderPipelineDesc`, and `RenderPipeline`, and `ColourLoad`, `DepthLoad`, `RenderPassDesc`, and `RenderPass`.
- `Error`, `ErrorKind`, `Operation`, and the alias `Result`.

Each enumeration that mirrors an enumeration of slang-rhi follows SLC-14 of the `slang-compiler` design: it is `#[non_exhaustive]`, has one variant for each enumerator of the recorded slang-rhi in Rust case, and is converted by an explicit `match` in a crate-private function, fallibly with the kind `Unrecognized` from slang-rhi.

### 6.2. Usage Illustration

This illustration shows the compute path. It doubles the values of a buffer with a kernel whose module declares `RWStructuredBuffer<float> values;` and `uniform float factor;`. The crate documentation holds a runnable form of it (RHI-17):

```rust
use slang_compiler::{GlobalSession, Toggle};
use slang_rhi::{BufferDesc, BufferUsage, Device, DeviceOptions, DeviceType, MemoryType};

fn double(global_session: GlobalSession, linked_source: &str) -> slang_rhi::Result<Vec<f32>> {
    let mut options = DeviceOptions::new(global_session);
    options.device_type(DeviceType::Vulkan).validation(Toggle::Off);
    let device = Device::new(&options)?;

    let program = load_and_link(device.slang_session(), linked_source)?;
    let program = device.create_shader_program(&program)?;
    let pipeline = device.create_compute_pipeline(&program)?;

    let input: Vec<u8> = [1.0_f32, 2.0, 3.0, 4.0].iter().flat_map(|v| v.to_le_bytes()).collect();
    let desc = BufferDesc::new(16, BufferUsage::UNORDERED_ACCESS | BufferUsage::COPY_SOURCE)
        .with_element_size(4)
        .with_memory(MemoryType::DeviceLocal);
    let values = device.create_buffer(&desc, Some(&input))?;

    let queue = device.queue()?;
    let mut encoder = queue.encoder()?;
    encoder.compute_pass(|pass| {
        let mut root = pass.bind_pipeline(&pipeline)?;
        let factor = root.offset_of("factor")?.expect("the kernel declares factor");
        root.set_data(factor, &2.0_f32.to_le_bytes())?;
        let target = root.offset_of("values")?.expect("the kernel declares values");
        root.set_buffer(target, &values)?;
        drop(root);
        pass.dispatch([4, 1, 1])
    })?;
    queue.submit([encoder.finish()?])?;
    queue.wait_on_host()?;

    let mut output = vec![0_u8; 16];
    device.read_buffer(&values, 0, &mut output)?;
    Ok(output.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
}
```

In the illustration, `load_and_link` stands for the loading and linking of a module through the API of `slang-compiler` (Section 6.5 of the `slang-compiler` design).

### 6.3. Adapters

`adapters(device_type: DeviceType) -> Result<Vec<Adapter>>` asks `IRHI::getAdapter` for the positions 0, 1, and so on until it returns null, and copies the information of each adapter. `Adapter` is a `Clone` record with the accessors `device_type`, `adapter_type`, `name`, which returns `&str`, `vendor_id`, `device_id`, and `luid`, which returns `[u8; 16]`.

The name is the text of `AdapterInfo::name` (`include/slang-rhi.h:3076`) up to its first NUL byte, or all 128 bytes when it has none, converted with each invalid sequence replaced by U+FFFD. A copy holds no foreign pointer, so `Adapter` is `Send` and `Sync`, and an adapter name that is not UTF-8 does not make the list fail.

### 6.4. Devices and the Debug Callback

`DeviceOptions::new(global_session: slang_compiler::GlobalSession) -> DeviceOptions` takes the global session that the device compiles with. Its setters take and return `&mut Self` (API-6): `device_type(DeviceType)`, whose default is `DeviceType::Default`, `adapter(&Adapter)`, whose default is none, which lets slang-rhi choose, `validation(Toggle)`, whose default is `Toggle::Off`, and `debug_callback(impl Fn(&DebugMessage<'_>) + Send + Sync + 'static)`, whose default is none. The options hold the closure in an `Arc`, so one value of `DeviceOptions` can open several devices, each of which holds a clone of it.

`validation` turns the debug device of slang-rhi on or off (Section 3). It turns on no validation layer of the graphics API, and its documentation says so. Whether the crate offers that layer is RHQ-6.

RHI-5. `Device::new(options: &DeviceOptions) -> Result<Device>` MUST pass the global session of the options in `SlangDesc::slangGlobalSession`, and MUST NOT let slang-rhi create a global session of its own. With an adapter, it MUST check that its device type equals the device type of the options, or fail with the kind `InvalidArgument`, and MUST fetch the adapter again by its device type and position for the call, failing with `InvalidArgument` when the position no longer exists. It MUST set `DeviceDesc::pipelineCompilationMode` to `Serial`.

A global session of slang-rhi's own has no loader of `slang-glslang-sys` and emits SPIR-V that is not optimized, after an error through the debug callback (Section 3, Section 4.1). The program of a shader program must come from the device's session (RHI-7), and a global session that the caller holds is the one place where the caller's modules and the device meet.

`Serial` is the default of the header (`include/slang-rhi.h:3350`, `src/device.h:529-542`). The crate sets it explicitly, because with it slang-rhi calls Slang only on the thread of each call of the crate, which the Slang call guard and the collector of Section 8.3 rely on.

`Device` offers `info(&self) -> DeviceInfo`, a record of the device type, the name of the API, the name of the adapter, and the limits of RHI-29, `slang_session(&self) -> &slang_compiler::Session`, `create_slang_session` (RHI-30), and the methods of Sections 6.5 to 6.8. `Device` is `Clone`, and a clone adds a reference to the same device. `Device::new` reads the limits of RHI-29 once into `DeviceInfo`, and the check of `dispatch` reads the limit of the thread groups there (Section 6.7).

RHI-6. The crate MUST pass every device a heap-allocated `IDebugCallback` object of its own, with or without a closure of the user, whose `handleMessage` is defined `extern "system"` and stored in the generated field through a conversion of the function pointer, and MUST never free that object. `handleMessage` MUST give each message whose source is Slang to the collector of Section 8.3, and MUST build a `DebugMessage<'_>` that borrows the message for the call only and call the user's closure, when the device has one.

The object always exists, because on Vulkan the errors of Slang reach the crate only through the debug callback (Section 3), and RHI-7 returns them as diagnostics. Without the object, slang-rhi would use its `NullDebugCallback` and drop them. With validation on, the debug device receives the same object (`src/rhi.cpp:352`).

slang-rhi gives no notice when a native device is freed, and children keep the native device alive after every Rust wrapper is gone (Section 3.3 of the sys design), so no Rust value knows when the callback can no longer be called. A freed callback object that slang-rhi calls later is a use after free, while a leaked one costs one small allocation for each device, and the user's closure when there is one, which UNS-4 of the coding standard permits (RHQ-3). The object is never unregistered, so FFI-8 holds with the context alive for the life of the process.

The closure takes the bounds `Send + Sync + 'static`, which CONC-6 of the coding standard permits for a callback that a foreign library keeps: slang-rhi calls it from several threads at once and from threads of the driver (Section 3), and keeps it beyond any borrow. A panic of the closure reaches the non-unwinding boundary and aborts the process, as FFI-7 requires for a callback that reports no result code. The crate documentation states this.

`DebugMessage` offers `message_type(&self) -> DebugMessageType`, `source(&self) -> DebugMessageSource`, and `text(&self) -> Cow<'_, str>`, converted with each invalid sequence replaced by U+FFFD, because a message must not be lost to an error about its encoding. A message whose type or source the crate does not know has no variant to represent it, so the closure does not receive it. A message of Slang still reaches the collector.

### 6.5. Shader Programs and Compute Pipelines

RHI-7. `Device::create_shader_program(&self, program: &slang_compiler::LinkedProgram) -> Result<ShaderProgram>` MUST check that the program belongs to the session of the device, by comparing the raw pointer of the session that `LinkedProgram::session` returns with that of `slang_session`, and that the program has no specialization parameter left, and MUST fail with the kind `InvalidArgument` otherwise. It MUST pass the program as `slangGlobalScope` with no entry points and the linking style `SingleProgram`, pass and read the diagnostics out-parameter, and collect the messages of Slang during the call (Section 8.3).

slang-rhi takes the Slang session from the program and generates code for the target at index 0 of that session (`src/shader.cpp:58`, Section 3.3 of the sys design, Section 3). The device's session has exactly one target, the one that the device runs, so a program of that session gets code that the device runs. A program of any other session could get code of another target, which the device would run as if it were its own. From M10, a program of a session that reproduces the device session takes the same checks through `CompileSession::create_shader_program` instead (RHI-30). The crate adopts the session with a target count of 1. With no entry points, slang-rhi uses the linked program as it is (`src/shader.cpp:58-62`), so the program that the caller linked is the program that runs. `LinkedProgram::session` is an addition to `slang-compiler` (SLC-40 of the `slang-compiler` design).

The crate reads the specialization parameter count through `IComponentType::getSpecializationParamCount` of the linked program, inside the guards of Section 8, because `slang-compiler` offers no safe accessor for it. A count other than 0 is the condition under which slang-rhi creates a virtual pipeline and generates its code at the first dispatch (Section 3), outside the collector, where the crate could not return the diagnostics. The refusal names the count and says to specialize the program before it is linked.

`create_shader_program` also reads the stages of the program's entry points through the reflection of `slang-compiler`, for the check of `create_compute_pipeline`, so it fails with the kind `Slang` when that reflection fails.

A failure has the kind `Compile` and holds as its diagnostics the text of the out-parameter, when slang-rhi fills it, or else the collected messages of Slang. Warnings on success are dropped, because the result type carries no diagnostics. This answers RHQ-4 by a choice of the design of M10: the one user of the crate, the backend `rhi` of `slango`, returns no warning of Slang on either backend (SLO-42 of `docs/design/slango.md`), so a result type with warnings would carry data that no caller reads. Returning them later is an additive change of the result type of new methods, so the simplest answer costs nothing now.

`Device::create_compute_pipeline(&self, program: &ShaderProgram) -> Result<ComputePipeline>` MUST fail with the kind `InvalidArgument`, before any call into slang-rhi, when the shader program belongs to another device, when its linked program does not have exactly one entry point, whose stage is compute, or when the thread group of that entry point exceeds the limits of the device, along an axis or in its number of threads (RHI-29). Otherwise it creates the pipeline with the default compilation policy and collects the messages of Slang during the call.

slang-rhi aborts the process on a compute pipeline of a program without an entry point (Section 3), and a stage other than compute in a compute pipeline breaks valid use of Vulkan (Section 9.1). `createShaderProgram` generates no code (`src/vulkan/vk-device.cpp:2521-2538`). With the default policy and the pipeline compilation mode `Serial`, `createComputePipeline` generates the code of the program at this call (`src/device.cpp:755-769`, `src/shader.cpp:110`, `src/device.cpp:447`), so a failure of that compile has the kind `Compile` with the collected messages.

`create_shader_program` reads the thread group size of the one compute entry point through the reflection of `slang-compiler` and keeps it in the `ShaderProgram`, and `create_compute_pipeline` compares it with `max_compute_thread_group_size` and `max_compute_threads_per_group` of `DeviceInfo`, with a checked product of the three sizes, so an overflow is refused as well. The two messages name the axis and the limit, or the number of threads and the limit. slang-rhi checks neither limit without its debug device, and a dispatch of a pipeline over them broke the device so that every later read failed (observed through `slango` on 2026-11-18), so this is a check of RHI-20.

### 6.6. Buffers

`BufferDesc::new(size: u64, usage: BufferUsage) -> BufferDesc` creates a descriptor with the element size 0 and the memory type `MemoryType::DeviceLocal`, and `with_element_size(u32)` and `with_memory(MemoryType)` return changed copies. `Buffer` offers `size(&self) -> u64`, and keeps its usage for the checks of Section 6.7 without a public accessor.

`BufferUsage` is a set of flags of `bitflags` with eight flags: `VERTEX_BUFFER`, `INDEX_BUFFER`, `CONSTANT_BUFFER`, `SHADER_RESOURCE`, `UNORDERED_ACCESS`, `INDIRECT_ARGUMENT`, `COPY_SOURCE`, and `COPY_DESTINATION`. Each maps to a buffer usage bit of core Vulkan 1.0, which is valid on every device. The flags `ACCELERATION_STRUCTURE`, `ACCELERATION_STRUCTURE_BUILD_INPUT`, `SHADER_TABLE`, and `SHARED` of `rhi::BufferUsage` are left out, because each needs an extension or external memory that a plain compute device need not enable (Section 3).

RHI-8. `Device::create_buffer(&self, desc: &BufferDesc, initial: Option<&[u8]>) -> Result<Buffer>` MUST fail with the kind `InvalidArgument` when the size of the descriptor is 0, when its usage is empty or holds a bit outside `BufferUsage::all()`, when its memory type is `Upload` or `ReadBack` and its usage holds `UNORDERED_ACCESS`, or when `initial` is given and either its length differs from the size of the descriptor or the memory type is `ReadBack`. `Device::read_buffer(&self, buffer: &Buffer, offset: u64, out: &mut [u8]) -> Result<()>` MUST fail with `InvalidArgument` when the buffer belongs to another device, when its usage lacks `COPY_SOURCE`, or when `offset` plus the length of `out` exceeds the size of the buffer, with checked arithmetic, before it calls `readBuffer`.

`createBuffer` receives no length for the initial data (`include/slang-rhi.h:3456-3460`), and the Vulkan backend copies the size of the descriptor from it (`src/vulkan/vk-buffer.cpp:390-396`, `:443-447`, `:457`), so a short slice is a read out of bounds in native code. Without the check, creating a buffer of size 0 crashes the process, and a buffer without a usage breaks valid use of Vulkan.

`readBuffer` copies with `vkCmdCopyBuffer` (Section 3), which needs `COPY_SOURCE`, and writes the requested range into the caller's pointer (`include/slang-rhi.h:3704`). The range check of slang-rhi can wrap (RKB-1 in Section 3.1), and the crate checks the range itself in any case, as a safe interface does.

Initial data needs no `COPY_DESTINATION`, because slang-rhi adds the transfer bit itself (Section 3).

D3D12 allows no unordered access to `Upload` and `ReadBack` memory, and slang-rhi aborts the process on initial data for a `ReadBack` buffer on D3D12 (Section 3). The crate refuses both on every backend, so that a program behaves the same on Vulkan and on D3D12.

A `ReadBack` buffer therefore has no use through the crate: a shader cannot write it, it takes no initial data, the crate has no command that copies into it and does not map it, and on D3D12 `read_buffer` cannot read it, because slang-rhi creates it there without `CopySource` (Section 3). Its API documentation states this. Whether the checks should depend on the backend, or the crate should gain a copy command that gives such a buffer a use, is RHQ-10.

`create_buffer` does not check the size against the largest buffer that the device allows. Whether it does is RHQ-8.

### 6.7. Command Encoding and Parameters

`Queue` comes from `Device::queue(&self) -> Result<Queue>`, which takes the graphics queue, and offers `encoder(&self) -> Result<CommandEncoder>`, `submit(&self, command_buffers: impl IntoIterator<Item = CommandBuffer>) -> Result<()>`, and `wait_on_host(&self) -> Result<()>`.

RHI-9. `CommandEncoder::compute_pass<R>(&mut self, f: impl FnOnce(&mut ComputePass<'_>) -> Result<R>) -> Result<R>` MUST begin the pass, run `f`, and end the pass whether `f` succeeds, fails, or panics, and MUST NOT offer a pass value that outlives the closure. A panic of `f` MUST continue after the pass has ended. `CommandEncoder::finish(self) -> Result<CommandBuffer>` consumes the encoder.

A pass that is begun and never ended leaves the encoder in a state that slang-rhi does not document. A guard that ends the pass in `Drop` would break UNS-4, because a leaked guard skips `end`, so the closure-scoped form that UNS-4 recommends ends it on every path. The method runs `f` under `catch_unwind`, ends the pass, and resumes the panic, because a caller that catches the panic could otherwise keep using the encoder with a pass still open and call `finish` on it.

RHI-21. `Queue::submit` MUST take ownership of its command buffers, MUST fail with the kind `InvalidArgument`, before any call into slang-rhi, when one of them belongs to another device, and MUST release its own references to them after the native call.

slang-rhi recycles the native command buffer once the queue has run it (Section 3), so a command buffer that the caller could submit again would name an object that slang-rhi already reuses. Taking ownership makes a second submission, or a command buffer listed twice, a compile error. The queue keeps its own reference to each command buffer in flight, so the release after the call is sound. The command buffers are consumed even when the check fails, which their documentation states. `submit` passes a `SubmitDesc` with its command buffers and `..SubmitDesc::default()` for every other field.

`ComputePass` offers `bind_pipeline(&mut self, pipeline: &ComputePipeline) -> Result<ShaderObject<'_>>` and `dispatch(&mut self, thread_groups: [u32; 3]) -> Result<()>`. `bind_pipeline` fails with the kind `InvalidArgument` when the pipeline belongs to another device, and with `Failed` when slang-rhi returns null. It marks the pass as unbound before the native call and as bound only after a root object is returned, so after a failed bind the pass refuses to dispatch until a bind succeeds.

`dispatch` fails with `InvalidArgument` when no pipeline is bound in the pass, and when a count is larger than the limit of the device along its axis, which `Device::new` read and which is 65535 on D3D12 (Section 3). A count of 0 runs nothing and is valid.

`ShaderObject<'p>` offers `offset_of(&self, name: &str) -> Result<Option<ShaderOffset>>`, `set_data(&mut self, offset: ShaderOffset, data: &[u8]) -> Result<()>`, `set_buffer(&mut self, offset: ShaderOffset, buffer: &Buffer) -> Result<()>`, `entry_point(&mut self, index: usize) -> Result<ShaderObject<'_>>`, and `object` (RHI-22).

`ShaderOffset` is a `Copy` record with private fields: the uniform offset, the uniform size, the binding range index, the binding range count, and the binding array index. It offers `uniform_offset`, `uniform_size`, `binding_range_index`, and `binding_array_index`, each returning a `u32`, and `with_binding_array_index(self, u32) -> Self`. Only `offset_of` creates one, and it has no `Default`, because no default offset names a parameter. A caller therefore cannot forge an offset, and every offset describes a parameter that the layout of a program declares.

RHI-10. `offset_of` MUST find a field of the element type layout of the object by name as `ShaderCursor::getField` does (Section 3): the field index through `spReflectionTypeLayout_findFieldIndexByName`, the uniform offset of the field's variable layout, and the binding range through `spReflectionTypeLayout_getFieldBindingRangeOffset`, with `None` for a name that the layout does not have. It MUST also read the uniform size and the binding range count of the field's type layout. It finds a field of the top level of the object only.

The offsets of `offset_of` are those that `ShaderCursor` finds, because the element type layout of a root object is the struct of the global parameters (Section 3). Unlike `ShaderCursor`, `offset_of` does not search the entry points when a name is missing at the root. A parameter of an entry point is found through `entry_point`.

RHI-11. Before each call, `set_data` MUST check that the parameter of the offset has uniform data, that the length of `data` does not exceed the uniform size of the parameter, and that the uniform offset plus that length does not exceed the uniform size of the element type layout. `set_buffer` MUST check that the parameter of the offset has exactly one binding range, that the binding range index is below the binding range count of the object, that the array index is below the binding count of that range, that the range is a raw buffer and not a typed buffer, that the usage of the buffer suits the access of the range, and that the buffer belongs to the device of the pass. Each check MUST fail with the kind `InvalidArgument`.

The header states no check of these bounds by slang-rhi, and a write past the uniform data of an object or into a range that does not exist is an access out of bounds in native code. These are the checks of an index that a safe interface needs, so they stay whatever slang-rhi checks (Hard Rule 14 of `AGENTS.md` for the analogous case of Slang). The checks against the parameter's own size and range stop an offset of one parameter, or of another object, from writing into the next parameter, such as an offset of a `float` that names the binding range of the next buffer.

A mutable raw buffer range needs `UNORDERED_ACCESS`, and a read-only raw buffer range needs `SHADER_RESOURCE` or `UNORDERED_ACCESS`, because both give the storage buffer bit that such a range needs (Section 3). A typed buffer range, such as one of `RWBuffer<float>`, needs a buffer with a format, and the crate creates no buffer with a format, so it refuses the range with that reason.

A parameter with several binding ranges is a structure of several resources, such as a structure with a `ConstantBuffer` and a buffer. Its fields cannot be addressed by name, because `offset_of` finds top-level parameters only, so `set_buffer` and `object` refuse it instead of using its first range.

A `ConstantBuffer` or `ParameterBlock` parameter has a uniform size of 0 at the top level, so `offset_of` finds it and `set_data` refuses it. Its fields are set on the object that `object` returns (RHI-22).

`set_buffer` builds a `Binding` of `slang-rhi-sys` with the type of a buffer, the buffer's interface pointer in `resource`, a null `resource2`, and the range of the whole buffer, which is the offset 0 and the size `u64::MAX` (Section 3), and passes it by pointer while the `Buffer` that it borrows keeps its own reference (Section 6.4 of the sys design).

A buffer parameter that the kernel uses and that has no buffer bound reads a null descriptor. On D3D12, slang-rhi binds a null descriptor, which reads zeros and drops writes (Section 3). On Vulkan, it works only on a device whose driver offers the feature `nullDescriptor`. The documentation of `set_buffer` and `dispatch` states this limit, and the crate does not check it. Whether it should is RHQ-7.

### 6.8. Errors

RHI-12. Every failure of the crate MUST be its one type `Error`, which is `Send`, `Sync`, and `'static` and holds an `ErrorKind`, an `Operation` that names the call, the raw result code when slang-rhi returned one, the text of diagnostics when there were any, and, for a failure of `slang-compiler`, that error as its `source()`. One crate-private function MUST map every result code to an `ErrorKind`, as COM-7 of the coding standard requires.

| Kind | Failure |
| --- | --- |
| `InvalidArgument` | A check of the crate failed, or slang-rhi returned `SLANG_E_INVALID_ARG` |
| `NoDevice` | `createDevice` failed, such as without a Vulkan loader or driver |
| `Compile` | `createShaderProgram`, `createComputePipeline`, `createInputLayout`, or `createRenderPipeline` failed with or without diagnostics |
| `OutOfMemory` | `SLANG_E_OUT_OF_MEMORY` |
| `Unsupported` | `SLANG_E_NOT_IMPLEMENTED` or `SLANG_E_NOT_AVAILABLE`, or a device whose session `create_slang_session` cannot reproduce (RHI-30) |
| `Failed` | Any other failure code that the mapping knows, such as `SLANG_FAIL`, and a null return where the API promises an object |
| `Unrecognized` | A result code or an enumerator that the recorded slang-rhi does not list |
| `Slang` | A failure of `slang-compiler`, such as the adoption of the device's session or the reflection of a program |

`ErrorKind` and `Operation` are `#[non_exhaustive]`, and `Operation` names each call, such as `GetObject` for `object` and `CreateSlangSession` for `create_slang_session`. The rendering phase adds `CreateTexture`, `ReadTexture`, `CreateSampler`, `SetTexture`, `SetSampler`, `CreateRenderPipeline`, `BeginRenderPass`, `BindRenderPipeline`, `SetVertexBuffers`, `SetIndexBuffer`, and `Draw`. `BindRenderPipeline` is apart from `BindPipeline`, whose text names a compute pipeline. The `Display` text names the operation and the kind and holds no advice (ERR-4). `Error` holds the error of `slang-compiler` in a `Box`, so that the type stays below the size at which `clippy::result_large_err` fires.

## 7. Threading

RHI-13. No type of the crate that holds a pointer of slang-rhi or a wrapper of `slang-compiler`, which are `DeviceOptions`, `Device`, `ShaderProgram`, `ComputePipeline`, `Buffer`, `Queue`, `CommandEncoder`, `ComputePass`, `ShaderObject`, and `CommandBuffer`, and from M10 `CompileSession`, `Texture`, `Sampler`, `RenderPipeline`, `RenderPass`, and `RenderPassDesc`, MUST be `Send` or `Sync`. `Adapter`, `DeviceInfo`, `BufferDesc`, `ShaderOffset`, the enumerations, `BufferUsage`, and `Error`, and from M10 the other value types of Section 11, MUST be `Send` and `Sync`. Each auto trait that a type implements gets a compile-time assertion test (CONC-1).

`RenderPassDesc<'t>` is a value type that borrows its target textures, so it is neither `Send` nor `Sync`. The crate has no public `TextureView` type, because each `Texture` holds its default view, which is the only view that the crate uses. `Texture`, `Sampler`, `RenderPipeline`, and `RenderPass` each have two `compile_fail` doctests for the missing traits, and `RenderPass` a third, which shows that it cannot leave its closure.

slang-rhi documents no threading rule for its objects (Section 3.3 of the sys design), so FFI-9 of the coding standard forbids `Send` and `Sync` for them. The wrappers of `slang-compiler` that a device holds are neither (SLC-21 of the `slang-compiler` design). The `rhi` backend of `slango` therefore cannot make its public types `Send` and `Sync` on this crate, and the owner decided on 2026-11-18 that `slango` gives up both traits on both backends in version 0.2 (Section 4.6 of `docs/design/slango.md`).

The types that M10 adds follow the same rule, as RHI-13 lists them.

RHI-14. The crate MUST make every call through the `IRHI` instance, which are `rhiGetInstance`, `getAdapter`, `createDevice`, and the `getCapabilityName` of `create_slang_session`, while it holds one crate-private `static` `Mutex<()>`, MUST check the per-thread guard of Section 8.2 before it takes the lock, and MUST NOT call `rhiDestroyInstance`.

The instance is one object of the process that every thread reaches, so a type that is not `Send` does not keep two threads apart, and slang-rhi states no rule for it. The lock makes the calls of this crate on it one at a time. A call of `rhiDestroyInstance` while a device lives aborts the process (Section 3.3 of the sys design), and the crate cannot know whether a device lives, so it never calls it, and the instance lives until the process exits.

CONC-8 of the coding standard permits the lock, as an exception to the ban on library statics of CONC-4. The lock guards no data, so a poisoned lock is recovered with `PoisonError::into_inner`. A debug callback that called `adapters` or `Device::new` during `createDevice` would wait for the lock that its own thread holds, so the check of the per-thread guard comes first and panics instead. The lock holds within one version of the crate only: two versions of the crate in one build each get their own `static`, while the `links` key of `slang-rhi-sys` allows one copy of slang-rhi.

Objects of two devices are used from two threads at once when a program opens a device on each thread. A reading of slang-rhi found that the state that two such devices share is synchronized, apart from the `IRHI` instance:

- The count of live devices is atomic (`src/rhi.h:51`), and the backends sit behind a mutex (`src/rhi.h:52`).
- The adapters of a backend are filled under a `once_flag` (`src/backend.h:24-32`).
- Block allocators lock a mutex (`src/core/block-allocator.h:178`), and the global task pool has a mutex (`src/core/task-pool.cpp:506`).
- Device creation reads the debug layer options of the instance (`src/vulkan/vk-device.cpp:1537`) and the capability names (`src/slang-context.h:55`), and runs under the lock of RHI-14.

The `gpu` test `devices_on_two_threads_run_the_compute_path_at_once` runs the whole compute path on two threads at once. The reading covers the Vulkan backend and the shared code only, and a test can show a race but not prove its absence.

## 8. Guards Around Native Calls

### 8.1. The Abort Guard and the Slang Call Guard

RHI-15. Every call of the crate into slang-rhi, and every call of a reflection function of `slang-compiler-sys` or of a method of Slang, MUST run inside the per-thread guard of Section 8.2, inside `slang_compiler::with_slang_call_guard` (SLC-41 of the `slang-compiler` design), and inside the crate-private abort guard as the innermost frame around that one call, as FFI-7 of the coding standard requires.

slang-rhi calls Slang, through which a C++ exception can escape (Section 6.4 of the sys design), and an exception that reaches a destructor of the crate while it unwinds would call native code again in the middle of the unwinding. The abort guard stops the process first, as the guard of `slang-compiler` does (Section 11.6 of the `slang-compiler` design).

slang-rhi calls Slang on the device's session and on programs that `slang-compiler` wraps, and it calls the debug callback, whose closure can call `slang-compiler`, while it is inside Slang. A call of Slang from inside Slang is reentrant, which Slang documents as unsupported. With the Slang call guard of `slang-compiler` raised for the duration of each call into slang-rhi, such a call of the closure into `slang-compiler` panics, which aborts at the boundary of the callback, instead of reentering Slang. The guard also defers the releases that `slang-compiler` would make during the call (SLC-28 of the `slang-compiler` design).

The abort guard of this crate is its own crate-private function, because that of `slang-compiler` is private and frozen (Technology Decisions of `AGENTS.md`). `check-slang-call-guard` checks the guard of `slang-compiler` only, so the placement of this guard is a judgement rule until a tool covers it, as Hard Rule 4 of `AGENTS.md` allows, and whether the checker is extended to this crate is RHQ-1.

### 8.2. The Per-Thread Guard

RHI-18. The crate MUST keep a per-thread mark in a `thread_local!` that is set while a call of the crate into slang-rhi runs on the thread. A call MUST panic before it reaches slang-rhi when the mark is already set, and the panic message MUST name the native function or the public function of the crate that was called.

`with_slang_call_guard` raises the Slang call guard but checks nothing, so a debug callback that reached a `Device` through storage of its own could call slang-rhi again from inside slang-rhi. slang-rhi documents no reentrancy, and the debug callback runs while `ShaderProgram::compileShaders` holds a `std::mutex` (Section 3), so a reentrant call could lock that mutex a second time, which is undefined behaviour. The panic happens inside the debug callback and aborts the process at its non-unwinding boundary.

The guard upholds this invariant, which CONC-4 of the coding standard requires the design to state: no thread is inside two calls of this crate into slang-rhi at once. A value that a call holds clears the mark when it is dropped, so the mark clears on every return and every unwinding.

A leaked value leaves the mark set, after which every later call on that thread panics, which is sound. The guard holds within one version of the crate only, because two versions of the crate in one build each get their own `thread_local!`.

### 8.3. The Collector of Diagnostics

RHI-19. The crate MUST keep the messages of Slang that the debug callback receives during `createShaderProgram` and `createComputePipeline` in a per-thread collector in a `thread_local!`, and MUST return them with the result of that call only.

The collector upholds this invariant: a message of Slang that the debug callback receives on a thread belongs to the call of `create_shader_program` or `create_compute_pipeline` in progress on that thread, and to no other call. The collector is set before the call and taken after it, and a message that arrives while no such call is in progress is dropped. A message that a thread of the driver reports does not reach the collector, which is sound, because with the pipeline compilation mode `Serial` slang-rhi calls Slang on the calling thread (RHI-5). The collector holds within one version of the crate only, as the per-thread guard does.

## 9. Soundness Invariants

### 9.1. Valid Use of the Graphics API

RHI-20. Before each call into slang-rhi, the crate MUST check every condition of valid use of the graphics API that a safe call of its API could otherwise break and that the crate can check from the values that it holds, and MUST fail with the kind `InvalidArgument` when one does not hold.

slang-rhi checks nothing unless its debug device is on (Section 3), and the debug device checks slang-rhi's own rules, not every condition of valid use. A call that breaks valid use has undefined behaviour by the specification of the graphics API, so a safe call must not reach the driver with one. These checks keep the safe interface within valid use, as the bounds checks of RHI-11 do, and they are not guards against defects of slang-rhi under Hard Rule 14 of `AGENTS.md`.

The checks of this kind are the usage checks of RHI-8 and RHI-11, the size 0 of RHI-8, the refusal of unordered access to `Upload` and `ReadBack` memory and of initial data for `ReadBack` memory of RHI-8, the refusal of a typed buffer range, the limit of `dispatch`, the single compute entry point of RHI-7 and the limits of its thread group, and the ownership of `submit` (RHI-21). The rendering phase adds these checks of Section 11.2:

- The size, format, and usage of a texture, its initial data, and on Vulkan the support of `D32Float` as a depth target (RHI-23).
- A texture parameter that is a plain two-dimensional texture with elements of floating-point numbers, and a sampler parameter that is not a comparison sampler, in compute passes and render passes alike (RHI-25).
- The entry points and formats of a render pipeline, the count, stride, location, and semantic of its vertex streams, and a stream for each vertex input that the vertex entry point declares, of the numeric kind and the semantic of that input (RHI-26).
- The targets of a render pass, the formats of the bound pipeline against them, the vertex and index buffers of each draw against its counts, and a texture that a pass both samples and draws into (RHI-27).
- The texture and the output of `read_texture` (RHI-28).

Conditions that the crate does not check yet are RHQ-7, RHQ-8, and RHQ-12. The crate also does not refuse a buffer that a draw both reads as a vertex or index buffer and writes through a read-write parameter, whose contents that draw reads are undefined. The higher crate `slango` refuses that case itself (SLO-44 of `docs/design/slango.md`). The documentation of `set_buffer`, `set_vertex_buffers`, and `set_index_buffer` states this.

### 9.2. Invariants

These are the invariants on which the unsafe code of the crate relies, each with the requirement that keeps it:

| Invariant | Kept by |
| --- | --- |
| Every interface pointer that the crate dereferences points to a live object | The owning interface pointer (RHI-3) and the borrows of the unowned pointers (RHI-4) |
| No pass encoder or root object is used after its owner ends or rebinds it | The mutable borrows of `ComputePass`, `RenderPass`, and `ShaderObject`, and the closures of RHI-9 and RHI-27 |
| Every pointer and length passed to slang-rhi is in bounds | RHI-8, RHI-11, RHI-23, and RHI-28 |
| Every object that a `Binding` names lives through the call | The borrow of the `Buffer`, `Texture`, or `Sampler` in `set_buffer`, `set_texture`, or `set_sampler`, and the default view that each `Texture` holds |
| Every vertex buffer, index buffer, and pipeline of a draw lives until slang-rhi retains it in `setRenderState` | The references that `RenderPass` holds (RHI-27) |
| No pass samples a texture that it draws into | The record of the textures of a pass (RHI-27) |
| No command buffer is submitted after slang-rhi has recycled it | RHI-21 |
| Every call stays within valid use of the graphics API, as far as the crate can check | RHI-20 |
| The debug callback object lives for every call that slang-rhi makes to it | RHI-6 |
| No object is used from two threads at once | RHI-13 and RHI-14 |
| No thread enters slang-rhi again from inside a call of slang-rhi | RHI-18 |
| No C++ exception unwinds through the crate, and no panic unwinds into slang-rhi | RHI-15, and the non-unwinding callback of RHI-6 |
| Every component that a shader program takes comes from the device's session or from the session of a `CompileSession` of the device | RHI-7 and RHI-30 |
| Every `getObject` reads an existing entry of a range of sub-objects that has a layout | The checks of `object` (RHI-22) |
| The Slang session of a program of a `CompileSession` lives while any native object made from it does | The session that `ShaderProgram` holds, and the native reference of the root layout of slang-rhi (Section 3) |

## 10. Tests

### 10.1. Layout and Tiers

The crate has unit tests in its modules and one integration-test binary, `tests/rhi/`, whose module `gpu` holds the tests that need an adapter (ARCH-22 of the architecture design). The recipe `test-rhi` runs them with the nextest profile `gpu`, which the CI job `vendored` runs on Linux on lavapipe and on Windows on a D3D12 device on WARP, and `check-miri-rhi` runs the unit tests under Miri (TOOL-52 and Section 7.4 of `docs/design/tooling.md`). The crate has no `ffi` tier of its own, because every call of the crate into slang-rhi needs a device, which needs an adapter.

### 10.2. The Fake for Miri

RHI-16. The unit tests MUST test, under Miri, the owning interface pointer, the closures of RHI-9 and RHI-27 with a panic of `f`, the per-thread guard and the collector, the bounds and usage checks of RHI-8, RHI-11, RHI-20, RHI-22, and RHI-23 to RHI-28, the choice of the target, profile, and macro of RHI-30, the building of `Binding`, the release after `submit`, and the mapping of result codes, against a fake of the interfaces `IBuffer`, `IComputePassEncoder`, `IRenderPassEncoder`, `IRenderPipeline`, `IShaderObject`, `ICommandBuffer`, and `ICommandQueue` built on the generated vtable structs, with its vtable functions defined `extern "system"`.

TEST-7 of the coding standard asks for a fake of the interfaces, and the fake of `slang-compiler` is the model (SLC-25 of the `slang-compiler` design). The fake covers the interfaces whose counting and borrowing the unsafe code relies on. Its queue records the command buffers of each submission and holds a reference to each until it is freed, as slang-rhi does. Its shader object answers `getObject` from a list of objects keyed by binding range and array index, holds a reference to each, and writes null with `SLANG_OK` for a missing entry, as slang-rhi does. Its render pass records the binds, the render states, and the draws, shares its log with its root objects, and creates a new root object on each `bindPipeline`, as slang-rhi does (Section 3), so a unit test checks that a texture bound on a root object stays in the record of the pass after a rebind and that a draw then refuses it as a target. Its render pipeline exists because a bind holds an owned pipeline. A fake of `IDevice`, `IRHI`, and `ICommandEncoder` would need a function for each of their many slots and would test only that the crate passes descriptors, which the `gpu` tests do on a real device. The bounds checks are pure functions over the numbers that the reflection calls return, so they run under Miri without the reflection of Slang (UNS-2).

### 10.3. GPU Tests

The `gpu` tests run on an adapter of the device type of the platform, which is Vulkan on Linux and D3D12 on Windows. In CI, that is lavapipe on Linux and WARP on Windows, and locally the development container's NVIDIA RTX 4070 or lavapipe. Each test fails, and never skips, when it finds no adapter (ARCH-23). The module holds 59 tests, 20 in `gpu.rs`, 26 in `gpu/render.rs`, 7 in `gpu/objects.rs`, and 6 in `gpu/compile_session.rs`, and they cover:

- `adapters` for the device type of the platform returns at least one adapter, and a device opens on each. An adapter of another device type than the options is refused.
- A device of the type `Default` opens on the graphics API of the platform.
- The compute path of Section 6.2 doubles a buffer with optimized code and no message of the debug callback.
- Uniform parameters at distinct offsets, buffers in distinct ranges, and a uniform parameter of an entry point, set through `entry_point(0)`, land where `offset_of` finds them. A name of an entry point is not found at the root.
- A shader program from a program of another session, offsets past the uniform data, past the binding ranges, or of another object, and initial data of the wrong length fail with `InvalidArgument`.
- A kernel that does not generate code for the device fails with `Compile` and holds the diagnostics, also on a device without a debug callback, which keeps working afterwards.
- A program without a compute entry point, and a program with a specialization parameter left, are refused before slang-rhi compiles them.
- Devices on two threads run the compute path at once (Section 7).
- A panic in a compute pass ends the pass, and the encoder still finishes.
- The objects of a device keep it alive after every `Device` handle is dropped.
- A submitted dispatch runs exactly once.
- Buffer descriptions of size 0, without a usage, or with an unknown usage are refused, each use of a buffer needs its usage flag, `UNORDERED_ACCESS` in `Upload` or `ReadBack` memory and initial data for `ReadBack` memory are refused, and a buffer in `Upload` memory with `SHADER_RESOURCE` and `COPY_SOURCE` and initial data reads back correctly.
- A typed buffer parameter is refused, and the buffer stays unchanged.
- A dispatch past the limit of the device is refused, and a dispatch of 0 groups runs nothing.
- A kernel whose thread group exceeds the device is refused by `create_compute_pipeline`, and the device keeps working.
- The limits of `DeviceInfo` are at least the minimums that both Vulkan and D3D12 guarantee, and the size of a buffer is above 0 (RHI-29).
- The fields of a `ConstantBuffer`, a `cbuffer`, and a `ParameterBlock` are set through `object`, and `set_data` and `set_buffer` on the parameter, `object` past its array, and `object` on a buffer or a uniform field are refused. Only the kernel of the `ParameterBlock` has a global uniform, which avoids RKB-2 on D3D12.
- Nested parameter blocks and constant buffers are reached one object at a time, without a uniform field beside the nested `ConstantBuffer`, once with the debug device off and once with it on. A third test has a uniform field beside it, which has the shape of RKB-2 one level down, and it passed on WARP (Section 10.4).
- A structure of several resources is refused by `object` and by `set_buffer`.
- A `CompileSession` gives the same layouts and code as the device session for the test shaders of `slango` and a kernel with a matrix in a `ConstantBuffer` and a `ParameterBlock` and a branch on the macro of the backend, and a session without the device's settings gives a different result (RHI-30).
- A shader program of a `CompileSession` loads a module by name from a search path, sees the macro of the backend, and runs when the linked program and the `CompileSession` are dropped before the pipeline is created and the shader program before the dispatch.
- Each `create_shader_program` refuses a program of another session, and a search path with a NUL byte fails with the error of `slang-compiler`.

The tests of `gpu/render.rs` cover the rendering phase of Section 11.2:

- The conventions of Section 3: an asymmetric triangle lands with clip space y up and the top row first, with the debug device off and on, counter-clockwise triangles face the front and `Back` culls the others, and the depth test `Less` keeps the nearest fragment.
- A draw with indices takes the vertices that its indices name, and an index past the end of the vertex buffer reaches the device, which keeps working (Section 3).
- A texture sampled with point filters at `SV_Position` gives each texel its quarter of the target.
- A pass with only a depth target clears it and refuses every draw.
- A stream that steps per instance moves each instance, and `Alpha` blends the colour over the target.
- A stream of stride 0 over a buffer of one `float4` gives each of 6 vertices and 3 instances that one attribute, with the debug device on and no message of the debug callback.
- A stream at a location that the shader does not declare, and an input that the shader declares and does not read, which takes a stream, leave the other inputs matched.
- A stream with fewer or more components than its input reads 0 for a missing second or third component and 1 for a missing fourth, and drops the extra ones.
- A triangle outside the depth range is clamped on Vulkan and clipped on D3D12 (RKB-9).
- A texture of each of the four colour formats with initial data reads back without the padding of its rows, at the sizes 3 by 2, 1 by 1, and 67 by 5, and a colour texture created from zeroed data reads back as zeros.
- Each refusal of RHI-23 to RHI-28 is asserted with its kind and operation, among them every refused vertex stream and every refused texture or sampler parameter, a texture that a pass both samples and draws into, and 13 refused draws after which the pass still ends.
- A panic in a render pass ends the pass, and the encoder still finishes.

Each test that draws asserts that no error message reached the debug callback, and a failed creation of a pipeline prints the error with its diagnostics.

The tests compile their kernels with the global session of `slang-compiler`, so they also show that the loader of `slang-glslang-sys` serves a device of slang-rhi.

### 10.4. What the Tests Do Not Cover

The tests run under the debug device of slang-rhi only, and none runs under the validation layer of Vulkan or the debug layer of D3D12, which the crate does not turn on (RHQ-6).

On 2026-11-18 and 2026-11-19, a review ran the 25 tests of `gpu/render.rs` by hand under the Khronos validation layer, which the Vulkan loader inserted, on lavapipe and on the NVIDIA RTX 4070. Before its fixes, the layer reported `VUID-vkCmdDraw-format-10390` for a stride that is not a multiple of 4, `VUID-VkGraphicsPipelineCreateInfo-Input-07904` and `VUID-VkGraphicsPipelineCreateInfo-Input-08733` for vertex inputs without a matching stream, `VUID-vkCmdDraw-viewType-07752` and `VUID-RuntimeSpirv-samples-08726` for texture parameters of other shapes, `VUID-vkCmdDraw-format-07753` for a texture parameter of integer elements, and `VUID-vkCmdDraw-None-06479` for a comparison sampler. The checks of RHI-25 and RHI-26 now refuse each of these. After the fixes, the 25 tests gave no error, and the one message was the performance warning `WARNING-Shader-OutputNotConsumed` for the stream at a location that the shader does not declare.

These are not verified:

- Valid use of the other `gpu` tests under the Khronos validation layer, and valid use on D3D12 under its debug layer.
- A Vulkan device without the feature `depthClamp`, on which every render pipeline of slang-rhi breaks valid use (RKB-9, RHQ-12).
- A draw into a target of a BGRA or sRGB format, linear filters, and the contents of a texture without initial data, which are undefined on Vulkan and which the crate documentation states.
- A hardware D3D12 adapter, ARM64 Windows, and a program on Windows without `dxcompiler.dll` (Section 7.3 of the sys design).
- A device whose Vulkan driver lacks `nullDescriptor` (RHQ-7).
- What slang-rhi and a driver do with a buffer larger than the device allows (RHQ-8).
- Arrays of `ParameterBlock` parameters.
- The path of a failed rebind, after which slang-rhi returns no root object, which was read in the source and not run.
- A message of the debug callback from a thread of the driver.
- That the SPIR-V that the device runs is optimized, beyond the absence of a message about `spirv-opt`.

In CI run 44388142812 of 2026-11-18, `test-rhi` ran all 143 tests of both crates on WARP and passed them, at the code of the compute phase of Section 11.1 before the limits of the thread group. So on D3D12 a device of the type `Default` opens, `create_slang_session` chooses a target and profile that the device runs, RHA-8 holds with DXIL, and the objects of RHI-22 and the limits of RHI-29 behave as on Vulkan, including the nested test with a uniform field.

In CI run 44407500494 of the same day, `test-rhi` ran all 148 tests of both crates on WARP and passed them, with the limits of the thread group. Among them, the `gpu` test of those limits reads both limits from the D3D12 device, links a kernel over each of them, and creates its shader program, which succeeds. `create_compute_pipeline` then refuses each kernel with the kind `InvalidArgument` and the message that names the limit, so the check of RHI-7 runs on D3D12.

The test generates no code before that call, so the DXIL compiler does not see the oversized kernel first, as it does in a load of `slango` (Section 7.6 of `docs/design/slango.md`). The run does not print the values of the two limits.

CI run 44423418363, for the first commit of the rendering phase, failed on WARP in the test of an input that the shader declares and does not read, which led to the rule of RHI-26 (Section 3). In CI run 44431645707, for the code of the rendering phase with the fixes of its review, `test-rhi` ran all 225 tests of both crates on WARP and passed them. So on D3D12, the conventions of Section 3, the counts of components, a stream at a location that the shader does not declare, an input that the shader declares and does not read, the read-back with the row pitch of D3D12, and the clipping of a triangle outside the depth range behave as Sections 3 and 11.2 state.

In CI run 44474920321 of 2026-11-19, for the rendering step of `slango`, `test-rhi` ran all 226 tests of both crates on WARP and passed them, among them the stream of stride 0 of RHI-26.

## 11. Additions for M10

M10 adds to the crate what the backend `rhi` of `slango` needs (Sections 7.6 to 9.7 of `docs/design/slango.md`), in the two phases of the owner's decision 3 of Section 4.6 of that design. Every addition keeps the rule of RHI-20: the crate checks every condition of valid use that a safe call could break and that it can check, before the native call, because slang-rhi checks nothing without its debug device.

### 11.1. The Compute Phase

RHI-30. `Device::create_slang_session(&self, search_paths: &[&Path]) -> Result<CompileSession>` MUST create a `slang_compiler::Session` from the global session of the device, with exactly the settings with which slang-rhi creates the device session: the one target of the device's compile target and profile, `forceGLSLScalarBufferLayout` on that target of either format, the row-major matrix layout, the macro of the backend, one `Capability` option for each capability of the device, and every other setting of `src/slang-context.h`, which the crate reproduces, and with the given search paths. `CompileSession` MUST hold its `Device` and its session, MUST offer `session(&self) -> &slang_compiler::Session`, and MUST offer `create_shader_program(&self, program: &slang_compiler::LinkedProgram) -> Result<ShaderProgram>`, which MUST check that the program belongs to its session, by comparing the raw pointer of the session of the program with that of its own, and MUST then do what `Device::create_shader_program` does under RHI-7.

The backend `rhi` of `slango` creates a session of its own for each load (owner decision 6 of Section 4.6 of `docs/design/slango.md`), because one device session for all loads fails a reload of a changed file and reuses stale imported modules (Section 3.11 of that design). slang-rhi takes the session from the program and generates code for target 0 of that session (`src/shader.cpp:58`), so a program of a session with the device's target and settings gets the code that the device session would give. Only the check of RHI-7 against the device session refuses it.

The `CompileSession` holds its session alive, so the comparison of raw pointers cannot match a later session at the same address. A program of any other session still fails with the kind `InvalidArgument`.

A `CompileSession` keeps its session, and the modules that it loads, until the `CompileSession`, every program of it, and every shader program created from them are dropped, so a program that creates a session for each load releases each load's Slang objects when it drops them. A `ShaderProgram` of a `CompileSession` holds the session and releases its native program first. A pipeline, shader object, or deferred compilation made from it keeps the session through the native reference of slang-rhi (Section 3), so dropping a `CompileSession` and its shader programs early is sound, and `ComputePipeline` holds no session of its own.

The reproduction follows the settings of Section 3. It reads the capabilities through `IDevice::getCapabilities` in the order of slang-rhi, takes each name from `IRHI::getCapabilityName` under the lock of RHI-14, skips a null name, which slang-rhi never returns for a capability of a device, and sets each identifier from `IGlobalSession::findCapability` as a `Capability` option.

On D3D12, it chooses DXIL with the profile `sm_X_Y` of the highest feature `SM_6_0` to `SM_6_10` that `IDevice::hasFeature` reports, rather than the raw shader model of the device. A D3D12 device below shader model 6.0 compiles to DXBC, for which `slang-compiler` has no target format, so `create_slang_session` fails there with the kind `Unsupported`, as it does for every device type other than Vulkan and D3D12.

Settings that slang-rhi repeats from its source, rather than reporting them, need the reproduction changed when a newer slang-rhi changes them, which the test of RHA-8 shows.

The `gpu` test of RHA-8 compared the layouts and the code of every entry point of the test shaders of `slango` and of a kernel with a `float3x4` in a `ConstantBuffer` and in a `ParameterBlock` and a branch on the macro of the backend, byte for byte, and found them equal on Vulkan on the NVIDIA RTX 4070 and on lavapipe, and with DXIL on WARP in CI run 44388142812. This confirmed the former assumption RHA-8 on both backends. Without the capability options, with a column-major matrix layout, without the scalar layout, or with another macro, the code differs. A review compared 13 further shaders on both adapters with the same result.

RHI-22. `ShaderObject::object(&mut self, offset: ShaderOffset) -> Result<ShaderObject<'_>>` MUST return the object of a `ConstantBuffer`, `cbuffer`, or `ParameterBlock` parameter through `IShaderObject::getObject`. Before the call, it MUST check that the parameter of the offset has exactly one binding range, that this range is a range of sub-objects of the object and that the array index is below the binding count of that range, and MUST fail with the kind `InvalidArgument` otherwise. A null object from `getObject` MUST give an error of the kind `Failed` instead of a value, because `getObject` itself checks neither the kind of the range nor the array index and returns a null entry for some ranges (`src/shader-object.cpp:121-130`). The returned value MUST own its reference and borrow its parent mutably, as the value of `entry_point` does.

`offset_of`, `set_data`, `set_buffer`, and the setters of Section 11.2 on the returned object find and set the fields of the element type of the parameter under RHI-10 and RHI-11, so a constant buffer and a parameter block, nested to any depth, are reached one object at a time. The crate-private kind of a binding range gains a kind for a range of sub-objects, beside the kinds of buffers, and `set_buffer` keeps refusing that kind. The kind covers the binding types `CONSTANT_BUFFER`, which a `cbuffer` also has, and `PARAMETER_BLOCK` only.

A range of `ExistentialValue` is refused, because slang-rhi creates no object for it, and so is a range of `PushConstant`, because slang-rhi gives it no sub-object on Vulkan (Section 3). The parameter itself has no uniform data, and its fields are set through its object. Like `set_buffer`, `object` refuses an offset whose parameter has several binding ranges (Section 6.7).

slang-rhi creates the object of each such parameter, at the root and below it, when the pipeline is bound, so `getObject` returns it without a prior `setObject` (Section 3). The `gpu` tests of `object` at the root and two levels down show this on Vulkan. On D3D12, a binding range that is a root parameter by `D3D12DeviceExtendedDesc::rootParameterShaderAttributeName` gets no sub-object index (`src/d3d12/d3d12-shader-object-layout.cpp:96-140`), and the crate never sets that name, whose default is null (`src/d3d12/d3d12-device.h:41`).

So the null object that RHI-22 turns into the kind `Failed` arises only for a range without a layout, which the crate refuses before the call. Under the debug device of slang-rhi, which `DeviceOptions::validation` turns on, a missing object would be wrapped in a live object of the debug device, so `object` cannot report it there. That is the one limit of `object` under the debug device, and it has no effect while the crate refuses, before the call, every range for which slang-rhi leaves the object missing. The documentation of `object` states this limit.

RHI-29. `DeviceInfo` MUST offer the limits of the device that the checks of the crate use, which are the maximum thread group counts of a dispatch, the maximum size of a thread group along each axis, the maximum number of threads of a thread group, the maximum size of a buffer, the maximum width and height of a two-dimensional texture, the maximum number of vertex streams, and the maximum number of vertex input elements, read once by `Device::new` from `DeviceInfo::limits` of slang-rhi (`maxComputeDispatchThreadGroups`, `maxComputeThreadGroupSize`, `maxComputeThreadsPerGroup`, `maxBufferSize`, `maxTextureDimension2D`, `maxVertexStreams`, and `maxVertexInputElements` of `include/slang-rhi.h`).

`slango` runs the checks of a dispatch, a buffer, and a draw before it calls the crate, with the same limits, so that a mistake gets the text and the kind of `slango` on both of its backends (SLO-39 and SLO-40 of `docs/design/slango.md`). The crate itself checks the thread group of a compute pipeline against its two limits (RHI-7). The seven accessors are `max_compute_dispatch_thread_groups`, `max_compute_thread_group_size`, which returns `[u32; 3]`, `max_compute_threads_per_group`, which returns `u32`, `max_buffer_size`, `max_texture_dimension_2d`, `max_vertex_streams`, and `max_vertex_input_elements`, after the fields of slang-rhi.

The six limits other than the size of a buffer come from the driver on Vulkan and are constants of the API on D3D12, which are 1024 threads for each group and 1024, 1024, and 64 along the three axes for a thread group. `maxBufferSize` is the 2 GiB that slang-rhi assumes on both backends, not a value of the device (Section 3), and the documentation of `max_buffer_size` says so. The crate does not check the size of a buffer against it (RHQ-8).

A kernel or a render pipeline with a `ConstantBuffer` and a global `uniform` fails on D3D12 (RKB-2). On Vulkan, an array of `ConstantBuffer` parameters is bound as if it had one element (RKB-5), and a kernel with a push-constant buffer aborts the process at its dispatch (RKB-6). The crate does not guard against these defects (Section 3.1), and its crate documentation states them among the limits of the crate.

### 11.2. The Rendering Phase

The rendering phase covers two-dimensional textures with one mip level, one layer, and one sample, samplers, render pipelines with a vertex and a fragment entry point, render passes with one colour target and an optional depth target, and the reading of a texture. Every call that it makes takes its descriptors by pointer or reference, and none is a placeholder slot of `slang-rhi-sys` (RSYS-10 of the sys design), so the rendering phase needs no change of the generator (Section 6.2 of the sys design).

The crate writes every field of the raw structs `TextureDesc`, `SamplerDesc`, `RenderPassColorAttachment`, `InputElementDesc`, `VertexStreamDesc`, `SubresourceData`, and `SubresourceLayout` without `..T::default()`, using the C++ default for each field that it does not choose. The first three have no `Default`. Unit tests compare the values that it writes into them with the defaults in the documentation of the generated fields.

RHI-23. `Device::create_texture(&self, desc: &TextureDesc, initial: Option<&[u8]>) -> Result<Texture>` MUST create a two-dimensional texture with one mip level, one layer, and one sample in `DeviceLocal` memory. It MUST fail with the kind `InvalidArgument` before the native call when a width or height is 0 or above the limit of RHI-29, when the usage is empty or holds a bit outside `TextureUsage::all()`, when a depth format has a usage other than `DEPTH_STENCIL`, when a colour format has `DEPTH_STENCIL`, when `initial` is given for a depth format, or when the length of `initial`, computed with checked arithmetic, is not the width times the height times the 4 bytes of a pixel. On a Vulkan device, it MUST also fail with `InvalidArgument` when `getFormatSupport` does not report `DepthStencil` for `D32Float` as the format of a depth target. A `Texture` MUST hold its default view, from `ITexture::getDefaultView`, and offers `size`, `format`, and `usage`.

The crate asks for the support of a format only there, which corrects the first design, which asked wherever slang-rhi exposes it. Every other format and use of the crate is mandatory on both graphics APIs, and so is a `D32Float` depth target on D3D12. `getFormatSupport` reports that format wrongly on D3D12 (RKB-7) and the copy bits wrongly on Vulkan (RKB-8), so a query there would refuse valid textures.

`TextureDesc::new(size: [u32; 2], format: Format, usage: TextureUsage)` is the crate's own descriptor. `Format` is a `#[non_exhaustive]` enumeration of `Rgba8Unorm`, `Rgba8UnormSrgb`, `Bgra8Unorm`, `Bgra8UnormSrgb`, and `D32Float`, the formats that `slango` uses, and `TextureUsage` is a set of flags of `bitflags` with `SHADER_RESOURCE`, `RENDER_TARGET`, `DEPTH_STENCIL`, `COPY_SOURCE`, and `COPY_DESTINATION`, with the values of the header. A wider set waits for a user that needs it.

`createTexture` reads as many bytes from the initial data as the descriptor implies, so a short slice is a read out of bounds in native code, which the length check prevents, as RHI-8 does for buffers. The initial data is one `SubresourceData` whose row pitch is the width times 4 and whose slice pitch is its length, with rows from the top, and slang-rhi copies each row with the smaller of the two row pitches (Section 3). Initial data reaches the texture through `createTexture`, so the placeholder slot `uploadTextureData` is not needed.

On Vulkan, a texture created without initial data holds undefined contents until a render pass clears it or draws into it, and the documentation of `create_texture`, `ColourLoad::Load`, and `DepthLoad::Load` states this. A `D32Float` texture takes no initial data, so a pass with that texture as its only target clears it.

RHI-24. `Device::create_sampler(&self, desc: &SamplerDesc) -> Result<Sampler>` MUST create a sampler from the crate's own `SamplerDesc`, whose fields are the filter of minification and magnification, the mip filter, and the address mode of each axis, and MUST write the other fields of the raw descriptor as the reduction `Average`, the comparison `Never`, an anisotropy of 1, a bias of 0, a level of detail from 0 to 1000, and a border colour of zero.

`SamplerDesc::new` and its builders `with_min_filter`, `with_mag_filter`, `with_mip_filter`, and `with_address_modes` follow `BufferDesc`. `FilterMode` has `Point` and `Linear`, and `AddressMode` has `ClampToEdge`, `Wrap`, and `MirrorRepeat`, the modes that `slango` uses, without `ClampToBorder` and `MirrorOnce`. A sampler descriptor has no value to refuse.

The filters of the crate's `SamplerDesc` default to `Point`, and its address modes to `ClampToEdge`, the defaults of wgpu, because the defaults of slang-rhi, `Linear` and `Wrap`, differ from them, and `slango` sets every field anyway (Section 8.3 of `docs/design/slango.md`).

RHI-25. `ShaderObject::set_texture(&mut self, offset: ShaderOffset, texture: &Texture) -> Result<()>` and `ShaderObject::set_sampler(&mut self, offset: ShaderOffset, sampler: &Sampler) -> Result<()>` MUST check, before the call, every condition of RHI-11 that does not concern buffers, that the binding range is a read-only texture range or a sampler range, that the texture has the usage `SHADER_RESOURCE`, and that the texture or sampler belongs to the device of the object. A texture range MUST also be a two-dimensional texture without array layers or several samples, whose elements are floating-point numbers of 16 or 32 bits, and a sampler range MUST NOT be a comparison sampler. They MUST pass a `Binding` of the type `Texture` with the default view, or of the type `Sampler` with the sampler, while the borrowed value keeps its own reference through the call.

A combined texture and sampler, such as `Sampler2D`, and a texture range that a shader writes stay refused, because `slango` rejects both (SLO-24 of `docs/design/slango.md`).

The crate creates only two-dimensional textures with one layer and one sample, of formats that a shader reads as floating-point numbers, and no comparison sampler. A draw that read such a texture through a parameter of another shape broke valid use of Vulkan. The Khronos validation layer reported `VUID-vkCmdDraw-viewType-07752` for a three-dimensional, cube, array, or one-dimensional parameter and `VUID-RuntimeSpirv-samples-08726` for a multisample parameter, and so did a parameter of integer elements, `VUID-vkCmdDraw-format-07753`, and a comparison sampler, `VUID-vkCmdDraw-None-06479`. The crate reads the shape and the result type of the leaf type of the binding range through `spReflectionTypeLayout_getBindingRangeLeafTypeLayout`, and knows a comparison sampler by the type name `SamplerComparisonState`. The checks run in compute passes and render passes alike, and only a render pass records the textures that are set (RHI-27).

RHI-26. `Device::create_render_pipeline(&self, program: &ShaderProgram, desc: &RenderPipelineDesc) -> Result<RenderPipeline>` MUST check, before any native call, that the program belongs to the device and has exactly one vertex and one fragment entry point, that the colour format is a colour format and the depth format, when there is one, is `D32Float`, and that the vertex streams number at most the limit of vertex streams of RHI-29 and at most 16, the size of the arrays of `RenderState`, which the public constant `MAX_VERTEX_STREAMS` holds. Each stream MUST have a stride from 0 to 2048 that is a multiple of 4, a semantic name that is not empty and holds no NUL byte, and a location of its own, and the highest location plus one MUST be at most the limit of vertex input elements of RHI-29. For each varying input of the vertex entry point, it MUST check that a stream sits at its location, even when the shader does not read the input, that the input is a scalar or vector of 32-bit floating-point numbers, unsigned integers, or signed integers, which a vertex format feeds, that the format of the stream has the same numeric kind, and, when the input has a semantic, that the stream has the same semantic name, with ASCII case ignored, and the same semantic index. Each check MUST fail with the kind `InvalidArgument`. It MUST create the input layout and the render pipeline with a list of triangles, counter-clockwise front faces, one colour target, and the depth test `Less` with depth writes when the descriptor has a depth format, and MUST collect the messages of Slang during the creation, as RHI-7 does for a compute pipeline.

The crate's `RenderPipelineDesc` holds the colour format, a blend of `Replace` or `Alpha`, an optional depth format, a cull mode of `None` or `Back`, and the vertex streams. Each vertex stream holds one attribute at offset 0, with its format, its stride, its step per vertex or per instance, its location, and its semantic name and index. `VertexStream::new(format, location)` gives the stride of the format, the step per vertex, and no semantic, and `with_stride`, `with_step`, and `with_semantic` change them. `VertexFormat` has 12 formats: one to four components of `f32`, of `u32`, and of `i32`.

A stride of 0 gives every vertex, or every instance, the one attribute at the start of the buffer. It is valid on both graphics APIs: the stride of a vertex binding of Vulkan has only the upper bound `maxVertexInputBindingStride`, slang-rhi sets static binding strides on Vulkan (`src/vulkan/vk-device.cpp:2493-2498`), and `StrideInBytes` 0 is valid on D3D12, where slang-rhi passes the stride through (`src/d3d12/d3d12-command.cpp:902`, `src/d3d12/d3d12-device.cpp:1717-1720`). The bound check of RHI-27 holds a buffer of one attribute for any count at stride 0, and nothing divides by the stride. The backend `rhi` of `slango` feeds a vertex attribute that the shader declares and does not use from such a stream (Section 7.7 of `docs/design/slango.md`).

Under the Khronos validation layer with synchronization validation, on lavapipe and on the NVIDIA RTX 4070, the stream of stride 0 gave no error, and its `gpu` test passed on WARP in CI run 44474920321.

`create_shader_program` reads the varying inputs of the vertex entry point from reflection and keeps them in the `ShaderProgram`, with the name, location, semantic, and type of each. The fields of a structure become inputs of their own, and system values and uniforms, which have no varying input, are skipped. The rule on these inputs is the same on both backends, for these reasons:

- On D3D12, DXIL keeps a declared input that the shader does not read, and Slang removes it from SPIR-V (Section 3). The crate cannot tell a used input from a declared one without a code generation pass before slang-rhi compiles the pipeline, so it requires a stream for every declared input.
- Before the rule, the Khronos validation layer reported `VUID-VkGraphicsPipelineCreateInfo-Input-07904` and `VUID-VkGraphicsPipelineCreateInfo-Input-08733` for a declared input without a stream and for a stream of integers that fed an input of floating-point numbers, and `VUID-vkCmdDraw-format-10390` for a stride that is not a multiple of 4, the size of the 32-bit numbers of every vertex format.
- The format of a stream may have fewer or more components than its input, as both graphics APIs allow. The shader reads 0 for a missing second or third component and 1 for a missing fourth one, and does not read the components past those of the input.
- A stream without a semantic name is refused, because the D3D12 backend copies a null semantic name into its input layout and matches inputs by semantic (`src/d3d12/d3d12-device.cpp:1699-1709`).

The crate builds the input layout from the streams by these rules:

- The Vulkan backend gives each input element the location of its index (Section 3), so on Vulkan the crate lists one element for each location from 0 to the highest location of a stream. A location without a stream, which the shader does not declare, gets a filler element of the format `R32Float` that reads the first 4 bytes of each element of the first stream, within its bounds, with the semantic name `SLANG_RHI_UNUSED` and the location as its semantic index.
- The D3D12 backend matches each element by its semantic, so on D3D12 the input layout holds no filler element.
- A stream that steps per vertex has the instance data step rate 0, which D3D12 requires for per-vertex data, and a stream that steps per instance has the rate 1, because the field has no default and D3D12 needs 1 for one value per instance (`src/d3d12/d3d12-device.cpp:1714`).
- With no stream, the pointer to the input layout is null, which both backends accept.
- A pipeline without a depth format turns depth writes off and gives the depth format `Undefined`. `RenderPipeline` holds its input layout, and a failure of `createInputLayout` or `createRenderPipeline` has the kind `Compile`.

These checks replace the former assumption RHA-6, which expected a matched layout from fillers for unused locations on both backends, and which the test of an unused input on WARP refuted (Sections 3 and 11.2).

`Alpha` sets the colour blend to the source alpha and its inverse with the operation `Add`, and the alpha blend to one and the inverse source alpha, which is wgpu's `BlendState::ALPHA_BLENDING`. With the pipeline compilation mode `Serial`, slang-rhi generates the code of the program at this call (RHI-5), so a failure of that compile has the kind `Compile` with the collected messages, as for a compute pipeline.

RHI-27. `CommandEncoder::render_pass<R>(&mut self, desc: &RenderPassDesc<'_>, f: impl FnOnce(&mut RenderPass<'_>) -> Result<R>) -> Result<R>` MUST begin the pass, run `f`, and end the pass on every path, including a panic of `f`, as RHI-9 requires of a compute pass. Before the pass begins, it MUST check that every texture of the descriptor belongs to the encoder's device, that the pass has a colour target or a depth target, that a colour target has a colour format and `RENDER_TARGET`, that a depth target has `D32Float`, `DEPTH_STENCIL`, and, beside a colour target, the size of the colour target, and that a clear depth is from 0 to 1. Before each draw, `RenderPass` MUST check that a pipeline is bound whose colour format and depth format equal those of the pass, that a buffer with `VERTEX_BUFFER` of the device is set for each vertex stream of the pipeline and holds the count of vertices or instances at its stride, that a buffer with `INDEX_BUFFER` of the device that holds the count of indices is set for a draw with indices, and that no texture set on an object of the pass with `set_texture` is a target of the pass. Each check MUST fail with the kind `InvalidArgument`. The pass MUST keep one record of the textures that `set_texture` set during the pass, which every `ShaderObject` of the pass writes to, including the objects that `object` returns, so that the last check sees a texture set on a constant buffer or a parameter block as well.

The crate's `RenderPassDesc` holds an optional colour target with a load of its contents or a clear to a colour, and an optional depth target with a load or a clear to a depth, and at least one of the two. A pass with a depth target alone and no draw is how `slango` clears a new depth texture (SLO-45 of `docs/design/slango.md`), and a draw in such a pass fails the check of the colour format of its pipeline. `RenderPass` offers `bind_pipeline`, which returns the root object of the pipeline as `ComputePass::bind_pipeline` does, `set_vertex_buffers`, `set_index_buffer`, `draw(vertex_count, instance_count)`, and `draw_indexed(index_count, instance_count)`. Each draw sets the render state first: one viewport and one scissor rectangle over the whole colour target with the depth range 0 to 1, the vertex buffers, and the index buffer with 32-bit indices.

A texture that a pass both samples and draws into breaks valid use of Vulkan and D3D12, and slang-rhi does not check it, so the crate does. The crate does not read the values of an index buffer, so an index past the end of a vertex buffer reaches the driver, which bounds the fetch: slang-rhi enables `robustBufferAccess` on every Vulkan device (Section 3).

The pass as built adds these details:

- The store operation of every target is `Store`, and the load is a load or a clear, as the descriptor states.
- A buffer of a stream holds `count` elements when (`count` - 1) times the stride plus the size of the format fits its size, with checked arithmetic. The count is the vertex count for a stream that steps per vertex in a draw without indices, and the instance count for a stream that steps per instance. In a draw with indices, a stream that steps per vertex needs only a buffer, because the indices decide which vertices it reads.
- An index buffer holds the count of indices times 4 bytes.
- `set_vertex_buffers` takes at most 16 buffers, each of the device with `VERTEX_BUFFER`, and a failure leaves the buffers of the pass unchanged. A clear depth that is not a number is refused, and so is a target wider or taller than 65535 texels, which keeps the viewport exact as an `f32`.
- The record of sampled textures is checked at each draw, so `set_texture` with a target succeeds and the draw that follows fails.
- `bind_pipeline` reports `Operation::BindRenderPipeline`.
- The pass holds its own reference to each vertex buffer, the index buffer, and the bound pipeline, because a caller can drop a `Buffer` between `set_vertex_buffers` and the draw, while slang-rhi retains them only in `setRenderState` (Section 3).
- On Vulkan, a primitive outside the depth range 0 to 1 is drawn with its depth clamped, and on D3D12 it is clipped (RKB-9). The documentation of `RenderPass` and `RenderPipelineDesc` states this difference, and the crate does not guard against it.

RHI-28. `Device::read_texture(&self, texture: &Texture, out: &mut [u8]) -> Result<()>` MUST check, before the native call, that the texture belongs to the device, has a colour format and `COPY_SOURCE`, and that the length of `out` is the width times the height times the size of a pixel. It MUST read the layout of the texture with `ITexture::getSubresourceLayout` with the row alignment `kDefaultAlignment` of the bindings, and the texture with `IDevice::readTexture` into a buffer of the crate of the `sizeInBytes` of that layout, because `readTexture` copies `sizeInBytes` bytes into it (`src/device.cpp:942`), and copy each row into `out` without the padding of the row pitch.

`readTexture` takes pointers only, so it needs no placeholder slot. The row pitch of slang-rhi is aligned to the platform, 256 bytes on D3D12 (Section 3), so the crate removes the padding, and `out` holds the rows from the top, as `slango` returns them (Section 8.1 of `docs/design/slango.md`). When the layout that `getSubresourceLayout` reports does not describe the texture or does not fit the bytes read, `read_texture` fails with the kind `Failed`.

### 11.3. Not Wrapped in M10

These parts of slang-rhi stay unwrapped:

- Surfaces, which no window of M10 needs (Section 4.6 of `docs/design/slango.md`).
- Fences, mapped buffers, timestamp queries, other queues, several command buffers in one submission with fences, and the persistent caches.
- Copy and clear commands, indirect draws, and every other method of a placeholder slot (RSYS-10 of the sys design).
- Buffers with a format and the typed buffer ranges that need them, textures of other shapes, arrays, mip levels, and samples, textures of integer elements, and combined textures and samplers.
- Comparison samplers, the address modes `ClampToBorder` and `MirrorOnce`, store operations other than `Store`, and the support of formats other than `D32Float` on Vulkan.
- Lookup of nested fields and of array elements by name, specialization arguments of shader objects, and shader objects that the program does not create.
- Native handles, interoperation with other APIs, and acceleration structures, which M14 needs.

A user who needs one of them reaches it through `slang-rhi-sys` under its safety contract.

### 11.4. Tests of the Additions

The additions follow the tiers of Section 10:

- The fake for Miri gains `IRenderPassEncoder` and `IRenderPipeline`, so that the unit tests cover the closure of RHI-27 with a panic of `f`, a recorded draw, refusals before any call, and the record of sampled textures across binds, and the pure checks of RHI-22 to RHI-28 run under Miri as those of RHI-11 do (RHI-16). The crate has 116 unit tests.
- The `gpu` tests cover `create_slang_session` with the comparison of the layouts and code of both sessions (Section 11.1), a shader program of a `CompileSession`, and the refusal of a program of another session by both `create_shader_program` methods (RHI-30).
- The `gpu` tests cover a kernel whose thread group exceeds the limits of the device, and four unit tests cover the check of RHI-7 for each limit and for an overflow of the number of threads.
- The `gpu` tests on lavapipe and WARP cover the fields of a `ConstantBuffer`, a `cbuffer`, and nested parameter blocks set through `object`, and the 26 tests of the rendering phase that Section 10.3 lists.
- The test of the fields of a `ConstantBuffer` checks that `object` sets them instead of `set_data`, in a kernel without a global `uniform`, which avoids RKB-2 on D3D12.
- Unit tests compare the fields that the crate writes into `TextureDesc`, `SamplerDesc`, and `RenderPassColorAttachment` with the C++ defaults that the generated documentation of each field states. The test of `RenderPassColorAttachment` is the unit test of the crate-private function `colour_attachment`, which builds it.

## 12. Crate Documentation

RHI-17. The crate-level documentation MUST state the compute path with a runnable example of the `gpu` tier marked `no_run`, that the crate always builds Slang and slang-rhi from source and what that build needs, the platforms of Section 7 of the sys design with the need of a program on Windows for `dxcompiler.dll`, that no type that holds a native object is `Send` or `Sync` and why, that the debug callback object of a device is never freed, that a panic in the debug callback or a C++ exception that escapes slang-rhi aborts the process, and a `# Features` section that says that the crate has no Cargo features. The methods of the compute path point to that example instead of repeating it.

From the rendering phase, the crate-level documentation also states the path of a draw, the textures and pipelines that the crate creates, render passes in its section on ownership and threads, the undefined contents of a texture without initial data on Vulkan, the depth clamping of Vulkan (RKB-9), and what the crate does not cover.

## 13. Security Considerations

The threat model has four parts:

1. A program passes shader source that an attacker controls through `slang-compiler` to a device, and parameter values and buffer contents that the attacker controls to a dispatch.
2. A program calls the API in an order or with values that its author did not foresee, such as a range past the end of a buffer.
3. slang-rhi and the graphics driver run inside the program's process and report messages through the debug callback from threads of the driver.
4. The debug callback runs the user's code inside a call of slang-rhi.

Unsafe code and FFI invariants. Section 9 lists what the crate relies on and what keeps it. The crate guarantees its callers that no safe call reaches undefined behaviour, on the reading of Section 7 for devices on several threads and within the conditions of valid use that Section 9.1 checks.

Untrusted shader source and file paths. The crate passes no path or source to Slang itself. Shader source reaches Slang through `slang-compiler`, with the considerations of Section 17 of the `slang-compiler` design. A shader that an attacker controls runs on the GPU with the buffers that the program binds, and can read and write them.

Build scripts that run native builds. The crate has no build script. Its dependencies build Slang and slang-rhi, as Section 11 of the sys design states.

Dependencies. The crate depends on `slang-rhi-sys`, `slang-compiler-sys`, `slang-compiler`, and `bitflags`, whose versions the workspace fixes.

These attacks are out of scope:

- A malicious graphics driver or Vulkan loader, which runs with the full privileges of the program.
- A shader that hangs the GPU or reads data that the program bound to it, which is the program's choice of what to bind.
- Attacks on the repository itself, which are a matter of access control.

The residual risk has five parts:

1. slang-rhi documents no threading rule, so the soundness of devices on several threads rests on a reading of its Vulkan backend (Section 7).
2. A defect of slang-rhi or of the driver can corrupt memory of the process, and slang-rhi aborts the process on a failed assertion in every build type (Section 3.3 of the sys design).
3. A panic of the user's debug callback, and a call from it into `slang-compiler` or into this crate while slang-rhi is inside a call, abort the process (Sections 6.4 and 8).
4. The crate leaks the debug callback object of each device (RHI-6).
5. The conditions of valid use that the crate does not check, which are RHQ-7, RHQ-8, and RHQ-12, can reach the driver.

## 14. Operational Considerations

### 14.1. Cargo Features

The crate has no Cargo features. It always builds the vendored Slang and slang-rhi (RHI-1).

### 14.2. Environment Variables

The crate reads no environment variable. Its dependencies read those of Section 12.2 of the sys design and of `slang-compiler`.

### 14.3. Platforms and Minimum Supported Rust Version

The crate builds where `slang-rhi-sys` builds, which is Linux with the GNU C library with the Vulkan backend and x86_64 Windows with the Microsoft toolchain with the D3D12 backend (RSYS-13 of the sys design). It is tested on x86_64 Linux on lavapipe and on an NVIDIA adapter, and on x86_64 Windows on WARP. A program on Windows that creates a pipeline needs `dxcompiler.dll` on its `PATH` or beside its executable, which the Windows SDK ships and which the program provides, by the owner's decision of 2026-11-18 (Section 4.1 of the sys design). The crate documentation states this. It uses the workspace's `rust-version`.

### 14.4. Failure Modes

| Failure | Reported by | Recovery |
| --- | --- | --- |
| No Vulkan loader, driver, or adapter on Linux, or no D3D12 adapter on Windows | `ErrorKind::NoDevice` from `Device::new` | Install the Vulkan loader and a driver, such as lavapipe, or use a Windows with D3D12 |
| No `dxcompiler.dll` on Windows | `ErrorKind::Compile` from `create_compute_pipeline` or `create_render_pipeline`, by a reading of the source (Section 7.3 of the sys design) | Put the directory of `dxcompiler.dll` of the Windows SDK on `PATH`, or copy the DLL beside the executable |
| An adapter of another device type, or one that slang-rhi no longer lists | `ErrorKind::InvalidArgument` from `Device::new` | Pass an adapter of `adapters` for the device type of the options |
| A program of another session | `ErrorKind::InvalidArgument` from `create_shader_program` | Load and link the program through `Device::slang_session`, or through the session of a `CompileSession` and its own `create_shader_program` (RHI-30) |
| A program with a specialization parameter left | `ErrorKind::InvalidArgument` from `create_shader_program` | Specialize the program before it is linked |
| A program without exactly one compute entry point, or a shader program of another device | `ErrorKind::InvalidArgument` from `create_compute_pipeline` | Link one compute entry point on the same device |
| A shader that does not compile for the device | `ErrorKind::Compile` with the diagnostics | Fix the shader source that the diagnostics name |
| A buffer of size 0, without a usage, or with an unknown usage | `ErrorKind::InvalidArgument` from `create_buffer` | Give a size and the usage flags of each use |
| `UNORDERED_ACCESS` in `Upload` or `ReadBack` memory, or initial data for `ReadBack` memory | `ErrorKind::InvalidArgument` from `create_buffer` | Use `DeviceLocal` memory for a buffer that a shader writes, and `Upload` or `DeviceLocal` memory for initial data |
| A read of a buffer without `COPY_SOURCE` | `ErrorKind::InvalidArgument` from `read_buffer` | Create the buffer with `COPY_SOURCE` |
| A buffer without the usage that its range needs, a typed buffer range, a parameter without a binding range, or a structure of several resources | `ErrorKind::InvalidArgument` from `set_buffer` | Add `UNORDERED_ACCESS` or `SHADER_RESOURCE`, declare a structured or raw buffer, use the offset of a buffer parameter, or declare each resource as a parameter of its own or inside a `ConstantBuffer` or `ParameterBlock` |
| A parameter that is not a `ConstantBuffer`, `cbuffer`, or `ParameterBlock`, an array index past its range, or a structure of several resources | `ErrorKind::InvalidArgument` from `object` | Use the offset of a constant buffer or parameter block within its array |
| A device that is neither Vulkan nor D3D12 of shader model 6.0 or later | `ErrorKind::Unsupported` from `create_slang_session` | Use the device session through `Device::slang_session` |
| A parameter without uniform data, or data longer than the parameter | `ErrorKind::InvalidArgument` from `set_data` | Use the offset of a uniform parameter and data of its size |
| A parameter or a range out of bounds | `ErrorKind::InvalidArgument` | Use the offsets that `offset_of` returns and the size of the buffer |
| A dispatch without a bound pipeline, or past the limit of the device | `ErrorKind::InvalidArgument` from `dispatch` | Bind a pipeline, and split the work into dispatches within the limit |
| A command buffer of another device | `ErrorKind::InvalidArgument` from `submit` | Submit each command buffer to a queue of its own device |
| A texture with a width or height of 0 or above the limit of the device, without a usage, with an unknown usage, of a depth format with a usage other than `DEPTH_STENCIL`, or of a colour format with `DEPTH_STENCIL` | `ErrorKind::InvalidArgument` from `create_texture` | Give a size within `max_texture_dimension_2d` and the usage flags that suit the format |
| Initial data for a depth format, or of a length other than the width times the height times 4 | `ErrorKind::InvalidArgument` from `create_texture` | Give no initial data for a depth texture and clear it with a render pass, and give the data of every pixel otherwise |
| A Vulkan device that does not support `D32Float` as a depth target | `ErrorKind::InvalidArgument` from `create_texture` | Draw without a depth target, or use another adapter |
| A read of a texture of another device, of a depth format, without `COPY_SOURCE`, or into an output of a length other than the width times the height times 4 | `ErrorKind::InvalidArgument` from `read_texture` | Read a colour texture with `COPY_SOURCE` into an output of its size |
| A subresource layout that does not describe the texture | `ErrorKind::Failed` from `read_texture` | None in the program. The layout comes from slang-rhi |
| A texture or sampler of another device, a range that does not take a texture or a sampler, or a texture range that a shader writes or that is combined with a sampler | `ErrorKind::InvalidArgument` from `set_texture` or `set_sampler` | Bind each texture and sampler of the device to a `Texture2D` and a separate `SamplerState` |
| A texture parameter of another shape than a plain two-dimensional texture, such as a `Texture3D`, `TextureCube`, `Texture2DArray`, `Texture1D`, or `Texture2DMS` | `ErrorKind::InvalidArgument` from `set_texture` | Declare the parameter as a `Texture2D` |
| A texture parameter whose elements are not floating-point numbers of 16 or 32 bits, such as a `Texture2D<uint4>` | `ErrorKind::InvalidArgument` from `set_texture` | Declare the parameter with elements of `float` or `half` |
| A texture without `SHADER_RESOURCE` | `ErrorKind::InvalidArgument` from `set_texture` | Create the texture with `SHADER_RESOURCE` |
| A comparison sampler parameter, a `SamplerComparisonState` | `ErrorKind::InvalidArgument` from `set_sampler` | Declare the parameter as a `SamplerState` |
| A shader program of another device, or without exactly one vertex and one fragment entry point | `ErrorKind::InvalidArgument` from `create_render_pipeline` | Link one vertex and one fragment entry point on the same device |
| A depth format as the colour format, or a depth format other than `D32Float` | `ErrorKind::InvalidArgument` from `create_render_pipeline` | Give a colour format for the colour target and `D32Float` for the depth target |
| More vertex streams than the device allows or than 16, a stride above 2048 or not a multiple of 4, a semantic that is missing, empty, or holds a NUL byte, two streams at one location, or a highest location past the limit of vertex input elements | `ErrorKind::InvalidArgument` from `create_render_pipeline` | Give each input its own stream with a semantic and a stride that is a multiple of 4 |
| A vertex input that the vertex entry point declares and that no stream feeds, even one that the shader does not read | `ErrorKind::InvalidArgument` from `create_render_pipeline` | Give a stream at the location of every declared input, or remove the input from the shader |
| A vertex input of a type that no vertex format feeds, such as a `double` or a matrix | `ErrorKind::InvalidArgument` from `create_render_pipeline` | Declare the input as a scalar or vector of `float`, `uint`, or `int` |
| A stream whose numeric kind differs from that of its input, such as `Uint32x2` for a `float2` | `ErrorKind::InvalidArgument` from `create_render_pipeline` | Give the stream a format of the numeric kind of the input |
| A stream whose semantic name or index differs from that of its input | `ErrorKind::InvalidArgument` from `create_render_pipeline` | Give the stream the semantic that the shader declares, in any ASCII case |
| A render pipeline that slang-rhi or the driver refuses | `ErrorKind::Compile` from `create_render_pipeline`, with the messages of Slang as diagnostics when there are any | Fix the shader source or the descriptor that the diagnostics name |
| A pass without a target, a colour target of a depth format or without `RENDER_TARGET`, a depth target other than `D32Float` or without `DEPTH_STENCIL`, a depth target of another size than the colour target, a clear depth outside the range 0 to 1, or a target of another device | `ErrorKind::InvalidArgument` from `render_pass` | Give a colour target, a depth target, or both, with the formats, uses, and size that the pass needs |
| A pipeline of another device | `ErrorKind::InvalidArgument` from `RenderPass::bind_pipeline` | Bind a pipeline of the device of the pass |
| More than 16 vertex buffers, or a vertex or index buffer of another device or without `VERTEX_BUFFER` or `INDEX_BUFFER` | `ErrorKind::InvalidArgument` from `set_vertex_buffers` or `set_index_buffer` | Create the buffers on the device with the usage of their use |
| A draw without a bound pipeline, with a pipeline whose formats differ from the targets of the pass, in a pass with only a depth target, or in a pass that samples one of its targets | `ErrorKind::InvalidArgument` from `draw` or `draw_indexed` | Bind a pipeline of the formats of the pass, and sample no target of the pass |
| A draw without a vertex buffer for a stream, with a buffer too small for its count, without an index buffer for indices, or with an index buffer too small for its count, or into a target wider or taller than 65535 texels | `ErrorKind::InvalidArgument` from `draw` or `draw_indexed` | Set a buffer for each stream that holds the count, and an index buffer of 32-bit indices for `draw_indexed` |
| A panic in the debug callback, or a call from it into this crate during a call of slang-rhi | The process aborts | Do not panic in the callback, and do not use the objects of this crate from it |

### 14.5. Logs

The crate emits no logs and has no logging dependency. slang-rhi reports its messages through the debug callback, which the user supplies. This answers Q-16 of the architecture design for this crate.

## 15. Assumptions to Confirm

Each row is a fact that this design relies on and that is not verified. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

RHA-1 to RHA-4 were confirmed, and Sections 3, 6.4, 6.5, 6.7, and 7 state the results. RHA-5 was confirmed, and Sections 3 and 11.1 state the result. RHA-8 was confirmed on Vulkan and with DXIL on WARP, and Section 11.1 states the result.

RHA-6, that an input layout with filler elements for the unused locations matches the shader on both backends, held on Vulkan and was refuted on D3D12, where DXIL keeps a declared input that the shader does not read. The rule on vertex inputs of RHI-26 replaces it, and Sections 3 and 11.2 state the facts. RHA-7, that a Vulkan device of slang-rhi enables `robustBufferAccess`, was confirmed by reading and by test, and Section 3 states the result.

No assumption of this document is open.

## 16. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row. An identifier is never reused.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| RHQ-1 | Should `check-slang-call-guard` check that every call of this crate into slang-rhi and into Slang sits alone in its abort guard, as it checks for `slang-compiler`? Extending the checker changes a mechanism that `AGENTS.md` declares frozen, and leaving it out keeps a deterministic check as a judgement rule (Section 8.1) | The owner of the project, by a decision | Not assigned |
| RHQ-2 | Will slang-rhi document which of its objects tolerate use from several threads, so that the types of this crate and of the `rhi` backend of `slango` can be `Send` and `Sync`? The owner's decision of 2026-11-18 removes both traits from `slango` without waiting for an answer (Section 7), so M10 does not need it | The owner, through an issue in the slang-rhi repository, `shader-slang/slang-rhi`, for which `docs/upstream-reports.md` holds no entry yet | Not assigned |
| RHQ-3 | Can the crate free the debug callback object of a device, if slang-rhi offers a notice when a native device is freed? | This document, after RHQ-2 is asked upstream | Not assigned |
| RHQ-5 | Should the wrappers get the raw accessors `from_raw`, `into_raw`, and `as_raw` that COM-11 of the coding standard asks of every interface wrapper? The crate has none, so a program that combines it with other native code cannot hand over or adopt an object (Section 5.1) | The owner of the project, by a decision | Not assigned |
| RHQ-6 | Should the crate offer the validation layer of the graphics API through `IRHI::enableDebugLayers`, which affects the whole process and cannot be turned off again? The crate does not call it, so the `gpu` tests run under the debug device of slang-rhi only, and no test has the Khronos layer check valid use (Section 10.4) | The owner of the project, by a decision | Not assigned |
| RHQ-7 | Should `Device::new` require the Vulkan feature `nullDescriptor`, on which a buffer parameter without a bound buffer relies? The crate documents the limit and does not check it. D3D12 binds a null descriptor without a feature, so the question concerns Vulkan only (Section 6.7) | The owner of the project, by a decision | Not assigned |
| RHQ-8 | Should `create_buffer` check the size against the largest buffer that the device allows? The crate does not, and what slang-rhi and a driver do with a larger size, such as `u64::MAX`, is not verified (Section 6.6). `DeviceInfo::max_buffer_size` cannot answer it alone, because it is the 2 GiB that slang-rhi assumes and not a value of the device (RHI-29) | The owner of the project, by a decision | Not assigned |
| RHQ-9 | Should CI run `check-miri-rhi`? It clones Slang a second time and builds it natively for the build scripts, which took 8 min 33 s on its first run in the development container (TOOL-52 of `docs/design/tooling.md`) | The owner of the project, by a decision | Not assigned |
| RHQ-10 | Should the memory checks of RHI-8 depend on the backend, or should the crate gain a copy command, so that a buffer in `ReadBack` memory gets a use? The crate refuses unordered access and initial data for that memory on every backend, and on D3D12 `read_buffer` cannot read it, so such a buffer has no use through the crate (Section 6.6) | The owner of the project, by a decision | Not assigned |
| RHQ-11 | Should the crate add a way to set a resource inside a structure that is a plain parameter, or a field of one, and to write the whole uniform data of a sub-object, such as that of a constant buffer whose element is not a structure? slang-rhi addresses neither by name, and `ShaderOffset` has no constructor for the second, so the backend `rhi` of `slango` refuses such used parameters at load (Section 6.5 and SOQ-11 of `docs/design/slango.md`). This is a missing feature of the crate, not a defect of slang-rhi | The owner of the project, by a decision | Not assigned |
| RHQ-12 | Should the crate refuse a render pipeline on a Vulkan device without the feature `depthClamp`? slang-rhi enables depth clamping in every render pipeline, which is valid use of Vulkan only with that feature (RKB-9), and the crate does not check it. The NVIDIA RTX 4070 and lavapipe have the feature, and the D3D12 backend does not clamp | The owner of the project, by a decision | Not assigned |

## 17. References

### 17.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| RHI-1 | 4.1 | The crate depends on `slang-rhi-sys`, `slang-compiler-sys`, `slang-compiler` with `vendored`, and `bitflags`, and has no features. |
| RHI-2 | 4.2 | A crate-level `unsafe_code` expectation, four private modules for the unsafe abstractions, and each wrapper module with the thunk calls of its own wrapper. |
| RHI-3 | 5.1 | One crate-private owning interface pointer does all counting. |
| RHI-4 | 5.2 | Every wrapper holds its `Device`, the device holds its global session and session, and every unowned pointer lives in a value that borrows its owner. |
| RHI-5 | 6.4 | A device always gets the caller's global session and serial pipeline compilation, and an adapter is checked against the device type and fetched again. |
| RHI-6 | 6.4 | Every device gets a leaked, non-unwinding callback object that feeds the collector and lends each message to the user's closure for one call. |
| RHI-7 | 6.5 | A shader program takes only a linked, fully specialized program of the device's session, a compute pipeline only one compute entry point whose thread group is within the limits of the device, and both collect diagnostics. |
| RHI-8 | 6.6 | Buffer size, usage, memory type, initial data, and read-back ranges are checked before any native call. |
| RHI-9 | 6.7 | A compute pass exists only inside a closure that begins it and ends it on every path, including a panic. |
| RHI-10 | 6.7 | `offset_of` finds a top-level field by name as `ShaderCursor` does, with its uniform size and range count. |
| RHI-11 | 6.7 | `set_data` and `set_buffer` check the parameter, offsets, ranges, binding types, buffer usage, and the device before each call, and `set_buffer` refuses a parameter of several binding ranges. |
| RHI-12 | 6.8 | One `Error` with a kind, an operation, the result code, and diagnostics, and one mapping of result codes. |
| RHI-13 | 7 | Types that hold native objects are neither `Send` nor `Sync`, and value types are both, with assertion tests. |
| RHI-14 | 7 | Every call through `IRHI` holds one crate-private lock, taken after the per-thread guard, and the crate never destroys the instance. |
| RHI-15 | 8.1 | Every native call runs in the per-thread guard, the Slang call guard of `slang-compiler`, and the crate's abort guard, innermost. |
| RHI-16 | 10.2 | Miri tests the counting, the pass closures, the guards, the checks, `Binding`, `submit`, and the result mapping against a fake of seven interfaces. |
| RHI-17 | 12 | The crate documentation states the path, the build, the platforms, the threading, the leaked callback, the aborts, and the features. |
| RHI-18 | 8.2 | A per-thread mark makes a reentrant call into slang-rhi panic before it starts. |
| RHI-19 | 8.3 | A per-thread collector returns the messages of Slang with the call of the same thread that caused them. |
| RHI-20 | 9.1 | The crate checks every condition of valid use of the graphics API that a safe call could break and that it can check. |
| RHI-21 | 6.7 | `submit` takes its command buffers, checks their device, and releases them after the call. |
| RHI-22 | 11.1 | `ShaderObject::object` returns the checked object of a constant buffer or parameter block, which borrows its parent, refuses a parameter of several binding ranges, and gives an error for a null object. |
| RHI-23 | 11.2 | `create_texture` creates a checked two-dimensional texture of five formats with its default view and checked initial data, and checks `D32Float` depth support on Vulkan. |
| RHI-24 | 11.2 | `create_sampler` creates a sampler from the crate's descriptor and writes every other raw field explicitly. |
| RHI-25 | 11.2 | `set_texture` and `set_sampler` check the range, its shape, element type, and comparison, the usage, and the device, and keep a reference through the call. |
| RHI-26 | 11.2 | `create_render_pipeline` checks the program, formats, vertex streams, and a matching stream for every declared vertex input, and creates a pipeline of triangles with one colour target. |
| RHI-27 | 11.2 | A render pass exists only inside a closure that ends it on every path, may hold a depth target alone, and each draw checks its pipeline, buffers, counts, and targets against one record of the pass. |
| RHI-28 | 11.2 | `read_texture` checks the texture and the output, reads `sizeInBytes` at the default alignment, and returns the rows without padding. |
| RHI-29 | 11.1 | `DeviceInfo` offers the seven limits of dispatches, thread groups, buffers, textures, vertex streams, and vertex input elements that the checks use. |
| RHI-30 | 11.1 | `create_slang_session` returns a `CompileSession` that reproduces the device session, whose own `create_shader_program` accepts programs of its session. |
