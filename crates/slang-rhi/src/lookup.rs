use std::ffi::CStr;
use std::ptr::NonNull;

use slang_compiler_sys::{
    SlangBindingType, SlangParameterCategory, SlangReflectionType, SlangReflectionTypeLayout,
    SlangResourceShape, SlangScalarType, SlangTypeKind, TypeLayoutReflection,
    spReflectionType_GetName, spReflectionType_GetResourceResultType,
    spReflectionType_GetResourceShape, spReflectionType_GetScalarType,
    spReflectionTypeLayout_GetFieldByIndex, spReflectionTypeLayout_GetSize,
    spReflectionTypeLayout_GetType, spReflectionTypeLayout_findFieldIndexByName,
    spReflectionTypeLayout_getBindingRangeBindingCount,
    spReflectionTypeLayout_getBindingRangeCount,
    spReflectionTypeLayout_getBindingRangeLeafTypeLayout,
    spReflectionTypeLayout_getBindingRangeType, spReflectionTypeLayout_getFieldBindingRangeOffset,
    spReflectionTypeLayout_getKind, spReflectionVariableLayout_GetOffset,
    spReflectionVariableLayout_GetTypeLayout,
};

use crate::guard;
use crate::shader_object::{BindingRange, RangeKind, ShaderOffset, TextureType};

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
        let leaf = match binding_type {
            SlangBindingType::SLANG_BINDING_TYPE_TEXTURE
            | SlangBindingType::SLANG_BINDING_TYPE_SAMPLER => self.leaf_type(index),
            _ => None,
        };
        let texture = leaf.map_or(
            TextureType {
                plain_2d: false,
                float: false,
            },
            |leaf| texture_type(leaf_shape(leaf), leaf_result(leaf)),
        );
        let comparison = leaf.is_some_and(leaf_is_comparison_sampler);
        Some(BindingRange {
            binding_count,
            kind: range_kind(binding_type, texture, comparison),
        })
    }

    /// Returns the type of the leaf of the binding range at `index`, which is
    /// below the count of binding ranges, or `None` when Slang reports none.
    fn leaf_type(self, index: isize) -> Option<NonNull<SlangReflectionType>> {
        let raw = self.raw.as_ptr();
        let leaf = guard::call(
            "spReflectionTypeLayout_getBindingRangeLeafTypeLayout",
            || {
                // SAFETY: the invariant of `ElementLayout` keeps `raw` live, and
                // `index` is below the count of binding ranges that Slang
                // reported for it.
                unsafe {
                    #[expect(
                        ffi_unwind_calls,
                        reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                    )]
                    spReflectionTypeLayout_getBindingRangeLeafTypeLayout(raw, index)
                }
            },
        );
        let leaf = NonNull::new(leaf)?;
        let ty = guard::call("spReflectionTypeLayout_GetType", || {
            // SAFETY: `leaf` is a non-null type layout that the live type
            // layout owns.
            unsafe {
                #[expect(
                    ffi_unwind_calls,
                    reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
                )]
                spReflectionTypeLayout_GetType(leaf.as_ptr())
            }
        });
        NonNull::new(ty)
    }
}

/// Returns the resource shape of the type `ty`, a type that the live type
/// layout of an `ElementLayout` owns.
fn leaf_shape(ty: NonNull<SlangReflectionType>) -> SlangResourceShape {
    guard::call("spReflectionType_GetResourceShape", || {
        // SAFETY: `ty` is a live type of the live type layout.
        unsafe {
            #[expect(
                ffi_unwind_calls,
                reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
            )]
            spReflectionType_GetResourceShape(ty.as_ptr())
        }
    })
}

/// Returns the scalar type of the elements of the resource type `ty`, a type
/// that the live type layout of an `ElementLayout` owns, or `None` when it
/// has no result type.
fn leaf_result(ty: NonNull<SlangReflectionType>) -> Option<SlangScalarType> {
    let result = guard::call("spReflectionType_GetResourceResultType", || {
        // SAFETY: `ty` is a live type of the live type layout.
        unsafe {
            #[expect(
                ffi_unwind_calls,
                reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
            )]
            spReflectionType_GetResourceResultType(ty.as_ptr())
        }
    });
    let result = NonNull::new(result)?;
    Some(guard::call("spReflectionType_GetScalarType", || {
        // SAFETY: `result` is a non-null type that the live type layout
        // owns.
        unsafe {
            #[expect(
                ffi_unwind_calls,
                reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
            )]
            spReflectionType_GetScalarType(result.as_ptr())
        }
    }))
}

/// Returns whether the type `ty`, a type that the live type layout of an
/// `ElementLayout` owns, is a comparison sampler, by its name.
fn leaf_is_comparison_sampler(ty: NonNull<SlangReflectionType>) -> bool {
    let name = guard::call("spReflectionType_GetName", || {
        // SAFETY: `ty` is a live type of the live type layout.
        unsafe {
            #[expect(
                ffi_unwind_calls,
                reason = "Slang is C++, and the call runs inside the abort of an escaping exception"
            )]
            spReflectionType_GetName(ty.as_ptr())
        }
    });
    if name.is_null() {
        return is_comparison_sampler(None);
    }
    // SAFETY: Slang returns a NUL-terminated name that it keeps for as long
    // as the type, which outlives this function.
    is_comparison_sampler(Some(unsafe { CStr::from_ptr(name) }))
}

/// Returns what a texture of the resource shape `shape`, whose elements have
/// the scalar type `result`, is.
fn texture_type(shape: SlangResourceShape, result: Option<SlangScalarType>) -> TextureType {
    TextureType {
        plain_2d: shape == SlangResourceShape::SLANG_TEXTURE_2D,
        float: matches!(
            result,
            Some(
                SlangScalarType::SLANG_SCALAR_TYPE_FLOAT16
                    | SlangScalarType::SLANG_SCALAR_TYPE_FLOAT32
            )
        ),
    }
}

/// Returns whether a sampler type of the name `name` is a comparison sampler.
fn is_comparison_sampler(name: Option<&CStr>) -> bool {
    name == Some(c"SamplerComparisonState")
}

/// Returns what a binding range of this type takes, where `texture` describes
/// the texture of a range of textures and `comparison` says whether a range
/// of samplers takes comparison samplers.
fn range_kind(binding_type: SlangBindingType, texture: TextureType, comparison: bool) -> RangeKind {
    match binding_type {
        SlangBindingType::SLANG_BINDING_TYPE_RAW_BUFFER => RangeKind::ReadOnlyBuffer,
        SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_RAW_BUFFER => RangeKind::MutableBuffer,
        SlangBindingType::SLANG_BINDING_TYPE_TYPED_BUFFER
        | SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_TYPED_BUFFER => RangeKind::TypedBuffer,
        SlangBindingType::SLANG_BINDING_TYPE_CONSTANT_BUFFER
        | SlangBindingType::SLANG_BINDING_TYPE_PARAMETER_BLOCK => RangeKind::SubObject,
        SlangBindingType::SLANG_BINDING_TYPE_TEXTURE => RangeKind::ReadOnlyTexture(texture),
        SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_TETURE => RangeKind::MutableTexture,
        SlangBindingType::SLANG_BINDING_TYPE_SAMPLER => RangeKind::Sampler { comparison },
        SlangBindingType::SLANG_BINDING_TYPE_COMBINED_TEXTURE_SAMPLER => {
            RangeKind::CombinedTextureSampler
        }
        _ => RangeKind::Other,
    }
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::SlangBindingType;

    use slang_compiler_sys::{SlangResourceShape, SlangScalarType};

    use super::{is_comparison_sampler, range_kind, texture_type};
    use crate::shader_object::{RangeKind, TextureType};

    const PLAIN: TextureType = TextureType {
        plain_2d: true,
        float: true,
    };

    #[test]
    fn range_kind_tells_buffers_objects_textures_and_samplers_apart_from_the_rest() {
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
            (
                SlangBindingType::SLANG_BINDING_TYPE_TEXTURE,
                RangeKind::ReadOnlyTexture(PLAIN),
            ),
            (
                SlangBindingType::SLANG_BINDING_TYPE_MUTABLE_TETURE,
                RangeKind::MutableTexture,
            ),
            (
                SlangBindingType::SLANG_BINDING_TYPE_SAMPLER,
                RangeKind::Sampler { comparison: true },
            ),
            (
                SlangBindingType::SLANG_BINDING_TYPE_COMBINED_TEXTURE_SAMPLER,
                RangeKind::CombinedTextureSampler,
            ),
        ];
        for (binding_type, kind) in cases {
            assert_eq!(
                range_kind(binding_type, PLAIN, true),
                kind,
                "{binding_type:?}"
            );
        }
        for other in [
            SlangBindingType::SLANG_BINDING_TYPE_UNKNOWN,
            SlangBindingType::SLANG_BINDING_TYPE_EXISTENTIAL_VALUE,
            SlangBindingType::SLANG_BINDING_TYPE_PUSH_CONSTANT,
        ] {
            assert_eq!(
                range_kind(other, PLAIN, true),
                RangeKind::Other,
                "{other:?}"
            );
        }
    }

    #[test]
    fn texture_type_is_plain_only_for_a_two_dimensional_texture_without_flags() {
        let shape = |raw| SlangResourceShape(raw);
        let float = Some(SlangScalarType::SLANG_SCALAR_TYPE_FLOAT32);
        assert!(texture_type(SlangResourceShape::SLANG_TEXTURE_2D, float).plain_2d);
        for other in [
            SlangResourceShape::SLANG_TEXTURE_1D,
            SlangResourceShape::SLANG_TEXTURE_3D,
            SlangResourceShape::SLANG_TEXTURE_CUBE,
            SlangResourceShape::SLANG_TEXTURE_2D_ARRAY,
            SlangResourceShape::SLANG_TEXTURE_2D_MULTISAMPLE,
            shape(
                SlangResourceShape::SLANG_TEXTURE_2D.0
                    | SlangResourceShape::SLANG_TEXTURE_FEEDBACK_FLAG.0,
            ),
            shape(
                SlangResourceShape::SLANG_TEXTURE_2D.0
                    | SlangResourceShape::SLANG_TEXTURE_SHADOW_FLAG.0,
            ),
        ] {
            assert!(!texture_type(other, float).plain_2d, "{other:?}");
        }
    }

    #[test]
    fn texture_type_takes_elements_of_16_and_32_bit_floating_point_numbers_only() {
        let float = |scalar| texture_type(SlangResourceShape::SLANG_TEXTURE_2D, scalar).float;
        assert!(float(Some(SlangScalarType::SLANG_SCALAR_TYPE_FLOAT32)));
        assert!(float(Some(SlangScalarType::SLANG_SCALAR_TYPE_FLOAT16)));
        for other in [
            Some(SlangScalarType::SLANG_SCALAR_TYPE_UINT32),
            Some(SlangScalarType::SLANG_SCALAR_TYPE_INT32),
            Some(SlangScalarType::SLANG_SCALAR_TYPE_FLOAT64),
            None,
        ] {
            assert!(!float(other), "{other:?}");
        }
    }

    #[test]
    fn only_a_sampler_comparison_state_is_a_comparison_sampler() {
        assert!(is_comparison_sampler(Some(c"SamplerComparisonState")));
        assert!(!is_comparison_sampler(Some(c"SamplerState")));
        assert!(!is_comparison_sampler(None));
    }
}
