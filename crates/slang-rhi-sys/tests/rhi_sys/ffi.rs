use std::ffi::CStr;
use std::hint::black_box;

use slang_rhi_sys::{DeviceType, IRHI, rhiGetInstance};

use crate::symbols::{C_FUNCTIONS, VTABLE_SLOTS, slang_rhi_sys_abi_probe_slots};

#[test]
fn every_c_function_of_the_bindings_resolves_to_a_native_symbol() {
    assert_ne!(C_FUNCTIONS, []);
    for (name, address) in C_FUNCTIONS {
        assert!(!black_box(address).is_null(), "{name}");
    }
}

#[test]
fn every_vtable_slot_of_the_bindings_has_the_offset_that_the_compiler_of_slang_rhi_gives_it() {
    assert_ne!(VTABLE_SLOTS, []);
    let mut offsets = vec![0_usize; VTABLE_SLOTS.len()];
    // SAFETY: `offsets` is a live buffer of `offsets.len()` elements, and the
    // probe writes at most `capacity` elements into it and reads nothing else.
    let count = unsafe { slang_rhi_sys_abi_probe_slots(offsets.as_mut_ptr(), offsets.len()) };
    assert_eq!(count, VTABLE_SLOTS.len());
    let wrong: Vec<String> = VTABLE_SLOTS
        .iter()
        .zip(&offsets)
        .filter(|((_, rust), probed)| rust != *probed)
        .map(|((name, rust), probed)| format!("{name}: Rust {rust}, C++ {probed}"))
        .collect();
    assert_eq!(wrong, Vec::<String>::new());
}

#[test]
#[expect(
    ffi_unwind_calls,
    reason = "the smoke test calls the C function of slang-rhi directly, and an exception that escaped it would unwind into the test, which is defined and fails it"
)]
fn the_instance_names_the_vulkan_device_type_without_a_device() {
    // SAFETY: `rhiGetInstance` takes no argument and returns the process-wide
    // instance, which it creates once under a mutex.
    let rhi: *mut IRHI = unsafe { rhiGetInstance() };
    assert!(!rhi.is_null());
    // SAFETY: `rhi` is the live process-wide instance, which slang-rhi never
    // frees while no test calls `rhiDestroyInstance`, and the call reads only
    // its argument.
    let name = unsafe { IRHI::getDeviceTypeName(rhi, DeviceType::Vulkan) };
    assert!(!name.is_null());
    // SAFETY: `name` is non-null and points to a null-terminated string
    // literal of slang-rhi, which lives as long as the program.
    let name = unsafe { CStr::from_ptr(name) };
    assert_eq!(name.to_str(), Ok("Vulkan"));
}
