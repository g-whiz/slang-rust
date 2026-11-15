use std::ptr::{self, NonNull};

use slang_compiler::{GlobalSession, Result, Session, Target, TargetFormat};
use slang_compiler_sys::{IGlobalSession, SlangGlobalSessionDesc, SlangResult};

use super::support::{compile, count, words};

const OP_FUNCTION: u32 = 54;
const OP_FUNCTION_CALL: u32 = 57;

const MISSING_OPTIMIZER: &str = "failed to load downstream compiler 'spirv-opt'";

const INLINED: &str = r#"
RWStructuredBuffer<float> output;

float helper(float x) { float y = x * 2.0; float z = y + 0.0; return z * 1.0; }

[shader("compute")]
[numthreads(64, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    float a = helper(float(id.x));
    for (int i = 0; i < 4; i++) { a += helper(a); }
    output[id.x] = a;
}
"#;

/// Compiles the shader whose helper the optimizer inlines to SPIR-V in a new
/// session of `global`, and returns the number of functions, the number of
/// calls, and the diagnostics of the generation.
fn functions_calls_and_diagnostics(global: &GlobalSession) -> Result<(usize, usize, String)> {
    let session = Session::builder(global)
        .target(Target::new(TargetFormat::Spirv).profile("spirv_1_5"))
        .build()?;
    let code = compile(&session, "inlined", INLINED, 0)?;
    let spirv = words(code.value.as_bytes());
    Ok((
        count(&spirv, OP_FUNCTION),
        count(&spirv, OP_FUNCTION_CALL),
        code.diagnostics.to_string(),
    ))
}

#[test]
fn a_global_session_of_the_crate_optimizes_spirv_through_the_installed_loader() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let (functions, calls, diagnostics) =
        functions_calls_and_diagnostics(&global).expect("the shader compiles to SPIR-V");
    assert!(!diagnostics.contains(MISSING_OPTIMIZER), "{diagnostics}");
    assert_eq!((functions, calls), (1, 0), "{diagnostics}");
}

#[test]
#[expect(
    ffi_unwind_calls,
    reason = "Slang is C++, and an exception that escaped the creation would unwind into the test, which is defined and fails it"
)]
fn a_global_session_without_the_loader_returns_unoptimized_spirv_and_names_the_missing_optimizer() {
    let desc = SlangGlobalSessionDesc::default();
    let mut raw: *mut IGlobalSession = ptr::null_mut();
    // SAFETY: `desc` is a live descriptor whose size field holds its own size,
    // which `Default` sets, and `raw` is a live out-pointer.
    let result =
        unsafe { slang_compiler_sys::slang_createGlobalSession2(&raw const desc, &raw mut raw) };
    assert_eq!(result, SlangResult::SLANG_OK);
    let raw = NonNull::new(raw).expect("Slang writes a global session");
    // SAFETY: `raw` is a new global session of the Slang of the build with
    // one reference, which this test transfers and uses on this thread only.
    let global = unsafe { GlobalSession::from_raw(raw) };
    let (functions, calls, diagnostics) =
        functions_calls_and_diagnostics(&global).expect("the shader compiles to SPIR-V");
    assert!(diagnostics.contains(MISSING_OPTIMIZER), "{diagnostics}");
    assert!(diagnostics.contains("slang-glslang"), "{diagnostics}");
    assert!(
        functions > 1 && calls > 0,
        "{functions} functions and {calls} calls"
    );
}
