mod shader_playground;
mod windowed_renderer;

use std::process::{Command, Output};

fn compile_and_reflect(file: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_compile-and-reflect"))
        .arg(file)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the example starts")
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
