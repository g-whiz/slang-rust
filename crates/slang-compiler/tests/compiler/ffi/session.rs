//! The tests of sessions, targets, and compiler options against the Slang
//! library of the pinned Vulkan SDK.

use slang_compiler::{
    CompilerOptions, DebugInfoLevel, ErrorKind, FloatingPointMode, GlobalSession, LanguageVersion,
    LineDirectiveMode, MatrixLayout, Operation, OptimizationLevel, OptionName, Session,
    SessionBuilder, Target, TargetFormat, Toggle,
};
use slang_compiler_sys::{CompilerOptionName, ISession, SLANG_BUILD_TAG};

fn spirv() -> Target {
    Target::new(TargetFormat::Spirv)
}

fn every_typed_option() -> CompilerOptions {
    let mut options = CompilerOptions::default();
    options
        .language_version(LanguageVersion::V2026)
        .warnings_as_errors("30081")
        .enable_warning("30081")
        .disable_warning("30081")
        .optimization(OptimizationLevel::High)
        .debug_info(DebugInfoLevel::Minimal)
        .floating_point_mode(FloatingPointMode::Precise)
        .line_directive_mode(LineDirectiveMode::None)
        .vulkan_use_entry_point_name(Toggle::On)
        .emit_spirv_directly(Toggle::On)
        .skip_spirv_validation(Toggle::Off)
        .glsl_force_scalar_layout(Toggle::On)
        .force_dx_layout(Toggle::Off)
        .force_c_layout(Toggle::Off)
        .preserve_parameters(Toggle::On);
    options
}

fn four_targets(global: &GlobalSession) -> SessionBuilder {
    Session::builder(global)
        .target(
            spirv()
                .profile("spirv_1_5")
                .capability("spvInt64Atomics")
                .options(every_typed_option()),
        )
        .target(Target::new(TargetFormat::Wgsl))
        .target(Target::new(TargetFormat::Hlsl).profile("sm_6_5"))
        .target(Target::new(TargetFormat::Metal))
}

fn assert_rejected(builder: SessionBuilder, kind: ErrorKind, text: &str) {
    let error = builder.build().err();
    assert!(
        error.is_some(),
        "the session was created from an invalid setting"
    );
    if let Some(error) = error {
        assert_eq!(error.kind(), kind, "{error}");
        assert_eq!(error.operation(), Operation::CreateSession);
        assert_eq!(error.code(), None);
        assert!(error.diagnostics().is_empty());
        assert!(error.to_string().contains(text), "{error}");
    }
}

#[test]
fn a_session_with_four_targets_and_every_setting_is_created() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let uniformity =
        OptionName::new(CompilerOptionName::ValidateUniformity.0).expect("a stored option");
    let mut options = every_typed_option();
    // SAFETY: Slang reads `ValidateUniformity` as an integer that is 0 or 1.
    unsafe { options.set_int(uniformity, 1, 0) };
    let session = four_targets(&global)
        .search_path("/tmp/slang-compiler-shaders")
        .search_path("relative/shaders")
        .define("BLOCK_SIZE", "64")
        .matrix_layout(MatrixLayout::ColumnMajor)
        .use_up_to_date_binary_module(Toggle::On)
        .options(options)
        .build()
        .expect("Slang creates the session");
    assert_eq!(session.target_count(), 4);
    assert_eq!(session.global_session().as_raw(), global.as_raw());
}

#[test]
fn target_index_accepts_each_target_and_rejects_the_position_past_the_end() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = four_targets(&global)
        .build()
        .expect("Slang creates the session");
    for position in 0..4 {
        let index = session.target_index(position).expect("the target exists");
        assert_eq!(session.target_index(position).ok(), Some(index));
    }
    assert_ne!(session.target_index(0).ok(), session.target_index(3).ok());
    for position in [4, usize::MAX] {
        let error = session
            .target_index(position)
            .expect_err("no target at this position");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::SelectTarget);
        assert_eq!(error.code(), None);
    }
}

#[test]
fn build_without_a_target_fails_with_invalid_argument() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    assert_rejected(
        Session::builder(&global).search_path("/tmp"),
        ErrorKind::InvalidArgument,
        "no target",
    );
}

#[test]
fn build_rejects_a_nul_byte_in_each_kind_of_string() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let mut warnings = CompilerOptions::default();
    warnings.enable_warning("30081\0");
    let cases = [
        (
            Session::builder(&global)
                .target(spirv())
                .search_path("/tmp/a\0b"),
            "the search path",
        ),
        (
            Session::builder(&global)
                .target(spirv())
                .define("NAME\0", "1"),
            "the macro name",
        ),
        (
            Session::builder(&global)
                .target(spirv())
                .define("NAME", "\u{0}1"),
            "the macro value",
        ),
        (
            Session::builder(&global).target(spirv().profile("spirv_1_5\0")),
            "the profile",
        ),
        (
            Session::builder(&global).target(spirv().capability("spvInt64Atomics\0")),
            "the capability",
        ),
        (
            Session::builder(&global)
                .target(spirv())
                .options(warnings.clone()),
            "compiler option 21",
        ),
        (
            Session::builder(&global).target(spirv().options(warnings)),
            "compiler option 21",
        ),
    ];
    for (builder, text) in cases {
        assert_rejected(builder, ErrorKind::InvalidArgument, text);
    }
}

#[cfg(unix)]
#[test]
fn build_rejects_a_search_path_that_is_not_utf8() {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt;

    let global = GlobalSession::new().expect("Slang creates a global session");
    let path = OsStr::from_bytes(b"/tmp/shaders-\xff");
    assert_rejected(
        Session::builder(&global).target(spirv()).search_path(path),
        ErrorKind::InvalidArgument,
        "not valid UTF-8",
    );
}

#[cfg(windows)]
#[test]
fn build_rejects_a_search_path_that_is_not_utf8() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    let global = GlobalSession::new().expect("Slang creates a global session");
    let path = OsString::from_wide(&[u16::from(b's'), 0xD800, u16::from(b'x')]);
    assert_rejected(
        Session::builder(&global).target(spirv()).search_path(path),
        ErrorKind::InvalidArgument,
        "not valid UTF-8",
    );
}

#[test]
fn build_rejects_a_profile_that_slang_does_not_know() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    assert_rejected(
        Session::builder(&global).target(spirv().profile("spirv_9_9")),
        ErrorKind::NotFound,
        "profile `spirv_9_9`",
    );
}

#[test]
fn build_rejects_a_capability_that_slang_does_not_know() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    assert_rejected(
        Session::builder(&global).target(spirv().capability("teleportation")),
        ErrorKind::NotFound,
        "capability `teleportation`",
    );
}

#[test]
fn a_session_keeps_its_global_session_alive_after_the_handle_is_dropped() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = Session::builder(&global)
        .target(spirv())
        .build()
        .expect("Slang creates the session");
    drop(global);
    assert_eq!(session.global_session().build_tag(), SLANG_BUILD_TAG);
    session
        .global_session()
        .check_target_support(TargetFormat::Spirv)
        .expect("the global session still works");
    assert_eq!(
        GlobalSession::new().map(drop).map_err(|error| error.kind()),
        Err(ErrorKind::AlreadyExists)
    );
    drop(session);
    GlobalSession::new().expect("no global session is alive on this thread");
}

#[test]
fn into_raw_and_from_raw_hand_the_session_over() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let raw = Session::builder(&global)
        .target(spirv())
        .target(Target::new(TargetFormat::Hlsl))
        .build()
        .expect("Slang creates the session")
        .into_raw();
    // SAFETY: `raw` is a live session, because the test owns the reference
    // that `into_raw` gave up, and only this thread uses it.
    let raw_global = unsafe { ISession::getGlobalSession(raw.as_ptr()) };
    assert_eq!(raw_global, global.as_raw().as_ptr());
    // SAFETY: `raw` carries the one reference that `into_raw` gave up, of a
    // session of the pinned build that `global` created with two targets, and
    // only this thread uses it.
    let session = unsafe { Session::from_raw(raw, global.clone(), 2) };
    assert_eq!(session.as_raw(), raw);
    session
        .target_index(1)
        .expect("the session has two targets");
}
