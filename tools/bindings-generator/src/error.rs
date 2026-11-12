use std::error::Error as StdError;
use std::fmt;

/// The kind of failure that stopped the binding generator.
///
/// Use it to tell failures apart without reading the message, such as in a
/// test that expects one particular refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorKind {
    /// The header directory could not be read, such as when it does not
    /// exist.
    Io,
    /// libclang, the C and C++ parser library of the LLVM project, could not
    /// be loaded, or it failed to parse a file.
    Libclang,
    /// A parse reported a diagnostic of error severity or above.
    Diagnostic,
    /// The build tag in `slang-tag-version.h` is missing, malformed, or
    /// differs from the pinned build tag.
    Version,
    /// A declaration or a macro matches no rule of the rule set.
    UnmatchedDeclaration,
    /// A rule of the rule set decides no declaration or macro of the headers,
    /// because none matches it or an earlier rule decides every match.
    StaleRule,
    /// An included declaration uses a construct that the generated Rust
    /// cannot express, such as an aggregate passed by value.
    Unsupported,
    /// The parses for two supported targets differ in a way that no rule
    /// accepts.
    TargetDifference,
    /// A macro or a default member initializer could not be evaluated, or its
    /// value does not fit its type.
    Evaluation,
    /// Two generated items would have the same Rust name.
    NameCollision,
    /// rustfmt could not be started, failed to format the generated text, or
    /// did not finish within 60 seconds.
    Rustfmt,
}

/// A place in a Slang header, given as the file name and the line number.
///
/// The file name has no directory, so that messages do not depend on where
/// the headers are installed.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Location {
    file: String,
    line: u32,
}

impl Location {
    pub(crate) fn new(file: impl Into<String>, line: u32) -> Self {
        Self {
            file: file.into(),
            line,
        }
    }

    /// Returns the name of the header file, such as `slang.h`.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the line number in the header file, starting at 1.
    #[must_use]
    pub fn line(&self) -> u32 {
        self.line
    }
}

impl fmt::Display for Location {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.file, self.line)
    }
}

/// The error that the binding generator returns when it cannot generate the
/// bindings.
///
/// Its message says what went wrong, and [`Error::location`] says where in
/// the headers, when the failure has a place there. The binary prints the
/// error as it is.
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    message: String,
    location: Option<Location>,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
}

impl Error {
    pub(crate) fn new(kind: ErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            location: None,
            source: None,
        }
    }

    pub(crate) fn at(kind: ErrorKind, location: &Location, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            location: Some(location.clone()),
            source: None,
        }
    }

    pub(crate) fn with_source(
        mut self,
        source: impl Into<Box<dyn StdError + Send + Sync + 'static>>,
    ) -> Self {
        self.source = Some(source.into());
        self
    }

    /// Returns the kind of the failure.
    #[must_use]
    pub fn kind(&self) -> ErrorKind {
        self.kind
    }

    /// Returns the place in the headers that caused the failure, if the
    /// failure has one.
    #[must_use]
    pub fn location(&self) -> Option<&Location> {
        self.location.as_ref()
    }

    /// Returns the message of the failure, without the location.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.location {
            Some(location) => write!(f, "{location}: {}", self.message),
            None => f.write_str(&self.message),
        }
    }
}

impl StdError for Error {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        let source: &(dyn StdError + 'static) = self.source.as_deref()?;
        Some(source)
    }
}

/// The result type of the binding generator.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
mod tests {
    use super::*;

    const fn assert_send_sync_static<T: Send + Sync + 'static>() {}

    #[test]
    fn error_and_location_are_send_sync_and_static() {
        assert_send_sync_static::<Error>();
        assert_send_sync_static::<Location>();
        assert_send_sync_static::<ErrorKind>();
    }

    #[test]
    fn display_puts_the_location_before_the_message() {
        let error = Error::at(
            ErrorKind::Unsupported,
            &Location::new("slang.h", 12),
            "function f is variadic",
        );
        assert_eq!(error.to_string(), "slang.h:12: function f is variadic");
        assert_eq!(
            Error::new(ErrorKind::Io, "cannot read").to_string(),
            "cannot read"
        );
    }
}
