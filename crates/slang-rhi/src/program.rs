use std::ptr::{self, NonNull};

use slang_compiler::reflection::{LayoutSize, ScalarType, TypeKind, VariableLayout};
use slang_compiler::{Blob, LinkedProgram, ParameterCategory, Session, Stage};
use slang_compiler_sys::{IComponentType, ISlangBlob};
use slang_rhi_sys::{IComputePipeline, IDevice, IShaderProgram};

use crate::device::Device;
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::owned::Owned;
use crate::render_pipeline::{InputType, Numbers, VertexInput};

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
/// [`Clone`] returns another handle to the same shader program, which adds a
/// reference to it in slang-rhi.
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
#[derive(Clone, Debug)]
pub struct ShaderProgram {
    raw: Owned<IShaderProgram>,
    _session: Option<Session>,
    device: Device,
    /// What the entry points of the program are, which the creation of a
    /// pipeline checks.
    entry_points: EntryPoints,
    /// The varying inputs of the vertex entry point of the program, which
    /// the creation of a render pipeline checks its vertex streams against.
    vertex_inputs: Vec<VertexInput>,
}

/// What the entry points of a linked program are.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryPoints {
    /// Exactly one entry point, a compute kernel with this thread group size.
    Compute([usize; 3]),
    /// Exactly two entry points, one of the vertex stage and one of the
    /// fragment stage.
    Render,
    /// Any other set of entry points.
    Other,
}

impl ShaderProgram {
    pub(crate) fn as_ptr(&self) -> *mut IShaderProgram {
        self.raw.as_ptr()
    }

    pub(crate) fn device_ptr(&self) -> NonNull<IDevice> {
        self.device.as_non_null()
    }

    pub(crate) fn entry_points(&self) -> EntryPoints {
        self.entry_points
    }

    pub(crate) fn vertex_inputs(&self) -> &[VertexInput] {
        &self.vertex_inputs
    }
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
/// [`Clone`] returns another handle to the same compute pipeline, which adds a
/// reference to it in slang-rhi.
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
#[derive(Clone, Debug)]
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
pub(crate) fn diagnostics(messages: &[String]) -> Option<String> {
    if messages.is_empty() {
        None
    } else {
        Some(messages.join("\n"))
    }
}

/// Returns what the entry points of a linked program of `session` are, and
/// the varying inputs of its vertex entry points.
fn entry_points(
    program: &LinkedProgram,
    session: &Session,
) -> Result<(EntryPoints, Vec<VertexInput>)> {
    let count = program.entry_point_count();
    if count != 1 && count != 2 {
        return Ok((EntryPoints::Other, Vec::new()));
    }
    let slang = |error| Error::slang(Operation::CreateShaderProgram, error);
    let target = session.target_index(0).map_err(slang)?;
    let layout = program.layout(target).map_err(slang)?.into_value();
    let mut stages = Vec::new();
    let mut inputs = Vec::new();
    for entry_point in layout.entry_point_layouts() {
        let stage = entry_point.stage().map_err(slang)?;
        let size = if stage == Some(Stage::Compute) {
            entry_point.compute_thread_group_size()
        } else {
            [0; 3]
        };
        if stage == Some(Stage::Vertex) {
            for parameter in entry_point.parameter_layouts() {
                collect_inputs(parameter, None, 0, &mut inputs).map_err(slang)?;
            }
        }
        stages.push((stage, size));
    }
    Ok((classify_entry_points(&stages), inputs))
}

/// Adds the varying inputs of the parameter or field `variable` to `inputs`,
/// where `parent` is the name of the structure that holds a field and `base`
/// is the location of that structure.
///
/// A variable without varying input, such as a system value or a `uniform`
/// parameter, adds nothing, and a structure adds each of its fields.
fn collect_inputs(
    variable: VariableLayout<'_>,
    parent: Option<&str>,
    base: u64,
    inputs: &mut Vec<VertexInput>,
) -> slang_compiler::Result<()> {
    let Some(type_layout) = variable.type_layout() else {
        return Ok(());
    };
    let name = variable
        .variable()
        .map(slang_compiler::reflection::Variable::name)
        .transpose()?
        .flatten()
        .unwrap_or("");
    let name = match parent {
        Some(parent) => format!("{parent}.{name}"),
        None => name.to_owned(),
    };
    if type_layout.size(ParameterCategory::VaryingInput).amount == LayoutSize::Finite(0) {
        return Ok(());
    }
    let location = match variable.offset(ParameterCategory::VaryingInput).amount {
        LayoutSize::Finite(offset) => base
            .checked_add(offset)
            .filter(|location| u32::try_from(*location).is_ok()),
        _ => None,
    };
    let Some(location) = location else {
        inputs.push(VertexInput {
            name,
            location: u32::MAX,
            semantic: None,
            ty: InputType::Other(String::from("at a location that Slang does not report")),
        });
        return Ok(());
    };
    let kind = type_layout.kind()?;
    if kind == TypeKind::Struct {
        for field in type_layout.field_layouts() {
            collect_inputs(field, Some(&name), location, inputs)?;
        }
        return Ok(());
    }
    let Ok(location) = u32::try_from(location) else {
        return Ok(());
    };
    let semantic = variable
        .semantic_name()?
        .map(|semantic| (semantic.to_owned(), variable.semantic_index()));
    let semantic = match semantic {
        Some((semantic, index)) => u32::try_from(index).ok().map(|index| (semantic, index)),
        None => None,
    };
    let ty = match (
        kind,
        type_layout
            .ty()
            .map(slang_compiler::reflection::Type::scalar_type)
            .transpose()?,
    ) {
        (TypeKind::Scalar | TypeKind::Vector, Some(ScalarType::Float32)) => {
            InputType::Numbers(Numbers::Float)
        }
        (TypeKind::Scalar | TypeKind::Vector, Some(ScalarType::Uint32)) => {
            InputType::Numbers(Numbers::Unsigned)
        }
        (TypeKind::Scalar | TypeKind::Vector, Some(ScalarType::Int32)) => {
            InputType::Numbers(Numbers::Signed)
        }
        (TypeKind::Scalar | TypeKind::Vector, Some(scalar)) => {
            InputType::Other(format!("of the scalar type {scalar:?}"))
        }
        (kind, _) => InputType::Other(format!("of the kind {kind:?}")),
    };
    inputs.push(VertexInput {
        name,
        location,
        semantic,
        ty,
    });
    Ok(())
}

/// Classifies the stages of the entry points of a program, each with the
/// thread group size that reflection reports for it.
fn classify_entry_points(stages: &[(Option<Stage>, [usize; 3])]) -> EntryPoints {
    match stages {
        [(Some(Stage::Compute), size)] => EntryPoints::Compute(*size),
        [(Some(Stage::Vertex), _), (Some(Stage::Fragment), _)]
        | [(Some(Stage::Fragment), _), (Some(Stage::Vertex), _)] => EntryPoints::Render,
        _ => EntryPoints::Other,
    }
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
        let (entry_points, vertex_inputs) = entry_points(program, session)?;
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
            entry_points,
            vertex_inputs,
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
        let EntryPoints::Compute(size) = program.entry_points else {
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
    use slang_compiler::Stage;

    use super::{EntryPoints, check_thread_group_size, classify_entry_points, diagnostics};
    use crate::ErrorKind;

    #[test]
    fn classify_entry_points_tells_a_kernel_a_vertex_and_fragment_pair_and_the_rest() {
        let size = [8, 4, 1];
        assert_eq!(
            classify_entry_points(&[(Some(Stage::Compute), size)]),
            EntryPoints::Compute(size)
        );
        for pair in [
            [(Some(Stage::Vertex), size), (Some(Stage::Fragment), size)],
            [(Some(Stage::Fragment), size), (Some(Stage::Vertex), size)],
        ] {
            assert_eq!(classify_entry_points(&pair), EntryPoints::Render);
        }
        for other in [
            &[][..],
            &[(Some(Stage::Vertex), size)][..],
            &[(Some(Stage::Fragment), size)][..],
            &[(None, size)][..],
            &[(Some(Stage::Vertex), size), (Some(Stage::Vertex), size)][..],
            &[(Some(Stage::Compute), size), (Some(Stage::Fragment), size)][..],
            &[(Some(Stage::Vertex), size), (None, size)][..],
            &[
                (Some(Stage::Vertex), size),
                (Some(Stage::Fragment), size),
                (Some(Stage::Compute), size),
            ][..],
        ] {
            assert_eq!(
                classify_entry_points(other),
                EntryPoints::Other,
                "{other:?}"
            );
        }
    }

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
