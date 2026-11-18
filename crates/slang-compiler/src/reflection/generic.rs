use std::fmt;
use std::ptr::NonNull;

use slang_compiler_sys::{
    SlangReflectionGeneric, spReflectionGeneric_GetConcreteIntVal,
    spReflectionGeneric_GetConcreteType, spReflectionGeneric_GetInnerKind,
    spReflectionGeneric_GetName, spReflectionGeneric_GetOuterGenericContainer,
    spReflectionGeneric_GetTypeParameter, spReflectionGeneric_GetTypeParameterConstraintCount,
    spReflectionGeneric_GetTypeParameterConstraintType, spReflectionGeneric_GetTypeParameterCount,
    spReflectionGeneric_GetValueParameter, spReflectionGeneric_GetValueParameterCount,
    spReflectionGeneric_applySpecializations,
};

use super::{DeclKind, Positions, Type, Variable, count, index};
use crate::component::other_session;
use crate::error::{Error, ErrorKind, Operation, Result};
use crate::guard;
use crate::session::Session;
use crate::strings::borrowed_str;

/// A generic of Slang code, the list of generic parameters around a
/// declaration, such as `<T : IArithmetic, let N : int>` around a function
/// or a structure.
///
/// A generic parameter stands for a type, such as `T`, or for a value, such
/// as `N`, which the code that uses the declaration fills in. Get a generic
/// from [`Function::generic_container`](super::Function::generic_container)
/// or [`Type::generic_container`](super::Type::generic_container). Slang
/// calls it `GenericReflection`. It borrows the object that it came from, so
/// it cannot outlive it, and it belongs to the session of that object: a
/// method of another session rejects it.
///
/// # Examples
///
/// This example describes the generic parameters of a function, each type
/// parameter with the Slang interfaces that it requires, an `interface`
/// declaration that Slang types implement:
///
/// ```
/// use slang_compiler::reflection::{DeclKind, Function};
///
/// fn describe(function: Function<'_>) -> slang_compiler::Result<Vec<String>> {
///     let mut lines = Vec::new();
///     let Some(generic) = function.generic_container() else {
///         return Ok(lines);
///     };
///     assert_eq!(generic.inner_kind()?, DeclKind::Func);
///     for parameter in generic.type_parameters() {
///         let mut bounds = Vec::new();
///         for bound in generic.type_parameter_constraints(parameter)? {
///             bounds.push(bound.full_name()?);
///         }
///         lines.push(format!("{:?}: {}", parameter.name()?, bounds.join(" + ")));
///     }
///     for parameter in generic.value_parameters() {
///         lines.push(format!("let {:?}", parameter.name()?));
///     }
///     Ok(lines)
/// }
/// # let _ = describe;
/// ```
///
/// A generic cannot outlive the object that it came from, so this example
/// does not compile:
///
/// ```compile_fail,E0505
/// use slang_compiler::EntryPoint;
///
/// fn outlive(entry_point: EntryPoint) {
///     let generic = entry_point.function().and_then(|f| f.generic_container());
///     drop(entry_point);
///     let _ = generic.map(|generic| generic.name());
/// }
/// ```
///
/// A generic handle cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::Generic<'static>>();
/// ```
///
/// A generic handle cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::Generic<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct Generic<'a> {
    raw: NonNull<SlangReflectionGeneric>,
    session: &'a Session,
}

impl<'a> Generic<'a> {
    /// Wraps a generic that a reflection function returned, or returns
    /// `None` for null.
    ///
    /// # Safety
    ///
    /// When `raw` is not null, it points to a generic in the syntax trees of
    /// `session`, which live as long as the session, and `session` is
    /// borrowed for `'a` from a wrapper that a handle reached. Breaking this
    /// is undefined behaviour.
    pub(super) unsafe fn from_output(
        raw: *mut SlangReflectionGeneric,
        session: &'a Session,
    ) -> Option<Self> {
        NonNull::new(raw).map(|raw| Self { raw, session })
    }

    /// Returns the session that the generic belongs to, so that a method can
    /// reject a generic of another session.
    pub(super) fn session(&self) -> &'a Session {
        self.session
    }

    pub(super) fn raw(self) -> *mut SlangReflectionGeneric {
        self.raw.as_ptr()
    }

    /// Checks that `parameter` belongs to the session of this generic and is
    /// one of its value parameters when `value` is set, or one of its type
    /// parameters otherwise, because Slang treats the variable that it
    /// receives as such a parameter without a check.
    fn check_parameter(self, parameter: Variable<'a>, value: bool) -> Result<()> {
        if !parameter.session().is_same(self.session) {
            return Err(other_session(Operation::Reflect, "the parameter"));
        }
        let raw = parameter.as_raw();
        let (found, what) = if value {
            (
                self.value_parameters().any(|own| own.as_raw() == raw),
                "a value parameter",
            )
        } else {
            (
                self.type_parameters().any(|own| own.as_raw() == raw),
                "a type parameter",
            )
        };
        if found {
            Ok(())
        } else {
            Err(
                Error::new(ErrorKind::InvalidArgument, Operation::Reflect).with_detail(format!(
                    "the variable is not {what} of this generic, and Slang reads it as one without a check"
                )),
            )
        }
    }

    /// Returns the name of the declaration that the generic belongs to, such
    /// as `twice` for the generic of `T twice<T>(T value)`, or `None` when
    /// Slang returns none.
    ///
    /// # Examples
    ///
    /// This example names the innermost generic of a type:
    ///
    /// ```
    /// use slang_compiler::reflection::Type;
    ///
    /// fn generic_name(ty: Type<'_>) -> slang_compiler::Result<Option<&str>> {
    ///     match ty.generic_container() {
    ///         Some(generic) => generic.name(),
    ///         None => Ok(None),
    ///     }
    /// }
    /// # let _ = generic_name;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidUtf8`] when the name is
    /// not UTF-8.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn name(self) -> Result<Option<&'a str>> {
        let raw = guard::call("Generic::name", || {
            // SAFETY: `self.raw` is a live generic of `self.session`, which
            // keeps it alive for `'a`, used only on this thread, and the call
            // runs with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_GetName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_GetName(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: the name is in the name pool of the global session, which
        // the session keeps alive for `'a`.
        unsafe { borrowed_str(raw, Operation::Reflect, "the name of a generic") }
    }

    /// Returns an iterator over the type parameters of the generic, such as
    /// `T` in `<T : IArithmetic>`, each as a [`Variable`], in the order of
    /// the source.
    ///
    /// # Examples
    ///
    /// See the example of [`Generic`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn type_parameters(self) -> TypeParameters<'a> {
        let raw_count = guard::call("Generic::type_parameters", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_GetTypeParameterCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_GetTypeParameterCount(self.raw.as_ptr())
                })
            }
        });
        TypeParameters {
            generic: self,
            positions: Positions::new(count(raw_count)),
        }
    }

    /// Returns an iterator over the value parameters of the generic, such as
    /// `N` in `<let N : int>`, each as a [`Variable`], in the order of the
    /// source.
    ///
    /// # Examples
    ///
    /// See the example of [`Generic`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn value_parameters(self) -> ValueParameters<'a> {
        let raw_count = guard::call("Generic::value_parameters", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_GetValueParameterCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_GetValueParameterCount(self.raw.as_ptr())
                })
            }
        });
        ValueParameters {
            generic: self,
            positions: Positions::new(count(raw_count)),
        }
    }

    /// Returns an iterator over the types that the type parameter `parameter`
    /// of the generic must conform to, such as `IArithmetic` for
    /// `T : IArithmetic`.
    ///
    /// # Examples
    ///
    /// See the example of [`Generic`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`], without
    /// calling Slang with it, when `parameter` belongs to another session or
    /// is not one of the [`Generic::type_parameters`] of this generic. A
    /// parameter of an enclosing generic is read through the generic that
    /// [`Generic::outer_generic_container`] returns.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn type_parameter_constraints(
        self,
        parameter: Variable<'a>,
    ) -> Result<TypeParameterConstraints<'a>> {
        self.check_parameter(parameter, false)?;
        let raw_count = guard::call("Generic::type_parameter_constraints", || {
            // SAFETY: as for `name`, and `parameter` is a live type parameter
            // of this generic, which the check above established.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionGeneric_GetTypeParameterConstraintCount",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                        )]
                        spReflectionGeneric_GetTypeParameterConstraintCount(
                            self.raw.as_ptr(),
                            parameter.as_raw().as_ptr(),
                        )
                    },
                )
            }
        });
        Ok(TypeParameterConstraints {
            generic: self,
            parameter,
            positions: Positions::new(count(raw_count)),
        })
    }

    /// Returns the kind of the declaration inside the generic, such as
    /// [`DeclKind::Func`] for a generic function or [`DeclKind::Struct`] for
    /// a generic structure.
    ///
    /// # Examples
    ///
    /// See the example of [`Generic`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Unrecognized`] when Slang
    /// reports a kind that the Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn inner_kind(self) -> Result<DeclKind> {
        let raw = guard::call("Generic::inner_kind", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_GetInnerKind", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_GetInnerKind(self.raw.as_ptr())
                })
            }
        });
        DeclKind::from_raw(raw)
    }

    /// Returns the generic that the declaration of this generic sits in, such
    /// as the generic of a structure `Box<T>` for a generic method of it, or
    /// `None` when there is none.
    ///
    /// # Examples
    ///
    /// This example counts the generics around a function, from the
    /// innermost out:
    ///
    /// ```
    /// use slang_compiler::reflection::Function;
    ///
    /// fn depth(function: Function<'_>) -> usize {
    ///     std::iter::successors(function.generic_container(), |generic| {
    ///         generic.outer_generic_container()
    ///     })
    ///     .count()
    /// }
    /// # let _ = depth;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn outer_generic_container(self) -> Option<Generic<'a>> {
        let raw = guard::call("Generic::outer_generic_container", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_GetOuterGenericContainer", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_GetOuterGenericContainer(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: Slang returned a generic of the same session.
        unsafe { Self::from_output(raw, self.session) }
    }

    /// Returns the type that this generic fills in for its type parameter
    /// `parameter`, such as `float` for `T` in the generic of
    /// `twice<float>`, or `None` when the generic does not fill it in.
    ///
    /// # Examples
    ///
    /// This example reads the types that a specialized function fills in for
    /// its type parameters:
    ///
    /// ```
    /// use slang_compiler::reflection::Function;
    ///
    /// fn filled_in(function: Function<'_>) -> slang_compiler::Result<Vec<String>> {
    ///     let mut names = Vec::new();
    ///     if let Some(generic) = function.generic_container() {
    ///         for parameter in generic.type_parameters() {
    ///             if let Some(ty) = generic.concrete_type(parameter)? {
    ///                 names.push(ty.full_name()?);
    ///             }
    ///         }
    ///     }
    ///     Ok(names)
    /// }
    /// # let _ = filled_in;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`], without
    /// calling Slang with it, when `parameter` belongs to another session or
    /// is not one of the [`Generic::type_parameters`] of this generic. A
    /// parameter of an enclosing generic is read through the generic that
    /// [`Generic::outer_generic_container`] returns.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn concrete_type(self, parameter: Variable<'a>) -> Result<Option<Type<'a>>> {
        self.check_parameter(parameter, false)?;
        let raw = guard::call("Generic::concrete_type", || {
            // SAFETY: as for `type_parameter_constraints`.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_GetConcreteType", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_GetConcreteType(
                        self.raw.as_ptr(),
                        parameter.as_raw().as_ptr(),
                    )
                })
            }
        });
        // SAFETY: Slang returned a type of the same session.
        Ok(unsafe { Type::from_output(raw, self.session) })
    }

    /// Returns the integer that this generic fills in for its value parameter
    /// `parameter`, such as 4 for `N` in the generic of `sum<4>`.
    ///
    /// The recorded Slang returns 0 when the generic does not fill the
    /// parameter in with a constant, such as for the generic of a function
    /// that is not specialized, so 0 does not tell whether the parameter is
    /// filled in.
    ///
    /// # Examples
    ///
    /// This example reads the integers that a specialized function fills in
    /// for its value parameters:
    ///
    /// ```
    /// use slang_compiler::reflection::Function;
    ///
    /// fn values(function: Function<'_>) -> slang_compiler::Result<Vec<i64>> {
    ///     let mut values = Vec::new();
    ///     if let Some(generic) = function.generic_container() {
    ///         for parameter in generic.value_parameters() {
    ///             values.push(generic.concrete_int_val(parameter)?);
    ///         }
    ///     }
    ///     Ok(values)
    /// }
    /// # let _ = values;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`], without
    /// calling Slang with it, when `parameter` belongs to another session or
    /// is not one of the [`Generic::value_parameters`] of this generic. A
    /// parameter of an enclosing generic is read through the generic that
    /// [`Generic::outer_generic_container`] returns.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn concrete_int_val(self, parameter: Variable<'a>) -> Result<i64> {
        self.check_parameter(parameter, true)?;
        Ok(guard::call("Generic::concrete_int_val", || {
            // SAFETY: as for `name`, and `parameter` is a live value parameter
            // of this generic, which the check above established.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_GetConcreteIntVal", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_GetConcreteIntVal(
                        self.raw.as_ptr(),
                        parameter.as_raw().as_ptr(),
                    )
                })
            }
        }))
    }

    /// Returns this generic with the arguments of `generic` put in place of
    /// the generic parameters that it names, or `None` when Slang returns
    /// none.
    ///
    /// [`Function::apply_specializations`](super::Function::apply_specializations)
    /// and [`Type::apply_specializations`](super::Type::apply_specializations)
    /// do the same for a function and a type.
    ///
    /// # Examples
    ///
    /// This example applies the arguments of the generic of a specialized
    /// function to the function that it specializes:
    ///
    /// ```
    /// use slang_compiler::reflection::Function;
    ///
    /// fn reapply<'a>(
    ///     generic_function: Function<'a>,
    ///     specialized: Function<'a>,
    /// ) -> slang_compiler::Result<Option<Function<'a>>> {
    ///     match specialized.generic_container() {
    ///         Some(arguments) => generic_function.apply_specializations(arguments),
    ///         None => Ok(None),
    ///     }
    /// }
    /// # let _ = reapply;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`], without
    /// calling Slang, when `generic` belongs to another session.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn apply_specializations(self, generic: Generic<'a>) -> Result<Option<Generic<'a>>> {
        if !generic.session.is_same(self.session) {
            return Err(other_session(Operation::Reflect, "the generic"));
        }
        let raw = guard::call("Generic::apply_specializations", || {
            // SAFETY: as for `name`, and `generic` is a live generic of the
            // same session, which the check above established.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_applySpecializations", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_applySpecializations(
                        self.raw.as_ptr(),
                        generic.raw.as_ptr(),
                    )
                })
            }
        });
        // SAFETY: Slang returned a generic of the same session.
        Ok(unsafe { Self::from_output(raw, self.session) })
    }

    /// Returns the pointer to the generic, for use with the reflection
    /// functions of `slang-compiler-sys` that this crate does not wrap.
    ///
    /// The pointer is valid only while the object that the generic borrows
    /// lives. Every use of it is unsafe and follows the safety contract of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example passes the pointer on to code that uses the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::Generic;
    /// use slang_compiler_sys::SlangReflectionGeneric;
    ///
    /// fn raw_pointer(generic: Generic<'_>) -> *mut SlangReflectionGeneric {
    ///     generic.as_raw().as_ptr()
    /// }
    /// # let _ = raw_pointer;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<SlangReflectionGeneric> {
        self.raw
    }
}

impl fmt::Debug for Generic<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Generic")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

/// An iterator over the type parameters of a generic, which
/// [`Generic::type_parameters`] returns.
///
/// Each call of `next` asks Slang for the parameter at the next position. A
/// position at which Slang returns nothing yields no item, so the iterator
/// does not implement [`ExactSizeIterator`]. Each call of `next` panics when
/// it is called while another call of this crate into Slang is in progress
/// on the same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`Generic`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::TypeParameters<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::TypeParameters<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct TypeParameters<'a> {
    generic: Generic<'a>,
    positions: Positions,
}

impl<'a> TypeParameters<'a> {
    fn at(&self, position: usize) -> Option<Variable<'a>> {
        let index = index(position)?;
        let raw = guard::call("TypeParameters::next", || {
            // SAFETY: as for `Generic::name`, and `index` is below the count
            // that Slang reported, which bounds the index that
            // `spReflectionGeneric_GetTypeParameter` does not check.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_GetTypeParameter", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_GetTypeParameter(self.generic.raw.as_ptr(), index)
                })
            }
        });
        // SAFETY: Slang returned a parameter of the generic, which lives in
        // the syntax trees of the same session.
        unsafe { Variable::from_output(raw, self.generic.session) }
    }
}

position_iterator!(TypeParameters<'a>, Variable<'a>);

/// An iterator over the value parameters of a generic, which
/// [`Generic::value_parameters`] returns.
///
/// Each call of `next` asks Slang for the parameter at the next position. A
/// position at which Slang returns nothing yields no item, so the iterator
/// does not implement [`ExactSizeIterator`]. Each call of `next` panics when
/// it is called while another call of this crate into Slang is in progress
/// on the same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`Generic`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::ValueParameters<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::ValueParameters<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct ValueParameters<'a> {
    generic: Generic<'a>,
    positions: Positions,
}

impl<'a> ValueParameters<'a> {
    fn at(&self, position: usize) -> Option<Variable<'a>> {
        let index = index(position)?;
        let raw = guard::call("ValueParameters::next", || {
            // SAFETY: as for `Generic::name`, and `index` is below the count
            // that Slang reported, which bounds the index that
            // `spReflectionGeneric_GetValueParameter` does not check.
            unsafe {
                guard::abort_on_exception("spReflectionGeneric_GetValueParameter", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionGeneric_GetValueParameter(self.generic.raw.as_ptr(), index)
                })
            }
        });
        // SAFETY: Slang returned a parameter of the generic, which lives in
        // the syntax trees of the same session.
        unsafe { Variable::from_output(raw, self.generic.session) }
    }
}

position_iterator!(ValueParameters<'a>, Variable<'a>);

/// An iterator over the types that a type parameter of a generic must
/// conform to, which [`Generic::type_parameter_constraints`] returns.
///
/// Each call of `next` asks Slang for the type at the next position. A
/// position at which Slang returns nothing yields no item, so the iterator
/// does not implement [`ExactSizeIterator`]. Each call of `next` panics when
/// it is called while another call of this crate into Slang is in progress
/// on the same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`Generic`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::TypeParameterConstraints<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::TypeParameterConstraints<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct TypeParameterConstraints<'a> {
    generic: Generic<'a>,
    parameter: Variable<'a>,
    positions: Positions,
}

impl<'a> TypeParameterConstraints<'a> {
    fn at(&self, position: usize) -> Option<Type<'a>> {
        let index = index(position)?;
        let raw = guard::call("TypeParameterConstraints::next", || {
            // SAFETY: as for `Generic::name`, `self.parameter` is a type
            // parameter of the generic, which `type_parameter_constraints`
            // checked, and `index` is below the count that Slang reported,
            // which bounds the index that
            // `spReflectionGeneric_GetTypeParameterConstraintType` does not
            // check.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionGeneric_GetTypeParameterConstraintType",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                        )]
                        spReflectionGeneric_GetTypeParameterConstraintType(
                            self.generic.raw.as_ptr(),
                            self.parameter.as_raw().as_ptr(),
                            index,
                        )
                    },
                )
            }
        });
        // SAFETY: Slang returned a type of the same session.
        unsafe { Type::from_output(raw, self.generic.session) }
    }
}

position_iterator!(TypeParameterConstraints<'a>, Type<'a>);
