use crate::model::{Enumerator, Prim};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuleSet {
    pub(crate) main_header: &'static str,
    pub(crate) declarations: &'static [DeclRule],
    pub(crate) macros: &'static [MacroRule],
    pub(crate) unnamed_enums: &'static [UnnamedEnumRule],
    pub(crate) result_type: &'static str,
    pub(crate) uuid_type: &'static str,
    pub(crate) pointer_sized: &'static [(&'static str, Prim)],
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct DeclRule {
    pub(crate) header: Option<&'static str>,
    pub(crate) pattern: &'static str,
    pub(crate) action: Action,
    pub(crate) reason: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Action {
    Include,
    Exclude,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MacroRule {
    pub(crate) pattern: &'static str,
    pub(crate) action: MacroAction,
    pub(crate) reason: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct UnnamedEnumRule {
    pub(crate) header: &'static str,
    pub(crate) enumerator: &'static str,
    pub(crate) typedef: &'static str,
    pub(crate) reason: &'static str,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MacroAction {
    ResultCode,
    Constant(Prim),
    Exclude,
}

impl RuleSet {
    pub(crate) fn declaration_position(&self, header: &str, name: &str) -> Option<usize> {
        self.declarations.iter().position(|rule| {
            rule.header.is_none_or(|wanted| wanted == header) && matches(rule.pattern, name)
        })
    }

    pub(crate) fn declaration_rule(&self, header: &str, name: &str) -> Option<&DeclRule> {
        self.declarations
            .get(self.declaration_position(header, name)?)
    }

    pub(crate) fn macro_position(&self, name: &str) -> Option<usize> {
        self.macros
            .iter()
            .position(|rule| matches(rule.pattern, name))
    }

    pub(crate) fn unnamed_enum_position(
        &self,
        header: &str,
        enumerators: &[Enumerator],
    ) -> Option<usize> {
        self.unnamed_enums.iter().position(|rule| {
            rule.header == header
                && enumerators
                    .iter()
                    .any(|enumerator| enumerator.name == rule.enumerator)
        })
    }

    pub(crate) fn pointer_sized(&self, name: &str) -> Option<Prim> {
        self.pointer_sized
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, prim)| *prim)
    }
}

pub(crate) fn matches(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((prefix, rest)) => {
            let Some(tail) = name.strip_prefix(prefix) else {
                return false;
            };
            if rest.is_empty() {
                return true;
            }
            tail.char_indices()
                .map(|(index, _)| index)
                .chain(std::iter::once(tail.len()))
                .any(|index| matches(rest, &tail[index..]))
        }
    }
}

const fn include(header: &'static str, pattern: &'static str, reason: &'static str) -> DeclRule {
    DeclRule {
        header: Some(header),
        pattern,
        action: Action::Include,
        reason,
    }
}

const fn exclude(header: &'static str, pattern: &'static str, reason: &'static str) -> DeclRule {
    DeclRule {
        header: Some(header),
        pattern,
        action: Action::Exclude,
        reason,
    }
}

const fn unnamed_enum(
    enumerator: &'static str,
    typedef: &'static str,
    reason: &'static str,
) -> UnnamedEnumRule {
    UnnamedEnumRule {
        header: "slang.h",
        enumerator,
        typedef,
        reason,
    }
}

const fn macro_rule(pattern: &'static str, action: MacroAction, reason: &'static str) -> MacroRule {
    MacroRule {
        pattern,
        action,
        reason,
    }
}

const REFLECTION_WRAPPER: &str =
    "an inline C++ wrapper over the spReflection functions, which is not part of the ABI (SCS-3)";
const PLATFORM_MACRO: &str =
    "a platform or compiler detection macro, whose value depends on the target (SCS-8)";
const ATTRIBUTE_MACRO: &str =
    "a macro that expands to an attribute, a keyword, or nothing, and has no value";
const UUID_MACRO: &str = "a UUID, which the generated Interface trait carries (COM-4)";
const FLAG_VALUES: &str =
    "the flag values of the typedef that the header declares directly before the enumeration";

pub(crate) const SLANG: RuleSet = RuleSet {
    main_header: "slang.h",
    declarations: &[
        exclude(
            "slang-deprecated.h",
            "spReflection_GetSession",
            "exported only under a mangled C++ name (SCS-3)",
        ),
        include(
            "slang-deprecated.h",
            "spReflection*",
            "the reflection ABI, which has no COM-style interface (SCS-1)",
        ),
        exclude(
            "slang-deprecated.h",
            "*",
            "the deprecated compile-request API, which the modern interfaces replace (SCS-1)",
        ),
        exclude(
            "slang.h",
            "slang_getEmbeddedCoreModule",
            "exported only under a mangled C++ name (SCS-3)",
        ),
        exclude(
            "slang.h",
            "slang::createGlobalSession",
            "an inline C++ helper, which is not part of the ABI (SCS-3)",
        ),
        exclude(
            "slang.h",
            "slang::shutdown",
            "an inline C++ helper, which is not part of the ABI (SCS-3)",
        ),
        exclude(
            "slang.h",
            "slang::getLastInternalErrorMessage",
            "an inline C++ helper, which is not part of the ABI (SCS-3)",
        ),
        exclude("slang.h", "slang::Attribute", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::UserAttribute", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::TypeReflection", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::TypeReflection::*", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::TypeLayoutReflection", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::Modifier", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::Modifier::*", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::VariableReflection", REFLECTION_WRAPPER),
        exclude(
            "slang.h",
            "slang::VariableLayoutReflection",
            REFLECTION_WRAPPER,
        ),
        exclude("slang.h", "slang::FunctionReflection", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::GenericReflection", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::EntryPointReflection", REFLECTION_WRAPPER),
        exclude(
            "slang.h",
            "slang::TypeParameterReflection",
            REFLECTION_WRAPPER,
        ),
        exclude("slang.h", "slang::ShaderReflection", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::DeclReflection", REFLECTION_WRAPPER),
        exclude("slang.h", "slang::DeclReflection::*", REFLECTION_WRAPPER),
        exclude(
            "slang.h",
            "slang::CompileCoreModuleFlag",
            "an empty struct that only scopes the enumeration CompileCoreModuleFlag::Enum, which is included",
        ),
        exclude(
            "slang.h",
            "*(StaticAssert at line *)",
            "a compile-time assertion of the header, which declares nothing",
        ),
        exclude(
            "slang.h",
            "slang::VMExecOperand",
            "a struct with bit-fields, which a Rust struct cannot express, used only by inline C++ helpers",
        ),
        include(
            "slang.h",
            "*",
            "the main header, whose declarations the bindings include in full (SCS-1)",
        ),
    ],
    macros: &[
        macro_rule("SLANG_OK", MacroAction::ResultCode, "a result code (SCS-8)"),
        macro_rule(
            "SLANG_FAIL",
            MacroAction::ResultCode,
            "a result code (SCS-8)",
        ),
        macro_rule(
            "SLANG_E_*",
            MacroAction::ResultCode,
            "a result code (SCS-8)",
        ),
        macro_rule(
            "SLANG_ERROR_*",
            MacroAction::ResultCode,
            "an alias of a result code (SCS-8)",
        ),
        macro_rule(
            "SLANG_FACILITY_*",
            MacroAction::Constant(Prim::I32),
            "a facility of a result code, an int in the header (SCS-8)",
        ),
        macro_rule(
            "SLANG_API_VERSION",
            MacroAction::Constant(Prim::U32),
            "the API version, typed like the field SlangGlobalSessionDesc::apiVersion that takes it (SCS-8)",
        ),
        macro_rule(
            "SLANG_UNBOUNDED_SIZE",
            MacroAction::Constant(Prim::Usize),
            "a size sentinel (SCS-8)",
        ),
        macro_rule(
            "SLANG_UNKNOWN_SIZE",
            MacroAction::Constant(Prim::Usize),
            "a size sentinel (SCS-8)",
        ),
        macro_rule("SLANG_UUID_*", MacroAction::Exclude, UUID_MACRO),
        macro_rule(
            "SLANG_H",
            MacroAction::Exclude,
            "the include guard of slang.h",
        ),
        macro_rule("SLANG_COMPILER", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_PLATFORM", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_CLANG", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_VC", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_SNC", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_GHS", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_GCC", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_LINUX", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_WINRT", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_XBOXONE", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_WIN64", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_X360", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_WIN32", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_ANDROID", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_IOS", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_OSX", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_PS3", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_PS4", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_PSP2", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_WIIU", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_WASM", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_*_FAMILY", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_ENABLE_*", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_HAS_*", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_PROCESSOR_*", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_PTR_IS_*", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_LITTLE_ENDIAN", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule("SLANG_BIG_ENDIAN", MacroAction::Exclude, PLATFORM_MACRO),
        macro_rule(
            "SLANG_UNALIGNED_ACCESS",
            MacroAction::Exclude,
            PLATFORM_MACRO,
        ),
        macro_rule("SLANG_NO_THROW", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_STDCALL", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_MCALL", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_DYNAMIC", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_DLL_EXPORT", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_API", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_NO_INLINE", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_FORCE_INLINE", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_INLINE", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_MAYBE_UNUSED", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_EXTERN_C", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_OVERRIDE", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule("SLANG_DEPRECATED", MacroAction::Exclude, ATTRIBUTE_MACRO),
        macro_rule(
            "SLANG_FORMAT",
            MacroAction::Exclude,
            "a function-like helper that writes the enumerators of SlangImageFormat, which libclang reports as object-like",
        ),
        macro_rule(
            "SLANG_RESOURCE_SHAPE",
            MacroAction::Exclude,
            "a helper that expands to nothing inside the declaration of SlangResourceShape",
        ),
    ],
    unnamed_enums: &[
        unnamed_enum(
            "SLANG_DIAGNOSTIC_FLAG_VERBOSE_PATHS",
            "SlangDiagnosticFlags",
            FLAG_VALUES,
        ),
        unnamed_enum(
            "SLANG_COMPILE_FLAG_NO_MANGLING",
            "SlangCompileFlags",
            FLAG_VALUES,
        ),
        unnamed_enum(
            "SLANG_TARGET_FLAG_GENERATE_SPIRV_DIRECTLY",
            "SlangTargetFlags",
            FLAG_VALUES,
        ),
        unnamed_enum("kSessionFlags_None", "slang::SessionFlags", FLAG_VALUES),
    ],
    result_type: "SlangResult",
    uuid_type: "SlangUUID",
    pointer_sized: &[
        ("SlangInt", Prim::Isize),
        ("SlangSSizeT", Prim::Isize),
        ("SlangUInt", Prim::Usize),
        ("SlangSizeT", Prim::Usize),
    ],
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_accepts_exact_names_only_without_a_wildcard() {
        assert!(matches("SLANG_OK", "SLANG_OK"));
        assert!(!matches("SLANG_OK", "SLANG_OK2"));
        assert!(!matches("SLANG_OK", "XSLANG_OK"));
    }

    #[test]
    fn matches_handles_prefix_suffix_and_inner_wildcards() {
        assert!(matches("spReflection*", "spReflectionType_GetKind"));
        assert!(matches("spReflection*", "spReflection"));
        assert!(!matches("spReflection*", "spGetBuildTagString"));
        assert!(matches("SLANG_*_FAMILY", "SLANG_UNIX_FAMILY"));
        assert!(!matches("SLANG_*_FAMILY", "SLANG_UNIX_FAMILY_X"));
        assert!(matches("*", "slang::TypeReflection::Kind"));
        assert!(matches(
            "slang::TypeReflection::*",
            "slang::TypeReflection::Kind"
        ));
        assert!(!matches(
            "slang::TypeReflection::*",
            "slang::TypeReflection"
        ));
    }

    fn action(header: &str, name: &str) -> Option<Action> {
        SLANG.declaration_rule(header, name).map(|rule| rule.action)
    }

    fn macro_action(name: &str) -> Option<MacroAction> {
        SLANG
            .macro_position(name)
            .map(|position| SLANG.macros[position].action)
    }

    #[test]
    fn declaration_rule_is_the_first_matching_rule_for_the_header() {
        assert_eq!(
            action("slang-deprecated.h", "spReflectionType_GetKind"),
            Some(Action::Include)
        );
        assert_eq!(
            action("slang-deprecated.h", "spCompile"),
            Some(Action::Exclude)
        );
        assert_eq!(
            action("slang.h", "slang::TypeReflection"),
            Some(Action::Exclude)
        );
        assert_eq!(
            action("slang.h", "slang::IGlobalSession"),
            Some(Action::Include)
        );
        assert_eq!(action("slang-gfx.h", "gfx::IDevice"), None);
    }

    #[test]
    fn macro_rule_types_result_codes_and_sentinels() {
        assert_eq!(
            macro_action("SLANG_E_NOT_FOUND"),
            Some(MacroAction::ResultCode)
        );
        assert_eq!(
            macro_action("SLANG_UNBOUNDED_SIZE"),
            Some(MacroAction::Constant(Prim::Usize))
        );
        assert_eq!(macro_action("SLANG_LINUX"), Some(MacroAction::Exclude));
        assert_eq!(macro_action("SLANG_SOMETHING_NEW"), None);
    }

    fn enumerators(names: &[&str]) -> Vec<Enumerator> {
        names
            .iter()
            .map(|name| Enumerator {
                name: (*name).to_owned(),
                value: 0,
            })
            .collect()
    }

    #[test]
    fn unnamed_enum_position_matches_any_enumerator_in_the_header() {
        let session = enumerators(&["kSessionFlags_Other", "kSessionFlags_None"]);
        let position = SLANG
            .unnamed_enum_position("slang.h", &session)
            .expect("a rule assigns the session flags");
        assert_eq!(SLANG.unnamed_enums[position].typedef, "slang::SessionFlags");
        assert_eq!(SLANG.unnamed_enum_position("slang-gfx.h", &session), None);
        assert_eq!(
            SLANG.unnamed_enum_position("slang.h", &enumerators(&["kUnknown"])),
            None
        );
    }
}
