use slang_slint::View;

fn assert_send_sync<T: Send + Sync + 'static>() {}

#[test]
fn a_view_is_send_and_sync() {
    assert_send_sync::<View>();
}
