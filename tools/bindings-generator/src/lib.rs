//! Generates the raw Rust bindings of `slang-compiler-sys` from the Slang
//! headers.
//!
//! Slang is a shader compiler, a compiler for the programs that run on a
//! graphics processing unit (GPU), and its C++ header `slang.h` declares its
//! API. This crate is the binding generator of the repository: it parses that
//! header with libclang, the C and C++ parser library of the LLVM project, and
//! writes the Rust declarations that `slang-compiler-sys` checks in, so that
//! no vtable, UUID, or thunk is written by hand. It is a tool for the
//! maintainers of the repository and is not published. Users of Slang from
//! Rust depend on `slang-compiler-sys` or a crate built on it instead.
//!
//! The generator parses the headers once for each supported 64-bit target,
//! decides which declarations to include through a checked-in rule set,
//! compares the parses, and fails on any declaration, construct, or
//! difference between targets that it has no rule for. The generated text is
//! formatted with rustfmt and is the same for the same headers on every run.
//!
//! # Examples
//!
//! This example generates the bindings from an installed Vulkan SDK. It needs
//! libclang, rustfmt, and the Slang headers at run time, so it is compiled but
//! not run:
//!
//! ```no_run
//! let text = bindings_generator::generate("/tmp/vulkan-sdk/include/slang")?;
//! assert!(text.contains("pub unsafe trait Interface"));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The generator runs on Linux, where it loads libclang at run time and finds
//! it in the locations that the crate `clang-sys` searches, or in the
//! directory that the environment variable `LIBCLANG_PATH` names. It also
//! needs `rustfmt` on the `PATH`. The crate builds on every platform without
//! libclang.

mod check;
mod emit;
mod error;
mod model;
mod parse;
mod rules;

use std::path::Path;

use clang::{Clang, Index};

pub use error::{Error, ErrorKind, Location, Result};

/// The pin of `slang-compiler-sys`: the Vulkan SDK version, the Slang build
/// tag, and the Slang commit that the bindings match.
///
/// The generator reads these constants from the file `pin.rs` of
/// `slang-compiler-sys`, which is their only record, and checks the build tag
/// of the headers against [`pin::SLANG_BUILD_TAG`].
pub mod pin {
    include!("../../../crates/slang-compiler-sys/pin.rs");
}

const TAG_HEADER: &str = "slang-tag-version.h";
const TAG_MACRO: &str = "SLANG_TAG_VERSION";
const CLANG_EXISTS: &str = "an instance of `Clang` already exists";

/// Generates the text of the raw bindings from the Slang headers in
/// `header_dir`.
///
/// Use this to write `crates/slang-compiler-sys/src/bindings.rs`, or to
/// compare that file with a fresh generation. `header_dir` is the directory
/// that holds `slang.h` and `slang-tag-version.h`, which is `include/slang` in
/// a Vulkan SDK. The function loads libclang, parses the headers for every
/// supported target, and returns the bindings formatted with rustfmt.
///
/// # Examples
///
/// This example writes the bindings into a file. It needs libclang, rustfmt,
/// and the Slang headers at run time, so it is compiled but not run:
///
/// ```no_run
/// let text = bindings_generator::generate("/tmp/vulkan-sdk/include/slang")?;
/// std::fs::write("/tmp/bindings.rs", text)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an [`Error`] whose [`ErrorKind`] says what failed:
///
/// - [`ErrorKind::Io`] when the header directory or a header cannot be read.
/// - [`ErrorKind::Version`] when `slang-tag-version.h` does not define the
///   build tag of [`pin::SLANG_BUILD_TAG`].
/// - [`ErrorKind::Libclang`] when libclang cannot be loaded or cannot parse.
/// - [`ErrorKind::Diagnostic`] when a parse reports an error.
/// - [`ErrorKind::UnmatchedDeclaration`] when a declaration or a macro of the
///   headers matches no rule of the rule set.
/// - [`ErrorKind::StaleRule`] when a rule of the rule set decides no
///   declaration or macro of the headers.
/// - [`ErrorKind::Unsupported`] when an included declaration uses a construct
///   that the bindings cannot express, such as an aggregate passed by value,
///   a variadic function, an interface with several bases, or a method that
///   is overloaded, not pure virtual, or uses another calling convention.
/// - [`ErrorKind::TargetDifference`] when the parses for two targets differ
///   in a way that no rule accepts.
/// - [`ErrorKind::Evaluation`] when a macro or a default member initializer
///   cannot be evaluated or does not fit its type.
/// - [`ErrorKind::NameCollision`] when two generated items would have the
///   same Rust name.
/// - [`ErrorKind::Rustfmt`] when rustfmt cannot format the text.
pub fn generate(header_dir: impl AsRef<Path>) -> Result<String> {
    generate_with(&rules::SLANG, header_dir.as_ref(), pin::SLANG_BUILD_TAG)
}

fn generate_with(rules: &rules::RuleSet, header_dir: &Path, expected_tag: &str) -> Result<String> {
    let header_dir = std::fs::canonicalize(header_dir).map_err(|error| {
        Error::new(
            ErrorKind::Io,
            format!("cannot read the header directory {}", header_dir.display()),
        )
        .with_source(error)
    })?;
    check_tag(&header_dir, expected_tag)?;
    let clang = Clang::new().map_err(|reason| libclang_error(&reason))?;
    let index = Index::new(&clang, false, false);
    let mut selections = Vec::new();
    for target in parse::TARGETS {
        let model = parse::parse(&index, target, &header_dir, rules.main_header)?;
        let mut selection = check::select(model, rules)?;
        parse::evaluate(
            &index,
            target,
            &header_dir,
            rules.main_header,
            &mut selection,
        )?;
        check::validate(&selection, rules, target)?;
        selections.push((target, selection));
    }
    let selection = check::compare(&selections)?;
    emit::rustfmt(&emit::emit(&selection, rules)?)
}

fn libclang_error(reason: &str) -> Error {
    if reason == CLANG_EXISTS {
        return Error::new(
            ErrorKind::Libclang,
            "cannot use libclang, because this process already holds a libclang instance or failed to load libclang before, and only one instance may exist at a time, so generate the bindings once per process and from one thread",
        );
    }
    Error::new(
        ErrorKind::Libclang,
        format!(
            "cannot load libclang: {reason}. Install libclang or set LIBCLANG_PATH to the directory that holds it"
        ),
    )
}

fn check_tag(header_dir: &Path, expected_tag: &str) -> Result<()> {
    let path = header_dir.join(TAG_HEADER);
    let text = std::fs::read_to_string(&path).map_err(|error| {
        Error::new(
            ErrorKind::Version,
            format!(
                "cannot read {}, which records the build tag of the headers",
                path.display()
            ),
        )
        .with_source(error)
    })?;
    check_tag_text(&text, &path.display().to_string(), expected_tag)
}

fn check_tag_text(text: &str, path: &str, expected_tag: &str) -> Result<()> {
    let mut found = None;
    for (position, line) in text.lines().enumerate() {
        let mut words = line.split_whitespace();
        if words.next() != Some("#define") || words.next() != Some(TAG_MACRO) {
            continue;
        }
        let location = Location::new(TAG_HEADER, u32::try_from(position + 1).unwrap_or(u32::MAX));
        let value = words.next().and_then(|word| {
            word.strip_prefix('"')?
                .strip_suffix('"')
                .filter(|tag| !tag.contains('"'))
        });
        let Some(tag) = value.filter(|_| words.next().is_none()) else {
            return Err(Error::at(
                ErrorKind::Version,
                &location,
                format!("{TAG_MACRO} is not defined as a single string literal"),
            ));
        };
        found = Some((tag.to_owned(), location));
    }
    let Some((tag, location)) = found else {
        return Err(Error::new(
            ErrorKind::Version,
            format!("{path} does not define {TAG_MACRO}"),
        ));
    };
    if tag != expected_tag {
        return Err(Error::at(
            ErrorKind::Version,
            &location,
            format!(
                "the headers have the Slang build tag {tag}, but the pin of Vulkan SDK {} expects {expected_tag}, so install the pinned SDK or move the pin",
                pin::VULKAN_SDK_VERSION
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
