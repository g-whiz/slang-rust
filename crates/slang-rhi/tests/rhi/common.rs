//! Helpers of the tests on a Vulkan adapter: devices whose debug callback
//! records every message, the linking of a kernel, buffers of `float`
//! values, and the kernel that doubles four values.

use std::io::Write as _;
use std::sync::{Arc, Mutex, PoisonError};

use slang_compiler::{Component, GlobalSession, LinkedProgram, Session, Toggle};
use slang_rhi::{
    Buffer, BufferDesc, BufferUsage, ComputePipeline, DebugMessageSource, DebugMessageType, Device,
    DeviceOptions, DeviceType,
};

/// One message that a debug callback received.
pub(crate) type Message = (DebugMessageType, DebugMessageSource, String);

/// The messages that the debug callback of a device received.
#[derive(Clone, Default)]
pub(crate) struct Messages(Arc<Mutex<Vec<Message>>>);

impl Messages {
    pub(crate) fn all(&self) -> Vec<Message> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(crate) fn errors(&self) -> Vec<Message> {
        self.all()
            .into_iter()
            .filter(|(kind, _, _)| *kind == DebugMessageType::Error)
            .collect()
    }
}

/// Returns the options of a Vulkan device whose debug callback records every
/// message in the returned list.
pub(crate) fn options(validation: Toggle) -> (DeviceOptions, Messages) {
    let messages = Messages::default();
    let record = messages.clone();
    let mut options =
        DeviceOptions::new(GlobalSession::new().expect("Slang creates a global session"));
    options
        .device_type(DeviceType::Vulkan)
        .validation(validation)
        .debug_callback(move |message| {
            let entry = (
                message.message_type(),
                message.source(),
                message.text().into_owned(),
            );
            let _ignored = writeln!(std::io::stderr(), "slang-rhi message: {entry:?}");
            record
                .0
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(entry);
        });
    (options, messages)
}

/// Opens a Vulkan device and prints its adapter.
pub(crate) fn open(options: &DeviceOptions) -> Device {
    let device = Device::new(options).expect("slang-rhi opens a Vulkan device");
    let _ignored = writeln!(
        std::io::stderr(),
        "the test runs on the Vulkan adapter {}",
        device.info().adapter_name()
    );
    device
}

/// Loads `source` into `session` and links its entry point `main`.
pub(crate) fn link(session: &Session, name: &str, source: &str) -> LinkedProgram {
    let module = session
        .load_module_from_source(name, format!("{name}.slang"), source)
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
        .link()
        .expect("the program links")
        .into_value()
}

/// Creates the compute pipeline of the entry point `main` of `source`.
pub(crate) fn pipeline(device: &Device, name: &str, source: &str) -> ComputePipeline {
    let linked = link(device.slang_session(), name, source);
    let program = device
        .create_shader_program(&linked)
        .expect("slang-rhi creates the shader program");
    device
        .create_compute_pipeline(&program)
        .expect("slang-rhi creates the compute pipeline")
}

pub(crate) fn bytes(values: &[f32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

pub(crate) fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .map(|word| f32::from_le_bytes(*word))
        .collect()
}

/// Creates a buffer of `values` that a kernel writes and the test reads back.
pub(crate) fn float_buffer(device: &Device, values: &[f32]) -> Buffer {
    let data = bytes(values);
    let size = u64::try_from(data.len()).expect("the size fits");
    let desc = BufferDesc::new(
        size,
        BufferUsage::UNORDERED_ACCESS | BufferUsage::COPY_SOURCE,
    )
    .with_element_size(4);
    device
        .create_buffer(&desc, Some(&data))
        .expect("slang-rhi creates the buffer")
}

pub(crate) fn read(device: &Device, buffer: &Buffer) -> Vec<f32> {
    let mut out = vec![0_u8; usize::try_from(buffer.size()).expect("the size fits")];
    device
        .read_buffer(buffer, 0, &mut out)
        .expect("slang-rhi reads the buffer");
    floats(&out)
}

pub(crate) const DOUBLE: &str = r#"
RWStructuredBuffer<float> values;
uniform float factor;

[shader("compute")]
[numthreads(4, 1, 1)]
void main(uint3 thread : SV_DispatchThreadID)
{
    values[thread.x] = values[thread.x] * factor;
}
"#;

/// Runs the kernel `DOUBLE` with the factor 2 on four values and returns the
/// result.
pub(crate) fn double(device: &Device) -> Vec<f32> {
    let pipeline = pipeline(device, "double", DOUBLE);
    let values = float_buffer(device, &[1.0, 2.0, 3.0, 4.0]);
    let queue = device.queue().expect("the device has a queue");
    let mut encoder = queue.encoder().expect("slang-rhi creates an encoder");
    encoder
        .compute_pass(|pass| {
            let mut root = pass.bind_pipeline(&pipeline)?;
            let factor = root
                .offset_of("factor")?
                .expect("the kernel declares factor");
            root.set_data(factor, &2.0_f32.to_le_bytes())?;
            let target = root
                .offset_of("values")?
                .expect("the kernel declares values");
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
    read(device, &values)
}
