# `slang-rhi` Design

| Field | Value |
| --- | --- |
| Status | Draft |
| Date | 2026-11-17 |
| Applies to | `crates/slang-rhi` |

## Abstract

This document specifies `slang-rhi`, the safe Rust crate over the raw bindings of `slang-rhi-sys`, in the scope of milestone M9: the path of one compute dispatch. A program lists the adapters of a backend, opens a device with a global session of `slang-compiler`, creates a shader program from a linked Slang program of the device's session and a compute pipeline from it, creates buffers, encodes a compute pass that binds parameters by name and dispatches, submits the work, and reads a buffer back. The document defines the ownership of every object, the borrows that turn the unowned pointers of slang-rhi into lifetimes, the errors, the threading rules that follow from slang-rhi documenting none, the guards around calls into native code, the tests, and what waits for M10.

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

- abort guard: The crate-private function of this crate that runs one call into native code and aborts the process when a C++ exception reaches it, as FFI-7 of the coding standard requires (Section 8).
- command buffer: A recorded list of GPU commands that a queue executes.
- command encoder: The object that records commands into a command buffer.
- compute pass: The part of a command encoder in which compute pipelines are bound and dispatched.
- compute pipeline: A GPU object that holds a compiled compute shader.
- debug callback: The object through which slang-rhi reports messages of the validation layer, the driver, and Slang (Section 3.3 of the sys design).
- queue: The object of a device that executes command buffers.
- root shader object: The shader object that holds the global parameters of the program of a bound pipeline.
- shader object: The slang-rhi object that holds the values and resources of the parameters of a program or of one of its entry points.
- shader offset: The position of a parameter in a shader object, made of a byte offset of uniform data, a binding range index, and an index in the array of that range.
- shader program: The slang-rhi object that holds a linked Slang program for a device.

## 3. Facts

This section is non-normative. Section 3 of the sys design states the facts about slang-rhi on which this design relies, and this section adds the facts of the compute path. Paths are in the slang-rhi tree of the sys design.

The path of one compute dispatch is the one of `tests/test-compute-trivial.cpp:6-45`. It creates a device, takes the Slang session of the device with `IDevice::getSlangSession` (`include/slang-rhi.h:3401`), loads and links a program there, creates a shader program (`:3618`) and a compute pipeline (`:3656`), creates buffers (`:3456`), takes a queue (`:3555`) and a command encoder (`:2862`), begins a compute pass (`:2608`), binds the pipeline, which returns the root shader object (`:2528`), sets the parameters, dispatches (`:2531`), ends the pass (`:2496`), finishes the encoder (`:2769`), submits (`:2886`), waits (`:2902`), and reads the result (`:3704`).

Every descriptor on that path is passed by `const` reference, and no method on it takes an aggregate by value. `ShaderOffset` holds three `uint32_t` (`:1655`). `IShaderObject` sets data by `setData(const ShaderOffset&, const void*, Size)` and a resource by `setBinding(const ShaderOffset&, const Binding&)` (`:1750`, `:1753`), and offers no lookup by name. The helper `ShaderCursor` finds a field by name through the reflection of the element type layout of the object: `findFieldIndexByName`, the field's uniform offset, and `getFieldBindingRangeOffset` (`include/slang-rhi/shader-cursor.h:87`, `:169-194`).

`createShaderProgram` takes an optional out-parameter for diagnostics (`:3618`). slang-rhi reports other messages through the debug callback of the device, from the calling thread or from a thread of the driver (`docs/error-handling.md:39-63`).

The `IRHI` instance is one object of the process, which any thread reaches through `rhiGetInstance`, and slang-rhi documents no threading rule for it (Section 3.3 of the sys design).

### 3.1. Known Upstream Bugs

This is the one list of the known defects of the recorded slang-rhi. It has the columns of Section 3.12 of the `slang-compiler` design and follows the rules stated there, and every path in the column "Source" is in the slang-rhi tree.

The crate does not guard against these defects, as Hard Rule 14 of `AGENTS.md` requires for Slang. The checks of RHI-8 and RHI-11 are not guards, because a safe interface checks the bounds of an index whatever slang-rhi does.

| Identifier | Defect and smallest input | Call | Outcome | Source | Report |
| --- | --- | --- | --- | --- | --- |
| RKB-1 | The range check of `readBuffer` adds `offset` and `size` with no guard against overflow, so a sum that wraps lets a range past the end of the buffer pass. By a reading, the smallest input is a buffer of any size read with `offset` set to `SIZE_MAX` and `size` set to 1, whose sum wraps to 0 | `IDevice::readBuffer` | Not observed, because the defect was read in the source and not reproduced. By the reading, the check passes and the backend reads from `offset` bytes past the start of the buffer, as the CPU backend does with `memcpy` (`src/cpu/cpu-buffer.cpp:61`) | `if (offset + size > bufferImpl->m_desc.size)` in the Vulkan backend (`src/vulkan/vk-device.cpp:1872`), where `Offset` and `Size` are `size_t` (`include/slang-rhi.h:57-58`) and `m_desc` is a `BufferDesc` (`src/rhi-shared.h:95`) whose `size` is `uint64_t` (`include/slang-rhi.h:745`). The CPU, WebGPU, CUDA, Metal, D3D12, and D3D11 backends have the same check (`src/cpu/cpu-buffer.cpp:57`, `src/wgpu/wgpu-device.cpp:470`, `src/cuda/cuda-device.cpp:534`, `src/metal/metal-device.cpp:377`, `src/d3d12/d3d12-device.cpp:1730`, `src/d3d11/d3d11-device.cpp:567`), and so does the debug layer (`src/debug-layer/debug-device.cpp:1221`) | Not reported |

## 4. Crate Structure

### 4.1. Package and Dependencies

RHI-1. The crate MUST depend on `slang-rhi-sys`, on `slang-compiler-sys`, on `slang-compiler` with the feature `vendored`, and on `bitflags`, and on no graphics crate. It MUST have no Cargo features.

`slang-rhi-sys` always links the vendored Slang (Section 4.1 of the sys design), and Cargo unifies the feature `vendored` of `slang-compiler-sys` in the whole build graph. `slang-compiler` without its own feature `vendored` would then create global sessions without the loader of `slang-glslang-sys`, and every program would get SPIR-V that is not optimized (SGL-12 of `docs/design/slang-glslang-sys.md`). Enabling the feature here installs the loader on every global session that `slang-compiler` creates.

The crate calls the reflection functions of `slang-compiler-sys` for the lookup of Section 6.7, so it depends on that crate directly. `bitflags` represents the usage flags of a buffer, which a caller combines, as TYPE-8 of the coding standard requires. The code step adds it under DEP-2.

The crate takes the licence `MIT OR Apache-2.0` of the workspace (ARCH-36 of the architecture design), sets `publish = false` until the change that prepares the second release (TOOL-35 of `docs/design/tooling.md`), and stays out of `default-members` and of every recipe that passes `--workspace`, because building it builds Slang and slang-rhi from source (TOOL-32).

### 4.2. Unsafe Code

RHI-2. The crate root MUST carry the opt-out `#![expect(unsafe_code, reason = "...")]` of Section 4.4 of the coding standard, and every unsafe operation MUST sit in the private modules that UNS-2 requires: the owning interface pointer, the abort guard, the debug callback, and the lookup of shader offsets.

The crate calls the generated thunks of `slang-rhi-sys` and the reflection functions of `slang-compiler-sys`, which are unsafe, as `slang-compiler` calls those of Slang. It therefore joins the crates that may hold unsafe code (Section 3.3 of the architecture design).

## 5. Ownership

### 5.1. The Owning Interface Pointer

RHI-3. The crate MUST hold every interface of `slang-rhi-sys` with a reference in one crate-private owning interface pointer type, generic over the interface, as COM-1 of the coding standard requires, and MUST call `addRef` and `release` only there.

The type follows the owning interface pointer of `slang-compiler` (SLC-2 of the `slang-compiler` design) without its deferral of releases, because the deferral serves the reentrancy rule of Slang, which Section 8 meets through the guard of `slang-compiler` itself. The crate keeps its own type because the type of `slang-compiler` is private.

### 5.2. References Returned by Each Call

This table records, for each call of the compute path, whether the returned pointer carries a reference, as COM-2 of the coding standard requires:

| Call | Returns | Owned or borrowed | Rust type |
| --- | --- | --- | --- |
| `rhiGetInstance` | `IRHI*` | Neither. A process-wide object that is never freed while the crate is in use (RHI-14) | Crate-private |
| `IRHI::getAdapter` | `IAdapter*` | Borrowed from the backend until `rhiDestroyInstance` | None. The crate copies the information into `Adapter` (Section 6.3) |
| `IRHI::createDevice` | `IDevice**` | Owned | `Device` |
| `IDevice::getSlangSession` | `slang::ISession**` | Owned | `slang_compiler::Session`, through `Session::from_raw` with the device's global session |
| `createShaderProgram`, `createComputePipeline`, `createBuffer`, `getQueue`, `createCommandEncoder`, `finish` | An interface through `**` | Owned (RSA-7 of the sys design) | `ShaderProgram`, `ComputePipeline`, `Buffer`, `Queue`, `CommandEncoder`, `CommandBuffer` |
| `ICommandEncoder::beginComputePass` | `IComputePassEncoder*` | Borrowed from the encoder | `ComputePass<'e>`, which borrows the encoder mutably |
| `IComputePassEncoder::bindPipeline` | `IShaderObject*` | Borrowed from the pass until its next `bindPipeline` | `ShaderObject<'p>`, which borrows the pass mutably |
| `IShaderObject::getEntryPoint` | `IShaderObject**` | Owned, and valid only while its root object is | `ShaderObject<'p>`, which borrows its root object mutably |
| `IShaderObject::getElementTypeLayout` | `slang::TypeLayoutReflection*` | Borrowed from the program of the bound pipeline | Used only inside one call of the crate (Section 6.7) |

RHI-4. Every wrapper of an owned interface MUST hold the `Device` that created it, as COM-5 of the coding standard requires, and a `Device` MUST hold the `slang_compiler::GlobalSession` that it was created with and its `slang_compiler::Session`. Every pointer that the table marks borrowed MUST be held only by a value whose lifetime borrows its owner as the table states.

A native child holds a strong reference to its device (Section 3.3 of the sys design), so the Rust parent field is not what keeps the device alive. It keeps the field order of COM-5 and makes the device reachable from every object for the check of RHI-7. The borrows make the compiler enforce the validity rules of Section 3.3 of the sys design at no run-time cost, as SLC-19 of the `slang-compiler` design does for reflection handles.

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

This illustration shows the compute path. It is an illustration of the planned API, so the names of `slang-rhi` in it do not exist yet. It doubles the values of a buffer with a kernel whose module declares `RWStructuredBuffer<float> values;` and `uniform float factor;`:

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
    queue.submit(&[&encoder.finish()?])?;
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

`DeviceOptions::new(global_session: slang_compiler::GlobalSession) -> DeviceOptions` takes the global session that the device compiles with. Its setters take and return `&mut Self` (API-6): `device_type(DeviceType)`, whose default is `DeviceType::Default`, `adapter(&Adapter)`, whose default is none, which lets slang-rhi choose, `validation(Toggle)`, whose default is `Toggle::Off`, and `debug_callback(impl Fn(&DebugMessage<'_>) + Send + Sync + 'static)`, whose default is none.

RHI-5. `Device::new(options: &DeviceOptions) -> Result<Device>` MUST pass the global session of the options in `SlangDesc::slangGlobalSession`, and MUST NOT let slang-rhi create a global session of its own. With an adapter, it MUST check that its device type equals the device type of the options, or fail with the kind `InvalidArgument`, and MUST fetch the adapter again by its device type and position for the call, failing with `InvalidArgument` when the position no longer exists.

A global session of slang-rhi's own would have no loader of `slang-glslang-sys` and would emit SPIR-V that is not optimized (Section 4.1). The program of a shader program must come from the device's session (RHI-7), and a global session that the caller holds is the one place where the caller's modules and the device meet.

`Device` offers `info(&self) -> DeviceInfo`, a record of the device type, the name of the API, and the name of the adapter, `slang_session(&self) -> &slang_compiler::Session`, and the methods of Sections 6.5 to 6.8. `Device` is `Clone`, and a clone adds a reference to the same device.

RHI-6. The crate MUST implement `IDebugCallback` with a heap-allocated object whose `handleMessage` is defined `extern "system"`, stored in the generated field through a conversion of the function pointer, and MUST never free that object. `handleMessage` MUST build a `DebugMessage<'_>` that borrows the message for the call only, and call the user's closure.

slang-rhi gives no notice when a native device is freed, and children keep the native device alive after every Rust wrapper is gone (Section 3.3 of the sys design), so no Rust value knows when the callback can no longer be called. A freed callback object that slang-rhi calls later is a use after free, while a leaked one costs one allocation and the user's closure for each device that has a callback, which UNS-4 of the coding standard permits (RHQ-3). The object is never unregistered, so FFI-8 holds with the context alive for the life of the process.

The closure takes the bounds `Send + Sync + 'static`, which CONC-6 of the coding standard permits for a callback that a foreign library keeps: slang-rhi calls it from several threads at once and from threads of the driver (Section 3), and keeps it beyond any borrow. A panic of the closure reaches the non-unwinding boundary and aborts the process, as FFI-7 requires for a callback that reports no result code. The crate documentation states this.

`DebugMessage` offers `message_type(&self) -> DebugMessageType`, `source(&self) -> DebugMessageSource`, and `text(&self) -> Cow<'_, str>`, converted with each invalid sequence replaced by U+FFFD, because a message must not be lost to an error about its encoding. Without a callback, the crate passes null, and slang-rhi decides what happens to its messages (Section 15, RHA-3).

### 6.5. Shader Programs and Compute Pipelines

RHI-7. `Device::create_shader_program(&self, program: &slang_compiler::LinkedProgram) -> Result<ShaderProgram>` MUST check that the program belongs to the session of the device, by comparing the raw pointer of the session that `LinkedProgram::session` returns with that of `slang_session`, and MUST fail with the kind `InvalidArgument` otherwise. It MUST pass the program as `slangGlobalScope` with no entry points and the linking style `SingleProgram`, and pass and read the diagnostics out-parameter.

slang-rhi takes the Slang session from the program and generates code for the target of that session (Section 3.3 of the sys design), and only the device's session has the target that the device runs (Section 15, RHA-2). With no entry points, slang-rhi uses the linked program as it is (`src/shader.cpp:58-62`), so the program that the caller linked is the program that runs. `LinkedProgram::session` is an addition to `slang-compiler` (SLC-40 of the `slang-compiler` design).

A failure with diagnostics has the kind `Compile` and holds their text, and warnings on success are dropped in M9, because the result type carries no diagnostics. Whether to return them, as `slang-compiler` does with `Compiled`, is RHQ-4.

`Device::create_compute_pipeline(&self, program: &ShaderProgram) -> Result<ComputePipeline>` creates the pipeline with the default compilation policy. `createShaderProgram` generates no code (`src/vulkan/vk-device.cpp:2521-2538`). With the default policy and the default pipeline compilation mode `Serial` of a device (`include/slang-rhi.h:3350`, `src/device.h:529-542`), `createComputePipeline` generates the code of the program at this call (`src/device.cpp:755-769`, `src/shader.cpp:110`, `src/device.cpp:447`), so a failure of that compile has the kind `Compile`.

Only a program that is still specializable gets a virtual pipeline instead, whose code slang-rhi generates when a command encoder finishes (`src/device.cpp:758-764`, `src/pipeline-resolver.cpp:96-106`).

### 6.6. Buffers

`BufferDesc::new(size: u64, usage: BufferUsage) -> BufferDesc` creates a descriptor with the element size 0 and the memory type `MemoryType::DeviceLocal`, and `with_element_size(u32)` and `with_memory(MemoryType)` return changed copies. `BufferUsage` is a set of flags of `bitflags`, with one flag for each flag of `rhi::BufferUsage`. `Buffer` offers `size(&self) -> u64`.

RHI-8. `Device::create_buffer(&self, desc: &BufferDesc, initial: Option<&[u8]>) -> Result<Buffer>` MUST fail with the kind `InvalidArgument` when `initial` is given and its length differs from the size of the descriptor. `Device::read_buffer(&self, buffer: &Buffer, offset: u64, out: &mut [u8]) -> Result<()>` MUST fail with `InvalidArgument` when the buffer belongs to another device or when `offset` plus the length of `out` exceeds the size of the buffer, with checked arithmetic, before it calls `readBuffer`.

`createBuffer` receives no length for the initial data (`include/slang-rhi.h:3456-3460`), and the Vulkan backend copies the size of the descriptor from it (`src/vulkan/vk-buffer.cpp:390-396`, `:443-447`, `:457`), so a short slice is a read out of bounds in native code. `readBuffer` writes the requested range into the caller's pointer (`include/slang-rhi.h:3704`). The range check of slang-rhi can wrap (RKB-1 in Section 3.1), and the crate checks the range itself in any case, as a safe interface does.

### 6.7. Command Encoding and Parameters

`Queue` comes from `Device::queue(&self) -> Result<Queue>`, which takes the graphics queue, and offers `encoder(&self) -> Result<CommandEncoder>`, `submit(&self, command_buffers: &[&CommandBuffer]) -> Result<()>`, and `wait_on_host(&self) -> Result<()>`.

RHI-9. `CommandEncoder::compute_pass<R>(&mut self, f: impl FnOnce(&mut ComputePass<'_>) -> Result<R>) -> Result<R>` MUST begin the pass, run `f`, and end the pass whether `f` succeeds or fails, and MUST NOT offer a pass value that outlives the closure. `CommandEncoder::finish(self) -> Result<CommandBuffer>` consumes the encoder.

A pass that is begun and never ended leaves the encoder in a state that slang-rhi does not document. A guard that ends the pass in `Drop` would break UNS-4, because a leaked guard skips `end`, so the closure-scoped form that UNS-4 recommends ends it on every path. A panic in `f` unwinds through the crate without ending the pass, and the encoder is then dropped unfinished, which releases it.

`ComputePass` offers `bind_pipeline(&mut self, pipeline: &ComputePipeline) -> Result<ShaderObject<'_>>`, which fails with the kind `Failed` when slang-rhi returns null, and `dispatch(&mut self, thread_groups: [u32; 3]) -> Result<()>`, which fails with `InvalidArgument` when no pipeline is bound in the pass.

`ShaderObject<'p>` offers `offset_of(&self, name: &str) -> Result<Option<ShaderOffset>>`, `set_data(&mut self, offset: ShaderOffset, data: &[u8]) -> Result<()>`, `set_buffer(&mut self, offset: ShaderOffset, buffer: &Buffer) -> Result<()>`, and `entry_point(&mut self, index: usize) -> Result<ShaderObject<'_>>`. `ShaderOffset` is a `Copy` record with the public fields `uniform_offset`, `binding_range_index`, and `binding_array_index`, each a `u32`.

RHI-10. `offset_of` MUST find a field of the element type layout of the object by name as `ShaderCursor::getField` does (Section 3): the field index through `spReflectionTypeLayout_findFieldIndexByName`, the uniform offset of the field's variable layout, and the binding range through `spReflectionTypeLayout_getFieldBindingRangeOffset`, with `None` for a name that the layout does not have. It finds a field of the top level of the object only (RHA-4).

RHI-11. Before each call, `set_data` MUST check that the uniform offset plus the length of `data` does not exceed the uniform size of the element type layout, and `set_buffer` MUST check that the binding range index is below the binding range count, that the array index is below the binding count of that range, that the binding type of the range is a buffer type, and that the buffer belongs to the device of the pass. Each check MUST fail with the kind `InvalidArgument`.

The header states no check of these bounds by slang-rhi, and a write past the uniform data of an object or into a range that does not exist is an access out of bounds in native code. These are the checks of an index that a safe interface needs, so they stay whatever slang-rhi checks (Hard Rule 14 of `AGENTS.md` for the analogous case of Slang).

`set_buffer` builds a `Binding` of `slang-rhi-sys` with the type of a buffer, the buffer's interface pointer in `resource`, a null `resource2`, and the range of the whole buffer, and passes it by pointer while the `Buffer` that it borrows keeps its own reference (Section 6.4 of the sys design).

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
| `Slang` | A failure of `slang-compiler`, such as the adoption of the device's session |

`ErrorKind` and `Operation` are `#[non_exhaustive]`. The `Display` text names the operation and the kind and holds no advice (ERR-4).

## 7. Threading

RHI-13. No type of the crate that holds a pointer of slang-rhi or a wrapper of `slang-compiler`, which are `DeviceOptions`, `Device`, `ShaderProgram`, `ComputePipeline`, `Buffer`, `Queue`, `CommandEncoder`, `ComputePass`, `ShaderObject`, and `CommandBuffer`, MUST be `Send` or `Sync`. `Adapter`, `DeviceInfo`, `BufferDesc`, `ShaderOffset`, the enumerations, `BufferUsage`, and `Error` MUST be `Send` and `Sync`. Each auto trait that a type implements gets a compile-time assertion test (CONC-1).

slang-rhi documents no threading rule for its objects (Section 3.3 of the sys design), so FFI-9 of the coding standard forbids `Send` and `Sync` for them. The wrappers of `slang-compiler` that a device holds are neither (SLC-21 of the `slang-compiler` design). The `rhi` backend of `slango` in M10 therefore cannot make its public types `Send` and `Sync` on this crate without a documented rule, which the owner accepted as a possible breaking version 0.2 of `slango` (Section 15.11 of the architecture design, RHQ-2).

RHI-14. The crate MUST make every call through the `IRHI` instance, which are `getAdapter`, `isDeviceTypeSupported`, `getDeviceTypeName`, and `createDevice`, while it holds one crate-private `static` `Mutex<()>`, and MUST NOT call `rhiDestroyInstance`.

The instance is one object of the process that every thread reaches, so a type that is not `Send` does not keep two threads apart, and slang-rhi states no rule for it. The lock makes the calls of this crate on it one at a time. A call of `rhiDestroyInstance` while a device lives aborts the process (Section 3.3 of the sys design), and the crate cannot know whether a device lives, so it never calls it, and the instance lives until the process exits.

The lock is library state in a `static`, which CONC-4 of the coding standard rules out. No rule of the standard covers a process-wide native object without a threading rule, so the code step that adds the lock looks the topic up in `docs/rust-references.md` and adds a rule CONC-8 to the standard, as Hard Rule 13 of `AGENTS.md` and Section 19 of the coding standard require. The rule permits one crate-private lock for a process-wide native object that is reachable from every thread and whose library states no threading rule.

Objects of two devices are used from two threads at once when a program opens a device on each thread. That this is sound rests on RHA-1.

## 8. Guards Around Native Calls

RHI-15. Every call of the crate into slang-rhi, and every call of a reflection function of `slang-compiler-sys`, MUST run inside the crate-private abort guard as the innermost frame around that one call, as FFI-7 of the coding standard requires, and inside `slang_compiler::with_slang_call_guard` (SLC-41 of the `slang-compiler` design).

slang-rhi calls Slang, through which a C++ exception can escape (Section 6.4 of the sys design), and an exception that reaches a destructor of the crate while it unwinds would call native code again in the middle of the unwinding. The abort guard stops the process first, as the guard of `slang-compiler` does (Section 11.6 of the `slang-compiler` design).

slang-rhi calls Slang on the device's session and on programs that `slang-compiler` wraps, and it calls the debug callback, whose closure can call `slang-compiler`, while it is inside Slang. A call of Slang from inside Slang is reentrant, which Slang documents as unsupported. With the Slang call guard of `slang-compiler` raised for the duration of each call into slang-rhi, such a call of the closure into `slang-compiler` panics, which aborts at the boundary of the callback, instead of reentering Slang. The guard also defers the releases that `slang-compiler` would make during the call (SLC-28 of the `slang-compiler` design).

The abort guard of this crate is its own crate-private function, because that of `slang-compiler` is private and frozen (Technology Decisions of `AGENTS.md`). `check-slang-call-guard` checks the guard of `slang-compiler` only, so the placement of this guard is a judgement rule until a tool covers it, as Hard Rule 4 of `AGENTS.md` allows, and whether the checker is extended to this crate is RHQ-1.

## 9. Soundness Invariants

These are the invariants on which the unsafe code of the crate relies, each with the requirement that keeps it:

| Invariant | Kept by |
| --- | --- |
| Every interface pointer that the crate dereferences points to a live object | The owning interface pointer (RHI-3) and the borrows of the unowned pointers (RHI-4) |
| No pass encoder or root object is used after its owner ends or rebinds it | The mutable borrows of `ComputePass` and `ShaderObject`, and the closure of RHI-9 |
| Every pointer and length passed to slang-rhi is in bounds | RHI-8 and RHI-11 |
| Every object that a `Binding` names lives through the call | The borrow of the `Buffer` in `set_buffer` |
| The debug callback object lives for every call that slang-rhi makes to it | RHI-6 |
| No object is used from two threads at once | RHI-13 and RHI-14 |
| No C++ exception unwinds through the crate, and no panic unwinds into slang-rhi | RHI-15, and the non-unwinding callback of RHI-6 |
| Every component that a shader program takes comes from the device's session | RHI-7 |

## 10. Tests

### 10.1. Layout and Tiers

The crate has unit tests in its modules and one integration-test binary, `tests/rhi/`, whose module `gpu` holds the tests that need an adapter (ARCH-22 of the architecture design). The recipe `test-rhi` runs them with the nextest profile `gpu`, which the CI job `vendored` runs on Linux on lavapipe (TOOL-52 and Section 7.4 of `docs/design/tooling.md`). The crate has no `ffi` tier of its own, because every call of the crate into slang-rhi needs a device, which needs an adapter.

### 10.2. The Fake for Miri

RHI-16. The unit tests MUST test, under Miri, the owning interface pointer, the closure of RHI-9, the bounds checks of RHI-8 and RHI-11, the building of `Binding`, and the mapping of result codes, against a fake of the interfaces `IBuffer`, `IComputePassEncoder`, and `IShaderObject` built on the generated vtable structs, with its vtable functions defined `extern "system"`.

TEST-7 of the coding standard asks for a fake of the interfaces, and the fake of `slang-compiler` is the model (SLC-25 of the `slang-compiler` design). The fake covers the interfaces whose counting and borrowing the unsafe code relies on. A fake of `IDevice`, `IRHI`, and `ICommandEncoder` would need a function for each of their many slots and would test only that the crate passes descriptors, which the `gpu` tests do on a real device. The bounds checks are pure functions over the numbers that the reflection calls return, so they run under Miri without the reflection of Slang (UNS-2).

### 10.3. GPU Tests

The `gpu` tests run on any adapter, which is lavapipe in CI and the development container's NVIDIA RTX 4070 or lavapipe locally. Each test fails, and never skips, when it finds no Vulkan adapter (ARCH-23). They cover:

- `adapters(DeviceType::Vulkan)` returns at least one adapter, and a device opens on each.
- The compute path of Section 6.2 doubles a buffer, with a parameter set by `set_data` and a buffer by `set_buffer`, and reads the result back.
- A kernel with a uniform parameter of an entry point, set through `entry_point(0)`.
- A shader program from a program of another session fails with `InvalidArgument`, a call of `set_data` past the uniform data and of `set_buffer` with a range index past the count fail with `InvalidArgument`, and a buffer with initial data of the wrong length fails before any native call.
- A shader that does not compile fails with `Compile` and holds the diagnostics.
- A dispatch without a bound pipeline fails with `InvalidArgument`.
- A debug callback that counts its calls, on a device with validation, which receives every message that slang-rhi reports and outlives the `Device`.

The tests compile their kernels with the global session of `slang-compiler`, so they also show that the loader of `slang-glslang-sys` serves a device of slang-rhi.

## 11. Out of Scope Until M10

These parts of slang-rhi are not wrapped in M9. M10 adds those that the `rhi` backend of `slango` needs, in the design of `docs/design/slango.md`:

- Render pipelines, render passes, textures, texture views, samplers, input layouts, and surfaces.
- Fences, mapped buffers, copy and clear commands, and timestamp queries.
- Lookup of nested fields and of array elements by name, specialization arguments of shader objects, and shader objects other than the root object and its entry points.
- Other queues, several command buffers in one submission with fences, and the persistent caches.
- Native handles, interoperation with other APIs, and acceleration structures, which M14 needs.

A user who needs one of them in M9 reaches it through `slang-rhi-sys` under its safety contract.

## 12. Crate Documentation

RHI-17. The crate-level documentation MUST state the compute path with a runnable example of the `gpu` tier marked `no_run`, that the crate always builds Slang and slang-rhi from source and what that build needs, the platforms of Section 7 of the sys design, that no type that holds a native object is `Send` or `Sync` and why, that the debug callback object of a device is never freed, that a panic in the debug callback or a C++ exception that escapes slang-rhi aborts the process, and a `# Features` section that says that the crate has no Cargo features.

## 13. Security Considerations

The threat model has four parts:

1. A program passes shader source that an attacker controls through `slang-compiler` to a device, and parameter values and buffer contents that the attacker controls to a dispatch.
2. A program calls the API in an order or with values that its author did not foresee, such as a range past the end of a buffer.
3. slang-rhi and the graphics driver run inside the program's process and report messages through the debug callback from threads of the driver.
4. The debug callback runs the user's code inside a call of slang-rhi.

Unsafe code and FFI invariants. Section 9 lists what the crate relies on and what keeps it. The crate guarantees its callers that no safe call reaches undefined behaviour under RHA-1 and RHA-2.

Untrusted shader source and file paths. The crate passes no path or source to Slang itself. Shader source reaches Slang through `slang-compiler`, with the considerations of Section 17 of the `slang-compiler` design. A shader that an attacker controls runs on the GPU with the buffers that the program binds, and can read and write them.

Build scripts that run native builds. The crate has no build script. Its dependencies build Slang and slang-rhi, as Section 11 of the sys design states.

Dependencies. The crate depends on `slang-rhi-sys`, `slang-compiler-sys`, `slang-compiler`, and `bitflags`, whose versions the workspace fixes.

These attacks are out of scope:

- A malicious graphics driver or Vulkan loader, which runs with the full privileges of the program.
- A shader that hangs the GPU or reads data that the program bound to it, which is the program's choice of what to bind.
- Attacks on the repository itself, which are a matter of access control.

The residual risk has four parts:

1. slang-rhi documents no threading rule, so the soundness of devices on several threads rests on RHA-1.
2. A defect of slang-rhi or of the driver can corrupt memory of the process, and slang-rhi aborts the process on a failed assertion in every build type (Section 3.3 of the sys design).
3. A panic of the user's debug callback, and a call from it into `slang-compiler` while slang-rhi is inside Slang, abort the process (Sections 6.4 and 8).
4. The crate leaks the debug callback of each device that has one (RHI-6).

## 14. Operational Considerations

### 14.1. Cargo Features

The crate has no Cargo features. It always builds the vendored Slang and slang-rhi (RHI-1).

### 14.2. Environment Variables

The crate reads no environment variable. Its dependencies read those of Section 12.2 of the sys design and of `slang-compiler`.

### 14.3. Platforms and Minimum Supported Rust Version

The crate builds where `slang-rhi-sys` builds, which in M9 is Linux with the GNU C library, and is tested on x86_64 Linux on lavapipe. It uses the workspace's `rust-version`.

### 14.4. Failure Modes

| Failure | Reported by | Recovery |
| --- | --- | --- |
| No Vulkan loader, driver, or adapter | `ErrorKind::NoDevice` from `Device::new` | Install the Vulkan loader and a driver, such as lavapipe |
| A program of another session | `ErrorKind::InvalidArgument` from `create_shader_program` | Load and link the program through `Device::slang_session` |
| A shader that does not compile for the device | `ErrorKind::Compile` with the diagnostics | Fix the shader source that the diagnostics name |
| A parameter or a range out of bounds | `ErrorKind::InvalidArgument` | Use the offsets that `offset_of` returns and the size of the buffer |
| A panic in the debug callback | The process aborts | Do not panic in the callback |

### 14.5. Logs

The crate emits no logs and has no logging dependency. slang-rhi reports its messages through the debug callback, which the user supplies. This answers Q-16 of the architecture design for this crate.

## 15. Assumptions to Confirm

Each row is a fact that this design relies on and that is not verified. A section that relies on one cites its identifier. The change in the last column confirms or corrects it, states the result in the section that relies on it, and removes the row. An identifier is never reused.

| Identifier | Assumption | What depends on it | Where it is confirmed |
| --- | --- | --- | --- |
| RHA-1 | Objects of two devices, each used on its own thread, share no state of slang-rhi that is not protected, apart from the `IRHI` instance | Section 7 | A reading of the backends of slang-rhi in `src/` and `src/vulkan/` in the code step of the crate, and a `gpu` test that runs the compute path on two threads at once |
| RHA-2 | slang-rhi generates the code of a shader program for the target of the program's session, and the device's session has the target and the options that the device needs | RHI-7 | The `gpu` tests, and a reading of `src/shader.cpp` and of the creation of the device's session in the code step of the crate |
| RHA-3 | Without a debug callback, slang-rhi drops its messages or writes them to standard error, and does not stop | Section 6.4 | A reading of `src/` and a `gpu` test without a callback |
| RHA-4 | The element type layout of a root shader object is the layout of the global parameters as the program's reflection gives it, so that the offsets of `offset_of` are those that `ShaderCursor` finds | RHI-10 | The `gpu` tests, which compare the results of kernels with a uniform and a buffer parameter |

## 16. Open Questions

Each row is a question that this document leaves open. The owner in the third column answers it before or during the milestone in the last column, and the change that answers it removes the row. An identifier is never reused.

| Identifier | Question | Owner | Milestone |
| --- | --- | --- | --- |
| RHQ-1 | Should `check-slang-call-guard` check that every call of this crate into slang-rhi and into Slang sits alone in its abort guard, as it checks for `slang-compiler`? Extending the checker changes a mechanism that `AGENTS.md` declares frozen, and leaving it out keeps a deterministic check as a judgement rule (Section 8) | The owner of the project, by a decision | M9 |
| RHQ-2 | Will slang-rhi document which of its objects tolerate use from several threads, so that the types of this crate and of the `rhi` backend of `slango` can be `Send` and `Sync`? | The owner, through an issue in the slang-rhi repository, `shader-slang/slang-rhi`, for which `docs/upstream-reports.md` holds no entry yet | M10 |
| RHQ-3 | Can the crate free the debug callback object of a device, if slang-rhi offers a notice when a native device is freed? | This document, after RHQ-2 is asked upstream | Not assigned |
| RHQ-4 | Should `create_shader_program` and `create_compute_pipeline` return the warnings of Slang on success, as `slang-compiler` does with `Compiled`? | This document | M10 |

## 17. References

### 17.1. Normative References

- [RFC2119] Bradner, S., "Key words for use in RFCs to Indicate Requirement Levels", BCP 14, RFC 2119, March 1997, <https://www.rfc-editor.org/info/rfc2119>.
- [RFC8174] Leiba, B., "Ambiguity of Uppercase vs Lowercase in RFC 2119 Key Words", BCP 14, RFC 8174, May 2017, <https://www.rfc-editor.org/info/rfc8174>.

## Appendix A. Requirement Summary

This table lists every labelled requirement of this document with a one-line summary.

| Label | Section | Summary |
| --- | --- | --- |
| RHI-1 | 4.1 | The crate depends on `slang-rhi-sys`, `slang-compiler-sys`, `slang-compiler` with `vendored`, and `bitflags`, and has no features. |
| RHI-2 | 4.2 | A crate-level `unsafe_code` expectation, with every unsafe operation in four private modules. |
| RHI-3 | 5.1 | One crate-private owning interface pointer does all counting. |
| RHI-4 | 5.2 | Every wrapper holds its `Device`, the device holds its global session and session, and every unowned pointer lives in a value that borrows its owner. |
| RHI-5 | 6.4 | A device always gets the caller's global session, and an adapter is checked against the device type and fetched again. |
| RHI-6 | 6.4 | The debug callback is a leaked, non-unwinding object that lends each message to the user's closure for one call. |
| RHI-7 | 6.5 | A shader program takes only a linked program of the device's session, with no entry points, and reads its diagnostics. |
| RHI-8 | 6.6 | Initial data and read-back ranges are checked against the buffer size before any native call. |
| RHI-9 | 6.7 | A compute pass exists only inside a closure that begins and ends it. |
| RHI-10 | 6.7 | `offset_of` finds a top-level field by name as `ShaderCursor` does. |
| RHI-11 | 6.7 | `set_data` and `set_buffer` check offsets, ranges, binding types, and the device before each call. |
| RHI-12 | 6.8 | One `Error` with a kind, an operation, the result code, and diagnostics, and one mapping of result codes. |
| RHI-13 | 7 | Types that hold native objects are neither `Send` nor `Sync`, and value types are both, with assertion tests. |
| RHI-14 | 7 | Every call through `IRHI` holds one crate-private lock, and the crate never destroys the instance. |
| RHI-15 | 8 | Every native call runs in the crate's abort guard, innermost, inside the Slang call guard of `slang-compiler`. |
| RHI-16 | 10.2 | Miri tests the counting, the pass closure, the bounds checks, `Binding`, and the result mapping against a fake of three interfaces. |
| RHI-17 | 12 | The crate documentation states the path, the build, the platforms, the threading, the leaked callback, the aborts, and the features. |
