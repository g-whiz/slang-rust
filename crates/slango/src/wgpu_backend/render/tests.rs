use super::{
    VertexBuffer, blend_state, colour_target, cull_face, depth_stencil, primitive,
    render_pipeline_error_kind, vertex_buffer_layouts, vertex_buffers, vertex_format,
};
use wgpu::VertexFormat;

use crate::draw::element_type;
use crate::error::ErrorKind;
use crate::layout::{Attribute, Scalar, Usage};
use crate::marshal::VertexElement;
use crate::render::{Blend, CullMode, Names, RenderPipelineOptions, StepMode, VertexInput};
use crate::texture::TextureFormat;

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

const NAMES: Names<'static> = Names {
    vertex: "vsMain",
    fragment: "fsMain",
};

#[test]
fn used_attributes_become_vertex_buffers_in_the_order_of_their_slots() {
    let input = |attribute, step_mode, slot| VertexInput {
        attribute,
        step_mode,
        slot,
    };
    let inputs = [
        input(
            attribute("input.position", 0, Scalar::Float32, 3, Usage::Used),
            StepMode::Vertex,
            Some(0),
        ),
        input(
            attribute("input.unused", 1, Scalar::Float32, 4, Usage::Unused),
            StepMode::Instance,
            None,
        ),
        input(
            attribute("input.uv", 2, Scalar::Float32, 2, Usage::Used),
            StepMode::Vertex,
            Some(1),
        ),
        input(
            attribute("offset", 3, Scalar::Int32, 2, Usage::Used),
            StepMode::Instance,
            Some(2),
        ),
    ];

    let buffers = vertex_buffers(&inputs, NAMES).expect("every attribute has a format");
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
            buffer(VertexFormat::Float32x3, 0, wgpu::VertexStepMode::Vertex),
            buffer(VertexFormat::Float32x2, 2, wgpu::VertexStepMode::Vertex),
            buffer(VertexFormat::Sint32x2, 3, wgpu::VertexStepMode::Instance),
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
    let inputs = [VertexInput {
        attribute: attributes[0].clone(),
        step_mode: StepMode::Vertex,
        slot: None,
    }];
    assert_eq!(vertex_buffers(&inputs, NAMES).ok(), Some(Vec::new()));
    assert_eq!(vertex_buffers(&[], NAMES).ok(), Some(Vec::new()));
}

#[test]
fn a_used_attribute_without_a_vertex_format_is_an_unsupported_parameter_and_an_unused_one_is_ignored()
 {
    let input = |path, scalar, usage, slot| VertexInput {
        attribute: attribute(path, 0, scalar, 1, usage),
        step_mode: StepMode::Vertex,
        slot,
    };
    let unused = [input("flag", Scalar::Bool, Usage::Unused, None)];
    assert_eq!(vertex_buffers(&unused, NAMES).ok(), Some(Vec::new()));
    let used = [input("flag", Scalar::Bool, Usage::Used, Some(0))];
    let error = vertex_buffers(&used, NAMES).expect_err("no vertex format holds a bool");
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter);
    assert_eq!(
        error.to_string(),
        "cannot create a render pipeline: for `vsMain` and `fsMain`, Slang reports the vertex attribute `flag` with a type that no vertex format holds"
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

fn format_of<T: VertexElement>() -> Option<VertexFormat> {
    let (scalar, components) = element_type::<T>()?;
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
