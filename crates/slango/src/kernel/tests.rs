use std::num::NonZeroU64;

use super::{Kernel, bind_group_layout_entries, checked_immediate_size, pipeline_error_kind};
use crate::Module;
use crate::error::ErrorKind;
use crate::layout::{Access, Binding, ElementType, KernelLayout, Resource, UniformBlock, Usage};

fn storage(set: u32, index: u32, usage: Usage, access: Access, stride: u64) -> Binding {
    Binding {
        path: format!("buffer_{set}_{index}"),
        set,
        index,
        usage,
        resource: Resource::StorageBuffer {
            access,
            stride,
            element: ElementType::Composite,
        },
    }
}

fn uniform(set: u32, index: u32, usage: Usage, size: u64) -> Binding {
    Binding {
        path: format!("uniform_{set}_{index}"),
        set,
        index,
        usage,
        resource: Resource::UniformBuffer(UniformBlock {
            size,
            values: Vec::new(),
        }),
    }
}

fn kernel(bindings: Vec<Binding>) -> KernelLayout {
    KernelLayout {
        name: "run".to_owned(),
        thread_group_size: [64, 1, 1],
        bindings,
        immediates: None,
        structures: Vec::new(),
    }
}

fn entry(index: u32, ty: wgpu::BufferBindingType, size: u64) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding: index,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty,
            has_dynamic_offset: false,
            min_binding_size: NonZeroU64::new(size),
        },
        count: None,
    }
}

const READ_ONLY: wgpu::BufferBindingType = wgpu::BufferBindingType::Storage { read_only: true };
const READ_WRITE: wgpu::BufferBindingType = wgpu::BufferBindingType::Storage { read_only: false };
const UNIFORM: wgpu::BufferBindingType = wgpu::BufferBindingType::Uniform;

#[test]
fn storage_and_uniform_buffers_become_entries_with_their_stride_and_size() {
    let layout = kernel(vec![
        uniform(0, 0, Usage::Used, 48),
        storage(0, 1, Usage::Used, Access::ReadOnly, 4),
        storage(0, 2, Usage::Used, Access::ReadWrite, 16),
    ]);
    assert_eq!(
        bind_group_layout_entries(&layout),
        [vec![
            entry(0, UNIFORM, 48),
            entry(1, READ_ONLY, 4),
            entry(2, READ_WRITE, 16),
        ]]
    );
}

#[test]
fn unused_bindings_have_no_entry() {
    let layout = kernel(vec![
        storage(0, 0, Usage::Unused, Access::ReadWrite, 4),
        storage(0, 1, Usage::Used, Access::ReadWrite, 4),
        uniform(0, 2, Usage::Unused, 16),
    ]);
    assert_eq!(
        bind_group_layout_entries(&layout),
        [vec![entry(1, READ_WRITE, 4)]]
    );
}

#[test]
fn each_set_up_to_the_last_used_one_gets_a_layout_and_a_gap_is_empty() {
    let layout = kernel(vec![
        storage(0, 0, Usage::Used, Access::ReadWrite, 4),
        uniform(2, 0, Usage::Used, 16),
        storage(2, 1, Usage::Used, Access::ReadOnly, 4),
        storage(3, 0, Usage::Unused, Access::ReadOnly, 4),
    ]);
    assert_eq!(
        bind_group_layout_entries(&layout),
        [
            vec![entry(0, READ_WRITE, 4)],
            Vec::new(),
            vec![entry(0, UNIFORM, 16), entry(1, READ_ONLY, 4)],
        ]
    );
}

#[test]
fn a_kernel_without_used_bindings_has_no_bind_group_layout() {
    assert_eq!(
        bind_group_layout_entries(&kernel(Vec::new())),
        Vec::<Vec<wgpu::BindGroupLayoutEntry>>::new()
    );
    let unused = kernel(vec![storage(1, 0, Usage::Unused, Access::ReadWrite, 4)]);
    assert_eq!(
        bind_group_layout_entries(&unused),
        Vec::<Vec<wgpu::BindGroupLayoutEntry>>::new()
    );
}

fn assert_shared<T: Clone + Send + Sync + 'static>() {}

#[test]
fn modules_and_kernels_are_clone_send_and_sync() {
    assert_shared::<Module>();
    assert_shared::<Kernel>();
}

const LIMIT: u32 = 256;

#[test]
fn immediates_up_to_the_limit_of_the_device_fit() {
    assert_eq!(
        checked_immediate_size("run", true, LIMIT, 4).expect("4 bytes fit"),
        4
    );
    assert_eq!(
        checked_immediate_size("run", true, LIMIT, u64::from(LIMIT)).expect("the limit fits"),
        LIMIT
    );
}

#[test]
fn immediates_over_the_limit_of_the_device_are_a_missing_feature() {
    for size in [u64::from(LIMIT) + 1, 4096] {
        let error = checked_immediate_size("run", true, LIMIT, size).expect_err("over the limit");
        assert_eq!(error.kind(), ErrorKind::MissingFeature);
        assert_eq!(
            error.to_string(),
            format!(
                "cannot load a Slang module: the `uniform` parameters of the kernel `run` take {size} bytes of immediates, and the device allows 256"
            )
        );
    }
}

#[test]
fn immediates_that_do_not_fit_u32_are_a_missing_feature() {
    let size = u64::from(u32::MAX) + 1;
    let error = checked_immediate_size("run", true, u32::MAX, size).expect_err("over u32");
    assert_eq!(error.kind(), ErrorKind::MissingFeature);
    assert!(error.to_string().contains(&size.to_string()), "{error}");
}

#[test]
fn immediates_on_a_device_without_the_feature_are_a_missing_feature() {
    let error = checked_immediate_size("run", false, LIMIT, 4).expect_err("no feature");
    assert_eq!(error.kind(), ErrorKind::MissingFeature);
    assert_eq!(
        error.to_string(),
        "cannot load a Slang module: the kernel `run` has `uniform` parameters, which need the feature IMMEDIATES that the device lacks"
    );
}

fn source() -> wgpu::ErrorSource {
    Box::new(std::io::Error::other("the backend failed"))
}

#[test]
fn an_internal_error_of_the_pipeline_is_an_unsupported_shader() {
    let error = wgpu::Error::Internal {
        source: source(),
        description: "internal".to_owned(),
    };
    assert_eq!(pipeline_error_kind(&error), ErrorKind::UnsupportedShader);
}

#[test]
fn a_validation_or_memory_error_of_the_pipeline_is_a_backend_error() {
    let validation = wgpu::Error::Validation {
        source: source(),
        description: "validation".to_owned(),
    };
    assert_eq!(pipeline_error_kind(&validation), ErrorKind::Backend);
    let out_of_memory = wgpu::Error::OutOfMemory { source: source() };
    assert_eq!(pipeline_error_kind(&out_of_memory), ErrorKind::Backend);
}

#[test]
fn a_texture_and_a_sampler_become_filterable_float_and_filtering_entries() {
    let binding = |index, resource| Binding {
        path: format!("resource_{index}"),
        set: 0,
        index,
        usage: Usage::Used,
        resource,
    };
    let layout = kernel(vec![
        binding(0, Resource::SampledTexture),
        binding(1, Resource::Sampler),
    ]);
    let visible = |binding, ty| wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty,
        count: None,
    };
    assert_eq!(
        bind_group_layout_entries(&layout),
        [vec![
            visible(
                0,
                wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                }
            ),
            visible(
                1,
                wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering)
            ),
        ]]
    );
}
