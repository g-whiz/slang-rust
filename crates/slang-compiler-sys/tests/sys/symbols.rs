//! The C functions of the raw Slang bindings, generated from the Slang headers by the binding generator. Run `just generate-bindings` to regenerate them instead of editing this file.

pub(super) const C_FUNCTIONS: &[(&str, *const ())] = &[
    (
        "spGetBuildTagString",
        slang_compiler_sys::spGetBuildTagString as *const (),
    ),
    (
        "spReflectionUserAttribute_GetName",
        slang_compiler_sys::spReflectionUserAttribute_GetName as *const (),
    ),
    (
        "spReflectionUserAttribute_GetArgumentCount",
        slang_compiler_sys::spReflectionUserAttribute_GetArgumentCount as *const (),
    ),
    (
        "spReflectionUserAttribute_GetArgumentType",
        slang_compiler_sys::spReflectionUserAttribute_GetArgumentType as *const (),
    ),
    (
        "spReflectionUserAttribute_GetArgumentValueInt",
        slang_compiler_sys::spReflectionUserAttribute_GetArgumentValueInt as *const (),
    ),
    (
        "spReflectionUserAttribute_GetArgumentValueFloat",
        slang_compiler_sys::spReflectionUserAttribute_GetArgumentValueFloat as *const (),
    ),
    (
        "spReflectionUserAttribute_GetArgumentValueString",
        slang_compiler_sys::spReflectionUserAttribute_GetArgumentValueString as *const (),
    ),
    (
        "spReflectionType_GetKind",
        slang_compiler_sys::spReflectionType_GetKind as *const (),
    ),
    (
        "spReflectionType_GetUserAttributeCount",
        slang_compiler_sys::spReflectionType_GetUserAttributeCount as *const (),
    ),
    (
        "spReflectionType_GetUserAttribute",
        slang_compiler_sys::spReflectionType_GetUserAttribute as *const (),
    ),
    (
        "spReflectionType_FindUserAttributeByName",
        slang_compiler_sys::spReflectionType_FindUserAttributeByName as *const (),
    ),
    (
        "spReflectionType_applySpecializations",
        slang_compiler_sys::spReflectionType_applySpecializations as *const (),
    ),
    (
        "spReflectionType_GetFieldCount",
        slang_compiler_sys::spReflectionType_GetFieldCount as *const (),
    ),
    (
        "spReflectionType_GetFieldByIndex",
        slang_compiler_sys::spReflectionType_GetFieldByIndex as *const (),
    ),
    (
        "spReflectionType_GetElementCount",
        slang_compiler_sys::spReflectionType_GetElementCount as *const (),
    ),
    (
        "spReflectionType_GetSpecializedElementCount",
        slang_compiler_sys::spReflectionType_GetSpecializedElementCount as *const (),
    ),
    (
        "spReflectionType_GetElementType",
        slang_compiler_sys::spReflectionType_GetElementType as *const (),
    ),
    (
        "spReflectionType_GetRowCount",
        slang_compiler_sys::spReflectionType_GetRowCount as *const (),
    ),
    (
        "spReflectionType_GetColumnCount",
        slang_compiler_sys::spReflectionType_GetColumnCount as *const (),
    ),
    (
        "spReflectionType_GetScalarType",
        slang_compiler_sys::spReflectionType_GetScalarType as *const (),
    ),
    (
        "spReflectionType_GetResourceShape",
        slang_compiler_sys::spReflectionType_GetResourceShape as *const (),
    ),
    (
        "spReflectionType_GetResourceAccess",
        slang_compiler_sys::spReflectionType_GetResourceAccess as *const (),
    ),
    (
        "spReflectionType_GetResourceResultType",
        slang_compiler_sys::spReflectionType_GetResourceResultType as *const (),
    ),
    (
        "spReflectionType_GetName",
        slang_compiler_sys::spReflectionType_GetName as *const (),
    ),
    (
        "spReflectionType_GetFullName",
        slang_compiler_sys::spReflectionType_GetFullName as *const (),
    ),
    (
        "spReflectionType_GetGenericContainer",
        slang_compiler_sys::spReflectionType_GetGenericContainer as *const (),
    ),
    (
        "spReflectionTypeLayout_GetType",
        slang_compiler_sys::spReflectionTypeLayout_GetType as *const (),
    ),
    (
        "spReflectionTypeLayout_getKind",
        slang_compiler_sys::spReflectionTypeLayout_getKind as *const (),
    ),
    (
        "spReflectionTypeLayout_GetSize",
        slang_compiler_sys::spReflectionTypeLayout_GetSize as *const (),
    ),
    (
        "spReflectionTypeLayout_GetStride",
        slang_compiler_sys::spReflectionTypeLayout_GetStride as *const (),
    ),
    (
        "spReflectionTypeLayout_getAlignment",
        slang_compiler_sys::spReflectionTypeLayout_getAlignment as *const (),
    ),
    (
        "spReflectionTypeLayout_GetFieldCount",
        slang_compiler_sys::spReflectionTypeLayout_GetFieldCount as *const (),
    ),
    (
        "spReflectionTypeLayout_GetFieldByIndex",
        slang_compiler_sys::spReflectionTypeLayout_GetFieldByIndex as *const (),
    ),
    (
        "spReflectionTypeLayout_findFieldIndexByName",
        slang_compiler_sys::spReflectionTypeLayout_findFieldIndexByName as *const (),
    ),
    (
        "spReflectionTypeLayout_GetExplicitCounter",
        slang_compiler_sys::spReflectionTypeLayout_GetExplicitCounter as *const (),
    ),
    (
        "spReflectionTypeLayout_GetElementStride",
        slang_compiler_sys::spReflectionTypeLayout_GetElementStride as *const (),
    ),
    (
        "spReflectionTypeLayout_GetElementTypeLayout",
        slang_compiler_sys::spReflectionTypeLayout_GetElementTypeLayout as *const (),
    ),
    (
        "spReflectionTypeLayout_GetElementVarLayout",
        slang_compiler_sys::spReflectionTypeLayout_GetElementVarLayout as *const (),
    ),
    (
        "spReflectionTypeLayout_getContainerVarLayout",
        slang_compiler_sys::spReflectionTypeLayout_getContainerVarLayout as *const (),
    ),
    (
        "spReflectionTypeLayout_GetParameterCategory",
        slang_compiler_sys::spReflectionTypeLayout_GetParameterCategory as *const (),
    ),
    (
        "spReflectionTypeLayout_GetCategoryCount",
        slang_compiler_sys::spReflectionTypeLayout_GetCategoryCount as *const (),
    ),
    (
        "spReflectionTypeLayout_GetCategoryByIndex",
        slang_compiler_sys::spReflectionTypeLayout_GetCategoryByIndex as *const (),
    ),
    (
        "spReflectionTypeLayout_GetMatrixLayoutMode",
        slang_compiler_sys::spReflectionTypeLayout_GetMatrixLayoutMode as *const (),
    ),
    (
        "spReflectionTypeLayout_getGenericParamIndex",
        slang_compiler_sys::spReflectionTypeLayout_getGenericParamIndex as *const (),
    ),
    (
        "spReflectionTypeLayout_getPendingDataTypeLayout",
        slang_compiler_sys::spReflectionTypeLayout_getPendingDataTypeLayout as *const (),
    ),
    (
        "spReflectionTypeLayout_getSpecializedTypePendingDataVarLayout",
        slang_compiler_sys::spReflectionTypeLayout_getSpecializedTypePendingDataVarLayout
            as *const (),
    ),
    (
        "spReflectionType_getSpecializedTypeArgCount",
        slang_compiler_sys::spReflectionType_getSpecializedTypeArgCount as *const (),
    ),
    (
        "spReflectionType_getSpecializedTypeArgType",
        slang_compiler_sys::spReflectionType_getSpecializedTypeArgType as *const (),
    ),
    (
        "spReflectionTypeLayout_getBindingRangeCount",
        slang_compiler_sys::spReflectionTypeLayout_getBindingRangeCount as *const (),
    ),
    (
        "spReflectionTypeLayout_getBindingRangeType",
        slang_compiler_sys::spReflectionTypeLayout_getBindingRangeType as *const (),
    ),
    (
        "spReflectionTypeLayout_isBindingRangeSpecializable",
        slang_compiler_sys::spReflectionTypeLayout_isBindingRangeSpecializable as *const (),
    ),
    (
        "spReflectionTypeLayout_getBindingRangeBindingCount",
        slang_compiler_sys::spReflectionTypeLayout_getBindingRangeBindingCount as *const (),
    ),
    (
        "spReflectionTypeLayout_getBindingRangeLeafTypeLayout",
        slang_compiler_sys::spReflectionTypeLayout_getBindingRangeLeafTypeLayout as *const (),
    ),
    (
        "spReflectionTypeLayout_getBindingRangeLeafVariable",
        slang_compiler_sys::spReflectionTypeLayout_getBindingRangeLeafVariable as *const (),
    ),
    (
        "spReflectionTypeLayout_getBindingRangeImageFormat",
        slang_compiler_sys::spReflectionTypeLayout_getBindingRangeImageFormat as *const (),
    ),
    (
        "spReflectionTypeLayout_getFieldBindingRangeOffset",
        slang_compiler_sys::spReflectionTypeLayout_getFieldBindingRangeOffset as *const (),
    ),
    (
        "spReflectionTypeLayout_getExplicitCounterBindingRangeOffset",
        slang_compiler_sys::spReflectionTypeLayout_getExplicitCounterBindingRangeOffset
            as *const (),
    ),
    (
        "spReflectionTypeLayout_getBindingRangeDescriptorSetIndex",
        slang_compiler_sys::spReflectionTypeLayout_getBindingRangeDescriptorSetIndex as *const (),
    ),
    (
        "spReflectionTypeLayout_getBindingRangeFirstDescriptorRangeIndex",
        slang_compiler_sys::spReflectionTypeLayout_getBindingRangeFirstDescriptorRangeIndex
            as *const (),
    ),
    (
        "spReflectionTypeLayout_getBindingRangeDescriptorRangeCount",
        slang_compiler_sys::spReflectionTypeLayout_getBindingRangeDescriptorRangeCount as *const (),
    ),
    (
        "spReflectionTypeLayout_getDescriptorSetCount",
        slang_compiler_sys::spReflectionTypeLayout_getDescriptorSetCount as *const (),
    ),
    (
        "spReflectionTypeLayout_getDescriptorSetSpaceOffset",
        slang_compiler_sys::spReflectionTypeLayout_getDescriptorSetSpaceOffset as *const (),
    ),
    (
        "spReflectionTypeLayout_getDescriptorSetDescriptorRangeCount",
        slang_compiler_sys::spReflectionTypeLayout_getDescriptorSetDescriptorRangeCount
            as *const (),
    ),
    (
        "spReflectionTypeLayout_getDescriptorSetDescriptorRangeIndexOffset",
        slang_compiler_sys::spReflectionTypeLayout_getDescriptorSetDescriptorRangeIndexOffset
            as *const (),
    ),
    (
        "spReflectionTypeLayout_getDescriptorSetDescriptorRangeDescriptorCount",
        slang_compiler_sys::spReflectionTypeLayout_getDescriptorSetDescriptorRangeDescriptorCount
            as *const (),
    ),
    (
        "spReflectionTypeLayout_getDescriptorSetDescriptorRangeType",
        slang_compiler_sys::spReflectionTypeLayout_getDescriptorSetDescriptorRangeType as *const (),
    ),
    (
        "spReflectionTypeLayout_getDescriptorSetDescriptorRangeCategory",
        slang_compiler_sys::spReflectionTypeLayout_getDescriptorSetDescriptorRangeCategory
            as *const (),
    ),
    (
        "spReflectionTypeLayout_getSubObjectRangeCount",
        slang_compiler_sys::spReflectionTypeLayout_getSubObjectRangeCount as *const (),
    ),
    (
        "spReflectionTypeLayout_getSubObjectRangeBindingRangeIndex",
        slang_compiler_sys::spReflectionTypeLayout_getSubObjectRangeBindingRangeIndex as *const (),
    ),
    (
        "spReflectionTypeLayout_getSubObjectRangeSpaceOffset",
        slang_compiler_sys::spReflectionTypeLayout_getSubObjectRangeSpaceOffset as *const (),
    ),
    (
        "spReflectionTypeLayout_getSubObjectRangeOffset",
        slang_compiler_sys::spReflectionTypeLayout_getSubObjectRangeOffset as *const (),
    ),
    (
        "spReflectionVariable_GetName",
        slang_compiler_sys::spReflectionVariable_GetName as *const (),
    ),
    (
        "spReflectionVariable_GetType",
        slang_compiler_sys::spReflectionVariable_GetType as *const (),
    ),
    (
        "spReflectionVariable_FindModifier",
        slang_compiler_sys::spReflectionVariable_FindModifier as *const (),
    ),
    (
        "spReflectionVariable_GetUserAttributeCount",
        slang_compiler_sys::spReflectionVariable_GetUserAttributeCount as *const (),
    ),
    (
        "spReflectionVariable_GetUserAttribute",
        slang_compiler_sys::spReflectionVariable_GetUserAttribute as *const (),
    ),
    (
        "spReflectionVariable_FindUserAttributeByName",
        slang_compiler_sys::spReflectionVariable_FindUserAttributeByName as *const (),
    ),
    (
        "spReflectionVariable_HasDefaultValue",
        slang_compiler_sys::spReflectionVariable_HasDefaultValue as *const (),
    ),
    (
        "spReflectionVariable_GetDefaultValueInt",
        slang_compiler_sys::spReflectionVariable_GetDefaultValueInt as *const (),
    ),
    (
        "spReflectionVariable_GetDefaultValueFloat",
        slang_compiler_sys::spReflectionVariable_GetDefaultValueFloat as *const (),
    ),
    (
        "spReflectionVariable_GetGenericContainer",
        slang_compiler_sys::spReflectionVariable_GetGenericContainer as *const (),
    ),
    (
        "spReflectionVariable_applySpecializations",
        slang_compiler_sys::spReflectionVariable_applySpecializations as *const (),
    ),
    (
        "spReflectionVariableLayout_GetVariable",
        slang_compiler_sys::spReflectionVariableLayout_GetVariable as *const (),
    ),
    (
        "spReflectionVariableLayout_GetTypeLayout",
        slang_compiler_sys::spReflectionVariableLayout_GetTypeLayout as *const (),
    ),
    (
        "spReflectionVariableLayout_GetOffset",
        slang_compiler_sys::spReflectionVariableLayout_GetOffset as *const (),
    ),
    (
        "spReflectionVariableLayout_GetSpace",
        slang_compiler_sys::spReflectionVariableLayout_GetSpace as *const (),
    ),
    (
        "spReflectionVariableLayout_GetImageFormat",
        slang_compiler_sys::spReflectionVariableLayout_GetImageFormat as *const (),
    ),
    (
        "spReflectionVariableLayout_GetSemanticName",
        slang_compiler_sys::spReflectionVariableLayout_GetSemanticName as *const (),
    ),
    (
        "spReflectionVariableLayout_GetSemanticIndex",
        slang_compiler_sys::spReflectionVariableLayout_GetSemanticIndex as *const (),
    ),
    (
        "spReflectionFunction_asDecl",
        slang_compiler_sys::spReflectionFunction_asDecl as *const (),
    ),
    (
        "spReflectionFunction_GetName",
        slang_compiler_sys::spReflectionFunction_GetName as *const (),
    ),
    (
        "spReflectionFunction_FindModifier",
        slang_compiler_sys::spReflectionFunction_FindModifier as *const (),
    ),
    (
        "spReflectionFunction_GetUserAttributeCount",
        slang_compiler_sys::spReflectionFunction_GetUserAttributeCount as *const (),
    ),
    (
        "spReflectionFunction_GetUserAttribute",
        slang_compiler_sys::spReflectionFunction_GetUserAttribute as *const (),
    ),
    (
        "spReflectionFunction_FindUserAttributeByName",
        slang_compiler_sys::spReflectionFunction_FindUserAttributeByName as *const (),
    ),
    (
        "spReflectionFunction_GetParameterCount",
        slang_compiler_sys::spReflectionFunction_GetParameterCount as *const (),
    ),
    (
        "spReflectionFunction_GetParameter",
        slang_compiler_sys::spReflectionFunction_GetParameter as *const (),
    ),
    (
        "spReflectionFunction_GetResultType",
        slang_compiler_sys::spReflectionFunction_GetResultType as *const (),
    ),
    (
        "spReflectionFunction_GetGenericContainer",
        slang_compiler_sys::spReflectionFunction_GetGenericContainer as *const (),
    ),
    (
        "spReflectionFunction_applySpecializations",
        slang_compiler_sys::spReflectionFunction_applySpecializations as *const (),
    ),
    (
        "spReflectionFunction_specializeWithArgTypes",
        slang_compiler_sys::spReflectionFunction_specializeWithArgTypes as *const (),
    ),
    (
        "spReflectionFunction_isOverloaded",
        slang_compiler_sys::spReflectionFunction_isOverloaded as *const (),
    ),
    (
        "spReflectionFunction_getOverloadCount",
        slang_compiler_sys::spReflectionFunction_getOverloadCount as *const (),
    ),
    (
        "spReflectionFunction_getOverload",
        slang_compiler_sys::spReflectionFunction_getOverload as *const (),
    ),
    (
        "spReflectionDecl_getChildrenCount",
        slang_compiler_sys::spReflectionDecl_getChildrenCount as *const (),
    ),
    (
        "spReflectionDecl_getChild",
        slang_compiler_sys::spReflectionDecl_getChild as *const (),
    ),
    (
        "spReflectionDecl_getName",
        slang_compiler_sys::spReflectionDecl_getName as *const (),
    ),
    (
        "spReflectionDecl_getKind",
        slang_compiler_sys::spReflectionDecl_getKind as *const (),
    ),
    (
        "spReflectionDecl_castToFunction",
        slang_compiler_sys::spReflectionDecl_castToFunction as *const (),
    ),
    (
        "spReflectionDecl_castToVariable",
        slang_compiler_sys::spReflectionDecl_castToVariable as *const (),
    ),
    (
        "spReflectionDecl_castToGeneric",
        slang_compiler_sys::spReflectionDecl_castToGeneric as *const (),
    ),
    (
        "spReflection_getTypeFromDecl",
        slang_compiler_sys::spReflection_getTypeFromDecl as *const (),
    ),
    (
        "spReflectionDecl_getParent",
        slang_compiler_sys::spReflectionDecl_getParent as *const (),
    ),
    (
        "spReflectionDecl_findModifier",
        slang_compiler_sys::spReflectionDecl_findModifier as *const (),
    ),
    (
        "spReflectionGeneric_asDecl",
        slang_compiler_sys::spReflectionGeneric_asDecl as *const (),
    ),
    (
        "spReflectionGeneric_GetName",
        slang_compiler_sys::spReflectionGeneric_GetName as *const (),
    ),
    (
        "spReflectionGeneric_GetTypeParameterCount",
        slang_compiler_sys::spReflectionGeneric_GetTypeParameterCount as *const (),
    ),
    (
        "spReflectionGeneric_GetTypeParameter",
        slang_compiler_sys::spReflectionGeneric_GetTypeParameter as *const (),
    ),
    (
        "spReflectionGeneric_GetValueParameterCount",
        slang_compiler_sys::spReflectionGeneric_GetValueParameterCount as *const (),
    ),
    (
        "spReflectionGeneric_GetValueParameter",
        slang_compiler_sys::spReflectionGeneric_GetValueParameter as *const (),
    ),
    (
        "spReflectionGeneric_GetTypeParameterConstraintCount",
        slang_compiler_sys::spReflectionGeneric_GetTypeParameterConstraintCount as *const (),
    ),
    (
        "spReflectionGeneric_GetTypeParameterConstraintType",
        slang_compiler_sys::spReflectionGeneric_GetTypeParameterConstraintType as *const (),
    ),
    (
        "spReflectionGeneric_GetInnerKind",
        slang_compiler_sys::spReflectionGeneric_GetInnerKind as *const (),
    ),
    (
        "spReflectionGeneric_GetInnerDecl",
        slang_compiler_sys::spReflectionGeneric_GetInnerDecl as *const (),
    ),
    (
        "spReflectionGeneric_GetOuterGenericContainer",
        slang_compiler_sys::spReflectionGeneric_GetOuterGenericContainer as *const (),
    ),
    (
        "spReflectionGeneric_GetConcreteType",
        slang_compiler_sys::spReflectionGeneric_GetConcreteType as *const (),
    ),
    (
        "spReflectionGeneric_GetConcreteIntVal",
        slang_compiler_sys::spReflectionGeneric_GetConcreteIntVal as *const (),
    ),
    (
        "spReflectionGeneric_applySpecializations",
        slang_compiler_sys::spReflectionGeneric_applySpecializations as *const (),
    ),
    (
        "spReflectionVariableLayout_getStage",
        slang_compiler_sys::spReflectionVariableLayout_getStage as *const (),
    ),
    (
        "spReflectionVariableLayout_getPendingDataLayout",
        slang_compiler_sys::spReflectionVariableLayout_getPendingDataLayout as *const (),
    ),
    (
        "spReflectionParameter_GetBindingIndex",
        slang_compiler_sys::spReflectionParameter_GetBindingIndex as *const (),
    ),
    (
        "spReflectionParameter_GetBindingSpace",
        slang_compiler_sys::spReflectionParameter_GetBindingSpace as *const (),
    ),
    (
        "spReflectionEntryPoint_getName",
        slang_compiler_sys::spReflectionEntryPoint_getName as *const (),
    ),
    (
        "spReflectionEntryPoint_getNameOverride",
        slang_compiler_sys::spReflectionEntryPoint_getNameOverride as *const (),
    ),
    (
        "spReflectionEntryPoint_getFunction",
        slang_compiler_sys::spReflectionEntryPoint_getFunction as *const (),
    ),
    (
        "spReflectionEntryPoint_getParameterCount",
        slang_compiler_sys::spReflectionEntryPoint_getParameterCount as *const (),
    ),
    (
        "spReflectionEntryPoint_getParameterByIndex",
        slang_compiler_sys::spReflectionEntryPoint_getParameterByIndex as *const (),
    ),
    (
        "spReflectionEntryPoint_getStage",
        slang_compiler_sys::spReflectionEntryPoint_getStage as *const (),
    ),
    (
        "spReflectionEntryPoint_getComputeThreadGroupSize",
        slang_compiler_sys::spReflectionEntryPoint_getComputeThreadGroupSize as *const (),
    ),
    (
        "spReflectionEntryPoint_getComputeWaveSize",
        slang_compiler_sys::spReflectionEntryPoint_getComputeWaveSize as *const (),
    ),
    (
        "spReflectionEntryPoint_usesAnySampleRateInput",
        slang_compiler_sys::spReflectionEntryPoint_usesAnySampleRateInput as *const (),
    ),
    (
        "spReflectionEntryPoint_getVarLayout",
        slang_compiler_sys::spReflectionEntryPoint_getVarLayout as *const (),
    ),
    (
        "spReflectionEntryPoint_getResultVarLayout",
        slang_compiler_sys::spReflectionEntryPoint_getResultVarLayout as *const (),
    ),
    (
        "spReflectionEntryPoint_hasDefaultConstantBuffer",
        slang_compiler_sys::spReflectionEntryPoint_hasDefaultConstantBuffer as *const (),
    ),
    (
        "spReflectionTypeParameter_GetName",
        slang_compiler_sys::spReflectionTypeParameter_GetName as *const (),
    ),
    (
        "spReflectionTypeParameter_GetIndex",
        slang_compiler_sys::spReflectionTypeParameter_GetIndex as *const (),
    ),
    (
        "spReflectionTypeParameter_GetConstraintCount",
        slang_compiler_sys::spReflectionTypeParameter_GetConstraintCount as *const (),
    ),
    (
        "spReflectionTypeParameter_GetConstraintByIndex",
        slang_compiler_sys::spReflectionTypeParameter_GetConstraintByIndex as *const (),
    ),
    (
        "spReflection_ToJson",
        slang_compiler_sys::spReflection_ToJson as *const (),
    ),
    (
        "spReflection_GetParameterCount",
        slang_compiler_sys::spReflection_GetParameterCount as *const (),
    ),
    (
        "spReflection_GetParameterByIndex",
        slang_compiler_sys::spReflection_GetParameterByIndex as *const (),
    ),
    (
        "spReflection_GetTypeParameterCount",
        slang_compiler_sys::spReflection_GetTypeParameterCount as *const (),
    ),
    (
        "spReflection_GetTypeParameterByIndex",
        slang_compiler_sys::spReflection_GetTypeParameterByIndex as *const (),
    ),
    (
        "spReflection_FindTypeParameter",
        slang_compiler_sys::spReflection_FindTypeParameter as *const (),
    ),
    (
        "spReflection_FindTypeByName",
        slang_compiler_sys::spReflection_FindTypeByName as *const (),
    ),
    (
        "spReflection_GetTypeLayout",
        slang_compiler_sys::spReflection_GetTypeLayout as *const (),
    ),
    (
        "spReflection_FindFunctionByName",
        slang_compiler_sys::spReflection_FindFunctionByName as *const (),
    ),
    (
        "spReflection_FindFunctionByNameInType",
        slang_compiler_sys::spReflection_FindFunctionByNameInType as *const (),
    ),
    (
        "spReflection_FindVarByNameInType",
        slang_compiler_sys::spReflection_FindVarByNameInType as *const (),
    ),
    (
        "spReflection_TryResolveOverloadedFunction",
        slang_compiler_sys::spReflection_TryResolveOverloadedFunction as *const (),
    ),
    (
        "spReflection_getEntryPointCount",
        slang_compiler_sys::spReflection_getEntryPointCount as *const (),
    ),
    (
        "spReflection_getEntryPointByIndex",
        slang_compiler_sys::spReflection_getEntryPointByIndex as *const (),
    ),
    (
        "spReflection_findEntryPointByName",
        slang_compiler_sys::spReflection_findEntryPointByName as *const (),
    ),
    (
        "spReflection_getGlobalConstantBufferBinding",
        slang_compiler_sys::spReflection_getGlobalConstantBufferBinding as *const (),
    ),
    (
        "spReflection_getGlobalConstantBufferSize",
        slang_compiler_sys::spReflection_getGlobalConstantBufferSize as *const (),
    ),
    (
        "spReflection_specializeType",
        slang_compiler_sys::spReflection_specializeType as *const (),
    ),
    (
        "spReflection_specializeGeneric",
        slang_compiler_sys::spReflection_specializeGeneric as *const (),
    ),
    (
        "spReflection_isSubType",
        slang_compiler_sys::spReflection_isSubType as *const (),
    ),
    (
        "spReflection_getHashedStringCount",
        slang_compiler_sys::spReflection_getHashedStringCount as *const (),
    ),
    (
        "spReflection_getHashedString",
        slang_compiler_sys::spReflection_getHashedString as *const (),
    ),
    (
        "spReflection_getGlobalParamsTypeLayout",
        slang_compiler_sys::spReflection_getGlobalParamsTypeLayout as *const (),
    ),
    (
        "spReflection_getGlobalParamsVarLayout",
        slang_compiler_sys::spReflection_getGlobalParamsVarLayout as *const (),
    ),
    (
        "spReflection_getBindlessSpaceIndex",
        slang_compiler_sys::spReflection_getBindlessSpaceIndex as *const (),
    ),
    (
        "slang_createBlob",
        slang_compiler_sys::slang_createBlob as *const (),
    ),
    (
        "slang_writeCoverageManifestJson",
        slang_compiler_sys::slang_writeCoverageManifestJson as *const (),
    ),
    (
        "slang_loadModuleFromSource",
        slang_compiler_sys::slang_loadModuleFromSource as *const (),
    ),
    (
        "slang_loadModuleFromIRBlob",
        slang_compiler_sys::slang_loadModuleFromIRBlob as *const (),
    ),
    (
        "slang_loadModuleInfoFromIRBlob",
        slang_compiler_sys::slang_loadModuleInfoFromIRBlob as *const (),
    ),
    (
        "slang_createGlobalSession",
        slang_compiler_sys::slang_createGlobalSession as *const (),
    ),
    (
        "slang_createGlobalSession2",
        slang_compiler_sys::slang_createGlobalSession2 as *const (),
    ),
    (
        "slang_createGlobalSessionWithoutCoreModule",
        slang_compiler_sys::slang_createGlobalSessionWithoutCoreModule as *const (),
    ),
    (
        "slang_shutdown",
        slang_compiler_sys::slang_shutdown as *const (),
    ),
    (
        "slang_enableRecordLayer",
        slang_compiler_sys::slang_enableRecordLayer as *const (),
    ),
    (
        "slang_isRecordLayerEnabled",
        slang_compiler_sys::slang_isRecordLayerEnabled as *const (),
    ),
    (
        "slang_setReplayDirectory",
        slang_compiler_sys::slang_setReplayDirectory as *const (),
    ),
    (
        "slang_getReplayDirectory",
        slang_compiler_sys::slang_getReplayDirectory as *const (),
    ),
    (
        "slang_getCurrentReplayPath",
        slang_compiler_sys::slang_getCurrentReplayPath as *const (),
    ),
    (
        "slang_loadReplay",
        slang_compiler_sys::slang_loadReplay as *const (),
    ),
    (
        "slang_loadLatestReplay",
        slang_compiler_sys::slang_loadLatestReplay as *const (),
    ),
    (
        "slang_replayMarker",
        slang_compiler_sys::slang_replayMarker as *const (),
    ),
    (
        "slang_getLastInternalErrorMessage",
        slang_compiler_sys::slang_getLastInternalErrorMessage as *const (),
    ),
    (
        "slang_createByteCodeRunner",
        slang_compiler_sys::slang_createByteCodeRunner as *const (),
    ),
    (
        "slang_disassembleByteCode",
        slang_compiler_sys::slang_disassembleByteCode as *const (),
    ),
];
