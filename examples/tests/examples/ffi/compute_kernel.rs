use crate::common::{assert_fails_with, assert_prints_usage};

const BIN: &str = env!("CARGO_BIN_EXE_compute-kernel");

#[test]
fn compute_kernel_prints_its_usage_for_help() {
    assert_prints_usage(BIN, "Usage: compute-kernel <FILE>\n");
}

#[test]
fn compute_kernel_rejects_a_wrong_number_of_files() {
    for args in [&[][..], &["shaders/saxpy.slang", "shaders/scale.slang"]] {
        assert_fails_with(BIN, args, "expected one Slang file, see --help");
    }
}
