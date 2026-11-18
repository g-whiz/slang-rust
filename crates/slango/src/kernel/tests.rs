use super::{Kernel, checked_immediate_size};
use crate::Module;
use crate::error::ErrorKind;

fn assert_clone<T: Clone + 'static>() {}

#[test]
fn modules_and_kernels_are_clone() {
    assert_clone::<Module>();
    assert_clone::<Kernel>();
}

const LIMIT: u32 = 256;

#[test]
fn immediates_up_to_the_limit_of_the_device_fit() {
    assert_eq!(
        checked_immediate_size("run", true, LIMIT, 4).expect("4 bytes fit"),
        4
    );
    assert_eq!(
        checked_immediate_size("run", true, LIMIT, u64::from(LIMIT)).expect("the limit fits"),
        LIMIT
    );
}

#[test]
fn immediates_over_the_limit_of_the_device_are_a_missing_feature() {
    for size in [u64::from(LIMIT) + 1, 4096] {
        let error = checked_immediate_size("run", true, LIMIT, size).expect_err("over the limit");
        assert_eq!(error.kind(), ErrorKind::MissingFeature);
        assert_eq!(
            error.to_string(),
            format!(
                "cannot load a Slang module: the `uniform` parameters of the kernel `run` take {size} bytes of immediates, and the device allows 256"
            )
        );
    }
}

#[test]
fn immediates_that_do_not_fit_u32_are_a_missing_feature() {
    let size = u64::from(u32::MAX) + 1;
    let error = checked_immediate_size("run", true, u32::MAX, size).expect_err("over u32");
    assert_eq!(error.kind(), ErrorKind::MissingFeature);
    assert!(error.to_string().contains(&size.to_string()), "{error}");
}

#[test]
fn immediates_on_a_device_without_the_feature_are_a_missing_feature() {
    let error = checked_immediate_size("run", false, LIMIT, 4).expect_err("no feature");
    assert_eq!(error.kind(), ErrorKind::MissingFeature);
    assert_eq!(
        error.to_string(),
        "cannot load a Slang module: the kernel `run` has `uniform` parameters, which need the feature IMMEDIATES that the device lacks"
    );
}
