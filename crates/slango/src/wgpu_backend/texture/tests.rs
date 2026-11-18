use super::{check_wrappable, from_wgpu, to_wgpu, usages};
use crate::error::ErrorKind;
use crate::texture::TextureFormat;

const FORMATS: [TextureFormat; 5] = [
    TextureFormat::Rgba8Unorm,
    TextureFormat::Rgba8UnormSrgb,
    TextureFormat::Bgra8Unorm,
    TextureFormat::Bgra8UnormSrgb,
    TextureFormat::Depth32Float,
];

#[test]
fn only_a_single_sample_two_dimensional_texture_of_one_mip_level_and_layer_is_wrappable() {
    check_wrappable(::wgpu::TextureDimension::D2, 1, 1, 1).expect("one level, layer, and sample");
    let cases = [
        (
            (::wgpu::TextureDimension::D1, 1, 1, 1),
            "the viewed texture has the dimension D1, and slango supports only a two-dimensional texture",
        ),
        (
            (::wgpu::TextureDimension::D3, 1, 1, 1),
            "the viewed texture has the dimension D3, and slango supports only a two-dimensional texture",
        ),
        (
            (::wgpu::TextureDimension::D2, 1, 1, 4),
            "the viewed texture has 4 samples per pixel, and slango supports only one",
        ),
        (
            (::wgpu::TextureDimension::D2, 2, 1, 1),
            "the viewed texture has 2 mip levels, and slango supports only one",
        ),
        (
            (::wgpu::TextureDimension::D2, 1, 6, 1),
            "the viewed texture has 6 array layers, and slango supports only one",
        ),
    ];
    for ((dimension, mip_level_count, layers, sample_count), detail) in cases {
        let error = check_wrappable(dimension, mip_level_count, layers, sample_count)
            .expect_err("a texture that slango cannot wrap");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            format!("cannot wrap a texture view: {detail}")
        );
    }
}

#[test]
fn each_format_maps_to_its_wgpu_format_and_back() {
    let expected = [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
        wgpu::TextureFormat::Bgra8Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Depth32Float,
    ];
    for (format, wgpu_format) in FORMATS.into_iter().zip(expected) {
        assert_eq!(to_wgpu(format), wgpu_format);
        assert_eq!(from_wgpu(wgpu_format), Some(format));
    }
    for other in [
        wgpu::TextureFormat::R8Unorm,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureFormat::Depth24Plus,
    ] {
        assert_eq!(from_wgpu(other), None);
    }
}

#[test]
fn colour_formats_get_every_usage_and_the_depth_format_only_render_attachment() {
    let colour = wgpu::TextureUsages::TEXTURE_BINDING
        | wgpu::TextureUsages::RENDER_ATTACHMENT
        | wgpu::TextureUsages::COPY_SRC
        | wgpu::TextureUsages::COPY_DST;
    for format in FORMATS {
        let expected = if format == TextureFormat::Depth32Float {
            wgpu::TextureUsages::RENDER_ATTACHMENT
        } else {
            colour
        };
        assert_eq!(usages(format), expected, "{format:?}");
    }
}
