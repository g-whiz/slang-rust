use std::fmt;
use std::ptr::{self, NonNull};

use slang_compiler_sys::{
    ISlangBlob, SlangReflectionFunction, SlangReflectionType, TypeReflection,
    spReflectionFunction_GetName, spReflectionFunction_GetParameter,
    spReflectionFunction_GetParameterCount, spReflectionFunction_GetResultType,
    spReflectionType_GetColumnCount, spReflectionType_GetElementCount,
    spReflectionType_GetElementType, spReflectionType_GetFieldByIndex,
    spReflectionType_GetFieldCount, spReflectionType_GetFullName, spReflectionType_GetKind,
    spReflectionType_GetName, spReflectionType_GetResourceAccess,
    spReflectionType_GetResourceResultType, spReflectionType_GetResourceShape,
    spReflectionType_GetRowCount, spReflectionType_GetScalarType,
};

use super::{
    LayoutSize, Positions, ResourceAccess, ResourceShape, ScalarType, TypeKind, Variable, count,
    index,
};
use crate::blob;
use crate::error::{Diagnostics, Operation, Result};
use crate::guard;
use crate::mapping;
use crate::owned::Owned;
use crate::session::Session;
use crate::strings::borrowed_str;

/// A type of Slang code, such as a structure, a vector, or a Slang
/// interface, an `interface` declaration that Slang types implement.
///
/// A type says what a value is, and a [`TypeLayout`](super::TypeLayout)
/// says where it sits for one use, because Slang can lay out one type
/// differently depending on its use. Get a type from a layout, such as
/// [`TypeLayout::ty`](super::TypeLayout::ty) or
/// [`ProgramLayout::find_type_by_name`](super::ProgramLayout::find_type_by_name).
/// [`ComponentType::specialize`](crate::ComponentType::specialize) and
/// [`Session::type_conformance`](crate::Session::type_conformance) take one.
/// A type borrows the object it came from, so it cannot outlive it, and it
/// belongs to the session of that object: a method of another session
/// rejects it.
///
/// # Examples
///
/// This example describes a type by its kind, its scalar type, and, for a
/// resource such as a texture or a buffer, its shape and access:
///
/// ```
/// use slang_compiler::reflection::{Type, TypeKind};
///
/// fn describe(ty: Type<'_>) -> slang_compiler::Result<String> {
///     let name = ty.full_name()?;
///     Ok(match ty.kind()? {
///         TypeKind::Scalar | TypeKind::Vector | TypeKind::Matrix => format!(
///             "{name}: {:?}, {} by {}",
///             ty.scalar_type()?,
///             ty.row_count(),
///             ty.column_count()
///         ),
///         TypeKind::Resource => {
///             let shape = ty.resource_shape();
///             format!(
///                 "{name}: {:?}{}, {:?}",
///                 shape.base()?,
///                 if shape.is_array() { " array" } else { "" },
///                 ty.resource_access()?
///             )
///         }
///         kind => format!("{name}: {kind:?}"),
///     })
/// }
/// # let _ = describe;
/// ```
///
/// A type cannot outlive the object that it came from, so this example does
/// not compile:
///
/// ```compile_fail,E0505
/// use slang_compiler::{Module, TargetIndex};
///
/// fn outlive(module: Module, target: TargetIndex) -> slang_compiler::Result<()> {
///     let layout = module.layout(target)?.into_value();
///     let ty = layout.find_type_by_name("float")?;
///     drop(module);
///     let _ = ty.map(|ty| ty.kind());
///     Ok(())
/// }
/// ```
///
/// A type handle cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::Type<'static>>();
/// ```
///
/// A type handle cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::Type<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct Type<'a> {
    raw: NonNull<TypeReflection>,
    session: &'a Session,
}

impl<'a> Type<'a> {
    /// Wraps a type that the caller found through the raw bindings of
    /// `slang-compiler-sys`, such as through a reflection function that this
    /// crate does not wrap.
    ///
    /// Prefer the safe sources of types, such as
    /// [`ProgramLayout::find_type_by_name`](super::ProgramLayout::find_type_by_name).
    ///
    /// # Examples
    ///
    /// This example finds a type by its name in a layout through the raw
    /// bindings:
    ///
    /// ```
    /// use std::ffi::CStr;
    /// use std::ptr::NonNull;
    ///
    /// use slang_compiler::Session;
    /// use slang_compiler::reflection::Type;
    /// use slang_compiler_sys::{SlangReflection, TypeReflection, spReflection_FindTypeByName};
    ///
    /// /// # Safety
    /// ///
    /// /// `layout` is a live layout of a module of `session`, which stays loaded
    /// /// while `session` lives. Breaking this is undefined behaviour.
    /// unsafe fn find<'session>(
    ///     session: &'session Session,
    ///     layout: NonNull<SlangReflection>,
    ///     name: &CStr,
    /// ) -> Option<Type<'session>> {
    ///     // SAFETY: the caller guarantees that `layout` is live, and `name` is a
    ///     // C string.
    ///     let raw = unsafe { spReflection_FindTypeByName(layout.as_ptr(), name.as_ptr()) };
    ///     let raw = NonNull::new(raw.cast::<TypeReflection>())?;
    ///     // SAFETY: Slang returned a non-null type of the session of `layout`,
    ///     // which `session` keeps alive.
    ///     Some(unsafe { Type::from_raw(raw, session) })
    /// }
    /// # let _ = find;
    /// ```
    ///
    /// # Safety
    ///
    /// The caller guarantees all of these, and breaking any of them is
    /// undefined behaviour:
    ///
    /// - `raw` points to a type that a reflection function of the Slang
    ///   library of the build tag [`slang_compiler_sys::SLANG_BUILD_TAG`]
    ///   returned for an object of `session`, such as the layout of one of its
    ///   modules.
    /// - The type stays alive while `session` lives, which holds for the types
    ///   of the modules that the session loaded and of the core module.
    /// - The type is not used on another thread while the returned value
    ///   exists.
    #[must_use]
    pub unsafe fn from_raw(raw: NonNull<TypeReflection>, session: &'a Session) -> Self {
        Self { raw, session }
    }

    /// Wraps a type that a reflection function returned.
    ///
    /// # Safety
    ///
    /// `raw` points to a type of the session `session`, which lives as long
    /// as the session, and `session` is borrowed for `'a` from a wrapper that
    /// a handle reached. Breaking this is undefined behaviour.
    pub(super) unsafe fn new(raw: NonNull<SlangReflectionType>, session: &'a Session) -> Self {
        Self {
            raw: raw.cast(),
            session,
        }
    }

    /// Wraps a type that a reflection function returned, or returns `None`
    /// for null.
    ///
    /// # Safety
    ///
    /// As for [`Type::new`], when `raw` is not null.
    pub(super) unsafe fn from_output(
        raw: *mut SlangReflectionType,
        session: &'a Session,
    ) -> Option<Self> {
        // SAFETY: the caller guarantees the conditions of `new`.
        NonNull::new(raw).map(|raw| unsafe { Self::new(raw, session) })
    }

    /// Returns the session that the type belongs to, so that a method can
    /// reject a type of another session.
    pub(crate) fn session(&self) -> &'a Session {
        self.session
    }

    fn raw(self) -> *mut SlangReflectionType {
        self.raw.as_ptr().cast()
    }

    /// Returns the kind of the type, such as a structure or an array.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when Slang
    /// reports a kind that the pinned Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn kind(self) -> Result<TypeKind> {
        let raw = guard::call("Type::kind", || {
            // SAFETY: `self.raw` is a live type of `self.session`, which keeps
            // it alive for `'a`, used only on this thread, and the call runs
            // with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetKind", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetKind(self.raw())
                })
            }
        });
        TypeKind::from_raw(raw)
    }

    /// Returns the name of the type as declared, such as `Particle`, or
    /// `None` when the type has no declared name, such as a vector or an
    /// array.
    ///
    /// # Examples
    ///
    /// This example prefers the declared name of a type and falls back to
    /// its full name:
    ///
    /// ```
    /// use slang_compiler::reflection::Type;
    ///
    /// fn label(ty: Type<'_>) -> slang_compiler::Result<String> {
    ///     Ok(match ty.name()? {
    ///         Some(name) => name.to_owned(),
    ///         None => ty.full_name()?,
    ///     })
    /// }
    /// # let _ = label;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidUtf8`](crate::ErrorKind::InvalidUtf8) when the
    /// name is not UTF-8.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn name(self) -> Result<Option<&'a str>> {
        let raw = guard::call("Type::name", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetName(self.raw())
                })
            }
        });
        // SAFETY: the name is in the name pool of the global session, which
        // the session keeps alive for `'a`.
        unsafe { borrowed_str(raw, Operation::Reflect, "the name of a type") }
    }

    /// Returns the full name of the type as Slang prints it, such as
    /// `vector<float,4>` or `Box<int>`, which every type has.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`].
    ///
    /// # Errors
    ///
    /// Returns the kind that Slang's result code maps to when Slang fails,
    /// an error of kind [`ErrorKind::Internal`](crate::ErrorKind::Internal)
    /// when Slang returns no name, and an error of kind
    /// [`ErrorKind::InvalidUtf8`](crate::ErrorKind::InvalidUtf8) when the
    /// name is not UTF-8.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn full_name(self) -> Result<String> {
        let operation = Operation::Reflect;
        guard::call("Type::full_name", || {
            let mut out: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: as for `kind`, and `out` is a live out-pointer.
            let result = unsafe {
                guard::abort_on_exception("spReflectionType_GetFullName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetFullName(self.raw(), &raw mut out)
                })
            };
            // SAFETY: `spReflectionType_GetFullName` returns a new blob
            // reference or null.
            let name = unsafe { Owned::adopt_output(out) };
            mapping::check(operation, result, Diagnostics::default())?;
            let name = name.ok_or_else(|| mapping::null(operation, Diagnostics::default()))?;
            blob::take_text(&name, operation, "the full name of a type")
        })
    }

    /// Returns the type of the elements of a scalar, vector, or matrix type,
    /// or [`ScalarType::None`] for any other type.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when Slang
    /// reports a scalar type that the pinned Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn scalar_type(self) -> Result<ScalarType> {
        let raw = guard::call("Type::scalar_type", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetScalarType", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetScalarType(self.raw())
                })
            }
        });
        ScalarType::from_raw(raw)
    }

    /// Returns the number of rows of a matrix type, 1 for a vector or a
    /// scalar, and 0 for any other type.
    ///
    /// Aborts the process, instead of returning a number, when the matrix
    /// has a number of rows that a generic value parameter gives, which Slang
    /// does not know until specialization, such as `matrix<float, R, 4>` in
    /// a generic function. Slang then lets a C++ exception escape, and the
    /// crate writes one line to standard error that names the call
    /// `spReflectionType_GetRowCount`, and aborts before any destructor runs.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn row_count(self) -> usize {
        let raw = guard::call("Type::row_count", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetRowCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetRowCount(self.raw())
                })
            }
        });
        count(raw)
    }

    /// Returns the number of columns of a matrix type, the number of
    /// elements of a vector type, 1 for a scalar, and 0 for any other type.
    ///
    /// Aborts the process, instead of returning a number, when the number
    /// comes from a generic value parameter, which Slang does not know until
    /// specialization, such as `vector<float, N>` in a generic function.
    /// Slang then lets a C++ exception escape, and the crate writes one line
    /// to standard error that names the call `spReflectionType_GetColumnCount`,
    /// and aborts before any destructor runs.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn column_count(self) -> usize {
        let raw = guard::call("Type::column_count", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetColumnCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetColumnCount(self.raw())
                })
            }
        });
        count(raw)
    }

    /// Returns an iterator over the fields of a structure type, or the cases
    /// of an enumeration type, each as a [`Variable`], which is empty for any
    /// other type.
    ///
    /// # Examples
    ///
    /// This example collects the names of the fields of a structure:
    ///
    /// ```
    /// use slang_compiler::reflection::Type;
    ///
    /// fn field_names(ty: Type<'_>) -> slang_compiler::Result<Vec<String>> {
    ///     let mut names = Vec::new();
    ///     for field in ty.fields() {
    ///         if let Some(name) = field.name()? {
    ///             names.push(name.to_owned());
    ///         }
    ///     }
    ///     Ok(names)
    /// }
    /// # let _ = field_names;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn fields(self) -> Fields<'a> {
        let raw_count = guard::call("Type::fields", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetFieldCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetFieldCount(self.raw())
                })
            }
        });
        Fields {
            ty: self,
            positions: Positions::new(count(raw_count)),
        }
    }

    /// Returns the type of the elements of an array, vector, matrix,
    /// constant buffer, parameter block, structured buffer, or output
    /// stream, the underlying integer type of an enumeration, or `None` for
    /// any other type.
    ///
    /// # Examples
    ///
    /// This example finds the innermost element type of nested arrays:
    ///
    /// ```
    /// use slang_compiler::reflection::{Type, TypeKind};
    ///
    /// fn innermost(mut ty: Type<'_>) -> slang_compiler::Result<Type<'_>> {
    ///     while ty.kind()? == TypeKind::Array {
    ///         match ty.element_type() {
    ///             Some(element) => ty = element,
    ///             None => break,
    ///         }
    ///     }
    ///     Ok(ty)
    /// }
    /// # let _ = innermost;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn element_type(self) -> Option<Type<'a>> {
        let raw = guard::call("Type::element_type", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetElementType", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetElementType(self.raw())
                })
            }
        });
        // SAFETY: Slang returned a type of the same session.
        unsafe { Self::from_output(raw, self.session) }
    }

    /// Returns the number of elements of an array or vector type, or
    /// `Finite(0)` for any other type.
    ///
    /// The count is `Finite(0)` also for an array whose length the source
    /// does not declare, and [`LayoutSize::Unknown`] for an array whose
    /// length linking fills in.
    ///
    /// Aborts the process, instead of returning a count, when the length
    /// comes from a generic value parameter, which Slang does not know until
    /// specialization, such as `float[N]` in a generic function. Slang then
    /// lets a C++ exception escape, and the crate writes one line to standard
    /// error that names the call `spReflectionType_GetElementCount`, and
    /// aborts before any destructor runs.
    ///
    /// # Examples
    ///
    /// This example reads the length of an array type when it is a number:
    ///
    /// ```
    /// use slang_compiler::reflection::{LayoutSize, Type};
    ///
    /// fn length(ty: Type<'_>) -> Option<u64> {
    ///     match ty.element_count() {
    ///         LayoutSize::Finite(count) => Some(count),
    ///         _ => None,
    ///     }
    /// }
    /// # let _ = length;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn element_count(self) -> LayoutSize {
        let raw = guard::call("Type::element_count", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetElementCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetElementCount(self.raw())
                })
            }
        });
        LayoutSize::from_raw(raw)
    }

    /// Returns the shape of a resource type, such as a two-dimensional
    /// texture or a structured buffer, or of the elements of an array of
    /// resources, with a base shape of
    /// [`ResourceBaseShape::None`](super::ResourceBaseShape::None) for any
    /// other type.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn resource_shape(self) -> ResourceShape {
        let raw = guard::call("Type::resource_shape", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetResourceShape", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetResourceShape(self.raw())
                })
            }
        });
        ResourceShape::from_raw(raw)
    }

    /// Returns how a shader may access a resource type, or the elements of
    /// an array of resources, or [`ResourceAccess::None`] for any other
    /// type.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when Slang
    /// reports an access that the pinned Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn resource_access(self) -> Result<ResourceAccess> {
        let raw = guard::call("Type::resource_access", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetResourceAccess", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetResourceAccess(self.raw())
                })
            }
        });
        ResourceAccess::from_raw(raw)
    }

    /// Returns the type of the values that a texture or a structured buffer
    /// holds, such as `float4`, or `None` for any other type.
    ///
    /// # Examples
    ///
    /// This example reads the element type of a buffer:
    ///
    /// ```
    /// use slang_compiler::reflection::Type;
    ///
    /// fn element_name(buffer: Type<'_>) -> slang_compiler::Result<Option<String>> {
    ///     buffer.resource_result_type().map(|ty| ty.full_name()).transpose()
    /// }
    /// # let _ = element_name;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn resource_result_type(self) -> Option<Type<'a>> {
        let raw = guard::call("Type::resource_result_type", || {
            // SAFETY: as for `kind`.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetResourceResultType", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetResourceResultType(self.raw())
                })
            }
        });
        // SAFETY: Slang returned a type of the same session.
        unsafe { Self::from_output(raw, self.session) }
    }

    /// Returns the pointer to the type, for use with the reflection
    /// functions of `slang-compiler-sys` that this crate does not wrap.
    ///
    /// The pointer is valid only while the object that the type borrows
    /// lives. Every use of it is unsafe and follows the safety contract of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example reads the kind of a type through the raw bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::Type;
    /// use slang_compiler_sys::{SlangReflectionType, SlangTypeKind, spReflectionType_GetKind};
    ///
    /// fn kind(ty: Type<'_>) -> SlangTypeKind {
    ///     // SAFETY: `ty` borrows its session, which keeps the type alive during
    ///     // the call, and only this thread uses it.
    ///     unsafe { spReflectionType_GetKind(ty.as_raw().as_ptr().cast::<SlangReflectionType>()) }
    /// }
    /// # let _ = kind;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<TypeReflection> {
        self.raw
    }
}

impl fmt::Debug for Type<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Type")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

/// An iterator over the fields of a structure type, or the cases of an
/// enumeration type, which [`Type::fields`] returns.
///
/// Each call of `next` asks Slang for the field at the next position. A
/// position at which Slang returns nothing yields no item, so the iterator
/// does not implement [`ExactSizeIterator`]. Each call of `next` panics when
/// it is called while another call of this crate into Slang is in progress
/// on the same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`Type::fields`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::Fields<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::Fields<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct Fields<'a> {
    ty: Type<'a>,
    positions: Positions,
}

impl<'a> Fields<'a> {
    fn at(&self, position: usize) -> Option<Variable<'a>> {
        let index = index(position)?;
        let raw = guard::call("Fields::next", || {
            // SAFETY: as for `Type::kind`, and `index` is below the count that
            // Slang reported, which bounds the index that
            // `spReflectionType_GetFieldByIndex` does not check.
            unsafe {
                guard::abort_on_exception("spReflectionType_GetFieldByIndex", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionType_GetFieldByIndex(self.ty.raw(), index)
                })
            }
        });
        // SAFETY: Slang returned a field of the type, which lives in the
        // syntax trees of the same session.
        unsafe { Variable::from_output(raw, self.ty.session) }
    }
}

position_iterator!(Fields<'a>, Variable<'a>);

/// A function of Slang code, as its declaration describes it.
///
/// Get one from [`EntryPoint::function`](crate::EntryPoint::function) or
/// [`EntryPointLayout::function`](super::EntryPointLayout::function). Slang
/// calls it `FunctionReflection`. It borrows the object that it came from, so
/// it cannot outlive it.
///
/// # Examples
///
/// This example prints the parameters of a function with their types:
///
/// ```
/// use slang_compiler::reflection::Function;
///
/// fn print_signature(function: Function<'_>) -> slang_compiler::Result<()> {
///     let name = function.name()?.unwrap_or("<unnamed>");
///     let mut parameters = Vec::new();
///     for parameter in function.parameters() {
///         let ty = parameter.ty().map(|ty| ty.full_name()).transpose()?;
///         parameters.push(format!(
///             "{}: {}",
///             parameter.name()?.unwrap_or("_"),
///             ty.unwrap_or_default()
///         ));
///     }
///     let result = function.return_type().map(|ty| ty.full_name()).transpose()?;
///     println!("{name}({}) -> {}", parameters.join(", "), result.unwrap_or_default());
///     Ok(())
/// }
/// # let _ = print_signature;
/// ```
///
/// A function cannot outlive the entry point that it came from, so this
/// example does not compile:
///
/// ```compile_fail,E0505
/// use slang_compiler::EntryPoint;
///
/// fn outlive(entry_point: EntryPoint) {
///     let function = entry_point.function();
///     drop(entry_point);
///     let _ = function.map(|function| function.name());
/// }
/// ```
///
/// A function handle cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::Function<'static>>();
/// ```
///
/// A function handle cannot be shared between threads either, so this
/// example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::Function<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct Function<'a> {
    raw: NonNull<SlangReflectionFunction>,
    session: &'a Session,
}

impl<'a> Function<'a> {
    /// Wraps a function that Slang returned.
    ///
    /// # Safety
    ///
    /// `raw` points to a function in the syntax trees of `session`, which
    /// live as long as the session, and `session` is borrowed for `'a` from
    /// the wrapper that the function came from. Breaking this is undefined
    /// behaviour.
    pub(crate) unsafe fn new(raw: NonNull<SlangReflectionFunction>, session: &'a Session) -> Self {
        Self { raw, session }
    }

    /// Returns the name of the function, or `None` when Slang returns none.
    ///
    /// # Examples
    ///
    /// See the example of [`Function`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidUtf8`](crate::ErrorKind::InvalidUtf8) when the
    /// name is not UTF-8.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn name(self) -> Result<Option<&'a str>> {
        let raw = guard::call("Function::name", || {
            // SAFETY: `self.raw` is a live function of `self.session`, which
            // keeps it alive for `'a`, used only on this thread, and the call
            // runs with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("spReflectionFunction_GetName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionFunction_GetName(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: the name is in the name pool of the global session, which
        // the session keeps alive for `'a`.
        unsafe { borrowed_str(raw, Operation::Reflect, "the name of a function") }
    }

    /// Returns an iterator over the parameters of the function, each as a
    /// [`Variable`].
    ///
    /// # Examples
    ///
    /// See the example of [`Function`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn parameters(self) -> Parameters<'a> {
        let raw_count = guard::call("Function::parameters", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionFunction_GetParameterCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionFunction_GetParameterCount(self.raw.as_ptr())
                })
            }
        });
        Parameters {
            function: self,
            positions: Positions::new(count(raw_count)),
        }
    }

    /// Returns the type that the function returns, or `None` when Slang
    /// returns none.
    ///
    /// # Examples
    ///
    /// See the example of [`Function`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn return_type(self) -> Option<Type<'a>> {
        let raw = guard::call("Function::return_type", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionFunction_GetResultType", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionFunction_GetResultType(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: Slang returned a type of the same session.
        unsafe { Type::from_output(raw, self.session) }
    }

    /// Returns the pointer to the function, for use with the reflection
    /// functions of `slang-compiler-sys` that this crate does not wrap.
    ///
    /// The pointer is valid only while the object that the function borrows
    /// lives. Every use of it is unsafe and follows the safety contract of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example counts the user attributes of a function through the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::Function;
    /// use slang_compiler_sys::spReflectionFunction_GetUserAttributeCount;
    ///
    /// fn attribute_count(function: Function<'_>) -> u32 {
    ///     // SAFETY: `function` keeps the function alive during the call, and
    ///     // only this thread uses it.
    ///     unsafe { spReflectionFunction_GetUserAttributeCount(function.as_raw().as_ptr()) }
    /// }
    /// # let _ = attribute_count;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<SlangReflectionFunction> {
        self.raw
    }
}

impl fmt::Debug for Function<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Function")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

/// An iterator over the parameters of a function, which
/// [`Function::parameters`] returns.
///
/// Each call of `next` asks Slang for the parameter at the next position. A
/// position at which Slang returns nothing yields no item, so the iterator
/// does not implement [`ExactSizeIterator`]. Each call of `next` panics when
/// it is called while another call of this crate into Slang is in progress
/// on the same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`Function`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::Parameters<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::Parameters<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct Parameters<'a> {
    function: Function<'a>,
    positions: Positions,
}

impl<'a> Parameters<'a> {
    fn at(&self, position: usize) -> Option<Variable<'a>> {
        let index = index(position)?;
        let raw = guard::call("Parameters::next", || {
            // SAFETY: as for `Function::name`, and `index` is below the count
            // that Slang reported, which bounds the index that
            // `spReflectionFunction_GetParameter` does not check.
            unsafe {
                guard::abort_on_exception("spReflectionFunction_GetParameter", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionFunction_GetParameter(self.function.raw.as_ptr(), index)
                })
            }
        });
        // SAFETY: Slang returned a parameter of the function, which lives in
        // the syntax trees of the same session.
        unsafe { Variable::from_output(raw, self.function.session) }
    }
}

position_iterator!(Parameters<'a>, Variable<'a>);
