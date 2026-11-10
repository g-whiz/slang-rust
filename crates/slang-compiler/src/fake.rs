//! A fake of the Slang interfaces for the tests under Miri (SLC-25).
//!
//! Each fake object is a `#[repr(C)]` struct whose first field points to a
//! static instance of the generated vtable struct of its interface, so a
//! pointer to it is a valid pointer to that interface. It counts its
//! references, frees itself when the count reaches zero, and records every
//! counting call, every `queryInterface`, every other call that the crate
//! makes, and its own release in a [`Log`] that the test reads.

use std::any::Any;
use std::cell::{Cell, RefCell};
use std::ffi::{CString, c_char, c_int, c_void};
use std::ptr::{self, NonNull};
use std::rc::Rc;

use slang_compiler_sys::{
    BuiltinModuleName, CompileCoreModuleFlags, ICompileRequest, IGlobalSession, IGlobalSessionVtbl,
    ISession, ISlangBlob, ISlangBlobVtbl, ISlangSharedLibraryLoader, ISlangUnknown,
    ISlangUnknownVtbl, Interface, SLANG_BUILD_TAG, SessionDesc, SlangArchiveType,
    SlangCapabilityID, SlangCompileTarget, SlangPassThrough, SlangProfileID, SlangResult,
    SlangSourceLanguage, SlangUUID,
};

use crate::owned::Owned;

/// One call that the crate made on a fake object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Event {
    AddRef { object: &'static str, count: u32 },
    Release { object: &'static str, count: u32 },
    Freed { object: &'static str },
    Query { object: &'static str, found: bool },
    CheckTarget { object: &'static str, target: i32 },
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

    pub(crate) fn query(object: &'static str, found: bool) -> Self {
        Self::Query { object, found }
    }

    pub(crate) fn check_target(object: &'static str, target: i32) -> Self {
        Self::CheckTarget { object, target }
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

/// Returns the message of a panic payload.
pub(crate) fn panic_message(payload: &(dyn Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_owned()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        String::from("<a payload that is not text>")
    }
}

#[repr(C)]
struct FakeObject<V: 'static, S> {
    vtable: &'static V,
    count: Cell<u32>,
    name: &'static str,
    log: Log,
    interfaces: &'static [SlangUUID],
    state: S,
}

impl<V: 'static, S> FakeObject<V, S> {
    fn create<I>(
        vtable: &'static V,
        log: &Log,
        name: &'static str,
        interfaces: &'static [SlangUUID],
        state: S,
    ) -> NonNull<I> {
        let object = Box::new(Self {
            vtable,
            count: Cell::new(1),
            name,
            log: log.clone(),
            interfaces,
            state,
        });
        NonNull::from(Box::leak(object)).cast::<I>()
    }

    /// Returns the fake object behind an interface pointer.
    ///
    /// # Safety
    ///
    /// `this` comes from [`FakeObject::create`] with the same `V` and `S`, and
    /// the object is not freed while the returned reference lives. Breaking
    /// either is undefined behaviour.
    unsafe fn from_this<'a, I>(this: *mut I) -> &'a Self {
        // SAFETY: the caller guarantees that `this` points to a live
        // `FakeObject<V, S>`, which `create` allocated as one `Box`, and the
        // object is only read through shared references and `Cell`s.
        unsafe { &*this.cast::<Self>() }
    }
}

fn same_uuid(left: &SlangUUID, right: &SlangUUID) -> bool {
    left.data1 == right.data1
        && left.data2 == right.data2
        && left.data3 == right.data3
        && left.data4 == right.data4
}

unsafe extern "system" fn query_interface<V: 'static, S>(
    this: *mut ISlangUnknown,
    uuid: *const SlangUUID,
    out: *mut *mut c_void,
) -> SlangResult {
    // SAFETY: every vtable of the fake that names this function belongs to a
    // `FakeObject<V, S>`, and Slang's contract, which the crate follows, keeps
    // the object alive during the call.
    let object = unsafe { FakeObject::<V, S>::from_this(this) };
    // SAFETY: the crate passes a live identifier, as `queryInterface` requires.
    let uuid = unsafe { &*uuid };
    let found = object.interfaces.iter().any(|known| same_uuid(known, uuid));
    object.log.push(Event::query(object.name, found));
    let result = if found {
        let count = object.count.get() + 1;
        object.count.set(count);
        object.log.push(Event::add_ref(object.name, count));
        this.cast::<c_void>()
    } else {
        ptr::null_mut()
    };
    // SAFETY: the crate passes a live out-pointer, as `queryInterface`
    // requires.
    unsafe { out.write(result) };
    if found {
        SlangResult::SLANG_OK
    } else {
        SlangResult::SLANG_E_NO_INTERFACE
    }
}

unsafe extern "system" fn add_ref<V: 'static, S>(this: *mut ISlangUnknown) -> u32 {
    // SAFETY: as in `query_interface`, `this` is a live `FakeObject<V, S>`.
    let object = unsafe { FakeObject::<V, S>::from_this(this) };
    let count = object.count.get() + 1;
    object.count.set(count);
    object.log.push(Event::add_ref(object.name, count));
    count
}

unsafe extern "system" fn release<V: 'static, S>(this: *mut ISlangUnknown) -> u32 {
    // SAFETY: as in `query_interface`, `this` is a live `FakeObject<V, S>`,
    // and the caller owns the reference that this releases.
    let object = unsafe { FakeObject::<V, S>::from_this(this) };
    let count = object.count.get() - 1;
    object.count.set(count);
    object.log.push(Event::release(object.name, count));
    if count == 0 {
        object.log.push(Event::freed(object.name));
        // SAFETY: the count reached zero, so no reference to the object
        // remains, and `create` allocated it as a `Box` of this type. The
        // shared reference `object` is not used after this point.
        drop(unsafe { Box::from_raw(this.cast::<FakeObject<V, S>>()) });
    }
    count
}

const fn unknown_vtable<V: 'static, S>() -> ISlangUnknownVtbl {
    ISlangUnknownVtbl {
        queryInterface: query_interface::<V, S>,
        addRef: add_ref::<V, S>,
        release: release::<V, S>,
    }
}

/// The state of a fake blob: its bytes, and an object that it holds until it
/// is freed, which models a destructor that releases another Slang object.
struct BlobState {
    bytes: Vec<u8>,
    held: RefCell<Option<Owned<ISlangBlob>>>,
}

/// A fake `ISlangBlob`.
pub(crate) struct FakeBlob;

static BLOB_INTERFACES: [SlangUUID; 2] = [ISlangUnknown::UUID, ISlangBlob::UUID];

static BLOB_VTABLE: ISlangBlobVtbl = ISlangBlobVtbl {
    base: unknown_vtable::<ISlangBlobVtbl, BlobState>(),
    getBufferPointer: blob_buffer_pointer,
    getBufferSize: blob_buffer_size,
};

type BlobObject = FakeObject<ISlangBlobVtbl, BlobState>;

unsafe extern "system" fn blob_buffer_pointer(this: *mut ISlangBlob) -> *const c_void {
    // SAFETY: `BLOB_VTABLE` belongs to a live `BlobObject` only.
    let object = unsafe { BlobObject::from_this(this) };
    object.state.bytes.as_ptr().cast::<c_void>()
}

unsafe extern "system" fn blob_buffer_size(this: *mut ISlangBlob) -> usize {
    // SAFETY: `BLOB_VTABLE` belongs to a live `BlobObject` only.
    let object = unsafe { BlobObject::from_this(this) };
    object.state.bytes.len()
}

impl FakeBlob {
    /// Creates a fake blob with one reference, which the caller owns.
    pub(crate) fn create(log: &Log, name: &'static str, bytes: &[u8]) -> NonNull<ISlangBlob> {
        let state = BlobState {
            bytes: bytes.to_vec(),
            held: RefCell::new(None),
        };
        BlobObject::create(&BLOB_VTABLE, log, name, &BLOB_INTERFACES, state)
    }

    /// Makes the fake blob `blob` hold `held` until it is freed.
    pub(crate) fn hold_until_freed(blob: NonNull<ISlangBlob>, held: Owned<ISlangBlob>) {
        // SAFETY: `blob` comes from `FakeBlob::create`, and the caller keeps it
        // alive during this call.
        let object = unsafe { BlobObject::from_this(blob.as_ptr()) };
        *object.state.held.borrow_mut() = Some(held);
    }

    /// Releases one reference of a fake blob through its vtable.
    ///
    /// # Safety
    ///
    /// `blob` comes from [`FakeBlob::create`], is live, and the caller owns
    /// the reference that this releases. Breaking either is undefined
    /// behaviour.
    pub(crate) unsafe fn release_raw(blob: NonNull<ISlangBlob>) {
        // SAFETY: the caller guarantees that `blob` is a live fake blob and
        // owns the reference that this releases.
        unsafe { ISlangBlob::release(blob.as_ptr()) };
    }
}

/// The state of a fake global session: its build tag and the result of every
/// check of target support.
struct GlobalSessionState {
    build_tag: CString,
    target_support: SlangResult,
}

/// A fake `IGlobalSession`, whose only working methods are the counting
/// methods, `getBuildTagString`, and `checkCompileTargetSupport`.
pub(crate) struct FakeGlobalSession;

type GlobalSessionObject = FakeObject<IGlobalSessionVtbl, GlobalSessionState>;

static GLOBAL_SESSION_INTERFACES: [SlangUUID; 2] = [ISlangUnknown::UUID, IGlobalSession::UUID];

impl FakeGlobalSession {
    /// Creates a fake global session with one reference, which the caller
    /// owns, whose checks of target support return `target_support`.
    pub(crate) fn create(
        log: &Log,
        name: &'static str,
        target_support: SlangResult,
    ) -> NonNull<IGlobalSession> {
        let state = GlobalSessionState {
            build_tag: CString::new(SLANG_BUILD_TAG).unwrap_or_default(),
            target_support,
        };
        GlobalSessionObject::create(
            &GLOBAL_SESSION_VTABLE,
            log,
            name,
            &GLOBAL_SESSION_INTERFACES,
            state,
        )
    }
}

unsafe extern "system" fn build_tag_string(this: *mut IGlobalSession) -> *const c_char {
    // SAFETY: `GLOBAL_SESSION_VTABLE` belongs to a live `GlobalSessionObject`
    // only.
    let object = unsafe { GlobalSessionObject::from_this(this) };
    object.state.build_tag.as_ptr()
}

unsafe extern "system" fn check_compile_target_support(
    this: *mut IGlobalSession,
    target: SlangCompileTarget,
) -> SlangResult {
    // SAFETY: `GLOBAL_SESSION_VTABLE` belongs to a live `GlobalSessionObject`
    // only.
    let object = unsafe { GlobalSessionObject::from_this(this) };
    object.log.push(Event::check_target(object.name, target.0));
    object.state.target_support
}

unsafe extern "system" fn create_session(
    _: *mut IGlobalSession,
    _: *const SessionDesc,
    _: *mut *mut ISession,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn find_profile(_: *mut IGlobalSession, _: *const c_char) -> SlangProfileID {
    SlangProfileID(0)
}

unsafe extern "system" fn set_pass_through_text(
    _: *mut IGlobalSession,
    _: SlangPassThrough,
    _: *const c_char,
) {
}

unsafe extern "system" fn get_pass_through_blob(
    _: *mut IGlobalSession,
    _: SlangPassThrough,
    _: *mut *mut ISlangBlob,
) {
}

unsafe extern "system" fn set_default_downstream_compiler(
    _: *mut IGlobalSession,
    _: SlangSourceLanguage,
    _: SlangPassThrough,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn get_default_downstream_compiler(
    _: *mut IGlobalSession,
    _: SlangSourceLanguage,
) -> SlangPassThrough {
    SlangPassThrough(0)
}

unsafe extern "system" fn set_language_prelude(
    _: *mut IGlobalSession,
    _: SlangSourceLanguage,
    _: *const c_char,
) {
}

unsafe extern "system" fn get_language_prelude(
    _: *mut IGlobalSession,
    _: SlangSourceLanguage,
    _: *mut *mut ISlangBlob,
) {
}

unsafe extern "system" fn create_compile_request(
    _: *mut IGlobalSession,
    _: *mut *mut ICompileRequest,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn add_builtins(_: *mut IGlobalSession, _: *const c_char, _: *const c_char) {
}

unsafe extern "system" fn set_shared_library_loader(
    _: *mut IGlobalSession,
    _: *mut ISlangSharedLibraryLoader,
) {
}

unsafe extern "system" fn get_shared_library_loader(
    _: *mut IGlobalSession,
) -> *mut ISlangSharedLibraryLoader {
    ptr::null_mut()
}

unsafe extern "system" fn check_pass_through_support(
    _: *mut IGlobalSession,
    _: SlangPassThrough,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn compile_core_module(
    _: *mut IGlobalSession,
    _: CompileCoreModuleFlags,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn load_core_module(
    _: *mut IGlobalSession,
    _: *const c_void,
    _: usize,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn save_core_module(
    _: *mut IGlobalSession,
    _: SlangArchiveType,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn find_capability(
    _: *mut IGlobalSession,
    _: *const c_char,
) -> SlangCapabilityID {
    SlangCapabilityID(0)
}

unsafe extern "system" fn set_downstream_compiler_for_transition(
    _: *mut IGlobalSession,
    _: SlangCompileTarget,
    _: SlangCompileTarget,
    _: SlangPassThrough,
) {
}

unsafe extern "system" fn get_downstream_compiler_for_transition(
    _: *mut IGlobalSession,
    _: SlangCompileTarget,
    _: SlangCompileTarget,
) -> SlangPassThrough {
    SlangPassThrough(0)
}

unsafe extern "system" fn get_compiler_elapsed_time(
    _: *mut IGlobalSession,
    _: *mut f64,
    _: *mut f64,
) {
}

unsafe extern "system" fn set_spirv_core_grammar(
    _: *mut IGlobalSession,
    _: *const c_char,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn parse_command_line_arguments(
    _: *mut IGlobalSession,
    _: c_int,
    _: *const *const c_char,
    _: *mut SessionDesc,
    _: *mut *mut ISlangUnknown,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn get_session_desc_digest(
    _: *mut IGlobalSession,
    _: *mut SessionDesc,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn compile_builtin_module(
    _: *mut IGlobalSession,
    _: BuiltinModuleName,
    _: CompileCoreModuleFlags,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn load_builtin_module(
    _: *mut IGlobalSession,
    _: BuiltinModuleName,
    _: *const c_void,
    _: usize,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn save_builtin_module(
    _: *mut IGlobalSession,
    _: BuiltinModuleName,
    _: SlangArchiveType,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn get_downstream_compiler_path(
    _: *mut IGlobalSession,
    _: SlangPassThrough,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

static GLOBAL_SESSION_VTABLE: IGlobalSessionVtbl = IGlobalSessionVtbl {
    base: unknown_vtable::<IGlobalSessionVtbl, GlobalSessionState>(),
    createSession: create_session,
    findProfile: find_profile,
    setDownstreamCompilerPath: set_pass_through_text,
    setDownstreamCompilerPrelude: set_pass_through_text,
    getDownstreamCompilerPrelude: get_pass_through_blob,
    getBuildTagString: build_tag_string,
    setDefaultDownstreamCompiler: set_default_downstream_compiler,
    getDefaultDownstreamCompiler: get_default_downstream_compiler,
    setLanguagePrelude: set_language_prelude,
    getLanguagePrelude: get_language_prelude,
    createCompileRequest: create_compile_request,
    addBuiltins: add_builtins,
    setSharedLibraryLoader: set_shared_library_loader,
    getSharedLibraryLoader: get_shared_library_loader,
    checkCompileTargetSupport: check_compile_target_support,
    checkPassThroughSupport: check_pass_through_support,
    compileCoreModule: compile_core_module,
    loadCoreModule: load_core_module,
    saveCoreModule: save_core_module,
    findCapability: find_capability,
    setDownstreamCompilerForTransition: set_downstream_compiler_for_transition,
    getDownstreamCompilerForTransition: get_downstream_compiler_for_transition,
    getCompilerElapsedTime: get_compiler_elapsed_time,
    setSPIRVCoreGrammar: set_spirv_core_grammar,
    parseCommandLineArguments: parse_command_line_arguments,
    getSessionDescDigest: get_session_desc_digest,
    compileBuiltinModule: compile_builtin_module,
    loadBuiltinModule: load_builtin_module,
    saveBuiltinModule: save_builtin_module,
    getDownstreamCompilerPath: get_downstream_compiler_path,
};
