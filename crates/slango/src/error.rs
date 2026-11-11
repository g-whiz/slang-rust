use std::error::Error as StdError;
use std::fmt;

/// The result type of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// The error of every fallible operation of this crate.
///
/// Every operation that can fail returns this one type. Match on
/// [`Error::kind`] to decide what to do. When another library caused the
/// failure, such as wgpu, the graphics library on which this crate runs, or
/// `slang-compiler`, the Slang shader compiler, [`source`](StdError::source)
/// returns its error.
///
/// The text that [`Display`](fmt::Display) prints is a lowercase phrase that
/// begins with `cannot` and names the failed operation, followed by a colon
/// and a detail when there is one, such as ``cannot find a GPU adapter: no
/// adapter has a name that contains `Radeon` ``. It does not repeat the text
/// of the source error. The text is meant for people and can change between
/// versions, so code matches on the kind and not on the text.
///
/// # Examples
///
/// This example tells the user to install a driver when the machine has no
/// adapter, a GPU or a software implementation of one. It needs a GPU driver
/// at run time, so it is compiled but not run:
///
/// ```no_run
/// use slango::{Device, DeviceOptions, ErrorKind};
///
/// match Device::new(&DeviceOptions::default()) {
///     Ok(device) => println!("running on {}", device.adapter_name()),
///     Err(error) if error.kind() == ErrorKind::NoAdapter => {
///         eprintln!("install a GPU driver or a software Vulkan driver: {error}");
///     }
///     Err(error) => return Err(error.into()),
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
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

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "step 3 of Section 14 of docs/design/slango.md adds the load of a module that calls it, and until then only the unit tests use it"
        )
    )]
    pub(crate) fn compile(error: slang_compiler::Error) -> Self {
        Self::new(ErrorKind::Compile, "compile a Slang module").with_source(error)
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
    /// Slang failed to compile or link a module. The source error is the
    /// error of `slang-compiler`, with the messages that Slang printed. Fix
    /// the shader.
    Compile,
    /// wgpu found no adapter, a GPU or a software implementation of one, that
    /// matches the options. Install a GPU driver or a software driver such as
    /// lavapipe, or choose another adapter.
    NoAdapter,
    /// The adapter refused the device that the options request. The source
    /// error is the error of wgpu.
    DeviceRequest,
    /// naga, the shader translator inside wgpu, rejected the SPIR-V of a
    /// kernel, the compiled form of the shader. The error carries naga's text,
    /// which names the construct to avoid.
    UnsupportedShader,
    /// A kernel has a parameter of a kind that this version does not
    /// support, such as a texture or a sampler.
    UnsupportedParameter,
    /// A kernel needs a feature of the device that the device lacks. Request
    /// the feature, or use another adapter.
    MissingFeature,
    /// An argument was invalid, such as an unknown kernel name, a parameter
    /// that a call names but the kernel lacks, or a file that cannot be read
    /// as text. Correct the call.
    InvalidArgument,
    /// wgpu reported a validation error for work that this crate submitted.
    /// Report it as a defect of this crate.
    Backend,
}

#[cfg(test)]
mod tests {
    use super::{Error, ErrorKind};
    use std::error::Error as StdError;
    use std::io;

    const KINDS: [ErrorKind; 8] = [
        ErrorKind::Compile,
        ErrorKind::NoAdapter,
        ErrorKind::DeviceRequest,
        ErrorKind::UnsupportedShader,
        ErrorKind::UnsupportedParameter,
        ErrorKind::MissingFeature,
        ErrorKind::InvalidArgument,
        ErrorKind::Backend,
    ];

    #[test]
    fn display_names_the_operation_and_the_detail_for_every_kind() {
        for kind in KINDS {
            let error = Error::new(kind, "load a module")
                .with_detail("the kernel `scale` failed".to_owned());
            assert_eq!(error.kind(), kind);
            assert_eq!(
                error.to_string(),
                "cannot load a module: the kernel `scale` failed"
            );
        }
    }

    #[test]
    fn display_without_a_detail_is_the_operation_alone() {
        let error = Error::new(ErrorKind::NoAdapter, "find a GPU adapter");
        assert_eq!(error.to_string(), "cannot find a GPU adapter");
        assert!(error.source().is_none());
    }

    #[test]
    fn source_returns_the_error_that_caused_the_failure() {
        let error = Error::new(ErrorKind::DeviceRequest, "request a device")
            .with_source(io::Error::other("the adapter is lost"));
        assert_eq!(error.to_string(), "cannot request a device");
        let source = error.source().map(ToString::to_string);
        assert_eq!(source.as_deref(), Some("the adapter is lost"));
    }

    #[test]
    fn compile_keeps_the_error_of_slang_compiler_as_its_source() {
        let Err(cause) = slang_compiler::OptionName::new(-1) else {
            panic!("a negative compiler option name was accepted");
        };
        let cause_text = cause.to_string();
        let error = Error::compile(cause);
        assert_eq!(error.kind(), ErrorKind::Compile);
        assert_eq!(error.to_string(), "cannot compile a Slang module");
        let source = error
            .source()
            .and_then(|source| source.downcast_ref::<slang_compiler::Error>());
        assert_eq!(source.map(ToString::to_string), Some(cause_text));
    }

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn error_types_are_send_and_sync() {
        assert_send_sync::<Error>();
        assert_send_sync::<ErrorKind>();
    }
}
