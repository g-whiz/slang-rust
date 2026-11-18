use std::ffi::{CStr, c_char};
use std::fmt;
use std::ptr::{self, NonNull};
use std::sync::Arc;

use slang_compiler::{GlobalSession, Session, Toggle};
use slang_compiler_sys::ISession;
use slang_rhi_sys::{IAdapter, IDevice, IRHI};

use crate::callback::{self, Callback, DebugMessage};
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::instance;
use crate::owned::Owned;

/// A graphics API that slang-rhi can open a device on.
///
/// The enumeration is non-exhaustive, so a later version can add device
/// types, and a `match` on it needs a wildcard arm. This crate builds only
/// the Vulkan backend of slang-rhi, so a device of any other type fails to
/// open.
///
/// # Examples
///
/// See the example of [`adapters`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DeviceType {
    /// The default graphics API of the platform, which is Vulkan on Linux.
    Default,
    /// Direct3D 11.
    D3d11,
    /// Direct3D 12.
    D3d12,
    /// Vulkan.
    Vulkan,
    /// Metal.
    Metal,
    /// The central processing unit (CPU), with no graphics API.
    Cpu,
    /// CUDA, the compute API of NVIDIA.
    Cuda,
    /// WebGPU.
    Wgpu,
}

impl DeviceType {
    pub(crate) fn to_raw(self) -> slang_rhi_sys::DeviceType {
        match self {
            Self::Default => slang_rhi_sys::DeviceType::Default,
            Self::D3d11 => slang_rhi_sys::DeviceType::D3D11,
            Self::D3d12 => slang_rhi_sys::DeviceType::D3D12,
            Self::Vulkan => slang_rhi_sys::DeviceType::Vulkan,
            Self::Metal => slang_rhi_sys::DeviceType::Metal,
            Self::Cpu => slang_rhi_sys::DeviceType::CPU,
            Self::Cuda => slang_rhi_sys::DeviceType::CUDA,
            Self::Wgpu => slang_rhi_sys::DeviceType::WGPU,
        }
    }

    fn from_raw(raw: slang_rhi_sys::DeviceType, operation: Operation) -> Result<Self> {
        match raw {
            slang_rhi_sys::DeviceType::Default => Ok(Self::Default),
            slang_rhi_sys::DeviceType::D3D11 => Ok(Self::D3d11),
            slang_rhi_sys::DeviceType::D3D12 => Ok(Self::D3d12),
            slang_rhi_sys::DeviceType::Vulkan => Ok(Self::Vulkan),
            slang_rhi_sys::DeviceType::Metal => Ok(Self::Metal),
            slang_rhi_sys::DeviceType::CPU => Ok(Self::Cpu),
            slang_rhi_sys::DeviceType::CUDA => Ok(Self::Cuda),
            slang_rhi_sys::DeviceType::WGPU => Ok(Self::Wgpu),
            other => Err(Error::unrecognized(
                operation,
                format!("slang-rhi reported the device type {}", other.0),
            )),
        }
    }
}

/// The kind of hardware of an adapter.
///
/// The enumeration is non-exhaustive, so a later version can add kinds, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`adapters`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AdapterType {
    /// A graphics card of its own.
    Discrete,
    /// A graphics processor that shares the chip and memory of the CPU.
    Integrated,
    /// A driver that runs on the CPU, such as lavapipe.
    Software,
    /// A kind that the driver does not report.
    Unknown,
}

impl AdapterType {
    fn from_raw(raw: slang_rhi_sys::AdapterType) -> Result<Self> {
        match raw {
            slang_rhi_sys::AdapterType::Discrete => Ok(Self::Discrete),
            slang_rhi_sys::AdapterType::Integrated => Ok(Self::Integrated),
            slang_rhi_sys::AdapterType::Software => Ok(Self::Software),
            slang_rhi_sys::AdapterType::Unknown => Ok(Self::Unknown),
            other => Err(Error::unrecognized(
                Operation::ListAdapters,
                format!("slang-rhi reported the adapter type {}", other.0),
            )),
        }
    }
}

/// One adapter, a graphics processor or a software driver that a device can
/// open, with a copy of its information.
///
/// Get the adapters of a graphics API from [`adapters`], and open a device on
/// one with [`DeviceOptions::adapter`].
///
/// # Examples
///
/// See the example of [`adapters`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Adapter {
    device_type: DeviceType,
    hardware: AdapterType,
    name: String,
    vendor_id: u32,
    device_id: u32,
    luid: [u8; 16],
    position: u32,
}

impl Adapter {
    /// Returns the graphics API of the adapter.
    ///
    /// # Examples
    ///
    /// See the example of [`adapters`].
    #[must_use]
    pub fn device_type(&self) -> DeviceType {
        self.device_type
    }

    /// Returns the kind of hardware of the adapter.
    ///
    /// # Examples
    ///
    /// See the example of [`adapters`].
    #[must_use]
    pub fn adapter_type(&self) -> AdapterType {
        self.hardware
    }

    /// Returns the name of the adapter, with each sequence that is not valid
    /// UTF-8 replaced by U+FFFD, the replacement character.
    ///
    /// # Examples
    ///
    /// See the example of [`adapters`].
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the identifier of the vendor of the adapter, which the
    /// Peripheral Component Interconnect (PCI) standard assigns.
    ///
    /// # Examples
    ///
    /// See the example of [`adapters`].
    #[must_use]
    pub fn vendor_id(&self) -> u32 {
        self.vendor_id
    }

    /// Returns the identifier of the adapter that its vendor assigns.
    ///
    /// # Examples
    ///
    /// See the example of [`adapters`].
    #[must_use]
    pub fn device_id(&self) -> u32 {
        self.device_id
    }

    /// Returns the locally unique identifier (LUID) of the adapter, which
    /// identifies it among the adapters of the machine.
    ///
    /// # Examples
    ///
    /// See the example of [`adapters`].
    #[must_use]
    pub fn luid(&self) -> [u8; 16] {
        self.luid
    }
}

/// Returns the text of a fixed array of C characters up to its first NUL
/// byte, or all of it when it has none, with each sequence that is not valid
/// UTF-8 replaced by U+FFFD.
fn name_from_array(name: &[c_char]) -> String {
    let bytes: Vec<u8> = name
        .iter()
        .map(|byte| byte.to_ne_bytes()[0])
        .take_while(|byte| *byte != 0)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Copies the information of the adapter at `position`, or returns `None`
/// when the instance has no adapter there.
fn fetch_adapter(
    rhi: NonNull<IRHI>,
    device_type: DeviceType,
    position: u32,
) -> Result<Option<(NonNull<IAdapter>, Adapter)>> {
    let raw_type = device_type.to_raw();
    let adapter = guard::call("IRHI::getAdapter", || {
        // SAFETY: `rhi` is the live instance of the process, and the caller
        // holds the lock of the crate around calls through it.
        unsafe { IRHI::getAdapter(rhi.as_ptr(), raw_type, position) }
    });
    let Some(adapter) = NonNull::new(adapter) else {
        return Ok(None);
    };
    let info = guard::call("IAdapter::getInfo", || {
        // SAFETY: `adapter` is non-null and stays valid until the instance
        // is destroyed, which this crate never does.
        unsafe { IAdapter::getInfo(adapter.as_ptr()) }
    });
    // SAFETY: a non-null `info` points to the information of the adapter,
    // which lives as long as the adapter, and slang-rhi does not write it
    // after it lists the adapter. The reference does not outlive this
    // function.
    let info = unsafe { info.as_ref() }.ok_or_else(|| Error::null(Operation::ListAdapters))?;
    Ok(Some((
        adapter,
        Adapter {
            device_type: DeviceType::from_raw(info.deviceType, Operation::ListAdapters)?,
            hardware: AdapterType::from_raw(info.adapterType)?,
            name: name_from_array(&info.name),
            vendor_id: info.vendorID,
            device_id: info.deviceID,
            luid: info.luid.luid,
            position,
        },
    )))
}

/// Returns the adapters of a graphics API in the order in which slang-rhi
/// lists them.
///
/// An empty list means that slang-rhi finds no adapter of that API, such as
/// when no Vulkan driver is installed, and it is not an error.
///
/// # Examples
///
/// This example prints every Vulkan adapter. It needs the Vulkan loader at
/// run time, so it is compiled but not run:
///
/// ```no_run
/// use slang_rhi::DeviceType;
///
/// for adapter in slang_rhi::adapters(DeviceType::Vulkan)? {
///     println!(
///         "{} ({:?}, vendor {:#06x}, device {:#06x}, {:?}, LUID {:?})",
///         adapter.name(),
///         adapter.adapter_type(),
///         adapter.vendor_id(),
///         adapter.device_id(),
///         adapter.device_type(),
///         adapter.luid(),
///     );
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an error of kind [`ErrorKind::Failed`](crate::ErrorKind::Failed)
/// when slang-rhi returns no instance or no information for an adapter, and
/// an error of kind
/// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when it
/// reports a device type or an adapter type that this crate does not know.
///
/// # Panics
///
/// Panics when it is called from the debug callback of a device while a call
/// of this crate into slang-rhi is in progress on the same thread. The panic
/// aborts the process at the boundary of the callback.
pub fn adapters(device_type: DeviceType) -> Result<Vec<Adapter>> {
    instance::with_instance(Operation::ListAdapters, "slang_rhi::adapters", |rhi| {
        let mut adapters = Vec::new();
        for position in 0.. {
            match fetch_adapter(rhi, device_type, position)? {
                Some((_, adapter)) => adapters.push(adapter),
                None => break,
            }
        }
        Ok(adapters)
    })
}

/// The options of a device, which [`Device::new`] reads.
///
/// The setters take and return `&mut Self`, so they chain, and the options
/// can open several devices.
///
/// The options hold a global session of `slang-compiler`, so they are neither
/// [`Send`] nor [`Sync`].
///
/// # Examples
///
/// See the example of [`DeviceOptions::debug_callback`].
///
/// The options cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::DeviceOptions>();
/// ```
///
/// They cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::DeviceOptions>();
/// ```
pub struct DeviceOptions {
    global_session: GlobalSession,
    device_type: DeviceType,
    adapter: Option<Adapter>,
    validation: Toggle,
    callback: Option<Arc<Callback>>,
}

impl DeviceOptions {
    /// Creates the options of a device that compiles with `global_session`,
    /// with the device type [`DeviceType::Default`], no adapter, which lets
    /// slang-rhi choose one, validation off, and no debug callback.
    ///
    /// The global session is the one through which the program loads and
    /// links its modules, because a shader program of the device takes only a
    /// program of the device's own session, which the device creates from
    /// this global session.
    ///
    /// # Examples
    ///
    /// See the example of [`DeviceOptions::debug_callback`].
    #[must_use]
    pub fn new(global_session: GlobalSession) -> Self {
        Self {
            global_session,
            device_type: DeviceType::Default,
            adapter: None,
            validation: Toggle::Off,
            callback: None,
        }
    }

    /// Sets the graphics API of the device.
    ///
    /// # Examples
    ///
    /// See the example of [`DeviceOptions::debug_callback`].
    pub fn device_type(&mut self, device_type: DeviceType) -> &mut Self {
        self.device_type = device_type;
        self
    }

    /// Sets the adapter that the device opens on, whose device type must
    /// equal the device type of the options.
    ///
    /// # Examples
    ///
    /// This example opens a device on each Vulkan adapter. It needs a Vulkan
    /// driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::GlobalSession;
    /// use slang_rhi::{Device, DeviceOptions, DeviceType};
    ///
    /// let mut options = DeviceOptions::new(GlobalSession::new()?);
    /// options.device_type(DeviceType::Vulkan);
    /// for adapter in slang_rhi::adapters(DeviceType::Vulkan)? {
    ///     let device = Device::new(options.adapter(&adapter))?;
    ///     println!("{}", device.info().adapter_name());
    /// }
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn adapter(&mut self, adapter: &Adapter) -> &mut Self {
        self.adapter = Some(adapter.clone());
        self
    }

    /// Turns the debug device of slang-rhi on or off.
    ///
    /// The debug device is a layer of slang-rhi around the device that checks
    /// each call into slang-rhi against the rules of slang-rhi and reports
    /// each violation through the debug callback. It makes every call slower.
    /// It does not turn on a validation layer of the graphics API, such as
    /// the Khronos validation layer of Vulkan, so it does not check the calls
    /// that slang-rhi makes into the graphics API.
    ///
    /// # Examples
    ///
    /// See the example of [`DeviceOptions::debug_callback`].
    pub fn validation(&mut self, validation: Toggle) -> &mut Self {
        self.validation = validation;
        self
    }

    /// Sets the closure that receives every message of the devices that
    /// these options open, such as an error of the validation layer or of
    /// Slang.
    ///
    /// slang-rhi calls the closure from the thread of the call that caused
    /// the message or from a thread of the driver, and from several threads at
    /// once, so the closure is [`Send`] and [`Sync`], and it is `'static`,
    /// because slang-rhi keeps it beyond any borrow of the caller.
    ///
    /// The closure must not panic, because a panic aborts the process when it
    /// reaches slang-rhi. It must not use the objects of this crate or of
    /// `slang-compiler`, because slang-rhi can call it while it is inside a
    /// call of this crate, and such a use panics, which also aborts the
    /// process. The crate never frees the closure of a device, because
    /// slang-rhi gives no notice when it stops calling it.
    ///
    /// # Examples
    ///
    /// This example counts the errors that a device reports. It needs a
    /// Vulkan driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use std::sync::Arc;
    /// use std::sync::atomic::{AtomicUsize, Ordering};
    ///
    /// use slang_compiler::{GlobalSession, Toggle};
    /// use slang_rhi::{DebugMessageSource, DebugMessageType, Device, DeviceOptions, DeviceType};
    ///
    /// let errors = Arc::new(AtomicUsize::new(0));
    /// let counter = Arc::clone(&errors);
    /// let mut options = DeviceOptions::new(GlobalSession::new()?);
    /// options
    ///     .device_type(DeviceType::Vulkan)
    ///     .validation(Toggle::On)
    ///     .debug_callback(move |message| {
    ///         if message.message_type() == DebugMessageType::Error {
    ///             counter.fetch_add(1, Ordering::Relaxed);
    ///         }
    ///         if message.source() == DebugMessageSource::Slang {
    ///             eprintln!("Slang: {}", message.text());
    ///         }
    ///     });
    /// let device = Device::new(&options)?;
    /// # drop(device);
    /// println!("{} errors", errors.load(Ordering::Relaxed));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn debug_callback(
        &mut self,
        callback: impl Fn(&DebugMessage<'_>) + Send + Sync + 'static,
    ) -> &mut Self {
        self.callback = Some(Arc::new(callback));
        self
    }
}

impl fmt::Debug for DeviceOptions {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeviceOptions")
            .field("global_session", &self.global_session)
            .field("device_type", &self.device_type)
            .field("adapter", &self.adapter)
            .field("validation", &self.validation)
            .field("debug_callback", &self.callback.is_some())
            .finish()
    }
}

/// The information of a device.
///
/// # Examples
///
/// See the example of [`Device::info`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeviceInfo {
    device_type: DeviceType,
    api_name: String,
    adapter_name: String,
}

impl DeviceInfo {
    /// Returns the graphics API of the device.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::info`].
    #[must_use]
    pub fn device_type(&self) -> DeviceType {
        self.device_type
    }

    /// Returns the name of the graphics API, such as `Vulkan`.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::info`].
    #[must_use]
    pub fn api_name(&self) -> &str {
        &self.api_name
    }

    /// Returns the name of the adapter that the device opened on.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::info`].
    #[must_use]
    pub fn adapter_name(&self) -> &str {
        &self.adapter_name
    }
}

/// Copies a NUL-terminated string that slang-rhi returned, or returns an
/// empty string for a null pointer.
///
/// # Safety
///
/// `text` is null or points to a NUL-terminated string that stays valid
/// during the call. Breaking this is undefined behaviour.
unsafe fn copy_text(text: *const c_char) -> String {
    if text.is_null() {
        return String::new();
    }
    // SAFETY: the caller guarantees that a non-null `text` is a live
    // NUL-terminated string, and the reference does not outlive the copy.
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

/// A device of slang-rhi, an open graphics processor or software driver that
/// runs compute kernels.
///
/// Open one with [`Device::new`]. A device compiles with the global session of
/// `slang-compiler` that its [`DeviceOptions`] name, through a session of its
/// own, which [`Device::slang_session`] returns. Every object that a device
/// creates holds the device, so the device lives at least as long as any of
/// them. [`Clone`] returns another handle to the same device.
///
/// A device is neither [`Send`] nor [`Sync`], because slang-rhi documents no
/// rule for the use of its objects from several threads, and the sessions of
/// `slang-compiler` that it holds are neither. A program that uses several
/// threads opens a device on each.
///
/// # Examples
///
/// See the example of [`Device::create_buffer`].
///
/// A device cannot move to another thread, so this example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::Device>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::Device>();
/// ```
#[derive(Clone, Debug)]
pub struct Device {
    raw: Owned<IDevice>,
    session: Session,
    _global_session: GlobalSession,
    info: DeviceInfo,
    max_thread_groups: [u32; 3],
}

impl Device {
    /// Opens a device with the options `options`.
    ///
    /// The device receives the global session of the options, so the loader
    /// that `slang-compiler` installs on its global sessions, which optimizes
    /// SPIR-V, serves the device too. With an adapter in the options, the
    /// device opens on that adapter, and otherwise slang-rhi chooses one.
    ///
    /// # Examples
    ///
    /// See the example of [`DeviceOptions::debug_callback`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument)
    /// when the options name an adapter whose device type differs from the
    /// device type of the options, or one that slang-rhi no longer lists. Returns
    /// an error of kind [`ErrorKind::NoDevice`](crate::ErrorKind::NoDevice)
    /// when slang-rhi cannot open the device, such as when no Vulkan loader or
    /// driver is installed, and an error of kind
    /// [`ErrorKind::Failed`](crate::ErrorKind::Failed) or
    /// [`ErrorKind::Unrecognized`](crate::ErrorKind::Unrecognized) when the
    /// opened device reports no information or information that this crate
    /// does not know.
    ///
    /// # Panics
    ///
    /// Panics when it is called from the debug callback of a device while a
    /// call of this crate into slang-rhi is in progress on the same thread.
    /// The panic aborts the process at the boundary of the callback.
    pub fn new(options: &DeviceOptions) -> Result<Self> {
        if let Some(adapter) = &options.adapter
            && adapter.device_type != options.device_type
        {
            return Err(Error::invalid(
                Operation::CreateDevice,
                format!(
                    "the adapter has the device type {:?} and the options {:?}",
                    adapter.device_type, options.device_type
                ),
            ));
        }
        let global_session = options.global_session.clone();
        let raw = instance::with_instance(Operation::CreateDevice, "Device::new", |rhi| {
            let adapter = match &options.adapter {
                Some(adapter) => {
                    let (raw, _) = fetch_adapter(rhi, adapter.device_type, adapter.position)?
                        .ok_or_else(|| {
                            Error::invalid(
                                Operation::CreateDevice,
                                format!(
                                    "slang-rhi lists no adapter at the position {}",
                                    adapter.position
                                ),
                            )
                        })?;
                    raw.as_ptr()
                }
                None => ptr::null_mut(),
            };
            let desc = slang_rhi_sys::DeviceDesc {
                deviceType: options.device_type.to_raw(),
                adapter,
                slang: slang_rhi_sys::SlangDesc {
                    slangGlobalSession: global_session.as_raw().as_ptr(),
                    ..slang_rhi_sys::SlangDesc::default()
                },
                enableValidation: matches!(options.validation, Toggle::On),
                debugCallback: callback::leak(options.callback.clone()),
                pipelineCompilationMode: slang_rhi_sys::PipelineCompilationMode::Serial,
                ..slang_rhi_sys::DeviceDesc::default()
            };
            let mut out: *mut IDevice = ptr::null_mut();
            let result = guard::call("IRHI::createDevice", || {
                // SAFETY: `rhi` is the live instance, and the lock of the
                // crate is held. `desc` is a live descriptor with the
                // defaults of the header, whose adapter is null or a live
                // adapter of the instance, whose global session lives while
                // `global_session` does, and which the device keeps a
                // reference of, and whose debug callback object is never
                // freed, and with serial compilation, so that slang-rhi calls
                // Slang only on the thread of each call of this crate. `out`
                // is a live out-pointer.
                unsafe { IRHI::createDevice(rhi.as_ptr(), &raw const desc, &raw mut out) }
            });
            error::check(Operation::CreateDevice, result, None)?;
            // SAFETY: the call succeeded, and slang-rhi returns the device
            // with one reference added for the caller.
            unsafe { Owned::adopt(out) }.ok_or_else(|| Error::null(Operation::CreateDevice))
        })?;
        let device = raw.as_ptr();
        let info = guard::call("IDevice::getInfo", || {
            // SAFETY: `device` is live while `raw` lives.
            unsafe { IDevice::getInfo(device) }
        });
        // SAFETY: a non-null `info` points to the information of the device,
        // which lives as long as the device, and the reference does not
        // outlive this function. Its two names are null or NUL-terminated
        // strings that live as long as the device.
        let info = unsafe { info.as_ref() }.ok_or_else(|| Error::null(Operation::CreateDevice))?;
        let max_thread_groups = info.limits.maxComputeDispatchThreadGroups;
        let info = DeviceInfo {
            device_type: DeviceType::from_raw(info.deviceType, Operation::CreateDevice)?,
            // SAFETY: as stated above for the names.
            api_name: unsafe { copy_text(info.apiName) },
            // SAFETY: as stated above for the names.
            adapter_name: unsafe { copy_text(info.adapterName) },
        };
        let mut session: *mut ISession = ptr::null_mut();
        let result = guard::call("IDevice::getSlangSession", || {
            // SAFETY: `device` is live, and `session` is a live out-pointer.
            unsafe { IDevice::getSlangSession(device, &raw mut session) }
        });
        error::check(Operation::CreateDevice, result, None)?;
        let session = NonNull::new(session).ok_or_else(|| Error::null(Operation::CreateDevice))?;
        // SAFETY: slang-rhi returns the session of the device with one
        // reference added for the caller. The Slang that it was created by is
        // the one that `slang-compiler` links, from `global_session`, with
        // one target, and this crate uses the device, and so the session, on
        // this thread only, because no type of the crate is `Send`.
        let session = unsafe { Session::from_raw(session, global_session.clone(), 1) };
        Ok(Self {
            raw,
            session,
            _global_session: global_session,
            info,
            max_thread_groups,
        })
    }

    /// Returns the information of the device.
    ///
    /// # Examples
    ///
    /// This example prints the graphics API and the adapter of a device. It
    /// needs a Vulkan driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::GlobalSession;
    /// use slang_rhi::{Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::new(GlobalSession::new()?))?;
    /// let info = device.info();
    /// println!("{} on {} ({:?})", info.api_name(), info.adapter_name(), info.device_type());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn info(&self) -> DeviceInfo {
        self.info.clone()
    }

    /// Returns the session of `slang-compiler` of the device, through which a
    /// program loads and links the modules of its shader programs.
    ///
    /// The session has one target, the code that the device runs, such as
    /// SPIR-V for Vulkan, and was created from the global session of the
    /// options of the device.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    #[must_use]
    pub fn slang_session(&self) -> &Session {
        &self.session
    }

    pub(crate) fn as_ptr(&self) -> *mut IDevice {
        self.raw.as_ptr()
    }

    pub(crate) fn as_non_null(&self) -> NonNull<IDevice> {
        self.raw.as_non_null()
    }

    pub(crate) fn max_thread_groups(&self) -> [u32; 3] {
        self.max_thread_groups
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::c_char;

    use super::{Adapter, AdapterType, DeviceInfo, DeviceType, name_from_array};
    use crate::error::Operation;

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn the_value_types_of_devices_are_send_and_sync() {
        assert_send_sync::<Adapter>();
        assert_send_sync::<DeviceInfo>();
        assert_send_sync::<DeviceType>();
        assert_send_sync::<AdapterType>();
    }

    fn array(text: &[u8]) -> [c_char; 128] {
        let mut name = [0; 128];
        for (slot, byte) in name.iter_mut().zip(text) {
            *slot = c_char::from_ne_bytes([*byte]);
        }
        name
    }

    #[test]
    fn name_from_array_stops_at_the_first_nul_and_replaces_invalid_bytes() {
        assert_eq!(name_from_array(&array(b"llvmpipe\0junk")), "llvmpipe");
        assert_eq!(
            name_from_array(&array(b"bad \xff name")),
            "bad \u{fffd} name"
        );
        assert_eq!(name_from_array(&array(&[b'x'; 128])), "x".repeat(128));
    }

    #[test]
    fn device_types_convert_to_the_values_of_the_header_and_back() {
        for device_type in [
            DeviceType::Default,
            DeviceType::D3d11,
            DeviceType::D3d12,
            DeviceType::Vulkan,
            DeviceType::Metal,
            DeviceType::Cpu,
            DeviceType::Cuda,
            DeviceType::Wgpu,
        ] {
            assert_eq!(
                DeviceType::from_raw(device_type.to_raw(), Operation::CreateDevice).expect("known"),
                device_type
            );
        }
        let unknown = DeviceType::from_raw(slang_rhi_sys::DeviceType(99), Operation::CreateDevice)
            .expect_err("unknown");
        assert_eq!(unknown.kind(), crate::ErrorKind::Unrecognized);
        let unknown = AdapterType::from_raw(slang_rhi_sys::AdapterType(99)).expect_err("unknown");
        assert_eq!(unknown.kind(), crate::ErrorKind::Unrecognized);
    }
}
