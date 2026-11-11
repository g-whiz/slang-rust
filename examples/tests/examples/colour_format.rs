use slango::TextureFormat;

#[path = "../../src/bin/windowed-renderer/format.rs"]
mod format;

use format::colour_format;

#[test]
fn chooses_the_first_format_that_slango_supports() {
    let formats = [
        wgpu::TextureFormat::Rgb10a2Unorm,
        wgpu::TextureFormat::Bgra8UnormSrgb,
        wgpu::TextureFormat::Rgba8Unorm,
    ];
    assert_eq!(
        colour_format(&formats),
        Some((
            wgpu::TextureFormat::Bgra8UnormSrgb,
            TextureFormat::Bgra8UnormSrgb
        ))
    );
}

#[test]
fn maps_each_supported_format_to_its_slango_format() {
    let pairs = [
        (wgpu::TextureFormat::Rgba8Unorm, TextureFormat::Rgba8Unorm),
        (
            wgpu::TextureFormat::Rgba8UnormSrgb,
            TextureFormat::Rgba8UnormSrgb,
        ),
        (wgpu::TextureFormat::Bgra8Unorm, TextureFormat::Bgra8Unorm),
        (
            wgpu::TextureFormat::Bgra8UnormSrgb,
            TextureFormat::Bgra8UnormSrgb,
        ),
    ];
    for (format, expected) in pairs {
        assert_eq!(colour_format(&[format]), Some((format, expected)));
    }
}

#[test]
fn finds_no_format_when_slango_supports_none() {
    assert_eq!(colour_format(&[]), None);
    assert_eq!(
        colour_format(&[
            wgpu::TextureFormat::Rgba16Float,
            wgpu::TextureFormat::Rgb10a2Unorm
        ]),
        None
    );
}
