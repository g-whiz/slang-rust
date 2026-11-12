//! The tests that drive invalid inputs through the calls that have no `try`
//! in Slang, so that a C++ exception that escapes one of them is observed.
//!
//! One input lets an exception escape `Module::entry_point_with_stage`. The
//! crate then aborts the process on purpose, so the test runs it in a child
//! process and checks how the child ended.
use std::env;
use std::fmt::Write as _;
use std::process::{Command, ExitStatus};
use std::ptr;

use slang_compiler::reflection::{EntryPointLayout, LayoutSize, TypeLayout, VariableLayout};
use slang_compiler::{
    Component, ComponentType, ErrorKind, GlobalSession, Module, ParameterCategory, Session,
    SpecializationArg, Stage, Target, TargetFormat,
};
use slang_compiler_sys::{IComponentType, IEntryPoint, ISlangBlob, SlangResult};

use super::support::{find_type, has_code};

/// The environment variable that selects the input of a child process.
const CHILD: &str = "SLANG_COMPILER_EXCEPTION_CHILD";

/// The environment variable that gives a child process the number of
/// structures or the depth of nesting of its input.
const DEPTH: &str = "SLANG_COMPILER_EXCEPTION_DEPTH";

/// The line that a child process prints when the module of its input does
/// not load.
const LOAD_FAILED: &str = "the module did not load";

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

/// A module with a generic structure and a generic compute kernel whose
/// sizes come from generic value parameters, which Slang knows only after
/// specialization.
const VALUE_GENERIC: &str = r#"
struct Sized<let N : int>
{
    float data[N];
    vector<float, N> v;
    matrix<float, N, 2> m;
}

[shader("compute")]
[numthreads(1, 1, 1)]
void main<let M : int>(uniform float values[M], uniform vector<float, M> v)
{
}
"#;

/// Returns a module source with a chain of `count` structures, in which
/// `S0` holds `S1` and so on, and the last holds one `uint`, and with the
/// global parameter `global`, such as `uniform S0 u;`.
fn chain_source(count: usize, global: &str) -> String {
    let last = count - 1;
    let mut source = format!("struct S{last} {{ uint x; }}\n");
    for level in (0..last).rev() {
        let next = level + 1;
        writeln!(source, "struct S{level} {{ S{next} inner; }}").expect("a String takes text");
    }
    source.push_str(global);
    source.push('\n');
    source
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
    run_child_with_depth(input, 0)
}

/// Runs this test binary again with only the test `child`, the input `input`,
/// and the depth `depth`.
fn run_child_with_depth(input: &str, depth: usize) -> Child {
    let output = Command::new(env::current_exe().expect("the test binary has a path"))
        .args([
            "--exact",
            "ffi::exceptions::child",
            "--nocapture",
            "--test-threads=1",
        ])
        .env(CHILD, input)
        .env(DEPTH, depth.to_string())
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

/// The body of the child processes of the tests of this module, which does
/// nothing unless the variable `CHILD` names an input.
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
        "module_layout_uniform" => child_module_layout(&session, "uniform S0 u;"),
        "module_layout_buffer" => child_module_layout(&session, "StructuredBuffer<S0> sb;"),
        "linked_program_layout" => child_linked_program_layout(&session),
        "sized_element_count" | "sized_vector_count" | "sized_column_count" | "sized_row_count" => {
            child_sized(&session, &input);
        }
        "generic_entry_point_parameter" => child_generic_entry_point_parameter(&session),
        "generic_composite_layout"
        | "generic_composite_json"
        | "generic_composite_binding_ranges"
        | "generic_composite_descriptor_sets"
        | "generic_composite_field_binding_range_offset"
        | "generic_composite_walk" => child_generic_composite(&session, &input),
        other => panic!("unknown input {other}"),
    }
}

/// Lays out a module of a chain of [`depth`] structures with the global
/// parameter `global`.
fn child_module_layout(session: &Session, global: &str) {
    let Ok(module) =
        session.load_module_from_source("chain", "chain.slang", &chain_source(depth(), global))
    else {
        println!("{LOAD_FAILED}");
        return;
    };
    let module = module.into_value();
    let target = session.target_index(0).expect("a target");
    println!("{ABOUT_TO_CALL}");
    let result = module.layout(target).map(|layout| layout.diagnostics);
    println!("{RETURNED} with {result:?}");
}

/// Lays out the generic kernel of [`GENERIC`] specialized with a `Box`
/// nested [`depth`] times around `Leaf`.
fn child_linked_program_layout(session: &Session) {
    let (module, composed) = composed(session);
    let deep =
        find_type(session, &module, &nested_type(depth())).expect("Slang finds the nested type");
    let program = composed
        .specialize(&[SpecializationArg::Type(deep)])
        .expect("Slang accepts the nested type")
        .into_value()
        .link()
        .expect("the specialization links")
        .into_value();
    let target = session.target_index(0).expect("a target");
    println!("{ABOUT_TO_CALL}");
    let result = program.layout(target).map(|layout| layout.diagnostics);
    println!("{RETURNED} with {result:?}");
}

/// Reads a count of a field of the generic structure of [`VALUE_GENERIC`],
/// whose sizes a generic value parameter gives.
fn child_sized(session: &Session, input: &str) {
    let module = session
        .load_module_from_source("sized", "sized.slang", VALUE_GENERIC)
        .expect("the source compiles")
        .into_value();
    let target = session.target_index(0).expect("a target");
    let layout = module
        .layout(target)
        .expect("Slang lays out the module")
        .into_value();
    let sized = layout
        .find_type_by_name("Sized")
        .expect("no NUL byte")
        .expect("Slang finds the generic structure");
    let fields = sized
        .fields()
        .map(|field| field.ty().expect("the field has a type"))
        .collect::<Vec<_>>();
    println!("{ABOUT_TO_CALL}");
    let result = match input {
        "sized_element_count" => format!("{:?}", fields[0].element_count()),
        "sized_vector_count" => format!("{:?}", fields[1].element_count()),
        "sized_column_count" => format!("{:?}", fields[1].column_count()),
        _ => format!("{:?}", fields[2].row_count()),
    };
    println!("{RETURNED} with {result}");
}

/// Reads the length of the array parameter of the generic entry point of
/// [`VALUE_GENERIC`] through the function of the entry point.
fn child_generic_entry_point_parameter(session: &Session) {
    let module = session
        .load_module_from_source("sized", "sized.slang", VALUE_GENERIC)
        .expect("the source compiles")
        .into_value();
    let entry_point = module.entry_point("main").expect("the entry point exists");
    let function = entry_point
        .function()
        .expect("the entry point has a function");
    let parameter = function.parameters().next().expect("a parameter");
    let ty = parameter.ty().expect("the parameter has a type");
    println!("{ABOUT_TO_CALL}");
    let result = ty.element_count();
    println!("{RETURNED} with {result:?}");
}

/// Lays out or reflects the composition of [`VALUE_GENERIC`] with its
/// generic entry point, which is not specialized.
fn child_generic_composite(session: &Session, input: &str) {
    let module = session
        .load_module_from_source("sized", "sized.slang", VALUE_GENERIC)
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
    let target = session.target_index(0).expect("a target");
    if input == "generic_composite_layout" {
        println!("{ABOUT_TO_CALL}");
        let result = composed.layout(target).map(|layout| layout.diagnostics);
        println!("{RETURNED} with {result:?}");
        return;
    }
    let layout = composed
        .layout(target)
        .expect("Slang lays it out")
        .into_value();
    let scope = layout
        .entry_point_layouts()
        .next()
        .and_then(EntryPointLayout::var_layout)
        .and_then(VariableLayout::type_layout)
        .expect("the entry point has a scope");
    println!("{ABOUT_TO_CALL}");
    let result = match input {
        "generic_composite_json" => format!("{:?}", layout.to_json().map(|json| json.len())),
        "generic_composite_binding_ranges" => {
            format!("{:?}", scope.binding_ranges().map(|ranges| ranges.len()))
        }
        "generic_composite_descriptor_sets" => {
            format!("{:?}", scope.descriptor_sets().map(|sets| sets.len()))
        }
        "generic_composite_field_binding_range_offset" => {
            format!("{:?}", scope.field_binding_range_offset(0))
        }
        _ => walk_fields(scope),
    };
    println!("{RETURNED} with {result}");
}

/// Describes each field of a type layout with the reflection functions that
/// need no binding ranges.
fn walk_fields(scope: TypeLayout<'_>) -> String {
    let mut out = String::new();
    for field in scope.field_layouts() {
        let layout = field.type_layout().expect("a type layout");
        let ty = layout.ty().expect("a type");
        writeln!(
            out,
            "{:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?} {:?}",
            layout.kind(),
            layout.size(ParameterCategory::Uniform),
            layout.stride(ParameterCategory::Uniform),
            layout.element_stride(ParameterCategory::Uniform),
            layout.categories().collect::<Vec<_>>(),
            field.offset(ParameterCategory::Uniform),
            ty.full_name(),
            ty.kind(),
            ty.fields().count(),
        )
        .expect("a String takes text");
    }
    out
}

/// Returns the depth that the parent gave the child.
fn depth() -> usize {
    env::var(DEPTH)
        .expect("the parent gives a depth")
        .parse()
        .expect("the depth is a number")
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

/// How a child process ended.
#[derive(Debug, PartialEq, Eq)]
enum Outcome {
    /// The module of the input did not load.
    LoadFailed,
    /// The call returned.
    Returned,
    /// The crate aborted the process at the named Slang call.
    Aborted(String),
}

/// Runs the child input `input` with `depth` and returns how it ended,
/// checking that a child that aborted reached the call, printed nothing
/// that says that the call returned, reported the call on standard error,
/// and ended with the status of an abort.
fn outcome(input: &str, depth: usize) -> Outcome {
    let child = run_child_with_depth(input, depth);
    let all = format!("{}{}", child.stdout, child.stderr);
    assert!(!all.contains("panicked at"), "the child panicked: {all}");
    if child.stdout.contains(LOAD_FAILED) {
        assert!(child.status.success(), "{all}");
        return Outcome::LoadFailed;
    }
    assert!(
        child.stdout.contains(ABOUT_TO_CALL),
        "the child did not reach the call: {all}"
    );
    if child.stdout.contains(RETURNED) {
        assert!(child.status.success(), "{all}");
        return Outcome::Returned;
    }
    assert!(
        aborted(child.status),
        "the child did not end by an abort, but with {:?}: {all}",
        child.status
    );
    let prefix = "slang-compiler: a C++ exception escaped from the Slang call ";
    let suffix = ", which Slang does not catch, so the process aborts";
    let line = child
        .stderr
        .lines()
        .find(|line| line.starts_with(prefix) && line.ends_with(suffix))
        .unwrap_or_else(|| panic!("the crate did not report the call: {all}"));
    Outcome::Aborted(line[prefix.len()..line.len() - suffix.len()].to_owned())
}

fn aborted_at(callee: &str) -> Outcome {
    Outcome::Aborted(callee.to_owned())
}

#[test]
fn module_layout_of_a_uniform_chain_returns_at_126_aborts_at_127_and_fails_to_load_from_128() {
    assert_eq!(outcome("module_layout_uniform", 126), Outcome::Returned);
    assert_eq!(
        outcome("module_layout_uniform", 127),
        aborted_at("IModule::getLayout")
    );
    assert_eq!(outcome("module_layout_uniform", 128), Outcome::LoadFailed);
}

#[test]
fn module_layout_of_a_structured_buffer_chain_returns_at_126_and_aborts_from_127() {
    assert_eq!(outcome("module_layout_buffer", 126), Outcome::Returned);
    for depth in [127, 128, 300] {
        assert_eq!(
            outcome("module_layout_buffer", depth),
            aborted_at("IModule::getLayout"),
            "{depth} structures"
        );
    }
}

#[test]
fn linked_program_layout_returns_for_126_nested_boxes_and_aborts_for_127() {
    assert_eq!(outcome("linked_program_layout", 126), Outcome::Returned);
    assert_eq!(
        outcome("linked_program_layout", 127),
        aborted_at("IComponentType::getLayout")
    );
}

#[test]
fn counts_of_types_sized_by_a_generic_value_parameter_abort_the_process() {
    for (input, callee) in [
        ("sized_element_count", "spReflectionType_GetElementCount"),
        ("sized_vector_count", "spReflectionType_GetElementCount"),
        ("sized_column_count", "spReflectionType_GetColumnCount"),
        ("sized_row_count", "spReflectionType_GetRowCount"),
        (
            "generic_entry_point_parameter",
            "spReflectionType_GetElementCount",
        ),
    ] {
        assert_eq!(outcome(input, 0), aborted_at(callee), "{input}");
    }
}

#[test]
fn json_and_binding_ranges_of_a_layout_sized_by_a_generic_value_parameter_abort_the_process() {
    assert_eq!(outcome("generic_composite_layout", 0), Outcome::Returned);
    assert_eq!(outcome("generic_composite_walk", 0), Outcome::Returned);
    for (input, callee) in [
        ("generic_composite_json", "spReflection_ToJson"),
        (
            "generic_composite_binding_ranges",
            "spReflectionTypeLayout_getBindingRangeCount",
        ),
        (
            "generic_composite_descriptor_sets",
            "spReflectionTypeLayout_getDescriptorSetCount",
        ),
        (
            "generic_composite_field_binding_range_offset",
            "spReflectionTypeLayout_getFieldBindingRangeOffset",
        ),
    ] {
        assert_eq!(outcome(input, 0), aborted_at(callee), "{input}");
    }
}

#[test]
fn type_layout_of_a_type_past_the_nesting_limit_returns_an_unknown_size() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = session(&global);
    let module = session
        .load_module_from_source("generic", "generic.slang", GENERIC)
        .expect("the source compiles")
        .into_value();
    let target = session.target_index(0).expect("a target");
    let layout = module
        .layout(target)
        .expect("Slang lays out the module")
        .into_value();
    for (depth, expected) in [
        (126, LayoutSize::Finite(16)),
        (127, LayoutSize::Unknown),
        (300, LayoutSize::Unknown),
        (1000, LayoutSize::Unknown),
    ] {
        let name = format!("{}Plain{}", "Box<".repeat(depth), ">".repeat(depth));
        let deep = layout
            .find_type_by_name(&name)
            .expect("no NUL byte")
            .expect("Slang finds the nested type");
        let type_layout = layout
            .type_layout(deep)
            .expect("the type belongs to the session")
            .expect("Slang lays out the type");
        assert_eq!(
            type_layout.size(ParameterCategory::Uniform).amount,
            expected,
            "{depth} boxes"
        );
    }
}

#[test]
fn hostile_names_are_answered_without_an_exception() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = session(&global);
    let (module, composed) = composed(&session);
    let target = session.target_index(0).expect("a target");
    let layout = module
        .layout(target)
        .expect("Slang lays out the module")
        .into_value();
    let long = "x".repeat(100_000);
    let qualified = "a::".repeat(1_000) + "b";
    let dotted = "a.".repeat(1_000) + "b";
    let malformed = [
        "",
        "Box<<<int,,>",
        "Box<",
        "1 + 2",
        "::",
        "Box<Leaf>>",
        "Box<Leaf, Leaf>",
        "IValue<Leaf>",
        ")",
        "Box<Leaf)>",
        long.as_str(),
        qualified.as_str(),
        dotted.as_str(),
    ];
    for name in malformed {
        let short = &name[..name.len().min(20)];
        layout
            .find_type_by_name(name)
            .expect("Slang answers without an exception");
        assert!(
            layout
                .find_entry_point_by_name(name)
                .expect("no NUL byte")
                .is_none(),
            "{short}"
        );
    }
    let deep = "Box<".repeat(100);
    assert!(
        layout
            .find_type_by_name(&deep)
            .expect("no NUL byte")
            .is_none()
    );
    let program = composed.link().expect("the composition links").into_value();
    let program_layout = program
        .layout(target)
        .expect("Slang lays out the program")
        .into_value();
    let scope = program_layout
        .entry_point_layouts()
        .next()
        .and_then(EntryPointLayout::var_layout)
        .and_then(VariableLayout::type_layout)
        .expect("the entry point has a scope");
    for name in malformed {
        assert!(
            scope
                .find_field_index_by_name(name)
                .expect("no NUL byte")
                .is_none()
        );
    }
    let parameter = scope
        .field_layouts()
        .next()
        .and_then(VariableLayout::variable)
        .expect("the entry point has a parameter");
    for name in malformed {
        assert!(
            parameter
                .find_user_attribute_by_name(name)
                .expect("no NUL byte")
                .is_none()
        );
    }
    let error = layout
        .find_type_by_name("Box<\0Leaf>")
        .expect_err("a NUL byte fails");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
}

#[test]
fn a_generic_thread_group_size_is_zero_without_an_exception() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = session(&global);
    let source = "[shader(\"compute\")] [numthreads(N, 2, 1)] void main<let N : int>() {}";
    let module = session
        .load_module_from_source("threads", "threads.slang", source)
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
    let target = session.target_index(0).expect("a target");
    let layout = composed
        .layout(target)
        .expect("Slang lays out the composition")
        .into_value();
    let entry = layout
        .entry_point_layouts()
        .next()
        .expect("the layout has the entry point");
    assert_eq!(entry.compute_thread_group_size(), [0, 2, 1]);
}
