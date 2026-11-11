//! The scene of the example `shader-playground`, which draws into a view of
//! `slang-slint` and needs no window, so that the tests of the examples draw
//! it offscreen with the same code as the window.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use slang_slint::View;
use slango::{Device, RenderPipeline, RenderPipelineOptions};

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
    path: PathBuf,
    modified: Option<SystemTime>,
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
        let modified = modified(path);
        let view = View::new(device.clone(), load(device, path)?, size)?;
        Ok(Self {
            device: device.clone(),
            path: path.to_owned(),
            modified,
            view,
        })
    }

    /// Loads the Slang file again when its modification time differs from
    /// the one of the last load, and returns whether it replaced the view.
    ///
    /// The new view has the size of the old one, and the program sets its
    /// image on the `Image` element. On an error, the scene keeps its view
    /// and does not try again until the modification time changes once
    /// more. A file whose modification time cannot be read counts as
    /// unchanged, as it can be while an editor replaces it.
    pub(crate) fn reload_if_changed(&mut self) -> Result<bool, Box<dyn Error>> {
        let modified = modified(&self.path);
        if modified.is_none() || modified == self.modified {
            return Ok(false);
        }
        self.modified = modified;
        let pipeline = load(&self.device, &self.path)?;
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

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
}

fn load(device: &Device, path: &Path) -> slango::Result<RenderPipeline> {
    device
        .load_module(path)?
        .render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())
}
