use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::ptr::NonNull;

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
    static LIVE_GLOBAL_SESSIONS: Cell<usize> = const { Cell::new(0) };
    static PENDING: RefCell<VecDeque<Pending>> = const { RefCell::new(VecDeque::new()) };
}

/// One reference of a Slang object, with the function that releases it.
pub(crate) struct Release {
    raw: NonNull<c_void>,
    release: unsafe fn(NonNull<c_void>),
}

impl Release {
    /// Takes over one reference for the guard to release.
    ///
    /// # Safety
    ///
    /// `raw` points to a live Slang object, the caller transfers one reference
    /// to it, `release` releases exactly that reference when it is called once
    /// with `raw`, and the object belongs to a graph of the current thread.
    /// Breaking any of them is undefined behaviour.
    pub(crate) unsafe fn new(raw: NonNull<c_void>, release: unsafe fn(NonNull<c_void>)) -> Self {
        Self { raw, release }
    }

    fn run(self) {
        // SAFETY: `Release::new` required that `raw` carries one reference that
        // `release` releases, and `self` is consumed, so the reference is
        // released once, on the thread of its graph.
        unsafe { (self.release)(self.raw) };
    }
}

enum Pending {
    Release(Release),
    EndGlobalSession,
}

/// A raised Slang call guard, which lowers the guard when it is dropped and
/// then runs the deferred work once the guard is down (SLC-28).
pub(crate) struct Raised {
    thread: PhantomData<*const ()>,
}

/// Raises the Slang call guard of this thread without checking it, for the
/// calls into Slang that the crate makes inside a public method or a release.
pub(crate) fn raise() -> Raised {
    DEPTH.with(|depth| depth.set(depth.get() + 1));
    Raised {
        thread: PhantomData,
    }
}

impl Drop for Raised {
    fn drop(&mut self) {
        let remaining = DEPTH.with(|depth| {
            let remaining = depth.get().saturating_sub(1);
            depth.set(remaining);
            remaining
        });
        if remaining == 0 {
            drain();
        }
    }
}

fn is_raised() -> bool {
    DEPTH.with(|depth| depth.get() > 0)
}

fn drain() {
    while let Some(pending) = PENDING
        .try_with(|queue| queue.borrow_mut().pop_front())
        .ok()
        .flatten()
    {
        match pending {
            Pending::Release(release) => {
                DEPTH.with(|depth| depth.set(depth.get() + 1));
                release.run();
                DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
            }
            Pending::EndGlobalSession => end_global_session(),
        }
    }
}

/// Queues work until the guard falls back to zero, and returns whether it did.
///
/// When the queue of this thread is already destroyed, which happens only
/// while the thread exits, the work is dropped. A dropped release leaks its
/// reference, which is sound (UNS-4), where a release during a call is not.
fn defer(pending: Pending) -> bool {
    PENDING
        .try_with(|queue| queue.borrow_mut().push_back(pending))
        .is_ok()
}

/// Runs `call` with the Slang call guard raised, for the public method named
/// `method`, which calls Slang inside `call`.
///
/// # Panics
///
/// Panics before it runs `call` when the guard is already raised on this
/// thread, which is a reentrant call (SLC-28).
pub(crate) fn call<R>(method: &str, call: impl FnOnce() -> R) -> R {
    if is_raised() {
        reentrant(method);
    }
    let _raised = raise();
    call()
}

#[cold]
#[expect(
    clippy::panic,
    reason = "a reentrant call is a bug in the caller's callback, and SLC-28 of docs/design/slang-compiler.md requires this panic before any call into Slang"
)]
fn reentrant(method: &str) -> ! {
    panic!(
        "reentrant call: {method} was called while a call of slang-compiler into Slang was in progress on this thread, but Slang is not reentrant, so a FileSystem implementation must not use the objects of this crate"
    );
}

/// Releases a reference now, or after the guard falls back to zero when it is
/// raised (rule 3 of SLC-28).
pub(crate) fn release(release: Release) {
    if is_raised() {
        defer(Pending::Release(release));
    } else {
        let _raised = raise();
        release.run();
    }
}

/// The record that a global session of this crate is alive on this thread
/// (SLC-29), which ends when the last wrapper of the global session drops and
/// its deferred release, if any, has run.
pub(crate) struct GlobalSessionRecord {
    thread: PhantomData<*const ()>,
}

impl GlobalSessionRecord {
    /// Starts the record of a new global session, or returns `None` when a
    /// global session of this crate is alive on this thread.
    pub(crate) fn begin() -> Option<Self> {
        LIVE_GLOBAL_SESSIONS.with(|live| {
            if live.get() == 0 {
                live.set(1);
                Some(Self {
                    thread: PhantomData,
                })
            } else {
                None
            }
        })
    }

    /// Starts the record of a global session that the caller adopted, without
    /// checking for another one, which the caller of `from_raw` guarantees.
    pub(crate) fn adopt() -> Self {
        LIVE_GLOBAL_SESSIONS.with(|live| live.set(live.get().saturating_add(1)));
        Self {
            thread: PhantomData,
        }
    }
}

impl Drop for GlobalSessionRecord {
    fn drop(&mut self) {
        if is_raised() && defer(Pending::EndGlobalSession) {
            return;
        }
        end_global_session();
    }
}

fn end_global_session() {
    LIVE_GLOBAL_SESSIONS.with(|live| live.set(live.get().saturating_sub(1)));
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::sync::mpsc::{self, Sender};
    use std::thread;

    use slang_compiler_sys::ISlangBlob;

    use super::{GlobalSessionRecord, PENDING, call, raise};
    use crate::fake::{Event, FakeBlob, Log, panic_message};
    use crate::owned::Owned;

    fn adopt_blob(log: &Log, name: &'static str) -> Owned<ISlangBlob> {
        // SAFETY: the new fake blob carries one reference, which `adopt` takes.
        unsafe { Owned::adopt(FakeBlob::create(log, name, b"")) }
    }

    #[test]
    fn call_while_the_guard_is_raised_panics_before_running_the_inner_call() {
        let ran = std::cell::Cell::new(false);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            call("Outer::method", || call("Inner::method", || ran.set(true)));
        }));
        let payload = outcome.expect_err("a nested call panics");
        let message = panic_message(payload.as_ref());
        assert!(
            message.starts_with("reentrant call: Inner::method "),
            "{message}"
        );
        assert!(message.contains("Slang is not reentrant"), "{message}");
        assert!(message.contains("FileSystem"), "{message}");
        assert!(!ran.get());
        assert!(!super::is_raised());
    }

    #[test]
    fn drops_while_the_guard_is_raised_release_in_drop_order_after_it_falls() {
        let log = Log::new();
        let first = adopt_blob(&log, "first");
        let second = adopt_blob(&log, "second");
        call("Test::method", || {
            drop(second);
            drop(first);
            assert_eq!(log.events(), []);
        });
        assert_eq!(
            log.events(),
            [
                Event::release("second", 0),
                Event::freed("second"),
                Event::release("first", 0),
                Event::freed("first"),
            ]
        );
    }

    #[test]
    fn nested_raises_defer_releases_until_the_outermost_falls() {
        let log = Log::new();
        let blob = adopt_blob(&log, "blob");
        let outer = raise();
        let inner = raise();
        drop(blob);
        drop(inner);
        assert_eq!(log.events(), []);
        drop(outer);
        assert_eq!(
            log.events(),
            [Event::release("blob", 0), Event::freed("blob")]
        );
    }

    #[test]
    fn a_release_caused_by_a_deferred_release_is_deferred_and_drained() {
        let log = Log::new();
        let child = adopt_blob(&log, "child");
        let parent = adopt_blob(&log, "parent");
        FakeBlob::hold_until_freed(parent.as_non_null(), child);
        call("Test::method", || drop(parent));
        assert_eq!(
            log.events(),
            [
                Event::release("parent", 0),
                Event::freed("parent"),
                Event::release("child", 0),
                Event::freed("child"),
            ]
        );
    }

    #[test]
    fn a_drop_without_the_guard_releases_at_once() {
        let log = Log::new();
        drop(adopt_blob(&log, "blob"));
        assert_eq!(
            log.events(),
            [Event::release("blob", 0), Event::freed("blob")]
        );
    }

    #[test]
    fn a_second_global_session_record_is_refused_until_the_first_ends() {
        let first = GlobalSessionRecord::begin().expect("no record exists yet");
        assert!(GlobalSessionRecord::begin().is_none());
        drop(first);
        assert!(GlobalSessionRecord::begin().is_some());
    }

    #[test]
    fn a_record_dropped_while_the_guard_is_raised_ends_after_it_falls() {
        let record = GlobalSessionRecord::begin().expect("no record exists yet");
        call("Test::method", || {
            drop(record);
            assert!(GlobalSessionRecord::begin().is_none());
        });
        assert!(GlobalSessionRecord::begin().is_some());
    }
    /// What happened to a release and to the record of a global session that
    /// were dropped with the guard raised after the queue of the thread was
    /// destroyed.
    #[derive(Debug, PartialEq, Eq)]
    struct AtThreadExit {
        record_began: bool,
        events_while_raised: Vec<Event>,
        record_ended_while_raised: bool,
        events_after_lowering: Vec<Event>,
        events_after_reclaim: Vec<Event>,
    }

    fn drop_with_the_guard_raised_after_the_queue_is_destroyed() -> AtThreadExit {
        let log = Log::new();
        let blob = adopt_blob(&log, "blob");
        let raw = blob.as_non_null();
        let record = GlobalSessionRecord::begin();
        let record_began = record.is_some();
        let raised = raise();
        drop(blob);
        drop(record);
        let events_while_raised = log.events();
        let record_ended_while_raised = GlobalSessionRecord::begin().is_some();
        drop(raised);
        let events_after_lowering = log.events();
        // SAFETY: the release of `blob` was dropped without running, so the
        // reference of `create` is still owned and nothing else releases it.
        unsafe { FakeBlob::release_raw(raw) };
        AtThreadExit {
            record_began,
            events_while_raised,
            record_ended_while_raised,
            events_after_lowering,
            events_after_reclaim: log.events(),
        }
    }

    /// A thread-local value whose destructor runs the drops of
    /// `drop_with_the_guard_raised_after_the_queue_is_destroyed` when it finds
    /// the queue already destroyed.
    struct ExitProbe {
        report: RefCell<Option<Sender<AtThreadExit>>>,
    }

    impl Drop for ExitProbe {
        fn drop(&mut self) {
            let Some(report) = self.report.borrow_mut().take() else {
                return;
            };
            if PENDING.try_with(|_| ()).is_err() {
                drop(report.send(drop_with_the_guard_raised_after_the_queue_is_destroyed()));
            }
        }
    }

    thread_local! {
        static BEFORE_QUEUE: ExitProbe = const { ExitProbe { report: RefCell::new(None) } };
        static AFTER_QUEUE: ExitProbe = const { ExitProbe { report: RefCell::new(None) } };
    }

    #[test]
    fn drops_with_the_guard_raised_at_thread_exit_leak_the_release_and_end_the_record() {
        let (report, outcomes) = mpsc::channel();
        thread::spawn(move || {
            BEFORE_QUEUE.with(|probe| *probe.report.borrow_mut() = Some(report.clone()));
            PENDING.with(|queue| queue.borrow().len());
            AFTER_QUEUE.with(|probe| *probe.report.borrow_mut() = Some(report));
        })
        .join()
        .expect("the thread exits without a panic");
        let outcomes: Vec<AtThreadExit> = outcomes.iter().collect();
        assert_eq!(
            outcomes,
            [AtThreadExit {
                record_began: true,
                events_while_raised: Vec::new(),
                record_ended_while_raised: true,
                events_after_lowering: Vec::new(),
                events_after_reclaim: vec![Event::release("blob", 0), Event::freed("blob")],
            }]
        );
    }
}
