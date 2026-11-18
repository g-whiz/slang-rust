use std::fmt;
use std::marker::PhantomData;

use crate::Device;
use crate::backend;
use crate::error::Result;

/// A sampler of a [`Device`], which states how a shader reads a
/// [`Texture`](crate::Texture) between its pixels and beyond its edges.
///
/// A kernel or a draw receives it as a `SamplerState`, which a shader passes
/// to a method of a texture such as `SampleLevel` or `Sample`. Create one with
/// [`Sampler::new`] from [`SamplerOptions`].
///
/// Cloning a sampler is cheap and returns another handle to the same
/// sampler. A sampler keeps its [`Device`] alive.
///
/// A sampler is neither [`Send`] nor [`Sync`]: it stays on the thread that
/// created its [`Device`], because the crate keeps all the GPU objects of a
/// device on one thread whatever graphics library runs them. A program sends
/// results to other threads as plain data, such as the vector that
/// [`Texture::read`](crate::Texture::read) returns. This program does not
/// compile, because a sampler is not `Send`:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slango::Sampler>();
/// ```
///
/// This program does not compile either, because a sampler is not `Sync`:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slango::Sampler>();
/// ```
///
/// # Examples
///
/// See the example of [`Sampler::new`].
#[derive(Clone)]
pub struct Sampler {
    device: Device,
    inner: backend::Sampler,
    options: SamplerOptions,
    thread_bound: PhantomData<*const ()>,
}

impl Sampler {
    /// Creates a sampler on the device with the options.
    ///
    /// # Examples
    ///
    /// This example creates a sampler that blends neighbouring pixels and
    /// repeats the texture beyond its edges. It needs a GPU driver or a
    /// software driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{AddressMode, Device, DeviceOptions, FilterMode, Sampler, SamplerOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let mut options = SamplerOptions::default();
    /// options.filter = FilterMode::Linear;
    /// options.address_mode = AddressMode::Repeat;
    /// let sampler = Sampler::new(&device, &options)?;
    /// # drop(sampler);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`](crate::Error) of kind
    /// [`ErrorKind::Backend`](crate::ErrorKind::Backend) when the backend
    /// reports an error while it creates the sampler. With the feature `rhi`, it returns
    /// an [`Error`](crate::Error) of kind
    /// [`ErrorKind::Unsupported`](crate::ErrorKind::Unsupported).
    pub fn new(device: &Device, options: &SamplerOptions) -> Result<Self> {
        let inner = backend::Sampler::new(device.backend(), options)?;
        Ok(Self {
            device: device.clone(),
            inner,
            options: options.clone(),
            thread_bound: PhantomData,
        })
    }

    /// Returns whether the sampler was created on `device` or a clone of it,
    /// and not on another [`Device`], even one built from the same wgpu
    /// device.
    pub(crate) fn is_on(&self, device: &Device) -> bool {
        self.device.is_same(device)
    }

    pub(crate) fn backend(&self) -> &backend::Sampler {
        &self.inner
    }
}

impl fmt::Debug for Sampler {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sampler")
            .field("options", &self.options)
            .finish_non_exhaustive()
    }
}

/// The options with which [`Sampler::new`] creates a sampler.
///
/// Start from [`SamplerOptions::default`], which reads the nearest pixel and
/// clamps coordinates to the edges, and change the fields that matter. The
/// struct is non-exhaustive, so a later version can add options, and a
/// program sets each field on a default value instead of writing a struct
/// literal.
///
/// # Examples
///
/// See the example of [`Sampler::new`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct SamplerOptions {
    /// How the sampler reads a texture between the centres of its pixels,
    /// when the texture is shown larger or smaller than its size.
    pub filter: FilterMode,
    /// How the sampler reads a texture at coordinates outside the range from
    /// 0.0 to 1.0, along both axes.
    pub address_mode: AddressMode,
}

/// How a [`Sampler`] reads a texture between the centres of its pixels.
///
/// The enumeration is non-exhaustive, so a later version can add modes, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Sampler::new`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FilterMode {
    /// The value of the pixel nearest to the coordinates.
    #[default]
    Nearest,
    /// The values of the four nearest pixels, blended by their distance to
    /// the coordinates.
    Linear,
}

/// How a [`Sampler`] reads a texture at coordinates outside the range from
/// 0.0 to 1.0.
///
/// The enumeration is non-exhaustive, so a later version can add modes, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`Sampler::new`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum AddressMode {
    /// The pixels at the edges extend beyond them.
    #[default]
    ClampToEdge,
    /// The texture repeats, so 1.25 reads what 0.25 reads.
    Repeat,
    /// The texture repeats with every second copy mirrored, so 1.25 reads
    /// what 0.75 reads.
    MirrorRepeat,
}

#[cfg(test)]
mod tests;
