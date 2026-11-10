//! Placeholder for the raw bindings to the Slang shader compiler.
//!
//! This crate contains no bindings yet. It will hold the raw bindings to the
//! native [Slang](https://shader-slang.org) library, which compiles shaders,
//! the programs that run on a graphics processing unit (GPU), together with a
//! build script that links that library. The bindings will be generated from
//! the Slang headers and will mirror the C API of Slang without making it
//! safe. Most programs will not use this crate directly. The planned crate
//! `slang-compiler` will wrap these bindings in a safe API, and the planned
//! crate `slango` will build on it to run Slang code on a GPU.
//!
//! The crate already records the version of Slang that its bindings will
//! match, which is called the pin: [`VULKAN_SDK_VERSION`] names the Vulkan
//! SDK that ships that Slang, and [`SLANG_BUILD_TAG`] and [`SLANG_COMMIT`]
//! name the Slang build. These constants are the only record of the pin in
//! the repository, and every tool that needs the pin reads them.
//!
//! # Examples
//!
//! This example prints the pinned versions:
//!
//! ```
//! println!(
//!     "Vulkan SDK {}, Slang {} at commit {}",
//!     slang_compiler_sys::VULKAN_SDK_VERSION,
//!     slang_compiler_sys::SLANG_BUILD_TAG,
//!     slang_compiler_sys::SLANG_COMMIT,
//! );
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Features
//!
//! This crate has no Cargo features.

include!("../pin.rs");
