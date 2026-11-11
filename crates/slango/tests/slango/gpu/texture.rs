use slango::{
    AdapterSelection, AddressMode, Device, ErrorKind, FilterMode, Sampler, SamplerOptions, Texture,
    TextureFormat,
};

use crate::common::device_with;

const COLOUR_FORMATS: [TextureFormat; 4] = [
    TextureFormat::Rgba8Unorm,
    TextureFormat::Rgba8UnormSrgb,
    TextureFormat::Bgra8Unorm,
    TextureFormat::Bgra8UnormSrgb,
];

pub(crate) fn device() -> Device {
    device_with(AdapterSelection::Default)
}

/// Returns `count` distinct pixels, each with four distinct bytes.
pub(crate) fn distinct_pixels(count: usize) -> Vec<[u8; 4]> {
    (0..count)
        .map(|index| {
            let base = u8::try_from(index * 4 % 252).unwrap_or_default();
            [base, base + 1, base + 2, base + 3]
        })
        .collect()
}

/// Writes pixels, given as the bytes of the texture's format in rows from
/// the top, into a texture through wgpu.
pub(crate) fn write_pixels(device: &Device, texture: &Texture, pixels: &[[u8; 4]]) {
    let [width, height] = texture.size();
    let bytes: Vec<u8> = pixels.iter().flatten().copied().collect();
    slango::wgpu::queue(device).write_texture(
        slango::wgpu::texture(texture).as_image_copy(),
        &bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(width * 4),
            rows_per_image: Some(height),
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
}

#[test]
fn a_texture_from_pixels_reads_back_its_pixels() -> slango::Result<()> {
    let device = device();
    for size in [[1, 1], [3, 2], [5, 3], [64, 2], [65, 3]] {
        let count = usize::try_from(size[0] * size[1]).expect("a count");
        let pixels = distinct_pixels(count);
        let texture = Texture::from_pixels(&device, size, &pixels)?;
        assert_eq!(texture.size(), size);
        assert_eq!(texture.format(), TextureFormat::Rgba8Unorm);
        assert_eq!(texture.read()?, pixels, "{size:?}");
        assert_eq!(texture.clone().read()?, pixels, "{size:?}");
    }
    Ok(())
}

#[test]
fn a_texture_of_each_colour_format_reads_back_the_bytes_written_into_it() -> slango::Result<()> {
    let device = device();
    for format in COLOUR_FORMATS {
        for size in [[3, 2], [5, 4]] {
            let texture = Texture::new(&device, size, format)?;
            assert_eq!(texture.format(), format);
            let count = usize::try_from(size[0] * size[1]).expect("a count");
            let pixels = distinct_pixels(count);
            write_pixels(&device, &texture, &pixels);
            assert_eq!(texture.read()?, pixels, "{format:?} {size:?}");
        }
    }
    Ok(())
}

#[test]
fn an_empty_texture_of_each_colour_format_reads_back_zeros() -> slango::Result<()> {
    let device = device();
    for format in COLOUR_FORMATS {
        let texture = Texture::new(&device, [5, 3], format)?;
        assert_eq!(texture.size(), [5, 3]);
        assert_eq!(texture.read()?, [[0; 4]; 15], "{format:?}");
    }
    Ok(())
}

#[test]
fn a_depth_texture_is_created_and_cannot_be_read() -> slango::Result<()> {
    let device = device();
    let depth = Texture::new(&device, [4, 4], TextureFormat::Depth32Float)?;
    assert_eq!(depth.format(), TextureFormat::Depth32Float);
    assert_eq!(
        slango::wgpu::texture(&depth).usage(),
        wgpu::TextureUsages::RENDER_ATTACHMENT
    );
    let error = depth.read().expect_err("a depth texture is not read");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot read a texture: the texture has the depth format Depth32Float, whose depths cannot be read as pixels"
    );
    Ok(())
}

fn assert_invalid(result: slango::Result<Texture>, text: &str) {
    let outcome = result
        .map(|texture| texture.size())
        .map_err(|error| (error.kind(), error.to_string()));
    assert_eq!(outcome, Err((ErrorKind::InvalidArgument, text.to_owned())));
}

#[test]
fn a_size_without_pixels_is_an_invalid_argument() {
    let device = device();
    assert_invalid(
        Texture::from_pixels(&device, [0, 1], &[]),
        "cannot create a texture: a texture needs a width and a height of at least 1 pixel, and the size is 0 by 1",
    );
    assert_invalid(
        Texture::new(&device, [2, 0], TextureFormat::Rgba8Unorm),
        "cannot create a texture: a texture needs a width and a height of at least 1 pixel, and the size is 2 by 0",
    );
}

#[test]
fn a_size_over_the_limit_of_the_device_is_an_invalid_argument() {
    let device = device();
    let limit = slango::wgpu::device(&device)
        .limits()
        .max_texture_dimension_2d;
    let over = limit + 1;
    assert_invalid(
        Texture::new(&device, [over, 1], TextureFormat::Bgra8Unorm),
        &format!(
            "cannot create a texture: a texture of {over} by 1 pixels exceeds the largest texture that the device allows, {limit} by {limit} pixels"
        ),
    );
    assert_invalid(
        Texture::from_pixels(&device, [1, over], &[]),
        &format!(
            "cannot create a texture: a texture of 1 by {over} pixels exceeds the largest texture that the device allows, {limit} by {limit} pixels"
        ),
    );
}

#[test]
fn pixels_that_do_not_fill_the_size_are_an_invalid_argument() {
    let device = device();
    assert_invalid(
        Texture::from_pixels(&device, [3, 2], &distinct_pixels(5)),
        "cannot create a texture: a texture of 3 by 2 pixels needs 6 pixels, and 5 are given",
    );
    assert_invalid(
        Texture::from_pixels(&device, [3, 2], &distinct_pixels(7)),
        "cannot create a texture: a texture of 3 by 2 pixels needs 6 pixels, and 7 are given",
    );
}

#[test]
fn the_wgpu_texture_of_a_texture_has_its_size_format_and_usages() -> slango::Result<()> {
    let device = device();
    let texture = Texture::new(&device, [7, 3], TextureFormat::Bgra8UnormSrgb)?;
    let wgpu_texture = slango::wgpu::texture(&texture);
    assert_eq!(wgpu_texture.width(), 7);
    assert_eq!(wgpu_texture.height(), 3);
    assert_eq!(wgpu_texture.depth_or_array_layers(), 1);
    assert_eq!(wgpu_texture.mip_level_count(), 1);
    assert_eq!(wgpu_texture.sample_count(), 1);
    assert_eq!(wgpu_texture.format(), wgpu::TextureFormat::Bgra8UnormSrgb);
    assert_eq!(
        wgpu_texture.usage(),
        wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::COPY_DST
    );
    Ok(())
}

pub(crate) fn wgpu_texture(
    device: &Device,
    size: [u32; 2],
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    slango::wgpu::device(device).create_texture(&wgpu::TextureDescriptor {
        label: Some("a texture of the test"),
        size: wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage,
        view_formats: &[],
    })
}

#[test]
fn a_wrapped_view_has_the_size_and_format_of_its_texture_and_reads_with_copy_src()
-> slango::Result<()> {
    let device = device();
    let texture = wgpu_texture(
        &device,
        [3, 2],
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::COPY_DST,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let wrapped = slango::wgpu::texture_from_view(&device, &view)?;
    assert_eq!(wrapped.size(), [3, 2]);
    assert_eq!(wrapped.format(), TextureFormat::Rgba8Unorm);
    assert_eq!(slango::wgpu::texture(&wrapped), &texture);
    let pixels = distinct_pixels(6);
    write_pixels(&device, &wrapped, &pixels);
    assert_eq!(wrapped.read()?, pixels);
    Ok(())
}

#[test]
fn a_wrapped_view_without_copy_src_cannot_be_read() -> slango::Result<()> {
    let device = device();
    let texture = wgpu_texture(
        &device,
        [4, 4],
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let wrapped = slango::wgpu::texture_from_view(&device, &view)?;
    assert_eq!(wrapped.format(), TextureFormat::Bgra8Unorm);
    let error = wrapped.read().expect_err("no COPY_SRC");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot read a texture: the texture lacks the usage COPY_SRC, which a read needs"
    );
    Ok(())
}

#[test]
fn a_view_of_a_format_that_slango_lacks_is_an_invalid_argument() {
    let device = device();
    let texture = wgpu_texture(
        &device,
        [2, 2],
        wgpu::TextureFormat::R8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING,
    );
    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let error = slango::wgpu::texture_from_view(&device, &view).expect_err("R8Unorm is no format");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot wrap a texture view: the viewed texture has the format R8Unorm, which slango does not support"
    );
}

#[test]
fn a_view_of_a_texture_with_several_mip_levels_layers_or_samples_is_an_invalid_argument() {
    let device = device();
    let cases = [
        (
            2,
            1,
            1,
            wgpu::TextureUsages::TEXTURE_BINDING,
            "the viewed texture has 2 mip levels, and slango supports only one",
        ),
        (
            1,
            2,
            1,
            wgpu::TextureUsages::TEXTURE_BINDING,
            "the viewed texture has 2 array layers, and slango supports only one",
        ),
        (
            1,
            1,
            4,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
            "the viewed texture has 4 samples per pixel, and slango supports only one",
        ),
    ];
    for (mip_level_count, depth_or_array_layers, sample_count, usage, detail) in cases {
        let texture = slango::wgpu::device(&device).create_texture(&wgpu::TextureDescriptor {
            label: Some("a texture that slango cannot wrap"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers,
            },
            mip_level_count,
            sample_count,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let outcome = slango::wgpu::texture_from_view(&device, &view)
            .map(|texture| texture.size())
            .map_err(|error| (error.kind(), error.to_string()));
        assert_eq!(
            outcome,
            Err((
                ErrorKind::InvalidArgument,
                format!("cannot wrap a texture view: {detail}")
            ))
        );
    }
}

#[test]
fn a_sampler_is_created_with_every_combination_of_options() -> slango::Result<()> {
    let device = device();
    for filter in [FilterMode::Nearest, FilterMode::Linear] {
        for address_mode in [
            AddressMode::ClampToEdge,
            AddressMode::Repeat,
            AddressMode::MirrorRepeat,
        ] {
            let mut options = SamplerOptions::default();
            options.filter = filter;
            options.address_mode = address_mode;
            let sampler = Sampler::new(&device, &options)?;
            let text = format!("{:?}", sampler.clone());
            assert!(text.contains(&format!("{filter:?}")), "{text}");
        }
    }
    Ok(())
}
