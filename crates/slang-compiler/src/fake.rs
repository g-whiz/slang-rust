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
use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::ptr::{self, NonNull};
use std::rc::Rc;
use std::slice;

use slang_compiler_sys::{
    BuiltinModuleName, CompileCoreModuleFlags, CompilerOptionEntry, CompilerOptionValueKind,
    ContainerType, DeclReflection, ICompileRequest, IComponentType, IGlobalSession,
    IGlobalSessionVtbl, IModule, ISession, ISessionVtbl, ISlangBlob, ISlangBlobVtbl,
    ISlangSharedLibraryLoader, ISlangUnknown, ISlangUnknownVtbl, ITypeConformance, Interface,
    LayoutRules, SLANG_BUILD_TAG, SessionDesc, SlangArchiveType, SlangCapabilityID,
    SlangCompileRequest, SlangCompileTarget, SlangInt, SlangPassThrough, SlangProfileID,
    SlangResult, SlangSourceLanguage, SlangUUID, SourceLocation, SpecializationArg, TargetDesc,
    TypeLayoutReflection, TypeReflection,
};

use crate::owned::Owned;

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
    Query {
        object: &'static str,
        found: bool,
    },
    CheckTarget {
        object: &'static str,
        target: i32,
    },
    FindProfile {
        object: &'static str,
        name: String,
    },
    FindCapability {
        object: &'static str,
        name: String,
    },
    CreateSession {
        object: &'static str,
        desc: SessionSnapshot,
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

    pub(crate) fn query(object: &'static str, found: bool) -> Self {
        Self::Query { object, found }
    }

    pub(crate) fn check_target(object: &'static str, target: i32) -> Self {
        Self::CheckTarget { object, target }
    }

    pub(crate) fn find_profile(object: &'static str, name: &str) -> Self {
        Self::FindProfile {
            object,
            name: name.to_owned(),
        }
    }

    pub(crate) fn find_capability(object: &'static str, name: &str) -> Self {
        Self::FindCapability {
            object,
            name: name.to_owned(),
        }
    }
}

/// A compiler option entry as Slang reads it, with each string copied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum OptionSnapshot {
    /// The name and the two integers of an entry of kind `Int`.
    Int(i32, i32, i32),
    /// The name and the two strings of an entry of kind `String`.
    Text(i32, String, String),
}

/// A target descriptor as Slang reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TargetSnapshot {
    pub(crate) structure_size: usize,
    pub(crate) format: i32,
    pub(crate) profile: u32,
    pub(crate) flags: u32,
    pub(crate) floating_point_mode: u32,
    pub(crate) line_directive_mode: u32,
    pub(crate) force_glsl_scalar_layout: bool,
    pub(crate) options: Vec<OptionSnapshot>,
}

/// A session descriptor as Slang reads it, with every string and array
/// copied, so that a test can assert what the crate passed.
#[derive(Clone, Debug, PartialEq, Eq)]
#[expect(
    clippy::struct_excessive_bools,
    reason = "the snapshot copies the fields of Slang's session descriptor, which are bools"
)]
pub(crate) struct SessionSnapshot {
    pub(crate) structure_size: usize,
    pub(crate) targets: Vec<TargetSnapshot>,
    pub(crate) flags: u32,
    pub(crate) matrix_layout: u32,
    pub(crate) search_paths: Vec<String>,
    pub(crate) macros: Vec<(String, String)>,
    pub(crate) has_file_system: bool,
    pub(crate) enable_effect_annotations: bool,
    pub(crate) allow_glsl_syntax: bool,
    pub(crate) skip_spirv_validation: bool,
    pub(crate) options: Vec<OptionSnapshot>,
}

/// Reads a C string as Slang does, and copies it.
///
/// # Safety
///
/// `raw` is non-null and points to a live C string. Breaking either is
/// undefined behaviour.
unsafe fn read_text(raw: *const c_char) -> String {
    assert!(!raw.is_null(), "the crate passes no null string");
    // SAFETY: the caller guarantees that `raw` points to a live C string.
    let text = unsafe { CStr::from_ptr(raw) };
    text.to_string_lossy().into_owned()
}

/// Reads `count` elements from `raw` as Slang does, without touching `raw`
/// when `count` is zero.
///
/// # Safety
///
/// When `count` is not zero, `raw` points to `count` live, initialized
/// elements. Breaking this is undefined behaviour.
unsafe fn read_array<'a, T>(raw: *const T, count: usize) -> &'a [T] {
    if count == 0 {
        return &[];
    }
    assert!(
        !raw.is_null(),
        "the crate passes no null array with elements"
    );
    // SAFETY: the caller guarantees that `raw` points to `count` live
    // elements, and `raw` is non-null and aligned because it comes from a
    // `Vec` of the crate.
    unsafe { slice::from_raw_parts(raw, count) }
}

/// Reads compiler option entries as Slang does.
///
/// # Safety
///
/// When `count` is not zero, `raw` points to `count` live entries, and each
/// string of an entry is a live C string. Breaking this is undefined
/// behaviour.
pub(crate) unsafe fn read_options(
    raw: *const CompilerOptionEntry,
    count: u32,
) -> Vec<OptionSnapshot> {
    let count = usize::try_from(count).expect("a u32 fits in usize");
    // SAFETY: the caller guarantees that `raw` points to `count` entries.
    let entries = unsafe { read_array(raw, count) };
    entries
        .iter()
        .map(|entry| {
            let value = &entry.value;
            // SAFETY: the caller guarantees that every string of an entry is a
            // live C string, which SLC-16 requires for both kinds.
            let value0 = unsafe { read_text(value.stringValue0) };
            // SAFETY: as for `value0`.
            let value1 = unsafe { read_text(value.stringValue1) };
            match value.kind {
                CompilerOptionValueKind::Int => {
                    assert_eq!((value0.as_str(), value1.as_str()), ("", ""));
                    OptionSnapshot::Int(entry.name.0, value.intValue0, value.intValue1)
                }
                CompilerOptionValueKind::String => {
                    assert_eq!((value.intValue0, value.intValue1), (0, 0));
                    OptionSnapshot::Text(entry.name.0, value0, value1)
                }
                kind => panic!("unknown value kind {kind:?}"),
            }
        })
        .collect()
}

impl SessionSnapshot {
    /// Reads a session descriptor as Slang does.
    ///
    /// # Safety
    ///
    /// Every pointer of `desc` and of its targets points to live data of the
    /// count that the descriptor states. Breaking this is undefined
    /// behaviour.
    unsafe fn read(desc: &SessionDesc) -> Self {
        let target_count = usize::try_from(desc.targetCount).expect("a non-negative count");
        // SAFETY: the caller guarantees that `targets` points to the targets.
        let targets = unsafe { read_array::<TargetDesc>(desc.targets, target_count) };
        let path_count = usize::try_from(desc.searchPathCount).expect("a non-negative count");
        // SAFETY: the caller guarantees that `searchPaths` points to the paths.
        let paths = unsafe { read_array(desc.searchPaths, path_count) };
        let macro_count =
            usize::try_from(desc.preprocessorMacroCount).expect("a non-negative count");
        // SAFETY: the caller guarantees that `preprocessorMacros` points to the
        // macros.
        let macros = unsafe { read_array(desc.preprocessorMacros, macro_count) };
        Self {
            structure_size: desc.structureSize,
            targets: targets
                .iter()
                .map(|target| TargetSnapshot {
                    structure_size: target.structureSize,
                    format: target.format.0,
                    profile: target.profile.0,
                    flags: target.flags,
                    floating_point_mode: target.floatingPointMode.0,
                    line_directive_mode: target.lineDirectiveMode.0,
                    force_glsl_scalar_layout: target.forceGLSLScalarBufferLayout,
                    // SAFETY: the caller guarantees that the entries of each
                    // target are live.
                    options: unsafe {
                        read_options(
                            target.compilerOptionEntries,
                            target.compilerOptionEntryCount,
                        )
                    },
                })
                .collect(),
            flags: desc.flags,
            matrix_layout: desc.defaultMatrixLayoutMode.0,
            // SAFETY: the caller guarantees that each search path is a live C
            // string.
            search_paths: paths
                .iter()
                .map(|path| unsafe { read_text(*path) })
                .collect(),
            macros: macros
                .iter()
                .map(|definition| {
                    // SAFETY: the caller guarantees that each name is a live C
                    // string.
                    let name = unsafe { read_text(definition.name) };
                    // SAFETY: the caller guarantees that each value is a live C
                    // string.
                    let value = unsafe { read_text(definition.value) };
                    (name, value)
                })
                .collect(),
            has_file_system: !desc.fileSystem.is_null(),
            enable_effect_annotations: desc.enableEffectAnnotations,
            allow_glsl_syntax: desc.allowGLSLSyntax,
            skip_spirv_validation: desc.skipSPIRVValidation,
            // SAFETY: the caller guarantees that the session's entries are live.
            options: unsafe {
                read_options(desc.compilerOptionEntries, desc.compilerOptionEntryCount)
            },
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
    session_result: Cell<Option<SlangResult>>,
}

/// The profile that the fake global session knows, and its number.
pub(crate) const FAKE_PROFILE: (&str, u32) = ("fake_profile", 41);

/// The capability that the fake global session knows, and its number.
pub(crate) const FAKE_CAPABILITY: (&str, i32) = ("fake_capability", 42);

/// A fake `IGlobalSession`, whose only working methods are the counting
/// methods, `getBuildTagString`, `checkCompileTargetSupport`, `findProfile`
/// and `findCapability`, which know one name each, and `createSession`, which
/// records the descriptor and creates a [`FakeSession`].
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
            session_result: Cell::new(None),
        };
        GlobalSessionObject::create(
            &GLOBAL_SESSION_VTABLE,
            log,
            name,
            &GLOBAL_SESSION_INTERFACES,
            state,
        )
    }

    /// Makes every later `createSession` of `global` return `result` without
    /// writing a session.
    pub(crate) fn fail_create_session(global: NonNull<IGlobalSession>, result: SlangResult) {
        // SAFETY: `global` comes from `FakeGlobalSession::create`, and the
        // caller keeps it alive during this call.
        let object = unsafe { GlobalSessionObject::from_this(global.as_ptr()) };
        object.state.session_result.set(Some(result));
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
    this: *mut IGlobalSession,
    desc: *const SessionDesc,
    out: *mut *mut ISession,
) -> SlangResult {
    // SAFETY: `GLOBAL_SESSION_VTABLE` belongs to a live `GlobalSessionObject`
    // only.
    let object = unsafe { GlobalSessionObject::from_this(this) };
    // SAFETY: the crate passes a live descriptor, as `createSession` requires.
    let desc = unsafe { &*desc };
    // SAFETY: `createSession` requires every pointer of the descriptor to be
    // live for the call, which is what the tests check under Miri.
    let snapshot = unsafe { SessionSnapshot::read(desc) };
    object.log.push(Event::CreateSession {
        object: object.name,
        desc: snapshot,
    });
    if let Some(result) = object.state.session_result.get() {
        return result;
    }
    let global = NonNull::new(this).expect("the crate passes a non-null global session");
    let session = FakeSession::create(&object.log, global);
    // SAFETY: the crate passes a live out-pointer, as `createSession`
    // requires.
    unsafe { out.write(session.as_ptr()) };
    SlangResult::SLANG_OK
}

unsafe extern "system" fn find_profile(
    this: *mut IGlobalSession,
    name: *const c_char,
) -> SlangProfileID {
    // SAFETY: `GLOBAL_SESSION_VTABLE` belongs to a live `GlobalSessionObject`
    // only.
    let object = unsafe { GlobalSessionObject::from_this(this) };
    // SAFETY: the crate passes a live C string, as `findProfile` requires.
    let name = unsafe { read_text(name) };
    object.log.push(Event::find_profile(object.name, &name));
    if name == FAKE_PROFILE.0 {
        SlangProfileID(FAKE_PROFILE.1)
    } else {
        SlangProfileID::SLANG_PROFILE_UNKNOWN
    }
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
    this: *mut IGlobalSession,
    name: *const c_char,
) -> SlangCapabilityID {
    // SAFETY: `GLOBAL_SESSION_VTABLE` belongs to a live `GlobalSessionObject`
    // only.
    let object = unsafe { GlobalSessionObject::from_this(this) };
    // SAFETY: the crate passes a live C string, as `findCapability` requires.
    let name = unsafe { read_text(name) };
    object.log.push(Event::find_capability(object.name, &name));
    if name == FAKE_CAPABILITY.0 {
        SlangCapabilityID(FAKE_CAPABILITY.1)
    } else {
        SlangCapabilityID::SLANG_CAPABILITY_UNKNOWN
    }
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

/// The state of a fake session: the global session that it holds a counted
/// reference to, as Slang's session does, until it is freed.
struct SessionState {
    global: NonNull<IGlobalSession>,
}

impl Drop for SessionState {
    fn drop(&mut self) {
        // SAFETY: `FakeSession::create` added the reference that this releases,
        // and the global session is live because of it.
        unsafe { IGlobalSession::release(self.global.as_ptr()) };
    }
}

/// A fake `ISession`, whose only working methods are the counting methods
/// and `getGlobalSession`, and which holds its global session until it is
/// freed.
pub(crate) struct FakeSession;

/// The name of every fake session in the log.
pub(crate) const FAKE_SESSION: &str = "session";

type SessionObject = FakeObject<ISessionVtbl, SessionState>;

static SESSION_INTERFACES: [SlangUUID; 2] = [ISlangUnknown::UUID, ISession::UUID];

impl FakeSession {
    /// Creates a fake session with one reference, which the caller owns, that
    /// holds a new reference to `global`.
    pub(crate) fn create(log: &Log, global: NonNull<IGlobalSession>) -> NonNull<ISession> {
        // SAFETY: the caller passes a live global session, and the session
        // state owns the reference that this adds.
        unsafe { IGlobalSession::addRef(global.as_ptr()) };
        SessionObject::create(
            &SESSION_VTABLE,
            log,
            FAKE_SESSION,
            &SESSION_INTERFACES,
            SessionState { global },
        )
    }
}

unsafe extern "system" fn session_global_session(this: *mut ISession) -> *mut IGlobalSession {
    // SAFETY: `SESSION_VTABLE` belongs to a live `SessionObject` only.
    let object = unsafe { SessionObject::from_this(this) };
    object.state.global.as_ptr()
}

unsafe extern "system" fn session_load_module(
    _: *mut ISession,
    _: *const c_char,
    _: *mut *mut ISlangBlob,
) -> *mut IModule {
    ptr::null_mut()
}

unsafe extern "system" fn session_load_module_from_blob(
    _: *mut ISession,
    _: *const c_char,
    _: *const c_char,
    _: *mut ISlangBlob,
    _: *mut *mut ISlangBlob,
) -> *mut IModule {
    ptr::null_mut()
}

unsafe extern "system" fn session_create_composite(
    _: *mut ISession,
    _: *const *mut IComponentType,
    _: SlangInt,
    _: *mut *mut IComponentType,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn session_specialize_type(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *const SpecializationArg,
    _: SlangInt,
    _: *mut *mut ISlangBlob,
) -> *mut TypeReflection {
    ptr::null_mut()
}

unsafe extern "system" fn session_type_layout(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: SlangInt,
    _: LayoutRules,
    _: *mut *mut ISlangBlob,
) -> *mut TypeLayoutReflection {
    ptr::null_mut()
}

unsafe extern "system" fn session_container_type(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: ContainerType,
    _: *mut *mut ISlangBlob,
) -> *mut TypeReflection {
    ptr::null_mut()
}

unsafe extern "system" fn session_dynamic_type(_: *mut ISession) -> *mut TypeReflection {
    ptr::null_mut()
}

unsafe extern "system" fn session_mangled_name(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn session_witness_mangled_name(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut TypeReflection,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn session_witness_sequential_id(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut TypeReflection,
    _: *mut u32,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn session_create_compile_request(
    _: *mut ISession,
    _: *mut *mut SlangCompileRequest,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn session_create_type_conformance(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut TypeReflection,
    _: *mut *mut ITypeConformance,
    _: SlangInt,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn session_loaded_module_count(_: *mut ISession) -> SlangInt {
    0
}

unsafe extern "system" fn session_loaded_module(_: *mut ISession, _: SlangInt) -> *mut IModule {
    ptr::null_mut()
}

unsafe extern "system" fn session_is_binary_module_up_to_date(
    _: *mut ISession,
    _: *const c_char,
    _: *mut ISlangBlob,
) -> bool {
    false
}

unsafe extern "system" fn session_load_module_from_string(
    _: *mut ISession,
    _: *const c_char,
    _: *const c_char,
    _: *const c_char,
    _: *mut *mut ISlangBlob,
) -> *mut IModule {
    ptr::null_mut()
}

unsafe extern "system" fn session_rtti_bytes(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut TypeReflection,
    _: *mut u32,
    _: u32,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn session_module_info(
    _: *mut ISession,
    _: *mut ISlangBlob,
    _: *mut SlangInt,
    _: *mut *const c_char,
    _: *mut *const c_char,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system" fn session_decl_source_location(
    _: *mut ISession,
    _: *mut DeclReflection,
    _: *mut SourceLocation,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

static SESSION_VTABLE: ISessionVtbl = ISessionVtbl {
    base: unknown_vtable::<ISessionVtbl, SessionState>(),
    getGlobalSession: session_global_session,
    loadModule: session_load_module,
    loadModuleFromSource: session_load_module_from_blob,
    createCompositeComponentType: session_create_composite,
    specializeType: session_specialize_type,
    getTypeLayout: session_type_layout,
    getContainerType: session_container_type,
    getDynamicType: session_dynamic_type,
    getTypeRTTIMangledName: session_mangled_name,
    getTypeConformanceWitnessMangledName: session_witness_mangled_name,
    getTypeConformanceWitnessSequentialID: session_witness_sequential_id,
    createCompileRequest: session_create_compile_request,
    createTypeConformanceComponentType: session_create_type_conformance,
    loadModuleFromIRBlob: session_load_module_from_blob,
    getLoadedModuleCount: session_loaded_module_count,
    getLoadedModule: session_loaded_module,
    isBinaryModuleUpToDate: session_is_binary_module_up_to_date,
    loadModuleFromSourceString: session_load_module_from_string,
    getDynamicObjectRTTIBytes: session_rtti_bytes,
    loadModuleInfoFromIRBlob: session_module_info,
    getDeclSourceLocation: session_decl_source_location,
};
