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
//! window into a [`slango::Device`]. A [`View`] draws with a render pipeline
//! of `slango` into a texture that Slint shows as an image. Every failure is
//! an [`Error`], whose [`Error::kind`] says what went wrong.
//!
//! The crate enables no windowing backend and no renderer of Slint. The
//! program enables a windowing backend, such as Slint's feature
//! `backend-winit`, and a renderer on wgpu, such as `renderer-femtovg-wgpu`,
//! because a renderer on OpenGL gives no wgpu device to share. The crate
//! contains no unsafe code.
//!
//! # Examples
//!
//! This example makes Slint render with a device that suits `slango`, and
//! creates the `slango` device on it when Slint sets up rendering. It needs
//! a windowing backend and a renderer of Slint on wgpu, which the program
//! enables, and a display at run time, so it is compiled but not run:
//!
//! ```no_run
//! slint::slint! {
//!     export component App inherits Window {}
//! }
//!
//! slint::BackendSelector::new()
//!     .require_wgpu_30(slang_slint::wgpu_configuration())
//!     .select()?;
//! let app = App::new()?;
//! app.window().set_rendering_notifier(|state, api| {
//!     if let slint::RenderingState::RenderingSetup = state {
//!         match slang_slint::device(api) {
//!             Ok(device) => println!("slango runs on {}", device.adapter_name()),
//!             Err(error) => eprintln!("{error}"),
//!         }
//!     }
//! })?;
//! app.run()?;
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
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
//! only.
//!
//! # Platform support
//!
//! The crate supports the platforms of `slango`, which are Linux on x86-64,
//! Windows on x86-64, and macOS on Apple silicon and on x86-64, with the
//! wgpu 30 that Slint and `slango` share. On Linux, building it needs the
//! development files of fontconfig, which Slint links.
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
