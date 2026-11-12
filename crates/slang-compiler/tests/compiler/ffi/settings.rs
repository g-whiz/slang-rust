//! The tests that observe the effect of the settings of a session, a target,
//! and `link_with_options` on what the Slang library of the pinned Vulkan SDK
//! loads and generates.

use std::fs;

use slang_compiler::reflection::{LayoutSize, TypeLayout, VariableLayout};
use slang_compiler::{
    Blob, CompilerOptions, Component, DebugInfoLevel, ErrorKind, FloatingPointMode, GlobalSession,
    LanguageVersion, LineDirectiveMode, LinkedProgram, MatrixLayout, OptimizationLevel, OptionName,
    ParameterCategory, Session, SessionBuilder, Target, TargetFormat, Toggle,
};
use slang_compiler_sys::CompilerOptionName;

use super::support::{
    DECORATION_ARRAY_STRIDE, DECORATION_OFFSET, GLSLANG_GENERATOR, OP_EXT_INST_IMPORT, compile,
    compile_spirv, compile_text, decorations, entry_point_names, has_code, instructions, literal,
    scratch_dir, words,
};

/// A compute kernel whose parameters exercise the layout options: a
/// constant buffer with a `float3` after a `float`, an array of `float3`, and
/// a matrix, and a structured buffer of a structure of the same kind.
const LAYOUT: &str = r#"
struct Params { float a; float3 b; float3 arr[2]; float4x3 m; };
ConstantBuffer<Params> params;
struct Item { float a; float3 b; };
RWStructuredBuffer<Item> items;
RWStructuredBuffer<float> data;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    float x = data[id.x];
    float y = x * params.a + params.b.y;
    data[id.x] = y * y + params.arr[1].z + params.m[1][2] + items[0].b.z;
}
"#;

/// A module whose only function uses the comma operator, which Slang warns
/// about with the diagnostic 41024 before language version 2026.
const COMMA: &str = "int pick(int a, int b) { return (a, b); }";

fn options(set: impl FnOnce(&mut CompilerOptions)) -> CompilerOptions {
    let mut options = CompilerOptions::default();
    set(&mut options);
    options
}

fn raw_int(name: CompilerOptionName, value: i32) -> CompilerOptions {
    let name = OptionName::new(name.0).expect("an option that an option set holds");
    let mut options = CompilerOptions::default();
    // SAFETY: the pinned build reads each option that these tests set this
    // way, `MatrixLayoutRow`, `MatrixLayoutColumn`, `UseUpToDateBinaryModule`,
    // and `RestrictiveCapabilityCheck`, as a Boolean from the first integer,
    // and 0 and 1 are values that it accepts.
    unsafe { options.set_int(name, value, 0) };
    options
}

fn build(builder: SessionBuilder) -> Session {
    builder.build().expect("Slang creates the session")
}

fn spirv_with(
    global: &GlobalSession,
    session: CompilerOptions,
    target: CompilerOptions,
) -> Session {
    build(
        Session::builder(global)
            .target(Target::new(TargetFormat::Spirv).options(target))
            .options(session),
    )
}

fn link(session: &Session, source: &str, with: Option<&CompilerOptions>) -> LinkedProgram {
    let module = session
        .load_module_from_source("layout", "layout.slang", source)
        .expect("the source compiles")
        .into_value();
    let entry_point = module.entry_point("main").expect("the entry point exists");
    let composed = session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])
        .expect("the parts compose")
        .into_value();
    match with {
        Some(options) => composed.link_with_options(options),
        None => composed.link(),
    }
    .expect("the composition links")
    .into_value()
}

fn code(session: &Session, program: &LinkedProgram) -> Vec<u32> {
    let blob: Blob = program
        .entry_point_code(0, session.target_index(0).expect("a target"))
        .expect("Slang generates code")
        .into_value();
    words(blob.as_bytes())
}

/// Returns the uniform offsets of the fields of the first parameter, the
/// constant buffer of [`LAYOUT`], from the layout that reflection reports for
/// `program`.
fn reflected_offsets(session: &Session, program: &LinkedProgram) -> Vec<LayoutSize> {
    let target = session.target_index(0).expect("a target");
    let layout = program
        .layout(target)
        .expect("Slang lays out the program")
        .into_value();
    let buffer = layout
        .parameter_layouts()
        .next()
        .and_then(VariableLayout::type_layout)
        .and_then(TypeLayout::element_type_layout)
        .expect("the first parameter is a constant buffer of a structure");
    buffer
        .field_layouts()
        .map(|field| field.offset(ParameterCategory::Uniform).amount)
        .collect()
}

#[test]
fn use_up_to_date_binary_module_compiles_changed_source_and_wins_over_a_raw_entry() {
    let dir = scratch_dir("up_to_date_binary_module");
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = |toggle: Option<Toggle>, raw_off: bool| {
        let mut builder = Session::builder(&global)
            .target(Target::new(TargetFormat::Spirv))
            .search_path(&dir);
        if let Some(toggle) = toggle {
            builder = builder.use_up_to_date_binary_module(toggle);
        }
        if raw_off {
            builder = builder.options(raw_int(CompilerOptionName::UseUpToDateBinaryModule, 0));
        }
        build(builder)
    };
    fs::write(
        dir.join("stale.slang"),
        "[shader(\"compute\")] [numthreads(1, 1, 1)] void old_kernel() {}",
    )
    .expect("the source is written");
    let ir = session(None, false)
        .load_module("stale")
        .expect("the source compiles")
        .into_value()
        .serialize()
        .expect("Slang serializes the module");
    fs::write(dir.join("stale.slang-module"), ir.as_bytes()).expect("the module is written");
    fs::write(
        dir.join("stale.slang"),
        "[shader(\"compute\")] [numthreads(1, 1, 1)] void new_kernel() {}",
    )
    .expect("the source is changed");
    for (toggle, raw_off, fresh) in [
        (None, false, false),
        (Some(Toggle::Off), false, false),
        (Some(Toggle::On), false, true),
        (Some(Toggle::On), true, true),
        (None, true, false),
    ] {
        let module = session(toggle, raw_off)
            .load_module("stale")
            .expect("Slang loads one of the files")
            .into_value();
        assert_eq!(
            module.entry_point("new_kernel").is_ok(),
            fresh,
            "{toggle:?}, raw entry off: {raw_off}"
        );
        assert_eq!(module.entry_point("old_kernel").is_ok(), !fresh);
    }
}

#[test]
fn matrix_layout_applies_only_without_a_raw_matrix_layout_entry() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let with = |layout: MatrixLayout, session_options: CompilerOptions| {
        let session = build(
            Session::builder(&global)
                .target(Target::new(TargetFormat::Spirv))
                .matrix_layout(layout)
                .options(session_options),
        );
        compile_spirv(&session, "layout", LAYOUT, 0)
    };
    let row = with(MatrixLayout::RowMajor, CompilerOptions::default());
    let column = with(MatrixLayout::ColumnMajor, CompilerOptions::default());
    assert_ne!(row, column);
    let raw_row = with(
        MatrixLayout::ColumnMajor,
        raw_int(CompilerOptionName::MatrixLayoutRow, 1),
    );
    assert_eq!(raw_row, row);
    let raw_column = with(
        MatrixLayout::RowMajor,
        raw_int(CompilerOptionName::MatrixLayoutColumn, 1),
    );
    assert_eq!(raw_column, column);
}

#[test]
fn a_line_directive_mode_of_the_session_reaches_a_target_and_the_target_wins() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let none = options(|options| {
        options.line_directive_mode(LineDirectiveMode::None);
    });
    let standard = options(|options| {
        options.line_directive_mode(LineDirectiveMode::Standard);
    });
    let session = build(
        Session::builder(&global)
            .target(Target::new(TargetFormat::Hlsl))
            .target(Target::new(TargetFormat::Hlsl).options(standard))
            .options(none),
    );
    let default = build(Session::builder(&global).target(Target::new(TargetFormat::Hlsl)));
    assert!(compile_text(&default, "layout", LAYOUT, 0).contains("#line"));
    assert!(!compile_text(&session, "layout", LAYOUT, 0).contains("#line"));
    assert!(compile_text(&session, "layout", LAYOUT, 1).contains("#line"));
}

#[test]
fn spirv_through_glslang_shows_in_the_generator_and_a_target_wins() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let via_glsl = options(|options| {
        options.emit_spirv_directly(Toggle::Off);
    });
    let direct = options(|options| {
        options.emit_spirv_directly(Toggle::On);
    });
    let generator = |session: &Session| compile_spirv(session, "layout", LAYOUT, 0)[2] >> 16;
    let default = spirv_with(
        &global,
        CompilerOptions::default(),
        CompilerOptions::default(),
    );
    assert_ne!(generator(&default), GLSLANG_GENERATOR);
    let session = spirv_with(&global, via_glsl.clone(), CompilerOptions::default());
    assert_eq!(generator(&session), GLSLANG_GENERATOR);
    let session = spirv_with(&global, via_glsl, direct);
    assert_ne!(generator(&session), GLSLANG_GENERATOR);
}

#[test]
fn a_floating_point_mode_of_the_session_reaches_a_target_and_the_target_wins() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let mode = |mode| {
        options(|options| {
            options.floating_point_mode(mode);
        })
    };
    let fast = compile_spirv(
        &spirv_with(
            &global,
            mode(FloatingPointMode::Fast),
            CompilerOptions::default(),
        ),
        "layout",
        LAYOUT,
        0,
    );
    let precise = compile_spirv(
        &spirv_with(
            &global,
            mode(FloatingPointMode::Precise),
            CompilerOptions::default(),
        ),
        "layout",
        LAYOUT,
        0,
    );
    assert_ne!(fast, precise);
    let overridden = compile_spirv(
        &spirv_with(
            &global,
            mode(FloatingPointMode::Fast),
            mode(FloatingPointMode::Precise),
        ),
        "layout",
        LAYOUT,
        0,
    );
    assert_eq!(overridden, precise);
}

#[test]
fn a_scalar_layout_of_the_session_reaches_a_target_and_the_target_wins() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let scalar = |toggle| {
        options(|options| {
            options.glsl_force_scalar_layout(toggle);
        })
    };
    let default = compile_spirv(
        &spirv_with(
            &global,
            CompilerOptions::default(),
            CompilerOptions::default(),
        ),
        "layout",
        LAYOUT,
        0,
    );
    let session = compile_spirv(
        &spirv_with(&global, scalar(Toggle::On), CompilerOptions::default()),
        "layout",
        LAYOUT,
        0,
    );
    assert!(decorations(&default, DECORATION_ARRAY_STRIDE).contains(&16));
    assert!(decorations(&session, DECORATION_ARRAY_STRIDE).contains(&12));
    let overridden = compile_spirv(
        &spirv_with(&global, scalar(Toggle::On), scalar(Toggle::Off)),
        "layout",
        LAYOUT,
        0,
    );
    assert_eq!(overridden, default);
}

#[test]
fn a_search_path_finds_an_imported_module_and_a_macro_reaches_the_source() {
    let dir = scratch_dir("search_path_and_macro");
    fs::write(
        dir.join("helper.slang"),
        "public float twice(float x) { return 2.0 * x; }",
    )
    .expect("the helper is written");
    let source = r#"
        import helper;
        RWStructuredBuffer<float> data;
        [shader("compute")] [numthreads(1, 1, 1)]
        void main() { data[0] = twice(SCALE); }
    "#;
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = build(
        Session::builder(&global)
            .target(Target::new(TargetFormat::Spirv))
            .search_path(&dir)
            .define("SCALE", "3.0"),
    );
    let code = compile(&session, "user", source, 0)
        .expect("the import and the macro resolve")
        .into_value();
    assert_ne!(code.as_bytes(), []);
    let without_macro = build(
        Session::builder(&global)
            .target(Target::new(TargetFormat::Spirv))
            .search_path(&dir),
    );
    let error = compile(&without_macro, "user", source, 0).expect_err("SCALE is undefined");
    assert_eq!(error.kind(), ErrorKind::Compile);
    let without_path = build(
        Session::builder(&global)
            .target(Target::new(TargetFormat::Spirv))
            .define("SCALE", "3.0"),
    );
    let error = compile(&without_path, "user", source, 0).expect_err("helper is not found");
    assert_eq!(error.kind(), ErrorKind::Compile);
}

#[test]
fn the_warning_options_turn_a_warning_into_an_error_silence_it_and_report_unknown_names() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let load = |session_options: CompilerOptions| {
        spirv_with(&global, session_options, CompilerOptions::default())
            .load_module_from_source("comma", "comma.slang", COMMA)
            .map(|compiled| compiled.diagnostics)
    };
    let plain = load(CompilerOptions::default()).expect("a warning does not fail the load");
    assert!(has_code(&plain, 41024), "{plain}");
    for value in ["41024", "comma-operator-used-in-expression"] {
        let error = load(options(|options| {
            options.warnings_as_errors(value);
        }))
        .expect_err("the warning is an error");
        assert_eq!(error.kind(), ErrorKind::Compile);
        assert!(has_code(error.diagnostics(), 41024), "{error}");
        let silenced = load(options(|options| {
            options.disable_warning(value);
        }))
        .expect("a disabled warning does not fail the load");
        assert!(!has_code(&silenced, 41024), "{silenced}");
    }
    let unknown = load(options(|options| {
        options.disable_warning("no-such-diagnostic");
    }))
    .expect("an unknown name does not fail the load");
    assert!(has_code(&unknown, 31111), "{unknown}");
}

#[test]
fn language_version_changes_how_slang_checks_source() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let load = |version| {
        spirv_with(
            &global,
            options(|options| {
                options.language_version(version);
            }),
            CompilerOptions::default(),
        )
        .load_module_from_source("comma", "comma.slang", COMMA)
    };
    let warned = load(LanguageVersion::V2025).expect("the comma operator compiles in 2025");
    assert!(
        has_code(&warned.diagnostics, 41024),
        "{}",
        warned.diagnostics
    );
    let error = load(LanguageVersion::V2026).expect_err("a tuple is no int in 2026");
    assert_eq!(error.kind(), ErrorKind::Compile);
    assert!(!has_code(error.diagnostics(), 41024), "{error}");
}

#[test]
fn optimization_debug_info_and_entry_point_names_change_the_spirv() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let spirv = |session_options: CompilerOptions| {
        compile_spirv(
            &spirv_with(&global, session_options, CompilerOptions::default()),
            "layout",
            LAYOUT,
            0,
        )
    };
    let none = spirv(options(|options| {
        options.optimization(OptimizationLevel::None);
    }));
    let maximal = spirv(options(|options| {
        options.optimization(OptimizationLevel::Maximal);
    }));
    assert_ne!(none, maximal);
    let imports = |words: &[u32]| {
        instructions(words)
            .iter()
            .filter(|instruction| instruction.opcode == OP_EXT_INST_IMPORT)
            .map(|instruction| literal(&instruction.operands[1..]))
            .collect::<Vec<_>>()
    };
    let debug = spirv(options(|options| {
        options.debug_info(DebugInfoLevel::Maximal);
    }));
    let no_debug = spirv(options(|options| {
        options.debug_info(DebugInfoLevel::None);
    }));
    assert!(
        imports(&debug)
            .iter()
            .any(|name| name.starts_with("NonSemantic.Shader.DebugInfo"))
    );
    assert!(
        !imports(&no_debug)
            .iter()
            .any(|name| name.starts_with("NonSemantic"))
    );
    let source = LAYOUT.replace("void main(", "void scale(");
    let session = spirv_with(
        &global,
        options(|options| {
            options.vulkan_use_entry_point_name(Toggle::On);
        }),
        CompilerOptions::default(),
    );
    let module = session
        .load_module_from_source("scaled", "scaled.slang", &source)
        .expect("the source compiles")
        .into_value();
    let program = module
        .entry_point("scale")
        .expect("the entry point exists")
        .link()
        .expect("the entry point links")
        .into_value();
    assert_eq!(entry_point_names(&code(&session, &program)), ["scale"]);
    let plain = spirv_with(
        &global,
        CompilerOptions::default(),
        CompilerOptions::default(),
    );
    let module = plain
        .load_module_from_source("scaled", "scaled.slang", &source)
        .expect("the source compiles")
        .into_value();
    let program = module
        .entry_point("scale")
        .expect("the entry point exists")
        .link()
        .expect("the entry point links")
        .into_value();
    assert_eq!(entry_point_names(&code(&plain, &program)), ["main"]);
}

#[test]
fn the_layout_options_and_preserve_parameters_change_the_spirv() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let spirv = |session_options: CompilerOptions, target: CompilerOptions| {
        compile_spirv(
            &spirv_with(&global, session_options, target),
            "layout",
            LAYOUT,
            0,
        )
    };
    let default = spirv(CompilerOptions::default(), CompilerOptions::default());
    let offsets = |words: &[u32]| decorations(words, DECORATION_OFFSET);
    assert!(offsets(&default).contains(&16));
    let setters: [fn(&mut CompilerOptions); 2] = [
        |options| {
            options.force_dx_layout(Toggle::On);
        },
        |options| {
            options.force_c_layout(Toggle::On);
        },
    ];
    for set in setters {
        let on_session = spirv(options(set), CompilerOptions::default());
        let on_target = spirv(CompilerOptions::default(), options(set));
        assert_ne!(on_session, default);
        assert_eq!(on_session, on_target);
        assert!(
            offsets(&on_session).contains(&4),
            "{:?}",
            offsets(&on_session)
        );
    }
    let preserve = options(|options| {
        options.preserve_parameters(Toggle::On);
    });
    let on_session = spirv(preserve.clone(), CompilerOptions::default());
    let on_target = spirv(CompilerOptions::default(), preserve);
    assert_ne!(on_session, default);
    assert_eq!(on_session, on_target);
    let skipped = spirv(
        options(|options| {
            options.skip_spirv_validation(Toggle::On);
        }),
        CompilerOptions::default(),
    );
    assert_eq!(skipped, default);
}

#[test]
fn a_capability_permits_code_that_needs_it_under_the_restrictive_check() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let source = r#"
        RWStructuredBuffer<float> data;
        [shader("compute")] [numthreads(1, 1, 1)]
        void main() { if (data[0] > 1.0) abort("too large"); data[1] = 2.0; }
    "#;
    let restrictive = raw_int(CompilerOptionName::RestrictiveCapabilityCheck, 1);
    let session = |target: Target| {
        build(
            Session::builder(&global)
                .target(target)
                .options(restrictive.clone()),
        )
    };
    let error = compile(
        &session(Target::new(TargetFormat::Spirv).profile("spirv_1_5")),
        "aborting",
        source,
        0,
    )
    .expect_err("the profile lacks the capability");
    assert_eq!(error.kind(), ErrorKind::Compile);
    assert!(has_code(error.diagnostics(), 41013), "{error}");
    let code = compile(
        &session(
            Target::new(TargetFormat::Spirv)
                .profile("spirv_1_5")
                .capability("abort"),
        ),
        "aborting",
        source,
        0,
    )
    .expect("the capability permits the call")
    .into_value();
    assert_ne!(code.as_bytes(), []);
}

#[test]
fn link_with_options_changes_the_code_but_not_the_reflected_layout_of_the_layout_options() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_with(
        &global,
        CompilerOptions::default(),
        CompilerOptions::default(),
    );
    let default = link(&session, LAYOUT, None);
    let default_code = code(&session, &default);
    let default_offsets = reflected_offsets(&session, &default);
    assert_eq!(default_offsets, [0, 16, 32, 64].map(LayoutSize::Finite));
    let setters: [fn(&mut CompilerOptions); 3] = [
        |options| {
            options.force_dx_layout(Toggle::On);
        },
        |options| {
            options.force_c_layout(Toggle::On);
        },
        |options| {
            options.glsl_force_scalar_layout(Toggle::On);
        },
    ];
    for set in setters {
        let fresh = spirv_with(
            &global,
            CompilerOptions::default(),
            CompilerOptions::default(),
        );
        let linked = link(&fresh, LAYOUT, Some(&options(set)));
        let on_session = spirv_with(&global, options(set), CompilerOptions::default());
        let reference = link(&on_session, LAYOUT, None);
        assert_eq!(code(&fresh, &linked), code(&on_session, &reference));
        assert_ne!(code(&fresh, &linked), default_code);
        assert_ne!(reflected_offsets(&on_session, &reference), default_offsets);
        assert_eq!(reflected_offsets(&fresh, &linked), default_offsets);
    }
    let preserve = options(|options| {
        options.preserve_parameters(Toggle::On);
    });
    let fresh = spirv_with(
        &global,
        CompilerOptions::default(),
        CompilerOptions::default(),
    );
    let linked = link(&fresh, LAYOUT, Some(&preserve));
    let on_session = spirv_with(&global, preserve, CompilerOptions::default());
    let reference = link(&on_session, LAYOUT, None);
    assert_eq!(code(&fresh, &linked), code(&on_session, &reference));
}

#[test]
fn link_with_options_does_not_change_entry_point_names_or_debug_information() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_with(
        &global,
        CompilerOptions::default(),
        CompilerOptions::default(),
    );
    let source = LAYOUT.replace("void main(", "void scale(");
    let module = session
        .load_module_from_source("scaled", "scaled.slang", &source)
        .expect("the source compiles")
        .into_value();
    let entry_point = module.entry_point("scale").expect("the entry point exists");
    let renamed = entry_point
        .link_with_options(&options(|options| {
            options.vulkan_use_entry_point_name(Toggle::On);
        }))
        .expect("the entry point links")
        .into_value();
    assert_eq!(entry_point_names(&code(&session, &renamed)), ["main"]);
    let fresh = spirv_with(
        &global,
        CompilerOptions::default(),
        CompilerOptions::default(),
    );
    let debug = link(
        &fresh,
        LAYOUT,
        Some(&options(|options| {
            options.debug_info(DebugInfoLevel::Maximal);
        })),
    );
    let imports = instructions(&code(&fresh, &debug))
        .iter()
        .filter(|instruction| instruction.opcode == OP_EXT_INST_IMPORT)
        .map(|instruction| literal(&instruction.operands[1..]))
        .collect::<Vec<_>>();
    assert!(
        !imports.iter().any(|name| name.starts_with("NonSemantic")),
        "{imports:?}"
    );
}

#[test]
fn link_with_options_after_code_was_generated_from_the_same_object_has_no_effect() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_with(
        &global,
        CompilerOptions::default(),
        CompilerOptions::default(),
    );
    let module = session
        .load_module_from_source("layout", "layout.slang", LAYOUT)
        .expect("the source compiles")
        .into_value();
    let entry_point = module.entry_point("main").expect("the entry point exists");
    let composed = session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])
        .expect("the parts compose")
        .into_value();
    let before = code(&session, &composed.link().expect("it links").into_value());
    let scalar = options(|options| {
        options.glsl_force_scalar_layout(Toggle::On);
    });
    let after = composed
        .link_with_options(&scalar)
        .expect("it links")
        .into_value();
    assert_eq!(after.as_raw(), composed.as_raw());
    assert_eq!(code(&session, &after), before);
}
