use std::process::{Command, Output};

fn windowed_renderer(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_windowed-renderer"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the example starts")
}

fn assert_fails_with(args: &[&str], message: &str) {
    let output = windowed_renderer(args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(stderr, format!("windowed-renderer: {message}\n"));
}

#[test]
fn windowed_renderer_prints_its_usage_for_help() {
    for flag in ["--help", "-h"] {
        let output = windowed_renderer(&[flag]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{}: {stderr}", output.status);
        assert_eq!(stderr, "");
        let stdout = String::from_utf8(output.stdout).expect("the usage is UTF-8");
        assert!(
            stdout.starts_with("Usage: windowed-renderer [--frames N] <FILE>\n"),
            "{stdout}"
        );
    }
}

#[test]
fn windowed_renderer_rejects_frames_without_a_value() {
    assert_fails_with(
        &["shaders/textured-quad.slang", "--frames"],
        "--frames needs a number",
    );
}

#[test]
fn windowed_renderer_rejects_frames_that_are_not_a_whole_number() {
    for value in ["many", "-1", "2.5"] {
        assert_fails_with(
            &["--frames", value, "shaders/textured-quad.slang"],
            "--frames needs a whole number",
        );
    }
}

#[test]
fn windowed_renderer_rejects_two_files() {
    assert_fails_with(
        &["shaders/textured-quad.slang", "shaders/scale.slang"],
        "expected one Slang file, see --help",
    );
}

#[test]
fn windowed_renderer_rejects_a_missing_file_argument() {
    assert_fails_with(&[], "expected one Slang file, see --help");
    assert_fails_with(&["--frames", "3"], "expected one Slang file, see --help");
}
