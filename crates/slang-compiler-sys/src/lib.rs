//! Raw bindings to the Slang shader compiler, generated from the Slang
//! headers.
//!
//! [Slang](https://shader-slang.org) is a shader compiler: it compiles
//! shaders, the programs that run on a graphics processing unit (GPU), from
//! the Slang language to targets such as SPIR-V for Vulkan. Its native library
//! exposes its API through the C++ header `slang.h`. This crate declares that
//! API in Rust exactly as the header defines it, and adds nothing: the C
//! functions, the COM-style interfaces with their vtables and UUIDs, the
//! enumerations, the structs, and the constants. It has no safe API. Every
//! function that calls into Slang is `unsafe`, and the caller manages
//! pointers, reference counts, lifetimes, and threads by hand. Most programs
//! do not use this crate directly. The planned crate `slang-compiler` will
//! wrap these bindings in a safe API, and the planned crate `slango` will
//! build on it to run Slang code on a GPU.
//!
//! A binding generator in the repository of this crate writes every item
//! except the pin from the headers of the pinned Slang build, and no item is
//! written by hand. The items keep the names of the C++ header without its
//! namespace, so `slang::IGlobalSession` is [`IGlobalSession`], and their
//! meaning is the one that Slang documents in its
//! [user guide](https://github.com/shader-slang/slang/tree/c7954ebd1f1984ad2bfde15f76248db41dc34aa3/docs/user-guide)
//! and in the comments of `slang.h`. These are the forms that the C++
//! declarations take in Rust:
//!
//! - A C function is a foreign function, such as
//!   [`slang_createGlobalSession2`], which creates the global session that
//!   every compilation starts from.
//! - A COM-style interface, which is a C++ class of virtual methods that
//!   Slang implements, is a struct that holds only the pointer to its vtable,
//!   such as [`IGlobalSession`], with a vtable struct, such as
//!   [`IGlobalSessionVtbl`]. Each method, including each method of a base
//!   interface, is an associated function that takes the interface pointer
//!   as its first argument and calls through the vtable, such as
//!   [`IGlobalSession::getBuildTagString`]. The UUID of each interface is
//!   the constant `UUID` of the trait [`Interface`].
//! - A C++ enumeration is a struct that wraps its integer type, with one
//!   associated constant for each enumerator, such as
//!   [`SlangCompileTarget::SLANG_SPIRV`], because Slang can return a value
//!   that the header does not list. The result codes are the associated
//!   constants of [`SlangResult`] in the same way, and a negative code means
//!   failure.
//! - A C++ struct is a `#[repr(C)]` struct. A struct whose C++ declaration
//!   gives its fields default values implements [`Default`] with those
//!   values, such as the size field of [`SlangGlobalSessionDesc`].
//! - A type that the header declares without its contents, such as
//!   [`SlangReflectionType`], is a struct that code cannot construct and only
//!   reaches through pointers.
//!
//! The pin is the version of Slang that the bindings match:
//! [`VULKAN_SDK_VERSION`] names the Vulkan SDK that ships that Slang, and
//! [`SLANG_BUILD_TAG`] and [`SLANG_COMMIT`] name the Slang build. These
//! constants are the only record of the pin in the repository, and every tool
//! that needs the pin reads them.
//!
//! # Examples
//!
//! This example creates the global session that every compilation starts
//! from, checks that the Slang library that the program loaded is the build
//! of the pin, and releases the session. It needs the Slang shared library at
//! run time, which the dynamic loader must find as [Linking Slang](#linking-slang)
//! describes, so it is compiled but not run:
//!
//! ```no_run
//! use std::ffi::CStr;
//! use std::ptr;
//!
//! use slang_compiler_sys::{
//!     IGlobalSession, SLANG_BUILD_TAG, SlangGlobalSessionDesc, SlangResult,
//!     slang_createGlobalSession2,
//! };
//!
//! let desc = SlangGlobalSessionDesc::default();
//! let mut session: *mut IGlobalSession = ptr::null_mut();
//! // SAFETY: `desc` is a live descriptor whose size field holds its own size,
//! // which `Default` sets, and `session` is a live out-pointer.
//! let result = unsafe { slang_createGlobalSession2(&raw const desc, &raw mut session) };
//! if result != SlangResult::SLANG_OK || session.is_null() {
//!     return Err(format!("cannot create a global session: {result:?}").into());
//! }
//!
//! // SAFETY: `session` is the live global session that Slang returned above,
//! // and only this thread uses it.
//! let tag = unsafe { IGlobalSession::getBuildTagString(session) };
//! let matches = !tag.is_null()
//!     // SAFETY: `tag` is non-null and points to the null-terminated build tag,
//!     // which Slang keeps for as long as the library stays loaded.
//!     && unsafe { CStr::from_ptr(tag) }.to_bytes() == SLANG_BUILD_TAG.as_bytes();
//!
//! // SAFETY: `session` holds the one reference that Slang returned, and the
//! // example does not use `session` or `tag` after this release.
//! unsafe { IGlobalSession::release(session) };
//! if !matches {
//!     return Err(format!("the loaded Slang library is not the build {SLANG_BUILD_TAG}").into());
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
//! The bindings support 64-bit targets only, which are these:
//!
//! - Linux on x86-64, `x86_64-unknown-linux-gnu`.
//! - Windows on x86-64 with the Microsoft toolchain, `x86_64-pc-windows-msvc`.
//! - macOS on Apple silicon, `aarch64-apple-darwin`, and on x86-64,
//!   `x86_64-apple-darwin`.
//!
//! One set of bindings serves all of them, and compile-time assertions check
//! the size, the alignment, and the offset of every field of every struct on
//! each. Compilation stops with an error on a target whose pointers are not
//! 64 bits wide, because Slang uses another calling convention and other
//! struct layouts there.
//!
//! # Linking Slang
//!
//! The build script of this crate links the Slang shared library of an
//! installed Vulkan SDK, which has to be the SDK that [`VULKAN_SDK_VERSION`]
//! names. The SDK is available from <https://vulkan.lunarg.com>. When the
//! build script runs, it reads the environment variable `VULKAN_SDK`, which
//! holds the absolute path of the directory of the SDK that contains
//! `include/slang` and `lib`. That is the directory `x86_64` of the SDK on
//! Linux, the directory `macOS` on macOS, and the installation directory on
//! Windows. The setup script of the SDK sets `VULKAN_SDK` on Linux and macOS,
//! and its installer sets it on Windows.
//!
//! Before it links anything, the build script reads the build tag that
//! `SLANG_TAG_VERSION` defines in `$VULKAN_SDK/include/slang/slang-tag-version.h`,
//! and it stops the build when that tag differs from [`SLANG_BUILD_TAG`],
//! because the bindings match only that Slang build. No setting skips this
//! check. It then tells Cargo to link the library `slang-compiler`
//! dynamically from `$VULKAN_SDK/lib`, which holds the import library
//! `slang-compiler.lib` on Windows. It uses no pkg-config, sets no run-time
//! search path (rpath), and searches no other location.
//!
//! The build stops with an error that says what to set when `VULKAN_SDK` is
//! unset, empty, or not an absolute path, when the header is missing or does
//! not define the build tag, when the build tag is another one, and when
//! `$VULKAN_SDK/lib` is missing.
//!
//! The environment variable `SLANG_SOURCE_DIR` is reserved for the other way
//! to obtain Slang, which builds Slang from a source tree and links it
//! statically. This version of the crate does not implement it. When
//! `SLANG_SOURCE_DIR` is present, whatever its value, the build stops with an
//! error, and it does not fall back to the Vulkan SDK.
//!
//! When the environment variable `DOCS_RS` is present, whatever its value, as
//! on docs.rs, the build script reads nothing else and links nothing, so the
//! documentation builds without Slang. A program built that way does not link.
//!
//! The manifest key `links` of this crate is `slang-compiler`, so the build
//! script of a crate that depends on this one reads the directory of the
//! Slang headers from `DEP_SLANG_COMPILER_INCLUDE` and the directory of the
//! library from `DEP_SLANG_COMPILER_LIB`. They are `$VULKAN_SDK/include` and
//! `$VULKAN_SDK/lib`, and neither is set when `DOCS_RS` is present.
//!
//! The dynamic loader of the operating system finds the shared library when
//! the program starts, and the build script cannot tell it where the library
//! is, so the environment of the program does:
//!
//! - Linux: `$VULKAN_SDK/lib` is on `LD_LIBRARY_PATH`. The setup script of
//!   the SDK puts it there.
//! - macOS: `$VULKAN_SDK/lib` is on `DYLD_LIBRARY_PATH`. The setup script of
//!   the SDK puts it there. macOS removes the variables whose names start with
//!   `DYLD_` when it starts a protected program, such as `/bin/bash`, so a
//!   variable that a script exports may not reach the program. Setting it on
//!   the command line of Cargo itself, as in
//!   `DYLD_LIBRARY_PATH="$VULKAN_SDK/lib" cargo test`, avoids that.
//! - Windows: `$VULKAN_SDK\Bin`, which holds `slang-compiler.dll`, is on
//!   `PATH`. The installer of the SDK puts it there.
//!
//! Otherwise the program, including a test binary, stops before `main` with
//! an error from the loader. On Linux, the error ends with "cannot open shared
//! object file: No such file or directory".
//!
//! # Safety
//!
//! Every function of this crate that calls into Slang is `unsafe`, and none
//! has a safety section of its own, because the Slang headers state no
//! contract for each function. This contract applies to every foreign
//! function and to every associated function of an interface that calls
//! through its vtable. A call that breaks any part of it is undefined
//! behaviour:
//!
//! - Every pointer that the caller passes is valid, correctly typed, and
//!   live, and was obtained from Slang or points to memory that the caller
//!   owns for as long as Slang reads or writes it. An interface pointer points
//!   to a live object that implements the interface of its type, which the
//!   caller obtained from Slang, such as through `queryInterface` with the
//!   UUID of that interface. A pointer to a type that the header declares
//!   without its contents is a handle that Slang returned and that is still
//!   valid.
//! - The caller meets the contract that Slang documents for the function,
//!   such as which pointers may be null, which strings are terminated by a
//!   null byte, and who releases each object that the function returns.
//! - The caller follows the threading and lifetime rules of Slang that this
//!   section lists.
//!
//! These are the threading and lifetime rules that Slang documents:
//!
//! - A global session is not thread-safe. A program that compiles on several
//!   threads at once creates one global session for each thread. Creating a
//!   global session and calling [`spGetBuildTagString`] are thread-safe.
//! - A global session is released after every object that was created from
//!   it.
//! - Reference counting is not atomic on every interface, so code does not
//!   add or release references to one object from unrelated threads.
//! - A program layout, which reflection returns, is not reference counted and
//!   lives only as long as the component type that it came from.
//!
//! This crate promises nothing about threads. No interface struct implements
//! [`Send`] or [`Sync`], and the planned safe crate `slang-compiler` decides
//! what can cross threads.
//!
//! [`Interface`] is an unsafe trait. A type that implements it promises that
//! it is an interface struct whose vtable has the layout of the Slang
//! interface with that UUID, and an implementation that breaks this promise
//! is undefined behaviour. This crate implements it for every interface of
//! the bindings.

#![expect(
    unsafe_code,
    reason = "the generated bindings declare foreign functions and call through vtables, which only unsafe code can do"
)]

#[expect(
    missing_docs,
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::exhaustive_structs,
    clippy::missing_safety_doc,
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block,
    clippy::similar_names,
    reason = "the generated bindings keep the names of the Slang headers and leave each item's meaning to Slang's documentation, their structs mirror C++ structs that Slang fills in, and the crate documentation states the safety contract that every generated function shares"
)]
mod bindings;

pub use bindings::*;

include!("../pin.rs");
