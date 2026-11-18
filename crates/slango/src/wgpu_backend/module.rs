use super::device::Device;
use super::kernel::Kernel;
use crate::error::Result;
use crate::layout::{self, Compiled, CompiledKernel, Source};
use crate::module::Loaded;

/// The objects that the backend wgpu keeps for a [`crate::Module`], which
/// are none, because its kernels hold everything that they run with.
#[derive(Clone)]
pub(crate) struct Module;

impl Module {
    /// Compiles the module of `source` to SPIR-V and creates every kernel of
    /// it on the device.
    pub(crate) fn load(device: &Device, source: &Source<'_>) -> Result<Loaded> {
        Self::load_compiled(device, layout::compile(source)?.compiled)
    }

    /// Creates every kernel of the snapshot of a compilation on the device,
    /// with no call into Slang.
    pub(crate) fn load_compiled(device: &Device, compiled: Compiled) -> Result<Loaded> {
        let kernels = compiled
            .kernels
            .into_iter()
            .map(|CompiledKernel { layout, spirv }| {
                let kernel = Kernel::create(device, &layout, spirv)?;
                Ok((layout, kernel))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(Loaded {
            module: Self,
            kernels,
            render: compiled.render,
        })
    }
}
