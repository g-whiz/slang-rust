use std::fmt;
use std::marker::PhantomData;
use std::sync::mpsc;

use ::wgpu::util::DeviceExt as _;

use crate::Device;
use crate::device::scoped;
use crate::error::{Error, ErrorKind, Result};
use crate::marshal::VertexElement;

const CREATE: &str = "create a buffer";
const READ: &str = "read a buffer";

/// A buffer on the GPU of a [`Device`], which holds elements of type `T` for
/// kernels to read and write.
///
/// A kernel reaches it as a `StructuredBuffer` or `RWStructuredBuffer` of
/// the matching Slang element type, which [`Element`](crate::Element) lists.
/// A buffer of any [`VertexElement`] can also hold the values of a vertex
/// attribute of a draw, which [`Draw::vertex`](crate::Draw::vertex) gives,
/// and a `Buffer<u32>` the indices of the vertices of triangles, which
/// [`Draw::submit_indexed`](crate::Draw::submit_indexed) reads. Create one from
/// the elements of a slice with [`Buffer::from_slice`], or filled with zeros
/// with [`Buffer::zeroed`], and copy its elements back with
/// [`Buffer::read`]. A buffer keeps its length for its whole life. The
/// elements are stored in little-endian byte order, which is the byte order
/// of every platform that this crate supports.
///
/// Cloning a buffer is cheap and returns another handle to the same buffer
/// on the GPU. A buffer keeps its [`Device`] alive and is [`Send`] and
/// [`Sync`], so threads can share it.
///
/// # Examples
///
/// See the example of [`Buffer::from_slice`].
#[derive(Clone)]
pub struct Buffer<T> {
    device: Device,
    inner: ::wgpu::Buffer,
    len: usize,
    element: PhantomData<fn() -> T>,
}

impl<T: VertexElement> Buffer<T> {
    /// Creates a buffer on the device that holds a copy of the elements of a
    /// slice.
    ///
    /// # Examples
    ///
    /// This example creates a buffer of four numbers and reads them back. It
    /// needs a GPU driver or a software driver at run time, so it is compiled
    /// but not run:
    ///
    /// ```no_run
    /// use slango::{Buffer, Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
    /// assert_eq!(data.len(), 4);
    /// assert_eq!(data.read()?, [1.0, 2.0, 3.0, 4.0]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::InvalidArgument`] when the
    /// buffer would be larger than the device allows, and of kind
    /// [`ErrorKind::Backend`] when wgpu reports an error, such as a lack of
    /// memory, while it creates the buffer.
    pub fn from_slice(device: &Device, elements: &[T]) -> Result<Self> {
        checked_size(device, elements.len(), T::STRIDE)?;
        let mut contents = Vec::with_capacity(elements.len().saturating_mul(T::STRIDE));
        for element in elements {
            element.write_bytes(&mut contents);
        }
        let wgpu_device = device.wgpu_device();
        let (buffer, error) = scoped(wgpu_device, || {
            wgpu_device.create_buffer_init(&::wgpu::util::BufferInitDescriptor {
                label: Some("slango buffer"),
                contents: &contents,
                usage: usages(),
            })
        });
        Self::created(device, buffer, elements.len(), error)
    }

    /// Creates a buffer on the device that holds `len` elements whose bytes
    /// are all zero.
    ///
    /// # Examples
    ///
    /// This example creates a buffer of eight vectors of zeros. It needs a
    /// GPU driver or a software driver at run time, so it is compiled but not
    /// run:
    ///
    /// ```no_run
    /// use slango::{Buffer, Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let results = Buffer::<[f32; 4]>::zeroed(&device, 8)?;
    /// assert_eq!(results.read()?, [[0.0; 4]; 8]);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of the kinds that [`Buffer::from_slice`]
    /// returns.
    pub fn zeroed(device: &Device, len: usize) -> Result<Self> {
        let size = checked_size(device, len, T::STRIDE)?;
        let wgpu_device = device.wgpu_device();
        let (buffer, error) = scoped(wgpu_device, || {
            wgpu_device.create_buffer(&::wgpu::BufferDescriptor {
                label: Some("slango buffer"),
                size,
                usage: usages(),
                mapped_at_creation: false,
            })
        });
        Self::created(device, buffer, len, error)
    }

    fn created(
        device: &Device,
        inner: ::wgpu::Buffer,
        len: usize,
        error: Option<::wgpu::Error>,
    ) -> Result<Self> {
        if let Some(error) = error {
            return Err(Error::new(ErrorKind::Backend, CREATE).with_source(error));
        }
        Ok(Self {
            device: device.clone(),
            inner,
            len,
            element: PhantomData,
        })
    }

    /// Copies the elements of the buffer back from the GPU into a vector.
    ///
    /// The call blocks until every kernel that the device ran or queued
    /// before it has finished, so the vector holds all their results. It and
    /// [`Texture::read`](crate::Texture::read) are the ways in which a
    /// program waits for the GPU.
    ///
    /// # Examples
    ///
    /// See the example of [`Buffer::from_slice`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::Backend`] when wgpu reports
    /// an error while it copies the buffer or maps the copy for reading.
    pub fn read(&self) -> Result<Vec<T>> {
        if self.len == 0 {
            return Ok(Vec::new());
        }
        let size = self.inner.size();
        let bytes = read_back(&self.device, READ, size, |encoder, staging| {
            encoder.copy_buffer_to_buffer(&self.inner, 0, staging, 0, size);
        })?;
        let elements = bytes
            .chunks_exact(T::STRIDE)
            .map(T::read_bytes)
            .collect::<Option<Vec<T>>>()
            .filter(|elements| elements.len() == self.len);
        elements.ok_or_else(|| {
            Error::new(ErrorKind::Backend, READ).with_detail(format!(
                "wgpu mapped {} bytes, which are not {} elements of {} bytes",
                bytes.len(),
                self.len,
                T::STRIDE
            ))
        })
    }

    /// Returns the number of elements of the buffer.
    ///
    /// # Examples
    ///
    /// See the example of [`Buffer::from_slice`].
    #[must_use]
    pub fn len(&self) -> usize {
        self.len
    }

    /// Returns whether the buffer has no elements.
    ///
    /// A kernel cannot bind an empty buffer, but a program may create one,
    /// such as for a list of results that turned out empty.
    ///
    /// # Examples
    ///
    /// This example creates an empty buffer. It needs a GPU driver or a
    /// software driver at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use slango::{Buffer, Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::default())?;
    /// let empty = Buffer::<u32>::from_slice(&device, &[])?;
    /// assert!(empty.is_empty());
    /// assert_eq!(empty.read()?, []);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns whether the buffer was created on `device` or a clone of it,
    /// and not on another [`Device`], even one built from the same wgpu device.
    pub(crate) fn is_on(&self, device: &Device) -> bool {
        self.device.is_same(device)
    }

    pub(crate) fn wgpu_buffer(&self) -> &::wgpu::Buffer {
        &self.inner
    }
}

impl<T> fmt::Debug for Buffer<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Buffer")
            .field("element", &std::any::type_name::<T>())
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

/// Copies `size` bytes from the GPU into a vector through a staging buffer
/// into which `record` records the copy, and blocks until the device has
/// finished every earlier submission and the copy.
///
/// Every error of wgpu is returned with the operation `operation` and the
/// kind [`ErrorKind::Backend`].
pub(crate) fn read_back(
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

fn usages() -> ::wgpu::BufferUsages {
    ::wgpu::BufferUsages::STORAGE
        | ::wgpu::BufferUsages::VERTEX
        | ::wgpu::BufferUsages::INDEX
        | ::wgpu::BufferUsages::COPY_SRC
        | ::wgpu::BufferUsages::COPY_DST
}

/// Returns the size in bytes of `len` elements of `stride` bytes, and fails
/// with [`ErrorKind::InvalidArgument`] when it exceeds the largest buffer that
/// the device allows.
fn checked_size(device: &Device, len: usize, stride: usize) -> Result<u64> {
    let limit = device.wgpu_device().limits().max_buffer_size;
    size_within(len, stride, limit)
}

fn size_within(len: usize, stride: usize, limit: u64) -> Result<u64> {
    match len
        .checked_mul(stride)
        .and_then(|size| u64::try_from(size).ok())
    {
        Some(size) if size <= limit => Ok(size),
        _ => Err(
            Error::new(ErrorKind::InvalidArgument, CREATE).with_detail(format!(
                "{len} elements of {stride} bytes exceed the largest buffer that the device allows, {limit} bytes"
            )),
        ),
    }
}

#[cfg(test)]
mod tests;
