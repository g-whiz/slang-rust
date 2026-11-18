use std::fmt;
use std::marker::PhantomData;
use std::rc::Rc;

use crate::Device;
use crate::backend;
use crate::error::{Error, ErrorKind, Result};
use crate::marshal::VertexElement;

const CREATE: &str = "create a buffer";
const READ: &str = "read a buffer";

/// A buffer on the GPU of a [`Device`], which holds elements of type `T` for
/// kernels and draws to read and write.
///
/// A kernel reaches it as a `StructuredBuffer` or `RWStructuredBuffer` of
/// the matching Slang element type, which [`Element`](crate::Element) lists.
/// A buffer of any [`VertexElement`] can also hold the values of a vertex
/// attribute of a draw, which [`Draw::vertex`](crate::Draw::vertex) gives,
/// and a `Buffer<u32>` the indices of the vertices of triangles, which
/// [`Draw::submit_indexed`](crate::Draw::submit_indexed) reads. Create one
/// from the elements of a slice with [`Buffer::from_slice`], or filled with
/// zeros with [`Buffer::zeroed`], and copy its elements back with
/// [`Buffer::read`]. A buffer keeps its length for its whole life. The
/// elements are stored in little-endian byte order, which is the byte order
/// of every platform that this crate supports.
///
/// Cloning a buffer is cheap and returns another handle to the same buffer
/// on the GPU. A buffer keeps its [`Device`] alive.
///
/// A buffer is neither [`Send`] nor [`Sync`]: it stays on the thread that
/// created its [`Device`], because the crate keeps all the GPU objects of a
/// device on one thread whatever graphics library runs them. A program sends
/// results to other threads as plain data, such as the vector that
/// [`Buffer::read`] returns. This program does not compile, because a buffer
/// is not `Send`:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slango::Buffer<f32>>();
/// ```
///
/// This program does not compile either, because a buffer is not `Sync`:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slango::Buffer<f32>>();
/// ```
///
/// # Examples
///
/// See the example of [`Buffer::from_slice`].
#[derive(Clone)]
pub struct Buffer<T> {
    device: Device,
    inner: backend::Buffer,
    len: usize,
    element: PhantomData<fn() -> T>,
    identity: Rc<()>,
    thread_bound: PhantomData<*const ()>,
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
    /// element type has a size of zero bytes or the buffer would be larger
    /// than the device allows, and of kind [`ErrorKind::Backend`] when the
    /// backend reports an error, such as a lack of memory, while it creates
    /// the buffer.
    pub fn from_slice(device: &Device, elements: &[T]) -> Result<Self> {
        let size = checked_size(device, elements.len(), T::STRIDE)?;
        if elements.is_empty() {
            return Self::zeroed(device, 0);
        }
        let inner = backend::Buffer::from_elements(device.backend(), elements, size)?;
        Ok(Self::created(device, inner, elements.len()))
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
    /// returns, including [`ErrorKind::InvalidArgument`] when the element
    /// type has a size of zero bytes.
    pub fn zeroed(device: &Device, len: usize) -> Result<Self> {
        let size = checked_size(device, len, T::STRIDE)?;
        let inner = backend::Buffer::zeroed(device.backend(), size, T::STRIDE)?;
        Ok(Self::created(device, inner, len))
    }

    fn created(device: &Device, inner: backend::Buffer, len: usize) -> Self {
        Self {
            device: device.clone(),
            inner,
            len,
            element: PhantomData,
            identity: Rc::new(()),
            thread_bound: PhantomData,
        }
    }

    /// Copies the elements of the buffer back from the GPU into a vector.
    ///
    /// The call blocks until every kernel and draw that the device ran or
    /// queued before it has finished, so the vector holds all their results.
    /// It and [`Texture::read`](crate::Texture::read) are the ways in which a
    /// program waits for the GPU.
    ///
    /// # Examples
    ///
    /// See the example of [`Buffer::from_slice`].
    ///
    /// # Errors
    ///
    /// Returns an [`Error`] of kind [`ErrorKind::Backend`] when the backend
    /// reports an error while it waits for the work of the device or copies
    /// the buffer for reading.
    pub fn read(&self) -> Result<Vec<T>> {
        if self.len == 0 {
            return Ok(Vec::new());
        }
        let bytes = self.inner.read_bytes(self.device.backend())?;
        let elements = bytes
            .chunks_exact(T::STRIDE)
            .map(T::read_bytes)
            .collect::<Option<Vec<T>>>()
            .filter(|elements| elements.len() == self.len);
        elements.ok_or_else(|| {
            Error::new(ErrorKind::Backend, READ).with_detail(format!(
                "the backend read {} bytes, which are not {} elements of {} bytes",
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
    /// A call or a draw cannot use an empty buffer, but a program may create
    /// one, such as for a list of results that turned out empty.
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

    /// Returns a number that this buffer and its clones share and that no
    /// other buffer that is alive at the same time has.
    pub(crate) fn identity(&self) -> usize {
        Rc::as_ptr(&self.identity).addr()
    }

    pub(crate) fn backend(&self) -> &backend::Buffer {
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

/// Returns the size in bytes of `len` elements of `stride` bytes, and fails
/// with [`ErrorKind::InvalidArgument`] when it exceeds the largest buffer that
/// the device allows.
fn checked_size(device: &Device, len: usize, stride: usize) -> Result<u64> {
    let limit = device.backend().max_buffer_size();
    size_within(len, stride, limit)
}

fn size_within(len: usize, stride: usize, limit: u64) -> Result<u64> {
    if stride == 0 {
        return Err(Error::new(ErrorKind::InvalidArgument, CREATE)
            .with_detail("the elements have a size of 0 bytes".to_owned()));
    }
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
