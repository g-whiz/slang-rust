use slang_rhi_sys::{HeapAllocDesc, SubmitDesc, kInvalidCUDAStream};

#[test]
fn the_cuda_stream_constant_is_the_address_with_every_bit_set() {
    assert_eq!(kInvalidCUDAStream.addr(), usize::MAX);
}

#[test]
fn the_default_submit_desc_has_the_values_of_the_header() {
    let desc = SubmitDesc::default();
    assert!(desc.commandBuffers.is_null());
    assert_eq!(desc.commandBufferCount, 0);
    assert!(desc.waitFences.is_null());
    assert!(desc.waitFenceValues.is_null());
    assert_eq!(desc.waitFenceCount, 0);
    assert!(desc.signalFences.is_null());
    assert!(desc.signalFenceValues.is_null());
    assert_eq!(desc.signalFenceCount, 0);
    assert_eq!(desc.cudaStream, kInvalidCUDAStream);
}

#[test]
fn the_default_heap_alloc_desc_has_the_values_of_the_header() {
    let desc = HeapAllocDesc::default();
    assert_eq!(desc.size, 0);
    assert_eq!(desc.alignment, 0);
    assert_eq!(desc.stream, kInvalidCUDAStream);
}
