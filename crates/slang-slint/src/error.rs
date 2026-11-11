use std::error::Error as StdError;
use std::fmt;

/// The result type of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// The error of every fallible operation of this crate.
///
/// Every operation that can fail returns this one type. Match on
/// [`Error::kind`] to decide what to do. When another library caused the
/// failure, such as Slint or `slango`, [`source`](StdError::source) returns
/// its error.
///
/// The text that [`Display`](fmt::Display) prints is a lowercase phrase that
/// begins with `cannot` and names the failed operation, followed by a colon
/// and a detail when there is one. It does not repeat the text of the source
/// error. The text is meant for people and can change between versions, so
/// code matches on the kind and not on the text.
///
/// # Examples
///
/// This example tells the user to choose a renderer of Slint on wgpu when
/// Slint renders with another graphics API:
///
/// ```
/// use slang_slint::ErrorKind;
///
/// fn report(api: &slint::GraphicsAPI<'_>) {
///     match slang_slint::device(api) {
///         Ok(device) => println!("slango runs on {}", device.adapter_name()),
///         Err(error) if error.kind() == ErrorKind::UnsupportedGraphicsApi => {
///             eprintln!("enable a renderer of Slint on wgpu 30: {error}");
///         }
///         Err(error) => eprintln!("{error}"),
///     }
/// }
/// # let _ = report;
/// ```
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    operation: &'static str,
    detail: Option<String>,
    source: Option<Box<dyn StdError + Send + Sync + 'static>>,
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

    pub(crate) fn with_source(mut self, source: impl StdError + Send + Sync + 'static) -> Self {
        self.source = Some(Box::new(source));
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
    /// The graphics API that [`device`](crate::device) received is not
    /// wgpu 30, such as when the application chose a renderer of Slint on
    /// OpenGL. Choose a renderer on wgpu, such as with the Slint feature
    /// `renderer-femtovg-wgpu`, and pass [`wgpu_configuration`](crate::wgpu_configuration)
    /// to Slint's backend selector.
    UnsupportedGraphicsApi,
    /// Slint rejected the texture that this crate gave it as an image. The
    /// source error is Slint's error. Report it as a defect of this crate.
    TextureImport,
    /// A size has a width or a height of zero. Skip resizing and drawing
    /// until the element has an area.
    InvalidSize,
    /// `slango` failed, such as when it created a texture. The source error
    /// is the error of `slango`, whose kind says how to recover.
    Slango,
}

#[cfg(test)]
mod tests {
    use super::{Error, ErrorKind};
    use std::error::Error as StdError;
    use std::io;

    const KINDS: [ErrorKind; 4] = [
        ErrorKind::UnsupportedGraphicsApi,
        ErrorKind::TextureImport,
        ErrorKind::InvalidSize,
        ErrorKind::Slango,
    ];

    #[test]
    fn display_names_the_operation_and_the_detail_for_every_kind() {
        for kind in KINDS {
            let error =
                Error::new(kind, "create a view").with_detail("the size is 0 by 4".to_owned());
            assert_eq!(error.kind(), kind);
            assert_eq!(
                error.to_string(),
                "cannot create a view: the size is 0 by 4"
            );
        }
    }

    #[test]
    fn display_without_a_detail_is_the_operation_alone() {
        let error = Error::new(ErrorKind::InvalidSize, "resize a view");
        assert_eq!(error.to_string(), "cannot resize a view");
        assert!(error.source().is_none());
    }

    #[test]
    fn source_returns_the_error_that_caused_the_failure() {
        let error = Error::new(ErrorKind::Slango, "create a texture")
            .with_source(io::Error::other("the device is lost"));
        assert_eq!(error.to_string(), "cannot create a texture");
        let source = error.source().map(ToString::to_string);
        assert_eq!(source.as_deref(), Some("the device is lost"));
    }

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn error_types_are_send_and_sync() {
        assert_send_sync::<Error>();
        assert_send_sync::<ErrorKind>();
    }
}
