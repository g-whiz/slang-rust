//! Generates typed Rust bindings to the compute kernels of a Slang file, for
//! `slango`.
//!
//! [Slang](https://shader-slang.org) is a language for shaders, the programs
//! that run on a graphics processing unit (GPU). This crate is the generator
//! behind the macro `shader!` of `slango-macros`, which is the crate to use in
//! a program. It compiles a Slang file with `slango`, as
//! `slango::Device::load_module` does, and returns the Rust code of the
//! bindings: a struct `Shader`, which loads the compiled module, and for each
//! compute entry point, or kernel, a struct that runs it with an arguments
//! struct that has one typed field for each parameter, and for each Slang
//! structure that a kernel takes, a struct of the same name. The generated
//! code
//! embeds the compiled module and calls `slango` at run time, so a crate
//! that contains it depends on `slango` of exactly the version that this
//! crate requires.
//!
//! [`generate`] compiles a file and generates its bindings, and
//! [`generate_precompiled`] generates the bindings of a module that is
//! already compiled. Every failure is an [`Error`], whose [`Error::kind`]
//! says what went wrong.
//!
//! # Examples
//!
//! This example generates the bindings of a Slang file and prints them. It
//! needs the Slang library at run time, so it is compiled but not run:
//!
//! ```no_run
//! let tokens = slango_codegen::generate("/tmp/shaders/saxpy.slang")?;
//! println!("{tokens}");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate runs wherever `slango` builds, on 64-bit Linux, Windows, and
//! macOS.
//!
//! # The Slang library
//!
//! The crate compiles Slang through `slango`, which links the Slang library
//! as the crate `slang-compiler-sys` describes.

mod emit;
mod error;
mod names;

use std::path::Path;

use proc_macro2::TokenStream;
use slango::__private::Precompiled;

pub use error::{Error, ErrorKind, Result};

const COMPILE: &str = "generate the bindings of the Slang file";

/// Compiles the Slang file at `path` and returns the Rust code of its
/// bindings.
///
/// Use this to generate the bindings of a file, as the macro `shader!` of
/// `slango-macros` does. The code holds an item for each file that Slang
/// read, which makes Cargo build the crate that contains the code again when
/// one of them changes, and it names each such file by the absolute path
/// that Slang gives it. So pass an absolute path.
///
/// # Examples
///
/// See the example of the [crate].
///
/// # Errors
///
/// Returns an [`Error`] of these kinds:
///
/// - [`ErrorKind::Compile`] when `slango` cannot read or compile the file.
/// - [`ErrorKind::Unsupported`] when a kernel has a parameter for which the
///   generator has no Rust type.
/// - [`ErrorKind::NameConflict`] when two names of the module give the same
///   Rust name, or a name gives a Rust name that cannot be used.
pub fn generate(path: impl AsRef<Path>) -> Result<TokenStream> {
    generate_file(path.as_ref())
}

fn generate_file(path: &Path) -> Result<TokenStream> {
    let precompiled = slango::__private::precompile_file(path).map_err(|error| {
        let kind = match error.kind() {
            slango::ErrorKind::UnsupportedParameter => ErrorKind::Unsupported,
            _ => ErrorKind::Compile,
        };
        Error::new(kind, COMPILE).with_source(error)
    })?;
    generate_precompiled(&precompiled)
}

/// Returns the Rust code of the bindings of a module that
/// `slango::__private::precompile_file` compiled.
///
/// Use this when the module is already compiled. The function calls no
/// Slang, so it is what the tests of the generator call with a description
/// that they build by hand.
///
/// # Examples
///
/// This example generates the bindings of a module without kernels, which
/// hold only the struct `Shader`:
///
/// ```
/// use slango::__private::{Interface, Precompiled};
///
/// let precompiled = Precompiled {
///     interface: Interface { kernels: Vec::new(), dependencies: Vec::new() },
///     encoded: Vec::new(),
/// };
/// let tokens = slango_codegen::generate_precompiled(&precompiled)?;
/// assert!(tokens.to_string().contains("pub struct Shader"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an [`Error`] of these kinds:
///
/// - [`ErrorKind::Unsupported`] when a kernel has a parameter for which the
///   generator has no Rust type, or the path of a file that Slang read is
///   not valid UTF-8.
/// - [`ErrorKind::NameConflict`] when two names of the module give the same
///   Rust name, or a name gives a Rust name that cannot be used.
pub fn generate_precompiled(precompiled: &Precompiled) -> Result<TokenStream> {
    emit::bindings(precompiled)
}
