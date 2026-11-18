use std::fmt;
use std::marker::PhantomData;

use crate::Device;
use crate::backend;
use crate::error::{Error, ErrorKind, Result};

const CREATE: &str = "create a texture";
const READ: &str = "read a texture";

/// The size in bytes of one pixel of every [`TextureFormat`].
const BYTES_PER_PIXEL: u32 = 4;

/// A two-dimensional image on the GPU of a [`Device`], which a shader samples
/// as a `Texture2D` and a draw draws into.
///
/// Create one from pixels with [`Texture::from_pixels`], or filled with zeros
/// in a [`TextureFormat`] with [`Texture::new`], and copy its pixels back
/// with [`Texture::read`]. A texture has one level of detail and one sample
/// per pixel, and keeps its size and format for its whole life. Its pixels
/// are in rows from the top, so the texture coordinates (0, 0) of a shader
/// address its top-left pixel.
///
/// A texture of a colour format can be sampled, read, and drawn into as the
/// colour target of a [`Draw`](crate::Draw). A texture of the depth format
/// [`TextureFormat::Depth32Float`] can only serve as the depth target of a
/// draw, so it is neither sampled nor read.
///
/// Cloning a texture is cheap and returns another handle to the same texture
/// on the GPU. A texture keeps its [`Device`] alive.
///
/// A texture is neither [`Send`] nor [`Sync`]: it stays on the thread that
/// created its [`Device`], because the crate keeps all the GPU objects of a
/// device on one thread whatever graphics library runs them. A program sends
/// results to other threads as plain data, such as the vector that
/// [`Texture::read`] returns. This program does not compile, because a
/// texture is not `Send`:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slango::Texture>();
/// ```
///
/// This program does not compile either, because a texture is not `Sync`:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slango::Texture>();
/// ```
///
/// # Examples
///
/// See the example of [`Texture::from_pixels`].
#[derive(Clone)]
pub struct Texture {
    device: Device,
    inner: backend::Texture,
    size: [u32; 2],
    format: TextureFormat,
    thread_bound: PhantomData<*const ()>,
}

impl Texture {
    /// Creates a texture of the format [`TextureFormat::Rgba8Unorm`] on the
    /// device from its pixels, given as `[red, green, blue, alpha]` in rows
    /// from the top, and a size of `[width, height]` pixels.
    ///
    /// # Examples
    ///
    /// This example creates a texture of two by one pixels, a red one on the
    /// left and a blue one on the right, and reads it back. It needs a GPU
    /// driver or a software driver at run time, so it is compiled but not
    /// run:
    ///
    /// ```no_run
    /// use slango::{Device, DeviceOptions, Texture, TextureFormat};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let pixels = [[255, 0, 0, 255], [0, 0, 255, 255]];
    /// let image = Texture::from_pixels(&device, [2, 1], &pixels)?;
    /// assert_eq!(image.size(), [2, 1]);
    /// assert_eq!(image.format(), TextureFormat::Rgba8Unorm);
    /// assert_eq!(image.read()?, pixels);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::InvalidArgument`] when the
    /// width or the height is zero or exceeds the largest texture that the
    /// device allows, or when the number of pixels is not the width times the
    /// height, and of kind [`ErrorKind::Backend`] when wgpu reports an error,
    /// such as a lack of memory, while it creates the texture.
    pub fn from_pixels(device: &Device, size: [u32; 2], pixels: &[[u8; 4]]) -> Result<Self> {
        check_size(size, device.backend().max_texture_dimension())?;
        check_pixel_count(size, pixels.len())?;
        let bytes: Vec<u8> = pixels.iter().flatten().copied().collect();
        let inner = backend::Texture::from_pixels(device.backend(), size, &bytes)?;
        Ok(Self::from_backend(
            device,
            inner,
            size,
            TextureFormat::Rgba8Unorm,
        ))
    }

    /// Creates a texture of `[width, height]` pixels in a format on the
    /// device, whose bytes are all zero.
    ///
    /// # Examples
    ///
    /// This example creates a texture of four by four pixels and reads its
    /// zeros back. It needs a GPU driver or a software driver at run time, so
    /// it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{Device, DeviceOptions, Texture, TextureFormat};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let target = Texture::new(&device, [4, 4], TextureFormat::Bgra8Unorm)?;
    /// assert_eq!(target.read()?, [[0; 4]; 16]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::InvalidArgument`] when the
    /// width or the height is zero or exceeds the largest texture that the
    /// device allows, and of kind [`ErrorKind::Backend`] when wgpu reports an
    /// error, such as a lack of memory, while it creates the texture.
    pub fn new(device: &Device, size: [u32; 2], format: TextureFormat) -> Result<Self> {
        check_size(size, device.backend().max_texture_dimension())?;
        let inner = backend::Texture::zeroed(device.backend(), size, format)?;
        Ok(Self::from_backend(device, inner, size, format))
    }

    /// Wraps a backend texture of `device` with its size and format.
    pub(crate) fn from_backend(
        device: &Device,
        inner: backend::Texture,
        size: [u32; 2],
        format: TextureFormat,
    ) -> Self {
        Self {
            device: device.clone(),
            inner,
            size,
            format,
            thread_bound: PhantomData,
        }
    }

    /// Returns the size of the texture as `[width, height]` in pixels.
    ///
    /// # Examples
    ///
    /// See the example of [`Texture::from_pixels`].
    #[must_use]
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// Returns the format of the texture.
    ///
    /// # Examples
    ///
    /// See the example of [`Texture::from_pixels`].
    #[must_use]
    pub fn format(&self) -> TextureFormat {
        self.format
    }

    /// Copies the pixels of a texture of a colour format back from the GPU,
    /// in rows from the top, each pixel as its four bytes in the order of its
    /// format.
    ///
    /// A pixel of [`TextureFormat::Rgba8Unorm`] is `[red, green, blue,
    /// alpha]`, and one of [`TextureFormat::Bgra8Unorm`] is `[blue, green,
    /// red, alpha]`. The call blocks until every kernel and draw that the
    /// device ran or queued before it has finished, as
    /// [`Buffer::read`](crate::Buffer::read) does.
    ///
    /// # Examples
    ///
    /// See the example of [`Texture::from_pixels`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::InvalidArgument`] when the
    /// texture has the depth format [`TextureFormat::Depth32Float`], or was
    /// made from a wgpu texture view by
    /// [`wgpu::texture_from_view`](crate::wgpu::texture_from_view) and its
    /// wgpu texture lacks the usage `COPY_SRC`, and of kind
    /// [`ErrorKind::Backend`] when wgpu reports an error while it copies the
    /// texture or maps the copy for reading.
    pub fn read(&self) -> Result<Vec<[u8; 4]>> {
        check_readable(self.format, self.inner.can_copy_from())?;
        let [width, height] = self.size;
        let layout = ReadLayout::new(self.size, backend::ROW_ALIGNMENT).ok_or_else(|| {
            Error::new(ErrorKind::InvalidArgument, READ).with_detail(format!(
                "a texture of {width} by {height} pixels is larger than a copy can hold"
            ))
        })?;
        let bytes = self.inner.read_bytes(
            self.device.backend(),
            self.size,
            layout.bytes_per_row,
            layout.size,
        )?;
        layout.unpad(&bytes).ok_or_else(|| {
            Error::new(ErrorKind::Backend, READ).with_detail(format!(
                "wgpu mapped {} bytes, which are not {height} rows of {} bytes",
                bytes.len(),
                layout.bytes_per_row
            ))
        })
    }

    /// Returns whether the texture was created on `device` or a clone of it,
    /// and not on another [`Device`], even one built from the same wgpu
    /// device.
    pub(crate) fn is_on(&self, device: &Device) -> bool {
        self.device.is_same(device)
    }

    /// Returns whether the texture has the usage `TEXTURE_BINDING`, which a
    /// shader needs to sample it.
    pub(crate) fn is_sampleable(&self) -> bool {
        self.inner.is_sampleable()
    }

    /// Returns whether the texture has the usage `RENDER_ATTACHMENT`, which
    /// a draw needs to draw into it or to test and write its depths.
    pub(crate) fn is_attachable(&self) -> bool {
        self.inner.is_attachable()
    }

    pub(crate) fn backend(&self) -> &backend::Texture {
        &self.inner
    }
}

impl fmt::Debug for Texture {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Texture")
            .field("size", &self.size)
            .field("format", &self.format)
            .finish_non_exhaustive()
    }
}

/// The format of the pixels of a [`Texture`].
///
/// Each colour format has four channels of 8 bits, which a shader reads as
/// numbers from 0.0 to 1.0, in the channel order and byte order that its
/// name gives. A format whose name ends in `Srgb` stores its colour
/// channels in the sRGB encoding, which the GPU converts to linear values
/// when a shader samples it. The enumeration is non-exhaustive, so a later
/// version can add formats, and a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Texture::new`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TextureFormat {
    /// Red, green, blue, and alpha, each a byte, in that byte order.
    Rgba8Unorm,
    /// Red, green, blue, and alpha, in that byte order, with the colour in
    /// the sRGB encoding.
    Rgba8UnormSrgb,
    /// Blue, green, red, and alpha, each a byte, in that byte order, which a
    /// shader still reads as red, green, blue, and alpha.
    Bgra8Unorm,
    /// Blue, green, red, and alpha, in that byte order, with the colour in
    /// the sRGB encoding.
    Bgra8UnormSrgb,
    /// One 32-bit floating-point depth per pixel, for a depth target.
    Depth32Float,
}

impl TextureFormat {
    pub(crate) fn is_depth(self) -> bool {
        match self {
            Self::Depth32Float => true,
            Self::Rgba8Unorm | Self::Rgba8UnormSrgb | Self::Bgra8Unorm | Self::Bgra8UnormSrgb => {
                false
            }
        }
    }
}

/// Fails with [`ErrorKind::InvalidArgument`] when a width or height is zero
/// or exceeds the device's `limit`.
fn check_size(size: [u32; 2], limit: u32) -> Result<()> {
    let [width, height] = size;
    if width == 0 || height == 0 {
        return Err(
            Error::new(ErrorKind::InvalidArgument, CREATE).with_detail(format!(
                "a texture needs a width and a height of at least 1 pixel, and the size is {width} by {height}"
            )),
        );
    }
    if width > limit || height > limit {
        return Err(
            Error::new(ErrorKind::InvalidArgument, CREATE).with_detail(format!(
                "a texture of {width} by {height} pixels exceeds the largest texture that the device allows, {limit} by {limit} pixels"
            )),
        );
    }
    Ok(())
}

/// Fails with [`ErrorKind::InvalidArgument`] when `count` pixels are not the
/// width times the height of `size`.
fn check_pixel_count(size: [u32; 2], count: usize) -> Result<()> {
    let [width, height] = size;
    let needed = u64::from(width) * u64::from(height);
    if u64::try_from(count).is_ok_and(|count| count == needed) {
        return Ok(());
    }
    Err(
        Error::new(ErrorKind::InvalidArgument, CREATE).with_detail(format!(
            "a texture of {width} by {height} pixels needs {needed} pixels, and {count} are given"
        )),
    )
}

/// Fails with [`ErrorKind::InvalidArgument`] when a texture of `format`,
/// with or without the usage `COPY_SRC`, cannot be read.
fn check_readable(format: TextureFormat, copy_source: bool) -> Result<()> {
    if format.is_depth() {
        return Err(
            Error::new(ErrorKind::InvalidArgument, READ).with_detail(format!(
                "the texture has the depth format {format:?}, whose depths cannot be read as pixels"
            )),
        );
    }
    if !copy_source {
        return Err(Error::new(ErrorKind::InvalidArgument, READ)
            .with_detail("the texture lacks the usage COPY_SRC, which a read needs".to_owned()));
    }
    Ok(())
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
