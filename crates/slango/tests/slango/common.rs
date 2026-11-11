use std::path::PathBuf;

use slango::{AdapterSelection, Device, DeviceOptions, Kernel, Module};

pub(crate) fn device_with(adapter: AdapterSelection) -> Device {
    let mut options = DeviceOptions::default();
    options.adapter = adapter;
    let device = Device::new(&options).expect("a device on the selected adapter");
    let wgpu_device = slango::wgpu::device(&device);
    println!(
        "slango gpu test adapter: {:?}, immediates: {}, max immediate size: {}",
        wgpu_device.adapter_info(),
        wgpu_device.features().contains(wgpu::Features::IMMEDIATES),
        wgpu_device.limits().max_immediate_size
    );
    device
}

pub(crate) fn instance() -> wgpu::Instance {
    wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::PRIMARY,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    })
}

pub(crate) fn assert_immediates_follow_the_adapter(device: &Device) {
    let wgpu_device = slango::wgpu::device(device);
    let info = wgpu_device.adapter_info();
    let adapter = pollster::block_on(instance().enumerate_adapters(wgpu::Backends::PRIMARY))
        .into_iter()
        .find(|adapter| adapter.get_info() == info)
        .expect("the adapter of the device among the adapters of a new instance");
    let offered = adapter.features().contains(wgpu::Features::IMMEDIATES);
    assert_eq!(
        wgpu_device.features().contains(wgpu::Features::IMMEDIATES),
        offered
    );
    let expected_size = if offered {
        adapter.limits().max_immediate_size
    } else {
        0
    };
    assert_eq!(wgpu_device.limits().max_immediate_size, expected_size);
}

pub(crate) fn device_without_immediates() -> Device {
    let adapter =
        pollster::block_on(instance().request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("an adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device without optional features");
    assert!(!device.features().contains(wgpu::Features::IMMEDIATES));
    slango::wgpu::from_device_and_queue(device, queue)
}

pub(crate) fn load(device: &Device, source: &str) -> Module {
    device
        .load_module_from_source("kernels", source)
        .expect("the module loads")
}

pub(crate) fn shader(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("slango")
        .join("shaders")
        .join(name)
}

pub(crate) fn kernel(device: &Device, source: &str, name: &str) -> Kernel {
    load(device, source)
        .kernel(name)
        .expect("the kernel exists")
        .clone()
}
