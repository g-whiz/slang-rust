use std::path::PathBuf;
use std::sync::PoisonError;

use super::{LIBCLANG, PRELUDE, TARGET, assert_contains, compact};
use crate::error::{Error, ErrorKind};
use crate::model::Prim;
use crate::rules::{Action, DeclRule, MacroAction, MacroRule, NameRule, OverloadRule, RuleSet};
use crate::{Generated, generate_with_external};

const SLANG_EXTRA: &str = r"
struct Hidden
{
    int32_t value;
};
struct Visible
{
    int32_t value;
};
namespace Slang
{
template <class T>
class ComPtr
{
public:
    ComPtr() : m_ptr(nullptr) {}
    T* m_ptr;
};
}
";

const SLANG_DECLARATIONS: &[DeclRule] = &[
    DeclRule {
        header: None,
        pattern: "Hidden",
        action: Action::Exclude,
        reason: "a test type that the Slang bindings leave out",
    },
    DeclRule {
        header: None,
        pattern: "Slang::*",
        action: Action::Exclude,
        reason: "a test template",
    },
    DeclRule {
        header: None,
        pattern: "*",
        action: Action::Include,
        reason: "a test declaration",
    },
];

const ALL_MACROS: &[MacroRule] = &[MacroRule {
    header: None,
    pattern: "*",
    action: MacroAction::Exclude,
    reason: "a test macro",
}];

const TEST_SLANG: RuleSet = super::rules(SLANG_DECLARATIONS, ALL_MACROS);

const RHI_DECLARATIONS: &[DeclRule] = &[
    DeclRule {
        header: Some("slang.h"),
        pattern: "*",
        action: Action::External,
        reason: "a declaration of the Slang headers",
    },
    DeclRule {
        header: None,
        pattern: "*",
        action: Action::Include,
        reason: "a test declaration",
    },
];

#[derive(Clone, Copy)]
struct Extra {
    members: &'static [NameRule],
    templates: &'static [NameRule],
    uuidless: &'static [NameRule],
    overloads: &'static [OverloadRule],
    placeholders: &'static [NameRule],
    called_callbacks: &'static [NameRule],
    without_default: &'static [NameRule],
}

const NO_EXTRA: Extra = Extra {
    members: &[],
    templates: &[],
    uuidless: &[],
    overloads: &[],
    placeholders: &[],
    called_callbacks: &[],
    without_default: &[],
};

const fn rhi_rules(extra: Extra) -> RuleSet {
    RuleSet {
        main_header: "rhi.h",
        language: "c++20",
        defines: &["TEST_DEFINE=\"rhi-define\""],
        library: "slang-rhi",
        own_crate: "slang_rhi_sys",
        external_crate: Some("slang_compiler_sys"),
        declarations: RHI_DECLARATIONS,
        macros: ALL_MACROS,
        unnamed_enums: &[],
        result_type: "SlangResult",
        uuid_type: "SlangUUID",
        pointer_sized: &[("SlangInt", Prim::Isize)],
        members: Some(extra.members),
        templates: extra.templates,
        uuidless: extra.uuidless,
        overloads: extra.overloads,
        placeholders: extra.placeholders,
        called_callbacks: extra.called_callbacks,
        without_default: extra.without_default,
        abi_probe: true,
        initializers_in_place: true,
    }
}

const fn name_rule(pattern: &'static str) -> NameRule {
    NameRule {
        pattern,
        reason: "a test rule",
    }
}

const RHI_PRELUDE: &str = "#pragma once\n#ifndef TEST_DEFINE\n#error the parse lacks the definitions of the rule set\n#endif\n#include <slang.h>\n#include <rhi-config.h>\n";

struct Dirs {
    root: PathBuf,
}

impl Dirs {
    fn new(test: &str, header: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "slang-bindgen-rhi-test-{}-{test}",
            std::process::id()
        ));
        for dir in ["rhi", "config", "slang"] {
            std::fs::create_dir_all(root.join(dir)).expect("create a header directory");
        }
        std::fs::write(
            root.join("rhi").join("rhi.h"),
            format!("{RHI_PRELUDE}{header}"),
        )
        .expect("write rhi.h");
        std::fs::write(
            root.join("config").join("rhi-config.h"),
            "#pragma once\nconstexpr int kConfigured = 1;\n",
        )
        .expect("write rhi-config.h");
        std::fs::write(
            root.join("slang").join("slang.h"),
            format!("{PRELUDE}{SLANG_EXTRA}"),
        )
        .expect("write slang.h");
        Self { root }
    }
}

impl Drop for Dirs {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) {
            eprintln!("cannot remove {}: {error}", self.root.display());
        }
    }
}

fn run(test: &str, header: &str, rules: &RuleSet, target: &str) -> crate::Result<Generated> {
    let _guard = LIBCLANG.lock().unwrap_or_else(PoisonError::into_inner);
    let dirs = Dirs::new(test, header);
    generate_with_external(
        &TEST_SLANG,
        rules,
        [
            &dirs.root.join("rhi"),
            &dirs.root.join("config"),
            &dirs.root.join("slang"),
        ],
        target,
    )
}

fn generate(test: &str, header: &str, rules: &RuleSet) -> Generated {
    run(test, header, rules, TARGET).unwrap_or_else(|error| panic!("generation failed: {error}"))
}

fn fail(test: &str, header: &str, rules: &RuleSet) -> Error {
    run(test, header, rules, TARGET).expect_err("generation fails")
}

fn assert_failure(error: &Error, kind: ErrorKind, needles: &[&str]) {
    assert_eq!(error.kind(), kind, "{error}");
    for needle in needles {
        assert!(
            error.message().contains(needle),
            "{needle:?} not in: {error}"
        );
    }
}

const RHI_INTERFACE: &str = r"
namespace rhi
{
struct IThing : public IRoot
{
    TEST_UUID
    virtual SlangInt count() = 0;
};
struct Holder
{
    SlangInt size;
    Visible visible;
    IRoot* root;
};
}
";

#[test]
fn generate_slang_rhi_refers_to_every_slang_declaration_through_slang_compiler_sys() {
    let generated = generate("external", RHI_INTERFACE, &rhi_rules(NO_EXTRA));
    let text = &generated.bindings;
    assert_contains(
        text,
        "pub struct IThingVtbl {
    pub base: ::slang_compiler_sys::IRootVtbl,
    pub count: unsafe extern \"system-unwind\" fn(this: *mut IThing) -> ::slang_compiler_sys::SlangInt,
}",
    );
    assert_contains(
        text,
        "unsafe impl ::slang_compiler_sys::Interface for IThing {
    const UUID: ::slang_compiler_sys::SlangUUID = ::slang_compiler_sys::SlangUUID {",
    );
    assert_contains(
        text,
        "pub struct Holder {
    pub size: ::slang_compiler_sys::SlangInt,
    pub visible: ::slang_compiler_sys::Visible,
    pub root: *mut ::slang_compiler_sys::IRoot,
}",
    );
    assert_contains(
        text,
        "pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }",
    );
    assert_contains(text, "pub const kConfigured: ::core::ffi::c_int = 1;");
    assert_contains(
        text,
        "compile_error!(\"the slang-rhi bindings support only 64-bit targets\");",
    );
    for defined in [
        "pub struct IRoot ",
        "pub struct SlangUUID",
        "pub unsafe trait Interface",
        "pub type SlangInt",
    ] {
        assert!(!text.contains(defined), "{defined}");
    }
}

#[test]
fn generate_slang_rhi_refuses_a_slang_type_that_slang_compiler_sys_does_not_declare() {
    for (test, header) in [
        (
            "external_hidden",
            "namespace rhi { struct UsesHidden { Hidden* hidden; }; }",
        ),
        (
            "external_hidden_value",
            "namespace rhi { struct UsesHidden { Hidden hidden; }; }",
        ),
    ] {
        let error = fail(test, header, &rhi_rules(NO_EXTRA));
        assert_failure(
            &error,
            ErrorKind::Unsupported,
            &[
                "field rhi::UsesHidden::hidden refers to the Slang type Hidden, which the bindings of slang_compiler_sys do not declare",
            ],
        );
    }
}

const CALLBACK_RULES: RuleSet = rhi_rules(Extra {
    uuidless: &[name_rule("rhi::ICallback")],
    ..NO_EXTRA
});

const NO_MEMBERS: RuleSet = rhi_rules(Extra {
    templates: &[name_rule("Slang::ComPtr")],
    ..NO_EXTRA
});

const NO_TEMPLATE: RuleSet = rhi_rules(Extra {
    members: &[
        name_rule("rhi::Binding::Binding"),
        name_rule("rhi::Binding::operator=="),
    ],
    ..NO_EXTRA
});

const WRONG_PLACEHOLDER_RULES: RuleSet = rhi_rules(Extra {
    placeholders: &[
        name_rule("rhi::IEncoder::clear"),
        name_rule("rhi::IEncoder::current"),
        name_rule("rhi::IEncoder::finish"),
    ],
    ..NO_EXTRA
});

const UUIDLESS_HEADER: &str = r"
namespace rhi
{
class ICallback
{
public:
    virtual void handle(int32_t kind, const char* message) = 0;
    virtual int32_t count() const = 0;
};
}
";

#[test]
fn generate_slang_rhi_emits_a_class_without_uuid_that_a_rule_names_as_an_interface_without_the_trait()
 {
    const RULES: RuleSet = rhi_rules(Extra {
        uuidless: &[name_rule("rhi::ICallback")],
        ..NO_EXTRA
    });
    let generated = generate("uuidless", UUIDLESS_HEADER, &RULES);
    assert_contains(
        &generated.bindings,
        "pub struct ICallbackVtbl {
    pub handle: unsafe extern \"system-unwind\" fn(this: *mut ICallback, kind: i32, message: *const ::core::ffi::c_char),
    pub count: unsafe extern \"system-unwind\" fn(this: *mut ICallback) -> i32,
}",
    );
    assert!(!generated.bindings.contains("Interface for ICallback"));
    let probe = generated.abi_probe.unwrap_or_default();
    assert_contains(
        &probe,
        "slang_rhi_sys_slot(static_cast<int (rhi::ICallback::*)() const>(&rhi::ICallback::count)),",
    );
}

#[test]
fn generate_slang_rhi_refuses_a_class_without_uuid_without_a_rule_or_with_a_base_or_destructor() {
    let error = fail("uuidless_no_rule", UUIDLESS_HEADER, &rhi_rules(NO_EXTRA));
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &[
            "rhi::ICallback is a class with virtual methods and no UUID, which the bindings cannot express",
        ],
    );
    let error = fail(
        "uuidless_base",
        "namespace rhi { class IBase { public: virtual void a() = 0; }; class ICallback : public IBase { public: virtual void b() = 0; }; }",
        &CALLBACK_RULES,
    );
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &["rhi::IBase is a class with virtual methods and no UUID"],
    );
    let error = fail(
        "uuidless_destructor",
        "namespace rhi { class ICallback { public: virtual ~ICallback() = default; virtual void b() = 0; }; }",
        &CALLBACK_RULES,
    );
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &["interface rhi::ICallback has a virtual destructor"],
    );
}

#[test]
fn generate_slang_rhi_refuses_a_class_without_uuid_that_has_a_base_class() {
    const RULES: RuleSet = rhi_rules(Extra {
        uuidless: &[name_rule("rhi::IBase"), name_rule("rhi::ICallback")],
        ..NO_EXTRA
    });
    let error = fail(
        "uuidless_named_base",
        "namespace rhi { class IBase { public: virtual void a() = 0; }; class ICallback : public IBase { public: virtual void b() = 0; }; }",
        &RULES,
    );
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &["class rhi::ICallback has virtual methods, no UUID, and a base class"],
    );
}

const OVERLOAD_HEADER: &str = r"
namespace rhi
{
struct IReader : public IRoot
{
    TEST_UUID
    virtual SlangResult read(int32_t index, void* out) = 0;
    virtual void other() = 0;
    virtual SlangResult read(int32_t index, IRoot** out) = 0;
};
}
";

const OVERLOADS: &[OverloadRule] = &[
    OverloadRule {
        method: "rhi::IReader::read",
        params: &["int32_t", "void *"],
        rust: "read",
        reason: "a test overload",
    },
    OverloadRule {
        method: "rhi::IReader::read",
        params: &["int32_t", "IRoot **"],
        rust: "read_root",
        reason: "a test overload",
    },
];

#[test]
fn generate_slang_rhi_names_overloads_by_rule_in_declaration_order() {
    const RULES: RuleSet = rhi_rules(Extra {
        overloads: OVERLOADS,
        ..NO_EXTRA
    });
    let generated = generate("overloads", OVERLOAD_HEADER, &RULES);
    assert_contains(
        &generated.bindings,
        "pub struct IReaderVtbl {
    pub base: ::slang_compiler_sys::IRootVtbl,
    pub read: unsafe extern \"system-unwind\" fn(this: *mut IReader, index: i32, out: *mut ::core::ffi::c_void) -> ::slang_compiler_sys::SlangResult,
    pub other: unsafe extern \"system-unwind\" fn(this: *mut IReader),
    pub read_root: unsafe extern \"system-unwind\" fn(this: *mut IReader, index: i32, out: *mut *mut ::slang_compiler_sys::IRoot) -> ::slang_compiler_sys::SlangResult,
}",
    );
    assert_contains(
        &generated.bindings,
        "pub unsafe fn read_root(this: *mut Self, index: i32, out: *mut *mut ::slang_compiler_sys::IRoot) -> ::slang_compiler_sys::SlangResult {
        unsafe { ((*(*this).vtable).read_root)(this, index, out) }
    }",
    );
    let probe = generated.abi_probe.unwrap_or_default();
    assert_contains(
        &probe,
        "slang_rhi_sys_slot(static_cast<int (rhi::IReader::*)(int, void *)>(&rhi::IReader::read)),
        slang_rhi_sys_slot(static_cast<void (rhi::IReader::*)()>(&rhi::IReader::other)),
        slang_rhi_sys_slot(static_cast<int (rhi::IReader::*)(int, IRoot **)>(&rhi::IReader::read)),",
    );
    assert_contains(
        &generated.symbols,
        "(\"IReader::read_root\", ::core::mem::offset_of!(slang_rhi_sys::IReaderVtbl, read_root)),",
    );
}

#[test]
fn generate_slang_rhi_refuses_overloads_without_a_rule() {
    let error = fail("overloads_no_rule", OVERLOAD_HEADER, &rhi_rules(NO_EXTRA));
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &[
            "method rhi::IReader::read overloads or overrides another method of the same name",
            "(int32_t, IRoot **)",
        ],
    );
}

const ADJACENT_OVERLOAD_HEADER: &str = r"
namespace rhi
{
struct IReader : public IRoot
{
    TEST_UUID
    virtual void other() = 0;
    virtual SlangResult read(int32_t index, void* out) = 0;
    virtual SlangResult read(int32_t index, IRoot** out) = 0;
};
}
";

#[test]
fn generate_slang_rhi_reverses_a_run_of_overloads_for_a_target_of_the_microsoft_abi() {
    const RULES: RuleSet = rhi_rules(Extra {
        overloads: OVERLOADS,
        ..NO_EXTRA
    });
    let generated = run(
        "overloads_msvc",
        ADJACENT_OVERLOAD_HEADER,
        &RULES,
        "x86_64-pc-windows-msvc",
    )
    .unwrap_or_else(|error| panic!("generation failed: {error}"));
    assert_contains(
        &generated.bindings,
        "pub struct IReaderVtbl {
    pub base: ::slang_compiler_sys::IRootVtbl,
    pub other: unsafe extern \"system-unwind\" fn(this: *mut IReader),
    pub read_root: unsafe extern \"system-unwind\" fn(this: *mut IReader, index: i32, out: *mut *mut ::slang_compiler_sys::IRoot) -> ::slang_compiler_sys::SlangResult,
    pub read: unsafe extern \"system-unwind\" fn(this: *mut IReader, index: i32, out: *mut ::core::ffi::c_void) -> ::slang_compiler_sys::SlangResult,
}",
    );
    let probe = generated.abi_probe.unwrap_or_default();
    assert_contains(
        &probe,
        "slang_rhi_sys_slot(static_cast<void (rhi::IReader::*)()>(&rhi::IReader::other)),
        slang_rhi_sys_slot(static_cast<int (rhi::IReader::*)(int, IRoot **)>(&rhi::IReader::read)),
        slang_rhi_sys_slot(static_cast<int (rhi::IReader::*)(int, void *)>(&rhi::IReader::read)),",
    );
    assert_contains(&probe, "slang_rhi_sys_vcall_offset");
    assert_contains(
        &generated.symbols,
        "(\"IReader::read_root\", ::core::mem::offset_of!(slang_rhi_sys::IReaderVtbl, read_root)),",
    );
}

#[test]
fn generate_slang_rhi_keeps_declaration_order_of_a_run_of_overloads_on_the_itanium_abi() {
    const RULES: RuleSet = rhi_rules(Extra {
        overloads: OVERLOADS,
        ..NO_EXTRA
    });
    let generated = generate("overloads_adjacent", ADJACENT_OVERLOAD_HEADER, &RULES);
    assert_contains(
        &generated.bindings,
        "    pub read: unsafe extern \"system-unwind\" fn(this: *mut IReader, index: i32, out: *mut ::core::ffi::c_void) -> ::slang_compiler_sys::SlangResult,
    pub read_root: unsafe extern",
    );
}

#[test]
fn generate_slang_rhi_refuses_scattered_overloads_for_a_target_of_the_microsoft_abi() {
    const RULES: RuleSet = rhi_rules(Extra {
        overloads: OVERLOADS,
        ..NO_EXTRA
    });
    let error = run(
        "overloads_scattered_msvc",
        OVERLOAD_HEADER,
        &RULES,
        "x86_64-pc-windows-msvc",
    )
    .expect_err("generation fails");
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &[
            "virtual methods share a name but are not declared next to each other on target x86_64-pc-windows-msvc",
            "rhi::IReader::read(int32_t, void *)",
            "rhi::IReader::read(int32_t, IRoot **)",
        ],
    );
}

const EARLY_HELPER_HEADER: &str = r"
namespace rhi
{
struct IReader : public IRoot
{
    TEST_UUID
    void other(int32_t) {}
    virtual SlangResult read(int32_t index, void* out) = 0;
    virtual void other() = 0;
};
}
";

const LATE_HELPER_HEADER: &str = r"
namespace rhi
{
struct IReader : public IRoot
{
    TEST_UUID
    virtual SlangResult read(int32_t index, void* out) = 0;
    virtual void other() = 0;
    void other(int32_t) {}
};
}
";

const EARLY_TYPE_HEADER: &str = r"
namespace rhi
{
struct IReader : public IRoot
{
    TEST_UUID
    struct other
    {
        int32_t value;
    };
    virtual SlangResult read(int32_t index, void* out) = 0;
    virtual void other() = 0;
};
}
";

const HELPER_RULES: RuleSet = rhi_rules(Extra {
    members: &[name_rule("rhi::IReader::other")],
    ..NO_EXTRA
});

#[test]
fn generate_slang_rhi_refuses_a_virtual_method_after_a_member_of_its_name_for_a_target_of_the_microsoft_abi()
 {
    let error = run(
        "early_helper_msvc",
        EARLY_HELPER_HEADER,
        &HELPER_RULES,
        "x86_64-pc-windows-msvc",
    )
    .expect_err("generation fails");
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &[
            "virtual methods share a name with a member that the class declares before them on target x86_64-pc-windows-msvc",
            "rhi::IReader::read(int32_t, void *)",
            "rhi::IReader::other()",
        ],
    );
    let generated = generate("early_helper_itanium", EARLY_HELPER_HEADER, &HELPER_RULES);
    assert_contains(
        &generated.bindings,
        "    pub read: unsafe extern \"system-unwind\" fn(this: *mut IReader, index: i32, out: *mut ::core::ffi::c_void) -> ::slang_compiler_sys::SlangResult,
    pub other: unsafe extern \"system-unwind\" fn(this: *mut IReader),",
    );
}

#[test]
fn generate_slang_rhi_refuses_a_virtual_method_after_a_nested_type_of_its_name_for_a_target_of_the_microsoft_abi()
 {
    let error = run(
        "early_type_msvc",
        EARLY_TYPE_HEADER,
        &rhi_rules(NO_EXTRA),
        "x86_64-pc-windows-msvc",
    )
    .expect_err("generation fails");
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &[
            "virtual methods share a name with a member that the class declares before them",
            "rhi::IReader::other()",
        ],
    );
}

#[test]
fn generate_slang_rhi_accepts_a_member_after_the_virtual_methods_of_its_name_for_a_target_of_the_microsoft_abi()
 {
    let generated = run(
        "late_helper_msvc",
        LATE_HELPER_HEADER,
        &HELPER_RULES,
        "x86_64-pc-windows-msvc",
    )
    .unwrap_or_else(|error| panic!("generation failed: {error}"));
    assert_contains(
        &generated.bindings,
        "    pub read: unsafe extern \"system-unwind\" fn(this: *mut IReader, index: i32, out: *mut ::core::ffi::c_void) -> ::slang_compiler_sys::SlangResult,
    pub other: unsafe extern \"system-unwind\" fn(this: *mut IReader),",
    );
}

#[test]
fn generate_slang_rhi_writes_the_slot_function_for_a_target_of_the_microsoft_abi() {
    let generated = run(
        "msvc_probe",
        RHI_INTERFACE,
        &rhi_rules(NO_EXTRA),
        "x86_64-pc-windows-msvc",
    )
    .unwrap_or_else(|error| panic!("generation failed: {error}"));
    let probe = generated.abi_probe.unwrap_or_default();
    assert_contains(
        &probe,
        "static_assert(sizeof(rhi::Holder) == 24, \"the size of rhi::Holder\");",
    );
    assert_contains(&probe, "slang_rhi_sys_abi_probe_slots");
    assert_contains(&generated.symbols, "VTABLE_SLOTS");
}

const MICROSOFT_ABI_SPELLINGS: [&str; 4] = [
    "x86_64-windows-msvc",
    "x86_64-pc-win32",
    "x86_64-pc-windows",
    "aarch64-windows-msvc",
];

#[test]
fn generate_slang_rhi_takes_the_microsoft_abi_from_the_target_that_libclang_resolves() {
    const RULES: RuleSet = rhi_rules(Extra {
        overloads: OVERLOADS,
        ..NO_EXTRA
    });
    for (position, target) in MICROSOFT_ABI_SPELLINGS.into_iter().enumerate() {
        let error = run(
            &format!("overloads_msvc_spelling_{position}"),
            OVERLOAD_HEADER,
            &RULES,
            target,
        )
        .expect_err("generation fails");
        assert_failure(
            &error,
            ErrorKind::Unsupported,
            &[&format!(
                "virtual methods share a name but are not declared next to each other on target {target},"
            )],
        );
        let generated = run(
            &format!("msvc_reversed_spelling_{position}"),
            ADJACENT_OVERLOAD_HEADER,
            &RULES,
            target,
        )
        .unwrap_or_else(|error| panic!("generation for {target} failed: {error}"));
        assert_contains(
            &generated.bindings,
            "    pub read_root: unsafe extern \"system-unwind\" fn(this: *mut IReader, index: i32, out: *mut *mut ::slang_compiler_sys::IRoot) -> ::slang_compiler_sys::SlangResult,
    pub read: unsafe extern",
        );
    }
}

#[test]
fn generate_slang_rhi_keeps_the_itanium_abi_for_a_windows_target_of_the_gnu_environment() {
    const RULES: RuleSet = rhi_rules(Extra {
        overloads: OVERLOADS,
        ..NO_EXTRA
    });
    let generated = run(
        "overloads_windows_gnu",
        OVERLOAD_HEADER,
        &RULES,
        "x86_64-pc-windows-gnu",
    )
    .unwrap_or_else(|error| panic!("generation failed: {error}"));
    assert_contains(&generated.bindings, "pub read_root: unsafe extern");
    let probe = generated.abi_probe.unwrap_or_default();
    assert_contains(&probe, "slang_rhi_sys_abi_probe_slots");
    assert_contains(&generated.symbols, "VTABLE_SLOTS");
}

const TASK_POOL_HEADER: &str = r"
namespace rhi
{
struct ITasks : public IRoot
{
    TEST_UUID
    virtual void* submit(void (*func)(void*), void* payload, void (*deleter)(void*), int32_t count) = 0;
};
struct Job
{
    void (*run)(void*) = nullptr;
};
}
";

#[test]
fn generate_slang_rhi_declares_a_callback_parameter_that_rust_calls_with_the_unwinding_c_abi() {
    const RULES: RuleSet = rhi_rules(Extra {
        called_callbacks: &[name_rule("rhi::ITasks::submit::func")],
        ..NO_EXTRA
    });
    let generated = generate("called_callback", TASK_POOL_HEADER, &RULES);
    assert_contains(
        &generated.bindings,
        "pub submit: unsafe extern \"system-unwind\" fn(this: *mut ITasks, func: Option<unsafe extern \"C-unwind\" fn(*mut ::core::ffi::c_void)>, payload: *mut ::core::ffi::c_void, deleter: Option<unsafe extern \"C\" fn(*mut ::core::ffi::c_void)>, count: i32) -> *mut ::core::ffi::c_void,",
    );
    assert_contains(
        &generated.bindings,
        "pub run: Option<unsafe extern \"C\" fn(*mut ::core::ffi::c_void)>,",
    );
    let probe = generated.abi_probe.unwrap_or_default();
    assert_contains(
        &probe,
        "slang_rhi_sys_slot(static_cast<void *(rhi::ITasks::*)(void (*)(void *), void *, void (*)(void *), int)>(&rhi::ITasks::submit)),",
    );
}

#[test]
fn generate_slang_rhi_refuses_a_called_callback_rule_that_names_no_callback_parameter() {
    const RULES: RuleSet = rhi_rules(Extra {
        called_callbacks: &[
            name_rule("rhi::ITasks::submit::payload"),
            name_rule("rhi::ITasks::submit::missing"),
            name_rule("rhi::Job::run"),
        ],
        ..NO_EXTRA
    });
    let error = fail("called_callback_stale", TASK_POOL_HEADER, &RULES);
    assert_failure(
        &error,
        ErrorKind::StaleRule,
        &[
            "3 rules of the rule set decide no declaration",
            "the called callback rule rhi::ITasks::submit::payload",
            "the called callback rule rhi::ITasks::submit::missing",
            "the called callback rule rhi::Job::run",
        ],
    );
}

const RECORD_HEADER: &str = r"
namespace rhi
{
struct IResource : public IRoot
{
    TEST_UUID
    virtual void touch() = 0;
};
struct Binding
{
    int32_t kind = 2;
    Slang::ComPtr<IResource> resource;
    union
    {
        int64_t offset;
        double weight;
    };
    Binding() = default;
    Binding(IResource* value) : resource() {}
    bool operator==(const Binding& other) const { return kind == other.kind; }
};
}
";

#[test]
fn generate_slang_rhi_ignores_named_members_and_maps_a_named_template_to_a_pointer() {
    const RULES: RuleSet = rhi_rules(Extra {
        members: &[
            name_rule("rhi::Binding::Binding"),
            name_rule("rhi::Binding::operator=="),
        ],
        templates: &[name_rule("Slang::ComPtr")],
        ..NO_EXTRA
    });
    let generated = generate("record", RECORD_HEADER, &RULES);
    assert_contains(
        &generated.bindings,
        "pub struct Binding {
    pub kind: i32,
    pub resource: *mut IResource,
    pub anon0: Binding_anon0,
}",
    );
    assert_contains(
        &generated.bindings,
        "impl Default for Binding {
    fn default() -> Self {
        Self {
            kind: 2,
            resource: ::core::ptr::null_mut(),
            anon0: Binding_anon0 { offset: 0 },
        }
    }
}",
    );
    let probe = generated.abi_probe.unwrap_or_default();
    for line in [
        "static_assert(sizeof(rhi::Binding) == 24, \"the size of rhi::Binding\");",
        "static_assert(offsetof(rhi::Binding, resource) == 8, \"the offset of rhi::Binding::resource\");",
        "static_assert(offsetof(rhi::Binding, offset) == 16, \"the offset of rhi::Binding::offset\");",
        "static_assert(offsetof(rhi::Binding, weight) == 16, \"the offset of rhi::Binding::weight\");",
    ] {
        assert!(probe.contains(line), "{line}\n{probe}");
    }
}

#[test]
fn generate_slang_rhi_refuses_a_member_or_a_template_without_a_rule() {
    let error = fail("record_no_member_rule", RECORD_HEADER, &NO_MEMBERS);
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &[
            "3 members of included records and interfaces are not data and not virtual methods, and no rule ignores them",
            "rhi::Binding::Binding (Constructor",
            "rhi::Binding::operator== (Method",
        ],
    );
    let error = fail("record_no_template_rule", RECORD_HEADER, &NO_TEMPLATE);
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &[
            "field rhi::Binding::resource has the type of the template Slang::ComPtr, which no rule maps",
        ],
    );
}

const PLACEHOLDER_HEADER: &str = r"
namespace rhi
{
struct Range
{
    int64_t offset;
    int64_t size;
};
struct IEncoder : public IRoot
{
    TEST_UUID
    virtual void clear(Range range, float values[4]) = 0;
    virtual Range current() = 0;
    virtual void finish() = 0;
};
}
";

#[test]
fn generate_slang_rhi_keeps_a_named_method_with_an_aggregate_by_value_as_a_placeholder_slot() {
    const RULES: RuleSet = rhi_rules(Extra {
        placeholders: &[
            name_rule("rhi::IEncoder::clear"),
            name_rule("rhi::IEncoder::current"),
        ],
        ..NO_EXTRA
    });
    let generated = generate("placeholder", PLACEHOLDER_HEADER, &RULES);
    assert_contains(
        &generated.bindings,
        "pub struct IEncoderVtbl {
    pub base: ::slang_compiler_sys::IRootVtbl,
    pub clear: *const ::core::ffi::c_void,
    pub current: *const ::core::ffi::c_void,
    pub finish: unsafe extern \"system-unwind\" fn(this: *mut IEncoder),
}",
    );
    assert!(!generated.bindings.contains("fn clear("));
    assert!(!generated.bindings.contains("fn current("));
    assert_contains(
        &generated.abi_probe.unwrap_or_default(),
        "slang_rhi_sys_slot(static_cast<void (rhi::IEncoder::*)(rhi::Range, float[4])>(&rhi::IEncoder::clear)),",
    );
}

#[test]
fn generate_slang_rhi_refuses_an_aggregate_by_value_without_a_placeholder_rule_and_a_rule_without_one()
 {
    let error = fail(
        "placeholder_no_rule",
        PLACEHOLDER_HEADER,
        &rhi_rules(NO_EXTRA),
    );
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &[
            "parameter range of method rhi::IEncoder::clear passes the aggregate rhi::Range by value",
        ],
    );
    let error = fail(
        "placeholder_wrong_rule",
        PLACEHOLDER_HEADER,
        &WRONG_PLACEHOLDER_RULES,
    );
    assert_failure(
        &error,
        ErrorKind::Unsupported,
        &[
            "a rule keeps method rhi::IEncoder::finish as a placeholder slot, but it passes no aggregate by value",
        ],
    );
}

#[test]
fn generate_slang_rhi_reproduces_scoped_enumerations_expressions_and_empty_braces() {
    let generated = generate(
        "initializers",
        r"
namespace rhi
{
enum class Kind
{
    First,
    Second = 7,
};
struct Inner
{
    uint32_t count = 3;
};
struct Desc
{
    Kind kind = Kind::Second;
    uint64_t pageSize = 16 * 1024 * 1024;
    uint32_t slot = uint32_t(-1);
    Inner inner = {};
    float scale = 1.5f;
};
}
",
        &rhi_rules(NO_EXTRA),
    );
    assert_contains(
        &generated.bindings,
        "impl Default for Desc {
    fn default() -> Self {
        Self {
            kind: Kind(7),
            pageSize: 16_777_216,
            slot: 4_294_967_295,
            inner: Inner::default(),
            scale: 1.5,
        }
    }
}",
    );
    let probe = generated.abi_probe.unwrap_or_default();
    assert_contains(
        &probe,
        "static_assert(sizeof(rhi::Kind) == 4, \"the size of the enumeration rhi::Kind\");",
    );
    assert_contains(
        &probe,
        "static_assert(std::is_signed_v<std::underlying_type_t<rhi::Kind>> == true, \"the signedness of the enumeration rhi::Kind\");",
    );
}

#[test]
fn generate_slang_rhi_refuses_an_initializer_that_is_a_nested_list_in_braces() {
    let error = fail(
        "nested_initializer",
        r"
namespace rhi
{
union Colour
{
    float values[4];
    int32_t integers[4];
};
struct Clear
{
    Colour colour = {{0.0f, 0.0f, 0.0f, 0.0f}};
};
}
",
        &rhi_rules(NO_EXTRA),
    );
    assert_failure(
        &error,
        ErrorKind::Evaluation,
        &["cannot evaluate the default member initializer of rhi::Clear::colour"],
    );
}

const WITHOUT_DEFAULT_HEADER: &str = r"
namespace rhi
{
void* const kInvalidStream = reinterpret_cast<void*>(~uintptr_t{0});
struct Extent
{
    uint32_t width;
    uint32_t height;
};
struct Clear
{
    Extent extent = {1, 2};
    void* stream = kInvalidStream;
    uint32_t count = 4;
    uint32_t plain;
};
struct Holder
{
    uint32_t flags = 1;
    Clear clears[2];
};
struct Outer
{
    Holder holder;
};
struct Unrelated
{
    uint32_t value = 5;
};
}
";

const fn without_default_rules(without_default: &'static [NameRule]) -> RuleSet {
    rhi_rules(Extra {
        without_default,
        ..NO_EXTRA
    })
}

const CLEAR_WITHOUT_DEFAULT: RuleSet = without_default_rules(&[name_rule("rhi::Clear")]);

#[test]
fn generate_slang_rhi_emits_a_struct_that_a_rule_names_without_default_and_with_its_layout() {
    let generated = generate(
        "without_default",
        WITHOUT_DEFAULT_HEADER,
        &CLEAR_WITHOUT_DEFAULT,
    );
    let text = &generated.bindings;
    assert_contains(
        text,
        "#[doc = \"This struct has no `Default` implementation, because a test rule, so the bindings cannot reproduce its C++ default. A caller states every field, and the documentation of each field gives its C++ default member initializer.\"]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Clear {
    #[doc = \"The C++ default member initializer of this field is `= {1, 2}`.\"]
    pub extent: Extent,
    #[doc = \"The C++ default member initializer of this field is `= kInvalidStream`.\"]
    pub stream: *mut ::core::ffi::c_void,
    #[doc = \"The C++ default member initializer of this field is `= 4`.\"]
    pub count: u32,
    #[doc = \"The C++ declaration of this field has no default member initializer.\"]
    pub plain: u32,
}",
    );
    assert_contains(text, "assert!(size_of::<Clear>() == 24);");
    assert_contains(
        text,
        "assert!(::core::mem::offset_of!(Clear, stream) == 8);",
    );
    assert!(!text.contains("impl Default for Clear"), "{text}");
    assert_contains(
        text,
        "impl Default for Unrelated { fn default() -> Self { Self { value: 5, } } }",
    );
    let probe = generated.abi_probe.unwrap_or_default();
    assert_contains(
        &probe,
        "static_assert(offsetof(rhi::Clear, stream) == 8, \"the offset of rhi::Clear::stream\");",
    );
    assert_contains(
        &probe,
        "static_assert(sizeof(rhi::Outer) == 56, \"the size of rhi::Outer\");",
    );
}

#[test]
fn generate_slang_rhi_leaves_out_the_default_of_every_struct_that_holds_one_without_default() {
    let generated = generate(
        "without_default_holder",
        WITHOUT_DEFAULT_HEADER,
        &CLEAR_WITHOUT_DEFAULT,
    );
    let text = &generated.bindings;
    for name in ["Holder", "Outer"] {
        assert!(
            !text.contains(&format!("impl Default for {name}")),
            "{name}\n{text}"
        );
    }
    assert_contains(
        text,
        "#[doc = \"This struct has no `Default` implementation, because its field `clears` holds `Clear` by value, which has none. A caller states every field, and the documentation of each field gives its C++ default member initializer.\"]
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Holder {
    #[doc = \"The C++ default member initializer of this field is `= 1`.\"]
    pub flags: u32,
    #[doc = \"The C++ declaration of this field has no default member initializer.\"]
    pub clears: [Clear; 2],
}",
    );
    assert_contains(
        text,
        "#[doc = \"This struct has no `Default` implementation, because its field `holder` holds `Holder` by value, which has none.",
    );
}

#[test]
fn generate_slang_rhi_still_refuses_an_initializer_that_it_cannot_reproduce_in_a_struct_that_no_rule_names()
 {
    let error = fail(
        "without_default_unnamed",
        WITHOUT_DEFAULT_HEADER,
        &rhi_rules(NO_EXTRA),
    );
    assert_failure(
        &error,
        ErrorKind::Evaluation,
        &["cannot evaluate the default member initializer of rhi::Clear::extent"],
    );
}

#[test]
fn generate_slang_rhi_refuses_a_rule_without_default_that_names_no_struct_or_one_that_does_not_need_it()
 {
    const RULES: RuleSet = without_default_rules(&[
        name_rule("rhi::Clear"),
        name_rule("rhi::Missing"),
        name_rule("rhi::Unrelated"),
        name_rule("rhi::Extent"),
    ]);
    let error = fail("without_default_stale", WITHOUT_DEFAULT_HEADER, &RULES);
    assert_failure(
        &error,
        ErrorKind::StaleRule,
        &[
            "3 rules of the rule set decide no declaration",
            "the rule without Default rhi::Missing (a test rule), which names no included struct",
            "the rule without Default rhi::Unrelated (a test rule), whose struct rhi::Unrelated has a default that the bindings reproduce",
            "the rule without Default rhi::Extent (a test rule), whose struct rhi::Extent has a default",
        ],
    );
    assert!(!error.message().contains("rhi::Clear "), "{error}");
}

#[test]
fn generate_slang_rhi_emits_a_pointer_constant_made_by_a_cast_of_an_unsigned_integer_of_pointer_size()
 {
    let generated = generate(
        "pointer_constant",
        WITHOUT_DEFAULT_HEADER,
        &CLEAR_WITHOUT_DEFAULT,
    );
    assert_contains(
        &generated.bindings,
        "pub const kInvalidStream: *mut ::core::ffi::c_void = ::core::ptr::without_provenance_mut(18_446_744_073_709_551_615_usize);",
    );
    let generated = generate(
        "pointer_constant_c_cast",
        "namespace rhi\n{\nconst void* const kSentinel = (const void*)uintptr_t(16);\n}\n",
        &rhi_rules(NO_EXTRA),
    );
    assert_contains(
        &generated.bindings,
        "pub const kSentinel: *const ::core::ffi::c_void = ::core::ptr::without_provenance(16_usize);",
    );
}

const POINTER_DEFAULT_HEADER: &str = r"
namespace rhi
{
void* const kInvalidStream = reinterpret_cast<void*>(~uintptr_t{0});
struct Submit
{
    uint32_t count = 3;
    void* stream = kInvalidStream;
};
}
";

#[test]
fn generate_slang_rhi_defaults_a_pointer_field_to_the_pointer_constant_that_its_initializer_names()
{
    const SUBMIT_WITHOUT_DEFAULT: RuleSet = without_default_rules(&[name_rule("rhi::Submit")]);
    let generated = generate(
        "pointer_default",
        POINTER_DEFAULT_HEADER,
        &rhi_rules(NO_EXTRA),
    );
    assert_contains(
        &generated.bindings,
        "impl Default for Submit { fn default() -> Self { Self { count: 3, stream: kInvalidStream, } } }",
    );
    let error = fail(
        "pointer_default_stale",
        POINTER_DEFAULT_HEADER,
        &SUBMIT_WITHOUT_DEFAULT,
    );
    assert_failure(
        &error,
        ErrorKind::StaleRule,
        &["whose struct rhi::Submit has a default that the bindings reproduce"],
    );
}

#[test]
fn generate_slang_rhi_refuses_a_pointer_field_whose_initializer_converts_the_pointer_constant() {
    let error = fail(
        "pointer_default_converted",
        "namespace rhi\n{\nvoid* const kInvalidStream = reinterpret_cast<void*>(~uintptr_t{0});\nstruct Submit\n{\n    const void* stream = kInvalidStream;\n};\n}\n",
        &rhi_rules(NO_EXTRA),
    );
    assert_failure(
        &error,
        ErrorKind::Evaluation,
        &[
            "the default member initializer of field rhi::Submit::stream names rhi::kInvalidStream, which the bindings do not declare as a constant of the type of the field",
        ],
    );
}

#[test]
fn generate_slang_rhi_refuses_a_pointer_constant_whose_address_it_cannot_read_exactly() {
    for (test, header) in [
        (
            "pointer_constant_signed",
            "namespace rhi\n{\nvoid* const kSigned = reinterpret_cast<void*>(intptr_t(-1));\n}\n",
        ),
        (
            "pointer_constant_narrow",
            "namespace rhi\n{\nvoid* const kNarrow = reinterpret_cast<void*>(uint32_t(7));\n}\n",
        ),
        (
            "pointer_constant_string",
            "namespace rhi\n{\nconst char* const kName = \"name\";\n}\n",
        ),
    ] {
        let error = fail(test, header, &rhi_rules(NO_EXTRA));
        assert_failure(
            &error,
            ErrorKind::Evaluation,
            &["cannot evaluate the constant rhi::k"],
        );
    }
}

const SHIM_DECLARATIONS: &[DeclRule] = &[
    RHI_DECLARATIONS[0],
    DeclRule {
        header: Some("rhi.h"),
        pattern: "rhi::same",
        action: Action::Exclude,
        reason: "a test inline helper",
    },
    RHI_DECLARATIONS[1],
];

#[test]
fn generate_slang_rhi_parses_through_the_cxx_shim_headers() {
    const RULES: RuleSet = RuleSet {
        declarations: SHIM_DECLARATIONS,
        ..rhi_rules(NO_EXTRA)
    };
    let generated = generate(
        "shims",
        r"
#include <algorithm>
#include <atomic>
#include <assert.h>
#include <cstddef>
#include <cstdint>
#include <iterator>
namespace rhi
{
inline bool same(const int (&left)[2], const int (&right)[2], std::nullptr_t, std::size_t, std::uint32_t)
{
    assert(true);
    return std::equal(left, left + std::size(left), right);
}
struct Sizes
{
    size_t size;
    uint32_t count;
};
}
",
        &RULES,
    );
    assert_contains(&generated.bindings, "pub size: usize,");
    assert_contains(&generated.bindings, "pub count: u32,");
    assert!(!generated.bindings.contains("same"));
    assert!(!generated.bindings.contains("equal"));
}

#[test]
fn generate_slang_rhi_lists_the_probe_function_and_every_slot_in_the_symbols() {
    let generated = generate("symbols", RHI_INTERFACE, &rhi_rules(NO_EXTRA));
    assert_eq!(
        compact(&generated.symbols),
        compact(
            "pub(super) const C_FUNCTIONS: &[(&str, *const ())] = &[];
unsafe extern \"C\" {
    pub(super) fn slang_rhi_sys_abi_probe_slots(out: *mut usize, capacity: usize) -> usize;
}
pub(super) const VTABLE_SLOTS: &[(&str, usize)] = &[
    (\"IThing::count\", ::core::mem::offset_of!(slang_rhi_sys::IThingVtbl, count)),
];"
        )
    );
    let probe = generated.abi_probe.unwrap_or_default();
    assert!(probe.starts_with("#include \"rhi.h\"\n"), "{probe}");
    assert_contains(
        &probe,
        "static_assert(offsetof(rhi::Holder, root) == 16, \"the offset of rhi::Holder::root\");",
    );
    assert_contains(
        &probe,
        "extern \"C\" std::size_t slang_rhi_sys_abi_probe_slots(std::size_t* out, std::size_t capacity)",
    );
    assert_contains(
        &probe,
        "slang_rhi_sys_slot(static_cast<long (rhi::IThing::*)()>(&rhi::IThing::count)),",
    );
}

#[test]
fn generate_slang_rhi_fails_on_rules_of_the_new_kinds_that_decide_nothing() {
    const RULES: RuleSet = rhi_rules(Extra {
        members: &[name_rule("rhi::Nothing::member")],
        templates: &[name_rule("Nothing::Template")],
        uuidless: &[name_rule("rhi::INothing")],
        overloads: &[OverloadRule {
            method: "rhi::INothing::read",
            params: &[],
            rust: "read",
            reason: "a test overload",
        }],
        placeholders: &[name_rule("rhi::INothing::clear")],
        called_callbacks: &[name_rule("rhi::IThing::count::func")],
        without_default: &[],
    });
    let error = fail("stale", RHI_INTERFACE, &RULES);
    assert_failure(
        &error,
        ErrorKind::StaleRule,
        &[
            "6 rules of the rule set decide no declaration",
            "the called callback rule rhi::IThing::count::func",
            "the member rule rhi::Nothing::member",
            "the template rule Nothing::Template",
            "the interface rule rhi::INothing",
            "the placeholder rule rhi::INothing::clear",
            "the overload rule rhi::INothing::read()",
        ],
    );
}

#[test]
fn generate_slang_rhi_fails_without_the_config_or_the_slang_headers() {
    let _guard = LIBCLANG.lock().unwrap_or_else(PoisonError::into_inner);
    let dirs = Dirs::new("missing_dirs", "");
    let missing = dirs.root.join("missing");
    for (index, what) in [
        (1, "the directory of the configured header"),
        (2, "the header directory"),
    ] {
        let mut paths = [
            dirs.root.join("rhi"),
            dirs.root.join("config"),
            dirs.root.join("slang"),
        ];
        paths[index] = missing.clone();
        let error = generate_with_external(
            &TEST_SLANG,
            &rhi_rules(NO_EXTRA),
            [&paths[0], &paths[1], &paths[2]],
            TARGET,
        )
        .expect_err("generation fails");
        assert_eq!(error.kind(), ErrorKind::Io, "{error}");
        assert!(error.message().contains(what), "{error}");
    }
}
