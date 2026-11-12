use std::fmt;
use std::ptr::NonNull;
use std::slice;
use std::str;

use slang_compiler_sys::{
    SlangReflectionTypeLayout, SlangReflectionUserAttribute, SlangReflectionVariable,
    SlangReflectionVariableLayout, spReflectionUserAttribute_GetArgumentCount,
    spReflectionUserAttribute_GetArgumentType, spReflectionUserAttribute_GetArgumentValueFloat,
    spReflectionUserAttribute_GetArgumentValueInt,
    spReflectionUserAttribute_GetArgumentValueString, spReflectionUserAttribute_GetName,
    spReflectionVariable_FindUserAttributeByName, spReflectionVariable_GetName,
    spReflectionVariable_GetType, spReflectionVariable_GetUserAttribute,
    spReflectionVariable_GetUserAttributeCount, spReflectionVariableLayout_GetOffset,
    spReflectionVariableLayout_GetSemanticIndex, spReflectionVariableLayout_GetSemanticName,
    spReflectionVariableLayout_GetSpace, spReflectionVariableLayout_GetTypeLayout,
    spReflectionVariableLayout_GetVariable, spReflectionVariableLayout_getStage,
};

use super::{
    LayoutSize, LayoutUnits, Positions, Type, TypeLayout, binding_number, count, index,
    out_of_range,
};
use crate::blob::BindingSpace;
use crate::enums::{ParameterCategory, Stage};
use crate::error::{Diagnostics, Error, ErrorKind, Operation, Result};
use crate::guard;
use crate::mapping;
use crate::session::Session;
use crate::strings::{borrowed_str, c_string, invalid_utf8};

/// The layout of one variable, such as a parameter or a field of a
/// structure: its offset in each parameter category and the layout of its
/// type.
///
/// A parameter category is a kind of resource that a layout counts
/// separately, such as bytes of uniform data or the descriptor slots of
/// Vulkan, the numbered bindings through which a shader reaches its
/// resources. Get a variable layout from an entry point layout, a program
/// layout, or a type layout. Slang calls it `VariableLayoutReflection`. It
/// borrows the component type of its program layout, so it cannot outlive
/// it.
///
/// # Examples
///
/// This example prints where a parameter sits: its offset in bytes of
/// uniform data, or, for a resource on Vulkan, its descriptor binding and
/// descriptor set:
///
/// ```
/// use slang_compiler::ParameterCategory;
/// use slang_compiler::reflection::VariableLayout;
///
/// fn print_location(parameter: VariableLayout<'_>) -> slang_compiler::Result<()> {
///     let name = parameter.variable().map(|v| v.name()).transpose()?.flatten();
///     let category = parameter
///         .type_layout()
///         .map(|t| t.parameter_category())
///         .transpose()?
///         .flatten();
///     match category {
///         Some(ParameterCategory::Uniform) => println!(
///             "{name:?}: offset {:?}",
///             parameter.offset(ParameterCategory::Uniform).amount
///         ),
///         Some(category) => println!(
///             "{name:?}: binding {:?} in set {:?}",
///             parameter.offset(category).amount,
///             parameter.binding_space(category)
///         ),
///         None => println!("{name:?}: uses no binding"),
///     }
///     Ok(())
/// }
/// # let _ = print_location;
/// ```
///
/// A variable layout cannot outlive the program that it came from, so this
/// example does not compile:
///
/// ```compile_fail,E0505
/// use slang_compiler::{LinkedProgram, TargetIndex};
///
/// fn outlive(program: LinkedProgram, target: TargetIndex) -> slang_compiler::Result<()> {
///     let layout = program.layout(target)?.into_value();
///     let globals = layout.global_params_var_layout();
///     drop(program);
///     let _ = globals.map(|globals| globals.semantic_index());
///     Ok(())
/// }
/// ```
///
/// A variable layout cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::VariableLayout<'static>>();
/// ```
///
/// A variable layout cannot be shared between threads either, so this
/// example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::VariableLayout<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct VariableLayout<'a> {
    raw: NonNull<SlangReflectionVariableLayout>,
    session: &'a Session,
}

impl<'a> VariableLayout<'a> {
    /// Wraps a variable layout that a reflection function returned.
    ///
    /// # Safety
    ///
    /// `raw` points to a variable layout of a program layout or of a type
    /// layout in the cache of a session, which a handle of lifetime `'a` of
    /// `session` reached, and so lives for `'a`. Breaking this is undefined
    /// behaviour.
    pub(super) unsafe fn new(
        raw: NonNull<SlangReflectionVariableLayout>,
        session: &'a Session,
    ) -> Self {
        Self { raw, session }
    }

    /// Wraps a variable layout that a reflection function returned, or
    /// returns `None` for null.
    ///
    /// # Safety
    ///
    /// As for [`VariableLayout::new`], when `raw` is not null.
    pub(super) unsafe fn from_output(
        raw: *mut SlangReflectionVariableLayout,
        session: &'a Session,
    ) -> Option<Self> {
        // SAFETY: the caller guarantees the conditions of `new`.
        NonNull::new(raw).map(|raw| unsafe { Self::new(raw, session) })
    }

    /// Returns the variable whose layout this is, with its name and type,
    /// or `None` when Slang returns none, such as for the variable layout of
    /// all the parameters of an entry point taken together.
    ///
    /// # Examples
    ///
    /// See the example of [`VariableLayout`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn variable(self) -> Option<Variable<'a>> {
        let raw = guard::call("VariableLayout::variable", || {
            // SAFETY: `self.raw` is a live variable layout, which the wrapper
            // that `'a` borrows keeps alive, used only on this thread, and the
            // call runs with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("spReflectionVariableLayout_GetVariable", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariableLayout_GetVariable(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: Slang returned the variable of the layout, which lives in
        // the syntax trees of the session.
        unsafe { Variable::from_output(raw, self.session) }
    }

    /// Returns the layout of the type of the variable, or `None` when Slang
    /// returns none.
    ///
    /// # Examples
    ///
    /// See the example of [`VariableLayout`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn type_layout(self) -> Option<TypeLayout<'a>> {
        let raw: *mut SlangReflectionTypeLayout = guard::call(
            "VariableLayout::type_layout",
            || {
                // SAFETY: as for `variable`.
                unsafe {
                    guard::abort_on_exception("spReflectionVariableLayout_GetTypeLayout", || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionVariableLayout_GetTypeLayout(self.raw.as_ptr())
                    })
                }
            },
        );
        // SAFETY: Slang returned the type layout of the variable layout,
        // which the same owner keeps alive.
        unsafe { TypeLayout::from_output(raw, self.session) }
    }

    /// Returns the offset of the variable in the units of `category`, such
    /// as its byte offset in uniform data, or its binding number among the
    /// descriptor slots of Vulkan.
    ///
    /// The offset is relative to the variable or scope that contains it.
    /// When the variable uses nothing of `category`, Slang tries a category
    /// that the caller may have meant, such as the descriptor slots of
    /// Vulkan for a texture asked about as a shader resource of Direct3D,
    /// and the offset is `Finite(0)` when it finds none.
    ///
    /// # Examples
    ///
    /// See the example of [`VariableLayout`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn offset(self, category: ParameterCategory) -> LayoutUnits {
        let raw = guard::call("VariableLayout::offset", || {
            // SAFETY: as for `variable`.
            unsafe {
                guard::abort_on_exception("spReflectionVariableLayout_GetOffset", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariableLayout_GetOffset(self.raw.as_ptr(), category.to_raw())
                })
            }
        });
        LayoutUnits::new(category, LayoutSize::from_raw(raw))
    }

    /// Returns the binding space of the variable for `category`, which
    /// Vulkan calls a descriptor set and Direct3D a register space, a
    /// numbered group of bindings.
    ///
    /// The space is relative to the variable or scope that contains it, and
    /// `None` when Slang does not know it, which happens when Slang cannot
    /// compute the space of a register space offset of the variable or when
    /// the sum of the offsets overflows.
    ///
    /// # Examples
    ///
    /// See the example of [`VariableLayout`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn binding_space(self, category: ParameterCategory) -> Option<BindingSpace> {
        let raw = guard::call("VariableLayout::binding_space", || {
            // SAFETY: as for `variable`.
            unsafe {
                guard::abort_on_exception("spReflectionVariableLayout_GetSpace", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariableLayout_GetSpace(self.raw.as_ptr(), category.to_raw())
                })
            }
        });
        binding_number(raw).map(BindingSpace::new)
    }

    /// Returns the name of the semantic of the variable, a name that marks
    /// the meaning of a shader input or output, such as
    /// `SV_DispatchThreadID`, or `None` when the variable has none.
    ///
    /// # Examples
    ///
    /// This example prints the semantic of a parameter with its index:
    ///
    /// ```
    /// use slang_compiler::reflection::VariableLayout;
    ///
    /// fn print_semantic(parameter: VariableLayout<'_>) -> slang_compiler::Result<()> {
    ///     if let Some(name) = parameter.semantic_name()? {
    ///         println!("{name}{}", parameter.semantic_index());
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = print_semantic;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidUtf8`] when the name is not UTF-8.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn semantic_name(self) -> Result<Option<&'a str>> {
        let raw = guard::call("VariableLayout::semantic_name", || {
            // SAFETY: as for `variable`.
            unsafe {
                guard::abort_on_exception("spReflectionVariableLayout_GetSemanticName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariableLayout_GetSemanticName(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: the name is held by the variable layout, which lives for
        // `'a`.
        unsafe { borrowed_str(raw, Operation::Reflect, "the semantic name of a variable") }
    }

    /// Returns the index of the semantic of the variable, such as 1 for
    /// `TEXCOORD1`, or 0 when the variable has no semantic.
    ///
    /// # Examples
    ///
    /// See the example of [`VariableLayout::semantic_name`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn semantic_index(self) -> usize {
        guard::call("VariableLayout::semantic_index", || {
            // SAFETY: as for `variable`.
            unsafe {
                guard::abort_on_exception("spReflectionVariableLayout_GetSemanticIndex", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariableLayout_GetSemanticIndex(self.raw.as_ptr())
                })
            }
        })
    }

    /// Returns the stage whose input or output the variable is, such as the
    /// vertex stage for a vertex input, or `None` for a variable that is not
    /// an input or output of a stage.
    ///
    /// # Examples
    ///
    /// This example tells whether a parameter is an input or output of a
    /// stage:
    ///
    /// ```
    /// use slang_compiler::reflection::VariableLayout;
    ///
    /// fn is_varying(parameter: VariableLayout<'_>) -> slang_compiler::Result<bool> {
    ///     Ok(parameter.stage()?.is_some())
    /// }
    /// # let _ = is_varying;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Unrecognized`] when Slang
    /// reports a stage that the pinned Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn stage(self) -> Result<Option<Stage>> {
        let raw = guard::call("VariableLayout::stage", || {
            // SAFETY: as for `variable`.
            unsafe {
                guard::abort_on_exception("spReflectionVariableLayout_getStage", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariableLayout_getStage(self.raw.as_ptr())
                })
            }
        });
        Stage::from_raw(raw)
    }

    /// Returns the pointer to the variable layout, for use with the
    /// reflection functions of `slang-compiler-sys` that this crate does not
    /// wrap.
    ///
    /// The pointer is valid only while the object that the layout borrows
    /// lives. Every use of it is unsafe and follows the safety contract of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example reads the image format of a variable through the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::VariableLayout;
    /// use slang_compiler_sys::{SlangImageFormat, spReflectionVariableLayout_GetImageFormat};
    ///
    /// fn image_format(parameter: VariableLayout<'_>) -> SlangImageFormat {
    ///     // SAFETY: `parameter` keeps the layout alive during the call, and
    ///     // only this thread uses it.
    ///     unsafe { spReflectionVariableLayout_GetImageFormat(parameter.as_raw().as_ptr()) }
    /// }
    /// # let _ = image_format;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<SlangReflectionVariableLayout> {
        self.raw
    }
}

impl fmt::Debug for VariableLayout<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("VariableLayout")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

/// A variable of Slang code as declared, such as a parameter, a field of a
/// structure, or a case of an enumeration, with its name and type.
///
/// Get one from [`VariableLayout::variable`], [`Type::fields`], or
/// [`Function::parameters`](super::Function::parameters). Slang calls it
/// `VariableReflection`. It borrows the object that it came from, so it
/// cannot outlive it.
///
/// # Examples
///
/// This example reads the user attributes of a variable, attributes with a
/// name that the source chooses, such as `[Range(0, 10)]`:
///
/// ```
/// use slang_compiler::reflection::Variable;
///
/// fn attribute_names(variable: Variable<'_>) -> slang_compiler::Result<Vec<String>> {
///     let mut names = Vec::new();
///     for attribute in variable.user_attributes() {
///         if let Some(name) = attribute.name()? {
///             names.push(name.to_owned());
///         }
///     }
///     Ok(names)
/// }
/// # let _ = attribute_names;
/// ```
///
/// A variable cannot outlive the program that it came from, so this example
/// does not compile:
///
/// ```compile_fail,E0505
/// use slang_compiler::{LinkedProgram, TargetIndex};
///
/// fn outlive(program: LinkedProgram, target: TargetIndex) -> slang_compiler::Result<()> {
///     let layout = program.layout(target)?.into_value();
///     let first = layout.parameter_layouts().next().and_then(|p| p.variable());
///     drop(program);
///     let _ = first.map(|variable| variable.name());
///     Ok(())
/// }
/// ```
///
/// A variable handle cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::Variable<'static>>();
/// ```
///
/// A variable handle cannot be shared between threads either, so this
/// example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::Variable<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct Variable<'a> {
    raw: NonNull<SlangReflectionVariable>,
    session: &'a Session,
}

impl<'a> Variable<'a> {
    /// Wraps a variable that a reflection function returned, or returns
    /// `None` for null.
    ///
    /// # Safety
    ///
    /// When `raw` is not null, it points to a variable in the syntax trees of
    /// `session`, which live as long as the session, and `session` is
    /// borrowed for `'a` from a wrapper that a handle reached. Breaking this
    /// is undefined behaviour.
    pub(super) unsafe fn from_output(
        raw: *mut SlangReflectionVariable,
        session: &'a Session,
    ) -> Option<Self> {
        NonNull::new(raw).map(|raw| Self { raw, session })
    }

    /// Returns the name of the variable, or `None` when it has none.
    ///
    /// # Examples
    ///
    /// See the example of [`Type::fields`].
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
        let raw = guard::call("Variable::name", || {
            // SAFETY: `self.raw` is a live variable of `self.session`, which
            // keeps it alive for `'a`, used only on this thread, and the call
            // runs with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("spReflectionVariable_GetName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariable_GetName(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: the name is in the name pool of the global session or a
        // string literal, which live for `'a`.
        unsafe { borrowed_str(raw, Operation::Reflect, "the name of a variable") }
    }

    /// Returns the type of the variable, or `None` when Slang returns none.
    ///
    /// # Examples
    ///
    /// See the example of [`Function`](super::Function).
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn ty(self) -> Option<Type<'a>> {
        let raw = guard::call("Variable::ty", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionVariable_GetType", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariable_GetType(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: Slang returned a type of the same session.
        unsafe { Type::from_output(raw, self.session) }
    }

    /// Returns an iterator over the user attributes of the variable,
    /// attributes with a name that the source chooses, such as
    /// `[Range(0, 10)]`, in the order of the source.
    ///
    /// # Examples
    ///
    /// See the example of [`Variable`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn user_attributes(self) -> UserAttributes<'a> {
        let raw_count = guard::call("Variable::user_attributes", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionVariable_GetUserAttributeCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariable_GetUserAttributeCount(self.raw.as_ptr())
                })
            }
        });
        UserAttributes {
            variable: self,
            positions: Positions::new(count(raw_count)),
        }
    }

    /// Returns the attribute of the variable named `name`, or `None` when
    /// the variable has none of that name.
    ///
    /// # Examples
    ///
    /// This example reads the first argument of the attribute `Range` as an
    /// integer:
    ///
    /// ```
    /// use slang_compiler::reflection::Variable;
    ///
    /// fn range_start(variable: Variable<'_>) -> slang_compiler::Result<Option<i32>> {
    ///     match variable.find_user_attribute_by_name("Range")? {
    ///         Some(range) if range.argument_count() > 0 => range.argument_value_int(0),
    ///         _ => Ok(None),
    ///     }
    /// }
    /// # let _ = range_start;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `name`
    /// contains a NUL byte, which Slang cannot take.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn find_user_attribute_by_name(self, name: &str) -> Result<Option<Attribute<'a>>> {
        let name = c_string(name, Operation::Reflect, "the attribute name")?;
        let global = self.session.global_session().as_raw();
        let raw = guard::call("Variable::find_user_attribute_by_name", || {
            // SAFETY: as for `name`, `global` is the live global session of
            // the session of the variable, which owns the name pool that the
            // call looks the name up in, and `name` is a C string that lives
            // until the call returns.
            unsafe {
                guard::abort_on_exception("spReflectionVariable_FindUserAttributeByName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariable_FindUserAttributeByName(
                        self.raw.as_ptr(),
                        global.as_ptr(),
                        name.as_ptr(),
                    )
                })
            }
        });
        // SAFETY: Slang returned an attribute of the variable, which lives in
        // the syntax trees of the same session.
        Ok(unsafe { Attribute::from_output(raw, self.session) })
    }

    /// Returns the pointer to the variable, for use with the reflection
    /// functions of `slang-compiler-sys` that this crate does not wrap.
    ///
    /// The pointer is valid only while the object that the variable borrows
    /// lives. Every use of it is unsafe and follows the safety contract of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example tells whether a variable has a modifier through the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::Variable;
    /// use slang_compiler_sys::{SlangModifierID, spReflectionVariable_FindModifier};
    ///
    /// fn has_modifier(variable: Variable<'_>, modifier: SlangModifierID) -> bool {
    ///     // SAFETY: `variable` keeps the variable alive during the call, and
    ///     // only this thread uses it.
    ///     !unsafe { spReflectionVariable_FindModifier(variable.as_raw().as_ptr(), modifier) }
    ///         .is_null()
    /// }
    /// # let _ = has_modifier;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<SlangReflectionVariable> {
        self.raw
    }
}

impl fmt::Debug for Variable<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Variable")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

/// An iterator over the user attributes of a variable, which
/// [`Variable::user_attributes`] returns.
///
/// Each call of `next` asks Slang for the attribute at the next position. A
/// position at which Slang returns nothing yields no item, so the iterator
/// does not implement [`ExactSizeIterator`]. Each call of `next` panics when
/// it is called while another call of this crate into Slang is in progress
/// on the same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`Variable`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::UserAttributes<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::UserAttributes<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct UserAttributes<'a> {
    variable: Variable<'a>,
    positions: Positions,
}

impl<'a> UserAttributes<'a> {
    fn at(&self, position: usize) -> Option<Attribute<'a>> {
        let index = index(position)?;
        let raw = guard::call("UserAttributes::next", || {
            // SAFETY: as for `Variable::name`, and `index` is below the count
            // that Slang reported.
            unsafe {
                guard::abort_on_exception("spReflectionVariable_GetUserAttribute", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionVariable_GetUserAttribute(self.variable.raw.as_ptr(), index)
                })
            }
        });
        // SAFETY: Slang returned an attribute of the variable, which lives in
        // the syntax trees of the same session.
        unsafe { Attribute::from_output(raw, self.variable.session) }
    }
}

position_iterator!(UserAttributes<'a>, Attribute<'a>);

/// A user attribute of a declaration, an attribute with a name that the
/// source chooses, such as `[Range(0, 10)]`, with its arguments.
///
/// Get one from [`Variable::user_attributes`] or
/// [`Variable::find_user_attribute_by_name`]. Slang calls it `Attribute` or
/// `UserAttribute`. It borrows the object that it came from, so it cannot
/// outlive it.
///
/// # Examples
///
/// This example prints each argument of an attribute as an integer, a
/// floating-point number, or a string, whichever it is:
///
/// ```
/// use slang_compiler::reflection::Attribute;
///
/// fn print_arguments(attribute: Attribute<'_>) -> slang_compiler::Result<()> {
///     for position in 0..attribute.argument_count() {
///         if let Some(value) = attribute.argument_value_int(position)? {
///             println!("{position}: {value}");
///         } else if let Some(value) = attribute.argument_value_float(position)? {
///             println!("{position}: {value}");
///         } else if let Some(value) = attribute.argument_value_string(position)? {
///             println!("{position}: {value:?}");
///         }
///     }
///     Ok(())
/// }
/// # let _ = print_arguments;
/// ```
///
/// An attribute handle cannot move to another thread, so this example does
/// not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::Attribute<'static>>();
/// ```
///
/// An attribute handle cannot be shared between threads either, so this
/// example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::Attribute<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct Attribute<'a> {
    raw: NonNull<SlangReflectionUserAttribute>,
    session: &'a Session,
}

impl<'a> Attribute<'a> {
    /// Wraps an attribute that a reflection function returned, or returns
    /// `None` for null.
    ///
    /// # Safety
    ///
    /// When `raw` is not null, it points to an attribute in the syntax trees
    /// of `session`, which live as long as the session, and `session` is
    /// borrowed for `'a` from a wrapper that a handle reached. Breaking this
    /// is undefined behaviour.
    unsafe fn from_output(
        raw: *mut SlangReflectionUserAttribute,
        session: &'a Session,
    ) -> Option<Self> {
        NonNull::new(raw).map(|raw| Self { raw, session })
    }

    /// Checks `position` against the number of arguments before a call
    /// that indexes the arguments, because Slang does not check it.
    fn check(self, position: usize) -> Result<u32> {
        let count = self.argument_count();
        if position >= count {
            return Err(out_of_range("argument", position, count));
        }
        index(position).ok_or_else(|| out_of_range("argument", position, count))
    }

    /// Returns the name of the attribute, or `None` when Slang returns none.
    ///
    /// # Examples
    ///
    /// See the example of [`Variable`].
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
        let raw = guard::call("Attribute::name", || {
            // SAFETY: `self.raw` is a live attribute of `self.session`, which
            // keeps it alive for `'a`, used only on this thread, and the call
            // runs with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("spReflectionUserAttribute_GetName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionUserAttribute_GetName(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: the name is in the name pool of the global session, which
        // the session keeps alive for `'a`.
        unsafe { borrowed_str(raw, Operation::Reflect, "the name of an attribute") }
    }

    /// Returns the number of arguments of the attribute.
    ///
    /// # Examples
    ///
    /// See the example of [`Attribute`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn argument_count(self) -> usize {
        let raw = guard::call("Attribute::argument_count", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionUserAttribute_GetArgumentCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionUserAttribute_GetArgumentCount(self.raw.as_ptr())
                })
            }
        });
        count(raw)
    }

    /// Returns the type of the argument at `position`, or `None` when Slang
    /// returns none.
    ///
    /// # Examples
    ///
    /// This example collects the type names of the arguments of an
    /// attribute:
    ///
    /// ```
    /// use slang_compiler::reflection::Attribute;
    ///
    /// fn argument_types(attribute: Attribute<'_>) -> slang_compiler::Result<Vec<String>> {
    ///     let mut names = Vec::new();
    ///     for position in 0..attribute.argument_count() {
    ///         if let Some(ty) = attribute.argument_type(position)? {
    ///             names.push(ty.full_name()?);
    ///         }
    ///     }
    ///     Ok(names)
    /// }
    /// # let _ = argument_types;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when
    /// `position` is not below [`Attribute::argument_count`], which the
    /// crate checks before it calls Slang, because Slang does not check it.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn argument_type(self, position: usize) -> Result<Option<Type<'a>>> {
        let index = self.check(position)?;
        let raw = guard::call("Attribute::argument_type", || {
            // SAFETY: as for `name`, and `index` is below the count that Slang
            // reported, which bounds the index that
            // `spReflectionUserAttribute_GetArgumentType` does not check.
            unsafe {
                guard::abort_on_exception("spReflectionUserAttribute_GetArgumentType", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionUserAttribute_GetArgumentType(self.raw.as_ptr(), index)
                })
            }
        });
        // SAFETY: Slang returned a type of the same session.
        Ok(unsafe { Type::from_output(raw, self.session) })
    }

    /// Returns the argument at `position` as an integer, or `None` when it
    /// is not an integer constant.
    ///
    /// # Examples
    ///
    /// See the example of [`Attribute`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when
    /// `position` is not below [`Attribute::argument_count`], which the
    /// crate checks before it calls Slang, and the kind that Slang's result
    /// code maps to when Slang fails for another reason.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn argument_value_int(self, position: usize) -> Result<Option<i32>> {
        let index = self.check(position)?;
        let mut value = 0;
        let result = guard::call("Attribute::argument_value_int", || {
            // SAFETY: as for `name`, `index` is below the count, and `value`
            // is a live out-pointer.
            unsafe {
                guard::abort_on_exception("spReflectionUserAttribute_GetArgumentValueInt", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionUserAttribute_GetArgumentValueInt(
                        self.raw.as_ptr(),
                        index,
                        &raw mut value,
                    )
                })
            }
        });
        Self::value_or_none(result, value)
    }

    /// Returns the argument at `position` as a floating-point number, or
    /// `None` when it is neither a floating-point nor an integer literal.
    ///
    /// # Examples
    ///
    /// See the example of [`Attribute`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when
    /// `position` is not below [`Attribute::argument_count`], which the
    /// crate checks before it calls Slang, and the kind that Slang's result
    /// code maps to when Slang fails for another reason.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn argument_value_float(self, position: usize) -> Result<Option<f32>> {
        let index = self.check(position)?;
        let mut value = 0.0;
        let result = guard::call("Attribute::argument_value_float", || {
            // SAFETY: as for `name`, `index` is below the count, and `value`
            // is a live out-pointer.
            unsafe {
                guard::abort_on_exception("spReflectionUserAttribute_GetArgumentValueFloat", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionUserAttribute_GetArgumentValueFloat(
                        self.raw.as_ptr(),
                        index,
                        &raw mut value,
                    )
                })
            }
        });
        Self::value_or_none(result, value)
    }

    /// Returns the value of a successful read of an argument, `None` when
    /// Slang reports that the argument is not of the kind asked for, which
    /// it reports with a code that maps to `InvalidArgument` after the crate
    /// checked the position, and an error for any other failure.
    fn value_or_none<T>(result: slang_compiler_sys::SlangResult, value: T) -> Result<Option<T>> {
        match mapping::check(Operation::Reflect, result, Diagnostics::default()) {
            Ok(()) => Ok(Some(value)),
            Err(error) if error.kind() == ErrorKind::InvalidArgument => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Returns the argument at `position` as a string, or `None` when it is
    /// not a string literal.
    ///
    /// # Examples
    ///
    /// See the example of [`Attribute`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when
    /// `position` is not below [`Attribute::argument_count`], which the
    /// crate checks before it calls Slang, an error of kind
    /// [`ErrorKind::InvalidUtf8`] when the string is not UTF-8, and an error
    /// of kind [`ErrorKind::Internal`] when Slang reports a length that no
    /// string can have.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn argument_value_string(self, position: usize) -> Result<Option<&'a str>> {
        let index = self.check(position)?;
        let mut len: usize = 0;
        let raw = guard::call("Attribute::argument_value_string", || {
            // SAFETY: as for `name`, `index` is below the count, and `len` is
            // a live out-pointer.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionUserAttribute_GetArgumentValueString",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionUserAttribute_GetArgumentValueString(
                            self.raw.as_ptr(),
                            index,
                            &raw mut len,
                        )
                    },
                )
            }
        });
        let Some(data) = NonNull::new(raw.cast_mut()) else {
            return Ok(None);
        };
        if isize::try_from(len).is_err() {
            return Err(
                Error::new(ErrorKind::Internal, Operation::Reflect).with_detail(format!(
                    "Slang reported a string argument of {len} bytes, which no string can have"
                )),
            );
        }
        // SAFETY: `data` is non-null and points to the `len` bytes of the
        // string literal of the attribute, which lives in the syntax trees
        // of the session for `'a` and does not change, and `len` fits
        // `isize`.
        let bytes = unsafe { slice::from_raw_parts(data.as_ptr().cast::<u8>(), len) };
        str::from_utf8(bytes).map(Some).map_err(|error| {
            invalid_utf8(
                Operation::Reflect,
                "a string argument of an attribute",
                error,
            )
        })
    }

    /// Returns the pointer to the attribute, for use with the reflection
    /// functions of `slang-compiler-sys` that this crate does not wrap.
    ///
    /// The pointer is valid only while the object that the attribute borrows
    /// lives. Every use of it is unsafe and follows the safety contract of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example passes the pointer on to code that uses the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::Attribute;
    /// use slang_compiler_sys::SlangReflectionUserAttribute;
    ///
    /// fn raw_pointer(attribute: Attribute<'_>) -> *mut SlangReflectionUserAttribute {
    ///     attribute.as_raw().as_ptr()
    /// }
    /// # let _ = raw_pointer;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<SlangReflectionUserAttribute> {
        self.raw
    }
}

impl fmt::Debug for Attribute<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Attribute")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::SlangResult;

    use super::Attribute;
    use crate::{ErrorKind, Operation};

    #[test]
    fn value_or_none_keeps_a_value_and_reads_an_argument_of_another_kind_as_none() {
        assert_eq!(
            Attribute::value_or_none(SlangResult::SLANG_OK, 7).ok(),
            Some(Some(7))
        );
        assert_eq!(
            Attribute::value_or_none(SlangResult::SLANG_E_INVALID_ARG, 7).ok(),
            Some(None)
        );
    }

    #[test]
    fn value_or_none_passes_on_every_other_failure() {
        for (result, kind) in [
            (SlangResult::SLANG_FAIL, ErrorKind::Internal),
            (SlangResult::SLANG_E_NOT_AVAILABLE, ErrorKind::Unsupported),
        ] {
            let error = Attribute::value_or_none(result, 7).expect_err("the read failed");
            assert_eq!(error.kind(), kind, "{result:?}");
            assert_eq!(error.operation(), Operation::Reflect);
        }
    }
}
