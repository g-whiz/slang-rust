use super::sealed::{SealedValue, SealedVertexElement};
use super::{
    Element, Marshalled, Value, ValueType, VertexElement, check, element_mismatch, marshal, write,
};
use crate::error::ErrorKind;
use crate::layout::{ElementType, PlainValue, Scalar, Shape};

mod ffi;

fn vector(length: u32) -> Shape {
    Shape::Vector { length }
}

fn matrix(rows: u32, columns: u32) -> Shape {
    Shape::Matrix { rows, columns }
}

fn array(length: u64, stride: u64, element: Shape) -> Shape {
    Shape::Array {
        length,
        stride,
        element: Box::new(element),
    }
}

fn plain(path: &str, offset: u64, size: u64, scalar: Scalar, shape: Shape) -> PlainValue {
    PlainValue {
        path: path.to_owned(),
        offset,
        size,
        scalar,
        shape,
    }
}

fn value_type(scalar: Scalar, dims: &[usize]) -> ValueType {
    ValueType::Plain {
        scalar,
        dims: dims.to_vec(),
    }
}

/// Returns a block of `len` zero bytes with each scalar of `scalars` at its
/// offset.
fn expected(len: usize, scalars: &[(usize, [u8; 4])]) -> Vec<u8> {
    let mut block = vec![0; len];
    for (offset, bytes) in scalars {
        block[*offset..offset + 4].copy_from_slice(bytes);
    }
    block
}

fn f(value: f32) -> [u8; 4] {
    value.to_le_bytes()
}

fn written<V: Value>(value: &V, plain: &PlainValue, block_len: usize) -> Vec<u8> {
    let mut block = vec![0; block_len];
    write(&plain.path, &marshal(value), plain, &mut block).expect("the value is written");
    block
}

#[test]
fn scalars_describe_their_slang_scalar_type() {
    assert_eq!(bool::value_type(), value_type(Scalar::Bool, &[]));
    assert_eq!(i32::value_type(), value_type(Scalar::Int32, &[]));
    assert_eq!(u32::value_type(), value_type(Scalar::Uint32, &[]));
    assert_eq!(f32::value_type(), value_type(Scalar::Float32, &[]));
}

#[test]
fn arrays_describe_their_lengths_from_the_outermost() {
    assert_eq!(<[f32; 3]>::value_type(), value_type(Scalar::Float32, &[3]));
    assert_eq!(
        <[[i32; 3]; 2]>::value_type(),
        value_type(Scalar::Int32, &[2, 3])
    );
    assert_eq!(
        <[[[bool; 4]; 3]; 2]>::value_type(),
        value_type(Scalar::Bool, &[2, 3, 4])
    );
}

#[test]
fn marshal_lists_the_scalars_with_the_last_index_fastest() {
    assert_eq!(
        marshal(&[[1_u32, 2, 3], [4, 5, 6]]),
        Marshalled {
            ty: value_type(Scalar::Uint32, &[2, 3]),
            scalars: (1_u32..=6).map(u32::to_le_bytes).collect(),
        }
    );
}

#[test]
fn bool_is_four_bytes_of_zero_or_one() {
    assert_eq!(marshal(&true).scalars, [[1, 0, 0, 0]]);
    assert_eq!(marshal(&false).scalars, [[0, 0, 0, 0]]);
}

#[test]
fn scalars_are_little_endian() {
    assert_eq!(marshal(&1.0_f32).scalars, [[0x00, 0x00, 0x80, 0x3f]]);
    assert_eq!(marshal(&-2_i32).scalars, [[0xfe, 0xff, 0xff, 0xff]]);
    assert_eq!(marshal(&0x0102_0304_u32).scalars, [[4, 3, 2, 1]]);
}

fn round_trip<T: VertexElement + PartialEq + std::fmt::Debug>(element: T, stride: usize) {
    assert_eq!(T::STRIDE, stride);
    let mut bytes = Vec::new();
    element.write_bytes(&mut bytes);
    assert_eq!(bytes.len(), stride);
    assert_eq!(T::read_bytes(&bytes), Some(element));
    assert_eq!(T::read_bytes(&bytes[1..]), None);
}

#[test]
fn elements_have_their_std430_stride_and_read_back_what_they_write() {
    round_trip(-7_i32, 4);
    round_trip(7_u32, 4);
    round_trip(1.5_f32, 4);
    round_trip([1_i32, -2, 3, -4], 16);
    round_trip([1_u32, 2, 3, 4], 16);
    round_trip([1.0_f32, -2.0, 0.5, 4.0], 16);
}

#[test]
fn vertex_elements_of_2_and_3_scalars_are_packed_and_read_back_what_they_write() {
    round_trip([1.5_f32, -2.0], 8);
    round_trip([-1_i32, 2], 8);
    round_trip([1_u32, u32::MAX], 8);
    round_trip([1.5_f32, -2.0, 0.25], 12);
    round_trip([i32::MIN, 0, i32::MAX], 12);
    round_trip([1_u32, 2, 3], 12);
    let mut bytes = Vec::new();
    [1_u32, 2, 3].write_bytes(&mut bytes);
    assert_eq!(bytes, [1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0]);
}

#[test]
fn element_bytes_are_little_endian_in_order() {
    let mut bytes = Vec::new();
    [1_u32, 2, 3, 4].write_bytes(&mut bytes);
    assert_eq!(bytes, [1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]);
}

#[test]
fn check_accepts_each_structure_that_an_array_stands_for() {
    let float = Scalar::Float32;
    let cases = [
        (f32::value_type(), Shape::Scalar),
        (<[f32; 3]>::value_type(), vector(3)),
        (<[f32; 3]>::value_type(), array(3, 16, Shape::Scalar)),
        (<[[f32; 3]; 2]>::value_type(), matrix(2, 3)),
        (<[[f32; 3]; 4]>::value_type(), array(4, 16, vector(3))),
        (
            <[[f32; 3]; 2]>::value_type(),
            array(2, 48, array(3, 16, Shape::Scalar)),
        ),
        (
            <[[[f32; 2]; 2]; 2]>::value_type(),
            array(2, 32, matrix(2, 2)),
        ),
    ];
    for (ty, shape) in cases {
        let plain = plain("v", 0, 256, float, shape);
        assert!(check("v", &ty, &plain).is_ok(), "{ty:?} against {plain:?}");
    }
}

fn mismatch(ty: &ValueType, scalar: Scalar, shape: Shape) -> String {
    let plain = plain("params.v", 0, 256, scalar, shape);
    let error = check("params.v", ty, &plain).expect_err("the value does not match");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    error.to_string()
}

fn mismatch_text(slang: &str, rust: &str) -> String {
    format!(
        "cannot set a plain value: the parameter `params.v` has the Slang type `{slang}`, and the value has the Rust type `{rust}`"
    )
}

#[test]
fn check_rejects_another_scalar_type() {
    assert_eq!(
        mismatch(&f32::value_type(), Scalar::Int32, Shape::Scalar),
        mismatch_text("int", "f32")
    );
    assert_eq!(
        mismatch(&u32::value_type(), Scalar::Bool, Shape::Scalar),
        mismatch_text("bool", "u32")
    );
    assert_eq!(
        mismatch(&<[i32; 2]>::value_type(), Scalar::Uint32, vector(2)),
        mismatch_text("uint2", "[i32; 2]")
    );
}

#[test]
fn check_rejects_another_vector_length() {
    assert_eq!(
        mismatch(&<[f32; 4]>::value_type(), Scalar::Float32, vector(3)),
        mismatch_text("float3", "[f32; 4]")
    );
    assert_eq!(
        mismatch(&f32::value_type(), Scalar::Float32, vector(3)),
        mismatch_text("float3", "f32")
    );
    assert_eq!(
        mismatch(&<[f32; 3]>::value_type(), Scalar::Float32, Shape::Scalar),
        mismatch_text("float", "[f32; 3]")
    );
}

#[test]
fn check_rejects_other_matrix_dimensions() {
    assert_eq!(
        mismatch(
            &<[[f32; 3]; 2]>::value_type(),
            Scalar::Float32,
            matrix(3, 2)
        ),
        mismatch_text("float3x2", "[[f32; 3]; 2]")
    );
    assert_eq!(
        mismatch(&<[f32; 4]>::value_type(), Scalar::Float32, matrix(2, 2)),
        mismatch_text("float2x2", "[f32; 4]")
    );
}

#[test]
fn check_rejects_another_array_length() {
    assert_eq!(
        mismatch(
            &<[[f32; 3]; 3]>::value_type(),
            Scalar::Float32,
            array(4, 16, vector(3))
        ),
        mismatch_text("float3[4]", "[[f32; 3]; 3]")
    );
    assert_eq!(
        mismatch(
            &<[[f32; 2]; 3]>::value_type(),
            Scalar::Float32,
            array(2, 48, array(3, 16, Shape::Scalar))
        ),
        mismatch_text("float[2][3]", "[[f32; 2]; 3]")
    );
}

#[test]
fn check_rejects_an_array_of_vectors_of_another_length() {
    assert_eq!(
        mismatch(
            &<[[f32; 4]; 4]>::value_type(),
            Scalar::Float32,
            array(4, 16, vector(3))
        ),
        mismatch_text("float3[4]", "[[f32; 4]; 4]")
    );
    assert_eq!(
        mismatch(
            &<[[[f32; 2]; 2]; 2]>::value_type(),
            Scalar::Float32,
            array(2, 32, matrix(2, 3))
        ),
        mismatch_text("float2x3[2]", "[[[f32; 2]; 2]; 2]")
    );
}

#[test]
fn write_puts_an_array_of_vectors_at_its_std140_stride() {
    let points = plain(
        "Values.points",
        0,
        64,
        Scalar::Float32,
        array(4, 16, vector(3)),
    );
    let value = [
        [1.0, 2.0, 3.0],
        [4.0, 5.0, 6.0],
        [7.0, 8.0, 9.0],
        [10.0, 11.0, 12.0],
    ];
    assert_eq!(
        written(&value, &points, 240),
        expected(
            240,
            &[
                (0, f(1.0)),
                (4, f(2.0)),
                (8, f(3.0)),
                (16, f(4.0)),
                (20, f(5.0)),
                (24, f(6.0)),
                (32, f(7.0)),
                (36, f(8.0)),
                (40, f(9.0)),
                (48, f(10.0)),
                (52, f(11.0)),
                (56, f(12.0)),
            ]
        )
    );
}

const MATS: [[[f32; 2]; 2]; 2] = [[[1.0, 2.0], [3.0, 4.0]], [[5.0, 6.0], [7.0, 8.0]]];

#[test]
fn write_puts_an_array_of_matrices_row_by_row_at_its_std140_strides() {
    let mats = plain(
        "Values.mats",
        64,
        64,
        Scalar::Float32,
        array(2, 32, matrix(2, 2)),
    );
    assert_eq!(
        written(&MATS, &mats, 240),
        expected(
            240,
            &[
                (64, f(1.0)),
                (68, f(2.0)),
                (80, f(3.0)),
                (84, f(4.0)),
                (96, f(5.0)),
                (100, f(6.0)),
                (112, f(7.0)),
                (116, f(8.0)),
            ]
        )
    );
}

#[test]
fn write_puts_an_array_of_matrices_row_by_row_at_its_std430_strides() {
    let rotations = plain(
        "rotations",
        64,
        32,
        Scalar::Float32,
        array(2, 16, matrix(2, 2)),
    );
    assert_eq!(
        written(&MATS, &rotations, 144),
        expected(
            144,
            &[
                (64, f(1.0)),
                (68, f(2.0)),
                (72, f(3.0)),
                (76, f(4.0)),
                (80, f(5.0)),
                (84, f(6.0)),
                (88, f(7.0)),
                (92, f(8.0)),
            ]
        )
    );
}

const GRID: [[f32; 3]; 2] = [[1.0, 2.0, 3.0], [4.0, 5.0, 6.0]];

#[test]
fn write_puts_an_array_of_arrays_at_its_std140_strides() {
    let grid = plain(
        "Values.grid",
        128,
        96,
        Scalar::Float32,
        array(2, 48, array(3, 16, Shape::Scalar)),
    );
    assert_eq!(
        written(&GRID, &grid, 240),
        expected(
            240,
            &[
                (128, f(1.0)),
                (144, f(2.0)),
                (160, f(3.0)),
                (176, f(4.0)),
                (192, f(5.0)),
                (208, f(6.0)),
            ]
        )
    );
}

#[test]
fn write_puts_an_array_of_arrays_at_its_std430_strides() {
    let cells = plain(
        "cells",
        96,
        24,
        Scalar::Float32,
        array(2, 12, array(3, 4, Shape::Scalar)),
    );
    assert_eq!(
        written(&GRID, &cells, 144),
        expected(
            144,
            &[
                (96, f(1.0)),
                (100, f(2.0)),
                (104, f(3.0)),
                (108, f(4.0)),
                (112, f(5.0)),
                (116, f(6.0)),
            ]
        )
    );
}

#[test]
fn write_puts_an_int2_and_an_int_at_their_offsets() {
    let pair = plain("Values.pair", 232, 8, Scalar::Int32, vector(2));
    let count = plain("Values.count", 224, 4, Scalar::Int32, Shape::Scalar);
    let mut block = vec![0; 240];
    write(&pair.path, &marshal(&[-1_i32, 7]), &pair, &mut block).expect("`pair` is written");
    write(&count.path, &marshal(&3_i32), &count, &mut block).expect("`count` is written");
    assert_eq!(
        block,
        expected(
            240,
            &[
                (224, 3_i32.to_le_bytes()),
                (232, (-1_i32).to_le_bytes()),
                (236, 7_i32.to_le_bytes()),
            ]
        )
    );
}

#[test]
fn write_puts_a_bool_as_a_four_byte_one() {
    let enabled = plain("enabled", 104, 4, Scalar::Bool, Shape::Scalar);
    assert_eq!(
        written(&true, &enabled, 112),
        expected(112, &[(104, [1, 0, 0, 0])])
    );
}

#[test]
fn write_puts_a_float4x4_row_by_row_in_the_immediates() {
    let transform = plain("transform", 32, 64, Scalar::Float32, matrix(4, 4));
    let mut value = [[0.0_f32; 4]; 4];
    let mut scalars = Vec::new();
    for (row, columns) in value.iter_mut().enumerate() {
        for (column, slot) in columns.iter_mut().enumerate() {
            let number = f32::from(u8::try_from(4 * row + column).expect("below 16"));
            *slot = number;
            scalars.push((32 + 16 * row + 4 * column, f(number)));
        }
    }
    assert_eq!(written(&value, &transform, 112), expected(112, &scalars));
}

#[test]
fn write_puts_each_row_of_a_float2x3_at_16_bytes_in_a_uniform_buffer() {
    let m23 = plain("Matrices.m23", 112, 32, Scalar::Float32, matrix(2, 3));
    assert_eq!(
        written(&[[1.0_f32, 2.0, 3.0], [4.0, 5.0, 6.0]], &m23, 160),
        expected(
            160,
            &[
                (112, f(1.0)),
                (116, f(2.0)),
                (120, f(3.0)),
                (128, f(4.0)),
                (132, f(5.0)),
                (136, f(6.0)),
            ]
        )
    );
}

#[test]
fn write_puts_each_row_of_a_float3x2_at_8_bytes_in_the_immediates() {
    let p32 = plain("p32", 80, 24, Scalar::Float32, matrix(3, 2));
    assert_eq!(
        written(&[[1.0_f32, 2.0], [3.0, 4.0], [5.0, 6.0]], &p32, 112),
        expected(
            112,
            &[
                (80, f(1.0)),
                (84, f(2.0)),
                (88, f(3.0)),
                (92, f(4.0)),
                (96, f(5.0)),
                (100, f(6.0)),
            ]
        )
    );
}

#[test]
fn write_of_a_mismatched_value_writes_no_byte() {
    let offset = plain("offset", 16, 12, Scalar::Float32, vector(3));
    let mut block = vec![0xaa; 112];
    let error = write("offset", &marshal(&[1.0_f32; 4]), &offset, &mut block)
        .expect_err("a float4 value does not match a float3");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(block, vec![0xaa; 112]);
}

#[test]
fn write_rejects_a_layout_past_the_reflected_size_and_writes_no_byte() {
    let short = plain("short", 0, 8, Scalar::Float32, vector(3));
    let mut block = vec![0; 16];
    let error = write("short", &marshal(&[1.0_f32; 3]), &short, &mut block)
        .expect_err("the third scalar lies past the reflected size");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter);
    assert_eq!(
        error.to_string(),
        "cannot set a plain value: the reflected layout of the parameter `short` places a scalar outside its 8 bytes"
    );
    assert_eq!(block, vec![0; 16]);
}

#[test]
fn write_rejects_a_value_past_the_end_of_its_block_and_writes_no_byte() {
    let late = plain("late", 12, 8, Scalar::Float32, vector(2));
    let mut block = vec![0; 16];
    let error = write("late", &marshal(&[1.0_f32; 2]), &late, &mut block)
        .expect_err("the second scalar lies past the end of the block");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter);
    assert_eq!(
        error.to_string(),
        "cannot set a plain value: the reflected layout of the parameter `late` places a scalar outside its block of 16 bytes"
    );
    assert_eq!(block, vec![0; 16]);
}

#[test]
fn write_rejects_a_matrix_without_rows() {
    let empty = plain("empty", 0, 16, Scalar::Float32, matrix(0, 2));
    let mut block = vec![0; 16];
    let error = write("empty", &marshal(&[[0.0_f32; 2]; 0]), &empty, &mut block)
        .expect_err("a matrix of zero rows has no row stride");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter);
}

fn element(scalar: Scalar, components: u32) -> ElementType {
    ElementType::Plain { scalar, components }
}

fn bound_to<T: Element>(element: &ElementType) -> Option<String> {
    element_mismatch(&T::value_type(), element)
}

#[test]
fn each_element_type_matches_the_scalar_or_vector_of_its_scalar_type_and_count() {
    assert_eq!(bound_to::<f32>(&element(Scalar::Float32, 1)), None);
    assert_eq!(bound_to::<i32>(&element(Scalar::Int32, 1)), None);
    assert_eq!(bound_to::<u32>(&element(Scalar::Uint32, 1)), None);
    assert_eq!(bound_to::<[f32; 4]>(&element(Scalar::Float32, 4)), None);
    assert_eq!(bound_to::<[i32; 4]>(&element(Scalar::Int32, 4)), None);
    assert_eq!(bound_to::<[u32; 4]>(&element(Scalar::Uint32, 4)), None);
}

#[test]
fn an_element_type_of_another_scalar_type_or_count_is_a_mismatch_with_the_slang_name() {
    let cases = [
        (bound_to::<u32>(&element(Scalar::Float32, 1)), "float"),
        (bound_to::<f32>(&element(Scalar::Int32, 1)), "int"),
        (bound_to::<i32>(&element(Scalar::Uint32, 1)), "uint"),
        (bound_to::<u32>(&element(Scalar::Bool, 1)), "bool"),
        (bound_to::<[u32; 4]>(&element(Scalar::Float32, 4)), "float4"),
        (bound_to::<[f32; 4]>(&element(Scalar::Int32, 4)), "int4"),
        (bound_to::<[i32; 4]>(&element(Scalar::Uint32, 4)), "uint4"),
        (bound_to::<[u32; 4]>(&element(Scalar::Bool, 4)), "bool4"),
        (bound_to::<f32>(&element(Scalar::Float32, 4)), "float4"),
        (bound_to::<[f32; 4]>(&element(Scalar::Float32, 1)), "float"),
        (bound_to::<[f32; 4]>(&element(Scalar::Float32, 3)), "float3"),
    ];
    for (outcome, name) in cases {
        assert_eq!(outcome.as_deref(), Some(name));
    }
}

#[test]
fn a_scalar_or_vector_of_another_scalar_type_matches_no_element_type() {
    let half2 = ElementType::OtherPlain {
        name: "vector<half,2>".to_owned(),
    };
    for outcome in [
        bound_to::<f32>(&half2),
        bound_to::<i32>(&half2),
        bound_to::<u32>(&half2),
        bound_to::<[f32; 4]>(&half2),
        bound_to::<[i32; 4]>(&half2),
        bound_to::<[u32; 4]>(&half2),
    ] {
        assert_eq!(outcome.as_deref(), Some("vector<half,2>"));
    }
}

#[test]
fn a_composite_element_matches_every_element_type() {
    let composite = ElementType::Composite;
    assert_eq!(bound_to::<f32>(&composite), None);
    assert_eq!(bound_to::<i32>(&composite), None);
    assert_eq!(bound_to::<u32>(&composite), None);
    assert_eq!(bound_to::<[f32; 4]>(&composite), None);
    assert_eq!(bound_to::<[i32; 4]>(&composite), None);
    assert_eq!(bound_to::<[u32; 4]>(&composite), None);
}
