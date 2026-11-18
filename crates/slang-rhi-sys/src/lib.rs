//! slang-rhi, the render hardware interface of Slang, built from source and
//! linked statically.
//!
//! slang-rhi is the C++ library of the Slang source tree that gives one API
//! over several graphics and compute APIs. This crate builds it with its
//! Vulkan backend on Linux and its Direct3D 12 backend on Windows from the
//! Slang source tree that `slang-src` clones, and links it into the program,
//! together with the Slang that `slang-compiler-sys` builds from the same tree
//! with its feature `vendored`. Slang and slang-rhi therefore always come from
//! one Slang commit, and the crate never uses the Slang of a Vulkan SDK.
//!
//! The crate holds the raw bindings of slang-rhi, which its build script
//! generates in every build from the headers of the slang-rhi that it links,
//! with the binding generator `slang-bindgen` and libclang. They are
//! `unsafe` and follow the C++ API one to one, so a program calls them only
//! under the contract of the section [Safety](#safety).
//!
//! Most programs use the crate `slang-rhi` instead, which wraps the bindings in
//! a safe interface for compute dispatches, and call these bindings only for
//! what it does not cover. The documentation of slang-rhi, at
//! <https://github.com/shader-slang/slang-rhi>, gives the meaning of each
//! item.
//!
//! # The bindings
//!
//! The bindings keep the names of the slang-rhi headers. Each interface of
//! slang-rhi, such as [`IDevice`], is a struct whose only field is its vtable
//! pointer, with a vtable struct such as [`IDeviceVtbl`] and one associated
//! function for each method, own and inherited, which takes the interface
//! pointer as its first argument. Each interface with a UUID implements the
//! trait `Interface` of `slang-compiler-sys`. [`IAdapter`],
//! [`IDebugCallback`], and [`IRHI`] have no UUID and no base, so their vtable
//! starts with their first method and they implement no trait.
//!
//! Every item of the Slang headers that the bindings use, such as
//! `ISlangUnknown`, `ISlangBlob`, `SlangResult`, and the interfaces of a
//! global session, is the item of `slang-compiler-sys`, so a pointer passes
//! between the two crates without a cast.
//!
//! The bindings differ from the headers in these places:
//!
//! - Virtual methods that share a name get one Rust name each. The first
//!   declared keeps the C++ name, and the later one gets a suffix, such as
//!   [`IDevice::readBuffer_blob`] next to [`IDevice::readBuffer`]. Their
//!   fields in the vtable struct follow the vtable of the target: in
//!   declaration order on Linux, and in reverse declaration order on Windows,
//!   where the Microsoft C++ ABI places a run of such methods in reverse.
//! - A virtual method that takes or returns a struct by value keeps its place
//!   in the vtable struct as a field of type `*const c_void` with the name of
//!   the method, and has no associated function, because the calling
//!   convention of a struct passed by value differs between platforms. Such
//!   methods include [`IRHI`]'s `setDebugLayerOptions` and
//!   `getDebugLayerOptions` and [`ICommandEncoder`]'s `clearBuffer`.
//! - A field of type `Slang::ComPtr<T>`, which [`Binding`] has, is a field of
//!   type `*mut T`.
//! - Each struct with default member initializers implements [`Default`]
//!   with the values of the header, and gives each field without an
//!   initializer zero, which is the value of the C++ value-initialization
//!   `T{}` and not the indeterminate value of `T t;`. A field whose
//!   initializer names the pointer constant [`kInvalidCUDAStream`], such as
//!   that of [`SubmitDesc`] and [`HeapAllocDesc`], gets that constant.
//! - The structs whose default the generator cannot reproduce have no
//!   `Default`. These are [`TextureDesc`], [`TextureViewDesc`],
//!   [`SamplerDesc`], [`ClearValue`], and [`RenderPassColorAttachment`],
//!   whose default member initializers include a list in braces or a
//!   constant of a struct type. A caller states every field, and the
//!   documentation of each of their fields gives its C++ default member
//!   initializer. A struct that holds one of them by value has no `Default`
//!   either.
//! - [`kInvalidCUDAStream`] is a pointer constant with the address that the
//!   header gives it.
//! - The headers' inline C++ helpers, their operators on flag enumerations,
//!   and their constants of struct types, such as `kEntireBuffer`, have no
//!   binding. Neither do the structs with bit-fields, such as `FormatInfo`,
//!   and the classes `AdapterList` and `DeviceScope`. A struct without a
//!   binding that a binding uses through a pointer is an opaque type.
//!
//! The functions [`rhiGetInstance`] and [`rhiDestroyInstance`] are the only C
//! functions of slang-rhi.
//!
//! # What the build does
//!
//! The build script runs `cmake` with the Ninja generator on a wrapper
//! project of the crate, which adds slang-rhi from the source tree and builds
//! three static libraries outside the tree:
//!
//! - `slang-rhi`, the library with its Vulkan backend on Linux and its
//!   Direct3D 12 backend on Windows.
//! - On Linux, `slang-rhi-vma`, the Vulkan Memory Allocator, and on Windows,
//!   `slang-rhi-d3d12ma`, the Direct3D 12 Memory Allocator.
//! - `slang-rhi-resources`, the files that slang-rhi embeds.
//!
//! It also builds the library `slang-rhi-sys-abi-probe` from a C++ file that
//! the generator writes. The compiler that builds slang-rhi checks in it the
//! size, the alignment, and every field offset of each struct of the bindings
//! and the size of each enumeration, and the file records the offset of every
//! vtable slot, which the tests of the crate compare with the bindings. A
//! disagreement of a struct stops the build. Only the tests link this
//! library.
//!
//! The build links the first three libraries in that order. On Linux, it
//! follows them with the C++ standard library `stdc++` and the system
//! libraries `dl` and `pthread`. On Windows, the runtime library that the
//! target feature `crt-static` selects provides the C++ standard library, and
//! `slang-compiler-sys` links the system libraries. slang-rhi loads the
//! Vulkan loader `libvulkan.so.1`, or `d3d12.dll` and DXGI on Windows, when
//! it creates a device, so the build links no graphics library.
//!
//! The build reaches no network. It turns off every download that the build
//! files of slang-rhi make by default, such as that of a prebuilt Slang, of
//! the DirectX Shader Compiler, of the Agility SDK, and of NVAPI, and takes
//! the Vulkan headers from the directory `external/vulkan` of the source
//! tree. On Windows, it gives the build of slang-rhi an empty directory in
//! place of `WinPixEventRuntime`, the runtime of the markers of the PIX tool,
//! which slang-rhi loads at run time only when the program provides it, so
//! the markers are not available. On Windows, the build finds the Microsoft
//! C++ compiler `cl.exe` as `slang-compiler-sys` does, so it needs no
//! developer command prompt, and takes the Direct3D 12 headers from the
//! Windows SDK of that toolchain.
//!
//! slang-rhi treats every compiler warning of its own sources as
//! an error, and the wrapper project turns that off, so that a compiler that
//! slang-rhi was never tested with does not stop the build. The warnings stay
//! in the log of the build.
//!
//! A build needs everything that a vendored build of `slang-compiler-sys`
//! needs, including libclang, with which it generates the bindings, which the
//! [documentation of slang-src](https://g-whiz.github.io/slang-rust/slang_src/#what-a-vendored-build-needs)
//! lists. On Linux, it also needs the development headers of Xlib, the C
//! library of the X Window System, such as those of the Debian package
//! `libx11-dev`, because the Vulkan backend of slang-rhi includes
//! `X11/Xlib.h` there. On Windows, it needs an installation of Visual Studio
//! or its Build Tools with the C++ tools and a Windows SDK. The first build in a
//! Cargo target directory clones Slang and builds Slang and slang-rhi from
//! source, which takes minutes.
//!
//! # The shared folder
//!
//! The build keeps the four libraries and the header `slang-rhi-config.h`
//! that the configure step writes in the directory `slang-rhi-sys` of the
//! shared folder, which is the directory `slang-src` in the Cargo target
//! directory that the
//! [documentation of slang-src](https://g-whiz.github.io/slang-rust/slang_src/#the-shared-folder)
//! describes. A later build with the same Slang commit, target, compiler,
//! version of `cmake`, compiler settings, configured header, and ABI probe
//! uses them without building again. Every build runs the configure step and
//! the binding generator before it looks for them, because the ABI probe that
//! the generator writes is one of these inputs. Removing the folder, or any
//! directory in it, is safe whenever no build runs.
//!
//! # Failures
//!
//! When a program of the build cannot start, the build stops with an error
//! that starts with "cannot start" and says that the build needs `cmake`
//! 3.22 or later, Ninja, and a C++20 compiler with, on Linux, the development
//! headers of Xlib, or, on Windows, the Microsoft C++ compiler with the
//! Windows SDK. On Windows, when no `cl.exe` is found, the error starts with
//! "cannot find the Microsoft C++ compiler cl.exe" and says where the build
//! script looked. When the configure or build step fails, the error names the command
//! and the log `build.log` in the output directory of the build script, and
//! ends with the last 40 lines of that log.
//!
//! When the binding generator fails, the build stops with an error that
//! starts with "cannot generate the slang-rhi bindings from" and holds the
//! error of the generator. Without libclang, the error ends with the advice
//! to install libclang or set `LIBCLANG_PATH` to the directory that holds it.
//!
//! Without the development headers of Xlib, the configure step fails before
//! it adds slang-rhi, and the last lines of the log hold the message "The C++
//! compiler cannot find the header X11/Xlib.h", which names the Debian package
//! `libx11-dev` as an example.
//!
//! # Third-party code in a program
//!
//! A program built with this crate contains, besides slang-rhi and Slang,
//! the Vulkan Memory Allocator on Linux or the Direct3D 12 Memory Allocator
//! on Windows, each under the MIT licence, and the runtime of
//! the `cmake` module `CMakeRC`, which embeds files in slang-rhi. A program
//! that ships a binary carries their notices.
//!
//! # Safety
//!
//! Every associated function of an interface and every foreign function of
//! this crate is `unsafe`, and none has a safety section of its own, because
//! the slang-rhi headers state no contract for each function. This contract
//! applies to all of them, and a call that breaks any part of it is undefined
//! behaviour:
//!
//! - Every pointer that the caller passes is valid for the access that the
//!   C++ signature implies, and every interface pointer points to a live
//!   object that implements the interface of its type.
//! - The caller meets the contract that slang-rhi documents for the function,
//!   such as which pointers may be null and who releases each object that the
//!   function returns.
//! - A pointer that a method returns without adding a reference is used only
//!   while its owner keeps it valid and is never released. These are the
//!   adapter of [`IRHI::getAdapter`], which stays valid until
//!   [`rhiDestroyInstance`], the pass encoder of
//!   [`ICommandEncoder::beginComputePass`] and of the other methods that begin
//!   a pass, which lives as long as its command encoder, and the root shader
//!   object that `bindPipeline` returns, which lives until the next
//!   `bindPipeline` of the pass or until the encoder is freed.
//! - Each object that a field of type `*mut T` of [`Binding`] names stays
//!   alive through a reference that the caller owns until the call that takes
//!   the struct returns, because slang-rhi adds its own references while Rust
//!   runs no C++ constructor or destructor of the struct.
//! - [`rhiDestroyInstance`] is never called while a device lives, because
//!   slang-rhi then aborts the process.
//! - No object is used from two threads at once, unless slang-rhi documents
//!   that the object allows it, which it does only for an [`IDebugCallback`]
//!   and an [`ITaskPool`].
//!
//! slang-rhi is written in C++ and calls Slang and the C++ standard library,
//! through which a C++ exception can escape. Every foreign function of this
//! crate is therefore declared with the ABI `"C-unwind"`, and every vtable
//! entry with `"system-unwind"`, which name the same calling conventions as
//! `"C"` and `"system"`. With these declarations, an exception that escapes
//! a call is not undefined behaviour. It unwinds through the caller's Rust
//! frames or aborts the process, as the documentation of `slang-compiler-sys`
//! describes for Slang. The type of a callback that slang-rhi calls keeps the
//! ABI `"C"`, so a panic in a callback aborts the process instead of
//! unwinding into slang-rhi. The two function pointers that
//! [`ITaskPool::submitTask`] receives, `func` and `payloadDeleter`, are the
//! other way round: they are functions of slang-rhi that the task pool calls,
//! so their type has the ABI `"C-unwind"`.
//!
//! A program that implements [`IDebugCallback`] in Rust fills an
//! [`IDebugCallbackVtbl`] with its own `handleMessage` and follows these
//! rules:
//!
//! - It defines `handleMessage` with the non-unwinding ABI, as
//!   `unsafe extern "system" fn`, so that a panic in it aborts the process at
//!   the boundary instead of unwinding into slang-rhi.
//! - It stores the function in the field, whose type is
//!   `unsafe extern "system-unwind" fn`, through a conversion of the function
//!   pointer, such as [`std::mem::transmute`] with both function pointer
//!   types written out. The documentation of the [`fn`](primitive@fn) type in
//!   the standard library makes the conversion well-defined. A vtable in a
//!   `static` needs the conversion in a `const fn`.
//! - The function tolerates calls from several threads at once, and it reads
//!   the message only during the call, because slang-rhi keeps the message
//!   valid only for that long.
//! - The callback object stays alive and at the same address for as long as
//!   a device that names it lives.
//!
//! A program that implements [`ITaskPool`] in Rust, to install it with
//! [`IRHI::setTaskPool`], follows these rules:
//!
//! - It defines each function of its [`ITaskPoolVtbl`] with the non-unwinding
//!   ABI, as `unsafe extern "system" fn`, and stores it in its
//!   `"system-unwind"` field through a conversion of the function pointer, as
//!   for [`IDebugCallback`], so that a panic in it aborts the process at the
//!   boundary instead of unwinding into slang-rhi.
//! - It calls the `func` and `payloadDeleter` that `submitTask` receives
//!   through their `"C-unwind"` type, so that a C++ exception from them is not
//!   undefined behaviour. Such an exception unwinds into the task pool's own
//!   Rust frames, and if it reaches a function of the vtable, the
//!   non-unwinding ABI of that function aborts the process.
//! - It tolerates calls from several threads at once, as slang-rhi documents
//!   for a task pool.
//!
//! The generated code implements neither `Send` nor `Sync` for any type, and
//! the raw pointers of every interface struct make it neither.
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate builds for Linux with the GNU C library, with the Vulkan backend
//! of slang-rhi, and for `x86_64` Windows with the Microsoft toolchain, with
//! its Direct3D 12 backend, on a host of the target. On every other target,
//! including macOS, the crate has no dependencies, and its build stops at once
//! with an error that names the target, before anything is cloned or built.
//!
//! A cross-compilation between two supported targets keeps the dependencies,
//! so the build of `slang-src` clones Slang first, and the build of
//! `slang-compiler-sys` then stops with its own error that it cannot
//! cross-compile. When it gets that far, the build of this crate stops with
//! an error that names both triples. The crate is tested on `x86_64` Linux
//! and on `x86_64` Windows with WARP, the software adapter of Direct3D 12, and
//! another architecture of Linux with the GNU C library is untested.
//!
//! On Linux, a program that creates a device needs the Vulkan loader
//! `libvulkan.so.1` and a Vulkan driver at run time, such as lavapipe, the
//! software driver of Mesa.
//!
//! On Windows, slang-rhi compiles every shader for a device of shader model
//! 6 or later, which WARP is, to DXIL through Slang, which loads
//! `dxcompiler.dll` of the DirectX Shader Compiler at run time. The build
//! neither downloads nor bundles it, so a program that creates a pipeline
//! needs `dxcompiler.dll` on its `PATH` or beside its executable, and
//! providing it is the responsibility of the program. The Windows SDK ships
//! it in its `bin` directory. Without it, the creation of a pipeline fails,
//! and the debug callback receives the messages `failed to load downstream
//! compiler 'dxc'` and `pass-through compiler not found`.

#![expect(
    unsafe_code,
    reason = "the generated bindings declare foreign functions and call through vtables, which only unsafe code can do"
)]

#[cfg(any(
    all(target_os = "linux", target_env = "gnu"),
    all(target_os = "windows", target_env = "msvc", target_arch = "x86_64")
))]
use slang_compiler_sys as _;
#[cfg(any(
    all(target_os = "linux", target_env = "gnu"),
    all(target_os = "windows", target_env = "msvc", target_arch = "x86_64")
))]
use slang_src as _;

#[expect(
    missing_docs,
    non_snake_case,
    non_upper_case_globals,
    non_camel_case_types,
    clippy::exhaustive_structs,
    clippy::missing_safety_doc,
    clippy::undocumented_unsafe_blocks,
    clippy::multiple_unsafe_ops_per_block,
    clippy::upper_case_acronyms,
    clippy::too_many_arguments,
    clippy::derivable_impls,
    reason = "the generated bindings keep the names and signatures of the slang-rhi headers and leave each item's meaning to the documentation of slang-rhi, their structs mirror C++ structs that slang-rhi reads and fills in, each of their Default implementations states the default member initializers of the header, and the crate documentation states the safety contract that every generated function shares"
)]
#[expect(
    ffi_unwind_calls,
    reason = "the thunks call vtable entries declared with the unwinding ABI, because slang-rhi is C++ that calls Slang and the C++ standard library, through which an exception can escape, which is undefined behaviour through a non-unwinding ABI"
)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use bindings::*;
