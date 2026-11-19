use std::rc::Rc;

use slang_compiler::{Component, EntryPoint};
use slang_rhi::{CompileSession, ComputePipeline, ShaderObject, ShaderProgram};

use super::device::Device;
use super::error::{Call, from_rhi};
use crate::call::{Argument, CallArgument, Plan, Source};
use crate::error::{Error, ErrorKind, Result};
use crate::layout::{Binding, KernelLayout, Place, Resource, UniformBlock, Usage};

const LOAD: &str = "load a Slang module";
const CALL: &str = "call a kernel";

/// The shader program and compute pipeline of slang-rhi of a
/// [`crate::Kernel`].
#[derive(Clone)]
pub(crate) struct Kernel {
    objects: Rc<Objects>,
}

struct Objects {
    pipeline: ComputePipeline,
    _program: ShaderProgram,
    /// The scope of each binding of the layout snapshot of the kernel, in
    /// the order of the bindings.
    scopes: Vec<Scope>,
}

/// Where a binding sits in the shader objects of a pipeline: among the
/// global parameters, which the root object holds, or among the parameters
/// of the entry point at this position of the program.
pub(super) type Scope = Option<usize>;

/// Why the backend wrote no argument into the shader objects of a dispatch
/// or a draw.
pub(super) enum Failure {
    /// slang-rhi failed.
    Rhi(slang_rhi::Error),
    /// The shader objects hold no parameter at the path, although the layout
    /// snapshot, which comes from the same program, holds one.
    Missing(String),
    /// The bytes or the buffer of a parameter do not fit what the shader
    /// object holds, as this text says.
    Mismatch(String),
}

impl From<slang_rhi::Error> for Failure {
    fn from(error: slang_rhi::Error) -> Self {
        Self::Rhi(error)
    }
}

impl Kernel {
    /// Links the kernel `entry_point` of `module`, a module of the session of
    /// `compile`, alone, and creates its shader program and compute pipeline
    /// on the device, for the kernel of `layout`, whose bindings sit at
    /// `places`.
    pub(crate) fn create(
        device: &Device,
        compile: &CompileSession,
        module: &slang_compiler::Module,
        entry_point: &EntryPoint,
        layout: &KernelLayout,
        places: &[Place],
    ) -> Result<Self> {
        check_addressable(layout, places)?;
        let name = layout.name.as_str();
        let linked = compile
            .session()
            .compose(&[
                Component::Module(module),
                Component::EntryPoint(entry_point),
            ])
            .map_err(Error::compile)?
            .into_value()
            .link()
            .map_err(Error::compile)?
            .into_value();
        let failed = |error| {
            from_rhi(LOAD, Call::Other, error).with_detail(format!(
                "slang-rhi cannot create the pipeline of the kernel `{name}`"
            ))
        };
        let program = compile.create_shader_program(&linked).map_err(failed)?;
        let pipeline = device
            .rhi_device()
            .create_compute_pipeline(&program)
            .map_err(failed)?;
        let scopes = places
            .iter()
            .map(|place| place.in_entry_point.then_some(0))
            .collect();
        Ok(Self {
            objects: Rc::new(Objects {
                pipeline,
                _program: program,
                scopes,
            }),
        })
    }

    /// Records one dispatch of `groups` thread groups with the arguments of
    /// `plan` written into the shader objects of the pipeline, and submits
    /// it.
    pub(crate) fn dispatch(
        &self,
        device: &Device,
        layout: &KernelLayout,
        plan: &Plan,
        arguments: &[(&str, CallArgument<'_>)],
        groups: [u32; 3],
    ) -> Result<()> {
        let name = layout.name.as_str();
        let rejected = |error| {
            from_rhi(CALL, Call::Other, error).with_detail(format!(
                "slang-rhi rejected the dispatch of the kernel `{name}`"
            ))
        };
        let mut encoder = device.queue().encoder().map_err(rejected)?;
        let unwritten = encoder
            .compute_pass(|pass| {
                let mut root = pass.bind_pipeline(&self.objects.pipeline)?;
                let written = write(
                    &mut root,
                    &layout.bindings,
                    &self.objects.scopes,
                    plan,
                    arguments,
                    layout.immediates.as_ref(),
                );
                match written {
                    Ok(()) => {}
                    Err(Failure::Rhi(error)) => return Err(error),
                    Err(failure) => return Ok(Some(failure)),
                }
                drop(root);
                pass.dispatch(groups)?;
                Ok(None)
            })
            .map_err(rejected)?;
        if let Some(failure) = unwritten {
            let detail = match failure {
                Failure::Missing(path) => format!(
                    "slang-rhi finds no parameter at the path `{path}` of the kernel `{name}`"
                ),
                Failure::Mismatch(text) => format!("{text}, of the kernel `{name}`"),
                Failure::Rhi(error) => return Err(rejected(error)),
            };
            return Err(Error::new(ErrorKind::Backend, CALL).with_detail(detail));
        }
        let commands = encoder.finish().map_err(rejected)?;
        device.queue().submit([commands]).map_err(rejected)
    }
}

/// Refuses a kernel of `layout`, whose bindings sit at `places`, that uses a
/// parameter that the shader objects of slang-rhi cannot address by name, as
/// [`unaddressable`] finds it.
fn check_addressable(layout: &KernelLayout, places: &[Place]) -> Result<()> {
    let name = layout.name.as_str();
    match unaddressable(layout.bindings.iter().zip(places)) {
        Some((path, what)) => Err(Error::new(ErrorKind::UnsupportedParameter, LOAD)
            .with_detail(format!(
                "the parameter `{path}` of the kernel `{name}` is {what}, which slango with the feature `rhi` does not support"
            ))),
        None => Ok(()),
    }
}

/// Returns the path of the first used binding of `placed`, each binding with
/// its place, that the shader objects of slang-rhi cannot address by name,
/// with what it is: a resource or block inside a structure that is a plain
/// parameter, or a constant buffer whose element is not a structure.
pub(super) fn unaddressable<'b>(
    placed: impl IntoIterator<Item = (&'b Binding, &'b Place)>,
) -> Option<(&'b str, &'static str)> {
    for (binding, place) in placed {
        if binding.usage == Usage::Unused {
            continue;
        }
        if place.in_structure {
            return Some((
                &binding.path,
                "a resource inside a structure that is not the element of a constant buffer or a parameter block",
            ));
        }
        if let Resource::UniformBuffer(block) = &binding.resource
            && !binding.path.is_empty()
            && top_level_fields(&binding.path, block).is_err()
        {
            return Some((
                &binding.path,
                "a constant buffer whose element is not a structure",
            ));
        }
    }
    None
}

/// Writes each used binding of `plan` into the root object `root` of a
/// pipeline whose bindings are `bindings`, or into the object of an entry
/// point for a binding whose scope in `scopes` is that entry point, and the
/// immediates of the block `immediates` into the object of the first entry
/// point.
pub(super) fn write<P: AsRef<Binding>>(
    root: &mut ShaderObject<'_>,
    bindings: &[P],
    scopes: &[Scope],
    plan: &Plan,
    arguments: &[(&str, CallArgument<'_>)],
    immediates: Option<&UniformBlock>,
) -> std::result::Result<(), Failure> {
    for entry in &plan.entries {
        let (Some(binding), Some(scope)) = (bindings.get(entry.binding), scopes.get(entry.binding))
        else {
            return Err(Failure::Mismatch(format!(
                "the call names the binding {}, past the {} bindings",
                entry.binding,
                bindings.len().min(scopes.len())
            )));
        };
        let binding = binding.as_ref();
        match scope {
            Some(index) => {
                let mut entry_point = root.entry_point(*index)?;
                write_entry(&mut entry_point, binding, &entry.source, arguments)?;
            }
            None => write_entry(root, binding, &entry.source, arguments)?,
        }
    }
    if let Some(block) = immediates
        && !plan.immediates.is_empty()
    {
        let mut entry_point = root.entry_point(0)?;
        write_values(&mut entry_point, "", block, &plan.immediates)?;
    }
    Ok(())
}

/// Writes the bytes or the argument of `source` into the parameter of
/// `binding`, whose path starts at `object`.
fn write_entry(
    object: &mut ShaderObject<'_>,
    binding: &Binding,
    source: &Source,
    arguments: &[(&str, CallArgument<'_>)],
) -> std::result::Result<(), Failure> {
    let path = binding.path.as_str();
    match source {
        Source::Uniform(bytes) => {
            let Resource::UniformBuffer(block) = &binding.resource else {
                return Err(Failure::Mismatch(format!(
                    "the call has plain values for the parameter `{path}`, which holds none"
                )));
            };
            descend(object, &segments(path), path, |object| {
                write_values(object, path, block, bytes)
            })
        }
        Source::Argument(index) => {
            let Some((_, argument)) = arguments.get(*index) else {
                return Err(Failure::Mismatch(format!(
                    "the call has no argument at the position {index} for the parameter `{path}`"
                )));
            };
            bind(object, binding, argument)
        }
    }
}

/// Binds the buffer of `argument` to the parameter of `binding`, which sits
/// in the object at the path of the binding without its last segment.
fn bind(
    root: &mut ShaderObject<'_>,
    binding: &Binding,
    argument: &CallArgument<'_>,
) -> std::result::Result<(), Failure> {
    let path = binding.path.as_str();
    let segments = segments(path);
    let Some((last, parents)) = segments.split_last() else {
        return Err(Failure::Missing(path.to_owned()));
    };
    descend(root, parents, path, |object| {
        let offset = object
            .offset_of(last)?
            .ok_or_else(|| Failure::Missing(path.to_owned()))?;
        match argument {
            Argument::Buffer(bound) => {
                let buffer = bound.handle.rhi_buffer().ok_or_else(|| {
                    Failure::Mismatch(format!(
                        "the buffer bound to the parameter `{path}` holds no buffer of slang-rhi"
                    ))
                })?;
                object.set_buffer(offset, buffer)?;
            }
            Argument::Texture(bound) => object.set_texture(offset, bound.handle.rhi_texture())?,
            Argument::Sampler(bound) => object.set_sampler(offset, bound.handle.rhi_sampler())?,
            Argument::Value(_) => {}
        }
        Ok(())
    })
}

/// Runs `f` on the object that the names `segments` lead to from `object`,
/// each the name of a constant buffer or parameter block of the object
/// before it, where `path` is the parameter path that the failures name.
fn descend<R>(
    object: &mut ShaderObject<'_>,
    segments: &[&str],
    path: &str,
    f: impl FnOnce(&mut ShaderObject<'_>) -> std::result::Result<R, Failure>,
) -> std::result::Result<R, Failure> {
    let Some((first, rest)) = segments.split_first() else {
        return f(object);
    };
    let offset = object
        .offset_of(first)?
        .ok_or_else(|| Failure::Missing(path.to_owned()))?;
    let mut inner = object.object(offset)?;
    descend(&mut inner, rest, path, f)
}

/// Writes the bytes of `block`, the uniform block at the parameter path
/// `prefix`, into `object`, the object of that block, as one write for each
/// top-level field of the object that holds a plain value of the block.
fn write_values(
    object: &mut ShaderObject<'_>,
    prefix: &str,
    block: &UniformBlock,
    bytes: &[u8],
) -> std::result::Result<(), Failure> {
    for field in top_level_fields(prefix, block).map_err(Failure::Missing)? {
        let path = if prefix.is_empty() {
            field.to_owned()
        } else {
            format!("{prefix}.{field}")
        };
        let offset = object
            .offset_of(field)?
            .ok_or_else(|| Failure::Missing(path.clone()))?;
        let start = usize::try_from(offset.uniform_offset()).ok();
        let size = usize::try_from(offset.uniform_size()).ok();
        let data = start
            .zip(size)
            .and_then(|(start, size)| bytes.get(start..start.checked_add(size)?))
            .ok_or_else(|| {
                Failure::Mismatch(format!(
                    "the parameter `{path}` takes {} bytes at the offset {}, past the {} bytes of its block",
                    offset.uniform_size(),
                    offset.uniform_offset(),
                    bytes.len()
                ))
            })?;
        object.set_data(offset, data)?;
    }
    Ok(())
}

/// Returns the names of the top-level fields of the object of the uniform
/// block `block` at the parameter path `prefix` that hold its plain values,
/// each once, in the order of the values, or the path of a value that does
/// not start with `prefix`.
fn top_level_fields<'b>(
    prefix: &str,
    block: &'b UniformBlock,
) -> std::result::Result<Vec<&'b str>, String> {
    let mut fields: Vec<&str> = Vec::new();
    for value in &block.values {
        let relative = if prefix.is_empty() {
            Some(value.path.as_str())
        } else {
            value
                .path
                .strip_prefix(prefix)
                .and_then(|rest| rest.strip_prefix('.'))
        };
        let Some(field) = relative.and_then(|relative| relative.split('.').next()) else {
            return Err(value.path.clone());
        };
        if !fields.contains(&field) {
            fields.push(field);
        }
    }
    Ok(fields)
}

/// Returns the names of a parameter path, which are none for the empty path
/// of the global plain values.
fn segments(path: &str) -> Vec<&str> {
    if path.is_empty() {
        Vec::new()
    } else {
        path.split('.').collect()
    }
}

#[cfg(test)]
mod tests;
