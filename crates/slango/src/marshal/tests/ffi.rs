use crate::layout::{self, Resource, Source};
use crate::marshal::Element;

const BUFFERS: &str = r#"
StructuredBuffer<float> floats;
StructuredBuffer<int> ints;
StructuredBuffer<uint> uints;
StructuredBuffer<float4> float4s;
StructuredBuffer<int4> int4s;
RWStructuredBuffer<uint4> uint4s;

[shader("compute")]
[numthreads(1, 1, 1)]
void run(uint3 id : SV_DispatchThreadID)
{
    uint4s[id.x] = uint4(uint(floats[id.x]), uint(ints[id.x]), uints[id.x], uint(float4s[id.x].x))
        + uint4(int4s[id.x]);
}
"#;

fn stride_of(kernel: &layout::KernelLayout, path: &str) -> u64 {
    kernel
        .bindings
        .iter()
        .find_map(|binding| match &binding.resource {
            Resource::StorageBuffer { stride, .. } if binding.path == path => Some(*stride),
            Resource::StorageBuffer { .. }
            | Resource::UniformBuffer(_)
            | Resource::SampledTexture
            | Resource::Sampler => None,
        })
        .unwrap_or_else(|| panic!("a storage buffer `{path}` in {kernel:#?}"))
}

fn stride<T: Element>() -> u64 {
    u64::try_from(T::STRIDE).expect("a stride fits in 64 bits")
}

#[test]
fn every_element_type_has_the_reflected_stride_of_its_slang_type() {
    let kernels = layout::compile(&Source {
        name: "buffers",
        path: "buffers.slang",
        text: BUFFERS,
        search_path: None,
    })
    .expect("the module compiles")
    .kernels;
    let [kernel] = kernels.as_slice() else {
        panic!("one kernel in {kernels:#?}");
    };
    let kernel = &kernel.layout;
    assert_eq!(stride_of(kernel, "floats"), stride::<f32>());
    assert_eq!(stride_of(kernel, "ints"), stride::<i32>());
    assert_eq!(stride_of(kernel, "uints"), stride::<u32>());
    assert_eq!(stride_of(kernel, "float4s"), stride::<[f32; 4]>());
    assert_eq!(stride_of(kernel, "int4s"), stride::<[i32; 4]>());
    assert_eq!(stride_of(kernel, "uint4s"), stride::<[u32; 4]>());
}
