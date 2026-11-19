use std::rc::Rc;

use slang_rhi::SamplerDesc;

use super::device::Device;
use super::error::{Call, from_rhi};
use crate::error::Result;
use crate::sampler::{AddressMode, FilterMode, SamplerOptions};

/// The sampler of slang-rhi that a [`crate::Sampler`] holds.
#[derive(Clone)]
pub(crate) struct Sampler {
    inner: Rc<slang_rhi::Sampler>,
}

impl Sampler {
    /// Creates the sampler of `options`, with the mip filter `Point` and
    /// every other setting of the sampler written explicitly, because the
    /// defaults of slang-rhi differ from those of this crate.
    pub(crate) fn new(device: &Device, options: &SamplerOptions) -> Result<Self> {
        let filter = filter_to_rhi(options.filter);
        let address_mode = address_mode_to_rhi(options.address_mode);
        let desc = SamplerDesc::new()
            .with_min_filter(filter)
            .with_mag_filter(filter)
            .with_mip_filter(slang_rhi::FilterMode::Point)
            .with_address_modes([address_mode; 3]);
        let inner = device
            .rhi_device()
            .create_sampler(&desc)
            .map_err(|error| from_rhi("create a sampler", Call::Other, error))?;
        Ok(Self {
            inner: Rc::new(inner),
        })
    }

    pub(super) fn rhi_sampler(&self) -> &slang_rhi::Sampler {
        &self.inner
    }
}

fn filter_to_rhi(filter: FilterMode) -> slang_rhi::FilterMode {
    match filter {
        FilterMode::Nearest => slang_rhi::FilterMode::Point,
        FilterMode::Linear => slang_rhi::FilterMode::Linear,
    }
}

fn address_mode_to_rhi(mode: AddressMode) -> slang_rhi::AddressMode {
    match mode {
        AddressMode::ClampToEdge => slang_rhi::AddressMode::ClampToEdge,
        AddressMode::Repeat => slang_rhi::AddressMode::Wrap,
        AddressMode::MirrorRepeat => slang_rhi::AddressMode::MirrorRepeat,
    }
}
