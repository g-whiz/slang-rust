//! The integration tests of `slang-compiler`, which use its public API and,
//! for the tests of what the crate assumes about Slang and of what the crate
//! does not wrap, the raw bindings of `slang-compiler-sys`.

#[expect(
    unsafe_code,
    reason = "the tests of CA-1 and CA-3, and the lookup of types for specialization, call Slang through the raw bindings, which only unsafe code can do"
)]
mod ffi;
