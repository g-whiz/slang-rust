use slango::{AdapterSelection, Device, DeviceOptions, ErrorKind};

use crate::common::{assert_immediates_follow_the_adapter, device_with, instance};

mod buffer;
mod call;
mod load;

#[test]
fn default_device_reports_its_adapter() {
    let device = device_with(AdapterSelection::Default);
    let name = device.adapter_name();
    assert_ne!(name, "");
    assert_eq!(name, slango::wgpu::device(&device).adapter_info().name);
    assert_eq!(device.clone().adapter_name(), name);
}

#[test]
fn default_device_has_immediates_when_its_adapter_offers_them() {
    assert_immediates_follow_the_adapter(&device_with(AdapterSelection::Default));
}

#[cfg(not(target_os = "macos"))]
#[test]
fn software_selection_creates_a_device_on_a_software_adapter() {
    let device = device_with(AdapterSelection::Software);
    let info = slango::wgpu::device(&device).adapter_info();
    assert_eq!(info.device_type, wgpu::DeviceType::Cpu);
    assert_immediates_follow_the_adapter(&device);
}

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

#[test]
fn device_from_wgpu_objects_uses_them() {
    let adapter =
        pollster::block_on(instance().request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("an adapter");
    let (wgpu_device, wgpu_queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device without optional features");
    let device = slango::wgpu::from_device_and_queue(wgpu_device.clone(), wgpu_queue.clone());
    assert_eq!(slango::wgpu::device(&device), &wgpu_device);
    assert_eq!(slango::wgpu::queue(&device), &wgpu_queue);
    assert_eq!(device.adapter_name(), adapter.get_info().name);
    assert!(
        !slango::wgpu::device(&device)
            .features()
            .contains(wgpu::Features::IMMEDIATES)
    );
}
