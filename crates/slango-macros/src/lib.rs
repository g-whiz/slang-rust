//! Compiles a Slang file while a crate builds and generates typed Rust
//! bindings to its compute kernels, for `slango`.
//!
//! [Slang](https://shader-slang.org) is a language for shaders, the programs
//! that run on a graphics processing unit (GPU). `slango` runs the compute
//! entry points of a Slang module, its kernels, and sets each parameter by
//! name. A misspelled name or a value of the wrong type therefore fails only
//! when the program runs.
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
//! public field of each kernel. A generated struct with padding keeps the
//! padding in private fields. A crate that enables the Clippy lint
//! `clippy::partial_pub_fields` therefore puts an `#[expect]` of that lint,
//! with a reason, on the module that calls the macro.
//!
//! A generated `new` with more than seven arguments carries an `#[expect]`
//! of `clippy::too_many_arguments`. A crate that raises the Clippy setting
//! `too-many-arguments-threshold` to that number of arguments or more gets
//! the warning `unfulfilled_lint_expectations` on that `new`.
//!
//! # Documentation without Slang
//!
//! When the environment variable `DOCS_RS` is present while this crate
//! builds, as on docs.rs, the macro reads no file and calls no Slang. It
//! generates only an empty, documented struct `Shader`, so the other
//! generated items are absent from the documentation of the calling crate.
//!
//! A crate that names another generated item outside the body of a
//! function, such as in a `use` declaration, a function signature, or a
//! struct field, then fails to build its documentation. A crate that names
//! those items only inside function bodies, in expressions and in the type
//! annotations of `let`, builds it.
//!
//! # Limits
//!
//! The macro has these limits:
//!
//! - A C++ exception that escapes Slang during the expansion ends the
//!   compiler, with one line on standard error and no compile error.
//! - The compiled code that the macro embeds depends on the Slang of the
//!   machine that builds the crate. Slang built from source without its
//!   SPIR-V optimizer can produce code that `slango` rejects with
//!   `slango::ErrorKind::UnsupportedShader` when `Shader::load` runs.
//! - The generated code holds the absolute paths of the Slang file and of
//!   every file that Slang read for it, on the machine that builds the
//!   crate.
//! - A new file that would change how an `import` resolves does not make
//!   Cargo build the crate again.
//! - Each call runs Slang again on every compilation of the calling crate.
//! - A release build compiles `slango` and its dependencies twice, once for
//!   the program and once for the macro, which runs inside the compiler.
//! - The program loads the Slang library when it starts, through `slango`,
//!   although `Shader::load` makes no call into Slang.
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate supports the platforms of `slango`, which are Linux on x86-64,
//! Windows on x86-64, and macOS on Apple silicon and on x86-64.
//!
//! # Finding the Slang library
//!
//! The macro runs Slang inside the compiler, so building a crate that calls
//! it needs the pinned Slang, as building `slango` does. That is the Vulkan
//! SDK that `VULKAN_SDK` names, or the source tree that `SLANG_SOURCE_DIR`
//! names, as the crate `slang-compiler-sys` describes.
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
//!
//! With Slang built from source and linked statically, the macro library
//! needs no Slang shared library.

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
///   public field for each parameter, named after its path in snake case. A
///   plain value is a `bool`, `i32`, `u32`, or `f32`, or an array of them for
///   a vector, a matrix, or an array. A structured buffer is a
///   `&slango::Buffer` of `i32`, `u32`, `f32`, or an array of four of them.
///   A texture is a `&slango::Texture`, and a sampler a `&slango::Sampler`.
///   A structure among the values or the elements of a structured buffer has
///   the generated struct of that structure as its type.
/// - For each Slang structure that a kernel takes, a struct of the same name
///   with one public field for each field of the structure and a `const fn
///   new` that takes them all. Each structure is generated once, however
///   many kernels take it.
///
/// A structure that is the element of a structured buffer gets the layout
/// that Slang gives it there. Its struct is `#[repr(C)]`, holds private
/// padding wherever the layout has a gap, and asserts its size and the
/// offset of each field at compile time. It binds as the element of a
/// `slango::Buffer`, and code creates it with `new`, because the private
/// padding rules out a struct literal.
///
/// A structure that is only a value has no padding, because a dispatch sets
/// each of its fields by its path through the runtime API of `slango`, so
/// every check of `slango` still runs.
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

#[cfg(not(slango_docs_rs))]
fn expand(literal: &LitStr) -> syn::Result<TokenStream> {
    let path = resolve(literal)?;
    slango_codegen::generate(&path)
        .map(Into::into)
        .map_err(|error| syn::Error::new(literal.span(), error.report()))
}

#[cfg(slango_docs_rs)]
fn expand(_literal: &LitStr) -> syn::Result<TokenStream> {
    use proc_macro::{Delimiter, Group, Ident, Literal, Punct, Spacing, Span, TokenTree};
    const DOCUMENTATION: &str = " The bindings that `slango_macros::shader!` generates, which exist only in a build with the Slang library, not in this documentation.";
    let ident = |name| TokenTree::Ident(Ident::new(name, Span::call_site()));
    let attribute = [
        ident("doc"),
        TokenTree::Punct(Punct::new('=', Spacing::Alone)),
        TokenTree::Literal(Literal::string(DOCUMENTATION)),
    ];
    Ok([
        TokenTree::Punct(Punct::new('#', Spacing::Alone)),
        TokenTree::Group(Group::new(
            Delimiter::Bracket,
            attribute.into_iter().collect(),
        )),
        ident("pub"),
        ident("struct"),
        ident("Shader"),
        TokenTree::Punct(Punct::new(';', Spacing::Alone)),
    ]
    .into_iter()
    .collect())
}

/// Returns the absolute path that a path literal names, relative to the
/// directory of the Rust file that calls the macro.
#[cfg(not(slango_docs_rs))]
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
