#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` has a public field for each kernel and keeps its module private behind `Shader::module`"
)]
pub(crate) mod scale {
    slango_macros::shader!("../shaders/scale.slang");
}
