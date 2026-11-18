use super::{AddressMode, FilterMode, Sampler, SamplerOptions};

#[test]
fn default_options_read_the_nearest_pixel_and_clamp_to_the_edges() {
    let options = SamplerOptions::default();
    assert_eq!(options.filter, FilterMode::Nearest);
    assert_eq!(options.address_mode, AddressMode::ClampToEdge);
}

fn assert_clone<T: Clone + 'static>() {}

fn assert_send_sync_clone<T: Send + Sync + Clone + 'static>() {}

#[test]
fn samplers_are_clone_and_their_options_are_send_sync_and_clone() {
    assert_clone::<Sampler>();
    assert_send_sync_clone::<SamplerOptions>();
    assert_send_sync_clone::<FilterMode>();
    assert_send_sync_clone::<AddressMode>();
}
