use std::error::Error as _;

use slango::{
    AdapterSelection, Blend, Buffer, CullMode, Device, Error, ErrorKind, RenderPipeline,
    RenderPipelineOptions, TextureFormat,
};

use crate::common::{device_with, load, shader};

mod conventions;
mod draw;
mod inputs;
mod rejections;
mod sharing;
mod uniforms;

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

fn options_with(
    colour_format: TextureFormat,
    depth_format: Option<TextureFormat>,
) -> RenderPipelineOptions {
    let mut options = RenderPipelineOptions::default();
    options.colour_format = colour_format;
    options.depth_format = depth_format;
    options
}

#[test]
fn the_quad_of_the_illustration_becomes_a_pipeline_with_every_option() {
    let module = device()
        .load_module(shader("quad.slang"))
        .expect("the module loads");
    for colour_format in [
        TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
    ] {
        for depth_format in [None, Some(TextureFormat::Depth32Float)] {
            for blend in [Blend::Replace, Blend::Alpha] {
                for cull_mode in [CullMode::None, CullMode::Back] {
                    let mut options = options_with(colour_format, depth_format);
                    options.blend = blend;
                    options.cull_mode = cull_mode;
                    let pipeline = module
                        .render_pipeline("vsMain", "fsMain", &options)
                        .unwrap_or_else(|error| panic!("{options:?}: {error}"));
                    let debug = format!("{pipeline:?}");
                    assert!(debug.starts_with("RenderPipeline {"), "{debug}");
                    assert!(debug.contains("\"vsMain\""), "{debug}");
                }
            }
        }
    }
}

#[test]
fn stages_that_share_uniforms_buffers_a_block_a_texture_and_a_sampler_become_a_pipeline() {
    let module = load(
        &device(),
        r#"
struct Frame { float4 tint; float scale; };
struct Lights { float4 ambient; };
struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };

uniform float4x4 transform;
ConstantBuffer<Frame> frame;
ParameterBlock<Lights> lights;
Texture2D<float4> image;
SamplerState imageSampler;
Texture2D<float4> unusedImage;

[shader("vertex")]
VOut vsMain(float2 position : POSITION, float2 uv : TEXCOORD0)
{
    VOut output;
    output.position = mul(transform, float4(position * frame.scale, 0.0, 1.0));
    output.uv = uv;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return image.Sample(imageSampler, input.uv) * frame.tint + lights.ambient;
}
"#,
    );
    module
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the visibility of each binding matches the stages that use it");
}

#[test]
fn a_texture_sampled_by_both_stages_becomes_a_pipeline() {
    let module = load(
        &device(),
        r#"
struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };

Texture2D<float4> heights;
SamplerState heightSampler;

[shader("vertex")]
VOut vsMain(float2 position : POSITION)
{
    VOut output;
    float height = heights.SampleLevel(heightSampler, position, 0).x;
    output.position = float4(position, height, 1.0);
    output.uv = position;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return heights.Sample(heightSampler, input.uv);
}
"#,
    );
    module
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the texture and the sampler are visible to both stages");
}

#[test]
fn attributes_of_every_form_become_vertex_buffers() {
    let module = load(
        &device(),
        r#"
struct VIn
{
    float3 position : POSITION;
    float4 unused : UNUSED;
    float2 uv : TEXCOORD0;
    float4 colour : COLOR;
};
struct VOut { float4 position : SV_Position; float4 colour : COLOR; float2 uv : TEXCOORD0; };

[shader("vertex")]
VOut vsStruct(VIn input)
{
    VOut output;
    output.position = float4(input.position, 1.0);
    output.colour = input.colour;
    output.uv = input.uv;
    return output;
}

[shader("vertex")]
VOut vsSeparate(float2 position : POSITION, float4 colour : COLOR, float2 offset : INSTANCE_OFFSET)
{
    VOut output;
    output.position = float4(position + offset, 0.0, 1.0);
    output.colour = colour;
    output.uv = position;
    return output;
}

[shader("vertex")]
VOut vsIntegers(int index : INDEX, uint2 cell : CELL, uint scalar : SCALAR, int3 shift : SHIFT)
{
    VOut output;
    float2 position = float2(cell) + float2(float(index), float(scalar)) + float2(shift.xy);
    output.position = float4(position, float(shift.z), 1.0);
    output.colour = float4(1.0, 1.0, 1.0, 1.0);
    output.uv = position;
    return output;
}

[shader("vertex")]
VOut vsSystemValues(
    float2 position : POSITION,
    uint vertex : SV_VulkanVertexID,
    float4 colour : COLOR,
    uint instance : SV_VulkanInstanceID)
{
    VOut output;
    output.position = float4(position + float2(float(vertex), float(instance)), 0.0, 1.0);
    output.colour = colour;
    output.uv = position;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return input.colour * float4(input.uv, 1.0, 1.0);
}
"#,
    );
    let defaults = RenderPipelineOptions::default();
    for vertex in ["vsStruct", "vsSeparate", "vsIntegers", "vsSystemValues"] {
        module
            .render_pipeline(vertex, "fsMain", &defaults)
            .unwrap_or_else(|error| panic!("{vertex}: {error}"));
    }
    let mut instanced = RenderPipelineOptions::default();
    instanced.instance_attributes = vec!["offset".to_owned(), "colour".to_owned()];
    module
        .render_pipeline("vsSeparate", "fsMain", &instanced)
        .expect("attributes that step per instance");
    let mut unused_instanced = RenderPipelineOptions::default();
    unused_instanced.instance_attributes = vec!["input.unused".to_owned()];
    module
        .render_pipeline("vsStruct", "fsMain", &unused_instanced)
        .expect("an unused attribute named as an instance attribute");
}

const CHECKED: &str = r#"
struct VOut { float4 position : SV_Position; float4 colour : COLOR; };

RWStructuredBuffer<float> data;

[shader("vertex")]
VOut vsMain(float2 position : POSITION, float4 colour : COLOR)
{
    VOut output;
    output.position = float4(position, 0.0, 1.0);
    output.colour = colour;
    return output;
}

[shader("vertex")]
VOut vsUniform(float2 position : POSITION, uniform float scale)
{
    VOut output;
    output.position = float4(position * scale, 0.0, 1.0);
    output.colour = float4(1.0, 1.0, 1.0, 1.0);
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target { return input.colour; }

[shader("fragment")]
float4 fsUniform(VOut input, uniform float4 tint) : SV_Target { return input.colour * tint; }

[shader("fragment")]
float4 fsCombined(VOut input, uniform Sampler2D image) : SV_Target
{
    return input.colour * image.Sample(input.position.xy);
}

[shader("compute")]
[numthreads(64, 1, 1)]
void double_each(uint3 id : SV_DispatchThreadID) { data[id.x] = data[id.x] * 2.0; }
"#;

fn checked(
    vertex: &str,
    fragment: &str,
    options: &RenderPipelineOptions,
) -> slango::Result<RenderPipeline> {
    load(&device(), CHECKED).render_pipeline(vertex, fragment, options)
}

fn assert_invalid(vertex: &str, fragment: &str, options: &RenderPipelineOptions, problem: &str) {
    let error = checked(vertex, fragment, options).err();
    assert_eq!(
        error.as_ref().map(Error::kind),
        Some(ErrorKind::InvalidArgument)
    );
    assert_eq!(
        error.map(|error| error.to_string()),
        Some(format!(
            "cannot create a render pipeline: for `{vertex}` and `{fragment}`, {problem}"
        ))
    );
}

#[test]
fn names_that_are_no_entry_point_of_their_stage_are_invalid_arguments() {
    let defaults = RenderPipelineOptions::default();
    assert_invalid(
        "missing",
        "fsMain",
        &defaults,
        "the module has no vertex entry point named `missing`",
    );
    assert_invalid(
        "vsMain",
        "missing",
        &defaults,
        "the module has no fragment entry point named `missing`",
    );
    assert_invalid(
        "double_each",
        "fsMain",
        &defaults,
        "the module has no vertex entry point named `double_each`",
    );
    assert_invalid(
        "vsMain",
        "double_each",
        &defaults,
        "the module has no fragment entry point named `double_each`",
    );
    assert_invalid(
        "fsMain",
        "vsMain",
        &defaults,
        "the module has no vertex entry point named `fsMain`",
    );
    assert_invalid(
        "vsMain",
        "vsMain",
        &defaults,
        "the module has no fragment entry point named `vsMain`",
    );
}

#[test]
fn a_rejected_entry_point_returns_its_rejection() {
    let error = checked("vsMain", "fsCombined", &RenderPipelineOptions::default())
        .expect_err("the creation fails");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter, "{error}");
    let text = error.to_string();
    assert!(
        text.starts_with("cannot create a render pipeline: for `vsMain` and `fsCombined`, "),
        "{text}"
    );
    assert!(text.contains("`image`"), "{text}");
    assert!(text.contains("`fsCombined`"), "{text}");
}

#[test]
fn uniform_parameters_of_either_entry_point_are_unsupported() {
    let defaults = RenderPipelineOptions::default();
    for (vertex, fragment, stage, name) in [
        ("vsUniform", "fsMain", "vertex", "vsUniform"),
        ("vsMain", "fsUniform", "fragment", "fsUniform"),
    ] {
        let error = checked(vertex, fragment, &defaults).expect_err("the creation fails");
        assert_eq!(error.kind(), ErrorKind::UnsupportedParameter, "{error}");
        assert_eq!(
            error.to_string(),
            format!(
                "cannot create a render pipeline: for `{vertex}` and `{fragment}`, the {stage} entry point `{name}` has `uniform` parameters of its own, which a render pipeline does not support, so declare them as global `uniform` values or in a constant buffer or parameter block"
            )
        );
    }
}

#[test]
fn wrong_formats_and_unknown_instance_attributes_are_invalid_arguments() {
    assert_invalid(
        "vsMain",
        "fsMain",
        &options_with(TextureFormat::Depth32Float, None),
        "`colour_format` is the depth format Depth32Float and not a colour format",
    );
    assert_invalid(
        "vsMain",
        "fsMain",
        &options_with(TextureFormat::Rgba8Unorm, Some(TextureFormat::Rgba8Unorm)),
        "`depth_format` is the colour format Rgba8Unorm and not the depth format Depth32Float",
    );
    let mut options = RenderPipelineOptions::default();
    options.instance_attributes = vec!["input.colour".to_owned()];
    assert_invalid(
        "vsMain",
        "fsMain",
        &options,
        "`instance_attributes` names `input.colour`, which is no vertex attribute of the vertex entry point `vsMain`",
    );
}

#[test]
fn varyings_of_another_type_at_one_location_are_an_unsupported_shader() {
    let module = load(
        &device(),
        r#"
struct VOut { float4 position : SV_Position; float4 colour : COLOR; float2 uv : TEXCOORD0; };
struct FIn { float4 position : SV_Position; float2 uv : TEXCOORD0; float4 colour : COLOR; };

[shader("vertex")]
VOut vsMain(float2 position : POSITION)
{
    VOut output;
    output.position = float4(position, 0.0, 1.0);
    output.colour = float4(1.0, 0.0, 0.0, 1.0);
    output.uv = position;
    return output;
}

[shader("fragment")]
float4 fsMain(FIn input) : SV_Target { return input.colour * float4(input.uv, 0.0, 1.0); }
"#,
    );
    let error = module
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect_err("the varyings do not match");
    assert_eq!(error.kind(), ErrorKind::UnsupportedShader, "{error}");
    assert_eq!(
        error.to_string(),
        "cannot create a render pipeline: for `vsMain` and `fsMain`, the backend rejected the render pipeline"
    );
    let text = error
        .source()
        .map(ToString::to_string)
        .expect("the error of the backend as the source");
    println!("the backend's text: {text}");
    assert_ne!(text.trim(), "");
}

#[test]
fn a_vertex_entry_point_that_naga_rejects_is_an_unsupported_shader() {
    let module = load(
        &device(),
        r#"
struct VOut { float4 position : SV_Position; };

[shader("vertex")]
VOut vsMain(float2 position : POSITION, uint vertex : SV_VertexID)
{
    VOut output;
    output.position = float4(position.x + float(vertex), position.y, 0.0, 1.0);
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target { return float4(1.0, 1.0, 1.0, 1.0); }
"#,
    );
    let error = module
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect_err("naga rejects SV_VertexID");
    assert_eq!(error.kind(), ErrorKind::UnsupportedShader, "{error}");
    assert_eq!(
        error.to_string(),
        "cannot create a render pipeline: for `vsMain` and `fsMain`, the backend rejected the SPIR-V of the vertex entry point `vsMain`"
    );
    let text = error
        .source()
        .map(ToString::to_string)
        .expect("the error of the backend as the source");
    println!("the backend's text: {text}");
    assert_ne!(text.trim(), "");
}

#[test]
fn a_kernel_runs_and_a_render_pipeline_is_created_from_one_module() -> slango::Result<()> {
    let device = device();
    let module = load(&device, CHECKED);
    let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
    let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0])?;
    module
        .kernel("double_each")?
        .call()
        .bind("data", &data)
        .dispatch([3, 1, 1])?;
    assert_eq!(data.read()?, [2.0, 4.0, 6.0]);
    drop(pipeline);
    Ok(())
}

#[test]
fn one_module_creates_several_pipelines_also_from_two_threads() -> slango::Result<()> {
    let module = load(&device(), CHECKED);
    let first = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
    let mut options = options_with(
        TextureFormat::Bgra8UnormSrgb,
        Some(TextureFormat::Depth32Float),
    );
    options.blend = Blend::Alpha;
    let second = module.render_pipeline("vsMain", "fsMain", &options)?;
    assert_ne!(format!("{first:?}"), format!("{second:?}"));
    std::thread::scope(|scope| {
        let handles: Vec<_> = [TextureFormat::Bgra8Unorm, TextureFormat::Rgba8UnormSrgb]
            .into_iter()
            .map(|colour_format| {
                let module = module.clone();
                scope.spawn(move || {
                    let mut options = options_with(colour_format, None);
                    options.cull_mode = CullMode::Back;
                    let pipeline = module.render_pipeline("vsMain", "fsMain", &options)?;
                    let clone = pipeline.clone();
                    drop(pipeline);
                    Ok::<_, Error>(clone)
                })
            })
            .collect();
        for handle in handles {
            handle
                .join()
                .expect("the thread finishes")
                .expect("a pipeline from another thread");
        }
    });
    Ok(())
}
