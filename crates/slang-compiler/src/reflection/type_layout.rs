use std::fmt;
use std::ptr::{self, NonNull};

use slang_compiler_sys::{
    SlangReflectionTypeLayout, spReflectionTypeLayout_GetCategoryByIndex,
    spReflectionTypeLayout_GetCategoryCount, spReflectionTypeLayout_GetElementStride,
    spReflectionTypeLayout_GetElementTypeLayout, spReflectionTypeLayout_GetElementVarLayout,
    spReflectionTypeLayout_GetFieldByIndex, spReflectionTypeLayout_GetFieldCount,
    spReflectionTypeLayout_GetMatrixLayoutMode, spReflectionTypeLayout_GetParameterCategory,
    spReflectionTypeLayout_GetSize, spReflectionTypeLayout_GetStride,
    spReflectionTypeLayout_GetType, spReflectionTypeLayout_findFieldIndexByName,
    spReflectionTypeLayout_getAlignment, spReflectionTypeLayout_getBindingRangeCount,
    spReflectionTypeLayout_getContainerVarLayout, spReflectionTypeLayout_getDescriptorSetCount,
    spReflectionTypeLayout_getExplicitCounterBindingRangeOffset,
    spReflectionTypeLayout_getFieldBindingRangeOffset, spReflectionTypeLayout_getKind,
};

use super::{
    BindingRanges, DescriptorSets, LayoutSize, LayoutUnits, Positions, Type, TypeKind,
    VariableLayout, count, index, out_of_range, signed, signed_index,
};
use crate::enums::{MatrixLayout, ParameterCategory};
use crate::error::{Error, ErrorKind, Operation, Result};
use crate::guard;
use crate::session::Session;
use crate::strings::c_string;

/// The layout of one type for one use: its size in each parameter category
/// and the layouts of its fields, elements, and bindings.
///
/// A parameter category is a kind of resource that a layout counts
/// separately, such as bytes of uniform data or the descriptor slots of
/// Vulkan, the numbered bindings through which a shader reaches its
/// resources, so every size of a type layout is a [`LayoutUnits`] with its
/// category. Get a type layout from [`VariableLayout::type_layout`] or
/// [`ProgramLayout::type_layout`](super::ProgramLayout::type_layout). Slang
/// calls it `TypeLayoutReflection`. It borrows the object that it came from,
/// so it cannot outlive it.
///
/// # Examples
///
/// This example prints the fields of a structure with their kind, byte
/// offset, and size in bytes of uniform data:
///
/// ```
/// use slang_compiler::ParameterCategory;
/// use slang_compiler::reflection::{LayoutSize, TypeKind, TypeLayout};
///
/// fn print_fields(layout: TypeLayout<'_>) -> slang_compiler::Result<()> {
///     if layout.kind()? != TypeKind::Struct {
///         return Ok(());
///     }
///     for field in layout.field_layouts() {
///         let name = field.variable().map(|v| v.name()).transpose()?.flatten();
///         let offset = field.offset(ParameterCategory::Uniform).amount;
///         let size = field
///             .type_layout()
///             .map_or(LayoutSize::Finite(0), |t| t.size(ParameterCategory::Uniform).amount);
///         println!("{name:?}: offset {offset:?}, size {size:?}");
///     }
///     Ok(())
/// }
/// # let _ = print_fields;
/// ```
///
/// A type layout cannot outlive the program that it came from, so this
/// example does not compile:
///
/// ```compile_fail,E0505
/// use slang_compiler::{LinkedProgram, TargetIndex};
///
/// fn outlive(program: LinkedProgram, target: TargetIndex) -> slang_compiler::Result<()> {
///     let layout = program.layout(target)?.into_value();
///     let globals = layout.global_params_var_layout().and_then(|v| v.type_layout());
///     drop(program);
///     let _ = globals.map(|globals| globals.kind());
///     Ok(())
/// }
/// ```
///
/// A type layout cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::TypeLayout<'static>>();
/// ```
///
/// A type layout cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::TypeLayout<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct TypeLayout<'a> {
    raw: NonNull<SlangReflectionTypeLayout>,
    session: &'a Session,
}

impl<'a> TypeLayout<'a> {
    /// Wraps a type layout that a reflection function returned.
    ///
    /// # Safety
    ///
    /// `raw` points to a type layout of a program layout, or of the cache of
    /// a target of `session`, which a handle of lifetime `'a` of `session`
    /// reached, and so lives for `'a`. Breaking this is undefined behaviour.
    pub(super) unsafe fn new(
        raw: NonNull<SlangReflectionTypeLayout>,
        session: &'a Session,
    ) -> Self {
        Self { raw, session }
    }

    /// Wraps a type layout that a reflection function returned, or returns
    /// `None` for null.
    ///
    /// # Safety
    ///
    /// As for [`TypeLayout::new`], when `raw` is not null.
    pub(super) unsafe fn from_output(
        raw: *mut SlangReflectionTypeLayout,
        session: &'a Session,
    ) -> Option<Self> {
        // SAFETY: the caller guarantees the conditions of `new`.
        NonNull::new(raw).map(|raw| unsafe { Self::new(raw, session) })
    }

    pub(super) fn raw(self) -> *mut SlangReflectionTypeLayout {
        self.raw.as_ptr()
    }

    pub(super) fn session(self) -> &'a Session {
        self.session
    }

    /// Returns the type that this layout lays out, or `None` when Slang
    /// returns none, such as for the layout of the parameters of a scope.
    ///
    /// # Examples
    ///
    /// This example reads the full name of the type of a layout:
    ///
    /// ```
    /// use slang_compiler::reflection::TypeLayout;
    ///
    /// fn type_name(layout: TypeLayout<'_>) -> slang_compiler::Result<Option<String>> {
    ///     layout.ty().map(|ty| ty.full_name()).transpose()
    /// }
    /// # let _ = type_name;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn ty(self) -> Option<Type<'a>> {
        let raw = guard::call("TypeLayout::ty", || {
            // SAFETY: `self.raw` is a live type layout, which the wrapper that
            // `'a` borrows keeps alive, used only on this thread, and the call
            // runs with the Slang call guard raised.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetType", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetType(self.raw())
                })
            }
        });
        // SAFETY: Slang returned a type of the same session.
        unsafe { Type::from_output(raw, self.session) }
    }

    /// Returns the kind of the type of this layout, such as a structure or
    /// a constant buffer.
    ///
    /// # Examples
    ///
    /// See the example of [`TypeLayout`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Unrecognized`] when Slang reports a kind that the
    /// pinned Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn kind(self) -> Result<TypeKind> {
        let raw = guard::call("TypeLayout::kind", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_getKind", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_getKind(self.raw())
                })
            }
        });
        TypeKind::from_raw(raw)
    }

    /// Returns the size of the type in the units of `category`, such as its
    /// size in bytes of uniform data, or the number of descriptor slots of
    /// Vulkan that it uses.
    ///
    /// The size is `Finite(0)` when the type uses nothing of `category`,
    /// and [`LayoutSize::Unbounded`] for an array whose length the source
    /// does not declare.
    ///
    /// # Examples
    ///
    /// See the example of [`TypeLayout`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn size(self, category: ParameterCategory) -> LayoutUnits {
        let raw = guard::call("TypeLayout::size", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetSize", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetSize(self.raw(), category.to_raw())
                })
            }
        });
        LayoutUnits::new(category, LayoutSize::from_raw(raw))
    }

    /// Returns the stride of the type in the units of `category`, its size
    /// rounded up to its alignment, which is the distance between two
    /// elements of an array of the type.
    ///
    /// # Examples
    ///
    /// This example reads the stride in bytes of the elements of a buffer
    /// whose element type is laid out by `element`:
    ///
    /// ```
    /// use slang_compiler::ParameterCategory;
    /// use slang_compiler::reflection::{LayoutSize, TypeLayout};
    ///
    /// fn stride(element: TypeLayout<'_>) -> LayoutSize {
    ///     element.stride(ParameterCategory::Uniform).amount
    /// }
    /// # let _ = stride;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn stride(self, category: ParameterCategory) -> LayoutUnits {
        let raw = guard::call("TypeLayout::stride", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetStride", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetStride(self.raw(), category.to_raw())
                })
            }
        });
        LayoutUnits::new(category, LayoutSize::from_raw(raw))
    }

    /// Returns the alignment of the type in the units of `category`, which
    /// Slang reports for uniform data in bytes and as 1 for every other
    /// category.
    ///
    /// # Examples
    ///
    /// This example reads the alignment in bytes of a type:
    ///
    /// ```
    /// use slang_compiler::ParameterCategory;
    /// use slang_compiler::reflection::{LayoutSize, TypeLayout};
    ///
    /// fn alignment(layout: TypeLayout<'_>) -> slang_compiler::Result<LayoutSize> {
    ///     Ok(layout.alignment(ParameterCategory::Uniform)?.amount)
    /// }
    /// # let _ = alignment;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Internal`] when Slang reports
    /// a negative alignment.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn alignment(self, category: ParameterCategory) -> Result<LayoutUnits> {
        let raw = guard::call("TypeLayout::alignment", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_getAlignment", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_getAlignment(self.raw(), category.to_raw())
                })
            }
        });
        let alignment = u64::try_from(raw).map_err(|error| {
            Error::new(ErrorKind::Internal, Operation::Reflect)
                .with_detail(format!("Slang reported the alignment {raw}: {error}"))
        })?;
        Ok(LayoutUnits::new(category, LayoutSize::Finite(alignment)))
    }

    /// Returns an iterator over the variable layouts of the fields of a
    /// structure, which is empty for a layout of any other kind.
    ///
    /// # Examples
    ///
    /// See the example of [`TypeLayout`].
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn field_layouts(self) -> FieldLayouts<'a> {
        FieldLayouts {
            layout: self,
            positions: Positions::new(self.field_count()),
        }
    }

    fn field_count(self) -> usize {
        let raw = guard::call("TypeLayout::field_layouts", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetFieldCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetFieldCount(self.raw())
                })
            }
        });
        count(raw)
    }

    /// Returns the position among [`TypeLayout::field_layouts`] of the field
    /// named `name`, or `None` when the layout is not a structure or has no
    /// field of that name.
    ///
    /// The name can name the field through the declarations that contain it,
    /// with `.` or `::` between the parts, such as `Outer::field`.
    ///
    /// # Examples
    ///
    /// This example reads the byte offset of the field `color`:
    ///
    /// ```
    /// use slang_compiler::ParameterCategory;
    /// use slang_compiler::reflection::{LayoutSize, TypeLayout};
    ///
    /// fn color_offset(layout: TypeLayout<'_>) -> slang_compiler::Result<Option<LayoutSize>> {
    ///     let Some(position) = layout.find_field_index_by_name("color")? else {
    ///         return Ok(None);
    ///     };
    ///     Ok(layout
    ///         .field_layouts()
    ///         .nth(position)
    ///         .map(|field| field.offset(ParameterCategory::Uniform).amount))
    /// }
    /// # let _ = color_offset;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `name`
    /// contains a NUL byte, which Slang cannot take, and an error of kind
    /// [`ErrorKind::Internal`] when Slang reports a negative position other
    /// than the -1 of a name that it does not find.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn find_field_index_by_name(self, name: &str) -> Result<Option<usize>> {
        let name = c_string(name, Operation::Reflect, "the field name")?;
        let raw = guard::call("TypeLayout::find_field_index_by_name", || {
            // SAFETY: as for `ty`, and `name` is a C string that lives until
            // the call returns, which Slang reads to its NUL byte because the
            // end pointer is null.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_findFieldIndexByName", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_findFieldIndexByName(
                        self.raw(),
                        name.as_ptr(),
                        ptr::null(),
                    )
                })
            }
        });
        if raw == -1 {
            return Ok(None);
        }
        signed(raw, "the field position").map(Some)
    }

    /// Returns the layout of the elements of an array, vector, matrix,
    /// constant buffer, parameter block, structured buffer, pointer, or
    /// output stream, or `None` for a layout of any other kind.
    ///
    /// For a constant buffer or a parameter block, the element layout has the
    /// offsets of the contents relative to the start of the buffer.
    ///
    /// # Examples
    ///
    /// This example reads the size in bytes of the elements of an array or a
    /// buffer:
    ///
    /// ```
    /// use slang_compiler::ParameterCategory;
    /// use slang_compiler::reflection::{LayoutSize, TypeLayout};
    ///
    /// fn element_size(layout: TypeLayout<'_>) -> Option<LayoutSize> {
    ///     layout
    ///         .element_type_layout()
    ///         .map(|element| element.size(ParameterCategory::Uniform).amount)
    /// }
    /// # let _ = element_size;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn element_type_layout(self) -> Option<TypeLayout<'a>> {
        let raw = guard::call("TypeLayout::element_type_layout", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetElementTypeLayout", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetElementTypeLayout(self.raw())
                })
            }
        });
        // SAFETY: Slang returned a type layout that this type layout holds.
        unsafe { Self::from_output(raw, self.session) }
    }

    /// Returns the distance between two elements of an array or a vector in
    /// the units of `category`, or `Finite(0)` for a layout of any other
    /// kind.
    ///
    /// # Examples
    ///
    /// This example reads the stride in bytes of the elements of an array:
    ///
    /// ```
    /// use slang_compiler::ParameterCategory;
    /// use slang_compiler::reflection::{LayoutSize, TypeLayout};
    ///
    /// fn array_stride(layout: TypeLayout<'_>) -> LayoutSize {
    ///     layout.element_stride(ParameterCategory::Uniform).amount
    /// }
    /// # let _ = array_stride;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn element_stride(self, category: ParameterCategory) -> LayoutUnits {
        let raw = guard::call("TypeLayout::element_stride", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetElementStride", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetElementStride(self.raw(), category.to_raw())
                })
            }
        });
        LayoutUnits::new(category, LayoutSize::from_raw(raw))
    }

    /// Returns the variable layout of the contents of a constant buffer or a
    /// parameter block, or `None` for a layout of any other kind.
    ///
    /// A parameter block is a group of parameters that a target binds as one
    /// unit, such as one descriptor set of Vulkan. The contents are laid out
    /// relative to the buffer or block.
    ///
    /// # Examples
    ///
    /// See the example of [`ProgramLayout`](super::ProgramLayout).
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn element_var_layout(self) -> Option<VariableLayout<'a>> {
        let raw = guard::call("TypeLayout::element_var_layout", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetElementVarLayout", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetElementVarLayout(self.raw())
                })
            }
        });
        // SAFETY: Slang returned a variable layout that this type layout
        // holds.
        unsafe { VariableLayout::from_output(raw, self.session) }
    }

    /// Returns the variable layout of the container of a constant buffer or
    /// a parameter block, the binding of the buffer or block itself, or
    /// `None` for a layout of any other kind.
    ///
    /// # Examples
    ///
    /// This example reads the binding of the buffer of a constant buffer
    /// among the descriptor slots of Vulkan:
    ///
    /// ```
    /// use slang_compiler::ParameterCategory;
    /// use slang_compiler::reflection::{LayoutSize, TypeLayout};
    ///
    /// fn buffer_binding(layout: TypeLayout<'_>) -> Option<LayoutSize> {
    ///     layout
    ///         .container_var_layout()
    ///         .map(|container| container.offset(ParameterCategory::DescriptorTableSlot).amount)
    /// }
    /// # let _ = buffer_binding;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn container_var_layout(self) -> Option<VariableLayout<'a>> {
        let raw = guard::call("TypeLayout::container_var_layout", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_getContainerVarLayout", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_getContainerVarLayout(self.raw())
                })
            }
        });
        // SAFETY: Slang returned a variable layout that this type layout
        // holds.
        unsafe { VariableLayout::from_output(raw, self.session) }
    }

    /// Returns the one parameter category that the type uses,
    /// [`ParameterCategory::Mixed`] when it uses several, or `None` when it
    /// uses none, such as an empty structure.
    ///
    /// # Examples
    ///
    /// See the example of [`VariableLayout`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Unrecognized`] when Slang
    /// reports a category that the pinned Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn parameter_category(self) -> Result<Option<ParameterCategory>> {
        let raw = guard::call("TypeLayout::parameter_category", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetParameterCategory", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetParameterCategory(self.raw())
                })
            }
        });
        ParameterCategory::from_raw(raw)
    }

    /// Returns an iterator over every parameter category that the type uses,
    /// such as uniform data and descriptor slots for a structure that holds
    /// numbers and a texture.
    ///
    /// # Examples
    ///
    /// This example prints the size of a type in each category that it uses:
    ///
    /// ```
    /// use slang_compiler::reflection::TypeLayout;
    ///
    /// fn print_sizes(layout: TypeLayout<'_>) -> slang_compiler::Result<()> {
    ///     for category in layout.categories() {
    ///         let category = category?;
    ///         println!("{category:?}: {:?}", layout.size(category).amount);
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = print_sizes;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn categories(self) -> Categories<'a> {
        let raw = guard::call("TypeLayout::categories", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetCategoryCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetCategoryCount(self.raw())
                })
            }
        });
        Categories {
            layout: self,
            positions: Positions::new(count(raw)),
        }
    }

    /// Returns the matrix layout of a matrix type, whether its rows or its
    /// columns are adjacent in memory, or `None` for a layout of any other
    /// kind.
    ///
    /// # Examples
    ///
    /// This example tells whether a matrix is stored row by row:
    ///
    /// ```
    /// use slang_compiler::MatrixLayout;
    /// use slang_compiler::reflection::TypeLayout;
    ///
    /// fn is_row_major(layout: TypeLayout<'_>) -> slang_compiler::Result<bool> {
    ///     Ok(layout.matrix_layout_mode()? == Some(MatrixLayout::RowMajor))
    /// }
    /// # let _ = is_row_major;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Unrecognized`] when Slang
    /// reports a matrix layout that the pinned Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn matrix_layout_mode(self) -> Result<Option<MatrixLayout>> {
        let raw = guard::call("TypeLayout::matrix_layout_mode", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetMatrixLayoutMode", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetMatrixLayoutMode(self.raw())
                })
            }
        });
        MatrixLayout::from_raw(raw)
    }

    /// Returns an iterator over the binding ranges of the type, the runs of
    /// consecutive bindings of one kind that it uses, such as the bindings
    /// of an array of textures.
    ///
    /// Binding ranges flatten the bindings of nested structures and arrays
    /// into one list, which is how a program fills the bindings of a type
    /// without walking its fields.
    ///
    /// Aborts the process, instead of returning an error, when this is the
    /// first call that needs the binding ranges of a layout that holds an
    /// array whose length a generic value parameter gives, such as
    /// `float values[M]` in the parameters of the generic entry point
    /// `main<let M : int>` of a component type that is not yet specialized.
    /// Slang computes the binding ranges of a layout once, lets a C++
    /// exception escape for such an array, and the crate then writes one
    /// line to standard error that names the call `spReflectionTypeLayout_getBindingRangeCount`, and aborts
    /// before any destructor runs.
    ///
    /// # Examples
    ///
    /// See the example of [`BindingRange`](super::BindingRange).
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
    pub fn binding_ranges(self) -> Result<BindingRanges<'a>> {
        let raw = guard::call("TypeLayout::binding_ranges", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_getBindingRangeCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_getBindingRangeCount(self.raw())
                })
            }
        });
        Ok(BindingRanges::new(
            self,
            signed(raw, "the binding range count")?,
        ))
    }

    /// Returns the position among [`TypeLayout::binding_ranges`] of the
    /// first binding range of the field at `field`, a position among
    /// [`TypeLayout::field_layouts`].
    ///
    /// Aborts the process, instead of returning an error, when this is the
    /// first call that needs the binding ranges of a layout that holds an
    /// array whose length a generic value parameter gives, such as
    /// `float values[M]` in the parameters of the generic entry point
    /// `main<let M : int>` of a component type that is not yet specialized.
    /// Slang computes the binding ranges of a layout once, lets a C++
    /// exception escape for such an array, and the crate then writes one
    /// line to standard error that names the call `spReflectionTypeLayout_getFieldBindingRangeOffset`, and aborts
    /// before any destructor runs.
    ///
    /// # Examples
    ///
    /// This example finds the first binding range of each field:
    ///
    /// ```
    /// use slang_compiler::reflection::TypeLayout;
    ///
    /// fn first_ranges(layout: TypeLayout<'_>) -> slang_compiler::Result<Vec<usize>> {
    ///     (0..layout.field_layouts().count())
    ///         .map(|field| layout.field_binding_range_offset(field))
    ///         .collect()
    /// }
    /// # let _ = first_ranges;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::InvalidArgument`] when `field`
    /// is not below the number of fields, which the crate checks before it
    /// calls Slang, because Slang does not check it, and an error of kind
    /// [`ErrorKind::Internal`] when Slang reports a negative position.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn field_binding_range_offset(self, field: usize) -> Result<usize> {
        let field_count = self.field_count();
        let index = signed_index(field)
            .filter(|_| field < field_count)
            .ok_or_else(|| out_of_range("field", field, field_count))?;
        let raw = guard::call("TypeLayout::field_binding_range_offset", || {
            // SAFETY: as for `ty`, and `index` is below the field count that
            // Slang reported, which bounds the index that
            // `spReflectionTypeLayout_getFieldBindingRangeOffset` does not
            // check.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getFieldBindingRangeOffset",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getFieldBindingRangeOffset(self.raw(), index)
                    },
                )
            }
        });
        signed(raw, "the binding range offset of a field")
    }

    /// Returns the position among [`TypeLayout::binding_ranges`] of the
    /// binding range of the counter of a structured buffer that has one,
    /// such as an append buffer, or 0 for a layout of any other kind.
    ///
    /// Aborts the process, instead of returning an error, when this is the
    /// first call that needs the binding ranges of a layout that holds an
    /// array whose length a generic value parameter gives, such as
    /// `float values[M]` in the parameters of the generic entry point
    /// `main<let M : int>` of a component type that is not yet specialized.
    /// Slang computes the binding ranges of a layout once, lets a C++
    /// exception escape for such an array, and the crate then writes one
    /// line to standard error that names the call `spReflectionTypeLayout_getExplicitCounterBindingRangeOffset`, and aborts
    /// before any destructor runs.
    ///
    /// # Examples
    ///
    /// This example reads the binding range of the counter of a buffer:
    ///
    /// ```
    /// use slang_compiler::reflection::TypeLayout;
    ///
    /// fn counter_range(layout: TypeLayout<'_>) -> slang_compiler::Result<usize> {
    ///     layout.explicit_counter_binding_range_offset()
    /// }
    /// # let _ = counter_range;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Internal`] when Slang reports a
    /// negative position.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn explicit_counter_binding_range_offset(self) -> Result<usize> {
        let raw = guard::call("TypeLayout::explicit_counter_binding_range_offset", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getExplicitCounterBindingRangeOffset",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getExplicitCounterBindingRangeOffset(self.raw())
                    },
                )
            }
        });
        signed(raw, "the binding range offset of a counter")
    }

    /// Returns an iterator over the descriptor sets that the type uses, the
    /// numbered groups of bindings of Vulkan, which Direct3D calls register
    /// spaces, with the descriptor ranges of each.
    ///
    /// Aborts the process, instead of returning an error, when this is the
    /// first call that needs the binding ranges of a layout that holds an
    /// array whose length a generic value parameter gives, such as
    /// `float values[M]` in the parameters of the generic entry point
    /// `main<let M : int>` of a component type that is not yet specialized.
    /// Slang computes the binding ranges of a layout once, lets a C++
    /// exception escape for such an array, and the crate then writes one
    /// line to standard error that names the call `spReflectionTypeLayout_getDescriptorSetCount`, and aborts
    /// before any destructor runs.
    ///
    /// # Examples
    ///
    /// See the example of [`DescriptorSet`](super::DescriptorSet).
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
    pub fn descriptor_sets(self) -> Result<DescriptorSets<'a>> {
        let raw = guard::call("TypeLayout::descriptor_sets", || {
            // SAFETY: as for `ty`.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_getDescriptorSetCount", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_getDescriptorSetCount(self.raw())
                })
            }
        });
        Ok(DescriptorSets::new(
            self,
            signed(raw, "the descriptor set count")?,
        ))
    }

    /// Returns the pointer to the type layout, for use with the reflection
    /// functions of `slang-compiler-sys` that this crate does not wrap.
    ///
    /// The pointer is valid only while the object that the layout borrows
    /// lives. Every use of it is unsafe and follows the safety contract of
    /// `slang-compiler-sys`.
    ///
    /// # Examples
    ///
    /// This example counts the sub-object ranges of a type layout through
    /// the raw bindings:
    ///
    /// ```
    /// use slang_compiler::reflection::TypeLayout;
    /// use slang_compiler_sys::{SlangInt, spReflectionTypeLayout_getSubObjectRangeCount};
    ///
    /// fn sub_object_ranges(layout: TypeLayout<'_>) -> SlangInt {
    ///     // SAFETY: `layout` keeps the type layout alive during the call, and
    ///     // only this thread uses it.
    ///     unsafe { spReflectionTypeLayout_getSubObjectRangeCount(layout.as_raw().as_ptr()) }
    /// }
    /// # let _ = sub_object_ranges;
    /// ```
    #[must_use]
    pub fn as_raw(&self) -> NonNull<SlangReflectionTypeLayout> {
        self.raw
    }
}

impl fmt::Debug for TypeLayout<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TypeLayout")
            .field("raw", &self.raw)
            .finish_non_exhaustive()
    }
}

/// An iterator over the variable layouts of the fields of a structure, which
/// [`TypeLayout::field_layouts`] returns.
///
/// Each call of `next` asks Slang for the field at the next position. A
/// position at which Slang returns nothing yields no item, so the iterator
/// does not implement [`ExactSizeIterator`]. Each call of `next` panics when
/// it is called while another call of this crate into Slang is in progress
/// on the same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`TypeLayout`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::FieldLayouts<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::FieldLayouts<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct FieldLayouts<'a> {
    layout: TypeLayout<'a>,
    positions: Positions,
}

impl<'a> FieldLayouts<'a> {
    fn at(&self, position: usize) -> Option<VariableLayout<'a>> {
        let index = index(position)?;
        let raw = guard::call("FieldLayouts::next", || {
            // SAFETY: as for `TypeLayout::ty`, and `index` is below the count
            // that Slang reported, which bounds the index that
            // `spReflectionTypeLayout_GetFieldByIndex` does not check.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetFieldByIndex", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetFieldByIndex(self.layout.raw(), index)
                })
            }
        });
        // SAFETY: Slang returned a variable layout that the type layout
        // holds.
        unsafe { VariableLayout::from_output(raw, self.layout.session) }
    }
}

position_iterator!(FieldLayouts<'a>, VariableLayout<'a>);

/// An iterator over the parameter categories that a type uses, which
/// [`TypeLayout::categories`] returns.
///
/// Each item is a [`Result`](crate::Result), because the crate converts each
/// category that Slang reports, and a value that the pinned Slang header
/// does not list is an error of kind [`ErrorKind::Unrecognized`]. A position
/// at which Slang reports no category yields no item, so the iterator does
/// not implement [`ExactSizeIterator`]. Each call of `next` panics when it is
/// called while another call of this crate into Slang is in progress on the
/// same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`TypeLayout::categories`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::Categories<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::Categories<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct Categories<'a> {
    layout: TypeLayout<'a>,
    positions: Positions,
}

impl Categories<'_> {
    fn at(&self, position: usize) -> Option<Result<ParameterCategory>> {
        let index = index(position)?;
        let raw = guard::call("Categories::next", || {
            // SAFETY: as for `TypeLayout::ty`, and `index` is below the count
            // that Slang reported, which bounds the index that
            // `spReflectionTypeLayout_GetCategoryByIndex` does not check.
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_GetCategoryByIndex", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_GetCategoryByIndex(self.layout.raw(), index)
                })
            }
        });
        ParameterCategory::from_raw(raw).transpose()
    }
}

position_iterator!(Categories<'a>, Result<ParameterCategory>);
