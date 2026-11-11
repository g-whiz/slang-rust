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
//! This version is the first part of the crate. It offers the [`Device`], the
//! connection to one adapter, a GPU or a software implementation of one,
//! which [`Device::new`] creates on the adapter that [`DeviceOptions`]
//! selects, and the module [`wgpu`], which connects a [`Device`] to wgpu code
//! of your own. [`Device::load_module`] compiles a Slang module and prepares
//! each of its compute entry points as a [`Kernel`] of the device, which
//! [`Module::kernel`] looks up by name. A [`Buffer`] holds elements of a type
//! that [`Element`] lists on the GPU, and [`Buffer::read`] copies them back.
//! [`Value`] lists the Rust types that stand for the plain values of Slang,
//! its scalars, vectors, matrices, and arrays. Every failure is an
//! [`Error`], whose [`Error::kind`] says what went wrong. The call of a
//! kernel, which binds buffers and sets values, follows in a later version,
//! so a kernel cannot run yet.
//!
//! Every type of the crate is [`Send`] and [`Sync`], and a [`Device`], a
//! [`Module`], a [`Kernel`], and a [`Buffer`] are cheap to clone.
//!
//! # Examples
//!
//! This example creates a device on the adapter that wgpu chooses, loads a
//! module from a file, prints the size of the thread groups of one of its
//! kernels, and creates a buffer on the device. It needs a GPU driver or a
//! software driver at run time, so it is compiled but not run:
//!
//! ```no_run
//! use slango::{Buffer, Device, DeviceOptions};
//!
//! let device = Device::new(&DeviceOptions::default())?;
//! println!("running on {}", device.adapter_name());
//! let module = device.load_module("/tmp/shaders/scale.slang")?;
//! let scale = module.kernel("scale")?;
//! println!("`scale` runs in groups of {:?}", scale.thread_group_size());
//! let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
//! assert_eq!(data.read()?, [1.0, 2.0, 3.0, 4.0]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
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
//! offers on the platform. It creates devices on Vulkan on Linux, on
//! Direct3D 12 on Windows, and on Metal on macOS. On macOS, Metal offers no
//! software adapter, so [`AdapterSelection::Software`] finds none there.
//!
//! # Finding the Slang library
//!
//! `slang-compiler` links the Slang library through `slang-compiler-sys`, as
//! the [crate documentation of `slang-compiler`](slang_compiler) describes.
//! By default, that is the Slang shared library of the pinned Vulkan SDK,
//! which the dynamic loader has to find when the program starts.

mod buffer;
mod device;
mod error;
mod kernel;
mod layout;
mod marshal;
mod module;
pub mod wgpu;

pub use buffer::Buffer;
pub use device::{AdapterSelection, Device, DeviceOptions};
pub use error::{Error, ErrorKind, Result};
pub use kernel::Kernel;
pub use marshal::{Element, Value};
pub use module::Module;
