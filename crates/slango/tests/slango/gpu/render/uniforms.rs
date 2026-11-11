use slango::{AdapterSelection, Buffer, RenderPipelineOptions, Texture, TextureFormat};

use crate::common::{device_with, load, pixel_at};

const SIZE: u32 = 32;

/// A square that the vertex stage sizes by `both.size` and
/// `placement.scale`, moves by `placement.offset`, and transforms by
/// `transform`, and that the fragment stage colours from `frame.tint`,
/// `brightness`, and `both.level`.
///
/// `transform` and `brightness` share the uniform buffer of the global
/// values, which both stages therefore use. The vertex stage alone uses the
/// parameter block `placement`, the fragment stage alone uses the constant
/// buffer `frame`, both stages use the constant buffer `both`, and neither
/// uses `unusedBlock` or `unusedImage`.
const UNIFORMS: &str = r#"
struct Placement { float2 offset; float scale; };
struct Both { float size; float level; };
struct Frame { float4 tint; };
struct Unused { float4 nothing; };

uniform float4x4 transform;
uniform float brightness;
ConstantBuffer<Frame> frame;
ParameterBlock<Placement> placement;
ConstantBuffer<Both> both;
ConstantBuffer<Unused> unusedBlock;
Texture2D<float4> unusedImage;

[shader("vertex")]
float4 vsMain(float2 corner : POSITION) : SV_Position
{
    float2 local = corner * both.size * placement.scale + placement.offset;
    return mul(transform, float4(local, 0.5, 1.0));
}

[shader("fragment")]
float4 fsMain() : SV_Target
{
    return float4(frame.tint.r, brightness, both.level, 1.0);
}
"#;

const SQUARE: [[f32; 2]; 6] = [
    [-1.0, -1.0],
    [1.0, -1.0],
    [1.0, 1.0],
    [-1.0, -1.0],
    [1.0, 1.0],
    [-1.0, 1.0],
];

/// The translation by (0.5, 0.25) in clip space, written as rows, which is
/// 8 pixels to the right and 4 pixels up on a target of 32 by 32 pixels.
const TRANSLATION: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.5],
    [0.0, 1.0, 0.0, 0.25],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

#[test]
fn uniforms_used_by_one_or_both_stages_are_set_by_name_and_unused_ones_may_be_omitted() {
    let device = device_with(AdapterSelection::Default);
    let pipeline = load(&device, UNIFORMS)
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the pipeline");
    let corners = Buffer::from_slice(&device, &SQUARE).expect("the corners");
    let unused_image = Texture::new(&device, [1, 1], TextureFormat::Rgba8Unorm).expect("a texture");
    let fill = [255, 128, 64, 255];
    let background = [0, 0, 0, 255];
    for give_unused in [false, true] {
        let target =
            Texture::new(&device, [SIZE, SIZE], TextureFormat::Rgba8Unorm).expect("a target");
        let mut draw = pipeline.draw();
        draw.set("transform", TRANSLATION)
            .set("brightness", 128.0_f32 / 255.0)
            .set("frame.tint", [1.0_f32, 0.0, 0.0, 0.0])
            .set("placement.offset", [-0.25_f32, 0.0])
            .set("placement.scale", 2.0_f32)
            .set("both.size", 0.125_f32)
            .set("both.level", 64.0_f32 / 255.0)
            .vertex("corner", &corners)
            .target(&target)
            .clear([0.0, 0.0, 0.0, 1.0]);
        if give_unused {
            draw.set("unusedBlock.nothing", [1.0_f32; 4])
                .texture("unusedImage", &unused_image);
        }
        draw.submit(6, 1).expect("the draw");
        let pixels = target.read().expect("the pixels");
        for (x, y) in [(20, 12), (17, 9), (23, 15), (17, 15), (23, 9)] {
            assert_eq!(pixel_at(&pixels, SIZE, x, y), fill, "inside at ({x}, {y})");
        }
        for (x, y) in [(15, 12), (24, 12), (20, 7), (20, 16), (8, 16), (16, 16)] {
            assert_eq!(
                pixel_at(&pixels, SIZE, x, y),
                background,
                "outside at ({x}, {y})"
            );
        }
    }
}
