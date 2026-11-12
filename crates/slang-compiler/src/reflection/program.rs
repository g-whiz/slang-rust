use std::fmt;
use std::ptr::{self, NonNull};

use slang_compiler_sys::{
    ISlangBlob, SlangLayoutRules, SlangReflection, SlangReflectionEntryPoint,
    SlangReflectionFunction, SlangReflectionType, SlangReflectionVariableLayout, SlangUInt,
    TypeReflection, spReflection_FindTypeByName, spReflection_GetParameterByIndex,
    spReflection_GetParameterCount, spReflection_GetTypeLayout, spReflection_ToJson,
    spReflection_findEntryPointByName, spReflection_getEntryPointByIndex,
    spReflection_getEntryPointCount, spReflection_getGlobalConstantBufferBinding,
    spReflection_getGlobalConstantBufferSize, spReflection_getGlobalParamsVarLayout,
    spReflectionEntryPoint_getComputeThreadGroupSize, spReflectionEntryPoint_getFunction,
    spReflectionEntryPoint_getName, spReflectionEntryPoint_getNameOverride,
    spReflectionEntryPoint_getParameterByIndex, spReflectionEntryPoint_getParameterCount,
    spReflectionEntryPoint_getResultVarLayout, spReflectionEntryPoint_getStage,
    spReflectionEntryPoint_getVarLayout, spReflectionEntryPoint_hasDefaultConstantBuffer,
};

use super::{
    Function, LayoutSize, LayoutUnits, Positions, Type, TypeLayout, VariableLayout, binding_number,
    count, index,
};
use crate::blob::{self, BindingIndex};
use crate::component::other_session;
use crate::enums::{ParameterCategory, Stage};
use crate::error::{Diagnostics, Operation, Result};
use crate::guard;
use crate::mapping;
use crate::owned::Owned;
use crate::session::Session;
use crate::strings::{borrowed_str, c_string};

/// The layout of a component type for one target, the root from which every
/// other reflection handle is reached.
///
/// A layout says where each parameter of a program sits in memory and in the
/// bindings of a target, the numbered slots through which a shader reaches
/// its resources. Get one from the method `layout` of a component type, such
/// as [`LinkedProgram::layout`](crate::LinkedProgram::layout). Slang calls it
/// `ShaderReflection` or `ProgramLayout`. It borrows the component type that
/// it came from, so it cannot outlive it, and that component type keeps the
/// layout and its session alive.
///
/// # Examples
///
/// This example lists the parameters of each entry point of a program with
/// the kind of each parameter's type, through the variable layout of the
/// entry point, and unwraps the constant buffer that Slang can place around
/// them. A constant buffer is a buffer of uniform data, the parameters of
/// ordinary types that a target places in buffer memory:
///
/// ```
/// use slang_compiler::reflection::{ProgramLayout, TypeKind};
///
/// fn print_parameters(layout: ProgramLayout<'_>) -> slang_compiler::Result<()> {
///     for entry in layout.entry_point_layouts() {
///         let Some(scope) = entry.var_layout().and_then(|v| v.type_layout()) else {
///             continue;
///         };
///         let parameters = match scope.kind()? {
///             TypeKind::ConstantBuffer | TypeKind::ParameterBlock => scope
///                 .element_var_layout()
///                 .and_then(|v| v.type_layout())
///                 .unwrap_or(scope),
///             _ => scope,
///         };
///         for field in parameters.field_layouts() {
///             let name = match field.variable() {
///                 Some(variable) => variable.name()?.unwrap_or("<unnamed>"),
///                 None => "<unnamed>",
///             };
///             let kind = field.type_layout().map(|t| t.kind()).transpose()?;
///             println!("{name}: {kind:?}");
///         }
///     }
///     Ok(())
/// }
/// # let _ = print_parameters;
/// ```
///
/// A layout cannot outlive the program that it came from, so this example
/// does not compile:
///
/// ```compile_fail,E0505
/// use slang_compiler::{LinkedProgram, TargetIndex};
///
/// fn outlive(program: LinkedProgram, target: TargetIndex) -> slang_compiler::Result<()> {
///     let layout = program.layout(target)?.into_value();
///     drop(program);
///     let _ = layout.entry_point_layouts();
///     Ok(())
/// }
/// ```
///
/// A layout cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::ProgramLayout<'static>>();
/// ```
///
/// A layout cannot be shared between threads either, so this example does
/// not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::ProgramLayout<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct ProgramLayout<'a> {
    raw: NonNull<SlangReflection>,
    session: &'a Session,
}

impl<'a> ProgramLayout<'a> {
    /// Wraps the program layout that `getLayout` returned.
    ///
    /// # Safety
    ///
    /// `raw` points to the program layout that `getLayout` returned for a
    /// component type of `session` whose wrapper the caller borrows for
    /// `'a`, which keeps the layout alive. Breaking this is undefined
    /// behaviour.
    pub(crate) unsafe fn from_slang(raw: NonNull<SlangReflection>, session: &'a Session) -> Self {
        Self { raw, session }
    }

    /// Returns an iterator over the layouts of the entry points of the
    /// program, functions that a graphics API calls, in the order of the
    /// component type.
    ///
    /// # Examples
    ///
    /// See the example of [`ProgramLayout`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn entry_point_layouts(self) -> EntryPointLayouts<'a> {
        let count = guard::call("ProgramLayout::entry_point_layouts", || {
            // SAFETY: `self.raw` is a live program layout, which the wrapper
            // that `'a` borrows keeps alive, used only on this thread, and the
            // call runs with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("spReflection_getEntryPointCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflection_getEntryPointCount(self.raw.as_ptr())
                })
            }
        });
        EntryPointLayouts {
            program: self,
            positions: Positions::new(count),
        }
    }

    /// Returns the layout of the entry point named `name`, or `None` when
    /// the program has no entry point of that name.
    ///
    /// # Examples
    ///
    /// This example reads the stage of the entry point `main`, the kind of
    /// work that it does in a pipeline:
    ///
    /// ```
    /// use slang_compiler::Stage;
    /// use slang_compiler::reflection::ProgramLayout;
    ///
    /// fn main_stage(layout: ProgramLayout<'_>) -> slang_compiler::Result<Option<Stage>> {
    ///     Ok(match layout.find_entry_point_by_name("main")? {
    ///         Some(entry) => entry.stage()?,
    ///         None => None,
    ///     })
    /// }
    /// # let _ = main_stage;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument) when
    /// `name` contains a NUL byte, which Slang cannot take.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn find_entry_point_by_name(self, name: &str) -> Result<Option<EntryPointLayout<'a>>> {
        let name = c_string(name, Operation::Reflect, "the entry point name")?;
        let raw = guard::call("ProgramLayout::find_entry_point_by_name", || {
            // SAFETY: as for `entry_point_layouts`, and `name` is a C string
            // that lives until the call returns.
            unsafe {
                guard::abort_on_exception("spReflection_findEntryPointByName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflection_findEntryPointByName(self.raw.as_ptr(), name.as_ptr())
                })
            }
        });
        // SAFETY: Slang returned an entry point layout of this program
        // layout, which lives as long as it.
        Ok(NonNull::new(raw).map(|raw| unsafe { EntryPointLayout::new(raw, self.session) }))
    }

    /// Returns the variable layout of the global parameters of the program,
    /// the parameters declared outside every function, or `None` when Slang
    /// returns none.
    ///
    /// Slang recommends this over [`ProgramLayout::parameter_layouts`],
    /// because the type layout of this variable also shows the constant
    /// buffer that Slang places around global uniform data, if any.
    ///
    /// # Examples
    ///
    /// This example counts the global parameters:
    ///
    /// ```
    /// use slang_compiler::reflection::ProgramLayout;
    ///
    /// fn global_count(layout: ProgramLayout<'_>) -> usize {
    ///     layout
    ///         .global_params_var_layout()
    ///         .and_then(|v| v.type_layout())
    ///         .map_or(0, |t| t.field_layouts().count())
    /// }
    /// # let _ = global_count;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn global_params_var_layout(self) -> Option<VariableLayout<'a>> {
        let raw = guard::call("ProgramLayout::global_params_var_layout", || {
            // SAFETY: as for `entry_point_layouts`.
            unsafe {
                guard::abort_on_exception("spReflection_getGlobalParamsVarLayout", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflection_getGlobalParamsVarLayout(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: Slang returned a variable layout of this program layout.
        NonNull::new(raw).map(|raw| unsafe { VariableLayout::new(raw, self.session) })
    }

    /// Returns an iterator over the variable layouts of the global
    /// parameters of the program, one for each parameter declared outside
    /// every function.
    ///
    /// # Examples
    ///
    /// This example collects the names of the global parameters:
    ///
    /// ```
    /// use slang_compiler::reflection::ProgramLayout;
    ///
    /// fn names(layout: ProgramLayout<'_>) -> slang_compiler::Result<Vec<String>> {
    ///     let mut names = Vec::new();
    ///     for parameter in layout.parameter_layouts() {
    ///         if let Some(name) = parameter.variable().map(|v| v.name()).transpose()?.flatten() {
    ///             names.push(name.to_owned());
    ///         }
    ///     }
    ///     Ok(names)
    /// }
    /// # let _ = names;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn parameter_layouts(self) -> ParameterLayouts<'a> {
        let raw_count = guard::call("ProgramLayout::parameter_layouts", || {
            // SAFETY: as for `entry_point_layouts`.
            unsafe {
                guard::abort_on_exception("spReflection_GetParameterCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflection_GetParameterCount(self.raw.as_ptr())
                })
            }
        });
        ParameterLayouts {
            owner: ParameterOwner::Program(self),
            positions: Positions::new(count(raw_count)),
        }
    }

    /// Finds a type by its name as Slang source writes it, such as
    /// `MyStruct` or `Box<float>`, in the modules of the program, and
    /// returns `None` when Slang finds no such type.
    ///
    /// Use it to get the types that
    /// [`ComponentType::specialize`](crate::ComponentType::specialize) and
    /// [`Session::type_conformance`](crate::Session::type_conformance) take.
    /// Slang parses and checks the name as source, so a name that is not a
    /// type, or that Slang cannot parse, is `None`.
    ///
    /// Known upstream bug: the pinned Slang build crashes the process for
    /// some names, such as a name that contains `(` or a name nested
    /// thousands of levels deep. This is a defect of Slang, and this crate
    /// does not check for it.
    ///
    /// # Examples
    ///
    /// This example finds a type in the layout of a module and specializes a
    /// component type with it, filling its generic parameter, a type
    /// parameter of Slang code:
    ///
    /// ```
    /// use slang_compiler::{ComponentType, Module, SpecializationArg, TargetIndex};
    ///
    /// fn specialize_with(
    ///     module: &Module,
    ///     composed: &ComponentType,
    ///     target: TargetIndex,
    ///     name: &str,
    /// ) -> slang_compiler::Result<Option<ComponentType>> {
    ///     let layout = module.layout(target)?.into_value();
    ///     let Some(ty) = layout.find_type_by_name(name)? else {
    ///         return Ok(None);
    ///     };
    ///     Ok(Some(composed.specialize(&[SpecializationArg::Type(ty)])?.into_value()))
    /// }
    /// # let _ = specialize_with;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// without calling Slang, when `name` contains a NUL byte, which Slang
    /// cannot take.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn find_type_by_name(self, name: &str) -> Result<Option<Type<'a>>> {
        let name = c_string(name, Operation::Reflect, "the type name")?;
        let raw = guard::call("ProgramLayout::find_type_by_name", || {
            // SAFETY: as for `find_entry_point_by_name`.
            unsafe {
                guard::abort_on_exception("spReflection_FindTypeByName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflection_FindTypeByName(self.raw.as_ptr(), name.as_ptr())
                })
            }
        });
        Ok(NonNull::new(raw.cast::<TypeReflection>()).map(|raw| {
            // SAFETY: Slang returned a type of the session of this layout,
            // which lives in the syntax trees of that session, its modules,
            // or the core module, and `self.session` keeps all of them alive
            // for `'a`.
            unsafe { Type::from_raw(raw, self.session) }
        }))
    }

    /// Returns the layout of `ty` for the target of this layout, with
    /// Slang's default layout rules, or `None` when Slang returns none.
    ///
    /// Use it to lay out a type that is not a parameter of the program, such
    /// as the element type of a buffer that the program fills. Slang creates
    /// the layout in a cache of the session and keeps it there. For a type
    /// that nests structures or generic types 127 or more levels deep, past
    /// Slang's limit, Slang stops at the limit and reports the size of the
    /// type as [`LayoutSize::Unknown`].
    ///
    /// # Examples
    ///
    /// This example reads the size in bytes of a type found by its name:
    ///
    /// ```
    /// use slang_compiler::ParameterCategory;
    /// use slang_compiler::reflection::{LayoutSize, ProgramLayout};
    ///
    /// fn size_of(layout: ProgramLayout<'_>, name: &str) -> slang_compiler::Result<Option<LayoutSize>> {
    ///     let Some(ty) = layout.find_type_by_name(name)? else {
    ///         return Ok(None);
    ///     };
    ///     Ok(layout
    ///         .type_layout(ty)?
    ///         .map(|t| t.size(ParameterCategory::Uniform).amount))
    /// }
    /// # let _ = size_of;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// without calling Slang, when `ty` belongs to another session, because
    /// Slang would keep the type of the other session in the cache of this
    /// one.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn type_layout(self, ty: Type<'_>) -> Result<Option<TypeLayout<'a>>> {
        if !ty.session().is_same(self.session) {
            return Err(other_session(Operation::Reflect, "the type"));
        }
        let raw = guard::call("ProgramLayout::type_layout", || {
            // SAFETY: as for `entry_point_layouts`, and `ty` is a non-null
            // type of the session of this layout, which the check above
            // established and which its borrow keeps alive.
            unsafe {
                guard::abort_on_exception("spReflection_GetTypeLayout", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflection_GetTypeLayout(
                        self.raw.as_ptr(),
                        ty.as_raw().as_ptr().cast::<SlangReflectionType>(),
                        SlangLayoutRules::SLANG_LAYOUT_RULES_DEFAULT,
                    )
                })
            }
        });
        // SAFETY: Slang returned a type layout in the cache of the target of
        // this layout, which the session owns and keeps for `'a`.
        Ok(NonNull::new(raw).map(|raw| unsafe { TypeLayout::new(raw, self.session) }))
    }

    /// Returns the whole layout as JSON text, in the format that Slang's
    /// command-line compiler writes for reflection.
    ///
    /// The text describes every parameter, entry point, and type of the
    /// layout, and it is the same for the same source and target.
    ///
    /// Aborts the process, instead of returning an error, when the layout
    /// holds an array or a vector whose length a generic value parameter
    /// gives, such as `float values[M]` in the parameters of the generic
    /// entry point `main<let M : int>` of a component type that is not yet
    /// specialized. Slang then lets a C++ exception escape, and the crate
    /// writes one line to standard error that names the call
    /// `spReflection_ToJson`, and aborts before any destructor runs.
    ///
    /// # Examples
    ///
    /// This example writes the JSON text of a layout to a file:
    ///
    /// ```
    /// use std::path::Path;
    ///
    /// use slang_compiler::reflection::ProgramLayout;
    ///
    /// fn save(layout: ProgramLayout<'_>, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    ///     std::fs::write(path, layout.to_json()?)?;
    ///     Ok(())
    /// }
    /// # let _ = save;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the kind that Slang's result code maps to when Slang fails,
    /// an error of kind [`ErrorKind::Internal`](crate::ErrorKind::Internal)
    /// when Slang returns no text, and an error of kind
    /// [`ErrorKind::InvalidUtf8`](crate::ErrorKind::InvalidUtf8) when the
    /// text is not UTF-8.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn to_json(self) -> Result<String> {
        let operation = Operation::Reflect;
        guard::call("ProgramLayout::to_json", || {
            let mut out: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: as for `entry_point_layouts`, Slang accepts a null
            // compile request, and `out` is a live out-pointer.
            let result = unsafe {
                guard::abort_on_exception("spReflection_ToJson", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflection_ToJson(self.raw.as_ptr(), ptr::null_mut(), &raw mut out)
                })
            };
            // SAFETY: `spReflection_ToJson` returns a new blob reference or
            // null.
            let text = unsafe { Owned::adopt_output(out) };
            mapping::check(operation, result, Diagnostics::default())?;
            let text = text.ok_or_else(|| mapping::null(operation, Diagnostics::default()))?;
            blob::take_text(&text, operation, "the JSON text of the layout")
        })
    }

    /// Returns the binding number of the constant buffer that Slang creates
    /// for the global uniform data of the program, `Some(0)` when it creates
    /// none, and `None` when Slang does not know the number.
    ///
    /// Uniform data is the parameters of ordinary types, such as numbers and
    /// structures of them, which a target places in buffer memory. The
    /// binding is counted in the units of
    /// [`ParameterCategory::ConstantBuffer`].
    ///
    /// # Examples
    ///
    /// This example reads the binding and the size of the global constant
    /// buffer:
    ///
    /// ```
    /// use slang_compiler::BindingIndex;
    /// use slang_compiler::reflection::{LayoutSize, ProgramLayout};
    ///
    /// fn global_buffer(layout: ProgramLayout<'_>) -> (Option<BindingIndex>, LayoutSize) {
    ///     (
    ///         layout.global_constant_buffer_binding(),
    ///         layout.global_constant_buffer_size().amount,
    ///     )
    /// }
    /// # let _ = global_buffer;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn global_constant_buffer_binding(self) -> Option<BindingIndex> {
        let binding: SlangUInt = guard::call(
            "ProgramLayout::global_constant_buffer_binding",
            || {
                // SAFETY: as for `entry_point_layouts`.
                unsafe {
                    guard::abort_on_exception("spReflection_getGlobalConstantBufferBinding", || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                        )]
                        spReflection_getGlobalConstantBufferBinding(self.raw.as_ptr())
                    })
                }
            },
        );
        binding_number(binding).map(BindingIndex::new)
    }

    /// Returns the size of the global uniform data of the program, in bytes,
    /// the units of [`ParameterCategory::Uniform`].
    ///
    /// # Examples
    ///
    /// See the example of [`ProgramLayout::global_constant_buffer_binding`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn global_constant_buffer_size(self) -> LayoutUnits {
        let size = guard::call("ProgramLayout::global_constant_buffer_size", || {
            // SAFETY: as for `entry_point_layouts`.
            unsafe {
                guard::abort_on_exception("spReflection_getGlobalConstantBufferSize", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflection_getGlobalConstantBufferSize(self.raw.as_ptr())
                })
            }
        });
        LayoutUnits::new(ParameterCategory::Uniform, LayoutSize::from_raw(size))
    }

    /// Returns the pointer to the program layout, for use with the
    /// reflection functions of `slang-compiler-sys` that this crate does not
    /// wrap.
    ///
    /// The pointer is valid only while the component type that the layout
    /// borrows lives. Every use of it is unsafe and follows the safety
    /// contract of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example counts the type parameters of a program through the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::ProgramLayout;
    /// use slang_compiler_sys::spReflection_GetTypeParameterCount;
    ///
    /// fn type_parameter_count(layout: ProgramLayout<'_>) -> u32 {
    ///     // SAFETY: `layout` borrows its component type, which keeps the
    ///     // layout alive during the call, and only this thread uses it.
    ///     unsafe { spReflection_GetTypeParameterCount(layout.as_raw().as_ptr()) }
    /// }
    /// # let _ = type_parameter_count;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<SlangReflection> {
        self.raw
    }
}

impl fmt::Debug for ProgramLayout<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ProgramLayout")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

/// An iterator over the entry point layouts of a program, which
/// [`ProgramLayout::entry_point_layouts`] returns.
///
/// Each call of `next` asks Slang for the entry point at the next position.
/// A position at which Slang returns nothing yields no item, so the iterator
/// does not implement [`ExactSizeIterator`]. Each call of `next` panics when
/// it is called while another call of this crate into Slang is in progress
/// on the same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`ProgramLayout`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::EntryPointLayouts<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::EntryPointLayouts<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct EntryPointLayouts<'a> {
    program: ProgramLayout<'a>,
    positions: Positions,
}

impl<'a> EntryPointLayouts<'a> {
    fn at(&self, position: usize) -> Option<EntryPointLayout<'a>> {
        let raw = guard::call("EntryPointLayouts::next", || {
            // SAFETY: as for `ProgramLayout::entry_point_layouts`, and
            // `position` is below the count that Slang reported, which bounds
            // the index that `spReflection_getEntryPointByIndex` does not
            // check.
            unsafe {
                guard::abort_on_exception("spReflection_getEntryPointByIndex", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflection_getEntryPointByIndex(self.program.raw.as_ptr(), position)
                })
            }
        });
        // SAFETY: Slang returned an entry point layout of this program
        // layout.
        NonNull::new(raw).map(|raw| unsafe { EntryPointLayout::new(raw, self.program.session) })
    }
}

position_iterator!(EntryPointLayouts<'a>, EntryPointLayout<'a>);

/// The layout of one entry point of a program, a function that a graphics
/// API calls, like `main` in a program.
///
/// It describes the entry point's parameters and result. Get one from
/// [`ProgramLayout::entry_point_layouts`]. Slang calls it
/// `EntryPointReflection`. It borrows the component type of its program
/// layout, so it cannot outlive it.
///
/// # Examples
///
/// See the example of [`ProgramLayout`].
///
/// An entry point layout cannot outlive the program that it came from, so
/// this example does not compile:
///
/// ```compile_fail,E0505
/// use slang_compiler::{LinkedProgram, TargetIndex};
///
/// fn outlive(program: LinkedProgram, target: TargetIndex) -> slang_compiler::Result<()> {
///     let layout = program.layout(target)?.into_value();
///     let entry = layout.entry_point_layouts().next();
///     drop(program);
///     let _ = entry.map(|entry| entry.name());
///     Ok(())
/// }
/// ```
///
/// An entry point layout cannot move to another thread, so this example does
/// not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::EntryPointLayout<'static>>();
/// ```
///
/// An entry point layout cannot be shared between threads either, so this
/// example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::EntryPointLayout<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct EntryPointLayout<'a> {
    raw: NonNull<SlangReflectionEntryPoint>,
    session: &'a Session,
}

impl<'a> EntryPointLayout<'a> {
    /// Wraps an entry point layout that a reflection function returned.
    ///
    /// # Safety
    ///
    /// `raw` points to an entry point layout of a program layout that a
    /// handle of lifetime `'a` of `session` reached, and so lives for `'a`.
    /// Breaking this is undefined behaviour.
    unsafe fn new(raw: NonNull<SlangReflectionEntryPoint>, session: &'a Session) -> Self {
        Self { raw, session }
    }

    /// Returns the name of the entry point's function, or `None` when Slang
    /// returns none.
    ///
    /// # Examples
    ///
    /// This example prints the name and the stage of each entry point:
    ///
    /// ```
    /// use slang_compiler::reflection::ProgramLayout;
    ///
    /// fn print_entry_points(layout: ProgramLayout<'_>) -> slang_compiler::Result<()> {
    ///     for entry in layout.entry_point_layouts() {
    ///         println!("{:?}: {:?}", entry.name()?, entry.stage()?);
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = print_entry_points;
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
        let raw = guard::call("EntryPointLayout::name", || {
            // SAFETY: `self.raw` is a live entry point layout, which the
            // wrapper that `'a` borrows keeps alive, used only on this
            // thread, and the call runs with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("spReflectionEntryPoint_getName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionEntryPoint_getName(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: the name is in the name pool of the global session, which
        // the session keeps alive for `'a`.
        unsafe { borrowed_str(raw, Operation::Reflect, "the name of an entry point") }
    }

    /// Returns the name under which the entry point appears in generated
    /// code, which is its name unless the component type renamed it, or
    /// `None` when Slang returns none.
    ///
    /// # Examples
    ///
    /// See the example of [`EntryPointLayout::name`].
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
    pub fn name_override(self) -> Result<Option<&'a str>> {
        let raw = guard::call("EntryPointLayout::name_override", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionEntryPoint_getNameOverride", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionEntryPoint_getNameOverride(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: the name is in the entry point layout or the name pool,
        // which live for `'a`.
        unsafe {
            borrowed_str(
                raw,
                Operation::Reflect,
                "the name override of an entry point",
            )
        }
    }

    /// Returns the stage of the entry point, the kind of work that it does
    /// in a pipeline, such as [`Stage::Compute`], or `None` when Slang
    /// reports none.
    ///
    /// # Examples
    ///
    /// See the example of [`EntryPointLayout::name`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when Slang
    /// reports a stage that the pinned Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn stage(self) -> Result<Option<Stage>> {
        let raw = guard::call("EntryPointLayout::stage", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionEntryPoint_getStage", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionEntryPoint_getStage(self.raw.as_ptr())
                })
            }
        });
        Stage::from_raw(raw)
    }

    /// Returns the size of a thread group of a compute shader along each of
    /// its three axes, as the attribute `numthreads` sets it.
    ///
    /// A compute shader runs as a general computation in groups of threads
    /// that run together. A size is 0 when it depends on a value that is not
    /// known yet, such as a specialization constant, and each size is 1 for
    /// an entry point that sets no group size.
    ///
    /// # Examples
    ///
    /// This example counts the threads of one group:
    ///
    /// ```
    /// use slang_compiler::reflection::EntryPointLayout;
    ///
    /// fn threads(entry: EntryPointLayout<'_>) -> Option<usize> {
    ///     let [x, y, z] = entry.compute_thread_group_size();
    ///     x.checked_mul(y)?.checked_mul(z)
    /// }
    /// # let _ = threads;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn compute_thread_group_size(self) -> [usize; 3] {
        let mut sizes: [SlangUInt; 3] = [1, 1, 1];
        guard::call("EntryPointLayout::compute_thread_group_size", || {
            // SAFETY: as for `name`, and `sizes` holds the three elements
            // that the call writes for an axis count of 3.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionEntryPoint_getComputeThreadGroupSize",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                        )]
                        spReflectionEntryPoint_getComputeThreadGroupSize(
                            self.raw.as_ptr(),
                            3,
                            sizes.as_mut_ptr(),
                        );
                    },
                );
            }
        });
        sizes
    }

    /// Returns the variable layout of the parameters of the entry point,
    /// taken together, or `None` when Slang returns none.
    ///
    /// Its type layout is a structure with one field for each parameter, or
    /// a constant buffer around such a structure when Slang places the
    /// entry point's uniform parameters in one. Slang recommends this over
    /// [`EntryPointLayout::parameter_layouts`].
    ///
    /// # Examples
    ///
    /// See the example of [`ProgramLayout`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn var_layout(self) -> Option<VariableLayout<'a>> {
        let raw = guard::call("EntryPointLayout::var_layout", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionEntryPoint_getVarLayout", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionEntryPoint_getVarLayout(self.raw.as_ptr())
                })
            }
        });
        self.variable_layout(raw)
    }

    /// Returns the variable layout of the result of the entry point, or
    /// `None` when Slang returns none, such as for a compute shader, whose
    /// result is `void`.
    ///
    /// # Examples
    ///
    /// This example reads the semantic of the result of an entry point, the
    /// name that marks what a stage outputs, such as `SV_Target`:
    ///
    /// ```
    /// use slang_compiler::reflection::EntryPointLayout;
    ///
    /// fn result_semantic<'a>(entry: EntryPointLayout<'a>) -> slang_compiler::Result<Option<&'a str>> {
    ///     match entry.result_var_layout() {
    ///         Some(result) => result.semantic_name(),
    ///         None => Ok(None),
    ///     }
    /// }
    /// # let _ = result_semantic;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn result_var_layout(self) -> Option<VariableLayout<'a>> {
        let raw = guard::call("EntryPointLayout::result_var_layout", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionEntryPoint_getResultVarLayout", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionEntryPoint_getResultVarLayout(self.raw.as_ptr())
                })
            }
        });
        self.variable_layout(raw)
    }

    fn variable_layout(
        self,
        raw: *mut SlangReflectionVariableLayout,
    ) -> Option<VariableLayout<'a>> {
        // SAFETY: the caller passes a variable layout that Slang returned for
        // this entry point layout, which lives as long as it.
        NonNull::new(raw).map(|raw| unsafe { VariableLayout::new(raw, self.session) })
    }

    /// Returns an iterator over the variable layouts of the parameters of
    /// the entry point, one for each parameter of its function.
    ///
    /// # Examples
    ///
    /// This example counts the parameters of an entry point:
    ///
    /// ```
    /// use slang_compiler::reflection::EntryPointLayout;
    ///
    /// fn parameter_count(entry: EntryPointLayout<'_>) -> usize {
    ///     entry.parameter_layouts().count()
    /// }
    /// # let _ = parameter_count;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn parameter_layouts(self) -> ParameterLayouts<'a> {
        let raw_count = guard::call("EntryPointLayout::parameter_layouts", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionEntryPoint_getParameterCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionEntryPoint_getParameterCount(self.raw.as_ptr())
                })
            }
        });
        ParameterLayouts {
            owner: ParameterOwner::EntryPoint(self),
            positions: Positions::new(count(raw_count)),
        }
    }

    /// Returns the function of the entry point, which describes its
    /// declaration in the source, or `None` when Slang returns none.
    ///
    /// # Examples
    ///
    /// This example reads the name of the function of an entry point:
    ///
    /// ```
    /// use slang_compiler::reflection::EntryPointLayout;
    ///
    /// fn function_name<'a>(entry: EntryPointLayout<'a>) -> slang_compiler::Result<Option<&'a str>> {
    ///     match entry.function() {
    ///         Some(function) => function.name(),
    ///         None => Ok(None),
    ///     }
    /// }
    /// # let _ = function_name;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn function(self) -> Option<Function<'a>> {
        let raw = guard::call("EntryPointLayout::function", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionEntryPoint_getFunction", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionEntryPoint_getFunction(self.raw.as_ptr())
                })
            }
        });
        // SAFETY: Slang returned the function of the entry point, which lives
        // in the syntax trees of the session for `'a`.
        NonNull::new(raw).map(|raw: NonNull<SlangReflectionFunction>| unsafe {
            Function::new(raw, self.session)
        })
    }

    /// Returns whether Slang placed the uniform parameters of the entry
    /// point in a constant buffer of their own, a buffer of uniform data
    /// that the program binds for this entry point.
    ///
    /// # Examples
    ///
    /// This example tells whether an entry point needs a buffer of its own
    /// for its uniform parameters:
    ///
    /// ```
    /// use slang_compiler::reflection::EntryPointLayout;
    ///
    /// fn needs_buffer(entry: EntryPointLayout<'_>) -> bool {
    ///     entry.has_default_constant_buffer()
    /// }
    /// # let _ = needs_buffer;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn has_default_constant_buffer(self) -> bool {
        let value = guard::call("EntryPointLayout::has_default_constant_buffer", || {
            // SAFETY: as for `name`.
            unsafe {
                guard::abort_on_exception("spReflectionEntryPoint_hasDefaultConstantBuffer", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionEntryPoint_hasDefaultConstantBuffer(self.raw.as_ptr())
                })
            }
        });
        value != 0
    }

    /// Returns the pointer to the entry point layout, for use with the
    /// reflection functions of `slang-compiler-sys` that this crate does not
    /// wrap.
    ///
    /// The pointer is valid only while the component type that the layout
    /// borrows lives. Every use of it is unsafe and follows the safety
    /// contract of `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example reads the wave size of an entry point through the raw
    /// bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::EntryPointLayout;
    /// use slang_compiler_sys::spReflectionEntryPoint_getComputeWaveSize;
    ///
    /// fn wave_size(entry: EntryPointLayout<'_>) -> usize {
    ///     let mut size = 0;
    ///     // SAFETY: `entry` keeps the layout alive during the call, `size` is
    ///     // a live out-pointer, and only this thread uses the layout.
    ///     unsafe { spReflectionEntryPoint_getComputeWaveSize(entry.as_raw().as_ptr(), &raw mut size) };
    ///     size
    /// }
    /// # let _ = wave_size;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<SlangReflectionEntryPoint> {
        self.raw
    }
}

impl fmt::Debug for EntryPointLayout<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EntryPointLayout")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

/// What a [`ParameterLayouts`] lists the parameters of.
#[derive(Clone, Copy, Debug)]
enum ParameterOwner<'a> {
    Program(ProgramLayout<'a>),
    EntryPoint(EntryPointLayout<'a>),
}

/// An iterator over the variable layouts of the parameters of a program or
/// an entry point, which [`ProgramLayout::parameter_layouts`] and
/// [`EntryPointLayout::parameter_layouts`] return.
///
/// Each call of `next` asks Slang for the parameter at the next position. A
/// position at which Slang returns nothing yields no item, so the iterator
/// does not implement [`ExactSizeIterator`]. Each call of `next` panics when
/// it is called while another call of this crate into Slang is in progress
/// on the same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`ProgramLayout::parameter_layouts`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::ParameterLayouts<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::ParameterLayouts<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct ParameterLayouts<'a> {
    owner: ParameterOwner<'a>,
    positions: Positions,
}

impl<'a> ParameterLayouts<'a> {
    fn at(&self, position: usize) -> Option<VariableLayout<'a>> {
        let index = index(position)?;
        let (raw, session) = match self.owner {
            ParameterOwner::Program(program) => (
                guard::call("ParameterLayouts::next", || {
                    // SAFETY: as for `ProgramLayout::parameter_layouts`, and
                    // `index` is below the count that Slang reported, which
                    // bounds the index that `spReflection_GetParameterByIndex`
                    // does not check.
                    unsafe {
                        guard::abort_on_exception("spReflection_GetParameterByIndex", || {
                            #[expect(
                                ffi_unwind_calls,
                                reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                            )]
                            spReflection_GetParameterByIndex(program.raw.as_ptr(), index)
                        })
                    }
                }),
                program.session,
            ),
            ParameterOwner::EntryPoint(entry) => (
                guard::call("ParameterLayouts::next", || {
                    // SAFETY: as for `EntryPointLayout::parameter_layouts`,
                    // and `index` is below the count that Slang reported,
                    // which bounds the index that
                    // `spReflectionEntryPoint_getParameterByIndex` does not
                    // check.
                    unsafe {
                        guard::abort_on_exception(
                            "spReflectionEntryPoint_getParameterByIndex",
                            || {
                                #[expect(
                                    ffi_unwind_calls,
                                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                                )]
                                spReflectionEntryPoint_getParameterByIndex(
                                    entry.raw.as_ptr(),
                                    index,
                                )
                            },
                        )
                    }
                }),
                entry.session,
            ),
        };
        // SAFETY: Slang returned a variable layout of the program or entry
        // point layout, which lives as long as it.
        NonNull::new(raw).map(|raw| unsafe { VariableLayout::new(raw, session) })
    }
}

position_iterator!(ParameterLayouts<'a>, VariableLayout<'a>);
