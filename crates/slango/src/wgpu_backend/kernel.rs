use std::borrow::Cow;
use std::num::NonZeroU64;
use std::sync::Arc;

use super::device::{Device, scoped};
use super::submit::bind_groups;
use crate::call::{CallArgument, Plan};
use crate::error::{Error, ErrorKind, Result};
use crate::kernel::checked_immediate_size;
use crate::layout::{Access, Binding, KernelLayout, Resource, Usage};

const LOAD: &str = "load a Slang module";
const CALL: &str = "call a kernel";

/// The wgpu bind group layouts and compute pipeline of a
/// [`crate::Kernel`].
#[derive(Clone)]
pub(crate) struct Kernel {
    bind_group_layouts: Arc<[::wgpu::BindGroupLayout]>,
    pipeline: ::wgpu::ComputePipeline,
}

impl Kernel {
    /// Creates the shader module, the bind group layouts, and the compute
    /// pipeline of one compiled kernel on the device.
    pub(crate) fn create(device: &Device, layout: &KernelLayout, spirv: Vec<u32>) -> Result<Self> {
        let wgpu_device = device.wgpu_device();
        let immediate_size = immediate_size(layout, wgpu_device)?;
        let name = layout.name.as_str();
        let (module, error) = scoped(wgpu_device, || {
            wgpu_device.create_shader_module(::wgpu::ShaderModuleDescriptor {
                label: Some(name),
                source: ::wgpu::ShaderSource::SpirV(Cow::Owned(spirv)),
            })
        });
        if let Some(error) = error {
            return Err(Error::new(ErrorKind::UnsupportedShader, LOAD)
                .with_detail(format!(
                    "the backend rejected the SPIR-V of the kernel `{name}`"
                ))
                .with_source(error));
        }
        let ((bind_group_layouts, pipeline), error) = scoped(wgpu_device, || {
            let bind_group_layouts: Vec<::wgpu::BindGroupLayout> =
                bind_group_layout_entries(layout)
                    .iter()
                    .map(|entries| {
                        wgpu_device.create_bind_group_layout(&::wgpu::BindGroupLayoutDescriptor {
                            label: Some(name),
                            entries,
                        })
                    })
                    .collect();
            let layout_refs: Vec<Option<&::wgpu::BindGroupLayout>> =
                bind_group_layouts.iter().map(Some).collect();
            let pipeline_layout =
                wgpu_device.create_pipeline_layout(&::wgpu::PipelineLayoutDescriptor {
                    label: Some(name),
                    bind_group_layouts: &layout_refs,
                    immediate_size,
                });
            let pipeline =
                wgpu_device.create_compute_pipeline(&::wgpu::ComputePipelineDescriptor {
                    label: Some(name),
                    layout: Some(&pipeline_layout),
                    module: &module,
                    entry_point: Some(name),
                    compilation_options: ::wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                });
            (bind_group_layouts, pipeline)
        });
        if let Some(error) = error {
            return Err(Error::new(pipeline_error_kind(&error), LOAD)
                .with_detail(format!(
                    "the backend failed to create the pipeline of the kernel `{name}`"
                ))
                .with_source(error));
        }
        Ok(Self {
            bind_group_layouts: bind_group_layouts.into(),
            pipeline,
        })
    }

    /// Records one dispatch of `groups` thread groups with the bindings of
    /// `plan` and submits it, and reports every error that wgpu raises.
    pub(crate) fn dispatch(
        &self,
        device: &Device,
        name: &str,
        plan: &Plan,
        arguments: &[(&str, CallArgument<'_>)],
        groups: [u32; 3],
    ) -> Result<()> {
        let ((), error) = scoped(device.wgpu_device(), || {
            self.submit(device, name, plan, arguments, groups);
        });
        match error {
            Some(error) => Err(Error::new(ErrorKind::Backend, CALL)
                .with_detail(format!("wgpu rejected the dispatch of the kernel `{name}`"))
                .with_source(error)),
            None => Ok(()),
        }
    }

    fn submit(
        &self,
        device: &Device,
        name: &str,
        plan: &Plan,
        arguments: &[(&str, CallArgument<'_>)],
        groups: [u32; 3],
    ) {
        let wgpu_device = device.wgpu_device();
        let bind_groups = bind_groups(wgpu_device, name, plan, &self.bind_group_layouts, arguments);
        let mut encoder = wgpu_device
            .create_command_encoder(&::wgpu::CommandEncoderDescriptor { label: Some(name) });
        {
            let mut pass = encoder.begin_compute_pass(&::wgpu::ComputePassDescriptor {
                label: Some(name),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.pipeline);
            for (set, group) in (0_u32..).zip(&bind_groups) {
                pass.set_bind_group(set, group, &[]);
            }
            if !plan.immediates.is_empty() {
                pass.set_immediates(0, &plan.immediates);
            }
            pass.dispatch_workgroups(groups[0], groups[1], groups[2]);
        }
        device.wgpu_queue().submit([encoder.finish()]);
    }
}

/// Returns the kind of the error when wgpu raises `error` while it creates
/// the layouts or the compute pipeline of a kernel, or the layouts of a
/// render pipeline.
pub(super) fn pipeline_error_kind(error: &::wgpu::Error) -> ErrorKind {
    match error {
        ::wgpu::Error::Internal { .. } => ErrorKind::UnsupportedShader,
        ::wgpu::Error::OutOfMemory { .. } | ::wgpu::Error::Validation { .. } => ErrorKind::Backend,
    }
}

/// Returns the size of the immediates of the kernel's pipeline layout, and
/// fails with [`ErrorKind::MissingFeature`] when the device cannot pass them.
fn immediate_size(layout: &KernelLayout, device: &::wgpu::Device) -> Result<u32> {
    let Some(immediates) = &layout.immediates else {
        return Ok(0);
    };
    checked_immediate_size(
        &layout.name,
        device.features().contains(::wgpu::Features::IMMEDIATES),
        device.limits().max_immediate_size,
        immediates.size,
    )
}

/// Returns the entries of the bind group layout of each set of the kernel,
/// from set 0 to the last set with a binding that the kernel uses.
///
/// A binding that the kernel does not use has no entry, because the SPIR-V
/// that Slang generates omits it, and a set between used sets gets an empty
/// layout.
fn bind_group_layout_entries(layout: &KernelLayout) -> Vec<Vec<::wgpu::BindGroupLayoutEntry>> {
    let used: Vec<(&Binding, ::wgpu::ShaderStages)> = layout
        .bindings
        .iter()
        .filter(|binding| binding.usage == Usage::Used)
        .map(|binding| (binding, ::wgpu::ShaderStages::COMPUTE))
        .collect();
    layout_entries(&used)
}

/// Returns the entries of the bind group layout of each set, from set 0 to
/// the last set of the `used` bindings, each visible to its stages, with an
/// empty layout for a set that holds no used binding.
pub(super) fn layout_entries(
    used: &[(&Binding, ::wgpu::ShaderStages)],
) -> Vec<Vec<::wgpu::BindGroupLayoutEntry>> {
    let Some(last_set) = used.iter().map(|(binding, _)| binding.set).max() else {
        return Vec::new();
    };
    (0..=last_set)
        .map(|set| {
            used.iter()
                .filter(|(binding, _)| binding.set == set)
                .map(|&(binding, visibility)| ::wgpu::BindGroupLayoutEntry {
                    binding: binding.index,
                    visibility,
                    ty: binding_type(&binding.resource),
                    count: None,
                })
                .collect()
        })
        .collect()
}

/// Returns the wgpu binding type of a resource.
fn binding_type(resource: &Resource) -> ::wgpu::BindingType {
    let buffer = |ty, size| ::wgpu::BindingType::Buffer {
        ty,
        has_dynamic_offset: false,
        min_binding_size: NonZeroU64::new(size),
    };
    match resource {
        Resource::StorageBuffer { access, stride, .. } => buffer(
            ::wgpu::BufferBindingType::Storage {
                read_only: *access == Access::ReadOnly,
            },
            *stride,
        ),
        Resource::UniformBuffer(block) => buffer(::wgpu::BufferBindingType::Uniform, block.size),
        Resource::SampledTexture => ::wgpu::BindingType::Texture {
            sample_type: ::wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: ::wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        Resource::Sampler => ::wgpu::BindingType::Sampler(::wgpu::SamplerBindingType::Filtering),
    }
}

#[cfg(test)]
mod tests;
