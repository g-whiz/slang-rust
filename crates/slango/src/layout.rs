use std::error::Error as StdError;
use std::fmt;
use std::path::Path;

use slang_compiler::reflection::{EntryPointLayout, ProgramLayout, Type};
use slang_compiler::reflection::{
    LayoutSize, LayoutUnits, ResourceAccess, ResourceBaseShape, ScalarType, TypeKind, TypeLayout,
    VariableLayout,
};
use slang_compiler::{
    BindingIndex, BindingSpace, CompilerOptions, Component, EntryPoint, GlobalSession,
    LinkedProgram, Metadata, ParameterCategory, Session, Stage, Target, TargetFormat, TargetIndex,
    Toggle,
};

use crate::error::{Error, ErrorKind, Result};

const LOAD: &str = "load a Slang module";

/// The layout snapshot of one kernel, the owned facts of Section 7.1 of
/// `docs/design/slango.md` from which the backend builds its layouts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KernelLayout {
    pub(crate) name: String,
    pub(crate) thread_group_size: [u32; 3],
    /// The bindings of the kernel, ordered by set and binding.
    pub(crate) bindings: Vec<Binding>,
    /// The `uniform` parameters of the entry point, which the dispatch
    /// passes as immediates, when it has any.
    pub(crate) immediates: Option<UniformBlock>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Binding {
    /// The parameter path, which is empty for the constant buffer that holds
    /// the global plain values.
    pub(crate) path: String,
    pub(crate) set: u32,
    pub(crate) index: u32,
    pub(crate) usage: Usage,
    pub(crate) resource: Resource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Usage {
    Used,
    Unused,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Resource {
    StorageBuffer {
        access: Access,
        stride: u64,
    },
    UniformBuffer(UniformBlock),
    /// A `Texture2D` whose result type has the scalar type `float`.
    SampledTexture,
    /// A `SamplerState`.
    Sampler,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Access {
    ReadOnly,
    ReadWrite,
}

/// A block of plain values, a uniform buffer or the immediates, with its
/// size in bytes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UniformBlock {
    pub(crate) size: u64,
    pub(crate) values: Vec<PlainValue>,
}

/// A plain value at its byte offset in its block.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PlainValue {
    pub(crate) path: String,
    pub(crate) offset: u64,
    pub(crate) size: u64,
    pub(crate) scalar: Scalar,
    pub(crate) shape: Shape,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Scalar {
    Bool,
    Int32,
    Uint32,
    Float32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Shape {
    Scalar,
    Vector {
        length: u32,
    },
    Matrix {
        rows: u32,
        columns: u32,
    },
    Array {
        length: u64,
        stride: u64,
        element: Box<Shape>,
    },
}

/// The stage of a vertex or fragment entry point.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RenderStage {
    Vertex,
    Fragment,
}

/// The layout snapshot of one vertex or fragment entry point, the owned facts
/// of Section 7.4 of `docs/design/slango.md` from which a render pipeline
/// builds its layouts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenderLayout {
    /// The bindings of the entry point, ordered by set and binding, each
    /// with the usage of this entry point.
    pub(crate) bindings: Vec<Binding>,
    /// The `uniform` parameters of the entry point, which a render pipeline
    /// rejects (SLO-23), when it has any.
    pub(crate) immediates: Option<UniformBlock>,
    /// The vertex attributes, ordered as the walk visits them, which is
    /// empty for a fragment entry point.
    pub(crate) attributes: Vec<Attribute>,
}

/// One vertex attribute of a vertex entry point (SLO-21).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Attribute {
    pub(crate) path: String,
    pub(crate) location: u32,
    pub(crate) scalar: Scalar,
    /// The number of components, 1 for a scalar.
    pub(crate) components: u32,
    pub(crate) usage: Usage,
}

/// The offsets that one variable adds along the path of a walk, in the units
/// of each parameter category.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Offsets {
    pub(crate) set: u64,
    pub(crate) binding: u64,
    pub(crate) byte: u64,
    pub(crate) register_space: u64,
    pub(crate) location: u64,
}

/// The running sums of a walk at one variable (Sections 3.3 and 7.4 of
/// `docs/design/slango.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Position {
    pub(crate) set: u64,
    pub(crate) binding: u64,
    pub(crate) byte: u64,
    pub(crate) register_space: u64,
    /// The location of a varying input.
    pub(crate) location: u64,
}

impl Position {
    pub(crate) fn descend(self, offsets: Offsets) -> Self {
        Self {
            set: self.set + offsets.set,
            binding: self.binding + offsets.binding,
            byte: self.byte + offsets.byte,
            register_space: self.register_space + offsets.register_space,
            location: self.location + offsets.location,
        }
    }

    /// The position inside a parameter block, whose set is the running sum
    /// of the register-space offsets and whose bindings restart at 0.
    pub(crate) fn parameter_block(self) -> Self {
        Self {
            set: self.register_space,
            binding: 0,
            ..self
        }
    }

    /// The position of the element of a constant buffer or parameter block,
    /// whose byte offsets restart at 0.
    pub(crate) fn block_element(self) -> Self {
        Self { byte: 0, ..self }
    }
}

/// One compute entry point as one compilation yields it: its layout snapshot
/// and its SPIR-V, whose entry point keeps the Slang name.
#[derive(Debug)]
pub(crate) struct CompiledKernel {
    pub(crate) layout: KernelLayout,
    pub(crate) spirv: Vec<u32>,
}

/// One vertex or fragment entry point as one compilation yields it: its
/// layout snapshot and its SPIR-V, or the rejection that makes it unusable.
#[derive(Debug)]
pub(crate) struct CompiledRender {
    pub(crate) name: String,
    pub(crate) stage: RenderStage,
    pub(crate) usable: std::result::Result<UsableRender, Rejection>,
}

/// The layout snapshot of a vertex or fragment entry point and its SPIR-V,
/// whose entry point keeps the Slang name.
#[derive(Debug)]
pub(crate) struct UsableRender {
    pub(crate) layout: RenderLayout,
    pub(crate) spirv: Vec<u32>,
}

/// Why a vertex or fragment entry point cannot be used: the kind of the
/// failure, [`ErrorKind::UnsupportedParameter`] when the walk refused one of
/// its parameters and [`ErrorKind::Compile`] when Slang failed on it, and the
/// text that describes it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Rejection {
    pub(crate) kind: ErrorKind,
    pub(crate) detail: String,
}

/// What one compilation of a module yields for its compute, vertex, and
/// fragment entry points, in the order of the module.
#[derive(Debug)]
pub(crate) struct Compiled {
    pub(crate) kernels: Vec<CompiledKernel>,
    pub(crate) render: Vec<CompiledRender>,
}

/// The Slang source of one module, with the path that diagnostics name and
/// the directory in which an `import` searches for sibling modules.
pub(crate) struct Source<'a> {
    pub(crate) name: &'a str,
    pub(crate) path: &'a str,
    pub(crate) text: &'a str,
    pub(crate) search_path: Option<&'a Path>,
}

/// The entry point that a walk describes, which the messages of its
/// rejections name.
#[derive(Clone, Copy)]
enum Role {
    Kernel,
    Vertex,
    Fragment,
}

impl Role {
    fn noun(self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::Vertex => "vertex entry point",
            Self::Fragment => "fragment entry point",
        }
    }
}

/// Compiles Slang source for SPIR-V and returns the layout snapshot and the
/// SPIR-V of each compute, vertex, and fragment entry point, releasing every
/// Slang object before it returns.
///
/// A failure that belongs to one kernel fails the compilation. A failure that
/// belongs to one vertex or fragment entry point, in its metadata, its walk,
/// or its SPIR-V, becomes the rejection of its record (SLO-20).
pub(crate) fn compile(source: &Source<'_>) -> Result<Compiled> {
    let global = GlobalSession::new().map_err(Error::compile)?;
    let mut options = CompilerOptions::default();
    options.vulkan_use_entry_point_name(Toggle::On);
    let mut builder = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv))
        .options(options);
    if let Some(search_path) = source.search_path {
        builder = builder.search_path(search_path);
    }
    let session = builder.build().map_err(Error::compile)?;
    let target = session.target_index(0).map_err(Error::compile)?;
    let module = session
        .load_module_from_source(source.name, source.path, source.text)
        .map_err(Error::compile)?
        .into_value();
    let entry_points = module
        .entry_points()
        .map_err(Error::compile)?
        .collect::<slang_compiler::Result<Vec<EntryPoint>>>()
        .map_err(Error::compile)?;
    let mut components = vec![Component::Module(&module)];
    components.extend(entry_points.iter().map(Component::EntryPoint));
    let program = session
        .compose(&components)
        .map_err(Error::compile)?
        .into_value()
        .link()
        .map_err(Error::compile)?
        .into_value();
    let layout = program.layout(target).map_err(Error::compile)?.into_value();
    let mut compiled = Compiled {
        kernels: Vec::new(),
        render: Vec::new(),
    };
    for (index, entry_point) in layout.entry_point_layouts().enumerate() {
        let role = match entry_point.stage().map_err(Error::compile)? {
            Some(Stage::Compute) => Role::Kernel,
            Some(Stage::Vertex) => Role::Vertex,
            Some(Stage::Fragment) => Role::Fragment,
            _ => continue,
        };
        let name = match entry_point.name().map_err(Error::compile) {
            Ok(Some(name)) => name.to_owned(),
            Ok(None) | Err(_) if !matches!(role, Role::Kernel) => continue,
            Ok(None) => {
                return Err(Error::new(ErrorKind::Compile, LOAD)
                    .with_detail("Slang reports no name for an entry point".to_owned()));
            }
            Err(error) => return Err(error),
        };
        let metadata = program
            .entry_point_metadata(index, target)
            .map_err(Error::compile)
            .map(slang_compiler::Compiled::into_value);
        match role {
            Role::Kernel => {
                let metadata = metadata?;
                let walk = walk(layout, entry_point, &metadata, &name, role)?;
                let [x, y, z] = entry_point.compute_thread_group_size();
                let thread_group_size = [
                    walk.narrow(x, "", "a thread group size")?,
                    walk.narrow(y, "", "a thread group size")?,
                    walk.narrow(z, "", "a thread group size")?,
                ];
                let spirv = spirv(&program, index, target, &name)?;
                compiled.kernels.push(CompiledKernel {
                    layout: KernelLayout {
                        thread_group_size,
                        bindings: walk.bindings,
                        immediates: walk.immediates,
                        name,
                    },
                    spirv,
                });
            }
            Role::Vertex | Role::Fragment => {
                let render_layout = metadata.and_then(|metadata| {
                    walk(layout, entry_point, &metadata, &name, role).map(|walk| RenderLayout {
                        bindings: walk.bindings,
                        immediates: walk.immediates,
                        attributes: walk.attributes,
                    })
                });
                let spirv = spirv(&program, index, target, &name);
                compiled
                    .render
                    .push(render_record(name, role, render_layout, spirv));
            }
        }
    }
    Ok(compiled)
}

/// Returns the record of a vertex or fragment entry point from the outcome of
/// its walk and of the generation of its SPIR-V, with the first failure as
/// its rejection.
fn render_record(
    name: String,
    role: Role,
    layout: Result<RenderLayout>,
    spirv: Result<Vec<u32>>,
) -> CompiledRender {
    let stage = if matches!(role, Role::Vertex) {
        RenderStage::Vertex
    } else {
        RenderStage::Fragment
    };
    let usable = layout
        .and_then(|layout| spirv.map(|spirv| UsableRender { layout, spirv }))
        .map_err(|error| rejection(&error, role, &name));
    CompiledRender {
        name,
        stage,
        usable,
    }
}

/// Returns the rejection of a vertex or fragment entry point from the error
/// that made it unusable, with the detail of the error, or the text of its
/// source when the error has no detail.
fn rejection(error: &Error, role: Role, name: &str) -> Rejection {
    let detail = match (error.detail(), StdError::source(error)) {
        (Some(detail), _) => detail.to_owned(),
        (None, Some(source)) => format!("Slang failed on the {} `{name}`: {source}", role.noun()),
        (None, None) => format!("{error}, for the {} `{name}`", role.noun()),
    };
    Rejection {
        kind: error.kind(),
        detail,
    }
}

/// Walks the parameters of one entry point, and its vertex attributes when
/// it is a vertex entry point, and returns the walk with its bindings sorted
/// by set and binding.
fn walk<'a>(
    layout: ProgramLayout<'_>,
    entry_point: EntryPointLayout<'_>,
    metadata: &'a Metadata,
    name: &'a str,
    role: Role,
) -> Result<Walk<'a>> {
    let mut walk = Walk {
        entry_point: name,
        role,
        metadata,
        bindings: Vec::new(),
        immediates: None,
        attributes: Vec::new(),
    };
    if let Some(globals) = layout.global_params_var_layout() {
        walk.variable(globals, "", Position::default(), None)?;
    }
    if let Some(parameters) = entry_point.var_layout() {
        walk.variable(parameters, "", Position::default(), None)?;
    }
    if matches!(role, Role::Vertex) {
        for parameter in entry_point.parameter_layouts() {
            let path = name_of(parameter)?;
            walk.attribute(parameter, &path, Position::default())?;
        }
    }
    walk.bindings
        .sort_by_key(|binding| (binding.set, binding.index));
    Ok(walk)
}

/// Returns the SPIR-V words of the entry point at `index`.
fn spirv(
    program: &LinkedProgram,
    index: usize,
    target: TargetIndex,
    name: &str,
) -> Result<Vec<u32>> {
    let code = program
        .entry_point_code(index, target)
        .map_err(Error::compile)?
        .into_value();
    Ok(code
        .as_spirv_words()
        .ok_or_else(|| {
            Error::new(ErrorKind::Compile, LOAD).with_detail(format!(
                "Slang generated code for the entry point `{name}` that is not a sequence of SPIR-V words"
            ))
        })?
        .to_vec())
}

/// Returns the scalar type and the component count of a vertex attribute
/// whose type has the scalar type `scalar` and `components` components, or
/// `None` when no vertex format holds it.
pub(crate) fn attribute_type(scalar: ScalarType, components: u64) -> Option<(Scalar, u32)> {
    let scalar = match scalar {
        ScalarType::Int32 => Scalar::Int32,
        ScalarType::Uint32 => Scalar::Uint32,
        ScalarType::Float32 => Scalar::Float32,
        _ => return None,
    };
    let components = u32::try_from(components)
        .ok()
        .filter(|components| (1..=4).contains(components))?;
    Some((scalar, components))
}

struct Walk<'a> {
    entry_point: &'a str,
    role: Role,
    metadata: &'a Metadata,
    bindings: Vec<Binding>,
    immediates: Option<UniformBlock>,
    attributes: Vec<Attribute>,
}

impl Walk<'_> {
    fn variable(
        &mut self,
        variable: VariableLayout<'_>,
        path: &str,
        base: Position,
        values: Option<&mut Vec<PlainValue>>,
    ) -> Result<()> {
        let Some(layout) = variable.type_layout() else {
            return Err(self.unsupported(path, "Slang reports no layout for it"));
        };
        let here = base.descend(offsets(variable));
        match layout.kind().map_err(Error::compile)? {
            TypeKind::Struct => {
                let mut values = values;
                for field in layout.field_layouts() {
                    let field_path = join(path, &name_of(field)?);
                    self.variable(field, &field_path, here, values.as_deref_mut())?;
                }
                Ok(())
            }
            kind @ (TypeKind::ConstantBuffer | TypeKind::ParameterBlock) => {
                self.block(layout, kind, path, here)
            }
            TypeKind::Resource => self.resource(layout, path, here),
            TypeKind::SamplerState => self.sampler(layout, path, here),
            TypeKind::Scalar | TypeKind::Vector | TypeKind::Matrix | TypeKind::Array => {
                let value = self.plain_value(layout, path, here)?;
                if value.size == 0 {
                    return Ok(());
                }
                let Some(values) = values else {
                    return Err(self.unsupported(path, "a plain value outside a constant buffer"));
                };
                values.push(value);
                Ok(())
            }
            other => Err(self.unsupported(path, &format!("a parameter of kind {other:?}"))),
        }
    }

    fn block(
        &mut self,
        layout: TypeLayout<'_>,
        kind: TypeKind,
        path: &str,
        here: Position,
    ) -> Result<()> {
        let (Some(container), Some(element)) =
            (layout.container_var_layout(), layout.element_var_layout())
        else {
            return Err(self.unsupported(path, "Slang reports no layout for its contents"));
        };
        let Some(element_layout) = element.type_layout() else {
            return Err(self.unsupported(path, "Slang reports no layout for its contents"));
        };
        let inner = if kind == TypeKind::ParameterBlock {
            here.parameter_block()
        } else {
            here
        };
        let size = self.finite(element_layout.size(ParameterCategory::Uniform), path)?;
        let mut values = Vec::new();
        self.variable(element, path, inner.block_element(), Some(&mut values))?;
        let block = UniformBlock { size, values };
        let is_push_constant = layout
            .categories()
            .collect::<slang_compiler::Result<Vec<_>>>()
            .map_err(Error::compile)?
            .contains(&ParameterCategory::PushConstantBuffer);
        if is_push_constant {
            if self.immediates.is_some() {
                return Err(self.unsupported(path, "a second block of immediates"));
            }
            self.immediates = Some(block);
        } else if size > 0 {
            let at = inner.descend(offsets(container));
            self.push_binding(path, at, Resource::UniformBuffer(block))?;
        }
        Ok(())
    }

    fn resource(&mut self, layout: TypeLayout<'_>, path: &str, here: Position) -> Result<()> {
        let Some(ty) = layout.ty() else {
            return Err(self.unsupported(path, "Slang reports no type for it"));
        };
        let shape = ty.resource_shape().base().map_err(Error::compile)?;
        let access = ty.resource_access().map_err(Error::compile)?;
        let access = match (shape, access) {
            (ResourceBaseShape::StructuredBuffer, ResourceAccess::Read) => Access::ReadOnly,
            (ResourceBaseShape::StructuredBuffer, ResourceAccess::ReadWrite) => Access::ReadWrite,
            (ResourceBaseShape::Texture2d, ResourceAccess::Read) => {
                return self.texture(ty, path, here);
            }
            (shape, access) => {
                return Err(self.unsupported(
                    path,
                    &format!("a resource of shape {shape:?} with access {access:?}"),
                ));
            }
        };
        let Some(element) = layout.element_type_layout() else {
            return Err(self.unsupported(path, "Slang reports no layout for its elements"));
        };
        let stride = self.finite(element.stride(ParameterCategory::Uniform), path)?;
        self.push_binding(path, here, Resource::StorageBuffer { access, stride })
    }

    /// Records a `Texture2D` with a `float` result, the one texture of the
    /// supported set of Section 8.1 of `docs/design/slango.md`.
    fn texture(&mut self, ty: Type<'_>, path: &str, here: Position) -> Result<()> {
        let shape = ty.resource_shape();
        let flags = [
            (shape.is_combined(), "a texture combined with a sampler"),
            (shape.is_array(), "an array texture"),
            (shape.is_multisample(), "a multisampled texture"),
            (shape.is_shadow(), "a shadow texture"),
            (shape.is_feedback(), "a feedback texture"),
        ];
        if let Some((_, what)) = flags.iter().find(|(flag, _)| *flag) {
            return Err(self.unsupported(path, what));
        }
        let Some(result) = ty.resource_result_type() else {
            return Err(self.unsupported(path, "a texture without a result type"));
        };
        let scalar = result.scalar_type().map_err(Error::compile)?;
        if scalar != ScalarType::Float32 {
            return Err(self.unsupported(
                path,
                &format!("a texture with results of scalar type {scalar:?}"),
            ));
        }
        self.push_binding(path, here, Resource::SampledTexture)
    }

    fn sampler(&mut self, layout: TypeLayout<'_>, path: &str, here: Position) -> Result<()> {
        let name = layout
            .ty()
            .map(Type::name)
            .transpose()
            .map_err(Error::compile)?
            .flatten();
        if name != Some("SamplerState") {
            let name = name.unwrap_or("an unnamed sampler type");
            return Err(self.unsupported(path, &format!("a sampler of type {name}")));
        }
        self.push_binding(path, here, Resource::Sampler)
    }

    /// Records the vertex attributes of `variable`, a parameter of a vertex
    /// entry point or a field of one, by the rules of Section 7.4 of
    /// `docs/design/slango.md`.
    fn attribute(
        &mut self,
        variable: VariableLayout<'_>,
        path: &str,
        base: Position,
    ) -> Result<()> {
        let Some(layout) = variable.type_layout() else {
            return Err(self.unsupported(path, "Slang reports no layout for it"));
        };
        let here = base.descend(offsets(variable));
        let kind = layout.kind().map_err(Error::compile)?;
        if kind == TypeKind::Struct {
            for field in layout.field_layouts() {
                let field_path = join(path, &name_of(field)?);
                self.attribute(field, &field_path, here)?;
            }
            return Ok(());
        }
        let is_varying_input = layout
            .categories()
            .collect::<slang_compiler::Result<Vec<_>>>()
            .map_err(Error::compile)?
            .contains(&ParameterCategory::VaryingInput);
        if !is_varying_input {
            return Ok(());
        }
        let Some(ty) = layout.ty() else {
            return Err(self.unsupported(path, "Slang reports no type for it"));
        };
        let components = match kind {
            TypeKind::Scalar => 1,
            TypeKind::Vector => match ty.element_count() {
                LayoutSize::Finite(length) => length,
                _ => 0,
            },
            other => {
                return Err(
                    self.unsupported(path, &format!("a vertex attribute of kind {other:?}"))
                );
            }
        };
        let scalar_type = ty.scalar_type().map_err(Error::compile)?;
        let Some((scalar, components)) = attribute_type(scalar_type, components) else {
            return Err(self.unsupported(
                path,
                &format!(
                    "a vertex attribute of scalar type {scalar_type:?} with {components} components"
                ),
            ));
        };
        let location = self.narrow(here.location, path, "a location")?;
        let used = self
            .metadata
            .is_parameter_location_used(
                ParameterCategory::VaryingInput,
                BindingSpace::new(0),
                BindingIndex::new(here.location),
            )
            .map_err(Error::compile)?;
        self.attributes.push(Attribute {
            path: path.to_owned(),
            location,
            scalar,
            components,
            usage: if used { Usage::Used } else { Usage::Unused },
        });
        Ok(())
    }

    fn plain_value(
        &self,
        layout: TypeLayout<'_>,
        path: &str,
        here: Position,
    ) -> Result<PlainValue> {
        let size = self.finite(layout.size(ParameterCategory::Uniform), path)?;
        let (scalar, shape) = self.value_type(layout, path)?;
        Ok(PlainValue {
            path: path.to_owned(),
            offset: here.byte,
            size,
            scalar,
            shape,
        })
    }

    fn value_type(&self, layout: TypeLayout<'_>, path: &str) -> Result<(Scalar, Shape)> {
        let Some(ty) = layout.ty() else {
            return Err(self.unsupported(path, "Slang reports no type for it"));
        };
        let kind = layout.kind().map_err(Error::compile)?;
        let shape = match kind {
            TypeKind::Scalar => Shape::Scalar,
            TypeKind::Vector => Shape::Vector {
                length: self.length(ty.element_count(), path)?,
            },
            TypeKind::Matrix => Shape::Matrix {
                rows: self.narrow(ty.row_count(), path, "a row count")?,
                columns: self.narrow(ty.column_count(), path, "a column count")?,
            },
            TypeKind::Array => {
                let Some(element) = layout.element_type_layout() else {
                    return Err(self.unsupported(path, "Slang reports no layout for its elements"));
                };
                let LayoutSize::Finite(length) = ty.element_count() else {
                    return Err(self.unsupported(path, "an array without a fixed length"));
                };
                let stride =
                    self.finite(layout.element_stride(ParameterCategory::Uniform), path)?;
                let (scalar, element) = self.value_type(element, path)?;
                return Ok((
                    scalar,
                    Shape::Array {
                        length,
                        stride,
                        element: Box::new(element),
                    },
                ));
            }
            other => {
                return Err(
                    self.unsupported(path, &format!("an array of elements of kind {other:?}"))
                );
            }
        };
        let scalar = match ty.scalar_type().map_err(Error::compile)? {
            ScalarType::Bool => Scalar::Bool,
            ScalarType::Int32 => Scalar::Int32,
            ScalarType::Uint32 => Scalar::Uint32,
            ScalarType::Float32 => Scalar::Float32,
            other => {
                return Err(
                    self.unsupported(path, &format!("a plain value of scalar type {other:?}"))
                );
            }
        };
        Ok((scalar, shape))
    }

    fn push_binding(&mut self, path: &str, at: Position, resource: Resource) -> Result<()> {
        let used = self
            .metadata
            .is_parameter_location_used(
                ParameterCategory::DescriptorTableSlot,
                BindingSpace::new(at.set),
                BindingIndex::new(at.binding),
            )
            .map_err(Error::compile)?;
        self.bindings.push(Binding {
            path: path.to_owned(),
            set: self.narrow(at.set, path, "a set")?,
            index: self.narrow(at.binding, path, "a binding")?,
            usage: if used { Usage::Used } else { Usage::Unused },
            resource,
        });
        Ok(())
    }

    fn finite(&self, units: LayoutUnits, path: &str) -> Result<u64> {
        match units.amount {
            LayoutSize::Finite(amount) => Ok(amount),
            _ => Err(self.unsupported(path, "a size that is not finite")),
        }
    }

    fn length(&self, length: LayoutSize, path: &str) -> Result<u32> {
        match length {
            LayoutSize::Finite(length) => self.narrow(length, path, "a length"),
            _ => Err(self.unsupported(path, "a length that is not finite")),
        }
    }

    fn narrow<T>(&self, value: T, path: &str, what: &str) -> Result<u32>
    where
        T: TryInto<u32> + Copy + fmt::Display,
    {
        let Ok(narrow) = value.try_into() else {
            return Err(self.unsupported(path, &format!("{what} of {value}, above 32 bits")));
        };
        Ok(narrow)
    }

    fn unsupported(&self, path: &str, what: &str) -> Error {
        let entry_point = self.entry_point;
        let noun = self.role.noun();
        Error::new(ErrorKind::UnsupportedParameter, LOAD).with_detail(format!(
            "the parameter `{path}` of the {noun} `{entry_point}` is {what}, which this version does not support"
        ))
    }
}

fn name_of(variable: VariableLayout<'_>) -> Result<String> {
    Ok(variable
        .variable()
        .map(slang_compiler::reflection::Variable::name)
        .transpose()
        .map_err(Error::compile)?
        .flatten()
        .unwrap_or_default()
        .to_owned())
}

fn join(path: &str, name: &str) -> String {
    if path.is_empty() {
        name.to_owned()
    } else {
        format!("{path}.{name}")
    }
}

fn offsets(variable: VariableLayout<'_>) -> Offsets {
    let amount = |units: LayoutUnits| match units.amount {
        LayoutSize::Finite(amount) => amount,
        _ => 0,
    };
    Offsets {
        set: variable
            .binding_space(ParameterCategory::DescriptorTableSlot)
            .map_or(0, BindingSpace::get),
        binding: amount(variable.offset(ParameterCategory::DescriptorTableSlot)),
        byte: amount(variable.offset(ParameterCategory::Uniform)),
        register_space: amount(variable.offset(ParameterCategory::SubElementRegisterSpace)),
        location: amount(variable.offset(ParameterCategory::VaryingInput)),
    }
}

#[cfg(test)]
mod tests;
