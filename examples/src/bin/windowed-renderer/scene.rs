//! The scene of the example `windowed-renderer`, which draws into any
//! [`Texture`] and needs no window, so that the tests of the examples draw it
//! offscreen with the same code as the window.

use std::path::Path;

use slango::{
    Buffer, Device, RenderPipeline, RenderPipelineOptions, Result, Sampler, SamplerOptions,
    Texture, TextureFormat,
};

use crate::watched::Watched;

const CLEAR: [f32; 4] = [0.05, 0.05, 0.08, 1.0];

/// A quad with a checkerboard texture, drawn with the render pipeline of a
/// Slang file that the scene loads again when the file changes.
#[derive(Debug)]
pub(crate) struct Scene {
    device: Device,
    file: Watched,
    options: RenderPipelineOptions,
    pipeline: RenderPipeline,
    positions: Buffer<[f32; 2]>,
    uvs: Buffer<[f32; 2]>,
    indices: Buffer<u32>,
    image: Texture,
    sampler: Sampler,
}

impl Scene {
    /// Loads the Slang file at `path`, creates its render pipeline from the
    /// entry points `vsMain` and `fsMain` for targets of `colour_format`,
    /// and creates the quad and its texture.
    pub(crate) fn new(device: &Device, path: &Path, colour_format: TextureFormat) -> Result<Self> {
        let mut options = RenderPipelineOptions::default();
        options.colour_format = colour_format;
        let file = Watched::new(path);
        let pipeline = load(device, path, &options)?;
        let corners = [[-1.0_f32, -1.0], [1.0, -1.0], [1.0, 1.0], [-1.0, 1.0]];
        let uvs = [[0.0_f32, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
        Ok(Self {
            device: device.clone(),
            file,
            options,
            pipeline,
            positions: Buffer::from_slice(device, &corners)?,
            uvs: Buffer::from_slice(device, &uvs)?,
            indices: Buffer::from_slice(device, &[0_u32, 1, 2, 0, 2, 3])?,
            image: Texture::from_pixels(device, [8, 8], &checkerboard())?,
            sampler: Sampler::new(device, &SamplerOptions::default())?,
        })
    }

    /// Loads the Slang file again when its modification time differs from
    /// the one of the last load, and returns whether it replaced the
    /// pipeline.
    ///
    /// On an error, the scene keeps its pipeline. `Watched::changed` says
    /// when the scene tries again.
    pub(crate) fn reload_if_changed(&mut self) -> Result<bool> {
        if !self.file.changed() {
            return Ok(false);
        }
        self.pipeline = load(&self.device, self.file.path(), &self.options)?;
        Ok(true)
    }

    /// Clears `target` and draws the quad into it, rotated by `angle` in
    /// radians and scaled to keep its square shape at the aspect ratio of
    /// the target.
    pub(crate) fn draw(&self, target: &Texture, angle: f32) -> Result<()> {
        let [width, height] = target.size().map(f64::from);
        #[expect(
            clippy::cast_possible_truncation,
            reason = "the aspect ratio of a texture size fits in an f32"
        )]
        let aspect = (width / height) as f32;
        let (x_scale, y_scale) = if aspect >= 1.0 {
            (0.7 / aspect, 0.7)
        } else {
            (0.7, 0.7 * aspect)
        };
        let (sin, cos) = angle.sin_cos();
        let transform = [
            [x_scale * cos, -x_scale * sin, 0.0, 0.0],
            [y_scale * sin, y_scale * cos, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        self.pipeline
            .draw()
            .set("transform", transform)
            .texture("image", &self.image)
            .sampler("imageSampler", &self.sampler)
            .vertex("input.position", &self.positions)
            .vertex("input.uv", &self.uvs)
            .target(target)
            .clear(CLEAR)
            .submit_indexed(&self.indices, 6, 1)
    }
}

fn load(device: &Device, path: &Path, options: &RenderPipelineOptions) -> Result<RenderPipeline> {
    device
        .load_module(path)?
        .render_pipeline("vsMain", "fsMain", options)
}

fn checkerboard() -> Vec<[u8; 4]> {
    (0..8_u32)
        .flat_map(|y| (0..8_u32).map(move |x| (x + y) % 2 == 0))
        .map(|light| {
            if light {
                [240, 240, 230, 255]
            } else {
                [230, 120, 30, 255]
            }
        })
        .collect()
}
