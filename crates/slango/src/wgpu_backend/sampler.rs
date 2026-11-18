use super::device::{Device, scoped};
use crate::error::{Error, ErrorKind, Result};
use crate::sampler::{AddressMode, FilterMode, SamplerOptions};

/// A wgpu sampler, which a [`crate::Sampler`] holds.
#[derive(Clone)]
pub(crate) struct Sampler {
    inner: ::wgpu::Sampler,
}

impl Sampler {
    pub(crate) fn new(device: &Device, options: &SamplerOptions) -> Result<Self> {
        let wgpu_device = device.wgpu_device();
        let (inner, error) = scoped(wgpu_device, || {
            wgpu_device.create_sampler(&descriptor(options))
        });
        if let Some(error) = error {
            return Err(Error::new(ErrorKind::Backend, "create a sampler").with_source(error));
        }
        Ok(Self { inner })
    }

    pub(super) fn wgpu_sampler(&self) -> &::wgpu::Sampler {
        &self.inner
    }
}

fn filter_to_wgpu(filter: FilterMode) -> ::wgpu::FilterMode {
    match filter {
        FilterMode::Nearest => ::wgpu::FilterMode::Nearest,
        FilterMode::Linear => ::wgpu::FilterMode::Linear,
    }
}

fn address_mode_to_wgpu(mode: AddressMode) -> ::wgpu::AddressMode {
    match mode {
        AddressMode::ClampToEdge => ::wgpu::AddressMode::ClampToEdge,
        AddressMode::Repeat => ::wgpu::AddressMode::Repeat,
        AddressMode::MirrorRepeat => ::wgpu::AddressMode::MirrorRepeat,
    }
}

fn descriptor(options: &SamplerOptions) -> ::wgpu::SamplerDescriptor<'static> {
    let address_mode = address_mode_to_wgpu(options.address_mode);
    let filter = filter_to_wgpu(options.filter);
    ::wgpu::SamplerDescriptor {
        label: Some("slango sampler"),
        address_mode_u: address_mode,
        address_mode_v: address_mode,
        address_mode_w: address_mode,
        mag_filter: filter,
        min_filter: filter,
        ..::wgpu::SamplerDescriptor::default()
    }
}

#[cfg(test)]
mod tests;
