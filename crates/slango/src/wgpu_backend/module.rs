use super::device::Device;
use super::kernel::Kernel;
use crate::encoding;
use crate::error::Result;
use crate::layout::{self, Compiled, CompiledKernel, Source};
use crate::module::Loaded;

/// A module that generated code embeds for the backend wgpu: its layout
/// snapshot with the SPIR-V of each entry point, encoded when the code was
/// generated.
#[expect(
    clippy::exhaustive_structs,
    reason = "the hidden module has no stability promise, and generated code builds it with a literal"
)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Embedded<'a> {
    /// The encoded layout snapshot of the module.
    pub encoded: &'a [u8],
}

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

    /// Decodes the encoded snapshot of an embedded module and creates every
    /// kernel of it on the device, with no call into Slang.
    pub(crate) fn load_embedded(device: &Device, embedded: &Embedded<'_>) -> Result<Loaded> {
        Self::load_compiled(device, encoding::decode(embedded.encoded)?)
    }

    /// Creates every kernel of the snapshot of a compilation on the device,
    /// with no call into Slang.
    fn load_compiled(device: &Device, compiled: Compiled) -> Result<Loaded> {
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
