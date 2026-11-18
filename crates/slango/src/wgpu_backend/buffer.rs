use std::sync::mpsc;

use super::device::{Device, scoped};
use crate::error::{Error, ErrorKind, Result};
use crate::marshal::VertexElement;

const CREATE: &str = "create a buffer";
const READ: &str = "read a buffer";

/// A wgpu buffer that holds the elements of a [`crate::Buffer`].
#[derive(Clone)]
pub(crate) struct Buffer {
    inner: ::wgpu::Buffer,
}

impl Buffer {
    /// Creates a buffer of `size` bytes that holds the bytes of `elements`,
    /// which are not empty.
    pub(crate) fn from_elements<T: VertexElement>(
        device: &Device,
        elements: &[T],
        size: u64,
    ) -> Result<Self> {
        let wgpu_device = device.wgpu_device();
        let ((buffer, mapped), error) = scoped(wgpu_device, || {
            let buffer = wgpu_device.create_buffer(&::wgpu::BufferDescriptor {
                label: Some("slango buffer"),
                size: size.next_multiple_of(::wgpu::COPY_BUFFER_ALIGNMENT),
                usage: usages(),
                mapped_at_creation: true,
            });
            let mapped = buffer
                .get_mapped_range_mut(..)
                .map(|mut view| write_elements(elements, view.slice(..)));
            buffer.unmap();
            (buffer, mapped)
        });
        let buffer = Self::created(buffer, error)?;
        mapped.map_err(|error| Error::new(ErrorKind::Backend, CREATE).with_source(error))?;
        Ok(buffer)
    }

    /// Creates a buffer of `size` bytes, all zero.
    pub(crate) fn zeroed(device: &Device, size: u64) -> Result<Self> {
        let wgpu_device = device.wgpu_device();
        let (buffer, error) = scoped(wgpu_device, || {
            wgpu_device.create_buffer(&::wgpu::BufferDescriptor {
                label: Some("slango buffer"),
                size,
                usage: usages(),
                mapped_at_creation: false,
            })
        });
        Self::created(buffer, error)
    }

    fn created(inner: ::wgpu::Buffer, error: Option<::wgpu::Error>) -> Result<Self> {
        if let Some(error) = error {
            return Err(Error::new(ErrorKind::Backend, CREATE).with_source(error));
        }
        Ok(Self { inner })
    }

    /// Copies the bytes of the buffer back from the GPU.
    pub(crate) fn read_bytes(&self, device: &Device) -> Result<Vec<u8>> {
        let size = self.inner.size();
        read_back(device, READ, size, |encoder, staging| {
            encoder.copy_buffer_to_buffer(&self.inner, 0, staging, 0, size);
        })
    }

    pub(super) fn wgpu_buffer(&self) -> &::wgpu::Buffer {
        &self.inner
    }
}

/// Copies `size` bytes from the GPU into a vector through a staging buffer
/// into which `record` records the copy, and blocks until the device has
/// finished every earlier submission and the copy.
///
/// Every error of wgpu is returned with the operation `operation` and the
/// kind [`ErrorKind::Backend`].
pub(super) fn read_back(
    device: &Device,
    operation: &'static str,
    size: u64,
    record: impl FnOnce(&mut ::wgpu::CommandEncoder, &::wgpu::Buffer),
) -> Result<Vec<u8>> {
    let wgpu_device = device.wgpu_device();
    let (staging, error) = scoped(wgpu_device, || {
        let staging = wgpu_device.create_buffer(&::wgpu::BufferDescriptor {
            label: Some("slango staging buffer"),
            size,
            usage: ::wgpu::BufferUsages::MAP_READ | ::wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder =
            wgpu_device.create_command_encoder(&::wgpu::CommandEncoderDescriptor::default());
        record(&mut encoder, &staging);
        device.wgpu_queue().submit([encoder.finish()]);
        staging
    });
    if let Some(error) = error {
        return Err(Error::new(ErrorKind::Backend, operation).with_source(error));
    }
    let (sender, receiver) = mpsc::channel();
    let ((), error) = scoped(wgpu_device, || {
        staging.map_async(::wgpu::MapMode::Read, .., move |result| {
            #[expect(
                clippy::let_underscore_must_use,
                reason = "a send fails only when `read_back` has already returned, so no one waits for the result"
            )]
            let _ = sender.send(result);
        });
    });
    if let Some(error) = error {
        return Err(Error::new(ErrorKind::Backend, operation).with_source(error));
    }
    wgpu_device
        .poll(::wgpu::PollType::wait_indefinitely())
        .map_err(|error| Error::new(ErrorKind::Backend, operation).with_source(error))?;
    match receiver.recv() {
        Ok(Ok(())) => {}
        Ok(Err(error)) => {
            return Err(Error::new(ErrorKind::Backend, operation).with_source(error));
        }
        Err(error) => {
            return Err(Error::new(ErrorKind::Backend, operation)
                .with_detail("wgpu finished waiting before it mapped the copy".to_owned())
                .with_source(error));
        }
    }
    let view = staging
        .get_mapped_range(..)
        .map_err(|error| Error::new(ErrorKind::Backend, operation).with_source(error))?;
    Ok(view.to_vec())
}

/// The number of bytes of elements that [`write_elements`] converts before
/// it copies them into the buffer.
const UPLOAD_CHUNK: usize = 64 * 1024;

/// Writes the bytes of `elements` to the start of `out`, a chunk of
/// [`UPLOAD_CHUNK`] bytes at a time, so that an upload holds no second copy
/// of all its elements on the host.
fn write_elements<T: VertexElement>(elements: &[T], mut out: ::wgpu::WriteOnly<'_, [u8]>) {
    let mut chunk = Vec::with_capacity(UPLOAD_CHUNK);
    let mut offset = 0;
    for group in elements.chunks((UPLOAD_CHUNK / T::STRIDE).max(1)) {
        chunk.clear();
        for element in group {
            element.write_bytes(&mut chunk);
        }
        let end = offset + chunk.len();
        out.slice(offset..end).copy_from_slice(&chunk);
        offset = end;
    }
}

fn usages() -> ::wgpu::BufferUsages {
    ::wgpu::BufferUsages::STORAGE
        | ::wgpu::BufferUsages::VERTEX
        | ::wgpu::BufferUsages::INDEX
        | ::wgpu::BufferUsages::COPY_SRC
        | ::wgpu::BufferUsages::COPY_DST
}
