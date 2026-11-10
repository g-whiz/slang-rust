use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::c_void;
use std::io::{self, Write as _};
use std::marker::PhantomData;
use std::mem::ManuallyDrop;
use std::process;
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

/// Runs `call`, which makes exactly one call into Slang, the function
/// `callee`, and aborts the process when a C++ exception escapes that call.
///
/// The bindings declare every function of Slang with an unwinding ABI, so an
/// exception that escapes Slang unwinds into Rust instead of being undefined
/// behaviour. The value that this function holds during the call is the first
/// value with a destructor that the unwinding reaches, as long as `call`
/// creates no value with a destructor of its own, so the process aborts before
/// any other destructor of the crate or of its caller runs and before any
/// release can enter Slang again. When the program is built with
/// `panic = "abort"`, the process aborts at the call instead, and this
/// function does not run. `call` captures its arguments by reference or by
/// copy, so that it owns nothing that a destructor would drop.
pub(crate) fn abort_on_exception<R>(callee: &'static str, call: impl FnOnce() -> R) -> R {
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
        "slang-compiler: a C++ exception escaped from the Slang call {callee}, which Slang does not catch, so the process aborts\n"
    ));
    process::abort()
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

    use super::{GlobalSessionRecord, PENDING, abort_on_exception, call, raise};
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
    fn abort_on_exception_returns_the_value_of_a_call_that_returns() {
        let mut calls = 0;
        let value = abort_on_exception("Test::call", || {
            calls += 1;
            calls * 10
        });
        assert_eq!((value, calls), (10, 1));
        assert!(!super::is_raised());
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

    /// The tests of the abort of [`abort_on_exception`], each of which runs
    /// this test binary again as a child process that the abort ends, so they
    /// sit in a module named `ffi`, which Miri skips.
    mod ffi {
        use std::env;
        use std::ffi::CString;
        use std::process::{Command, ExitStatus};
        use std::ptr::{self, NonNull};

        use slang_compiler_sys::{
            IComponentType, IModule, ISlangBlob, SlangReflection, TypeReflection,
            spReflection_FindTypeByName,
        };

        use super::super::abort_on_exception;
        use crate::reflection::Type;
        use crate::{Component, GlobalSession, Session, SpecializationArg, Target, TargetFormat};

        /// The environment variable that selects the input of a child process.
        const CHILD: &str = "SLANG_COMPILER_GUARD_CHILD";

        /// The line that a child prints immediately before the call.
        const ABOUT_TO_CALL: &str = "the child is about to make the call";

        /// The line that a child prints when the call returned.
        const RETURNED: &str = "the call returned";

        /// The line that a child prints when a destructor outside the call
        /// ran.
        const OUTER_DROPPED: &str = "a destructor outside the call ran";

        /// A module with a generic compute kernel and a generic structure that
        /// a type name can nest to any depth.
        const GENERIC: &str = r#"
interface IValue { uint get(); }
struct Leaf : IValue { uint get() { return 1; } }
struct Box<T> : IValue { T inner; uint get() { return 2; } }
RWStructuredBuffer<uint> output;

[shader("compute")]
[numthreads(1, 1, 1)]
void main<T : IValue>(uniform T value)
{
    output[0] = value.get();
}
"#;

        struct PrintOnDrop;

        impl Drop for PrintOnDrop {
            fn drop(&mut self) {
                println!("{OUTER_DROPPED}");
            }
        }

        /// What a child process printed, and how it ended.
        struct Child {
            status: ExitStatus,
            stdout: String,
            stderr: String,
        }

        /// Runs this test binary again with only the test `child` and the
        /// input `input`.
        fn run_child(input: &str) -> Child {
            let output = Command::new(env::current_exe().expect("the test binary has a path"))
                .args([
                    "--exact",
                    "guard::tests::ffi::child",
                    "--nocapture",
                    "--test-threads=1",
                ])
                .env(CHILD, input)
                .output()
                .expect("the child process starts");
            Child {
                status: output.status,
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            }
        }

        /// Returns whether `status` is the end of a process that
        /// `std::process::abort` stopped: the signal `SIGABRT` on Unix, and
        /// the status of `__fastfail` on Windows.
        fn aborted(status: ExitStatus) -> bool {
            #[cfg(unix)]
            {
                use std::os::unix::process::ExitStatusExt as _;
                status.signal() == Some(6)
            }
            #[cfg(windows)]
            {
                status.code() == Some(0xC000_0409_u32.cast_signed())
            }
        }

        /// Asserts that the child reached the call, that the guard of the call
        /// `callee` aborted the process with its message, and that the call
        /// did not return.
        fn assert_aborted_by_the_guard(child: &Child, callee: &str) {
            let all = format!("{}{}", child.stdout, child.stderr);
            assert!(
                child.stdout.contains(ABOUT_TO_CALL),
                "the child did not reach the call: {all}"
            );
            assert!(
                !child.stdout.contains(RETURNED),
                "the call returned, so Slang no longer lets the exception escape: {all}"
            );
            assert!(
                child.stderr.contains(&format!(
                    "slang-compiler: a C++ exception escaped from the Slang call {callee}, which Slang does not catch, so the process aborts"
                )),
                "the guard did not report the call: {all}"
            );
            assert!(
                aborted(child.status),
                "the child did not end by an abort, but with {:?}: {all}",
                child.status
            );
            assert!(
                !child.stdout.contains(OUTER_DROPPED),
                "a destructor outside the call ran before the abort: {all}"
            );
        }

        /// Finds the type `name` in the layout of a module that has no
        /// deeply nested parameter, which Slang lays out without an exception.
        fn find_type<'session>(
            session: &'session Session,
            module: &crate::Module,
            name: &str,
        ) -> Option<Type<'session>> {
            let name = CString::new(name).expect("no NUL byte");
            let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
            // SAFETY: `module` keeps the module and its session alive, the
            // session has a target at index 0, and `diagnostics` is a live
            // out-pointer.
            let layout =
                unsafe { IModule::getLayout(module.as_raw().as_ptr(), 0, &raw mut diagnostics) };
            if !diagnostics.is_null() {
                // SAFETY: the call returned a new reference, which the test
                // owns.
                unsafe { ISlangBlob::release(diagnostics) };
            }
            assert!(!layout.is_null(), "Slang lays out the module");
            #[expect(
                ffi_unwind_calls,
                reason = "the test calls a C function of Slang directly, and an exception that escaped it would unwind into the test, which is defined and fails it"
            )]
            // SAFETY: `layout` is a live layout of `module`, `ShaderReflection`
            // and `SlangReflection` name the same object, and `name` is a C
            // string.
            let raw = unsafe {
                spReflection_FindTypeByName(layout.cast::<SlangReflection>(), name.as_ptr())
            };
            let raw = NonNull::new(raw.cast::<TypeReflection>())?;
            // SAFETY: Slang returned a type of the session of `module`, which
            // `session` is and keeps alive with the module, on this thread.
            Some(unsafe { Type::from_raw(raw, session) })
        }

        /// The body of the child processes of the tests below, which does
        /// nothing unless the variable `CHILD` names an input.
        #[test]
        fn child() {
            let Ok(input) = env::var(CHILD) else {
                return;
            };
            let _outer = PrintOnDrop;
            match input.as_str() {
                "panic" => {
                    println!("{ABOUT_TO_CALL}");
                    abort_on_exception("Test::panicking_call", || {
                        panic!("a Rust panic that unwinds as a C++ exception would")
                    });
                    println!("{RETURNED}");
                }
                "get_layout" => {
                    let global = GlobalSession::new().expect("Slang creates a global session");
                    let session = Session::builder(&global)
                        .target(Target::new(TargetFormat::Spirv))
                        .build()
                        .expect("Slang creates the session");
                    let module = session
                        .load_module_from_source("generic", "generic.slang", GENERIC)
                        .expect("the source compiles")
                        .into_value();
                    let entry_point = module.entry_point("main").expect("the entry point exists");
                    let composed = session
                        .compose(&[
                            Component::Module(&module),
                            Component::EntryPoint(&entry_point),
                        ])
                        .expect("the parts compose")
                        .into_value();
                    let name = format!("{}Leaf{}", "Box<".repeat(300), ">".repeat(300));
                    let deep =
                        find_type(&session, &module, &name).expect("Slang finds the nested type");
                    let program = composed
                        .specialize(&[SpecializationArg::Type(deep)])
                        .expect("Slang accepts the nested type")
                        .into_value()
                        .link()
                        .expect("the specialization links")
                        .into_value();
                    let raw = program.as_raw().as_ptr();
                    let mut diagnostics: *mut ISlangBlob = ptr::null_mut();
                    println!("{ABOUT_TO_CALL}");
                    // SAFETY: `program` keeps the program and its session
                    // alive, the target index 0 exists, and the out-pointer is
                    // live.
                    let layout = unsafe {
                        abort_on_exception("IComponentType::getLayout", || {
                            IComponentType::getLayout(raw, 0, &raw mut diagnostics)
                        })
                    };
                    println!("{RETURNED} with {layout:?}");
                }
                other => panic!("unknown input {other}"),
            }
        }

        #[test]
        fn an_unwinding_call_aborts_the_process_before_an_outer_destructor_runs() {
            let child = run_child("panic");
            assert_aborted_by_the_guard(&child, "Test::panicking_call");
            assert!(
                child
                    .stderr
                    .contains("a Rust panic that unwinds as a C++ exception would"),
                "{}",
                child.stderr
            );
        }

        #[test]
        fn get_layout_past_the_nesting_limit_aborts_the_process_through_the_guard() {
            let child = run_child("get_layout");
            assert_aborted_by_the_guard(&child, "IComponentType::getLayout");
            assert!(
                !child.stderr.contains("panicked at"),
                "{}{}",
                child.stdout,
                child.stderr
            );
        }
    }
}
