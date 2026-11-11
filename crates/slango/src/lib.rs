//! Runs Slang compute kernels on the GPU from Rust.
//!
//! [Slang](https://shader-slang.org) is a language for shaders, the programs
//! that run on a graphics processing unit (GPU). A compute kernel is a
//! function of a shader that runs as a general computation on many threads
//! of the GPU at once. This crate is the high-level library of this
//! repository. It compiles Slang through
//! [`slang-compiler`](slang_compiler), the safe wrappers over the Slang
//! compiler, and runs the result through [wgpu](https://wgpu.rs), a
//! graphics library for Vulkan, Direct3D 12, and Metal. Use this crate to run
//! kernels, and `slang-compiler` alone to compile or inspect shaders without
//! a GPU.
//!
//! [`Device::new`] creates a [`Device`], the connection to one adapter, a
//! GPU or a software implementation of one, on the adapter that
//! [`DeviceOptions`] selects, and the module [`wgpu`] connects a [`Device`]
//! to wgpu code of your own. [`Device::load_module`] compiles a Slang module
//! and prepares each of its compute entry points as a [`Kernel`] of the
//! device, which [`Module::kernel`] looks up by name. A [`Buffer`] holds
//! elements of a type that [`Element`] lists on the GPU. [`Kernel::call`]
//! starts a [`Call`], which sets the plain values of the kernel by name,
//! from the Rust types that [`Value`] lists, binds buffers by name, checks
//! every argument against the parameters of the kernel, and runs the kernel
//! on the GPU. [`Buffer::read`] waits for the kernel and copies the results
//! back. Every failure is an [`Error`], whose [`Error::kind`] says what went
//! wrong.
//!
//! A kernel is a function of the module marked `[shader("compute")]`, and
//! the crate supports these parameters of a kernel:
//!
//! - `StructuredBuffer<T>` and `RWStructuredBuffer<T>`, to which a call binds
//!   a [`Buffer`] of the matching element type.
//! - Plain values of the types `bool`, `int`, `uint`, and `float`, as
//!   scalars, vectors, matrices, arrays of fixed length, and structures of
//!   them, declared as `uniform` globals, as `uniform` parameters of the
//!   entry point, or as fields of a `ConstantBuffer`, a `cbuffer`, or a
//!   `ParameterBlock`, which a call sets.
//! - System values such as `SV_DispatchThreadID`, which the GPU provides.
//!
//! A load fails with [`ErrorKind::UnsupportedParameter`] when a kernel has
//! any other parameter, such as a texture, a sampler, a `ByteAddressBuffer`,
//! a `double`, or an array of structures. It fails with
//! [`ErrorKind::UnsupportedShader`] when wgpu rejects a construct of the
//! compiled kernel, and with [`ErrorKind::MissingFeature`] when a kernel has
//! `uniform` parameters of its entry point and the device cannot receive
//! them. The crate loads every kernel through wgpu's validated path and
//! contains no unsafe code.
//!
//! Every type of the crate is [`Send`] and [`Sync`], and a [`Device`], a
//! [`Module`], a [`Kernel`], and a [`Buffer`] are cheap to clone.
//!
//! # Examples
//!
//! This example creates a device on the adapter that wgpu chooses, loads the
//! module in `scale.slang`, and runs its kernel `scale` over a buffer of four
//! numbers. The file holds this Slang source:
//!
//! ```slang
//! RWStructuredBuffer<float> data;
//! uniform float factor;
//! uniform uint count;
//!
//! [shader("compute")]
//! [numthreads(64, 1, 1)]
//! void scale(uint3 id : SV_DispatchThreadID)
//! {
//!     if (id.x < count) { data[id.x] *= factor; }
//! }
//! ```
//!
//! The program sets the two uniforms, binds the buffer, runs the kernel on
//! four threads, and reads the result back. It needs a GPU driver or a
//! software driver at run time, so it is compiled but not run:
//!
//! ```no_run
//! use slango::{Buffer, Device, DeviceOptions};
//!
//! fn main() -> slango::Result<()> {
//!     let device = Device::new(&DeviceOptions::default())?;
//!     let module = device.load_module("/tmp/shaders/scale.slang")?;
//!     let scale = module.kernel("scale")?;
//!     let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
//!     scale
//!         .call()
//!         .set("factor", 2.0_f32)
//!         .set("count", 4_u32)
//!         .bind("data", &data)
//!         .dispatch([4, 1, 1])?;
//!     let result: Vec<f32> = data.read()?;
//!     assert_eq!(result, [2.0, 4.0, 6.0, 8.0]);
//!     Ok(())
//! }
//! ```
//!
//! The example `compute-kernel` of the package `slang-rust-examples` in the
//! repository of this crate is a complete program of the same flow. It runs
//! a kernel with two input buffers, an output buffer, and two `uniform`
//! values, and checks its results against the same computation on the CPU.
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate supports the platforms of `slang-compiler`, which are Linux on
//! x86-64, Windows on x86-64, and macOS on Apple silicon and on x86-64. It
//! searches the adapters of whichever of Vulkan, Direct3D 12, and Metal wgpu
//! offers on the platform. It creates devices and runs kernels on Vulkan on
//! Linux, on Direct3D 12 on Windows, and on Metal on macOS, and its tests run
//! in continuous integration on lavapipe on Linux, on the Windows Advanced
//! Rasterization Platform (WARP) on Windows, and on Metal on macOS. On macOS,
//! Metal offers no software adapter, so [`AdapterSelection::Software`] finds
//! none there.
//!
//! # Finding the Slang library
//!
//! `slang-compiler` links the Slang library through `slang-compiler-sys`, as
//! the [crate documentation of `slang-compiler`](slang_compiler) describes.
//! By default, that is the Slang shared library of the pinned Vulkan SDK,
//! which the dynamic loader has to find when the program starts.

mod buffer;
mod call;
mod device;
mod error;
mod kernel;
mod layout;
mod marshal;
mod module;
pub mod wgpu;

pub use buffer::Buffer;
pub use call::Call;
pub use device::{AdapterSelection, Device, DeviceOptions};
pub use error::{Error, ErrorKind, Result};
pub use kernel::Kernel;
pub use marshal::{Element, Value};
pub use module::Module;
