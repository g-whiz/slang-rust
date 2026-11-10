//! Raw bindings to the Slang API, generated from the Slang headers by the binding generator. Run `just generate-bindings` to regenerate them instead of editing this file.

#[cfg(not(target_pointer_width = "64"))]
compile_error!(
    "the Slang bindings support only 64-bit targets: x86_64 Linux, x86_64 Windows with the Microsoft toolchain, and aarch64 and x86_64 macOS"
);

pub type SlangUInt32 = u32;

pub type SlangInt32 = i32;

pub type SlangInt = isize;

pub type SlangUInt = usize;

pub type SlangSSizeT = isize;

pub type SlangSizeT = usize;

pub type SlangBool = bool;

pub type SlangSeverityIntegral = ::core::ffi::c_int;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangSeverity(pub SlangSeverityIntegral);

impl SlangSeverity {
    pub const SLANG_SEVERITY_DISABLED: Self = Self(0);
    pub const SLANG_SEVERITY_NOTE: Self = Self(1);
    pub const SLANG_SEVERITY_WARNING: Self = Self(2);
    pub const SLANG_SEVERITY_ERROR: Self = Self(3);
    pub const SLANG_SEVERITY_FATAL: Self = Self(4);
    pub const SLANG_SEVERITY_INTERNAL: Self = Self(5);
}

pub type SlangWarningLevelIntegral = ::core::ffi::c_int;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangWarningLevel(pub SlangWarningLevelIntegral);

impl SlangWarningLevel {
    pub const SLANG_WARNING_LEVEL_DEFAULT: Self = Self(0);
    pub const SLANG_WARNING_LEVEL_ALL: Self = Self(1);
    pub const SLANG_WARNING_LEVEL_EXTRA: Self = Self(2);
    pub const SLANG_WARNING_LEVEL_PEDANTIC: Self = Self(3);
}

pub type SlangDiagnosticFlags = ::core::ffi::c_int;

pub const SLANG_DIAGNOSTIC_FLAG_VERBOSE_PATHS: SlangDiagnosticFlags = 1;

pub const SLANG_DIAGNOSTIC_FLAG_TREAT_WARNINGS_AS_ERRORS: SlangDiagnosticFlags = 2;

pub type SlangBindableResourceIntegral = ::core::ffi::c_int;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangBindableResourceType(pub SlangBindableResourceIntegral);

impl SlangBindableResourceType {
    pub const SLANG_NON_BINDABLE: Self = Self(0);
    pub const SLANG_TEXTURE: Self = Self(1);
    pub const SLANG_SAMPLER: Self = Self(2);
    pub const SLANG_UNIFORM_BUFFER: Self = Self(3);
    pub const SLANG_STORAGE_BUFFER: Self = Self(4);
}

pub type SlangCompileTargetIntegral = ::core::ffi::c_int;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangCompileTarget(pub SlangCompileTargetIntegral);

impl SlangCompileTarget {
    pub const SLANG_TARGET_UNKNOWN: Self = Self(0);
    pub const SLANG_TARGET_NONE: Self = Self(1);
    pub const SLANG_GLSL: Self = Self(2);
    pub const SLANG_GLSL_VULKAN_DEPRECATED: Self = Self(3);
    pub const SLANG_GLSL_VULKAN_ONE_DESC_DEPRECATED: Self = Self(4);
    pub const SLANG_HLSL: Self = Self(5);
    pub const SLANG_SPIRV: Self = Self(6);
    pub const SLANG_SPIRV_ASM: Self = Self(7);
    pub const SLANG_DXBC: Self = Self(8);
    pub const SLANG_DXBC_ASM: Self = Self(9);
    pub const SLANG_DXIL: Self = Self(10);
    pub const SLANG_DXIL_ASM: Self = Self(11);
    pub const SLANG_C_SOURCE: Self = Self(12);
    pub const SLANG_CPP_SOURCE: Self = Self(13);
    pub const SLANG_HOST_EXECUTABLE: Self = Self(14);
    pub const SLANG_SHADER_SHARED_LIBRARY: Self = Self(15);
    pub const SLANG_SHADER_HOST_CALLABLE: Self = Self(16);
    pub const SLANG_CUDA_SOURCE: Self = Self(17);
    pub const SLANG_PTX: Self = Self(18);
    pub const SLANG_CUDA_OBJECT_CODE: Self = Self(19);
    pub const SLANG_OBJECT_CODE: Self = Self(20);
    pub const SLANG_HOST_CPP_SOURCE: Self = Self(21);
    pub const SLANG_HOST_HOST_CALLABLE: Self = Self(22);
    pub const SLANG_CPP_PYTORCH_BINDING: Self = Self(23);
    pub const SLANG_METAL: Self = Self(24);
    pub const SLANG_METAL_LIB: Self = Self(25);
    pub const SLANG_METAL_LIB_ASM: Self = Self(26);
    pub const SLANG_HOST_SHARED_LIBRARY: Self = Self(27);
    pub const SLANG_WGSL: Self = Self(28);
    pub const SLANG_WGSL_SPIRV_ASM: Self = Self(29);
    pub const SLANG_WGSL_SPIRV: Self = Self(30);
    pub const SLANG_HOST_VM: Self = Self(31);
    pub const SLANG_CPP_HEADER: Self = Self(32);
    pub const SLANG_CUDA_HEADER: Self = Self(33);
    pub const SLANG_HOST_OBJECT_CODE: Self = Self(34);
    pub const SLANG_HOST_LLVM_IR: Self = Self(35);
    pub const SLANG_SHADER_LLVM_IR: Self = Self(36);
    pub const SLANG_TARGET_COUNT_OF: Self = Self(37);
}

pub type SlangContainerFormatIntegral = ::core::ffi::c_int;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangContainerFormat(pub SlangContainerFormatIntegral);

impl SlangContainerFormat {
    pub const SLANG_CONTAINER_FORMAT_NONE: Self = Self(0);
    pub const SLANG_CONTAINER_FORMAT_SLANG_MODULE: Self = Self(1);
}

pub type SlangPassThroughIntegral = ::core::ffi::c_int;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangPassThrough(pub SlangPassThroughIntegral);

impl SlangPassThrough {
    pub const SLANG_PASS_THROUGH_NONE: Self = Self(0);
    pub const SLANG_PASS_THROUGH_FXC: Self = Self(1);
    pub const SLANG_PASS_THROUGH_DXC: Self = Self(2);
    pub const SLANG_PASS_THROUGH_GLSLANG: Self = Self(3);
    pub const SLANG_PASS_THROUGH_SPIRV_DIS: Self = Self(4);
    pub const SLANG_PASS_THROUGH_CLANG: Self = Self(5);
    pub const SLANG_PASS_THROUGH_VISUAL_STUDIO: Self = Self(6);
    pub const SLANG_PASS_THROUGH_GCC: Self = Self(7);
    pub const SLANG_PASS_THROUGH_GENERIC_C_CPP: Self = Self(8);
    pub const SLANG_PASS_THROUGH_NVRTC: Self = Self(9);
    pub const SLANG_PASS_THROUGH_LLVM: Self = Self(10);
    pub const SLANG_PASS_THROUGH_SPIRV_OPT: Self = Self(11);
    pub const SLANG_PASS_THROUGH_METAL: Self = Self(12);
    pub const SLANG_PASS_THROUGH_TINT: Self = Self(13);
    pub const SLANG_PASS_THROUGH_SPIRV_LINK: Self = Self(14);
    pub const SLANG_PASS_THROUGH_COUNT_OF: Self = Self(15);
}

pub type SlangArchiveTypeIntegral = ::core::ffi::c_int;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangArchiveType(pub SlangArchiveTypeIntegral);

impl SlangArchiveType {
    pub const SLANG_ARCHIVE_TYPE_UNDEFINED: Self = Self(0);
    pub const SLANG_ARCHIVE_TYPE_ZIP: Self = Self(1);
    pub const SLANG_ARCHIVE_TYPE_RIFF: Self = Self(2);
    pub const SLANG_ARCHIVE_TYPE_RIFF_DEFLATE: Self = Self(3);
    pub const SLANG_ARCHIVE_TYPE_RIFF_LZ4: Self = Self(4);
    pub const SLANG_ARCHIVE_TYPE_COUNT_OF: Self = Self(5);
}

pub type SlangCompileFlags = ::core::ffi::c_uint;

pub const SLANG_COMPILE_FLAG_NO_MANGLING: SlangCompileFlags = 8;

pub const SLANG_COMPILE_FLAG_NO_CODEGEN: SlangCompileFlags = 16;

pub const SLANG_COMPILE_FLAG_OBFUSCATE: SlangCompileFlags = 32;

pub const SLANG_COMPILE_FLAG_NO_CHECKING: SlangCompileFlags = 0;

pub const SLANG_COMPILE_FLAG_SPLIT_MIXED_TYPES: SlangCompileFlags = 0;

pub type SlangTargetFlags = ::core::ffi::c_uint;

pub const SLANG_TARGET_FLAG_PARAMETER_BLOCKS_USE_REGISTER_SPACES: SlangTargetFlags = 16;

pub const SLANG_TARGET_FLAG_GENERATE_WHOLE_PROGRAM: SlangTargetFlags = 256;

pub const SLANG_TARGET_FLAG_DUMP_IR: SlangTargetFlags = 512;

pub const SLANG_TARGET_FLAG_GENERATE_SPIRV_DIRECTLY: SlangTargetFlags = 1024;

pub const kDefaultTargetFlags: SlangTargetFlags = 1024;

pub type SlangFloatingPointModeIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangFloatingPointMode(pub SlangFloatingPointModeIntegral);

impl SlangFloatingPointMode {
    pub const SLANG_FLOATING_POINT_MODE_DEFAULT: Self = Self(0);
    pub const SLANG_FLOATING_POINT_MODE_FAST: Self = Self(1);
    pub const SLANG_FLOATING_POINT_MODE_PRECISE: Self = Self(2);
}

pub type SlangFpDenormalModeIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangFpDenormalMode(pub SlangFpDenormalModeIntegral);

impl SlangFpDenormalMode {
    pub const SLANG_FP_DENORM_MODE_ANY: Self = Self(0);
    pub const SLANG_FP_DENORM_MODE_PRESERVE: Self = Self(1);
    pub const SLANG_FP_DENORM_MODE_FTZ: Self = Self(2);
}

pub type SlangLineDirectiveModeIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangLineDirectiveMode(pub SlangLineDirectiveModeIntegral);

impl SlangLineDirectiveMode {
    pub const SLANG_LINE_DIRECTIVE_MODE_DEFAULT: Self = Self(0);
    pub const SLANG_LINE_DIRECTIVE_MODE_NONE: Self = Self(1);
    pub const SLANG_LINE_DIRECTIVE_MODE_STANDARD: Self = Self(2);
    pub const SLANG_LINE_DIRECTIVE_MODE_GLSL: Self = Self(3);
    pub const SLANG_LINE_DIRECTIVE_MODE_SOURCE_MAP: Self = Self(4);
}

pub type SlangSourceLanguageIntegral = ::core::ffi::c_int;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangSourceLanguage(pub SlangSourceLanguageIntegral);

impl SlangSourceLanguage {
    pub const SLANG_SOURCE_LANGUAGE_UNKNOWN: Self = Self(0);
    pub const SLANG_SOURCE_LANGUAGE_SLANG: Self = Self(1);
    pub const SLANG_SOURCE_LANGUAGE_HLSL: Self = Self(2);
    pub const SLANG_SOURCE_LANGUAGE_GLSL: Self = Self(3);
    pub const SLANG_SOURCE_LANGUAGE_C: Self = Self(4);
    pub const SLANG_SOURCE_LANGUAGE_CPP: Self = Self(5);
    pub const SLANG_SOURCE_LANGUAGE_CUDA: Self = Self(6);
    pub const SLANG_SOURCE_LANGUAGE_SPIRV: Self = Self(7);
    pub const SLANG_SOURCE_LANGUAGE_METAL: Self = Self(8);
    pub const SLANG_SOURCE_LANGUAGE_WGSL: Self = Self(9);
    pub const SLANG_SOURCE_LANGUAGE_LLVM: Self = Self(10);
    pub const SLANG_SOURCE_LANGUAGE_COUNT_OF: Self = Self(11);
}

pub type SlangProfileIDIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangProfileID(pub SlangProfileIDIntegral);

impl SlangProfileID {
    pub const SLANG_PROFILE_UNKNOWN: Self = Self(0);
}

pub type SlangCapabilityIDIntegral = SlangInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangCapabilityID(pub SlangCapabilityIDIntegral);

impl SlangCapabilityID {
    pub const SLANG_CAPABILITY_UNKNOWN: Self = Self(0);
}

pub type SlangMatrixLayoutModeIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangMatrixLayoutMode(pub SlangMatrixLayoutModeIntegral);

impl SlangMatrixLayoutMode {
    pub const SLANG_MATRIX_LAYOUT_MODE_UNKNOWN: Self = Self(0);
    pub const SLANG_MATRIX_LAYOUT_ROW_MAJOR: Self = Self(1);
    pub const SLANG_MATRIX_LAYOUT_COLUMN_MAJOR: Self = Self(2);
}

pub type SlangStageIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangStage(pub SlangStageIntegral);

impl SlangStage {
    pub const SLANG_STAGE_NONE: Self = Self(0);
    pub const SLANG_STAGE_VERTEX: Self = Self(1);
    pub const SLANG_STAGE_HULL: Self = Self(2);
    pub const SLANG_STAGE_DOMAIN: Self = Self(3);
    pub const SLANG_STAGE_GEOMETRY: Self = Self(4);
    pub const SLANG_STAGE_FRAGMENT: Self = Self(5);
    pub const SLANG_STAGE_COMPUTE: Self = Self(6);
    pub const SLANG_STAGE_RAY_GENERATION: Self = Self(7);
    pub const SLANG_STAGE_INTERSECTION: Self = Self(8);
    pub const SLANG_STAGE_ANY_HIT: Self = Self(9);
    pub const SLANG_STAGE_CLOSEST_HIT: Self = Self(10);
    pub const SLANG_STAGE_MISS: Self = Self(11);
    pub const SLANG_STAGE_CALLABLE: Self = Self(12);
    pub const SLANG_STAGE_MESH: Self = Self(13);
    pub const SLANG_STAGE_AMPLIFICATION: Self = Self(14);
    pub const SLANG_STAGE_DISPATCH: Self = Self(15);
    pub const SLANG_STAGE_NODE: Self = Self(16);
    pub const SLANG_STAGE_COUNT: Self = Self(17);
    pub const SLANG_STAGE_PIXEL: Self = Self(5);
}

pub type SlangScopeIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangScope(pub SlangScopeIntegral);

impl SlangScope {
    pub const SLANG_SCOPE_NONE: Self = Self(0);
    pub const SLANG_SCOPE_THREAD: Self = Self(1);
    pub const SLANG_SCOPE_WAVE: Self = Self(2);
    pub const SLANG_SCOPE_THREAD_GROUP: Self = Self(3);
}

pub type SlangCooperativeMatrixUseIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangCooperativeMatrixUse(pub SlangCooperativeMatrixUseIntegral);

impl SlangCooperativeMatrixUse {
    pub const SLANG_COOPERATIVE_MATRIX_USE_A: Self = Self(0);
    pub const SLANG_COOPERATIVE_MATRIX_USE_B: Self = Self(1);
    pub const SLANG_COOPERATIVE_MATRIX_USE_ACCUMULATOR: Self = Self(2);
}

pub type SlangCooperativeVectorMatrixLayoutIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangCooperativeVectorMatrixLayout(pub SlangCooperativeVectorMatrixLayoutIntegral);

impl SlangCooperativeVectorMatrixLayout {
    pub const SLANG_COOPERATIVE_VECTOR_MATRIX_LAYOUT_ROW_MAJOR: Self = Self(0);
    pub const SLANG_COOPERATIVE_VECTOR_MATRIX_LAYOUT_COLUMN_MAJOR: Self = Self(1);
    pub const SLANG_COOPERATIVE_VECTOR_MATRIX_LAYOUT_INFERENCING_OPTIMAL: Self = Self(2);
    pub const SLANG_COOPERATIVE_VECTOR_MATRIX_LAYOUT_TRAINING_OPTIMAL: Self = Self(3);
}

pub type SlangDebugInfoLevelIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangDebugInfoLevel(pub SlangDebugInfoLevelIntegral);

impl SlangDebugInfoLevel {
    pub const SLANG_DEBUG_INFO_LEVEL_NONE: Self = Self(0);
    pub const SLANG_DEBUG_INFO_LEVEL_MINIMAL: Self = Self(1);
    pub const SLANG_DEBUG_INFO_LEVEL_STANDARD: Self = Self(2);
    pub const SLANG_DEBUG_INFO_LEVEL_MAXIMAL: Self = Self(3);
}

pub type SlangDebugInfoFormatIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangDebugInfoFormat(pub SlangDebugInfoFormatIntegral);

impl SlangDebugInfoFormat {
    pub const SLANG_DEBUG_INFO_FORMAT_DEFAULT: Self = Self(0);
    pub const SLANG_DEBUG_INFO_FORMAT_C7: Self = Self(1);
    pub const SLANG_DEBUG_INFO_FORMAT_PDB: Self = Self(2);
    pub const SLANG_DEBUG_INFO_FORMAT_STABS: Self = Self(3);
    pub const SLANG_DEBUG_INFO_FORMAT_COFF: Self = Self(4);
    pub const SLANG_DEBUG_INFO_FORMAT_DWARF: Self = Self(5);
    pub const SLANG_DEBUG_INFO_FORMAT_COUNT_OF: Self = Self(6);
}

pub type SlangOptimizationLevelIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangOptimizationLevel(pub SlangOptimizationLevelIntegral);

impl SlangOptimizationLevel {
    pub const SLANG_OPTIMIZATION_LEVEL_NONE: Self = Self(0);
    pub const SLANG_OPTIMIZATION_LEVEL_DEFAULT: Self = Self(1);
    pub const SLANG_OPTIMIZATION_LEVEL_HIGH: Self = Self(2);
    pub const SLANG_OPTIMIZATION_LEVEL_MAXIMAL: Self = Self(3);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangEmitSpirvMethod(pub u32);

impl SlangEmitSpirvMethod {
    pub const SLANG_EMIT_SPIRV_DEFAULT: Self = Self(0);
    pub const SLANG_EMIT_SPIRV_VIA_GLSL: Self = Self(1);
    pub const SLANG_EMIT_SPIRV_DIRECTLY: Self = Self(2);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangEmitCPUMethod(pub u32);

impl SlangEmitCPUMethod {
    pub const SLANG_EMIT_CPU_DEFAULT: Self = Self(0);
    pub const SLANG_EMIT_CPU_VIA_CPP: Self = Self(1);
    pub const SLANG_EMIT_CPU_VIA_LLVM: Self = Self(2);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangDiagnosticColor(pub u32);

impl SlangDiagnosticColor {
    pub const SLANG_DIAGNOSTIC_COLOR_AUTO: Self = Self(0);
    pub const SLANG_DIAGNOSTIC_COLOR_ALWAYS: Self = Self(1);
    pub const SLANG_DIAGNOSTIC_COLOR_NEVER: Self = Self(2);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompilerOptionName(pub ::core::ffi::c_int);

impl CompilerOptionName {
    pub const MacroDefine: Self = Self(0);
    pub const DepFile: Self = Self(1);
    pub const EntryPointName: Self = Self(2);
    pub const Specialize: Self = Self(3);
    pub const Help: Self = Self(4);
    pub const HelpStyle: Self = Self(5);
    pub const Include: Self = Self(6);
    pub const Language: Self = Self(7);
    pub const MatrixLayoutColumn: Self = Self(8);
    pub const MatrixLayoutRow: Self = Self(9);
    pub const ZeroInitialize: Self = Self(10);
    pub const IgnoreCapabilities: Self = Self(11);
    pub const RestrictiveCapabilityCheck: Self = Self(12);
    pub const ModuleName: Self = Self(13);
    pub const Output: Self = Self(14);
    pub const Profile: Self = Self(15);
    pub const Stage: Self = Self(16);
    pub const Target: Self = Self(17);
    pub const Version: Self = Self(18);
    pub const WarningsAsErrors: Self = Self(19);
    pub const DisableWarnings: Self = Self(20);
    pub const EnableWarning: Self = Self(21);
    pub const DisableWarning: Self = Self(22);
    pub const DumpWarningDiagnostics: Self = Self(23);
    pub const InputFilesRemain: Self = Self(24);
    pub const EmitIr: Self = Self(25);
    pub const ReportDownstreamTime: Self = Self(26);
    pub const ReportPerfBenchmark: Self = Self(27);
    pub const ReportCheckpointIntermediates: Self = Self(28);
    pub const SkipSPIRVValidation: Self = Self(29);
    pub const SourceEmbedStyle: Self = Self(30);
    pub const SourceEmbedName: Self = Self(31);
    pub const SourceEmbedLanguage: Self = Self(32);
    pub const DisableShortCircuit: Self = Self(33);
    pub const MinimumSlangOptimization: Self = Self(34);
    pub const DisableNonEssentialValidations: Self = Self(35);
    pub const DisableSourceMap: Self = Self(36);
    pub const UnscopedEnum: Self = Self(37);
    pub const PreserveParameters: Self = Self(38);
    pub const Capability: Self = Self(39);
    pub const DefaultImageFormatUnknown: Self = Self(40);
    pub const DisableDynamicDispatch: Self = Self(41);
    pub const DisableSpecialization: Self = Self(42);
    pub const FloatingPointMode: Self = Self(43);
    pub const DebugInformation: Self = Self(44);
    pub const LineDirectiveMode: Self = Self(45);
    pub const Optimization: Self = Self(46);
    pub const Obfuscate: Self = Self(47);
    pub const VulkanBindShift: Self = Self(48);
    pub const VulkanBindGlobals: Self = Self(49);
    pub const VulkanInvertY: Self = Self(50);
    pub const VulkanUseDxPositionW: Self = Self(51);
    pub const VulkanUseEntryPointName: Self = Self(52);
    pub const VulkanUseGLLayout: Self = Self(53);
    pub const VulkanEmitReflection: Self = Self(54);
    pub const GLSLForceScalarLayout: Self = Self(55);
    pub const EnableEffectAnnotations: Self = Self(56);
    pub const EmitSpirvViaGLSL: Self = Self(57);
    pub const EmitSpirvDirectly: Self = Self(58);
    pub const SPIRVCoreGrammarJSON: Self = Self(59);
    pub const IncompleteLibrary: Self = Self(60);
    pub const CompilerPath: Self = Self(61);
    pub const DefaultDownstreamCompiler: Self = Self(62);
    pub const DownstreamArgs: Self = Self(63);
    pub const PassThrough: Self = Self(64);
    pub const DumpRepro: Self = Self(65);
    pub const DumpReproOnError: Self = Self(66);
    pub const ExtractRepro: Self = Self(67);
    pub const LoadRepro: Self = Self(68);
    pub const LoadReproDirectory: Self = Self(69);
    pub const ReproFallbackDirectory: Self = Self(70);
    pub const DumpAst: Self = Self(71);
    pub const DumpIntermediatePrefix: Self = Self(72);
    pub const DumpIntermediates: Self = Self(73);
    pub const DumpIr: Self = Self(74);
    pub const DumpIrIds: Self = Self(75);
    pub const PreprocessorOutput: Self = Self(76);
    pub const OutputIncludes: Self = Self(77);
    pub const ReproFileSystem: Self = Self(78);
    pub const REMOVED_SerialIR: Self = Self(79);
    pub const SkipCodeGen: Self = Self(80);
    pub const ValidateIr: Self = Self(81);
    pub const VerbosePaths: Self = Self(82);
    pub const VerifyDebugSerialIr: Self = Self(83);
    pub const NoCodeGen: Self = Self(84);
    pub const FileSystem: Self = Self(85);
    pub const Heterogeneous: Self = Self(86);
    pub const NoMangle: Self = Self(87);
    pub const NoHLSLBinding: Self = Self(88);
    pub const NoHLSLPackConstantBufferElements: Self = Self(89);
    pub const ValidateUniformity: Self = Self(90);
    pub const AllowGLSL: Self = Self(91);
    pub const EnableExperimentalPasses: Self = Self(92);
    pub const BindlessSpaceIndex: Self = Self(93);
    pub const SPIRVResourceHeapStride: Self = Self(94);
    pub const SPIRVSamplerHeapStride: Self = Self(95);
    pub const ArchiveType: Self = Self(96);
    pub const CompileCoreModule: Self = Self(97);
    pub const Doc: Self = Self(98);
    pub const IrCompression: Self = Self(99);
    pub const LoadCoreModule: Self = Self(100);
    pub const ReferenceModule: Self = Self(101);
    pub const SaveCoreModule: Self = Self(102);
    pub const SaveCoreModuleBinSource: Self = Self(103);
    pub const TrackLiveness: Self = Self(104);
    pub const LoopInversion: Self = Self(105);
    pub const ParameterBlocksUseRegisterSpaces: Self = Self(106);
    pub const LanguageVersion: Self = Self(107);
    pub const TypeConformance: Self = Self(108);
    pub const EnableExperimentalDynamicDispatch: Self = Self(109);
    pub const EmitReflectionJSON: Self = Self(110);
    pub const CountOfParsableOptions: Self = Self(111);
    pub const DebugInformationFormat: Self = Self(112);
    pub const VulkanBindShiftAll: Self = Self(113);
    pub const GenerateWholeProgram: Self = Self(114);
    pub const UseUpToDateBinaryModule: Self = Self(115);
    pub const EmbedDownstreamIR: Self = Self(116);
    pub const ForceDXLayout: Self = Self(117);
    pub const EmitSpirvMethod: Self = Self(118);
    pub const SaveGLSLModuleBinSource: Self = Self(119);
    pub const SkipDownstreamLinking: Self = Self(120);
    pub const DumpModule: Self = Self(121);
    pub const GetModuleInfo: Self = Self(122);
    pub const GetSupportedModuleVersions: Self = Self(123);
    pub const EmitSeparateDebug: Self = Self(124);
    pub const DenormalModeFp16: Self = Self(125);
    pub const DenormalModeFp32: Self = Self(126);
    pub const DenormalModeFp64: Self = Self(127);
    pub const UseMSVCStyleBitfieldPacking: Self = Self(128);
    pub const ForceCLayout: Self = Self(129);
    pub const ExperimentalFeature: Self = Self(130);
    pub const ReportDetailedPerfBenchmark: Self = Self(131);
    pub const ValidateIRDetailed: Self = Self(132);
    pub const DumpIRBefore: Self = Self(133);
    pub const DumpIRAfter: Self = Self(134);
    pub const EmitCPUMethod: Self = Self(135);
    pub const EmitCPUViaCPP: Self = Self(136);
    pub const EmitCPUViaLLVM: Self = Self(137);
    pub const LLVMTargetTriple: Self = Self(138);
    pub const LLVMCPU: Self = Self(139);
    pub const LLVMFeatures: Self = Self(140);
    pub const EnableRichDiagnostics: Self = Self(141);
    pub const ReportDynamicDispatchSites: Self = Self(142);
    pub const EnableMachineReadableDiagnostics: Self = Self(143);
    pub const DiagnosticColor: Self = Self(144);
    pub const TraceCoverage: Self = Self(145);
    pub const TraceCoverageBinding: Self = Self(146);
    pub const TraceCoverageReservedSpace: Self = Self(147);
    pub const TraceFunctionCoverage: Self = Self(148);
    pub const TraceBranchCoverage: Self = Self(149);
    pub const CoverageManifestOutput: Self = Self(150);
    pub const TraceCoverageCounterByteWidth: Self = Self(151);
    pub const TraceCoverageBoolean: Self = Self(152);
    pub const SPIRVUnifiedDescriptorHeapStride: Self = Self(154);
    pub const WarningLevel: Self = Self(155);
    pub const SeparateDebugInfoOutput: Self = Self(156);
    pub const DebugInfoIncludeSource: Self = Self(157);
    pub const TraceCoverageBindlessIndex: Self = Self(158);
    pub const GetCompilerPath: Self = Self(159);
    pub const CountOf: Self = Self(160);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompilerOptionValueKind(pub ::core::ffi::c_int);

impl CompilerOptionValueKind {
    pub const Int: Self = Self(0);
    pub const String: Self = Self(1);
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CompilerOptionValue {
    pub kind: CompilerOptionValueKind,
    pub intValue0: i32,
    pub intValue1: i32,
    pub stringValue0: *const ::core::ffi::c_char,
    pub stringValue1: *const ::core::ffi::c_char,
}

impl Default for CompilerOptionValue {
    fn default() -> Self {
        Self {
            kind: CompilerOptionValueKind(0),
            intValue0: 0,
            intValue1: 0,
            stringValue0: ::core::ptr::null(),
            stringValue1: ::core::ptr::null(),
        }
    }
}

const _: () = {
    assert!(size_of::<CompilerOptionValue>() == 32);
    assert!(align_of::<CompilerOptionValue>() == 8);
    assert!(::core::mem::offset_of!(CompilerOptionValue, kind) == 0);
    assert!(::core::mem::offset_of!(CompilerOptionValue, intValue0) == 4);
    assert!(::core::mem::offset_of!(CompilerOptionValue, intValue1) == 8);
    assert!(::core::mem::offset_of!(CompilerOptionValue, stringValue0) == 16);
    assert!(::core::mem::offset_of!(CompilerOptionValue, stringValue1) == 24);
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CompilerOptionEntry {
    pub name: CompilerOptionName,
    pub value: CompilerOptionValue,
}

const _: () = {
    assert!(size_of::<CompilerOptionEntry>() == 40);
    assert!(align_of::<CompilerOptionEntry>() == 8);
    assert!(::core::mem::offset_of!(CompilerOptionEntry, name) == 0);
    assert!(::core::mem::offset_of!(CompilerOptionEntry, value) == 8);
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangResult(pub i32);

impl SlangResult {
    pub const SLANG_OK: Self = Self(0);
    pub const SLANG_FAIL: Self = Self(-2_147_467_259);
    pub const SLANG_E_NOT_IMPLEMENTED: Self = Self(-2_147_467_263);
    pub const SLANG_E_NO_INTERFACE: Self = Self(-2_147_467_262);
    pub const SLANG_E_ABORT: Self = Self(-2_147_467_260);
    pub const SLANG_E_INVALID_HANDLE: Self = Self(-2_147_024_890);
    pub const SLANG_E_INVALID_ARG: Self = Self(-2_147_024_809);
    pub const SLANG_E_OUT_OF_MEMORY: Self = Self(-2_147_024_882);
    pub const SLANG_E_BUFFER_TOO_SMALL: Self = Self(-2_113_929_215);
    pub const SLANG_E_UNINITIALIZED: Self = Self(-2_113_929_214);
    pub const SLANG_E_PENDING: Self = Self(-2_113_929_213);
    pub const SLANG_E_CANNOT_OPEN: Self = Self(-2_113_929_212);
    pub const SLANG_E_NOT_FOUND: Self = Self(-2_113_929_211);
    pub const SLANG_E_INTERNAL_FAIL: Self = Self(-2_113_929_210);
    pub const SLANG_E_NOT_AVAILABLE: Self = Self(-2_113_929_209);
    pub const SLANG_E_TIME_OUT: Self = Self(-2_113_929_208);
    pub const SLANG_ERROR_INSUFFICIENT_BUFFER: Self = Self(-2_113_929_215);
    pub const SLANG_ERROR_INVALID_PARAMETER: Self = Self(-2_147_024_809);
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SlangUUID {
    pub data1: u32,
    pub data2: u16,
    pub data3: u16,
    pub data4: [u8; 8],
}

const _: () = {
    assert!(size_of::<SlangUUID>() == 16);
    assert!(align_of::<SlangUUID>() == 4);
    assert!(::core::mem::offset_of!(SlangUUID, data1) == 0);
    assert!(::core::mem::offset_of!(SlangUUID, data2) == 4);
    assert!(::core::mem::offset_of!(SlangUUID, data3) == 6);
    assert!(::core::mem::offset_of!(SlangUUID, data4) == 8);
};

pub unsafe trait Interface {
    const UUID: SlangUUID;
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangUnknown {
    pub vtable: *const ISlangUnknownVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangUnknownVtbl {
    pub queryInterface: unsafe extern "system" fn(
        this: *mut ISlangUnknown,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult,
    pub addRef: unsafe extern "system" fn(this: *mut ISlangUnknown) -> u32,
    pub release: unsafe extern "system" fn(this: *mut ISlangUnknown) -> u32,
}

unsafe impl Interface for ISlangUnknown {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x0000_0000,
        data2: 0x0000,
        data3: 0x0000,
        data4: [0xC0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x46],
    };
}

impl ISlangUnknown {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).queryInterface)(this, uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).addRef)(this) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).release)(this) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangCastable {
    pub vtable: *const ISlangCastableVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangCastableVtbl {
    pub base: ISlangUnknownVtbl,
    pub castAs: unsafe extern "system" fn(
        this: *mut ISlangCastable,
        guid: *const SlangUUID,
    ) -> *mut ::core::ffi::c_void,
}

unsafe impl Interface for ISlangCastable {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x87ED_E0E1,
        data2: 0x4852,
        data3: 0x44B0,
        data4: [0x8B, 0xF2, 0xCB, 0x31, 0x87, 0x4D, 0xE2, 0x39],
    };
}

impl ISlangCastable {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).castAs)(this, guid) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangClonable {
    pub vtable: *const ISlangClonableVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangClonableVtbl {
    pub base: ISlangCastableVtbl,
    pub clone: unsafe extern "system" fn(
        this: *mut ISlangClonable,
        guid: *const SlangUUID,
    ) -> *mut ::core::ffi::c_void,
}

unsafe impl Interface for ISlangClonable {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x1EC3_6168,
        data2: 0xE9F4,
        data3: 0x430D,
        data4: [0xBB, 0x17, 0x04, 0x8A, 0x80, 0x46, 0xB3, 0x1F],
    };
}

impl ISlangClonable {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn clone(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).clone)(this, guid) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangBlob {
    pub vtable: *const ISlangBlobVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangBlobVtbl {
    pub base: ISlangUnknownVtbl,
    pub getBufferPointer:
        unsafe extern "system" fn(this: *mut ISlangBlob) -> *const ::core::ffi::c_void,
    pub getBufferSize: unsafe extern "system" fn(this: *mut ISlangBlob) -> usize,
}

unsafe impl Interface for ISlangBlob {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x8BA5_FB08,
        data2: 0x5195,
        data3: 0x40E2,
        data4: [0xAC, 0x58, 0x0D, 0x98, 0x9C, 0x3A, 0x01, 0x02],
    };
}

impl ISlangBlob {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn getBufferPointer(this: *mut Self) -> *const ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).getBufferPointer)(this) }
    }

    pub unsafe fn getBufferSize(this: *mut Self) -> usize {
        unsafe { ((*(*this).vtable).getBufferSize)(this) }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SlangTerminatedChars {
    pub chars: [::core::ffi::c_char; 1],
}

const _: () = {
    assert!(size_of::<SlangTerminatedChars>() == 1);
    assert!(align_of::<SlangTerminatedChars>() == 1);
    assert!(::core::mem::offset_of!(SlangTerminatedChars, chars) == 0);
};

#[repr(C)]
#[derive(Debug)]
pub struct ISlangFileSystem {
    pub vtable: *const ISlangFileSystemVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangFileSystemVtbl {
    pub base: ISlangCastableVtbl,
    pub loadFile: unsafe extern "system" fn(
        this: *mut ISlangFileSystem,
        path: *const ::core::ffi::c_char,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult,
}

unsafe impl Interface for ISlangFileSystem {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x003A_09FC,
        data2: 0x3A4D,
        data3: 0x4BA0,
        data4: [0xAD, 0x60, 0x1F, 0xD8, 0x63, 0xA9, 0x15, 0xAB],
    };
}

impl ISlangFileSystem {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn loadFile(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).loadFile)(this, path, outBlob) }
    }
}

pub type SlangFuncPtr = Option<unsafe extern "C" fn()>;

#[repr(C)]
#[derive(Debug)]
pub struct ISlangSharedLibrary_Dep1 {
    pub vtable: *const ISlangSharedLibrary_Dep1Vtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangSharedLibrary_Dep1Vtbl {
    pub base: ISlangUnknownVtbl,
    pub findSymbolAddressByName: unsafe extern "system" fn(
        this: *mut ISlangSharedLibrary_Dep1,
        name: *const ::core::ffi::c_char,
    ) -> *mut ::core::ffi::c_void,
}

unsafe impl Interface for ISlangSharedLibrary_Dep1 {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x9C9D_5BC5,
        data2: 0xEB61,
        data3: 0x496F,
        data4: [0x80, 0xD7, 0xD1, 0x47, 0xC4, 0xA2, 0x37, 0x30],
    };
}

impl ISlangSharedLibrary_Dep1 {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn findSymbolAddressByName(
        this: *mut Self,
        name: *const ::core::ffi::c_char,
    ) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).findSymbolAddressByName)(this, name) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangSharedLibrary {
    pub vtable: *const ISlangSharedLibraryVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangSharedLibraryVtbl {
    pub base: ISlangCastableVtbl,
    pub findSymbolAddressByName: unsafe extern "system" fn(
        this: *mut ISlangSharedLibrary,
        name: *const ::core::ffi::c_char,
    ) -> *mut ::core::ffi::c_void,
}

unsafe impl Interface for ISlangSharedLibrary {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x70DB_C7C4,
        data2: 0xDC3B,
        data3: 0x4A07,
        data4: [0xAE, 0x7E, 0x75, 0x2A, 0xF6, 0xA8, 0x15, 0x55],
    };
}

impl ISlangSharedLibrary {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn findSymbolAddressByName(
        this: *mut Self,
        name: *const ::core::ffi::c_char,
    ) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).findSymbolAddressByName)(this, name) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangSharedLibraryLoader {
    pub vtable: *const ISlangSharedLibraryLoaderVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangSharedLibraryLoaderVtbl {
    pub base: ISlangUnknownVtbl,
    pub loadSharedLibrary: unsafe extern "system" fn(
        this: *mut ISlangSharedLibraryLoader,
        path: *const ::core::ffi::c_char,
        sharedLibraryOut: *mut *mut ISlangSharedLibrary,
    ) -> SlangResult,
}

unsafe impl Interface for ISlangSharedLibraryLoader {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x6264_AB2B,
        data2: 0xA3E8,
        data3: 0x4A06,
        data4: [0x97, 0xF1, 0x49, 0xBC, 0x2D, 0x2A, 0xB1, 0x4D],
    };
}

impl ISlangSharedLibraryLoader {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn loadSharedLibrary(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        sharedLibraryOut: *mut *mut ISlangSharedLibrary,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).loadSharedLibrary)(this, path, sharedLibraryOut) }
    }
}

pub type SlangPathTypeIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangPathType(pub SlangPathTypeIntegral);

impl SlangPathType {
    pub const SLANG_PATH_TYPE_DIRECTORY: Self = Self(0);
    pub const SLANG_PATH_TYPE_FILE: Self = Self(1);
}

pub type FileSystemContentsCallBack = Option<
    unsafe extern "C" fn(SlangPathType, *const ::core::ffi::c_char, *mut ::core::ffi::c_void),
>;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OSPathKind(pub u8);

impl OSPathKind {
    pub const None: Self = Self(0);
    pub const Direct: Self = Self(1);
    pub const OperatingSystem: Self = Self(2);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PathKind(pub ::core::ffi::c_int);

impl PathKind {
    pub const Simplified: Self = Self(0);
    pub const Canonical: Self = Self(1);
    pub const Display: Self = Self(2);
    pub const OperatingSystem: Self = Self(3);
    pub const CountOf: Self = Self(4);
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangFileSystemExt {
    pub vtable: *const ISlangFileSystemExtVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangFileSystemExtVtbl {
    pub base: ISlangFileSystemVtbl,
    pub getFileUniqueIdentity: unsafe extern "system" fn(
        this: *mut ISlangFileSystemExt,
        path: *const ::core::ffi::c_char,
        outUniqueIdentity: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub calcCombinedPath: unsafe extern "system" fn(
        this: *mut ISlangFileSystemExt,
        fromPathType: SlangPathType,
        fromPath: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        pathOut: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub getPathType: unsafe extern "system" fn(
        this: *mut ISlangFileSystemExt,
        path: *const ::core::ffi::c_char,
        pathTypeOut: *mut SlangPathType,
    ) -> SlangResult,
    pub getPath: unsafe extern "system" fn(
        this: *mut ISlangFileSystemExt,
        kind: PathKind,
        path: *const ::core::ffi::c_char,
        outPath: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub clearCache: unsafe extern "system" fn(this: *mut ISlangFileSystemExt),
    pub enumeratePathContents: unsafe extern "system" fn(
        this: *mut ISlangFileSystemExt,
        path: *const ::core::ffi::c_char,
        callback: FileSystemContentsCallBack,
        userData: *mut ::core::ffi::c_void,
    ) -> SlangResult,
    pub getOSPathKind: unsafe extern "system" fn(this: *mut ISlangFileSystemExt) -> OSPathKind,
}

unsafe impl Interface for ISlangFileSystemExt {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x5FB6_32D2,
        data2: 0x979D,
        data3: 0x4481,
        data4: [0x9F, 0xEE, 0x66, 0x3C, 0x3F, 0x14, 0x49, 0xE1],
    };
}

impl ISlangFileSystemExt {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn loadFile(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.loadFile)(this.cast(), path, outBlob) }
    }

    pub unsafe fn getFileUniqueIdentity(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        outUniqueIdentity: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getFileUniqueIdentity)(this, path, outUniqueIdentity) }
    }

    pub unsafe fn calcCombinedPath(
        this: *mut Self,
        fromPathType: SlangPathType,
        fromPath: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        pathOut: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).calcCombinedPath)(this, fromPathType, fromPath, path, pathOut) }
    }

    pub unsafe fn getPathType(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        pathTypeOut: *mut SlangPathType,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getPathType)(this, path, pathTypeOut) }
    }

    pub unsafe fn getPath(
        this: *mut Self,
        kind: PathKind,
        path: *const ::core::ffi::c_char,
        outPath: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getPath)(this, kind, path, outPath) }
    }

    pub unsafe fn clearCache(this: *mut Self) {
        unsafe {
            ((*(*this).vtable).clearCache)(this);
        }
    }

    pub unsafe fn enumeratePathContents(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        callback: FileSystemContentsCallBack,
        userData: *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).enumeratePathContents)(this, path, callback, userData) }
    }

    pub unsafe fn getOSPathKind(this: *mut Self) -> OSPathKind {
        unsafe { ((*(*this).vtable).getOSPathKind)(this) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangMutableFileSystem {
    pub vtable: *const ISlangMutableFileSystemVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangMutableFileSystemVtbl {
    pub base: ISlangFileSystemExtVtbl,
    pub saveFile: unsafe extern "system" fn(
        this: *mut ISlangMutableFileSystem,
        path: *const ::core::ffi::c_char,
        data: *const ::core::ffi::c_void,
        size: usize,
    ) -> SlangResult,
    pub saveFileBlob: unsafe extern "system" fn(
        this: *mut ISlangMutableFileSystem,
        path: *const ::core::ffi::c_char,
        dataBlob: *mut ISlangBlob,
    ) -> SlangResult,
    pub remove: unsafe extern "system" fn(
        this: *mut ISlangMutableFileSystem,
        path: *const ::core::ffi::c_char,
    ) -> SlangResult,
    pub createDirectory: unsafe extern "system" fn(
        this: *mut ISlangMutableFileSystem,
        path: *const ::core::ffi::c_char,
    ) -> SlangResult,
}

unsafe impl Interface for ISlangMutableFileSystem {
    const UUID: SlangUUID = SlangUUID {
        data1: 0xA058_675C,
        data2: 0x1D65,
        data3: 0x452A,
        data4: [0x84, 0x58, 0xCC, 0xDE, 0xD1, 0x42, 0x71, 0x05],
    };
}

impl ISlangMutableFileSystem {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.base.base.base.queryInterface)(this.cast(), uuid, outObject)
        }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.base.base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn loadFile(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.loadFile)(this.cast(), path, outBlob) }
    }

    pub unsafe fn getFileUniqueIdentity(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        outUniqueIdentity: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getFileUniqueIdentity)(this.cast(), path, outUniqueIdentity)
        }
    }

    pub unsafe fn calcCombinedPath(
        this: *mut Self,
        fromPathType: SlangPathType,
        fromPath: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        pathOut: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.calcCombinedPath)(
                this.cast(),
                fromPathType,
                fromPath,
                path,
                pathOut,
            )
        }
    }

    pub unsafe fn getPathType(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        pathTypeOut: *mut SlangPathType,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.getPathType)(this.cast(), path, pathTypeOut) }
    }

    pub unsafe fn getPath(
        this: *mut Self,
        kind: PathKind,
        path: *const ::core::ffi::c_char,
        outPath: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.getPath)(this.cast(), kind, path, outPath) }
    }

    pub unsafe fn clearCache(this: *mut Self) {
        unsafe {
            ((*(*this).vtable).base.clearCache)(this.cast());
        }
    }

    pub unsafe fn enumeratePathContents(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        callback: FileSystemContentsCallBack,
        userData: *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.enumeratePathContents)(this.cast(), path, callback, userData)
        }
    }

    pub unsafe fn getOSPathKind(this: *mut Self) -> OSPathKind {
        unsafe { ((*(*this).vtable).base.getOSPathKind)(this.cast()) }
    }

    pub unsafe fn saveFile(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        data: *const ::core::ffi::c_void,
        size: usize,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).saveFile)(this, path, data, size) }
    }

    pub unsafe fn saveFileBlob(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
        dataBlob: *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).saveFileBlob)(this, path, dataBlob) }
    }

    pub unsafe fn remove(this: *mut Self, path: *const ::core::ffi::c_char) -> SlangResult {
        unsafe { ((*(*this).vtable).remove)(this, path) }
    }

    pub unsafe fn createDirectory(
        this: *mut Self,
        path: *const ::core::ffi::c_char,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).createDirectory)(this, path) }
    }
}

pub type SlangWriterChannelIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangWriterChannel(pub SlangWriterChannelIntegral);

impl SlangWriterChannel {
    pub const SLANG_WRITER_CHANNEL_DIAGNOSTIC: Self = Self(0);
    pub const SLANG_WRITER_CHANNEL_STD_OUTPUT: Self = Self(1);
    pub const SLANG_WRITER_CHANNEL_STD_ERROR: Self = Self(2);
    pub const SLANG_WRITER_CHANNEL_COUNT_OF: Self = Self(3);
}

pub type SlangWriterModeIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangWriterMode(pub SlangWriterModeIntegral);

impl SlangWriterMode {
    pub const SLANG_WRITER_MODE_TEXT: Self = Self(0);
    pub const SLANG_WRITER_MODE_BINARY: Self = Self(1);
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangWriter {
    pub vtable: *const ISlangWriterVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangWriterVtbl {
    pub base: ISlangUnknownVtbl,
    pub beginAppendBuffer: unsafe extern "system" fn(
        this: *mut ISlangWriter,
        maxNumChars: usize,
    ) -> *mut ::core::ffi::c_char,
    pub endAppendBuffer: unsafe extern "system" fn(
        this: *mut ISlangWriter,
        buffer: *mut ::core::ffi::c_char,
        numChars: usize,
    ) -> SlangResult,
    pub write: unsafe extern "system" fn(
        this: *mut ISlangWriter,
        chars: *const ::core::ffi::c_char,
        numChars: usize,
    ) -> SlangResult,
    pub flush: unsafe extern "system" fn(this: *mut ISlangWriter),
    pub isConsole: unsafe extern "system" fn(this: *mut ISlangWriter) -> SlangBool,
    pub setMode:
        unsafe extern "system" fn(this: *mut ISlangWriter, mode: SlangWriterMode) -> SlangResult,
}

unsafe impl Interface for ISlangWriter {
    const UUID: SlangUUID = SlangUUID {
        data1: 0xEC45_7F0E,
        data2: 0x9ADD,
        data3: 0x4E6B,
        data4: [0x85, 0x1C, 0xD7, 0xFA, 0x71, 0x6D, 0x15, 0xFD],
    };
}

impl ISlangWriter {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn beginAppendBuffer(
        this: *mut Self,
        maxNumChars: usize,
    ) -> *mut ::core::ffi::c_char {
        unsafe { ((*(*this).vtable).beginAppendBuffer)(this, maxNumChars) }
    }

    pub unsafe fn endAppendBuffer(
        this: *mut Self,
        buffer: *mut ::core::ffi::c_char,
        numChars: usize,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).endAppendBuffer)(this, buffer, numChars) }
    }

    pub unsafe fn write(
        this: *mut Self,
        chars: *const ::core::ffi::c_char,
        numChars: usize,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).write)(this, chars, numChars) }
    }

    pub unsafe fn flush(this: *mut Self) {
        unsafe {
            ((*(*this).vtable).flush)(this);
        }
    }

    pub unsafe fn isConsole(this: *mut Self) -> SlangBool {
        unsafe { ((*(*this).vtable).isConsole)(this) }
    }

    pub unsafe fn setMode(this: *mut Self, mode: SlangWriterMode) -> SlangResult {
        unsafe { ((*(*this).vtable).setMode)(this, mode) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangProfiler {
    pub vtable: *const ISlangProfilerVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISlangProfilerVtbl {
    pub base: ISlangUnknownVtbl,
    pub getEntryCount: unsafe extern "system" fn(this: *mut ISlangProfiler) -> usize,
    pub getEntryName: unsafe extern "system" fn(
        this: *mut ISlangProfiler,
        index: u32,
    ) -> *const ::core::ffi::c_char,
    pub getEntryTimeMS:
        unsafe extern "system" fn(this: *mut ISlangProfiler, index: u32) -> ::core::ffi::c_long,
    pub getEntryInvocationTimes:
        unsafe extern "system" fn(this: *mut ISlangProfiler, index: u32) -> u32,
}

unsafe impl Interface for ISlangProfiler {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x1977_72C7,
        data2: 0x0155,
        data3: 0x4B91,
        data4: [0x84, 0xE8, 0x66, 0x68, 0xBA, 0xFF, 0x06, 0x19],
    };
}

impl ISlangProfiler {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn getEntryCount(this: *mut Self) -> usize {
        unsafe { ((*(*this).vtable).getEntryCount)(this) }
    }

    pub unsafe fn getEntryName(this: *mut Self, index: u32) -> *const ::core::ffi::c_char {
        unsafe { ((*(*this).vtable).getEntryName)(this, index) }
    }

    pub unsafe fn getEntryTimeMS(this: *mut Self, index: u32) -> ::core::ffi::c_long {
        unsafe { ((*(*this).vtable).getEntryTimeMS)(this, index) }
    }

    pub unsafe fn getEntryInvocationTimes(this: *mut Self, index: u32) -> u32 {
        unsafe { ((*(*this).vtable).getEntryInvocationTimes)(this, index) }
    }
}

pub type SlangSession = IGlobalSession;

#[repr(C)]
#[derive(Debug)]
pub struct SlangProgramLayout {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

pub type SlangCompileRequest = ICompileRequest;

pub type SlangDiagnosticCallback =
    Option<unsafe extern "C" fn(*const ::core::ffi::c_char, *mut ::core::ffi::c_void)>;

#[repr(C)]
#[derive(Debug)]
pub struct SlangEntryPoint {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangEntryPointLayout {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionDecl {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionModifier {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionType {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionTypeLayout {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionVariable {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionVariableLayout {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionTypeParameter {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionUserAttribute {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

pub type SlangReflectionAttribute = SlangReflectionUserAttribute;

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionFunction {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct SlangReflectionGeneric {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union SlangReflectionGenericArg {
    pub typeVal: *mut SlangReflectionType,
    pub intVal: i64,
    pub boolVal: bool,
}

impl ::core::fmt::Debug for SlangReflectionGenericArg {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("SlangReflectionGenericArg")
            .finish_non_exhaustive()
    }
}

const _: () = {
    assert!(size_of::<SlangReflectionGenericArg>() == 8);
    assert!(align_of::<SlangReflectionGenericArg>() == 8);
    assert!(::core::mem::offset_of!(SlangReflectionGenericArg, typeVal) == 0);
    assert!(::core::mem::offset_of!(SlangReflectionGenericArg, intVal) == 0);
    assert!(::core::mem::offset_of!(SlangReflectionGenericArg, boolVal) == 0);
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangReflectionGenericArgType(pub u32);

impl SlangReflectionGenericArgType {
    pub const SLANG_GENERIC_ARG_TYPE: Self = Self(0);
    pub const SLANG_GENERIC_ARG_INT: Self = Self(1);
    pub const SLANG_GENERIC_ARG_BOOL: Self = Self(2);
}

pub type SlangReflection = SlangProgramLayout;

pub type SlangReflectionEntryPoint = SlangEntryPointLayout;

pub type SlangTypeKindIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangTypeKind(pub SlangTypeKindIntegral);

impl SlangTypeKind {
    pub const SLANG_TYPE_KIND_NONE: Self = Self(0);
    pub const SLANG_TYPE_KIND_STRUCT: Self = Self(1);
    pub const SLANG_TYPE_KIND_ARRAY: Self = Self(2);
    pub const SLANG_TYPE_KIND_MATRIX: Self = Self(3);
    pub const SLANG_TYPE_KIND_VECTOR: Self = Self(4);
    pub const SLANG_TYPE_KIND_SCALAR: Self = Self(5);
    pub const SLANG_TYPE_KIND_CONSTANT_BUFFER: Self = Self(6);
    pub const SLANG_TYPE_KIND_RESOURCE: Self = Self(7);
    pub const SLANG_TYPE_KIND_SAMPLER_STATE: Self = Self(8);
    pub const SLANG_TYPE_KIND_TEXTURE_BUFFER: Self = Self(9);
    pub const SLANG_TYPE_KIND_SHADER_STORAGE_BUFFER: Self = Self(10);
    pub const SLANG_TYPE_KIND_PARAMETER_BLOCK: Self = Self(11);
    pub const SLANG_TYPE_KIND_GENERIC_TYPE_PARAMETER: Self = Self(12);
    pub const SLANG_TYPE_KIND_INTERFACE: Self = Self(13);
    pub const SLANG_TYPE_KIND_OUTPUT_STREAM: Self = Self(14);
    pub const SLANG_TYPE_KIND_MESH_OUTPUT: Self = Self(15);
    pub const SLANG_TYPE_KIND_SPECIALIZED: Self = Self(16);
    pub const SLANG_TYPE_KIND_FEEDBACK: Self = Self(17);
    pub const SLANG_TYPE_KIND_POINTER: Self = Self(18);
    pub const SLANG_TYPE_KIND_DYNAMIC_RESOURCE: Self = Self(19);
    pub const SLANG_TYPE_KIND_ENUM: Self = Self(20);
    pub const SLANG_TYPE_KIND_COUNT: Self = Self(21);
}

pub type SlangScalarTypeIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangScalarType(pub SlangScalarTypeIntegral);

impl SlangScalarType {
    pub const SLANG_SCALAR_TYPE_NONE: Self = Self(0);
    pub const SLANG_SCALAR_TYPE_VOID: Self = Self(1);
    pub const SLANG_SCALAR_TYPE_BOOL: Self = Self(2);
    pub const SLANG_SCALAR_TYPE_INT32: Self = Self(3);
    pub const SLANG_SCALAR_TYPE_UINT32: Self = Self(4);
    pub const SLANG_SCALAR_TYPE_INT64: Self = Self(5);
    pub const SLANG_SCALAR_TYPE_UINT64: Self = Self(6);
    pub const SLANG_SCALAR_TYPE_FLOAT16: Self = Self(7);
    pub const SLANG_SCALAR_TYPE_FLOAT32: Self = Self(8);
    pub const SLANG_SCALAR_TYPE_FLOAT64: Self = Self(9);
    pub const SLANG_SCALAR_TYPE_INT8: Self = Self(10);
    pub const SLANG_SCALAR_TYPE_UINT8: Self = Self(11);
    pub const SLANG_SCALAR_TYPE_INT16: Self = Self(12);
    pub const SLANG_SCALAR_TYPE_UINT16: Self = Self(13);
    pub const SLANG_SCALAR_TYPE_INTPTR: Self = Self(14);
    pub const SLANG_SCALAR_TYPE_UINTPTR: Self = Self(15);
    pub const SLANG_SCALAR_TYPE_BFLOAT16: Self = Self(16);
    pub const SLANG_SCALAR_TYPE_FLOAT_E4M3: Self = Self(17);
    pub const SLANG_SCALAR_TYPE_FLOAT_E5M2: Self = Self(18);
}

pub type SlangDeclKindIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangDeclKind(pub SlangDeclKindIntegral);

impl SlangDeclKind {
    pub const SLANG_DECL_KIND_UNSUPPORTED_FOR_REFLECTION: Self = Self(0);
    pub const SLANG_DECL_KIND_STRUCT: Self = Self(1);
    pub const SLANG_DECL_KIND_FUNC: Self = Self(2);
    pub const SLANG_DECL_KIND_MODULE: Self = Self(3);
    pub const SLANG_DECL_KIND_GENERIC: Self = Self(4);
    pub const SLANG_DECL_KIND_VARIABLE: Self = Self(5);
    pub const SLANG_DECL_KIND_NAMESPACE: Self = Self(6);
    pub const SLANG_DECL_KIND_ENUM: Self = Self(7);
}

pub type SlangResourceShapeIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangResourceShape(pub SlangResourceShapeIntegral);

impl SlangResourceShape {
    pub const SLANG_RESOURCE_BASE_SHAPE_MASK: Self = Self(15);
    pub const SLANG_RESOURCE_NONE: Self = Self(0);
    pub const SLANG_TEXTURE_1D: Self = Self(1);
    pub const SLANG_TEXTURE_2D: Self = Self(2);
    pub const SLANG_TEXTURE_3D: Self = Self(3);
    pub const SLANG_TEXTURE_CUBE: Self = Self(4);
    pub const SLANG_TEXTURE_BUFFER: Self = Self(5);
    pub const SLANG_STRUCTURED_BUFFER: Self = Self(6);
    pub const SLANG_BYTE_ADDRESS_BUFFER: Self = Self(7);
    pub const SLANG_RESOURCE_UNKNOWN: Self = Self(8);
    pub const SLANG_ACCELERATION_STRUCTURE: Self = Self(9);
    pub const SLANG_TEXTURE_SUBPASS: Self = Self(10);
    pub const SLANG_RESOURCE_EXT_SHAPE_MASK: Self = Self(496);
    pub const SLANG_TEXTURE_FEEDBACK_FLAG: Self = Self(16);
    pub const SLANG_TEXTURE_SHADOW_FLAG: Self = Self(32);
    pub const SLANG_TEXTURE_ARRAY_FLAG: Self = Self(64);
    pub const SLANG_TEXTURE_MULTISAMPLE_FLAG: Self = Self(128);
    pub const SLANG_TEXTURE_COMBINED_FLAG: Self = Self(256);
    pub const SLANG_TEXTURE_1D_ARRAY: Self = Self(65);
    pub const SLANG_TEXTURE_2D_ARRAY: Self = Self(66);
    pub const SLANG_TEXTURE_CUBE_ARRAY: Self = Self(68);
    pub const SLANG_TEXTURE_2D_MULTISAMPLE: Self = Self(130);
    pub const SLANG_TEXTURE_2D_MULTISAMPLE_ARRAY: Self = Self(194);
    pub const SLANG_TEXTURE_SUBPASS_MULTISAMPLE: Self = Self(138);
}

pub type SlangResourceAccessIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangResourceAccess(pub SlangResourceAccessIntegral);

impl SlangResourceAccess {
    pub const SLANG_RESOURCE_ACCESS_NONE: Self = Self(0);
    pub const SLANG_RESOURCE_ACCESS_READ: Self = Self(1);
    pub const SLANG_RESOURCE_ACCESS_READ_WRITE: Self = Self(2);
    pub const SLANG_RESOURCE_ACCESS_RASTER_ORDERED: Self = Self(3);
    pub const SLANG_RESOURCE_ACCESS_APPEND: Self = Self(4);
    pub const SLANG_RESOURCE_ACCESS_CONSUME: Self = Self(5);
    pub const SLANG_RESOURCE_ACCESS_WRITE: Self = Self(6);
    pub const SLANG_RESOURCE_ACCESS_FEEDBACK: Self = Self(7);
    pub const SLANG_RESOURCE_ACCESS_UNKNOWN: Self = Self(2_147_483_647);
}

pub type SlangParameterCategoryIntegral = ::core::ffi::c_uint;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangParameterCategory(pub SlangParameterCategoryIntegral);

impl SlangParameterCategory {
    pub const SLANG_PARAMETER_CATEGORY_NONE: Self = Self(0);
    pub const SLANG_PARAMETER_CATEGORY_MIXED: Self = Self(1);
    pub const SLANG_PARAMETER_CATEGORY_CONSTANT_BUFFER: Self = Self(2);
    pub const SLANG_PARAMETER_CATEGORY_SHADER_RESOURCE: Self = Self(3);
    pub const SLANG_PARAMETER_CATEGORY_UNORDERED_ACCESS: Self = Self(4);
    pub const SLANG_PARAMETER_CATEGORY_VARYING_INPUT: Self = Self(5);
    pub const SLANG_PARAMETER_CATEGORY_VARYING_OUTPUT: Self = Self(6);
    pub const SLANG_PARAMETER_CATEGORY_SAMPLER_STATE: Self = Self(7);
    pub const SLANG_PARAMETER_CATEGORY_UNIFORM: Self = Self(8);
    pub const SLANG_PARAMETER_CATEGORY_DESCRIPTOR_TABLE_SLOT: Self = Self(9);
    pub const SLANG_PARAMETER_CATEGORY_SPECIALIZATION_CONSTANT: Self = Self(10);
    pub const SLANG_PARAMETER_CATEGORY_PUSH_CONSTANT_BUFFER: Self = Self(11);
    pub const SLANG_PARAMETER_CATEGORY_REGISTER_SPACE: Self = Self(12);
    pub const SLANG_PARAMETER_CATEGORY_GENERIC: Self = Self(13);
    pub const SLANG_PARAMETER_CATEGORY_RAY_PAYLOAD: Self = Self(14);
    pub const SLANG_PARAMETER_CATEGORY_HIT_ATTRIBUTES: Self = Self(15);
    pub const SLANG_PARAMETER_CATEGORY_CALLABLE_PAYLOAD: Self = Self(16);
    pub const SLANG_PARAMETER_CATEGORY_SHADER_RECORD: Self = Self(17);
    pub const SLANG_PARAMETER_CATEGORY_EXISTENTIAL_TYPE_PARAM: Self = Self(18);
    pub const SLANG_PARAMETER_CATEGORY_EXISTENTIAL_OBJECT_PARAM: Self = Self(19);
    pub const SLANG_PARAMETER_CATEGORY_SUB_ELEMENT_REGISTER_SPACE: Self = Self(20);
    pub const SLANG_PARAMETER_CATEGORY_SUBPASS: Self = Self(21);
    pub const SLANG_PARAMETER_CATEGORY_METAL_ARGUMENT_BUFFER_ELEMENT: Self = Self(22);
    pub const SLANG_PARAMETER_CATEGORY_METAL_ATTRIBUTE: Self = Self(23);
    pub const SLANG_PARAMETER_CATEGORY_METAL_PAYLOAD: Self = Self(24);
    pub const SLANG_PARAMETER_CATEGORY_COUNT: Self = Self(25);
    pub const SLANG_PARAMETER_CATEGORY_METAL_BUFFER: Self = Self(2);
    pub const SLANG_PARAMETER_CATEGORY_METAL_TEXTURE: Self = Self(3);
    pub const SLANG_PARAMETER_CATEGORY_METAL_SAMPLER: Self = Self(7);
    pub const SLANG_PARAMETER_CATEGORY_VERTEX_INPUT: Self = Self(5);
    pub const SLANG_PARAMETER_CATEGORY_FRAGMENT_OUTPUT: Self = Self(6);
    pub const SLANG_PARAMETER_CATEGORY_COUNT_V1: Self = Self(21);
}

pub type SlangBindingTypeIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangBindingType(pub SlangBindingTypeIntegral);

impl SlangBindingType {
    pub const SLANG_BINDING_TYPE_UNKNOWN: Self = Self(0);
    pub const SLANG_BINDING_TYPE_SAMPLER: Self = Self(1);
    pub const SLANG_BINDING_TYPE_TEXTURE: Self = Self(2);
    pub const SLANG_BINDING_TYPE_CONSTANT_BUFFER: Self = Self(3);
    pub const SLANG_BINDING_TYPE_PARAMETER_BLOCK: Self = Self(4);
    pub const SLANG_BINDING_TYPE_TYPED_BUFFER: Self = Self(5);
    pub const SLANG_BINDING_TYPE_RAW_BUFFER: Self = Self(6);
    pub const SLANG_BINDING_TYPE_COMBINED_TEXTURE_SAMPLER: Self = Self(7);
    pub const SLANG_BINDING_TYPE_INPUT_RENDER_TARGET: Self = Self(8);
    pub const SLANG_BINDING_TYPE_INLINE_UNIFORM_DATA: Self = Self(9);
    pub const SLANG_BINDING_TYPE_RAY_TRACING_ACCELERATION_STRUCTURE: Self = Self(10);
    pub const SLANG_BINDING_TYPE_VARYING_INPUT: Self = Self(11);
    pub const SLANG_BINDING_TYPE_VARYING_OUTPUT: Self = Self(12);
    pub const SLANG_BINDING_TYPE_EXISTENTIAL_VALUE: Self = Self(13);
    pub const SLANG_BINDING_TYPE_PUSH_CONSTANT: Self = Self(14);
    pub const SLANG_BINDING_TYPE_MUTABLE_FLAG: Self = Self(256);
    pub const SLANG_BINDING_TYPE_MUTABLE_TETURE: Self = Self(258);
    pub const SLANG_BINDING_TYPE_MUTABLE_TYPED_BUFFER: Self = Self(261);
    pub const SLANG_BINDING_TYPE_MUTABLE_RAW_BUFFER: Self = Self(262);
    pub const SLANG_BINDING_TYPE_BASE_MASK: Self = Self(255);
    pub const SLANG_BINDING_TYPE_EXT_MASK: Self = Self(65_280);
}

pub type SlangLayoutRulesIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangLayoutRules(pub SlangLayoutRulesIntegral);

impl SlangLayoutRules {
    pub const SLANG_LAYOUT_RULES_DEFAULT: Self = Self(0);
    pub const SLANG_LAYOUT_RULES_METAL_ARGUMENT_BUFFER_TIER_2: Self = Self(1);
    pub const SLANG_LAYOUT_RULES_DEFAULT_STRUCTURED_BUFFER: Self = Self(2);
    pub const SLANG_LAYOUT_RULES_DEFAULT_CONSTANT_BUFFER: Self = Self(3);
}

pub type SlangModifierIDIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangModifierID(pub SlangModifierIDIntegral);

impl SlangModifierID {
    pub const SLANG_MODIFIER_SHARED: Self = Self(0);
    pub const SLANG_MODIFIER_NO_DIFF: Self = Self(1);
    pub const SLANG_MODIFIER_STATIC: Self = Self(2);
    pub const SLANG_MODIFIER_CONST: Self = Self(3);
    pub const SLANG_MODIFIER_EXPORT: Self = Self(4);
    pub const SLANG_MODIFIER_EXTERN: Self = Self(5);
    pub const SLANG_MODIFIER_DIFFERENTIABLE: Self = Self(6);
    pub const SLANG_MODIFIER_MUTATING: Self = Self(7);
    pub const SLANG_MODIFIER_IN: Self = Self(8);
    pub const SLANG_MODIFIER_OUT: Self = Self(9);
    pub const SLANG_MODIFIER_INOUT: Self = Self(10);
}

pub type SlangImageFormatIntegral = SlangUInt32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangImageFormat(pub SlangImageFormatIntegral);

impl SlangImageFormat {
    pub const SLANG_IMAGE_FORMAT_unknown: Self = Self(0);
    pub const SLANG_IMAGE_FORMAT_rgba32f: Self = Self(1);
    pub const SLANG_IMAGE_FORMAT_rgba16f: Self = Self(2);
    pub const SLANG_IMAGE_FORMAT_rg32f: Self = Self(3);
    pub const SLANG_IMAGE_FORMAT_rg16f: Self = Self(4);
    pub const SLANG_IMAGE_FORMAT_r11f_g11f_b10f: Self = Self(5);
    pub const SLANG_IMAGE_FORMAT_r32f: Self = Self(6);
    pub const SLANG_IMAGE_FORMAT_r16f: Self = Self(7);
    pub const SLANG_IMAGE_FORMAT_rgba16: Self = Self(8);
    pub const SLANG_IMAGE_FORMAT_rgb10_a2: Self = Self(9);
    pub const SLANG_IMAGE_FORMAT_rgba8: Self = Self(10);
    pub const SLANG_IMAGE_FORMAT_rg16: Self = Self(11);
    pub const SLANG_IMAGE_FORMAT_rg8: Self = Self(12);
    pub const SLANG_IMAGE_FORMAT_r16: Self = Self(13);
    pub const SLANG_IMAGE_FORMAT_r8: Self = Self(14);
    pub const SLANG_IMAGE_FORMAT_rgba16_snorm: Self = Self(15);
    pub const SLANG_IMAGE_FORMAT_rgba8_snorm: Self = Self(16);
    pub const SLANG_IMAGE_FORMAT_rg16_snorm: Self = Self(17);
    pub const SLANG_IMAGE_FORMAT_rg8_snorm: Self = Self(18);
    pub const SLANG_IMAGE_FORMAT_r16_snorm: Self = Self(19);
    pub const SLANG_IMAGE_FORMAT_r8_snorm: Self = Self(20);
    pub const SLANG_IMAGE_FORMAT_rgba32i: Self = Self(21);
    pub const SLANG_IMAGE_FORMAT_rgba16i: Self = Self(22);
    pub const SLANG_IMAGE_FORMAT_rgba8i: Self = Self(23);
    pub const SLANG_IMAGE_FORMAT_rg32i: Self = Self(24);
    pub const SLANG_IMAGE_FORMAT_rg16i: Self = Self(25);
    pub const SLANG_IMAGE_FORMAT_rg8i: Self = Self(26);
    pub const SLANG_IMAGE_FORMAT_r32i: Self = Self(27);
    pub const SLANG_IMAGE_FORMAT_r16i: Self = Self(28);
    pub const SLANG_IMAGE_FORMAT_r8i: Self = Self(29);
    pub const SLANG_IMAGE_FORMAT_rgba32ui: Self = Self(30);
    pub const SLANG_IMAGE_FORMAT_rgba16ui: Self = Self(31);
    pub const SLANG_IMAGE_FORMAT_rgb10_a2ui: Self = Self(32);
    pub const SLANG_IMAGE_FORMAT_rgba8ui: Self = Self(33);
    pub const SLANG_IMAGE_FORMAT_rg32ui: Self = Self(34);
    pub const SLANG_IMAGE_FORMAT_rg16ui: Self = Self(35);
    pub const SLANG_IMAGE_FORMAT_rg8ui: Self = Self(36);
    pub const SLANG_IMAGE_FORMAT_r32ui: Self = Self(37);
    pub const SLANG_IMAGE_FORMAT_r16ui: Self = Self(38);
    pub const SLANG_IMAGE_FORMAT_r8ui: Self = Self(39);
    pub const SLANG_IMAGE_FORMAT_r64ui: Self = Self(40);
    pub const SLANG_IMAGE_FORMAT_r64i: Self = Self(41);
    pub const SLANG_IMAGE_FORMAT_bgra8: Self = Self(42);
}

pub type SlangReflectionParameter = SlangReflectionVariableLayout;

#[repr(C)]
#[derive(Debug)]
pub struct BufferReflection {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union GenericArgReflection {
    pub typeVal: *mut TypeReflection,
    pub intVal: i64,
    pub boolVal: bool,
}

impl ::core::fmt::Debug for GenericArgReflection {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("GenericArgReflection")
            .finish_non_exhaustive()
    }
}

const _: () = {
    assert!(size_of::<GenericArgReflection>() == 8);
    assert!(align_of::<GenericArgReflection>() == 8);
    assert!(::core::mem::offset_of!(GenericArgReflection, typeVal) == 0);
    assert!(::core::mem::offset_of!(GenericArgReflection, intVal) == 0);
    assert!(::core::mem::offset_of!(GenericArgReflection, boolVal) == 0);
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ParameterCategory(pub SlangParameterCategoryIntegral);

impl ParameterCategory {
    pub const None: Self = Self(0);
    pub const Mixed: Self = Self(1);
    pub const ConstantBuffer: Self = Self(2);
    pub const ShaderResource: Self = Self(3);
    pub const UnorderedAccess: Self = Self(4);
    pub const VaryingInput: Self = Self(5);
    pub const VaryingOutput: Self = Self(6);
    pub const SamplerState: Self = Self(7);
    pub const Uniform: Self = Self(8);
    pub const DescriptorTableSlot: Self = Self(9);
    pub const SpecializationConstant: Self = Self(10);
    pub const PushConstantBuffer: Self = Self(11);
    pub const RegisterSpace: Self = Self(12);
    pub const GenericResource: Self = Self(13);
    pub const RayPayload: Self = Self(14);
    pub const HitAttributes: Self = Self(15);
    pub const CallablePayload: Self = Self(16);
    pub const ShaderRecord: Self = Self(17);
    pub const ExistentialTypeParam: Self = Self(18);
    pub const ExistentialObjectParam: Self = Self(19);
    pub const SubElementRegisterSpace: Self = Self(20);
    pub const InputAttachmentIndex: Self = Self(21);
    pub const MetalBuffer: Self = Self(2);
    pub const MetalTexture: Self = Self(3);
    pub const MetalArgumentBufferElement: Self = Self(22);
    pub const MetalAttribute: Self = Self(23);
    pub const MetalPayload: Self = Self(24);
    pub const VertexInput: Self = Self(5);
    pub const FragmentOutput: Self = Self(6);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BindingType(pub SlangBindingTypeIntegral);

impl BindingType {
    pub const Unknown: Self = Self(0);
    pub const Sampler: Self = Self(1);
    pub const Texture: Self = Self(2);
    pub const ConstantBuffer: Self = Self(3);
    pub const ParameterBlock: Self = Self(4);
    pub const TypedBuffer: Self = Self(5);
    pub const RawBuffer: Self = Self(6);
    pub const CombinedTextureSampler: Self = Self(7);
    pub const InputRenderTarget: Self = Self(8);
    pub const InlineUniformData: Self = Self(9);
    pub const RayTracingAccelerationStructure: Self = Self(10);
    pub const VaryingInput: Self = Self(11);
    pub const VaryingOutput: Self = Self(12);
    pub const ExistentialValue: Self = Self(13);
    pub const PushConstant: Self = Self(14);
    pub const MutableFlag: Self = Self(256);
    pub const MutableTexture: Self = Self(258);
    pub const MutableTypedBuffer: Self = Self(261);
    pub const MutableRawBuffer: Self = Self(262);
    pub const BaseMask: Self = Self(255);
    pub const ExtMask: Self = Self(65_280);
}

pub type EntryPointLayout = EntryPointReflection;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct LayoutRules(pub SlangLayoutRulesIntegral);

impl LayoutRules {
    pub const Default: Self = Self(0);
    pub const MetalArgumentBufferTier2: Self = Self(1);
    pub const DefaultStructuredBuffer: Self = Self(2);
    pub const DefaultConstantBuffer: Self = Self(3);
}

pub type ProgramLayout = ShaderReflection;

pub type GenericArgType = SlangReflectionGenericArgType;

pub type CompileCoreModuleFlags = u32;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CompileCoreModuleFlag_Enum(pub CompileCoreModuleFlags);

impl CompileCoreModuleFlag_Enum {
    pub const WriteDocumentation: Self = Self(1);
}

pub type IBlob = ISlangBlob;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BuiltinModuleName(pub ::core::ffi::c_int);

impl BuiltinModuleName {
    pub const Core: Self = Self(0);
    pub const GLSL: Self = Self(1);
}

#[repr(C)]
#[derive(Debug)]
pub struct IGlobalSession {
    pub vtable: *const IGlobalSessionVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IGlobalSessionVtbl {
    pub base: ISlangUnknownVtbl,
    pub createSession: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        desc: *const SessionDesc,
        outSession: *mut *mut ISession,
    ) -> SlangResult,
    pub findProfile: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        name: *const ::core::ffi::c_char,
    ) -> SlangProfileID,
    pub setDownstreamCompilerPath: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        passThrough: SlangPassThrough,
        path: *const ::core::ffi::c_char,
    ),
    pub setDownstreamCompilerPrelude: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        passThrough: SlangPassThrough,
        preludeText: *const ::core::ffi::c_char,
    ),
    pub getDownstreamCompilerPrelude: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        passThrough: SlangPassThrough,
        outPrelude: *mut *mut ISlangBlob,
    ),
    pub getBuildTagString:
        unsafe extern "system" fn(this: *mut IGlobalSession) -> *const ::core::ffi::c_char,
    pub setDefaultDownstreamCompiler: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        sourceLanguage: SlangSourceLanguage,
        defaultCompiler: SlangPassThrough,
    ) -> SlangResult,
    pub getDefaultDownstreamCompiler: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        sourceLanguage: SlangSourceLanguage,
    ) -> SlangPassThrough,
    pub setLanguagePrelude: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        sourceLanguage: SlangSourceLanguage,
        preludeText: *const ::core::ffi::c_char,
    ),
    pub getLanguagePrelude: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        sourceLanguage: SlangSourceLanguage,
        outPrelude: *mut *mut ISlangBlob,
    ),
    pub createCompileRequest: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        outCompileRequest: *mut *mut ICompileRequest,
    ) -> SlangResult,
    pub addBuiltins: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        sourcePath: *const ::core::ffi::c_char,
        sourceString: *const ::core::ffi::c_char,
    ),
    pub setSharedLibraryLoader: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        loader: *mut ISlangSharedLibraryLoader,
    ),
    pub getSharedLibraryLoader:
        unsafe extern "system" fn(this: *mut IGlobalSession) -> *mut ISlangSharedLibraryLoader,
    pub checkCompileTargetSupport: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        target: SlangCompileTarget,
    ) -> SlangResult,
    pub checkPassThroughSupport: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        passThrough: SlangPassThrough,
    ) -> SlangResult,
    pub compileCoreModule: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        flags: CompileCoreModuleFlags,
    ) -> SlangResult,
    pub loadCoreModule: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        coreModule: *const ::core::ffi::c_void,
        coreModuleSizeInBytes: usize,
    ) -> SlangResult,
    pub saveCoreModule: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        archiveType: SlangArchiveType,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub findCapability: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        name: *const ::core::ffi::c_char,
    ) -> SlangCapabilityID,
    pub setDownstreamCompilerForTransition: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        source: SlangCompileTarget,
        target: SlangCompileTarget,
        compiler: SlangPassThrough,
    ),
    pub getDownstreamCompilerForTransition: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        source: SlangCompileTarget,
        target: SlangCompileTarget,
    ) -> SlangPassThrough,
    pub getCompilerElapsedTime: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        outTotalTime: *mut f64,
        outDownstreamTime: *mut f64,
    ),
    pub setSPIRVCoreGrammar: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        jsonPath: *const ::core::ffi::c_char,
    ) -> SlangResult,
    pub parseCommandLineArguments: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        argc: ::core::ffi::c_int,
        argv: *const *const ::core::ffi::c_char,
        outSessionDesc: *mut SessionDesc,
        outAuxAllocation: *mut *mut ISlangUnknown,
    ) -> SlangResult,
    pub getSessionDescDigest: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        sessionDesc: *mut SessionDesc,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub compileBuiltinModule: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        module: BuiltinModuleName,
        flags: CompileCoreModuleFlags,
    ) -> SlangResult,
    pub loadBuiltinModule: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        module: BuiltinModuleName,
        moduleData: *const ::core::ffi::c_void,
        sizeInBytes: usize,
    ) -> SlangResult,
    pub saveBuiltinModule: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        module: BuiltinModuleName,
        archiveType: SlangArchiveType,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub getDownstreamCompilerPath: unsafe extern "system" fn(
        this: *mut IGlobalSession,
        passThrough: SlangPassThrough,
        outPath: *mut *mut ISlangBlob,
    ) -> SlangResult,
}

unsafe impl Interface for IGlobalSession {
    const UUID: SlangUUID = SlangUUID {
        data1: 0xC140_B5FD,
        data2: 0x0C78,
        data3: 0x452E,
        data4: [0xBA, 0x7C, 0x1A, 0x1E, 0x70, 0xC7, 0xF7, 0x1C],
    };
}

impl IGlobalSession {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn createSession(
        this: *mut Self,
        desc: *const SessionDesc,
        outSession: *mut *mut ISession,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).createSession)(this, desc, outSession) }
    }

    pub unsafe fn findProfile(this: *mut Self, name: *const ::core::ffi::c_char) -> SlangProfileID {
        unsafe { ((*(*this).vtable).findProfile)(this, name) }
    }

    pub unsafe fn setDownstreamCompilerPath(
        this: *mut Self,
        passThrough: SlangPassThrough,
        path: *const ::core::ffi::c_char,
    ) {
        unsafe {
            ((*(*this).vtable).setDownstreamCompilerPath)(this, passThrough, path);
        }
    }

    pub unsafe fn setDownstreamCompilerPrelude(
        this: *mut Self,
        passThrough: SlangPassThrough,
        preludeText: *const ::core::ffi::c_char,
    ) {
        unsafe {
            ((*(*this).vtable).setDownstreamCompilerPrelude)(this, passThrough, preludeText);
        }
    }

    pub unsafe fn getDownstreamCompilerPrelude(
        this: *mut Self,
        passThrough: SlangPassThrough,
        outPrelude: *mut *mut ISlangBlob,
    ) {
        unsafe {
            ((*(*this).vtable).getDownstreamCompilerPrelude)(this, passThrough, outPrelude);
        }
    }

    pub unsafe fn getBuildTagString(this: *mut Self) -> *const ::core::ffi::c_char {
        unsafe { ((*(*this).vtable).getBuildTagString)(this) }
    }

    pub unsafe fn setDefaultDownstreamCompiler(
        this: *mut Self,
        sourceLanguage: SlangSourceLanguage,
        defaultCompiler: SlangPassThrough,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).setDefaultDownstreamCompiler)(this, sourceLanguage, defaultCompiler)
        }
    }

    pub unsafe fn getDefaultDownstreamCompiler(
        this: *mut Self,
        sourceLanguage: SlangSourceLanguage,
    ) -> SlangPassThrough {
        unsafe { ((*(*this).vtable).getDefaultDownstreamCompiler)(this, sourceLanguage) }
    }

    pub unsafe fn setLanguagePrelude(
        this: *mut Self,
        sourceLanguage: SlangSourceLanguage,
        preludeText: *const ::core::ffi::c_char,
    ) {
        unsafe {
            ((*(*this).vtable).setLanguagePrelude)(this, sourceLanguage, preludeText);
        }
    }

    pub unsafe fn getLanguagePrelude(
        this: *mut Self,
        sourceLanguage: SlangSourceLanguage,
        outPrelude: *mut *mut ISlangBlob,
    ) {
        unsafe {
            ((*(*this).vtable).getLanguagePrelude)(this, sourceLanguage, outPrelude);
        }
    }

    pub unsafe fn createCompileRequest(
        this: *mut Self,
        outCompileRequest: *mut *mut ICompileRequest,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).createCompileRequest)(this, outCompileRequest) }
    }

    pub unsafe fn addBuiltins(
        this: *mut Self,
        sourcePath: *const ::core::ffi::c_char,
        sourceString: *const ::core::ffi::c_char,
    ) {
        unsafe {
            ((*(*this).vtable).addBuiltins)(this, sourcePath, sourceString);
        }
    }

    pub unsafe fn setSharedLibraryLoader(this: *mut Self, loader: *mut ISlangSharedLibraryLoader) {
        unsafe {
            ((*(*this).vtable).setSharedLibraryLoader)(this, loader);
        }
    }

    pub unsafe fn getSharedLibraryLoader(this: *mut Self) -> *mut ISlangSharedLibraryLoader {
        unsafe { ((*(*this).vtable).getSharedLibraryLoader)(this) }
    }

    pub unsafe fn checkCompileTargetSupport(
        this: *mut Self,
        target: SlangCompileTarget,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).checkCompileTargetSupport)(this, target) }
    }

    pub unsafe fn checkPassThroughSupport(
        this: *mut Self,
        passThrough: SlangPassThrough,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).checkPassThroughSupport)(this, passThrough) }
    }

    pub unsafe fn compileCoreModule(this: *mut Self, flags: CompileCoreModuleFlags) -> SlangResult {
        unsafe { ((*(*this).vtable).compileCoreModule)(this, flags) }
    }

    pub unsafe fn loadCoreModule(
        this: *mut Self,
        coreModule: *const ::core::ffi::c_void,
        coreModuleSizeInBytes: usize,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).loadCoreModule)(this, coreModule, coreModuleSizeInBytes) }
    }

    pub unsafe fn saveCoreModule(
        this: *mut Self,
        archiveType: SlangArchiveType,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).saveCoreModule)(this, archiveType, outBlob) }
    }

    pub unsafe fn findCapability(
        this: *mut Self,
        name: *const ::core::ffi::c_char,
    ) -> SlangCapabilityID {
        unsafe { ((*(*this).vtable).findCapability)(this, name) }
    }

    pub unsafe fn setDownstreamCompilerForTransition(
        this: *mut Self,
        source: SlangCompileTarget,
        target: SlangCompileTarget,
        compiler: SlangPassThrough,
    ) {
        unsafe {
            ((*(*this).vtable).setDownstreamCompilerForTransition)(this, source, target, compiler);
        }
    }

    pub unsafe fn getDownstreamCompilerForTransition(
        this: *mut Self,
        source: SlangCompileTarget,
        target: SlangCompileTarget,
    ) -> SlangPassThrough {
        unsafe { ((*(*this).vtable).getDownstreamCompilerForTransition)(this, source, target) }
    }

    pub unsafe fn getCompilerElapsedTime(
        this: *mut Self,
        outTotalTime: *mut f64,
        outDownstreamTime: *mut f64,
    ) {
        unsafe {
            ((*(*this).vtable).getCompilerElapsedTime)(this, outTotalTime, outDownstreamTime);
        }
    }

    pub unsafe fn setSPIRVCoreGrammar(
        this: *mut Self,
        jsonPath: *const ::core::ffi::c_char,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).setSPIRVCoreGrammar)(this, jsonPath) }
    }

    pub unsafe fn parseCommandLineArguments(
        this: *mut Self,
        argc: ::core::ffi::c_int,
        argv: *const *const ::core::ffi::c_char,
        outSessionDesc: *mut SessionDesc,
        outAuxAllocation: *mut *mut ISlangUnknown,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).parseCommandLineArguments)(
                this,
                argc,
                argv,
                outSessionDesc,
                outAuxAllocation,
            )
        }
    }

    pub unsafe fn getSessionDescDigest(
        this: *mut Self,
        sessionDesc: *mut SessionDesc,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getSessionDescDigest)(this, sessionDesc, outBlob) }
    }

    pub unsafe fn compileBuiltinModule(
        this: *mut Self,
        module: BuiltinModuleName,
        flags: CompileCoreModuleFlags,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).compileBuiltinModule)(this, module, flags) }
    }

    pub unsafe fn loadBuiltinModule(
        this: *mut Self,
        module: BuiltinModuleName,
        moduleData: *const ::core::ffi::c_void,
        sizeInBytes: usize,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).loadBuiltinModule)(this, module, moduleData, sizeInBytes) }
    }

    pub unsafe fn saveBuiltinModule(
        this: *mut Self,
        module: BuiltinModuleName,
        archiveType: SlangArchiveType,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).saveBuiltinModule)(this, module, archiveType, outBlob) }
    }

    pub unsafe fn getDownstreamCompilerPath(
        this: *mut Self,
        passThrough: SlangPassThrough,
        outPath: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getDownstreamCompilerPath)(this, passThrough, outPath) }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct TargetDesc {
    pub structureSize: usize,
    pub format: SlangCompileTarget,
    pub profile: SlangProfileID,
    pub flags: SlangTargetFlags,
    pub floatingPointMode: SlangFloatingPointMode,
    pub lineDirectiveMode: SlangLineDirectiveMode,
    pub forceGLSLScalarBufferLayout: bool,
    pub compilerOptionEntries: *const CompilerOptionEntry,
    pub compilerOptionEntryCount: u32,
}

impl Default for TargetDesc {
    fn default() -> Self {
        Self {
            structureSize: 48,
            format: SlangCompileTarget(0),
            profile: SlangProfileID(0),
            flags: 1024,
            floatingPointMode: SlangFloatingPointMode(0),
            lineDirectiveMode: SlangLineDirectiveMode(0),
            forceGLSLScalarBufferLayout: false,
            compilerOptionEntries: ::core::ptr::null(),
            compilerOptionEntryCount: 0,
        }
    }
}

const _: () = {
    assert!(size_of::<TargetDesc>() == 48);
    assert!(align_of::<TargetDesc>() == 8);
    assert!(::core::mem::offset_of!(TargetDesc, structureSize) == 0);
    assert!(::core::mem::offset_of!(TargetDesc, format) == 8);
    assert!(::core::mem::offset_of!(TargetDesc, profile) == 12);
    assert!(::core::mem::offset_of!(TargetDesc, flags) == 16);
    assert!(::core::mem::offset_of!(TargetDesc, floatingPointMode) == 20);
    assert!(::core::mem::offset_of!(TargetDesc, lineDirectiveMode) == 24);
    assert!(::core::mem::offset_of!(TargetDesc, forceGLSLScalarBufferLayout) == 28);
    assert!(::core::mem::offset_of!(TargetDesc, compilerOptionEntries) == 32);
    assert!(::core::mem::offset_of!(TargetDesc, compilerOptionEntryCount) == 40);
};

pub type SessionFlags = u32;

pub const kSessionFlags_None: SessionFlags = 0;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct PreprocessorMacroDesc {
    pub name: *const ::core::ffi::c_char,
    pub value: *const ::core::ffi::c_char,
}

const _: () = {
    assert!(size_of::<PreprocessorMacroDesc>() == 16);
    assert!(align_of::<PreprocessorMacroDesc>() == 8);
    assert!(::core::mem::offset_of!(PreprocessorMacroDesc, name) == 0);
    assert!(::core::mem::offset_of!(PreprocessorMacroDesc, value) == 8);
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SessionDesc {
    pub structureSize: usize,
    pub targets: *const TargetDesc,
    pub targetCount: SlangInt,
    pub flags: SessionFlags,
    pub defaultMatrixLayoutMode: SlangMatrixLayoutMode,
    pub searchPaths: *const *const ::core::ffi::c_char,
    pub searchPathCount: SlangInt,
    pub preprocessorMacros: *const PreprocessorMacroDesc,
    pub preprocessorMacroCount: SlangInt,
    pub fileSystem: *mut ISlangFileSystem,
    pub enableEffectAnnotations: bool,
    pub allowGLSLSyntax: bool,
    pub compilerOptionEntries: *const CompilerOptionEntry,
    pub compilerOptionEntryCount: u32,
    pub skipSPIRVValidation: bool,
}

impl Default for SessionDesc {
    fn default() -> Self {
        Self {
            structureSize: 96,
            targets: ::core::ptr::null(),
            targetCount: 0,
            flags: 0,
            defaultMatrixLayoutMode: SlangMatrixLayoutMode(1),
            searchPaths: ::core::ptr::null(),
            searchPathCount: 0,
            preprocessorMacros: ::core::ptr::null(),
            preprocessorMacroCount: 0,
            fileSystem: ::core::ptr::null_mut(),
            enableEffectAnnotations: false,
            allowGLSLSyntax: false,
            compilerOptionEntries: ::core::ptr::null(),
            compilerOptionEntryCount: 0,
            skipSPIRVValidation: false,
        }
    }
}

const _: () = {
    assert!(size_of::<SessionDesc>() == 96);
    assert!(align_of::<SessionDesc>() == 8);
    assert!(::core::mem::offset_of!(SessionDesc, structureSize) == 0);
    assert!(::core::mem::offset_of!(SessionDesc, targets) == 8);
    assert!(::core::mem::offset_of!(SessionDesc, targetCount) == 16);
    assert!(::core::mem::offset_of!(SessionDesc, flags) == 24);
    assert!(::core::mem::offset_of!(SessionDesc, defaultMatrixLayoutMode) == 28);
    assert!(::core::mem::offset_of!(SessionDesc, searchPaths) == 32);
    assert!(::core::mem::offset_of!(SessionDesc, searchPathCount) == 40);
    assert!(::core::mem::offset_of!(SessionDesc, preprocessorMacros) == 48);
    assert!(::core::mem::offset_of!(SessionDesc, preprocessorMacroCount) == 56);
    assert!(::core::mem::offset_of!(SessionDesc, fileSystem) == 64);
    assert!(::core::mem::offset_of!(SessionDesc, enableEffectAnnotations) == 72);
    assert!(::core::mem::offset_of!(SessionDesc, allowGLSLSyntax) == 73);
    assert!(::core::mem::offset_of!(SessionDesc, compilerOptionEntries) == 80);
    assert!(::core::mem::offset_of!(SessionDesc, compilerOptionEntryCount) == 88);
    assert!(::core::mem::offset_of!(SessionDesc, skipSPIRVValidation) == 92);
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ContainerType(pub ::core::ffi::c_int);

impl ContainerType {
    pub const None: Self = Self(0);
    pub const UnsizedArray: Self = Self(1);
    pub const StructuredBuffer: Self = Self(2);
    pub const ConstantBuffer: Self = Self(3);
    pub const ParameterBlock: Self = Self(4);
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SourceLocation {
    pub filePath: *const ::core::ffi::c_char,
    pub line: SlangInt,
    pub column: SlangInt,
}

impl Default for SourceLocation {
    fn default() -> Self {
        Self {
            filePath: ::core::ptr::null(),
            line: -1,
            column: -1,
        }
    }
}

const _: () = {
    assert!(size_of::<SourceLocation>() == 24);
    assert!(align_of::<SourceLocation>() == 8);
    assert!(::core::mem::offset_of!(SourceLocation, filePath) == 0);
    assert!(::core::mem::offset_of!(SourceLocation, line) == 8);
    assert!(::core::mem::offset_of!(SourceLocation, column) == 16);
};

#[repr(C)]
#[derive(Debug)]
pub struct ISession {
    pub vtable: *const ISessionVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISessionVtbl {
    pub base: ISlangUnknownVtbl,
    pub getGlobalSession: unsafe extern "system" fn(this: *mut ISession) -> *mut IGlobalSession,
    pub loadModule: unsafe extern "system" fn(
        this: *mut ISession,
        moduleName: *const ::core::ffi::c_char,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut IModule,
    pub loadModuleFromSource: unsafe extern "system" fn(
        this: *mut ISession,
        moduleName: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        source: *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut IModule,
    pub createCompositeComponentType: unsafe extern "system" fn(
        this: *mut ISession,
        componentTypes: *const *mut IComponentType,
        componentTypeCount: SlangInt,
        outCompositeComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub specializeType: unsafe extern "system" fn(
        this: *mut ISession,
        r#type: *mut TypeReflection,
        specializationArgs: *const SpecializationArg,
        specializationArgCount: SlangInt,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut TypeReflection,
    pub getTypeLayout: unsafe extern "system" fn(
        this: *mut ISession,
        r#type: *mut TypeReflection,
        targetIndex: SlangInt,
        rules: LayoutRules,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut TypeLayoutReflection,
    pub getContainerType: unsafe extern "system" fn(
        this: *mut ISession,
        elementType: *mut TypeReflection,
        containerType: ContainerType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut TypeReflection,
    pub getDynamicType: unsafe extern "system" fn(this: *mut ISession) -> *mut TypeReflection,
    pub getTypeRTTIMangledName: unsafe extern "system" fn(
        this: *mut ISession,
        r#type: *mut TypeReflection,
        outNameBlob: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub getTypeConformanceWitnessMangledName: unsafe extern "system" fn(
        this: *mut ISession,
        r#type: *mut TypeReflection,
        interfaceType: *mut TypeReflection,
        outNameBlob: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub getTypeConformanceWitnessSequentialID: unsafe extern "system" fn(
        this: *mut ISession,
        r#type: *mut TypeReflection,
        interfaceType: *mut TypeReflection,
        outId: *mut u32,
    ) -> SlangResult,
    pub createCompileRequest: unsafe extern "system" fn(
        this: *mut ISession,
        outCompileRequest: *mut *mut SlangCompileRequest,
    ) -> SlangResult,
    pub createTypeConformanceComponentType: unsafe extern "system" fn(
        this: *mut ISession,
        r#type: *mut TypeReflection,
        interfaceType: *mut TypeReflection,
        outConformance: *mut *mut ITypeConformance,
        conformanceIdOverride: SlangInt,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub loadModuleFromIRBlob: unsafe extern "system" fn(
        this: *mut ISession,
        moduleName: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        source: *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut IModule,
    pub getLoadedModuleCount: unsafe extern "system" fn(this: *mut ISession) -> SlangInt,
    pub getLoadedModule:
        unsafe extern "system" fn(this: *mut ISession, index: SlangInt) -> *mut IModule,
    pub isBinaryModuleUpToDate: unsafe extern "system" fn(
        this: *mut ISession,
        modulePath: *const ::core::ffi::c_char,
        binaryModuleBlob: *mut IBlob,
    ) -> bool,
    pub loadModuleFromSourceString: unsafe extern "system" fn(
        this: *mut ISession,
        moduleName: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        string: *const ::core::ffi::c_char,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut IModule,
    pub getDynamicObjectRTTIBytes: unsafe extern "system" fn(
        this: *mut ISession,
        r#type: *mut TypeReflection,
        interfaceType: *mut TypeReflection,
        outRTTIDataBuffer: *mut u32,
        bufferSizeInBytes: u32,
    ) -> SlangResult,
    pub loadModuleInfoFromIRBlob: unsafe extern "system" fn(
        this: *mut ISession,
        source: *mut IBlob,
        outModuleVersion: *mut SlangInt,
        outModuleCompilerVersion: *mut *const ::core::ffi::c_char,
        outModuleName: *mut *const ::core::ffi::c_char,
    ) -> SlangResult,
    pub getDeclSourceLocation: unsafe extern "system" fn(
        this: *mut ISession,
        decl: *mut DeclReflection,
        outLocation: *mut SourceLocation,
    ) -> SlangResult,
}

unsafe impl Interface for ISession {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x6761_8701,
        data2: 0xD116,
        data3: 0x468F,
        data4: [0xAB, 0x3B, 0x47, 0x4B, 0xED, 0xCE, 0x0E, 0x3D],
    };
}

impl ISession {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn getGlobalSession(this: *mut Self) -> *mut IGlobalSession {
        unsafe { ((*(*this).vtable).getGlobalSession)(this) }
    }

    pub unsafe fn loadModule(
        this: *mut Self,
        moduleName: *const ::core::ffi::c_char,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut IModule {
        unsafe { ((*(*this).vtable).loadModule)(this, moduleName, outDiagnostics) }
    }

    pub unsafe fn loadModuleFromSource(
        this: *mut Self,
        moduleName: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        source: *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut IModule {
        unsafe {
            ((*(*this).vtable).loadModuleFromSource)(this, moduleName, path, source, outDiagnostics)
        }
    }

    pub unsafe fn createCompositeComponentType(
        this: *mut Self,
        componentTypes: *const *mut IComponentType,
        componentTypeCount: SlangInt,
        outCompositeComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).createCompositeComponentType)(
                this,
                componentTypes,
                componentTypeCount,
                outCompositeComponentType,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn specializeType(
        this: *mut Self,
        r#type: *mut TypeReflection,
        specializationArgs: *const SpecializationArg,
        specializationArgCount: SlangInt,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut TypeReflection {
        unsafe {
            ((*(*this).vtable).specializeType)(
                this,
                r#type,
                specializationArgs,
                specializationArgCount,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getTypeLayout(
        this: *mut Self,
        r#type: *mut TypeReflection,
        targetIndex: SlangInt,
        rules: LayoutRules,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut TypeLayoutReflection {
        unsafe {
            ((*(*this).vtable).getTypeLayout)(this, r#type, targetIndex, rules, outDiagnostics)
        }
    }

    pub unsafe fn getContainerType(
        this: *mut Self,
        elementType: *mut TypeReflection,
        containerType: ContainerType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut TypeReflection {
        unsafe {
            ((*(*this).vtable).getContainerType)(this, elementType, containerType, outDiagnostics)
        }
    }

    pub unsafe fn getDynamicType(this: *mut Self) -> *mut TypeReflection {
        unsafe { ((*(*this).vtable).getDynamicType)(this) }
    }

    pub unsafe fn getTypeRTTIMangledName(
        this: *mut Self,
        r#type: *mut TypeReflection,
        outNameBlob: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getTypeRTTIMangledName)(this, r#type, outNameBlob) }
    }

    pub unsafe fn getTypeConformanceWitnessMangledName(
        this: *mut Self,
        r#type: *mut TypeReflection,
        interfaceType: *mut TypeReflection,
        outNameBlob: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getTypeConformanceWitnessMangledName)(
                this,
                r#type,
                interfaceType,
                outNameBlob,
            )
        }
    }

    pub unsafe fn getTypeConformanceWitnessSequentialID(
        this: *mut Self,
        r#type: *mut TypeReflection,
        interfaceType: *mut TypeReflection,
        outId: *mut u32,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getTypeConformanceWitnessSequentialID)(
                this,
                r#type,
                interfaceType,
                outId,
            )
        }
    }

    pub unsafe fn createCompileRequest(
        this: *mut Self,
        outCompileRequest: *mut *mut SlangCompileRequest,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).createCompileRequest)(this, outCompileRequest) }
    }

    pub unsafe fn createTypeConformanceComponentType(
        this: *mut Self,
        r#type: *mut TypeReflection,
        interfaceType: *mut TypeReflection,
        outConformance: *mut *mut ITypeConformance,
        conformanceIdOverride: SlangInt,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).createTypeConformanceComponentType)(
                this,
                r#type,
                interfaceType,
                outConformance,
                conformanceIdOverride,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn loadModuleFromIRBlob(
        this: *mut Self,
        moduleName: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        source: *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut IModule {
        unsafe {
            ((*(*this).vtable).loadModuleFromIRBlob)(this, moduleName, path, source, outDiagnostics)
        }
    }

    pub unsafe fn getLoadedModuleCount(this: *mut Self) -> SlangInt {
        unsafe { ((*(*this).vtable).getLoadedModuleCount)(this) }
    }

    pub unsafe fn getLoadedModule(this: *mut Self, index: SlangInt) -> *mut IModule {
        unsafe { ((*(*this).vtable).getLoadedModule)(this, index) }
    }

    pub unsafe fn isBinaryModuleUpToDate(
        this: *mut Self,
        modulePath: *const ::core::ffi::c_char,
        binaryModuleBlob: *mut IBlob,
    ) -> bool {
        unsafe { ((*(*this).vtable).isBinaryModuleUpToDate)(this, modulePath, binaryModuleBlob) }
    }

    pub unsafe fn loadModuleFromSourceString(
        this: *mut Self,
        moduleName: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        string: *const ::core::ffi::c_char,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut IModule {
        unsafe {
            ((*(*this).vtable).loadModuleFromSourceString)(
                this,
                moduleName,
                path,
                string,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getDynamicObjectRTTIBytes(
        this: *mut Self,
        r#type: *mut TypeReflection,
        interfaceType: *mut TypeReflection,
        outRTTIDataBuffer: *mut u32,
        bufferSizeInBytes: u32,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getDynamicObjectRTTIBytes)(
                this,
                r#type,
                interfaceType,
                outRTTIDataBuffer,
                bufferSizeInBytes,
            )
        }
    }

    pub unsafe fn loadModuleInfoFromIRBlob(
        this: *mut Self,
        source: *mut IBlob,
        outModuleVersion: *mut SlangInt,
        outModuleCompilerVersion: *mut *const ::core::ffi::c_char,
        outModuleName: *mut *const ::core::ffi::c_char,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).loadModuleInfoFromIRBlob)(
                this,
                source,
                outModuleVersion,
                outModuleCompilerVersion,
                outModuleName,
            )
        }
    }

    pub unsafe fn getDeclSourceLocation(
        this: *mut Self,
        decl: *mut DeclReflection,
        outLocation: *mut SourceLocation,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getDeclSourceLocation)(this, decl, outLocation) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IMetadata {
    pub vtable: *const IMetadataVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IMetadataVtbl {
    pub base: ISlangCastableVtbl,
    pub isParameterLocationUsed: unsafe extern "system" fn(
        this: *mut IMetadata,
        category: SlangParameterCategory,
        spaceIndex: SlangUInt,
        registerIndex: SlangUInt,
        outUsed: *mut bool,
    ) -> SlangResult,
    pub getDebugBuildIdentifier:
        unsafe extern "system" fn(this: *mut IMetadata) -> *const ::core::ffi::c_char,
}

unsafe impl Interface for IMetadata {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x8044_A8A3,
        data2: 0xDDC0,
        data3: 0x4B7F,
        data4: [0xAF, 0x8E, 0x02, 0x6E, 0x90, 0x5D, 0x73, 0x32],
    };
}

impl IMetadata {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn isParameterLocationUsed(
        this: *mut Self,
        category: SlangParameterCategory,
        spaceIndex: SlangUInt,
        registerIndex: SlangUInt,
        outUsed: *mut bool,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).isParameterLocationUsed)(
                this,
                category,
                spaceIndex,
                registerIndex,
                outUsed,
            )
        }
    }

    pub unsafe fn getDebugBuildIdentifier(this: *mut Self) -> *const ::core::ffi::c_char {
        unsafe { ((*(*this).vtable).getDebugBuildIdentifier)(this) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IBindlessResourceMetadata {
    pub vtable: *const IBindlessResourceMetadataVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IBindlessResourceMetadataVtbl {
    pub base: ISlangCastableVtbl,
    pub usesBindlessResourceHeap:
        unsafe extern "system" fn(this: *mut IBindlessResourceMetadata) -> bool,
}

unsafe impl Interface for IBindlessResourceMetadata {
    const UUID: SlangUUID = SlangUUID {
        data1: 0xEAFA_96D3,
        data2: 0x2352,
        data3: 0x4BF4,
        data4: [0x88, 0x64, 0x32, 0x28, 0xA4, 0x07, 0x7A, 0x83],
    };
}

impl IBindlessResourceMetadata {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn usesBindlessResourceHeap(this: *mut Self) -> bool {
        unsafe { ((*(*this).vtable).usesBindlessResourceHeap)(this) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverageEntryKind(pub u32);

impl CoverageEntryKind {
    pub const Unknown: Self = Self(0);
    pub const Line: Self = Self(1);
    pub const Branch: Self = Self(2);
    pub const Function: Self = Self(3);
    pub const Region: Self = Self(4);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverageCounterMode(pub u32);

impl CoverageCounterMode {
    pub const Count: Self = Self(0);
    pub const Boolean: Self = Self(1);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverageBranchArmKind(pub u32);

impl CoverageBranchArmKind {
    pub const Unknown: Self = Self(0);
    pub const TrueArm: Self = Self(1);
    pub const FalseArm: Self = Self(2);
    pub const CaseArm: Self = Self(3);
    pub const DefaultArm: Self = Self(4);
}

pub const kInvalidCoverageCounterIndex: u32 = 4_294_967_295;

pub const kUnboundedSyntheticResourceArraySize: u32 = 4_294_967_295;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CoverageEntryInfo {
    pub structSize: usize,
    pub file: *const ::core::ffi::c_char,
    pub line: u32,
    pub counterIndex: u32,
    pub kind: CoverageEntryKind,
    pub counterMode: CoverageCounterMode,
    pub startColumn: u32,
    pub endLine: u32,
    pub endColumn: u32,
    pub functionName: *const ::core::ffi::c_char,
    pub functionMangledName: *const ::core::ffi::c_char,
    pub branchSiteID: u32,
    pub branchArmID: u32,
    pub branchArmKind: CoverageBranchArmKind,
}

impl Default for CoverageEntryInfo {
    fn default() -> Self {
        Self {
            structSize: 80,
            file: ::core::ptr::null(),
            line: 0,
            counterIndex: 4_294_967_295,
            kind: CoverageEntryKind(0),
            counterMode: CoverageCounterMode(0),
            startColumn: 0,
            endLine: 0,
            endColumn: 0,
            functionName: ::core::ptr::null(),
            functionMangledName: ::core::ptr::null(),
            branchSiteID: 0,
            branchArmID: 0,
            branchArmKind: CoverageBranchArmKind(0),
        }
    }
}

const _: () = {
    assert!(size_of::<CoverageEntryInfo>() == 80);
    assert!(align_of::<CoverageEntryInfo>() == 8);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, structSize) == 0);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, file) == 8);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, line) == 16);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, counterIndex) == 20);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, kind) == 24);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, counterMode) == 28);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, startColumn) == 32);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, endLine) == 36);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, endColumn) == 40);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, functionName) == 48);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, functionMangledName) == 56);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, branchSiteID) == 64);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, branchArmID) == 68);
    assert!(::core::mem::offset_of!(CoverageEntryInfo, branchArmKind) == 72);
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CoverageBufferInfo {
    pub structSize: usize,
    pub space: i32,
    pub binding: i32,
    pub elementByteWidth: u32,
}

impl Default for CoverageBufferInfo {
    fn default() -> Self {
        Self {
            structSize: 24,
            space: -1,
            binding: -1,
            elementByteWidth: 4,
        }
    }
}

const _: () = {
    assert!(size_of::<CoverageBufferInfo>() == 24);
    assert!(align_of::<CoverageBufferInfo>() == 8);
    assert!(::core::mem::offset_of!(CoverageBufferInfo, structSize) == 0);
    assert!(::core::mem::offset_of!(CoverageBufferInfo, space) == 8);
    assert!(::core::mem::offset_of!(CoverageBufferInfo, binding) == 12);
    assert!(::core::mem::offset_of!(CoverageBufferInfo, elementByteWidth) == 16);
};

#[repr(C)]
#[derive(Debug)]
pub struct ICoverageTracingMetadata {
    pub vtable: *const ICoverageTracingMetadataVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ICoverageTracingMetadataVtbl {
    pub base: ISlangCastableVtbl,
    pub getCounterCount: unsafe extern "system" fn(this: *mut ICoverageTracingMetadata) -> u32,
    pub getEntryInfo: unsafe extern "system" fn(
        this: *mut ICoverageTracingMetadata,
        index: u32,
        outInfo: *mut CoverageEntryInfo,
    ) -> SlangResult,
    pub getBufferInfo: unsafe extern "system" fn(
        this: *mut ICoverageTracingMetadata,
        outInfo: *mut CoverageBufferInfo,
    ) -> SlangResult,
    pub getEntryCount: unsafe extern "system" fn(this: *mut ICoverageTracingMetadata) -> u32,
}

unsafe impl Interface for ICoverageTracingMetadata {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x7C9F_1D50,
        data2: 0x1E4A,
        data3: 0x4B9C,
        data4: [0x8E, 0x21, 0x3F, 0x7B, 0x82, 0xA3, 0xD9, 0x51],
    };
}

impl ICoverageTracingMetadata {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn getCounterCount(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).getCounterCount)(this) }
    }

    pub unsafe fn getEntryInfo(
        this: *mut Self,
        index: u32,
        outInfo: *mut CoverageEntryInfo,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getEntryInfo)(this, index, outInfo) }
    }

    pub unsafe fn getBufferInfo(this: *mut Self, outInfo: *mut CoverageBufferInfo) -> SlangResult {
        unsafe { ((*(*this).vtable).getBufferInfo)(this, outInfo) }
    }

    pub unsafe fn getEntryCount(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).getEntryCount)(this) }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntheticResourceScope(pub u32);

impl SyntheticResourceScope {
    pub const Global: Self = Self(0);
    pub const EntryPoint: Self = Self(1);
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SyntheticResourceAccess(pub u32);

impl SyntheticResourceAccess {
    pub const Read: Self = Self(0);
    pub const Write: Self = Self(1);
    pub const ReadWrite: Self = Self(2);
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SyntheticResourceInfo {
    pub structSize: usize,
    pub id: u32,
    pub bindingType: BindingType,
    pub arraySize: u32,
    pub scope: SyntheticResourceScope,
    pub access: SyntheticResourceAccess,
    pub entryPointIndex: i32,
    pub space: i32,
    pub binding: i32,
    pub uniformOffset: i32,
    pub uniformStride: i32,
    pub debugName: *const ::core::ffi::c_char,
    pub bindlessIndex: i32,
}

impl Default for SyntheticResourceInfo {
    fn default() -> Self {
        Self {
            structSize: 64,
            id: 0,
            bindingType: BindingType(0),
            arraySize: 1,
            scope: SyntheticResourceScope(0),
            access: SyntheticResourceAccess(0),
            entryPointIndex: -1,
            space: -1,
            binding: -1,
            uniformOffset: -1,
            uniformStride: 0,
            debugName: ::core::ptr::null(),
            bindlessIndex: -1,
        }
    }
}

const _: () = {
    assert!(size_of::<SyntheticResourceInfo>() == 64);
    assert!(align_of::<SyntheticResourceInfo>() == 8);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, structSize) == 0);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, id) == 8);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, bindingType) == 12);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, arraySize) == 16);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, scope) == 20);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, access) == 24);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, entryPointIndex) == 28);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, space) == 32);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, binding) == 36);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, uniformOffset) == 40);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, uniformStride) == 44);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, debugName) == 48);
    assert!(::core::mem::offset_of!(SyntheticResourceInfo, bindlessIndex) == 56);
};

#[repr(C)]
#[derive(Debug)]
pub struct ISyntheticResourceMetadata {
    pub vtable: *const ISyntheticResourceMetadataVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ISyntheticResourceMetadataVtbl {
    pub base: ISlangCastableVtbl,
    pub getResourceCount: unsafe extern "system" fn(this: *mut ISyntheticResourceMetadata) -> u32,
    pub getResourceInfo: unsafe extern "system" fn(
        this: *mut ISyntheticResourceMetadata,
        index: u32,
        outInfo: *mut SyntheticResourceInfo,
    ) -> SlangResult,
    pub findResourceIndexByID: unsafe extern "system" fn(
        this: *mut ISyntheticResourceMetadata,
        id: u32,
        outIndex: *mut u32,
    ) -> SlangResult,
}

unsafe impl Interface for ISyntheticResourceMetadata {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x47A3_3723,
        data2: 0x181B,
        data3: 0x4D2B,
        data4: [0xB8, 0x9E, 0x21, 0x54, 0x95, 0xBB, 0x38, 0x8B],
    };
}

impl ISyntheticResourceMetadata {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn getResourceCount(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).getResourceCount)(this) }
    }

    pub unsafe fn getResourceInfo(
        this: *mut Self,
        index: u32,
        outInfo: *mut SyntheticResourceInfo,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getResourceInfo)(this, index, outInfo) }
    }

    pub unsafe fn findResourceIndexByID(
        this: *mut Self,
        id: u32,
        outIndex: *mut u32,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).findResourceIndexByID)(this, id, outIndex) }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CooperativeMatrixType {
    pub componentType: SlangScalarType,
    pub scope: SlangScope,
    pub rowCount: u32,
    pub columnCount: u32,
    pub r#use: SlangCooperativeMatrixUse,
}

impl Default for CooperativeMatrixType {
    fn default() -> Self {
        Self {
            componentType: SlangScalarType(0),
            scope: SlangScope(0),
            rowCount: 0,
            columnCount: 0,
            r#use: SlangCooperativeMatrixUse(0),
        }
    }
}

const _: () = {
    assert!(size_of::<CooperativeMatrixType>() == 20);
    assert!(align_of::<CooperativeMatrixType>() == 4);
    assert!(::core::mem::offset_of!(CooperativeMatrixType, componentType) == 0);
    assert!(::core::mem::offset_of!(CooperativeMatrixType, scope) == 4);
    assert!(::core::mem::offset_of!(CooperativeMatrixType, rowCount) == 8);
    assert!(::core::mem::offset_of!(CooperativeMatrixType, columnCount) == 12);
    assert!(::core::mem::offset_of!(CooperativeMatrixType, r#use) == 16);
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CooperativeMatrixCombination {
    pub m: u32,
    pub n: u32,
    pub k: u32,
    pub componentTypeA: SlangScalarType,
    pub componentTypeB: SlangScalarType,
    pub componentTypeC: SlangScalarType,
    pub componentTypeResult: SlangScalarType,
    pub saturate: SlangBool,
    pub scope: SlangScope,
}

impl Default for CooperativeMatrixCombination {
    fn default() -> Self {
        Self {
            m: 0,
            n: 0,
            k: 0,
            componentTypeA: SlangScalarType(0),
            componentTypeB: SlangScalarType(0),
            componentTypeC: SlangScalarType(0),
            componentTypeResult: SlangScalarType(0),
            saturate: false,
            scope: SlangScope(0),
        }
    }
}

const _: () = {
    assert!(size_of::<CooperativeMatrixCombination>() == 36);
    assert!(align_of::<CooperativeMatrixCombination>() == 4);
    assert!(::core::mem::offset_of!(CooperativeMatrixCombination, m) == 0);
    assert!(::core::mem::offset_of!(CooperativeMatrixCombination, n) == 4);
    assert!(::core::mem::offset_of!(CooperativeMatrixCombination, k) == 8);
    assert!(::core::mem::offset_of!(CooperativeMatrixCombination, componentTypeA) == 12);
    assert!(::core::mem::offset_of!(CooperativeMatrixCombination, componentTypeB) == 16);
    assert!(::core::mem::offset_of!(CooperativeMatrixCombination, componentTypeC) == 20);
    assert!(::core::mem::offset_of!(CooperativeMatrixCombination, componentTypeResult) == 24);
    assert!(::core::mem::offset_of!(CooperativeMatrixCombination, saturate) == 28);
    assert!(::core::mem::offset_of!(CooperativeMatrixCombination, scope) == 32);
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CooperativeVectorTypeUsageInfo {
    pub componentType: SlangScalarType,
    pub maxSize: u32,
    pub usedForTrainingOp: SlangBool,
}

impl Default for CooperativeVectorTypeUsageInfo {
    fn default() -> Self {
        Self {
            componentType: SlangScalarType(0),
            maxSize: 0,
            usedForTrainingOp: false,
        }
    }
}

const _: () = {
    assert!(size_of::<CooperativeVectorTypeUsageInfo>() == 12);
    assert!(align_of::<CooperativeVectorTypeUsageInfo>() == 4);
    assert!(::core::mem::offset_of!(CooperativeVectorTypeUsageInfo, componentType) == 0);
    assert!(::core::mem::offset_of!(CooperativeVectorTypeUsageInfo, maxSize) == 4);
    assert!(::core::mem::offset_of!(CooperativeVectorTypeUsageInfo, usedForTrainingOp) == 8);
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct CooperativeVectorCombination {
    pub inputType: SlangScalarType,
    pub inputInterpretation: SlangScalarType,
    pub inputPackingFactor: u32,
    pub matrixInterpretation: SlangScalarType,
    pub biasInterpretation: SlangScalarType,
    pub resultType: SlangScalarType,
    pub transpose: SlangBool,
}

impl Default for CooperativeVectorCombination {
    fn default() -> Self {
        Self {
            inputType: SlangScalarType(0),
            inputInterpretation: SlangScalarType(0),
            inputPackingFactor: 1,
            matrixInterpretation: SlangScalarType(0),
            biasInterpretation: SlangScalarType(0),
            resultType: SlangScalarType(0),
            transpose: false,
        }
    }
}

const _: () = {
    assert!(size_of::<CooperativeVectorCombination>() == 28);
    assert!(align_of::<CooperativeVectorCombination>() == 4);
    assert!(::core::mem::offset_of!(CooperativeVectorCombination, inputType) == 0);
    assert!(::core::mem::offset_of!(CooperativeVectorCombination, inputInterpretation) == 4);
    assert!(::core::mem::offset_of!(CooperativeVectorCombination, inputPackingFactor) == 8);
    assert!(::core::mem::offset_of!(CooperativeVectorCombination, matrixInterpretation) == 12);
    assert!(::core::mem::offset_of!(CooperativeVectorCombination, biasInterpretation) == 16);
    assert!(::core::mem::offset_of!(CooperativeVectorCombination, resultType) == 20);
    assert!(::core::mem::offset_of!(CooperativeVectorCombination, transpose) == 24);
};

#[repr(C)]
#[derive(Debug)]
pub struct ICooperativeTypesMetadata {
    pub vtable: *const ICooperativeTypesMetadataVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ICooperativeTypesMetadataVtbl {
    pub base: ISlangCastableVtbl,
    pub getCooperativeMatrixTypeCount:
        unsafe extern "system" fn(this: *mut ICooperativeTypesMetadata) -> SlangUInt,
    pub getCooperativeMatrixTypeByIndex: unsafe extern "system" fn(
        this: *mut ICooperativeTypesMetadata,
        index: SlangUInt,
        outType: *mut CooperativeMatrixType,
    ) -> SlangResult,
    pub getCooperativeMatrixCombinationCount:
        unsafe extern "system" fn(this: *mut ICooperativeTypesMetadata) -> SlangUInt,
    pub getCooperativeMatrixCombinationByIndex: unsafe extern "system" fn(
        this: *mut ICooperativeTypesMetadata,
        index: SlangUInt,
        outCombination: *mut CooperativeMatrixCombination,
    ) -> SlangResult,
    pub getCooperativeVectorTypeCount:
        unsafe extern "system" fn(this: *mut ICooperativeTypesMetadata) -> SlangUInt,
    pub getCooperativeVectorTypeByIndex: unsafe extern "system" fn(
        this: *mut ICooperativeTypesMetadata,
        index: SlangUInt,
        outType: *mut CooperativeVectorTypeUsageInfo,
    ) -> SlangResult,
    pub getCooperativeVectorCombinationCount:
        unsafe extern "system" fn(this: *mut ICooperativeTypesMetadata) -> SlangUInt,
    pub getCooperativeVectorCombinationByIndex: unsafe extern "system" fn(
        this: *mut ICooperativeTypesMetadata,
        index: SlangUInt,
        outCombination: *mut CooperativeVectorCombination,
    ) -> SlangResult,
}

unsafe impl Interface for ICooperativeTypesMetadata {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x64C4_D536,
        data2: 0xD949,
        data3: 0x49C3,
        data4: [0x9F, 0xDE, 0x3F, 0x0F, 0x9C, 0x6F, 0x01, 0x31],
    };
}

impl ICooperativeTypesMetadata {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn getCooperativeMatrixTypeCount(this: *mut Self) -> SlangUInt {
        unsafe { ((*(*this).vtable).getCooperativeMatrixTypeCount)(this) }
    }

    pub unsafe fn getCooperativeMatrixTypeByIndex(
        this: *mut Self,
        index: SlangUInt,
        outType: *mut CooperativeMatrixType,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getCooperativeMatrixTypeByIndex)(this, index, outType) }
    }

    pub unsafe fn getCooperativeMatrixCombinationCount(this: *mut Self) -> SlangUInt {
        unsafe { ((*(*this).vtable).getCooperativeMatrixCombinationCount)(this) }
    }

    pub unsafe fn getCooperativeMatrixCombinationByIndex(
        this: *mut Self,
        index: SlangUInt,
        outCombination: *mut CooperativeMatrixCombination,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getCooperativeMatrixCombinationByIndex)(this, index, outCombination)
        }
    }

    pub unsafe fn getCooperativeVectorTypeCount(this: *mut Self) -> SlangUInt {
        unsafe { ((*(*this).vtable).getCooperativeVectorTypeCount)(this) }
    }

    pub unsafe fn getCooperativeVectorTypeByIndex(
        this: *mut Self,
        index: SlangUInt,
        outType: *mut CooperativeVectorTypeUsageInfo,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getCooperativeVectorTypeByIndex)(this, index, outType) }
    }

    pub unsafe fn getCooperativeVectorCombinationCount(this: *mut Self) -> SlangUInt {
        unsafe { ((*(*this).vtable).getCooperativeVectorCombinationCount)(this) }
    }

    pub unsafe fn getCooperativeVectorCombinationByIndex(
        this: *mut Self,
        index: SlangUInt,
        outCombination: *mut CooperativeVectorCombination,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getCooperativeVectorCombinationByIndex)(this, index, outCombination)
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ICompileResult {
    pub vtable: *const ICompileResultVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ICompileResultVtbl {
    pub base: ISlangCastableVtbl,
    pub getItemCount: unsafe extern "system" fn(this: *mut ICompileResult) -> u32,
    pub getItemData: unsafe extern "system" fn(
        this: *mut ICompileResult,
        index: u32,
        outblob: *mut *mut IBlob,
    ) -> SlangResult,
    pub getMetadata: unsafe extern "system" fn(
        this: *mut ICompileResult,
        outMetadata: *mut *mut IMetadata,
    ) -> SlangResult,
}

unsafe impl Interface for ICompileResult {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x5FA9_380E,
        data2: 0xB62F,
        data3: 0x41E5,
        data4: [0x9F, 0x12, 0x4B, 0xAD, 0x4D, 0x9E, 0xAA, 0xE4],
    };
}

impl ICompileResult {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn castAs(this: *mut Self, guid: *const SlangUUID) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).base.castAs)(this.cast(), guid) }
    }

    pub unsafe fn getItemCount(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).getItemCount)(this) }
    }

    pub unsafe fn getItemData(
        this: *mut Self,
        index: u32,
        outblob: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getItemData)(this, index, outblob) }
    }

    pub unsafe fn getMetadata(this: *mut Self, outMetadata: *mut *mut IMetadata) -> SlangResult {
        unsafe { ((*(*this).vtable).getMetadata)(this, outMetadata) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IComponentType {
    pub vtable: *const IComponentTypeVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IComponentTypeVtbl {
    pub base: ISlangUnknownVtbl,
    pub getSession: unsafe extern "system" fn(this: *mut IComponentType) -> *mut ISession,
    pub getLayout: unsafe extern "system" fn(
        this: *mut IComponentType,
        targetIndex: SlangInt,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut ProgramLayout,
    pub getSpecializationParamCount:
        unsafe extern "system" fn(this: *mut IComponentType) -> SlangInt,
    pub getEntryPointCode: unsafe extern "system" fn(
        this: *mut IComponentType,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
    pub getResultAsFileSystem: unsafe extern "system" fn(
        this: *mut IComponentType,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outFileSystem: *mut *mut ISlangMutableFileSystem,
    ) -> SlangResult,
    pub getEntryPointHash: unsafe extern "system" fn(
        this: *mut IComponentType,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outHash: *mut *mut IBlob,
    ),
    pub specialize: unsafe extern "system" fn(
        this: *mut IComponentType,
        specializationArgs: *const SpecializationArg,
        specializationArgCount: SlangInt,
        outSpecializedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub link: unsafe extern "system" fn(
        this: *mut IComponentType,
        outLinkedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub getEntryPointHostCallable: unsafe extern "system" fn(
        this: *mut IComponentType,
        entryPointIndex: ::core::ffi::c_int,
        targetIndex: ::core::ffi::c_int,
        outSharedLibrary: *mut *mut ISlangSharedLibrary,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
    pub renameEntryPoint: unsafe extern "system" fn(
        this: *mut IComponentType,
        newName: *const ::core::ffi::c_char,
        outEntryPoint: *mut *mut IComponentType,
    ) -> SlangResult,
    pub linkWithOptions: unsafe extern "system" fn(
        this: *mut IComponentType,
        outLinkedComponentType: *mut *mut IComponentType,
        compilerOptionEntryCount: u32,
        compilerOptionEntries: *const CompilerOptionEntry,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub getTargetCode: unsafe extern "system" fn(
        this: *mut IComponentType,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
    pub getTargetMetadata: unsafe extern "system" fn(
        this: *mut IComponentType,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
    pub getEntryPointMetadata: unsafe extern "system" fn(
        this: *mut IComponentType,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
}

unsafe impl Interface for IComponentType {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x5BC4_2BE8,
        data2: 0x5C50,
        data3: 0x4929,
        data4: [0x9E, 0x5E, 0xD1, 0x5E, 0x7C, 0x24, 0x01, 0x5F],
    };
}

impl IComponentType {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn getSession(this: *mut Self) -> *mut ISession {
        unsafe { ((*(*this).vtable).getSession)(this) }
    }

    pub unsafe fn getLayout(
        this: *mut Self,
        targetIndex: SlangInt,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut ProgramLayout {
        unsafe { ((*(*this).vtable).getLayout)(this, targetIndex, outDiagnostics) }
    }

    pub unsafe fn getSpecializationParamCount(this: *mut Self) -> SlangInt {
        unsafe { ((*(*this).vtable).getSpecializationParamCount)(this) }
    }

    pub unsafe fn getEntryPointCode(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getEntryPointCode)(
                this,
                entryPointIndex,
                targetIndex,
                outCode,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getResultAsFileSystem(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outFileSystem: *mut *mut ISlangMutableFileSystem,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getResultAsFileSystem)(
                this,
                entryPointIndex,
                targetIndex,
                outFileSystem,
            )
        }
    }

    pub unsafe fn getEntryPointHash(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outHash: *mut *mut IBlob,
    ) {
        unsafe {
            ((*(*this).vtable).getEntryPointHash)(this, entryPointIndex, targetIndex, outHash);
        }
    }

    pub unsafe fn specialize(
        this: *mut Self,
        specializationArgs: *const SpecializationArg,
        specializationArgCount: SlangInt,
        outSpecializedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).specialize)(
                this,
                specializationArgs,
                specializationArgCount,
                outSpecializedComponentType,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn link(
        this: *mut Self,
        outLinkedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).link)(this, outLinkedComponentType, outDiagnostics) }
    }

    pub unsafe fn getEntryPointHostCallable(
        this: *mut Self,
        entryPointIndex: ::core::ffi::c_int,
        targetIndex: ::core::ffi::c_int,
        outSharedLibrary: *mut *mut ISlangSharedLibrary,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getEntryPointHostCallable)(
                this,
                entryPointIndex,
                targetIndex,
                outSharedLibrary,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn renameEntryPoint(
        this: *mut Self,
        newName: *const ::core::ffi::c_char,
        outEntryPoint: *mut *mut IComponentType,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).renameEntryPoint)(this, newName, outEntryPoint) }
    }

    pub unsafe fn linkWithOptions(
        this: *mut Self,
        outLinkedComponentType: *mut *mut IComponentType,
        compilerOptionEntryCount: u32,
        compilerOptionEntries: *const CompilerOptionEntry,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).linkWithOptions)(
                this,
                outLinkedComponentType,
                compilerOptionEntryCount,
                compilerOptionEntries,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getTargetCode(
        this: *mut Self,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getTargetCode)(this, targetIndex, outCode, outDiagnostics) }
    }

    pub unsafe fn getTargetMetadata(
        this: *mut Self,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getTargetMetadata)(this, targetIndex, outMetadata, outDiagnostics)
        }
    }

    pub unsafe fn getEntryPointMetadata(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getEntryPointMetadata)(
                this,
                entryPointIndex,
                targetIndex,
                outMetadata,
                outDiagnostics,
            )
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IEntryPoint {
    pub vtable: *const IEntryPointVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IEntryPointVtbl {
    pub base: IComponentTypeVtbl,
    pub getFunctionReflection:
        unsafe extern "system" fn(this: *mut IEntryPoint) -> *mut FunctionReflection,
}

unsafe impl Interface for IEntryPoint {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x8F24_1361,
        data2: 0xF5BD,
        data3: 0x4CA0,
        data4: [0xA3, 0xAC, 0x02, 0xF7, 0xFA, 0x24, 0x02, 0xB8],
    };
}

impl IEntryPoint {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn getSession(this: *mut Self) -> *mut ISession {
        unsafe { ((*(*this).vtable).base.getSession)(this.cast()) }
    }

    pub unsafe fn getLayout(
        this: *mut Self,
        targetIndex: SlangInt,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut ProgramLayout {
        unsafe { ((*(*this).vtable).base.getLayout)(this.cast(), targetIndex, outDiagnostics) }
    }

    pub unsafe fn getSpecializationParamCount(this: *mut Self) -> SlangInt {
        unsafe { ((*(*this).vtable).base.getSpecializationParamCount)(this.cast()) }
    }

    pub unsafe fn getEntryPointCode(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getEntryPointCode)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outCode,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getResultAsFileSystem(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outFileSystem: *mut *mut ISlangMutableFileSystem,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getResultAsFileSystem)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outFileSystem,
            )
        }
    }

    pub unsafe fn getEntryPointHash(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outHash: *mut *mut IBlob,
    ) {
        unsafe {
            ((*(*this).vtable).base.getEntryPointHash)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outHash,
            );
        }
    }

    pub unsafe fn specialize(
        this: *mut Self,
        specializationArgs: *const SpecializationArg,
        specializationArgCount: SlangInt,
        outSpecializedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.specialize)(
                this.cast(),
                specializationArgs,
                specializationArgCount,
                outSpecializedComponentType,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn link(
        this: *mut Self,
        outLinkedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.link)(this.cast(), outLinkedComponentType, outDiagnostics)
        }
    }

    pub unsafe fn getEntryPointHostCallable(
        this: *mut Self,
        entryPointIndex: ::core::ffi::c_int,
        targetIndex: ::core::ffi::c_int,
        outSharedLibrary: *mut *mut ISlangSharedLibrary,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getEntryPointHostCallable)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outSharedLibrary,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn renameEntryPoint(
        this: *mut Self,
        newName: *const ::core::ffi::c_char,
        outEntryPoint: *mut *mut IComponentType,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.renameEntryPoint)(this.cast(), newName, outEntryPoint) }
    }

    pub unsafe fn linkWithOptions(
        this: *mut Self,
        outLinkedComponentType: *mut *mut IComponentType,
        compilerOptionEntryCount: u32,
        compilerOptionEntries: *const CompilerOptionEntry,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.linkWithOptions)(
                this.cast(),
                outLinkedComponentType,
                compilerOptionEntryCount,
                compilerOptionEntries,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getTargetCode(
        this: *mut Self,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getTargetCode)(
                this.cast(),
                targetIndex,
                outCode,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getTargetMetadata(
        this: *mut Self,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getTargetMetadata)(
                this.cast(),
                targetIndex,
                outMetadata,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getEntryPointMetadata(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getEntryPointMetadata)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outMetadata,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getFunctionReflection(this: *mut Self) -> *mut FunctionReflection {
        unsafe { ((*(*this).vtable).getFunctionReflection)(this) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct ITypeConformance {
    pub vtable: *const ITypeConformanceVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct ITypeConformanceVtbl {
    pub base: IComponentTypeVtbl,
}

unsafe impl Interface for ITypeConformance {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x73EB_3147,
        data2: 0xE544,
        data3: 0x41B5,
        data4: [0xB8, 0xF0, 0xA2, 0x44, 0xDF, 0x21, 0x94, 0x0B],
    };
}

impl ITypeConformance {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn getSession(this: *mut Self) -> *mut ISession {
        unsafe { ((*(*this).vtable).base.getSession)(this.cast()) }
    }

    pub unsafe fn getLayout(
        this: *mut Self,
        targetIndex: SlangInt,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut ProgramLayout {
        unsafe { ((*(*this).vtable).base.getLayout)(this.cast(), targetIndex, outDiagnostics) }
    }

    pub unsafe fn getSpecializationParamCount(this: *mut Self) -> SlangInt {
        unsafe { ((*(*this).vtable).base.getSpecializationParamCount)(this.cast()) }
    }

    pub unsafe fn getEntryPointCode(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getEntryPointCode)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outCode,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getResultAsFileSystem(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outFileSystem: *mut *mut ISlangMutableFileSystem,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getResultAsFileSystem)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outFileSystem,
            )
        }
    }

    pub unsafe fn getEntryPointHash(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outHash: *mut *mut IBlob,
    ) {
        unsafe {
            ((*(*this).vtable).base.getEntryPointHash)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outHash,
            );
        }
    }

    pub unsafe fn specialize(
        this: *mut Self,
        specializationArgs: *const SpecializationArg,
        specializationArgCount: SlangInt,
        outSpecializedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.specialize)(
                this.cast(),
                specializationArgs,
                specializationArgCount,
                outSpecializedComponentType,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn link(
        this: *mut Self,
        outLinkedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.link)(this.cast(), outLinkedComponentType, outDiagnostics)
        }
    }

    pub unsafe fn getEntryPointHostCallable(
        this: *mut Self,
        entryPointIndex: ::core::ffi::c_int,
        targetIndex: ::core::ffi::c_int,
        outSharedLibrary: *mut *mut ISlangSharedLibrary,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getEntryPointHostCallable)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outSharedLibrary,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn renameEntryPoint(
        this: *mut Self,
        newName: *const ::core::ffi::c_char,
        outEntryPoint: *mut *mut IComponentType,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.renameEntryPoint)(this.cast(), newName, outEntryPoint) }
    }

    pub unsafe fn linkWithOptions(
        this: *mut Self,
        outLinkedComponentType: *mut *mut IComponentType,
        compilerOptionEntryCount: u32,
        compilerOptionEntries: *const CompilerOptionEntry,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.linkWithOptions)(
                this.cast(),
                outLinkedComponentType,
                compilerOptionEntryCount,
                compilerOptionEntries,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getTargetCode(
        this: *mut Self,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getTargetCode)(
                this.cast(),
                targetIndex,
                outCode,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getTargetMetadata(
        this: *mut Self,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getTargetMetadata)(
                this.cast(),
                targetIndex,
                outMetadata,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getEntryPointMetadata(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getEntryPointMetadata)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outMetadata,
                outDiagnostics,
            )
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IComponentType2 {
    pub vtable: *const IComponentType2Vtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IComponentType2Vtbl {
    pub base: ISlangUnknownVtbl,
    pub getTargetCompileResult: unsafe extern "system" fn(
        this: *mut IComponentType2,
        targetIndex: SlangInt,
        outCompileResult: *mut *mut ICompileResult,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
    pub getEntryPointCompileResult: unsafe extern "system" fn(
        this: *mut IComponentType2,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outCompileResult: *mut *mut ICompileResult,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
    pub getTargetHostCallable: unsafe extern "system" fn(
        this: *mut IComponentType2,
        targetIndex: ::core::ffi::c_int,
        outSharedLibrary: *mut *mut ISlangSharedLibrary,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
}

unsafe impl Interface for IComponentType2 {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x9C2A_4B3D,
        data2: 0x7F68,
        data3: 0x4E91,
        data4: [0xA5, 0x2C, 0x8B, 0x19, 0x3E, 0x45, 0x7A, 0x9F],
    };
}

impl IComponentType2 {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn getTargetCompileResult(
        this: *mut Self,
        targetIndex: SlangInt,
        outCompileResult: *mut *mut ICompileResult,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getTargetCompileResult)(
                this,
                targetIndex,
                outCompileResult,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getEntryPointCompileResult(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outCompileResult: *mut *mut ICompileResult,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getEntryPointCompileResult)(
                this,
                entryPointIndex,
                targetIndex,
                outCompileResult,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getTargetHostCallable(
        this: *mut Self,
        targetIndex: ::core::ffi::c_int,
        outSharedLibrary: *mut *mut ISlangSharedLibrary,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getTargetHostCallable)(
                this,
                targetIndex,
                outSharedLibrary,
                outDiagnostics,
            )
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IModule {
    pub vtable: *const IModuleVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IModuleVtbl {
    pub base: IComponentTypeVtbl,
    pub findEntryPointByName: unsafe extern "system" fn(
        this: *mut IModule,
        name: *const ::core::ffi::c_char,
        outEntryPoint: *mut *mut IEntryPoint,
    ) -> SlangResult,
    pub getDefinedEntryPointCount: unsafe extern "system" fn(this: *mut IModule) -> SlangInt32,
    pub getDefinedEntryPoint: unsafe extern "system" fn(
        this: *mut IModule,
        index: SlangInt32,
        outEntryPoint: *mut *mut IEntryPoint,
    ) -> SlangResult,
    pub serialize: unsafe extern "system" fn(
        this: *mut IModule,
        outSerializedBlob: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub writeToFile: unsafe extern "system" fn(
        this: *mut IModule,
        fileName: *const ::core::ffi::c_char,
    ) -> SlangResult,
    pub getName: unsafe extern "system" fn(this: *mut IModule) -> *const ::core::ffi::c_char,
    pub getFilePath: unsafe extern "system" fn(this: *mut IModule) -> *const ::core::ffi::c_char,
    pub getUniqueIdentity:
        unsafe extern "system" fn(this: *mut IModule) -> *const ::core::ffi::c_char,
    pub findAndCheckEntryPoint: unsafe extern "system" fn(
        this: *mut IModule,
        name: *const ::core::ffi::c_char,
        stage: SlangStage,
        outEntryPoint: *mut *mut IEntryPoint,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub getDependencyFileCount: unsafe extern "system" fn(this: *mut IModule) -> SlangInt32,
    pub getDependencyFilePath: unsafe extern "system" fn(
        this: *mut IModule,
        index: SlangInt32,
    ) -> *const ::core::ffi::c_char,
    pub getModuleReflection: unsafe extern "system" fn(this: *mut IModule) -> *mut DeclReflection,
    pub disassemble: unsafe extern "system" fn(
        this: *mut IModule,
        outDisassembledBlob: *mut *mut IBlob,
    ) -> SlangResult,
}

unsafe impl Interface for IModule {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x0C72_0E64,
        data2: 0x8722,
        data3: 0x4D31,
        data4: [0x89, 0x90, 0x63, 0x8A, 0x98, 0xB1, 0xC2, 0x79],
    };
}

impl IModule {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.base.release)(this.cast()) }
    }

    pub unsafe fn getSession(this: *mut Self) -> *mut ISession {
        unsafe { ((*(*this).vtable).base.getSession)(this.cast()) }
    }

    pub unsafe fn getLayout(
        this: *mut Self,
        targetIndex: SlangInt,
        outDiagnostics: *mut *mut IBlob,
    ) -> *mut ProgramLayout {
        unsafe { ((*(*this).vtable).base.getLayout)(this.cast(), targetIndex, outDiagnostics) }
    }

    pub unsafe fn getSpecializationParamCount(this: *mut Self) -> SlangInt {
        unsafe { ((*(*this).vtable).base.getSpecializationParamCount)(this.cast()) }
    }

    pub unsafe fn getEntryPointCode(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getEntryPointCode)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outCode,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getResultAsFileSystem(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outFileSystem: *mut *mut ISlangMutableFileSystem,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getResultAsFileSystem)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outFileSystem,
            )
        }
    }

    pub unsafe fn getEntryPointHash(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outHash: *mut *mut IBlob,
    ) {
        unsafe {
            ((*(*this).vtable).base.getEntryPointHash)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outHash,
            );
        }
    }

    pub unsafe fn specialize(
        this: *mut Self,
        specializationArgs: *const SpecializationArg,
        specializationArgCount: SlangInt,
        outSpecializedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.specialize)(
                this.cast(),
                specializationArgs,
                specializationArgCount,
                outSpecializedComponentType,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn link(
        this: *mut Self,
        outLinkedComponentType: *mut *mut IComponentType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.link)(this.cast(), outLinkedComponentType, outDiagnostics)
        }
    }

    pub unsafe fn getEntryPointHostCallable(
        this: *mut Self,
        entryPointIndex: ::core::ffi::c_int,
        targetIndex: ::core::ffi::c_int,
        outSharedLibrary: *mut *mut ISlangSharedLibrary,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getEntryPointHostCallable)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outSharedLibrary,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn renameEntryPoint(
        this: *mut Self,
        newName: *const ::core::ffi::c_char,
        outEntryPoint: *mut *mut IComponentType,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.renameEntryPoint)(this.cast(), newName, outEntryPoint) }
    }

    pub unsafe fn linkWithOptions(
        this: *mut Self,
        outLinkedComponentType: *mut *mut IComponentType,
        compilerOptionEntryCount: u32,
        compilerOptionEntries: *const CompilerOptionEntry,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.linkWithOptions)(
                this.cast(),
                outLinkedComponentType,
                compilerOptionEntryCount,
                compilerOptionEntries,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getTargetCode(
        this: *mut Self,
        targetIndex: SlangInt,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getTargetCode)(
                this.cast(),
                targetIndex,
                outCode,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getTargetMetadata(
        this: *mut Self,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getTargetMetadata)(
                this.cast(),
                targetIndex,
                outMetadata,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getEntryPointMetadata(
        this: *mut Self,
        entryPointIndex: SlangInt,
        targetIndex: SlangInt,
        outMetadata: *mut *mut IMetadata,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).base.getEntryPointMetadata)(
                this.cast(),
                entryPointIndex,
                targetIndex,
                outMetadata,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn findEntryPointByName(
        this: *mut Self,
        name: *const ::core::ffi::c_char,
        outEntryPoint: *mut *mut IEntryPoint,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).findEntryPointByName)(this, name, outEntryPoint) }
    }

    pub unsafe fn getDefinedEntryPointCount(this: *mut Self) -> SlangInt32 {
        unsafe { ((*(*this).vtable).getDefinedEntryPointCount)(this) }
    }

    pub unsafe fn getDefinedEntryPoint(
        this: *mut Self,
        index: SlangInt32,
        outEntryPoint: *mut *mut IEntryPoint,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getDefinedEntryPoint)(this, index, outEntryPoint) }
    }

    pub unsafe fn serialize(
        this: *mut Self,
        outSerializedBlob: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).serialize)(this, outSerializedBlob) }
    }

    pub unsafe fn writeToFile(
        this: *mut Self,
        fileName: *const ::core::ffi::c_char,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).writeToFile)(this, fileName) }
    }

    pub unsafe fn getName(this: *mut Self) -> *const ::core::ffi::c_char {
        unsafe { ((*(*this).vtable).getName)(this) }
    }

    pub unsafe fn getFilePath(this: *mut Self) -> *const ::core::ffi::c_char {
        unsafe { ((*(*this).vtable).getFilePath)(this) }
    }

    pub unsafe fn getUniqueIdentity(this: *mut Self) -> *const ::core::ffi::c_char {
        unsafe { ((*(*this).vtable).getUniqueIdentity)(this) }
    }

    pub unsafe fn findAndCheckEntryPoint(
        this: *mut Self,
        name: *const ::core::ffi::c_char,
        stage: SlangStage,
        outEntryPoint: *mut *mut IEntryPoint,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).findAndCheckEntryPoint)(
                this,
                name,
                stage,
                outEntryPoint,
                outDiagnostics,
            )
        }
    }

    pub unsafe fn getDependencyFileCount(this: *mut Self) -> SlangInt32 {
        unsafe { ((*(*this).vtable).getDependencyFileCount)(this) }
    }

    pub unsafe fn getDependencyFilePath(
        this: *mut Self,
        index: SlangInt32,
    ) -> *const ::core::ffi::c_char {
        unsafe { ((*(*this).vtable).getDependencyFilePath)(this, index) }
    }

    pub unsafe fn getModuleReflection(this: *mut Self) -> *mut DeclReflection {
        unsafe { ((*(*this).vtable).getModuleReflection)(this) }
    }

    pub unsafe fn disassemble(
        this: *mut Self,
        outDisassembledBlob: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).disassemble)(this, outDisassembledBlob) }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct IModulePrecompileService_Experimental {
    pub vtable: *const IModulePrecompileService_ExperimentalVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IModulePrecompileService_ExperimentalVtbl {
    pub base: ISlangUnknownVtbl,
    pub precompileForTarget: unsafe extern "system" fn(
        this: *mut IModulePrecompileService_Experimental,
        target: SlangCompileTarget,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult,
    pub getPrecompiledTargetCode: unsafe extern "system" fn(
        this: *mut IModulePrecompileService_Experimental,
        target: SlangCompileTarget,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
    pub getModuleDependencyCount:
        unsafe extern "system" fn(this: *mut IModulePrecompileService_Experimental) -> SlangInt,
    pub getModuleDependency: unsafe extern "system" fn(
        this: *mut IModulePrecompileService_Experimental,
        dependencyIndex: SlangInt,
        outModule: *mut *mut IModule,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult,
}

unsafe impl Interface for IModulePrecompileService_Experimental {
    const UUID: SlangUUID = SlangUUID {
        data1: 0x8E12_E8E3,
        data2: 0x5FCD,
        data3: 0x433E,
        data4: [0xAF, 0xCB, 0x13, 0xA0, 0x88, 0xBC, 0x5E, 0xE5],
    };
}

impl IModulePrecompileService_Experimental {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn precompileForTarget(
        this: *mut Self,
        target: SlangCompileTarget,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).precompileForTarget)(this, target, outDiagnostics) }
    }

    pub unsafe fn getPrecompiledTargetCode(
        this: *mut Self,
        target: SlangCompileTarget,
        outCode: *mut *mut IBlob,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getPrecompiledTargetCode)(this, target, outCode, outDiagnostics)
        }
    }

    pub unsafe fn getModuleDependencyCount(this: *mut Self) -> SlangInt {
        unsafe { ((*(*this).vtable).getModuleDependencyCount)(this) }
    }

    pub unsafe fn getModuleDependency(
        this: *mut Self,
        dependencyIndex: SlangInt,
        outModule: *mut *mut IModule,
        outDiagnostics: *mut *mut IBlob,
    ) -> SlangResult {
        unsafe {
            ((*(*this).vtable).getModuleDependency)(
                this,
                dependencyIndex,
                outModule,
                outDiagnostics,
            )
        }
    }
}

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SpecializationArg_Kind(pub i32);

impl SpecializationArg_Kind {
    pub const Unknown: Self = Self(0);
    pub const Type: Self = Self(1);
    pub const Expr: Self = Self(2);
}

#[repr(C)]
#[derive(Clone, Copy)]
pub union SpecializationArg_anon0 {
    pub r#type: *mut TypeReflection,
    pub expr: *const ::core::ffi::c_char,
}

impl ::core::fmt::Debug for SpecializationArg_anon0 {
    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
        f.debug_struct("SpecializationArg_anon0")
            .finish_non_exhaustive()
    }
}

const _: () = {
    assert!(size_of::<SpecializationArg_anon0>() == 8);
    assert!(align_of::<SpecializationArg_anon0>() == 8);
    assert!(::core::mem::offset_of!(SpecializationArg_anon0, r#type) == 0);
    assert!(::core::mem::offset_of!(SpecializationArg_anon0, expr) == 0);
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SpecializationArg {
    pub kind: SpecializationArg_Kind,
    pub anon0: SpecializationArg_anon0,
}

const _: () = {
    assert!(size_of::<SpecializationArg>() == 16);
    assert!(align_of::<SpecializationArg>() == 8);
    assert!(::core::mem::offset_of!(SpecializationArg, kind) == 0);
    assert!(::core::mem::offset_of!(SpecializationArg, anon0) == 8);
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SlangLanguageVersion(pub u32);

impl SlangLanguageVersion {
    pub const SLANG_LANGUAGE_VERSION_UNKNOWN: Self = Self(0);
    pub const SLANG_LANGUAGE_VERSION_LEGACY: Self = Self(2018);
    pub const SLANG_LANGUAGE_VERSION_202A: Self = Self(2025);
    pub const SLANG_LANGUAGE_VERSION_2025: Self = Self(2025);
    pub const SLANG_LANGUAGE_VERSION_202B: Self = Self(2026);
    pub const SLANG_LANGUAGE_VERSION_2026: Self = Self(2026);
    pub const SLANG_LANGUAGE_VERSION_202C: Self = Self(2027);
    pub const SLANG_LANGAUGE_VERSION_DEFAULT: Self = Self(2018);
    pub const SLANG_LANGUAGE_VERSION_DEFAULT: Self = Self(2018);
    pub const SLANG_LANGUAGE_VERSION_LATEST: Self = Self(2026);
    pub const SLANG_LANGUAGE_VERSION_NEXT: Self = Self(2027);
}

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct SlangGlobalSessionDesc {
    pub structureSize: u32,
    pub apiVersion: u32,
    pub minLanguageVersion: u32,
    pub enableGLSL: bool,
    pub reserved: [u32; 16],
}

impl Default for SlangGlobalSessionDesc {
    fn default() -> Self {
        Self {
            structureSize: 80,
            apiVersion: 0,
            minLanguageVersion: 2025,
            enableGLSL: false,
            reserved: [0; 16],
        }
    }
}

const _: () = {
    assert!(size_of::<SlangGlobalSessionDesc>() == 80);
    assert!(align_of::<SlangGlobalSessionDesc>() == 4);
    assert!(::core::mem::offset_of!(SlangGlobalSessionDesc, structureSize) == 0);
    assert!(::core::mem::offset_of!(SlangGlobalSessionDesc, apiVersion) == 4);
    assert!(::core::mem::offset_of!(SlangGlobalSessionDesc, minLanguageVersion) == 8);
    assert!(::core::mem::offset_of!(SlangGlobalSessionDesc, enableGLSL) == 12);
    assert!(::core::mem::offset_of!(SlangGlobalSessionDesc, reserved) == 16);
};

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct OperandDataType(pub ::core::ffi::c_int);

impl OperandDataType {
    pub const General: Self = Self(0);
    pub const Int32: Self = Self(1);
    pub const Int64: Self = Self(2);
    pub const Float32: Self = Self(3);
    pub const Float64: Self = Self(4);
    pub const String: Self = Self(5);
}

pub type VMExtFunction = Option<
    unsafe extern "C" fn(*mut IByteCodeRunner, *mut VMExecInstHeader, *mut ::core::ffi::c_void),
>;

pub type VMPrintFunc =
    Option<unsafe extern "C" fn(*const ::core::ffi::c_char, *mut ::core::ffi::c_void)>;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VMExecInstHeader {
    pub functionPtr: VMExtFunction,
    pub opcodeExtension: u32,
    pub operandCount: u32,
}

const _: () = {
    assert!(size_of::<VMExecInstHeader>() == 16);
    assert!(align_of::<VMExecInstHeader>() == 8);
    assert!(::core::mem::offset_of!(VMExecInstHeader, functionPtr) == 0);
    assert!(::core::mem::offset_of!(VMExecInstHeader, opcodeExtension) == 8);
    assert!(::core::mem::offset_of!(VMExecInstHeader, operandCount) == 12);
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ByteCodeFuncInfo {
    pub parameterCount: u32,
    pub returnValueSize: u32,
}

const _: () = {
    assert!(size_of::<ByteCodeFuncInfo>() == 8);
    assert!(align_of::<ByteCodeFuncInfo>() == 4);
    assert!(::core::mem::offset_of!(ByteCodeFuncInfo, parameterCount) == 0);
    assert!(::core::mem::offset_of!(ByteCodeFuncInfo, returnValueSize) == 4);
};

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct ByteCodeRunnerDesc {
    pub structSize: usize,
}

impl Default for ByteCodeRunnerDesc {
    fn default() -> Self {
        Self { structSize: 8 }
    }
}

const _: () = {
    assert!(size_of::<ByteCodeRunnerDesc>() == 8);
    assert!(align_of::<ByteCodeRunnerDesc>() == 8);
    assert!(::core::mem::offset_of!(ByteCodeRunnerDesc, structSize) == 0);
};

#[repr(C)]
#[derive(Debug)]
pub struct IByteCodeRunner {
    pub vtable: *const IByteCodeRunnerVtbl,
}

#[repr(C)]
#[derive(Debug)]
pub struct IByteCodeRunnerVtbl {
    pub base: ISlangUnknownVtbl,
    pub loadModule: unsafe extern "system" fn(
        this: *mut IByteCodeRunner,
        moduleBlob: *mut IBlob,
    ) -> SlangResult,
    pub selectFunctionByIndex:
        unsafe extern "system" fn(this: *mut IByteCodeRunner, functionIndex: u32) -> SlangResult,
    pub findFunctionByName: unsafe extern "system" fn(
        this: *mut IByteCodeRunner,
        name: *const ::core::ffi::c_char,
    ) -> ::core::ffi::c_int,
    pub getFunctionInfo: unsafe extern "system" fn(
        this: *mut IByteCodeRunner,
        index: u32,
        outInfo: *mut ByteCodeFuncInfo,
    ) -> SlangResult,
    pub getCurrentWorkingSet:
        unsafe extern "system" fn(this: *mut IByteCodeRunner) -> *mut ::core::ffi::c_void,
    pub execute: unsafe extern "system" fn(
        this: *mut IByteCodeRunner,
        argumentData: *mut ::core::ffi::c_void,
        argumentSize: usize,
    ) -> SlangResult,
    pub getErrorString:
        unsafe extern "system" fn(this: *mut IByteCodeRunner, outBlob: *mut *mut IBlob),
    pub getReturnValue: unsafe extern "system" fn(
        this: *mut IByteCodeRunner,
        outValueSize: *mut usize,
    ) -> *mut ::core::ffi::c_void,
    pub setExtInstHandlerUserData:
        unsafe extern "system" fn(this: *mut IByteCodeRunner, userData: *mut ::core::ffi::c_void),
    pub registerExtCall: unsafe extern "system" fn(
        this: *mut IByteCodeRunner,
        name: *const ::core::ffi::c_char,
        functionPtr: VMExtFunction,
    ) -> SlangResult,
    pub setPrintCallback: unsafe extern "system" fn(
        this: *mut IByteCodeRunner,
        callback: VMPrintFunc,
        userData: *mut ::core::ffi::c_void,
    ) -> SlangResult,
}

unsafe impl Interface for IByteCodeRunner {
    const UUID: SlangUUID = SlangUUID {
        data1: 0xAFDA_B195,
        data2: 0x361F,
        data3: 0x42CB,
        data4: [0x95, 0x13, 0x90, 0x06, 0x26, 0x1D, 0xD8, 0xCD],
    };
}

impl IByteCodeRunner {
    pub unsafe fn queryInterface(
        this: *mut Self,
        uuid: *const SlangUUID,
        outObject: *mut *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).base.queryInterface)(this.cast(), uuid, outObject) }
    }

    pub unsafe fn addRef(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.addRef)(this.cast()) }
    }

    pub unsafe fn release(this: *mut Self) -> u32 {
        unsafe { ((*(*this).vtable).base.release)(this.cast()) }
    }

    pub unsafe fn loadModule(this: *mut Self, moduleBlob: *mut IBlob) -> SlangResult {
        unsafe { ((*(*this).vtable).loadModule)(this, moduleBlob) }
    }

    pub unsafe fn selectFunctionByIndex(this: *mut Self, functionIndex: u32) -> SlangResult {
        unsafe { ((*(*this).vtable).selectFunctionByIndex)(this, functionIndex) }
    }

    pub unsafe fn findFunctionByName(
        this: *mut Self,
        name: *const ::core::ffi::c_char,
    ) -> ::core::ffi::c_int {
        unsafe { ((*(*this).vtable).findFunctionByName)(this, name) }
    }

    pub unsafe fn getFunctionInfo(
        this: *mut Self,
        index: u32,
        outInfo: *mut ByteCodeFuncInfo,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).getFunctionInfo)(this, index, outInfo) }
    }

    pub unsafe fn getCurrentWorkingSet(this: *mut Self) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).getCurrentWorkingSet)(this) }
    }

    pub unsafe fn execute(
        this: *mut Self,
        argumentData: *mut ::core::ffi::c_void,
        argumentSize: usize,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).execute)(this, argumentData, argumentSize) }
    }

    pub unsafe fn getErrorString(this: *mut Self, outBlob: *mut *mut IBlob) {
        unsafe {
            ((*(*this).vtable).getErrorString)(this, outBlob);
        }
    }

    pub unsafe fn getReturnValue(
        this: *mut Self,
        outValueSize: *mut usize,
    ) -> *mut ::core::ffi::c_void {
        unsafe { ((*(*this).vtable).getReturnValue)(this, outValueSize) }
    }

    pub unsafe fn setExtInstHandlerUserData(this: *mut Self, userData: *mut ::core::ffi::c_void) {
        unsafe {
            ((*(*this).vtable).setExtInstHandlerUserData)(this, userData);
        }
    }

    pub unsafe fn registerExtCall(
        this: *mut Self,
        name: *const ::core::ffi::c_char,
        functionPtr: VMExtFunction,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).registerExtCall)(this, name, functionPtr) }
    }

    pub unsafe fn setPrintCallback(
        this: *mut Self,
        callback: VMPrintFunc,
        userData: *mut ::core::ffi::c_void,
    ) -> SlangResult {
        unsafe { ((*(*this).vtable).setPrintCallback)(this, callback, userData) }
    }
}

pub const SLANG_FACILITY_WIN_GENERAL: i32 = 0;

pub const SLANG_FACILITY_WIN_INTERFACE: i32 = 4;

pub const SLANG_FACILITY_WIN_API: i32 = 7;

pub const SLANG_FACILITY_BASE: i32 = 512;

pub const SLANG_FACILITY_CORE: i32 = 512;

pub const SLANG_FACILITY_INTERNAL: i32 = 513;

pub const SLANG_FACILITY_EXTERNAL_BASE: i32 = 528;

pub const SLANG_UNBOUNDED_SIZE: usize = 18_446_744_073_709_551_615;

pub const SLANG_UNKNOWN_SIZE: usize = 18_446_744_073_709_551_614;

pub const SLANG_API_VERSION: u32 = 0;

#[repr(C)]
#[derive(Debug)]
pub struct DeclReflection {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct EntryPointReflection {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct FunctionReflection {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct ICompileRequest {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct ShaderReflection {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct TypeLayoutReflection {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

#[repr(C)]
#[derive(Debug)]
pub struct TypeReflection {
    _data: [u8; 0],
    _marker: ::core::marker::PhantomData<(*mut u8, ::core::marker::PhantomPinned)>,
}

unsafe extern "C" {
    pub fn spGetBuildTagString() -> *const ::core::ffi::c_char;
    pub fn spReflectionUserAttribute_GetName(
        attrib: *mut SlangReflectionUserAttribute,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflectionUserAttribute_GetArgumentCount(
        attrib: *mut SlangReflectionUserAttribute,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionUserAttribute_GetArgumentType(
        attrib: *mut SlangReflectionUserAttribute,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionUserAttribute_GetArgumentValueInt(
        attrib: *mut SlangReflectionUserAttribute,
        index: ::core::ffi::c_uint,
        rs: *mut ::core::ffi::c_int,
    ) -> SlangResult;
    pub fn spReflectionUserAttribute_GetArgumentValueFloat(
        attrib: *mut SlangReflectionUserAttribute,
        index: ::core::ffi::c_uint,
        rs: *mut f32,
    ) -> SlangResult;
    pub fn spReflectionUserAttribute_GetArgumentValueString(
        attrib: *mut SlangReflectionUserAttribute,
        index: ::core::ffi::c_uint,
        outSize: *mut usize,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflectionType_GetKind(r#type: *mut SlangReflectionType) -> SlangTypeKind;
    pub fn spReflectionType_GetUserAttributeCount(
        r#type: *mut SlangReflectionType,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionType_GetUserAttribute(
        r#type: *mut SlangReflectionType,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionUserAttribute;
    pub fn spReflectionType_FindUserAttributeByName(
        r#type: *mut SlangReflectionType,
        name: *const ::core::ffi::c_char,
    ) -> *mut SlangReflectionUserAttribute;
    pub fn spReflectionType_applySpecializations(
        r#type: *mut SlangReflectionType,
        generic: *mut SlangReflectionGeneric,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionType_GetFieldCount(r#type: *mut SlangReflectionType) -> ::core::ffi::c_uint;
    pub fn spReflectionType_GetFieldByIndex(
        r#type: *mut SlangReflectionType,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionVariable;
    pub fn spReflectionType_GetElementCount(r#type: *mut SlangReflectionType) -> usize;
    pub fn spReflectionType_GetSpecializedElementCount(
        r#type: *mut SlangReflectionType,
        reflection: *mut SlangReflection,
    ) -> usize;
    pub fn spReflectionType_GetElementType(
        r#type: *mut SlangReflectionType,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionType_GetRowCount(r#type: *mut SlangReflectionType) -> ::core::ffi::c_uint;
    pub fn spReflectionType_GetColumnCount(r#type: *mut SlangReflectionType)
    -> ::core::ffi::c_uint;
    pub fn spReflectionType_GetScalarType(r#type: *mut SlangReflectionType) -> SlangScalarType;
    pub fn spReflectionType_GetResourceShape(
        r#type: *mut SlangReflectionType,
    ) -> SlangResourceShape;
    pub fn spReflectionType_GetResourceAccess(
        r#type: *mut SlangReflectionType,
    ) -> SlangResourceAccess;
    pub fn spReflectionType_GetResourceResultType(
        r#type: *mut SlangReflectionType,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionType_GetName(r#type: *mut SlangReflectionType)
    -> *const ::core::ffi::c_char;
    pub fn spReflectionType_GetFullName(
        r#type: *mut SlangReflectionType,
        outNameBlob: *mut *mut ISlangBlob,
    ) -> SlangResult;
    pub fn spReflectionType_GetGenericContainer(
        r#type: *mut SlangReflectionType,
    ) -> *mut SlangReflectionGeneric;
    pub fn spReflectionTypeLayout_GetType(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionTypeLayout_getKind(r#type: *mut SlangReflectionTypeLayout) -> SlangTypeKind;
    pub fn spReflectionTypeLayout_GetSize(
        r#type: *mut SlangReflectionTypeLayout,
        category: SlangParameterCategory,
    ) -> usize;
    pub fn spReflectionTypeLayout_GetStride(
        r#type: *mut SlangReflectionTypeLayout,
        category: SlangParameterCategory,
    ) -> usize;
    pub fn spReflectionTypeLayout_getAlignment(
        r#type: *mut SlangReflectionTypeLayout,
        category: SlangParameterCategory,
    ) -> i32;
    pub fn spReflectionTypeLayout_GetFieldCount(r#type: *mut SlangReflectionTypeLayout) -> u32;
    pub fn spReflectionTypeLayout_GetFieldByIndex(
        r#type: *mut SlangReflectionTypeLayout,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionTypeLayout_findFieldIndexByName(
        typeLayout: *mut SlangReflectionTypeLayout,
        nameBegin: *const ::core::ffi::c_char,
        nameEnd: *const ::core::ffi::c_char,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_GetExplicitCounter(
        typeLayout: *mut SlangReflectionTypeLayout,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionTypeLayout_GetElementStride(
        r#type: *mut SlangReflectionTypeLayout,
        category: SlangParameterCategory,
    ) -> usize;
    pub fn spReflectionTypeLayout_GetElementTypeLayout(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> *mut SlangReflectionTypeLayout;
    pub fn spReflectionTypeLayout_GetElementVarLayout(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionTypeLayout_getContainerVarLayout(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionTypeLayout_GetParameterCategory(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> SlangParameterCategory;
    pub fn spReflectionTypeLayout_GetCategoryCount(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionTypeLayout_GetCategoryByIndex(
        r#type: *mut SlangReflectionTypeLayout,
        index: ::core::ffi::c_uint,
    ) -> SlangParameterCategory;
    pub fn spReflectionTypeLayout_GetMatrixLayoutMode(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> SlangMatrixLayoutMode;
    pub fn spReflectionTypeLayout_getGenericParamIndex(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> ::core::ffi::c_int;
    pub fn spReflectionTypeLayout_getPendingDataTypeLayout(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> *mut SlangReflectionTypeLayout;
    pub fn spReflectionTypeLayout_getSpecializedTypePendingDataVarLayout(
        r#type: *mut SlangReflectionTypeLayout,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionType_getSpecializedTypeArgCount(
        r#type: *mut SlangReflectionType,
    ) -> SlangInt;
    pub fn spReflectionType_getSpecializedTypeArgType(
        r#type: *mut SlangReflectionType,
        index: SlangInt,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionTypeLayout_getBindingRangeCount(
        typeLayout: *mut SlangReflectionTypeLayout,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getBindingRangeType(
        typeLayout: *mut SlangReflectionTypeLayout,
        index: SlangInt,
    ) -> SlangBindingType;
    pub fn spReflectionTypeLayout_isBindingRangeSpecializable(
        typeLayout: *mut SlangReflectionTypeLayout,
        index: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getBindingRangeBindingCount(
        typeLayout: *mut SlangReflectionTypeLayout,
        index: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getBindingRangeLeafTypeLayout(
        typeLayout: *mut SlangReflectionTypeLayout,
        index: SlangInt,
    ) -> *mut SlangReflectionTypeLayout;
    pub fn spReflectionTypeLayout_getBindingRangeLeafVariable(
        typeLayout: *mut SlangReflectionTypeLayout,
        index: SlangInt,
    ) -> *mut SlangReflectionVariable;
    pub fn spReflectionTypeLayout_getBindingRangeImageFormat(
        typeLayout: *mut SlangReflectionTypeLayout,
        index: SlangInt,
    ) -> SlangImageFormat;
    pub fn spReflectionTypeLayout_getFieldBindingRangeOffset(
        typeLayout: *mut SlangReflectionTypeLayout,
        fieldIndex: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getExplicitCounterBindingRangeOffset(
        inTypeLayout: *mut SlangReflectionTypeLayout,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getBindingRangeDescriptorSetIndex(
        typeLayout: *mut SlangReflectionTypeLayout,
        index: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getBindingRangeFirstDescriptorRangeIndex(
        typeLayout: *mut SlangReflectionTypeLayout,
        index: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getBindingRangeDescriptorRangeCount(
        typeLayout: *mut SlangReflectionTypeLayout,
        index: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getDescriptorSetCount(
        typeLayout: *mut SlangReflectionTypeLayout,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getDescriptorSetSpaceOffset(
        typeLayout: *mut SlangReflectionTypeLayout,
        setIndex: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getDescriptorSetDescriptorRangeCount(
        typeLayout: *mut SlangReflectionTypeLayout,
        setIndex: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getDescriptorSetDescriptorRangeIndexOffset(
        typeLayout: *mut SlangReflectionTypeLayout,
        setIndex: SlangInt,
        rangeIndex: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getDescriptorSetDescriptorRangeDescriptorCount(
        typeLayout: *mut SlangReflectionTypeLayout,
        setIndex: SlangInt,
        rangeIndex: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getDescriptorSetDescriptorRangeType(
        typeLayout: *mut SlangReflectionTypeLayout,
        setIndex: SlangInt,
        rangeIndex: SlangInt,
    ) -> SlangBindingType;
    pub fn spReflectionTypeLayout_getDescriptorSetDescriptorRangeCategory(
        typeLayout: *mut SlangReflectionTypeLayout,
        setIndex: SlangInt,
        rangeIndex: SlangInt,
    ) -> SlangParameterCategory;
    pub fn spReflectionTypeLayout_getSubObjectRangeCount(
        typeLayout: *mut SlangReflectionTypeLayout,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getSubObjectRangeBindingRangeIndex(
        typeLayout: *mut SlangReflectionTypeLayout,
        subObjectRangeIndex: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getSubObjectRangeSpaceOffset(
        typeLayout: *mut SlangReflectionTypeLayout,
        subObjectRangeIndex: SlangInt,
    ) -> SlangInt;
    pub fn spReflectionTypeLayout_getSubObjectRangeOffset(
        typeLayout: *mut SlangReflectionTypeLayout,
        subObjectRangeIndex: SlangInt,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionVariable_GetName(
        var: *mut SlangReflectionVariable,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflectionVariable_GetType(
        var: *mut SlangReflectionVariable,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionVariable_FindModifier(
        var: *mut SlangReflectionVariable,
        modifierID: SlangModifierID,
    ) -> *mut SlangReflectionModifier;
    pub fn spReflectionVariable_GetUserAttributeCount(
        var: *mut SlangReflectionVariable,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionVariable_GetUserAttribute(
        var: *mut SlangReflectionVariable,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionUserAttribute;
    pub fn spReflectionVariable_FindUserAttributeByName(
        var: *mut SlangReflectionVariable,
        globalSession: *mut SlangSession,
        name: *const ::core::ffi::c_char,
    ) -> *mut SlangReflectionUserAttribute;
    pub fn spReflectionVariable_HasDefaultValue(inVar: *mut SlangReflectionVariable) -> bool;
    pub fn spReflectionVariable_GetDefaultValueInt(
        inVar: *mut SlangReflectionVariable,
        rs: *mut i64,
    ) -> SlangResult;
    pub fn spReflectionVariable_GetDefaultValueFloat(
        inVar: *mut SlangReflectionVariable,
        rs: *mut f32,
    ) -> SlangResult;
    pub fn spReflectionVariable_GetGenericContainer(
        var: *mut SlangReflectionVariable,
    ) -> *mut SlangReflectionGeneric;
    pub fn spReflectionVariable_applySpecializations(
        var: *mut SlangReflectionVariable,
        generic: *mut SlangReflectionGeneric,
    ) -> *mut SlangReflectionVariable;
    pub fn spReflectionVariableLayout_GetVariable(
        var: *mut SlangReflectionVariableLayout,
    ) -> *mut SlangReflectionVariable;
    pub fn spReflectionVariableLayout_GetTypeLayout(
        var: *mut SlangReflectionVariableLayout,
    ) -> *mut SlangReflectionTypeLayout;
    pub fn spReflectionVariableLayout_GetOffset(
        var: *mut SlangReflectionVariableLayout,
        category: SlangParameterCategory,
    ) -> usize;
    pub fn spReflectionVariableLayout_GetSpace(
        var: *mut SlangReflectionVariableLayout,
        category: SlangParameterCategory,
    ) -> usize;
    pub fn spReflectionVariableLayout_GetImageFormat(
        var: *mut SlangReflectionVariableLayout,
    ) -> SlangImageFormat;
    pub fn spReflectionVariableLayout_GetSemanticName(
        var: *mut SlangReflectionVariableLayout,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflectionVariableLayout_GetSemanticIndex(
        var: *mut SlangReflectionVariableLayout,
    ) -> usize;
    pub fn spReflectionFunction_asDecl(
        func: *mut SlangReflectionFunction,
    ) -> *mut SlangReflectionDecl;
    pub fn spReflectionFunction_GetName(
        func: *mut SlangReflectionFunction,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflectionFunction_FindModifier(
        var: *mut SlangReflectionFunction,
        modifierID: SlangModifierID,
    ) -> *mut SlangReflectionModifier;
    pub fn spReflectionFunction_GetUserAttributeCount(
        func: *mut SlangReflectionFunction,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionFunction_GetUserAttribute(
        func: *mut SlangReflectionFunction,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionUserAttribute;
    pub fn spReflectionFunction_FindUserAttributeByName(
        func: *mut SlangReflectionFunction,
        globalSession: *mut SlangSession,
        name: *const ::core::ffi::c_char,
    ) -> *mut SlangReflectionUserAttribute;
    pub fn spReflectionFunction_GetParameterCount(
        func: *mut SlangReflectionFunction,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionFunction_GetParameter(
        func: *mut SlangReflectionFunction,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionVariable;
    pub fn spReflectionFunction_GetResultType(
        func: *mut SlangReflectionFunction,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionFunction_GetGenericContainer(
        func: *mut SlangReflectionFunction,
    ) -> *mut SlangReflectionGeneric;
    pub fn spReflectionFunction_applySpecializations(
        func: *mut SlangReflectionFunction,
        generic: *mut SlangReflectionGeneric,
    ) -> *mut SlangReflectionFunction;
    pub fn spReflectionFunction_specializeWithArgTypes(
        func: *mut SlangReflectionFunction,
        argTypeCount: SlangInt,
        argTypes: *const *mut SlangReflectionType,
    ) -> *mut SlangReflectionFunction;
    pub fn spReflectionFunction_isOverloaded(func: *mut SlangReflectionFunction) -> bool;
    pub fn spReflectionFunction_getOverloadCount(
        func: *mut SlangReflectionFunction,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionFunction_getOverload(
        func: *mut SlangReflectionFunction,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionFunction;
    pub fn spReflectionDecl_getChildrenCount(
        parentDecl: *mut SlangReflectionDecl,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionDecl_getChild(
        parentDecl: *mut SlangReflectionDecl,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionDecl;
    pub fn spReflectionDecl_getName(decl: *mut SlangReflectionDecl) -> *const ::core::ffi::c_char;
    pub fn spReflectionDecl_getKind(decl: *mut SlangReflectionDecl) -> SlangDeclKind;
    pub fn spReflectionDecl_castToFunction(
        decl: *mut SlangReflectionDecl,
    ) -> *mut SlangReflectionFunction;
    pub fn spReflectionDecl_castToVariable(
        decl: *mut SlangReflectionDecl,
    ) -> *mut SlangReflectionVariable;
    pub fn spReflectionDecl_castToGeneric(
        decl: *mut SlangReflectionDecl,
    ) -> *mut SlangReflectionGeneric;
    pub fn spReflection_getTypeFromDecl(decl: *mut SlangReflectionDecl)
    -> *mut SlangReflectionType;
    pub fn spReflectionDecl_getParent(decl: *mut SlangReflectionDecl) -> *mut SlangReflectionDecl;
    pub fn spReflectionDecl_findModifier(
        decl: *mut SlangReflectionDecl,
        modifierID: SlangModifierID,
    ) -> *mut SlangReflectionModifier;
    pub fn spReflectionGeneric_asDecl(
        generic: *mut SlangReflectionGeneric,
    ) -> *mut SlangReflectionDecl;
    pub fn spReflectionGeneric_GetName(
        generic: *mut SlangReflectionGeneric,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflectionGeneric_GetTypeParameterCount(
        generic: *mut SlangReflectionGeneric,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionGeneric_GetTypeParameter(
        generic: *mut SlangReflectionGeneric,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionVariable;
    pub fn spReflectionGeneric_GetValueParameterCount(
        generic: *mut SlangReflectionGeneric,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionGeneric_GetValueParameter(
        generic: *mut SlangReflectionGeneric,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionVariable;
    pub fn spReflectionGeneric_GetTypeParameterConstraintCount(
        generic: *mut SlangReflectionGeneric,
        typeParam: *mut SlangReflectionVariable,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionGeneric_GetTypeParameterConstraintType(
        generic: *mut SlangReflectionGeneric,
        typeParam: *mut SlangReflectionVariable,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionGeneric_GetInnerKind(generic: *mut SlangReflectionGeneric) -> SlangDeclKind;
    pub fn spReflectionGeneric_GetInnerDecl(
        generic: *mut SlangReflectionGeneric,
    ) -> *mut SlangReflectionDecl;
    pub fn spReflectionGeneric_GetOuterGenericContainer(
        generic: *mut SlangReflectionGeneric,
    ) -> *mut SlangReflectionGeneric;
    pub fn spReflectionGeneric_GetConcreteType(
        generic: *mut SlangReflectionGeneric,
        typeParam: *mut SlangReflectionVariable,
    ) -> *mut SlangReflectionType;
    pub fn spReflectionGeneric_GetConcreteIntVal(
        generic: *mut SlangReflectionGeneric,
        valueParam: *mut SlangReflectionVariable,
    ) -> i64;
    pub fn spReflectionGeneric_applySpecializations(
        currGeneric: *mut SlangReflectionGeneric,
        generic: *mut SlangReflectionGeneric,
    ) -> *mut SlangReflectionGeneric;
    pub fn spReflectionVariableLayout_getStage(
        var: *mut SlangReflectionVariableLayout,
    ) -> SlangStage;
    pub fn spReflectionVariableLayout_getPendingDataLayout(
        var: *mut SlangReflectionVariableLayout,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionParameter_GetBindingIndex(
        parameter: *mut SlangReflectionParameter,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionParameter_GetBindingSpace(
        parameter: *mut SlangReflectionParameter,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionEntryPoint_getName(
        entryPoint: *mut SlangReflectionEntryPoint,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflectionEntryPoint_getNameOverride(
        entryPoint: *mut SlangReflectionEntryPoint,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflectionEntryPoint_getFunction(
        entryPoint: *mut SlangReflectionEntryPoint,
    ) -> *mut SlangReflectionFunction;
    pub fn spReflectionEntryPoint_getParameterCount(
        entryPoint: *mut SlangReflectionEntryPoint,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionEntryPoint_getParameterByIndex(
        entryPoint: *mut SlangReflectionEntryPoint,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionEntryPoint_getStage(
        entryPoint: *mut SlangReflectionEntryPoint,
    ) -> SlangStage;
    pub fn spReflectionEntryPoint_getComputeThreadGroupSize(
        entryPoint: *mut SlangReflectionEntryPoint,
        axisCount: SlangUInt,
        outSizeAlongAxis: *mut SlangUInt,
    );
    pub fn spReflectionEntryPoint_getComputeWaveSize(
        entryPoint: *mut SlangReflectionEntryPoint,
        outWaveSize: *mut SlangUInt,
    );
    pub fn spReflectionEntryPoint_usesAnySampleRateInput(
        entryPoint: *mut SlangReflectionEntryPoint,
    ) -> ::core::ffi::c_int;
    pub fn spReflectionEntryPoint_getVarLayout(
        entryPoint: *mut SlangReflectionEntryPoint,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionEntryPoint_getResultVarLayout(
        entryPoint: *mut SlangReflectionEntryPoint,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflectionEntryPoint_hasDefaultConstantBuffer(
        entryPoint: *mut SlangReflectionEntryPoint,
    ) -> ::core::ffi::c_int;
    pub fn spReflectionTypeParameter_GetName(
        typeParam: *mut SlangReflectionTypeParameter,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflectionTypeParameter_GetIndex(
        typeParam: *mut SlangReflectionTypeParameter,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionTypeParameter_GetConstraintCount(
        typeParam: *mut SlangReflectionTypeParameter,
    ) -> ::core::ffi::c_uint;
    pub fn spReflectionTypeParameter_GetConstraintByIndex(
        typeParam: *mut SlangReflectionTypeParameter,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionType;
    pub fn spReflection_ToJson(
        reflection: *mut SlangReflection,
        request: *mut SlangCompileRequest,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult;
    pub fn spReflection_GetParameterCount(reflection: *mut SlangReflection) -> ::core::ffi::c_uint;
    pub fn spReflection_GetParameterByIndex(
        reflection: *mut SlangReflection,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionParameter;
    pub fn spReflection_GetTypeParameterCount(
        reflection: *mut SlangReflection,
    ) -> ::core::ffi::c_uint;
    pub fn spReflection_GetTypeParameterByIndex(
        reflection: *mut SlangReflection,
        index: ::core::ffi::c_uint,
    ) -> *mut SlangReflectionTypeParameter;
    pub fn spReflection_FindTypeParameter(
        reflection: *mut SlangReflection,
        name: *const ::core::ffi::c_char,
    ) -> *mut SlangReflectionTypeParameter;
    pub fn spReflection_FindTypeByName(
        reflection: *mut SlangReflection,
        name: *const ::core::ffi::c_char,
    ) -> *mut SlangReflectionType;
    pub fn spReflection_GetTypeLayout(
        reflection: *mut SlangReflection,
        reflectionType: *mut SlangReflectionType,
        rules: SlangLayoutRules,
    ) -> *mut SlangReflectionTypeLayout;
    pub fn spReflection_FindFunctionByName(
        reflection: *mut SlangReflection,
        name: *const ::core::ffi::c_char,
    ) -> *mut SlangReflectionFunction;
    pub fn spReflection_FindFunctionByNameInType(
        reflection: *mut SlangReflection,
        reflType: *mut SlangReflectionType,
        name: *const ::core::ffi::c_char,
    ) -> *mut SlangReflectionFunction;
    pub fn spReflection_FindVarByNameInType(
        reflection: *mut SlangReflection,
        reflType: *mut SlangReflectionType,
        name: *const ::core::ffi::c_char,
    ) -> *mut SlangReflectionVariable;
    pub fn spReflection_TryResolveOverloadedFunction(
        reflection: *mut SlangReflection,
        candidateCount: u32,
        candidates: *mut *mut SlangReflectionFunction,
    ) -> *mut SlangReflectionFunction;
    pub fn spReflection_getEntryPointCount(reflection: *mut SlangReflection) -> SlangUInt;
    pub fn spReflection_getEntryPointByIndex(
        reflection: *mut SlangReflection,
        index: SlangUInt,
    ) -> *mut SlangReflectionEntryPoint;
    pub fn spReflection_findEntryPointByName(
        reflection: *mut SlangReflection,
        name: *const ::core::ffi::c_char,
    ) -> *mut SlangReflectionEntryPoint;
    pub fn spReflection_getGlobalConstantBufferBinding(
        reflection: *mut SlangReflection,
    ) -> SlangUInt;
    pub fn spReflection_getGlobalConstantBufferSize(reflection: *mut SlangReflection) -> usize;
    pub fn spReflection_specializeType(
        reflection: *mut SlangReflection,
        r#type: *mut SlangReflectionType,
        specializationArgCount: SlangInt,
        specializationArgs: *const *mut SlangReflectionType,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut SlangReflectionType;
    pub fn spReflection_specializeGeneric(
        inProgramLayout: *mut SlangReflection,
        generic: *mut SlangReflectionGeneric,
        argCount: SlangInt,
        argTypes: *const SlangReflectionGenericArgType,
        args: *const SlangReflectionGenericArg,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut SlangReflectionGeneric;
    pub fn spReflection_isSubType(
        reflection: *mut SlangReflection,
        subType: *mut SlangReflectionType,
        superType: *mut SlangReflectionType,
    ) -> bool;
    pub fn spReflection_getHashedStringCount(reflection: *mut SlangReflection) -> SlangUInt;
    pub fn spReflection_getHashedString(
        reflection: *mut SlangReflection,
        index: SlangUInt,
        outCount: *mut usize,
    ) -> *const ::core::ffi::c_char;
    pub fn spReflection_getGlobalParamsTypeLayout(
        reflection: *mut SlangReflection,
    ) -> *mut SlangReflectionTypeLayout;
    pub fn spReflection_getGlobalParamsVarLayout(
        reflection: *mut SlangReflection,
    ) -> *mut SlangReflectionVariableLayout;
    pub fn spReflection_getBindlessSpaceIndex(reflection: *mut SlangReflection) -> SlangInt;
    pub fn slang_createBlob(data: *const ::core::ffi::c_void, size: usize) -> *mut ISlangBlob;
    pub fn slang_writeCoverageManifestJson(
        metadata: *mut ICoverageTracingMetadata,
        outBlob: *mut *mut ISlangBlob,
    ) -> SlangResult;
    pub fn slang_loadModuleFromSource(
        session: *mut ISession,
        moduleName: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        source: *const ::core::ffi::c_char,
        sourceSize: usize,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut IModule;
    pub fn slang_loadModuleFromIRBlob(
        session: *mut ISession,
        moduleName: *const ::core::ffi::c_char,
        path: *const ::core::ffi::c_char,
        source: *const ::core::ffi::c_void,
        sourceSize: usize,
        outDiagnostics: *mut *mut ISlangBlob,
    ) -> *mut IModule;
    pub fn slang_loadModuleInfoFromIRBlob(
        session: *mut ISession,
        source: *const ::core::ffi::c_void,
        sourceSize: usize,
        outModuleVersion: *mut SlangInt,
        outModuleCompilerVersion: *mut *const ::core::ffi::c_char,
        outModuleName: *mut *const ::core::ffi::c_char,
    ) -> SlangResult;
    pub fn slang_createGlobalSession(
        apiVersion: SlangInt,
        outGlobalSession: *mut *mut IGlobalSession,
    ) -> SlangResult;
    pub fn slang_createGlobalSession2(
        desc: *const SlangGlobalSessionDesc,
        outGlobalSession: *mut *mut IGlobalSession,
    ) -> SlangResult;
    pub fn slang_createGlobalSessionWithoutCoreModule(
        apiVersion: SlangInt,
        outGlobalSession: *mut *mut IGlobalSession,
    ) -> SlangResult;
    pub fn slang_shutdown();
    pub fn slang_enableRecordLayer(enable: bool);
    pub fn slang_isRecordLayerEnabled() -> bool;
    pub fn slang_setReplayDirectory(path: *const ::core::ffi::c_char);
    pub fn slang_getReplayDirectory() -> *const ::core::ffi::c_char;
    pub fn slang_getCurrentReplayPath() -> *const ::core::ffi::c_char;
    pub fn slang_loadReplay(folderPath: *const ::core::ffi::c_char) -> SlangResult;
    pub fn slang_loadLatestReplay() -> SlangResult;
    pub fn slang_replayMarker(label: *const ::core::ffi::c_char);
    pub fn slang_getLastInternalErrorMessage() -> *const ::core::ffi::c_char;
    pub fn slang_createByteCodeRunner(
        desc: *const ByteCodeRunnerDesc,
        outByteCodeRunner: *mut *mut IByteCodeRunner,
    ) -> SlangResult;
    pub fn slang_disassembleByteCode(
        moduleBlob: *mut IBlob,
        outDisassemblyBlob: *mut *mut IBlob,
    ) -> SlangResult;
}
