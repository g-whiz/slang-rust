use slango::Buffer;

use super::device;
use crate::common::kernel;

/// A kernel that copies the elements of matrices of every number of rows
/// from a constant buffer, a parameter block, and the `uniform` parameters
/// of its entry point into a buffer, row by row. The matrices in the two
/// uniform buffers have three or four columns, because wgpu rejects a matrix
/// of two columns in a uniform buffer, whose rows are 16 bytes apart there.
const MATRICES: &str = r#"
struct Mats
{
    float2x3 m23; float2x4 m24;
    float3x3 m33; float3x4 m34;
    float4x3 m43; float4x4 m44;
};

ConstantBuffer<Mats> cb;
ParameterBlock<Mats> pb;
RWStructuredBuffer<float> results;

void putMats(Mats m, inout uint next)
{
    for (int r = 0; r < 2; r++) { for (int c = 0; c < 3; c++) { results[next] = m.m23[r][c]; next++; } }
    for (int r = 0; r < 2; r++) { for (int c = 0; c < 4; c++) { results[next] = m.m24[r][c]; next++; } }
    for (int r = 0; r < 3; r++) { for (int c = 0; c < 3; c++) { results[next] = m.m33[r][c]; next++; } }
    for (int r = 0; r < 3; r++) { for (int c = 0; c < 4; c++) { results[next] = m.m34[r][c]; next++; } }
    for (int r = 0; r < 4; r++) { for (int c = 0; c < 3; c++) { results[next] = m.m43[r][c]; next++; } }
    for (int r = 0; r < 4; r++) { for (int c = 0; c < 4; c++) { results[next] = m.m44[r][c]; next++; } }
}

[shader("compute")]
[numthreads(1, 1, 1)]
void readMatrices(uniform float2x3 u23, uniform float3x2 u32, uniform float3x3 u33)
{
    uint next = 0;
    putMats(cb, next);
    putMats(pb, next);
    for (int r = 0; r < 2; r++) { for (int c = 0; c < 3; c++) { results[next] = u23[r][c]; next++; } }
    for (int r = 0; r < 3; r++) { for (int c = 0; c < 2; c++) { results[next] = u32[r][c]; next++; } }
    for (int r = 0; r < 3; r++) { for (int c = 0; c < 3; c++) { results[next] = u33[r][c]; next++; } }
}
"#;

/// Hands out matrices whose elements are distinct numbers, and records the
/// elements in the order of their rows, which is the order in which the
/// kernel writes them.
struct Numbers {
    next: f32,
    expected: Vec<f32>,
}

impl Numbers {
    fn matrix<const R: usize, const C: usize>(&mut self) -> [[f32; C]; R] {
        let mut matrix = [[0.0; C]; R];
        for row in &mut matrix {
            for element in row {
                *element = self.next;
                self.expected.push(self.next);
                self.next += 1.0;
            }
        }
        matrix
    }
}

#[test]
fn matrices_reach_the_kernel_row_by_row_at_the_reflected_row_stride_in_each_kind_of_uniform()
-> slango::Result<()> {
    let device = device();
    let kernel = kernel(&device, MATRICES, "readMatrices");
    let mut numbers = Numbers {
        next: 1.0,
        expected: Vec::new(),
    };
    let paths = |block: &str| {
        ["m23", "m24", "m33", "m34", "m43", "m44"].map(|matrix| format!("{block}.{matrix}"))
    };
    let blocks = [paths("cb"), paths("pb")];
    let mut call = kernel.call();
    for [m23, m24, m33, m34, m43, m44] in &blocks {
        call.set(m23, numbers.matrix::<2, 3>())
            .set(m24, numbers.matrix::<2, 4>())
            .set(m33, numbers.matrix::<3, 3>())
            .set(m34, numbers.matrix::<3, 4>())
            .set(m43, numbers.matrix::<4, 3>())
            .set(m44, numbers.matrix::<4, 4>());
    }
    call.set("u23", numbers.matrix::<2, 3>())
        .set("u32", numbers.matrix::<3, 2>())
        .set("u33", numbers.matrix::<3, 3>());
    let results = Buffer::<f32>::zeroed(&device, numbers.expected.len())?;
    call.bind("results", &results);
    call.dispatch([1, 1, 1])?;
    assert_eq!(results.read()?, numbers.expected);
    Ok(())
}

/// A kernel that copies the elements of matrices of two columns, which the
/// backend `rhi` takes in uniform buffers, from a constant buffer, a
/// parameter block, and the `uniform` parameters of its entry point into a
/// buffer, row by row.
#[cfg(feature = "rhi")]
const TWO_COLUMNS: &str = r#"
struct Narrow { float2x2 m22; float3x2 m32; float4x2 m42; };

ConstantBuffer<Narrow> cb;
ParameterBlock<Narrow> pb;
RWStructuredBuffer<float> results;

void putNarrow(Narrow m, inout uint next)
{
    for (int r = 0; r < 2; r++) { for (int c = 0; c < 2; c++) { results[next] = m.m22[r][c]; next++; } }
    for (int r = 0; r < 3; r++) { for (int c = 0; c < 2; c++) { results[next] = m.m32[r][c]; next++; } }
    for (int r = 0; r < 4; r++) { for (int c = 0; c < 2; c++) { results[next] = m.m42[r][c]; next++; } }
}

[shader("compute")]
[numthreads(1, 1, 1)]
void readNarrow(uniform float2x2 u22, uniform float4x2 u42)
{
    uint next = 0;
    putNarrow(cb, next);
    putNarrow(pb, next);
    for (int r = 0; r < 2; r++) { for (int c = 0; c < 2; c++) { results[next] = u22[r][c]; next++; } }
    for (int r = 0; r < 4; r++) { for (int c = 0; c < 2; c++) { results[next] = u42[r][c]; next++; } }
}
"#;

#[cfg(feature = "rhi")]
#[test]
fn matrices_of_two_columns_reach_the_kernel_row_by_row_in_each_kind_of_uniform()
-> slango::Result<()> {
    let device = device();
    let kernel = kernel(&device, TWO_COLUMNS, "readNarrow");
    let mut numbers = Numbers {
        next: 1.0,
        expected: Vec::new(),
    };
    let mut call = kernel.call();
    for [m22, m32, m42] in [
        ["cb.m22", "cb.m32", "cb.m42"],
        ["pb.m22", "pb.m32", "pb.m42"],
    ] {
        call.set(m22, numbers.matrix::<2, 2>())
            .set(m32, numbers.matrix::<3, 2>())
            .set(m42, numbers.matrix::<4, 2>());
    }
    call.set("u22", numbers.matrix::<2, 2>())
        .set("u42", numbers.matrix::<4, 2>());
    let results = Buffer::<f32>::zeroed(&device, numbers.expected.len())?;
    call.bind("results", &results);
    call.dispatch([1, 1, 1])?;
    assert_eq!(results.read()?, numbers.expected);
    Ok(())
}
