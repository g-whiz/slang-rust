use slango::{AdapterSelection, Buffer, Device, Element, ErrorKind, Module};

use crate::common::{device_with, load};

const ELEMENTS: &str = r#"
StructuredBuffer<float> floats;
StructuredBuffer<int> ints;
StructuredBuffer<uint> uints;
StructuredBuffer<float4> floats4;
StructuredBuffer<int4> ints4;
StructuredBuffer<uint4> uints4;
StructuredBuffer<bool> flags;
RWStructuredBuffer<float4> output;

struct Particle { float3 position; float mass; };
RWStructuredBuffer<Particle> particles;

[shader("compute")]
[numthreads(1, 1, 1)]
void readFloat() { output[0] = float4(floats[0]); }

[shader("compute")]
[numthreads(1, 1, 1)]
void readInt() { output[0] = float4(float(ints[0])); }

[shader("compute")]
[numthreads(1, 1, 1)]
void readUint() { output[0] = float4(float(uints[0])); }

[shader("compute")]
[numthreads(1, 1, 1)]
void readFloat4() { output[0] = floats4[0]; }

[shader("compute")]
[numthreads(1, 1, 1)]
void readInt4() { output[0] = float4(ints4[0]); }

[shader("compute")]
[numthreads(1, 1, 1)]
void readUint4() { output[0] = float4(uints4[0]); }

[shader("compute")]
[numthreads(1, 1, 1)]
void readFlag() { output[0] = float4(flags[0] ? 1.0 : 0.0); }

[shader("compute")]
[numthreads(1, 1, 1)]
void moveParticle()
{
    particles[0].position += float3(1.0, 2.0, 3.0);
    particles[0].mass *= 2.0;
}
"#;

const SENTINEL: [f32; 4] = [7.0; 4];

struct Fixture {
    device: Device,
    module: Module,
    output: Buffer<[f32; 4]>,
}

impl Fixture {
    fn new() -> slango::Result<Self> {
        let device = device_with(AdapterSelection::Default);
        let module = load(&device, ELEMENTS);
        let output = Buffer::from_slice(&device, &[SENTINEL])?;
        Ok(Self {
            device,
            module,
            output,
        })
    }

    /// Dispatches the kernel `kernel` with `elements` bound to `path`, and
    /// returns the elements of the output.
    fn run<T: Element>(
        &self,
        kernel: &str,
        path: &str,
        elements: &[T],
    ) -> slango::Result<Vec<[f32; 4]>> {
        let input = Buffer::from_slice(&self.device, elements)?;
        self.module
            .kernel(kernel)?
            .call()
            .bind(path, &input)
            .bind("output", &self.output)
            .dispatch([1, 1, 1])?;
        self.output.read()
    }

    /// Asserts that the kernel `kernel` rejects a buffer of `elements` bound
    /// to `path`, whose Slang element type is `slang`, with the Rust type
    /// `rust` in the message, and that the output is unchanged.
    fn assert_rejected<T: Element>(
        &self,
        kernel: &str,
        path: &str,
        elements: &[T],
        slang: &str,
        rust: &str,
    ) -> slango::Result<()> {
        let outcome = self
            .run(kernel, path, elements)
            .map_err(|error| (error.kind(), error.to_string()));
        assert_eq!(
            outcome,
            Err((
                ErrorKind::InvalidArgument,
                format!(
                    "cannot call a kernel: the parameter `{path}` of the kernel `{kernel}` has elements of the Slang type `{slang}`, and the bound buffer has elements of the Rust type `{rust}`"
                )
            ))
        );
        assert_eq!(self.output.read()?, [SENTINEL]);
        Ok(())
    }
}

#[test]
fn a_buffer_of_another_scalar_type_of_the_same_stride_is_rejected_before_any_work()
-> slango::Result<()> {
    let fixture = Fixture::new()?;
    fixture.assert_rejected("readFloat", "floats", &[1_u32], "float", "u32")?;
    fixture.assert_rejected("readFloat", "floats", &[1_i32], "float", "i32")?;
    fixture.assert_rejected("readInt", "ints", &[1.0_f32], "int", "f32")?;
    fixture.assert_rejected("readInt", "ints", &[1_u32], "int", "u32")?;
    fixture.assert_rejected("readUint", "uints", &[1_i32], "uint", "i32")?;
    fixture.assert_rejected("readUint", "uints", &[1.0_f32], "uint", "f32")?;
    fixture.assert_rejected("readFlag", "flags", &[1_u32], "bool", "u32")
}

#[test]
fn a_buffer_of_vectors_of_another_scalar_type_is_rejected_before_any_work() -> slango::Result<()> {
    let fixture = Fixture::new()?;
    fixture.assert_rejected("readFloat4", "floats4", &[[1_u32; 4]], "float4", "[u32; 4]")?;
    fixture.assert_rejected("readFloat4", "floats4", &[[1_i32; 4]], "float4", "[i32; 4]")?;
    fixture.assert_rejected("readInt4", "ints4", &[[1.0_f32; 4]], "int4", "[f32; 4]")?;
    fixture.assert_rejected("readInt4", "ints4", &[[1_u32; 4]], "int4", "[u32; 4]")?;
    fixture.assert_rejected("readUint4", "uints4", &[[1_i32; 4]], "uint4", "[i32; 4]")?;
    fixture.assert_rejected("readUint4", "uints4", &[[1.0_f32; 4]], "uint4", "[f32; 4]")
}

#[test]
fn a_buffer_of_the_same_scalar_type_and_count_binds_and_runs() -> slango::Result<()> {
    let fixture = Fixture::new()?;
    assert_eq!(fixture.run("readFloat", "floats", &[2.5_f32])?, [[2.5; 4]]);
    assert_eq!(fixture.run("readInt", "ints", &[-3_i32])?, [[-3.0; 4]]);
    assert_eq!(fixture.run("readUint", "uints", &[4_u32])?, [[4.0; 4]]);
    assert_eq!(
        fixture.run("readFloat4", "floats4", &[[1.0_f32, 2.0, 3.0, 4.0]])?,
        [[1.0, 2.0, 3.0, 4.0]]
    );
    assert_eq!(
        fixture.run("readInt4", "ints4", &[[-1_i32, 2, -3, 4]])?,
        [[-1.0, 2.0, -3.0, 4.0]]
    );
    assert_eq!(
        fixture.run("readUint4", "uints4", &[[5_u32, 6, 7, 8]])?,
        [[5.0, 6.0, 7.0, 8.0]]
    );
    Ok(())
}

#[test]
fn a_buffer_of_a_built_in_element_of_the_same_stride_binds_to_a_buffer_of_structures()
-> slango::Result<()> {
    let fixture = Fixture::new()?;
    let kernel = fixture.module.kernel("moveParticle")?;
    let floats = Buffer::from_slice(&fixture.device, &[[1.0_f32, 1.0, 1.0, 10.0]])?;
    kernel
        .call()
        .bind("particles", &floats)
        .dispatch([1, 1, 1])?;
    assert_eq!(floats.read()?, [[2.0, 3.0, 4.0, 20.0]]);
    let bits = Buffer::from_slice(
        &fixture.device,
        &[[0.0_f32, 0.0, 0.0, 1.5].map(f32::to_bits)],
    )?;
    kernel.call().bind("particles", &bits).dispatch([1, 1, 1])?;
    assert_eq!(bits.read()?, [[1.0_f32, 2.0, 3.0, 3.0].map(f32::to_bits)]);
    Ok(())
}
