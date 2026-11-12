mod nested;

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` has a public field for each kernel and keeps its module private behind `Shader::module` (SLM-15)"
)]
pub(crate) mod saxpy {
    slango_macros::shader!("shaders/saxpy.slang");
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn the_shader_its_kernels_and_their_arguments_are_send_and_sync() {
    assert_send_sync::<saxpy::Shader>();
    assert_send_sync::<saxpy::Saxpy>();
    assert_send_sync::<saxpy::SaxpyArgs<'static>>();
}

#[test]
fn a_path_resolves_against_the_directory_of_the_rust_file_that_calls_the_macro() {
    assert_eq!(saxpy::Saxpy::THREAD_GROUP_SIZE, [64, 1, 1]);
    assert_eq!(nested::scale::Scale::THREAD_GROUP_SIZE, [16, 2, 1]);
}
