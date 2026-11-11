//! The boundary between this crate and wgpu, the graphics library on which it
//! runs.
//!
//! Go to this module to use this crate together with wgpu code of your own:
//! to create a [`Device`] from a wgpu device that your program already holds,
//! with [`from_device_and_queue`], or to reach the wgpu objects of a
//! [`Device`], with [`device`] and [`queue`]. Every public item of this crate
//! that names a type of wgpu is in this module, so the rest of the crate does
//! not depend on wgpu's API. The types are those of wgpu 30.

use crate::Device;

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
