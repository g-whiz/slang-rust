use std::ffi::OsString;
use std::fmt;

const VULKAN_SDK: &str = "VULKAN_SDK";
const DOCS_RS: &str = "DOCS_RS";
const TARGET_OS: &str = "CARGO_CFG_TARGET_OS";

/// What the logic of the build script reads from its environment.
trait Environment {
    /// Returns the value of an environment variable, or `None` when it is
    /// not set.
    fn var(&self, name: &str) -> Option<OsString>;
}

/// The failure of the build script.
#[derive(PartialEq, Eq)]
struct BuildError(String);

impl fmt::Debug for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Returns the instructions for Cargo: the rerun instructions, the `cfg` of
/// the stub for docs.rs, and, on Linux and macOS, the rpath of the library
/// directory of the Vulkan SDK, through which the macro library finds the
/// Slang library when rustc loads it (SLM-12 and SLM-13 of
/// `docs/design/slango-macros.md`).
fn instructions(environment: &impl Environment) -> Result<Vec<String>, BuildError> {
    let mut out = vec![
        format!("cargo::rerun-if-env-changed={VULKAN_SDK}"),
        format!("cargo::rerun-if-env-changed={DOCS_RS}"),
        "cargo::rustc-check-cfg=cfg(slango_docs_rs)".to_owned(),
    ];
    if environment.var(DOCS_RS).is_some() {
        out.push("cargo::rustc-cfg=slango_docs_rs".to_owned());
    }
    let target_os = environment.var(TARGET_OS);
    let rpath = matches!(
        target_os.as_ref().and_then(|os| os.to_str()),
        Some("linux" | "macos")
    );
    let sdk = environment.var(VULKAN_SDK).filter(|sdk| !sdk.is_empty());
    if let (true, Some(sdk)) = (rpath, sdk) {
        let Some(sdk) = sdk.to_str() else {
            return Err(BuildError(format!(
                "{VULKAN_SDK} is not valid UTF-8, so the build script of slango-macros cannot give the macro library an rpath to the Slang library of the Vulkan SDK. Install the SDK in a directory whose path is valid UTF-8."
            )));
        };
        if sdk.contains(',') {
            return Err(BuildError(format!(
                "{VULKAN_SDK} contains a comma, which the linker would read as the end of the rpath of the macro library to the Slang library of the Vulkan SDK. Install the SDK in a directory whose path has no comma."
            )));
        }
        out.push(format!("cargo::rustc-link-arg=-Wl,-rpath,{sdk}/lib"));
    }
    Ok(out)
}
