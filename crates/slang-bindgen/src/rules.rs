use crate::model::{Enumerator, Prim};

#[derive(Clone, Copy, Debug)]
pub(crate) struct RuleSet {
    pub(crate) main_header: &'static str,
    pub(crate) language: &'static str,
    pub(crate) defines: &'static [&'static str],
    pub(crate) library: &'static str,
    pub(crate) own_crate: &'static str,
    pub(crate) external_crate: Option<&'static str>,
    pub(crate) declarations: &'static [DeclRule],
    pub(crate) macros: &'static [MacroRule],
    pub(crate) unnamed_enums: &'static [UnnamedEnumRule],
    pub(crate) result_type: &'static str,
    pub(crate) uuid_type: &'static str,
    pub(crate) pointer_sized: &'static [(&'static str, Prim)],
    pub(crate) members: Option<&'static [NameRule]>,
    pub(crate) templates: &'static [NameRule],
    pub(crate) uuidless: &'static [NameRule],
    pub(crate) overloads: &'static [OverloadRule],
    pub(crate) placeholders: &'static [NameRule],
    pub(crate) called_callbacks: &'static [NameRule],
    pub(crate) without_default: &'static [NameRule],
    pub(crate) abi_probe: bool,
    pub(crate) initializers_in_place: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct NameRule {
    pub(crate) pattern: &'static str,
    pub(crate) reason: &'static str,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct OverloadRule {
    pub(crate) method: &'static str,
    pub(crate) params: &'static [&'static str],
    pub(crate) rust: &'static str,
    pub(crate) reason: &'static str,
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
    External,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct MacroRule {
    pub(crate) header: Option<&'static str>,
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

    pub(crate) fn macro_position(&self, header: &str, name: &str) -> Option<usize> {
        self.macros.iter().position(|rule| {
            rule.header.is_none_or(|wanted| wanted == header) && matches(rule.pattern, name)
        })
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
        header: None,
        pattern,
        action,
        reason,
    }
}

const REFLECTION_WRAPPER: &str =
    "an inline C++ wrapper over the spReflection functions, which is not part of the ABI";
const PLATFORM_MACRO: &str =
    "a platform or compiler detection macro, whose value depends on the target";
const ATTRIBUTE_MACRO: &str =
    "a macro that expands to an attribute, a keyword, or nothing, and has no value";
const UUID_MACRO: &str = "a UUID, which the generated Interface trait carries";
const FLAG_VALUES: &str =
    "the flag values of the typedef that the header declares directly before the enumeration";

pub(crate) const SLANG: RuleSet = RuleSet {
    main_header: "slang.h",
    language: "c++17",
    defines: &[],
    library: "Slang",
    own_crate: "slang_compiler_sys",
    external_crate: None,
    declarations: &[
        exclude(
            "slang-deprecated.h",
            "spReflection_GetSession",
            "exported only under a mangled C++ name",
        ),
        include(
            "slang-deprecated.h",
            "spReflection*",
            "the reflection ABI, which has no COM-style interface",
        ),
        exclude(
            "slang-deprecated.h",
            "*",
            "the deprecated compile-request API, which the modern interfaces replace",
        ),
        exclude(
            "slang.h",
            "slang_getEmbeddedCoreModule",
            "exported only under a mangled C++ name",
        ),
        exclude(
            "slang.h",
            "slang::createGlobalSession",
            "an inline C++ helper, which is not part of the ABI",
        ),
        exclude(
            "slang.h",
            "slang::shutdown",
            "an inline C++ helper, which is not part of the ABI",
        ),
        exclude(
            "slang.h",
            "slang::getLastInternalErrorMessage",
            "an inline C++ helper, which is not part of the ABI",
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
            "*(StaticAssert at slang.h:*)",
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
            "the main header, whose declarations the bindings include in full",
        ),
    ],
    macros: &[
        macro_rule("SLANG_OK", MacroAction::ResultCode, "a result code"),
        macro_rule("SLANG_FAIL", MacroAction::ResultCode, "a result code"),
        macro_rule("SLANG_E_*", MacroAction::ResultCode, "a result code"),
        macro_rule(
            "SLANG_ERROR_*",
            MacroAction::ResultCode,
            "an alias of a result code",
        ),
        macro_rule(
            "SLANG_FACILITY_*",
            MacroAction::Constant(Prim::I32),
            "a facility of a result code, an int in the header",
        ),
        macro_rule(
            "SLANG_API_VERSION",
            MacroAction::Constant(Prim::U32),
            "the API version, typed like the field SlangGlobalSessionDesc::apiVersion that takes it",
        ),
        macro_rule(
            "SLANG_UNBOUNDED_SIZE",
            MacroAction::Constant(Prim::Usize),
            "a size sentinel",
        ),
        macro_rule(
            "SLANG_UNKNOWN_SIZE",
            MacroAction::Constant(Prim::Usize),
            "a size sentinel",
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
    members: None,
    templates: &[],
    uuidless: &[],
    overloads: &[],
    placeholders: &[],
    called_callbacks: &[],
    without_default: &[],
    abi_probe: false,
    initializers_in_place: false,
};

const SLANG_HEADER: &str = "a declaration of the Slang headers, which slang-compiler-sys declares";

const fn external(header: &'static str) -> DeclRule {
    DeclRule {
        header: Some(header),
        pattern: "*",
        action: Action::External,
        reason: SLANG_HEADER,
    }
}

const fn slang_macros(header: &'static str) -> MacroRule {
    MacroRule {
        header: Some(header),
        pattern: "*",
        action: MacroAction::Exclude,
        reason: "a macro of the Slang headers, whose constants slang-compiler-sys declares",
    }
}

const fn rhi_macro(header: &'static str, pattern: &'static str, reason: &'static str) -> MacroRule {
    MacroRule {
        header: Some(header),
        pattern,
        action: MacroAction::Exclude,
        reason,
    }
}

const USING_DECLARATION: &str = "a using declaration, which declares no new type or function";
const STATIC_ASSERT: &str = "a compile-time assertion of the header, which declares nothing";
const ENUM_OPERATOR: &str =
    "an inline operator or helper of a flags enumeration, which is not part of the ABI";
const INLINE_FUNCTION: &str = "an inline C++ helper, which is not part of the ABI";
const BIT_FIELDS: &str = "a struct with bit-fields, which a Rust struct cannot express";
const HOLDS_BIT_FIELDS: &str =
    "a struct that holds a struct with bit-fields by value, which a Rust struct cannot express";
const STRUCT_CONSTANT: &str = "a constant of a struct type, whose value no rule evaluates";
const COM_PTR_HELPER: &str =
    "a class that holds a Slang::ComPtr and serves only inline helpers or values passed by value";
const NESTED_INITIALIZER: &str = "the default member initializer of a field is a nested list in braces, which libclang does not evaluate to one value";
const BRACED_INITIALIZER: &str = "the default member initializer of a field is a list in braces, which libclang does not evaluate to one value";
const STRUCT_INITIALIZER: &str = "the default member initializer of a field names a constant of a struct type that is not constexpr, which libclang does not evaluate";
const fn without_default(pattern: &'static str, reason: &'static str) -> NameRule {
    NameRule { pattern, reason }
}

const INTERFACE_HELPER: &str = "an inline C++ helper of an interface, which is not a virtual method and so not part of its vtable";
const RECORD_MEMBER: &str =
    "a constructor, operator, or member function of a record, which is not part of its data layout";
const STATIC_MEMBER: &str =
    "a static data member, which is not part of the data layout of its record";

const PLACEHOLDER: &str = "a virtual method that takes or returns an aggregate by value, whose calling convention differs between targets, so the vtable keeps its slot without a declaration";

const fn placeholder(pattern: &'static str) -> NameRule {
    NameRule {
        pattern,
        reason: PLACEHOLDER,
    }
}

const CALLED_CALLBACK: &str = "a function of slang-rhi that a task pool which a program implements receives and calls, so a C++ exception from it unwinds into the caller in Rust";

const fn called_callback(pattern: &'static str) -> NameRule {
    NameRule {
        pattern,
        reason: CALLED_CALLBACK,
    }
}

const FIRST_OVERLOAD: &str = "the first declared of virtual methods that share a name, which keeps the C++ name and its slot in declaration order on the Itanium C++ ABI";
const LATER_OVERLOAD: &str = "a later declared of virtual methods that share a name, which gets a suffix and its slot in declaration order on the Itanium C++ ABI";

const fn overload(
    method: &'static str,
    params: &'static [&'static str],
    rust: &'static str,
    reason: &'static str,
) -> OverloadRule {
    OverloadRule {
        method,
        params,
        rust,
        reason,
    }
}

const fn member(pattern: &'static str, reason: &'static str) -> NameRule {
    NameRule { pattern, reason }
}

const SLANG_RHI_MEMBERS: &[NameRule] = &[
    member("rhi::AdapterLUID::operator!=", RECORD_MEMBER),
    member("rhi::AdapterLUID::operator==", RECORD_MEMBER),
    member("rhi::Binding::Binding", RECORD_MEMBER),
    member("rhi::BufferOffsetPair::BufferOffsetPair", RECORD_MEMBER),
    member("rhi::BufferOffsetPair::operator bool", RECORD_MEMBER),
    member("rhi::BufferOffsetPair::operator==", RECORD_MEMBER),
    member("rhi::BufferOffsetPair::operator!=", RECORD_MEMBER),
    member("rhi::BufferOffsetPair::getDeviceAddress", RECORD_MEMBER),
    member("rhi::BufferOffsetPair::getSize", RECORD_MEMBER),
    member("rhi::BufferRange::operator!=", RECORD_MEMBER),
    member("rhi::BufferRange::operator==", RECORD_MEMBER),
    member(
        "rhi::DebugLayerOptions::isDebugLayersEnabled",
        RECORD_MEMBER,
    ),
    member("rhi::DebugLayerOptions::operator!=", RECORD_MEMBER),
    member("rhi::DebugLayerOptions::operator==", RECORD_MEMBER),
    member("rhi::DescriptorHandle::operator bool", RECORD_MEMBER),
    member("rhi::Extent3D::isWholeTexture", RECORD_MEMBER),
    member("rhi::Extent3D::kWholeTexture", STATIC_MEMBER),
    member("rhi::Extent3D::operator==", RECORD_MEMBER),
    member("rhi::HeapAlloc::getDeviceAddress", RECORD_MEMBER),
    member("rhi::HeapAlloc::getHostPtr", RECORD_MEMBER),
    member("rhi::HeapAlloc::isValid", RECORD_MEMBER),
    member("rhi::HeapAlloc::operator bool", RECORD_MEMBER),
    member("rhi::ICommandEncoder::clearBuffer", INTERFACE_HELPER),
    member("rhi::ICommandEncoder::finish", INTERFACE_HELPER),
    member("rhi::ICommandEncoder::setTextureState", INTERFACE_HELPER),
    member("rhi::ICommandQueue::createCommandEncoder", INTERFACE_HELPER),
    member("rhi::ICommandQueue::submit", INTERFACE_HELPER),
    member("rhi::IDevice::createBuffer", INTERFACE_HELPER),
    member("rhi::IDevice::createComputePipeline", INTERFACE_HELPER),
    member("rhi::IDevice::createFence", INTERFACE_HELPER),
    member("rhi::IDevice::createInputLayout", INTERFACE_HELPER),
    member("rhi::IDevice::createRayTracingPipeline", INTERFACE_HELPER),
    member("rhi::IDevice::createRenderPipeline", INTERFACE_HELPER),
    member("rhi::IDevice::createRootShaderObject", INTERFACE_HELPER),
    member("rhi::IDevice::createSampler", INTERFACE_HELPER),
    member("rhi::IDevice::createShaderObject", INTERFACE_HELPER),
    member("rhi::IDevice::createShaderProgram", INTERFACE_HELPER),
    member("rhi::IDevice::createSurface", INTERFACE_HELPER),
    member("rhi::IDevice::createTexture", INTERFACE_HELPER),
    member("rhi::IDevice::createTextureView", INTERFACE_HELPER),
    member("rhi::IDevice::getDeviceType", INTERFACE_HELPER),
    member("rhi::IDevice::getQueue", INTERFACE_HELPER),
    member("rhi::IDevice::getSlangSession", INTERFACE_HELPER),
    member("rhi::IDevice::getTextureRowAlignment", INTERFACE_HELPER),
    member("rhi::IHeap::report", INTERFACE_HELPER),
    member("rhi::IRHI::createBlob", INTERFACE_HELPER),
    member("rhi::IRHI::createDevice", INTERFACE_HELPER),
    member("rhi::IRHI::getAdapters", INTERFACE_HELPER),
    member("rhi::IRHI::isDebugLayersEnabled", INTERFACE_HELPER),
    member("rhi::IShaderObject::getEntryPoint", INTERFACE_HELPER),
    member("rhi::IShaderObject::getObject", INTERFACE_HELPER),
    member("rhi::ISurface::acquireNextImage", INTERFACE_HELPER),
    member("rhi::ITexture::createView", INTERFACE_HELPER),
    member("rhi::ITexture::getDefaultView", INTERFACE_HELPER),
    member("rhi::ITexture::getSubresourceLayout", INTERFACE_HELPER),
    member("rhi::NativeHandle::operator bool", RECORD_MEMBER),
    member("rhi::Offset3D::isZero", RECORD_MEMBER),
    member("rhi::Offset3D::Offset3D", RECORD_MEMBER),
    member("rhi::Offset3D::operator!=", RECORD_MEMBER),
    member("rhi::Offset3D::operator==", RECORD_MEMBER),
    member("rhi::ScissorRect::fromSize", RECORD_MEMBER),
    member("rhi::ShaderOffset::operator!=", RECORD_MEMBER),
    member("rhi::ShaderOffset::operator<", RECORD_MEMBER),
    member("rhi::ShaderOffset::operator<=", RECORD_MEMBER),
    member("rhi::ShaderOffset::operator==", RECORD_MEMBER),
    member("rhi::ShaderOffset::operator>", RECORD_MEMBER),
    member("rhi::ShaderOffset::operator>=", RECORD_MEMBER),
    member("rhi::SubresourceRange::operator!=", RECORD_MEMBER),
    member("rhi::SubresourceRange::operator==", RECORD_MEMBER),
    member("rhi::Viewport::fromSize", RECORD_MEMBER),
    member("rhi::WindowHandle::fromAndroidWindow", RECORD_MEMBER),
    member("rhi::WindowHandle::fromHwnd", RECORD_MEMBER),
    member("rhi::WindowHandle::fromNSWindow", RECORD_MEMBER),
    member("rhi::WindowHandle::fromXlibWindow", RECORD_MEMBER),
    member("rhi::TextureDesc::getLayerCount", RECORD_MEMBER),
    member("rhi::TextureDesc::getSubresourceCount", RECORD_MEMBER),
];

pub(crate) const SLANG_RHI: RuleSet = RuleSet {
    main_header: "slang-rhi.h",
    language: "c++20",
    defines: &["SLANG_USER_CONFIG=\"slang-user-config.h\""],
    library: "slang-rhi",
    own_crate: "slang_rhi_sys",
    external_crate: Some("slang_compiler_sys"),
    declarations: &[
        external("slang.h"),
        external("slang-deprecated.h"),
        external("slang-com-ptr.h"),
        external("slang-com-helper.h"),
        exclude("slang-rhi.h", "rhi::ComPtr", USING_DECLARATION),
        exclude("slang-rhi.h", "rhi::Guid", USING_DECLARATION),
        exclude(
            "slang-rhi.h",
            "*(StaticAssert at slang-rhi.h:*)",
            STATIC_ASSERT,
        ),
        exclude("slang-rhi.h", "rhi::operator*", ENUM_OPERATOR),
        exclude("slang-rhi.h", "rhi::is_set", ENUM_OPERATOR),
        exclude("slang-rhi.h", "rhi::flip_bit", ENUM_OPERATOR),
        exclude("slang-rhi.h", "rhi::getTextureDimension", INLINE_FUNCTION),
        exclude("slang-rhi.h", "rhi::getRHI", INLINE_FUNCTION),
        exclude("slang-rhi.h", "rhi::destroyRHI", INLINE_FUNCTION),
        exclude("slang-rhi.h", "rhi::getFormatInfo", INLINE_FUNCTION),
        exclude("slang-rhi.h", "rhi::FormatInfo", BIT_FIELDS),
        exclude(
            "slang-rhi.h",
            "rhi::AccelerationStructureInstanceDescGeneric",
            BIT_FIELDS,
        ),
        exclude("slang-rhi.h", "rhi::kEntireBuffer", STRUCT_CONSTANT),
        exclude("slang-rhi.h", "rhi::kEntireTexture", STRUCT_CONSTANT),
        exclude("slang-rhi.h", "rhi::kAllSubresources", STRUCT_CONSTANT),
        exclude("slang-rhi.h", "rhi::AdapterList", COM_PTR_HELPER),
        exclude(
            "slang-rhi.h",
            "rhi::DeviceScope",
            "an inline helper class that pushes and pops the CUDA context of a device in its constructor and destructor, which is not part of the ABI",
        ),
        exclude(
            "slang-rhi-device.h",
            "*(StaticAssert at slang-rhi-device.h:*)",
            STATIC_ASSERT,
        ),
        exclude(
            "slang-rhi-device.h",
            "rhi::AccelerationStructureInstanceDescD3D12",
            BIT_FIELDS,
        ),
        exclude(
            "slang-rhi-device.h",
            "rhi::AccelerationStructureInstanceDescVulkan",
            BIT_FIELDS,
        ),
        exclude(
            "slang-rhi-device.h",
            "rhi::AccelerationStructureMatrixMotionInstanceDescVulkan",
            BIT_FIELDS,
        ),
        exclude(
            "slang-rhi-device.h",
            "rhi::AccelerationStructureSRTMotionInstanceDescVulkan",
            BIT_FIELDS,
        ),
        exclude("slang-rhi-device.h", "rhi::TriangleClusterArgs", BIT_FIELDS),
        exclude(
            "slang-rhi-device.h",
            "rhi::AccelerationStructureStaticMotionInstanceVulkan",
            HOLDS_BIT_FIELDS,
        ),
        exclude(
            "slang-rhi-device.h",
            "rhi::AccelerationStructureMatrixMotionInstanceVulkan",
            HOLDS_BIT_FIELDS,
        ),
        exclude(
            "slang-rhi-device.h",
            "rhi::AccelerationStructureSRTMotionInstanceVulkan",
            HOLDS_BIT_FIELDS,
        ),
        include(
            "slang-rhi.h",
            "*",
            "the main header of slang-rhi, whose declarations the bindings include where they can",
        ),
        include(
            "slang-rhi-device.h",
            "*",
            "the header of the device-side records of slang-rhi, which slang-rhi.h includes",
        ),
        include(
            "capabilities.h",
            "*",
            "the header of the capabilities of a device, which slang-rhi.h includes",
        ),
    ],
    macros: &[
        slang_macros("slang.h"),
        slang_macros("slang-deprecated.h"),
        slang_macros("slang-com-ptr.h"),
        slang_macros("slang-com-helper.h"),
        rhi_macro(
            "slang-rhi-config.h",
            "SLANG_RHI_*",
            "a configuration macro of the build of slang-rhi, which names its backends and changes no declaration of slang-rhi.h",
        ),
        rhi_macro("slang-rhi.h", "SLANG_RHI_API", ATTRIBUTE_MACRO),
        rhi_macro(
            "slang-rhi.h",
            "SLANG_RHI_FEATURE_X",
            "a function-like helper that writes the enumerators of Feature, which libclang reports as object-like",
        ),
        rhi_macro(
            "capabilities.h",
            "SLANG_RHI_CAPABILITIES_x",
            "a function-like helper that writes the enumerators of Capability, which libclang reports as object-like",
        ),
    ],
    unnamed_enums: &[],
    result_type: "SlangResult",
    uuid_type: "SlangUUID",
    pointer_sized: &[
        ("SlangInt", Prim::Isize),
        ("SlangSSizeT", Prim::Isize),
        ("SlangUInt", Prim::Usize),
        ("SlangSizeT", Prim::Usize),
    ],
    members: Some(SLANG_RHI_MEMBERS),
    templates: &[NameRule {
        pattern: "Slang::ComPtr",
        reason: "a smart pointer of Slang that holds exactly one pointer to its argument, whose reference counting Rust does not run",
    }],
    uuidless: &[
        NameRule {
            pattern: "rhi::IAdapter",
            reason: "a class of slang-rhi with pure virtual methods, no base, and no UUID, whose vtable is the list of its methods",
        },
        NameRule {
            pattern: "rhi::IDebugCallback",
            reason: "a class with one pure virtual method, no base, and no UUID, which the caller implements to receive the messages of a device",
        },
        NameRule {
            pattern: "rhi::IRHI",
            reason: "the process-wide object of slang-rhi, a class with pure virtual methods, no base, and no UUID",
        },
    ],
    overloads: &[
        overload("rhi::IQueryPool::reset", &[], "reset", FIRST_OVERLOAD),
        overload(
            "rhi::IQueryPool::reset",
            &["uint32_t", "uint32_t"],
            "reset_range",
            LATER_OVERLOAD,
        ),
        overload(
            "rhi::IRenderPassEncoder::bindPipeline",
            &["IRenderPipeline *"],
            "bindPipeline",
            FIRST_OVERLOAD,
        ),
        overload(
            "rhi::IRenderPassEncoder::bindPipeline",
            &["IRenderPipeline *", "IShaderObject *"],
            "bindPipeline_root",
            LATER_OVERLOAD,
        ),
        overload(
            "rhi::IComputePassEncoder::bindPipeline",
            &["IComputePipeline *"],
            "bindPipeline",
            FIRST_OVERLOAD,
        ),
        overload(
            "rhi::IComputePassEncoder::bindPipeline",
            &["IComputePipeline *", "IShaderObject *"],
            "bindPipeline_root",
            LATER_OVERLOAD,
        ),
        overload(
            "rhi::IRayTracingPassEncoder::bindPipeline",
            &["IRayTracingPipeline *", "IShaderTable *"],
            "bindPipeline",
            FIRST_OVERLOAD,
        ),
        overload(
            "rhi::IRayTracingPassEncoder::bindPipeline",
            &["IRayTracingPipeline *", "IShaderTable *", "IShaderObject *"],
            "bindPipeline_root",
            LATER_OVERLOAD,
        ),
        overload(
            "rhi::IDevice::hasFeature",
            &["Feature"],
            "hasFeature",
            FIRST_OVERLOAD,
        ),
        overload(
            "rhi::IDevice::hasFeature",
            &["const char *"],
            "hasFeature_name",
            LATER_OVERLOAD,
        ),
        overload(
            "rhi::IDevice::hasCapability",
            &["Capability"],
            "hasCapability",
            FIRST_OVERLOAD,
        ),
        overload(
            "rhi::IDevice::hasCapability",
            &["const char *"],
            "hasCapability_name",
            LATER_OVERLOAD,
        ),
        overload(
            "rhi::IDevice::readTexture",
            &[
                "ITexture *",
                "uint32_t",
                "uint32_t",
                "const SubresourceLayout &",
                "void *",
            ],
            "readTexture",
            FIRST_OVERLOAD,
        ),
        overload(
            "rhi::IDevice::readTexture",
            &[
                "ITexture *",
                "uint32_t",
                "uint32_t",
                "ISlangBlob **",
                "SubresourceLayout *",
            ],
            "readTexture_blob",
            LATER_OVERLOAD,
        ),
        overload(
            "rhi::IDevice::readBuffer",
            &["IBuffer *", "Offset", "Size", "void *"],
            "readBuffer",
            FIRST_OVERLOAD,
        ),
        overload(
            "rhi::IDevice::readBuffer",
            &["IBuffer *", "Offset", "Size", "ISlangBlob **"],
            "readBuffer_blob",
            LATER_OVERLOAD,
        ),
    ],
    placeholders: &[
        placeholder("rhi::IRHI::getDebugLayerOptions"),
        placeholder("rhi::IRHI::setDebugLayerOptions"),
        placeholder("rhi::IDevice::createSurface"),
        placeholder("rhi::IDevice::createBufferFromSharedHandle"),
        placeholder("rhi::IDevice::createBufferFromNativeHandle"),
        placeholder("rhi::IDevice::createTextureFromSharedHandle"),
        placeholder("rhi::IDevice::createTextureFromNativeHandle"),
        placeholder("rhi::IHeap::free"),
        placeholder("rhi::ICommandEncoder::setTextureState"),
        placeholder("rhi::ICommandEncoder::buildAccelerationStructure"),
        placeholder("rhi::ICommandEncoder::clearTextureDepthStencil"),
        placeholder("rhi::ICommandEncoder::clearTextureSint"),
        placeholder("rhi::ICommandEncoder::clearTextureUint"),
        placeholder("rhi::ICommandEncoder::clearTextureFloat"),
        placeholder("rhi::ICommandEncoder::clearBuffer"),
        placeholder("rhi::ICommandEncoder::uploadTextureData"),
        placeholder("rhi::ICommandEncoder::copyBufferToTexture"),
        placeholder("rhi::ICommandEncoder::copyTextureToBuffer"),
        placeholder("rhi::ICommandEncoder::copyTexture"),
        placeholder("rhi::IComputePassEncoder::dispatchComputeIndirect"),
        placeholder("rhi::IRenderPassEncoder::drawIndexedIndirect"),
        placeholder("rhi::IRenderPassEncoder::drawIndirect"),
        placeholder("rhi::IAccelerationStructure::getHandle"),
        placeholder("rhi::IBuffer::getDescriptorHandle"),
    ],
    called_callbacks: &[
        called_callback("rhi::ITaskPool::submitTask::func"),
        called_callback("rhi::ITaskPool::submitTask::payloadDeleter"),
    ],
    without_default: &[
        without_default("rhi::ClearValue", NESTED_INITIALIZER),
        without_default("rhi::TextureDesc", BRACED_INITIALIZER),
        without_default("rhi::TextureViewDesc", STRUCT_INITIALIZER),
        without_default("rhi::SamplerDesc", BRACED_INITIALIZER),
        without_default("rhi::RenderPassColorAttachment", BRACED_INITIALIZER),
    ],
    abi_probe: true,
    initializers_in_place: true,
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
            .macro_position("slang.h", name)
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

    #[test]
    fn slang_rhi_takes_every_slang_header_as_external_and_its_own_headers_as_included() {
        for header in [
            "slang.h",
            "slang-deprecated.h",
            "slang-com-ptr.h",
            "slang-com-helper.h",
        ] {
            assert_eq!(
                SLANG_RHI
                    .declaration_rule(header, "slang::IGlobalSession")
                    .map(|rule| rule.action),
                Some(Action::External),
                "{header}"
            );
        }
        assert_eq!(
            SLANG_RHI
                .declaration_rule("slang-rhi.h", "rhi::IDevice")
                .map(|rule| rule.action),
            Some(Action::Include)
        );
        assert_eq!(
            SLANG_RHI
                .declaration_rule("slang-rhi.h", "rhi::AdapterList")
                .map(|rule| rule.action),
            Some(Action::Exclude)
        );
        assert_eq!(
            SLANG_RHI
                .declaration_rule("other.h", "rhi::IDevice")
                .map(|rule| rule.action),
            None
        );
    }

    #[test]
    fn macro_position_matches_the_header_of_a_rule_with_one() {
        let slang = SLANG_RHI
            .macro_position("slang.h", "SLANG_OK")
            .map(|position| SLANG_RHI.macros[position].action);
        assert_eq!(slang, Some(MacroAction::Exclude));
        assert_eq!(SLANG_RHI.macro_position("slang-rhi.h", "SLANG_OK"), None);
        assert!(
            SLANG_RHI
                .macro_position("slang-rhi-config.h", "SLANG_RHI_ENABLE_VULKAN")
                .is_some()
        );
        assert_eq!(
            SLANG_RHI.macro_position("slang-rhi.h", "SLANG_RHI_ENABLE_VULKAN"),
            None
        );
    }

    #[test]
    fn slang_rhi_names_both_overloads_of_every_overloaded_method() {
        let mut methods: Vec<&str> = SLANG_RHI.overloads.iter().map(|rule| rule.method).collect();
        methods.sort_unstable();
        for pair in methods.chunk_by(|left, right| left == right) {
            assert_eq!(pair.len(), 2, "{pair:?}");
        }
        for rule in SLANG_RHI.overloads {
            let suffixed = rule.rust.contains('_');
            assert_eq!(
                rule.reason == LATER_OVERLOAD,
                suffixed,
                "{} {}",
                rule.method,
                rule.rust
            );
            assert!(
                rule.method
                    .ends_with(&format!("::{}", rule.rust.split('_').next().unwrap_or(""))),
                "{} {}",
                rule.method,
                rule.rust
            );
        }
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
