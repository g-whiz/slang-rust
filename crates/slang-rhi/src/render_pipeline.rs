use std::ffi::CString;
use std::ptr::{self, NonNull};

use slang_rhi_sys::{IDevice, IInputLayout, IRenderPipeline};

use crate::device::{Device, DeviceType};
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::owned::Owned;
use crate::program::{EntryPoints, ShaderProgram, diagnostics};
use crate::texture::Format;

/// The format of one vertex attribute in a vertex buffer.
///
/// Each format is one to four 32-bit numbers. The enumeration is
/// non-exhaustive, so a later version can add formats, and a `match` on it
/// needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`RenderPipelineDesc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum VertexFormat {
    /// One `float`.
    Float32,
    /// A `float2`.
    Float32x2,
    /// A `float3`.
    Float32x3,
    /// A `float4`.
    Float32x4,
    /// One `uint`.
    Uint32,
    /// A `uint2`.
    Uint32x2,
    /// A `uint3`.
    Uint32x3,
    /// A `uint4`.
    Uint32x4,
    /// One `int`.
    Sint32,
    /// An `int2`.
    Sint32x2,
    /// An `int3`.
    Sint32x3,
    /// An `int4`.
    Sint32x4,
}

impl VertexFormat {
    fn to_raw(self) -> slang_rhi_sys::Format {
        match self {
            Self::Float32 => slang_rhi_sys::Format::R32Float,
            Self::Float32x2 => slang_rhi_sys::Format::RG32Float,
            Self::Float32x3 => slang_rhi_sys::Format::RGB32Float,
            Self::Float32x4 => slang_rhi_sys::Format::RGBA32Float,
            Self::Uint32 => slang_rhi_sys::Format::R32Uint,
            Self::Uint32x2 => slang_rhi_sys::Format::RG32Uint,
            Self::Uint32x3 => slang_rhi_sys::Format::RGB32Uint,
            Self::Uint32x4 => slang_rhi_sys::Format::RGBA32Uint,
            Self::Sint32 => slang_rhi_sys::Format::R32Sint,
            Self::Sint32x2 => slang_rhi_sys::Format::RG32Sint,
            Self::Sint32x3 => slang_rhi_sys::Format::RGB32Sint,
            Self::Sint32x4 => slang_rhi_sys::Format::RGBA32Sint,
        }
    }

    /// Returns the kind of the numbers of this format.
    fn numbers(self) -> Numbers {
        match self {
            Self::Float32 | Self::Float32x2 | Self::Float32x3 | Self::Float32x4 => Numbers::Float,
            Self::Uint32 | Self::Uint32x2 | Self::Uint32x3 | Self::Uint32x4 => Numbers::Unsigned,
            Self::Sint32 | Self::Sint32x2 | Self::Sint32x3 | Self::Sint32x4 => Numbers::Signed,
        }
    }

    /// Returns the number of bytes of one attribute of this format.
    pub(crate) fn size(self) -> u32 {
        match self {
            Self::Float32 | Self::Uint32 | Self::Sint32 => 4,
            Self::Float32x2 | Self::Uint32x2 | Self::Sint32x2 => 8,
            Self::Float32x3 | Self::Uint32x3 | Self::Sint32x3 => 12,
            Self::Float32x4 | Self::Uint32x4 | Self::Sint32x4 => 16,
        }
    }
}

/// How often a vertex stream advances to its next element.
///
/// # Examples
///
/// See the example of [`RenderPipelineDesc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum StepMode {
    /// The stream advances once for each vertex.
    Vertex,
    /// The stream advances once for each instance.
    Instance,
}

/// One vertex stream of a render pipeline, a vertex buffer that holds one
/// attribute at the start of each element.
///
/// A stream feeds the input of the vertex entry point at its location, the
/// position that Slang gives the input, and names the semantic of that input,
/// which Direct3D 12 matches inputs by. Reflection reports both.
///
/// # Examples
///
/// See the example of [`RenderPipelineDesc`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct VertexStream {
    format: VertexFormat,
    stride: u32,
    step: StepMode,
    location: u32,
    semantic: Option<(String, u32)>,
}

impl VertexStream {
    /// Creates a stream of attributes of the format `format` for the input
    /// at `location`, with a stride of the size of one attribute, one
    /// element for each vertex, and no semantic.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipelineDesc`].
    #[must_use]
    pub fn new(format: VertexFormat, location: u32) -> Self {
        Self {
            format,
            stride: format.size(),
            step: StepMode::Vertex,
            location,
            semantic: None,
        }
    }

    /// Returns a copy whose elements are `stride` bytes apart.
    ///
    /// A stride of 0 gives every vertex, or every instance, the one
    /// attribute at the start of the buffer, so that a buffer of one
    /// attribute feeds a draw of any count.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipelineDesc`].
    #[must_use]
    pub fn with_stride(self, stride: u32) -> Self {
        Self { stride, ..self }
    }

    /// Returns a copy that advances as `step` says.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipelineDesc`].
    #[must_use]
    pub fn with_step(self, step: StepMode) -> Self {
        Self { step, ..self }
    }

    /// Returns a copy with the semantic `name` and `index` of the input, such
    /// as `POSITION` and 0 for an input that a shader declares as
    /// `float3 position : POSITION`.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipelineDesc`].
    #[must_use]
    pub fn with_semantic(self, name: impl Into<String>, index: u32) -> Self {
        Self {
            semantic: Some((name.into(), index)),
            ..self
        }
    }
}

/// How a render pipeline combines the colour that it draws with the colour of
/// the target.
///
/// # Examples
///
/// See the example of [`RenderPipelineDesc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BlendMode {
    /// The drawn colour replaces the colour of the target.
    Replace,
    /// The drawn colour is blended over the target by its alpha: the colour
    /// is the source times its alpha plus the target times one minus that
    /// alpha, and the alpha is the source alpha plus the target alpha times
    /// one minus the source alpha.
    Alpha,
}

/// Which triangles a render pipeline leaves out.
///
/// A triangle whose vertices run counter-clockwise in clip space, with the y
/// axis up, faces the front.
///
/// # Examples
///
/// See the example of [`RenderPipelineDesc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CullMode {
    /// Every triangle is drawn.
    None,
    /// Triangles that face the back are left out.
    Back,
}

/// The description of a render pipeline that
/// [`Device::create_render_pipeline`] creates, which draws lists of triangles
/// into one colour target and an optional depth target.
///
/// A pipeline draws counter-clockwise triangles as front faces, replaces the
/// colour of the target and draws every triangle unless the description
/// says otherwise, and with a depth format, keeps a fragment whose depth is
/// less than the depth of the target and writes its depth.
///
/// On Vulkan, slang-rhi clamps the depth of the parts of a primitive outside
/// the range from 0 to 1 to that range instead of clipping them, so they are
/// drawn, while Direct3D 12 clips them, so they are not drawn.
///
/// # Examples
///
/// This example describes a pipeline whose vertex entry point declares
/// `float2 position : POSITION` at location 0 and `float4 colour : COLOR` at
/// location 1, the second taken once for each instance, and that tests
/// depth:
///
/// ```
/// use slang_rhi::{
///     BlendMode, CullMode, Format, RenderPipelineDesc, StepMode, VertexFormat, VertexStream,
/// };
///
/// let desc = RenderPipelineDesc::new(Format::Rgba8Unorm)
///     .with_depth_format(Format::D32Float)
///     .with_blend(BlendMode::Alpha)
///     .with_cull_mode(CullMode::Back)
///     .with_vertex_streams([
///         VertexStream::new(VertexFormat::Float32x2, 0).with_semantic("POSITION", 0),
///         VertexStream::new(VertexFormat::Float32x4, 1)
///             .with_semantic("COLOR", 0)
///             .with_stride(16)
///             .with_step(StepMode::Instance),
///     ]);
/// # let _ = desc;
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct RenderPipelineDesc {
    colour_format: Format,
    depth_format: Option<Format>,
    blend: BlendMode,
    cull_mode: CullMode,
    vertex_streams: Vec<VertexStream>,
}

impl RenderPipelineDesc {
    /// Creates the description of a pipeline that draws into a colour target
    /// of the format `colour_format`, with no depth target, no blend, no
    /// culling, and no vertex stream.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipelineDesc`].
    #[must_use]
    pub fn new(colour_format: Format) -> Self {
        Self {
            colour_format,
            depth_format: None,
            blend: BlendMode::Replace,
            cull_mode: CullMode::None,
            vertex_streams: Vec::new(),
        }
    }

    /// Returns a copy that tests and writes the depth of a depth target of
    /// the format `format`, which is [`Format::D32Float`].
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipelineDesc`].
    #[must_use]
    pub fn with_depth_format(self, format: Format) -> Self {
        Self {
            depth_format: Some(format),
            ..self
        }
    }

    /// Returns a copy that combines colours as `blend` says.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipelineDesc`].
    #[must_use]
    pub fn with_blend(self, blend: BlendMode) -> Self {
        Self { blend, ..self }
    }

    /// Returns a copy that leaves out triangles as `cull_mode` says.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipelineDesc`].
    #[must_use]
    pub fn with_cull_mode(self, cull_mode: CullMode) -> Self {
        Self { cull_mode, ..self }
    }

    /// Returns a copy with the vertex streams `streams`, in the order of the
    /// vertex buffers that
    /// [`RenderPass::set_vertex_buffers`](crate::RenderPass::set_vertex_buffers)
    /// sets.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipelineDesc`].
    #[must_use]
    pub fn with_vertex_streams(self, streams: impl IntoIterator<Item = VertexStream>) -> Self {
        Self {
            vertex_streams: streams.into_iter().collect(),
            ..self
        }
    }
}

/// The kind of the 32-bit numbers of a vertex format or of a vertex input.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Numbers {
    /// Floating-point numbers, such as `float`.
    Float,
    /// Unsigned integers, such as `uint`.
    Unsigned,
    /// Signed integers, such as `int`.
    Signed,
}

impl Numbers {
    fn describe(self) -> &'static str {
        match self {
            Self::Float => "floating-point numbers",
            Self::Unsigned => "unsigned integers",
            Self::Signed => "signed integers",
        }
    }
}

/// What reflection reports of the type of one varying input of a vertex
/// entry point.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum InputType {
    /// A scalar or vector of 32-bit numbers of this kind.
    Numbers(Numbers),
    /// Any other type, such as a matrix, an array, or a scalar of another
    /// width, with its description.
    Other(String),
}

/// One varying input of a vertex entry point, which a vertex stream feeds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct VertexInput {
    /// The name of the parameter, with the names of the fields that lead to
    /// it for a field of a structure.
    pub(crate) name: String,
    /// The location that Slang gives the input.
    pub(crate) location: u32,
    /// The semantic name and index of the input, which reflection reports in
    /// capitals, or `None` for an input without a semantic.
    pub(crate) semantic: Option<(String, u32)>,
    /// The type of the input.
    pub(crate) ty: InputType,
}

/// The largest number of vertex streams of a render pipeline on any device,
/// the number of vertex buffers that the render state of slang-rhi holds.
///
/// A device can allow fewer, which
/// [`DeviceInfo::max_vertex_streams`](crate::DeviceInfo::max_vertex_streams)
/// reports, and [`Device::create_render_pipeline`] refuses more streams than
/// the smaller of the two.
///
/// # Examples
///
/// See the example of [`Device::create_render_pipeline`].
pub const MAX_VERTEX_STREAMS: u32 = 16;

/// The largest stride of a vertex stream that every device of both graphics
/// APIs allows, which Vulkan guarantees as the least `maxVertexInputBindingStride`
/// and Direct3D 12 sets as its limit.
const MAX_STRIDE: u32 = 2048;

/// The semantic name of an input element that fills a location that no
/// stream feeds.
const FILLER_SEMANTIC: &str = "SLANG_RHI_UNUSED";

/// What a draw needs to know of one vertex stream of a bound pipeline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct StreamLayout {
    /// The bytes between two elements.
    pub(crate) stride: u32,
    /// Whether the stream advances for each vertex or each instance.
    pub(crate) step: StepMode,
    /// The bytes of the one attribute at the start of each element.
    pub(crate) attribute_size: u32,
}

/// One input element of an input layout, with its semantic as a C string.
#[derive(Clone, Debug, PartialEq, Eq)]
struct ElementPlan {
    semantic: CString,
    semantic_index: u32,
    format: slang_rhi_sys::Format,
    slot: u32,
    /// Whether the element fills a location that no stream feeds.
    filler: bool,
}

/// The input layout that a description of a render pipeline gives: one
/// element for each location from 0 to the highest location of a stream, and
/// the streams.
#[derive(Clone, Debug, PartialEq, Eq)]
struct InputPlan {
    elements: Vec<ElementPlan>,
    streams: Vec<StreamLayout>,
}

/// Checks the formats of a description of a render pipeline.
fn check_formats(desc: &RenderPipelineDesc) -> Result<()> {
    if desc.colour_format.is_depth() {
        return Err(Error::invalid(
            Operation::CreateRenderPipeline,
            format!(
                "the colour target has the depth format {:?}",
                desc.colour_format
            ),
        ));
    }
    match desc.depth_format {
        Some(format) if format != Format::D32Float => Err(Error::invalid(
            Operation::CreateRenderPipeline,
            format!("the depth target has the format {format:?}, and not D32Float"),
        )),
        _ => Ok(()),
    }
}

/// Checks the stride and the semantic of the vertex stream `stream` at
/// `index`.
fn check_stream(index: usize, stream: &VertexStream) -> Result<()> {
    let invalid = |detail: String| Err(Error::invalid(Operation::CreateRenderPipeline, detail));
    if stream.stride > MAX_STRIDE {
        return invalid(format!(
            "the vertex stream {index} has the stride {}, above the largest stride of {MAX_STRIDE}",
            stream.stride
        ));
    }
    if !stream.stride.is_multiple_of(4) {
        return invalid(format!(
            "the vertex stream {index} has the stride {}, which is not a multiple of 4, the size of the 32-bit numbers of every vertex format",
            stream.stride
        ));
    }
    let Some((name, _)) = &stream.semantic else {
        return invalid(format!(
            "the vertex stream {index} at the location {} has no semantic, which Direct3D 12 matches inputs by",
            stream.location
        ));
    };
    if name.is_empty() || name.contains('\0') {
        return invalid(format!(
            "the semantic of the vertex stream {index} is empty or holds a NUL byte"
        ));
    }
    Ok(())
}

/// Checks the vertex streams of a description against the largest number of
/// vertex streams `max_streams` and of input elements `max_elements` of a
/// device, and returns the input layout that they give.
///
/// The Vulkan backend of slang-rhi gives each input element the location of
/// its index, so the layout lists one element for each location up to the
/// highest one, with a filler element for a location that no stream feeds,
/// which reads the first 4 bytes of each element of the first stream, inside
/// the bounds that a draw checks for that stream. The Direct3D 12 backend
/// matches each element by its semantic, so [`for_device`] leaves the filler
/// elements out there.
fn plan_inputs(streams: &[VertexStream], max_streams: u32, max_elements: u32) -> Result<InputPlan> {
    let invalid = |detail: String| Err(Error::invalid(Operation::CreateRenderPipeline, detail));
    let limit = max_streams.min(MAX_VERTEX_STREAMS);
    if u32::try_from(streams.len()).map_or(true, |count| count > limit) {
        return invalid(format!(
            "the pipeline has {} vertex streams, more than the {limit} that the device and slang-rhi allow",
            streams.len()
        ));
    }
    let mut highest = None;
    for (index, stream) in streams.iter().enumerate() {
        check_stream(index, stream)?;
        if streams[..index]
            .iter()
            .any(|other| other.location == stream.location)
        {
            return invalid(format!(
                "two vertex streams feed the location {}",
                stream.location
            ));
        }
        highest = highest.max(Some(stream.location));
    }
    let count = match highest {
        None => 0,
        Some(highest) => highest.checked_add(1).filter(|count| *count <= max_elements).ok_or_else(|| {
            Error::invalid(
                Operation::CreateRenderPipeline,
                format!(
                    "the vertex inputs up to the location {highest} need {} input elements, more than the {max_elements} that the device allows",
                    u64::from(highest) + 1
                ),
            )
        })?,
    };
    let mut elements = Vec::new();
    for location in 0..count {
        let found = streams
            .iter()
            .zip(0_u32..)
            .find(|(stream, _)| stream.location == location);
        let element = match found {
            Some((stream, slot)) => {
                let (name, index) = stream.semantic.clone().unwrap_or_default();
                ElementPlan {
                    semantic: CString::new(name).map_err(|_nul| {
                        Error::invalid(
                            Operation::CreateRenderPipeline,
                            "a semantic holds a NUL byte",
                        )
                    })?,
                    semantic_index: index,
                    format: stream.format.to_raw(),
                    slot,
                    filler: false,
                }
            }
            None => ElementPlan {
                semantic: CString::new(FILLER_SEMANTIC).map_err(|_nul| {
                    Error::invalid(
                        Operation::CreateRenderPipeline,
                        "the filler semantic holds a NUL byte",
                    )
                })?,
                semantic_index: location,
                format: slang_rhi_sys::Format::R32Float,
                slot: 0,
                filler: true,
            },
        };
        elements.push(element);
    }
    Ok(InputPlan {
        elements,
        streams: streams
            .iter()
            .map(|stream| StreamLayout {
                stride: stream.stride,
                step: stream.step,
                attribute_size: stream.format.size(),
            })
            .collect(),
    })
}

/// Returns the plan of the input layout for a device of the graphics API
/// `device_type`: without the filler elements on Direct3D 12, which matches
/// elements by semantic, so that the layout holds only the elements of the
/// streams there, and unchanged on every other API.
fn for_device(mut plan: InputPlan, device_type: DeviceType) -> InputPlan {
    if device_type == DeviceType::D3d12 {
        plan.elements.retain(|element| !element.filler);
    }
    plan
}

/// Checks the vertex streams of a description against the varying inputs
/// that the vertex entry point declares.
///
/// Each input needs a stream at its location whose format holds numbers of
/// the kind of the input, because Vulkan requires both, and whose semantic
/// is that of the input, ignoring case, when the input has one, because
/// Direct3D 12 matches each input to the stream of its semantic.
fn check_inputs(inputs: &[VertexInput], streams: &[VertexStream]) -> Result<()> {
    let invalid = |detail: String| Err(Error::invalid(Operation::CreateRenderPipeline, detail));
    for input in inputs {
        let Some(stream) = streams
            .iter()
            .find(|stream| stream.location == input.location)
        else {
            return invalid(format!(
                "the vertex entry point declares the input {} at the location {}, which no vertex stream feeds",
                input.name, input.location
            ));
        };
        let numbers = match &input.ty {
            InputType::Numbers(numbers) => *numbers,
            InputType::Other(description) => {
                return invalid(format!(
                    "the input {} at the location {} is {description}, which no vertex format feeds",
                    input.name, input.location
                ));
            }
        };
        if stream.format.numbers() != numbers {
            return invalid(format!(
                "the vertex stream at the location {} has the format {:?} of {}, and the input {} there takes {}",
                input.location,
                stream.format,
                stream.format.numbers().describe(),
                input.name,
                numbers.describe()
            ));
        }
        if let (Some((name, index)), Some((stream_name, stream_index))) =
            (&input.semantic, &stream.semantic)
            && (!name.eq_ignore_ascii_case(stream_name) || index != stream_index)
        {
            return invalid(format!(
                "the vertex stream at the location {} has the semantic {stream_name} with the index {stream_index}, and the input {} there has the semantic {name} with the index {index}, which Direct3D 12 matches inputs by",
                input.location, input.name
            ));
        }
    }
    Ok(())
}

/// Returns the raw input elements of a plan, which borrow its semantics.
fn raw_elements(plan: &InputPlan) -> Vec<slang_rhi_sys::InputElementDesc> {
    plan.elements
        .iter()
        .map(|element| slang_rhi_sys::InputElementDesc {
            semanticName: element.semantic.as_ptr(),
            semanticIndex: element.semantic_index,
            format: element.format,
            offset: 0,
            bufferSlotIndex: element.slot,
        })
        .collect()
}

/// Returns the raw vertex streams of a plan, with the step rate 1 for a
/// stream that steps for each instance, which Direct3D 12 needs for one
/// element for each instance, and 0 for one that steps for each vertex, which
/// it needs there.
fn raw_streams(plan: &InputPlan) -> Vec<slang_rhi_sys::VertexStreamDesc> {
    plan.streams
        .iter()
        .map(|stream| match stream.step {
            StepMode::Instance => slang_rhi_sys::VertexStreamDesc {
                stride: stream.stride,
                slotClass: slang_rhi_sys::InputSlotClass::PerInstance,
                instanceDataStepRate: 1,
            },
            StepMode::Vertex => slang_rhi_sys::VertexStreamDesc {
                stride: stream.stride,
                slotClass: slang_rhi_sys::InputSlotClass::PerVertex,
                instanceDataStepRate: 0,
            },
        })
        .collect()
}

/// Returns the raw colour target of a description.
fn colour_target(desc: &RenderPipelineDesc) -> slang_rhi_sys::ColorTargetDesc {
    let (color, alpha, enable) = match desc.blend {
        BlendMode::Replace => (
            slang_rhi_sys::AspectBlendDesc::default(),
            slang_rhi_sys::AspectBlendDesc::default(),
            false,
        ),
        BlendMode::Alpha => (
            slang_rhi_sys::AspectBlendDesc {
                srcFactor: slang_rhi_sys::BlendFactor::SrcAlpha,
                dstFactor: slang_rhi_sys::BlendFactor::InvSrcAlpha,
                op: slang_rhi_sys::BlendOp::Add,
            },
            slang_rhi_sys::AspectBlendDesc {
                srcFactor: slang_rhi_sys::BlendFactor::One,
                dstFactor: slang_rhi_sys::BlendFactor::InvSrcAlpha,
                op: slang_rhi_sys::BlendOp::Add,
            },
            true,
        ),
    };
    slang_rhi_sys::ColorTargetDesc {
        format: desc.colour_format.to_raw(),
        color,
        alpha,
        enableBlend: enable,
        writeMask: slang_rhi_sys::RenderTargetWriteMask::All,
        ..slang_rhi_sys::ColorTargetDesc::default()
    }
}

/// Returns the raw depth state of a description: the test `Less` with depth
/// writes for a depth format, and neither without one.
fn depth_state(desc: &RenderPipelineDesc) -> slang_rhi_sys::DepthStencilDesc {
    match desc.depth_format {
        Some(format) => slang_rhi_sys::DepthStencilDesc {
            format: format.to_raw(),
            depthTestEnable: true,
            depthWriteEnable: true,
            depthFunc: slang_rhi_sys::ComparisonFunc::Less,
            stencilEnable: false,
            ..slang_rhi_sys::DepthStencilDesc::default()
        },
        None => slang_rhi_sys::DepthStencilDesc {
            format: slang_rhi_sys::Format::Undefined,
            depthTestEnable: false,
            depthWriteEnable: false,
            stencilEnable: false,
            ..slang_rhi_sys::DepthStencilDesc::default()
        },
    }
}

/// Returns the raw rasterizer state of a description, with counter-clockwise
/// front faces.
fn rasterizer(desc: &RenderPipelineDesc) -> slang_rhi_sys::RasterizerDesc {
    slang_rhi_sys::RasterizerDesc {
        cullMode: match desc.cull_mode {
            CullMode::None => slang_rhi_sys::CullMode::None,
            CullMode::Back => slang_rhi_sys::CullMode::Back,
        },
        frontFace: slang_rhi_sys::FrontFaceMode::CounterClockwise,
        ..slang_rhi_sys::RasterizerDesc::default()
    }
}

/// A render pipeline of a device, which holds the compiled code of a vertex
/// and a fragment entry point and the state of the draws that use it.
///
/// Create one with [`Device::create_render_pipeline`], and bind it in a
/// render pass with
/// [`RenderPass::bind_pipeline`](crate::RenderPass::bind_pipeline). A render
/// pipeline holds its [`Device`], so the device lives at least as long as the
/// pipeline.
///
/// A render pipeline is neither [`Send`] nor [`Sync`], because slang-rhi
/// documents no rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of [`CommandEncoder::render_pass`](crate::CommandEncoder::render_pass).
///
/// A render pipeline cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::RenderPipeline>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::RenderPipeline>();
/// ```
#[derive(Debug)]
pub struct RenderPipeline {
    raw: Owned<IRenderPipeline>,
    _input_layout: Option<Owned<IInputLayout>>,
    device: Device,
    colour_format: Format,
    depth_format: Option<Format>,
    streams: Vec<StreamLayout>,
}

impl RenderPipeline {
    pub(crate) fn owned(&self) -> Owned<IRenderPipeline> {
        self.raw.clone()
    }

    pub(crate) fn device_ptr(&self) -> NonNull<IDevice> {
        self.device.as_non_null()
    }

    pub(crate) fn formats(&self) -> (Format, Option<Format>) {
        (self.colour_format, self.depth_format)
    }

    pub(crate) fn streams(&self) -> &[StreamLayout] {
        &self.streams
    }
}

impl Device {
    /// Creates a render pipeline from `program`, which compiles the code of
    /// its vertex and fragment entry points for the device, with the state of
    /// `desc`.
    ///
    /// The program has exactly one vertex and one fragment entry point. Each
    /// vertex stream of the description feeds the input of the vertex entry
    /// point at its location, and has a semantic, so that the pipeline
    /// matches its inputs on Vulkan, by location, and on Direct3D 12, by
    /// semantic. On Vulkan, a location below the highest one that no stream
    /// feeds gets an input element that reads the first 4 bytes of each
    /// element of the first stream, at a location that no input of the
    /// vertex entry point has, and on Direct3D 12 the input layout holds only
    /// the elements of the streams.
    ///
    /// Every varying input that the vertex entry point declares, which
    /// reflection reports with its location and semantic, needs a stream at
    /// its location, even an input that the shader does not read. Direct3D
    /// 12 keeps such an input in the input signature of the compiled code and
    /// requires an element for it, and whether the compiled code reads an
    /// input is known only after slang-rhi compiles it. The fields of a structure are inputs of their own, and a
    /// system value, such as `SV_VertexID`, is not an input. Each input is a
    /// scalar or a vector of 32-bit floating-point numbers, unsigned
    /// integers, or signed integers, and the format of its stream holds
    /// numbers of the same kind. When the input has a semantic, the stream
    /// has the same semantic name, ignoring case, and the same index.
    ///
    /// The format of a stream may have fewer or more components than its
    /// input, as both graphics APIs allow: the shader reads 0 for a missing
    /// second or third component and 1 for a missing fourth one, and the
    /// components past those of the input are not read.
    ///
    /// # Examples
    ///
    /// See the example of [`CommandEncoder::render_pass`](crate::CommandEncoder::render_pass).
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when `program` belongs to another
    /// device or does not have exactly one vertex and one fragment entry
    /// point, when the colour format is a depth format or the depth format is
    /// not [`Format::D32Float`], when the vertex streams are more than
    /// [`DeviceInfo::max_vertex_streams`](crate::DeviceInfo::max_vertex_streams)
    /// or [`MAX_VERTEX_STREAMS`], when a stream has a stride above 2048 bytes or not a multiple
    /// of 4 bytes, or no semantic, or a semantic that is empty or
    /// holds a NUL byte, when two streams have the same location, when the
    /// locations up to the highest one are more than
    /// [`DeviceInfo::max_vertex_input_elements`](crate::DeviceInfo::max_vertex_input_elements),
    /// when an input of the vertex entry point has no stream at its location
    /// or a type other than a scalar or a vector of 32-bit numbers, or when
    /// the stream of an input holds numbers of another kind or has another
    /// semantic.
    /// Returns an error of kind
    /// [`ErrorKind::Compile`](crate::ErrorKind::Compile), with the messages of
    /// Slang as its diagnostics, when slang-rhi cannot compile the program or
    /// create the pipeline.
    pub fn create_render_pipeline(
        &self,
        program: &ShaderProgram,
        desc: &RenderPipelineDesc,
    ) -> Result<RenderPipeline> {
        if program.device_ptr() != self.as_non_null() {
            return Err(Error::invalid(
                Operation::CreateRenderPipeline,
                "the shader program belongs to another device",
            ));
        }
        if program.entry_points() != EntryPoints::Render {
            return Err(Error::invalid(
                Operation::CreateRenderPipeline,
                "the shader program does not have exactly one vertex and one fragment entry point",
            ));
        }
        check_formats(desc)?;
        let info = self.info_ref();
        let plan = plan_inputs(
            &desc.vertex_streams,
            info.max_vertex_streams(),
            info.max_vertex_input_elements(),
        )?;
        check_inputs(program.vertex_inputs(), &desc.vertex_streams)?;
        let plan = for_device(plan, info.device_type());
        let input_layout = if plan.streams.is_empty() {
            None
        } else {
            Some(self.input_layout(&plan)?)
        };
        let target = colour_target(desc);
        let raw_desc = slang_rhi_sys::RenderPipelineDesc {
            program: program.as_ptr(),
            inputLayout: input_layout.as_ref().map_or(ptr::null_mut(), Owned::as_ptr),
            primitiveTopology: slang_rhi_sys::PrimitiveTopology::TriangleList,
            targets: &raw const target,
            targetCount: 1,
            depthStencil: depth_state(desc),
            rasterizer: rasterizer(desc),
            ..slang_rhi_sys::RenderPipelineDesc::default()
        };
        let device = self.as_ptr();
        let mut out: *mut IRenderPipeline = ptr::null_mut();
        let (result, messages) = guard::call_collecting("IDevice::createRenderPipeline", || {
            // SAFETY: `device` is live while `self` lives, `raw_desc` is a
            // live descriptor whose program is a live program of this device
            // with one vertex and one fragment entry point, whose input layout
            // is null or a live layout of this device, and whose one colour
            // target lives in `target` through the call, and `out` is a live
            // out-pointer. slang-rhi keeps references to the program and the
            // input layout and copies the rest.
            unsafe { IDevice::createRenderPipeline(device, &raw const raw_desc, &raw mut out) }
        });
        error::check(
            Operation::CreateRenderPipeline,
            result,
            diagnostics(&messages),
        )?;
        // SAFETY: the call succeeded, and slang-rhi returns the pipeline with
        // one reference added for the caller.
        let raw = unsafe { Owned::adopt(out) }
            .ok_or_else(|| Error::null(Operation::CreateRenderPipeline))?;
        Ok(RenderPipeline {
            raw,
            _input_layout: input_layout,
            device: self.clone(),
            colour_format: desc.colour_format,
            depth_format: desc.depth_format,
            streams: plan.streams,
        })
    }

    /// Creates the input layout of `plan`.
    fn input_layout(&self, plan: &InputPlan) -> Result<Owned<IInputLayout>> {
        let elements = raw_elements(plan);
        let streams = raw_streams(plan);
        let too_many = |_overflow| {
            Error::invalid(
                Operation::CreateRenderPipeline,
                "the input layout has too many elements",
            )
        };
        let desc = slang_rhi_sys::InputLayoutDesc {
            inputElements: elements.as_ptr(),
            inputElementCount: u32::try_from(elements.len()).map_err(too_many)?,
            vertexStreams: streams.as_ptr(),
            vertexStreamCount: u32::try_from(streams.len()).map_err(too_many)?,
            ..slang_rhi_sys::InputLayoutDesc::default()
        };
        let device = self.as_ptr();
        let mut out: *mut IInputLayout = ptr::null_mut();
        let result = guard::call("IDevice::createInputLayout", || {
            // SAFETY: `device` is live while `self` lives, `desc` names the
            // live elements and streams of the two vectors, each element
            // names a stream of the layout and a semantic that `plan` keeps
            // alive through the call, and slang-rhi copies them all, and
            // `out` is a live out-pointer.
            unsafe { IDevice::createInputLayout(device, &raw const desc, &raw mut out) }
        });
        error::check(Operation::CreateRenderPipeline, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the layout with
        // one reference added for the caller.
        unsafe { Owned::adopt(out) }.ok_or_else(|| Error::null(Operation::CreateRenderPipeline))
    }
}

#[cfg(test)]
mod tests {
    use std::ffi::CStr;

    use super::{
        BlendMode, CullMode, InputPlan, InputType, Numbers, RenderPipelineDesc, StepMode,
        StreamLayout, VertexFormat, VertexInput, VertexStream, check_formats, check_inputs,
        colour_target, depth_state, for_device, plan_inputs, rasterizer, raw_elements, raw_streams,
    };
    use crate::ErrorKind;
    use crate::device::DeviceType;
    use crate::error::Operation;
    use crate::texture::Format;

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn the_value_types_of_render_pipelines_are_send_and_sync() {
        assert_send_sync::<VertexFormat>();
        assert_send_sync::<StepMode>();
        assert_send_sync::<VertexStream>();
        assert_send_sync::<BlendMode>();
        assert_send_sync::<CullMode>();
        assert_send_sync::<RenderPipelineDesc>();
    }

    fn stream(location: u32) -> VertexStream {
        VertexStream::new(VertexFormat::Float32x2, location).with_semantic("TEXCOORD", location)
    }

    fn plan(streams: &[VertexStream]) -> InputPlan {
        plan_inputs(streams, 32, 32).expect("the streams are valid")
    }

    #[test]
    fn plan_inputs_lists_an_element_for_each_location_with_fillers_for_the_gaps() {
        let streams = [
            stream(2).with_stride(12),
            VertexStream::new(VertexFormat::Sint32x4, 0)
                .with_semantic("COLOR", 1)
                .with_step(StepMode::Instance),
        ];
        let plan = plan(&streams);
        let elements = raw_elements(&plan);
        let names: Vec<_> = elements
            .iter()
            .map(|element| {
                // SAFETY: each name points to a C string that `plan` owns.
                unsafe { CStr::from_ptr(element.semanticName) }.to_owned()
            })
            .collect();
        assert_eq!(
            names,
            [c"COLOR", c"SLANG_RHI_UNUSED", c"TEXCOORD"].map(CStr::to_owned)
        );
        let rest: Vec<_> = elements
            .iter()
            .map(|element| {
                (
                    element.semanticIndex,
                    element.format,
                    element.offset,
                    element.bufferSlotIndex,
                )
            })
            .collect();
        assert_eq!(
            rest,
            [
                (1, slang_rhi_sys::Format::RGBA32Sint, 0, 1),
                (1, slang_rhi_sys::Format::R32Float, 0, 0),
                (2, slang_rhi_sys::Format::RG32Float, 0, 0),
            ]
        );
        assert_eq!(
            plan.streams,
            [
                StreamLayout {
                    stride: 12,
                    step: StepMode::Vertex,
                    attribute_size: 8,
                },
                StreamLayout {
                    stride: 16,
                    step: StepMode::Instance,
                    attribute_size: 16,
                },
            ]
        );
        let streams = raw_streams(&plan);
        assert_eq!(
            streams
                .iter()
                .map(|stream| (stream.stride, stream.slotClass, stream.instanceDataStepRate))
                .collect::<Vec<_>>(),
            [
                (12, slang_rhi_sys::InputSlotClass::PerVertex, 0),
                (16, slang_rhi_sys::InputSlotClass::PerInstance, 1),
            ]
        );
    }

    #[test]
    fn for_device_leaves_out_the_filler_elements_on_direct3d_12_only() {
        let streams = [stream(0), stream(2)];
        let vulkan = for_device(plan(&streams), DeviceType::Vulkan);
        assert_eq!(vulkan, plan(&streams));
        let d3d12 = for_device(plan(&streams), DeviceType::D3d12);
        let names: Vec<_> = raw_elements(&d3d12)
            .iter()
            .map(|element| {
                // SAFETY: each name points to a C string that `d3d12` owns.
                (
                    unsafe { CStr::from_ptr(element.semanticName) }.to_owned(),
                    element.semanticIndex,
                    element.bufferSlotIndex,
                )
            })
            .collect();
        assert_eq!(
            names,
            [
                (c"TEXCOORD".to_owned(), 0, 0),
                (c"TEXCOORD".to_owned(), 2, 1)
            ]
        );
        assert_eq!(d3d12.streams, vulkan.streams);
    }

    #[test]
    fn plan_inputs_of_no_stream_is_empty() {
        let plan = plan(&[]);
        assert_eq!(plan.elements, []);
        assert_eq!(plan.streams, []);
    }

    #[test]
    fn plan_inputs_accepts_the_limits_exactly() {
        let streams: Vec<_> = (0..16).map(stream).collect();
        plan_inputs(&streams, 16, 16).expect("at the limits");
        plan_inputs(&[stream(31)], 1, 32).expect("the highest location fits");
        plan_inputs(&[stream(0).with_stride(2048)], 1, 1).expect("the largest stride");
        plan_inputs(&[stream(0).with_stride(0)], 1, 1).expect("the stride 0");
    }

    #[test]
    fn plan_inputs_refuses_each_stream_that_breaks_a_rule() {
        let many: Vec<_> = (0..17).map(stream).collect();
        let cases: Vec<(Vec<VertexStream>, u32, u32)> = vec![
            (many, 32, 32),
            (vec![stream(0), stream(1)], 1, 32),
            (vec![stream(0).with_stride(2049)], 32, 32),
            (vec![stream(0).with_stride(9)], 32, 32),
            (vec![stream(0).with_stride(10)], 32, 32),
            (vec![VertexStream::new(VertexFormat::Float32, 0)], 32, 32),
            (
                vec![VertexStream::new(VertexFormat::Float32, 0).with_semantic("", 0)],
                32,
                32,
            ),
            (
                vec![VertexStream::new(VertexFormat::Float32, 0).with_semantic("A\0B", 0)],
                32,
                32,
            ),
            (vec![stream(1), stream(1)], 32, 32),
            (vec![stream(32)], 32, 32),
            (vec![stream(u32::MAX)], 32, u32::MAX),
        ];
        for (streams, max_streams, max_elements) in cases {
            let error = plan_inputs(&streams, max_streams, max_elements)
                .expect_err("the streams are refused");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{streams:?}");
            assert_eq!(error.operation(), Operation::CreateRenderPipeline);
        }
    }

    fn input(
        name: &str,
        location: u32,
        semantic: Option<(&str, u32)>,
        ty: InputType,
    ) -> VertexInput {
        VertexInput {
            name: name.to_owned(),
            location,
            semantic: semantic.map(|(name, index)| (name.to_owned(), index)),
            ty,
        }
    }

    const FLOAT: InputType = InputType::Numbers(Numbers::Float);

    #[test]
    fn check_inputs_accepts_streams_of_the_kind_and_semantic_of_each_input() {
        let inputs = [
            input("position", 0, Some(("POSITION", 0)), FLOAT),
            input(
                "kind",
                1,
                Some(("KIND", 2)),
                InputType::Numbers(Numbers::Unsigned),
            ),
            input(
                "offset",
                2,
                Some(("OFFSET", 0)),
                InputType::Numbers(Numbers::Signed),
            ),
            input("plain", 3, None, FLOAT),
        ];
        let streams = [
            VertexStream::new(VertexFormat::Float32x4, 0).with_semantic("position", 0),
            VertexStream::new(VertexFormat::Uint32, 1).with_semantic("KIND", 2),
            VertexStream::new(VertexFormat::Sint32x2, 2).with_semantic("Offset", 0),
            VertexStream::new(VertexFormat::Float32, 3).with_semantic("ANY", 7),
            VertexStream::new(VertexFormat::Uint32x4, 9).with_semantic("EXTRA", 0),
        ];
        check_inputs(&inputs, &streams).expect("every input has its stream");
        check_inputs(&[], &streams).expect("no input needs no stream");
    }

    #[test]
    fn check_inputs_refuses_a_missing_stream_another_kind_another_semantic_and_another_type() {
        let position = || input("position", 0, Some(("POSITION", 0)), FLOAT);
        let float2 = || VertexStream::new(VertexFormat::Float32x2, 0).with_semantic("POSITION", 0);
        let cases = [
            (vec![position()], vec![]),
            (
                vec![position()],
                vec![VertexStream::new(VertexFormat::Float32x2, 1).with_semantic("POSITION", 0)],
            ),
            (
                vec![position()],
                vec![VertexStream::new(VertexFormat::Uint32x2, 0).with_semantic("POSITION", 0)],
            ),
            (
                vec![input(
                    "kind",
                    0,
                    Some(("KIND", 0)),
                    InputType::Numbers(Numbers::Unsigned),
                )],
                vec![VertexStream::new(VertexFormat::Sint32, 0).with_semantic("KIND", 0)],
            ),
            (
                vec![position()],
                vec![VertexStream::new(VertexFormat::Float32x2, 0).with_semantic("COLOR", 0)],
            ),
            (
                vec![position()],
                vec![VertexStream::new(VertexFormat::Float32x2, 0).with_semantic("POSITION", 1)],
            ),
            (
                vec![input(
                    "matrix",
                    0,
                    Some(("POSITION", 0)),
                    InputType::Other(String::from("of the kind Matrix")),
                )],
                vec![float2()],
            ),
            (
                vec![
                    position(),
                    input(
                        "filled",
                        1,
                        Some(("FILLED", 0)),
                        InputType::Numbers(Numbers::Unsigned),
                    ),
                ],
                vec![
                    float2(),
                    VertexStream::new(VertexFormat::Float32x2, 2).with_semantic("OTHER", 0),
                ],
            ),
        ];
        for (inputs, streams) in cases {
            let error = check_inputs(&inputs, &streams).expect_err("the streams are refused");
            assert_eq!(
                error.kind(),
                ErrorKind::InvalidArgument,
                "{inputs:?} {streams:?}"
            );
            assert_eq!(error.operation(), Operation::CreateRenderPipeline);
        }
    }

    #[test]
    fn check_formats_refuses_a_depth_colour_target_and_a_colour_depth_target() {
        check_formats(&RenderPipelineDesc::new(Format::Bgra8UnormSrgb)).expect("valid");
        check_formats(
            &RenderPipelineDesc::new(Format::Rgba8Unorm).with_depth_format(Format::D32Float),
        )
        .expect("valid");
        for desc in [
            RenderPipelineDesc::new(Format::D32Float),
            RenderPipelineDesc::new(Format::Rgba8Unorm).with_depth_format(Format::Rgba8Unorm),
        ] {
            let error = check_formats(&desc).expect_err("refused");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        }
    }

    #[test]
    fn the_blend_modes_set_the_factors_of_alpha_blending_or_none() {
        let replace = colour_target(&RenderPipelineDesc::new(Format::Rgba8Unorm));
        assert!(!replace.enableBlend);
        assert_eq!(replace.format, slang_rhi_sys::Format::RGBA8Unorm);
        assert_eq!(replace.writeMask, slang_rhi_sys::RenderTargetWriteMask::All);
        let alpha = colour_target(
            &RenderPipelineDesc::new(Format::Rgba8Unorm).with_blend(BlendMode::Alpha),
        );
        assert!(alpha.enableBlend);
        assert_eq!(
            (alpha.color.srcFactor, alpha.color.dstFactor, alpha.color.op),
            (
                slang_rhi_sys::BlendFactor::SrcAlpha,
                slang_rhi_sys::BlendFactor::InvSrcAlpha,
                slang_rhi_sys::BlendOp::Add
            )
        );
        assert_eq!(
            (alpha.alpha.srcFactor, alpha.alpha.dstFactor, alpha.alpha.op),
            (
                slang_rhi_sys::BlendFactor::One,
                slang_rhi_sys::BlendFactor::InvSrcAlpha,
                slang_rhi_sys::BlendOp::Add
            )
        );
    }

    #[test]
    fn the_depth_state_tests_less_and_writes_only_with_a_depth_format() {
        let depth = depth_state(
            &RenderPipelineDesc::new(Format::Rgba8Unorm).with_depth_format(Format::D32Float),
        );
        assert!(depth.depthTestEnable && depth.depthWriteEnable && !depth.stencilEnable);
        assert_eq!(depth.depthFunc, slang_rhi_sys::ComparisonFunc::Less);
        assert_eq!(depth.format, slang_rhi_sys::Format::D32Float);
        let none = depth_state(&RenderPipelineDesc::new(Format::Rgba8Unorm));
        assert!(!none.depthTestEnable && !none.depthWriteEnable && !none.stencilEnable);
        assert_eq!(none.format, slang_rhi_sys::Format::Undefined);
    }

    #[test]
    fn the_rasterizer_has_counter_clockwise_front_faces_and_the_cull_mode() {
        for (cull, raw) in [
            (CullMode::None, slang_rhi_sys::CullMode::None),
            (CullMode::Back, slang_rhi_sys::CullMode::Back),
        ] {
            let state =
                rasterizer(&RenderPipelineDesc::new(Format::Rgba8Unorm).with_cull_mode(cull));
            assert_eq!(state.cullMode, raw);
            assert_eq!(
                state.frontFace,
                slang_rhi_sys::FrontFaceMode::CounterClockwise
            );
            assert_eq!(state.fillMode, slang_rhi_sys::FillMode::Solid);
        }
    }
}
