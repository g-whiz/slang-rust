use std::ffi::c_void;
use std::ptr::{self, NonNull};

use slang_rhi_sys::{IDevice, ITexture, ITextureView};

use crate::device::{Device, DeviceType};
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::owned::Owned;

/// The format of the texels of a texture.
///
/// Each format has four bytes for each texel. The enumeration is
/// non-exhaustive, so a later version can add formats, and a `match` on it
/// needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`TextureDesc`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Format {
    /// Red, green, blue, and alpha of 8 bits each, read as values from 0 to 1.
    Rgba8Unorm,
    /// Red, green, blue, and alpha of 8 bits each in the sRGB colour space,
    /// read as linear values from 0 to 1.
    Rgba8UnormSrgb,
    /// Blue, green, red, and alpha of 8 bits each, read as values from 0 to 1.
    Bgra8Unorm,
    /// Blue, green, red, and alpha of 8 bits each in the sRGB colour space,
    /// read as linear values from 0 to 1.
    Bgra8UnormSrgb,
    /// A depth of a 32-bit floating-point number, for a depth target.
    D32Float,
}

impl Format {
    pub(crate) fn to_raw(self) -> slang_rhi_sys::Format {
        match self {
            Self::Rgba8Unorm => slang_rhi_sys::Format::RGBA8Unorm,
            Self::Rgba8UnormSrgb => slang_rhi_sys::Format::RGBA8UnormSrgb,
            Self::Bgra8Unorm => slang_rhi_sys::Format::BGRA8Unorm,
            Self::Bgra8UnormSrgb => slang_rhi_sys::Format::BGRA8UnormSrgb,
            Self::D32Float => slang_rhi_sys::Format::D32Float,
        }
    }

    /// Returns whether the format holds a depth instead of a colour.
    pub(crate) fn is_depth(self) -> bool {
        self == Self::D32Float
    }

    /// The number of bytes of one texel, which is 4 for every format.
    pub(crate) const TEXEL_SIZE: u64 = 4;
}

bitflags::bitflags! {
    /// The ways in which a texture is used, which a caller combines with `|`.
    ///
    /// slang-rhi creates the texture with the access that these uses need,
    /// and the crate checks each use of a texture against them. A texture
    /// needs at least one use. A texture of a colour format cannot have
    /// [`TextureUsage::DEPTH_STENCIL`], and a texture of
    /// [`Format::D32Float`] has exactly that use.
    ///
    /// # Examples
    ///
    /// ```
    /// use slang_rhi::TextureUsage;
    ///
    /// let usage = TextureUsage::RENDER_TARGET | TextureUsage::COPY_SOURCE;
    /// assert!(usage.contains(TextureUsage::COPY_SOURCE));
    /// ```
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct TextureUsage: u32 {
        /// A shader samples or reads the texture.
        const SHADER_RESOURCE = 1;
        /// A render pass draws into the texture as its colour target.
        const RENDER_TARGET = 1 << 2;
        /// A render pass uses the texture as its depth target.
        const DEPTH_STENCIL = 1 << 3;
        /// The texture is the source of a copy, such as that of
        /// [`Device::read_texture`].
        const COPY_SOURCE = 1 << 5;
        /// The texture is the destination of a copy.
        const COPY_DESTINATION = 1 << 6;
    }
}

/// The description of a two-dimensional texture that
/// [`Device::create_texture`] creates, with one mip level, one layer, and
/// one sample.
///
/// # Examples
///
/// This example describes a texture of 64 by 32 texels that a render pass
/// draws into and the program reads back:
///
/// ```
/// use slang_rhi::{Format, TextureDesc, TextureUsage};
///
/// let desc = TextureDesc::new(
///     [64, 32],
///     Format::Rgba8Unorm,
///     TextureUsage::RENDER_TARGET | TextureUsage::COPY_SOURCE,
/// );
/// # let _ = desc;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TextureDesc {
    size: [u32; 2],
    format: Format,
    usage: TextureUsage,
}

impl TextureDesc {
    /// Creates the description of a texture of `size` texels, the width and
    /// the height, of the format `format` with the uses `usage`.
    ///
    /// # Examples
    ///
    /// See the example of [`TextureDesc`].
    #[must_use]
    pub fn new(size: [u32; 2], format: Format, usage: TextureUsage) -> Self {
        Self {
            size,
            format,
            usage,
        }
    }

    /// Returns the raw descriptor, with every field that this description
    /// does not choose at the default of the header.
    fn to_raw(self) -> Result<slang_rhi_sys::TextureDesc> {
        let usage = i32::try_from(self.usage.bits()).map_err(|_overflow| {
            Error::invalid(
                Operation::CreateTexture,
                "the usage flags do not fit slang-rhi",
            )
        })?;
        let [width, height] = self.size;
        Ok(slang_rhi_sys::TextureDesc {
            structType: slang_rhi_sys::StructType::TextureDesc,
            next: ptr::null(),
            r#type: slang_rhi_sys::TextureType::Texture2D,
            size: slang_rhi_sys::Extent3D {
                width,
                height,
                depth: 1,
            },
            arrayLength: 1,
            mipCount: 1,
            format: self.format.to_raw(),
            sampleCount: 1,
            sampleQuality: 0,
            memoryType: slang_rhi_sys::MemoryType::DeviceLocal,
            usage: slang_rhi_sys::TextureUsage(usage),
            defaultState: slang_rhi_sys::ResourceState::Undefined,
            optimalClearValue: ptr::null(),
            sampler: ptr::null_mut(),
            label: ptr::null(),
        })
    }
}

/// Checks that a description has a size within `max_dimension` along each
/// axis, at least one use, no bit of a use that this crate does not define,
/// and the uses that its format allows.
fn check_desc(desc: &TextureDesc, max_dimension: u32) -> Result<()> {
    let invalid = |detail: String| Err(Error::invalid(Operation::CreateTexture, detail));
    let [width, height] = desc.size;
    if width == 0 || height == 0 {
        return invalid(format!(
            "the texture of {width} by {height} texels has no texel"
        ));
    }
    if width > max_dimension || height > max_dimension {
        return invalid(format!(
            "the texture of {width} by {height} texels is larger than the {max_dimension} texels along an axis that the device allows"
        ));
    }
    if desc.usage.is_empty() {
        return invalid(String::from("the texture has no use"));
    }
    let unknown = desc.usage.bits() & !TextureUsage::all().bits();
    if unknown != 0 {
        return invalid(format!(
            "the uses of the texture hold the unknown bits {unknown:#x}"
        ));
    }
    if desc.format.is_depth() && desc.usage != TextureUsage::DEPTH_STENCIL {
        return invalid(format!(
            "a texture of the depth format {:?} has the use DEPTH_STENCIL and no other",
            desc.format
        ));
    }
    if !desc.format.is_depth() && desc.usage.contains(TextureUsage::DEPTH_STENCIL) {
        return invalid(format!(
            "a texture of the colour format {:?} cannot have the use DEPTH_STENCIL",
            desc.format
        ));
    }
    Ok(())
}

/// Returns the number of bytes of all the texels of a texture of `size`
/// texels, or `None` when the number does not fit a `u64`.
fn texel_bytes(size: [u32; 2]) -> Option<u64> {
    let [width, height] = size;
    u64::from(width)
        .checked_mul(u64::from(height))?
        .checked_mul(Format::TEXEL_SIZE)
}

/// Checks that a texture of the description `desc` can take initial data of
/// `len` bytes, which holds every texel, row after row from the top.
fn check_initial(desc: &TextureDesc, len: usize) -> Result<()> {
    if desc.format.is_depth() {
        return Err(Error::invalid(
            Operation::CreateTexture,
            "a texture of a depth format takes no initial data",
        ));
    }
    if texel_bytes(desc.size) == u64::try_from(len).ok() {
        Ok(())
    } else {
        let [width, height] = desc.size;
        Err(Error::invalid(
            Operation::CreateTexture,
            format!(
                "the initial data has {len} bytes, and {width} by {height} texels of 4 bytes need {}",
                u128::from(width) * u128::from(height) * 4
            ),
        ))
    }
}

/// Checks that a device whose support of the format `format` is `support`
/// can create a texture of that format with the uses `usage`.
///
/// Only a depth target needs the check: every colour format of [`Format`]
/// supports every use of [`TextureUsage`] on both graphics APIs that the
/// crate opens, and Direct3D 12 also requires a depth target of
/// [`Format::D32Float`], which Vulkan does not.
fn check_support(
    format: Format,
    usage: TextureUsage,
    support: slang_rhi_sys::FormatSupport,
) -> Result<()> {
    let depth = slang_rhi_sys::FormatSupport::DepthStencil.0;
    if usage.contains(TextureUsage::DEPTH_STENCIL) && support.0 & depth == 0 {
        Err(Error::invalid(
            Operation::CreateTexture,
            format!("the device does not support the format {format:?} as a depth target"),
        ))
    } else {
        Ok(())
    }
}

/// Checks that a texture with the format `format` and the uses `usage` can be
/// read back into `len` bytes, the number of bytes of its `size` texels.
fn check_readable(format: Format, usage: TextureUsage, size: [u32; 2], len: usize) -> Result<()> {
    let invalid = |detail: String| Err(Error::invalid(Operation::ReadTexture, detail));
    if format.is_depth() {
        return invalid(format!(
            "a texture of the depth format {format:?} cannot be read back"
        ));
    }
    if !usage.contains(TextureUsage::COPY_SOURCE) {
        return invalid(String::from(
            "the texture does not have the use COPY_SOURCE",
        ));
    }
    if texel_bytes(size) != u64::try_from(len).ok() {
        let [width, height] = size;
        return invalid(format!(
            "the output has {len} bytes, and {width} by {height} texels of 4 bytes need {}",
            u128::from(width) * u128::from(height) * 4
        ));
    }
    Ok(())
}

/// Copies the rows of a texture of `size` texels from `staged`, whose rows
/// are `layout.rowPitch` bytes apart, into `out`, whose rows follow each
/// other without padding.
///
/// Returns an error of kind [`ErrorKind::Failed`](crate::ErrorKind::Failed)
/// when the layout that slang-rhi reported does not describe the texture or
/// does not fit `staged`.
fn copy_rows(
    layout: &slang_rhi_sys::SubresourceLayout,
    size: [u32; 2],
    staged: &[u8],
    out: &mut [u8],
) -> Result<()> {
    let failed = || {
        Error::failed(
            Operation::ReadTexture,
            "slang-rhi reported a layout that does not describe the texture",
        )
    };
    let [width, height] = size;
    if layout.size.width != width || layout.size.height != height || layout.size.depth != 1 {
        return Err(failed());
    }
    let row = usize::try_from(width)
        .ok()
        .and_then(|width| width.checked_mul(4))
        .ok_or_else(failed)?;
    if layout.rowPitch < row || row == 0 {
        return Err(failed());
    }
    for (index, target) in out.chunks_exact_mut(row).enumerate() {
        let start = index.checked_mul(layout.rowPitch).ok_or_else(failed)?;
        let end = start.checked_add(row).ok_or_else(failed)?;
        let source = staged.get(start..end).ok_or_else(failed)?;
        target.copy_from_slice(source);
    }
    Ok(())
}

/// A two-dimensional texture of a device, with one mip level, one layer, and
/// one sample.
///
/// Create one with [`Device::create_texture`], bind it to a shader with
/// [`ShaderObject::set_texture`](crate::ShaderObject::set_texture), draw into
/// it in a render pass with
/// [`CommandEncoder::render_pass`](crate::CommandEncoder::render_pass), and
/// read it back with [`Device::read_texture`]. A texture holds its
/// [`Device`], so the device lives at least as long as the texture.
///
/// [`Clone`] returns another handle to the same texture, which adds a
/// reference to it in slang-rhi.
///
/// A texture is neither [`Send`] nor [`Sync`], because slang-rhi documents no
/// rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of [`Device::create_texture`].
///
/// A texture cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::Texture>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::Texture>();
/// ```
#[derive(Clone, Debug)]
pub struct Texture {
    view: Owned<ITextureView>,
    raw: Owned<ITexture>,
    device: Device,
    desc: TextureDesc,
}

impl Texture {
    /// Returns the width and the height of the texture in texels.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::create_texture`].
    #[must_use]
    pub fn size(&self) -> [u32; 2] {
        self.desc.size
    }

    /// Returns the format of the texture.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::create_texture`].
    #[must_use]
    pub fn format(&self) -> Format {
        self.desc.format
    }

    /// Returns the uses of the texture.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::create_texture`].
    #[must_use]
    pub fn usage(&self) -> TextureUsage {
        self.desc.usage
    }

    pub(crate) fn as_non_null(&self) -> NonNull<ITexture> {
        self.raw.as_non_null()
    }

    pub(crate) fn view(&self) -> NonNull<ITextureView> {
        self.view.as_non_null()
    }

    pub(crate) fn device_ptr(&self) -> NonNull<IDevice> {
        self.device.as_non_null()
    }
}

impl Device {
    /// Creates a texture with the description `desc`, filled with `initial`
    /// when it is given.
    ///
    /// The initial data holds every texel, four bytes each in the order of
    /// the channels of the format, row after row from the top row. A texture
    /// without initial data has undefined contents on Vulkan until a render
    /// pass clears or draws into it. A texture of [`Format::D32Float`] takes
    /// no initial data, so clear it with a render pass that has it as its
    /// only target.
    ///
    /// # Examples
    ///
    /// This example creates a texture of 2 by 2 texels and reads it back. It
    /// needs a GPU driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_compiler::GlobalSession;
    /// use slang_rhi::{Device, DeviceOptions, Format, TextureDesc, TextureUsage};
    ///
    /// let device = Device::new(&DeviceOptions::new(GlobalSession::new()?))?;
    /// let texels: Vec<u8> = (0..16).collect();
    /// let desc = TextureDesc::new(
    ///     [2, 2],
    ///     Format::Rgba8Unorm,
    ///     TextureUsage::SHADER_RESOURCE | TextureUsage::COPY_SOURCE,
    /// );
    /// let texture = device.create_texture(&desc, Some(&texels))?;
    /// assert_eq!(texture.size(), [2, 2]);
    /// assert_eq!(texture.format(), Format::Rgba8Unorm);
    /// assert!(texture.usage().contains(TextureUsage::COPY_SOURCE));
    /// let mut out = vec![0_u8; 16];
    /// device.read_texture(&texture, &mut out)?;
    /// assert_eq!(out, texels);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before the call that creates the texture, when its width or height is
    /// 0 or larger than
    /// [`DeviceInfo::max_texture_dimension_2d`](crate::DeviceInfo::max_texture_dimension_2d),
    /// when its uses are empty or hold a bit that [`TextureUsage`] does not
    /// define, when a texture of a depth format has a use other than
    /// [`TextureUsage::DEPTH_STENCIL`] or one of a colour format has that
    /// use, when a Vulkan device does not support the depth format as a depth
    /// target, or when `initial` is given for a depth format or its length is
    /// not 4 bytes for each texel. Returns the error that slang-rhi reports
    /// for the creation.
    pub fn create_texture(&self, desc: &TextureDesc, initial: Option<&[u8]>) -> Result<Texture> {
        check_desc(desc, self.info().max_texture_dimension_2d())?;
        if let Some(initial) = initial {
            check_initial(desc, initial.len())?;
        }
        if desc.format.is_depth() && self.info().device_type() == DeviceType::Vulkan {
            let support = self.format_support(desc.format, Operation::CreateTexture)?;
            check_support(desc.format, desc.usage, support)?;
        }
        let raw_desc = desc.to_raw()?;
        let [width, _] = desc.size;
        let row_pitch =
            usize::try_from(u64::from(width) * Format::TEXEL_SIZE).map_err(|_overflow| {
                Error::invalid(
                    Operation::CreateTexture,
                    "a row of the texture does not fit memory",
                )
            })?;
        let data = initial.map(|initial| slang_rhi_sys::SubresourceData {
            data: initial.as_ptr().cast::<c_void>(),
            rowPitch: row_pitch,
            slicePitch: initial.len(),
        });
        let data_ptr = data.as_ref().map_or(ptr::null(), ptr::from_ref);
        let device = self.as_ptr();
        let mut out: *mut ITexture = ptr::null_mut();
        let result = guard::call("IDevice::createTexture", || {
            // SAFETY: `device` is live while `self` lives, `raw_desc` is a
            // live descriptor of a two-dimensional texture with one mip
            // level, one layer, and one sample, `data_ptr` is null or points
            // to one live record of initial data whose rows of `row_pitch`
            // bytes cover exactly the bytes of `initial` by the checks above,
            // which slang-rhi copies before it returns, and `out` is a live
            // out-pointer.
            unsafe { IDevice::createTexture(device, &raw const raw_desc, data_ptr, &raw mut out) }
        });
        error::check(Operation::CreateTexture, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the texture with
        // one reference added for the caller.
        let raw =
            unsafe { Owned::adopt(out) }.ok_or_else(|| Error::null(Operation::CreateTexture))?;
        let texture = raw.as_ptr();
        let mut view: *mut ITextureView = ptr::null_mut();
        let result = guard::call("ITexture::getDefaultView", || {
            // SAFETY: `texture` is live while `raw` lives, and `view` is a
            // live out-pointer.
            unsafe { ITexture::getDefaultView(texture, &raw mut view) }
        });
        error::check(Operation::CreateTexture, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the default view
        // with one reference added for the caller.
        let view =
            unsafe { Owned::adopt(view) }.ok_or_else(|| Error::null(Operation::CreateTexture))?;
        Ok(Texture {
            view,
            raw,
            device: self.clone(),
            desc: *desc,
        })
    }

    /// Reads every texel of `texture` into `out`, four bytes each in the
    /// order of the channels of its format, row after row from the top row.
    ///
    /// The call waits until the device has finished the copy and every
    /// command buffer that the queue of the device received before it. The
    /// texture needs a colour format and the use
    /// [`TextureUsage::COPY_SOURCE`].
    ///
    /// # Examples
    ///
    /// See the example of [`Device::create_texture`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the texture belongs to another
    /// device, has a depth format, or does not have the use
    /// [`TextureUsage::COPY_SOURCE`], or when the length of `out` is not 4
    /// bytes for each texel. Returns an error of kind
    /// [`ErrorKind::Failed`](crate::ErrorKind::Failed) when slang-rhi
    /// reports a layout that does not describe the texture, and the error
    /// that slang-rhi reports for the layout or the read.
    pub fn read_texture(&self, texture: &Texture, out: &mut [u8]) -> Result<()> {
        if texture.device_ptr() != self.as_non_null() {
            return Err(Error::invalid(
                Operation::ReadTexture,
                "the texture belongs to another device",
            ));
        }
        check_readable(
            texture.desc.format,
            texture.desc.usage,
            texture.desc.size,
            out.len(),
        )?;
        let raw = texture.raw.as_ptr();
        let mut layout = slang_rhi_sys::SubresourceLayout {
            size: slang_rhi_sys::Extent3D::default(),
            colPitch: 0,
            rowPitch: 0,
            slicePitch: 0,
            sizeInBytes: 0,
            blockWidth: 0,
            blockHeight: 0,
            rowCount: 0,
        };
        let result = guard::call("ITexture::getSubresourceLayout", || {
            // SAFETY: `raw` is live while `texture` lives, mip level 0
            // exists, and `layout` is a live out-pointer.
            unsafe {
                ITexture::getSubresourceLayout(
                    raw,
                    0,
                    slang_rhi_sys::kDefaultAlignment,
                    &raw mut layout,
                )
            }
        });
        error::check(Operation::ReadTexture, result, None)?;
        let mut staged = vec![0_u8; layout.sizeInBytes];
        let device = self.as_ptr();
        let destination = staged.as_mut_ptr().cast::<c_void>();
        let result = guard::call("IDevice::readTexture", || {
            // SAFETY: `device` and `raw` are live while `self` and `texture`
            // live, the texture belongs to the device and has the use of a
            // copy source, layer 0 and mip level 0 exist, `layout` is the
            // layout that slang-rhi reported for that mip level, and
            // `destination` points to the `sizeInBytes` writable bytes of
            // `staged`, which is as many as slang-rhi copies.
            unsafe { IDevice::readTexture(device, raw, 0, 0, &raw const layout, destination) }
        });
        error::check(Operation::ReadTexture, result, None)?;
        copy_rows(&layout, texture.desc.size, &staged, out)
    }

    /// Returns what the device supports of the format `format`.
    fn format_support(
        &self,
        format: Format,
        operation: Operation,
    ) -> Result<slang_rhi_sys::FormatSupport> {
        let device = self.as_ptr();
        let raw_format = format.to_raw();
        let mut support = slang_rhi_sys::FormatSupport::None;
        let result = guard::call("IDevice::getFormatSupport", || {
            // SAFETY: `device` is live while `self` lives, and `support` is a
            // live out-pointer.
            unsafe { IDevice::getFormatSupport(device, raw_format, &raw mut support) }
        });
        error::check(operation, result, None)?;
        Ok(support)
    }
}

#[cfg(test)]
mod tests {
    use std::ptr;

    use super::{
        Format, TextureDesc, TextureUsage, check_desc, check_initial, check_readable,
        check_support, copy_rows,
    };
    use crate::ErrorKind;
    use crate::error::Operation;

    fn assert_send_sync<T: Send + Sync + 'static>() {}

    #[test]
    fn the_value_types_of_textures_are_send_and_sync() {
        assert_send_sync::<Format>();
        assert_send_sync::<TextureUsage>();
        assert_send_sync::<TextureDesc>();
    }

    const COLOUR: TextureUsage = TextureUsage::RENDER_TARGET.union(TextureUsage::COPY_SOURCE);

    #[test]
    fn check_desc_accepts_sizes_up_to_the_limit_and_the_uses_of_each_format() {
        for desc in [
            TextureDesc::new([1, 1], Format::Rgba8Unorm, COLOUR),
            TextureDesc::new(
                [16384, 3],
                Format::Bgra8UnormSrgb,
                TextureUsage::all().difference(TextureUsage::DEPTH_STENCIL),
            ),
            TextureDesc::new([4, 4], Format::D32Float, TextureUsage::DEPTH_STENCIL),
        ] {
            check_desc(&desc, 16384).expect("the description is valid");
        }
    }

    #[test]
    fn check_desc_rejects_an_empty_or_oversized_texture_and_uses_the_format_does_not_allow() {
        for desc in [
            TextureDesc::new([0, 4], Format::Rgba8Unorm, COLOUR),
            TextureDesc::new([4, 0], Format::Rgba8Unorm, COLOUR),
            TextureDesc::new([16385, 4], Format::Rgba8Unorm, COLOUR),
            TextureDesc::new([4, 16385], Format::Rgba8Unorm, COLOUR),
            TextureDesc::new([4, 4], Format::Rgba8Unorm, TextureUsage::empty()),
            TextureDesc::new(
                [4, 4],
                Format::Rgba8Unorm,
                TextureUsage::from_bits_retain(1 << 1),
            ),
            TextureDesc::new(
                [4, 4],
                Format::Rgba8Unorm,
                COLOUR | TextureUsage::from_bits_retain(1 << 9),
            ),
            TextureDesc::new(
                [4, 4],
                Format::Rgba8Unorm,
                COLOUR | TextureUsage::DEPTH_STENCIL,
            ),
            TextureDesc::new(
                [4, 4],
                Format::D32Float,
                TextureUsage::DEPTH_STENCIL | TextureUsage::SHADER_RESOURCE,
            ),
            TextureDesc::new([4, 4], Format::D32Float, TextureUsage::COPY_SOURCE),
        ] {
            let error = check_desc(&desc, 16384).expect_err("the description is refused");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{desc:?}");
            assert_eq!(error.operation(), Operation::CreateTexture);
        }
    }

    #[test]
    fn check_initial_accepts_four_bytes_for_each_texel_of_a_colour_texture_only() {
        let colour = TextureDesc::new([3, 2], Format::Rgba8Unorm, COLOUR);
        check_initial(&colour, 24).expect("every texel");
        for len in [0, 23, 25] {
            let error = check_initial(&colour, len).expect_err("the wrong length");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        }
        let depth = TextureDesc::new([3, 2], Format::D32Float, TextureUsage::DEPTH_STENCIL);
        let error = check_initial(&depth, 24).expect_err("a depth texture takes none");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::CreateTexture);
    }

    #[test]
    fn check_support_refuses_a_depth_target_that_the_device_does_not_support() {
        let depth = slang_rhi_sys::FormatSupport(
            slang_rhi_sys::FormatSupport::DepthStencil.0 | slang_rhi_sys::FormatSupport::Texture.0,
        );
        check_support(Format::D32Float, TextureUsage::DEPTH_STENCIL, depth).expect("supported");
        let error = check_support(
            Format::D32Float,
            TextureUsage::DEPTH_STENCIL,
            slang_rhi_sys::FormatSupport::Texture,
        )
        .expect_err("no depth target");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        check_support(
            Format::Rgba8Unorm,
            COLOUR,
            slang_rhi_sys::FormatSupport::None,
        )
        .expect("a colour use needs no check");
    }

    #[test]
    fn check_readable_requires_a_colour_format_the_use_copy_source_and_every_texel() {
        check_readable(Format::Rgba8Unorm, COLOUR, [3, 2], 24).expect("readable");
        for (format, usage, len) in [
            (Format::D32Float, TextureUsage::DEPTH_STENCIL, 24),
            (Format::Rgba8Unorm, TextureUsage::RENDER_TARGET, 24),
            (Format::Rgba8Unorm, COLOUR, 23),
            (Format::Rgba8Unorm, COLOUR, 0),
        ] {
            let error = check_readable(format, usage, [3, 2], len).expect_err("refused");
            assert_eq!(
                error.kind(),
                ErrorKind::InvalidArgument,
                "{format:?} {usage:?} {len}"
            );
            assert_eq!(error.operation(), Operation::ReadTexture);
        }
    }

    fn layout(size: [u32; 2], row_pitch: usize) -> slang_rhi_sys::SubresourceLayout {
        let [width, height] = size;
        slang_rhi_sys::SubresourceLayout {
            size: slang_rhi_sys::Extent3D {
                width,
                height,
                depth: 1,
            },
            colPitch: 4,
            rowPitch: row_pitch,
            slicePitch: row_pitch * usize::try_from(height).expect("small"),
            sizeInBytes: row_pitch * usize::try_from(height).expect("small"),
            blockWidth: 1,
            blockHeight: 1,
            rowCount: usize::try_from(height).expect("small"),
        }
    }

    #[test]
    fn copy_rows_drops_the_padding_of_each_row() {
        let staged: Vec<u8> = (0..32).collect();
        let mut out = vec![0_u8; 16];
        copy_rows(&layout([2, 2], 16), [2, 2], &staged, &mut out).expect("the layout fits");
        let expected: Vec<u8> = (0..8).chain(16..24).collect();
        assert_eq!(out, expected);
    }

    #[test]
    fn copy_rows_refuses_a_layout_that_does_not_describe_the_texture_or_fit_the_data() {
        let staged = [0_u8; 32];
        let mut out = vec![0_u8; 16];
        for (layout, staged) in [
            (layout([2, 3], 16), &staged[..]),
            (layout([3, 2], 16), &staged[..]),
            (layout([2, 2], 4), &staged[..]),
            (layout([2, 2], 16), &staged[..20]),
        ] {
            let error = copy_rows(&layout, [2, 2], staged, &mut out).expect_err("refused");
            assert_eq!(error.kind(), ErrorKind::Failed);
            assert_eq!(error.operation(), Operation::ReadTexture);
        }
    }

    #[test]
    fn texture_desc_writes_every_field_with_the_defaults_of_the_header() {
        let raw = TextureDesc::new([5, 7], Format::Bgra8Unorm, COLOUR)
            .to_raw()
            .expect("the flags fit");
        assert_eq!(raw.structType, slang_rhi_sys::StructType::TextureDesc);
        assert!(raw.next.is_null());
        assert_eq!(raw.r#type, slang_rhi_sys::TextureType::Texture2D);
        assert_eq!((raw.size.width, raw.size.height, raw.size.depth), (5, 7, 1));
        assert_eq!(raw.arrayLength, 1);
        assert_eq!(raw.mipCount, 1);
        assert_eq!(raw.format, slang_rhi_sys::Format::BGRA8Unorm);
        assert_eq!(raw.sampleCount, 1);
        assert_eq!(raw.sampleQuality, 0);
        assert_eq!(raw.memoryType, slang_rhi_sys::MemoryType::DeviceLocal);
        assert_eq!(
            raw.usage,
            slang_rhi_sys::TextureUsage(
                slang_rhi_sys::TextureUsage::RenderTarget.0
                    | slang_rhi_sys::TextureUsage::CopySource.0
            )
        );
        assert_eq!(raw.defaultState, slang_rhi_sys::ResourceState::Undefined);
        assert_eq!(raw.optimalClearValue, ptr::null());
        assert!(raw.sampler.is_null());
        assert!(raw.label.is_null());
    }

    #[test]
    fn each_format_and_usage_flag_has_the_value_of_the_header() {
        for (format, raw) in [
            (Format::Rgba8Unorm, slang_rhi_sys::Format::RGBA8Unorm),
            (
                Format::Rgba8UnormSrgb,
                slang_rhi_sys::Format::RGBA8UnormSrgb,
            ),
            (Format::Bgra8Unorm, slang_rhi_sys::Format::BGRA8Unorm),
            (
                Format::Bgra8UnormSrgb,
                slang_rhi_sys::Format::BGRA8UnormSrgb,
            ),
            (Format::D32Float, slang_rhi_sys::Format::D32Float),
        ] {
            assert_eq!(format.to_raw(), raw);
        }
        for (flag, raw) in [
            (
                TextureUsage::SHADER_RESOURCE,
                slang_rhi_sys::TextureUsage::ShaderResource,
            ),
            (
                TextureUsage::RENDER_TARGET,
                slang_rhi_sys::TextureUsage::RenderTarget,
            ),
            (
                TextureUsage::DEPTH_STENCIL,
                slang_rhi_sys::TextureUsage::DepthStencil,
            ),
            (
                TextureUsage::COPY_SOURCE,
                slang_rhi_sys::TextureUsage::CopySource,
            ),
            (
                TextureUsage::COPY_DESTINATION,
                slang_rhi_sys::TextureUsage::CopyDestination,
            ),
        ] {
            assert_eq!(i32::try_from(flag.bits()).ok(), Some(raw.0), "{flag:?}");
        }
        assert_eq!(TextureUsage::all().bits().count_ones(), 5);
    }
}
