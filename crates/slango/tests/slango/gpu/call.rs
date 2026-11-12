use std::fmt::Debug;
use std::thread;

use slango::{AdapterSelection, Buffer, Device, Element};

use crate::common::{device_with, kernel, load, shader};

mod elements;
mod validation;

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

#[test]
fn the_usage_illustration_of_the_design_scales_a_buffer() -> slango::Result<()> {
    let device = device();
    let module = device.load_module(shader("scale.slang"))?;
    let scale = module.kernel("scale")?;
    let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
    scale
        .call()
        .set("factor", 2.0_f32)
        .set("count", 4_u32)
        .bind("data", &data)
        .dispatch([4, 1, 1])?;
    let result: Vec<f32> = data.read()?;
    assert_eq!(result, [2.0, 4.0, 6.0, 8.0]);
    Ok(())
}

#[test]
fn a_call_runs_again_with_the_same_arguments() -> slango::Result<()> {
    let device = device();
    let module = device.load_module(shader("scale.slang"))?;
    let data = Buffer::from_slice(&device, &[1.0_f32, -2.0, 3.0])?;
    let mut call = module.kernel("scale")?.call();
    call.set("factor", 2.0_f32)
        .set("count", 2_u32)
        .bind("data", &data);
    call.dispatch([3, 1, 1])?;
    call.set("factor", 10.0_f32);
    call.dispatch([3, 1, 1])?;
    assert_eq!(data.read()?, [20.0, -40.0, 3.0]);
    Ok(())
}

#[test]
fn a_later_argument_for_a_path_replaces_the_earlier_one_whatever_its_kind() -> slango::Result<()> {
    let device = device();
    let module = device.load_module(shader("scale.slang"))?;
    let earlier = Buffer::from_slice(&device, &[1.0_f32, 2.0])?;
    let later = Buffer::from_slice(&device, &[3.0_f32, 4.0])?;
    module
        .kernel("scale")?
        .call()
        .set("data", 1.0_f32)
        .bind("factor", &earlier)
        .set("count", 2_u32)
        .bind("data", &earlier)
        .set("factor", 3.0_f32)
        .bind("data", &later)
        .set("factor", 5.0_f32)
        .dispatch([2, 1, 1])?;
    assert_eq!(earlier.read()?, [1.0, 2.0]);
    assert_eq!(later.read()?, [15.0, 20.0]);
    Ok(())
}

const SHAPES: &str = r#"
RWStructuredBuffer<float> output;

cbuffer Constants
{
    float2x3 constant_matrix;
    float3 constant_points[2];
}

[shader("compute")]
[numthreads(1, 1, 1)]
void scalars_vectors_and_a_matrix(
    uniform float amount,
    uniform float3 direction,
    uniform float2x3 rows,
    uniform bool flag,
    uniform int integer)
{
    output[0] = amount;
    output[1] = direction.x;
    output[2] = direction.y;
    output[3] = direction.z;
    for (int r = 0; r < 2; r++)
    {
        for (int c = 0; c < 3; c++) { output[4 + r * 3 + c] = rows[r][c]; }
    }
    output[10] = flag ? 1.0 : -1.0;
    output[11] = float(integer);
    for (int r = 0; r < 2; r++)
    {
        for (int c = 0; c < 3; c++) { output[12 + r * 3 + c] = constant_matrix[r][c]; }
    }
    for (int p = 0; p < 2; p++)
    {
        output[18 + p * 3] = constant_points[p].x;
        output[19 + p * 3] = constant_points[p].y;
        output[20 + p * 3] = constant_points[p].z;
    }
}

[shader("compute")]
[numthreads(1, 1, 1)]
void points_and_a_tall_matrix(uniform float3 points[4], uniform float3x2 tall)
{
    for (int p = 0; p < 4; p++)
    {
        output[p * 3] = points[p].x;
        output[p * 3 + 1] = points[p].y;
        output[p * 3 + 2] = points[p].z;
    }
    for (int r = 0; r < 3; r++)
    {
        for (int c = 0; c < 2; c++) { output[12 + r * 2 + c] = tall[r][c]; }
    }
}
"#;

#[test]
fn immediates_and_a_constant_buffer_reach_the_kernel_element_by_element() -> slango::Result<()> {
    let device = device();
    let kernel = kernel(&device, SHAPES, "scalars_vectors_and_a_matrix");
    let output = Buffer::<f32>::zeroed(&device, 24)?;
    kernel
        .call()
        .set("amount", 0.5_f32)
        .set("direction", [1.0_f32, 2.0, 3.0])
        .set("rows", [[11.0_f32, 12.0, 13.0], [21.0, 22.0, 23.0]])
        .set("flag", true)
        .set("integer", -7_i32)
        .set(
            "Constants.constant_matrix",
            [[31.0_f32, 32.0, 33.0], [41.0, 42.0, 43.0]],
        )
        .set(
            "Constants.constant_points",
            [[51.0_f32, 52.0, 53.0], [61.0, 62.0, 63.0]],
        )
        .bind("output", &output)
        .dispatch([1, 1, 1])?;
    assert_eq!(
        output.read()?,
        [
            0.5, 1.0, 2.0, 3.0, 11.0, 12.0, 13.0, 21.0, 22.0, 23.0, 1.0, -7.0, 31.0, 32.0, 33.0,
            41.0, 42.0, 43.0, 51.0, 52.0, 53.0, 61.0, 62.0, 63.0,
        ]
    );
    Ok(())
}

#[test]
fn a_false_bool_reaches_the_kernel_as_false() -> slango::Result<()> {
    let device = device();
    let kernel = kernel(&device, SHAPES, "scalars_vectors_and_a_matrix");
    let output = Buffer::<f32>::zeroed(&device, 24)?;
    kernel
        .call()
        .set("amount", 0.0_f32)
        .set("direction", [0.0_f32; 3])
        .set("rows", [[0.0_f32; 3]; 2])
        .set("flag", false)
        .set("integer", 0_i32)
        .set("Constants.constant_matrix", [[0.0_f32; 3]; 2])
        .set("Constants.constant_points", [[0.0_f32; 3]; 2])
        .bind("output", &output)
        .dispatch([1, 1, 1])?;
    assert_eq!(output.read()?.get(10), Some(&-1.0));
    Ok(())
}

#[test]
fn an_array_of_vectors_and_a_matrix_of_three_rows_reach_the_kernel() -> slango::Result<()> {
    let device = device();
    let kernel = kernel(&device, SHAPES, "points_and_a_tall_matrix");
    let output = Buffer::<f32>::zeroed(&device, 18)?;
    kernel
        .call()
        .set(
            "points",
            [
                [1.0_f32, 2.0, 3.0],
                [4.0, 5.0, 6.0],
                [7.0, 8.0, 9.0],
                [10.0, 11.0, 12.0],
            ],
        )
        .set("tall", [[11.0_f32, 12.0], [21.0, 22.0], [31.0, 32.0]])
        .bind("output", &output)
        .dispatch([1, 1, 1])?;
    assert_eq!(
        output.read()?,
        [
            1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 11.0, 12.0, 21.0, 22.0,
            31.0, 32.0,
        ]
    );
    Ok(())
}

const OFFSET: &str = r#"
StructuredBuffer<float> input;
RWStructuredBuffer<float> output;
uniform float bias;
uniform uint count;

[shader("compute")]
[numthreads(64, 1, 1)]
void offset(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count) { output[id.x] = input[id.x] + bias; }
}
"#;

#[test]
fn global_uniforms_and_two_buffers_reach_the_kernel() -> slango::Result<()> {
    let device = device();
    let kernel = kernel(&device, OFFSET, "offset");
    let input = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0, 5.0])?;
    let output = Buffer::<f32>::zeroed(&device, 5)?;
    kernel
        .call()
        .set("bias", 0.25_f32)
        .set("count", 4_u32)
        .bind("input", &input)
        .bind("output", &output)
        .dispatch([5, 1, 1])?;
    assert_eq!(output.read()?, [1.25, 2.25, 3.25, 4.25, 0.0]);
    Ok(())
}

const BLOCKS: &str = r#"
struct Params { float scale; uint count; float3 shift; }
struct Extra { float bias; RWStructuredBuffer<float> values; }

StructuredBuffer<float> input;
ConstantBuffer<Params> params;
ParameterBlock<Extra> extra;

[shader("compute")]
[numthreads(64, 1, 1)]
void blocks(uint3 id : SV_DispatchThreadID)
{
    if (id.x < params.count)
    {
        extra.values[id.x] = input[id.x] * params.scale + extra.bias + params.shift.z;
    }
}
"#;

#[test]
fn a_constant_buffer_and_a_parameter_block_with_a_buffer_and_a_value_reach_the_kernel()
-> slango::Result<()> {
    let device = device();
    let kernel = kernel(&device, BLOCKS, "blocks");
    let input = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0])?;
    let values = Buffer::from_slice(&device, &[-1.0_f32; 3])?;
    kernel
        .call()
        .set("params.scale", 10.0_f32)
        .set("params.count", 3_u32)
        .set("params.shift", [0.0_f32, 0.0, 100.0])
        .set("extra.bias", 0.5_f32)
        .bind("input", &input)
        .bind("extra.values", &values)
        .dispatch([3, 1, 1])?;
    assert_eq!(values.read()?, [110.5, 120.5, 130.5]);
    Ok(())
}

const PIPELINE: &str = r#"
StructuredBuffer<float> source;
RWStructuredBuffer<float> middle;
RWStructuredBuffer<float> target;
uniform uint count;

[shader("compute")]
[numthreads(64, 1, 1)]
void double_each(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count) { middle[id.x] = source[id.x] * 2.0; }
}

[shader("compute")]
[numthreads(32, 1, 1)]
void add_one(uint3 id : SV_DispatchThreadID)
{
    if (id.x < count) { target[id.x] = middle[id.x] + 1.0; }
}
"#;

#[test]
fn two_kernels_of_one_module_run_in_sequence_on_the_same_buffers() -> slango::Result<()> {
    let device = device();
    let module = load(&device, PIPELINE);
    let source = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0])?;
    let middle = Buffer::<f32>::zeroed(&device, 3)?;
    let target = Buffer::<f32>::zeroed(&device, 3)?;
    module
        .kernel("double_each")?
        .call()
        .set("count", 3_u32)
        .bind("source", &source)
        .bind("middle", &middle)
        .dispatch([3, 1, 1])?;
    module
        .kernel("add_one")?
        .call()
        .set("count", 3_u32)
        .bind("middle", &middle)
        .bind("target", &target)
        .dispatch([3, 1, 1])?;
    assert_eq!(target.read()?, [3.0, 5.0, 7.0]);
    assert_eq!(middle.read()?, [2.0, 4.0, 6.0]);
    Ok(())
}

const ELEMENTS: &str = r#"
RWStructuredBuffer<float> floats;
RWStructuredBuffer<int> ints;
RWStructuredBuffer<uint> uints;
RWStructuredBuffer<float4> float4s;
RWStructuredBuffer<int4> int4s;
RWStructuredBuffer<uint4> uint4s;

[shader("compute")] [numthreads(1, 1, 1)]
void float_kernel(uint3 id : SV_DispatchThreadID) { floats[id.x] = floats[id.x] * 2.0 + 0.5; }

[shader("compute")] [numthreads(1, 1, 1)]
void int_kernel(uint3 id : SV_DispatchThreadID) { ints[id.x] = ints[id.x] * -3 + 1; }

[shader("compute")] [numthreads(1, 1, 1)]
void uint_kernel(uint3 id : SV_DispatchThreadID) { uints[id.x] = uints[id.x] * 3u + 1u; }

[shader("compute")] [numthreads(1, 1, 1)]
void float4_kernel(uint3 id : SV_DispatchThreadID) { float4s[id.x] = float4s[id.x].wzyx * 2.0; }

[shader("compute")] [numthreads(1, 1, 1)]
void int4_kernel(uint3 id : SV_DispatchThreadID) { int4s[id.x] = -int4s[id.x].yxwz; }

[shader("compute")] [numthreads(1, 1, 1)]
void uint4_kernel(uint3 id : SV_DispatchThreadID) { uint4s[id.x] = uint4s[id.x].wxyz + 1u; }
"#;

fn run_on<T: Element + PartialEq + Debug>(
    device: &Device,
    kernel: &str,
    parameter: &str,
    input: &[T; 2],
    expected: &[T; 2],
) -> slango::Result<()> {
    let module = load(device, ELEMENTS);
    let buffer = Buffer::from_slice(device, input)?;
    module
        .kernel(kernel)?
        .call()
        .bind(parameter, &buffer)
        .dispatch([2, 1, 1])?;
    assert_eq!(buffer.read()?, expected, "{kernel}");
    Ok(())
}

#[test]
fn a_kernel_reads_and_writes_each_element_type() -> slango::Result<()> {
    let device = device();
    run_on(
        &device,
        "float_kernel",
        "floats",
        &[1.0_f32, -2.0],
        &[2.5, -3.5],
    )?;
    run_on(&device, "int_kernel", "ints", &[2_i32, -5], &[-5, 16])?;
    run_on(&device, "uint_kernel", "uints", &[2_u32, 7], &[7, 22])?;
    run_on(
        &device,
        "float4_kernel",
        "float4s",
        &[[1.0_f32, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]],
        &[[8.0, 6.0, 4.0, 2.0], [16.0, 14.0, 12.0, 10.0]],
    )?;
    run_on(
        &device,
        "int4_kernel",
        "int4s",
        &[[1_i32, -2, 3, -4], [5, 6, 7, 8]],
        &[[2, -1, 4, -3], [-6, -5, -8, -7]],
    )?;
    run_on(
        &device,
        "uint4_kernel",
        "uint4s",
        &[[1_u32, 2, 3, 4], [10, 20, 30, 40]],
        &[[5, 2, 3, 4], [41, 11, 21, 31]],
    )?;
    Ok(())
}

const GRID: &str = r#"
RWStructuredBuffer<uint> cells;
uniform uint3 size;

[shader("compute")]
[numthreads(4, 2, 2)]
void grid(uint3 id : SV_DispatchThreadID)
{
    if (all(id < size))
    {
        cells[(id.z * size.y + id.y) * size.x + id.x] = 1 + id.x + 100 * id.y + 10000 * id.z;
    }
}
"#;

/// Returns the cells that the kernel `grid` writes over a grid of `size`.
fn expected_cells(size: [u32; 3]) -> Vec<u32> {
    let [width, height, depth] = size;
    let mut cells = Vec::new();
    for z in 0..depth {
        for y in 0..height {
            for x in 0..width {
                cells.push(1 + x + 100 * y + 10_000 * z);
            }
        }
    }
    cells
}

fn run_grid(device: &Device, size: [u32; 3], groups: Option<[u32; 3]>) -> slango::Result<()> {
    let kernel = kernel(device, GRID, "grid");
    let expected = expected_cells(size);
    let cells = Buffer::<u32>::zeroed(device, expected.len())?;
    let mut call = kernel.call();
    call.set("size", size).bind("cells", &cells);
    match groups {
        Some(groups) => call.dispatch_groups(groups)?,
        None => call.dispatch(size)?,
    }
    assert_eq!(cells.read()?, expected, "{size:?} {groups:?}");
    Ok(())
}

#[test]
fn element_counts_that_are_no_multiple_of_the_group_size_cover_every_element() -> slango::Result<()>
{
    let device = device();
    run_grid(&device, [10, 1, 1], None)?;
    run_grid(&device, [5, 3, 1], None)?;
    run_grid(&device, [5, 3, 3], None)
}

#[test]
fn workgroup_counts_run_whole_groups_in_one_two_and_three_dimensions() -> slango::Result<()> {
    let device = device();
    run_grid(&device, [12, 1, 1], Some([3, 1, 1]))?;
    run_grid(&device, [8, 4, 1], Some([2, 2, 1]))?;
    run_grid(&device, [4, 4, 4], Some([1, 2, 2]))
}

#[test]
fn a_dispatch_of_zero_elements_runs_no_thread() -> slango::Result<()> {
    let device = device();
    let kernel = kernel(&device, GRID, "grid");
    let cells = Buffer::from_slice(&device, &[7_u32; 4])?;
    let mut call = kernel.call();
    call.set("size", [4_u32, 1, 1]).bind("cells", &cells);
    call.dispatch([0, 1, 1])?;
    call.dispatch_groups([1, 1, 0])?;
    assert_eq!(cells.read()?, [7; 4]);
    Ok(())
}

#[test]
fn clones_of_a_kernel_run_on_two_threads_at_once() -> slango::Result<()> {
    let device = device();
    let kernel = kernel(&device, OFFSET, "offset");
    let results = thread::scope(|scope| {
        let workers: Vec<_> = [1.0_f32, 100.0]
            .into_iter()
            .map(|bias| {
                let kernel = kernel.clone();
                let device = device.clone();
                scope.spawn(move || -> slango::Result<Vec<f32>> {
                    let input = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0])?;
                    let output = Buffer::<f32>::zeroed(&device, 3)?;
                    for _ in 0..20 {
                        kernel
                            .call()
                            .set("bias", bias)
                            .set("count", 3_u32)
                            .bind("input", &input)
                            .bind("output", &output)
                            .dispatch([3, 1, 1])?;
                    }
                    output.read()
                })
            })
            .collect();
        workers
            .into_iter()
            .map(|worker| worker.join().expect("the worker does not panic"))
            .collect::<slango::Result<Vec<Vec<f32>>>>()
    })?;
    assert_eq!(results, [vec![2.0, 3.0, 4.0], vec![101.0, 102.0, 103.0]]);
    Ok(())
}

#[test]
fn a_dispatch_with_plain_values_on_a_destroyed_device_is_a_backend_error() -> slango::Result<()> {
    let device = device();
    let module = device.load_module(shader("scale.slang"))?;
    let data = Buffer::from_slice(&device, &[1.0_f32, 2.0])?;
    let kernel = module.kernel("scale")?;
    slango::wgpu::device(&device).destroy();
    let error = kernel
        .call()
        .set("factor", 2.0_f32)
        .set("count", 2_u32)
        .bind("data", &data)
        .dispatch([2, 1, 1])
        .expect_err("the device is destroyed");
    assert_eq!(error.kind(), slango::ErrorKind::Backend, "{error}");
    assert!(
        error
            .to_string()
            .contains("wgpu rejected the dispatch of the kernel `scale`"),
        "{error}"
    );
    Ok(())
}
