use slang_compiler_sys::{SlangCompileTarget, SlangLanguageVersion};

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

#[cfg(test)]
mod tests {
    use slang_compiler_sys::{SlangCompileTarget, SlangLanguageVersion};

    use super::{LanguageVersion, TargetFormat, Toggle};

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
}
