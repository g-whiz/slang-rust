use std::error::Error as StdError;

use super::{
    Blend, CullMode, Names, PipelineBinding, RenderPipeline, RenderPipelineOptions, Stages,
    StepMode, VertexInput, merge_bindings, select, snapshot, vertex_format_supported,
    vertex_inputs,
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

fn rejected(
    name: &str,
    stage: RenderStage,
    kind: ErrorKind,
    detail: &str,
    source: Option<&str>,
) -> CompiledRender {
    CompiledRender {
        name: name.to_owned(),
        stage,
        usable: Err(Rejection {
            kind,
            detail: detail.to_owned(),
            source: source.map(str::to_owned),
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
            None,
        ),
        rejected(
            "fsBroken",
            RenderStage::Fragment,
            ErrorKind::Compile,
            "Slang failed on the fragment entry point `fsBroken`",
            Some("cannot generate code: no code"),
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
    let broken = select_error("vsMain", "fsBroken");
    assert_error(
        &broken,
        ErrorKind::Compile,
        "cannot create a render pipeline: for `vsMain` and `fsBroken`, Slang failed on the fragment entry point `fsBroken`",
    );
    assert_eq!(
        StdError::source(&broken)
            .map(ToString::to_string)
            .as_deref(),
        Some("cannot generate code: no code")
    );
    assert!(StdError::source(&select_error("vsRejected", "fsMain")).is_none());
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

fn merged(binding: Binding, visibility: Stages) -> PipelineBinding {
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
                Stages::VERTEX | Stages::FRAGMENT
            ),
            merged(binding(0, 1, Usage::Used, storage), Stages::VERTEX),
            merged(
                binding(0, 2, Usage::Used, Resource::SampledTexture),
                Stages::FRAGMENT
            ),
            merged(
                binding(0, 3, Usage::Unused, Resource::Sampler),
                Stages::NONE
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
                Stages::FRAGMENT
            ),
            merged(binding(1, 0, Usage::Used, uniform(16)), Stages::VERTEX),
            merged(
                binding(2, 0, Usage::Unused, Resource::SampledTexture),
                Stages::NONE
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
    let input = |attribute: &Attribute, step_mode, slot| VertexInput {
        attribute: attribute.clone(),
        step_mode,
        slot,
    };
    assert_eq!(
        inputs,
        [
            input(&attributes[1], StepMode::Vertex, Some(0)),
            input(&attributes[2], StepMode::Instance, None),
            input(&attributes[0], StepMode::Vertex, Some(1)),
            input(&attributes[3], StepMode::Instance, Some(2)),
        ]
    );
}

#[test]
fn a_vertex_entry_point_without_used_attributes_has_no_slot() {
    let attributes = [attribute("unused", 0, Scalar::Float32, 1, Usage::Unused)];
    let inputs = vertex_inputs(&attributes, NAMES, &RenderPipelineOptions::default())
        .expect("the attribute is valid");
    assert!(inputs.iter().all(|input| input.slot.is_none()));
    assert_eq!(
        vertex_inputs(&[], NAMES, &RenderPipelineOptions::default())
            .expect("no attribute is valid"),
        []
    );
}

#[test]
fn a_vertex_format_holds_one_to_four_components_of_a_float_or_an_integer() {
    for scalar in [Scalar::Float32, Scalar::Int32, Scalar::Uint32] {
        for components in 1..=4 {
            assert!(vertex_format_supported(scalar, components));
        }
        assert!(!vertex_format_supported(scalar, 0));
        assert!(!vertex_format_supported(scalar, 5));
    }
    assert!(!vertex_format_supported(Scalar::Bool, 1));
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

fn assert_clone<T: Clone + 'static>() {}

fn assert_send_sync_clone<T: Send + Sync + Clone + 'static>() {}

#[test]
fn render_pipelines_are_clone_and_their_options_are_send_sync_and_clone() {
    assert_clone::<RenderPipeline>();
    assert_send_sync_clone::<RenderPipelineOptions>();
    assert_send_sync_clone::<Blend>();
    assert_send_sync_clone::<CullMode>();
}
