//! The operations that the backend rhi does not offer, which fail with
//! [`ErrorKind::Unsupported`] after the checks of their arguments.

use slango::{
    AdapterSelection, Device, ErrorKind, RenderPipelineOptions, Sampler, SamplerOptions, Texture,
    TextureFormat,
};

use crate::common::{device_with, shader};

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

fn assert_unsupported<T>(result: slango::Result<T>, what: &str) {
    assert_eq!(
        result.err().map(|error| error.kind()),
        Some(ErrorKind::Unsupported),
        "{what}"
    );
}

#[test]
fn a_texture_is_unsupported() {
    let device = device();
    assert_unsupported(
        Texture::new(&device, [4, 4], TextureFormat::Rgba8Unorm),
        "Texture::new",
    );
    assert_unsupported(
        Texture::from_pixels(&device, [2, 1], &[[255, 0, 0, 255], [0, 255, 0, 255]]),
        "Texture::from_pixels",
    );
}

#[test]
fn a_sampler_is_unsupported() {
    assert_unsupported(
        Sampler::new(&device(), &SamplerOptions::default()),
        "Sampler::new",
    );
}

#[test]
fn a_render_pipeline_of_existing_entry_points_is_unsupported() -> slango::Result<()> {
    let module = device().load_module(shader("quad.slang"))?;
    assert_unsupported(
        module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default()),
        "Module::render_pipeline",
    );
    Ok(())
}
