use std::ptr::{self, NonNull};

use slang_rhi_sys::{IDevice, ISampler};

use crate::device::Device;
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::owned::Owned;

/// How a sampler picks the value between texels.
///
/// The enumeration is non-exhaustive, so a later version can add modes, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`SamplerDesc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FilterMode {
    /// The value of the nearest texel.
    Point,
    /// A linear blend of the nearest texels.
    Linear,
}

impl FilterMode {
    fn to_raw(self) -> slang_rhi_sys::TextureFilteringMode {
        match self {
            Self::Point => slang_rhi_sys::TextureFilteringMode::Point,
            Self::Linear => slang_rhi_sys::TextureFilteringMode::Linear,
        }
    }
}

/// How a sampler treats a coordinate outside the range from 0 to 1.
///
/// The enumeration is non-exhaustive, so a later version can add modes, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`SamplerDesc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AddressMode {
    /// The coordinate is clamped to the texels at the edge.
    ClampToEdge,
    /// The texture repeats.
    Wrap,
    /// The texture repeats, mirrored at every other repetition.
    MirrorRepeat,
}

impl AddressMode {
    fn to_raw(self) -> slang_rhi_sys::TextureAddressingMode {
        match self {
            Self::ClampToEdge => slang_rhi_sys::TextureAddressingMode::ClampToEdge,
            Self::Wrap => slang_rhi_sys::TextureAddressingMode::Wrap,
            Self::MirrorRepeat => slang_rhi_sys::TextureAddressingMode::MirrorRepeat,
        }
    }
}

/// The description of a sampler that [`Device::create_sampler`] creates.
///
/// Every filter is [`FilterMode::Point`] and every address mode is
/// [`AddressMode::ClampToEdge`] unless a method of the description changes
/// it. These defaults differ from those of slang-rhi, which are linear
/// filters and repeating textures.
///
/// # Examples
///
/// This example describes a sampler that blends texels and repeats the
/// texture along both axes of a two-dimensional texture:
///
/// ```
/// use slang_rhi::{AddressMode, FilterMode, SamplerDesc};
///
/// let desc = SamplerDesc::new()
///     .with_min_filter(FilterMode::Linear)
///     .with_mag_filter(FilterMode::Linear)
///     .with_mip_filter(FilterMode::Point)
///     .with_address_modes([AddressMode::Wrap, AddressMode::Wrap, AddressMode::ClampToEdge]);
/// assert_ne!(desc, SamplerDesc::default());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SamplerDesc {
    min_filter: FilterMode,
    mag_filter: FilterMode,
    mip_filter: FilterMode,
    address_modes: [AddressMode; 3],
}

impl Default for SamplerDesc {
    fn default() -> Self {
        Self::new()
    }
}

impl SamplerDesc {
    /// Creates the description of a sampler with point filters and the
    /// address mode [`AddressMode::ClampToEdge`] on every axis.
    ///
    /// # Examples
    ///
    /// See the example of [`SamplerDesc`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            min_filter: FilterMode::Point,
            mag_filter: FilterMode::Point,
            mip_filter: FilterMode::Point,
            address_modes: [AddressMode::ClampToEdge; 3],
        }
    }

    /// Returns a copy with the filter of minification, which applies where
    /// a texel covers less than a pixel.
    ///
    /// # Examples
    ///
    /// See the example of [`SamplerDesc`].
    #[must_use]
    pub fn with_min_filter(self, filter: FilterMode) -> Self {
        Self {
            min_filter: filter,
            ..self
        }
    }

    /// Returns a copy with the filter of magnification, which applies where
    /// a texel covers more than a pixel.
    ///
    /// # Examples
    ///
    /// See the example of [`SamplerDesc`].
    #[must_use]
    pub fn with_mag_filter(self, filter: FilterMode) -> Self {
        Self {
            mag_filter: filter,
            ..self
        }
    }

    /// Returns a copy with the filter between mip levels.
    ///
    /// # Examples
    ///
    /// See the example of [`SamplerDesc`].
    #[must_use]
    pub fn with_mip_filter(self, filter: FilterMode) -> Self {
        Self {
            mip_filter: filter,
            ..self
        }
    }

    /// Returns a copy with the address modes of the three axes of a texture
    /// coordinate, in the order u, v, and w.
    ///
    /// # Examples
    ///
    /// See the example of [`SamplerDesc`].
    #[must_use]
    pub fn with_address_modes(self, address_modes: [AddressMode; 3]) -> Self {
        Self {
            address_modes,
            ..self
        }
    }

    /// Returns the raw descriptor, with every field that this description
    /// does not choose at a fixed value: the reduction `Average`, the
    /// comparison `Never`, no anisotropy, no bias, levels of detail from 0 to
    /// 1000, and a border colour of zero.
    fn to_raw(self) -> slang_rhi_sys::SamplerDesc {
        let [address_u, address_v, address_w] = self.address_modes.map(AddressMode::to_raw);
        slang_rhi_sys::SamplerDesc {
            structType: slang_rhi_sys::StructType::SamplerDesc,
            next: ptr::null(),
            minFilter: self.min_filter.to_raw(),
            magFilter: self.mag_filter.to_raw(),
            mipFilter: self.mip_filter.to_raw(),
            reductionOp: slang_rhi_sys::TextureReductionOp::Average,
            addressU: address_u,
            addressV: address_v,
            addressW: address_w,
            mipLODBias: 0.0,
            maxAnisotropy: 1,
            comparisonFunc: slang_rhi_sys::ComparisonFunc::Never,
            borderColor: [0.0; 4],
            minLOD: 0.0,
            maxLOD: 1000.0,
            label: ptr::null(),
        }
    }
}

/// A sampler of a device, which says how a shader reads a texture between and
/// beyond its texels.
///
/// Create one with [`Device::create_sampler`], and bind it to a shader with
/// [`ShaderObject::set_sampler`](crate::ShaderObject::set_sampler). A
/// sampler holds its [`Device`], so the device lives at least as long as the
/// sampler.
///
/// [`Clone`] returns another handle to the same sampler, which adds a
/// reference to it in slang-rhi.
///
/// A sampler is neither [`Send`] nor [`Sync`], because slang-rhi documents no
/// rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of [`Device::create_sampler`].
///
/// A sampler cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::Sampler>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::Sampler>();
/// ```
#[derive(Clone, Debug)]
pub struct Sampler {
    raw: Owned<ISampler>,
    device: Device,
}

impl Sampler {
    pub(crate) fn as_non_null(&self) -> NonNull<ISampler> {
        self.raw.as_non_null()
    }

    pub(crate) fn device_ptr(&self) -> NonNull<IDevice> {
        self.device.as_non_null()
    }
}

impl Device {
    /// Creates a sampler with the description `desc`.
    ///
    /// # Examples
    ///
    /// This example creates a sampler with the defaults of [`SamplerDesc`]. It
    /// needs a GPU driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::GlobalSession;
    /// use slang_rhi::{Device, DeviceOptions, SamplerDesc};
    ///
    /// let device = Device::new(&DeviceOptions::new(GlobalSession::new()?))?;
    /// let sampler = device.create_sampler(&SamplerDesc::new())?;
    /// # drop(sampler);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns the error that slang-rhi reports for the creation.
    pub fn create_sampler(&self, desc: &SamplerDesc) -> Result<Sampler> {
        let raw_desc = desc.to_raw();
        let device = self.as_ptr();
        let mut out: *mut ISampler = ptr::null_mut();
        let result = guard::call("IDevice::createSampler", || {
            // SAFETY: `device` is live while `self` lives, `raw_desc` is a
            // live descriptor with every field written, and `out` is a live
            // out-pointer.
            unsafe { IDevice::createSampler(device, &raw const raw_desc, &raw mut out) }
        });
        error::check(Operation::CreateSampler, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the sampler with
        // one reference added for the caller.
        let raw =
            unsafe { Owned::adopt(out) }.ok_or_else(|| Error::null(Operation::CreateSampler))?;
        Ok(Sampler {
            raw,
            device: self.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AddressMode, FilterMode, SamplerDesc};

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn the_value_types_of_samplers_are_send_and_sync() {
        assert_send_sync::<SamplerDesc>();
        assert_send_sync::<FilterMode>();
        assert_send_sync::<AddressMode>();
    }

    #[test]
    fn sampler_desc_writes_every_field_with_the_fixed_values() {
        let raw = SamplerDesc::new()
            .with_min_filter(FilterMode::Linear)
            .with_mip_filter(FilterMode::Linear)
            .with_address_modes([
                AddressMode::Wrap,
                AddressMode::MirrorRepeat,
                AddressMode::ClampToEdge,
            ])
            .to_raw();
        assert_eq!(raw.structType, slang_rhi_sys::StructType::SamplerDesc);
        assert!(raw.next.is_null());
        assert_eq!(raw.minFilter, slang_rhi_sys::TextureFilteringMode::Linear);
        assert_eq!(raw.magFilter, slang_rhi_sys::TextureFilteringMode::Point);
        assert_eq!(raw.mipFilter, slang_rhi_sys::TextureFilteringMode::Linear);
        assert_eq!(raw.reductionOp, slang_rhi_sys::TextureReductionOp::Average);
        assert_eq!(raw.addressU, slang_rhi_sys::TextureAddressingMode::Wrap);
        assert_eq!(
            raw.addressV,
            slang_rhi_sys::TextureAddressingMode::MirrorRepeat
        );
        assert_eq!(
            raw.addressW,
            slang_rhi_sys::TextureAddressingMode::ClampToEdge
        );
        assert_eq!(raw.mipLODBias.to_bits(), 0.0_f32.to_bits());
        assert_eq!(raw.maxAnisotropy, 1);
        assert_eq!(raw.comparisonFunc, slang_rhi_sys::ComparisonFunc::Never);
        assert_eq!(raw.borderColor.map(f32::to_bits), [0.0_f32.to_bits(); 4]);
        assert_eq!(raw.minLOD.to_bits(), 0.0_f32.to_bits());
        assert_eq!(raw.maxLOD.to_bits(), 1000.0_f32.to_bits());
        assert!(raw.label.is_null());
    }

    #[test]
    fn the_defaults_are_point_filters_and_clamped_addresses() {
        let raw = SamplerDesc::default().to_raw();
        assert_eq!(raw.minFilter, slang_rhi_sys::TextureFilteringMode::Point);
        assert_eq!(raw.magFilter, slang_rhi_sys::TextureFilteringMode::Point);
        assert_eq!(raw.mipFilter, slang_rhi_sys::TextureFilteringMode::Point);
        for address in [raw.addressU, raw.addressV, raw.addressW] {
            assert_eq!(address, slang_rhi_sys::TextureAddressingMode::ClampToEdge);
        }
    }
}
