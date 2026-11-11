#[cfg(test)]
mod compile;
#[cfg(test)]
mod exceptions;
#[cfg(test)]
mod paths;
#[cfg(test)]
mod reflection;
mod session;
#[cfg(test)]
mod settings;
#[cfg(test)]
mod support;

use std::thread;

use slang_compiler::{
    ErrorKind, GlobalSession, LanguageVersion, Operation, Session, Target, TargetFormat, Toggle,
};
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
fn two_global_sessions_on_one_thread_create_sessions_and_drop_in_either_order() {
    for first_dropped_first in [true, false] {
        let first = GlobalSession::new().expect("Slang creates a global session");
        let second = GlobalSession::new().expect("Slang creates a second global session");
        let sessions = [&first, &second].map(|global| {
            Session::builder(global)
                .target(Target::new(TargetFormat::Spirv))
                .build()
                .expect("Slang creates a session of each global session")
        });
        let [first_session, second_session] = sessions;
        if first_dropped_first {
            drop((first_session, first));
            drop((second_session, second));
        } else {
            drop((second_session, second));
            drop((first_session, first));
        }
    }
}

#[test]
fn a_global_session_on_another_thread_succeeds_while_one_is_alive() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let other = thread::spawn(|| {
        let global = GlobalSession::new().expect("Slang creates a global session");
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
    // global session of the pinned build, and only this thread uses it.
    let global = unsafe { GlobalSession::from_raw(raw) };
    assert_eq!(global.as_raw(), raw);
    global
        .check_target_support(TargetFormat::Spirv)
        .expect("the adopted global session works");
}
