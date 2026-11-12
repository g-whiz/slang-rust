use slango::{
    AddressMode, Buffer, Call, Device, ErrorKind, FilterMode, Kernel, Sampler, SamplerOptions,
    Texture, TextureFormat,
};

use super::texture::{device, wgpu_texture, write_pixels};
use crate::common::kernel;

/// Samples each pixel of `image` at its centre and writes it, scaled back to
/// bytes, in rows from the top.
const TEXELS: &str = r#"
Texture2D<float4> image;
SamplerState pick;
RWStructuredBuffer<uint4> output;
uniform uint2 size;

[shader("compute")]
[numthreads(64, 1, 1)]
void texels(uint3 id : SV_DispatchThreadID)
{
    if (id.x >= size.x * size.y) { return; }
    float2 centre = (float2(id.x % size.x, id.x / size.x) + 0.5) / float2(size);
    output[id.x] = uint4(round(image.SampleLevel(pick, centre, 0) * 255.0));
}
"#;

fn sampler(
    device: &Device,
    filter: FilterMode,
    address_mode: AddressMode,
) -> slango::Result<Sampler> {
    let mut options = SamplerOptions::default();
    options.filter = filter;
    options.address_mode = address_mode;
    Sampler::new(device, &options)
}

fn nearest(device: &Device) -> slango::Result<Sampler> {
    Sampler::new(device, &SamplerOptions::default())
}

fn widen(pixels: &[[u8; 4]]) -> Vec<[u32; 4]> {
    pixels.iter().map(|pixel| pixel.map(u32::from)).collect()
}

/// Runs the kernel `texels` over `image` with a nearest sampler and returns
/// what it wrote.
fn sample_texels(device: &Device, image: &Texture) -> slango::Result<Vec<[u32; 4]>> {
    let texels = kernel(device, TEXELS, "texels");
    let [width, height] = image.size();
    let count = usize::try_from(width * height).unwrap_or(usize::MAX);
    let output = Buffer::<[u32; 4]>::zeroed(device, count)?;
    let pick = nearest(device)?;
    texels
        .call()
        .set("size", [width, height])
        .texture("image", image)
        .sampler("pick", &pick)
        .bind("output", &output)
        .dispatch([width * height, 1, 1])?;
    output.read()
}

/// Six pixels of three by two, each with four distinct bytes, so that a
/// flipped or swapped axis or swapped channels change the result.
const ASYMMETRIC: [[u8; 4]; 6] = [
    [10, 20, 30, 40],
    [50, 60, 70, 80],
    [90, 100, 110, 120],
    [130, 140, 150, 160],
    [170, 180, 190, 200],
    [210, 220, 230, 240],
];

#[test]
fn a_kernel_samples_each_pixel_of_a_texture_from_pixels_at_its_centre() -> slango::Result<()> {
    let device = device();
    let image = Texture::from_pixels(&device, [3, 2], &ASYMMETRIC)?;
    assert_eq!(sample_texels(&device, &image)?, widen(&ASYMMETRIC));
    Ok(())
}

#[test]
fn a_kernel_reads_a_bgra_texture_as_red_green_blue_and_alpha() -> slango::Result<()> {
    let device = device();
    let image = Texture::new(&device, [3, 2], TextureFormat::Bgra8Unorm)?;
    let bytes: Vec<[u8; 4]> = ASYMMETRIC
        .iter()
        .map(|&[red, green, blue, alpha]| [blue, green, red, alpha])
        .collect();
    write_pixels(&device, &image, &bytes);
    assert_eq!(image.read()?, bytes);
    assert_eq!(sample_texels(&device, &image)?, widen(&ASYMMETRIC));
    Ok(())
}

#[test]
fn a_kernel_reads_the_channels_of_an_srgb_texture_in_the_order_of_its_format() -> slango::Result<()>
{
    let device = device();
    let pixels = [
        [255, 0, 0, 255],
        [0, 255, 0, 128],
        [0, 0, 255, 0],
        [255, 255, 255, 255],
    ];
    for format in [TextureFormat::Rgba8UnormSrgb, TextureFormat::Bgra8UnormSrgb] {
        let image = Texture::new(&device, [2, 2], format)?;
        let bytes: Vec<[u8; 4]> = if format == TextureFormat::Bgra8UnormSrgb {
            pixels
                .iter()
                .map(|&[red, green, blue, alpha]| [blue, green, red, alpha])
                .collect()
        } else {
            pixels.to_vec()
        };
        write_pixels(&device, &image, &bytes);
        assert_eq!(
            sample_texels(&device, &image)?,
            widen(&pixels),
            "{format:?}"
        );
    }
    Ok(())
}

/// Returns the linear value of an sRGB-encoded byte, by the sRGB transfer
/// function: `c / 12.92` up to 0.04045, and `((c + 0.055) / 1.055)^2.4`
/// above it, with `c` the byte over 255.
fn srgb_to_linear(byte: u8) -> f32 {
    let encoded = f32::from(byte) / 255.0;
    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

#[test]
fn a_kernel_decodes_a_mid_range_channel_of_an_srgb_texture_and_not_of_a_unorm_one()
-> slango::Result<()> {
    let device = device();
    let centre = kernel(
        &device,
        r#"
Texture2D<float4> image;
SamplerState pick;
RWStructuredBuffer<float4> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void centre() { output[0] = image.SampleLevel(pick, float2(0.5, 0.5), 0); }
"#,
        "centre",
    );
    let pick = nearest(&device)?;
    let output = Buffer::<[f32; 4]>::zeroed(&device, 1)?;
    let decoded = srgb_to_linear(128);
    assert!((decoded - 0.2158).abs() < 0.0001, "{decoded}");
    let raw = 128.0 / 255.0;
    let red_mid = [128, 0, 255, 255];
    let cases = [
        (TextureFormat::Rgba8UnormSrgb, red_mid, decoded),
        (TextureFormat::Rgba8Unorm, red_mid, raw),
        (TextureFormat::Bgra8UnormSrgb, [255, 0, 128, 255], decoded),
        (TextureFormat::Bgra8Unorm, [255, 0, 128, 255], raw),
    ];
    let tolerance = 1.0 / 255.0;
    for (format, bytes, red) in cases {
        let image = Texture::new(&device, [1, 1], format)?;
        write_pixels(&device, &image, &[bytes]);
        centre
            .call()
            .texture("image", &image)
            .sampler("pick", &pick)
            .bind("output", &output)
            .dispatch([1, 1, 1])?;
        let sample = output.read()?[0];
        println!("{format:?} samples a channel of 128 as {}", sample[0]);
        let expected = [red, 0.0, 1.0, 1.0];
        for (channel, expected) in sample.iter().zip(expected) {
            assert!(
                (channel - expected).abs() <= tolerance,
                "{format:?}: {sample:?} is not within {tolerance} of {expected:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn a_linear_sampler_blends_two_pixels_by_their_distance() -> slango::Result<()> {
    let device = device();
    let blend = kernel(
        &device,
        r#"
Texture2D<float4> image;
SamplerState blend;
RWStructuredBuffer<float4> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void between()
{
    output[0] = image.SampleLevel(blend, float2(0.5, 0.5), 0);
    output[1] = image.SampleLevel(blend, float2(0.375, 0.5), 0);
}
"#,
        "between",
    );
    let image = Texture::from_pixels(&device, [2, 1], &[[0, 0, 255, 255], [255, 128, 0, 255]])?;
    let linear = sampler(&device, FilterMode::Linear, AddressMode::ClampToEdge)?;
    let output = Buffer::<[f32; 4]>::zeroed(&device, 2)?;
    blend
        .call()
        .texture("image", &image)
        .sampler("blend", &linear)
        .bind("output", &output)
        .dispatch([1, 1, 1])?;
    let green = 128.0 / 255.0;
    let expected = [[0.5, green / 2.0, 0.5, 1.0], [0.25, green / 4.0, 0.75, 1.0]];
    let tolerance = 0.01;
    let result = output.read()?;
    for (sample, expected) in result.iter().zip(&expected) {
        for (channel, expected) in sample.iter().zip(expected) {
            assert!(
                (channel - expected).abs() <= tolerance,
                "{result:?} is not within {tolerance} of {expected:?}"
            );
        }
    }
    Ok(())
}

const LEFT: [u8; 4] = [255, 0, 0, 255];
const RIGHT: [u8; 4] = [0, 0, 255, 255];

#[test]
fn each_address_mode_reads_its_own_pixels_outside_the_unit_square() -> slango::Result<()> {
    let device = device();
    let edges = kernel(
        &device,
        r#"
Texture2D<float4> image;
SamplerState pick;
RWStructuredBuffer<uint4> output;

static const float across[3] = { 1.25, -0.25, 1.75 };

[shader("compute")]
[numthreads(4, 1, 1)]
void edges(uint3 id : SV_DispatchThreadID)
{
    if (id.x < 3)
    {
        float4 colour = image.SampleLevel(pick, float2(across[id.x], 0.5), 0);
        output[id.x] = uint4(round(colour * 255.0));
    }
}
"#,
        "edges",
    );
    let image = Texture::from_pixels(&device, [2, 1], &[LEFT, RIGHT])?;
    let output = Buffer::<[u32; 4]>::zeroed(&device, 3)?;
    let cases = [
        (AddressMode::ClampToEdge, [RIGHT, LEFT, RIGHT]),
        (AddressMode::Repeat, [LEFT, RIGHT, RIGHT]),
        (AddressMode::MirrorRepeat, [RIGHT, LEFT, LEFT]),
    ];
    let samplers = cases
        .iter()
        .map(|(mode, _)| sampler(&device, FilterMode::Nearest, *mode))
        .collect::<slango::Result<Vec<Sampler>>>()?;
    let mut call = edges.call();
    call.texture("image", &image).bind("output", &output);
    for ((mode, expected), pick) in cases.iter().zip(&samplers) {
        call.sampler("pick", pick).dispatch([3, 1, 1])?;
        assert_eq!(output.read()?, widen(expected), "{mode:?}");
    }
    Ok(())
}

#[test]
fn two_textures_and_two_samplers_reach_their_own_parameters() -> slango::Result<()> {
    let device = device();
    let pairs = kernel(
        &device,
        r#"
Texture2D<float4> second;
SamplerState repeated;
Texture2D<float4> first;
SamplerState clamped;
RWStructuredBuffer<uint4> output;

uint4 bytes(float4 colour) { return uint4(round(colour * 255.0)); }

[shader("compute")]
[numthreads(1, 1, 1)]
void pairs()
{
    float2 at = float2(1.25, 0.5);
    output[0] = bytes(first.SampleLevel(clamped, at, 0));
    output[1] = bytes(second.SampleLevel(repeated, at, 0));
    output[2] = bytes(first.SampleLevel(repeated, at, 0));
    output[3] = bytes(second.SampleLevel(clamped, at, 0));
}
"#,
        "pairs",
    );
    let other_left = [0, 255, 0, 255];
    let other_right = [255, 255, 0, 128];
    let first = Texture::from_pixels(&device, [2, 1], &[LEFT, RIGHT])?;
    let second = Texture::from_pixels(&device, [2, 1], &[other_left, other_right])?;
    let clamped = sampler(&device, FilterMode::Nearest, AddressMode::ClampToEdge)?;
    let repeated = sampler(&device, FilterMode::Nearest, AddressMode::Repeat)?;
    let output = Buffer::<[u32; 4]>::zeroed(&device, 4)?;
    pairs
        .call()
        .bind("output", &output)
        .sampler("clamped", &clamped)
        .texture("first", &first)
        .sampler("repeated", &repeated)
        .texture("second", &second)
        .dispatch([1, 1, 1])?;
    assert_eq!(
        output.read()?,
        widen(&[RIGHT, other_left, LEFT, other_right])
    );
    Ok(())
}

#[test]
fn a_kernel_loads_a_pixel_of_a_texture_without_a_sampler() -> slango::Result<()> {
    let device = device();
    let load = kernel(
        &device,
        r#"
Texture2D<float4> image;
RWStructuredBuffer<uint4> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void load_pixel() { output[0] = uint4(round(image.Load(int3(2, 1, 0)) * 255.0)); }
"#,
        "load_pixel",
    );
    let image = Texture::from_pixels(&device, [3, 2], &ASYMMETRIC)?;
    let output = Buffer::<[u32; 4]>::zeroed(&device, 1)?;
    load.call()
        .texture("image", &image)
        .bind("output", &output)
        .dispatch([1, 1, 1])?;
    assert_eq!(output.read()?, widen(&[ASYMMETRIC[5]]));
    Ok(())
}

const SENTINEL: [[u32; 4]; 6] = [[7; 4]; 6];

struct Fixture {
    device: Device,
    kernel: Kernel,
    image: Texture,
    pick: Sampler,
    output: Buffer<[u32; 4]>,
}

fn fixture() -> slango::Result<Fixture> {
    let device = device();
    let kernel = kernel(&device, TEXELS, "texels");
    let image = Texture::from_pixels(&device, [3, 2], &ASYMMETRIC)?;
    let pick = nearest(&device)?;
    let output = Buffer::from_slice(&device, &SENTINEL)?;
    Ok(Fixture {
        device,
        kernel,
        image,
        pick,
        output,
    })
}

impl Fixture {
    /// Returns a call that gives every parameter of the kernel `texels`
    /// except the one at `skip`.
    fn call_without(&self, skip: &str) -> Call<'_> {
        let mut call = self.kernel.call();
        if skip != "size" {
            call.set("size", [3_u32, 2]);
        }
        if skip != "image" {
            call.texture("image", &self.image);
        }
        if skip != "pick" {
            call.sampler("pick", &self.pick);
        }
        if skip != "output" {
            call.bind("output", &self.output);
        }
        call
    }

    /// Dispatches `call`, asserts that it fails as an invalid argument with
    /// `text` after the operation, and that the output buffer is unchanged.
    fn assert_rejected(&self, call: &Call<'_>, text: &str) -> slango::Result<()> {
        let outcome = call
            .dispatch([6, 1, 1])
            .map_err(|error| (error.kind(), error.to_string()));
        assert_eq!(
            outcome,
            Err((
                ErrorKind::InvalidArgument,
                format!("cannot call a kernel: {text}")
            ))
        );
        assert_eq!(self.output.read()?, SENTINEL);
        Ok(())
    }
}

#[test]
fn the_complete_call_of_the_fixture_runs() -> slango::Result<()> {
    let fixture = fixture()?;
    fixture.call_without("").dispatch([6, 1, 1])?;
    assert_eq!(fixture.output.read()?, widen(&ASYMMETRIC));
    Ok(())
}

#[test]
fn a_missing_texture_or_sampler_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    fixture.assert_rejected(
        &fixture.call_without("image"),
        "no texture is bound to the parameter `image` of the kernel `texels`",
    )?;
    fixture.assert_rejected(
        &fixture.call_without("pick"),
        "no sampler is bound to the parameter `pick` of the kernel `texels`",
    )
}

#[test]
fn an_unknown_path_of_a_texture_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let mut call = fixture.call_without("");
    call.texture("imag", &fixture.image);
    fixture.assert_rejected(
        &call,
        "the kernel `texels` has no parameter at the path `imag`",
    )
}

#[test]
fn a_texture_or_sampler_for_a_parameter_of_another_kind_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let mut call = fixture.call_without("image");
    call.sampler("image", &fixture.pick);
    fixture.assert_rejected(
        &call,
        "the parameter `image` of the kernel `texels` is a texture, and the call binds a sampler to it",
    )?;
    let mut call = fixture.call_without("pick");
    call.texture("pick", &fixture.image);
    fixture.assert_rejected(
        &call,
        "the parameter `pick` of the kernel `texels` is a sampler, and the call binds a texture to it",
    )?;
    let mut call = fixture.call_without("output");
    call.texture("output", &fixture.image);
    fixture.assert_rejected(
        &call,
        "the parameter `output` of the kernel `texels` is a buffer, and the call binds a texture to it",
    )?;
    let mut call = fixture.call_without("image");
    call.bind("image", &fixture.output);
    fixture.assert_rejected(
        &call,
        "the parameter `image` of the kernel `texels` is a texture, and the call binds a buffer to it",
    )
}

#[test]
fn a_texture_or_sampler_of_another_device_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let other = device();
    let image = Texture::from_pixels(&other, [3, 2], &ASYMMETRIC)?;
    let mut call = fixture.call_without("image");
    call.texture("image", &image);
    fixture.assert_rejected(
        &call,
        "the texture bound to the parameter `image` of the kernel `texels` belongs to another device",
    )?;
    let pick = nearest(&other)?;
    let mut call = fixture.call_without("pick");
    call.sampler("pick", &pick);
    fixture.assert_rejected(
        &call,
        "the sampler bound to the parameter `pick` of the kernel `texels` belongs to another device",
    )
}

#[test]
fn a_depth_texture_is_rejected() -> slango::Result<()> {
    let fixture = fixture()?;
    let depth = Texture::new(&fixture.device, [3, 2], TextureFormat::Depth32Float)?;
    let mut call = fixture.call_without("image");
    call.texture("image", &depth);
    fixture.assert_rejected(
        &call,
        "the texture bound to the parameter `image` of the kernel `texels` has the depth format Depth32Float, which a kernel cannot sample",
    )
}

#[test]
fn a_wrapped_view_is_sampled_with_texture_binding_and_rejected_without() -> slango::Result<()> {
    let fixture = fixture()?;
    let target = wgpu_texture(
        &fixture.device,
        [3, 2],
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let view = target.create_view(&wgpu::TextureViewDescriptor::default());
    let unsampleable = slango::wgpu::texture_from_view(&fixture.device, &view)?;
    let mut call = fixture.call_without("image");
    call.texture("image", &unsampleable);
    fixture.assert_rejected(
        &call,
        "the texture bound to the parameter `image` of the kernel `texels` lacks the usage TEXTURE_BINDING, which a kernel needs to sample it",
    )?;
    let sampled = wgpu_texture(
        &fixture.device,
        [3, 2],
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let view = sampled.create_view(&wgpu::TextureViewDescriptor::default());
    let wrapped = slango::wgpu::texture_from_view(&fixture.device, &view)?;
    write_pixels(&fixture.device, &wrapped, &ASYMMETRIC);
    let mut call = fixture.call_without("image");
    call.texture("image", &wrapped).dispatch([6, 1, 1])?;
    assert_eq!(fixture.output.read()?, widen(&ASYMMETRIC));
    Ok(())
}
