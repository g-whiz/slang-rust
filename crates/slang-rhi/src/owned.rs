use std::fmt;
use std::ptr::NonNull;

use slang_rhi_sys::{
    IBuffer, ICommandBuffer, ICommandEncoder, ICommandQueue, IComputePipeline, IDevice,
    IShaderObject, IShaderProgram,
};

use crate::guard;

/// The counting methods of a generated interface type of slang-rhi.
///
/// # Safety
///
/// An implementer guarantees that `Self` is a generated interface type of
/// slang-rhi that derives from `ISlangUnknown`, and that each function calls
/// the method of the same name through the vtable of the object that `this`
/// points to. Breaking either condition is undefined behaviour, because
/// [`Owned`] then counts through the wrong vtable entry.
pub(crate) unsafe trait RhiInterface {
    /// Calls `addRef` on the object.
    ///
    /// # Safety
    ///
    /// `this` points to a live object that implements `Self`. Breaking it is
    /// undefined behaviour.
    unsafe fn add_ref(this: *mut Self) -> u32;

    /// Calls `release` on the object.
    ///
    /// # Safety
    ///
    /// `this` points to a live object that implements `Self`, and the caller
    /// owns the reference that the call releases. Breaking either is
    /// undefined behaviour.
    unsafe fn release(this: *mut Self) -> u32;
}

macro_rules! rhi_interfaces {
    ($($interface:ident),+ $(,)?) => {$(
        // SAFETY: `$interface` is a generated interface type of slang-rhi
        // that derives from `ISlangUnknown`, and its generated thunks call the
        // counting methods through the object's own vtable.
        unsafe impl RhiInterface for $interface {
            unsafe fn add_ref(this: *mut Self) -> u32 {
                guard::call(concat!(stringify!($interface), "::addRef"), || {
                    // SAFETY: the caller guarantees that `this` points to a
                    // live object that implements the interface.
                    unsafe { $interface::addRef(this) }
                })
            }

            unsafe fn release(this: *mut Self) -> u32 {
                guard::call(concat!(stringify!($interface), "::release"), || {
                    // SAFETY: the caller guarantees that `this` points to a
                    // live object that implements the interface and owns the
                    // reference that this call releases.
                    unsafe { $interface::release(this) }
                })
            }
        }
    )+};
}

rhi_interfaces!(
    IDevice,
    IShaderProgram,
    IComputePipeline,
    IBuffer,
    ICommandQueue,
    ICommandEncoder,
    ICommandBuffer,
    IShaderObject,
);

/// An owning interface pointer, which holds exactly one reference to an
/// object of slang-rhi.
///
/// It is the only code of the crate that adds or releases a reference. Its
/// invariant is that `raw` points to a live object that implements `T`, and
/// that the value owns one reference to it.
pub(crate) struct Owned<T: RhiInterface> {
    raw: NonNull<T>,
}

impl<T: RhiInterface> Owned<T> {
    /// Adopts the reference of an output pointer that a call of slang-rhi
    /// wrote, or returns `None` for a null pointer.
    ///
    /// # Safety
    ///
    /// `raw` is null, or it points to a live object that implements `T`, the
    /// call that wrote it added one reference for the caller, and the caller
    /// transfers that reference to the returned value. Breaking any of them is
    /// undefined behaviour.
    pub(crate) unsafe fn adopt(raw: *mut T) -> Option<Self> {
        NonNull::new(raw).map(|raw| Self { raw })
    }

    /// Returns the pointer without changing the count.
    pub(crate) fn as_ptr(&self) -> *mut T {
        self.raw.as_ptr()
    }

    /// Returns the pointer without changing the count.
    pub(crate) fn as_non_null(&self) -> NonNull<T> {
        self.raw
    }
}

impl<T: RhiInterface> Clone for Owned<T> {
    fn clone(&self) -> Self {
        // SAFETY: `self.raw` points to a live object that implements `T`,
        // because `self` owns a reference to it, and the returned value owns
        // the reference that this adds.
        unsafe { T::add_ref(self.raw.as_ptr()) };
        Self { raw: self.raw }
    }
}

impl<T: RhiInterface> Drop for Owned<T> {
    fn drop(&mut self) {
        // SAFETY: `self` owns one reference to a live object that implements
        // `T`, and `drop` runs once, so the reference is released once.
        unsafe { T::release(self.raw.as_ptr()) };
    }
}

impl<T: RhiInterface> fmt::Debug for Owned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Owned").field(&self.raw).finish()
    }
}

#[cfg(test)]
mod tests {
    use slang_rhi_sys::{IBuffer, IShaderObject};

    use super::Owned;
    use crate::fake::{Event, FakeBuffer, FakeShaderObject, Log};

    #[test]
    fn adopt_of_null_is_none_and_counts_nothing() {
        // SAFETY: a null pointer meets the condition of `adopt`.
        assert!(unsafe { Owned::<IBuffer>::adopt(std::ptr::null_mut()) }.is_none());
    }

    #[test]
    fn adopt_then_drop_releases_the_one_reference_and_frees_the_object() {
        let log = Log::new();
        let buffer = FakeBuffer::create(&log, "buffer");
        // SAFETY: the new fake buffer carries one reference, which `adopt`
        // takes.
        let owned = unsafe { Owned::<IBuffer>::adopt(buffer.as_ptr()) }.expect("non-null");
        drop(owned);
        assert_eq!(
            log.events(),
            [Event::release("buffer", 0), Event::freed("buffer")]
        );
    }

    #[test]
    fn clone_adds_a_reference_and_each_drop_releases_one() {
        let log = Log::new();
        let raw = FakeShaderObject::create(&log, "object");
        // SAFETY: the new fake object carries one reference, which `adopt`
        // takes.
        let first = unsafe { Owned::<IShaderObject>::adopt(raw.as_ptr()) }.expect("non-null");
        let second = first.clone();
        assert_eq!(first.as_ptr(), second.as_ptr());
        drop(first);
        drop(second);
        assert_eq!(
            log.events(),
            [
                Event::add_ref("object", 2),
                Event::release("object", 1),
                Event::release("object", 0),
                Event::freed("object"),
            ]
        );
    }
}
