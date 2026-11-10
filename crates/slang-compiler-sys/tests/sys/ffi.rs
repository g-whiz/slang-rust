use std::ffi::{CStr, c_void};
use std::hint::black_box;
use std::ptr;

use slang_compiler_sys::{
    IGlobalSession, ISlangUnknown, Interface, SLANG_BUILD_TAG, SlangGlobalSessionDesc, SlangResult,
    slang_createGlobalSession2, spGetBuildTagString,
};

use crate::symbols::C_FUNCTIONS;

#[test]
#[expect(
    ffi_unwind_calls,
    reason = "the smoke test calls the C functions of Slang directly, and an exception that escaped one would unwind into the test, which is defined and fails it"
)]
fn global_session_reports_the_pinned_build_tag_and_releases_every_reference() {
    let desc = SlangGlobalSessionDesc::default();
    let mut session: *mut IGlobalSession = ptr::null_mut();
    // SAFETY: `desc` is a live descriptor whose size field holds its own size,
    // which `Default` sets, and `session` is a live out-pointer, which is all
    // that `slang_createGlobalSession2` reads and writes.
    let result = unsafe { slang_createGlobalSession2(&raw const desc, &raw mut session) };
    assert_eq!(result, SlangResult::SLANG_OK);
    assert!(!session.is_null());

    // SAFETY: `spGetBuildTagString` takes no argument, and Slang documents it
    // as thread-safe.
    let tag = unsafe { spGetBuildTagString() };
    assert!(!tag.is_null());
    // SAFETY: `tag` is non-null and points to the null-terminated build tag,
    // which Slang keeps for as long as the library stays loaded, and the test
    // binary never unloads it.
    let tag = unsafe { CStr::from_ptr(tag) };
    assert_eq!(tag.to_str(), Ok(SLANG_BUILD_TAG));

    // SAFETY: `session` is the live global session that
    // `slang_createGlobalSession2` returned above, and only this thread uses it.
    let session_tag = unsafe { IGlobalSession::getBuildTagString(session) };
    assert!(!session_tag.is_null());
    // SAFETY: `session_tag` is non-null and points to the null-terminated
    // build tag, which Slang keeps for as long as the library stays loaded.
    let session_tag = unsafe { CStr::from_ptr(session_tag) };
    assert_eq!(session_tag.to_str(), Ok(SLANG_BUILD_TAG));

    let uuid = ISlangUnknown::UUID;
    let mut unknown: *mut c_void = ptr::null_mut();
    // SAFETY: `session` is the live global session, `uuid` is a live copy of
    // the UUID of `ISlangUnknown`, which every Slang object implements, and
    // `unknown` is a live out-pointer that receives the interface pointer with
    // one added reference.
    let result =
        unsafe { IGlobalSession::queryInterface(session, &raw const uuid, &raw mut unknown) };
    assert_eq!(result, SlangResult::SLANG_OK);
    assert_eq!(unknown, session.cast::<c_void>());

    // SAFETY: `unknown` is the interface pointer of `ISlangUnknown` that
    // `queryInterface` returned with one added reference, and this releases
    // exactly that reference.
    let count = unsafe { ISlangUnknown::release(unknown.cast::<ISlangUnknown>()) };
    assert_eq!(count, 1);
    // SAFETY: `session` holds the one reference that
    // `slang_createGlobalSession2` returned, no object was created from it,
    // and the test does not use `session` after this release.
    let count = unsafe { IGlobalSession::release(session) };
    assert_eq!(count, 0);
}

#[test]
fn every_c_function_of_the_bindings_resolves_to_a_native_symbol() {
    assert_ne!(C_FUNCTIONS, []);
    for (name, address) in C_FUNCTIONS {
        assert!(!black_box(address).is_null(), "{name}");
    }
}
