//! The integration tests of `slang-compiler-sys`, which use only its public
//! API, and the tests of its build script.

mod build_script;
#[expect(
    unsafe_code,
    reason = "the tests of this module call Slang through the raw bindings, which only unsafe code can do"
)]
mod ffi;
mod pin;
