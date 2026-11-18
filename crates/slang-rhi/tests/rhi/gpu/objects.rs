//! The tests of the shader objects of `ConstantBuffer`, `cbuffer`, and
//! `ParameterBlock` parameters, which `ShaderObject::object` returns.
//!
//! No kernel here that has a `ConstantBuffer` or a `cbuffer` also declares a
//! global `uniform`, because that combination fails to create a compute
//! pipeline on Direct3D 12 through a defect of slang-rhi. The kernels with a
//! global `uniform` use a `ParameterBlock`, which works on both APIs.

use slang_compiler::Toggle;
use slang_rhi::{Buffer, ComputePipeline, Device, ErrorKind, Operation, ShaderObject};

use crate::common::{bytes, float_buffer, open, options, pipeline, read};

/// Binds `pipeline`, lets `set` set its parameters, runs one thread group,
/// and waits for the queue.
fn run(
    device: &Device,
    pipeline: &ComputePipeline,
    set: impl FnOnce(&mut ShaderObject<'_>) -> slang_rhi::Result<()>,
) {
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(pipeline)?;
            set(&mut root)?;
            drop(root);
            pass.dispatch([1, 1, 1])
        })
        .expect("the pass records");
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
}

/// Sets the field `name` of `object` to the bytes of `values`.
fn set_floats(object: &mut ShaderObject<'_>, name: &str, values: &[f32]) -> slang_rhi::Result<()> {
    let offset = object.offset_of(name)?.expect("the object has the field");
    object.set_data(offset, &bytes(values))
}

/// Binds `buffer` to the parameter `name` of `object`.
fn set_buffer(object: &mut ShaderObject<'_>, name: &str, buffer: &Buffer) -> slang_rhi::Result<()> {
    let offset = object
        .offset_of(name)?
        .expect("the object has the parameter");
    object.set_buffer(offset, buffer)
}

#[test]
fn the_fields_of_a_constant_buffer_are_set_through_its_object_and_not_as_uniform_data() {
    const CONSTANTS: &str = r#"
struct Params
{
    float4 value;
    float scale;
};
ConstantBuffer<Params> constants;
RWStructuredBuffer<float> values;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    values[0] = constants.value.x * constants.scale;
    values[1] = constants.value.w;
}
"#;
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = pipeline(&device, "constants", CONSTANTS);
    let values = float_buffer(&device, &[0.0, 0.0]);
    let mut refusals = Vec::new();
    run(&device, &pipeline, |root| {
        let constants = root.offset_of("constants")?.expect("constants");
        assert_eq!(constants.uniform_size(), 0);
        refusals.push(
            root.set_data(constants, &[0; 4])
                .map_err(|error| error.kind()),
        );
        refusals.push(
            root.set_buffer(constants, &values)
                .map_err(|error| error.kind()),
        );
        refusals.push(
            root.object(constants.with_binding_array_index(1))
                .map(drop)
                .map_err(|error| error.kind()),
        );
        let target = root.offset_of("values")?.expect("values");
        refusals.push(root.object(target).map(drop).map_err(|error| error.kind()));
        root.set_buffer(target, &values)?;
        let mut object = root.object(constants)?;
        set_floats(&mut object, "value", &[3.0, 0.0, 0.0, 5.0])?;
        set_floats(&mut object, "scale", &[2.0])?;
        let value = object.offset_of("value")?.expect("value");
        refusals.push(object.object(value).map(drop).map_err(|error| error.kind()));
        Ok(())
    });
    assert_eq!(refusals, [Err(ErrorKind::InvalidArgument); 5]);
    assert_eq!(read(&device, &values), [6.0, 5.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn the_fields_of_a_cbuffer_are_set_through_its_object() {
    const SETTINGS: &str = r#"
cbuffer Settings
{
    float4 offset;
    float factor;
};
RWStructuredBuffer<float> values;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    values[0] = offset.y * factor;
}
"#;
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = pipeline(&device, "settings", SETTINGS);
    let values = float_buffer(&device, &[0.0]);
    run(&device, &pipeline, |root| {
        set_buffer(root, "values", &values)?;
        let settings = root
            .offset_of("Settings")?
            .expect("the cbuffer is a parameter");
        let mut object = root.object(settings)?;
        set_floats(&mut object, "offset", &[0.0, 7.0, 0.0, 0.0])?;
        set_floats(&mut object, "factor", &[3.0])
    });
    assert_eq!(read(&device, &values), [21.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn the_fields_of_a_parameter_block_are_set_through_its_object_and_not_as_uniform_data() {
    const BLOCK: &str = r#"
struct Params
{
    float4 value;
};
ParameterBlock<Params> block;
uniform float scale;
RWStructuredBuffer<float> values;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    values[0] = block.value.x + scale;
}
"#;
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = pipeline(&device, "block", BLOCK);
    let values = float_buffer(&device, &[0.0]);
    let mut refused = None;
    run(&device, &pipeline, |root| {
        set_buffer(root, "values", &values)?;
        set_floats(root, "scale", &[1.5])?;
        let block = root.offset_of("block")?.expect("block");
        assert_eq!(block.uniform_size(), 0);
        refused = Some(root.set_data(block, &[0; 4]).map_err(|error| error.kind()));
        let mut object = root.object(block)?;
        set_floats(&mut object, "value", &[4.0, 0.0, 0.0, 0.0])
    });
    assert_eq!(refused, Some(Err(ErrorKind::InvalidArgument)));
    assert_eq!(read(&device, &values), [5.5]);
    assert_eq!(messages.errors(), []);
}

/// Runs a kernel whose `ParameterBlock` holds a nested `ConstantBuffer`, a
/// nested `ParameterBlock`, and a buffer, and no plain uniform field, on a
/// device with the debug device of slang-rhi on or off.
fn nested_objects_without_a_uniform_field(validation: Toggle) {
    const NESTED: &str = r#"
struct Inner
{
    float4 value;
};
struct Outer
{
    ConstantBuffer<Inner> inner;
    ParameterBlock<Inner> deep;
    RWStructuredBuffer<float> results;
};
ParameterBlock<Outer> outer;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    outer.results[0] = outer.inner.value.x + outer.deep.value.y;
}
"#;
    let (options, messages) = options(validation);
    let device = open(&options);
    let pipeline = pipeline(&device, "nested", NESTED);
    let results = float_buffer(&device, &[0.0]);
    run(&device, &pipeline, |root| {
        let offset = root.offset_of("outer")?.expect("outer");
        let mut outer = root.object(offset)?;
        let target = outer.offset_of("results")?.expect("results");
        let refused = outer
            .object(target)
            .map(drop)
            .expect_err("a buffer has no object");
        assert_eq!(refused.kind(), ErrorKind::InvalidArgument);
        outer.set_buffer(target, &results)?;
        let inner = outer.offset_of("inner")?.expect("inner");
        set_floats(&mut outer.object(inner)?, "value", &[10.0, 0.0, 0.0, 0.0])?;
        let deep = outer.offset_of("deep")?.expect("deep");
        set_floats(&mut outer.object(deep)?, "value", &[0.0, 0.5, 0.0, 0.0])
    });
    assert_eq!(read(&device, &results), [10.5]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn nested_objects_without_a_uniform_field_are_reached_one_object_at_a_time() {
    nested_objects_without_a_uniform_field(Toggle::Off);
}

#[test]
fn nested_objects_are_reached_through_the_debug_device_of_slang_rhi() {
    nested_objects_without_a_uniform_field(Toggle::On);
}

#[test]
fn nested_objects_beside_a_uniform_field_are_reached_one_object_at_a_time() {
    const NESTED: &str = r#"
struct Inner
{
    float4 value;
};
struct Outer
{
    float factor;
    ConstantBuffer<Inner> inner;
    ParameterBlock<Inner> deep;
    RWStructuredBuffer<float> results;
};
ParameterBlock<Outer> outer;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    outer.results[0] = outer.factor * outer.inner.value.x + outer.deep.value.y;
}
"#;
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = pipeline(&device, "nested", NESTED);
    let results = float_buffer(&device, &[0.0]);
    run(&device, &pipeline, |root| {
        let offset = root.offset_of("outer")?.expect("outer");
        let mut outer = root.object(offset)?;
        let factor = outer.offset_of("factor")?.expect("factor");
        let refused = outer
            .object(factor)
            .map(drop)
            .expect_err("a float has no object");
        assert_eq!(refused.kind(), ErrorKind::InvalidArgument);
        assert_eq!(refused.operation(), Operation::GetObject);
        outer.set_data(factor, &2.0_f32.to_le_bytes())?;
        set_buffer(&mut outer, "results", &results)?;
        let inner = outer.offset_of("inner")?.expect("inner");
        set_floats(&mut outer.object(inner)?, "value", &[10.0, 0.0, 0.0, 0.0])?;
        let deep = outer.offset_of("deep")?.expect("deep");
        set_floats(&mut outer.object(deep)?, "value", &[0.0, 0.5, 0.0, 0.0])
    });
    assert_eq!(read(&device, &results), [20.5]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_structure_of_several_resources_is_refused_as_an_object_and_as_a_buffer() {
    const STRUCTURES: &str = r#"
struct Params
{
    float value;
};
struct Mixed
{
    ConstantBuffer<Params> constants;
    RWStructuredBuffer<float> scratch;
};
struct Pair
{
    RWStructuredBuffer<float> first;
    RWStructuredBuffer<float> second;
};
Mixed mixed;
Pair pair;
RWStructuredBuffer<float> values;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    values[0] = 1.0;
}
"#;
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = pipeline(&device, "structures", STRUCTURES);
    let values = float_buffer(&device, &[0.0]);
    let mut refusals = Vec::new();
    run(&device, &pipeline, |root| {
        set_buffer(root, "values", &values)?;
        let mixed = root.offset_of("mixed")?.expect("mixed");
        let pair = root.offset_of("pair")?.expect("pair");
        refusals.push(root.object(mixed).map(drop));
        refusals.push(root.set_buffer(mixed, &values));
        refusals.push(root.set_buffer(pair, &values));
        Ok(())
    });
    assert_eq!(refusals.len(), 3);
    for refusal in refusals {
        let error = refusal.expect_err("the parameter has several binding ranges");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert!(
            error.to_string().contains("structure of several resources"),
            "{error}"
        );
    }
    assert_eq!(read(&device, &values), [1.0]);
    assert_eq!(messages.errors(), []);
}
