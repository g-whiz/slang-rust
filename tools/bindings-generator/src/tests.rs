#[cfg(target_os = "linux")]
mod ffi {
    use std::path::PathBuf;
    use std::sync::{Mutex, PoisonError};

    use crate::error::{Error, ErrorKind};
    use crate::model::Prim;
    use crate::rules::{Action, DeclRule, MacroAction, MacroRule, RuleSet, UnnamedEnumRule};
    use crate::{Generated, generate_with, pin};

    static LIBCLANG: Mutex<()> = Mutex::new(());

    const PRELUDE: &str = r"#include <stddef.h>
#include <stdint.h>
typedef int32_t SlangResult;
typedef int64_t SlangInt;
struct SlangUUID
{
    uint32_t data1;
    uint16_t data2;
    uint16_t data3;
    uint8_t data4[8];
};
#define TEST_COM_INTERFACE(a, b, c, d0, d1, d2, d3, d4, d5, d6, d7) \
public: \
    static constexpr SlangUUID getTypeGuid() { return {a, b, c, d0, d1, d2, d3, d4, d5, d6, d7}; }
#define TEST_UUID TEST_COM_INTERFACE(0x11223344, 0x5566, 0x7788, {0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00})
struct IRoot
{
    TEST_UUID
    virtual SlangResult queryInterface(SlangUUID const& uuid, void** outObject) = 0;
    virtual uint32_t release() = 0;
};
";

    const INCLUDE_ALL: &[DeclRule] = &[DeclRule {
        header: None,
        pattern: "*",
        action: Action::Include,
        reason: "a test declaration",
    }];

    const NO_MACROS: &[MacroRule] = &[MacroRule {
        pattern: "*",
        action: MacroAction::Exclude,
        reason: "a test macro",
    }];

    const fn rules(declarations: &'static [DeclRule], macros: &'static [MacroRule]) -> RuleSet {
        RuleSet {
            main_header: "slang.h",
            declarations,
            macros,
            unnamed_enums: &[],
            result_type: "SlangResult",
            uuid_type: "SlangUUID",
            pointer_sized: &[("SlangInt", Prim::Isize)],
        }
    }

    const DEFAULT_RULES: RuleSet = rules(INCLUDE_ALL, NO_MACROS);

    struct Headers {
        dir: PathBuf,
    }

    impl Headers {
        fn new(test: &str, header: &str, tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!(
                "bindings-generator-test-{}-{test}",
                std::process::id()
            ));
            std::fs::create_dir_all(&dir).expect("create the header directory");
            std::fs::write(dir.join("slang.h"), format!("{PRELUDE}{header}"))
                .expect("write slang.h");
            std::fs::write(
                dir.join("slang-tag-version.h"),
                format!(
                    "#define SLANG_TAG_VERSION \"{tag}\"\n#define SLANG_VERSION_NUMERIC \"0\"\n"
                ),
            )
            .expect("write slang-tag-version.h");
            Self { dir }
        }
    }

    impl Drop for Headers {
        fn drop(&mut self) {
            if let Err(error) = std::fs::remove_dir_all(&self.dir) {
                eprintln!("cannot remove {}: {error}", self.dir.display());
            }
        }
    }

    fn run(test: &str, header: &str, rules: &RuleSet, tag: &str) -> crate::Result<String> {
        run_in(test, header, rules, tag)
            .0
            .map(|generated| generated.bindings)
    }

    fn run_in(
        test: &str,
        header: &str,
        rules: &RuleSet,
        tag: &str,
    ) -> (crate::Result<Generated>, PathBuf) {
        let _guard = LIBCLANG.lock().unwrap_or_else(PoisonError::into_inner);
        let headers = Headers::new(test, header, tag);
        let result = generate_with(rules, &headers.dir, pin::SLANG_BUILD_TAG);
        (result, headers.dir.clone())
    }

    fn generate_with_extra_header(
        test: &str,
        header: &str,
        extra: (&str, &str),
        rules: &RuleSet,
    ) -> String {
        let _guard = LIBCLANG.lock().unwrap_or_else(PoisonError::into_inner);
        let headers = Headers::new(test, header, pin::SLANG_BUILD_TAG);
        std::fs::write(headers.dir.join(extra.0), extra.1).expect("write the extra header");
        generate_with(rules, &headers.dir, pin::SLANG_BUILD_TAG)
            .unwrap_or_else(|error| panic!("generation failed: {error}"))
            .bindings
    }

    fn generate(test: &str, header: &str) -> String {
        generate_rules(test, header, &DEFAULT_RULES)
    }

    fn generate_rules(test: &str, header: &str, rules: &RuleSet) -> String {
        run(test, header, rules, pin::SLANG_BUILD_TAG)
            .unwrap_or_else(|error| panic!("generation failed: {error}"))
    }

    fn fail(test: &str, header: &str) -> Error {
        fail_rules(test, header, &DEFAULT_RULES)
    }

    fn fail_rules(test: &str, header: &str, rules: &RuleSet) -> Error {
        run(test, header, rules, pin::SLANG_BUILD_TAG).expect_err("generation fails")
    }

    fn line_of(header: &str, needle: &str) -> u32 {
        let position = format!("{PRELUDE}{header}")
            .lines()
            .position(|line| line.contains(needle))
            .expect("the needle is in the header");
        u32::try_from(position + 1).expect("the line number fits u32")
    }

    fn assert_error(error: &Error, kind: ErrorKind, line: u32, needles: &[&str]) {
        assert_eq!(error.kind(), kind, "{error}");
        let location = error.location().expect("the error has a location");
        assert_eq!(location.file(), "slang.h", "{error}");
        assert_eq!(location.line(), line, "{error}");
        for needle in needles {
            assert!(
                error.message().contains(needle),
                "{needle:?} not in: {error}"
            );
        }
    }

    fn assert_contains(text: &str, needle: &str) {
        assert!(text.contains(needle), "{needle}\n--- not in ---\n{text}");
    }

    #[test]
    fn generate_orders_vtable_slots_after_single_and_multi_level_bases() {
        let text = generate(
            "vtable_order",
            r"
struct IMiddle : public IRoot
{
    TEST_UUID
    virtual int32_t first() = 0;
};
struct ILeaf : public IMiddle
{
    TEST_UUID
    virtual int32_t second() = 0;
    virtual void third(int32_t value) = 0;
};
",
        );
        assert_contains(
            &text,
            "pub struct IRootVtbl {
    pub queryInterface: unsafe extern \"system-unwind\" fn(
        this: *mut IRoot,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult,
    pub release: unsafe extern \"system-unwind\" fn(this: *mut IRoot) -> u32,
}",
        );
        assert_contains(
            &text,
            "pub struct IMiddleVtbl {
    pub base: IRootVtbl,
    pub first: unsafe extern \"system-unwind\" fn(this: *mut IMiddle) -> i32,
}",
        );
        assert_contains(
            &text,
            "pub struct ILeafVtbl {
    pub base: IMiddleVtbl,
    pub second: unsafe extern \"system-unwind\" fn(this: *mut ILeaf) -> i32,
    pub third: unsafe extern \"system-unwind\" fn(this: *mut ILeaf, value: i32),
}",
        );
        assert_contains(
            &text,
            "    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }",
        );
        assert_contains(
            &text,
            "    pub unsafe fn first(this: *mut Self) -> i32 {
        unsafe { ((*(*this).vtable).base.first)(this.cast()) }
    }",
        );
        assert_contains(
            &text,
            "    pub unsafe fn third(this: *mut Self, value: i32) {
        unsafe {
            ((*(*this).vtable).third)(this, value);
        }
    }",
        );
        let leaf = text.find("impl ILeaf {").expect("thunks of ILeaf");
        let order: Vec<usize> = [
            "fn queryInterface",
            "fn release",
            "fn first",
            "fn second",
            "fn third",
        ]
        .iter()
        .map(|name| leaf + text[leaf..].find(name).expect("thunk"))
        .collect();
        assert!(order.is_sorted(), "thunks of ILeaf are out of slot order");
    }

    #[test]
    fn generate_reads_the_uuid_from_the_body_of_get_type_guid() {
        let text = generate("uuid", "");
        assert_contains(
            &text,
            "unsafe impl Interface for IRoot {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x1122_3344,
        data2: 0x5566,
        data3: 0x7788,
        data4: [0x99, 0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF, 0x00],
    };
}",
        );
        assert_contains(
            &text,
            "pub unsafe trait Interface {\n    const UUID: SlangUUID;\n}",
        );
    }

    #[test]
    fn generate_maps_types_by_typedef_name() {
        let text = generate(
            "type_mapping",
            r"
typedef uint64_t Size64;
typedef SlangInt Index;
struct Mapping
{
    int8_t a;
    uint16_t b;
    int64_t c;
    size_t d;
    SlangInt e;
    Index f;
    int g;
    unsigned long long h;
    char i;
    bool j;
    double k;
    const char* l;
    void* m;
    uint8_t n[3];
};
",
        );
        assert_contains(&text, "pub struct SlangResult(pub i32);");
        assert_contains(&text, "pub type Size64 = u64;");
        assert_contains(&text, "pub type SlangInt = isize;");
        assert_contains(&text, "pub type Index = SlangInt;");
        assert_contains(
            &text,
            "pub struct Mapping {
    pub a: i8,
    pub b: u16,
    pub c: i64,
    pub d: usize,
    pub e: SlangInt,
    pub f: Index,
    pub g: ::core::ffi::c_int,
    pub h: ::core::ffi::c_ulonglong,
    pub i: ::core::ffi::c_char,
    pub j: bool,
    pub k: f64,
    pub l: *const ::core::ffi::c_char,
    pub m: *mut ::core::ffi::c_void,
    pub n: [u8; 3],
}",
        );
    }

    #[test]
    fn generate_lowers_references_to_pointers() {
        let text = generate(
            "references",
            r"
struct Pair
{
    int32_t first;
};
struct IRefs : public IRoot
{
    TEST_UUID
    virtual void read(const Pair& input, int32_t& output, const char*& text) = 0;
};
",
        );
        assert_contains(
            &text,
            "    pub read: unsafe extern \"system-unwind\" fn(
        this: *mut IRefs,
        input: *const Pair,
        output: *mut i32,
        text: *mut *const ::core::ffi::c_char,
    ),",
        );
    }

    #[test]
    fn generate_maps_callbacks_to_optional_c_function_pointers() {
        let text = generate(
            "callbacks",
            r#"
typedef void (*Callback)(const char* message, void* userData);
struct Holder
{
    Callback callback;
    int32_t (*inline_pointer)(int32_t);
};
extern "C" void setCallback(Callback callback, void* userData);
"#,
        );
        assert_contains(
            &text,
            "pub type Callback =\n    Option<unsafe extern \"C\" fn(*const ::core::ffi::c_char, *mut ::core::ffi::c_void)>;",
        );
        assert_contains(&text, "pub callback: Callback,");
        assert_contains(
            &text,
            "pub inline_pointer: Option<unsafe extern \"C\" fn(i32) -> i32>,",
        );
        assert_contains(
            &text,
            "unsafe extern \"C-unwind\" {\n    pub fn setCallback(callback: Callback, userData: *mut ::core::ffi::c_void);\n}",
        );
    }

    #[test]
    fn generate_emits_enumerations_as_integer_newtypes() {
        const RULES: RuleSet = RuleSet {
            unnamed_enums: &[UnnamedEnumRule {
                header: "slang.h",
                enumerator: "LOOSE_B",
                typedef: "LooseFlags",
                reason: "the test flags",
            }],
            ..DEFAULT_RULES
        };
        let text = generate_rules(
            "enumerations",
            r"
enum Fixed : uint8_t
{
    FIXED_A = 1,
    FIXED_B = 200,
};
enum class Scoped
{
    Minus = -1,
    Plus = 1,
};
enum Unfixed
{
    UNFIXED_A = 1,
    UNFIXED_B = 0x7FFFFFFF,
};
enum Negative
{
    NEGATIVE_A = -5,
};
typedef uint16_t LooseFlags;
enum
{
    LOOSE_A = 4,
    LOOSE_B = 0x8000,
};
",
            &RULES,
        );
        assert_contains(
            &text,
            "#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Fixed(pub u8);

impl Fixed {
    pub const FIXED_A: Self = Self(1);
    pub const FIXED_B: Self = Self(200);
}",
        );
        assert_contains(&text, "pub struct Scoped(pub ::core::ffi::c_int);");
        assert_contains(&text, "    pub const Minus: Self = Self(-1);");
        assert_contains(&text, "pub struct Unfixed(pub u32);");
        assert_contains(
            &text,
            "    pub const UNFIXED_B: Self = Self(2_147_483_647);",
        );
        assert_contains(&text, "pub struct Negative(pub i32);");
        assert_contains(&text, "pub type LooseFlags = u16;");
        assert_contains(&text, "pub const LOOSE_A: LooseFlags = 4;");
        assert_contains(&text, "pub const LOOSE_B: LooseFlags = 32_768;");
        assert!(!text.contains("pub enum"), "a Rust enum was generated");
    }

    #[test]
    fn generate_evaluates_result_codes_and_allowed_macros_only() {
        const MACROS: &[MacroRule] = &[
            MacroRule {
                pattern: "SLANG_OK",
                action: MacroAction::ResultCode,
                reason: "a result code",
            },
            MacroRule {
                pattern: "SLANG_E_*",
                action: MacroAction::ResultCode,
                reason: "a result code",
            },
            MacroRule {
                pattern: "TEST_SIZE",
                action: MacroAction::Constant(Prim::Usize),
                reason: "a size sentinel",
            },
            MacroRule {
                pattern: "TEST_FACILITY",
                action: MacroAction::Constant(Prim::I32),
                reason: "a facility",
            },
            MacroRule {
                pattern: "*",
                action: MacroAction::Exclude,
                reason: "a platform macro",
            },
        ];
        const RULES: RuleSet = rules(INCLUDE_ALL, MACROS);
        let text = generate_rules(
            "macros",
            r"
#define TEST_MAKE_ERROR(facility, code) ((int32_t)(((uint32_t)1 << 31) | ((facility) << 16) | (code)))
#define TEST_FACILITY 7
#define SLANG_OK 0
#define SLANG_E_FAIL TEST_MAKE_ERROR(0, 0x4005)
#define SLANG_E_INVALID_ARG TEST_MAKE_ERROR(TEST_FACILITY, 0x57)
#define TEST_SIZE (~size_t(0))
#define TEST_PLATFORM 1
",
            &RULES,
        );
        assert_contains(
            &text,
            "#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangResult(pub i32);

impl SlangResult {
    pub const SLANG_OK: Self = Self(0);
    pub const SLANG_E_FAIL: Self = Self(-2_147_467_259);
    pub const SLANG_E_INVALID_ARG: Self = Self(-2_147_024_809);
}",
        );
        assert_contains(&text, "pub const TEST_FACILITY: i32 = 7;");
        assert_contains(
            &text,
            "pub const TEST_SIZE: usize = 18_446_744_073_709_551_615;",
        );
        assert!(
            !text.contains("TEST_PLATFORM"),
            "an excluded macro was generated"
        );
    }

    #[test]
    fn generate_reproduces_default_member_initializers() {
        let text = generate(
            "defaults",
            r"
enum class Mode : int32_t
{
    Off = 0,
    On = 2,
};
struct Inner
{
    int32_t level = 3;
};
struct Desc
{
    size_t structureSize = sizeof(Desc);
    const char* name = nullptr;
    void* data = nullptr;
    SlangInt line = -1;
    Mode mode = Mode::On;
    bool enabled = true;
    uint32_t reserved[4] = {};
    uint32_t plain;
    Inner inner;
};
struct Plain
{
    int32_t value;
};
",
        );
        assert_contains(
            &text,
            "impl Default for Desc {
    fn default() -> Self {
        Self {
            structureSize: 64,
            name: ::core::ptr::null(),
            data: ::core::ptr::null_mut(),
            line: -1,
            mode: Mode(2),
            enabled: true,
            reserved: [0; 4],
            plain: 0,
            inner: Inner::default(),
        }
    }
}",
        );
        assert_contains(&text, "impl Default for Inner {");
        assert!(
            !text.contains("impl Default for Plain"),
            "Plain has no initializers"
        );
    }

    #[test]
    fn generate_declares_opaque_handles_and_c_functions() {
        let text = generate(
            "opaque",
            r#"
typedef struct Handle Handle;
extern "C" Handle* openHandle(const char* path);
"#,
        );
        assert_contains(
            &text,
            "#[repr(C)]
#[derive(Debug)]
pub struct Handle {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}",
        );
        assert!(
            !text.contains("pub type Handle"),
            "a same-name typedef was emitted"
        );
        assert_contains(
            &text,
            "    pub fn openHandle(path: *const ::core::ffi::c_char) -> *mut Handle;",
        );
    }

    #[test]
    fn generate_asserts_size_alignment_and_field_offsets() {
        let text = generate(
            "layout",
            r"
struct Padded
{
    uint8_t small;
    uint64_t large;
    uint16_t tail;
};
",
        );
        assert_contains(
            &text,
            "const _: () = {
    assert!(size_of::<Padded>() == 24);
    assert!(align_of::<Padded>() == 8);
    assert!(::core::mem::offset_of!(Padded, small) == 0);
    assert!(::core::mem::offset_of!(Padded, large) == 8);
    assert!(::core::mem::offset_of!(Padded, tail) == 16);
};",
        );
        assert_contains(
            &text,
            "#[cfg(not(target_pointer_width = \"64\"))]\ncompile_error!(",
        );
    }

    #[test]
    fn generate_emits_unions_and_anonymous_members() {
        let text = generate(
            "unions",
            r"
struct Argument
{
    int32_t kind;
    union
    {
        void* pointer;
        int64_t number;
    };
};
",
        );
        assert_contains(
            &text,
            "pub union Argument_anon0 {
    pub pointer: *mut ::core::ffi::c_void,
    pub number: i64,
}",
        );
        assert_contains(&text, "    pub anon0: Argument_anon0,");
        assert_contains(&text, "impl ::core::fmt::Debug for Argument_anon0 {");
        assert_contains(
            &text,
            "assert!(::core::mem::offset_of!(Argument, anon0) == 8);",
        );
    }

    #[test]
    fn generate_keeps_the_name_and_initializer_of_a_field_whose_type_has_no_name() {
        let text = generate(
            "named_field_of_unnamed_type",
            r"
struct Holder
{
    int32_t kind;
    struct
    {
        int32_t x;
    } point = {};
};
",
        );
        assert_contains(
            &text,
            "pub struct Holder {\n    pub kind: i32,\n    pub point: Holder_anon0,\n}",
        );
        assert_contains(&text, "pub struct Holder_anon0 {\n    pub x: i32,\n}");
        assert_contains(&text, "point: Holder_anon0 { x: 0 },");
        assert_contains(
            &text,
            "assert!(::core::mem::offset_of!(Holder, point) == 4);",
        );
    }

    #[test]
    fn generate_emits_excluded_types_used_through_pointers_as_opaque_types() {
        const DECLARATIONS: &[DeclRule] = &[
            DeclRule {
                header: Some("slang.h"),
                pattern: "IHidden",
                action: Action::Exclude,
                reason: "a deprecated interface",
            },
            DeclRule {
                header: None,
                pattern: "*",
                action: Action::Include,
                reason: "a test declaration",
            },
        ];
        const RULES: RuleSet = rules(DECLARATIONS, NO_MACROS);
        let text = generate_rules(
            "excluded_opaque",
            r"
struct IHidden : public IRoot
{
    TEST_UUID
    virtual void hidden() = 0;
};
struct IVisible : public IRoot
{
    TEST_UUID
    virtual SlangResult create(IHidden** outHidden) = 0;
    virtual void deprecated() = 0;
};
",
            &RULES,
        );
        assert_contains(&text, "pub struct IHidden {\n    _data: [u8; 0],");
        assert!(
            !text.contains("IHiddenVtbl"),
            "the excluded interface has a vtable"
        );
        assert_contains(&text, "outHidden: *mut *mut IHidden)");
        assert_contains(
            &text,
            "pub deprecated: unsafe extern \"system-unwind\" fn(this: *mut IVisible),",
        );
    }

    #[test]
    fn generate_emits_a_struct_whose_typedef_precedes_its_definition() {
        let text = generate(
            "typedef_before_definition",
            r"
typedef struct Later Later;
struct Later
{
    int32_t value;
};
",
        );
        assert_contains(&text, "pub struct Later {\n    pub value: i32,\n}");
        assert!(
            !text.contains("pub type Later"),
            "the typedef is emitted: {text}"
        );
    }

    #[test]
    fn generate_accepts_a_canonical_type_difference_behind_a_typedef() {
        let text = generate(
            "canonical_difference",
            r"
typedef int64_t Wide;
typedef size_t Count;
",
        );
        assert_contains(&text, "pub type Wide = i64;");
        assert_contains(&text, "pub type Count = usize;");
    }

    #[test]
    fn generate_is_deterministic() {
        let header = r"
struct IOther : public IRoot
{
    TEST_UUID
    virtual void act(int32_t value) = 0;
};
";
        let (first, first_dir) = run_in(
            "deterministic_first",
            header,
            &DEFAULT_RULES,
            pin::SLANG_BUILD_TAG,
        );
        let (second, _) = run_in(
            "deterministic_second",
            header,
            &DEFAULT_RULES,
            pin::SLANG_BUILD_TAG,
        );
        let first = first.unwrap_or_else(|error| panic!("generation failed: {error}"));
        let second = second.unwrap_or_else(|error| panic!("generation failed: {error}"));
        assert_eq!(first, second);
        let temp_dir = std::env::temp_dir();
        for path in [
            first_dir.to_string_lossy(),
            temp_dir.to_string_lossy(),
            env!("CARGO_MANIFEST_DIR").into(),
            "bindings-generator-test".into(),
        ] {
            for (file, text) in first.files() {
                assert!(
                    !text.contains(path.as_ref()),
                    "{file} contains the path {path}"
                );
            }
        }
    }

    #[test]
    fn generate_lists_every_c_function_with_its_address_in_declaration_order() {
        let header = "\nextern \"C\" int32_t second(int32_t value);\nextern \"C\" void type();\nextern \"C\" void first();\n";
        let generated = run_in("symbols", header, &DEFAULT_RULES, pin::SLANG_BUILD_TAG)
            .0
            .unwrap_or_else(|error| panic!("generation failed: {error}"));
        assert_eq!(
            generated.symbols,
            "//! The C functions of the raw Slang bindings, generated from the Slang headers by the binding generator. Run `just generate-bindings` to regenerate them instead of editing this file.\n\npub(super) const C_FUNCTIONS: &[(&str, *const ())] = &[\n    (\"second\", slang_compiler_sys::second as *const ()),\n    (\"type\", slang_compiler_sys::r#type as *const ()),\n    (\"first\", slang_compiler_sys::first as *const ()),\n];\n"
        );
        assert_contains(&generated.bindings, "pub fn r#type();");
    }

    #[test]
    fn generate_without_c_functions_writes_an_empty_list() {
        let generated = run_in("no_symbols", "", &DEFAULT_RULES, pin::SLANG_BUILD_TAG)
            .0
            .unwrap_or_else(|error| panic!("generation failed: {error}"));
        assert_contains(
            &generated.symbols,
            "pub(super) const C_FUNCTIONS: &[(&str, *const ())] = &[];\n",
        );
    }

    #[test]
    fn generate_rejects_a_difference_between_targets_without_a_rule() {
        let header = r"
struct Differs
{
    int32_t first;
#ifdef _WIN32
    int32_t second;
#endif
};
";
        let error = fail("target_difference", header);
        assert_error(
            &error,
            ErrorKind::TargetDifference,
            line_of(header, "struct Differs"),
            &[
                "declaration Differs differs between x86_64-unknown-linux-gnu and x86_64-pc-windows-msvc",
                "no rule accepts the difference",
            ],
        );
    }

    #[test]
    fn generate_fails_on_an_error_diagnostic() {
        let header = "\n#include <missing_header.h>\n";
        let error = fail("diagnostic", header);
        assert_error(
            &error,
            ErrorKind::Diagnostic,
            line_of(header, "missing_header"),
            &["x86_64-unknown-linux-gnu", "missing_header.h"],
        );
    }

    #[test]
    fn generate_fails_on_a_declaration_that_matches_no_rule() {
        const DECLARATIONS: &[DeclRule] = &[
            DeclRule {
                header: Some("slang.h"),
                pattern: "Slang*",
                action: Action::Include,
                reason: "a test declaration",
            },
            DeclRule {
                header: Some("slang.h"),
                pattern: "IRoot",
                action: Action::Include,
                reason: "a test declaration",
            },
        ];
        const RULES: RuleSet = rules(DECLARATIONS, NO_MACROS);
        let header = "\nstruct Unknown\n{\n    int32_t value;\n};\n";
        let error = fail_rules("unmatched_declaration", header, &RULES);
        assert_error(
            &error,
            ErrorKind::UnmatchedDeclaration,
            line_of(header, "struct Unknown"),
            &["match no rule", "Unknown (slang.h:"],
        );
    }

    #[test]
    fn generate_fails_on_a_macro_that_matches_no_rule() {
        const MACROS: &[MacroRule] = &[MacroRule {
            pattern: "TEST_*",
            action: MacroAction::Exclude,
            reason: "a test macro",
        }];
        const RULES: RuleSet = rules(INCLUDE_ALL, MACROS);
        let header = "\n#define NEW_MACRO 5\n";
        let error = fail_rules("unmatched_macro", header, &RULES);
        assert_error(
            &error,
            ErrorKind::UnmatchedDeclaration,
            line_of(header, "NEW_MACRO"),
            &["macro NEW_MACRO"],
        );
    }

    const TYPED_MACROS: &[MacroRule] = &[
        MacroRule {
            pattern: "TEST_VALUE",
            action: MacroAction::Constant(Prim::I32),
            reason: "a typed test constant",
        },
        MacroRule {
            pattern: "*",
            action: MacroAction::Exclude,
            reason: "a test macro",
        },
    ];

    #[test]
    fn generate_fails_on_a_macro_that_is_not_a_constant_expression() {
        const RULES: RuleSet = rules(INCLUDE_ALL, TYPED_MACROS);
        let header = "\nextern \"C\" int32_t compute();\n#define TEST_VALUE compute()\n";
        let error = fail_rules("macro_not_constant", header, &RULES);
        assert_error(
            &error,
            ErrorKind::Evaluation,
            line_of(header, "#define TEST_VALUE"),
            &[
                "cannot evaluate macro TEST_VALUE",
                "a typed test constant",
                "x86_64-unknown-linux-gnu",
            ],
        );
    }

    #[test]
    fn generate_fails_on_a_macro_value_that_does_not_fit_its_type() {
        const RULES: RuleSet = rules(INCLUDE_ALL, TYPED_MACROS);
        let header = "\n#define TEST_VALUE 0x100000000\n";
        let error = fail_rules("macro_too_large", header, &RULES);
        assert_error(
            &error,
            ErrorKind::Evaluation,
            line_of(header, "#define TEST_VALUE"),
            &[
                "macro TEST_VALUE",
                "4294967296, which does not fit its type",
            ],
        );
    }

    #[test]
    fn generate_fails_on_an_initializer_that_cannot_be_evaluated() {
        let header = "\nextern \"C\" int32_t compute();\nstruct Desc\n{\n    int32_t value = compute();\n};\n";
        let error = fail("initializer_not_constant", header);
        assert_error(
            &error,
            ErrorKind::Evaluation,
            line_of(header, "int32_t value"),
            &["cannot evaluate the default member initializer of Desc::value"],
        );
    }

    #[test]
    fn generate_refuses_an_aggregate_parameter_passed_by_value() {
        let header =
            "\nstruct Pair\n{\n    int32_t a;\n};\nextern \"C\" void takePair(Pair pair);\n";
        let error = fail("by_value_parameter", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "takePair"),
            &[
                "parameter pair of function takePair",
                "aggregate Pair by value",
            ],
        );
    }

    #[test]
    fn generate_refuses_an_aggregate_returned_by_value_from_a_method() {
        let header = r"
struct Pair
{
    int32_t a;
};
struct IReturns : public IRoot
{
    TEST_UUID
    virtual Pair makePair() = 0;
};
";
        let error = fail("by_value_return", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "makePair"),
            &[
                "the return value of method IReturns::makePair",
                "aggregate Pair by value",
            ],
        );
    }

    #[test]
    fn generate_refuses_a_variadic_function() {
        let header = "\nextern \"C\" void logf(const char* format, ...);\n";
        let error = fail("variadic", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "logf"),
            &["function logf is variadic"],
        );
    }

    #[test]
    fn generate_refuses_an_interface_with_two_bases() {
        let header = r"
struct IOther
{
    TEST_UUID
    virtual void other() = 0;
};
struct IBoth : public IRoot, public IOther
{
    TEST_UUID
};
";
        let error = fail("multiple_bases", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "struct IBoth"),
            &["interface IBoth has 2 base classes"],
        );
    }

    #[test]
    fn generate_refuses_an_overloaded_method() {
        let header = r"
struct IOverloads : public IRoot
{
    TEST_UUID
    virtual void set(int32_t value) = 0;
    virtual void set(const char* value) = 0;
};
";
        let error = fail("overload", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "set(const char"),
            &["method IOverloads::set overloads or overrides"],
        );
    }

    #[test]
    fn generate_refuses_a_method_that_is_not_pure_virtual() {
        let header = r"
struct IImpure : public IRoot
{
    TEST_UUID
    virtual void act() {}
};
";
        let error = fail("not_pure", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "act()"),
            &["method IImpure::act is not pure virtual"],
        );
    }

    #[test]
    fn generate_refuses_a_method_with_another_calling_convention() {
        let header = r"
#if defined(__linux__) && defined(__x86_64__)
#define TEST_ODD_CALL __attribute__((ms_abi))
#else
#define TEST_ODD_CALL
#endif
struct IConvention : public IRoot
{
    TEST_UUID
    virtual void normal() = 0;
    virtual void TEST_ODD_CALL odd() = 0;
};
";
        let error = fail("calling_convention", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "odd()"),
            &[
                "method IConvention::odd uses the calling convention",
                "on target x86_64-unknown-linux-gnu",
                "differs from the C calling convention",
            ],
        );
    }

    #[test]
    fn generate_refuses_an_included_function_with_cxx_linkage() {
        let header = "\nvoid mangled(int32_t value);\n";
        let error = fail("cxx_linkage", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "mangled"),
            &["function mangled has C++ linkage"],
        );
    }

    #[test]
    fn generate_refuses_an_included_inline_function() {
        let header = "\nextern \"C\" inline int32_t helper() { return 1; }\n";
        let error = fail("inline_function", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "helper"),
            &["function helper is defined inline"],
        );
    }

    #[test]
    fn generate_refuses_an_excluded_type_used_by_value() {
        const DECLARATIONS: &[DeclRule] = &[
            DeclRule {
                header: Some("slang.h"),
                pattern: "Hidden",
                action: Action::Exclude,
                reason: "a hidden test struct",
            },
            DeclRule {
                header: None,
                pattern: "*",
                action: Action::Include,
                reason: "a test declaration",
            },
        ];
        const RULES: RuleSet = rules(DECLARATIONS, NO_MACROS);
        let header =
            "\nstruct Hidden\n{\n    int32_t a;\n};\nstruct Shown\n{\n    Hidden hidden;\n};\n";
        let error = fail_rules("excluded_by_value", header, &RULES);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "Hidden hidden"),
            &[
                "field Shown::hidden uses Hidden by value",
                "a hidden test struct",
            ],
        );
    }

    #[test]
    fn generate_refuses_a_bit_field() {
        let header = "\nstruct Bits\n{\n    uint32_t low : 8;\n    uint32_t high : 24;\n};\n";
        let error = fail("bit_field", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "low : 8"),
            &["field Bits::low is a bit-field"],
        );
    }

    #[test]
    fn generate_refuses_an_empty_record() {
        let header = "\nstruct Empty\n{\n};\n";
        let error = fail("empty_record", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "struct Empty"),
            &["record Empty has no fields"],
        );
    }

    #[test]
    fn generate_refuses_an_interface_with_a_data_member() {
        let header = r"
struct IData : public IRoot
{
    TEST_UUID
    int32_t count;
};
";
        let error = fail("interface_data_member", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "struct IData"),
            &["interface IData has a data member"],
        );
    }

    #[test]
    fn generate_refuses_an_interface_with_a_virtual_destructor() {
        let header = r"
struct IDestroyed : public IRoot
{
    TEST_UUID
    virtual ~IDestroyed() = 0;
};
";
        let error = fail("virtual_destructor", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "struct IDestroyed"),
            &["interface IDestroyed has a virtual destructor"],
        );
    }

    #[test]
    fn generate_refuses_a_record_with_a_base_class() {
        let header = r"
struct Base
{
    int32_t a;
};
struct Derived : public Base
{
    int32_t b;
};
";
        let error = fail("record_base", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "struct Derived"),
            &["record Derived has a base class"],
        );
    }

    #[test]
    fn generate_refuses_a_default_member_initializer_in_a_union() {
        let header = r"
union Choice
{
    int32_t whole = 1;
    float part;
};
";
        let error = fail("union_initializer", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "int32_t whole"),
            &["field Choice::whole has a default member initializer in a union"],
        );
    }

    #[test]
    fn generate_refuses_an_array_parameter() {
        let header = "\nextern \"C\" void takeArray(int32_t values[4]);\n";
        let error = fail("array_parameter", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "takeArray"),
            &["parameter values of function takeArray is an array, which C passes as a pointer"],
        );
    }

    #[test]
    fn generate_refuses_void_outside_a_pointer() {
        let header = "\ntypedef void Nothing;\n";
        let error = fail("void_outside_pointer", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "typedef void Nothing"),
            &["typedef Nothing has the type void outside a pointer"],
        );
    }

    #[test]
    fn generate_refuses_two_items_with_the_same_rust_name() {
        let header = "\nnamespace first\n{\nstruct Twin\n{\n    int32_t a;\n};\n}\nstruct Twin\n{\n    int32_t b;\n};\n";
        let error = fail("name_collision", header);
        let second = line_of(header, "int32_t b") - 2;
        assert_error(
            &error,
            ErrorKind::NameCollision,
            second,
            &["Twin and first::Twin", "would both have the Rust name Twin"],
        );
    }

    #[test]
    fn generate_refuses_headers_with_another_build_tag() {
        let _guard = LIBCLANG.lock().unwrap_or_else(PoisonError::into_inner);
        let headers = Headers::new("other_tag", "", "2000.1-1-gdeadbeef");
        let error = generate_with(&DEFAULT_RULES, &headers.dir, pin::SLANG_BUILD_TAG)
            .expect_err("a different build tag is refused");
        assert_eq!(error.kind(), ErrorKind::Version);
        let location = error.location().expect("the error has a location");
        assert_eq!(
            (location.file(), location.line()),
            ("slang-tag-version.h", 1)
        );
        assert!(error.message().contains("2000.1-1-gdeadbeef"), "{error}");
        assert!(error.message().contains(pin::SLANG_BUILD_TAG), "{error}");
        assert!(error.message().contains(pin::VULKAN_SDK_VERSION), "{error}");
    }

    #[test]
    fn generate_refuses_headers_without_a_build_tag() {
        let _guard = LIBCLANG.lock().unwrap_or_else(PoisonError::into_inner);
        let headers = Headers::new("missing_tag", "", pin::SLANG_BUILD_TAG);
        std::fs::remove_file(headers.dir.join("slang-tag-version.h")).expect("remove the tag");
        let error = generate_with(&DEFAULT_RULES, &headers.dir, pin::SLANG_BUILD_TAG)
            .expect_err("missing build tag is refused");
        assert_eq!(error.kind(), ErrorKind::Version);
        assert!(error.message().contains("slang-tag-version.h"), "{error}");
    }

    #[test]
    fn generate_fails_without_the_header_directory() {
        let missing = std::env::temp_dir().join("bindings-generator-test-no-such-directory");
        let error = generate_with(&DEFAULT_RULES, &missing, pin::SLANG_BUILD_TAG)
            .expect_err("a missing directory is refused");
        assert_eq!(error.kind(), ErrorKind::Io);
        assert!(error.message().contains("no-such-directory"), "{error}");
    }

    #[test]
    fn generate_fails_on_an_enumerator_outside_the_underlying_type() {
        let header = "\nenum Letter : char\n{\n    LETTER_NEGATIVE = -1,\n};\n";
        let error = fail("enumerator_range", header);
        assert_error(
            &error,
            ErrorKind::Evaluation,
            line_of(header, "enum Letter"),
            &[
                "enumerator Letter::LETTER_NEGATIVE has the value -1",
                "does not fit the underlying type of the enumeration",
            ],
        );
    }

    #[test]
    fn generate_refuses_nullptr_on_a_field_that_is_not_a_pointer() {
        let header = r"
struct FromNull
{
    FromNull(decltype(nullptr)) {}
    int32_t value;
};
struct Holder
{
    FromNull wrapped = nullptr;
};
";
        let error = fail("nullptr_not_pointer", header);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "FromNull wrapped"),
            &["field Holder::wrapped is initialized with nullptr but is not a pointer"],
        );
    }

    #[test]
    fn generate_fails_on_a_constant_without_a_value() {
        let header = "\nextern const int32_t kExternal;\n";
        let error = fail("constant_without_value", header);
        assert_error(
            &error,
            ErrorKind::Evaluation,
            line_of(header, "kExternal"),
            &["cannot evaluate the constant kExternal"],
        );
    }

    #[test]
    fn generate_refuses_a_pointer_sized_typedef_of_a_type_that_is_not_an_integer() {
        const RULES: RuleSet = RuleSet {
            pointer_sized: &[("SlangInt", Prim::Isize), ("Ratio", Prim::Isize)],
            ..DEFAULT_RULES
        };
        let header = "\ntypedef double Ratio;\n";
        let error = fail_rules("pointer_sized_float", header, &RULES);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of(header, "Ratio"),
            &["typedef Ratio is mapped to isize but is not an integer type"],
        );
    }

    #[test]
    fn generate_refuses_a_non_finite_floating_point_initializer() {
        for (test, value) in [
            ("infinite_initializer", "__builtin_inf()"),
            ("nan_initializer", "__builtin_nan(\"\")"),
        ] {
            let header = format!("\nstruct Limits\n{{\n    double bound = {value};\n}};\n");
            let error = fail(test, &header);
            assert_error(
                &error,
                ErrorKind::Evaluation,
                line_of(&header, "double bound"),
                &[
                    "field Limits::bound has the value",
                    "which is not a finite number and has no Rust literal",
                ],
            );
        }
    }

    #[test]
    fn generate_writes_finite_floating_point_initializers_as_literals() {
        let text = generate(
            "float_initializers",
            "\nstruct Scales\n{\n    double tiny = 1e-300;\n    float half = 0.5f;\n    double whole = 3;\n};\n",
        );
        assert_contains(&text, "tiny: 1e-300,");
        assert_contains(&text, "half: 0.5,");
        assert_contains(&text, "whole: 3.0,");
    }

    #[test]
    fn generate_emits_an_anonymous_record_nested_in_another_anonymous_record() {
        let header = r"
struct Vec2
{
    union
    {
        struct
        {
            float x;
            float y;
        };
        float v[2];
    };
};
";
        let text = generate("nested_anonymous", header);
        assert_contains(
            &text,
            "pub struct Vec2_anon0_anon0 {
    pub x: f32,
    pub y: f32,
}",
        );
        assert_contains(
            &text,
            "pub union Vec2_anon0 {
    pub anon0: Vec2_anon0_anon0,
    pub v: [f32; 2],
}",
        );
        assert_contains(&text, "pub struct Vec2 {\n    pub anon0: Vec2_anon0,\n}");
        assert_contains(&text, "assert!(size_of::<Vec2>() == 8);");
        assert_contains(
            &text,
            "assert!(::core::mem::offset_of!(Vec2_anon0_anon0, y) == 4);",
        );
    }

    #[test]
    fn generate_refuses_interfaces_when_a_rule_excludes_the_uuid_type() {
        const DECLARATIONS: &[DeclRule] = &[
            DeclRule {
                header: Some("slang.h"),
                pattern: "SlangUUID",
                action: Action::Exclude,
                reason: "a hidden UUID type",
            },
            DeclRule {
                header: None,
                pattern: "*",
                action: Action::Include,
                reason: "a test declaration",
            },
        ];
        const RULES: RuleSet = rules(DECLARATIONS, NO_MACROS);
        let error = fail_rules("uuid_excluded", "", &RULES);
        assert_error(
            &error,
            ErrorKind::Unsupported,
            line_of("", "struct IRoot"),
            &["interface IRoot needs the struct SlangUUID with four fields for its UUID"],
        );
    }

    const SESSION_FLAGS: &str = r"
namespace scope
{
typedef uint32_t SessionFlags;
enum
{
    kSessionFlags_None = 0,
    kSessionFlags_Last = 0x40000000,
};
struct SessionDesc
{
    SessionFlags flags = kSessionFlags_None;
};
}
";

    const SESSION_RULES: RuleSet = RuleSet {
        unnamed_enums: &[UnnamedEnumRule {
            header: "slang.h",
            enumerator: "kSessionFlags_None",
            typedef: "scope::SessionFlags",
            reason: "the session flags",
        }],
        ..DEFAULT_RULES
    };

    #[test]
    fn generate_types_the_constants_of_an_unnamed_enumeration_by_its_typedef() {
        let text = generate_rules("unnamed_assigned", SESSION_FLAGS, &SESSION_RULES);
        assert_contains(&text, "pub type SessionFlags = u32;");
        assert_contains(&text, "pub const kSessionFlags_None: SessionFlags = 0;");
        assert_contains(
            &text,
            "pub const kSessionFlags_Last: SessionFlags = 1_073_741_824;",
        );
        assert_contains(&text, "Self { flags: 0 }");
    }

    #[test]
    fn generate_keeps_unnamed_enumerations_at_the_same_line_of_two_headers() {
        const RULES: RuleSet = RuleSet {
            unnamed_enums: &[
                UnnamedEnumRule {
                    header: "slang.h",
                    enumerator: "kMain_One",
                    typedef: "MainFlags",
                    reason: "the main flags",
                },
                UnnamedEnumRule {
                    header: "slang-extra.h",
                    enumerator: "kExtra_One",
                    typedef: "ExtraFlags",
                    reason: "the extra flags",
                },
            ],
            ..DEFAULT_RULES
        };
        let header = r#"
typedef uint32_t MainFlags;
typedef uint32_t ExtraFlags;
enum
{
    kMain_One = 1,
};
#include "slang-extra.h"
"#;
        let padding = "\n".repeat(usize::try_from(line_of(header, "enum")).expect("fits") - 1);
        let extra = format!("{padding}enum\n{{\n    kExtra_One = 2,\n}};\n");
        let text = generate_with_extra_header(
            "unnamed_same_line",
            header,
            ("slang-extra.h", &extra),
            &RULES,
        );
        assert_contains(&text, "pub const kMain_One: MainFlags = 1;");
        assert_contains(&text, "pub const kExtra_One: ExtraFlags = 2;");
    }

    #[test]
    fn generate_fails_on_an_unnamed_enumeration_that_no_rule_assigns() {
        let error = fail("unnamed_unassigned", SESSION_FLAGS);
        assert_error(
            &error,
            ErrorKind::UnmatchedDeclaration,
            line_of(SESSION_FLAGS, "enum"),
            &["the unnamed enumeration that starts with kSessionFlags_None (slang.h:"],
        );
    }

    #[test]
    fn generate_fails_on_a_value_outside_the_typedef_of_an_unnamed_enumeration() {
        const RULES: RuleSet = RuleSet {
            unnamed_enums: &[UnnamedEnumRule {
                header: "slang.h",
                enumerator: "kNarrow_Big",
                typedef: "NarrowFlags",
                reason: "the narrow flags",
            }],
            ..DEFAULT_RULES
        };
        let header = "\ntypedef uint8_t NarrowFlags;\nenum\n{\n    kNarrow_Big = 256,\n};\n";
        let error = fail_rules("unnamed_too_large", header, &RULES);
        assert_error(
            &error,
            ErrorKind::Evaluation,
            line_of(header, "enum"),
            &["enumerator kNarrow_Big has the value 256, which does not fit the type NarrowFlags"],
        );
    }

    #[test]
    fn generate_fails_on_rules_that_decide_nothing() {
        const DECLARATIONS: &[DeclRule] = &[
            DeclRule {
                header: Some("slang.h"),
                pattern: "Gone*",
                action: Action::Exclude,
                reason: "a removed declaration",
            },
            DeclRule {
                header: None,
                pattern: "*",
                action: Action::Include,
                reason: "a test declaration",
            },
            DeclRule {
                header: None,
                pattern: "IRoot",
                action: Action::Exclude,
                reason: "a shadowed rule",
            },
        ];
        const MACROS: &[MacroRule] = &[
            MacroRule {
                pattern: "OLD_MACRO",
                action: MacroAction::Exclude,
                reason: "a removed macro",
            },
            MacroRule {
                pattern: "*",
                action: MacroAction::Exclude,
                reason: "a test macro",
            },
        ];
        const RULES: RuleSet = RuleSet {
            unnamed_enums: &[UnnamedEnumRule {
                header: "slang.h",
                enumerator: "kGone_None",
                typedef: "GoneFlags",
                reason: "removed flags",
            }],
            ..rules(DECLARATIONS, MACROS)
        };
        let error = fail_rules("stale_rules", "", &RULES);
        assert_eq!(error.kind(), ErrorKind::StaleRule, "{error}");
        assert_eq!(error.location(), None, "{error}");
        for needle in [
            "4 rules of the rule set decide no declaration or macro",
            "the declaration rule Gone* in slang.h (a removed declaration)",
            "the declaration rule IRoot (a shadowed rule)",
            "the macro rule OLD_MACRO (a removed macro)",
            "the unnamed enumeration with the enumerator kGone_None in slang.h to GoneFlags (removed flags)",
        ] {
            assert!(
                error.message().contains(needle),
                "{needle:?} not in: {error}"
            );
        }
        assert!(
            !error.message().contains("a test declaration"),
            "a used rule is reported: {error}"
        );
    }

    #[test]
    fn generate_says_when_this_process_already_holds_libclang() {
        let _guard = LIBCLANG.lock().unwrap_or_else(PoisonError::into_inner);
        let headers = Headers::new("libclang_twice", "", pin::SLANG_BUILD_TAG);
        let clang = clang::Clang::new().expect("load libclang");
        let error = generate_with(&DEFAULT_RULES, &headers.dir, pin::SLANG_BUILD_TAG)
            .expect_err("a second instance is refused");
        drop(clang);
        assert_eq!(error.kind(), ErrorKind::Libclang);
        assert!(
            error
                .message()
                .contains("this process already holds a libclang instance"),
            "{error}"
        );
        assert!(!error.message().contains("LIBCLANG_PATH"), "{error}");
    }
}

mod tag {
    use crate::check_tag_text;
    use crate::error::{Error, ErrorKind};

    const PATH: &str = "/tmp/headers/slang-tag-version.h";
    const EXPECTED: &str = "2026.17-25-gc7954ebd1";

    fn refuse(text: &str) -> Error {
        check_tag_text(text, PATH, EXPECTED).expect_err("the tag is refused")
    }

    fn assert_malformed(text: &str, line: u32) {
        let error = refuse(text);
        assert_eq!(error.kind(), ErrorKind::Version, "{error}");
        let location = error.location().expect("the error has a location");
        assert_eq!(
            (location.file(), location.line()),
            ("slang-tag-version.h", line),
            "{error}"
        );
        assert_eq!(
            error.message(),
            "SLANG_TAG_VERSION is not defined as a single string literal"
        );
    }

    #[test]
    fn check_tag_text_accepts_the_expected_tag() {
        let text = "#pragma once\n#define SLANG_TAG_VERSION \"2026.17-25-gc7954ebd1\"\n";
        check_tag_text(text, PATH, EXPECTED).expect("the tag is accepted");
    }

    #[test]
    fn check_tag_text_refuses_a_file_without_the_define() {
        let error = refuse("#define SLANG_VERSION_NUMERIC \"2026.17\"\n");
        assert_eq!(error.kind(), ErrorKind::Version, "{error}");
        assert_eq!(error.location(), None, "{error}");
        assert_eq!(
            error.message(),
            "/tmp/headers/slang-tag-version.h does not define SLANG_TAG_VERSION"
        );
    }

    #[test]
    fn check_tag_text_refuses_a_value_that_is_not_a_string_literal() {
        assert_malformed("#define SLANG_TAG_VERSION 2026\n", 1);
    }

    #[test]
    fn check_tag_text_refuses_two_string_literals() {
        assert_malformed("\n#define SLANG_TAG_VERSION \"a\" \"b\"\n", 2);
    }

    #[test]
    fn check_tag_text_refuses_an_unterminated_string_literal() {
        assert_malformed("#define SLANG_TAG_VERSION \"2026.17-25-gc7954ebd1\n", 1);
    }

    #[test]
    fn check_tag_text_refuses_a_define_without_a_value() {
        assert_malformed("#define SLANG_TAG_VERSION\n", 1);
    }

    #[test]
    fn check_tag_text_uses_the_last_define() {
        let error = refuse(
            "#define SLANG_TAG_VERSION \"2026.17-25-gc7954ebd1\"\n#undef SLANG_TAG_VERSION\n#define SLANG_TAG_VERSION \"2000.1-1-gdeadbeef\"\n",
        );
        assert_eq!(error.kind(), ErrorKind::Version, "{error}");
        assert_eq!(error.location().map(crate::Location::line), Some(3));
        assert!(error.message().contains("2000.1-1-gdeadbeef"), "{error}");
    }
}
