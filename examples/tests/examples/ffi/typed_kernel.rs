use std::process::{Command, Output};

fn typed_kernel(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_typed-kernel"))
        .args(args)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("the example starts")
}

#[test]
fn typed_kernel_prints_its_usage_for_help() {
    for flag in ["--help", "-h"] {
        let output = typed_kernel(&[flag]);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{}: {stderr}", output.status);
        assert_eq!(stderr, "");
        let stdout = String::from_utf8(output.stdout).expect("the usage is UTF-8");
        assert!(stdout.starts_with("Usage: typed-kernel\n"), "{stdout}");
    }
}

#[test]
fn typed_kernel_rejects_an_argument() {
    for args in [&["shaders/particles.slang"][..], &["--help", "--help"]] {
        let output = typed_kernel(args);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert_eq!(String::from_utf8_lossy(&output.stdout), "");
        assert_eq!(stderr, "typed-kernel: expected no arguments, see --help\n");
    }
}
