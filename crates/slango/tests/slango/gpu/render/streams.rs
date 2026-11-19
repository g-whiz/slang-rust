//! The vertex attributes that a draw gives no buffer, the semantics and the
//! number of the vertex attributes that the backend rhi needs, the depths of
//! a new depth target, and resources among the parameters of an entry point.

use slango::{
    AdapterSelection, Buffer, Device, RenderPipelineOptions, Sampler, SamplerOptions, Texture,
    TextureFormat,
};

use crate::common::{
    FLAT, GREEN, GREEN_PIXEL, RED_PIXEL, Triangles, buffer, device_with, load, pixel_at, read,
    render_pipeline, texture,
};

const SIZE: u32 = 4;
const BLACK_PIXEL: [u8; 4] = [0, 0, 0, 255];

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

/// A module whose vertex entry point declares the attribute `input.unused`
/// between its two other attributes and never reads it.
const UNUSED: &str = r#"
struct VIn { float2 position : POSITION; float4 unused : UNUSED; float4 colour : COLOR; };
struct VOut { float4 position : SV_Position; float4 colour : COLOR; };

[shader("vertex")]
VOut vsMain(VIn input, uint3 shift : SHIFT)
{
    VOut output;
    output.position = float4(input.position, 0.0, 1.0);
    output.colour = input.colour;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target { return input.colour; }
"#;

/// A triangle that covers the whole target.
const FULL: [[f32; 2]; 3] = [[-1.0, -1.0], [3.0, -1.0], [-1.0, 3.0]];

#[test]
fn attributes_that_the_shader_declares_and_does_not_use_need_no_buffer_in_any_draw() {
    let device = device();
    let mut instanced = RenderPipelineOptions::default();
    instanced.instance_attributes = vec!["input.unused".to_owned(), "shift".to_owned()];
    for options in [RenderPipelineOptions::default(), instanced] {
        let pipeline = render_pipeline(&device, UNUSED, &options);
        let positions = buffer(&device, &FULL);
        let colours = buffer(&device, &[GREEN; 3]);
        let indices = buffer(&device, &[0_u32, 1, 2, 2, 1, 0, 0, 2, 1]);
        let short = buffer(&device, &[[1.0_f32; 4]]);
        for (indexed, instances, given) in [
            (false, 1, false),
            (false, 5, false),
            (true, 7, false),
            (false, 3, true),
        ] {
            let target = texture(&device, [SIZE, SIZE], TextureFormat::Rgba8Unorm);
            let mut draw = pipeline.draw();
            draw.vertex("input.position", &positions)
                .vertex("input.colour", &colours)
                .target(&target)
                .clear([0.0, 0.0, 0.0, 1.0]);
            if given {
                draw.vertex("input.unused", &short);
            }
            let drawn = if indexed {
                draw.submit_indexed(&indices, 9, instances)
            } else {
                draw.submit(3, instances)
            };
            drawn.unwrap_or_else(|error| {
                panic!("{options:?}, indexed {indexed}, {instances} instances: {error}")
            });
            assert_eq!(
                read(&target),
                [GREEN_PIXEL; 16],
                "{options:?}, indexed {indexed}, {instances} instances"
            );
        }
    }
}

#[test]
fn a_new_depth_target_holds_the_depth_0_so_a_draw_that_loads_it_draws_nothing() {
    let device = device();
    let mut options = RenderPipelineOptions::default();
    options.depth_format = Some(TextureFormat::Depth32Float);
    let pipeline = render_pipeline(&device, FLAT, &options);
    let triangles = Triangles::new(
        &device,
        &[(
            [[-1.0, -1.0, 0.0], [3.0, -1.0, 0.0], [-1.0, 3.0, 0.0]],
            GREEN,
        )],
    );
    let target = texture(&device, [SIZE, SIZE], TextureFormat::Rgba8Unorm);
    let depth = texture(&device, [SIZE, SIZE], TextureFormat::Depth32Float);
    pipeline
        .draw()
        .vertex("input.position", &triangles.positions)
        .vertex("input.colour", &triangles.colours)
        .target(&target)
        .clear([0.0, 0.0, 0.0, 1.0])
        .depth(&depth)
        .submit(triangles.vertices, 1)
        .expect("the draw");
    assert_eq!(read(&target), [BLACK_PIXEL; 16]);
    pipeline
        .draw()
        .vertex("input.position", &triangles.positions)
        .vertex("input.colour", &triangles.colours)
        .target(&target)
        .depth(&depth)
        .clear_depth(1.0)
        .submit(triangles.vertices, 1)
        .expect("the draw");
    assert_eq!(read(&target), [GREEN_PIXEL; 16]);
}

#[test]
fn a_texture_and_a_sampler_among_the_parameters_of_the_fragment_entry_point_are_bound_by_name()
-> slango::Result<()> {
    let device = device();
    let pipeline = load(
        &device,
        r#"
struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };

[shader("vertex")]
VOut vsMain(float2 position : POSITION)
{
    VOut output;
    output.position = float4(position, 0.0, 1.0);
    output.uv = position * float2(0.5, -0.5) + 0.5;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input, uniform Texture2D<float4> image, uniform SamplerState imageSampler) : SV_Target
{
    return image.Sample(imageSampler, input.uv);
}
"#,
    )
    .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
    let image = Texture::from_pixels(&device, [1, 1], &[RED_PIXEL])?;
    let sampler = Sampler::new(&device, &SamplerOptions::default())?;
    let positions = Buffer::from_slice(&device, &FULL)?;
    let target = Texture::new(&device, [SIZE, SIZE], TextureFormat::Rgba8Unorm)?;
    pipeline
        .draw()
        .texture("image", &image)
        .sampler("imageSampler", &sampler)
        .vertex("position", &positions)
        .target(&target)
        .submit(3, 1)?;
    let pixels = target.read()?;
    assert_eq!(pixel_at(&pixels, SIZE, 1, 1), RED_PIXEL);
    assert_eq!(pixels, [RED_PIXEL; 16]);
    Ok(())
}

#[cfg(feature = "rhi")]
#[test]
fn a_vertex_attribute_without_a_semantic_is_unsupported_on_rhi() {
    use slango::ErrorKind;

    let module = load(
        &device(),
        r#"
struct VOut { float4 position : SV_Position; };

[shader("vertex")]
VOut vsMain(float2 position : POSITION, float shade)
{
    VOut output;
    output.position = float4(position * shade, 0.0, 1.0);
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target { return float4(1.0, 1.0, 1.0, 1.0); }
"#,
    );
    let error = module
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect_err("an attribute without a semantic");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter, "{error}");
    assert_eq!(
        error.to_string(),
        "cannot create a render pipeline: for `vsMain` and `fsMain`, the vertex attribute `shade` has no semantic, which slango with the feature `rhi` needs, because Direct3D 12 matches vertex attributes by semantic"
    );
}

#[cfg(feature = "rhi")]
#[test]
fn more_vertex_attributes_than_vertex_streams_are_an_invalid_argument_on_rhi() {
    use slango::ErrorKind;

    let count = 17;
    let fields: Vec<String> = (0..count)
        .map(|index| format!("float a{index} : A{index};"))
        .collect();
    let sum: Vec<String> = (0..count).map(|index| format!("input.a{index}")).collect();
    let source = format!(
        r#"
struct VIn {{ {} }};

[shader("vertex")]
float4 vsMain(VIn input) : SV_Position {{ return float4({}, 0.0, 0.0, 1.0); }}

[shader("fragment")]
float4 fsMain() : SV_Target {{ return float4(1.0, 1.0, 1.0, 1.0); }}
"#,
        fields.join(" "),
        sum.join(" + ")
    );
    let error = load(&device(), &source)
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect_err("more attributes than streams");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{error}");
    let text = error.to_string();
    assert!(
        text.starts_with(
            "cannot create a render pipeline: for `vsMain` and `fsMain`, the vertex entry point has 17 vertex attributes, each of which needs a vertex stream, more than the "
        ),
        "{text}"
    );
}
