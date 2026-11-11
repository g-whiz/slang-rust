use std::fmt;

use crate::buffer::Buffer;
use crate::call::{
    self, Argument, CallArgument, Parameters, Plan, Subject, buffer_argument, insert,
    sampler_argument, texture_argument,
};
use crate::device::scoped;
use crate::error::Result;
use crate::layout::Scalar;
use crate::marshal::{self, Element, Value, ValueType, VertexElement};
use crate::render::{PipelineSnapshot, RenderPipeline};
use crate::sampler::Sampler;
use crate::texture::{Texture, TextureFormat};

/// One draw with a [`RenderPipeline`], which collects the arguments of its
/// two entry points, its vertex buffers, and its targets, and then draws
/// triangles into the colour target on the GPU.
///
/// Get one from [`RenderPipeline::draw`]. Give each parameter of the vertex
/// and fragment entry points, which share the global parameters of the
/// module, by its parameter path, as for a [`Call`](crate::Call):
///
/// - [`Draw::set`] sets a plain value, a scalar, vector, matrix, or array
///   that the entry points declare as a global `uniform`, or as a field of a
///   `ConstantBuffer`, `cbuffer`, or `ParameterBlock`. A matrix is given as
///   its rows, so a transform written in Rust as rows works with
///   `mul(transform, position)` in Slang.
/// - [`Draw::bind`] binds a [`Buffer`] to a `StructuredBuffer` or
///   `RWStructuredBuffer`.
/// - [`Draw::texture`] binds a [`Texture`] to a `Texture2D`.
/// - [`Draw::sampler`] binds a [`Sampler`] to a `SamplerState`.
/// - [`Draw::vertex`] gives the buffer of a vertex attribute, an input of
///   the vertex entry point such as `input.position`.
/// - [`Draw::target`] gives the colour target, the texture that the draw
///   draws into, which every draw needs, and [`Draw::clear`] clears it
///   first.
/// - [`Draw::depth`] gives the depth target, which a pipeline with a depth
///   format needs, and [`Draw::clear_depth`] clears it first.
///
/// Every setter always succeeds, and a later setter for the same parameter
/// path or vertex attribute replaces the earlier one. [`Draw::submit`] or
/// [`Draw::submit_indexed`] then checks every argument against the
/// pipeline before any work reaches the GPU, and draws. A draw keeps its
/// arguments, so it can draw again with the same arguments.
///
/// A draw borrows its pipeline, the paths, the buffers, the textures, and
/// the samplers, and is [`Send`] and [`Sync`]. Each thread that draws at the
/// same time as another uses its own draw.
///
/// # Examples
///
/// See the example of [`RenderPipeline::draw`].
pub struct Draw<'a> {
    pipeline: &'a RenderPipeline,
    arguments: Vec<(&'a str, CallArgument<'a>)>,
    vertices: Vec<(&'a str, BoundVertex<&'a ::wgpu::Buffer>)>,
    target: Option<BoundTarget<&'a ::wgpu::TextureView>>,
    clear: Option<[f32; 4]>,
    depth: Option<BoundTarget<&'a ::wgpu::TextureView>>,
    clear_depth: Option<f32>,
}

/// The buffer of a vertex attribute, with the facts that the checks of a
/// draw need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoundVertex<B> {
    pub(crate) handle: B,
    /// The Slang scalar type and the number of components of the elements
    /// of the buffer, or `None` for a buffer of a generated struct, which no
    /// vertex attribute holds.
    pub(crate) element: Option<(Scalar, u32)>,
    pub(crate) len: usize,
    pub(crate) on_device: bool,
}

/// A colour or depth target of a draw, with the facts that the checks of a
/// draw need.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BoundTarget<V> {
    pub(crate) handle: V,
    pub(crate) format: TextureFormat,
    pub(crate) size: [u32; 2],
    pub(crate) attachable: bool,
    pub(crate) on_device: bool,
}

/// The index buffer of an indexed draw, with the facts that the checks of a
/// draw need.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct BoundIndex {
    pub(crate) len: usize,
    pub(crate) on_device: bool,
}

/// How many vertices a draw draws: a number of vertices in the order of the
/// vertex buffers, or a number of indices from an index buffer.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Count {
    Vertices(u32),
    Indices { buffer: BoundIndex, count: u32 },
}

impl Count {
    fn number(self) -> u32 {
        match self {
            Self::Vertices(count) | Self::Indices { count, .. } => count,
        }
    }
}

/// The targets of a draw and the depth to which it clears the depth target,
/// with the facts that the checks of a draw need.
#[derive(Debug)]
pub(crate) struct Targets<'t, V> {
    pub(crate) colour: Option<&'t BoundTarget<V>>,
    pub(crate) depth: Option<&'t BoundTarget<V>>,
    pub(crate) clear_depth: Option<f32>,
}

/// What a draw binds: the bind groups of [`Plan`], and the vertex buffer of
/// each slot, as the index of its argument among the vertex buffers.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct DrawPlan {
    pub(crate) plan: Plan,
    pub(crate) vertex_buffers: Vec<(u32, usize)>,
}

impl RenderPipeline {
    /// Starts a draw with the pipeline, to which the returned [`Draw`] adds
    /// the arguments and the targets before it draws.
    ///
    /// # Examples
    ///
    /// This example draws a quad of two triangles through an index buffer
    /// into a target of 64 by 64 pixels. The quad covers the whole target and
    /// shows a texture of two by two pixels, red and blue, through a
    /// sampler, after the identity transform. The top-left pixel of the
    /// target shows the top-left pixel of the texture. It needs a GPU driver
    /// or a software driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{
    ///     Buffer, Device, DeviceOptions, RenderPipelineOptions, Sampler, SamplerOptions,
    ///     Texture, TextureFormat,
    /// };
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module_from_source(
    ///     "quad",
    ///     r#"
    ///         struct VIn { float2 position : POSITION; float2 uv : TEXCOORD0; };
    ///         struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };
    ///
    ///         uniform float4x4 transform;
    ///         Texture2D<float4> image;
    ///         SamplerState imageSampler;
    ///
    ///         [shader("vertex")]
    ///         VOut vsMain(VIn input)
    ///         {
    ///             VOut output;
    ///             output.position = mul(transform, float4(input.position, 0.0, 1.0));
    ///             output.uv = input.uv;
    ///             return output;
    ///         }
    ///
    ///         [shader("fragment")]
    ///         float4 fsMain(VOut input) : SV_Target
    ///         {
    ///             return image.Sample(imageSampler, input.uv);
    ///         }
    ///     "#,
    /// )?;
    /// let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
    /// let corners = [[-1.0_f32, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
    /// let positions = Buffer::from_slice(&device, &corners)?;
    /// let uvs = Buffer::from_slice(&device, &[[0.0_f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]])?;
    /// let indices = Buffer::from_slice(&device, &[0_u32, 1, 2, 0, 2, 3])?;
    /// let red_and_blue = [[255, 0, 0, 255], [0, 0, 255, 255], [0, 0, 255, 255], [255, 0, 0, 255]];
    /// let image = Texture::from_pixels(&device, [2, 2], &red_and_blue)?;
    /// let sampler = Sampler::new(&device, &SamplerOptions::default())?;
    /// let target = Texture::new(&device, [64, 64], TextureFormat::Rgba8Unorm)?;
    /// let identity = [
    ///     [1.0_f32, 0.0, 0.0, 0.0],
    ///     [0.0, 1.0, 0.0, 0.0],
    ///     [0.0, 0.0, 1.0, 0.0],
    ///     [0.0, 0.0, 0.0, 1.0],
    /// ];
    /// pipeline
    ///     .draw()
    ///     .set("transform", identity)
    ///     .texture("image", &image)
    ///     .sampler("imageSampler", &sampler)
    ///     .vertex("input.position", &positions)
    ///     .vertex("input.uv", &uvs)
    ///     .target(&target)
    ///     .clear([0.0, 0.0, 0.0, 1.0])
    ///     .submit_indexed(&indices, 6, 1)?;
    /// let pixels: Vec<[u8; 4]> = target.read()?;
    /// assert_eq!(pixels[0], [255, 0, 0, 255]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn draw(&self) -> Draw<'_> {
        Draw {
            pipeline: self,
            arguments: Vec::new(),
            vertices: Vec::new(),
            target: None,
            clear: None,
            depth: None,
            clear_depth: None,
        }
    }
}

impl<'a> Draw<'a> {
    /// Sets the plain value at a parameter path to `value`.
    ///
    /// [`Value`] lists the Rust types that stand for the plain values of
    /// Slang. The draw checks that the value matches the Slang type of the
    /// parameter, and writes it where the entry points read it.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipeline::draw`].
    pub fn set<V: Value>(&mut self, path: &'a str, value: V) -> &mut Self {
        insert(
            &mut self.arguments,
            path,
            Argument::Value(marshal::marshal(&value)),
        );
        self
    }

    /// Binds a buffer to the buffer parameter at a parameter path.
    ///
    /// The draw checks that the buffer belongs to the device of the
    /// pipeline, is not empty, and has elements of the size of the elements
    /// of the parameter. When the elements of the parameter are scalars or
    /// vectors, it also checks that the elements of the buffer have their
    /// scalar type and number of components, as [`Call::bind`](crate::Call::bind)
    /// states.
    ///
    /// # Examples
    ///
    /// See the example of [`Kernel::call`](crate::Kernel::call), whose
    /// [`Call::bind`](crate::Call::bind) works the same way.
    pub fn bind<T: Element>(&mut self, path: &'a str, buffer: &'a Buffer<T>) -> &mut Self {
        let argument = buffer_argument(buffer, &self.pipeline.device);
        insert(&mut self.arguments, path, argument);
        self
    }

    /// Binds a texture to the `Texture2D` parameter at a parameter path.
    ///
    /// The draw checks that the texture belongs to the device of the
    /// pipeline and that a shader can sample it, which a texture of the
    /// depth format [`TextureFormat::Depth32Float`] cannot.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipeline::draw`].
    pub fn texture(&mut self, path: &'a str, texture: &'a Texture) -> &mut Self {
        let argument = texture_argument(texture, &self.pipeline.device);
        insert(&mut self.arguments, path, argument);
        self
    }

    /// Binds a sampler to the `SamplerState` parameter at a parameter path.
    ///
    /// The draw checks that the sampler belongs to the device of the
    /// pipeline.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipeline::draw`].
    pub fn sampler(&mut self, path: &'a str, sampler: &'a Sampler) -> &mut Self {
        let argument = sampler_argument(sampler, &self.pipeline.device);
        insert(&mut self.arguments, path, argument);
        self
    }

    /// Gives the buffer of the vertex attribute at a parameter path, such as
    /// `input.position` for the field `position` of the parameter `input` of
    /// the vertex entry point.
    ///
    /// Element `i` of the buffer is the value of the attribute for vertex
    /// `i`, or for instance `i` when the attribute steps once per instance,
    /// as [`RenderPipelineOptions::instance_attributes`](crate::RenderPipelineOptions::instance_attributes)
    /// states. The draw checks that the buffer belongs to the device of the
    /// pipeline, is not empty, and has elements of the Slang type of the
    /// attribute, such as `[f32; 2]` for a `float2` or `u32` for a `uint`.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipeline::draw`].
    pub fn vertex<T: VertexElement>(&mut self, path: &'a str, buffer: &'a Buffer<T>) -> &mut Self {
        let bound = BoundVertex {
            handle: buffer.wgpu_buffer(),
            element: element_type::<T>(),
            len: buffer.len(),
            on_device: buffer.is_on(&self.pipeline.device),
        };
        insert(&mut self.vertices, path, bound);
        self
    }

    /// Gives the colour target, the texture that the draw draws into.
    ///
    /// The draw checks that the texture belongs to the device of the
    /// pipeline, has the colour format of the pipeline, and can be drawn
    /// into. A texture from
    /// [`wgpu::texture_from_view`](crate::wgpu::texture_from_view), such as
    /// the texture of a frame of a window, is a target like any other.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipeline::draw`].
    pub fn target(&mut self, texture: &'a Texture) -> &mut Self {
        self.target = Some(bound_target(texture, self.pipeline));
        self
    }

    /// Clears the colour target to a colour, given as red, green, blue, and
    /// alpha from 0.0 to 1.0, before the draw draws into it.
    ///
    /// Without it, the draw keeps the pixels that the colour target already
    /// holds and draws over them.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipeline::draw`].
    pub fn clear(&mut self, colour: [f32; 4]) -> &mut Self {
        self.clear = Some(colour);
        self
    }

    /// Gives the depth target, a texture of the format
    /// [`TextureFormat::Depth32Float`] that holds the depth of the nearest
    /// triangle at each pixel.
    ///
    /// A pipeline whose
    /// [`RenderPipelineOptions::depth_format`](crate::RenderPipelineOptions::depth_format)
    /// is set needs a depth target, and every other pipeline rejects one. The
    /// draw checks that the texture belongs to the device of the pipeline,
    /// has the depth format of the pipeline, and has the size of the colour
    /// target.
    ///
    /// # Examples
    ///
    /// This example draws a triangle with a depth test into a colour target
    /// and a depth target of 4 by 4 pixels, and clears the depth to 1.0, the
    /// farthest depth, first. It needs a GPU driver or a software driver at
    /// run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{Buffer, Device, DeviceOptions, RenderPipelineOptions, Texture, TextureFormat};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module_from_source(
    ///     "depth",
    ///     r#"
    ///         [shader("vertex")]
    ///         float4 vsMain(float3 position : POSITION) : SV_Position
    ///         {
    ///             return float4(position, 1.0);
    ///         }
    ///
    ///         [shader("fragment")]
    ///         float4 fsMain() : SV_Target { return float4(1.0, 1.0, 1.0, 1.0); }
    ///     "#,
    /// )?;
    /// let mut options = RenderPipelineOptions::default();
    /// options.depth_format = Some(TextureFormat::Depth32Float);
    /// let pipeline = module.render_pipeline("vsMain", "fsMain", &options)?;
    /// let triangle = [[-1.0_f32, -1.0, 0.5], [3.0, -1.0, 0.5], [-1.0, 3.0, 0.5]];
    /// let positions = Buffer::from_slice(&device, &triangle)?;
    /// let target = Texture::new(&device, [4, 4], TextureFormat::Rgba8Unorm)?;
    /// let depth = Texture::new(&device, [4, 4], TextureFormat::Depth32Float)?;
    /// pipeline
    ///     .draw()
    ///     .vertex("position", &positions)
    ///     .target(&target)
    ///     .clear([0.0, 0.0, 0.0, 1.0])
    ///     .depth(&depth)
    ///     .clear_depth(1.0)
    ///     .submit(3, 1)?;
    /// assert_eq!(target.read()?, [[255; 4]; 16]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn depth(&mut self, texture: &'a Texture) -> &mut Self {
        self.depth = Some(bound_target(texture, self.pipeline));
        self
    }

    /// Clears the depth target to a depth from 0.0, the nearest, to 1.0, the
    /// farthest, before the draw tests and writes it.
    ///
    /// Without it, the draw keeps the depths that the depth target already
    /// holds. The draw checks that the depth is from 0.0 to 1.0, and so not
    /// NaN, also when the draw has no depth target, which then ignores it.
    ///
    /// # Examples
    ///
    /// See the example of [`Draw::depth`].
    pub fn clear_depth(&mut self, depth: f32) -> &mut Self {
        self.clear_depth = Some(depth);
        self
    }

    /// Draws `instances` instances of the first `vertices` vertices of the
    /// vertex buffers, three vertices for each triangle.
    ///
    /// The draw records one render pass with this one draw, which first
    /// clears the targets that [`Draw::clear`] and [`Draw::clear_depth`]
    /// clear, submits it to the GPU, and returns without waiting for it.
    /// [`Texture::read`] waits for it and returns the pixels. A count of zero
    /// draws nothing and still clears, after the arguments have been
    /// checked.
    ///
    /// # Examples
    ///
    /// See the example of [`Draw::depth`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::Error) of these kinds, before any work
    /// reaches the GPU unless the kind is
    /// [`ErrorKind::Backend`](crate::ErrorKind::Backend):
    ///
    /// - [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument)
    ///   for each mistake that [`Call::dispatch`](crate::Call::dispatch)
    ///   reports for a parameter. It is also returned when a path names no
    ///   vertex attribute, a vertex attribute that the vertex entry point
    ///   uses has no buffer, a vertex buffer belongs to another device, is
    ///   empty, or has elements of another Slang type than its attribute,
    ///   the colour target is missing, or a target belongs to another
    ///   device, has another format than the pipeline, or cannot be drawn
    ///   into. A depth target given to a pipeline without a depth format, or
    ///   missing for one with it, or of another size than the colour target,
    ///   is rejected too. So are a depth given to [`Draw::clear_depth`]
    ///   outside 0.0 to 1.0, vertex buffers of the attributes that step per
    ///   vertex, or of those that step per instance, with different lengths,
    ///   and a count of vertices or instances larger than the length of those
    ///   buffers.
    /// - [`ErrorKind::UnsupportedParameter`](crate::ErrorKind::UnsupportedParameter)
    ///   when the reflected layout of a plain value does not hold the value.
    /// - [`ErrorKind::Backend`](crate::ErrorKind::Backend) when wgpu reports
    ///   an error while it prepares or submits the work, such as when one
    ///   texture is both sampled and a target of the draw.
    pub fn submit(&self, vertices: u32, instances: u32) -> Result<()> {
        self.run(Count::Vertices(vertices), instances, None)
    }

    /// Draws `instances` instances of the vertices that the first `count`
    /// indices of `indices` select, three indices for each triangle, so
    /// that triangles can share vertices.
    ///
    /// It works as [`Draw::submit`] does. The draw does not read the
    /// indices, so an index past the end of the vertex buffers reaches the
    /// GPU, whose handling of it rests on wgpu and the driver.
    ///
    /// # Examples
    ///
    /// See the example of [`RenderPipeline::draw`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::Error) of the kinds that [`Draw::submit`]
    /// returns, and of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument)
    /// when the index buffer belongs to another device, is empty, or holds
    /// fewer than `count` indices.
    pub fn submit_indexed(&self, indices: &Buffer<u32>, count: u32, instances: u32) -> Result<()> {
        let buffer = BoundIndex {
            len: indices.len(),
            on_device: indices.is_on(&self.pipeline.device),
        };
        self.run(
            Count::Indices { buffer, count },
            instances,
            Some(indices.wgpu_buffer()),
        )
    }

    fn run(&self, count: Count, instances: u32, indices: Option<&::wgpu::Buffer>) -> Result<()> {
        let pipeline = self.pipeline;
        let plan = plan_draw(
            &pipeline.snapshot,
            &self.arguments,
            &self.vertices,
            &Targets {
                colour: self.target.as_ref(),
                depth: self.depth.as_ref(),
                clear_depth: self.clear_depth,
            },
            count,
            instances,
        )?;
        let device = pipeline.device.wgpu_device();
        let ((), error) = scoped(device, || {
            self.record(&plan, indices, count.number(), instances);
        });
        match error {
            Some(error) => Err(subject(&pipeline.snapshot).rejected(error)),
            None => Ok(()),
        }
    }

    fn record(
        &self,
        plan: &DrawPlan,
        indices: Option<&::wgpu::Buffer>,
        count: u32,
        instances: u32,
    ) {
        let pipeline = self.pipeline;
        let device = pipeline.device.wgpu_device();
        let label = Some(pipeline.snapshot.vertex.as_str());
        let bind_groups = call::bind_groups(
            device,
            &pipeline.snapshot.vertex,
            &plan.plan,
            &pipeline.bind_group_layouts,
            &self.arguments,
        );
        let colour = [self
            .target
            .as_ref()
            .map(|target| ::wgpu::RenderPassColorAttachment {
                view: target.handle,
                depth_slice: None,
                resolve_target: None,
                ops: ::wgpu::Operations {
                    load: self.clear.map_or(::wgpu::LoadOp::Load, |[r, g, b, a]| {
                        ::wgpu::LoadOp::Clear(::wgpu::Color {
                            r: f64::from(r),
                            g: f64::from(g),
                            b: f64::from(b),
                            a: f64::from(a),
                        })
                    }),
                    store: ::wgpu::StoreOp::Store,
                },
            })];
        let depth = self
            .depth
            .as_ref()
            .map(|depth| ::wgpu::RenderPassDepthStencilAttachment {
                view: depth.handle,
                depth_ops: Some(::wgpu::Operations {
                    load: self
                        .clear_depth
                        .map_or(::wgpu::LoadOp::Load, ::wgpu::LoadOp::Clear),
                    store: ::wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            });
        let mut encoder =
            device.create_command_encoder(&::wgpu::CommandEncoderDescriptor { label });
        {
            let mut pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
                label,
                color_attachments: &colour,
                depth_stencil_attachment: depth,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&pipeline.pipeline);
            for (set, group) in (0_u32..).zip(&bind_groups) {
                pass.set_bind_group(set, group, &[]);
            }
            for &(slot, index) in &plan.vertex_buffers {
                if let Some((_, bound)) = self.vertices.get(index) {
                    pass.set_vertex_buffer(slot, bound.handle.slice(..));
                }
            }
            match indices {
                Some(indices) => {
                    pass.set_index_buffer(indices.slice(..), ::wgpu::IndexFormat::Uint32);
                    pass.draw_indexed(0..count, 0, 0..instances);
                }
                None => pass.draw(0..count, 0..instances),
            }
        }
        pipeline.device.wgpu_queue().submit([encoder.finish()]);
    }
}

impl fmt::Debug for Draw<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let paths: Vec<&str> = self.arguments.iter().map(|(path, _)| *path).collect();
        let vertices: Vec<&str> = self.vertices.iter().map(|(path, _)| *path).collect();
        f.debug_struct("Draw")
            .field("vertex", &self.pipeline.snapshot.vertex)
            .field("fragment", &self.pipeline.snapshot.fragment)
            .field("arguments", &paths)
            .field("vertices", &vertices)
            .finish_non_exhaustive()
    }
}

fn bound_target<'a>(
    texture: &'a Texture,
    pipeline: &RenderPipeline,
) -> BoundTarget<&'a ::wgpu::TextureView> {
    BoundTarget {
        handle: texture.wgpu_view(),
        format: texture.format(),
        size: texture.size(),
        attachable: texture.is_attachable(),
        on_device: texture.is_on(&pipeline.device),
    }
}

fn subject(snapshot: &PipelineSnapshot) -> Subject<'_> {
    Subject::Pipeline {
        vertex: &snapshot.vertex,
        fragment: &snapshot.fragment,
    }
}

/// Returns the Slang scalar type and the number of components of the
/// elements of a `Buffer<T>`, such as `float` and 2 for `[f32; 2]`, which
/// are those of the vertex attribute that the buffer can hold, or `None`
/// when the elements are a generated struct.
pub(crate) fn element_type<T: VertexElement>() -> Option<(Scalar, u32)> {
    match T::value_type() {
        ValueType::Plain { scalar, dims } => {
            let components = dims
                .first()
                .and_then(|&length| u32::try_from(length).ok())
                .unwrap_or(1);
            Some((scalar, components))
        }
        ValueType::Structure { .. } => None,
    }
}

/// Returns the name of the Slang type of a vertex attribute, such as `float2`
/// or `uint`.
fn slang_type(scalar: Scalar, components: u32) -> String {
    let name = match scalar {
        Scalar::Bool => "bool",
        Scalar::Int32 => "int",
        Scalar::Uint32 => "uint",
        Scalar::Float32 => "float",
    };
    if components == 1 {
        name.to_owned()
    } else {
        format!("{name}{components}")
    }
}

/// Checks a draw against the snapshot of its pipeline, before any call of
/// wgpu, and returns what the draw binds.
///
/// It fails with [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument)
/// on the first failure of these checks, in this order:
///
/// 1. Each argument of a parameter, in the order given, as a call checks
///    it, and then each vertex buffer, in the order given: its path names a
///    vertex attribute, and the buffer belongs to the device, is not empty,
///    and has the attribute's scalar type and component count.
/// 2. Each used parameter that no argument gives, as a call checks it, and
///    then each used vertex attribute without a buffer, in the order of
///    locations.
/// 3. The colour target is given, belongs to the device, has the pipeline's
///    colour format, and can be drawn into. A depth target is given exactly
///    when the pipeline has a depth format, belongs to the device, has that
///    format, can be drawn into, and has the size of the colour target. The
///    depth to which the draw clears the depth target, when it is given, is
///    from 0.0 to 1.0, also when the draw has no depth target.
/// 4. The used vertex buffers that step per vertex have one length, and so
///    do those that step per instance. The index buffer of an indexed draw
///    belongs to the device and is not empty. The count of vertices, or of
///    indices, is at most the length of those buffers or of the index
///    buffer, and the count of instances is at most the length of the
///    buffers that step per instance, when there are any.
///
/// A vertex buffer for an attribute that the vertex entry point does not use
/// is checked and then ignored, as an argument for an unused parameter is.
pub(crate) fn plan_draw<B, V, S>(
    snapshot: &PipelineSnapshot,
    arguments: &[(&str, Argument<B, V, S>)],
    vertices: &[(&str, BoundVertex<B>)],
    targets: &Targets<'_, V>,
    count: Count,
    instances: u32,
) -> Result<DrawPlan> {
    let subject = subject(snapshot);
    let parameters = Parameters {
        subject,
        bindings: &snapshot.bindings,
        immediates: None,
    };
    let checked = call::check_arguments(parameters, arguments)?;
    check_vertices(snapshot, subject, vertices)?;
    let plan = call::finish(parameters, checked)?;
    let vertex_buffers = require_vertices(snapshot, subject, vertices)?;
    check_targets(snapshot, subject, targets.colour, targets.depth)?;
    check_clear_depth(subject, targets.clear_depth)?;
    check_counts(snapshot, subject, vertices, count, instances)?;
    Ok(DrawPlan {
        plan,
        vertex_buffers,
    })
}

fn check_vertices<B>(
    snapshot: &PipelineSnapshot,
    subject: Subject<'_>,
    vertices: &[(&str, BoundVertex<B>)],
) -> Result<()> {
    for (path, bound) in vertices {
        let Some(input) = snapshot
            .inputs
            .iter()
            .find(|input| input.attribute.path == *path)
        else {
            return Err(subject.invalid(format!(
                "{subject} has no vertex attribute at the path `{path}`"
            )));
        };
        if !bound.on_device {
            return Err(subject.invalid(format!(
                "the vertex buffer of the attribute `{path}` of {subject} belongs to another device"
            )));
        }
        if bound.len == 0 {
            return Err(subject.invalid(format!(
                "the vertex buffer of the attribute `{path}` of {subject} is empty"
            )));
        }
        let attribute = &input.attribute;
        let expected = slang_type(attribute.scalar, attribute.components);
        match bound.element {
            Some(element) if element == (attribute.scalar, attribute.components) => {}
            Some((scalar, components)) => {
                return Err(subject.invalid(format!(
                    "the vertex attribute `{path}` of {subject} has the type {expected}, and its vertex buffer has elements of the type {}",
                    slang_type(scalar, components)
                )));
            }
            None => {
                return Err(subject.invalid(format!(
                    "the vertex attribute `{path}` of {subject} has the type {expected}, and its vertex buffer has elements of a structure, which no vertex attribute holds"
                )));
            }
        }
    }
    Ok(())
}

/// Fails on the first used vertex attribute, in the order of locations, that
/// has no vertex buffer, and otherwise returns the slot of each used
/// attribute with the index of its buffer among `vertices`.
fn require_vertices<B>(
    snapshot: &PipelineSnapshot,
    subject: Subject<'_>,
    vertices: &[(&str, BoundVertex<B>)],
) -> Result<Vec<(u32, usize)>> {
    snapshot
        .inputs
        .iter()
        .filter_map(|input| input.slot.map(|slot| (slot, &input.attribute.path)))
        .map(|(slot, path)| {
            vertices
                .iter()
                .position(|(given, _)| *given == path)
                .map(|index| (slot, index))
                .ok_or_else(|| {
                    subject.invalid(format!(
                        "no vertex buffer is given for the vertex attribute `{path}` of {subject}"
                    ))
                })
        })
        .collect()
}

fn check_targets<V>(
    snapshot: &PipelineSnapshot,
    subject: Subject<'_>,
    target: Option<&BoundTarget<V>>,
    depth: Option<&BoundTarget<V>>,
) -> Result<()> {
    let Some(target) = target else {
        return Err(subject.invalid(format!("the draw with {subject} has no colour target")));
    };
    check_target(subject, "colour", target, snapshot.options.colour_format)?;
    match (snapshot.options.depth_format, depth) {
        (None, None) => Ok(()),
        (Some(_), None) => Err(subject.invalid(format!(
            "{subject} has a depth format, and the draw has no depth target"
        ))),
        (None, Some(_)) => Err(subject.invalid(format!(
            "{subject} has no depth format, and the draw has a depth target"
        ))),
        (Some(format), Some(depth)) => {
            check_target(subject, "depth", depth, format)?;
            if depth.size == target.size {
                return Ok(());
            }
            let [width, height] = depth.size;
            let [target_width, target_height] = target.size;
            Err(subject.invalid(format!(
                "the depth target of the draw with {subject} has {width} by {height} pixels, and the colour target has {target_width} by {target_height}"
            )))
        }
    }
}

fn check_clear_depth(subject: Subject<'_>, clear_depth: Option<f32>) -> Result<()> {
    match clear_depth {
        Some(depth) if !(0.0..=1.0).contains(&depth) => Err(subject.invalid(format!(
            "the draw with {subject} clears the depth target to {depth:?}, and a depth is from 0.0 to 1.0"
        ))),
        _ => Ok(()),
    }
}

fn check_target<V>(
    subject: Subject<'_>,
    what: &str,
    target: &BoundTarget<V>,
    format: TextureFormat,
) -> Result<()> {
    if !target.on_device {
        return Err(subject.invalid(format!(
            "the {what} target of the draw with {subject} belongs to another device"
        )));
    }
    if target.format != format {
        return Err(subject.invalid(format!(
            "the {what} target of the draw with {subject} has the format {:?}, and the pipeline draws into the format {format:?}",
            target.format
        )));
    }
    if !target.attachable {
        return Err(subject.invalid(format!(
            "the {what} target of the draw with {subject} lacks the usage RENDER_ATTACHMENT, which a draw needs to draw into it"
        )));
    }
    Ok(())
}

fn check_counts<B>(
    snapshot: &PipelineSnapshot,
    subject: Subject<'_>,
    vertices: &[(&str, BoundVertex<B>)],
    count: Count,
    instances: u32,
) -> Result<()> {
    let length = |step_mode: ::wgpu::VertexStepMode, steps: &str| -> Result<Option<usize>> {
        let mut first: Option<(&str, usize)> = None;
        for input in &snapshot.inputs {
            if input.slot.is_none() || input.step_mode != step_mode {
                continue;
            }
            let path = input.attribute.path.as_str();
            let Some((_, bound)) = vertices.iter().find(|(given, _)| *given == path) else {
                continue;
            };
            match first {
                None => first = Some((path, bound.len)),
                Some((first_path, len)) if len != bound.len => {
                    return Err(subject.invalid(format!(
                        "the vertex buffers of the attributes `{first_path}` and `{path}` of {subject}, which step once per {steps}, have the different lengths {len} and {}",
                        bound.len
                    )));
                }
                Some(_) => {}
            }
        }
        Ok(first.map(|(_, len)| len))
    };
    let per_vertex = length(::wgpu::VertexStepMode::Vertex, "vertex")?;
    let per_instance = length(::wgpu::VertexStepMode::Instance, "instance")?;
    let within = |count: u32, len: usize| usize::try_from(count).is_ok_and(|count| count <= len);
    match count {
        Count::Vertices(count) => {
            if let Some(len) = per_vertex
                && !within(count, len)
            {
                return Err(subject.invalid(format!(
                    "the draw with {subject} draws {count} vertices, and the vertex buffers that step once per vertex hold {len}"
                )));
            }
        }
        Count::Indices { buffer, count } => {
            if !buffer.on_device {
                return Err(subject.invalid(format!(
                    "the index buffer of the draw with {subject} belongs to another device"
                )));
            }
            if buffer.len == 0 {
                return Err(subject.invalid(format!(
                    "the index buffer of the draw with {subject} is empty"
                )));
            }
            if !within(count, buffer.len) {
                return Err(subject.invalid(format!(
                    "the draw with {subject} draws {count} indices, and the index buffer holds {}",
                    buffer.len
                )));
            }
        }
    }
    if let Some(len) = per_instance
        && !within(instances, len)
    {
        return Err(subject.invalid(format!(
            "the draw with {subject} draws {instances} instances, and the vertex buffers that step once per instance hold {len}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
