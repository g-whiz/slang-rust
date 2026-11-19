//! The scene of the example `shader-playground`, which draws into a view of
//! `slang-slint` with the backend `wgpu`, and into a texture of its own with
//! the backend `rhi`, which `slang-slint` does not support, and needs no
//! window.

use std::error::Error;
use std::path::Path;

#[cfg(feature = "wgpu")]
use slang_slint::View;
use slango::{Buffer, Device, RenderPipeline, RenderPipelineOptions};
#[cfg(feature = "rhi")]
use slango::{Texture, TextureFormat};

use crate::watched::Watched;

/// The corners of the triangle that covers the view, in the coordinates of
/// the view from 0 to 2, which `playground.slang` maps to clip space.
const CORNERS: [[f32; 2]; 3] = [[0.0, 0.0], [2.0, 0.0], [0.0, 2.0]];

/// The values that the scene sets by name on the shader in each draw.
#[derive(Clone, Copy)]
pub(crate) struct Uniforms {
    /// The time in seconds, which shifts the colours.
    pub(crate) time: f32,
    /// The factor of every colour channel, from the first slider.
    pub(crate) brightness: f32,
    /// The scale of the pattern, from the second slider.
    pub(crate) scale: f32,
}

/// A full-screen triangle drawn with the render pipeline of a Slang file,
/// which the scene loads again when the file changes.
pub(crate) struct Scene {
    device: Device,
    file: Watched,
    corners: Buffer<[f32; 2]>,
    #[cfg(feature = "wgpu")]
    view: View,
    #[cfg(feature = "rhi")]
    pipeline: RenderPipeline,
    #[cfg(feature = "rhi")]
    texture: Texture,
}

impl Scene {
    /// Loads the Slang file at `path`, creates its render pipeline from the
    /// entry points `vsMain` and `fsMain`, and creates a view, or a texture,
    /// of `size` physical pixels that it draws into.
    pub(crate) fn new(
        device: &Device,
        path: &Path,
        size: [u32; 2],
    ) -> Result<Self, Box<dyn Error>> {
        let file = Watched::new(path);
        let pipeline = load(device, path)?;
        let corners = Buffer::from_slice(device, &CORNERS)?;
        #[cfg(feature = "wgpu")]
        let view = View::new(device.clone(), pipeline, size)?;
        #[cfg(feature = "rhi")]
        let texture = Texture::new(device, size, TextureFormat::Rgba8Unorm)?;
        Ok(Self {
            device: device.clone(),
            file,
            corners,
            #[cfg(feature = "wgpu")]
            view,
            #[cfg(feature = "rhi")]
            pipeline,
            #[cfg(feature = "rhi")]
            texture,
        })
    }

    /// Loads the Slang file again when its modification time differs from
    /// the one of the last load, and returns whether it replaced the view,
    /// or the pipeline and the texture.
    ///
    /// The new view or texture has the size of the old one, and the program
    /// sets the view's image on the `Image` element. On an error, the scene
    /// keeps its view. `Watched::changed` says when the scene tries again.
    pub(crate) fn reload_if_changed(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.file.changed() {
            return Ok(false);
        }
        let pipeline = load(&self.device, self.file.path())?;
        #[cfg(feature = "wgpu")]
        {
            self.view = View::new(self.device.clone(), pipeline, self.view.size())?;
        }
        #[cfg(feature = "rhi")]
        {
            let texture = Texture::new(&self.device, self.texture.size(), self.texture.format())?;
            self.pipeline = pipeline;
            self.texture = texture;
        }
        Ok(true)
    }

    /// Resizes the view to `size` physical pixels, and returns whether it
    /// replaced the view's texture, as `View::resize` does.
    #[cfg(feature = "wgpu")]
    pub(crate) fn resize(&mut self, size: [u32; 2]) -> slang_slint::Result<bool> {
        self.view.resize(size)
    }

    /// Replaces the texture with one of `size` physical pixels when the size
    /// differs, and returns whether it did.
    #[cfg(feature = "rhi")]
    pub(crate) fn resize(&mut self, size: [u32; 2]) -> slango::Result<bool> {
        if size == self.texture.size() {
            return Ok(false);
        }
        self.texture = Texture::new(&self.device, size, self.texture.format())?;
        Ok(true)
    }

    /// Draws the full-screen triangle into the view, or the texture, with
    /// `uniforms` set by name.
    pub(crate) fn draw(&self, uniforms: Uniforms) -> slango::Result<()> {
        #[cfg(feature = "wgpu")]
        let mut draw = self.view.draw();
        #[cfg(feature = "rhi")]
        let mut draw = self.pipeline.draw();
        #[cfg(feature = "rhi")]
        draw.target(&self.texture);
        draw.set("time", uniforms.time)
            .set("brightness", uniforms.brightness)
            .set("scale", uniforms.scale)
            .vertex("corner", &self.corners)
            .submit(3, 1)
    }

    /// Returns the view that the scene draws into.
    #[cfg(feature = "wgpu")]
    pub(crate) fn view(&self) -> &View {
        &self.view
    }

    /// Returns the texture that the scene draws into.
    #[cfg(feature = "rhi")]
    pub(crate) fn texture(&self) -> &Texture {
        &self.texture
    }
}

fn load(device: &Device, path: &Path) -> slango::Result<RenderPipeline> {
    device
        .load_module(path)?
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
}
