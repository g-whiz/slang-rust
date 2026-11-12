mod shader_playground;
mod windowed_renderer;

use std::process::Output;

use crate::common::run;

fn compute_kernel(file: &str) -> Output {
    run(env!("CARGO_BIN_EXE_compute-kernel"), &[file])
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

#[test]
fn typed_kernel_advances_the_particles_and_prints_the_checked_results() {
    let output = run::<&str>(env!("CARGO_BIN_EXE_typed-kernel"), &[]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{}: {stderr}", output.status);
    assert!(stderr.starts_with("running on "), "{stderr}");
    let stdout = String::from_utf8(output.stdout).expect("the report is UTF-8");
    insta::assert_snapshot!("particles", stdout);
}
