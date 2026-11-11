use std::fmt;
use std::path::Path;
use std::sync::Arc;

use crate::Device;
use crate::error::{Error, ErrorKind, Result};
use crate::kernel::Kernel;
use crate::layout::{self, Source};

const READ: &str = "read a Slang file";

/// One loaded Slang module, with a [`Kernel`] for each of its compute entry
/// points.
///
/// Load a module with [`Device::load_module`] or
/// [`Device::load_module_from_source`], then get its kernels by name with
/// [`Module::kernel`].
///
/// Cloning a module is cheap and returns another handle to the same module.
/// A module keeps its [`Device`] alive, holds no object of the Slang
/// compiler, and is [`Send`] and [`Sync`], so threads can share it.
///
/// # Examples
///
/// See the example of [`Module::kernel`].
#[derive(Clone)]
pub struct Module {
    #[expect(
        dead_code,
        reason = "SLO-3 of docs/design/slango.md has a module hold its device, and no method of M3 reads it, because each kernel holds the device too"
    )]
    device: Device,
    kernels: Arc<[Kernel]>,
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
    /// The load compiles the module to SPIR-V on the calling thread, with the
    /// directory of the file as the place where an `import` finds sibling
    /// modules. It then creates on the device everything that a dispatch of
    /// each kernel needs, so every error of the module surfaces here. Each
    /// load starts the Slang compiler afresh, which takes about a tenth of a
    /// second, so load a module once and keep it.
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
    ///   a kind that this version does not support, such as a texture.
    /// - [`ErrorKind::MissingFeature`] when a kernel has `uniform` parameters
    ///   of its entry point and the device lacks the immediates feature or
    ///   enough immediate data for them.
    /// - [`ErrorKind::UnsupportedShader`] when the backend rejects the
    ///   compiled form of a kernel. The source error carries the backend's
    ///   text, which names the construct to avoid.
    /// - [`ErrorKind::Backend`] when wgpu reports another error while it
    ///   creates a kernel.
    pub fn load_module(&self, path: impl AsRef<Path>) -> Result<Module> {
        self.load_module_from_path(path.as_ref())
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
        self.load(&Source {
            name,
            path: &path,
            text: source,
            search_path: None,
        })
    }

    fn load_module_from_path(&self, path: &Path) -> Result<Module> {
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
            .unwrap_or(path_text);
        let directory = path
            .parent()
            .filter(|directory| !directory.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        self.load(&Source {
            name,
            path: path_text,
            text: &text,
            search_path: Some(directory),
        })
    }

    fn load(&self, source: &Source<'_>) -> Result<Module> {
        let kernels = layout::compile(source)?
            .into_iter()
            .map(|compiled| Kernel::create(self, compiled))
            .collect::<Result<Vec<Kernel>>>()?;
        Ok(Module {
            device: self.clone(),
            kernels: kernels.into(),
        })
    }
}
