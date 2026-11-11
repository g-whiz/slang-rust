use std::path::PathBuf;

use crate::layout::{self, Binding, Compilation, CompiledKernel, KernelLayout, Resource};

/// The description of a compiled Slang module that the generator of
/// `slango-codegen` reads: its kernels and the files that Slang read.
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise (SLM-6), and the tests of the generator build it with a literal"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interface {
    /// The compute entry points of the module, the functions that a dispatch
    /// runs, in the order of the module.
    pub kernels: Vec<KernelInterface>,
    /// The paths of the files that Slang read to compile the module: the
    /// file itself and every file that it imports.
    pub dependencies: Vec<PathBuf>,
}

/// The description of one kernel, a compute entry point of a module.
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise (SLM-6), and the tests of the generator build it with a literal"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KernelInterface {
    /// The name of the entry point in the Slang source.
    pub name: String,
    /// The number of threads in each thread group, along x, y, and z.
    pub thread_group_size: [u32; 3],
    /// Every parameter of the kernel, including the parameters that its
    /// compiled code does not use, in the order of their bindings, with the
    /// plain values of each uniform buffer at the place of the buffer and the
    /// `uniform` parameters of the entry point last.
    pub parameters: Vec<Parameter>,
    /// The structures among the plain values of the kernel, in the order of
    /// the walk of its parameters, an outer structure before the structures
    /// inside it.
    pub structures: Vec<UniformStructure>,
}

/// One parameter of a kernel that a call sets or binds by its path.
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise (SLM-6), and the tests of the generator build it with a literal"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Parameter {
    /// The path by which a call sets or binds the parameter, such as `scale`,
    /// `params.offset`, or `data`.
    pub path: String,
    /// What the parameter is, with its type.
    pub kind: ParameterKind,
}

/// What a parameter of a kernel is, and so which method of
/// [`Call`](crate::Call) gives it.
#[expect(
    clippy::exhaustive_enums,
    reason = "the module has no stability promise (SLM-6), and the generator matches every variant"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParameterKind {
    /// A plain value, such as a `float` or a `uint3`, which
    /// [`Call::set`](crate::Call::set) gives. A plain value inside a
    /// structure has the dotted path of its field.
    Value(ValueType),
    /// A structured buffer, which [`Call::bind`](crate::Call::bind) binds.
    Buffer {
        /// Whether the kernel can write the buffer.
        access: Access,
        /// The type of the elements of the buffer.
        element: ElementType,
    },
    /// A `Texture2D` with `float` results, which
    /// [`Call::texture`](crate::Call::texture) binds.
    Texture,
    /// A `SamplerState`, which [`Call::sampler`](crate::Call::sampler)
    /// binds.
    Sampler,
}

/// The type of a plain value: a scalar, a vector, a matrix, or an array of
/// fixed length of one of them.
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise (SLM-6), and the tests of the generator build it with a literal"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueType {
    /// The scalar type of every component of the value.
    pub scalar: Scalar,
    /// How the scalars of the value are arranged.
    pub shape: Shape,
}

/// The scalar type of a plain value, the Slang `bool`, `int`, `uint`, or
/// `float`.
#[expect(
    clippy::exhaustive_enums,
    reason = "the module has no stability promise (SLM-6), and the generator matches every variant"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scalar {
    /// The Slang `bool`, a [`bool`] in Rust.
    Bool,
    /// The Slang `int`, an [`i32`] in Rust.
    Int32,
    /// The Slang `uint`, a [`u32`] in Rust.
    Uint32,
    /// The Slang `float`, an [`f32`] in Rust.
    Float32,
}

/// How the scalars of a plain value are arranged.
#[expect(
    clippy::exhaustive_enums,
    reason = "the module has no stability promise (SLM-6), and the generator matches every variant"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Shape {
    /// One scalar.
    Scalar,
    /// A vector of `length` scalars, such as a `float3`.
    Vector {
        /// The number of components.
        length: u32,
    },
    /// A matrix of `rows` rows of `columns` scalars, such as a `float2x3`.
    Matrix {
        /// The number of rows.
        rows: u32,
        /// The number of columns of each row.
        columns: u32,
    },
    /// An array of fixed length.
    Array {
        /// The number of elements.
        length: u64,
        /// The distance in bytes from one element to the next.
        stride: u64,
        /// The shape of each element.
        element: Box<Shape>,
    },
}

/// Whether a kernel can write a structured buffer.
#[expect(
    clippy::exhaustive_enums,
    reason = "the module has no stability promise (SLM-6), and the generator matches every variant"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Access {
    /// A `StructuredBuffer`, which the kernel only reads.
    ReadOnly,
    /// A `RWStructuredBuffer`, which the kernel reads and writes.
    ReadWrite,
}

/// The type of the elements of a structured buffer.
#[expect(
    clippy::exhaustive_enums,
    reason = "the module has no stability promise (SLM-6), and the generator matches every variant"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ElementType {
    /// A scalar or a vector of `bool`, `int`, `uint`, or `float`.
    Plain {
        /// The scalar type of each component.
        scalar: Scalar,
        /// The number of components, 1 for a scalar.
        components: u32,
    },
    /// A scalar or a vector of another scalar type, such as `half2`.
    OtherPlain {
        /// The name that Slang gives the type, such as `vector<half,2>`.
        name: String,
    },
    /// A structure.
    Structure(Structure),
    /// A matrix, an array, or another type that is neither a scalar, a
    /// vector, nor a structure.
    Composite,
}

/// A Slang structure that is the element of a structured buffer, or a field
/// of one, with its layout in the buffer.
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise (SLM-6), and the tests of the generator build it with a literal"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Structure {
    /// The name of the structure in the Slang source.
    pub name: String,
    /// The size of the structure in bytes, which is its stride in the
    /// buffer.
    pub size: u64,
    /// The fields of the structure, in the order of the Slang source.
    pub fields: Vec<Field>,
}

/// A field of a [`Structure`].
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise (SLM-6), and the tests of the generator build it with a literal"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Field {
    /// The name of the field in the Slang source.
    pub name: String,
    /// The offset of the field from the start of the structure, in bytes.
    pub offset: u64,
    /// The type of the field.
    pub ty: FieldType,
}

/// The type of a [`Field`].
#[expect(
    clippy::exhaustive_enums,
    reason = "the module has no stability promise (SLM-6), and the generator matches every variant"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldType {
    /// A plain value.
    Value(ValueType),
    /// A structure inside the structure.
    Structure(Structure),
    /// Any other type, such as a `half` or an array of structures.
    Other {
        /// The name that Slang gives the type.
        name: String,
    },
}

/// A Slang structure among the plain values of a kernel, such as the type of
/// `uniform Params params;`, whose fields are the parameters under its path.
///
/// The element of a constant buffer is not such a structure: its fields are
/// the parameters of the kernel, with the name of the buffer as the first
/// part of their paths.
#[expect(
    clippy::exhaustive_structs,
    reason = "the module has no stability promise (SLM-6), and the tests of the generator build it with a literal"
)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UniformStructure {
    /// The path of the value of the structure, such as `params`.
    pub path: String,
    /// The name of the structure in the Slang source.
    pub name: String,
}

/// Returns the description of the kernels of a compilation and of the files
/// that Slang read for it.
pub(super) fn describe(compilation: &Compilation) -> Interface {
    Interface {
        kernels: compilation.compiled.kernels.iter().map(kernel).collect(),
        dependencies: compilation.dependencies.iter().map(PathBuf::from).collect(),
    }
}

fn kernel(kernel: &CompiledKernel) -> KernelInterface {
    let KernelLayout {
        name,
        thread_group_size,
        bindings,
        immediates,
        structures,
    } = &kernel.layout;
    let mut parameters: Vec<Parameter> = bindings.iter().flat_map(binding).collect();
    parameters.extend(immediates.iter().flat_map(values));
    KernelInterface {
        name: name.clone(),
        thread_group_size: *thread_group_size,
        parameters,
        structures: structures
            .iter()
            .map(|structure| UniformStructure {
                path: structure.path.clone(),
                name: structure.name.clone(),
            })
            .collect(),
    }
}

fn binding(binding: &Binding) -> Vec<Parameter> {
    let parameter = |kind| {
        vec![Parameter {
            path: binding.path.clone(),
            kind,
        }]
    };
    match &binding.resource {
        Resource::StorageBuffer {
            access, element, ..
        } => parameter(ParameterKind::Buffer {
            access: match access {
                layout::Access::ReadOnly => Access::ReadOnly,
                layout::Access::ReadWrite => Access::ReadWrite,
            },
            element: element_type(element),
        }),
        Resource::UniformBuffer(block) => values(block),
        Resource::SampledTexture => parameter(ParameterKind::Texture),
        Resource::Sampler => parameter(ParameterKind::Sampler),
    }
}

fn values(block: &layout::UniformBlock) -> Vec<Parameter> {
    block
        .values
        .iter()
        .map(|value| Parameter {
            path: value.path.clone(),
            kind: ParameterKind::Value(value_type(value.scalar, &value.shape)),
        })
        .collect()
}

fn value_type(scalar: layout::Scalar, shape: &layout::Shape) -> ValueType {
    ValueType {
        scalar: self::scalar(scalar),
        shape: self::shape(shape),
    }
}

fn scalar(scalar: layout::Scalar) -> Scalar {
    match scalar {
        layout::Scalar::Bool => Scalar::Bool,
        layout::Scalar::Int32 => Scalar::Int32,
        layout::Scalar::Uint32 => Scalar::Uint32,
        layout::Scalar::Float32 => Scalar::Float32,
    }
}

fn shape(shape: &layout::Shape) -> Shape {
    match shape {
        layout::Shape::Scalar => Shape::Scalar,
        layout::Shape::Vector { length } => Shape::Vector { length: *length },
        layout::Shape::Matrix { rows, columns } => Shape::Matrix {
            rows: *rows,
            columns: *columns,
        },
        layout::Shape::Array {
            length,
            stride,
            element,
        } => Shape::Array {
            length: *length,
            stride: *stride,
            element: Box::new(self::shape(element)),
        },
    }
}

fn element_type(element: &layout::ElementType) -> ElementType {
    match element {
        layout::ElementType::Plain { scalar, components } => ElementType::Plain {
            scalar: self::scalar(*scalar),
            components: *components,
        },
        layout::ElementType::OtherPlain { name } => ElementType::OtherPlain { name: name.clone() },
        layout::ElementType::Structure(inner) => ElementType::Structure(structure(inner)),
        layout::ElementType::Composite => ElementType::Composite,
    }
}

fn structure(structure: &layout::Structure) -> Structure {
    Structure {
        name: structure.name.clone(),
        size: structure.size,
        fields: structure
            .fields
            .iter()
            .map(|field| Field {
                name: field.name.clone(),
                offset: field.offset,
                ty: match &field.ty {
                    layout::FieldType::Plain { scalar, shape } => {
                        FieldType::Value(value_type(*scalar, shape))
                    }
                    layout::FieldType::Structure(inner) => {
                        FieldType::Structure(self::structure(inner))
                    }
                    layout::FieldType::Other { name } => FieldType::Other { name: name.clone() },
                },
            })
            .collect(),
    }
}
