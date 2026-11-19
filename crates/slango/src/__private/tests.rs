use std::path::PathBuf;

use super::interface::describe;
use super::{
    Access, ElementType, Field, FieldType, Interface, KernelInterface, LoadedModule, Parameter,
    ParameterKind, Scalar, Shape, StructElement, StructField, StructFieldType, Structure,
    UniformStructure, ValueType, slang_value_type,
};
use crate::draw::element_type;
use crate::error::ErrorKind;
use crate::layout::{
    self, Compilation, Compiled, CompiledKernel, KernelLayout, SessionModule, Usage,
};
use crate::marshal::{self, ValueType as RustType};

mod ffi;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pair {
    weight: f32,
    id: u32,
}

impl StructElement for Pair {
    const SLANG_NAME: &'static str = "Pair";
    const STRIDE: usize = 8;
    const FIELDS: &'static [StructField] = &[
        StructField {
            name: "weight",
            offset: 0,
            ty: StructFieldType::Plain("float"),
        },
        StructField {
            name: "id",
            offset: 4,
            ty: StructFieldType::Plain("uint"),
        },
    ];

    fn write_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.weight.to_le_bytes());
        out.extend_from_slice(&self.id.to_le_bytes());
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        let [weight, id] = bytes.as_chunks::<4>().0 else {
            return None;
        };
        (bytes.len() == Self::STRIDE).then(|| Self {
            weight: f32::from_le_bytes(*weight),
            id: u32::from_le_bytes(*id),
        })
    }
}

fn float() -> layout::PlainValue {
    layout::PlainValue {
        path: "params".to_owned(),
        offset: 0,
        size: 4,
        scalar: layout::Scalar::Float32,
        shape: layout::Shape::Scalar,
    }
}

#[test]
fn a_struct_element_and_an_array_of_it_are_values_that_set_rejects_with_the_structure_name() {
    let pair = Pair { weight: 1.0, id: 2 };
    for ty in [marshal::marshal(&pair).ty, marshal::marshal(&[pair; 2]).ty] {
        assert_eq!(
            ty,
            RustType::Structure {
                name: "Pair",
                fields: Pair::FIELDS
            }
        );
        let error =
            marshal::check("params", &ty, &float()).expect_err("a structure is no plain value");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            "cannot set a plain value: the parameter `params` has the Slang type `float`, and the value has the Rust type `Pair`"
        );
    }
}

#[test]
fn a_struct_element_matches_any_structure_or_composite_element_and_no_plain_element() {
    let ty = marshal::marshal(&Pair { weight: 0.0, id: 0 }).ty;
    let structure = layout::ElementType::Structure(layout::Structure {
        name: "Other".to_owned(),
        size: 8,
        fields: Vec::new(),
    });
    assert_eq!(marshal::element_mismatch(&ty, &structure), None);
    assert_eq!(
        marshal::element_mismatch(&ty, &layout::ElementType::Composite),
        None
    );
    let plain = layout::ElementType::Plain {
        scalar: layout::Scalar::Float32,
        components: 2,
    };
    assert_eq!(
        marshal::element_mismatch(&ty, &plain),
        Some("float2".to_owned())
    );
}

fn scalar_field(name: &str, offset: u64) -> layout::Field {
    typed_field(name, offset, layout::Scalar::Float32, layout::Shape::Scalar)
}

fn typed_field(
    name: &str,
    offset: u64,
    scalar: layout::Scalar,
    shape: layout::Shape,
) -> layout::Field {
    layout::Field {
        name: name.to_owned(),
        offset,
        ty: layout::FieldType::Plain { scalar, shape },
    }
}

fn uint_field(name: &str, offset: u64) -> layout::Field {
    typed_field(name, offset, layout::Scalar::Uint32, layout::Shape::Scalar)
}

fn reflected_pair(weight: u64, id: u64) -> layout::ElementType {
    element(layout::Structure {
        name: "Pair".to_owned(),
        size: 8,
        fields: vec![scalar_field("weight", weight), uint_field("id", id)],
    })
}

fn element(structure: layout::Structure) -> layout::ElementType {
    layout::ElementType::Structure(structure)
}

#[test]
fn a_struct_element_with_the_reflected_fields_at_their_offsets_matches() {
    assert_eq!(
        marshal::field_mismatch("Pair", Pair::FIELDS, &reflected_pair(0, 4)),
        None
    );
}

#[test]
fn a_struct_element_whose_field_is_at_another_offset_names_the_first_such_field() {
    assert_eq!(
        marshal::field_mismatch("Pair", Pair::FIELDS, &reflected_pair(4, 0)).as_deref(),
        Some(
            "has elements of the structure `Pair` with the field `weight` at byte 4, and the bound buffer has elements of the Rust struct `Pair` with that field at byte 0"
        )
    );
}

#[test]
fn a_struct_element_with_another_field_or_a_missing_or_extra_field_does_not_match() {
    let renamed = layout::Structure {
        name: "Pair".to_owned(),
        size: 8,
        fields: vec![scalar_field("weight", 0), uint_field("count", 4)],
    };
    assert_eq!(
        marshal::field_mismatch("Pair", Pair::FIELDS, &element(renamed)).as_deref(),
        Some(
            "has elements of the structure `Pair` with the field `count` at byte 4, and the bound buffer has elements of the Rust struct `Pair` without that field there"
        )
    );
    let longer = layout::Structure {
        name: "Pair".to_owned(),
        size: 12,
        fields: vec![
            scalar_field("weight", 0),
            uint_field("id", 4),
            scalar_field("spare", 8),
        ],
    };
    assert_eq!(
        marshal::field_mismatch("Pair", Pair::FIELDS, &element(longer)).as_deref(),
        Some(
            "has elements of the structure `Pair` with the field `spare` at byte 8, and the bound buffer has elements of the Rust struct `Pair` without that field there"
        )
    );
    let shorter = layout::Structure {
        name: "Pair".to_owned(),
        size: 8,
        fields: vec![scalar_field("weight", 0)],
    };
    assert_eq!(
        marshal::field_mismatch("Pair", Pair::FIELDS, &element(shorter)).as_deref(),
        Some(
            "has elements of the structure `Pair`, and the bound buffer has elements of the Rust struct `Pair` with the field `id` at byte 4, which the structure lacks"
        )
    );
}

#[test]
fn the_fields_of_a_nested_structure_are_compared_by_their_paths_and_offsets_from_the_outer_start() {
    let outer = layout::Structure {
        name: "Body".to_owned(),
        size: 16,
        fields: vec![
            scalar_field("mass", 0),
            layout::Field {
                name: "motion".to_owned(),
                offset: 4,
                ty: layout::FieldType::Structure(layout::Structure {
                    name: "Motion".to_owned(),
                    size: 8,
                    fields: vec![scalar_field("speed", 0), scalar_field("drag", 4)],
                }),
            },
            scalar_field("id", 12),
        ],
    };
    let fields = [
        StructField {
            name: "mass",
            offset: 0,
            ty: StructFieldType::Plain("float"),
        },
        StructField {
            name: "motion",
            offset: 4,
            ty: StructFieldType::Structure("Motion"),
        },
        StructField {
            name: "motion.speed",
            offset: 4,
            ty: StructFieldType::Plain("float"),
        },
        StructField {
            name: "motion.drag",
            offset: 8,
            ty: StructFieldType::Plain("float"),
        },
        StructField {
            name: "id",
            offset: 12,
            ty: StructFieldType::Plain("float"),
        },
    ];
    let outer = element(outer);
    assert_eq!(marshal::field_mismatch("Body", &fields, &outer), None);
    let mut renamed = fields;
    if let Some(motion) = renamed.get_mut(1) {
        motion.ty = StructFieldType::Structure("Drift");
    }
    assert_eq!(marshal::field_mismatch("Body", &renamed, &outer), None);
    let mut flattened = fields;
    if let Some(motion) = flattened.get_mut(1) {
        motion.ty = StructFieldType::Plain("float2");
    }
    assert_eq!(
        marshal::field_mismatch("Body", &flattened, &outer).as_deref(),
        Some(
            "has elements of the structure `Body` with the field `motion` of the structure `Motion`, and the bound buffer has elements of the Rust struct `Body` with that field of the Slang type `float2`"
        )
    );
    let mut moved = fields;
    if let Some(drag) = moved.get_mut(3) {
        drag.offset = 12;
    }
    assert_eq!(
        marshal::field_mismatch("Body", &moved, &outer).as_deref(),
        Some(
            "has elements of the structure `Body` with the field `motion.drag` at byte 8, and the bound buffer has elements of the Rust struct `Body` with that field at byte 12"
        )
    );
}

#[test]
fn a_struct_element_whose_field_has_another_scalar_type_or_width_names_the_field_and_both_types() {
    let vector = |length| layout::Shape::Vector { length };
    let reflected = |scalar, shape| {
        element(layout::Structure {
            name: "P".to_owned(),
            size: 16,
            fields: vec![typed_field("a", 0, scalar, shape), scalar_field("b", 12)],
        })
    };
    let fields = [
        StructField {
            name: "a",
            offset: 0,
            ty: StructFieldType::Plain("float3"),
        },
        StructField {
            name: "b",
            offset: 12,
            ty: StructFieldType::Plain("float"),
        },
    ];
    assert_eq!(
        marshal::field_mismatch("P", &fields, &reflected(layout::Scalar::Float32, vector(3))),
        None
    );
    for (scalar, shape, name) in [
        (layout::Scalar::Int32, vector(3), "int3"),
        (layout::Scalar::Uint32, vector(3), "uint3"),
        (layout::Scalar::Float32, vector(2), "float2"),
        (layout::Scalar::Float32, layout::Shape::Scalar, "float"),
        (
            layout::Scalar::Float32,
            layout::Shape::Array {
                length: 3,
                stride: 4,
                element: Box::new(layout::Shape::Scalar),
            },
            "float[3]",
        ),
    ] {
        assert_eq!(
            marshal::field_mismatch("P", &fields, &reflected(scalar, shape)).as_deref(),
            Some(
                format!(
                    "has elements of the structure `P` with the field `a` of the Slang type `{name}`, and the bound buffer has elements of the Rust struct `P` with that field of the Slang type `float3`"
                )
                .as_str()
            )
        );
    }
}

#[test]
fn a_struct_element_matches_a_structure_of_another_name_with_the_same_fields() {
    let other = layout::Structure {
        name: "Couple".to_owned(),
        size: 8,
        fields: vec![scalar_field("weight", 0), uint_field("id", 4)],
    };
    assert_eq!(
        marshal::field_mismatch("Pair", Pair::FIELDS, &element(other)),
        None
    );
}

#[test]
fn a_struct_element_does_not_match_an_element_that_is_not_a_structure() {
    assert_eq!(
        marshal::field_mismatch("Pair", Pair::FIELDS, &layout::ElementType::Composite).as_deref(),
        Some(
            "has elements that are not a structure, and the bound buffer has elements of the Rust struct `Pair`"
        )
    );
}

#[test]
fn a_field_of_another_type_than_a_plain_value_or_a_structure_matches_no_generated_field() {
    let reflected = element(layout::Structure {
        name: "Pair".to_owned(),
        size: 8,
        fields: vec![
            layout::Field {
                name: "weight".to_owned(),
                offset: 0,
                ty: layout::FieldType::Other {
                    name: "half2".to_owned(),
                },
            },
            uint_field("id", 4),
        ],
    });
    assert_eq!(
        marshal::field_mismatch("Pair", Pair::FIELDS, &reflected).as_deref(),
        Some(
            "has elements of the structure `Pair` with the field `weight` of the Slang type `half2`, and the bound buffer has elements of the Rust struct `Pair` with that field of the Slang type `float`"
        )
    );
}

#[test]
fn the_slang_name_of_a_value_tells_apart_every_scalar_type_and_shape() {
    let named = |scalar, shape| slang_value_type(&ValueType { scalar, shape });
    assert_eq!(named(Scalar::Int32, Shape::Scalar), "int");
    assert_eq!(named(Scalar::Uint32, Shape::Vector { length: 3 }), "uint3");
    assert_eq!(
        named(
            Scalar::Float32,
            Shape::Matrix {
                rows: 2,
                columns: 3
            }
        ),
        "float2x3"
    );
    assert_eq!(
        named(
            Scalar::Float32,
            Shape::Array {
                length: 4,
                stride: 16,
                element: Box::new(Shape::Array {
                    length: 2,
                    stride: 8,
                    element: Box::new(Shape::Vector { length: 2 }),
                }),
            }
        ),
        "float2[4][2]"
    );
    assert_eq!(
        named(
            Scalar::Bool,
            Shape::Array {
                length: 3,
                stride: 4,
                element: Box::new(Shape::Scalar),
            }
        ),
        "bool[3]"
    );
}

#[test]
fn a_struct_element_has_no_type_of_a_vertex_attribute() {
    assert_eq!(element_type::<Pair>(), None);
    assert_eq!(
        element_type::<[f32; 2]>(),
        Some((layout::Scalar::Float32, 2))
    );
}

fn storage(path: &str, access: layout::Access, element: layout::ElementType) -> layout::Binding {
    layout::Binding {
        path: path.to_owned(),
        set: 0,
        index: 1,
        usage: Usage::Unused,
        resource: layout::Resource::StorageBuffer {
            access,
            stride: 16,
            element,
        },
    }
}

fn resource(path: &str, resource: layout::Resource) -> layout::Binding {
    layout::Binding {
        path: path.to_owned(),
        set: 1,
        index: 0,
        usage: Usage::Used,
        resource,
    }
}

fn plain(path: &str, scalar: layout::Scalar, shape: layout::Shape) -> layout::PlainValue {
    layout::PlainValue {
        path: path.to_owned(),
        offset: 0,
        size: 4,
        scalar,
        shape,
    }
}

fn particle() -> layout::Structure {
    layout::Structure {
        name: "Particle".to_owned(),
        size: 16,
        fields: vec![
            layout::Field {
                name: "position".to_owned(),
                offset: 0,
                ty: layout::FieldType::Plain {
                    scalar: layout::Scalar::Float32,
                    shape: layout::Shape::Vector { length: 3 },
                },
            },
            layout::Field {
                name: "small".to_owned(),
                offset: 12,
                ty: layout::FieldType::Other {
                    name: "half".to_owned(),
                },
            },
        ],
    }
}

/// A compilation of one kernel with a parameter of each kind, and of a
/// render entry point, which the description leaves out.
fn compilation() -> Compilation {
    let element = particle();
    let block = layout::UniformBlock {
        size: 64,
        values: vec![
            plain("factor", layout::Scalar::Float32, layout::Shape::Scalar),
            plain(
                "params.rotation",
                layout::Scalar::Float32,
                layout::Shape::Matrix {
                    rows: 2,
                    columns: 2,
                    row_stride: 16,
                },
            ),
        ],
    };
    let immediates = layout::UniformBlock {
        size: 8,
        values: vec![plain(
            "offsets",
            layout::Scalar::Int32,
            layout::Shape::Array {
                length: 2,
                stride: 4,
                element: Box::new(layout::Shape::Scalar),
            },
        )],
    };
    let kernel = CompiledKernel {
        layout: KernelLayout {
            name: "advance".to_owned(),
            thread_group_size: [8, 4, 1],
            bindings: vec![
                resource("", layout::Resource::UniformBuffer(block)),
                storage(
                    "particles",
                    layout::Access::ReadWrite,
                    layout::ElementType::Structure(element),
                ),
                storage(
                    "weights",
                    layout::Access::ReadOnly,
                    layout::ElementType::Plain {
                        scalar: layout::Scalar::Uint32,
                        components: 4,
                    },
                ),
                storage(
                    "halves",
                    layout::Access::ReadOnly,
                    layout::ElementType::OtherPlain {
                        name: "half".to_owned(),
                    },
                ),
                storage(
                    "matrices",
                    layout::Access::ReadOnly,
                    layout::ElementType::Composite,
                ),
                resource("image", layout::Resource::SampledTexture),
                resource("nearest", layout::Resource::Sampler),
            ],
            immediates: Some(immediates),
            structures: vec![layout::UniformStructure {
                path: "params".to_owned(),
                name: "Params".to_owned(),
            }],
        },
        spirv: vec![1, 2, 3],
    };
    Compilation {
        compiled: Compiled {
            kernels: vec![kernel],
            render: vec![layout::CompiledRender {
                name: "vsMain".to_owned(),
                stage: layout::RenderStage::Vertex,
                usable: Err(layout::Rejection {
                    kind: ErrorKind::Compile,
                    detail: "rejected".to_owned(),
                    source: Some("Slang reports error 30019".to_owned()),
                }),
            }],
        },
        dependencies: vec![
            "/tmp/shaders/particles.slang".to_owned(),
            "/tmp/shaders/common.slang".to_owned(),
        ],
        modules: session_modules(),
    }
}

/// The modules of the session of [`compilation`]: the module `particles`,
/// which imports `common`.
fn session_modules() -> Vec<SessionModule> {
    vec![
        SessionModule {
            name: "common".to_owned(),
            path: "/tmp/shaders/common.slang".to_owned(),
            files: vec!["/tmp/shaders/common.slang".to_owned()],
        },
        SessionModule {
            name: "particles".to_owned(),
            path: "/tmp/shaders/particles.slang".to_owned(),
            files: vec![
                "/tmp/shaders/particles.slang".to_owned(),
                "/tmp/shaders/common.slang".to_owned(),
            ],
        },
    ]
}

/// The packed layout of the structure `Particle` of [`compilation`] for the
/// kernel `advance`, and a structure for a kernel and a path that the
/// compilation lacks.
fn packed() -> Vec<(String, Vec<layout::ElementStructure>)> {
    let mut packed = particle();
    packed.size = 14;
    vec![
        (
            "advance".to_owned(),
            vec![
                ("particles".to_owned(), packed),
                ("missing".to_owned(), particle()),
            ],
        ),
        (
            "other".to_owned(),
            vec![("particles".to_owned(), particle())],
        ),
    ]
}

fn parameter(path: &str, kind: ParameterKind) -> Parameter {
    Parameter {
        path: path.to_owned(),
        kind,
    }
}

fn value(scalar: Scalar, shape: Shape) -> ParameterKind {
    ParameterKind::Value(ValueType { scalar, shape })
}

/// The description of the structure of [`particle`].
fn described_particle() -> Structure {
    Structure {
        name: "Particle".to_owned(),
        size: 16,
        fields: vec![
            Field {
                name: "position".to_owned(),
                offset: 0,
                ty: FieldType::Value(ValueType {
                    scalar: Scalar::Float32,
                    shape: Shape::Vector { length: 3 },
                }),
            },
            Field {
                name: "small".to_owned(),
                offset: 12,
                ty: FieldType::Other {
                    name: "half".to_owned(),
                },
            },
        ],
    }
}

#[test]
fn the_description_of_a_compilation_lists_each_parameter_of_each_kernel_with_its_kind_and_type() {
    let element = described_particle();
    let mut packed_element = element.clone();
    packed_element.size = 14;
    let buffer = |access, element| ParameterKind::Buffer {
        access,
        element,
        packed: None,
    };
    let expected = Interface {
        kernels: vec![KernelInterface {
            name: "advance".to_owned(),
            thread_group_size: [8, 4, 1],
            parameters: vec![
                parameter("factor", value(Scalar::Float32, Shape::Scalar)),
                parameter(
                    "params.rotation",
                    value(
                        Scalar::Float32,
                        Shape::Matrix {
                            rows: 2,
                            columns: 2,
                        },
                    ),
                ),
                parameter(
                    "particles",
                    ParameterKind::Buffer {
                        access: Access::ReadWrite,
                        element: ElementType::Structure(element),
                        packed: Some(packed_element),
                    },
                ),
                parameter(
                    "weights",
                    buffer(
                        Access::ReadOnly,
                        ElementType::Plain {
                            scalar: Scalar::Uint32,
                            components: 4,
                        },
                    ),
                ),
                parameter(
                    "halves",
                    buffer(
                        Access::ReadOnly,
                        ElementType::OtherPlain {
                            name: "half".to_owned(),
                        },
                    ),
                ),
                parameter("matrices", buffer(Access::ReadOnly, ElementType::Composite)),
                parameter("image", ParameterKind::Texture),
                parameter("nearest", ParameterKind::Sampler),
                parameter(
                    "offsets",
                    value(
                        Scalar::Int32,
                        Shape::Array {
                            length: 2,
                            stride: 4,
                            element: Box::new(Shape::Scalar),
                        },
                    ),
                ),
            ],
            structures: vec![UniformStructure {
                path: "params".to_owned(),
                name: "Params".to_owned(),
            }],
        }],
        dependencies: vec![
            PathBuf::from("/tmp/shaders/particles.slang"),
            PathBuf::from("/tmp/shaders/common.slang"),
        ],
        modules: session_modules()
            .into_iter()
            .map(|module| LoadedModule {
                name: module.name,
                path: PathBuf::from(module.path),
                files: module.files.into_iter().map(PathBuf::from).collect(),
            })
            .collect(),
    };
    assert_eq!(describe(&compilation(), &packed()), expected);
}

#[test]
fn every_scalar_type_keeps_its_meaning_in_the_description() {
    let scalars = [
        (layout::Scalar::Bool, Scalar::Bool),
        (layout::Scalar::Int32, Scalar::Int32),
        (layout::Scalar::Uint32, Scalar::Uint32),
        (layout::Scalar::Float32, Scalar::Float32),
    ];
    for (scalar, expected) in scalars {
        let mut compilation = compilation();
        if let Some(kernel) = compilation.compiled.kernels.first_mut() {
            kernel.layout.bindings = vec![storage(
                "data",
                layout::Access::ReadOnly,
                layout::ElementType::Plain {
                    scalar,
                    components: 1,
                },
            )];
            kernel.layout.immediates = None;
        }
        let parameters = describe(&compilation, &[])
            .kernels
            .into_iter()
            .flat_map(|kernel| kernel.parameters)
            .collect::<Vec<_>>();
        assert_eq!(
            parameters,
            [parameter(
                "data",
                ParameterKind::Buffer {
                    access: Access::ReadOnly,
                    element: ElementType::Plain {
                        scalar: expected,
                        components: 1,
                    },
                    packed: None,
                },
            )]
        );
    }
}
