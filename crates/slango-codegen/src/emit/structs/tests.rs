use std::path::PathBuf;

use slango::__private::{
    Access, ElementType, Field, FieldType, Interface, KernelInterface, Parameter, ParameterKind,
    Precompiled, Scalar, Shape, Structure, UniformStructure, ValueType,
};

use crate::{ErrorKind, generate_precompiled};

fn value(scalar: Scalar, shape: Shape) -> ValueType {
    ValueType { scalar, shape }
}

fn float(length: u32) -> ValueType {
    if length == 1 {
        value(Scalar::Float32, Shape::Scalar)
    } else {
        value(Scalar::Float32, Shape::Vector { length })
    }
}

fn field(name: &str, offset: u64, ty: FieldType) -> Field {
    Field {
        name: name.to_owned(),
        offset,
        ty,
    }
}

fn plain(name: &str, offset: u64, ty: ValueType) -> Field {
    field(name, offset, FieldType::Value(ty))
}

fn structure(name: &str, size: u64, fields: Vec<Field>) -> Structure {
    Structure {
        name: name.to_owned(),
        size,
        fields,
    }
}

/// The structure `Particle`: a `float3`, a `float`, and a `float2` at the
/// offsets 0, 12, and 16, with 8 bytes of tail padding.
fn particle() -> Structure {
    structure(
        "Particle",
        32,
        vec![
            plain("position", 0, float(3)),
            plain("mass", 12, float(1)),
            plain("velocity", 16, float(2)),
        ],
    )
}

fn motion() -> Structure {
    structure(
        "Motion",
        16,
        vec![plain("velocity", 0, float(2)), plain("drag", 8, float(1))],
    )
}

/// The structure `Body`, which holds `Motion` after a gap of 4 bytes.
fn body() -> Structure {
    structure(
        "Body",
        48,
        vec![
            plain("position", 0, float(3)),
            field("motion", 16, FieldType::Structure(motion())),
            plain("id", 32, value(Scalar::Uint32, Shape::Scalar)),
        ],
    )
}

fn buffer(path: &str, element: Structure) -> Parameter {
    Parameter {
        path: path.to_owned(),
        kind: ParameterKind::Buffer {
            access: Access::ReadWrite,
            element: ElementType::Structure(element),
        },
    }
}

fn leaf(path: &str, ty: ValueType) -> Parameter {
    Parameter {
        path: path.to_owned(),
        kind: ParameterKind::Value(ty),
    }
}

fn uniform(path: &str, name: &str) -> UniformStructure {
    UniformStructure {
        path: path.to_owned(),
        name: name.to_owned(),
    }
}

fn kernel(name: &str, parameters: Vec<Parameter>) -> KernelInterface {
    KernelInterface {
        name: name.to_owned(),
        thread_group_size: [64, 1, 1],
        parameters,
        structures: Vec::new(),
    }
}

fn with_structures(
    mut kernel: KernelInterface,
    structures: Vec<UniformStructure>,
) -> KernelInterface {
    kernel.structures = structures;
    kernel
}

fn precompiled(kernels: Vec<KernelInterface>) -> Precompiled {
    Precompiled {
        interface: Interface {
            kernels,
            dependencies: vec![PathBuf::from("/tmp/shaders/structs.slang")],
        },
        encoded: vec![1, 2, 3],
    }
}

fn format(kernels: Vec<KernelInterface>) -> String {
    let tokens = generate_precompiled(&precompiled(kernels)).expect("the bindings");
    let file = syn::parse2::<syn::File>(tokens).expect("the bindings parse as a Rust file");
    prettyplease::unparse(&file)
}

fn error(kernels: Vec<KernelInterface>) -> (ErrorKind, String) {
    let error = generate_precompiled(&precompiled(kernels)).expect_err("the generator rejects it");
    (error.kind(), error.to_string())
}

fn assert_contains(text: &str, lines: &[&str]) {
    for line in lines {
        assert!(text.contains(line), "{line}\n{text}");
    }
}

#[test]
fn an_element_gets_fields_at_its_offsets_with_tail_padding_and_layout_assertions() {
    let text = format(vec![kernel("step", vec![buffer("particles", particle())])]);
    assert_contains(
        &text,
        &[
            "#[repr(C)]\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct Particle {\n    /// The field `position` of the Slang structure.\n    pub position: [f32; 3],\n    /// The field `mass` of the Slang structure.\n    pub mass: f32,\n    /// The field `velocity` of the Slang structure.\n    pub velocity: [f32; 2],\n    _padding0: [u8; 8],\n}",
            "pub const fn new(position: [f32; 3], mass: f32, velocity: [f32; 2]) -> Self {\n        Self {\n            position,\n            mass,\n            velocity,\n            _padding0: [0; 8],\n        }\n    }",
            "::core::mem::size_of:: < Particle > () == 32,",
            "::core::mem::offset_of!(Particle, position) == 0,",
            "::core::mem::offset_of!(Particle, mass) == 12,",
            "::core::mem::offset_of!(Particle, velocity) == 16,",
            "impl ::slango::__private::StructElement for Particle {\n    const SLANG_NAME: &'static str = \"Particle\";\n    const STRIDE: usize = 32;",
            "pub particles: &'a ::slango::Buffer<Particle>,",
        ],
    );
}

#[test]
fn write_bytes_appends_each_scalar_and_zero_padding_in_the_order_of_the_offsets() {
    let text = format(vec![kernel("step", vec![buffer("particles", particle())])]);
    assert_contains(
        &text,
        &[
            "fn write_bytes(&self, out: &mut ::std::vec::Vec<u8>) {\n        for item0 in self.position {\n            out.extend_from_slice(&item0.to_le_bytes());\n        }\n        out.extend_from_slice(&self.mass.to_le_bytes());\n        for item0 in self.velocity {\n            out.extend_from_slice(&item0.to_le_bytes());\n        }\n        out.extend_from_slice(&[0_u8; 8]);\n    }",
        ],
    );
}

#[test]
fn read_bytes_reads_each_scalar_at_its_offset_after_checking_the_length() {
    let text = format(vec![kernel("step", vec![buffer("particles", particle())])]);
    assert_contains(
        &text,
        &[
            "if bytes.len() != 32 {\n            return ::core::option::Option::None;\n        }",
            "position: [\n                f32::from_le_bytes(*bytes.get(0..)?.first_chunk()?),\n                f32::from_le_bytes(*bytes.get(4..)?.first_chunk()?),\n                f32::from_le_bytes(*bytes.get(8..)?.first_chunk()?),\n            ],",
            "mass: f32::from_le_bytes(*bytes.get(12..)?.first_chunk()?),",
            "f32::from_le_bytes(*bytes.get(20..)?.first_chunk()?),\n            ],\n            _padding0: [0; 8],",
        ],
    );
}

#[test]
fn a_gap_between_fields_gets_its_own_padding_and_an_array_is_read_at_its_stride() {
    let grid = value(
        Scalar::Int32,
        Shape::Array {
            length: 2,
            stride: 8,
            element: Box::new(Shape::Array {
                length: 2,
                stride: 4,
                element: Box::new(Shape::Scalar),
            }),
        },
    );
    let gap = structure(
        "Gap",
        32,
        vec![
            plain("weight", 0, float(1)),
            plain(
                "colour",
                16,
                value(Scalar::Uint32, Shape::Vector { length: 4 }),
            ),
        ],
    );
    let grid = structure("Grid", 16, vec![plain("cells", 0, grid)]);
    let text = format(vec![kernel(
        "main",
        vec![buffer("gaps", gap), buffer("grids", grid)],
    )]);
    assert_contains(
        &text,
        &[
            "pub weight: f32,\n    _padding0: [u8; 12],\n    /// The field `colour` of the Slang structure.\n    pub colour: [u32; 4],\n}",
            "out.extend_from_slice(&self.weight.to_le_bytes());\n        out.extend_from_slice(&[0_u8; 12]);\n        for item0 in self.colour {",
            "pub cells: [[i32; 2]; 2],\n}",
            "for item0 in self.cells {\n            for item1 in item0 {\n                out.extend_from_slice(&item1.to_le_bytes());\n            }\n        }",
            "i32::from_le_bytes(*bytes.get(12..)?.first_chunk()?),\n                ],\n            ],",
        ],
    );
}

#[test]
fn a_nested_element_is_its_own_struct_written_and_read_through_its_implementation() {
    let text = format(vec![kernel("settle", vec![buffer("bodies", body())])]);
    assert_contains(
        &text,
        &[
            "pub struct Motion {",
            "pub position: [f32; 3],\n    _padding0: [u8; 4],\n    /// The field `motion` of the Slang structure.\n    pub motion: Motion,\n    /// The field `id` of the Slang structure.\n    pub id: u32,\n    _padding1: [u8; 12],\n}",
            "::core::mem::offset_of!(Body, motion) == 16,",
            "::slango::__private::StructElement::write_bytes(&self.motion, out);",
            "motion: <Motion as ::slango::__private::StructElement>::read_bytes(\n                bytes.get(16..32)?,\n            )?,",
            "impl ::slango::__private::StructElement for Motion {",
        ],
    );
    assert!(
        text.find("pub struct Motion").unwrap() < text.find("pub struct Body").unwrap(),
        "{text}"
    );
}

#[test]
fn a_structure_that_two_kernels_and_a_nested_field_use_is_emitted_once() {
    let text = format(vec![
        kernel("push", vec![buffer("motions", motion())]),
        kernel("settle", vec![buffer("bodies", body())]),
        kernel("again", vec![buffer("bodies", body())]),
    ]);
    assert_eq!(text.matches("pub struct Motion {").count(), 1, "{text}");
    assert_eq!(text.matches("pub struct Body {").count(), 1, "{text}");
    assert_contains(
        &text,
        &[
            "pub motions: &'a ::slango::Buffer<Motion>,",
            "pub bodies: &'a ::slango::Buffer<Body>,",
        ],
    );
}

fn light_leaves(path: &str) -> Vec<Parameter> {
    vec![
        leaf(&format!("{path}.colour"), float(3)),
        leaf(&format!("{path}.intensity"), float(1)),
    ]
}

/// A kernel with `uniform Params params`, whose structure `Params` holds a
/// `float`, a nested `Light`, a `bool`, and a matrix.
fn shade() -> KernelInterface {
    let mut parameters = vec![leaf("params.scale", float(1))];
    parameters.extend(light_leaves("params.light"));
    parameters.push(leaf("params.enabled", value(Scalar::Bool, Shape::Scalar)));
    parameters.push(leaf(
        "params.rotation",
        value(
            Scalar::Float32,
            Shape::Matrix {
                rows: 2,
                columns: 2,
            },
        ),
    ));
    parameters.push(leaf("count", value(Scalar::Uint32, Shape::Scalar)));
    with_structures(
        kernel("shade", parameters),
        vec![
            uniform("params", "Params"),
            uniform("params.light", "Light"),
        ],
    )
}

#[test]
fn a_uniform_structure_is_a_plain_struct_whose_leaves_are_set_by_their_paths() {
    let text = format(vec![shade()]);
    assert_contains(
        &text,
        &[
            "/// A dispatch sets each field of the value by its path, so the\n/// struct has no padding.\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct Params {\n    /// The field `scale` of the Slang structure.\n    pub scale: f32,\n    /// The field `light` of the Slang structure.\n    pub light: Light,\n    /// The field `enabled` of the Slang structure.\n    pub enabled: bool,\n    /// The field `rotation` of the Slang structure.\n    pub rotation: [[f32; 2]; 2],\n}",
            "pub const fn new(\n        scale: f32,\n        light: Light,\n        enabled: bool,\n        rotation: [[f32; 2]; 2],\n    ) -> Self {",
            "pub struct Light {\n    /// The field `colour` of the Slang structure.\n    pub colour: [f32; 3],\n    /// The field `intensity` of the Slang structure.\n    pub intensity: f32,\n}",
            "pub struct ShadeArgs {\n    /// The value of the parameter `params`, whose fields the call sets one by one.\n    pub params: Params,\n    /// The value of the parameter `count`.\n    pub count: u32,\n}",
            ".set(\"params.scale\", args.params.scale)\n            .set(\"params.light.colour\", args.params.light.colour)\n            .set(\"params.light.intensity\", args.params.light.intensity)\n            .set(\"params.enabled\", args.params.enabled)\n            .set(\"params.rotation\", args.params.rotation)\n            .set(\"count\", args.count)",
        ],
    );
    assert!(!text.contains("repr(C)"), "{text}");
    assert!(!text.contains("StructElement"), "{text}");
}

#[test]
fn a_uniform_structure_inside_a_constant_buffer_is_named_by_its_whole_path() {
    let mut parameters = vec![leaf("settings.tint", float(4))];
    parameters.extend(light_leaves("settings.light"));
    let text = format(vec![with_structures(
        kernel("tint", parameters),
        vec![uniform("settings.light", "Light")],
    )]);
    assert_contains(
        &text,
        &[
            "pub settings_tint: [f32; 4],\n    /// The value of the parameter `settings.light`, whose fields the call sets one by one.\n    pub settings_light: Light,",
            ".set(\"settings.light.intensity\", args.settings_light.intensity)",
        ],
    );
}

#[test]
fn an_element_that_is_also_a_uniform_value_is_emitted_once_in_the_element_form() {
    let light = structure(
        "Light",
        16,
        vec![
            plain("colour", 0, float(3)),
            plain("intensity", 12, float(1)),
        ],
    );
    let uniform_light = with_structures(
        kernel("glow", light_leaves("extra")),
        vec![uniform("extra", "Light")],
    );
    for kernels in [
        vec![
            kernel("lit", vec![buffer("lights", light.clone())]),
            uniform_light.clone(),
        ],
        vec![
            uniform_light.clone(),
            kernel("lit", vec![buffer("lights", light.clone())]),
        ],
    ] {
        let text = format(kernels);
        assert_eq!(text.matches("pub struct Light {").count(), 1, "{text}");
        assert_contains(
            &text,
            &[
                "#[repr(C)]\n#[derive(Clone, Copy, Debug, PartialEq)]\npub struct Light {",
                "impl ::slango::__private::StructElement for Light {",
                "pub extra: Light,",
                ".set(\"extra.intensity\", args.extra.intensity)",
            ],
        );
    }
}

#[test]
fn a_uniform_structure_without_leaves_still_gets_its_field() {
    let text = format(vec![with_structures(
        kernel(
            "main",
            vec![leaf("count", value(Scalar::Uint32, Shape::Scalar))],
        ),
        vec![uniform("marker", "Marker")],
    )]);
    assert_contains(
        &text,
        &[
            "pub struct Marker {}",
            "pub count: u32,\n    /// The value of the parameter `marker`, whose fields the call sets one by one.\n    pub marker: Marker,\n}",
        ],
    );
}

fn unsupported_field(ty: FieldType) -> (ErrorKind, String) {
    error(vec![kernel(
        "step",
        vec![buffer(
            "particles",
            structure(
                "Particle",
                16,
                vec![plain("mass", 0, float(1)), field("extra", 4, ty)],
            ),
        )],
    )])
}

fn unsupported_message(what: &str) -> (ErrorKind, String) {
    (
        ErrorKind::Unsupported,
        format!(
            "cannot generate the bindings: the parameter `particles` of the kernel `step` is not supported, because the field `extra` of the structure `Particle` is {what}, which a generated struct does not support"
        ),
    )
}

#[test]
fn a_bool_field_of_an_element_is_unsupported() {
    assert_eq!(
        unsupported_field(FieldType::Value(value(Scalar::Bool, Shape::Scalar))),
        unsupported_message("a `bool`")
    );
    assert_eq!(
        unsupported_field(FieldType::Value(value(
            Scalar::Bool,
            Shape::Vector { length: 2 }
        ))),
        unsupported_message("made of `bool` values")
    );
}

#[test]
fn a_matrix_field_of_an_element_is_unsupported() {
    assert_eq!(
        unsupported_field(FieldType::Value(value(
            Scalar::Float32,
            Shape::Matrix {
                rows: 2,
                columns: 2
            }
        ))),
        unsupported_message("a matrix")
    );
}

#[test]
fn an_array_field_whose_stride_exceeds_its_element_is_unsupported() {
    let array = Shape::Array {
        length: 2,
        stride: 16,
        element: Box::new(Shape::Vector { length: 3 }),
    };
    assert_eq!(
        unsupported_field(FieldType::Value(value(Scalar::Float32, array))),
        unsupported_message(
            "an array whose stride of 16 bytes differs from the 12 bytes of its element"
        )
    );
}

#[test]
fn a_field_of_a_16_bit_or_64_bit_scalar_or_an_array_of_structures_is_unsupported() {
    for name in ["half", "vector<double,2>", "uint64_t", "Motion[2]"] {
        assert_eq!(
            unsupported_field(FieldType::Other {
                name: name.to_owned()
            }),
            unsupported_message(&format!("of the type `{name}`"))
        );
    }
}

#[test]
fn an_unsupported_field_of_a_nested_structure_names_the_nested_structure() {
    let inner = structure(
        "Motion",
        16,
        vec![plain("alive", 0, value(Scalar::Bool, Shape::Scalar))],
    );
    let outer = structure(
        "Body",
        16,
        vec![field("motion", 0, FieldType::Structure(inner))],
    );
    assert_eq!(
        error(vec![kernel("settle", vec![buffer("bodies", outer)])]),
        (
            ErrorKind::Unsupported,
            "cannot generate the bindings: the parameter `bodies` of the kernel `settle` is not supported, because the field `alive` of the structure `Motion` is a `bool`, which a generated struct does not support".to_owned()
        )
    );
}

#[test]
fn an_element_of_0_bytes_or_with_overlapping_fields_is_unsupported() {
    let message = |reason: &str| {
        (
            ErrorKind::Unsupported,
            format!(
                "cannot generate the bindings: the parameter `particles` of the kernel `step` is not supported, because {reason}"
            ),
        )
    };
    assert_eq!(
        error(vec![kernel(
            "step",
            vec![buffer("particles", structure("Empty", 0, Vec::new()))]
        )]),
        message("it is a structured buffer of the structure `Empty`, whose size is 0 bytes")
    );
    assert_eq!(
        error(vec![kernel(
            "step",
            vec![buffer(
                "particles",
                structure(
                    "Particle",
                    16,
                    vec![plain("position", 0, float(3)), plain("mass", 8, float(1))]
                )
            )]
        )]),
        message(
            "the field `mass` of the structure `Particle` is at the offset 8, inside the field before it, which a generated struct does not support"
        )
    );
}

#[test]
fn an_element_whose_size_ends_before_its_last_field_is_unsupported() {
    assert_eq!(
        error(vec![kernel(
            "step",
            vec![buffer(
                "particles",
                structure(
                    "Particle",
                    8,
                    vec![plain("mass", 0, float(1)), plain("velocity", 4, float(2))]
                )
            )]
        )]),
        (
            ErrorKind::Unsupported,
            "cannot generate the bindings: the parameter `particles` of the kernel `step` is not supported, because the structure `Particle` has a size of 8 bytes, which ends before its last field".to_owned()
        )
    );
}

#[test]
fn a_resource_inside_a_uniform_structure_is_unsupported() {
    let kernel = with_structures(
        kernel(
            "sample",
            vec![
                leaf("material.scale", float(1)),
                Parameter {
                    path: "material.image".to_owned(),
                    kind: ParameterKind::Texture,
                },
            ],
        ),
        vec![uniform("material", "Material")],
    );
    assert_eq!(
        error(vec![kernel]),
        (
            ErrorKind::Unsupported,
            "cannot generate the bindings: the parameter `material.image` of the kernel `sample` is not supported, because it is a resource inside the structure `Material`, which a generated struct does not hold".to_owned()
        )
    );
}

#[test]
fn a_keyword_field_becomes_a_raw_identifier_in_the_struct_and_in_the_call() {
    let element = structure(
        "Tagged",
        8,
        vec![
            plain("type", 0, value(Scalar::Uint32, Shape::Scalar)),
            plain("matchCount", 4, value(Scalar::Uint32, Shape::Scalar)),
        ],
    );
    let uniform_kernel = with_structures(
        kernel(
            "pick",
            vec![
                leaf("filter.type", value(Scalar::Uint32, Shape::Scalar)),
                leaf("filter.matchCount", value(Scalar::Uint32, Shape::Scalar)),
            ],
        ),
        vec![uniform("filter", "Tagged")],
    );
    let text = format(vec![
        kernel("tag", vec![buffer("tags", element)]),
        uniform_kernel,
    ]);
    assert_contains(
        &text,
        &[
            "pub r#type: u32,",
            "pub match_count: u32,",
            "::core::mem::offset_of!(Tagged, r#type) == 0,",
            "::core::mem::offset_of!(Tagged, match_count) == 4,",
            ".set(\"filter.type\", args.filter.r#type)",
            ".set(\"filter.matchCount\", args.filter.match_count)",
        ],
    );
}

#[test]
fn two_fields_with_one_rust_name_conflict() {
    let element = structure(
        "Pair",
        8,
        vec![plain("fooBar", 0, float(1)), plain("foo_bar", 4, float(1))],
    );
    assert_eq!(
        error(vec![kernel("main", vec![buffer("pairs", element)])]),
        (
            ErrorKind::NameConflict,
            "cannot name an item of the bindings: the field `fooBar` of the structure `Pair` and the field `foo_bar` of the structure `Pair` both become the Rust name `foo_bar`".to_owned()
        )
    );
}

#[test]
fn a_uniform_structure_named_like_a_parameter_conflicts() {
    let with_leaf = with_structures(
        kernel(
            "main",
            vec![
                leaf("cb.params.scale", float(1)),
                leaf("cb_params", float(1)),
            ],
        ),
        vec![uniform("cb.params", "Params")],
    );
    let without_leaves = with_structures(
        kernel("main", vec![leaf("Marker", float(1))]),
        vec![uniform("marker", "Marker")],
    );
    for (kernel, detail) in [
        (
            with_leaf,
            "the parameter `cb.params` of the kernel `main` and the parameter `cb_params` of the kernel `main` both become the Rust name `cb_params`",
        ),
        (
            without_leaves,
            "the parameter `Marker` of the kernel `main` and the parameter `marker` of the kernel `main` both become the Rust name `marker`",
        ),
    ] {
        assert_eq!(
            error(vec![kernel]),
            (
                ErrorKind::NameConflict,
                format!("cannot name an item of the bindings: {detail}")
            )
        );
    }
}

#[test]
fn a_leaf_inside_a_uniform_structure_that_cannot_be_a_rust_identifier_conflicts() {
    let kernel = with_structures(
        kernel("main", vec![leaf("params.super", float(1))]),
        vec![uniform("params", "Params")],
    );
    assert_eq!(
        error(vec![kernel]),
        (
            ErrorKind::NameConflict,
            "cannot name an item of the bindings: the parameter `params.super` of the kernel `main` becomes the Rust name `super`, which is not a Rust identifier".to_owned()
        )
    );
}

#[test]
fn a_field_named_like_the_padding_conflicts() {
    let element = structure("Padded", 8, vec![plain("_padding0", 4, float(1))]);
    assert_eq!(
        error(vec![kernel("main", vec![buffer("data", element)])]),
        (
            ErrorKind::NameConflict,
            "cannot name an item of the bindings: the field `_padding0` of the structure `Padded` and the padding of the structure `Padded` both become the Rust name `_padding0`".to_owned()
        )
    );
}

#[test]
fn a_structure_named_like_a_kernel_or_shader_conflicts() {
    let cases = [
        (
            "Step",
            "the kernel `step` and the structure `Step` both become the Rust name `Step`",
        ),
        (
            "StepArgs",
            "the arguments of the kernel `step` and the structure `StepArgs` both become the Rust name `StepArgs`",
        ),
        (
            "Shader",
            "the struct `Shader` and the structure `Shader` both become the Rust name `Shader`",
        ),
    ];
    for (name, detail) in cases {
        let element = structure(name, 4, vec![plain("mass", 0, float(1))]);
        assert_eq!(
            error(vec![kernel("step", vec![buffer("data", element)])]),
            (
                ErrorKind::NameConflict,
                format!("cannot name an item of the bindings: {detail}")
            )
        );
    }
}

#[test]
fn a_structure_name_that_is_no_rust_identifier_conflicts() {
    let element = structure("Pair<float>", 4, vec![plain("first", 0, float(1))]);
    assert_eq!(
        error(vec![kernel("main", vec![buffer("pairs", element)])]),
        (
            ErrorKind::NameConflict,
            "cannot name an item of the bindings: the structure `Pair<float>` becomes the Rust name `Pair<float>`, which is not a Rust identifier".to_owned()
        )
    );
}

#[test]
fn two_different_structures_with_one_name_conflict() {
    let first = structure("Item", 4, vec![plain("weight", 0, float(1))]);
    let second = structure(
        "Item",
        4,
        vec![plain("count", 0, value(Scalar::Uint32, Shape::Scalar))],
    );
    let moved = structure("Item", 8, vec![plain("weight", 4, float(1))]);
    let detail = "cannot name an item of the bindings: the structure `Item` of the parameter `items` of the kernel `first` and the structure `Item` of the parameter `others` of the kernel `second` are different structures that both become the Rust name `Item`";
    for other in [second, moved] {
        assert_eq!(
            error(vec![
                kernel("first", vec![buffer("items", first.clone())]),
                kernel("second", vec![buffer("others", other)]),
            ]),
            (ErrorKind::NameConflict, detail.to_owned())
        );
    }
    let uniform_item = with_structures(
        kernel(
            "second",
            vec![leaf("others.count", value(Scalar::Uint32, Shape::Scalar))],
        ),
        vec![uniform("others", "Item")],
    );
    assert_eq!(
        error(vec![
            kernel("first", vec![buffer("items", first)]),
            uniform_item
        ]),
        (ErrorKind::NameConflict, detail.to_owned())
    );
}

#[test]
fn new_of_more_than_seven_fields_expects_the_lint_of_too_many_arguments() {
    let names = ["a", "b", "c", "d", "e", "f", "g", "h"];
    let fields = |count: usize| {
        names
            .iter()
            .take(count)
            .zip(0..)
            .map(|(name, index)| plain(name, index * 4, float(1)))
            .collect::<Vec<_>>()
    };
    let expect = "#[expect(\n        clippy::too_many_arguments,\n        reason = \"`new` takes one argument for each field of the Slang structure\"\n    )]\n    pub const fn new(";
    let eight = structure("Eight", 32, fields(8));
    let seven = structure("Seven", 28, fields(7));
    let text = format(vec![kernel(
        "main",
        vec![buffer("eights", eight), buffer("sevens", seven)],
    )]);
    assert_eq!(text.matches(expect).count(), 1, "{text}");
    assert!(
        text.find(expect).unwrap() < text.find("pub struct Seven").unwrap(),
        "{text}"
    );
    let mut parameters: Vec<Parameter> = names
        .iter()
        .map(|name| leaf(&format!("wide.{name}"), float(1)))
        .collect();
    parameters.truncate(8);
    let text = format(vec![with_structures(
        kernel("main", parameters),
        vec![uniform("wide", "Wide")],
    )]);
    assert_eq!(text.matches(expect).count(), 1, "{text}");
}
