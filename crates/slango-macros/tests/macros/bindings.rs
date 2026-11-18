mod nested;

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` has a public field for each kernel and keeps its module private behind `Shader::module`"
)]
pub(crate) mod saxpy {
    slango_macros::shader!("shaders/saxpy.slang");
}

macro_rules! assert_not_impl {
    ($type:ty: $trait:path) => {
        const _: () = {
            trait AmbiguousIfImpl<Marker> {
                fn some_item() {}
            }
            impl<T: ?Sized> AmbiguousIfImpl<()> for T {}
            impl<T: ?Sized + $trait> AmbiguousIfImpl<u8> for T {}
            let _ = <$type as AmbiguousIfImpl<_>>::some_item;
        };
    };
}

assert_not_impl!(saxpy::Saxpy: Send);
assert_not_impl!(saxpy::Saxpy: Sync);
assert_not_impl!(saxpy::Shader: Send);
assert_not_impl!(saxpy::Shader: Sync);
assert_not_impl!(saxpy::SaxpyArgs<'static>: Send);
assert_not_impl!(saxpy::SaxpyArgs<'static>: Sync);

#[test]
fn a_path_resolves_against_the_directory_of_the_rust_file_that_calls_the_macro() {
    assert_eq!(saxpy::Saxpy::THREAD_GROUP_SIZE, [64, 1, 1]);
    assert_eq!(nested::scale::Scale::THREAD_GROUP_SIZE, [16, 2, 1]);
}
