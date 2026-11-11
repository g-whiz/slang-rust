use std::error::Error as _;
use std::path::PathBuf;

use slango::{AdapterSelection, Device, ErrorKind};

use crate::common::{device_with, device_without_immediates, load};

fn shader(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("slango")
        .join("shaders")
        .join(name)
}

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

#[test]
fn a_module_file_with_two_kernels_and_an_import_loads_both_by_name() {
    let module = device()
        .load_module(shader("two_kernels.slang"))
        .expect("the module loads");
    let scale = module.kernel("scale").expect("the kernel `scale`");
    assert_eq!(scale.name(), "scale");
    assert_eq!(scale.thread_group_size(), [64, 1, 1]);
    let double_each = module
        .clone()
        .kernel("double_each")
        .expect("the kernel `double_each`")
        .clone();
    assert_eq!(double_each.name(), "double_each");
    assert_eq!(double_each.thread_group_size(), [8, 4, 2]);
}

#[test]
fn a_kernel_with_buffers_a_global_uniform_a_constant_buffer_and_a_parameter_block_loads() {
    let module = load(
        &device(),
        r#"
struct Params { float scale; float3 offset; uint count; float2 pair; }
struct Extra { float bias; RWStructuredBuffer<float> output; }

StructuredBuffer<float4> input;
RWStructuredBuffer<float> data;
ConstantBuffer<Params> params;
ParameterBlock<Extra> extra;
uniform float factor;

[shader("compute")]
[numthreads(64, 1, 1)]
void combine(uint3 id : SV_DispatchThreadID)
{
    if (id.x < params.count)
    {
        float value = input[id.x].x * params.scale + params.offset.y + params.pair.x;
        data[id.x] = value * factor;
        extra.output[id.x] = value + extra.bias;
    }
}
"#,
    );
    assert_eq!(
        module.kernel("combine").expect("the kernel").name(),
        "combine"
    );
}

#[test]
fn a_kernel_with_unused_parameters_loads() {
    let module = load(
        &device(),
        r#"
RWStructuredBuffer<float> data;
RWStructuredBuffer<float> never_used;
uniform float factor;
uniform float never_read;

[shader("compute")]
[numthreads(64, 1, 1)]
void scale(uint3 id : SV_DispatchThreadID) { data[id.x] *= factor; }
"#,
    );
    module.kernel("scale").expect("the kernel");
}

const ENTRY_POINT_UNIFORMS: &str = r#"
struct Run { float gain; uint width; }
RWStructuredBuffer<float> data;

[shader("compute")]
[numthreads(64, 1, 1)]
void amplify(uint3 id : SV_DispatchThreadID, uniform float factor, uniform Run run)
{
    if (id.x < run.width) { data[id.x] *= factor * run.gain; }
}
"#;

#[test]
fn a_kernel_with_uniform_parameters_of_its_entry_point_loads_with_immediates() {
    let device = device();
    assert!(
        slango::wgpu::device(&device)
            .features()
            .contains(wgpu::Features::IMMEDIATES),
        "the adapter offers immediates"
    );
    let module = load(&device, ENTRY_POINT_UNIFORMS);
    module.kernel("amplify").expect("the kernel");
}

#[test]
fn a_kernel_with_uniform_parameters_of_its_entry_point_needs_immediates() {
    let error = device_without_immediates()
        .load_module_from_source("kernels", ENTRY_POINT_UNIFORMS)
        .expect_err("the device lacks immediates");
    assert_eq!(error.kind(), ErrorKind::MissingFeature, "{error}");
    assert!(error.to_string().contains("`amplify`"), "{error}");
}

#[test]
fn uniform_parameters_of_an_entry_point_over_the_immediate_limit_are_a_missing_feature() {
    let device = device();
    let wgpu_device = slango::wgpu::device(&device);
    let has_immediates = wgpu_device.features().contains(wgpu::Features::IMMEDIATES);
    let limit = wgpu_device.limits().max_immediate_size;
    let error = device
        .load_module_from_source(
            "kernels",
            r#"
RWStructuredBuffer<float4> data;

[shader("compute")]
[numthreads(64, 1, 1)]
void sum(uint3 id : SV_DispatchThreadID, uniform float4 values[256])
{
    data[id.x] = values[id.x % 256];
}
"#,
        )
        .expect_err("4096 bytes of immediates exceed every adapter's limit");
    assert_eq!(error.kind(), ErrorKind::MissingFeature, "{error}");
    assert!(error.to_string().contains("`sum`"), "{error}");
    if has_immediates {
        assert!(
            error.to_string().ends_with(&format!(
                "bytes of immediates, and the device allows {limit}"
            )),
            "{error}"
        );
    }
    println!("the device allows {limit} bytes of immediates: {error}");
}

#[cfg(unix)]
#[test]
fn a_path_that_is_not_utf8_is_an_invalid_argument() {
    use std::os::unix::ffi::OsStrExt as _;

    let path = std::env::temp_dir().join(std::ffi::OsStr::from_bytes(b"\xff.slang"));
    let error = device()
        .load_module(&path)
        .expect_err("the path is not UTF-8");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        format!(
            "cannot read a Slang file: the path `{}` is not valid UTF-8, which Slang needs",
            path.display()
        )
    );
}

#[test]
fn an_unknown_kernel_name_is_an_invalid_argument() {
    let module = device()
        .load_module(shader("two_kernels.slang"))
        .expect("the module loads");
    let error = module
        .kernel("missing")
        .expect_err("no kernel has the name");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot find a kernel: the module has no compute entry point named `missing`"
    );
}

#[test]
fn an_unreadable_file_is_an_invalid_argument() {
    let path = shader("no_such_file.slang");
    let error = device()
        .load_module(&path)
        .expect_err("the file does not exist");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        format!("cannot read a Slang file: `{}`", path.display())
    );
    assert!(
        error
            .source()
            .is_some_and(<dyn std::error::Error>::is::<std::io::Error>)
    );
}

#[test]
fn source_that_does_not_compile_is_a_compile_error_with_slangs_diagnostics() {
    let error = device()
        .load_module_from_source("broken", "void run() { undefined_name = 1; }")
        .expect_err("the source does not compile");
    assert_eq!(error.kind(), ErrorKind::Compile);
    let source = error
        .source()
        .and_then(|source| source.downcast_ref::<slang_compiler::Error>())
        .expect("the error of slang-compiler as the source");
    assert!(
        source.diagnostics().as_str().contains("undefined_name"),
        "{}",
        source.diagnostics()
    );
}

#[test]
fn a_shader_that_the_backend_rejects_is_unsupported_with_the_backends_text() {
    let error = device()
        .load_module_from_source(
            "split",
            r#"
RWStructuredBuffer<float> data;

[shader("compute")]
[numthreads(1, 1, 1)]
void split()
{
    int exponent;
    data[0] = frexp(data[0], exponent);
    data[1] = float(exponent);
}
"#,
        )
        .expect_err("naga rejects frexp");
    assert_eq!(error.kind(), ErrorKind::UnsupportedShader, "{error}");
    assert!(error.to_string().contains("`split`"), "{error}");
    let text = error
        .source()
        .map(ToString::to_string)
        .expect("the error of the backend as the source");
    println!("the backend's text: {text}");
    assert_ne!(text.trim(), "");
}

#[test]
fn a_kernel_with_a_texture_is_an_unsupported_parameter() {
    let error = device()
        .load_module_from_source(
            "textured",
            r#"
Texture2D<float4> image;
RWStructuredBuffer<float4> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void sample() { output[0] = image.Load(int3(0, 0, 0)); }
"#,
        )
        .expect_err("the walk rejects textures");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter, "{error}");
    assert!(error.to_string().contains("`image`"), "{error}");
}
