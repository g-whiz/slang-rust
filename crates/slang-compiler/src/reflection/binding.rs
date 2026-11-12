use std::fmt;

use slang_compiler_sys::{
    SlangBindingType, SlangInt, spReflectionTypeLayout_getBindingRangeBindingCount,
    spReflectionTypeLayout_getBindingRangeDescriptorRangeCount,
    spReflectionTypeLayout_getBindingRangeDescriptorSetIndex,
    spReflectionTypeLayout_getBindingRangeFirstDescriptorRangeIndex,
    spReflectionTypeLayout_getBindingRangeLeafTypeLayout,
    spReflectionTypeLayout_getBindingRangeLeafVariable, spReflectionTypeLayout_getBindingRangeType,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeCategory,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeCount,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeDescriptorCount,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeIndexOffset,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeType,
    spReflectionTypeLayout_getDescriptorSetSpaceOffset,
    spReflectionTypeLayout_isBindingRangeSpecializable,
};

use super::{
    LayoutSize, Positions, TypeLayout, Variable, signed, signed_binding_number, signed_index,
};
use crate::blob::{BindingIndex, BindingSpace};
use crate::enums::{ParameterCategory, unrecognized};
use crate::error::{Error, ErrorKind, Operation, Result};
use crate::guard;

/// The kind of the bindings of a binding range or a descriptor range, such
/// as textures, samplers, or constant buffers.
///
/// A binding is a numbered slot of a graphics API through which a shader
/// reaches one resource. Slang reports a kind as a base kind combined with a
/// flag that marks resources that a shader writes, and this enumeration has
/// one variant for each combination that the pinned Slang header names. The
/// enumeration is non-exhaustive, so a later version can add kinds, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`BindingRange`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BindingType {
    /// A kind that Slang does not know, as Slang's
    /// `SLANG_BINDING_TYPE_UNKNOWN`.
    Unknown,
    /// A sampler, as Slang's `SLANG_BINDING_TYPE_SAMPLER`.
    Sampler,
    /// A texture that a shader reads, as Slang's
    /// `SLANG_BINDING_TYPE_TEXTURE`.
    Texture,
    /// A constant buffer, a buffer of uniform data, as Slang's
    /// `SLANG_BINDING_TYPE_CONSTANT_BUFFER`.
    ConstantBuffer,
    /// A parameter block, a group of parameters bound as one unit, as
    /// Slang's `SLANG_BINDING_TYPE_PARAMETER_BLOCK`.
    ParameterBlock,
    /// A typed buffer that a shader reads, as Slang's
    /// `SLANG_BINDING_TYPE_TYPED_BUFFER`.
    TypedBuffer,
    /// A raw or structured buffer that a shader reads, as Slang's
    /// `SLANG_BINDING_TYPE_RAW_BUFFER`.
    RawBuffer,
    /// A texture combined with its sampler, as Slang's
    /// `SLANG_BINDING_TYPE_COMBINED_TEXTURE_SAMPLER`.
    CombinedTextureSampler,
    /// An input attachment of a render pass, as Slang's
    /// `SLANG_BINDING_TYPE_INPUT_RENDER_TARGET`.
    InputRenderTarget,
    /// Uniform data placed directly in a binding, as Slang's
    /// `SLANG_BINDING_TYPE_INLINE_UNIFORM_DATA`.
    InlineUniformData,
    /// An acceleration structure of ray tracing, as Slang's
    /// `SLANG_BINDING_TYPE_RAY_TRACING_ACCELERATION_STRUCTURE`.
    RayTracingAccelerationStructure,
    /// A varying input of a stage, as Slang's
    /// `SLANG_BINDING_TYPE_VARYING_INPUT`.
    VaryingInput,
    /// A varying output of a stage, as Slang's
    /// `SLANG_BINDING_TYPE_VARYING_OUTPUT`.
    VaryingOutput,
    /// A value of a Slang interface type, as Slang's
    /// `SLANG_BINDING_TYPE_EXISTENTIAL_VALUE`.
    ExistentialValue,
    /// Push constants of Vulkan, as Slang's
    /// `SLANG_BINDING_TYPE_PUSH_CONSTANT`.
    PushConstant,
    /// A texture that a shader writes, as Slang's
    /// `SLANG_BINDING_TYPE_MUTABLE_TETURE`, which the header spells so.
    MutableTexture,
    /// A typed buffer that a shader writes, as Slang's
    /// `SLANG_BINDING_TYPE_MUTABLE_TYPED_BUFFER`.
    MutableTypedBuffer,
    /// A raw or structured buffer that a shader writes, as Slang's
    /// `SLANG_BINDING_TYPE_MUTABLE_RAW_BUFFER`.
    MutableRawBuffer,
}

impl BindingType {
    /// Converts a binding type that reflection returned (SLC-14).
    ///
    /// The flag `SLANG_BINDING_TYPE_MUTABLE_FLAG` alone and the masks
    /// `SLANG_BINDING_TYPE_BASE_MASK` and `SLANG_BINDING_TYPE_EXT_MASK` name no
    /// kind, so their values are unrecognized.
    pub(crate) fn from_raw(raw: SlangBindingType) -> Result<Self> {
        use SlangBindingType as B;
        Ok(match raw {
            B::SLANG_BINDING_TYPE_UNKNOWN => Self::Unknown,
            B::SLANG_BINDING_TYPE_SAMPLER => Self::Sampler,
            B::SLANG_BINDING_TYPE_TEXTURE => Self::Texture,
            B::SLANG_BINDING_TYPE_CONSTANT_BUFFER => Self::ConstantBuffer,
            B::SLANG_BINDING_TYPE_PARAMETER_BLOCK => Self::ParameterBlock,
            B::SLANG_BINDING_TYPE_TYPED_BUFFER => Self::TypedBuffer,
            B::SLANG_BINDING_TYPE_RAW_BUFFER => Self::RawBuffer,
            B::SLANG_BINDING_TYPE_COMBINED_TEXTURE_SAMPLER => Self::CombinedTextureSampler,
            B::SLANG_BINDING_TYPE_INPUT_RENDER_TARGET => Self::InputRenderTarget,
            B::SLANG_BINDING_TYPE_INLINE_UNIFORM_DATA => Self::InlineUniformData,
            B::SLANG_BINDING_TYPE_RAY_TRACING_ACCELERATION_STRUCTURE => {
                Self::RayTracingAccelerationStructure
            }
            B::SLANG_BINDING_TYPE_VARYING_INPUT => Self::VaryingInput,
            B::SLANG_BINDING_TYPE_VARYING_OUTPUT => Self::VaryingOutput,
            B::SLANG_BINDING_TYPE_EXISTENTIAL_VALUE => Self::ExistentialValue,
            B::SLANG_BINDING_TYPE_PUSH_CONSTANT => Self::PushConstant,
            B::SLANG_BINDING_TYPE_MUTABLE_TETURE => Self::MutableTexture,
            B::SLANG_BINDING_TYPE_MUTABLE_TYPED_BUFFER => Self::MutableTypedBuffer,
            B::SLANG_BINDING_TYPE_MUTABLE_RAW_BUFFER => Self::MutableRawBuffer,
            SlangBindingType(other) => return Err(unrecognized("binding type", other)),
        })
    }
}

/// One binding range of a type layout, a run of consecutive bindings of one
/// kind, such as the bindings of an array of textures.
///
/// A binding is a numbered slot of a graphics API through which a shader
/// reaches one resource. Binding ranges flatten the bindings of nested
/// structures and arrays into one list. Get them from
/// [`TypeLayout::binding_ranges`]. Each method asks Slang about the range at
/// the position that this value holds. It borrows the object that its type
/// layout came from, so it cannot outlive it.
///
/// # Examples
///
/// This example prints the kind and the number of bindings of each binding
/// range of a type layout:
///
/// ```
/// use slang_compiler::reflection::TypeLayout;
///
/// fn print_ranges(layout: TypeLayout<'_>) -> slang_compiler::Result<()> {
///     for range in layout.binding_ranges()? {
///         println!(
///             "{:?} x {:?} in descriptor set {:?}",
///             range.binding_type()?,
///             range.binding_count()?,
///             range.descriptor_set_index()?
///         );
///     }
///     Ok(())
/// }
/// # let _ = print_ranges;
/// ```
///
/// A binding range cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::BindingRange<'static>>();
/// ```
///
/// A binding range cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::BindingRange<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct BindingRange<'a> {
    layout: TypeLayout<'a>,
    index: SlangInt,
}

impl<'a> BindingRange<'a> {
    /// Returns the kind of the bindings of the range.
    ///
    /// # Examples
    ///
    /// See the example of [`BindingRange`].
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
    pub fn binding_type(self) -> Result<BindingType> {
        let raw = guard::call("BindingRange::binding_type", || {
            // SAFETY: the type layout is live, which the wrapper that `'a`
            // borrows keeps alive, used only on this thread, the call runs
            // with the Slang call guard raised, and the index is below the
            // count that Slang reported, which the function also checks
            // itself (Section 11.3 of `docs/design/slang-compiler.md`).
            unsafe {
                guard::abort_on_exception("spReflectionTypeLayout_getBindingRangeType", || {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                    )]
                    spReflectionTypeLayout_getBindingRangeType(self.layout.raw(), self.index)
                })
            }
        });
        BindingType::from_raw(raw)
    }

    /// Returns whether the range holds a value or a parameter block of a
    /// Slang interface type, whose bindings depend on the concrete type that
    /// specialization supplies.
    ///
    /// # Examples
    ///
    /// This example tells whether any range of a type layout depends on
    /// specialization:
    ///
    /// ```
    /// use slang_compiler::reflection::TypeLayout;
    ///
    /// fn needs_specialization(layout: TypeLayout<'_>) -> slang_compiler::Result<bool> {
    ///     Ok(layout.binding_ranges()?.any(|range| range.is_specializable()))
    /// }
    /// # let _ = needs_specialization;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn is_specializable(self) -> bool {
        let raw = guard::call("BindingRange::is_specializable", || {
            // SAFETY: as for `binding_type`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_isBindingRangeSpecializable",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_isBindingRangeSpecializable(
                            self.layout.raw(),
                            self.index,
                        )
                    },
                )
            }
        });
        raw != 0
    }

    /// Returns the number of bindings of the range, which is
    /// [`LayoutSize::Unbounded`] for an array whose length the source does
    /// not declare.
    ///
    /// # Examples
    ///
    /// See the example of [`BindingRange`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Internal`](crate::ErrorKind::Internal) when Slang reports
    /// a negative count other than its marks of an unbounded or unknown size.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn binding_count(self) -> Result<LayoutSize> {
        let raw = guard::call("BindingRange::binding_count", || {
            // SAFETY: as for `binding_type`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getBindingRangeBindingCount",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getBindingRangeBindingCount(
                            self.layout.raw(),
                            self.index,
                        )
                    },
                )
            }
        });
        LayoutSize::from_signed(raw, "the binding count of a binding range")
    }

    /// Returns the layout of the type of the leaf of the range, the
    /// innermost type whose bindings the range holds, such as the texture
    /// type of an array of textures, or `None` when Slang returns none.
    ///
    /// # Examples
    ///
    /// This example reads the type name of the leaf of a range:
    ///
    /// ```
    /// use slang_compiler::reflection::BindingRange;
    ///
    /// fn leaf_type(range: BindingRange<'_>) -> slang_compiler::Result<Option<String>> {
    ///     range
    ///         .leaf_type_layout()
    ///         .and_then(|layout| layout.ty())
    ///         .map(|ty| ty.full_name())
    ///         .transpose()
    /// }
    /// # let _ = leaf_type;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn leaf_type_layout(self) -> Option<TypeLayout<'a>> {
        let raw = guard::call("BindingRange::leaf_type_layout", || {
            // SAFETY: as for `binding_type`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getBindingRangeLeafTypeLayout",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getBindingRangeLeafTypeLayout(
                            self.layout.raw(),
                            self.index,
                        )
                    },
                )
            }
        });
        // SAFETY: Slang returned a type layout that the type layout of the
        // range holds, which lives for `'a`.
        unsafe { TypeLayout::from_output(raw, self.layout.session()) }
    }

    /// Returns the variable of the leaf of the range, the innermost
    /// variable whose bindings the range holds, or `None` when Slang returns
    /// none.
    ///
    /// # Examples
    ///
    /// This example reads the name of the variable of the leaf of a range:
    ///
    /// ```
    /// use slang_compiler::reflection::BindingRange;
    ///
    /// fn leaf_name<'a>(range: BindingRange<'a>) -> slang_compiler::Result<Option<&'a str>> {
    ///     Ok(range.leaf_variable().map(|v| v.name()).transpose()?.flatten())
    /// }
    /// # let _ = leaf_name;
    /// ```
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    #[must_use]
    pub fn leaf_variable(self) -> Option<Variable<'a>> {
        let raw = guard::call("BindingRange::leaf_variable", || {
            // SAFETY: as for `binding_type`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getBindingRangeLeafVariable",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getBindingRangeLeafVariable(
                            self.layout.raw(),
                            self.index,
                        )
                    },
                )
            }
        });
        // SAFETY: Slang returned a variable in the syntax trees of the
        // session of the type layout, which live for `'a`.
        unsafe { Variable::from_output(raw, self.layout.session()) }
    }

    /// Returns the position among [`TypeLayout::descriptor_sets`] of the
    /// descriptor set that holds the bindings of the range, or `None` for a
    /// range that has its own descriptor set, such as a parameter block.
    ///
    /// # Examples
    ///
    /// See the example of [`BindingRange`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Internal`](crate::ErrorKind::Internal) when Slang reports
    /// a negative position other than the -1 of a range without one.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn descriptor_set_index(self) -> Result<Option<usize>> {
        let raw = guard::call("BindingRange::descriptor_set_index", || {
            // SAFETY: as for `binding_type`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getBindingRangeDescriptorSetIndex",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getBindingRangeDescriptorSetIndex(
                            self.layout.raw(),
                            self.index,
                        )
                    },
                )
            }
        });
        if raw == -1 {
            return Ok(None);
        }
        signed(raw, "the descriptor set of a binding range").map(Some)
    }

    /// Returns the position, among the descriptor ranges of the descriptor
    /// set of the range, of the first descriptor range that the range
    /// fills.
    ///
    /// # Examples
    ///
    /// This example lists the positions of the descriptor ranges of a
    /// binding range:
    ///
    /// ```
    /// use std::ops::Range;
    ///
    /// use slang_compiler::reflection::BindingRange;
    ///
    /// fn descriptor_ranges(range: BindingRange<'_>) -> slang_compiler::Result<Range<usize>> {
    ///     let first = range.first_descriptor_range_index()?;
    ///     Ok(first..first + range.descriptor_range_count()?)
    /// }
    /// # let _ = descriptor_ranges;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Internal`](crate::ErrorKind::Internal) when Slang reports
    /// a negative position.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn first_descriptor_range_index(self) -> Result<usize> {
        let raw = guard::call("BindingRange::first_descriptor_range_index", || {
            // SAFETY: as for `binding_type`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getBindingRangeFirstDescriptorRangeIndex",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getBindingRangeFirstDescriptorRangeIndex(
                            self.layout.raw(),
                            self.index,
                        )
                    },
                )
            }
        });
        signed(raw, "the first descriptor range of a binding range")
    }

    /// Returns the number of descriptor ranges that the range fills in its
    /// descriptor set.
    ///
    /// # Examples
    ///
    /// See the example of [`BindingRange::first_descriptor_range_index`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Internal`](crate::ErrorKind::Internal) when Slang reports
    /// a negative count.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn descriptor_range_count(self) -> Result<usize> {
        let raw = guard::call("BindingRange::descriptor_range_count", || {
            // SAFETY: as for `binding_type`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getBindingRangeDescriptorRangeCount",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getBindingRangeDescriptorRangeCount(
                            self.layout.raw(),
                            self.index,
                        )
                    },
                )
            }
        });
        signed(raw, "the descriptor range count of a binding range")
    }
}

impl fmt::Debug for BindingRange<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BindingRange")
            .field("layout", &self.layout)
            .field("index", &self.index)
            .finish()
    }
}

/// An iterator over the binding ranges of a type layout, which
/// [`TypeLayout::binding_ranges`] returns.
///
/// It yields one [`BindingRange`] for each position below the count that
/// Slang reported, so it implements [`ExactSizeIterator`]. Creating each item
/// calls nothing in Slang, and each method of an item panics when it is
/// called while another call of this crate into Slang is in progress on the
/// same thread, because Slang is not reentrant.
///
/// # Examples
///
/// See the example of [`BindingRange`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::BindingRanges<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::BindingRanges<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct BindingRanges<'a> {
    layout: TypeLayout<'a>,
    positions: Positions,
}

impl<'a> BindingRanges<'a> {
    pub(super) fn new(layout: TypeLayout<'a>, count: usize) -> Self {
        Self {
            layout,
            positions: Positions::new(count),
        }
    }

    fn at(&self, position: usize) -> Option<BindingRange<'a>> {
        signed_index(position).map(|index| BindingRange {
            layout: self.layout,
            index,
        })
    }
}

position_iterator!(BindingRanges<'a>, BindingRange<'a>, exact);

/// One descriptor set that a type layout uses, a numbered group of bindings
/// of Vulkan, which Direct3D calls a register space, with its descriptor
/// ranges.
///
/// A descriptor is the record of one binding in a descriptor set, and a
/// descriptor range is a run of consecutive descriptors of one kind. Get
/// descriptor sets from [`TypeLayout::descriptor_sets`]. Each method asks
/// Slang about the set at the position that this value holds. It borrows the
/// object that its type layout came from, so it cannot outlive it.
///
/// # Examples
///
/// This example prints the descriptor ranges of each descriptor set of a
/// type layout:
///
/// ```
/// use slang_compiler::reflection::TypeLayout;
///
/// fn print_sets(layout: TypeLayout<'_>) -> slang_compiler::Result<()> {
///     for set in layout.descriptor_sets()? {
///         println!("set at offset {}", set.space_offset()?.get());
///         for range in set.descriptor_ranges()? {
///             println!(
///                 "  {:?} x {:?} at {:?}",
///                 range.binding_type()?,
///                 range.descriptor_count()?,
///                 range.index_offset()?
///             );
///         }
///     }
///     Ok(())
/// }
/// # let _ = print_sets;
/// ```
///
/// A descriptor set cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::DescriptorSet<'static>>();
/// ```
///
/// A descriptor set cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::DescriptorSet<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct DescriptorSet<'a> {
    layout: TypeLayout<'a>,
    index: SlangInt,
}

impl<'a> DescriptorSet<'a> {
    /// Returns the binding space of the set, which Vulkan calls the number of
    /// a descriptor set, as an offset from the binding space of the variable
    /// whose type layout this is.
    ///
    /// # Examples
    ///
    /// See the example of [`DescriptorSet`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Internal`](crate::ErrorKind::Internal) when Slang reports
    /// a negative offset.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn space_offset(self) -> Result<BindingSpace> {
        let raw = guard::call("DescriptorSet::space_offset", || {
            // SAFETY: as for `BindingRange::binding_type`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getDescriptorSetSpaceOffset",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getDescriptorSetSpaceOffset(
                            self.layout.raw(),
                            self.index,
                        )
                    },
                )
            }
        });
        u64::try_from(raw).map(BindingSpace::new).map_err(|error| {
            Error::new(ErrorKind::Internal, Operation::Reflect).with_detail(format!(
                "Slang reported the space offset of a descriptor set {raw}: {error}"
            ))
        })
    }

    /// Returns an iterator over the descriptor ranges of the set.
    ///
    /// # Examples
    ///
    /// See the example of [`DescriptorSet`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Internal`](crate::ErrorKind::Internal) when Slang reports
    /// a negative count.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn descriptor_ranges(self) -> Result<DescriptorRanges<'a>> {
        let raw = guard::call("DescriptorSet::descriptor_ranges", || {
            // SAFETY: as for `BindingRange::binding_type`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getDescriptorSetDescriptorRangeCount",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getDescriptorSetDescriptorRangeCount(
                            self.layout.raw(),
                            self.index,
                        )
                    },
                )
            }
        });
        Ok(DescriptorRanges {
            set: self,
            positions: Positions::new(signed(raw, "the descriptor range count of a set")?),
        })
    }
}

impl fmt::Debug for DescriptorSet<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DescriptorSet")
            .field("layout", &self.layout)
            .field("index", &self.index)
            .finish()
    }
}

/// An iterator over the descriptor sets of a type layout, which
/// [`TypeLayout::descriptor_sets`] returns.
///
/// It yields one [`DescriptorSet`] for each position below the count that
/// Slang reported, so it implements [`ExactSizeIterator`].
///
/// # Examples
///
/// See the example of [`DescriptorSet`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::DescriptorSets<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::DescriptorSets<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct DescriptorSets<'a> {
    layout: TypeLayout<'a>,
    positions: Positions,
}

impl<'a> DescriptorSets<'a> {
    pub(super) fn new(layout: TypeLayout<'a>, count: usize) -> Self {
        Self {
            layout,
            positions: Positions::new(count),
        }
    }

    fn at(&self, position: usize) -> Option<DescriptorSet<'a>> {
        signed_index(position).map(|index| DescriptorSet {
            layout: self.layout,
            index,
        })
    }
}

position_iterator!(DescriptorSets<'a>, DescriptorSet<'a>, exact);

/// One descriptor range of a descriptor set, a run of consecutive
/// descriptors of one kind, such as the descriptors of an array of
/// textures.
///
/// Get descriptor ranges from [`DescriptorSet::descriptor_ranges`]. Each
/// method asks Slang about the range at the positions that this value holds.
/// It borrows the object that its type layout came from, so it cannot
/// outlive it.
///
/// # Examples
///
/// See the example of [`DescriptorSet`].
///
/// A descriptor range cannot move to another thread, so this example does
/// not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::DescriptorRange<'static>>();
/// ```
///
/// A descriptor range cannot be shared between threads either, so this
/// example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::DescriptorRange<'static>>();
/// ```
#[derive(Clone, Copy)]
pub struct DescriptorRange<'a> {
    set: DescriptorSet<'a>,
    index: SlangInt,
}

impl DescriptorRange<'_> {
    /// Returns the binding number of the first descriptor of the range,
    /// relative to the start of its descriptor set, or `None` when Slang does
    /// not know it.
    ///
    /// # Examples
    ///
    /// See the example of [`DescriptorSet`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Internal`](crate::ErrorKind::Internal) when Slang reports
    /// a negative offset other than its mark of an unknown size.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn index_offset(self) -> Result<Option<BindingIndex>> {
        let raw = guard::call("DescriptorRange::index_offset", || {
            // SAFETY: as for `BindingRange::binding_type`, with both indexes
            // below the counts that Slang reported, which the function also
            // checks itself.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getDescriptorSetDescriptorRangeIndexOffset",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getDescriptorSetDescriptorRangeIndexOffset(
                            self.set.layout.raw(),
                            self.set.index,
                            self.index,
                        )
                    },
                )
            }
        });
        Ok(
            signed_binding_number(raw, "the index offset of a descriptor range")?
                .map(BindingIndex::new),
        )
    }

    /// Returns the number of descriptors of the range, which is
    /// [`LayoutSize::Unbounded`] for an array whose length the source does
    /// not declare.
    ///
    /// # Examples
    ///
    /// See the example of [`DescriptorSet`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Internal`](crate::ErrorKind::Internal) when Slang reports
    /// a negative count other than its marks of an unbounded or unknown size.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn descriptor_count(self) -> Result<LayoutSize> {
        let raw = guard::call("DescriptorRange::descriptor_count", || {
            // SAFETY: as for `index_offset`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getDescriptorSetDescriptorRangeDescriptorCount",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getDescriptorSetDescriptorRangeDescriptorCount(
                            self.set.layout.raw(),
                            self.set.index,
                            self.index,
                        )
                    },
                )
            }
        });
        LayoutSize::from_signed(raw, "the descriptor count of a descriptor range")
    }

    /// Returns the kind of the descriptors of the range.
    ///
    /// # Examples
    ///
    /// See the example of [`DescriptorSet`].
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
    pub fn binding_type(self) -> Result<BindingType> {
        let raw = guard::call("DescriptorRange::binding_type", || {
            // SAFETY: as for `index_offset`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getDescriptorSetDescriptorRangeType",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getDescriptorSetDescriptorRangeType(
                            self.set.layout.raw(),
                            self.set.index,
                            self.index,
                        )
                    },
                )
            }
        });
        BindingType::from_raw(raw)
    }

    /// Returns the parameter category of the descriptors of the range, the
    /// kind of binding in whose units they are counted, or `None` when Slang
    /// reports none.
    ///
    /// # Examples
    ///
    /// This example reads the categories of the descriptor ranges of a set:
    ///
    /// ```
    /// use slang_compiler::ParameterCategory;
    /// use slang_compiler::reflection::DescriptorSet;
    ///
    /// fn categories(set: DescriptorSet<'_>) -> slang_compiler::Result<Vec<Option<ParameterCategory>>> {
    ///     set.descriptor_ranges()?.map(|range| range.category()).collect()
    /// }
    /// # let _ = categories;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when Slang
    /// reports a category that the pinned Slang header does not list.
    ///
    /// # Panics
    ///
    /// Panics when it is called while another call of this crate into Slang
    /// is in progress on the same thread, because Slang is not reentrant. Only
    /// code that Slang calls back during such a call can do that.
    pub fn category(self) -> Result<Option<ParameterCategory>> {
        let raw = guard::call("DescriptorRange::category", || {
            // SAFETY: as for `index_offset`.
            unsafe {
                guard::abort_on_exception(
                    "spReflectionTypeLayout_getDescriptorSetDescriptorRangeCategory",
                    || {
                        #[expect(
                            ffi_unwind_calls,
                            reason = "Slang is C++, and the call runs inside the abort of an escaping exception (FFI-7)"
                        )]
                        spReflectionTypeLayout_getDescriptorSetDescriptorRangeCategory(
                            self.set.layout.raw(),
                            self.set.index,
                            self.index,
                        )
                    },
                )
            }
        });
        ParameterCategory::from_raw(raw)
    }
}

impl fmt::Debug for DescriptorRange<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DescriptorRange")
            .field("set", &self.set)
            .field("index", &self.index)
            .finish()
    }
}

/// An iterator over the descriptor ranges of a descriptor set, which
/// [`DescriptorSet::descriptor_ranges`] returns.
///
/// It yields one [`DescriptorRange`] for each position below the count that
/// Slang reported, so it implements [`ExactSizeIterator`].
///
/// # Examples
///
/// See the example of [`DescriptorSet`].
///
/// The iterator cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_compiler::reflection::DescriptorRanges<'static>>();
/// ```
///
/// The iterator cannot be shared between threads either, so this example
/// does not compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_compiler::reflection::DescriptorRanges<'static>>();
/// ```
#[derive(Clone, Debug)]
pub struct DescriptorRanges<'a> {
    set: DescriptorSet<'a>,
    positions: Positions,
}

impl<'a> DescriptorRanges<'a> {
    fn at(&self, position: usize) -> Option<DescriptorRange<'a>> {
        signed_index(position).map(|index| DescriptorRange {
            set: self.set,
            index,
        })
    }
}

position_iterator!(DescriptorRanges<'a>, DescriptorRange<'a>, exact);

#[cfg(test)]
mod tests {
    use slang_compiler_sys::SlangBindingType;

    use super::BindingType;
    use crate::ErrorKind;

    #[test]
    fn every_named_binding_type_converts_to_a_distinct_variant() {
        let mut seen = Vec::new();
        let values = (0..=SlangBindingType::SLANG_BINDING_TYPE_PUSH_CONSTANT.0).chain([
            SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_TETURE.0,
            SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_TYPED_BUFFER.0,
            SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_RAW_BUFFER.0,
        ]);
        for value in values {
            let kind = BindingType::from_raw(SlangBindingType(value)).expect("a named kind");
            assert!(!seen.contains(&kind), "{kind:?} twice");
            seen.push(kind);
        }
        assert_eq!(seen.len(), 18);
    }

    #[test]
    fn the_flag_and_masks_of_binding_types_are_unrecognized() {
        for value in [
            SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_FLAG,
            SlangBindingType::SLANG_BINDING_TYPE_BASE_MASK,
            SlangBindingType::SLANG_BINDING_TYPE_EXT_MASK,
            SlangBindingType(15),
            SlangBindingType(257),
        ] {
            let error = BindingType::from_raw(value).expect_err("no kind has this value");
            assert_eq!(error.kind(), ErrorKind::Unrecognized);
        }
    }
}
