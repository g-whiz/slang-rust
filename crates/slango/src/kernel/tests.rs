use super::Kernel;
use crate::Module;

fn assert_clone<T: Clone + 'static>() {}

#[test]
fn modules_and_kernels_are_clone() {
    assert_clone::<Module>();
    assert_clone::<Kernel>();
}
