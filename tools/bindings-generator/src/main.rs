//! Writes the raw bindings of `slang-compiler-sys` and the list of their C
//! functions from the Slang headers of the Vulkan SDK that the environment
//! variable `VULKAN_SDK` names.

#![expect(
    clippy::print_stderr,
    reason = "the binary reports why it failed on standard error"
)]

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("bindings-generator: {error}");
            let mut source = error.source();
            while let Some(cause) = source {
                eprintln!("  caused by: {cause}");
                source = cause.source();
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let sdk = std::env::var_os("VULKAN_SDK")
        .filter(|sdk| !sdk.is_empty())
        .ok_or_else(|| {
            format!(
                "VULKAN_SDK is not set, set it to the directory of the installed Vulkan SDK {}",
                bindings_generator::pin::VULKAN_SDK_VERSION
            )
        })?;
    let headers = Path::new(&sdk).join("include").join("slang");
    let generated = bindings_generator::generate(&headers)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for (path, text) in generated.files() {
        let output = root.join(path);
        std::fs::write(&output, text)
            .map_err(|error| format!("cannot write {}: {error}", output.display()))?;
    }
    Ok(())
}
