//! Runs a compute kernel of a Slang file on the GPU with `slango` and checks
//! its results.
//!
//! The example shows the whole flow of `slango`: it creates a device on the
//! adapter that wgpu chooses, loads the Slang file as a module, looks up its
//! kernel `saxpy`, creates two input buffers and an output buffer, sets the
//! two `uniform` values of the kernel, binds the three buffers, dispatches the
//! kernel over every element, and reads the output back. The kernel computes
//! `result = a * x + y` for each element. The example prints the name of the
//! adapter on standard error, and the inputs and results on standard output.
//! It then computes the same values on the central processing unit (CPU) and
//! exits with a failure status when any result differs. The inputs are small
//! whole numbers and `a` is 2.5, so every result is exact on every adapter. On
//! any error, it prints the error and each error that caused it on standard
//! error and exits with a failure status.
//!
//! Run it from the root of the workspace on the sample shader:
//!
//! ```sh
//! cargo run -p slang-rust-examples --bin compute-kernel -- examples/shaders/saxpy.slang
//! ```
//!
//! The program needs a GPU driver or a software driver, such as lavapipe on
//! Linux. It loads the Slang shared library of the Vulkan SDK when it starts,
//! so `$VULKAN_SDK/lib` has to be on `LD_LIBRARY_PATH` on Linux or on
//! `DYLD_LIBRARY_PATH` on macOS, and the SDK's `Bin` directory on `PATH` on
//! Windows.

#![expect(
    clippy::print_stdout,
    reason = "the example prints its inputs and results on standard output"
)]
#![expect(
    clippy::print_stderr,
    reason = "the example prints the adapter and its errors on standard error"
)]

use std::error::Error;
use std::path::PathBuf;
use std::process::ExitCode;

use slango::{Buffer, Device, DeviceOptions};

const USAGE: &str = "\
Usage: compute-kernel <FILE>

Runs the kernel saxpy of the Slang file FILE on the GPU, which computes
result = a * x + y for each element, prints the results, and checks them
against the same computation on the CPU.

Options:
  -h, --help  Print this help
";

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("compute-kernel: {error}");
            let mut source = error.source();
            while let Some(cause) = source {
                eprintln!("  caused by: {cause}");
                source = cause.source();
            }
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut args = std::env::args_os().skip(1);
    let file = match (args.next(), args.next()) {
        (Some(arg), None) if arg == "-h" || arg == "--help" => {
            print!("{USAGE}");
            return Ok(());
        }
        (Some(file), None) => PathBuf::from(file),
        _ => return Err("expected one Slang file, see --help".into()),
    };

    let device = Device::new(&DeviceOptions::default())?;
    eprintln!("running on {}", device.adapter_name());
    let module = device.load_module(&file)?;
    let saxpy = module.kernel("saxpy")?;

    let a = 2.5_f32;
    let x: Vec<f32> = (0..8_u16).map(f32::from).collect();
    let y: Vec<f32> = (1..=8_u16).rev().map(f32::from).collect();
    let count = u32::try_from(x.len())?;

    let x_buffer = Buffer::from_slice(&device, &x)?;
    let y_buffer = Buffer::from_slice(&device, &y)?;
    let result_buffer = Buffer::<f32>::zeroed(&device, x.len())?;
    saxpy
        .call()
        .set("a", a)
        .set("count", count)
        .bind("x", &x_buffer)
        .bind("y", &y_buffer)
        .bind("result", &result_buffer)
        .dispatch([count, 1, 1])?;
    let result = result_buffer.read()?;

    println!(
        "kernel {} with a = {a:?} over {count} elements",
        saxpy.name()
    );
    println!("x      = {x:?}");
    println!("y      = {y:?}");
    println!("result = {result:?}");

    let expected: Vec<f32> = x.iter().zip(&y).map(|(x, y)| a * x + y).collect();
    if result != expected {
        return Err(
            format!("the GPU computed {result:?}, but the CPU computes {expected:?}").into(),
        );
    }
    println!("every result equals a * x + y as the CPU computes it");
    Ok(())
}
