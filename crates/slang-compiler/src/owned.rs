use std::ffi::c_void;
use std::fmt;
use std::mem::ManuallyDrop;
use std::ptr::{self, NonNull};

use slang_compiler_sys::{
    IComponentType, IEntryPoint, IGlobalSession, IMetadata, IModule, ISession, ISlangBlob,
    ITypeConformance, Interface, SlangResult,
};

use crate::error::{Diagnostics, Operation, Result};
use crate::guard::{self, Release};
use crate::mapping;

/// The counting methods of a generated interface type of the Slang API.
///
/// # Safety
///
/// An implementer guarantees that `Self` is a generated interface type, and
/// that each function calls the method of the same name of `ISlangUnknown`
/// through the vtable of the object that `this` points to. Breaking either
/// condition is undefined behaviour, because [`Owned`] then counts through the
/// wrong vtable entry.
pub(crate) unsafe trait ComInterface: Interface {
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
    /// owns the reference that the call releases. Breaking either is undefined
    /// behaviour.
    unsafe fn release(this: *mut Self) -> u32;

    /// Calls `queryInterface` on the object.
    ///
    /// # Safety
    ///
    /// `this` points to a live object that implements `Self`, `uuid` points
    /// to a live identifier, and `out` is a live out-pointer. Breaking any of
    /// them is undefined behaviour.
    unsafe fn query_interface(
        this: *mut Self,
        uuid: *const slang_compiler_sys::SlangUUID,
        out: *mut *mut c_void,
    ) -> SlangResult;
}

macro_rules! com_interfaces {
    ($($interface:ident),+ $(,)?) => {$(
        // SAFETY: `$interface` is a generated interface type, and its
        // generated thunks call the methods of `ISlangUnknown` through the
        // object's own vtable (COM-9).
        unsafe impl ComInterface for $interface {
            unsafe fn add_ref(this: *mut Self) -> u32 {
                // SAFETY: the caller guarantees that `this` points to a live
                // object that implements the interface.
                unsafe {
                    guard::abort_on_exception(concat!(stringify!($interface), "::addRef"), || {
                        $interface::addRef(this)
                    })
                }
            }

            unsafe fn release(this: *mut Self) -> u32 {
                // SAFETY: the caller guarantees that `this` points to a live
                // object that implements the interface and owns the reference
                // that this call releases.
                unsafe {
                    guard::abort_on_exception(concat!(stringify!($interface), "::release"), || {
                        $interface::release(this)
                    })
                }
            }

            unsafe fn query_interface(
                this: *mut Self,
                uuid: *const slang_compiler_sys::SlangUUID,
                out: *mut *mut c_void,
            ) -> SlangResult {
                // SAFETY: the caller guarantees that `this` points to a live
                // object that implements the interface, that `uuid` is a live
                // identifier, and that `out` is a live out-pointer.
                unsafe {
                    guard::abort_on_exception(
                        concat!(stringify!($interface), "::queryInterface"),
                        || $interface::queryInterface(this, uuid, out),
                    )
                }
            }
        }
    )+};
}

com_interfaces!(
    ISlangBlob,
    IGlobalSession,
    ISession,
    IModule,
    IEntryPoint,
    IComponentType,
    ITypeConformance,
    IMetadata,
);

/// An owning interface pointer, which holds exactly one reference to a Slang
/// object (SLC-2).
///
/// It is the only code of the crate that adds or releases a reference.
/// Section 11.1 of `docs/design/slang-compiler.md` states its invariant: `raw`
/// points to a live object of the pinned Slang build that implements `T`, and
/// the value owns one reference to it.
pub(crate) struct Owned<T: ComInterface> {
    raw: NonNull<T>,
}

impl<T: ComInterface> Owned<T> {
    /// Adopts the reference that a Slang call returned.
    ///
    /// # Safety
    ///
    /// `raw` points to a live object of the pinned Slang build that implements
    /// `T`, the caller owns one reference to it, which Section 5.3 of
    /// `docs/design/slang-compiler.md` marks as a new reference, and the caller
    /// transfers that reference to the returned value. Breaking any of them is
    /// undefined behaviour.
    pub(crate) unsafe fn adopt(raw: NonNull<T>) -> Self {
        Self { raw }
    }

    /// Adopts the reference of an output pointer that a Slang call wrote, or
    /// returns `None` for a null pointer.
    ///
    /// # Safety
    ///
    /// `raw` is null or meets the conditions of [`Owned::adopt`]. Breaking
    /// this is undefined behaviour.
    pub(crate) unsafe fn adopt_output(raw: *mut T) -> Option<Self> {
        NonNull::new(raw).map(|raw| {
            // SAFETY: `raw` is non-null, and the caller guarantees the
            // conditions of `adopt` for it.
            unsafe { Self::adopt(raw) }
        })
    }

    /// Adds a reference to an object that a Slang call returned without one,
    /// and owns that reference.
    ///
    /// # Safety
    ///
    /// `raw` points to a live object of the pinned Slang build that implements
    /// `T`, which Section 5.3 of `docs/design/slang-compiler.md` marks as a
    /// borrowed return, and the object that owns it is still alive. Breaking
    /// either is undefined behaviour.
    pub(crate) unsafe fn from_borrowed(raw: NonNull<T>) -> Self {
        // SAFETY: the caller guarantees that `raw` points to a live object that
        // implements `T`, and the reference that this adds is the one that the
        // returned value owns.
        unsafe { T::add_ref(raw.as_ptr()) };
        Self { raw }
    }

    /// Returns the pointer without changing the count.
    pub(crate) fn as_ptr(&self) -> *mut T {
        self.raw.as_ptr()
    }

    /// Returns the pointer without changing the count.
    pub(crate) fn as_non_null(&self) -> NonNull<T> {
        self.raw
    }

    /// Gives up the reference without releasing it.
    pub(crate) fn into_raw(self) -> NonNull<T> {
        ManuallyDrop::new(self).raw
    }

    /// Asks the object for the interface `U` through `queryInterface`, with
    /// the identifier of `U` that the binding generator produced (COM-3,
    /// COM-4), and owns the reference that a successful call adds.
    ///
    /// A failure is reported as an error of `operation`. The caller raises the
    /// Slang call guard (SLC-28).
    pub(crate) fn cast<U: ComInterface>(&self, operation: Operation) -> Result<Owned<U>> {
        let mut out: *mut c_void = ptr::null_mut();
        let uuid = U::UUID;
        // SAFETY: `self.raw` points to a live object that implements `T`,
        // because `self` owns a reference to it, `uuid` is a live identifier,
        // and `out` is a live out-pointer.
        let result =
            unsafe { T::query_interface(self.raw.as_ptr(), &raw const uuid, &raw mut out) };
        mapping::check(operation, result, Diagnostics::default())?;
        let raw = NonNull::new(out.cast::<U>())
            .ok_or_else(|| mapping::null(operation, Diagnostics::default()))?;
        // SAFETY: `queryInterface` succeeded with the identifier of `U` and
        // wrote a non-null pointer, which is a new reference to a live object
        // that implements `U` (Section 5.3 of `docs/design/slang-compiler.md`).
        Ok(unsafe { Owned::adopt(raw) })
    }
}

impl<T: ComInterface> Clone for Owned<T> {
    fn clone(&self) -> Self {
        // SAFETY: `self.raw` points to a live object that implements `T`,
        // because `self` owns a reference to it, and the returned value owns
        // the reference that this adds. `addRef` runs without the Slang call
        // guard, as rule 1 of SLC-28 allows.
        unsafe { T::add_ref(self.raw.as_ptr()) };
        Self { raw: self.raw }
    }
}

impl<T: ComInterface> Drop for Owned<T> {
    fn drop(&mut self) {
        // SAFETY: `self` owns one reference to a live object that implements
        // `T`, `release_erased::<T>` releases exactly one reference of such an
        // object, and `drop` runs once, so the reference passes to the guard
        // once. The object belongs to a graph of this thread, because no
        // wrapper is `Send` (SLC-21).
        let release = unsafe { Release::new(self.raw.cast::<c_void>(), release_erased::<T>) };
        guard::release(release);
    }
}

impl<T: ComInterface> fmt::Debug for Owned<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Owned").field(&self.raw).finish()
    }
}

/// Releases one reference of an object that implements `T`.
///
/// # Safety
///
/// `raw` points to a live object that implements `T`, and the caller owns the
/// reference that this releases. Breaking either is undefined behaviour.
unsafe fn release_erased<T: ComInterface>(raw: NonNull<c_void>) {
    // SAFETY: the caller guarantees that `raw` points to a live object that
    // implements `T` and owns the reference that this releases.
    unsafe { T::release(raw.cast::<T>().as_ptr()) };
}

#[cfg(test)]
mod tests {
    use slang_compiler_sys::{IModule, ISlangBlob, SlangResult};

    use super::Owned;
    use crate::fake::{Event, FakeBlob, Log};
    use crate::{ErrorKind, Operation};

    #[test]
    fn adopt_then_drop_releases_the_one_reference() {
        let log = Log::new();
        let blob = FakeBlob::create(&log, "blob", b"bytes");
        // SAFETY: `blob` carries the one reference of a new fake blob.
        let owned = unsafe { Owned::<ISlangBlob>::adopt(blob) };
        drop(owned);
        assert_eq!(
            log.events(),
            [Event::release("blob", 0), Event::freed("blob")]
        );
    }

    #[test]
    fn from_borrowed_adds_a_reference_that_outlives_the_owner() {
        let log = Log::new();
        let blob = FakeBlob::create(&log, "blob", b"bytes");
        // SAFETY: `blob` is live, and the test keeps its own reference until
        // after the wrapper exists.
        let owned = unsafe { Owned::<ISlangBlob>::from_borrowed(blob) };
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeBlob::release_raw(blob) };
        assert_eq!(
            log.events(),
            [Event::add_ref("blob", 2), Event::release("blob", 1)]
        );
        drop(owned);
        assert_eq!(
            log.events(),
            [
                Event::add_ref("blob", 2),
                Event::release("blob", 1),
                Event::release("blob", 0),
                Event::freed("blob"),
            ]
        );
    }

    #[test]
    fn clone_adds_a_reference_and_each_drop_releases_one() {
        let log = Log::new();
        // SAFETY: the new fake blob carries one reference, which `adopt` takes.
        let first = unsafe { Owned::<ISlangBlob>::adopt(FakeBlob::create(&log, "blob", b"")) };
        let second = first.clone();
        assert_eq!(first.as_ptr(), second.as_ptr());
        drop(first);
        drop(second);
        assert_eq!(
            log.events(),
            [
                Event::add_ref("blob", 2),
                Event::release("blob", 1),
                Event::release("blob", 0),
                Event::freed("blob"),
            ]
        );
    }

    #[test]
    fn into_raw_gives_up_the_reference_without_releasing_it() {
        let log = Log::new();
        // SAFETY: the new fake blob carries one reference, which `adopt` takes.
        let owned = unsafe { Owned::<ISlangBlob>::adopt(FakeBlob::create(&log, "blob", b"")) };
        let raw = owned.into_raw();
        assert_eq!(log.events(), []);
        // SAFETY: `into_raw` gave up the one reference, which `adopt` takes back.
        drop(unsafe { Owned::<ISlangBlob>::adopt(raw) });
        assert_eq!(
            log.events(),
            [Event::release("blob", 0), Event::freed("blob")]
        );
    }

    #[test]
    fn cast_to_an_interface_that_the_object_lacks_fails_without_a_reference() {
        let log = Log::new();
        // SAFETY: the new fake blob carries one reference, which `adopt` takes.
        let blob = unsafe { Owned::<ISlangBlob>::adopt(FakeBlob::create(&log, "blob", b"")) };
        let error = blob
            .cast::<IModule>(Operation::Compose)
            .expect_err("a blob is not a module");
        assert_eq!(error.kind(), ErrorKind::Unsupported);
        assert_eq!(error.operation(), Operation::Compose);
        assert_eq!(error.code(), Some(SlangResult::SLANG_E_NO_INTERFACE.0));
        drop(blob);
        assert_eq!(
            log.events(),
            [
                Event::query("blob", false),
                Event::release("blob", 0),
                Event::freed("blob"),
            ]
        );
    }

    #[test]
    fn cast_that_succeeds_with_a_null_output_is_an_internal_error() {
        let log = Log::new();
        let raw = FakeBlob::create(&log, "blob", b"");
        FakeBlob::answer_queries_with_null(raw);
        // SAFETY: the new fake blob carries one reference, which `adopt` takes.
        let blob = unsafe { Owned::<ISlangBlob>::adopt(raw) };
        let error = blob
            .cast::<ISlangBlob>(Operation::Compose)
            .expect_err("a null output fails");
        assert_eq!(error.kind(), ErrorKind::Internal);
        assert_eq!(error.operation(), Operation::Compose);
        assert_eq!(error.code(), None);
        drop(blob);
        assert_eq!(
            log.events(),
            [
                Event::query("blob", true),
                Event::release("blob", 0),
                Event::freed("blob"),
            ]
        );
    }
}
