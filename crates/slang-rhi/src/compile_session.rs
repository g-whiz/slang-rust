use std::ffi::{CStr, CString};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use slang_compiler::{
    CompilerOptions, GlobalSession, LinkedProgram, MatrixLayout, OptionName, Session, Target,
    TargetFormat, Toggle,
};
use slang_compiler_sys::{CompilerOptionName, IGlobalSession, SlangCapabilityID};
use slang_rhi_sys::{Capability, Feature, IDevice, IRHI};

use crate::device::{Device, DeviceType};
use crate::error::{self, Error, Operation, Result};
use crate::guard;
use crate::instance;
use crate::program::ShaderProgram;

/// A Slang session of `slang-compiler` with the target and the settings of
/// the session of a device, whose programs the device takes.
///
/// Create one with [`Device::create_slang_session`], load and link programs
/// through [`CompileSession::session`], and create their shader programs with
/// [`CompileSession::create_shader_program`]. A program that loads each
/// shader in a session of its own, such as to load a changed file again,
/// uses one compile session for each load. Dropping the compile session, its
/// programs, and their shader programs releases the modules that it loaded.
///
/// A compile session holds its [`Device`], so the device lives at least as
/// long as the compile session. [`Clone`] returns another handle to the
/// same compile session, with the same Slang session. A compile session is
/// neither [`Send`] nor [`Sync`], because it holds a session of
/// `slang-compiler` and a device, which are neither.
///
/// # Examples
///
/// See the example of [`Device::create_slang_session`].
///
/// A compile session cannot move to another thread, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_send<T: Send>() {}
/// assert_send::<slang_rhi::CompileSession>();
/// ```
///
/// It cannot be shared between threads either, so this example does not
/// compile:
///
/// ```compile_fail,E0277
/// fn assert_sync<T: Sync>() {}
/// assert_sync::<slang_rhi::CompileSession>();
/// ```
#[derive(Clone, Debug)]
pub struct CompileSession {
    session: Session,
    device: Device,
}

impl CompileSession {
    /// Returns the session of `slang-compiler`, through which a program loads
    /// and links the modules of its shader programs.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::create_slang_session`].
    #[must_use]
    pub fn session(&self) -> &Session {
        &self.session
    }

    /// Creates a shader program of the device of this compile session from
    /// `program`, a linked Slang program of [`CompileSession::session`].
    ///
    /// It takes the programs that [`Device::create_shader_program`] takes,
    /// but from the session of this compile session instead of the session
    /// of the device. The shader program holds the session, so the session
    /// lives at least as long as the shader program.
    ///
    /// # Examples
    ///
    /// See the example of [`Device::create_slang_session`].
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::InvalidArgument`](crate::ErrorKind::InvalidArgument),
    /// before any call into slang-rhi, when `program` belongs to another
    /// session, such as the session of the device or of another compile
    /// session, or has a specialization parameter left. Returns the other
    /// errors of [`Device::create_shader_program`].
    pub fn create_shader_program(&self, program: &LinkedProgram) -> Result<ShaderProgram> {
        if program.session().as_raw() != self.session.as_raw() {
            return Err(Error::invalid(
                Operation::CreateShaderProgram,
                "the program belongs to another session than the session of the compile session",
            ));
        }
        self.device
            .shader_program(program, &self.session, Some(self.session.clone()))
    }
}

/// The target of the session of a device, as slang-rhi chooses it for the
/// type of the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SessionTarget {
    /// The format of the code that the device runs.
    format: TargetFormat,
    /// The name of the profile, or `None` for the profile that Slang chooses.
    profile: Option<&'static str>,
    /// The macro that slang-rhi defines as `1` for the backend.
    define: &'static str,
}

/// The shader models of DXIL in ascending order, each with the feature by
/// which a device of Direct3D 12 reports it and the name of its profile.
const SHADER_MODELS: [(Feature, &str); 11] = [
    (Feature::SM_6_0, "sm_6_0"),
    (Feature::SM_6_1, "sm_6_1"),
    (Feature::SM_6_2, "sm_6_2"),
    (Feature::SM_6_3, "sm_6_3"),
    (Feature::SM_6_4, "sm_6_4"),
    (Feature::SM_6_5, "sm_6_5"),
    (Feature::SM_6_6, "sm_6_6"),
    (Feature::SM_6_7, "sm_6_7"),
    (Feature::SM_6_8, "sm_6_8"),
    (Feature::SM_6_9, "sm_6_9"),
    (Feature::SM_6_10, "sm_6_10"),
];

/// Returns the target with which slang-rhi creates the session of a device
/// of the type `device_type`, where `has_feature` says whether the device
/// reports a feature.
///
/// slang-rhi gives a Vulkan device a SPIR-V target with no profile and
/// defines `__VULKAN__`. It gives a Direct3D 12 device the highest shader
/// model that the device reports, which is DXIL from shader model 6.0 and
/// defines `__D3D12__`. A device below shader model 6.0 compiles to DXBC,
/// which `slang-compiler` has no target format for.
fn session_target(
    device_type: DeviceType,
    has_feature: impl Fn(Feature) -> bool,
) -> Result<SessionTarget> {
    match device_type {
        DeviceType::Vulkan => Ok(SessionTarget {
            format: TargetFormat::Spirv,
            profile: None,
            define: "__VULKAN__",
        }),
        DeviceType::D3d12 => SHADER_MODELS
            .iter()
            .rev()
            .find(|(feature, _)| has_feature(*feature))
            .map(|(_, profile)| SessionTarget {
                format: TargetFormat::Dxil,
                profile: Some(profile),
                define: "__D3D12__",
            })
            .ok_or_else(|| {
                Error::unsupported(
                    Operation::CreateSlangSession,
                    "the Direct3D 12 device reports no shader model of 6.0 or later, so its session compiles to DXBC, for which slang-compiler has no target format",
                )
            }),
        DeviceType::Default
        | DeviceType::D3d11
        | DeviceType::Metal
        | DeviceType::Cpu
        | DeviceType::Cuda
        | DeviceType::Wgpu => Err(Error::unsupported(
            Operation::CreateSlangSession,
            format!("the crate creates no Slang session for a device of the type {device_type:?}"),
        )),
    }
}

/// Returns whether the device reports `feature`.
fn has_feature(device: NonNull<IDevice>, feature: Feature) -> bool {
    guard::call("IDevice::hasFeature", || {
        // SAFETY: `device` is live while the `Device` that the caller
        // borrows lives, and `feature` is a value of the enumeration.
        unsafe { IDevice::hasFeature(device.as_ptr(), feature) }
    })
}

/// Returns the capabilities of the device in ascending order, as slang-rhi
/// lists them when it creates the session of the device.
fn capabilities(device: NonNull<IDevice>) -> Result<Vec<Capability>> {
    let mut count = 0_u32;
    let result = guard::call("IDevice::getCapabilities", || {
        // SAFETY: `device` is live, `count` is a live out-pointer, and a null
        // array asks for the count only.
        unsafe { IDevice::getCapabilities(device.as_ptr(), &raw mut count, std::ptr::null_mut()) }
    });
    error::check(Operation::CreateSlangSession, result, None)?;
    let length = usize::try_from(count).map_err(|error| {
        Error::invalid(
            Operation::CreateSlangSession,
            format!("slang-rhi reported {count} capabilities: {error}"),
        )
    })?;
    let mut capabilities = vec![Capability(0); length];
    let result = guard::call("IDevice::getCapabilities", || {
        // SAFETY: `device` is live, `count` holds the length of
        // `capabilities`, and slang-rhi writes at most that many values into
        // it, failing when the device has more.
        unsafe {
            IDevice::getCapabilities(device.as_ptr(), &raw mut count, capabilities.as_mut_ptr())
        }
    });
    error::check(Operation::CreateSlangSession, result, None)?;
    Ok(capabilities)
}

/// Returns the name of each capability, without the capabilities that
/// slang-rhi names with null.
fn capability_names(capabilities: &[Capability]) -> Result<Vec<CString>> {
    instance::with_instance(
        Operation::CreateSlangSession,
        "Device::create_slang_session",
        |rhi| {
            let mut names = Vec::with_capacity(capabilities.len());
            for capability in capabilities {
                let name = guard::call("IRHI::getCapabilityName", || {
                    // SAFETY: `rhi` is the live instance, and the lock of the
                    // crate is held.
                    unsafe { IRHI::getCapabilityName(rhi.as_ptr(), *capability) }
                });
                if !name.is_null() {
                    // SAFETY: a non-null name is a NUL-terminated string of a
                    // static table of slang-rhi, which the copy reads during
                    // this call.
                    names.push(unsafe { CStr::from_ptr(name) }.to_owned());
                }
            }
            Ok(names)
        },
    )
}

/// Returns the identifier of each capability that Slang knows by `names`,
/// in the same order, and leaves out every name that Slang does not know, as
/// slang-rhi does.
fn capability_ids(global: &GlobalSession, names: &[CString]) -> Vec<SlangCapabilityID> {
    let raw = global.as_raw();
    names
        .iter()
        .map(|name| {
            guard::call("IGlobalSession::findCapability", || {
                // SAFETY: `raw` points to the live global session that
                // `global` holds, used on this thread only, the call runs
                // inside the Slang call guard, and `name` is a C string that
                // lives through the call.
                unsafe { IGlobalSession::findCapability(raw.as_ptr(), name.as_ptr()) }
            })
        })
        .filter(|id| *id != SlangCapabilityID::SLANG_CAPABILITY_UNKNOWN)
        .collect()
}

impl Device {
    /// Creates a new Slang session with the target and the settings of the
    /// session of this device, and with the search paths `search_paths`, for
    /// the modules that a program loads by name or imports.
    ///
    /// `search_paths` is any sequence of paths, such as an array of `&str` or
    /// a `Vec<PathBuf>`. A program without search paths passes an empty
    /// sequence whose type names a path type, such as
    /// `std::iter::empty::<&Path>()`.
    ///
    /// The session gets the format, the profile, and the capabilities that
    /// the device reports, and the settings with which slang-rhi creates the
    /// session of the device: the row-major matrix layout, the scalar layout
    /// of buffers, the macro `__VULKAN__` or `__D3D12__` defined as `1`, and
    /// the defaults of Slang for everything else. A program of the session
    /// therefore gets the same layouts and code as in [`Device::slang_session`],
    /// and its shader programs come from
    /// [`CompileSession::create_shader_program`].
    ///
    /// Use a compile session to load each shader in a session of its own,
    /// which the device's one session cannot do once it has loaded a module
    /// of the same name.
    ///
    /// # Examples
    ///
    /// This example compiles a kernel in a compile session and creates its
    /// compute pipeline. It needs a Vulkan driver on Linux or a Direct3D 12
    /// adapter on Windows at run time, so it is compiled but not run:
    ///
    /// ```no_run
    /// use std::path::Path;
    ///
    /// use slang_compiler::{Component, GlobalSession};
    /// use slang_rhi::{Device, DeviceOptions};
    ///
    /// let device = Device::new(&DeviceOptions::new(GlobalSession::new()?))?;
    /// let compile = device.create_slang_session([Path::new("shaders")])?;
    /// let session = compile.session();
    /// let module = session.load_module("double")?.into_value();
    /// let entry_point = module.entry_point("main")?;
    /// let linked = session
    ///     .compose(&[Component::Module(&module), Component::EntryPoint(&entry_point)])?
    ///     .into_value()
    ///     .link()?
    ///     .into_value();
    /// let program = compile.create_shader_program(&linked)?;
    /// let pipeline = device.create_compute_pipeline(&program)?;
    /// # drop(pipeline);
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    ///
    /// # Errors
    ///
    /// Returns an error of kind
    /// [`ErrorKind::Unsupported`](crate::ErrorKind::Unsupported) for a device
    /// other than Vulkan or a Direct3D 12 device of shader model 6.0 or later,
    /// an error of kind [`ErrorKind::Slang`](crate::ErrorKind::Slang) when
    /// `slang-compiler` cannot create the session, such as for a search path
    /// with a NUL byte, and the error that slang-rhi reports for the lookup of
    /// the capabilities of the device.
    ///
    /// # Panics
    ///
    /// Panics when it is called from the debug callback of a device while a
    /// call of this crate into slang-rhi is in progress on the same thread.
    /// The panic aborts the process at the boundary of the callback.
    pub fn create_slang_session(
        &self,
        search_paths: impl IntoIterator<Item = impl AsRef<Path>>,
    ) -> Result<CompileSession> {
        let search_paths: Vec<PathBuf> = search_paths
            .into_iter()
            .map(|path| path.as_ref().to_path_buf())
            .collect();
        self.slang_session_with(&search_paths)
    }

    fn slang_session_with(&self, search_paths: &[PathBuf]) -> Result<CompileSession> {
        let slang = |error| Error::slang(Operation::CreateSlangSession, error);
        let device = self.as_non_null();
        let target = session_target(self.info().device_type(), |feature| {
            has_feature(device, feature)
        })?;
        let names = capability_names(&capabilities(device)?)?;
        let ids = capability_ids(self.global_session(), &names);
        let capability = OptionName::new(CompilerOptionName::Capability.0).map_err(slang)?;
        let mut options = CompilerOptions::default();
        for id in ids {
            // SAFETY: Slang reads the option `Capability` as an integer, the
            // identifier of a capability, and `id` is one that Slang returned
            // for a name that it knows.
            unsafe { options.set_int(capability, id.0, 0) };
        }
        let mut target_options = CompilerOptions::default();
        target_options.glsl_force_scalar_layout(Toggle::On);
        let mut slang_target = Target::new(target.format).options(target_options);
        if let Some(profile) = target.profile {
            slang_target = slang_target.profile(profile);
        }
        let mut builder = Session::builder(self.global_session())
            .target(slang_target)
            .define(target.define, "1")
            .matrix_layout(MatrixLayout::RowMajor)
            .options(options);
        for path in search_paths {
            builder = builder.search_path(path);
        }
        let session = builder.build().map_err(slang)?;
        Ok(CompileSession {
            session,
            device: self.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use slang_compiler::TargetFormat;
    use slang_rhi_sys::Feature;

    use super::{SHADER_MODELS, SessionTarget, session_target};
    use crate::ErrorKind;
    use crate::device::DeviceType;
    use crate::error::Operation;

    #[test]
    fn a_vulkan_device_gets_spirv_with_no_profile_and_its_macro() {
        let target = session_target(DeviceType::Vulkan, |_| true).expect("Vulkan");
        assert_eq!(
            target,
            SessionTarget {
                format: TargetFormat::Spirv,
                profile: None,
                define: "__VULKAN__",
            }
        );
    }

    #[test]
    fn a_direct3d_12_device_gets_dxil_with_its_highest_shader_model() {
        let up_to = |highest: Feature| {
            move |feature: Feature| {
                feature == Feature::SM_5_1
                    || (feature.0 <= highest.0 && feature.0 >= Feature::SM_6_0.0)
            }
        };
        for (feature, profile) in SHADER_MODELS {
            let target = session_target(DeviceType::D3d12, up_to(feature)).expect("DXIL");
            assert_eq!(
                target,
                SessionTarget {
                    format: TargetFormat::Dxil,
                    profile: Some(profile),
                    define: "__D3D12__",
                }
            );
        }
    }

    #[test]
    fn a_direct3d_12_device_below_shader_model_6_and_other_types_are_unsupported() {
        let dxbc = session_target(DeviceType::D3d12, |feature| feature == Feature::SM_5_1)
            .expect_err("DXBC");
        assert_eq!(dxbc.kind(), ErrorKind::Unsupported);
        assert_eq!(dxbc.operation(), Operation::CreateSlangSession);
        for device_type in [
            DeviceType::Default,
            DeviceType::D3d11,
            DeviceType::Metal,
            DeviceType::Cpu,
            DeviceType::Cuda,
            DeviceType::Wgpu,
        ] {
            let error = session_target(device_type, |_| true).expect_err("unsupported");
            assert_eq!(error.kind(), ErrorKind::Unsupported, "{device_type:?}");
        }
    }
}
