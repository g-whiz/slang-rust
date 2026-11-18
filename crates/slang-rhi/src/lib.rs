//! Safe Rust wrappers over slang-rhi, the render hardware interface of Slang,
//! for compute dispatches on a graphics processing unit (GPU).
//!
//! slang-rhi is the C++ library of the Slang source tree that gives one API
//! over several graphics APIs, such as Vulkan. This crate wraps its raw
//! bindings, which `slang-rhi-sys` holds, for the path of one compute
//! dispatch: a program lists the adapters, opens a device, creates a shader
//! program and a compute pipeline from a program that `slang-compiler` linked,
//! creates buffers, records a compute pass that binds parameters by name and
//! dispatches the kernel, submits the work, and reads a buffer back.
//! Everything that concerns Slang, the shader compiler, such as loading and
//! linking modules, goes through `slang-compiler`. Use `slango` for a
//! higher-level API on wgpu, and this crate to drive slang-rhi itself.
//!
//! # Examples
//!
//! This example doubles four `float` values with a compute kernel, a function
//! that the GPU runs once for each thread. It needs a Vulkan driver at run
//! time, so it is compiled but not run:
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
//! program of the device's session. The global sessions that `slang-compiler`
//! creates carry the loader of the SPIR-V optimizer, so the device's code is
//! optimized.
//!
//! # Ownership and threads
//!
//! Every object that a device creates holds the device, so a device lives as
//! long as any of its objects. A compute pass exists only inside the closure
//! of [`CommandEncoder::compute_pass`], and a shader object borrows its pass,
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
//! Aborts the process, instead of returning an error, when a C++ exception
//! escapes a call into slang-rhi or into Slang, after it writes one line to
//! standard error that names the call. Aborts the process when the closure of
//! a debug callback panics, because the panic reaches the boundary of
//! slang-rhi, and when that closure uses an object of this crate or of
//! `slang-compiler` while slang-rhi is inside a call of the same thread,
//! which panics in the closure.
//!
//! # How the crate gets slang-rhi
//!
//! The crate always builds Slang and slang-rhi from source and links them
//! statically, through `slang-rhi-sys` and the feature `vendored` of
//! `slang-compiler`. It never uses the Slang of a Vulkan SDK. The first build
//! in a Cargo target directory clones the Slang source tree, which needs git
//! and network access, and builds Slang, its SPIR-V optimizer, and slang-rhi,
//! which takes minutes. The build needs libclang, `cmake` 3.22 or later,
//! Ninja, a C++20 compiler, Python 3, and the development headers of Xlib,
//! such as those of the Debian package `libx11-dev`, as the documentation of
//! `slang-rhi-sys` states.
//!
//! A program that opens a device needs the Vulkan loader `libvulkan.so.1` and
//! a Vulkan driver at run time, such as lavapipe, the software driver of Mesa.
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate builds where `slang-rhi-sys` builds, which is Linux with the GNU
//! C library, with the Vulkan backend of slang-rhi only. On every other
//! target, the build of `slang-rhi-sys` stops at once with an error that
//! names the target, before anything is cloned or built. The crate is tested
//! on `x86_64` Linux, on lavapipe and on a hardware GPU.

#![expect(
    unsafe_code,
    reason = "the crate calls the generated bindings of slang-rhi and the reflection functions of Slang, which only unsafe code can do"
)]

mod buffer;
mod callback;
mod command;
mod device;
mod error;
#[cfg(test)]
mod fake;
mod guard;
mod instance;
mod lookup;
mod owned;
mod program;
mod shader_object;

pub use buffer::{Buffer, BufferDesc, BufferUsage, MemoryType};
pub use callback::{DebugMessage, DebugMessageSource, DebugMessageType};
pub use command::{CommandBuffer, CommandEncoder, ComputePass, Queue};
pub use device::{Adapter, AdapterType, Device, DeviceInfo, DeviceOptions, DeviceType, adapters};
pub use error::{Error, ErrorKind, Operation, Result};
pub use program::{ComputePipeline, ShaderProgram};
pub use shader_object::{ShaderObject, ShaderOffset};
