//! The code that `slango_macros::shader!` generates for the test shaders of
//! `slango-macros`, run on the backend of the build.
//!
//! The test binary of `slango-macros` cannot build `slango` with the feature
//! `rhi`, so these tests run the generated code on that backend here, with
//! the same shaders.

use slango::Buffer;

use crate::common::device;

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module`"
)]
mod saxpy {
    slango_macros::shader!("../../../../crates/slango-macros/tests/macros/shaders/saxpy.slang");
}

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module`, and each structure keeps its padding private"
)]
mod particles {
    slango_macros::shader!("../../../../crates/slango-macros/tests/macros/shaders/particles.slang");
}

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module`, and each structure keeps its padding private"
)]
mod layouts {
    slango_macros::shader!("../../../../crates/slango-macros/tests/macros/shaders/layouts.slang");
}

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module`"
)]
mod imports {
    slango_macros::shader!("../../../../crates/slango-macros/tests/macros/shaders/imports.slang");
}

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module`, and each structure keeps its padding, or a field of zero size, private"
)]
mod float_fields {
    slango_macros::shader!(
        "../../../../crates/slango-macros/tests/macros/shaders/float-fields.slang"
    );
}

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module`, and each structure keeps its padding, or a field of zero size, private"
)]
mod int_fields {
    slango_macros::shader!(
        "../../../../crates/slango-macros/tests/macros/shaders/int-fields.slang"
    );
}

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module`, and each structure keeps a private field of zero size"
)]
mod wide_vector {
    slango_macros::shader!(
        "../../../../crates/slango-macros/tests/macros/shaders/wide-vector.slang"
    );
}

#[expect(
    clippy::partial_pub_fields,
    reason = "the generated `Shader` keeps its module private behind `Shader::module`, and each structure keeps its padding, or a field of zero size, private"
)]
mod narrow_vector {
    slango_macros::shader!(
        "../../../../crates/slango-macros/tests/macros/shaders/narrow-vector.slang"
    );
}

/// The text of the error of a dispatch of the kernel `step` of
/// `narrow-vector.slang` with a buffer of `wide_vector::Wide`: the field
/// types differ on the backend wgpu, where both structures have a stride of
/// 16 bytes.
#[cfg(feature = "wgpu")]
const NARROW_ERROR: &str = "cannot call a kernel: the parameter `items` of the kernel `step` has elements of the structure `Narrow` with the field `a` of the Slang type `float3`, and the bound buffer has elements of the Rust struct `Wide` with that field of the Slang type `float4`";

/// The text of the error of a dispatch of the kernel `step` of
/// `narrow-vector.slang` with a buffer of `wide_vector::Wide`: the strides
/// differ on the backend rhi, whose packed layout gives `Narrow` 12 bytes.
#[cfg(feature = "rhi")]
const NARROW_ERROR: &str = "cannot call a kernel: the parameter `items` of the kernel `step` has elements of 12 bytes, and the bound buffer has elements of 16 bytes";

/// The sizes of the generated structs `Particle` and `Nested` in the layout
/// of the build's backend: std430 on the backend wgpu.
#[cfg(feature = "wgpu")]
const SIZES: [usize; 2] = [48, 80];

/// The sizes of the generated structs `Particle` and `Nested` in the layout
/// of the build's backend: the packed layout on the backend rhi.
#[cfg(feature = "rhi")]
const SIZES: [usize; 2] = [32, 48];

#[test]
fn a_generated_struct_has_the_layout_of_the_backend_of_the_build() {
    assert_eq!(
        [
            size_of::<particles::Particle>(),
            size_of::<layouts::Nested>()
        ],
        SIZES
    );
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
    let device = device();
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
    let device = device();
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

fn particle(index: u16) -> particles::Particle {
    let at = f32::from(index);
    particles::Particle::new(
        [at, -at, 0.5 * at],
        1.0 + f32::from(index % 3),
        particles::Motion::new(
            [0.125 * f32::from(index % 8), -0.25],
            0.25 * f32::from(index % 4),
        ),
        u32::from(index),
    )
}

/// Advances a particle as the kernel `advance` does, with values that every
/// adapter computes exactly, with or without fused multiply-adds.
fn advanced_on_the_cpu(
    particle: particles::Particle,
    index: u32,
    step: &particles::Step,
    boost: &particles::Motion,
) -> particles::Particle {
    let particles::Particle {
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
    particles::Particle::new(
        [x + vx * step.dt, y + vy * step.dt, z + mass],
        mass * 2.0,
        particles::Motion::new([vx, vy], motion.drag),
        id * 3 + index,
    )
}

#[test]
fn a_kernel_over_a_buffer_of_a_generated_struct_gives_the_results_computed_on_the_cpu()
-> slango::Result<()> {
    let device = device();
    let shader = particles::Shader::load(&device)?;
    let initial: Vec<particles::Particle> = (0..100).map(particle).collect();
    let buffer = Buffer::from_slice(&device, &initial)?;
    let args = particles::AdvanceArgs {
        particles: &buffer,
        step: particles::Step::new(0.5, particles::Wind::new([0.25, -0.5])),
        boost: particles::Motion::new([1.0, 0.5], 0.5),
        count: 90,
    };
    shader.advance.dispatch(&args, [100, 1, 1])?;
    let expected: Vec<particles::Particle> = initial
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

#[test]
fn a_kernel_reads_and_writes_fields_of_structures_whose_layouts_differ_between_backends()
-> slango::Result<()> {
    let device = device();
    let shader = layouts::Shader::load(&device)?;
    let tails: Vec<layouts::Tail> = (0_u8..4)
        .map(|index| layouts::Tail::new([f32::from(index), 0.5, -0.5]))
        .collect();
    let gaps: Vec<layouts::Gap> = (0_u8..4)
        .map(|index| layouts::Gap::new(0.25 * f32::from(index), [1.0, 2.0, 3.0, 4.0]))
        .collect();
    let grids: Vec<layouts::Grid> = (0_i8..4)
        .map(|index| {
            let at = i32::from(index);
            layouts::Grid::new([[at, 1, 2], [3, 4, 10 * at]], [7, 8, 9, 10])
        })
        .collect();
    let wides: Vec<layouts::Wide> = (0_u8..4)
        .map(|index| layouts::Wide::new(1.0, 2.0, 3.0, 0.5 * f32::from(index), 5, 6, -7, -8))
        .collect();
    let initial: Vec<layouts::Nested> = (0_u8..4)
        .map(|index| {
            let at = f32::from(index);
            layouts::Nested::new(
                [at, -at, 2.0],
                layouts::Inner::new([0.5, at], u32::from(index)),
                layouts::Gap::new(-at, [at, 1.0, 2.0, 3.0]),
                100.0,
            )
        })
        .collect();
    let nested = Buffer::from_slice(&device, &initial)?;
    let args = layouts::CombineArgs {
        tails: &Buffer::from_slice(&device, &tails)?,
        gaps: &Buffer::from_slice(&device, &gaps)?,
        grids: &Buffer::from_slice(&device, &grids)?,
        nested: &nested,
        wides: &Buffer::from_slice(&device, &wides)?,
    };
    shader.combine.dispatch(&args, [4, 1, 1])?;
    let expected: Vec<layouts::Nested> = initial
        .iter()
        .zip(&tails)
        .zip(&gaps)
        .zip(&grids)
        .zip(&wides)
        .map(|((((element, tail), gap), grid), wide)| {
            let mut element = *element;
            let cell = grid.cells.get(1).and_then(|row| row.get(2)).copied();
            element.weight = tail.direction[0]
                + gap.weight
                + f32::from(i16::try_from(cell.unwrap_or_default()).unwrap_or_default())
                + wide.d;
            element
        })
        .collect();
    assert_eq!(nested.read()?, expected);
    Ok(())
}

#[test]
fn a_shader_that_imports_a_module_that_imports_another_runs_its_kernel() -> slango::Result<()> {
    let device = device();
    let shader = imports::Shader::load(&device)?;
    let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0, 4.0])?;
    shader.grow.dispatch(
        &imports::GrowArgs {
            data: &data,
            count: 3,
        },
        [4, 1, 1],
    )?;
    assert_eq!(data.read()?, [5.0, 9.0, 13.0, 4.0]);
    Ok(())
}

#[test]
fn a_buffer_of_a_struct_binds_only_where_each_field_has_the_same_type() -> slango::Result<()> {
    let device = device();
    let floats = float_fields::Shader::load(&device)?;
    let ints = int_fields::Shader::load(&device)?;
    let item = float_fields::Item::new([2.5, 0.0, 0.0], 0.0);
    let buffer = Buffer::from_slice(&device, &[item])?;
    let error = ints
        .step
        .kernel()
        .call()
        .bind("items", &buffer)
        .dispatch([1, 1, 1])
        .expect_err("an int3 field does not hold a float3");
    assert_eq!(error.kind(), slango::ErrorKind::InvalidArgument);
    assert_eq!(
        error.to_string(),
        "cannot call a kernel: the parameter `items` of the kernel `step` has elements of the structure `Item` with the field `a` of the Slang type `int3`, and the bound buffer has elements of the Rust struct `Item` with that field of the Slang type `float3`"
    );
    assert_eq!(buffer.read()?, [item]);
    floats
        .step
        .dispatch(&float_fields::StepArgs { items: &buffer }, [1, 1, 1])?;
    assert_eq!(
        buffer.read()?,
        [float_fields::Item::new([2.5, 0.0, 0.0], 3.5)]
    );
    Ok(())
}

#[test]
fn a_buffer_of_a_struct_with_a_wider_vector_does_not_bind() -> slango::Result<()> {
    let device = device();
    let narrow = narrow_vector::Shader::load(&device)?;
    let wide = wide_vector::Wide::new([1.0, 2.0, 3.0, 4.0]);
    let buffer = Buffer::from_slice(&device, &[wide])?;
    let error = narrow
        .step
        .kernel()
        .call()
        .bind("items", &buffer)
        .dispatch([1, 1, 1])
        .expect_err("a float3 field does not hold a float4");
    assert_eq!(error.kind(), slango::ErrorKind::InvalidArgument);
    assert_eq!(error.to_string(), NARROW_ERROR);
    assert_eq!(buffer.read()?, [wide]);
    Ok(())
}
