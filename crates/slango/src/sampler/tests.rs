use super::{AddressMode, FilterMode, Sampler, SamplerOptions, descriptor};

#[test]
fn default_options_read_the_nearest_pixel_and_clamp_to_the_edges() {
    let options = SamplerOptions::default();
    assert_eq!(options.filter, FilterMode::Nearest);
    assert_eq!(options.address_mode, AddressMode::ClampToEdge);
}

#[test]
fn the_filter_applies_to_magnification_and_minification() {
    for (filter, expected) in [
        (FilterMode::Nearest, wgpu::FilterMode::Nearest),
        (FilterMode::Linear, wgpu::FilterMode::Linear),
    ] {
        let descriptor = descriptor(&SamplerOptions {
            filter,
            ..SamplerOptions::default()
        });
        assert_eq!(descriptor.mag_filter, expected);
        assert_eq!(descriptor.min_filter, expected);
    }
}

#[test]
fn the_address_mode_applies_to_every_axis() {
    for (mode, expected) in [
        (AddressMode::ClampToEdge, wgpu::AddressMode::ClampToEdge),
        (AddressMode::Repeat, wgpu::AddressMode::Repeat),
        (AddressMode::MirrorRepeat, wgpu::AddressMode::MirrorRepeat),
    ] {
        let descriptor = descriptor(&SamplerOptions {
            address_mode: mode,
            ..SamplerOptions::default()
        });
        assert_eq!(descriptor.address_mode_u, expected);
        assert_eq!(descriptor.address_mode_v, expected);
        assert_eq!(descriptor.address_mode_w, expected);
    }
}

#[test]
fn a_sampler_is_neither_a_comparison_nor_an_anisotropic_sampler() {
    let descriptor = descriptor(&SamplerOptions::default());
    assert_eq!(descriptor.compare, None);
    assert_eq!(descriptor.anisotropy_clamp, 1);
    assert_eq!(descriptor.border_color, None);
}

fn assert_send_sync_clone<T: Send + Sync + Clone + 'static>() {}

#[test]
fn samplers_and_their_options_are_send_sync_and_clone() {
    assert_send_sync_clone::<Sampler>();
    assert_send_sync_clone::<SamplerOptions>();
    assert_send_sync_clone::<FilterMode>();
    assert_send_sync_clone::<AddressMode>();
}
