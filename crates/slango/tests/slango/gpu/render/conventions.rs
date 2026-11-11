use slango::{
    AdapterSelection, Buffer, CullMode, Device, RenderPipelineOptions, Texture, TextureFormat,
};

use crate::common::{
    BLUE, BLUE_PIXEL, FLAT, GREEN, GREEN_PIXEL, RED, RED_PIXEL, Triangles, device_with, load,
    pixel_at,
};

const SIZE: u32 = 16;

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

fn target(device: &Device) -> slango::Result<Texture> {
    Texture::new(device, [SIZE, SIZE], TextureFormat::Rgba8Unorm)
}

/// The pixels at the centres of the four corners of the target, in the
/// order upper left, upper right, lower left, and lower right.
fn corners(pixels: &[[u8; 4]]) -> [[u8; 4]; 4] {
    [
        pixel_at(pixels, SIZE, 1, 1),
        pixel_at(pixels, SIZE, SIZE - 2, 1),
        pixel_at(pixels, SIZE, 1, SIZE - 2),
        pixel_at(pixels, SIZE, SIZE - 2, SIZE - 2),
    ]
}

#[test]
fn a_triangle_in_the_upper_left_of_clip_space_lands_in_the_upper_left_rows_of_the_target() {
    let device = device();
    let module = load(&device, FLAT);
    let pipeline = module
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the pipeline");
    let target = target(&device).expect("a target");
    let triangle = Triangles::new(
        &device,
        &[([[-1.0, 1.0, 0.5], [-1.0, 0.0, 0.5], [0.0, 1.0, 0.5]], RED)],
    );
    triangle
        .draw(&pipeline, &target, Some(BLUE))
        .expect("the draw");
    let pixels = target.read().expect("the pixels");
    assert_eq!(
        corners(&pixels),
        [RED_PIXEL, BLUE_PIXEL, BLUE_PIXEL, BLUE_PIXEL]
    );
    assert_eq!(pixel_at(&pixels, SIZE, 0, 0), RED_PIXEL);
    assert_eq!(pixel_at(&pixels, SIZE, 7, 7), BLUE_PIXEL);
    assert_eq!(pixel_at(&pixels, SIZE, 5, 1), RED_PIXEL);
    assert_eq!(pixel_at(&pixels, SIZE, 1, 5), RED_PIXEL);
}

#[test]
fn back_face_culling_keeps_the_counter_clockwise_triangle_and_discards_its_clockwise_twin() {
    let device = device();
    let module = load(&device, FLAT);
    let triangles = Triangles::new(
        &device,
        &[
            ([[-1.0, -1.0, 0.5], [0.0, -1.0, 0.5], [-1.0, 1.0, 0.5]], RED),
            ([[1.0, -1.0, 0.5], [0.0, -1.0, 0.5], [1.0, 1.0, 0.5]], GREEN),
        ],
    );
    for (cull_mode, right) in [(CullMode::None, GREEN_PIXEL), (CullMode::Back, BLUE_PIXEL)] {
        let mut options = RenderPipelineOptions::default();
        options.cull_mode = cull_mode;
        let pipeline = module
            .render_pipeline("vsMain", "fsMain", &options)
            .expect("the pipeline");
        let target = target(&device).expect("a target");
        triangles
            .draw(&pipeline, &target, Some(BLUE))
            .expect("the draw");
        let pixels = target.read().expect("the pixels");
        assert_eq!(
            [
                pixel_at(&pixels, SIZE, 1, SIZE - 2),
                pixel_at(&pixels, SIZE, SIZE - 2, SIZE - 2)
            ],
            [RED_PIXEL, right],
            "{cull_mode:?}"
        );
    }
}

#[test]
fn the_nearer_triangle_hides_the_farther_one_in_either_order_within_one_draw() {
    let device = device();
    let module = load(&device, FLAT);
    let mut options = RenderPipelineOptions::default();
    options.depth_format = Some(TextureFormat::Depth32Float);
    let pipeline = module
        .render_pipeline("vsMain", "fsMain", &options)
        .expect("the pipeline");
    let far = (
        [[-1.0, -1.0, 0.75], [3.0, -1.0, 0.75], [-1.0, 3.0, 0.75]],
        GREEN,
    );
    let near = (
        [[-1.0, -1.0, 0.25], [0.5, -1.0, 0.25], [-1.0, 0.5, 0.25]],
        RED,
    );
    for (order, triangles) in [("far first", [far, near]), ("near first", [near, far])] {
        let triangles = Triangles::new(&device, &triangles);
        let target = target(&device).expect("a target");
        let depth = Texture::new(&device, [SIZE, SIZE], TextureFormat::Depth32Float)
            .expect("a depth target");
        pipeline
            .draw()
            .vertex("input.position", &triangles.positions)
            .vertex("input.colour", &triangles.colours)
            .target(&target)
            .clear(BLUE)
            .depth(&depth)
            .clear_depth(1.0)
            .submit(triangles.vertices, 1)
            .expect("the draw");
        let pixels = target.read().expect("the pixels");
        assert_eq!(
            corners(&pixels),
            [GREEN_PIXEL, GREEN_PIXEL, RED_PIXEL, GREEN_PIXEL],
            "{order}"
        );
    }
}

#[test]
fn a_depth_target_that_is_not_cleared_keeps_the_depths_of_the_last_draw() {
    let device = device();
    let module = load(&device, FLAT);
    let mut options = RenderPipelineOptions::default();
    options.depth_format = Some(TextureFormat::Depth32Float);
    let pipeline = module
        .render_pipeline("vsMain", "fsMain", &options)
        .expect("the pipeline");
    let target = target(&device).expect("a target");
    let depth =
        Texture::new(&device, [SIZE, SIZE], TextureFormat::Depth32Float).expect("a depth target");
    let whole = |z: f32, colour: [f32; 4]| {
        Triangles::new(
            &device,
            &[([[-1.0, -1.0, z], [3.0, -1.0, z], [-1.0, 3.0, z]], colour)],
        )
    };
    let near = whole(0.25, RED);
    pipeline
        .draw()
        .vertex("input.position", &near.positions)
        .vertex("input.colour", &near.colours)
        .target(&target)
        .clear(BLUE)
        .depth(&depth)
        .clear_depth(1.0)
        .submit(near.vertices, 1)
        .expect("the near draw");
    let far = whole(0.5, GREEN);
    pipeline
        .draw()
        .vertex("input.position", &far.positions)
        .vertex("input.colour", &far.colours)
        .target(&target)
        .depth(&depth)
        .submit(far.vertices, 1)
        .expect("the far draw");
    assert_eq!(corners(&target.read().expect("the pixels")), [RED_PIXEL; 4]);
}

#[test]
fn depths_outside_zero_to_one_are_clipped_and_depths_inside_are_drawn() {
    let device = device();
    let module = load(&device, FLAT);
    let pipeline = module
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the pipeline");
    for (z, expected) in [
        (-0.5, BLUE_PIXEL),
        (1.5, BLUE_PIXEL),
        (0.01, RED_PIXEL),
        (0.99, RED_PIXEL),
    ] {
        let triangle = Triangles::new(
            &device,
            &[([[-1.0, -1.0, z], [3.0, -1.0, z], [-1.0, 3.0, z]], RED)],
        );
        let target = target(&device).expect("a target");
        triangle
            .draw(&pipeline, &target, Some(BLUE))
            .expect("the draw");
        assert_eq!(
            corners(&target.read().expect("the pixels")),
            [expected; 4],
            "a depth of {z}"
        );
    }
}

#[test]
fn the_fragment_position_has_pixel_centres_at_one_half_and_rows_from_the_top() {
    let device = device();
    let module = load(
        &device,
        r#"
[shader("vertex")]
float4 vsMain(float2 position : POSITION) : SV_Position
{
    return float4(position, 0.5, 1.0);
}

[shader("fragment")]
float4 fsMain(float4 position : SV_Position) : SV_Target
{
    return float4((position.xy - 0.5) * 16.0 / 255.0, 0.0, 1.0);
}
"#,
    );
    let pipeline = module
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the pipeline");
    let positions = Buffer::from_slice(&device, &[[-1.0_f32, -1.0], [3.0, -1.0], [-1.0, 3.0]])
        .expect("the positions");
    let target = target(&device).expect("a target");
    pipeline
        .draw()
        .vertex("position", &positions)
        .target(&target)
        .submit(3, 1)
        .expect("the draw");
    let pixels = target.read().expect("the pixels");
    for y in 0..SIZE {
        for x in 0..SIZE {
            let expected = [
                u8::try_from(x * 16).expect("a byte"),
                u8::try_from(y * 16).expect("a byte"),
                0,
                255,
            ];
            assert_eq!(pixel_at(&pixels, SIZE, x, y), expected, "pixel ({x}, {y})");
        }
    }
}
