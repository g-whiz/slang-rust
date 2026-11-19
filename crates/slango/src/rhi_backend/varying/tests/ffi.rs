use slang_compiler::GlobalSession;

use super::super::{LoadSource, spirv_usage};
use crate::__private::EmbeddedSource;
use crate::layout::{Source, Usage};

const SHADER: &str = r#"
import shared;

struct VIn { float2 position : POSITION; float4 spare : COLOR; };

[shader("vertex")]
float4 vsMain(VIn input, uint3 unused : TEXCOORD0, float4 platform : TEXCOORD1) : SV_Position
{
    float4 position = float4(shift(input.position), 0.0, 1.0);
#ifdef __D3D12__
    position += platform;
#endif
    return position;
}

[shader("vertex")]
float4 vsEmpty() : SV_Position
{
    return float4(0.0, 0.0, 0.0, 1.0);
}

[shader("fragment")]
float4 fsMain() : SV_Target
{
    return float4(1.0, 0.0, 0.0, 1.0);
}

[shader("compute")]
[numthreads(1, 1, 1)]
void unrelated() {}
"#;

const SHARED: &str = r"
module shared;
public float2 shift(float2 value) { return value + float2(1.0, 1.0); }
";

fn usage_of(define: &str) -> Vec<(String, Vec<(String, Usage)>)> {
    let global = GlobalSession::new().expect("a global session");
    let source = Source {
        name: "stages",
        path: "stages.slang",
        text: SHADER,
        search_path: None,
    };
    let imports = [EmbeddedSource {
        name: "shared",
        path: "shared.slang",
        text: SHARED,
    }];
    let load = LoadSource {
        imports: &imports,
        search_paths: &[],
        source: &source,
    };
    spirv_usage(&global, define, &load, &["vsMain"]).expect("the compile for SPIR-V")
}

fn expected(platform: Usage) -> Vec<(String, Vec<(String, Usage)>)> {
    vec![(
        "vsMain".to_owned(),
        vec![
            ("input.position".to_owned(), Usage::Used),
            ("input.spare".to_owned(), Usage::Unused),
            ("unused".to_owned(), Usage::Unused),
            ("platform".to_owned(), platform),
        ],
    )]
}

#[test]
fn spirv_reports_the_attributes_that_the_vertex_entry_point_reads_under_the_macro_of_the_device() {
    assert_eq!(usage_of("__D3D12__"), expected(Usage::Used));
    assert_eq!(usage_of("__VULKAN__"), expected(Usage::Unused));
}
