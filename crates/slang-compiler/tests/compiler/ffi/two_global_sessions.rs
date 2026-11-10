//! The test of assumption CA-14 of `docs/design/slang-compiler.md`, through
//! the raw bindings, because the crate refuses a second global session on a
//! thread (SLC-29).

use std::ffi::{CStr, c_void};
use std::ptr;

use slang_compiler_sys::{
    IComponentType, IEntryPoint, IGlobalSession, IModule, ISession, ISlangBlob, Interface,
    SessionDesc, SlangCompileTarget, SlangGlobalSessionDesc, SlangParameterCategory,
    SlangReflection, SlangReflectionType, SlangReflectionTypeLayout, SlangReflectionVariableLayout,
    SlangResult, TargetDesc, slang_createGlobalSession2, spReflection_FindTypeByName,
    spReflection_GetParameterByIndex, spReflection_GetParameterCount,
    spReflectionType_GetColumnCount, spReflectionType_GetElementCount,
    spReflectionType_GetElementType, spReflectionType_GetFieldByIndex,
    spReflectionType_GetFieldCount, spReflectionType_GetFullName, spReflectionType_GetKind,
    spReflectionType_GetName, spReflectionType_GetResourceResultType, spReflectionType_GetRowCount,
    spReflectionTypeLayout_GetElementTypeLayout, spReflectionTypeLayout_GetFieldByIndex,
    spReflectionTypeLayout_GetFieldCount, spReflectionTypeLayout_GetSize,
    spReflectionTypeLayout_GetType, spReflectionTypeLayout_getKind, spReflectionVariable_GetName,
    spReflectionVariable_GetType, spReflectionVariableLayout_GetTypeLayout,
    spReflectionVariableLayout_GetVariable,
};

const SOURCE: &CStr = cr#"
typealias Color = float4;

struct Pair<T>
{
    T first;
    T second;
}

struct Light
{
    float3 position;
    Color color;
    float intensities[4];
    float4x4 transform;
}

struct Material
{
    Pair<float3> tint;
    Light lights[2];
}

ParameterBlock<Material> material;
ConstantBuffer<Pair<Color>> pairs;
StructuredBuffer<Light> lights;
RWStructuredBuffer<float4> output;
Texture2D<float4> albedo;
SamplerState linearSampler;

[shader("compute")]
[numthreads(8, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    Light light = lights[id.x];
    float4 lit = mul(light.transform, float4(light.position, 1.0)) * light.intensities[1];
    float4 sampled = albedo.SampleLevel(linearSampler, float2(0.5, 0.5), 0);
    output[id.x] = float4(material.tint.first, 1.0) + pairs.second + light.color + lit + sampled
        + material.lights[1].color;
}
"#;

const TYPE_NAMES: [&CStr; 6] = [
    c"Light",
    c"Material",
    c"Pair<float3>",
    c"Pair<Color>",
    c"Color",
    c"StructuredBuffer<Light>",
];

const MAX_DEPTH: usize = 16;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
unsafe extern "C" {
    fn mallopt(param: std::ffi::c_int, value: std::ffi::c_int) -> std::ffi::c_int;
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
const M_PERTURB: std::ffi::c_int = -6;

/// Makes glibc overwrite every block that `free` releases while the value
/// lives, so that a read of freed memory reads garbage instead of the old
/// value, and restores glibc's default, which overwrites nothing, when it
/// drops.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub(super) struct FreedBlocksPoisoned;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
impl FreedBlocksPoisoned {
    pub(super) fn set() -> Self {
        // SAFETY: `mallopt` with `M_PERTURB` only sets the byte that glibc
        // writes into freed blocks and returns whether it accepted the
        // setting.
        let accepted = unsafe { mallopt(M_PERTURB, 0x5A) };
        assert_eq!(accepted, 1);
        Self
    }
}

#[cfg(all(target_os = "linux", target_env = "gnu"))]
impl Drop for FreedBlocksPoisoned {
    fn drop(&mut self) {
        // SAFETY: `mallopt` with `M_PERTURB` and zero only restores glibc's
        // default of overwriting nothing.
        unsafe { mallopt(M_PERTURB, 0) };
    }
}

/// Allocates and frees many blocks of many sizes, filled with a pattern, and
/// keeps every second one, so that freed memory is reused and overwritten.
pub(super) fn churn_the_heap() -> Vec<Vec<u8>> {
    let mut kept = Vec::new();
    for round in 0..4_u8 {
        for size in (16..=65_536).step_by(48) {
            let block = vec![0xA5 ^ round; size];
            if size % 96 == 16 {
                kept.push(block);
            }
        }
    }
    kept
}

fn check(result: SlangResult, what: &str) {
    assert!(result.0 >= 0, "{what} failed with {result:?}");
}

/// Takes the text of a blob and releases it.
///
/// # Safety
///
/// `blob` is null or a live blob whose one reference the caller owns.
unsafe fn take_text(blob: *mut ISlangBlob) -> String {
    if blob.is_null() {
        return String::new();
    }
    // SAFETY: `blob` is a live blob, as the caller guarantees.
    let data = unsafe { ISlangBlob::getBufferPointer(blob) };
    // SAFETY: `blob` is a live blob, as the caller guarantees.
    let size = unsafe { ISlangBlob::getBufferSize(blob) };
    let text = if data.is_null() {
        String::new()
    } else {
        // SAFETY: Slang reports `size` bytes at `data`, which the blob keeps
        // until the release below.
        let bytes = unsafe { std::slice::from_raw_parts(data.cast::<u8>(), size) };
        String::from_utf8_lossy(bytes).into_owned()
    };
    // SAFETY: the caller owns the one reference of `blob`, and `text` is a copy.
    unsafe { ISlangBlob::release(blob) };
    text
}

/// One reference to a Slang object, which the value releases when it drops.
///
/// A local drops before every local declared earlier, and a field before
/// every field declared after it, on the normal path and on unwinding alike,
/// so an object declared after the object it was created from is released
/// first.
struct Ref<T> {
    raw: *mut T,
    release: unsafe fn(*mut T) -> u32,
}

impl<T> Ref<T> {
    /// Takes over the reference that `raw` carries, and fails the test when
    /// `raw` is null.
    ///
    /// # Safety
    ///
    /// `raw` is null or points to a live Slang object whose one reference the
    /// caller transfers, `release` releases one reference of such an object,
    /// and every object that Slang creates from it and that holds no counted
    /// reference to it, such as a program of a session, is held by a `Ref`
    /// that drops before the returned value.
    unsafe fn adopt(raw: *mut T, release: unsafe fn(*mut T) -> u32, what: &str) -> Self {
        assert!(!raw.is_null(), "{what} returned null");
        Self { raw, release }
    }

    fn as_ptr(&self) -> *mut T {
        self.raw
    }
}

impl<T> Drop for Ref<T> {
    fn drop(&mut self) {
        // SAFETY: `adopt` received a live object with one reference that
        // `release` releases, `drop` runs once, and the caller of `adopt`
        // guaranteed that every object created from this one that holds no
        // counted reference to it was released before.
        unsafe { (self.release)(self.raw) };
    }
}

/// A global session and a session with one SPIR-V target, created through the
/// raw bindings.
///
/// The session is declared before the global session, so it drops first.
/// Every program that `link` returns is held in a local that the test
/// declares after the `RawSession`, so it drops before it.
struct RawSession {
    session: Ref<ISession>,
    _global: Ref<IGlobalSession>,
}

#[expect(
    ffi_unwind_calls,
    reason = "the test calls the C functions of Slang directly, and an exception that escaped one would unwind into the test, which is defined and fails it"
)]
impl RawSession {
    fn create() -> Self {
        let desc = SlangGlobalSessionDesc::default();
        let mut global: *mut IGlobalSession = ptr::null_mut();
        // SAFETY: `desc` is a live descriptor whose size field holds its own
        // size, and `global` is a live out-pointer.
        let result = unsafe { slang_createGlobalSession2(&raw const desc, &raw mut global) };
        check(result, "slang_createGlobalSession2");
        // SAFETY: `slang_createGlobalSession2` succeeded and returned a new
        // reference, and its one session is held in the field `session`, which
        // drops first.
        let global = unsafe { Ref::adopt(global, IGlobalSession::release, "the global session") };

        let target = TargetDesc {
            format: SlangCompileTarget::SLANG_SPIRV,
            ..TargetDesc::default()
        };
        let session_desc = SessionDesc {
            targets: &raw const target,
            targetCount: 1,
            ..SessionDesc::default()
        };
        let mut session: *mut ISession = ptr::null_mut();
        // SAFETY: `global` is live, `session_desc` and the target it points to
        // are live locals, and `session` is a live out-pointer.
        let result = unsafe {
            IGlobalSession::createSession(
                global.as_ptr(),
                &raw const session_desc,
                &raw mut session,
            )
        };
        check(result, "createSession");
        // SAFETY: `createSession` succeeded and returned a new reference, and
        // every program created from the session is held in a `Ref` that the
        // test declares after this `RawSession`.
        let session = unsafe { Ref::adopt(session, ISession::release, "createSession") };
        Self {
            session,
            _global: global,
        }
    }

    /// Loads `SOURCE` as the module `name`, links it with its entry point
    /// `main`, and returns the linked program.
    fn link(&self, name: &CStr, path: &CStr) -> Ref<IComponentType> {
        let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
        // SAFETY: the session is live, the three strings are live and
        // null-terminated, and `diagnostics` is a live out-pointer.
        let module = unsafe {
            ISession::loadModuleFromSourceString(
                self.session.as_ptr(),
                name.as_ptr(),
                path.as_ptr(),
                SOURCE.as_ptr(),
                &raw mut diagnostics,
            )
        };
        // SAFETY: `diagnostics` is null or a blob whose reference this owns.
        let text = unsafe { take_text(diagnostics) };
        assert!(!module.is_null(), "the module does not compile:\n{text}");

        let mut entry_point: *mut IEntryPoint = ptr::null_mut();
        // SAFETY: `module` is live, held by the session, the name is a
        // null-terminated literal, and `entry_point` is a live out-pointer.
        let result = unsafe {
            IModule::findEntryPointByName(module, c"main".as_ptr(), &raw mut entry_point)
        };
        check(result, "findEntryPointByName");
        // SAFETY: `findEntryPointByName` succeeded and returned a new
        // reference, and the component type queried from it is the same
        // object with a counted reference of its own.
        let entry_point =
            unsafe { Ref::adopt(entry_point, IEntryPoint::release, "findEntryPointByName") };

        let module_component = query_component_type(|uuid, out| {
            // SAFETY: `module` is live, and `uuid` and `out` are live.
            unsafe { IModule::queryInterface(module, uuid, out) }
        });
        let entry_component = query_component_type(|uuid, out| {
            // SAFETY: `entry_point` is live, and `uuid` and `out` are live.
            unsafe { IEntryPoint::queryInterface(entry_point.as_ptr(), uuid, out) }
        });
        let components = [module_component.as_ptr(), entry_component.as_ptr()];
        let mut composite: *mut IComponentType = ptr::null_mut();
        let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
        // SAFETY: the session is live, `components` holds two live component
        // types, and both out-pointers are live.
        let result = unsafe {
            ISession::createCompositeComponentType(
                self.session.as_ptr(),
                components.as_ptr(),
                2,
                &raw mut composite,
                &raw mut diagnostics,
            )
        };
        // SAFETY: `diagnostics` is null or a blob whose reference this owns.
        let text = unsafe { take_text(diagnostics) };
        check(result, &format!("createCompositeComponentType: {text}"));
        // SAFETY: `createCompositeComponentType` succeeded and returned a new
        // reference, and the program linked from it holds a counted reference
        // to it.
        let composite = unsafe {
            Ref::adopt(
                composite,
                IComponentType::release,
                "createCompositeComponentType",
            )
        };

        let mut linked: *mut IComponentType = ptr::null_mut();
        let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
        // SAFETY: `composite` is live, and both out-pointers are live.
        let result = unsafe {
            IComponentType::link(composite.as_ptr(), &raw mut linked, &raw mut diagnostics)
        };
        // SAFETY: `diagnostics` is null or a blob whose reference this owns.
        let text = unsafe { take_text(diagnostics) };
        check(result, &format!("link: {text}"));
        // SAFETY: `link` succeeded and returned a new reference, and the
        // caller holds the program in a local declared after this
        // `RawSession`. Nothing that the test creates from the program is
        // counted.
        unsafe { Ref::adopt(linked, IComponentType::release, "link") }
    }
}

/// Queries the `IComponentType` interface of an object through `query`.
fn query_component_type(
    query: impl FnOnce(*const slang_compiler_sys::SlangUUID, *mut *mut c_void) -> SlangResult,
) -> Ref<IComponentType> {
    let uuid = IComponentType::UUID;
    let mut out: *mut c_void = ptr::null_mut();
    check(query(&raw const uuid, &raw mut out), "queryInterface");
    // SAFETY: `queryInterface` succeeded and returned a new reference of the
    // same object, and the composite that the test creates from it holds a
    // counted reference to it.
    unsafe {
        Ref::adopt(
            out.cast::<IComponentType>(),
            IComponentType::release,
            "queryInterface",
        )
    }
}

/// Returns the program layout of a linked program for target 0, and fails
/// the test when the program has none.
///
/// # Safety
///
/// `program` is a live linked program.
unsafe fn layout(program: &Ref<IComponentType>) -> *mut SlangReflection {
    let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
    // SAFETY: `program` is live, as the caller guarantees, target 0 exists,
    // and `diagnostics` is a live out-pointer.
    let layout = unsafe { IComponentType::getLayout(program.as_ptr(), 0, &raw mut diagnostics) };
    // SAFETY: `diagnostics` is null or a blob whose reference this owns.
    let text = unsafe { take_text(diagnostics) };
    assert!(!layout.is_null(), "the program has no layout: {text}");
    layout.cast::<SlangReflection>()
}

/// A description of everything that reflection reports about a program, and
/// the address of every type handle that it returned.
#[derive(Debug, Default, PartialEq, Eq)]
struct Reflected {
    text: String,
    types: Vec<usize>,
}

#[expect(
    ffi_unwind_calls,
    reason = "the test calls the C functions of Slang directly, and an exception that escaped one would unwind into the test, which is defined and fails it"
)]
impl Reflected {
    /// Describes every parameter of `layout` and every type that `TYPE_NAMES`
    /// names.
    ///
    /// # Safety
    ///
    /// `layout` is the live program layout of a live linked program.
    unsafe fn of(layout: *mut SlangReflection) -> Self {
        let mut reflected = Self::default();
        // SAFETY: `layout` is live, as the caller guarantees.
        let count = unsafe { spReflection_GetParameterCount(layout) };
        for index in 0..count {
            // SAFETY: `layout` is live, and `index` is below its count.
            let parameter = unsafe { spReflection_GetParameterByIndex(layout, index) };
            // SAFETY: `parameter` is null or a variable layout of `layout`.
            unsafe { reflected.variable_layout(parameter, 0) };
        }
        for name in TYPE_NAMES {
            // SAFETY: `layout` is live, and `name` is a null-terminated literal.
            let ty = unsafe { spReflection_FindTypeByName(layout, name.as_ptr()) };
            reflected.line(0, &format!("lookup {name:?}"));
            // SAFETY: `ty` is null or a type of the program.
            unsafe { reflected.ty(ty, 1) };
        }
        reflected
    }

    fn line(&mut self, depth: usize, text: &str) {
        let line = format!("{:indent$}{text}\n", "", indent = depth * 2);
        self.text.push_str(&line);
    }

    /// # Safety
    ///
    /// `variable` is null or a live variable layout of a live program.
    unsafe fn variable_layout(
        &mut self,
        variable: *mut SlangReflectionVariableLayout,
        depth: usize,
    ) {
        if variable.is_null() || depth > MAX_DEPTH {
            self.line(depth, "variable layout: none");
            return;
        }
        // SAFETY: `variable` is a live variable layout.
        let var = unsafe { spReflectionVariableLayout_GetVariable(variable) };
        let name = if var.is_null() {
            String::from("<none>")
        } else {
            // SAFETY: `var` is a live variable of the program.
            let name = unsafe { spReflectionVariable_GetName(var) };
            text_of(name)
        };
        self.line(depth, &format!("variable {name}"));
        if !var.is_null() {
            // SAFETY: `var` is a live variable of the program.
            let ty = unsafe { spReflectionVariable_GetType(var) };
            // SAFETY: `ty` is null or a type of the program.
            unsafe { self.ty(ty, depth + 1) };
        }
        // SAFETY: `variable` is a live variable layout.
        let type_layout = unsafe { spReflectionVariableLayout_GetTypeLayout(variable) };
        // SAFETY: `type_layout` is null or a type layout of the program.
        unsafe { self.type_layout(type_layout, depth + 1) };
    }

    /// # Safety
    ///
    /// `layout` is null or a live type layout of a live program.
    unsafe fn type_layout(&mut self, layout: *mut SlangReflectionTypeLayout, depth: usize) {
        if layout.is_null() || depth > MAX_DEPTH {
            self.line(depth, "type layout: none");
            return;
        }
        // SAFETY: `layout` is a live type layout.
        let kind = unsafe { spReflectionTypeLayout_getKind(layout) };
        // SAFETY: `layout` is a live type layout, and the category is listed.
        let size = unsafe {
            spReflectionTypeLayout_GetSize(
                layout,
                SlangParameterCategory::SLANG_PARAMETER_CATEGORY_UNIFORM,
            )
        };
        self.line(
            depth,
            &format!("type layout kind {} uniform size {size}", kind.0),
        );
        // SAFETY: `layout` is a live type layout.
        let ty = unsafe { spReflectionTypeLayout_GetType(layout) };
        // SAFETY: `ty` is null or a type of the program.
        unsafe { self.ty(ty, depth + 1) };
        // SAFETY: `layout` is a live type layout.
        let fields = unsafe { spReflectionTypeLayout_GetFieldCount(layout) };
        for index in 0..fields {
            // SAFETY: `layout` is live, and `index` is below its field count.
            let field = unsafe { spReflectionTypeLayout_GetFieldByIndex(layout, index) };
            // SAFETY: `field` is null or a variable layout of the program.
            unsafe { self.variable_layout(field, depth + 1) };
        }
        // SAFETY: `layout` is a live type layout.
        let element = unsafe { spReflectionTypeLayout_GetElementTypeLayout(layout) };
        if !element.is_null() {
            // SAFETY: `element` is a type layout of the program.
            unsafe { self.type_layout(element, depth + 1) };
        }
    }

    /// # Safety
    ///
    /// `ty` is null or a live type of a live program.
    unsafe fn ty(&mut self, ty: *mut SlangReflectionType, depth: usize) {
        self.types.push(ty.addr());
        if ty.is_null() || depth > MAX_DEPTH {
            self.line(depth, "type: none");
            return;
        }
        // SAFETY: `ty` is a live type.
        let kind = unsafe { spReflectionType_GetKind(ty) };
        // SAFETY: `ty` is a live type.
        let name = text_of(unsafe { spReflectionType_GetName(ty) });
        let mut blob: *mut ISlangBlob = ptr::null_mut();
        // SAFETY: `ty` is a live type, and `blob` is a live out-pointer.
        let result = unsafe { spReflectionType_GetFullName(ty, &raw mut blob) };
        // SAFETY: `blob` is null or a blob whose reference this owns.
        let full_name = unsafe { take_text(blob) };
        // SAFETY: `ty` is a live type.
        let rows = unsafe { spReflectionType_GetRowCount(ty) };
        // SAFETY: `ty` is a live type.
        let columns = unsafe { spReflectionType_GetColumnCount(ty) };
        // SAFETY: `ty` is a live type.
        let elements = unsafe { spReflectionType_GetElementCount(ty) };
        self.line(
            depth,
            &format!(
                "type kind {} name {name} full name {full_name} ({}) {rows}x{columns} elements {elements}",
                kind.0, result.0
            ),
        );
        // SAFETY: `ty` is a live type.
        let fields = unsafe { spReflectionType_GetFieldCount(ty) };
        for index in 0..fields {
            // SAFETY: `ty` is live, and `index` is below its field count.
            let field = unsafe { spReflectionType_GetFieldByIndex(ty, index) };
            if field.is_null() {
                continue;
            }
            // SAFETY: `field` is a live variable of the program.
            let name = text_of(unsafe { spReflectionVariable_GetName(field) });
            self.line(depth + 1, &format!("field {name}"));
            // SAFETY: `field` is a live variable of the program.
            let field_type = unsafe { spReflectionVariable_GetType(field) };
            // SAFETY: `field_type` is null or a type of the program.
            unsafe { self.ty(field_type, depth + 2) };
        }
        // SAFETY: `ty` is a live type.
        let element = unsafe { spReflectionType_GetElementType(ty) };
        if !element.is_null() {
            // SAFETY: `element` is a type of the program.
            unsafe { self.ty(element, depth + 1) };
        }
        // SAFETY: `ty` is a live type.
        let result_type = unsafe { spReflectionType_GetResourceResultType(ty) };
        if !result_type.is_null() {
            // SAFETY: `result_type` is a type of the program.
            unsafe { self.ty(result_type, depth + 1) };
        }
    }
}

/// Copies a string that Slang returned, or `<none>` for null.
fn text_of(text: *const std::ffi::c_char) -> String {
    if text.is_null() {
        return String::from("<none>");
    }
    // SAFETY: Slang returned `text` as a null-terminated string of a live
    // object, and this copies it at once.
    unsafe { CStr::from_ptr(text) }
        .to_string_lossy()
        .into_owned()
}

/// Runs the sequence of CA-14: reflection on a program of a first global
/// session before a second global session exists on the same thread, while it
/// is alive, after it compiled a program, after it was destroyed, and after
/// freed memory was reused, and a new program of the first, and asserts that
/// every description and every type handle stays the same.
///
/// The second global session makes its syntax tree builder the current one of
/// the thread. Reflection on the first then resolves types while that builder
/// is current, and any node that resolution allocates there and caches in a
/// node of the first would dangle once the second is destroyed.
fn reflect_across_a_second_global_session() {
    let first = RawSession::create();
    let program = first.link(c"first", c"first.slang");
    // SAFETY: `program` is a live linked program of `first`.
    let layout = unsafe { layout(&program) };
    // SAFETY: `layout` belongs to `program`, which lives until the end.
    let before = unsafe { Reflected::of(layout) };
    assert!(before.text.contains("variable material"), "{}", before.text);
    assert!(before.text.contains("Pair<float3>"), "{}", before.text);

    let second = RawSession::create();
    // SAFETY: as above, and the second global session is now the newest one
    // of the thread.
    let while_alive = unsafe { Reflected::of(layout) };
    let other_program = second.link(c"second", c"second.slang");
    // SAFETY: as above, after the second global session compiled a program.
    let after_compiling = unsafe { Reflected::of(layout) };
    drop(other_program);
    drop(second);

    let mut kept = churn_the_heap();
    // SAFETY: as above, after the second global session was destroyed.
    let after_drop = unsafe { Reflected::of(layout) };
    kept.extend(churn_the_heap());
    // SAFETY: as above.
    let after_churn = unsafe { Reflected::of(layout) };
    let relinked = first.link(c"first_again", c"first_again.slang");
    // SAFETY: `relinked` is a live linked program of `first`.
    let relinked_layout = unsafe { self::layout(&relinked) };
    // SAFETY: `relinked_layout` belongs to `relinked`, which is live.
    let after_relink = unsafe { Reflected::of(relinked_layout) };
    drop(kept);

    assert_eq!(
        while_alive, before,
        "reflection changed while a second global session was alive"
    );
    assert_eq!(
        after_compiling, before,
        "reflection changed after the second global session compiled"
    );
    assert_eq!(
        after_drop, before,
        "reflection changed after the second global session was destroyed"
    );
    assert_eq!(
        after_churn, before,
        "reflection changed after freed memory was reused"
    );
    assert_eq!(
        after_relink.text, before.text,
        "a new program of the first global session differs"
    );
}

/// Checks CA-14 on Linux with glibc, with every block that `free` releases
/// overwritten.
///
/// A reflection call that reads a node which the second global session
/// allocated with `malloc` and freed reads the overwritten bytes, so a
/// description changes or the process crashes. The test cannot detect a read
/// of memory that Slang's own allocators reuse without calling `free`, or a
/// read of freed memory whose result never reaches a description. The
/// setting applies to the whole process while the test runs and is reset when
/// it ends, and nextest runs each test in a process of its own.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[test]
fn reflection_on_a_first_global_session_is_unchanged_by_a_second_one_with_freed_blocks_poisoned() {
    let _poisoned = FreedBlocksPoisoned::set();
    reflect_across_a_second_global_session();
}

/// Checks CA-14 on every platform other than Linux with glibc, where the
/// allocator does not overwrite freed blocks.
///
/// The test checks only that the sequence does not crash and that reflection
/// stays the same. A read of freed memory that still holds its old value, or
/// that no reuse by the heap churn overwrote, passes unnoticed, so this test
/// is weaker than the one on Linux with glibc.
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
#[test]
fn reflection_on_a_first_global_session_is_unchanged_by_a_second_one_without_poisoning() {
    reflect_across_a_second_global_session();
}
