//! The scene of the example `shader-playground`, which draws into a view of
//! `slang-slint` and needs no window, so that the tests of the examples draw
//! it offscreen with the same code as the window.

use std::error::Error;
use std::path::Path;

use slang_slint::View;
use slango::{Device, RenderPipeline, RenderPipelineOptions};

use crate::watched::Watched;

/// The values that the scene sets by name on the shader in each draw.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Uniforms {
    /// The time in seconds, which shifts the colours.
    pub(crate) time: f32,
    /// The factor of every colour channel, from the first slider.
    pub(crate) brightness: f32,
    /// The scale of the pattern, from the second slider.
    pub(crate) scale: f32,
}

/// A full-screen triangle drawn into a [`View`] with the render pipeline of
/// a Slang file, which the scene loads again when the file changes.
#[derive(Debug)]
pub(crate) struct Scene {
    device: Device,
    file: Watched,
    view: View,
}

impl Scene {
    /// Loads the Slang file at `path`, creates its render pipeline from the
    /// entry points `vsMain` and `fsMain`, and creates a view of `size`
    /// physical pixels that draws with it.
    pub(crate) fn new(
        device: &Device,
        path: &Path,
        size: [u32; 2],
    ) -> Result<Self, Box<dyn Error>> {
        let file = Watched::new(path);
        let view = View::new(device.clone(), load(device, path)?, size)?;
        Ok(Self {
            device: device.clone(),
            file,
            view,
        })
    }

    /// Loads the Slang file again when its modification time differs from
    /// the one of the last load, and returns whether it replaced the view.
    ///
    /// The new view has the size of the old one, and the program sets its
    /// image on the `Image` element. On an error, the scene keeps its view.
    /// `Watched::changed` says when the scene tries again.
    pub(crate) fn reload_if_changed(&mut self) -> Result<bool, Box<dyn Error>> {
        if !self.file.changed() {
            return Ok(false);
        }
        let pipeline = load(&self.device, self.file.path())?;
        self.view = View::new(self.device.clone(), pipeline, self.view.size())?;
        Ok(true)
    }

    /// Resizes the view to `size` physical pixels, and returns whether it
    /// replaced the view's texture, as [`View::resize`] does.
    pub(crate) fn resize(&mut self, size: [u32; 2]) -> slang_slint::Result<bool> {
        self.view.resize(size)
    }

    /// Draws the full-screen triangle into the view with `uniforms` set by
    /// name.
    pub(crate) fn draw(&self, uniforms: Uniforms) -> slango::Result<()> {
        self.view
            .draw()
            .set("time", uniforms.time)
            .set("brightness", uniforms.brightness)
            .set("scale", uniforms.scale)
            .submit(3, 1)
    }

    /// Returns the view that the scene draws into.
    pub(crate) fn view(&self) -> &View {
        &self.view
    }
}

fn load(device: &Device, path: &Path) -> slango::Result<RenderPipeline> {
    device
        .load_module(path)?
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
}
