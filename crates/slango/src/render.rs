use std::collections::BTreeMap;
use std::fmt;
use std::marker::PhantomData;
use std::ops::{BitOr, BitOrAssign};
use std::sync::Arc;

use crate::Device;
use crate::backend;
use crate::error::{Error, ErrorKind, Result, SourceText};
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
/// pipeline. A render pipeline keeps its [`Device`] alive and holds no object
/// of the Slang compiler.
///
/// A render pipeline is neither [`Send`] nor [`Sync`]: it and every object
/// created from it stay on the thread that created them, because the crate
/// keeps all the GPU objects of a device on one thread whatever graphics
/// library runs them. A program sends results to other threads as plain
/// data, such as the vector that [`Texture::read`](crate::Texture::read)
/// returns. This program does not compile, because a render pipeline is not
/// `Send`:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slango::RenderPipeline>();
/// ```
///
/// This program does not compile either, because a render pipeline is not
/// `Sync`:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slango::RenderPipeline>();
/// ```
///
/// # Examples
///
/// See the example of
/// [`Module::render_pipeline`](crate::Module::render_pipeline).
#[derive(Clone)]
pub struct RenderPipeline {
    pub(crate) snapshot: Arc<PipelineSnapshot>,
    pub(crate) device: Device,
    pub(crate) inner: backend::RenderPipeline,
    thread_bound: PhantomData<*const ()>,
}

impl RenderPipeline {
    /// Checks the entry points and the options, then creates the render
    /// pipeline on the device.
    pub(crate) fn create(
        device: &Device,
        module: &backend::Module,
        records: &[CompiledRender],
        vertex: &str,
        fragment: &str,
        options: &RenderPipelineOptions,
    ) -> Result<Self> {
        let names = Names { vertex, fragment };
        let (vertex_record, fragment_record) = select(records, names)?;
        let keys =
            backend::RenderPipeline::binding_keys(module, vertex_record, fragment_record, names)?;
        let (snapshot, keys) = snapshot(vertex_record, fragment_record, keys, names, options)?;
        let inner = backend::RenderPipeline::create(
            device.backend(),
            module,
            vertex_record,
            fragment_record,
            names,
            &snapshot,
            &keys,
        )?;
        Ok(Self {
            snapshot: Arc::new(snapshot),
            device: device.clone(),
            inner,
            thread_bound: PhantomData,
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
    /// The bindings of both entry points, ordered by the keys of the
    /// backend, which order them by set and binding.
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
    pub(crate) visibility: Stages,
}

/// The shader stages of a render pipeline that use a binding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Stages {
    pub(crate) vertex: bool,
    pub(crate) fragment: bool,
}

impl Stages {
    pub(crate) const NONE: Self = Self {
        vertex: false,
        fragment: false,
    };
    pub(crate) const VERTEX: Self = Self {
        vertex: true,
        fragment: false,
    };
    pub(crate) const FRAGMENT: Self = Self {
        vertex: false,
        fragment: true,
    };

    pub(crate) fn is_empty(self) -> bool {
        self == Self::NONE
    }
}

impl BitOr for Stages {
    type Output = Self;

    fn bitor(self, other: Self) -> Self {
        Self {
            vertex: self.vertex || other.vertex,
            fragment: self.fragment || other.fragment,
        }
    }
}

impl BitOrAssign for Stages {
    fn bitor_assign(&mut self, other: Self) {
        *self = *self | other;
    }
}

/// How often a vertex attribute advances in its vertex buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum StepMode {
    Vertex,
    Instance,
}

impl AsRef<Binding> for PipelineBinding {
    fn as_ref(&self) -> &Binding {
        &self.binding
    }
}

/// One vertex attribute of a render pipeline, with its step mode and its
/// vertex buffer slot, which only a used attribute has.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct VertexInput {
    pub(crate) attribute: Attribute,
    pub(crate) step_mode: StepMode,
    pub(crate) slot: Option<u32>,
}

/// The key of each binding of the vertex and of the fragment entry point, in
/// the order of the bindings of each, by which the bindings of the two
/// entry points merge into the bindings of a render pipeline.
pub(crate) struct BindingKeys<K> {
    pub(crate) vertex: Vec<K>,
    pub(crate) fragment: Vec<K>,
}

/// The key of a binding by its set and binding, which identifies the
/// binding on a target that places every binding in a descriptor set, such
/// as SPIR-V, and with which one binding of a set holds one parameter.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SlotKey {
    pub(crate) set: u32,
    pub(crate) index: u32,
}

impl SlotKey {
    pub(crate) fn of(binding: &Binding) -> Self {
        Self {
            set: binding.set,
            index: binding.index,
        }
    }
}

impl fmt::Display for SlotKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "at set {}, binding {}", self.set, self.index)
    }
}

/// The names of the two entry points of a render pipeline, which every error
/// of its creation names.
#[derive(Clone, Copy)]
pub(crate) struct Names<'a> {
    pub(crate) vertex: &'a str,
    pub(crate) fragment: &'a str,
}

impl Names<'_> {
    pub(crate) fn error(self, kind: ErrorKind, problem: &str) -> Error {
        Error::new(kind, CREATE).with_detail(format!(
            "for `{}` and `{}`, {problem}",
            self.vertex, self.fragment
        ))
    }
}

pub(crate) fn stage_word(stage: RenderStage) -> &'static str {
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
        record.usable.as_ref().map_err(|rejection| {
            let error = names.error(rejection.kind, &rejection.detail);
            match &rejection.source {
                Some(text) => error.with_source(SourceText(text.clone())),
                None => error,
            }
        })
    };
    Ok((usable(vertex)?, usable(fragment)?))
}

/// Checks the parameters of both entry points and the options, and returns
/// the snapshot of the pipeline with the key of each of its bindings, in the
/// order of its bindings, all before the backend creates anything.
fn snapshot<K: Ord + fmt::Display>(
    vertex: &UsableRender,
    fragment: &UsableRender,
    keys: BindingKeys<K>,
    names: Names<'_>,
    options: &RenderPipelineOptions,
) -> Result<(PipelineSnapshot, Vec<K>)> {
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
    let (bindings, keys) = merge_bindings(
        &vertex.layout.bindings,
        &fragment.layout.bindings,
        keys,
        names,
    )?;
    let snapshot = PipelineSnapshot {
        vertex: names.vertex.to_owned(),
        fragment: names.fragment.to_owned(),
        bindings,
        inputs,
        options: options.clone(),
    };
    Ok((snapshot, keys))
}

/// Merges the bindings of the two entry points into one binding for each
/// key, visible to the stages whose metadata reports it used, and returns
/// the merged bindings in the order of their keys, each with its key.
///
/// Both entry points come from one program, so a key that both report holds
/// one parameter with one layout. When it does not, Slang's reflection is
/// inconsistent, and the merge fails with
/// [`ErrorKind::UnsupportedParameter`], as a layout that cannot be used does.
pub(crate) fn merge_bindings<K: Ord + fmt::Display>(
    vertex: &[Binding],
    fragment: &[Binding],
    keys: BindingKeys<K>,
    names: Names<'_>,
) -> Result<(Vec<PipelineBinding>, Vec<K>)> {
    let mut merged: BTreeMap<K, PipelineBinding> = BTreeMap::new();
    for (bindings, keys, stage) in [
        (vertex, keys.vertex, Stages::VERTEX),
        (fragment, keys.fragment, Stages::FRAGMENT),
    ] {
        for (binding, key) in bindings.iter().zip(keys) {
            let visibility = if binding.usage == Usage::Used {
                stage
            } else {
                Stages::NONE
            };
            match merged.get_mut(&key) {
                Some(existing) => {
                    if existing.binding.path != binding.path
                        || existing.binding.resource != binding.resource
                    {
                        return Err(names.error(
                            ErrorKind::UnsupportedParameter,
                            &format!(
                                "Slang reports the parameters `{}` and `{}` {key} with different layouts",
                                existing.binding.path, binding.path
                            ),
                        ));
                    }
                    existing.visibility |= visibility;
                }
                None => {
                    merged.insert(
                        key,
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
        .into_iter()
        .map(|(key, mut merged)| {
            merged.binding.usage = if merged.visibility.is_empty() {
                Usage::Unused
            } else {
                Usage::Used
            };
            (merged, key)
        })
        .unzip())
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
            if !vertex_format_supported(attribute.scalar, attribute.components) {
                return Err(names.error(
                    ErrorKind::UnsupportedParameter,
                    &format!(
                        "Slang reports the vertex attribute `{}` with a type that no vertex format holds",
                        attribute.path
                    ),
                ));
            }
            let step_mode = if options.instance_attributes.contains(&attribute.path) {
                StepMode::Instance
            } else {
                StepMode::Vertex
            };
            let slot = (attribute.usage == Usage::Used).then(|| {
                let slot = next_slot;
                next_slot += 1;
                slot
            });
            Ok(VertexInput {
                attribute: attribute.clone(),
                step_mode,
                slot,
            })
        })
        .collect()
}

/// Returns whether a vertex format holds an attribute of a scalar type with
/// a number of components, which is true for 1 to 4 components of `float`,
/// `int`, and `uint`.
pub(crate) fn vertex_format_supported(scalar: Scalar, components: u32) -> bool {
    matches!(scalar, Scalar::Float32 | Scalar::Int32 | Scalar::Uint32)
        && (1..=4).contains(&components)
}

#[cfg(test)]
mod tests;
