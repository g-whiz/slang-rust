//! The tests of SLC-24 of `docs/design/slang-compiler.md`, which drive
//! invalid inputs through the calls that have no `try` in Slang, so that a
//! C++ exception that escapes one of them is observed (CA-1).
//!
//! One input lets an exception escape `Module::entry_point_with_stage`. The
//! crate then aborts the process on purpose, so the test runs it in a child
//! process and checks how the child ended.
use std::env;
use std::fmt::Write as _;
use std::process::{Command, ExitStatus};
use std::ptr;

use slang_compiler::{
    Component, ComponentType, ErrorKind, GlobalSession, Module, Session, SpecializationArg, Stage,
    Target, TargetFormat,
};
use slang_compiler_sys::{IComponentType, IEntryPoint, ISlangBlob, SlangResult};

use super::support::{find_type, has_code};

/// The environment variable that selects the input of a child process.
const CHILD: &str = "SLANG_COMPILER_EXCEPTION_CHILD";

/// The line that a child process prints immediately before the call.
const ABOUT_TO_CALL: &str = "the child is about to make the call";

/// The line that a child process prints when the call returned.
const RETURNED: &str = "the call returned";

/// A module with a generic compute kernel and a generic structure that a
/// type name can nest to any depth.
const GENERIC: &str = r#"
interface IValue { uint get(); }
struct Leaf : IValue { uint get() { return 1; } }
struct Box<T> : IValue { T inner; uint get() { return 2; } }
struct Plain { uint value; }
RWStructuredBuffer<uint> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void main<T : IValue>(uniform T value)
{
    output[0] = value.get();
}
"#;

/// Returns the name of a `Box` type nested `depth` times around `Leaf`,
/// deeper than the nesting limit of Slang, which is 128.
fn nested_type(depth: usize) -> String {
    format!("{}Leaf{}", "Box<".repeat(depth), ">".repeat(depth))
}

/// Returns a module source with a function whose parameter is a structure
/// nested `depth` levels deep, which Slang checks only when an entry point is
/// looked up with a stage, and which has no `[shader(...)]` attribute, so that
/// the load does not check it.
fn nested_parameter_source(depth: usize) -> String {
    let mut source = format!("struct S{depth} {{ uint x; }}\n");
    for level in (0..depth).rev() {
        let next = level + 1;
        writeln!(source, "struct S{level} {{ S{next} inner; }}").expect("a String takes text");
    }
    source.push_str("void deep(S0 value, uint3 id : SV_DispatchThreadID) {}\n");
    source
}

fn session(global: &GlobalSession) -> Session {
    Session::builder(global)
        .target(Target::new(TargetFormat::Spirv))
        .build()
        .expect("Slang creates the session")
}

fn composed(session: &Session) -> (Module, ComponentType) {
    let module = session
        .load_module_from_source("generic", "generic.slang", GENERIC)
        .expect("the source compiles")
        .into_value();
    let entry_point = module.entry_point("main").expect("the entry point exists");
    let composed = session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])
        .expect("the parts compose")
        .into_value();
    (module, composed)
}

/// Releases a diagnostics blob that a raw call returned, and returns whether
/// there was one.
fn release_diagnostics(blob: *mut ISlangBlob) -> bool {
    if blob.is_null() {
        return false;
    }
    // SAFETY: the raw call returned a new reference, which the test owns.
    unsafe { ISlangBlob::release(blob) };
    true
}

#[test]
fn specialize_with_invalid_and_deeply_nested_types_returns_errors() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = session(&global);
    let target = session.target_index(0).expect("a target");
    let (module, composed) = composed(&session);
    let plain = find_type(&session, &module, "Plain").expect("Plain exists");
    let error = composed
        .specialize(&[SpecializationArg::Type(plain)])
        .expect_err("Plain does not implement IValue");
    assert_eq!(error.kind(), ErrorKind::Compile);
    let error = composed
        .specialize(&[])
        .expect("specialization without arguments returns the input")
        .into_value()
        .link()
        .expect("an unspecialized composition links")
        .into_value()
        .entry_point_code(0, target)
        .expect_err("an unspecialized program generates no code");
    assert_eq!(error.kind(), ErrorKind::Compile, "{error}");
    assert_eq!(error.code(), Some(SlangResult::SLANG_FAIL.0), "{error}");
    assert!(has_code(error.diagnostics(), 99997), "{error}");
    let deep =
        find_type(&session, &module, &nested_type(300)).expect("Slang finds the nested type");
    let program = composed
        .specialize(&[SpecializationArg::Type(deep)])
        .expect("Slang accepts the nested type")
        .into_value()
        .link()
        .expect("the specialization links")
        .into_value();
    let error = program
        .entry_point_code(0, target)
        .expect_err("the nesting limit stops code generation");
    assert_eq!(error.kind(), ErrorKind::Compile);
    assert!(has_code(error.diagnostics(), 39997), "{error}");
    let error = program
        .target_metadata(target)
        .expect_err("the nesting limit stops code generation");
    assert_eq!(error.kind(), ErrorKind::Compile);
}

#[test]
fn code_generation_with_the_checks_of_the_crate_bypassed_fails_without_an_exception() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = session(&global);
    let (module, composed) = composed(&session);
    let entry_point = module.entry_point("main").expect("the entry point exists");
    let program = composed.link().expect("the composition links").into_value();
    let call = |raw: *mut IComponentType, entry_point: isize, target: isize| {
        let mut code: *mut ISlangBlob = ptr::null_mut();
        let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
        // SAFETY: `raw` is a live component type of `session`, which the test
        // keeps alive, and both out-pointers are live. The indexes are the
        // invalid inputs of the test, each of which Slang checks before it
        // indexes. An entry point index past the end is not among them,
        // because Slang reads out of bounds for it, which crashed on Windows.
        let result = unsafe {
            IComponentType::getEntryPointCode(
                raw,
                entry_point,
                target,
                &raw mut code,
                &raw mut diagnostics,
            )
        };
        if !code.is_null() {
            // SAFETY: the call returned a new reference, which the test owns.
            unsafe { ISlangBlob::release(code) };
        }
        (result, release_diagnostics(diagnostics))
    };
    let linked = program.as_raw().as_ptr();
    for (entry, target) in [(-1, 0), (0, 1), (0, -1)] {
        let (result, _) = call(linked, entry, target);
        assert!(
            result.0 < 0,
            "entry point {entry}, target {target}: {result:?}"
        );
    }
    let mut code: *mut ISlangBlob = ptr::null_mut();
    let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
    // SAFETY: `entry_point` keeps the entry point and its session alive, and
    // both out-pointers are live. The entry point is not linked, which is the
    // invalid input of this call.
    let result = unsafe {
        IEntryPoint::getEntryPointCode(
            entry_point.as_raw().as_ptr(),
            0,
            0,
            &raw mut code,
            &raw mut diagnostics,
        )
    };
    assert!(code.is_null());
    release_diagnostics(diagnostics);
    assert_eq!(result, SlangResult::SLANG_FAIL);
}

/// What a child process printed, and how it ended.
struct Child {
    status: ExitStatus,
    stdout: String,
    stderr: String,
}

/// Runs this test binary again with only the test `child` and the input
/// `input`.
fn run_child(input: &str) -> Child {
    let output = Command::new(env::current_exe().expect("the test binary has a path"))
        .args([
            "--exact",
            "ffi::exceptions::child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, input)
        .output()
        .expect("the child process starts");
    Child {
        status: output.status,
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// Returns whether `status` is the end of a process that
/// `std::process::abort` stopped: the signal `SIGABRT` on Unix, and the
/// status of `__fastfail` on Windows.
fn aborted(status: ExitStatus) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt as _;
        status.signal() == Some(6)
    }
    #[cfg(windows)]
    {
        status.code() == Some(0xC000_0409_u32.cast_signed())
    }
}

/// The body of the child process of the test below, which does nothing
/// unless the variable `CHILD` names an input.
#[test]
fn child() {
    let Ok(input) = env::var(CHILD) else {
        return;
    };
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = session(&global);
    match input.as_str() {
        "entry_point_with_stage" => {
            let module = session
                .load_module_from_source("deep", "deep.slang", &nested_parameter_source(140))
                .expect("the front end accepts the source")
                .into_value();
            println!("{ABOUT_TO_CALL}");
            let result = module.entry_point_with_stage("deep", Stage::Compute);
            println!(
                "{RETURNED} with {:?}",
                result.map(|compiled| compiled.diagnostics)
            );
        }
        other => panic!("unknown input {other}"),
    }
}

#[test]
fn entry_point_with_stage_aborts_the_process_when_slang_lets_an_exception_escape() {
    let child = run_child("entry_point_with_stage");
    let all = format!("{}{}", child.stdout, child.stderr);
    assert!(
        child.stdout.contains(ABOUT_TO_CALL),
        "the child did not reach the call: {all}"
    );
    assert!(
        !child.stdout.contains(RETURNED),
        "the call returned, so findAndCheckEntryPoint no longer lets the exception escape, and CA-1 may hold for it now: {all}"
    );
    assert!(
        child.stderr.contains(
            "slang-compiler: a C++ exception escaped from the Slang call IModule::findAndCheckEntryPoint, which Slang does not catch, so the process aborts"
        ),
        "the crate did not report the call: {all}"
    );
    assert!(
        aborted(child.status),
        "the child did not end by an abort, but with {:?}: {all}",
        child.status
    );
    assert!(!all.contains("panicked at"), "the child panicked: {all}");
}
