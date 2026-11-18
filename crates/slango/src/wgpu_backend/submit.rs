use crate::call::{Argument, CallArgument, Plan, Source, Subject};
use crate::error::{Error, ErrorKind};
use crate::layout::Binding;

/// Returns an error of [`ErrorKind::Backend`] for an error that wgpu
/// reported while it prepared or submitted the work of `subject`.
pub(super) fn rejected(subject: Subject<'_>, error: ::wgpu::Error) -> Error {
    Error::new(ErrorKind::Backend, subject.operation())
        .with_detail(format!(
            "wgpu rejected the {} of {subject}",
            subject.request()
        ))
        .with_source(error)
}

/// Returns the entries of `plan` of each set, each as its binding index in
/// the set and what it holds, from set 0 to the last set with a used
/// binding, where `bindings` are the bindings of the kernel or render
/// pipeline of the plan, ordered by set and binding.
///
/// These are the sets of the bind group layouts that the load or the
/// creation of the render pipeline created, and a set between used sets
/// holds no entry.
pub(super) fn group_entries<'p, P: AsRef<Binding>>(
    bindings: &[P],
    plan: &'p Plan,
) -> Vec<Vec<(u32, &'p Source)>> {
    let placed: Vec<(u32, u32, &Source)> = plan
        .entries
        .iter()
        .filter_map(|entry| {
            let binding = bindings.get(entry.binding)?.as_ref();
            Some((binding.set, binding.index, &entry.source))
        })
        .collect();
    let Some(last_set) = placed.iter().map(|(set, _, _)| *set).max() else {
        return Vec::new();
    };
    (0..=last_set)
        .map(|set| {
            placed
                .iter()
                .filter(|(of, _, _)| *of == set)
                .map(|&(_, index, source)| (index, source))
                .collect()
        })
        .collect()
}

/// Creates the uniform buffers and the bind group of each set that `plan`
/// binds, from the `bindings` and the bind group `layouts` of a kernel or
/// render pipeline and the buffers, textures, and samplers of `arguments`.
pub(super) fn bind_groups<P: AsRef<Binding>>(
    device: &::wgpu::Device,
    label: &str,
    bindings: &[P],
    plan: &Plan,
    layouts: &[::wgpu::BindGroupLayout],
    arguments: &[(&str, CallArgument<'_>)],
) -> Vec<::wgpu::BindGroup> {
    let groups = group_entries(bindings, plan);
    let uniforms: Vec<Vec<Option<::wgpu::Buffer>>> = groups
        .iter()
        .map(|entries| {
            entries
                .iter()
                .map(|(_, source)| match source {
                    Source::Uniform(bytes) => Some(uniform_buffer(device, label, bytes)),
                    Source::Argument(_) => None,
                })
                .collect()
        })
        .collect();
    groups
        .iter()
        .zip(&uniforms)
        .zip(layouts)
        .map(|((entries, uniforms), layout)| {
            let entries: Vec<::wgpu::BindGroupEntry<'_>> = entries
                .iter()
                .zip(uniforms)
                .filter_map(|(&(index, source), uniform)| {
                    let resource = match (source, uniform) {
                        (Source::Uniform(_), Some(buffer)) => buffer.as_entire_binding(),
                        (Source::Argument(argument), _) => resource_of(arguments, *argument)?,
                        (Source::Uniform(_), None) => return None,
                    };
                    Some(::wgpu::BindGroupEntry {
                        binding: index,
                        resource,
                    })
                })
                .collect();
            device.create_bind_group(&::wgpu::BindGroupDescriptor {
                label: Some(label),
                layout,
                entries: &entries,
            })
        })
        .collect()
}

/// Creates a uniform buffer that holds `bytes`.
///
/// A buffer that wgpu fails to create cannot be mapped, and wgpu reports the
/// failure in the error scope around the work, so the bytes are then not
/// written.
#[expect(
    clippy::expect_used,
    reason = "slango builds for 64-bit targets only, where every usize fits in a u64"
)]
fn uniform_buffer(device: &::wgpu::Device, label: &str, bytes: &[u8]) -> ::wgpu::Buffer {
    let size = u64::try_from(bytes.len())
        .expect("internal error: a usize fits in a u64 on the 64-bit targets of slango")
        .next_multiple_of(::wgpu::COPY_BUFFER_ALIGNMENT)
        .max(::wgpu::COPY_BUFFER_ALIGNMENT);
    let buffer = device.create_buffer(&::wgpu::BufferDescriptor {
        label: Some(label),
        size,
        usage: ::wgpu::BufferUsages::UNIFORM,
        mapped_at_creation: true,
    });
    if let Ok(mut view) = buffer.get_mapped_range_mut(..) {
        view.slice(..bytes.len()).copy_from_slice(bytes);
    }
    buffer.unmap();
    buffer
}

fn resource_of<'a>(
    arguments: &[(&str, CallArgument<'a>)],
    index: usize,
) -> Option<::wgpu::BindingResource<'a>> {
    match arguments.get(index) {
        Some((_, Argument::Buffer(bound))) => Some(bound.handle.wgpu_buffer().as_entire_binding()),
        Some((_, Argument::Texture(bound))) => Some(::wgpu::BindingResource::TextureView(
            bound.handle.wgpu_view(),
        )),
        Some((_, Argument::Sampler(bound))) => Some(::wgpu::BindingResource::Sampler(
            bound.handle.wgpu_sampler(),
        )),
        Some((_, Argument::Value(_))) | None => None,
    }
}

#[cfg(test)]
mod tests;
