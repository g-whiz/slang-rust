use std::collections::HashMap;

use super::{BindingKey, StreamBuffer, scope, streams, vertex_format};
use crate::error::{Error, ErrorKind};
use crate::layout::{
    Attribute, Binding, Place, RenderStage, Resource, Scalar, Semantic, UniformBlock, Usage,
};
use crate::render::{
    BindingKeys, Names, PipelineBinding, SlotKey, Stages, StepMode, VertexInput, merge_bindings,
};

const NAMES: Names<'static> = Names {
    vertex: "vsMain",
    fragment: "fsMain",
};

fn input(
    path: &str,
    location: u32,
    components: u32,
    slot: Option<u32>,
    step_mode: StepMode,
) -> VertexInput {
    VertexInput {
        attribute: Attribute {
            path: path.to_owned(),
            location,
            scalar: Scalar::Float32,
            components,
            usage: if slot.is_some() {
                Usage::Used
            } else {
                Usage::Unused
            },
        },
        step_mode,
        slot,
    }
}

#[test]
fn a_used_attribute_reads_its_slot_and_an_unused_one_the_zeros_with_the_stride_0() {
    let inputs = [
        input("input.position", 0, 2, Some(0), StepMode::Vertex),
        input("input.unused", 1, 4, None, StepMode::Instance),
        input("input.offset", 2, 3, Some(1), StepMode::Instance),
    ];
    let semantics: [Semantic; 3] = [
        ("POSITION".to_owned(), 0),
        ("UNUSED".to_owned(), 0),
        ("TEXCOORD".to_owned(), 3),
    ];
    let by_path: HashMap<&str, &Semantic> = inputs
        .iter()
        .map(|input| input.attribute.path.as_str())
        .zip(&semantics)
        .collect();
    let (streams, buffers) =
        streams(&inputs, &by_path, NAMES).expect("every attribute has a semantic");
    assert_eq!(
        buffers,
        [
            StreamBuffer::Slot(0),
            StreamBuffer::Zeros,
            StreamBuffer::Slot(1)
        ]
    );
    let expected = [
        slang_rhi::VertexStream::new(slang_rhi::VertexFormat::Float32x2, 0)
            .with_semantic("POSITION", 0)
            .with_step(slang_rhi::StepMode::Vertex),
        slang_rhi::VertexStream::new(slang_rhi::VertexFormat::Float32x4, 1)
            .with_semantic("UNUSED", 0)
            .with_stride(0),
        slang_rhi::VertexStream::new(slang_rhi::VertexFormat::Float32x3, 2)
            .with_semantic("TEXCOORD", 3)
            .with_step(slang_rhi::StepMode::Instance),
    ];
    assert_eq!(streams, expected);
}

#[test]
fn an_attribute_without_a_semantic_is_an_unsupported_parameter() {
    let inputs = [input("position", 0, 3, Some(0), StepMode::Vertex)];
    let error = streams(&inputs, &HashMap::new(), NAMES).expect_err("no semantic");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter);
    assert_eq!(
        error.to_string(),
        "cannot create a render pipeline: for `vsMain` and `fsMain`, the vertex attribute `position` has no semantic, which slango with the feature `rhi` needs, because Direct3D 12 matches vertex attributes by semantic"
    );
}

#[test]
fn each_scalar_type_and_count_of_components_has_its_vertex_format() {
    use slang_rhi::VertexFormat as F;
    assert_eq!(vertex_format(Scalar::Float32, 1), Some(F::Float32));
    assert_eq!(vertex_format(Scalar::Int32, 3), Some(F::Sint32x3));
    assert_eq!(vertex_format(Scalar::Uint32, 4), Some(F::Uint32x4));
    assert_eq!(vertex_format(Scalar::Float32, 0), None);
    assert_eq!(vertex_format(Scalar::Float32, 5), None);
    assert_eq!(vertex_format(Scalar::Bool, 1), None);
}

fn merged(binding: Binding, visibility: Stages) -> PipelineBinding {
    PipelineBinding {
        binding,
        visibility,
    }
}

fn uniform(size: u64) -> Resource {
    Resource::UniformBuffer(UniformBlock {
        size,
        values: Vec::new(),
    })
}

fn assert_error(error: &Error, kind: ErrorKind, text: &str) {
    assert_eq!(error.kind(), kind);
    assert_eq!(error.to_string(), text);
}

#[test]
fn each_scope_is_the_position_of_its_entry_point_in_the_program_or_the_globals() {
    assert_eq!(scope(&key("a", None)), None);
    assert_eq!(scope(&key("a", Some(RenderStage::Vertex))), Some(0));
    assert_eq!(scope(&key("a", Some(RenderStage::Fragment))), Some(1));
}

const GLOBAL: Place = Place {
    in_entry_point: false,
    in_structure: false,
};

const OWN: Place = Place {
    in_entry_point: true,
    in_structure: false,
};

/// A binding at the set 0 and the binding 0, where DXIL reports every
/// binding.
fn dxil(path: &str, usage: Usage, resource: Resource) -> Binding {
    Binding {
        path: path.to_owned(),
        set: 0,
        index: 0,
        usage,
        resource,
    }
}

/// Merges the bindings by their paths and scopes, each at its place, and
/// returns the merged bindings with their keys.
fn merge_scoped(
    vertex: &[(Binding, Place)],
    fragment: &[(Binding, Place)],
) -> (Vec<PipelineBinding>, Vec<BindingKey>) {
    let keys = |placed: &[(Binding, Place)], stage| {
        placed
            .iter()
            .map(|(binding, place)| BindingKey::of(binding, *place, stage))
            .collect()
    };
    let keys = BindingKeys {
        vertex: keys(vertex, RenderStage::Vertex),
        fragment: keys(fragment, RenderStage::Fragment),
    };
    let bindings = |placed: &[(Binding, Place)]| -> Vec<Binding> {
        placed.iter().map(|(binding, _)| binding.clone()).collect()
    };
    merge_bindings(&bindings(vertex), &bindings(fragment), keys, NAMES).expect("the bindings agree")
}

fn key(path: &str, scope: Option<RenderStage>) -> BindingKey {
    BindingKey {
        slot: SlotKey { set: 0, index: 0 },
        scope,
        path: path.to_owned(),
    }
}

#[test]
fn a_texture_and_a_sampler_that_dxil_reports_at_one_binding_stay_two_bindings() {
    let texture = dxil("heights", Usage::Used, Resource::SampledTexture);
    let sampler = dxil("heightSampler", Usage::Used, Resource::Sampler);
    let vertex = [(texture.clone(), GLOBAL), (sampler.clone(), GLOBAL)];
    let fragment = [
        (
            dxil("heights", Usage::Used, Resource::SampledTexture),
            GLOBAL,
        ),
        (
            dxil("heightSampler", Usage::Unused, Resource::Sampler),
            GLOBAL,
        ),
    ];
    let (bindings, keys) = merge_scoped(&vertex, &fragment);
    assert_eq!(
        bindings,
        [
            merged(sampler, Stages::VERTEX),
            merged(texture, Stages::VERTEX | Stages::FRAGMENT),
        ]
    );
    assert_eq!(keys, [key("heightSampler", None), key("heights", None)]);
}

#[test]
fn a_global_and_an_entry_point_parameter_that_dxil_reports_at_one_binding_stay_two_bindings() {
    let global = dxil("colours", Usage::Used, Resource::SampledTexture);
    let own = dxil("colours", Usage::Used, Resource::SampledTexture);
    let fragment_own = dxil("colours", Usage::Used, Resource::SampledTexture);
    let (bindings, keys) = merge_scoped(
        &[(global.clone(), GLOBAL), (own.clone(), OWN)],
        &[(global.clone(), GLOBAL), (fragment_own.clone(), OWN)],
    );
    assert_eq!(
        bindings,
        [
            merged(global, Stages::VERTEX | Stages::FRAGMENT),
            merged(own, Stages::VERTEX),
            merged(fragment_own, Stages::FRAGMENT),
        ]
    );
    assert_eq!(
        keys,
        [
            key("colours", None),
            key("colours", Some(RenderStage::Vertex)),
            key("colours", Some(RenderStage::Fragment)),
        ]
    );
}

#[test]
fn a_global_that_both_stages_use_is_one_binding_visible_to_both() {
    let block = dxil("frame", Usage::Used, uniform(64));
    let (bindings, keys) = merge_scoped(&[(block.clone(), GLOBAL)], &[(block.clone(), GLOBAL)]);
    assert_eq!(bindings, [merged(block, Stages::VERTEX | Stages::FRAGMENT)]);
    assert_eq!(keys, [key("frame", None)]);
}

#[test]
fn bindings_that_merge_by_scope_keep_the_order_of_sets_and_bindings() {
    let first = Binding {
        set: 1,
        ..dxil("early", Usage::Used, Resource::Sampler)
    };
    let second = Binding {
        index: 3,
        ..dxil("late", Usage::Used, uniform(16))
    };
    let (bindings, _) = merge_scoped(&[(first.clone(), GLOBAL)], &[(second.clone(), GLOBAL)]);
    assert_eq!(
        bindings,
        [
            merged(second, Stages::FRAGMENT),
            merged(first, Stages::VERTEX)
        ]
    );
}

#[test]
fn two_layouts_of_one_global_path_are_an_unsupported_parameter() {
    let vertex = [dxil("frame", Usage::Used, uniform(16))];
    let fragment = [dxil("frame", Usage::Used, uniform(32))];
    let keys = BindingKeys {
        vertex: vec![BindingKey::of(&vertex[0], GLOBAL, RenderStage::Vertex)],
        fragment: vec![BindingKey::of(&fragment[0], GLOBAL, RenderStage::Fragment)],
    };
    assert_error(
        &merge_bindings(&vertex, &fragment, keys, NAMES).expect_err("the layouts differ"),
        ErrorKind::UnsupportedParameter,
        "cannot create a render pipeline: for `vsMain` and `fsMain`, Slang reports the parameters `frame` and `frame` among the global parameters with different layouts",
    );
}
