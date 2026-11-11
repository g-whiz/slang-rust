mod windowed_renderer;

use std::process::{Command, Output};

fn compute_kernel(file: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_compute-kernel"))
        .arg(file)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the example starts")
}

#[test]
fn compute_kernel_runs_saxpy_and_prints_the_checked_results() {
    let output = compute_kernel("shaders/saxpy.slang");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{}: {stderr}", output.status);
    assert!(stderr.starts_with("running on "), "{stderr}");
    let stdout = String::from_utf8(output.stdout).expect("the report is UTF-8");
    insta::assert_snapshot!("saxpy", stdout);
}

#[test]
fn compute_kernel_reports_a_compile_error_on_standard_error() {
    let output = compute_kernel("shaders/broken.slang");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        stderr.contains("compute-kernel: cannot compile a Slang module"),
        "{stderr}"
    );
    assert!(
        stderr.contains("undefined identifier 'undefinedValue'"),
        "{stderr}"
    );
}

#[test]
fn compute_kernel_reports_a_missing_file_on_standard_error() {
    let output = compute_kernel("shaders/missing.slang");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        stderr.contains("compute-kernel: cannot read a Slang file: `shaders/missing.slang`"),
        "{stderr}"
    );
    assert!(stderr.contains("  caused by: "), "{stderr}");
}
