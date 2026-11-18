//! Generates the raw Rust bindings of `slang-compiler-sys` from the Slang
//! headers, and those of `slang-rhi-sys` from the headers of slang-rhi.
//!
//! Slang is a shader compiler, a compiler for the programs that run on a
//! graphics processing unit (GPU), and its C++ header `slang.h` declares its
//! API. This crate parses that header with libclang, the C and C++ parser
//! library of the LLVM project, and returns the Rust declarations of that
//! API as text, so that no vtable, UUID, or thunk is written by hand. A
//! vtable is the table of function pointers through which C++ calls a
//! virtual method, a UUID is the 128-bit identifier of an interface, and a
//! thunk is the Rust function that calls one method through the vtable.
//!
//! The build script of `slang-compiler-sys` calls [`generate`] in every
//! build, on the headers of the Slang that the build links, and includes the
//! two texts from its output directory. The build script of `slang-rhi-sys`
//! calls [`generate_slang_rhi`] in the same way on the headers of slang-rhi,
//! the render hardware interface of the Slang source tree, whose bindings
//! refer to the Slang types of `slang-compiler-sys`. Use this crate directly
//! only to see the bindings that a build of either crate compiles, such as to
//! compare the bindings of two Slang versions. Programs that use Slang depend
//! on `slang-compiler-sys`, or on the safe crate `slang-compiler`, instead.
//!
//! The generator parses the headers once, for the target that its caller
//! names, and decides which declarations to include through a rule set that
//! is part of this crate. It fails on every declaration and macro that no
//! rule decides, on every rule that decides nothing, and on every construct
//! that the generated Rust cannot express, so the headers of a Slang other
//! than the one that the rule set was written for can make it fail. The
//! generated text holds no comment and no inner attribute, so that a crate
//! can include it with [`include!`], and it is not formatted.
//!
//! # Examples
//!
//! This example generates the bindings from an installed Vulkan SDK for
//! `x86_64` Linux. It needs libclang and the Slang headers at run time, so it
//! is compiled but not run:
//!
//! ```no_run
//! let generated =
//!     slang_bindgen::generate("/tmp/vulkan-sdk/include/slang", "x86_64-unknown-linux-gnu")?;
//! assert!(generated.bindings.contains("pub unsafe trait Interface"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! The package also has a binary, `slang-bindgen`, which takes a header
//! directory, a target triple, and an output directory, and writes the two
//! texts there as `bindings.rs` and `symbols.rs`.
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The generator runs on every platform that the crate `clang-sys`
//! supports. It loads libclang when it runs, not when it is compiled, so the
//! crate builds without libclang, and a missing libclang is an error of kind
//! [`ErrorKind::Libclang`]. It finds libclang in the directory that the
//! environment variable `LIBCLANG_PATH` names, or otherwise in the locations
//! that `clang-sys` searches on each platform. Only one libclang instance can
//! exist in a process, so a process calls [`generate`] from one thread at a
//! time. A later call, from the same thread or from another one, works once
//! the earlier call has returned.
//!
//! The first call that loads libclang keeps it loaded until the process
//! exits. On Windows, a process that unloaded `libclang.dll` after using it
//! can crash when it exits, so the generator never unloads it.
//!
//! Every parse uses the shim headers of this crate in place of the system
//! headers, so the text depends only on the Slang headers and the target,
//! not on the machine that runs the generator. The generator keeps no list of
//! targets. It refuses what it cannot generate correctly for the target of
//! the parse, and the generated layout assertions compare every struct with
//! the layout that libclang reports for that target.

mod check;
mod emit;
mod error;
mod model;
mod parse;
mod probe;
mod rules;

use std::path::Path;
use std::sync::{Arc, OnceLock};

use clang::{Clang, Index};

pub use error::{Error, ErrorKind, Location, Result};

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;

const CLANG_EXISTS: &str = "an instance of `Clang` already exists";

/// The texts that one call of [`generate`] or [`generate_slang_rhi`] returns.
///
/// The texts come from the same parse of the headers, so the list of C
/// functions and the ABI probe always match the bindings.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Generated {
    /// The raw bindings, which start with a `compile_error!` for every
    /// target whose pointers are not 64 bits wide.
    pub bindings: String,
    /// The constant `C_FUNCTIONS`, which pairs the name of every C function
    /// of the bindings with its address, in declaration order, for a module
    /// that includes it next to the crate of the bindings, which is
    /// `slang_compiler_sys` for [`generate`] and `slang_rhi_sys` for
    /// [`generate_slang_rhi`].
    pub symbols: String,
    /// The C++ translation unit that checks the layout of the bindings
    /// against the compiler that builds slang-rhi, which only
    /// [`generate_slang_rhi`] returns. [`generate`] returns `None`.
    pub abi_probe: Option<String>,
}

/// Generates the raw bindings and the list of their C functions from the
/// Slang headers in `header_dir` for the target `target`.
///
/// Use this in a build script to get the bindings of the Slang that the
/// build links. `header_dir` is the directory that holds `slang.h`, which is
/// `include/slang` in a Vulkan SDK, and `target` is a target triple that
/// libclang knows, such as `x86_64-pc-windows-msvc`. The function loads
/// libclang, which stays loaded until the process exits, parses the headers
/// once for the target, and returns both texts without formatting them. It
/// writes no file and starts no process.
///
/// # Examples
///
/// This example writes the bindings into a file. It needs libclang and the
/// Slang headers at run time, so it is compiled but not run:
///
/// ```no_run
/// let generated =
///     slang_bindgen::generate("/tmp/vulkan-sdk/include/slang", "aarch64-apple-darwin")?;
/// std::fs::write("/tmp/bindings.rs", generated.bindings)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an [`Error`] whose [`ErrorKind`] says what failed:
///
/// - [`ErrorKind::Io`] when the header directory, `slang.h` in it, or the
///   directory of the shim headers cannot be read.
/// - [`ErrorKind::Libclang`] when libclang cannot be loaded, or when this
///   process already holds a libclang instance, such as during a call of the
///   generator on another thread. Only one instance may exist at a time, so
///   call the generator from one thread at a time: a later call works once
///   the earlier one has returned.
/// - [`ErrorKind::Target`] when libclang refuses to parse for `target`,
///   which it does for a target triple that it does not know.
/// - [`ErrorKind::Diagnostic`] when the parse reports an error.
/// - [`ErrorKind::UnmatchedDeclaration`] when a declaration or a macro of the
///   headers matches no rule of the rule set.
/// - [`ErrorKind::StaleRule`] when a rule of the rule set decides no
///   declaration or macro of the headers.
/// - [`ErrorKind::Unsupported`] when an included declaration uses a construct
///   that the bindings cannot express, such as an aggregate passed by value,
///   a variadic function, an interface with several bases, or a method that
///   is overloaded, not pure virtual, or uses another calling convention.
/// - [`ErrorKind::Evaluation`] when a macro or a default member initializer
///   cannot be evaluated or does not fit its type.
/// - [`ErrorKind::NameCollision`] when two generated items would have the
///   same Rust name.
pub fn generate(header_dir: impl AsRef<Path>, target: &str) -> Result<Generated> {
    generate_with(&rules::SLANG, header_dir.as_ref(), target)
}

/// Generates the raw bindings of slang-rhi, the list of their C functions,
/// and the ABI probe from the slang-rhi headers for the target `target`.
///
/// Use this in a build script that builds slang-rhi, to get the bindings of
/// the slang-rhi that the build links. `rhi_header_dir` is the `include`
/// directory of slang-rhi, which holds `slang-rhi.h`. `config_dir` is the
/// directory that holds the `slang-rhi-config.h` that the configure step of
/// slang-rhi wrote, so that the parse sees the configuration that the library
/// was built with. `slang_header_dir` holds the headers of the Slang that the
/// same build links, the directory that [`generate`] takes. `target` is a
/// target triple that libclang knows.
///
/// The bindings refer to every declaration of the Slang headers by its path
/// in the crate `slang_compiler_sys`, whose bindings [`generate`] writes for
/// the same headers and target, so that one native type has one Rust type.
/// The interfaces of slang-rhi implement the trait `Interface` of that crate.
/// The C functions are listed for a module that includes the list next to
/// the crate `slang_rhi_sys`.
///
/// [`Generated::abi_probe`] holds a C++ translation unit for the compiler
/// that builds slang-rhi. It asserts the size, the alignment, and the offset
/// of each field of every generated struct. On a target of the Itanium C++
/// ABI, which is every target that libclang does not resolve to Windows with
/// the `msvc` environment, it also defines the C function `slang_rhi_sys_abi_probe_slots`, which writes the
/// byte offset of the vtable slot of every generated method into an array of
/// `size_t` of the given capacity and returns their number.
/// [`Generated::symbols`] then also holds the declaration of that function and
/// the constant `VTABLE_SLOTS`, which pairs each method with the offset of
/// its field in the generated vtable struct, in the same order.
///
/// # Examples
///
/// This example generates the bindings of slang-rhi for `x86_64` Linux. It
/// needs libclang and the headers at run time, so it is compiled but not run:
///
/// ```no_run
/// let generated = slang_bindgen::generate_slang_rhi(
///     "/tmp/slang/external/slang-rhi/include",
///     "/tmp/slang-rhi-build/include",
///     "/tmp/slang/include",
///     "x86_64-unknown-linux-gnu",
/// )?;
/// assert!(generated.abi_probe.is_some());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an [`Error`] with the kinds that [`generate`] returns, for the
/// slang-rhi headers and for the Slang headers, which this function also
/// parses as [`generate`] does. These are added:
///
/// - [`ErrorKind::Io`] when one of the three directories, or `slang-rhi.h`
///   in the first, cannot be read.
/// - [`ErrorKind::Unsupported`] when a declaration of slang-rhi refers to a
///   Slang type that the bindings of [`generate`] do not declare in the form
///   that the reference needs, and when virtual methods share a name on a
///   target that libclang resolves to Windows with the `msvc` environment,
///   such as `x86_64-pc-windows-msvc` or `x86_64-pc-win32`, whose Microsoft
///   C++ ABI orders such methods in a vtable in a way that is not
///   established.
pub fn generate_slang_rhi(
    rhi_header_dir: impl AsRef<Path>,
    config_dir: impl AsRef<Path>,
    slang_header_dir: impl AsRef<Path>,
    target: &str,
) -> Result<Generated> {
    generate_with_external(
        &rules::SLANG,
        &rules::SLANG_RHI,
        [
            rhi_header_dir.as_ref(),
            config_dir.as_ref(),
            slang_header_dir.as_ref(),
        ],
        target,
    )
}

fn generate_with_external(
    external_rules: &rules::RuleSet,
    rules: &rules::RuleSet,
    [header_dir, config_dir, external_dir]: [&Path; 3],
    target: &str,
) -> Result<Generated> {
    readable_dir(config_dir, "the directory of the configured header")?;
    main_header(external_dir, external_rules)?;
    let clang = start(header_dir, rules)?;
    let index = Index::new(&clang, false, false);
    let external = run(&index, external_rules, &[external_dir], target, None)?;
    let emitted = run(
        &index,
        rules,
        &[header_dir, config_dir, external_dir],
        target,
        Some(&external.types),
    )?;
    Ok(Generated {
        bindings: emitted.bindings,
        symbols: emitted.symbols,
        abi_probe: emitted.abi_probe,
    })
}

fn generate_with(rules: &rules::RuleSet, header_dir: &Path, target: &str) -> Result<Generated> {
    let clang = start(header_dir, rules)?;
    let index = Index::new(&clang, false, false);
    let emitted = run(&index, rules, &[header_dir], target, None)?;
    Ok(Generated {
        bindings: emitted.bindings,
        symbols: emitted.symbols,
        abi_probe: emitted.abi_probe,
    })
}

fn main_header(header_dir: &Path, rules: &rules::RuleSet) -> Result<()> {
    readable_dir(header_dir, "the header directory")?;
    let main_header = header_dir.join(rules.main_header);
    std::fs::File::open(&main_header)
        .map(drop)
        .map_err(|error| {
            Error::new(
                ErrorKind::Io,
                format!("cannot read the main header {}", main_header.display()),
            )
            .with_source(error)
        })
}

fn start(header_dir: &Path, rules: &rules::RuleSet) -> Result<Clang> {
    main_header(header_dir, rules)?;
    readable_dir(&parse::shim_dir(), "the directory of the shim headers")?;
    keep_libclang_loaded()?;
    Clang::new().map_err(|reason| libclang_error(&reason))
}

fn run(
    index: &Index<'_>,
    rules: &rules::RuleSet,
    dirs: &[&Path],
    target: &str,
    external: Option<&emit::TypeNames>,
) -> Result<emit::Emitted> {
    let source = parse::Source {
        dirs,
        initializers_in_place: rules.initializers_in_place,
        main_header: rules.main_header,
        language: rules.language,
        defines: rules.defines,
    };
    let model = parse::parse(index, target, &source)?;
    let mut selection = check::select(model, rules, target)?;
    check::evaluate(&mut selection, rules, |evaluations| {
        parse::evaluate(index, target, &source, evaluations)
    })?;
    check::validate(&selection, rules, target, external)?;
    emit::emit(&selection, rules, target)
}

fn readable_dir(dir: &Path, what: &str) -> Result<()> {
    std::fs::read_dir(dir).map(drop).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read {what} {}", dir.display()),
        )
        .with_source(error)
    })
}

fn keep_libclang_loaded() -> Result<()> {
    static LIBCLANG: OnceLock<Arc<clang_sys::SharedLibrary>> = OnceLock::new();
    if LIBCLANG.get().is_some() {
        return Ok(());
    }
    let previous = clang_sys::set_library(None);
    let loaded = clang_sys::load().map_err(|reason| libclang_error(&reason));
    let library = clang_sys::set_library(previous);
    loaded?;
    if let Some(library) = library {
        LIBCLANG.get_or_init(|| library);
    }
    Ok(())
}

fn libclang_error(reason: &str) -> Error {
    if reason == CLANG_EXISTS {
        return Error::new(
            ErrorKind::Libclang,
            "cannot use libclang, because this process already holds a libclang instance",
        );
    }
    Error::new(
        ErrorKind::Libclang,
        format!("cannot load libclang: {reason}"),
    )
}

#[cfg(test)]
mod tests;
