use std::ffi::CStr;
use std::fmt;
use std::ptr::{self, NonNull};

use slang_compiler_sys::{
    IGlobalSession, SLANG_BUILD_TAG, SlangGlobalSessionDesc, SlangResult,
    slang_createGlobalSession2, spGetBuildTagString,
};

use crate::enums::{LanguageVersion, TargetFormat, Toggle};
use crate::error::{Diagnostics, Error, ErrorKind, Operation, Result};
use crate::guard;
use crate::mapping;
use crate::owned::Owned;

/// The root object of Slang, from which every compilation starts.
///
/// Slang is a compiler for shaders, the programs that run on a graphics
/// processing unit (GPU). A global session holds what every compilation
/// shares, above all the core module, Slang's module of built-in types and
/// functions, which Slang loads when it creates the global session. Creating
/// one is slow, so a program creates one global session and keeps it. Use it
/// to check which target formats this installation of Slang can produce,
/// with [`GlobalSession::check_target_support`].
///
/// A global session is neither [`Send`] nor [`Sync`], so it stays on the
/// thread that created it. Slang documents a global session and every object
/// created from it as not safe to use from several threads at once.
/// [`Clone`] returns another handle to the same global session, and the
/// global session lives until its last handle is dropped.
///
/// # Examples
///
/// This example creates a global session and prints the version of the Slang
/// library that the program loaded. It needs the Slang shared library at run
/// time, so it is compiled but not run:
///
/// ```no_run
/// use slang_compiler::GlobalSession;
///
/// let global = GlobalSession::new()?;
/// println!("Slang {}", global.build_tag());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// A global session cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::GlobalSession>();
/// ```
///
/// A global session cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::GlobalSession>();
/// ```
#[derive(Clone)]
pub struct GlobalSession {
    raw: Owned<IGlobalSession>,
}

impl GlobalSession {
    /// Creates a global session with Slang's default settings.
    ///
    /// Use this unless you need a setting of [`GlobalSessionBuilder`]. The
    /// call loads Slang's core module, which takes a noticeable time, so keep
    /// the global session for every compilation of the thread.
    ///
    /// # Examples
    ///
    /// See the example of [`GlobalSession`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind:
    ///
    /// - [`ErrorKind::VersionMismatch`] when the Slang library that the
    ///   program loaded is not the build that this crate was built for. The
    ///   error names both builds.
    /// - The kind that Slang's result code maps to when Slang cannot create
    ///   the global session, such as [`ErrorKind::Internal`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn new() -> Result<Self> {
        Self::create(&GlobalSessionBuilder::default(), "GlobalSession::new")
    }

    /// Returns a builder for a global session with settings other than
    /// Slang's defaults.
    ///
    /// # Examples
    ///
    /// See the example of [`GlobalSessionBuilder`].
    #[must_use]
    pub fn builder() -> GlobalSessionBuilder {
        GlobalSessionBuilder::default()
    }

    /// Returns the build tag of the Slang library, which is the version
    /// string of that Slang build, such as `2026.17-25-gc7954ebd1`.
    ///
    /// Use it to report which Slang a program runs with. Creating the global
    /// session checked that the loaded library has the build tag that this
    /// crate was built for, so the value is always that one, and reading it
    /// makes no call into Slang.
    ///
    /// # Examples
    ///
    /// See the example of [`GlobalSession`].
    #[must_use]
    #[expect(
        clippy::unused_self,
        reason = "creation checked the build tag against the pin, so every global session has the same tag, and the accessor stays a method so that the tag is read from a global session that passed that check"
    )]
    pub fn build_tag(&self) -> &str {
        SLANG_BUILD_TAG
    }

    /// Checks whether Slang reports that it can produce code in a target
    /// format.
    ///
    /// Some target formats need a downstream compiler, a separate program or
    /// library that Slang calls to produce or check that format, such as the
    /// glslang library that validates SPIR-V. Use this check to find a
    /// missing one before a compilation fails. A successful check does not
    /// guarantee that code generation succeeds, because Slang can report
    /// support that it then lacks, as it does for some formats that need a C++
    /// compiler.
    ///
    /// # Examples
    ///
    /// This example checks that Slang can produce SPIR-V, the shader format
    /// of Vulkan. It needs the Slang shared library at run time, so it is
    /// compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{ErrorKind, GlobalSession, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// match global.check_target_support(TargetFormat::Spirv) {
    ///     Ok(()) => println!("SPIR-V is available"),
    ///     Err(error) if error.kind() == ErrorKind::Unsupported => eprintln!("{error}"),
    ///     Err(error) => return Err(error.into()),
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Unsupported`] for every failure
    /// that Slang reports, whatever its result code, which [`Error::code`]
    /// keeps.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn check_target_support(&self, target: TargetFormat) -> Result<()> {
        guard::call("GlobalSession::check_target_support", || {
            // SAFETY: `self.raw` points to a live global session, the session
            // is used only on this thread, and the call runs with the Slang
            // call guard raised.
            let result = unsafe {
                guard::abort_on_exception("IGlobalSession::checkCompileTargetSupport", || {
                    IGlobalSession::checkCompileTargetSupport(self.raw.as_ptr(), target.to_raw())
                })
            };
            mapping::check(
                Operation::CheckTargetSupport,
                result,
                Diagnostics::default(),
            )
            .map_err(|error| error.with_subject(format!("`{target:?}`")))
        })
    }

    /// Adopts a global session that the caller created through the raw
    /// bindings of `slang-compiler-sys`.
    ///
    /// Use this to hand a global session from code that calls Slang directly
    /// to this crate. The wrapper takes over the reference that the caller
    /// owns and releases it when its last handle drops.
    ///
    /// # Examples
    ///
    /// This example moves a global session out of the crate and back. It
    /// needs the Slang shared library at run time, so it is compiled but not
    /// run:
    ///
    /// ```no_run
    /// use slang_compiler::GlobalSession;
    ///
    /// let raw = GlobalSession::new()?.into_raw();
    /// // SAFETY: `raw` carries the one reference that `into_raw` gave up, of
    /// // a global session of the pinned Slang build, and this thread alone
    /// // uses it.
    /// let global = unsafe { GlobalSession::from_raw(raw) };
    /// println!("Slang {}", global.build_tag());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a live global session that the Slang library of the
    ///   build tag [`slang_compiler_sys::SLANG_BUILD_TAG`] created.
    /// - The caller owns one reference to it and transfers that reference.
    /// - The global session and every object created from it are not used on
    ///   another thread while the returned value or a clone of it exists.
    #[must_use]
    pub unsafe fn from_raw(raw: NonNull<IGlobalSession>) -> Self {
        // SAFETY: the caller guarantees that `raw` points to a live global
        // session of the pinned build and transfers one reference to it.
        let raw = unsafe { Owned::adopt(raw) };
        Self { raw }
    }

    /// Gives up this handle's reference to the global session without
    /// releasing it, and returns the pointer for use with the raw bindings of
    /// `slang-compiler-sys`.
    ///
    /// The caller then owns the reference and releases it.
    ///
    /// # Examples
    ///
    /// See the example of [`GlobalSession::from_raw`].
    #[must_use]
    pub fn into_raw(self) -> NonNull<IGlobalSession> {
        self.raw.into_raw()
    }

    /// Returns the pointer to the global session without changing its
    /// reference count, for use with the raw bindings of
    /// `slang-compiler-sys`.
    ///
    /// The pointer is valid only while this handle lives. Every use of it is
    /// unsafe and follows the safety contract of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example reads the build tag through the raw bindings. It needs
    /// the Slang shared library at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use std::ffi::CStr;
    ///
    /// use slang_compiler::GlobalSession;
    /// use slang_compiler_sys::IGlobalSession;
    ///
    /// let global = GlobalSession::new()?;
    /// // SAFETY: `global` keeps the global session alive during the call, and
    /// // only this thread uses it.
    /// let tag = unsafe { IGlobalSession::getBuildTagString(global.as_raw().as_ptr()) };
    /// if !tag.is_null() {
    ///     // SAFETY: `tag` is non-null and points to the build tag, which Slang
    ///     // keeps while the library stays loaded.
    ///     println!("{:?}", unsafe { CStr::from_ptr(tag) });
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<IGlobalSession> {
        self.raw.as_non_null()
    }

    fn create(builder: &GlobalSessionBuilder, method: &str) -> Result<Self> {
        let build_tag = || {
            // SAFETY: `spGetBuildTagString` takes no argument and needs no
            // global session, which Slang documents.
            let tag = unsafe {
                guard::abort_on_exception("spGetBuildTagString", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spGetBuildTagString()
                })
            };
            (!tag.is_null()).then(|| {
                // SAFETY: `tag` is non-null and points to the null-terminated
                // build tag, which the library keeps in static storage while
                // it stays loaded, and the crate never unloads it.
                unsafe { CStr::from_ptr(tag) }
            })
        };
        let create = |desc: &SlangGlobalSessionDesc, out: &mut *mut IGlobalSession| {
            // SAFETY: `desc` is a live descriptor whose size field holds its
            // own size, which `Default` sets, and `out` is a live out-pointer,
            // which is all that `slang_createGlobalSession2` reads and writes.
            // The call runs with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("slang_createGlobalSession2", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    slang_createGlobalSession2(desc, out)
                })
            }
        };
        // SAFETY: `create` calls the creation function of the linked Slang
        // library, whose build tag `build_tag` reports, and on success that
        // function returns a global session with one new reference that only
        // this thread has.
        unsafe { Self::create_with(builder, method, build_tag, create) }
    }

    /// Creates a global session with `create`, after the check of the build
    /// tag against the pin has passed, so that nothing reaches a library of
    /// another build except the call that reads its build tag.
    ///
    /// # Safety
    ///
    /// When `create` returns a result code that is not negative and has
    /// written a non-null pointer, that pointer points to a live global
    /// session of the library whose build tag `build_tag` reports, the caller
    /// owns one reference to it, and only this thread has the pointer.
    /// Breaking this is undefined behaviour.
    unsafe fn create_with<'tag>(
        builder: &GlobalSessionBuilder,
        method: &str,
        build_tag: impl FnOnce() -> Option<&'tag CStr>,
        create: impl FnOnce(&SlangGlobalSessionDesc, &mut *mut IGlobalSession) -> SlangResult,
    ) -> Result<Self> {
        guard::call(method, || {
            check_build_tag(build_tag())?;
            let desc = builder.desc();
            let mut raw: *mut IGlobalSession = ptr::null_mut();
            let result = create(&desc, &mut raw);
            mapping::check(
                Operation::CreateGlobalSession,
                result,
                Diagnostics::default(),
            )?;
            let raw = NonNull::new(raw).ok_or_else(|| {
                mapping::null(Operation::CreateGlobalSession, Diagnostics::default())
            })?;
            // SAFETY: `create` succeeded and wrote a non-null pointer, so the
            // caller of `create_with` guarantees that it is a live global
            // session of the library whose build tag matched the pin, with one
            // reference that this thread alone owns.
            let raw = unsafe { Owned::adopt(raw) };
            Ok(Self { raw })
        })
    }
}

impl fmt::Debug for GlobalSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GlobalSession")
            .field("raw", &self.raw.as_non_null())
            .finish_non_exhaustive()
    }
}

/// Compares the build tag that the loaded library reported with the pin of
/// `slang-compiler-sys`.
fn check_build_tag(tag: Option<&CStr>) -> Result<()> {
    if tag.is_some_and(|tag| tag.to_bytes() == SLANG_BUILD_TAG.as_bytes()) {
        return Ok(());
    }
    let loaded = tag.map_or_else(
        || "no build tag".to_owned(),
        |tag| format!("build tag `{}`", tag.to_string_lossy()),
    );
    Err(
        Error::new(ErrorKind::VersionMismatch, Operation::CreateGlobalSession).with_detail(
            format!(
                "the Slang library that the program loaded reports {loaded}, but slang-compiler was built for build tag `{SLANG_BUILD_TAG}`"
            ),
        ),
    )
}

/// A builder for a [`GlobalSession`] with settings other than Slang's
/// defaults.
///
/// Use it when the defaults of [`GlobalSession::new`] do not fit. Each setter
/// changes one setting and returns the builder, so calls can be chained, and
/// [`GlobalSessionBuilder::build`] creates the global session. A setting that
/// is never set keeps Slang's default. The builder holds only settings, so it
/// can be kept, cloned, and sent to other threads.
///
/// # Examples
///
/// This example creates a global session that accepts source in the OpenGL
/// Shading Language (GLSL), the shader language of OpenGL, besides Slang. It
/// needs the Slang shared library at run time, so it is compiled but not
/// run:
///
/// ```no_run
/// use slang_compiler::{GlobalSession, LanguageVersion, Toggle};
///
/// let global = GlobalSession::builder()
///     .glsl(Toggle::On)
///     .min_language_version(LanguageVersion::V2025)
///     .build()?;
/// # drop(global);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default)]
pub struct GlobalSessionBuilder {
    glsl: Option<Toggle>,
    min_language_version: Option<LanguageVersion>,
}

impl GlobalSessionBuilder {
    /// Sets whether the global session accepts source in the OpenGL Shading
    /// Language (GLSL), the shader language of OpenGL, which is off by
    /// default.
    ///
    /// The pinned Slang build parses GLSL in a module when the module is
    /// loaded from a file whose extension names GLSL, such as `.glsl` or
    /// `.vert`, when its source has a `#version` directive with a GLSL
    /// version, or when the options of the session set Slang's compiler
    /// option `AllowGLSL`, which
    /// [`CompilerOptions::set_int`](crate::CompilerOptions::set_int) sets.
    /// Neither Slang's header nor its user guide states the kind of the value
    /// of `AllowGLSL`, and the pinned build reads the first integer of the
    /// entry, as a Boolean that any value other than 0 turns on.
    ///
    /// # Examples
    ///
    /// See the example of [`GlobalSessionBuilder`].
    pub fn glsl(&mut self, glsl: Toggle) -> &mut Self {
        self.glsl = Some(glsl);
        self
    }

    /// Sets the oldest version of the Slang language that the global session
    /// accepts, which is [`LanguageVersion::V2025`] by default.
    ///
    /// # Examples
    ///
    /// See the example of [`GlobalSessionBuilder`].
    pub fn min_language_version(&mut self, version: LanguageVersion) -> &mut Self {
        self.min_language_version = Some(version);
        self
    }

    /// Creates the global session with the settings of this builder.
    ///
    /// # Examples
    ///
    /// See the example of [`GlobalSessionBuilder`].
    ///
    /// # Errors
    ///
    /// Returns the errors of [`GlobalSession::new`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn build(&self) -> Result<GlobalSession> {
        GlobalSession::create(self, "GlobalSessionBuilder::build")
    }

    fn desc(&self) -> SlangGlobalSessionDesc {
        let mut desc = SlangGlobalSessionDesc::default();
        if let Some(glsl) = self.glsl {
            desc.enableGLSL = glsl.is_on();
        }
        if let Some(version) = self.min_language_version {
            desc.minLanguageVersion = version.to_raw().0;
        }
        desc
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::ffi::{CStr, CString};
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::ptr;

    use slang_compiler_sys::{
        IGlobalSession, SLANG_BUILD_TAG, SlangGlobalSessionDesc, SlangResult,
    };

    use super::{GlobalSession, GlobalSessionBuilder, check_build_tag};
    use crate::fake::{Event, FakeGlobalSession, Log, panic_message};
    use crate::{ErrorKind, LanguageVersion, Operation, TargetFormat, Toggle, guard};

    fn adopt(log: &Log, name: &'static str, target_support: SlangResult) -> GlobalSession {
        // SAFETY: the new fake global session carries one reference, it
        // reports the pinned build tag, and the test uses it on this thread
        // only.
        unsafe { GlobalSession::from_raw(FakeGlobalSession::create(log, name, target_support)) }
    }

    fn pin() -> CString {
        CString::new(SLANG_BUILD_TAG).expect("the pin has no NUL byte")
    }

    /// Runs `create_with` with a build tag of `tag` and a creator that returns
    /// `result` and writes `created`, and returns the result with the names of
    /// the calls that it made, in order.
    fn create_with(
        builder: &GlobalSessionBuilder,
        tag: Option<&CStr>,
        result: SlangResult,
        created: *mut IGlobalSession,
    ) -> (crate::Result<GlobalSession>, Vec<&'static str>) {
        let calls = RefCell::new(Vec::new());
        // SAFETY: every test passes as `created` either null or a new fake
        // global session of the pinned build with one reference, which only
        // this thread has and which `create_with` adopts on success.
        let global = unsafe {
            GlobalSession::create_with(
                builder,
                "Test::create",
                || {
                    calls.borrow_mut().push("build tag");
                    tag
                },
                |_, out| {
                    calls.borrow_mut().push("create");
                    *out = created;
                    result
                },
            )
        };
        (global, calls.into_inner())
    }

    #[test]
    fn create_with_a_build_tag_of_another_build_fails_without_creating() {
        let (result, calls) = create_with(
            &GlobalSessionBuilder::default(),
            Some(c"2026.16"),
            SlangResult::SLANG_OK,
            ptr::null_mut(),
        );
        let error = result.expect_err("another build tag fails");
        assert_eq!(error.kind(), ErrorKind::VersionMismatch);
        assert_eq!(error.operation(), Operation::CreateGlobalSession);
        assert_eq!(calls, ["build tag"]);
    }

    #[test]
    fn create_with_no_build_tag_fails_without_creating() {
        let (result, calls) = create_with(
            &GlobalSessionBuilder::default(),
            None,
            SlangResult::SLANG_OK,
            ptr::null_mut(),
        );
        assert_eq!(
            result.map(drop).map_err(|error| error.kind()),
            Err(ErrorKind::VersionMismatch)
        );
        assert_eq!(calls, ["build tag"]);
    }

    #[test]
    fn create_with_a_failing_result_code_fails_with_the_mapped_kind() {
        let pin = pin();
        let (result, calls) = create_with(
            &GlobalSessionBuilder::default(),
            Some(&pin),
            SlangResult::SLANG_E_INTERNAL_FAIL,
            ptr::null_mut(),
        );
        let error = result.expect_err("the creator fails");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.code(), Some(SlangResult::SLANG_E_INTERNAL_FAIL.0));
        assert_eq!(calls, ["build tag", "create"]);
    }

    #[test]
    fn create_with_a_null_global_session_fails() {
        let pin = pin();
        let (result, calls) = create_with(
            &GlobalSessionBuilder::default(),
            Some(&pin),
            SlangResult::SLANG_OK,
            ptr::null_mut(),
        );
        let error = result.expect_err("a null global session fails");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.code(), None);
        assert_eq!(calls, ["build tag", "create"]);
    }

    #[test]
    fn create_with_the_pinned_build_tag_adopts_the_created_global_session() {
        let log = Log::new();
        let pin = pin();
        let fake = FakeGlobalSession::create(&log, "global", SlangResult::SLANG_OK);
        let mut builder = GlobalSession::builder();
        builder.glsl(Toggle::On);
        let desc = RefCell::new(None);
        // SAFETY: `fake` is a new fake global session of the pinned build with
        // one reference, which only this thread has and `create_with` adopts.
        let global = unsafe {
            GlobalSession::create_with(
                &builder,
                "Test::create",
                || Some(pin.as_c_str()),
                |passed, out| {
                    *desc.borrow_mut() = Some((passed.enableGLSL, passed.structureSize));
                    *out = fake.as_ptr();
                    SlangResult::SLANG_OK
                },
            )
        }
        .expect("the creator succeeds");
        assert_eq!(global.as_raw(), fake);
        assert_eq!(
            desc.into_inner(),
            Some((true, SlangGlobalSessionDesc::default().structureSize))
        );
        assert_eq!(log.events(), []);
        drop(global);
        assert_eq!(
            log.events(),
            [Event::release("global", 0), Event::freed("global")]
        );
    }

    #[test]
    fn check_build_tag_accepts_the_pin() {
        assert!(check_build_tag(Some(&pin())).is_ok());
    }

    #[test]
    fn check_build_tag_rejects_another_build_and_names_both() {
        let error = check_build_tag(Some(c"2026.16")).expect_err("another build tag fails");
        assert_eq!(error.kind(), ErrorKind::VersionMismatch);
        assert_eq!(error.operation(), Operation::CreateGlobalSession);
        let text = error.to_string();
        assert!(text.contains("`2026.16`"), "{text}");
        assert!(text.contains(&format!("`{SLANG_BUILD_TAG}`")), "{text}");
    }

    #[test]
    fn check_build_tag_rejects_a_prefix_of_the_pin() {
        let pin = SLANG_BUILD_TAG.as_bytes();
        let prefix = CString::new(pin.get(..pin.len() - 1).expect("the pin is not empty"))
            .expect("the pin has no NUL byte");
        let prefix: &CStr = &prefix;
        assert_eq!(
            check_build_tag(Some(prefix)).map_err(|error| error.kind()),
            Err(ErrorKind::VersionMismatch)
        );
    }

    #[test]
    fn check_build_tag_rejects_a_missing_tag() {
        let error = check_build_tag(None).expect_err("a missing build tag fails");
        assert_eq!(error.kind(), ErrorKind::VersionMismatch);
        assert!(error.to_string().contains("no build tag"));
    }

    #[test]
    fn builder_keeps_slang_defaults_for_unset_settings() {
        let desc = GlobalSession::builder().desc();
        let default = SlangGlobalSessionDesc::default();
        assert_eq!(desc.enableGLSL, default.enableGLSL);
        assert_eq!(desc.minLanguageVersion, default.minLanguageVersion);
        assert_eq!(desc.structureSize, default.structureSize);
    }

    #[test]
    fn builder_sets_glsl_and_the_minimum_language_version() {
        let desc = GlobalSession::builder()
            .glsl(Toggle::On)
            .min_language_version(LanguageVersion::V2026)
            .desc();
        assert!(desc.enableGLSL);
        assert_eq!(desc.minLanguageVersion, 2026);
    }

    #[test]
    fn check_target_support_returns_ok_when_slang_reports_support() {
        let log = Log::new();
        let global = adopt(&log, "global", SlangResult::SLANG_OK);
        assert!(global.check_target_support(TargetFormat::Spirv).is_ok());
        drop(global);
        assert_eq!(
            log.events(),
            [
                Event::check_target("global", 6),
                Event::release("global", 0),
                Event::freed("global")
            ]
        );
    }

    #[test]
    fn check_target_support_reports_every_failure_as_unsupported() {
        let log = Log::new();
        let global = adopt(&log, "global", SlangResult::SLANG_E_NOT_FOUND);
        let error = global
            .check_target_support(TargetFormat::Dxil)
            .expect_err("the fake reports no support");
        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert_eq!(error.operation(), Operation::CheckTargetSupport);
        assert_eq!(error.code(), Some(SlangResult::SLANG_E_NOT_FOUND.0));
        assert_eq!(
            error.to_string(),
            "cannot produce code for target `Dxil` (Slang result code 0x82000005)"
        );
    }

    #[test]
    fn a_method_called_while_the_guard_is_raised_panics_without_calling_slang() {
        let log = Log::new();
        let global = adopt(&log, "global", SlangResult::SLANG_OK);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            guard::call("Test::callback", || {
                global.check_target_support(TargetFormat::Spirv)
            })
        }));
        let message = panic_message(outcome.expect_err("the nested call panics").as_ref());
        assert!(
            message.starts_with("reentrant call: GlobalSession::check_target_support "),
            "{message}"
        );
        assert_eq!(log.events(), []);
    }

    #[test]
    fn creating_a_global_session_while_the_guard_is_raised_panics() {
        let creators: [fn() -> crate::Result<GlobalSession>; 2] =
            [GlobalSession::new, || GlobalSession::builder().build()];
        for create in creators {
            let outcome = catch_unwind(AssertUnwindSafe(|| guard::call("Test::callback", create)));
            let message = panic_message(outcome.expect_err("the nested creation panics").as_ref());
            assert!(
                message.starts_with("reentrant call: GlobalSession"),
                "{message}"
            );
        }
    }

    #[test]
    fn clones_share_one_global_session_and_the_last_drop_releases_it() {
        let log = Log::new();
        let global = adopt(&log, "global", SlangResult::SLANG_OK);
        let clone = global.clone();
        assert_eq!(global.as_raw(), clone.as_raw());
        assert_eq!(global.build_tag(), SLANG_BUILD_TAG);
        drop(global);
        drop(clone);
        assert_eq!(
            log.events(),
            [
                Event::add_ref("global", 2),
                Event::release("global", 1),
                Event::release("global", 0),
                Event::freed("global"),
            ]
        );
    }

    #[test]
    fn a_global_session_dropped_while_the_guard_is_raised_stays_alive_until_it_falls() {
        let log = Log::new();
        let global = adopt(&log, "global", SlangResult::SLANG_OK);
        guard::call("Test::callback", || {
            drop(global);
            assert_eq!(log.events(), []);
        });
        assert_eq!(
            log.events(),
            [Event::release("global", 0), Event::freed("global")]
        );
    }

    #[test]
    fn into_raw_gives_up_the_reference_without_releasing_it() {
        let log = Log::new();
        let raw = adopt(&log, "global", SlangResult::SLANG_OK).into_raw();
        assert_eq!(log.events(), []);
        // SAFETY: `into_raw` gave up the one reference of the fake, which
        // `from_raw` takes back.
        drop(unsafe { GlobalSession::from_raw(raw) });
        assert_eq!(
            log.events(),
            [Event::release("global", 0), Event::freed("global")]
        );
    }

    #[test]
    fn into_raw_while_a_clone_is_alive_gives_up_one_reference() {
        let log = Log::new();
        let global = adopt(&log, "global", SlangResult::SLANG_OK);
        let clone = global.clone();
        let raw = global.into_raw();
        drop(clone);
        // SAFETY: `into_raw` gave up one reference of the fake, which
        // `from_raw` takes back.
        drop(unsafe { GlobalSession::from_raw(raw) });
        assert_eq!(
            log.events(),
            [
                Event::add_ref("global", 2),
                Event::release("global", 1),
                Event::release("global", 0),
                Event::freed("global"),
            ]
        );
    }
}
