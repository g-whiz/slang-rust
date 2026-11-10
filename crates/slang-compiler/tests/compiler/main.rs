//! The integration tests of `slang-compiler`, which use only its public API
//! and, for the test of an assumption of its design, the raw bindings of
//! `slang-compiler-sys`.

#[expect(
    unsafe_code,
    reason = "the test of CA-14 calls Slang through the raw bindings, which only unsafe code can do"
)]
mod ffi;
