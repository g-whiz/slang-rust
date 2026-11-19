//! Runs Slang compute kernels and draws with Slang shaders on the GPU from
//! Rust.
//!
//! [Slang](https://shader-slang.org) is a language for shaders, the programs
//! that run on a graphics processing unit (GPU). This crate is the
//! high-level library of this repository. It compiles Slang through
//! [`slang-compiler`](slang_compiler), the safe wrappers over the Slang
//! compiler, and runs the result on the GPU through one of two backends:
//! [wgpu](https://wgpu.rs), a graphics library for Vulkan, Direct3D 12, and
//! Metal, by default, or slang-rhi, the render hardware interface of Slang,
//! with the feature `rhi`. Use this crate
//! to run compute kernels and to draw triangles into a texture, and
//! `slang-compiler` alone to compile or inspect shaders without a GPU.
//!
//! [`Device::new`] creates a [`Device`], the connection to one adapter, a
//! GPU or a software implementation of one, on the adapter that
//! [`DeviceOptions`] selects. [`Device::load_module`] compiles a Slang module
//! and prepares each of its compute entry points as a [`Kernel`], which
//! [`Module::kernel`] looks up by name, and [`Module::render_pipeline`]
//! creates a [`RenderPipeline`] from a vertex and a fragment entry point of
//! the module. A [`Buffer`] holds elements on the GPU, a [`Texture`] holds an
//! image in a [`TextureFormat`], and a [`Sampler`] states how a shader reads
//! a texture. [`Kernel::call`] starts a [`Call`] and [`RenderPipeline::draw`]
//! starts a [`Draw`]. Each sets the parameters of its shader by name, checks
//! every argument against them before any work reaches the GPU, and then
//! runs the kernel or draws. [`Buffer::read`] and [`Texture::read`] wait for
//! the GPU and copy the results back. Every failure is an [`Error`], whose
//! [`Error::kind`] says what went wrong.
//!
//! A [`Device`], a [`Module`], a [`Kernel`], a [`RenderPipeline`], a
//! [`Buffer`], a [`Texture`], a [`Sampler`], a [`Call`], and a [`Draw`] hold
//! GPU objects, so none of them is [`Send`] or [`Sync`], and each stays on
//! the thread that created its [`Device`]. A program sends results to other
//! threads as plain data. An [`Error`], an [`ErrorKind`], the options
//! structs, and the enumerations are [`Send`] and [`Sync`], and a [`Device`],
//! a [`Module`], a [`Kernel`], a [`RenderPipeline`], a [`Buffer`], a
//! [`Texture`], and a [`Sampler`] are cheap to clone. With the feature
//! `wgpu`, the crate loads every shader through wgpu's validated path. It
//! contains no unsafe code.
//!
//! # Examples
//!
//! This example creates a device on the adapter that the backend chooses,
//! loads the module in `scale.slang`, and runs its kernel `scale` over a
//! buffer of four numbers. The file holds this Slang source:
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
//! let device = Device::new(&DeviceOptions::default())?;
//! let module = device.load_module("/tmp/shaders/scale.slang")?;
//! let scale = module.kernel("scale")?;
//! let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
//! scale
//!     .call()
//!     .set("factor", 2.0_f32)
//!     .set("count", 4_u32)
//!     .bind("data", &data)
//!     .dispatch([4, 1, 1])?;
//! let result: Vec<f32> = data.read()?;
//! assert_eq!(result, [2.0, 4.0, 6.0, 8.0]);
//! # Ok::<(), slango::Error>(())
//! ```
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
//! let device = Device::new(&DeviceOptions::default())?;
//! let module = device.load_module("/tmp/shaders/quad.slang")?;
//! let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
//! let corners = [[-1.0_f32, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
//! let positions = Buffer::from_slice(&device, &corners)?;
//! let uvs = Buffer::from_slice(&device, &[[0.0_f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]])?;
//! let indices = Buffer::from_slice(&device, &[0_u32, 1, 2, 0, 2, 3])?;
//! let red_and_blue = [[255, 0, 0, 255], [0, 0, 255, 255], [0, 0, 255, 255], [255, 0, 0, 255]];
//! let image = Texture::from_pixels(&device, [2, 2], &red_and_blue)?;
//! let sampler = Sampler::new(&device, &SamplerOptions::default())?;
//! let target = Texture::new(&device, [64, 64], TextureFormat::Rgba8Unorm)?;
//! let identity = [
//!     [1.0_f32, 0.0, 0.0, 0.0],
//!     [0.0, 1.0, 0.0, 0.0],
//!     [0.0, 0.0, 1.0, 0.0],
//!     [0.0, 0.0, 0.0, 1.0],
//! ];
//! pipeline
//!     .draw()
//!     .set("transform", identity)
//!     .texture("image", &image)
//!     .sampler("imageSampler", &sampler)
//!     .vertex("input.position", &positions)
//!     .vertex("input.uv", &uvs)
//!     .target(&target)
//!     .clear([0.0, 0.0, 0.0, 1.0])
//!     .submit_indexed(&indices, 6, 1)?;
//! let pixels: Vec<[u8; 4]> = target.read()?;
//! assert_eq!(pixels[0], [255, 0, 0, 255]);
//! # Ok::<(), slango::Error>(())
//! ```
//!
//! The package `slang-rust-examples` in the repository of this crate holds
//! five complete programs:
//!
//! - `compile-and-reflect` compiles a Slang file and prints its entry points
//!   and parameters with `slang-compiler` alone.
//! - `compute-kernel` runs a kernel with two input buffers, an output buffer,
//!   and two `uniform` values, and checks its results against the same
//!   computation on the CPU.
//! - `windowed-renderer` draws a rotating textured quad into a window and
//!   loads its shader again whenever the file changes.
//! - `shader-playground` shows what a shader draws in a Slint window with two
//!   sliders, through the crate `slang-slint`, and also reloads its shader.
//! - `typed-kernel` runs a kernel through the typed bindings that the macro
//!   `shader!` of the crate `slango-macros` generates, and checks its
//!   results.
//!
//! # Parameters
//!
//! A kernel is a function of the module marked `[shader("compute")]`, and
//! the crate supports these parameters of a kernel:
//!
//! - `StructuredBuffer<T>` and `RWStructuredBuffer<T>`, to which a call binds
//!   a [`Buffer`] of the matching [`Element`] type.
//! - `Texture2D` with a `float` result, such as `Texture2D<float4>`, to which
//!   a call binds a [`Texture`], and `SamplerState`, to which a call binds a
//!   [`Sampler`]. A kernel has no derivatives, so it samples a texture with
//!   an explicit level of detail, such as `image.SampleLevel(sampler, uv,
//!   0)`, or reads a pixel with `image.Load`.
//! - Plain values of the types `bool`, `int`, `uint`, and `float`, as
//!   scalars, vectors, matrices, arrays of fixed length, and structures of
//!   them, declared as `uniform` globals, as `uniform` parameters of the
//!   entry point, or as fields of a `ConstantBuffer`, a `cbuffer`, or a
//!   `ParameterBlock`, which a call sets from the Rust types that [`Value`]
//!   lists.
//! - System values such as `SV_DispatchThreadID`, which the GPU provides.
//!
//! The vertex and fragment entry points of a render pipeline, functions
//! marked `[shader("vertex")]` and `[shader("fragment")]`, support these
//! parameters:
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
//!   `SV_VulkanInstanceID`, which the GPU provides. With the feature `rhi`
//!   on Direct3D 12, a vertex entry point that reads `SV_VulkanVertexID` or
//!   `SV_VulkanInstanceID` fails with [`ErrorKind::Compile`], as
//!   [Choosing a backend](#choosing-a-backend) describes.
//!
//! The load of a module fails with [`ErrorKind::UnsupportedParameter`] when
//! a kernel has any other parameter, such as a storage texture, a
//! `Sampler2D`, a `SamplerComparisonState`, a `ByteAddressBuffer`, a
//! `double`, or an array of structures. The creation of a render pipeline
//! fails with the same kind when one of its entry points has such a
//! parameter or `uniform` parameters of its own. With the feature `wgpu`,
//! the load fails with [`ErrorKind::MissingFeature`] when a kernel has
//! `uniform` parameters of its entry point and the device cannot receive
//! them, and the load and the creation fail with
//! [`ErrorKind::UnsupportedShader`] when wgpu rejects the compiled form of an
//! entry point, or the two entry points of a render pipeline together, such
//! as when their varyings do not match. The feature `rhi` refuses a few more
//! parameters, which [Choosing a backend](#choosing-a-backend) lists.
//!
//! # Limits
//!
//! The crate has these limits:
//!
//! - It runs the compute, vertex, and fragment entry points that a module
//!   declares, and generates no shader of its own.
//! - With the feature `wgpu`, it compiles every entry point to SPIR-V and
//!   loads it through wgpu's validated path, so it rejects what naga, the
//!   shader translator inside wgpu, rejects, with
//!   [`ErrorKind::UnsupportedShader`]. naga rejects, among other constructs,
//!   an array of buffers or of textures, a call of `frexp`, and
//!   `SV_VertexID` and `SV_InstanceID`, in place of which a vertex entry
//!   point reads `SV_VulkanVertexID` and `SV_VulkanInstanceID`.
//! - One draw is one render pass with one draw command and one colour
//!   target, a [`Texture`] of a colour format of [`TextureFormat`], and at
//!   most one depth target of the format [`TextureFormat::Depth32Float`]. The
//!   draw clears the targets first or keeps their contents.
//! - Each vertex attribute takes its own vertex buffer, at offset 0 with
//!   the size of one element as its stride, so the device's limit on vertex
//!   buffers bounds the attributes of a vertex entry point: with the
//!   feature `wgpu`, those that it uses, 8 by default, and with the feature
//!   `rhi`, all that it declares.
//! - Every pipeline draws a list of triangles with one sample per pixel. The
//!   crate offers no multisampling, no storage textures, and no texture
//!   arrays.
//!
//! # Conventions of a draw
//!
//! The draws follow the conventions of wgpu on every backend. The y axis of
//! a position in clip space points up, so (-1, 1) is the upper-left corner
//! of the target, and a depth from 0.0, the nearest, to 1.0, the farthest,
//! is drawn, while a depth outside that range is clipped. The one exception
//! is the feature `rhi` on Vulkan, which draws the part of a triangle
//! outside that range with its depth clamped to it. A triangle whose
//! vertices appear counter-clockwise on the target faces the viewer. The
//! texture coordinates (0, 0) address the upper-left pixel of a texture. A
//! matrix is set as its rows, so a transform written in Rust as rows works
//! with `mul(transform, position)` in Slang. A math library that stores a
//! matrix as its columns needs `mul(position, transform)` or a transpose.
//!
#![cfg_attr(
    all(feature = "wgpu", not(feature = "rhi")),
    doc = r"
# Drawing into a window

The crate draws into a texture and owns no window, surface, or frame
loop. A program that draws into a window creates the window, the wgpu
surface, and the wgpu device itself, and hands the device and its queue
to this crate with [`wgpu::from_device_and_queue`]. In each frame, it
wraps the view of the surface texture as a [`Texture`] with
[`wgpu::texture_from_view`], draws into it, and presents the frame with
wgpu. The example `windowed-renderer` does this with winit.
"
)]
//!
//! # Features
//!
//! The crate needs exactly one backend, so a build with neither `wgpu` nor
//! `rhi`, or with both, stops with a compile error that says so.
//!
//! - `wgpu`, on by default: runs every kernel and every draw on wgpu 30, and
//!   provides the module `wgpu`, the boundary to wgpu code of your own.
//! - `rhi`, off by default: runs every kernel and every draw on slang-rhi,
//!   the render hardware interface of Slang, with Vulkan on Linux and
//!   Direct3D 12 on Windows on x86-64. It excludes `wgpu`, so a program
//!   turns the default features off to enable it, and it implies
//!   `vendored`. It builds Slang and slang-rhi from source, and the build
//!   stops on every other target. [Choosing a backend](#choosing-a-backend)
//!   says what else follows from it.
//! - `vendored`, off by default: enables the feature `vendored` of
//!   slang-compiler, so that a program that depends on this crate alone
//!   links a Slang built from source statically, in place of the Slang shared
//!   library of a Vulkan SDK. It adds and removes no item of the API. The
//!   crate documentation of slang-compiler says what that build needs.
//!
//! # Choosing a backend
//!
//! A backend is the graphics library through which the crate runs kernels
//! and draws. Every build has exactly one, which a Cargo feature chooses:
//!
//! - wgpu, through the feature `wgpu`, which is on by default. A program
//!   that keeps the default features uses it.
//! - slang-rhi, through the feature `rhi`. A program turns the default
//!   features off and enables `rhi` in its dependency:
//!
//! ```toml
//! [dependencies]
//! slango = { version = "0.1.0-alpha.2", default-features = false, features = ["rhi"] }
//! ```
//!
//! With `rhi`, the build clones the Slang source tree and builds Slang and
//! slang-rhi from it, which takes minutes on the first build in a Cargo
//! target directory and needs the tools that the crate documentation of
//! slang-rhi-sys lists. It builds on Linux with the GNU C library, where it
//! runs on Vulkan, and on Windows on x86-64 with the Microsoft toolchain,
//! where it runs on Direct3D 12. On every other target, macOS included, the
//! build stops with an error.
//!
//! A program on Windows that loads a module needs `dxcompiler.dll`, the
//! DirectX Shader Compiler, on its `PATH` or beside its executable at run
//! time. The Windows SDK ships it.
//!
//! Cargo enables the features of one crate for the whole build, so every
//! crate of a program that depends on this crate shares one backend. When
//! two crates of one build ask for different backends, or one keeps the
//! default features while another enables `rhi`, the build stops with a
//! compile error.
//!
//! A library that works on either backend turns the default features off,
//! enables neither, and leaves the choice to the program. The
//! crate slang-slint always uses the backend `wgpu`, so a program on `rhi`
//! cannot use it. The macro of the crate slango-macros works on both
//! backends, and the code that it generates follows the backend of the
//! program's copy of this crate.
//!
//! Every type, function, and check of the crate exists on both backends,
//! apart from the module `wgpu`, which exists only with the feature `wgpu`.
//! A program written for one backend compiles on the other, and the same
//! program gives the same results, and the same error kinds and texts for
//! bad arguments, on both, apart from these differences:
//!
//! - The module `wgpu` and its functions, the section on drawing into a
//!   window, and the error kinds [`ErrorKind::MissingFeature`] and
//!   [`ErrorKind::UnsupportedShader`] concern the backend `wgpu` only. The
//!   backend `rhi` draws into textures only and offers no window.
//! - The backend `rhi` hands the code that Slang generates to the driver
//!   without the validation of wgpu, and without the checks of memory
//!   accesses that wgpu adds to a shader. It therefore runs some kernels that
//!   wgpu rejects, such as kernels that use 64-bit integers, half-precision
//!   floats, or wave operations, where the device supports them, and a shader
//!   that reads or writes past the end of a buffer reaches whatever the
//!   driver allows.
//! - No shader that reads the index of a vertex or of an instance runs on
//!   both backends on Direct3D 12. wgpu rejects `SV_VertexID` and
//!   `SV_InstanceID`, while the backend `rhi` on Direct3D 12 fails with
//!   [`ErrorKind::Compile`] on `SV_VulkanVertexID` and
//!   `SV_VulkanInstanceID`. A shader for both backends takes such values from
//!   a vertex buffer instead. The backend `rhi` on Vulkan draws a vertex
//!   entry point that reads `SV_InstanceID`.
//! - The backend `rhi` needs a semantic, such as `POSITION` or `TEXCOORD0`,
//!   on every vertex attribute, because Direct3D 12 matches attributes by
//!   their semantics. Without one, the module still loads, and
//!   [`Module::render_pipeline`] fails for that vertex entry point with
//!   [`ErrorKind::UnsupportedParameter`].
//! - The backend `wgpu` counts the vertex attributes that a vertex entry
//!   point uses against its limit of vertex buffers, 8 by default. The
//!   backend `rhi` counts every attribute that the entry point declares, used
//!   or not, against the vertex streams of the device, at most 16, and
//!   [`Module::render_pipeline`] fails with [`ErrorKind::InvalidArgument`]
//!   when they do not fit. On both backends, an attribute that the shader
//!   declares and does not use needs no buffer.
//! - The backend `rhi` refuses two shapes of parameter that the backend
//!   `wgpu` runs: a resource inside a structure that is not the element of a
//!   constant buffer or a parameter block, and a constant buffer whose
//!   element is not a structure. A kernel that uses one fails to load, and a
//!   render pipeline that uses one fails to be created, with
//!   [`ErrorKind::UnsupportedParameter`]. Such a parameter that the shader
//!   does not use works on both backends.
//! - A kernel whose thread group is larger than the device allows fails to
//!   load with [`ErrorKind::Backend`], except on the backend `rhi` on
//!   Direct3D 12, where the DXIL compiler refuses it first with
//!   [`ErrorKind::Compile`].
//! - On the backend `rhi` on Vulkan, the part of a triangle outside the depth
//!   range 0 to 1 is drawn with its depth clamped, while the backend `wgpu`
//!   and the backend `rhi` on Direct3D 12 clip it.
//! - On the backend `rhi` on Direct3D 12, a kernel or a render pipeline that
//!   uses both a `ConstantBuffer` and a global `uniform` fails, through a
//!   defect of slang-rhi: a kernel fails to load with [`ErrorKind::Backend`],
//!   and [`Module::render_pipeline`] fails with [`ErrorKind::Compile`]. A
//!   `ParameterBlock` in place of the constant buffer, or no global `uniform`
//!   beside it, avoids the failure.
//! - A message about a texture that cannot be sampled, drawn into, or read
//!   names the usages of the backend. Only a texture that the function
//!   `wgpu::texture_from_view` makes can lack one, so only the backend
//!   `wgpu` gives such a message.
//!
//! On the backend `rhi`, a load compiles the module for the graphics API of
//! the device, SPIR-V on Vulkan and DXIL on Direct3D 12, and the module keeps
//! the Slang session of its load until the module and every kernel and
//! render pipeline from it are dropped.
//!
//! # Platform support
//!
//! The crate supports the platforms of `slang-compiler`, which are Linux on
//! x86-64, Windows on x86-64, and macOS on Apple silicon and on x86-64. With
//! the feature `wgpu`, it searches the adapters of whichever of Vulkan,
//! Direct3D 12, and Metal wgpu offers on the platform. Its tests create
//! devices, run kernels, sample textures, and draw in continuous integration
//! on three adapters: lavapipe on Vulkan on Linux, the Windows Advanced
//! Rasterization Platform (WARP) on Direct3D 12 on Windows, and the Apple
//! paravirtual device on Metal on macOS. On macOS, Metal offers no software
//! adapter, so [`AdapterSelection::Software`] finds none there.
//!
//! With the feature `rhi`, the crate builds only on Linux with the GNU C
//! library, where it runs on Vulkan, and on Windows on x86-64 with the
//! Microsoft toolchain, where it runs on Direct3D 12. Its tests run in
//! continuous integration on lavapipe and on WARP.
//!
//! # Finding the Slang library
//!
//! `slang-compiler` links the Slang library through `slang-compiler-sys`, as
//! the [crate documentation of `slang-compiler`](slang_compiler) describes.
//! By default, that is the Slang shared library of the Vulkan SDK of the build,
//! which the dynamic loader has to find when the program starts. With the
//! feature `vendored`, it is a Slang built from source and linked
//! statically, and the build needs no Vulkan SDK. The feature `rhi` implies
//! `vendored`, so a build with `rhi` always links a Slang built from source.
//! A program that also calls the macro of `slango-macros` enables `vendored`
//! there as well when it builds without a Vulkan SDK, because the macro
//! links its own copy of Slang, as the crate documentation of
//! `slango-macros` explains.
//!
//! # Changes from 0.1.0-alpha.1
//!
//! The published release `0.1.0-alpha.1` of this crate had the backend
//! `wgpu` only. A program written for it needs these changes:
//!
//! - [`Device`], [`Module`], [`Kernel`], [`Buffer`], [`Texture`], [`Sampler`],
//!   [`RenderPipeline`], [`Call`], and [`Draw`] are no longer [`Send`] or
//!   [`Sync`]. A program keeps each device, and everything created from it,
//!   on the thread that created the device, and sends results to other
//!   threads as plain data. [`Error`], [`ErrorKind`], the options structs,
//!   and the enumerations stay [`Send`] and [`Sync`].
//! - A call or a draw that binds one buffer to a parameter that the shader
//!   only reads and to one that it writes, or that gives a vertex or index
//!   buffer that is also bound to a parameter that the shader writes, fails
//!   with [`ErrorKind::InvalidArgument`] before any work reaches the GPU,
//!   where it failed with [`ErrorKind::Backend`] before. Separate buffers
//!   avoid it.
//! - [`ErrorKind`] has the new kind [`ErrorKind::Unsupported`], for an
//!   operation that the backend of the build does not offer. A `match` with
//!   a wildcard arm, which the non-exhaustive enumeration already requires,
//!   needs no change.
//! - The text of the error of a buffer read whose bytes do not fit its
//!   elements begins with "the backend read" where it began with "wgpu
//!   mapped". Code that matches on the kind needs no change.
//! - The module `wgpu` exists only with the feature `wgpu`. Its functions
//!   keep their signatures, and its accessors `device`, `queue`, and
//!   `texture` still return an `Option`, which is always `Some` in a build
//!   with that feature.
//! - A program that turns the default features off names exactly one of
//!   `wgpu` and `rhi`, and a build that enables neither or both stops with a
//!   compile error.
//! - The code that the macro of `slango-macros` generates calls this crate
//!   in a new way, so a program rebuilds it with the `slango-macros` of the
//!   same version as this crate. The generated `Shader`, kernel structs, and
//!   arguments structs are no longer [`Send`] or [`Sync`] either.
//! - The `View` of the crate slang-slint is no longer [`Send`] or [`Sync`],
//!   and stays on the thread of the user interface.
//!
//! A program that uses the crate on one thread with its default features
//! needs no change of its code. A program that moves to the backend `rhi`
//! also meets the differences of [Choosing a backend](#choosing-a-backend).

#[cfg(not(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
)))]
compile_error!(
    "slango needs exactly one backend: keep its default Cargo feature `wgpu`, or turn the default features off and enable its Cargo feature `rhi`, but not both"
);

/// Expands to the items that it is given in a build with the feature `wgpu`,
/// and to nothing in a build with the feature `rhi`.
///
/// The code that `slango-macros` generates puts every item that only the
/// backend wgpu needs inside this macro, and names it through the slango of
/// the crate that calls the macro of `slango-macros`, so that the items
/// follow the backend of that slango. It is not part of the API of this
/// crate.
#[cfg(all(feature = "wgpu", not(feature = "rhi")))]
#[doc(hidden)]
#[macro_export]
macro_rules! wgpu_only {
    ($($item:item)*) => {
        $($item)*
    };
}

/// Expands to the items that it is given in a build with the feature `wgpu`,
/// and to nothing in a build with the feature `rhi`.
///
/// The code that `slango-macros` generates puts every item that only the
/// backend wgpu needs inside this macro, and names it through the slango of
/// the crate that calls the macro of `slango-macros`, so that the items
/// follow the backend of that slango. It is not part of the API of this
/// crate.
#[cfg(all(feature = "rhi", not(feature = "wgpu")))]
#[doc(hidden)]
#[macro_export]
macro_rules! wgpu_only {
    ($($item:item)*) => {};
}

/// Expands to the items that it is given in a build with the feature `rhi`,
/// and to nothing in a build with the feature `wgpu`.
///
/// The code that `slango-macros` generates puts every item that only the
/// backend rhi needs inside this macro, and names it through the slango of
/// the crate that calls the macro of `slango-macros`, so that the items
/// follow the backend of that slango. It is not part of the API of this
/// crate.
#[cfg(all(feature = "rhi", not(feature = "wgpu")))]
#[doc(hidden)]
#[macro_export]
macro_rules! rhi_only {
    ($($item:item)*) => {
        $($item)*
    };
}

/// Expands to the items that it is given in a build with the feature `rhi`,
/// and to nothing in a build with the feature `wgpu`.
///
/// The code that `slango-macros` generates puts every item that only the
/// backend rhi needs inside this macro, and names it through the slango of
/// the crate that calls the macro of `slango-macros`, so that the items
/// follow the backend of that slango. It is not part of the API of this
/// crate.
#[cfg(all(feature = "wgpu", not(feature = "rhi")))]
#[doc(hidden)]
#[macro_export]
macro_rules! rhi_only {
    ($($item:item)*) => {};
}

#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
#[doc(hidden)]
pub mod __private;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod buffer;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod call;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod device;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod draw;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
#[cfg_attr(
    all(feature = "rhi", not(feature = "wgpu"), not(test)),
    expect(
        dead_code,
        reason = "only the backend wgpu decodes the encoded snapshot that generated code embeds, while the backend rhi compiles the embedded sources"
    )
)]
mod encoding;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod error;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod kernel;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod layout;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod marshal;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod module;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod render;
#[cfg(all(feature = "rhi", not(feature = "wgpu")))]
mod rhi_backend;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod sampler;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
mod texture;
#[cfg(all(feature = "wgpu", not(feature = "rhi")))]
pub mod wgpu;
#[cfg(all(feature = "wgpu", not(feature = "rhi")))]
mod wgpu_backend;

#[cfg(all(feature = "rhi", not(feature = "wgpu")))]
use rhi_backend as backend;
#[cfg(all(feature = "wgpu", not(feature = "rhi")))]
use wgpu_backend as backend;

#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use buffer::Buffer;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use call::Call;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use device::{AdapterSelection, Device, DeviceOptions};
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use draw::Draw;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use error::{Error, ErrorKind, Result};
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use kernel::Kernel;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use marshal::{Element, Value, VertexElement};
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use module::Module;
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use render::{Blend, CullMode, RenderPipeline, RenderPipelineOptions};
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use sampler::{AddressMode, FilterMode, Sampler, SamplerOptions};
#[cfg(any(
    all(feature = "wgpu", not(feature = "rhi")),
    all(feature = "rhi", not(feature = "wgpu"))
))]
pub use texture::{Texture, TextureFormat};

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
pub struct ReadmeDoctests;
