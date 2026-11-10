//! The tests of modules, entry points, composition, specialization, linking,
//! code generation, blobs, and metadata against the Slang library of the
//! pinned Vulkan SDK.

use std::fs;
use std::slice;

use slang_compiler::{
    BindingIndex, BindingSpace, CompilerOptions, Component, ErrorKind, GlobalSession,
    LinkedProgram, Module, Operation, ParameterCategory, Session, SpecializationArg, Stage, Target,
    TargetFormat, Toggle,
};
use slang_compiler_sys::{IMetadata, ISlangBlob, SlangResult};

use super::support::{
    self, SPIRV_MAGIC, compile, compile_spirv, entry_point_names, find_type, has_code, scratch_dir,
    words,
};

const KERNEL: &str = r#"
RWStructuredBuffer<float> data;

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID, uniform float factor)
{
    data[id.x] *= factor;
}
"#;

const GENERIC: &str = r#"
interface IValue { uint get(); }
struct One : IValue { uint get() { return 1; } }
struct Plain { uint value; }
RWStructuredBuffer<uint> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void main<T : IValue>(uniform T value)
{
    output[0] = value.get();
}
"#;

fn session(global: &GlobalSession, format: TargetFormat) -> Session {
    Session::builder(global)
        .target(Target::new(format))
        .build()
        .expect("Slang creates the session")
}

fn spirv_session(global: &GlobalSession) -> Session {
    session(global, TargetFormat::Spirv)
}

fn link_kernel(session: &Session) -> (Module, LinkedProgram) {
    let module = session
        .load_module_from_source("kernel", "kernel.slang", KERNEL)
        .expect("the kernel compiles")
        .into_value();
    let entry_point = module.entry_point("main").expect("the kernel defines main");
    let program = session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])
        .expect("the parts compose")
        .into_value()
        .link()
        .expect("the composition links")
        .into_value();
    (module, program)
}

#[test]
fn every_supported_target_format_compiles_a_compute_kernel() {
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
        if let Err(error) = global.check_target_support(format) {
            assert_eq!(error.kind(), ErrorKind::Unsupported, "{error}");
            println!("{format:?}: not supported here, {error}");
            continue;
        }
        let session = session(&global, format);
        let code = compile(&session, "kernel", KERNEL, 0)
            .unwrap_or_else(|error| panic!("{format:?}: {error}"))
            .into_value();
        assert!(!code.as_bytes().is_empty(), "{format:?}");
        match format {
            TargetFormat::Spirv => {
                assert_eq!(words(code.as_bytes()).first(), Some(&SPIRV_MAGIC));
            }
            TargetFormat::Dxil => {}
            _ => {
                let text = code.to_str().expect("a text target returns text");
                assert!(text.contains("main"), "{format:?}: {text}");
            }
        }
        println!("{format:?}: {} bytes", code.as_bytes().len());
    }
}

#[test]
fn one_linked_program_generates_code_for_each_target_of_its_session() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv).profile("spirv_1_5"))
        .target(Target::new(TargetFormat::Wgsl))
        .target(Target::new(TargetFormat::Hlsl))
        .target(Target::new(TargetFormat::Metal))
        .build()
        .expect("Slang creates the session");
    let (_module, program) = link_kernel(&session);
    assert_eq!(program.entry_point_count(), 1);
    for position in 0..session.target_count() {
        let target = session.target_index(position).expect("a target");
        let compiled = program
            .entry_point_code(0, target)
            .expect("Slang generates the code");
        assert_ne!(compiled.value.as_bytes(), []);
        let whole = program
            .target_code(target)
            .expect("Slang generates the whole program");
        assert_ne!(whole.value.as_bytes(), []);
    }
    let spirv = program
        .entry_point_code(0, session.target_index(0).expect("a target"))
        .expect("Slang generates SPIR-V")
        .into_value();
    let copied = words(spirv.as_bytes());
    assert_eq!(copied.first(), Some(&SPIRV_MAGIC));
    let aligned = spirv.as_bytes().as_ptr().align_offset(align_of::<u32>()) == 0;
    let native = spirv
        .as_bytes()
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| u32::from_ne_bytes(*word))
        .collect::<Vec<_>>();
    assert_eq!(spirv.as_spirv_words(), aligned.then_some(native.as_slice()));
}

#[test]
fn a_warning_is_returned_with_a_successful_load() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let compiled = session
        .load_module_from_source(
            "warns",
            "warns.slang",
            "int pick(int a, int b) { return (a, b); }",
        )
        .expect("a warning does not fail the load");
    assert!(
        has_code(&compiled.diagnostics, 41024),
        "{}",
        compiled.diagnostics
    );
}

#[test]
fn malformed_source_is_a_compile_error_with_the_diagnostics() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let error = session
        .load_module_from_source("broken", "broken.slang", "float f() { return missing; }")
        .expect_err("an undefined identifier fails");
    assert_eq!(error.kind(), ErrorKind::Compile);
    assert_eq!(error.operation(), Operation::LoadModule);
    assert_eq!(error.code(), None);
    assert!(has_code(error.diagnostics(), 30015), "{error}");
    assert!(
        error
            .to_string()
            .starts_with("cannot load module `broken`:\n"),
        "{error}"
    );
}

#[test]
fn a_missing_module_fails_to_compile_and_a_second_load_is_not_found() {
    let dir = scratch_dir("missing_module");
    let global = GlobalSession::new().expect("Slang creates a global session");
    let builder = || {
        Session::builder(&global)
            .target(Target::new(TargetFormat::Spirv))
            .search_path(&dir)
    };
    let session = builder().build().expect("Slang creates the session");
    let first = session
        .load_module("nowhere")
        .expect_err("no file holds the module");
    assert_eq!(first.kind(), ErrorKind::Compile, "{first}");
    assert!(has_code(first.diagnostics(), 1), "{first}");
    fs::write(
        dir.join("nowhere.slang"),
        "float twice(float x) { return 2.0 * x; }",
    )
    .expect("the file is written");
    let second = session
        .load_module("nowhere")
        .expect_err("Slang remembers the failure");
    assert_eq!(second.kind(), ErrorKind::NotFound, "{second}");
    assert!(second.diagnostics().is_empty(), "{second}");
    assert!(
        second.to_string().contains("a new session loads it again"),
        "{second}"
    );
    let third = session
        .load_module_from_source(
            "nowhere",
            "nowhere.slang",
            "float half(float x) { return x; }",
        )
        .expect_err("Slang remembers the failure for a load from source too");
    assert_eq!(third.kind(), ErrorKind::NotFound, "{third}");
    let fresh = builder().build().expect("Slang creates the session");
    let module = fresh
        .load_module("nowhere")
        .expect("a new session finds the file")
        .into_value();
    assert_eq!(module.name().expect("UTF-8"), Some("nowhere"));
}

#[test]
fn a_second_load_of_a_name_with_different_source_fails() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let first = session
        .load_module_from_source(
            "twice",
            "twice.slang",
            "float twice(float x) { return 2.0 * x; }",
        )
        .expect("the first load compiles")
        .into_value();
    let same = session
        .load_module_from_source(
            "twice",
            "twice.slang",
            "float twice(float x) { return 2.0 * x; }",
        )
        .expect("identical source returns the loaded module")
        .into_value();
    assert_eq!(same.as_raw(), first.as_raw());
    let error = session
        .load_module_from_source(
            "twice",
            "twice.slang",
            "float twice(float x) { return x + x; }",
        )
        .expect_err("different source fails");
    assert_eq!(error.kind(), ErrorKind::Compile);
    assert!(has_code(error.diagnostics(), 38202), "{error}");
}

#[test]
fn a_module_and_a_program_work_after_every_other_handle_is_dropped() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let target = session.target_index(0).expect("a target");
    let (module, program) = link_kernel(&session);
    drop(session);
    drop(global);
    let entry_point = module
        .entry_point("main")
        .expect("the module keeps its session");
    let code = program
        .entry_point_code(0, target)
        .expect("the program keeps its session")
        .into_value();
    assert_eq!(words(code.as_bytes()).first(), Some(&SPIRV_MAGIC));
    let relinked = entry_point
        .link()
        .expect("the entry point links on its own")
        .into_value();
    drop(module);
    drop(program);
    drop(entry_point);
    let code = relinked
        .entry_point_code(0, target)
        .expect("the program keeps its session")
        .into_value();
    assert_eq!(words(code.as_bytes()).first(), Some(&SPIRV_MAGIC));
}

#[test]
fn modules_report_their_names_paths_entry_points_and_files() {
    let dir = scratch_dir("module_files");
    fs::write(dir.join("kernel.slang"), KERNEL).expect("the file is written");
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv))
        .search_path(&dir)
        .build()
        .expect("Slang creates the session");
    let module = session
        .load_module("kernel")
        .expect("the file compiles")
        .into_value();
    assert_eq!(module.name().expect("UTF-8"), Some("kernel"));
    let path = module.file_path().expect("UTF-8").expect("a file path");
    assert!(path.ends_with("kernel.slang"), "{path}");
    let entry_points = module
        .entry_points()
        .expect("Slang counts the entry points")
        .collect::<slang_compiler::Result<Vec<_>>>()
        .expect("each entry point is returned");
    assert_eq!(entry_points.len(), 1);
    let files = module
        .dependency_file_paths()
        .expect("Slang counts the files")
        .collect::<slang_compiler::Result<Vec<_>>>()
        .expect("each path is UTF-8");
    assert!(
        files
            .iter()
            .flatten()
            .any(|file| file.ends_with("kernel.slang")),
        "{files:?}"
    );
    let loaded = session
        .loaded_modules()
        .expect("Slang counts the modules")
        .filter(|loaded| loaded.as_raw() == module.as_raw())
        .count();
    assert_eq!(loaded, 1);
    let error = module
        .entry_point("absent")
        .expect_err("no such entry point");
    assert_eq!(error.kind(), ErrorKind::NotFound);
    assert_eq!(error.operation(), Operation::FindEntryPoint);
}

#[test]
fn a_serialized_module_loads_in_another_session() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let first = spirv_session(&global);
    let ir = first
        .load_module_from_source("kernel", "kernel.slang", KERNEL)
        .expect("the kernel compiles")
        .into_value()
        .serialize()
        .expect("Slang serializes the module");
    assert_ne!(ir.as_bytes(), []);
    let second = spirv_session(&global);
    let module = second
        .load_module_from_ir("kernel", "kernel.slang-module", ir.as_bytes())
        .expect("Slang loads the IR")
        .into_value();
    let entry_point = module
        .entry_point("main")
        .expect("the IR holds the entry point");
    let program = second
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])
        .expect("the parts compose")
        .into_value()
        .link()
        .expect("the composition links")
        .into_value();
    let code = program
        .entry_point_code(0, second.target_index(0).expect("a target"))
        .expect("Slang generates code from the IR")
        .into_value();
    assert_eq!(words(code.as_bytes()).first(), Some(&SPIRV_MAGIC));
    let error = second
        .load_module_from_ir("garbage", "garbage.slang-module", b"not a module")
        .expect_err("Slang rejects bytes that are no module");
    assert_eq!(error.kind(), ErrorKind::NotFound, "{error}");
    assert!(error.diagnostics().is_empty(), "{error}");
    assert!(
        error.to_string().contains("the bytes are not a module"),
        "{error}"
    );
}

#[test]
fn link_returns_its_input_and_link_with_options_changes_that_input() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let target = session.target_index(0).expect("a target");
    let source = r#"
        struct Item { float a; float3 b[2]; };
        RWStructuredBuffer<Item> items;
        [shader("compute")] [numthreads(1, 1, 1)]
        void main() { items[0].a = items[1].b[1].z; }
    "#;
    let compose = |name: &str| {
        let module = session
            .load_module_from_source(name, &format!("{name}.slang"), source)
            .expect("the source compiles")
            .into_value();
        let entry_point = module.entry_point("main").expect("the entry point exists");
        session
            .compose(&[
                Component::Module(&module),
                Component::EntryPoint(&entry_point),
            ])
            .expect("the parts compose")
            .into_value()
    };
    let plain = compose("plain");
    let linked = plain.link().expect("the composition links").into_value();
    assert_eq!(linked.as_raw(), plain.as_raw());
    let strides = |program: &LinkedProgram| {
        let code = program
            .entry_point_code(0, target)
            .expect("Slang generates code")
            .into_value();
        support::decorations(&words(code.as_bytes()), support::DECORATION_ARRAY_STRIDE)
    };
    assert!(strides(&linked).contains(&16));
    let composed = compose("scalar");
    let mut options = CompilerOptions::default();
    options.glsl_force_scalar_layout(Toggle::On);
    let with_options = composed
        .link_with_options(&options)
        .expect("the composition links")
        .into_value();
    assert_eq!(with_options.as_raw(), composed.as_raw());
    let again = composed.link().expect("the composition links").into_value();
    assert!(strides(&again).contains(&12), "{:?}", strides(&again));
}

#[test]
fn metadata_reports_used_bindings_and_a_failed_generation_is_a_compile_error() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let target = session.target_index(0).expect("a target");
    let (_module, program) = link_kernel(&session);
    let metadata = program
        .entry_point_metadata(0, target)
        .expect("Slang returns metadata")
        .into_value();
    let used = |index| {
        metadata
            .is_parameter_location_used(
                ParameterCategory::DescriptorTableSlot,
                BindingSpace::new(0),
                BindingIndex::new(index),
            )
            .expect("Slang answers")
    };
    assert!(used(0));
    assert!(!used(7));
    let whole = program
        .target_metadata(target)
        .expect("Slang returns metadata of the whole program")
        .into_value();
    assert!(
        whole
            .is_parameter_location_used(
                ParameterCategory::DescriptorTableSlot,
                BindingSpace::new(0),
                BindingIndex::new(0),
            )
            .expect("Slang answers")
    );

    let failing = r#"
        RWStructuredBuffer<float> data;
        float down(float x) { return x < 1.0 ? x : down(x - 1.0); }
        [shader("compute")] [numthreads(1, 1, 1)]
        void main() { data[0] = down(data[1]); }
    "#;
    let module = session
        .load_module_from_source("failing", "failing.slang", failing)
        .expect("the front end accepts the source")
        .into_value();
    let entry_point = module.entry_point("main").expect("the entry point exists");
    let program = session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])
        .expect("the parts compose")
        .into_value()
        .link()
        .expect("the composition links")
        .into_value();
    let error = program
        .entry_point_metadata(0, target)
        .expect_err("code generation fails");
    assert_eq!(error.kind(), ErrorKind::Compile, "{error}");
    assert_eq!(error.operation(), Operation::ReadMetadata);
    assert_ne!(error.diagnostics().as_str(), "");
}

#[test]
fn specialize_fills_a_generic_parameter_and_reports_wrong_arguments() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let target = session.target_index(0).expect("a target");
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
    assert_eq!(composed.specialization_param_count().expect("a count"), 1);
    let one = find_type(&session, &module, "One").expect("the module defines One");
    let plain = find_type(&session, &module, "Plain").expect("the module defines Plain");
    let program = composed
        .specialize(&[SpecializationArg::Type(one)])
        .expect("One implements IValue")
        .into_value()
        .link()
        .expect("the specialization links")
        .into_value();
    let code = program
        .entry_point_code(0, target)
        .expect("Slang generates code")
        .into_value();
    assert_eq!(words(code.as_bytes()).first(), Some(&SPIRV_MAGIC));

    let error = composed
        .specialize(&[SpecializationArg::Type(plain)])
        .expect_err("Plain does not implement IValue");
    assert_eq!(error.kind(), ErrorKind::Compile);
    assert!(has_code(error.diagnostics(), 38029), "{error}");
    let error = composed
        .specialize(&[SpecializationArg::Type(one), SpecializationArg::Type(one)])
        .expect_err("one argument too many");
    assert_eq!(error.kind(), ErrorKind::Compile);
    assert!(has_code(error.diagnostics(), 38025), "{error}");
    let error = composed
        .link()
        .expect("an unspecialized composition links")
        .into_value()
        .entry_point_code(0, target)
        .expect_err("an unspecialized program generates no code");
    assert_eq!(error.kind(), ErrorKind::Compile, "{error}");
    assert_eq!(error.code(), Some(SlangResult::SLANG_FAIL.0), "{error}");
    assert!(has_code(error.diagnostics(), 99997), "{error}");
}

#[test]
fn a_type_conformance_composes_and_a_non_conforming_type_fails() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let module = session
        .load_module_from_source("generic", "generic.slang", GENERIC)
        .expect("the source compiles")
        .into_value();
    let find = |name| find_type(&session, &module, name).expect("the module defines the type");
    let conformance = session
        .type_conformance(find("One"), find("IValue"), Some(3))
        .expect("One implements IValue")
        .into_value();
    let entry_point = module.entry_point("main").expect("the entry point exists");
    let composed = session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
            Component::TypeConformance(&conformance),
        ])
        .expect("the parts compose")
        .into_value();
    assert_eq!(composed.entry_point_count(), 1);
    let linked = conformance
        .link()
        .expect("a type conformance links on its own")
        .into_value();
    assert_eq!(linked.entry_point_count(), 0);
    let error = session
        .type_conformance(find("Plain"), find("IValue"), None)
        .expect_err("Plain does not implement IValue");
    println!("{error}");
    assert_eq!(error.operation(), Operation::CreateTypeConformance);
    assert_eq!(error.kind(), ErrorKind::Internal, "{error}");
    let error = session
        .type_conformance(find("One"), find("IValue"), Some(-1))
        .expect_err("a negative identifier is rejected");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
}

#[test]
fn every_method_that_takes_an_object_rejects_one_of_another_session() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let other = spirv_session(&global);
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
    let foreign = other
        .load_module_from_source("generic", "generic.slang", GENERIC)
        .expect("the source compiles")
        .into_value();
    let mine = |name| find_type(&session, &module, name).expect("the module defines the type");
    let theirs = |name| find_type(&other, &foreign, name).expect("the module defines the type");
    let errors = [
        other
            .compose(&[Component::EntryPoint(&entry_point)])
            .map(drop)
            .expect_err("the entry point belongs to the first session"),
        composed
            .specialize(&[SpecializationArg::Type(theirs("One"))])
            .map(drop)
            .expect_err("the type belongs to the second session"),
        session
            .type_conformance(theirs("One"), mine("IValue"), None)
            .map(drop)
            .expect_err("the type belongs to the second session"),
        session
            .type_conformance(mine("One"), theirs("IValue"), None)
            .map(drop)
            .expect_err("the interface belongs to the second session"),
    ];
    for error in errors {
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{error}");
        assert!(
            error.to_string().contains("belongs to another session"),
            "{error}"
        );
    }
}

#[test]
fn every_call_that_takes_a_target_index_checks_it_against_the_session_again() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let wider = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv))
        .target(Target::new(TargetFormat::Hlsl))
        .build()
        .expect("Slang creates the session");
    let past = wider.target_index(1).expect("two targets");
    let (_module, program) = link_kernel(&session);
    let target = session.target_index(0).expect("a target");
    let errors = [
        program.entry_point_code(0, past).map(drop),
        program.target_code(past).map(drop),
        program.entry_point_metadata(0, past).map(drop),
        program.target_metadata(past).map(drop),
        program.entry_point_code(1, target).map(drop),
        program.entry_point_metadata(1, target).map(drop),
    ];
    for result in errors {
        let error = result.expect_err("the index is out of range");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{error}");
        assert_eq!(error.code(), None);
    }
}

#[test]
fn blobs_and_metadata_stay_valid_after_every_other_object_is_released() {
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    let _poisoned = super::two_global_sessions::FreedBlocksPoisoned::set();
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let target = session.target_index(0).expect("a target");
    let (module, program) = link_kernel(&session);
    let code = program
        .entry_point_code(0, target)
        .expect("Slang generates code")
        .into_value();
    let metadata = program
        .entry_point_metadata(0, target)
        .expect("Slang returns metadata")
        .into_value();
    let copy = code.to_vec();
    let (raw_code, code_session) = code.into_raw();
    let (raw_metadata, metadata_session) = metadata.into_raw();
    drop(module);
    drop(program);
    drop(session);
    drop(code_session);
    drop(metadata_session);
    drop(global);
    let _churn = super::two_global_sessions::churn_the_heap();
    // SAFETY: `raw_code` carries the reference that `into_raw` gave up, and
    // this test reads the blob through it on this thread, which is what CA-3
    // of `docs/design/slang-compiler.md` asserts stays valid.
    let data = unsafe { ISlangBlob::getBufferPointer(raw_code.as_ptr()) };
    // SAFETY: as above.
    let size = unsafe { ISlangBlob::getBufferSize(raw_code.as_ptr()) };
    assert_eq!(size, copy.len());
    assert!(!data.is_null());
    // SAFETY: Slang reports `size` bytes at `data`, which the blob keeps until
    // the release below.
    let bytes = unsafe { slice::from_raw_parts(data.cast::<u8>(), size) };
    assert_eq!(bytes, copy.as_slice());
    let mut used = false;
    // SAFETY: `raw_metadata` carries the reference that `into_raw` gave up,
    // and `used` is a live out-pointer.
    let result = unsafe {
        IMetadata::isParameterLocationUsed(
            raw_metadata.as_ptr(),
            slang_compiler_sys::SlangParameterCategory::SLANG_PARAMETER_CATEGORY_DESCRIPTOR_TABLE_SLOT,
            0,
            0,
            &raw mut used,
        )
    };
    assert!(result.0 >= 0);
    assert!(used);
    // SAFETY: the test owns the one reference of each, which it releases once.
    unsafe {
        ISlangBlob::release(raw_code.as_ptr());
    }
    // SAFETY: as above.
    unsafe {
        IMetadata::release(raw_metadata.as_ptr());
    }
}

#[test]
fn code_of_a_compute_kernel_is_spirv_with_its_entry_point() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let words = compile_spirv(&session, "kernel", KERNEL, 0);
    assert_eq!(entry_point_names(&words), ["main"]);
    assert!(support::count(&words, support::OP_VARIABLE) > 0);
}

#[test]
fn entry_point_with_stage_checks_a_function_without_a_shader_attribute() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv))
        .build()
        .expect("Slang creates the session");
    let target = session.target_index(0).expect("a target");
    let module = session
        .load_module_from_source(
            "plain",
            "plain.slang",
            "RWStructuredBuffer<uint> data;\n[numthreads(1, 1, 1)]\nvoid double_it(uint3 id : SV_DispatchThreadID) { data[id.x] *= 2; }\n",
        )
        .expect("the source compiles")
        .into_value();
    let entry_point = module
        .entry_point_with_stage("double_it", Stage::Compute)
        .expect("the function is a valid compute entry point")
        .into_value();
    let code = entry_point
        .link()
        .expect("the entry point links")
        .into_value()
        .entry_point_code(0, target)
        .expect("Slang generates code")
        .into_value();
    let words = words(code.as_bytes());
    assert_eq!(words.first(), Some(&SPIRV_MAGIC));
    assert_eq!(entry_point_names(&words), ["main"]);

    let error = module
        .entry_point_with_stage("missing", Stage::Compute)
        .expect_err("the module has no such function");
    assert_eq!(error.kind(), ErrorKind::Compile, "{error}");
    assert_eq!(error.operation(), Operation::FindEntryPoint);
    assert_eq!(error.code(), Some(SlangResult::SLANG_FAIL.0), "{error}");
    assert!(has_code(error.diagnostics(), 38000), "{error}");
    assert!(error.to_string().contains("`missing`"), "{error}");
}
