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

/// The layout snapshot of one kernel, the owned facts from which the
/// backend builds its layouts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct KernelLayout {
    pub(crate) name: String,
    pub(crate) thread_group_size: [u32; 3],
    /// The bindings of the kernel, ordered by set and binding.
    pub(crate) bindings: Vec<Binding>,
    /// The `uniform` parameters of the entry point, which the dispatch
    /// passes as immediates, when it has any.
    pub(crate) immediates: Option<UniformBlock>,
    /// The structures among the plain values of the kernel, in the order of
    /// the walk, which the generator of typed bindings reads.
    pub(crate) structures: Vec<UniformStructure>,
}

/// A structure among the plain values of a kernel, whose fields are the
/// plain values under its path.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct UniformStructure {
    pub(crate) path: String,
    /// The Slang name of the structure.
    pub(crate) name: String,
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

impl AsRef<Self> for Binding {
    fn as_ref(&self) -> &Self {
        self
    }
}

/// Where a binding of a kernel sits among the parameters of its program,
/// which the walk records beside the layout snapshot of the kernel for the
/// backend that sets each parameter by its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Place {
    /// Whether the binding belongs to the parameters of the entry point,
    /// rather than to the global parameters.
    pub(crate) in_entry_point: bool,
    /// Whether the path of the binding passes through a structure that is
    /// a plain parameter, or a field of one, rather than the element of a
    /// constant buffer or a parameter block.
    pub(crate) in_structure: bool,
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
        element: ElementType,
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

/// The element type of a structured buffer, which the check of a bound
/// buffer compares with the element type of the buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ElementType {
    /// A scalar or a vector of `bool`, `int`, `uint`, or `float`, with its
    /// number of components, 1 for a scalar.
    Plain { scalar: Scalar, components: u32 },
    /// A scalar or a vector of another scalar type, such as `half2`, which
    /// no Rust element type has, with the name that Slang gives it.
    OtherPlain { name: String },
    /// A structure, whose bound buffer is checked by its stride alone.
    Structure(Structure),
    /// A matrix, an array, or another type that is neither a scalar, a
    /// vector, nor a structure, whose bound buffer is checked by its stride
    /// alone.
    Composite,
}

/// A structure that is the element of a structured buffer, with its layout
/// in the buffer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Structure {
    /// The Slang name of the structure.
    pub(crate) name: String,
    /// The size of the structure in bytes.
    pub(crate) size: u64,
    pub(crate) fields: Vec<Field>,
}

/// A field of a structure, at its byte offset in the structure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Field {
    pub(crate) name: String,
    pub(crate) offset: u64,
    pub(crate) ty: FieldType,
}

/// The type of a field of a structure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FieldType {
    /// A scalar, vector, matrix, or array of `bool`, `int`, `uint`, or
    /// `float`.
    Plain {
        scalar: Scalar,
        shape: Shape,
    },
    Structure(Structure),
    /// Any other type, with the name that Slang gives it.
    Other {
        name: String,
    },
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

pub(crate) fn slang_scalar_name(scalar: Scalar) -> &'static str {
    match scalar {
        Scalar::Bool => "bool",
        Scalar::Int32 => "int",
        Scalar::Uint32 => "uint",
        Scalar::Float32 => "float",
    }
}

/// Returns the name of the Slang type of `components` scalars of the type
/// `scalar`, such as `float2`, or `uint` for one component.
pub(crate) fn slang_type(scalar: Scalar, components: u32) -> String {
    let name = slang_scalar_name(scalar);
    if components == 1 {
        name.to_owned()
    } else {
        format!("{name}{components}")
    }
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
        row_stride: u64,
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
/// from which a render pipeline builds its layouts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RenderLayout {
    /// The bindings of the entry point, ordered by set and binding, each
    /// with the usage of this entry point.
    pub(crate) bindings: Vec<Binding>,
    /// The `uniform` parameters of the entry point, which a render pipeline
    /// rejects, when it has any.
    pub(crate) immediates: Option<UniformBlock>,
    /// The vertex attributes, ordered as the walk visits them, which is
    /// empty for a fragment entry point.
    pub(crate) attributes: Vec<Attribute>,
}

/// One vertex attribute of a vertex entry point.
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
    pub(crate) registers: Registers,
}

/// The parameter categories of the register classes of a target that places
/// each resource in a register of its class, such as DXIL, in the order of
/// the arrays of [`Registers`].
const REGISTER_CLASSES: [ParameterCategory; 4] = [
    ParameterCategory::ShaderResource,
    ParameterCategory::UnorderedAccess,
    ParameterCategory::ConstantBuffer,
    ParameterCategory::SamplerState,
];

/// The register index and the register space in each register class of
/// [`REGISTER_CLASSES`], which a target such as DXIL reports in place of a
/// set and a binding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Registers {
    pub(crate) index: [u64; 4],
    pub(crate) space: [u64; 4],
}

/// The running sums of a walk at one variable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Position {
    pub(crate) set: u64,
    pub(crate) binding: u64,
    pub(crate) byte: u64,
    pub(crate) register_space: u64,
    /// The location of a varying input.
    pub(crate) location: u64,
    pub(crate) registers: Registers,
}

impl Position {
    pub(crate) fn descend(self, offsets: Offsets) -> Self {
        let mut registers = self.registers;
        for (sum, offset) in registers.index.iter_mut().zip(offsets.registers.index) {
            *sum += offset;
        }
        for (sum, offset) in registers.space.iter_mut().zip(offsets.registers.space) {
            *sum += offset;
        }
        Self {
            set: self.set + offsets.set,
            binding: self.binding + offsets.binding,
            byte: self.byte + offsets.byte,
            register_space: self.register_space + offsets.register_space,
            location: self.location + offsets.location,
            registers,
        }
    }

    /// The position inside a parameter block, whose set and register spaces
    /// are the running sum of the register-space offsets and whose bindings
    /// and registers restart at 0.
    pub(crate) fn parameter_block(self) -> Self {
        Self {
            set: self.register_space,
            binding: 0,
            registers: Registers {
                index: [0; 4],
                space: [self.register_space; 4],
            },
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
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CompiledKernel {
    pub(crate) layout: KernelLayout,
    pub(crate) spirv: Vec<u32>,
}

/// One vertex or fragment entry point as one compilation yields it: its
/// layout snapshot and its SPIR-V, or the rejection that makes it unusable.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct CompiledRender {
    pub(crate) name: String,
    pub(crate) stage: RenderStage,
    pub(crate) usable: std::result::Result<UsableRender, Rejection>,
}

/// The layout snapshot of a vertex or fragment entry point and its SPIR-V,
/// whose entry point keeps the Slang name.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct UsableRender {
    pub(crate) layout: RenderLayout,
    pub(crate) spirv: Vec<u32>,
}

/// Why a vertex or fragment entry point cannot be used: the kind of the
/// failure, [`ErrorKind::UnsupportedParameter`] when the walk refused one of
/// its parameters and [`ErrorKind::Compile`] when Slang failed on it, the
/// text that describes it, and the text of the source error, if any.
///
/// It keeps the text of the source error rather than the error itself,
/// because a module returns the rejection from every call of
/// [`Module::render_pipeline`](crate::Module::render_pipeline) that names
/// the entry point, and a precompiled module decodes it from bytes. That
/// call returns the text as the source of its error.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Rejection {
    pub(crate) kind: ErrorKind,
    pub(crate) detail: String,
    pub(crate) source: Option<String>,
}

/// What one compilation of a module yields for its compute, vertex, and
/// fragment entry points, in the order of the module: the layout snapshot
/// of the module with the SPIR-V of each entry point, from which a load
/// creates a [`Module`](crate::Module).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Compiled {
    pub(crate) kernels: Vec<CompiledKernel>,
    pub(crate) render: Vec<CompiledRender>,
}

/// One compilation of a module: its snapshot, and the paths of the files
/// that Slang read for it, which a load does not need.
#[derive(Debug)]
pub(crate) struct Compilation {
    pub(crate) compiled: Compiled,
    pub(crate) dependencies: Vec<String>,
}

/// The Slang source of one module, with the path that diagnostics name and
/// the directory, if any, that the compilation adds as a search path.
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
/// SPIR-V of each compute, vertex, and fragment entry point, with the paths
/// of the files that Slang read, releasing every Slang object before it
/// returns.
///
/// A failure that belongs to one kernel fails the compilation. A failure that
/// belongs to one vertex or fragment entry point, in its metadata, its walk,
/// or its SPIR-V, becomes the rejection of its record.
pub(crate) fn compile(source: &Source<'_>) -> Result<Compilation> {
    let (program, target, dependencies) = link(source)?;
    let (kernels, render) = walk_program(
        &program,
        target,
        |at, layout, _| {
            let spirv = spirv(at.program, at.index, at.target, &layout.name)?;
            Ok(CompiledKernel { layout, spirv })
        },
        |at, name, stage| {
            let role = match stage {
                RenderStage::Vertex => Role::Vertex,
                RenderStage::Fragment => Role::Fragment,
            };
            render_entry(at, name, role)
        },
    )?;
    Ok(Compilation {
        compiled: Compiled { kernels, render },
        dependencies,
    })
}

/// One entry point of a linked program, with what its records read.
pub(crate) struct LinkedEntryPoint<'p> {
    program: &'p LinkedProgram,
    layout: ProgramLayout<'p>,
    entry_point: EntryPointLayout<'p>,
    /// The position of the entry point among the entry points of the
    /// program.
    pub(crate) index: usize,
    target: TargetIndex,
}

/// Compiles, composes, and links the module of `source` with all its entry
/// points, and returns the program, its one target, and the paths of the
/// files that Slang read.
fn link(source: &Source<'_>) -> Result<(LinkedProgram, TargetIndex, Vec<String>)> {
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
    let dependencies = dependencies(&module)?;
    Ok((program, target, dependencies))
}

/// Returns the role of an entry point by its stage, or `None` for a stage
/// that the load ignores.
fn role_of(entry_point: EntryPointLayout<'_>) -> Result<Option<Role>> {
    Ok(match entry_point.stage().map_err(Error::compile)? {
        Some(Stage::Compute) => Some(Role::Kernel),
        Some(Stage::Vertex) => Some(Role::Vertex),
        Some(Stage::Fragment) => Some(Role::Fragment),
        _ => None,
    })
}

/// Returns the name of an entry point, or `None` for a vertex or fragment
/// entry point whose name Slang cannot report, which the load skips, and
/// fails for such a kernel.
fn entry_point_name(entry_point: EntryPointLayout<'_>, role: Role) -> Result<Option<String>> {
    let name = entry_point.name().map_err(Error::compile);
    if !matches!(role, Role::Kernel) {
        return Ok(name.ok().flatten().map(str::to_owned));
    }
    match name? {
        Some(name) => Ok(Some(name.to_owned())),
        None => Err(Error::new(ErrorKind::Compile, LOAD)
            .with_detail("Slang reports no name for an entry point".to_owned())),
    }
}

/// Returns the metadata of the entry point `at`.
fn metadata(at: &LinkedEntryPoint<'_>) -> Result<Metadata> {
    at.program
        .entry_point_metadata(at.index, at.target)
        .map_err(Error::compile)
        .map(slang_compiler::Compiled::into_value)
}

/// Returns the layout snapshot of the kernel `at`, whose name is `name`, and
/// the place of each of its bindings, in the order of the bindings.
fn kernel_layout(at: &LinkedEntryPoint<'_>, name: String) -> Result<(KernelLayout, Vec<Place>)> {
    let metadata = metadata(at)?;
    let walk = walk(at.layout, at.entry_point, &metadata, &name, Role::Kernel)?;
    let [x, y, z] = at.entry_point.compute_thread_group_size();
    let thread_group_size = [
        walk.narrow(x, "", "a thread group size")?,
        walk.narrow(y, "", "a thread group size")?,
        walk.narrow(z, "", "a thread group size")?,
    ];
    let places = walk.places;
    let layout = KernelLayout {
        thread_group_size,
        bindings: walk.bindings,
        immediates: walk.immediates,
        structures: walk.structures,
        name,
    };
    Ok((layout, places))
}

/// Walks each entry point of `program`, a module linked with all its entry
/// points in the order of the module, for the target `target` of its
/// session, and returns what `kernel` makes of each kernel and its layout
/// snapshot and what `render` makes of each vertex and fragment entry point,
/// its name, and its stage, in the order of the module. `kernel` also gets
/// the place of each binding of the snapshot, in the order of the bindings.
///
/// Every offset, size, and stride of a snapshot is that of the target, and
/// the usage of each binding is the one that the metadata of the target
/// reports. The walk generates the code of each kernel for the target, which
/// its metadata needs. A failure of a kernel fails the walk.
pub(crate) fn walk_program<K, R>(
    program: &LinkedProgram,
    target: TargetIndex,
    mut kernel: impl FnMut(&LinkedEntryPoint<'_>, KernelLayout, Vec<Place>) -> Result<K>,
    mut render: impl FnMut(&LinkedEntryPoint<'_>, String, RenderStage) -> R,
) -> Result<(Vec<K>, Vec<R>)> {
    let layout = program.layout(target).map_err(Error::compile)?.into_value();
    let mut kernels = Vec::new();
    let mut records = Vec::new();
    for (index, entry_point) in layout.entry_point_layouts().enumerate() {
        let Some(role) = role_of(entry_point)? else {
            continue;
        };
        let Some(name) = entry_point_name(entry_point, role)? else {
            continue;
        };
        let at = LinkedEntryPoint {
            program,
            layout,
            entry_point,
            index,
            target,
        };
        match role {
            Role::Kernel => {
                let (snapshot, places) = kernel_layout(&at, name)?;
                kernels.push(kernel(&at, snapshot, places)?);
            }
            Role::Vertex => records.push(render(&at, name, RenderStage::Vertex)),
            Role::Fragment => records.push(render(&at, name, RenderStage::Fragment)),
        }
    }
    Ok((kernels, records))
}

/// Returns the record of the vertex or fragment entry point `at`, whose
/// name is `name`, with the first failure as its rejection.
fn render_entry(at: &LinkedEntryPoint<'_>, name: String, role: Role) -> CompiledRender {
    let layout = metadata(at).and_then(|metadata| {
        walk(at.layout, at.entry_point, &metadata, &name, role).map(|walk| RenderLayout {
            bindings: walk.bindings,
            immediates: walk.immediates,
            attributes: walk.attributes,
        })
    });
    let spirv = spirv(at.program, at.index, at.target, &name);
    render_record(name, role, layout, spirv)
}

/// Returns the paths of the files that Slang read to compile `module`.
fn dependencies(module: &slang_compiler::Module) -> Result<Vec<String>> {
    module
        .dependency_file_paths()
        .map_err(Error::compile)?
        .filter_map(|path| path.map(|path| path.map(str::to_owned)).transpose())
        .collect::<slang_compiler::Result<Vec<String>>>()
        .map_err(Error::compile)
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
/// that made it unusable, with the detail of the error and the text of its
/// source.
fn rejection(error: &Error, role: Role, name: &str) -> Rejection {
    let source = StdError::source(error).map(ToString::to_string);
    let detail = match (error.detail(), &source) {
        (Some(detail), _) => detail.to_owned(),
        (None, Some(_)) => format!("Slang failed on the {} `{name}`", role.noun()),
        (None, None) => format!("{error}, for the {} `{name}`", role.noun()),
    };
    Rejection {
        kind: error.kind(),
        detail,
        source,
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
        places: Vec::new(),
        immediates: None,
        attributes: Vec::new(),
        structures: Vec::new(),
        in_entry_point: false,
        structure_depth: 0,
    };
    if let Some(globals) = layout.global_params_var_layout() {
        walk.variable(globals, "", Position::default(), None)?;
    }
    if let Some(parameters) = entry_point.var_layout() {
        walk.in_entry_point = true;
        walk.variable(parameters, "", Position::default(), None)?;
        walk.in_entry_point = false;
    }
    if matches!(role, Role::Vertex) {
        for parameter in entry_point.parameter_layouts() {
            let path = name_of(parameter)?;
            walk.attribute(parameter, &path, Position::default())?;
        }
    }
    let mut placed: Vec<(Binding, Place)> = std::mem::take(&mut walk.bindings)
        .into_iter()
        .zip(std::mem::take(&mut walk.places))
        .collect();
    placed.sort_by_key(|(binding, _)| (binding.set, binding.index));
    (walk.bindings, walk.places) = placed.into_iter().unzip();
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
    /// The place of each binding, in the order of `bindings`.
    places: Vec<Place>,
    immediates: Option<UniformBlock>,
    attributes: Vec<Attribute>,
    structures: Vec<UniformStructure>,
    /// Whether the walk is at the parameters of the entry point, whose
    /// outermost block holds its `uniform` parameters, which become the
    /// immediates also on a target that does not place them in push
    /// constants, such as DXIL.
    in_entry_point: bool,
    /// The number of structures that are plain parameters, or fields of
    /// one, around the current parameter.
    structure_depth: u32,
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
                if values.is_some() {
                    let name = self.structure_name(layout, path)?;
                    self.structures.push(UniformStructure {
                        path: path.to_owned(),
                        name,
                    });
                }
                if path.is_empty() {
                    return self.fields(layout, path, here, values);
                }
                self.structure_depth += 1;
                let walked = self.fields(layout, path, here, values);
                self.structure_depth -= 1;
                walked
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

    fn fields(
        &mut self,
        layout: TypeLayout<'_>,
        path: &str,
        here: Position,
        mut values: Option<&mut Vec<PlainValue>>,
    ) -> Result<()> {
        for field in layout.field_layouts() {
            let field_path = join(path, &name_of(field)?);
            self.variable(field, &field_path, here, values.as_deref_mut())?;
        }
        Ok(())
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
        if element_layout.kind().map_err(Error::compile)? == TypeKind::Struct {
            let here = inner.block_element().descend(offsets(element));
            self.fields(element_layout, path, here, Some(&mut values))?;
        } else {
            self.variable(element, path, inner.block_element(), Some(&mut values))?;
        }
        let block = UniformBlock { size, values };
        let is_push_constant = categories(layout)?.contains(&ParameterCategory::PushConstantBuffer)
            || (self.in_entry_point && path.is_empty());
        if is_push_constant {
            if self.immediates.is_some() {
                return Err(self.unsupported(path, "a second block of immediates"));
            }
            self.immediates = Some(block);
        } else if size > 0 {
            let at = inner.descend(offsets(container));
            let categories = match container.type_layout() {
                Some(container) => categories(container)?,
                None => Vec::new(),
            };
            self.push_binding(path, at, &categories, Resource::UniformBuffer(block))?;
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
                return self.texture(layout, ty, path, here);
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
        let element = self.element_type(element, path)?;
        self.push_binding(
            path,
            here,
            &categories(layout)?,
            Resource::StorageBuffer {
                access,
                stride,
                element,
            },
        )
    }

    /// Returns the element type of a structured buffer whose elements have
    /// the layout `element`.
    fn element_type(&self, element: TypeLayout<'_>, path: &str) -> Result<ElementType> {
        let kind = element.kind().map_err(Error::compile)?;
        if kind == TypeKind::Struct {
            return self.structure(element, path).map(ElementType::Structure);
        }
        if !matches!(kind, TypeKind::Scalar | TypeKind::Vector) {
            return Ok(ElementType::Composite);
        }
        let Some(ty) = element.ty() else {
            return Err(self.unsupported(path, "Slang reports no type for its elements"));
        };
        let components = if kind == TypeKind::Vector {
            self.length(ty.element_count(), path)?
        } else {
            1
        };
        let scalar = match ty.scalar_type().map_err(Error::compile)? {
            ScalarType::Bool => Scalar::Bool,
            ScalarType::Int32 => Scalar::Int32,
            ScalarType::Uint32 => Scalar::Uint32,
            ScalarType::Float32 => Scalar::Float32,
            _ => {
                let name = ty.full_name().map_err(Error::compile)?;
                return Ok(ElementType::OtherPlain { name });
            }
        };
        Ok(ElementType::Plain { scalar, components })
    }

    /// Returns the layout of a structure that is, or is inside, the element
    /// of the structured buffer at `path`.
    fn structure(&self, layout: TypeLayout<'_>, path: &str) -> Result<Structure> {
        let name = self.structure_name(layout, path)?;
        let size = self.finite(layout.size(ParameterCategory::Uniform), path)?;
        let mut fields = Vec::new();
        for field in layout.field_layouts() {
            let name = name_of(field)?;
            let Some(field_layout) = field.type_layout() else {
                return Err(self.unsupported(path, "a structure whose field has no layout"));
            };
            let offset = self.finite(field.offset(ParameterCategory::Uniform), path)?;
            let ty = self.field_type(field_layout, path)?;
            fields.push(Field { name, offset, ty });
        }
        Ok(Structure { name, size, fields })
    }

    /// Returns the type of a field of a structure in the structured buffer at
    /// `path`. A type that a plain value cannot have is recorded by its
    /// Slang name, because only the generator of typed bindings reads it.
    fn field_type(&self, layout: TypeLayout<'_>, path: &str) -> Result<FieldType> {
        if layout.kind().map_err(Error::compile)? == TypeKind::Struct {
            return self.structure(layout, path).map(FieldType::Structure);
        }
        match self.value_type(layout, path) {
            Ok((scalar, shape)) => Ok(FieldType::Plain { scalar, shape }),
            Err(error) if error.kind() == ErrorKind::UnsupportedParameter => {
                let Some(ty) = layout.ty() else {
                    return Err(self.unsupported(path, "a structure whose field has no type"));
                };
                let name = ty.full_name().map_err(Error::compile)?;
                Ok(FieldType::Other { name })
            }
            Err(error) => Err(error),
        }
    }

    /// Returns the Slang name of the structure whose layout is `layout`.
    fn structure_name(&self, layout: TypeLayout<'_>, path: &str) -> Result<String> {
        let Some(ty) = layout.ty() else {
            return Err(self.unsupported(path, "a structure without a type"));
        };
        match ty.name().map_err(Error::compile)? {
            Some(name) => Ok(name.to_owned()),
            None => ty.full_name().map_err(Error::compile),
        }
    }

    /// Records a `Texture2D` with a `float` result, the one texture that the
    /// crate supports.
    fn texture(
        &mut self,
        layout: TypeLayout<'_>,
        ty: Type<'_>,
        path: &str,
        here: Position,
    ) -> Result<()> {
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
        self.push_binding(path, here, &categories(layout)?, Resource::SampledTexture)
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
        self.push_binding(path, here, &categories(layout)?, Resource::Sampler)
    }

    /// Records the vertex attributes of `variable`, a parameter of a vertex
    /// entry point or a field of one.
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
        let is_varying_input = categories(layout)?.contains(&ParameterCategory::VaryingInput);
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
            TypeKind::Matrix => {
                let Some(row) = layout.element_type_layout() else {
                    return Err(self.unsupported(path, "Slang reports no layout for its rows"));
                };
                Shape::Matrix {
                    rows: self.narrow(ty.row_count(), path, "a row count")?,
                    columns: self.narrow(ty.column_count(), path, "a column count")?,
                    row_stride: self.finite(row.stride(ParameterCategory::Uniform), path)?,
                }
            }
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

    fn push_binding(
        &mut self,
        path: &str,
        at: Position,
        categories: &[ParameterCategory],
        resource: Resource,
    ) -> Result<()> {
        let (category, space, index) = location_of(categories, at);
        let used = self
            .metadata
            .is_parameter_location_used(
                category,
                BindingSpace::new(space),
                BindingIndex::new(index),
            )
            .map_err(Error::compile)?;
        self.bindings.push(Binding {
            path: path.to_owned(),
            set: self.narrow(at.set, path, "a set")?,
            index: self.narrow(at.binding, path, "a binding")?,
            usage: if used { Usage::Used } else { Usage::Unused },
            resource,
        });
        self.places.push(Place {
            in_entry_point: self.in_entry_point,
            in_structure: self.structure_depth > 0,
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
            "the parameter `{path}` of the {noun} `{entry_point}` is {what}, which slango does not support"
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
    let space = |category| {
        variable
            .binding_space(category)
            .map_or(0, BindingSpace::get)
    };
    Offsets {
        set: space(ParameterCategory::DescriptorTableSlot),
        binding: amount(variable.offset(ParameterCategory::DescriptorTableSlot)),
        byte: amount(variable.offset(ParameterCategory::Uniform)),
        register_space: amount(variable.offset(ParameterCategory::SubElementRegisterSpace)),
        location: amount(variable.offset(ParameterCategory::VaryingInput)),
        registers: Registers {
            index: REGISTER_CLASSES.map(|category| amount(variable.offset(category))),
            space: REGISTER_CLASSES.map(space),
        },
    }
}

/// Returns the parameter categories that the type of `layout` uses.
fn categories(layout: TypeLayout<'_>) -> Result<Vec<ParameterCategory>> {
    layout
        .categories()
        .collect::<slang_compiler::Result<Vec<_>>>()
        .map_err(Error::compile)
}

/// Returns the category, the space, and the index at which the metadata of an
/// entry point reports whether its code uses a binding at `at` whose type
/// uses the parameter categories `categories`.
///
/// A target of descriptor sets, such as SPIR-V, places the binding by its
/// set and binding. A target of register classes, such as DXIL, places it by
/// the register space and the index of its class, which the binding's type
/// names among its categories.
fn location_of(categories: &[ParameterCategory], at: Position) -> (ParameterCategory, u64, u64) {
    if !categories.contains(&ParameterCategory::DescriptorTableSlot) {
        let class = REGISTER_CLASSES
            .iter()
            .position(|class| categories.contains(class));
        if let Some(class) = class {
            return (
                REGISTER_CLASSES[class],
                at.registers.space[class],
                at.registers.index[class],
            );
        }
    }
    (ParameterCategory::DescriptorTableSlot, at.set, at.binding)
}

#[cfg(test)]
mod tests;
