use std::ptr::{self, NonNull};

use slang_compiler::{Blob, LinkedProgram, Session, Stage};
use slang_compiler_sys::{IComponentType, ISlangBlob};
use slang_rhi_sys::{IComputePipeline, IDevice, IShaderProgram};

use crate::device::Device;
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::owned::Owned;

/// A shader program of a device, which holds a linked Slang program for that
/// device.
///
/// Create one with [`Device::create_shader_program`] or
/// [`CompileSession::create_shader_program`](crate::CompileSession::create_shader_program),
/// and a compute pipeline from it with [`Device::create_compute_pipeline`]. A
/// shader program holds its [`Device`], so the device lives at least as long
/// as the program, and a shader program of a
/// [`CompileSession`](crate::CompileSession) also holds the Slang session of
/// that compile session.
///
/// A shader program is neither [`Send`] nor [`Sync`], because slang-rhi
/// documents no rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of the [crate documentation](crate).
///
/// A shader program cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::ShaderProgram>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::ShaderProgram>();
/// ```
#[derive(Debug)]
pub struct ShaderProgram {
    raw: Owned<IShaderProgram>,
    _session: Option<Session>,
    device: Device,
    /// The thread group size of the one compute entry point of the program,
    /// or `None` when it does not have exactly one entry point that is a
    /// compute kernel.
    thread_group_size: Option<[usize; 3]>,
}

/// A compute pipeline of a device, which holds the compiled code of the
/// compute kernel of a shader program.
///
/// Create one with [`Device::create_compute_pipeline`], and bind it in a
/// compute pass with
/// [`ComputePass::bind_pipeline`](crate::ComputePass::bind_pipeline). A
/// compute pipeline holds its [`Device`], so the device lives at least as long
/// as the pipeline.
///
/// A compute pipeline is neither [`Send`] nor [`Sync`], because slang-rhi
/// documents no rule for the use of its objects from several threads.
///
/// # Examples
///
/// See the example of the [crate documentation](crate).
///
/// A compute pipeline cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::ComputePipeline>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::ComputePipeline>();
/// ```
#[derive(Debug)]
pub struct ComputePipeline {
    raw: Owned<IComputePipeline>,
    device: Device,
}

impl ComputePipeline {
    pub(crate) fn as_ptr(&self) -> *mut IComputePipeline {
        self.raw.as_ptr()
    }

    pub(crate) fn device_ptr(&self) -> NonNull<IDevice> {
        self.device.as_non_null()
    }
}

/// Joins the messages that a call collected into the text of diagnostics, or
/// returns `None` when there are none.
fn diagnostics(messages: &[String]) -> Option<String> {
    if messages.is_empty() {
        None
    } else {
        Some(messages.join("\n"))
    }
}

/// Returns the thread group size of the entry point of a linked program of
/// `session` when the program has exactly one entry point and that entry
/// point is a compute kernel, and `None` otherwise.
fn compute_thread_group_size(
    program: &LinkedProgram,
    session: &Session,
) -> Result<Option<[usize; 3]>> {
    if program.entry_point_count() != 1 {
        return Ok(None);
    }
    let slang = |error| Error::slang(Operation::CreateShaderProgram, error);
    let target = session.target_index(0).map_err(slang)?;
    let layout = program.layout(target).map_err(slang)?.into_value();
    let mut kernels = Vec::new();
    for entry_point in layout.entry_point_layouts() {
        let stage = entry_point.stage().map_err(slang)?;
        kernels
            .push((stage == Some(Stage::Compute)).then(|| entry_point.compute_thread_group_size()));
    }
    Ok(match kernels.as_slice() {
        [Some(size)] => Some(*size),
        _ => None,
    })
}

/// Checks that a thread group of `size` threads along the three axes fits
/// the largest size along each axis, `max_size`, and the largest number of
/// threads of one group, `max_threads`.
///
/// slang-rhi checks neither without its debug device, and a dispatch of a
/// larger group can break the device for every later call.
fn check_thread_group_size(size: [usize; 3], max_size: [u32; 3], max_threads: u32) -> Result<()> {
    let [x, y, z] = size;
    let fits =
        |value: usize, max: u32| u64::try_from(value).is_ok_and(|value| value <= u64::from(max));
    let axes = ['x', 'y', 'z'];
    for ((axis, value), max) in axes.into_iter().zip(size).zip(max_size) {
        if !fits(value, max) {
            return Err(Error::invalid(
                Operation::CreateComputePipeline,
                format!(
                    "the thread group of the kernel has {value} threads along the axis {axis}, more than the {max} that the device allows"
                ),
            ));
        }
    }
    let threads = x.checked_mul(y).and_then(|xy| xy.checked_mul(z));
    if !threads.is_some_and(|threads| fits(threads, max_threads)) {
        return Err(Error::invalid(
            Operation::CreateComputePipeline,
            format!(
                "the thread group of the kernel has {x} by {y} by {z} threads, more than the {max_threads} threads of one group that the device allows"
            ),
        ));
    }
    Ok(())
}

/// Checks that a linked program has no specialization parameter left, which
/// is a parameter that a generic or an interface type leaves open.
///
/// slang-rhi compiles the code of a program with such a parameter only at the
/// first dispatch, where this crate cannot return the messages of Slang.
fn check_specialized(program: &LinkedProgram) -> Result<()> {
    let raw = program.as_raw().as_ptr();
    let count = guard::call("IComponentType::getSpecializationParamCount", || {
        // SAFETY: `raw` points to the live linked program that `program`
        // holds, and the call runs inside the Slang call guard.
        unsafe { IComponentType::getSpecializationParamCount(raw) }
    });
    if count == 0 {
        Ok(())
    } else {
        Err(Error::invalid(
            Operation::CreateShaderProgram,
            format!(
                "the program has {count} specialization parameters, so specialize it before it is linked"
            ),
        ))
    }
}

impl Device {
    /// Creates a shader program from `program`, a linked Slang program of the
    /// session of this device.
    ///
    /// Load and link the program through [`Device::slang_session`], because
    /// slang-rhi generates the code of a program for the target of the
    /// program's own session, and only the device's session has the target
    /// that the device runs. A program of a session that
    /// [`Device::create_slang_session`] created goes to
    /// [`CompileSession::create_shader_program`](crate::CompileSession::create_shader_program)
    /// instead. slang-rhi uses the linked program as it is, with
    /// its entry points. The crate takes only a program without a
    /// specialization parameter left, such as one of a generic entry point or
    /// of a global parameter of an interface type, so specialize it with
    /// [`ComponentType::specialize`](slang_compiler::ComponentType::specialize)
    /// before it is linked.
    ///
    /// # Examples
    ///
    /// See the example of the [crate documentation](crate).
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when `program` belongs to another
    /// session or has a specialization parameter left. Returns an error of
    /// kind [`ErrorKind::Slang`](crate::ErrorKind::Slang) when the reflection of
    /// the program fails, and an error of kind
    /// [`ErrorKind::Compile`](crate::ErrorKind::Compile), with the messages of
    /// Slang as its diagnostics, when slang-rhi cannot create the program.
    pub fn create_shader_program(&self, program: &LinkedProgram) -> Result<ShaderProgram> {
        if program.session().as_raw() != self.slang_session().as_raw() {
            return Err(Error::invalid(
                Operation::CreateShaderProgram,
                "the program belongs to another session than the session of the device",
            ));
        }
        self.shader_program(program, self.slang_session(), None)
    }

    /// Creates a shader program from `program`, a linked program of
    /// `session`, after the caller has checked that `session` is the session
    /// of the device or one with its settings, which `compile_session` holds
    /// for the shader program when it is not the session of the device.
    pub(crate) fn shader_program(
        &self,
        program: &LinkedProgram,
        session: &Session,
        compile_session: Option<Session>,
    ) -> Result<ShaderProgram> {
        check_specialized(program)?;
        let thread_group_size = compute_thread_group_size(program, session)?;
        let desc = slang_rhi_sys::ShaderProgramDesc {
            linkingStyle: slang_rhi_sys::LinkingStyle::SingleProgram,
            slangGlobalScope: program.as_raw().as_ptr(),
            slangEntryPoints: ptr::null_mut(),
            slangEntryPointCount: 0,
            ..slang_rhi_sys::ShaderProgramDesc::default()
        };
        let device = self.as_ptr();
        let mut out: *mut IShaderProgram = ptr::null_mut();
        let mut blob: *mut ISlangBlob = ptr::null_mut();
        let (result, messages) = guard::call_collecting("IDevice::createShaderProgram", || {
            // SAFETY: `device` is live while `self` lives, `desc` is a live
            // descriptor whose global scope is the live linked program of a
            // session with the target and settings of the device's session,
            // which slang-rhi keeps a reference of, and `out` and `blob` are
            // live out-pointers.
            unsafe {
                IDevice::createShaderProgram(device, &raw const desc, &raw mut out, &raw mut blob)
            }
        });
        let mut text = NonNull::new(blob).and_then(|blob| {
            // SAFETY: slang-rhi returns the diagnostics blob with one
            // reference added for the caller, created by the Slang that
            // `slang-compiler` links through the session of the program, and
            // the blob is used on this thread only.
            unsafe { Blob::from_raw(blob, session.clone()) }
                .ok()
                .map(|blob| String::from_utf8_lossy(blob.as_bytes()).into_owned())
                .filter(|text| !text.is_empty())
        });
        if text.is_none() {
            text = diagnostics(&messages);
        }
        error::check(Operation::CreateShaderProgram, result, text)?;
        // SAFETY: the call succeeded, and slang-rhi returns the program with
        // one reference added for the caller.
        let raw = unsafe { Owned::adopt(out) }
            .ok_or_else(|| Error::null(Operation::CreateShaderProgram))?;
        Ok(ShaderProgram {
            raw,
            _session: compile_session,
            device: self.clone(),
            thread_group_size,
        })
    }

    /// Creates a compute pipeline from `program`, which compiles the code of
    /// its compute kernel for the device.
    ///
    /// # Examples
    ///
    /// This example reports the messages of Slang when the code of a program
    /// does not compile for the device:
    ///
    /// ```
    /// use slang_rhi::{ComputePipeline, Device, ErrorKind, ShaderProgram};
    ///
    /// fn pipeline(device: &Device, program: &ShaderProgram) -> slang_rhi::Result<ComputePipeline> {
    ///     device.create_compute_pipeline(program).inspect_err(|error| {
    ///         if error.kind() == ErrorKind::Compile {
    ///             eprintln!("{}", error.diagnostics().unwrap_or("no diagnostics"));
    ///         }
    ///     })
    /// }
    /// # let _ = pipeline;
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when `program` belongs to another
    /// device, when its linked program does not have exactly one entry point
    /// that is a compute kernel, or when the thread group of that kernel is
    /// larger along an axis than
    /// [`DeviceInfo::max_compute_thread_group_size`](crate::DeviceInfo::max_compute_thread_group_size)
    /// or has more threads than
    /// [`DeviceInfo::max_compute_threads_per_group`](crate::DeviceInfo::max_compute_threads_per_group).
    /// Returns an error of kind
    /// [`ErrorKind::Compile`](crate::ErrorKind::Compile), with the messages of
    /// Slang as its diagnostics, when slang-rhi cannot compile the program or
    /// create the pipeline.
    pub fn create_compute_pipeline(&self, program: &ShaderProgram) -> Result<ComputePipeline> {
        if program.device.as_non_null() != self.as_non_null() {
            return Err(Error::invalid(
                Operation::CreateComputePipeline,
                "the shader program belongs to another device",
            ));
        }
        let Some(size) = program.thread_group_size else {
            return Err(Error::invalid(
                Operation::CreateComputePipeline,
                "the shader program does not have exactly one entry point that is a compute kernel",
            ));
        };
        let (max_size, max_threads) = self.thread_group_limits();
        check_thread_group_size(size, max_size, max_threads)?;
        let desc = slang_rhi_sys::ComputePipelineDesc {
            program: program.raw.as_ptr(),
            ..slang_rhi_sys::ComputePipelineDesc::default()
        };
        let device = self.as_ptr();
        let mut out: *mut IComputePipeline = ptr::null_mut();
        let (result, messages) = guard::call_collecting("IDevice::createComputePipeline", || {
            // SAFETY: `device` is live while `self` lives, `desc` is a live
            // descriptor whose program is a live program of this device with
            // one compute entry point, which slang-rhi keeps a reference of,
            // and `out` is a live out-pointer.
            unsafe { IDevice::createComputePipeline(device, &raw const desc, &raw mut out) }
        });
        error::check(
            Operation::CreateComputePipeline,
            result,
            diagnostics(&messages),
        )?;
        // SAFETY: the call succeeded, and slang-rhi returns the pipeline with
        // one reference added for the caller.
        let raw = unsafe { Owned::adopt(out) }
            .ok_or_else(|| Error::null(Operation::CreateComputePipeline))?;
        Ok(ComputePipeline {
            raw,
            device: self.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{check_thread_group_size, diagnostics};
    use crate::ErrorKind;

    #[test]
    fn a_thread_group_within_the_limits_is_accepted() {
        assert!(check_thread_group_size([1024, 1, 1], [1024, 1024, 64], 1024).is_ok());
        assert!(check_thread_group_size([8, 8, 16], [1024, 1024, 64], 1024).is_ok());
    }

    #[test]
    fn a_thread_group_over_the_limit_of_an_axis_names_the_axis_and_the_limit() {
        let error = check_thread_group_size([1, 1, 65], [1024, 1024, 64], 1024).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            "cannot create a compute pipeline: the thread group of the kernel has 65 threads along the axis z, more than the 64 that the device allows"
        );
    }

    #[test]
    fn a_thread_group_over_the_threads_of_one_group_names_its_size_and_the_limit() {
        let error = check_thread_group_size([64, 32, 1], [1024, 1024, 64], 1024).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            "cannot create a compute pipeline: the thread group of the kernel has 64 by 32 by 1 threads, more than the 1024 threads of one group that the device allows"
        );
    }

    #[test]
    fn a_thread_group_whose_thread_count_overflows_is_refused() {
        let side = usize::try_from(u32::MAX).unwrap_or(usize::MAX);
        let error = check_thread_group_size([side, side, 2], [u32::MAX; 3], u32::MAX).unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    }

    #[test]
    fn diagnostics_joins_the_messages_with_new_lines_and_is_none_without_any() {
        assert_eq!(diagnostics(&[]), None);
        assert_eq!(
            diagnostics(&[String::from("first"), String::from("second")]),
            Some(String::from("first\nsecond"))
        );
    }
}
