use crate::__private::{StructElement, StructField, StructFieldType, slang_layout_type};
use crate::error::{Error, ErrorKind, Result};
use crate::layout::{
    ElementType, FieldType, PlainValue, Scalar, Shape, Structure, slang_scalar_name,
};

const SET: &str = "set a plain value";

/// The size in bytes of every scalar that a plain value or a buffer element
/// holds, a `bool`, `int`, `uint`, or `float` of Slang.
const SCALAR_SIZE: u64 = 4;

mod sealed {
    use crate::__private::StructField;
    use crate::layout::Scalar;

    /// The Slang type of a Rust value.
    #[expect(
        unnameable_types,
        reason = "it appears only in the methods of the supertrait that seals `Value`"
    )]
    #[expect(
        private_interfaces,
        reason = "no other crate can name the enum, so the crate-private scalar type of its variant reaches none"
    )]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum ValueType {
        /// A scalar or an array of fixed length, with its scalar type and the
        /// lengths of its nested arrays, from the outermost to the innermost.
        Plain { scalar: Scalar, dims: Vec<usize> },
        /// A generated struct, with the Slang name of its structure and the
        /// offset of each of its fields, which only a buffer can hold.
        Structure {
            name: &'static str,
            fields: &'static [StructField],
        },
    }

    #[expect(
        unnameable_types,
        reason = "the supertrait that seals `Value` is private so that no other crate implements it"
    )]
    pub trait SealedValue {
        fn value_type() -> ValueType;

        fn write_scalars(&self, out: &mut Vec<[u8; 4]>);
    }

    #[expect(
        unnameable_types,
        reason = "the supertrait that seals `VertexElement` is private so that no other crate implements it"
    )]
    pub trait SealedVertexElement: SealedValue + Sized {
        const STRIDE: usize;

        fn write_bytes(&self, out: &mut Vec<u8>);

        fn read_bytes(bytes: &[u8]) -> Option<Self>;
    }

    #[expect(
        unnameable_types,
        reason = "the supertrait that seals `Element` is private so that no other crate implements it"
    )]
    pub trait SealedElement {}
}

pub(crate) use sealed::ValueType;

/// A Rust value that a kernel or a draw can receive as a plain value of
/// Slang: a scalar, a vector, a matrix, or an array of fixed length.
///
/// The scalars are [`bool`], [`i32`], [`u32`], and [`f32`], which stand for
/// the Slang types `bool`, `int`, `uint`, and `float`. An array of length `N`
/// of a value, `[T; N]`, stands for each Slang type of that structure:
///
/// - `[f32; 3]` is a vector `float3` or an array `float[3]`.
/// - `[[f32; 3]; 2]` is a matrix `float2x3` of two rows of three columns,
///   an array of vectors `float3[2]`, or an array of arrays `float[2][3]`.
///   The first index of a matrix selects its row, as `m[row][column]` does
///   in Slang.
///
/// A value is checked against the Slang type of its parameter before any
/// byte of it is written, and its scalars are written at the offsets that
/// Slang's layout gives them, so the layout of the Rust type does not
/// matter.
///
/// This trait is sealed, so code outside this crate cannot implement it. The
/// code that `slango-macros` generates implements it for its structs through
/// a hidden trait that is not part of the API. It is used only as a generic
/// bound, not as a trait object.
///
/// # Examples
///
/// This example passes a rotation as the parameter `rotation` of type
/// `float3x3`, which a nested array of three rows of three `f32` values
/// stands for:
///
/// ```
/// use slango::{Kernel, Value};
///
/// fn dispatch_with<V: Value>(kernel: &Kernel, path: &str, value: V) -> slango::Result<()> {
///     kernel.call().set(path, value).dispatch([1, 1, 1])
/// }
///
/// fn rotate(kernel: &Kernel) -> slango::Result<()> {
///     let rotation = [[0.0_f32, -1.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 1.0]];
///     dispatch_with(kernel, "rotation", rotation)
/// }
/// # let _ = rotate;
/// ```
pub trait Value: sealed::SealedValue + Copy {}

/// A Rust type that a [`Buffer`](crate::Buffer) holds as its element, whose
/// bytes are the packed little-endian scalars of the matching Slang type.
///
/// The elements are [`f32`], [`i32`], and [`u32`], which stand for `float`,
/// `int`, and `uint`, and arrays of 2, 3, or 4 of them, such as `[f32; 3]`,
/// which stand for vectors such as `float3`. Every [`Element`] is a vertex
/// element. The arrays of 2 and 3 scalars are not, so a buffer of them, whose
/// elements lie packed 8 or 12 bytes apart as a vertex attribute reads them,
/// cannot be bound to a structured buffer.
///
/// This trait is sealed, so code outside this crate cannot implement it. The
/// code that `slango-macros` generates implements it for its structs through
/// a hidden trait that is not part of the API. It is used only as a generic
/// bound, not as a trait object.
///
/// # Examples
///
/// See the example of [`RenderPipeline::draw`](crate::RenderPipeline::draw),
/// which binds buffers of `[f32; 2]` to vertex attributes of type `float2`.
pub trait VertexElement: sealed::SealedVertexElement + Copy + Send + Sync + 'static {}

/// A Rust type that a [`Buffer`](crate::Buffer) bound to a structured buffer
/// holds as its element, whose bytes equal the layout of the matching Slang
/// element type.
///
/// The elements are [`f32`], [`i32`], and [`u32`], which stand for `float`,
/// `int`, and `uint`, and `[f32; 4]`, `[i32; 4]`, and `[u32; 4]`, which
/// stand for `float4`, `int4`, and `uint4`. Each is also a
/// [`VertexElement`].
///
/// This trait is sealed, so code outside this crate cannot implement it. The
/// code that `slango-macros` generates implements it for its structs through
/// a hidden trait that is not part of the API. It is used only as a generic
/// bound, not as a trait object.
///
/// # Examples
///
/// See the example of [`Buffer::from_slice`](crate::Buffer::from_slice).
pub trait Element: VertexElement + sealed::SealedElement {}

impl sealed::SealedValue for bool {
    fn value_type() -> ValueType {
        scalar_type(Scalar::Bool)
    }

    fn write_scalars(&self, out: &mut Vec<[u8; 4]>) {
        out.push(u32::from(*self).to_le_bytes());
    }
}

impl Value for bool {}

impl sealed::SealedValue for i32 {
    fn value_type() -> ValueType {
        scalar_type(Scalar::Int32)
    }

    fn write_scalars(&self, out: &mut Vec<[u8; 4]>) {
        out.push(self.to_le_bytes());
    }
}

impl Value for i32 {}

impl sealed::SealedValue for u32 {
    fn value_type() -> ValueType {
        scalar_type(Scalar::Uint32)
    }

    fn write_scalars(&self, out: &mut Vec<[u8; 4]>) {
        out.push(self.to_le_bytes());
    }
}

impl Value for u32 {}

impl sealed::SealedValue for f32 {
    fn value_type() -> ValueType {
        scalar_type(Scalar::Float32)
    }

    fn write_scalars(&self, out: &mut Vec<[u8; 4]>) {
        out.push(self.to_le_bytes());
    }
}

impl Value for f32 {}

impl<T: Value, const N: usize> sealed::SealedValue for [T; N] {
    fn value_type() -> ValueType {
        match T::value_type() {
            ValueType::Plain {
                scalar,
                dims: element,
            } => {
                let mut dims = Vec::with_capacity(element.len().saturating_add(1));
                dims.push(N);
                dims.extend(element);
                ValueType::Plain { scalar, dims }
            }
            structure @ ValueType::Structure { .. } => structure,
        }
    }

    fn write_scalars(&self, out: &mut Vec<[u8; 4]>) {
        for element in self {
            element.write_scalars(out);
        }
    }
}

impl<T: Value, const N: usize> Value for [T; N] {}

impl sealed::SealedVertexElement for i32 {
    const STRIDE: usize = 4;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        Some(Self::from_le_bytes(bytes.try_into().ok()?))
    }
}

impl VertexElement for i32 {}

impl sealed::SealedElement for i32 {}

impl Element for i32 {}

impl sealed::SealedVertexElement for u32 {
    const STRIDE: usize = 4;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        Some(Self::from_le_bytes(bytes.try_into().ok()?))
    }
}

impl VertexElement for u32 {}

impl sealed::SealedElement for u32 {}

impl Element for u32 {}

impl sealed::SealedVertexElement for f32 {
    const STRIDE: usize = 4;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.to_le_bytes());
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        Some(Self::from_le_bytes(bytes.try_into().ok()?))
    }
}

impl VertexElement for f32 {}

impl sealed::SealedElement for f32 {}

impl Element for f32 {}

impl sealed::SealedVertexElement for [f32; 2] {
    const STRIDE: usize = 8;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        write_vector(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        read_vector(bytes)
    }
}

impl VertexElement for [f32; 2] {}

impl sealed::SealedVertexElement for [i32; 2] {
    const STRIDE: usize = 8;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        write_vector(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        read_vector(bytes)
    }
}

impl VertexElement for [i32; 2] {}

impl sealed::SealedVertexElement for [u32; 2] {
    const STRIDE: usize = 8;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        write_vector(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        read_vector(bytes)
    }
}

impl VertexElement for [u32; 2] {}

impl sealed::SealedVertexElement for [f32; 3] {
    const STRIDE: usize = 12;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        write_vector(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        read_vector(bytes)
    }
}

impl VertexElement for [f32; 3] {}

impl sealed::SealedVertexElement for [i32; 3] {
    const STRIDE: usize = 12;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        write_vector(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        read_vector(bytes)
    }
}

impl VertexElement for [i32; 3] {}

impl sealed::SealedVertexElement for [u32; 3] {
    const STRIDE: usize = 12;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        write_vector(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        read_vector(bytes)
    }
}

impl VertexElement for [u32; 3] {}

impl sealed::SealedVertexElement for [f32; 4] {
    const STRIDE: usize = 16;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        write_vector(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        read_vector(bytes)
    }
}

impl VertexElement for [f32; 4] {}

impl sealed::SealedElement for [f32; 4] {}

impl Element for [f32; 4] {}

impl sealed::SealedVertexElement for [i32; 4] {
    const STRIDE: usize = 16;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        write_vector(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        read_vector(bytes)
    }
}

impl VertexElement for [i32; 4] {}

impl sealed::SealedElement for [i32; 4] {}

impl Element for [i32; 4] {}

impl sealed::SealedVertexElement for [u32; 4] {
    const STRIDE: usize = 16;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        write_vector(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        read_vector(bytes)
    }
}

impl VertexElement for [u32; 4] {}

impl sealed::SealedElement for [u32; 4] {}

impl Element for [u32; 4] {}

impl<T: StructElement> sealed::SealedValue for T {
    fn value_type() -> ValueType {
        ValueType::Structure {
            name: T::SLANG_NAME,
            fields: T::FIELDS,
        }
    }

    fn write_scalars(&self, _out: &mut Vec<[u8; 4]>) {}
}

impl<T: StructElement> Value for T {}

impl<T: StructElement> sealed::SealedVertexElement for T {
    const STRIDE: usize = <T as StructElement>::STRIDE;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        StructElement::write_bytes(self, out);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        <T as StructElement>::read_bytes(bytes)
    }
}

impl<T: StructElement> VertexElement for T {}

impl<T: StructElement> sealed::SealedElement for T {}

impl<T: StructElement> Element for T {}

fn write_vector<S: sealed::SealedVertexElement, const N: usize>(
    vector: &[S; N],
    out: &mut Vec<u8>,
) {
    for scalar in vector {
        scalar.write_bytes(out);
    }
}

fn read_vector<S: sealed::SealedVertexElement + Copy + Default, const N: usize>(
    bytes: &[u8],
) -> Option<[S; N]> {
    if bytes.len() != N.checked_mul(S::STRIDE)? {
        return None;
    }
    let mut vector = [S::default(); N];
    for (slot, chunk) in vector.iter_mut().zip(bytes.chunks_exact(S::STRIDE)) {
        *slot = S::read_bytes(chunk)?;
    }
    Some(vector)
}

fn scalar_type(scalar: Scalar) -> ValueType {
    ValueType::Plain {
        scalar,
        dims: Vec::new(),
    }
}

/// A value converted for a kernel: its Slang type and its scalars as bytes,
/// in the order of its indexes, the last index varying fastest.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Marshalled {
    pub(crate) ty: ValueType,
    pub(crate) scalars: Vec<[u8; 4]>,
}

pub(crate) fn marshal<V: Value>(value: &V) -> Marshalled {
    let mut scalars = Vec::new();
    value.write_scalars(&mut scalars);
    Marshalled {
        ty: V::value_type(),
        scalars,
    }
}

/// Checks that a value of the Rust type `ty` matches the reflected plain
/// value at `path`, and fails with [`ErrorKind::InvalidArgument`] when its
/// scalar type or its structure differs.
pub(crate) fn check(path: &str, ty: &ValueType, plain: &PlainValue) -> Result<()> {
    if let ValueType::Plain { scalar, dims } = ty
        && *scalar == plain.scalar
        && shape_matches(&plain.shape, dims)
    {
        return Ok(());
    }
    Err(
        Error::new(ErrorKind::InvalidArgument, SET).with_detail(format!(
            "the parameter `{path}` has the Slang type `{}`, and the value has the Rust type `{}`",
            slang_type_name(plain.scalar, &plain.shape),
            rust_type_name(ty),
        )),
    )
}

/// Checks a value against the reflected plain value at `path` and writes
/// its scalars into `block`, the bytes of the uniform buffer or immediates
/// that hold the plain value, at their reflected offsets.
///
/// No byte is written unless the value matches and every scalar falls
/// inside both the reflected size of the plain value and `block`.
pub(crate) fn write(
    path: &str,
    value: &Marshalled,
    plain: &PlainValue,
    block: &mut [u8],
) -> Result<()> {
    check(path, &value.ty, plain)?;
    let mut offsets = Vec::with_capacity(value.scalars.len());
    let fits = scalar_offsets(&plain.shape, 0, &mut offsets).is_some()
        && offsets.iter().all(|offset| {
            offset
                .checked_add(SCALAR_SIZE)
                .is_some_and(|end| end <= plain.size)
        });
    if !fits {
        return Err(outside(path, &format!("its {} bytes", plain.size)));
    }
    let ranges = offsets
        .iter()
        .map(|offset| {
            let start = usize::try_from(plain.offset.checked_add(*offset)?).ok()?;
            let end = start.checked_add(4)?;
            (end <= block.len()).then_some(start..end)
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| outside(path, &format!("its block of {} bytes", block.len())))?;
    for (range, scalar) in ranges.into_iter().zip(&value.scalars) {
        if let Some(slot) = block.get_mut(range) {
            slot.copy_from_slice(scalar);
        }
    }
    Ok(())
}

fn outside(path: &str, what: &str) -> Error {
    Error::new(ErrorKind::UnsupportedParameter, SET).with_detail(format!(
        "the reflected layout of the parameter `{path}` places a scalar outside {what}"
    ))
}

/// Returns `None` when a buffer whose elements have the Rust type `ty` may
/// be bound to a structured buffer whose elements have the type `element`,
/// and otherwise the Slang name of `element`, such as `float4`.
///
/// A scalar or a vector of `bool`, `int`, `uint`, or `float` matches only
/// the Rust type of the same scalar type and number of components. A scalar
/// or a vector of another scalar type, such as `half2`, matches no Rust
/// type. A structure or any other element type matches every Rust type here,
/// because the check of the stride covers it, and [`field_mismatch`] then
/// checks a generated struct field by field.
pub(crate) fn element_mismatch(ty: &ValueType, element: &ElementType) -> Option<String> {
    match element {
        ElementType::Plain { scalar, components } => {
            let shape = if *components == 1 {
                Shape::Scalar
            } else {
                Shape::Vector {
                    length: *components,
                }
            };
            let same = match ty {
                ValueType::Plain {
                    scalar: rust_scalar,
                    dims,
                } => rust_scalar == scalar && shape_matches(&shape, dims),
                ValueType::Structure { .. } => false,
            };
            (!same).then(|| slang_type_name(*scalar, &shape))
        }
        ElementType::OtherPlain { name } => Some(name.clone()),
        ElementType::Structure(_) | ElementType::Composite => None,
    }
}

/// Returns how the fields of the generated struct `name`, whose fields are
/// `fields`, differ from those of `element`, the element type of a
/// parameter, or `None` when a buffer of the struct may be bound to it.
///
/// An element that is not a structure differs from every struct. Otherwise
/// the difference is the first field of either, in the order of the offsets,
/// whose path, offset, or type the other does not have. A plain field
/// matches only a field of the same Slang type, which has the same scalar
/// type, number of components, and array lengths, and a structure matches a
/// structure of any name, whose own fields follow it.
pub(crate) fn field_mismatch(
    name: &str,
    fields: &[StructField],
    element: &ElementType,
) -> Option<String> {
    let ElementType::Structure(structure) = element else {
        return Some(format!(
            "has elements that are not a structure, and the bound buffer has elements of the Rust struct `{name}`"
        ));
    };
    let mut reflected = Vec::new();
    flatten(structure, "", 0, &mut reflected);
    let slang = &structure.name;
    let count = reflected.len().max(fields.len());
    (0..count).find_map(|index| match (reflected.get(index), fields.get(index)) {
        (Some(field), Some(given)) if given.name == field.path => {
            if !u64::try_from(given.offset).is_ok_and(|offset| offset == field.offset) {
                return Some(format!(
                    "has elements of the structure `{slang}` with the field `{}` at byte {}, and the bound buffer has elements of the Rust struct `{name}` with that field at byte {}",
                    field.path, field.offset, given.offset
                ));
            }
            let same = match (&field.ty, given.ty) {
                (Leaf::Plain(reflected), StructFieldType::Plain(given)) => reflected == given,
                (Leaf::Structure(_), StructFieldType::Structure(_)) => true,
                _ => false,
            };
            (!same).then(|| {
                format!(
                    "has elements of the structure `{slang}` with the field `{}` of {}, and the bound buffer has elements of the Rust struct `{name}` with that field of {}",
                    field.path,
                    field.ty.describe(),
                    describe_given(given.ty)
                )
            })
        }
        (Some(field), _) => Some(format!(
            "has elements of the structure `{slang}` with the field `{}` at byte {}, and the bound buffer has elements of the Rust struct `{name}` without that field there",
            field.path, field.offset
        )),
        (None, Some(given)) => Some(format!(
            "has elements of the structure `{slang}`, and the bound buffer has elements of the Rust struct `{name}` with the field `{}` at byte {}, which the structure lacks",
            given.name, given.offset
        )),
        (None, None) => None,
    })
}

/// A field of a reflected structure, by its path and its offset from the
/// start of the outermost structure.
struct Flat {
    path: String,
    offset: u64,
    ty: Leaf,
}

/// The type of a [`Flat`] field.
enum Leaf {
    /// A plain value, with the name of its Slang type.
    Plain(String),
    /// A structure, with its Slang name.
    Structure(String),
    /// Any other type, which no generated struct holds, with its Slang name.
    Other(String),
}

impl Leaf {
    fn describe(&self) -> String {
        match self {
            Self::Plain(name) | Self::Other(name) => format!("the Slang type `{name}`"),
            Self::Structure(name) => format!("the structure `{name}`"),
        }
    }
}

fn describe_given(ty: StructFieldType) -> String {
    match ty {
        StructFieldType::Plain(name) => format!("the Slang type `{name}`"),
        StructFieldType::Structure(name) => format!("the structure `{name}`"),
    }
}

/// Appends every field of `structure`, which starts at `base` under the path
/// `prefix`, with each field of a nested structure after the field that
/// holds it.
fn flatten(structure: &Structure, prefix: &str, base: u64, out: &mut Vec<Flat>) {
    for field in &structure.fields {
        let path = if prefix.is_empty() {
            field.name.clone()
        } else {
            format!("{prefix}.{}", field.name)
        };
        let offset = base.saturating_add(field.offset);
        let ty = match &field.ty {
            FieldType::Plain { scalar, shape } => Leaf::Plain(slang_layout_type(*scalar, shape)),
            FieldType::Structure(inner) => Leaf::Structure(inner.name.clone()),
            FieldType::Other { name } => Leaf::Other(name.clone()),
        };
        out.push(Flat {
            path: path.clone(),
            offset,
            ty,
        });
        if let FieldType::Structure(inner) = &field.ty {
            flatten(inner, &path, offset, out);
        }
    }
}

fn shape_matches(shape: &Shape, dims: &[usize]) -> bool {
    let same = |length: u64, dim: usize| u64::try_from(dim).is_ok_and(|dim| dim == length);
    match shape {
        Shape::Scalar => dims.is_empty(),
        Shape::Vector { length } => matches!(dims, [n] if same(u64::from(*length), *n)),
        Shape::Matrix { rows, columns, .. } => {
            matches!(dims, [r, c] if same(u64::from(*rows), *r) && same(u64::from(*columns), *c))
        }
        Shape::Array {
            length, element, ..
        } => match dims.split_first() {
            Some((first, rest)) => same(*length, *first) && shape_matches(element, rest),
            None => false,
        },
    }
}

/// Appends the byte offset of each scalar of a value of `shape` that starts
/// at `base`, in the order of its indexes, the last index varying fastest,
/// and returns `None` when an offset overflows.
///
/// A matrix is a sequence of rows, each a vector of its columns, because
/// the session of a load keeps Slang's default row-major layout, in which
/// the rows of a Slang matrix are its contiguous vectors. Its rows are the
/// row stride that reflection reports apart, which depends on the target
/// and the layout: 16 bytes in a uniform buffer of SPIR-V, the size of a row
/// rounded up to 8 or 16 bytes in the immediates, and the size of a row in
/// a scalar layout. The last row is not always padded to the stride, so the
/// stride cannot be derived from the size of the matrix.
fn scalar_offsets(shape: &Shape, base: u64, out: &mut Vec<u64>) -> Option<()> {
    match shape {
        Shape::Scalar => out.push(base),
        Shape::Vector { length } => {
            for index in 0..u64::from(*length) {
                out.push(base.checked_add(index.checked_mul(SCALAR_SIZE)?)?);
            }
        }
        Shape::Matrix {
            rows,
            columns,
            row_stride,
        } => {
            for row in 0..u64::from(*rows) {
                let row_base = base.checked_add(row.checked_mul(*row_stride)?)?;
                for column in 0..u64::from(*columns) {
                    out.push(row_base.checked_add(column.checked_mul(SCALAR_SIZE)?)?);
                }
            }
        }
        Shape::Array {
            length,
            stride,
            element,
        } => {
            for index in 0..*length {
                let element_base = base.checked_add(index.checked_mul(*stride)?)?;
                scalar_offsets(element, element_base, out)?;
            }
        }
    }
    Some(())
}

fn rust_scalar_name(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool => "bool",
        Scalar::Int32 => "i32",
        Scalar::Uint32 => "u32",
        Scalar::Float32 => "f32",
    }
}

/// Returns the Slang name of a type, such as `float3[4]` or `float[2][3]`,
/// with the lengths of nested arrays from the outermost to the innermost.
fn slang_type_name(scalar: Scalar, shape: &Shape) -> String {
    let mut lengths = String::new();
    let mut shape = shape;
    while let Shape::Array {
        length, element, ..
    } = shape
    {
        lengths.push('[');
        lengths.push_str(&length.to_string());
        lengths.push(']');
        shape = element;
    }
    let name = slang_scalar_name(scalar);
    match shape {
        Shape::Scalar | Shape::Array { .. } => format!("{name}{lengths}"),
        Shape::Vector { length } => format!("{name}{length}{lengths}"),
        Shape::Matrix { rows, columns, .. } => format!("{name}{rows}x{columns}{lengths}"),
    }
}

/// Returns the Rust name of a value type, such as `[[f32; 3]; 2]`, or the
/// Slang name of the structure of a generated struct.
pub(crate) fn rust_type_name(ty: &ValueType) -> String {
    match ty {
        ValueType::Plain { scalar, dims } => dims
            .iter()
            .rev()
            .fold(rust_scalar_name(*scalar).to_owned(), |inner, dim| {
                format!("[{inner}; {dim}]")
            }),
        ValueType::Structure { name, .. } => (*name).to_owned(),
    }
}

#[cfg(test)]
mod tests;
