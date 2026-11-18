use std::rc::Rc;

use slang_rhi::{BufferDesc, BufferUsage};

use super::device::Device;
use super::error::{Call, from_rhi};
use crate::error::{Error, ErrorKind, Result};
use crate::marshal::VertexElement;

const CREATE: &str = "create a buffer";
const READ: &str = "read a buffer";

/// The buffer of slang-rhi that holds the elements of a [`crate::Buffer`],
/// or none for a buffer of length zero, which slang-rhi cannot create and no
/// call binds.
#[derive(Clone)]
pub(crate) struct Buffer {
    inner: Option<Rc<slang_rhi::Buffer>>,
}

impl Buffer {
    /// Creates a buffer of `size` bytes that holds the bytes of `elements`,
    /// which are not empty.
    pub(crate) fn from_elements<T: VertexElement>(
        device: &Device,
        elements: &[T],
        size: u64,
    ) -> Result<Self> {
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        for element in elements {
            element.write_bytes(&mut bytes);
        }
        Self::create(device, size, T::STRIDE, &bytes)
    }

    /// Creates a buffer of `size` bytes, all zero, for elements of
    /// `element_size` bytes.
    pub(crate) fn zeroed(device: &Device, size: u64, element_size: usize) -> Result<Self> {
        if size == 0 {
            return Ok(Self { inner: None });
        }
        let len = usize::try_from(size).map_err(|error| {
            Error::new(ErrorKind::InvalidArgument, CREATE)
                .with_detail(format!("{size} bytes do not fit in the memory of the host"))
                .with_source(error)
        })?;
        Self::create(device, size, element_size, &vec![0; len])
    }

    fn create(device: &Device, size: u64, element_size: usize, bytes: &[u8]) -> Result<Self> {
        let element_size = u32::try_from(element_size).map_err(|error| {
            Error::new(ErrorKind::InvalidArgument, CREATE)
                .with_detail(format!(
                    "the elements have a size of {element_size} bytes, more than a buffer of slang-rhi takes"
                ))
                .with_source(error)
        })?;
        let desc = BufferDesc::new(size, usages()).with_element_size(element_size);
        let inner = device
            .rhi_device()
            .create_buffer(&desc, Some(bytes))
            .map_err(|error| from_rhi(CREATE, Call::Other, error))?;
        Ok(Self {
            inner: Some(Rc::new(inner)),
        })
    }

    /// Copies the bytes of the buffer back from the GPU, after the device has
    /// finished every earlier submission.
    pub(crate) fn read_bytes(&self, device: &Device) -> Result<Vec<u8>> {
        let Some(inner) = &self.inner else {
            return Ok(Vec::new());
        };
        device
            .queue()
            .wait_on_host()
            .map_err(|error| from_rhi(READ, Call::Other, error))?;
        let len = usize::try_from(inner.size()).map_err(|error| {
            Error::new(ErrorKind::Backend, READ)
                .with_detail(format!(
                    "{} bytes do not fit in the memory of the host",
                    inner.size()
                ))
                .with_source(error)
        })?;
        let mut bytes = vec![0; len];
        device
            .rhi_device()
            .read_buffer(inner, 0, &mut bytes)
            .map_err(|error| from_rhi(READ, Call::Other, error))?;
        Ok(bytes)
    }

    /// Returns the buffer of slang-rhi, which a buffer of length zero lacks.
    pub(super) fn rhi_buffer(&self) -> Option<&slang_rhi::Buffer> {
        self.inner.as_deref()
    }
}

/// Returns the uses of every buffer, which cover every use that a call and a
/// draw make of it.
fn usages() -> BufferUsage {
    BufferUsage::SHADER_RESOURCE
        | BufferUsage::UNORDERED_ACCESS
        | BufferUsage::VERTEX_BUFFER
        | BufferUsage::INDEX_BUFFER
        | BufferUsage::COPY_SOURCE
        | BufferUsage::COPY_DESTINATION
}
