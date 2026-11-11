use std::error::Error as _;

use super::{
    Argument, Bound, BoundSampler, BoundTexture, Call, Entry, Plan, Source, check_workgroups,
    insert, plan, workgroup_counts,
};
use crate::error::{ErrorKind, Result};
use crate::layout::{
    Access, Binding, KernelLayout, PlainValue, Resource, Scalar, Shape, UniformBlock, Usage,
};
use crate::marshal::{Value, marshal};
use crate::texture::TextureFormat;

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

type Arg = Argument<(), (), ()>;

fn value<V: Value>(value: V) -> Arg {
    Argument::Value(marshal(&value))
}

fn buffer(stride: usize, len: usize) -> Arg {
    Argument::Buffer(Bound {
        handle: (),
        stride,
        len,
        on_device: true,
    })
}

fn complete() -> Vec<(&'static str, Arg)> {
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

fn rejected(arguments: &[(&str, Arg)]) -> (ErrorKind, String) {
    let error = plan(&layout(), arguments).expect_err("the call is rejected");
    (error.kind(), error.to_string())
}

fn replaced(path: &str, argument: &Arg) -> Vec<(&'static str, Arg)> {
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

fn without(path: &str) -> Vec<(&'static str, Arg)> {
    complete()
        .into_iter()
        .filter(|(given, _)| *given != path)
        .collect()
}

#[test]
fn a_later_argument_for_a_path_replaces_the_earlier_one_whatever_its_kind() {
    let mut arguments = Vec::new();
    insert(&mut arguments, "factor", value(1.0_f32));
    insert(&mut arguments, "data", buffer(4, 4));
    insert(&mut arguments, "factor", value(2.0_f32));
    assert_eq!(
        arguments,
        vec![("factor", value(2.0_f32)), ("data", buffer(4, 4))]
    );
    insert(&mut arguments, "data", value(3_u32));
    insert(&mut arguments, "factor", buffer(16, 2));
    assert_eq!(
        arguments,
        vec![("factor", buffer(16, 2)), ("data", value(3_u32))]
    );
}

#[test]
fn an_unknown_path_is_an_invalid_argument() {
    let mut arguments = complete();
    arguments.push(("fator", value(1.0_f32)));
    assert_eq!(
        rejected(&arguments),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the kernel `scale` has no parameter at the path `fator`"
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
        plan::<(), (), ()>(&layout, &[]).expect("nothing to give"),
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

fn resource(path: &str, index: u32, usage: Usage, resource: Resource) -> Binding {
    Binding {
        path: path.to_owned(),
        set: 0,
        index,
        usage,
        resource,
    }
}

/// The layout of a kernel `sample` that reads the texture `image` through the
/// sampler `nearest` into the buffer `output`, with an unused texture and
/// sampler after them.
fn textured() -> KernelLayout {
    KernelLayout {
        name: "sample".to_owned(),
        thread_group_size: [1, 1, 1],
        bindings: vec![
            resource("image", 0, Usage::Used, Resource::SampledTexture),
            resource("nearest", 1, Usage::Used, Resource::Sampler),
            storage("output", 0, 2, Usage::Used, 16),
            resource("spare", 3, Usage::Unused, Resource::SampledTexture),
            resource("spareSampler", 4, Usage::Unused, Resource::Sampler),
        ],
        immediates: None,
    }
}

fn texture(format: TextureFormat) -> Arg {
    Argument::Texture(BoundTexture {
        handle: (),
        format,
        sampleable: true,
        on_device: true,
    })
}

fn sampler() -> Arg {
    Argument::Sampler(BoundSampler {
        handle: (),
        on_device: true,
    })
}

fn textured_call() -> Vec<(&'static str, Arg)> {
    vec![
        ("output", buffer(16, 4)),
        ("nearest", sampler()),
        ("image", texture(TextureFormat::Rgba8Unorm)),
    ]
}

fn textured_rejected(arguments: &[(&str, Arg)]) -> (ErrorKind, String) {
    let error = plan(&textured(), arguments).expect_err("the call is rejected");
    (error.kind(), error.to_string())
}

fn textured_replaced(path: &str, argument: &Arg) -> Vec<(&'static str, Arg)> {
    textured_call()
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

#[test]
fn a_texture_and_a_sampler_are_bound_from_their_arguments() {
    let plan = plan(&textured(), &textured_call()).expect("the arguments match");
    assert_eq!(
        plan,
        Plan {
            groups: vec![vec![
                Entry {
                    index: 0,
                    source: Source::Argument(2),
                },
                Entry {
                    index: 1,
                    source: Source::Argument(1),
                },
                Entry {
                    index: 2,
                    source: Source::Argument(0),
                },
            ]],
            immediates: Vec::new(),
        }
    );
}

#[test]
fn every_colour_format_can_be_bound_to_a_texture() {
    for format in [
        TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
    ] {
        plan(&textured(), &textured_replaced("image", &texture(format)))
            .unwrap_or_else(|error| panic!("{format:?} is rejected: {error}"));
    }
}

#[test]
fn an_unused_texture_and_sampler_may_be_omitted_and_given_ones_are_checked_and_ignored() {
    let plan_without = plan(&textured(), &textured_call()).expect("the unused ones may be omitted");
    let mut arguments = textured_call();
    arguments.push(("spare", texture(TextureFormat::Bgra8Unorm)));
    arguments.push(("spareSampler", sampler()));
    assert_eq!(
        plan(&textured(), &arguments).expect("the unused ones are accepted"),
        plan_without
    );
    let mut arguments = textured_call();
    arguments.push(("spare", texture(TextureFormat::Depth32Float)));
    assert_eq!(textured_rejected(&arguments).0, ErrorKind::InvalidArgument);
}

#[test]
fn a_missing_texture_is_an_invalid_argument() {
    let arguments: Vec<_> = textured_call()
        .into_iter()
        .filter(|(path, _)| *path != "image")
        .collect();
    assert_eq!(
        textured_rejected(&arguments),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: no texture is bound to the parameter `image` of the kernel `sample`"
                .to_owned()
        )
    );
}

#[test]
fn a_missing_sampler_is_an_invalid_argument() {
    let arguments: Vec<_> = textured_call()
        .into_iter()
        .filter(|(path, _)| *path != "nearest")
        .collect();
    assert_eq!(
        textured_rejected(&arguments),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: no sampler is bound to the parameter `nearest` of the kernel `sample`"
                .to_owned()
        )
    );
}

#[test]
fn an_argument_of_another_kind_than_its_parameter_is_an_invalid_argument() {
    let cases = [
        (
            "image",
            sampler(),
            "is a texture, and the call binds a sampler to it",
        ),
        (
            "image",
            buffer(16, 1),
            "is a texture, and the call binds a buffer to it",
        ),
        (
            "image",
            value(1.0_f32),
            "is a texture, and the call sets a value for it",
        ),
        (
            "nearest",
            texture(TextureFormat::Rgba8Unorm),
            "is a sampler, and the call binds a texture to it",
        ),
        (
            "nearest",
            buffer(16, 1),
            "is a sampler, and the call binds a buffer to it",
        ),
        (
            "nearest",
            value(1_u32),
            "is a sampler, and the call sets a value for it",
        ),
        (
            "output",
            texture(TextureFormat::Rgba8Unorm),
            "is a buffer, and the call binds a texture to it",
        ),
        (
            "output",
            sampler(),
            "is a buffer, and the call binds a sampler to it",
        ),
    ];
    for (path, argument, text) in cases {
        assert_eq!(
            textured_rejected(&textured_replaced(path, &argument)),
            (
                ErrorKind::InvalidArgument,
                format!(
                    "cannot call a kernel: the parameter `{path}` of the kernel `sample` {text}"
                )
            )
        );
    }
    for (argument, text) in [
        (texture(TextureFormat::Rgba8Unorm), "binds a texture to it"),
        (sampler(), "binds a sampler to it"),
    ] {
        assert_eq!(
            rejected(&replaced("count", &argument)).1,
            format!(
                "cannot call a kernel: the parameter `count` of the kernel `scale` is a plain value, and the call {text}"
            )
        );
    }
}

#[test]
fn a_texture_or_sampler_of_another_device_is_an_invalid_argument() {
    let other_texture = Argument::Texture(BoundTexture {
        handle: (),
        format: TextureFormat::Rgba8Unorm,
        sampleable: true,
        on_device: false,
    });
    assert_eq!(
        textured_rejected(&textured_replaced("image", &other_texture)),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the texture bound to the parameter `image` of the kernel `sample` belongs to another device"
                .to_owned()
        )
    );
    let other_sampler = Argument::Sampler(BoundSampler {
        handle: (),
        on_device: false,
    });
    assert_eq!(
        textured_rejected(&textured_replaced("nearest", &other_sampler)),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the sampler bound to the parameter `nearest` of the kernel `sample` belongs to another device"
                .to_owned()
        )
    );
}

#[test]
fn a_depth_texture_is_an_invalid_argument() {
    assert_eq!(
        textured_rejected(&textured_replaced(
            "image",
            &texture(TextureFormat::Depth32Float)
        )),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the texture bound to the parameter `image` of the kernel `sample` has the depth format Depth32Float, which a kernel cannot sample"
                .to_owned()
        )
    );
}

#[test]
fn a_texture_without_texture_binding_is_an_invalid_argument() {
    let unsampleable = Argument::Texture(BoundTexture {
        handle: (),
        format: TextureFormat::Bgra8Unorm,
        sampleable: false,
        on_device: true,
    });
    assert_eq!(
        textured_rejected(&textured_replaced("image", &unsampleable)),
        (
            ErrorKind::InvalidArgument,
            "cannot call a kernel: the texture bound to the parameter `image` of the kernel `sample` lacks the usage TEXTURE_BINDING, which a kernel needs to sample it"
                .to_owned()
        )
    );
}
