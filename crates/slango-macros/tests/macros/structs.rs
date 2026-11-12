use std::mem::offset_of;

use slango::__private::StructElement;

use layouts::{Gap, Grid, Inner, Nested, Tail, Wide};

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module` (SLM-15), and each structure keeps its padding private (SLM-18)"
)]
pub(crate) mod layouts {
    slango_macros::shader!("shaders/layouts.slang");
}

fn nested(seed: u16) -> Nested {
    let base = f32::from(seed);
    Nested::new(
        [base, base + 0.5, -base],
        Inner::new([base * 2.0, 1.25], u32::from(seed) * 7 + 1),
        Gap::new(base - 3.0, [0.5, base, 2.0, -1.0]),
        base * 0.25,
    )
}

fn grid(seed: u16) -> Grid {
    let base = i32::from(seed);
    Grid::new(
        [[base, -base, 3], [base * 2, 7, -1]],
        [u32::from(seed), 0xffff_ffff, 1, u32::from(seed) << 8],
    )
}

fn bytes_of<T: StructElement>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    value.write_bytes(&mut bytes);
    bytes
}

fn round_trip<T: StructElement + PartialEq + std::fmt::Debug>(value: T) {
    let bytes = bytes_of(&value);
    assert_eq!(bytes.len(), T::STRIDE);
    assert_eq!(T::read_bytes(&bytes), Some(value));
}

#[test]
fn write_bytes_and_read_bytes_round_trip_for_each_generated_struct() {
    for seed in 0..32 {
        let value = f32::from(seed);
        round_trip(nested(seed));
        round_trip(grid(seed));
        round_trip(Tail::new([value, -value, 0.25]));
        round_trip(Gap::new(value, [value, 1.0, -2.5, 0.0]));
        round_trip(Inner::new([value, value / 4.0], u32::from(seed)));
        let small = u32::from(seed);
        let signed = i32::from(seed);
        round_trip(Wide::new(
            value, -value, 0.5, 2.0, small, !small, signed, -signed,
        ));
    }
}

#[test]
fn read_bytes_rejects_bytes_of_another_length() {
    let bytes = bytes_of(&nested(3));
    for length in [0, 1, Nested::STRIDE - 1] {
        assert_eq!(Nested::read_bytes(bytes.get(..length).unwrap()), None);
    }
    let mut longer = bytes;
    longer.push(0);
    assert_eq!(Nested::read_bytes(&longer), None);
}

/// Returns the bytes of `scalars` placed at their offsets in a zeroed
/// element of `stride` bytes, and panics when one is outside it.
fn placed(stride: usize, scalars: &[(usize, [u8; 4])]) -> Vec<u8> {
    let mut bytes = vec![0; stride];
    for (offset, scalar) in scalars {
        bytes[*offset..offset + 4].copy_from_slice(scalar);
    }
    bytes
}

fn floats<const N: usize>(offset: usize, values: [f32; N]) -> Vec<(usize, [u8; 4])> {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| (offset + 4 * index, value.to_le_bytes()))
        .collect()
}

#[test]
fn write_bytes_puts_each_scalar_at_the_offset_of_its_field_and_zero_in_the_padding() {
    let value = nested(5);
    let mut scalars = floats(offset_of!(Nested, position), value.position);
    scalars.extend(floats(offset_of!(Nested, inner.offset), value.inner.offset));
    scalars.push((offset_of!(Nested, inner.id), value.inner.id.to_le_bytes()));
    scalars.extend(floats(offset_of!(Nested, gap.weight), [value.gap.weight]));
    scalars.extend(floats(offset_of!(Nested, gap.colour), value.gap.colour));
    scalars.extend(floats(offset_of!(Nested, weight), [value.weight]));
    assert_eq!(bytes_of(&value), placed(Nested::STRIDE, &scalars));

    let value = grid(9);
    let cells = value.cells.as_flattened();
    let mut scalars: Vec<_> = cells
        .iter()
        .enumerate()
        .map(|(index, cell)| (offset_of!(Grid, cells) + 4 * index, cell.to_le_bytes()))
        .collect();
    scalars.extend(
        value
            .mask
            .iter()
            .enumerate()
            .map(|(index, mask)| (offset_of!(Grid, mask) + 4 * index, mask.to_le_bytes())),
    );
    assert_eq!(bytes_of(&value), placed(Grid::STRIDE, &scalars));
}
