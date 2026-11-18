use std::error::Error as StdError;
use std::fmt;

use slang_compiler_sys::SlangResult;

/// The result type of every fallible operation of this crate.
pub type Result<T> = std::result::Result<T, Error>;

/// The error of every fallible operation of this crate.
///
/// Every operation that can fail returns this one type. Match on
/// [`Error::kind`] to decide what to do. [`Error::operation`] says which
/// operation failed, [`Error::code`] gives the numeric result code that
/// slang-rhi returned, when it returned one, and [`Error::diagnostics`] gives
/// the messages that Slang, the shader compiler, reported for the failed
/// call, when there were any. An error of the kind [`ErrorKind::Slang`]
/// holds the error of `slang-compiler` as its
/// [`source`](StdError::source).
///
/// The text that [`Display`](fmt::Display) prints starts with `cannot`, names
/// the operation, gives the result code in hexadecimal when there is one, and
/// ends with a detail and with the diagnostics on a new line when there are
/// any. The text is meant for people and can change between versions, so code
/// matches on the kind and not on the text.
///
/// # Examples
///
/// This example tells a missing Vulkan driver apart from other failures. It
/// needs a Vulkan driver at run time, so it is compiled but not run:
///
/// ```no_run
/// use slang_compiler::GlobalSession;
/// use slang_rhi::{Device, DeviceOptions, ErrorKind};
///
/// let options = DeviceOptions::new(GlobalSession::new()?);
/// match Device::new(&options) {
///     Ok(device) => println!("{}", device.info().adapter_name()),
///     Err(error) if error.kind() == ErrorKind::NoDevice => {
///         eprintln!("install a Vulkan driver, such as lavapipe: {error}");
///     }
///     Err(error) => return Err(error.into()),
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct Error {
    kind: ErrorKind,
    operation: Operation,
    code: Option<i32>,
    detail: Option<String>,
    diagnostics: Option<String>,
    source: Option<Box<slang_compiler::Error>>,
}

impl Error {
    fn new(kind: ErrorKind, operation: Operation) -> Self {
        Self {
            kind,
            operation,
            code: None,
            detail: None,
            diagnostics: None,
            source: None,
        }
    }

    /// Returns the error of a check of this crate that failed before any call
    /// into slang-rhi.
    pub(crate) fn invalid(operation: Operation, detail: impl Into<String>) -> Self {
        let mut error = Self::new(ErrorKind::InvalidArgument, operation);
        error.detail = Some(detail.into());
        error
    }

    /// Returns the error of a call that returned null where slang-rhi promises
    /// an object.
    pub(crate) fn null(operation: Operation) -> Self {
        let mut error = Self::new(ErrorKind::Failed, operation);
        error.detail = Some(String::from("slang-rhi returned no object"));
        error
    }

    /// Returns the error of a value of slang-rhi that the crate does not
    /// know.
    pub(crate) fn unrecognized(operation: Operation, detail: impl Into<String>) -> Self {
        let mut error = Self::new(ErrorKind::Unrecognized, operation);
        error.detail = Some(detail.into());
        error
    }

    /// Returns the error of a device whose kind of Slang session the crate
    /// cannot create.
    pub(crate) fn unsupported(operation: Operation, detail: impl Into<String>) -> Self {
        let mut error = Self::new(ErrorKind::Unsupported, operation);
        error.detail = Some(detail.into());
        error
    }

    /// Returns the error of a failed call of `slang-compiler`.
    pub(crate) fn slang(operation: Operation, source: slang_compiler::Error) -> Self {
        let mut error = Self::new(ErrorKind::Slang, operation);
        error.source = Some(Box::new(source));
        error
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
    /// This example needs a Vulkan driver at run time, so it is compiled but
    /// not run:
    ///
    /// ```no_run
    /// use slang_compiler::GlobalSession;
    /// use slang_rhi::{Device, DeviceOptions, Operation};
    ///
    /// let options = DeviceOptions::new(GlobalSession::new()?);
    /// if let Err(error) = Device::new(&options) {
    ///     assert_eq!(error.operation(), Operation::CreateDevice);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn operation(&self) -> Operation {
        self.operation
    }

    /// Returns the result code that slang-rhi returned for the failed call,
    /// or `None` when it returned no code.
    ///
    /// A result code is the signed number that a function of slang-rhi
    /// returns to say whether it succeeded, where a negative number means
    /// failure. It is `None` when this crate found the failure itself, before
    /// or after the call. Use it to report a failure, and use
    /// [`Error::kind`] to decide what to do.
    ///
    /// # Examples
    ///
    /// See the example of [`Error::operation`], which reads the operation in
    /// the same way.
    #[must_use]
    pub fn code(&self) -> Option<i32> {
        self.code
    }

    /// Returns the messages that Slang reported for the failed call, or
    /// `None` when it reported none.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::create_compute_pipeline`](crate::Device::create_compute_pipeline).
    #[must_use]
    pub fn diagnostics(&self) -> Option<&str> {
        self.diagnostics.as_deref()
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "cannot {}", self.operation.phrase())?;
        if let Some(code) = self.code {
            write!(f, " (slang-rhi result code {code:#010x})")?;
        }
        if let Some(detail) = &self.detail {
            write!(f, ": {detail}")?;
        }
        if let Some(diagnostics) = &self.diagnostics {
            write!(f, ":\n{diagnostics}")?;
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
    /// An argument was invalid, such as a range past the end of a buffer, a
    /// program of another session, or an object of another device, or
    /// slang-rhi reported an invalid argument. Correct the call.
    InvalidArgument,
    /// slang-rhi could not create a device, such as when no Vulkan loader or
    /// driver is installed on Linux. Install the Vulkan loader and a driver,
    /// such as lavapipe, the software driver of Mesa, or on Windows a driver
    /// of Direct3D 12.
    NoDevice,
    /// slang-rhi could not create a shader program or a compute pipeline,
    /// such as when Slang cannot generate code for the device. The
    /// diagnostics say why, when Slang gave a reason.
    Compile,
    /// slang-rhi ran out of memory.
    OutOfMemory,
    /// slang-rhi or the device does not support what the call asked.
    Unsupported,
    /// slang-rhi failed without a more specific reason, or returned no object
    /// where it promises one. Report it with the input that caused it.
    Failed,
    /// slang-rhi returned a result code or a value that this crate does not
    /// know. Report it with the input that caused it.
    Unrecognized,
    /// A call of `slang-compiler` failed. The error of `slang-compiler` is the
    /// [`source`](StdError::source) of this error.
    Slang,
}

/// The operation that an [`Error`] reports.
///
/// Each variant names one public operation of the crate that can fail. The
/// enumeration is non-exhaustive, so a later version can add operations, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Error::operation`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Operation {
    /// The listing of the adapters of a device type, by
    /// [`adapters`](crate::adapters).
    ListAdapters,
    /// The creation of a device, by [`Device::new`](crate::Device::new).
    CreateDevice,
    /// The creation of a shader program, by
    /// [`Device::create_shader_program`](crate::Device::create_shader_program).
    CreateShaderProgram,
    /// The creation of a compute pipeline, by
    /// [`Device::create_compute_pipeline`](crate::Device::create_compute_pipeline).
    CreateComputePipeline,
    /// The creation of a buffer, by
    /// [`Device::create_buffer`](crate::Device::create_buffer).
    CreateBuffer,
    /// The reading of a buffer, by
    /// [`Device::read_buffer`](crate::Device::read_buffer).
    ReadBuffer,
    /// The lookup of the queue of a device, by
    /// [`Device::queue`](crate::Device::queue).
    GetQueue,
    /// The creation of a command encoder, by
    /// [`Queue::encoder`](crate::Queue::encoder).
    CreateCommandEncoder,
    /// The beginning of a compute pass, by
    /// [`CommandEncoder::compute_pass`](crate::CommandEncoder::compute_pass).
    BeginComputePass,
    /// The binding of a compute pipeline, by
    /// [`ComputePass::bind_pipeline`](crate::ComputePass::bind_pipeline).
    BindPipeline,
    /// A dispatch, by [`ComputePass::dispatch`](crate::ComputePass::dispatch).
    Dispatch,
    /// The end of the encoding, by
    /// [`CommandEncoder::finish`](crate::CommandEncoder::finish).
    FinishEncoder,
    /// The submission of command buffers, by
    /// [`Queue::submit`](crate::Queue::submit).
    Submit,
    /// The wait for a queue, by
    /// [`Queue::wait_on_host`](crate::Queue::wait_on_host).
    WaitOnHost,
    /// The lookup of a parameter by its name, by
    /// [`ShaderObject::offset_of`](crate::ShaderObject::offset_of).
    FindOffset,
    /// The setting of uniform data, by
    /// [`ShaderObject::set_data`](crate::ShaderObject::set_data).
    SetData,
    /// The binding of a buffer, by
    /// [`ShaderObject::set_buffer`](crate::ShaderObject::set_buffer).
    SetBuffer,
    /// The lookup of the shader object of an entry point, by
    /// [`ShaderObject::entry_point`](crate::ShaderObject::entry_point).
    GetEntryPoint,
    /// The lookup of the shader object of a `ConstantBuffer`, a `cbuffer`, or
    /// a `ParameterBlock` parameter, by
    /// [`ShaderObject::object`](crate::ShaderObject::object).
    GetObject,
    /// The creation of a Slang session like the session of a device, by
    /// [`Device::create_slang_session`](crate::Device::create_slang_session).
    CreateSlangSession,
}

impl Operation {
    fn phrase(self) -> &'static str {
        match self {
            Self::ListAdapters => "list the adapters",
            Self::CreateDevice => "create a device",
            Self::CreateShaderProgram => "create a shader program",
            Self::CreateComputePipeline => "create a compute pipeline",
            Self::CreateBuffer => "create a buffer",
            Self::ReadBuffer => "read a buffer",
            Self::GetQueue => "get the queue of a device",
            Self::CreateCommandEncoder => "create a command encoder",
            Self::BeginComputePass => "begin a compute pass",
            Self::BindPipeline => "bind a compute pipeline",
            Self::Dispatch => "dispatch a compute kernel",
            Self::FinishEncoder => "finish a command encoder",
            Self::Submit => "submit command buffers",
            Self::WaitOnHost => "wait for a queue",
            Self::FindOffset => "find a parameter",
            Self::SetData => "set uniform data",
            Self::SetBuffer => "bind a buffer",
            Self::GetEntryPoint => "get the shader object of an entry point",
            Self::GetObject => "get the shader object of a parameter",
            Self::CreateSlangSession => "create a Slang session for a device",
        }
    }
}

/// Returns `Ok` for a result code that is not negative, and the classified
/// error, which holds the diagnostics, for a negative one.
///
/// This is the one function of the crate that reads a result code of
/// slang-rhi.
pub(crate) fn check(
    operation: Operation,
    result: SlangResult,
    diagnostics: Option<String>,
) -> Result<()> {
    if result.0 >= 0 {
        return Ok(());
    }
    let mut error = Error::new(classify(operation, result), operation);
    error.code = Some(result.0);
    error.diagnostics = diagnostics;
    Err(error)
}

/// Classifies a negative result code by an ordered table of rows, one for
/// each arm, in which the first row that matches decides the kind.
fn classify(operation: Operation, result: SlangResult) -> ErrorKind {
    match (operation, result) {
        (Operation::CreateDevice, _) => ErrorKind::NoDevice,
        (Operation::CreateShaderProgram | Operation::CreateComputePipeline, _) => {
            ErrorKind::Compile
        }
        (_, SlangResult::SLANG_E_INVALID_ARG) => ErrorKind::InvalidArgument,
        (_, SlangResult::SLANG_E_OUT_OF_MEMORY) => ErrorKind::OutOfMemory,
        (_, SlangResult::SLANG_E_NOT_IMPLEMENTED | SlangResult::SLANG_E_NOT_AVAILABLE) => {
            ErrorKind::Unsupported
        }
        (
            _,
            SlangResult::SLANG_FAIL
            | SlangResult::SLANG_E_NO_INTERFACE
            | SlangResult::SLANG_E_ABORT
            | SlangResult::SLANG_E_INVALID_HANDLE
            | SlangResult::SLANG_E_BUFFER_TOO_SMALL
            | SlangResult::SLANG_E_UNINITIALIZED
            | SlangResult::SLANG_E_PENDING
            | SlangResult::SLANG_E_CANNOT_OPEN
            | SlangResult::SLANG_E_NOT_FOUND
            | SlangResult::SLANG_E_INTERNAL_FAIL
            | SlangResult::SLANG_E_TIME_OUT,
        ) => ErrorKind::Failed,
        _ => ErrorKind::Unrecognized,
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error as _;

    use slang_compiler_sys::SlangResult;

    use super::{Error, ErrorKind, Operation, check};

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn error_and_its_kinds_are_send_sync_and_static() {
        assert_send_sync::<Error>();
        assert_send_sync::<ErrorKind>();
        assert_send_sync::<Operation>();
    }

    #[test]
    fn check_accepts_every_code_that_is_not_negative() {
        for code in [0, 1, i32::MAX] {
            check(Operation::Submit, SlangResult(code), None)
                .expect("a code of 0 or more succeeds");
        }
    }

    #[test]
    fn check_classifies_each_known_code_of_an_ordinary_operation() {
        let rows = [
            (SlangResult::SLANG_E_INVALID_ARG, ErrorKind::InvalidArgument),
            (SlangResult::SLANG_E_OUT_OF_MEMORY, ErrorKind::OutOfMemory),
            (SlangResult::SLANG_E_NOT_IMPLEMENTED, ErrorKind::Unsupported),
            (SlangResult::SLANG_E_NOT_AVAILABLE, ErrorKind::Unsupported),
            (SlangResult::SLANG_FAIL, ErrorKind::Failed),
            (SlangResult::SLANG_E_NO_INTERFACE, ErrorKind::Failed),
            (SlangResult::SLANG_E_ABORT, ErrorKind::Failed),
            (SlangResult::SLANG_E_INVALID_HANDLE, ErrorKind::Failed),
            (SlangResult::SLANG_E_BUFFER_TOO_SMALL, ErrorKind::Failed),
            (SlangResult::SLANG_E_UNINITIALIZED, ErrorKind::Failed),
            (SlangResult::SLANG_E_PENDING, ErrorKind::Failed),
            (SlangResult::SLANG_E_CANNOT_OPEN, ErrorKind::Failed),
            (SlangResult::SLANG_E_NOT_FOUND, ErrorKind::Failed),
            (SlangResult::SLANG_E_INTERNAL_FAIL, ErrorKind::Failed),
            (SlangResult::SLANG_E_TIME_OUT, ErrorKind::Failed),
            (SlangResult(-7), ErrorKind::Unrecognized),
        ];
        for (result, kind) in rows {
            let error = check(Operation::SetData, result, None).expect_err("a negative code fails");
            assert_eq!(error.kind(), kind, "{result:?}");
            assert_eq!(error.code(), Some(result.0));
            assert_eq!(error.operation(), Operation::SetData);
            assert_eq!(error.diagnostics(), None);
        }
    }

    #[test]
    fn check_reports_every_failure_of_device_creation_as_no_device() {
        for result in [
            SlangResult::SLANG_FAIL,
            SlangResult::SLANG_E_INVALID_ARG,
            SlangResult(-7),
        ] {
            let error = check(Operation::CreateDevice, result, None).expect_err("it fails");
            assert_eq!(error.kind(), ErrorKind::NoDevice, "{result:?}");
        }
    }

    #[test]
    fn check_reports_every_failure_of_a_program_or_pipeline_as_compile_with_its_diagnostics() {
        for operation in [
            Operation::CreateShaderProgram,
            Operation::CreateComputePipeline,
        ] {
            let error = check(
                operation,
                SlangResult::SLANG_E_INVALID_ARG,
                Some(String::from("error[E30015]: undefined identifier")),
            )
            .expect_err("it fails");
            assert_eq!(error.kind(), ErrorKind::Compile);
            assert_eq!(
                error.diagnostics(),
                Some("error[E30015]: undefined identifier")
            );
            assert_eq!(
                error.to_string(),
                format!(
                    "cannot {} (slang-rhi result code 0x80070057):\nerror[E30015]: undefined identifier",
                    if operation == Operation::CreateShaderProgram {
                        "create a shader program"
                    } else {
                        "create a compute pipeline"
                    }
                )
            );
        }
    }

    #[test]
    fn invalid_and_null_errors_carry_no_code_and_name_the_detail() {
        let invalid = Error::invalid(Operation::ReadBuffer, "the range ends past the buffer");
        assert_eq!(invalid.kind(), ErrorKind::InvalidArgument);
        assert_eq!(invalid.code(), None);
        assert_eq!(
            invalid.to_string(),
            "cannot read a buffer: the range ends past the buffer"
        );
        let null = Error::null(Operation::BindPipeline);
        assert_eq!(null.kind(), ErrorKind::Failed);
        assert_eq!(
            null.to_string(),
            "cannot bind a compute pipeline: slang-rhi returned no object"
        );
        assert!(null.source().is_none());
    }
}
