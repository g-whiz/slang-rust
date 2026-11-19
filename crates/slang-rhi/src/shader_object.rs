use std::cell::RefCell;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::{self, NonNull};

use slang_rhi_sys::{
    Binding, Binding_anon0, BindingType, BufferRange, IBuffer, IDevice, IResource, ISampler,
    IShaderObject, ITexture, ITextureView,
};

use crate::buffer::{Buffer, BufferUsage};
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::lookup::ElementLayout;
use crate::owned::Owned;
use crate::sampler::Sampler;
use crate::texture::{Texture, TextureUsage};

/// The record of the textures that `set_texture` bound on the shader objects
/// of one render pass, which the pass compares with its targets before each
/// draw.
pub(crate) type TextureRecord = RefCell<Vec<NonNull<ITexture>>>;

/// The position of a parameter in a shader object.
///
/// A shader object holds the values of the parameters of a shader program.
/// Its uniform data is a block of bytes, and its resources, such as buffers,
/// sit in binding ranges, each of which holds an array of bindings. An offset
/// names a byte position in the uniform data, a binding range, and a position
/// in the array of that range, and it records how many bytes of uniform data
/// and how many binding ranges its parameter has. Get one from
/// [`ShaderObject::offset_of`], and pass it to [`ShaderObject::set_data`],
/// [`ShaderObject::set_buffer`], or [`ShaderObject::object`].
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
    /// The objects of a `ConstantBuffer`, a `cbuffer`, or a `ParameterBlock`,
    /// which hold the fields of the parameter.
    SubObject,
    /// A texture that shaders only read, such as a `Texture2D`, of this
    /// type.
    ReadOnlyTexture(TextureType),
    /// A texture that shaders write, such as a `RWTexture2D`.
    MutableTexture,
    /// A sampler, such as a `SamplerState`, or with `comparison` a
    /// `SamplerComparisonState`.
    Sampler {
        /// Whether the range takes comparison samplers.
        comparison: bool,
    },
    /// A texture combined with a sampler, such as a `Sampler2D`.
    CombinedTextureSampler,
    /// Anything else, such as a push-constant buffer.
    Other,
}

/// What the reflection of a texture parameter says about the textures that
/// it takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TextureType {
    /// Whether the texture is two-dimensional, without array layers, several
    /// samples, or any other flag of its shape.
    pub(crate) plain_2d: bool,
    /// Whether the elements that a shader reads from the texture are
    /// floating-point numbers.
    pub(crate) float: bool,
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

/// Returns the range of `offset` after the checks that the parameter has
/// exactly one binding range, that the object has a range at its binding
/// range index, and that the array index is below the binding count of that
/// range, where `range` is the range at that index, or `None` when the object
/// has no range there.
fn existing_range(
    offset: ShaderOffset,
    range: Option<BindingRange>,
    operation: Operation,
) -> Result<BindingRange> {
    let invalid = |detail: String| Err(Error::invalid(operation, detail));
    if offset.binding_range_count == 0 {
        return invalid(String::from("the parameter has no binding range"));
    }
    if offset.binding_range_count > 1 {
        return invalid(format!(
            "the parameter is a structure of several resources, with {} binding ranges, and its fields cannot be addressed by name",
            offset.binding_range_count
        ));
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
    Ok(range)
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
    let range = existing_range(offset, range, Operation::SetBuffer)?;
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
        RangeKind::SubObject => invalid(format!(
            "the binding range {} holds the object of a ConstantBuffer or a ParameterBlock, not a buffer",
            offset.binding_range_index
        )),
        RangeKind::ReadOnlyTexture(_)
        | RangeKind::MutableTexture
        | RangeKind::Sampler { .. }
        | RangeKind::CombinedTextureSampler
        | RangeKind::Other => invalid(format!(
            "the binding range {} does not take a buffer",
            offset.binding_range_index
        )),
    }
}

/// Checks that `offset` names an existing binding of a range of the objects
/// of a `ConstantBuffer`, a `cbuffer`, or a `ParameterBlock`, where `range`
/// is the range at its binding range index, or `None` when the object has no
/// range there.
fn check_object(offset: ShaderOffset, range: Option<BindingRange>) -> Result<()> {
    let range = existing_range(offset, range, Operation::GetObject)?;
    match range.kind {
        RangeKind::SubObject => Ok(()),
        RangeKind::ReadOnlyBuffer
        | RangeKind::MutableBuffer
        | RangeKind::TypedBuffer
        | RangeKind::ReadOnlyTexture(_)
        | RangeKind::MutableTexture
        | RangeKind::Sampler { .. }
        | RangeKind::CombinedTextureSampler
        | RangeKind::Other => Err(Error::invalid(
            Operation::GetObject,
            format!(
                "the binding range {} is not the object of a ConstantBuffer or a ParameterBlock",
                offset.binding_range_index
            ),
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

/// Checks that `offset` names an existing binding of a range that takes a
/// texture that shaders only read, and that a texture with the uses `usage`
/// can be read by a shader, where `range` is the range at its binding range
/// index, or `None` when the object has no range there.
fn check_texture_binding(
    offset: ShaderOffset,
    range: Option<BindingRange>,
    usage: TextureUsage,
) -> Result<()> {
    let invalid = |detail: String| Err(Error::invalid(Operation::SetTexture, detail));
    let range = existing_range(offset, range, Operation::SetTexture)?;
    match range.kind {
        RangeKind::ReadOnlyTexture(texture) if !texture.plain_2d => invalid(format!(
            "the binding range {} is a texture of another shape than a two-dimensional texture without array layers or several samples, the only shape that this crate creates",
            offset.binding_range_index
        )),
        RangeKind::ReadOnlyTexture(texture) if !texture.float => invalid(format!(
            "the binding range {} is a texture whose elements are not floating-point numbers, while a shader reads every format that this crate creates as floating-point numbers",
            offset.binding_range_index
        )),
        RangeKind::ReadOnlyTexture(_) if usage.contains(TextureUsage::SHADER_RESOURCE) => Ok(()),
        RangeKind::ReadOnlyTexture(_) => invalid(String::from(
            "a texture that a shader reads needs the use SHADER_RESOURCE",
        )),
        RangeKind::MutableTexture => invalid(format!(
            "the binding range {} is a texture that a shader writes, which this crate does not bind",
            offset.binding_range_index
        )),
        RangeKind::CombinedTextureSampler => invalid(format!(
            "the binding range {} is a texture combined with a sampler, which this crate does not bind",
            offset.binding_range_index
        )),
        RangeKind::ReadOnlyBuffer
        | RangeKind::MutableBuffer
        | RangeKind::TypedBuffer
        | RangeKind::SubObject
        | RangeKind::Sampler { .. }
        | RangeKind::Other => invalid(format!(
            "the binding range {} does not take a texture",
            offset.binding_range_index
        )),
    }
}

/// Checks that `offset` names an existing binding of a range that takes a
/// sampler, where `range` is the range at its binding range index, or `None`
/// when the object has no range there.
fn check_sampler_binding(offset: ShaderOffset, range: Option<BindingRange>) -> Result<()> {
    let range = existing_range(offset, range, Operation::SetSampler)?;
    match range.kind {
        RangeKind::Sampler { comparison: false } => Ok(()),
        RangeKind::Sampler { comparison: true } => Err(Error::invalid(
            Operation::SetSampler,
            format!(
                "the binding range {} is a comparison sampler, which this crate does not create",
                offset.binding_range_index
            ),
        )),
        RangeKind::CombinedTextureSampler => Err(Error::invalid(
            Operation::SetSampler,
            format!(
                "the binding range {} is a texture combined with a sampler, which this crate does not bind",
                offset.binding_range_index
            ),
        )),
        RangeKind::ReadOnlyBuffer
        | RangeKind::MutableBuffer
        | RangeKind::TypedBuffer
        | RangeKind::SubObject
        | RangeKind::ReadOnlyTexture(_)
        | RangeKind::MutableTexture
        | RangeKind::Other => Err(Error::invalid(
            Operation::SetSampler,
            format!(
                "the binding range {} does not take a sampler",
                offset.binding_range_index
            ),
        )),
    }
}

/// Returns the binding of the view `view` of a texture, which names the view
/// as its one resource.
fn texture_binding(view: NonNull<ITextureView>) -> Binding {
    Binding {
        r#type: BindingType::Texture,
        resource: view.as_ptr().cast::<IResource>(),
        resource2: ptr::null_mut(),
        anon0: Binding_anon0 {
            bufferRange: BufferRange { offset: 0, size: 0 },
        },
    }
}

/// Returns the binding of the sampler `sampler`, which names the sampler as
/// its one resource.
fn sampler_binding(sampler: NonNull<ISampler>) -> Binding {
    Binding {
        r#type: BindingType::Sampler,
        resource: sampler.as_ptr().cast::<IResource>(),
        resource2: ptr::null_mut(),
        anon0: Binding_anon0 {
            bufferRange: BufferRange { offset: 0, size: 0 },
        },
    }
}

/// A shader object of a bound compute or render pipeline, which holds the
/// values of the parameters of its program.
///
/// [`ComputePass::bind_pipeline`](crate::ComputePass::bind_pipeline) and
/// [`RenderPass::bind_pipeline`](crate::RenderPass::bind_pipeline) return
/// the root shader object, which holds the global parameters of the program,
/// [`ShaderObject::entry_point`] returns the shader object of one entry
/// point, which holds its `uniform` parameters, and [`ShaderObject::object`]
/// returns the shader object of a `ConstantBuffer`, a `cbuffer`, or a
/// `ParameterBlock` parameter, which holds its fields. A shader object borrows
/// the pass, or the object, that it came from, so it cannot outlive them, and
/// a pass cannot dispatch or bind another pipeline while it lives.
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
    /// The record of the textures bound in the render pass of the object, or
    /// `None` for an object of a compute pass.
    textures: Option<&'p TextureRecord>,
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
    /// of the object, and `textures` the record of the render pass of the
    /// object, if it has one. Breaking either is undefined behaviour.
    pub(crate) unsafe fn borrowed(
        raw: NonNull<IShaderObject>,
        device: NonNull<IDevice>,
        textures: Option<&TextureRecord>,
    ) -> ShaderObject<'_> {
        ShaderObject {
            raw,
            _owned: None,
            device,
            textures,
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
    /// The fields of a `ConstantBuffer`, a `cbuffer`, or a `ParameterBlock`
    /// parameter live in an object of their own. Find the parameter here,
    /// get its object with [`ShaderObject::object`], and find and set its
    /// fields on that object.
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
    /// In a render pass, the crate does not refuse a buffer that a draw both
    /// reads as a vertex or index buffer and writes through a read-write
    /// parameter, and the contents that such a draw reads from the buffer
    /// are undefined.
    ///
    /// Bind a buffer to every buffer parameter that the kernel uses before
    /// the dispatch. A parameter left without a buffer reads zeros and drops
    /// writes on Direct3D 12, where slang-rhi binds a null descriptor, and on
    /// Vulkan, where it works only on a device whose driver offers the feature
    /// `nullDescriptor`, which this crate does not check.
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
    /// device, when the parameter has no binding range, when it is a
    /// structure of several resources, whose fields cannot be addressed by
    /// name, when the object has
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

    /// Binds the default view of `texture`, which covers the whole texture,
    /// at `offset`.
    ///
    /// The shader object keeps its own reference to the view, so the texture
    /// stays alive while the shader object or a command buffer that uses it
    /// needs it. The parameter is a texture that shaders only read, such as a
    /// `Texture2D`, and the texture needs the use
    /// [`TextureUsage::SHADER_RESOURCE`](crate::TextureUsage::SHADER_RESOURCE).
    /// A texture that a shader writes, such as a `RWTexture2D`, and a texture
    /// combined with a sampler, such as a `Sampler2D`, are refused. So is a
    /// parameter of another shape than the two-dimensional textures without
    /// array layers or several samples that this crate creates, such as a
    /// `Texture3D`, a `TextureCube`, a `Texture2DArray`, or a `Texture2DMS`,
    /// and a parameter whose elements are not floating-point numbers, such as
    /// a `Texture2D<uint4>`, because a shader reads every format that this
    /// crate creates as floating-point numbers.
    ///
    /// In a render pass, a draw refuses a texture that this method bound
    /// during the pass and that is also a target of the pass, because no
    /// graphics API lets a pass read the texture that it draws into.
    ///
    /// # Examples
    ///
    /// This example binds a texture and a sampler that a fragment shader
    /// declares as `Texture2D colours;` and `SamplerState nearest;`:
    ///
    /// ```
    /// use slang_rhi::{Sampler, ShaderObject, Texture};
    ///
    /// fn bind(root: &mut ShaderObject<'_>, texture: &Texture, sampler: &Sampler) -> slang_rhi::Result<()> {
    ///     if let Some(offset) = root.offset_of("colours")? {
    ///         root.set_texture(offset, texture)?;
    ///     }
    ///     if let Some(offset) = root.offset_of("nearest")? {
    ///         root.set_sampler(offset, sampler)?;
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = bind;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the texture belongs to another
    /// device, when the parameter has no binding range or is a structure of
    /// several resources, when the object has no binding range at the binding
    /// range index of `offset` or that range has no binding at its array
    /// index, when the range does not take a texture that shaders only read,
    /// when it takes a texture of another shape or with elements that are
    /// not floating-point numbers, or when the texture lacks the use
    /// [`TextureUsage::SHADER_RESOURCE`](crate::TextureUsage::SHADER_RESOURCE).
    /// Returns an error of kind [`ErrorKind::Failed`](crate::ErrorKind::Failed)
    /// when slang-rhi returns no layout for the object, and the error that
    /// slang-rhi reports for the binding.
    pub fn set_texture(&mut self, offset: ShaderOffset, texture: &Texture) -> Result<()> {
        let range = self
            .element_layout(Operation::SetTexture)?
            .binding_range(offset.binding_range_index);
        self.bind_texture(
            offset,
            range,
            BoundTexture {
                texture: texture.as_non_null(),
                view: texture.view(),
                device: texture.device_ptr(),
                usage: texture.usage(),
            },
        )
    }

    /// Binds `sampler` at `offset`.
    ///
    /// The shader object keeps its own reference to the sampler. The
    /// parameter is a sampler, such as a `SamplerState`. A texture combined
    /// with a sampler, such as a `Sampler2D`, is refused, and so is a
    /// comparison sampler, a `SamplerComparisonState`, because this crate
    /// creates no sampler that compares.
    ///
    /// # Examples
    ///
    /// See the example of [`ShaderObject::set_texture`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the sampler belongs to another
    /// device, when the parameter has no binding range or is a structure of
    /// several resources, when the object has no binding range at the binding
    /// range index of `offset` or that range has no binding at its array
    /// index, or when the range does not take a sampler or takes a comparison
    /// sampler. Returns an error of
    /// kind [`ErrorKind::Failed`](crate::ErrorKind::Failed) when slang-rhi
    /// returns no layout for the object, and the error that slang-rhi reports
    /// for the binding.
    pub fn set_sampler(&mut self, offset: ShaderOffset, sampler: &Sampler) -> Result<()> {
        let range = self
            .element_layout(Operation::SetSampler)?
            .binding_range(offset.binding_range_index);
        self.bind_sampler(offset, range, sampler.as_non_null(), sampler.device_ptr())
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
            textures: self.textures,
            borrow: PhantomData,
        })
    }

    /// Returns the shader object of the `ConstantBuffer`, `cbuffer`, or
    /// `ParameterBlock` parameter at `offset`, which holds the fields of the
    /// parameter.
    ///
    /// Find the fields of the parameter with [`ShaderObject::offset_of`] on
    /// the returned object, and set them with its [`ShaderObject::set_data`]
    /// and [`ShaderObject::set_buffer`]. A parameter whose fields hold another
    /// `ConstantBuffer` or `ParameterBlock` is reached one object at a time.
    /// slang-rhi creates these objects when it binds the pipeline, so each
    /// one exists without a call that sets it.
    ///
    /// The elements of an array of such parameters sit in one binding range,
    /// and [`ShaderOffset::with_binding_array_index`] names the object of one
    /// element. On Vulkan, slang-rhi binds an array of `ConstantBuffer`
    /// parameters as if it had one element, so the values set on the object
    /// of any element after the first do not reach the kernel as set.
    ///
    /// A parameter that is a structure of several resources, such as a
    /// structure with a `ConstantBuffer` field and a buffer field, is
    /// refused, because its fields cannot be addressed by name.
    ///
    /// # Examples
    ///
    /// This example sets the field `value` of a parameter that a kernel
    /// declares as `ConstantBuffer<Params> constants;`:
    ///
    /// ```
    /// use slang_rhi::ShaderObject;
    ///
    /// fn set_value(root: &mut ShaderObject<'_>, value: [f32; 4]) -> slang_rhi::Result<()> {
    ///     if let Some(offset) = root.offset_of("constants")? {
    ///         let mut constants = root.object(offset)?;
    ///         if let Some(field) = constants.offset_of("value")? {
    ///             let bytes: Vec<u8> = value.iter().flat_map(|v| v.to_le_bytes()).collect();
    ///             constants.set_data(field, &bytes)?;
    ///         }
    ///     }
    ///     Ok(())
    /// }
    /// # let _ = set_value;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the parameter has no binding
    /// range, when it is a structure of several resources, when the object
    /// has no binding range at the binding range index of `offset` or that
    /// range has no binding at its array index, or when the range is not that
    /// of a `ConstantBuffer`, a `cbuffer`, or a `ParameterBlock`. Returns an
    /// error of kind [`ErrorKind::Failed`](crate::ErrorKind::Failed) when
    /// slang-rhi returns no layout for the object or no object for the
    /// parameter, and the error that slang-rhi reports for the lookup.
    ///
    /// slang-rhi returns no object only for a parameter whose element it has
    /// no layout for, because it creates every other object when it binds
    /// the pipeline, or fails the bind. With
    /// [`DeviceOptions::validation`](crate::DeviceOptions::validation) on,
    /// slang-rhi wraps the missing object in an object of its debug device,
    /// so this function returns that object instead of an error, and the
    /// next call on it crashes the process inside slang-rhi.
    pub fn object(&mut self, offset: ShaderOffset) -> Result<ShaderObject<'_>> {
        let range = self
            .element_layout(Operation::GetObject)?
            .binding_range(offset.binding_range_index);
        self.sub_object(offset, range)
    }

    /// Returns the object at `offset` after the checks against `range`, the
    /// binding range that the layout of the object reports at the binding
    /// range index of `offset`.
    fn sub_object(
        &mut self,
        offset: ShaderOffset,
        range: Option<BindingRange>,
    ) -> Result<ShaderObject<'_>> {
        check_object(offset, range)?;
        let raw = self.raw.as_ptr();
        let offset = offset.to_raw();
        let mut out: *mut IShaderObject = ptr::null_mut();
        let result = guard::call("IShaderObject::getObject", || {
            // SAFETY: the invariant of `ShaderObject` keeps `raw` live, the
            // mutable borrow of `self` keeps every other use of it out,
            // `offset` names an existing binding of a range of objects by
            // the checks above, so slang-rhi reads an entry of its array of
            // objects, and `out` is a live out-pointer.
            unsafe { IShaderObject::getObject(raw, &raw const offset, &raw mut out) }
        });
        error::check(Operation::GetObject, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the object with
        // one reference added for the caller, or null for an entry without
        // an object. Its debug device instead returns a live object of its
        // own that wraps the null entry, which no call through the bindings
        // can tell apart without a call into the null entry. When slang-rhi
        // binds the pipeline, it fills every entry of a range of objects
        // whose element layout it knows, or fails the bind, so the entry is
        // null only for an element without a layout in slang-rhi.
        let owned =
            unsafe { Owned::adopt(out) }.ok_or_else(|| Error::null(Operation::GetObject))?;
        Ok(ShaderObject {
            raw: owned.as_non_null(),
            _owned: Some(owned),
            device: self.device,
            textures: self.textures,
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

    /// Binds the default view of a texture after the checks against
    /// `range`, the binding range that the layout of the object reports at
    /// the binding range index of `offset`, and adds the texture to the
    /// record of the render pass of the object. The caller keeps its own
    /// reference to the texture and its view through the call.
    pub(crate) fn bind_texture(
        &mut self,
        offset: ShaderOffset,
        range: Option<BindingRange>,
        texture: BoundTexture,
    ) -> Result<()> {
        if texture.device != self.device {
            return Err(Error::invalid(
                Operation::SetTexture,
                "the texture belongs to another device",
            ));
        }
        check_texture_binding(offset, range, texture.usage)?;
        let raw = self.raw.as_ptr();
        let offset = offset.to_raw();
        let binding = texture_binding(texture.view);
        let result = guard::call("IShaderObject::setBinding", || {
            // SAFETY: `raw` is live, `offset` names an existing binding of a
            // range that takes a texture that shaders only read by the checks
            // above, the texture has the use of a shader resource, and
            // `binding` names the live default view of the texture, whose
            // interface derives from the resource interface as its first and
            // only base, so the cast pointer is a valid resource pointer. The
            // caller keeps its own reference to the view through the call, and
            // slang-rhi adds its own.
            unsafe { IShaderObject::setBinding(raw, &raw const offset, &raw const binding) }
        });
        error::check(Operation::SetTexture, result, None)?;
        if let Some(textures) = self.textures {
            textures.borrow_mut().push(texture.texture);
        }
        Ok(())
    }

    /// Binds the sampler `sampler` of the device `sampler_device` after the
    /// checks against `range`, the binding range that the layout of the
    /// object reports at the binding range index of `offset`. The caller
    /// keeps its own reference to the sampler through the call.
    fn bind_sampler(
        &mut self,
        offset: ShaderOffset,
        range: Option<BindingRange>,
        sampler: NonNull<ISampler>,
        sampler_device: NonNull<IDevice>,
    ) -> Result<()> {
        if sampler_device != self.device {
            return Err(Error::invalid(
                Operation::SetSampler,
                "the sampler belongs to another device",
            ));
        }
        check_sampler_binding(offset, range)?;
        let raw = self.raw.as_ptr();
        let offset = offset.to_raw();
        let binding = sampler_binding(sampler);
        let result = guard::call("IShaderObject::setBinding", || {
            // SAFETY: `raw` is live, `offset` names an existing binding of a
            // range that takes a sampler by the checks above, and `binding`
            // names the live sampler, whose interface derives from the
            // resource interface as its first and only base, so the cast
            // pointer is a valid resource pointer. The caller keeps its own
            // reference to the sampler through the call, and slang-rhi adds
            // its own.
            unsafe { IShaderObject::setBinding(raw, &raw const offset, &raw const binding) }
        });
        error::check(Operation::SetSampler, result, None)
    }
}

/// A texture that `set_texture` binds: its interface, its default view, its
/// device, and its uses.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BoundTexture {
    pub(crate) texture: NonNull<ITexture>,
    pub(crate) view: NonNull<ITextureView>,
    pub(crate) device: NonNull<IDevice>,
    pub(crate) usage: TextureUsage,
}

#[cfg(test)]
mod tests {
    use std::ptr::NonNull;

    use slang_rhi_sys::{BindingType, IDevice};

    use super::{
        BindingRange, BoundTexture, RangeKind, ShaderObject, ShaderOffset, TextureRecord,
        TextureType, buffer_binding, check_buffer_binding, check_data, check_object,
        check_sampler_binding, check_texture_binding, sampler_binding, texture_binding,
    };
    use crate::ErrorKind;
    use crate::buffer::BufferUsage;
    use crate::error::Operation;
    use crate::fake::{Event, FakeBuffer, FakeShaderObject, Log};
    use crate::texture::TextureUsage;

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
            (binding(0, 0), range(RangeKind::SubObject), WRITABLE),
            (
                ShaderOffset::new(0, 0, 0, 2),
                range(RangeKind::MutableBuffer),
                WRITABLE,
            ),
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
    fn a_structure_of_several_resources_is_refused_with_the_reason() {
        let offset = ShaderOffset::new(0, 0, 3, 2);
        let buffer = check_buffer_binding(offset, Some(buffer_range(1)), WRITABLE)
            .expect_err("two binding ranges");
        let object = check_object(offset, Some(object_range(1))).expect_err("two binding ranges");
        for error in [buffer, object] {
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert!(
                error.to_string().contains("structure of several resources"),
                "{error}"
            );
        }
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
        let mut object = unsafe { ShaderObject::borrowed(raw, device(8), None) };
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
        let mut object = unsafe { ShaderObject::borrowed(root, device(8), None) };
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
        let mut object = unsafe { ShaderObject::borrowed(root, device(8), None) };
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

    fn object_range(binding_count: isize) -> BindingRange {
        BindingRange {
            binding_count,
            kind: RangeKind::SubObject,
        }
    }

    #[test]
    fn check_object_accepts_an_existing_binding_of_a_range_of_objects() {
        check_object(binding(0, 0), Some(object_range(1))).expect("it exists");
        check_object(binding(2, 3), Some(object_range(4))).expect("the last element exists");
    }

    #[test]
    fn check_object_rejects_a_missing_range_a_missing_binding_and_every_other_kind() {
        let kind = |kind| {
            Some(BindingRange {
                binding_count: 1,
                kind,
            })
        };
        let cases = [
            (ShaderOffset::new(0, 4, 0, 0), Some(object_range(1))),
            (binding(5, 0), None),
            (binding(0, 1), Some(object_range(1))),
            (binding(0, 0), Some(object_range(-1))),
            (binding(0, 0), kind(RangeKind::ReadOnlyBuffer)),
            (binding(0, 0), kind(RangeKind::MutableBuffer)),
            (binding(0, 0), kind(RangeKind::TypedBuffer)),
            (binding(0, 0), kind(RangeKind::Other)),
            (ShaderOffset::new(0, 0, 0, 2), Some(object_range(1))),
        ];
        for (offset, range) in cases {
            let error = check_object(offset, range).expect_err("it is refused");
            assert_eq!(
                error.kind(),
                ErrorKind::InvalidArgument,
                "{offset:?} {range:?}"
            );
            assert_eq!(error.operation(), Operation::GetObject);
            assert_eq!(error.code(), None);
        }
    }

    #[test]
    fn sub_object_owns_a_reference_until_it_is_dropped_and_its_data_reaches_it() {
        let log = Log::new();
        let constants = FakeShaderObject::create(&log, "constants");
        let root = FakeShaderObject::create(&log, "root");
        FakeShaderObject::add_object(root, (1, 2), constants);
        // SAFETY: the fake object lives until the test releases it below,
        // and nothing else uses it while the wrapper lives.
        let mut object = unsafe { ShaderObject::borrowed(root, device(8), None) };
        let mut found = object
            .sub_object(binding(1, 2), Some(object_range(3)))
            .expect("the object exists");
        found
            .write_data(uniform(0), &[7, 7], 4)
            .expect("the data fits");
        drop(found);
        drop(object);
        assert_eq!(
            log.events(),
            [
                Event::add_ref("constants", 2),
                Event::add_ref("constants", 3),
                Event::set_data("constants", (0, 0, 0), &[7, 7]),
                Event::release("constants", 2),
            ]
        );
        // SAFETY: the test owns the reference of `create` of each fake and
        // releases each once. The root releases the reference that it holds
        // to the object of its parameter when it is freed.
        unsafe { FakeShaderObject::release_raw(root) };
        // SAFETY: as above.
        unsafe { FakeShaderObject::release_raw(constants) };
        assert_eq!(
            log.events()[4..],
            [
                Event::release("root", 0),
                Event::freed("root"),
                Event::release("constants", 1),
                Event::release("constants", 0),
                Event::freed("constants"),
            ]
        );
    }

    #[test]
    fn sub_object_turns_a_null_object_into_an_error_and_refuses_a_bad_offset_before_the_call() {
        let log = Log::new();
        let root = FakeShaderObject::create(&log, "root");
        // SAFETY: the fake object lives until the test releases it below,
        // and nothing else uses it while the wrapper lives.
        let mut object = unsafe { ShaderObject::borrowed(root, device(8), None) };
        let null = object
            .sub_object(binding(0, 0), Some(object_range(1)))
            .map(drop)
            .expect_err("the parameter has no object");
        assert_eq!(null.kind(), ErrorKind::Failed);
        assert_eq!(null.operation(), Operation::GetObject);
        let buffer = object
            .sub_object(binding(0, 0), Some(buffer_range(1)))
            .map(drop)
            .expect_err("the range takes a buffer");
        assert_eq!(buffer.kind(), ErrorKind::InvalidArgument);
        let past = object
            .sub_object(binding(0, 1), Some(object_range(1)))
            .map(drop)
            .expect_err("the range has one binding");
        assert_eq!(past.kind(), ErrorKind::InvalidArgument);
        drop(object);
        assert_eq!(log.events(), []);
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeShaderObject::release_raw(root) };
    }

    #[test]
    fn entry_point_owns_a_reference_until_it_is_dropped_and_checks_the_index() {
        let log = Log::new();
        let entry = FakeShaderObject::create(&log, "entry");
        let root = FakeShaderObject::create(&log, "root");
        FakeShaderObject::add_entry_point(root, entry);
        // SAFETY: the fake object lives until the test releases it below,
        // and nothing else uses it while the wrapper lives.
        let mut object = unsafe { ShaderObject::borrowed(root, device(8), None) };
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

    fn kind_range(kind: RangeKind) -> BindingRange {
        BindingRange {
            binding_count: 2,
            kind,
        }
    }

    const SAMPLED: TextureUsage = TextureUsage::SHADER_RESOURCE.union(TextureUsage::COPY_SOURCE);

    const PLAIN_2D: TextureType = TextureType {
        plain_2d: true,
        float: true,
    };

    #[test]
    fn check_texture_binding_accepts_a_read_only_texture_range_and_a_texture_that_a_shader_reads() {
        check_texture_binding(
            binding(0, 1),
            Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
            SAMPLED,
        )
        .expect("it exists");
    }

    #[test]
    fn check_texture_binding_rejects_every_other_range_and_a_texture_without_the_use() {
        let cases = [
            (
                binding(0, 0),
                Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
                TextureUsage::RENDER_TARGET,
            ),
            (
                binding(0, 2),
                Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
                SAMPLED,
            ),
            (binding(3, 0), None, SAMPLED),
            (
                ShaderOffset::new(0, 4, 0, 0),
                Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
                SAMPLED,
            ),
            (
                ShaderOffset::new(0, 0, 0, 2),
                Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
                SAMPLED,
            ),
            (
                binding(0, 0),
                Some(kind_range(RangeKind::MutableTexture)),
                SAMPLED,
            ),
            (
                binding(0, 0),
                Some(kind_range(RangeKind::CombinedTextureSampler)),
                SAMPLED,
            ),
            (
                binding(0, 0),
                Some(kind_range(RangeKind::Sampler { comparison: false })),
                SAMPLED,
            ),
            (
                binding(0, 0),
                Some(kind_range(RangeKind::ReadOnlyBuffer)),
                SAMPLED,
            ),
            (
                binding(0, 0),
                Some(kind_range(RangeKind::SubObject)),
                SAMPLED,
            ),
            (binding(0, 0), Some(kind_range(RangeKind::Other)), SAMPLED),
            (
                binding(0, 0),
                Some(kind_range(RangeKind::ReadOnlyTexture(TextureType {
                    plain_2d: false,
                    float: true,
                }))),
                SAMPLED,
            ),
            (
                binding(0, 0),
                Some(kind_range(RangeKind::ReadOnlyTexture(TextureType {
                    plain_2d: true,
                    float: false,
                }))),
                SAMPLED,
            ),
        ];
        for (offset, range, usage) in cases {
            let error = check_texture_binding(offset, range, usage).expect_err("it is refused");
            assert_eq!(
                error.kind(),
                ErrorKind::InvalidArgument,
                "{offset:?} {range:?}"
            );
            assert_eq!(error.operation(), Operation::SetTexture);
        }
    }

    #[test]
    fn check_sampler_binding_accepts_only_an_existing_binding_of_a_sampler_range() {
        check_sampler_binding(
            binding(1, 1),
            Some(kind_range(RangeKind::Sampler { comparison: false })),
        )
        .expect("it exists");
        for (offset, range) in [
            (
                binding(1, 2),
                Some(kind_range(RangeKind::Sampler { comparison: false })),
            ),
            (binding(1, 0), None),
            (
                binding(1, 0),
                Some(kind_range(RangeKind::CombinedTextureSampler)),
            ),
            (
                binding(1, 0),
                Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
            ),
            (binding(1, 0), Some(kind_range(RangeKind::MutableBuffer))),
            (
                binding(1, 0),
                Some(kind_range(RangeKind::Sampler { comparison: true })),
            ),
        ] {
            let error = check_sampler_binding(offset, range).expect_err("it is refused");
            assert_eq!(
                error.kind(),
                ErrorKind::InvalidArgument,
                "{offset:?} {range:?}"
            );
            assert_eq!(error.operation(), Operation::SetSampler);
        }
    }

    #[test]
    fn texture_and_sampler_bindings_name_their_one_resource() {
        let log = Log::new();
        let raw = FakeBuffer::create(&log, "resource");
        let texture = texture_binding(raw.cast());
        assert_eq!(texture.r#type, BindingType::Texture);
        assert_eq!(texture.resource.cast::<()>(), raw.as_ptr().cast::<()>());
        assert!(texture.resource2.is_null());
        let sampler = sampler_binding(raw.cast());
        assert_eq!(sampler.r#type, BindingType::Sampler);
        assert_eq!(sampler.resource.cast::<()>(), raw.as_ptr().cast::<()>());
        assert!(sampler.resource2.is_null());
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeBuffer::release_raw(raw) };
    }

    #[test]
    fn bind_texture_passes_the_view_and_records_the_texture_in_the_pass() {
        let log = Log::new();
        let root = FakeShaderObject::create(&log, "root");
        let view = FakeBuffer::create(&log, "view");
        let record = TextureRecord::default();
        let texture = BoundTexture {
            texture: NonNull::new(std::ptr::without_provenance_mut(512)).expect("non-zero"),
            view: view.cast(),
            device: device(8),
            usage: SAMPLED,
        };
        // SAFETY: the fake object lives until the test releases it below,
        // and nothing else uses it while the wrapper lives.
        let mut object = unsafe { ShaderObject::borrowed(root, device(8), Some(&record)) };
        object
            .bind_texture(
                binding(2, 0),
                Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
                texture,
            )
            .expect("the binding exists");
        let other = object
            .bind_texture(
                binding(2, 0),
                Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
                BoundTexture {
                    device: device(16),
                    ..texture
                },
            )
            .expect_err("another device");
        assert_eq!(other.kind(), ErrorKind::InvalidArgument);
        let sampler = object
            .bind_sampler(
                binding(3, 0),
                Some(kind_range(RangeKind::Sampler { comparison: false })),
                view.cast(),
                device(8),
            )
            .map(drop);
        sampler.expect("the sampler binding exists");
        let wrong = object
            .bind_sampler(
                binding(2, 0),
                Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
                view.cast(),
                device(8),
            )
            .expect_err("a texture range");
        assert_eq!(wrong.operation(), Operation::SetSampler);
        drop(object);
        assert_eq!(record.borrow().as_slice(), [texture.texture]);
        assert_eq!(
            log.events(),
            [
                Event::add_ref("view", 2),
                Event::set_binding("root", (0, 2, 0), "view", 2),
                Event::add_ref("view", 3),
                Event::set_binding("root", (0, 3, 0), "view", 3),
            ]
        );
        // SAFETY: the test owns the reference of `create` of each fake and
        // releases each once. The fake object releases the references that
        // it added to the view when it is freed.
        unsafe { FakeShaderObject::release_raw(root) };
        // SAFETY: as above.
        unsafe { FakeBuffer::release_raw(view) };
    }

    #[test]
    fn a_texture_bound_in_a_compute_pass_is_recorded_nowhere() {
        let log = Log::new();
        let root = FakeShaderObject::create(&log, "root");
        let view = FakeBuffer::create(&log, "view");
        // SAFETY: the fake object lives until the test releases it below,
        // and nothing else uses it while the wrapper lives.
        let mut object = unsafe { ShaderObject::borrowed(root, device(8), None) };
        object
            .bind_texture(
                binding(0, 0),
                Some(kind_range(RangeKind::ReadOnlyTexture(PLAIN_2D))),
                BoundTexture {
                    texture: NonNull::new(std::ptr::without_provenance_mut(512)).expect("non-zero"),
                    view: view.cast(),
                    device: device(8),
                    usage: SAMPLED,
                },
            )
            .expect("the binding exists");
        drop(object);
        // SAFETY: the test owns the reference of `create` of each fake and
        // releases each once.
        unsafe { FakeShaderObject::release_raw(root) };
        // SAFETY: as above.
        unsafe { FakeBuffer::release_raw(view) };
    }
}
