use slango::{
    AdapterSelection, Buffer, Device, RenderPipeline, RenderPipelineOptions, Texture,
    TextureFormat, VertexElement,
};

use crate::common::{
    BLUE, BLUE_PIXEL, FLAT, GREEN_PIXEL, RED, RED_PIXEL, buffer, device_with, pixel_at, read,
    render_pipeline, texture,
};

const SIZE: u32 = 16;
const BLACK_PIXEL: [u8; 4] = [0, 0, 0, 255];

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

fn pipeline(device: &Device, source: &str, options: &RenderPipelineOptions) -> RenderPipeline {
    render_pipeline(device, source, options)
}

fn target(device: &Device) -> Texture {
    texture(device, [SIZE, SIZE], TextureFormat::Rgba8Unorm)
}

/// The positions of two triangles, one in the left half of the target and
/// one in the right half.
const HALVES: [[f32; 2]; 6] = [
    [-1.0, -1.0],
    [0.0, -1.0],
    [-1.0, 1.0],
    [1.0, -1.0],
    [0.0, -1.0],
    [1.0, 1.0],
];

/// Returns a buffer of six elements, `left` for the three vertices of the
/// left triangle of [`HALVES`] and `right` for the three of the right one.
fn halves<T: VertexElement>(device: &Device, left: T, right: T) -> Buffer<T> {
    buffer(device, &[left, left, left, right, right, right])
}

/// Returns the pixels inside the left and the right triangle of [`HALVES`].
fn left_and_right(target: &Texture) -> [[u8; 4]; 2] {
    let pixels = read(target);
    [
        pixel_at(&pixels, SIZE, 2, 12),
        pixel_at(&pixels, SIZE, 13, 12),
    ]
}

#[test]
fn float_attributes_of_one_to_four_components_arrive_at_their_locations() {
    let device = device();
    let pipeline = pipeline(
        &device,
        r#"
struct VIn { float2 position : POSITION; float s : S; float3 t : T; float4 u : U; };
struct VOut { float4 position : SV_Position; float4 colour : COLOR; };

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = float4(input.position, 0.5, 1.0);
    float3 weights = float3(1.0, 8.0, 64.0);
    output.colour = float4(input.s, dot(input.t, weights) / 255.0, dot(input.u.xyz, weights) / 255.0, input.u.w);
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target { return input.colour; }
"#,
        &RenderPipelineOptions::default(),
    );
    let positions = Buffer::from_slice(&device, &HALVES).expect("the positions");
    let s = halves(&device, 50.0_f32 / 255.0, 200.0 / 255.0);
    let t = halves(&device, [1.0_f32, 2.0, 0.0], [0.0, 1.0, 3.0]);
    let u = halves(&device, [3.0_f32, 1.0, 2.0, 1.0], [1.0, 0.0, 1.0, 0.6]);
    let target = target(&device);
    pipeline
        .draw()
        .vertex("input.position", &positions)
        .vertex("input.s", &s)
        .vertex("input.t", &t)
        .vertex("input.u", &u)
        .target(&target)
        .submit(6, 1)
        .expect("the draw");
    assert_eq!(
        left_and_right(&target),
        [[50, 17, 139, 255], [200, 200, 65, 153]]
    );
}

#[test]
fn int_attributes_of_one_to_four_components_arrive_unchanged_through_nointerpolation() {
    let device = device();
    let pipeline = pipeline(
        &device,
        r#"
struct VIn { float2 position : POSITION; int a : A; int2 b : B; int3 c : C; int4 d : D; };
struct VOut
{
    float4 position : SV_Position;
    nointerpolation int a : A;
    nointerpolation int2 b : B;
    nointerpolation int3 c : C;
    nointerpolation int4 d : D;
};

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = float4(input.position, 0.5, 1.0);
    output.a = input.a;
    output.b = input.b;
    output.c = input.c;
    output.d = input.d;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return float4(
        float(input.a + 2000000000),
        float(input.b.x * 16 + input.b.y + 100),
        float(input.c.x + 8 * input.c.y + 64 * input.c.z + 100),
        float(input.d.x + 4 * input.d.y + 16 * input.d.z + 64 * input.d.w)) / 255.0;
}
"#,
        &RenderPipelineOptions::default(),
    );
    let positions = Buffer::from_slice(&device, &HALVES).expect("the positions");
    let a = halves(&device, -1_999_999_950_i32, -1_999_999_800);
    let b = halves(&device, [-3_i32, 7], [2, -5]);
    let c = halves(&device, [1_i32, 2, -1], [-2, 0, 1]);
    let d = halves(&device, [1_i32, 2, 3, 0], [0, 1, 2, 3]);
    let target = target(&device);
    pipeline
        .draw()
        .vertex("input.position", &positions)
        .vertex("input.a", &a)
        .vertex("input.b", &b)
        .vertex("input.c", &c)
        .vertex("input.d", &d)
        .target(&target)
        .submit(6, 1)
        .expect("the draw");
    assert_eq!(
        left_and_right(&target),
        [[50, 59, 53, 57], [200, 127, 162, 228]]
    );
}

#[test]
fn uint_attributes_of_one_to_four_components_arrive_unchanged_through_nointerpolation() {
    let device = device();
    let pipeline = pipeline(
        &device,
        r#"
struct VIn { float2 position : POSITION; uint a : A; uint2 b : B; uint3 c : C; uint4 d : D; };
struct VOut
{
    float4 position : SV_Position;
    nointerpolation uint a : A;
    nointerpolation uint2 b : B;
    nointerpolation uint3 c : C;
    nointerpolation uint4 d : D;
};

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = float4(input.position, 0.5, 1.0);
    output.a = input.a;
    output.b = input.b;
    output.c = input.c;
    output.d = input.d;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target
{
    return float4(
        float(input.a - 4294967000u),
        float(input.b.x * 16u + input.b.y),
        float(input.c.x + 8u * input.c.y + 64u * input.c.z),
        float(input.d.x + 4u * input.d.y + 16u * input.d.z + 64u * input.d.w)) / 255.0;
}
"#,
        &RenderPipelineOptions::default(),
    );
    let positions = Buffer::from_slice(&device, &HALVES).expect("the positions");
    let a = halves(&device, 4_294_967_050_u32, 4_294_967_245);
    let b = halves(&device, [3_u32, 7], [10, 1]);
    let c = halves(&device, [1_u32, 2, 0], [3, 0, 2]);
    let d = halves(&device, [1_u32, 2, 3, 0], [3, 3, 3, 3]);
    let target = target(&device);
    pipeline
        .draw()
        .vertex("input.position", &positions)
        .vertex("input.a", &a)
        .vertex("input.b", &b)
        .vertex("input.c", &c)
        .vertex("input.d", &d)
        .target(&target)
        .submit(6, 1)
        .expect("the draw");
    assert_eq!(
        left_and_right(&target),
        [[50, 55, 17, 57], [245, 161, 131, 255]]
    );
}

#[test]
fn an_indexed_draw_of_four_vertices_and_six_indices_fills_a_quad() {
    let device = device();
    let pipeline = pipeline(&device, FLAT, &RenderPipelineOptions::default());
    let positions = Buffer::from_slice(
        &device,
        &[
            [-0.5_f32, -0.5, 0.5],
            [0.5, -0.5, 0.5],
            [0.5, 0.5, 0.5],
            [-0.5, 0.5, 0.5],
        ],
    )
    .expect("the positions");
    let colours = Buffer::from_slice(&device, &[RED; 4]).expect("the colours");
    let indices = Buffer::from_slice(&device, &[0_u32, 1, 2, 0, 2, 3]).expect("the indices");
    for (count, upper_left) in [(6, RED_PIXEL), (3, BLUE_PIXEL)] {
        let target = target(&device);
        pipeline
            .draw()
            .vertex("input.position", &positions)
            .vertex("input.colour", &colours)
            .target(&target)
            .clear(BLUE)
            .submit_indexed(&indices, count, 1)
            .expect("the draw");
        let pixels = target.read().expect("the pixels");
        assert_eq!(
            [
                pixel_at(&pixels, SIZE, 5, 4),
                pixel_at(&pixels, SIZE, 11, 5),
                pixel_at(&pixels, SIZE, 4, 10),
                pixel_at(&pixels, SIZE, 10, 11),
                pixel_at(&pixels, SIZE, 2, 2),
                pixel_at(&pixels, SIZE, 13, 13),
            ],
            [
                upper_left, RED_PIXEL, upper_left, RED_PIXEL, BLUE_PIXEL, BLUE_PIXEL
            ],
            "{count} indices"
        );
    }
}

#[test]
fn an_instanced_draw_places_each_instance_by_its_own_attributes() {
    let device = device();
    let mut options = RenderPipelineOptions::default();
    options.instance_attributes = vec!["input.offset".to_owned(), "input.colour".to_owned()];
    let pipeline = pipeline(
        &device,
        r#"
struct VIn { float2 position : POSITION; float2 offset : OFFSET; float4 colour : COLOR; };
struct VOut { float4 position : SV_Position; float4 colour : COLOR; };

[shader("vertex")]
VOut vsMain(VIn input)
{
    VOut output;
    output.position = float4(input.position + input.offset, 0.5, 1.0);
    output.colour = input.colour;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input) : SV_Target { return input.colour; }
"#,
        &options,
    );
    let positions =
        Buffer::from_slice(&device, &[[-0.25_f32, -0.25], [0.25, -0.25], [-0.25, 0.25]])
            .expect("the positions");
    let offsets = Buffer::from_slice(&device, &[[-0.5_f32, 0.5], [0.5, 0.5], [-0.5, -0.5]])
        .expect("the offsets");
    let colours =
        Buffer::from_slice(&device, &[RED, [0.0, 1.0, 0.0, 1.0], BLUE]).expect("the colours");
    for (instances, third) in [(3, BLUE_PIXEL), (2, BLACK_PIXEL)] {
        let target = target(&device);
        pipeline
            .draw()
            .vertex("input.position", &positions)
            .vertex("input.offset", &offsets)
            .vertex("input.colour", &colours)
            .target(&target)
            .clear([0.0, 0.0, 0.0, 1.0])
            .submit(3, instances)
            .expect("the draw");
        let pixels = target.read().expect("the pixels");
        assert_eq!(
            [
                pixel_at(&pixels, SIZE, 3, 4),
                pixel_at(&pixels, SIZE, 11, 4),
                pixel_at(&pixels, SIZE, 3, 12),
                pixel_at(&pixels, SIZE, 11, 12),
                pixel_at(&pixels, SIZE, 8, 8),
            ],
            [RED_PIXEL, GREEN_PIXEL, third, BLACK_PIXEL, BLACK_PIXEL],
            "{instances} instances"
        );
    }
}
