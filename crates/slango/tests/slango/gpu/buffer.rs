use std::fmt::Debug;

use slango::{AdapterSelection, Buffer, Device, VertexElement};

use crate::common::device_with;

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

fn assert_round_trip<T: VertexElement + PartialEq + Debug>(
    device: &Device,
    elements: &[T],
) -> slango::Result<()> {
    let buffer = Buffer::from_slice(device, elements)?;
    assert_eq!(buffer.len(), elements.len());
    assert!(!buffer.is_empty());
    assert_eq!(buffer.read()?, elements);
    assert_eq!(buffer.clone().read()?, elements);
    Ok(())
}

#[test]
fn a_buffer_from_a_slice_reads_back_its_elements_for_every_element_type() {
    let device = device();
    assert_round_trip(&device, &[1.5_f32, -2.25, 0.0, f32::MAX]).expect("the buffer round-trips");
    assert_round_trip(&device, &[-1_i32, 0, i32::MIN, i32::MAX]).expect("the buffer round-trips");
    assert_round_trip(&device, &[0_u32, 1, u32::MAX]).expect("the buffer round-trips");
    assert_round_trip(
        &device,
        &[[1.0_f32, 2.0, 3.0, 4.0], [-1.0, -2.0, -3.0, -4.0]],
    )
    .expect("the buffer round-trips");
    assert_round_trip(&device, &[[1_i32, -2, 3, -4], [i32::MIN, 0, 0, i32::MAX]])
        .expect("the buffer round-trips");
    assert_round_trip(&device, &[[1_u32, 2, 3, 4], [u32::MAX, 0, 0, 7]])
        .expect("the buffer round-trips");
}

#[test]
fn a_buffer_of_packed_vectors_of_2_and_3_scalars_reads_back_its_elements() {
    let device = device();
    assert_round_trip(&device, &[[1.5_f32, -2.0], [0.25, f32::MAX], [0.0, -0.0]])
        .expect("the buffer round-trips");
    assert_round_trip(&device, &[[-1_i32, 2], [i32::MIN, i32::MAX]])
        .expect("the buffer round-trips");
    assert_round_trip(&device, &[[0_u32, 1], [u32::MAX, 7], [3, 4]])
        .expect("the buffer round-trips");
    assert_round_trip(&device, &[[1.0_f32, 2.0, 3.0], [-4.0, 5.5, -6.25]])
        .expect("the buffer round-trips");
    assert_round_trip(
        &device,
        &[[1_i32, -2, 3], [i32::MIN, 0, i32::MAX], [7, 8, 9]],
    )
    .expect("the buffer round-trips");
    assert_round_trip(&device, &[[1_u32, 2, 3], [u32::MAX, 0, 5]]).expect("the buffer round-trips");
    let zeroed = Buffer::<[f32; 3]>::zeroed(&device, 3).expect("the buffer is created");
    assert_eq!(zeroed.read().expect("the buffer reads back"), [[0.0; 3]; 3]);
}

#[test]
fn a_zeroed_buffer_reads_back_zeros() {
    let device = device();
    let buffer = Buffer::<[f32; 4]>::zeroed(&device, 5).expect("the buffer is created");
    assert_eq!(buffer.len(), 5);
    assert_eq!(buffer.read().expect("the buffer reads back"), [[0.0; 4]; 5]);
    let buffer = Buffer::<u32>::zeroed(&device, 3).expect("the buffer is created");
    assert_eq!(buffer.read().expect("the buffer reads back"), [0; 3]);
}

#[test]
fn a_buffer_of_several_megabytes_reads_back_its_elements() {
    let elements: Vec<u32> = (0..2 * 1024 * 1024).collect();
    let buffer = Buffer::from_slice(&device(), &elements).expect("the buffer is created");
    assert_eq!(buffer.read().expect("the buffer reads back"), elements);
}

#[test]
fn an_empty_buffer_is_created_and_reads_back_no_elements() {
    let device = device();
    let empty = Buffer::<f32>::from_slice(&device, &[]).expect("the buffer is created");
    assert!(empty.is_empty());
    assert_eq!(empty.len(), 0);
    assert_eq!(
        empty.read().expect("the buffer reads back"),
        Vec::<f32>::new()
    );
    let zeroed = Buffer::<[u32; 4]>::zeroed(&device, 0).expect("the buffer is created");
    assert!(zeroed.is_empty());
    assert_eq!(
        zeroed.read().expect("the buffer reads back"),
        Vec::<[u32; 4]>::new()
    );
}

#[test]
fn a_buffer_larger_than_the_device_allows_is_an_invalid_argument() {
    let device = device();
    let limit = slango::wgpu::device(&device).limits().max_buffer_size;
    let len = usize::try_from(limit / 16 + 1).expect("the length fits in usize");
    let error = Buffer::<[f32; 4]>::zeroed(&device, len).expect_err("the buffer is too large");
    assert_eq!(error.kind(), slango::ErrorKind::InvalidArgument);
}

#[test]
fn a_buffer_from_a_slice_on_a_destroyed_device_is_a_backend_error() {
    let device = device();
    slango::wgpu::device(&device).destroy();
    let error = Buffer::from_slice(&device, &[1_u32, 2, 3]).expect_err("the device is destroyed");
    assert_eq!(error.kind(), slango::ErrorKind::Backend);
    assert!(
        error.to_string().starts_with("cannot create a buffer"),
        "{error}"
    );
}
