use std::ffi::c_void;
use std::ptr::{self, NonNull};

use slang_rhi_sys::{IBuffer, IDevice};

use crate::device::Device;
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::owned::Owned;

bitflags::bitflags! {
    /// The ways in which a buffer is used, which a caller combines with `|`.
    ///
    /// slang-rhi creates the buffer with the access that these uses need, and
    /// the crate checks each use of a buffer against them. A buffer needs at
    /// least one use. A buffer that a kernel writes needs
    /// [`BufferUsage::UNORDERED_ACCESS`], a buffer that a kernel only reads
    /// needs [`BufferUsage::SHADER_RESOURCE`] or
    /// [`BufferUsage::UNORDERED_ACCESS`], and a buffer that
    /// [`Device::read_buffer`] reads needs [`BufferUsage::COPY_SOURCE`].
    /// Initial data needs no use of its own.
    ///
    /// # Examples
    ///
    /// ```
    /// use slang_rhi::BufferUsage;
    ///
    /// let usage = BufferUsage::UNORDERED_ACCESS | BufferUsage::COPY_SOURCE;
    /// assert!(usage.contains(BufferUsage::COPY_SOURCE));
    /// ```
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct BufferUsage: u32 {
        /// The buffer holds vertices of a draw.
        const VERTEX_BUFFER = 1;
        /// The buffer holds indices of a draw.
        const INDEX_BUFFER = 1 << 1;
        /// The buffer holds the uniform data of a constant buffer. slang-rhi
        /// places such a buffer in memory that the program can write.
        const CONSTANT_BUFFER = 1 << 2;
        /// A shader reads the buffer.
        const SHADER_RESOURCE = 1 << 3;
        /// A shader reads and writes the buffer, such as a
        /// `RWStructuredBuffer` of a kernel.
        const UNORDERED_ACCESS = 1 << 4;
        /// The buffer holds the arguments of an indirect dispatch or draw.
        const INDIRECT_ARGUMENT = 1 << 5;
        /// The buffer is the source of a copy, such as that of
        /// [`Device::read_buffer`].
        const COPY_SOURCE = 1 << 6;
        /// The buffer is the destination of a copy.
        const COPY_DESTINATION = 1 << 7;
    }
}

/// Where the memory of a buffer lives.
///
/// The enumeration is non-exhaustive, so a later version can add memory
/// types, and a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`BufferDesc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MemoryType {
    /// Memory of the device, which shaders read and write fastest.
    DeviceLocal,
    /// Memory that the program writes and the device reads.
    Upload,
    /// Memory that the device writes and the program reads.
    ReadBack,
}

impl MemoryType {
    fn to_raw(self) -> slang_rhi_sys::MemoryType {
        match self {
            Self::DeviceLocal => slang_rhi_sys::MemoryType::DeviceLocal,
            Self::Upload => slang_rhi_sys::MemoryType::Upload,
            Self::ReadBack => slang_rhi_sys::MemoryType::ReadBack,
        }
    }
}

/// The description of a buffer that [`Device::create_buffer`] creates.
///
/// # Examples
///
/// This example describes a buffer of four `float` values that a kernel
/// writes and the program reads back:
///
/// ```
/// use slang_rhi::{BufferDesc, BufferUsage, MemoryType};
///
/// let desc = BufferDesc::new(16, BufferUsage::UNORDERED_ACCESS | BufferUsage::COPY_SOURCE)
///     .with_element_size(4)
///     .with_memory(MemoryType::DeviceLocal);
/// # let _ = desc;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BufferDesc {
    size: u64,
    usage: BufferUsage,
    element_size: u32,
    memory: MemoryType,
}

impl BufferDesc {
    /// Creates the description of a buffer of `size` bytes with the uses
    /// `usage`, an element size of 0, and the memory type
    /// [`MemoryType::DeviceLocal`].
    ///
    /// # Examples
    ///
    /// See the example of [`BufferDesc`].
    #[must_use]
    pub fn new(size: u64, usage: BufferUsage) -> Self {
        Self {
            size,
            usage,
            element_size: 0,
            memory: MemoryType::DeviceLocal,
        }
    }

    /// Returns a copy with the size in bytes of one element, which a
    /// structured buffer of a shader reads, such as 4 for a buffer of
    /// `float`.
    ///
    /// # Examples
    ///
    /// See the example of [`BufferDesc`].
    #[must_use]
    pub fn with_element_size(self, element_size: u32) -> Self {
        Self {
            element_size,
            ..self
        }
    }

    /// Returns a copy with the memory type `memory`.
    ///
    /// # Examples
    ///
    /// See the example of [`BufferDesc`].
    #[must_use]
    pub fn with_memory(self, memory: MemoryType) -> Self {
        Self { memory, ..self }
    }

    fn to_raw(self) -> Result<slang_rhi_sys::BufferDesc> {
        let usage = i32::try_from(self.usage.bits()).map_err(|_overflow| {
            Error::invalid(
                Operation::CreateBuffer,
                "the usage flags do not fit slang-rhi",
            )
        })?;
        Ok(slang_rhi_sys::BufferDesc {
            size: self.size,
            elementSize: self.element_size,
            memoryType: self.memory.to_raw(),
            usage: slang_rhi_sys::BufferUsage(usage),
            ..slang_rhi_sys::BufferDesc::default()
        })
    }
}

/// Checks that a description has a size and at least one use, and no bit of
/// a use that this crate does not define.
fn check_desc(desc: &BufferDesc) -> Result<()> {
    if desc.size == 0 {
        return Err(Error::invalid(
            Operation::CreateBuffer,
            "the size of the buffer is 0",
        ));
    }
    if desc.usage.is_empty() {
        return Err(Error::invalid(
            Operation::CreateBuffer,
            "the buffer has no use",
        ));
    }
    let unknown = desc.usage.bits() & !BufferUsage::all().bits();
    if unknown != 0 {
        return Err(Error::invalid(
            Operation::CreateBuffer,
            format!("the uses of the buffer hold the unknown bits {unknown:#x}"),
        ));
    }
    Ok(())
}

/// Checks that a buffer with the uses `usage` can be read back.
fn check_readable(usage: BufferUsage) -> Result<()> {
    if usage.contains(BufferUsage::COPY_SOURCE) {
        Ok(())
    } else {
        Err(Error::invalid(
            Operation::ReadBuffer,
            "the buffer does not have the use COPY_SOURCE",
        ))
    }
}

/// Checks that initial data of `len` bytes fills a buffer of `size` bytes.
fn check_initial(len: usize, size: u64) -> Result<()> {
    if u64::try_from(len).ok() == Some(size) {
        Ok(())
    } else {
        Err(Error::invalid(
            Operation::CreateBuffer,
            format!("the initial data has {len} bytes and the buffer {size}"),
        ))
    }
}

/// Checks that `len` bytes at `offset` lie inside a buffer of `size` bytes,
/// and returns the offset and the length as the types of slang-rhi.
fn check_read(offset: u64, len: usize, size: u64) -> Result<(usize, usize)> {
    let end = u64::try_from(len)
        .ok()
        .and_then(|len| offset.checked_add(len));
    let offset_native = usize::try_from(offset).ok();
    match (end, offset_native) {
        (Some(end), Some(offset)) if end <= size => Ok((offset, len)),
        _ => Err(Error::invalid(
            Operation::ReadBuffer,
            format!("{len} bytes at the offset {offset} do not fit the {size} bytes of the buffer"),
        )),
    }
}

/// A buffer of a device, a block of memory that kernels read and write.
///
/// Create one with [`Device::create_buffer`], bind it to a kernel with
/// [`ShaderObject::set_buffer`](crate::ShaderObject::set_buffer), and read it
/// back with [`Device::read_buffer`]. A buffer holds its [`Device`], so the
/// device lives at least as long as the buffer.
///
/// A buffer is neither [`Send`] nor [`Sync`], because slang-rhi documents no
/// rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of [`Device::create_buffer`].
///
/// A buffer cannot move to another thread, so this example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::Buffer>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::Buffer>();
/// ```
#[derive(Debug)]
pub struct Buffer {
    raw: Owned<IBuffer>,
    device: Device,
    size: u64,
    usage: BufferUsage,
}

impl Buffer {
    /// Returns the size of the buffer in bytes.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::create_buffer`].
    #[must_use]
    pub fn size(&self) -> u64 {
        self.size
    }

    pub(crate) fn as_non_null(&self) -> NonNull<IBuffer> {
        self.raw.as_non_null()
    }

    pub(crate) fn device_ptr(&self) -> NonNull<IDevice> {
        self.device.as_non_null()
    }

    pub(crate) fn usage(&self) -> BufferUsage {
        self.usage
    }
}

impl Device {
    /// Creates a buffer with the description `desc`, filled with `initial`
    /// when it is given.
    ///
    /// # Examples
    ///
    /// This example creates a buffer of four `float` values and reads it
    /// back. It needs a Vulkan driver at run time, so it is compiled but not
    /// run:
    ///
    /// ```no_run
    /// use slang_compiler::GlobalSession;
    /// use slang_rhi::{BufferDesc, BufferUsage, Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::new(GlobalSession::new()?))?;
    /// let input: Vec<u8> = [1.0_f32, 2.0, 3.0, 4.0]
    ///     .iter()
    ///     .flat_map(|value| value.to_le_bytes())
    ///     .collect();
    /// let desc = BufferDesc::new(16, BufferUsage::SHADER_RESOURCE | BufferUsage::COPY_SOURCE);
    /// let buffer = device.create_buffer(&desc, Some(&input))?;
    /// let mut output = vec![0_u8; 16];
    /// device.read_buffer(&buffer, 0, &mut output)?;
    /// assert_eq!(output, input);
    /// assert_eq!(buffer.size(), 16);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the size of the description is 0,
    /// when its uses are empty or hold a bit that [`BufferUsage`] does not
    /// define, or when `initial` is given and its length differs from the
    /// size of the description. Returns the error that slang-rhi reports for
    /// the creation.
    pub fn create_buffer(&self, desc: &BufferDesc, initial: Option<&[u8]>) -> Result<Buffer> {
        check_desc(desc)?;
        if let Some(initial) = initial {
            check_initial(initial.len(), desc.size)?;
        }
        let raw_desc = desc.to_raw()?;
        let device = self.as_ptr();
        let data = initial.map_or(ptr::null(), |initial| initial.as_ptr().cast::<c_void>());
        let mut out: *mut IBuffer = ptr::null_mut();
        let result = guard::call("IDevice::createBuffer", || {
            // SAFETY: `device` is live while `self` lives, `raw_desc` is a live
            // descriptor, `data` is null or points to exactly the size of the
            // descriptor in bytes by the check above, which slang-rhi copies
            // before it returns, and `out` is a live out-pointer.
            unsafe { IDevice::createBuffer(device, &raw const raw_desc, data, &raw mut out) }
        });
        error::check(Operation::CreateBuffer, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the buffer with
        // one reference added for the caller.
        let raw =
            unsafe { Owned::adopt(out) }.ok_or_else(|| Error::null(Operation::CreateBuffer))?;
        Ok(Buffer {
            raw,
            device: self.clone(),
            size: desc.size,
            usage: desc.usage,
        })
    }

    /// Reads `out.len()` bytes of `buffer`, starting `offset` bytes into it,
    /// into `out`.
    ///
    /// The call waits until the device has finished the copy. The buffer
    /// needs the use [`BufferUsage::COPY_SOURCE`].
    ///
    /// # Examples
    ///
    /// See the example of [`Device::create_buffer`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the buffer belongs to another
    /// device, when it does not have the use [`BufferUsage::COPY_SOURCE`], or
    /// when the range ends past the end of the buffer, and the error that
    /// slang-rhi reports for the read.
    pub fn read_buffer(&self, buffer: &Buffer, offset: u64, out: &mut [u8]) -> Result<()> {
        if buffer.device_ptr() != self.as_non_null() {
            return Err(Error::invalid(
                Operation::ReadBuffer,
                "the buffer belongs to another device",
            ));
        }
        check_readable(buffer.usage)?;
        let (offset, size) = check_read(offset, out.len(), buffer.size)?;
        let device = self.as_ptr();
        let raw = buffer.raw.as_ptr();
        let destination = out.as_mut_ptr().cast::<c_void>();
        let result = guard::call("IDevice::readBuffer", || {
            // SAFETY: `device` and `raw` are live while `self` and `buffer`
            // live, the buffer belongs to the device, the range lies inside
            // the buffer by the check above, and `destination` points to the
            // `size` writable bytes of `out`, which no other reference
            // reaches during the call.
            unsafe { IDevice::readBuffer(device, raw, offset, size, destination) }
        });
        error::check(Operation::ReadBuffer, result, None)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        BufferDesc, BufferUsage, MemoryType, check_desc, check_initial, check_read, check_readable,
    };
    use crate::error::Operation;
    use crate::{ErrorKind, ShaderOffset};

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn the_value_types_of_buffers_and_offsets_are_send_and_sync() {
        assert_send_sync::<BufferDesc>();
        assert_send_sync::<BufferUsage>();
        assert_send_sync::<MemoryType>();
        assert_send_sync::<ShaderOffset>();
    }

    #[test]
    fn check_initial_accepts_only_the_exact_size() {
        check_initial(16, 16).expect("it fills the buffer");
        for (len, size) in [(15, 16), (17, 16), (0, 1)] {
            let error = check_initial(len, size).expect_err("it does not fill the buffer");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert_eq!(error.operation(), Operation::CreateBuffer);
        }
    }

    #[test]
    fn check_desc_accepts_a_size_and_any_combination_of_defined_uses() {
        for usage in [
            BufferUsage::COPY_SOURCE,
            BufferUsage::all(),
            BufferUsage::UNORDERED_ACCESS | BufferUsage::COPY_SOURCE,
        ] {
            check_desc(&BufferDesc::new(4, usage)).expect("the description is valid");
        }
    }

    #[test]
    fn check_desc_rejects_a_size_of_zero_no_use_and_an_unknown_bit() {
        for desc in [
            BufferDesc::new(0, BufferUsage::UNORDERED_ACCESS),
            BufferDesc::new(4, BufferUsage::empty()),
            BufferDesc::new(4, BufferUsage::from_bits_retain(1 << 8)),
            BufferDesc::new(
                4,
                BufferUsage::UNORDERED_ACCESS | BufferUsage::from_bits_retain(1 << 31),
            ),
        ] {
            let error = check_desc(&desc).expect_err("the description is refused");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{desc:?}");
            assert_eq!(error.operation(), Operation::CreateBuffer);
        }
    }

    #[test]
    fn check_readable_requires_the_use_copy_source() {
        check_readable(BufferUsage::COPY_SOURCE | BufferUsage::UNORDERED_ACCESS)
            .expect("it can be copied");
        let error = check_readable(BufferUsage::UNORDERED_ACCESS | BufferUsage::COPY_DESTINATION)
            .expect_err("it cannot be copied");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::ReadBuffer);
    }

    #[test]
    fn check_read_accepts_a_range_inside_the_buffer() {
        assert_eq!(check_read(0, 16, 16).expect("whole"), (0, 16));
        assert_eq!(check_read(12, 4, 16).expect("tail"), (12, 4));
        assert_eq!(check_read(16, 0, 16).expect("empty at the end"), (16, 0));
    }

    #[test]
    fn check_read_rejects_a_range_past_the_end_and_one_whose_end_overflows() {
        for (offset, len, size) in [(13, 4, 16), (0, 17, 16), (u64::MAX, 1, 16), (17, 0, 16)] {
            let error = check_read(offset, len, size).expect_err("it is refused");
            assert_eq!(
                error.kind(),
                ErrorKind::InvalidArgument,
                "{offset} {len} {size}"
            );
            assert_eq!(error.operation(), Operation::ReadBuffer);
        }
    }

    #[test]
    fn buffer_desc_converts_each_field_and_keeps_the_defaults_of_the_header() {
        let raw = BufferDesc::new(64, BufferUsage::UNORDERED_ACCESS | BufferUsage::COPY_SOURCE)
            .with_element_size(4)
            .with_memory(MemoryType::ReadBack)
            .to_raw()
            .expect("the flags fit");
        let default = slang_rhi_sys::BufferDesc::default();
        assert_eq!(raw.size, 64);
        assert_eq!(raw.elementSize, 4);
        assert_eq!(raw.memoryType, slang_rhi_sys::MemoryType::ReadBack);
        assert_eq!(
            raw.usage,
            slang_rhi_sys::BufferUsage(
                slang_rhi_sys::BufferUsage::UnorderedAccess.0
                    | slang_rhi_sys::BufferUsage::CopySource.0
            )
        );
        assert_eq!(raw.structType, default.structType);
        assert_eq!(raw.defaultState, default.defaultState);
        assert_eq!(raw.format, default.format);
        assert!(raw.label.is_null());
        assert!(raw.next.is_null());
    }

    #[test]
    fn each_usage_flag_has_the_value_of_the_header() {
        let pairs = [
            (
                BufferUsage::VERTEX_BUFFER,
                slang_rhi_sys::BufferUsage::VertexBuffer,
            ),
            (
                BufferUsage::INDEX_BUFFER,
                slang_rhi_sys::BufferUsage::IndexBuffer,
            ),
            (
                BufferUsage::CONSTANT_BUFFER,
                slang_rhi_sys::BufferUsage::ConstantBuffer,
            ),
            (
                BufferUsage::SHADER_RESOURCE,
                slang_rhi_sys::BufferUsage::ShaderResource,
            ),
            (
                BufferUsage::UNORDERED_ACCESS,
                slang_rhi_sys::BufferUsage::UnorderedAccess,
            ),
            (
                BufferUsage::INDIRECT_ARGUMENT,
                slang_rhi_sys::BufferUsage::IndirectArgument,
            ),
            (
                BufferUsage::COPY_SOURCE,
                slang_rhi_sys::BufferUsage::CopySource,
            ),
            (
                BufferUsage::COPY_DESTINATION,
                slang_rhi_sys::BufferUsage::CopyDestination,
            ),
        ];
        for (flag, raw) in pairs {
            assert_eq!(i32::try_from(flag.bits()).ok(), Some(raw.0), "{flag:?}");
        }
        assert_eq!(BufferUsage::all().bits().count_ones(), 8);
    }
}
