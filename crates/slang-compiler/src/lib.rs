//! Safe Rust wrappers over the Slang shader compiler.
//!
//! [Slang](https://shader-slang.org) is a compiler for shaders, the programs
//! that run on a graphics processing unit (GPU). It compiles the Slang
//! language to the formats that graphics APIs take, such as SPIR-V, the
//! binary shader format of Vulkan, and it reflects compiled code, which means
//! that it describes the parameters and types that the code uses. This crate
//! gives Rust programs a safe API for that, with no GPU dependency. It sits on
//! [`slang-compiler-sys`](slang_compiler_sys), the raw bindings of Slang's
//! native API, and the planned crate `slango` builds on it to run Slang code
//! on a GPU. Use this crate to compile and reflect shaders, and
//! `slang-compiler-sys` only to reach a part of Slang that this crate does not
//! wrap.
//!
//! This version is the first part of the crate. It offers the
//! [`GlobalSession`], the root object of Slang from which every compilation
//! starts, with its builder, the check of which [`TargetFormat`] values Slang
//! can produce, and the [`Error`] type of every fallible operation with the
//! [`Diagnostics`] that Slang prints. Sessions, modules, linking, code
//! generation, and reflection follow in later versions.
//!
//! These rules hold for every object of the crate:
//!
//! - One global session for each thread. Creating a second global session on
//!   a thread on which one is alive fails with [`ErrorKind::AlreadyExists`].
//!   A program that compiles on several threads creates one global session on
//!   each.
//! - No object that holds a Slang object is [`Send`] or [`Sync`], because
//!   Slang documents its objects as not safe to use from several threads at
//!   once. Errors, diagnostics, enumerations, and builders that hold only
//!   settings are both.
//! - Slang is not reentrant. A method that would call Slang while another
//!   call of this crate into Slang is in progress on the same thread panics
//!   instead. Only code that Slang calls back during a call can do that.
//! - Every failure is an [`Error`], whose [`Error::kind`] says what went
//!   wrong. When Slang printed messages for the failed call, the error holds
//!   them, and a call that succeeds with messages returns them in a
//!   [`Compiled`] record together with its value.
//!
//! # Examples
//!
//! This example creates a global session and checks that Slang can produce
//! SPIR-V on this installation. It needs the Slang shared library at run
//! time, which the dynamic loader must find, so it is compiled but not run:
//!
//! ```no_run
//! use slang_compiler::{GlobalSession, TargetFormat};
//!
//! let global = GlobalSession::new()?;
//! println!("Slang {}", global.build_tag());
//! global.check_target_support(TargetFormat::Spirv)?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate supports the platforms of `slang-compiler-sys`, which are 64-bit
//! only: Linux on x86-64, `x86_64-unknown-linux-gnu`, Windows on x86-64 with
//! the Microsoft toolchain, `x86_64-pc-windows-msvc`, and macOS on Apple
//! silicon, `aarch64-apple-darwin`, and on x86-64, `x86_64-apple-darwin`. It
//! behaves the same on each.
//!
//! # Finding the Slang library
//!
//! `slang-compiler-sys` finds or builds the Slang library and links it, as
//! its [crate documentation](slang_compiler_sys) describes. By default, that
//! is the Slang shared library of the pinned Vulkan SDK, which the dynamic
//! loader has to find when the program starts. Creating a global session
//! checks that the loaded library is the build that this crate was built for,
//! and fails with [`ErrorKind::VersionMismatch`] when it is not.
//!
//! When Slang is linked statically, from a build of Slang's source, Slang has
//! no glslang, the library that validates and optimizes SPIR-V. This crate
//! does not detect which way Slang was linked, and it returns what Slang
//! reports. What [`GlobalSession::check_target_support`] reports for SPIR-V
//! on that build has not been established.

#![expect(
    unsafe_code,
    reason = "the crate calls Slang through the raw bindings and counts the references of Slang objects, which only unsafe code can do"
)]

mod enums;
mod error;
#[cfg(test)]
mod fake;
mod global_session;
mod guard;
mod mapping;
mod owned;

pub use enums::{LanguageVersion, TargetFormat, Toggle};
pub use error::{Compiled, Diagnostics, Error, ErrorKind, Operation, Result};
pub use global_session::{GlobalSession, GlobalSessionBuilder};
