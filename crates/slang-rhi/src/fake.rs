//! A fake of the interfaces `IBuffer`, `ICommandBuffer`, `ICommandQueue`,
//! `IComputePassEncoder`, and `IShaderObject` of slang-rhi for the tests
//! under Miri.
//!
//! Each fake object is a `#[repr(C)]` struct whose first field points to a
//! static instance of the generated vtable struct of its interface, so a
//! pointer to it is a valid pointer to that interface. It counts its
//! references, frees itself when the count reaches zero, holds the references
//! that slang-rhi would hold, and records every call that the crate makes in
//! a [`Log`] that the test reads. A call that no test expects aborts the
//! process.

use std::cell::{Cell, RefCell};
use std::ffi::c_void;
use std::io::Write as _;
use std::ptr::{self, NonNull};
use std::rc::Rc;
use std::slice;

use slang_compiler_sys::{ISlangUnknown, ISlangUnknownVtbl, SlangResult, SlangUUID};
use slang_rhi_sys::{
    Binding, IBuffer, IBufferVtbl, ICommandBuffer, ICommandBufferVtbl, ICommandQueue,
    ICommandQueueVtbl, IComputePassEncoder, IComputePassEncoderVtbl, IComputePipeline,
    IPassEncoder, IPassEncoderVtbl, IResource, IResourceVtbl, IShaderObject, IShaderObjectVtbl,
    ShaderOffset, SubmitDesc,
};

/// One call that the crate made on a fake object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    AddRef {
        object: &'static str,
        count: u32,
    },
    Release {
        object: &'static str,
        count: u32,
    },
    Freed {
        object: &'static str,
    },
    Call {
        object: &'static str,
        method: &'static str,
    },
    SetData {
        object: &'static str,
        offset: (u32, u32, u32),
        data: Vec<u8>,
    },
    SetBinding {
        object: &'static str,
        offset: (u32, u32, u32),
        resource: &'static str,
        resource_count: u32,
    },
    BindPipeline {
        object: &'static str,
        pipeline: usize,
    },
    Dispatch {
        object: &'static str,
        groups: [u32; 3],
    },
    Submit {
        object: &'static str,
        command_buffers: Vec<&'static str>,
    },
}

impl Event {
    pub(crate) fn add_ref(object: &'static str, count: u32) -> Self {
        Self::AddRef { object, count }
    }

    pub(crate) fn release(object: &'static str, count: u32) -> Self {
        Self::Release { object, count }
    }

    pub(crate) fn freed(object: &'static str) -> Self {
        Self::Freed { object }
    }

    pub(crate) fn call(object: &'static str, method: &'static str) -> Self {
        Self::Call { object, method }
    }

    pub(crate) fn set_data(object: &'static str, offset: (u32, u32, u32), data: &[u8]) -> Self {
        Self::SetData {
            object,
            offset,
            data: data.to_vec(),
        }
    }

    pub(crate) fn set_binding(
        object: &'static str,
        offset: (u32, u32, u32),
        resource: &'static str,
        resource_count: u32,
    ) -> Self {
        Self::SetBinding {
            object,
            offset,
            resource,
            resource_count,
        }
    }

    pub(crate) fn bind_pipeline(object: &'static str, pipeline: usize) -> Self {
        Self::BindPipeline { object, pipeline }
    }

    pub(crate) fn dispatch(object: &'static str, groups: [u32; 3]) -> Self {
        Self::Dispatch { object, groups }
    }

    pub(crate) fn submit(object: &'static str, command_buffers: &[&'static str]) -> Self {
        Self::Submit {
            object,
            command_buffers: command_buffers.to_vec(),
        }
    }
}

/// The record of the calls on the fake objects of one test, in order.
#[derive(Clone, Debug, Default)]
pub(crate) struct Log {
    events: Rc<RefCell<Vec<Event>>>,
}

impl Log {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn events(&self) -> Vec<Event> {
        self.events.borrow().clone()
    }

    fn push(&self, event: Event) {
        self.events.borrow_mut().push(event);
    }
}

/// The fields that every fake object starts with, whatever its vtable and
/// state, so that a fake can read another fake that a binding names.
#[repr(C)]
struct Header {
    vtable: *const c_void,
    count: Cell<u32>,
    name: &'static str,
    log: Log,
}

#[repr(C)]
struct FakeObject<S> {
    header: Header,
    state: S,
}

impl<S> FakeObject<S> {
    fn create<I>(vtable: *const c_void, log: &Log, name: &'static str, state: S) -> NonNull<I> {
        let object = Box::new(Self {
            header: Header {
                vtable,
                count: Cell::new(1),
                name,
                log: log.clone(),
            },
            state,
        });
        NonNull::from(Box::leak(object)).cast::<I>()
    }

    /// Returns the fake object behind an interface pointer.
    ///
    /// # Safety
    ///
    /// `this` comes from [`FakeObject::create`] with the same `S`, and the
    /// object is not freed while the returned reference lives. Breaking
    /// either is undefined behaviour.
    unsafe fn from_this<'a, I>(this: *mut I) -> &'a Self {
        // SAFETY: the caller guarantees that `this` points to a live
        // `FakeObject<S>`, which `create` allocated as one `Box`, and the
        // object is only read through shared references and cells.
        unsafe { &*this.cast::<Self>() }
    }
}

/// Returns the header of any fake object.
///
/// # Safety
///
/// `this` points to a live fake object of this module. Breaking this is
/// undefined behaviour.
unsafe fn header<'a, I>(this: *mut I) -> &'a Header {
    // SAFETY: every fake object is a `#[repr(C)]` struct that starts with a
    // `Header`, and the caller guarantees that it is live.
    unsafe { &*this.cast::<Header>() }
}

/// A vtable in a `static`, which may hold the raw pointers of placeholder
/// slots.
#[repr(transparent)]
struct Shared<V>(V);

// SAFETY: a vtable of the fake is written once, when the program starts, and
// only read afterwards, and its placeholder pointers are null and never
// dereferenced, so reads from several threads are sound.
unsafe impl<V> Sync for Shared<V> {}

macro_rules! unwinding {
    ($name:ident, $($argument:ident),+) => {
        /// Converts a function of the fake, defined with the non-unwinding ABI
        /// so that a panic in it aborts, to the unwinding type of the vtable
        /// field that holds it.
        const fn $name<$($argument,)+ R>(
            function: unsafe extern "system" fn($($argument),+) -> R,
        ) -> unsafe extern "system-unwind" fn($($argument),+) -> R {
            // SAFETY: the two types differ only in the unwinding form of one
            // calling convention, and a caller through the unwinding form may
            // call a function of the non-unwinding form, as the standard
            // library documents for function pointers.
            unsafe {
                std::mem::transmute::<
                    unsafe extern "system" fn($($argument),+) -> R,
                    unsafe extern "system-unwind" fn($($argument),+) -> R,
                >(function)
            }
        }
    };
}

unwinding!(unwinding1, A);
unwinding!(unwinding2, A, B);
unwinding!(unwinding3, A, B, C);
unwinding!(unwinding4, A, B, C, D);

fn unexpected() -> ! {
    #[expect(
        clippy::let_underscore_must_use,
        let_underscore_drop,
        reason = "the process aborts next whether or not the message reaches standard error"
    )]
    let _ = std::io::stderr()
        .write_all(b"the fake of slang-rhi received a call that no test expects\n");
    std::process::abort()
}

unsafe extern "system" fn unexpected1<A, R>(_: A) -> R {
    unexpected()
}

unsafe extern "system" fn unexpected2<A, B, R>(_: A, _: B) -> R {
    unexpected()
}

unsafe extern "system" fn unexpected3<A, B, C, R>(_: A, _: B, _: C) -> R {
    unexpected()
}

unsafe extern "system" fn unexpected4<A, B, C, D, R>(_: A, _: B, _: C, _: D) -> R {
    unexpected()
}

unsafe extern "system" fn query_interface(
    _: *mut ISlangUnknown,
    _: *const SlangUUID,
    out: *mut *mut c_void,
) -> SlangResult {
    // SAFETY: the crate passes a live out-pointer, as `queryInterface`
    // requires.
    unsafe { out.write(ptr::null_mut()) };
    SlangResult::SLANG_E_NO_INTERFACE
}

unsafe extern "system" fn add_ref(this: *mut ISlangUnknown) -> u32 {
    // SAFETY: every vtable of the fake that names this function belongs to a
    // fake object, which the contract of slang-rhi keeps alive during the
    // call.
    let header = unsafe { header(this) };
    let count = header.count.get() + 1;
    header.count.set(count);
    header.log.push(Event::add_ref(header.name, count));
    count
}

unsafe extern "system" fn release<S>(this: *mut ISlangUnknown) -> u32 {
    // SAFETY: as in `add_ref`, and the vtable that names this function
    // belongs to a `FakeObject<S>`.
    let object = unsafe { FakeObject::<S>::from_this(this) };
    let count = object.header.count.get() - 1;
    object.header.count.set(count);
    object
        .header
        .log
        .push(Event::release(object.header.name, count));
    if count == 0 {
        object.header.log.push(Event::freed(object.header.name));
        // SAFETY: the count reached zero, so no reference to the object
        // remains, and `create` allocated it as a `Box` of this type. The
        // shared reference `object` is not used after this point.
        drop(unsafe { Box::from_raw(this.cast::<FakeObject<S>>()) });
    }
    count
}

const fn unknown_vtable<S>() -> ISlangUnknownVtbl {
    ISlangUnknownVtbl {
        queryInterface: unwinding3(query_interface),
        addRef: unwinding1(add_ref),
        release: unwinding1(release::<S>),
    }
}

/// A fake `IBuffer`, which holds nothing.
pub(crate) struct FakeBuffer;

static BUFFER_VTABLE: Shared<IBufferVtbl> = Shared(IBufferVtbl {
    base: IResourceVtbl {
        base: unknown_vtable::<()>(),
        getNativeHandle: unwinding2(unexpected2),
    },
    getDesc: unwinding1(unexpected1),
    getSharedHandle: unwinding2(unexpected2),
    getDeviceAddress: unwinding1(unexpected1),
    getDescriptorHandle: ptr::null(),
});

impl FakeBuffer {
    /// Creates a fake buffer with one reference, which the caller owns.
    pub(crate) fn create(log: &Log, name: &'static str) -> NonNull<IBuffer> {
        FakeObject::create((&raw const BUFFER_VTABLE).cast::<c_void>(), log, name, ())
    }

    /// Releases one reference of a fake buffer.
    ///
    /// # Safety
    ///
    /// `raw` is a live fake buffer, and the caller owns the reference that
    /// this releases. Breaking either is undefined behaviour.
    pub(crate) unsafe fn release_raw(raw: NonNull<IBuffer>) {
        // SAFETY: the caller guarantees both conditions of `release`.
        unsafe { IBuffer::release(raw.as_ptr()) };
    }
}

/// The object of a parameter of a fake shader object, at its binding range
/// index and array index.
type SubObject = ((u32, u32), NonNull<IShaderObject>);

/// The state of a fake shader object: the objects of its entry points, its
/// objects of parameters by binding range index and array index, and the
/// resources that its bindings hold, each with a reference that the object
/// owns.
#[derive(Default)]
struct ShaderObjectState {
    entry_points: RefCell<Vec<NonNull<IShaderObject>>>,
    objects: RefCell<Vec<SubObject>>,
    held: RefCell<Vec<NonNull<IResource>>>,
}

impl Drop for ShaderObjectState {
    fn drop(&mut self) {
        for entry_point in self.entry_points.take() {
            // SAFETY: the object owns one reference to each entry point, and
            // releases it once, here.
            unsafe { IShaderObject::release(entry_point.as_ptr()) };
        }
        for (_, object) in self.objects.take() {
            // SAFETY: the object owns one reference to each object of a
            // parameter, and releases it once, here.
            unsafe { IShaderObject::release(object.as_ptr()) };
        }
        for resource in self.held.take() {
            // SAFETY: the object owns one reference to each resource that a
            // binding named, and releases it once, here.
            unsafe { IResource::release(resource.as_ptr()) };
        }
    }
}

/// A fake `IShaderObject`.
pub(crate) struct FakeShaderObject;

static SHADER_OBJECT_VTABLE: Shared<IShaderObjectVtbl> = Shared(IShaderObjectVtbl {
    base: unknown_vtable::<ShaderObjectState>(),
    getElementTypeLayout: unwinding1(unexpected1),
    getContainerType: unwinding1(unexpected1),
    getEntryPointCount: unwinding1(shader_object_entry_point_count),
    getEntryPoint: unwinding3(shader_object_entry_point),
    setData: unwinding4(shader_object_set_data),
    getObject: unwinding3(shader_object_object),
    setObject: unwinding3(unexpected3),
    setBinding: unwinding3(shader_object_set_binding),
    setDescriptorHandle: unwinding3(unexpected3),
    reserveData: unwinding4(unexpected4),
    setSpecializationArgs: unwinding4(unexpected4),
    getRawData: unwinding1(unexpected1),
    getSize: unwinding1(unexpected1),
    setConstantBufferOverride: unwinding2(unexpected2),
    finalize: unwinding1(unexpected1),
    isFinalized: unwinding1(unexpected1),
});

type ShaderObjectObject = FakeObject<ShaderObjectState>;

fn offset_tuple(offset: &ShaderOffset) -> (u32, u32, u32) {
    (
        offset.uniformOffset,
        offset.bindingRangeIndex,
        offset.bindingArrayIndex,
    )
}

unsafe extern "system" fn shader_object_entry_point_count(this: *mut IShaderObject) -> u32 {
    // SAFETY: `SHADER_OBJECT_VTABLE` belongs to a live fake shader object
    // only.
    let object = unsafe { ShaderObjectObject::from_this(this) };
    u32::try_from(object.state.entry_points.borrow().len()).unwrap_or(u32::MAX)
}

unsafe extern "system" fn shader_object_entry_point(
    this: *mut IShaderObject,
    index: u32,
    out: *mut *mut IShaderObject,
) -> SlangResult {
    // SAFETY: as in `shader_object_entry_point_count`.
    let object = unsafe { ShaderObjectObject::from_this(this) };
    let entry_point = usize::try_from(index)
        .ok()
        .and_then(|index| object.state.entry_points.borrow().get(index).copied());
    let Some(entry_point) = entry_point else {
        return SlangResult::SLANG_E_INVALID_ARG;
    };
    // SAFETY: the object owns a reference to the live entry point, and the
    // reference that this adds is the one that the caller receives.
    unsafe { IShaderObject::addRef(entry_point.as_ptr()) };
    // SAFETY: the crate passes a live out-pointer.
    unsafe { out.write(entry_point.as_ptr()) };
    SlangResult::SLANG_OK
}

unsafe extern "system" fn shader_object_object(
    this: *mut IShaderObject,
    offset: *const ShaderOffset,
    out: *mut *mut IShaderObject,
) -> SlangResult {
    // SAFETY: as in `shader_object_entry_point_count`.
    let object = unsafe { ShaderObjectObject::from_this(this) };
    // SAFETY: the crate passes a live offset.
    let offset = unsafe { &*offset };
    let key = (offset.bindingRangeIndex, offset.bindingArrayIndex);
    let found = object
        .state
        .objects
        .borrow()
        .iter()
        .find(|(position, _)| *position == key)
        .map(|(_, found)| *found);
    let raw = found.map_or(ptr::null_mut(), |found| {
        // SAFETY: the object owns a reference to the live object of the
        // parameter, and the reference that this adds is the one that the
        // caller receives.
        unsafe { IShaderObject::addRef(found.as_ptr()) };
        found.as_ptr()
    });
    // SAFETY: the crate passes a live out-pointer. Like slang-rhi, the fake
    // writes null and succeeds for an entry without an object.
    unsafe { out.write(raw) };
    SlangResult::SLANG_OK
}

unsafe extern "system" fn shader_object_set_data(
    this: *mut IShaderObject,
    offset: *const ShaderOffset,
    data: *const c_void,
    size: usize,
) -> SlangResult {
    // SAFETY: as in `shader_object_entry_point_count`.
    let object = unsafe { ShaderObjectObject::from_this(this) };
    // SAFETY: the crate passes a live offset.
    let offset = unsafe { &*offset };
    // SAFETY: the crate passes `size` readable bytes at `data`, as the
    // contract of `setData` requires.
    let data = unsafe { slice::from_raw_parts(data.cast::<u8>(), size) };
    object.header.log.push(Event::set_data(
        object.header.name,
        offset_tuple(offset),
        data,
    ));
    SlangResult::SLANG_OK
}

unsafe extern "system" fn shader_object_set_binding(
    this: *mut IShaderObject,
    offset: *const ShaderOffset,
    binding: *const Binding,
) -> SlangResult {
    // SAFETY: as in `shader_object_entry_point_count`.
    let object = unsafe { ShaderObjectObject::from_this(this) };
    // SAFETY: the crate passes a live offset.
    let offset = unsafe { &*offset };
    // SAFETY: the crate passes a live binding.
    let binding = unsafe { &*binding };
    let Some(resource) = NonNull::new(binding.resource) else {
        return SlangResult::SLANG_E_INVALID_ARG;
    };
    // SAFETY: the binding names a live fake object, to which the caller
    // keeps a reference through the call, and the reference that this adds
    // is the one that the shader object holds, as slang-rhi does.
    unsafe { IResource::addRef(resource.as_ptr()) };
    object.state.held.borrow_mut().push(resource);
    // SAFETY: the resource is a live fake object.
    let resource = unsafe { header(resource.as_ptr()) };
    object.header.log.push(Event::set_binding(
        object.header.name,
        offset_tuple(offset),
        resource.name,
        resource.count.get(),
    ));
    SlangResult::SLANG_OK
}

impl FakeShaderObject {
    /// Creates a fake shader object with one reference, which the caller
    /// owns.
    pub(crate) fn create(log: &Log, name: &'static str) -> NonNull<IShaderObject> {
        FakeObject::create(
            (&raw const SHADER_OBJECT_VTABLE).cast::<c_void>(),
            log,
            name,
            ShaderObjectState::default(),
        )
    }

    /// Adds `entry_point` as the next entry point of `root`, which takes a
    /// reference of its own to it.
    pub(crate) fn add_entry_point(
        root: NonNull<IShaderObject>,
        entry_point: NonNull<IShaderObject>,
    ) {
        // SAFETY: the test passes two live fake shader objects, and the
        // reference that this adds is the one that `root` owns.
        unsafe { IShaderObject::addRef(entry_point.as_ptr()) };
        // SAFETY: `root` is a live fake shader object.
        let root = unsafe { ShaderObjectObject::from_this(root.as_ptr()) };
        root.state.entry_points.borrow_mut().push(entry_point);
    }

    /// Adds `object` as the object of the parameter at the binding range
    /// `range` and the array index `index` of `parent`, which takes a
    /// reference of its own to it.
    pub(crate) fn add_object(
        parent: NonNull<IShaderObject>,
        (range, index): (u32, u32),
        object: NonNull<IShaderObject>,
    ) {
        // SAFETY: the test passes two live fake shader objects, and the
        // reference that this adds is the one that `parent` owns.
        unsafe { IShaderObject::addRef(object.as_ptr()) };
        // SAFETY: `parent` is a live fake shader object.
        let parent = unsafe { ShaderObjectObject::from_this(parent.as_ptr()) };
        parent
            .state
            .objects
            .borrow_mut()
            .push(((range, index), object));
    }

    /// Releases one reference of a fake shader object.
    ///
    /// # Safety
    ///
    /// `raw` is a live fake shader object, and the caller owns the reference
    /// that this releases. Breaking either is undefined behaviour.
    pub(crate) unsafe fn release_raw(raw: NonNull<IShaderObject>) {
        // SAFETY: the caller guarantees both conditions of `release`.
        unsafe { IShaderObject::release(raw.as_ptr()) };
    }
}

/// The state of a fake compute pass encoder: its root shader object, which
/// it owns, and whether `bindPipeline` returns null.
struct PassState {
    root: NonNull<IShaderObject>,
    null_binds: Cell<bool>,
}

impl Drop for PassState {
    fn drop(&mut self) {
        // SAFETY: the pass owns one reference to its root object and releases
        // it once, here.
        unsafe { IShaderObject::release(self.root.as_ptr()) };
    }
}

/// A fake `IComputePassEncoder`.
pub(crate) struct FakePass;

static PASS_VTABLE: Shared<IComputePassEncoderVtbl> = Shared(IComputePassEncoderVtbl {
    base: IPassEncoderVtbl {
        base: unknown_vtable::<PassState>(),
        pushDebugGroup: unwinding3(unexpected3),
        popDebugGroup: unwinding1(unexpected1),
        insertDebugMarker: unwinding3(unexpected3),
        writeTimestamp: unwinding3(unexpected3),
        end: unwinding1(pass_end),
    },
    bindPipeline: unwinding2(pass_bind_pipeline),
    bindPipeline_root: unwinding3(unexpected3),
    dispatchCompute: unwinding4(pass_dispatch),
    dispatchComputeIndirect: ptr::null(),
});

type PassObject = FakeObject<PassState>;

unsafe extern "system" fn pass_end(this: *mut IPassEncoder) {
    // SAFETY: `PASS_VTABLE` belongs to a live fake pass only.
    let header = unsafe { header(this) };
    header.log.push(Event::call(header.name, "end"));
}

unsafe extern "system" fn pass_bind_pipeline(
    this: *mut IComputePassEncoder,
    pipeline: *mut IComputePipeline,
) -> *mut IShaderObject {
    // SAFETY: `PASS_VTABLE` belongs to a live fake pass only.
    let object = unsafe { PassObject::from_this(this) };
    object
        .header
        .log
        .push(Event::bind_pipeline(object.header.name, pipeline.addr()));
    if object.state.null_binds.get() {
        ptr::null_mut()
    } else {
        object.state.root.as_ptr()
    }
}

unsafe extern "system" fn pass_dispatch(this: *mut IComputePassEncoder, x: u32, y: u32, z: u32) {
    // SAFETY: `PASS_VTABLE` belongs to a live fake pass only.
    let header = unsafe { header(this) };
    header.log.push(Event::dispatch(header.name, [x, y, z]));
}

impl FakePass {
    /// Creates a fake compute pass encoder with one reference, which the
    /// caller owns, and a root shader object named `root`.
    pub(crate) fn create(log: &Log, name: &'static str) -> NonNull<IComputePassEncoder> {
        let root = FakeShaderObject::create(&Log::new(), "root");
        FakeObject::create(
            (&raw const PASS_VTABLE).cast::<c_void>(),
            log,
            name,
            PassState {
                root,
                null_binds: Cell::new(false),
            },
        )
    }

    /// Makes `bindPipeline` of the fake pass return null.
    pub(crate) fn answer_binds_with_null(raw: NonNull<IComputePassEncoder>) {
        // SAFETY: the test passes a live fake pass.
        let object = unsafe { PassObject::from_this(raw.as_ptr()) };
        object.state.null_binds.set(true);
    }

    /// Releases one reference of a fake pass.
    ///
    /// # Safety
    ///
    /// `raw` is a live fake pass, and the caller owns the reference that this
    /// releases. Breaking either is undefined behaviour.
    pub(crate) unsafe fn release_raw(raw: NonNull<IComputePassEncoder>) {
        // SAFETY: the caller guarantees both conditions of `release`.
        unsafe { IComputePassEncoder::release(raw.as_ptr()) };
    }
}

/// A fake `ICommandBuffer`, which holds nothing.
pub(crate) struct FakeCommandBuffer;

static COMMAND_BUFFER_VTABLE: Shared<ICommandBufferVtbl> = Shared(ICommandBufferVtbl {
    base: unknown_vtable::<()>(),
    getDesc: unwinding1(unexpected1),
    getNativeHandle: unwinding2(unexpected2),
});

impl FakeCommandBuffer {
    /// Creates a fake command buffer with one reference, which the caller
    /// owns.
    pub(crate) fn create(log: &Log, name: &'static str) -> NonNull<ICommandBuffer> {
        FakeObject::create(
            (&raw const COMMAND_BUFFER_VTABLE).cast::<c_void>(),
            log,
            name,
            (),
        )
    }
}

/// The state of a fake command queue: the command buffers that it received,
/// each with a reference that the queue owns, as slang-rhi keeps one until a
/// command buffer has run.
#[derive(Default)]
struct QueueState {
    in_flight: RefCell<Vec<NonNull<ICommandBuffer>>>,
}

impl Drop for QueueState {
    fn drop(&mut self) {
        for command_buffer in self.in_flight.take() {
            // SAFETY: the queue owns one reference to each command buffer that
            // it received, and releases it once, here.
            unsafe { ICommandBuffer::release(command_buffer.as_ptr()) };
        }
    }
}

/// A fake `ICommandQueue`.
pub(crate) struct FakeQueue;

static QUEUE_VTABLE: Shared<ICommandQueueVtbl> = Shared(ICommandQueueVtbl {
    base: unknown_vtable::<QueueState>(),
    getType: unwinding1(unexpected1),
    createCommandEncoder: unwinding3(unexpected3),
    submit: unwinding2(queue_submit),
    waitOnHost: unwinding1(unexpected1),
    getNativeHandle: unwinding2(unexpected2),
    getTimestampCalibration: unwinding2(unexpected2),
});

type QueueObject = FakeObject<QueueState>;

unsafe extern "system" fn queue_submit(
    this: *mut ICommandQueue,
    desc: *const SubmitDesc,
) -> SlangResult {
    // SAFETY: `QUEUE_VTABLE` belongs to a live fake queue only.
    let object = unsafe { QueueObject::from_this(this) };
    // SAFETY: the crate passes a live descriptor.
    let desc = unsafe { &*desc };
    let count = usize::try_from(desc.commandBufferCount).unwrap_or(usize::MAX);
    let command_buffers = if count == 0 {
        &[][..]
    } else {
        // SAFETY: the crate passes `count` command buffer pointers at
        // `commandBuffers`, as the contract of `submit` requires.
        unsafe { slice::from_raw_parts(desc.commandBuffers, count) }
    };
    let mut names = Vec::new();
    for command_buffer in command_buffers {
        let Some(command_buffer) = NonNull::new(*command_buffer) else {
            return SlangResult::SLANG_E_INVALID_ARG;
        };
        // SAFETY: the descriptor names live fake command buffers, to which
        // the caller keeps a reference through the call, and the reference
        // that this adds is the one that the queue holds, as slang-rhi does.
        unsafe { ICommandBuffer::addRef(command_buffer.as_ptr()) };
        object.state.in_flight.borrow_mut().push(command_buffer);
        // SAFETY: the command buffer is a live fake object.
        names.push(unsafe { header(command_buffer.as_ptr()) }.name);
    }
    object
        .header
        .log
        .push(Event::submit(object.header.name, &names));
    SlangResult::SLANG_OK
}

impl FakeQueue {
    /// Creates a fake command queue with one reference, which the caller
    /// owns.
    pub(crate) fn create(log: &Log, name: &'static str) -> NonNull<ICommandQueue> {
        FakeObject::create(
            (&raw const QUEUE_VTABLE).cast::<c_void>(),
            log,
            name,
            QueueState::default(),
        )
    }

    /// Releases one reference of a fake queue.
    ///
    /// # Safety
    ///
    /// `raw` is a live fake queue, and the caller owns the reference that
    /// this releases. Breaking either is undefined behaviour.
    pub(crate) unsafe fn release_raw(raw: NonNull<ICommandQueue>) {
        // SAFETY: the caller guarantees both conditions of `release`.
        unsafe { ICommandQueue::release(raw.as_ptr()) };
    }
}
