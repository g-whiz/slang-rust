use super::{Buffer, size_within};
use crate::error::ErrorKind;

mod gpu;

#[test]
fn size_within_multiplies_the_length_by_the_stride() {
    assert_eq!(size_within(3, 16, 48).expect("48 bytes fit"), 48);
    assert_eq!(size_within(0, 4, 0).expect("an empty buffer fits"), 0);
}

#[test]
fn size_within_rejects_a_size_above_the_limit() {
    let error = size_within(5, 4, 16).expect_err("20 bytes exceed 16");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot create a buffer: 5 elements of 4 bytes exceed the largest buffer that the device allows, 16 bytes"
    );
}

#[test]
fn size_within_rejects_a_size_that_overflows() {
    let error = size_within(usize::MAX, 16, u64::MAX).expect_err("the size overflows");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
}

fn assert_send_sync_clone<T: Send + Sync + Clone + 'static>() {}

#[test]
fn buffers_are_send_sync_and_clone() {
    assert_send_sync_clone::<Buffer<f32>>();
    assert_send_sync_clone::<Buffer<i32>>();
    assert_send_sync_clone::<Buffer<u32>>();
    assert_send_sync_clone::<Buffer<[f32; 4]>>();
    assert_send_sync_clone::<Buffer<[i32; 4]>>();
    assert_send_sync_clone::<Buffer<[u32; 4]>>();
}
