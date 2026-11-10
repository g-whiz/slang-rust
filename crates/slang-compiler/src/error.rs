use std::error::Error as StdError;
use std::fmt;

/// The result type of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// The error of every fallible operation of this crate.
///
/// Every operation that can fail returns this one type. Match on
/// [`Error::kind`] to decide what to do, and read [`Error::diagnostics`] for
/// the messages that Slang, the shader compiler, printed for the failed call.
/// [`Error::operation`] says which operation failed, and [`Error::code`] gives
/// the numeric result code that Slang returned, when it returned one.
///
/// The text that [`Display`](fmt::Display) prints has these parts, in this
/// order:
///
/// 1. A lowercase phrase that begins with `cannot` and names the failed
///    operation.
/// 2. The subject of the operation in backticks, when the operation has one.
/// 3. The Slang result code in hexadecimal in parentheses, when Slang
///    returned one.
/// 4. A colon and a detail, when there is one.
/// 5. A colon and the diagnostics on a new line, when there are any.
///
/// For example, an error from
/// [`GlobalSession::check_target_support`](crate::GlobalSession::check_target_support)
/// can print ``cannot produce code for target `Dxil` (Slang result code
/// 0x82000005)``. The text is meant for people and can change between
/// versions, so code matches on the kind and not on the text.
///
/// # Examples
///
/// This example handles a second global session on the same thread, which
/// the crate refuses. It needs the Slang shared library at run time, so it is
/// compiled but not run:
///
/// ```no_run
/// use slang_compiler::{ErrorKind, GlobalSession};
///
/// let global = GlobalSession::new()?;
/// match GlobalSession::new() {
///     Ok(_second) => println!("created a second global session"),
///     Err(error) if error.kind() == ErrorKind::AlreadyExists => {
///         eprintln!("reusing the first global session: {error}");
///     }
///     Err(error) => return Err(error.into()),
/// }
/// # drop(global);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    operation: Operation,
    code: Option<i32>,
    subject: Option<String>,
    detail: Option<String>,
    diagnostics: Diagnostics,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, operation: Operation) -> Self {
        Self {
            kind,
            operation,
            code: None,
            subject: None,
            detail: None,
            diagnostics: Diagnostics::default(),
            source: None,
        }
    }

    pub(crate) fn with_code(mut self, code: i32) -> Self {
        self.code = Some(code);
        self
    }

    pub(crate) fn with_subject(mut self, subject: String) -> Self {
        self.subject = Some(subject);
        self
    }

    pub(crate) fn with_detail(mut self, detail: String) -> Self {
        self.detail = Some(detail);
        self
    }

    pub(crate) fn with_diagnostics(mut self, diagnostics: Diagnostics) -> Self {
        self.diagnostics = diagnostics;
        self
    }

    /// Returns the kind of the failure, which says what went wrong and how a
    /// caller can recover.
    ///
    /// # Examples
    ///
    /// See the example of [`Error`].
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the operation that failed.
    ///
    /// # Examples
    ///
    /// This example needs the Slang shared library at run time, so it is
    /// compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Operation};
    ///
    /// let global = GlobalSession::new()?;
    /// if let Err(error) = GlobalSession::new() {
    ///     assert_eq!(error.operation(), Operation::CreateGlobalSession);
    /// }
    /// # drop(global);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns the result code that Slang returned for the failed call, or
    /// `None` when Slang returned no code.
    ///
    /// A result code is the signed number that a Slang function returns to
    /// say whether it succeeded, where a negative number means failure. It is
    /// `None` when Slang signalled the failure by returning nothing, or when
    /// this crate found the failure itself before or after the call. Use it to
    /// report a failure, and use [`Error::kind`] to decide what to do.
    ///
    /// # Examples
    ///
    /// See the example of [`Error`].
    #[must_use]
    pub fn code(&self) -> Option<i32> {
        self.code
    }

    /// Returns the diagnostics of the failed call, which are empty when Slang
    /// printed no message.
    ///
    /// # Examples
    ///
    /// See the example of [`Diagnostics`].
    #[must_use]
    pub fn diagnostics(&self) -> &Diagnostics {
        &self.diagnostics
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot {}", self.operation.phrase())?;
        if let Some(subject) = &self.subject {
            write!(f, " {subject}")?;
        }
        if let Some(code) = self.code {
            write!(f, " (Slang result code {code:#010x})")?;
        }
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        if !self.diagnostics.is_empty() {
            write!(f, ":\n{}", self.diagnostics)?;
        }
        Ok(())
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        let source: &(dyn StdError + 'static) = self.source.as_deref()?;
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
/// See the example of [`Error`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// An argument was invalid, such as a string with a NUL byte or a position
    /// past the end of a list, or Slang reported an invalid argument.
    /// Correct the call.
    InvalidArgument,
    /// Something that the call named does not exist, such as a module, an
    /// entry point, or a profile. Correct the name.
    NotFound,
    /// Slang could not compile the shader source. The diagnostics say why.
    /// Fix the source.
    Compile,
    /// Slang cannot do what the call asked on this installation, such as
    /// produce a target whose downstream compiler, a separate program or
    /// library that Slang calls for some targets, is missing. Install what is
    /// missing, or choose another target.
    Unsupported,
    /// Slang failed without giving a reason, or returned a number that does
    /// not fit the type of this crate. Report it with the input that caused
    /// it.
    Internal,
    /// Slang returned a result code or a value that this crate does not know.
    /// A value that the Slang headers of the pinned version do not list means
    /// that the loaded library differs from that version. Report it with the
    /// input that caused it.
    Unrecognized,
    /// A file system that the program gave Slang failed or panicked during the
    /// call.
    FileSystem,
    /// Text that Slang returned is not valid UTF-8.
    InvalidUtf8,
    /// The Slang library that the program loaded is not the build that this
    /// crate was built for. Make the dynamic loader find the library of the
    /// Vulkan SDK that `slang-compiler-sys` names.
    VersionMismatch,
    /// A global session of this crate is already alive on this thread, and the
    /// crate allows one for each thread. Reuse it, or create the new one on
    /// another thread.
    AlreadyExists,
}

/// The operation that an [`Error`] reports.
///
/// Each variant names one public operation of the crate that can fail. The
/// enumeration is non-exhaustive, so a later version can add operations, and
/// a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Error::operation`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Operation {
    /// The creation of a global session, by [`GlobalSession::new`] or
    /// [`GlobalSessionBuilder::build`].
    ///
    /// [`GlobalSession::new`]: crate::GlobalSession::new
    /// [`GlobalSessionBuilder::build`]: crate::GlobalSessionBuilder::build
    CreateGlobalSession,
    /// The check of [`GlobalSession::check_target_support`].
    ///
    /// [`GlobalSession::check_target_support`]: crate::GlobalSession::check_target_support
    CheckTargetSupport,
}

impl Operation {
    fn phrase(self) -> &'static str {
        match self {
            Self::CreateGlobalSession => "create a global session",
            Self::CheckTargetSupport => "produce code for target",
        }
    }
}

/// The messages that Slang printed for one call, such as errors, warnings,
/// and notes.
///
/// Slang returns its messages as one text, not as a list. The crate keeps that
/// text unchanged, except that it replaces each byte sequence that is not
/// valid UTF-8 with U+FFFD, the replacement character, so that an error is
/// never lost to a second error about the encoding of its own text. The text
/// is empty when Slang printed nothing. Read it from [`Error::diagnostics`]
/// after a failure.
///
/// # Examples
///
/// This example prints the diagnostics of a failure when there are any:
///
/// ```
/// use slang_compiler::Error;
///
/// fn report(error: &Error) {
///     if error.diagnostics().is_empty() {
///         eprintln!("{error}");
///     } else {
///         eprintln!("Slang said:\n{}", error.diagnostics().as_str());
///     }
/// }
/// # let _ = report;
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct Diagnostics {
    text: String,
}

impl Diagnostics {
    #[cfg(test)]
    pub(crate) fn from_text(text: &str) -> Self {
        Self {
            text: text.to_owned(),
        }
    }

    /// Returns `true` when Slang printed no message.
    ///
    /// # Examples
    ///
    /// See the example of [`Diagnostics`].
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Returns the text of the messages.
    ///
    /// # Examples
    ///
    /// See the example of [`Diagnostics`].
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

impl fmt::Display for Diagnostics {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// The value of a successful call together with the diagnostics that Slang
/// printed for it.
///
/// Slang can print warnings and other messages for a call that succeeds, and
/// they are information that a program usually shows to its user. Every
/// method whose Slang call prints diagnostics returns its value inside this
/// record, so the messages are not lost. Read `diagnostics`, then take the
/// value with [`Compiled::into_value`] or from the field `value`.
///
/// # Examples
///
/// This example shows the messages of a successful call and keeps its value:
///
/// ```
/// use slang_compiler::Compiled;
///
/// fn keep<T>(compiled: Compiled<T>) -> T {
///     if !compiled.diagnostics.is_empty() {
///         eprintln!("{}", compiled.diagnostics);
///     }
///     compiled.into_value()
/// }
/// # let _ = keep::<()>;
/// ```
///
/// The record has exactly these two fields, so a program can also take it
/// apart with a pattern:
///
/// ```
/// use slang_compiler::Compiled;
///
/// fn split<T>(compiled: Compiled<T>) -> (T, String) {
///     let Compiled { value, diagnostics } = compiled;
///     (value, diagnostics.to_string())
/// }
/// # let _ = split::<()>;
/// ```
#[must_use]
#[expect(
    clippy::exhaustive_structs,
    reason = "a value and its diagnostics close the meaning of the record, which API-9 exempts, and Section 6.8 of docs/design/slang-compiler.md defines it as a passive record with two public fields (API-10)"
)]
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Compiled<T> {
    /// The value that the call returned.
    pub value: T,
    /// The messages that Slang printed for the call, empty when there were
    /// none.
    pub diagnostics: Diagnostics,
}

impl<T> Compiled<T> {
    /// Returns the value and drops the diagnostics.
    ///
    /// # Examples
    ///
    /// See the example of [`Compiled`].
    pub fn into_value(self) -> T {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::{Compiled, Diagnostics, Error, ErrorKind, Operation};
    use crate::{GlobalSessionBuilder, LanguageVersion, TargetFormat, Toggle};

    fn diagnostics(text: &str) -> Diagnostics {
        Diagnostics::from_text(text)
    }

    #[test]
    fn display_names_the_operation_alone_for_a_bare_error() {
        let error = Error::new(ErrorKind::Internal, Operation::CreateGlobalSession);
        assert_eq!(error.to_string(), "cannot create a global session");
    }

    #[test]
    fn display_adds_the_subject_and_the_result_code_in_hexadecimal() {
        let error = Error::new(ErrorKind::Unsupported, Operation::CheckTargetSupport)
            .with_subject("`Dxil`".to_owned())
            .with_code(-2_113_929_211);
        assert_eq!(
            error.to_string(),
            "cannot produce code for target `Dxil` (Slang result code 0x82000005)"
        );
    }

    #[test]
    fn display_adds_the_detail_after_a_colon() {
        let error = Error::new(ErrorKind::AlreadyExists, Operation::CreateGlobalSession)
            .with_detail("a global session is alive".to_owned());
        assert_eq!(
            error.to_string(),
            "cannot create a global session: a global session is alive"
        );
    }

    #[test]
    fn display_ends_with_the_diagnostics_on_their_own_lines() {
        let error = Error::new(ErrorKind::Compile, Operation::CreateGlobalSession)
            .with_code(-2_147_467_259)
            .with_diagnostics(diagnostics("error[E1]: first\nnote: second\n"));
        assert_eq!(
            error.to_string(),
            "cannot create a global session (Slang result code 0x80004005):\nerror[E1]: first\nnote: second\n"
        );
        assert_eq!(
            error.diagnostics().as_str(),
            "error[E1]: first\nnote: second\n"
        );
    }

    #[test]
    fn accessors_return_what_the_error_records() {
        let error = Error::new(ErrorKind::NotFound, Operation::CheckTargetSupport).with_code(-1);
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(error.operation(), Operation::CheckTargetSupport);
        assert_eq!(error.code(), Some(-1));
        assert!(error.diagnostics().is_empty());
        assert!(std::error::Error::source(&error).is_none());
    }

    #[test]
    fn compiled_into_value_returns_the_value() {
        let compiled = Compiled {
            value: 7,
            diagnostics: diagnostics("warning"),
        };
        assert_eq!(compiled.diagnostics.to_string(), "warning");
        assert_eq!(compiled.into_value(), 7);
    }

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn value_types_are_send_and_sync() {
        assert_send_sync::<Error>();
        assert_send_sync::<ErrorKind>();
        assert_send_sync::<Operation>();
        assert_send_sync::<Diagnostics>();
        assert_send_sync::<Compiled<Diagnostics>>();
        assert_send_sync::<GlobalSessionBuilder>();
        assert_send_sync::<TargetFormat>();
        assert_send_sync::<Toggle>();
        assert_send_sync::<LanguageVersion>();
    }
}
