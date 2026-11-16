//! glslang and SPIRV-Tools built statically for a Slang built from source,
//! with the loader through which that Slang optimizes SPIR-V.
//!
//! [Slang](https://shader-slang.org) is a shader compiler. It optimizes the
//! SPIR-V that it generates, SPIR-V being the binary shader format of Vulkan,
//! through slang-glslang, Slang's library around glslang and SPIRV-Tools,
//! which compiles GLSL to SPIR-V and optimizes, disassembles, validates, and
//! links SPIR-V. Slang's own build produces slang-glslang only as a shared
//! library that Slang loads by name at run time. A Slang that the crate `slang-compiler-sys` builds from source with
//! its Cargo feature `vendored` and links statically finds no such library,
//! so it returns SPIR-V that is not optimized, and the first generation of
//! SPIR-V in each global session returns diagnostics that begin with
//! `failed to load downstream compiler 'spirv-opt'` while it succeeds.
//!
//! This crate builds slang-glslang and a small C++ loader shim from the same
//! Slang source tree as static libraries, and links them into the program. The loader implements Slang's interface
//! [`ISlangSharedLibraryLoader`], through which a global session, the root
//! object of Slang from which every compilation starts, loads every library
//! that it needs. It answers each request for a library whose file
//! name, after any directory and an optional `lib` prefix, starts with
//! `slang-glslang` with the functions that the program links, and passes
//! every other request, such as for the DirectX Shader Compiler, to Slang's
//! default loader unchanged. Once it is installed on a global session, that
//! global session optimizes SPIR-V without any library file at run time, and
//! its output does not depend on the libraries that the program's
//! environment happens to provide.
//!
//! The crate exports one function, [`slang_glslang_sys_loader`], which
//! returns the loader. It installs nothing itself.
//!
//! # Installing the loader
//!
//! The crate `slang-compiler`, with its Cargo feature `vendored`, depends on
//! this crate and installs the loader on every global session that it
//! creates, before it returns the global session. A program that uses
//! `slang-compiler`, or a crate above it, needs nothing else.
//!
//! A program that uses the raw bindings of `slang-compiler-sys` directly,
//! with its feature `vendored`, gets SPIR-V that is not optimized unless it
//! depends on this crate and installs the loader on each global session
//! through
//! [`IGlobalSession::setSharedLibraryLoader`](slang_compiler_sys::IGlobalSession::setSharedLibraryLoader),
//! before the global
//! session generates any code. A global session that generated SPIR-V
//! without the loader has already failed to load slang-glslang and does not
//! try again.
//!
//! # Examples
//!
//! This example creates a global session through the raw bindings and
//! installs the loader on it before any other use. It needs a vendored build,
//! which links Slang statically, and it creates a global session, which loads
//! Slang's core module, so it is compiled but not run:
//!
//! ```no_run
//! use std::ptr;
//!
//! use slang_compiler_sys::{
//!     IGlobalSession, SlangGlobalSessionDesc, SlangResult, slang_createGlobalSession2,
//! };
//!
//! let desc = SlangGlobalSessionDesc::default();
//! let mut global: *mut IGlobalSession = ptr::null_mut();
//! // SAFETY: `desc` is a live descriptor whose size field holds its own size,
//! // which `Default` sets, and `global` is a live out-pointer.
//! let result = unsafe { slang_createGlobalSession2(&raw const desc, &raw mut global) };
//! if result != SlangResult::SLANG_OK || global.is_null() {
//!     return Err(format!("cannot create a global session: {result:?}").into());
//! }
//!
//! let loader = slang_glslang_sys::slang_glslang_sys_loader();
//! // SAFETY: `global` is a live global session of the vendored Slang that
//! // this program links, only this thread uses it, and the loader is a static
//! // object.
//! unsafe { IGlobalSession::setSharedLibraryLoader(global, loader) };
//!
//! // SAFETY: `global` holds the one reference that Slang returned, and the
//! // example does not use it after this release.
//! unsafe { IGlobalSession::release(global) };
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Features
//!
//! This crate has no Cargo features. It exists only for a build of Slang from
//! source, so it enables that build of slang-compiler-sys itself, and
//! slang-compiler enables this crate through its own feature.
//!
//! # Platform support
//!
//! The crate builds on every target for which it has a rule to link the C++
//! runtime library, because glslang and SPIRV-Tools are written in C++. Those
//! targets are Linux with the GNU C library, macOS, and Windows with the
//! Microsoft toolchain, the same as for the Slang that `slang-compiler-sys`
//! builds from source. The build stops with an error that names both target
//! triples when the target differs from the host, because that Slang cannot
//! be cross-compiled, and with an error when the target has no such rule.
//! It does not stop for a target that is untested.
//!
//! It is tested on Linux on x86-64, Windows on x86-64 with the Microsoft
//! toolchain, and macOS on Apple silicon. Every other target on which it
//! builds is untested and unsupported.
//!
//! # Building
//!
//! The build script builds a small project of this crate for the build
//! system `cmake` with `ninja`, outside the source tree, in its `OUT_DIR`.
//! The project reads glslang, SPIRV-Tools, the SPIR-V headers, and the glue code from the Slang
//! source tree that the crate `slang-src` clones, and writes nothing into
//! that tree. The build needs these programs on the build machine:
//!
//! - The programs `cmake` 3.22 or later and `ninja` on `PATH`.
//! - A C++20 compiler. With the Microsoft toolchain, that is an installation
//!   of Visual Studio or its Build Tools with the C++ tools. The build script
//!   finds `cl.exe` of the newest installation, or of the developer command
//!   prompt that the build runs in, and gives its environment to `cmake`, so the
//!   build needs no developer command prompt. The environment variable
//!   `VCToolsVersion` selects another installed version of the toolset.
//! - Python 3 on `PATH`, because SPIRV-Tools generates some of its sources
//!   with a Python script.
//! - git and network access for the first build in a target directory, in
//!   which `slang-src` clones the source tree.
//!
//! The build writes the output of `cmake` and `ninja` to `build.log` under
//! `OUT_DIR`, and a failed step stops the build with an error that names the
//! log and holds its last 40 lines. It then checks that the five static
//! libraries `slang-glslang-static`, `glslang`, `SPIRV-Tools-opt`,
//! `SPIRV-Tools-link`, and `SPIRV-Tools` exist, and links them in that order,
//! followed by the C++ standard library of the target: `stdc++` on Linux and
//! `c++` on macOS. With the Microsoft toolchain, the runtime library that the
//! target feature `crt-static` selects provides it. It also links the system
//! libraries that the build of slang-glslang links: `pthread` on Linux,
//! which glslang declares, and with the Microsoft toolchain the libraries
//! that `cmake` links into every program and library by default, which are
//! `kernel32`, `user32`, `gdi32`, `winspool`, `shell32`, `ole32`,
//! `oleaut32`, `uuid`, `comdlg32`, and `advapi32`. Cargo copies the static
//! libraries into the library of this crate, so a program that links it
//! needs no file of slang-glslang at run time.
//!
//! A build takes a few minutes, so the build script keeps the libraries that
//! it built in the shared folder of the Cargo target directory, next to the
//! source tree of `slang-src`. The shared folder is the directory
//! `slang-src` in the target directory, or in the build directory when Cargo
//! uses a separate one. Every profile, and both the host and the target
//! units, of a project use it, so a procedural macro and the program build
//! slang-glslang once, and a build whose compiler, flags, or options differ
//! builds again. `cargo clean` removes the folder, and removing it when no
//! build runs is safe. Projects share it only by sharing a target directory,
//! such as through `CARGO_TARGET_DIR`. When Cargo did not tag the target
//! directory, such as for a directory that existed before the first build,
//! each unit builds in its own `OUT_DIR` instead, as the documentation of
//! `slang-src` describes.
//!
//! # Linking another glslang
//!
//! A program that also links another static copy of glslang or SPIRV-Tools,
//! such as through a crate that builds shaderc, can fail to link with
//! duplicate symbols. When the linker accepts both copies, code of one copy
//! can call the other copy through a name that both define. Remove the other
//! copy, or build without the feature `vendored`, with which Slang loads
//! slang-glslang as a separate shared library of the Vulkan SDK.
//!
//! The glue code and the optimizer run inside the program's process, so a
//! defect in them that a shader reaches affects the program as a defect of
//! Slang does.

#![expect(
    unsafe_code,
    reason = "the crate declares the foreign function of its C++ shim that returns the loader, which only unsafe code can do"
)]

use slang_compiler_sys::ISlangSharedLibraryLoader;
use slang_src as _;

unsafe extern "C" {
    /// Returns the loader through which a statically linked Slang finds
    /// slang-glslang in the program.
    ///
    /// Slang-glslang is Slang's library around glslang and SPIRV-Tools, which
    /// compiles GLSL to SPIR-V and optimizes, disassembles, validates, and
    /// links SPIR-V, and through which Slang optimizes the SPIR-V that it
    /// generates. A global session is the root object of Slang from
    /// which every compilation starts. Pass the result to
    /// [`IGlobalSession::setSharedLibraryLoader`](slang_compiler_sys::IGlobalSession::setSharedLibraryLoader)
    /// of each global session, before the global session generates any code,
    /// as the
    /// [crate documentation](crate) shows. The function only returns a
    /// pointer, so calling it more than once returns the same pointer, and it
    /// is safe to call. The pointer is never null and stays valid for the life
    /// of the process, to a loader whose reference counting changes and frees
    /// nothing, and which any thread may use at any time.
    ///
    /// Pass the pointer only to
    /// [`setSharedLibraryLoader`](slang_compiler_sys::IGlobalSession::setSharedLibraryLoader)
    /// of a global session
    /// of the Slang that `slang-compiler-sys` builds from source and this
    /// program links. A Slang that another copy of the Slang library loaded
    /// has another default loader, so the loader would answer it from the
    /// wrong Slang. That call is unsafe, and its caller upholds this
    /// condition.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    pub safe fn slang_glslang_sys_loader() -> *mut ISlangSharedLibraryLoader;
}
