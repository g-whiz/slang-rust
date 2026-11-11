use slango::{Device, Draw, RenderPipeline, Texture, TextureFormat};

use crate::error::{Error, ErrorKind, Result};

/// A texture that a render pipeline of `slango` draws into and that Slint
/// shows as an image.
///
/// Use a view to show what a Slang shader draws inside a Slint user
/// interface. A render pipeline is the pair of a vertex and a fragment
/// entry point, the functions of a shader that place the triangles and
/// colour their pixels, as [`slango::RenderPipeline`] describes. The view
/// owns the device, the pipeline, and a texture of the format
/// [`TextureFormat::Rgba8Unorm`] whose size is in physical pixels, the
/// pixels of the window's surface. [`View::draw`] starts a draw into the
/// texture, and [`View::to_image`] turns the texture into a
/// [`slint::Image`] for an `Image` element of Slint. Every later draw shows
/// in that image on the next frame that Slint renders, and
/// [`View::resize`] replaces the texture when the element changes size.
///
/// The pipeline needs the colour format [`TextureFormat::Rgba8Unorm`],
/// which is the default of [`slango::RenderPipelineOptions`]. A pipeline of
/// another colour format, or of another device, fails at the draw. The view
/// has no depth target, so a pipeline with a depth format is given one with
/// [`Draw::depth`] at each draw.
///
/// A view holds nothing of Slint and is [`Send`] and [`Sync`]. The image
/// that [`View::to_image`] returns is neither, so the program creates and
/// sets it on the thread that runs Slint's event loop.
///
/// # Examples
///
/// This example creates a view of 64 by 32 pixels, fills it with one
/// colour that it sets by name, and reads the colour back. It draws one
/// triangle that covers the whole texture, with no vertex buffer. It needs
/// a GPU driver or a software driver at run time, so it is compiled but not
/// run:
///
/// ```no_run
/// use slang_slint::View;
/// use slango::{Device, DeviceOptions, RenderPipelineOptions};
///
/// let device = Device::new(&DeviceOptions::default())?;
/// let module = device.load_module_from_source(
///     "fill",
///     r#"
///         uniform float4 colour;
///
///         struct VOut { float4 position : SV_Position; };
///
///         [shader("vertex")]
///         VOut vsMain(uint vertex : SV_VulkanVertexID)
///         {
///             float2 corner = float2(float((vertex << 1) & 2), float(vertex & 2));
///             VOut output;
///             output.position = float4(corner * 2.0 - 1.0, 0.0, 1.0);
///             return output;
///         }
///
///         [shader("fragment")]
///         float4 fsMain(VOut input) : SV_Target
///         {
///             return colour;
///         }
///     "#,
/// )?;
/// let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
/// let view = View::new(device, pipeline, [64, 32])?;
/// view.draw().set("colour", [1.0_f32, 0.0, 0.0, 1.0]).submit(3, 1)?;
/// assert!(view.texture().read()?.iter().all(|pixel| *pixel == [255, 0, 0, 255]));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Debug)]
pub struct View {
    device: Device,
    pipeline: RenderPipeline,
    texture: Texture,
}

impl View {
    /// Creates a view that draws with a render pipeline on a device into a
    /// new texture of `[width, height]` physical pixels.
    ///
    /// Call it once the device exists, such as when Slint sets up rendering,
    /// with the size of the `Image` element in physical pixels, which is its
    /// size in the `.slint` file times the scale factor of the window. The
    /// texture has the format [`TextureFormat::Rgba8Unorm`], so the colours
    /// that the shader writes appear unchanged. The view does not check the
    /// pipeline, whose colour format and device the draw checks.
    ///
    /// # Examples
    ///
    /// See the example of [`View`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::InvalidSize`] when the
    /// width or the height is zero, and of kind [`ErrorKind::Slango`] when
    /// `slango` cannot create the texture, such as when the size exceeds the
    /// largest texture that the device allows.
    pub fn new(device: Device, pipeline: RenderPipeline, size: [u32; 2]) -> Result<Self> {
        const CREATE: &str = "create a view";
        check_size(size, CREATE)?;
        let texture = create_texture(&device, size, CREATE)?;
        Ok(Self {
            device,
            pipeline,
            texture,
        })
    }

    /// Starts a draw with the view's render pipeline into the view's
    /// texture.
    ///
    /// The returned [`Draw`] has the texture as its colour target and
    /// nothing else set. Set the parameters of the shader by name, its
    /// buffers, textures, samplers, vertex buffers, and a clear colour on it
    /// as for [`RenderPipeline::draw`], and then submit it. A draw keeps the
    /// pixels that the texture holds and draws over them unless it clears
    /// the texture first with [`Draw::clear`].
    ///
    /// # Examples
    ///
    /// See the example of [`View`].
    #[must_use]
    pub fn draw(&self) -> Draw<'_> {
        let mut draw = self.pipeline.draw();
        draw.target(&self.texture);
        draw
    }

    /// Returns an image of Slint that shows the view's current texture.
    ///
    /// Set the image on an `Image` element of Slint after [`View::new`] and
    /// after each call of [`View::resize`] that returns `true`. The image
    /// shares the texture on the GPU, so every later draw into the texture
    /// shows on the next frame that Slint renders, without a new image.
    ///
    /// # Examples
    ///
    /// This example creates a view of the size of an element of 200 by 100
    /// logical pixels in a window with a scale factor of 2, and makes its
    /// image. The program sets the image on its `Image` element. It needs a
    /// GPU driver or a software driver at run time, so it is compiled but
    /// not run:
    ///
    /// ```no_run
    /// use slang_slint::View;
    /// use slango::{Device, DeviceOptions, RenderPipelineOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module("gradient.slang")?;
    /// let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
    /// let view = View::new(device, pipeline, [400, 200])?;
    /// let image: slint::Image = view.to_image()?;
    /// assert_eq!([image.size().width, image.size().height], [400, 200]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::TextureImport`] when Slint
    /// rejects the texture. Slint accepts every texture of a view, so this
    /// error is a defect of this crate.
    pub fn to_image(&self) -> Result<slint::Image> {
        let texture = slango::wgpu::texture(&self.texture).clone();
        slint::Image::try_from(texture).map_err(|error| {
            Error::new(ErrorKind::TextureImport, "make an image of a view").with_source(error)
        })
    }

    /// Replaces the view's texture with a new one of `[width, height]`
    /// physical pixels when the size differs from the current size, and
    /// returns whether it did.
    ///
    /// Call it before each draw with the current size of the `Image`
    /// element in physical pixels. When it returns `true`, the new texture
    /// holds zeros and the image that Slint holds still shows the old
    /// texture, so set the image of [`View::to_image`] on the element. An
    /// equal size returns `false` and keeps the texture with its pixels.
    ///
    /// # Examples
    ///
    /// This example resizes a view when its element grows, and sets the new
    /// image only then. It needs a GPU driver or a software driver at run
    /// time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slang_slint::View;
    /// use slango::{Device, DeviceOptions, RenderPipelineOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let module = device.load_module("gradient.slang")?;
    /// let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
    /// let mut view = View::new(device, pipeline, [400, 200])?;
    /// let mut image = view.to_image()?;
    /// for size in [[400, 200], [640, 480]] {
    ///     if view.resize(size)? {
    ///         image = view.to_image()?;
    ///     }
    ///     view.draw().submit(3, 1)?;
    /// }
    /// assert_eq!(view.size(), [640, 480]);
    /// assert_eq!([image.size().width, image.size().height], [640, 480]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::InvalidSize`] when the
    /// width or the height is zero, so the program skips resizing and
    /// drawing while the element has no area, and of kind
    /// [`ErrorKind::Slango`] when `slango` cannot create the texture, such
    /// as when the size exceeds the largest texture that the device allows.
    /// The view keeps its current texture on every error.
    pub fn resize(&mut self, size: [u32; 2]) -> Result<bool> {
        const RESIZE: &str = "resize a view";
        check_size(size, RESIZE)?;
        if size == self.size() {
            return Ok(false);
        }
        self.texture = create_texture(&self.device, size, RESIZE)?;
        Ok(true)
    }

    /// Returns the size of the view's texture as `[width, height]` in
    /// physical pixels.
    ///
    /// # Examples
    ///
    /// See the example of [`View::resize`].
    #[must_use]
    pub fn size(&self) -> [u32; 2] {
        self.texture.size()
    }

    /// Returns the view's current texture, such as to read its pixels back
    /// with [`Texture::read`] or to bind it to another draw.
    ///
    /// [`View::resize`] replaces the texture, so a texture that this method
    /// returned before a resize that returned `true` is no longer the
    /// view's.
    ///
    /// # Examples
    ///
    /// See the example of [`View`].
    #[must_use]
    pub fn texture(&self) -> &Texture {
        &self.texture
    }
}

fn check_size([width, height]: [u32; 2], operation: &'static str) -> Result<()> {
    if width == 0 || height == 0 {
        return Err(
            Error::new(ErrorKind::InvalidSize, operation).with_detail(format!(
                "the size is {width} by {height} pixels, which has no area"
            )),
        );
    }
    Ok(())
}

fn create_texture(device: &Device, size: [u32; 2], operation: &'static str) -> Result<Texture> {
    Texture::new(device, size, TextureFormat::Rgba8Unorm)
        .map_err(|error| Error::new(ErrorKind::Slango, operation).with_source(error))
}
