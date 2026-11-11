use std::error::Error as _;

use super::{Argument, Bound, Call, Entry, Plan, Source, check_workgroups, plan, workgroup_counts};
use crate::error::{ErrorKind, Result};
use crate::layout::{
    Access, Binding, KernelLayout, PlainValue, Resource, Scalar, Shape, UniformBlock, Usage,
};
use crate::marshal::{Value, marshal};

fn plain(path: &str, offset: u64, size: u64, scalar: Scalar, shape: Shape) -> PlainValue {
    PlainValue {
        path: path.to_owned(),
        offset,
        size,
        scalar,
        shape,
    }
}

fn storage(path: &str, set: u32, index: u32, usage: Usage, stride: u64) -> Binding {
    Binding {
        path: path.to_owned(),
        set,
        index,
        usage,
        resource: Resource::StorageBuffer {
            access: Access::ReadWrite,
            stride,
        },
    }
}

fn uniform(path: &str, set: u32, index: u32, usage: Usage, block: UniformBlock) -> Binding {
    Binding {
        path: path.to_owned(),
        set,
        index,
        usage,
        resource: Resource::UniformBuffer(block),
    }
}

/// The layout of the kernel `scale` of Section 5.2 of
/// `docs/design/slango.md`, with an unused buffer at set 0, binding 2, a
/// float4 buffer in a parameter block at set 2, and an `int` immediate.
fn layout() -> KernelLayout {
    KernelLayout {
        name: "scale".to_owned(),
        thread_group_size: [64, 1, 1],
        bindings: vec![
            uniform(
                "",
                0,
                0,
                Usage::Used,
                UniformBlock {
                    size: 16,
                    values: vec![
                        plain("factor", 0, 4, Scalar::Float32, Shape::Scalar),
                        plain("count", 4, 4, Scalar::Uint32, Shape::Scalar),
                    ],
                },
            ),
            storage("data", 0, 1, Usage::Used, 4),
            storage("spare", 0, 2, Usage::Unused, 4),
            storage("extra.points", 2, 0, Usage::Used, 16),
        ],
        immediates: Some(UniformBlock {
            size: 16,
            values: vec![plain("offset", 0, 4, Scalar::Int32, Shape::Scalar)],
        }),
    }
}

fn value<V: Value>(value: V) -> Argument<()> {
    Argument::Value(marshal(&value))
}

fn buffer(stride: usize, len: usize) -> Argument<()> {
    Argument::Buffer(Bound {
        handle: (),
        stride,
        len,
        on_device: true,
    })
}

fn complete() -> Vec<(&'static str, Argument<()>)> {
    vec![
        ("factor", value(2.0_f32)),
        ("count", value(4_u32)),
        ("data", buffer(4, 4)),
        ("extra.points", buffer(16, 2)),
        ("offset", value(-3_i32)),
    ]
}

fn le(scalars: &[[u8; 4]]) -> Vec<u8> {
    let mut bytes: Vec<u8> = scalars.iter().flatten().copied().collect();
    bytes.resize(16, 0);
    bytes
}

#[test]
fn a_complete_call_binds_each_used_binding_and_fills_its_blocks() {
    let plan = plan(&layout(), &complete()).expect("the arguments match");
    assert_eq!(
        plan,
        Plan {
            groups: vec![
                vec![
                    Entry {
                        index: 0,
                        source: Source::Uniform(le(&[2.0_f32.to_le_bytes(), 4_u32.to_le_bytes()])),
                    },
                    Entry {
                        index: 1,
                        source: Source::Argument(2),
                    },
                ],
                vec![],
                vec![Entry {
                    index: 0,
                    source: Source::Argument(3),
                }],
            ],
            immediates: le(&[(-3_i32).to_le_bytes()]),
        }
    );
}

fn rejected(arguments: &[(&str, Argument<()>)]) -> (ErrorKind, String) {
    let error = plan(&layout(), arguments).expect_err("the call is rejected");
    (error.kind(), error.to_string())
}

fn replaced(path: &str, argument: &Argument<()>) -> Vec<(&'static str, Argument<()>)> {
    complete()
        .into_iter()
        .map(|(given, old)| {
            if given == path {
                (given, argument.clone())
            } else {
                (given, old)
            }
        })
        .collect()
}

fn without(path: &str) -> Vec<(&'static str, Argument<()>)> {
    complete()
        .into_iter()
        .filter(|(given, _)| *given != path)
        .collect()
}

#[test]
fn an_unknown_path_is_an_invalid_argument() {
    let mut arguments = complete();
    arguments.push(("fator", value(1.0_f32)));
    assert_eq!(
        rejected(&arguments),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the kernel `scale` has no buffer or plain value at the path `fator`"
                .to_owned()
        )
    );
}

#[test]
fn the_path_of_a_constant_buffer_itself_is_an_unknown_path() {
    let mut arguments = complete();
    arguments.push(("", value(1.0_f32)));
    assert_eq!(rejected(&arguments).0, ErrorKind::InvalidArgument);
}

#[test]
fn a_missing_plain_value_is_an_invalid_argument() {
    assert_eq!(
        rejected(&without("count")),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the parameter `count` of the kernel `scale` is not set"
                .to_owned()
        )
    );
}

#[test]
fn a_missing_immediate_is_an_invalid_argument() {
    assert_eq!(
        rejected(&without("offset")).1,
        "cannot call a kernel: the parameter `offset` of the kernel `scale` is not set"
    );
}

#[test]
fn a_missing_buffer_is_an_invalid_argument() {
    assert_eq!(
        rejected(&without("extra.points")),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: no buffer is bound to the parameter `extra.points` of the kernel `scale`"
                .to_owned()
        )
    );
}

#[test]
fn an_unused_buffer_may_be_omitted_and_a_given_one_is_checked_and_ignored() {
    let plan_without = plan(&layout(), &complete()).expect("the unused buffer may be omitted");
    let mut arguments = complete();
    arguments.push(("spare", buffer(4, 1)));
    let plan_with = plan(&layout(), &arguments).expect("the unused buffer is accepted");
    assert_eq!(plan_with, plan_without);
    let mut arguments = complete();
    arguments.push(("spare", buffer(16, 1)));
    assert_eq!(rejected(&arguments).0, ErrorKind::InvalidArgument);
}

#[test]
fn the_plain_values_of_an_unused_uniform_buffer_may_be_omitted() {
    let mut layout = layout();
    if let Some(binding) = layout.bindings.first_mut() {
        binding.usage = Usage::Unused;
    }
    let arguments: Vec<_> = complete()
        .into_iter()
        .filter(|(path, _)| *path != "factor" && *path != "count")
        .collect();
    let plan = plan(&layout, &arguments).expect("the unused values may be omitted");
    assert_eq!(
        plan.groups.first().map(Vec::len),
        Some(1),
        "set 0 binds only `data`"
    );
}

#[test]
fn a_value_of_another_type_is_an_invalid_argument_with_the_mismatch_as_its_source() {
    let error =
        plan(&layout(), &replaced("factor", &value(2_u32))).expect_err("a uint for a float");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot call a kernel: the kernel `scale` cannot take the value of the parameter `factor`"
    );
    assert_eq!(
        error.source().map(ToString::to_string).as_deref(),
        Some(
            "cannot set a plain value: the parameter `factor` has the Slang type `float`, and the value has the Rust type `u32`"
        )
    );
}

#[test]
fn a_value_for_a_buffer_is_an_invalid_argument() {
    assert_eq!(
        rejected(&replaced("data", &value(1.0_f32))),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the parameter `data` of the kernel `scale` is a buffer, and the call sets a value for it"
                .to_owned()
        )
    );
}

#[test]
fn a_buffer_for_a_value_is_an_invalid_argument() {
    assert_eq!(
        rejected(&replaced("offset", &buffer(4, 1))),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the parameter `offset` of the kernel `scale` is a plain value, and the call binds a buffer to it"
                .to_owned()
        )
    );
}

#[test]
fn a_buffer_of_another_stride_is_an_invalid_argument() {
    assert_eq!(
        rejected(&replaced("extra.points", &buffer(4, 8))),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the parameter `extra.points` of the kernel `scale` has elements of 16 bytes, and the bound buffer has elements of 4 bytes"
                .to_owned()
        )
    );
}

#[test]
fn an_empty_buffer_is_an_invalid_argument() {
    assert_eq!(
        rejected(&replaced("data", &buffer(4, 0))),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the buffer bound to the parameter `data` of the kernel `scale` is empty"
                .to_owned()
        )
    );
}

#[test]
fn a_buffer_of_another_device_is_an_invalid_argument() {
    let other = Argument::Buffer(Bound {
        handle: (),
        stride: 4,
        len: 4,
        on_device: false,
    });
    assert_eq!(
        rejected(&replaced("data", &other)),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the buffer bound to the parameter `data` of the kernel `scale` belongs to another device"
                .to_owned()
        )
    );
}

#[test]
fn the_first_argument_in_order_that_fails_is_reported() {
    let mut arguments = vec![("unknown", value(1.0_f32))];
    arguments.extend(replaced("factor", &value(1_u32)));
    assert!(rejected(&arguments).1.contains("`unknown`"));
}

#[test]
fn a_kernel_without_parameters_has_no_groups_and_no_immediates() {
    let layout = KernelLayout {
        name: "empty".to_owned(),
        thread_group_size: [1, 1, 1],
        bindings: Vec::new(),
        immediates: None,
    };
    assert_eq!(
        plan::<()>(&layout, &[]).expect("nothing to give"),
        Plan {
            groups: Vec::new(),
            immediates: Vec::new(),
        }
    );
}

#[test]
fn workgroup_counts_round_each_axis_up() {
    assert_eq!(workgroup_counts([4, 1, 1], [64, 1, 1]), Some([1, 1, 1]));
    assert_eq!(workgroup_counts([64, 1, 1], [64, 1, 1]), Some([1, 1, 1]));
    assert_eq!(workgroup_counts([65, 9, 3], [64, 4, 2]), Some([2, 3, 2]));
    assert_eq!(workgroup_counts([0, 5, 0], [8, 4, 2]), Some([0, 2, 0]));
}

#[test]
fn workgroup_counts_of_the_largest_element_counts_do_not_overflow() {
    assert_eq!(
        workgroup_counts([u32::MAX; 3], [64, 1, 2]),
        Some([u32::MAX / 64 + 1, u32::MAX, u32::MAX / 2 + 1])
    );
}

#[test]
fn workgroup_counts_of_a_group_size_of_zero_are_none() {
    assert_eq!(workgroup_counts([4, 1, 1], [64, 0, 1]), None);
}

#[test]
fn workgroups_up_to_the_limit_pass() -> Result<()> {
    check_workgroups("scale", [65_535, 1, 0], 65_535)
}

#[test]
fn workgroups_over_the_limit_are_an_invalid_argument() {
    let error = check_workgroups("scale", [1, 65_536, 1], 65_535).expect_err("over the limit");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot call a kernel: the dispatch of the kernel `scale` needs 65536 thread groups along the y axis, and the device allows 65535"
    );
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn a_call_is_send_and_sync() {
    assert_send_sync::<Call<'_>>();
}
