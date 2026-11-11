use slango::{AdapterSelection, Buffer, ErrorKind, RenderPipelineOptions, TextureFormat};

use crate::common::{
    BLUE, BLUE_PIXEL, GREEN, GREEN_PIXEL, buffer, device_with, read, render_pipeline, texture,
};

const COLOURED: &str = r#"
StructuredBuffer<float4> colours;

[shader("vertex")]
float4 vsMain(float3 position : POSITION) : SV_Position { return float4(position, 1.0); }

[shader("fragment")]
float4 fsMain() : SV_Target { return colours[0]; }
"#;

#[test]
fn a_draw_rejects_a_buffer_of_another_scalar_type_and_draws_with_the_same_one() {
    let device = device_with(AdapterSelection::Default);
    let pipeline = render_pipeline(&device, COLOURED, &RenderPipelineOptions::default());
    let positions = buffer(
        &device,
        &[[-1.0_f32, -1.0, 0.5], [3.0, -1.0, 0.5], [-1.0, 3.0, 0.5]],
    );
    let target = texture(&device, [2, 2], TextureFormat::Rgba8Unorm);
    let blue = buffer(&device, &[BLUE]);
    let cleared = pipeline
        .draw()
        .bind("colours", &blue)
        .vertex("position", &positions)
        .target(&target)
        .clear(BLUE)
        .submit(0, 1);
    assert!(cleared.is_ok(), "the clear: {cleared:?}");
    let bits: Buffer<[u32; 4]> = buffer(&device, &[GREEN.map(f32::to_bits)]);
    let rejected = pipeline
        .draw()
        .bind("colours", &bits)
        .vertex("position", &positions)
        .target(&target)
        .submit(3, 1)
        .map_err(|error| (error.kind(), error.to_string()));
    assert_eq!(
        rejected,
        Err((
            ErrorKind::InvalidArgument,
            "cannot draw with a render pipeline: the parameter `colours` of the render pipeline of `vsMain` and `fsMain` has elements of the Slang type `float4`, and the bound buffer has elements of the Rust type `[u32; 4]`"
                .to_owned()
        ))
    );
    assert_eq!(read(&target), [BLUE_PIXEL; 4]);
    let colours = buffer(&device, &[GREEN]);
    let drawn = pipeline
        .draw()
        .bind("colours", &colours)
        .vertex("position", &positions)
        .target(&target)
        .submit(3, 1);
    assert!(drawn.is_ok(), "the draw: {drawn:?}");
    assert_eq!(read(&target), [GREEN_PIXEL; 4]);
}
