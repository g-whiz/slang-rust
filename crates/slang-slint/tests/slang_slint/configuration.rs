use slint::wgpu_30::wgpu::{Features, Limits};
use slint::wgpu_30::{WGPUConfiguration, WGPUSettings};

fn automatic_settings() -> Option<WGPUSettings> {
    match slang_slint::wgpu_configuration() {
        WGPUConfiguration::Automatic(settings) => Some(settings),
        _ => None,
    }
}

#[test]
fn the_device_requires_the_immediates_feature_alone() {
    assert_eq!(
        automatic_settings()
            .expect("Slint creates the device")
            .device_required_features,
        Features::IMMEDIATES
    );
}

#[test]
fn the_device_requires_128_bytes_of_immediates_and_wgpu_default_limits() {
    let expected = Limits {
        max_immediate_size: 128,
        ..Limits::default()
    };
    assert_eq!(
        automatic_settings()
            .expect("Slint creates the device")
            .device_required_limits,
        expected
    );
}

#[test]
fn every_other_setting_keeps_the_default_of_slint() {
    let mut expected = WGPUSettings::default();
    expected.device_required_features = Features::IMMEDIATES;
    expected.device_required_limits = Limits {
        max_immediate_size: 128,
        ..Limits::default()
    };
    assert_eq!(
        format!(
            "{:?}",
            automatic_settings().expect("Slint creates the device")
        ),
        format!("{expected:?}")
    );
}
