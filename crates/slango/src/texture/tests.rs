use super::{
    ReadLayout, Texture, TextureFormat, check_pixel_count, check_readable, check_size,
    check_wrappable,
};
use crate::error::ErrorKind;

const FORMATS: [TextureFormat; 5] = [
    TextureFormat::Rgba8Unorm,
    TextureFormat::Rgba8UnormSrgb,
    TextureFormat::Bgra8Unorm,
    TextureFormat::Bgra8UnormSrgb,
    TextureFormat::Depth32Float,
];

#[test]
fn a_size_within_the_limit_is_accepted() {
    check_size([1, 1], 8192).expect("one pixel fits");
    check_size([8192, 3], 8192).expect("the limit itself fits");
}

#[test]
fn a_width_or_height_of_zero_is_an_invalid_argument() {
    for size in [[0, 4], [4, 0], [0, 0]] {
        let error = check_size(size, 8192).expect_err("a texture without pixels");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            format!(
                "cannot create a texture: a texture needs a width and a height of at least 1 pixel, and the size is {} by {}",
                size[0], size[1]
            )
        );
    }
}

#[test]
fn a_size_over_the_limit_is_an_invalid_argument() {
    for size in [[8193, 1], [1, 8193]] {
        let error = check_size(size, 8192).expect_err("a texture over the limit");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            format!(
                "cannot create a texture: a texture of {} by {} pixels exceeds the largest texture that the device allows, 8192 by 8192 pixels",
                size[0], size[1]
            )
        );
    }
}

#[test]
fn the_pixels_must_number_the_width_times_the_height() {
    check_pixel_count([3, 2], 6).expect("six pixels fill three by two");
    for count in [0, 5, 7] {
        let error = check_pixel_count([3, 2], count).expect_err("the wrong number of pixels");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            format!(
                "cannot create a texture: a texture of 3 by 2 pixels needs 6 pixels, and {count} are given"
            )
        );
    }
    check_pixel_count([u32::MAX, u32::MAX], usize::MAX).expect_err("no slice holds so many pixels");
}

#[test]
fn only_a_colour_texture_with_copy_src_is_readable() {
    check_readable(TextureFormat::Bgra8Unorm, true).expect("a colour texture with COPY_SRC");
    let error = check_readable(TextureFormat::Rgba8Unorm, false).expect_err("no COPY_SRC");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot read a texture: the texture lacks the usage COPY_SRC, which a read needs"
    );
    let error = check_readable(TextureFormat::Depth32Float, true).expect_err("a depth texture");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot read a texture: the texture has the depth format Depth32Float, whose depths cannot be read as pixels"
    );
}

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
fn each_row_of_a_read_is_padded_to_a_multiple_of_256_bytes() {
    let cases = [
        ([1, 1], 256, 256),
        ([3, 2], 256, 512),
        ([64, 3], 256, 768),
        ([65, 2], 512, 1024),
        ([8192, 8192], 32768, 32768 * 8192),
    ];
    for (size, bytes_per_row, total) in cases {
        let layout = ReadLayout::new(size).expect("the layout of a valid size");
        assert_eq!(layout.bytes_per_row, bytes_per_row, "{size:?}");
        assert_eq!(layout.size, total, "{size:?}");
    }
    assert_eq!(ReadLayout::new([u32::MAX, 1]), None);
}

#[test]
fn unpad_drops_the_padding_of_each_row_and_keeps_the_rows_in_order() {
    let layout = ReadLayout::new([3, 2]).expect("the layout of three by two");
    let mut bytes = vec![0xee_u8; 512];
    let pixels: [[u8; 4]; 6] = [
        [1, 2, 3, 4],
        [5, 6, 7, 8],
        [9, 10, 11, 12],
        [13, 14, 15, 16],
        [17, 18, 19, 20],
        [21, 22, 23, 24],
    ];
    for (index, pixel) in pixels.iter().enumerate() {
        let start = (index / 3) * 256 + (index % 3) * 4;
        bytes[start..start + 4].copy_from_slice(pixel);
    }
    assert_eq!(layout.unpad(&bytes), Some(pixels.to_vec()));
    assert_eq!(layout.unpad(&bytes[..511]), None);
    assert_eq!(layout.unpad(&[0; 768]), None);
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
        assert_eq!(format.to_wgpu(), wgpu_format);
        assert_eq!(TextureFormat::from_wgpu(wgpu_format), Some(format));
    }
    for other in [
        wgpu::TextureFormat::R8Unorm,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureFormat::Depth24Plus,
    ] {
        assert_eq!(TextureFormat::from_wgpu(other), None);
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
        assert_eq!(format.usages(), expected, "{format:?}");
        assert_eq!(format.is_depth(), format == TextureFormat::Depth32Float);
    }
}

fn assert_send_sync_clone<T: Send + Sync + Clone + 'static>() {}

#[test]
fn textures_and_formats_are_send_sync_and_clone() {
    assert_send_sync_clone::<Texture>();
    assert_send_sync_clone::<TextureFormat>();
}
