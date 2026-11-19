use slang_compiler::{
    CompilerOptions, Component, EntryPoint, GlobalSession, MatrixLayout, Session, Target,
    TargetFormat, Toggle,
};

use super::super::super::{Binding, KernelLayout, Resource, Usage, render_entry, walk_program};
use super::{kernels, render};

/// A kernel with every kind of parameter that the walk supports, among them
/// structures and matrices whose layouts differ between targets, and a
/// buffer that the code never uses.
const EVERY_KIND: &str = r#"
struct Gap { float scale; float3 colour; };
struct Params { float gain; float3x3 rotation; float2x3 wide; float3x2 tall; uint flags[3]; };
struct Inner { float4 tint; RWStructuredBuffer<float> values; };
struct Outer { float weight; ParameterBlock<Inner> inner; StructuredBuffer<uint> ids; };

StructuredBuffer<Gap> gaps;
RWStructuredBuffer<float4> results;
ConstantBuffer<Params> params;
cbuffer Named { float2 offset; };
uniform float factor;
ParameterBlock<Outer> block;
RWStructuredBuffer<float> spare;

[shader("compute")]
[numthreads(8, 1, 1)]
void run(uint3 id : SV_DispatchThreadID, uniform float bias, uniform float2x3 shear)
{
    Gap gap = gaps[id.x];
    float3 turned = mul(params.rotation, gap.colour) + mul(params.wide, float3(1, 2, 3)).xyx;
    float2 skew = mul(params.tall, float2(1, 2)).xy + mul(shear, float3(1, 1, 1));
    results[id.x] = float4(turned * gap.scale * params.gain + offset.x, factor + bias + skew.x
        + float(params.flags[id.x % 3]) + block.weight + block.inner.tint.x
        + float(block.ids[id.x]));
    block.inner.values[id.x] = skew.y;
}
"#;

/// Compiles `source` for `target` in a session of the row-major matrix
/// layout, as slang-rhi creates its sessions, and walks its kernels.
fn walk_in(target: Target, source: &str) -> Vec<KernelLayout> {
    let global = GlobalSession::new().expect("a global session");
    let session = Session::builder(&global)
        .target(target)
        .matrix_layout(MatrixLayout::RowMajor)
        .build()
        .expect("a session");
    let module = session
        .load_module_from_source("targets", "targets.slang", source)
        .expect("the module compiles")
        .into_value();
    let entry_points = module
        .entry_points()
        .expect("the entry points")
        .collect::<slang_compiler::Result<Vec<EntryPoint>>>()
        .expect("each entry point");
    let mut components = vec![Component::Module(&module)];
    components.extend(entry_points.iter().map(Component::EntryPoint));
    let program = session
        .compose(&components)
        .expect("the composition")
        .into_value()
        .link()
        .expect("the link")
        .into_value();
    let target = session.target_index(0).expect("the target");
    let (kernels, _) = walk_program(&program, target, |_, layout, _| Ok(layout), |_, _, _| ())
        .expect("the walk of every kernel");
    kernels
}

/// The SPIR-V target with the scalar layout of buffers, which slang-rhi
/// gives its sessions on Vulkan.
fn scalar_spirv() -> Target {
    let mut options = CompilerOptions::default();
    options.glsl_force_scalar_layout(Toggle::On);
    Target::new(TargetFormat::Spirv).options(options)
}

/// What the walk records of each parameter apart from its place and its
/// layout: its path, its kind, and its usage, and the paths of the plain
/// values of each uniform buffer and of the immediates.
fn shape_of(kernels: &[KernelLayout]) -> Vec<String> {
    let mut lines = Vec::new();
    for kernel in kernels {
        for Binding {
            path,
            usage,
            resource,
            ..
        } in &kernel.bindings
        {
            let kind = match resource {
                Resource::StorageBuffer { access, .. } => format!("storage buffer {access:?}"),
                Resource::UniformBuffer(block) => {
                    let values: Vec<&str> = block
                        .values
                        .iter()
                        .map(|value| value.path.as_str())
                        .collect();
                    format!("uniform buffer {values:?}")
                }
                Resource::SampledTexture => "sampled texture".to_owned(),
                Resource::Sampler => "sampler".to_owned(),
            };
            lines.push(format!("{}: `{path}` {usage:?} {kind}", kernel.name));
        }
        if let Some(block) = &kernel.immediates {
            let values: Vec<&str> = block
                .values
                .iter()
                .map(|value| value.path.as_str())
                .collect();
            lines.push(format!("{}: immediates {values:?}", kernel.name));
        }
    }
    lines.sort();
    lines
}

#[test]
fn the_scalar_layout_of_spirv_packs_structures_and_matrix_rows() {
    let walked = walk_in(scalar_spirv(), EVERY_KIND);
    insta::assert_snapshot!("targets_scalar_spirv", render(&walked));
}

#[test]
fn the_scalar_layout_of_spirv_keeps_the_paths_kinds_and_usage_of_the_default_layout() {
    assert_eq!(
        shape_of(&walk_in(scalar_spirv(), EVERY_KIND)),
        shape_of(&kernels(EVERY_KIND))
    );
}

#[cfg(all(target_os = "linux", feature = "wgpu"))]
#[test]
fn dxil_places_each_resource_in_a_register_and_packs_the_last_row_of_a_matrix() {
    let walked = walk_in(
        Target::new(TargetFormat::Dxil).profile("sm_6_0"),
        EVERY_KIND,
    );
    insta::assert_snapshot!("targets_dxil", render(&walked));
}

#[cfg(all(target_os = "linux", feature = "wgpu"))]
#[test]
fn dxil_keeps_the_paths_kinds_and_usage_of_the_default_layout() {
    assert_eq!(
        shape_of(&walk_in(
            Target::new(TargetFormat::Dxil).profile("sm_6_0"),
            EVERY_KIND
        )),
        shape_of(&kernels(EVERY_KIND))
    );
}

/// A vertex and a fragment entry point that both sample a global texture
/// and each take a texture of their own among their parameters.
const BOTH_STAGES: &str = r#"
struct VOut { float4 position : SV_Position; float2 uv : TEXCOORD0; };

Texture2D<float4> heights;
SamplerState heightSampler;

[shader("vertex")]
VOut vsMain(float2 position : POSITION, uniform Texture2D<float4> shifts)
{
    VOut output;
    float height = heights.SampleLevel(heightSampler, position, 0).x;
    output.position = float4(position + shifts.Load(int3(0, 0, 0)).xy, height, 1.0);
    output.uv = position;
    return output;
}

[shader("fragment")]
float4 fsMain(VOut input, uniform Texture2D<float4> shifts) : SV_Target
{
    return heights.Sample(heightSampler, input.uv) + shifts.Load(int3(0, 0, 0));
}
"#;

/// The path, the set, the binding, and the usage of one binding of a render
/// entry point, and whether it sits among the parameters of the entry point.
type Placed = (String, u32, u32, Usage, bool);

/// Compiles `source` for `target` and walks its vertex and fragment entry
/// points, and returns, for each by its name, its bindings as their paths,
/// sets, bindings, and usage, with whether each sits among the parameters
/// of the entry point.
fn render_bindings_in(target: Target, source: &str) -> Vec<(String, Vec<Placed>)> {
    let global = GlobalSession::new().expect("a global session");
    let session = Session::builder(&global)
        .target(target)
        .build()
        .expect("a session");
    let module = session
        .load_module_from_source("stages", "stages.slang", source)
        .expect("the module compiles")
        .into_value();
    let entry_points = module
        .entry_points()
        .expect("the entry points")
        .collect::<slang_compiler::Result<Vec<EntryPoint>>>()
        .expect("each entry point");
    let mut components = vec![Component::Module(&module)];
    components.extend(entry_points.iter().map(Component::EntryPoint));
    let program = session
        .compose(&components)
        .expect("the composition")
        .into_value()
        .link()
        .expect("the link")
        .into_value();
    let target = session.target_index(0).expect("the target");
    let (_, records) = walk_program(
        &program,
        target,
        |_, layout, _| Ok(layout),
        |at, name, stage| render_entry(at, name, stage, None, |_, _| Ok(Vec::new())),
    )
    .expect("the walk");
    records
        .into_iter()
        .map(|(record, places, _)| {
            let layout = record.usable.expect("the entry point is usable").layout;
            let bindings = layout
                .bindings
                .into_iter()
                .zip(places)
                .map(|(binding, place)| {
                    (
                        binding.path,
                        binding.set,
                        binding.index,
                        binding.usage,
                        place.in_entry_point,
                    )
                })
                .collect();
            (record.name, bindings)
        })
        .collect()
}

fn placed(path: &str, set: u32, index: u32, usage: Usage, in_entry_point: bool) -> Placed {
    (path.to_owned(), set, index, usage, in_entry_point)
}

#[test]
fn spirv_gives_each_binding_of_both_render_stages_its_own_set_and_binding() {
    let walked = render_bindings_in(Target::new(TargetFormat::Spirv), BOTH_STAGES);
    assert_eq!(
        walked,
        [
            (
                "vsMain".to_owned(),
                vec![
                    placed("heights", 0, 0, Usage::Used, false),
                    placed("heightSampler", 0, 1, Usage::Used, false),
                    placed("shifts", 0, 2, Usage::Used, true),
                ]
            ),
            (
                "fsMain".to_owned(),
                vec![
                    placed("heights", 0, 0, Usage::Used, false),
                    placed("heightSampler", 0, 1, Usage::Used, false),
                    placed("shifts", 0, 3, Usage::Used, true),
                ]
            ),
        ]
    );
}

#[cfg(all(target_os = "linux", feature = "wgpu"))]
#[test]
fn dxil_reports_every_binding_of_both_render_stages_at_set_0_binding_0_with_its_path_and_place() {
    let walked = render_bindings_in(
        Target::new(TargetFormat::Dxil).profile("sm_6_0"),
        BOTH_STAGES,
    );
    assert_eq!(
        walked,
        [
            (
                "vsMain".to_owned(),
                vec![
                    placed("heights", 0, 0, Usage::Used, false),
                    placed("heightSampler", 0, 0, Usage::Used, false),
                    placed("shifts", 0, 0, Usage::Used, true),
                ]
            ),
            (
                "fsMain".to_owned(),
                vec![
                    placed("heights", 0, 0, Usage::Used, false),
                    placed("heightSampler", 0, 0, Usage::Used, false),
                    placed("shifts", 0, 0, Usage::Used, true),
                ]
            ),
        ]
    );
}
