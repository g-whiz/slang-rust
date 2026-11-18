use std::cell::{Cell, RefCell};
use std::io::{self, Write as _};
use std::mem::ManuallyDrop;
use std::process;

thread_local! {
    static IN_CALL: Cell<bool> = const { Cell::new(false) };
    static COLLECTED: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// Runs `call`, which makes exactly one call into slang-rhi, or into Slang's
/// reflection, the function `callee`, with the guards around a native call.
///
/// The call runs inside the Slang call guard of `slang-compiler`, because
/// slang-rhi calls Slang on the objects that `slang-compiler` wraps, and
/// inside [`abort_on_exception`], the innermost frame around the call.
///
/// # Panics
///
/// Panics before it calls slang-rhi when another call of this crate into
/// slang-rhi is in progress on this thread, which only the debug callback of a
/// device can cause. The panic aborts the process at the boundary of the
/// callback.
pub(crate) fn call<R>(callee: &'static str, call: impl FnOnce() -> R) -> R {
    let _entered = Entered::enter(callee);
    slang_compiler::with_slang_call_guard(|| abort_on_exception(callee, call))
}

/// Panics when a call of this crate into slang-rhi is in progress on this
/// thread, for code that takes a lock before its call into slang-rhi, so that
/// a reentrant call panics before it waits for a lock that its own thread
/// holds.
pub(crate) fn check_not_in_call(callee: &'static str) {
    if IN_CALL.with(Cell::get) {
        reentrant(callee);
    }
}

/// Runs `call` as [`call`] does, and returns its value together with every
/// message that slang-rhi reported from the source Slang through the debug
/// callback on this thread during the call.
pub(crate) fn call_collecting<R>(
    callee: &'static str,
    call: impl FnOnce() -> R,
) -> (R, Vec<String>) {
    COLLECTED.with(|collected| *collected.borrow_mut() = Some(Vec::new()));
    let value = self::call(callee, call);
    let messages = COLLECTED
        .with(|collected| collected.borrow_mut().take())
        .unwrap_or_default();
    (value, messages)
}

/// Keeps a message of Slang that the debug callback received on this thread,
/// when a call of [`call_collecting`] is in progress.
pub(crate) fn collect(message: &str) {
    COLLECTED
        .try_with(|collected| {
            if let Ok(mut collected) = collected.try_borrow_mut()
                && let Some(messages) = collected.as_mut()
            {
                messages.push(message.to_owned());
            }
        })
        .unwrap_or_default();
}

/// The mark of a call into slang-rhi in progress on this thread, which
/// clears the mark when it is dropped.
///
/// A leaked value leaves the mark set, so that every later call on the
/// thread panics, which is sound.
struct Entered {
    marker: std::marker::PhantomData<*const ()>,
}

impl Entered {
    fn enter(callee: &'static str) -> Self {
        if IN_CALL.with(|in_call| in_call.replace(true)) {
            reentrant(callee);
        }
        Self {
            marker: std::marker::PhantomData,
        }
    }
}

impl Drop for Entered {
    fn drop(&mut self) {
        IN_CALL.with(|in_call| in_call.set(false));
    }
}

#[cold]
#[expect(
    clippy::panic,
    reason = "a call from the debug callback into slang-rhi while slang-rhi is inside a call on the same thread is a bug in the callback, and slang-rhi documents no reentrancy, so the guard panics before the call reaches slang-rhi"
)]
fn reentrant(callee: &str) -> ! {
    panic!(
        "reentrant call: {callee} was called while a call of slang-rhi was in progress on this thread, so a debug callback must not use the objects of this crate"
    );
}

/// Runs `call`, which makes exactly one call into native code, the function
/// `callee`, and aborts the process when a C++ exception escapes that call.
///
/// The bindings declare every function of slang-rhi and of Slang with an
/// unwinding ABI, so an exception that escapes them unwinds into Rust instead
/// of being undefined behaviour. The value that this function holds during
/// the call is the first value with a destructor that the unwinding reaches,
/// as long as `call` creates no value with a destructor of its own, so the
/// process aborts before any other destructor of the crate or of its caller
/// runs and before any release can enter slang-rhi again. `call` captures its
/// arguments by reference or by copy, so that it owns nothing that a
/// destructor would drop.
fn abort_on_exception<R>(callee: &'static str, call: impl FnOnce() -> R) -> R {
    let armed = AbortOnUnwind { callee };
    let value = call();
    let _disarmed = ManuallyDrop::new(armed);
    value
}

/// The value whose destructor runs only when an exception unwinds through
/// [`abort_on_exception`].
struct AbortOnUnwind {
    callee: &'static str,
}

impl Drop for AbortOnUnwind {
    fn drop(&mut self) {
        exception_escaped(self.callee);
    }
}

#[cold]
#[inline(never)]
fn exception_escaped(callee: &str) -> ! {
    #[expect(
        clippy::let_underscore_must_use,
        let_underscore_drop,
        reason = "the process aborts next whether or not the message reaches standard error, and nothing can report a failure to write it"
    )]
    let _ = io::stderr().write_fmt(format_args!(
        "slang-rhi: a C++ exception escaped from the native call {callee}, which slang-rhi does not catch, so the process aborts\n"
    ));
    process::abort()
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    use super::{Entered, call, call_collecting, collect};

    #[test]
    fn call_returns_the_value_of_the_call_and_clears_its_mark() {
        assert_eq!(call("Test::first", || 7), 7);
        assert_eq!(call("Test::second", || 8), 8);
    }

    #[test]
    fn a_call_while_another_is_in_progress_panics_and_the_mark_clears_after() {
        let outer = Entered::enter("Test::outer");
        let outcome = catch_unwind(AssertUnwindSafe(|| drop(Entered::enter("Test::inner"))));
        let payload = outcome.expect_err("the inner call panics");
        let message = payload
            .downcast_ref::<String>()
            .cloned()
            .unwrap_or_default();
        assert!(
            message.starts_with("reentrant call: Test::inner "),
            "{message}"
        );
        drop(outer);
        assert_eq!(call("Test::after", || 1), 1);
    }

    #[test]
    fn call_collecting_keeps_only_the_messages_of_its_own_call() {
        collect("before");
        let (value, messages) = call_collecting("Test::collect", || {
            collect("first");
            collect("second");
            3
        });
        collect("after");
        assert_eq!(value, 3);
        assert_eq!(messages, ["first", "second"]);
        let ((), none) = call_collecting("Test::quiet", || ());
        assert_eq!(none, Vec::<String>::new());
    }
}
