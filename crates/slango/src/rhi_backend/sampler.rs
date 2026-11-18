use std::convert::Infallible;

use super::device::Device;
use crate::error::{Error, ErrorKind, Result};
use crate::sampler::SamplerOptions;

/// The sampler of a [`crate::Sampler`], which this backend never creates,
/// so no value of the type exists.
#[derive(Clone)]
pub(crate) struct Sampler {
    never: Infallible,
}

impl Sampler {
    pub(crate) fn new(_device: &Device, _options: &SamplerOptions) -> Result<Self> {
        Err(Error::new(ErrorKind::Unsupported, "create a sampler")
            .with_detail("slango with the feature `rhi` creates no samplers".to_owned()))
    }

    /// Returns any value, because no sampler exists to call it on.
    pub(super) fn unreachable<T>(&self) -> T {
        match self.never {}
    }
}
