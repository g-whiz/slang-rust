use std::ptr::NonNull;

use slang_compiler_sys::{
    SlangBindingType, SlangParameterCategory, SlangReflectionTypeLayout, SlangTypeKind,
    TypeLayoutReflection, spReflectionTypeLayout_GetFieldByIndex, spReflectionTypeLayout_GetSize,
    spReflectionTypeLayout_findFieldIndexByName,
    spReflectionTypeLayout_getBindingRangeBindingCount,
    spReflectionTypeLayout_getBindingRangeCount, spReflectionTypeLayout_getBindingRangeType,
    spReflectionTypeLayout_getFieldBindingRangeOffset, spReflectionTypeLayout_getKind,
    spReflectionVariableLayout_GetOffset, spReflectionVariableLayout_GetTypeLayout,
};

use crate::guard;
use crate::shader_object::{BindingRange, RangeKind, ShaderOffset};

/// The element type layout of a shader object, which Slang's reflection
/// describes, read only through the reflection functions of Slang.
///
/// Its invariant is that `raw` points to a live type layout of Slang, which
/// stays alive while the value lives. A shader object holds the layout of
/// its program, which holds the Slang program that owns the type layout, and
/// the value lives only inside one method of a borrow of that object.
#[derive(Clone, Copy, Debug)]
pub(crate) struct ElementLayout {
    raw: NonNull<SlangReflectionTypeLayout>,
}

impl ElementLayout {
    /// Wraps the element type layout that a shader object returned.
    ///
    /// # Safety
    ///
    /// `raw` points to a live type layout of Slang that stays alive while the
    /// returned value lives. Breaking this is undefined behaviour.
    pub(crate) unsafe fn new(raw: NonNull<TypeLayoutReflection>) -> Self {
        Self {
            raw: raw.cast::<SlangReflectionTypeLayout>(),
        }
    }

    /// Finds a field of the layout by its name, as the shader cursor of
    /// slang-rhi finds a field of a structure, with the size of its uniform
    /// data and the number of its binding ranges, or returns `None` when the
    /// layout is not a structure or has no field of that name, or when a
    /// number of the field does not fit a `u32`.
    pub(crate) fn find_field(self, name: &str) -> Option<ShaderOffset> {
        let raw = self.raw.as_ptr();
        let kind = guard::call("spReflectionTypeLayout_getKind", || {
            // SAFETY: the invariant of `ElementLayout` keeps `raw` live.
            unsafe {
                #[expect(
                    ffi_unwind_calls,
                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                )]
                spReflectionTypeLayout_getKind(raw)
            }
        });
        if kind != SlangTypeKind::SLANG_TYPE_KIND_STRUCT {
            return None;
        }
        let range = name.as_bytes().as_ptr_range();
        let index = guard::call("spReflectionTypeLayout_findFieldIndexByName", || {
            // SAFETY: `raw` is live, and the two pointers delimit the bytes
            // of `name`, which Slang reads only during the call and which
            // need no terminating NUL.
            unsafe {
                #[expect(
                    ffi_unwind_calls,
                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                )]
                spReflectionTypeLayout_findFieldIndexByName(
                    raw,
                    range.start.cast(),
                    range.end.cast(),
                )
            }
        });
        let field_index = u32::try_from(index).ok()?;
        let field = guard::call("spReflectionTypeLayout_GetFieldByIndex", || {
            // SAFETY: `raw` is live, and `field_index` is an index that Slang
            // returned for this layout, so the field exists.
            unsafe {
                #[expect(
                    ffi_unwind_calls,
                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                )]
                spReflectionTypeLayout_GetFieldByIndex(raw, field_index)
            }
        });
        let field = NonNull::new(field)?;
        let uniform_offset = guard::call("spReflectionVariableLayout_GetOffset", || {
            // SAFETY: `field` is a non-null variable layout that the live
            // type layout owns.
            unsafe {
                #[expect(
                    ffi_unwind_calls,
                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                )]
                spReflectionVariableLayout_GetOffset(
                    field.as_ptr(),
                    SlangParameterCategory::SLANG_PARAMETER_CATEGORY_UNIFORM,
                )
            }
        });
        let binding_range = guard::call(
            "spReflectionTypeLayout_getFieldBindingRangeOffset",
            || {
                // SAFETY: `raw` is live, and `index` is a field index that Slang
                // returned for this layout.
                unsafe {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionTypeLayout_getFieldBindingRangeOffset(raw, index)
                }
            },
        );
        let field_type = guard::call("spReflectionVariableLayout_GetTypeLayout", || {
            // SAFETY: `field` is a non-null variable layout that the live
            // type layout owns.
            unsafe {
                #[expect(
                    ffi_unwind_calls,
                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                )]
                spReflectionVariableLayout_GetTypeLayout(field.as_ptr())
            }
        });
        let field_type = Self {
            raw: NonNull::new(field_type)?,
        };
        Some(ShaderOffset::new(
            u32::try_from(uniform_offset).ok()?,
            u32::try_from(field_type.uniform_size()).ok()?,
            u32::try_from(binding_range).ok()?,
            u32::try_from(field_type.binding_range_count()).ok()?,
        ))
    }

    /// Returns the number of binding ranges of the layout.
    fn binding_range_count(self) -> isize {
        let raw = self.raw.as_ptr();
        guard::call("spReflectionTypeLayout_getBindingRangeCount", || {
            // SAFETY: the invariant of `ElementLayout` keeps `raw` live.
            unsafe {
                #[expect(
                    ffi_unwind_calls,
                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                )]
                spReflectionTypeLayout_getBindingRangeCount(raw)
            }
        })
    }

    /// Returns the size in bytes of the uniform data of the layout.
    pub(crate) fn uniform_size(self) -> usize {
        let raw = self.raw.as_ptr();
        guard::call("spReflectionTypeLayout_GetSize", || {
            // SAFETY: the invariant of `ElementLayout` keeps `raw` live.
            unsafe {
                #[expect(
                    ffi_unwind_calls,
                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                )]
                spReflectionTypeLayout_GetSize(
                    raw,
                    SlangParameterCategory::SLANG_PARAMETER_CATEGORY_UNIFORM,
                )
            }
        })
    }

    /// Returns the binding range at `index`, or `None` when the layout has no
    /// range at that index.
    pub(crate) fn binding_range(self, index: u32) -> Option<BindingRange> {
        let raw = self.raw.as_ptr();
        let count = self.binding_range_count();
        let index = isize::try_from(index).ok().filter(|index| *index < count)?;
        let binding_count = guard::call(
            "spReflectionTypeLayout_getBindingRangeBindingCount",
            || {
                // SAFETY: `raw` is live, and `index` is below the count of
                // binding ranges that Slang reported for it.
                unsafe {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionTypeLayout_getBindingRangeBindingCount(raw, index)
                }
            },
        );
        let binding_type = guard::call("spReflectionTypeLayout_getBindingRangeType", || {
            // SAFETY: `raw` is live, and `index` is below the count of
            // binding ranges that Slang reported for it.
            unsafe {
                #[expect(
                    ffi_unwind_calls,
                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                )]
                spReflectionTypeLayout_getBindingRangeType(raw, index)
            }
        });
        Some(BindingRange {
            binding_count,
            kind: range_kind(binding_type),
        })
    }
}

/// Returns what a binding range of this type takes.
fn range_kind(binding_type: SlangBindingType) -> RangeKind {
    match binding_type {
        SlangBindingType::SLANG_BINDING_TYPE_RAW_BUFFER => RangeKind::ReadOnlyBuffer,
        SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_RAW_BUFFER => RangeKind::MutableBuffer,
        SlangBindingType::SLANG_BINDING_TYPE_TYPED_BUFFER
        | SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_TYPED_BUFFER => RangeKind::TypedBuffer,
        SlangBindingType::SLANG_BINDING_TYPE_CONSTANT_BUFFER
        | SlangBindingType::SLANG_BINDING_TYPE_PARAMETER_BLOCK => RangeKind::SubObject,
        _ => RangeKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::SlangBindingType;

    use super::range_kind;
    use crate::shader_object::RangeKind;

    #[test]
    fn range_kind_tells_raw_buffers_by_access_from_typed_buffers_objects_and_the_rest() {
        let cases = [
            (
                SlangBindingType::SLANG_BINDING_TYPE_RAW_BUFFER,
                RangeKind::ReadOnlyBuffer,
            ),
            (
                SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_RAW_BUFFER,
                RangeKind::MutableBuffer,
            ),
            (
                SlangBindingType::SLANG_BINDING_TYPE_TYPED_BUFFER,
                RangeKind::TypedBuffer,
            ),
            (
                SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_TYPED_BUFFER,
                RangeKind::TypedBuffer,
            ),
            (
                SlangBindingType::SLANG_BINDING_TYPE_CONSTANT_BUFFER,
                RangeKind::SubObject,
            ),
            (
                SlangBindingType::SLANG_BINDING_TYPE_PARAMETER_BLOCK,
                RangeKind::SubObject,
            ),
        ];
        for (binding_type, kind) in cases {
            assert_eq!(range_kind(binding_type), kind, "{binding_type:?}");
        }
        for other in [
            SlangBindingType::SLANG_BINDING_TYPE_UNKNOWN,
            SlangBindingType::SLANG_BINDING_TYPE_SAMPLER,
            SlangBindingType::SLANG_BINDING_TYPE_TEXTURE,
            SlangBindingType::SLANG_BINDING_TYPE_EXISTENTIAL_VALUE,
            SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_TETURE,
            SlangBindingType::SLANG_BINDING_TYPE_PUSH_CONSTANT,
        ] {
            assert_eq!(range_kind(other), RangeKind::Other, "{other:?}");
        }
    }
}
