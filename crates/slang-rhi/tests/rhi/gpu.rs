//! The tests that open a device on an adapter of the graphics API of the
//! platform, Vulkan on Linux and Direct3D 12 on Windows, which fail, and
//! never skip, when no such adapter exists.

use std::thread;

use slang_compiler::{GlobalSession, Session, Target, TargetFormat, Toggle};
use slang_rhi::{
    BufferDesc, BufferUsage, DebugMessageSource, Device, DeviceOptions, DeviceType, ErrorKind,
    MemoryType, Operation,
};

use crate::common::{
    API, DEVICE_TYPE, DOUBLE, Message, bytes, double, float_buffer, link, open, options, pipeline,
    read,
};

#[cfg(test)]
mod compile_session;
#[cfg(test)]
mod objects;
#[cfg(test)]
mod render;

#[test]
fn adapters_lists_an_adapter_and_a_device_opens_on_each() {
    let adapters = slang_rhi::adapters(DEVICE_TYPE).expect("slang-rhi lists the adapters");
    assert!(!adapters.is_empty(), "slang-rhi finds no {API} adapter");
    let (mut options, messages) = options(Toggle::Off);
    for adapter in &adapters {
        assert_eq!(adapter.device_type(), DEVICE_TYPE);
        let device = Device::new(options.adapter(adapter)).expect("a device opens on the adapter");
        assert_eq!(device.info().device_type(), DEVICE_TYPE);
        assert_eq!(device.info().adapter_name(), adapter.name());
    }
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_device_of_the_default_type_opens_on_the_api_of_the_platform() {
    let (mut options, messages) = options(Toggle::Off);
    options.device_type(DeviceType::Default);
    let device = open(&options);
    assert_eq!(device.info().device_type(), DEVICE_TYPE);
    assert_eq!(device.info().api_name(), API);
    assert_eq!(messages.errors(), []);
}

#[test]
fn the_information_of_a_device_holds_limits_within_the_minimums_of_both_apis() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let info = device.info();
    assert!(
        info.max_compute_dispatch_thread_groups()
            .iter()
            .all(|count| *count >= 65_535),
        "{info:?}"
    );
    let [x, y, z] = info.max_compute_thread_group_size();
    assert!(x >= 1024 && y >= 1024 && z >= 64, "{info:?}");
    assert!(info.max_compute_threads_per_group() >= 1024, "{info:?}");
    assert!(info.max_buffer_size() > 0, "{info:?}");
    assert!(info.max_texture_dimension_2d() >= 4096, "{info:?}");
    assert!(info.max_vertex_streams() >= 16, "{info:?}");
    assert!(info.max_vertex_input_elements() >= 16, "{info:?}");
    assert_eq!(messages.errors(), []);
}

#[test]
fn an_adapter_of_another_device_type_than_the_options_is_refused() {
    let adapter = slang_rhi::adapters(DEVICE_TYPE)
        .expect("slang-rhi lists the adapters")
        .into_iter()
        .next()
        .expect("slang-rhi finds an adapter");
    let (mut options, _) = options(Toggle::Off);
    options.device_type(DeviceType::Default).adapter(&adapter);
    let error = Device::new(&options).expect_err("the device types differ");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.operation(), Operation::CreateDevice);
}

#[test]
fn the_compute_path_doubles_a_buffer_with_optimized_code_and_no_message() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    assert_eq!(device.info().api_name(), API);
    assert_eq!(double(&device), [2.0, 4.0, 6.0, 8.0]);
    let all = messages.all();
    assert!(
        all.iter().all(|(_, _, text)| !text.contains("spirv-opt")),
        "the device's session lacks the loader of the SPIR-V optimizer: {all:?}"
    );
    assert_eq!(messages.errors(), []);
}

#[test]
fn uniform_parameters_and_buffers_land_at_the_offsets_that_offset_of_finds() {
    const MIXED: &str = r#"
RWStructuredBuffer<float> first;
uniform float scale;
uniform uint index;
RWStructuredBuffer<float> second;
uniform float bias;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID, uniform float extra)
{
    first[index] = first[index] * scale + bias;
    second[0] = second[0] + extra;
}
"#;
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let pipeline = pipeline(&device, "mixed", MIXED);
    let first = float_buffer(&device, &[1.0, 2.0, 3.0]);
    let second = float_buffer(&device, &[10.0]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let scale = root.offset_of("scale")?.expect("scale");
            let index = root.offset_of("index")?.expect("index");
            let bias = root.offset_of("bias")?.expect("bias");
            assert_ne!(scale.uniform_offset(), bias.uniform_offset());
            root.set_data(scale, &3.0_f32.to_le_bytes())?;
            root.set_data(index, &2_u32.to_le_bytes())?;
            root.set_data(bias, &0.5_f32.to_le_bytes())?;
            let first_offset = root.offset_of("first")?.expect("first");
            let second_offset = root.offset_of("second")?.expect("second");
            assert_ne!(
                first_offset.binding_range_index(),
                second_offset.binding_range_index()
            );
            root.set_buffer(first_offset, &first)?;
            root.set_buffer(second_offset, &second)?;
            assert_eq!(root.offset_of("missing")?, None);
            assert_eq!(root.offset_of("extra")?, None);
            let mut entry = root.entry_point(0)?;
            let extra = entry
                .offset_of("extra")?
                .expect("the entry point declares extra");
            entry.set_data(extra, &5.0_f32.to_le_bytes())?;
            drop(entry);
            assert_eq!(
                root.entry_point(1)
                    .map(drop)
                    .expect_err("one entry point")
                    .kind(),
                ErrorKind::InvalidArgument
            );
            drop(root);
            pass.dispatch([1, 1, 1])
        })
        .expect("the pass records");
    let commands = encoder.finish().expect("the encoder finishes");
    queue
        .submit([commands])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_eq!(read(&device, &first), [1.0, 2.0, 9.5]);
    assert_eq!(read(&device, &second), [15.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn checks_refuse_a_program_of_another_session_and_parameters_out_of_bounds() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);

    let global = GlobalSession::new().expect("Slang creates a global session");
    let other = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv))
        .build()
        .expect("Slang creates a session");
    let foreign = link(&other, "foreign", DOUBLE);
    let error = device
        .create_shader_program(&foreign)
        .expect_err("the program belongs to another session");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.operation(), Operation::CreateShaderProgram);

    let wrong = BufferDesc::new(16, BufferUsage::UNORDERED_ACCESS);
    let error = device
        .create_buffer(&wrong, Some(&[0; 12]))
        .expect_err("the initial data is short");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.code(), None);

    let values = float_buffer(&device, &[1.0, 2.0, 3.0, 4.0]);
    let mut out = [0_u8; 8];
    let error = device
        .read_buffer(&values, 12, &mut out)
        .expect_err("the range ends past the buffer");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    let error = device
        .read_buffer(&values, u64::MAX, &mut out)
        .expect_err("the end of the range overflows");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);

    let other_device = open(&options);
    let foreign_buffer = float_buffer(&other_device, &[1.0]);
    let error = device
        .read_buffer(&foreign_buffer, 0, &mut out[..4])
        .expect_err("the buffer belongs to another device");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);

    let pipeline = pipeline(&device, "double", DOUBLE);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let kinds = encoder
        .compute_pass(|pass| {
            let dispatch = pass
                .dispatch([1, 1, 1])
                .expect_err("nothing is bound")
                .kind();
            let mut root = pass.bind_pipeline(&pipeline)?;
            let factor = root.offset_of("factor")?.expect("factor");
            let past_data = root
                .set_data(factor, &[0; 8])
                .expect_err("the data ends past the uniform data of the parameter")
                .kind();
            let target = root.offset_of("values")?.expect("values");
            let no_uniform = root
                .set_data(target, &[0; 4])
                .expect_err("a buffer has no uniform data")
                .kind();
            let no_range = root
                .set_buffer(factor, &values)
                .expect_err("a float has no binding range")
                .kind();
            let other_device_buffer = root
                .set_buffer(target, &foreign_buffer)
                .expect_err("the buffer belongs to another device")
                .kind();
            let past_element = root
                .set_buffer(target.with_binding_array_index(1), &values)
                .expect_err("the range has one binding")
                .kind();
            let mut entry = root.entry_point(0)?;
            let far_data = entry
                .set_data(factor, &[0; 4])
                .expect_err("the entry point has no uniform data")
                .kind();
            let past_range = entry
                .set_buffer(target, &values)
                .expect_err("the entry point has no binding range")
                .kind();
            drop(entry);
            Ok([
                dispatch,
                past_data,
                no_uniform,
                no_range,
                other_device_buffer,
                past_element,
                far_data,
                past_range,
            ])
        })
        .expect("the closure returns the kinds");
    assert_eq!(kinds, [ErrorKind::InvalidArgument; 8]);
    drop(encoder);
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_kernel_that_does_not_generate_code_for_the_device_fails_with_its_diagnostics() {
    const RECURSIVE: &str = r#"
RWStructuredBuffer<float> values;

float countDown(float x)
{
    if (x > 1.0)
    {
        return countDown(x - 1.0);
    }
    return x;
}

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    values[thread.x] = countDown(values[thread.x]);
}
"#;
    let (options, messages) = options(Toggle::On);
    let device = open(&options);
    let linked = link(device.slang_session(), "recursive", RECURSIVE);
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program, which generates no code");
    let error = device
        .create_compute_pipeline(&program)
        .expect_err("Slang cannot generate code for the kernel");
    assert_eq!(error.kind(), ErrorKind::Compile);
    let diagnostics = error.diagnostics().expect("Slang reported why");
    assert!(diagnostics.contains("recursion"), "{diagnostics}");
    drop((program, linked, device));
    let slang_errors: Vec<Message> = messages
        .errors()
        .into_iter()
        .filter(|(_, source, _)| *source == DebugMessageSource::Slang)
        .collect();
    assert_eq!(
        slang_errors.len(),
        1,
        "the debug callback, which outlives the device, received the error: {slang_errors:?}"
    );
    assert!(diagnostics.contains(&slang_errors[0].2), "{diagnostics}");
}

#[test]
fn a_device_without_a_debug_callback_still_reports_diagnostics_and_keeps_working() {
    const BROKEN: &str = r#"
RWStructuredBuffer<float> values;

float countDown(float x)
{
    if (x > 1.0)
    {
        return countDown(x - 1.0);
    }
    return x;
}

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    values[thread.x] = countDown(values[thread.x]);
}
"#;
    let mut options =
        DeviceOptions::new(GlobalSession::new().expect("Slang creates a global session"));
    options.device_type(DEVICE_TYPE);
    let device = open(&options);
    let linked = link(device.slang_session(), "broken", BROKEN);
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    let error = device
        .create_compute_pipeline(&program)
        .expect_err("Slang cannot generate code for the kernel");
    assert_eq!(error.kind(), ErrorKind::Compile);
    assert!(
        error
            .diagnostics()
            .is_some_and(|text| text.contains("recursion")),
        "{error}"
    );
    assert_eq!(double(&device), [2.0, 4.0, 6.0, 8.0]);
}

#[test]
fn a_program_without_a_compute_entry_point_is_refused_before_slang_rhi_compiles_it() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let module = device
        .slang_session()
        .load_module_from_source("plain", "plain.slang", DOUBLE)
        .expect("the source compiles")
        .into_value();
    let linked = module.link().expect("the module links").into_value();
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates a program without entry points");
    let error = device
        .create_compute_pipeline(&program)
        .expect_err("the program has no entry point");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(messages.errors(), []);
}

#[test]
fn devices_on_two_threads_run_the_compute_path_at_once() {
    let results: Vec<Vec<f32>> = thread::scope(|scope| {
        let workers: Vec<_> = (0..2)
            .map(|_| {
                scope.spawn(|| {
                    let (options, messages) = options(Toggle::Off);
                    let device = open(&options);
                    let mut results = Vec::new();
                    for _ in 0..8 {
                        results.push(double(&device));
                    }
                    assert_eq!(messages.errors(), []);
                    results
                })
            })
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("the worker does not panic"))
            .collect()
    });
    assert_eq!(results.len(), 16);
    for result in results {
        assert_eq!(result, [2.0, 4.0, 6.0, 8.0]);
    }
}

#[test]
fn a_panic_in_a_compute_pass_ends_the_pass_and_the_encoder_still_finishes() {
    let (options, messages) = options(Toggle::On);
    let device = open(&options);
    let pipeline = pipeline(&device, "double", DOUBLE);
    let values = float_buffer(&device, &[1.0, 2.0, 3.0, 4.0]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        encoder.compute_pass(|pass| -> slang_rhi::Result<()> {
            let _root = pass.bind_pipeline(&pipeline)?;
            panic!("a panic inside the pass");
        })
    }));
    assert!(outcome.is_err());
    encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let factor = root.offset_of("factor")?.expect("factor");
            root.set_data(factor, &2.0_f32.to_le_bytes())?;
            let target = root.offset_of("values")?.expect("values");
            root.set_buffer(target, &values)?;
            drop(root);
            pass.dispatch([1, 1, 1])
        })
        .expect("a second pass records after the panic");
    let commands = encoder.finish().expect("the encoder finishes");
    queue
        .submit([commands])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_eq!(read(&device, &values), [2.0, 4.0, 6.0, 8.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn the_objects_of_a_device_keep_it_alive_after_every_device_handle_is_dropped() {
    let (options, messages) = options(Toggle::On);
    let device = open(&options);
    let pipeline = pipeline(&device, "double", DOUBLE);
    let values = float_buffer(&device, &[1.0, 2.0, 3.0, 4.0]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    drop(device);
    encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let factor = root.offset_of("factor")?.expect("factor");
            root.set_data(factor, &2.0_f32.to_le_bytes())?;
            let target = root.offset_of("values")?.expect("values");
            root.set_buffer(target, &values)?;
            drop(root);
            pass.dispatch([1, 1, 1])
        })
        .expect("the pass records without a device handle");
    let commands = encoder.finish().expect("the encoder finishes");
    queue
        .submit([commands])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    drop((queue, values, pipeline));
    assert_eq!(messages.errors(), []);
}

#[test]
fn clones_of_the_handles_of_the_compute_path_run_it_after_the_originals_are_dropped() {
    let (options, messages) = options(Toggle::On);
    let device = open(&options);
    let compile = device
        .create_slang_session([std::env::temp_dir()])
        .expect("slang-rhi creates a compile session");
    let compile_clone = compile.clone();
    drop(compile);
    let linked = link(compile_clone.session(), "double", DOUBLE);
    let program = compile_clone
        .create_shader_program(&linked)
        .expect("the clone of the compile session takes its own programs");
    let program_clone = program.clone();
    drop(program);
    let pipeline = device
        .create_compute_pipeline(&program_clone)
        .expect("slang-rhi creates the compute pipeline from the clone of the program");
    let pipeline_clone = pipeline.clone();
    drop(pipeline);
    let values = float_buffer(&device, &[1.0, 2.0, 3.0, 4.0]);
    let values_clone = values.clone();
    assert_eq!(values_clone.size(), values.size());
    assert_eq!(values_clone.usage(), values.usage());
    drop(values);
    let queue = device.queue().expect("the device has a queue");
    let queue_clone = queue.clone();
    drop(queue);
    let mut encoder = queue_clone.encoder().expect("slang-rhi creates an encoder");
    encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline_clone)?;
            let factor = root.offset_of("factor")?.expect("factor");
            root.set_data(factor, &2.0_f32.to_le_bytes())?;
            let target = root.offset_of("values")?.expect("values");
            root.set_buffer(target, &values_clone)?;
            drop(root);
            pass.dispatch([1, 1, 1])
        })
        .expect("the pass records with the clones");
    queue_clone
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue_clone
        .wait_on_host()
        .expect("the queue finishes the work");
    assert_eq!(read(&device, &values_clone), [2.0, 4.0, 6.0, 8.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_submitted_dispatch_runs_exactly_once() {
    let (options, messages) = options(Toggle::On);
    let device = open(&options);
    let pipeline = pipeline(&device, "double", DOUBLE);
    let values = float_buffer(&device, &[1.0, 2.0, 3.0, 4.0]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let factor = root.offset_of("factor")?.expect("factor");
            root.set_data(factor, &2.0_f32.to_le_bytes())?;
            let target = root.offset_of("values")?.expect("values");
            root.set_buffer(target, &values)?;
            drop(root);
            pass.dispatch([1, 1, 1])
        })
        .expect("the pass records");
    let commands = encoder.finish().expect("the encoder finishes");
    queue
        .submit([commands])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    let next = queue.encoder().expect("slang-rhi creates an encoder");
    queue
        .submit([next.finish().expect("the empty encoder finishes")])
        .expect("the queue receives the empty work");
    queue
        .submit(Vec::new())
        .expect("the queue receives no work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_eq!(read(&device, &values), [2.0, 4.0, 6.0, 8.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn buffer_descriptions_without_a_size_or_a_use_or_with_an_unknown_use_are_refused() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    for desc in [
        BufferDesc::new(0, BufferUsage::UNORDERED_ACCESS),
        BufferDesc::new(16, BufferUsage::empty()),
        BufferDesc::new(16, BufferUsage::from_bits_retain(1 << 8)),
    ] {
        let error = device
            .create_buffer(&desc, None)
            .expect_err("the description is refused");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{desc:?}");
        assert_eq!(error.operation(), Operation::CreateBuffer);
    }
    let error = device
        .create_buffer(&BufferDesc::new(0, BufferUsage::COPY_SOURCE), Some(&[]))
        .expect_err("the size is 0");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    for memory in [MemoryType::Upload, MemoryType::ReadBack] {
        let desc = BufferDesc::new(16, BufferUsage::UNORDERED_ACCESS | BufferUsage::COPY_SOURCE)
            .with_memory(memory);
        let error = device
            .create_buffer(&desc, None)
            .expect_err("memory that the program maps allows no unordered access");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{memory:?}");
        assert_eq!(error.operation(), Operation::CreateBuffer);
    }
    let read_back =
        BufferDesc::new(16, BufferUsage::COPY_DESTINATION).with_memory(MemoryType::ReadBack);
    let error = device
        .create_buffer(&read_back, Some(&bytes(&[1.0, 2.0, 3.0, 4.0])))
        .expect_err("read-back memory takes no initial data");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.operation(), Operation::CreateBuffer);
    device
        .create_buffer(&read_back, None)
        .expect("slang-rhi creates a buffer in read-back memory");
    let upload = BufferDesc::new(16, BufferUsage::SHADER_RESOURCE | BufferUsage::COPY_SOURCE)
        .with_memory(MemoryType::Upload);
    let buffer = device
        .create_buffer(&upload, Some(&bytes(&[1.0, 2.0, 3.0, 4.0])))
        .expect("slang-rhi creates a buffer in upload memory");
    assert_eq!(read(&device, &buffer), [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn each_use_of_a_buffer_needs_its_usage_flag() {
    const COPY: &str = r#"
StructuredBuffer<float> input;
RWStructuredBuffer<float> output;

[shader("compute")]
[numthreads(4, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    output[thread.x] = input[thread.x] * 3.0;
}
"#;
    let (options, messages) = options(Toggle::On);
    let device = open(&options);
    let pipeline = pipeline(&device, "copy", COPY);
    let input = device
        .create_buffer(
            &BufferDesc::new(16, BufferUsage::SHADER_RESOURCE).with_element_size(4),
            Some(&bytes(&[1.0, 2.0, 3.0, 4.0])),
        )
        .expect("slang-rhi creates a buffer that shaders read");
    let error = device
        .read_buffer(&input, 0, &mut [0; 16])
        .expect_err("the buffer lacks COPY_SOURCE");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.operation(), Operation::ReadBuffer);
    let copy_only = device
        .create_buffer(&BufferDesc::new(16, BufferUsage::COPY_SOURCE), None)
        .expect("slang-rhi creates a buffer that is only copied");
    let output = float_buffer(&device, &[0.0; 4]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let kinds = encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let input_offset = root.offset_of("input")?.expect("input");
            let output_offset = root.offset_of("output")?.expect("output");
            let read_only_output = root
                .set_buffer(output_offset, &input)
                .expect_err("the kernel writes output")
                .kind();
            let copy_only_input = root
                .set_buffer(input_offset, &copy_only)
                .expect_err("the kernel reads input")
                .kind();
            root.set_buffer(input_offset, &input)?;
            root.set_buffer(output_offset, &output)?;
            drop(root);
            pass.dispatch([1, 1, 1])?;
            Ok([read_only_output, copy_only_input])
        })
        .expect("the pass records");
    assert_eq!(kinds, [ErrorKind::InvalidArgument; 2]);
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_eq!(read(&device, &output), [3.0, 6.0, 9.0, 12.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_typed_buffer_parameter_is_refused_because_a_buffer_has_no_format() {
    const TYPED: &str = r#"
RWBuffer<float> values;

[shader("compute")]
[numthreads(4, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    values[thread.x] = 7.0;
}
"#;
    let (options, messages) = options(Toggle::On);
    let device = open(&options);
    let pipeline = pipeline(&device, "typed", TYPED);
    let values = float_buffer(&device, &[1.0, 2.0, 3.0, 4.0]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let error = encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let target = root.offset_of("values")?.expect("values");
            root.set_buffer(target, &values)
        })
        .expect_err("a typed buffer is refused");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.operation(), Operation::SetBuffer);
    assert!(error.to_string().contains("format"), "{error}");
    drop(encoder);
    assert_eq!(read(&device, &values), [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_dispatch_past_the_limit_of_the_device_is_refused_and_one_of_zero_runs_nothing() {
    let (options, messages) = options(Toggle::On);
    let device = open(&options);
    let pipeline = pipeline(&device, "double", DOUBLE);
    let values = float_buffer(&device, &[1.0, 2.0, 3.0, 4.0]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    let kinds = encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let factor = root.offset_of("factor")?.expect("factor");
            root.set_data(factor, &2.0_f32.to_le_bytes())?;
            let target = root.offset_of("values")?.expect("values");
            root.set_buffer(target, &values)?;
            drop(root);
            let kinds = [[u32::MAX, 1, 1], [1, u32::MAX, 1], [1, 1, u32::MAX]]
                .map(|groups| pass.dispatch(groups).map_err(|error| error.kind()));
            pass.dispatch([0, 0, 0])?;
            pass.dispatch([0, 1, 1])?;
            Ok(kinds)
        })
        .expect("the pass records");
    assert_eq!(kinds, [Err(ErrorKind::InvalidArgument); 3]);
    queue
        .submit([encoder.finish().expect("the encoder finishes")])
        .expect("the queue receives the work");
    queue.wait_on_host().expect("the queue finishes the work");
    assert_eq!(read(&device, &values), [1.0, 2.0, 3.0, 4.0]);
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_program_with_a_specialization_parameter_left_is_refused() {
    const GENERIC: &str = r#"
interface IValue
{
    float get();
}

struct Two : IValue
{
    float get() { return 2.0; }
}

RWStructuredBuffer<float> values;

[shader("compute")]
[numthreads(1, 1, 1)]
void main<T : IValue>(uint3 thread : SV_DispatchThreadID)
{
    T value;
    values[0] = value.get();
}
"#;
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let linked = link(device.slang_session(), "generic", GENERIC);
    let error = device
        .create_shader_program(&linked)
        .expect_err("the entry point is generic");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.operation(), Operation::CreateShaderProgram);
    assert!(error.to_string().contains("specialization"), "{error}");
    assert_eq!(messages.errors(), []);
}

#[test]
fn a_kernel_whose_thread_group_exceeds_the_device_is_refused_and_the_device_keeps_working() {
    let (options, messages) = options(Toggle::Off);
    let device = open(&options);
    let info = device.info();
    let [max_x, _, max_z] = info.max_compute_thread_group_size();
    let shapes = [
        (
            format!("[numthreads({}, 1, 1)]", max_x + 1),
            format!(
                "{} threads along the axis x, more than the {max_x}",
                max_x + 1
            ),
        ),
        (
            format!("[numthreads(1, 1, {})]", max_z + 1),
            format!(
                "{} threads along the axis z, more than the {max_z}",
                max_z + 1
            ),
        ),
        (
            format!(
                "[numthreads({}, 2, 1)]",
                info.max_compute_threads_per_group().min(max_x)
            ),
            format!(
                "more than the {} threads of one group",
                info.max_compute_threads_per_group()
            ),
        ),
    ];
    for (position, (attribute, expected)) in shapes.into_iter().enumerate() {
        let source = format!(
            "RWStructuredBuffer<uint> values;\n[shader(\"compute\")]\n{attribute}\nvoid main(uint3 thread : SV_DispatchThreadID) {{ values[thread.x] = 1; }}\n"
        );
        let linked = link(device.slang_session(), &format!("large{position}"), &source);
        let program = device
            .create_shader_program(&linked)
            .expect("slang-rhi creates the program");
        let error = device
            .create_compute_pipeline(&program)
            .expect_err("the thread group exceeds the device");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{error}");
        assert!(error.to_string().contains(&expected), "{error}");
    }
    assert_eq!(double(&device), [2.0, 4.0, 6.0, 8.0]);
    assert_eq!(messages.errors(), []);
}
