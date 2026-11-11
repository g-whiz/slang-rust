use slango::__private::{StructElement, load_precompiled, precompile_file};
use slango::{AdapterSelection, Buffer, Device, ErrorKind, Module};

use crate::common::{device_with, shader};

fn device() -> Device {
    device_with(AdapterSelection::Default)
}

/// Loads a test shader twice, once from the file and once from its encoded
/// snapshot.
fn both(device: &Device, name: &str) -> slango::Result<[Module; 2]> {
    let precompiled = precompile_file(&shader(name))?;
    Ok([
        device.load_module(shader(name))?,
        load_precompiled(device, &precompiled.encoded)?,
    ])
}

#[test]
fn a_precompiled_kernel_gives_the_results_of_the_kernel_that_a_load_compiles() -> slango::Result<()>
{
    let device = device();
    let input = [1.0_f32, -2.0, 3.5, 4.0, 5.0];
    let mut results = Vec::new();
    for module in both(&device, "scale.slang")? {
        let kernel = module.kernel("scale")?;
        assert_eq!(kernel.thread_group_size(), [64, 1, 1]);
        let data = Buffer::from_slice(&device, &input)?;
        kernel
            .call()
            .set("factor", 3.0_f32)
            .set("count", 4_u32)
            .bind("data", &data)
            .dispatch([5, 1, 1])?;
        results.push(data.read()?);
    }
    assert_eq!(results, [[3.0, -6.0, 10.5, 12.0, 5.0]; 2]);
    Ok(())
}

#[test]
fn a_precompiled_shader_with_an_import_runs_both_kernels_as_a_loaded_one() -> slango::Result<()> {
    let device = device();
    let mut results = Vec::new();
    for module in both(&device, "two_kernels.slang")? {
        let double_each = module.kernel("double_each")?;
        assert_eq!(double_each.thread_group_size(), [8, 4, 2]);
        let data = Buffer::from_slice(&device, &[1.0_f32, 2.0, 3.0])?;
        double_each
            .call()
            .set("factor", 1.0_f32)
            .set("count", 3_u32)
            .bind("data", &data)
            .dispatch([3, 1, 1])?;
        module
            .kernel("scale")?
            .call()
            .set("factor", 0.5_f32)
            .set("count", 2_u32)
            .bind("data", &data)
            .dispatch([3, 1, 1])?;
        results.push(data.read()?);
    }
    assert_eq!(results, [[1.0, 2.0, 6.0]; 2]);
    Ok(())
}

/// The Slang structure `Particle` of `particles.slang`, with the layout of
/// its structured buffer: offsets 0, 12, and 16, and a stride of 32 bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Particle {
    position: [f32; 3],
    mass: f32,
    velocity: [f32; 2],
}

impl StructElement for Particle {
    const SLANG_NAME: &'static str = "Particle";
    const STRIDE: usize = 32;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        let scalars = self
            .position
            .iter()
            .chain([&self.mass])
            .chain(&self.velocity);
        for scalar in scalars {
            out.extend_from_slice(&scalar.to_le_bytes());
        }
        out.extend_from_slice(&[0; 8]);
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() != Self::STRIDE {
            return None;
        }
        let scalars: Vec<f32> = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|chunk| f32::from_le_bytes(*chunk))
            .collect();
        let [px, py, pz, mass, vx, vy, _, _] = scalars[..] else {
            return None;
        };
        Some(Self {
            position: [px, py, pz],
            mass,
            velocity: [vx, vy],
        })
    }
}

fn advanced(particle: Particle, step: f32) -> Particle {
    let [px, py, pz] = particle.position;
    let [vx, vy] = particle.velocity;
    Particle {
        position: [px + vx * step, py + vy * step, pz + particle.mass],
        mass: particle.mass * 2.0,
        velocity: particle.velocity,
    }
}

#[test]
fn a_precompiled_kernel_runs_over_a_buffer_of_a_struct_element() -> slango::Result<()> {
    let device = device();
    let particles: Vec<Particle> = (0_u8..5)
        .map(|index| {
            let value = f32::from(index);
            Particle {
                position: [value, -value, 0.5],
                mass: value + 1.0,
                velocity: [0.25, value],
            }
        })
        .collect();
    let mut results = Vec::new();
    for module in both(&device, "particles.slang")? {
        let buffer = Buffer::from_slice(&device, &particles)?;
        module
            .kernel("advance")?
            .call()
            .set("step", 2.0_f32)
            .set("count", 4_u32)
            .bind("particles", &buffer)
            .dispatch([5, 1, 1])?;
        results.push(buffer.read()?);
    }
    let mut expected: Vec<Particle> = particles
        .iter()
        .take(4)
        .map(|particle| advanced(*particle, 2.0))
        .collect();
    expected.extend(particles.last().copied());
    assert_eq!(results, [expected.clone(), expected]);
    Ok(())
}

#[test]
fn a_struct_element_is_rejected_as_a_plain_value_before_any_work() -> slango::Result<()> {
    let device = device();
    let [_, module] = both(&device, "particles.slang")?;
    let particle = Particle {
        position: [1.0, 2.0, 3.0],
        mass: 4.0,
        velocity: [5.0, 6.0],
    };
    let buffer = Buffer::from_slice(&device, &[particle])?;
    let error = module
        .kernel("advance")?
        .call()
        .set("step", particle)
        .set("count", 1_u32)
        .bind("particles", &buffer)
        .dispatch([1, 1, 1])
        .expect_err("a structure is not a plain value");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(buffer.read()?, [particle]);
    Ok(())
}

/// A structure of 8 bytes, a smaller stride than that of `Particle`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Pair {
    weight: f32,
    id: u32,
}

impl StructElement for Pair {
    const SLANG_NAME: &'static str = "Particle";
    const STRIDE: usize = 8;

    fn write_bytes(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.weight.to_le_bytes());
        out.extend_from_slice(&self.id.to_le_bytes());
    }

    fn read_bytes(bytes: &[u8]) -> Option<Self> {
        let [weight, id] = bytes.as_chunks::<4>().0 else {
            return None;
        };
        (bytes.len() == Self::STRIDE).then(|| Self {
            weight: f32::from_le_bytes(*weight),
            id: u32::from_le_bytes(*id),
        })
    }
}

#[test]
fn a_buffer_of_a_struct_element_of_another_stride_is_rejected_before_any_work() -> slango::Result<()>
{
    let device = device();
    let pairs: Vec<Pair> = (0_u8..4)
        .map(|index| Pair {
            weight: f32::from(index) + 0.5,
            id: u32::from(index),
        })
        .collect();
    for module in both(&device, "particles.slang")? {
        let buffer = Buffer::from_slice(&device, &pairs)?;
        let error = module
            .kernel("advance")?
            .call()
            .set("step", 2.0_f32)
            .set("count", 1_u32)
            .bind("particles", &buffer)
            .dispatch([1, 1, 1])
            .expect_err("the strides differ");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(
            error.to_string(),
            "cannot call a kernel: the parameter `particles` of the kernel `advance` has elements of 32 bytes, and the bound buffer has elements of 8 bytes"
        );
        assert_eq!(buffer.read()?, pairs);
    }
    Ok(())
}

#[test]
fn an_encoded_snapshot_of_another_version_or_with_trailing_bytes_does_not_load()
-> slango::Result<()> {
    let device = device();
    let encoded = precompile_file(&shader("scale.slang"))?.encoded;
    let mut other_version = encoded.clone();
    if let Some(version) = other_version.get_mut(8..12) {
        version.copy_from_slice(&(slango::__private::ENCODING_VERSION + 1).to_le_bytes());
    }
    let mut trailing = encoded;
    trailing.push(0);
    for bytes in [other_version, trailing, Vec::new()] {
        let error = load_precompiled(&device, &bytes).expect_err("the bytes do not load");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    }
    Ok(())
}
