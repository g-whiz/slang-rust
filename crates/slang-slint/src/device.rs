use slint::GraphicsAPI;
use slint::wgpu_30::wgpu::{Features, Limits};
use slint::wgpu_30::{WGPUConfiguration, WGPUSettings};

use crate::error::{Error, ErrorKind, Result};

const MAX_IMMEDIATE_SIZE: u32 = 128;

/// Returns the configuration with which Slint creates a wgpu device that
/// `slango` can use fully.
///
/// Pass it to Slint's backend selector with `require_wgpu_30` before the
/// program creates its first window. A device that Slint creates with its own
/// defaults has no immediates and no storage buffers, so a kernel of `slango`
/// with a `uniform` parameter of its entry point or with a buffer does not
/// load on it. With this configuration, Slint requests the immediates
/// feature, through which a kernel receives the `uniform` parameters of its
/// entry point, with 128 bytes of immediate data, and wgpu's default limits
/// for everything else. Every other setting keeps Slint's default.
///
/// Slint chooses the adapter only after it receives the configuration, so the
/// size of immediate data is fixed and not the largest that the adapter
/// allows, as it is for a device of [`slango::Device::new`]. On an adapter
/// that offers no immediates or fewer than 128 bytes of them, Slint cannot
/// create the device, and the program passes a configuration of its own
/// instead.
///
/// # Examples
///
/// This example makes Slint render with wgpu 30 on a device that suits
/// `slango`. It needs a windowing backend and a renderer of Slint on wgpu,
/// which the program enables, and a display at run time, so it is compiled
/// but not run:
///
/// ```no_run
/// slint::BackendSelector::new()
///     .require_wgpu_30(slang_slint::wgpu_configuration())
///     .select()?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[must_use]
pub fn wgpu_configuration() -> WGPUConfiguration {
    let mut settings = WGPUSettings::default();
    settings.device_required_features = Features::IMMEDIATES;
    settings.device_required_limits = Limits {
        max_immediate_size: MAX_IMMEDIATE_SIZE,
        ..Limits::default()
    };
    WGPUConfiguration::Automatic(settings)
}

/// Creates a [`slango::Device`] on the wgpu device and queue with which Slint
/// renders.
///
/// Call it in the rendering notifier of a Slint window, when Slint passes
/// [`slint::RenderingState::RenderingSetup`] and the graphics API, so that
/// `slango` draws on the device whose textures Slint can show. The device
/// shares the wgpu device and queue with Slint, and has the features and
/// limits with which Slint created it, which [`wgpu_configuration`] sets.
///
/// # Examples
///
/// This example creates the device when Slint sets up rendering. It needs a
/// window and a display at run time, so it is compiled but not run:
///
/// ```no_run
/// slint::slint! {
///     export component App inherits Window {}
/// }
///
/// let app = App::new()?;
/// app.window().set_rendering_notifier(|state, api| {
///     if let slint::RenderingState::RenderingSetup = state {
///         match slang_slint::device(api) {
///             Ok(device) => println!("slango runs on {}", device.adapter_name()),
///             Err(error) => eprintln!("{error}"),
///         }
///     }
/// })?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
///
/// Returns an [`Error`] of kind [`ErrorKind::UnsupportedGraphicsApi`] when
/// the graphics API is not wgpu 30, such as when Slint renders with OpenGL.
pub fn device(graphics_api: &GraphicsAPI<'_>) -> Result<slango::Device> {
    match graphics_api {
        GraphicsAPI::WGPU30 { device, queue, .. } => Ok(slango::wgpu::from_device_and_queue(
            device.clone(),
            queue.clone(),
        )),
        other => Err(Error::new(
            ErrorKind::UnsupportedGraphicsApi,
            "create a device from the graphics API of Slint",
        )
        .with_detail(format!("Slint renders with {other:?} and not with wgpu 30"))),
    }
}
