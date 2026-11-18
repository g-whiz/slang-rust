use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};

use slang_rhi_sys::{
    Binding, Binding_anon0, BindingType, BufferRange, IBuffer, IDevice, IResource, IShaderObject,
};

use crate::buffer::{Buffer, BufferUsage};
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::lookup::ElementLayout;
use crate::owned::Owned;

/// The position of a parameter in a shader object.
///
/// A shader object holds the values of the parameters of a shader program.
/// Its uniform data is a block of bytes, and its resources, such as buffers,
/// sit in binding ranges, each of which holds an array of bindings. An offset
/// names a byte position in the uniform data, a binding range, and a position
/// in the array of that range, and it records how many bytes of uniform data
/// and how many binding ranges its parameter has. Get one from
/// [`ShaderObject::offset_of`], and pass it to [`ShaderObject::set_data`] or
/// [`ShaderObject::set_buffer`].
///
/// Only [`ShaderObject::offset_of`] creates an offset, and only the position
/// in the array of bindings can change, through
/// [`ShaderOffset::with_binding_array_index`], so that an offset always
/// describes a parameter that the layout of the program declares.
///
/// # Examples
///
/// This example binds a buffer to the third element of an array of buffers
/// that a kernel declares as `RWStructuredBuffer<float> values[4];`:
///
/// ```
/// use slang_rhi::{Buffer, ShaderObject};
///
/// fn bind_third(root: &mut ShaderObject<'_>, buffer: &Buffer) -> slang_rhi::Result<()> {
///     if let Some(offset) = root.offset_of("values")? {
///         root.set_buffer(offset.with_binding_array_index(2), buffer)?;
///     }
///     Ok(())
/// }
/// # let _ = bind_third;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct ShaderOffset {
    uniform_offset: u32,
    uniform_size: u32,
    binding_range_index: u32,
    binding_range_count: u32,
    binding_array_index: u32,
}

impl ShaderOffset {
    pub(crate) fn new(
        uniform_offset: u32,
        uniform_size: u32,
        binding_range_index: u32,
        binding_range_count: u32,
    ) -> Self {
        Self {
            uniform_offset,
            uniform_size,
            binding_range_index,
            binding_range_count,
            binding_array_index: 0,
        }
    }

    /// Returns the byte offset of the parameter in the uniform data of the
    /// object.
    ///
    /// The offset is 0 for a parameter without uniform data, such as a
    /// buffer.
    ///
    /// # Examples
    ///
    /// See the example of [`ShaderOffset::uniform_size`].
    #[must_use]
    pub fn uniform_offset(&self) -> u32 {
        self.uniform_offset
    }

    /// Returns the number of bytes of uniform data of the parameter, which is
    /// 0 for a parameter that holds only resources, such as a buffer, a
    /// `ConstantBuffer`, or a `ParameterBlock`.
    ///
    /// # Examples
    ///
    /// This example writes a value only when it fits the uniform data of the
    /// parameter:
    ///
    /// ```
    /// use slang_rhi::{ShaderObject, ShaderOffset};
    ///
    /// fn write(root: &mut ShaderObject<'_>, offset: ShaderOffset, bytes: &[u8]) -> slang_rhi::Result<bool> {
    ///     if usize::try_from(offset.uniform_size()).is_ok_and(|size| bytes.len() <= size) {
    ///         println!("writing at the byte {}", offset.uniform_offset());
    ///         root.set_data(offset, bytes)?;
    ///         return Ok(true);
    ///     }
    ///     Ok(false)
    /// }
    /// # let _ = write;
    /// ```
    #[must_use]
    pub fn uniform_size(&self) -> u32 {
        self.uniform_size
    }

    /// Returns the index of the first binding range of the parameter.
    ///
    /// # Examples
    ///
    /// See the example of [`ShaderOffset::with_binding_array_index`].
    #[must_use]
    pub fn binding_range_index(&self) -> u32 {
        self.binding_range_index
    }

    /// Returns the position in the array of bindings of the binding range,
    /// which is 0 for an offset that [`ShaderObject::offset_of`] returned.
    ///
    /// # Examples
    ///
    /// See the example of [`ShaderOffset::with_binding_array_index`].
    #[must_use]
    pub fn binding_array_index(&self) -> u32 {
        self.binding_array_index
    }

    /// Returns a copy at the position `index` in the array of bindings of the
    /// binding range, for a parameter that is an array of resources.
    ///
    /// # Examples
    ///
    /// ```
    /// use slang_rhi::ShaderObject;
    ///
    /// fn last(root: &ShaderObject<'_>) -> slang_rhi::Result<()> {
    ///     if let Some(offset) = root.offset_of("values")? {
    ///         let last = offset.with_binding_array_index(3);
    ///         assert_eq!(last.binding_range_index(), offset.binding_range_index());
    ///         assert_eq!(last.binding_array_index(), 3);
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = last;
    /// ```
    #[must_use]
    pub fn with_binding_array_index(self, index: u32) -> Self {
        Self {
            binding_array_index: index,
            ..self
        }
    }

    fn to_raw(self) -> slang_rhi_sys::ShaderOffset {
        slang_rhi_sys::ShaderOffset {
            uniformOffset: self.uniform_offset,
            bindingRangeIndex: self.binding_range_index,
            bindingArrayIndex: self.binding_array_index,
        }
    }
}

/// What a binding range takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RangeKind {
    /// A raw buffer that shaders only read, such as a `StructuredBuffer`.
    ReadOnlyBuffer,
    /// A raw buffer that shaders read and write, such as a
    /// `RWStructuredBuffer`.
    MutableBuffer,
    /// A typed buffer, such as an `RWBuffer<float>`, which needs a buffer
    /// with a format.
    TypedBuffer,
    /// Anything else, such as a texture or a constant buffer.
    Other,
}

/// What the layout of a shader object says about one binding range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BindingRange {
    /// The number of bindings in the range, which Slang reports as negative
    /// for an array without a fixed size.
    pub(crate) binding_count: isize,
    /// What the range takes.
    pub(crate) kind: RangeKind,
}

/// Checks that `offset` names a parameter with uniform data, and that `len`
/// bytes at its uniform offset lie inside both the uniform data of the
/// parameter and the uniform data of `uniform_size` bytes of the object.
fn check_data(offset: ShaderOffset, len: usize, uniform_size: usize) -> Result<()> {
    if offset.uniform_size == 0 {
        return Err(Error::invalid(
            Operation::SetData,
            "the parameter has no uniform data, as a buffer, a ConstantBuffer, or a ParameterBlock has none",
        ));
    }
    if usize::try_from(offset.uniform_size).map_or(true, |size| len > size) {
        return Err(Error::invalid(
            Operation::SetData,
            format!(
                "{len} bytes do not fit the {} bytes of uniform data of the parameter",
                offset.uniform_size
            ),
        ));
    }
    let start = usize::try_from(offset.uniform_offset).ok();
    let end = start.and_then(|start| start.checked_add(len));
    match end {
        Some(end) if end <= uniform_size => Ok(()),
        _ => Err(Error::invalid(
            Operation::SetData,
            format!(
                "{len} bytes at the offset {} do not fit the {uniform_size} bytes of uniform data of the object",
                offset.uniform_offset
            ),
        )),
    }
}

/// Checks that `offset` names an existing binding of a range that takes a
/// raw buffer, and that a buffer with the uses `usage` suits the access of
/// that range, where `range` is the range at its binding range index, or
/// `None` when the object has no range there.
fn check_buffer_binding(
    offset: ShaderOffset,
    range: Option<BindingRange>,
    usage: BufferUsage,
) -> Result<()> {
    let invalid = |detail: String| Err(Error::invalid(Operation::SetBuffer, detail));
    if offset.binding_range_count == 0 {
        return invalid(String::from("the parameter has no binding range"));
    }
    let Some(range) = range else {
        return invalid(format!(
            "the object has no binding range {}",
            offset.binding_range_index
        ));
    };
    if isize::try_from(offset.binding_array_index)
        .map_or(true, |index| index >= range.binding_count)
    {
        return invalid(format!(
            "the binding range {} has no binding {}",
            offset.binding_range_index, offset.binding_array_index
        ));
    }
    match range.kind {
        RangeKind::MutableBuffer if !usage.contains(BufferUsage::UNORDERED_ACCESS) => invalid(
            String::from("a buffer that the kernel writes needs the use UNORDERED_ACCESS"),
        ),
        RangeKind::ReadOnlyBuffer
            if !usage.intersects(BufferUsage::SHADER_RESOURCE | BufferUsage::UNORDERED_ACCESS) =>
        {
            invalid(String::from(
                "a buffer that the kernel reads needs the use SHADER_RESOURCE or UNORDERED_ACCESS",
            ))
        }
        RangeKind::MutableBuffer | RangeKind::ReadOnlyBuffer => Ok(()),
        RangeKind::TypedBuffer => invalid(format!(
            "the binding range {} is a typed buffer, which needs a buffer with a format, and this crate creates no buffer with a format",
            offset.binding_range_index
        )),
        RangeKind::Other => invalid(format!(
            "the binding range {} does not take a buffer",
            offset.binding_range_index
        )),
    }
}

/// Returns the binding of the whole of a buffer, which names the buffer as
/// its one resource.
fn buffer_binding(buffer: NonNull<IBuffer>) -> Binding {
    Binding {
        r#type: BindingType::Buffer,
        resource: buffer.as_ptr().cast::<IResource>(),
        resource2: ptr::null_mut(),
        anon0: Binding_anon0 {
            bufferRange: BufferRange {
                offset: 0,
                size: u64::MAX,
            },
        },
    }
}

/// A shader object of a bound compute pipeline, which holds the values of the
/// parameters of its program.
///
/// [`ComputePass::bind_pipeline`](crate::ComputePass::bind_pipeline) returns
/// the root shader object, which holds the global parameters of the program,
/// and [`ShaderObject::entry_point`] returns the shader object of one entry
/// point, which holds its `uniform` parameters. A shader object borrows the
/// pass, or the root object, that it came from, so it cannot outlive them,
/// and a pass cannot dispatch or bind another pipeline while it lives.
///
/// A shader object is neither [`Send`] nor [`Sync`], because slang-rhi
/// documents no rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of the [crate documentation](crate).
///
/// A shader object cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::ShaderObject<'static>>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::ShaderObject<'static>>();
/// ```
#[derive(Debug)]
pub struct ShaderObject<'p> {
    raw: NonNull<IShaderObject>,
    _owned: Option<Owned<IShaderObject>>,
    device: NonNull<IDevice>,
    borrow: PhantomData<&'p mut ()>,
}

impl ShaderObject<'_> {
    /// Wraps a shader object that its owner lends.
    ///
    /// # Safety
    ///
    /// `raw` points to a live shader object that stays alive and is not used
    /// in any other way for the lifetime of the returned value, which the
    /// caller ties to a mutable borrow of the owner. `device` is the device
    /// of the object. Breaking either is undefined behaviour.
    pub(crate) unsafe fn borrowed(raw: NonNull<IShaderObject>, device: NonNull<IDevice>) -> Self {
        Self {
            raw,
            _owned: None,
            device,
            borrow: PhantomData,
        }
    }

    /// Finds a parameter of the top level of the object by its name, and
    /// returns its offset, or `None` when the object has no such parameter.
    ///
    /// For the root object, the parameters are the global parameters of the
    /// program, which a Slang module declares outside every function, such as
    /// `uniform float factor;` and `RWStructuredBuffer<float> values;`. For
    /// the object of an entry point, they are the `uniform` parameters of that
    /// function. A field of a structure parameter and an element of an array
    /// are not found by name, and an offset of an array starts at its first
    /// element.
    ///
    /// The fields of a `ConstantBuffer` or a `ParameterBlock` parameter live
    /// in an object of their own, which this crate does not reach, so such a
    /// parameter can be found but not set.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns an error of kind [`ErrorKind::Failed`](crate::ErrorKind::Failed)
    /// when slang-rhi returns no layout for the object.
    pub fn offset_of(&self, name: &str) -> Result<Option<ShaderOffset>> {
        Ok(self.element_layout(Operation::FindOffset)?.find_field(name))
    }

    /// Writes `data` into the uniform data of the object at the uniform
    /// offset of `offset`.
    ///
    /// The bytes are the value of the parameter in the layout that Slang
    /// gives it, which for a `float` is the four bytes of
    /// [`f32::to_le_bytes`] on every platform that this crate supports. They
    /// start at the start of the parameter and fit its uniform data.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the parameter has no uniform
    /// data, such as a buffer, a `ConstantBuffer`, or a `ParameterBlock`,
    /// when the bytes are more than the uniform data of the parameter, or
    /// when they end past the uniform data of the object, such as for an
    /// offset of another object. Returns an error of kind
    /// [`ErrorKind::Failed`](crate::ErrorKind::Failed) when slang-rhi
    /// returns no layout for the object, and the error that slang-rhi reports
    /// for the write.
    pub fn set_data(&mut self, offset: ShaderOffset, data: &[u8]) -> Result<()> {
        let uniform_size = self.element_layout(Operation::SetData)?.uniform_size();
        self.write_data(offset, data, uniform_size)
    }

    /// Binds the whole of `buffer` at `offset`.
    ///
    /// The shader object keeps its own reference to the buffer, so the
    /// buffer stays alive while the shader object or a command buffer that
    /// uses it needs it.
    ///
    /// The parameter is a raw buffer, such as a `StructuredBuffer`, a
    /// `RWStructuredBuffer`, or a `ByteAddressBuffer`. A buffer that the
    /// kernel writes needs the use
    /// [`BufferUsage::UNORDERED_ACCESS`](crate::BufferUsage::UNORDERED_ACCESS),
    /// and one that it only reads needs
    /// [`BufferUsage::SHADER_RESOURCE`](crate::BufferUsage::SHADER_RESOURCE)
    /// or [`BufferUsage::UNORDERED_ACCESS`](crate::BufferUsage::UNORDERED_ACCESS).
    /// A typed buffer, such as an `RWBuffer<float>`, needs a buffer with a
    /// format, which this crate does not create.
    ///
    /// Bind a buffer to every buffer parameter that the kernel uses before
    /// the dispatch. A parameter left without a buffer works only on a device
    /// whose Vulkan driver offers the feature `nullDescriptor`, which reads
    /// zeros and drops writes, and this crate does not check it.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the buffer belongs to another
    /// device, when the parameter has no binding range, when the object has
    /// no binding range at the binding range index of `offset` or that range
    /// has no binding at its array index, when the range does not take a raw
    /// buffer, or when the buffer lacks the use that the range needs. Returns
    /// an error of kind [`ErrorKind::Failed`](crate::ErrorKind::Failed) when
    /// slang-rhi returns no layout for the object, and the error that
    /// slang-rhi reports for the binding.
    pub fn set_buffer(&mut self, offset: ShaderOffset, buffer: &Buffer) -> Result<()> {
        let range = self
            .element_layout(Operation::SetBuffer)?
            .binding_range(offset.binding_range_index);
        self.bind_buffer(
            offset,
            range,
            buffer.as_non_null(),
            buffer.device_ptr(),
            buffer.usage(),
        )
    }

    /// Returns the shader object of the entry point at `index` of the
    /// program of the bound pipeline, which holds the `uniform` parameters of
    /// that entry point.
    ///
    /// # Examples
    ///
    /// This example sets the `uniform float scale` parameter of the first
    /// entry point of a program:
    ///
    /// ```
    /// use slang_rhi::ShaderObject;
    ///
    /// fn set_scale(root: &mut ShaderObject<'_>, scale: f32) -> slang_rhi::Result<()> {
    ///     let mut entry = root.entry_point(0)?;
    ///     if let Some(offset) = entry.offset_of("scale")? {
    ///         entry.set_data(offset, &scale.to_le_bytes())?;
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = set_scale;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument)
    /// when the object has no entry point at `index`, and the error that
    /// slang-rhi reports for the lookup.
    pub fn entry_point(&mut self, index: usize) -> Result<ShaderObject<'_>> {
        let raw = self.raw.as_ptr();
        let count = guard::call("IShaderObject::getEntryPointCount", || {
            // SAFETY: the invariant of `ShaderObject` keeps `raw` live, and
            // the mutable borrow of `self` keeps every other use of it out.
            unsafe { IShaderObject::getEntryPointCount(raw) }
        });
        let position = u32::try_from(index)
            .ok()
            .filter(|position| *position < count)
            .ok_or_else(|| {
                Error::invalid(
                    Operation::GetEntryPoint,
                    format!("the object has {count} entry points and none at {index}"),
                )
            })?;
        let mut out: *mut IShaderObject = ptr::null_mut();
        let result = guard::call("IShaderObject::getEntryPoint", || {
            // SAFETY: `raw` is live, `position` is below the count that the
            // object reported, and `out` is a live out-pointer.
            unsafe { IShaderObject::getEntryPoint(raw, position, &raw mut out) }
        });
        error::check(Operation::GetEntryPoint, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the object of an
        // entry point with one reference added for the caller.
        let owned =
            unsafe { Owned::adopt(out) }.ok_or_else(|| Error::null(Operation::GetEntryPoint))?;
        Ok(ShaderObject {
            raw: owned.as_non_null(),
            _owned: Some(owned),
            device: self.device,
            borrow: PhantomData,
        })
    }

    fn element_layout(&self, operation: Operation) -> Result<ElementLayout> {
        let raw = self.raw.as_ptr();
        let layout = guard::call("IShaderObject::getElementTypeLayout", || {
            // SAFETY: the invariant of `ShaderObject` keeps `raw` live.
            unsafe { IShaderObject::getElementTypeLayout(raw) }
        });
        let layout = NonNull::new(layout).ok_or_else(|| Error::null(operation))?;
        // SAFETY: the object holds the layout of its program, which keeps the
        // Slang program that owns this type layout alive, and the returned
        // value lives only inside a method of a borrow of the object.
        Ok(unsafe { ElementLayout::new(layout) })
    }

    /// Writes `data` after the check against `uniform_size`, the size of the
    /// uniform data that the layout of the object reports.
    fn write_data(&mut self, offset: ShaderOffset, data: &[u8], uniform_size: usize) -> Result<()> {
        check_data(offset, data.len(), uniform_size)?;
        let raw = self.raw.as_ptr();
        let offset = offset.to_raw();
        let result = guard::call("IShaderObject::setData", || {
            // SAFETY: `raw` is live, `offset` is a live offset, and the
            // pointer and length describe `data`, which lie inside the
            // uniform data of the object by the check above and which
            // slang-rhi copies during the call.
            unsafe {
                IShaderObject::setData(
                    raw,
                    &raw const offset,
                    data.as_ptr().cast::<c_void>(),
                    data.len(),
                )
            }
        });
        error::check(Operation::SetData, result, None)
    }

    /// Binds the buffer `buffer` of the device `buffer_device` with the uses
    /// `usage` after the checks against `range`, the binding range that the
    /// layout of the object reports at the binding range index of `offset`.
    /// The caller keeps its own reference to the buffer through the call.
    fn bind_buffer(
        &mut self,
        offset: ShaderOffset,
        range: Option<BindingRange>,
        buffer: NonNull<IBuffer>,
        buffer_device: NonNull<IDevice>,
        usage: BufferUsage,
    ) -> Result<()> {
        if buffer_device != self.device {
            return Err(Error::invalid(
                Operation::SetBuffer,
                "the buffer belongs to another device",
            ));
        }
        check_buffer_binding(offset, range, usage)?;
        let raw = self.raw.as_ptr();
        let offset = offset.to_raw();
        let binding = buffer_binding(buffer);
        let result = guard::call("IShaderObject::setBinding", || {
            // SAFETY: `raw` is live, `offset` names an existing binding of a
            // range that takes a raw buffer by the checks above, the buffer
            // has the use that the access of the range needs, and `binding`
            // names the live buffer, whose interface derives from the
            // resource interface as its first and only base, so the cast
            // pointer is a valid resource pointer. The caller keeps its own
            // reference to the buffer through the call, and slang-rhi adds
            // its own.
            unsafe { IShaderObject::setBinding(raw, &raw const offset, &raw const binding) }
        });
        error::check(Operation::SetBuffer, result, None)
    }
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use slang_rhi_sys::{BindingType, IDevice};

    use super::{
        BindingRange, RangeKind, ShaderObject, ShaderOffset, buffer_binding, check_buffer_binding,
        check_data,
    };
    use crate::ErrorKind;
    use crate::buffer::BufferUsage;
    use crate::error::Operation;
    use crate::fake::{Event, FakeBuffer, FakeShaderObject, Log};

    /// The offset of a parameter with 4 bytes of uniform data at `uniform`.
    fn uniform(uniform: u32) -> ShaderOffset {
        ShaderOffset::new(uniform, 4, 0, 0)
    }

    /// The offset of the binding `element` of a buffer parameter whose one
    /// binding range is `range`.
    fn binding(range: u32, element: u32) -> ShaderOffset {
        ShaderOffset::new(0, 0, range, 1).with_binding_array_index(element)
    }

    fn buffer_range(binding_count: isize) -> BindingRange {
        BindingRange {
            binding_count,
            kind: RangeKind::MutableBuffer,
        }
    }

    const WRITABLE: BufferUsage = BufferUsage::UNORDERED_ACCESS;

    fn device(address: usize) -> NonNull<IDevice> {
        NonNull::new(std::ptr::without_provenance_mut(address)).expect("non-zero")
    }

    #[test]
    fn check_data_accepts_data_inside_the_parameter_and_the_object() {
        check_data(uniform(12), 4, 16).expect("it fits");
        check_data(uniform(12), 2, 16).expect("a prefix fits");
        check_data(uniform(0), 0, 4).expect("nothing fits");
        check_data(ShaderOffset::new(0, 16, 0, 0), 16, 16).expect("a vector fits");
    }

    #[test]
    fn check_data_rejects_a_parameter_without_uniform_data_and_data_past_either_end() {
        let cases = [
            (ShaderOffset::new(0, 0, 0, 1), 0, 16),
            (ShaderOffset::new(0, 0, 0, 1), 4, 16),
            (uniform(0), 5, 16),
            (uniform(13), 4, 16),
            (
                ShaderOffset::new(u32::MAX, u32::MAX, 0, 0),
                usize::MAX,
                usize::MAX,
            ),
        ];
        for (offset, len, size) in cases {
            let error = check_data(offset, len, size).expect_err("it does not fit");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{offset:?} {len}");
            assert_eq!(error.operation(), Operation::SetData);
            assert_eq!(error.code(), None);
        }
    }

    #[test]
    fn check_buffer_binding_accepts_an_existing_binding_of_a_raw_buffer_with_the_right_use() {
        check_buffer_binding(binding(1, 3), Some(buffer_range(4)), WRITABLE).expect("it exists");
        for usage in [BufferUsage::SHADER_RESOURCE, BufferUsage::UNORDERED_ACCESS] {
            let range = BindingRange {
                binding_count: 1,
                kind: RangeKind::ReadOnlyBuffer,
            };
            check_buffer_binding(binding(0, 0), Some(range), usage).expect("it can be read");
        }
    }

    #[test]
    fn check_buffer_binding_rejects_a_missing_range_a_missing_binding_and_another_type() {
        let range = |kind| {
            Some(BindingRange {
                binding_count: 1,
                kind,
            })
        };
        let cases = [
            (
                ShaderOffset::new(0, 4, 0, 0),
                Some(buffer_range(1)),
                WRITABLE,
            ),
            (binding(5, 0), None, WRITABLE),
            (binding(0, 4), Some(buffer_range(4)), WRITABLE),
            (binding(0, 0), Some(buffer_range(-1)), WRITABLE),
            (binding(0, 0), range(RangeKind::Other), WRITABLE),
            (binding(0, 0), range(RangeKind::TypedBuffer), WRITABLE),
            (
                binding(0, 0),
                range(RangeKind::MutableBuffer),
                BufferUsage::SHADER_RESOURCE,
            ),
            (
                binding(0, 0),
                range(RangeKind::ReadOnlyBuffer),
                BufferUsage::CONSTANT_BUFFER | BufferUsage::COPY_SOURCE,
            ),
        ];
        for (offset, range, usage) in cases {
            let error = check_buffer_binding(offset, range, usage).expect_err("it is refused");
            assert_eq!(
                error.kind(),
                ErrorKind::InvalidArgument,
                "{offset:?} {range:?} {usage:?}"
            );
            assert_eq!(error.operation(), Operation::SetBuffer);
        }
    }

    #[test]
    fn a_typed_buffer_range_is_refused_with_the_reason() {
        let range = Some(BindingRange {
            binding_count: 1,
            kind: RangeKind::TypedBuffer,
        });
        let error = check_buffer_binding(binding(0, 0), range, WRITABLE).expect_err("typed");
        assert!(error.to_string().contains("format"), "{error}");
    }

    #[test]
    fn buffer_binding_names_the_whole_buffer_as_its_one_resource() {
        let log = Log::new();
        let raw = FakeBuffer::create(&log, "buffer");
        let binding = buffer_binding(raw);
        assert_eq!(binding.r#type, BindingType::Buffer);
        assert_eq!(binding.resource.cast::<()>(), raw.as_ptr().cast::<()>());
        assert!(binding.resource2.is_null());
        // SAFETY: `buffer_binding` writes the field `bufferRange` of the
        // union, so reading it reads initialized memory.
        let range = unsafe { binding.anon0.bufferRange };
        assert_eq!((range.offset, range.size), (0, u64::MAX));
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeBuffer::release_raw(raw) };
    }

    #[test]
    fn write_data_passes_the_offset_and_bytes_and_refuses_before_the_call() {
        let log = Log::new();
        let raw = FakeShaderObject::create(&log, "root");
        // SAFETY: the fake object lives until the test releases it below,
        // and nothing else uses it while the wrapper lives.
        let mut object = unsafe { ShaderObject::borrowed(raw, device(8)) };
        object
            .write_data(uniform(4), &[1, 2, 3, 4], 8)
            .expect("the data fits");
        let error = object
            .write_data(uniform(6), &[1, 2, 3, 4], 8)
            .expect_err("the data does not fit");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        drop(object);
        assert_eq!(
            log.events(),
            [Event::set_data("root", (4, 0, 0), &[1, 2, 3, 4])]
        );
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeShaderObject::release_raw(raw) };
    }

    #[test]
    fn bind_buffer_passes_a_binding_while_the_caller_keeps_its_reference() {
        let log = Log::new();
        let root = FakeShaderObject::create(&log, "root");
        let buffer = FakeBuffer::create(&log, "buffer");
        // SAFETY: the fake object lives until the test releases it below,
        // and nothing else uses it while the wrapper lives.
        let mut object = unsafe { ShaderObject::borrowed(root, device(8)) };
        object
            .bind_buffer(
                binding(1, 2),
                Some(buffer_range(3)),
                buffer,
                device(8),
                WRITABLE,
            )
            .expect("the binding exists");
        drop(object);
        assert_eq!(
            log.events(),
            [
                Event::add_ref("buffer", 2),
                Event::set_binding("root", (0, 1, 2), "buffer", 2),
            ]
        );
        // SAFETY: the test owns the reference of `create` of each fake and
        // releases each once. The fake object releases the reference that it
        // added to the buffer when it is freed.
        unsafe { FakeShaderObject::release_raw(root) };
        // SAFETY: as above.
        unsafe { FakeBuffer::release_raw(buffer) };
        assert_eq!(
            log.events()[2..],
            [
                Event::release("root", 0),
                Event::freed("root"),
                Event::release("buffer", 1),
                Event::release("buffer", 0),
                Event::freed("buffer"),
            ]
        );
    }

    #[test]
    fn bind_buffer_refuses_a_buffer_of_another_device_and_a_bad_offset_before_the_call() {
        let log = Log::new();
        let root = FakeShaderObject::create(&log, "root");
        let buffer = FakeBuffer::create(&log, "buffer");
        // SAFETY: the fake object lives until the test releases it below,
        // and nothing else uses it while the wrapper lives.
        let mut object = unsafe { ShaderObject::borrowed(root, device(8)) };
        let other = object
            .bind_buffer(
                binding(0, 0),
                Some(buffer_range(1)),
                buffer,
                device(16),
                WRITABLE,
            )
            .expect_err("another device");
        assert_eq!(other.kind(), ErrorKind::InvalidArgument);
        let past = object
            .bind_buffer(binding(7, 0), None, buffer, device(8), WRITABLE)
            .expect_err("no such range");
        assert_eq!(past.kind(), ErrorKind::InvalidArgument);
        drop(object);
        assert_eq!(log.events(), []);
        // SAFETY: the test owns the reference of `create` of each fake and
        // releases each once.
        unsafe { FakeShaderObject::release_raw(root) };
        // SAFETY: as above.
        unsafe { FakeBuffer::release_raw(buffer) };
    }

    #[test]
    fn entry_point_owns_a_reference_until_it_is_dropped_and_checks_the_index() {
        let log = Log::new();
        let entry = FakeShaderObject::create(&log, "entry");
        let root = FakeShaderObject::create(&log, "root");
        FakeShaderObject::add_entry_point(root, entry);
        // SAFETY: the fake object lives until the test releases it below,
        // and nothing else uses it while the wrapper lives.
        let mut object = unsafe { ShaderObject::borrowed(root, device(8)) };
        let missing = object.entry_point(1).expect_err("one entry point only");
        assert_eq!(missing.kind(), ErrorKind::InvalidArgument);
        assert_eq!(missing.operation(), Operation::GetEntryPoint);
        let mut first = object.entry_point(0).expect("the entry point exists");
        first
            .write_data(uniform(0), &[9], 4)
            .expect("the data fits");
        drop(first);
        drop(object);
        assert_eq!(
            log.events(),
            [
                Event::add_ref("entry", 2),
                Event::add_ref("entry", 3),
                Event::set_data("entry", (0, 0, 0), &[9]),
                Event::release("entry", 2),
            ]
        );
        // SAFETY: the test owns the reference of `create` of each fake and
        // releases each once. The root releases the reference that it holds
        // to its entry point when it is freed.
        unsafe { FakeShaderObject::release_raw(root) };
        // SAFETY: as above.
        unsafe { FakeShaderObject::release_raw(entry) };
        assert_eq!(
            log.events()[4..],
            [
                Event::release("root", 0),
                Event::freed("root"),
                Event::release("entry", 1),
                Event::release("entry", 0),
                Event::freed("entry"),
            ]
        );
    }
}
