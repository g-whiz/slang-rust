use std::fmt;

use crate::error::{Error, ErrorKind, Result};

/// A GPU device on which this crate runs compute kernels, with the queue
/// through which it submits work to the device.
///
/// A device is the connection of a program to one adapter, which is a GPU or
/// a software implementation of one, such as lavapipe. Create one with
/// [`Device::new`], or from a device that the program already holds with
/// [`wgpu::from_device_and_queue`](crate::wgpu::from_device_and_queue).
///
/// Cloning a device is cheap and returns another handle to the same device.
/// A device is [`Send`] and [`Sync`], so threads can share it.
///
/// # Examples
///
/// See the example of [`Device::new`].
#[derive(Clone)]
pub struct Device {
    device: ::wgpu::Device,
    queue: ::wgpu::Queue,
}

impl Device {
    /// Creates a device on the adapter that the options select.
    ///
    /// Use this to start running kernels. The device has the immediates
    /// feature, through which a kernel receives the `uniform` parameters of
    /// its entry point, the function that a dispatch runs, whenever the
    /// adapter offers that feature, and then with the largest size of
    /// immediate data that the adapter allows. A device searches the adapters
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
    /// Returns an [`Error`] of kind [`ErrorKind::NoAdapter`] when no adapter
    /// matches [`DeviceOptions::adapter`], and of kind
    /// [`ErrorKind::DeviceRequest`] when the adapter refuses to create the
    /// device.
    pub fn new(options: &DeviceOptions) -> Result<Self> {
        let instance = ::wgpu::Instance::new(::wgpu::InstanceDescriptor {
            backends: ::wgpu::Backends::PRIMARY,
            ..::wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = select_adapter(&instance, &options.adapter)?;
        request_device(&adapter)
    }

    /// Returns the name of the adapter of the device, as its driver reports
    /// it, such as `llvmpipe (LLVM 19.1.7, 256 bits)` for lavapipe.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::new`].
    #[must_use]
    pub fn adapter_name(&self) -> String {
        self.device.adapter_info().name
    }

    pub(crate) fn from_wgpu(device: ::wgpu::Device, queue: ::wgpu::Queue) -> Self {
        Self { device, queue }
    }

    pub(crate) fn wgpu_device(&self) -> &::wgpu::Device {
        &self.device
    }

    pub(crate) fn wgpu_queue(&self) -> &::wgpu::Queue {
        &self.queue
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
    /// The adapter that wgpu chooses when a program states no preference,
    /// which is the first adapter that wgpu finds, a GPU or a software
    /// adapter.
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

const FIND_ADAPTER: &str = "find a GPU adapter";

fn select_adapter(
    instance: &::wgpu::Instance,
    selection: &AdapterSelection,
) -> Result<::wgpu::Adapter> {
    match selection {
        AdapterSelection::Default => request_adapter(instance, false),
        AdapterSelection::Software => request_adapter(instance, true)
            .map_err(|error| error.with_detail("no adapter is a software adapter".to_owned())),
        AdapterSelection::Name(name) => {
            let adapters =
                pollster::block_on(instance.enumerate_adapters(::wgpu::Backends::PRIMARY));
            let names: Vec<String> = adapters
                .iter()
                .map(|adapter| adapter.get_info().name)
                .collect();
            first_name_containing(&names, name)
                .and_then(|index| adapters.into_iter().nth(index))
                .ok_or_else(|| {
                    Error::new(ErrorKind::NoAdapter, FIND_ADAPTER)
                        .with_detail(format!("no adapter has a name that contains `{name}`"))
                })
        }
    }
}

fn first_name_containing(names: &[String], text: &str) -> Option<usize> {
    names.iter().position(|name| name.contains(text))
}

fn request_adapter(
    instance: &::wgpu::Instance,
    force_fallback_adapter: bool,
) -> Result<::wgpu::Adapter> {
    let options = ::wgpu::RequestAdapterOptions {
        force_fallback_adapter,
        ..::wgpu::RequestAdapterOptions::default()
    };
    pollster::block_on(instance.request_adapter(&options))
        .map_err(|error| Error::new(ErrorKind::NoAdapter, FIND_ADAPTER).with_source(error))
}

fn request_device(adapter: &::wgpu::Adapter) -> Result<Device> {
    let required_features = adapter.features() & ::wgpu::Features::IMMEDIATES;
    let max_immediate_size = if required_features.is_empty() {
        0
    } else {
        adapter.limits().max_immediate_size
    };
    let descriptor = ::wgpu::DeviceDescriptor {
        label: Some("slango"),
        required_features,
        required_limits: ::wgpu::Limits {
            max_immediate_size,
            ..::wgpu::Limits::default()
        },
        ..::wgpu::DeviceDescriptor::default()
    };
    let (device, queue) =
        pollster::block_on(adapter.request_device(&descriptor)).map_err(|error| {
            Error::new(ErrorKind::DeviceRequest, "request a device")
                .with_detail(format!(
                    "the adapter `{}` refused it",
                    adapter.get_info().name
                ))
                .with_source(error)
        })?;
    Ok(Device::from_wgpu(device, queue))
}

#[cfg(test)]
mod tests {
    use super::{AdapterSelection, Device, DeviceOptions, first_name_containing};

    #[test]
    fn default_options_select_the_default_adapter() {
        assert_eq!(DeviceOptions::default().adapter, AdapterSelection::Default);
    }

    fn names(names: &[&str]) -> Vec<String> {
        names.iter().map(|&name| name.to_owned()).collect()
    }

    #[test]
    fn name_selection_finds_a_match_after_the_first_name() {
        let names = names(&[
            "llvmpipe (LLVM 19.1.7, 256 bits)",
            "NVIDIA GeForce RTX 4070",
        ]);
        assert_eq!(first_name_containing(&names, "NVIDIA"), Some(1));
    }

    #[test]
    fn name_selection_takes_the_first_of_several_matches() {
        let names = names(&[
            "AMD Radeon",
            "NVIDIA GeForce RTX 4070",
            "NVIDIA GeForce RTX 3060",
        ]);
        assert_eq!(first_name_containing(&names, "NVIDIA"), Some(1));
    }

    #[test]
    fn name_selection_matches_with_the_case_of_each_letter() {
        let names = names(&["NVIDIA GeForce RTX 4070"]);
        assert_eq!(first_name_containing(&names, "nvidia"), None);
    }

    #[test]
    fn name_selection_without_a_match_finds_nothing() {
        let names = names(&[
            "llvmpipe (LLVM 19.1.7, 256 bits)",
            "NVIDIA GeForce RTX 4070",
        ]);
        assert_eq!(first_name_containing(&names, "Radeon"), None);
        assert_eq!(first_name_containing(&[], "Radeon"), None);
    }

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn device_and_options_are_send_and_sync() {
        assert_send_sync::<Device>();
        assert_send_sync::<DeviceOptions>();
        assert_send_sync::<AdapterSelection>();
    }
}
