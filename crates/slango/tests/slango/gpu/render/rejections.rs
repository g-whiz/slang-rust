use std::error::Error as _;

use slango::{
    AdapterSelection, Buffer, Device, ErrorKind, RenderPipeline, RenderPipelineOptions, Sampler,
    SamplerOptions, Texture, TextureFormat,
};

use crate::common::{
    FLAT, GREEN, GREEN_PIXEL, RED, Triangles, device_with, load, read, render_pipeline, shader,
    texture,
};

const PREFIX: &str = "cannot draw with a render pipeline: ";
const SIZE: u32 = 4;

/// The objects of the rejected draws: a pipeline of [`FLAT`] with and
/// without a depth target, a triangle that covers the target, and a target
/// that a draw of no vertices cleared to green.
struct Setup {
    device: Device,
    other: Device,
    flat: RenderPipeline,
    with_depth: RenderPipeline,
    triangle: Triangles,
    target: Texture,
}

impl Setup {
    fn new() -> Self {
        let device = device_with(AdapterSelection::Default);
        let other = device_with(AdapterSelection::Default);
        let flat = render_pipeline(&device, FLAT, &RenderPipelineOptions::default());
        let mut options = RenderPipelineOptions::default();
        options.depth_format = Some(TextureFormat::Depth32Float);
        let with_depth = render_pipeline(&device, FLAT, &options);
        let triangle = Triangles::new(
            &device,
            &[([[-1.0, -1.0, 0.5], [3.0, -1.0, 0.5], [-1.0, 3.0, 0.5]], RED)],
        );
        let target = texture(&device, [SIZE, SIZE], TextureFormat::Rgba8Unorm);
        let cleared = flat
            .draw()
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &triangle.colours)
            .target(&target)
            .clear(GREEN)
            .submit(0, 1);
        assert!(cleared.is_ok(), "the clear: {cleared:?}");
        Self {
            device,
            other,
            flat,
            with_depth,
            triangle,
            target,
        }
    }

    /// Asserts that a draw failed with `kind` and a message that contains
    /// `text`, and that the target still holds the pixels of the clear.
    fn assert_rejected(&self, result: slango::Result<()>, kind: ErrorKind, text: &str) {
        let error = result.err();
        let message = error.as_ref().map_or_default(ToString::to_string);
        assert_eq!(error.map(|error| error.kind()), Some(kind), "{message}");
        assert!(message.starts_with(PREFIX), "{message}");
        assert!(message.contains(text), "{message} does not contain {text}");
        assert_eq!(read(&self.target), [GREEN_PIXEL; 16], "{message}");
    }

    fn assert_invalid(&self, result: slango::Result<()>, text: &str) {
        self.assert_rejected(result, ErrorKind::InvalidArgument, text);
    }
}

#[test]
fn mistakes_of_vertex_buffers_and_counts_are_rejected_before_any_work() {
    let setup = Setup::new();
    let Setup {
        device,
        other,
        flat,
        triangle,
        target,
        ..
    } = &setup;
    setup.assert_invalid(
        flat.draw()
            .vertex("input.place", &triangle.positions)
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &triangle.colours)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "the render pipeline of `vsMain` and `fsMain` has no vertex attribute at the path `input.place`",
    );
    setup.assert_invalid(
        flat.draw()
            .vertex("input.position", &triangle.positions)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "no vertex buffer is given for the vertex attribute `input.colour` of the render pipeline of `vsMain` and `fsMain`",
    );
    let three = Buffer::from_slice(device, &[[1.0_f32, 0.0, 0.0]; 3]).expect("a buffer");
    setup.assert_invalid(
        flat.draw()
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &three)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "the vertex attribute `input.colour` of the render pipeline of `vsMain` and `fsMain` has the type float4, and its vertex buffer has elements of the type float3",
    );
    let foreign = Buffer::from_slice(other, &[RED; 3]).expect("a buffer");
    setup.assert_invalid(
        flat.draw()
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &foreign)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "the vertex buffer of the attribute `input.colour` of the render pipeline of `vsMain` and `fsMain` belongs to another device",
    );
    let empty = Buffer::<[f32; 4]>::zeroed(device, 0).expect("a buffer");
    setup.assert_invalid(
        flat.draw()
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &empty)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "the vertex buffer of the attribute `input.colour` of the render pipeline of `vsMain` and `fsMain` is empty",
    );
    let four = Buffer::from_slice(device, &[RED; 4]).expect("a buffer");
    setup.assert_invalid(
        flat.draw()
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &four)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "the vertex buffers of the attributes `input.position` and `input.colour` of the render pipeline of `vsMain` and `fsMain`, which step once per vertex, have the different lengths 3 and 4",
    );
    setup.assert_invalid(
        flat.draw()
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &triangle.colours)
            .target(target)
            .clear(RED)
            .submit(4, 1),
        "the draw with the render pipeline of `vsMain` and `fsMain` draws 4 vertices, and the vertex buffers that step once per vertex hold 3",
    );
    let mut options = RenderPipelineOptions::default();
    options.instance_attributes = vec!["input.colour".to_owned()];
    let instanced = load(device, FLAT)
        .render_pipeline("vsMain", "fsMain", &options)
        .expect("the instanced pipeline");
    let one = Buffer::from_slice(device, &[RED]).expect("a buffer");
    setup.assert_invalid(
        instanced
            .draw()
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &one)
            .target(target)
            .clear(RED)
            .submit(3, 2),
        "the draw with the render pipeline of `vsMain` and `fsMain` draws 2 instances, and the vertex buffers that step once per instance hold 1",
    );
}

#[test]
fn mistakes_of_index_buffers_are_rejected_before_any_work() {
    let setup = Setup::new();
    let Setup {
        device,
        other,
        flat,
        triangle,
        target,
        ..
    } = &setup;
    let draw = || {
        let mut draw = flat.draw();
        draw.vertex("input.position", &triangle.positions)
            .vertex("input.colour", &triangle.colours)
            .target(target)
            .clear(RED);
        draw
    };
    let indices = Buffer::from_slice(device, &[0_u32, 1, 2]).expect("the indices");
    setup.assert_invalid(
        draw().submit_indexed(&indices, 4, 1),
        "the draw with the render pipeline of `vsMain` and `fsMain` draws 4 indices, and the index buffer holds 3",
    );
    let foreign = Buffer::from_slice(other, &[0_u32, 1, 2]).expect("the indices");
    setup.assert_invalid(
        draw().submit_indexed(&foreign, 3, 1),
        "the index buffer of the draw with the render pipeline of `vsMain` and `fsMain` belongs to another device",
    );
    let empty = Buffer::<u32>::zeroed(device, 0).expect("the indices");
    setup.assert_invalid(
        draw().submit_indexed(&empty, 0, 1),
        "the index buffer of the draw with the render pipeline of `vsMain` and `fsMain` is empty",
    );
}

#[test]
fn mistakes_of_targets_are_rejected_before_any_work() {
    let setup = Setup::new();
    let Setup {
        device,
        other,
        flat,
        with_depth,
        triangle,
        target,
    } = &setup;
    let draw = |pipeline: &RenderPipeline, target: Option<&Texture>, depth: Option<&Texture>| {
        let mut draw = pipeline.draw();
        draw.vertex("input.position", &triangle.positions)
            .vertex("input.colour", &triangle.colours)
            .clear(RED)
            .clear_depth(1.0);
        if let Some(target) = target {
            draw.target(target);
        }
        if let Some(depth) = depth {
            draw.depth(depth);
        }
        draw.submit(3, 1)
    };
    setup.assert_invalid(
        draw(flat, None, None),
        "the draw with the render pipeline of `vsMain` and `fsMain` has no colour target",
    );
    let bgra = texture(device, [SIZE, SIZE], TextureFormat::Bgra8Unorm);
    setup.assert_invalid(
        draw(flat, Some(&bgra), None),
        "the colour target of the draw with the render pipeline of `vsMain` and `fsMain` has the format Bgra8Unorm, and the pipeline draws into the format Rgba8Unorm",
    );
    let foreign = texture(other, [SIZE, SIZE], TextureFormat::Rgba8Unorm);
    setup.assert_invalid(
        draw(flat, Some(&foreign), None),
        "the colour target of the draw with the render pipeline of `vsMain` and `fsMain` belongs to another device",
    );
    let sampled_only = slango::wgpu::device(device).create_texture(&wgpu::TextureDescriptor {
        label: Some("a texture that cannot be drawn into"),
        size: wgpu::Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let wrapped = slango::wgpu::texture_from_view(
        device,
        &sampled_only.create_view(&wgpu::TextureViewDescriptor::default()),
    )
    .expect("the wrapped texture");
    setup.assert_invalid(
        draw(flat, Some(&wrapped), None),
        "the colour target of the draw with the render pipeline of `vsMain` and `fsMain` lacks the usage RENDER_ATTACHMENT, which a draw needs to draw into it",
    );
    let depth = texture(device, [SIZE, SIZE], TextureFormat::Depth32Float);
    setup.assert_invalid(
        draw(flat, Some(target), Some(&depth)),
        "the render pipeline of `vsMain` and `fsMain` has no depth format, and the draw has a depth target",
    );
    setup.assert_invalid(
        draw(with_depth, Some(target), None),
        "the render pipeline of `vsMain` and `fsMain` has a depth format, and the draw has no depth target",
    );
    let large_depth = texture(device, [2 * SIZE, 2 * SIZE], TextureFormat::Depth32Float);
    setup.assert_invalid(
        draw(with_depth, Some(target), Some(&large_depth)),
        "the depth target of the draw with the render pipeline of `vsMain` and `fsMain` has 8 by 8 pixels, and the colour target has 4 by 4",
    );
    let colour_depth = texture(device, [SIZE, SIZE], TextureFormat::Rgba8Unorm);
    setup.assert_invalid(
        draw(with_depth, Some(target), Some(&colour_depth)),
        "the depth target of the draw with the render pipeline of `vsMain` and `fsMain` has the format Rgba8Unorm, and the pipeline draws into the format Depth32Float",
    );
    let foreign_depth = texture(other, [SIZE, SIZE], TextureFormat::Depth32Float);
    setup.assert_invalid(
        draw(with_depth, Some(target), Some(&foreign_depth)),
        "the depth target of the draw with the render pipeline of `vsMain` and `fsMain` belongs to another device",
    );
}

#[test]
fn mistakes_of_parameters_are_rejected_as_for_a_call() {
    let setup = Setup::new();
    let Setup {
        device,
        other,
        flat,
        triangle,
        target,
        ..
    } = &setup;
    setup.assert_invalid(
        flat.draw()
            .set("tint", 1.0_f32)
            .vertex("input.position", &triangle.positions)
            .vertex("input.colour", &triangle.colours)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "the render pipeline of `vsMain` and `fsMain` has no parameter at the path `tint`",
    );
    let quad = device
        .load_module(shader("quad.slang"))
        .expect("the module")
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the pipeline");
    let positions = Buffer::from_slice(device, &[[0.0_f32, 0.0]; 3]).expect("the positions");
    let image = Texture::new(device, [1, 1], TextureFormat::Rgba8Unorm).expect("an image");
    let sampler = Sampler::new(device, &SamplerOptions::default()).expect("a sampler");
    setup.assert_invalid(
        quad.draw()
            .texture("image", &image)
            .sampler("imageSampler", &sampler)
            .vertex("input.position", &positions)
            .vertex("input.uv", &positions)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "the parameter `transform` of the render pipeline of `vsMain` and `fsMain` is not set",
    );
    let foreign = Texture::new(other, [1, 1], TextureFormat::Rgba8Unorm).expect("an image");
    setup.assert_invalid(
        quad.draw()
            .set("transform", [[0.0_f32; 4]; 4])
            .texture("image", &foreign)
            .sampler("imageSampler", &sampler)
            .vertex("input.position", &positions)
            .vertex("input.uv", &positions)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "the texture bound to the parameter `image` of the render pipeline of `vsMain` and `fsMain` belongs to another device",
    );
    setup.assert_invalid(
        quad.draw()
            .set("transform", [0.0_f32; 4])
            .texture("image", &image)
            .sampler("imageSampler", &sampler)
            .vertex("input.position", &positions)
            .vertex("input.uv", &positions)
            .target(target)
            .clear(RED)
            .submit(3, 1),
        "the render pipeline of `vsMain` and `fsMain` cannot take the value of the parameter `transform`",
    );
}

#[test]
fn a_texture_both_sampled_and_the_target_of_one_draw_is_rejected_by_wgpu_as_a_backend_error() {
    let setup = Setup::new();
    let Setup { device, target, .. } = &setup;
    let quad = device
        .load_module(shader("quad.slang"))
        .expect("the module")
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
        .expect("the pipeline");
    let positions = Buffer::from_slice(device, &[[-1.0_f32, -1.0], [3.0, -1.0], [-1.0, 3.0]])
        .expect("the positions");
    let sampler = Sampler::new(device, &SamplerOptions::default()).expect("a sampler");
    let result = quad
        .draw()
        .set(
            "transform",
            [
                [1.0_f32, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        )
        .texture("image", target)
        .sampler("imageSampler", &sampler)
        .vertex("input.position", &positions)
        .vertex("input.uv", &positions)
        .target(target)
        .clear(RED)
        .submit(3, 1);
    let source = result
        .as_ref()
        .err()
        .and_then(|error| error.source())
        .map(ToString::to_string);
    println!("slango gpu test: a texture both sampled and a target: {source:?}");
    setup.assert_rejected(
        result,
        ErrorKind::Backend,
        "wgpu rejected the draw of the render pipeline of `vsMain` and `fsMain`",
    );
}

#[test]
fn a_depth_clear_value_outside_zero_to_one_is_rejected_before_any_work() {
    let setup = Setup::new();
    let depth = texture(&setup.device, [SIZE, SIZE], TextureFormat::Depth32Float);
    setup.assert_invalid(
        setup
            .with_depth
            .draw()
            .vertex("input.position", &setup.triangle.positions)
            .vertex("input.colour", &setup.triangle.colours)
            .target(&setup.target)
            .clear(RED)
            .depth(&depth)
            .clear_depth(2.0)
            .submit(3, 1),
        "the draw with the render pipeline of `vsMain` and `fsMain` clears the depth target to 2.0, and a depth is from 0.0 to 1.0",
    );
}
