//! Shares one wgpu device between Slint and `slango`, so that what a Slang
//! shader draws can appear inside a Slint user interface.
//!
//! [Slint](https://slint.dev) is a toolkit for graphical user interfaces,
//! and [`slango`] is the high-level library of this repository, which runs
//! Slang compute kernels and draws with Slang render pipelines through wgpu.
//! Use this crate when a Slint program draws part of its window with Slang,
//! and `slango` alone when the program has no Slint user interface.
//!
//! [`wgpu_configuration`] returns the configuration that the program passes
//! to Slint's backend selector, so that Slint creates a wgpu device on which
//! every kernel and render pipeline of `slango` can run. [`device`] turns the
//! wgpu device and queue that Slint passes to the rendering notifier of a
//! window into a [`slango::Device`]. The rendering notifier is the callback
//! that the program registers with [`slint::Window::set_rendering_notifier`],
//! which Slint calls when it sets up rendering, before and after each frame,
//! and when it tears rendering down. A [`View`] draws with a render pipeline
//! of `slango`, the pair of a vertex and a fragment entry point that place
//! triangles and colour their pixels, into a texture that Slint shows as an
//! image. Every function of this crate that can fail returns an [`Error`],
//! whose [`Error::kind`] says what went wrong. The draw that [`View::draw`]
//! starts is a [`slango::Draw`], so a failed draw returns a
//! [`slango::Error`].
//!
//! A program that creates the wgpu device itself can instead pass it to
//! Slint with
//! [`WGPUConfiguration::Manual`](slint::wgpu_30::WGPUConfiguration::Manual)
//! and turn it into a [`slango::Device`] with
//! [`slango::wgpu::from_device_and_queue`], which needs nothing from this
//! crate. With [`wgpu_configuration`], the program needs no code of its own
//! to create the device.
//!
//! The crate enables no windowing backend and no renderer of Slint. The
//! program enables a windowing backend, such as Slint's feature
//! `backend-winit`, and a renderer on wgpu, such as `renderer-femtovg-wgpu`,
//! because a renderer on OpenGL gives no wgpu device to share. The crate
//! installs no rendering notifier, because a window has only one, and the
//! program needs it. The crate contains no unsafe code.
//!
//! # Examples
//!
//! This example shows what a Slang shader draws in a Slint window. The file
//! `/tmp/shaders/gradient.slang` holds a vertex entry point `vsMain` and a
//! fragment entry point `fsMain` that draw one triangle over the whole
//! target with no vertex buffer and no parameters. The program makes Slint
//! render with a device that suits `slango`. When Slint sets up rendering,
//! it creates the `slango` device, the render pipeline, and a view of the
//! size of the `Image` element, and sets the view's image on the element.
//! Before each frame, it resizes the view to the element and draws, and
//! after each frame, it requests the next one. It needs a windowing backend
//! and a renderer of Slint on wgpu, which the program enables, the shader
//! file, a GPU driver or a software driver, and a display at run time, so it
//! is compiled but not run:
//!
//! ```no_run
//! use std::cell::RefCell;
//! use std::error::Error;
//! use std::rc::Rc;
//!
//! use slang_slint::View;
//! use slango::RenderPipelineOptions;
//! use slint::{ComponentHandle, RenderingState};
//!
//! slint::slint! {
//!     export component App inherits Window {
//!         in property <image> view;
//!         out property <length> view-width: area.width;
//!         out property <length> view-height: area.height;
//!         area := Rectangle {
//!             Image {
//!                 width: parent.width;
//!                 height: parent.height;
//!                 source: root.view;
//!             }
//!         }
//!     }
//! }
//!
//! fn physical_size(app: &App) -> [u32; 2] {
//!     let scale_factor = app.window().scale_factor();
//!     [app.get_view_width(), app.get_view_height()]
//!         .map(|length| ((length * scale_factor).round() as u32).max(1))
//! }
//!
//! fn set_up(app: &App, api: &slint::GraphicsAPI<'_>) -> Result<View, Box<dyn Error>> {
//!     let device = slang_slint::device(api)?;
//!     let module = device.load_module("/tmp/shaders/gradient.slang")?;
//!     let pipeline = module.render_pipeline("vsMain", "fsMain", &RenderPipelineOptions::default())?;
//!     let view = View::new(device, pipeline, physical_size(app))?;
//!     app.set_view(view.to_image()?);
//!     Ok(view)
//! }
//!
//! fn frame(app: &App, view: &mut View) -> Result<(), Box<dyn Error>> {
//!     if view.resize(physical_size(app))? {
//!         app.set_view(view.to_image()?);
//!     }
//!     view.draw().submit(3, 1)?;
//!     Ok(())
//! }
//!
//! slint::BackendSelector::new()
//!     .require_wgpu_30(slang_slint::wgpu_configuration())
//!     .select()?;
//! let app = App::new()?;
//! let weak = app.as_weak();
//! let view: Rc<RefCell<Option<View>>> = Rc::default();
//! app.window().set_rendering_notifier(move |state, api| {
//!     let Some(app) = weak.upgrade() else { return };
//!     let mut view = view.borrow_mut();
//!     let result = match state {
//!         RenderingState::RenderingSetup => set_up(&app, api).map(|new| *view = Some(new)),
//!         RenderingState::BeforeRendering => match view.as_mut() {
//!             Some(view) => frame(&app, view),
//!             None => Ok(()),
//!         },
//!         RenderingState::AfterRendering => {
//!             app.window().request_redraw();
//!             Ok(())
//!         }
//!         RenderingState::RenderingTeardown => {
//!             *view = None;
//!             Ok(())
//!         }
//!         _ => Ok(()),
//!     };
//!     if let Err(error) = result {
//!         eprintln!("{error}");
//!     }
//! })?;
//! app.run()?;
//! # Ok::<(), Box<dyn Error>>(())
//! ```
//!
//! The package `slang-rust-examples` in the repository of this crate holds
//! the complete program `shader-playground`, which shows a shader in a Slint
//! window with two sliders whose values it sets on the shader by name, and
//! loads the shader again whenever its file changes.
//!
//! # Driving frames
//!
//! Slint renders a frame only when something requests one. A program draws
//! in its rendering notifier when Slint passes
//! [`slint::RenderingState::BeforeRendering`], and requests the next frame
//! with [`slint::Window::request_redraw`] when Slint passes
//! [`slint::RenderingState::AfterRendering`], as the example does. It can
//! instead draw and request a redraw from a repeated [`slint::Timer`] on the
//! thread that runs Slint's event loop. A program that draws only when
//! something changes, such as the value of a slider, requests a redraw only
//! then.
//!
//! Before each draw, the program passes the size of the `Image` element in
//! physical pixels, the pixels of the window's surface, to [`View::resize`].
//! A length in a `.slint` file is in logical pixels, so the program reads
//! the width and the height of the element from `out` properties, multiplies
//! them by [`slint::Window::scale_factor`], and rounds them. When
//! [`View::resize`] returns `true`, the program sets the image of
//! [`View::to_image`] on the element again. The `Image` element fills a
//! parent element whose size the layout sets, such as a `Rectangle`, so that
//! the size of the texture does not feed back into the layout.
//!
//! An element with no area gives [`ErrorKind::InvalidSize`]. The program
//! then skips resizing and drawing until the element has an area, or keeps
//! at least one pixel on each side, as the example does.
//!
//! # Threading
//!
//! A [`View`], an [`Error`], and an [`ErrorKind`] are [`Send`] and [`Sync`].
//! The [`slint::Image`] that [`View::to_image`] returns is neither, so the
//! program creates it and sets it on the thread that runs Slint's event
//! loop. The program draws on that thread as well, in its rendering notifier
//! or in a timer. The crate starts no thread.
//!
//! # Features
//!
//! This crate has no Cargo features.
//!
//! # Licence
//!
//! The source of this crate is under version 3 or any later version of the
//! GNU General Public License. Slint is available under several licences, and
//! this crate takes it under version 3 of the GNU General Public License only.
//! A program that combines this crate with Slint under that option is
//! therefore distributed under version 3 of the GNU General Public License
//! only. `slango` and `slang-compiler` do not depend on Slint, so their
//! licence does not change.
//!
//! # Platform support
//!
//! The crate supports the platforms of `slango`, which are Linux on x86-64,
//! Windows on x86-64, and macOS on Apple silicon and on x86-64, with the
//! wgpu 30 that Slint and `slango` share. On Linux, building it needs the
//! development files of fontconfig, which Slint links. With the
//! configuration of [`wgpu_configuration`], Slint refuses an adapter that
//! runs on the CPU, such as lavapipe, unless the environment variable
//! `SLINT_WGPU_CPU` is `1`.
//!
//! # Finding the Slang library
//!
//! `slango` links the Slang library through `slang-compiler-sys`, as the
//! [crate documentation of `slango`](slango) describes.

mod device;
mod error;
mod view;

pub use device::{device, wgpu_configuration};
pub use error::{Error, ErrorKind, Result};
pub use view::View;
