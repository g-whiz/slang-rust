use wgpu::ShaderStages;

use super::{
    Blend, CullMode, Names, PipelineBinding, RenderPipeline, RenderPipelineOptions, VertexBuffer,
    VertexInput, blend_state, colour_target, cull_face, depth_stencil, merge_bindings, primitive,
    render_pipeline_error_kind, select, snapshot, vertex_buffer_layouts, vertex_buffers,
    vertex_format, vertex_inputs,
};
use crate::error::{Error, ErrorKind};
use crate::layout::{
    Access, Attribute, Binding, CompiledRender, ElementType, Rejection, RenderLayout, RenderStage,
    Resource, Scalar, UniformBlock, UsableRender, Usage,
};
use crate::texture::TextureFormat;

const NAMES: Names<'static> = Names {
    vertex: "vsMain",
    fragment: "fsMain",
};

fn binding(set: u32, index: u32, usage: Usage, resource: Resource) -> Binding {
    Binding {
        path: format!("parameter_{set}_{index}"),
        set,
        index,
        usage,
        resource,
    }
}

fn uniform(size: u64) -> Resource {
    Resource::UniformBuffer(UniformBlock {
        size,
        values: Vec::new(),
    })
}

fn attribute(
    path: &str,
    location: u32,
    scalar: Scalar,
    components: u32,
    usage: Usage,
) -> Attribute {
    Attribute {
        path: path.to_owned(),
        location,
        scalar,
        components,
        usage,
    }
}

fn usable(bindings: Vec<Binding>, attributes: Vec<Attribute>) -> UsableRender {
    UsableRender {
        layout: RenderLayout {
            bindings,
            immediates: None,
            attributes,
        },
        spirv: Vec::new(),
    }
}

fn record(name: &str, stage: RenderStage, usable: UsableRender) -> CompiledRender {
    CompiledRender {
        name: name.to_owned(),
        stage,
        usable: Ok(usable),
    }
}

fn rejected(name: &str, stage: RenderStage, kind: ErrorKind, detail: &str) -> CompiledRender {
    CompiledRender {
        name: name.to_owned(),
        stage,
        usable: Err(Rejection {
            kind,
            detail: detail.to_owned(),
        }),
    }
}

fn records() -> Vec<CompiledRender> {
    vec![
        record(
            "vsMain",
            RenderStage::Vertex,
            usable(
                Vec::new(),
                vec![attribute(
                    "input.position",
                    0,
                    Scalar::Float32,
                    2,
                    Usage::Used,
                )],
            ),
        ),
        record(
            "fsMain",
            RenderStage::Fragment,
            usable(Vec::new(), Vec::new()),
        ),
        record(
            "vsOther",
            RenderStage::Vertex,
            usable(Vec::new(), Vec::new()),
        ),
        rejected(
            "vsRejected",
            RenderStage::Vertex,
            ErrorKind::UnsupportedParameter,
            "the vertex entry point `vsRejected` has the parameter `table` of an unsupported kind",
        ),
        rejected(
            "fsBroken",
            RenderStage::Fragment,
            ErrorKind::Compile,
            "Slang failed on the fragment entry point `fsBroken`: no code",
        ),
    ]
}

fn assert_error(error: &Error, kind: ErrorKind, text: &str) {
    assert_eq!(error.kind(), kind);
    assert_eq!(error.to_string(), text);
}

fn select_error(vertex: &'static str, fragment: &'static str) -> Error {
    let records = records();
    select(&records, Names { vertex, fragment }).expect_err("the selection fails")
}

#[test]
fn select_returns_the_records_of_the_two_named_entry_points() {
    let records = records();
    let (vertex, fragment) = select(&records, NAMES).expect("both entry points are usable");
    assert_eq!(vertex.layout.attributes.len(), 1);
    assert_eq!(fragment.layout.attributes, []);
}

#[test]
fn an_unknown_vertex_or_fragment_name_is_an_invalid_argument() {
    assert_error(
        &select_error("missing", "fsMain"),
        ErrorKind::InvalidArgument,
        "cannot create a render pipeline: for `missing` and `fsMain`, the module has no vertex entry point named `missing`",
    );
    assert_error(
        &select_error("vsMain", "missing"),
        ErrorKind::InvalidArgument,
        "cannot create a render pipeline: for `vsMain` and `missing`, the module has no fragment entry point named `missing`",
    );
}

#[test]
fn a_name_of_the_other_stage_is_an_invalid_argument() {
    assert_error(
        &select_error("fsMain", "fsMain"),
        ErrorKind::InvalidArgument,
        "cannot create a render pipeline: for `fsMain` and `fsMain`, the module has no vertex entry point named `fsMain`",
    );
    assert_error(
        &select_error("vsMain", "vsOther"),
        ErrorKind::InvalidArgument,
        "cannot create a render pipeline: for `vsMain` and `vsOther`, the module has no fragment entry point named `vsOther`",
    );
}

#[test]
fn a_rejected_entry_point_returns_its_rejection() {
    assert_error(
        &select_error("vsRejected", "fsMain"),
        ErrorKind::UnsupportedParameter,
        "cannot create a render pipeline: for `vsRejected` and `fsMain`, the vertex entry point `vsRejected` has the parameter `table` of an unsupported kind",
    );
    assert_error(
        &select_error("vsMain", "fsBroken"),
        ErrorKind::Compile,
        "cannot create a render pipeline: for `vsMain` and `fsBroken`, Slang failed on the fragment entry point `fsBroken`: no code",
    );
}

#[test]
fn the_vertex_name_is_checked_before_the_fragment_name_and_names_before_rejections() {
    assert_eq!(
        select_error("missing", "missing").to_string(),
        "cannot create a render pipeline: for `missing` and `missing`, the module has no vertex entry point named `missing`"
    );
    assert_eq!(
        select_error("vsRejected", "missing").kind(),
        ErrorKind::InvalidArgument
    );
}

fn snapshot_error(
    vertex: &UsableRender,
    fragment: &UsableRender,
    options: &RenderPipelineOptions,
) -> Error {
    snapshot(vertex, fragment, NAMES, options).expect_err("the snapshot fails")
}

#[test]
fn uniform_parameters_of_either_entry_point_are_unsupported() {
    let plain = usable(Vec::new(), Vec::new());
    let mut with_immediates = usable(Vec::new(), Vec::new());
    with_immediates.layout.immediates = Some(UniformBlock {
        size: 16,
        values: Vec::new(),
    });
    let options = RenderPipelineOptions::default();
    assert_error(
        &snapshot_error(&with_immediates, &plain, &options),
        ErrorKind::UnsupportedParameter,
        "cannot create a render pipeline: for `vsMain` and `fsMain`, the vertex entry point `vsMain` has `uniform` parameters of its own, which a render pipeline does not support, so declare them as global `uniform` values or in a constant buffer or parameter block",
    );
    assert_error(
        &snapshot_error(&plain, &with_immediates, &options),
        ErrorKind::UnsupportedParameter,
        "cannot create a render pipeline: for `vsMain` and `fsMain`, the fragment entry point `fsMain` has `uniform` parameters of its own, which a render pipeline does not support, so declare them as global `uniform` values or in a constant buffer or parameter block",
    );
}

#[test]
fn a_depth_colour_format_or_a_colour_depth_format_is_an_invalid_argument() {
    let plain = usable(Vec::new(), Vec::new());
    let options = RenderPipelineOptions {
        colour_format: TextureFormat::Depth32Float,
        ..RenderPipelineOptions::default()
    };
    assert_error(
        &snapshot_error(&plain, &plain, &options),
        ErrorKind::InvalidArgument,
        "cannot create a render pipeline: for `vsMain` and `fsMain`, `colour_format` is the depth format Depth32Float and not a colour format",
    );
    let options = RenderPipelineOptions {
        depth_format: Some(TextureFormat::Bgra8UnormSrgb),
        ..RenderPipelineOptions::default()
    };
    assert_error(
        &snapshot_error(&plain, &plain, &options),
        ErrorKind::InvalidArgument,
        "cannot create a render pipeline: for `vsMain` and `fsMain`, `depth_format` is the colour format Bgra8UnormSrgb and not the depth format Depth32Float",
    );
}

#[test]
fn every_colour_format_with_and_without_the_depth_format_is_accepted() {
    let plain = usable(Vec::new(), Vec::new());
    for colour_format in [
        TextureFormat::Rgba8Unorm,
        TextureFormat::Rgba8UnormSrgb,
        TextureFormat::Bgra8Unorm,
        TextureFormat::Bgra8UnormSrgb,
    ] {
        for depth_format in [None, Some(TextureFormat::Depth32Float)] {
            let options = RenderPipelineOptions {
                colour_format,
                depth_format,
                ..RenderPipelineOptions::default()
            };
            let snapshot =
                snapshot(&plain, &plain, NAMES, &options).expect("the options are valid");
            assert_eq!(snapshot.options, options);
            assert_eq!(snapshot.vertex, "vsMain");
            assert_eq!(snapshot.fragment, "fsMain");
        }
    }
}

#[test]
fn an_instance_attribute_that_names_no_attribute_is_an_invalid_argument() {
    let vertex = usable(
        Vec::new(),
        vec![attribute(
            "input.position",
            0,
            Scalar::Float32,
            2,
            Usage::Used,
        )],
    );
    let fragment = usable(Vec::new(), Vec::new());
    let options = RenderPipelineOptions {
        instance_attributes: vec!["input.position".to_owned(), "input.offset".to_owned()],
        ..RenderPipelineOptions::default()
    };
    assert_error(
        &snapshot_error(&vertex, &fragment, &options),
        ErrorKind::InvalidArgument,
        "cannot create a render pipeline: for `vsMain` and `fsMain`, `instance_attributes` names `input.offset`, which is no vertex attribute of the vertex entry point `vsMain`",
    );
}

#[test]
fn the_options_are_checked_after_the_uniform_parameters() {
    let mut with_immediates = usable(Vec::new(), Vec::new());
    with_immediates.layout.immediates = Some(UniformBlock {
        size: 4,
        values: Vec::new(),
    });
    let options = RenderPipelineOptions {
        colour_format: TextureFormat::Depth32Float,
        instance_attributes: vec!["missing".to_owned()],
        ..RenderPipelineOptions::default()
    };
    assert_eq!(
        snapshot_error(&with_immediates, &with_immediates, &options).kind(),
        ErrorKind::UnsupportedParameter
    );
}

fn merged(binding: Binding, visibility: ShaderStages) -> PipelineBinding {
    PipelineBinding {
        binding,
        visibility,
    }
}

#[test]
fn a_binding_is_visible_to_exactly_the_stages_that_use_it() {
    let storage = Resource::StorageBuffer {
        access: Access::ReadOnly,
        stride: 16,
        element: ElementType::Composite,
    };
    let vertex = [
        binding(0, 0, Usage::Used, uniform(64)),
        binding(0, 1, Usage::Used, storage.clone()),
        binding(0, 2, Usage::Unused, Resource::SampledTexture),
        binding(0, 3, Usage::Unused, Resource::Sampler),
    ];
    let fragment = [
        binding(0, 0, Usage::Used, uniform(64)),
        binding(0, 1, Usage::Unused, storage.clone()),
        binding(0, 2, Usage::Used, Resource::SampledTexture),
        binding(0, 3, Usage::Unused, Resource::Sampler),
    ];
    let merged_bindings = merge_bindings(&vertex, &fragment, NAMES).expect("the bindings agree");
    assert_eq!(
        merged_bindings,
        [
            merged(
                binding(0, 0, Usage::Used, uniform(64)),
                ShaderStages::VERTEX | ShaderStages::FRAGMENT
            ),
            merged(binding(0, 1, Usage::Used, storage), ShaderStages::VERTEX),
            merged(
                binding(0, 2, Usage::Used, Resource::SampledTexture),
                ShaderStages::FRAGMENT
            ),
            merged(
                binding(0, 3, Usage::Unused, Resource::Sampler),
                ShaderStages::NONE
            ),
        ]
    );
}

#[test]
fn bindings_of_one_stage_only_are_merged_in_the_order_of_sets_and_bindings() {
    let vertex = [binding(1, 0, Usage::Used, uniform(16))];
    let fragment = [
        binding(0, 1, Usage::Used, Resource::Sampler),
        binding(2, 0, Usage::Unused, Resource::SampledTexture),
    ];
    let merged_bindings = merge_bindings(&vertex, &fragment, NAMES).expect("the bindings agree");
    assert_eq!(
        merged_bindings,
        [
            merged(
                binding(0, 1, Usage::Used, Resource::Sampler),
                ShaderStages::FRAGMENT
            ),
            merged(
                binding(1, 0, Usage::Used, uniform(16)),
                ShaderStages::VERTEX
            ),
            merged(
                binding(2, 0, Usage::Unused, Resource::SampledTexture),
                ShaderStages::NONE
            ),
        ]
    );
}

#[test]
fn two_layouts_at_one_binding_are_an_unsupported_parameter() {
    let vertex = [binding(0, 0, Usage::Used, uniform(16))];
    let fragment = [binding(0, 0, Usage::Used, uniform(32))];
    assert_error(
        &merge_bindings(&vertex, &fragment, NAMES).expect_err("the layouts differ"),
        ErrorKind::UnsupportedParameter,
        "cannot create a render pipeline: for `vsMain` and `fsMain`, Slang reports the parameters `parameter_0_0` and `parameter_0_0` at set 0, binding 0 with different layouts",
    );
    let mut renamed = binding(0, 0, Usage::Used, uniform(16));
    renamed.path = "other".to_owned();
    assert_error(
        &merge_bindings(&vertex, &[renamed], NAMES).expect_err("the paths differ"),
        ErrorKind::UnsupportedParameter,
        "cannot create a render pipeline: for `vsMain` and `fsMain`, Slang reports the parameters `parameter_0_0` and `other` at set 0, binding 0 with different layouts",
    );
}

#[test]
fn used_attributes_take_slots_in_the_order_of_locations_and_unused_ones_none() {
    let attributes = [
        attribute("input.uv", 2, Scalar::Float32, 2, Usage::Used),
        attribute("input.position", 0, Scalar::Float32, 3, Usage::Used),
        attribute("input.unused", 1, Scalar::Float32, 4, Usage::Unused),
        attribute("offset", 3, Scalar::Int32, 2, Usage::Used),
    ];
    let options = RenderPipelineOptions {
        instance_attributes: vec!["offset".to_owned(), "input.unused".to_owned()],
        ..RenderPipelineOptions::default()
    };
    let inputs = vertex_inputs(&attributes, NAMES, &options).expect("the attributes are valid");
    let input = |attribute: &Attribute, format, step_mode, slot| VertexInput {
        attribute: attribute.clone(),
        format,
        step_mode,
        slot,
    };
    assert_eq!(
        inputs,
        [
            input(
                &attributes[1],
                wgpu::VertexFormat::Float32x3,
                wgpu::VertexStepMode::Vertex,
                Some(0)
            ),
            input(
                &attributes[2],
                wgpu::VertexFormat::Float32x4,
                wgpu::VertexStepMode::Instance,
                None
            ),
            input(
                &attributes[0],
                wgpu::VertexFormat::Float32x2,
                wgpu::VertexStepMode::Vertex,
                Some(1)
            ),
            input(
                &attributes[3],
                wgpu::VertexFormat::Sint32x2,
                wgpu::VertexStepMode::Instance,
                Some(2)
            ),
        ]
    );
    let buffers = vertex_buffers(&inputs);
    let buffer = |format, shader_location, step_mode| VertexBuffer {
        attribute: wgpu::VertexAttribute {
            format,
            offset: 0,
            shader_location,
        },
        step_mode,
    };
    assert_eq!(
        buffers,
        [
            buffer(
                wgpu::VertexFormat::Float32x3,
                0,
                wgpu::VertexStepMode::Vertex
            ),
            buffer(
                wgpu::VertexFormat::Float32x2,
                2,
                wgpu::VertexStepMode::Vertex
            ),
            buffer(
                wgpu::VertexFormat::Sint32x2,
                3,
                wgpu::VertexStepMode::Instance
            ),
        ]
    );
    let layouts = vertex_buffer_layouts(&buffers);
    let strides: Vec<Option<(u64, wgpu::VertexStepMode, usize)>> = layouts
        .iter()
        .map(|layout| {
            layout.as_ref().map(|layout| {
                (
                    layout.array_stride,
                    layout.step_mode,
                    layout.attributes.len(),
                )
            })
        })
        .collect();
    assert_eq!(
        strides,
        [
            Some((12, wgpu::VertexStepMode::Vertex, 1)),
            Some((8, wgpu::VertexStepMode::Vertex, 1)),
            Some((8, wgpu::VertexStepMode::Instance, 1)),
        ]
    );
    let first = layouts[0].as_ref().map(|layout| layout.attributes[0]);
    assert_eq!(first, Some(buffers[0].attribute));
}

#[test]
fn a_vertex_entry_point_without_used_attributes_has_no_vertex_buffer() {
    let attributes = [attribute("unused", 0, Scalar::Float32, 1, Usage::Unused)];
    let inputs = vertex_inputs(&attributes, NAMES, &RenderPipelineOptions::default())
        .expect("the attribute is valid");
    assert_eq!(vertex_buffers(&inputs), []);
    assert_eq!(
        vertex_inputs(&[], NAMES, &RenderPipelineOptions::default())
            .expect("no attribute is valid"),
        []
    );
}

#[test]
fn each_scalar_type_and_component_count_has_its_vertex_format() {
    use wgpu::VertexFormat as F;
    for (scalar, formats) in [
        (
            Scalar::Float32,
            [F::Float32, F::Float32x2, F::Float32x3, F::Float32x4],
        ),
        (
            Scalar::Int32,
            [F::Sint32, F::Sint32x2, F::Sint32x3, F::Sint32x4],
        ),
        (
            Scalar::Uint32,
            [F::Uint32, F::Uint32x2, F::Uint32x3, F::Uint32x4],
        ),
    ] {
        for (components, format) in (1..=4).zip(formats) {
            assert_eq!(vertex_format(scalar, components), Some(format));
            assert_eq!(format.size(), 4 * u64::from(components));
        }
        assert_eq!(vertex_format(scalar, 0), None);
        assert_eq!(vertex_format(scalar, 5), None);
    }
    assert_eq!(vertex_format(Scalar::Bool, 1), None);
}

#[test]
fn an_attribute_without_a_vertex_format_is_an_unsupported_parameter() {
    let attributes = [attribute("flag", 0, Scalar::Bool, 1, Usage::Used)];
    assert_error(
        &vertex_inputs(&attributes, NAMES, &RenderPipelineOptions::default())
            .expect_err("no vertex format holds a bool"),
        ErrorKind::UnsupportedParameter,
        "cannot create a render pipeline: for `vsMain` and `fsMain`, Slang reports the vertex attribute `flag` with a type that no vertex format holds",
    );
}

#[test]
fn default_options_replace_into_rgba8_without_depth_culling_or_instances() {
    let options = RenderPipelineOptions::default();
    assert_eq!(options.colour_format, TextureFormat::Rgba8Unorm);
    assert_eq!(options.depth_format, None);
    assert_eq!(options.blend, Blend::Replace);
    assert_eq!(options.cull_mode, CullMode::None);
    assert_eq!(options.instance_attributes, Vec::<String>::new());
}

#[test]
fn blend_and_cull_mode_map_to_their_wgpu_states() {
    assert_eq!(blend_state(Blend::Replace), None);
    assert_eq!(
        blend_state(Blend::Alpha),
        Some(wgpu::BlendState::ALPHA_BLENDING)
    );
    assert_eq!(cull_face(CullMode::None), None);
    assert_eq!(cull_face(CullMode::Back), Some(wgpu::Face::Back));
}

#[test]
fn the_colour_target_has_the_colour_format_and_writes_every_channel() {
    let options = RenderPipelineOptions {
        colour_format: TextureFormat::Bgra8UnormSrgb,
        blend: Blend::Alpha,
        ..RenderPipelineOptions::default()
    };
    assert_eq!(
        colour_target(&options),
        wgpu::ColorTargetState {
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            blend: Some(wgpu::BlendState::ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        }
    );
}

#[test]
fn the_primitive_state_is_a_triangle_list_with_counter_clockwise_front_faces() {
    let options = RenderPipelineOptions {
        cull_mode: CullMode::Back,
        ..RenderPipelineOptions::default()
    };
    assert_eq!(
        primitive(&options),
        wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: Some(wgpu::Face::Back),
            unclipped_depth: false,
            polygon_mode: wgpu::PolygonMode::Fill,
            conservative: false,
        }
    );
}

#[test]
fn a_depth_format_tests_less_and_writes_depth() {
    assert_eq!(depth_stencil(None), None);
    assert_eq!(
        depth_stencil(Some(TextureFormat::Depth32Float)),
        Some(wgpu::DepthStencilState {
            format: wgpu::TextureFormat::Depth32Float,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: wgpu::StencilState::default(),
            bias: wgpu::DepthBiasState::default(),
        })
    );
}

fn source() -> wgpu::ErrorSource {
    Box::new(std::io::Error::other("the backend failed"))
}

#[test]
fn a_validation_or_internal_error_of_the_render_pipeline_is_an_unsupported_shader() {
    let validation = wgpu::Error::Validation {
        source: source(),
        description: "validation".to_owned(),
    };
    assert_eq!(
        render_pipeline_error_kind(&validation),
        ErrorKind::UnsupportedShader
    );
    let internal = wgpu::Error::Internal {
        source: source(),
        description: "internal".to_owned(),
    };
    assert_eq!(
        render_pipeline_error_kind(&internal),
        ErrorKind::UnsupportedShader
    );
    let out_of_memory = wgpu::Error::OutOfMemory { source: source() };
    assert_eq!(
        render_pipeline_error_kind(&out_of_memory),
        ErrorKind::Backend
    );
}

fn assert_send_sync_clone<T: Send + Sync + Clone + 'static>() {}

#[test]
fn render_pipelines_and_their_options_are_send_sync_and_clone() {
    assert_send_sync_clone::<RenderPipeline>();
    assert_send_sync_clone::<RenderPipelineOptions>();
    assert_send_sync_clone::<Blend>();
    assert_send_sync_clone::<CullMode>();
}
