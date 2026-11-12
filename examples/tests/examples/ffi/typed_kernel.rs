use crate::common::{assert_fails_with, assert_prints_usage};

const BIN: &str = env!("CARGO_BIN_EXE_typed-kernel");

#[test]
fn typed_kernel_prints_its_usage_for_help() {
    assert_prints_usage(BIN, "Usage: typed-kernel\n");
}

#[test]
fn typed_kernel_rejects_an_argument() {
    for args in [&["shaders/particles.slang"][..], &["--help", "--help"]] {
        assert_fails_with(BIN, args, "expected no arguments, see --help");
    }
}
