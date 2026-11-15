use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const TARGET: &str = "x86_64-unknown-linux-gnu";

fn scratch(name: &str) -> std::io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("slang-bindgen-binary")
        .join(name);
    match std::fs::remove_dir_all(&dir) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error),
        _ => Ok(dir),
    }
}

#[test]
fn binary_writes_the_texts_of_the_library_into_a_new_directory() {
    let sdk = std::env::var_os("VULKAN_SDK")
        .filter(|sdk| !sdk.is_empty())
        .expect("VULKAN_SDK names the Vulkan SDK whose Slang headers the test generates from");
    let headers = Path::new(&sdk).join("include").join("slang");
    let output = scratch("written")
        .expect("remove the output of an earlier run")
        .join("nested");
    let status = Command::new(env!("CARGO_BIN_EXE_slang-bindgen"))
        .arg(&headers)
        .arg(TARGET)
        .arg(&output)
        .status()
        .expect("start the binary");
    assert!(status.success(), "{status}");
    let generated = slang_bindgen::generate(&headers, TARGET)
        .unwrap_or_else(|error| panic!("generation failed: {error}"));
    let read = |name: &str| {
        let path = output.join(name);
        std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
    };
    assert_eq!(read("bindings.rs"), generated.bindings);
    assert_eq!(read("symbols.rs"), generated.symbols);
}

fn sdk_headers() -> PathBuf {
    let sdk = std::env::var_os("VULKAN_SDK")
        .filter(|sdk| !sdk.is_empty())
        .unwrap_or_default();
    Path::new(&sdk).join("include").join("slang")
}

fn run_binary(headers: &Path, target: &OsStr, output: &Path) -> std::io::Result<Output> {
    Command::new(env!("CARGO_BIN_EXE_slang-bindgen"))
        .arg(headers)
        .arg(target)
        .arg(output)
        .output()
}

#[test]
fn binary_reports_an_unreadable_header_directory_with_its_cause_and_writes_nothing() {
    let root = scratch("refused").expect("remove the output of an earlier run");
    let headers = root.join("missing-headers");
    let output = root.join("output");
    let result = run_binary(&headers, OsStr::new(TARGET), &output).expect("start the binary");
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let errors = String::from_utf8_lossy(&result.stderr);
    let mut lines = errors.lines();
    assert_eq!(
        lines.next(),
        Some(
            format!(
                "slang-bindgen: cannot read the header directory {}",
                headers.display()
            )
            .as_str()
        ),
        "{errors}"
    );
    assert!(
        lines
            .next()
            .is_some_and(|line| line.starts_with("  caused by: ")),
        "{errors}"
    );
    assert!(!output.exists(), "{}", output.display());
}

#[test]
fn binary_without_libclang_advises_to_install_it_or_set_libclang_path() {
    let root = scratch("no-libclang").expect("remove the output of an earlier run");
    let output = root.join("output");
    let result = Command::new(env!("CARGO_BIN_EXE_slang-bindgen"))
        .arg(sdk_headers())
        .arg(TARGET)
        .arg(&output)
        .env("LIBCLANG_PATH", root.join("no-such-directory"))
        .output()
        .expect("start the binary");
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let errors = String::from_utf8_lossy(&result.stderr);
    assert!(
        errors.starts_with("slang-bindgen: cannot load libclang: "),
        "{errors}"
    );
    assert!(
        errors.contains("Install libclang or set LIBCLANG_PATH to the directory that holds it"),
        "{errors}"
    );
    assert!(!output.exists(), "{}", output.display());
}

#[test]
fn binary_refuses_a_target_triple_that_is_not_utf8() {
    let root = scratch("not-utf8").expect("remove the output of an earlier run");
    let output = root.join("output");
    let target = OsStr::from_bytes(b"x86_64-\xff");
    let result = run_binary(&sdk_headers(), target, &output).expect("start the binary");
    assert_eq!(result.status.code(), Some(2), "{result:?}");
    let errors = String::from_utf8_lossy(&result.stderr);
    assert!(
        errors.starts_with("slang-bindgen: the target triple x86_64-\u{FFFD} is not valid UTF-8\nusage: slang-bindgen "),
        "{errors}"
    );
    assert!(!output.exists(), "{}", output.display());
}

#[test]
fn binary_reports_an_output_directory_that_it_cannot_create() {
    let root = scratch("not-a-directory").expect("remove the output of an earlier run");
    std::fs::create_dir_all(&root).expect("create the scratch directory");
    let output = root.join("file");
    std::fs::write(&output, "").expect("write a file where the directory would go");
    let result = run_binary(&sdk_headers(), OsStr::new(TARGET), &output).expect("start the binary");
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let errors = String::from_utf8_lossy(&result.stderr);
    assert!(
        errors.starts_with(&format!(
            "slang-bindgen: cannot create {}: ",
            output.display()
        )),
        "{errors}"
    );
}

#[test]
fn binary_reports_a_file_that_it_cannot_write() {
    let root = scratch("unwritable").expect("remove the output of an earlier run");
    let output = root.join("output");
    std::fs::create_dir_all(output.join("bindings.rs"))
        .expect("create a directory in place of a file");
    let result = run_binary(&sdk_headers(), OsStr::new(TARGET), &output).expect("start the binary");
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    let errors = String::from_utf8_lossy(&result.stderr);
    assert!(
        errors.starts_with(&format!(
            "slang-bindgen: cannot write {}: ",
            output.join("bindings.rs").display()
        )),
        "{errors}"
    );
    assert!(!output.join("symbols.rs").exists());
}

#[test]
fn binary_without_three_arguments_prints_its_usage() {
    let result = Command::new(env!("CARGO_BIN_EXE_slang-bindgen"))
        .arg(TARGET)
        .output()
        .expect("start the binary");
    assert_eq!(result.status.code(), Some(2), "{result:?}");
    assert!(
        String::from_utf8_lossy(&result.stderr).starts_with("usage: slang-bindgen "),
        "{result:?}"
    );
}
