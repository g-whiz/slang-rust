#[cfg(feature = "wgpu")]
use slango::Buffer;
#[cfg(feature = "rhi")]
use slango::ErrorKind;

use super::device;

const BUFFER_IN_A_STRUCTURE: &str = r#"
struct Target { float value; RWStructuredBuffer<float> output; }

Target target;

[shader("compute")]
[numthreads(1, 1, 1)]
void write_value()
{
    target.output[0] = target.value;
}
"#;

const CONSTANT_BUFFER_OF_A_VECTOR: &str = r#"
ConstantBuffer<float4> tint;
RWStructuredBuffer<float> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void read_tint()
{
    output[0] = tint.x;
    output[1] = tint.w;
}
"#;

#[cfg(feature = "wgpu")]
#[test]
fn a_buffer_in_a_plain_structure_reaches_the_kernel_on_wgpu() -> slango::Result<()> {
    let device = device();
    let output = Buffer::<f32>::zeroed(&device, 1)?;
    device
        .load_module_from_source("structures", BUFFER_IN_A_STRUCTURE)?
        .kernel("write_value")?
        .call()
        .set("target.value", 6.5_f32)
        .bind("target.output", &output)
        .dispatch([1, 1, 1])?;
    assert_eq!(output.read()?, [6.5]);
    Ok(())
}

#[cfg(feature = "rhi")]
#[test]
fn a_buffer_in_a_plain_structure_is_refused_at_load_on_rhi() {
    let error = device()
        .load_module_from_source("structures", BUFFER_IN_A_STRUCTURE)
        .expect_err("the backend rhi cannot address the buffer");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter, "{error}");
    assert!(
        error.to_string().contains(
            "the parameter `target.output` of the kernel `write_value` is a resource inside a structure"
        ),
        "{error}"
    );
}

#[cfg(feature = "wgpu")]
#[test]
fn a_constant_buffer_of_a_vector_reaches_the_kernel_on_wgpu() -> slango::Result<()> {
    let device = device();
    let output = Buffer::<f32>::zeroed(&device, 2)?;
    device
        .load_module_from_source("vector_constants", CONSTANT_BUFFER_OF_A_VECTOR)?
        .kernel("read_tint")?
        .call()
        .set("tint", [1.0_f32, 2.0, 3.0, 4.0])
        .bind("output", &output)
        .dispatch([1, 1, 1])?;
    assert_eq!(output.read()?, [1.0, 4.0]);
    Ok(())
}

#[cfg(feature = "rhi")]
#[test]
fn a_constant_buffer_of_a_vector_is_refused_at_load_on_rhi() {
    let error = device()
        .load_module_from_source("vector_constants", CONSTANT_BUFFER_OF_A_VECTOR)
        .expect_err("the backend rhi cannot address the vector");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter, "{error}");
    assert!(
        error.to_string().contains(
            "the parameter `tint` of the kernel `read_tint` is a constant buffer whose element is not a structure"
        ),
        "{error}"
    );
}
