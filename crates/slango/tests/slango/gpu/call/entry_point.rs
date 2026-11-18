use slango::Buffer;

use super::device;
use crate::common::kernel;

const ENTRY_BUFFER: &str = r#"
[shader("compute")]
[numthreads(64, 1, 1)]
void scale_in_place(uint3 id : SV_DispatchThreadID, RWStructuredBuffer<float> values, uniform float factor, uniform uint count)
{
    if (id.x < count) { values[id.x] = values[id.x] * factor; }
}
"#;

#[test]
fn a_structured_buffer_among_the_parameters_of_the_entry_point_reaches_the_kernel()
-> slango::Result<()> {
    let device = device();
    let values = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
    kernel(&device, ENTRY_BUFFER, "scale_in_place")
        .call()
        .bind("values", &values)
        .set("factor", 2.5_f32)
        .set("count", 3_u32)
        .dispatch([4, 1, 1])?;
    assert_eq!(values.read()?, [2.5, 5.0, 7.5, 4.0]);
    Ok(())
}

const ENTRY_CONSTANT_BUFFER: &str = r#"
struct Params { float scale; float3 offset; }

RWStructuredBuffer<float> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void read_constants(uniform ConstantBuffer<Params> constants)
{
    output[0] = constants.scale;
    output[1] = constants.offset.x;
    output[2] = constants.offset.y;
    output[3] = constants.offset.z;
}
"#;

#[test]
fn a_constant_buffer_among_the_parameters_of_the_entry_point_reaches_the_kernel()
-> slango::Result<()> {
    let device = device();
    let output = Buffer::<f32>::zeroed(&device, 4)?;
    kernel(&device, ENTRY_CONSTANT_BUFFER, "read_constants")
        .call()
        .set("constants.scale", 1.5_f32)
        .set("constants.offset", [2.0_f32, 3.0, 4.0])
        .bind("output", &output)
        .dispatch([1, 1, 1])?;
    assert_eq!(output.read()?, [1.5, 2.0, 3.0, 4.0]);
    Ok(())
}

const ENTRY_PARAMETER_BLOCK: &str = r#"
struct Extra { float bias; RWStructuredBuffer<float> output; }

StructuredBuffer<float> input;

[shader("compute")]
[numthreads(64, 1, 1)]
void add_bias(uint3 id : SV_DispatchThreadID, uniform ParameterBlock<Extra> extra, uniform uint count)
{
    if (id.x < count) { extra.output[id.x] = input[id.x] + extra.bias; }
}
"#;

#[test]
fn a_parameter_block_with_a_buffer_among_the_parameters_of_the_entry_point_reaches_the_kernel()
-> slango::Result<()> {
    let device = device();
    let input = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0])?;
    let output = Buffer::from_slice(&device, &[-1.0_f32; 3])?;
    kernel(&device, ENTRY_PARAMETER_BLOCK, "add_bias")
        .call()
        .bind("input", &input)
        .set("extra.bias", 0.5_f32)
        .bind("extra.output", &output)
        .set("count", 2_u32)
        .dispatch([3, 1, 1])?;
    assert_eq!(output.read()?, [1.5, 2.5, -1.0]);
    Ok(())
}
