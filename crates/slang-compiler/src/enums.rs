use slang_compiler_sys::{
    SlangCompileTarget, SlangDebugInfoLevel, SlangFloatingPointMode, SlangLanguageVersion,
    SlangLineDirectiveMode, SlangMatrixLayoutMode, SlangOptimizationLevel, SlangStage,
};

/// A format of code that Slang can generate, which this crate calls a target
/// format.
///
/// A shader is a program that runs on a graphics processing unit (GPU), and
/// each graphics API takes shaders in its own format. Choose the format of
/// the API that runs the code, such as [`TargetFormat::Spirv`] for Vulkan.
/// The enumeration lists the formats that this version supports, which need
/// no downstream compiler, a separate program or library that Slang calls
/// for some formats, beyond what the Vulkan SDK ships. It is non-exhaustive,
/// so a later version can add formats, and a `match` on it needs a wildcard
/// arm.
///
/// # Examples
///
/// See the example of
/// [`GlobalSession::check_target_support`](crate::GlobalSession::check_target_support).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum TargetFormat {
    /// SPIR-V, the binary shader format of Vulkan, as Slang's
    /// `SLANG_SPIRV`.
    Spirv,
    /// SPIR-V as readable assembly text, as Slang's `SLANG_SPIRV_ASM`.
    SpirvAssembly,
    /// Source code in the OpenGL Shading Language (GLSL), the shader language
    /// of OpenGL, as Slang's `SLANG_GLSL`.
    Glsl,
    /// Source code in the High-Level Shading Language (HLSL), the shader
    /// language of Direct3D, as Slang's `SLANG_HLSL`.
    Hlsl,
    /// Source code in the WebGPU Shading Language (WGSL), as Slang's
    /// `SLANG_WGSL`.
    Wgsl,
    /// Source code in the Metal Shading Language, the shader language of
    /// Apple's Metal API, as Slang's `SLANG_METAL`.
    Metal,
    /// C++ source code that runs the shader on the central processing unit
    /// (CPU), as Slang's `SLANG_CPP_SOURCE`.
    CppSource,
    /// Source code for CUDA, NVIDIA's API for computing on its GPUs, as
    /// Slang's `SLANG_CUDA_SOURCE`.
    CudaSource,
    /// DirectX Intermediate Language (DXIL), the binary shader format of
    /// Direct3D 12, as Slang's `SLANG_DXIL`.
    Dxil,
}

impl TargetFormat {
    pub(crate) fn to_raw(self) -> SlangCompileTarget {
        match self {
            Self::Spirv => SlangCompileTarget::SLANG_SPIRV,
            Self::SpirvAssembly => SlangCompileTarget::SLANG_SPIRV_ASM,
            Self::Glsl => SlangCompileTarget::SLANG_GLSL,
            Self::Hlsl => SlangCompileTarget::SLANG_HLSL,
            Self::Wgsl => SlangCompileTarget::SLANG_WGSL,
            Self::Metal => SlangCompileTarget::SLANG_METAL,
            Self::CppSource => SlangCompileTarget::SLANG_CPP_SOURCE,
            Self::CudaSource => SlangCompileTarget::SLANG_CUDA_SOURCE,
            Self::Dxil => SlangCompileTarget::SLANG_DXIL,
        }
    }
}

/// A setting that is either on or off.
///
/// The crate takes this type wherever Slang takes a yes-or-no setting, so
/// that a call names the state it sets instead of passing a bare `true` or
/// `false`. It is non-exhaustive, as every enumeration that the crate passes
/// to Slang is, so a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`GlobalSessionBuilder`](crate::GlobalSessionBuilder).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Toggle {
    /// The setting is on.
    On,
    /// The setting is off.
    Off,
}

impl Toggle {
    pub(crate) fn is_on(self) -> bool {
        match self {
            Self::On => true,
            Self::Off => false,
        }
    }
}

/// A version of the Slang language, which decides which language rules apply
/// to the source.
///
/// Each variant names one fixed version. Slang's own names for the default,
/// latest, and next versions are left out, because they name another version
/// at each Slang release. The enumeration is non-exhaustive, so a later
/// version can add language versions, and a `match` on it needs a wildcard
/// arm.
///
/// # Examples
///
/// See the example of [`GlobalSessionBuilder`](crate::GlobalSessionBuilder).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LanguageVersion {
    /// The rules of Slang before the language had versions, which Slang calls
    /// `SLANG_LANGUAGE_VERSION_LEGACY`, the value 2018.
    Legacy,
    /// The rules of 2025, which Slang calls `SLANG_LANGUAGE_VERSION_2025` and
    /// also `SLANG_LANGUAGE_VERSION_202A`.
    V2025,
    /// The rules of 2026, which Slang calls `SLANG_LANGUAGE_VERSION_2026` and
    /// also `SLANG_LANGUAGE_VERSION_202B`.
    V2026,
    /// The rules of the version in development, which Slang calls
    /// `SLANG_LANGUAGE_VERSION_202C`, the value 2027. Slang can change these
    /// rules until the version gets its official name.
    V202C,
}

impl LanguageVersion {
    pub(crate) fn to_raw(self) -> SlangLanguageVersion {
        match self {
            Self::Legacy => SlangLanguageVersion::SLANG_LANGUAGE_VERSION_LEGACY,
            Self::V2025 => SlangLanguageVersion::SLANG_LANGUAGE_VERSION_2025,
            Self::V2026 => SlangLanguageVersion::SLANG_LANGUAGE_VERSION_2026,
            Self::V202C => SlangLanguageVersion::SLANG_LANGUAGE_VERSION_202C,
        }
    }
}

/// The order in which the elements of a matrix are stored in memory, which
/// this crate calls a matrix layout.
///
/// In a row-major layout, the elements of each row are adjacent in memory,
/// and in a column-major layout, the elements of each column are. A shader
/// and the program that fills its buffers have to agree on the layout, so set
/// it to the layout that the program writes. The enumeration is
/// non-exhaustive, so a later version can add layouts, and a `match` on it
/// needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`SessionBuilder::matrix_layout`](crate::SessionBuilder::matrix_layout).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MatrixLayout {
    /// The elements of each row are adjacent in memory, as Slang's
    /// `SLANG_MATRIX_LAYOUT_ROW_MAJOR`.
    RowMajor,
    /// The elements of each column are adjacent in memory, as Slang's
    /// `SLANG_MATRIX_LAYOUT_COLUMN_MAJOR`.
    ColumnMajor,
}

impl MatrixLayout {
    pub(crate) fn to_raw(self) -> SlangMatrixLayoutMode {
        match self {
            Self::RowMajor => SlangMatrixLayoutMode::SLANG_MATRIX_LAYOUT_ROW_MAJOR,
            Self::ColumnMajor => SlangMatrixLayoutMode::SLANG_MATRIX_LAYOUT_COLUMN_MAJOR,
        }
    }
}

/// The kind of work that an entry point of a shader does in a pipeline,
/// which Slang calls a stage.
///
/// An entry point is a function of a shader that a graphics API calls, like
/// `main` in a program, and its stage says where in the pipeline it runs,
/// such as [`Stage::Compute`] for a compute shader, a shader that runs as a
/// general computation outside the drawing pipeline. The enumeration is
/// non-exhaustive, so a later version can add stages, and a `match` on it
/// needs a wildcard arm.
///
/// # Examples
///
/// This example names the stage of a compute shader:
///
/// ```
/// use slang_compiler::Stage;
///
/// let stage = Stage::Compute;
/// assert_ne!(stage, Stage::Fragment);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Stage {
    /// The vertex stage, which transforms each vertex, as Slang's
    /// `SLANG_STAGE_VERTEX`.
    Vertex,
    /// The hull stage of tessellation, as Slang's `SLANG_STAGE_HULL`.
    Hull,
    /// The domain stage of tessellation, as Slang's `SLANG_STAGE_DOMAIN`.
    Domain,
    /// The geometry stage, as Slang's `SLANG_STAGE_GEOMETRY`.
    Geometry,
    /// The fragment stage, which computes the colour of each pixel, as
    /// Slang's `SLANG_STAGE_FRAGMENT`, which Slang also calls
    /// `SLANG_STAGE_PIXEL`.
    Fragment,
    /// The compute stage, as Slang's `SLANG_STAGE_COMPUTE`.
    Compute,
    /// The ray generation stage of ray tracing, as Slang's
    /// `SLANG_STAGE_RAY_GENERATION`.
    RayGeneration,
    /// The intersection stage of ray tracing, as Slang's
    /// `SLANG_STAGE_INTERSECTION`.
    Intersection,
    /// The any-hit stage of ray tracing, as Slang's `SLANG_STAGE_ANY_HIT`.
    AnyHit,
    /// The closest-hit stage of ray tracing, as Slang's
    /// `SLANG_STAGE_CLOSEST_HIT`.
    ClosestHit,
    /// The miss stage of ray tracing, as Slang's `SLANG_STAGE_MISS`.
    Miss,
    /// The callable stage of ray tracing, as Slang's `SLANG_STAGE_CALLABLE`.
    Callable,
    /// The mesh stage of mesh shading, as Slang's `SLANG_STAGE_MESH`.
    Mesh,
    /// The amplification stage of mesh shading, as Slang's
    /// `SLANG_STAGE_AMPLIFICATION`.
    Amplification,
    /// The dispatch stage, as Slang's `SLANG_STAGE_DISPATCH`.
    Dispatch,
    /// The node stage of work graphs, as Slang's `SLANG_STAGE_NODE`.
    Node,
}

impl Stage {
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the lookup of an entry point with its stage arrives in step 3 of Section 15 of docs/design/slang-compiler.md, and the unit tests use this conversion"
        )
    )]
    pub(crate) fn to_raw(self) -> SlangStage {
        match self {
            Self::Vertex => SlangStage::SLANG_STAGE_VERTEX,
            Self::Hull => SlangStage::SLANG_STAGE_HULL,
            Self::Domain => SlangStage::SLANG_STAGE_DOMAIN,
            Self::Geometry => SlangStage::SLANG_STAGE_GEOMETRY,
            Self::Fragment => SlangStage::SLANG_STAGE_FRAGMENT,
            Self::Compute => SlangStage::SLANG_STAGE_COMPUTE,
            Self::RayGeneration => SlangStage::SLANG_STAGE_RAY_GENERATION,
            Self::Intersection => SlangStage::SLANG_STAGE_INTERSECTION,
            Self::AnyHit => SlangStage::SLANG_STAGE_ANY_HIT,
            Self::ClosestHit => SlangStage::SLANG_STAGE_CLOSEST_HIT,
            Self::Miss => SlangStage::SLANG_STAGE_MISS,
            Self::Callable => SlangStage::SLANG_STAGE_CALLABLE,
            Self::Mesh => SlangStage::SLANG_STAGE_MESH,
            Self::Amplification => SlangStage::SLANG_STAGE_AMPLIFICATION,
            Self::Dispatch => SlangStage::SLANG_STAGE_DISPATCH,
            Self::Node => SlangStage::SLANG_STAGE_NODE,
        }
    }
}

/// How much Slang optimizes the code that it generates.
///
/// Higher levels make faster code and take longer to compile. The
/// enumeration is non-exhaustive, so a later version can add levels, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`CompilerOptions`](crate::CompilerOptions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum OptimizationLevel {
    /// No optimization, as Slang's `SLANG_OPTIMIZATION_LEVEL_NONE`.
    None,
    /// Slang's default balance of code quality and compile time, as Slang's
    /// `SLANG_OPTIMIZATION_LEVEL_DEFAULT`.
    Default,
    /// Aggressive optimization, as Slang's `SLANG_OPTIMIZATION_LEVEL_HIGH`.
    High,
    /// Every optimization, including those that can take a very long time,
    /// as Slang's `SLANG_OPTIMIZATION_LEVEL_MAXIMAL`.
    Maximal,
}

impl OptimizationLevel {
    pub(crate) fn to_raw(self) -> SlangOptimizationLevel {
        match self {
            Self::None => SlangOptimizationLevel::SLANG_OPTIMIZATION_LEVEL_NONE,
            Self::Default => SlangOptimizationLevel::SLANG_OPTIMIZATION_LEVEL_DEFAULT,
            Self::High => SlangOptimizationLevel::SLANG_OPTIMIZATION_LEVEL_HIGH,
            Self::Maximal => SlangOptimizationLevel::SLANG_OPTIMIZATION_LEVEL_MAXIMAL,
        }
    }
}

/// How much debug information Slang puts into the code that it generates.
///
/// Debug information lets a debugger or a profiler of the graphics API map
/// the generated code back to the Slang source. The enumeration is
/// non-exhaustive, so a later version can add levels, and a `match` on it
/// needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`CompilerOptions`](crate::CompilerOptions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DebugInfoLevel {
    /// No debug information, as Slang's `SLANG_DEBUG_INFO_LEVEL_NONE`.
    None,
    /// As little debug information as possible, as Slang's
    /// `SLANG_DEBUG_INFO_LEVEL_MINIMAL`.
    Minimal,
    /// The usual amount of debug information, as Slang's
    /// `SLANG_DEBUG_INFO_LEVEL_STANDARD`.
    Standard,
    /// As much debug information as possible, as Slang's
    /// `SLANG_DEBUG_INFO_LEVEL_MAXIMAL`.
    Maximal,
}

impl DebugInfoLevel {
    pub(crate) fn to_raw(self) -> SlangDebugInfoLevel {
        match self {
            Self::None => SlangDebugInfoLevel::SLANG_DEBUG_INFO_LEVEL_NONE,
            Self::Minimal => SlangDebugInfoLevel::SLANG_DEBUG_INFO_LEVEL_MINIMAL,
            Self::Standard => SlangDebugInfoLevel::SLANG_DEBUG_INFO_LEVEL_STANDARD,
            Self::Maximal => SlangDebugInfoLevel::SLANG_DEBUG_INFO_LEVEL_MAXIMAL,
        }
    }
}

/// How strictly the generated code follows the rules of floating-point
/// arithmetic.
///
/// A fast mode lets the code reorder and simplify arithmetic, which can
/// change results slightly, and a precise mode keeps the order that the
/// source gives. The enumeration is non-exhaustive, so a later version can
/// add modes, and a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`CompilerOptions`](crate::CompilerOptions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FloatingPointMode {
    /// Slang's default mode for the target, as Slang's
    /// `SLANG_FLOATING_POINT_MODE_DEFAULT`.
    Default,
    /// Arithmetic that may be reordered and simplified, as Slang's
    /// `SLANG_FLOATING_POINT_MODE_FAST`.
    Fast,
    /// Arithmetic in the order that the source gives, as Slang's
    /// `SLANG_FLOATING_POINT_MODE_PRECISE`.
    Precise,
}

impl FloatingPointMode {
    pub(crate) fn to_raw(self) -> SlangFloatingPointMode {
        match self {
            Self::Default => SlangFloatingPointMode::SLANG_FLOATING_POINT_MODE_DEFAULT,
            Self::Fast => SlangFloatingPointMode::SLANG_FLOATING_POINT_MODE_FAST,
            Self::Precise => SlangFloatingPointMode::SLANG_FLOATING_POINT_MODE_PRECISE,
        }
    }
}

/// Whether and how generated source code marks the line of the Slang source
/// that each part came from.
///
/// Such a mark is a line directive, such as `#line 12 "shader.slang"` in C,
/// which lets the compiler of the generated source report errors at the
/// Slang source. It applies to targets that produce source code, such as
/// HLSL, the High-Level Shading Language of Direct3D. The enumeration is
/// non-exhaustive, so a later version can add modes, and a `match` on it
/// needs a wildcard arm.
///
/// # Examples
///
/// See the example of [`CompilerOptions`](crate::CompilerOptions).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum LineDirectiveMode {
    /// Slang's default for the target, as Slang's
    /// `SLANG_LINE_DIRECTIVE_MODE_DEFAULT`.
    Default,
    /// No line directives, as Slang's `SLANG_LINE_DIRECTIVE_MODE_NONE`.
    None,
    /// Line directives in the form of C, as Slang's
    /// `SLANG_LINE_DIRECTIVE_MODE_STANDARD`.
    Standard,
    /// Line directives in the form of GLSL, the OpenGL Shading Language,
    /// which name each file by a number, as Slang's
    /// `SLANG_LINE_DIRECTIVE_MODE_GLSL`.
    Glsl,
    /// A source map, which records the origin of each line separately
    /// instead of in the generated source, as Slang's
    /// `SLANG_LINE_DIRECTIVE_MODE_SOURCE_MAP`.
    SourceMap,
}

impl LineDirectiveMode {
    pub(crate) fn to_raw(self) -> SlangLineDirectiveMode {
        match self {
            Self::Default => SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_DEFAULT,
            Self::None => SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_NONE,
            Self::Standard => SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_STANDARD,
            Self::Glsl => SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_GLSL,
            Self::SourceMap => SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_SOURCE_MAP,
        }
    }
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::{
        SlangCompileTarget, SlangDebugInfoLevel, SlangFloatingPointMode, SlangLanguageVersion,
        SlangLineDirectiveMode, SlangMatrixLayoutMode, SlangOptimizationLevel, SlangStage,
    };

    use super::{
        DebugInfoLevel, FloatingPointMode, LanguageVersion, LineDirectiveMode, MatrixLayout,
        OptimizationLevel, Stage, TargetFormat, Toggle,
    };

    #[test]
    fn every_target_format_converts_to_its_slang_target() {
        let pairs = [
            (TargetFormat::Spirv, SlangCompileTarget::SLANG_SPIRV),
            (
                TargetFormat::SpirvAssembly,
                SlangCompileTarget::SLANG_SPIRV_ASM,
            ),
            (TargetFormat::Glsl, SlangCompileTarget::SLANG_GLSL),
            (TargetFormat::Hlsl, SlangCompileTarget::SLANG_HLSL),
            (TargetFormat::Wgsl, SlangCompileTarget::SLANG_WGSL),
            (TargetFormat::Metal, SlangCompileTarget::SLANG_METAL),
            (
                TargetFormat::CppSource,
                SlangCompileTarget::SLANG_CPP_SOURCE,
            ),
            (
                TargetFormat::CudaSource,
                SlangCompileTarget::SLANG_CUDA_SOURCE,
            ),
            (TargetFormat::Dxil, SlangCompileTarget::SLANG_DXIL),
        ];
        for (format, raw) in pairs {
            assert_eq!(format.to_raw(), raw, "{format:?}");
        }
    }

    #[test]
    fn every_language_version_converts_to_its_slang_value() {
        let pairs = [
            (LanguageVersion::Legacy, 2018),
            (LanguageVersion::V2025, 2025),
            (LanguageVersion::V2026, 2026),
            (LanguageVersion::V202C, 2027),
        ];
        for (version, value) in pairs {
            assert_eq!(version.to_raw(), SlangLanguageVersion(value), "{version:?}");
        }
    }

    #[test]
    fn toggle_converts_to_the_matching_flag() {
        assert!(Toggle::On.is_on());
        assert!(!Toggle::Off.is_on());
    }

    #[test]
    fn every_matrix_layout_converts_to_its_slang_value() {
        assert_eq!(
            MatrixLayout::RowMajor.to_raw(),
            SlangMatrixLayoutMode::SLANG_MATRIX_LAYOUT_ROW_MAJOR
        );
        assert_eq!(
            MatrixLayout::ColumnMajor.to_raw(),
            SlangMatrixLayoutMode::SLANG_MATRIX_LAYOUT_COLUMN_MAJOR
        );
    }

    #[test]
    fn every_stage_converts_to_its_slang_value_and_fragment_is_pixel() {
        let stages = [
            Stage::Vertex,
            Stage::Hull,
            Stage::Domain,
            Stage::Geometry,
            Stage::Fragment,
            Stage::Compute,
            Stage::RayGeneration,
            Stage::Intersection,
            Stage::AnyHit,
            Stage::ClosestHit,
            Stage::Miss,
            Stage::Callable,
            Stage::Mesh,
            Stage::Amplification,
            Stage::Dispatch,
            Stage::Node,
        ];
        for (value, stage) in (1..).zip(stages) {
            assert_eq!(stage.to_raw(), SlangStage(value), "{stage:?}");
        }
        assert_eq!(Stage::Fragment.to_raw(), SlangStage::SLANG_STAGE_PIXEL);
        assert_eq!(
            SlangStage(u32::try_from(stages.len()).expect("the count fits") + 1),
            SlangStage::SLANG_STAGE_COUNT
        );
    }

    #[test]
    fn every_optimization_level_converts_to_its_slang_value() {
        let pairs = [
            (
                OptimizationLevel::None,
                SlangOptimizationLevel::SLANG_OPTIMIZATION_LEVEL_NONE,
            ),
            (
                OptimizationLevel::Default,
                SlangOptimizationLevel::SLANG_OPTIMIZATION_LEVEL_DEFAULT,
            ),
            (
                OptimizationLevel::High,
                SlangOptimizationLevel::SLANG_OPTIMIZATION_LEVEL_HIGH,
            ),
            (
                OptimizationLevel::Maximal,
                SlangOptimizationLevel::SLANG_OPTIMIZATION_LEVEL_MAXIMAL,
            ),
        ];
        for (level, raw) in pairs {
            assert_eq!(level.to_raw(), raw, "{level:?}");
        }
    }

    #[test]
    fn every_debug_info_level_converts_to_its_slang_value() {
        let pairs = [
            (
                DebugInfoLevel::None,
                SlangDebugInfoLevel::SLANG_DEBUG_INFO_LEVEL_NONE,
            ),
            (
                DebugInfoLevel::Minimal,
                SlangDebugInfoLevel::SLANG_DEBUG_INFO_LEVEL_MINIMAL,
            ),
            (
                DebugInfoLevel::Standard,
                SlangDebugInfoLevel::SLANG_DEBUG_INFO_LEVEL_STANDARD,
            ),
            (
                DebugInfoLevel::Maximal,
                SlangDebugInfoLevel::SLANG_DEBUG_INFO_LEVEL_MAXIMAL,
            ),
        ];
        for (level, raw) in pairs {
            assert_eq!(level.to_raw(), raw, "{level:?}");
        }
    }

    #[test]
    fn every_floating_point_mode_converts_to_its_slang_value() {
        let pairs = [
            (
                FloatingPointMode::Default,
                SlangFloatingPointMode::SLANG_FLOATING_POINT_MODE_DEFAULT,
            ),
            (
                FloatingPointMode::Fast,
                SlangFloatingPointMode::SLANG_FLOATING_POINT_MODE_FAST,
            ),
            (
                FloatingPointMode::Precise,
                SlangFloatingPointMode::SLANG_FLOATING_POINT_MODE_PRECISE,
            ),
        ];
        for (mode, raw) in pairs {
            assert_eq!(mode.to_raw(), raw, "{mode:?}");
        }
    }

    #[test]
    fn every_line_directive_mode_converts_to_its_slang_value() {
        let pairs = [
            (
                LineDirectiveMode::Default,
                SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_DEFAULT,
            ),
            (
                LineDirectiveMode::None,
                SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_NONE,
            ),
            (
                LineDirectiveMode::Standard,
                SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_STANDARD,
            ),
            (
                LineDirectiveMode::Glsl,
                SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_GLSL,
            ),
            (
                LineDirectiveMode::SourceMap,
                SlangLineDirectiveMode::SLANG_LINE_DIRECTIVE_MODE_SOURCE_MAP,
            ),
        ];
        for (mode, raw) in pairs {
            assert_eq!(mode.to_raw(), raw, "{mode:?}");
        }
    }
}
