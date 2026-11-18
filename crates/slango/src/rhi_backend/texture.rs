use std::convert::Infallible;

use super::device::Device;
use crate::error::{Error, ErrorKind, Result};
use crate::texture::TextureFormat;

/// The alignment of the rows of a texture that a read copies, which no read
/// on this backend reaches, because it creates no texture.
pub(crate) const ROW_ALIGNMENT: u32 = 1;

/// The texture of a [`crate::Texture`], which this backend never creates, so
/// no value of the type exists.
#[derive(Clone)]
pub(crate) struct Texture {
    never: Infallible,
}

impl Texture {
    pub(crate) fn from_pixels(_device: &Device, _size: [u32; 2], _bytes: &[u8]) -> Result<Self> {
        Err(unsupported())
    }

    pub(crate) fn zeroed(
        _device: &Device,
        _size: [u32; 2],
        _format: TextureFormat,
    ) -> Result<Self> {
        Err(unsupported())
    }

    pub(crate) fn read_bytes(
        &self,
        _device: &Device,
        _extent: [u32; 2],
        _bytes_per_row: u32,
        _size: u64,
    ) -> Result<Vec<u8>> {
        match self.never {}
    }

    pub(crate) fn can_copy_from(&self) -> bool {
        match self.never {}
    }

    pub(crate) fn is_sampleable(&self) -> bool {
        match self.never {}
    }

    pub(crate) fn is_attachable(&self) -> bool {
        match self.never {}
    }

    /// Returns any value, because no texture exists to call it on.
    pub(super) fn unreachable<T>(&self) -> T {
        match self.never {}
    }
}

fn unsupported() -> Error {
    Error::new(ErrorKind::Unsupported, "create a texture")
        .with_detail("slango with the feature `rhi` creates no textures".to_owned())
}
