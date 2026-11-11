use slango::{
    AdapterSelection, Blend, Buffer, Device, DeviceOptions, FilterMode, RenderPipeline,
    RenderPipelineOptions, Sampler, SamplerOptions, Texture, TextureFormat,
};

use crate::common::{
    BLUE, BLUE_PIXEL, FLAT, GREEN, GREEN_PIXEL, RED, RED_PIXEL, Triangles, assert_near, buffer,
    device_with, load, pixel_at, quad_pipeline, render_pipeline, sampler, shader,
};

/// The program of the usage illustration of Section 5.4 of
/// `docs/design/slango.md`, as written there, with the shader file of the
/// tests.
fn illustration() -> slango::Result<()> {
    let device = Device::new(&DeviceOptions::default())?;
    let module = device.load_module(shader("quad.slang"))?;
    let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
    let corners = [[-1.0_f32, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    let positions = Buffer::from_slice(&device, &corners)?;
    let uvs = Buffer::from_slice(
        &device,
        &[[0.0_f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
    )?;
    let indices = Buffer::from_slice(&device, &[0_u32, 1, 2, 0, 2, 3])?;
    let red_and_blue = [
        [255, 0, 0, 255],
        [0, 0, 255, 255],
        [0, 0, 255, 255],
        [255, 0, 0, 255],
    ];
    let image = Texture::from_pixels(&device, [2, 2], &red_and_blue)?;
    let sampler = Sampler::new(&device, &SamplerOptions::default())?;
    let target = Texture::new(&device, [64, 64], TextureFormat::Rgba8Unorm)?;
    let identity = [
        [1.0_f32, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    pipeline
        .draw()
        .set("transform", identity)
        .texture("image", &image)
        .sampler("imageSampler", &sampler)
        .vertex("input.position", &positions)
        .vertex("input.uv", &uvs)
        .target(&target)
        .clear([0.0, 0.0, 0.0, 1.0])
        .submit_indexed(&indices, 6, 1)?;
    let pixels: Vec<[u8; 4]> = target.read()?;
    assert_eq!(pixels[0], [255, 0, 0, 255]);
    assert_eq!(
        [
            pixel_at(&pixels, 64, 16, 16),
            pixel_at(&pixels, 64, 48, 16),
            pixel_at(&pixels, 64, 16, 48),
            pixel_at(&pixels, 64, 48, 48),
            pixel_at(&pixels, 64, 63, 63),
        ],
        [RED_PIXEL, BLUE_PIXEL, BLUE_PIXEL, RED_PIXEL, RED_PIXEL]
    );
    Ok(())
}

#[test]
fn the_usage_illustration_draws_the_texture_with_its_top_left_pixel_at_the_top_left() {
    illustration().expect("the illustration runs");
}

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

const IDENTITY: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

/// The vertex buffers of a quad that covers the whole target, with the
/// texture coordinates (0, 0) at its upper-left corner.
struct Quad {
    positions: Buffer<[f32; 2]>,
    uvs: Buffer<[f32; 2]>,
    indices: Buffer<u32>,
}

impl Quad {
    fn new(device: &Device) -> Self {
        Self {
            positions: buffer(
                device,
                &[[-1.0_f32, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]],
            ),
            uvs: buffer(
                device,
                &[[0.0_f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]],
            ),
            indices: buffer(device, &[0_u32, 1, 2, 0, 2, 3]),
        }
    }

    fn draw(
        &self,
        pipeline: &RenderPipeline,
        image: &Texture,
        sampler: &Sampler,
        target: &Texture,
    ) -> slango::Result<()> {
        pipeline
            .draw()
            .set("transform", IDENTITY)
            .texture("image", image)
            .sampler("imageSampler", sampler)
            .vertex("input.position", &self.positions)
            .vertex("input.uv", &self.uvs)
            .target(target)
            .submit_indexed(&self.indices, 6, 1)
    }
}

#[test]
fn a_texture_is_sampled_with_nearest_and_with_linear_filtering() {
    let device = device();
    let pipeline = quad_pipeline(&device);
    let quad = Quad::new(&device);
    let image = Texture::from_pixels(&device, [2, 1], &[[0, 0, 0, 255], [255, 255, 255, 255]])
        .expect("an image");
    let grey = |value: u8| [value, value, value, 255];
    for (filter, expected, tolerance) in [
        (
            FilterMode::Nearest,
            [(0, 0), (4, 0), (7, 0), (8, 255), (11, 255), (15, 255)],
            0,
        ),
        (
            FilterMode::Linear,
            [(0, 0), (4, 16), (7, 112), (8, 143), (11, 239), (15, 255)],
            3,
        ),
    ] {
        let target = Texture::new(&device, [16, 1], TextureFormat::Rgba8Unorm).expect("a target");
        quad.draw(&pipeline, &image, &sampler(&device, filter), &target)
            .expect("the draw");
        let pixels = target.read().expect("the pixels");
        for (x, value) in expected {
            assert_near(
                pixel_at(&pixels, 16, x, 0),
                grey(value),
                tolerance,
                &format!("{filter:?} at column {x}"),
            );
        }
    }
}

#[test]
fn a_texture_drawn_into_by_one_draw_is_sampled_by_the_next() {
    let device = device();
    let pipeline = quad_pipeline(&device);
    let quad = Quad::new(&device);
    let nearest = sampler(&device, FilterMode::Nearest);
    let image = Texture::from_pixels(
        &device,
        [2, 2],
        &[
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 0, 255, 255],
            [255, 0, 0, 255],
        ],
    )
    .expect("an image");
    let intermediate = Texture::new(&device, [8, 8], TextureFormat::Rgba8Unorm).expect("a texture");
    quad.draw(&pipeline, &image, &nearest, &intermediate)
        .expect("the first draw");
    let target = Texture::new(&device, [16, 16], TextureFormat::Rgba8Unorm).expect("a target");
    quad.draw(&pipeline, &intermediate, &nearest, &target)
        .expect("the second draw");
    let pixels = target.read().expect("the pixels");
    assert_eq!(
        [
            pixel_at(&pixels, 16, 2, 2),
            pixel_at(&pixels, 16, 13, 2),
            pixel_at(&pixels, 16, 2, 13),
            pixel_at(&pixels, 16, 13, 13),
        ],
        [RED_PIXEL, BLUE_PIXEL, BLUE_PIXEL, RED_PIXEL]
    );
}

fn flat_pipeline(device: &Device, options: &RenderPipelineOptions) -> RenderPipeline {
    render_pipeline(device, FLAT, options)
}

const WHOLE: [[f32; 3]; 3] = [[-1.0, -1.0, 0.5], [3.0, -1.0, 0.5], [-1.0, 3.0, 0.5]];

#[test]
fn alpha_blending_mixes_over_the_target_and_replace_does_not() {
    let device = device();
    let half_red = Triangles::new(&device, &[(WHOLE, [1.0, 0.0, 0.0, 0.5])]);
    for (blend, expected) in [
        (Blend::Alpha, [128, 0, 128, 255]),
        (Blend::Replace, [255, 0, 0, 128]),
    ] {
        let mut options = RenderPipelineOptions::default();
        options.blend = blend;
        let pipeline = flat_pipeline(&device, &options);
        let target = Texture::new(&device, [4, 4], TextureFormat::Rgba8Unorm).expect("a target");
        half_red
            .draw(&pipeline, &target, Some(BLUE))
            .expect("the draw");
        assert_near(
            pixel_at(&target.read().expect("the pixels"), 4, 1, 1),
            expected,
            1,
            &format!("{blend:?}"),
        );
    }
}

#[test]
fn a_draw_without_a_clear_keeps_the_pixels_of_the_previous_draw() {
    let device = device();
    let pipeline = flat_pipeline(&device, &RenderPipelineOptions::default());
    let target = Texture::new(&device, [16, 16], TextureFormat::Rgba8Unorm).expect("a target");
    Triangles::new(
        &device,
        &[([[-1.0, -1.0, 0.5], [0.0, -1.0, 0.5], [-1.0, 1.0, 0.5]], RED)],
    )
    .draw(&pipeline, &target, Some(BLUE))
    .expect("the first draw");
    Triangles::new(
        &device,
        &[([[1.0, -1.0, 0.5], [0.0, -1.0, 0.5], [1.0, 1.0, 0.5]], GREEN)],
    )
    .draw(&pipeline, &target, None)
    .expect("the second draw");
    let pixels = target.read().expect("the pixels");
    assert_eq!(
        [
            pixel_at(&pixels, 16, 2, 12),
            pixel_at(&pixels, 16, 13, 12),
            pixel_at(&pixels, 16, 8, 1),
        ],
        [RED_PIXEL, GREEN_PIXEL, BLUE_PIXEL]
    );
}

#[test]
fn a_draw_of_zero_vertices_or_instances_clears_the_target_and_draws_nothing() {
    let device = device();
    let pipeline = flat_pipeline(&device, &RenderPipelineOptions::default());
    let triangle = Triangles::new(&device, &[(WHOLE, RED)]);
    for (vertices, instances) in [(0, 1), (3, 0)] {
        let target = Texture::new(&device, [4, 4], TextureFormat::Rgba8Unorm).expect("a target");
        pipeline
            .draw()
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &triangle.colours)
            .target(&target)
            .clear(GREEN)
            .submit(vertices, instances)
            .expect("the draw");
        assert_eq!(target.read().expect("the pixels"), [GREEN_PIXEL; 16]);
    }
}

/// The colour of a draw into each format, with the bytes that
/// [`Texture::read`] returns for it in the byte order of the format and the
/// tolerance of each channel.
///
/// A channel of 0.2 is 51/255 in a linear format. A linear channel of 0.5 is
/// 0.7354 in the sRGB encoding, which is 187.5/255, so a GPU may store 187
/// or 188.
const FORMATS: [(TextureFormat, [f32; 4], [u8; 4], u8); 4] = [
    (
        TextureFormat::Rgba8Unorm,
        [1.0, 0.2, 0.0, 1.0],
        [255, 51, 0, 255],
        0,
    ),
    (
        TextureFormat::Bgra8Unorm,
        [1.0, 0.2, 0.0, 1.0],
        [0, 51, 255, 255],
        0,
    ),
    (
        TextureFormat::Rgba8UnormSrgb,
        [1.0, 0.5, 0.0, 1.0],
        [255, 188, 0, 255],
        1,
    ),
    (
        TextureFormat::Bgra8UnormSrgb,
        [1.0, 0.5, 0.0, 1.0],
        [0, 188, 255, 255],
        1,
    ),
];

#[test]
fn each_colour_format_is_a_target_and_reads_back_in_its_byte_order_and_encoding() {
    let device = device();
    let module = load(&device, FLAT);
    for (format, colour, expected, tolerance) in FORMATS {
        let mut options = RenderPipelineOptions::default();
        options.colour_format = format;
        let pipeline = module
            .render_pipeline("vsMain", "fsMain", &options)
            .expect("the pipeline");
        let target = Texture::new(&device, [4, 4], format).expect("a target");
        Triangles::new(&device, &[(WHOLE, colour)])
            .draw(&pipeline, &target, None)
            .expect("the draw");
        for pixel in target.read().expect("the pixels") {
            assert_near(pixel, expected, tolerance, &format!("{format:?}"));
        }
    }
}

#[test]
fn a_texture_wrapped_from_a_wgpu_view_is_a_target() {
    let device = device();
    let mut options = RenderPipelineOptions::default();
    options.colour_format = TextureFormat::Bgra8Unorm;
    let pipeline = flat_pipeline(&device, &options);
    let texture = slango::wgpu::device(&device).create_texture(&wgpu::TextureDescriptor {
        label: Some("a texture like that of a frame of a window"),
        size: wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Bgra8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let target = slango::wgpu::texture_from_view(&device, &view).expect("the wrapped texture");
    Triangles::new(&device, &[(WHOLE, [1.0, 0.2, 0.0, 1.0])])
        .draw(&pipeline, &target, Some(BLUE))
        .expect("the draw");
    assert_eq!(target.read().expect("the pixels"), [[0, 51, 255, 255]; 16]);
}

#[test]
fn a_later_set_or_texture_for_a_path_replaces_the_earlier_one() {
    let device = device();
    let pipeline = quad_pipeline(&device);
    let quad = Quad::new(&device);
    let nearest = sampler(&device, FilterMode::Nearest);
    let depth =
        Texture::new(&device, [1, 1], TextureFormat::Depth32Float).expect("a depth texture");
    let green = Texture::from_pixels(&device, [1, 1], &[[0, 255, 0, 255]]).expect("an image");
    let red_and_blue = Texture::from_pixels(&device, [2, 1], &[[255, 0, 0, 255], [0, 0, 255, 255]])
        .expect("an image");
    let mirror = [
        [-1.0_f32, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ];
    let target = Texture::new(&device, [16, 16], TextureFormat::Rgba8Unorm).expect("a target");
    pipeline
        .draw()
        .set("transform", 1.0_f32)
        .texture("image", &depth)
        .set("transform", mirror)
        .texture("image", &green)
        .sampler("imageSampler", &nearest)
        .vertex("input.position", &quad.positions)
        .vertex("input.uv", &quad.uvs)
        .target(&target)
        .set("transform", IDENTITY)
        .texture("image", &red_and_blue)
        .submit_indexed(&quad.indices, 6, 1)
        .expect("the draw");
    let pixels = target.read().expect("the pixels");
    assert_eq!(
        [pixel_at(&pixels, 16, 2, 8), pixel_at(&pixels, 16, 13, 8)],
        [RED_PIXEL, BLUE_PIXEL]
    );
}

#[test]
fn a_later_vertex_buffer_for_an_attribute_replaces_the_earlier_one() {
    let device = device();
    let pipeline = flat_pipeline(&device, &RenderPipelineOptions::default());
    let triangle = Triangles::new(&device, &[(WHOLE, RED)]);
    let wrong_type = buffer(&device, &[[0.0_f32, 0.0, 1.0]; 3]);
    let green = buffer(&device, &[GREEN; 3]);
    let target = Texture::new(&device, [4, 4], TextureFormat::Rgba8Unorm).expect("a target");
    pipeline
        .draw()
        .vertex("input.position", &triangle.positions)
        .vertex("input.colour", &wrong_type)
        .vertex("input.colour", &triangle.colours)
        .target(&target)
        .clear(BLUE)
        .submit(triangle.vertices, 1)
        .expect("the draw with the corrected colours");
    assert_eq!(target.read().expect("the pixels"), [RED_PIXEL; 16]);
    pipeline
        .draw()
        .vertex("input.position", &triangle.positions)
        .vertex("input.colour", &triangle.colours)
        .vertex("input.colour", &green)
        .target(&target)
        .clear(BLUE)
        .submit(triangle.vertices, 1)
        .expect("the draw with the later colours");
    assert_eq!(target.read().expect("the pixels"), [GREEN_PIXEL; 16]);
}
