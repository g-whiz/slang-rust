use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Duration;

use slango::{Device, DeviceOptions};

/// Runs the example binary at `bin` with `args` in the directory of the
/// examples package, where the paths of the shaders start.
pub(crate) fn run<S: AsRef<OsStr>>(bin: &str, args: &[S]) -> Output {
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

/// Creates a device on the adapter that wgpu chooses.
pub(crate) fn device() -> Device {
    Device::new(&DeviceOptions::default()).expect("a device is created")
}

/// Returns the path of the shader `file` under `examples/shaders/`.
pub(crate) fn shader(file: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("shaders")
        .join(file)
}

/// Copies the shader `file` into the empty directory `example/name` under
/// the scratch directory of the tests, and returns the path of the copy.
pub(crate) fn scratch_copy(example: &str, file: &str, name: &str) -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join(example)
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir).expect("the old scratch directory is removed");
    }
    fs::create_dir_all(&dir).expect("the scratch directory is created");
    let path = dir.join(file);
    fs::copy(shader(file), &path).expect("the shader is copied");
    path
}

/// Writes `source` to the file at `path` and moves its modification time
/// 10 seconds past the old one, so that a reload sees the change on a file
/// system with coarse timestamps.
pub(crate) fn rewrite(path: &Path, source: &str) {
    let modified = fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .expect("the modification time is read");
    fs::write(path, source).expect("the shader is written");
    fs::File::options()
        .write(true)
        .open(path)
        .and_then(|file| file.set_modified(modified + Duration::from_secs(10)))
        .expect("the modification time is set");
}
