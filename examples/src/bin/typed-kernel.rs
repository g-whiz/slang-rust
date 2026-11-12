//! Runs a compute kernel on the GPU through the typed bindings that
//! `slango_macros::shader!` generates, and checks its results.
//!
//! The macro compiles `examples/shaders/particles.slang` while the example
//! builds, so a parameter that the Rust code names wrongly or gives a value
//! of the wrong type is a compile error, not an error at the dispatch. The
//! shader declares the structure `Particle`, and the macro generates a Rust
//! struct `Particle` with the layout that Slang gives it in a structured
//! buffer, padding included. The example creates a device on the adapter
//! that wgpu chooses, loads the embedded shader, creates a buffer of eight
//! particles and a buffer of `f32` for their energies, and dispatches the
//! kernel `advance` with the plain values `gravity`, `dt`, and `count`
//! through the generated arguments struct. The kernel applies gravity to the
//! velocity of each particle, moves it, counts the step, and writes its
//! kinetic energy. The example prints the name of the adapter on standard
//! error, and the inputs and results on standard output. It then computes
//! the same values on the central processing unit (CPU) and exits with a
//! failure status when any result differs. Every input is a multiple of a
//! small power of two, so every result is exact on every adapter. On any
//! error, it prints the error and each error that caused it on standard
//! error and exits with a failure status.
//!
//! Without the Slang library, as on docs.rs, the macro generates only an
//! empty `Shader`. The code therefore names the other generated items only
//! inside the body of a function, which the documentation build does not
//! resolve, so `just check-doc-without-slang` documents the example. A
//! `use` declaration or the signature of a function that names such an item
//! fails that build.
//!
//! Run it from the root of the workspace:
//!
//! ```sh
//! cargo run -p slang-rust-examples --bin typed-kernel
//! ```
//!
//! Building the example needs the pinned Slang, as the crate `slango-macros`
//! describes. The program needs a GPU driver or a software driver, such as
//! lavapipe on Linux. It loads the Slang shared library of the Vulkan SDK
//! when it starts, so `$VULKAN_SDK/lib` has to be on `LD_LIBRARY_PATH` on
//! Linux or on `DYLD_LIBRARY_PATH` on macOS, and the SDK's `Bin` directory
//! on `PATH` on Windows.

#![expect(
    clippy::print_stdout,
    reason = "the example prints its inputs and results on standard output"
)]
#![expect(
    clippy::print_stderr,
    reason = "the example prints the adapter and its errors on standard error"
)]

#[path = "../shared/report.rs"]
mod report;

use std::error::Error;
use std::process::ExitCode;

use slango::{Buffer, Device, DeviceOptions};

use crate::report::report;
use particles::Shader;

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module` (SLM-15), and `Particle` keeps its padding private (SLM-18)"
)]
mod particles {
    slango_macros::shader!("../../shaders/particles.slang");
}

const USAGE: &str = "\
Usage: typed-kernel

Runs the kernel advance of examples/shaders/particles.slang on the GPU
through the typed bindings that slango_macros::shader! generates, prints
the particles and their energies, and checks them against the same
computation on the CPU.

Options:
  -h, --help  Print this help
";

const GRAVITY: [f32; 2] = [0.0, -2.0];
const DT: f32 = 0.5;

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
    let mut args = std::env::args_os().skip(1);
    match (args.next(), args.next()) {
        (None, None) => {}
        (Some(arg), None) if arg == "-h" || arg == "--help" => {
            print!("{USAGE}");
            return Ok(());
        }
        _ => return Err("expected no arguments, see --help".into()),
    }

    let device = Device::new(&DeviceOptions::default())?;
    eprintln!("running on {}", device.adapter_name());
    let shader = Shader::load(&device)?;

    let initial: Vec<particles::Particle> = (0..8_u16)
        .map(|index| {
            let at = f32::from(index);
            particles::Particle::new(
                [at, 0.5 * at, 1.0],
                1.0 + f32::from(index % 3),
                [0.25 * at, 1.0 - 0.5 * at],
                0,
            )
        })
        .collect();
    let count = u32::try_from(initial.len())?;
    let particles = Buffer::from_slice(&device, &initial)?;
    let energy = Buffer::<f32>::zeroed(&device, initial.len())?;
    let args = particles::AdvanceArgs {
        particles: &particles,
        energy: &energy,
        gravity: GRAVITY,
        dt: DT,
        count,
    };
    shader.advance.dispatch(&args, [count, 1, 1])?;
    let advanced = particles.read()?;
    let energies = energy.read()?;

    println!(
        "kernel {} with gravity = {GRAVITY:?} and dt = {DT:?} over {count} particles",
        shader.advance.kernel().name()
    );
    for (index, (before, (after, energy))) in initial
        .iter()
        .zip(advanced.iter().zip(&energies))
        .enumerate()
    {
        println!(
            "particle {index}: position {:?} -> {:?}, velocity {:?} -> {:?}, mass {:?}, steps {}, energy {energy:?}",
            before.position,
            after.position,
            before.velocity,
            after.velocity,
            after.mass,
            after.steps,
        );
    }

    let expected: Vec<(particles::Particle, f32)> = initial
        .iter()
        .map(|particle| {
            let velocity = [0, 1].map(|axis| particle.velocity[axis] + GRAVITY[axis] * DT);
            let [x, y, z] = particle.position;
            let position = [x + velocity[0] * DT, y + velocity[1] * DT, z];
            let energy =
                0.5 * particle.mass * (velocity[0] * velocity[0] + velocity[1] * velocity[1]);
            (
                particles::Particle::new(position, particle.mass, velocity, particle.steps + 1),
                energy,
            )
        })
        .collect();
    let computed: Vec<(particles::Particle, f32)> = advanced.into_iter().zip(energies).collect();
    if computed != expected {
        return Err(
            format!("the GPU computed {computed:?}, but the CPU computes {expected:?}").into(),
        );
    }
    println!("every particle and energy equals what the CPU computes");
    Ok(())
}
