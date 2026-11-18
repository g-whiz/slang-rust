use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::backend;
use crate::error::Result;

/// A GPU device on which this crate runs compute kernels and draws, with the
/// queue through which it submits work to the device.
///
/// A device is the connection of a program to one adapter, which is a GPU or
/// a software implementation of one, such as lavapipe. Create one with
/// [`Device::new`], or from a device that the program already holds with
/// `wgpu::from_device_and_queue`.
///
/// Cloning a device is cheap and returns another handle to the same device.
///
/// A device is neither [`Send`] nor [`Sync`]: it and every object created
/// from it stay on the thread that created them, because the crate keeps all
/// the GPU objects of a device on one thread whatever graphics library runs
/// them, so a program sends results to other threads as plain data, such as
/// the vector that [`Buffer::read`](crate::Buffer::read) returns. This
/// program does not compile, because a device is not `Send`:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slango::Device>();
/// ```
///
/// This program does not compile either, because a device is not `Sync`:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slango::Device>();
/// ```
///
/// # Examples
///
/// See the example of [`Device::new`].
#[derive(Clone)]
pub struct Device {
    inner: backend::Device,
    identity: Arc<()>,
    thread_bound: PhantomData<*const ()>,
}

impl Device {
    /// Creates a device on the adapter that the options select.
    ///
    /// Use this to start running kernels and drawing. The device has the
    /// immediates feature, through which a kernel receives the `uniform`
    /// parameters of its entry point, the function that a dispatch runs,
    /// whenever the adapter offers that feature, and then with the largest
    /// size of immediate data that the adapter allows. A device searches the adapters
    /// of Vulkan, Direct3D 12, and Metal, the graphics interfaces of Linux,
    /// Windows, and macOS.
    ///
    /// # Examples
    ///
    /// This example creates a device on a software adapter and prints its
    /// name. It needs a software adapter at run time, such as lavapipe on
    /// Linux, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{AdapterSelection, Device, DeviceOptions};
    ///
    /// let mut options = DeviceOptions::default();
    /// options.adapter = AdapterSelection::Software;
    /// let device = Device::new(&options)?;
    /// println!("running on {}", device.adapter_name());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::Error) of kind
    /// [`ErrorKind::NoAdapter`](crate::ErrorKind::NoAdapter) when no adapter
    /// matches [`DeviceOptions::adapter`], and of kind
    /// [`ErrorKind::DeviceRequest`](crate::ErrorKind::DeviceRequest) when the
    /// adapter refuses to create the device.
    pub fn new(options: &DeviceOptions) -> Result<Self> {
        Ok(Self::from_backend(backend::Device::new(options)?))
    }

    /// Returns the name of the adapter of the device, as its driver reports
    /// it, such as `llvmpipe (LLVM 19.1.7, 256 bits)` for lavapipe.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::new`].
    #[must_use]
    pub fn adapter_name(&self) -> String {
        self.inner.adapter_name()
    }

    pub(crate) fn from_backend(inner: backend::Device) -> Self {
        Self {
            inner,
            identity: Arc::new(()),
            thread_bound: PhantomData,
        }
    }

    /// Returns whether `other` is this device or a clone of it.
    ///
    /// wgpu 30 compares two devices by an identifier that counts within one
    /// wgpu instance only, so two devices of two instances can compare
    /// equal, and this compares the allocation that the clones share instead.
    pub(crate) fn is_same(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }

    pub(crate) fn backend(&self) -> &backend::Device {
        &self.inner
    }
}

impl fmt::Debug for Device {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Device")
            .field("adapter", &self.adapter_name())
            .finish_non_exhaustive()
    }
}

/// The options with which [`Device::new`] creates a device.
///
/// Start from [`DeviceOptions::default`] and change the fields that matter.
/// The struct is non-exhaustive, so a later version can add options, and a
/// program sets each field on a default value instead of writing a struct
/// literal.
///
/// # Examples
///
/// See the example of [`Device::new`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct DeviceOptions {
    /// The adapter on which to create the device.
    pub adapter: AdapterSelection,
}

/// The way in which [`Device::new`] chooses an adapter, a GPU or a software
/// implementation of one.
///
/// The enumeration is non-exhaustive, so a later version can add ways, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// This example selects the first adapter whose name contains `NVIDIA`. It
/// needs such an adapter at run time, so it is compiled but not run:
///
/// ```no_run
/// use slango::{AdapterSelection, Device, DeviceOptions};
///
/// let mut options = DeviceOptions::default();
/// options.adapter = AdapterSelection::Name("NVIDIA".to_owned());
/// let device = Device::new(&options)?;
/// # drop(device);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AdapterSelection {
    /// The adapter that the backend chooses when a program states no
    /// preference, a GPU or a software adapter. With the feature `wgpu`, it
    /// is the first adapter that wgpu finds, and with the feature `rhi`, the
    /// adapter that slang-rhi opens a device on when it is given none.
    #[default]
    Default,
    /// A software adapter, an implementation of a GPU that runs on the
    /// central processing unit (CPU), such as lavapipe on Linux or the
    /// Windows Advanced Rasterization Platform (WARP) on Windows. Use it for
    /// results that do not depend on the GPU of the machine, such as in
    /// continuous integration.
    ///
    /// # Platform support
    ///
    /// Metal, the graphics interface of macOS, offers no software adapter,
    /// so this selection finds no adapter on macOS unless a Vulkan driver
    /// provides one.
    Software,
    /// The first adapter whose name contains this text, which matches with
    /// the case of each letter. The name is what [`Device::adapter_name`]
    /// returns.
    Name(String),
}

#[cfg(test)]
mod tests {
    use super::{AdapterSelection, Device, DeviceOptions};

    #[test]
    fn default_options_select_the_default_adapter() {
        assert_eq!(DeviceOptions::default().adapter, AdapterSelection::Default);
    }

    fn assert_clone<T: Clone + 'static>() {}

    fn assert_send_sync_clone<T: Send + Sync + Clone + 'static>() {}

    #[test]
    fn devices_are_clone_and_their_options_are_send_sync_and_clone() {
        assert_clone::<Device>();
        assert_send_sync_clone::<DeviceOptions>();
        assert_send_sync_clone::<AdapterSelection>();
    }
}
