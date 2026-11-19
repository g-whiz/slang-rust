use std::rc::Rc;

use slang_rhi::{DepthLoad, Format, RenderPassDesc, TextureDesc, TextureUsage};

use super::device::Device;
use super::error::{Call, from_rhi};
use crate::error::{Error, ErrorKind, Result};
use crate::texture::TextureFormat;

const CREATE: &str = "create a texture";
const READ: &str = "read a texture";

/// The usage that a shader needs to sample a texture, which the errors of
/// the checks name.
pub(crate) const SAMPLE_USAGE: &str = "SHADER_RESOURCE";

/// The usage that a draw needs to draw into a texture or to test and write
/// its depths, which the errors of the checks name.
pub(crate) const TARGET_USAGE: &str = "RENDER_TARGET or DEPTH_STENCIL";

/// The usage that a read needs, which the errors of the checks name.
pub(crate) const COPY_USAGE: &str = "COPY_SOURCE";

/// The texture of slang-rhi, with its default view, that holds the pixels
/// of a [`crate::Texture`].
#[derive(Clone)]
pub(crate) struct Texture {
    inner: Rc<slang_rhi::Texture>,
}

impl Texture {
    /// Creates a texture of the format `Rgba8Unorm` from `bytes`, the pixels
    /// in rows from the top.
    pub(crate) fn from_pixels(device: &Device, size: [u32; 2], bytes: &[u8]) -> Result<Self> {
        Self::create(device, size, TextureFormat::Rgba8Unorm, Some(bytes))
    }

    /// Creates a texture of the format whose bytes are all zero: a colour
    /// texture from initial data of zeros, and a depth texture cleared to
    /// the depth 0 by a render pass before the call returns, because
    /// slang-rhi takes no initial data for a depth format and leaves a
    /// texture without it undefined on Vulkan.
    pub(crate) fn zeroed(device: &Device, size: [u32; 2], format: TextureFormat) -> Result<Self> {
        if format.is_depth() {
            let texture = Self::create(device, size, format, None)?;
            texture.clear_depth(device)?;
            return Ok(texture);
        }
        let len = pixel_bytes(size).ok_or_else(|| too_large(CREATE, size))?;
        Self::create(device, size, format, Some(&vec![0; len]))
    }

    fn create(
        device: &Device,
        size: [u32; 2],
        format: TextureFormat,
        initial: Option<&[u8]>,
    ) -> Result<Self> {
        let desc = TextureDesc::new(size, to_rhi(format), usages(format));
        let inner = device
            .rhi_device()
            .create_texture(&desc, initial)
            .map_err(|error| from_rhi(CREATE, Call::Other, error))?;
        Ok(Self {
            inner: Rc::new(inner),
        })
    }

    /// Records and submits one render pass that clears the depth texture to
    /// the depth 0 and draws nothing.
    fn clear_depth(&self, device: &Device) -> Result<()> {
        let failed = |error| from_rhi(CREATE, Call::Other, error);
        let mut encoder = device.queue().encoder().map_err(failed)?;
        let pass = RenderPassDesc::new().with_depth_target(&self.inner, DepthLoad::Clear(0.0));
        encoder.render_pass(&pass, |_| Ok(())).map_err(failed)?;
        let commands = encoder.finish().map_err(failed)?;
        device.queue().submit([commands]).map_err(failed)
    }

    /// Copies the pixels of the texture, of `size` pixels, back from the
    /// GPU, in rows from the top, after the device has finished every
    /// earlier submission.
    pub(crate) fn read_pixels(&self, device: &Device, size: [u32; 2]) -> Result<Vec<[u8; 4]>> {
        let len = pixel_bytes(size).ok_or_else(|| too_large(READ, size))?;
        device
            .queue()
            .wait_on_host()
            .map_err(|error| from_rhi(READ, Call::Other, error))?;
        let mut bytes = vec![0; len];
        device
            .rhi_device()
            .read_texture(&self.inner, &mut bytes)
            .map_err(|error| from_rhi(READ, Call::Other, error))?;
        Ok(bytes.as_chunks::<4>().0.to_vec())
    }

    /// Returns whether the texture has the usage `COPY_SOURCE`, which a read
    /// needs.
    pub(crate) fn can_copy_from(&self) -> bool {
        self.inner.usage().contains(TextureUsage::COPY_SOURCE)
    }

    /// Returns whether the texture has the usage `SHADER_RESOURCE`, which a
    /// shader needs to sample it.
    pub(crate) fn is_sampleable(&self) -> bool {
        self.inner.usage().contains(TextureUsage::SHADER_RESOURCE)
    }

    /// Returns whether the texture has the usage `RENDER_TARGET` or
    /// `DEPTH_STENCIL`, which a draw needs to draw into it or to test and
    /// write its depths.
    pub(crate) fn is_attachable(&self) -> bool {
        self.inner
            .usage()
            .intersects(TextureUsage::RENDER_TARGET | TextureUsage::DEPTH_STENCIL)
    }

    pub(super) fn rhi_texture(&self) -> &slang_rhi::Texture {
        &self.inner
    }
}

pub(super) fn to_rhi(format: TextureFormat) -> Format {
    match format {
        TextureFormat::Rgba8Unorm => Format::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb => Format::Rgba8UnormSrgb,
        TextureFormat::Bgra8Unorm => Format::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb => Format::Bgra8UnormSrgb,
        TextureFormat::Depth32Float => Format::D32Float,
    }
}

/// Returns the usages of a texture of the format that this crate creates: a
/// colour texture can be sampled, drawn into, and copied in both directions,
/// and a depth texture can only be a depth target.
fn usages(format: TextureFormat) -> TextureUsage {
    if format.is_depth() {
        TextureUsage::DEPTH_STENCIL
    } else {
        TextureUsage::SHADER_RESOURCE
            | TextureUsage::RENDER_TARGET
            | TextureUsage::COPY_SOURCE
            | TextureUsage::COPY_DESTINATION
    }
}

/// Returns the number of bytes of the pixels of a texture of `size` pixels,
/// four for each, or `None` when it does not fit the memory of the host.
fn pixel_bytes(size: [u32; 2]) -> Option<usize> {
    let [width, height] = size;
    usize::try_from(width)
        .ok()?
        .checked_mul(usize::try_from(height).ok()?)?
        .checked_mul(4)
}

fn too_large(operation: &'static str, size: [u32; 2]) -> Error {
    let [width, height] = size;
    Error::new(ErrorKind::InvalidArgument, operation).with_detail(format!(
        "the pixels of a texture of {width} by {height} pixels do not fit in the memory of the host"
    ))
}
