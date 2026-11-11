use slango::{AdapterSelection, Device, DeviceOptions};

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
