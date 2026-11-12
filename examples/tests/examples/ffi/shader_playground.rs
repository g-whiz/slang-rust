use crate::common::{
    assert_lists_the_windowed_options, assert_prints_usage, assert_rejects_bad_arguments,
};

const BIN: &str = env!("CARGO_BIN_EXE_shader-playground");

#[test]
fn shader_playground_prints_its_usage_for_help() {
    assert_prints_usage(BIN, "Usage: shader-playground [--frames N] <FILE>\n");
    assert_lists_the_windowed_options(BIN);
}

#[test]
fn shader_playground_rejects_each_bad_argument() {
    assert_rejects_bad_arguments(BIN, "shaders/playground.slang");
}
