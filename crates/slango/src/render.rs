use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

use crate::Device;
use crate::device::scoped;
use crate::error::{Error, ErrorKind, Result};
use crate::kernel::{layout_entries, pipeline_error_kind};
use crate::layout::{Attribute, Binding, CompiledRender, RenderStage, Scalar, UsableRender, Usage};
use crate::texture::TextureFormat;

const CREATE: &str = "create a render pipeline";

/// A render pipeline of a loaded [`Module`](crate::Module), built from one
/// of its vertex entry points and one of its fragment entry points.
///
/// A vertex entry point is a function of the module marked
/// `[shader("vertex")]`, which computes the position of each vertex, and a
/// fragment entry point is one marked `[shader("fragment")]`, which computes
/// the colour of each pixel that a triangle covers. Create a pipeline with
/// [`Module::render_pipeline`](crate::Module::render_pipeline), which
/// checks that the device accepts both entry points together, so every error
/// of the pipeline surfaces there. [`RenderPipeline::draw`] draws with it
/// into a [`Texture`](crate::Texture).
///
/// Cloning a render pipeline is cheap and returns another handle to the same
/// pipeline. A render pipeline keeps its [`Device`] alive, holds no object of
/// the Slang compiler, and is [`Send`] and [`Sync`], so threads can share it.
///
/// # Examples
///
/// See the example of
/// [`Module::render_pipeline`](crate::Module::render_pipeline).
#[derive(Clone)]
pub struct RenderPipeline {
    pub(crate) snapshot: Arc<PipelineSnapshot>,
    pub(crate) device: Device,
    pub(crate) bind_group_layouts: Arc<[::wgpu::BindGroupLayout]>,
    pub(crate) pipeline: ::wgpu::RenderPipeline,
}

impl RenderPipeline {
    /// Checks the entry points and the options, then creates both shader
    /// modules, the layouts, and the render pipeline on the device.
    pub(crate) fn create(
        device: &Device,
        records: &[CompiledRender],
        vertex: &str,
        fragment: &str,
        options: &RenderPipelineOptions,
    ) -> Result<Self> {
        let names = Names { vertex, fragment };
        let (vertex_record, fragment_record) = select(records, names)?;
        let snapshot = snapshot(vertex_record, fragment_record, names, options)?;
        let wgpu_device = device.wgpu_device();
        let vertex_module = shader_module(wgpu_device, vertex_record, names, RenderStage::Vertex)?;
        let fragment_module =
            shader_module(wgpu_device, fragment_record, names, RenderStage::Fragment)?;
        let label = Some(vertex);
        let ((bind_group_layouts, pipeline_layout), error) = scoped(wgpu_device, || {
            let used: Vec<(&Binding, ::wgpu::ShaderStages)> = snapshot
                .bindings
                .iter()
                .filter(|binding| !binding.visibility.is_empty())
                .map(|binding| (&binding.binding, binding.visibility))
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
        let buffers = vertex_buffers(&snapshot.inputs);
        let buffer_layouts = vertex_buffer_layouts(&buffers);
        let targets = [Some(colour_target(&snapshot.options))];
        let (pipeline, error) = scoped(wgpu_device, || {
            wgpu_device.create_render_pipeline(&::wgpu::RenderPipelineDescriptor {
                label,
                layout: Some(&pipeline_layout),
                vertex: ::wgpu::VertexState {
                    module: &vertex_module,
                    entry_point: Some(vertex),
                    compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                    buffers: &buffer_layouts,
                },
                primitive: primitive(&snapshot.options),
                depth_stencil: depth_stencil(snapshot.options.depth_format),
                multisample: ::wgpu::MultisampleState::default(),
                fragment: Some(::wgpu::FragmentState {
                    module: &fragment_module,
                    entry_point: Some(fragment),
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
            snapshot: Arc::new(snapshot),
            device: device.clone(),
            bind_group_layouts: bind_group_layouts.into(),
            pipeline,
        })
    }
}

impl fmt::Debug for RenderPipeline {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RenderPipeline")
            .field("vertex", &self.snapshot.vertex)
            .field("fragment", &self.snapshot.fragment)
            .field("options", &self.snapshot.options)
            .finish_non_exhaustive()
    }
}

/// The options with which
/// [`Module::render_pipeline`](crate::Module::render_pipeline) creates a
/// render pipeline.
///
/// Start from [`RenderPipelineOptions::default`], which draws into an
/// [`Rgba8Unorm`](TextureFormat::Rgba8Unorm) colour target without a depth
/// target, replaces the colour of each pixel, culls no triangle, and steps
/// every vertex attribute once per vertex, and change the fields that
/// matter. The struct is non-exhaustive, so a later version can add options,
/// and a program sets each field on a default value instead of writing a
/// struct literal.
///
/// Every pipeline draws a list of triangles, and a triangle whose vertices
/// appear counter-clockwise on the target faces the viewer.
///
/// # Examples
///
/// See the example of
/// [`Module::render_pipeline`](crate::Module::render_pipeline).
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub struct RenderPipelineOptions {
    /// The format of the colour target, which must be a colour format and
    /// not [`TextureFormat::Depth32Float`].
    pub colour_format: TextureFormat,
    /// The format of the depth target, or `None` for a pipeline without
    /// one. The only depth format is [`TextureFormat::Depth32Float`], with
    /// which the pipeline keeps the pixels of the nearest triangle: it draws
    /// a pixel only when its depth is less than the depth already in the
    /// target, and then writes the new depth.
    pub depth_format: Option<TextureFormat>,
    /// How the colour of the fragment entry point combines with the colour
    /// already in the target.
    pub blend: Blend,
    /// Which triangles the pipeline discards by the side they face.
    pub cull_mode: CullMode,
    /// The parameter paths of the vertex attributes that step once per
    /// instance, such as `input.offset` for the field `offset` of the
    /// parameter `input` of the vertex entry point. Every other attribute
    /// steps once per vertex.
    pub instance_attributes: Vec<String>,
}

impl Default for RenderPipelineOptions {
    fn default() -> Self {
        Self {
            colour_format: TextureFormat::Rgba8Unorm,
            depth_format: None,
            blend: Blend::default(),
            cull_mode: CullMode::default(),
            instance_attributes: Vec::new(),
        }
    }
}

/// How a render pipeline combines the colour of its fragment entry point
/// with the colour already in the target.
///
/// The enumeration is non-exhaustive, so a later version can add modes, and a
/// `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of
/// [`Module::render_pipeline`](crate::Module::render_pipeline).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Blend {
    /// The colour of the fragment entry point replaces the colour in the
    /// target.
    #[default]
    Replace,
    /// The colour of the fragment entry point is blended over the colour in
    /// the target by its alpha channel, so an alpha of 0.0 keeps the target
    /// and 1.0 replaces it.
    Alpha,
}

/// Which triangles a render pipeline discards by the side they face.
///
/// A triangle whose vertices appear counter-clockwise on the target faces
/// the viewer. The enumeration is non-exhaustive, so a later version can add
/// modes, and a `match` on it needs a wildcard arm.
///
/// # Examples
///
/// See the example of
/// [`Module::render_pipeline`](crate::Module::render_pipeline).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CullMode {
    /// The pipeline draws every triangle.
    #[default]
    None,
    /// The pipeline discards the triangles that face away from the viewer.
    Back,
}

/// The owned facts of a render pipeline, which its creation builds the
/// layouts from and which a draw checks its arguments against.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PipelineSnapshot {
    pub(crate) vertex: String,
    pub(crate) fragment: String,
    /// The bindings of both entry points, ordered by set and binding.
    pub(crate) bindings: Vec<PipelineBinding>,
    /// The vertex attributes of the vertex entry point, ordered by location.
    pub(crate) inputs: Vec<VertexInput>,
    pub(crate) options: RenderPipelineOptions,
}

/// One binding of a render pipeline with the stages that use it, which are
/// none when neither entry point uses it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct PipelineBinding {
    pub(crate) binding: Binding,
    pub(crate) visibility: ::wgpu::ShaderStages,
}

impl AsRef<Binding> for PipelineBinding {
    fn as_ref(&self) -> &Binding {
        &self.binding
    }
}

/// One vertex attribute of a render pipeline, with its vertex format, its
/// step mode, and its vertex buffer slot, which only a used attribute has.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct VertexInput {
    pub(crate) attribute: Attribute,
    pub(crate) format: ::wgpu::VertexFormat,
    pub(crate) step_mode: ::wgpu::VertexStepMode,
    pub(crate) slot: Option<u32>,
}

/// The vertex buffer of one used attribute, at the position of its slot.
#[derive(Debug, PartialEq)]
struct VertexBuffer {
    attribute: ::wgpu::VertexAttribute,
    step_mode: ::wgpu::VertexStepMode,
}

/// The names of the two entry points of a render pipeline, which every error
/// of its creation names.
#[derive(Clone, Copy)]
struct Names<'a> {
    vertex: &'a str,
    fragment: &'a str,
}

impl Names<'_> {
    fn error(self, kind: ErrorKind, problem: &str) -> Error {
        Error::new(kind, CREATE).with_detail(format!(
            "for `{}` and `{}`, {problem}",
            self.vertex, self.fragment
        ))
    }
}

fn stage_word(stage: RenderStage) -> &'static str {
    match stage {
        RenderStage::Vertex => "vertex",
        RenderStage::Fragment => "fragment",
    }
}

/// Returns the usable records of the vertex and the fragment entry point
/// that `names` names, and fails for a name that is no entry point of its
/// stage or a record that holds a rejection.
fn select<'a>(
    records: &'a [CompiledRender],
    names: Names<'_>,
) -> Result<(&'a UsableRender, &'a UsableRender)> {
    let find = |name: &str, stage: RenderStage| {
        records
            .iter()
            .find(|record| record.stage == stage && record.name == name)
            .ok_or_else(|| {
                names.error(
                    ErrorKind::InvalidArgument,
                    &format!(
                        "the module has no {} entry point named `{name}`",
                        stage_word(stage)
                    ),
                )
            })
    };
    let vertex = find(names.vertex, RenderStage::Vertex)?;
    let fragment = find(names.fragment, RenderStage::Fragment)?;
    let usable = |record: &'a CompiledRender| {
        record
            .usable
            .as_ref()
            .map_err(|rejection| names.error(rejection.kind, &rejection.detail))
    };
    Ok((usable(vertex)?, usable(fragment)?))
}

/// Checks the parameters of both entry points and the options, and returns
/// the snapshot of the pipeline, all before any call of wgpu.
fn snapshot(
    vertex: &UsableRender,
    fragment: &UsableRender,
    names: Names<'_>,
    options: &RenderPipelineOptions,
) -> Result<PipelineSnapshot> {
    for (record, stage, name) in [
        (vertex, RenderStage::Vertex, names.vertex),
        (fragment, RenderStage::Fragment, names.fragment),
    ] {
        if record.layout.immediates.is_some() {
            return Err(names.error(
                ErrorKind::UnsupportedParameter,
                &format!(
                    "the {} entry point `{name}` has `uniform` parameters of its own, which a render pipeline does not support, so declare them as global `uniform` values or in a constant buffer or parameter block",
                    stage_word(stage)
                ),
            ));
        }
    }
    if options.colour_format.is_depth() {
        return Err(names.error(
            ErrorKind::InvalidArgument,
            &format!(
                "`colour_format` is the depth format {:?} and not a colour format",
                options.colour_format
            ),
        ));
    }
    if let Some(depth_format) = options.depth_format
        && !depth_format.is_depth()
    {
        return Err(names.error(
            ErrorKind::InvalidArgument,
            &format!(
                "`depth_format` is the colour format {depth_format:?} and not the depth format Depth32Float"
            ),
        ));
    }
    let inputs = vertex_inputs(&vertex.layout.attributes, names, options)?;
    let bindings = merge_bindings(&vertex.layout.bindings, &fragment.layout.bindings, names)?;
    Ok(PipelineSnapshot {
        vertex: names.vertex.to_owned(),
        fragment: names.fragment.to_owned(),
        bindings,
        inputs,
        options: options.clone(),
    })
}

/// Merges the bindings of the two entry points into one binding for each
/// set and binding, visible to the stages whose metadata reports it used.
///
/// Both entry points come from one program, so a set and binding that both
/// report holds one parameter with one layout. When it does not, Slang's
/// reflection is inconsistent, and the merge fails with
/// [`ErrorKind::UnsupportedParameter`], as a layout that cannot be used does.
fn merge_bindings(
    vertex: &[Binding],
    fragment: &[Binding],
    names: Names<'_>,
) -> Result<Vec<PipelineBinding>> {
    let mut merged: BTreeMap<(u32, u32), PipelineBinding> = BTreeMap::new();
    for (bindings, stage) in [
        (vertex, ::wgpu::ShaderStages::VERTEX),
        (fragment, ::wgpu::ShaderStages::FRAGMENT),
    ] {
        for binding in bindings {
            let visibility = if binding.usage == Usage::Used {
                stage
            } else {
                ::wgpu::ShaderStages::NONE
            };
            match merged.get_mut(&(binding.set, binding.index)) {
                Some(existing) => {
                    if existing.binding.path != binding.path
                        || existing.binding.resource != binding.resource
                    {
                        return Err(names.error(
                            ErrorKind::UnsupportedParameter,
                            &format!(
                                "Slang reports the parameters `{}` and `{}` at set {}, binding {} with different layouts",
                                existing.binding.path, binding.path, binding.set, binding.index
                            ),
                        ));
                    }
                    existing.visibility |= visibility;
                }
                None => {
                    merged.insert(
                        (binding.set, binding.index),
                        PipelineBinding {
                            binding: binding.clone(),
                            visibility,
                        },
                    );
                }
            }
        }
    }
    Ok(merged
        .into_values()
        .map(|mut merged| {
            merged.binding.usage = if merged.visibility.is_empty() {
                Usage::Unused
            } else {
                Usage::Used
            };
            merged
        })
        .collect())
}

/// Returns the vertex attributes ordered by location, each with its format,
/// its step mode from `instance_attributes`, and, when the vertex entry
/// point uses it, the next vertex buffer slot.
///
/// A path in `instance_attributes` must name an attribute of the vertex
/// entry point, used or not.
fn vertex_inputs(
    attributes: &[Attribute],
    names: Names<'_>,
    options: &RenderPipelineOptions,
) -> Result<Vec<VertexInput>> {
    if let Some(path) = options
        .instance_attributes
        .iter()
        .find(|path| !attributes.iter().any(|attribute| &attribute.path == *path))
    {
        return Err(names.error(
            ErrorKind::InvalidArgument,
            &format!(
                "`instance_attributes` names `{path}`, which is no vertex attribute of the vertex entry point `{}`",
                names.vertex
            ),
        ));
    }
    let mut sorted: Vec<&Attribute> = attributes.iter().collect();
    sorted.sort_by_key(|attribute| attribute.location);
    let mut next_slot = 0;
    sorted
        .into_iter()
        .map(|attribute| {
            let format = vertex_format(attribute.scalar, attribute.components).ok_or_else(|| {
                names.error(
                    ErrorKind::UnsupportedParameter,
                    &format!(
                        "Slang reports the vertex attribute `{}` with a type that no vertex format holds",
                        attribute.path
                    ),
                )
            })?;
            let step_mode = if options.instance_attributes.contains(&attribute.path) {
                ::wgpu::VertexStepMode::Instance
            } else {
                ::wgpu::VertexStepMode::Vertex
            };
            let slot = (attribute.usage == Usage::Used).then(|| {
                let slot = next_slot;
                next_slot += 1;
                slot
            });
            Ok(VertexInput {
                attribute: attribute.clone(),
                format,
                step_mode,
                slot,
            })
        })
        .collect()
}

/// Returns the vertex format of an attribute of a scalar type with a number
/// of components, or `None` when no vertex format holds it.
pub(crate) fn vertex_format(scalar: Scalar, components: u32) -> Option<::wgpu::VertexFormat> {
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
fn vertex_buffers(inputs: &[VertexInput]) -> Vec<VertexBuffer> {
    inputs
        .iter()
        .filter(|input| input.slot.is_some())
        .map(|input| VertexBuffer {
            attribute: ::wgpu::VertexAttribute {
                format: input.format,
                offset: 0,
                shader_location: input.attribute.location,
            },
            step_mode: input.step_mode,
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
        format: options.colour_format.to_wgpu(),
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
        format: format.to_wgpu(),
        depth_write_enabled: Some(true),
        depth_compare: Some(::wgpu::CompareFunction::Less),
        stencil: ::wgpu::StencilState::default(),
        bias: ::wgpu::DepthBiasState::default(),
    })
}

#[cfg(test)]
mod tests;
