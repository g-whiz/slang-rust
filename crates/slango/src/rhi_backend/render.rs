use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use slang_compiler::Component;
use slang_rhi::{
    BlendMode, ColourLoad, DepthLoad, RenderPassDesc, RenderPipelineDesc, ShaderProgram,
    VertexFormat, VertexStream,
};

use super::device::Device;
use super::error::{Call, from_rhi};
use super::kernel::{Failure, Scope, unaddressable, write};
use super::module::{Module, RenderEntryPoint};
use super::texture::to_rhi;
use crate::backend;
use crate::call::Subject;
use crate::draw::{BoundVertex, DrawPlan, Recording};
use crate::error::{Error, ErrorKind, Result};
use crate::layout::{Binding, Place, RenderStage, Scalar, Semantic, UsableRender};
use crate::render::{
    BindingKeys, Blend, CullMode, Names, PipelineSnapshot, SlotKey, StepMode, VertexInput,
    stage_word,
};

const CREATE: &str = "create a render pipeline";

/// The shader program and render pipeline of slang-rhi of a
/// [`crate::RenderPipeline`].
#[derive(Clone)]
pub(crate) struct RenderPipeline {
    objects: Rc<Objects>,
}

struct Objects {
    pipeline: slang_rhi::RenderPipeline,
    _program: ShaderProgram,
    /// The buffer of each vertex stream of the pipeline, in the order of the
    /// streams.
    streams: Vec<StreamBuffer>,
    /// The scope of each binding of the merged snapshot, in the order of the
    /// bindings.
    scopes: Vec<Scope>,
}

/// Which buffer feeds a vertex stream of a render pipeline in a draw.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StreamBuffer {
    /// The vertex buffer that the draw gives for the used attribute of this
    /// vertex buffer slot.
    Slot(u32),
    /// The buffer of zeros of the device, for an attribute that the vertex
    /// entry point declares and does not use. Its stream has the stride 0,
    /// so every vertex reads the one attribute at the start of the buffer,
    /// for any count of vertices or instances.
    Zeros,
}

/// The key by which the bindings of the two entry points of a render
/// pipeline merge: the parameter path and whether the binding sits among the
/// global parameters or among those of one entry point, which identify the
/// binding on every target, also on DXIL, which reports the set 0 and the
/// binding 0 for every binding. The shader objects of slang-rhi address each
/// parameter by its path in its scope.
///
/// The set and the binding come first, so that on SPIR-V the bindings keep
/// the order of the backend wgpu.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct BindingKey {
    slot: SlotKey,
    /// The stage of the entry point among whose parameters the binding sits,
    /// or `None` for a global parameter.
    scope: Option<RenderStage>,
    path: String,
}

impl BindingKey {
    /// Returns the key of `binding` of the entry point of `stage`, which
    /// sits at `place`.
    fn of(binding: &Binding, place: Place, stage: RenderStage) -> Self {
        Self {
            slot: SlotKey::of(binding),
            scope: place.in_entry_point.then_some(stage),
            path: binding.path.clone(),
        }
    }
}

impl fmt::Display for BindingKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.scope {
            None => f.write_str("among the global parameters"),
            Some(stage) => write!(
                f,
                "among the parameters of the {} entry point",
                stage_word(stage)
            ),
        }
    }
}

impl RenderPipeline {
    /// Returns the key of each binding of both entry points of `module`.
    pub(crate) fn binding_keys(
        module: &Module,
        vertex_record: &UsableRender,
        fragment_record: &UsableRender,
        names: Names<'_>,
    ) -> Result<BindingKeys<BindingKey>> {
        let vertex = entry_point(module, names, RenderStage::Vertex)?;
        let fragment = entry_point(module, names, RenderStage::Fragment)?;
        Ok(BindingKeys {
            vertex: scoped_keys(vertex_record, vertex),
            fragment: scoped_keys(fragment_record, fragment),
        })
    }

    /// Checks the semantics, the streams, and the parameters of both entry
    /// points of `module` for slang-rhi, then composes and links the module
    /// with both in the session of its load and creates the shader program
    /// and the render pipeline of slang-rhi.
    pub(crate) fn create(
        device: &Device,
        module: &Module,
        vertex_record: &UsableRender,
        fragment_record: &UsableRender,
        names: Names<'_>,
        snapshot: &PipelineSnapshot,
        keys: &[BindingKey],
    ) -> Result<Self> {
        let vertex = entry_point(module, names, RenderStage::Vertex)?;
        let fragment = entry_point(module, names, RenderStage::Fragment)?;
        let semantics = semantics(vertex_record, vertex, names)?;
        check_limits(device, &snapshot.inputs, names)?;
        check_addressable(
            [(vertex_record, vertex), (fragment_record, fragment)],
            names,
        )?;
        let scopes = keys.iter().map(scope).collect();
        let (streams, buffers) = streams(&snapshot.inputs, &semantics, names)?;
        let session = module.compile_session();
        let linked = session
            .session()
            .compose(&[
                Component::Module(module.slang_module()),
                Component::EntryPoint(&vertex.entry_point),
                Component::EntryPoint(&fragment.entry_point),
            ])
            .map_err(Error::compile)?
            .into_value()
            .link()
            .map_err(Error::compile)?
            .into_value();
        let failed = |error| {
            from_rhi(CREATE, Call::Other, error).with_detail(format!(
                "for `{}` and `{}`, slang-rhi cannot create the render pipeline",
                names.vertex, names.fragment
            ))
        };
        let program = session.create_shader_program(&linked).map_err(failed)?;
        let options = &snapshot.options;
        let mut desc = RenderPipelineDesc::new(to_rhi(options.colour_format))
            .with_blend(match options.blend {
                Blend::Replace => BlendMode::Replace,
                Blend::Alpha => BlendMode::Alpha,
            })
            .with_cull_mode(match options.cull_mode {
                CullMode::None => slang_rhi::CullMode::None,
                CullMode::Back => slang_rhi::CullMode::Back,
            })
            .with_vertex_streams(streams);
        if let Some(format) = options.depth_format {
            desc = desc.with_depth_format(to_rhi(format));
        }
        let pipeline = device
            .rhi_device()
            .create_render_pipeline(&program, &desc)
            .map_err(failed)?;
        Ok(Self {
            objects: Rc::new(Objects {
                pipeline,
                _program: program,
                streams: buffers,
                scopes,
            }),
        })
    }

    /// Records one render pass with one draw and submits it, and reports
    /// every error that slang-rhi raises as an error of `subject`.
    pub(crate) fn draw(
        &self,
        device: &Device,
        subject: Subject<'_>,
        recording: &Recording<'_, '_>,
    ) -> Result<()> {
        let rejected = |error| {
            from_rhi(subject.operation(), Call::Other, error).with_detail(format!(
                "slang-rhi rejected the {} of {subject}",
                subject.request()
            ))
        };
        let mismatch = |text: String| {
            Error::new(ErrorKind::Backend, subject.operation())
                .with_detail(format!("{text}, of {subject}"))
        };
        let Recording {
            label: _label,
            bindings,
            plan,
            arguments,
            vertices,
            target,
            clear,
            depth,
            clear_depth,
            indices,
            count,
            instances,
        } = recording;
        let Some(target) = target else {
            return Err(mismatch("the draw has no colour target".to_owned()));
        };
        let colour_load = clear.map_or(ColourLoad::Load, ColourLoad::Clear);
        let mut pass =
            RenderPassDesc::new().with_colour_target(target.handle.rhi_texture(), colour_load);
        if let Some(depth) = depth {
            let depth_load = clear_depth.map_or(DepthLoad::Load, DepthLoad::Clear);
            pass = pass.with_depth_target(depth.handle.rhi_texture(), depth_load);
        }
        let zeros = if self.objects.streams.contains(&StreamBuffer::Zeros) {
            Some(device.zeros().map_err(rejected)?)
        } else {
            None
        };
        let buffers = self
            .stream_buffers(plan, vertices, zeros.as_deref())
            .map_err(|stream| {
                mismatch(format!(
                    "the draw holds no buffer of slang-rhi for the vertex stream {stream}"
                ))
            })?;
        let indices = match indices {
            Some(indices) => Some(indices.rhi_buffer().ok_or_else(|| {
                mismatch("the index buffer holds no buffer of slang-rhi".to_owned())
            })?),
            None => None,
        };
        let mut encoder = device.queue().encoder().map_err(rejected)?;
        let unwritten = encoder
            .render_pass(&pass, |pass| {
                let mut root = pass.bind_pipeline(&self.objects.pipeline)?;
                let written = write(
                    &mut root,
                    bindings,
                    &self.objects.scopes,
                    &plan.plan,
                    arguments,
                    None,
                );
                match written {
                    Ok(()) => {}
                    Err(Failure::Rhi(error)) => return Err(error),
                    Err(failure) => return Ok(Some(failure)),
                }
                drop(root);
                if !buffers.is_empty() {
                    pass.set_vertex_buffers(&buffers)?;
                }
                match indices {
                    Some(indices) => {
                        pass.set_index_buffer(indices)?;
                        pass.draw_indexed(*count, *instances)?;
                    }
                    None => pass.draw(*count, *instances)?,
                }
                Ok(None)
            })
            .map_err(rejected)?;
        if let Some(failure) = unwritten {
            return Err(match failure {
                Failure::Missing(path) => {
                    mismatch(format!("slang-rhi finds no parameter at the path `{path}`"))
                }
                Failure::Mismatch(text) => mismatch(text),
                Failure::Rhi(error) => rejected(error),
            });
        }
        let commands = encoder.finish().map_err(rejected)?;
        device.queue().submit([commands]).map_err(rejected)
    }
}

impl RenderPipeline {
    /// Returns the buffer of each vertex stream of the pipeline for a draw
    /// with the vertex buffers `vertices`, which `plan` assigns to their
    /// slots, and the buffer of zeros `zeros`, or the position of the first
    /// stream without a buffer.
    fn stream_buffers<'b>(
        &self,
        plan: &DrawPlan,
        vertices: &'b [(&str, BoundVertex<&backend::Buffer>)],
        zeros: Option<&'b slang_rhi::Buffer>,
    ) -> std::result::Result<Vec<&'b slang_rhi::Buffer>, usize> {
        self.objects
            .streams
            .iter()
            .enumerate()
            .map(|(position, stream)| {
                let buffer = match stream {
                    StreamBuffer::Slot(slot) => plan
                        .vertex_buffers
                        .iter()
                        .find(|(given, _)| given == slot)
                        .and_then(|(_, index)| vertices.get(*index))
                        .and_then(|(_, bound)| bound.handle.rhi_buffer()),
                    StreamBuffer::Zeros => zeros,
                };
                buffer.ok_or(position)
            })
            .collect()
    }
}

/// Returns the usable entry point of `stage` that `names` names, which the
/// module keeps for every usable record that the checks of the snapshot
/// found.
fn entry_point<'m>(
    module: &'m Module,
    names: Names<'_>,
    stage: RenderStage,
) -> Result<&'m RenderEntryPoint> {
    let name = match stage {
        RenderStage::Vertex => names.vertex,
        RenderStage::Fragment => names.fragment,
    };
    module.render_entry_point(name, stage).ok_or_else(|| {
        names.error(
            ErrorKind::Backend,
            &format!(
                "the load kept no {} entry point named `{name}`",
                stage_word(stage)
            ),
        )
    })
}

/// Returns the semantic of each vertex attribute of the vertex entry point
/// by its path, and fails with [`ErrorKind::UnsupportedParameter`] on the
/// first attribute without one, because Direct3D 12 matches attributes by
/// semantic and DXIL needs one, on Vulkan too, so that one shader behaves
/// alike on both platforms of the backend.
fn semantics<'r>(
    record: &'r UsableRender,
    entry: &'r RenderEntryPoint,
    names: Names<'_>,
) -> Result<HashMap<&'r str, &'r Semantic>> {
    let mut found = HashMap::new();
    for (index, attribute) in record.layout.attributes.iter().enumerate() {
        let Some(Some(semantic)) = entry.semantics.get(index) else {
            return Err(no_semantic(names, &attribute.path));
        };
        found.insert(attribute.path.as_str(), semantic);
    }
    Ok(found)
}

fn no_semantic(names: Names<'_>, path: &str) -> Error {
    names.error(ErrorKind::UnsupportedParameter, &no_semantic_problem(path))
}

/// Returns the problem of the vertex attribute at `path`, which has no
/// semantic. A load records it as the rejection of the vertex entry point
/// before Slang generates code for that entry point, because the generation
/// of DXIL fails on such an attribute first.
pub(super) fn no_semantic_problem(path: &str) -> String {
    format!(
        "the vertex attribute `{path}` has no semantic, which slango with the feature `rhi` needs, because Direct3D 12 matches vertex attributes by semantic"
    )
}

/// Fails with [`ErrorKind::InvalidArgument`] when the vertex attributes, one
/// vertex stream each, are more than the device and slang-rhi allow, or
/// when the input elements, one for each location from 0 to the highest
/// location of an attribute, are more than the device allows.
fn check_limits(device: &Device, inputs: &[VertexInput], names: Names<'_>) -> Result<()> {
    let info = device.info();
    let max_streams = info.max_vertex_streams().min(slang_rhi::MAX_VERTEX_STREAMS);
    let streams = inputs.len();
    if u32::try_from(streams).map_or(true, |streams| streams > max_streams) {
        return Err(names.error(
            ErrorKind::InvalidArgument,
            &format!(
                "the vertex entry point has {streams} vertex attributes, each of which needs a vertex stream, more than the {max_streams} that the device and slang-rhi allow"
            ),
        ));
    }
    let max_elements = info.max_vertex_input_elements();
    if let Some(highest) = inputs.iter().map(|input| input.attribute.location).max()
        && highest >= max_elements
    {
        return Err(names.error(
            ErrorKind::InvalidArgument,
            &format!(
                "the vertex attributes up to the location {highest} need {} input elements, more than the {max_elements} that the device allows",
                u64::from(highest) + 1
            ),
        ));
    }
    Ok(())
}

/// Fails with [`ErrorKind::UnsupportedParameter`] on the first used binding
/// of either entry point that the shader objects of slang-rhi cannot
/// address by name.
fn check_addressable(
    entries: [(&UsableRender, &RenderEntryPoint); 2],
    names: Names<'_>,
) -> Result<()> {
    for (record, entry) in entries {
        if let Some((path, what)) = unaddressable(record.layout.bindings.iter().zip(&entry.places))
        {
            return Err(names.error(
                ErrorKind::UnsupportedParameter,
                &format!(
                    "the parameter `{path}` is {what}, which slango with the feature `rhi` does not support"
                ),
            ));
        }
    }
    Ok(())
}

/// Returns the key of each binding of `record`, the record of `entry`.
fn scoped_keys(record: &UsableRender, entry: &RenderEntryPoint) -> Vec<BindingKey> {
    record
        .layout
        .bindings
        .iter()
        .zip(&entry.places)
        .map(|(binding, place)| BindingKey::of(binding, *place, entry.stage))
        .collect()
}

/// Returns the scope of the binding of the key `key`: the position of its
/// entry point, 0 for the vertex and 1 for the fragment entry point as the
/// program composes them, or the global parameters.
fn scope(key: &BindingKey) -> Scope {
    key.scope.map(|stage| match stage {
        RenderStage::Vertex => 0,
        RenderStage::Fragment => 1,
    })
}

/// Returns the vertex streams of the attributes `inputs`, ordered by
/// location, and the buffer that feeds each stream in a draw.
///
/// The stream of a used attribute has the stride of its format and the step
/// of the attribute, and its buffer is the one that the draw gives for the
/// slot of the attribute. slang-rhi needs a stream for every attribute that
/// the vertex entry point declares, because DXIL keeps an attribute that the
/// shader does not read in its input signature, so the stream of an unused
/// attribute reads the buffer of zeros of the device with the stride 0.
fn streams(
    inputs: &[VertexInput],
    semantics: &HashMap<&str, &Semantic>,
    names: Names<'_>,
) -> Result<(Vec<VertexStream>, Vec<StreamBuffer>)> {
    let mut streams = Vec::with_capacity(inputs.len());
    let mut buffers = Vec::with_capacity(inputs.len());
    for input in inputs {
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
        let Some((semantic, index)) = semantics.get(attribute.path.as_str()) else {
            return Err(no_semantic(names, &attribute.path));
        };
        let stream =
            VertexStream::new(format, attribute.location).with_semantic(semantic.as_str(), *index);
        if let Some(slot) = input.slot {
            let step = match input.step_mode {
                StepMode::Vertex => slang_rhi::StepMode::Vertex,
                StepMode::Instance => slang_rhi::StepMode::Instance,
            };
            streams.push(stream.with_step(step));
            buffers.push(StreamBuffer::Slot(slot));
        } else {
            streams.push(stream.with_stride(0));
            buffers.push(StreamBuffer::Zeros);
        }
    }
    Ok((streams, buffers))
}

/// Returns the vertex format of an attribute of a scalar type with a number
/// of components, or `None` when no vertex format holds it.
fn vertex_format(scalar: Scalar, components: u32) -> Option<VertexFormat> {
    use VertexFormat as F;
    let formats = match scalar {
        Scalar::Float32 => [F::Float32, F::Float32x2, F::Float32x3, F::Float32x4],
        Scalar::Int32 => [F::Sint32, F::Sint32x2, F::Sint32x3, F::Sint32x4],
        Scalar::Uint32 => [F::Uint32, F::Uint32x2, F::Uint32x3, F::Uint32x4],
        Scalar::Bool => return None,
    };
    let index = usize::try_from(components.checked_sub(1)?).ok()?;
    formats.get(index).copied()
}

#[cfg(test)]
mod tests;
