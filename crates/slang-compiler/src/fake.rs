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
    ContainerType, DeclReflection, FunctionReflection, ICompileRequest, IComponentType,
    IComponentTypeVtbl, IEntryPoint, IEntryPointVtbl, IGlobalSession, IGlobalSessionVtbl,
    IMetadata, IMetadataVtbl, IModule, IModuleVtbl, ISession, ISessionVtbl, ISlangBlob,
    ISlangBlobVtbl, ISlangCastable, ISlangCastableVtbl, ISlangMutableFileSystem,
    ISlangSharedLibrary, ISlangSharedLibraryLoader, ISlangUnknown, ISlangUnknownVtbl,
    ITypeConformance, Interface, LayoutRules, ProgramLayout, SLANG_BUILD_TAG, SessionDesc,
    SlangArchiveType, SlangCapabilityID, SlangCompileRequest, SlangCompileTarget, SlangInt,
    SlangInt32, SlangParameterCategory, SlangPassThrough, SlangProfileID, SlangResult,
    SlangSourceLanguage, SlangStage, SlangUInt, SlangUUID, SourceLocation, SpecializationArg,
    TargetDesc, TypeLayoutReflection, TypeReflection,
};

use crate::owned::Owned;
use crate::{GlobalSession, Session, Target, TargetFormat};

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
    Call {
        object: &'static str,
        method: &'static str,
    },
    LinkWithOptions {
        object: &'static str,
        options: Vec<OptionSnapshot>,
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

    pub(crate) fn call(object: &'static str, method: &'static str) -> Self {
        Self::Call { object, method }
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
    null_queries: Cell<bool>,
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
            null_queries: Cell::new(false),
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

unsafe extern "system-unwind" fn query_interface<V: 'static, S>(
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
    if object.null_queries.get() {
        // SAFETY: the crate passes a live out-pointer, as `queryInterface`
        // requires.
        unsafe { out.write(ptr::null_mut()) };
        return SlangResult::SLANG_OK;
    }
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

unsafe extern "system-unwind" fn add_ref<V: 'static, S>(this: *mut ISlangUnknown) -> u32 {
    // SAFETY: as in `query_interface`, `this` is a live `FakeObject<V, S>`.
    let object = unsafe { FakeObject::<V, S>::from_this(this) };
    let count = object.count.get() + 1;
    object.count.set(count);
    object.log.push(Event::add_ref(object.name, count));
    count
}

unsafe extern "system-unwind" fn release<V: 'static, S>(this: *mut ISlangUnknown) -> u32 {
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

/// The state of a fake blob: its bytes, what it reports instead of them, and
/// an object that it holds until it is freed, which models a destructor that
/// releases another Slang object.
///
/// The bytes start `offset` bytes into storage of words, so that their
/// alignment does not depend on the allocator.
struct BlobState {
    words: Vec<u32>,
    offset: usize,
    len: usize,
    reported: Cell<Option<Reported>>,
    held: RefCell<Option<Owned<ISlangBlob>>>,
}

/// What a fake blob reports instead of its bytes.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Reported {
    /// A null pointer with this size.
    Null(usize),
    /// Its own pointer with this size.
    Size(usize),
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

unsafe extern "system-unwind" fn blob_buffer_pointer(this: *mut ISlangBlob) -> *const c_void {
    // SAFETY: `BLOB_VTABLE` belongs to a live `BlobObject` only.
    let object = unsafe { BlobObject::from_this(this) };
    match object.state.reported.get() {
        Some(Reported::Null(_)) => ptr::null(),
        Some(Reported::Size(_)) | None => object
            .state
            .words
            .as_ptr()
            .cast::<u8>()
            .wrapping_add(object.state.offset)
            .cast::<c_void>(),
    }
}

unsafe extern "system-unwind" fn blob_buffer_size(this: *mut ISlangBlob) -> usize {
    // SAFETY: `BLOB_VTABLE` belongs to a live `BlobObject` only.
    let object = unsafe { BlobObject::from_this(this) };
    match object.state.reported.get() {
        Some(Reported::Null(size) | Reported::Size(size)) => size,
        None => object.state.len,
    }
}

impl FakeBlob {
    /// Creates a fake blob with one reference, which the caller owns.
    pub(crate) fn create(log: &Log, name: &'static str, bytes: &[u8]) -> NonNull<ISlangBlob> {
        Self::create_at(log, name, bytes, 0)
    }

    /// Creates a fake blob with one reference, which the caller owns, whose
    /// bytes start one byte past an address aligned for `u32`.
    pub(crate) fn create_misaligned(
        log: &Log,
        name: &'static str,
        bytes: &[u8],
    ) -> NonNull<ISlangBlob> {
        Self::create_at(log, name, bytes, 1)
    }

    fn create_at(
        log: &Log,
        name: &'static str,
        bytes: &[u8],
        offset: usize,
    ) -> NonNull<ISlangBlob> {
        let mut padded = vec![0; offset];
        padded.extend_from_slice(bytes);
        padded.resize(padded.len().next_multiple_of(4), 0);
        let words = padded
            .as_chunks::<4>()
            .0
            .iter()
            .map(|word| u32::from_ne_bytes(*word))
            .collect();
        let state = BlobState {
            words,
            offset,
            len: bytes.len(),
            reported: Cell::new(None),
            held: RefCell::new(None),
        };
        BlobObject::create(&BLOB_VTABLE, log, name, &BLOB_INTERFACES, state)
    }

    /// Makes every `queryInterface` of the fake blob `blob` succeed with a
    /// null output.
    pub(crate) fn answer_queries_with_null(blob: NonNull<ISlangBlob>) {
        // SAFETY: `blob` comes from `FakeBlob::create`, and the caller keeps it
        // alive during this call.
        let object = unsafe { BlobObject::from_this(blob.as_ptr()) };
        object.null_queries.set(true);
    }

    /// Makes the fake blob `blob` report `reported` instead of its bytes.
    pub(crate) fn report(blob: NonNull<ISlangBlob>, reported: Reported) {
        // SAFETY: `blob` comes from `FakeBlob::create`, and the caller keeps it
        // alive during this call.
        let object = unsafe { BlobObject::from_this(blob.as_ptr()) };
        object.state.reported.set(Some(reported));
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

/// The state of a fake global session: its build tag, the result of every
/// check of target support, and the counter that a session's destructor
/// increments, as Slang's does.
struct GlobalSessionState {
    build_tag: CString,
    target_support: SlangResult,
    session_result: Cell<Option<SlangResult>>,
    epoch: Cell<u64>,
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
            epoch: Cell::new(0),
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

unsafe extern "system-unwind" fn build_tag_string(this: *mut IGlobalSession) -> *const c_char {
    // SAFETY: `GLOBAL_SESSION_VTABLE` belongs to a live `GlobalSessionObject`
    // only.
    let object = unsafe { GlobalSessionObject::from_this(this) };
    object.state.build_tag.as_ptr()
}

unsafe extern "system-unwind" fn check_compile_target_support(
    this: *mut IGlobalSession,
    target: SlangCompileTarget,
) -> SlangResult {
    // SAFETY: `GLOBAL_SESSION_VTABLE` belongs to a live `GlobalSessionObject`
    // only.
    let object = unsafe { GlobalSessionObject::from_this(this) };
    object.log.push(Event::check_target(object.name, target.0));
    object.state.target_support
}

unsafe extern "system-unwind" fn create_session(
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

unsafe extern "system-unwind" fn find_profile(
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

unsafe extern "system-unwind" fn set_pass_through_text(
    _: *mut IGlobalSession,
    _: SlangPassThrough,
    _: *const c_char,
) {
}

unsafe extern "system-unwind" fn get_pass_through_blob(
    _: *mut IGlobalSession,
    _: SlangPassThrough,
    _: *mut *mut ISlangBlob,
) {
}

unsafe extern "system-unwind" fn set_default_downstream_compiler(
    _: *mut IGlobalSession,
    _: SlangSourceLanguage,
    _: SlangPassThrough,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn get_default_downstream_compiler(
    _: *mut IGlobalSession,
    _: SlangSourceLanguage,
) -> SlangPassThrough {
    SlangPassThrough(0)
}

unsafe extern "system-unwind" fn set_language_prelude(
    _: *mut IGlobalSession,
    _: SlangSourceLanguage,
    _: *const c_char,
) {
}

unsafe extern "system-unwind" fn get_language_prelude(
    _: *mut IGlobalSession,
    _: SlangSourceLanguage,
    _: *mut *mut ISlangBlob,
) {
}

unsafe extern "system-unwind" fn create_compile_request(
    _: *mut IGlobalSession,
    _: *mut *mut ICompileRequest,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn add_builtins(
    _: *mut IGlobalSession,
    _: *const c_char,
    _: *const c_char,
) {
}

unsafe extern "system-unwind" fn set_shared_library_loader(
    _: *mut IGlobalSession,
    _: *mut ISlangSharedLibraryLoader,
) {
}

unsafe extern "system-unwind" fn get_shared_library_loader(
    _: *mut IGlobalSession,
) -> *mut ISlangSharedLibraryLoader {
    ptr::null_mut()
}

unsafe extern "system-unwind" fn check_pass_through_support(
    _: *mut IGlobalSession,
    _: SlangPassThrough,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn compile_core_module(
    _: *mut IGlobalSession,
    _: CompileCoreModuleFlags,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn load_core_module(
    _: *mut IGlobalSession,
    _: *const c_void,
    _: usize,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn save_core_module(
    _: *mut IGlobalSession,
    _: SlangArchiveType,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn find_capability(
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

unsafe extern "system-unwind" fn set_downstream_compiler_for_transition(
    _: *mut IGlobalSession,
    _: SlangCompileTarget,
    _: SlangCompileTarget,
    _: SlangPassThrough,
) {
}

unsafe extern "system-unwind" fn get_downstream_compiler_for_transition(
    _: *mut IGlobalSession,
    _: SlangCompileTarget,
    _: SlangCompileTarget,
) -> SlangPassThrough {
    SlangPassThrough(0)
}

unsafe extern "system-unwind" fn get_compiler_elapsed_time(
    _: *mut IGlobalSession,
    _: *mut f64,
    _: *mut f64,
) {
}

unsafe extern "system-unwind" fn set_spirv_core_grammar(
    _: *mut IGlobalSession,
    _: *const c_char,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn parse_command_line_arguments(
    _: *mut IGlobalSession,
    _: c_int,
    _: *const *const c_char,
    _: *mut SessionDesc,
    _: *mut *mut ISlangUnknown,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn get_session_desc_digest(
    _: *mut IGlobalSession,
    _: *mut SessionDesc,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn compile_builtin_module(
    _: *mut IGlobalSession,
    _: BuiltinModuleName,
    _: CompileCoreModuleFlags,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn load_builtin_module(
    _: *mut IGlobalSession,
    _: BuiltinModuleName,
    _: *const c_void,
    _: usize,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn save_builtin_module(
    _: *mut IGlobalSession,
    _: BuiltinModuleName,
    _: SlangArchiveType,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn get_downstream_compiler_path(
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

/// What the fake objects of a session do on the calls that a test scripts.
#[derive(Debug, Default)]
pub(crate) struct Script {
    /// What the next module loads return.
    pub(crate) load: Cell<Load>,
    /// What `link` and `linkWithOptions` return.
    pub(crate) link: Cell<Link>,
    /// What `specialize` returns.
    pub(crate) specialize: Cell<Specialize>,
    /// What `createCompositeComponentType` returns.
    pub(crate) compose: Cell<Compose>,
    /// What the calls that generate code or metadata return.
    pub(crate) code: Cell<Code>,
    /// The count that every count of the session and its objects returns
    /// instead of its own when it is set, such as a negative count.
    pub(crate) count: Cell<Option<i16>>,
}

/// What a fake module load returns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Load {
    /// The module, borrowed from the session, and no diagnostics blob.
    #[default]
    Module,
    /// The module and a diagnostics blob, as a load with warnings returns.
    ModuleWithDiagnostics,
    /// Null and a diagnostics blob, as a failed compile returns.
    NullWithDiagnostics,
    /// Null and no diagnostics blob, as a load after a failed search returns.
    Null,
}

/// What a fake `link` returns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Link {
    /// The input object with one more reference.
    #[default]
    Itself,
    /// A new object that holds the input.
    NewObject,
    /// `SLANG_FAIL` with diagnostics.
    Fail,
}

/// What a fake `specialize` returns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Specialize {
    /// A new object that holds the input.
    #[default]
    NewObject,
    /// `SLANG_FAIL` with diagnostics and a new object in the output, as
    /// Slang's `specialize` can do.
    FailWithOutput,
    /// `SLANG_FAIL` with diagnostics and a null output.
    FailWithoutOutput,
}

/// What a fake `createCompositeComponentType` returns.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Compose {
    /// A composite, or the input itself with one more reference when there
    /// is one input, as Slang does.
    #[default]
    Composite,
    /// `SLANG_FAIL` with diagnostics.
    Fail,
}

/// What the fake calls that generate code or metadata return.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Code {
    /// Success with no diagnostics blob.
    #[default]
    Plain,
    /// Success with a diagnostics blob, as Slang returns warnings.
    WithDiagnostics,
    /// `SLANG_E_NOT_AVAILABLE` with diagnostics, as a failed generation of
    /// the code of an entry point returns from `getEntryPointMetadata`.
    NotAvailable,
    /// `SLANG_FAIL` with diagnostics and a code blob written all the same.
    FailWithOutput,
}

/// The text of every diagnostics blob of the fake.
pub(crate) const FAKE_DIAGNOSTICS: &str = "warning[E1]: fake diagnostics";

/// The bytes of every code blob of the fake.
pub(crate) const FAKE_CODE: &[u8] = &[0x03, 0x02, 0x23, 0x07, 1, 0, 0, 0];

/// The state of a fake session: the global session that it holds a counted
/// reference to, as Slang's session does, the modules that it holds, each
/// with one counted reference, and the script of its objects.
struct SessionState {
    global: NonNull<IGlobalSession>,
    modules: RefCell<Vec<(CString, NonNull<IModule>)>>,
    script: Script,
}

impl Drop for SessionState {
    /// Releases the modules, then the global session, and then writes into
    /// the global session, in the order of Slang's session, whose syntax tree
    /// builder increments a counter of the global session after the session
    /// released its reference to it. When that was the last reference, the
    /// write is a use after free, which Miri reports.
    fn drop(&mut self) {
        for (_, module) in self.modules.take() {
            // SAFETY: the session added the reference that this releases when
            // it created the module.
            unsafe { IModule::release(module.as_ptr()) };
        }
        // SAFETY: `FakeSession::create` added the reference that this releases,
        // and the global session is live because of it.
        unsafe { IGlobalSession::release(self.global.as_ptr()) };
        // SAFETY: the global session is live only while another reference to
        // it exists, which the crate's wrappers guarantee by releasing a
        // session before its global session (Section 11.2 of
        // `docs/design/slang-compiler.md`). A test that breaks that order makes
        // this a use after free on purpose, so that Miri reports it.
        let global = unsafe { GlobalSessionObject::from_this(self.global.as_ptr()) };
        global.state.epoch.set(global.state.epoch.get() + 1);
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
            SessionState {
                global,
                modules: RefCell::new(Vec::new()),
                script: Script::default(),
            },
        )
    }

    /// Returns the script of the fake session `session`.
    ///
    /// # Safety
    ///
    /// `session` comes from [`FakeSession::create`] and stays alive while the
    /// returned reference lives. Breaking this is undefined behaviour.
    pub(crate) unsafe fn script<'a>(session: NonNull<ISession>) -> &'a Script {
        // SAFETY: the caller guarantees that `session` is a live fake session
        // for `'a`.
        let object = unsafe { SessionObject::from_this::<ISession>(session.as_ptr()) };
        &object.state.script
    }
}

/// Returns the script of a live fake session, which a fake component reads
/// in every call, as a component type of Slang reads its session. A session
/// that the crate freed too early makes this a use after free, which Miri
/// reports.
///
/// # Safety
///
/// `session` comes from [`FakeSession::create`]. Breaking this is undefined
/// behaviour.
unsafe fn session_script<'a>(session: NonNull<ISession>) -> &'a Script {
    // SAFETY: the caller guarantees that `session` is a fake session, and the
    // crate keeps it alive while one of its components is used.
    unsafe { FakeSession::script(session) }
}

/// Creates a diagnostics blob for an out-pointer, when it is not null.
///
/// # Safety
///
/// `out` is null or a live out-pointer. Breaking this is undefined behaviour.
unsafe fn write_diagnostics(log: &Log, out: *mut *mut ISlangBlob) {
    if !out.is_null() {
        let blob = FakeBlob::create(log, "diagnostics", FAKE_DIAGNOSTICS.as_bytes());
        // SAFETY: the caller guarantees that `out` is a live out-pointer.
        unsafe { out.write(blob.as_ptr()) };
    }
}

unsafe extern "system-unwind" fn session_global_session(
    this: *mut ISession,
) -> *mut IGlobalSession {
    // SAFETY: `SESSION_VTABLE` belongs to a live `SessionObject` only.
    let object = unsafe { SessionObject::from_this(this) };
    object.state.global.as_ptr()
}

/// Loads a module as Slang's loaders do: the session creates the module of
/// a new name, holds one reference to it, and returns it without a reference
/// for the caller, which is a borrowed return.
///
/// # Safety
///
/// `this` is a live fake session, `name` a live C string, and `diagnostics`
/// null or a live out-pointer. Breaking this is undefined behaviour.
unsafe fn load(
    this: *mut ISession,
    name: *const c_char,
    diagnostics: *mut *mut ISlangBlob,
) -> *mut IModule {
    // SAFETY: `SESSION_VTABLE` belongs to a live `SessionObject` only.
    let object = unsafe { SessionObject::from_this(this) };
    // SAFETY: the crate passes a live C string.
    let name = CString::new(unsafe { read_text(name) }).expect("a C string has no NUL byte");
    object.log.push(Event::call(object.name, "loadModule"));
    let load = object.state.script.load.get();
    if matches!(
        load,
        Load::ModuleWithDiagnostics | Load::NullWithDiagnostics
    ) {
        // SAFETY: the crate passes a live out-pointer.
        unsafe { write_diagnostics(&object.log, diagnostics) };
    }
    if matches!(load, Load::Null | Load::NullWithDiagnostics) {
        return ptr::null_mut();
    }
    let mut modules = object.state.modules.borrow_mut();
    if let Some((_, module)) = modules.iter().find(|(known, _)| *known == name) {
        return module.as_ptr();
    }
    let session = NonNull::new(this).expect("the crate passes a non-null session");
    let module = FakeComponent::create_module(&object.log, session);
    modules.push((name, module));
    module.as_ptr()
}

unsafe extern "system-unwind" fn session_load_module(
    this: *mut ISession,
    name: *const c_char,
    diagnostics: *mut *mut ISlangBlob,
) -> *mut IModule {
    // SAFETY: the crate passes what `loadModule` requires.
    unsafe { load(this, name, diagnostics) }
}

unsafe extern "system-unwind" fn session_load_module_from_blob(
    this: *mut ISession,
    name: *const c_char,
    _: *const c_char,
    _: *mut ISlangBlob,
    diagnostics: *mut *mut ISlangBlob,
) -> *mut IModule {
    // SAFETY: the crate passes what the loader requires.
    unsafe { load(this, name, diagnostics) }
}

unsafe extern "system-unwind" fn session_create_composite(
    this: *mut ISession,
    components: *const *mut IComponentType,
    count: SlangInt,
    out: *mut *mut IComponentType,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    // SAFETY: `SESSION_VTABLE` belongs to a live `SessionObject` only.
    let object = unsafe { SessionObject::from_this(this) };
    object
        .log
        .push(Event::call(object.name, "createCompositeComponentType"));
    if object.state.script.compose.get() == Compose::Fail {
        // SAFETY: the crate passes a live out-pointer.
        unsafe { write_diagnostics(&object.log, diagnostics) };
        return SlangResult::SLANG_FAIL;
    }
    let count = usize::try_from(count).expect("a non-negative count");
    // SAFETY: the crate passes `count` live component types.
    let components = unsafe { read_array(components, count) };
    let result = if let [only] = components {
        // SAFETY: the crate passes a live component type, and the caller owns
        // the reference that this adds, as Slang returns its one input.
        unsafe { IComponentType::addRef(*only) };
        *only
    } else {
        let children = components
            .iter()
            .map(|child| NonNull::new(*child).expect("the crate passes non-null components"))
            .collect();
        let session = NonNull::new(this).expect("the crate passes a non-null session");
        FakeComponent::create_holding(&object.log, "composite", session, children).as_ptr()
    };
    // SAFETY: the crate passes a live out-pointer.
    unsafe { out.write(result) };
    SlangResult::SLANG_OK
}

unsafe extern "system-unwind" fn session_specialize_type(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *const SpecializationArg,
    _: SlangInt,
    _: *mut *mut ISlangBlob,
) -> *mut TypeReflection {
    ptr::null_mut()
}

unsafe extern "system-unwind" fn session_type_layout(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: SlangInt,
    _: LayoutRules,
    _: *mut *mut ISlangBlob,
) -> *mut TypeLayoutReflection {
    ptr::null_mut()
}

unsafe extern "system-unwind" fn session_container_type(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: ContainerType,
    _: *mut *mut ISlangBlob,
) -> *mut TypeReflection {
    ptr::null_mut()
}

unsafe extern "system-unwind" fn session_dynamic_type(_: *mut ISession) -> *mut TypeReflection {
    ptr::null_mut()
}

unsafe extern "system-unwind" fn session_mangled_name(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn session_witness_mangled_name(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut TypeReflection,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn session_witness_sequential_id(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut TypeReflection,
    _: *mut u32,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn session_create_compile_request(
    _: *mut ISession,
    _: *mut *mut SlangCompileRequest,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn session_create_type_conformance(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut TypeReflection,
    _: *mut *mut ITypeConformance,
    _: SlangInt,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn session_loaded_module_count(this: *mut ISession) -> SlangInt {
    // SAFETY: `SESSION_VTABLE` belongs to a live `SessionObject` only.
    let object = unsafe { SessionObject::from_this(this) };
    object.state.script.count.get().map_or_else(
        || SlangInt::try_from(object.state.modules.borrow().len()).expect("the count fits"),
        SlangInt::from,
    )
}

unsafe extern "system-unwind" fn session_loaded_module(
    this: *mut ISession,
    index: SlangInt,
) -> *mut IModule {
    // SAFETY: `SESSION_VTABLE` belongs to a live `SessionObject` only.
    let object = unsafe { SessionObject::from_this(this) };
    object.log.push(Event::call(object.name, "getLoadedModule"));
    usize::try_from(index)
        .ok()
        .and_then(|index| {
            object
                .state
                .modules
                .borrow()
                .get(index)
                .map(|(_, module)| module.as_ptr())
        })
        .unwrap_or(ptr::null_mut())
}

unsafe extern "system-unwind" fn session_is_binary_module_up_to_date(
    _: *mut ISession,
    _: *const c_char,
    _: *mut ISlangBlob,
) -> bool {
    false
}

unsafe extern "system-unwind" fn session_load_module_from_string(
    this: *mut ISession,
    name: *const c_char,
    _: *const c_char,
    source: *const c_char,
    diagnostics: *mut *mut ISlangBlob,
) -> *mut IModule {
    // SAFETY: the crate passes a live C string as the source, which this reads
    // so that Miri checks it.
    let _source = unsafe { read_text(source) };
    // SAFETY: the crate passes what the loader requires.
    unsafe { load(this, name, diagnostics) }
}

unsafe extern "system-unwind" fn session_rtti_bytes(
    _: *mut ISession,
    _: *mut TypeReflection,
    _: *mut TypeReflection,
    _: *mut u32,
    _: u32,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn session_module_info(
    _: *mut ISession,
    _: *mut ISlangBlob,
    _: *mut SlangInt,
    _: *mut *const c_char,
    _: *mut *const c_char,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn session_decl_source_location(
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

/// The state of a fake component type: the session that it points to
/// without a counted reference, as Slang's component types do, and the
/// component types that it holds, each with one counted reference, as a
/// composite holds its children.
struct ComponentState {
    session: NonNull<ISession>,
    held: Vec<NonNull<IComponentType>>,
    entry_points: RefCell<Vec<NonNull<IEntryPoint>>>,
}

impl Drop for ComponentState {
    fn drop(&mut self) {
        for child in self.held.drain(..) {
            // SAFETY: the component added or took over the reference that this
            // releases when it was created.
            unsafe { IComponentType::release(child.as_ptr()) };
        }
        for entry_point in self.entry_points.take() {
            // SAFETY: the module added the reference that this releases when
            // it created the entry point.
            unsafe { IEntryPoint::release(entry_point.as_ptr()) };
        }
    }
}

/// The fake component types: modules, entry points, and composites,
/// specialized, and linked component types.
pub(crate) struct FakeComponent;

/// The name of every fake module in the log.
pub(crate) const FAKE_MODULE: &str = "module";

/// The name of the one entry point that each fake module defines.
pub(crate) const FAKE_ENTRY_POINT: &str = "main";

static MODULE_INTERFACES: [SlangUUID; 3] =
    [ISlangUnknown::UUID, IComponentType::UUID, IModule::UUID];
static ENTRY_POINT_INTERFACES: [SlangUUID; 3] =
    [ISlangUnknown::UUID, IComponentType::UUID, IEntryPoint::UUID];
static COMPONENT_INTERFACES: [SlangUUID; 2] = [ISlangUnknown::UUID, IComponentType::UUID];

type ModuleObject = FakeObject<IModuleVtbl, ComponentState>;
type EntryPointObject = FakeObject<IEntryPointVtbl, ComponentState>;
type ComponentObject = FakeObject<IComponentTypeVtbl, ComponentState>;

impl FakeComponent {
    fn state(session: NonNull<ISession>, held: Vec<NonNull<IComponentType>>) -> ComponentState {
        ComponentState {
            session,
            held,
            entry_points: RefCell::new(Vec::new()),
        }
    }

    /// Creates a fake module of `session` with one reference, which the
    /// caller owns.
    fn create_module(log: &Log, session: NonNull<ISession>) -> NonNull<IModule> {
        ModuleObject::create(
            &MODULE_VTABLE,
            log,
            FAKE_MODULE,
            &MODULE_INTERFACES,
            Self::state(session, Vec::new()),
        )
    }

    /// Creates a fake component type named `name` of `session` with one
    /// reference, which the caller owns, that takes over one reference of
    /// each of `held`.
    fn create_holding(
        log: &Log,
        name: &'static str,
        session: NonNull<ISession>,
        held: Vec<NonNull<IComponentType>>,
    ) -> NonNull<IComponentType> {
        for child in &held {
            // SAFETY: the caller passes live component types, and the new
            // component owns the reference that this adds.
            unsafe { IComponentType::addRef(child.as_ptr()) };
        }
        ComponentObject::create(
            &COMPONENT_VTABLE,
            log,
            name,
            &COMPONENT_INTERFACES,
            Self::state(session, held),
        )
    }
}

/// Returns the state of any fake component type, which has the same layout
/// whatever its vtable type, because the vtable field is a reference.
///
/// # Safety
///
/// `this` is a live fake component type of this module. Breaking this is
/// undefined behaviour.
unsafe fn component<'a, I>(this: *mut I) -> &'a ComponentObject {
    // SAFETY: every fake component type is a `FakeObject` whose state is a
    // `ComponentState`, and its first field is a reference to a vtable, so
    // its layout does not depend on the vtable type.
    unsafe { ComponentObject::from_this(this) }
}

unsafe extern "system-unwind" fn component_session(this: *mut IComponentType) -> *mut ISession {
    // SAFETY: the vtable belongs to a live fake component type only.
    let object = unsafe { component(this) };
    object.state.session.as_ptr()
}

unsafe extern "system-unwind" fn component_layout(
    this: *mut IComponentType,
    _: SlangInt,
    _: *mut *mut ISlangBlob,
) -> *mut ProgramLayout {
    // SAFETY: the vtable belongs to a live fake component type only.
    let object = unsafe { component(this) };
    object.log.push(Event::call(object.name, "getLayout"));
    ptr::null_mut()
}

unsafe extern "system-unwind" fn component_specialization_param_count(
    this: *mut IComponentType,
) -> SlangInt {
    // SAFETY: the vtable belongs to a live fake component type only.
    let object = unsafe { component(this) };
    // SAFETY: the crate keeps the session of a component alive.
    let script = unsafe { session_script(object.state.session) };
    script.count.get().map_or(1, SlangInt::from)
}

/// Writes code or metadata as the script of the session says.
///
/// # Safety
///
/// `this` is a live fake component type, and `out` and `diagnostics` are
/// live out-pointers or null. Breaking this is undefined behaviour.
unsafe fn generate(
    this: *mut IComponentType,
    method: &'static str,
    out: impl FnOnce(&Log),
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    // SAFETY: the caller guarantees that `this` is a live fake component.
    let object = unsafe { component(this) };
    object.log.push(Event::call(object.name, method));
    // SAFETY: the crate keeps the session of a component alive, as Slang's
    // component types require (Section 3.4 of `docs/design/slang-compiler.md`).
    let script = unsafe { session_script(object.state.session) };
    let code = script.code.get();
    if code != Code::Plain {
        // SAFETY: the caller guarantees that `diagnostics` is live or null.
        unsafe { write_diagnostics(&object.log, diagnostics) };
    }
    match code {
        Code::Plain | Code::WithDiagnostics => {
            out(&object.log);
            SlangResult::SLANG_OK
        }
        Code::NotAvailable => SlangResult::SLANG_E_NOT_AVAILABLE,
        Code::FailWithOutput => {
            out(&object.log);
            SlangResult::SLANG_FAIL
        }
    }
}

/// Writes a new code blob to `out`.
///
/// # Safety
///
/// `out` is a live out-pointer. Breaking this is undefined behaviour.
unsafe fn write_code(log: &Log, out: *mut *mut ISlangBlob) {
    let blob = FakeBlob::create(log, "code", FAKE_CODE);
    // SAFETY: the caller guarantees that `out` is a live out-pointer.
    unsafe { out.write(blob.as_ptr()) };
}

unsafe extern "system-unwind" fn component_entry_point_code(
    this: *mut IComponentType,
    _: SlangInt,
    _: SlangInt,
    out: *mut *mut ISlangBlob,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    let write = |log: &Log| {
        // SAFETY: the crate passes a live out-pointer.
        unsafe { write_code(log, out) };
    };
    // SAFETY: the crate passes a live component and live out-pointers.
    unsafe { generate(this, "getEntryPointCode", write, diagnostics) }
}

unsafe extern "system-unwind" fn component_target_code(
    this: *mut IComponentType,
    _: SlangInt,
    out: *mut *mut ISlangBlob,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    let write = |log: &Log| {
        // SAFETY: the crate passes a live out-pointer.
        unsafe { write_code(log, out) };
    };
    // SAFETY: the crate passes a live component and live out-pointers.
    unsafe { generate(this, "getTargetCode", write, diagnostics) }
}

unsafe extern "system-unwind" fn component_entry_point_metadata(
    this: *mut IComponentType,
    _: SlangInt,
    _: SlangInt,
    out: *mut *mut IMetadata,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    let write = |log: &Log| {
        // SAFETY: the crate passes a live out-pointer.
        unsafe { out.write(FakeMetadata::create(log).as_ptr()) };
    };
    // SAFETY: the crate passes a live component and live out-pointers.
    unsafe { generate(this, "getEntryPointMetadata", write, diagnostics) }
}

unsafe extern "system-unwind" fn component_target_metadata(
    this: *mut IComponentType,
    _: SlangInt,
    out: *mut *mut IMetadata,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    let write = |log: &Log| {
        // SAFETY: the crate passes a live out-pointer.
        unsafe { out.write(FakeMetadata::create(log).as_ptr()) };
    };
    // SAFETY: the crate passes a live component and live out-pointers.
    unsafe { generate(this, "getTargetMetadata", write, diagnostics) }
}

unsafe extern "system-unwind" fn component_result_as_file_system(
    _: *mut IComponentType,
    _: SlangInt,
    _: SlangInt,
    _: *mut *mut ISlangMutableFileSystem,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn component_entry_point_hash(
    _: *mut IComponentType,
    _: SlangInt,
    _: SlangInt,
    _: *mut *mut ISlangBlob,
) {
}

unsafe extern "system-unwind" fn component_specialize(
    this: *mut IComponentType,
    _: *const SpecializationArg,
    _: SlangInt,
    out: *mut *mut IComponentType,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    // SAFETY: the vtable belongs to a live fake component type only.
    let object = unsafe { component(this) };
    object.log.push(Event::call(object.name, "specialize"));
    // SAFETY: the crate keeps the session of a component alive.
    let script = unsafe { session_script(object.state.session) };
    let mode = script.specialize.get();
    if mode != Specialize::FailWithoutOutput {
        let input = NonNull::new(this).expect("the crate passes a non-null component");
        let specialized = FakeComponent::create_holding(
            &object.log,
            "specialized",
            object.state.session,
            vec![input],
        );
        // SAFETY: the crate passes a live out-pointer.
        unsafe { out.write(specialized.as_ptr()) };
    }
    if mode == Specialize::NewObject {
        SlangResult::SLANG_OK
    } else {
        // SAFETY: the crate passes a live out-pointer.
        unsafe { write_diagnostics(&object.log, diagnostics) };
        SlangResult::SLANG_FAIL
    }
}

/// Links as the script of the session says.
///
/// # Safety
///
/// `this` is a live fake component type, and `out` and `diagnostics` are
/// live out-pointers. Breaking this is undefined behaviour.
unsafe fn fake_link(
    this: *mut IComponentType,
    out: *mut *mut IComponentType,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    // SAFETY: the caller guarantees that `this` is a live fake component.
    let object = unsafe { component(this) };
    // SAFETY: the crate keeps the session of a component alive.
    let script = unsafe { session_script(object.state.session) };
    let input = NonNull::new(this).expect("the crate passes a non-null component");
    let linked = match script.link.get() {
        Link::Itself => {
            // SAFETY: `this` is live, and the caller owns the reference that
            // this adds, as Slang's `link` returns its input.
            unsafe { IComponentType::addRef(this) };
            input
        }
        Link::NewObject => {
            FakeComponent::create_holding(&object.log, "linked", object.state.session, vec![input])
        }
        Link::Fail => {
            // SAFETY: the caller guarantees that `diagnostics` is live.
            unsafe { write_diagnostics(&object.log, diagnostics) };
            return SlangResult::SLANG_FAIL;
        }
    };
    // SAFETY: the caller guarantees that `out` is a live out-pointer.
    unsafe { out.write(linked.as_ptr()) };
    SlangResult::SLANG_OK
}

unsafe extern "system-unwind" fn component_link(
    this: *mut IComponentType,
    out: *mut *mut IComponentType,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    // SAFETY: the vtable belongs to a live fake component type only.
    let object = unsafe { component(this) };
    object.log.push(Event::call(object.name, "link"));
    // SAFETY: the crate passes a live component and live out-pointers.
    unsafe { fake_link(this, out, diagnostics) }
}

unsafe extern "system-unwind" fn component_link_with_options(
    this: *mut IComponentType,
    out: *mut *mut IComponentType,
    count: u32,
    entries: *const CompilerOptionEntry,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    // SAFETY: the vtable belongs to a live fake component type only.
    let object = unsafe { component(this) };
    // SAFETY: `linkWithOptions` requires `count` live entries, which this
    // reads so that Miri checks them.
    let options = unsafe { read_options(entries, count) };
    object.log.push(Event::LinkWithOptions {
        object: object.name,
        options,
    });
    // SAFETY: the crate passes a live component and live out-pointers.
    unsafe { fake_link(this, out, diagnostics) }
}

unsafe extern "system-unwind" fn component_host_callable(
    _: *mut IComponentType,
    _: c_int,
    _: c_int,
    _: *mut *mut ISlangSharedLibrary,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn component_rename(
    _: *mut IComponentType,
    _: *const c_char,
    _: *mut *mut IComponentType,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

const fn component_vtable<V: 'static>() -> IComponentTypeVtbl {
    IComponentTypeVtbl {
        base: unknown_vtable::<V, ComponentState>(),
        getSession: component_session,
        getLayout: component_layout,
        getSpecializationParamCount: component_specialization_param_count,
        getEntryPointCode: component_entry_point_code,
        getResultAsFileSystem: component_result_as_file_system,
        getEntryPointHash: component_entry_point_hash,
        specialize: component_specialize,
        link: component_link,
        getEntryPointHostCallable: component_host_callable,
        renameEntryPoint: component_rename,
        linkWithOptions: component_link_with_options,
        getTargetCode: component_target_code,
        getTargetMetadata: component_target_metadata,
        getEntryPointMetadata: component_entry_point_metadata,
    }
}

static COMPONENT_VTABLE: IComponentTypeVtbl = component_vtable::<IComponentTypeVtbl>();

static ENTRY_POINT_VTABLE: IEntryPointVtbl = IEntryPointVtbl {
    base: component_vtable::<IEntryPointVtbl>(),
    getFunctionReflection: entry_point_function,
};

unsafe extern "system-unwind" fn entry_point_function(
    _: *mut IEntryPoint,
) -> *mut FunctionReflection {
    ptr::null_mut()
}

/// Returns the entry point of a fake module, creating it on the first call,
/// with a new reference for the caller.
///
/// # Safety
///
/// `this` is a live fake module. Breaking this is undefined behaviour.
unsafe fn module_entry_point(this: *mut IModule) -> NonNull<IEntryPoint> {
    // SAFETY: the caller guarantees that `this` is a live fake module.
    let object = unsafe { component(this) };
    let mut entry_points = object.state.entry_points.borrow_mut();
    if entry_points.is_empty() {
        entry_points.push(EntryPointObject::create(
            &ENTRY_POINT_VTABLE,
            &object.log,
            "entry_point",
            &ENTRY_POINT_INTERFACES,
            FakeComponent::state(object.state.session, Vec::new()),
        ));
    }
    let entry_point = *entry_points.first().expect("an entry point exists");
    // SAFETY: the entry point is live, because the module holds it, and the
    // caller owns the reference that this adds, as Slang's module returns its
    // entry points with a new reference.
    unsafe { IEntryPoint::addRef(entry_point.as_ptr()) };
    entry_point
}

unsafe extern "system-unwind" fn module_find_entry_point(
    this: *mut IModule,
    name: *const c_char,
    out: *mut *mut IEntryPoint,
) -> SlangResult {
    // SAFETY: the vtable belongs to a live fake module only.
    let object = unsafe { component(this) };
    object
        .log
        .push(Event::call(object.name, "findEntryPointByName"));
    // SAFETY: the crate passes a live C string.
    if unsafe { read_text(name) } != FAKE_ENTRY_POINT {
        return SlangResult::SLANG_FAIL;
    }
    // SAFETY: `this` is a live fake module.
    let entry_point = unsafe { module_entry_point(this) };
    // SAFETY: the crate passes a live out-pointer.
    unsafe { out.write(entry_point.as_ptr()) };
    SlangResult::SLANG_OK
}

unsafe extern "system-unwind" fn module_entry_point_count(this: *mut IModule) -> SlangInt32 {
    // SAFETY: the vtable belongs to a live fake module only.
    let object = unsafe { component(this) };
    // SAFETY: the crate keeps the session of a module alive.
    let script = unsafe { session_script(object.state.session) };
    script.count.get().map_or(1, SlangInt32::from)
}

unsafe extern "system-unwind" fn module_defined_entry_point(
    this: *mut IModule,
    index: SlangInt32,
    out: *mut *mut IEntryPoint,
) -> SlangResult {
    if index != 0 {
        return SlangResult::SLANG_E_INVALID_ARG;
    }
    // SAFETY: `this` is a live fake module.
    let entry_point = unsafe { module_entry_point(this) };
    // SAFETY: the crate passes a live out-pointer.
    unsafe { out.write(entry_point.as_ptr()) };
    SlangResult::SLANG_OK
}

unsafe extern "system-unwind" fn module_serialize(
    this: *mut IModule,
    out: *mut *mut ISlangBlob,
) -> SlangResult {
    // SAFETY: the vtable belongs to a live fake module only.
    let object = unsafe { component(this) };
    // SAFETY: the crate passes a live out-pointer.
    unsafe { write_code(&object.log, out) };
    SlangResult::SLANG_OK
}

unsafe extern "system-unwind" fn module_write_to_file(
    _: *mut IModule,
    _: *const c_char,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

unsafe extern "system-unwind" fn module_name(_: *mut IModule) -> *const c_char {
    c"fake_module".as_ptr()
}

unsafe extern "system-unwind" fn module_no_text(_: *mut IModule) -> *const c_char {
    ptr::null()
}

/// Finds the entry point as `findEntryPointByName` does, and accepts it only
/// for the compute stage. Any other name or stage fails with diagnostics, as
/// a failed validation does in Slang.
unsafe extern "system-unwind" fn module_find_and_check(
    this: *mut IModule,
    name: *const c_char,
    stage: SlangStage,
    out: *mut *mut IEntryPoint,
    diagnostics: *mut *mut ISlangBlob,
) -> SlangResult {
    // SAFETY: the vtable belongs to a live fake module only.
    let object = unsafe { component(this) };
    object
        .log
        .push(Event::call(object.name, "findAndCheckEntryPoint"));
    // SAFETY: the crate passes a live C string.
    if unsafe { read_text(name) } != FAKE_ENTRY_POINT || stage != SlangStage::SLANG_STAGE_COMPUTE {
        // SAFETY: the crate passes a live out-pointer.
        unsafe { write_diagnostics(&object.log, diagnostics) };
        return SlangResult::SLANG_FAIL;
    }
    // SAFETY: `this` is a live fake module.
    let entry_point = unsafe { module_entry_point(this) };
    // SAFETY: the crate passes a live out-pointer.
    unsafe { out.write(entry_point.as_ptr()) };
    SlangResult::SLANG_OK
}

unsafe extern "system-unwind" fn module_dependency_count(this: *mut IModule) -> SlangInt32 {
    // SAFETY: the vtable belongs to a live fake module only.
    let object = unsafe { component(this) };
    // SAFETY: the crate keeps the session of a module alive.
    let script = unsafe { session_script(object.state.session) };
    script.count.get().map_or(2, SlangInt32::from)
}

unsafe extern "system-unwind" fn module_dependency_path(
    _: *mut IModule,
    index: SlangInt32,
) -> *const c_char {
    assert!(
        index == 0 || index == 1,
        "the crate checks the index against the count"
    );
    if index == 0 {
        c"fake.slang".as_ptr()
    } else {
        ptr::null()
    }
}

unsafe extern "system-unwind" fn module_reflection(_: *mut IModule) -> *mut DeclReflection {
    ptr::null_mut()
}

unsafe extern "system-unwind" fn module_disassemble(
    _: *mut IModule,
    _: *mut *mut ISlangBlob,
) -> SlangResult {
    SlangResult::SLANG_E_NOT_IMPLEMENTED
}

static MODULE_VTABLE: IModuleVtbl = IModuleVtbl {
    base: component_vtable::<IModuleVtbl>(),
    findEntryPointByName: module_find_entry_point,
    getDefinedEntryPointCount: module_entry_point_count,
    getDefinedEntryPoint: module_defined_entry_point,
    serialize: module_serialize,
    writeToFile: module_write_to_file,
    getName: module_name,
    getFilePath: module_no_text,
    getUniqueIdentity: module_no_text,
    findAndCheckEntryPoint: module_find_and_check,
    getDependencyFileCount: module_dependency_count,
    getDependencyFilePath: module_dependency_path,
    getModuleReflection: module_reflection,
    disassemble: module_disassemble,
};

/// A fake `IMetadata`, which reports the binding 0 of the binding space 0 as
/// used and every other binding as unused.
pub(crate) struct FakeMetadata;

type MetadataObject = FakeObject<IMetadataVtbl, ()>;

static METADATA_INTERFACES: [SlangUUID; 2] = [ISlangUnknown::UUID, IMetadata::UUID];

static METADATA_VTABLE: IMetadataVtbl = IMetadataVtbl {
    base: ISlangCastableVtbl {
        base: unknown_vtable::<IMetadataVtbl, ()>(),
        castAs: metadata_cast_as,
    },
    isParameterLocationUsed: metadata_location_used,
    getDebugBuildIdentifier: metadata_build_identifier,
};

impl FakeMetadata {
    fn create(log: &Log) -> NonNull<IMetadata> {
        MetadataObject::create(&METADATA_VTABLE, log, "metadata", &METADATA_INTERFACES, ())
    }
}

unsafe extern "system-unwind" fn metadata_cast_as(
    _: *mut ISlangCastable,
    _: *const SlangUUID,
) -> *mut c_void {
    ptr::null_mut()
}

unsafe extern "system-unwind" fn metadata_location_used(
    this: *mut IMetadata,
    _: SlangParameterCategory,
    space: SlangUInt,
    index: SlangUInt,
    out: *mut bool,
) -> SlangResult {
    // SAFETY: `METADATA_VTABLE` belongs to a live `MetadataObject` only.
    let object = unsafe { MetadataObject::from_this(this) };
    object
        .log
        .push(Event::call(object.name, "isParameterLocationUsed"));
    // SAFETY: the crate passes a live out-pointer.
    unsafe { out.write(space == 0 && index == 0) };
    SlangResult::SLANG_OK
}

unsafe extern "system-unwind" fn metadata_build_identifier(_: *mut IMetadata) -> *const c_char {
    c"".as_ptr()
}

/// Adopts a new fake global session for a test, which uses it on this thread
/// only and adopts at most one global session at a time.
pub(crate) fn global_session(log: &Log) -> GlobalSession {
    // SAFETY: the new fake global session carries one reference, it reports
    // the pinned build tag, the test uses it on this thread only, and each
    // test adopts at most one global session at a time.
    unsafe {
        GlobalSession::from_raw(FakeGlobalSession::create(
            log,
            "global",
            SlangResult::SLANG_OK,
        ))
    }
}

/// Creates a fake session of `global` with `target_count` SPIR-V targets.
pub(crate) fn session(global: &GlobalSession, target_count: usize) -> Session {
    let mut builder = Session::builder(global);
    for _ in 0..target_count {
        builder = builder.target(Target::new(TargetFormat::Spirv));
    }
    builder.build().expect("the fake creates the session")
}

/// Returns the releases and frees of the objects named `names`, in order.
pub(crate) fn releases(log: &Log, names: &[&str]) -> Vec<Event> {
    log.events()
        .into_iter()
        .filter(|event| match event {
            Event::Release { object, .. } | Event::Freed { object } => names.contains(object),
            _ => false,
        })
        .collect()
}

/// Returns the calls other than the counting calls and `queryInterface`.
pub(crate) fn calls(log: &Log) -> Vec<Event> {
    log.events()
        .into_iter()
        .filter(|event| matches!(event, Event::Call { .. } | Event::LinkWithOptions { .. }))
        .collect()
}
