//! Runs Slang compute kernels and draws with Slang shaders on the GPU from
//! Rust.
//!
//! [Slang](https://shader-slang.org) is a language for shaders, the programs
//! that run on a graphics processing unit (GPU). A compute kernel is a
//! function of a shader that runs as a general computation on many threads
//! of the GPU at once. This crate is the high-level library of this
//! repository. It compiles Slang through
//! [`slang-compiler`](slang_compiler), the safe wrappers over the Slang
//! compiler, and runs the result through [wgpu](https://wgpu.rs), a
//! graphics library for Vulkan, Direct3D 12, and Metal. Use this crate to run
//! kernels and to draw, and `slang-compiler` alone to compile or inspect
//! shaders without a GPU.
//!
//! [`Device::new`] creates a [`Device`], the connection to one adapter, a
//! GPU or a software implementation of one, on the adapter that
//! [`DeviceOptions`] selects, and the module [`wgpu`] connects a [`Device`]
//! to wgpu code of your own. [`Device::load_module`] compiles a Slang module
//! and prepares each of its compute entry points as a [`Kernel`] of the
//! device, which [`Module::kernel`] looks up by name. A [`Buffer`] holds
//! elements of a type that [`VertexElement`] lists on the GPU, and a kernel
//! binds a buffer of a type that [`Element`] lists. A [`Texture`] holds an
//! image in a [`TextureFormat`], and a [`Sampler`], created from
//! [`SamplerOptions`], states how a kernel reads a texture. [`Kernel::call`]
//! starts a [`Call`], which sets the plain values of the kernel by name,
//! from the Rust types that [`Value`] lists, binds buffers, textures, and
//! samplers by name, checks every argument against the parameters of the
//! kernel, and runs the kernel on the GPU. [`Buffer::read`] waits for the
//! kernel and copies the results back, and [`Texture::read`] copies the
//! pixels of a texture back. Every failure is an [`Error`], whose
//! [`Error::kind`] says what went wrong.
//!
//! A kernel is a function of the module marked `[shader("compute")]`, and
//! the crate supports these parameters of a kernel:
//!
//! - `StructuredBuffer<T>` and `RWStructuredBuffer<T>`, to which a call binds
//!   a [`Buffer`] of the matching element type.
//! - `Texture2D` with a `float` result, such as `Texture2D<float4>`, to which
//!   a call binds a [`Texture`], and `SamplerState`, to which a call binds a
//!   [`Sampler`]. A kernel has no derivatives, so it samples a texture with
//!   an explicit level of detail, such as `image.SampleLevel(sampler, uv,
//!   0)`, or reads a pixel with `image.Load`.
//! - Plain values of the types `bool`, `int`, `uint`, and `float`, as
//!   scalars, vectors, matrices, arrays of fixed length, and structures of
//!   them, declared as `uniform` globals, as `uniform` parameters of the
//!   entry point, or as fields of a `ConstantBuffer`, a `cbuffer`, or a
//!   `ParameterBlock`, which a call sets.
//! - System values such as `SV_DispatchThreadID`, which the GPU provides.
//!
//! A load fails with [`ErrorKind::UnsupportedParameter`] when a kernel has
//! any other parameter, such as a storage texture, a `Sampler2D`, a
//! `SamplerComparisonState`, a `ByteAddressBuffer`, a `double`, or an array
//! of structures. It fails with
//! [`ErrorKind::UnsupportedShader`] when wgpu rejects a construct of the
//! compiled kernel, and with [`ErrorKind::MissingFeature`] when a kernel has
//! `uniform` parameters of its entry point and the device cannot receive
//! them. The crate loads every kernel through wgpu's validated path and
//! contains no unsafe code.
//!
//! A module may also declare vertex and fragment entry points, marked
//! `[shader("vertex")]` and `[shader("fragment")]`. [`Module::render_pipeline`]
//! creates a [`RenderPipeline`] from one of each, with the colour format, the
//! optional depth format, the [`Blend`], the [`CullMode`], and the vertex
//! attributes that step per instance that [`RenderPipelineOptions`] give.
//! [`RenderPipeline::draw`] starts a [`Draw`], which sets the parameters of
//! the two entry points by name as a call does, gives a [`Buffer`] for each
//! vertex attribute, a colour target, and a depth target when the pipeline
//! has a depth format, checks all of them against the pipeline, and draws
//! triangles into the colour target, through an index buffer or without one,
//! and in one or more instances. [`Texture::read`] then copies the pixels
//! back, and a later draw or call can sample the target as a texture.
//!
//! The two entry points of a render pipeline support these parameters:
//!
//! - The parameters of a kernel above, except `uniform` parameters of an
//!   entry point. The two entry points share the global parameters,
//!   constant buffers, parameter blocks, textures, and samplers of the
//!   module, and each binding is visible to the entry points that use it.
//! - Vertex attributes, the inputs of the vertex entry point of the types
//!   `int`, `uint`, and `float` and their vectors of 2 to 4 components, as
//!   separate parameters or as the fields of a structure, to which a draw
//!   gives a [`Buffer`] of the matching [`VertexElement`], such as
//!   `[f32; 3]` for a `float3`. An integer varying, an output of the vertex
//!   entry point, needs `nointerpolation`.
//! - System values such as `SV_Position`, `SV_VulkanVertexID`, and
//!   `SV_VulkanInstanceID`, which the GPU provides. `SV_VertexID` and
//!   `SV_InstanceID` are rejected by wgpu.
//!
//! The creation of a render pipeline fails with
//! [`ErrorKind::UnsupportedParameter`] when an entry point has any other
//! parameter or `uniform` parameters of its own, and with
//! [`ErrorKind::UnsupportedShader`] when wgpu rejects an entry point or the
//! two together, such as when their varyings do not match.
//!
//! Drawing has these limits in this version:
//!
//! - One draw is one render pass with one draw command, which clears the
//!   targets first or keeps their contents.
//! - A draw has one colour target, a [`Texture`] of a colour format of
//!   [`TextureFormat`], and at most one depth target of the format
//!   [`TextureFormat::Depth32Float`].
//! - Each vertex attribute takes its own vertex buffer, at offset 0 with
//!   the size of one element as its stride, so the device's limit on vertex
//!   buffers, 8 by default, bounds the attributes that a vertex entry point
//!   uses.
//! - Every pipeline draws a list of triangles with one sample per pixel and
//!   no multisampling.
//!
//! The draws follow the conventions of wgpu on every backend. The y axis of
//! a position in clip space points up, so (-1, 1) is the upper-left corner
//! of the target, and a depth from 0.0, the nearest, to 1.0, the farthest,
//! is drawn, while a depth outside that range is clipped. A triangle whose
//! vertices appear counter-clockwise on the target faces the viewer. The
//! texture coordinates (0, 0) address the upper-left pixel of a texture. A
//! matrix is set as its rows, so a transform written in Rust as rows works
//! with `mul(transform, position)` in Slang.
//!
//! The crate draws into a texture and owns no window. A program that draws
//! into a window configures the surface of the window with wgpu itself,
//! wraps the view of the texture of each frame as a [`Texture`] with
//! [`wgpu::texture_from_view`], draws into it, and presents the frame.
//!
//! Every type of the crate is [`Send`] and [`Sync`], and a [`Device`], a
//! [`Module`], a [`Kernel`], a [`RenderPipeline`], a [`Buffer`], a
//! [`Texture`], and a [`Sampler`] are cheap to clone.
//!
//! # Examples
//!
//! This example creates a device on the adapter that wgpu chooses, loads the
//! module in `scale.slang`, and runs its kernel `scale` over a buffer of four
//! numbers. The file holds this Slang source:
//!
//! ```slang
//! RWStructuredBuffer<float> data;
//! uniform float factor;
//! uniform uint count;
//!
//! [shader("compute")]
//! [numthreads(64, 1, 1)]
//! void scale(uint3 id : SV_DispatchThreadID)
//! {
//!     if (id.x < count) { data[id.x] *= factor; }
//! }
//! ```
//!
//! The program sets the two uniforms, binds the buffer, runs the kernel on
//! four threads, and reads the result back. It needs a GPU driver or a
//! software driver at run time, so it is compiled but not run:
//!
//! ```no_run
//! use slango::{Buffer, Device, DeviceOptions};
//!
//! fn main() -> slango::Result<()> {
//!     let device = Device::new(&DeviceOptions::default())?;
//!     let module = device.load_module("/tmp/shaders/scale.slang")?;
//!     let scale = module.kernel("scale")?;
//!     let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
//!     scale
//!         .call()
//!         .set("factor", 2.0_f32)
//!         .set("count", 4_u32)
//!         .bind("data", &data)
//!         .dispatch([4, 1, 1])?;
//!     let result: Vec<f32> = data.read()?;
//!     assert_eq!(result, [2.0, 4.0, 6.0, 8.0]);
//!     Ok(())
//! }
//! ```
//!
//! The example `compute-kernel` of the package `slang-rust-examples` in the
//! repository of this crate is a complete program of the same flow. It runs
//! a kernel with two input buffers, an output buffer, and two `uniform`
//! values, and checks its results against the same computation on the CPU.
//!
//! This example draws a texture of two by two pixels onto a quad that
//! covers a target of 64 by 64 pixels. The file `quad.slang` holds this
//! Slang source:
//!
//! ```slang
//! struct VIn { float2 position : POSITION; float2 uv : TEXCOORD0; };
//! struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };
//!
//! uniform float4x4 transform;
//! Texture2D<float4> image;
//! SamplerState imageSampler;
//!
//! [shader("vertex")]
//! VOut vsMain(VIn input)
//! {
//!     VOut output;
//!     output.position = mul(transform, float4(input.position, 0.0, 1.0));
//!     output.uv = input.uv;
//!     return output;
//! }
//!
//! [shader("fragment")]
//! float4 fsMain(VOut input) : SV_Target
//! {
//!     return image.Sample(imageSampler, input.uv);
//! }
//! ```
//!
//! The program creates the pipeline, the vertex buffers, the index buffer,
//! the texture, the sampler, and the target, draws the two triangles of the
//! quad through the index buffer with the identity transform, and reads the
//! target back. Its upper-left pixel shows the upper-left pixel of the
//! texture, which is red. It needs a GPU driver or a software driver at run
//! time, so it is compiled but not run:
//!
//! ```no_run
//! use slango::{
//!     Buffer, Device, DeviceOptions, RenderPipelineOptions, Sampler, SamplerOptions, Texture,
//!     TextureFormat,
//! };
//!
//! fn main() -> slango::Result<()> {
//!     let device = Device::new(&DeviceOptions::default())?;
//!     let module = device.load_module("/tmp/shaders/quad.slang")?;
//!     let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
//!     let corners = [[-1.0_f32, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
//!     let positions = Buffer::from_slice(&device, &corners)?;
//!     let uvs = Buffer::from_slice(&device, &[[0.0_f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]])?;
//!     let indices = Buffer::from_slice(&device, &[0_u32, 1, 2, 0, 2, 3])?;
//!     let red_and_blue = [[255, 0, 0, 255], [0, 0, 255, 255], [0, 0, 255, 255], [255, 0, 0, 255]];
//!     let image = Texture::from_pixels(&device, [2, 2], &red_and_blue)?;
//!     let sampler = Sampler::new(&device, &SamplerOptions::default())?;
//!     let target = Texture::new(&device, [64, 64], TextureFormat::Rgba8Unorm)?;
//!     let identity = [
//!         [1.0_f32, 0.0, 0.0, 0.0],
//!         [0.0, 1.0, 0.0, 0.0],
//!         [0.0, 0.0, 1.0, 0.0],
//!         [0.0, 0.0, 0.0, 1.0],
//!     ];
//!     pipeline
//!         .draw()
//!         .set("transform", identity)
//!         .texture("image", &image)
//!         .sampler("imageSampler", &sampler)
//!         .vertex("input.position", &positions)
//!         .vertex("input.uv", &uvs)
//!         .target(&target)
//!         .clear([0.0, 0.0, 0.0, 1.0])
//!         .submit_indexed(&indices, 6, 1)?;
//!     let pixels: Vec<[u8; 4]> = target.read()?;
//!     assert_eq!(pixels[0], [255, 0, 0, 255]);
//!     Ok(())
//! }
//! ```
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Platform support
//!
//! The crate supports the platforms of `slang-compiler`, which are Linux on
//! x86-64, Windows on x86-64, and macOS on Apple silicon and on x86-64. It
//! searches the adapters of whichever of Vulkan, Direct3D 12, and Metal wgpu
//! offers on the platform. It creates devices, runs kernels, and draws on
//! Vulkan on Linux, on Direct3D 12 on Windows, and on Metal on macOS, and
//! its tests run in continuous integration on lavapipe on Linux, on the
//! Windows Advanced Rasterization Platform (WARP) on Windows, and on Metal
//! on macOS. On macOS, Metal offers no software adapter, so
//! [`AdapterSelection::Software`] finds none there.
//!
//! # Finding the Slang library
//!
//! `slang-compiler` links the Slang library through `slang-compiler-sys`, as
//! the [crate documentation of `slang-compiler`](slang_compiler) describes.
//! By default, that is the Slang shared library of the pinned Vulkan SDK,
//! which the dynamic loader has to find when the program starts.

mod buffer;
mod call;
mod device;
mod draw;
mod error;
mod kernel;
mod layout;
mod marshal;
mod module;
mod render;
mod sampler;
mod texture;
pub mod wgpu;

pub use buffer::Buffer;
pub use call::Call;
pub use device::{AdapterSelection, Device, DeviceOptions};
pub use draw::Draw;
pub use error::{Error, ErrorKind, Result};
pub use kernel::Kernel;
pub use marshal::{Element, Value, VertexElement};
pub use module::Module;
pub use render::{Blend, CullMode, RenderPipeline, RenderPipelineOptions};
pub use sampler::{AddressMode, FilterMode, Sampler, SamplerOptions};
pub use texture::{Texture, TextureFormat};
