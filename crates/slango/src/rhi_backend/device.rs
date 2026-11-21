use std::cell::RefCell;
use std::rc::Rc;

use slang_compiler::GlobalSession;
use slang_rhi::{Adapter, AdapterType, BufferDesc, BufferUsage, DeviceInfo, DeviceType, Queue};

use super::error::{Call, from_rhi};
use crate::device::{AdapterSelection, DeviceOptions};
use crate::error::{Error, ErrorKind, Result};

const FIND_ADAPTER: &str = "find a GPU adapter";
const REQUEST: &str = "request a device";

/// The graphics API of the devices of the platform, whose adapters
/// [`AdapterSelection::Software`] and [`AdapterSelection::Name`] search.
#[cfg(target_os = "windows")]
const DEVICE_TYPE: DeviceType = DeviceType::D3d12;
#[cfg(not(target_os = "windows"))]
const DEVICE_TYPE: DeviceType = DeviceType::Vulkan;

/// The device of slang-rhi of a [`crate::Device`], with its queue and the
/// global session of Slang with which it compiles.
#[derive(Clone)]
pub(crate) struct Device {
    shared: Rc<Shared>,
}

struct Shared {
    queue: Queue,
    device: slang_rhi::Device,
    /// The buffer of zeros that feeds the vertex attributes that a vertex
    /// entry point declares and does not use, whether or not a draw gives a
    /// buffer for them, once a draw has needed it.
    zeros: RefCell<Option<slang_rhi::Buffer>>,
    global_session: GlobalSession,
}

/// The size in bytes of the buffer of zeros, one attribute of the largest
/// vertex format, four 32-bit numbers.
const ZEROS_SIZE: u64 = 16;

impl Device {
    pub(crate) fn new(options: &DeviceOptions) -> Result<Self> {
        let adapter = select_adapter(&options.adapter)?;
        let global_session = GlobalSession::new().map_err(|error| {
            Error::new(ErrorKind::DeviceRequest, REQUEST)
                .with_detail("Slang cannot create a global session".to_owned())
                .with_source(error)
        })?;
        let mut rhi_options = slang_rhi::DeviceOptions::new(global_session.clone());
        rhi_options.device_type(DEVICE_TYPE);
        if let Some(adapter) = &adapter {
            rhi_options.adapter(adapter);
        }
        let device = slang_rhi::Device::new(&rhi_options)
            .map_err(|error| from_rhi(REQUEST, Call::NewDevice, error))?;
        let queue = device
            .queue()
            .map_err(|error| from_rhi(REQUEST, Call::NewDevice, error))?;
        Ok(Self {
            shared: Rc::new(Shared {
                queue,
                device,
                zeros: RefCell::new(None),
                global_session,
            }),
        })
    }

    pub(crate) fn adapter_name(&self) -> String {
        self.info().adapter_name().to_owned()
    }

    /// Returns the size in bytes of the largest buffer that the device
    /// allows, which is the size that slang-rhi assumes.
    pub(crate) fn max_buffer_size(&self) -> u64 {
        self.info().max_buffer_size()
    }

    /// Returns the largest width or height of a two-dimensional texture that
    /// the device allows.
    pub(crate) fn max_texture_dimension(&self) -> u32 {
        self.info().max_texture_dimension_2d()
    }

    /// Returns the largest number of thread groups of one dispatch along
    /// each axis that the device allows.
    pub(crate) fn max_workgroups(&self) -> [u32; 3] {
        self.info().max_compute_dispatch_thread_groups()
    }

    /// Returns the limits of the device that slang-rhi reports.
    pub(super) fn info(&self) -> &DeviceInfo {
        self.shared.device.info()
    }

    /// Returns the buffer of 16 zero bytes of the device, which holds one
    /// attribute of every vertex format and which the device creates when
    /// a draw first needs it.
    pub(super) fn zeros(&self) -> std::result::Result<slang_rhi::Buffer, slang_rhi::Error> {
        if let Some(zeros) = self.shared.zeros.borrow().as_ref() {
            return Ok(zeros.clone());
        }
        let desc = BufferDesc::new(ZEROS_SIZE, BufferUsage::VERTEX_BUFFER);
        let zeros = self.shared.device.create_buffer(&desc, Some(&[0; 16]))?;
        *self.shared.zeros.borrow_mut() = Some(zeros.clone());
        Ok(zeros)
    }

    /// Returns the global session of Slang with which the device compiles.
    pub(super) fn global_session(&self) -> &GlobalSession {
        &self.shared.global_session
    }

    pub(super) fn rhi_device(&self) -> &slang_rhi::Device {
        &self.shared.device
    }

    pub(super) fn queue(&self) -> &Queue {
        &self.shared.queue
    }
}

/// Returns the adapter that `selection` selects among the adapters of the
/// platform's graphics API, or `None` for the adapter that slang-rhi chooses.
fn select_adapter(selection: &AdapterSelection) -> Result<Option<Adapter>> {
    let detail = match selection {
        AdapterSelection::Default => return Ok(None),
        AdapterSelection::Software => "no adapter is a software adapter".to_owned(),
        AdapterSelection::Name(name) => {
            format!("no adapter has a name that contains `{name}`")
        }
    };
    let adapters = slang_rhi::adapters(DEVICE_TYPE).map_err(|error| {
        Error::new(ErrorKind::NoAdapter, FIND_ADAPTER)
            .with_detail("slang-rhi cannot list the adapters".to_owned())
            .with_source(error)
    })?;
    let found = adapters.into_iter().find(|adapter| match selection {
        AdapterSelection::Software => adapter.adapter_type() == AdapterType::Software,
        AdapterSelection::Name(name) => adapter.name().contains(name.as_str()),
        AdapterSelection::Default => true,
    });
    match found {
        Some(adapter) => Ok(Some(adapter)),
        None => Err(Error::new(ErrorKind::NoAdapter, FIND_ADAPTER).with_detail(detail)),
    }
}
