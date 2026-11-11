use std::borrow::Cow;
use std::fmt;
use std::num::NonZeroU64;
use std::sync::Arc;

use crate::Device;
use crate::device::scoped;
use crate::error::{Error, ErrorKind, Result};
use crate::layout::{Access, CompiledKernel, KernelLayout, Resource, Usage};

const LOAD: &str = "load a Slang module";

/// One compute kernel of a loaded [`Module`](crate::Module), ready to run on
/// its device.
///
/// A kernel is a function of the module marked `[shader("compute")]`, which
/// a dispatch runs on many threads of the GPU at once. Get one from
/// [`Module::kernel`](crate::Module::kernel) by its Slang name. Loading the
/// module already checked that the device accepts the kernel, so a kernel
/// always has its compiled form on the device. [`Kernel::call`] runs it.
///
/// Cloning a kernel is cheap and returns another handle to the same kernel. A
/// kernel keeps its [`Device`] alive, holds no object of the Slang compiler,
/// and is [`Send`] and [`Sync`], so threads can share it.
///
/// # Examples
///
/// See the example of [`Module::kernel`](crate::Module::kernel).
#[derive(Clone)]
pub struct Kernel {
    pub(crate) layout: Arc<KernelLayout>,
    pub(crate) device: Device,
    pub(crate) bind_group_layouts: Arc<[::wgpu::BindGroupLayout]>,
    pub(crate) pipeline: ::wgpu::ComputePipeline,
}

impl Kernel {
    /// Returns the name of the kernel, the name of its function in the Slang
    /// source.
    ///
    /// # Examples
    ///
    /// See the example of [`Module::kernel`](crate::Module::kernel).
    #[must_use]
    pub fn name(&self) -> &str {
        &self.layout.name
    }

    /// Returns the size of a thread group of the kernel along each of its
    /// three axes, the numbers of its `[numthreads(x, y, z)]` attribute.
    ///
    /// A dispatch runs the kernel in whole thread groups, so this is the
    /// granularity of the number of threads it runs.
    ///
    /// # Examples
    ///
    /// See the example of [`Module::kernel`](crate::Module::kernel).
    #[must_use]
    pub fn thread_group_size(&self) -> [u32; 3] {
        self.layout.thread_group_size
    }

    /// Creates the shader module, the bind group layouts, and the compute
    /// pipeline of one compiled kernel on the device.
    pub(crate) fn create(device: &Device, compiled: CompiledKernel) -> Result<Self> {
        let CompiledKernel { layout, spirv } = compiled;
        let wgpu_device = device.wgpu_device();
        let immediate_size = immediate_size(&layout, wgpu_device)?;
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
                bind_group_layout_entries(&layout)
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
            device: device.clone(),
            bind_group_layouts: bind_group_layouts.into(),
            pipeline,
            layout: Arc::new(layout),
        })
    }
}

impl fmt::Debug for Kernel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Kernel")
            .field("name", &self.name())
            .field("thread_group_size", &self.thread_group_size())
            .finish_non_exhaustive()
    }
}

/// Returns the kind of the error of a load when wgpu raises `error` while it
/// creates the layouts or the pipeline of a kernel.
fn pipeline_error_kind(error: &::wgpu::Error) -> ErrorKind {
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

/// Returns `size` as the size of the immediates of the kernel `name`, and
/// fails with [`ErrorKind::MissingFeature`] when the device lacks the feature
/// or `size` exceeds the device's `limit`.
fn checked_immediate_size(name: &str, has_feature: bool, limit: u32, size: u64) -> Result<u32> {
    if !has_feature {
        return Err(Error::new(ErrorKind::MissingFeature, LOAD).with_detail(format!(
            "the kernel `{name}` has `uniform` parameters, which need the feature IMMEDIATES that the device lacks"
        )));
    }
    match u32::try_from(size) {
        Ok(size) if size <= limit => Ok(size),
        _ => Err(Error::new(ErrorKind::MissingFeature, LOAD).with_detail(format!(
            "the `uniform` parameters of the kernel `{name}` take {size} bytes of immediates, and the device allows {limit}"
        ))),
    }
}

/// Returns the entries of the bind group layout of each set of the kernel,
/// from set 0 to the last set with a binding that the kernel uses.
///
/// A binding that the kernel does not use has no entry, because the SPIR-V
/// that Slang generates omits it, and a set between used sets gets an empty
/// layout.
fn bind_group_layout_entries(layout: &KernelLayout) -> Vec<Vec<::wgpu::BindGroupLayoutEntry>> {
    let used = || {
        layout
            .bindings
            .iter()
            .filter(|binding| binding.usage == Usage::Used)
    };
    let Some(last_set) = used().map(|binding| binding.set).max() else {
        return Vec::new();
    };
    (0..=last_set)
        .map(|set| {
            used()
                .filter(|binding| binding.set == set)
                .map(|binding| ::wgpu::BindGroupLayoutEntry {
                    binding: binding.index,
                    visibility: ::wgpu::ShaderStages::COMPUTE,
                    ty: binding_type(&binding.resource),
                    count: None,
                })
                .collect()
        })
        .collect()
}

/// Returns the wgpu binding type of a resource, by the mapping of Sections
/// 3.3 and 3.9 of `docs/design/slango.md`.
fn binding_type(resource: &Resource) -> ::wgpu::BindingType {
    let buffer = |ty, size| ::wgpu::BindingType::Buffer {
        ty,
        has_dynamic_offset: false,
        min_binding_size: NonZeroU64::new(size),
    };
    match resource {
        Resource::StorageBuffer { access, stride } => buffer(
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
