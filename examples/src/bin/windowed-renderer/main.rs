//! Draws a rotating, textured quad in a window with a render pipeline of
//! `slango`, and reloads its Slang file whenever the file changes.
//!
//! The example shows how `slango` draws into a window that the program owns.
//! The window, the wgpu surface, the adapter, the device, and the frame loop
//! are the example's own winit and wgpu code. The example hands the wgpu
//! device and its queue to `slango` with `slango::wgpu::from_device_and_queue`,
//! loads the Slang file given on the command line, and creates a render
//! pipeline from its vertex entry point `vsMain` and its fragment entry point
//! `fsMain` for the format of the surface. In each frame, it wraps the view of
//! the surface texture with `slango::wgpu::texture_from_view` and draws a quad
//! with an index buffer and a checkerboard texture into it, rotated by a
//! `float4x4` transform whose angle grows with time.
//!
//! Before each frame, the example compares the modification time of the Slang
//! file with the one it loaded. When the time changed, it loads the file and
//! creates the pipeline again. When either fails, it prints the error and each
//! error that caused it on standard error and keeps drawing with the old
//! pipeline, so a shader can be edited while the window shows the result of
//! each save. Each reload creates a Slang global session, which takes about a
//! tenth of a second.
//!
//! Run it from the root of the workspace on the sample shader, on a desktop
//! with a display:
//!
//! ```sh
//! cargo run -p slang-rust-examples --bin windowed-renderer -- examples/shaders/textured-quad.slang
//! ```
//!
//! With `--frames N`, the example closes the window after `N` frames, prints
//! one line on standard output, and exits, so that it runs without a person.
//! When it then draws no frame for ten seconds, as with a window that is
//! hidden or has a size of zero, it exits with an error instead of waiting.
//! The program needs a GPU driver or a software driver, such as lavapipe on
//! Linux. It loads the Slang shared library of the Vulkan SDK when it starts,
//! so `$VULKAN_SDK/lib` has to be on `LD_LIBRARY_PATH` on Linux or on
//! `DYLD_LIBRARY_PATH` on macOS, and the SDK's `Bin` directory on `PATH` on
//! Windows.

#![expect(
    clippy::print_stdout,
    reason = "the example prints the number of frames that it drew on standard output"
)]
#![expect(
    clippy::print_stderr,
    reason = "the example prints the adapter, its reloads, and its errors on standard error"
)]

mod format;
#[path = "../../shared/frames.rs"]
mod frames;
#[path = "../../shared/report.rs"]
mod report;
mod scene;
#[path = "../../shared/watched.rs"]
mod watched;

use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

use crate::format::colour_format;
use crate::frames::{Frames, OPTIONS, parse_args};
use crate::report::report;
use crate::scene::Scene;

const USAGE: &str = "\
Usage: windowed-renderer [--frames N] <FILE>

Draws a rotating, textured quad in a window with the entry points vsMain and
fsMain of the Slang file FILE, and loads FILE again whenever it changes.
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

    let mut app = App {
        file: args.file,
        frames: Frames::new(args.frames, Instant::now()),
        start: Instant::now(),
        state: None,
        error: None,
    };
    EventLoop::new()?.run_app(&mut app)?;
    if let Some(error) = app.error {
        return Err(error);
    }
    if let Some(summary) = app.frames.summary() {
        println!("{summary}");
    }
    Ok(())
}

struct App {
    file: PathBuf,
    frames: Frames,
    start: Instant,
    state: Option<State>,
    error: Option<Box<dyn Error>>,
}

struct State {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: slango::Device,
    scene: Scene,
}

impl State {
    fn new(event_loop: &ActiveEventLoop, file: &Path) -> Result<Self, Box<dyn Error>> {
        let attributes = Window::default_attributes().with_title("slango windowed renderer");
        let window = Arc::new(event_loop.create_window(attributes)?);
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..wgpu::InstanceDescriptor::new_with_display_handle(Box::new(Arc::clone(&window)))
        });
        let surface = instance.create_surface(Arc::clone(&window))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..wgpu::RequestAdapterOptions::default()
        }))?;
        eprintln!("running on {}", adapter.get_info().name);
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))?;
        let device = slango::wgpu::from_device_and_queue(device, queue);

        let formats = surface.get_capabilities(&adapter).formats;
        let (format, colour_format) = colour_format(&formats).ok_or_else(|| {
            format!("the surface offers the formats {formats:?}, of which slango supports none")
        })?;
        let PhysicalSize { width, height } = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, width.max(1), height.max(1))
            .ok_or("the surface is not supported by the adapter")?;
        config.format = format;
        surface.configure(slango::wgpu::device(&device), &config);

        let scene = Scene::new(&device, file, colour_format)?;
        Ok(Self {
            window,
            surface,
            config,
            device,
            scene,
        })
    }

    fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            self.config.width = size.width;
            self.config.height = size.height;
            self.surface
                .configure(slango::wgpu::device(&self.device), &self.config);
        }
    }

    fn frame(&mut self, angle: f32) -> Result<bool, Box<dyn Error>> {
        match self.scene.reload_if_changed() {
            Ok(true) => eprintln!("reloaded the shader"),
            Ok(false) => {}
            Err(error) => report(&error),
        }
        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                self.resize(self.window.inner_size());
                return Ok(false);
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                return Err("wgpu rejected the surface texture of a frame".into());
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let target = slango::wgpu::texture_from_view(&self.device, &view)?;
        self.scene.draw(&target, angle)?;
        self.window.pre_present_notify();
        slango::wgpu::queue(&self.device).present(frame);
        Ok(true)
    }
}

impl App {
    fn fail(&mut self, event_loop: &ActiveEventLoop, error: Box<dyn Error>) {
        self.error = Some(error);
        event_loop.exit();
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() {
            match State::new(event_loop, &self.file) {
                Ok(state) => {
                    self.state = Some(state);
                    self.frames.start(Instant::now());
                }
                Err(error) => self.fail(event_loop, error),
            }
        }
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        let Some(state) = self.state.as_mut() else {
            return;
        };
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => state.resize(size),
            WindowEvent::RedrawRequested => {
                let angle = self.start.elapsed().as_secs_f32() * 0.8;
                match state.frame(angle) {
                    Ok(drawn) => {
                        if drawn {
                            self.frames.drawn(Instant::now());
                        }
                        if self.frames.running() {
                            state.window.request_redraw();
                        } else {
                            event_loop.exit();
                        }
                    }
                    Err(error) => self.fail(event_loop, error),
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.state.is_none() || event_loop.exiting() {
            return;
        }
        match self.frames.watch(Instant::now()) {
            Ok(Some(deadline)) => event_loop.set_control_flow(ControlFlow::WaitUntil(deadline)),
            Ok(None) => {}
            Err(error) => self.fail(event_loop, error),
        }
    }
}
