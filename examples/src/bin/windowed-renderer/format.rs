//! The choice of the colour format of the surface of the example
//! `windowed-renderer`.

use slango::TextureFormat;

/// Returns the first of `formats` that `slango` can draw into, together with
/// its `slango` format, or `None` when it supports none of them.
pub(crate) fn colour_format(
    formats: &[wgpu::TextureFormat],
) -> Option<(wgpu::TextureFormat, TextureFormat)> {
    formats.iter().find_map(|&format| {
        let colour = match format {
            wgpu::TextureFormat::Rgba8Unorm => TextureFormat::Rgba8Unorm,
            wgpu::TextureFormat::Rgba8UnormSrgb => TextureFormat::Rgba8UnormSrgb,
            wgpu::TextureFormat::Bgra8Unorm => TextureFormat::Bgra8Unorm,
            wgpu::TextureFormat::Bgra8UnormSrgb => TextureFormat::Bgra8UnormSrgb,
            _ => return None,
        };
        Some((format, colour))
    })
}
