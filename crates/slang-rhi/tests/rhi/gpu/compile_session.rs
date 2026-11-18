//! The tests of `Device::create_slang_session` and `CompileSession`, among
//! them the comparison of the layouts and code of a compile session with
//! those of the session of the device.

use std::path::{Path, PathBuf};

use slang_compiler::{Component, LinkedProgram, Session, Target, Toggle};
use slang_rhi::{CompileSession, Device, ErrorKind, Operation};

use crate::common::{DOUBLE, TARGET_FORMAT, float_buffer, link, open, options, read};

/// The directory of the test shaders of `slango`.
fn slango_shaders() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../slango/tests/slango/shaders")
}

/// A kernel whose code depends on the macro of the backend, and whose
/// layouts depend on the matrix layout and the layout of buffers. It has a
/// `ConstantBuffer` and no global `uniform`.
const SETTINGS: &str = r#"
struct Transform
{
    float3x4 matrix;
    float2 pad;
};
ConstantBuffer<Transform> transform;
ParameterBlock<Transform> block;
RWStructuredBuffer<float> values;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
#if defined(__VULKAN__) || defined(__D3D12__)
    float marker = 1.0;
#else
    float marker = 2.0;
#endif
    values[0] = mul(transform.matrix, float4(1.0, 2.0, 3.0, 4.0)).x + block.matrix[1][2] + marker;
    values[1] = marker;
}
"#;

/// One module to load: its name, its path, and its source.
type Source = (&'static str, PathBuf, String);

/// Returns the modules of each test shader of `slango`, each after the
/// modules that it imports, and the kernel `SETTINGS`.
fn shaders() -> Vec<Vec<Source>> {
    let directory = slango_shaders();
    let file = |name: &'static str| {
        let path = directory.join(format!("{name}.slang"));
        let source = std::fs::read_to_string(&path).expect("the test shader of slango exists");
        (name, path, source)
    };
    vec![
        vec![file("scale")],
        vec![file("particles")],
        vec![file("quad")],
        vec![file("helpers"), file("two_kernels")],
        vec![(
            "settings",
            PathBuf::from("settings.slang"),
            String::from(SETTINGS),
        )],
    ]
}

/// The reflected layout and the code of each entry point of a program.
#[derive(Debug, PartialEq, Eq)]
struct Compiled {
    layout: String,
    code: Vec<Vec<u8>>,
}

/// Loads `modules` into `session`, links the last one with every entry point
/// that it defines, and returns its layout as JSON and the code of each entry
/// point for the one target of the session.
fn compile(session: &Session, modules: &[Source]) -> slang_compiler::Result<Compiled> {
    let mut loaded = Vec::new();
    for (name, path, source) in modules {
        loaded.push(
            session
                .load_module_from_source(name, path, source)?
                .into_value(),
        );
    }
    let module = loaded.last().expect("each shader has a module");
    let entry_points = module
        .entry_points()?
        .collect::<slang_compiler::Result<Vec<_>>>()?;
    let mut components = vec![Component::Module(module)];
    components.extend(entry_points.iter().map(Component::EntryPoint));
    let linked = session
        .compose(&components)?
        .into_value()
        .link()?
        .into_value();
    let target = session.target_index(0)?;
    let layout = linked.layout(target)?.into_value().to_json()?;
    let mut code = Vec::new();
    for index in 0..linked.entry_point_count() {
        code.push(
            linked
                .entry_point_code(index, target)?
                .into_value()
                .as_bytes()
                .to_vec(),
        );
    }
    Ok(Compiled { layout, code })
}

#[test]
fn a_compile_session_gives_the_layouts_and_code_of_the_session_of_the_device() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let compile_session = device
        .create_slang_session(&[])
        .expect("the crate creates a compile session");
    for modules in shaders() {
        let name = modules.last().map(|(name, _, _)| *name);
        let of_device = compile(device.slang_session(), &modules).expect("the device compiles it");
        let of_compile =
            compile(compile_session.session(), &modules).expect("the compile session compiles it");
        assert!(
            !of_device.code.is_empty() && of_device.code.iter().all(|code| !code.is_empty()),
            "{name:?}"
        );
        assert_eq!(
            of_device.layout, of_compile.layout,
            "the layouts of {name:?} differ"
        );
        assert_eq!(
            of_device.code, of_compile.code,
            "the code of {name:?} differs"
        );
    }
    assert_eq!(messages.errors(), []);
}

#[test]
fn the_comparison_sees_a_session_without_the_settings_of_the_device() {
    let (options, _) = options(Toggle::Off);
    let device = open(&options);
    let plain = Session::builder(device.slang_session().global_session())
        .target(Target::new(TARGET_FORMAT))
        .build()
        .expect("Slang creates a session");
    let modules = shaders().pop().expect("the kernel SETTINGS");
    let of_device = compile(device.slang_session(), &modules).expect("the device compiles it");
    let of_plain = compile(&plain, &modules);
    assert_ne!(of_plain.ok(), Some(of_device));
}

/// Loads the module `name` of `session` from its search paths and links it
/// with its entry point `entry`.
fn link_from_search_path(session: &Session, name: &str, entry: &str) -> LinkedProgram {
    let module = session
        .load_module(name)
        .expect("the module is on a search path")
        .into_value();
    let entry_point = module.entry_point(entry).expect("the entry point exists");
    session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])
        .expect("the parts compose")
        .into_value()
        .link()
        .expect("the program links")
        .into_value()
}

#[test]
fn a_shader_program_of_a_compile_session_runs_after_the_compile_session_is_dropped() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let shaders = slango_shaders();
    let compile_session: CompileSession = device
        .create_slang_session(&[shaders.as_path()])
        .expect("the crate creates a compile session");
    let linked = link_from_search_path(compile_session.session(), "two_kernels", "double_each");
    let program = compile_session
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    drop(linked);
    drop(compile_session);
    let pipeline = device
        .create_compute_pipeline(&program)
        .expect("slang-rhi creates the compute pipeline");
    drop(program);
    let data = float_buffer(&device, &[1.0, 2.0, 3.0, 4.0]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let target = root.offset_of("data")?.expect("data");
            root.set_buffer(target, &data)?;
            let count = root.offset_of("count")?.expect("count");
            root.set_data(count, &3_u32.to_le_bytes())?;
            drop(root);
            pass.dispatch([1, 1, 1])
        })
        .expect("the pass records");
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_eq!(read(&device, &data), [2.0, 4.0, 6.0, 4.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_program_of_a_compile_session_sees_the_macro_of_the_backend_when_it_runs() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let compile_session = device
        .create_slang_session(&[])
        .expect("the crate creates a compile session");
    let linked = link(compile_session.session(), "settings", SETTINGS);
    let program = compile_session
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    let pipeline = device
        .create_compute_pipeline(&program)
        .expect("slang-rhi creates the compute pipeline");
    let values = float_buffer(&device, &[0.0, 0.0]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let target = root.offset_of("values")?.expect("values");
            root.set_buffer(target, &values)?;
            drop(root);
            pass.dispatch([1, 1, 1])
        })
        .expect("the pass records");
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_eq!(read(&device, &values)[1], 1.0);
    assert_eq!(messages.errors(), []);
}

#[test]
fn each_create_shader_program_refuses_a_program_of_another_session() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let first = device
        .create_slang_session(&[])
        .expect("the crate creates a compile session");
    let second = device
        .create_slang_session(&[])
        .expect("the crate creates a compile session");
    let of_device = link(device.slang_session(), "double", DOUBLE);
    let of_first = link(first.session(), "double", DOUBLE);
    let of_second = link(second.session(), "double", DOUBLE);
    let refusals = [
        device.create_shader_program(&of_first).map(drop),
        first.create_shader_program(&of_device).map(drop),
        first.create_shader_program(&of_second).map(drop),
    ];
    for refusal in refusals {
        let error = refusal.expect_err("the program belongs to another session");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::CreateShaderProgram);
    }
    first
        .create_shader_program(&of_first)
        .expect("the compile session takes its own program");
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_search_path_with_a_nul_byte_fails_with_the_error_of_slang_compiler() {
    let (options, _) = options(Toggle::Off);
    let device: Device = open(&options);
    let error = device
        .create_slang_session(&[Path::new("bad\0path")])
        .expect_err("a NUL byte is refused");
    assert_eq!(error.kind(), ErrorKind::Slang);
    assert_eq!(error.operation(), Operation::CreateSlangSession);
}
