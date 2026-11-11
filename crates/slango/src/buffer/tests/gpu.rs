use crate::{Buffer, Device, DeviceOptions};

#[test]
fn is_on_tells_the_devices_of_two_buffers_apart() {
    let first = Device::new(&DeviceOptions::default()).expect("a first device");
    let second = Device::new(&DeviceOptions::default()).expect("a second device");
    let on_first = Buffer::from_slice(&first, &[1_u32, 2]).expect("a buffer on the first device");
    let on_second = Buffer::<u32>::zeroed(&second, 2).expect("a buffer on the second device");
    assert!(on_first.is_on(&first));
    assert!(on_first.is_on(&first.clone()));
    assert!(on_first.clone().is_on(&first));
    assert!(!on_first.is_on(&second));
    assert!(on_second.is_on(&second));
    assert!(!on_second.is_on(&first));
    let rewrapped = crate::wgpu::from_device_and_queue(
        crate::wgpu::device(&first).clone(),
        crate::wgpu::queue(&first).clone(),
    );
    assert!(!on_first.is_on(&rewrapped));
}
