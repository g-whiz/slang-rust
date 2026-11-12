mod compute_kernel;
mod shader_playground;
mod typed_kernel;
mod watched;
mod windowed_renderer;

use std::fs;
use std::path::PathBuf;
use std::process::Output;

use crate::common::{assert_fails_with, assert_prints_usage, run};

const COMPILE_AND_REFLECT: &str = env!("CARGO_BIN_EXE_compile-and-reflect");

fn compile_and_reflect(file: &str) -> Output {
    run(COMPILE_AND_REFLECT, &[file])
}

fn report_of(output: Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{}: {stderr}", output.status);
    String::from_utf8(output.stdout).expect("the report is UTF-8")
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

#[test]
fn compile_and_reflect_prints_its_usage_for_help() {
    assert_prints_usage(
        COMPILE_AND_REFLECT,
        "Usage: compile-and-reflect [OPTIONS] <FILE>\n",
    );
}

#[test]
fn compile_and_reflect_compiles_only_the_entry_point_that_entry_names() {
    let report = report_of(run(
        COMPILE_AND_REFLECT,
        &["--entry", "clear", "shaders/scale.slang"],
    ));
    let entry_points: Vec<&str> = report
        .lines()
        .filter(|line| line.starts_with("entry point "))
        .collect();
    assert_eq!(entry_points, ["entry point clear (Compute)"], "{report}");
}

#[test]
fn compile_and_reflect_prints_the_code_size_in_bytes_for_the_target_that_target_names() {
    let report = report_of(run(
        COMPILE_AND_REFLECT,
        &["--target", "wgsl", "shaders/scale.slang"],
    ));
    assert!(report.starts_with("target: wgsl\n"), "{report}");
    let code: Vec<&str> = report
        .lines()
        .filter(|line| line.starts_with("  code: "))
        .collect();
    assert_eq!(code.len(), 2, "{report}");
    assert!(
        code.iter()
            .all(|line| line.ends_with(" bytes") && !line.contains("words")),
        "{report}"
    );
}

#[test]
fn compile_and_reflect_finds_an_imported_module_in_a_search_path() {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("compile-and-reflect")
        .join("search-path");
    let modules = dir.join("modules");
    fs::create_dir_all(&modules).expect("the scratch directory is created");
    fs::write(
        modules.join("helper.slang"),
        "public float twice(float x) { return 2.0 * x; }\n",
    )
    .expect("the module is written");
    let main = dir.join("main.slang");
    fs::write(
        &main,
        "import helper;\n\
         RWStructuredBuffer<float> data;\n\
         [shader(\"compute\")]\n\
         [numthreads(1, 1, 1)]\n\
         void main(uint3 id : SV_DispatchThreadID) { data[id.x] = twice(data[id.x]); }\n",
    )
    .expect("the shader is written");
    let main = main.to_str().expect("the scratch path is UTF-8");
    let modules = modules.to_str().expect("the scratch path is UTF-8");

    let without = run(COMPILE_AND_REFLECT, &[main]);
    let stderr = String::from_utf8_lossy(&without.stderr);
    assert!(!without.status.success(), "{stderr}");
    assert!(
        stderr.contains("cannot open file 'helper.slang'"),
        "{stderr}"
    );

    let report = report_of(run(COMPILE_AND_REFLECT, &["--search-path", modules, main]));
    assert!(report.contains("entry point main (Compute)\n"), "{report}");
}

#[test]
fn compile_and_reflect_rejects_each_bad_argument() {
    let file = "shaders/scale.slang";
    let cases: [(&[&str], &str); 7] = [
        (&["--bogus", file], "unknown option --bogus, see --help"),
        (
            &["--target", "nope", file],
            "unknown target \"nope\", see --help",
        ),
        (&[file, "--target"], "--target needs a value"),
        (&[file, "--entry"], "--entry needs a value"),
        (&[file, "--search-path"], "--search-path needs a directory"),
        (
            &[file, "shaders/broken.slang"],
            "expected one Slang file, see --help",
        ),
        (&[], "missing the Slang file to compile, see --help"),
    ];
    for (args, message) in cases {
        assert_fails_with(COMPILE_AND_REFLECT, args, message);
    }
}

#[test]
fn compile_and_reflect_reports_a_file_that_it_cannot_read() {
    let output = compile_and_reflect("shaders/missing.slang");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert!(
        stderr.starts_with("compile-and-reflect: cannot read shaders/missing.slang: "),
        "{stderr}"
    );
    assert_eq!(stderr.lines().count(), 1, "{stderr}");
}

#[cfg(unix)]
#[test]
fn compile_and_reflect_rejects_a_value_that_is_not_utf8() {
    use std::os::unix::ffi::OsStrExt;

    let value = std::ffi::OsStr::from_bytes(b"main\xff");
    let args = [
        std::ffi::OsStr::new("--entry"),
        value,
        std::ffi::OsStr::new("shaders/scale.slang"),
    ];
    let output = run(COMPILE_AND_REFLECT, &args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(
        stderr,
        "compile-and-reflect: the value main\u{fffd} of --entry is not valid UTF-8\n"
    );
}
