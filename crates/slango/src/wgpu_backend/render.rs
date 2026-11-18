use std::borrow::Cow;
use std::sync::Arc;

use super::device::{Device, scoped};
use super::kernel::{layout_entries, pipeline_error_kind};
use super::submit::{bind_groups, rejected};
use super::texture::to_wgpu;
use crate::call::Subject;
use crate::draw::Recording;
use crate::error::{ErrorKind, Result};
use crate::layout::{Binding, RenderStage, Scalar, UsableRender};
use crate::render::{
    Blend, CullMode, Names, PipelineSnapshot, RenderPipelineOptions, Stages, StepMode, VertexInput,
    stage_word,
};
use crate::texture::TextureFormat;

/// The wgpu bind group layouts and render pipeline of a
/// [`crate::RenderPipeline`].
#[derive(Clone)]
pub(crate) struct RenderPipeline {
    bind_group_layouts: Arc<[::wgpu::BindGroupLayout]>,
    pipeline: ::wgpu::RenderPipeline,
}

/// The vertex buffer of one used attribute, at the position of its slot.
#[derive(Debug, PartialEq)]
struct VertexBuffer {
    attribute: ::wgpu::VertexAttribute,
    step_mode: ::wgpu::VertexStepMode,
}

impl RenderPipeline {
    /// Creates both shader modules, the layouts, and the render pipeline on
    /// the device.
    pub(crate) fn create(
        device: &Device,
        vertex_record: &UsableRender,
        fragment_record: &UsableRender,
        names: Names<'_>,
        snapshot: &PipelineSnapshot,
    ) -> Result<Self> {
        let wgpu_device = device.wgpu_device();
        let vertex_module = shader_module(wgpu_device, vertex_record, names, RenderStage::Vertex)?;
        let fragment_module =
            shader_module(wgpu_device, fragment_record, names, RenderStage::Fragment)?;
        let label = Some(names.vertex);
        let ((bind_group_layouts, pipeline_layout), error) = scoped(wgpu_device, || {
            let used: Vec<(&Binding, ::wgpu::ShaderStages)> = snapshot
                .bindings
                .iter()
                .filter(|binding| !binding.visibility.is_empty())
                .map(|binding| (&binding.binding, shader_stages(binding.visibility)))
                .collect();
            let bind_group_layouts: Vec<::wgpu::BindGroupLayout> = layout_entries(&used)
                .iter()
                .map(|entries| {
                    wgpu_device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                        label,
                        entries,
                    })
                })
                .collect();
            let layout_refs: Vec<Option<&::wgpu::BindGroupLayout>> =
                bind_group_layouts.iter().map(Some).collect();
            let pipeline_layout =
                wgpu_device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
                    label,
                    bind_group_layouts: &layout_refs,
                    immediate_size: 0,
                });
            (bind_group_layouts, pipeline_layout)
        });
        if let Some(error) = error {
            return Err(names
                .error(
                    pipeline_error_kind(&error),
                    "the backend failed to create the layouts of the pipeline",
                )
                .with_source(error));
        }
        let buffers = vertex_buffers(&snapshot.inputs, names)?;
        let buffer_layouts = vertex_buffer_layouts(&buffers);
        let targets = [Some(colour_target(&snapshot.options))];
        let (pipeline, error) = scoped(wgpu_device, || {
            wgpu_device.create_render_pipeline(&::wgpu::RenderPipelineDescriptor {
                label,
                layout: Some(&pipeline_layout),
                vertex: ::wgpu::VertexState {
                    module: &vertex_module,
                    entry_point: Some(names.vertex),
                    compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                    buffers: &buffer_layouts,
                },
                primitive: primitive(&snapshot.options),
                depth_stencil: depth_stencil(snapshot.options.depth_format),
                multisample: ::wgpu::MultisampleState::default(),
                fragment: Some(::wgpu::FragmentState {
                    module: &fragment_module,
                    entry_point: Some(names.fragment),
                    compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                    targets: &targets,
                }),
                multiview_mask: None,
                cache: None,
            })
        });
        if let Some(error) = error {
            return Err(names
                .error(
                    render_pipeline_error_kind(&error),
                    "the backend rejected the render pipeline",
                )
                .with_source(error));
        }
        Ok(Self {
            bind_group_layouts: bind_group_layouts.into(),
            pipeline,
        })
    }

    /// Records one draw and submits it, and reports every error that wgpu
    /// raises as an error of `subject`.
    pub(crate) fn draw(
        &self,
        device: &Device,
        subject: Subject<'_>,
        recording: &Recording<'_, '_>,
    ) -> Result<()> {
        let ((), error) = scoped(device.wgpu_device(), || self.record(device, recording));
        match error {
            Some(error) => Err(rejected(subject, error)),
            None => Ok(()),
        }
    }

    fn record(&self, device: &Device, recording: &Recording<'_, '_>) {
        let wgpu_device = device.wgpu_device();
        let label = Some(recording.label);
        let bind_groups = bind_groups(
            wgpu_device,
            recording.label,
            &recording.plan.plan,
            &self.bind_group_layouts,
            recording.arguments,
        );
        let colour = [recording
            .target
            .map(|target| ::wgpu::RenderPassColorAttachment {
                view: target.handle.wgpu_view(),
                depth_slice: None,
                resolve_target: None,
                ops: ::wgpu::Operations {
                    load: recording
                        .clear
                        .map_or(::wgpu::LoadOp::Load, |[r, g, b, a]| {
                            ::wgpu::LoadOp::Clear(::wgpu::Color {
                                r: f64::from(r),
                                g: f64::from(g),
                                b: f64::from(b),
                                a: f64::from(a),
                            })
                        }),
                    store: ::wgpu::StoreOp::Store,
                },
            })];
        let depth = recording
            .depth
            .map(|depth| ::wgpu::RenderPassDepthStencilAttachment {
                view: depth.handle.wgpu_view(),
                depth_ops: Some(::wgpu::Operations {
                    load: recording
                        .clear_depth
                        .map_or(::wgpu::LoadOp::Load, ::wgpu::LoadOp::Clear),
                    store: ::wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            });
        let mut encoder =
            wgpu_device.create_command_encoder(&::wgpu::CommandEncoderDescriptor { label });
        {
            let mut pass = encoder.begin_render_pass(&::wgpu::RenderPassDescriptor {
                label,
                color_attachments: &colour,
                depth_stencil_attachment: depth,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.pipeline);
            for (set, group) in (0_u32..).zip(&bind_groups) {
                pass.set_bind_group(set, group, &[]);
            }
            for &(slot, index) in &recording.plan.vertex_buffers {
                pass.set_vertex_buffer(
                    slot,
                    recording.vertices[index].1.handle.wgpu_buffer().slice(..),
                );
            }
            match recording.indices {
                Some(indices) => {
                    pass.set_index_buffer(
                        indices.wgpu_buffer().slice(..),
                        ::wgpu::IndexFormat::Uint32,
                    );
                    pass.draw_indexed(0..recording.count, 0, 0..recording.instances);
                }
                None => pass.draw(0..recording.count, 0..recording.instances),
            }
        }
        device.wgpu_queue().submit([encoder.finish()]);
    }
}

fn shader_stages(stages: Stages) -> ::wgpu::ShaderStages {
    let mut out = ::wgpu::ShaderStages::NONE;
    if stages.vertex {
        out |= ::wgpu::ShaderStages::VERTEX;
    }
    if stages.fragment {
        out |= ::wgpu::ShaderStages::FRAGMENT;
    }
    out
}

fn step_mode(mode: StepMode) -> ::wgpu::VertexStepMode {
    match mode {
        StepMode::Vertex => ::wgpu::VertexStepMode::Vertex,
        StepMode::Instance => ::wgpu::VertexStepMode::Instance,
    }
}

/// Returns the vertex format of an attribute of a scalar type with a number
/// of components, or `None` when no vertex format holds it.
fn vertex_format(scalar: Scalar, components: u32) -> Option<::wgpu::VertexFormat> {
    use ::wgpu::VertexFormat as F;
    let formats = match scalar {
        Scalar::Float32 => [F::Float32, F::Float32x2, F::Float32x3, F::Float32x4],
        Scalar::Int32 => [F::Sint32, F::Sint32x2, F::Sint32x3, F::Sint32x4],
        Scalar::Uint32 => [F::Uint32, F::Uint32x2, F::Uint32x3, F::Uint32x4],
        Scalar::Bool => return None,
    };
    let index = usize::try_from(components.checked_sub(1)?).ok()?;
    formats.get(index).copied()
}

/// Returns one vertex buffer for each used attribute, in the order of their
/// slots, with the attribute at offset 0.
///
/// It fails with [`ErrorKind::UnsupportedParameter`] on the first used
/// attribute whose type no vertex format holds, because the slots count
/// every used attribute and a dropped one would shift the later slots.
fn vertex_buffers(inputs: &[VertexInput], names: Names<'_>) -> Result<Vec<VertexBuffer>> {
    inputs
        .iter()
        .filter(|input| input.slot.is_some())
        .map(|input| {
            let attribute = &input.attribute;
            let format = vertex_format(attribute.scalar, attribute.components).ok_or_else(|| {
                names.error(
                    ErrorKind::UnsupportedParameter,
                    &format!(
                        "Slang reports the vertex attribute `{}` with a type that no vertex format holds",
                        attribute.path
                    ),
                )
            })?;
            Ok(VertexBuffer {
                attribute: ::wgpu::VertexAttribute {
                    format,
                    offset: 0,
                    shader_location: attribute.location,
                },
                step_mode: step_mode(input.step_mode),
            })
        })
        .collect()
}

/// Returns the wgpu layouts of the vertex buffers, one for each slot from 0,
/// each with a stride equal to the size of its attribute's format.
fn vertex_buffer_layouts(buffers: &[VertexBuffer]) -> Vec<Option<::wgpu::VertexBufferLayout<'_>>> {
    buffers
        .iter()
        .map(|buffer| {
            Some(::wgpu::VertexBufferLayout {
                array_stride: buffer.attribute.format.size(),
                step_mode: buffer.step_mode,
                attributes: std::slice::from_ref(&buffer.attribute),
            })
        })
        .collect()
}

/// Creates the shader module of one entry point inside an error scope, and
/// fails with [`ErrorKind::UnsupportedShader`] when the backend rejects it.
fn shader_module(
    device: &::wgpu::Device,
    record: &UsableRender,
    names: Names<'_>,
    stage: RenderStage,
) -> Result<::wgpu::ShaderModule> {
    let name = match stage {
        RenderStage::Vertex => names.vertex,
        RenderStage::Fragment => names.fragment,
    };
    let (module, error) = scoped(device, || {
        device.create_shader_module(::wgpu::ShaderModuleDescriptor {
            label: Some(name),
            source: ::wgpu::ShaderSource::SpirV(Cow::Borrowed(&record.spirv)),
        })
    });
    match error {
        Some(error) => Err(names
            .error(
                ErrorKind::UnsupportedShader,
                &format!(
                    "the backend rejected the SPIR-V of the {} entry point `{name}`",
                    stage_word(stage)
                ),
            )
            .with_source(error)),
        None => Ok(module),
    }
}

/// Returns the kind of the error when wgpu raises `error` while it creates
/// the render pipeline itself: a validation error reports a mismatch between
/// the two stages, such as of their varyings, which only a change of the
/// shader fixes.
fn render_pipeline_error_kind(error: &::wgpu::Error) -> ErrorKind {
    match error {
        ::wgpu::Error::Internal { .. } | ::wgpu::Error::Validation { .. } => {
            ErrorKind::UnsupportedShader
        }
        ::wgpu::Error::OutOfMemory { .. } => ErrorKind::Backend,
    }
}

fn colour_target(options: &RenderPipelineOptions) -> ::wgpu::ColorTargetState {
    ::wgpu::ColorTargetState {
        format: to_wgpu(options.colour_format),
        blend: blend_state(options.blend),
        write_mask: ::wgpu::ColorWrites::ALL,
    }
}

fn blend_state(blend: Blend) -> Option<::wgpu::BlendState> {
    match blend {
        Blend::Replace => None,
        Blend::Alpha => Some(::wgpu::BlendState::ALPHA_BLENDING),
    }
}

fn primitive(options: &RenderPipelineOptions) -> ::wgpu::PrimitiveState {
    ::wgpu::PrimitiveState {
        topology: ::wgpu::PrimitiveTopology::TriangleList,
        front_face: ::wgpu::FrontFace::Ccw,
        cull_mode: cull_face(options.cull_mode),
        ..::wgpu::PrimitiveState::default()
    }
}

fn cull_face(cull_mode: CullMode) -> Option<::wgpu::Face> {
    match cull_mode {
        CullMode::None => None,
        CullMode::Back => Some(::wgpu::Face::Back),
    }
}

fn depth_stencil(depth_format: Option<TextureFormat>) -> Option<::wgpu::DepthStencilState> {
    depth_format.map(|format| ::wgpu::DepthStencilState {
        format: to_wgpu(format),
        depth_write_enabled: Some(true),
        depth_compare: Some(::wgpu::CompareFunction::Less),
        stencil: ::wgpu::StencilState::default(),
        bias: ::wgpu::DepthBiasState::default(),
    })
}

#[cfg(test)]
mod tests;
