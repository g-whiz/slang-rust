use std::cell::RefCell;
use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::ptr::{self, NonNull};

use slang_rhi_sys::{
    IBuffer, ICommandEncoder, IDevice, IRenderPassEncoder, IRenderPipeline, ITexture, ITextureView,
};

use crate::buffer::{Buffer, BufferUsage};
use crate::command::CommandEncoder;
use crate::error::{Error, Operation, Result};
use crate::guard;
use crate::owned::Owned;
use crate::render_pipeline::{RenderPipeline, StepMode, StreamLayout};
use crate::shader_object::{ShaderObject, TextureRecord};
use crate::texture::{Format, Texture, TextureUsage};

/// What a render pass does with the contents of its colour target when it
/// begins.
///
/// # Examples
///
/// See the example of [`CommandEncoder::render_pass`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum ColourLoad {
    /// The pass keeps the contents of the target. A texture that was never
    /// written, neither by initial data nor by a pass, holds undefined
    /// contents.
    Load,
    /// The pass clears the target to the colour of red, green, blue, and
    /// alpha, each from 0 to 1 for a format whose values range from 0 to 1.
    Clear([f32; 4]),
}

/// What a render pass does with the contents of its depth target when it
/// begins.
///
/// # Examples
///
/// See the example of [`CommandEncoder::render_pass`].
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum DepthLoad {
    /// The pass keeps the contents of the target. A depth texture that no
    /// pass has written holds undefined contents.
    Load,
    /// The pass clears the target to the depth, from 0 to 1.
    Clear(f32),
}

/// The description of a render pass that [`CommandEncoder::render_pass`]
/// records: an optional colour target and an optional depth target, at least
/// one of the two, each with what the pass does with its contents when it
/// begins.
///
/// The pass stores what it draws into each target. A pass with a depth
/// target alone draws nothing, and clears or keeps that target.
///
/// # Examples
///
/// See the example of [`CommandEncoder::render_pass`].
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderPassDesc<'t> {
    colour: Option<(&'t Texture, ColourLoad)>,
    depth: Option<(&'t Texture, DepthLoad)>,
}

impl<'t> RenderPassDesc<'t> {
    /// Creates the description of a pass without a target, to which a
    /// colour target, a depth target, or both are added.
    ///
    /// # Examples
    ///
    /// See the example of [`CommandEncoder::render_pass`].
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a copy that draws into `texture` as its colour target, whose
    /// contents the pass loads or clears as `load` says.
    ///
    /// # Examples
    ///
    /// See the example of [`CommandEncoder::render_pass`].
    #[must_use]
    pub fn with_colour_target(self, texture: &'t Texture, load: ColourLoad) -> Self {
        Self {
            colour: Some((texture, load)),
            ..self
        }
    }

    /// Returns a copy that tests and writes depth in `texture` as its depth
    /// target, whose contents the pass loads or clears as `load` says.
    ///
    /// # Examples
    ///
    /// See the example of [`CommandEncoder::render_pass`].
    #[must_use]
    pub fn with_depth_target(self, texture: &'t Texture, load: DepthLoad) -> Self {
        Self {
            depth: Some((texture, load)),
            ..self
        }
    }
}

/// What a render pass knows of one of its targets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Target {
    texture: NonNull<ITexture>,
    format: Format,
    usage: TextureUsage,
    size: [u32; 2],
}

impl Target {
    fn of(texture: &Texture) -> Self {
        Self {
            texture: texture.as_non_null(),
            format: texture.format(),
            usage: texture.usage(),
            size: texture.size(),
        }
    }
}

/// Checks the targets of a render pass: at least one of the two, a colour
/// target of a colour format with the use of a render target, a depth target
/// of [`Format::D32Float`] with the use of a depth target and, beside a
/// colour target, its size, and a clear depth from 0 to 1.
fn check_targets(
    colour: Option<Target>,
    depth: Option<Target>,
    depth_load: Option<DepthLoad>,
) -> Result<()> {
    let invalid = |detail: String| Err(Error::invalid(Operation::BeginRenderPass, detail));
    if colour.is_none() && depth.is_none() {
        return invalid(String::from(
            "the pass has neither a colour nor a depth target",
        ));
    }
    if let Some(colour) = colour {
        if colour.format.is_depth() {
            return invalid(format!(
                "the colour target has the depth format {:?}",
                colour.format
            ));
        }
        if !colour.usage.contains(TextureUsage::RENDER_TARGET) {
            return invalid(String::from(
                "the colour target does not have the use RENDER_TARGET",
            ));
        }
    }
    if let Some(depth) = depth {
        if depth.format != Format::D32Float {
            return invalid(format!(
                "the depth target has the format {:?}, and not D32Float",
                depth.format
            ));
        }
        if !depth.usage.contains(TextureUsage::DEPTH_STENCIL) {
            return invalid(String::from(
                "the depth target does not have the use DEPTH_STENCIL",
            ));
        }
        if let Some(colour) = colour
            && colour.size != depth.size
        {
            return invalid(format!(
                "the depth target of {:?} texels differs from the colour target of {:?} texels",
                depth.size, colour.size
            ));
        }
    }
    if let Some(DepthLoad::Clear(value)) = depth_load
        && !(0.0..=1.0).contains(&value)
    {
        return invalid(format!(
            "the clear depth {value} is outside the range from 0 to 1"
        ));
    }
    Ok(())
}

/// Checks that the formats of a bound pipeline, `pipeline`, equal those of
/// the targets of the pass, `colour` and `depth`.
fn check_formats(
    colour: Option<Format>,
    depth: Option<Format>,
    pipeline: (Format, Option<Format>),
) -> Result<()> {
    let (pipeline_colour, pipeline_depth) = pipeline;
    if colour != Some(pipeline_colour) {
        return Err(Error::invalid(
            Operation::Draw,
            format!(
                "the pipeline draws into a colour target of {pipeline_colour:?}, and the pass has {colour:?}"
            ),
        ));
    }
    if depth != pipeline_depth {
        return Err(Error::invalid(
            Operation::Draw,
            format!(
                "the pipeline has the depth format {pipeline_depth:?}, and the depth target of the pass {depth:?}"
            ),
        ));
    }
    Ok(())
}

/// Checks that no texture of `sampled`, the textures that a shader object of
/// the pass bound, is a target of the pass.
fn check_sampled(sampled: &[NonNull<ITexture>], targets: &[Target]) -> Result<()> {
    if sampled
        .iter()
        .any(|texture| targets.iter().any(|target| target.texture == *texture))
    {
        Err(Error::invalid(
            Operation::Draw,
            "a texture that a shader object of the pass binds is also a target of the pass",
        ))
    } else {
        Ok(())
    }
}

/// Returns whether a buffer of `size` bytes holds `count` elements of
/// `stride` bytes, the last of which reads `attribute_size` bytes.
fn holds(size: u64, count: u32, stride: u32, attribute_size: u32) -> bool {
    match count.checked_sub(1) {
        None => true,
        Some(last) => u64::from(last)
            .checked_mul(u64::from(stride))
            .and_then(|start| start.checked_add(u64::from(attribute_size)))
            .is_some_and(|end| end <= size),
    }
}

/// Checks that a buffer of `sizes` is set for each stream of `streams`, and
/// that it holds `instance_count` elements for a stream that steps for each
/// instance and, for a draw without indices, `vertex_count` elements for a
/// stream that steps for each vertex.
fn check_vertex_buffers(
    streams: &[StreamLayout],
    sizes: &[u64],
    vertex_count: Option<u32>,
    instance_count: u32,
) -> Result<()> {
    for (index, stream) in streams.iter().enumerate() {
        let Some(size) = sizes.get(index) else {
            return Err(Error::invalid(
                Operation::Draw,
                format!("no vertex buffer is set for the vertex stream {index}"),
            ));
        };
        let count = match stream.step {
            StepMode::Instance => Some(instance_count),
            StepMode::Vertex => vertex_count,
        };
        if let Some(count) = count
            && !holds(*size, count, stream.stride, stream.attribute_size)
        {
            return Err(Error::invalid(
                Operation::Draw,
                format!(
                    "the vertex buffer of {size} bytes of the stream {index} does not hold {count} elements of {} bytes",
                    stream.stride
                ),
            ));
        }
    }
    Ok(())
}

/// Checks that an index buffer of `size` bytes is set and holds `count`
/// indices of 32 bits.
fn check_index_buffer(size: Option<u64>, count: u32) -> Result<()> {
    let Some(size) = size else {
        return Err(Error::invalid(
            Operation::Draw,
            "no index buffer is set for a draw with indices",
        ));
    };
    if holds(size, count, 4, 4) {
        Ok(())
    } else {
        Err(Error::invalid(
            Operation::Draw,
            format!("the index buffer of {size} bytes does not hold {count} indices of 4 bytes"),
        ))
    }
}

/// Checks that a buffer of the device `buffer_device` with the uses `usage`
/// can be set in a pass of the device `device` for the use `needed`.
fn check_pass_buffer(
    buffer_device: NonNull<IDevice>,
    device: NonNull<IDevice>,
    usage: BufferUsage,
    needed: BufferUsage,
    operation: Operation,
) -> Result<()> {
    if buffer_device != device {
        return Err(Error::invalid(
            operation,
            "the buffer belongs to another device",
        ));
    }
    if usage.contains(needed) {
        Ok(())
    } else {
        Err(Error::invalid(
            operation,
            format!("the buffer does not have the use {needed:?}"),
        ))
    }
}

/// Returns the raw colour attachment of the view `view` of a colour target,
/// which the pass loads or clears as `load` says and stores, with every
/// field written.
fn colour_attachment(
    view: NonNull<ITextureView>,
    load: ColourLoad,
) -> slang_rhi_sys::RenderPassColorAttachment {
    let (load_op, clear) = match load {
        ColourLoad::Load => (slang_rhi_sys::LoadOp::Load, [0.0; 4]),
        ColourLoad::Clear(colour) => (slang_rhi_sys::LoadOp::Clear, colour),
    };
    slang_rhi_sys::RenderPassColorAttachment {
        view: view.as_ptr(),
        resolveTarget: ptr::null_mut(),
        loadOp: load_op,
        storeOp: slang_rhi_sys::StoreOp::Store,
        clearValue: clear,
    }
}

/// Returns the render state of a draw into a target of `size` texels, with
/// one viewport and one scissor rectangle over the whole target and the
/// depth range from 0 to 1, the vertex buffers `vertex`, and the index
/// buffer `index` of 32-bit indices.
fn render_state(
    size: [u32; 2],
    vertex: &[NonNull<IBuffer>],
    index: Option<NonNull<IBuffer>>,
) -> Result<slang_rhi_sys::RenderState> {
    let [width, height] = size;
    let mut state = slang_rhi_sys::RenderState::default();
    let extent = |value: u32| {
        u16::try_from(value).map(f32::from).map_err(|_large| {
            Error::invalid(
                Operation::Draw,
                format!("the target of {width} by {height} texels is too large for a viewport"),
            )
        })
    };
    state.viewports[0] = slang_rhi_sys::Viewport {
        originX: 0.0,
        originY: 0.0,
        extentX: extent(width)?,
        extentY: extent(height)?,
        minZ: 0.0,
        maxZ: 1.0,
    };
    state.viewportCount = 1;
    state.scissorRects[0] = slang_rhi_sys::ScissorRect {
        minX: 0,
        minY: 0,
        maxX: width,
        maxY: height,
    };
    state.scissorRectCount = 1;
    for (slot, buffer) in state.vertexBuffers.iter_mut().zip(vertex) {
        *slot = slang_rhi_sys::BufferOffsetPair {
            buffer: buffer.as_ptr(),
            offset: 0,
        };
    }
    state.vertexBufferCount = u32::try_from(vertex.len().min(state.vertexBuffers.len()))
        .map_err(|_overflow| Error::invalid(Operation::Draw, "too many vertex buffers"))?;
    state.indexBuffer = slang_rhi_sys::BufferOffsetPair {
        buffer: index.map_or(ptr::null_mut(), NonNull::as_ptr),
        offset: 0,
    };
    state.indexFormat = slang_rhi_sys::IndexFormat::Uint32;
    Ok(state)
}

/// A buffer that a render pass holds for its draws, with its size.
#[derive(Clone, Debug)]
pub(crate) struct PassBuffer {
    raw: Owned<IBuffer>,
    size: u64,
}

/// The pipeline that a render pass has bound, with what its draws check.
#[derive(Debug)]
pub(crate) struct BoundPipeline {
    raw: Owned<IRenderPipeline>,
    formats: (Format, Option<Format>),
    streams: Vec<StreamLayout>,
}

/// A render pass of a command encoder, in which render pipelines are bound
/// and draw into the targets of the pass.
///
/// A pass exists only inside the closure of
/// [`CommandEncoder::render_pass`], which begins it before the closure runs
/// and ends it after the closure returns, so no pass outlives its closure.
/// Each draw sets one viewport and one scissor rectangle over the whole
/// colour target, with the depth range from 0 to 1. Clip space has its y
/// axis up and the top row of a target at y = 1, and the fragment at the
/// top-left texel of a target has the position (0.5, 0.5), on both graphics
/// APIs.
///
/// The parts of a primitive whose depth lies outside the range from 0 to 1
/// differ between the graphics APIs. On Vulkan, slang-rhi clamps their depth
/// to that range instead of clipping them, so they are drawn, while
/// Direct3D 12 clips them, so they are not drawn.
///
/// A pass is neither [`Send`] nor [`Sync`], because slang-rhi documents no
/// rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of [`CommandEncoder::render_pass`].
///
/// A pass cannot leave its closure, so this example, which tries to keep the
/// root shader object of a pass after the closure, does not compile:
///
/// ```compile_fail,E0521
/// use slang_rhi::{CommandEncoder, RenderPassDesc, RenderPipeline};
///
/// fn escape(
///     encoder: &mut CommandEncoder,
///     desc: &RenderPassDesc<'_>,
///     pipeline: &RenderPipeline,
/// ) -> slang_rhi::Result<()> {
///     let mut kept = None;
///     encoder.render_pass(desc, |pass| {
///         kept = Some(pass.bind_pipeline(pipeline)?);
///         Ok(())
///     })?;
///     drop(kept);
///     Ok(())
/// }
/// ```
///
/// A pass cannot move to another thread, so this example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::RenderPass<'static>>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::RenderPass<'static>>();
/// ```
#[derive(Debug)]
pub struct RenderPass<'e> {
    raw: NonNull<IRenderPassEncoder>,
    device: NonNull<IDevice>,
    colour: Option<Target>,
    depth: Option<Target>,
    pipeline: Option<BoundPipeline>,
    vertex_buffers: Vec<PassBuffer>,
    index_buffer: Option<PassBuffer>,
    textures: TextureRecord,
    encoder: PhantomData<&'e mut ()>,
}

impl CommandEncoder {
    /// Records a render pass with the targets of `desc`, in which `f` binds
    /// render pipelines, sets their parameters and buffers, and draws, and
    /// returns the value of `f`.
    ///
    /// The method begins the pass, runs `f`, and ends the pass, whether `f`
    /// returns a value or an error or panics. A panic of `f` continues after
    /// the pass has ended. A pass whose only target is a depth target clears
    /// or keeps that target and cannot draw.
    ///
    /// # Examples
    ///
    /// This example draws a triangle with a colour from a vertex buffer into
    /// a texture of 4 by 4 texels and reads it back. It needs a GPU driver at
    /// run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::{Component, GlobalSession, Stage};
    /// use slang_rhi::{
    ///     BufferDesc, BufferUsage, ColourLoad, Device, DeviceOptions, Format, RenderPassDesc,
    ///     RenderPipelineDesc, TextureDesc, TextureUsage, VertexFormat, VertexStream,
    /// };
    ///
    /// const SHADER: &str = r#"
    /// struct Output { float4 position : SV_Position; };
    ///
    /// [shader("vertex")]
    /// Output vertex_main(float2 position : POSITION)
    /// {
    ///     Output output;
    ///     output.position = float4(position, 0.0, 1.0);
    ///     return output;
    /// }
    ///
    /// [shader("fragment")]
    /// float4 fragment_main() : SV_Target
    /// {
    ///     return float4(1.0, 0.0, 0.0, 1.0);
    /// }
    /// "#;
    ///
    /// let device = Device::new(&DeviceOptions::new(GlobalSession::new()?))?;
    /// let session = device.slang_session();
    /// let module = session
    ///     .load_module_from_source("triangle", "triangle.slang", SHADER)?
    ///     .into_value();
    /// let vertex = module.entry_point_with_stage("vertex_main", Stage::Vertex)?.into_value();
    /// let fragment = module
    ///     .entry_point_with_stage("fragment_main", Stage::Fragment)?
    ///     .into_value();
    /// let linked = session
    ///     .compose(&[
    ///         Component::Module(&module),
    ///         Component::EntryPoint(&vertex),
    ///         Component::EntryPoint(&fragment),
    ///     ])?
    ///     .into_value()
    ///     .link()?
    ///     .into_value();
    /// let program = device.create_shader_program(&linked)?;
    /// let desc = RenderPipelineDesc::new(Format::Rgba8Unorm).with_vertex_streams([
    ///     VertexStream::new(VertexFormat::Float32x2, 0).with_semantic("POSITION", 0),
    /// ]);
    /// let pipeline = device.create_render_pipeline(&program, &desc)?;
    ///
    /// let corners: Vec<u8> = [-1.0_f32, -1.0, 3.0, -1.0, -1.0, 3.0]
    ///     .iter()
    ///     .flat_map(|value| value.to_le_bytes())
    ///     .collect();
    /// let vertices =
    ///     device.create_buffer(&BufferDesc::new(24, BufferUsage::VERTEX_BUFFER), Some(&corners))?;
    /// let target = device.create_texture(
    ///     &TextureDesc::new(
    ///         [4, 4],
    ///         Format::Rgba8Unorm,
    ///         TextureUsage::RENDER_TARGET | TextureUsage::COPY_SOURCE,
    ///     ),
    ///     None,
    /// )?;
    ///
    /// let queue = device.queue()?;
    /// let mut encoder = queue.encoder()?;
    /// let pass = RenderPassDesc::new().with_colour_target(&target, ColourLoad::Clear([0.0; 4]));
    /// encoder.render_pass(&pass, |pass| {
    ///     drop(pass.bind_pipeline(&pipeline)?);
    ///     pass.set_vertex_buffers(&[&vertices])?;
    ///     pass.draw(3, 1)
    /// })?;
    /// queue.submit([encoder.finish()?])?;
    /// queue.wait_on_host()?;
    ///
    /// let mut texels = vec![0_u8; 64];
    /// device.read_texture(&target, &mut texels)?;
    /// assert!(texels.chunks_exact(4).all(|texel| texel == [255, 0, 0, 255]));
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the error that `f` returns. Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when a target belongs to another
    /// device, when the pass has neither a colour nor a depth target, when
    /// the colour target has a depth format or lacks the use
    /// [`TextureUsage::RENDER_TARGET`], when the depth target is not of
    /// [`Format::D32Float`], lacks the use [`TextureUsage::DEPTH_STENCIL`],
    /// or differs in size from the colour target, or when the clear depth is
    /// outside the range from 0 to 1. Returns an error of kind
    /// [`ErrorKind::Failed`](crate::ErrorKind::Failed) when slang-rhi begins
    /// no pass.
    pub fn render_pass<R>(
        &mut self,
        desc: &RenderPassDesc<'_>,
        f: impl FnOnce(&mut RenderPass<'_>) -> Result<R>,
    ) -> Result<R> {
        let device = self.device().as_non_null();
        for (texture, kind) in [
            (desc.colour.map(|(texture, _)| texture), "colour"),
            (desc.depth.map(|(texture, _)| texture), "depth"),
        ] {
            if texture.is_some_and(|texture| texture.device_ptr() != device) {
                return Err(Error::invalid(
                    Operation::BeginRenderPass,
                    format!("the {kind} target belongs to another device"),
                ));
            }
        }
        let colour = desc.colour.map(|(texture, _)| Target::of(texture));
        let depth = desc.depth.map(|(texture, _)| Target::of(texture));
        check_targets(colour, depth, desc.depth.map(|(_, load)| load))?;
        let colour_attachment = desc
            .colour
            .map(|(texture, load)| colour_attachment(texture.view(), load));
        let depth_attachment = desc.depth.map(|(texture, load)| {
            let (load_op, clear) = match load {
                DepthLoad::Load => (slang_rhi_sys::LoadOp::Load, 1.0),
                DepthLoad::Clear(depth) => (slang_rhi_sys::LoadOp::Clear, depth),
            };
            slang_rhi_sys::RenderPassDepthStencilAttachment {
                view: texture.view().as_ptr(),
                depthLoadOp: load_op,
                depthStoreOp: slang_rhi_sys::StoreOp::Store,
                depthClearValue: clear,
                depthReadOnly: false,
                ..slang_rhi_sys::RenderPassDepthStencilAttachment::default()
            }
        });
        let raw_desc = slang_rhi_sys::RenderPassDesc {
            colorAttachments: colour_attachment
                .as_ref()
                .map_or(ptr::null(), ptr::from_ref),
            colorAttachmentCount: u32::from(colour_attachment.is_some()),
            depthStencilAttachment: depth_attachment.as_ref().map_or(ptr::null(), ptr::from_ref),
        };
        let encoder: *mut ICommandEncoder = self.as_ptr();
        let pass = guard::call("ICommandEncoder::beginRenderPass", || {
            // SAFETY: `encoder` is live while `self` lives, and no other pass
            // of it is open, because every pass ends before this method
            // returns or unwinds. `raw_desc` names at most one colour and one
            // depth attachment, each the live default view of a target of
            // this device that `desc` borrows through the call, of the format
            // and the use that its attachment needs by the checks above, and
            // slang-rhi copies the attachments and keeps a reference to each
            // view.
            unsafe { ICommandEncoder::beginRenderPass(encoder, &raw const raw_desc) }
        });
        let pass = NonNull::new(pass).ok_or_else(|| Error::null(Operation::BeginRenderPass))?;
        // SAFETY: the pass belongs to the encoder, which `self` keeps alive,
        // and the mutable borrow of `self` keeps every other use of the
        // encoder out until the pass ends inside `run_render_pass`. The
        // targets are the textures of `desc`, which outlive the call.
        unsafe { run_render_pass(pass, device, colour, depth, f) }
    }
}

/// Runs `f` with the render pass `raw`, and ends the pass after `f` returns
/// or panics.
///
/// # Safety
///
/// `raw` points to the live pass encoder of a render pass that has begun
/// and not ended, whose command encoder stays alive and is not used in any
/// other way until this function returns. `device` is the device of the
/// encoder, and `colour` and `depth` describe the live targets of the pass,
/// which stay alive until this function returns. Breaking any of these is
/// undefined behaviour.
unsafe fn run_render_pass<R>(
    raw: NonNull<IRenderPassEncoder>,
    device: NonNull<IDevice>,
    colour: Option<Target>,
    depth: Option<Target>,
    f: impl FnOnce(&mut RenderPass<'_>) -> Result<R>,
) -> Result<R> {
    let mut pass = RenderPass {
        raw,
        device,
        colour,
        depth,
        pipeline: None,
        vertex_buffers: Vec::new(),
        index_buffer: None,
        textures: RefCell::new(Vec::new()),
        encoder: PhantomData,
    };
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| f(&mut pass)));
    let pass_encoder = raw.as_ptr();
    guard::call("IRenderPassEncoder::end", || {
        // SAFETY: the caller guarantees that the pass is live and open, and
        // `f` has returned, so nothing borrows the pass any longer.
        unsafe { IRenderPassEncoder::end(pass_encoder) }
    });
    drop(pass);
    match outcome {
        Ok(value) => value,
        Err(payload) => panic::resume_unwind(payload),
    }
}

impl RenderPass<'_> {
    /// Binds `pipeline` for the draws that follow, and returns the root
    /// shader object of its program, which holds the global parameters of the
    /// program.
    ///
    /// The root shader object borrows the pass, so the pass can draw or bind
    /// another pipeline only after the root object is dropped. After a failed
    /// bind, the pass refuses to draw until a bind succeeds.
    ///
    /// # Examples
    ///
    /// See the example of [`CommandEncoder::render_pass`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the pipeline belongs to another
    /// device, and an error of kind
    /// [`ErrorKind::Failed`](crate::ErrorKind::Failed) when slang-rhi returns
    /// no root shader object.
    pub fn bind_pipeline(&mut self, pipeline: &RenderPipeline) -> Result<ShaderObject<'_>> {
        if pipeline.device_ptr() != self.device {
            return Err(Error::invalid(
                Operation::BindRenderPipeline,
                "the pipeline belongs to another device",
            ));
        }
        self.bind(BoundPipeline {
            raw: pipeline.owned(),
            formats: pipeline.formats(),
            streams: pipeline.streams().to_vec(),
        })
    }

    /// Binds the pipeline of `bound`, a pipeline of the device of the pass.
    fn bind(&mut self, bound: BoundPipeline) -> Result<ShaderObject<'_>> {
        self.pipeline = None;
        let pass = self.raw.as_ptr();
        let pipeline = bound.raw.as_ptr();
        let root = guard::call("IRenderPassEncoder::bindPipeline", || {
            // SAFETY: the pass is live and open while `self` lives, and
            // `pipeline` is a live pipeline of the same device, which `bound`
            // keeps alive. slang-rhi keeps its own reference to the pipeline.
            unsafe { IRenderPassEncoder::bindPipeline(pass, pipeline) }
        });
        let root = NonNull::new(root).ok_or_else(|| Error::null(Operation::BindRenderPipeline))?;
        self.pipeline = Some(bound);
        // SAFETY: the pass holds the root object until its next
        // `bindPipeline` or until the encoder is freed, and the returned value
        // borrows `self` mutably, so neither happens while it lives.
        Ok(unsafe { ShaderObject::borrowed(root, self.device, Some(&self.textures)) })
    }

    /// Sets the vertex buffers of the draws that follow, one for each vertex
    /// stream of the bound pipeline, in the order of the streams.
    ///
    /// Each buffer feeds its stream from its start. The pass keeps its own
    /// reference to each buffer.
    ///
    /// The crate does not refuse a vertex buffer that a draw also writes
    /// through a read-write parameter, and the contents that such a draw
    /// reads from the buffer are undefined.
    ///
    /// # Examples
    ///
    /// See the example of [`CommandEncoder::render_pass`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// when there are more than 16 buffers, or when a buffer belongs to
    /// another device or lacks the use
    /// [`BufferUsage::VERTEX_BUFFER`](crate::BufferUsage::VERTEX_BUFFER).
    /// The buffers of the pass do not change then.
    pub fn set_vertex_buffers(&mut self, buffers: &[&Buffer]) -> Result<()> {
        if buffers.len() > 16 {
            return Err(Error::invalid(
                Operation::SetVertexBuffers,
                format!(
                    "{} vertex buffers are more than the 16 of a draw",
                    buffers.len()
                ),
            ));
        }
        for buffer in buffers {
            check_pass_buffer(
                buffer.device_ptr(),
                self.device,
                buffer.usage(),
                BufferUsage::VERTEX_BUFFER,
                Operation::SetVertexBuffers,
            )?;
        }
        self.vertex_buffers = buffers
            .iter()
            .map(|buffer| PassBuffer {
                raw: buffer.owned(),
                size: buffer.size(),
            })
            .collect();
        Ok(())
    }

    /// Sets the index buffer of the draws with indices that follow, which
    /// holds 32-bit indices from its start.
    ///
    /// The pass keeps its own reference to the buffer. The crate does not
    /// read the indices, so an index past the end of a vertex buffer reaches
    /// the device, which reads zeros or values from within the buffer for it,
    /// as Vulkan guarantees with the feature `robustBufferAccess` that
    /// slang-rhi enables and Direct3D 12 guarantees always.
    ///
    /// The crate does not refuse an index buffer that a draw also writes
    /// through a read-write parameter, and the contents that such a draw
    /// reads from the buffer are undefined.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPass::draw_indexed`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument)
    /// when the buffer belongs to another device or lacks the use
    /// [`BufferUsage::INDEX_BUFFER`](crate::BufferUsage::INDEX_BUFFER).
    pub fn set_index_buffer(&mut self, buffer: &Buffer) -> Result<()> {
        check_pass_buffer(
            buffer.device_ptr(),
            self.device,
            buffer.usage(),
            BufferUsage::INDEX_BUFFER,
            Operation::SetIndexBuffer,
        )?;
        self.index_buffer = Some(PassBuffer {
            raw: buffer.owned(),
            size: buffer.size(),
        });
        Ok(())
    }

    /// Draws `instance_count` instances of `vertex_count` vertices with the
    /// bound pipeline, as a list of triangles.
    ///
    /// # Examples
    ///
    /// See the example of [`CommandEncoder::render_pass`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when no pipeline is bound in the pass,
    /// when the colour or depth format of the pipeline differs from that of
    /// the targets of the pass, such as in a pass without a colour target,
    /// when a texture that a shader object of the pass binds is a target of
    /// the pass, or when a vertex stream of the pipeline has no vertex buffer
    /// or one that holds fewer elements than the vertices, or the instances
    /// for a stream that steps for each instance.
    pub fn draw(&mut self, vertex_count: u32, instance_count: u32) -> Result<()> {
        self.record(Some(vertex_count), instance_count, None)
    }

    /// Draws `instance_count` instances of the vertices that the first
    /// `index_count` indices of the index buffer name, with the bound
    /// pipeline, as a list of triangles.
    ///
    /// # Examples
    ///
    /// This example draws a square of two triangles from four corners and
    /// six indices into a pass whose pipeline and buffers the caller made:
    ///
    /// ```
    /// use slang_rhi::{Buffer, RenderPass, RenderPipeline};
    ///
    /// fn square(
    ///     pass: &mut RenderPass<'_>,
    ///     pipeline: &RenderPipeline,
    ///     corners: &Buffer,
    ///     indices: &Buffer,
    /// ) -> slang_rhi::Result<()> {
    ///     drop(pass.bind_pipeline(pipeline)?);
    ///     pass.set_vertex_buffers(&[corners])?;
    ///     pass.set_index_buffer(indices)?;
    ///     pass.draw_indexed(6, 1)
    /// }
    /// # let _ = square;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, in the cases of [`RenderPass::draw`],
    /// apart from the number of vertices, which the indices decide, and when
    /// no index buffer is set or it holds fewer than `index_count` indices.
    pub fn draw_indexed(&mut self, index_count: u32, instance_count: u32) -> Result<()> {
        self.record(None, instance_count, Some(index_count))
    }

    /// Checks and records a draw of `vertex_count` vertices, or of
    /// `index_count` indices, and `instance_count` instances.
    fn record(
        &mut self,
        vertex_count: Option<u32>,
        instance_count: u32,
        index_count: Option<u32>,
    ) -> Result<()> {
        let Some(pipeline) = &self.pipeline else {
            return Err(Error::invalid(
                Operation::Draw,
                "no pipeline is bound in the pass",
            ));
        };
        check_formats(
            self.colour.map(|target| target.format),
            self.depth.map(|target| target.format),
            pipeline.formats,
        )?;
        let targets: Vec<Target> = self.colour.into_iter().chain(self.depth).collect();
        check_sampled(&self.textures.borrow(), &targets)?;
        let sizes: Vec<u64> = self
            .vertex_buffers
            .iter()
            .map(|buffer| buffer.size)
            .collect();
        check_vertex_buffers(&pipeline.streams, &sizes, vertex_count, instance_count)?;
        if let Some(index_count) = index_count {
            check_index_buffer(
                self.index_buffer.as_ref().map(|buffer| buffer.size),
                index_count,
            )?;
        }
        let size = self.colour.map_or([0, 0], |target| target.size);
        let vertex: Vec<NonNull<IBuffer>> = self
            .vertex_buffers
            .iter()
            .take(pipeline.streams.len())
            .map(|buffer| buffer.raw.as_non_null())
            .collect();
        let index = self
            .index_buffer
            .as_ref()
            .map(|buffer| buffer.raw.as_non_null());
        let state = render_state(size, &vertex, index)?;
        let args = slang_rhi_sys::DrawArguments {
            vertexCount: vertex_count.or(index_count).unwrap_or(0),
            instanceCount: instance_count,
            ..slang_rhi_sys::DrawArguments::default()
        };
        let pass = self.raw.as_ptr();
        guard::call("IRenderPassEncoder::setRenderState", || {
            // SAFETY: the pass is live and open while `self` lives, and
            // `state` names one viewport and one scissor rectangle over the
            // colour target, and live buffers of this device, which the pass
            // keeps alive, with the uses of their roles by the checks of the
            // setters. slang-rhi copies the state and keeps a reference to
            // each buffer.
            unsafe { IRenderPassEncoder::setRenderState(pass, &raw const state) }
        });
        if index_count.is_some() {
            guard::call("IRenderPassEncoder::drawIndexed", || {
                // SAFETY: the pass is live and open, a pipeline whose formats
                // equal those of the targets is bound, and the render state
                // sets a vertex buffer for each stream and an index buffer
                // that holds the indices by the checks above.
                unsafe { IRenderPassEncoder::drawIndexed(pass, &raw const args) }
            });
        } else {
            guard::call("IRenderPassEncoder::draw", || {
                // SAFETY: the pass is live and open, a pipeline whose formats
                // equal those of the targets is bound, and the render state
                // sets a vertex buffer for each stream that holds the
                // vertices or instances by the checks above.
                unsafe { IRenderPassEncoder::draw(pass, &raw const args) }
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::ptr::NonNull;

    use slang_rhi_sys::{IBuffer, IDevice, ITexture};

    use super::{
        BoundPipeline, ColourLoad, DepthLoad, PassBuffer, RenderPass, Target, check_formats,
        check_index_buffer, check_pass_buffer, check_sampled, check_targets, check_vertex_buffers,
        colour_attachment, holds, render_state, run_render_pass,
    };
    use crate::ErrorKind;
    use crate::buffer::BufferUsage;
    use crate::error::{Error, Operation};
    use crate::fake::{Event, FakeBuffer, FakeRenderPass, FakeRenderPipeline, Log};
    use crate::owned::Owned;
    use crate::render_pipeline::{StepMode, StreamLayout};
    use crate::shader_object::{BindingRange, BoundTexture, RangeKind, ShaderOffset, TextureType};
    use crate::texture::{Format, TextureUsage};

    fn device(address: usize) -> NonNull<IDevice> {
        NonNull::new(std::ptr::without_provenance_mut(address)).expect("non-zero")
    }

    fn texture(address: usize) -> NonNull<ITexture> {
        NonNull::new(std::ptr::without_provenance_mut(address)).expect("non-zero")
    }

    fn colour(size: [u32; 2]) -> Target {
        Target {
            texture: texture(64),
            format: Format::Rgba8Unorm,
            usage: TextureUsage::RENDER_TARGET | TextureUsage::COPY_SOURCE,
            size,
        }
    }

    fn depth(size: [u32; 2]) -> Target {
        Target {
            texture: texture(128),
            format: Format::D32Float,
            usage: TextureUsage::DEPTH_STENCIL,
            size,
        }
    }

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn the_load_operations_of_render_passes_are_send_and_sync() {
        assert_send_sync::<ColourLoad>();
        assert_send_sync::<DepthLoad>();
    }

    #[test]
    fn colour_attachment_writes_every_field_and_stores_the_target() {
        let view = NonNull::new(std::ptr::without_provenance_mut(32)).expect("non-zero");
        let cleared = colour_attachment(view, ColourLoad::Clear([0.25, 0.5, 0.75, 1.0]));
        assert_eq!(cleared.view, view.as_ptr());
        assert!(cleared.resolveTarget.is_null());
        assert_eq!(cleared.loadOp, slang_rhi_sys::LoadOp::Clear);
        assert_eq!(cleared.storeOp, slang_rhi_sys::StoreOp::Store);
        assert_eq!(
            cleared.clearValue.map(f32::to_bits),
            [0.25_f32, 0.5, 0.75, 1.0].map(f32::to_bits)
        );
        let loaded = colour_attachment(view, ColourLoad::Load);
        assert_eq!(loaded.loadOp, slang_rhi_sys::LoadOp::Load);
        assert_eq!(loaded.storeOp, slang_rhi_sys::StoreOp::Store);
        assert_eq!(loaded.clearValue.map(f32::to_bits), [0.0_f32.to_bits(); 4]);
    }

    #[test]
    fn check_targets_accepts_a_colour_target_a_depth_target_or_both() {
        check_targets(Some(colour([4, 4])), None, None).expect("colour alone");
        check_targets(None, Some(depth([4, 4])), Some(DepthLoad::Clear(0.0))).expect("depth alone");
        check_targets(
            Some(colour([4, 4])),
            Some(depth([4, 4])),
            Some(DepthLoad::Clear(1.0)),
        )
        .expect("both");
        check_targets(
            Some(colour([4, 4])),
            Some(depth([4, 4])),
            Some(DepthLoad::Load),
        )
        .expect("a loaded depth");
    }

    #[test]
    fn check_targets_refuses_each_target_that_breaks_a_rule() {
        let no_use = Target {
            usage: TextureUsage::COPY_SOURCE,
            ..colour([4, 4])
        };
        let depth_as_colour = Target {
            format: Format::D32Float,
            ..colour([4, 4])
        };
        let colour_as_depth = Target {
            format: Format::Rgba8Unorm,
            ..depth([4, 4])
        };
        let depth_without_use = Target {
            usage: TextureUsage::COPY_SOURCE,
            ..depth([4, 4])
        };
        let cases = [
            (None, None, None),
            (Some(no_use), None, None),
            (Some(depth_as_colour), None, None),
            (None, Some(colour_as_depth), None),
            (None, Some(depth_without_use), None),
            (Some(colour([4, 4])), Some(depth([4, 2])), None),
            (None, Some(depth([4, 4])), Some(DepthLoad::Clear(1.5))),
            (None, Some(depth([4, 4])), Some(DepthLoad::Clear(-0.5))),
            (None, Some(depth([4, 4])), Some(DepthLoad::Clear(f32::NAN))),
        ];
        for (colour, depth, load) in cases {
            let error = check_targets(colour, depth, load).expect_err("refused");
            assert_eq!(
                error.kind(),
                ErrorKind::InvalidArgument,
                "{colour:?} {depth:?} {load:?}"
            );
            assert_eq!(error.operation(), Operation::BeginRenderPass);
        }
    }

    #[test]
    fn check_formats_requires_the_formats_of_the_pass() {
        check_formats(Some(Format::Rgba8Unorm), None, (Format::Rgba8Unorm, None)).expect("equal");
        check_formats(
            Some(Format::Rgba8Unorm),
            Some(Format::D32Float),
            (Format::Rgba8Unorm, Some(Format::D32Float)),
        )
        .expect("equal with depth");
        for (colour, depth, pipeline) in [
            (
                Some(Format::Rgba8UnormSrgb),
                None,
                (Format::Rgba8Unorm, None),
            ),
            (
                None,
                Some(Format::D32Float),
                (Format::Rgba8Unorm, Some(Format::D32Float)),
            ),
            (
                Some(Format::Rgba8Unorm),
                Some(Format::D32Float),
                (Format::Rgba8Unorm, None),
            ),
            (
                Some(Format::Rgba8Unorm),
                None,
                (Format::Rgba8Unorm, Some(Format::D32Float)),
            ),
        ] {
            let error = check_formats(colour, depth, pipeline).expect_err("refused");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert_eq!(error.operation(), Operation::Draw);
        }
    }

    #[test]
    fn check_sampled_refuses_a_target_that_a_shader_object_binds() {
        let targets = [colour([4, 4]), depth([4, 4])];
        check_sampled(&[texture(256)], &targets).expect("another texture");
        check_sampled(&[], &targets).expect("no texture");
        for sampled in [texture(64), texture(128)] {
            let error = check_sampled(&[texture(256), sampled], &targets).expect_err("a target");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        }
    }

    #[test]
    fn holds_counts_the_last_attribute_and_not_the_last_stride() {
        assert!(holds(0, 0, 16, 16));
        assert!(holds(12, 3, 4, 4));
        assert!(!holds(11, 3, 4, 4));
        assert!(holds(20, 2, 12, 8));
        assert!(!holds(19, 2, 12, 8));
        assert!(holds(16, u32::MAX, 0, 16));
        assert!(!holds(15, 1, 0, 16));
        assert!(holds(u64::MAX, u32::MAX, u32::MAX, u32::MAX));
        assert!(!holds(1 << 40, u32::MAX, u32::MAX, u32::MAX));
    }

    const STREAMS: [StreamLayout; 2] = [
        StreamLayout {
            stride: 8,
            step: StepMode::Vertex,
            attribute_size: 8,
        },
        StreamLayout {
            stride: 16,
            step: StepMode::Instance,
            attribute_size: 16,
        },
    ];

    #[test]
    fn check_vertex_buffers_counts_vertices_and_instances_by_the_step_of_each_stream() {
        check_vertex_buffers(&STREAMS, &[24, 32], Some(3), 2).expect("both fit");
        check_vertex_buffers(&STREAMS, &[0, 32], None, 2).expect("indices decide the vertices");
        check_vertex_buffers(&[], &[], Some(3), 1).expect("no stream");
        for (sizes, vertices, instances) in [
            (&[24][..], Some(3), 2),
            (&[23, 32][..], Some(3), 2),
            (&[24, 31][..], Some(3), 2),
            (&[24, 32][..], None, 3),
        ] {
            let error =
                check_vertex_buffers(&STREAMS, sizes, vertices, instances).expect_err("refused");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{sizes:?}");
            assert_eq!(error.operation(), Operation::Draw);
        }
    }

    #[test]
    fn check_index_buffer_requires_a_buffer_that_holds_the_indices() {
        check_index_buffer(Some(24), 6).expect("it fits");
        check_index_buffer(Some(0), 0).expect("no index");
        for (size, count) in [(None, 3), (Some(23), 6)] {
            let error = check_index_buffer(size, count).expect_err("refused");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        }
    }

    #[test]
    fn check_pass_buffer_requires_the_device_and_the_use() {
        check_pass_buffer(
            device(8),
            device(8),
            BufferUsage::VERTEX_BUFFER | BufferUsage::COPY_SOURCE,
            BufferUsage::VERTEX_BUFFER,
            Operation::SetVertexBuffers,
        )
        .expect("valid");
        for (buffer_device, usage) in [
            (device(16), BufferUsage::VERTEX_BUFFER),
            (device(8), BufferUsage::INDEX_BUFFER),
        ] {
            let error = check_pass_buffer(
                buffer_device,
                device(8),
                usage,
                BufferUsage::VERTEX_BUFFER,
                Operation::SetVertexBuffers,
            )
            .expect_err("refused");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert_eq!(error.operation(), Operation::SetVertexBuffers);
        }
    }

    #[test]
    fn render_state_covers_the_target_with_one_viewport_and_one_scissor() {
        let log = Log::new();
        let vertices = FakeBuffer::create(&log, "vertices");
        let indices = FakeBuffer::create(&log, "indices");
        let state = render_state([640, 480], &[vertices], Some(indices)).expect("it fits");
        assert_eq!(state.viewportCount, 1);
        let viewport = state.viewports[0];
        assert_eq!(
            [
                viewport.originX,
                viewport.originY,
                viewport.extentX,
                viewport.extentY,
                viewport.minZ,
                viewport.maxZ
            ]
            .map(f32::to_bits),
            [0.0_f32, 0.0, 640.0, 480.0, 0.0, 1.0].map(f32::to_bits)
        );
        assert_eq!(state.scissorRectCount, 1);
        let scissor = state.scissorRects[0];
        assert_eq!(
            (scissor.minX, scissor.minY, scissor.maxX, scissor.maxY),
            (0, 0, 640, 480)
        );
        assert_eq!(state.vertexBufferCount, 1);
        assert_eq!(state.vertexBuffers[0].buffer, vertices.as_ptr());
        assert_eq!(state.vertexBuffers[0].offset, 0);
        assert_eq!(state.indexBuffer.buffer, indices.as_ptr());
        assert_eq!(state.indexFormat, slang_rhi_sys::IndexFormat::Uint32);
        let without = render_state([1, 1], &[], None).expect("it fits");
        assert!(without.indexBuffer.buffer.is_null());
        assert_eq!(without.vertexBufferCount, 0);
        let error = render_state([65536, 1], &[], None).expect_err("too large");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        // SAFETY: the test owns the reference of `create` of each fake and
        // releases each once.
        unsafe { FakeBuffer::release_raw(vertices) };
        // SAFETY: as above.
        unsafe { FakeBuffer::release_raw(indices) };
    }

    /// Runs `f` in a render pass on the fake pass `raw` with a colour target
    /// of 4 by 4 texels.
    fn with_pass<R>(
        raw: NonNull<slang_rhi_sys::IRenderPassEncoder>,
        f: impl FnOnce(&mut RenderPass<'_>) -> crate::Result<R>,
    ) -> crate::Result<R> {
        // SAFETY: the fake pass lives until the test releases it, nothing
        // else uses it during the call, and the targets are never
        // dereferenced.
        unsafe { run_render_pass(raw, device(8), Some(colour([4, 4])), None, f) }
    }

    #[test]
    fn run_render_pass_ends_the_pass_after_the_closure_returns_or_fails() {
        let log = Log::new();
        let pass = FakeRenderPass::create(&log, "pass");
        assert_eq!(with_pass(pass, |_| Ok(5)).expect("the closure succeeds"), 5);
        let error = with_pass::<()>(pass, |_| {
            Err(Error::invalid(Operation::Draw, "a failure of the closure"))
        })
        .expect_err("the closure fails");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            log.events(),
            [Event::call("pass", "end"), Event::call("pass", "end")]
        );
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeRenderPass::release_raw(pass) };
    }

    #[test]
    fn run_render_pass_ends_the_pass_before_a_panic_of_the_closure_continues() {
        let log = Log::new();
        let pass = FakeRenderPass::create(&log, "pass");
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            with_pass::<()>(pass, |_| std::panic::resume_unwind(Box::new("a panic")))
        }));
        assert!(outcome.is_err());
        assert_eq!(log.events(), [Event::call("pass", "end")]);
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeRenderPass::release_raw(pass) };
    }

    fn bound(log: &Log, streams: &[StreamLayout]) -> BoundPipeline {
        let raw = FakeRenderPipeline::create(log, "pipeline");
        BoundPipeline {
            // SAFETY: the new fake pipeline carries one reference, which
            // `adopt` takes.
            raw: unsafe { Owned::adopt(raw.as_ptr()) }.expect("non-null"),
            formats: (Format::Rgba8Unorm, None),
            streams: streams.to_vec(),
        }
    }

    fn pass_buffer(log: &Log, name: &'static str, size: u64) -> PassBuffer {
        let raw = FakeBuffer::create(log, name);
        PassBuffer {
            // SAFETY: the new fake buffer carries one reference, which
            // `adopt` takes.
            raw: unsafe { Owned::<IBuffer>::adopt(raw.as_ptr()) }.expect("non-null"),
            size,
        }
    }

    #[test]
    fn a_draw_sets_the_render_state_and_then_draws_with_the_counts() {
        let log = Log::new();
        let pass = FakeRenderPass::create(&log, "pass");
        let stream = [STREAMS[0]];
        with_pass(pass, |pass| {
            drop(pass.bind(bound(&log, &stream))?);
            pass.vertex_buffers = vec![pass_buffer(&log, "vertices", 24)];
            pass.draw(3, 2)?;
            pass.index_buffer = Some(pass_buffer(&log, "indices", 24));
            pass.draw_indexed(6, 1)
        })
        .expect("both draws are recorded");
        let events: Vec<Event> = log
            .events()
            .into_iter()
            .filter(|event| {
                !matches!(
                    event,
                    Event::AddRef { .. } | Event::Release { .. } | Event::Freed { .. }
                )
            })
            .collect();
        assert_eq!(
            events,
            [
                Event::bind_render_pipeline("pass", "pipeline"),
                Event::render_state("pass", [4.0, 4.0], &["vertices"], None),
                Event::draw("pass", false, 3, 2),
                Event::render_state("pass", [4.0, 4.0], &["vertices"], Some("indices")),
                Event::draw("pass", true, 6, 1),
                Event::call("pass", "end"),
            ]
        );
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeRenderPass::release_raw(pass) };
    }

    #[test]
    fn a_draw_without_a_pipeline_or_buffers_or_after_a_failed_bind_fails_before_any_call() {
        let log = Log::new();
        let pass = FakeRenderPass::create(&log, "pass");
        let stream = [STREAMS[0]];
        let errors = with_pass(pass, |pass| {
            let unbound = pass.draw(3, 1).expect_err("nothing is bound");
            drop(pass.bind(bound(&log, &stream))?);
            let missing = pass.draw(3, 1).expect_err("no vertex buffer");
            pass.vertex_buffers = vec![pass_buffer(&log, "vertices", 16)];
            let short = pass.draw(3, 1).expect_err("the buffer is too short");
            let no_index = pass.draw_indexed(3, 1).expect_err("no index buffer");
            FakeRenderPass::answer_binds_with_null(pass.raw);
            pass.bind(bound(&log, &stream))
                .map(drop)
                .expect_err("the fake returns no root object");
            let after = pass.draw(2, 1).expect_err("the last bind failed");
            Ok([unbound, missing, short, no_index, after].map(|error| error.kind()))
        })
        .expect("the closure returns the errors");
        assert_eq!(errors, [ErrorKind::InvalidArgument; 5]);
        let calls: Vec<Event> = log
            .events()
            .into_iter()
            .filter(|event| {
                matches!(
                    event,
                    Event::Call { .. } | Event::Draw { .. } | Event::RenderState { .. }
                )
            })
            .collect();
        assert_eq!(calls, [Event::call("pass", "end")]);
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeRenderPass::release_raw(pass) };
    }

    #[test]
    fn a_texture_bound_on_a_root_object_stays_recorded_after_a_rebind_and_a_draw_refuses_it_as_the_target()
     {
        let log = Log::new();
        let pass = FakeRenderPass::create(&log, "pass");
        let view = FakeBuffer::create(&log, "view");
        let range = BindingRange {
            binding_count: 1,
            kind: RangeKind::ReadOnlyTexture(TextureType {
                plain_2d: true,
                float: true,
            }),
        };
        let error = with_pass(pass, |pass| {
            let mut root = pass.bind(bound(&log, &[]))?;
            root.bind_texture(
                ShaderOffset::new(0, 0, 0, 1),
                Some(range),
                BoundTexture {
                    texture: texture(64),
                    view: view.cast(),
                    device: device(8),
                    usage: TextureUsage::SHADER_RESOURCE,
                },
            )?;
            drop(root);
            drop(pass.bind(bound(&log, &[]))?);
            pass.draw(3, 1)
        })
        .expect_err("the target is sampled");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::Draw);
        let events: Vec<Event> = log
            .events()
            .into_iter()
            .filter(|event| {
                !matches!(
                    event,
                    Event::AddRef { .. } | Event::Release { .. } | Event::Freed { .. }
                )
            })
            .collect();
        assert_eq!(
            events,
            [
                Event::bind_render_pipeline("pass", "pipeline"),
                Event::set_binding("root", (0, 0, 0), "view", 2),
                Event::bind_render_pipeline("pass", "pipeline"),
                Event::call("pass", "end"),
            ]
        );
        assert!(
            log.events().contains(&Event::freed("root")),
            "the second bind frees the first root object"
        );
        // SAFETY: the test owns the reference of `create` of each fake and
        // releases each once. The pass releases its root object, which
        // releases the view, when the pass is freed.
        unsafe { FakeRenderPass::release_raw(pass) };
        // SAFETY: as above.
        unsafe { FakeBuffer::release_raw(view) };
    }

    #[test]
    fn a_draw_refuses_a_target_that_a_shader_object_of_the_pass_bound() {
        let log = Log::new();
        let pass = FakeRenderPass::create(&log, "pass");
        let error = with_pass(pass, |pass| {
            drop(pass.bind(bound(&log, &[]))?);
            pass.textures.borrow_mut().push(texture(64));
            pass.draw(3, 1)
        })
        .expect_err("the target is sampled");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::Draw);
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeRenderPass::release_raw(pass) };
    }
}
