//! The integration tests of `slang-rhi-sys`: the tests of its build script,
//! of the symbols, the vtable layout, and the defaults of its bindings, and of
//! a device.

mod build_script;
mod defaults;
#[expect(
    unsafe_code,
    reason = "the tests of this module call slang-rhi and the ABI probe through the raw bindings, which only unsafe code can do"
)]
mod ffi;
#[expect(
    unsafe_code,
    reason = "the tests of this module open a device through the raw bindings and implement a debug callback, which only unsafe code can do"
)]
mod gpu;
#[expect(
    unsafe_code,
    reason = "the generated list declares the C function of the ABI probe, which only an unsafe extern block can do"
)]
mod symbols {
    include!(concat!(env!("OUT_DIR"), "/symbols.rs"));
}
