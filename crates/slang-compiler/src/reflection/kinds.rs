use slang_compiler_sys::{
    SLANG_UNBOUNDED_SIZE, SLANG_UNKNOWN_SIZE, SlangInt, SlangResourceAccess, SlangResourceShape,
    SlangResourceShapeIntegral, SlangScalarType, SlangTypeKind,
};

use crate::enums::{ParameterCategory, unrecognized};
use crate::error::{Error, ErrorKind, Operation, Result};

/// The kind of a Slang type, such as a structure, an array, or a resource.
///
/// A resource is an object that a shader reads or writes through a binding,
/// a numbered slot of the graphics API, such as a buffer or a texture. The
/// enumeration is non-exhaustive, so a later version can add kinds, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`TypeLayout`](super::TypeLayout).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TypeKind {
    /// No kind, which Slang reports for a type that it cannot classify and
    /// for the missing result of a function, as Slang's
    /// `SLANG_TYPE_KIND_NONE`.
    None,
    /// A structure, as Slang's `SLANG_TYPE_KIND_STRUCT`.
    Struct,
    /// An array, as Slang's `SLANG_TYPE_KIND_ARRAY`.
    Array,
    /// A matrix, as Slang's `SLANG_TYPE_KIND_MATRIX`.
    Matrix,
    /// A vector, as Slang's `SLANG_TYPE_KIND_VECTOR`.
    Vector,
    /// A scalar, a single number or Boolean, as Slang's
    /// `SLANG_TYPE_KIND_SCALAR`.
    Scalar,
    /// A constant buffer, a buffer of uniform data that a shader reads, as
    /// Slang's `SLANG_TYPE_KIND_CONSTANT_BUFFER`.
    ConstantBuffer,
    /// A resource such as a texture or a structured buffer, as Slang's
    /// `SLANG_TYPE_KIND_RESOURCE`.
    Resource,
    /// A sampler, which describes how a shader reads a texture, as Slang's
    /// `SLANG_TYPE_KIND_SAMPLER_STATE`.
    SamplerState,
    /// A texture buffer, as Slang's `SLANG_TYPE_KIND_TEXTURE_BUFFER`.
    TextureBuffer,
    /// A shader storage buffer of GLSL, as Slang's
    /// `SLANG_TYPE_KIND_SHADER_STORAGE_BUFFER`.
    ShaderStorageBuffer,
    /// A parameter block, a group of parameters that a target binds as one
    /// unit, such as one descriptor set of Vulkan, as Slang's
    /// `SLANG_TYPE_KIND_PARAMETER_BLOCK`.
    ParameterBlock,
    /// A generic type parameter, as Slang's
    /// `SLANG_TYPE_KIND_GENERIC_TYPE_PARAMETER`.
    GenericTypeParameter,
    /// A Slang interface, an `interface` declaration that Slang types
    /// implement, as Slang's `SLANG_TYPE_KIND_INTERFACE`.
    Interface,
    /// An output stream of a geometry shader, as Slang's
    /// `SLANG_TYPE_KIND_OUTPUT_STREAM`.
    OutputStream,
    /// An output of a mesh shader, as Slang's `SLANG_TYPE_KIND_MESH_OUTPUT`.
    MeshOutput,
    /// A type specialized with the types of its Slang interface parameters,
    /// as Slang's `SLANG_TYPE_KIND_SPECIALIZED`.
    Specialized,
    /// A feedback texture, as Slang's `SLANG_TYPE_KIND_FEEDBACK`.
    Feedback,
    /// A pointer, as Slang's `SLANG_TYPE_KIND_POINTER`.
    Pointer,
    /// A resource whose type is chosen at run time, as Slang's
    /// `SLANG_TYPE_KIND_DYNAMIC_RESOURCE`.
    DynamicResource,
    /// An enumeration, as Slang's `SLANG_TYPE_KIND_ENUM`.
    Enum,
}

impl TypeKind {
    /// Converts a type kind that reflection returned (SLC-14).
    pub(crate) fn from_raw(raw: SlangTypeKind) -> Result<Self> {
        use SlangTypeKind as K;
        Ok(match raw {
            K::SLANG_TYPE_KIND_NONE => Self::None,
            K::SLANG_TYPE_KIND_STRUCT => Self::Struct,
            K::SLANG_TYPE_KIND_ARRAY => Self::Array,
            K::SLANG_TYPE_KIND_MATRIX => Self::Matrix,
            K::SLANG_TYPE_KIND_VECTOR => Self::Vector,
            K::SLANG_TYPE_KIND_SCALAR => Self::Scalar,
            K::SLANG_TYPE_KIND_CONSTANT_BUFFER => Self::ConstantBuffer,
            K::SLANG_TYPE_KIND_RESOURCE => Self::Resource,
            K::SLANG_TYPE_KIND_SAMPLER_STATE => Self::SamplerState,
            K::SLANG_TYPE_KIND_TEXTURE_BUFFER => Self::TextureBuffer,
            K::SLANG_TYPE_KIND_SHADER_STORAGE_BUFFER => Self::ShaderStorageBuffer,
            K::SLANG_TYPE_KIND_PARAMETER_BLOCK => Self::ParameterBlock,
            K::SLANG_TYPE_KIND_GENERIC_TYPE_PARAMETER => Self::GenericTypeParameter,
            K::SLANG_TYPE_KIND_INTERFACE => Self::Interface,
            K::SLANG_TYPE_KIND_OUTPUT_STREAM => Self::OutputStream,
            K::SLANG_TYPE_KIND_MESH_OUTPUT => Self::MeshOutput,
            K::SLANG_TYPE_KIND_SPECIALIZED => Self::Specialized,
            K::SLANG_TYPE_KIND_FEEDBACK => Self::Feedback,
            K::SLANG_TYPE_KIND_POINTER => Self::Pointer,
            K::SLANG_TYPE_KIND_DYNAMIC_RESOURCE => Self::DynamicResource,
            K::SLANG_TYPE_KIND_ENUM => Self::Enum,
            SlangTypeKind(other) => return Err(unrecognized("type kind", other)),
        })
    }
}

/// The type of the elements of a scalar, vector, or matrix type, such as a
/// 32-bit floating-point number.
///
/// The enumeration is non-exhaustive, so a later version can add types, and
/// a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Type`](super::Type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ScalarType {
    /// No scalar type, for a type that is not a scalar, vector, or matrix,
    /// as Slang's `SLANG_SCALAR_TYPE_NONE`.
    None,
    /// The empty type `void`, as Slang's `SLANG_SCALAR_TYPE_VOID`.
    Void,
    /// A Boolean, as Slang's `SLANG_SCALAR_TYPE_BOOL`.
    Bool,
    /// A signed integer of 32 bits, as Slang's `SLANG_SCALAR_TYPE_INT32`.
    Int32,
    /// An unsigned integer of 32 bits, as Slang's `SLANG_SCALAR_TYPE_UINT32`.
    Uint32,
    /// A signed integer of 64 bits, as Slang's `SLANG_SCALAR_TYPE_INT64`.
    Int64,
    /// An unsigned integer of 64 bits, as Slang's `SLANG_SCALAR_TYPE_UINT64`.
    Uint64,
    /// A floating-point number of 16 bits, as Slang's
    /// `SLANG_SCALAR_TYPE_FLOAT16`.
    Float16,
    /// A floating-point number of 32 bits, as Slang's
    /// `SLANG_SCALAR_TYPE_FLOAT32`.
    Float32,
    /// A floating-point number of 64 bits, as Slang's
    /// `SLANG_SCALAR_TYPE_FLOAT64`.
    Float64,
    /// A signed integer of 8 bits, as Slang's `SLANG_SCALAR_TYPE_INT8`.
    Int8,
    /// An unsigned integer of 8 bits, as Slang's `SLANG_SCALAR_TYPE_UINT8`.
    Uint8,
    /// A signed integer of 16 bits, as Slang's `SLANG_SCALAR_TYPE_INT16`.
    Int16,
    /// An unsigned integer of 16 bits, as Slang's `SLANG_SCALAR_TYPE_UINT16`.
    Uint16,
    /// A signed integer of the size of a pointer, as Slang's
    /// `SLANG_SCALAR_TYPE_INTPTR`.
    Intptr,
    /// An unsigned integer of the size of a pointer, as Slang's
    /// `SLANG_SCALAR_TYPE_UINTPTR`.
    Uintptr,
    /// The brain floating-point format of 16 bits, as Slang's
    /// `SLANG_SCALAR_TYPE_BFLOAT16`.
    Bfloat16,
    /// A floating-point number of 8 bits with 4 exponent bits and 3 mantissa
    /// bits, as Slang's `SLANG_SCALAR_TYPE_FLOAT_E4M3`.
    FloatE4m3,
    /// A floating-point number of 8 bits with 5 exponent bits and 2 mantissa
    /// bits, as Slang's `SLANG_SCALAR_TYPE_FLOAT_E5M2`.
    FloatE5m2,
}

impl ScalarType {
    /// Converts a scalar type that reflection returned (SLC-14).
    pub(crate) fn from_raw(raw: SlangScalarType) -> Result<Self> {
        use SlangScalarType as S;
        Ok(match raw {
            S::SLANG_SCALAR_TYPE_NONE => Self::None,
            S::SLANG_SCALAR_TYPE_VOID => Self::Void,
            S::SLANG_SCALAR_TYPE_BOOL => Self::Bool,
            S::SLANG_SCALAR_TYPE_INT32 => Self::Int32,
            S::SLANG_SCALAR_TYPE_UINT32 => Self::Uint32,
            S::SLANG_SCALAR_TYPE_INT64 => Self::Int64,
            S::SLANG_SCALAR_TYPE_UINT64 => Self::Uint64,
            S::SLANG_SCALAR_TYPE_FLOAT16 => Self::Float16,
            S::SLANG_SCALAR_TYPE_FLOAT32 => Self::Float32,
            S::SLANG_SCALAR_TYPE_FLOAT64 => Self::Float64,
            S::SLANG_SCALAR_TYPE_INT8 => Self::Int8,
            S::SLANG_SCALAR_TYPE_UINT8 => Self::Uint8,
            S::SLANG_SCALAR_TYPE_INT16 => Self::Int16,
            S::SLANG_SCALAR_TYPE_UINT16 => Self::Uint16,
            S::SLANG_SCALAR_TYPE_INTPTR => Self::Intptr,
            S::SLANG_SCALAR_TYPE_UINTPTR => Self::Uintptr,
            S::SLANG_SCALAR_TYPE_BFLOAT16 => Self::Bfloat16,
            S::SLANG_SCALAR_TYPE_FLOAT_E4M3 => Self::FloatE4m3,
            S::SLANG_SCALAR_TYPE_FLOAT_E5M2 => Self::FloatE5m2,
            SlangScalarType(other) => return Err(unrecognized("scalar type", other)),
        })
    }
}

/// How a shader may access a resource, such as read-only or read and write.
///
/// The enumeration is non-exhaustive, so a later version can add kinds of
/// access, and a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Type`](super::Type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceAccess {
    /// No access, for a type that is not a resource, as Slang's
    /// `SLANG_RESOURCE_ACCESS_NONE`.
    None,
    /// Read-only, as Slang's `SLANG_RESOURCE_ACCESS_READ`.
    Read,
    /// Read and write, as Slang's `SLANG_RESOURCE_ACCESS_READ_WRITE`.
    ReadWrite,
    /// Read and write in the order of the pixels that a draw rasterizes, as
    /// Slang's `SLANG_RESOURCE_ACCESS_RASTER_ORDERED`.
    RasterOrdered,
    /// Append to a buffer, as Slang's `SLANG_RESOURCE_ACCESS_APPEND`.
    Append,
    /// Consume from a buffer, as Slang's `SLANG_RESOURCE_ACCESS_CONSUME`.
    Consume,
    /// Write-only, as Slang's `SLANG_RESOURCE_ACCESS_WRITE`.
    Write,
    /// The access of a feedback texture, as Slang's
    /// `SLANG_RESOURCE_ACCESS_FEEDBACK`.
    Feedback,
    /// An access that Slang does not know, as Slang's
    /// `SLANG_RESOURCE_ACCESS_UNKNOWN`.
    Unknown,
}

impl ResourceAccess {
    /// Converts a resource access that reflection returned (SLC-14).
    pub(crate) fn from_raw(raw: SlangResourceAccess) -> Result<Self> {
        use SlangResourceAccess as A;
        Ok(match raw {
            A::SLANG_RESOURCE_ACCESS_NONE => Self::None,
            A::SLANG_RESOURCE_ACCESS_READ => Self::Read,
            A::SLANG_RESOURCE_ACCESS_READ_WRITE => Self::ReadWrite,
            A::SLANG_RESOURCE_ACCESS_RASTER_ORDERED => Self::RasterOrdered,
            A::SLANG_RESOURCE_ACCESS_APPEND => Self::Append,
            A::SLANG_RESOURCE_ACCESS_CONSUME => Self::Consume,
            A::SLANG_RESOURCE_ACCESS_WRITE => Self::Write,
            A::SLANG_RESOURCE_ACCESS_FEEDBACK => Self::Feedback,
            A::SLANG_RESOURCE_ACCESS_UNKNOWN => Self::Unknown,
            SlangResourceAccess(other) => return Err(unrecognized("resource access", other)),
        })
    }
}

/// The base shape of a resource, such as a two-dimensional texture or a
/// structured buffer, without the flags of its [`ResourceShape`].
///
/// The enumeration is non-exhaustive, so a later version can add shapes, and
/// a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Type`](super::Type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ResourceBaseShape {
    /// No shape, for a type that is not a resource, as Slang's
    /// `SLANG_RESOURCE_NONE`.
    None,
    /// A one-dimensional texture, as Slang's `SLANG_TEXTURE_1D`.
    Texture1d,
    /// A two-dimensional texture, as Slang's `SLANG_TEXTURE_2D`.
    Texture2d,
    /// A three-dimensional texture, as Slang's `SLANG_TEXTURE_3D`.
    Texture3d,
    /// A cube texture, six square faces, as Slang's `SLANG_TEXTURE_CUBE`.
    TextureCube,
    /// A texture buffer, a buffer read through a texture, as Slang's
    /// `SLANG_TEXTURE_BUFFER`.
    TextureBuffer,
    /// A structured buffer, an array of structures in a buffer, as Slang's
    /// `SLANG_STRUCTURED_BUFFER`.
    StructuredBuffer,
    /// A buffer addressed by bytes, as Slang's `SLANG_BYTE_ADDRESS_BUFFER`.
    ByteAddressBuffer,
    /// A resource of a shape that Slang does not know, as Slang's
    /// `SLANG_RESOURCE_UNKNOWN`.
    Unknown,
    /// An acceleration structure of ray tracing, as Slang's
    /// `SLANG_ACCELERATION_STRUCTURE`.
    AccelerationStructure,
    /// A subpass input of Vulkan, as Slang's `SLANG_TEXTURE_SUBPASS`.
    TextureSubpass,
}

/// The shape of a resource, a base shape combined with flags such as whether
/// it is an array.
///
/// Slang reports a resource shape as one number in which the low bits name
/// the base shape and the higher bits are flags. Read the base shape with
/// [`ResourceShape::base`] and each flag with its own method.
///
/// # Examples
///
/// See the example of [`Type`](super::Type).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ResourceShape(SlangResourceShapeIntegral);

impl ResourceShape {
    pub(crate) fn from_raw(raw: SlangResourceShape) -> Self {
        Self(raw.0)
    }

    fn has(self, flag: SlangResourceShape) -> bool {
        self.0 & flag.0 != 0
    }

    /// Returns the base shape, without the flags.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`](super::Type).
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when the
    /// base shape is a value that the pinned Slang header does not list.
    pub fn base(self) -> Result<ResourceBaseShape> {
        use SlangResourceShape as S;
        Ok(
            match SlangResourceShape(self.0 & S::SLANG_RESOURCE_BASE_SHAPE_MASK.0) {
                S::SLANG_RESOURCE_NONE => ResourceBaseShape::None,
                S::SLANG_TEXTURE_1D => ResourceBaseShape::Texture1d,
                S::SLANG_TEXTURE_2D => ResourceBaseShape::Texture2d,
                S::SLANG_TEXTURE_3D => ResourceBaseShape::Texture3d,
                S::SLANG_TEXTURE_CUBE => ResourceBaseShape::TextureCube,
                S::SLANG_TEXTURE_BUFFER => ResourceBaseShape::TextureBuffer,
                S::SLANG_STRUCTURED_BUFFER => ResourceBaseShape::StructuredBuffer,
                S::SLANG_BYTE_ADDRESS_BUFFER => ResourceBaseShape::ByteAddressBuffer,
                S::SLANG_RESOURCE_UNKNOWN => ResourceBaseShape::Unknown,
                S::SLANG_ACCELERATION_STRUCTURE => ResourceBaseShape::AccelerationStructure,
                S::SLANG_TEXTURE_SUBPASS => ResourceBaseShape::TextureSubpass,
                SlangResourceShape(other) => {
                    return Err(unrecognized("base resource shape", other));
                }
            },
        )
    }

    /// Returns whether the resource is a feedback texture, Slang's flag
    /// `SLANG_TEXTURE_FEEDBACK_FLAG`.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`](super::Type).
    #[must_use]
    pub fn is_feedback(self) -> bool {
        self.has(SlangResourceShape::SLANG_TEXTURE_FEEDBACK_FLAG)
    }

    /// Returns whether the resource is a shadow texture, which a shader
    /// compares against a value as it reads it, Slang's flag
    /// `SLANG_TEXTURE_SHADOW_FLAG`.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`](super::Type).
    #[must_use]
    pub fn is_shadow(self) -> bool {
        self.has(SlangResourceShape::SLANG_TEXTURE_SHADOW_FLAG)
    }

    /// Returns whether the resource is an array of textures, Slang's flag
    /// `SLANG_TEXTURE_ARRAY_FLAG`.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`](super::Type).
    #[must_use]
    pub fn is_array(self) -> bool {
        self.has(SlangResourceShape::SLANG_TEXTURE_ARRAY_FLAG)
    }

    /// Returns whether the resource is a multisampled texture, which holds
    /// several samples for each pixel, Slang's flag
    /// `SLANG_TEXTURE_MULTISAMPLE_FLAG`.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`](super::Type).
    #[must_use]
    pub fn is_multisample(self) -> bool {
        self.has(SlangResourceShape::SLANG_TEXTURE_MULTISAMPLE_FLAG)
    }

    /// Returns whether the resource is a texture combined with its sampler,
    /// Slang's flag `SLANG_TEXTURE_COMBINED_FLAG`.
    ///
    /// # Examples
    ///
    /// See the example of [`Type`](super::Type).
    #[must_use]
    pub fn is_combined(self) -> bool {
        self.has(SlangResourceShape::SLANG_TEXTURE_COMBINED_FLAG)
    }
}

/// A size, a count, or an offset that Slang reports, which can be unbounded
/// or unknown instead of a number.
///
/// Slang reports an unbounded size for a part of a layout that has no fixed
/// size, such as an array whose length the shader does not declare, and an
/// unknown size where the size depends on something that is not known yet,
/// such as a value that linking fills in. Slang marks both with numbers near
/// the largest integer, which this type keeps apart from real sizes so that a
/// program does not allocate or index by them. The enumeration is
/// non-exhaustive, so a later version can add cases, and a `match` on it
/// needs a wildcard arm.
///
/// # Examples
///
/// This example reads a size as a number of bytes when it is finite:
///
/// ```
/// use slang_compiler::reflection::LayoutSize;
///
/// fn bytes(size: LayoutSize) -> Option<u64> {
///     match size {
///         LayoutSize::Finite(bytes) => Some(bytes),
///         _ => None,
///     }
/// }
/// assert_eq!(bytes(LayoutSize::Finite(16)), Some(16));
/// assert_eq!(bytes(LayoutSize::Unbounded), None);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LayoutSize {
    /// A finite number.
    Finite(u64),
    /// No bound, Slang's `SLANG_UNBOUNDED_SIZE`.
    Unbounded,
    /// A number that Slang does not know, Slang's `SLANG_UNKNOWN_SIZE`.
    Unknown,
}

impl LayoutSize {
    /// Converts a size that Slang returned as `size_t` (SLC-13).
    ///
    /// Every supported platform has a `usize` of 64 bits, so every other
    /// value is finite. Were a value not to fit `u64`, it would be
    /// `Unknown`, because the crate could not say what it is.
    pub(crate) fn from_raw(raw: usize) -> Self {
        match raw {
            SLANG_UNBOUNDED_SIZE => Self::Unbounded,
            SLANG_UNKNOWN_SIZE => Self::Unknown,
            finite => u64::try_from(finite).map_or(Self::Unknown, Self::Finite),
        }
    }

    /// Converts a size that Slang returned as a signed integer, which holds
    /// the sentinels of `size_t` as -1 and -2, and reports any other
    /// negative value as an error of kind `Internal` that names `what`.
    pub(crate) fn from_signed(raw: SlangInt, what: &str) -> Result<Self> {
        if raw >= 0 {
            return Ok(Self::from_raw(raw.cast_unsigned()));
        }
        match raw.cast_unsigned() {
            SLANG_UNBOUNDED_SIZE => Ok(Self::Unbounded),
            SLANG_UNKNOWN_SIZE => Ok(Self::Unknown),
            _ => Err(Error::new(ErrorKind::Internal, Operation::Reflect)
                .with_detail(format!("Slang reported {what} {raw}"))),
        }
    }
}

/// A size, a stride, an alignment, or an offset of a layout, with the
/// parameter category in whose units Slang counts it.
///
/// A layout counts each kind of resource separately: uniform data in bytes,
/// and bindings, such as the descriptor slots of Vulkan, in whole slots. The
/// parameter category says which kind the amount counts, so that a number of
/// bindings is not read as a number of bytes. The struct is non-exhaustive,
/// so a later version can add fields, and only this crate creates it.
///
/// # Examples
///
/// See the example of [`TypeLayout`](super::TypeLayout).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct LayoutUnits {
    /// The parameter category whose units the amount counts.
    pub category: ParameterCategory,
    /// The amount, in the units of the category.
    pub amount: LayoutSize,
}

impl LayoutUnits {
    pub(crate) fn new(category: ParameterCategory, amount: LayoutSize) -> Self {
        Self { category, amount }
    }
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::{
        SLANG_UNBOUNDED_SIZE, SLANG_UNKNOWN_SIZE, SlangResourceAccess, SlangResourceShape,
        SlangScalarType, SlangTypeKind,
    };

    use super::{
        LayoutSize, LayoutUnits, ResourceAccess, ResourceBaseShape, ResourceShape, ScalarType,
        TypeKind,
    };
    use crate::{ErrorKind, ParameterCategory};

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn the_value_types_of_reflection_are_send_and_sync() {
        assert_send_sync::<TypeKind>();
        assert_send_sync::<ScalarType>();
        assert_send_sync::<ResourceAccess>();
        assert_send_sync::<ResourceBaseShape>();
        assert_send_sync::<ResourceShape>();
        assert_send_sync::<LayoutSize>();
        assert_send_sync::<LayoutUnits>();
    }

    #[test]
    fn every_type_kind_below_the_sentinel_converts_to_its_variant() {
        let pairs = [
            (SlangTypeKind::SLANG_TYPE_KIND_NONE, TypeKind::None),
            (SlangTypeKind::SLANG_TYPE_KIND_STRUCT, TypeKind::Struct),
            (SlangTypeKind::SLANG_TYPE_KIND_ARRAY, TypeKind::Array),
            (SlangTypeKind::SLANG_TYPE_KIND_MATRIX, TypeKind::Matrix),
            (SlangTypeKind::SLANG_TYPE_KIND_VECTOR, TypeKind::Vector),
            (SlangTypeKind::SLANG_TYPE_KIND_SCALAR, TypeKind::Scalar),
            (
                SlangTypeKind::SLANG_TYPE_KIND_CONSTANT_BUFFER,
                TypeKind::ConstantBuffer,
            ),
            (SlangTypeKind::SLANG_TYPE_KIND_RESOURCE, TypeKind::Resource),
            (
                SlangTypeKind::SLANG_TYPE_KIND_SAMPLER_STATE,
                TypeKind::SamplerState,
            ),
            (
                SlangTypeKind::SLANG_TYPE_KIND_TEXTURE_BUFFER,
                TypeKind::TextureBuffer,
            ),
            (
                SlangTypeKind::SLANG_TYPE_KIND_SHADER_STORAGE_BUFFER,
                TypeKind::ShaderStorageBuffer,
            ),
            (
                SlangTypeKind::SLANG_TYPE_KIND_PARAMETER_BLOCK,
                TypeKind::ParameterBlock,
            ),
            (
                SlangTypeKind::SLANG_TYPE_KIND_GENERIC_TYPE_PARAMETER,
                TypeKind::GenericTypeParameter,
            ),
            (
                SlangTypeKind::SLANG_TYPE_KIND_INTERFACE,
                TypeKind::Interface,
            ),
            (
                SlangTypeKind::SLANG_TYPE_KIND_OUTPUT_STREAM,
                TypeKind::OutputStream,
            ),
            (
                SlangTypeKind::SLANG_TYPE_KIND_MESH_OUTPUT,
                TypeKind::MeshOutput,
            ),
            (
                SlangTypeKind::SLANG_TYPE_KIND_SPECIALIZED,
                TypeKind::Specialized,
            ),
            (SlangTypeKind::SLANG_TYPE_KIND_FEEDBACK, TypeKind::Feedback),
            (SlangTypeKind::SLANG_TYPE_KIND_POINTER, TypeKind::Pointer),
            (
                SlangTypeKind::SLANG_TYPE_KIND_DYNAMIC_RESOURCE,
                TypeKind::DynamicResource,
            ),
            (SlangTypeKind::SLANG_TYPE_KIND_ENUM, TypeKind::Enum),
        ];
        for (raw, kind) in pairs {
            assert_eq!(TypeKind::from_raw(raw).ok(), Some(kind), "{raw:?}");
        }
        assert!((0..SlangTypeKind::SLANG_TYPE_KIND_COUNT.0).eq(pairs.iter().map(|(raw, _)| raw.0)));
        for value in [SlangTypeKind::SLANG_TYPE_KIND_COUNT.0, 1000] {
            let error = TypeKind::from_raw(SlangTypeKind(value)).expect_err("not a kind");
            assert_eq!(error.kind(), ErrorKind::Unrecognized);
        }
    }

    #[test]
    fn every_scalar_type_converts_to_its_variant() {
        let pairs = [
            (SlangScalarType::SLANG_SCALAR_TYPE_NONE, ScalarType::None),
            (SlangScalarType::SLANG_SCALAR_TYPE_VOID, ScalarType::Void),
            (SlangScalarType::SLANG_SCALAR_TYPE_BOOL, ScalarType::Bool),
            (SlangScalarType::SLANG_SCALAR_TYPE_INT32, ScalarType::Int32),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_UINT32,
                ScalarType::Uint32,
            ),
            (SlangScalarType::SLANG_SCALAR_TYPE_INT64, ScalarType::Int64),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_UINT64,
                ScalarType::Uint64,
            ),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_FLOAT16,
                ScalarType::Float16,
            ),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_FLOAT32,
                ScalarType::Float32,
            ),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_FLOAT64,
                ScalarType::Float64,
            ),
            (SlangScalarType::SLANG_SCALAR_TYPE_INT8, ScalarType::Int8),
            (SlangScalarType::SLANG_SCALAR_TYPE_UINT8, ScalarType::Uint8),
            (SlangScalarType::SLANG_SCALAR_TYPE_INT16, ScalarType::Int16),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_UINT16,
                ScalarType::Uint16,
            ),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_INTPTR,
                ScalarType::Intptr,
            ),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_UINTPTR,
                ScalarType::Uintptr,
            ),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_BFLOAT16,
                ScalarType::Bfloat16,
            ),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_FLOAT_E4M3,
                ScalarType::FloatE4m3,
            ),
            (
                SlangScalarType::SLANG_SCALAR_TYPE_FLOAT_E5M2,
                ScalarType::FloatE5m2,
            ),
        ];
        for (raw, scalar) in pairs {
            assert_eq!(ScalarType::from_raw(raw).ok(), Some(scalar), "{raw:?}");
        }
        assert!(
            (0..=SlangScalarType::SLANG_SCALAR_TYPE_FLOAT_E5M2.0)
                .eq(pairs.iter().map(|(raw, _)| raw.0))
        );
        let error = ScalarType::from_raw(SlangScalarType(19)).expect_err("not a scalar type");
        assert_eq!(error.kind(), ErrorKind::Unrecognized);
    }

    #[test]
    fn every_resource_access_converts_to_its_variant() {
        let pairs = [
            (
                SlangResourceAccess::SLANG_RESOURCE_ACCESS_NONE,
                ResourceAccess::None,
            ),
            (
                SlangResourceAccess::SLANG_RESOURCE_ACCESS_READ,
                ResourceAccess::Read,
            ),
            (
                SlangResourceAccess::SLANG_RESOURCE_ACCESS_READ_WRITE,
                ResourceAccess::ReadWrite,
            ),
            (
                SlangResourceAccess::SLANG_RESOURCE_ACCESS_RASTER_ORDERED,
                ResourceAccess::RasterOrdered,
            ),
            (
                SlangResourceAccess::SLANG_RESOURCE_ACCESS_APPEND,
                ResourceAccess::Append,
            ),
            (
                SlangResourceAccess::SLANG_RESOURCE_ACCESS_CONSUME,
                ResourceAccess::Consume,
            ),
            (
                SlangResourceAccess::SLANG_RESOURCE_ACCESS_WRITE,
                ResourceAccess::Write,
            ),
            (
                SlangResourceAccess::SLANG_RESOURCE_ACCESS_FEEDBACK,
                ResourceAccess::Feedback,
            ),
            (
                SlangResourceAccess::SLANG_RESOURCE_ACCESS_UNKNOWN,
                ResourceAccess::Unknown,
            ),
        ];
        for (raw, access) in pairs {
            assert_eq!(ResourceAccess::from_raw(raw).ok(), Some(access), "{raw:?}");
        }
        let named = (0..=SlangResourceAccess::SLANG_RESOURCE_ACCESS_FEEDBACK.0)
            .chain([SlangResourceAccess::SLANG_RESOURCE_ACCESS_UNKNOWN.0]);
        assert!(named.eq(pairs.iter().map(|(raw, _)| raw.0)));
        let error = ResourceAccess::from_raw(SlangResourceAccess(8)).expect_err("not listed");
        assert_eq!(error.kind(), ErrorKind::Unrecognized);
    }

    #[test]
    fn a_resource_shape_splits_into_its_base_shape_and_flags() {
        let shape = ResourceShape::from_raw(SlangResourceShape::SLANG_TEXTURE_2D_MULTISAMPLE_ARRAY);
        assert_eq!(shape.base().ok(), Some(ResourceBaseShape::Texture2d));
        assert!(shape.is_array());
        assert!(shape.is_multisample());
        assert!(!shape.is_shadow());
        assert!(!shape.is_feedback());
        assert!(!shape.is_combined());
        let combined = ResourceShape::from_raw(SlangResourceShape(
            SlangResourceShape::SLANG_TEXTURE_CUBE.0
                | SlangResourceShape::SLANG_TEXTURE_SHADOW_FLAG.0
                | SlangResourceShape::SLANG_TEXTURE_COMBINED_FLAG.0
                | SlangResourceShape::SLANG_TEXTURE_FEEDBACK_FLAG.0,
        ));
        assert_eq!(combined.base().ok(), Some(ResourceBaseShape::TextureCube));
        assert!(combined.is_shadow() && combined.is_combined() && combined.is_feedback());
        let subpass =
            ResourceShape::from_raw(SlangResourceShape::SLANG_TEXTURE_SUBPASS_MULTISAMPLE);
        assert_eq!(subpass.base().ok(), Some(ResourceBaseShape::TextureSubpass));
        assert!(subpass.is_multisample());
    }

    #[test]
    fn every_base_shape_converts_and_an_unlisted_one_is_unrecognized() {
        let mut seen = Vec::new();
        for value in 0..=SlangResourceShape::SLANG_TEXTURE_SUBPASS.0 {
            let base = ResourceShape::from_raw(SlangResourceShape(value))
                .base()
                .expect("a listed shape");
            assert!(!seen.contains(&base), "{base:?} twice");
            seen.push(base);
        }
        for value in 11..=15 {
            let error = ResourceShape::from_raw(SlangResourceShape(value))
                .base()
                .expect_err("not a base shape");
            assert_eq!(error.kind(), ErrorKind::Unrecognized);
        }
    }

    #[test]
    fn layout_sizes_keep_the_sentinels_apart_from_numbers() {
        assert_eq!(LayoutSize::from_raw(0), LayoutSize::Finite(0));
        assert_eq!(LayoutSize::from_raw(12), LayoutSize::Finite(12));
        assert_eq!(
            LayoutSize::from_raw(SLANG_UNBOUNDED_SIZE),
            LayoutSize::Unbounded
        );
        assert_eq!(
            LayoutSize::from_raw(SLANG_UNKNOWN_SIZE),
            LayoutSize::Unknown
        );
        assert_eq!(
            LayoutSize::from_raw(SLANG_UNKNOWN_SIZE - 1),
            LayoutSize::Finite(18_446_744_073_709_551_613)
        );
    }

    #[test]
    fn signed_layout_sizes_read_the_sentinels_and_reject_other_negatives() {
        assert_eq!(
            LayoutSize::from_signed(4, "the count").ok(),
            Some(LayoutSize::Finite(4))
        );
        assert_eq!(
            LayoutSize::from_signed(-1, "the count").ok(),
            Some(LayoutSize::Unbounded)
        );
        assert_eq!(
            LayoutSize::from_signed(-2, "the count").ok(),
            Some(LayoutSize::Unknown)
        );
        let error = LayoutSize::from_signed(-3, "the binding count").expect_err("not a size");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(
            error.to_string(),
            "cannot read reflection: Slang reported the binding count -3"
        );
    }

    #[test]
    fn layout_units_pair_a_category_with_an_amount() {
        let units = LayoutUnits::new(ParameterCategory::Uniform, LayoutSize::Finite(16));
        assert_eq!(units.category, ParameterCategory::Uniform);
        assert_eq!(units.amount, LayoutSize::Finite(16));
        assert_ne!(
            units,
            LayoutUnits::new(
                ParameterCategory::DescriptorTableSlot,
                LayoutSize::Finite(16)
            )
        );
    }
}
