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
//! This example prepares the descriptor that [`slang_createGlobalSession2`]
//! takes, and reads the UUID that `queryInterface` takes to ask an object for
//! the interface [`IGlobalSession`]. It calls no Slang function, so it needs
//! no Slang library:
//!
//! ```
//! use slang_compiler_sys::{IGlobalSession, Interface, SlangGlobalSessionDesc};
//!
//! let desc = SlangGlobalSessionDesc::default();
//! assert_eq!(
//!     usize::try_from(desc.structureSize)?,
//!     size_of::<SlangGlobalSessionDesc>()
//! );
//!
//! let uuid = <IGlobalSession as Interface>::UUID;
//! println!("IGlobalSession has the UUID {:08x}-{:04x}-{:04x}", uuid.data1, uuid.data2, uuid.data3);
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
//! This version of the crate does not link the Slang library, because it has
//! no build script. A program that calls a function of this crate links the
//! Slang shared library of the pinned Vulkan SDK itself, or it fails to link.
//! On Linux, for example, the build script of the program prints
//! `cargo::rustc-link-search=native=` followed by the directory `lib` of the
//! SDK, and `cargo::rustc-link-lib=dylib=slang-compiler`. A program that uses
//! only the types and constants of this crate, as the example above does,
//! needs no Slang library.
//!
//! The dynamic loader of the operating system finds the shared library when
//! the program starts, so the directory `lib` of the SDK is on
//! `LD_LIBRARY_PATH` on Linux and on `DYLD_LIBRARY_PATH` on macOS, and the
//! directory that holds the Slang DLL is on `PATH` on Windows. Otherwise the
//! program stops before `main` with an error from the loader.
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
