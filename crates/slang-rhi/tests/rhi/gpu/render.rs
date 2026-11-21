//! The tests of textures, samplers, render pipelines, render passes, and the
//! reading of textures on an adapter of the graphics API of the platform,
//! which check the drawn texels exactly where the rules of rasterization
//! make them exact.

use std::panic::{AssertUnwindSafe, catch_unwind};

use slang_compiler::reflection::LayoutSize;
use slang_compiler::{Component, LinkedProgram, ParameterCategory, Session, Stage, Toggle};
use slang_rhi::{
    AddressMode, Blend, Buffer, BufferDesc, BufferUsage, ColourLoad, CullMode, DepthLoad, Device,
    ErrorKind, Format, Operation, RenderPass, RenderPipeline, RenderPipelineDesc, SamplerDesc,
    StepMode, Texture, TextureDesc, TextureUsage, VertexFormat, VertexStream,
};

use crate::common::{Messages, bytes, open, options};

/// A shader whose vertex entry point takes a position of three components
/// and a colour, and whose fragment entry point returns that colour.
const FLAT: &str = r#"
struct VertexOutput
{
    float4 position : SV_Position;
    float4 colour : COLOR;
};

[shader("vertex")]
VertexOutput vertex_main(float3 position : POSITION, float4 colour : COLOR)
{
    VertexOutput output;
    output.position = float4(position, 1.0);
    output.colour = colour;
    return output;
}

[shader("fragment")]
float4 fragment_main(VertexOutput input) : SV_Target
{
    return input.colour;
}
"#;

/// A shader whose vertex entry point declares an input between its two
/// inputs that it does not read.
const GAP: &str = r#"
struct VertexOutput
{
    float4 position : SV_Position;
    float4 colour : COLOR;
};

[shader("vertex")]
VertexOutput vertex_main(float2 position : POSITION, float unused : UNUSED, float4 colour : COLOR)
{
    VertexOutput output;
    output.position = float4(position, 0.0, 1.0);
    output.colour = colour;
    return output;
}

[shader("fragment")]
float4 fragment_main(VertexOutput input) : SV_Target
{
    return input.colour;
}
"#;

/// A shader that draws a triangle once for each instance, moved by an
/// offset that steps for each instance, in green.
const INSTANCED: &str = r#"
[shader("vertex")]
float4 vertex_main(float2 position : POSITION, float2 offset : OFFSET) : SV_Position
{
    return float4(position + offset, 0.5, 1.0);
}

[shader("fragment")]
float4 fragment_main() : SV_Target
{
    return float4(0.0, 1.0, 0.0, 1.0);
}
"#;

/// A shader that samples a texture at the position of each fragment in a
/// target of 4 by 4 texels.
const TEXTURED: &str = r#"
Texture2D colours;
SamplerState nearest;

[shader("vertex")]
float4 vertex_main(float2 position : POSITION) : SV_Position
{
    return float4(position, 0.0, 1.0);
}

[shader("fragment")]
float4 fragment_main(float4 position : SV_Position) : SV_Target
{
    return colours.Sample(nearest, position.xy / 4.0);
}
"#;

/// A shader whose vertex entry point takes an unsigned integer between two
/// positions and draws at the second position when it is 1.
const KINDS: &str = r#"
[shader("vertex")]
float4 vertex_main(float2 position : POSITION, uint kind : KIND, float2 other : OTHER) : SV_Position
{
    return float4(kind == 1 ? other : position, 0.0, 1.0);
}

[shader("fragment")]
float4 fragment_main() : SV_Target
{
    return float4(1.0, 0.0, 0.0, 1.0);
}
"#;

/// A shader whose vertex entry point takes a position of four components and
/// a colour of two, and draws in the colour with blue 0 and alpha 1.
const COMPONENTS: &str = r#"
struct VertexOutput
{
    float4 position : SV_Position;
    float4 colour : COLOR;
};

[shader("vertex")]
VertexOutput vertex_main(float4 position : POSITION, float2 colour : COLOR)
{
    VertexOutput output;
    output.position = position;
    output.colour = float4(colour, 0.0, 1.0);
    return output;
}

[shader("fragment")]
float4 fragment_main(VertexOutput input) : SV_Target
{
    return input.colour;
}
"#;

/// A shader whose fragment entry point reads a texture of each shape that the
/// crate does not create, a texture of unsigned integers, and a comparison
/// sampler.
const UNSUITED: &str = r#"
Texture2D<uint4> integers;
Texture3D volume;
TextureCube cube;
Texture2DArray layers;
Texture2DMS<float4> multisampled;
Texture2D shadows;
SamplerComparisonState comparison;
SamplerState nearest;

[shader("vertex")]
float4 vertex_main(float2 position : POSITION) : SV_Position
{
    return float4(position, 0.0, 1.0);
}

[shader("fragment")]
float4 fragment_main(float4 position : SV_Position) : SV_Target
{
    float3 at = float3(position.xy / 4.0, 0.0);
    float value = float(integers.Load(int3(0, 0, 0)).x)
        + volume.Sample(nearest, at).x
        + cube.Sample(nearest, at).x
        + layers.Sample(nearest, at).x
        + multisampled.Load(int2(0, 0), 0).x
        + shadows.SampleCmp(comparison, at.xy, 0.5);
    return float4(value, 0.0, 0.0, 1.0);
}
"#;

/// The rows of a target of 4 by 4 texels after a draw of a red triangle
/// that covers it at a depth outside the range from 0 to 1, which Vulkan
/// draws, because slang-rhi clamps the depth there, and Direct3D 12 clips.
#[cfg(windows)]
const OUTSIDE_DEPTH_ROWS: [&str; 4] = ["...."; 4];

/// The rows of a target of 4 by 4 texels after a draw of a red triangle
/// that covers it at a depth outside the range from 0 to 1, which Vulkan
/// draws, because slang-rhi clamps the depth there, and Direct3D 12 clips.
#[cfg(not(windows))]
const OUTSIDE_DEPTH_ROWS: [&str; 4] = ["RRRR"; 4];

const SIZE: [u32; 2] = [4, 4];
const BLACK: ColourLoad = ColourLoad::Clear([0.0, 0.0, 0.0, 1.0]);
const RED: [f32; 4] = [1.0, 0.0, 0.0, 1.0];
const GREEN: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const BLUE: [f32; 4] = [0.0, 0.0, 1.0, 1.0];

/// The corners of a triangle that covers the whole target.
const FULL: [[f32; 2]; 3] = [[-1.0, -1.0], [3.0, -1.0], [-1.0, 3.0]];

/// The corners of a triangle in the upper left of a target of 4 by 4 texels,
/// in counter-clockwise order, whose edges pass through no texel centre.
const UPPER_LEFT: [[f32; 2]; 3] = [[-1.0, 1.0], [-1.0, -0.9], [0.9, 1.0]];

/// The texels of a target of 4 by 4 texels that [`UPPER_LEFT`] covers, row
/// after row from the top, with clip space y up.
const UPPER_LEFT_ROWS: [&str; 4] = ["RRR.", "RR..", "R...", "...."];

/// Loads `source` into the session of `device` and links its entry points
/// `vertex_main` and `fragment_main`.
fn link_render(session: &Session, name: &str, source: &str) -> LinkedProgram {
    let module = session
        .load_module_from_source(name, format!("{name}.slang"), source)
        .expect("the source compiles")
        .into_value();
    let vertex = module
        .entry_point_with_stage("vertex_main", Stage::Vertex)
        .expect("the vertex entry point exists")
        .into_value();
    let fragment = module
        .entry_point_with_stage("fragment_main", Stage::Fragment)
        .expect("the fragment entry point exists")
        .into_value();
    session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&vertex),
            Component::EntryPoint(&fragment),
        ])
        .expect("the parts compose")
        .into_value()
        .link()
        .expect("the program links")
        .into_value()
}

/// One varying input of a vertex entry point: its name, location, semantic
/// name, and semantic index.
type Input = (String, u32, String, u32);

/// Reads the varying inputs of the vertex entry point of `linked`.
fn vertex_inputs(session: &Session, linked: &LinkedProgram) -> Vec<Input> {
    let target = session.target_index(0).expect("the session has a target");
    let layout = linked
        .layout(target)
        .expect("the program has a layout")
        .into_value();
    let mut inputs = Vec::new();
    for entry_point in layout.entry_point_layouts() {
        if entry_point.stage().expect("the stage reads") != Some(Stage::Vertex) {
            continue;
        }
        for parameter in entry_point.parameter_layouts() {
            let name = parameter
                .variable()
                .and_then(|variable| variable.name().expect("the name reads"))
                .expect("the parameter has a name")
                .to_owned();
            let LayoutSize::Finite(location) =
                parameter.offset(ParameterCategory::VaryingInput).amount
            else {
                panic!("the input {name} has no location");
            };
            let semantic = parameter
                .semantic_name()
                .expect("the semantic reads")
                .expect("the input has a semantic")
                .to_owned();
            inputs.push((
                name,
                u32::try_from(location).expect("the location fits"),
                semantic,
                u32::try_from(parameter.semantic_index()).expect("the index fits"),
            ));
        }
    }
    inputs
}

/// Returns the vertex stream of the input `name` of `inputs` with the format
/// `format`, at the location and with the semantic of reflection.
fn stream(inputs: &[Input], name: &str, format: VertexFormat) -> VertexStream {
    let (_, location, semantic, index) = inputs
        .iter()
        .find(|input| input.0 == name)
        .unwrap_or_else(|| panic!("the vertex entry point has no input {name}"));
    VertexStream::new(format, *location).with_semantic(semantic.as_str(), *index)
}

/// Creates the render pipeline of `source` into a colour target of
/// `colour_format`, whose vertex streams are those of `inputs`, the names,
/// formats, and steps of the inputs, and whose other state `state` sets.
fn render_pipeline(
    device: &Device,
    (name, source): (&str, &str),
    colour_format: Format,
    inputs: &[(&str, VertexFormat, StepMode)],
    state: impl FnOnce(RenderPipelineDesc) -> RenderPipelineDesc,
) -> RenderPipeline {
    let linked = link_render(device.slang_session(), name, source);
    let reflected = vertex_inputs(device.slang_session(), &linked);
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    let streams = inputs
        .iter()
        .map(|(name, format, step)| stream(&reflected, name, *format).with_step(*step));
    let desc = state(RenderPipelineDesc::new(colour_format).with_vertex_streams(streams));
    device
        .create_render_pipeline(&program, &desc)
        .expect("slang-rhi creates the render pipeline")
}

/// The inputs of the vertex entry point of [`FLAT`].
const FLAT_INPUTS: [(&str, VertexFormat, StepMode); 2] = [
    ("position", VertexFormat::Float32x3, StepMode::Vertex),
    ("colour", VertexFormat::Float32x4, StepMode::Vertex),
];

/// The inputs of the vertex entry point of [`TEXTURED`].
const TEXTURED_INPUTS: [(&str, VertexFormat, StepMode); 1] =
    [("position", VertexFormat::Float32x2, StepMode::Vertex)];

/// Creates the pipeline of [`FLAT`] into a colour target of `Rgba8Unorm`
/// with its streams of positions and colours and the state that `state`
/// sets.
fn flat_pipeline(
    device: &Device,
    state: impl FnOnce(RenderPipelineDesc) -> RenderPipelineDesc,
) -> RenderPipeline {
    render_pipeline(
        device,
        ("flat", FLAT),
        Format::Rgba8Unorm,
        &FLAT_INPUTS,
        state,
    )
}

fn colour_target(device: &Device, usage: TextureUsage) -> Texture {
    device
        .create_texture(
            &TextureDesc::new(
                SIZE,
                Format::Rgba8Unorm,
                TextureUsage::RENDER_TARGET | TextureUsage::COPY_SOURCE | usage,
            ),
            None,
        )
        .expect("slang-rhi creates the colour target")
}

fn depth_target(device: &Device) -> Texture {
    device
        .create_texture(
            &TextureDesc::new(SIZE, Format::D32Float, TextureUsage::DEPTH_STENCIL),
            None,
        )
        .expect("slang-rhi creates the depth target")
}

fn vertex_buffer(device: &Device, values: &[f32]) -> Buffer {
    let data = bytes(values);
    let size = u64::try_from(data.len()).expect("the size fits");
    device
        .create_buffer(
            &BufferDesc::new(size, BufferUsage::VERTEX_BUFFER),
            Some(&data),
        )
        .expect("slang-rhi creates the vertex buffer")
}

fn index_buffer(device: &Device, indices: &[u32]) -> Buffer {
    let data: Vec<u8> = indices
        .iter()
        .flat_map(|index| index.to_le_bytes())
        .collect();
    let size = u64::try_from(data.len()).expect("the size fits");
    device
        .create_buffer(
            &BufferDesc::new(size, BufferUsage::INDEX_BUFFER),
            Some(&data),
        )
        .expect("slang-rhi creates the index buffer")
}

/// Returns the buffers of the positions and colours of a triangle of the
/// corners `corners` at the depth `depth`, all of the colour `colour`.
fn triangle(device: &Device, corners: &[[f32; 2]], depth: f32, colour: [f32; 4]) -> [Buffer; 2] {
    let positions: Vec<f32> = corners.iter().flat_map(|[x, y]| [*x, *y, depth]).collect();
    let colours: Vec<f32> = corners.iter().flat_map(|_| colour).collect();
    [
        vertex_buffer(device, &positions),
        vertex_buffer(device, &colours),
    ]
}

/// Records one render pass with the targets of `desc`, in which `f` draws,
/// submits it, and waits for it.
fn render(
    device: &Device,
    desc: &slang_rhi::RenderPassDesc<'_>,
    f: impl FnOnce(&mut RenderPass<'_>) -> slang_rhi::Result<()>,
) {
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    encoder.render_pass(desc, f).expect("the pass records");
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
}

/// Draws `vertex_count` vertices of the buffers `buffers` with `pipeline`
/// into `target`, which the pass clears to black first.
fn draw_into(
    device: &Device,
    target: &Texture,
    pipeline: &RenderPipeline,
    buffers: &[&Buffer],
    vertex_count: u32,
) {
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(target, BLACK);
    render(device, &desc, |pass| {
        drop(pass.bind_pipeline(pipeline)?);
        pass.set_vertex_buffers(buffers)?;
        pass.draw(vertex_count, 1)
    });
}

/// Reads the texels of a target of 4 by 4 texels of the format
/// `Rgba8Unorm`.
fn texels(device: &Device, target: &Texture) -> Vec<[u8; 4]> {
    let mut out = vec![0_u8; 64];
    device
        .read_texture(target, &mut out)
        .expect("slang-rhi reads the texture");
    out.as_chunks::<4>().0.to_vec()
}

/// Returns the rows of a target of 4 by 4 texels from the top, with one
/// letter for each texel: `R`, `G`, `B`, or `W` for red, green, blue, or
/// white, `.` for black, and `?` for any other value.
fn rows(device: &Device, target: &Texture) -> Vec<String> {
    texels(device, target)
        .chunks(4)
        .map(|row| {
            row.iter()
                .map(|texel| match texel {
                    [255, 0, 0, 255] => 'R',
                    [0, 255, 0, 255] => 'G',
                    [0, 0, 255, 255] => 'B',
                    [255, 255, 255, 255] => 'W',
                    [0, 0, 0, 255] => '.',
                    _ => '?',
                })
                .collect()
        })
        .collect()
}

fn assert_no_error(messages: &Messages) {
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_triangle_lands_with_clip_space_y_up_and_the_top_row_first() {
    for validation in [Toggle::Off, Toggle::On] {
        let (options, messages) = options(validation);
        let device = open(&options);
        let pipeline = flat_pipeline(&device, |desc| desc);
        let target = colour_target(&device, TextureUsage::empty());
        let [positions, colours] = triangle(&device, &UPPER_LEFT, 0.0, RED);
        draw_into(&device, &target, &pipeline, &[&positions, &colours], 3);
        assert_eq!(rows(&device, &target), UPPER_LEFT_ROWS, "{validation:?}");
        assert_no_error(&messages);
    }
}

#[test]
fn counter_clockwise_triangles_face_the_front_and_back_faces_are_culled() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = flat_pipeline(&device, |desc| desc.with_cull_mode(CullMode::Back));
    let target = colour_target(&device, TextureUsage::empty());
    let [positions, colours] = triangle(&device, &UPPER_LEFT, 0.0, RED);
    draw_into(&device, &target, &pipeline, &[&positions, &colours], 3);
    assert_eq!(rows(&device, &target), UPPER_LEFT_ROWS);
    let [a, b, c] = UPPER_LEFT;
    let [positions, colours] = triangle(&device, &[a, c, b], 0.0, RED);
    draw_into(&device, &target, &pipeline, &[&positions, &colours], 3);
    assert_eq!(rows(&device, &target), ["...."; 4]);
    let unculled = flat_pipeline(&device, |desc| desc.with_cull_mode(CullMode::None));
    draw_into(&device, &target, &unculled, &[&positions, &colours], 3);
    assert_eq!(rows(&device, &target), UPPER_LEFT_ROWS);
    assert_no_error(&messages);
}

#[test]
fn an_indexed_draw_takes_the_vertices_that_its_indices_name() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = flat_pipeline(&device, |desc| desc);
    let target = colour_target(&device, TextureUsage::empty());
    let corners = [[5.0, 5.0], UPPER_LEFT[0], UPPER_LEFT[1], UPPER_LEFT[2]];
    let [positions, colours] = triangle(&device, &corners, 0.0, RED);
    let indices = index_buffer(&device, &[1, 2, 3, 0]);
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(&target, BLACK);
    render(&device, &desc, |pass| {
        drop(pass.bind_pipeline(&pipeline)?);
        pass.set_vertex_buffers(&[&positions, &colours])?;
        pass.set_index_buffer(&indices)?;
        pass.draw_indexed(3, 1)
    });
    assert_eq!(rows(&device, &target), UPPER_LEFT_ROWS);
    assert_no_error(&messages);
}

#[test]
fn an_index_past_the_vertex_buffer_reaches_the_device_and_it_keeps_working() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = flat_pipeline(&device, |desc| desc);
    let target = colour_target(&device, TextureUsage::empty());
    let [positions, colours] = triangle(&device, &UPPER_LEFT, 0.0, RED);
    let indices = index_buffer(&device, &[0, 1, 100_000]);
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(&target, BLACK);
    render(&device, &desc, |pass| {
        drop(pass.bind_pipeline(&pipeline)?);
        pass.set_vertex_buffers(&[&positions, &colours])?;
        pass.set_index_buffer(&indices)?;
        pass.draw_indexed(3, 1)
    });
    let _ignored = texels(&device, &target);
    draw_into(&device, &target, &pipeline, &[&positions, &colours], 3);
    assert_eq!(rows(&device, &target), UPPER_LEFT_ROWS);
    assert_no_error(&messages);
}

#[test]
fn a_sampled_texture_with_point_filters_gives_each_texel_its_quarter_of_the_target() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = render_pipeline(
        &device,
        ("textured", TEXTURED),
        Format::Rgba8Unorm,
        &TEXTURED_INPUTS,
        |desc| desc,
    );
    let texels_data: Vec<u8> = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ]
    .concat();
    let source = device
        .create_texture(
            &TextureDesc::new([2, 2], Format::Rgba8Unorm, TextureUsage::SHADER_RESOURCE),
            Some(&texels_data),
        )
        .expect("slang-rhi creates the texture");
    let sampler = device
        .create_sampler(&SamplerDesc::new().with_address_modes([AddressMode::ClampToEdge; 3]))
        .expect("slang-rhi creates the sampler");
    let target = colour_target(&device, TextureUsage::empty());
    let corners = vertex_buffer(&device, FULL.as_flattened());
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(&target, BLACK);
    render(&device, &desc, |pass| {
        let mut root = pass.bind_pipeline(&pipeline)?;
        let colours = root
            .offset_of("colours")?
            .expect("the shader declares colours");
        root.set_texture(colours, &source)?;
        let nearest = root
            .offset_of("nearest")?
            .expect("the shader declares nearest");
        root.set_sampler(nearest, &sampler)?;
        drop(root);
        pass.set_vertex_buffers(&[&corners])?;
        pass.draw(3, 1)
    });
    assert_eq!(rows(&device, &target), ["RRGG", "RRGG", "BBWW", "BBWW"]);
    assert_no_error(&messages);
}

#[test]
fn clones_of_textures_samplers_and_render_pipelines_draw_after_the_originals_are_dropped() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = render_pipeline(
        &device,
        ("textured", TEXTURED),
        Format::Rgba8Unorm,
        &TEXTURED_INPUTS,
        |desc| desc,
    );
    let texels_data: Vec<u8> = [
        [255, 0, 0, 255],
        [0, 255, 0, 255],
        [0, 0, 255, 255],
        [255, 255, 255, 255],
    ]
    .concat();
    let source = device
        .create_texture(
            &TextureDesc::new([2, 2], Format::Rgba8Unorm, TextureUsage::SHADER_RESOURCE),
            Some(&texels_data),
        )
        .expect("slang-rhi creates the texture");
    let sampler = device
        .create_sampler(&SamplerDesc::new().with_address_modes([AddressMode::ClampToEdge; 3]))
        .expect("slang-rhi creates the sampler");
    let target = colour_target(&device, TextureUsage::empty());
    let corners = vertex_buffer(&device, FULL.as_flattened());
    let (pipeline_clone, source_clone, sampler_clone, target_clone, corners_clone) = (
        pipeline.clone(),
        source.clone(),
        sampler.clone(),
        target.clone(),
        corners.clone(),
    );
    assert_eq!(source_clone.size(), source.size());
    assert_eq!(source_clone.format(), source.format());
    assert_eq!(source_clone.usage(), source.usage());
    drop((pipeline, source, sampler, target, corners));
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(&target_clone, BLACK);
    render(&device, &desc, |pass| {
        let mut root = pass.bind_pipeline(&pipeline_clone)?;
        let colours = root
            .offset_of("colours")?
            .expect("the shader declares colours");
        root.set_texture(colours, &source_clone)?;
        let nearest = root
            .offset_of("nearest")?
            .expect("the shader declares nearest");
        root.set_sampler(nearest, &sampler_clone)?;
        drop(root);
        pass.set_vertex_buffers(&[&corners_clone])?;
        pass.draw(3, 1)
    });
    assert_eq!(
        rows(&device, &target_clone),
        ["RRGG", "RRGG", "BBWW", "BBWW"]
    );
    assert_no_error(&messages);
}

#[test]
fn a_depth_target_keeps_the_nearest_fragment_with_the_test_less() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = flat_pipeline(&device, |desc| desc.with_depth_format(Format::D32Float));
    let target = colour_target(&device, TextureUsage::empty());
    let depth = depth_target(&device);
    let near = triangle(&device, &FULL, 0.5, RED);
    let far = triangle(&device, &FULL, 0.75, GREEN);
    let desc = slang_rhi::RenderPassDesc::new()
        .with_colour_target(&target, BLACK)
        .with_depth_target(&depth, DepthLoad::Clear(1.0));
    render(&device, &desc, |pass| {
        drop(pass.bind_pipeline(&pipeline)?);
        pass.set_vertex_buffers(&[&near[0], &near[1]])?;
        pass.draw(3, 1)?;
        pass.set_vertex_buffers(&[&far[0], &far[1]])?;
        pass.draw(3, 1)
    });
    assert_eq!(rows(&device, &target), ["RRRR"; 4]);
    let nearer = triangle(&device, &FULL, 0.25, BLUE);
    let desc = slang_rhi::RenderPassDesc::new()
        .with_colour_target(&target, ColourLoad::Load)
        .with_depth_target(&depth, DepthLoad::Load);
    render(&device, &desc, |pass| {
        drop(pass.bind_pipeline(&pipeline)?);
        pass.set_vertex_buffers(&[&far[0], &far[1]])?;
        pass.draw(3, 1)?;
        pass.set_vertex_buffers(&[&nearer[0], &nearer[1]])?;
        pass.draw(3, 1)
    });
    assert_eq!(rows(&device, &target), ["BBBB"; 4]);
    assert_no_error(&messages);
}

#[test]
fn a_pass_with_only_a_depth_target_clears_it_and_cannot_draw() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = flat_pipeline(&device, |desc| desc.with_depth_format(Format::D32Float));
    let target = colour_target(&device, TextureUsage::empty());
    let depth = depth_target(&device);
    let red = triangle(&device, &FULL, 0.5, RED);
    let clear_depth = |value: f32| {
        let desc =
            slang_rhi::RenderPassDesc::new().with_depth_target(&depth, DepthLoad::Clear(value));
        let queue = device.queue().expect("the device has a queue");
        let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
        let error = encoder
            .render_pass(&desc, |pass| {
                drop(pass.bind_pipeline(&pipeline)?);
                pass.set_vertex_buffers(&[&red[0], &red[1]])?;
                pass.draw(3, 1)
            })
            .expect_err("a pass without a colour target cannot draw");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::Draw);
        queue
            .submit([encoder.finish().expect("the encoder finishes")])
            .expect("the queue receives the work");
        queue.wait_on_host().expect("the queue finishes the work");
    };
    let draw_over = || {
        let desc = slang_rhi::RenderPassDesc::new()
            .with_colour_target(&target, BLACK)
            .with_depth_target(&depth, DepthLoad::Load);
        render(&device, &desc, |pass| {
            drop(pass.bind_pipeline(&pipeline)?);
            pass.set_vertex_buffers(&[&red[0], &red[1]])?;
            pass.draw(3, 1)
        });
        rows(&device, &target)
    };
    clear_depth(0.0);
    assert_eq!(draw_over(), ["...."; 4]);
    clear_depth(1.0);
    assert_eq!(draw_over(), ["RRRR"; 4]);
    assert_no_error(&messages);
}

#[test]
fn a_stream_that_steps_for_each_instance_moves_each_instance() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = render_pipeline(
        &device,
        ("instanced", INSTANCED),
        Format::Rgba8Unorm,
        &[
            ("position", VertexFormat::Float32x2, StepMode::Vertex),
            ("offset", VertexFormat::Float32x2, StepMode::Instance),
        ],
        |desc| desc,
    );
    let target = colour_target(&device, TextureUsage::empty());
    let corners = vertex_buffer(&device, &[-1.0, 1.0, -1.0, 0.4, -0.4, 1.0]);
    let offsets = vertex_buffer(&device, &[0.0, 0.0, 0.5, 0.0, 1.0, -1.0]);
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(&target, BLACK);
    render(&device, &desc, |pass| {
        drop(pass.bind_pipeline(&pipeline)?);
        pass.set_vertex_buffers(&[&corners, &offsets])?;
        pass.draw(3, 3)
    });
    assert_eq!(rows(&device, &target), ["GG..", "....", "..G.", "...."]);
    assert_no_error(&messages);
}

#[test]
fn a_stream_of_stride_0_gives_every_vertex_and_instance_its_one_attribute() {
    let (options, messages) = options(Toggle::On);
    let device = open(&options);
    let linked = link_render(device.slang_session(), "flat", FLAT);
    let reflected = vertex_inputs(device.slang_session(), &linked);
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    let desc = RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
        stream(&reflected, "position", VertexFormat::Float32x3),
        stream(&reflected, "colour", VertexFormat::Float32x4).with_stride(0),
    ]);
    let pipeline = device
        .create_render_pipeline(&program, &desc)
        .expect("slang-rhi creates the render pipeline");
    let target = colour_target(&device, TextureUsage::empty());
    let positions: Vec<f32> = FULL
        .iter()
        .chain(&UPPER_LEFT)
        .flat_map(|[x, y]| [*x, *y, 0.0])
        .collect();
    let positions = vertex_buffer(&device, &positions);
    let colour = vertex_buffer(&device, &GREEN);
    let pass = slang_rhi::RenderPassDesc::new().with_colour_target(&target, BLACK);
    render(&device, &pass, |pass| {
        drop(pass.bind_pipeline(&pipeline)?);
        pass.set_vertex_buffers(&[&positions, &colour])?;
        pass.draw(6, 3)
    });
    assert_eq!(rows(&device, &target), ["GGGG"; 4]);
    assert_no_error(&messages);
}

#[test]
fn alpha_blending_mixes_the_colour_over_the_target_by_its_alpha() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = flat_pipeline(&device, |desc| desc.with_blend(Blend::Alpha));
    let target = colour_target(&device, TextureUsage::empty());
    let [positions, colours] = triangle(&device, &FULL, 0.0, [1.0, 0.0, 0.0, 0.6]);
    let desc =
        slang_rhi::RenderPassDesc::new().with_colour_target(&target, ColourLoad::Clear(BLUE));
    render(&device, &desc, |pass| {
        drop(pass.bind_pipeline(&pipeline)?);
        pass.set_vertex_buffers(&[&positions, &colours])?;
        pass.draw(3, 1)
    });
    for texel in texels(&device, &target) {
        let expected = [153, 0, 102, 255];
        for (channel, value) in texel.iter().zip(expected) {
            assert!(
                channel.abs_diff(value) <= 1,
                "{texel:?} against {expected:?}"
            );
        }
    }
    assert_no_error(&messages);
}

#[test]
fn a_stream_at_a_location_that_the_shader_does_not_declare_leaves_the_inputs_matched() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let linked = link_render(device.slang_session(), "flat", FLAT);
    let reflected = vertex_inputs(device.slang_session(), &linked);
    let locations: Vec<(&str, u32)> = reflected
        .iter()
        .map(|(name, location, _, _)| (name.as_str(), *location))
        .collect();
    assert_eq!(locations, [("position", 0), ("colour", 1)]);
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    let desc = RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
        stream(&reflected, "position", VertexFormat::Float32x3),
        stream(&reflected, "colour", VertexFormat::Float32x4),
        VertexStream::new(VertexFormat::Float32x2, 3).with_semantic("EXTRA", 0),
    ]);
    let pipeline = device
        .create_render_pipeline(&program, &desc)
        .unwrap_or_else(|error| {
            panic!(
                "slang-rhi creates the render pipeline: {error:?}, with the diagnostics {:?}",
                error.diagnostics()
            )
        });
    let target = colour_target(&device, TextureUsage::empty());
    let [positions, colours] = triangle(&device, &FULL, 0.0, GREEN);
    let extra = vertex_buffer(&device, &[0.0; 6]);
    draw_into(
        &device,
        &target,
        &pipeline,
        &[&positions, &colours, &extra],
        3,
    );
    assert_eq!(rows(&device, &target), ["GGGG"; 4]);
    assert_no_error(&messages);
}

#[test]
fn an_input_that_the_shader_declares_and_does_not_read_takes_a_stream_and_leaves_the_others_matched()
 {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let linked = link_render(device.slang_session(), "gap", GAP);
    let reflected = vertex_inputs(device.slang_session(), &linked);
    let locations: Vec<(&str, u32)> = reflected
        .iter()
        .map(|(name, location, _, _)| (name.as_str(), *location))
        .collect();
    assert_eq!(locations, [("position", 0), ("unused", 1), ("colour", 2)]);
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    let desc = RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
        stream(&reflected, "position", VertexFormat::Float32x2),
        stream(&reflected, "unused", VertexFormat::Float32),
        stream(&reflected, "colour", VertexFormat::Float32x4),
    ]);
    let pipeline = device
        .create_render_pipeline(&program, &desc)
        .unwrap_or_else(|error| {
            panic!(
                "slang-rhi creates the render pipeline: {error:?}, with the diagnostics {:?}",
                error.diagnostics()
            )
        });
    let target = colour_target(&device, TextureUsage::empty());
    let corners = vertex_buffer(&device, FULL.as_flattened());
    let unused = vertex_buffer(&device, &[0.0; 3]);
    let colours = vertex_buffer(&device, &[GREEN, GREEN, GREEN].concat());
    draw_into(
        &device,
        &target,
        &pipeline,
        &[&corners, &unused, &colours],
        3,
    );
    assert_eq!(rows(&device, &target), ["GGGG"; 4]);
    assert_no_error(&messages);
}

#[test]
fn a_stream_with_fewer_or_more_components_than_its_input_fills_or_drops_them() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = render_pipeline(
        &device,
        ("components", COMPONENTS),
        Format::Rgba8Unorm,
        &[
            ("position", VertexFormat::Float32x2, StepMode::Vertex),
            ("colour", VertexFormat::Float32x4, StepMode::Vertex),
        ],
        |desc| desc,
    );
    let target = colour_target(&device, TextureUsage::empty());
    let positions = vertex_buffer(&device, FULL.as_flattened());
    let colours = vertex_buffer(&device, &[[0.0, 1.0, 0.5, 0.5]; 3].concat());
    draw_into(&device, &target, &pipeline, &[&positions, &colours], 3);
    assert_eq!(rows(&device, &target), ["GGGG"; 4]);
    assert_no_error(&messages);
}

#[test]
fn a_triangle_outside_the_depth_range_is_clamped_on_vulkan_and_clipped_on_direct3d_12() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = flat_pipeline(&device, |desc| desc);
    for depth in [1.5, -0.5] {
        let target = colour_target(&device, TextureUsage::empty());
        let [positions, colours] = triangle(&device, &FULL, depth, RED);
        draw_into(&device, &target, &pipeline, &[&positions, &colours], 3);
        assert_eq!(rows(&device, &target), OUTSIDE_DEPTH_ROWS, "{depth}");
    }
    assert_no_error(&messages);
}

#[test]
fn a_texture_with_initial_data_reads_back_without_the_padding_of_its_rows() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    for format in [
        Format::Rgba8Unorm,
        Format::Rgba8UnormSrgb,
        Format::Bgra8Unorm,
        Format::Bgra8UnormSrgb,
    ] {
        for size in [[3, 2], [1, 1], [67, 5]] {
            let [width, height] = size;
            let len = usize::try_from(width * height * 4).expect("the size fits");
            let data: Vec<u8> = (0..len)
                .map(|index| u8::try_from(index % 251).expect("below 251"))
                .collect();
            let texture = device
                .create_texture(
                    &TextureDesc::new(
                        size,
                        format,
                        TextureUsage::SHADER_RESOURCE | TextureUsage::COPY_SOURCE,
                    ),
                    Some(&data),
                )
                .expect("slang-rhi creates the texture");
            let mut out = vec![0_u8; len];
            device
                .read_texture(&texture, &mut out)
                .expect("slang-rhi reads the texture");
            assert_eq!(out, data, "{format:?} {size:?}");
        }
    }
    assert_no_error(&messages);
}

#[test]
fn a_zeroed_colour_texture_from_initial_data_reads_back_as_zeros() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let texture = device
        .create_texture(
            &TextureDesc::new(
                SIZE,
                Format::Rgba8Unorm,
                TextureUsage::SHADER_RESOURCE
                    | TextureUsage::RENDER_TARGET
                    | TextureUsage::COPY_SOURCE,
            ),
            Some(&[0; 64]),
        )
        .expect("slang-rhi creates the texture");
    assert_eq!(texels(&device, &texture), [[0; 4]; 16]);
    assert_no_error(&messages);
}

#[test]
fn texture_descriptions_and_initial_data_that_break_a_rule_are_refused() {
    let (options, _messages) = options(Toggle::Off);
    let device = open(&options);
    let largest = device.info().max_texture_dimension_2d();
    let colour = TextureUsage::RENDER_TARGET | TextureUsage::COPY_SOURCE;
    for (desc, initial) in [
        (TextureDesc::new([0, 4], Format::Rgba8Unorm, colour), None),
        (TextureDesc::new([4, 0], Format::Rgba8Unorm, colour), None),
        (
            TextureDesc::new([largest + 1, 1], Format::Rgba8Unorm, colour),
            None,
        ),
        (
            TextureDesc::new(SIZE, Format::Rgba8Unorm, TextureUsage::empty()),
            None,
        ),
        (
            TextureDesc::new(
                SIZE,
                Format::Rgba8Unorm,
                TextureUsage::from_bits_retain(1 << 1),
            ),
            None,
        ),
        (
            TextureDesc::new(
                SIZE,
                Format::Rgba8Unorm,
                colour | TextureUsage::DEPTH_STENCIL,
            ),
            None,
        ),
        (
            TextureDesc::new(SIZE, Format::D32Float, TextureUsage::SHADER_RESOURCE),
            None,
        ),
        (
            TextureDesc::new(SIZE, Format::D32Float, TextureUsage::DEPTH_STENCIL),
            Some(vec![0; 64]),
        ),
        (
            TextureDesc::new(SIZE, Format::Rgba8Unorm, colour),
            Some(vec![0; 63]),
        ),
    ] {
        let error = device
            .create_texture(&desc, initial.as_deref())
            .map(drop)
            .expect_err("the texture is refused");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{desc:?}");
        assert_eq!(error.operation(), Operation::CreateTexture);
    }
}

#[test]
fn reading_a_texture_that_cannot_be_read_is_refused() {
    let (options, _messages) = options(Toggle::Off);
    let device = open(&options);
    let other = open(&options);
    let readable = colour_target(&device, TextureUsage::empty());
    let unreadable = device
        .create_texture(
            &TextureDesc::new(SIZE, Format::Rgba8Unorm, TextureUsage::RENDER_TARGET),
            None,
        )
        .expect("slang-rhi creates the texture");
    let depth = depth_target(&device);
    for (reader, texture, len) in [
        (&other, &readable, 64),
        (&device, &unreadable, 64),
        (&device, &depth, 64),
        (&device, &readable, 63),
        (&device, &readable, 65),
    ] {
        let mut out = vec![0_u8; len];
        let error = reader
            .read_texture(texture, &mut out)
            .expect_err("the read is refused");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::ReadTexture);
    }
}

#[test]
fn render_pipelines_that_break_a_rule_are_refused_before_slang_rhi_compiles_them() {
    let (options, _messages) = options(Toggle::Off);
    let device = open(&options);
    let other = open(&options);
    let linked = link_render(device.slang_session(), "flat", FLAT);
    let reflected = vertex_inputs(device.slang_session(), &linked);
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    let position = stream(&reflected, "position", VertexFormat::Float32x3);
    let colour = stream(&reflected, "colour", VertexFormat::Float32x4);
    let valid = RenderPipelineDesc::new(Format::Rgba8Unorm)
        .with_vertex_streams([position.clone(), colour.clone()]);
    let other_linked = link_render(other.slang_session(), "flat", FLAT);
    let other_program = other
        .create_shader_program(&other_linked)
        .expect("slang-rhi creates the shader program");
    let error = device
        .create_render_pipeline(&other_program, &valid)
        .map(drop)
        .expect_err("a program of another device");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    let vertex_only = {
        let session = device.slang_session();
        let module = session
            .load_module_from_source("only", "only.slang", FLAT)
            .expect("the source compiles")
            .into_value();
        let vertex = module
            .entry_point_with_stage("vertex_main", Stage::Vertex)
            .expect("the entry point exists")
            .into_value();
        let linked = session
            .compose(&[Component::Module(&module), Component::EntryPoint(&vertex)])
            .expect("the parts compose")
            .into_value()
            .link()
            .expect("the program links")
            .into_value();
        device
            .create_shader_program(&linked)
            .expect("slang-rhi creates the shader program")
    };
    let error = device
        .create_render_pipeline(&vertex_only, &valid)
        .map(drop)
        .expect_err("no fragment entry point");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    let max_streams = device.info().max_vertex_streams().min(16);
    let too_many: Vec<VertexStream> = (0..=max_streams)
        .map(|location| {
            VertexStream::new(VertexFormat::Float32, location).with_semantic("EXTRA", location)
        })
        .collect();
    let max_elements = device.info().max_vertex_input_elements();
    for desc in [
        RenderPipelineDesc::new(Format::D32Float)
            .with_vertex_streams([position.clone(), colour.clone()]),
        valid.clone().with_depth_format(Format::Rgba8Unorm),
        RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams(too_many),
        RenderPipelineDesc::new(Format::Rgba8Unorm)
            .with_vertex_streams([position.clone().with_stride(2052), colour.clone()]),
        RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
            VertexStream::new(VertexFormat::Float32x3, 0),
            colour.clone(),
        ]),
        RenderPipelineDesc::new(Format::Rgba8Unorm)
            .with_vertex_streams([position.clone(), position.clone()]),
        RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
            position.clone(),
            VertexStream::new(VertexFormat::Float32x4, max_elements).with_semantic("COLOR", 0),
        ]),
        RenderPipelineDesc::new(Format::Rgba8Unorm)
            .with_vertex_streams([position.clone(), colour.clone().with_stride(18)]),
        RenderPipelineDesc::new(Format::Rgba8Unorm)
            .with_vertex_streams([position.clone().with_stride(9), colour.clone()]),
    ] {
        let error = device
            .create_render_pipeline(&program, &desc)
            .map(drop)
            .expect_err("the description is refused");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{desc:?}");
        assert_eq!(error.operation(), Operation::CreateRenderPipeline);
    }
    let compute = crate::common::link(device.slang_session(), "double", crate::common::DOUBLE);
    let compute = device
        .create_shader_program(&compute)
        .expect("slang-rhi creates the shader program");
    let error = device
        .create_render_pipeline(&compute, &valid)
        .map(drop)
        .expect_err("a compute kernel");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    device
        .create_render_pipeline(&program, &valid)
        .expect("the valid description creates a pipeline");
}

#[test]
fn render_pipelines_whose_streams_do_not_suit_the_vertex_inputs_are_refused() {
    let (options, _messages) = options(Toggle::Off);
    let device = open(&options);
    let linked = link_render(device.slang_session(), "flat", FLAT);
    let reflected = vertex_inputs(device.slang_session(), &linked);
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    let position = stream(&reflected, "position", VertexFormat::Float32x3);
    let colour = stream(&reflected, "colour", VertexFormat::Float32x4);
    for desc in [
        RenderPipelineDesc::new(Format::Rgba8Unorm),
        RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([position.clone()]),
        RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
            stream(&reflected, "position", VertexFormat::Uint32x3),
            colour.clone(),
        ]),
        RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
            stream(&reflected, "position", VertexFormat::Sint32x3),
            colour.clone(),
        ]),
        RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
            position.clone(),
            VertexStream::new(VertexFormat::Float32x4, 1).with_semantic("TEXCOORD", 0),
        ]),
        RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
            position.clone(),
            VertexStream::new(VertexFormat::Float32x4, 1).with_semantic("COLOR", 1),
        ]),
    ] {
        let error = device
            .create_render_pipeline(&program, &desc)
            .map(drop)
            .expect_err("the description is refused");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{desc:?}");
        assert_eq!(error.operation(), Operation::CreateRenderPipeline);
    }
    for (name, source, streams) in [
        (
            "gap",
            GAP,
            [
                ("position", VertexFormat::Float32x2),
                ("colour", VertexFormat::Float32x4),
            ],
        ),
        (
            "kinds",
            KINDS,
            [
                ("position", VertexFormat::Float32x2),
                ("other", VertexFormat::Float32x2),
            ],
        ),
    ] {
        let linked = link_render(device.slang_session(), name, source);
        let reflected = vertex_inputs(device.slang_session(), &linked);
        let program = device
            .create_shader_program(&linked)
            .expect("slang-rhi creates the shader program");
        let desc = RenderPipelineDesc::new(Format::Rgba8Unorm)
            .with_vertex_streams(streams.map(|(input, format)| stream(&reflected, input, format)));
        let error = device
            .create_render_pipeline(&program, &desc)
            .map(drop)
            .expect_err("an input between two streams has no stream");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{name}");
        assert_eq!(error.operation(), Operation::CreateRenderPipeline);
    }
}

#[test]
fn render_passes_with_targets_that_break_a_rule_are_refused_before_they_begin() {
    let (options, _messages) = options(Toggle::Off);
    let device = open(&options);
    let other = open(&options);
    let target = colour_target(&device, TextureUsage::empty());
    let other_target = colour_target(&other, TextureUsage::empty());
    let no_use = device
        .create_texture(
            &TextureDesc::new(SIZE, Format::Rgba8Unorm, TextureUsage::SHADER_RESOURCE),
            None,
        )
        .expect("slang-rhi creates the texture");
    let depth = depth_target(&device);
    let small_depth = device
        .create_texture(
            &TextureDesc::new([2, 2], Format::D32Float, TextureUsage::DEPTH_STENCIL),
            None,
        )
        .expect("slang-rhi creates the depth texture");
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    for desc in [
        slang_rhi::RenderPassDesc::new(),
        slang_rhi::RenderPassDesc::new().with_colour_target(&other_target, BLACK),
        slang_rhi::RenderPassDesc::new().with_colour_target(&no_use, BLACK),
        slang_rhi::RenderPassDesc::new().with_colour_target(&depth, BLACK),
        slang_rhi::RenderPassDesc::new().with_depth_target(&target, DepthLoad::Clear(1.0)),
        slang_rhi::RenderPassDesc::new()
            .with_colour_target(&target, BLACK)
            .with_depth_target(&small_depth, DepthLoad::Clear(1.0)),
        slang_rhi::RenderPassDesc::new().with_depth_target(&depth, DepthLoad::Clear(2.0)),
    ] {
        let error = encoder
            .render_pass(&desc, |_| Ok(()))
            .expect_err("the pass is refused");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{desc:?}");
        assert_eq!(error.operation(), Operation::BeginRenderPass);
    }
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
}

#[test]
fn draws_that_break_a_rule_are_refused_and_the_pass_still_ends() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let other = open(&options);
    let pipeline = flat_pipeline(&device, |desc| desc);
    let srgb = render_pipeline(
        &device,
        ("flat", FLAT),
        Format::Rgba8UnormSrgb,
        &FLAT_INPUTS,
        |desc| desc,
    );
    let depth_pipeline = flat_pipeline(&device, |desc| desc.with_depth_format(Format::D32Float));
    let other_pipeline = flat_pipeline(&other, |desc| desc);
    let target = colour_target(&device, TextureUsage::empty());
    let [positions, colours] = triangle(&device, &UPPER_LEFT, 0.0, RED);
    let short = vertex_buffer(&device, &[0.0; 8]);
    let indices = index_buffer(&device, &[0, 1]);
    let not_vertex = crate::common::float_buffer(&device, &[0.0; 12]);
    let other_buffer = vertex_buffer(&other, &[0.0; 12]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(&target, BLACK);
    let errors = encoder
        .render_pass(&desc, |pass| {
            let mut errors = vec![pass.draw(3, 1).expect_err("nothing is bound")];
            errors.push(
                pass.bind_pipeline(&other_pipeline)
                    .map(drop)
                    .expect_err("a pipeline of another device"),
            );
            drop(pass.bind_pipeline(&srgb)?);
            errors.push(pass.draw(3, 1).expect_err("another colour format"));
            drop(pass.bind_pipeline(&depth_pipeline)?);
            errors.push(
                pass.draw(3, 1)
                    .expect_err("a depth format without a depth target"),
            );
            drop(pass.bind_pipeline(&pipeline)?);
            errors.push(pass.draw(3, 1).expect_err("no vertex buffer"));
            pass.set_vertex_buffers(&[&positions])?;
            errors.push(pass.draw(3, 1).expect_err("one vertex buffer of two"));
            pass.set_vertex_buffers(&[&positions, &short])?;
            errors.push(pass.draw(3, 1).expect_err("a short vertex buffer"));
            errors.push(
                pass.set_vertex_buffers(&[&positions, &not_vertex])
                    .expect_err("a buffer without the use VERTEX_BUFFER"),
            );
            errors.push(
                pass.set_vertex_buffers(&[&other_buffer, &colours])
                    .expect_err("a buffer of another device"),
            );
            pass.set_vertex_buffers(&[&positions, &colours])?;
            errors.push(
                pass.draw(4, 1)
                    .expect_err("more vertices than the buffers hold"),
            );
            errors.push(pass.draw_indexed(3, 1).expect_err("no index buffer"));
            errors.push(
                pass.set_index_buffer(&positions)
                    .expect_err("a buffer without the use INDEX_BUFFER"),
            );
            pass.set_index_buffer(&indices)?;
            errors.push(pass.draw_indexed(3, 1).expect_err("a short index buffer"));
            Ok(errors)
        })
        .expect("the closure returns the errors");
    for error in &errors {
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{error}");
    }
    assert_eq!(errors.len(), 13);
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_eq!(rows(&device, &target), ["...."; 4]);
    assert_no_error(&messages);
}

#[test]
fn a_texture_that_a_pass_both_samples_and_draws_into_is_refused() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = render_pipeline(
        &device,
        ("textured", TEXTURED),
        Format::Rgba8Unorm,
        &TEXTURED_INPUTS,
        |desc| desc,
    );
    let target = colour_target(&device, TextureUsage::SHADER_RESOURCE);
    let sampler = device
        .create_sampler(&SamplerDesc::new())
        .expect("slang-rhi creates the sampler");
    let corners = vertex_buffer(&device, FULL.as_flattened());
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(&target, BLACK);
    let error = encoder
        .render_pass(&desc, |pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let colours = root
                .offset_of("colours")?
                .expect("the shader declares colours");
            root.set_texture(colours, &target)?;
            let nearest = root
                .offset_of("nearest")?
                .expect("the shader declares nearest");
            root.set_sampler(nearest, &sampler)?;
            drop(root);
            pass.set_vertex_buffers(&[&corners])?;
            pass.draw(3, 1)
        })
        .expect_err("the target is sampled");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.operation(), Operation::Draw);
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_no_error(&messages);
}

#[test]
fn textures_and_samplers_that_do_not_suit_their_parameter_are_refused() {
    let (options, _messages) = options(Toggle::Off);
    let device = open(&options);
    let other = open(&options);
    let pipeline = render_pipeline(
        &device,
        ("textured", TEXTURED),
        Format::Rgba8Unorm,
        &TEXTURED_INPUTS,
        |desc| desc,
    );
    let shader_texture = device
        .create_texture(
            &TextureDesc::new(SIZE, Format::Rgba8Unorm, TextureUsage::SHADER_RESOURCE),
            None,
        )
        .expect("slang-rhi creates the texture");
    let unsampled = colour_target(&device, TextureUsage::empty());
    let other_texture = other
        .create_texture(
            &TextureDesc::new(SIZE, Format::Rgba8Unorm, TextureUsage::SHADER_RESOURCE),
            None,
        )
        .expect("slang-rhi creates the texture");
    let sampler = device
        .create_sampler(&SamplerDesc::new())
        .expect("slang-rhi creates the sampler");
    let other_sampler = other
        .create_sampler(&SamplerDesc::new())
        .expect("slang-rhi creates the sampler");
    let target = colour_target(&device, TextureUsage::empty());
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(&target, BLACK);
    let errors = encoder
        .render_pass(&desc, |pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let colours = root
                .offset_of("colours")?
                .expect("the shader declares colours");
            let nearest = root
                .offset_of("nearest")?
                .expect("the shader declares nearest");
            let errors = vec![
                root.set_texture(colours, &unsampled)
                    .expect_err("no use SHADER_RESOURCE"),
                root.set_texture(colours, &other_texture)
                    .expect_err("another device"),
                root.set_texture(nearest, &shader_texture)
                    .expect_err("a sampler parameter"),
                root.set_texture(colours.with_binding_array_index(1), &shader_texture)
                    .expect_err("past the range"),
                root.set_sampler(colours, &sampler)
                    .expect_err("a texture parameter"),
                root.set_sampler(nearest, &other_sampler)
                    .expect_err("another device"),
            ];
            root.set_texture(colours, &shader_texture)?;
            root.set_sampler(nearest, &sampler)?;
            Ok(errors)
        })
        .expect("the closure returns the errors");
    let operations: Vec<Operation> = errors.iter().map(slang_rhi::Error::operation).collect();
    assert_eq!(
        operations,
        [
            Operation::SetTexture,
            Operation::SetTexture,
            Operation::SetTexture,
            Operation::SetTexture,
            Operation::SetSampler,
            Operation::SetSampler,
        ]
    );
    for error in &errors {
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{error}");
    }
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
}

#[test]
fn textures_and_samplers_of_types_that_the_crate_does_not_create_are_refused() {
    let (options, _messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = render_pipeline(
        &device,
        ("unsuited", UNSUITED),
        Format::Rgba8Unorm,
        &TEXTURED_INPUTS,
        |desc| desc,
    );
    let texture = device
        .create_texture(
            &TextureDesc::new(SIZE, Format::Rgba8Unorm, TextureUsage::SHADER_RESOURCE),
            None,
        )
        .expect("slang-rhi creates the texture");
    let sampler = device
        .create_sampler(&SamplerDesc::new())
        .expect("slang-rhi creates the sampler");
    let target = colour_target(&device, TextureUsage::empty());
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let desc = slang_rhi::RenderPassDesc::new().with_colour_target(&target, BLACK);
    let errors = encoder
        .render_pass(&desc, |pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let mut errors = Vec::new();
            for name in ["integers", "volume", "cube", "layers", "multisampled"] {
                let offset = root
                    .offset_of(name)?
                    .unwrap_or_else(|| panic!("the shader declares {name}"));
                errors.push(
                    root.set_texture(offset, &texture)
                        .expect_err("a texture that the crate does not create"),
                );
            }
            let comparison = root
                .offset_of("comparison")?
                .expect("the shader declares comparison");
            errors.push(
                root.set_sampler(comparison, &sampler)
                    .expect_err("a comparison sampler"),
            );
            Ok(errors)
        })
        .expect("the closure returns the errors");
    let operations: Vec<Operation> = errors.iter().map(slang_rhi::Error::operation).collect();
    assert_eq!(
        operations,
        [
            Operation::SetTexture,
            Operation::SetTexture,
            Operation::SetTexture,
            Operation::SetTexture,
            Operation::SetTexture,
            Operation::SetSampler,
        ]
    );
    for error in &errors {
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{error}");
    }
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
}

#[test]
fn a_panic_in_a_render_pass_ends_the_pass_and_the_encoder_still_finishes() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let target = colour_target(&device, TextureUsage::empty());
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let desc =
        slang_rhi::RenderPassDesc::new().with_colour_target(&target, ColourLoad::Clear(GREEN));
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        encoder.render_pass(&desc, |_| -> slang_rhi::Result<()> {
            std::panic::resume_unwind(Box::new("a panic in the pass"))
        })
    }));
    assert!(outcome.is_err());
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_eq!(rows(&device, &target), ["GGGG"; 4]);
    assert_no_error(&messages);
}
