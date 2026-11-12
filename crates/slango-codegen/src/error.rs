use std::error::Error as StdError;
use std::fmt;

/// The result type of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// The error of every fallible operation of this crate.
///
/// Every operation that can fail returns this one type. Match on
/// [`Error::kind`] to decide what to do. When `slango` caused the failure,
/// such as when Slang cannot compile the module, [`source`](StdError::source)
/// returns its error, whose own sources hold the messages of Slang.
///
/// The text that [`Display`](fmt::Display) prints is a lowercase phrase that
/// says what failed, followed by a colon and a detail when there is one. It
/// does not repeat the text of the source error. The text is meant for
/// people and can change between versions, so code matches on the kind and
/// not on the text.
///
/// # Examples
///
/// See the example of [`Error::report`].
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    operation: &'static str,
    detail: Option<String>,
    source: Option<slango::Error>,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, operation: &'static str) -> Self {
        Self {
            kind,
            operation,
            detail: None,
            source: None,
        }
    }

    pub(crate) fn with_detail(mut self, detail: String) -> Self {
        self.detail = Some(detail);
        self
    }

    pub(crate) fn with_source(mut self, source: slango::Error) -> Self {
        self.source = Some(source);
        self
    }

    /// Returns the kind of the failure, which says what went wrong and how a
    /// caller can recover.
    ///
    /// # Examples
    ///
    /// This example tells a compile error of Slang apart from a kernel that
    /// the generator does not support:
    ///
    /// ```no_run
    /// use slango_codegen::ErrorKind;
    ///
    /// match slango_codegen::generate("/tmp/shaders/saxpy.slang") {
    ///     Ok(tokens) => println!("{tokens}"),
    ///     Err(error) if error.kind() == ErrorKind::Compile => eprintln!("fix the shader: {error}"),
    ///     Err(error) => eprintln!("use the runtime API of slango instead: {error}"),
    /// }
    /// ```
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the text of the error followed by the text of each of its
    /// causes, each on a line of its own that starts with `  caused by: `,
    /// which is the text that the macro of `slango-macros` reports.
    ///
    /// # Examples
    ///
    /// This example prints an error with each of its causes:
    ///
    /// ```no_run
    /// if let Err(error) = slango_codegen::generate("/tmp/shaders/saxpy.slang") {
    ///     eprintln!("{}", error.report());
    /// }
    /// ```
    #[must_use]
    pub fn report(&self) -> String {
        let mut text = self.to_string();
        let mut cause = self.source();
        while let Some(error) = cause {
            text.push_str("\n  caused by: ");
            text.push_str(&error.to_string());
            cause = error.source();
        }
        text
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot {}", self.operation)?;
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        Ok(())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        let source: &(dyn StdError + 'static) = self.source.as_ref()?;
        Some(source)
    }
}

/// The kind of an [`Error`], which says what went wrong and how a caller can
/// recover.
///
/// The enumeration is non-exhaustive, so a later version can add kinds, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Error::kind`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// `slango` cannot compile the Slang file, such as when the file cannot
    /// be read or its source has an error. The source error is the error of
    /// `slango`, with the messages of Slang. Fix the shader or the path.
    Compile,
    /// A kernel has a parameter for which the generator has no Rust type,
    /// such as a structured buffer of `float3`, a 64-bit scalar, or a
    /// structure in a structured buffer with a `bool` field. The detail names
    /// the kernel, the parameter, the field when there is one, and the
    /// reason. Change the shader, or use the runtime API of `slango` for the
    /// kernel.
    Unsupported,
    /// Two names of the Slang module give the same Rust name, or a name gives
    /// a Rust name that cannot be used, such as `self`. The detail names the
    /// Slang names. Rename one of them in the shader.
    NameConflict,
}

#[cfg(test)]
mod tests {
    use super::{Error, ErrorKind};
    use std::error::Error as StdError;

    #[test]
    fn display_names_the_operation_and_the_detail() {
        let error = Error::new(ErrorKind::Unsupported, "generate the kernel `saxpy`")
            .with_detail("the parameter `x` is a byte address buffer".to_owned());
        assert_eq!(
            error.to_string(),
            "cannot generate the kernel `saxpy`: the parameter `x` is a byte address buffer"
        );
        assert!(error.source().is_none());
    }

    #[test]
    fn the_report_of_an_error_without_a_source_is_its_text() {
        let error = Error::new(ErrorKind::Compile, "compile the Slang module");
        assert_eq!(error.report(), "cannot compile the Slang module");
    }

    #[test]
    fn display_without_a_detail_is_the_operation_alone() {
        let error = Error::new(ErrorKind::Compile, "compile the Slang module");
        assert_eq!(error.to_string(), "cannot compile the Slang module");
    }

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn error_types_are_send_and_sync() {
        assert_send_sync::<Error>();
        assert_send_sync::<ErrorKind>();
    }
}
