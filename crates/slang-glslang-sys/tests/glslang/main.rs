//! The integration tests of `slang-glslang-sys`: the tests of its build
//! script, and the tests of the loader that the crate links.

mod build_script;
#[expect(
    unsafe_code,
    reason = "the tests call the loader of the C++ shim and the libraries that it returns through the raw bindings, which only unsafe code can do"
)]
mod ffi;
