//! Shows what a Slang shader draws in a Slint window, with two sliders whose
//! values reach the shader by name, and reloads the shader whenever its file
//! changes.
//!
//! The example shows how `slang-slint` puts a render pipeline of `slango`
//! into a Slint user interface. It passes `slang_slint::wgpu_configuration`
//! to Slint's backend selector, so that Slint creates a wgpu device that
//! suits `slango`, and turns that device into a `slango::Device` with
//! `slang_slint::device` when Slint sets up rendering. It then loads the
//! Slang file given on the command line, creates a render pipeline from its
//! vertex entry point `vsMain` and its fragment entry point `fsMain`, and
//! creates a `slang_slint::View` of the size of the window's `Image`
//! element, whose image it sets on that element. Before each frame that
//! Slint renders, it resizes the view to the element and draws one triangle
//! that covers the view, with no vertex buffer, and with the global
//! uniforms `time`, `brightness`, and `scale` set by name, the last two from
//! the sliders. After each frame, it requests the next one.
//!
//! Before each frame, the example also compares the modification time of
//! the Slang file with the one it loaded. When the time changed, it loads
//! the file, creates the pipeline and a new view, and sets the view's image.
//! When the load fails, it prints the error and each error that caused it on
//! standard error and keeps drawing with the old view, so a shader can be
//! edited while the window shows the result of each save.
//!
//! Run it from the root of the workspace on the sample shader, on a desktop
//! with a display:
//!
//! ```sh
//! cargo run -p slang-rust-examples --bin shader-playground -- examples/shaders/playground.slang
//! ```
//!
//! With `--frames N`, the example closes the window after `N` frames, prints
//! one line on standard output, and exits, so that it runs without a person.
//! When it then draws no frame for ten seconds, as with a window that is
//! hidden, it exits with an error instead of waiting. The program needs a
//! GPU driver or a software driver, such as lavapipe on Linux. Slint refuses
//! an adapter that runs on the CPU, such as lavapipe, unless the environment
//! variable `SLINT_WGPU_CPU` is `1`. The program loads the Slang shared
//! library of the Vulkan SDK when it starts, so `$VULKAN_SDK/lib` has to be
//! on `LD_LIBRARY_PATH` on Linux or on `DYLD_LIBRARY_PATH` on macOS, and the
//! SDK's `Bin` directory on `PATH` on Windows.

#![expect(
    clippy::print_stdout,
    reason = "the example prints the number of frames that it drew on standard output"
)]
#![expect(
    clippy::print_stderr,
    reason = "the example prints the adapter, its reloads, and its errors on standard error"
)]

#[path = "../../shared/frames.rs"]
mod frames;
#[path = "../../shared/report.rs"]
mod report;
mod scene;
mod ui;

use std::cell::RefCell;
use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;
use std::rc::Rc;
use std::time::{Duration, Instant};

use slint::{ComponentHandle, RenderingState, Timer, TimerMode};

use crate::frames::{Frames, OPTIONS, parse_args};
use crate::report::report;
use crate::scene::{Scene, Uniforms};
use crate::ui::Playground;

const USAGE: &str = "\
Usage: shader-playground [--frames N] <FILE>

Shows what the entry points vsMain and fsMain of the Slang file FILE draw in a
Slint window, with sliders for the uniforms brightness and scale, and loads
FILE again whenever it changes.
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            report(&*error);
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let Some(args) = parse_args(std::env::args_os().skip(1))? else {
        print!("{USAGE}{OPTIONS}");
        return Ok(());
    };

    slint::BackendSelector::new()
        .require_wgpu_30(slang_slint::wgpu_configuration())
        .select()?;
    let app = Playground::new()?;
    let state = Rc::new(RefCell::new(State {
        file: args.file,
        frames: Frames::new(args.frames, Instant::now()),
        start: Instant::now(),
        scene: None,
        error: None,
    }));

    let weak = app.as_weak();
    let notified = Rc::clone(&state);
    app.window().set_rendering_notifier(move |rendering, api| {
        let Some(app) = weak.upgrade() else {
            return;
        };
        let mut state = notified.borrow_mut();
        let result = match rendering {
            RenderingState::RenderingSetup => state.set_up(&app, api),
            RenderingState::BeforeRendering => state.frame(&app),
            RenderingState::AfterRendering => {
                if state.running() {
                    app.window().request_redraw();
                }
                Ok(())
            }
            RenderingState::RenderingTeardown => {
                state.scene = None;
                Ok(())
            }
            _ => Ok(()),
        };
        if let Err(error) = result {
            state.fail(error);
        }
    })?;

    let watchdog = Timer::default();
    if args.frames.is_some() {
        let watched = Rc::clone(&state);
        watchdog.start(TimerMode::Repeated, Duration::from_secs(1), move || {
            let mut state = watched.borrow_mut();
            if state.running()
                && let Err(error) = state.frames.watch(Instant::now())
            {
                state.fail(error);
            }
        });
    }

    app.run()?;
    drop(watchdog);
    let mut state = state.borrow_mut();
    if let Some(error) = state.error.take() {
        return Err(error);
    }
    if let Some(summary) = state.frames.summary() {
        println!("{summary}");
    }
    Ok(())
}

struct State {
    file: PathBuf,
    frames: Frames,
    start: Instant,
    scene: Option<Scene>,
    error: Option<Box<dyn Error>>,
}

impl State {
    fn running(&self) -> bool {
        self.error.is_none() && self.frames.running()
    }

    fn fail(&mut self, error: Box<dyn Error>) {
        if self.error.is_none() {
            self.error = Some(error);
        }
        if let Err(error) = slint::quit_event_loop() {
            report(&error);
        }
    }

    fn set_up(
        &mut self,
        app: &Playground,
        api: &slint::GraphicsAPI<'_>,
    ) -> Result<(), Box<dyn Error>> {
        let device = slang_slint::device(api)?;
        eprintln!("running on {}", device.adapter_name());
        let scene = Scene::new(&device, &self.file, physical_size(app))?;
        app.set_view(scene.view().to_image()?);
        self.scene = Some(scene);
        self.frames.start(Instant::now());
        Ok(())
    }

    fn frame(&mut self, app: &Playground) -> Result<(), Box<dyn Error>> {
        if !self.running() {
            return Ok(());
        }
        let Some(scene) = self.scene.as_mut() else {
            return Ok(());
        };
        let mut replaced = match scene.reload_if_changed() {
            Ok(reloaded) => {
                if reloaded {
                    eprintln!("reloaded the shader");
                }
                reloaded
            }
            Err(error) => {
                report(&*error);
                false
            }
        };
        replaced |= scene.resize(physical_size(app))?;
        if replaced {
            app.set_view(scene.view().to_image()?);
        }
        scene.draw(Uniforms {
            time: self.start.elapsed().as_secs_f32(),
            brightness: app.get_brightness(),
            scale: app.get_scale(),
        })?;
        self.frames.drawn(Instant::now());
        if !self.running() {
            slint::quit_event_loop()?;
        }
        Ok(())
    }
}

/// Returns the size of the view's element in physical pixels, at least one
/// pixel on each side, so that a view exists while the element has no area.
fn physical_size(app: &Playground) -> [u32; 2] {
    let scale_factor = app.window().scale_factor();
    [app.get_view_width(), app.get_view_height()].map(|length| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a conversion of a float to u32 saturates, and the length of an element is far below u32::MAX"
        )]
        let pixels = (length * scale_factor).round() as u32;
        pixels.max(1)
    })
}
