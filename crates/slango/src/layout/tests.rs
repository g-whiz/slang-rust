use super::{KernelLayout, Offsets, Position};

mod ffi;

fn offsets(set: u64, binding: u64, byte: u64, register_space: u64) -> Offsets {
    Offsets {
        set,
        binding,
        byte,
        register_space,
    }
}

#[test]
fn descend_adds_every_offset_of_the_variable_to_the_running_sums() {
    let parent = Position {
        set: 1,
        binding: 2,
        byte: 16,
        register_space: 1,
    };
    assert_eq!(
        parent.descend(offsets(0, 3, 8, 1)),
        Position {
            set: 1,
            binding: 5,
            byte: 24,
            register_space: 2,
        }
    );
}

#[test]
fn a_parameter_block_takes_the_register_space_sum_as_its_set_and_restarts_bindings() {
    let outside = Position::default().descend(offsets(0, 3, 0, 0));
    let block = outside.descend(offsets(0, 0, 0, 1)).parameter_block();
    assert_eq!(block.set, 1);
    assert_eq!(block.binding, 0);
    let resource = block.block_element().descend(offsets(0, 1, 0, 0));
    assert_eq!((resource.set, resource.binding), (1, 1));
}

#[test]
fn a_nested_parameter_block_takes_the_set_after_its_outer_block() {
    let outer = Position::default()
        .descend(offsets(0, 0, 0, 1))
        .parameter_block();
    let element = outer.block_element().descend(offsets(0, 0, 0, 1));
    let inner = element.descend(offsets(0, 0, 0, 0)).parameter_block();
    assert_eq!(outer.set, 1);
    assert_eq!(inner.set, outer.set + 1);
}

#[test]
fn the_element_of_a_block_restarts_byte_offsets_and_keeps_its_bindings() {
    let block = Position {
        set: 2,
        binding: 1,
        byte: 48,
        register_space: 2,
    };
    assert_eq!(block.block_element(), Position { byte: 0, ..block });
}

fn assert_send_sync<T: Send + Sync + 'static>() {}

#[test]
fn the_layout_snapshot_is_send_and_sync() {
    assert_send_sync::<KernelLayout>();
}
