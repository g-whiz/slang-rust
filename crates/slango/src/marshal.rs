use crate::error::{Error, ErrorKind, Result};
use crate::layout::{ElementType, PlainValue, Scalar, Shape};

const SET: &str = "set a plain value";

/// The size in bytes of every scalar that a plain value or a buffer element
/// holds, a `bool`, `int`, `uint`, or `float` of Slang.
const SCALAR_SIZE: u64 = 4;

mod sealed {
    use crate::layout::Scalar;

    /// The Slang type of a Rust value: its scalar type and the lengths of
    /// its nested arrays, from the outermost to the innermost.
    #[expect(
        unnameable_types,
        reason = "it appears only in the methods of the supertrait that seals `Value` (API-8)"
    )]
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ValueType {
        pub(crate) scalar: Scalar,
        pub(crate) dims: Vec<usize>,
    }

    #[expect(
        unnameable_types,
        reason = "the supertrait that seals `Value` is private so that no other crate implements it (API-8)"
    )]
    pub trait SealedValue {
        fn value_type() -> ValueType;

        fn write_scalars(&self, out: &mut Vec<[u8; 4]>);
    }

    #[expect(
        unnameable_types,
        reason = "the supertrait that seals `VertexElement` is private so that no other crate implements it (API-8)"
    )]
    pub trait SealedVertexElement: SealedValue + Sized {
        const STRIDE: usize;

        fn write_bytes(&self, out: &mut Vec<u8>);

        fn read_bytes(bytes: &[u8]) -> Option<Self>;
    }

    #[expect(
        unnameable_types,
        reason = "the supertrait that seals `Element` is private so that no other crate implements it (API-8)"
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
/// This trait is sealed: no type outside this crate can implement it. It
/// serves as a generic bound only, not as a trait object.
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
/// This trait is sealed: no type outside this crate can implement it. It
/// serves as a generic bound only, not as a trait object.
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
/// This trait is sealed: no type outside this crate can implement it. It
/// serves as a generic bound only, not as a trait object.
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
        let element = T::value_type();
        let mut dims = Vec::with_capacity(element.dims.len() + 1);
        dims.push(N);
        dims.extend(element.dims);
        ValueType {
            scalar: element.scalar,
            dims,
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
    ValueType {
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
    if ty.scalar == plain.scalar && shape_matches(&plain.shape, &ty.dims) {
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
    let fits = scalar_offsets(&plain.shape, plain.size, 0, &mut offsets).is_some()
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
/// A scalar or a vector matches only the Rust type of the same scalar type
/// and number of components, and any other element type matches every Rust
/// type, because the check of the stride alone covers it (SLM-5 of
/// `docs/design/slango-macros.md`).
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
            let same = ty.scalar == *scalar && shape_matches(&shape, &ty.dims);
            (!same).then(|| slang_type_name(*scalar, &shape))
        }
        ElementType::OtherPlain { name } => Some(name.clone()),
        ElementType::Composite => None,
    }
}

fn shape_matches(shape: &Shape, dims: &[usize]) -> bool {
    let same = |length: u64, dim: usize| u64::try_from(dim).is_ok_and(|dim| dim == length);
    match shape {
        Shape::Scalar => dims.is_empty(),
        Shape::Vector { length } => matches!(dims, [n] if same(u64::from(*length), *n)),
        Shape::Matrix { rows, columns } => {
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

/// Appends the byte offset of each scalar of a value of `shape` that takes
/// `extent` bytes and starts at `base`, in the order of its indexes, the last
/// index varying fastest, and returns `None` when an offset overflows.
///
/// A matrix is a sequence of rows, each a vector of its columns, because
/// the session of a load keeps Slang's default row-major layout, in which
/// the rows of a Slang matrix are its contiguous vectors. Its rows are
/// `extent / rows` bytes apart, which is 16 bytes in a uniform buffer and
/// the size of a row rounded up to 8 or 16 bytes in the immediates.
fn scalar_offsets(shape: &Shape, extent: u64, base: u64, out: &mut Vec<u64>) -> Option<()> {
    match shape {
        Shape::Scalar => out.push(base),
        Shape::Vector { length } => {
            for index in 0..u64::from(*length) {
                out.push(base.checked_add(index.checked_mul(SCALAR_SIZE)?)?);
            }
        }
        Shape::Matrix { rows, columns } => {
            let row_stride = extent.checked_div(u64::from(*rows))?;
            for row in 0..u64::from(*rows) {
                let row_base = base.checked_add(row.checked_mul(row_stride)?)?;
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
                scalar_offsets(element, *stride, element_base, out)?;
            }
        }
    }
    Some(())
}

fn slang_scalar_name(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool => "bool",
        Scalar::Int32 => "int",
        Scalar::Uint32 => "uint",
        Scalar::Float32 => "float",
    }
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
        Shape::Matrix { rows, columns } => format!("{name}{rows}x{columns}{lengths}"),
    }
}

/// Returns the Rust name of a value type, such as `[[f32; 3]; 2]`.
pub(crate) fn rust_type_name(ty: &ValueType) -> String {
    ty.dims
        .iter()
        .rev()
        .fold(rust_scalar_name(ty.scalar).to_owned(), |inner, dim| {
            format!("[{inner}; {dim}]")
        })
}

#[cfg(test)]
mod tests;
