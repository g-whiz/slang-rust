use std::path::PathBuf;

use super::interface::describe;
use super::{
    Access, ElementType, Field, FieldType, Interface, KernelInterface, Parameter, ParameterKind,
    Scalar, Shape, StructElement, Structure, UniformStructure, ValueType,
};
use crate::draw::element_type;
use crate::error::ErrorKind;
use crate::layout::{self, Compilation, Compiled, CompiledKernel, KernelLayout, Usage};
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
        assert_eq!(ty, RustType::Structure { name: "Pair" });
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
                }),
            }],
        },
        dependencies: vec!["/tmp/shaders/particles.slang".to_owned()],
    }
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

#[test]
fn the_description_of_a_compilation_lists_each_parameter_of_each_kernel_with_its_kind_and_type() {
    let element = Structure {
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
    };
    let buffer = |access, element| ParameterKind::Buffer { access, element };
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
                    buffer(Access::ReadWrite, ElementType::Structure(element)),
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
        dependencies: vec![PathBuf::from("/tmp/shaders/particles.slang")],
    };
    assert_eq!(describe(&compilation()), expected);
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
        let parameters = describe(&compilation)
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
                },
            )]
        );
    }
}
