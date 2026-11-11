//! The integration tests of `slang-compiler`, which use its public API and,
//! for the tests of the assumptions of its design and for what the crate does
//! not wrap yet, the raw bindings of `slang-compiler-sys`.

#[expect(
    unsafe_code,
    reason = "the tests of CA-1 and CA-3, and the lookup of types for specialization, call Slang through the raw bindings, which only unsafe code can do"
)]
mod ffi;
