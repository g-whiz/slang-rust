use std::path::PathBuf;

use slango::__private::{
    Access, ElementType, Interface, KernelInterface, Parameter, ParameterKind, Precompiled, Scalar,
    Shape, ValueType,
};

use crate::{ErrorKind, generate_precompiled};

fn value(scalar: Scalar, shape: Shape) -> ParameterKind {
    ParameterKind::Value(ValueType { scalar, shape })
}

fn scalar(scalar: Scalar) -> ParameterKind {
    value(scalar, Shape::Scalar)
}

fn buffer(access: Access, scalar: Scalar, components: u32) -> ParameterKind {
    ParameterKind::Buffer {
        access,
        element: ElementType::Plain { scalar, components },
    }
}

fn parameter(path: &str, kind: ParameterKind) -> Parameter {
    Parameter {
        path: path.to_owned(),
        kind,
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

fn precompiled(kernels: Vec<KernelInterface>) -> Precompiled {
    Precompiled {
        interface: Interface {
            kernels,
            dependencies: vec![PathBuf::from("/tmp/shaders/saxpy.slang")],
        },
        encoded: vec![1, 2, 3],
    }
}

fn format(precompiled: &Precompiled) -> String {
    let tokens = generate_precompiled(precompiled).expect("the bindings of the interface");
    let file = syn::parse2::<syn::File>(tokens).expect("the bindings parse as a Rust file");
    prettyplease::unparse(&file)
}

fn error(precompiled: &Precompiled) -> (ErrorKind, String) {
    let error = generate_precompiled(precompiled).expect_err("the generator rejects the interface");
    (error.kind(), error.to_string())
}

fn saxpy() -> KernelInterface {
    kernel(
        "saxpy",
        vec![
            parameter("x", buffer(Access::ReadOnly, Scalar::Float32, 1)),
            parameter("y", buffer(Access::ReadOnly, Scalar::Float32, 1)),
            parameter("result", buffer(Access::ReadWrite, Scalar::Float32, 1)),
            parameter("a", scalar(Scalar::Float32)),
            parameter("count", scalar(Scalar::Uint32)),
        ],
    )
}

const SAXPY: &str = r#"const _: &[u8] = ::core::include_bytes!("/tmp/shaders/saxpy.slang");
const _: () = ::core::assert!(
    ::slango::__private::ENCODING_VERSION == 1,
    "the bindings were generated for another version of slango, so the versions of slango and slango-macros differ"
);
/// The arguments of the kernel `saxpy`, one field for each parameter.
///
/// Each field holds the value or the resource that the call gives the
/// parameter of the same name. A parameter that the compiled kernel
/// does not use has a field too, so the fields do not change when the
/// compiler drops a parameter.
#[derive(Clone, Copy, Debug)]
pub struct SaxpyArgs<'a> {
    /// The buffer bound to the parameter `x`, which the kernel reads.
    pub x: &'a ::slango::Buffer<f32>,
    /// The buffer bound to the parameter `y`, which the kernel reads.
    pub y: &'a ::slango::Buffer<f32>,
    /// The buffer bound to the parameter `result`, which the kernel reads and writes.
    pub result: &'a ::slango::Buffer<f32>,
    /// The value of the parameter `a`.
    pub a: f32,
    /// The value of the parameter `count`.
    pub count: u32,
}
"#;

#[test]
fn saxpy_gives_tracking_the_version_check_and_typed_arguments() {
    let text = format(&precompiled(vec![saxpy()]));
    assert!(text.starts_with(SAXPY), "{text}");
    assert!(
        text.contains("pub const THREAD_GROUP_SIZE: [u32; 3] = [64, 1, 1];"),
        "{text}"
    );
    assert!(
        text.contains(
            "pub fn dispatch(\n        &self,\n        args: &SaxpyArgs<'_>,\n        elements: [u32; 3],\n    ) -> ::slango::Result<()> {\n        self.kernel\n            .call()\n            .bind(\"x\", args.x)\n            .bind(\"y\", args.y)\n            .bind(\"result\", args.result)\n            .set(\"a\", args.a)\n            .set(\"count\", args.count)\n            .dispatch(elements)\n    }"
        ),
        "{text}"
    );
    assert!(text.contains(".dispatch_groups(groups)"), "{text}");
    assert!(
        text.contains("pub struct Shader {\n    /// The kernel `saxpy`.\n    pub saxpy: Saxpy,\n    module: ::slango::Module,\n}"),
        "{text}"
    );
    assert!(
        text.contains(
            "let module = ::slango::__private::load_precompiled(device, b\"\\x01\\x02\\x03\")?;\n        ::core::result::Result::Ok(Self {\n            saxpy: Saxpy {\n                kernel: module.kernel(\"saxpy\")?.clone(),\n            },\n            module,\n        })"
        ),
        "{text}"
    );
}

#[test]
fn each_scalar_maps_to_its_rust_type_and_is_set() {
    let text = format(&precompiled(vec![kernel(
        "scalars",
        vec![
            parameter("flag", scalar(Scalar::Bool)),
            parameter("offset", scalar(Scalar::Int32)),
            parameter("count", scalar(Scalar::Uint32)),
            parameter("scale", scalar(Scalar::Float32)),
        ],
    )]));
    for (field, ty) in [
        ("flag", "bool"),
        ("offset", "i32"),
        ("count", "u32"),
        ("scale", "f32"),
    ] {
        assert!(text.contains(&format!("pub {field}: {ty},")), "{text}");
        assert!(
            text.contains(&format!(".set(\"{field}\", args.{field})")),
            "{text}"
        );
    }
    assert!(text.contains("pub struct ScalarsArgs {"), "{text}");
    assert!(text.contains("args: &ScalarsArgs,"), "{text}");
}

#[test]
fn vectors_matrices_and_arrays_map_to_arrays_nested_from_the_outermost_length() {
    let array = |length, element| Shape::Array {
        length,
        stride: 16,
        element: Box::new(element),
    };
    let text = format(&precompiled(vec![kernel(
        "shapes",
        vec![
            parameter(
                "direction",
                value(Scalar::Float32, Shape::Vector { length: 3 }),
            ),
            parameter(
                "transform",
                value(
                    Scalar::Float32,
                    Shape::Matrix {
                        rows: 2,
                        columns: 3,
                    },
                ),
            ),
            parameter(
                "pairs",
                value(Scalar::Uint32, array(4, Shape::Vector { length: 2 })),
            ),
            parameter(
                "grid",
                value(Scalar::Int32, array(2, array(3, Shape::Scalar))),
            ),
        ],
    )]));
    for line in [
        "pub direction: [f32; 3],",
        "pub transform: [[f32; 3]; 2],",
        "pub pairs: [[u32; 2]; 4],",
        "pub grid: [[i32; 3]; 2],",
        ".set(\"transform\", args.transform)",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
}

#[test]
fn buffers_of_built_in_elements_are_borrowed_and_bound() {
    let text = format(&precompiled(vec![kernel(
        "buffers",
        vec![
            parameter("ints", buffer(Access::ReadOnly, Scalar::Int32, 1)),
            parameter("uints", buffer(Access::ReadOnly, Scalar::Uint32, 1)),
            parameter("floats4", buffer(Access::ReadWrite, Scalar::Float32, 4)),
            parameter("ints4", buffer(Access::ReadWrite, Scalar::Int32, 4)),
            parameter("uints4", buffer(Access::ReadWrite, Scalar::Uint32, 4)),
        ],
    )]));
    for line in [
        "pub ints: &'a ::slango::Buffer<i32>,",
        "pub uints: &'a ::slango::Buffer<u32>,",
        "pub floats4: &'a ::slango::Buffer<[f32; 4]>,",
        "pub ints4: &'a ::slango::Buffer<[i32; 4]>,",
        "pub uints4: &'a ::slango::Buffer<[u32; 4]>,",
        ".bind(\"floats4\", args.floats4)",
        "The buffer bound to the parameter `ints`, which the kernel reads.",
        "The buffer bound to the parameter `uints4`, which the kernel reads and writes.",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
}

#[test]
fn textures_and_samplers_are_borrowed_and_given_by_their_own_calls() {
    let text = format(&precompiled(vec![kernel(
        "sample",
        vec![
            parameter("image", ParameterKind::Texture),
            parameter("linear", ParameterKind::Sampler),
        ],
    )]));
    for line in [
        "pub struct SampleArgs<'a> {",
        "pub image: &'a ::slango::Texture,",
        "pub linear: &'a ::slango::Sampler,",
        ".texture(\"image\", args.image)",
        ".sampler(\"linear\", args.linear)",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
}

#[test]
fn a_field_of_a_named_constant_buffer_is_its_own_field_set_by_its_path() {
    let text = format(&precompiled(vec![kernel(
        "tint",
        vec![parameter("Settings.colourScale", scalar(Scalar::Float32))],
    )]));
    assert!(text.contains("pub settings_colour_scale: f32,"), "{text}");
    assert!(
        text.contains(".set(\"Settings.colourScale\", args.settings_colour_scale)"),
        "{text}"
    );
}

#[test]
fn a_kernel_without_parameters_has_empty_arguments_that_its_dispatch_reads() {
    let text = format(&precompiled(vec![kernel("clear", Vec::new())]));
    assert!(text.contains("pub struct ClearArgs {}"), "{text}");
    assert!(
        text.contains("let ClearArgs {} = args;\n        self.kernel.call().dispatch(elements)"),
        "{text}"
    );
}

#[test]
fn shader_looks_up_every_kernel_under_its_snake_case_name() {
    let mut second = kernel("scaleTwice", Vec::new());
    second.thread_group_size = [8, 4, 2];
    let text = format(&precompiled(vec![saxpy(), second]));
    for line in [
        "pub saxpy: Saxpy,",
        "pub scale_twice: ScaleTwice,",
        "kernel: module.kernel(\"scaleTwice\")?.clone(),",
        "pub struct ScaleTwiceArgs {}",
        "pub const THREAD_GROUP_SIZE: [u32; 3] = [8, 4, 2];",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
}

#[test]
fn every_file_that_slang_read_is_included_to_track_it() {
    let mut precompiled = precompiled(Vec::new());
    precompiled.interface.dependencies = vec![
        PathBuf::from("/tmp/shaders/main.slang"),
        PathBuf::from("/tmp/shaders/common.slang"),
    ];
    let text = format(&precompiled);
    assert!(
        text.starts_with(
            "const _: &[u8] = ::core::include_bytes!(\"/tmp/shaders/main.slang\");\nconst _: &[u8] = ::core::include_bytes!(\"/tmp/shaders/common.slang\");\n"
        ),
        "{text}"
    );
}

#[cfg(unix)]
#[test]
fn a_file_whose_path_is_not_utf8_is_unsupported() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let mut precompiled = precompiled(Vec::new());
    precompiled.interface.dependencies = vec![PathBuf::from(OsStr::from_bytes(b"/tmp/\xff.slang"))];
    assert_eq!(
        error(&precompiled),
        (
            ErrorKind::Unsupported,
            "cannot generate the bindings: the path `/tmp/\u{fffd}.slang` of a file that Slang read is not valid UTF-8".to_owned()
        )
    );
}

#[test]
fn a_keyword_becomes_a_raw_identifier_set_by_its_slang_name() {
    let text = format(&precompiled(vec![kernel(
        "loop",
        vec![parameter("type", scalar(Scalar::Uint32))],
    )]));
    for line in [
        "pub r#type: u32,",
        ".set(\"type\", args.r#type)",
        "pub r#loop: Loop,",
    ] {
        assert!(text.contains(line), "{line}\n{text}");
    }
}

#[test]
fn a_buffer_without_a_built_in_element_type_is_unsupported() {
    let cases = [
        (
            ElementType::Plain {
                scalar: Scalar::Float32,
                components: 3,
            },
            "it is a structured buffer of `float3`, which has no built-in element type in slango",
        ),
        (
            ElementType::Plain {
                scalar: Scalar::Uint32,
                components: 2,
            },
            "it is a structured buffer of `uint2`, which has no built-in element type in slango",
        ),
        (
            ElementType::Plain {
                scalar: Scalar::Bool,
                components: 1,
            },
            "it is a structured buffer of `bool`, which has no built-in element type in slango",
        ),
        (
            ElementType::OtherPlain {
                name: "half".to_owned(),
            },
            "it is a structured buffer of `half`, which has no built-in element type in slango",
        ),
        (
            ElementType::OtherPlain {
                name: "vector<double,2>".to_owned(),
            },
            "it is a structured buffer of `vector<double,2>`, which has no built-in element type in slango",
        ),
        (
            ElementType::Composite,
            "it is a structured buffer of a matrix, an array, or another composite type, which has no built-in element type in slango",
        ),
    ];
    for (element, reason) in cases {
        let interface = precompiled(vec![kernel(
            "main",
            vec![parameter(
                "data",
                ParameterKind::Buffer {
                    access: Access::ReadWrite,
                    element,
                },
            )],
        )]);
        assert_eq!(
            error(&interface),
            (
                ErrorKind::Unsupported,
                format!(
                    "cannot generate the bindings: the parameter `data` of the kernel `main` is not supported, because {reason}"
                )
            )
        );
    }
}

#[test]
fn two_kernels_with_one_rust_name_conflict() {
    assert_eq!(
        error(&precompiled(vec![
            kernel("scale_twice", Vec::new()),
            kernel("scaleTwice", Vec::new()),
        ])),
        (
            ErrorKind::NameConflict,
            "cannot name an item of the bindings: the kernel `scale_twice` and the kernel `scaleTwice` both become the Rust name `ScaleTwice`".to_owned()
        )
    );
}

#[test]
fn a_kernel_named_like_the_arguments_of_another_conflicts() {
    assert_eq!(
        error(&precompiled(vec![
            kernel("scale", Vec::new()),
            kernel("scaleArgs", Vec::new()),
        ])),
        (
            ErrorKind::NameConflict,
            "cannot name an item of the bindings: the arguments of the kernel `scale` and the kernel `scaleArgs` both become the Rust name `ScaleArgs`".to_owned()
        )
    );
}

#[test]
fn a_kernel_named_like_an_item_of_shader_conflicts() {
    let cases = [
        (
            "shader",
            "the struct `Shader` and the kernel `shader` both become the Rust name `Shader`",
        ),
        (
            "load",
            "an item of `Shader` and the kernel `load` both become the Rust name `load`",
        ),
        (
            "module",
            "an item of `Shader` and the kernel `module` both become the Rust name `module`",
        ),
    ];
    for (name, detail) in cases {
        assert_eq!(
            error(&precompiled(vec![kernel(name, Vec::new())])),
            (
                ErrorKind::NameConflict,
                format!("cannot name an item of the bindings: {detail}")
            )
        );
    }
}

#[test]
fn two_parameters_with_one_rust_name_conflict() {
    assert_eq!(
        error(&precompiled(vec![kernel(
            "main",
            vec![
                parameter("cb.scale", scalar(Scalar::Float32)),
                parameter("cb_scale", scalar(Scalar::Float32)),
            ],
        )])),
        (
            ErrorKind::NameConflict,
            "cannot name an item of the bindings: the parameter `cb.scale` of the kernel `main` and the parameter `cb_scale` of the kernel `main` both become the Rust name `cb_scale`".to_owned()
        )
    );
}

#[test]
fn a_parameter_that_cannot_be_a_rust_identifier_conflicts() {
    assert_eq!(
        error(&precompiled(vec![kernel(
            "main",
            vec![parameter("self", scalar(Scalar::Float32))],
        )])),
        (
            ErrorKind::NameConflict,
            "cannot name an item of the bindings: the parameter `self` of the kernel `main` becomes the Rust name `self`, which is not a Rust identifier".to_owned()
        )
    );
}
