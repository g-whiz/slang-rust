use std::ffi::{CStr, c_char, c_void};
use std::io::Write as _;
use std::ptr;
use std::sync::atomic::{AtomicUsize, Ordering};

use slang_compiler_sys::{
    IComponentType, IEntryPoint, IModule, ISession, ISlangBlob, SlangInt, SlangResult,
};
use slang_rhi_sys::{
    Binding, Binding_anon0, BindingType, BufferDesc, BufferRange, BufferUsage, CommandBufferDesc,
    CommandEncoderDesc, ComputePipelineDesc, DebugMessageSource, DebugMessageType, DeviceDesc,
    DeviceType, IAdapter, IBuffer, ICommandBuffer, ICommandEncoder, ICommandQueue,
    IComputePassEncoder, IComputePipeline, IDebugCallback, IDebugCallbackVtbl, IDevice, IRHI,
    IResource, IShaderObject, IShaderProgram, MemoryType, QueueType, ResourceState, ShaderOffset,
    ShaderProgramDesc, SubmitDesc, kInvalidCUDAStream, rhiGetInstance,
};

#[repr(C)]
struct Callback {
    base: IDebugCallback,
    errors: AtomicUsize,
}

unsafe extern "system" fn handle_message(
    this: *mut IDebugCallback,
    kind: DebugMessageType,
    source: DebugMessageSource,
    message: *const c_char,
) {
    if message.is_null() {
        return;
    }
    // SAFETY: slang-rhi passes a null-terminated message that stays valid for
    // the duration of the call, and the call does not keep it.
    let text = unsafe { CStr::from_ptr(message) }.to_string_lossy();
    let _ignored = writeln!(
        std::io::stderr(),
        "slang-rhi message of type {} from source {}: {text}",
        kind.0,
        source.0
    );
    if kind == DebugMessageType::Error {
        // SAFETY: `this` points to a live `Callback`, whose first field is
        // `base`, and the test derives it from the whole `Callback`, so it
        // covers the counter. The test keeps the `Callback` alive until the
        // device is released, and the counter is atomic, so calls from
        // several threads at once are sound through a shared reference.
        let callback = unsafe { &*this.cast::<Callback>() };
        callback.errors.fetch_add(1, Ordering::Relaxed);
    }
}

const fn unwinding(
    function: unsafe extern "system" fn(
        *mut IDebugCallback,
        DebugMessageType,
        DebugMessageSource,
        *const c_char,
    ),
) -> unsafe extern "system-unwind" fn(
    *mut IDebugCallback,
    DebugMessageType,
    DebugMessageSource,
    *const c_char,
) {
    // SAFETY: the two types differ only in the unwinding form of one calling
    // convention, and a caller through the unwinding form may call a function
    // of the non-unwinding form, as the standard library documents for
    // function pointers.
    unsafe {
        std::mem::transmute::<
            unsafe extern "system" fn(
                *mut IDebugCallback,
                DebugMessageType,
                DebugMessageSource,
                *const c_char,
            ),
            unsafe extern "system-unwind" fn(
                *mut IDebugCallback,
                DebugMessageType,
                DebugMessageSource,
                *const c_char,
            ),
        >(function)
    }
}

static CALLBACK_VTABLE: IDebugCallbackVtbl = IDebugCallbackVtbl {
    handleMessage: unwinding(handle_message),
};

#[test]
#[expect(
    ffi_unwind_calls,
    reason = "the test calls the C function of slang-rhi directly, and an exception that escaped it would unwind into the test, which is defined and fails it"
)]
fn a_vulkan_device_reads_back_a_buffer_through_both_overloads_of_read_buffer() {
    let callback = Callback {
        base: IDebugCallback {
            vtable: &raw const CALLBACK_VTABLE,
        },
        errors: AtomicUsize::new(0),
    };
    // SAFETY: `rhiGetInstance` takes no argument and returns the process-wide
    // instance, which it creates once under a mutex.
    let rhi: *mut IRHI = unsafe { rhiGetInstance() };
    assert!(!rhi.is_null());
    // SAFETY: `rhi` is the live instance. The adapter that it returns carries
    // no reference and stays valid until `rhiDestroyInstance`, which no test
    // calls.
    let adapter = unsafe { IRHI::getAdapter(rhi, DeviceType::Vulkan, 0) };
    assert!(!adapter.is_null(), "slang-rhi finds no Vulkan adapter");
    // SAFETY: `adapter` is the live adapter above.
    let info = unsafe { IAdapter::getInfo(adapter) };
    assert!(!info.is_null());
    // SAFETY: `info` is non-null and points to the information of the
    // adapter, which lives as long as the adapter, and nothing writes to it.
    let info = unsafe { &*info };
    // SAFETY: slang-rhi fills `name` with a null-terminated string that fits
    // the array.
    let name = unsafe { CStr::from_ptr(info.name.as_ptr()) };
    let _ignored = writeln!(
        std::io::stderr(),
        "the test runs on the Vulkan adapter {}",
        name.to_string_lossy()
    );

    let desc = DeviceDesc {
        deviceType: DeviceType::Vulkan,
        adapter,
        debugCallback: (&raw const callback).cast::<IDebugCallback>().cast_mut(),
        ..DeviceDesc::default()
    };
    let mut device: *mut IDevice = ptr::null_mut();
    // SAFETY: `desc` is a live descriptor whose structure type and defaults
    // come from the header, and its adapter is live. Its debug callback
    // points to the whole `Callback`, which lives until the device is
    // released below, tolerates calls from several threads, and is never
    // written through the pointer, because the counter is atomic. `device`
    // is a live out-pointer.
    let result = unsafe { IRHI::createDevice(rhi, &raw const desc, &raw mut device) };
    assert_eq!(result, SlangResult::SLANG_OK);
    assert!(!device.is_null());

    let data: [u8; 16] = std::array::from_fn(|index| u8::try_from(index * 7 + 3).unwrap_or(0));
    let buffer_desc = BufferDesc {
        size: 16,
        memoryType: MemoryType::DeviceLocal,
        usage: BufferUsage(
            BufferUsage::ShaderResource.0
                | BufferUsage::CopySource.0
                | BufferUsage::CopyDestination.0,
        ),
        ..BufferDesc::default()
    };
    let mut buffer: *mut IBuffer = ptr::null_mut();
    // SAFETY: `device` is live, `buffer_desc` is a live descriptor of 16
    // bytes, `data` holds those 16 bytes, which slang-rhi copies before the
    // call returns, and `buffer` is a live out-pointer.
    let result = unsafe {
        IDevice::createBuffer(
            device,
            &raw const buffer_desc,
            data.as_ptr().cast::<c_void>(),
            &raw mut buffer,
        )
    };
    assert_eq!(result, SlangResult::SLANG_OK);
    assert!(!buffer.is_null());

    let mut read = [0_u8; 16];
    // SAFETY: `device` and `buffer` are live, the range lies inside the
    // buffer, and `read` is a live buffer of the 16 bytes that the call
    // writes.
    let result =
        unsafe { IDevice::readBuffer(device, buffer, 0, 16, read.as_mut_ptr().cast::<c_void>()) };
    assert_eq!(result, SlangResult::SLANG_OK);
    assert_eq!(read, data);

    let mut blob: *mut ISlangBlob = ptr::null_mut();
    // SAFETY: `device` and `buffer` are live, the range lies inside the
    // buffer, and `blob` is a live out-pointer that receives a blob with one
    // reference.
    let result = unsafe { IDevice::readBuffer_blob(device, buffer, 0, 16, &raw mut blob) };
    assert_eq!(result, SlangResult::SLANG_OK);
    assert!(!blob.is_null());
    // SAFETY: `blob` is the live blob that the call returned.
    let pointer = unsafe { ISlangBlob::getBufferPointer(blob) };
    // SAFETY: `blob` is the live blob that the call returned.
    let size = unsafe { ISlangBlob::getBufferSize(blob) };
    assert_eq!(size, 16);
    assert!(!pointer.is_null());
    // SAFETY: `pointer` is non-null and points to the `size` bytes that the
    // blob owns, which stay valid until the blob is released below.
    let contents = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), size) };
    assert_eq!(contents, data);

    // SAFETY: `blob` holds the one reference that `readBuffer_blob`
    // returned, and nothing uses it after this release.
    unsafe { ISlangBlob::release(blob) };
    // SAFETY: `buffer` holds the one reference that `createBuffer` returned,
    // and nothing uses it after this release.
    unsafe { IBuffer::release(buffer) };
    // SAFETY: `device` holds the one reference that `createDevice` returned,
    // and every object created from it is released above.
    let count = unsafe { IDevice::release(device) };
    assert_eq!(count, 0);
    assert_eq!(callback.errors.load(Ordering::Relaxed), 0);
}

const KERNEL: &CStr = c"
RWStructuredBuffer<float> values;

[shader(\"compute\")]
[numthreads(4, 1, 1)]
void doubleAndAdd(uint3 thread : SV_DispatchThreadID)
{
    values[thread.x] = values[thread.x] * 2.0f + 1.0f;
}
";

fn assert_ok(result: SlangResult, what: &str, diagnostics: *mut ISlangBlob) {
    let text = if result != SlangResult::SLANG_OK && !diagnostics.is_null() {
        // SAFETY: `diagnostics` is a live blob that Slang returned with the
        // failure, whose contents are text.
        let pointer = unsafe { ISlangBlob::getBufferPointer(diagnostics) };
        // SAFETY: `diagnostics` is the live blob above.
        let size = unsafe { ISlangBlob::getBufferSize(diagnostics) };
        // SAFETY: `pointer` points to the `size` bytes that the blob owns,
        // which stay valid while the blob lives.
        let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), size) };
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        String::new()
    };
    assert_eq!(result, SlangResult::SLANG_OK, "{what}: {text}");
}

fn release_blob(blob: *mut ISlangBlob) {
    if !blob.is_null() {
        // SAFETY: a non-null `blob` holds the one reference that the call
        // which returned it gave, and nothing uses it after this release.
        unsafe { ISlangBlob::release(blob) };
    }
}

#[test]
#[expect(
    ffi_unwind_calls,
    reason = "the test calls the C function of slang-rhi directly, and an exception that escaped it would unwind into the test, which is defined and fails it"
)]
#[expect(
    clippy::too_many_lines,
    reason = "the test walks the whole compute path through the raw bindings in the order that a caller follows"
)]
fn a_vulkan_device_dispatches_a_kernel_and_submits_it_with_the_default_submit_desc() {
    let callback = Callback {
        base: IDebugCallback {
            vtable: &raw const CALLBACK_VTABLE,
        },
        errors: AtomicUsize::new(0),
    };
    // SAFETY: `rhiGetInstance` takes no argument and returns the process-wide
    // instance, which it creates once under a mutex.
    let rhi: *mut IRHI = unsafe { rhiGetInstance() };
    assert!(!rhi.is_null());
    let desc = DeviceDesc {
        deviceType: DeviceType::Vulkan,
        debugCallback: (&raw const callback).cast::<IDebugCallback>().cast_mut(),
        ..DeviceDesc::default()
    };
    let mut device: *mut IDevice = ptr::null_mut();
    // SAFETY: `desc` is a live descriptor whose structure type and defaults
    // come from the header. Its debug callback points to the whole
    // `Callback`, which lives until the device is released below, tolerates
    // calls from several threads, and is never written through the pointer,
    // because the counter is atomic. `device` is a live out-pointer.
    let result = unsafe { IRHI::createDevice(rhi, &raw const desc, &raw mut device) };
    assert_ok(result, "createDevice", ptr::null_mut());
    assert!(!device.is_null());
    // SAFETY: `device` is live.
    let info = unsafe { IDevice::getInfo(device) };
    assert!(!info.is_null());
    // SAFETY: `info` is non-null and points to the information of the device,
    // which lives as long as the device and is not written while the test
    // reads it.
    let info = unsafe { &*info };
    assert!(!info.adapterName.is_null());
    // SAFETY: slang-rhi sets `adapterName` to a null-terminated string that
    // lives as long as the device.
    let name = unsafe { CStr::from_ptr(info.adapterName) };
    let _ignored = writeln!(
        std::io::stderr(),
        "the dispatch runs on the Vulkan adapter {}",
        name.to_string_lossy()
    );

    let initial: [f32; 4] = [0.0, 1.0, 2.0, 3.0];
    let buffer_desc = BufferDesc {
        size: 16,
        elementSize: 4,
        memoryType: MemoryType::DeviceLocal,
        usage: BufferUsage(
            BufferUsage::ShaderResource.0
                | BufferUsage::UnorderedAccess.0
                | BufferUsage::CopySource.0
                | BufferUsage::CopyDestination.0,
        ),
        defaultState: ResourceState::UnorderedAccess,
        ..BufferDesc::default()
    };
    let mut buffer: *mut IBuffer = ptr::null_mut();
    // SAFETY: `device` is live, `buffer_desc` describes 16 bytes, `initial`
    // holds those 16 bytes, which slang-rhi copies before the call returns,
    // and `buffer` is a live out-pointer.
    let result = unsafe {
        IDevice::createBuffer(
            device,
            &raw const buffer_desc,
            initial.as_ptr().cast::<c_void>(),
            &raw mut buffer,
        )
    };
    assert_ok(result, "createBuffer", ptr::null_mut());

    let mut session: *mut ISession = ptr::null_mut();
    // SAFETY: `device` is live and `session` is a live out-pointer, which
    // receives the Slang session of the device with one added reference.
    let result = unsafe { IDevice::getSlangSession(device, &raw mut session) };
    assert_ok(result, "getSlangSession", ptr::null_mut());
    let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
    // SAFETY: `session` is live, the three strings are null-terminated and
    // outlive the call, and `diagnostics` is a live out-pointer. The module
    // that the call returns belongs to the session, so the test does not
    // release it.
    let module = unsafe {
        ISession::loadModuleFromSourceString(
            session,
            c"dispatch".as_ptr(),
            c"dispatch.slang".as_ptr(),
            KERNEL.as_ptr(),
            &raw mut diagnostics,
        )
    };
    assert!(!module.is_null(), "loadModuleFromSourceString");
    release_blob(diagnostics);
    let mut entry_point: *mut IEntryPoint = ptr::null_mut();
    // SAFETY: `module` is live, the name is null-terminated, and
    // `entry_point` is a live out-pointer that receives one reference.
    let result = unsafe {
        IModule::findEntryPointByName(module, c"doubleAndAdd".as_ptr(), &raw mut entry_point)
    };
    assert_ok(result, "findEntryPointByName", ptr::null_mut());
    let parts: [*mut IComponentType; 2] = [module.cast(), entry_point.cast()];
    let mut linked: *mut IComponentType = ptr::null_mut();
    let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
    // SAFETY: `session` is live, `parts` holds two live component types of
    // that session, whose interfaces derive from the component type with it
    // as their first base, and both out-pointers are live.
    let result = unsafe {
        ISession::createCompositeComponentType(
            session,
            parts.as_ptr(),
            SlangInt::try_from(parts.len()).unwrap_or(0),
            &raw mut linked,
            &raw mut diagnostics,
        )
    };
    assert_ok(result, "createCompositeComponentType", diagnostics);
    release_blob(diagnostics);

    let program_desc = ShaderProgramDesc {
        slangGlobalScope: linked,
        ..ShaderProgramDesc::default()
    };
    let mut program: *mut IShaderProgram = ptr::null_mut();
    let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
    // SAFETY: `device` is live, `program_desc` names the live linked program
    // of the session of the device, and both out-pointers are live.
    let result = unsafe {
        IDevice::createShaderProgram(
            device,
            &raw const program_desc,
            &raw mut program,
            &raw mut diagnostics,
        )
    };
    assert_ok(result, "createShaderProgram", diagnostics);
    release_blob(diagnostics);
    let pipeline_desc = ComputePipelineDesc {
        program,
        ..ComputePipelineDesc::default()
    };
    let mut pipeline: *mut IComputePipeline = ptr::null_mut();
    // SAFETY: `device` and `program` are live, and `pipeline` is a live
    // out-pointer.
    let result = unsafe {
        IDevice::createComputePipeline(device, &raw const pipeline_desc, &raw mut pipeline)
    };
    assert_ok(result, "createComputePipeline", ptr::null_mut());

    let mut queue: *mut ICommandQueue = ptr::null_mut();
    // SAFETY: `device` is live and `queue` is a live out-pointer that
    // receives one reference.
    let result = unsafe { IDevice::getQueue(device, QueueType::Graphics, &raw mut queue) };
    assert_ok(result, "getQueue", ptr::null_mut());
    let encoder_desc = CommandEncoderDesc::default();
    let mut encoder: *mut ICommandEncoder = ptr::null_mut();
    // SAFETY: `queue` is live, `encoder_desc` is a live descriptor, and
    // `encoder` is a live out-pointer that receives one reference.
    let result = unsafe {
        ICommandQueue::createCommandEncoder(queue, &raw const encoder_desc, &raw mut encoder)
    };
    assert_ok(result, "createCommandEncoder", ptr::null_mut());
    // SAFETY: `encoder` is live and records no other pass. The pass belongs
    // to the encoder, so the test does not release it.
    let pass: *mut IComputePassEncoder = unsafe { ICommandEncoder::beginComputePass(encoder) };
    assert!(!pass.is_null());
    // SAFETY: `pass` and `pipeline` are live. The root object belongs to the
    // pass, so the test does not release it.
    let root: *mut IShaderObject = unsafe { IComputePassEncoder::bindPipeline(pass, pipeline) };
    assert!(!root.is_null());
    let binding = Binding {
        r#type: BindingType::Buffer,
        resource: buffer.cast::<IResource>(),
        resource2: ptr::null_mut(),
        anon0: Binding_anon0 {
            bufferRange: BufferRange {
                offset: 0,
                size: u64::MAX,
            },
        },
    };
    let offset = ShaderOffset::default();
    // SAFETY: `root` is live, the offset names the first and only parameter
    // of the kernel, and `binding` names the live buffer, whose interface
    // derives from the resource interface with it as its first base.
    // slang-rhi copies the binding and adds its own reference to the buffer.
    let result = unsafe { IShaderObject::setBinding(root, &raw const offset, &raw const binding) };
    assert_ok(result, "setBinding", ptr::null_mut());
    // SAFETY: `pass` is live with its pipeline and root object bound.
    unsafe { IComputePassEncoder::dispatchCompute(pass, 1, 1, 1) };
    // SAFETY: `pass` is live and nothing uses it after it ends.
    unsafe { IComputePassEncoder::end(pass) };
    let buffer_desc = CommandBufferDesc::default();
    let mut command_buffer: *mut ICommandBuffer = ptr::null_mut();
    // SAFETY: `encoder` is live with its pass ended, `buffer_desc` is a live
    // descriptor, and `command_buffer` is a live out-pointer that receives
    // one reference.
    let result = unsafe {
        ICommandEncoder::finish(encoder, &raw const buffer_desc, &raw mut command_buffer)
    };
    assert_ok(result, "finish", ptr::null_mut());

    let mut command_buffers = [command_buffer];
    let submit = SubmitDesc {
        commandBuffers: command_buffers.as_mut_ptr(),
        commandBufferCount: 1,
        ..SubmitDesc::default()
    };
    assert_eq!(submit.cudaStream, kInvalidCUDAStream);
    // SAFETY: `queue` is live, and `submit` names one live command buffer of
    // the queue, no fence, and the CUDA stream that the header gives as the
    // default, which a Vulkan queue ignores.
    let result = unsafe { ICommandQueue::submit(queue, &raw const submit) };
    assert_ok(result, "submit", ptr::null_mut());
    // SAFETY: `queue` is live.
    let result = unsafe { ICommandQueue::waitOnHost(queue) };
    assert_ok(result, "waitOnHost", ptr::null_mut());

    let mut read = [0.0_f32; 4];
    // SAFETY: `device` and `buffer` are live, the range is the whole buffer,
    // and `read` is a live buffer of the 16 bytes that the call writes.
    let result =
        unsafe { IDevice::readBuffer(device, buffer, 0, 16, read.as_mut_ptr().cast::<c_void>()) };
    assert_ok(result, "readBuffer", ptr::null_mut());
    assert_eq!(read, [1.0, 3.0, 5.0, 7.0]);

    // SAFETY: `command_buffer` holds the one reference that `finish` returned, and the queue has finished it, and nothing uses it after this release.
    unsafe { ICommandBuffer::release(command_buffer) };
    // SAFETY: `encoder` holds the one reference that `createCommandEncoder` returned, and nothing uses it after this release.
    unsafe { ICommandEncoder::release(encoder) };
    // SAFETY: `queue` holds the one reference that `getQueue` returned, and nothing uses it after this release.
    unsafe { ICommandQueue::release(queue) };
    // SAFETY: `pipeline` holds the one reference that `createComputePipeline` returned, and nothing uses it after this release.
    unsafe { IComputePipeline::release(pipeline) };
    // SAFETY: `program` holds the one reference that `createShaderProgram` returned, and nothing uses it after this release.
    unsafe { IShaderProgram::release(program) };
    // SAFETY: `linked` holds the one reference that `createCompositeComponentType` returned, and nothing uses it after this release.
    unsafe { IComponentType::release(linked) };
    // SAFETY: `entry_point` holds the one reference that `findEntryPointByName` returned, and nothing uses it after this release.
    unsafe { IEntryPoint::release(entry_point) };
    // SAFETY: `session` holds the one reference that `getSlangSession` added, and nothing uses it after this release.
    unsafe { ISession::release(session) };
    // SAFETY: `buffer` holds the one reference that `createBuffer` returned, and nothing uses it after this release.
    unsafe { IBuffer::release(buffer) };
    // SAFETY: `device` holds the one reference that `createDevice` returned,
    // and every object created from it is released above.
    let count = unsafe { IDevice::release(device) };
    assert_eq!(count, 0);
    assert_eq!(callback.errors.load(Ordering::Relaxed), 0);
}
