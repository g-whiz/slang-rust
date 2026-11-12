use std::fmt;
use std::iter::FusedIterator;
use std::ptr::{self, NonNull};

use slang_compiler_sys::{
    IEntryPoint, IModule, ISession, ISlangBlob, SlangInt32, SlangReflectionFunction,
    slang_createBlob,
};

use crate::blob::{self, Blob};
use crate::component::{self, LinkedProgram};
use crate::enums::Stage;
use crate::error::{Compiled, Diagnostics, Error, ErrorKind, Operation, Result};
use crate::guard;
use crate::mapping;
use crate::options::CompilerOptions;
use crate::owned::Owned;
use crate::reflection::{Function, ProgramLayout};
use crate::session::{Session, TargetIndex};
use crate::strings::{borrowed_str, c_string};

impl Session {
    /// Loads the module `name` from the search paths of the session.
    ///
    /// A module is one unit of Slang source, like a Rust module, and its name
    /// is the name that an `import` declaration of Slang uses. Slang looks for
    /// a file for the name in each search path in order, as
    /// [`SessionBuilder::search_path`](crate::SessionBuilder::search_path)
    /// describes, trying a precompiled `.slang-module` file before a
    /// `.slang` source file, and with each underscore of the name also
    /// written as a hyphen. A module is loaded once in each session: a later
    /// load of the same name returns the same module, and a changed file is
    /// seen only by a new session.
    ///
    /// # Examples
    ///
    /// This example loads the module of the file `/tmp/shaders/lighting.slang`.
    /// It needs the Slang shared library at run time, so it is compiled but
    /// not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .search_path("/tmp/shaders")
    ///     .build()?;
    /// let module = session.load_module("lighting")?;
    /// if !module.diagnostics.is_empty() {
    ///     eprintln!("{}", module.diagnostics);
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind:
    ///
    /// - [`ErrorKind::InvalidArgument`] when `name` contains a NUL byte.
    /// - [`ErrorKind::Compile`], with the diagnostics, when the source does
    ///   not compile, and also when no search path holds a file for the name,
    ///   with a diagnostic that says that Slang cannot open the file.
    /// - [`ErrorKind::NotFound`], with empty diagnostics, when an earlier load
    ///   of the same name in this session found no file. Slang remembers that
    ///   failure for the session, so adding the file does not help. Create a
    ///   new session to load the module again.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn load_module(&self, name: &str) -> Result<Compiled<Module>> {
        let operation = Operation::LoadModule;
        let c_name = c_string(name, operation, "the module name")?;
        guard::call("Session::load_module", || {
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: `self.raw()` points to a live session, used only on this
            // thread (Sections 11.1 and 11.2 of
            // `docs/design/slang-compiler.md`), `c_name` is a C string that
            // lives until the call returns, `diagnostics` is a live
            // out-pointer, and the call runs with the Slang call guard raised
            // (SLC-28).
            let raw = unsafe {
                guard::abort_on_exception("ISession::loadModule", || {
                    ISession::loadModule(self.raw().as_ptr(), c_name.as_ptr(), &raw mut diagnostics)
                })
            };
            self.loaded(name, raw, diagnostics)
        })
    }

    /// Compiles the module `name` from the Slang source `source`, which is
    /// held in memory.
    ///
    /// Use this when the source is a string, such as source that the program
    /// generates or embeds. `path` names the source in diagnostics, and Slang
    /// resolves an `import` or an `#include` of the source relative to it.
    /// When a file exists at `path`, Slang reads its path through the
    /// operating system, but it compiles `source`, not the file. A module
    /// name is loaded once in each session: a second load of the same name
    /// with identical source returns the same module, and one with different
    /// source fails.
    ///
    /// # Examples
    ///
    /// See the example of [`Session::compose`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind:
    ///
    /// - [`ErrorKind::InvalidArgument`] when `name`, `path`, or `source`
    ///   contains a NUL byte.
    /// - [`ErrorKind::Compile`], with the diagnostics, when the source does
    ///   not compile, or when the session already loaded the name with
    ///   different source.
    /// - [`ErrorKind::NotFound`], with empty diagnostics, when an earlier load
    ///   of the same name in this session found no file, as for
    ///   [`Session::load_module`]. Create a new session to load the module
    ///   again.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn load_module_from_source(
        &self,
        name: &str,
        path: &str,
        source: &str,
    ) -> Result<Compiled<Module>> {
        let operation = Operation::LoadModule;
        let c_name = c_string(name, operation, "the module name")?;
        let c_path = c_string(path, operation, "the path")?;
        let c_source = c_string(source, operation, "the source")?;
        guard::call("Session::load_module_from_source", || {
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: as in `load_module`, with the C strings `c_path` and
            // `c_source`, which also live until the call returns.
            let raw = unsafe {
                guard::abort_on_exception("ISession::loadModuleFromSourceString", || {
                    ISession::loadModuleFromSourceString(
                        self.raw().as_ptr(),
                        c_name.as_ptr(),
                        c_path.as_ptr(),
                        c_source.as_ptr(),
                        &raw mut diagnostics,
                    )
                })
            };
            self.loaded(name, raw, diagnostics)
        })
    }

    /// Loads the module `name` from its intermediate representation (IR),
    /// Slang's compiled form of a module, which [`Module::serialize`]
    /// returns.
    ///
    /// Use this to load a module that was compiled earlier without compiling
    /// its source again. `path` names the module in diagnostics, as for
    /// [`Session::load_module_from_source`].
    ///
    /// # Examples
    ///
    /// This example serializes a module and loads it in a second session. It
    /// needs the Slang shared library at run time, so it is compiled but not
    /// run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let first = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .build()?;
    /// let ir = first
    ///     .load_module_from_source("shapes", "shapes.slang", "float twice(float x) { return 2.0 * x; }")?
    ///     .into_value()
    ///     .serialize()?;
    /// let second = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .build()?;
    /// let module = second
    ///     .load_module_from_ir("shapes", "shapes.slang-module", ir.as_bytes())?
    ///     .into_value();
    /// # let _ = module;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind:
    ///
    /// - [`ErrorKind::InvalidArgument`] when `ir` is empty, or `name` or
    ///   `path` contains a NUL byte.
    /// - [`ErrorKind::Compile`], with the diagnostics, when Slang reports why
    ///   it cannot load the bytes as a module.
    /// - [`ErrorKind::NotFound`], with empty diagnostics, when Slang returns
    ///   no module and no reason, which it does for bytes that are not a
    ///   module of this Slang build, and after an earlier load of the same
    ///   name in this session that found no file. Create a new session to load
    ///   the module again after such a load.
    /// - [`ErrorKind::Internal`] when Slang cannot copy the bytes.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn load_module_from_ir(
        &self,
        name: &str,
        path: &str,
        ir: &[u8],
    ) -> Result<Compiled<Module>> {
        let create_blob = |ir: &[u8]| {
            // SAFETY: `ir` points to `ir.len()` initialized bytes, which
            // `slang_createBlob` copies into a blob of its own during the call
            // (Section 3.8 of `docs/design/slang-compiler.md`), and the call
            // runs with the Slang call guard raised (SLC-28).
            unsafe {
                guard::abort_on_exception("slang_createBlob", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    slang_createBlob(ir.as_ptr().cast(), ir.len())
                })
            }
        };
        // SAFETY: `slang_createBlob` returns a new reference or null (Section
        // 5.3 of `docs/design/slang-compiler.md`).
        unsafe { self.load_module_from_ir_with(name, path, ir, create_blob) }
    }

    /// Loads a module from the intermediate representation `ir`, which
    /// `create_blob` copies into a blob, as `load_module_from_ir` describes.
    ///
    /// # Safety
    ///
    /// `create_blob` returns null or a blob of the pinned Slang build with
    /// one new reference that the caller of `create_blob` owns, and the blob
    /// holds a copy of the bytes that it was given. Breaking this is undefined
    /// behaviour.
    unsafe fn load_module_from_ir_with(
        &self,
        name: &str,
        path: &str,
        ir: &[u8],
        create_blob: impl FnOnce(&[u8]) -> *mut ISlangBlob,
    ) -> Result<Compiled<Module>> {
        let operation = Operation::LoadModule;
        if ir.is_empty() {
            return Err(Error::new(ErrorKind::InvalidArgument, operation)
                .with_subject(format!("`{name}`"))
                .with_detail("the intermediate representation is empty".to_owned()));
        }
        let c_name = c_string(name, operation, "the module name")?;
        let c_path = c_string(path, operation, "the path")?;
        guard::call("Session::load_module_from_ir", || {
            let raw_blob = create_blob(ir);
            // SAFETY: the caller guarantees that `create_blob` returns a new
            // reference or null.
            let ir_blob = unsafe { Owned::adopt_output(raw_blob) }.ok_or_else(|| {
                Error::new(ErrorKind::Internal, operation)
                    .with_subject(format!("`{name}`"))
                    .with_detail(
                        "Slang could not copy the intermediate representation into a blob"
                            .to_owned(),
                    )
            })?;
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: as in `load_module`, with the C string `c_path`, and
            // `ir_blob`, which the call borrows and the crate releases after
            // it returns (Section 5.3).
            let raw = unsafe {
                guard::abort_on_exception("ISession::loadModuleFromIRBlob", || {
                    ISession::loadModuleFromIRBlob(
                        self.raw().as_ptr(),
                        c_name.as_ptr(),
                        c_path.as_ptr(),
                        ir_blob.as_ptr(),
                        &raw mut diagnostics,
                    )
                })
            };
            self.loaded_from_ir(name, raw, diagnostics)
        })
    }

    /// Returns an iterator over the modules that the session holds, which
    /// includes every module that it loaded and every module that those
    /// import.
    ///
    /// # Examples
    ///
    /// This example prints the name of each loaded module:
    ///
    /// ```
    /// use slang_compiler::Session;
    ///
    /// fn print_modules(session: &Session) -> slang_compiler::Result<()> {
    ///     for module in session.loaded_modules()? {
    ///         println!("{:?}", module.name()?);
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = print_modules;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Internal`] when Slang reports a
    /// negative count.
    ///
    /// # Panics
    ///
    /// Panics when it, or the `next` of the iterator, is called while another
    /// call of this crate into Slang is in progress on the same thread,
    /// because Slang is not reentrant. Only code that Slang calls back during
    /// such a call can do that.
    pub fn loaded_modules(&self) -> Result<LoadedModules> {
        guard::call("Session::loaded_modules", || {
            // SAFETY: `self.raw()` points to a live session, used only on this
            // thread, and the call runs with the Slang call guard raised.
            let count = unsafe {
                guard::abort_on_exception("ISession::getLoadedModuleCount", || {
                    ISession::getLoadedModuleCount(self.raw().as_ptr())
                })
            };
            let count = usize::try_from(count).map_err(|error| {
                Error::new(ErrorKind::Internal, Operation::ListLoadedModules)
                    .with_detail(format!("Slang reported the count {count}: {error}"))
            })?;
            Ok(LoadedModules {
                session: self.clone(),
                front: 0,
                back: count,
            })
        })
    }

    /// Finishes a module load whose loader returned `raw`. The caller raises
    /// the guard.
    fn loaded(
        &self,
        name: &str,
        raw: *mut IModule,
        diagnostics: *mut ISlangBlob,
    ) -> Result<Compiled<Module>> {
        self.finish_load(name, raw, diagnostics, "Slang returned no module and no reason, which happens after an earlier load of the name failed to find a file in this session, and a new session loads it again")
    }

    /// Finishes a load from intermediate representation, for which Slang
    /// also returns no reason when the bytes are not a module. The caller
    /// raises the guard.
    fn loaded_from_ir(
        &self,
        name: &str,
        raw: *mut IModule,
        diagnostics: *mut ISlangBlob,
    ) -> Result<Compiled<Module>> {
        self.finish_load(name, raw, diagnostics, "Slang returned no module and no reason, which happens when the bytes are not a module of this Slang build, or after an earlier load of the name failed to find a file in this session, and a new session loads it again")
    }

    fn finish_load(
        &self,
        name: &str,
        raw: *mut IModule,
        diagnostics: *mut ISlangBlob,
        no_reason: &str,
    ) -> Result<Compiled<Module>> {
        let operation = Operation::LoadModule;
        // SAFETY: every diagnostics output carries a new reference or is null
        // (Section 5.3 of `docs/design/slang-compiler.md`).
        let diagnostics = unsafe { blob::take_diagnostics(diagnostics) };
        let Some(raw) = NonNull::new(raw) else {
            let error = mapping::null(operation, diagnostics).with_subject(format!("`{name}`"));
            return Err(if error.kind() == ErrorKind::NotFound {
                error.with_detail(no_reason.to_owned())
            } else {
                error
            });
        };
        // SAFETY: the module loaders return a borrowed pointer to a module
        // that the session holds, and the session is alive (Section 5.3).
        let raw = unsafe { Owned::from_borrowed(raw) };
        Ok(Compiled {
            value: Module {
                raw,
                session: self.clone(),
            },
            diagnostics,
        })
    }
}

/// A module, one unit of Slang source that a session loaded.
///
/// Slang is a compiler for shaders, the programs that run on a graphics
/// processing unit (GPU), and a module is one file of its source, like a Rust
/// module. Get one from [`Session::load_module`],
/// [`Session::load_module_from_source`], or [`Session::load_module_from_ir`].
/// A module yields its entry points, the functions that a graphics API
/// calls, and takes part in a composition through
/// [`Component::Module`](crate::Component::Module).
///
/// A module holds its [`Session`], so the session lives at least as long as
/// the module. It is neither [`Send`] nor [`Sync`]. [`Clone`] returns another
/// handle to the same module.
///
/// # Examples
///
/// See the example of [`Session::compose`](crate::Session::compose).
///
/// A module cannot move to another thread, so this example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::Module>();
/// ```
///
/// A module cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::Module>();
/// ```
#[derive(Clone)]
pub struct Module {
    raw: Owned<IModule>,
    session: Session,
}

impl Module {
    pub(crate) fn raw(&self) -> &Owned<IModule> {
        &self.raw
    }

    pub(crate) fn session(&self) -> &Session {
        &self.session
    }

    /// Returns the name of the module, or `None` when it has none.
    ///
    /// # Examples
    ///
    /// See the example of [`Session::loaded_modules`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidUtf8`] when the name is
    /// not valid UTF-8.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn name(&self) -> Result<Option<&str>> {
        guard::call("Module::name", || {
            // SAFETY: `self.raw` points to a live module of `self.session`
            // (Sections 11.1 and 11.2 of `docs/design/slang-compiler.md`), and
            // the call runs with the Slang call guard raised (SLC-28).
            let raw = unsafe {
                guard::abort_on_exception("IModule::getName", || {
                    IModule::getName(self.raw.as_ptr())
                })
            };
            // SAFETY: the name is a string of the name pool of the global
            // session, which `self` keeps alive through its session, and the
            // pool never changes a string (Section 3.3).
            unsafe { borrowed_str(raw, Operation::ReadModuleName, "the name of the module") }
        })
    }

    /// Returns the path of the file that the module was loaded from, or
    /// `None` when it has none, such as for a module compiled from source in
    /// memory without a file at its path.
    ///
    /// # Examples
    ///
    /// This example prints where a module came from:
    ///
    /// ```
    /// use slang_compiler::Module;
    ///
    /// fn print_origin(module: &Module) -> slang_compiler::Result<()> {
    ///     match module.file_path()? {
    ///         Some(path) => println!("loaded from {path}"),
    ///         None => println!("compiled from memory"),
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = print_origin;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidUtf8`] when the path is
    /// not valid UTF-8.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn file_path(&self) -> Result<Option<&str>> {
        guard::call("Module::file_path", || {
            // SAFETY: as in `name`.
            let raw = unsafe {
                guard::abort_on_exception("IModule::getFilePath", || {
                    IModule::getFilePath(self.raw.as_ptr())
                })
            };
            // SAFETY: the path is a string that the module holds and never
            // changes after the load (Section 3.3), and `self` keeps the
            // module alive.
            unsafe {
                borrowed_str(
                    raw,
                    Operation::ReadModuleFilePath,
                    "the file path of the module",
                )
            }
        })
    }

    /// Finds the entry point `name` among the entry points that the module
    /// defines, the functions that carry a `[shader(...)]` attribute.
    ///
    /// An entry point is a function of a shader that a graphics API calls,
    /// like `main` in a program.
    ///
    /// # Examples
    ///
    /// See the example of [`Session::compose`](crate::Session::compose).
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::NotFound`] when the module
    /// defines no entry point of that name, and
    /// [`ErrorKind::InvalidArgument`] when `name` contains a NUL byte.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn entry_point(&self, name: &str) -> Result<EntryPoint> {
        let operation = Operation::FindEntryPoint;
        let c_name = c_string(name, operation, "the entry point name")?;
        guard::call("Module::entry_point", || {
            let mut out: *mut IEntryPoint = ptr::null_mut();
            // SAFETY: as in `name`, with the C string `c_name`, which lives
            // until the call returns, and the live out-pointer `out`.
            let result = unsafe {
                guard::abort_on_exception("IModule::findEntryPointByName", || {
                    IModule::findEntryPointByName(self.raw.as_ptr(), c_name.as_ptr(), &raw mut out)
                })
            };
            // SAFETY: `findEntryPointByName` returns a new reference or leaves
            // the output null (Section 5.3 of `docs/design/slang-compiler.md`).
            let out = unsafe { Owned::adopt_output(out) };
            let subject = || format!("`{name}`");
            mapping::check(operation, result, Diagnostics::default())
                .map_err(|error| error.with_subject(subject()))?;
            let raw = out.ok_or_else(|| {
                mapping::null(operation, Diagnostics::default()).with_subject(subject())
            })?;
            Ok(EntryPoint {
                raw,
                session: self.session.clone(),
            })
        })
    }

    /// Finds the function `name` of the module and checks it as an entry point
    /// of the stage `stage`, which also accepts a function without a
    /// `[shader(...)]` attribute.
    ///
    /// An entry point is a function of a shader that a graphics API calls,
    /// like `main` in a program, and its stage is the part of the pipeline
    /// that runs it, such as [`Stage::Compute`]. Slang checks the parameters
    /// and the semantics of the function against the stage, and returns its
    /// warnings with the entry point. A function that carries a
    /// `[shader(...)]` attribute was checked when the module was loaded, and
    /// Slang returns it without another check, whatever `stage` is.
    ///
    /// Aborts the process, instead of returning an error, when the pinned
    /// Slang build lets a C++ exception escape this call, which it does for
    /// some functions, such as one whose parameter is a struct nested more
    /// than 128 levels deep. Before it aborts, it writes one line to standard
    /// error that names the Slang call `IModule::findAndCheckEntryPoint`. When
    /// the program is built with `panic = "abort"`, the process aborts without
    /// that line.
    ///
    /// # Examples
    ///
    /// This example finds a compute kernel that has no `[shader(...)]`
    /// attribute and links it.
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Stage, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .build()?;
    /// let module = session
    ///     .load_module_from_source(
    ///         "kernel",
    ///         "kernel.slang",
    ///         "[numthreads(64, 1, 1)] void double_it(uint3 id : SV_DispatchThreadID) {}",
    ///     )?
    ///     .value;
    /// let entry_point = module.entry_point_with_stage("double_it", Stage::Compute)?;
    /// let program = entry_point.value.link()?.value;
    /// # let _ = program;
    /// # Ok::<(), slang_compiler::Error>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Compile`] with Slang's
    /// diagnostics when the module has no function `name` or the function is
    /// not a valid entry point of the stage, [`ErrorKind::NotFound`] when
    /// Slang fails without diagnostics, and [`ErrorKind::InvalidArgument`]
    /// when `name` contains a NUL byte.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn entry_point_with_stage(&self, name: &str, stage: Stage) -> Result<Compiled<EntryPoint>> {
        let operation = Operation::FindEntryPoint;
        let c_name = c_string(name, operation, "the entry point name")?;
        guard::call("Module::entry_point_with_stage", || {
            let mut out: *mut IEntryPoint = ptr::null_mut();
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: as in `name`, with the C string `c_name`, which lives
            // until the call returns, and the live out-pointers `out` and
            // `diagnostics`.
            let result = unsafe {
                guard::abort_on_exception("IModule::findAndCheckEntryPoint", || {
                    IModule::findAndCheckEntryPoint(
                        self.raw.as_ptr(),
                        c_name.as_ptr(),
                        stage.to_raw(),
                        &raw mut out,
                        &raw mut diagnostics,
                    )
                })
            };
            // SAFETY: `findAndCheckEntryPoint` returns a new reference or
            // leaves the output null, and every diagnostics output carries a
            // new reference or is null (Section 5.3 of
            // `docs/design/slang-compiler.md`).
            let Compiled {
                value: raw,
                diagnostics,
            } = unsafe { blob::take_output(operation, result, out, diagnostics) }
                .map_err(|error| error.with_subject(format!("`{name}`")))?;
            Ok(Compiled {
                value: EntryPoint {
                    raw,
                    session: self.session.clone(),
                },
                diagnostics,
            })
        })
    }

    /// Returns an iterator over the entry points that the module defines, in
    /// the order of the source.
    ///
    /// # Examples
    ///
    /// This example collects every entry point of a module:
    ///
    /// ```
    /// use slang_compiler::{EntryPoint, Module};
    ///
    /// fn all(module: &Module) -> slang_compiler::Result<Vec<EntryPoint>> {
    ///     module.entry_points()?.collect()
    /// }
    /// # let _ = all;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Internal`] when Slang reports a
    /// negative count. Each item is an error when Slang fails to return the
    /// entry point.
    ///
    /// # Panics
    ///
    /// Panics when it, or the `next` of the iterator, is called while another
    /// call of this crate into Slang is in progress on the same thread,
    /// because Slang is not reentrant. Only code that Slang calls back during
    /// such a call can do that.
    pub fn entry_points(&self) -> Result<EntryPoints> {
        guard::call("Module::entry_points", || {
            // SAFETY: as in `name`.
            let count = unsafe {
                guard::abort_on_exception("IModule::getDefinedEntryPointCount", || {
                    IModule::getDefinedEntryPointCount(self.raw.as_ptr())
                })
            };
            let count = usize::try_from(count).map_err(|error| {
                Error::new(ErrorKind::Internal, Operation::ListEntryPoints)
                    .with_detail(format!("Slang reported the count {count}: {error}"))
            })?;
            Ok(EntryPoints {
                module: self.clone(),
                front: 0,
                back: count,
            })
        })
    }

    /// Returns an iterator over the paths of the files that the module
    /// depends on, such as its own source and the files that it includes.
    ///
    /// Use it to watch the files of a module for changes, then load the
    /// module in a new session when one changes.
    ///
    /// # Examples
    ///
    /// This example prints the files of a module:
    ///
    /// ```
    /// use slang_compiler::Module;
    ///
    /// fn print_files(module: &Module) -> slang_compiler::Result<()> {
    ///     for path in module.dependency_file_paths()? {
    ///         if let Some(path) = path? {
    ///             println!("{path}");
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = print_files;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Internal`] when Slang reports a
    /// negative count. Each item is `None` for a file without a path, and an
    /// error of kind [`ErrorKind::InvalidUtf8`] for a path that is not valid
    /// UTF-8.
    ///
    /// # Panics
    ///
    /// Panics when it, or the `next` of the iterator, is called while another
    /// call of this crate into Slang is in progress on the same thread,
    /// because Slang is not reentrant. Only code that Slang calls back during
    /// such a call can do that.
    pub fn dependency_file_paths(&self) -> Result<DependencyFilePaths<'_>> {
        guard::call("Module::dependency_file_paths", || {
            let count = self.dependency_file_count()?;
            Ok(DependencyFilePaths {
                module: self,
                front: 0,
                back: count,
            })
        })
    }

    /// Returns the module in Slang's intermediate representation (IR), its
    /// compiled form, which [`Session::load_module_from_ir`] loads.
    ///
    /// # Examples
    ///
    /// See the example of [`Session::load_module_from_ir`].
    ///
    /// # Errors
    ///
    /// Returns the kind that Slang's result code maps to when Slang cannot
    /// serialize the module, such as [`ErrorKind::Internal`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn serialize(&self) -> Result<Blob> {
        let operation = Operation::SerializeModule;
        guard::call("Module::serialize", || {
            let mut out: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: as in `name`, with the live out-pointer `out`.
            let result = unsafe {
                guard::abort_on_exception("IModule::serialize", || {
                    IModule::serialize(self.raw.as_ptr(), &raw mut out)
                })
            };
            // SAFETY: `serialize` returns a new blob reference or leaves the
            // output null (Section 5.3 of `docs/design/slang-compiler.md`).
            let out = unsafe { Owned::adopt_output(out) };
            mapping::check(operation, result, Diagnostics::default())?;
            let out = out.ok_or_else(|| mapping::null(operation, Diagnostics::default()))?;
            Blob::new(out, self.session.clone(), operation)
        })
    }

    /// Links the module on its own and returns the linked program, which
    /// generates code for the module as a whole.
    ///
    /// A module has no entry points of its own in a link, so its program
    /// generates code through
    /// [`LinkedProgram::target_code`](crate::LinkedProgram::target_code).
    /// Compose the module with its entry points to generate the code of an
    /// entry point.
    ///
    /// # Examples
    ///
    /// This example links a module on its own. It needs the Slang shared
    /// library at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Hlsl))
    ///     .build()?;
    /// let module = session
    ///     .load_module_from_source("shapes", "shapes.slang", "float twice(float x) { return 2.0 * x; }")?
    ///     .into_value();
    /// let program = module.link()?.into_value();
    /// assert_eq!(program.entry_point_count(), 0);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the errors of
    /// [`ComponentType::link`](crate::ComponentType::link).
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn link(&self) -> Result<Compiled<LinkedProgram>> {
        component::link(&self.raw, &self.session, 0, None, "Module::link")
    }

    /// Links the module on its own with compiler options, as
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options)
    /// does for a component type.
    ///
    /// Slang returns the module itself as the linked program when nothing
    /// more needs linking, so the options stay in the module that the session
    /// returns to every later load of its name, as
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options)
    /// describes.
    ///
    /// The options take effect as the documentation of
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options)
    /// describes: which options change the generated code, which change
    /// nothing, and why options added after code was generated for a target
    /// have no effect on it.
    ///
    /// # Examples
    ///
    /// This example links a module with the option that keeps unused shader
    /// parameters. It needs the Slang shared library at run time, so it is
    /// compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{CompilerOptions, GlobalSession, Session, Target, TargetFormat, Toggle};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .build()?;
    /// let module = session
    ///     .load_module_from_source("shapes", "shapes.slang", "float twice(float x) { return 2.0 * x; }")?
    ///     .into_value();
    /// let mut options = CompilerOptions::default();
    /// options.preserve_parameters(Toggle::On);
    /// let program = module.link_with_options(&options)?.into_value();
    /// # let _ = program;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the errors of
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options).
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn link_with_options(&self, options: &CompilerOptions) -> Result<Compiled<LinkedProgram>> {
        component::link(
            &self.raw,
            &self.session,
            0,
            Some(options),
            "Module::link_with_options",
        )
    }

    /// Returns the layout of the module for the target `target`, the root of
    /// its reflection, which describes where each parameter sits in memory
    /// and in the bindings of the target.
    ///
    /// A binding is a numbered slot of a graphics API through which a shader
    /// reaches one resource, such as a buffer or a texture. The layout
    /// borrows the module, so it cannot outlive it. Slang creates the layout
    /// once for each target and returns the same layout to every later call.
    ///
    /// Aborts the process, instead of returning an error, when Slang lets a
    /// C++ exception escape while it lays out the parameters, which it does
    /// for a parameter whose type is a chain of 127 or more nested
    /// structures, one inside the other, or of generic types nested as
    /// deeply, just under Slang's nesting limit of 128 levels, such as a
    /// `uniform` parameter or the element type of a `StructuredBuffer`. The
    /// crate then writes one line to standard error that names the call
    /// `IModule::getLayout` and aborts before any destructor runs.
    ///
    /// # Examples
    ///
    /// See the example of
    /// [`ProgramLayout::find_type_by_name`](crate::reflection::ProgramLayout::find_type_by_name).
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `target`
    /// belongs to another session and is past the end of this session's
    /// targets, which the crate checks before it calls Slang, and an error of
    /// kind [`ErrorKind::Compile`] with the diagnostics, or of kind
    /// [`ErrorKind::Internal`] without them, when Slang returns no layout.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn layout(&self, target: TargetIndex) -> Result<Compiled<ProgramLayout<'_>>> {
        component::layout(&self.raw, &self.session, target, "Module::layout")
    }

    /// Adopts a module that the caller obtained through the raw bindings of
    /// `slang-compiler-sys`.
    ///
    /// The module loaders of Slang return a module without a reference for
    /// the caller, so a caller that got the module from one adds a reference
    /// before it passes the module here.
    ///
    /// # Examples
    ///
    /// This example moves a module out of the crate and back:
    ///
    /// ```
    /// use slang_compiler::Module;
    ///
    /// fn round_trip(module: Module) -> Module {
    ///     let (raw, session) = module.into_raw();
    ///     // SAFETY: `raw` carries the one reference that `into_raw` gave up,
    ///     // of a module that `session` loaded, and only this thread uses it.
    ///     unsafe { Module::from_raw(raw, session) }
    /// }
    /// # let _ = round_trip;
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a live module that the Slang library of the build
    ///   tag [`slang_compiler_sys::SLANG_BUILD_TAG`] created.
    /// - The caller owns one reference to it and transfers that reference.
    /// - `session` is the session that loaded the module.
    /// - The module is not used on another thread while the returned value or
    ///   a clone of it exists.
    ///
    /// Section 11.7 of the design of this crate states these conditions.
    #[must_use]
    pub unsafe fn from_raw(raw: NonNull<IModule>, session: Session) -> Self {
        // SAFETY: the caller guarantees that `raw` points to a live module of
        // the pinned build and transfers one reference to it (Section 11.7 of
        // `docs/design/slang-compiler.md`).
        let raw = unsafe { Owned::adopt(raw) };
        Self { raw, session }
    }

    /// Gives up this handle's reference to the module without releasing it,
    /// and returns the pointer for use with the raw bindings of
    /// `slang-compiler-sys`, together with this handle's handle to its
    /// session.
    ///
    /// The caller then owns the reference and releases it. Keep the returned
    /// [`Session`] until after that release, or pass both back to
    /// [`Module::from_raw`], because a module of Slang reads its session in
    /// every call and does not keep it alive.
    ///
    /// # Examples
    ///
    /// See the example of [`Module::from_raw`].
    #[must_use]
    pub fn into_raw(self) -> (NonNull<IModule>, Session) {
        let Self { raw, session } = self;
        (raw.into_raw(), session)
    }

    /// Returns the pointer to the module without changing its reference
    /// count, for use with the raw bindings of `slang-compiler-sys`.
    ///
    /// The pointer is valid only while this handle lives. Every use of it is
    /// unsafe and follows the safety contract of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example passes the pointer on to code that uses the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::Module;
    ///
    /// fn raw_pointer(module: &Module) -> *mut slang_compiler_sys::IModule {
    ///     module.as_raw().as_ptr()
    /// }
    /// # let _ = raw_pointer;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<IModule> {
        self.raw.as_non_null()
    }

    /// Reads the number of dependency files. The caller raises the guard.
    fn dependency_file_count(&self) -> Result<usize> {
        // SAFETY: as in `name`.
        let count = unsafe {
            guard::abort_on_exception("IModule::getDependencyFileCount", || {
                IModule::getDependencyFileCount(self.raw.as_ptr())
            })
        };
        usize::try_from(count).map_err(|error| {
            Error::new(ErrorKind::Internal, Operation::ListDependencyFiles)
                .with_detail(format!("Slang reported the count {count}: {error}"))
        })
    }

    /// Reads the dependency file at `position`, or `None` when the position
    /// is no longer below the count that Slang reports, because Slang does
    /// not check the index of `getDependencyFilePath` (Section 3.6 of
    /// `docs/design/slang-compiler.md`).
    fn dependency_file_path(&self, position: usize) -> Option<Result<Option<&str>>> {
        let operation = Operation::ListDependencyFiles;
        guard::call("DependencyFilePaths::next", || {
            match self.dependency_file_count() {
                Ok(count) if position < count => {}
                Ok(_) => return None,
                Err(error) => return Some(Err(error)),
            }
            let index = match SlangInt32::try_from(position) {
                Ok(index) => index,
                Err(error) => {
                    return Some(Err(Error::new(ErrorKind::Internal, operation).with_detail(
                        format!("position {position} does not fit Slang's index: {error}"),
                    )));
                }
            };
            // SAFETY: as in `name`, with an index below the count that Slang
            // reported during the same call of the crate.
            let raw = unsafe {
                guard::abort_on_exception("IModule::getDependencyFilePath", || {
                    IModule::getDependencyFilePath(self.raw.as_ptr(), index)
                })
            };
            // SAFETY: the path is a string of a source file that the session's
            // source manager holds (Section 3.3), and `self` keeps the session
            // alive.
            Some(unsafe { borrowed_str(raw, operation, "a dependency file path of the module") })
        })
    }
}

impl fmt::Debug for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Module")
            .field("raw", &self.raw.as_non_null())
            .finish_non_exhaustive()
    }
}

/// An entry point of a module, a function of a shader that a graphics API
/// calls, like `main` in a program.
///
/// Get one from [`Module::entry_point`] or [`Module::entry_points`], and
/// compose it with its module through
/// [`Component::EntryPoint`](crate::Component::EntryPoint) to generate its
/// code.
///
/// An entry point holds its [`Session`]. It is neither [`Send`] nor
/// [`Sync`]. [`Clone`] returns another handle to the same entry point.
///
/// # Examples
///
/// See the example of [`Session::compose`](crate::Session::compose).
///
/// An entry point cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::EntryPoint>();
/// ```
///
/// An entry point cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::EntryPoint>();
/// ```
#[derive(Clone)]
pub struct EntryPoint {
    raw: Owned<IEntryPoint>,
    session: Session,
}

impl EntryPoint {
    pub(crate) fn raw(&self) -> &Owned<IEntryPoint> {
        &self.raw
    }

    pub(crate) fn session(&self) -> &Session {
        &self.session
    }

    /// Links the entry point on its own, as
    /// [`ComponentType::link`](crate::ComponentType::link) does for a
    /// component type.
    ///
    /// Slang links the module of the entry point with it, so the program
    /// generates the code of the entry point at position 0.
    ///
    /// # Examples
    ///
    /// This example compiles one entry point without a composition:
    ///
    /// ```
    /// use slang_compiler::{EntryPoint, TargetIndex};
    ///
    /// fn compile(entry_point: &EntryPoint, target: TargetIndex) -> slang_compiler::Result<Vec<u8>> {
    ///     let program = entry_point.link()?.into_value();
    ///     Ok(program.entry_point_code(0, target)?.into_value().to_vec())
    /// }
    /// # let _ = compile;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the errors of
    /// [`ComponentType::link`](crate::ComponentType::link).
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn link(&self) -> Result<Compiled<LinkedProgram>> {
        component::link(&self.raw, &self.session, 1, None, "EntryPoint::link")
    }

    /// Links the entry point on its own with compiler options, as
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options)
    /// does for a component type.
    ///
    /// The options take effect as the documentation of
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options)
    /// describes: which options change the generated code, which change
    /// nothing, and why options added after code was generated for a target
    /// have no effect on it.
    ///
    /// # Examples
    ///
    /// See the example of [`Module::link_with_options`].
    ///
    /// # Errors
    ///
    /// Returns the errors of
    /// [`ComponentType::link_with_options`](crate::ComponentType::link_with_options).
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn link_with_options(&self, options: &CompilerOptions) -> Result<Compiled<LinkedProgram>> {
        component::link(
            &self.raw,
            &self.session,
            1,
            Some(options),
            "EntryPoint::link_with_options",
        )
    }

    /// Returns the layout of the entry point for the target `target`, the root of
    /// its reflection, which describes where each parameter sits in memory
    /// and in the bindings of the target.
    ///
    /// A binding is a numbered slot of a graphics API through which a shader
    /// reaches one resource, such as a buffer or a texture. The layout
    /// borrows the entry point, so it cannot outlive it. Slang creates the layout
    /// once for each target and returns the same layout to every later call.
    ///
    /// Aborts the process, instead of returning an error, when Slang lets a
    /// C++ exception escape while it lays out the parameters, which it does
    /// for a parameter whose type is a chain of 127 or more nested
    /// structures, one inside the other, or of generic types nested as
    /// deeply, just under Slang's nesting limit of 128 levels, such as a
    /// `uniform` parameter or the element type of a `StructuredBuffer`. The
    /// crate then writes one line to standard error that names the call
    /// `IEntryPoint::getLayout` and aborts before any destructor runs.
    ///
    /// # Examples
    ///
    /// See the example of [`ProgramLayout`](crate::reflection::ProgramLayout).
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `target`
    /// belongs to another session and is past the end of this session's
    /// targets, which the crate checks before it calls Slang, and an error of
    /// kind [`ErrorKind::Compile`] with the diagnostics, or of kind
    /// [`ErrorKind::Internal`] without them, when Slang returns no layout.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn layout(&self, target: TargetIndex) -> Result<Compiled<ProgramLayout<'_>>> {
        component::layout(&self.raw, &self.session, target, "EntryPoint::layout")
    }

    /// Returns the function of the entry point, which describes its
    /// declaration in the source with its name, parameters, and return type,
    /// or `None` when Slang returns none.
    ///
    /// The function borrows the entry point, so it cannot outlive it.
    ///
    /// # Examples
    ///
    /// This example reads the names of the parameters of an entry point:
    ///
    /// ```
    /// use slang_compiler::EntryPoint;
    ///
    /// fn parameter_names(entry_point: &EntryPoint) -> slang_compiler::Result<Vec<String>> {
    ///     let mut names = Vec::new();
    ///     if let Some(function) = entry_point.function() {
    ///         for parameter in function.parameters() {
    ///             if let Some(name) = parameter.name()? {
    ///                 names.push(name.to_owned());
    ///             }
    ///         }
    ///     }
    ///     Ok(names)
    /// }
    /// # let _ = parameter_names;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn function(&self) -> Option<Function<'_>> {
        let raw = guard::call("EntryPoint::function", || {
            // SAFETY: `self.raw` points to a live entry point of
            // `self.session` (Sections 11.1 and 11.2 of
            // `docs/design/slang-compiler.md`), and the call runs with the
            // Slang call guard raised (SLC-28).
            unsafe {
                guard::abort_on_exception("IEntryPoint::getFunctionReflection", || {
                    IEntryPoint::getFunctionReflection(self.raw.as_ptr())
                })
            }
        });
        NonNull::new(raw.cast::<SlangReflectionFunction>()).map(|raw| {
            // SAFETY: `getFunctionReflection` returns a pointer that is not
            // counted to the function of the entry point, which lives in the
            // syntax trees that the session keeps, and `self` holds the session
            // for the borrow (Sections 5.3 and 11.3 of
            // `docs/design/slang-compiler.md`).
            unsafe { Function::new(raw, &self.session) }
        })
    }

    /// Adopts an entry point that the caller obtained through the raw
    /// bindings of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example moves an entry point out of the crate and back:
    ///
    /// ```
    /// use slang_compiler::EntryPoint;
    ///
    /// fn round_trip(entry_point: EntryPoint) -> EntryPoint {
    ///     let (raw, session) = entry_point.into_raw();
    ///     // SAFETY: `raw` carries the one reference that `into_raw` gave up,
    ///     // of an entry point of a module that `session` loaded, and only this
    ///     // thread uses it.
    ///     unsafe { EntryPoint::from_raw(raw, session) }
    /// }
    /// # let _ = round_trip;
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a live entry point that the Slang library of the
    ///   build tag [`slang_compiler_sys::SLANG_BUILD_TAG`] created.
    /// - The caller owns one reference to it and transfers that reference.
    /// - `session` is the session of the module of the entry point.
    /// - The entry point is not used on another thread while the returned
    ///   value or a clone of it exists.
    ///
    /// Section 11.7 of the design of this crate states these conditions.
    #[must_use]
    pub unsafe fn from_raw(raw: NonNull<IEntryPoint>, session: Session) -> Self {
        // SAFETY: the caller guarantees that `raw` points to a live entry point
        // of the pinned build and transfers one reference to it (Section 11.7
        // of `docs/design/slang-compiler.md`).
        let raw = unsafe { Owned::adopt(raw) };
        Self { raw, session }
    }

    /// Gives up this handle's reference to the entry point without releasing
    /// it, and returns the pointer for use with the raw bindings of
    /// `slang-compiler-sys`, together with this handle's handle to its
    /// session.
    ///
    /// The caller then owns the reference and releases it. Keep the returned
    /// [`Session`] until after that release, or pass both back to
    /// [`EntryPoint::from_raw`].
    ///
    /// # Examples
    ///
    /// See the example of [`EntryPoint::from_raw`].
    #[must_use]
    pub fn into_raw(self) -> (NonNull<IEntryPoint>, Session) {
        let Self { raw, session } = self;
        (raw.into_raw(), session)
    }

    /// Returns the pointer to the entry point without changing its reference
    /// count, for use with the raw bindings of `slang-compiler-sys`.
    ///
    /// The pointer is valid only while this handle lives. Every use of it is
    /// unsafe and follows the safety contract of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example passes the pointer on to code that uses the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::EntryPoint;
    ///
    /// fn raw_pointer(entry_point: &EntryPoint) -> *mut slang_compiler_sys::IEntryPoint {
    ///     entry_point.as_raw().as_ptr()
    /// }
    /// # let _ = raw_pointer;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<IEntryPoint> {
        self.raw.as_non_null()
    }
}

impl fmt::Debug for EntryPoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryPoint")
            .field("raw", &self.raw.as_non_null())
            .finish_non_exhaustive()
    }
}

/// An iterator over the modules of a session, which
/// [`Session::loaded_modules`] returns.
///
/// Each call of `next` asks Slang for the module at the next position. A
/// position at which Slang returns no module yields no item, so the iterator
/// does not implement [`ExactSizeIterator`].
///
/// # Examples
///
/// See the example of [`Session::loaded_modules`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::LoadedModules>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::LoadedModules>();
/// ```
#[derive(Clone, Debug)]
pub struct LoadedModules {
    session: Session,
    front: usize,
    back: usize,
}

impl LoadedModules {
    fn module_at(&self, position: usize) -> Option<Module> {
        guard::call("LoadedModules::next", || {
            let index = slang_compiler_sys::SlangInt::try_from(position).ok()?;
            // SAFETY: `self.session` holds a live session, used only on this
            // thread, and the call runs with the Slang call guard raised.
            // `getLoadedModule` checks its index (Section 3.6 of
            // `docs/design/slang-compiler.md`).
            let raw = unsafe {
                guard::abort_on_exception("ISession::getLoadedModule", || {
                    ISession::getLoadedModule(self.session.raw().as_ptr(), index)
                })
            };
            let raw = NonNull::new(raw)?;
            // SAFETY: `getLoadedModule` returns a borrowed pointer to a module
            // that the session holds, and the session is alive (Section 5.3).
            let raw = unsafe { Owned::from_borrowed(raw) };
            Some(Module {
                raw,
                session: self.session.clone(),
            })
        })
    }
}

impl Iterator for LoadedModules {
    type Item = Module;

    fn next(&mut self) -> Option<Module> {
        while self.front < self.back {
            let position = self.front;
            self.front += 1;
            if let Some(module) = self.module_at(position) {
                return Some(module);
            }
        }
        None
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.back - self.front))
    }
}

impl DoubleEndedIterator for LoadedModules {
    fn next_back(&mut self) -> Option<Module> {
        while self.front < self.back {
            self.back -= 1;
            if let Some(module) = self.module_at(self.back) {
                return Some(module);
            }
        }
        None
    }
}

impl FusedIterator for LoadedModules {}

/// An iterator over the entry points that a module defines, which
/// [`Module::entry_points`] returns.
///
/// Each item is a [`Result`], because Slang reports the failure to return an
/// entry point through a result code.
///
/// # Examples
///
/// See the example of [`Module::entry_points`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::EntryPoints>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::EntryPoints>();
/// ```
#[derive(Clone, Debug)]
pub struct EntryPoints {
    module: Module,
    front: usize,
    back: usize,
}

impl EntryPoints {
    fn entry_point_at(&self, position: usize) -> Result<EntryPoint> {
        let operation = Operation::ListEntryPoints;
        guard::call("EntryPoints::next", || {
            let index = SlangInt32::try_from(position).map_err(|error| {
                Error::new(ErrorKind::Internal, operation).with_detail(format!(
                    "position {position} does not fit Slang's index: {error}"
                ))
            })?;
            let mut out: *mut IEntryPoint = ptr::null_mut();
            // SAFETY: `self.module` holds a live module, used only on this
            // thread, `out` is a live out-pointer, and the call runs with the
            // Slang call guard raised. `getDefinedEntryPoint` checks its index
            // (Section 3.6 of `docs/design/slang-compiler.md`).
            let result = unsafe {
                guard::abort_on_exception("IModule::getDefinedEntryPoint", || {
                    IModule::getDefinedEntryPoint(self.module.raw.as_ptr(), index, &raw mut out)
                })
            };
            // SAFETY: `getDefinedEntryPoint` returns a new reference or leaves
            // the output null (Section 5.3).
            let out = unsafe { Owned::adopt_output(out) };
            mapping::check(operation, result, Diagnostics::default())?;
            let raw = out.ok_or_else(|| mapping::null(operation, Diagnostics::default()))?;
            Ok(EntryPoint {
                raw,
                session: self.module.session.clone(),
            })
        })
    }
}

impl Iterator for EntryPoints {
    type Item = Result<EntryPoint>;

    fn next(&mut self) -> Option<Result<EntryPoint>> {
        if self.front >= self.back {
            return None;
        }
        let position = self.front;
        self.front += 1;
        Some(self.entry_point_at(position))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let remaining = self.back - self.front;
        (remaining, Some(remaining))
    }
}

impl DoubleEndedIterator for EntryPoints {
    fn next_back(&mut self) -> Option<Result<EntryPoint>> {
        if self.front >= self.back {
            return None;
        }
        self.back -= 1;
        Some(self.entry_point_at(self.back))
    }
}

impl ExactSizeIterator for EntryPoints {}

impl FusedIterator for EntryPoints {}

/// An iterator over the paths of the files that a module depends on, which
/// [`Module::dependency_file_paths`] returns.
///
/// Each item is `None` for a file without a path and an error for a path
/// that is not valid UTF-8. The iterator checks the count again before each
/// item, so it ends early if the count shrinks, and it does not implement
/// [`ExactSizeIterator`].
///
/// # Examples
///
/// See the example of [`Module::dependency_file_paths`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::DependencyFilePaths<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::DependencyFilePaths<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct DependencyFilePaths<'module> {
    module: &'module Module,
    front: usize,
    back: usize,
}

impl<'module> Iterator for DependencyFilePaths<'module> {
    type Item = Result<Option<&'module str>>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.front >= self.back {
            return None;
        }
        let position = self.front;
        self.front += 1;
        let item = self.module.dependency_file_path(position);
        if item.is_none() {
            self.front = self.back;
        }
        item
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, Some(self.back - self.front))
    }
}

impl DoubleEndedIterator for DependencyFilePaths<'_> {
    fn next_back(&mut self) -> Option<Self::Item> {
        while self.front < self.back {
            self.back -= 1;
            if let Some(item) = self.module.dependency_file_path(self.back) {
                return Some(item);
            }
        }
        None
    }
}

impl FusedIterator for DependencyFilePaths<'_> {}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::Module;
    use crate::fake::{
        self, Event, FAKE_CODE, FAKE_DIAGNOSTICS, FAKE_ENTRY_POINT, FAKE_LAYOUT, FAKE_MODULE,
        FAKE_SESSION, FakeBlob, FakeSession, Layout, Load, Log, panic_message,
    };
    use crate::{ErrorKind, Operation, guard};

    const SOURCE: &str = "[shader(\"compute\")] void main() {}";

    fn load(session: &crate::Session) -> Module {
        session
            .load_module_from_source("kernel", "kernel.slang", SOURCE)
            .expect("the fake loads the module")
            .into_value()
    }

    #[test]
    fn a_loaded_module_adds_a_reference_and_outlives_the_session_handles() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        assert_eq!(
            log.events()
                .iter()
                .filter(
                    |event| matches!(event, Event::AddRef { object, .. } if *object == FAKE_MODULE)
                )
                .collect::<Vec<_>>(),
            [&Event::add_ref(FAKE_MODULE, 2)]
        );
        drop(session);
        drop(global);
        let entry_point = module
            .entry_point(FAKE_ENTRY_POINT)
            .expect("the module is alive");
        drop(entry_point);
        drop(module);
        assert_eq!(
            fake::releases(&log, &[FAKE_MODULE, FAKE_SESSION]),
            [
                Event::release(FAKE_SESSION, 1),
                Event::release(FAKE_SESSION, 1),
                Event::release(FAKE_MODULE, 1),
                Event::release(FAKE_SESSION, 0),
                Event::freed(FAKE_SESSION),
                Event::release(FAKE_MODULE, 0),
                Event::freed(FAKE_MODULE),
            ]
        );
    }

    #[test]
    fn a_second_load_of_a_name_returns_the_same_module() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let first = load(&session);
        let second = session
            .load_module("kernel")
            .expect("the fake returns the loaded module")
            .into_value();
        assert_eq!(first.as_raw(), second.as_raw());
        let loaded = session
            .loaded_modules()
            .expect("the fake counts its modules")
            .collect::<Vec<_>>();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded.first().map(Module::as_raw), Some(first.as_raw()));
    }

    #[test]
    fn a_load_with_diagnostics_returns_them_with_the_module_and_releases_the_blob() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        // SAFETY: `session` keeps the fake session alive during the call.
        unsafe { FakeSession::script(session.as_raw()) }
            .load
            .set(Load::ModuleWithDiagnostics);
        let compiled = session
            .load_module_from_source("kernel", "kernel.slang", SOURCE)
            .expect("the fake loads the module");
        assert_eq!(compiled.diagnostics.as_str(), FAKE_DIAGNOSTICS);
        assert!(log.events().contains(&Event::freed("diagnostics")));
    }

    #[test]
    fn a_null_load_with_diagnostics_is_a_compile_error() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        // SAFETY: `session` keeps the fake session alive during the call.
        unsafe { FakeSession::script(session.as_raw()) }
            .load
            .set(Load::NullWithDiagnostics);
        let error = session.load_module("kernel").expect_err("the load fails");
        assert_eq!(error.kind(), ErrorKind::Compile);
        assert_eq!(error.operation(), Operation::LoadModule);
        assert_eq!(error.code(), None);
        assert_eq!(error.diagnostics().as_str(), FAKE_DIAGNOSTICS);
        assert!(log.events().contains(&Event::freed("diagnostics")));
    }

    #[test]
    fn a_null_load_without_diagnostics_is_not_found_and_names_the_cause() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        // SAFETY: `session` keeps the fake session alive during the call.
        unsafe { FakeSession::script(session.as_raw()) }
            .load
            .set(Load::Null);
        let error = session.load_module("missing").expect_err("the load fails");
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert!(error.diagnostics().is_empty());
        let text = error.to_string();
        assert!(text.starts_with("cannot load module `missing`: "), "{text}");
        assert!(
            text.contains("an earlier load of the name failed"),
            "{text}"
        );
        assert!(text.contains("a new session loads it again"), "{text}");
    }

    fn set_count(session: &crate::Session, count: Option<i16>) {
        // SAFETY: `session` keeps the fake session alive during the call.
        unsafe { FakeSession::script(session.as_raw()) }
            .count
            .set(count);
    }

    fn assert_negative_count<T: std::fmt::Debug>(result: crate::Result<T>, operation: Operation) {
        let error = result.expect_err("a negative count fails");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.operation(), operation);
        assert!(
            error.to_string().contains("Slang reported the count -1"),
            "{error}"
        );
    }

    #[test]
    fn a_negative_count_of_loaded_modules_entry_points_or_dependencies_is_internal() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        set_count(&session, Some(-1));
        assert_negative_count(session.loaded_modules(), Operation::ListLoadedModules);
        assert_negative_count(module.entry_points(), Operation::ListEntryPoints);
        assert_negative_count(
            module.dependency_file_paths(),
            Operation::ListDependencyFiles,
        );
    }

    #[test]
    fn dependency_paths_end_when_the_count_shrinks_during_the_iteration() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        let mut paths = module
            .dependency_file_paths()
            .expect("the fake counts two files");
        assert_eq!(paths.size_hint(), (0, Some(2)));
        assert_eq!(
            paths.next().map(|path| path.expect("the path is UTF-8")),
            Some(Some("fake.slang"))
        );
        set_count(&session, Some(1));
        let calls = fake::calls(&log);
        assert!(paths.next().is_none());
        assert_eq!(fake::calls(&log), calls);
        assert!(paths.next().is_none());
        assert_eq!(paths.size_hint(), (0, Some(0)));
    }

    #[test]
    fn a_dependency_count_that_turns_negative_during_the_iteration_yields_an_error() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        let mut paths = module
            .dependency_file_paths()
            .expect("the fake counts two files");
        set_count(&session, Some(-1));
        let error = paths
            .next()
            .expect("the iterator reports the count")
            .expect_err("a negative count fails");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.operation(), Operation::ListDependencyFiles);
    }

    #[test]
    fn a_null_ir_blob_is_an_internal_error_before_the_load() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let before = fake::calls(&log);
        // SAFETY: the blob creation returns null, which the seam allows.
        let error = unsafe {
            session.load_module_from_ir_with("kernel", "kernel.slang-module", b"ir", |_| {
                std::ptr::null_mut()
            })
        }
        .expect_err("a null blob fails the load");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.operation(), Operation::LoadModule);
        assert_eq!(error.code(), None);
        assert_eq!(
            error.to_string(),
            "cannot load module `kernel`: Slang could not copy the intermediate representation into a blob"
        );
        assert_eq!(fake::calls(&log), before);
    }

    #[test]
    fn an_ir_blob_is_released_after_the_load_returns() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        // SAFETY: the blob creation returns a new fake blob with one
        // reference, which holds a copy of the bytes.
        let module = unsafe {
            session.load_module_from_ir_with("kernel", "kernel.slang-module", b"ir", |ir| {
                FakeBlob::create(&log, "ir", ir).as_ptr()
            })
        }
        .expect("the fake loads the module")
        .into_value();
        let events = log.events();
        let loaded = events
            .iter()
            .position(|event| *event == Event::call(FAKE_SESSION, "loadModule"))
            .expect("the session loads the module");
        let freed = events
            .iter()
            .position(|event| *event == Event::freed("ir"))
            .expect("the blob is freed");
        assert!(loaded < freed, "{events:?}");
        drop(module);
    }

    #[test]
    fn a_nul_byte_in_a_load_fails_before_any_call() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let before = fake::calls(&log);
        for result in [
            session.load_module("a\0b"),
            session.load_module_from_source("a", "a\0.slang", SOURCE),
            session.load_module_from_source("a", "a.slang", "\0"),
            session.load_module_from_ir("a\0", "a.slang-module", b"ir"),
        ] {
            let error = result.expect_err("a NUL byte is rejected");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert_eq!(error.operation(), Operation::LoadModule);
        }
        let error = session
            .load_module_from_ir("a", "a.slang-module", &[])
            .expect_err("an empty IR slice is rejected");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(fake::calls(&log), before);
    }

    #[test]
    fn a_module_reports_its_name_path_entry_points_and_dependencies() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        assert_eq!(module.name().expect("valid UTF-8"), Some("fake_module"));
        assert_eq!(module.file_path().expect("valid UTF-8"), None);
        let entry_points = module.entry_points().expect("the fake counts one");
        assert_eq!(entry_points.len(), 1);
        assert_eq!(entry_points.flatten().count(), 1);
        let paths = module
            .dependency_file_paths()
            .expect("the fake counts two")
            .collect::<crate::Result<Vec<_>>>()
            .expect("valid UTF-8");
        assert_eq!(paths, [Some("fake.slang"), None]);
        let reversed = module
            .dependency_file_paths()
            .expect("the fake counts two")
            .rev()
            .collect::<crate::Result<Vec<_>>>()
            .expect("valid UTF-8");
        assert_eq!(reversed, [None, Some("fake.slang")]);
        let ir = module.serialize().expect("the fake serializes");
        assert_eq!(ir.as_bytes(), FAKE_CODE);
    }

    #[test]
    fn entry_point_of_an_unknown_name_is_not_found() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        let error = module
            .entry_point("other")
            .expect_err("no such entry point");
        assert_eq!(error.kind(), ErrorKind::NotFound);
        assert_eq!(error.operation(), Operation::FindEntryPoint);
        assert_eq!(
            error.code(),
            Some(slang_compiler_sys::SlangResult::SLANG_FAIL.0)
        );
        assert_eq!(
            error.to_string(),
            format!(
                "cannot find entry point `other` (Slang result code {:#010x})",
                slang_compiler_sys::SlangResult::SLANG_FAIL.0
            )
        );
    }

    #[test]
    fn entry_point_with_stage_checks_the_stage_and_returns_a_new_reference() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        let compiled = module
            .entry_point_with_stage(FAKE_ENTRY_POINT, crate::Stage::Compute)
            .expect("the fake accepts the compute stage");
        assert!(compiled.diagnostics.is_empty());
        assert!(
            log.events()
                .contains(&Event::call(FAKE_MODULE, "findAndCheckEntryPoint"))
        );
        let program = compiled
            .value
            .link()
            .expect("the fake links the entry point")
            .into_value();
        assert_eq!(program.entry_point_count(), 1);
        drop(program);
        drop(compiled);
        drop(module);
        drop(session);
        drop(global);
        assert!(log.events().contains(&Event::freed("entry_point")));
    }

    #[test]
    fn entry_point_with_stage_of_another_stage_is_a_compile_error_with_the_diagnostics() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        let error = module
            .entry_point_with_stage(FAKE_ENTRY_POINT, crate::Stage::Vertex)
            .expect_err("the fake rejects the vertex stage");
        assert_eq!(error.kind(), ErrorKind::Compile);
        assert_eq!(error.operation(), Operation::FindEntryPoint);
        assert_eq!(
            error.code(),
            Some(slang_compiler_sys::SlangResult::SLANG_FAIL.0)
        );
        assert_eq!(error.diagnostics().as_str(), FAKE_DIAGNOSTICS);
        assert!(error.to_string().contains("`main`"), "{error}");
        assert!(log.events().contains(&Event::freed("diagnostics")));
    }

    #[test]
    fn entry_point_with_stage_rejects_a_nul_byte_before_any_call() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        let before = log.events().len();
        let error = module
            .entry_point_with_stage("ma\0in", crate::Stage::Compute)
            .expect_err("a NUL byte is rejected");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::FindEntryPoint);
        assert_eq!(log.events().len(), before);
    }

    #[test]
    fn a_load_while_the_guard_is_raised_panics_before_calling_slang() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let before = fake::calls(&log);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            let _raised = guard::raise();
            drop(session.load_module("kernel"));
        }));
        let message = panic_message(&*outcome.expect_err("the reentrant call panics"));
        assert!(
            message.starts_with("reentrant call: Session::load_module "),
            "{message}"
        );
        assert_eq!(fake::calls(&log), before);
    }

    #[test]
    fn into_raw_and_from_raw_hand_a_module_over() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        let raw = module.as_raw();
        let (pointer, parent) = module.into_raw();
        assert_eq!(pointer, raw);
        // SAFETY: `pointer` carries the reference that `into_raw` gave up, of a
        // module that `parent` loaded, on this thread.
        let module = unsafe { Module::from_raw(pointer, parent) };
        assert_eq!(module.as_raw(), raw);
    }

    #[test]
    fn entry_point_function_is_none_for_null_and_borrows_the_entry_point() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 1);
        let module = load(&session);
        let entry_point = module
            .entry_point(FAKE_ENTRY_POINT)
            .expect("the fake finds its entry point");
        assert!(entry_point.function().is_none());
        // SAFETY: `session` keeps the fake session alive while the script
        // is used.
        unsafe { FakeSession::script(session.as_raw()) }
            .function
            .set(true);
        let function = entry_point.function().expect("the fake returns a function");
        assert_eq!(
            function.as_raw().as_ptr().cast_const().cast::<u8>(),
            std::ptr::from_ref(&FAKE_LAYOUT)
        );
        let panic = catch_unwind(AssertUnwindSafe(|| {
            let _raised = guard::raise();
            let _function = entry_point.function();
        }))
        .expect_err("a reentrant call panics");
        assert!(
            panic_message(&*panic).starts_with("reentrant call: EntryPoint::function"),
            "{}",
            panic_message(&*panic)
        );
    }

    #[test]
    fn module_and_entry_point_layouts_reach_the_fake_with_their_target() {
        let log = Log::new();
        let global = fake::global_session(&log);
        let session = fake::session(&global, 2);
        let module = load(&session);
        // SAFETY: `session` keeps the fake session alive while the script
        // is used.
        unsafe { FakeSession::script(session.as_raw()) }
            .layout
            .set(Layout::WithDiagnostics);
        let target = session.target_index(1).expect("a second target");
        let layout = module.layout(target).expect("the fake lays out");
        assert_eq!(layout.diagnostics.as_str(), FAKE_DIAGNOSTICS);
        assert!(
            log.events().contains(&Event::check_target(FAKE_MODULE, 1)),
            "{:?}",
            log.events()
        );
        let entry_point = module
            .entry_point(FAKE_ENTRY_POINT)
            .expect("the fake finds its entry point");
        let layout = entry_point.layout(target).expect("the fake lays out");
        assert_eq!(
            layout.value.as_raw().as_ptr().cast_const().cast::<u8>(),
            std::ptr::from_ref(&FAKE_LAYOUT)
        );
    }
}
