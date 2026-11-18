//! Raw bindings to the Slang shader compiler, generated from the Slang
//! headers.
//!
//! [Slang](https://shader-slang.org) is a shader compiler: it compiles
//! shaders, the programs that run on a graphics processing unit (GPU), from
//! the Slang language to targets such as SPIR-V, the binary shader format of
//! the Vulkan graphics API. Its native library exposes its API through the C++
//! header `slang.h`. This crate declares that API in Rust exactly as the
//! header defines it, and adds nothing: the C functions, the interfaces, the
//! enumerations, the structs, and the constants. It has no safe API. Every
//! function that calls into Slang is `unsafe`, and the caller manages
//! pointers, reference counts, lifetimes, and threads by hand.
//!
//! Use this crate when you need the Slang API exactly as Slang declares it
//! and accept writing unsafe code, such as to build a safe wrapper of your
//! own. Most programs do not need it directly. The crate `slang-compiler`
//! wraps these bindings in a safe API, which compiles shaders and reflects
//! their parameters, and the crate `slango` builds on that to run compute
//! kernels and draw with render pipelines on a GPU.
//!
//! The items keep the names of the C++ header without its namespace, so
//! `slang::IGlobalSession` is [`IGlobalSession`], and their meaning is the
//! one that Slang documents in its
//! [user guide](https://github.com/shader-slang/slang/tree/c7954ebd1f1984ad2bfde15f76248db41dc34aa3/docs/user-guide)
//! and in the comments of `slang.h`. These are the forms that the C++
//! declarations take in Rust:
//!
//! - A C function is a foreign function, such as
//!   [`slang_createGlobalSession2`], which creates the global session that
//!   every compilation starts from.
//! - A COM-style interface, which is a C++ class of virtual methods that
//!   Slang implements, is a struct that holds only the pointer to its vtable,
//!   such as [`IGlobalSession`]. The vtable is the table of function pointers
//!   through which C++ calls a virtual method, and it is a vtable struct, such
//!   as [`IGlobalSessionVtbl`]. Each method, including each method of a base
//!   interface, is an associated function that takes the interface pointer
//!   as its first argument and calls through the vtable, such as
//!   [`IGlobalSession::getBuildTagString`]. The UUID of each interface, which
//!   is its 128-bit identifier, is the constant [`Interface::UUID`] of the
//!   trait [`Interface`].
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
//! Every build generates the bindings, with the binding generator
//! `slang-bindgen`, from the headers of the Slang that it links, so the
//! declarations always match that library. The constant [`BUILD_INFO`]
//! records which Slang the build used and how it links it. The crate is
//! written and tested against the Slang that the version of `slang-src` that
//! it requires records, which is the Slang of the Vulkan software development
//! kit (SDK) 1.4.363.1. Another Slang is best effort: its headers generate
//! bindings that match it, but the binding generator refuses headers that
//! hold a declaration or a construct that its rules do not cover, and the
//! crates built on this one can depend on a behaviour of the recorded Slang.
//!
//! # Examples
//!
//! This example calls no Slang function. It shows that a struct takes the
//! default values of its C++ declaration, that an enumeration accepts a value
//! that the header does not list, that a failed result code is negative, and
//! what the build information records:
//!
//! ```
//! use slang_compiler_sys::{
//!     BUILD_INFO, Linkage, SlangCompileTarget, SlangGlobalSessionDesc, SlangResult,
//! };
//!
//! let desc = SlangGlobalSessionDesc::default();
//! assert_eq!(usize::try_from(desc.structureSize)?, size_of::<SlangGlobalSessionDesc>());
//!
//! let unlisted = SlangCompileTarget(1000);
//! assert_ne!(unlisted, SlangCompileTarget::SLANG_SPIRV);
//!
//! assert!(SlangResult::SLANG_FAIL.0 < 0);
//! assert!(SlangResult::SLANG_OK.0 >= 0);
//!
//! assert_ne!(BUILD_INFO.slang_build_tag, "");
//! if BUILD_INFO.linkage == Linkage::Static {
//!     assert!(BUILD_INFO.slang_commit.is_some());
//!     assert!(BUILD_INFO.version.ends_with(".vendored"));
//! } else {
//!     assert_eq!(BUILD_INFO.slang_commit, None);
//!     assert!(BUILD_INFO.version.ends_with(".dynamic"));
//! }
//! println!("{} on a tested target: {}", BUILD_INFO.version, BUILD_INFO.tested_target);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! This example creates the global session that every compilation starts
//! from, checks that the Slang library that the program loaded reports the
//! build tag of [`BUILD_INFO`], and releases the session. A default build
//! needs the Slang shared library at run time, which the dynamic loader must
//! find as [Linking Slang](#linking-slang) describes, so it is compiled but
//! not run:
//!
//! ```no_run
//! use std::ffi::CStr;
//! use std::ptr;
//!
//! use slang_compiler_sys::{
//!     BUILD_INFO, IGlobalSession, SlangGlobalSessionDesc, SlangResult, slang_createGlobalSession2,
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
//!     && unsafe { CStr::from_ptr(tag) }.to_bytes() == BUILD_INFO.slang_build_tag.as_bytes();
//!
//! // SAFETY: `session` holds the one reference that Slang returned, and the
//! // example does not use `session` or `tag` after this release.
//! unsafe { IGlobalSession::release(session) };
//! if !matches {
//!     return Err(format!(
//!         "the loaded Slang library is not the build {}",
//!         BUILD_INFO.slang_build_tag
//!     )
//!     .into());
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Features
//!
//! - `vendored`, off by default: builds Slang from source and links it
//!   statically, in place of the Slang shared library of a Vulkan SDK, as
//!   [Linking Slang](#linking-slang) describes. It enables the dependency on
//!   the crate slang-src, which obtains the source tree. It adds and removes
//!   no item of the API, so code that compiles without it compiles with it.
//!   Cargo unifies features, so one crate of a build that enables it makes the
//!   whole build link the static Slang, and enabling every feature of this
//!   crate enables it as well. Every crate of this project between a program
//!   and this crate, which are slang-compiler, slango, slango-codegen,
//!   slango-macros, and slang-slint, has a feature `vendored` that forwards
//!   to this one, and the feature of slang-compiler also installs the loader
//!   of the crate slang-glslang-sys, which the static Slang needs to optimize
//!   SPIR-V.
//!
//! # Platform support
//!
//! The bindings support 64-bit targets only. These four targets are tested:
//!
//! - Linux on x86-64, `x86_64-unknown-linux-gnu`.
//! - Windows on x86-64 with the Microsoft toolchain, `x86_64-pc-windows-msvc`.
//! - macOS on Apple silicon, `aarch64-apple-darwin`, and on x86-64,
//!   `x86_64-apple-darwin`.
//!
//! Every build generates the bindings for its own target, and compile-time
//! assertions check the size, the alignment, and the offset of every field of
//! every struct against the layout that the binding generator read for that
//! target. The tested targets are `x86_64` Linux, `x86_64` Windows, and
//! aarch64 macOS, and the Slang of the Vulkan SDK is linked and its tests run
//! on each. On `x86_64` macOS, the bindings are only type-checked, and they are
//! never linked or run.
//!
//! The static Slang of the feature `vendored` is built and tested on the same
//! three platforms as the Slang of the Vulkan SDK. On Windows, it is tested
//! only with the one installation of Visual Studio of a hosted continuous
//! integration runner and in a target directory with a short path. Another
//! installation of Visual Studio, several installations on one machine, and a
//! deep target directory are untested.
//!
//! Every other 64-bit target is allowed, untested, and unsupported. The build
//! script then emits a warning that names the target, which Cargo shows when
//! the build fails, and [`BuildInfo::tested_target`] is `false`, but the
//! build goes on. The build stops with an error on a target whose pointers
//! are not 64 bits wide, because Slang uses another calling convention and
//! other struct layouts there.
//!
//! With the feature `vendored`, the build also stops when the target differs
//! from the host, because Slang's build runs programs that it builds for the
//! target, so it cannot cross-compile. It stops as well on a target for which
//! it has no rule to link the C++ runtime library, because Slang is written
//! in C++. It has such a rule for Linux with the GNU C library, macOS, and
//! Windows with the Microsoft toolchain.
//!
//! # Linking Slang
//!
//! The build script obtains Slang in one of two ways, and only the feature
//! `vendored` chooses between them:
//!
//! | | Default | With the feature `vendored` |
//! | --- | --- | --- |
//! | Which Slang | The Slang shared library of an installed LunarG Vulkan SDK | Slang built from the source tree that the crate slang-src clones |
//! | How the build links it | Dynamically | Statically |
//! | Build time | Needs the Vulkan SDK and libclang | Needs the tools that [slang-src lists](https://g-whiz.github.io/slang-rust/slang_src/#what-a-vendored-build-needs), and no Vulkan SDK |
//! | Run time | Needs the Slang shared library, which the dynamic loader finds | Needs no Slang library |
//! | First build in a target directory | Generates the bindings, as every build does | Also clones and builds Slang, at the cost that [slang-src describes](https://g-whiz.github.io/slang-rust/slang_src/#what-a-vendored-build-costs) |
//!
//! No environment variable selects the way. The build never falls back from
//! one way to the other, so a build with the feature never links the Slang of
//! an SDK, and a build without it never builds Slang.
//!
//! ## libclang
//!
//! Every build, on either way, needs libclang, the C and C++ parser library
//! of the LLVM project, because the build script generates the bindings with
//! it. The binding generator loads libclang from the directory that the
//! environment variable `LIBCLANG_PATH` names, or otherwise from the standard
//! locations of each platform. On Linux, the package of the distribution,
//! such as `libclang-dev` on Debian and Ubuntu, provides it. On macOS, the
//! Xcode command line tools provide it, and on Windows, an installation of
//! LLVM does.
//!
//! When libclang cannot be loaded, the build stops with an error that starts
//! with "cannot generate the Slang bindings from", names the header directory
//! and the target, and gives the reason that libclang could not be loaded.
//!
//! ## The Slang of a Vulkan SDK
//!
//! By default, the build script links the Slang of the Vulkan SDK, which you
//! can download from <https://vulkan.lunarg.com>. When the build script runs,
//! it reads the environment variable `VULKAN_SDK`, which holds the absolute
//! path of the directory of the SDK that contains `include/slang` and `lib`.
//! That is the directory `x86_64` of the SDK on Linux, the directory `macOS`
//! on macOS, and the installation directory on Windows. The setup script of
//! the SDK sets `VULKAN_SDK` on Linux and macOS, and its installer sets it on
//! Windows.
//!
//! The build script reads the build tag that `SLANG_TAG_VERSION` defines in
//! `$VULKAN_SDK/include/slang/slang-tag-version.h` and records it in
//! [`BUILD_INFO`]. It accepts the Slang of any SDK and compares the build tag
//! with no recorded version. It then generates the bindings from the headers
//! in `$VULKAN_SDK/include/slang` and tells Cargo to link the library
//! `slang-compiler` dynamically from `$VULKAN_SDK/lib`. On Windows, that
//! directory holds the import library `slang-compiler.lib`, the file that tells
//! the linker which functions the library exports. The build script uses no
//! pkg-config, sets no run-time search path (rpath), and searches no other
//! location.
//!
//! The build stops with an error that says what to set when `VULKAN_SDK` is
//! unset, empty, or not an absolute path, when the header is missing or does
//! not define the build tag as one string literal, when `$VULKAN_SDK/lib` is
//! not a directory or cannot be accessed, and when the binding generator
//! refuses the headers. When the operating system
//! cannot report on a path, the error includes the reason that it gives, such
//! as a denied permission. When `VULKAN_SDK` is unset, the error starts with
//! "`VULKAN_SDK` is not set" and ends with the advice that the feature
//! `vendored` builds Slang from source instead.
//!
//! When the target has the operating system of the host, the build also stops
//! when `$VULKAN_SDK/lib` does not hold the Slang library of the target:
//! `libslang-compiler.so` on Linux, `libslang-compiler.dylib` on macOS, and
//! `slang-compiler.lib` on Windows. For a target with another operating
//! system, such as a check of the Windows target on Linux, the build script
//! does not look for the library, because the SDK of the host holds only the
//! library of the host. Commands that do not link, such as `cargo check` and
//! `cargo clippy`, then work, and a build that links fails in the linker.
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
//!   `DYLD_LIBRARY_PATH="$VULKAN_SDK/lib" cargo test`, is expected to avoid
//!   that, but this has not been verified on a Mac with that protection
//!   enabled.
//! - Windows: `$VULKAN_SDK\Bin`, which holds `slang-compiler.dll`, is on
//!   `PATH`. The installer of the SDK puts it there.
//!
//! Otherwise the program, including a test binary, stops before `main` with
//! an error from the loader. On Linux, the error ends with "cannot open shared
//! object file: No such file or directory".
//!
//! ## Slang built from source
//!
//! With the feature `vendored`, the build script builds Slang from the source
//! tree that the crate slang-src clones, and it never reads `VULKAN_SDK`. The
//! [documentation of slang-src](https://g-whiz.github.io/slang-rust/slang_src/)
//! says what this build needs and costs, which Slang commit it builds, how to
//! build another commit, and where the build keeps the clone and the
//! libraries that it builds.
//!
//! The build script first finds its tools. It runs `ninja --version`. With the
//! Microsoft toolchain, it finds the compiler `cl.exe` of the developer
//! command prompt that the build runs in, or otherwise of the newest
//! installation of Visual Studio, and gives its environment to `cmake` and
//! `ninja`, so the build needs no developer command prompt. The environment
//! variable `VCToolsVersion` selects another installed version of the
//! toolset. With any other toolchain, it runs the C++ compiler that the
//! environment variable `CXX` names, or `c++` when `CXX` is unset or empty,
//! with `--version`. The build stops with an error in these cases:
//!
//! - A program that the build runs cannot start. The error starts with
//!   "cannot start" and names the command.
//! - With the Microsoft toolchain, no `cl.exe` is found. The error starts
//!   with "cannot find the Microsoft C++ compiler cl.exe" and says where the
//!   build script looked.
//!
//! The build script copies the files that git tracks in the source tree into
//! its `OUT_DIR`, because Slang's build writes into its source tree, and
//! builds a minimal static Slang there with `cmake` and `ninja`. It writes
//! the output of `cmake` and `ninja` to `slang/build.log` under `OUT_DIR`, and
//! a failed step stops the build with an error that names the failed command
//! and the step, names the log, and holds its last 40 lines. It then checks
//! that the build produced the six static libraries and that its
//! `slang-tag-version.h` names the build tag of the source tree,
//! and it generates the bindings from the headers that the build produced.
//! [`BUILD_INFO`] records that build tag, the full commit of the source tree,
//! and [`Linkage::Static`].
//!
//! The build links the six static libraries `slang-compiler`,
//! `compiler-core`, `core`, `cmark-gfm`, `miniz`, and `lz4`, and the C++
//! standard library of the target: `stdc++` on Linux and `c++` on macOS. With
//! the Microsoft toolchain, the runtime library that the target feature
//! `crt-static` selects provides it. It also links the system libraries that
//! Slang's own build links: `pthread` and `dl` on Linux, which Slang's
//! library `core` declares, and with the Microsoft toolchain the libraries
//! that `cmake` links into every program and library by default, which are
//! `kernel32`, `user32`, `gdi32`, `winspool`, `shell32`, `ole32`,
//! `oleaut32`, `uuid`, `comdlg32`, and `advapi32`, because Slang's code calls
//! functions of some of them without declaring them. Cargo copies the static
//! libraries into the library of this crate, so a program that links it needs
//! no Slang library at run time.
//!
//! The build script keeps the libraries and headers that it built as an entry
//! in the directory `slang-compiler-sys` of the shared folder, which the
//! [documentation of slang-src](https://g-whiz.github.io/slang-rust/slang_src/#the-shared-folder)
//! describes, and a later build uses that entry. A build builds Slang again,
//! into a new entry, when any of these differs from every existing entry: the
//! version of this crate, the Slang commit, the target, the version of
//! `cmake`, the compiler, the options of the build of Slang, or one of the
//! environment variables `CC`, `CXX`, `CFLAGS`, `CXXFLAGS`, `LDFLAGS`,
//! `CMAKE_TOOLCHAIN_FILE`, `MACOSX_DEPLOYMENT_TARGET`, and `VCToolsVersion`.
//!
//! The vendored Slang of this crate alone has no slang-glslang, the part of
//! Slang that optimizes SPIR-V. Unless the program installs a loader that
//! provides it, the first generation of SPIR-V in each global session
//! returns diagnostics that begin with `failed to load downstream compiler
//! 'spirv-opt'` while it succeeds, and the SPIR-V is not optimized. When the
//! dynamic loader finds a slang-glslang shared library, Slang loads it
//! instead, so the output then depends on the environment of the program.
//! The crate slang-glslang-sys builds slang-glslang from the same source tree
//! and provides that loader. slang-compiler with its feature `vendored`
//! installs it on every global session that it creates, and a program that
//! uses this crate directly depends on slang-glslang-sys and installs the
//! loader through [`IGlobalSession::setSharedLibraryLoader`] on each global
//! session, before the global session generates any code, as the
//! [documentation of slang-glslang-sys](https://g-whiz.github.io/slang-rust/slang_glslang_sys/)
//! shows. On either way of obtaining Slang, Slang validates the SPIR-V that
//! it generates only when the environment variable
//! `SLANG_RUN_SPIRV_VALIDATION` is `1`.
//!
//! With the Microsoft toolchain, the vendored Slang is built with its support
//! for DXIL, the shader format of Direct3D 12, and on every other target
//! without it. Slang compiles to DXIL through `dxcompiler.dll`, the library of
//! the DirectX Shader Compiler, which it loads at run time, and the build
//! neither downloads nor bundles that library. A program on Windows that
//! compiles a shader to DXIL therefore needs `dxcompiler.dll` on its `PATH`
//! or beside its executable, and providing it is the responsibility of the
//! program. The Windows SDK ships it in its `bin` directory. Without it, the
//! compilation fails, and Slang reports `failed to load downstream compiler
//! 'dxc'`.
//!
//! ## Metadata for build scripts
//!
//! The manifest key `links` of this crate is `slang-compiler`, so the build
//! script of a crate that depends on this one reads what this build links
//! from these variables:
//!
//! - `DEP_SLANG_COMPILER_INCLUDE`: the directory that holds `slang.h` and
//!   `slang-tag-version.h`, which is `$VULKAN_SDK/include/slang` by default
//!   and the directory `include` of the built Slang in the shared folder with
//!   the feature `vendored`.
//! - `DEP_SLANG_COMPILER_LIB`: the directory of the libraries, which is
//!   `$VULKAN_SDK/lib` by default and the directory `lib` of the built Slang
//!   in the shared folder with the feature `vendored`.
//! - `DEP_SLANG_COMPILER_LINKAGE`: `dynamic` by default and `static` with the
//!   feature `vendored`.
//! - `DEP_SLANG_COMPILER_BUILD_TAG`: the build tag of [`BUILD_INFO`].
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
//!   caller obtained from Slang, such as through
//!   [`queryInterface`](ISlangUnknown::queryInterface) with the
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
//! - Reference counting is not guaranteed to be atomic on every interface, so
//!   code does not add or release references to one object from unrelated
//!   threads.
//! - A program layout, which reflection returns, is not reference counted and
//!   lives only as long as the component type that it came from. Reflection
//!   is the part of the API that describes the parameters and types of
//!   compiled shader code, and a component type is a unit of Slang code that
//!   can be linked, such as a module or an entry point, which is the function
//!   where a shader starts to run.
//!
//! Slang is written in C++ and raises its internal errors as C++ exceptions.
//! Most calls catch them, but some calls of the recorded Slang let an exception
//! escape for some inputs. Every foreign function of this crate is therefore
//! declared with the ABI `"C-unwind"`, and every vtable entry with
//! `"system-unwind"`, which name the same calling conventions as `"C"` and
//! `"system"`. With these declarations, an exception that escapes a call is
//! not undefined behaviour. When the program is built with `panic = "unwind"`,
//! the exception unwinds through the caller's Rust frames and runs their
//! destructors, and the process aborts when it reaches
//! [`std::panic::catch_unwind`] or the root of the thread, or the exception is
//! returned as an opaque error, which Rust leaves unspecified. When the
//! program is built with `panic = "abort"`, the process aborts as soon as the
//! exception reaches a Rust frame. A caller that must not run its destructors
//! during such an exception stops the unwinding itself, for example with a
//! value whose destructor aborts the process. The type of a callback that
//! Slang calls keeps the ABI `"C"`, so a panic in a callback aborts the
//! process instead of unwinding into Slang.
//!
//! A program that implements a Slang interface in Rust, such as a file
//! system or a shared library loader that it passes to Slang, fills a vtable
//! struct with its own functions. Each field of a vtable struct has the type
//! `unsafe extern "system-unwind" fn`, so a function defined with that ABI
//! fits the field without a conversion, and a panic in it would unwind into
//! the C++ frames of Slang that called it. To keep a panic out of Slang,
//! follow these rules:
//!
//! - Define each function of the vtable with the non-unwinding ABI, as
//!   `unsafe extern "system" fn`, so that a panic in it aborts the process at
//!   the boundary instead of unwinding.
//! - Store the function in the field through a conversion of the function
//!   pointer to the type of the field, such as [`std::mem::transmute`] with
//!   both function pointer types written out. The documentation of the
//!   [`fn`](primitive@fn) type in the standard library makes the conversion
//!   well-defined in Rust, and Rust code may call the function through the
//!   converted pointer. Slang's C++ code calls the function by the calling
//!   convention of the platform, which is what the non-unwinding ABI
//!   declares, and the function itself stays non-unwinding, so a panic in it
//!   aborts at the boundary. A vtable in a `static` needs the conversion in a
//!   `const fn`.
//! - When the function reports failure through a result code, it may catch
//!   a panic with [`std::panic::catch_unwind`] and return a failure code
//!   instead. `catch_unwind` catches nothing when the program is built with
//!   `panic = "abort"`, where the panic aborts the process.
//!
//! This crate promises nothing about threads. No interface struct implements
//! [`Send`] or [`Sync`], and the safe crate `slang-compiler` decides what can
//! cross threads.
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
#[expect(
    ffi_unwind_calls,
    reason = "the thunks call vtable entries declared with the unwinding ABI, because Slang is C++ and some of its calls let an exception escape, which is undefined behaviour through a non-unwinding ABI"
)]
mod bindings {
    include!(concat!(env!("OUT_DIR"), "/bindings.rs"));
}

pub use bindings::*;
#[cfg(feature = "vendored")]
use slang_src as _;

/// The record of the Slang that a build of this crate generated its
/// bindings from and links, and of how it links it.
///
/// Use it to report which Slang a program uses, or to check that the Slang
/// library that a program loads at run time is the one that the build
/// generated the bindings from. The build script writes the values, so they
/// describe the build and not the source of this crate. [`BUILD_INFO`] holds
/// the record of the current build.
///
/// # Examples
///
/// This example prints the version of the build and the Slang that it uses:
///
/// ```
/// use slang_compiler_sys::BUILD_INFO;
///
/// println!(
///     "{} uses Slang {} on a tested target: {}",
///     BUILD_INFO.version, BUILD_INFO.slang_build_tag, BUILD_INFO.tested_target
/// );
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct BuildInfo {
    /// The build tag of the Slang of the build, such as
    /// `2026.17-25-gc7954ebd1`.
    ///
    /// A build tag is the output of `git describe` on the Slang commit: the
    /// nearest release tag, the number of commits after it, and `g` followed
    /// by the abbreviated commit hash. It is the value that
    /// `SLANG_TAG_VERSION` defines in `slang-tag-version.h` of the Vulkan SDK
    /// that the build used, or the build tag of the source tree that a build
    /// with the feature `vendored` built. The Slang library returns the same string from
    /// [`spGetBuildTagString`], so a program can compare the two at run time.
    pub slang_build_tag: &'static str,
    /// The full hash of the Slang commit of the build, when the build knows
    /// it.
    ///
    /// It is the commit of the source tree for a build with the feature
    /// `vendored`, and `None` for a build that links the Slang of a Vulkan
    /// SDK, because the SDK records no full commit.
    pub slang_commit: Option<&'static str>,
    /// How the build links Slang.
    pub linkage: Linkage,
    /// Whether the target of the build is one of the tested targets that
    /// [Platform support](crate#platform-support) lists.
    pub tested_target: bool,
    /// The full version of the build, which names the Slang and the way the
    /// build obtained it, such as
    /// `0.1.0-alpha.2+slang.2026.17.0.25.gc7954ebd1.dynamic`.
    ///
    /// It is the version of this crate, a plus sign, and build metadata made
    /// of `slang`, the numbers of the build tag with a missing patch or
    /// commit count written as `0`, `g` followed by the abbreviated commit
    /// hash of the build tag, and `dynamic` for a build that links the Slang
    /// of a Vulkan SDK or `vendored` for a build with the feature `vendored`. A build tag of another form appears with every
    /// character other than an ASCII letter, a digit, or a hyphen replaced by
    /// a hyphen.
    pub version: &'static str,
}

/// How a build of this crate links Slang.
///
/// Use it to tell whether a program needs the Slang shared library at run
/// time. [`BuildInfo::linkage`] holds the linkage of the current build. Its
/// text, which [`Display`](std::fmt::Display) writes, is `dynamic` or
/// `static`, the value of `DEP_SLANG_COMPILER_LINKAGE`.
///
/// # Examples
///
/// This example says whether the program needs the Slang shared library:
///
/// ```
/// use slang_compiler_sys::{BUILD_INFO, Linkage};
///
/// if BUILD_INFO.linkage == Linkage::Dynamic {
///     println!("this program loads the Slang shared library when it starts");
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Linkage {
    /// The build links the Slang shared library of a Vulkan SDK, which the
    /// dynamic loader finds when the program starts.
    Dynamic,
    /// Slang is linked statically into the program, which then needs no
    /// Slang library at run time. A build with the feature `vendored` builds
    /// Slang from source and links it in this way.
    Static,
}

impl std::fmt::Display for Linkage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Dynamic => "dynamic",
            Self::Static => "static",
        })
    }
}

/// The record of the Slang that this build generated its bindings from and
/// links.
///
/// Use it to report the Slang of a program, such as in its version output, or
/// to compare it with the build tag that the loaded library reports.
/// [`BuildInfo`] describes each field and has an example.
pub const BUILD_INFO: BuildInfo = include!(concat!(env!("OUT_DIR"), "/build_info.rs"));
