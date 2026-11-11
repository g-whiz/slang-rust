use super::{MAGIC, MAX_DEPTH, Reader, decode, encode};
use crate::__private::ENCODING_VERSION;
use crate::error::{Error, ErrorKind};
use crate::layout::{
    Access, Attribute, Binding, Compiled, CompiledKernel, CompiledRender, ElementType, Field,
    FieldType, KernelLayout, PlainValue, Rejection, RenderLayout, RenderStage, Resource, Scalar,
    Shape, Structure, UniformBlock, UniformStructure, UsableRender, Usage,
};

const SCALARS: [Scalar; 4] = [Scalar::Bool, Scalar::Int32, Scalar::Uint32, Scalar::Float32];

const KINDS: [ErrorKind; 8] = [
    ErrorKind::Compile,
    ErrorKind::NoAdapter,
    ErrorKind::DeviceRequest,
    ErrorKind::UnsupportedShader,
    ErrorKind::UnsupportedParameter,
    ErrorKind::MissingFeature,
    ErrorKind::InvalidArgument,
    ErrorKind::Backend,
];

fn array(length: u64, stride: u64, element: Shape) -> Shape {
    Shape::Array {
        length,
        stride,
        element: Box::new(element),
    }
}

fn shapes() -> Vec<Shape> {
    vec![
        Shape::Scalar,
        Shape::Vector { length: 3 },
        Shape::Matrix {
            rows: 2,
            columns: 3,
        },
        array(4, 16, Shape::Vector { length: 3 }),
        array(
            2,
            u64::MAX,
            array(
                3,
                48,
                Shape::Matrix {
                    rows: 3,
                    columns: 4,
                },
            ),
        ),
    ]
}

fn value(path: &str, offset: u64, scalar: Scalar, shape: Shape) -> PlainValue {
    PlainValue {
        path: path.to_owned(),
        offset,
        size: offset.wrapping_add(4),
        scalar,
        shape,
    }
}

/// Every scalar type with every shape, as the values of one block.
fn full_block() -> UniformBlock {
    let mut values = Vec::new();
    for (index, scalar) in SCALARS.into_iter().enumerate() {
        for (offset, shape) in (0_u64..).zip(shapes()) {
            values.push(value(
                &format!("params.v{index}.{offset}"),
                offset * 16,
                scalar,
                shape,
            ));
        }
    }
    UniformBlock { size: 1024, values }
}

fn empty_block() -> UniformBlock {
    UniformBlock {
        size: 0,
        values: Vec::new(),
    }
}

fn field(name: &str, offset: u64, ty: FieldType) -> Field {
    Field {
        name: name.to_owned(),
        offset,
        ty,
    }
}

/// A structure with a field of each type, a nested structure among them.
fn structure() -> Structure {
    let inner = Structure {
        name: "Inner".to_owned(),
        size: 16,
        fields: vec![
            field(
                "id",
                0,
                FieldType::Plain {
                    scalar: Scalar::Uint32,
                    shape: Shape::Scalar,
                },
            ),
            field(
                "empty",
                4,
                FieldType::Structure(Structure {
                    name: String::new(),
                    size: 0,
                    fields: Vec::new(),
                }),
            ),
        ],
    };
    let mut fields: Vec<Field> = (0_u64..)
        .zip(shapes())
        .map(|(index, shape)| {
            field(
                &format!("f{index}"),
                index * 16,
                FieldType::Plain {
                    scalar: Scalar::Float32,
                    shape,
                },
            )
        })
        .collect();
    fields.push(field("inner", 96, FieldType::Structure(inner)));
    fields.push(field(
        "small",
        112,
        FieldType::Other {
            name: "half".to_owned(),
        },
    ));
    Structure {
        name: "Particle".to_owned(),
        size: 128,
        fields,
    }
}

fn elements() -> Vec<ElementType> {
    let mut elements: Vec<ElementType> = SCALARS
        .into_iter()
        .zip(1..)
        .map(|(scalar, components)| ElementType::Plain { scalar, components })
        .collect();
    elements.push(ElementType::OtherPlain {
        name: "vector<half,2>".to_owned(),
    });
    elements.push(ElementType::Structure(structure()));
    elements.push(ElementType::Composite);
    elements
}

fn binding(path: &str, index: u32, usage: Usage, resource: Resource) -> Binding {
    Binding {
        path: path.to_owned(),
        set: index % 3,
        index,
        usage,
        resource,
    }
}

/// A binding of every kind of resource, with each access and element type of
/// a storage buffer, and both usages.
fn bindings() -> Vec<Binding> {
    let mut resources = Vec::new();
    for access in [Access::ReadOnly, Access::ReadWrite] {
        for element in elements() {
            resources.push(Resource::StorageBuffer {
                access,
                stride: 16,
                element,
            });
        }
    }
    resources.push(Resource::UniformBuffer(full_block()));
    resources.push(Resource::UniformBuffer(empty_block()));
    resources.push(Resource::SampledTexture);
    resources.push(Resource::Sampler);
    (0_u32..)
        .zip(resources)
        .map(|(index, resource)| {
            let usage = if index % 2 == 0 {
                Usage::Used
            } else {
                Usage::Unused
            };
            binding(&format!("binding{index}"), index, usage, resource)
        })
        .collect()
}

fn kernel(name: &str, bindings: Vec<Binding>, immediates: Option<UniformBlock>) -> CompiledKernel {
    CompiledKernel {
        layout: KernelLayout {
            name: name.to_owned(),
            thread_group_size: [64, 1, u32::MAX],
            bindings,
            immediates,
            structures: vec![
                UniformStructure {
                    path: "params".to_owned(),
                    name: "Params".to_owned(),
                },
                UniformStructure {
                    path: "params.inner".to_owned(),
                    name: "Inner".to_owned(),
                },
            ],
        },
        spirv: vec![0x0723_0203, 0x0001_0600, 0, u32::MAX],
    }
}

fn attributes() -> Vec<Attribute> {
    (0_u32..)
        .zip(SCALARS)
        .map(|(location, scalar)| Attribute {
            path: format!("input.a{location}"),
            location,
            scalar,
            components: location + 1,
            usage: if location == 0 {
                Usage::Unused
            } else {
                Usage::Used
            },
        })
        .collect()
}

fn usable(name: &str, stage: RenderStage, attributes: Vec<Attribute>) -> CompiledRender {
    CompiledRender {
        name: name.to_owned(),
        stage,
        usable: Ok(UsableRender {
            layout: RenderLayout {
                bindings: bindings(),
                immediates: Some(full_block()),
                attributes,
            },
            spirv: vec![1, 2, 3],
        }),
    }
}

fn rejected(name: &str, kind: ErrorKind) -> CompiledRender {
    CompiledRender {
        name: name.to_owned(),
        stage: RenderStage::Fragment,
        usable: Err(Rejection {
            kind,
            detail: format!("the parameter `p` of `{name}` is rejected"),
        }),
    }
}

fn render_records() -> Vec<CompiledRender> {
    let mut records = vec![
        usable("vsMain", RenderStage::Vertex, attributes()),
        usable("fsMain", RenderStage::Fragment, Vec::new()),
    ];
    records.extend(
        KINDS
            .into_iter()
            .map(|kind| rejected(&format!("rejected{kind:?}"), kind)),
    );
    records
}

/// A snapshot that holds every record, with every variant of each.
fn full() -> Compiled {
    Compiled {
        kernels: vec![
            kernel("scale", bindings(), Some(full_block())),
            kernel("", Vec::new(), None),
            kernel("ünïcödé", vec![], Some(empty_block())),
        ],
        render: render_records(),
    }
}

/// A small snapshot with one storage buffer of a structure, one uniform
/// buffer, immediates, and one rejected render record, for the tests that
/// decode every variation of its bytes.
fn small() -> Compiled {
    let bindings = vec![
        binding(
            "particles",
            0,
            Usage::Used,
            Resource::StorageBuffer {
                access: Access::ReadWrite,
                stride: 16,
                element: ElementType::Structure(Structure {
                    name: "P".to_owned(),
                    size: 16,
                    fields: vec![field(
                        "p",
                        0,
                        FieldType::Plain {
                            scalar: Scalar::Float32,
                            shape: array(2, 8, Shape::Vector { length: 2 }),
                        },
                    )],
                }),
            },
        ),
        binding(
            "",
            1,
            Usage::Unused,
            Resource::UniformBuffer(UniformBlock {
                size: 16,
                values: vec![value("f", 0, Scalar::Float32, Shape::Scalar)],
            }),
        ),
    ];
    let immediates = UniformBlock {
        size: 4,
        values: vec![value("n", 0, Scalar::Uint32, Shape::Scalar)],
    };
    Compiled {
        kernels: vec![kernel("k", bindings, Some(immediates))],
        render: vec![rejected("r", ErrorKind::UnsupportedParameter)],
    }
}

fn round_trip(compiled: &Compiled) -> Compiled {
    let bytes = encode(compiled).expect("the snapshot encodes");
    decode(&bytes).expect("the encoding decodes")
}

fn rejection(bytes: &[u8]) -> Error {
    match decode(bytes) {
        Ok(compiled) => panic!("the bytes decoded as {compiled:#?}"),
        Err(error) => error,
    }
}

fn detail_of(bytes: &[u8]) -> String {
    let error = rejection(bytes);
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    error.to_string()
}

#[test]
fn a_snapshot_with_every_record_and_variant_decodes_to_itself() {
    let compiled = full();
    assert_eq!(round_trip(&compiled), compiled);
}

#[test]
fn each_binding_and_each_render_record_on_its_own_decodes_to_itself() {
    for binding in bindings() {
        let compiled = Compiled {
            kernels: vec![kernel("k", vec![binding], None)],
            render: Vec::new(),
        };
        assert_eq!(round_trip(&compiled), compiled);
    }
    for record in render_records() {
        let compiled = Compiled {
            kernels: Vec::new(),
            render: vec![record],
        };
        assert_eq!(round_trip(&compiled), compiled);
    }
}

#[test]
fn an_empty_snapshot_decodes_to_itself() {
    let compiled = Compiled {
        kernels: Vec::new(),
        render: Vec::new(),
    };
    assert_eq!(round_trip(&compiled), compiled);
}

#[test]
fn the_encoding_starts_with_the_magic_bytes_and_the_little_endian_version() {
    let bytes = encode(&small()).expect("the snapshot encodes");
    assert_eq!(bytes.get(..8), Some(MAGIC.as_slice()));
    assert_eq!(
        bytes.get(8..12),
        Some(ENCODING_VERSION.to_le_bytes().as_slice())
    );
}

/// The encoding of [`small`] in hexadecimal, 32 bytes to a line.
const SMALL_ENCODING: [&str; 11] = [
    "736c616e676f00530100000001000000010000006b4000000001000000ffffff",
    "ff02000000090000007061727469636c65730000000000000000000001100000",
    "0000000000020100000050100000000000000001000000010000007000000000",
    "0000000000030302000000000000000800000000000000010200000000000000",
    "0100000001000000010110000000000000000100000001000000660000000000",
    "0000000400000000000000030001040000000000000001000000010000006e00",
    "00000000000000040000000000000002000200000006000000706172616d7306",
    "000000506172616d730c000000706172616d732e696e6e657205000000496e6e",
    "657204000000030223070006010000000000ffffffff01000000010000007201",
    "01042400000074686520706172616d6574657220607060206f66206072602069",
    "732072656a6563746564",
];

#[test]
fn the_encoding_of_the_small_snapshot_has_the_bytes_of_its_version() {
    let bytes = encode(&small()).expect("the snapshot encodes");
    let lines: Vec<String> = bytes
        .chunks(32)
        .map(|chunk| {
            chunk
                .iter()
                .flat_map(|byte| [byte >> 4, byte & 0xF])
                .filter_map(|digit| char::from_digit(u32::from(digit), 16))
                .collect()
        })
        .collect();
    assert_eq!(
        lines, SMALL_ENCODING,
        "the bytes of the encoding changed, so change ENCODING_VERSION in crates/slango/src/__private.rs and SMALL_ENCODING together"
    );
}

#[test]
fn an_encoding_of_another_version_is_an_invalid_argument_that_names_both_versions() {
    let bytes = encode(&small()).expect("the snapshot encodes");
    for version in [0, ENCODING_VERSION + 1, u32::MAX] {
        let mut other = bytes.clone();
        if let Some(slot) = other.get_mut(8..12) {
            slot.copy_from_slice(&version.to_le_bytes());
        }
        assert_eq!(
            detail_of(&other),
            format!(
                "cannot load a precompiled module: the encoded snapshot has version {version}, and this version of slango reads version {ENCODING_VERSION}"
            )
        );
    }
}

#[test]
fn bytes_without_the_magic_bytes_are_an_invalid_argument() {
    let mut bytes = encode(&small()).expect("the snapshot encodes");
    if let Some(first) = bytes.first_mut() {
        *first = b'S';
    }
    let expected =
        "cannot load a precompiled module: the bytes are not an encoded snapshot of slango";
    assert_eq!(detail_of(&bytes), expected);
    assert_eq!(detail_of(&[]), expected);
    assert_eq!(detail_of(MAGIC.get(..4).unwrap_or_default()), expected);
}

#[test]
fn every_truncation_of_an_encoding_is_an_invalid_argument() {
    let bytes = encode(&small()).expect("the snapshot encodes");
    for len in MAGIC.len()..bytes.len() {
        let truncated = bytes.get(..len).unwrap_or_default();
        assert_eq!(
            detail_of(truncated),
            "cannot load a precompiled module: the encoded snapshot is truncated or malformed",
            "a truncation to {len} of {} bytes",
            bytes.len()
        );
    }
}

#[test]
fn an_encoding_with_trailing_bytes_is_an_invalid_argument() {
    let mut bytes = encode(&small()).expect("the snapshot encodes");
    bytes.extend_from_slice(&[0, 0]);
    assert_eq!(
        detail_of(&bytes),
        "cannot load a precompiled module: the encoded snapshot has 2 trailing bytes"
    );
}

#[test]
fn a_corrupt_byte_anywhere_fails_or_decodes_to_a_snapshot_that_encodes_to_the_same_bytes() {
    let bytes = encode(&small()).expect("the snapshot encodes");
    let mut decoded = 0_usize;
    for position in 0..bytes.len() {
        for corrupt in [0x00, 0xFF] {
            let mut changed = bytes.clone();
            if let Some(byte) = changed.get_mut(position) {
                *byte = corrupt;
            }
            match decode(&changed) {
                Ok(compiled) => {
                    decoded += 1;
                    assert_eq!(encode(&compiled).ok(), Some(changed));
                }
                Err(error) => assert_eq!(error.kind(), ErrorKind::InvalidArgument),
            }
        }
    }
    assert!(decoded > 0, "no corrupt byte changed a value only");
}

#[test]
fn a_count_beyond_the_bytes_left_is_rejected_before_any_allocation() {
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&ENCODING_VERSION.to_le_bytes());
    bytes.extend_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(
        detail_of(&bytes),
        "cannot load a precompiled module: the encoded snapshot is truncated or malformed"
    );
}

#[test]
fn a_kernel_name_that_is_not_utf8_is_rejected() {
    let bytes = encode(&small()).expect("the snapshot encodes");
    assert_eq!(decode(&bytes).ok(), Some(small()));
    let name = MAGIC.len() + 12;
    assert_eq!(
        bytes.get(name - 4..name),
        Some(1_u32.to_le_bytes().as_slice())
    );
    assert_eq!(bytes.get(name), Some(&b'k'));
    let mut changed = bytes;
    if let Some(byte) = changed.get_mut(name) {
        *byte = 0xFF;
    }
    assert_eq!(
        detail_of(&changed),
        "cannot load a precompiled module: the encoded snapshot is truncated or malformed"
    );
}

fn nested(depth: u32) -> Shape {
    (0..depth).fold(Shape::Scalar, |shape, _| array(1, 4, shape))
}

fn nested_structure(depth: u32) -> Structure {
    (0..depth).fold(
        Structure {
            name: "Leaf".to_owned(),
            size: 4,
            fields: Vec::new(),
        },
        |inner, _| Structure {
            name: "Outer".to_owned(),
            size: 4,
            fields: vec![field("inner", 0, FieldType::Structure(inner))],
        },
    )
}

fn with_value_shape(shape: Shape) -> Compiled {
    Compiled {
        kernels: vec![kernel(
            "k",
            Vec::new(),
            Some(UniformBlock {
                size: 4,
                values: vec![value("v", 0, Scalar::Float32, shape)],
            }),
        )],
        render: Vec::new(),
    }
}

fn with_element(structure: Structure) -> Compiled {
    Compiled {
        kernels: vec![kernel(
            "k",
            vec![binding(
                "b",
                0,
                Usage::Used,
                Resource::StorageBuffer {
                    access: Access::ReadOnly,
                    stride: 4,
                    element: ElementType::Structure(structure),
                },
            )],
            None,
        )],
        render: Vec::new(),
    }
}

#[test]
fn nesting_up_to_the_deepest_level_round_trips_and_deeper_nesting_does_not_encode() {
    let deepest = MAX_DEPTH - 1;
    let shape = with_value_shape(nested(deepest));
    assert_eq!(round_trip(&shape), shape);
    let structure = with_element(nested_structure(deepest));
    assert_eq!(round_trip(&structure), structure);
    for compiled in [
        with_value_shape(nested(MAX_DEPTH)),
        with_element(nested_structure(MAX_DEPTH)),
    ] {
        let error = encode(&compiled).err();
        assert_eq!(
            error.as_ref().map(Error::kind),
            Some(ErrorKind::UnsupportedParameter),
            "{error:?}"
        );
    }
}

#[test]
fn nesting_deeper_than_the_deepest_level_does_not_decode() {
    let mut array = Vec::new();
    for _ in 0..=MAX_DEPTH {
        array.push(3);
        array.extend_from_slice(&1_u64.to_le_bytes());
        array.extend_from_slice(&4_u64.to_le_bytes());
    }
    array.push(0);
    assert_eq!(Reader { bytes: &array }.shape(0), None);
    let within = array.get(34..).unwrap_or_default();
    assert!(Reader { bytes: within }.shape(0).is_some());
    let mut structure = Vec::new();
    for _ in 0..=MAX_DEPTH {
        structure.extend_from_slice(&0_u32.to_le_bytes());
        structure.extend_from_slice(&4_u64.to_le_bytes());
        structure.extend_from_slice(&1_u32.to_le_bytes());
        structure.extend_from_slice(&0_u32.to_le_bytes());
        structure.extend_from_slice(&0_u64.to_le_bytes());
        structure.push(1);
    }
    structure.extend_from_slice(&0_u32.to_le_bytes());
    structure.extend_from_slice(&4_u64.to_le_bytes());
    structure.extend_from_slice(&0_u32.to_le_bytes());
    assert_eq!(Reader { bytes: &structure }.structure(0), None);
    let within = structure.get(2 * 29..).unwrap_or_default();
    assert!(Reader { bytes: within }.structure(0).is_some());
}
