use std::ptr::NonNull;
use std::sync::{Mutex, PoisonError};

use slang_rhi_sys::{IRHI, rhiGetInstance};

use crate::error::{Error, Operation, Result};
use crate::guard;

/// The one lock of the crate around every call through the process-wide
/// instance of slang-rhi, which documents no rule for the use of the
/// instance from several threads.
///
/// The lock guards no data, so a panic while it is held leaves nothing
/// invalid, and a poisoned lock is recovered.
static INSTANCE: Mutex<()> = Mutex::new(());

/// Runs `f` for the public function `function` of the crate with the
/// process-wide instance of slang-rhi while the lock of the crate is held, so
/// that the calls of this crate through the instance run one at a time.
///
/// The crate never destroys the instance, which lives until the process
/// exits.
///
/// # Panics
///
/// Panics before it waits for the lock when a call of this crate into
/// slang-rhi is in progress on this thread, which only the debug callback of
/// a device can cause.
pub(crate) fn with_instance<R>(
    operation: Operation,
    function: &'static str,
    f: impl FnOnce(NonNull<IRHI>) -> Result<R>,
) -> Result<R> {
    guard::check_not_in_call(function);
    let _locked = INSTANCE.lock().unwrap_or_else(PoisonError::into_inner);
    let rhi = guard::call("rhiGetInstance", || {
        // SAFETY: `rhiGetInstance` takes no argument and returns the instance
        // of the process, which it creates once under its own mutex.
        unsafe {
            #[expect(
                ffi_unwind_calls,
                reason = "slang-rhi is C++, and the call runs inside the abort of an escaping exception"
            )]
            rhiGetInstance()
        }
    });
    let rhi = NonNull::new(rhi).ok_or_else(|| Error::null(operation))?;
    f(rhi)
}
