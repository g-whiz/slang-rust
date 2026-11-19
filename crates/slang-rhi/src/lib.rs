//! Safe Rust wrappers over slang-rhi, the render hardware interface of Slang,
//! for compute dispatches and draws on a graphics processing unit (GPU).
//!
//! slang-rhi is the C++ library of the Slang source tree that gives one API
//! over several graphics APIs, such as Vulkan. This crate wraps its raw
//! bindings, which `slang-rhi-sys` holds, for the path of one compute
//! dispatch, which runs a compute kernel on the GPU, and for the path of one
//! draw, which draws triangles into a texture.
//!
//! A program lists the adapters, opens a device, creates a shader program and
//! a compute pipeline from a program that `slang-compiler` linked, and creates
//! buffers. It then records a compute pass, the commands of one or more
//! dispatches, in which it binds the parameters of the kernel by name and
//! dispatches the kernel, submits the work, and reads a buffer back.
//!
//! To draw, a program creates a render pipeline from a program with a vertex
//! and a fragment entry point, textures, and samplers, and records a render
//! pass into a colour target and an optional depth target, in which it binds
//! the parameters, textures, and samplers of the shaders by name, sets the
//! vertex and index buffers, and draws. It then reads the colour target back
//! with [`Device::read_texture`]. The example of
//! [`CommandEncoder::render_pass`] draws a triangle.
//!
//! Everything that concerns Slang, the shader compiler, such as loading and
//! linking modules, goes through `slang-compiler`. Use `slango` for a
//! higher-level API on wgpu, and this crate to drive slang-rhi itself.
//!
//! # Examples
//!
//! This example doubles four `float` values with a compute kernel, a function
//! that the GPU runs once for each thread. It needs a Vulkan driver at run
//! time, so it is compiled but not run. On Windows, a program names
//! [`DeviceType::D3d12`] instead:
//!
//! ```no_run
//! use slang_compiler::{Component, GlobalSession};
//! use slang_rhi::{BufferDesc, BufferUsage, Device, DeviceOptions, DeviceType};
//!
//! const KERNEL: &str = r#"
//! RWStructuredBuffer<float> values;
//! uniform float factor;
//!
//! [shader("compute")]
//! [numthreads(4, 1, 1)]
//! void main(uint3 thread : SV_DispatchThreadID)
//! {
//!     values[thread.x] = values[thread.x] * factor;
//! }
//! "#;
//!
//! let mut options = DeviceOptions::new(GlobalSession::new()?);
//! options.device_type(DeviceType::Vulkan);
//! let device = Device::new(&options)?;
//!
//! let session = device.slang_session();
//! let module = session
//!     .load_module_from_source("double", "double.slang", KERNEL)?
//!     .into_value();
//! let entry_point = module.entry_point("main")?;
//! let linked = session
//!     .compose(&[Component::Module(&module), Component::EntryPoint(&entry_point)])?
//!     .into_value()
//!     .link()?
//!     .into_value();
//! let program = device.create_shader_program(&linked)?;
//! let pipeline = device.create_compute_pipeline(&program)?;
//!
//! let input: Vec<u8> = [1.0_f32, 2.0, 3.0, 4.0]
//!     .iter()
//!     .flat_map(|value| value.to_le_bytes())
//!     .collect();
//! let desc = BufferDesc::new(16, BufferUsage::UNORDERED_ACCESS | BufferUsage::COPY_SOURCE)
//!     .with_element_size(4);
//! let values = device.create_buffer(&desc, Some(&input))?;
//!
//! let queue = device.queue()?;
//! let mut encoder = queue.encoder()?;
//! encoder.compute_pass(|pass| {
//!     let mut root = pass.bind_pipeline(&pipeline)?;
//!     if let Some(factor) = root.offset_of("factor")? {
//!         root.set_data(factor, &2.0_f32.to_le_bytes())?;
//!     }
//!     if let Some(target) = root.offset_of("values")? {
//!         root.set_buffer(target, &values)?;
//!     }
//!     drop(root);
//!     pass.dispatch([1, 1, 1])
//! })?;
//! queue.submit([encoder.finish()?])?;
//! queue.wait_on_host()?;
//!
//! let mut output = vec![0_u8; 16];
//! device.read_buffer(&values, 0, &mut output)?;
//! let doubled: Vec<f32> = output
//!     .chunks_exact(4)
//!     .map(|bytes| f32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
//!     .collect();
//! assert_eq!(doubled, [2.0, 4.0, 6.0, 8.0]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! A device receives the global session of `slang-compiler` that the program
//! creates, and compiles through a session of its own, which
//! [`Device::slang_session`] returns. Load and link every program of the
//! device through that session, because a shader program takes only a
//! program of the device's session, or through a [`CompileSession`] that
//! [`Device::create_slang_session`] creates with the settings of the device's
//! session, whose own shader programs take its programs. The global sessions
//! that `slang-compiler` creates carry the loader of the SPIR-V optimizer, so
//! the SPIR-V code of a Vulkan device is optimized. SPIR-V is the shader code
//! format of Vulkan.
//!
//! # Ownership and threads
//!
//! Every object that a device creates holds the device, so a device lives as
//! long as any of its objects. A compute pass exists only inside the closure
//! of [`CommandEncoder::compute_pass`], a render pass only inside the closure
//! of [`CommandEncoder::render_pass`], and a shader object borrows its pass,
//! so neither can be used after the pass ends.
//!
//! No type that holds an object of slang-rhi is [`Send`] or [`Sync`], because
//! slang-rhi documents no rule for the use of its objects from several
//! threads. A program that uses several threads opens a device on each. The
//! calls of the crate through the one instance of slang-rhi of the process,
//! which [`adapters`] and [`Device::new`] make, run one at a time.
//!
//! The crate keeps the debug callback object of each device until the
//! process exits, together with the closure that
//! [`DeviceOptions::debug_callback`] sets, because slang-rhi gives no notice
//! when it stops calling it. Each device costs one small allocation that is
//! never freed.
//!
//! The crate aborts the process, instead of returning an error, when a C++
//! exception escapes a call into slang-rhi or into Slang, after it writes one
//! line to standard error that names the call. It also aborts the process when
//! the closure of a debug callback panics, because the panic reaches the
//! boundary of slang-rhi, and when that closure uses an object of this crate
//! or of `slang-compiler` while slang-rhi is inside a call of the same thread,
//! which panics in the closure.
//!
//! # Debug messages
//!
//! slang-rhi reports errors, warnings, and information through a debug
//! callback, a function that it calls with each message. A program receives
//! these messages by setting a closure with
//! [`DeviceOptions::debug_callback`], which gets each one as a
//! [`DebugMessage`]. A device works without a closure.
//!
//! slang-rhi calls the closure from the thread of the call that caused the
//! message or from a thread of the driver, and from several threads at once.
//!
//! The messages that Slang reports while slang-rhi compiles a kernel also
//! become the diagnostics of the error of the failed call, which
//! [`Error::diagnostics`] returns, whether or not the program sets a closure.
//!
//! [`DeviceOptions::validation`] turns on the debug device of slang-rhi,
//! which checks each call against the rules of slang-rhi and reports a
//! violation through the debug callback. It does not turn on the validation
//! layer of Vulkan or the debug layer of Direct3D 12.
//!
//! # What the crate does not cover
//!
//! The crate wraps the compute path and the path of a draw only. It has no
//! surfaces or windows, fences, mapped buffers, copy or clear commands,
//! indirect dispatches or draws, or queries. A program that needs one of
//! them calls the raw bindings of `slang-rhi-sys` under their safety
//! contract. These limits also apply:
//!
//! - A device opens only with [`DeviceType::Vulkan`] on Linux and
//!   [`DeviceType::D3d12`] on Windows, or with [`DeviceType::Default`], which
//!   selects that API. A device of any other type fails to open.
//! - The crate creates no buffer with a format, so a parameter takes only a
//!   raw buffer, such as a `StructuredBuffer`, a `RWStructuredBuffer`, or a
//!   `ByteAddressBuffer`, and not a typed buffer such as `RWBuffer<float>`.
//! - A texture is two-dimensional with one mip level, one layer, and one
//!   sample, of one of the formats of [`Format`]. A shader reads it through a
//!   parameter such as a `Texture2D` and a separate `SamplerState`. A texture
//!   that a shader writes, such as a `RWTexture2D`, and a texture combined
//!   with a sampler, such as a `Sampler2D`, are refused.
//! - On Vulkan, a texture created without initial data has undefined
//!   contents until a render pass clears or draws into it. A texture of
//!   [`Format::D32Float`] takes no initial data, so a render pass with that
//!   texture as its only target clears it.
//! - A render pipeline draws lists of triangles into one colour target and
//!   an optional depth target, with counter-clockwise front faces. Each
//!   vertex stream holds one attribute at the start of each element and
//!   names the semantic of its input, because Direct3D 12 matches inputs by
//!   semantic.
//! - A draw refuses a texture that a shader object of its pass binds and
//!   that is also a target of the pass. The crate does not read the values of
//!   an index buffer, so an index past the end of a vertex buffer reaches the
//!   device, which reads zeros or values from within the buffer for it.
//! - On Vulkan, slang-rhi clamps the depth of a fragment outside the range
//!   from 0 to 1 instead of clipping the triangle, as Direct3D 12 does.
//! - [`ShaderObject::offset_of`] finds only the parameters at the top level
//!   of a shader object by name, and not a field of a structure or an element
//!   of an array. The fields of a `ConstantBuffer` or a `ParameterBlock`
//!   parameter are found and set on the object that [`ShaderObject::object`]
//!   returns. A parameter that is a structure of several resources, such as
//!   a structure with a `ConstantBuffer` and a buffer, is refused, because
//!   its fields cannot be addressed by name.
//! - On Vulkan, slang-rhi binds an array of `ConstantBuffer` parameters, such
//!   as `ConstantBuffer<Params> constants[3];`, as if it had one element, so
//!   every element after the first holds wrong values in the kernel.
//! - Push-constant buffers, which a kernel declares with
//!   `[vk::push_constant]`, are not supported. On Vulkan, slang-rhi aborts
//!   the process at the dispatch of a kernel that declares one, whether or
//!   not the program sets it.
//! - A compute pipeline takes a program with exactly one compute entry point,
//!   a render pipeline one with exactly one vertex and one fragment entry
//!   point, and both no specialization parameter left.
//! - On Vulkan, a parameter that is left without a buffer works only on a
//!   driver with the feature `nullDescriptor`, which the crate does not check.
//!
//! # How the crate gets slang-rhi
//!
//! The crate always builds Slang and slang-rhi from source and links them
//! statically, through `slang-rhi-sys` and the feature `vendored` of
//! `slang-compiler`, and never uses the Slang of a Vulkan SDK. The first build
//! in a Cargo target directory clones the Slang source tree, which needs git
//! and network access, and builds Slang, its SPIR-V optimizer, and slang-rhi,
//! which takes minutes.
//!
//! The build needs libclang, `cmake` 3.22 or later, Ninja, a C++20 compiler,
//! and Python 3. It also needs the development headers of Xlib on Linux, such
//! as those of the Debian package `libx11-dev`, and on Windows an installation
//! of Visual Studio or its Build Tools with the C++ tools and a Windows SDK, as
//! the documentation of `slang-rhi-sys` states.
//!
//! On Linux, a program that opens a device needs the Vulkan loader
//! `libvulkan.so.1` and a Vulkan driver at run time, such as lavapipe, the
//! software driver of Mesa. On Windows, a program that creates a pipeline
//! needs `dxcompiler.dll` of the DirectX Shader Compiler on its `PATH` or
//! beside its executable, through which Slang compiles the kernels to DXIL,
//! the shader code format of Direct3D 12. The program provides it, and the
//! Windows SDK ships it.
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate builds where `slang-rhi-sys` builds, which is Linux with the GNU
//! C library, with the Vulkan backend of slang-rhi, and `x86_64` Windows with
//! the Microsoft toolchain, with its Direct3D 12 backend. On every other
//! target, the build of `slang-rhi-sys` stops at once with an error that
//! names the target, before anything is cloned or built. The crate is tested
//! on `x86_64` Linux, on lavapipe and on a hardware GPU, and on `x86_64`
//! Windows on WARP, the software adapter of Direct3D 12.

#![expect(
    unsafe_code,
    reason = "the crate calls the generated bindings of slang-rhi and the reflection functions of Slang, which only unsafe code can do"
)]

mod buffer;
mod callback;
mod command;
mod compile_session;
mod device;
mod error;
#[cfg(test)]
mod fake;
mod guard;
mod instance;
mod lookup;
mod owned;
mod program;
mod render_pass;
mod render_pipeline;
mod sampler;
mod shader_object;
mod texture;

pub use buffer::{Buffer, BufferDesc, BufferUsage, MemoryType};
pub use callback::{DebugMessage, DebugMessageSource, DebugMessageType};
pub use command::{CommandBuffer, CommandEncoder, ComputePass, Queue};
pub use compile_session::CompileSession;
pub use device::{Adapter, AdapterType, Device, DeviceInfo, DeviceOptions, DeviceType, adapters};
pub use error::{Error, ErrorKind, Operation, Result};
pub use program::{ComputePipeline, ShaderProgram};
pub use render_pass::{ColourLoad, DepthLoad, RenderPass, RenderPassDesc};
pub use render_pipeline::{
    BlendMode, CullMode, RenderPipeline, RenderPipelineDesc, StepMode, VertexFormat, VertexStream,
};
pub use sampler::{AddressMode, FilterMode, Sampler, SamplerDesc};
pub use shader_object::{ShaderObject, ShaderOffset};
pub use texture::{Format, Texture, TextureDesc, TextureUsage};

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
