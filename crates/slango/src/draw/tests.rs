use wgpu::{ShaderStages, VertexFormat, VertexStepMode};

use super::{
    BoundIndex, BoundTarget, BoundVertex, Count, Draw, DrawPlan, Targets, element_type, plan_draw,
};
use crate::call::{Argument, Bound, BoundSampler, BoundTexture, Entry, Plan, Source};
use crate::error::ErrorKind;
use crate::layout::{
    Access, Attribute, Binding, ElementType, PlainValue, Resource, Scalar, Shape, UniformBlock,
    Usage,
};
use crate::marshal::{Value, VertexElement, marshal};
use crate::render::{
    PipelineBinding, PipelineSnapshot, RenderPipelineOptions, VertexInput, vertex_format,
};
use crate::texture::TextureFormat;

type Arg = Argument<(), (), ()>;

fn binding(
    path: &str,
    index: u32,
    visibility: ShaderStages,
    resource: Resource,
) -> PipelineBinding {
    PipelineBinding {
        binding: Binding {
            path: path.to_owned(),
            set: 0,
            index,
            usage: if visibility.is_empty() {
                Usage::Unused
            } else {
                Usage::Used
            },
            resource,
        },
        visibility,
    }
}

fn input(
    path: &str,
    location: u32,
    scalar: Scalar,
    components: u32,
    step_mode: VertexStepMode,
    slot: Option<u32>,
) -> VertexInput {
    VertexInput {
        attribute: Attribute {
            path: path.to_owned(),
            location,
            scalar,
            components,
            usage: if slot.is_some() {
                Usage::Used
            } else {
                Usage::Unused
            },
        },
        format: vertex_format(scalar, components).expect("a vertex format"),
        step_mode,
        slot,
    }
}

/// The snapshot of a pipeline whose global uniform buffer holds a matrix
/// that the vertex stage reads and a colour that the fragment stage reads,
/// with a texture and a sampler of the fragment stage, a storage buffer
/// that neither stage uses, two attributes per vertex, one per instance, and
/// one that the vertex entry point does not use.
fn snapshot(depth_format: Option<TextureFormat>) -> PipelineSnapshot {
    let options = RenderPipelineOptions {
        depth_format,
        instance_attributes: vec!["input.offset".to_owned()],
        ..RenderPipelineOptions::default()
    };
    PipelineSnapshot {
        vertex: "vsMain".to_owned(),
        fragment: "fsMain".to_owned(),
        bindings: vec![
            binding(
                "",
                0,
                ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                Resource::UniformBuffer(UniformBlock {
                    size: 80,
                    values: vec![
                        PlainValue {
                            path: "transform".to_owned(),
                            offset: 0,
                            size: 64,
                            scalar: Scalar::Float32,
                            shape: Shape::Matrix {
                                rows: 4,
                                columns: 4,
                            },
                        },
                        PlainValue {
                            path: "tint".to_owned(),
                            offset: 64,
                            size: 16,
                            scalar: Scalar::Float32,
                            shape: Shape::Vector { length: 4 },
                        },
                    ],
                }),
            ),
            binding("image", 1, ShaderStages::FRAGMENT, Resource::SampledTexture),
            binding("imageSampler", 2, ShaderStages::FRAGMENT, Resource::Sampler),
            binding(
                "unused",
                3,
                ShaderStages::NONE,
                Resource::StorageBuffer {
                    access: Access::ReadOnly,
                    stride: 4,
                    element: ElementType::Plain {
                        scalar: Scalar::Float32,
                        components: 1,
                    },
                },
            ),
        ],
        inputs: vec![
            input(
                "input.position",
                0,
                Scalar::Float32,
                2,
                VertexStepMode::Vertex,
                Some(0),
            ),
            input(
                "input.colour",
                1,
                Scalar::Uint32,
                1,
                VertexStepMode::Vertex,
                Some(1),
            ),
            input(
                "input.offset",
                2,
                Scalar::Float32,
                2,
                VertexStepMode::Instance,
                Some(2),
            ),
            input(
                "input.spare",
                3,
                Scalar::Int32,
                3,
                VertexStepMode::Vertex,
                None,
            ),
        ],
        options,
    }
}

fn value<V: Value>(value: V) -> Arg {
    Argument::Value(marshal(&value))
}

fn texture() -> Arg {
    Argument::Texture(BoundTexture {
        handle: (),
        format: TextureFormat::Rgba8Unorm,
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

const IDENTITY: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn arguments() -> Vec<(&'static str, Arg)> {
    vec![
        ("transform", value(IDENTITY)),
        ("tint", value([1.0_f32, 0.5, 0.25, 1.0])),
        ("image", texture()),
        ("imageSampler", sampler()),
    ]
}

fn vertex(scalar: Scalar, components: u32, len: usize) -> BoundVertex<()> {
    BoundVertex {
        handle: (),
        scalar,
        components,
        len,
        on_device: true,
    }
}

fn vertices() -> Vec<(&'static str, BoundVertex<()>)> {
    vec![
        ("input.offset", vertex(Scalar::Float32, 2, 2)),
        ("input.position", vertex(Scalar::Float32, 2, 3)),
        ("input.colour", vertex(Scalar::Uint32, 1, 3)),
    ]
}

fn target(format: TextureFormat, size: [u32; 2]) -> BoundTarget<()> {
    BoundTarget {
        handle: (),
        format,
        size,
        attachable: true,
        on_device: true,
    }
}

fn colour() -> BoundTarget<()> {
    target(TextureFormat::Rgba8Unorm, [8, 8])
}

fn depth() -> BoundTarget<()> {
    target(TextureFormat::Depth32Float, [8, 8])
}

/// The parts of a draw that a test changes, starting from a complete draw
/// without a depth target of 3 vertices and 2 instances.
struct Case {
    snapshot: PipelineSnapshot,
    arguments: Vec<(&'static str, Arg)>,
    vertices: Vec<(&'static str, BoundVertex<()>)>,
    target: Option<BoundTarget<()>>,
    depth: Option<BoundTarget<()>>,
    clear_depth: Option<f32>,
    count: Count,
    instances: u32,
}

impl Case {
    fn new() -> Self {
        Self {
            snapshot: snapshot(None),
            arguments: arguments(),
            vertices: vertices(),
            target: Some(colour()),
            depth: None,
            clear_depth: None,
            count: Count::Vertices(3),
            instances: 2,
        }
    }

    fn with_depth() -> Self {
        Self {
            snapshot: snapshot(Some(TextureFormat::Depth32Float)),
            depth: Some(depth()),
            ..Self::new()
        }
    }

    fn vertex(mut self, path: &'static str, bound: BoundVertex<()>) -> Self {
        match self.vertices.iter_mut().find(|(given, _)| *given == path) {
            Some((_, slot)) => *slot = bound,
            None => self.vertices.push((path, bound)),
        }
        self
    }

    fn without_vertex(mut self, path: &str) -> Self {
        self.vertices.retain(|(given, _)| *given != path);
        self
    }

    fn plan(&self) -> crate::error::Result<DrawPlan> {
        plan_draw(
            &self.snapshot,
            &self.arguments,
            &self.vertices,
            &Targets {
                colour: self.target.as_ref(),
                depth: self.depth.as_ref(),
                clear_depth: self.clear_depth,
            },
            self.count,
            self.instances,
        )
    }

    fn rejected(&self) -> (ErrorKind, String) {
        let error = self.plan().expect_err("the draw is rejected");
        (error.kind(), error.to_string())
    }
}

const PREFIX: &str = "cannot draw with a render pipeline: ";

fn invalid(detail: &str) -> (ErrorKind, String) {
    (ErrorKind::InvalidArgument, format!("{PREFIX}{detail}"))
}

#[test]
fn a_complete_draw_binds_the_used_bindings_and_each_used_attribute_at_its_slot() {
    let plan = Case::new().plan().expect("the draw matches the pipeline");
    let mut uniform = Vec::new();
    for row in IDENTITY {
        for scalar in row {
            uniform.extend_from_slice(&scalar.to_le_bytes());
        }
    }
    for scalar in [1.0_f32, 0.5, 0.25, 1.0] {
        uniform.extend_from_slice(&scalar.to_le_bytes());
    }
    assert_eq!(
        plan,
        DrawPlan {
            plan: Plan {
                groups: vec![vec![
                    Entry {
                        index: 0,
                        source: Source::Uniform(uniform),
                    },
                    Entry {
                        index: 1,
                        source: Source::Argument(2),
                    },
                    Entry {
                        index: 2,
                        source: Source::Argument(3),
                    },
                ]],
                immediates: Vec::new(),
            },
            vertex_buffers: vec![(0, 1), (1, 2), (2, 0)],
        }
    );
}

#[test]
fn a_parameter_that_neither_stage_uses_may_be_omitted_and_a_given_one_is_checked_and_ignored() {
    let mut case = Case::new();
    let without = case.plan().expect("the unused parameter may be omitted");
    case.arguments.push((
        "unused",
        Argument::Buffer(Bound {
            handle: (),
            stride: 4,
            element: marshal(&0.0_f32).ty,
            len: 1,
            on_device: true,
        }),
    ));
    assert_eq!(case.plan().expect("the unused buffer is accepted"), without);
    case.arguments.pop();
    case.arguments.push((
        "unused",
        Argument::Buffer(Bound {
            handle: (),
            stride: 16,
            element: marshal(&[0.0_f32; 4]).ty,
            len: 1,
            on_device: true,
        }),
    ));
    assert_eq!(
        case.rejected(),
        invalid(
            "the parameter `unused` of the render pipeline of `vsMain` and `fsMain` has elements of 4 bytes, and the bound buffer has elements of 16 bytes"
        )
    );
}

#[test]
fn a_buffer_of_another_scalar_type_is_rejected_as_a_call_rejects_it() {
    let mut case = Case::new();
    case.arguments.push((
        "unused",
        Argument::Buffer(Bound {
            handle: (),
            stride: 4,
            element: marshal(&0_u32).ty,
            len: 1,
            on_device: true,
        }),
    ));
    assert_eq!(
        case.rejected(),
        invalid(
            "the parameter `unused` of the render pipeline of `vsMain` and `fsMain` has elements of the Slang type `float`, and the bound buffer has elements of the Rust type `u32`"
        )
    );
}

#[test]
fn the_checks_of_a_parameter_are_those_of_a_call_and_name_both_entry_points() {
    let mut case = Case::new();
    case.arguments.push(("tnit", value(1.0_f32)));
    assert_eq!(
        case.rejected(),
        invalid("the render pipeline of `vsMain` and `fsMain` has no parameter at the path `tnit`")
    );
    let mut case = Case::new();
    case.arguments.retain(|(path, _)| *path != "tint");
    assert_eq!(
        case.rejected(),
        invalid("the parameter `tint` of the render pipeline of `vsMain` and `fsMain` is not set")
    );
    let mut case = Case::new();
    case.arguments.retain(|(path, _)| *path != "image");
    assert_eq!(
        case.rejected(),
        invalid(
            "no texture is bound to the parameter `image` of the render pipeline of `vsMain` and `fsMain`"
        )
    );
    let mut case = Case::new();
    case.arguments.push(("image", sampler()));
    case.arguments.swap_remove(2);
    assert_eq!(
        case.rejected(),
        invalid(
            "the parameter `image` of the render pipeline of `vsMain` and `fsMain` is a texture, and the draw binds a sampler to it"
        )
    );
    let mut case = Case::new();
    case.arguments[2].1 = Argument::Texture(BoundTexture {
        handle: (),
        format: TextureFormat::Depth32Float,
        sampleable: true,
        on_device: true,
    });
    assert_eq!(
        case.rejected(),
        invalid(
            "the texture bound to the parameter `image` of the render pipeline of `vsMain` and `fsMain` has the depth format Depth32Float, which a render pipeline cannot sample"
        )
    );
    let mut case = Case::new();
    case.arguments[0].1 = value(1.0_f32);
    let error = case.plan().expect_err("a float for a matrix");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        format!(
            "{PREFIX}the render pipeline of `vsMain` and `fsMain` cannot take the value of the parameter `transform`"
        )
    );
}

#[test]
fn a_vertex_buffer_for_an_unknown_path_is_an_invalid_argument() {
    let case = Case::new().vertex("input.place", vertex(Scalar::Float32, 2, 3));
    assert_eq!(
        case.rejected(),
        invalid(
            "the render pipeline of `vsMain` and `fsMain` has no vertex attribute at the path `input.place`"
        )
    );
}

#[test]
fn a_vertex_buffer_of_another_device_or_empty_is_an_invalid_argument() {
    let mut other = vertex(Scalar::Float32, 2, 3);
    other.on_device = false;
    assert_eq!(
        Case::new().vertex("input.position", other).rejected(),
        invalid(
            "the vertex buffer of the attribute `input.position` of the render pipeline of `vsMain` and `fsMain` belongs to another device"
        )
    );
    assert_eq!(
        Case::new()
            .vertex("input.position", vertex(Scalar::Float32, 2, 0))
            .rejected(),
        invalid(
            "the vertex buffer of the attribute `input.position` of the render pipeline of `vsMain` and `fsMain` is empty"
        )
    );
}

#[test]
fn a_vertex_buffer_of_another_type_is_an_invalid_argument() {
    for (scalar, components, name) in [
        (Scalar::Float32, 3, "float3"),
        (Scalar::Int32, 2, "int2"),
        (Scalar::Uint32, 2, "uint2"),
        (Scalar::Float32, 1, "float"),
    ] {
        assert_eq!(
            Case::new()
                .vertex("input.position", vertex(scalar, components, 3))
                .rejected(),
            invalid(&format!(
                "the vertex attribute `input.position` of the render pipeline of `vsMain` and `fsMain` has the type float2, and its vertex buffer has elements of the type {name}"
            ))
        );
    }
}

#[test]
fn a_buffer_for_an_unused_attribute_is_checked_and_ignored() {
    let without = Case::new()
        .plan()
        .expect("the unused attribute may be omitted");
    let with = Case::new()
        .vertex("input.spare", vertex(Scalar::Int32, 3, 1))
        .plan()
        .expect("a buffer for the unused attribute is accepted");
    assert_eq!(with, without);
    assert_eq!(
        Case::new()
            .vertex("input.spare", vertex(Scalar::Float32, 3, 1))
            .rejected(),
        invalid(
            "the vertex attribute `input.spare` of the render pipeline of `vsMain` and `fsMain` has the type int3, and its vertex buffer has elements of the type float3"
        )
    );
}

#[test]
fn a_used_attribute_without_a_buffer_is_an_invalid_argument() {
    for path in ["input.position", "input.colour", "input.offset"] {
        assert_eq!(
            Case::new().without_vertex(path).rejected(),
            invalid(&format!(
                "no vertex buffer is given for the vertex attribute `{path}` of the render pipeline of `vsMain` and `fsMain`"
            ))
        );
    }
}

#[test]
fn the_checks_run_in_the_order_of_the_design() {
    let mut case = Case::new()
        .vertex("input.place", vertex(Scalar::Float32, 2, 3))
        .without_vertex("input.colour");
    case.arguments.push(("tnit", value(1.0_f32)));
    case.arguments.retain(|(path, _)| *path != "image");
    case.target = None;
    case.count = Count::Vertices(100);
    assert!(
        case.rejected()
            .1
            .contains("no parameter at the path `tnit`")
    );
    case.arguments.pop();
    case.arguments.push(("tint", value([0.0_f32; 4])));
    assert!(
        case.rejected()
            .1
            .contains("no vertex attribute at the path")
    );
    case.vertices.retain(|(path, _)| *path != "input.place");
    assert!(case.rejected().1.contains("no texture is bound"));
    case.arguments.push(("image", texture()));
    assert!(case.rejected().1.contains("no vertex buffer is given"));
    case = case.vertex("input.colour", vertex(Scalar::Uint32, 1, 3));
    assert!(case.rejected().1.contains("has no colour target"));
    case.target = Some(colour());
    assert!(case.rejected().1.contains("draws 100 vertices"));
    case.count = Count::Vertices(3);
    case.plan().expect("every check passes");
}

#[test]
fn a_draw_without_a_colour_target_is_an_invalid_argument() {
    let mut case = Case::new();
    case.target = None;
    assert_eq!(
        case.rejected(),
        invalid("the draw with the render pipeline of `vsMain` and `fsMain` has no colour target")
    );
}

#[test]
fn a_colour_target_of_another_device_format_or_without_render_attachment_is_an_invalid_argument() {
    let mut case = Case::new();
    let mut other = colour();
    other.on_device = false;
    case.target = Some(other);
    assert_eq!(
        case.rejected(),
        invalid(
            "the colour target of the draw with the render pipeline of `vsMain` and `fsMain` belongs to another device"
        )
    );
    for format in [
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
        TextureFormat::Depth32Float,
    ] {
        case.target = Some(target(format, [8, 8]));
        assert_eq!(
            case.rejected(),
            invalid(&format!(
                "the colour target of the draw with the render pipeline of `vsMain` and `fsMain` has the format {format:?}, and the pipeline draws into the format Rgba8Unorm"
            ))
        );
    }
    let mut unattachable = colour();
    unattachable.attachable = false;
    case.target = Some(unattachable);
    assert_eq!(
        case.rejected(),
        invalid(
            "the colour target of the draw with the render pipeline of `vsMain` and `fsMain` lacks the usage RENDER_ATTACHMENT, which a draw needs to draw into it"
        )
    );
}

#[test]
fn a_depth_target_is_needed_exactly_when_the_pipeline_has_a_depth_format() {
    Case::with_depth().plan().expect("the depth target matches");
    let mut case = Case::new();
    case.depth = Some(depth());
    assert_eq!(
        case.rejected(),
        invalid(
            "the render pipeline of `vsMain` and `fsMain` has no depth format, and the draw has a depth target"
        )
    );
    let mut case = Case::with_depth();
    case.depth = None;
    assert_eq!(
        case.rejected(),
        invalid(
            "the render pipeline of `vsMain` and `fsMain` has a depth format, and the draw has no depth target"
        )
    );
}

#[test]
fn a_depth_target_of_another_device_format_or_size_or_without_render_attachment_is_an_invalid_argument()
 {
    let mut case = Case::with_depth();
    let mut other = depth();
    other.on_device = false;
    case.depth = Some(other);
    assert_eq!(
        case.rejected(),
        invalid(
            "the depth target of the draw with the render pipeline of `vsMain` and `fsMain` belongs to another device"
        )
    );
    case.depth = Some(target(TextureFormat::Rgba8Unorm, [8, 8]));
    assert_eq!(
        case.rejected(),
        invalid(
            "the depth target of the draw with the render pipeline of `vsMain` and `fsMain` has the format Rgba8Unorm, and the pipeline draws into the format Depth32Float"
        )
    );
    let mut unattachable = depth();
    unattachable.attachable = false;
    case.depth = Some(unattachable);
    assert!(
        case.rejected()
            .1
            .contains("lacks the usage RENDER_ATTACHMENT")
    );
    for size in [[8, 4], [4, 8], [16, 16]] {
        case.depth = Some(target(TextureFormat::Depth32Float, size));
        let [width, height] = size;
        assert_eq!(
            case.rejected(),
            invalid(&format!(
                "the depth target of the draw with the render pipeline of `vsMain` and `fsMain` has {width} by {height} pixels, and the colour target has 8 by 8"
            ))
        );
    }
}

#[test]
fn a_depth_clear_value_from_zero_to_one_passes_and_others_are_an_invalid_argument() {
    for new in [Case::with_depth, Case::new] {
        for depth in [0.0, -0.0, 0.5, 1.0] {
            let mut case = new();
            case.clear_depth = Some(depth);
            case.plan().expect("the depth clear value is in range");
        }
        for (depth, shown) in [
            (-0.1, "-0.1"),
            (1.1, "1.1"),
            (f32::INFINITY, "inf"),
            (f32::NEG_INFINITY, "-inf"),
            (f32::NAN, "NaN"),
        ] {
            let mut case = new();
            case.clear_depth = Some(depth);
            assert_eq!(
                case.rejected(),
                invalid(&format!(
                    "the draw with the render pipeline of `vsMain` and `fsMain` clears the depth target to {shown}, and a depth is from 0.0 to 1.0"
                ))
            );
        }
    }
}

#[test]
fn a_depth_clear_value_is_checked_after_the_targets_and_before_the_counts() {
    let mut case = Case::with_depth();
    case.clear_depth = Some(2.0);
    case.depth = None;
    case.count = Count::Vertices(100);
    assert!(case.rejected().1.contains("has no depth target"));
    case.depth = Some(depth());
    assert!(case.rejected().1.contains("clears the depth target to 2.0"));
    case.clear_depth = Some(1.0);
    assert!(case.rejected().1.contains("draws 100 vertices"));
}

#[test]
fn vertex_buffers_of_one_step_mode_with_different_lengths_are_an_invalid_argument() {
    assert_eq!(
        Case::new()
            .vertex("input.colour", vertex(Scalar::Uint32, 1, 4))
            .rejected(),
        invalid(
            "the vertex buffers of the attributes `input.position` and `input.colour` of the render pipeline of `vsMain` and `fsMain`, which step once per vertex, have the different lengths 3 and 4"
        )
    );
}

#[test]
fn a_buffer_of_an_unused_attribute_does_not_bound_the_counts() {
    Case::new()
        .vertex("input.spare", vertex(Scalar::Int32, 3, 1))
        .plan()
        .expect("the unused attribute's length does not count");
}

#[test]
fn counts_up_to_the_lengths_of_the_buffers_pass_and_larger_ones_are_an_invalid_argument() {
    let mut case = Case::new();
    for (vertices, instances) in [(0, 0), (3, 2), (1, 1), (0, 2), (3, 0)] {
        case.count = Count::Vertices(vertices);
        case.instances = instances;
        case.plan().expect("the counts are within the buffers");
    }
    case.count = Count::Vertices(4);
    case.instances = 1;
    assert_eq!(
        case.rejected(),
        invalid(
            "the draw with the render pipeline of `vsMain` and `fsMain` draws 4 vertices, and the vertex buffers that step once per vertex hold 3"
        )
    );
    case.count = Count::Vertices(3);
    case.instances = 3;
    assert_eq!(
        case.rejected(),
        invalid(
            "the draw with the render pipeline of `vsMain` and `fsMain` draws 3 instances, and the vertex buffers that step once per instance hold 2"
        )
    );
}

#[test]
fn counts_without_buffers_of_their_step_mode_have_no_bound() {
    let mut snapshot = snapshot(None);
    snapshot.inputs.clear();
    let case = Case {
        snapshot,
        vertices: Vec::new(),
        count: Count::Vertices(u32::MAX),
        instances: u32::MAX,
        ..Case::new()
    };
    case.plan().expect("nothing bounds the counts");
}

fn indices(len: usize, count: u32) -> Count {
    Count::Indices {
        buffer: BoundIndex {
            len,
            on_device: true,
        },
        count,
    }
}

#[test]
fn an_indexed_draw_is_bounded_by_its_index_buffer_and_not_by_the_vertex_buffers() {
    let mut case = Case::new();
    for count in [0, 3, 6] {
        case.count = indices(6, count);
        case.plan()
            .expect("the index count is within the index buffer");
    }
    case.count = indices(6, 7);
    assert_eq!(
        case.rejected(),
        invalid(
            "the draw with the render pipeline of `vsMain` and `fsMain` draws 7 indices, and the index buffer holds 6"
        )
    );
    case.count = indices(0, 0);
    assert_eq!(
        case.rejected(),
        invalid(
            "the index buffer of the draw with the render pipeline of `vsMain` and `fsMain` is empty"
        )
    );
    case.count = Count::Indices {
        buffer: BoundIndex {
            len: 6,
            on_device: false,
        },
        count: 6,
    };
    assert_eq!(
        case.rejected(),
        invalid(
            "the index buffer of the draw with the render pipeline of `vsMain` and `fsMain` belongs to another device"
        )
    );
    case.count = indices(6, 6);
    case.instances = 3;
    assert!(case.rejected().1.contains("draws 3 instances"));
}

fn format_of<T: VertexElement>() -> Option<VertexFormat> {
    let (scalar, components) = element_type::<T>();
    vertex_format(scalar, components)
}

#[test]
fn each_vertex_element_type_has_the_vertex_format_of_its_attribute() {
    let formats = [
        (format_of::<f32>(), VertexFormat::Float32),
        (format_of::<[f32; 2]>(), VertexFormat::Float32x2),
        (format_of::<[f32; 3]>(), VertexFormat::Float32x3),
        (format_of::<[f32; 4]>(), VertexFormat::Float32x4),
        (format_of::<i32>(), VertexFormat::Sint32),
        (format_of::<[i32; 2]>(), VertexFormat::Sint32x2),
        (format_of::<[i32; 3]>(), VertexFormat::Sint32x3),
        (format_of::<[i32; 4]>(), VertexFormat::Sint32x4),
        (format_of::<u32>(), VertexFormat::Uint32),
        (format_of::<[u32; 2]>(), VertexFormat::Uint32x2),
        (format_of::<[u32; 3]>(), VertexFormat::Uint32x3),
        (format_of::<[u32; 4]>(), VertexFormat::Uint32x4),
    ];
    for (given, expected) in formats {
        assert_eq!(given, Some(expected));
    }
}

#[test]
fn the_stride_of_each_vertex_element_type_is_the_size_of_its_vertex_format() {
    fn stride_matches<T: VertexElement>() -> bool {
        format_of::<T>().is_some_and(|format| {
            u64::try_from(T::STRIDE).is_ok_and(|stride| stride == format.size())
        })
    }
    assert!(stride_matches::<f32>());
    assert!(stride_matches::<[f32; 2]>());
    assert!(stride_matches::<[f32; 3]>());
    assert!(stride_matches::<[f32; 4]>());
    assert!(stride_matches::<i32>());
    assert!(stride_matches::<[i32; 2]>());
    assert!(stride_matches::<[i32; 3]>());
    assert!(stride_matches::<[i32; 4]>());
    assert!(stride_matches::<u32>());
    assert!(stride_matches::<[u32; 2]>());
    assert!(stride_matches::<[u32; 3]>());
    assert!(stride_matches::<[u32; 4]>());
}

fn assert_send_sync<T: Send + Sync>() {}

#[test]
fn a_draw_is_send_and_sync() {
    assert_send_sync::<Draw<'_>>();
}
