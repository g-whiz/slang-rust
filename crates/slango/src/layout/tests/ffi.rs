use std::error::Error as _;
use std::fmt::Write as _;

use super::super::{
    Access, Attribute, Binding, Compiled, CompiledKernel, CompiledRender, ElementType, Field,
    FieldType, KernelLayout, PlainValue, Rejection, RenderLayout, RenderStage, Resource, Scalar,
    Shape, Source, Structure, UniformBlock, UniformStructure, UsableRender, Usage,
};
use crate::{ErrorKind, Result};

fn compile_all(name: &str, text: &str) -> Result<Compiled> {
    let path = format!("{name}.slang");
    super::super::compile(&Source {
        name,
        path: &path,
        text,
        search_path: None,
    })
    .map(|compilation| compilation.compiled)
}

fn compile(name: &str, text: &str) -> Result<Vec<CompiledKernel>> {
    compile_all(name, text).map(|compiled| compiled.kernels)
}

fn kernels(source: &str) -> Vec<KernelLayout> {
    compile("kernels", source)
        .expect("the source compiles and every parameter is supported")
        .into_iter()
        .map(|kernel| kernel.layout)
        .collect()
}

fn kernel(source: &str) -> KernelLayout {
    let mut kernels = kernels(source);
    assert_eq!(kernels.len(), 1, "one kernel in {kernels:#?}");
    kernels.remove(0)
}

fn shape(shape: &Shape) -> String {
    match shape {
        Shape::Scalar => "scalar".to_owned(),
        Shape::Vector { length } => format!("vector {length}"),
        Shape::Matrix { rows, columns } => format!("matrix {rows}x{columns}"),
        Shape::Array {
            length,
            stride,
            element,
        } => format!("array {length} stride {stride} of {}", self::shape(element)),
    }
}

fn element_type(element: &ElementType) -> String {
    match element {
        ElementType::Plain { scalar, components } => format!("{scalar:?} x{components}"),
        ElementType::OtherPlain { name } => format!("other `{name}`"),
        ElementType::Structure(structure) => self::structure(structure),
        ElementType::Composite => "composite".to_owned(),
    }
}

fn structure(structure: &Structure) -> String {
    let Structure { name, size, fields } = structure;
    let fields = fields
        .iter()
        .map(|Field { name, offset, ty }| {
            let ty = match ty {
                FieldType::Plain { scalar, shape } => {
                    format!("{scalar:?} {}", self::shape(shape))
                }
                FieldType::Structure(inner) => self::structure(inner),
                FieldType::Other { name } => format!("other `{name}`"),
            };
            format!("+{offset} {name}: {ty}")
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("structure {name} of {size} bytes {{ {fields} }}")
}

fn render_block(out: &mut String, block: &UniformBlock) {
    let UniformBlock { size, values } = block;
    writeln!(out, "    size {size}").expect("writing to a string");
    for value in values {
        let PlainValue {
            path,
            offset,
            size,
            scalar,
            shape: value_shape,
        } = value;
        writeln!(
            out,
            "    +{offset} {size} bytes `{path}`: {scalar:?} {}",
            shape(value_shape)
        )
        .expect("writing to a string");
    }
}

fn render_bindings(out: &mut String, bindings: &[Binding], immediates: Option<&UniformBlock>) {
    for binding in bindings {
        let Binding {
            path,
            set,
            index,
            usage,
            resource,
        } = binding;
        match resource {
            Resource::StorageBuffer {
                access,
                stride,
                element,
            } => writeln!(
                out,
                "  set {set} binding {index} `{path}` {usage:?}: storage buffer {access:?}, stride {stride}, element {}",
                element_type(element)
            )
            .expect("writing to a string"),
            Resource::UniformBuffer(block) => {
                writeln!(
                    out,
                    "  set {set} binding {index} `{path}` {usage:?}: uniform buffer"
                )
                .expect("writing to a string");
                render_block(out, block);
            }
            Resource::SampledTexture => writeln!(
                out,
                "  set {set} binding {index} `{path}` {usage:?}: sampled texture"
            )
            .expect("writing to a string"),
            Resource::Sampler => writeln!(
                out,
                "  set {set} binding {index} `{path}` {usage:?}: sampler"
            )
            .expect("writing to a string"),
        }
    }
    if let Some(block) = immediates {
        writeln!(out, "  immediates").expect("writing to a string");
        render_block(out, block);
    }
}

fn render(kernels: &[KernelLayout]) -> String {
    let mut out = String::new();
    for kernel in kernels {
        let KernelLayout {
            name,
            thread_group_size,
            bindings,
            immediates,
            structures,
        } = kernel;
        writeln!(out, "kernel `{name}` thread group {thread_group_size:?}")
            .expect("writing to a string");
        render_bindings(&mut out, bindings, immediates.as_ref());
        for UniformStructure { path, name } in structures {
            writeln!(out, "  structure `{path}`: {name}").expect("writing to a string");
        }
    }
    out
}

fn render_entry_points(records: &[CompiledRender]) -> String {
    let mut out = String::new();
    for record in records {
        let CompiledRender {
            name,
            stage,
            usable,
        } = record;
        writeln!(out, "{stage:?} `{name}`").expect("writing to a string");
        match usable {
            Ok(UsableRender {
                layout:
                    RenderLayout {
                        bindings,
                        immediates,
                        attributes,
                    },
                spirv: _,
            }) => {
                render_bindings(&mut out, bindings, immediates.as_ref());
                for attribute in attributes {
                    let Attribute {
                        path,
                        location,
                        scalar,
                        components,
                        usage,
                    } = attribute;
                    writeln!(
                        out,
                        "  location {location} `{path}` {usage:?}: attribute {scalar:?} x{components}"
                    )
                    .expect("writing to a string");
                }
            }
            Err(Rejection { kind, detail }) => {
                writeln!(out, "  rejected {kind:?}: {detail}").expect("writing to a string");
            }
        }
    }
    out
}

fn value<'a>(block: &'a UniformBlock, path: &str) -> &'a PlainValue {
    block
        .values
        .iter()
        .find(|value| value.path == path)
        .unwrap_or_else(|| panic!("a plain value `{path}` in {block:#?}"))
}

fn offsets_and_size(block: &UniformBlock, paths: &[&str]) -> (Vec<u64>, u64) {
    let offsets = paths.iter().map(|path| value(block, path).offset).collect();
    (offsets, block.size)
}

fn uniform_buffer<'a>(kernel: &'a KernelLayout, path: &str) -> (&'a Binding, &'a UniformBlock) {
    for binding in &kernel.bindings {
        if let Resource::UniformBuffer(block) = &binding.resource
            && binding.path == path
        {
            return (binding, block);
        }
    }
    panic!("a uniform buffer `{path}` in {kernel:#?}");
}

fn binding<'a>(kernel: &'a KernelLayout, path: &str) -> &'a Binding {
    kernel
        .bindings
        .iter()
        .find(|binding| binding.path == path)
        .unwrap_or_else(|| panic!("a binding `{path}` in {kernel:#?}"))
}

const IMMEDIATES: &str = r#"
struct Pair { float first; uint second; };
RWStructuredBuffer<float> output;

[shader("compute")]
[numthreads(64, 1, 1)]
void run(
    uint3 id : SV_DispatchThreadID,
    uniform float scale,
    uniform float3 offset,
    uniform float4x4 transform,
    uniform Pair pair,
    uniform bool enabled)
{
    if (enabled) {
        output[id.x] = scale * offset.x * transform[0][0] * pair.first + float(pair.second);
    }
}
"#;

#[test]
fn uniform_parameters_of_the_entry_point_become_immediates_with_std430_offsets() {
    let kernel = kernel(IMMEDIATES);
    let immediates = kernel.immediates.as_ref().expect("immediates");
    assert_eq!(
        offsets_and_size(
            immediates,
            &[
                "scale",
                "offset",
                "transform",
                "pair.first",
                "pair.second",
                "enabled"
            ]
        ),
        (vec![0, 16, 32, 96, 100, 104], 112)
    );
    let output = binding(&kernel, "output");
    assert_eq!((output.set, output.index), (0, 0));
    insta::assert_snapshot!("immediates", render(&[kernel]));
}

const ELEMENTS: &str = r#"
struct Particle { float3 position; float mass; };
RWStructuredBuffer<float> floats;
RWStructuredBuffer<int> ints;
RWStructuredBuffer<uint4> uints;
RWStructuredBuffer<bool> flags;
RWStructuredBuffer<half2> halves;
RWStructuredBuffer<Particle> particles;
RWStructuredBuffer<float2x2> matrices;

[shader("compute")]
[numthreads(1, 1, 1)]
void run()
{
    floats[0] = float(ints[0]) + float(uints[0].x) + (flags[0] ? 1.0 : 0.0)
        + float(halves[0].x) + particles[0].mass + matrices[0][0][0];
}
"#;

#[test]
fn the_element_type_of_each_structured_buffer_is_recorded_by_its_kind() {
    let kernel = kernel(ELEMENTS);
    let element = |path: &str| match &binding(&kernel, path).resource {
        Resource::StorageBuffer { element, .. } => element.clone(),
        other => panic!("`{path}` is not a storage buffer: {other:?}"),
    };
    let plain = |scalar, components| ElementType::Plain { scalar, components };
    assert_eq!(element("floats"), plain(Scalar::Float32, 1));
    assert_eq!(element("ints"), plain(Scalar::Int32, 1));
    assert_eq!(element("uints"), plain(Scalar::Uint32, 4));
    assert_eq!(element("flags"), plain(Scalar::Bool, 1));
    assert!(matches!(element("halves"), ElementType::OtherPlain { .. }));
    let float = |length| FieldType::Plain {
        scalar: Scalar::Float32,
        shape: if length == 1 {
            Shape::Scalar
        } else {
            Shape::Vector { length }
        },
    };
    let field = |name: &str, offset, ty| Field {
        name: name.to_owned(),
        offset,
        ty,
    };
    assert_eq!(
        element("particles"),
        ElementType::Structure(Structure {
            name: "Particle".to_owned(),
            size: 16,
            fields: vec![field("position", 0, float(3)), field("mass", 12, float(1))],
        })
    );
    assert_eq!(element("matrices"), ElementType::Composite);
    insta::assert_snapshot!("elements", render(&[kernel]));
}

const STRUCTURE_ELEMENTS: &str = r#"
struct Inner { uint id; float2 pair; };
struct Particle { float3 position; float mass; float2 velocity; };
struct Mixed
{
    float3 values[2];
    Inner inner;
    bool flag;
    float2x2 rotation;
    half small;
    Inner inners[2];
    int count;
};
RWStructuredBuffer<Particle> particles;
StructuredBuffer<Mixed> mixed;

[shader("compute")]
[numthreads(1, 1, 1)]
void run()
{
    particles[0].mass = mixed[0].values[1].x + float(mixed[0].inner.id) + (mixed[0].flag ? 1.0 : 0.0)
        + mixed[0].rotation[0][0] + float(mixed[0].small) + float(mixed[0].inners[1].id)
        + float(mixed[0].count);
}
"#;

#[test]
fn a_structure_element_records_its_slang_name_size_and_each_field_at_its_offset() {
    let kernel = kernel(STRUCTURE_ELEMENTS);
    let structure = |path: &str| match &binding(&kernel, path).resource {
        Resource::StorageBuffer {
            element: ElementType::Structure(structure),
            stride,
            ..
        } => (structure.clone(), *stride),
        other => panic!("`{path}` is not a buffer of structures: {other:?}"),
    };
    let (particle, stride) = structure("particles");
    assert_eq!(particle.name, "Particle");
    assert_eq!((particle.size, stride), (32, 32));
    let offsets: Vec<(&str, u64)> = particle
        .fields
        .iter()
        .map(|field| (field.name.as_str(), field.offset))
        .collect();
    assert_eq!(offsets, [("position", 0), ("mass", 12), ("velocity", 16)]);
    let (mixed, _) = structure("mixed");
    let inner = mixed
        .fields
        .iter()
        .find(|field| field.name == "inner")
        .map(|field| &field.ty);
    assert!(
        matches!(inner, Some(FieldType::Structure(Structure { name, .. })) if name == "Inner"),
        "{inner:?}"
    );
    let small = mixed.fields.iter().find(|field| field.name == "small");
    assert!(
        matches!(small.map(|field| &field.ty), Some(FieldType::Other { .. })),
        "{small:?}"
    );
    insta::assert_snapshot!("structure_elements", render(&[kernel]));
}

const UNIFORM_STRUCTURES: &str = r#"
struct Inner { float gain; uint2 size; };
struct Params { float scale; Inner inner; };
struct Run { uint width; Inner inner; };
struct Extra { float bias; RWStructuredBuffer<float> output; };

RWStructuredBuffer<float> data;
uniform Params params;
ConstantBuffer<Params> settings;
ParameterBlock<Extra> extra;

[shader("compute")]
[numthreads(1, 1, 1)]
void run(uniform Run run)
{
    data[0] = params.scale + params.inner.gain + float(params.inner.size.x) + settings.scale
        + settings.inner.gain + float(run.width) + run.inner.gain + extra.bias;
    extra.output[0] = 1.0;
}
"#;

#[test]
fn each_structure_among_the_plain_values_is_recorded_with_its_path_and_slang_name() {
    let kernel = kernel(UNIFORM_STRUCTURES);
    let structures: Vec<(&str, &str)> = kernel
        .structures
        .iter()
        .map(|structure| (structure.path.as_str(), structure.name.as_str()))
        .collect();
    assert_eq!(
        structures,
        [
            ("params", "Params"),
            ("params.inner", "Inner"),
            ("settings.inner", "Inner"),
            ("run", "Run"),
            ("run.inner", "Inner"),
        ]
    );
    insta::assert_snapshot!("uniform_structures", render(&[kernel]));
}

const GLOBALS: &str = r#"
struct Params { float scale; float3 offset; uint count; float2 pair; };
StructuredBuffer<float4> input;
RWStructuredBuffer<float> output;
ConstantBuffer<Params> params;
uniform float factor;
uniform uint count;

[shader("compute")]
[numthreads(8, 4, 1)]
void run(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count) {
        output[id.x] = input[id.x].x * factor * params.scale + params.offset.y
            + float(params.count) + params.pair.x;
    }
}
"#;

#[test]
fn global_plain_values_take_set_0_binding_0_and_shift_the_other_bindings() {
    let kernel = kernel(GLOBALS);
    let (globals, block) = uniform_buffer(&kernel, "");
    assert_eq!((globals.set, globals.index), (0, 0));
    assert_eq!(
        offsets_and_size(block, &["factor", "count"]),
        (vec![0, 4], 16)
    );
    let input = binding(&kernel, "input");
    assert_eq!(
        (input.set, input.index, &input.resource),
        (
            0,
            1,
            &Resource::StorageBuffer {
                access: Access::ReadOnly,
                stride: 16,
                element: ElementType::Plain {
                    scalar: Scalar::Float32,
                    components: 4
                }
            }
        )
    );
    let output = binding(&kernel, "output");
    assert_eq!(
        (output.set, output.index, &output.resource),
        (
            0,
            2,
            &Resource::StorageBuffer {
                access: Access::ReadWrite,
                stride: 4,
                element: ElementType::Plain {
                    scalar: Scalar::Float32,
                    components: 1
                }
            }
        )
    );
    let (params, block) = uniform_buffer(&kernel, "params");
    assert_eq!((params.set, params.index), (0, 3));
    assert_eq!(
        offsets_and_size(
            block,
            &[
                "params.scale",
                "params.offset",
                "params.count",
                "params.pair"
            ]
        ),
        (vec![0, 16, 28, 32], 48)
    );
    insta::assert_snapshot!("globals", render(&[kernel]));
}

const PLAIN_VALUES: &str = r#"
cbuffer Arrays { float c[5]; };
cbuffer Matrices { float4x4 m4; float3x3 m3; float2x3 m23; float3 v3; };
RWStructuredBuffer<float> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void run()
{
    output[0] = c[4] + m4[0][0] + m3[0][0] + m23[0][0] + v3.x;
}
"#;

#[test]
fn plain_values_in_constant_buffers_take_the_offsets_of_slangs_default_layout() {
    let kernel = kernel(PLAIN_VALUES);
    let (_, arrays) = uniform_buffer(&kernel, "Arrays");
    assert_eq!(arrays.size, 80);
    assert_eq!(
        value(arrays, "Arrays.c").shape,
        Shape::Array {
            length: 5,
            stride: 16,
            element: Box::new(Shape::Scalar)
        }
    );
    let (_, matrices) = uniform_buffer(&kernel, "Matrices");
    assert_eq!(
        offsets_and_size(
            matrices,
            &["Matrices.m4", "Matrices.m3", "Matrices.m23", "Matrices.v3"]
        ),
        (vec![0, 64, 112, 144], 160)
    );
    insta::assert_snapshot!("plain_values", render(&[kernel]));
}

const NESTED_VALUES: &str = r#"
cbuffer Values { float3 points[4]; float2x2 mats[2]; float grid[2][3]; int count; int2 pair; };
RWStructuredBuffer<float> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void run(
    uniform float3 corners[4],
    uniform float2x2 rotations[2],
    uniform float cells[2][3],
    uniform int total,
    uniform int2 size)
{
    output[0] = points[3].z + mats[1][1][1] + grid[1][2] + float(count) + float(pair.y)
        + corners[3].z + rotations[1][1][1] + cells[1][2] + float(total) + float(size.y);
}
"#;

fn array(length: u64, stride: u64, element: Shape) -> Shape {
    Shape::Array {
        length,
        stride,
        element: Box::new(element),
    }
}

fn layout_of(value: &PlainValue) -> (u64, u64, Scalar, Shape) {
    (value.offset, value.size, value.scalar, value.shape.clone())
}

#[test]
fn arrays_of_vectors_matrices_and_arrays_and_ints_take_std140_and_std430_strides() {
    let kernel = kernel(NESTED_VALUES);
    let matrix = Shape::Matrix {
        rows: 2,
        columns: 2,
    };
    let (_, values) = uniform_buffer(&kernel, "Values");
    assert_eq!(values.size, 240);
    let std140: Vec<_> = values.values.iter().map(layout_of).collect();
    assert_eq!(
        std140,
        [
            (
                0,
                64,
                Scalar::Float32,
                array(4, 16, Shape::Vector { length: 3 })
            ),
            (64, 64, Scalar::Float32, array(2, 32, matrix.clone())),
            (
                128,
                96,
                Scalar::Float32,
                array(2, 48, array(3, 16, Shape::Scalar))
            ),
            (224, 4, Scalar::Int32, Shape::Scalar),
            (232, 8, Scalar::Int32, Shape::Vector { length: 2 }),
        ]
    );
    let immediates = kernel.immediates.as_ref().expect("immediates");
    assert_eq!(immediates.size, 144);
    let std430: Vec<_> = immediates.values.iter().map(layout_of).collect();
    assert_eq!(
        std430,
        [
            (
                0,
                64,
                Scalar::Float32,
                array(4, 16, Shape::Vector { length: 3 })
            ),
            (64, 32, Scalar::Float32, array(2, 16, matrix)),
            (
                96,
                24,
                Scalar::Float32,
                array(2, 12, array(3, 4, Shape::Scalar))
            ),
            (120, 4, Scalar::Int32, Shape::Scalar),
            (128, 8, Scalar::Int32, Shape::Vector { length: 2 }),
        ]
    );
    insta::assert_snapshot!("nested_values", render(&[kernel]));
}

const PARAMETER_BLOCKS: &str = r#"
struct Inner { RWStructuredBuffer<float> deep; };
struct Outer { float weight; ParameterBlock<Inner> inner; };
struct Extra { float bias; RWStructuredBuffer<float> values; };
RWStructuredBuffer<float> data;
ParameterBlock<Extra> extra;
ParameterBlock<Outer> outer;

[shader("compute")]
[numthreads(64, 1, 1)]
void run(uint3 id : SV_DispatchThreadID)
{
    data[id.x] = extra.values[id.x] + extra.bias + outer.weight * outer.inner.deep[id.x];
}
"#;

#[test]
fn a_parameter_block_takes_its_own_set_and_a_nested_one_the_next() {
    let kernel = kernel(PARAMETER_BLOCKS);
    let data = binding(&kernel, "data");
    assert_eq!((data.set, data.index), (0, 0));
    let (extra, block) = uniform_buffer(&kernel, "extra");
    assert_eq!((extra.set, extra.index), (1, 0));
    assert_eq!(offsets_and_size(block, &["extra.bias"]), (vec![0], 16));
    let values = binding(&kernel, "extra.values");
    assert_eq!((values.set, values.index), (1, 1));
    let (outer, _) = uniform_buffer(&kernel, "outer");
    assert_eq!((outer.set, outer.index), (2, 0));
    let deep = binding(&kernel, "outer.inner.deep");
    assert_eq!((deep.set, deep.index), (3, 0));
    assert!(
        kernel
            .bindings
            .iter()
            .all(|binding| binding.usage == Usage::Used)
    );
    insta::assert_snapshot!("parameter_blocks", render(&[kernel]));
}

const TWO_KERNELS: &str = r#"
RWStructuredBuffer<float> first;
RWStructuredBuffer<uint> second;

[shader("compute")]
[numthreads(64, 1, 1)]
void fill(uint3 id : SV_DispatchThreadID, uniform float value)
{
    first[id.x] = value;
}

[shader("compute")]
[numthreads(8, 8, 1)]
void add(uint3 id : SV_DispatchThreadID, uniform uint left, uniform uint right)
{
    second[id.x + id.y * 64] = left + right;
}
"#;

#[test]
fn each_kernel_of_a_module_has_its_own_immediates_and_usage() {
    let kernels = kernels(TWO_KERNELS);
    let [fill, add] = kernels.as_slice() else {
        panic!("two kernels in {kernels:#?}");
    };
    assert_eq!(fill.name, "fill");
    assert_eq!(fill.thread_group_size, [64, 1, 1]);
    assert_eq!(binding(fill, "first").usage, Usage::Used);
    assert_eq!(binding(fill, "second").usage, Usage::Unused);
    assert_eq!(add.name, "add");
    assert_eq!(add.thread_group_size, [8, 8, 1]);
    assert_eq!(binding(add, "first").usage, Usage::Unused);
    assert_eq!(binding(add, "second").usage, Usage::Used);
    let immediates = add.immediates.as_ref().expect("immediates");
    assert_eq!(
        offsets_and_size(immediates, &["left", "right"]),
        (vec![0, 4], 8)
    );
    insta::assert_snapshot!("two_kernels", render(&kernels));
}

#[test]
fn a_kernel_with_another_resource_or_an_array_of_resources_is_unsupported() {
    let cases = [
        (
            "[format(\"rgba32f\")] RWTexture2D<float4> target;",
            "target[uint2(0, 0)].x",
            "target",
        ),
        ("ByteAddressBuffer raw;", "asfloat(raw.Load(0))", "raw"),
        ("uniform double precise;", "float(precise)", "precise"),
        (
            "Texture2D<float4> images[2];",
            "images[1].Load(int3(0, 0, 0)).x",
            "images",
        ),
        (
            "SamplerState samplers[2]; Texture2D<float4> image;",
            "image.SampleLevel(samplers[1], float2(0, 0), 0).x",
            "samplers",
        ),
        (
            "StructuredBuffer<float> inputs[2];",
            "inputs[1][0]",
            "inputs",
        ),
        (
            "struct Pair { float first; float second; }; cbuffer Pairs { Pair pairs[2]; };",
            "pairs[1].second",
            "Pairs.pairs",
        ),
    ];
    for (declaration, read, parameter) in cases {
        let source = format!(
            "{declaration}\nRWStructuredBuffer<float> output;\n\
             [shader(\"compute\")] [numthreads(1, 1, 1)] void run() {{ output[0] = {read}; }}"
        );
        let error = compile("unsupported", &source).expect_err("an unsupported parameter");
        assert_eq!(error.kind(), ErrorKind::UnsupportedParameter, "{error}");
        let text = error.to_string();
        assert!(
            text.contains(&format!("`{parameter}`")) && text.contains("`run`"),
            "{text}"
        );
    }
}

#[test]
fn a_push_constant_block_becomes_the_immediates_and_a_second_block_is_unsupported() {
    let one = kernel(
        "struct Push { float scale; uint count; };\n\
         [[vk::push_constant]] ConstantBuffer<Push> push;\n\
         RWStructuredBuffer<float> output;\n\
         [shader(\"compute\")] [numthreads(1, 1, 1)] \
         void run() { output[0] = push.scale * float(push.count); }",
    );
    let immediates = one
        .immediates
        .as_ref()
        .expect("the block is the immediates");
    assert_eq!(
        offsets_and_size(immediates, &["push.scale", "push.count"]),
        (vec![0, 4], 8)
    );
    assert_eq!(
        one.bindings
            .iter()
            .map(|binding| binding.path.as_str())
            .collect::<Vec<_>>(),
        ["output"]
    );
    let cases = [
        (
            "[[vk::push_constant]] ConstantBuffer<Push> first;\n\
             [[vk::push_constant]] ConstantBuffer<Push> second;",
            "first.scale + second.scale",
            "void run()",
            "second",
        ),
        (
            "[[vk::push_constant]] ConstantBuffer<Push> first;",
            "first.scale + extra",
            "void run(uniform float extra)",
            "",
        ),
    ];
    for (declarations, read, signature, second) in cases {
        let source = format!(
            "struct Push {{ float scale; }};\n{declarations}\n\
             RWStructuredBuffer<float> output;\n\
             [shader(\"compute\")] [numthreads(1, 1, 1)] {signature} {{ output[0] = {read}; }}"
        );
        let error = compile("two_blocks", &source).expect_err("a second block of immediates");
        assert_eq!(error.kind(), ErrorKind::UnsupportedParameter, "{error}");
        assert_eq!(
            error.to_string(),
            format!(
                "cannot load a Slang module: the parameter `{second}` of the kernel `run` is a second block of immediates, which this version does not support"
            )
        );
    }
}

#[test]
fn an_entry_point_of_another_stage_is_not_a_kernel() {
    let kernels = kernels(
        r#"
RWStructuredBuffer<float> output;

[shader("fragment")]
float4 shade() : SV_Target { return float4(1, 0, 0, 1); }

[shader("compute")]
[numthreads(4, 1, 1)]
void run(uint3 id : SV_DispatchThreadID) { output[id.x] = 1.0; }
"#,
    );
    let names: Vec<&str> = kernels.iter().map(|kernel| kernel.name.as_str()).collect();
    assert_eq!(names, ["run"]);
}

#[test]
fn source_that_does_not_compile_is_a_compile_error_with_slangs_diagnostics() {
    let error = compile("broken", "void run() { undefined_name = 1; }")
        .expect_err("the source does not compile");
    assert_eq!(error.kind(), ErrorKind::Compile);
    let source = error
        .source()
        .and_then(|source| source.downcast_ref::<slang_compiler::Error>())
        .expect("the error of slang-compiler as the source");
    assert!(
        source.diagnostics().as_str().contains("undefined_name"),
        "{}",
        source.diagnostics()
    );
}

#[test]
fn each_kernel_gets_spirv_whose_entry_point_keeps_its_slang_name() {
    let compiled = compile(
        "two",
        r#"
RWStructuredBuffer<float> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void first_kernel() { output[0] = 1.0; }

[shader("compute")]
[numthreads(1, 1, 1)]
void second_kernel() { output[1] = 2.0; }
"#,
    )
    .expect("the source compiles");
    assert_eq!(compiled.len(), 2);
    for kernel in &compiled {
        assert_eq!(
            kernel.spirv.first(),
            Some(&0x0723_0203),
            "the SPIR-V magic number"
        );
        let bytes: Vec<u8> = kernel
            .spirv
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        let name = kernel.layout.name.as_bytes();
        assert!(
            bytes.windows(name.len()).any(|window| window == name),
            "the SPIR-V of `{}` names its entry point",
            kernel.layout.name
        );
    }
}

fn entry_points(name: &str, source: &str) -> Vec<CompiledRender> {
    compile_all(name, source)
        .expect("the source compiles")
        .render
}

fn accepted<'a>(records: &'a [CompiledRender], name: &str) -> &'a RenderLayout {
    let record = records
        .iter()
        .find(|record| record.name == name)
        .unwrap_or_else(|| panic!("an entry point `{name}` in {records:#?}"));
    &record
        .usable
        .as_ref()
        .unwrap_or_else(|rejection| panic!("the entry point `{name}` is accepted: {rejection:?}"))
        .layout
}

fn attributes(layout: &RenderLayout) -> Vec<(&str, u32, Scalar, u32, Usage)> {
    layout
        .attributes
        .iter()
        .map(|attribute| {
            (
                attribute.path.as_str(),
                attribute.location,
                attribute.scalar,
                attribute.components,
                attribute.usage,
            )
        })
        .collect()
}

fn placement(layout: &RenderLayout, path: &str) -> (u32, u32, Usage) {
    let binding = layout
        .bindings
        .iter()
        .find(|binding| binding.path == path)
        .unwrap_or_else(|| panic!("a binding `{path}` in {layout:#?}"));
    (binding.set, binding.index, binding.usage)
}

const ATTRIBUTES: &str = r#"
struct VIn { float3 position : POSITION; float3 normal : NORMAL; float2 uv : TEXCOORD0; float4 colour : COLOR; };
struct VOut { float4 position : SV_Position; float4 colour : COLOR; float2 uv : TEXCOORD0; float3 normal : NORMAL; };

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = float4(input.position, 1.0);
    output.colour = input.colour;
    output.uv = input.uv;
    output.normal = input.normal;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return float4(input.colour.r, input.uv.x, input.normal.z, input.colour.a);
}
"#;

#[test]
fn the_fields_of_a_vertex_input_structure_are_attributes_at_their_reflected_locations() {
    let records = entry_points("attributes", ATTRIBUTES);
    let stages: Vec<(&str, RenderStage)> = records
        .iter()
        .map(|record| (record.name.as_str(), record.stage))
        .collect();
    assert_eq!(
        stages,
        [
            ("vsMain", RenderStage::Vertex),
            ("fsMain", RenderStage::Fragment)
        ]
    );
    assert_eq!(
        attributes(accepted(&records, "vsMain")),
        [
            ("input.position", 0, Scalar::Float32, 3, Usage::Used),
            ("input.normal", 1, Scalar::Float32, 3, Usage::Used),
            ("input.uv", 2, Scalar::Float32, 2, Usage::Used),
            ("input.colour", 3, Scalar::Float32, 4, Usage::Used),
        ]
    );
    let fragment = accepted(&records, "fsMain");
    assert!(fragment.attributes.is_empty() && fragment.bindings.is_empty());
    insta::assert_snapshot!("attributes", render_entry_points(&records));
}

#[test]
fn an_attribute_that_the_code_never_reads_is_unused_and_keeps_every_location() {
    let source = ATTRIBUTES.replace("output.uv = input.uv;", "output.uv = float2(0.0, 0.0);");
    let records = entry_points("unused_attribute", &source);
    assert_eq!(
        attributes(accepted(&records, "vsMain")),
        [
            ("input.position", 0, Scalar::Float32, 3, Usage::Used),
            ("input.normal", 1, Scalar::Float32, 3, Usage::Used),
            ("input.uv", 2, Scalar::Float32, 2, Usage::Unused),
            ("input.colour", 3, Scalar::Float32, 4, Usage::Used),
        ]
    );
}

const PATHS: &str = r#"
struct Surface { float3 normal : NORMAL; float2 uv : TEXCOORD0; };
struct VIn { float3 position : POSITION; Surface surface; };

[shader("vertex")]
float4 separate(float3 position : POSITION, float4 colour : COLOR) : SV_Position
{
    return float4(position, colour.a);
}

[shader("vertex")]
float4 nested(VIn input) : SV_Position
{
    return float4(input.position + input.surface.normal, input.surface.uv.x);
}
"#;

#[test]
fn a_separate_parameter_has_its_name_as_its_path_and_a_nested_field_a_dotted_path() {
    let records = entry_points("paths", PATHS);
    assert_eq!(
        attributes(accepted(&records, "separate")),
        [
            ("position", 0, Scalar::Float32, 3, Usage::Used),
            ("colour", 1, Scalar::Float32, 4, Usage::Used),
        ]
    );
    assert_eq!(
        attributes(accepted(&records, "nested")),
        [
            ("input.position", 0, Scalar::Float32, 3, Usage::Used),
            ("input.surface.normal", 1, Scalar::Float32, 3, Usage::Used),
            ("input.surface.uv", 2, Scalar::Float32, 2, Usage::Used),
        ]
    );
    insta::assert_snapshot!("attribute_paths", render_entry_points(&records));
}

const SYSTEM_VALUES: &str = r#"
struct VIn {
    float3 position : POSITION;
    uint vertex : SV_VulkanVertexID;
    uint instance : SV_VulkanInstanceID;
    float2 uv : TEXCOORD0;
};
struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = float4(input.position + float(input.vertex + input.instance), 1.0);
    output.uv = input.uv;
    return output;
}

[shader("vertex")]
float4 vsIds(float3 position : POSITION, uint vertex : SV_VulkanVertexID, uint instance : SV_VulkanInstanceID) : SV_Position
{
    return float4(position, float(vertex + instance));
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return float4(input.position.x / 64.0, input.uv, 1.0);
}
"#;

#[test]
fn system_values_among_the_inputs_are_not_attributes() {
    let records = entry_points("system_values", SYSTEM_VALUES);
    assert_eq!(
        attributes(accepted(&records, "vsMain")),
        [
            ("input.position", 0, Scalar::Float32, 3, Usage::Used),
            ("input.uv", 1, Scalar::Float32, 2, Usage::Used),
        ]
    );
    assert_eq!(
        attributes(accepted(&records, "vsIds")),
        [("position", 0, Scalar::Float32, 3, Usage::Used)]
    );
    assert_eq!(accepted(&records, "fsMain").attributes, Vec::new());
    insta::assert_snapshot!("system_values", render_entry_points(&records));
}

#[test]
fn integer_attributes_record_their_scalar_type_and_component_count() {
    let records = entry_points(
        "integers",
        r#"
struct VIn { float3 position : POSITION; int index : INDEX; uint flags : FLAGS; uint2 pair : PAIR; };

[shader("vertex")]
float4 vsMain(VIn input) : SV_Position
{
    return float4(input.position, float(input.index) + float(input.flags + input.pair.y));
}
"#,
    );
    assert_eq!(
        attributes(accepted(&records, "vsMain")),
        [
            ("input.position", 0, Scalar::Float32, 3, Usage::Used),
            ("input.index", 1, Scalar::Int32, 1, Usage::Used),
            ("input.flags", 2, Scalar::Uint32, 1, Usage::Used),
            ("input.pair", 3, Scalar::Uint32, 2, Usage::Used),
        ]
    );
}

#[test]
fn an_attribute_of_another_type_rejects_its_vertex_entry_point_and_not_the_module() {
    let cases = [
        (
            "float4x4 transform : TRANSFORM",
            "input.transform[0].x",
            "input.transform",
        ),
        ("bool flag : FLAG", "(input.flag ? 1.0 : 0.0)", "input.flag"),
        ("float2 pairs[2] : PAIRS", "input.pairs[1].x", "input.pairs"),
        (
            "double precise : PRECISE",
            "float(input.precise)",
            "input.precise",
        ),
    ];
    for (declaration, read, path) in cases {
        let source = format!(
            "struct VIn {{ float3 position : POSITION; {declaration}; }};\n\
             [shader(\"vertex\")] float4 vsMain(VIn input) : SV_Position \
             {{ return float4(input.position, {read}); }}"
        );
        let records = entry_points("rejected_attribute", &source);
        let [record] = records.as_slice() else {
            panic!("one entry point in {records:#?}");
        };
        let rejection = record.usable.as_ref().expect_err("a rejected attribute");
        assert_eq!(rejection.kind, ErrorKind::UnsupportedParameter);
        let message = &rejection.detail;
        assert!(
            message.contains(&format!("`{path}`"))
                && message.contains("vertex entry point `vsMain`"),
            "{message}"
        );
    }
}

const SHARED: &str = r#"
struct VIn { float2 position : POSITION; };
struct VOut { float4 position : SV_Position; float4 colour : COLOR; };
struct Light { float4 colour; };
struct Material { float4 albedo; };

uniform float4x4 transform;
uniform float4 tint;
ConstantBuffer<Light> light;
ParameterBlock<Material> material;

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = mul(transform, float4(input.position, 0.0, 1.0));
    output.colour = light.colour;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return input.colour * tint * material.albedo;
}
"#;

#[test]
fn each_entry_point_records_the_shared_bindings_with_its_own_usage() {
    let records = entry_points("shared", SHARED);
    let vertex = accepted(&records, "vsMain");
    let fragment = accepted(&records, "fsMain");
    assert_eq!(placement(vertex, ""), (0, 0, Usage::Used));
    assert_eq!(placement(fragment, ""), (0, 0, Usage::Used));
    assert_eq!(placement(vertex, "light"), (0, 1, Usage::Used));
    assert_eq!(placement(fragment, "light"), (0, 1, Usage::Unused));
    assert_eq!(placement(vertex, "material"), (1, 0, Usage::Unused));
    assert_eq!(placement(fragment, "material"), (1, 0, Usage::Used));
    insta::assert_snapshot!("shared", render_entry_points(&records));
}

const TEXTURES: &str = r#"
struct VIn { float2 position : POSITION; float2 uv : TEXCOORD0; };
struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };

Texture2D<float4> image;
SamplerState imageSampler;
Texture2D<float> heights;
RWStructuredBuffer<float4> output;

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut result;
    result.position = float4(input.position, 0.0, 1.0);
    result.uv = input.uv;
    return result;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return image.Sample(imageSampler, input.uv);
}

[shader("compute")]
[numthreads(1, 1, 1)]
void run()
{
    output[0] = image.SampleLevel(imageSampler, float2(0.5, 0.5), 0)
        + heights.SampleLevel(imageSampler, float2(0.5, 0.5), 0);
}
"#;

#[test]
fn a_float_texture_and_a_sampler_are_bindings_of_render_entry_points_and_kernels() {
    let compiled = compile_all("textures", TEXTURES).expect("the source compiles");
    let vertex = accepted(&compiled.render, "vsMain");
    let fragment = accepted(&compiled.render, "fsMain");
    assert_eq!(placement(vertex, "image"), (0, 0, Usage::Unused));
    assert_eq!(placement(fragment, "image"), (0, 0, Usage::Used));
    assert_eq!(placement(fragment, "imageSampler"), (0, 1, Usage::Used));
    assert_eq!(placement(fragment, "heights"), (0, 2, Usage::Unused));
    let [kernel] = compiled.kernels.as_slice() else {
        panic!("one kernel in {:#?}", compiled.kernels);
    };
    let kernel = &kernel.layout;
    let kinds: Vec<(&str, u32, u32, &Resource, Usage)> = kernel
        .bindings
        .iter()
        .map(|binding| {
            (
                binding.path.as_str(),
                binding.set,
                binding.index,
                &binding.resource,
                binding.usage,
            )
        })
        .collect();
    assert_eq!(
        kinds,
        [
            ("image", 0, 0, &Resource::SampledTexture, Usage::Used),
            ("imageSampler", 0, 1, &Resource::Sampler, Usage::Used),
            ("heights", 0, 2, &Resource::SampledTexture, Usage::Used),
            (
                "output",
                0,
                3,
                &Resource::StorageBuffer {
                    access: Access::ReadWrite,
                    stride: 16,
                    element: ElementType::Plain {
                        scalar: Scalar::Float32,
                        components: 4
                    }
                },
                Usage::Used
            ),
        ]
    );
    insta::assert_snapshot!(
        "textures",
        format!(
            "{}{}",
            render(std::slice::from_ref(kernel)),
            render_entry_points(&compiled.render)
        )
    );
}

#[test]
fn every_other_texture_or_sampler_is_unsupported() {
    let cases = [
        (
            "Sampler2D image;",
            "image.SampleLevel(float2(0, 0), 0)",
            "image",
        ),
        (
            "[format(\"rgba32f\")] RWTexture2D<float4> image;",
            "image[uint2(0, 0)]",
            "image",
        ),
        (
            "Texture3D<float4> image; SamplerState linear;",
            "image.SampleLevel(linear, float3(0, 0, 0), 0)",
            "image",
        ),
        (
            "TextureCube<float4> image; SamplerState linear;",
            "image.SampleLevel(linear, float3(0, 0, 1), 0)",
            "image",
        ),
        (
            "Texture2D<int4> image;",
            "float4(image.Load(int3(0, 0, 0)))",
            "image",
        ),
        (
            "Texture2DArray<float4> image; SamplerState linear;",
            "image.SampleLevel(linear, float3(0, 0, 0), 0)",
            "image",
        ),
        (
            "Texture2D<float4> images[2]; SamplerState linear;",
            "images[1].SampleLevel(linear, float2(0, 0), 0)",
            "images",
        ),
        (
            "Texture2D<float4> image; SamplerState samplers[2];",
            "image.SampleLevel(samplers[1], float2(0, 0), 0)",
            "samplers",
        ),
        (
            "Texture2D<float> image; SamplerComparisonState compare;",
            "float4(image.SampleCmpLevelZero(compare, float2(0, 0), 0.5))",
            "compare",
        ),
    ];
    for (declaration, read, parameter) in cases {
        let source = format!(
            "{declaration}\nRWStructuredBuffer<float4> output;\n\
             [shader(\"compute\")] [numthreads(1, 1, 1)] void run() {{ output[0] = {read}; }}"
        );
        let error = compile("unsupported", &source).expect_err("an unsupported parameter");
        assert_eq!(error.kind(), ErrorKind::UnsupportedParameter, "{error}");
        let text = error.to_string();
        assert!(
            text.contains(&format!("`{parameter}`")) && text.contains("kernel `run`"),
            "{declaration}: {text}"
        );
    }
}

#[test]
fn uniform_parameters_of_render_entry_points_are_recorded_as_their_immediates() {
    let records = entry_points(
        "render_immediates",
        r#"
struct VIn { float2 position : POSITION; float4 colour : COLOR; };
struct VOut { float4 position : SV_Position; float4 colour : COLOR; };

[shader("vertex")]
VOut vsMain(VIn input, uniform float4 offset)
{
    VOut output;
    output.position = float4(input.position, 0.0, 1.0) + offset;
    output.colour = input.colour;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input, uniform float4 colour) : SV_Target
{
    return input.colour * colour;
}
"#,
    );
    let vertex = accepted(&records, "vsMain");
    let fragment = accepted(&records, "fsMain");
    let immediates = vertex.immediates.as_ref().expect("immediates");
    assert_eq!(offsets_and_size(immediates, &["offset"]), (vec![0], 16));
    let immediates = fragment.immediates.as_ref().expect("immediates");
    assert_eq!(offsets_and_size(immediates, &["colour"]), (vec![0], 16));
    assert_eq!(
        attributes(vertex),
        [
            ("input.position", 0, Scalar::Float32, 2, Usage::Used),
            ("input.colour", 1, Scalar::Float32, 4, Usage::Used),
        ]
    );
    insta::assert_snapshot!("render_immediates", render_entry_points(&records));
}

#[test]
fn a_module_with_kernels_and_render_entry_points_keeps_the_kernels_unchanged() {
    let render_entry_points_source = r#"
struct VOut { float4 position : SV_Position; float4 colour : COLOR; };

[shader("vertex")]
VOut vsMain(float3 position : POSITION, float4 colour : COLOR)
{
    VOut output;
    output.position = float4(position, 1.0);
    output.colour = colour;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input, uniform Sampler2D image) : SV_Target
{
    return input.colour * image.Sample(input.position.xy);
}
"#;
    let source = format!("{TWO_KERNELS}{render_entry_points_source}");
    let compiled = compile_all("mixed", &source).expect("the source compiles");
    let mixed: Vec<KernelLayout> = compiled
        .kernels
        .into_iter()
        .map(|kernel| kernel.layout)
        .collect();
    assert_eq!(render(&mixed), render(&kernels(TWO_KERNELS)));
    let stages: Vec<(&str, RenderStage)> = compiled
        .render
        .iter()
        .map(|record| (record.name.as_str(), record.stage))
        .collect();
    assert_eq!(
        stages,
        [
            ("vsMain", RenderStage::Vertex),
            ("fsMain", RenderStage::Fragment)
        ]
    );
    accepted(&compiled.render, "vsMain");
    let fragment = compiled
        .render
        .iter()
        .find(|record| record.name == "fsMain")
        .expect("the fragment entry point");
    let rejection = fragment.usable.as_ref().expect_err("Sampler2D is rejected");
    assert_eq!(rejection.kind, ErrorKind::UnsupportedParameter);
    let message = &rejection.detail;
    assert!(
        message.contains("`image`") && message.contains("fragment entry point `fsMain`"),
        "{message}"
    );
    let vertex = compiled
        .render
        .iter()
        .find(|record| record.name == "vsMain")
        .and_then(|record| record.usable.as_ref().ok())
        .expect("the vertex entry point is usable");
    assert_eq!(
        vertex.spirv.first(),
        Some(&0x0723_0203),
        "the SPIR-V magic number"
    );
    let bytes: Vec<u8> = vertex
        .spirv
        .iter()
        .flat_map(|word| word.to_le_bytes())
        .collect();
    assert!(
        bytes
            .windows(b"vsMain".len())
            .any(|window| window == b"vsMain"),
        "the SPIR-V of `vsMain` names its entry point"
    );
}
