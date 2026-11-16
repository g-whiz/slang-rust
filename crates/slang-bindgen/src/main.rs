//! Writes the raw Slang bindings and the list of their C functions, which
//! `slang_bindgen::generate` returns for a header directory and a target
//! triple, as `bindings.rs` and `symbols.rs` into an output directory.

#![expect(
    clippy::print_stderr,
    reason = "the binary reports its usage and why it failed on standard error"
)]

use std::error::Error;
use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage: slang-bindgen <header directory> <target triple> <output directory>";

fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let [headers, target, output] = args.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };
    let Some(target) = target.to_str() else {
        eprintln!(
            "slang-bindgen: the target triple {} is not valid UTF-8\n{USAGE}",
            target.to_string_lossy()
        );
        return ExitCode::from(2);
    };
    match run(Path::new(headers), target, Path::new(output)) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("slang-bindgen: {error}");
            let mut source = error.source();
            while let Some(cause) = source {
                eprintln!("  caused by: {cause}");
                source = cause.source();
            }
            if error
                .downcast_ref::<slang_bindgen::Error>()
                .is_some_and(|error| error.kind() == slang_bindgen::ErrorKind::Libclang)
            {
                eprintln!(
                    "slang-bindgen generates the bindings with libclang, so install libclang or set LIBCLANG_PATH to the directory that holds it"
                );
            }
            ExitCode::FAILURE
        }
    }
}

fn run(headers: &Path, target: &str, output: &Path) -> Result<(), Box<dyn Error>> {
    let generated = slang_bindgen::generate(headers, target)?;
    std::fs::create_dir_all(output)
        .map_err(|error| format!("cannot create {}: {error}", output.display()))?;
    for (name, text) in [
        ("bindings.rs", &generated.bindings),
        ("symbols.rs", &generated.symbols),
    ] {
        let path = output.join(name);
        std::fs::write(&path, text)
            .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
    }
    Ok(())
}
