use crate::device::{AdapterSelection, DeviceOptions};
use crate::error::{Error, ErrorKind, Result};

const FIND_ADAPTER: &str = "find a GPU adapter";

/// The wgpu device of a [`crate::Device`] with the queue through which it
/// submits work.
#[derive(Clone)]
pub(crate) struct Device {
    handle: ::wgpu::Device,
    queue: ::wgpu::Queue,
}

impl Device {
    pub(crate) fn new(options: &DeviceOptions) -> Result<Self> {
        let instance = ::wgpu::Instance::new(::wgpu::InstanceDescriptor {
            backends: ::wgpu::Backends::PRIMARY,
            ..::wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = select_adapter(&instance, &options.adapter)?;
        request_device(&adapter)
    }

    pub(crate) fn from_wgpu(device: ::wgpu::Device, queue: ::wgpu::Queue) -> Self {
        Self {
            handle: device,
            queue,
        }
    }

    pub(crate) fn adapter_name(&self) -> String {
        self.handle.adapter_info().name
    }

    pub(crate) fn wgpu_device(&self) -> &::wgpu::Device {
        &self.handle
    }

    pub(crate) fn wgpu_queue(&self) -> &::wgpu::Queue {
        &self.queue
    }

    /// Returns the size in bytes of the largest buffer that the device
    /// allows.
    pub(crate) fn max_buffer_size(&self) -> u64 {
        self.handle.limits().max_buffer_size
    }

    /// Returns the largest width or height of a two-dimensional texture that
    /// the device allows.
    pub(crate) fn max_texture_dimension(&self) -> u32 {
        self.handle.limits().max_texture_dimension_2d
    }

    /// Returns the largest number of thread groups of one dispatch along
    /// each axis that the device allows.
    pub(crate) fn max_workgroups(&self) -> [u32; 3] {
        [self.handle.limits().max_compute_workgroups_per_dimension; 3]
    }
}

/// Runs `create` inside error scopes for every filter of wgpu and returns the
/// first error that it raised, so that no error reaches wgpu's handler of
/// uncaptured errors, which panics by default.
pub(super) fn scoped<T>(
    device: &::wgpu::Device,
    create: impl FnOnce() -> T,
) -> (T, Option<::wgpu::Error>) {
    let validation = device.push_error_scope(::wgpu::ErrorFilter::Validation);
    let internal = device.push_error_scope(::wgpu::ErrorFilter::Internal);
    let out_of_memory = device.push_error_scope(::wgpu::ErrorFilter::OutOfMemory);
    let value = create();
    let out_of_memory = pollster::block_on(out_of_memory.pop());
    let internal = pollster::block_on(internal.pop());
    let validation = pollster::block_on(validation.pop());
    (value, validation.or(internal).or(out_of_memory))
}

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
mod tests;
