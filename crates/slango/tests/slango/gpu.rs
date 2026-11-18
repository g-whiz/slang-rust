use slango::{AdapterSelection, Device, DeviceOptions, ErrorKind};

use crate::common::device_with;
#[cfg(feature = "wgpu")]
use crate::common::{assert_immediates_follow_the_adapter, instance};

mod buffer;
mod call;
mod load;
mod precompiled;
mod render;
mod sample;
mod texture;
#[cfg(feature = "rhi")]
mod unsupported;

#[test]
fn default_device_reports_its_adapter() {
    let device = device_with(AdapterSelection::Default);
    let name = device.adapter_name();
    assert_ne!(name, "");
    #[cfg(feature = "wgpu")]
    assert_eq!(
        name,
        slango::wgpu::device(&device)
            .expect("the wgpu device of a device")
            .adapter_info()
            .name
    );
    assert_eq!(device.clone().adapter_name(), name);
}

#[cfg(feature = "wgpu")]
#[test]
fn default_device_has_immediates_when_its_adapter_offers_them() {
    assert_immediates_follow_the_adapter(&device_with(AdapterSelection::Default));
}

#[cfg(all(feature = "wgpu", not(target_os = "macos")))]
#[test]
fn software_selection_creates_a_device_on_a_software_adapter() {
    let device = device_with(AdapterSelection::Software);
    let info = slango::wgpu::device(&device)
        .expect("the wgpu device of a device")
        .adapter_info();
    assert_eq!(info.device_type, wgpu::DeviceType::Cpu);
    assert_immediates_follow_the_adapter(&device);
}

#[cfg(feature = "wgpu")]
#[test]
fn name_selection_creates_a_device_on_the_named_adapter() {
    let default_name = device_with(AdapterSelection::Default).adapter_name();
    let names: Vec<String> =
        pollster::block_on(instance().enumerate_adapters(wgpu::Backends::PRIMARY))
            .iter()
            .map(|adapter| adapter.get_info().name)
            .collect();
    let first_containing = |text: &str| {
        names
            .iter()
            .find(|name| name.contains(text))
            .cloned()
            .expect("an adapter whose name contains the text")
    };
    let name = names
        .iter()
        .find(|name| first_containing(name) != default_name)
        .unwrap_or(&default_name)
        .clone();
    println!("slango gpu test selects by name: {name}, default adapter: {default_name}");
    let device = device_with(AdapterSelection::Name(name.clone()));
    assert_eq!(device.adapter_name(), first_containing(&name));
}

#[test]
fn name_selection_without_a_matching_adapter_is_no_adapter() {
    let mut options = DeviceOptions::default();
    options.adapter = AdapterSelection::Name("no adapter has this name".to_owned());
    let error = Device::new(&options).expect_err("no adapter has the name");
    assert_eq!(error.kind(), ErrorKind::NoAdapter);
    assert_eq!(
        error.to_string(),
        "cannot find a GPU adapter: no adapter has a name that contains `no adapter has this name`"
    );
}

#[cfg(feature = "rhi")]
#[expect(
    clippy::expect_used,
    reason = "the helper fails the test when slang-rhi cannot list the adapters"
)]
fn rhi_adapters() -> Vec<slang_rhi::Adapter> {
    let device_type = if cfg!(target_os = "windows") {
        slang_rhi::DeviceType::D3d12
    } else {
        slang_rhi::DeviceType::Vulkan
    };
    slang_rhi::adapters(device_type).expect("the adapters of slang-rhi")
}

#[cfg(feature = "rhi")]
#[test]
fn software_selection_creates_a_device_on_a_software_adapter() {
    let software: Vec<String> = rhi_adapters()
        .iter()
        .filter(|adapter| adapter.adapter_type() == slang_rhi::AdapterType::Software)
        .map(|adapter| adapter.name().to_owned())
        .collect();
    let device = device_with(AdapterSelection::Software);
    assert_eq!(Some(&device.adapter_name()), software.first());
}

#[cfg(feature = "rhi")]
#[test]
fn name_selection_creates_a_device_on_the_named_adapter() {
    let names: Vec<String> = rhi_adapters()
        .iter()
        .map(|adapter| adapter.name().to_owned())
        .collect();
    let name = names.last().expect("an adapter").clone();
    let first_containing = names
        .iter()
        .find(|other| other.contains(name.as_str()))
        .expect("an adapter whose name contains the name")
        .clone();
    println!("slango gpu test selects by name: {name}");
    let device = device_with(AdapterSelection::Name(name));
    assert_eq!(device.adapter_name(), first_containing);
}

#[cfg(feature = "wgpu")]
#[test]
fn device_from_wgpu_objects_uses_them() {
    let adapter =
        pollster::block_on(instance().request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("an adapter");
    let (wgpu_device, wgpu_queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device without optional features");
    let device = slango::wgpu::from_device_and_queue(wgpu_device.clone(), wgpu_queue.clone());
    assert_eq!(slango::wgpu::device(&device), Some(&wgpu_device));
    assert_eq!(slango::wgpu::queue(&device), Some(&wgpu_queue));
    assert_eq!(device.adapter_name(), adapter.get_info().name);
    assert!(
        !slango::wgpu::device(&device)
            .expect("the wgpu device of a device")
            .features()
            .contains(wgpu::Features::IMMEDIATES)
    );
}
