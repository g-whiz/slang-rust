# `slang-rhi` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-18 |
| Applies to | `crates/slang-rhi` |

## Abstract

This document specifies `slang-rhi`, the safe Rust crate over the raw bindings of `slang-rhi-sys`, in the scope of milestone M9: the path of one compute dispatch. A program lists the adapters of a backend, opens a device with a global session of `slang-compiler`, creates a shader program from a linked Slang program of the device's session and a compute pipeline from it, creates buffers, encodes a compute pass that binds parameters by name and dispatches, submits the work, and reads a buffer back. The document defines the ownership of every object, the borrows that turn the unowned pointers of slang-rhi into lifetimes, the checks that keep a safe call within valid use of the graphics API, the errors, the threading rules that follow from slang-rhi documenting none, the guards around calls into native code, the tests, and what waits for M10.

## 1. Introduction

### 1.1. Problem

The raw bindings of `slang-rhi-sys` require a caller to count references, to keep the owner of every unowned pointer alive, to build parameter bindings whose fields hold references, and to decide which objects may be used from which threads, while slang-rhi documents almost none of these rules (Section 3.3 of `docs/design/slang-rhi-sys.md`). A mistake is undefined behaviour that a test on one driver may not show.

### 1.2. Purpose

`slang-rhi` gives Rust users a safe API for the compute path of slang-rhi, built on `slang-compiler` for everything that concerns Slang. The `rhi` backend of `slango` in M10 builds on it (Section 15.13 of the architecture design).

This document implements the part of Section 15.12 of the architecture design that concerns `slang-rhi`, within the owner's decisions of Section 4.1 of `docs/design/slang-rhi-sys.md`. It answers Q-16 of the architecture design for this crate (Section 14.5).

### 1.3. Audience

The primary readers are the coding agents that implement the crate and the `rhi` backend of `slango`. The secondary readers are the reviewers of those changes and the owner. A reader is expected to know Rust and the ownership model of `docs/design/slang-compiler.md`, but not slang-rhi.

### 1.4. Scope

This document covers the crate and its dependencies, the ownership model, the public API of the compute path, errors, threading, the guards around calls into native code, the soundness invariants, the tests, the crate documentation, and the security and operational considerations.

### 1.5. Non-Goals

This document does not cover, in M9:

- Rendering, textures, samplers, fences, mapped buffers, copy commands, and every other part of the API of slang-rhi that the compute path does not need, which M10 adds as `slango` needs them (Section 11).
- Backends other than Vulkan, which follow the platforms of Section 7 of `docs/design/slang-rhi-sys.md`.
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

This section is non-normative. Section 3 of the sys design states the facts about slang-rhi on which this design relies, and this section adds the facts of the compute path. Paths are in the slang-rhi tree of the sys design.

The path of one compute dispatch is the one of `tests/test-compute-trivial.cpp:6-45`. It creates a device, takes the Slang session of the device with `IDevice::getSlangSession` (`include/slang-rhi.h:3401`), loads and links a program there, creates a shader program (`:3618`) and a compute pipeline (`:3656`), creates buffers (`:3456`), takes a queue (`:3555`) and a command encoder (`:2862`), begins a compute pass (`:2608`), binds the pipeline, which returns the root shader object (`:2528`), sets the parameters, dispatches (`:2531`), ends the pass (`:2496`), finishes the encoder (`:2769`), submits (`:2886`), waits (`:2902`), and reads the result (`:3704`).

Every descriptor on that path is passed by `const` reference, and no method on it takes an aggregate by value. `ShaderOffset` holds three `uint32_t` (`:1655`). `IShaderObject` sets data by `setData(const ShaderOffset&, const void*, Size)` and a resource by `setBinding(const ShaderOffset&, const Binding&)` (`:1750`, `:1753`), and offers no lookup by name. The helper `ShaderCursor` finds a field by name through the reflection of the element type layout of the object: `findFieldIndexByName`, the field's uniform offset, and `getFieldBindingRangeOffset` (`include/slang-rhi/shader-cursor.h:87`, `:169-194`). When a lookup at the root fails, `ShaderCursor` also searches the entry points (`include/slang-rhi/shader-cursor.h:245-262`).

The element type layout of a root shader object is the struct of the global parameters of the program, unwrapped from its container (`src/shader-object.h:132-166`, `src/vulkan/vk-shader-object-layout.cpp:901-903`). `ShaderObject::setData` clamps a size that runs past the uniform data of the object on purpose (`src/shader-object.cpp:67-72`).

`createShaderProgram` takes an optional out-parameter for diagnostics (`:3618`), which the Vulkan backend never fills (`src/vulkan/vk-device.cpp:2521-2538`). Code generation reports the errors of Slang only through the debug callback (`src/shader.cpp:200-207`). slang-rhi reports other messages through the debug callback of the device, from the calling thread or from a thread of the driver (`docs/error-handling.md:39-63`). A device whose `DeviceDesc::debugCallback` is null gets the static `NullDebugCallback`, which drops every message (`src/device.cpp:493`, `src/device.h:139-158`).

`ShaderProgram::compileShaders` holds a `std::mutex` while it compiles (`src/shader.cpp:113`), and the debug callback runs while that mutex is held. A second lock of a `std::mutex` by the thread that holds it is undefined behaviour.

slang-rhi checks the arguments of a call against the rules of the graphics API only in its debug device. Without the debug device, it passes them to the driver unchecked. The validation layer of Vulkan, which checks valid use, is turned on only through `IRHI::enableDebugLayers` (`src/vulkan/vk-device.cpp:382-449`), which affects the whole process and cannot be turned off again.

The `IRHI` instance is one object of the process, which any thread reaches through `rhiGetInstance`, and slang-rhi documents no threading rule for it (Section 3.3 of the sys design).

A device whose `SlangDesc::slangGlobalSession` is null creates a global session of its own (Section 3.3 of the sys design), and that session has no loader of `slang-glslang-sys`. A C++ program outside the repository, linked against the vendored Slang and slang-rhi, created such a device and compiled a compute shader through `IDevice::getSlangSession`. Its debug callback received `error[E00100]: failed to load downstream compiler 'spirv-opt'` with `note[E99996]: failed to load dynamic library 'slang-glslang-2026.17'`, and the SPIR-V, which was not optimized, still ran and gave the expected results.

The device's session has exactly one target, the device's compile target, with `forceGLSLScalarBufferLayout` (`src/slang-context.h:78-92`), and slang-rhi generates the code of a program for the target at index 0 of the program's session (`src/shader.cpp:150-156`).

The facts that the checks of the buffers and of the dispatch rely on are these:

- `createBuffer` maps each flag of `BufferUsage` to a Vulkan buffer usage bit (`src/vulkan/vk-utils.cpp:479-494`). `ShaderResource` and `UnorderedAccess` both give `STORAGE_BUFFER`, and `ConstantBuffer` gives only `UNIFORM_BUFFER`. `AccelerationStructure`, `AccelerationStructureBuildInput`, and `ShaderTable` map to bits of extensions, and `Shared` to external memory (`src/vulkan/vk-buffer.cpp:424-434`), which a plain compute device need not enable.
- `createBuffer` adds `TRANSFER_DST` itself when initial data is given (`src/vulkan/vk-buffer.cpp:408-411`). The memory types `Upload` and `ReadBack`, and the usage `ConstantBuffer`, select host-visible coherent memory, into which initial data is copied through a mapping, and `DeviceLocal` memory receives initial data through a staging copy (`src/vulkan/vk-buffer.cpp:413-421`, `:443-460`). A memory type that the device lacks makes the call fail.
- `readBuffer` copies with `vkCmdCopyBuffer` for every memory type (`src/vulkan/vk-device.cpp:1869-1918`), which needs `TRANSFER_SRC` on the buffer.
- A command buffer that the queue has run is recycled by slang-rhi for a later encoder (`src/vulkan/vk-command.cpp:1912-1937`), and the queue keeps its own reference to each command buffer in flight (`src/vulkan/vk-command.cpp:1996-2003`).
- `DeviceInfo::limits.maxComputeDispatchThreadGroups` holds the Vulkan limit `maxComputeWorkGroupCount` (`src/vulkan/vk-device.cpp:1613-1615`).
- With the linking style `SingleProgram` and no separate entry points, `ShaderProgram::_isSpecializable` checks the specialization parameter count of the global scope (`src/shader.h:132-146`). For a specializable program, `createComputePipeline` creates a virtual pipeline whose code slang-rhi generates at the first dispatch (`src/device.cpp:758-764`, `src/pipeline-resolver.cpp:96-106`).
- The Vulkan backend of `createComputePipeline` asserts that the program has an entry point, and the failed assertion aborts the process (`src/vulkan/vk-pipeline.cpp:836`, Section 3.3 of the sys design).

The raw types of the compute path in `slang-rhi-sys` have these shapes (RSYS-10 of the sys design), and a `gpu` test of that crate runs the whole compute path through them (Section 9 of the sys design):

- `DeviceDesc`, `BufferDesc`, `SubmitDesc`, and the other descriptors of the compute path implement `Default` with the C++ defaults, so the crate writes `..T::default()` for every field that it does not set. The default `cudaStream` of `SubmitDesc` is `kInvalidCUDAStream`.
- `Binding` holds its two resources as the raw pointers `resource` and `resource2` of type `*mut IResource`, and the range in a field of an anonymous union. slang-rhi adds its own references during `setBinding`, so the crate keeps its own reference through the call (Section 6.7).
- The struct constant `kEntireBuffer` has no binding, so the crate writes the range of a whole buffer itself, as an offset of 0 and a size of `u64::MAX`.
- `ShaderOffset` is a plain record of three `u32` fields.
- The methods of the placeholder slots have no thunk, and none of them is on the compute path.

### 3.1. Known Upstream Bugs

This is the one list of the known defects of the recorded slang-rhi. It has the columns of Section 3.12 of the `slang-compiler` design and follows the rules stated there, and every path in the column "Source" is in the slang-rhi tree.

The crate does not guard against these defects, as Hard Rule 14 of `AGENTS.md` requires for Slang. The checks of RHI-8, RHI-11, and RHI-20 are not guards, because a safe interface checks the bounds of an index and the valid use of the graphics API whatever slang-rhi does (Section 9.1).

| Identifier | Defect and smallest input | Call | Outcome | Source | Report |
| --- | --- | --- | --- | --- | --- |
| RKB-1 | The range check of `readBuffer` adds `offset` and `size` with no guard against overflow, so a sum that wraps lets a range past the end of the buffer pass. By a reading, the smallest input is a buffer of any size read with `offset` set to `SIZE_MAX` and `size` set to 1, whose sum wraps to 0 | `IDevice::readBuffer` | Not observed, because the defect was read in the source and not reproduced. By the reading, the check passes and the backend reads from `offset` bytes past the start of the buffer, as the CPU backend does with `memcpy` (`src/cpu/cpu-buffer.cpp:61`) | `if (offset + size > bufferImpl->m_desc.size)` in the Vulkan backend (`src/vulkan/vk-device.cpp:1872`), where `Offset` and `Size` are `size_t` (`include/slang-rhi.h:57-58`) and `m_desc` is a `BufferDesc` (`src/rhi-shared.h:95`) whose `size` is `uint64_t` (`include/slang-rhi.h:745`). The CPU, WebGPU, CUDA, Metal, D3D12, and D3D11 backends have the same check (`src/cpu/cpu-buffer.cpp:57`, `src/wgpu/wgpu-device.cpp:470`, `src/cuda/cuda-device.cpp:534`, `src/metal/metal-device.cpp:377`, `src/d3d12/d3d12-device.cpp:1730`, `src/d3d11/d3d11-device.cpp:567`), and so does the debug layer (`src/debug-layer/debug-device.cpp:1221`) | Not reported |

## 4. Crate Structure

### 4.1. Package and Dependencies

RHI-1. The crate MUST depend on `slang-rhi-sys`, on `slang-compiler-sys`, on `slang-compiler` with the feature `vendored`, and on `bitflags`, and on no graphics crate. It MUST have no Cargo features.

`slang-rhi-sys` always links the vendored Slang (Section 4.1 of the sys design), and Cargo unifies the feature `vendored` of `slang-compiler-sys` in the whole build graph. `slang-compiler` without its own feature `vendored` would then create global sessions without the loader of `slang-glslang-sys`, and every program would get SPIR-V that is not optimized (SGL-12 of `docs/design/slang-glslang-sys.md`). Enabling the feature here installs the loader on every global session that `slang-compiler` creates.

The crate calls the reflection functions of `slang-compiler-sys` for the lookup of Section 6.7, so it depends on that crate directly. `bitflags` represents the usage flags of a buffer, which a caller combines, as TYPE-8 of the coding standard requires. It is in `[workspace.dependencies]` under DEP-2.

The manifest declares `slang-rhi-sys` and `bitflags` for every target, and `slang-compiler` and `slang-compiler-sys` for Linux with the GNU C library only, as `slang-rhi-sys` declares its own dependencies (Section 7.1 of the sys design). On another target, the build script of `slang-rhi-sys` stops the build at once, before anything is cloned, so the crate root carries no `cfg` of its own.

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

This table records, for each call of the compute path, whether the returned pointer carries a reference, as COM-2 of the coding standard requires. Section 3.3 of the sys design states where slang-rhi adds the reference of each owned return:

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
| `IShaderObject::getElementTypeLayout` | `slang::TypeLayoutReflection*` | Borrowed from the program of the bound pipeline | Used only inside one call of the crate (Section 6.7) |

RHI-4. Every wrapper of an owned interface MUST hold the `Device` that created it, as COM-5 of the coding standard requires, and a `Device` MUST hold the `slang_compiler::GlobalSession` that it was created with and its `slang_compiler::Session`. Every pointer that the table marks borrowed MUST be held only by a value whose lifetime borrows its owner as the table states.

A native child holds a strong reference to its device (Section 3.3 of the sys design), so the Rust parent field is not what keeps the device alive. It keeps the field order of COM-5 and makes the device reachable from every object for the checks of RHI-7 and Section 6.7. The borrows make the compiler enforce the validity rules of Section 3.3 of the sys design at no run-time cost, as SLC-19 of the `slang-compiler` design does for reflection handles.

## 6. Public API

### 6.1. Overview

The public items are at the crate root (API-10):

- The free function `adapters` and the type `Adapter`, with the enumerations `DeviceType` and `AdapterType`.
- `DeviceOptions`, `Device`, `DeviceInfo`, `DebugMessage`, `DebugMessageType`, and `DebugMessageSource`.
- `ShaderProgram` and `ComputePipeline`.
- `BufferDesc`, `MemoryType`, `BufferUsage`, and `Buffer`.
- `Queue`, `CommandEncoder`, `ComputePass`, `ShaderObject`, `ShaderOffset`, and `CommandBuffer`.
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

`Device` offers `info(&self) -> DeviceInfo`, a record of the device type, the name of the API, and the name of the adapter, `slang_session(&self) -> &slang_compiler::Session`, and the methods of Sections 6.5 to 6.8. `Device` is `Clone`, and a clone adds a reference to the same device. `Device::new` reads `DeviceInfo::limits.maxComputeDispatchThreadGroups` once and keeps it for the check of `dispatch` (Section 6.7).

RHI-6. The crate MUST pass every device a heap-allocated `IDebugCallback` object of its own, with or without a closure of the user, whose `handleMessage` is defined `extern "system"` and stored in the generated field through a conversion of the function pointer, and MUST never free that object. `handleMessage` MUST give each message whose source is Slang to the collector of Section 8.3, and MUST build a `DebugMessage<'_>` that borrows the message for the call only and call the user's closure, when the device has one.

The object always exists, because on Vulkan the errors of Slang reach the crate only through the debug callback (Section 3), and RHI-7 returns them as diagnostics. Without the object, slang-rhi would use its `NullDebugCallback` and drop them. With validation on, the debug device receives the same object (`src/rhi.cpp:352`).

slang-rhi gives no notice when a native device is freed, and children keep the native device alive after every Rust wrapper is gone (Section 3.3 of the sys design), so no Rust value knows when the callback can no longer be called. A freed callback object that slang-rhi calls later is a use after free, while a leaked one costs one small allocation for each device, and the user's closure when there is one, which UNS-4 of the coding standard permits (RHQ-3). The object is never unregistered, so FFI-8 holds with the context alive for the life of the process.

The closure takes the bounds `Send + Sync + 'static`, which CONC-6 of the coding standard permits for a callback that a foreign library keeps: slang-rhi calls it from several threads at once and from threads of the driver (Section 3), and keeps it beyond any borrow. A panic of the closure reaches the non-unwinding boundary and aborts the process, as FFI-7 requires for a callback that reports no result code. The crate documentation states this.

`DebugMessage` offers `message_type(&self) -> DebugMessageType`, `source(&self) -> DebugMessageSource`, and `text(&self) -> Cow<'_, str>`, converted with each invalid sequence replaced by U+FFFD, because a message must not be lost to an error about its encoding. A message whose type or source the crate does not know has no variant to represent it, so the closure does not receive it. A message of Slang still reaches the collector.

### 6.5. Shader Programs and Compute Pipelines

RHI-7. `Device::create_shader_program(&self, program: &slang_compiler::LinkedProgram) -> Result<ShaderProgram>` MUST check that the program belongs to the session of the device, by comparing the raw pointer of the session that `LinkedProgram::session` returns with that of `slang_session`, and that the program has no specialization parameter left, and MUST fail with the kind `InvalidArgument` otherwise. It MUST pass the program as `slangGlobalScope` with no entry points and the linking style `SingleProgram`, pass and read the diagnostics out-parameter, and collect the messages of Slang during the call (Section 8.3).

slang-rhi takes the Slang session from the program and generates code for the target at index 0 of that session (Section 3.3 of the sys design, Section 3). The device's session has exactly one target, the one that the device runs, so only a program of that session gets code that the device runs. The crate adopts the session with a target count of 1. With no entry points, slang-rhi uses the linked program as it is (`src/shader.cpp:58-62`), so the program that the caller linked is the program that runs. `LinkedProgram::session` is an addition to `slang-compiler` (SLC-40 of the `slang-compiler` design).

The crate reads the specialization parameter count through `IComponentType::getSpecializationParamCount` of the linked program, inside the guards of Section 8, because `slang-compiler` offers no safe accessor for it. A count other than 0 is the condition under which slang-rhi creates a virtual pipeline and generates its code at the first dispatch (Section 3), outside the collector, where the crate could not return the diagnostics. The refusal names the count and says to specialize the program before it is linked.

`create_shader_program` also reads the stages of the program's entry points through the reflection of `slang-compiler`, for the check of `create_compute_pipeline`, so it fails with the kind `Slang` when that reflection fails.

A failure has the kind `Compile` and holds as its diagnostics the text of the out-parameter, when slang-rhi fills it, or else the collected messages of Slang. Warnings on success are dropped in M9, because the result type carries no diagnostics. Whether to return them, as `slang-compiler` does with `Compiled`, is RHQ-4.

`Device::create_compute_pipeline(&self, program: &ShaderProgram) -> Result<ComputePipeline>` MUST fail with the kind `InvalidArgument`, before any call into slang-rhi, when the shader program belongs to another device, or when its linked program does not have exactly one entry point, whose stage is compute. Otherwise it creates the pipeline with the default compilation policy and collects the messages of Slang during the call.

slang-rhi aborts the process on a compute pipeline of a program without an entry point (Section 3), and a stage other than compute in a compute pipeline breaks valid use of Vulkan (Section 9.1). `createShaderProgram` generates no code (`src/vulkan/vk-device.cpp:2521-2538`). With the default policy and the pipeline compilation mode `Serial`, `createComputePipeline` generates the code of the program at this call (`src/device.cpp:755-769`, `src/shader.cpp:110`, `src/device.cpp:447`), so a failure of that compile has the kind `Compile` with the collected messages.

### 6.6. Buffers

`BufferDesc::new(size: u64, usage: BufferUsage) -> BufferDesc` creates a descriptor with the element size 0 and the memory type `MemoryType::DeviceLocal`, and `with_element_size(u32)` and `with_memory(MemoryType)` return changed copies. `Buffer` offers `size(&self) -> u64`, and keeps its usage for the checks of Section 6.7 without a public accessor.

`BufferUsage` is a set of flags of `bitflags` with eight flags: `VERTEX_BUFFER`, `INDEX_BUFFER`, `CONSTANT_BUFFER`, `SHADER_RESOURCE`, `UNORDERED_ACCESS`, `INDIRECT_ARGUMENT`, `COPY_SOURCE`, and `COPY_DESTINATION`. Each maps to a buffer usage bit of core Vulkan 1.0, which is valid on every device. The flags `ACCELERATION_STRUCTURE`, `ACCELERATION_STRUCTURE_BUILD_INPUT`, `SHADER_TABLE`, and `SHARED` of `rhi::BufferUsage` are left out, because each needs an extension or external memory that a plain compute device need not enable (Section 3).

RHI-8. `Device::create_buffer(&self, desc: &BufferDesc, initial: Option<&[u8]>) -> Result<Buffer>` MUST fail with the kind `InvalidArgument` when the size of the descriptor is 0, when its usage is empty or holds a bit outside `BufferUsage::all()`, or when `initial` is given and its length differs from the size of the descriptor. `Device::read_buffer(&self, buffer: &Buffer, offset: u64, out: &mut [u8]) -> Result<()>` MUST fail with `InvalidArgument` when the buffer belongs to another device, when its usage lacks `COPY_SOURCE`, or when `offset` plus the length of `out` exceeds the size of the buffer, with checked arithmetic, before it calls `readBuffer`.

`createBuffer` receives no length for the initial data (`include/slang-rhi.h:3456-3460`), and the Vulkan backend copies the size of the descriptor from it (`src/vulkan/vk-buffer.cpp:390-396`, `:443-447`, `:457`), so a short slice is a read out of bounds in native code. Without the check, creating a buffer of size 0 crashes the process, and a buffer without a usage breaks valid use of Vulkan.

`readBuffer` copies with `vkCmdCopyBuffer` (Section 3), which needs `COPY_SOURCE`, and writes the requested range into the caller's pointer (`include/slang-rhi.h:3704`). The range check of slang-rhi can wrap (RKB-1 in Section 3.1), and the crate checks the range itself in any case, as a safe interface does.

Initial data needs no `COPY_DESTINATION`, because slang-rhi adds the transfer bit itself, and the memory type adds no condition that the crate checks (Section 3).

`create_buffer` does not check the size against the largest buffer that the device allows. Whether it does is RHQ-8.

### 6.7. Command Encoding and Parameters

`Queue` comes from `Device::queue(&self) -> Result<Queue>`, which takes the graphics queue, and offers `encoder(&self) -> Result<CommandEncoder>`, `submit(&self, command_buffers: impl IntoIterator<Item = CommandBuffer>) -> Result<()>`, and `wait_on_host(&self) -> Result<()>`.

RHI-9. `CommandEncoder::compute_pass<R>(&mut self, f: impl FnOnce(&mut ComputePass<'_>) -> Result<R>) -> Result<R>` MUST begin the pass, run `f`, and end the pass whether `f` succeeds, fails, or panics, and MUST NOT offer a pass value that outlives the closure. A panic of `f` MUST continue after the pass has ended. `CommandEncoder::finish(self) -> Result<CommandBuffer>` consumes the encoder.

A pass that is begun and never ended leaves the encoder in a state that slang-rhi does not document. A guard that ends the pass in `Drop` would break UNS-4, because a leaked guard skips `end`, so the closure-scoped form that UNS-4 recommends ends it on every path. The method runs `f` under `catch_unwind`, ends the pass, and resumes the panic, because a caller that catches the panic could otherwise keep using the encoder with a pass still open and call `finish` on it.

RHI-21. `Queue::submit` MUST take ownership of its command buffers, MUST fail with the kind `InvalidArgument`, before any call into slang-rhi, when one of them belongs to another device, and MUST release its own references to them after the native call.

slang-rhi recycles the native command buffer once the queue has run it (Section 3), so a command buffer that the caller could submit again would name an object that slang-rhi already reuses. Taking ownership makes a second submission, or a command buffer listed twice, a compile error. The queue keeps its own reference to each command buffer in flight, so the release after the call is sound. The command buffers are consumed even when the check fails, which their documentation states. `submit` passes a `SubmitDesc` with its command buffers and `..SubmitDesc::default()` for every other field.

`ComputePass` offers `bind_pipeline(&mut self, pipeline: &ComputePipeline) -> Result<ShaderObject<'_>>` and `dispatch(&mut self, thread_groups: [u32; 3]) -> Result<()>`. `bind_pipeline` fails with the kind `InvalidArgument` when the pipeline belongs to another device, and with `Failed` when slang-rhi returns null. It marks the pass as unbound before the native call and as bound only after a root object is returned, so after a failed bind the pass refuses to dispatch until a bind succeeds.

`dispatch` fails with `InvalidArgument` when no pipeline is bound in the pass, and when a count is larger than the limit of the device along its axis, which `Device::new` read. A count of 0 runs nothing and is valid.

`ShaderObject<'p>` offers `offset_of(&self, name: &str) -> Result<Option<ShaderOffset>>`, `set_data(&mut self, offset: ShaderOffset, data: &[u8]) -> Result<()>`, `set_buffer(&mut self, offset: ShaderOffset, buffer: &Buffer) -> Result<()>`, and `entry_point(&mut self, index: usize) -> Result<ShaderObject<'_>>`.

`ShaderOffset` is a `Copy` record with private fields: the uniform offset, the uniform size, the binding range index, the binding range count, and the binding array index. It offers `uniform_offset`, `uniform_size`, `binding_range_index`, and `binding_array_index`, each returning a `u32`, and `with_binding_array_index(self, u32) -> Self`. Only `offset_of` creates one, and it has no `Default`, because no default offset names a parameter. A caller therefore cannot forge an offset, and every offset describes a parameter that the layout of a program declares.

RHI-10. `offset_of` MUST find a field of the element type layout of the object by name as `ShaderCursor::getField` does (Section 3): the field index through `spReflectionTypeLayout_findFieldIndexByName`, the uniform offset of the field's variable layout, and the binding range through `spReflectionTypeLayout_getFieldBindingRangeOffset`, with `None` for a name that the layout does not have. It MUST also read the uniform size and the binding range count of the field's type layout. It finds a field of the top level of the object only.

The offsets of `offset_of` are those that `ShaderCursor` finds, because the element type layout of a root object is the struct of the global parameters (Section 3). Unlike `ShaderCursor`, `offset_of` does not search the entry points when a name is missing at the root. A parameter of an entry point is found through `entry_point`.

RHI-11. Before each call, `set_data` MUST check that the parameter of the offset has uniform data, that the length of `data` does not exceed the uniform size of the parameter, and that the uniform offset plus that length does not exceed the uniform size of the element type layout. `set_buffer` MUST check that the parameter of the offset has a binding range, that the binding range index is below the binding range count of the object, that the array index is below the binding count of that range, that the range is a raw buffer and not a typed buffer, that the usage of the buffer suits the access of the range, and that the buffer belongs to the device of the pass. Each check MUST fail with the kind `InvalidArgument`.

The header states no check of these bounds by slang-rhi, and a write past the uniform data of an object or into a range that does not exist is an access out of bounds in native code. These are the checks of an index that a safe interface needs, so they stay whatever slang-rhi checks (Hard Rule 14 of `AGENTS.md` for the analogous case of Slang). The checks against the parameter's own size and range stop an offset of one parameter, or of another object, from writing into the next parameter, such as an offset of a `float` that names the binding range of the next buffer.

A mutable raw buffer range needs `UNORDERED_ACCESS`, and a read-only raw buffer range needs `SHADER_RESOURCE` or `UNORDERED_ACCESS`, because both give the storage buffer bit that such a range needs (Section 3). A typed buffer range, such as one of `RWBuffer<float>`, needs a buffer with a format, and the crate creates no buffer with a format, so it refuses the range with that reason.

A `ConstantBuffer` or `ParameterBlock` parameter has a uniform size of 0 at the top level, so `offset_of` finds it and `set_data` refuses it. The documentation of `offset_of` states this, and setting the fields of such a parameter waits for M10 (Section 11).

`set_buffer` builds a `Binding` of `slang-rhi-sys` with the type of a buffer, the buffer's interface pointer in `resource`, a null `resource2`, and the range of the whole buffer, which is the offset 0 and the size `u64::MAX` (Section 3), and passes it by pointer while the `Buffer` that it borrows keeps its own reference (Section 6.4 of the sys design).

A buffer parameter that the kernel uses and that has no buffer bound reads a null descriptor, which works only on a device whose Vulkan driver offers the feature `nullDescriptor`. The documentation of `set_buffer` and `dispatch` states this limit, and the crate does not check it. Whether it should is RHQ-7.

### 6.8. Errors

RHI-12. Every failure of the crate MUST be its one type `Error`, which is `Send`, `Sync`, and `'static` and holds an `ErrorKind`, an `Operation` that names the call, the raw result code when slang-rhi returned one, the text of diagnostics when there were any, and, for a failure of `slang-compiler`, that error as its `source()`. One crate-private function MUST map every result code to an `ErrorKind`, as COM-7 of the coding standard requires.

| Kind | Failure |
| --- | --- |
| `InvalidArgument` | A check of the crate failed, or slang-rhi returned `SLANG_E_INVALID_ARG` |
| `NoDevice` | `createDevice` failed, such as without a Vulkan loader or driver |
| `Compile` | `createShaderProgram` or `createComputePipeline` failed with or without diagnostics |
| `OutOfMemory` | `SLANG_E_OUT_OF_MEMORY` |
| `Unsupported` | `SLANG_E_NOT_IMPLEMENTED` or `SLANG_E_NOT_AVAILABLE` |
| `Failed` | Any other failure code that the mapping knows, such as `SLANG_FAIL`, and a null return where the API promises an object |
| `Unrecognized` | A result code or an enumerator that the recorded slang-rhi does not list |
| `Slang` | A failure of `slang-compiler`, such as the adoption of the device's session or the reflection of a program |

`ErrorKind` and `Operation` are `#[non_exhaustive]`. The `Display` text names the operation and the kind and holds no advice (ERR-4). `Error` holds the error of `slang-compiler` in a `Box`, so that the type stays below the size at which `clippy::result_large_err` fires.

## 7. Threading

RHI-13. No type of the crate that holds a pointer of slang-rhi or a wrapper of `slang-compiler`, which are `DeviceOptions`, `Device`, `ShaderProgram`, `ComputePipeline`, `Buffer`, `Queue`, `CommandEncoder`, `ComputePass`, `ShaderObject`, and `CommandBuffer`, MUST be `Send` or `Sync`. `Adapter`, `DeviceInfo`, `BufferDesc`, `ShaderOffset`, the enumerations, `BufferUsage`, and `Error` MUST be `Send` and `Sync`. Each auto trait that a type implements gets a compile-time assertion test (CONC-1).

slang-rhi documents no threading rule for its objects (Section 3.3 of the sys design), so FFI-9 of the coding standard forbids `Send` and `Sync` for them. The wrappers of `slang-compiler` that a device holds are neither (SLC-21 of the `slang-compiler` design). The `rhi` backend of `slango` in M10 therefore cannot make its public types `Send` and `Sync` on this crate without a documented rule, which the owner accepted as a possible breaking version 0.2 of `slango` (Section 15.11 of the architecture design, RHQ-2).

RHI-14. The crate MUST make every call through the `IRHI` instance, which are `rhiGetInstance`, `getAdapter`, and `createDevice`, while it holds one crate-private `static` `Mutex<()>`, MUST check the per-thread guard of Section 8.2 before it takes the lock, and MUST NOT call `rhiDestroyInstance`.

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

The checks of this kind are the usage checks of RHI-8 and RHI-11, the size 0 of RHI-8, the refusal of a typed buffer range, the limit of `dispatch`, the single compute entry point of RHI-7, and the ownership of `submit` (RHI-21). Conditions that the crate does not check yet are RHQ-7 and RHQ-8.

### 9.2. Invariants

These are the invariants on which the unsafe code of the crate relies, each with the requirement that keeps it:

| Invariant | Kept by |
| --- | --- |
| Every interface pointer that the crate dereferences points to a live object | The owning interface pointer (RHI-3) and the borrows of the unowned pointers (RHI-4) |
| No pass encoder or root object is used after its owner ends or rebinds it | The mutable borrows of `ComputePass` and `ShaderObject`, and the closure of RHI-9 |
| Every pointer and length passed to slang-rhi is in bounds | RHI-8 and RHI-11 |
| Every object that a `Binding` names lives through the call | The borrow of the `Buffer` in `set_buffer` |
| No command buffer is submitted after slang-rhi has recycled it | RHI-21 |
| Every call stays within valid use of the graphics API, as far as the crate can check | RHI-20 |
| The debug callback object lives for every call that slang-rhi makes to it | RHI-6 |
| No object is used from two threads at once | RHI-13 and RHI-14 |
| No thread enters slang-rhi again from inside a call of slang-rhi | RHI-18 |
| No C++ exception unwinds through the crate, and no panic unwinds into slang-rhi | RHI-15, and the non-unwinding callback of RHI-6 |
| Every component that a shader program takes comes from the device's session | RHI-7 |

## 10. Tests

### 10.1. Layout and Tiers

The crate has unit tests in its modules and one integration-test binary, `tests/rhi/`, whose module `gpu` holds the tests that need an adapter (ARCH-22 of the architecture design). The recipe `test-rhi` runs them with the nextest profile `gpu`, which the CI job `vendored` runs on Linux on lavapipe, and `check-miri-rhi` runs the unit tests under Miri (TOOL-52 and Section 7.4 of `docs/design/tooling.md`). The crate has no `ffi` tier of its own, because every call of the crate into slang-rhi needs a device, which needs an adapter.

### 10.2. The Fake for Miri

RHI-16. The unit tests MUST test, under Miri, the owning interface pointer, the closure of RHI-9 with a panic of `f`, the per-thread guard and the collector, the bounds and usage checks of RHI-8, RHI-11, and RHI-20, the building of `Binding`, the release after `submit`, and the mapping of result codes, against a fake of the interfaces `IBuffer`, `IComputePassEncoder`, `IShaderObject`, `ICommandBuffer`, and `ICommandQueue` built on the generated vtable structs, with its vtable functions defined `extern "system"`.

TEST-7 of the coding standard asks for a fake of the interfaces, and the fake of `slang-compiler` is the model (SLC-25 of the `slang-compiler` design). The fake covers the interfaces whose counting and borrowing the unsafe code relies on. Its queue records the command buffers of each submission and holds a reference to each until it is freed, as slang-rhi does. A fake of `IDevice`, `IRHI`, and `ICommandEncoder` would need a function for each of their many slots and would test only that the crate passes descriptors, which the `gpu` tests do on a real device. The bounds checks are pure functions over the numbers that the reflection calls return, so they run under Miri without the reflection of Slang (UNS-2).

### 10.3. GPU Tests

The `gpu` tests run on any adapter, which is lavapipe in CI and the development container's NVIDIA RTX 4070 or lavapipe locally. Each test fails, and never skips, when it finds no Vulkan adapter (ARCH-23). They cover:

- `adapters(DeviceType::Vulkan)` returns at least one adapter, and a device opens on each. An adapter of another device type than the options is refused.
- The compute path of Section 6.2 doubles a buffer with optimized SPIR-V and no message of the debug callback.
- Uniform parameters at distinct offsets, buffers in distinct ranges, and a uniform parameter of an entry point, set through `entry_point(0)`, land where `offset_of` finds them. A name of an entry point is not found at the root.
- A shader program from a program of another session, offsets past the uniform data, past the binding ranges, or of another object, and initial data of the wrong length fail with `InvalidArgument`.
- A kernel that does not generate code for the device fails with `Compile` and holds the diagnostics, also on a device without a debug callback, which keeps working afterwards.
- A program without a compute entry point, and a program with a specialization parameter left, are refused before slang-rhi compiles them.
- Devices on two threads run the compute path at once (Section 7).
- A panic in a compute pass ends the pass, and the encoder still finishes.
- The objects of a device keep it alive after every `Device` handle is dropped.
- A submitted dispatch runs exactly once.
- Buffer descriptions of size 0, without a usage, or with an unknown usage are refused, each use of a buffer needs its usage flag, and buffers in the memory types `Upload` and `ReadBack` with initial data read back correctly.
- A typed buffer parameter is refused, and the buffer stays unchanged.
- A dispatch past the limit of the device is refused, and a dispatch of 0 groups runs nothing.
- The fields of a `ConstantBuffer` or a `ParameterBlock` cannot be set as uniform data.

The tests compile their kernels with the global session of `slang-compiler`, so they also show that the loader of `slang-glslang-sys` serves a device of slang-rhi.

### 10.4. What the Tests Do Not Cover

The tests run under the debug device of slang-rhi only, and none runs under the validation layer of Vulkan, which the crate does not turn on (RHQ-6). These are not verified:

- Valid use on the NVIDIA adapter under the Khronos validation layer.
- A device whose Vulkan driver lacks `nullDescriptor` (RHQ-7).
- What slang-rhi and a driver do with a buffer larger than the device allows (RHQ-8).
- The path of a failed rebind, after which slang-rhi returns no root object, which was read in the source and not run.
- A message of the debug callback from a thread of the driver.
- That the SPIR-V that the device runs is optimized, beyond the absence of a message about `spirv-opt`.

## 11. Out of Scope Until M10

These parts of slang-rhi are not wrapped in M9. M10 adds those that the `rhi` backend of `slango` needs, in the design of `docs/design/slango.md`:

- Render pipelines, render passes, textures, texture views, samplers, input layouts, and surfaces.
- Fences, mapped buffers, copy and clear commands, and timestamp queries.
- Buffers with a format, and the typed buffer ranges that need them.
- Lookup of nested fields and of array elements by name, the fields of a `ConstantBuffer` or `ParameterBlock` parameter, specialization arguments of shader objects, and shader objects other than the root object and its entry points.
- Other queues, several command buffers in one submission with fences, and the persistent caches.
- Native handles, interoperation with other APIs, and acceleration structures, which M14 needs.

A user who needs one of them in M9 reaches it through `slang-rhi-sys` under its safety contract.

## 12. Crate Documentation

RHI-17. The crate-level documentation MUST state the compute path with a runnable example of the `gpu` tier marked `no_run`, that the crate always builds Slang and slang-rhi from source and what that build needs, the platforms of Section 7 of the sys design, that no type that holds a native object is `Send` or `Sync` and why, that the debug callback object of a device is never freed, that a panic in the debug callback or a C++ exception that escapes slang-rhi aborts the process, and a `# Features` section that says that the crate has no Cargo features. The methods of the compute path point to that example instead of repeating it.

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
5. The conditions of valid use that the crate does not check, which are RHQ-7 and RHQ-8, can reach the driver.

## 14. Operational Considerations

### 14.1. Cargo Features

The crate has no Cargo features. It always builds the vendored Slang and slang-rhi (RHI-1).

### 14.2. Environment Variables

The crate reads no environment variable. Its dependencies read those of Section 12.2 of the sys design and of `slang-compiler`.

### 14.3. Platforms and Minimum Supported Rust Version

The crate builds where `slang-rhi-sys` builds, which is Linux with the GNU C library until the code step for Windows of M9 (Section 7.3 of the sys design), and is tested on x86_64 Linux on lavapipe and on an NVIDIA adapter. It uses the workspace's `rust-version`.

### 14.4. Failure Modes

| Failure | Reported by | Recovery |
| --- | --- | --- |
| No Vulkan loader, driver, or adapter | `ErrorKind::NoDevice` from `Device::new` | Install the Vulkan loader and a driver, such as lavapipe |
| An adapter of another device type, or one that slang-rhi no longer lists | `ErrorKind::InvalidArgument` from `Device::new` | Pass an adapter of `adapters` for the device type of the options |
| A program of another session | `ErrorKind::InvalidArgument` from `create_shader_program` | Load and link the program through `Device::slang_session` |
| A program with a specialization parameter left | `ErrorKind::InvalidArgument` from `create_shader_program` | Specialize the program before it is linked |
| A program without exactly one compute entry point, or a shader program of another device | `ErrorKind::InvalidArgument` from `create_compute_pipeline` | Link one compute entry point on the same device |
| A shader that does not compile for the device | `ErrorKind::Compile` with the diagnostics | Fix the shader source that the diagnostics name |
| A buffer of size 0, without a usage, or with an unknown usage | `ErrorKind::InvalidArgument` from `create_buffer` | Give a size and the usage flags of each use |
| A read of a buffer without `COPY_SOURCE` | `ErrorKind::InvalidArgument` from `read_buffer` | Create the buffer with `COPY_SOURCE` |
| A buffer without the usage that its range needs, a typed buffer range, or a parameter without a binding range | `ErrorKind::InvalidArgument` from `set_buffer` | Add `UNORDERED_ACCESS` or `SHADER_RESOURCE`, declare a structured or raw buffer, or use the offset of a buffer parameter |
| A parameter without uniform data, or data longer than the parameter | `ErrorKind::InvalidArgument` from `set_data` | Use the offset of a uniform parameter and data of its size |
| A parameter or a range out of bounds | `ErrorKind::InvalidArgument` | Use the offsets that `offset_of` returns and the size of the buffer |
| A dispatch without a bound pipeline, or past the limit of the device | `ErrorKind::InvalidArgument` from `dispatch` | Bind a pipeline, and split the work into dispatches within the limit |
| A command buffer of another device | `ErrorKind::InvalidArgument` from `submit` | Submit each command buffer to a queue of its own device |
| A panic in the debug callback, or a call from it into this crate during a call of slang-rhi | The process aborts | Do not panic in the callback, and do not use the objects of this crate from it |

### 14.5. Logs

The crate emits no logs and has no logging dependency. slang-rhi reports its messages through the debug callback, which the user supplies. This answers Q-16 of the architecture design for this crate.

## 15. Assumptions to Confirm

Each row is a fact that this design relies on and that is not verified. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

No assumption is open. RHA-1 to RHA-4 were confirmed, and Sections 3, 6.4, 6.5, 6.7, and 7 state the results.

## 16. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row. An identifier is never reused.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| RHQ-1 | Should `check-slang-call-guard` check that every call of this crate into slang-rhi and into Slang sits alone in its abort guard, as it checks for `slang-compiler`? Extending the checker changes a mechanism that `AGENTS.md` declares frozen, and leaving it out keeps a deterministic check as a judgement rule (Section 8.1) | The owner of the project, by a decision | M9 |
| RHQ-2 | Will slang-rhi document which of its objects tolerate use from several threads, so that the types of this crate and of the `rhi` backend of `slango` can be `Send` and `Sync`? | The owner, through an issue in the slang-rhi repository, `shader-slang/slang-rhi`, for which `docs/upstream-reports.md` holds no entry yet | M10 |
| RHQ-3 | Can the crate free the debug callback object of a device, if slang-rhi offers a notice when a native device is freed? | This document, after RHQ-2 is asked upstream | Not assigned |
| RHQ-4 | Should `create_shader_program` and `create_compute_pipeline` return the warnings of Slang on success, as `slang-compiler` does with `Compiled`? | This document | M10 |
| RHQ-5 | Should the wrappers get the raw accessors `from_raw`, `into_raw`, and `as_raw` that COM-11 of the coding standard asks of every interface wrapper? The crate has none, so a program that combines it with other native code cannot hand over or adopt an object (Section 5.1) | The owner of the project, by a decision | Not assigned |
| RHQ-6 | Should the crate offer the validation layer of the graphics API through `IRHI::enableDebugLayers`, which affects the whole process and cannot be turned off again? The crate does not call it, so the `gpu` tests run under the debug device of slang-rhi only, and no test has the Khronos layer check valid use (Section 10.4) | The owner of the project, by a decision | Not assigned |
| RHQ-7 | Should `Device::new` require the Vulkan feature `nullDescriptor`, on which a buffer parameter without a bound buffer relies? The crate documents the limit and does not check it (Section 6.7) | The owner of the project, by a decision | Not assigned |
| RHQ-8 | Should `create_buffer` check the size against the largest buffer that the device allows? The crate does not, and what slang-rhi and a driver do with a larger size, such as `u64::MAX`, is not verified (Section 6.6) | The owner of the project, by a decision | Not assigned |
| RHQ-9 | Should CI run `check-miri-rhi`? It clones Slang a second time and builds it natively for the build scripts, which took 8 min 33 s on its first run in the development container (TOOL-52 of `docs/design/tooling.md`) | The owner of the project, by a decision | Not assigned |

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
| RHI-7 | 6.5 | A shader program takes only a linked, fully specialized program of the device's session, a compute pipeline only one compute entry point, and both collect diagnostics. |
| RHI-8 | 6.6 | Buffer size, usage, initial data, and read-back ranges are checked before any native call. |
| RHI-9 | 6.7 | A compute pass exists only inside a closure that begins it and ends it on every path, including a panic. |
| RHI-10 | 6.7 | `offset_of` finds a top-level field by name as `ShaderCursor` does, with its uniform size and range count. |
| RHI-11 | 6.7 | `set_data` and `set_buffer` check the parameter, offsets, ranges, binding types, buffer usage, and the device before each call. |
| RHI-12 | 6.8 | One `Error` with a kind, an operation, the result code, and diagnostics, and one mapping of result codes. |
| RHI-13 | 7 | Types that hold native objects are neither `Send` nor `Sync`, and value types are both, with assertion tests. |
| RHI-14 | 7 | Every call through `IRHI` holds one crate-private lock, taken after the per-thread guard, and the crate never destroys the instance. |
| RHI-15 | 8.1 | Every native call runs in the per-thread guard, the Slang call guard of `slang-compiler`, and the crate's abort guard, innermost. |
| RHI-16 | 10.2 | Miri tests the counting, the pass closure, the guards, the checks, `Binding`, `submit`, and the result mapping against a fake of five interfaces. |
| RHI-17 | 12 | The crate documentation states the path, the build, the platforms, the threading, the leaked callback, the aborts, and the features. |
| RHI-18 | 8.2 | A per-thread mark makes a reentrant call into slang-rhi panic before it starts. |
| RHI-19 | 8.3 | A per-thread collector returns the messages of Slang with the call of the same thread that caused them. |
| RHI-20 | 9.1 | The crate checks every condition of valid use of the graphics API that a safe call could break and that it can check. |
| RHI-21 | 6.7 | `submit` takes its command buffers, checks their device, and releases them after the call. |
