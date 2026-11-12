mod shader_playground;
mod typed_kernel;
mod windowed_renderer;

use std::process::Output;

use crate::common::run;

fn compile_and_reflect(file: &str) -> Output {
    run(env!("CARGO_BIN_EXE_compile-and-reflect"), &[file])
}

#[test]
fn compile_and_reflect_prints_the_entry_points_and_parameters_of_the_sample_shader() {
    let output = compile_and_reflect("shaders/scale.slang");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{}: {stderr}", output.status);
    let stdout = String::from_utf8(output.stdout).expect("the report is UTF-8");
    insta::assert_snapshot!("scale", stdout);
}

#[test]
fn compile_and_reflect_reports_a_compile_error_on_standard_error() {
    let output = compile_and_reflect("shaders/broken.slang");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!output.status.success(), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        stderr.contains("undefined identifier 'undefinedValue'"),
        "{stderr}"
    );
}
