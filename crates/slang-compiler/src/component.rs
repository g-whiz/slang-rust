use std::fmt;
use std::ptr::{self, NonNull};

use slang_compiler_sys::{
    CompilerOptionEntry, IComponentType, IEntryPoint, IMetadata, IModule, ISlangBlob,
    ITypeConformance, SlangInt, SlangResult, SpecializationArg as RawSpecializationArg,
    SpecializationArg_Kind, SpecializationArg_anon0,
};

use crate::blob::{self, Blob, Metadata};
use crate::error::{Compiled, Error, ErrorKind, Operation, Result};
use crate::guard;
use crate::module::{EntryPoint, Module};
use crate::options::{CompilerOptions, PreparedOptions};
use crate::owned::{ComInterface, Owned};
use crate::reflection::Type;
use crate::session::{Session, TargetIndex};

/// The linking calls of a generated interface type that derives from
/// `IComponentType`.
///
/// # Safety
///
/// An implementer guarantees that each function calls the method of the same
/// name of `IComponentType` through the generated thunk of `Self`, which calls
/// through the vtable of the object that `this` points to (SCS-10 of
/// `docs/design/slang-compiler-sys.md`). Breaking this is undefined behaviour.
pub(crate) unsafe trait ComponentInterface: ComInterface {
    /// Calls `link` on the object.
    ///
    /// # Safety
    ///
    /// `this` points to a live object that implements `Self`, `out` and
    /// `diagnostics` are live out-pointers, and the caller raises the Slang
    /// call guard. Breaking any of them is undefined behaviour.
    unsafe fn link(
        this: *mut Self,
        out: *mut *mut IComponentType,
        diagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult;

    /// Calls `linkWithOptions` on the object.
    ///
    /// # Safety
    ///
    /// As for [`ComponentInterface::link`], and `entries` points to `count`
    /// live option entries whose strings are live C strings, or is null when
    /// `count` is zero. Breaking any of them is undefined behaviour.
    unsafe fn link_with_options(
        this: *mut Self,
        out: *mut *mut IComponentType,
        count: u32,
        entries: *const CompilerOptionEntry,
        diagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult;
}

macro_rules! component_interfaces {
    ($($interface:ident),+ $(,)?) => {$(
        // SAFETY: `$interface` derives from `IComponentType`, and its
        // generated thunks call `link` and `linkWithOptions` through the
        // object's own vtable (COM-9, SCS-10).
        unsafe impl ComponentInterface for $interface {
            unsafe fn link(
                this: *mut Self,
                out: *mut *mut IComponentType,
                diagnostics: *mut *mut ISlangBlob,
            ) -> SlangResult {
                // SAFETY: the caller guarantees that `this` is live and that
                // both out-pointers are live.
                unsafe {
                    guard::abort_on_exception(concat!(stringify!($interface), "::link"), || {
                        $interface::link(this, out, diagnostics)
                    })
                }
            }

            unsafe fn link_with_options(
                this: *mut Self,
                out: *mut *mut IComponentType,
                count: u32,
                entries: *const CompilerOptionEntry,
                diagnostics: *mut *mut ISlangBlob,
            ) -> SlangResult {
                // SAFETY: the caller guarantees that `this` is live, that both
                // out-pointers are live, and that `entries` holds `count` live
                // entries.
                unsafe {
                    guard::abort_on_exception(
                        concat!(stringify!($interface), "::linkWithOptions"),
                        || $interface::linkWithOptions(this, out, count, entries, diagnostics),
                    )
                }
            }
        }
    )+};
}

component_interfaces!(IModule, IEntryPoint, IComponentType, ITypeConformance);

/// Links the component type `raw` of `session`, with `options` for
/// `linkWithOptions` or without options for `link`, for the public method
/// `method`.
pub(crate) fn link<I: ComponentInterface>(
    raw: &Owned<I>,
    session: &Session,
    entry_point_count: usize,
    options: Option<&CompilerOptions>,
    method: &str,
) -> Result<Compiled<LinkedProgram>> {
    let operation = Operation::Link;
    let mut prepared = PreparedOptions::default();
    if let Some(options) = options {
        prepared.append(options, operation)?;
    }
    let entries = prepared.raw(operation)?;
    guard::call(method, || {
        let mut out: *mut IComponentType = ptr::null_mut();
        let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
        let result = if options.is_some() {
            // SAFETY: `raw` points to a live component type of `session`, which
            // the caller's wrapper holds (Section 11.2 of
            // `docs/design/slang-compiler.md`), `out` and `diagnostics` are
            // live out-pointers, `entries` owns the entry array and borrows
            // `prepared`, which owns their strings until the call returns, and
            // the call runs with the Slang call guard raised (SLC-28).
            unsafe {
                I::link_with_options(
                    raw.as_ptr(),
                    &raw mut out,
                    entries.count(),
                    entries.as_ptr(),
                    &raw mut diagnostics,
                )
            }
        } else {
            // SAFETY: as for `linkWithOptions`, without the entries.
            unsafe { I::link(raw.as_ptr(), &raw mut out, &raw mut diagnostics) }
        };
        // SAFETY: `link` and `linkWithOptions` return a new reference, which
        // can be the input object itself, so the crate adopts it and assumes
        // nothing about whether it is a distinct object, and every
        // diagnostics output carries a new reference or is null (Section 5.3
        // of `docs/design/slang-compiler.md`).
        let Compiled {
            value: raw,
            diagnostics,
        } = unsafe { blob::take_output(operation, result, out, diagnostics) }?;
        Ok(Compiled {
            value: LinkedProgram {
                raw,
                session: session.clone(),
                entry_point_count,
            },
            diagnostics,
        })
    })
}

/// One part of a composition, a borrow of a module, an entry point, a
/// component type, or a type conformance.
///
/// [`Session::compose`] takes a slice of them and combines the parts into
/// one [`ComponentType`]. The enumeration is non-exhaustive, so a later
/// version can add kinds of part, and a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Session::compose`].
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
#[expect(
    clippy::enum_variant_names,
    reason = "each variant is named after the type that it borrows, and Section 6.6 of docs/design/slang-compiler.md names the variant ComponentType after the type ComponentType"
)]
pub enum Component<'a> {
    /// A module, one unit of Slang source.
    Module(&'a Module),
    /// An entry point, a function that a graphics API calls.
    EntryPoint(&'a EntryPoint),
    /// A component type that an earlier composition or specialization
    /// returned.
    ComponentType(&'a ComponentType),
    /// A type conformance, the record that a type implements a Slang
    /// interface.
    TypeConformance(&'a TypeConformance),
}

impl<'a> Component<'a> {
    fn session(self) -> &'a Session {
        match self {
            Self::Module(module) => module.session(),
            Self::EntryPoint(entry_point) => entry_point.session(),
            Self::ComponentType(component) => &component.session,
            Self::TypeConformance(conformance) => &conformance.session,
        }
    }

    fn entry_point_count(self) -> usize {
        match self {
            Self::Module(_) | Self::TypeConformance(_) => 0,
            Self::EntryPoint(_) => 1,
            Self::ComponentType(component) => component.entry_point_count,
        }
    }

    /// Returns an owning pointer to the part as an `IComponentType`, through
    /// the cast of Section 5.4 of `docs/design/slang-compiler.md`. The caller
    /// raises the Slang call guard.
    fn component_type(self, operation: Operation) -> Result<Owned<IComponentType>> {
        match self {
            Self::Module(module) => module.raw().cast(operation),
            Self::EntryPoint(entry_point) => entry_point.raw().cast(operation),
            Self::ComponentType(component) => Ok(component.raw.clone()),
            Self::TypeConformance(conformance) => conformance.raw.cast(operation),
        }
    }
}

impl Session {
    /// Combines modules, entry points, and other component types into one
    /// component type, which is called composition.
    ///
    /// A component type is a unit of Slang code that can be linked, such as a
    /// module, an entry point, or a composition of them. To generate code for
    /// an entry point, compose it with its module and link the result. The
    /// entry points of the result are those of the parts, in the order of
    /// the parts, and [`LinkedProgram::entry_point_code`] selects them by that
    /// position.
    ///
    /// # Examples
    ///
    /// This example compiles a compute shader to SPIR-V, the binary shader
    /// format of Vulkan. A compute shader runs as a general computation on
    /// the graphics processing unit (GPU). It needs the Slang shared library
    /// at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{Component, GlobalSession, Session, Target, TargetFormat};
    ///
    /// let global = GlobalSession::new()?;
    /// let session = Session::builder(&global)
    ///     .target(Target::new(TargetFormat::Spirv))
    ///     .build()?;
    /// let source = "[shader(\"compute\")] [numthreads(1, 1, 1)] void main() {}";
    /// let module = session
    ///     .load_module_from_source("kernel", "kernel.slang", source)?
    ///     .into_value();
    /// let entry_point = module.entry_point("main")?;
    /// let composed = session
    ///     .compose(&[Component::Module(&module), Component::EntryPoint(&entry_point)])?
    ///     .into_value();
    /// let program = composed.link()?.into_value();
    /// let code = program.entry_point_code(0, session.target_index(0)?)?.into_value();
    /// println!("{} bytes of SPIR-V", code.as_bytes().len());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when a part
    /// belongs to another session, which the crate checks before it calls
    /// Slang, and the kind that Slang's result code maps to when Slang cannot
    /// compose the parts, such as [`ErrorKind::Compile`] with the
    /// diagnostics.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn compose(&self, components: &[Component<'_>]) -> Result<Compiled<ComponentType>> {
        let operation = Operation::Compose;
        let mut entry_point_count: usize = 0;
        for (position, component) in components.iter().enumerate() {
            if !component.session().is_same(self) {
                return Err(other_session(operation, &format!("component {position}")));
            }
            entry_point_count = entry_point_count
                .checked_add(component.entry_point_count())
                .ok_or_else(|| {
                    Error::new(ErrorKind::InvalidArgument, operation).with_detail(
                        "the components have more entry points than a count holds".to_owned(),
                    )
                })?;
        }
        let count = SlangInt::try_from(components.len()).map_err(|error| {
            Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
                "{} components are more than Slang takes: {error}",
                components.len()
            ))
        })?;
        guard::call("Session::compose", || {
            let parts = components
                .iter()
                .map(|component| component.component_type(operation))
                .collect::<Result<Vec<_>>>()?;
            let pointers = parts.iter().map(Owned::as_ptr).collect::<Vec<_>>();
            let mut out: *mut IComponentType = ptr::null_mut();
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: `self.raw()` points to a live session (Section 11.1 of
            // `docs/design/slang-compiler.md`), used only on this thread
            // (Section 11.2). `pointers` holds `count` live component types of
            // this session, which `parts` keeps alive until the call returns,
            // `out` and `diagnostics` are live out-pointers, and the call runs
            // with the Slang call guard raised (SLC-28).
            let result = unsafe {
                guard::abort_on_exception("ISession::createCompositeComponentType", || {
                    slang_compiler_sys::ISession::createCompositeComponentType(
                        self.raw().as_ptr(),
                        pointers.as_ptr(),
                        count,
                        &raw mut out,
                        &raw mut diagnostics,
                    )
                })
            };
            // SAFETY: `createCompositeComponentType` returns a new reference,
            // which can be an input with one more reference, and every
            // diagnostics output carries a new reference or is null (Section
            // 5.3 of `docs/design/slang-compiler.md`).
            let Compiled {
                value: raw,
                diagnostics,
            } = unsafe { blob::take_output(operation, result, out, diagnostics) }?;
            Ok(Compiled {
                value: ComponentType {
                    raw,
                    session: self.clone(),
                    entry_point_count,
                },
                diagnostics,
            })
        })
    }

    /// Records that the type `ty` implements the Slang interface `interface`,
    /// an `interface` declaration of Slang code, as a type conformance.
    ///
    /// Compose type conformances with the code that calls the interface to
    /// choose which implementations dynamic dispatch, the choice of an
    /// implementation at run time, can reach. `id_override` sets the number
    /// that the generated code uses for `ty`, and `None` lets Slang choose it.
    ///
    /// # Examples
    ///
    /// This example records that a structure implements an interface and
    /// composes the record with a module and its entry point. It takes the
    /// two types as arguments, because this version finds types only through
    /// the raw bindings, as [`Type::from_raw`] describes:
    ///
    /// ```
    /// use slang_compiler::reflection::Type;
    /// use slang_compiler::{Component, ComponentType, EntryPoint, Module, Session};
    ///
    /// fn with_square(
    ///     session: &Session,
    ///     module: &Module,
    ///     entry_point: &EntryPoint,
    ///     square: Type<'_>,
    ///     shape: Type<'_>,
    /// ) -> slang_compiler::Result<ComponentType> {
    ///     let conformance = session.type_conformance(square, shape, None)?.into_value();
    ///     Ok(session
    ///         .compose(&[
    ///             Component::Module(module),
    ///             Component::EntryPoint(entry_point),
    ///             Component::TypeConformance(&conformance),
    ///         ])?
    ///         .into_value())
    /// }
    /// # let _ = with_square;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `ty` or
    /// `interface` belongs to another session, or `id_override` is negative
    /// or does not fit Slang's integer type, which the crate checks before it
    /// calls Slang. Returns the kind that Slang's result code maps to when
    /// Slang fails, which for a type that does not implement the interface is
    /// [`ErrorKind::Internal`], because Slang reports that case without a
    /// reason.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn type_conformance(
        &self,
        ty: Type<'_>,
        interface: Type<'_>,
        id_override: Option<i64>,
    ) -> Result<Compiled<TypeConformance>> {
        let operation = Operation::CreateTypeConformance;
        if !ty.session().is_same(self) {
            return Err(other_session(operation, "the type"));
        }
        if !interface.session().is_same(self) {
            return Err(other_session(operation, "the interface"));
        }
        let id = match id_override {
            None => -1,
            Some(id) => {
                let converted = SlangInt::try_from(id).ok().filter(|id| *id >= 0);
                converted.ok_or_else(|| {
                    Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
                        "the identifier {id} is negative or does not fit Slang's integer type"
                    ))
                })?
            }
        };
        guard::call("Session::type_conformance", || {
            let mut out: *mut ITypeConformance = ptr::null_mut();
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: `self.raw()` points to a live session, used only on this
            // thread (Sections 11.1 and 11.2 of
            // `docs/design/slang-compiler.md`). `ty` and `interface` are
            // non-null types of this session, which the check above
            // established and which their borrows keep alive (Section 11.3),
            // `out` and `diagnostics` are live out-pointers, and the call runs
            // with the Slang call guard raised (SLC-28).
            let result = unsafe {
                guard::abort_on_exception("ISession::createTypeConformanceComponentType", || {
                    slang_compiler_sys::ISession::createTypeConformanceComponentType(
                        self.raw().as_ptr(),
                        ty.as_raw().as_ptr(),
                        interface.as_raw().as_ptr(),
                        &raw mut out,
                        id,
                        &raw mut diagnostics,
                    )
                })
            };
            // SAFETY: `createTypeConformanceComponentType` returns a new
            // reference, and every diagnostics output carries a new reference
            // or is null (Section 5.3 of `docs/design/slang-compiler.md`).
            let Compiled {
                value: raw,
                diagnostics,
            } = unsafe { blob::take_output(operation, result, out, diagnostics) }?;
            Ok(Compiled {
                value: TypeConformance {
                    raw,
                    session: self.clone(),
                },
                diagnostics,
            })
        })
    }
}

/// The error of an argument of another session (SLC-7).
pub(crate) fn other_session(operation: Operation, what: &str) -> Error {
    Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
        "{what} belongs to another session, and Slang resolves it only in its own session"
    ))
}

/// A type argument of [`ComponentType::specialize`].
///
/// The enumeration is non-exhaustive, so a later version can add kinds of
/// argument, and a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`ComponentType::specialize`].
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum SpecializationArg<'a> {
    /// A type, which fills in a generic parameter or a parameter of a Slang
    /// interface type.
    Type(Type<'a>),
}

/// A unit of Slang code that a composition or a specialization returned,
/// which can be specialized and linked.
///
/// A component type is a unit of Slang code that can be linked, such as a
/// module, an entry point, or a composition of them. Get one from
/// [`Session::compose`] or [`ComponentType::specialize`], and link it with
/// [`ComponentType::link`] to get a [`LinkedProgram`], which generates code.
/// Linking does not consume it, so it stays usable.
///
/// A component type holds its [`Session`]. It is neither [`Send`] nor
/// [`Sync`]. [`Clone`] returns another handle to the same component type.
///
/// # Examples
///
/// See the example of [`Session::compose`].
///
/// A component type cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::ComponentType>();
/// ```
///
/// A component type cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::ComponentType>();
/// ```
#[derive(Clone)]
pub struct ComponentType {
    raw: Owned<IComponentType>,
    session: Session,
    entry_point_count: usize,
}

impl ComponentType {
    /// Returns the number of entry points of the component type, the
    /// positions that [`LinkedProgram::entry_point_code`] accepts after a
    /// link.
    ///
    /// The crate counts the entry points of each composition and keeps the
    /// count, because Slang reports it only through the layout of a program.
    ///
    /// # Examples
    ///
    /// This example counts the entry points of a composition:
    ///
    /// ```
    /// use slang_compiler::ComponentType;
    ///
    /// fn has_entry_points(composed: &ComponentType) -> bool {
    ///     composed.entry_point_count() > 0
    /// }
    /// # let _ = has_entry_points;
    /// ```
    #[must_use]
    pub fn entry_point_count(&self) -> usize {
        self.entry_point_count
    }

    /// Returns the number of specialization parameters of the component
    /// type, the generic parameters and Slang interface parameters that
    /// [`ComponentType::specialize`] fills in.
    ///
    /// # Examples
    ///
    /// See the example of [`ComponentType::specialize`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Internal`] when Slang reports a
    /// negative count.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn specialization_param_count(&self) -> Result<usize> {
        guard::call("ComponentType::specialization_param_count", || {
            // SAFETY: `self.raw` points to a live component type of
            // `self.session` (Sections 11.1 and 11.2 of
            // `docs/design/slang-compiler.md`), and the call runs with the
            // Slang call guard raised (SLC-28).
            let count = unsafe {
                guard::abort_on_exception("IComponentType::getSpecializationParamCount", || {
                    IComponentType::getSpecializationParamCount(self.raw.as_ptr())
                })
            };
            usize::try_from(count).map_err(|error| {
                Error::new(ErrorKind::Internal, Operation::CountSpecializationParams)
                    .with_detail(format!("Slang reported the count {count}: {error}"))
            })
        })
    }

    /// Fills in the specialization parameters of the component type with
    /// `args`, in order, and returns the specialized component type, which is
    /// called specialization.
    ///
    /// A generic parameter is a type parameter of Slang code, like a type
    /// parameter of a Rust function, and a component type with one cannot
    /// generate code until it is specialized. This version finds types only
    /// through the raw bindings, as [`Type::from_raw`] describes.
    ///
    /// # Examples
    ///
    /// This example specializes a component type with one generic parameter
    /// and generates the code of its entry point. It takes the type as an
    /// argument, because this version finds types only through the raw
    /// bindings, as [`Type::from_raw`] describes:
    ///
    /// ```
    /// use slang_compiler::reflection::Type;
    /// use slang_compiler::{Blob, ComponentType, SpecializationArg, TargetIndex};
    ///
    /// fn compile_for(
    ///     composed: &ComponentType,
    ///     value_type: Type<'_>,
    ///     target: TargetIndex,
    /// ) -> slang_compiler::Result<Blob> {
    ///     assert_eq!(composed.specialization_param_count()?, 1);
    ///     let program = composed
    ///         .specialize(&[SpecializationArg::Type(value_type)])?
    ///         .into_value()
    ///         .link()?
    ///         .into_value();
    ///     Ok(program.entry_point_code(0, target)?.into_value())
    /// }
    /// # let _ = compile_for;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when a type
    /// belongs to another session, which the crate checks before it calls
    /// Slang, and the kind that Slang's result code maps to when Slang cannot
    /// specialize, such as [`ErrorKind::Compile`] with the diagnostics for a
    /// wrong number of arguments or a type that does not implement the
    /// interface that a parameter requires.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn specialize(&self, args: &[SpecializationArg<'_>]) -> Result<Compiled<Self>> {
        let operation = Operation::Specialize;
        let raw_args = args
            .iter()
            .enumerate()
            .map(|(position, arg)| match arg {
                SpecializationArg::Type(ty) => {
                    if ty.session().is_same(&self.session) {
                        Ok(RawSpecializationArg {
                            kind: SpecializationArg_Kind::Type,
                            anon0: SpecializationArg_anon0 {
                                r#type: ty.as_raw().as_ptr(),
                            },
                        })
                    } else {
                        Err(other_session(operation, &format!("argument {position}")))
                    }
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let count = SlangInt::try_from(raw_args.len()).map_err(|error| {
            Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
                "{} arguments are more than Slang takes: {error}",
                raw_args.len()
            ))
        })?;
        guard::call("ComponentType::specialize", || {
            let mut out: *mut IComponentType = ptr::null_mut();
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: `self.raw` points to a live component type of
            // `self.session` (Sections 11.1 and 11.2 of
            // `docs/design/slang-compiler.md`). `raw_args` holds `count`
            // arguments, each a non-null type of this session, which the
            // check above established and whose borrows keep it alive
            // (Section 11.3). `out` and `diagnostics` are live out-pointers,
            // and the call runs with the Slang call guard raised (SLC-28).
            let result = unsafe {
                guard::abort_on_exception("IComponentType::specialize", || {
                    IComponentType::specialize(
                        self.raw.as_ptr(),
                        raw_args.as_ptr(),
                        count,
                        &raw mut out,
                        &raw mut diagnostics,
                    )
                })
            };
            // SAFETY: `specialize` returns a new reference, which it can set
            // also when it fails, so the crate adopts it either way and the
            // drop releases it on failure, and every diagnostics output carries
            // a new reference or is null (Section 5.3 of
            // `docs/design/slang-compiler.md`).
            let Compiled {
                value: raw,
                diagnostics,
            } = unsafe { blob::take_output(operation, result, out, diagnostics) }?;
            Ok(Compiled {
                value: Self {
                    raw,
                    session: self.session.clone(),
                    entry_point_count: self.entry_point_count,
                },
                diagnostics,
            })
        })
    }

    /// Links the component type, resolving every module that it depends on,
    /// and returns the linked program, which generates code.
    ///
    /// Linking does not consume the component type. Slang can return the
    /// component type itself as the linked program when nothing more needs
    /// linking.
    ///
    /// # Examples
    ///
    /// See the example of [`Session::compose`].
    ///
    /// # Errors
    ///
    /// Returns the kind that Slang's result code maps to when Slang cannot
    /// link, such as [`ErrorKind::Compile`] with the diagnostics.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn link(&self) -> Result<Compiled<LinkedProgram>> {
        link(
            &self.raw,
            &self.session,
            self.entry_point_count,
            None,
            "ComponentType::link",
        )
    }

    /// Links the component type as [`ComponentType::link`] does, and adds
    /// `options` to the compiler options of the linked program, which apply
    /// when it generates code.
    ///
    /// Slang adds the options to the object that it returns, and that object
    /// is the component type itself whenever nothing more needs linking, as
    /// for a composition of a module with its entry points. The options then
    /// stay in the component type that the caller holds, every later link of
    /// it returns a program with them, and repeated calls add more. For a
    /// module, they stay in the module that the session returns to every
    /// later load of its name. To compile once with other options without
    /// changing a shared object, set the options on the session or on a
    /// [`Target`](crate::Target) instead.
    ///
    /// Slang fixes the options of a program for a target when it first
    /// generates code or metadata for that target, so options added after
    /// that have no effect on that target.
    ///
    /// The pinned Slang build applies these options of [`CompilerOptions`]
    /// this way:
    ///
    /// - [`CompilerOptions::force_dx_layout`],
    ///   [`CompilerOptions::force_c_layout`], and
    ///   [`CompilerOptions::glsl_force_scalar_layout`] change the generated
    ///   code as they do on the session, but not the layout that reflection
    ///   reports, which keeps the layout without the option. The code then
    ///   places data where the reflected layout does not, so set these
    ///   options on the session or on a target when a program fills buffers
    ///   from the reflected layout.
    /// - [`CompilerOptions::preserve_parameters`] changes the generated code
    ///   as it does on the session.
    /// - [`CompilerOptions::vulkan_use_entry_point_name`] and
    ///   [`CompilerOptions::debug_info`] have no effect, and neither has
    ///   [`CompilerOptions::language_version`], because Slang reads them
    ///   before linking.
    ///
    /// # Examples
    ///
    /// See the example of [`Module::link_with_options`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when a text
    /// value of `options` contains a NUL byte or there are more options than
    /// Slang takes, and the errors of [`ComponentType::link`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn link_with_options(&self, options: &CompilerOptions) -> Result<Compiled<LinkedProgram>> {
        link(
            &self.raw,
            &self.session,
            self.entry_point_count,
            Some(options),
            "ComponentType::link_with_options",
        )
    }

    /// Adopts a component type that the caller obtained through the raw
    /// bindings of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example moves a component type out of the crate and back:
    ///
    /// ```
    /// use slang_compiler::ComponentType;
    ///
    /// fn round_trip(composed: ComponentType) -> ComponentType {
    ///     let count = composed.entry_point_count();
    ///     let (raw, session) = composed.into_raw();
    ///     // SAFETY: `raw` carries the one reference that `into_raw` gave up,
    ///     // of a component type of `session` with `count` entry points, and
    ///     // only this thread uses it.
    ///     unsafe { ComponentType::from_raw(raw, session, count) }
    /// }
    /// # let _ = round_trip;
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a live component type that the Slang library of the
    ///   build tag [`slang_compiler_sys::SLANG_BUILD_TAG`] created.
    /// - The caller owns one reference to it and transfers that reference.
    /// - `session` is the session that the component type was created in.
    /// - `entry_point_count` is the number of entry points of the component
    ///   type, because the crate checks every entry point position against it
    ///   before it passes the position to Slang.
    /// - The component type is not used on another thread while the returned
    ///   value or a clone of it exists.
    ///
    /// Section 11.7 of the design of this crate states these conditions.
    #[must_use]
    pub unsafe fn from_raw(
        raw: NonNull<IComponentType>,
        session: Session,
        entry_point_count: usize,
    ) -> Self {
        // SAFETY: the caller guarantees that `raw` points to a live component
        // type of the pinned build and transfers one reference to it (Section
        // 11.7 of `docs/design/slang-compiler.md`).
        let raw = unsafe { Owned::adopt(raw) };
        Self {
            raw,
            session,
            entry_point_count,
        }
    }

    /// Gives up this handle's reference to the component type without
    /// releasing it, and returns the pointer for use with the raw bindings of
    /// `slang-compiler-sys`, together with this handle's handle to its
    /// session.
    ///
    /// The caller then owns the reference and releases it. Keep the returned
    /// [`Session`] until after that release, or pass both back to
    /// [`ComponentType::from_raw`], because a component type of Slang reads
    /// its session in every call and does not keep it alive.
    ///
    /// # Examples
    ///
    /// See the example of [`ComponentType::from_raw`].
    #[must_use]
    pub fn into_raw(self) -> (NonNull<IComponentType>, Session) {
        let Self { raw, session, .. } = self;
        (raw.into_raw(), session)
    }

    /// Returns the pointer to the component type without changing its
    /// reference count, for use with the raw bindings of
    /// `slang-compiler-sys`.
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
    /// use slang_compiler::ComponentType;
    ///
    /// fn raw_pointer(composed: &ComponentType) -> *mut slang_compiler_sys::IComponentType {
    ///     composed.as_raw().as_ptr()
    /// }
    /// # let _ = raw_pointer;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<IComponentType> {
        self.raw.as_non_null()
    }
}

impl fmt::Debug for ComponentType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ComponentType")
            .field("raw", &self.raw.as_non_null())
            .field("entry_point_count", &self.entry_point_count)
            .finish_non_exhaustive()
    }
}

/// The record that a type implements a Slang interface, an `interface`
/// declaration that Slang types implement, which a composition can include.
///
/// Get it from [`Session::type_conformance`]. It holds its [`Session`]. It
/// is neither [`Send`] nor [`Sync`]. [`Clone`] returns another handle to the
/// same type conformance.
///
/// # Examples
///
/// See the example of [`Session::type_conformance`].
///
/// A type conformance cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::TypeConformance>();
/// ```
///
/// A type conformance cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::TypeConformance>();
/// ```
#[derive(Clone)]
pub struct TypeConformance {
    raw: Owned<ITypeConformance>,
    session: Session,
}

impl TypeConformance {
    /// Links the type conformance on its own, as [`ComponentType::link`]
    /// does for a component type.
    ///
    /// # Examples
    ///
    /// See the example of [`Session::compose`], which links a composition.
    ///
    /// # Errors
    ///
    /// Returns the errors of [`ComponentType::link`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn link(&self) -> Result<Compiled<LinkedProgram>> {
        link(&self.raw, &self.session, 0, None, "TypeConformance::link")
    }

    /// Links the type conformance on its own with compiler options, as
    /// [`ComponentType::link_with_options`] does for a component type.
    ///
    /// The options take effect as the documentation of
    /// [`ComponentType::link_with_options`] describes: which options change the generated code, which change
    /// nothing, and why options added after code was generated for a target
    /// have no effect on it.
    ///
    /// # Examples
    ///
    /// See the example of [`Module::link_with_options`].
    ///
    /// # Errors
    ///
    /// Returns the errors of [`ComponentType::link_with_options`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn link_with_options(&self, options: &CompilerOptions) -> Result<Compiled<LinkedProgram>> {
        link(
            &self.raw,
            &self.session,
            0,
            Some(options),
            "TypeConformance::link_with_options",
        )
    }

    /// Adopts a type conformance that the caller obtained through the raw
    /// bindings of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example moves a type conformance out of the crate and back:
    ///
    /// ```
    /// use slang_compiler::TypeConformance;
    ///
    /// fn round_trip(conformance: TypeConformance) -> TypeConformance {
    ///     let (raw, session) = conformance.into_raw();
    ///     // SAFETY: `raw` carries the one reference that `into_raw` gave up,
    ///     // of a type conformance of `session`, and only this thread uses it.
    ///     unsafe { TypeConformance::from_raw(raw, session) }
    /// }
    /// # let _ = round_trip;
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a live type conformance that the Slang library of
    ///   the build tag [`slang_compiler_sys::SLANG_BUILD_TAG`] created.
    /// - The caller owns one reference to it and transfers that reference.
    /// - `session` is the session that the type conformance was created in.
    /// - The type conformance is not used on another thread while the
    ///   returned value or a clone of it exists.
    ///
    /// Section 11.7 of the design of this crate states these conditions.
    #[must_use]
    pub unsafe fn from_raw(raw: NonNull<ITypeConformance>, session: Session) -> Self {
        // SAFETY: the caller guarantees that `raw` points to a live type
        // conformance of the pinned build and transfers one reference to it
        // (Section 11.7 of `docs/design/slang-compiler.md`).
        let raw = unsafe { Owned::adopt(raw) };
        Self { raw, session }
    }

    /// Gives up this handle's reference to the type conformance without
    /// releasing it, and returns the pointer for use with the raw bindings of
    /// `slang-compiler-sys`, together with this handle's handle to its
    /// session.
    ///
    /// The caller then owns the reference and releases it. Keep the returned
    /// [`Session`] until after that release, or pass both back to
    /// [`TypeConformance::from_raw`].
    ///
    /// # Examples
    ///
    /// See the example of [`TypeConformance::from_raw`].
    #[must_use]
    pub fn into_raw(self) -> (NonNull<ITypeConformance>, Session) {
        let Self { raw, session } = self;
        (raw.into_raw(), session)
    }

    /// Returns the pointer to the type conformance without changing its
    /// reference count, for use with the raw bindings of
    /// `slang-compiler-sys`.
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
    /// use slang_compiler::TypeConformance;
    ///
    /// fn raw_pointer(conformance: &TypeConformance) -> *mut slang_compiler_sys::ITypeConformance {
    ///     conformance.as_raw().as_ptr()
    /// }
    /// # let _ = raw_pointer;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<ITypeConformance> {
        self.raw.as_non_null()
    }
}

impl fmt::Debug for TypeConformance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypeConformance")
            .field("raw", &self.raw.as_non_null())
            .finish_non_exhaustive()
    }
}

/// A linked program, a component type that linking returned, which
/// generates code and metadata.
///
/// Slang generates code only for a component type that is fully specialized
/// and linked, so only this type offers code generation. Get it from the
/// method `link` or `link_with_options` of a [`Module`], [`EntryPoint`],
/// [`ComponentType`], or [`TypeConformance`]. Its entry points are those of
/// the component type that was linked, in the same order.
///
/// A linked program holds its [`Session`]. It is neither [`Send`] nor
/// [`Sync`]. [`Clone`] returns another handle to the same program.
///
/// # Examples
///
/// See the example of [`Session::compose`].
///
/// A linked program cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::LinkedProgram>();
/// ```
///
/// A linked program cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::LinkedProgram>();
/// ```
#[derive(Clone)]
pub struct LinkedProgram {
    raw: Owned<IComponentType>,
    session: Session,
    entry_point_count: usize,
}

impl LinkedProgram {
    /// Returns the number of entry points of the program, the positions that
    /// [`LinkedProgram::entry_point_code`] accepts.
    ///
    /// # Examples
    ///
    /// This example generates code for every entry point of a program:
    ///
    /// ```
    /// use slang_compiler::{Blob, LinkedProgram, TargetIndex};
    ///
    /// fn every_entry_point(program: &LinkedProgram, target: TargetIndex) -> slang_compiler::Result<Vec<Blob>> {
    ///     (0..program.entry_point_count())
    ///         .map(|position| Ok(program.entry_point_code(position, target)?.into_value()))
    ///         .collect()
    /// }
    /// # let _ = every_entry_point;
    /// ```
    #[must_use]
    pub fn entry_point_count(&self) -> usize {
        self.entry_point_count
    }

    /// Generates the code of one entry point of the program for one target.
    ///
    /// An entry point is a function of a shader that a graphics API calls,
    /// like `main` in a program. `entry_point` is its position in the program
    /// and `target` selects one target of the session, such as SPIR-V, the
    /// binary shader format of Vulkan.
    ///
    /// Slang can return messages with code that it generated, such as
    /// warnings, in the diagnostics of the returned record. When Slang lacks
    /// an optional library, the diagnostics of a successful generation can
    /// begin with `error`, as the crate documentation describes.
    ///
    /// # Examples
    ///
    /// See the example of [`Session::compose`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when
    /// `entry_point` is not below [`LinkedProgram::entry_point_count`] or
    /// `target` is past the end of the session's targets, and the kind that
    /// Slang's result code maps to when Slang cannot generate the code, such
    /// as [`ErrorKind::Compile`] with the diagnostics.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn entry_point_code(
        &self,
        entry_point: usize,
        target: TargetIndex,
    ) -> Result<Compiled<Blob>> {
        let operation = Operation::GenerateCode;
        let entry_point = self.slang_entry_point(entry_point, operation)?;
        let target = self.session.slang_target(target, operation)?;
        guard::call("LinkedProgram::entry_point_code", || {
            let mut code: *mut ISlangBlob = ptr::null_mut();
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: `self.raw` points to a live linked program of
            // `self.session` (Sections 11.1 and 11.2 of
            // `docs/design/slang-compiler.md`), `entry_point` and `target` are
            // below the counts of the program and the session, `code` and
            // `diagnostics` are live out-pointers, and the call runs with the
            // Slang call guard raised (SLC-28).
            let result = unsafe {
                guard::abort_on_exception("IComponentType::getEntryPointCode", || {
                    IComponentType::getEntryPointCode(
                        self.raw.as_ptr(),
                        entry_point,
                        target,
                        &raw mut code,
                        &raw mut diagnostics,
                    )
                })
            };
            self.code(operation, result, code, diagnostics)
        })
    }

    /// Generates the code of the whole program for one target, with every
    /// entry point of the program in one output, for targets that support
    /// that, such as SPIR-V, the binary shader format of Vulkan.
    ///
    /// # Examples
    ///
    /// This example generates SPIR-V for a whole program:
    ///
    /// ```
    /// use slang_compiler::{LinkedProgram, TargetIndex};
    ///
    /// fn whole_program(program: &LinkedProgram, spirv: TargetIndex) -> slang_compiler::Result<Vec<u8>> {
    ///     Ok(program.target_code(spirv)?.into_value().to_vec())
    /// }
    /// # let _ = whole_program;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `target`
    /// is past the end of the session's targets, and the kind that Slang's
    /// result code maps to when Slang cannot generate the code, such as
    /// [`ErrorKind::Compile`] with the diagnostics.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn target_code(&self, target: TargetIndex) -> Result<Compiled<Blob>> {
        let operation = Operation::GenerateCode;
        let target = self.session.slang_target(target, operation)?;
        guard::call("LinkedProgram::target_code", || {
            let mut code: *mut ISlangBlob = ptr::null_mut();
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: as in `entry_point_code`, without an entry point.
            let result = unsafe {
                guard::abort_on_exception("IComponentType::getTargetCode", || {
                    IComponentType::getTargetCode(
                        self.raw.as_ptr(),
                        target,
                        &raw mut code,
                        &raw mut diagnostics,
                    )
                })
            };
            self.code(operation, result, code, diagnostics)
        })
    }

    /// Returns the metadata of the code of one entry point for one target,
    /// such as which parameters the code uses, generating the code first
    /// when it does not exist yet.
    ///
    /// # Examples
    ///
    /// See the example of [`Metadata`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when
    /// `entry_point` or `target` is out of range, as for
    /// [`LinkedProgram::entry_point_code`], an error of kind
    /// [`ErrorKind::Compile`] with the diagnostics when the code cannot be
    /// generated, and the kind that Slang's result code maps to for another
    /// failure, such as [`ErrorKind::Unsupported`] when the code has no
    /// metadata.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn entry_point_metadata(
        &self,
        entry_point: usize,
        target: TargetIndex,
    ) -> Result<Compiled<Metadata>> {
        let operation = Operation::ReadMetadata;
        let entry_point = self.slang_entry_point(entry_point, operation)?;
        let target = self.session.slang_target(target, operation)?;
        guard::call("LinkedProgram::entry_point_metadata", || {
            let mut metadata: *mut IMetadata = ptr::null_mut();
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: as in `entry_point_code`, with `metadata` as the live
            // out-pointer of the result.
            let result = unsafe {
                guard::abort_on_exception("IComponentType::getEntryPointMetadata", || {
                    IComponentType::getEntryPointMetadata(
                        self.raw.as_ptr(),
                        entry_point,
                        target,
                        &raw mut metadata,
                        &raw mut diagnostics,
                    )
                })
            };
            self.metadata(operation, result, metadata, diagnostics)
        })
    }

    /// Returns the metadata of the code of the whole program for one target,
    /// generating the code first when it does not exist yet.
    ///
    /// # Examples
    ///
    /// This example checks whether the whole program uses the binding 0 of
    /// the descriptor set 0 of Vulkan:
    ///
    /// ```
    /// use slang_compiler::{BindingIndex, BindingSpace, LinkedProgram, ParameterCategory, TargetIndex};
    ///
    /// fn uses_first_binding(program: &LinkedProgram, spirv: TargetIndex) -> slang_compiler::Result<bool> {
    ///     program.target_metadata(spirv)?.into_value().is_parameter_location_used(
    ///         ParameterCategory::DescriptorTableSlot,
    ///         BindingSpace::new(0),
    ///         BindingIndex::new(0),
    ///     )
    /// }
    /// # let _ = uses_first_binding;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `target`
    /// is past the end of the session's targets, and the kind that Slang's
    /// result code maps to when Slang fails, such as [`ErrorKind::Compile`]
    /// with the diagnostics when the code cannot be generated.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn target_metadata(&self, target: TargetIndex) -> Result<Compiled<Metadata>> {
        let operation = Operation::ReadMetadata;
        let target = self.session.slang_target(target, operation)?;
        guard::call("LinkedProgram::target_metadata", || {
            let mut metadata: *mut IMetadata = ptr::null_mut();
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: as in `target_code`, with `metadata` as the live
            // out-pointer of the result.
            let result = unsafe {
                guard::abort_on_exception("IComponentType::getTargetMetadata", || {
                    IComponentType::getTargetMetadata(
                        self.raw.as_ptr(),
                        target,
                        &raw mut metadata,
                        &raw mut diagnostics,
                    )
                })
            };
            self.metadata(operation, result, metadata, diagnostics)
        })
    }

    /// Adopts a linked program that the caller obtained through the raw
    /// bindings of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example moves a linked program out of the crate and back:
    ///
    /// ```
    /// use slang_compiler::LinkedProgram;
    ///
    /// fn round_trip(program: LinkedProgram) -> LinkedProgram {
    ///     let count = program.entry_point_count();
    ///     let (raw, session) = program.into_raw();
    ///     // SAFETY: `raw` carries the one reference that `into_raw` gave up,
    ///     // of a linked program of `session` with `count` entry points, and
    ///     // only this thread uses it.
    ///     unsafe { LinkedProgram::from_raw(raw, session, count) }
    /// }
    /// # let _ = round_trip;
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a live component type that the Slang library of the
    ///   build tag [`slang_compiler_sys::SLANG_BUILD_TAG`] created, and that
    ///   is fully specialized and linked.
    /// - The caller owns one reference to it and transfers that reference.
    /// - `session` is the session that the component type was created in.
    /// - `entry_point_count` is the number of entry points of the component
    ///   type, because the crate checks every entry point position against it
    ///   before it passes the position to Slang.
    /// - The program is not used on another thread while the returned value
    ///   or a clone of it exists.
    ///
    /// Section 11.7 of the design of this crate states these conditions.
    #[must_use]
    pub unsafe fn from_raw(
        raw: NonNull<IComponentType>,
        session: Session,
        entry_point_count: usize,
    ) -> Self {
        // SAFETY: the caller guarantees that `raw` points to a live linked
        // component type of the pinned build and transfers one reference to
        // it (Section 11.7 of `docs/design/slang-compiler.md`).
        let raw = unsafe { Owned::adopt(raw) };
        Self {
            raw,
            session,
            entry_point_count,
        }
    }

    /// Gives up this handle's reference to the program without releasing it,
    /// and returns the pointer for use with the raw bindings of
    /// `slang-compiler-sys`, together with this handle's handle to its
    /// session.
    ///
    /// The caller then owns the reference and releases it. Keep the returned
    /// [`Session`] until after that release, or pass both back to
    /// [`LinkedProgram::from_raw`], because a program of Slang reads its
    /// session in every call and does not keep it alive.
    ///
    /// # Examples
    ///
    /// See the example of [`LinkedProgram::from_raw`].
    #[must_use]
    pub fn into_raw(self) -> (NonNull<IComponentType>, Session) {
        let Self { raw, session, .. } = self;
        (raw.into_raw(), session)
    }

    /// Returns the pointer to the program without changing its reference
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
    /// use slang_compiler::LinkedProgram;
    ///
    /// fn raw_pointer(program: &LinkedProgram) -> *mut slang_compiler_sys::IComponentType {
    ///     program.as_raw().as_ptr()
    /// }
    /// # let _ = raw_pointer;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<IComponentType> {
        self.raw.as_non_null()
    }

    /// Checks an entry point position against the count of the program
    /// before it reaches Slang, which indexes without a check (Section 6.7 of
    /// `docs/design/slang-compiler.md`).
    fn slang_entry_point(&self, position: usize, operation: Operation) -> Result<SlangInt> {
        if position >= self.entry_point_count {
            return Err(Error::new(ErrorKind::InvalidArgument, operation)
                .with_subject(format!("`{position}`"))
                .with_detail(format!(
                    "the program has {} entry points, numbered from 0",
                    self.entry_point_count
                )));
        }
        SlangInt::try_from(position).map_err(|error| {
            Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
                "entry point {position} does not fit Slang's index: {error}"
            ))
        })
    }

    /// Finishes a call that returns code. The caller raises the guard.
    fn code(
        &self,
        operation: Operation,
        result: SlangResult,
        code: *mut ISlangBlob,
        diagnostics: *mut ISlangBlob,
    ) -> Result<Compiled<Blob>> {
        // SAFETY: `getEntryPointCode` and `getTargetCode` return a new blob
        // reference or leave the output null, and every diagnostics output
        // carries a new reference or is null (Section 5.3 of
        // `docs/design/slang-compiler.md`).
        let Compiled {
            value: code,
            diagnostics,
        } = unsafe { blob::take_output(operation, result, code, diagnostics) }?;
        match Blob::new(code, self.session.clone(), operation) {
            Ok(value) => Ok(Compiled { value, diagnostics }),
            Err(error) => Err(error.with_diagnostics(diagnostics)),
        }
    }

    /// Finishes a call that returns metadata. The caller raises the guard.
    fn metadata(
        &self,
        operation: Operation,
        result: SlangResult,
        metadata: *mut IMetadata,
        diagnostics: *mut ISlangBlob,
    ) -> Result<Compiled<Metadata>> {
        // SAFETY: `getEntryPointMetadata` and `getTargetMetadata` return a new
        // reference or leave the output null, and every diagnostics output
        // carries a new reference or is null (Section 5.3 of
        // `docs/design/slang-compiler.md`).
        let Compiled {
            value: metadata,
            diagnostics,
        } = unsafe { blob::take_output(operation, result, metadata, diagnostics) }?;
        Ok(Compiled {
            value: Metadata::new(metadata, self.session.clone()),
            diagnostics,
        })
    }
}

impl fmt::Debug for LinkedProgram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LinkedProgram")
            .field("raw", &self.raw.as_non_null())
            .field("entry_point_count", &self.entry_point_count)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::{CompilerOptionName, SlangResult};

    use super::{Component, ComponentType, LinkedProgram};
    use crate::fake::{
        self, Code, Compose, Event, FAKE_CODE, FAKE_DIAGNOSTICS, FAKE_ENTRY_POINT, FAKE_MODULE,
        FAKE_SESSION, FakeSession, Link, Log, OptionSnapshot, Script, Specialize,
    };
    use crate::{
        BindingIndex, BindingSpace, CompilerOptions, ErrorKind, GlobalSession, Module, Operation,
        OptimizationLevel, ParameterCategory, Session,
    };

    struct Fixture {
        log: Log,
        global: GlobalSession,
        session: Session,
        module: Module,
    }

    impl Fixture {
        fn new(target_count: usize) -> Self {
            let log = Log::new();
            let global = fake::global_session(&log);
            let session = fake::session(&global, target_count);
            let module = session
                .load_module_from_source("kernel", "kernel.slang", "void main() {}")
                .expect("the fake loads the module")
                .into_value();
            Self {
                log,
                global,
                session,
                module,
            }
        }

        fn script(&self) -> &Script {
            // SAFETY: `self.session` keeps the fake session alive while the
            // returned reference borrows `self`.
            unsafe { FakeSession::script(self.session.as_raw()) }
        }

        fn composed(&self) -> ComponentType {
            let entry_point = self
                .module
                .entry_point(FAKE_ENTRY_POINT)
                .expect("the fake finds its entry point");
            self.session
                .compose(&[
                    Component::Module(&self.module),
                    Component::EntryPoint(&entry_point),
                ])
                .expect("the fake composes")
                .into_value()
        }

        fn program(&self) -> LinkedProgram {
            self.composed().link().expect("the fake links").into_value()
        }
    }

    #[test]
    fn a_negative_specialization_parameter_count_is_internal() {
        let fixture = Fixture::new(1);
        let composed = fixture.composed();
        fixture.script().count.set(Some(-1));
        let error = composed
            .specialization_param_count()
            .expect_err("a negative count fails");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.operation(), Operation::CountSpecializationParams);
        assert!(
            error.to_string().contains("Slang reported the count -1"),
            "{error}"
        );
    }

    #[test]
    fn compose_holds_its_parts_and_counts_their_entry_points() {
        let fixture = Fixture::new(1);
        let composed = fixture.composed();
        assert_eq!(composed.entry_point_count(), 1);
        assert_eq!(composed.specialization_param_count().ok(), Some(1));
        assert!(
            fixture
                .log
                .events()
                .contains(&Event::query(FAKE_MODULE, true))
        );
        assert!(
            fixture
                .log
                .events()
                .contains(&Event::query("entry_point", true))
        );
        drop(composed);
        assert!(fixture.log.events().contains(&Event::freed("composite")));
    }

    #[test]
    fn compose_of_one_part_returns_that_part() {
        let fixture = Fixture::new(1);
        let composed = fixture
            .session
            .compose(&[Component::Module(&fixture.module)])
            .expect("the fake composes")
            .into_value();
        assert_eq!(
            composed.as_raw().cast::<()>(),
            fixture.module.as_raw().cast::<()>()
        );
        assert_eq!(composed.entry_point_count(), 0);
    }

    #[test]
    fn compose_rejects_a_part_of_another_session_before_any_call() {
        let fixture = Fixture::new(1);
        let other = fake::session(&fixture.global, 1);
        let before = fake::calls(&fixture.log);
        let error = other
            .compose(&[Component::Module(&fixture.module)])
            .expect_err("the module belongs to another session");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::Compose);
        assert_eq!(
            error.to_string(),
            "cannot compose component types: component 0 belongs to another session, and Slang resolves it only in its own session"
        );
        assert_eq!(fake::calls(&fixture.log), before);
    }

    #[test]
    fn compose_failure_is_a_compile_error_with_the_diagnostics() {
        let fixture = Fixture::new(1);
        fixture.script().compose.set(Compose::Fail);
        let error = fixture
            .session
            .compose(&[Component::Module(&fixture.module)])
            .expect_err("the fake fails");
        assert_eq!(error.kind(), ErrorKind::Compile);
        assert_eq!(error.diagnostics().as_str(), FAKE_DIAGNOSTICS);
    }

    #[test]
    fn link_that_returns_its_input_is_adopted_and_released_once() {
        let fixture = Fixture::new(1);
        let composed = fixture.composed();
        let program = composed.link().expect("the fake links").into_value();
        assert_eq!(program.as_raw(), composed.as_raw());
        assert_eq!(program.entry_point_count(), 1);
        drop(composed);
        assert!(!fixture.log.events().contains(&Event::freed("composite")));
        drop(program);
        assert!(fixture.log.events().contains(&Event::freed("composite")));
    }

    #[test]
    fn link_that_returns_a_new_object_releases_it_before_its_input() {
        let fixture = Fixture::new(1);
        fixture.script().link.set(Link::NewObject);
        let composed = fixture.composed();
        let program = composed.link().expect("the fake links").into_value();
        assert_ne!(program.as_raw(), composed.as_raw());
        drop(composed);
        drop(program);
        assert_eq!(
            fake::releases(&fixture.log, &["linked", "composite"])
                .into_iter()
                .filter(|event| matches!(event, Event::Freed { .. }))
                .collect::<Vec<_>>(),
            [Event::freed("linked"), Event::freed("composite")]
        );
    }

    #[test]
    fn link_failure_returns_the_diagnostics() {
        let fixture = Fixture::new(1);
        fixture.script().link.set(Link::Fail);
        let error = fixture.module.link().expect_err("the fake fails");
        assert_eq!(error.kind(), ErrorKind::Compile);
        assert_eq!(error.operation(), Operation::Link);
        assert_eq!(error.code(), Some(SlangResult::SLANG_FAIL.0));
        assert_eq!(error.diagnostics().as_str(), FAKE_DIAGNOSTICS);
        assert!(fixture.log.events().contains(&Event::freed("diagnostics")));
    }

    #[test]
    fn link_with_options_passes_the_entries_and_rejects_a_nul_byte_first() {
        let fixture = Fixture::new(1);
        let mut options = CompilerOptions::default();
        options.optimization(OptimizationLevel::Maximal);
        let program = fixture
            .module
            .link_with_options(&options)
            .expect("the fake links")
            .into_value();
        assert_eq!(program.entry_point_count(), 0);
        assert!(fixture.log.events().contains(&Event::LinkWithOptions {
            object: FAKE_MODULE,
            options: vec![OptionSnapshot::Int(
                CompilerOptionName::Optimization.0,
                OptimizationLevel::Maximal.to_raw().0.cast_signed(),
                0
            )],
        }));
        let before = fake::calls(&fixture.log);
        let mut options = CompilerOptions::default();
        options.disable_warning("a\0b");
        let error = fixture
            .module
            .link_with_options(&options)
            .expect_err("a NUL byte is rejected");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::Link);
        assert_eq!(fake::calls(&fixture.log), before);
    }

    #[test]
    fn specialize_returns_a_new_component_type_with_the_entry_point_count() {
        let fixture = Fixture::new(1);
        let composed = fixture.composed();
        let specialized = composed
            .specialize(&[])
            .expect("the fake specializes")
            .into_value();
        assert_eq!(specialized.entry_point_count(), 1);
        assert_ne!(specialized.as_raw(), composed.as_raw());
    }

    #[test]
    fn specialize_that_fails_with_an_output_releases_the_output() {
        for mode in [Specialize::FailWithOutput, Specialize::FailWithoutOutput] {
            let fixture = Fixture::new(1);
            fixture.script().specialize.set(mode);
            let error = fixture
                .composed()
                .specialize(&[])
                .expect_err("the fake fails");
            assert_eq!(error.kind(), ErrorKind::Compile);
            assert_eq!(error.operation(), Operation::Specialize);
            assert_eq!(error.diagnostics().as_str(), FAKE_DIAGNOSTICS);
            assert_eq!(
                fixture.log.events().contains(&Event::freed("specialized")),
                mode == Specialize::FailWithOutput
            );
        }
    }

    #[test]
    fn entry_point_code_returns_the_blob_and_the_diagnostics_of_a_success() {
        let fixture = Fixture::new(1);
        fixture.script().code.set(Code::WithDiagnostics);
        let program = fixture.program();
        let target = fixture.session.target_index(0).expect("one target");
        let compiled = program
            .entry_point_code(0, target)
            .expect("the fake generates code");
        assert_eq!(compiled.diagnostics.as_str(), FAKE_DIAGNOSTICS);
        assert_eq!(compiled.value.as_bytes(), FAKE_CODE);
        assert_eq!(compiled.value.to_vec(), FAKE_CODE);
        let code = program
            .target_code(target)
            .expect("the fake generates code");
        assert_eq!(code.value.as_bytes(), FAKE_CODE);
    }

    #[test]
    fn entry_point_code_checks_the_entry_point_and_the_target_before_any_call() {
        let fixture = Fixture::new(1);
        let program = fixture.program();
        let wider = fake::session(&fixture.global, 2);
        let past = wider.target_index(1).expect("two targets");
        let first = fixture.session.target_index(0).expect("one target");
        let before = fake::calls(&fixture.log);
        let error = program
            .entry_point_code(1, first)
            .expect_err("the program has one entry point");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            "cannot generate code `1`: the program has 1 entry points, numbered from 0"
        );
        for error in [
            program.entry_point_code(0, past).map(drop),
            program.target_code(past).map(drop),
            program.entry_point_metadata(0, past).map(drop),
            program.target_metadata(past).map(drop),
        ]
        .map(|result| result.expect_err("the target is past the end"))
        {
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert!(
                error.to_string().contains("target 1 is past the end"),
                "{error}"
            );
        }
        assert_eq!(fake::calls(&fixture.log), before);
    }

    #[test]
    fn code_that_fails_with_an_output_releases_the_output() {
        let fixture = Fixture::new(1);
        fixture.script().code.set(Code::FailWithOutput);
        let program = fixture.program();
        let target = fixture.session.target_index(0).expect("one target");
        let error = program
            .entry_point_code(0, target)
            .expect_err("the fake fails");
        assert_eq!(error.kind(), ErrorKind::Compile);
        assert!(fixture.log.events().contains(&Event::freed("code")));
    }

    #[test]
    fn metadata_after_a_failed_generation_is_a_compile_error() {
        let fixture = Fixture::new(1);
        fixture.script().code.set(Code::NotAvailable);
        let program = fixture.program();
        let target = fixture.session.target_index(0).expect("one target");
        let error = program
            .entry_point_metadata(0, target)
            .expect_err("the fake fails");
        assert_eq!(error.kind(), ErrorKind::Compile);
        assert_eq!(error.operation(), Operation::ReadMetadata);
        assert_eq!(error.code(), Some(SlangResult::SLANG_E_NOT_AVAILABLE.0));
    }

    #[test]
    fn metadata_reports_the_used_locations() {
        let fixture = Fixture::new(1);
        let program = fixture.program();
        let target = fixture.session.target_index(0).expect("one target");
        for metadata in [
            program.entry_point_metadata(0, target),
            program.target_metadata(target),
        ] {
            let metadata = metadata.expect("the fake returns metadata").into_value();
            let used = |space, index| {
                metadata
                    .is_parameter_location_used(
                        ParameterCategory::DescriptorTableSlot,
                        BindingSpace::new(space),
                        BindingIndex::new(index),
                    )
                    .expect("the fake answers")
            };
            assert!(used(0, 0));
            assert!(!used(0, 1));
        }
    }

    #[test]
    fn a_linked_program_works_after_every_other_handle_is_dropped() {
        let fixture = Fixture::new(1);
        let program = fixture.program();
        let target = fixture.session.target_index(0).expect("one target");
        let Fixture {
            log,
            global,
            session,
            module,
        } = fixture;
        drop(module);
        drop(session);
        drop(global);
        let code = program
            .entry_point_code(0, target)
            .expect("the program keeps its session alive")
            .into_value();
        drop(program);
        assert!(!log.events().contains(&Event::freed(FAKE_SESSION)));
        drop(code);
        let freed = log
            .events()
            .into_iter()
            .filter(|event| matches!(event, Event::Freed { .. }))
            .collect::<Vec<_>>();
        let position = |name| {
            freed
                .iter()
                .position(|event| *event == Event::freed(name))
                .expect("every object is freed")
        };
        assert!(position("composite") < position(FAKE_SESSION));
        assert!(position("code") < position(FAKE_SESSION));
        assert!(position(FAKE_SESSION) < position("global"));
    }

    #[test]
    fn into_raw_and_from_raw_hand_component_types_over() {
        let fixture = Fixture::new(1);
        let composed = fixture.composed();
        let count = composed.entry_point_count();
        let (raw, session) = composed.into_raw();
        // SAFETY: `raw` carries the reference that `into_raw` gave up, of a
        // component type of `session` with `count` entry points.
        let composed = unsafe { ComponentType::from_raw(raw, session, count) };
        let program = composed.link().expect("the fake links").into_value();
        let (raw, session) = program.into_raw();
        // SAFETY: as above, for a linked program.
        let program = unsafe { LinkedProgram::from_raw(raw, session, count) };
        assert_eq!(program.entry_point_count(), 1);
    }
}
