use std::ffi::{CStr, c_void};
use std::hint::black_box;
use std::ptr;

use slang_compiler_sys::{
    IGlobalSession, ISlangUnknown, Interface, SLANG_BUILD_TAG, SlangGlobalSessionDesc, SlangResult,
    slang_createGlobalSession2, spGetBuildTagString,
};

macro_rules! c_functions {
    ($($name:ident),* $(,)?) => {
        [$((stringify!($name), slang_compiler_sys::$name as *const ())),*]
    };
}

fn declared_c_functions(bindings: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut in_block = false;
    for line in bindings.lines() {
        if line == "unsafe extern \"C\" {" {
            in_block = true;
        } else if line == "}" {
            in_block = false;
        } else if in_block && let Some(rest) = line.strip_prefix("    pub fn ") {
            names.extend(rest.split('(').next());
        }
    }
    names
}

#[test]
fn global_session_reports_the_pinned_build_tag_and_releases_every_reference() {
    let desc = SlangGlobalSessionDesc::default();
    let mut session: *mut IGlobalSession = ptr::null_mut();
    // SAFETY: `desc` is a live descriptor whose size field holds its own size,
    // which `Default` sets, and `session` is a live out-pointer, which is all
    // that `slang_createGlobalSession2` reads and writes.
    let result = unsafe { slang_createGlobalSession2(&raw const desc, &raw mut session) };
    assert_eq!(result, SlangResult::SLANG_OK);
    assert!(!session.is_null());

    // SAFETY: `spGetBuildTagString` takes no argument, and Slang documents it
    // as thread-safe.
    let tag = unsafe { spGetBuildTagString() };
    assert!(!tag.is_null());
    // SAFETY: `tag` is non-null and points to the null-terminated build tag,
    // which Slang keeps for as long as the library stays loaded, and the test
    // binary never unloads it.
    let tag = unsafe { CStr::from_ptr(tag) };
    assert_eq!(tag.to_str(), Ok(SLANG_BUILD_TAG));

    // SAFETY: `session` is the live global session that
    // `slang_createGlobalSession2` returned above, and only this thread uses it.
    let session_tag = unsafe { IGlobalSession::getBuildTagString(session) };
    assert!(!session_tag.is_null());
    // SAFETY: `session_tag` is non-null and points to the null-terminated
    // build tag, which Slang keeps for as long as the library stays loaded.
    let session_tag = unsafe { CStr::from_ptr(session_tag) };
    assert_eq!(session_tag.to_str(), Ok(SLANG_BUILD_TAG));

    let uuid = ISlangUnknown::UUID;
    let mut unknown: *mut c_void = ptr::null_mut();
    // SAFETY: `session` is the live global session, `uuid` is a live copy of
    // the UUID of `ISlangUnknown`, which every Slang object implements, and
    // `unknown` is a live out-pointer that receives the interface pointer with
    // one added reference.
    let result =
        unsafe { IGlobalSession::queryInterface(session, &raw const uuid, &raw mut unknown) };
    assert_eq!(result, SlangResult::SLANG_OK);
    assert_eq!(unknown, session.cast::<c_void>());

    // SAFETY: `unknown` is the interface pointer of `ISlangUnknown` that
    // `queryInterface` returned with one added reference, and this releases
    // exactly that reference.
    let count = unsafe { ISlangUnknown::release(unknown.cast::<ISlangUnknown>()) };
    assert_eq!(count, 1);
    // SAFETY: `session` holds the one reference that
    // `slang_createGlobalSession2` returned, no object was created from it,
    // and the test does not use `session` after this release.
    let count = unsafe { IGlobalSession::release(session) };
    assert_eq!(count, 0);
}

const C_FUNCTIONS: &[(&str, *const ())] = &c_functions![
    spGetBuildTagString,
    spReflectionUserAttribute_GetName,
    spReflectionUserAttribute_GetArgumentCount,
    spReflectionUserAttribute_GetArgumentType,
    spReflectionUserAttribute_GetArgumentValueInt,
    spReflectionUserAttribute_GetArgumentValueFloat,
    spReflectionUserAttribute_GetArgumentValueString,
    spReflectionType_GetKind,
    spReflectionType_GetUserAttributeCount,
    spReflectionType_GetUserAttribute,
    spReflectionType_FindUserAttributeByName,
    spReflectionType_applySpecializations,
    spReflectionType_GetFieldCount,
    spReflectionType_GetFieldByIndex,
    spReflectionType_GetElementCount,
    spReflectionType_GetSpecializedElementCount,
    spReflectionType_GetElementType,
    spReflectionType_GetRowCount,
    spReflectionType_GetColumnCount,
    spReflectionType_GetScalarType,
    spReflectionType_GetResourceShape,
    spReflectionType_GetResourceAccess,
    spReflectionType_GetResourceResultType,
    spReflectionType_GetName,
    spReflectionType_GetFullName,
    spReflectionType_GetGenericContainer,
    spReflectionTypeLayout_GetType,
    spReflectionTypeLayout_getKind,
    spReflectionTypeLayout_GetSize,
    spReflectionTypeLayout_GetStride,
    spReflectionTypeLayout_getAlignment,
    spReflectionTypeLayout_GetFieldCount,
    spReflectionTypeLayout_GetFieldByIndex,
    spReflectionTypeLayout_findFieldIndexByName,
    spReflectionTypeLayout_GetExplicitCounter,
    spReflectionTypeLayout_GetElementStride,
    spReflectionTypeLayout_GetElementTypeLayout,
    spReflectionTypeLayout_GetElementVarLayout,
    spReflectionTypeLayout_getContainerVarLayout,
    spReflectionTypeLayout_GetParameterCategory,
    spReflectionTypeLayout_GetCategoryCount,
    spReflectionTypeLayout_GetCategoryByIndex,
    spReflectionTypeLayout_GetMatrixLayoutMode,
    spReflectionTypeLayout_getGenericParamIndex,
    spReflectionTypeLayout_getPendingDataTypeLayout,
    spReflectionTypeLayout_getSpecializedTypePendingDataVarLayout,
    spReflectionType_getSpecializedTypeArgCount,
    spReflectionType_getSpecializedTypeArgType,
    spReflectionTypeLayout_getBindingRangeCount,
    spReflectionTypeLayout_getBindingRangeType,
    spReflectionTypeLayout_isBindingRangeSpecializable,
    spReflectionTypeLayout_getBindingRangeBindingCount,
    spReflectionTypeLayout_getBindingRangeLeafTypeLayout,
    spReflectionTypeLayout_getBindingRangeLeafVariable,
    spReflectionTypeLayout_getBindingRangeImageFormat,
    spReflectionTypeLayout_getFieldBindingRangeOffset,
    spReflectionTypeLayout_getExplicitCounterBindingRangeOffset,
    spReflectionTypeLayout_getBindingRangeDescriptorSetIndex,
    spReflectionTypeLayout_getBindingRangeFirstDescriptorRangeIndex,
    spReflectionTypeLayout_getBindingRangeDescriptorRangeCount,
    spReflectionTypeLayout_getDescriptorSetCount,
    spReflectionTypeLayout_getDescriptorSetSpaceOffset,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeCount,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeIndexOffset,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeDescriptorCount,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeType,
    spReflectionTypeLayout_getDescriptorSetDescriptorRangeCategory,
    spReflectionTypeLayout_getSubObjectRangeCount,
    spReflectionTypeLayout_getSubObjectRangeBindingRangeIndex,
    spReflectionTypeLayout_getSubObjectRangeSpaceOffset,
    spReflectionTypeLayout_getSubObjectRangeOffset,
    spReflectionVariable_GetName,
    spReflectionVariable_GetType,
    spReflectionVariable_FindModifier,
    spReflectionVariable_GetUserAttributeCount,
    spReflectionVariable_GetUserAttribute,
    spReflectionVariable_FindUserAttributeByName,
    spReflectionVariable_HasDefaultValue,
    spReflectionVariable_GetDefaultValueInt,
    spReflectionVariable_GetDefaultValueFloat,
    spReflectionVariable_GetGenericContainer,
    spReflectionVariable_applySpecializations,
    spReflectionVariableLayout_GetVariable,
    spReflectionVariableLayout_GetTypeLayout,
    spReflectionVariableLayout_GetOffset,
    spReflectionVariableLayout_GetSpace,
    spReflectionVariableLayout_GetImageFormat,
    spReflectionVariableLayout_GetSemanticName,
    spReflectionVariableLayout_GetSemanticIndex,
    spReflectionFunction_asDecl,
    spReflectionFunction_GetName,
    spReflectionFunction_FindModifier,
    spReflectionFunction_GetUserAttributeCount,
    spReflectionFunction_GetUserAttribute,
    spReflectionFunction_FindUserAttributeByName,
    spReflectionFunction_GetParameterCount,
    spReflectionFunction_GetParameter,
    spReflectionFunction_GetResultType,
    spReflectionFunction_GetGenericContainer,
    spReflectionFunction_applySpecializations,
    spReflectionFunction_specializeWithArgTypes,
    spReflectionFunction_isOverloaded,
    spReflectionFunction_getOverloadCount,
    spReflectionFunction_getOverload,
    spReflectionDecl_getChildrenCount,
    spReflectionDecl_getChild,
    spReflectionDecl_getName,
    spReflectionDecl_getKind,
    spReflectionDecl_castToFunction,
    spReflectionDecl_castToVariable,
    spReflectionDecl_castToGeneric,
    spReflection_getTypeFromDecl,
    spReflectionDecl_getParent,
    spReflectionDecl_findModifier,
    spReflectionGeneric_asDecl,
    spReflectionGeneric_GetName,
    spReflectionGeneric_GetTypeParameterCount,
    spReflectionGeneric_GetTypeParameter,
    spReflectionGeneric_GetValueParameterCount,
    spReflectionGeneric_GetValueParameter,
    spReflectionGeneric_GetTypeParameterConstraintCount,
    spReflectionGeneric_GetTypeParameterConstraintType,
    spReflectionGeneric_GetInnerKind,
    spReflectionGeneric_GetInnerDecl,
    spReflectionGeneric_GetOuterGenericContainer,
    spReflectionGeneric_GetConcreteType,
    spReflectionGeneric_GetConcreteIntVal,
    spReflectionGeneric_applySpecializations,
    spReflectionVariableLayout_getStage,
    spReflectionVariableLayout_getPendingDataLayout,
    spReflectionParameter_GetBindingIndex,
    spReflectionParameter_GetBindingSpace,
    spReflectionEntryPoint_getName,
    spReflectionEntryPoint_getNameOverride,
    spReflectionEntryPoint_getFunction,
    spReflectionEntryPoint_getParameterCount,
    spReflectionEntryPoint_getParameterByIndex,
    spReflectionEntryPoint_getStage,
    spReflectionEntryPoint_getComputeThreadGroupSize,
    spReflectionEntryPoint_getComputeWaveSize,
    spReflectionEntryPoint_usesAnySampleRateInput,
    spReflectionEntryPoint_getVarLayout,
    spReflectionEntryPoint_getResultVarLayout,
    spReflectionEntryPoint_hasDefaultConstantBuffer,
    spReflectionTypeParameter_GetName,
    spReflectionTypeParameter_GetIndex,
    spReflectionTypeParameter_GetConstraintCount,
    spReflectionTypeParameter_GetConstraintByIndex,
    spReflection_ToJson,
    spReflection_GetParameterCount,
    spReflection_GetParameterByIndex,
    spReflection_GetTypeParameterCount,
    spReflection_GetTypeParameterByIndex,
    spReflection_FindTypeParameter,
    spReflection_FindTypeByName,
    spReflection_GetTypeLayout,
    spReflection_FindFunctionByName,
    spReflection_FindFunctionByNameInType,
    spReflection_FindVarByNameInType,
    spReflection_TryResolveOverloadedFunction,
    spReflection_getEntryPointCount,
    spReflection_getEntryPointByIndex,
    spReflection_findEntryPointByName,
    spReflection_getGlobalConstantBufferBinding,
    spReflection_getGlobalConstantBufferSize,
    spReflection_specializeType,
    spReflection_specializeGeneric,
    spReflection_isSubType,
    spReflection_getHashedStringCount,
    spReflection_getHashedString,
    spReflection_getGlobalParamsTypeLayout,
    spReflection_getGlobalParamsVarLayout,
    spReflection_getBindlessSpaceIndex,
    slang_createBlob,
    slang_writeCoverageManifestJson,
    slang_loadModuleFromSource,
    slang_loadModuleFromIRBlob,
    slang_loadModuleInfoFromIRBlob,
    slang_createGlobalSession,
    slang_createGlobalSession2,
    slang_createGlobalSessionWithoutCoreModule,
    slang_shutdown,
    slang_enableRecordLayer,
    slang_isRecordLayerEnabled,
    slang_setReplayDirectory,
    slang_getReplayDirectory,
    slang_getCurrentReplayPath,
    slang_loadReplay,
    slang_loadLatestReplay,
    slang_replayMarker,
    slang_getLastInternalErrorMessage,
    slang_createByteCodeRunner,
    slang_disassembleByteCode,
];

#[test]
fn every_c_function_of_the_bindings_resolves_to_a_native_symbol() {
    let listed: Vec<&str> = C_FUNCTIONS.iter().map(|(name, _)| *name).collect();
    assert_eq!(
        listed,
        declared_c_functions(include_str!("../../src/bindings.rs"))
    );
    for (name, address) in C_FUNCTIONS {
        assert!(!black_box(address).is_null(), "{name}");
    }
}
