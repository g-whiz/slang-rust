//! Compiles a Slang file while a crate builds and generates typed Rust
//! bindings to its compute kernels, for `slango`.
//!
//! [Slang](https://shader-slang.org) is a language for shaders, the programs
//! that run on a graphics processing unit (GPU). `slango` runs the compute
//! entry points of a Slang module, its kernels, and sets each of their
//! parameters by name, so a misspelled name or a value of the wrong type
//! fails only when the program runs. The macro [`shader!`] of this crate
//! turns these mistakes into compile errors: it compiles the Slang file with
//! the Slang library while the crate that calls it builds, and generates a
//! struct `Shader` with a typed kernel for each compute entry point, whose
//! arguments struct has one field of the matching Rust type for each
//! parameter. The generated code calls `slango`, so a crate that uses the
//! macro also depends on `slango`. The generator itself is the crate
//! `slango-codegen`.
//!
//! # Examples
//!
//! This example calls the macro for a Slang file beside the Rust file and
//! runs its kernel `saxpy`, which computes `result = a * x + y`. The macro
//! resolves the path against the Rust file that calls it, which a doctest
//! does not have, so the example is not a doctest:
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
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The macro runs on 64-bit Linux, Windows, and macOS, wherever `slango`
//! builds.
//!
//! # The Slang library
//!
//! The macro calls the Slang library inside the compiler, so building a
//! crate that uses it needs the pinned Slang, as the crate
//! `slang-compiler-sys` describes. On Linux and macOS, the build script of
//! this crate stores the library directory of the Vulkan SDK that
//! `VULKAN_SDK` names in the macro library, so the compiler finds the Slang
//! library without a library path. On Windows, the directory of
//! `slang-compiler.dll` must be on `PATH`. When `DOCS_RS` is set, the macro
//! generates only an empty struct `Shader` and calls no Slang.

use proc_macro::TokenStream;
use syn::LitStr;

const EXPECTED: &str = "shader! takes one string literal, the path of a Slang file";

/// Compiles the Slang file at a path and generates typed bindings to its
/// compute kernels where the macro is called.
///
/// Call the macro once in a module of its own, with one string literal, the
/// path of the `.slang` file, relative to the directory of the Rust file
/// that calls the macro, as for [`include_str!`]. It generates these items
/// in the module:
///
/// - `Shader`, with `Shader::load`, which loads the module on a
///   `slango::Device` from the compiled code that the crate embeds, and
///   `Shader::module`, which returns the `slango::Module`.
/// - For each compute entry point, a kernel struct whose name is the name of
///   the entry point in upper camel case, such as `Saxpy` for `saxpy`, and
///   a public field of `Shader` for it whose name is the name in snake case.
///   The kernel struct has `THREAD_GROUP_SIZE`, `dispatch`, which runs the
///   kernel on a number of threads, `dispatch_groups`, which runs it on a
///   number of thread groups, and `kernel`, which returns the
///   `slango::Kernel`.
/// - For each kernel, an arguments struct, such as `SaxpyArgs`, with one
///   public field for each parameter, named after its path in snake case: a
///   `bool`, `i32`, `u32`, or `f32`, an array of them for a vector, a matrix,
///   or an array, a `&slango::Buffer` of `i32`, `u32`, `f32`, or an array of
///   four of them for a structured buffer, a `&slango::Texture`, or a
///   `&slango::Sampler`. A structure among the values or the elements of a
///   structured buffer has the generated struct of that structure as its
///   type.
/// - For each Slang structure that a kernel takes, a struct of the same name
///   with one public field for each field of the structure and a `const fn
///   new` that takes them all. A structure that is the element of a
///   structured buffer gets the layout that Slang gives it there: the struct
///   is `#[repr(C)]`, holds private padding wherever the layout has a gap,
///   asserts its size and the offset of each field at compile time, and
///   binds as the element of a `slango::Buffer`. A structure that is only a
///   value has no padding, because a dispatch sets each of its fields by its
///   path. Each structure is generated once, however many kernels take it.
///
/// A dispatch sets each parameter through the runtime API of `slango` by
/// its path, so every check of `slango` still runs. The crate builds again
/// whenever the Slang file or a file that it imports changes.
///
/// The macro reports each failure as a compile error at the path: a Slang
/// file that cannot be read or compiled, with the messages of Slang, a
/// parameter for which the macro has no Rust type, such as a structured
/// buffer of `float3` or a 64-bit scalar, a field of a structure in a
/// structured buffer that a Rust struct cannot lay out as Slang does, such as
/// a `bool`, a matrix, or a `float3[2]`, and two Slang names that give the
/// same Rust name, such as two different structures of one name.
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

#[cfg(not(slango_docs_rs))]
fn expand(literal: &LitStr) -> syn::Result<TokenStream> {
    let path = resolve(literal)?;
    slango_codegen::generate(&path)
        .map(Into::into)
        .map_err(|error| syn::Error::new(literal.span(), report(&error)))
}

#[cfg(slango_docs_rs)]
fn expand(literal: &LitStr) -> syn::Result<TokenStream> {
    const STUB: &str = "/// The bindings that `slango_macros::shader!` generates, which exist\n/// only in a build with the Slang library, not in this documentation.\npub struct Shader;";
    STUB.parse().map_err(|error| {
        syn::Error::new(
            literal.span(),
            format!("cannot emit the stub of the bindings: {error}"),
        )
    })
}

/// Returns the absolute path that a path literal names, relative to the
/// directory of the Rust file that calls the macro (SLM-10 of
/// `docs/design/slango-macros.md`).
#[cfg(not(slango_docs_rs))]
fn resolve(literal: &LitStr) -> syn::Result<std::path::PathBuf> {
    let error = |text: String| syn::Error::new(literal.span(), text);
    let file = proc_macro::Span::call_site().local_file().ok_or_else(|| {
        error("cannot resolve the path of the Slang file, because the compiler gives no Rust file for the call of shader!".to_owned())
    })?;
    let directory = file
        .parent()
        .map(std::path::PathBuf::from)
        .unwrap_or_default();
    std::path::absolute(directory.join(literal.value())).map_err(|cause| {
        error(format!(
            "cannot resolve the path of the Slang file against the working directory of the compiler\n  caused by: {cause}"
        ))
    })
}

/// Returns the text of an error followed by the text of each of its causes.
#[cfg(not(slango_docs_rs))]
fn report(error: &slango_codegen::Error) -> String {
    let mut text = error.to_string();
    let mut cause = std::error::Error::source(error);
    while let Some(error) = cause {
        text.push_str("\n  caused by: ");
        text.push_str(&error.to_string());
        cause = error.source();
    }
    text
}
