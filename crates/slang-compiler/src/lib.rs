//! Safe Rust wrappers over the Slang shader compiler.
//!
//! [Slang](https://shader-slang.org) is a compiler for shaders, the programs
//! that run on a graphics processing unit (GPU). It compiles the Slang
//! language to the formats that graphics APIs take, such as SPIR-V, the
//! binary shader format of Vulkan, and it reflects compiled code, which means
//! that it describes the parameters and types that the code uses. This crate
//! gives Rust programs a safe API for that, with no GPU dependency. It sits on
//! [`slang-compiler-sys`](slang_compiler_sys), the raw bindings of Slang's
//! native API, and the crate `slango` builds on it to run Slang code on a
//! GPU. Use this crate to compile and reflect shaders, and
//! `slang-compiler-sys` only to reach a part of Slang that this crate does not
//! wrap.
//!
//! The crate offers the [`GlobalSession`], the root object of Slang from
//! which every compilation starts, with its builder and the check of which
//! [`TargetFormat`] values Slang can produce. It offers the [`Session`], the
//! context of a compilation with its targets, search paths, macros, and
//! [`CompilerOptions`], which [`SessionBuilder`] checks and creates. A
//! session loads each [`Module`], a
//! unit of Slang source, from text, from a file on its search paths, or from
//! Slang's compiled form. A module yields each [`EntryPoint`], a function
//! that a graphics API calls. [`Session::compose`] combines them into a
//! [`ComponentType`], which [`ComponentType::specialize`] can specialize,
//! and linking returns a [`LinkedProgram`], which generates code as a
//! [`Blob`] and reports [`Metadata`] about it. The method `layout` of each of
//! them returns a [`reflection::ProgramLayout`], the start of reflection,
//! which describes each parameter of the code with its type and where it
//! sits in memory and in the bindings of a target, the numbered slots
//! through which a shader reaches its resources. The module [`reflection`]
//! holds every handle of reflection. Every failure is an [`Error`] with the
//! [`Diagnostics`] that Slang prints. A session cannot take a custom file
//! system.
//!
//! These rules hold for every object of the crate:
//!
//! - No object that holds a Slang object is [`Send`] or [`Sync`], because
//!   Slang documents its objects as not safe to use from several threads at
//!   once. Errors, diagnostics, enumerations, options, targets, and builders
//!   that hold only settings are both.
//! - Each object holds the object that Slang created it from, so a
//!   [`Session`] keeps its [`GlobalSession`] alive, and a [`Module`], an
//!   [`EntryPoint`], a [`ComponentType`], a [`LinkedProgram`], a [`Blob`],
//!   and [`Metadata`] keep their [`Session`] alive. Dropping a handle in any
//!   order is safe. A handle of [`reflection`] borrows the object that it
//!   came from, so the compiler rejects a use after that object is dropped.
//! - An object of one session is rejected by a method of another session
//!   with [`ErrorKind::InvalidArgument`], before any call into Slang.
//! - Slang is not reentrant. A method that would call Slang while another
//!   call of this crate into Slang is in progress on the same thread panics
//!   instead. Only code that Slang calls back during a call can do that.
//! - Every failure is an [`Error`], whose [`Error::kind`] says what went
//!   wrong. When Slang printed messages for the failed call, the error holds
//!   them, and a call that succeeds with messages returns them in a
//!   [`Compiled`] record together with its value.
//!
//! Aborts of the process, instead of errors, happen when a C++ exception
//! escapes a call of this crate into Slang. Slang is written in C++ and
//! catches its exceptions in most calls, but the pinned build lets one escape
//! some calls for some inputs. The crate then writes one line to standard
//! error that names the Slang call and aborts the process, before any
//! destructor of the crate or of the program runs. When the program is built
//! with `panic = "abort"`, the process aborts at the call without that line.
//! [`std::panic::catch_unwind`] cannot catch either. Any call into Slang can
//! abort this way, so a method states the abort in its own documentation
//! only when an exception has been observed to escape the call that it
//! makes. That is so for [`Module::entry_point_with_stage`] with a parameter
//! type nested more than 128 levels deep, for the method `layout` of each
//! component type with a parameter whose type nests 127 or more structures,
//! and for some methods of [`reflection`] on a type or a layout whose sizes
//! a generic value parameter gives.
//!
//! # Examples
//!
//! This example compiles a compute shader, a shader that runs as a general
//! computation on the GPU, to SPIR-V for version 1.5 of SPIR-V, and prints
//! the size of the code and any messages of Slang. It then lists the
//! parameters of the entry point with the kind of each parameter's type,
//! through the layout of the program, and unwraps the constant buffer, a
//! buffer of uniform data, that Slang can place around them. It needs the
//! Slang shared library at run time, which the dynamic loader must find, so
//! it is compiled but not run:
//!
//! ```no_run
//! use slang_compiler::reflection::TypeKind;
//! use slang_compiler::{Component, GlobalSession, Session, Target, TargetFormat};
//!
//! let source = r#"
//!     RWStructuredBuffer<float> data;
//!     [shader("compute")]
//!     [numthreads(64, 1, 1)]
//!     void scale(uint3 id : SV_DispatchThreadID, uniform float factor) { data[id.x] *= factor; }
//! "#;
//!
//! let global = GlobalSession::new()?;
//! let session = Session::builder(&global)
//!     .target(Target::new(TargetFormat::Spirv).profile("spirv_1_5"))
//!     .build()?;
//! let spirv = session.target_index(0)?;
//!
//! let module = session.load_module_from_source("kernels", "kernels.slang", source)?;
//! if !module.diagnostics.is_empty() {
//!     eprintln!("{}", module.diagnostics);
//! }
//! let module = module.into_value();
//! let entry_point = module.entry_point("scale")?;
//! let program = session
//!     .compose(&[Component::Module(&module), Component::EntryPoint(&entry_point)])?
//!     .into_value()
//!     .link()?
//!     .into_value();
//!
//! let code = program.entry_point_code(0, spirv)?;
//! if !code.diagnostics.is_empty() {
//!     eprintln!("{}", code.diagnostics);
//! }
//! println!("{} bytes of SPIR-V", code.value.as_bytes().len());
//!
//! let layout = program.layout(spirv)?.into_value();
//! for entry in layout.entry_point_layouts() {
//!     let Some(scope) = entry.var_layout().and_then(|v| v.type_layout()) else {
//!         continue;
//!     };
//!     let parameters = match scope.kind()? {
//!         TypeKind::ConstantBuffer | TypeKind::ParameterBlock => scope
//!             .element_var_layout()
//!             .and_then(|v| v.type_layout())
//!             .unwrap_or(scope),
//!         _ => scope,
//!     };
//!     for field in parameters.field_layouts() {
//!         let name = match field.variable() {
//!             Some(variable) => variable.name()?.unwrap_or("<unnamed>"),
//!             None => "<unnamed>",
//!         };
//!         let kind = field.type_layout().map(|t| t.kind()).transpose()?;
//!         println!("{name}: {kind:?}");
//!     }
//! }
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
//! When Slang is linked statically, from a build of Slang's source, it does
//! not include `slang-glslang`, the library through which Slang optimizes
//! SPIR-V and validates it when the environment variable
//! `SLANG_RUN_SPIRV_VALIDATION` is `1`. When the dynamic loader then finds no
//! `slang-glslang` shared library at run time, generating SPIR-V still
//! succeeds, but the SPIR-V is neither optimized nor validated. Slang reports
//! the missing library once in each [`GlobalSession`], in the diagnostics of
//! the first code generation, which begin
//! `error[E00100]: failed to load downstream compiler 'spirv-opt'` although
//! the generation succeeds.
//! `spirv-opt` is the SPIR-V optimizer, which Slang loads as a separate tool.
//! No message says that validation was skipped. When the dynamic loader finds
//! a `slang-glslang` shared library, such as the one in the library directory
//! of the Vulkan SDK, the statically linked Slang loads it and behaves as the
//! Slang shared library does, so the output of such a program depends on the
//! environment in which it runs. This crate
//! does not detect which way Slang was linked, and it returns what Slang
//! reports. What [`GlobalSession::check_target_support`] reports for SPIR-V
//! on that build has not been established.
//!
//! # Process-level inputs
//!
//! The crate reads no environment variable. Slang and the operating system
//! read these inputs of the process, which the crate does not control:
//!
//! - `SLANG_RECORD_LAYER`: Unset by default, which leaves Slang's record
//!   layer, a debugging mode, off. When the variable switches the layer on,
//!   creating a global session returns recording proxies of Slang's objects,
//!   and Slang writes files.
//! - `SLANG_RUN_SPIRV_VALIDATION`: Unset by default, so Slang does not
//!   validate the SPIR-V that it generates directly. When it is `1`, Slang
//!   validates that SPIR-V, unless
//!   [`CompilerOptions::skip_spirv_validation`] is on.
//! - `PATH`: Slang finds downstream compilers through it, the separate
//!   programs that it runs for some targets, such as a C++ compiler for
//!   host code. A target whose program is missing fails at code generation.
//! - The working directory: Slang resolves every relative path against it,
//!   such as the path of a module file, a search path, or an included file.
//! - The library path of the dynamic loader, which is `LD_LIBRARY_PATH` on
//!   Linux, `DYLD_LIBRARY_PATH` on macOS, and `PATH` on Windows: it decides
//!   which Slang shared library the program loads, as
//!   [Finding the Slang library](#finding-the-slang-library) describes.

#![expect(
    unsafe_code,
    reason = "the crate calls Slang through the raw bindings and counts the references of Slang objects, which only unsafe code can do"
)]

mod blob;
mod component;
mod enums;
mod error;
#[cfg(test)]
mod fake;
mod global_session;
mod guard;
mod mapping;
mod module;
mod options;
mod owned;
pub mod reflection;
mod session;
mod strings;

pub use blob::{BindingIndex, BindingSpace, Blob, Metadata};
pub use component::{Component, ComponentType, LinkedProgram, SpecializationArg, TypeConformance};
pub use enums::{
    DebugInfoLevel, FloatingPointMode, LanguageVersion, LineDirectiveMode, MatrixLayout,
    OptimizationLevel, ParameterCategory, Stage, TargetFormat, Toggle,
};
pub use error::{Compiled, Diagnostics, Error, ErrorKind, Operation, Result};
pub use global_session::{GlobalSession, GlobalSessionBuilder};
pub use module::{DependencyFilePaths, EntryPoint, EntryPoints, LoadedModules, Module};
pub use options::{CompilerOptions, OptionName};
pub use session::{Session, SessionBuilder, Target, TargetIndex};
