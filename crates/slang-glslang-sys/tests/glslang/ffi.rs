use std::ffi::{CStr, CString, c_void};
use std::ptr::{self, NonNull};

use slang_compiler_sys::{
    BUILD_INFO, IGlobalSession, ISlangCastable, ISlangSharedLibrary, ISlangSharedLibraryLoader,
    ISlangUnknown, Interface, SlangResult, SlangUUID,
};
use slang_glslang_sys::slang_glslang_sys_loader;

const FUNCTIONS: [&CStr; 9] = [
    c"glslang_compile",
    c"glslang_compile_1_1",
    c"glslang_compile_1_2",
    c"glslang_compile_1_3",
    c"glslang_disassembleSPIRV",
    c"glslang_disassembleSPIRVWithResult",
    c"glslang_freeDisassembly",
    c"glslang_linkSPIRV",
    c"glslang_validateSPIRV",
];

/// Loads `name` through the loader of the crate, and returns the result code
/// and the library.
fn load(name: &str) -> (SlangResult, *mut ISlangSharedLibrary) {
    let name = CString::new(name).unwrap_or_default();
    let mut library: *mut ISlangSharedLibrary = ptr::null_mut();
    // SAFETY: the loader has no precondition, and the test uses it only with
    // the Slang that this test binary links.
    let loader = unsafe { slang_glslang_sys_loader() };
    assert!(!loader.is_null());
    // SAFETY: `loader` is the live static loader, `name` is a live string
    // terminated by a null byte, and `library` is a live out-pointer.
    let result = unsafe {
        ISlangSharedLibraryLoader::loadSharedLibrary(loader, name.as_ptr(), &raw mut library)
    };
    (result, library)
}

/// Returns the address of `symbol` in `library`.
fn find(library: *mut ISlangSharedLibrary, symbol: &CStr) -> *mut c_void {
    // SAFETY: `library` is a live library that the loader returned, and
    // `symbol` is a string terminated by a null byte.
    unsafe { ISlangSharedLibrary::findSymbolAddressByName(library, symbol.as_ptr()) }
}

fn numeric_version() -> &'static str {
    let tag = BUILD_INFO.slang_build_tag;
    tag.split_once('-').map_or(tag, |(numeric, _)| numeric)
}

#[test]
fn the_loader_answers_slang_glslang_with_the_nine_linked_functions() {
    let names = [
        format!("slang-glslang-{}", numeric_version()),
        format!("libslang-glslang-{}", numeric_version()),
        format!("some/directory/slang-glslang-{}", numeric_version()),
        format!("C:\\some\\directory\\slang-glslang-{}", numeric_version()),
        format!("C:\\some/mixed\\libslang-glslang-{}", numeric_version()),
        "slang-glslang-0.0.0".to_owned(),
    ];
    let (_, first) = load(&names[0]);
    for name in names {
        let (result, library) = load(&name);
        assert_eq!(result, SlangResult::SLANG_OK, "{name}");
        assert_eq!(library, first, "{name}");
        for function in FUNCTIONS {
            assert!(!find(library, function).is_null(), "{name}: {function:?}");
        }
        assert!(find(library, c"glslang_unknown").is_null(), "{name}");
        assert!(find(library, c"malloc").is_null(), "{name}");
        // SAFETY: `library` is the static library of the loader, whose
        // release frees nothing.
        unsafe { ISlangSharedLibrary::release(library) };
    }
    let (_, again) = load(&format!("slang-glslang-{}", numeric_version()));
    assert!(!find(again, FUNCTIONS[0]).is_null());
}

#[cfg(target_os = "linux")]
const PLATFORM_LIBRARY: (&str, &CStr) = ("libc.so.6", c"malloc");
#[cfg(target_os = "macos")]
const PLATFORM_LIBRARY: (&str, &CStr) = ("System", c"malloc");
#[cfg(windows)]
const PLATFORM_LIBRARY: (&str, &CStr) = ("kernel32", c"GetProcAddress");

#[test]
fn the_loader_passes_every_other_name_to_the_default_loader_of_slang() {
    let (name, symbol) = PLATFORM_LIBRARY;
    let (result, library) = load(name);
    assert_eq!(result, SlangResult::SLANG_OK);
    assert!(!library.is_null());
    assert!(!find(library, symbol).is_null());
    assert!(find(library, FUNCTIONS[0]).is_null());
    // SAFETY: `library` holds the one reference that the default loader
    // returned, and the test does not use it after this release.
    unsafe { ISlangSharedLibrary::release(library) };
    for name in [
        "no-such-library-of-slang-rust",
        "slang-glslang-directory/no-such-library-of-slang-rust",
        "slang-glslang-directory\\no-such-library-of-slang-rust",
    ] {
        let (result, library) = load(name);
        assert!(result.0 < 0, "{name}: {result:?}");
        assert!(library.is_null(), "{name}");
    }
}

/// Asks `object` for the interface `uuid` through `query`, and returns the
/// result code and the pointer that it wrote, starting from a pointer that is
/// not null, so that a missing write shows.
fn query<T>(
    object: *mut T,
    uuid: &SlangUUID,
    query: unsafe fn(*mut T, *const SlangUUID, *mut *mut c_void) -> SlangResult,
) -> (SlangResult, *mut c_void) {
    let mut out: *mut c_void = NonNull::dangling().as_ptr();
    // SAFETY: `object` is a live object of the shim, `uuid` a live UUID, and
    // `out` a live out-pointer, which is all that `queryInterface` reads and
    // writes.
    let result = unsafe { query(object, uuid, &raw mut out) };
    (result, out)
}

#[test]
fn the_loader_answers_queries_for_its_own_interfaces_only() {
    // SAFETY: the loader has no precondition.
    let loader = unsafe { slang_glslang_sys_loader() };
    for uuid in [ISlangUnknown::UUID, ISlangSharedLibraryLoader::UUID] {
        assert_eq!(
            query(loader, &uuid, ISlangSharedLibraryLoader::queryInterface),
            (SlangResult::SLANG_OK, loader.cast())
        );
    }
    for uuid in [ISlangSharedLibrary::UUID, IGlobalSession::UUID] {
        assert_eq!(
            query(loader, &uuid, ISlangSharedLibraryLoader::queryInterface),
            (SlangResult::SLANG_E_NO_INTERFACE, ptr::null_mut())
        );
    }
}

#[test]
fn the_linked_library_answers_queries_and_casts_for_its_interfaces_only() {
    let (_, library) = load(&format!("slang-glslang-{}", numeric_version()));
    for uuid in [
        ISlangUnknown::UUID,
        ISlangCastable::UUID,
        ISlangSharedLibrary::UUID,
    ] {
        assert_eq!(
            query(library, &uuid, ISlangSharedLibrary::queryInterface),
            (SlangResult::SLANG_OK, library.cast())
        );
        // SAFETY: `library` is the live static library of the loader, and
        // `uuid` a live UUID.
        let cast = unsafe { ISlangSharedLibrary::castAs(library, &raw const uuid) };
        assert_eq!(cast, library.cast());
    }
    for uuid in [ISlangSharedLibraryLoader::UUID, IGlobalSession::UUID] {
        assert_eq!(
            query(library, &uuid, ISlangSharedLibrary::queryInterface),
            (SlangResult::SLANG_E_NO_INTERFACE, ptr::null_mut())
        );
        // SAFETY: as above.
        let cast = unsafe { ISlangSharedLibrary::castAs(library, &raw const uuid) };
        assert!(cast.is_null());
    }
}
