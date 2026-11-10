/// The version of the Vulkan SDK whose Slang these bindings match.
///
/// The LunarG Vulkan SDK is the software development kit for the Vulkan
/// graphics API, and it ships a build of Slang. The headers of that build do
/// not record this version, so it identifies the SDK to install, while
/// [`SLANG_BUILD_TAG`] and [`SLANG_COMMIT`] identify the Slang build itself.
pub const VULKAN_SDK_VERSION: &str = "1.4.363.1";

/// The build tag of the Slang build that these bindings match.
///
/// A build tag is the output of `git describe` on the Slang commit: the
/// nearest release tag, the number of commits after it, and `g` followed by
/// the abbreviated commit hash. The Slang library returns the same string from
/// `spGetBuildTagString`, so a program can compare the two at run time.
pub const SLANG_BUILD_TAG: &str = "2026.17-25-gc7954ebd1";

/// The full hash of the Slang commit that these bindings match.
///
/// A build of Slang from source has to start from a source tree at this
/// commit, because a library built from another commit does not match these
/// bindings.
pub const SLANG_COMMIT: &str = "c7954ebd1f1984ad2bfde15f76248db41dc34aa3";
