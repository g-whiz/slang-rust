use crate::error::Location;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Model {
    pub(crate) items: Vec<Item>,
    pub(crate) macros: Vec<Macro>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Item {
    pub(crate) name: Name,
    pub(crate) location: Location,
    pub(crate) owner: Option<String>,
    pub(crate) kind: ItemKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Name {
    pub(crate) cxx: String,
    pub(crate) rust: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum ItemKind {
    Interface(Interface),
    Record(Record),
    Enum(Enum),
    Typedef(Ty),
    Function(Function),
    Constant(Constant),
    Opaque,
    Unsupported(String),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Interface {
    pub(crate) bases: Vec<String>,
    pub(crate) uuid: Option<Guid>,
    pub(crate) methods: Vec<Method>,
    pub(crate) has_fields: bool,
    pub(crate) has_virtual_destructor: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Guid {
    pub(crate) data1: u32,
    pub(crate) data2: u16,
    pub(crate) data3: u16,
    pub(crate) data4: [u8; 8],
}

impl Guid {
    pub(crate) fn from_values(values: &[u64]) -> Option<Self> {
        let [data1, data2, data3, rest @ ..] = values else {
            return None;
        };
        if rest.len() != 8 {
            return None;
        }
        let mut data4 = [0_u8; 8];
        for (byte, value) in data4.iter_mut().zip(rest) {
            *byte = u8::try_from(*value).ok()?;
        }
        Some(Self {
            data1: u32::try_from(*data1).ok()?,
            data2: u16::try_from(*data2).ok()?,
            data3: u16::try_from(*data3).ok()?,
            data4,
        })
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Method {
    pub(crate) name: String,
    pub(crate) location: Location,
    pub(crate) signature: Signature,
    pub(crate) pure: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Signature {
    pub(crate) params: Vec<Param>,
    pub(crate) result: Ty,
    pub(crate) variadic: bool,
    pub(crate) convention: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Param {
    pub(crate) name: Option<String>,
    pub(crate) ty: Ty,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Record {
    pub(crate) union: bool,
    pub(crate) size: usize,
    pub(crate) align: usize,
    pub(crate) fields: Vec<Field>,
    pub(crate) has_bases: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Field {
    pub(crate) name: Option<String>,
    pub(crate) location: Location,
    pub(crate) ty: Ty,
    pub(crate) offset_bits: usize,
    pub(crate) bit_width: Option<usize>,
    pub(crate) init: Init,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Init {
    None,
    Null,
    Zero,
    Expression,
    Value(Value),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Enum {
    pub(crate) unnamed: bool,
    pub(crate) constants_of: Option<String>,
    pub(crate) underlying: Underlying,
    pub(crate) size: usize,
    pub(crate) enumerators: Vec<Enumerator>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Underlying {
    Fixed(Ty),
    Unfixed { canonical_signed: bool },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Enumerator {
    pub(crate) name: String,
    pub(crate) value: i128,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Function {
    pub(crate) signature: Signature,
    pub(crate) c_linkage: bool,
    pub(crate) inline: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Constant {
    pub(crate) ty: Ty,
    pub(crate) value: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Macro {
    pub(crate) name: String,
    pub(crate) location: Location,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Value {
    Integer(i128),
    Float(f64),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Ty {
    Void,
    Bool,
    C(CType),
    Prim(Prim),
    Named(String),
    Pointer { pointee: Box<Ty>, is_const: bool },
    Array { element: Box<Ty>, len: usize },
    Callback(Box<Signature>),
    Unsupported(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CType {
    Char,
    SChar,
    UChar,
    Short,
    UShort,
    Int,
    UInt,
    Long,
    ULong,
    LongLong,
    ULongLong,
    Float,
    Double,
}

impl CType {
    pub(crate) fn rust(self) -> &'static str {
        match self {
            Self::Char => "::core::ffi::c_char",
            Self::SChar => "::core::ffi::c_schar",
            Self::UChar => "::core::ffi::c_uchar",
            Self::Short => "::core::ffi::c_short",
            Self::UShort => "::core::ffi::c_ushort",
            Self::Int => "::core::ffi::c_int",
            Self::UInt => "::core::ffi::c_uint",
            Self::Long => "::core::ffi::c_long",
            Self::ULong => "::core::ffi::c_ulong",
            Self::LongLong => "::core::ffi::c_longlong",
            Self::ULongLong => "::core::ffi::c_ulonglong",
            Self::Float => "f32",
            Self::Double => "f64",
        }
    }

    pub(crate) fn range(self) -> Option<(i128, i128)> {
        match self {
            Self::Char => Some((0, i128::from(i8::MAX))),
            Self::SChar => Some(Prim::I8.range()),
            Self::UChar => Some(Prim::U8.range()),
            Self::Short => Some(Prim::I16.range()),
            Self::UShort => Some(Prim::U16.range()),
            Self::Int | Self::Long => Some(Prim::I32.range()),
            Self::UInt | Self::ULong => Some(Prim::U32.range()),
            Self::LongLong => Some(Prim::I64.range()),
            Self::ULongLong => Some(Prim::U64.range()),
            Self::Float | Self::Double => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Prim {
    I8,
    I16,
    I32,
    I64,
    Isize,
    U8,
    U16,
    U32,
    U64,
    Usize,
}

impl Prim {
    pub(crate) fn rust(self) -> &'static str {
        match self {
            Self::I8 => "i8",
            Self::I16 => "i16",
            Self::I32 => "i32",
            Self::I64 => "i64",
            Self::Isize => "isize",
            Self::U8 => "u8",
            Self::U16 => "u16",
            Self::U32 => "u32",
            Self::U64 => "u64",
            Self::Usize => "usize",
        }
    }

    pub(crate) fn range(self) -> (i128, i128) {
        match self {
            Self::I8 => (i128::from(i8::MIN), i128::from(i8::MAX)),
            Self::I16 => (i128::from(i16::MIN), i128::from(i16::MAX)),
            Self::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
            Self::I64 | Self::Isize => (i128::from(i64::MIN), i128::from(i64::MAX)),
            Self::U8 => (0, i128::from(u8::MAX)),
            Self::U16 => (0, i128::from(u16::MAX)),
            Self::U32 => (0, i128::from(u32::MAX)),
            Self::U64 | Self::Usize => (0, i128::from(u64::MAX)),
        }
    }

    pub(crate) fn from_std_typedef(name: &str) -> Option<Self> {
        match name {
            "int8_t" => Some(Self::I8),
            "int16_t" => Some(Self::I16),
            "int32_t" => Some(Self::I32),
            "int64_t" => Some(Self::I64),
            "intptr_t" | "ptrdiff_t" => Some(Self::Isize),
            "uint8_t" => Some(Self::U8),
            "uint16_t" => Some(Self::U16),
            "uint32_t" => Some(Self::U32),
            "uint64_t" => Some(Self::U64),
            "uintptr_t" | "size_t" => Some(Self::Usize),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_from_values_reads_eleven_values_that_fit_their_fields() {
        let guid = Guid::from_values(&[0x1122_3344, 0x5566, 0x7788, 1, 2, 3, 4, 5, 6, 7, 0xFF]);
        assert_eq!(
            guid,
            Some(Guid {
                data1: 0x1122_3344,
                data2: 0x5566,
                data3: 0x7788,
                data4: [1, 2, 3, 4, 5, 6, 7, 0xFF],
            })
        );
    }

    #[test]
    fn guid_from_values_rejects_a_wrong_count_or_a_value_out_of_range() {
        assert_eq!(Guid::from_values(&[]), None);
        assert_eq!(Guid::from_values(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10]), None);
        assert_eq!(
            Guid::from_values(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]),
            None
        );
        assert_eq!(
            Guid::from_values(&[1 << 32, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
            None
        );
        assert_eq!(
            Guid::from_values(&[1, 1 << 16, 3, 4, 5, 6, 7, 8, 9, 10, 11]),
            None
        );
        assert_eq!(
            Guid::from_values(&[1, 2, 1 << 16, 4, 5, 6, 7, 8, 9, 10, 11]),
            None
        );
        assert_eq!(
            Guid::from_values(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 256]),
            None
        );
    }
}
