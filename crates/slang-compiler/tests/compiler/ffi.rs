#[cfg(test)]
mod compile;
#[cfg(test)]
mod exceptions;
mod session;
#[cfg(test)]
mod settings;
#[cfg(test)]
mod support;
mod two_global_sessions;

use std::thread;

use slang_compiler::{ErrorKind, GlobalSession, LanguageVersion, Operation, TargetFormat, Toggle};
use slang_compiler_sys::SLANG_BUILD_TAG;

#[test]
fn global_session_new_reports_the_pinned_build_tag() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    assert_eq!(global.build_tag(), SLANG_BUILD_TAG);
}

#[test]
fn global_session_builder_creates_a_global_session_with_its_settings() {
    let global = GlobalSession::builder()
        .glsl(Toggle::On)
        .min_language_version(LanguageVersion::V2026)
        .build()
        .expect("Slang creates a global session with GLSL input");
    assert_eq!(global.build_tag(), SLANG_BUILD_TAG);
}

#[test]
fn check_target_support_accepts_spirv() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    global
        .check_target_support(TargetFormat::Spirv)
        .expect("the Vulkan SDK ships what SPIR-V needs");
}

#[test]
fn check_target_support_reports_each_format_as_supported_or_unsupported() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    for format in [
        TargetFormat::Spirv,
        TargetFormat::SpirvAssembly,
        TargetFormat::Glsl,
        TargetFormat::Hlsl,
        TargetFormat::Wgsl,
        TargetFormat::Metal,
        TargetFormat::CppSource,
        TargetFormat::CudaSource,
        TargetFormat::Dxil,
    ] {
        match global.check_target_support(format) {
            Ok(()) => println!("{format:?}: supported"),
            Err(error) => {
                assert_eq!(error.kind(), ErrorKind::Unsupported, "{error}");
                assert_eq!(error.operation(), Operation::CheckTargetSupport);
                println!("{format:?}: {error}");
            }
        }
    }
}

#[test]
fn a_second_global_session_on_the_same_thread_fails_with_already_exists() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    for result in [GlobalSession::new(), GlobalSession::builder().build()] {
        let error = result.expect_err("a global session is alive on this thread");
        assert_eq!(error.kind(), ErrorKind::AlreadyExists);
        assert_eq!(error.operation(), Operation::CreateGlobalSession);
        assert_eq!(error.code(), None);
        assert!(error.diagnostics().is_empty());
    }
    let clone = global.clone();
    drop(global);
    let error = GlobalSession::new().expect_err("a clone keeps the global session alive");
    assert_eq!(error.kind(), ErrorKind::AlreadyExists);
    drop(clone);
}

#[test]
fn a_global_session_after_the_first_is_dropped_succeeds() {
    let first = GlobalSession::new().expect("Slang creates a global session");
    drop(first);
    let second = GlobalSession::new().expect("no global session is alive on this thread");
    assert_eq!(second.build_tag(), SLANG_BUILD_TAG);
}

#[test]
fn a_global_session_on_another_thread_succeeds_while_one_is_alive() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let other = thread::spawn(|| {
        let global = GlobalSession::new().expect("the other thread has no global session");
        global
            .check_target_support(TargetFormat::Spirv)
            .map(|()| global.build_tag().to_owned())
    })
    .join()
    .expect("the other thread does not panic");
    assert_eq!(other.expect("SPIR-V is supported"), SLANG_BUILD_TAG);
    global
        .check_target_support(TargetFormat::Spirv)
        .expect("the first global session still works");
}

#[test]
fn into_raw_and_from_raw_hand_the_global_session_over() {
    let raw = GlobalSession::new()
        .expect("Slang creates a global session")
        .into_raw();
    // SAFETY: `raw` carries the one reference that `into_raw` gave up, of a
    // global session of the pinned build, no other global session of the
    // crate is alive on this thread, and only this thread uses it.
    let global = unsafe { GlobalSession::from_raw(raw) };
    assert_eq!(global.as_raw(), raw);
    global
        .check_target_support(TargetFormat::Spirv)
        .expect("the adopted global session works");
    assert_eq!(
        GlobalSession::new().map(drop).map_err(|error| error.kind()),
        Err(ErrorKind::AlreadyExists)
    );
}
