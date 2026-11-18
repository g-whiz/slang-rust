use crate::call::{Argument, CallArgument, Plan, Source, Subject};
use crate::error::{Error, ErrorKind};

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

/// Creates the uniform buffers and the bind group of each set that `plan`
/// binds, from the bind group `layouts` of a kernel or render pipeline and
/// the buffers, textures, and samplers of `arguments`.
pub(super) fn bind_groups(
    device: &::wgpu::Device,
    label: &str,
    plan: &Plan,
    layouts: &[::wgpu::BindGroupLayout],
    arguments: &[(&str, CallArgument<'_>)],
) -> Vec<::wgpu::BindGroup> {
    let uniforms: Vec<Vec<Option<::wgpu::Buffer>>> = plan
        .groups
        .iter()
        .map(|entries| {
            entries
                .iter()
                .map(|entry| match &entry.source {
                    Source::Uniform(bytes) => Some(uniform_buffer(device, label, bytes)),
                    Source::Argument(_) => None,
                })
                .collect()
        })
        .collect();
    plan.groups
        .iter()
        .zip(&uniforms)
        .zip(layouts)
        .map(|((entries, uniforms), layout)| {
            let entries: Vec<::wgpu::BindGroupEntry<'_>> = entries
                .iter()
                .zip(uniforms)
                .filter_map(|(entry, uniform)| {
                    let resource = match (&entry.source, uniform) {
                        (Source::Uniform(_), Some(buffer)) => buffer.as_entire_binding(),
                        (Source::Argument(index), _) => resource_of(arguments, *index)?,
                        (Source::Uniform(_), None) => return None,
                    };
                    Some(::wgpu::BindGroupEntry {
                        binding: entry.index,
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
