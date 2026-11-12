use crate::common::{assert_prints_usage, assert_rejects_bad_arguments};

const BIN: &str = env!("CARGO_BIN_EXE_windowed-renderer");

#[test]
fn windowed_renderer_prints_its_usage_for_help() {
    assert_prints_usage(BIN, "Usage: windowed-renderer [--frames N] <FILE>\n");
}

#[test]
fn windowed_renderer_rejects_each_bad_argument() {
    assert_rejects_bad_arguments(BIN, "shaders/textured-quad.slang");
}
