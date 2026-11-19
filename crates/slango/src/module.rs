use std::fmt;
use std::marker::PhantomData;
use std::path::Path;
use std::sync::Arc;

use crate::Device;
use crate::backend;
use crate::error::{Error, ErrorKind, Result};
use crate::kernel::Kernel;
use crate::layout::{CompiledRender, KernelLayout, Source};
use crate::render::{RenderPipeline, RenderPipelineOptions};

const READ: &str = "read a Slang file";

/// One loaded Slang module, with a [`Kernel`] for each of its compute entry
/// points, and its vertex and fragment entry points.
///
/// Load a module with [`Device::load_module`] or
/// [`Device::load_module_from_source`], then get its kernels by name with
/// [`Module::kernel`], and create a [`RenderPipeline`] from a vertex and a
/// fragment entry point with [`Module::render_pipeline`].
///
/// Cloning a module is cheap and returns another handle to the same module.
/// A module keeps its [`Device`] alive. With the feature `wgpu`, it holds no
/// object of the Slang compiler, and with the feature `rhi`, it keeps the
/// Slang session of its load alive.
///
/// A module is neither [`Send`] nor [`Sync`]: it stays on the thread that
/// created its [`Device`], because the crate keeps all the GPU objects of a
/// device on one thread whatever graphics library runs them. A program sends
/// results to other threads as plain data, such as the vector that
/// [`Buffer::read`](crate::Buffer::read) returns. This program does not
/// compile, because a module is not `Send`:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slango::Module>();
/// ```
///
/// This program does not compile either, because a module is not `Sync`:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slango::Module>();
/// ```
///
/// # Examples
///
/// See the example of [`Module::kernel`].
#[derive(Clone)]
pub struct Module {
    device: Device,
    kernels: Arc<[Kernel]>,
    render: Arc<[CompiledRender]>,
    inner: backend::Module,
    thread_bound: PhantomData<*const ()>,
}

/// What the backend created on a device for one load of a module: its own
/// objects of the module, each kernel with its layout snapshot, and the
/// records of the vertex and fragment entry points.
pub(crate) struct Loaded {
    pub(crate) module: backend::Module,
    pub(crate) kernels: Vec<(KernelLayout, backend::Kernel)>,
    pub(crate) render: Vec<CompiledRender>,
}

impl Module {
    /// Returns the kernel with the given name, the name of its function in
    /// the Slang source.
    ///
    /// # Examples
    ///
    /// This example loads a module with one kernel, looks the kernel up, and
    /// prints the size of its thread groups. It needs a GPU driver or a
    /// software driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module_from_source(
    ///     "fill",
    ///     r#"
    ///         RWStructuredBuffer<float> data;
    ///
    ///         [shader("compute")]
    ///         [numthreads(64, 1, 1)]
    ///         void fill(uint3 id : SV_DispatchThreadID) { data[id.x] = 1.0; }
    ///     "#,
    /// )?;
    /// let fill = module.kernel("fill")?;
    /// println!("{} runs in groups of {:?}", fill.name(), fill.thread_group_size());
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::InvalidArgument`] when no
    /// compute entry point of the module has the name.
    pub fn kernel(&self, name: &str) -> Result<&Kernel> {
        self.kernels
            .iter()
            .find(|kernel| kernel.name() == name)
            .ok_or_else(|| {
                Error::new(ErrorKind::InvalidArgument, "find a kernel").with_detail(format!(
                    "the module has no compute entry point named `{name}`"
                ))
            })
    }

    /// Creates a render pipeline from the vertex entry point named `vertex`
    /// and the fragment entry point named `fragment`, with the options.
    ///
    /// The pipeline binds every global parameter that either entry point
    /// uses, and takes one vertex buffer for each vertex attribute that the
    /// vertex entry point uses. The creation checks the entry points and the
    /// options first, then creates everything that a draw needs on the
    /// device, so every error of the pipeline surfaces here. Each call
    /// creates a new pipeline.
    ///
    /// # Examples
    ///
    /// This example loads a module with a vertex and a fragment entry point
    /// and creates a render pipeline from them that draws into a
    /// `Bgra8Unorm` target with a depth target and discards the triangles
    /// that face away. It needs a GPU driver or a software driver at run
    /// time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{CullMode, Device, DeviceOptions, RenderPipelineOptions, TextureFormat};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module_from_source(
    ///     "triangle",
    ///     r#"
    ///         struct VOut { float4 position : SV_Position; float3 colour : COLOR; };
    ///
    ///         [shader("vertex")]
    ///         VOut vsMain(float2 position : POSITION, float3 colour : COLOR)
    ///         {
    ///             VOut output;
    ///             output.position = float4(position, 0.0, 1.0);
    ///             output.colour = colour;
    ///             return output;
    ///         }
    ///
    ///         [shader("fragment")]
    ///         float4 fsMain(VOut input) : SV_Target { return float4(input.colour, 1.0); }
    ///     "#,
    /// )?;
    /// let mut options = RenderPipelineOptions::default();
    /// options.colour_format = TextureFormat::Bgra8Unorm;
    /// options.depth_format = Some(TextureFormat::Depth32Float);
    /// options.cull_mode = CullMode::Back;
    /// let pipeline = module.render_pipeline("vsMain", "fsMain", &options)?;
    /// # drop(pipeline);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of these kinds:
    ///
    /// - [`ErrorKind::InvalidArgument`] when the module has no vertex entry
    ///   point named `vertex` or no fragment entry point named `fragment`,
    ///   when the colour format is a depth format or the depth format is a
    ///   colour format, or when `instance_attributes` names no vertex
    ///   attribute of the vertex entry point.
    /// - [`ErrorKind::UnsupportedParameter`] when either entry point has a
    ///   parameter of a kind that this crate does not support, or
    ///   `uniform` parameters of its own, which a render pipeline does not
    ///   accept.
    /// - [`ErrorKind::Compile`] when Slang failed on either entry point while
    ///   the module was loaded. The source error holds the messages that
    ///   Slang printed.
    /// - [`ErrorKind::UnsupportedShader`] when the backend rejects the
    ///   compiled form of either entry point, or the two together, such as
    ///   when the outputs of the vertex entry point do not match the inputs
    ///   of the fragment entry point. The source error carries the backend's
    ///   text.
    /// - [`ErrorKind::Backend`] when the backend reports another error while
    ///   it creates the pipeline.
    ///
    /// With the feature `rhi`, it also returns an [`Error`] of these kinds:
    ///
    /// - [`ErrorKind::UnsupportedParameter`] when a vertex attribute has no
    ///   semantic, which Direct3D 12 matches attributes by, or when a used
    ///   resource is a parameter of an entry point or sits inside a
    ///   structure that is not the element of a constant buffer or a
    ///   parameter block.
    /// - [`ErrorKind::InvalidArgument`] when the vertex attributes need more
    ///   vertex streams or input elements than the device allows.
    /// - [`ErrorKind::Compile`] when slang-rhi cannot compile the two entry
    ///   points together or create the pipeline from them.
    pub fn render_pipeline(
        &self,
        vertex: &str,
        fragment: &str,
        options: &RenderPipelineOptions,
    ) -> Result<RenderPipeline> {
        RenderPipeline::create(
            &self.device,
            &self.inner,
            &self.render,
            vertex,
            fragment,
            options,
        )
    }
}

impl fmt::Debug for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Module")
            .field("kernels", &self.kernels)
            .finish_non_exhaustive()
    }
}

impl Device {
    /// Loads the Slang module in a file and prepares each of its compute
    /// entry points as a [`Kernel`] of this device.
    ///
    /// The load compiles the module on the calling thread, to SPIR-V with
    /// the feature `wgpu`. With the feature `rhi`, it compiles the module for
    /// the device, SPIR-V on Vulkan and DXIL on Direct3D 12, and on Direct3D
    /// 12 it also compiles the vertex entry points to SPIR-V to learn which
    /// vertex attributes they read, because the metadata of DXIL reports
    /// every declared attribute as read.
    ///
    /// Slang resolves an `import` relative to the file, and the load also
    /// gives Slang the directory of the file as a search path. It then creates on
    /// the device everything that a dispatch of each kernel needs, so every
    /// error of the module surfaces here. Each load starts the Slang compiler
    /// afresh, which takes about a tenth of a second, so load a module once
    /// and keep it.
    ///
    /// # Examples
    ///
    /// This example loads a module from a file and looks up one of its
    /// kernels. It needs a GPU driver or a software driver at run time, so it
    /// is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module("/tmp/shaders/scale.slang")?;
    /// let scale = module.kernel("scale")?;
    /// # drop(scale);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of these kinds:
    ///
    /// - [`ErrorKind::InvalidArgument`] when the file cannot be read as text,
    ///   or its path is not valid UTF-8, which Slang needs.
    /// - [`ErrorKind::Compile`] when Slang cannot compile the module. The
    ///   source error is the error of `slang-compiler` with Slang's messages.
    /// - [`ErrorKind::UnsupportedParameter`] when a kernel has a parameter of
    ///   a kind that this crate does not support, such as a storage
    ///   texture.
    /// - [`ErrorKind::MissingFeature`] when a kernel has `uniform` parameters
    ///   of its entry point and the device lacks the immediates feature or
    ///   enough immediate data for them.
    /// - [`ErrorKind::UnsupportedShader`] when the backend rejects the
    ///   compiled form of a kernel. The source error carries the backend's
    ///   text, which names the construct to avoid.
    /// - [`ErrorKind::Backend`] when the backend reports another error while
    ///   it creates a kernel, such as for a thread group larger than the
    ///   device allows.
    ///
    /// With the feature `rhi`, the load compiles the module for the device,
    /// SPIR-V on Vulkan and DXIL on Direct3D 12, and also returns these kinds:
    ///
    /// - [`ErrorKind::Compile`] when Slang or slang-rhi cannot generate the
    ///   code of a kernel for the device.
    /// - [`ErrorKind::Backend`] when slang-rhi reports another error while it
    ///   creates a kernel.
    pub fn load_module(&self, path: impl AsRef<Path>) -> Result<Module> {
        let file = SourceFile::read(path.as_ref())?;
        self.load_source(&file.source())
    }

    /// Loads a Slang module from source text and prepares each of its compute
    /// entry points as a [`Kernel`] of this device.
    ///
    /// It works as [`Device::load_module`] does, with `name` as the name of
    /// the module, and an `import` in the source searches only the places
    /// that Slang searches by default.
    ///
    /// # Examples
    ///
    /// See the example of [`Module::kernel`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of the kinds that [`Device::load_module`]
    /// returns, except that no file is read.
    pub fn load_module_from_source(&self, name: &str, source: &str) -> Result<Module> {
        let path = format!("{name}.slang");
        self.load_source(&Source {
            name,
            path: &path,
            text: source,
            search_path: None,
        })
    }

    /// Compiles the module of `source` for the backend of this device and
    /// creates it on the device.
    fn load_source(&self, source: &Source<'_>) -> Result<Module> {
        let loaded = backend::Module::load(self.backend(), source)?;
        Ok(self.module(loaded))
    }

    /// Creates a module on this device from what generated code embeds for
    /// the backend of the build.
    pub(crate) fn load_embedded(&self, embedded: &backend::Embedded<'_>) -> Result<Module> {
        let loaded = backend::Module::load_embedded(self.backend(), embedded)?;
        Ok(self.module(loaded))
    }

    /// Wraps what the backend created for one load as a module of this
    /// device.
    fn module(&self, loaded: Loaded) -> Module {
        let Loaded {
            module,
            kernels,
            render,
        } = loaded;
        let kernels: Vec<Kernel> = kernels
            .into_iter()
            .map(|(layout, inner)| Kernel::from_backend(self, layout, inner))
            .collect();
        Module {
            device: self.clone(),
            kernels: kernels.into(),
            render: render.into(),
            inner: module,
            thread_bound: PhantomData,
        }
    }
}

/// The Slang source of a module read from a file, with the name and the
/// search path that a load of the file gives Slang.
pub(crate) struct SourceFile<'p> {
    name: String,
    path: &'p str,
    text: String,
    directory: &'p Path,
}

impl<'p> SourceFile<'p> {
    /// Reads the Slang source in a file, with the file stem as the module
    /// name and the directory of the file, or `.` when the path has none, as
    /// the search path.
    pub(crate) fn read(path: &'p Path) -> Result<Self> {
        let Some(path_text) = path.to_str() else {
            return Err(
                Error::new(ErrorKind::InvalidArgument, READ).with_detail(format!(
                    "the path `{}` is not valid UTF-8, which Slang needs",
                    path.display()
                )),
            );
        };
        let text = std::fs::read_to_string(path).map_err(|error| {
            Error::new(ErrorKind::InvalidArgument, READ)
                .with_detail(format!("`{}`", path.display()))
                .with_source(error)
        })?;
        let name = path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(path_text)
            .to_owned();
        let directory = path
            .parent()
            .filter(|directory| !directory.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        Ok(Self {
            name,
            path: path_text,
            text,
            directory,
        })
    }

    /// Returns the source of the file for a compilation.
    pub(crate) fn source(&self) -> Source<'_> {
        Source {
            name: &self.name,
            path: self.path,
            text: &self.text,
            search_path: Some(self.directory),
        }
    }
}
