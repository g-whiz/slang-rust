use ::wgpu::util::DeviceExt as _;

use super::buffer::read_back;
use super::device::{Device, scoped};
use crate::error::{Error, ErrorKind, Result};
use crate::texture::TextureFormat;

const CREATE: &str = "create a texture";
const READ: &str = "read a texture";
const WRAP: &str = "wrap a texture view";

/// The multiple of bytes at which each row of the copy of a texture into a
/// buffer starts.
const ROW_ALIGNMENT: u32 = ::wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

/// The size in bytes of one pixel of every [`TextureFormat`].
const BYTES_PER_PIXEL: u32 = 4;

/// The usage that a shader needs to sample a texture, which the errors of
/// the checks name.
pub(crate) const SAMPLE_USAGE: &str = "TEXTURE_BINDING";

/// The usage that a draw needs to draw into a texture or to test and write
/// its depths, which the errors of the checks name.
pub(crate) const TARGET_USAGE: &str = "RENDER_ATTACHMENT";

/// The usage that a read needs, which the errors of the checks name.
pub(crate) const COPY_USAGE: &str = "COPY_SRC";

/// A wgpu texture with its view, which holds the pixels of a
/// [`crate::Texture`].
#[derive(Clone)]
pub(crate) struct Texture {
    inner: ::wgpu::Texture,
    view: ::wgpu::TextureView,
}

impl Texture {
    /// Creates a texture of the format `Rgba8Unorm` from `bytes`, the pixels
    /// in rows from the top.
    pub(crate) fn from_pixels(device: &Device, size: [u32; 2], bytes: &[u8]) -> Result<Self> {
        let format = TextureFormat::Rgba8Unorm;
        let wgpu_device = device.wgpu_device();
        let (created, error) = scoped(wgpu_device, || {
            let texture = wgpu_device.create_texture_with_data(
                device.wgpu_queue(),
                &descriptor(size, format),
                ::wgpu::util::TextureDataOrder::LayerMajor,
                bytes,
            );
            let view = texture.create_view(&::wgpu::TextureViewDescriptor::default());
            (texture, view)
        });
        Self::created(created, error)
    }

    /// Creates a texture of the format whose bytes are all zero.
    pub(crate) fn zeroed(device: &Device, size: [u32; 2], format: TextureFormat) -> Result<Self> {
        let wgpu_device = device.wgpu_device();
        let (created, error) = scoped(wgpu_device, || {
            let texture = wgpu_device.create_texture(&descriptor(size, format));
            let view = texture.create_view(&::wgpu::TextureViewDescriptor::default());
            (texture, view)
        });
        Self::created(created, error)
    }

    fn created(
        (inner, view): (::wgpu::Texture, ::wgpu::TextureView),
        error: Option<::wgpu::Error>,
    ) -> Result<Self> {
        if let Some(error) = error {
            return Err(Error::new(ErrorKind::Backend, CREATE).with_source(error));
        }
        Ok(Self { inner, view })
    }

    /// Wraps a view of a wgpu texture as a texture, and returns it with the
    /// size and format of the viewed texture.
    pub(crate) fn from_view(view: &::wgpu::TextureView) -> Result<(Self, [u32; 2], TextureFormat)> {
        let texture = view.texture();
        let format = from_wgpu(texture.format()).ok_or_else(|| {
            Error::new(ErrorKind::InvalidArgument, WRAP).with_detail(format!(
                "the viewed texture has the format {:?}, which slango does not support",
                texture.format()
            ))
        })?;
        check_wrappable(
            texture.dimension(),
            texture.mip_level_count(),
            texture.depth_or_array_layers(),
            texture.sample_count(),
        )?;
        let wrapped = Self {
            inner: texture.clone(),
            view: view.clone(),
        };
        Ok((wrapped, [texture.width(), texture.height()], format))
    }

    /// Copies the pixels of the texture, of `size` pixels, back from the
    /// GPU, in rows from the top, each row without the padding of the copy.
    pub(crate) fn read_pixels(&self, device: &Device, size: [u32; 2]) -> Result<Vec<[u8; 4]>> {
        let [width, height] = size;
        let layout = ReadLayout::new(size, ROW_ALIGNMENT).ok_or_else(|| {
            Error::new(ErrorKind::InvalidArgument, READ).with_detail(format!(
                "a texture of {width} by {height} pixels is larger than a copy can hold"
            ))
        })?;
        let bytes = self.read_bytes(device, size, layout.bytes_per_row, layout.size)?;
        layout.unpad(&bytes).ok_or_else(|| {
            Error::new(ErrorKind::Backend, READ).with_detail(format!(
                "wgpu mapped {} bytes, which are not {height} rows of {} bytes",
                bytes.len(),
                layout.bytes_per_row
            ))
        })
    }

    /// Copies the bytes of the texture back from the GPU, in rows of
    /// `bytes_per_row` bytes, `size` in all.
    fn read_bytes(
        &self,
        device: &Device,
        extent: [u32; 2],
        bytes_per_row: u32,
        size: u64,
    ) -> Result<Vec<u8>> {
        let [width, height] = extent;
        read_back(device, READ, size, |encoder, staging| {
            encoder.copy_texture_to_buffer(
                ::wgpu::TexelCopyTextureInfo {
                    texture: &self.inner,
                    mip_level: 0,
                    origin: ::wgpu::Origin3d::ZERO,
                    aspect: ::wgpu::TextureAspect::All,
                },
                ::wgpu::TexelCopyBufferInfo {
                    buffer: staging,
                    layout: ::wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bytes_per_row),
                        rows_per_image: Some(height),
                    },
                },
                ::wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        })
    }

    /// Returns whether the texture has the usage `COPY_SRC`, which a read
    /// needs.
    pub(crate) fn can_copy_from(&self) -> bool {
        self.inner.usage().contains(::wgpu::TextureUsages::COPY_SRC)
    }

    /// Returns whether the texture has the usage `TEXTURE_BINDING`, which a
    /// shader needs to sample it.
    pub(crate) fn is_sampleable(&self) -> bool {
        self.inner
            .usage()
            .contains(::wgpu::TextureUsages::TEXTURE_BINDING)
    }

    /// Returns whether the texture has the usage `RENDER_ATTACHMENT`, which
    /// a draw needs to draw into it or to test and write its depths.
    pub(crate) fn is_attachable(&self) -> bool {
        self.inner
            .usage()
            .contains(::wgpu::TextureUsages::RENDER_ATTACHMENT)
    }

    pub(crate) fn wgpu_texture(&self) -> &::wgpu::Texture {
        &self.inner
    }

    pub(super) fn wgpu_view(&self) -> &::wgpu::TextureView {
        &self.view
    }
}

pub(super) fn to_wgpu(format: TextureFormat) -> ::wgpu::TextureFormat {
    match format {
        TextureFormat::Rgba8Unorm => ::wgpu::TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb => ::wgpu::TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8Unorm => ::wgpu::TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb => ::wgpu::TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Depth32Float => ::wgpu::TextureFormat::Depth32Float,
    }
}

fn from_wgpu(format: ::wgpu::TextureFormat) -> Option<TextureFormat> {
    match format {
        ::wgpu::TextureFormat::Rgba8Unorm => Some(TextureFormat::Rgba8Unorm),
        ::wgpu::TextureFormat::Rgba8UnormSrgb => Some(TextureFormat::Rgba8UnormSrgb),
        ::wgpu::TextureFormat::Bgra8Unorm => Some(TextureFormat::Bgra8Unorm),
        ::wgpu::TextureFormat::Bgra8UnormSrgb => Some(TextureFormat::Bgra8UnormSrgb),
        ::wgpu::TextureFormat::Depth32Float => Some(TextureFormat::Depth32Float),
        _ => None,
    }
}

/// Returns the usages of a texture of the format that this crate creates: a
/// colour texture can be sampled, drawn into, and copied in both directions,
/// and a depth texture can only be a depth target.
fn usages(format: TextureFormat) -> ::wgpu::TextureUsages {
    if format.is_depth() {
        ::wgpu::TextureUsages::RENDER_ATTACHMENT
    } else {
        ::wgpu::TextureUsages::TEXTURE_BINDING
            | ::wgpu::TextureUsages::RENDER_ATTACHMENT
            | ::wgpu::TextureUsages::COPY_SRC
            | ::wgpu::TextureUsages::COPY_DST
    }
}

fn descriptor(size: [u32; 2], format: TextureFormat) -> ::wgpu::TextureDescriptor<'static> {
    let [width, height] = size;
    ::wgpu::TextureDescriptor {
        label: Some("slango texture"),
        size: ::wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: ::wgpu::TextureDimension::D2,
        format: to_wgpu(format),
        usage: usages(format),
        view_formats: &[],
    }
}

/// Fails with [`ErrorKind::InvalidArgument`] unless a wgpu texture with these
/// properties is two-dimensional, with one mip level, one array layer, and
/// one sample per pixel, so that its only view is the one that a kernel
/// samples and that [`crate::Texture::size`] and [`crate::Texture::read`]
/// describe.
fn check_wrappable(
    dimension: ::wgpu::TextureDimension,
    mip_level_count: u32,
    depth_or_array_layers: u32,
    sample_count: u32,
) -> Result<()> {
    let detail = if dimension != ::wgpu::TextureDimension::D2 {
        format!(
            "the viewed texture has the dimension {dimension:?}, and slango supports only a two-dimensional texture"
        )
    } else if sample_count != 1 {
        format!(
            "the viewed texture has {sample_count} samples per pixel, and slango supports only one"
        )
    } else if mip_level_count != 1 {
        format!("the viewed texture has {mip_level_count} mip levels, and slango supports only one")
    } else if depth_or_array_layers != 1 {
        format!(
            "the viewed texture has {depth_or_array_layers} array layers, and slango supports only one"
        )
    } else {
        return Ok(());
    };
    Err(Error::new(ErrorKind::InvalidArgument, WRAP).with_detail(detail))
}

/// The layout of the copy of a texture into a buffer: the backend needs each
/// row of the copy to start at a multiple of an alignment, so a row of the
/// copy is the row of the texture padded to that multiple.
#[derive(Debug, PartialEq, Eq)]
struct ReadLayout {
    width: u32,
    height: u32,
    bytes_per_row: u32,
    size: u64,
}

impl ReadLayout {
    fn new(size: [u32; 2], alignment: u32) -> Option<Self> {
        let [width, height] = size;
        let bytes_per_row = width
            .checked_mul(BYTES_PER_PIXEL)?
            .checked_next_multiple_of(alignment)?;
        Some(Self {
            width,
            height,
            bytes_per_row,
            size: u64::from(bytes_per_row).checked_mul(u64::from(height))?,
        })
    }

    /// Returns the pixels of the rows of a copy, without the padding of each
    /// row, or `None` when `bytes` does not hold the whole copy.
    fn unpad(&self, bytes: &[u8]) -> Option<Vec<[u8; 4]>> {
        if u64::try_from(bytes.len()).ok()? != self.size {
            return None;
        }
        let row = usize::try_from(self.bytes_per_row).ok()?;
        let used = usize::try_from(self.width.checked_mul(BYTES_PER_PIXEL)?).ok()?;
        let mut pixels = Vec::with_capacity(bytes.len() / 4);
        for padded in bytes.chunks_exact(row) {
            pixels.extend_from_slice(padded.get(..used)?.as_chunks::<4>().0);
        }
        let expected = u64::from(self.width) * u64::from(self.height);
        (u64::try_from(pixels.len()).ok()? == expected).then_some(pixels)
    }
}

#[cfg(test)]
mod tests;
