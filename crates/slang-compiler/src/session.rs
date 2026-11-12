use std::ffi::{CString, c_char};
use std::fmt;
use std::path::{Path, PathBuf};
use std::ptr::{self, NonNull};

use slang_compiler_sys::{
    CompilerOptionName, IGlobalSession, ISession, PreprocessorMacroDesc,
    SLANG_TARGET_FLAG_GENERATE_SPIRV_DIRECTLY, SessionDesc, SlangEmitSpirvMethod,
    SlangFloatingPointMode, SlangInt, SlangLineDirectiveMode, SlangProfileID, TargetDesc,
};

use crate::enums::{MatrixLayout, TargetFormat, Toggle};
use crate::error::{Diagnostics, Error, ErrorKind, Operation, Result};
use crate::global_session::GlobalSession;
use crate::guard;
use crate::mapping;
use crate::options::{CompilerOptions, PreparedOptions, RawOptions};
use crate::owned::Owned;
use crate::strings::c_string;

/// A compilation context of Slang, with its targets, search paths, macros,
/// and compiler options.
///
/// Slang is a compiler for shaders, the programs that run on a graphics
/// processing unit (GPU). A session is created from a [`GlobalSession`] with
/// [`Session::builder`] and holds the settings that every compilation in it
/// shares, above all its targets, the formats of code that it generates, such
/// as SPIR-V for Vulkan. Code that a session loads stays loaded until the
/// session is dropped, so a program that needs to see a changed source file
/// creates a new session from the same global session.
///
/// A session holds its global session, so the global session lives at least
/// as long as the session, even when the program drops its own handle to it.
///
/// A session is neither [`Send`] nor [`Sync`], so it stays on the thread that
/// created it. Slang documents a session and every object created from it as
/// not safe to use from several threads at once. [`Clone`] returns another
/// handle to the same session, and the session lives until its last handle is
/// dropped.
///
/// # Examples
///
/// This example creates a session that generates SPIR-V and WebGPU Shading
/// Language (WGSL) source, and selects the first target. It needs the Slang
/// shared library at run time, so it is compiled but not run:
///
/// ```no_run
/// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
///
/// let global = GlobalSession::new()?;
/// let session = Session::builder(&global)
///     .target(Target::new(TargetFormat::Spirv).profile("spirv_1_5"))
///     .target(Target::new(TargetFormat::Wgsl))
///     .build()?;
/// let spirv = session.target_index(0)?;
/// # let _ = spirv;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// A session cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::Session>();
/// ```
///
/// A session cannot be shared between threads either, so this example does
/// not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::Session>();
/// ```
#[derive(Clone)]
pub struct Session {
    raw: Owned<ISession>,
    global: GlobalSession,
    target_count: usize,
}

impl Session {
    /// Returns a builder for a session of `global`.
    ///
    /// The builder holds a handle to the global session, so `global` can be
    /// dropped before the session is built.
    ///
    /// # Examples
    ///
    /// See the example of [`Session`].
    #[must_use]
    pub fn builder(global: &GlobalSession) -> SessionBuilder {
        SessionBuilder {
            global: global.clone(),
            targets: Vec::new(),
            search_paths: Vec::new(),
            macros: Vec::new(),
            matrix_layout: None,
            use_up_to_date_binary_module: None,
            options: CompilerOptions::default(),
        }
    }

    /// Returns the global session that this session was created from.
    ///
    /// # Examples
    ///
    /// This example needs the Slang shared library at run time, so it is
    /// compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .build()?;
    /// assert_eq!(session.global_session().as_raw(), global.as_raw());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn global_session(&self) -> &GlobalSession {
        &self.global
    }

    /// Returns the number of targets of the session.
    ///
    /// # Examples
    ///
    /// This example needs the Slang shared library at run time, so it is
    /// compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .target(Target::new(TargetFormat::Hlsl))
    ///     .build()?;
    /// assert_eq!(session.target_count(), 2);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn target_count(&self) -> usize {
        self.target_count
    }

    /// Returns the target at a position in the list of targets, for the
    /// calls that generate code or a layout for one target.
    ///
    /// The targets are numbered from 0 in the order in which
    /// [`SessionBuilder::target`] added them. The returned [`TargetIndex`]
    /// selects that target in later calls on objects of this session.
    ///
    /// # Examples
    ///
    /// See the example of [`Session`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `position`
    /// is not below [`Session::target_count`].
    pub fn target_index(&self, position: usize) -> Result<TargetIndex> {
        if position < self.target_count {
            Ok(TargetIndex { position })
        } else {
            Err(
                Error::new(ErrorKind::InvalidArgument, Operation::SelectTarget)
                    .with_subject(format!("`{position}`"))
                    .with_detail(format!(
                        "the session has {} targets, numbered from 0",
                        self.target_count
                    )),
            )
        }
    }

    /// Adopts a session that the caller created through the raw bindings of
    /// `slang-compiler-sys`.
    ///
    /// Use this to hand a session from code that calls Slang directly to this
    /// crate. The wrapper takes over the reference that the caller owns and
    /// releases it when its last handle drops, before it drops its handle to
    /// `global`.
    ///
    /// # Examples
    ///
    /// This example moves a session out of the crate and back. It needs the
    /// Slang shared library at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .build()?;
    /// let target_count = session.target_count();
    /// let (raw, parent) = session.into_raw();
    /// // SAFETY: `raw` carries the one reference that `into_raw` gave up, of a
    /// // session that `parent` created with `target_count` targets, and this
    /// // thread alone uses it.
    /// let session = unsafe { Session::from_raw(raw, parent, target_count) };
    /// # let _ = session;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a live session that the Slang library of the build
    ///   tag [`slang_compiler_sys::SLANG_BUILD_TAG`] created.
    /// - The caller owns one reference to it and transfers that reference.
    /// - `global` is the global session that the session was created from.
    /// - `target_count` is the number of targets that the session was created
    ///   with.
    /// - The session and every object created from it are not used on another
    ///   thread while the returned value or a clone of it exists.
    ///
    /// Section 11.7 of the design of this crate states these conditions.
    #[must_use]
    pub unsafe fn from_raw(
        raw: NonNull<ISession>,
        global: GlobalSession,
        target_count: usize,
    ) -> Self {
        // SAFETY: the caller guarantees that `raw` points to a live session of
        // the pinned build and transfers one reference to it (Section 11.7 of
        // `docs/design/slang-compiler.md`).
        let raw = unsafe { Owned::adopt(raw) };
        Self {
            raw,
            global,
            target_count,
        }
    }

    /// Gives up this handle's reference to the session without releasing it,
    /// and returns the pointer for use with the raw bindings of
    /// `slang-compiler-sys`, together with this handle's handle to the global
    /// session that the session was created from.
    ///
    /// The caller then owns the reference to the session and releases it.
    /// Keep the returned [`GlobalSession`] until after that release, or pass
    /// both back to [`Session::from_raw`]. Slang's session holds a reference
    /// to its global session of its own, but when that is the last reference,
    /// releasing the session frees the global session first and then writes
    /// into it, so the release order is safe only while another handle keeps
    /// the global session alive.
    ///
    /// # Examples
    ///
    /// See the example of [`Session::from_raw`].
    #[must_use]
    pub fn into_raw(self) -> (NonNull<ISession>, GlobalSession) {
        let Self { raw, global, .. } = self;
        (raw.into_raw(), global)
    }

    /// Returns the pointer to the session without changing its reference
    /// count, for use with the raw bindings of `slang-compiler-sys`.
    ///
    /// The pointer is valid only while this handle lives. Every use of it is
    /// unsafe and follows the safety contract of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example reads the global session of a session through the raw
    /// bindings. It needs the Slang shared library at run time, so it is
    /// compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
    /// use slang_compiler_sys::ISession;
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .build()?;
    /// // SAFETY: `session` keeps the session alive during the call, and only
    /// // this thread uses it.
    /// let raw_global = unsafe { ISession::getGlobalSession(session.as_raw().as_ptr()) };
    /// assert_eq!(raw_global, global.as_raw().as_ptr());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<ISession> {
        self.raw.as_non_null()
    }

    /// Returns the owning pointer to the session.
    pub(crate) fn raw(&self) -> &Owned<ISession> {
        &self.raw
    }

    /// Returns whether `other` is a handle to the same Slang session, for the
    /// check of SLC-7.
    pub(crate) fn is_same(&self, other: &Self) -> bool {
        self.raw.as_non_null() == other.raw.as_non_null()
    }

    /// Checks `target` against the target count of this session again, for
    /// a call that passes it to Slang, and returns it as Slang's index.
    ///
    /// A target index of another session can be past the end of this
    /// session's targets, which is reported as an error of kind
    /// `InvalidArgument` of `operation` (Section 6.4 of
    /// `docs/design/slang-compiler.md`).
    pub(crate) fn slang_target(
        &self,
        target: TargetIndex,
        operation: Operation,
    ) -> Result<SlangInt> {
        let position = target.position;
        if position >= self.target_count {
            return Err(Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
                "target {position} is past the end of the session's {} targets, so it belongs to another session",
                self.target_count
            )));
        }
        SlangInt::try_from(position).map_err(|error| {
            Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
                "target {position} does not fit Slang's index: {error}"
            ))
        })
    }
}

impl fmt::Debug for Session {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Session")
            .field("raw", &self.raw.as_non_null())
            .field("target_count", &self.target_count)
            .finish_non_exhaustive()
    }
}

/// The position of a target in the list of targets of a session.
///
/// A target is a format of code that a session generates, such as SPIR-V,
/// together with its profile and options. Get a target index from
/// [`Session::target_index`], which returns one only for a position below
/// [`Session::target_count`]. A target index belongs to the session that
/// returned it, and it holds only the position, not the session.
///
/// # Examples
///
/// See the example of [`Session`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TargetIndex {
    position: usize,
}

/// One target of a session: a format of code that Slang generates, with its
/// profile, capabilities, and compiler options.
///
/// A shader is a program that runs on a graphics processing unit (GPU), and
/// each graphics API takes shaders in its own format, such as SPIR-V for
/// Vulkan. A session generates code for each of its targets. A target is plain
/// data and does not check itself: [`SessionBuilder::build`] checks every
/// target, because only Slang knows the names of profiles and capabilities.
///
/// # Examples
///
/// This example describes a SPIR-V target for version 1.5 of SPIR-V that
/// keeps the names of entry points:
///
/// ```
/// use slang_compiler::{CompilerOptions, Target, TargetFormat, Toggle};
///
/// let mut options = CompilerOptions::default();
/// options.vulkan_use_entry_point_name(Toggle::On);
/// let target = Target::new(TargetFormat::Spirv)
///     .profile("spirv_1_5")
///     .options(options);
/// # let _ = target;
/// ```
#[derive(Clone, Debug)]
pub struct Target {
    format: TargetFormat,
    profile: Option<String>,
    capabilities: Vec<String>,
    options: CompilerOptions,
}

impl Target {
    /// Describes a target that generates code in `format`, with Slang's
    /// default profile and no options.
    ///
    /// # Examples
    ///
    /// See the example of [`Target`].
    #[must_use]
    pub fn new(format: TargetFormat) -> Self {
        Self {
            format,
            profile: None,
            capabilities: Vec::new(),
            options: CompilerOptions::default(),
        }
    }

    /// Sets the profile of the target, by Slang's name for it, and replaces
    /// any profile set earlier.
    ///
    /// A profile is a named set of features of a target that the generated
    /// code may use, such as `spirv_1_5` for version 1.5 of SPIR-V or
    /// `sm_6_5` for shader model 6.5 of Direct3D. Slang resolves the name when
    /// [`SessionBuilder::build`] runs.
    ///
    /// # Examples
    ///
    /// See the example of [`Target`].
    #[must_use]
    pub fn profile(self, name: impl Into<String>) -> Self {
        self.set_profile(name.into())
    }

    /// Adds a capability that the generated code may use, by Slang's name for
    /// it.
    ///
    /// A capability is a named feature of a target, such as an extension of
    /// SPIR-V, beyond those of the profile. Each call adds one more
    /// capability. Slang resolves the name when [`SessionBuilder::build`]
    /// runs, and the target passes it to Slang as the compiler option
    /// `Capability`, after the options of [`Target::options`].
    ///
    /// # Examples
    ///
    /// This example lets SPIR-V code use 64-bit atomic operations on
    /// integers:
    ///
    /// ```
    /// use slang_compiler::{Target, TargetFormat};
    ///
    /// let target = Target::new(TargetFormat::Spirv).capability("spvInt64Atomics");
    /// # let _ = target;
    /// ```
    #[must_use]
    pub fn capability(self, name: impl Into<String>) -> Self {
        self.add_capability(name.into())
    }

    /// Sets the compiler options of the target, which override the options of
    /// the session for this target, and replaces any options set earlier.
    ///
    /// The pinned Slang build never reads the options `LanguageVersion`,
    /// `WarningsAsErrors`, `EnableWarning`, and `DisableWarning` from the
    /// options of a target, so [`SessionBuilder::build`] rejects a target
    /// whose options hold an entry of any of them, also one that a raw setter
    /// added. Set them in the options of the session instead.
    ///
    /// # Examples
    ///
    /// See the example of [`Target`].
    #[must_use]
    pub fn options(mut self, options: CompilerOptions) -> Self {
        self.options = options;
        self
    }

    fn set_profile(mut self, name: String) -> Self {
        self.profile = Some(name);
        self
    }

    fn add_capability(mut self, name: String) -> Self {
        self.capabilities.push(name);
        self
    }
}

/// A builder for a [`Session`].
///
/// Each setter changes one setting and returns the builder, so calls can be
/// chained, and [`SessionBuilder::build`] checks every setting and creates the
/// session. A setting that is never set keeps Slang's default. A session
/// needs at least one target, which [`SessionBuilder::target`] adds.
///
/// The builder holds a handle to its global session, so it is neither
/// [`Send`] nor [`Sync`].
///
/// # Examples
///
/// See the example of [`Session`].
///
/// A session builder cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::SessionBuilder>();
/// ```
///
/// A session builder cannot be shared between threads either, so this
/// example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::SessionBuilder>();
/// ```
#[derive(Debug)]
pub struct SessionBuilder {
    global: GlobalSession,
    targets: Vec<Target>,
    search_paths: Vec<PathBuf>,
    macros: Vec<(String, String)>,
    matrix_layout: Option<MatrixLayout>,
    use_up_to_date_binary_module: Option<Toggle>,
    options: CompilerOptions,
}

impl SessionBuilder {
    /// Adds a target, a format of code that the session generates.
    ///
    /// The targets are numbered from 0 in the order of the calls, and
    /// [`Session::target_index`] selects one by that number.
    ///
    /// # Examples
    ///
    /// See the example of [`Session`].
    #[must_use]
    pub fn target(mut self, target: Target) -> Self {
        self.targets.push(target);
        self
    }

    /// Adds a search path, a directory in which Slang looks for the modules
    /// that source imports and the files that it includes.
    ///
    /// Slang looks for an imported module or an included file first relative
    /// to the file that imports or includes it, then in the search paths in
    /// the order of the calls. A relative search path is passed to Slang
    /// unchanged and resolves against the working directory of the process.
    /// Slang reads the file system of the operating system through these
    /// paths.
    ///
    /// # Examples
    ///
    /// This example adds a directory of shaders. It needs the Slang shared
    /// library at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .search_path("/tmp/shaders")
    ///     .build()?;
    /// # let _ = session;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn search_path(self, path: impl AsRef<Path>) -> Self {
        self.add_search_path(path.as_ref())
    }

    /// Defines a preprocessor macro for every module of the session.
    ///
    /// A preprocessor macro is a name that Slang's preprocessor replaces with
    /// a value, as in C, so `define("BLOCK_SIZE", "64")` acts like
    /// `#define BLOCK_SIZE 64` at the start of each source file.
    ///
    /// # Examples
    ///
    /// This example defines a macro. It needs the Slang shared library at
    /// run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .define("BLOCK_SIZE", "64")
    ///     .build()?;
    /// # let _ = session;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn define(self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.add_macro(name.into(), value.into())
    }

    /// Sets the default matrix layout of the session, the order in which the
    /// elements of a matrix are stored in memory, which is
    /// [`MatrixLayout::RowMajor`] by default.
    ///
    /// The session passes it to Slang as the field `defaultMatrixLayoutMode`
    /// of its descriptor. Slang uses it only when the session's options set
    /// no matrix layout of their own through a raw option.
    ///
    /// # Examples
    ///
    /// This example stores matrices column by column, as many graphics
    /// libraries do. It needs the Slang shared library at run time, so it is
    /// compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, MatrixLayout, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .matrix_layout(MatrixLayout::ColumnMajor)
    ///     .build()?;
    /// # let _ = session;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn matrix_layout(mut self, layout: MatrixLayout) -> Self {
        self.matrix_layout = Some(layout);
        self
    }

    /// Sets whether Slang checks that a precompiled module is up to date with
    /// its source before it loads it, as Slang's option
    /// `UseUpToDateBinaryModule`.
    ///
    /// A precompiled module is a file with the extension `.slang-module` that
    /// holds a module in Slang's intermediate representation. When the check
    /// is on, Slang compiles the source again instead of loading a module
    /// that is out of date. Slang reads this option only from the options of
    /// the session, so it is a setting of the session builder only. It is
    /// passed to Slang before the options of [`SessionBuilder::options`], and
    /// Slang uses the first value of the option, so this setting wins over
    /// a raw option of the same name.
    ///
    /// # Examples
    ///
    /// This example turns the check on. It needs the Slang shared library at
    /// run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat, Toggle};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .use_up_to_date_binary_module(Toggle::On)
    ///     .build()?;
    /// # let _ = session;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn use_up_to_date_binary_module(mut self, toggle: Toggle) -> Self {
        self.use_up_to_date_binary_module = Some(toggle);
        self
    }

    /// Sets the compiler options of the session, which every target starts
    /// from, and replaces any options set earlier.
    ///
    /// The options of a [`Target`] override these for that target. Slang
    /// takes the floating-point mode, the line directive mode, the scalar
    /// layout of buffers, and the method of generating SPIR-V of each target
    /// from fields of the target's descriptor rather than from the options it
    /// inherits, so [`SessionBuilder::build`] fills those fields from the
    /// options of the target when they set the option, and otherwise from
    /// these options.
    ///
    /// # Examples
    ///
    /// This example treats every warning as an error in every target. It
    /// needs the Slang shared library at run time, so it is compiled but not
    /// run:
    ///
    /// ```no_run
    /// use slang_compiler::{CompilerOptions, GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let mut options = CompilerOptions::default();
    /// options.warnings_as_errors("all");
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .options(options)
    ///     .build()?;
    /// # let _ = session;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn options(mut self, options: CompilerOptions) -> Self {
        self.options = options;
        self
    }

    /// Checks every setting and creates the session.
    ///
    /// The checks run before anything is created, so a setting that Slang
    /// would not accept is reported here, with the setting that caused it,
    /// instead of at a later call. They run in this order, and the first
    /// failure is returned: that the builder has a target, that no target's
    /// options set an option that Slang reads only from the session, the
    /// search paths, the macros, the options of the session, each target's
    /// options, profile, and capabilities in the order of the targets, and
    /// last the names of the profile and the capabilities of each target,
    /// which only Slang can resolve.
    ///
    /// # Examples
    ///
    /// See the example of [`Session`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind:
    ///
    /// - [`ErrorKind::InvalidArgument`] when the builder has no target, when a
    ///   search path is not valid UTF-8, when a search path, a macro name or
    ///   value, a profile, a capability, or a text value of the compiler
    ///   options contains a NUL byte, when there are more compiler options
    ///   than Slang takes, or when the options of a target set
    ///   `LanguageVersion`, `WarningsAsErrors`, `EnableWarning`, or
    ///   `DisableWarning`, which the pinned Slang build reads only from the
    ///   options of the session, as [`Target::options`] states.
    /// - [`ErrorKind::NotFound`] when Slang does not know the name of a
    ///   profile or a capability of a target.
    /// - The kind that Slang's result code maps to when Slang cannot create
    ///   the session, such as [`ErrorKind::Internal`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn build(self) -> Result<Session> {
        guard::call("SessionBuilder::build", || self.create())
    }

    fn add_search_path(mut self, path: &Path) -> Self {
        self.search_paths.push(path.to_path_buf());
        self
    }

    fn add_macro(mut self, name: String, value: String) -> Self {
        self.macros.push((name, value));
        self
    }

    /// Creates the session, with the Slang call guard raised by the caller.
    fn create(self) -> Result<Session> {
        let operation = Operation::CreateSession;
        if self.targets.is_empty() {
            return Err(Error::new(ErrorKind::InvalidArgument, operation)
                .with_detail("the builder has no target, and a session needs one".to_owned()));
        }
        for (position, target) in self.targets.iter().enumerate() {
            if let Some(name) = target.options.first_session_only() {
                return Err(
                    Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
                        "the options of target {position} set `{name}`, which the pinned Slang \
                     build reads only from the options of the session, so set it on the \
                     session instead"
                    )),
                );
            }
        }
        let search_paths = self
            .search_paths
            .iter()
            .map(|path| search_path(path))
            .collect::<Result<Vec<_>>>()?;
        let macros = self
            .macros
            .iter()
            .map(|(name, value)| {
                Ok((
                    c_string(name, operation, "the macro name")?,
                    c_string(value, operation, "the macro value")?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        let mut options = PreparedOptions::default();
        if let Some(toggle) = self.use_up_to_date_binary_module {
            options.push_int(
                CompilerOptionName::UseUpToDateBinaryModule,
                i32::from(toggle.is_on()),
            );
        }
        options.append(&self.options, operation)?;
        let targets = self
            .targets
            .iter()
            .map(|target| PendingTarget::new(target, &self.options))
            .collect::<Result<Vec<_>>>()?;
        let global = self.global.as_raw();
        let targets = targets
            .into_iter()
            .map(|target| target.resolve(global))
            .collect::<Result<Vec<_>>>()?;

        let target_options = targets
            .iter()
            .map(|target| target.options.raw(operation))
            .collect::<Result<Vec<_>>>()?;
        let target_descs = targets
            .iter()
            .zip(&target_options)
            .map(|(target, options)| target.desc(options))
            .collect::<Vec<_>>();
        let search_path_pointers = search_paths
            .iter()
            .map(|path| path.as_ptr())
            .collect::<Vec<*const c_char>>();
        let macro_descs = macros
            .iter()
            .map(|(name, value)| PreprocessorMacroDesc {
                name: name.as_ptr(),
                value: value.as_ptr(),
            })
            .collect::<Vec<_>>();
        let options = options.raw(operation)?;
        let desc = SessionDesc {
            targets: array_pointer(&target_descs),
            targetCount: count(target_descs.len(), "targets")?,
            defaultMatrixLayoutMode: self.matrix_layout.map_or(
                SessionDesc::default().defaultMatrixLayoutMode,
                MatrixLayout::to_raw,
            ),
            searchPaths: array_pointer(&search_path_pointers),
            searchPathCount: count(search_path_pointers.len(), "search paths")?,
            preprocessorMacros: array_pointer(&macro_descs),
            preprocessorMacroCount: count(macro_descs.len(), "macros")?,
            compilerOptionEntries: options.as_ptr(),
            compilerOptionEntryCount: options.count(),
            ..SessionDesc::default()
        };
        let mut raw: *mut ISession = ptr::null_mut();
        // SAFETY: `global` points to a live global session, which `self.global`
        // holds (Section 11.1 of `docs/design/slang-compiler.md`), used only on
        // this thread (Section 11.2), and the call runs with the Slang call
        // guard raised (SLC-28). `desc` is a live descriptor whose size field
        // holds its own size, which `Default` sets, as does each target
        // descriptor. Every pointer in it stays valid until the call returns,
        // which is all that Slang needs, because it copies the strings and
        // arrays during the call (Section 3.2): `target_descs`,
        // `search_path_pointers`, and `macro_descs` own the arrays, the C
        // strings of `search_paths` and `macros` own the strings that they
        // point to, `options` and each element of `target_options` own the
        // entry arrays, and the prepared options of the session and of
        // `targets` own the strings of the entries, which the borrows of the
        // raw options keep alive. Each count is the length of its array, and
        // `raw` is a live out-pointer.
        let result = unsafe {
            guard::abort_on_exception("IGlobalSession::createSession", || {
                IGlobalSession::createSession(global.as_ptr(), &raw const desc, &raw mut raw)
            })
        };
        mapping::check(operation, result, Diagnostics::default())?;
        let raw =
            NonNull::new(raw).ok_or_else(|| mapping::null(operation, Diagnostics::default()))?;
        // SAFETY: `createSession` succeeded and wrote a non-null pointer, which
        // is a new reference to a live session of the pinned build (Section
        // 5.3 of `docs/design/slang-compiler.md`), and only this thread has it.
        let raw = unsafe { Owned::adopt(raw) };
        Ok(Session {
            raw,
            global: self.global,
            target_count: targets.len(),
        })
    }
}

/// A target with its text converted to C strings, before Slang resolves its
/// names.
struct PendingTarget {
    format: TargetFormat,
    profile: Option<CString>,
    capabilities: Vec<CString>,
    options: PreparedOptions,
    fields: TargetFields,
}

/// The settings that Slang's `addTarget` sets on each target from fields of
/// the target descriptor, after the target inherited the session's options,
/// each taken from the options of the target when they set it, otherwise
/// from the options of the session, so that a setting of the session reaches
/// every target that does not set its own. `None` keeps the field's default.
struct TargetFields {
    floating_point_mode: Option<i32>,
    line_directive_mode: Option<i32>,
    glsl_force_scalar_layout: Option<i32>,
    spirv_via_glsl: bool,
}

impl TargetFields {
    fn new(target: &CompilerOptions, session: &CompilerOptions) -> Self {
        let effective = |name| target.first_int(name).or_else(|| session.first_int(name));
        let method = target.spirv_method().or_else(|| session.spirv_method());
        Self {
            floating_point_mode: effective(CompilerOptionName::FloatingPointMode),
            line_directive_mode: effective(CompilerOptionName::LineDirectiveMode),
            glsl_force_scalar_layout: effective(CompilerOptionName::GLSLForceScalarLayout),
            spirv_via_glsl: method
                == Some(
                    SlangEmitSpirvMethod::SLANG_EMIT_SPIRV_VIA_GLSL
                        .0
                        .cast_signed(),
                ),
        }
    }
}

/// A target whose profile and capabilities Slang resolved.
struct ResolvedTarget {
    format: TargetFormat,
    profile: SlangProfileID,
    options: PreparedOptions,
    fields: TargetFields,
}

impl PendingTarget {
    fn new(target: &Target, session: &CompilerOptions) -> Result<Self> {
        let operation = Operation::CreateSession;
        let mut options = PreparedOptions::default();
        options.append(&target.options, operation)?;
        Ok(Self {
            format: target.format,
            profile: target
                .profile
                .as_deref()
                .map(|profile| c_string(profile, operation, "the profile"))
                .transpose()?,
            capabilities: target
                .capabilities
                .iter()
                .map(|capability| c_string(capability, operation, "the capability"))
                .collect::<Result<Vec<_>>>()?,
            options,
            fields: TargetFields::new(&target.options, session),
        })
    }

    /// Resolves the names of the profile and the capabilities through
    /// `global`, with the Slang call guard raised by the caller.
    fn resolve(self, global: NonNull<IGlobalSession>) -> Result<ResolvedTarget> {
        let Self {
            format,
            profile,
            capabilities,
            mut options,
            fields,
        } = self;
        let profile = match profile {
            None => SlangProfileID::SLANG_PROFILE_UNKNOWN,
            Some(name) => {
                // SAFETY: `global` points to a live global session that the
                // builder holds, used only on this thread, `name` is a C string
                // that lives until the call returns, and the call runs with the
                // Slang call guard raised (SLC-28).
                let id = unsafe {
                    guard::abort_on_exception("IGlobalSession::findProfile", || {
                        IGlobalSession::findProfile(global.as_ptr(), name.as_ptr())
                    })
                };
                if id == SlangProfileID::SLANG_PROFILE_UNKNOWN {
                    return Err(unknown_name("profile", &name));
                }
                id
            }
        };
        for name in capabilities {
            // SAFETY: as for `findProfile` above, with the C string `name`.
            let id = unsafe {
                guard::abort_on_exception("IGlobalSession::findCapability", || {
                    IGlobalSession::findCapability(global.as_ptr(), name.as_ptr())
                })
            };
            if id == slang_compiler_sys::SlangCapabilityID::SLANG_CAPABILITY_UNKNOWN {
                return Err(unknown_name("capability", &name));
            }
            options.push_int(CompilerOptionName::Capability, id.0);
        }
        Ok(ResolvedTarget {
            format,
            profile,
            options,
            fields,
        })
    }
}

impl ResolvedTarget {
    fn desc(&self, options: &RawOptions<'_>) -> TargetDesc {
        let default = TargetDesc::default();
        let fields = &self.fields;
        TargetDesc {
            format: self.format.to_raw(),
            profile: self.profile,
            flags: if fields.spirv_via_glsl {
                default.flags & !SLANG_TARGET_FLAG_GENERATE_SPIRV_DIRECTLY
            } else {
                default.flags
            },
            floatingPointMode: fields
                .floating_point_mode
                .map_or(default.floatingPointMode, |mode| {
                    SlangFloatingPointMode(mode.cast_unsigned())
                }),
            lineDirectiveMode: fields
                .line_directive_mode
                .map_or(default.lineDirectiveMode, |mode| {
                    SlangLineDirectiveMode(mode.cast_unsigned())
                }),
            forceGLSLScalarBufferLayout: fields
                .glsl_force_scalar_layout
                .map_or(default.forceGLSLScalarBufferLayout, |value| value != 0),
            compilerOptionEntries: options.as_ptr(),
            compilerOptionEntryCount: options.count(),
            ..default
        }
    }
}

fn unknown_name(what: &str, name: &CString) -> Error {
    Error::new(ErrorKind::NotFound, Operation::CreateSession).with_detail(format!(
        "Slang does not know the {what} `{}`",
        name.to_string_lossy()
    ))
}

fn search_path(path: &Path) -> Result<CString> {
    let operation = Operation::CreateSession;
    let text = path.to_str().ok_or_else(|| {
        Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
            "the search path {} is not valid UTF-8, which Slang needs",
            path.display()
        ))
    })?;
    c_string(text, operation, "the search path")
}

/// Returns the pointer to the first element of `array`, or null when it is
/// empty.
fn array_pointer<T>(array: &[T]) -> *const T {
    if array.is_empty() {
        ptr::null()
    } else {
        array.as_ptr()
    }
}

/// Converts the length of an array of the descriptor into Slang's count.
fn count(length: usize, what: &str) -> Result<isize> {
    isize::try_from(length).map_err(|error| {
        Error::new(ErrorKind::InvalidArgument, Operation::CreateSession).with_detail(format!(
            "{length} {what} are more than Slang takes: {error}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::path::PathBuf;

    use slang_compiler_sys::{
        CompilerOptionName, ISession, SLANG_TARGET_FLAG_GENERATE_SPIRV_DIRECTLY,
        SlangEmitSpirvMethod, SlangResult, TargetDesc,
    };

    use super::{Session, SessionBuilder, Target};
    use crate::fake::{
        Event, FAKE_CAPABILITY, FAKE_PROFILE, FAKE_SESSION, FakeGlobalSession, Log, OptionSnapshot,
        SessionSnapshot, TargetSnapshot, panic_message,
    };
    use crate::{
        CompilerOptions, ErrorKind, FloatingPointMode, GlobalSession, LanguageVersion,
        LineDirectiveMode, MatrixLayout, Operation, OptimizationLevel, OptionName, TargetFormat,
        Toggle, guard,
    };

    fn adopt(log: &Log) -> GlobalSession {
        // SAFETY: the new fake global session carries one reference, it
        // reports the pinned build tag, and the test uses it on this thread
        // only.
        unsafe {
            GlobalSession::from_raw(FakeGlobalSession::create(
                log,
                "global",
                SlangResult::SLANG_OK,
            ))
        }
    }

    fn spirv() -> Target {
        Target::new(TargetFormat::Spirv)
    }

    fn created(log: &Log) -> Vec<SessionSnapshot> {
        log.events()
            .into_iter()
            .filter_map(|event| match event {
                Event::CreateSession { desc, .. } => Some(desc),
                _ => None,
            })
            .collect()
    }

    /// Returns the calls on the fake objects other than the counting calls.
    fn calls(log: &Log) -> Vec<Event> {
        log.events()
            .into_iter()
            .filter(|event| {
                !matches!(
                    event,
                    Event::AddRef { .. } | Event::Release { .. } | Event::Freed { .. }
                )
            })
            .collect()
    }

    fn default_target(format: i32, profile: u32, options: Vec<OptionSnapshot>) -> TargetSnapshot {
        let default = TargetDesc::default();
        TargetSnapshot {
            structure_size: size_of::<TargetDesc>(),
            format,
            profile,
            flags: default.flags,
            floating_point_mode: default.floatingPointMode.0,
            line_directive_mode: default.lineDirectiveMode.0,
            force_glsl_scalar_layout: default.forceGLSLScalarBufferLayout,
            options,
        }
    }

    fn assert_invalid(builder: SessionBuilder, log: &Log, text: &str) {
        let error = builder.build().expect_err("the setting is rejected");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{error}");
        assert_eq!(error.operation(), Operation::CreateSession);
        assert_eq!(error.code(), None);
        assert!(error.to_string().contains(text), "{error}");
        assert_eq!(calls(log), []);
    }

    #[test]
    fn build_passes_every_setting_to_slang_in_the_descriptor() {
        let log = Log::new();
        let global = adopt(&log);
        let mut session_options = CompilerOptions::default();
        session_options.warnings_as_errors("all");
        let mut target_options = CompilerOptions::default();
        target_options.optimization(OptimizationLevel::High);
        let session = Session::builder(&global)
            .target(
                spirv()
                    .profile(FAKE_PROFILE.0)
                    .options(target_options)
                    .capability(FAKE_CAPABILITY.0),
            )
            .target(Target::new(TargetFormat::Wgsl))
            .search_path("/tmp/shaders")
            .search_path(PathBuf::from("relative"))
            .define("SIZE", "64")
            .define("EMPTY", "")
            .matrix_layout(MatrixLayout::ColumnMajor)
            .use_up_to_date_binary_module(Toggle::On)
            .options(session_options)
            .build()
            .expect("the fake creates the session");
        assert_eq!(session.target_count(), 2);
        assert_eq!(
            created(&log),
            [SessionSnapshot {
                structure_size: size_of::<slang_compiler_sys::SessionDesc>(),
                targets: vec![
                    default_target(
                        TargetFormat::Spirv.to_raw().0,
                        FAKE_PROFILE.1,
                        vec![
                            OptionSnapshot::Int(CompilerOptionName::Optimization.0, 2, 0),
                            OptionSnapshot::Int(
                                CompilerOptionName::Capability.0,
                                FAKE_CAPABILITY.1,
                                0
                            ),
                        ],
                    ),
                    default_target(TargetFormat::Wgsl.to_raw().0, 0, vec![]),
                ],
                flags: 0,
                matrix_layout: MatrixLayout::ColumnMajor.to_raw().0,
                search_paths: vec!["/tmp/shaders".to_owned(), "relative".to_owned()],
                macros: vec![
                    ("SIZE".to_owned(), "64".to_owned()),
                    ("EMPTY".to_owned(), String::new()),
                ],
                has_file_system: false,
                enable_effect_annotations: false,
                allow_glsl_syntax: false,
                skip_spirv_validation: false,
                options: vec![
                    OptionSnapshot::Int(CompilerOptionName::UseUpToDateBinaryModule.0, 1, 0),
                    OptionSnapshot::Text(
                        CompilerOptionName::WarningsAsErrors.0,
                        "all".to_owned(),
                        String::new()
                    ),
                ],
            }]
        );
        let calls = calls(&log);
        assert_eq!(
            calls.get(..2),
            Some(
                [
                    Event::find_profile("global", FAKE_PROFILE.0),
                    Event::find_capability("global", FAKE_CAPABILITY.0),
                ]
                .as_slice()
            )
        );
        assert_eq!(calls.len(), 3);
    }

    /// Builds a session with `session` as its options and one target with
    /// `target` as its options, and returns the target descriptor that the
    /// fake received.
    fn target_desc(session: &CompilerOptions, target: &CompilerOptions) -> TargetSnapshot {
        let log = Log::new();
        let global = adopt(&log);
        Session::builder(&global)
            .target(spirv().options(target.clone()))
            .options(session.clone())
            .build()
            .expect("the fake creates the session");
        let mut snapshots = created(&log);
        assert_eq!(snapshots.len(), 1);
        let mut targets = snapshots.remove(0).targets;
        assert_eq!(targets.len(), 1);
        targets.remove(0)
    }

    /// Checks one setting that Slang takes from a field of the target
    /// descriptor: set on the session only, on the target only, on both with
    /// different values, and on neither. `field` reads the field from the
    /// descriptor, and the session's value is `a`, the target's `b`.
    fn assert_field_follows_the_effective_option<T: PartialEq + std::fmt::Debug>(
        set_a: impl Fn(&mut CompilerOptions),
        set_b: impl Fn(&mut CompilerOptions),
        field: impl Fn(&TargetSnapshot) -> T,
        (a, b, default): (T, T, T),
    ) {
        let none = CompilerOptions::default();
        let mut session = CompilerOptions::default();
        set_a(&mut session);
        let mut target = CompilerOptions::default();
        set_b(&mut target);
        assert_eq!(field(&target_desc(&session, &none)), a, "session only");
        assert_eq!(field(&target_desc(&none, &target)), b, "target only");
        assert_eq!(field(&target_desc(&session, &target)), b, "both");
        assert_eq!(field(&target_desc(&none, &none)), default, "neither");
    }

    #[test]
    fn the_floating_point_mode_of_the_session_reaches_the_target_descriptor() {
        assert_field_follows_the_effective_option(
            |options| {
                options.floating_point_mode(FloatingPointMode::Fast);
            },
            |options| {
                options.floating_point_mode(FloatingPointMode::Precise);
            },
            |desc| desc.floating_point_mode,
            (
                FloatingPointMode::Fast.to_raw().0,
                FloatingPointMode::Precise.to_raw().0,
                TargetDesc::default().floatingPointMode.0,
            ),
        );
    }

    #[test]
    fn the_line_directive_mode_of_the_session_reaches_the_target_descriptor() {
        assert_field_follows_the_effective_option(
            |options| {
                options.line_directive_mode(LineDirectiveMode::None);
            },
            |options| {
                options.line_directive_mode(LineDirectiveMode::Standard);
            },
            |desc| desc.line_directive_mode,
            (
                LineDirectiveMode::None.to_raw().0,
                LineDirectiveMode::Standard.to_raw().0,
                TargetDesc::default().lineDirectiveMode.0,
            ),
        );
    }

    #[test]
    fn the_scalar_layout_of_the_session_reaches_the_target_descriptor() {
        assert_field_follows_the_effective_option(
            |options| {
                options.glsl_force_scalar_layout(Toggle::On);
            },
            |options| {
                options.glsl_force_scalar_layout(Toggle::Off);
            },
            |desc| desc.force_glsl_scalar_layout,
            (
                true,
                false,
                TargetDesc::default().forceGLSLScalarBufferLayout,
            ),
        );
        assert_field_follows_the_effective_option(
            |options| {
                options.glsl_force_scalar_layout(Toggle::Off);
            },
            |options| {
                options.glsl_force_scalar_layout(Toggle::On);
            },
            |desc| desc.force_glsl_scalar_layout,
            (false, true, false),
        );
    }

    #[test]
    fn the_spirv_method_of_the_session_reaches_the_target_flags() {
        let default = TargetDesc::default().flags;
        let via_glsl = default & !SLANG_TARGET_FLAG_GENERATE_SPIRV_DIRECTLY;
        assert_ne!(default, via_glsl);
        assert_field_follows_the_effective_option(
            |options| {
                options.emit_spirv_directly(Toggle::Off);
            },
            |options| {
                options.emit_spirv_directly(Toggle::On);
            },
            |desc| desc.flags,
            (via_glsl, default, default),
        );
        assert_field_follows_the_effective_option(
            |options| {
                options.emit_spirv_directly(Toggle::On);
            },
            |options| {
                options.emit_spirv_directly(Toggle::Off);
            },
            |desc| desc.flags,
            (default, via_glsl, default),
        );
    }

    #[test]
    fn the_target_fields_follow_raw_entries_as_slang_reads_them() {
        let int = |name: CompilerOptionName, value: i32| {
            let mut options = CompilerOptions::default();
            let name = OptionName::new(name.0).expect("a stored option");
            // SAFETY: the options are only encoded and passed to the fake,
            // which reads but never interprets them.
            unsafe { options.set_int(name, value, 0) };
            options
        };
        let none = CompilerOptions::default();
        let default = TargetDesc::default();
        let via_glsl = default.flags & !SLANG_TARGET_FLAG_GENERATE_SPIRV_DIRECTLY;
        let mut first_wins = int(CompilerOptionName::FloatingPointMode, 1);
        // SAFETY: as above.
        unsafe {
            first_wins.set_int(
                OptionName::new(CompilerOptionName::FloatingPointMode.0).expect("stored"),
                2,
                0,
            );
        }
        assert_eq!(target_desc(&first_wins, &none).floating_point_mode, 1);
        let glsl = int(CompilerOptionName::EmitSpirvViaGLSL, 1);
        assert_eq!(target_desc(&glsl, &none).flags, via_glsl);
        let method = int(
            CompilerOptionName::EmitSpirvMethod,
            SlangEmitSpirvMethod::SLANG_EMIT_SPIRV_VIA_GLSL
                .0
                .cast_signed(),
        );
        assert_eq!(target_desc(&none, &method).flags, via_glsl);
        let mut directly_then_glsl = int(CompilerOptionName::EmitSpirvDirectly, 1);
        // SAFETY: as above.
        unsafe {
            directly_then_glsl.set_int(
                OptionName::new(CompilerOptionName::EmitSpirvViaGLSL.0).expect("stored"),
                1,
                0,
            );
        }
        assert_eq!(target_desc(&directly_then_glsl, &none).flags, default.flags);
    }

    #[test]
    fn build_keeps_slang_defaults_for_unset_settings() {
        let log = Log::new();
        let global = adopt(&log);
        let session = Session::builder(&global)
            .target(spirv())
            .build()
            .expect("the fake creates the session");
        let default = slang_compiler_sys::SessionDesc::default();
        let snapshots = created(&log);
        let snapshot = snapshots.first().expect("one session was created");
        assert_eq!(snapshot.matrix_layout, default.defaultMatrixLayoutMode.0);
        assert_eq!(snapshot.flags, default.flags);
        assert_eq!(snapshot.search_paths, Vec::<String>::new());
        assert_eq!(snapshot.macros, []);
        assert_eq!(snapshot.options, []);
        assert_eq!(
            snapshot.targets,
            [default_target(TargetFormat::Spirv.to_raw().0, 0, vec![])]
        );
        assert!(
            !log.events()
                .iter()
                .any(|event| matches!(event, Event::FindProfile { .. }))
        );
        drop(session);
    }

    #[test]
    fn the_session_holds_its_global_session_and_is_released_first() {
        let log = Log::new();
        let global = adopt(&log);
        let session = Session::builder(&global)
            .target(spirv())
            .build()
            .expect("the fake creates the session");
        drop(global);
        assert_eq!(
            session.global_session().build_tag(),
            slang_compiler_sys::SLANG_BUILD_TAG
        );
        let before = log.events().len();
        drop(session);
        assert_eq!(
            log.events().split_off(before),
            [
                Event::release(FAKE_SESSION, 0),
                Event::freed(FAKE_SESSION),
                Event::release("global", 1),
                Event::release("global", 0),
                Event::freed("global"),
            ]
        );
    }

    #[test]
    fn build_without_a_target_fails_without_calling_slang() {
        let log = Log::new();
        let global = adopt(&log);
        assert_invalid(Session::builder(&global), &log, "no target");
    }

    #[test]
    fn build_rejects_a_nul_byte_in_every_string_without_calling_slang() {
        let log = Log::new();
        let global = adopt(&log);
        let mut options = CompilerOptions::default();
        options.disable_warning("a\0b");
        let mut target_options = CompilerOptions::default();
        let define = OptionName::new(CompilerOptionName::MacroDefine.0).expect("a stored option");
        // SAFETY: Slang reads `MacroDefine` as two strings. The NUL byte stops
        // `build` before the options reach Slang.
        unsafe { target_options.set_string(define, "A", Some("1\0")) };
        let builders = [
            (
                Session::builder(&global)
                    .target(spirv())
                    .search_path("a\0b"),
                "the search path",
            ),
            (
                Session::builder(&global)
                    .target(spirv())
                    .define("A\0B", "1"),
                "the macro name",
            ),
            (
                Session::builder(&global).target(spirv()).define("A", "1\0"),
                "the macro value",
            ),
            (
                Session::builder(&global).target(spirv().profile("spirv\0")),
                "the profile",
            ),
            (
                Session::builder(&global).target(spirv().capability("\0")),
                "the capability",
            ),
            (
                Session::builder(&global)
                    .target(spirv())
                    .options(options.clone()),
                "compiler option 22",
            ),
            (
                Session::builder(&global).target(spirv().options(target_options)),
                "compiler option 0",
            ),
        ];
        for (builder, text) in builders {
            assert_invalid(builder, &log, text);
        }
    }

    /// Returns options with an optimization level and then `name`, set once
    /// through `typed` and once through a raw setter with the value `raw`.
    fn session_only_cases(
        typed: impl Fn(&mut CompilerOptions),
        name: CompilerOptionName,
        raw: Result<i32, &str>,
    ) -> [CompilerOptions; 2] {
        let mut typed_options = CompilerOptions::default();
        typed_options.optimization(OptimizationLevel::High);
        typed(&mut typed_options);
        let mut raw_options = CompilerOptions::default();
        raw_options.optimization(OptimizationLevel::High);
        let name = OptionName::new(name.0).expect("a stored option");
        match raw {
            // SAFETY: the options reach only the fake, which reads but never
            // interprets them, and `build` rejects them on a target.
            Ok(value) => unsafe {
                raw_options.set_int(name, value, 0);
            },
            // SAFETY: as above.
            Err(text) => unsafe {
                raw_options.set_string(name, text, None);
            },
        }
        [typed_options, raw_options]
    }

    /// Checks that `build` rejects each of `cases` in the options of the
    /// second target with an error that names `name`, before it calls Slang,
    /// and that a session accepts the same options.
    fn assert_session_only(cases: [CompilerOptions; 2], name: &str) {
        for options in cases {
            let log = Log::new();
            let global = adopt(&log);
            let error = Session::builder(&global)
                .target(Target::new(TargetFormat::Hlsl))
                .target(spirv().profile(FAKE_PROFILE.0).options(options.clone()))
                .build()
                .expect_err("the target sets an option that Slang reads from the session");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{error}");
            assert_eq!(error.operation(), Operation::CreateSession);
            assert_eq!(error.code(), None);
            assert_eq!(
                error.to_string(),
                format!(
                    "cannot create a session: the options of target 1 set `{name}`, which the \
                     pinned Slang build reads only from the options of the session, so set it \
                     on the session instead"
                )
            );
            assert_eq!(calls(&log), []);
            drop(global);

            let log = Log::new();
            let global = adopt(&log);
            Session::builder(&global)
                .target(spirv())
                .options(options)
                .build()
                .expect("the session takes the option");
            assert_eq!(created(&log).len(), 1);
        }
    }

    #[test]
    fn build_rejects_a_language_version_in_the_options_of_a_target() {
        let cases = session_only_cases(
            |options| {
                options.language_version(LanguageVersion::V2026);
            },
            CompilerOptionName::LanguageVersion,
            Ok(2026),
        );
        assert_session_only(cases, "LanguageVersion");
    }

    #[test]
    fn build_rejects_warnings_as_errors_in_the_options_of_a_target() {
        let cases = session_only_cases(
            |options| {
                options.warnings_as_errors("all");
            },
            CompilerOptionName::WarningsAsErrors,
            Err("all"),
        );
        assert_session_only(cases, "WarningsAsErrors");
    }

    #[test]
    fn build_rejects_an_enabled_warning_in_the_options_of_a_target() {
        let cases = session_only_cases(
            |options| {
                options.enable_warning("30081");
            },
            CompilerOptionName::EnableWarning,
            Err("30081"),
        );
        assert_session_only(cases, "EnableWarning");
    }

    #[test]
    fn build_rejects_a_disabled_warning_in_the_options_of_a_target() {
        let cases = session_only_cases(
            |options| {
                options.disable_warning("30081");
            },
            CompilerOptionName::DisableWarning,
            Err("30081"),
        );
        assert_session_only(cases, "DisableWarning");
    }

    #[test]
    fn the_session_takes_every_option_that_a_target_rejects() {
        let log = Log::new();
        let global = adopt(&log);
        let mut options = CompilerOptions::default();
        options
            .language_version(LanguageVersion::V2026)
            .warnings_as_errors("all")
            .enable_warning("30081")
            .disable_warning("30081");
        Session::builder(&global)
            .target(spirv())
            .options(options)
            .build()
            .expect("the session takes every option");
        let snapshots = created(&log);
        let snapshot = snapshots.first().expect("one session was created");
        assert_eq!(
            snapshot
                .options
                .iter()
                .map(|option| match option {
                    OptionSnapshot::Int(name, ..) | OptionSnapshot::Text(name, ..) => *name,
                })
                .collect::<Vec<_>>(),
            [
                CompilerOptionName::LanguageVersion.0,
                CompilerOptionName::WarningsAsErrors.0,
                CompilerOptionName::EnableWarning.0,
                CompilerOptionName::DisableWarning.0,
            ]
        );
    }

    #[cfg(unix)]
    #[test]
    fn build_rejects_a_search_path_that_is_not_utf8() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let log = Log::new();
        let global = adopt(&log);
        let path = OsStr::from_bytes(b"shaders\xff");
        let builder = Session::builder(&global).target(spirv()).search_path(path);
        assert_invalid(builder, &log, "not valid UTF-8");
    }

    #[test]
    fn build_rejects_an_unknown_profile_with_not_found_before_creating() {
        let log = Log::new();
        let global = adopt(&log);
        let error = Session::builder(&global)
            .target(spirv().profile("spirv_9_9"))
            .build()
            .expect_err("the fake knows no such profile");
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(error.operation(), Operation::CreateSession);
        assert_eq!(
            error.to_string(),
            "cannot create a session: Slang does not know the profile `spirv_9_9`"
        );
        assert_eq!(calls(&log), [Event::find_profile("global", "spirv_9_9")]);
    }

    #[test]
    fn build_rejects_an_unknown_capability_with_not_found_before_creating() {
        let log = Log::new();
        let global = adopt(&log);
        let error = Session::builder(&global)
            .target(spirv().capability(FAKE_CAPABILITY.0).capability("teleport"))
            .build()
            .expect_err("the fake knows no such capability");
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert!(
            error.to_string().contains("capability `teleport`"),
            "{error}"
        );
        assert_eq!(
            calls(&log),
            [
                Event::find_capability("global", FAKE_CAPABILITY.0),
                Event::find_capability("global", "teleport"),
            ]
        );
    }

    #[test]
    fn build_maps_a_failing_result_code_and_a_null_session() {
        let log = Log::new();
        let global = adopt(&log);
        FakeGlobalSession::fail_create_session(global.as_raw(), SlangResult::SLANG_FAIL);
        let error = Session::builder(&global)
            .target(spirv())
            .build()
            .expect_err("the fake fails");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.code(), Some(SlangResult::SLANG_FAIL.0));
        FakeGlobalSession::fail_create_session(global.as_raw(), SlangResult::SLANG_OK);
        let error = Session::builder(&global)
            .target(spirv())
            .build()
            .expect_err("the fake writes no session");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.code(), None);
        assert_eq!(created(&log).len(), 2);
    }

    #[test]
    fn target_index_accepts_each_position_below_the_count() {
        let log = Log::new();
        let global = adopt(&log);
        let session = Session::builder(&global)
            .target(spirv())
            .target(Target::new(TargetFormat::Hlsl))
            .build()
            .expect("the fake creates the session");
        for position in 0..2 {
            let index = session.target_index(position).expect("in range");
            assert_eq!(index.position, position);
        }
        for position in [2, usize::MAX] {
            let error = session.target_index(position).expect_err("past the end");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert_eq!(error.operation(), Operation::SelectTarget);
        }
        assert_eq!(
            session.target_index(2).map_err(|error| error.to_string()),
            Err("cannot select target `2`: the session has 2 targets, numbered from 0".to_owned())
        );
    }

    #[test]
    fn build_while_the_guard_is_raised_panics_without_calling_slang() {
        let log = Log::new();
        let global = adopt(&log);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            guard::call("Test::callback", || {
                Session::builder(&global).target(spirv()).build()
            })
        }));
        let message = panic_message(outcome.expect_err("the nested call panics").as_ref());
        assert!(
            message.starts_with("reentrant call: SessionBuilder::build "),
            "{message}"
        );
        assert_eq!(calls(&log), []);
    }

    #[test]
    fn a_session_dropped_while_the_guard_is_raised_is_released_when_it_falls() {
        let log = Log::new();
        let global = adopt(&log);
        let session = Session::builder(&global)
            .target(spirv())
            .build()
            .expect("the fake creates the session");
        drop(global);
        let before = log.events().len();
        guard::call("Test::callback", || {
            drop(session);
            assert_eq!(log.events().len(), before);
        });
        assert_eq!(
            log.events().split_off(before),
            [
                Event::release(FAKE_SESSION, 0),
                Event::freed(FAKE_SESSION),
                Event::release("global", 1),
                Event::release("global", 0),
                Event::freed("global"),
            ]
        );
    }

    #[test]
    fn into_raw_and_from_raw_hand_the_session_over() {
        let log = Log::new();
        let global = adopt(&log);
        let session = Session::builder(&global)
            .target(spirv())
            .target(spirv())
            .build()
            .expect("the fake creates the session");
        let before = log.events().len();
        let (raw, parent) = session.into_raw();
        assert_eq!(log.events().split_off(before), []);
        assert_eq!(parent.as_raw(), global.as_raw());
        // SAFETY: `raw` is a live fake session, as `ISession::getGlobalSession`
        // requires, and the test owns a reference to it.
        let raw_global = unsafe { ISession::getGlobalSession(raw.as_ptr()) };
        assert_eq!(raw_global, global.as_raw().as_ptr());
        // SAFETY: `raw` carries the one reference that `into_raw` gave up, of a
        // fake session that `parent` created with two targets.
        let session = unsafe { Session::from_raw(raw, parent, 2) };
        assert_eq!(session.as_raw(), raw);
        assert_eq!(session.target_count(), 2);
        assert!(session.target_index(1).is_ok());
    }

    #[test]
    fn into_raw_returns_the_handle_that_keeps_the_global_session_alive() {
        let log = Log::new();
        let global = adopt(&log);
        let (raw, parent) = Session::builder(&global)
            .target(spirv())
            .build()
            .expect("the fake creates the session")
            .into_raw();
        drop(global);
        let before = log.events().len();
        // SAFETY: `raw` carries the one reference that `into_raw` gave up, of a
        // live fake session, and `parent` keeps its global session alive past
        // this release, as `into_raw` requires.
        unsafe { ISession::release(raw.as_ptr()) };
        drop(parent);
        assert_eq!(
            log.events().split_off(before),
            [
                Event::release(FAKE_SESSION, 0),
                Event::freed(FAKE_SESSION),
                Event::release("global", 1),
                Event::release("global", 0),
                Event::freed("global"),
            ]
        );
    }

    #[test]
    fn clones_share_one_session_and_the_last_drop_releases_it() {
        let log = Log::new();
        let global = adopt(&log);
        let session = Session::builder(&global)
            .target(spirv())
            .build()
            .expect("the fake creates the session");
        let clone = session.clone();
        assert_eq!(session.as_raw(), clone.as_raw());
        let before = log.events().len();
        drop(session);
        assert!(
            !log.events()
                .split_off(before)
                .contains(&Event::freed(FAKE_SESSION))
        );
        drop(clone);
        assert!(log.events().contains(&Event::freed(FAKE_SESSION)));
    }
}
