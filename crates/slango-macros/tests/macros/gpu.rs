use slango::{Buffer, Device, DeviceOptions};

use particles::{Motion, Particle, Step, Wind};

use crate::bindings::saxpy;

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module`, and each structure keeps its padding private"
)]
mod particles {
    slango_macros::shader!("shaders/particles.slang");
}

fn inputs(count: u16) -> (Vec<f32>, Vec<f32>) {
    let x = (0..count).map(f32::from).collect();
    let y = (0..count)
        .map(|index| f32::from(index) * 0.5 + 1.0)
        .collect();
    (x, y)
}

fn saxpy_on_the_cpu(a: f32, x: &[f32], y: &[f32]) -> Vec<f32> {
    x.iter().zip(y).map(|(x, y)| a * x + y).collect()
}

#[test]
fn saxpy_through_the_bindings_gives_the_results_computed_on_the_cpu() -> slango::Result<()> {
    let device = Device::new(&DeviceOptions::default())?;
    let shader = saxpy::Shader::load(&device)?;
    let (x, y) = inputs(1000);
    let x_buffer = Buffer::from_slice(&device, &x)?;
    let y_buffer = Buffer::from_slice(&device, &y)?;
    let result = Buffer::<f32>::zeroed(&device, 1000)?;
    let args = saxpy::SaxpyArgs {
        a: 2.5,
        count: 1000,
        x: &x_buffer,
        y: &y_buffer,
        result: &result,
    };
    shader.saxpy.dispatch(&args, [1000, 1, 1])?;
    assert_eq!(result.read()?, saxpy_on_the_cpu(2.5, &x, &y));
    Ok(())
}

#[test]
fn saxpy_on_thread_groups_stops_at_the_count() -> slango::Result<()> {
    let device = Device::new(&DeviceOptions::default())?;
    let shader = saxpy::Shader::load(&device)?;
    let (x, y) = inputs(200);
    let x_buffer = Buffer::from_slice(&device, &x)?;
    let y_buffer = Buffer::from_slice(&device, &y)?;
    let result = Buffer::from_slice(&device, &[-1.0_f32; 200])?;
    let args = saxpy::SaxpyArgs {
        a: -3.0,
        count: 130,
        x: &x_buffer,
        y: &y_buffer,
        result: &result,
    };
    shader.saxpy.dispatch_groups(&args, [3, 1, 1])?;
    let mut expected = saxpy_on_the_cpu(-3.0, &x[..130], &y[..130]);
    expected.resize(200, -1.0);
    assert_eq!(result.read()?, expected);
    assert_eq!(shader.saxpy.kernel().name(), "saxpy");
    assert!(shader.module().kernel("saxpy").is_ok());
    Ok(())
}

fn particle(index: u16) -> Particle {
    let at = f32::from(index);
    Particle::new(
        [at, -at, 0.5 * at],
        1.0 + f32::from(index % 3),
        Motion::new(
            [0.125 * f32::from(index % 8), -0.25],
            0.25 * f32::from(index % 4),
        ),
        u32::from(index),
    )
}

/// Advances a particle as the kernel `advance` does, with values that every
/// adapter computes exactly, with or without fused multiply-adds.
fn advanced_on_the_cpu(particle: Particle, index: u32, step: &Step, boost: &Motion) -> Particle {
    let Particle {
        position: [x, y, z],
        mass,
        motion,
        id,
        ..
    } = particle;
    let factor = 1.0 - motion.drag * boost.drag;
    let [vx, vy] = [0, 1].map(|axis| {
        (motion.velocity[axis] + (step.wind.force[axis] + boost.velocity[axis]) * step.dt) * factor
    });
    Particle::new(
        [x + vx * step.dt, y + vy * step.dt, z + mass],
        mass * 2.0,
        Motion::new([vx, vy], motion.drag),
        id * 3 + index,
    )
}

#[test]
fn a_kernel_over_a_buffer_of_a_generated_struct_gives_the_results_computed_on_the_cpu()
-> slango::Result<()> {
    let device = Device::new(&DeviceOptions::default())?;
    let shader = particles::Shader::load(&device)?;
    let initial: Vec<Particle> = (0..100).map(particle).collect();
    let buffer = Buffer::from_slice(&device, &initial)?;
    let args = particles::AdvanceArgs {
        particles: &buffer,
        step: Step::new(0.5, Wind::new([0.25, -0.5])),
        boost: Motion::new([1.0, 0.5], 0.5),
        count: 90,
    };
    shader.advance.dispatch(&args, [100, 1, 1])?;
    let expected: Vec<Particle> = initial
        .iter()
        .zip(0..)
        .map(|(particle, index)| {
            if index < 90 {
                advanced_on_the_cpu(*particle, index, &args.step, &args.boost)
            } else {
                *particle
            }
        })
        .collect();
    assert_eq!(buffer.read()?, expected);
    Ok(())
}
