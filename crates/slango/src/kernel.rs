use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

use crate::Device;
use crate::backend;
use crate::layout::KernelLayout;

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
/// kernel keeps its [`Device`] alive. With the feature `wgpu`, it holds no
/// object of the Slang compiler, and with the feature `rhi`, it keeps the
/// Slang session of its load alive.
///
/// A kernel is neither [`Send`] nor [`Sync`]: it stays on the thread that
/// created its [`Device`], because the crate keeps all the GPU objects of a
/// device on one thread whatever graphics library runs them. A program sends
/// results to other threads as plain data, such as the vector that
/// [`Buffer::read`](crate::Buffer::read) returns. This program does not
/// compile, because a kernel is not `Send`:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slango::Kernel>();
/// ```
///
/// This program does not compile either, because a kernel is not `Sync`:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slango::Kernel>();
/// ```
///
/// # Examples
///
/// See the example of [`Module::kernel`](crate::Module::kernel).
#[derive(Clone)]
pub struct Kernel {
    pub(crate) layout: Arc<KernelLayout>,
    pub(crate) device: Device,
    pub(crate) inner: backend::Kernel,
    thread_bound: PhantomData<*const ()>,
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

    /// Wraps the kernel `inner` that the backend created on `device` from
    /// the layout snapshot `layout`.
    pub(crate) fn from_backend(
        device: &Device,
        layout: KernelLayout,
        inner: backend::Kernel,
    ) -> Self {
        Self {
            device: device.clone(),
            inner,
            layout: Arc::new(layout),
            thread_bound: PhantomData,
        }
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

#[cfg(test)]
mod tests;
