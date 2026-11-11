use std::fmt;

use slang_compiler::reflection::{
    LayoutSize, LayoutUnits, ResourceAccess, ResourceBaseShape, ScalarType, TypeKind, TypeLayout,
    VariableLayout,
};
use slang_compiler::{
    BindingIndex, BindingSpace, CompilerOptions, Component, EntryPoint, GlobalSession, Metadata,
    ParameterCategory, Session, Stage, Target, TargetFormat, Toggle,
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
    StorageBuffer { access: Access, stride: u64 },
    UniformBuffer(UniformBlock),
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

/// The offsets that one variable adds along the path of a walk, in the units
/// of each parameter category.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Offsets {
    pub(crate) set: u64,
    pub(crate) binding: u64,
    pub(crate) byte: u64,
    pub(crate) register_space: u64,
}

/// The running sums of a walk at one variable (Section 3.3 of
/// `docs/design/slango.md`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Position {
    pub(crate) set: u64,
    pub(crate) binding: u64,
    pub(crate) byte: u64,
    pub(crate) register_space: u64,
}

impl Position {
    pub(crate) fn descend(self, offsets: Offsets) -> Self {
        Self {
            set: self.set + offsets.set,
            binding: self.binding + offsets.binding,
            byte: self.byte + offsets.byte,
            register_space: self.register_space + offsets.register_space,
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

/// Compiles Slang source for SPIR-V and returns the layout snapshot of each
/// compute entry point, releasing every Slang object before it returns.
pub(crate) fn compile(name: &str, source: &str) -> Result<Vec<KernelLayout>> {
    let global = GlobalSession::new().map_err(Error::compile)?;
    let mut options = CompilerOptions::default();
    options.vulkan_use_entry_point_name(Toggle::On);
    let session = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv))
        .options(options)
        .build()
        .map_err(Error::compile)?;
    let target = session.target_index(0).map_err(Error::compile)?;
    let module = session
        .load_module_from_source(name, &format!("{name}.slang"), source)
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
    let mut kernels = Vec::new();
    for (index, entry_point) in layout.entry_point_layouts().enumerate() {
        if entry_point.stage().map_err(Error::compile)? != Some(Stage::Compute) {
            continue;
        }
        let name = entry_point
            .name()
            .map_err(Error::compile)?
            .ok_or_else(|| {
                Error::new(ErrorKind::Compile, LOAD)
                    .with_detail("Slang reports no name for an entry point".to_owned())
            })?
            .to_owned();
        let metadata = program
            .entry_point_metadata(index, target)
            .map_err(Error::compile)?
            .into_value();
        let mut walk = Walk {
            kernel: &name,
            metadata: &metadata,
            bindings: Vec::new(),
            immediates: None,
        };
        if let Some(globals) = layout.global_params_var_layout() {
            walk.variable(globals, "", Position::default(), None)?;
        }
        if let Some(parameters) = entry_point.var_layout() {
            walk.variable(parameters, "", Position::default(), None)?;
        }
        let [x, y, z] = entry_point.compute_thread_group_size();
        let thread_group_size = [
            walk.narrow(x, "", "a thread group size")?,
            walk.narrow(y, "", "a thread group size")?,
            walk.narrow(z, "", "a thread group size")?,
        ];
        let mut bindings = walk.bindings;
        bindings.sort_by_key(|binding| (binding.set, binding.index));
        kernels.push(KernelLayout {
            thread_group_size,
            bindings,
            immediates: walk.immediates,
            name,
        });
    }
    Ok(kernels)
}

struct Walk<'a> {
    kernel: &'a str,
    metadata: &'a Metadata,
    bindings: Vec<Binding>,
    immediates: Option<UniformBlock>,
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
                    let name = field
                        .variable()
                        .map(slang_compiler::reflection::Variable::name)
                        .transpose()
                        .map_err(Error::compile)?
                        .flatten()
                        .unwrap_or_default();
                    let field_path = if path.is_empty() {
                        name.to_owned()
                    } else {
                        format!("{path}.{name}")
                    };
                    self.variable(field, &field_path, here, values.as_deref_mut())?;
                }
                Ok(())
            }
            kind @ (TypeKind::ConstantBuffer | TypeKind::ParameterBlock) => {
                self.block(layout, kind, path, here)
            }
            TypeKind::Resource => self.resource(layout, path, here),
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
        let kernel = self.kernel;
        Error::new(ErrorKind::UnsupportedParameter, LOAD).with_detail(format!(
            "the parameter `{path}` of the kernel `{kernel}` is {what}, which this version does not support"
        ))
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
    }
}

#[cfg(test)]
mod tests;
