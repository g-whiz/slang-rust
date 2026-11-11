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
/// This example handles a Slang library of another build than the one this
/// crate was built for, which the crate refuses. It needs the Slang shared
/// library at run time, so it is compiled but not run:
///
/// ```no_run
/// use slang_compiler::{ErrorKind, GlobalSession};
///
/// match GlobalSession::new() {
///     Ok(global) => println!("Slang {}", global.build_tag()),
///     Err(error) if error.kind() == ErrorKind::VersionMismatch => {
///         eprintln!("install the Vulkan SDK that slang-compiler-sys names: {error}");
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
    /// The creation of a session, by [`SessionBuilder::build`].
    ///
    /// [`SessionBuilder::build`]: crate::SessionBuilder::build
    CreateSession,
    /// The selection of a target by its position, by
    /// [`Session::target_index`].
    ///
    /// [`Session::target_index`]: crate::Session::target_index
    SelectTarget,
    /// The check of a compiler option name, by [`OptionName::new`].
    ///
    /// [`OptionName::new`]: crate::OptionName::new
    CreateOptionName,
    /// The load of a module, by [`Session::load_module`],
    /// [`Session::load_module_from_source`], or [`Session::load_module_from_ir`].
    ///
    /// [`Session::load_module`]: crate::Session::load_module
    /// [`Session::load_module_from_source`]: crate::Session::load_module_from_source
    /// [`Session::load_module_from_ir`]: crate::Session::load_module_from_ir
    LoadModule,
    /// The listing of the modules of a session, by [`Session::loaded_modules`].
    ///
    /// [`Session::loaded_modules`]: crate::Session::loaded_modules
    ListLoadedModules,
    /// The reading of the name of a module, by [`Module::name`].
    ///
    /// [`Module::name`]: crate::Module::name
    ReadModuleName,
    /// The reading of the file path of a module, by [`Module::file_path`].
    ///
    /// [`Module::file_path`]: crate::Module::file_path
    ReadModuleFilePath,
    /// The listing of the files that a module depends on, by
    /// [`Module::dependency_file_paths`].
    ///
    /// [`Module::dependency_file_paths`]: crate::Module::dependency_file_paths
    ListDependencyFiles,
    /// The serialization of a module, by [`Module::serialize`].
    ///
    /// [`Module::serialize`]: crate::Module::serialize
    SerializeModule,
    /// The lookup of an entry point by its name, by [`Module::entry_point`].
    ///
    /// [`Module::entry_point`]: crate::Module::entry_point
    FindEntryPoint,
    /// The listing of the entry points of a module, by [`Module::entry_points`].
    ///
    /// [`Module::entry_points`]: crate::Module::entry_points
    ListEntryPoints,
    /// The composition of component types, by [`Session::compose`].
    ///
    /// [`Session::compose`]: crate::Session::compose
    Compose,
    /// The creation of a type conformance, by [`Session::type_conformance`].
    ///
    /// [`Session::type_conformance`]: crate::Session::type_conformance
    CreateTypeConformance,
    /// The count of the specialization parameters of a component type, by
    /// [`ComponentType::specialization_param_count`].
    ///
    /// [`ComponentType::specialization_param_count`]: crate::ComponentType::specialization_param_count
    CountSpecializationParams,
    /// The specialization of a component type, by [`ComponentType::specialize`].
    ///
    /// [`ComponentType::specialize`]: crate::ComponentType::specialize
    Specialize,
    /// The linking of a component type, by the methods `link` and
    /// `link_with_options` of [`Module`], [`EntryPoint`], [`ComponentType`], and
    /// [`TypeConformance`].
    ///
    /// [`Module`]: crate::Module
    /// [`EntryPoint`]: crate::EntryPoint
    /// [`ComponentType`]: crate::ComponentType
    /// [`TypeConformance`]: crate::TypeConformance
    Link,
    /// The generation of target code, by [`LinkedProgram::entry_point_code`] or
    /// [`LinkedProgram::target_code`].
    ///
    /// [`LinkedProgram::entry_point_code`]: crate::LinkedProgram::entry_point_code
    /// [`LinkedProgram::target_code`]: crate::LinkedProgram::target_code
    GenerateCode,
    /// The reading of the metadata of generated code, by
    /// [`LinkedProgram::entry_point_metadata`] or
    /// [`LinkedProgram::target_metadata`].
    ///
    /// [`LinkedProgram::entry_point_metadata`]: crate::LinkedProgram::entry_point_metadata
    /// [`LinkedProgram::target_metadata`]: crate::LinkedProgram::target_metadata
    ReadMetadata,
    /// A query of the metadata of generated code, by
    /// [`Metadata::is_parameter_location_used`].
    ///
    /// [`Metadata::is_parameter_location_used`]: crate::Metadata::is_parameter_location_used
    QueryMetadata,
    /// The reading of a blob, by [`Blob::to_str`] or [`Blob::from_raw`].
    ///
    /// [`Blob::to_str`]: crate::Blob::to_str
    /// [`Blob::from_raw`]: crate::Blob::from_raw
    ReadBlob,
    /// The layout of a component type for a target, by the methods `layout`
    /// of [`Module`], [`EntryPoint`], [`ComponentType`], [`TypeConformance`],
    /// and [`LinkedProgram`].
    ///
    /// [`Module`]: crate::Module
    /// [`EntryPoint`]: crate::EntryPoint
    /// [`ComponentType`]: crate::ComponentType
    /// [`TypeConformance`]: crate::TypeConformance
    /// [`LinkedProgram`]: crate::LinkedProgram
    Layout,
    /// A query of reflection, by a method of a handle of the module
    /// [`reflection`](crate::reflection).
    Reflect,
}

impl Operation {
    fn phrase(self) -> &'static str {
        match self {
            Self::CreateGlobalSession => "create a global session",
            Self::CheckTargetSupport => "produce code for target",
            Self::CreateSession => "create a session",
            Self::SelectTarget => "select target",
            Self::CreateOptionName => "name compiler option",
            Self::LoadModule => "load module",
            Self::ListLoadedModules => "list the loaded modules",
            Self::ReadModuleName => "read the name of a module",
            Self::ReadModuleFilePath => "read the file path of a module",
            Self::ListDependencyFiles => "list the dependency files of a module",
            Self::SerializeModule => "serialize module",
            Self::FindEntryPoint => "find entry point",
            Self::ListEntryPoints => "list the entry points of a module",
            Self::Compose => "compose component types",
            Self::CreateTypeConformance => "create a type conformance",
            Self::CountSpecializationParams => "count the specialization parameters",
            Self::Specialize => "specialize a component type",
            Self::Link => "link a component type",
            Self::GenerateCode => "generate code",
            Self::ReadMetadata => "read metadata",
            Self::QueryMetadata => "query metadata",
            Self::ReadBlob => "read a blob",
            Self::Layout => "lay out a component type",
            Self::Reflect => "read reflection",
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
    /// Converts the bytes of a diagnostics blob into text, replacing each
    /// sequence that is not valid UTF-8 with U+FFFD and dropping the NUL byte
    /// that ends the text when Slang counted it in the size.
    pub(crate) fn from_bytes(bytes: &[u8]) -> Self {
        let bytes = bytes.strip_suffix(&[0]).unwrap_or(bytes);
        Self {
            text: String::from_utf8_lossy(bytes).into_owned(),
        }
    }

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
    use crate::{
        BindingIndex, BindingSpace, CompilerOptions, DebugInfoLevel, FloatingPointMode,
        GlobalSessionBuilder, LanguageVersion, LineDirectiveMode, MatrixLayout, OptimizationLevel,
        OptionName, ParameterCategory, Stage, Target, TargetFormat, TargetIndex, Toggle,
    };

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
        let error = Error::new(ErrorKind::VersionMismatch, Operation::CreateGlobalSession)
            .with_detail("the library reports no build tag".to_owned());
        assert_eq!(
            error.to_string(),
            "cannot create a global session: the library reports no build tag"
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
        assert_send_sync::<MatrixLayout>();
        assert_send_sync::<Stage>();
        assert_send_sync::<OptimizationLevel>();
        assert_send_sync::<DebugInfoLevel>();
        assert_send_sync::<FloatingPointMode>();
        assert_send_sync::<LineDirectiveMode>();
        assert_send_sync::<CompilerOptions>();
        assert_send_sync::<OptionName>();
        assert_send_sync::<Target>();
        assert_send_sync::<TargetIndex>();
        assert_send_sync::<ParameterCategory>();
        assert_send_sync::<BindingSpace>();
        assert_send_sync::<BindingIndex>();
    }

    #[test]
    fn display_names_each_operation_of_sessions_and_options() {
        let cases = [
            (Operation::CreateSession, "cannot create a session"),
            (Operation::SelectTarget, "cannot select target"),
            (Operation::CreateOptionName, "cannot name compiler option"),
            (Operation::LoadModule, "cannot load module"),
            (
                Operation::ListLoadedModules,
                "cannot list the loaded modules",
            ),
            (
                Operation::ReadModuleName,
                "cannot read the name of a module",
            ),
            (
                Operation::ReadModuleFilePath,
                "cannot read the file path of a module",
            ),
            (
                Operation::ListDependencyFiles,
                "cannot list the dependency files of a module",
            ),
            (Operation::SerializeModule, "cannot serialize module"),
            (Operation::FindEntryPoint, "cannot find entry point"),
            (
                Operation::ListEntryPoints,
                "cannot list the entry points of a module",
            ),
            (Operation::Compose, "cannot compose component types"),
            (
                Operation::CreateTypeConformance,
                "cannot create a type conformance",
            ),
            (
                Operation::CountSpecializationParams,
                "cannot count the specialization parameters",
            ),
            (Operation::Specialize, "cannot specialize a component type"),
            (Operation::Link, "cannot link a component type"),
            (Operation::GenerateCode, "cannot generate code"),
            (Operation::ReadMetadata, "cannot read metadata"),
            (Operation::QueryMetadata, "cannot query metadata"),
            (Operation::ReadBlob, "cannot read a blob"),
            (Operation::Layout, "cannot lay out a component type"),
            (Operation::Reflect, "cannot read reflection"),
        ];
        for (operation, text) in cases {
            assert_eq!(
                Error::new(ErrorKind::InvalidArgument, operation).to_string(),
                text
            );
        }
    }
}
