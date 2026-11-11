//! The boundary between this crate and wgpu, the graphics library on which it
//! runs.
//!
//! Go to this module to use this crate together with wgpu code of your own:
//! to create a [`Device`] from a wgpu device that your program already holds,
//! with [`from_device_and_queue`], to reach the wgpu objects of a
//! [`Device`], with [`device`] and [`queue`], to reach the wgpu texture of a
//! [`Texture`], with [`texture`], or to use a wgpu texture view of your own as
//! a [`Texture`], with [`texture_from_view`]. Every public item of this crate
//! that names a type of wgpu is in this module, so the rest of the crate does
//! not depend on wgpu's API. The types are those of wgpu 30.

use crate::error::Result;
use crate::{Device, Texture};

/// Creates a [`Device`] from a wgpu device and its queue.
///
/// Use this when your program creates its wgpu device itself, such as to
/// share it with a renderer or to request features that
/// [`Device::new`](crate::Device::new) does not request. The device keeps the
/// features and limits that it was created with. A kernel that receives
/// `uniform` parameters of its entry point needs the feature
/// [`wgpu::Features::IMMEDIATES`](::wgpu::Features::IMMEDIATES) with a large
/// enough limit of immediate data.
///
/// # Examples
///
/// This example wraps a device that a program created with wgpu:
///
/// ```
/// fn wrap(device: wgpu::Device, queue: wgpu::Queue) -> slango::Device {
///     slango::wgpu::from_device_and_queue(device, queue)
/// }
/// # let _ = wrap;
/// ```
#[must_use]
pub fn from_device_and_queue(device: ::wgpu::Device, queue: ::wgpu::Queue) -> Device {
    Device::from_wgpu(device, queue)
}

/// Returns the wgpu device of a [`Device`].
///
/// Use it to create wgpu objects of your own on the same device, such as a
/// texture to render the results of a kernel.
///
/// # Examples
///
/// This example reads the features of the device:
///
/// ```
/// fn has_immediates(device: &slango::Device) -> bool {
///     slango::wgpu::device(device)
///         .features()
///         .contains(wgpu::Features::IMMEDIATES)
/// }
/// # let _ = has_immediates;
/// ```
#[must_use]
pub fn device(device: &Device) -> &::wgpu::Device {
    device.wgpu_device()
}

/// Returns the wgpu queue of a [`Device`], through which work is submitted to
/// the device.
///
/// Use it to submit work of your own to the same queue as this crate, so that
/// the device runs both in the order of submission.
///
/// # Examples
///
/// This example writes bytes into a wgpu buffer through the queue:
///
/// ```
/// fn upload(device: &slango::Device, buffer: &wgpu::Buffer, bytes: &[u8]) {
///     slango::wgpu::queue(device).write_buffer(buffer, 0, bytes);
/// }
/// # let _ = upload;
/// ```
#[must_use]
pub fn queue(device: &Device) -> &::wgpu::Queue {
    device.wgpu_queue()
}

/// Returns the wgpu texture of a [`Texture`].
///
/// Use it to work on the texture with wgpu code of your own, such as to copy
/// pixels into it with [`wgpu::Queue::write_texture`](::wgpu::Queue::write_texture),
/// which needs the usage `COPY_DST` that every texture of a colour format
/// has.
///
/// # Examples
///
/// This example writes one pixel into the top-left corner of a texture:
///
/// ```
/// fn write_corner(device: &slango::Device, texture: &slango::Texture, pixel: [u8; 4]) {
///     slango::wgpu::queue(device).write_texture(
///         slango::wgpu::texture(texture).as_image_copy(),
///         &pixel,
///         wgpu::TexelCopyBufferLayout::default(),
///         wgpu::Extent3d::default(),
///     );
/// }
/// # let _ = write_corner;
/// ```
#[must_use]
pub fn texture(texture: &Texture) -> &::wgpu::Texture {
    texture.wgpu_texture()
}

/// Wraps a wgpu texture view of the wgpu device of a [`Device`] as a
/// [`Texture`] of that device, such as the view of the texture of a window's
/// surface.
///
/// The wgpu texture that the view shows must be a two-dimensional texture
/// with one mip level, one array layer, and one sample per pixel, so that
/// the view shows all of it. The texture takes its size and format from that
/// wgpu texture, and can do what the usages of that wgpu texture allow. A
/// shader samples it only with the usage `TEXTURE_BINDING`, a draw draws
/// into it only with the usage `RENDER_ATTACHMENT`, which the texture of a
/// surface has, and [`Texture::read`](crate::Texture::read) reads it only
/// with the usage `COPY_SRC`, which the texture of a surface usually lacks.
/// The view must belong to the wgpu device of `device`.
///
/// # Examples
///
/// This example wraps the view of a texture that a program created with
/// wgpu:
///
/// ```
/// fn wrap(device: &slango::Device, texture: &wgpu::Texture) -> slango::Result<slango::Texture> {
///     let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
///     slango::wgpu::texture_from_view(device, &view)
/// }
/// # let _ = wrap;
/// ```
///
/// # Errors
///
/// Returns an [`Error`](crate::Error) of kind
/// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument) when
/// the wgpu texture has a format that
/// [`TextureFormat`](crate::TextureFormat) lacks, is not two-dimensional, or
/// has more than one mip level, array layer, or sample per pixel.
pub fn texture_from_view(device: &Device, view: &::wgpu::TextureView) -> Result<Texture> {
    Texture::from_view(device, view)
}
