use std::path::Path;
use std::process::{Command, Output};

/// Runs the example binary at `bin` with `args` in the directory of the
/// examples package, where the paths of the shaders start.
pub(crate) fn run(bin: &str, args: &[&str]) -> Output {
    Command::new(bin)
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the example starts")
}

/// Asserts that the example prints its usage, which starts with
/// `first_line`, for `--help` and `-h`, and nothing on standard error.
pub(crate) fn assert_prints_usage(bin: &str, first_line: &str) {
    for flag in ["--help", "-h"] {
        let output = run(bin, &[flag]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{}: {stderr}", output.status);
        assert_eq!(stderr, "");
        let stdout = String::from_utf8(output.stdout).expect("the usage is UTF-8");
        assert!(stdout.starts_with(first_line), "{stdout}");
    }
}

/// Asserts that the example fails with exit status 1 for `args`, prints
/// nothing on standard output, and prints exactly `message` after its name
/// on standard error.
pub(crate) fn assert_fails_with(bin: &str, args: &[&str], message: &str) {
    let name = Path::new(bin)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .expect("the binary has a name");
    let output = run(bin, args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "");
    assert_eq!(stderr, format!("{name}: {message}\n"));
}

/// Asserts that a windowed example rejects each wrong use of `--frames`
/// and of its file argument, with `shader` as a valid file.
pub(crate) fn assert_rejects_bad_arguments(bin: &str, shader: &str) {
    assert_fails_with(bin, &[shader, "--frames"], "--frames needs a number");
    for value in ["many", "-1", "2.5"] {
        assert_fails_with(
            bin,
            &["--frames", value, shader],
            "--frames needs a whole number",
        );
    }
    assert_fails_with(
        bin,
        &["--frames", "0", shader],
        "--frames needs a number above zero",
    );
    let one_file = "expected one Slang file, see --help";
    assert_fails_with(bin, &[shader, "shaders/scale.slang"], one_file);
    assert_fails_with(bin, &[], one_file);
    assert_fails_with(bin, &["--frames", "3"], one_file);
}
