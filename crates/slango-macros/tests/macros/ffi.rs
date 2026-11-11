#[test]
fn the_macro_compiles_a_shader_and_reports_each_failure_at_the_path() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/macros/ui/saxpy.rs");
    cases.compile_fail("tests/macros/ui/slang_error.rs");
    cases.compile_fail("tests/macros/ui/unsupported_parameter.rs");
    cases.compile_fail("tests/macros/ui/name_conflict.rs");
    cases.compile_fail("tests/macros/ui/not_a_literal.rs");
}
