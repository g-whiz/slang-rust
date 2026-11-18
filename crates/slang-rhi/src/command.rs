use std::marker::PhantomData;
use std::panic::{self, AssertUnwindSafe};
use std::ptr::{self, NonNull};

use slang_rhi_sys::{
    ICommandBuffer, ICommandEncoder, ICommandQueue, IComputePassEncoder, IComputePipeline, IDevice,
};

use crate::device::Device;
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::owned::Owned;
use crate::program::ComputePipeline;
use crate::shader_object::ShaderObject;

/// The queue of a device, which runs the command buffers that it receives.
///
/// Get it from [`Device::queue`]. A queue holds its [`Device`], so the device
/// lives at least as long as the queue.
///
/// A queue is neither [`Send`] nor [`Sync`], because slang-rhi documents no
/// rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of the [crate documentation](crate).
///
/// A queue cannot move to another thread, so this example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::Queue>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::Queue>();
/// ```
#[derive(Debug)]
pub struct Queue {
    raw: Owned<ICommandQueue>,
    device: Device,
}

/// A command encoder, which records commands, such as compute passes, into a
/// command buffer.
///
/// Get one from [`Queue::encoder`], record with
/// [`CommandEncoder::compute_pass`], and end the recording with
/// [`CommandEncoder::finish`]. An encoder holds its [`Device`], so the device
/// lives at least as long as the encoder.
///
/// An encoder is neither [`Send`] nor [`Sync`], because slang-rhi documents
/// no rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of the [crate documentation](crate).
///
/// An encoder cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::CommandEncoder>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::CommandEncoder>();
/// ```
#[derive(Debug)]
pub struct CommandEncoder {
    raw: Owned<ICommandEncoder>,
    device: Device,
}

/// A command buffer, a recorded list of commands that a queue runs.
///
/// [`CommandEncoder::finish`] returns one, and [`Queue::submit`] takes it and
/// runs it once, because slang-rhi reuses the native command buffer after it
/// has run. A command buffer holds its [`Device`], so the device lives at
/// least as long as the command buffer.
///
/// A command buffer is neither [`Send`] nor [`Sync`], because slang-rhi
/// documents no rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of the [crate documentation](crate).
///
/// A command buffer cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::CommandBuffer>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::CommandBuffer>();
/// ```
#[derive(Debug)]
pub struct CommandBuffer {
    raw: Owned<ICommandBuffer>,
    device: Device,
}

/// A compute pass of a command encoder, in which compute pipelines are bound
/// and dispatched.
///
/// A pass exists only inside the closure of [`CommandEncoder::compute_pass`],
/// which begins it before the closure runs and ends it after the closure
/// returns, so no pass outlives its closure.
///
/// A pass is neither [`Send`] nor [`Sync`], because slang-rhi documents no
/// rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of the [crate documentation](crate).
///
/// A pass cannot leave its closure, so this example, which tries to keep the
/// root shader object of a pass after the closure, does not compile:
///
/// ```compile_fail,E0521
/// use slang_rhi::{CommandEncoder, ComputePipeline};
///
/// fn escape(encoder: &mut CommandEncoder, pipeline: &ComputePipeline) -> slang_rhi::Result<()> {
///     let mut kept = None;
///     encoder.compute_pass(|pass| {
///         kept = Some(pass.bind_pipeline(pipeline)?);
///         Ok(())
///     })?;
///     drop(kept);
///     Ok(())
/// }
/// ```
///
/// A pass cannot move to another thread, so this example does not compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::ComputePass<'static>>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::ComputePass<'static>>();
/// ```
#[derive(Debug)]
pub struct ComputePass<'e> {
    raw: NonNull<IComputePassEncoder>,
    device: NonNull<IDevice>,
    max_thread_groups: [u32; 3],
    bound: bool,
    encoder: PhantomData<&'e mut ()>,
}

impl Device {
    /// Returns the queue of the device.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns the error that slang-rhi reports for the lookup.
    pub fn queue(&self) -> Result<Queue> {
        let device = self.as_ptr();
        let mut out: *mut ICommandQueue = ptr::null_mut();
        let result = guard::call("IDevice::getQueue", || {
            // SAFETY: `device` is live while `self` lives, and `out` is a live
            // out-pointer.
            unsafe { IDevice::getQueue(device, slang_rhi_sys::QueueType::Graphics, &raw mut out) }
        });
        error::check(Operation::GetQueue, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the queue with one
        // reference added for the caller.
        let raw = unsafe { Owned::adopt(out) }.ok_or_else(|| Error::null(Operation::GetQueue))?;
        Ok(Queue {
            raw,
            device: self.clone(),
        })
    }
}

impl Queue {
    /// Creates a command encoder that records commands for this queue.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns the error that slang-rhi reports for the creation.
    pub fn encoder(&self) -> Result<CommandEncoder> {
        let queue = self.raw.as_ptr();
        let desc = slang_rhi_sys::CommandEncoderDesc::default();
        let mut out: *mut ICommandEncoder = ptr::null_mut();
        let result = guard::call("ICommandQueue::createCommandEncoder", || {
            // SAFETY: `queue` is live while `self` lives, `desc` is a live
            // descriptor with the defaults of the header, and `out` is a live
            // out-pointer.
            unsafe { ICommandQueue::createCommandEncoder(queue, &raw const desc, &raw mut out) }
        });
        error::check(Operation::CreateCommandEncoder, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the encoder with
        // one reference added for the caller.
        let raw = unsafe { Owned::adopt(out) }
            .ok_or_else(|| Error::null(Operation::CreateCommandEncoder))?;
        Ok(CommandEncoder {
            raw,
            device: self.device.clone(),
        })
    }

    /// Runs the command buffers `command_buffers` on the queue, in order, and
    /// takes them.
    ///
    /// The call returns once the queue has received them, not once they have
    /// run. Wait for them with [`Queue::wait_on_host`]. Each command buffer
    /// runs once, because slang-rhi reuses the native command buffer after it
    /// has run, so the call takes the command buffers, even when it fails.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// A command buffer cannot be submitted twice, so this example does not
    /// compile:
    ///
    /// ```compile_fail,E0382
    /// use slang_rhi::{CommandBuffer, Queue};
    ///
    /// fn twice(queue: &Queue, commands: CommandBuffer) -> slang_rhi::Result<()> {
    ///     queue.submit([commands])?;
    ///     queue.submit([commands])
    /// }
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when a command buffer belongs to
    /// another device, and the error that slang-rhi reports for the
    /// submission.
    pub fn submit(&self, command_buffers: impl IntoIterator<Item = CommandBuffer>) -> Result<()> {
        let command_buffers: Vec<CommandBuffer> = command_buffers.into_iter().collect();
        if command_buffers
            .iter()
            .any(|buffer| buffer.device.as_non_null() != self.device.as_non_null())
        {
            return Err(Error::invalid(
                Operation::Submit,
                "a command buffer belongs to another device",
            ));
        }
        let raw_buffers = command_buffers
            .into_iter()
            .map(|buffer| buffer.raw)
            .collect();
        // SAFETY: the queue is live while `self` lives, and every command
        // buffer belongs to the device of the queue by the check above.
        unsafe { submit_owned(self.raw.as_non_null(), raw_buffers) }
    }

    /// Waits until the queue has run every command buffer that it received.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns the error that slang-rhi reports for the wait.
    pub fn wait_on_host(&self) -> Result<()> {
        let queue = self.raw.as_ptr();
        let result = guard::call("ICommandQueue::waitOnHost", || {
            // SAFETY: `queue` is live while `self` lives.
            unsafe { ICommandQueue::waitOnHost(queue) }
        });
        error::check(Operation::WaitOnHost, result, None)
    }
}

impl CommandEncoder {
    /// Records a compute pass, in which `f` binds compute pipelines, sets
    /// their parameters, and dispatches them, and returns the value of `f`.
    ///
    /// The method begins the pass, runs `f`, and ends the pass, whether `f`
    /// returns a value or an error or panics. A panic of `f` continues after
    /// the pass has ended.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns the error that `f` returns, and an error of kind
    /// [`ErrorKind::Failed`](crate::ErrorKind::Failed) when slang-rhi begins
    /// no pass.
    pub fn compute_pass<R>(
        &mut self,
        f: impl FnOnce(&mut ComputePass<'_>) -> Result<R>,
    ) -> Result<R> {
        let encoder = self.raw.as_ptr();
        let pass = guard::call("ICommandEncoder::beginComputePass", || {
            // SAFETY: `encoder` is live while `self` lives, and no other pass
            // of it is open, because every pass ends before this method
            // returns or unwinds.
            unsafe { ICommandEncoder::beginComputePass(encoder) }
        });
        let pass = NonNull::new(pass).ok_or_else(|| Error::null(Operation::BeginComputePass))?;
        // SAFETY: the pass belongs to the encoder, which `self` keeps alive,
        // and the mutable borrow of `self` keeps every other use of the
        // encoder out until the pass ends inside `run_pass`.
        unsafe {
            run_pass(
                pass,
                self.device.as_non_null(),
                self.device.max_thread_groups(),
                f,
            )
        }
    }

    /// Ends the recording and returns the command buffer that holds the
    /// recorded commands.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns the error that slang-rhi reports for the end of the recording.
    pub fn finish(self) -> Result<CommandBuffer> {
        let encoder = self.raw.as_ptr();
        let desc = slang_rhi_sys::CommandBufferDesc::default();
        let mut out: *mut ICommandBuffer = ptr::null_mut();
        let result = guard::call("ICommandEncoder::finish", || {
            // SAFETY: `encoder` is live while `self` lives, no pass of it is
            // open, `desc` is a live descriptor with the defaults of the
            // header, and `out` is a live out-pointer.
            unsafe { ICommandEncoder::finish(encoder, &raw const desc, &raw mut out) }
        });
        error::check(Operation::FinishEncoder, result, None)?;
        // SAFETY: the call succeeded, and slang-rhi returns the command
        // buffer with one reference added for the caller.
        let raw =
            unsafe { Owned::adopt(out) }.ok_or_else(|| Error::null(Operation::FinishEncoder))?;
        Ok(CommandBuffer {
            raw,
            device: self.device,
        })
    }
}

/// Submits the command buffers `command_buffers` to the queue `queue`, and
/// releases the references of the crate to them after the call, so that no
/// command buffer can be submitted again.
///
/// slang-rhi keeps its own reference to each command buffer until it has run.
///
/// # Safety
///
/// `queue` points to a live command queue, and each command buffer belongs to
/// the device of the queue. Breaking either is undefined behaviour.
unsafe fn submit_owned(
    queue: NonNull<ICommandQueue>,
    command_buffers: Vec<Owned<ICommandBuffer>>,
) -> Result<()> {
    let mut raw_buffers: Vec<*mut ICommandBuffer> =
        command_buffers.iter().map(Owned::as_ptr).collect();
    let count = u32::try_from(raw_buffers.len())
        .map_err(|_overflow| Error::invalid(Operation::Submit, "too many command buffers"))?;
    let desc = slang_rhi_sys::SubmitDesc {
        commandBuffers: raw_buffers.as_mut_ptr(),
        commandBufferCount: count,
        ..slang_rhi_sys::SubmitDesc::default()
    };
    let queue = queue.as_ptr();
    let result = guard::call("ICommandQueue::submit", || {
        // SAFETY: the caller guarantees that `queue` is live and that every
        // command buffer belongs to its device. `desc` names `count` live
        // command buffers, which `command_buffers` keeps alive during the
        // call, no fence, and the CUDA stream that the header gives as the
        // default.
        unsafe { ICommandQueue::submit(queue, &raw const desc) }
    });
    drop(command_buffers);
    error::check(Operation::Submit, result, None)
}

/// Runs `f` with the pass `raw`, and ends the pass after `f` returns or
/// panics.
///
/// # Safety
///
/// `raw` points to the live pass encoder of a pass that has begun and not
/// ended, whose command encoder stays alive and is not used in any other way
/// until this function returns. `device` is the device of the encoder, and
/// `max_thread_groups` the largest number of thread groups along each axis of
/// a dispatch that it allows. Breaking any of these is undefined behaviour.
unsafe fn run_pass<R>(
    raw: NonNull<IComputePassEncoder>,
    device: NonNull<IDevice>,
    max_thread_groups: [u32; 3],
    f: impl FnOnce(&mut ComputePass<'_>) -> Result<R>,
) -> Result<R> {
    let mut pass = ComputePass {
        raw,
        device,
        max_thread_groups,
        bound: false,
        encoder: PhantomData,
    };
    let outcome = panic::catch_unwind(AssertUnwindSafe(|| f(&mut pass)));
    let pass_encoder = raw.as_ptr();
    guard::call("IComputePassEncoder::end", || {
        // SAFETY: the caller guarantees that the pass is live and open, and
        // `f` has returned, so nothing borrows the pass any longer.
        unsafe { IComputePassEncoder::end(pass_encoder) }
    });
    match outcome {
        Ok(value) => value,
        Err(payload) => panic::resume_unwind(payload),
    }
}

impl ComputePass<'_> {
    /// Binds `pipeline` for the dispatches that follow, and returns the root
    /// shader object of its program, which holds the global parameters of the
    /// program.
    ///
    /// The root shader object borrows the pass, so the pass can dispatch or
    /// bind another pipeline only after the root object is dropped. After a
    /// failed bind, the pass refuses to dispatch until a bind succeeds.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when the pipeline belongs to another
    /// device, and an error of kind
    /// [`ErrorKind::Failed`](crate::ErrorKind::Failed) when slang-rhi returns
    /// no root shader object.
    pub fn bind_pipeline(&mut self, pipeline: &ComputePipeline) -> Result<ShaderObject<'_>> {
        if pipeline.device_ptr() != self.device {
            return Err(Error::invalid(
                Operation::BindPipeline,
                "the pipeline belongs to another device",
            ));
        }
        self.bind(pipeline.as_ptr())
    }

    /// Binds the pipeline `pipeline` of the device of the pass, which the
    /// caller keeps alive through the call.
    fn bind(&mut self, pipeline: *mut IComputePipeline) -> Result<ShaderObject<'_>> {
        self.bound = false;
        let pass = self.raw.as_ptr();
        let root = guard::call("IComputePassEncoder::bindPipeline", || {
            // SAFETY: the pass is live and open while `self` lives, and
            // `pipeline` is a live pipeline of the same device. slang-rhi
            // keeps its own reference to the pipeline.
            unsafe { IComputePassEncoder::bindPipeline(pass, pipeline) }
        });
        let root = NonNull::new(root).ok_or_else(|| Error::null(Operation::BindPipeline))?;
        self.bound = true;
        // SAFETY: the pass holds the root object until its next
        // `bindPipeline` or until the encoder is freed, and the returned value
        // borrows `self` mutably, so neither happens while it lives.
        Ok(unsafe { ShaderObject::borrowed(root, self.device) })
    }

    /// Dispatches the bound pipeline with `thread_groups` groups of threads
    /// along the x, y, and z axes.
    ///
    /// The kernel runs once for each thread, and each group has the number of
    /// threads that its `[numthreads(x, y, z)]` attribute declares. A count of
    /// 0 along any axis runs nothing.
    ///
    /// Bind a buffer to every buffer parameter that the kernel uses before the
    /// dispatch. A parameter left without a buffer works on Direct3D 12, where
    /// slang-rhi binds a null descriptor, and on Vulkan only on a device whose
    /// driver offers the feature `nullDescriptor`, which this crate does not
    /// check.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when no pipeline is bound in the pass,
    /// or when a count is larger than the device allows along its axis, which
    /// the Vulkan limit `maxComputeWorkGroupCount` gives, and which is 65535
    /// on Direct3D 12.
    pub fn dispatch(&mut self, thread_groups: [u32; 3]) -> Result<()> {
        if !self.bound {
            return Err(Error::invalid(
                Operation::Dispatch,
                "no pipeline is bound in the pass",
            ));
        }
        check_thread_groups(thread_groups, self.max_thread_groups)?;
        let pass = self.raw.as_ptr();
        let [x, y, z] = thread_groups;
        guard::call("IComputePassEncoder::dispatchCompute", || {
            // SAFETY: the pass is live and open while `self` lives, a
            // pipeline is bound in it, and each count is within the limit of
            // the device.
            unsafe { IComputePassEncoder::dispatchCompute(pass, x, y, z) }
        });
        Ok(())
    }
}

/// Checks that each count of `thread_groups` is at most the limit of its axis
/// in `max_thread_groups`.
fn check_thread_groups(thread_groups: [u32; 3], max_thread_groups: [u32; 3]) -> Result<()> {
    for ((axis, count), max) in ["x", "y", "z"]
        .into_iter()
        .zip(thread_groups)
        .zip(max_thread_groups)
    {
        if count > max {
            return Err(Error::invalid(
                Operation::Dispatch,
                format!("{count} thread groups along {axis} exceed the limit {max} of the device"),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    use std::ptr::NonNull;

    use slang_rhi_sys::IDevice;

    use super::{check_thread_groups, run_pass, submit_owned};
    use crate::ErrorKind;
    use crate::error::{Error, Operation};
    use crate::fake::{Event, FakeCommandBuffer, FakePass, FakeQueue, Log};
    use crate::owned::Owned;

    const LIMITS: [u32; 3] = [65535, 65535, 64];

    fn device() -> NonNull<IDevice> {
        NonNull::new(std::ptr::without_provenance_mut(8)).expect("non-zero")
    }

    #[test]
    fn run_pass_ends_the_pass_after_the_closure_returns_its_value() {
        let log = Log::new();
        let pass = FakePass::create(&log, "pass");
        // SAFETY: the fake pass lives until the test releases it below, and
        // nothing else uses it during the call.
        let value =
            unsafe { run_pass(pass, device(), LIMITS, |_| Ok(5)) }.expect("the closure succeeds");
        assert_eq!(value, 5);
        assert_eq!(log.events(), [Event::call("pass", "end")]);
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakePass::release_raw(pass) };
    }

    #[test]
    fn run_pass_ends_the_pass_after_the_closure_fails_and_returns_its_error() {
        let log = Log::new();
        let pass = FakePass::create(&log, "pass");
        // SAFETY: as in the test above.
        let error = unsafe {
            run_pass::<()>(pass, device(), LIMITS, |_| {
                Err(Error::invalid(
                    Operation::Dispatch,
                    "a failure of the closure",
                ))
            })
        }
        .expect_err("the closure fails");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(log.events(), [Event::call("pass", "end")]);
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakePass::release_raw(pass) };
    }

    #[test]
    fn run_pass_ends_the_pass_before_a_panic_of_the_closure_continues() {
        let log = Log::new();
        let pass = FakePass::create(&log, "pass");
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            // SAFETY: as in the tests above.
            unsafe {
                run_pass::<()>(pass, device(), LIMITS, |_| {
                    std::panic::resume_unwind(Box::new("a panic of the closure"))
                })
            }
        }));
        assert!(outcome.is_err());
        assert_eq!(log.events(), [Event::call("pass", "end")]);
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakePass::release_raw(pass) };
    }

    #[test]
    fn dispatch_without_a_bound_pipeline_fails_before_any_call() {
        let log = Log::new();
        let pass = FakePass::create(&log, "pass");
        // SAFETY: as in the tests above.
        let error = unsafe { run_pass(pass, device(), LIMITS, |pass| pass.dispatch([1, 1, 1])) }
            .expect_err("nothing is bound");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::Dispatch);
        assert_eq!(log.events(), [Event::call("pass", "end")]);
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakePass::release_raw(pass) };
    }

    #[test]
    fn bind_returns_the_root_object_of_the_pass_and_enables_dispatch() {
        let log = Log::new();
        let pass = FakePass::create(&log, "pass");
        let pipeline = std::ptr::without_provenance_mut(64);
        // SAFETY: as in the tests above. The fake pass never reads the
        // pipeline pointer.
        unsafe {
            run_pass(pass, device(), LIMITS, |pass| {
                let mut root = pass.bind(pipeline)?;
                root.entry_point(0)
                    .map(drop)
                    .expect_err("the fake root has no entry point");
                drop(root);
                pass.dispatch([2, 3, 4])
            })
        }
        .expect("the dispatch is recorded");
        assert_eq!(
            log.events(),
            [
                Event::bind_pipeline("pass", 64),
                Event::dispatch("pass", [2, 3, 4]),
                Event::call("pass", "end"),
            ]
        );
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakePass::release_raw(pass) };
    }

    #[test]
    fn a_failed_bind_leaves_dispatch_refused() {
        let log = Log::new();
        let pass = FakePass::create(&log, "pass");
        FakePass::answer_binds_with_null(pass);
        // SAFETY: as in the tests above.
        let errors = unsafe {
            run_pass(pass, device(), LIMITS, |pass| {
                let bind = pass
                    .bind(std::ptr::without_provenance_mut(64))
                    .map(drop)
                    .expect_err("the fake returns no root object");
                let dispatch = pass.dispatch([1, 1, 1]).expect_err("nothing is bound");
                Ok((bind.kind(), dispatch.kind()))
            })
        }
        .expect("the closure returns both errors");
        assert_eq!(errors, (ErrorKind::Failed, ErrorKind::InvalidArgument));
        assert_eq!(
            log.events(),
            [Event::bind_pipeline("pass", 64), Event::call("pass", "end")]
        );
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakePass::release_raw(pass) };
    }

    #[test]
    fn check_thread_groups_accepts_counts_up_to_the_limit_and_zero() {
        check_thread_groups([65535, 65535, 64], LIMITS).expect("at the limit");
        check_thread_groups([0, 0, 0], LIMITS).expect("nothing to run");
        for groups in [[65536, 1, 1], [1, 65536, 1], [1, 1, 65], [u32::MAX; 3]] {
            let error = check_thread_groups(groups, LIMITS).expect_err("past the limit");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument, "{groups:?}");
            assert_eq!(error.operation(), Operation::Dispatch);
        }
    }

    #[test]
    fn dispatch_past_the_limit_of_the_device_fails_before_any_call() {
        let log = Log::new();
        let pass = FakePass::create(&log, "pass");
        // SAFETY: as in the tests above. The fake pass never reads the
        // pipeline pointer.
        let error = unsafe {
            run_pass(pass, device(), LIMITS, |pass| {
                drop(pass.bind(std::ptr::without_provenance_mut(64))?);
                pass.dispatch([1, 1, 65])
            })
        }
        .expect_err("past the limit");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            log.events(),
            [Event::bind_pipeline("pass", 64), Event::call("pass", "end")]
        );
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakePass::release_raw(pass) };
    }

    #[test]
    fn a_failed_bind_after_a_successful_one_refuses_dispatch() {
        let log = Log::new();
        let pass = FakePass::create(&log, "pass");
        // SAFETY: as in the tests above. The fake pass never reads the
        // pipeline pointer.
        let error = unsafe {
            run_pass(pass, device(), LIMITS, |pass| {
                drop(pass.bind(std::ptr::without_provenance_mut(64))?);
                FakePass::answer_binds_with_null(pass.raw);
                pass.bind(std::ptr::without_provenance_mut(128))
                    .map(drop)
                    .expect_err("the fake returns no root object");
                pass.dispatch([1, 1, 1])
            })
        }
        .expect_err("the last bind failed");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::Dispatch);
        assert_eq!(
            log.events(),
            [
                Event::bind_pipeline("pass", 64),
                Event::bind_pipeline("pass", 128),
                Event::call("pass", "end"),
            ]
        );
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakePass::release_raw(pass) };
    }

    #[test]
    fn submit_owned_releases_each_command_buffer_after_the_queue_takes_its_own() {
        let log = Log::new();
        let queue = FakeQueue::create(&log, "queue");
        let first = FakeCommandBuffer::create(&log, "first");
        let second = FakeCommandBuffer::create(&log, "second");
        // SAFETY: each fake command buffer is live, and its one reference
        // passes to the owning pointer.
        let owned = [first, second].map(|raw| unsafe { Owned::adopt(raw.as_ptr()) });
        let owned: Vec<_> = owned
            .into_iter()
            .map(|raw| raw.expect("non-null"))
            .collect();
        // SAFETY: the fake queue is live, and the fake command buffers have
        // no device that the queue checks.
        unsafe { submit_owned(queue, owned) }.expect("the queue receives them");
        assert_eq!(
            log.events(),
            [
                Event::add_ref("first", 2),
                Event::add_ref("second", 2),
                Event::submit("queue", &["first", "second"]),
                Event::release("first", 1),
                Event::release("second", 1),
            ]
        );
        // SAFETY: the test owns the reference of `create` and releases it once.
        // The queue releases the references that it took when it is freed.
        unsafe { FakeQueue::release_raw(queue) };
        assert_eq!(
            log.events()[5..],
            [
                Event::release("queue", 0),
                Event::freed("queue"),
                Event::release("first", 0),
                Event::freed("first"),
                Event::release("second", 0),
                Event::freed("second"),
            ]
        );
    }

    #[test]
    fn submit_owned_with_no_command_buffer_submits_an_empty_list() {
        let log = Log::new();
        let queue = FakeQueue::create(&log, "queue");
        // SAFETY: the fake queue is live.
        unsafe { submit_owned(queue, Vec::new()) }.expect("the queue receives nothing");
        assert_eq!(log.events(), [Event::submit("queue", &[])]);
        // SAFETY: the test owns the reference of `create` and releases it once.
        unsafe { FakeQueue::release_raw(queue) };
    }
}
