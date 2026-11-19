//! Compiles a Slang file while a crate builds and generates typed Rust
//! bindings to its compute kernels, for `slango`.
//!
//! [Slang](https://shader-slang.org) is a language for shaders, the programs
//! that run on a graphics processing unit (GPU). The crate
//! [`slango`](../slango/index.html) runs the compute entry points of a Slang
//! module, its kernels, which are functions that run a general computation on
//! the GPU, and sets each parameter by name. A misspelled name or a value of
//! the wrong type therefore fails only when the program runs.
//!
//! The macro [`shader!`] turns these mistakes into compile errors. It
//! compiles a Slang file while the calling crate builds, and generates a
//! struct `Shader` with a typed kernel for each compute entry point. The
//! documentation of [`shader!`] lists every generated item.
//!
//! Use the macro when the Slang file is part of the crate, and the runtime
//! API of `slango` when a program chooses its shaders while it runs. The
//! generator behind the macro is the crate [`slango_codegen`].
//!
//! The generated code names `slango` by the path `::slango`, so the calling
//! crate depends on `slango` directly, under that name, as well as on this
//! crate. `slango` does not re-export the macro.
//!
//! The generated code follows the backend of the `slango` of the calling
//! crate, its feature `wgpu` or `rhi`, as the documentation of [`shader!`]
//! describes, so this crate has no feature that chooses the backend.
//!
//! # Examples
//!
//! This example calls the macro for a Slang file beside the Rust file and
//! runs its kernel `saxpy`, which computes `result = a * x + y`. The macro
//! resolves the path against the directory of the Rust file that calls it.
//! A doctest has no Slang file beside it, so the example is not a doctest:
//!
//! ```text
//! mod saxpy {
//!     slango_macros::shader!("saxpy.slang");
//! }
//!
//! fn run(device: &slango::Device) -> slango::Result<Vec<f32>> {
//!     let x = slango::Buffer::from_slice(device, &[1.0_f32, 2.0, 3.0, 4.0])?;
//!     let y = slango::Buffer::from_slice(device, &[1.0_f32; 4])?;
//!     let result = slango::Buffer::<f32>::zeroed(device, 4)?;
//!     let shader = saxpy::Shader::load(device)?;
//!     let args = saxpy::SaxpyArgs { a: 2.0, count: 4, x: &x, y: &y, result: &result };
//!     shader.saxpy.dispatch(&args, [4, 1, 1])?;
//!     result.read()
//! }
//! ```
//!
//! The example program `typed-kernel` in the repository of this crate calls
//! the macro in the same way, runs the kernel, and checks its results.
//!
//! # Compile errors
//!
//! The macro reports every failure as a compile error at the call. The
//! message is the text of the error and of each of its causes, as
//! [`slango_codegen::Error::report`] gives it, and the causes of a Slang
//! file that does not compile hold the messages of Slang. The documentation
//! of [`shader!`] lists the failures.
//!
//! # Versions
//!
//! This crate requires [`slango_codegen`] of exactly its own version. That
//! crate explains why `slango` must have the same version too, so depend on
//! the `slango` of the same version as this crate.
//!
//! The generated code also checks at compile time that the `slango` it
//! builds against encodes compiled modules as the generator did. A mismatch
//! fails to compile at that check, or fails when `Shader::load` runs.
//!
//! # Lints
//!
//! The generated `Shader` keeps its module in a private field beside the
//! public field of each kernel. A generated struct of the element of a
//! structured buffer keeps its padding, or a field of zero size where its
//! layout has no padding, in private fields. A crate that enables the Clippy
//! lint `clippy::partial_pub_fields` therefore puts an `#[expect]` of that
//! lint, with a reason, on the module that calls the macro.
//!
//! A generated `new` with more than seven arguments carries an `#[expect]`
//! of `clippy::too_many_arguments`. A crate that raises the Clippy setting
//! `too-many-arguments-threshold` to that number of arguments or more gets
//! the warning `unfulfilled_lint_expectations` on that `new`.
//!
//! # Limits
//!
//! The macro has these limits:
//!
//! - A C++ exception that escapes Slang during the expansion ends the
//!   compiler, with one line on standard error and no compile error.
//! - The compiled code that the macro embeds depends on the Slang of the
//!   machine that builds the crate.
//! - The generated code holds the absolute paths of the Slang file and of
//!   every file that Slang read for it, on the machine that builds the
//!   crate.
//! - A new file that would change how an `import` resolves does not make
//!   Cargo build the crate again.
//! - Each call runs Slang again on every compilation of the calling crate.
//! - A release build compiles `slango` and its dependencies twice, once for
//!   the program and once for the macro, which runs inside the compiler.
//! - The program loads the Slang library when it starts, through `slango`,
//!   although `Shader::load` makes no call into Slang with the feature `wgpu`
//!   of `slango`.
//! - Each call compiles the Slang file twice, once more for the layout of
//!   structures with the feature `rhi` of `slango`.
//! - That second compilation does not define the preprocessor macros that a
//!   device defines for its graphics API, such as `__VULKAN__` or
//!   `__D3D12__`. Where the structures or the kernels of a file depend on
//!   them, the generated bindings can differ from what `Shader::load`
//!   reflects with the feature `rhi`, and the load or a dispatch then fails
//!   with an error of `slango` instead of running with the wrong layout.
//! - With the feature `rhi` of `slango`, the program holds the text of the
//!   Slang file and of every module that it imports, with their absolute
//!   paths on the machine that built the crate.
//! - With the feature `rhi` of `slango`, a file that the Slang file or a
//!   module that it imports includes with `#include`, or a part of a module
//!   that it adds with `__include`, is not embedded. `Shader::load` reads it
//!   from its path on the machine that built the crate, as it is when the
//!   program runs, and fails with the error kind `Compile` of `slango` where
//!   that file does not exist. A module that `import` names is embedded and
//!   has no such limit.
//!
//! # Features
//!
//! - `vendored`, off by default: enables the feature `vendored` of
//!   slango-codegen, so that the macro compiles with a Slang built from
//!   source and linked statically into the macro, in place of the Slang
//!   shared library of a Vulkan SDK, and the build needs no Vulkan SDK for
//!   the macro. It adds and removes no item of the API.
//!
//! Cargo resolves the features of the macro, which runs on the build
//! machine, apart from those of the program. The feature `vendored` of
//! slango therefore does not reach the Slang that the macro links, and the
//! feature of this crate does not reach the Slang that the program links. A
//! program that needs no Vulkan SDK at build time enables `vendored` on both:
//!
//! ```toml
//! [dependencies]
//! slango = { version = "0.1.0-alpha.2", features = ["vendored"] }
//! slango-macros = { version = "0.1.0-alpha.2", features = ["vendored"] }
//! ```
//!
//! Both copies of Slang are built once from one source tree, because the
//! macro and the program share the shared folder of the Cargo target
//! directory in which the crate slang-src keeps the source tree and the
//! built libraries. The crate documentation of slang-src describes that
//! folder and what the build needs.
//!
//! # Platform support
//!
//! The crate supports the platforms of `slango`, which are Linux on x86-64,
//! Windows on x86-64, and macOS on Apple silicon and on x86-64. The
//! generated code runs on the backend of the program's `slango`, so with
//! its feature `rhi` it builds only where that backend builds, on Linux with
//! the GNU C library and on Windows on x86-64.
//!
//! # Finding the Slang library
//!
//! The macro runs Slang inside the compiler, so building a crate that calls
//! it needs Slang, as building `slango` does. By default, that is the Slang
//! of the Vulkan SDK that `VULKAN_SDK` names, as the crate
//! `slang-compiler-sys` describes. With the feature `vendored`, it is a Slang
//! built from source and linked statically into the macro, as
//! [Features](#features) describes, and the rest of this section does not
//! apply. On a machine without a Vulkan SDK, a build with the feature on only
//! one of `slango` and this crate stops with the error that starts
//! `VULKAN_SDK is not set`, because the other copy of Slang still links the
//! Slang of a Vulkan SDK. A documentation build expands the macro with Slang
//! like any other build.
//!
//! The feature `rhi` of `slango` always builds the program's copy of Slang
//! from source, and it does not reach the macro's copy either. A program on
//! that backend without a Vulkan SDK at build time therefore enables
//! `vendored` on this crate:
//!
//! ```toml
//! [dependencies]
//! slango = { version = "0.1.0-alpha.2", default-features = false, features = ["rhi"] }
//! slango-macros = { version = "0.1.0-alpha.2", features = ["vendored"] }
//! ```
//!
//! The macro's copy of `slango` keeps the backend `wgpu`, because it only
//! compiles the Slang file and creates no device. Cargo resolves the
//! features of the two copies apart, so the program's copy keeps exactly one
//! backend.
//!
//! With the Vulkan SDK, the macro library links the Slang shared library, so
//! the compiler has to find that library when it loads the macro. Without
//! it, every build of a crate that calls the macro fails with error `E0463`,
//! which names the crate `slango_macros` and not the missing library.
//!
//! On Linux and macOS, the build script of this crate stores
//! `$VULKAN_SDK/lib` in the macro library as its rpath, a directory in which
//! the dynamic loader looks, so the build needs no library path. The build
//! script stops when `VULKAN_SDK` is not valid UTF-8 or contains a comma,
//! which the linker would misread.
//!
//! On macOS, also put `$VULKAN_SDK/lib` on `DYLD_LIBRARY_PATH`, because the
//! rpath alone may not suffice there. On Windows, a library has no rpath, so
//! `$VULKAN_SDK\Bin`, which holds `slang-compiler.dll`, has to be on `PATH`
//! when the compiler runs.

use proc_macro::TokenStream;
use syn::LitStr;

const EXPECTED: &str = "shader! takes one string literal, the path of a Slang file";

/// Compiles the Slang file at a path and generates typed bindings to its
/// compute kernels where the macro is called.
///
/// Call the macro once in a module of its own, with one string literal, the
/// path of the `.slang` file, relative to the directory of the Rust file
/// that calls the macro, as for [`include_str!`]. A compute entry point, or
/// kernel, is a function of the file marked `[shader("compute")]`, which
/// runs as a general computation on the graphics processing unit (GPU). The
/// macro generates these items in the module:
///
/// - `Shader`, with `Shader::load`, which loads the module on a
///   [`slango::Device`](../slango/struct.Device.html) from what the crate
///   embeds, and `Shader::module`, which returns the
///   [`slango::Module`](../slango/struct.Module.html).
/// - For each compute entry point, a kernel struct whose name is the name of
///   the entry point in upper camel case, such as `Saxpy` for `saxpy`, and
///   a public field of `Shader` for it whose name is the name in snake case.
///   The kernel struct has `THREAD_GROUP_SIZE`, `dispatch`, which runs the
///   kernel on a number of threads, `dispatch_groups`, which runs it on a
///   number of thread groups, and `kernel`, which returns the
///   [`slango::Kernel`](../slango/struct.Kernel.html). A thread group is a
///   set of threads that run together on the GPU, whose size the attribute
///   `[numthreads]` of the entry point sets.
/// - For each kernel, an arguments struct, such as `SaxpyArgs`, with one
///   public field for each parameter, named after its path in snake case. A
///   plain value is a `bool`, `i32`, `u32`, or `f32`, or an array of them for
///   a vector, a matrix, or an array. A structured buffer, a buffer in GPU
///   memory that holds an array of elements of one type, is a
///   [`&slango::Buffer`](../slango/struct.Buffer.html) of `i32`, `u32`,
///   `f32`, or an array of four of them. A texture, an image in GPU memory,
///   is a [`&slango::Texture`](../slango/struct.Texture.html), and a sampler,
///   which describes how a shader reads a texture, is a
///   [`&slango::Sampler`](../slango/struct.Sampler.html). A structure among
///   the values or the elements of a structured buffer has the generated
///   struct of that structure as its type.
/// - For each Slang structure that a kernel takes, a struct of the same name
///   with one public field for each field of the structure and a `const fn
///   new` that takes them all. Each structure is generated once, however
///   many kernels take it.
///
/// A structure that is the element of a structured buffer gets the layout
/// that Slang gives it there, the size of the structure and the offset in
/// bytes of each field. Its struct is `#[repr(C)]`, holds private padding,
/// unused bytes, wherever the layout has a gap, and asserts its size and the
/// offset of each field at compile time. Where the layout has no gap, the
/// struct holds instead one private field of zero size, which changes
/// neither its size nor its layout. It binds as the element of a
/// [`slango::Buffer`](../slango/struct.Buffer.html), and code creates it with
/// `new`, because the private field rules out a struct literal, so the same
/// code compiles with either layout.
///
/// For a structure `Wide` of eight 4-byte fields, which has no gap in either
/// layout, `new` creates a value:
///
/// ```no_run
/// # mod layouts {
/// #     slango_macros::shader!("../tests/macros/shaders/layouts.slang");
/// # }
/// let wide = layouts::Wide::new(0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0);
/// ```
///
/// A struct expression does not compile, because of the private field. The
/// compiler gives that error no error code:
///
/// ```compile_fail
/// # mod layouts {
/// #     slango_macros::shader!("../tests/macros/shaders/layouts.slang");
/// # }
/// let wide = layouts::Wide { a: 0.0, b: 0.0, c: 0.0, d: 0.0, e: 0, f: 0, g: 0, h: 0 };
/// ```
///
/// A structure that is only a value has no padding, because a dispatch sets
/// each of its fields by its path through the runtime API of `slango`, so
/// every check of `slango` still runs.
///
/// The generated code follows the backend of the `slango` of the calling
/// crate, which a build has exactly one of:
///
/// - With the feature `wgpu` of `slango`, the crate embeds the code that the
///   macro compiled, and `Shader::load` creates it on the device with no call
///   into Slang. A struct of the element of a structured buffer has the
///   std430 layout that Slang gives the structure for the GPU code of wgpu.
/// - With the feature `rhi` of `slango`, the crate embeds the Slang source of
///   the file and of every module that it imports, and `Shader::load`
///   compiles them for the device, so each load takes about as long as a load
///   of the file with [`slango::Device::load_module`](../slango/struct.Device.html#method.load_module).
///   A struct of the element of a structured buffer has the packed layout
///   that Slang gives the structure there, in which a structure has no
///   padding between scalars, vectors, and arrays.
///
/// The macro still compiles the file while the crate builds, so that a
/// mistake in the file is a compile error on either backend. A dispatch
/// checks that the stride of a struct and the offset and the type of each of
/// its fields match those that the load reflects, and fails with the error
/// kind `InvalidArgument` of `slango`, naming the struct and the field, where
/// they differ. A field of another scalar type or number of components, such
/// as an `int3` where the struct holds a `float3`, fails even at the same
/// offset.
///
/// `Shader`, the kernel structs, and the arguments structs hold objects of
/// `slango` that stay on the thread of their device, so none of them is
/// [`Send`] or [`Sync`].
///
/// The crate builds again whenever the Slang file or a file that it imports
/// changes.
///
/// The macro reports each of these failures as a compile error at the call:
///
/// - Input other than one string literal.
/// - A Slang file that cannot be read or compiled.
/// - A parameter for which the macro has no Rust type, such as a structured
///   buffer of `float3` or a 64-bit scalar.
/// - A field of a structure in a structured buffer that a Rust struct cannot
///   lay out as Slang does, such as a `bool`, a matrix, or a `float3[2]`.
/// - Two Slang names that give the same Rust name, such as two different
///   structures of one name, or a Slang name that gives no usable Rust name,
///   such as `self`.
///
/// # Examples
///
/// See the example of the [crate].
#[proc_macro]
pub fn shader(input: TokenStream) -> TokenStream {
    match parse(input).and_then(|literal| expand(&literal)) {
        Ok(tokens) => tokens,
        Err(error) => error.to_compile_error().into(),
    }
}

fn parse(input: TokenStream) -> syn::Result<LitStr> {
    let literal =
        syn::parse::<LitStr>(input).map_err(|error| syn::Error::new(error.span(), EXPECTED))?;
    if literal.suffix().is_empty() {
        Ok(literal)
    } else {
        Err(syn::Error::new(literal.span(), EXPECTED))
    }
}

fn expand(literal: &LitStr) -> syn::Result<TokenStream> {
    let path = resolve(literal)?;
    slango_codegen::generate(&path)
        .map(Into::into)
        .map_err(|error| syn::Error::new(literal.span(), error.report()))
}

/// Returns the absolute path that a path literal names, relative to the
/// directory of the Rust file that calls the macro.
fn resolve(literal: &LitStr) -> syn::Result<std::path::PathBuf> {
    let error = |text: String| syn::Error::new(literal.span(), text);
    let file = proc_macro::Span::call_site().local_file().ok_or_else(|| {
        error("cannot resolve the path of the Slang file, because the compiler gives no Rust file for the call of shader!".to_owned())
    })?;
    let directory = file.parent().map_or_default(std::path::PathBuf::from);
    std::path::absolute(directory.join(literal.value())).map_err(|cause| {
        error(format!(
            "cannot resolve the path of the Slang file against the working directory of the compiler\n  caused by: {cause}"
        ))
    })
}
