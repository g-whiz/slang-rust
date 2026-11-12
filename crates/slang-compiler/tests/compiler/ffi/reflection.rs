//! The tests of reflection through the public API: the usage flow of the
//! crate-level example, snapshots of the JSON text of a layout and of a
//! printed layout, the checks of positions, null returns, and objects of
//! another session, including those of `ProgramLayout::type_layout`, and the
//! lifetime of the handles.

use std::fmt::Write as _;

use slang_compiler::reflection::{
    BindingType, EntryPointLayout, LayoutSize, ProgramLayout, ScalarType, TypeKind, TypeLayout,
    Variable, VariableLayout,
};
use slang_compiler::{
    BindingIndex, BindingSpace, Component, ErrorKind, GlobalSession, LinkedProgram, MatrixLayout,
    Operation, ParameterCategory, Session, Stage, Target, TargetFormat,
};

/// The compute shader of the usage flow of the crate-level example.
const SCALE: &str = r#"
RWStructuredBuffer<float> data;
[shader("compute")]
[numthreads(64, 1, 1)]
void scale(uint3 id : SV_DispatchThreadID, uniform float factor) { data[id.x] *= factor; }
"#;

/// A compute shader whose parameters use every kind of layout that the
/// printed layout shows: a constant buffer, a parameter block with a texture
/// and a sampler, structured buffers, an array of textures, a matrix, uniform
/// data of the entry point, a user attribute, and a varying input.
const SCENE: &str = r#"
[__AttributeUsage(_AttributeTargets.Var)]
struct HintAttribute
{
    int level;
    float weight;
    string mode;
};

struct Light
{
    float3 position;
    float intensity;
};

struct Material
{
    Texture2D<float4> albedo;
    SamplerState linearSampler;
    float roughness;
};

ConstantBuffer<Light> light;
ParameterBlock<Material> material;
StructuredBuffer<float4> inputs;
RWStructuredBuffer<float4> outputs;
Texture2D<float> masks[4];
[Hint(3, 1.5, "fast")]
uniform float4x4 transform;

[shader("compute")]
[numthreads(8, 4, 1)]
void process(uint3 id : SV_DispatchThreadID, uniform float scale, uniform uint count)
{
    float4 value = inputs[id.x] * scale * light.intensity;
    value += material.albedo.SampleLevel(material.linearSampler, float2(0, 0), material.roughness);
    value += masks[id.y % 4].Load(int3(0, 0, 0));
    outputs[id.x % count] = mul(transform, value);
}
"#;

fn spirv_session(global: &GlobalSession) -> Session {
    Session::builder(global)
        .target(Target::new(TargetFormat::Spirv).profile("spirv_1_5"))
        .build()
        .expect("Slang creates the session")
}

fn link(session: &Session, name: &str, source: &str, entry_point: &str) -> LinkedProgram {
    let module = session
        .load_module_from_source(name, &format!("{name}.slang"), source)
        .expect("the source compiles")
        .into_value();
    let entry_point = module
        .entry_point(entry_point)
        .expect("the entry point exists");
    session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])
        .expect("the parts compose")
        .into_value()
        .link()
        .expect("the composition links")
        .into_value()
}

/// Returns the type layout of the parameters of the first entry point,
/// without the constant buffer that Slang can place around them, as the
/// crate-level example does.
fn entry_parameters(layout: ProgramLayout<'_>) -> TypeLayout<'_> {
    let scope = layout
        .entry_point_layouts()
        .next()
        .and_then(EntryPointLayout::var_layout)
        .and_then(VariableLayout::type_layout)
        .expect("the entry point has a scope");
    match scope.kind().expect("a known kind") {
        TypeKind::ConstantBuffer | TypeKind::ParameterBlock => scope
            .element_var_layout()
            .and_then(VariableLayout::type_layout)
            .unwrap_or(scope),
        _ => scope,
    }
}

#[test]
fn the_usage_flow_lists_the_parameters_of_the_entry_point_with_their_kinds() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let spirv = session.target_index(0).expect("a target");
    let program = link(&session, "kernels", SCALE, "scale");
    let layout = program
        .layout(spirv)
        .expect("Slang lays out the program")
        .into_value();
    let mut listed = Vec::new();
    for field in entry_parameters(layout).field_layouts() {
        let name = field
            .variable()
            .and_then(|variable| variable.name().expect("UTF-8"))
            .unwrap_or("<unnamed>");
        let kind = field.type_layout().map(TypeLayout::kind).transpose();
        listed.push((name, kind.expect("known kinds")));
    }
    assert_eq!(
        listed,
        [
            ("id", Some(TypeKind::Vector)),
            ("factor", Some(TypeKind::Scalar))
        ]
    );
    let entry = layout
        .find_entry_point_by_name("scale")
        .expect("no NUL byte")
        .expect("the entry point is in the layout");
    assert_eq!(entry.name().expect("UTF-8"), Some("scale"));
    assert_eq!(entry.stage().expect("a known stage"), Some(Stage::Compute));
    assert_eq!(entry.compute_thread_group_size(), [64, 1, 1]);
    let globals = layout
        .parameter_layouts()
        .map(|parameter| {
            parameter
                .variable()
                .and_then(|variable| variable.name().expect("UTF-8"))
        })
        .collect::<Vec<_>>();
    assert_eq!(globals, [Some("data")]);
}

/// Writes one line for a variable layout and its type, and the lines of its
/// fields and of the contents of a buffer, indented below it.
fn print_variable(out: &mut String, variable: VariableLayout<'_>, depth: usize) {
    let indent = "  ".repeat(depth);
    let name = variable
        .variable()
        .map_or_default(|v| v.name().expect("UTF-8"))
        .unwrap_or("<scope>");
    let Some(layout) = variable.type_layout() else {
        writeln!(out, "{indent}{name}: no type layout").expect("a String takes text");
        return;
    };
    let ty = layout
        .ty()
        .map_or_default(|ty| ty.full_name().expect("a full name"));
    write!(
        out,
        "{indent}{name}: {ty} ({:?})",
        layout.kind().expect("a known kind")
    )
    .expect("a String takes text");
    for category in layout.categories() {
        let category = category.expect("a known category");
        write!(
            out,
            " [{category:?} offset {:?} space {:?} size {:?}]",
            variable.offset(category).amount,
            variable.binding_space(category).map(BindingSpace::get),
            layout.size(category).amount
        )
        .expect("a String takes text");
    }
    if let Some(semantic) = variable.semantic_name().expect("UTF-8") {
        write!(out, " semantic {semantic}{}", variable.semantic_index())
            .expect("a String takes text");
    }
    if let Some(stage) = variable.stage().expect("a known stage") {
        write!(out, " stage {stage:?}").expect("a String takes text");
    }
    if let Some(mode) = layout.matrix_layout_mode().expect("a known mode") {
        write!(out, " matrix {mode:?}").expect("a String takes text");
    }
    writeln!(out).expect("a String takes text");
    if depth > 6 {
        return;
    }
    for field in layout.field_layouts() {
        print_variable(out, field, depth + 1);
    }
    if let Some(element) = layout.element_var_layout() {
        print_variable(out, element, depth + 1);
    }
}

/// Writes the binding ranges and descriptor sets of a type layout.
fn print_bindings(out: &mut String, layout: TypeLayout<'_>) {
    for range in layout.binding_ranges().expect("a count") {
        let leaf = range
            .leaf_variable()
            .map_or_default(|v| v.name().expect("UTF-8"))
            .unwrap_or("<none>");
        writeln!(
            out,
            "  range {leaf}: {:?} x {:?}, set {:?}, descriptor ranges {} from {}, specializable {}",
            range.binding_type().expect("a known kind"),
            range.binding_count().expect("a count"),
            range.descriptor_set_index().expect("a position"),
            range.descriptor_range_count().expect("a count"),
            range.first_descriptor_range_index().expect("a position"),
            range.is_specializable()
        )
        .expect("a String takes text");
    }
    for set in layout.descriptor_sets().expect("a count") {
        writeln!(
            out,
            "  set at space offset {}",
            set.space_offset().expect("an offset").get()
        )
        .expect("a String takes text");
        for range in set.descriptor_ranges().expect("a count") {
            writeln!(
                out,
                "    {:?} {:?} x {:?} at {:?}",
                range.category().expect("a known category"),
                range.binding_type().expect("a known kind"),
                range.descriptor_count().expect("a count"),
                range
                    .index_offset()
                    .expect("an offset")
                    .map(BindingIndex::get)
            )
            .expect("a String takes text");
        }
    }
}

/// Prints a whole program layout as text that depends only on the source
/// and the target, with no path and no pointer value.
fn print_layout(layout: ProgramLayout<'_>) -> String {
    let mut out = String::new();
    writeln!(
        out,
        "global constant buffer: binding {:?}, size {:?}",
        layout
            .global_constant_buffer_binding()
            .map(BindingIndex::get),
        layout.global_constant_buffer_size()
    )
    .expect("a String takes text");
    let globals = layout
        .global_params_var_layout()
        .expect("the program has global parameters");
    out.push_str("global parameters:\n");
    print_variable(&mut out, globals, 1);
    out.push_str("global binding ranges:\n");
    print_bindings(
        &mut out,
        globals.type_layout().expect("the scope has a layout"),
    );
    for entry in layout.entry_point_layouts() {
        writeln!(
            out,
            "entry point {:?}, override {:?}: stage {:?}, threads {:?}, default constant buffer {}",
            entry.name().expect("UTF-8"),
            entry.name_override().expect("UTF-8"),
            entry.stage().expect("a known stage"),
            entry.compute_thread_group_size(),
            entry.has_default_constant_buffer()
        )
        .expect("a String takes text");
        if let Some(scope) = entry.var_layout() {
            print_variable(&mut out, scope, 1);
        }
        writeln!(
            out,
            "  result: {}",
            if entry.result_var_layout().is_some() {
                "a layout"
            } else {
                "none"
            }
        )
        .expect("a String takes text");
        for parameter in entry.parameter_layouts() {
            let name = parameter
                .variable()
                .map_or_default(|v| v.name().expect("UTF-8"));
            writeln!(out, "  parameter {name:?}").expect("a String takes text");
        }
        if let Some(function) = entry.function() {
            let parameters = function
                .parameters()
                .map(|p| {
                    p.ty()
                        .map_or_default(|ty| ty.full_name().expect("a full name"))
                })
                .collect::<Vec<_>>();
            writeln!(
                out,
                "  function {:?}({}) -> {:?}",
                function.name().expect("UTF-8"),
                parameters.join(", "),
                function
                    .return_type()
                    .map(|ty| ty.full_name().expect("a full name"))
            )
            .expect("a String takes text");
        }
    }
    out
}

#[test]
fn a_printed_layout_matches_its_snapshot() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let program = link(&session, "scene", SCENE, "process");
    let layout = program
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the program");
    assert!(layout.diagnostics.is_empty(), "{}", layout.diagnostics);
    insta::assert_snapshot!("printed_layout", print_layout(layout.value));
}

#[test]
fn the_json_text_of_a_layout_matches_its_snapshot() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let program = link(&session, "scene", SCENE, "process");
    let layout = program
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the program")
        .into_value();
    let json = layout.to_json().expect("Slang writes the layout as JSON");
    assert!(!json.contains('\0'));
    assert!(!json.contains("scene.slang"), "the text holds a path");
    insta::assert_snapshot!("layout_json", json);
}

#[test]
fn types_report_their_kinds_and_shapes() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let program = link(&session, "scene", SCENE, "process");
    let layout = program
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the program")
        .into_value();
    let parameter = |name: &str| {
        layout
            .parameter_layouts()
            .find(|parameter| {
                parameter.variable().and_then(|v| v.name().expect("UTF-8")) == Some(name)
            })
            .expect("the parameter exists")
    };
    let masks = parameter("masks")
        .variable()
        .and_then(Variable::ty)
        .expect("a type");
    assert_eq!(masks.kind().expect("a kind"), TypeKind::Array);
    assert_eq!(masks.element_count(), LayoutSize::Finite(4));
    let texture = masks.element_type().expect("an element type");
    assert_eq!(texture.kind().expect("a kind"), TypeKind::Resource);
    let shape = texture.resource_shape();
    assert_eq!(
        shape.base().expect("a base shape"),
        slang_compiler::reflection::ResourceBaseShape::Texture2d
    );
    assert!(!shape.is_array());
    assert_eq!(
        texture.resource_access().expect("an access"),
        slang_compiler::reflection::ResourceAccess::Read
    );
    let result = texture.resource_result_type().expect("a result type");
    assert_eq!(result.scalar_type().expect("a scalar"), ScalarType::Float32);
    let outputs = parameter("outputs")
        .variable()
        .and_then(Variable::ty)
        .expect("a type");
    assert_eq!(
        outputs.resource_access().expect("an access"),
        slang_compiler::reflection::ResourceAccess::ReadWrite
    );
    let transform = parameter("transform");
    let matrix = transform.variable().and_then(Variable::ty).expect("a type");
    assert_eq!(matrix.kind().expect("a kind"), TypeKind::Matrix);
    assert_eq!((matrix.row_count(), matrix.column_count()), (4, 4));
    assert_eq!(matrix.name().expect("UTF-8"), Some("matrix"));
    assert_eq!(
        transform
            .type_layout()
            .expect("a layout")
            .matrix_layout_mode()
            .expect("a mode"),
        Some(MatrixLayout::RowMajor)
    );
    let light = layout
        .find_type_by_name("Light")
        .expect("no NUL byte")
        .expect("the module defines Light");
    assert_eq!(light.name().expect("UTF-8"), Some("Light"));
    let fields = light
        .fields()
        .map(|field| field.name().expect("UTF-8").unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(fields, ["position", "intensity"]);
    let light_layout = layout
        .type_layout(light)
        .expect("the type belongs to the session")
        .expect("Slang lays out the type");
    assert_eq!(
        light_layout.size(ParameterCategory::Uniform).amount,
        LayoutSize::Finite(16)
    );
    assert_eq!(
        light_layout
            .alignment(ParameterCategory::Uniform)
            .expect("an alignment")
            .amount,
        LayoutSize::Finite(16)
    );
    assert_eq!(
        light_layout
            .find_field_index_by_name("intensity")
            .expect("no NUL byte"),
        Some(1)
    );
    assert_eq!(
        light_layout
            .field_binding_range_offset(1)
            .expect("in range"),
        0
    );
}

/// Returns the layout of the global parameter `name` of the program of
/// [`SCENE`].
fn scene_parameter<'a>(layout: ProgramLayout<'a>, name: &str) -> VariableLayout<'a> {
    layout
        .parameter_layouts()
        .find(|parameter| parameter.variable().and_then(|v| v.name().expect("UTF-8")) == Some(name))
        .expect("the parameter exists")
}

#[test]
fn user_attributes_report_their_arguments() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let program = link(&session, "scene", SCENE, "process");
    let layout = program
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the program")
        .into_value();
    let transform = scene_parameter(layout, "transform");
    let variable = transform.variable().expect("a variable");
    let attributes = variable.user_attributes().collect::<Vec<_>>();
    assert_eq!(attributes.len(), 1);
    let hint = variable
        .find_user_attribute_by_name("Hint")
        .expect("no NUL byte")
        .expect("the attribute exists");
    assert_eq!(hint.name().expect("UTF-8"), Some("Hint"));
    assert_eq!(hint.argument_count(), 3);
    assert_eq!(hint.argument_value_int(0).expect("in range"), Some(3));
    assert_eq!(hint.argument_value_float(1).expect("in range"), Some(1.5));
    assert_eq!(
        hint.argument_value_string(2).expect("in range"),
        Some("fast")
    );
    assert_eq!(hint.argument_value_int(2).expect("in range"), None);
    assert_eq!(hint.argument_value_string(0).expect("in range"), None);
    let argument_type = hint.argument_type(0).expect("in range").expect("a type");
    assert_eq!(
        argument_type.scalar_type().expect("a scalar"),
        ScalarType::Int32
    );
    let hint = transform
        .variable()
        .and_then(|v| v.user_attributes().next())
        .expect("the attribute exists");
    for position in [3, 4, usize::MAX] {
        for error in [
            hint.argument_type(position).map(|_| ()),
            hint.argument_value_int(position).map(|_| ()),
            hint.argument_value_float(position).map(|_| ()),
            hint.argument_value_string(position).map(|_| ()),
        ] {
            let error = error.expect_err("a position past the end fails");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert_eq!(error.operation(), Operation::Reflect);
        }
    }
}

#[test]
fn reflection_checks_every_position_and_returns_none_for_null() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let program = link(&session, "scene", SCENE, "process");
    let layout = program
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the program")
        .into_value();
    let transform = scene_parameter(layout, "transform");
    let light = layout
        .find_type_by_name("Light")
        .expect("no NUL byte")
        .expect("the module defines Light");
    let light_layout = layout
        .type_layout(light)
        .expect("the type belongs to the session")
        .expect("Slang lays out the type");
    for field in [2, 3, usize::MAX] {
        let error = light_layout
            .field_binding_range_offset(field)
            .expect_err("a field past the end fails");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert!(
            error
                .to_string()
                .contains(&format!("field {field} is past the end of the 2")),
            "{error}"
        );
    }
    let entry = layout.entry_point_layouts().next().expect("an entry point");
    assert!(entry.result_var_layout().is_none());
    let scalar = layout
        .find_type_by_name("float")
        .expect("no NUL byte")
        .expect("the core module defines float");
    assert!(scalar.element_type().is_none());
    assert!(scalar.resource_result_type().is_none());
    assert_eq!(scalar.fields().count(), 0);
    let scalar_layout = layout
        .type_layout(scalar)
        .expect("the type belongs to the session")
        .expect("Slang lays out the type");
    assert!(scalar_layout.element_var_layout().is_none());
    assert!(scalar_layout.container_var_layout().is_none());
    assert!(scalar_layout.element_type_layout().is_none());
    assert_eq!(scalar_layout.field_layouts().count(), 0);
    assert_eq!(scalar_layout.matrix_layout_mode().expect("a mode"), None);
    for missing in [
        layout
            .find_type_by_name("Missing")
            .map(|found| found.is_some()),
        layout
            .find_entry_point_by_name("missing")
            .map(|found| found.is_some()),
        light_layout
            .find_field_index_by_name("missing")
            .map(|found| found.is_some()),
        transform
            .variable()
            .expect("a variable")
            .find_user_attribute_by_name("Missing")
            .map(|found| found.is_some()),
    ] {
        assert_eq!(missing.ok(), Some(false));
    }
    for error in [
        layout.find_type_by_name("Li\0ght").map(|_| ()),
        layout.find_entry_point_by_name("pro\0cess").map(|_| ()),
        light_layout
            .find_field_index_by_name("inten\0sity")
            .map(|_| ()),
        transform
            .variable()
            .expect("a variable")
            .find_user_attribute_by_name("Hi\0nt")
            .map(|_| ()),
    ] {
        let error = error.expect_err("a NUL byte fails");
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert!(error.to_string().contains("NUL byte"), "{error}");
    }
}

fn name(parameter: VariableLayout<'_>) -> Option<&str> {
    parameter.variable().and_then(|v| v.name().expect("UTF-8"))
}

#[test]
fn iterators_stop_at_the_count_from_both_ends() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let program = link(&session, "scene", SCENE, "process");
    let layout = program
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the program")
        .into_value();
    let mut parameters = layout.parameter_layouts();
    assert_eq!(parameters.size_hint(), (0, Some(6)));
    let first = parameters.next().expect("a first parameter");
    let last = parameters.next_back().expect("a last parameter");
    assert_eq!(parameters.size_hint(), (0, Some(4)));
    assert_eq!(name(first), Some("light"));
    assert_eq!(name(last), Some("transform"));
    assert_eq!(parameters.by_ref().count(), 4);
    assert!(parameters.next().is_none());
    assert!(parameters.next_back().is_none());
    let reversed = layout
        .parameter_layouts()
        .rev()
        .map(name)
        .collect::<Vec<_>>();
    assert_eq!(
        reversed,
        [
            Some("transform"),
            Some("masks"),
            Some("outputs"),
            Some("inputs"),
            Some("material"),
            Some("light")
        ]
    );
    let globals = layout
        .global_params_var_layout()
        .and_then(VariableLayout::type_layout)
        .expect("the global scope has a layout");
    let mut ranges = globals.binding_ranges().expect("a count");
    assert_eq!(ranges.size_hint(), (1, Some(1)));
    let range = ranges.next_back().expect("one binding range");
    assert_eq!(
        range.binding_type().expect("a known kind"),
        BindingType::ConstantBuffer
    );
    assert_eq!(ranges.len(), 0);
    assert!(ranges.next().is_none());
    let mut sets = globals.descriptor_sets().expect("a count");
    assert_eq!(sets.len(), 1);
    let set = sets.next().expect("one descriptor set");
    assert!(sets.next().is_none());
    let mut descriptor_ranges = set.descriptor_ranges().expect("a count");
    assert_eq!(descriptor_ranges.len(), 5);
    let last = descriptor_ranges.next_back().expect("a last range");
    assert_eq!(
        last.binding_type().expect("a known kind"),
        BindingType::Texture
    );
    assert_eq!(
        last.descriptor_count().expect("a count"),
        LayoutSize::Finite(4)
    );
    let kinds = descriptor_ranges
        .map(|range| range.binding_type().expect("a known kind"))
        .collect::<Vec<_>>();
    assert_eq!(
        kinds,
        [
            BindingType::ConstantBuffer,
            BindingType::ConstantBuffer,
            BindingType::RawBuffer,
            BindingType::MutableRawBuffer
        ]
    );
}

#[test]
fn type_layout_rejects_a_type_of_another_session_before_any_call() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let other = spirv_session(&global);
    let program = link(&session, "kernels", SCALE, "scale");
    let foreign = link(&other, "kernels", SCALE, "scale");
    let target = session.target_index(0).expect("a target");
    let layout = program
        .layout(target)
        .expect("Slang lays out the program")
        .into_value();
    let foreign_layout = foreign
        .layout(other.target_index(0).expect("a target"))
        .expect("Slang lays out the program")
        .into_value();
    let theirs = foreign_layout
        .find_type_by_name("float")
        .expect("no NUL byte")
        .expect("the core module defines float");
    let error = layout
        .type_layout(theirs)
        .expect_err("a type of another session fails");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.operation(), Operation::Reflect);
    assert_eq!(error.code(), None);
    assert_eq!(
        error.to_string(),
        "cannot read reflection: the type belongs to another session, and Slang resolves it only in its own session"
    );
    let mine = layout
        .find_type_by_name("float")
        .expect("no NUL byte")
        .expect("the core module defines float");
    assert!(layout.type_layout(mine).expect("same session").is_some());
}

#[test]
fn layout_rejects_a_target_index_of_another_session_past_the_end() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let wide = Session::builder(&global)
        .target(Target::new(TargetFormat::Spirv))
        .target(Target::new(TargetFormat::Hlsl))
        .build()
        .expect("Slang creates the session");
    let second = wide.target_index(1).expect("a second target");
    let module = session
        .load_module_from_source("kernels", "kernels.slang", SCALE)
        .expect("the source compiles")
        .into_value();
    let error = module
        .layout(second)
        .expect_err("a target past the end fails");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    assert_eq!(error.operation(), Operation::Layout);
    let entry_point = module.entry_point("scale").expect("the entry point exists");
    let error = entry_point
        .layout(second)
        .expect_err("a target past the end fails");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    let program = link(&session, "kernels", SCALE, "scale");
    let error = program
        .layout(second)
        .expect_err("a target past the end fails");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
}

#[test]
fn every_component_type_has_a_layout_and_an_entry_point_has_a_function() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let target = session.target_index(0).expect("a target");
    let module = session
        .load_module_from_source("kernels", "kernels.slang", SCALE)
        .expect("the source compiles")
        .into_value();
    let module_layout = module
        .layout(target)
        .expect("Slang lays out the module")
        .into_value();
    assert_eq!(module_layout.entry_point_layouts().count(), 0);
    assert_eq!(module_layout.parameter_layouts().count(), 1);
    let entry_point = module.entry_point("scale").expect("the entry point exists");
    let function = entry_point
        .function()
        .expect("the entry point has a function");
    assert_eq!(function.name().expect("UTF-8"), Some("scale"));
    let parameters = function
        .parameters()
        .map(|p| p.name().expect("UTF-8").unwrap_or_default())
        .collect::<Vec<_>>();
    assert_eq!(parameters, ["id", "factor"]);
    assert_eq!(
        function
            .return_type()
            .map(|ty| ty.full_name().expect("a name")),
        Some("void".to_owned())
    );
    let entry_layout = entry_point
        .layout(target)
        .expect("Slang lays out the entry point")
        .into_value();
    assert_eq!(entry_layout.entry_point_layouts().count(), 1);
    let composed = session
        .compose(&[
            Component::Module(&module),
            Component::EntryPoint(&entry_point),
        ])
        .expect("the parts compose")
        .into_value();
    let composed_layout = composed
        .layout(target)
        .expect("Slang lays out the composition")
        .into_value();
    assert_eq!(composed_layout.entry_point_layouts().count(), 1);
    assert_eq!(
        composed
            .layout(target)
            .expect("the same layout")
            .into_value()
            .as_raw(),
        composed_layout.as_raw()
    );
}

/// Links the kernel of the usage flow and returns only the program, so that
/// every other handle to its session and global session is dropped.
fn program_alone() -> (LinkedProgram, slang_compiler::TargetIndex) {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let target = session.target_index(0).expect("a target");
    (link(&session, "kernels", SCALE, "scale"), target)
}

#[test]
fn reflection_works_after_every_other_handle_to_the_session_is_dropped() {
    let (program, target) = program_alone();
    let layout = program
        .layout(target)
        .expect("Slang lays out the program")
        .into_value();
    let parameters = entry_parameters(layout);
    let names = parameters
        .field_layouts()
        .map(|field| {
            field
                .variable()
                .and_then(|v| v.name().expect("UTF-8"))
                .unwrap_or_default()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, ["id", "factor"]);
    let float = layout
        .find_type_by_name("float")
        .expect("no NUL byte")
        .expect("the core module defines float");
    assert_eq!(float.full_name().expect("a name"), "float");
}

/// A compute shader with an empty structure, a structure of several
/// parameter categories, a buffer with a counter, and a resource at an
/// explicit binding number in an explicit binding space.
const BINDINGS: &str = r#"
struct Empty {};

struct Mixed
{
    Texture2D<float4> texture;
    SamplerState sampler;
    float weight;
};

AppendStructuredBuffer<float4> appended;
RWStructuredBuffer<float4> plain;
Texture2D<float> masks[4];
[[vk::binding(2, 5)]] Texture2D<float> pinned;

[shader("compute")]
[numthreads(1, 1, 1)]
void main(uint3 id : SV_DispatchThreadID)
{
    appended.Append(plain[id.x] + masks[id.x % 4].Load(int3(0, 0, 0)) + pinned.Load(int3(0, 0, 0)));
}
"#;

/// Links [`BINDINGS`] and runs `check` on its layout.
fn with_bindings_layout(check: fn(ProgramLayout<'_>)) {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let program = link(&session, "bindings", BINDINGS, "main");
    check(
        program
            .layout(session.target_index(0).expect("a target"))
            .expect("Slang lays out the program")
            .into_value(),
    );
}

/// Returns the type layout of the type `name` of a layout of [`BINDINGS`].
fn named_type_layout<'a>(layout: ProgramLayout<'a>, name: &str) -> TypeLayout<'a> {
    let ty = layout
        .find_type_by_name(name)
        .expect("a valid name")
        .expect("the type exists");
    layout
        .type_layout(ty)
        .expect("the type belongs to the session")
        .expect("Slang lays out the type")
}

#[test]
fn parameter_category_is_one_mixed_or_none() {
    with_bindings_layout(|layout| {
        for (name, category) in [
            ("float", Some(ParameterCategory::Uniform)),
            ("Mixed", Some(ParameterCategory::Mixed)),
            ("Empty", None),
        ] {
            assert_eq!(
                named_type_layout(layout, name)
                    .parameter_category()
                    .expect("a known category"),
                category,
                "{name}"
            );
        }
        for name in ["appended", "plain", "masks", "pinned"] {
            assert_eq!(
                scene_parameter(layout, name)
                    .type_layout()
                    .expect("a type layout")
                    .parameter_category()
                    .expect("a known category"),
                Some(ParameterCategory::DescriptorTableSlot),
                "{name}"
            );
        }
    });
}

#[test]
fn counters_and_leaves_of_binding_ranges_report_what_slang_computes() {
    with_bindings_layout(|layout| {
        let counter = |name: &str| {
            scene_parameter(layout, name)
                .type_layout()
                .expect("a type layout")
                .explicit_counter_binding_range_offset()
                .expect("an offset")
        };
        assert_eq!(counter("appended"), 1);
        for name in ["plain", "masks", "pinned"] {
            assert_eq!(counter(name), 0, "{name} has no counter");
        }
        let globals = layout
            .global_params_var_layout()
            .and_then(VariableLayout::type_layout)
            .expect("the global scope has a layout");
        let leaves = globals
            .binding_ranges()
            .expect("a count")
            .map(|range| {
                let leaf = range
                    .leaf_type_layout()
                    .expect("every range of the program has a leaf");
                (
                    range
                        .leaf_variable()
                        .and_then(|v| v.name().expect("UTF-8"))
                        .unwrap_or_default(),
                    leaf.kind().expect("a known kind"),
                    leaf.ty()
                        .map_or_default(|ty| ty.full_name().expect("a name")),
                    range.binding_count().expect("a count"),
                )
            })
            .collect::<Vec<_>>();
        let resource = |name: &'static str, ty: &str, count| {
            (
                name,
                TypeKind::Resource,
                ty.to_owned(),
                LayoutSize::Finite(count),
            )
        };
        assert_eq!(
            leaves,
            [
                resource(
                    "appended",
                    "AppendStructuredBuffer<vector<float,4>, DefaultDataLayout>",
                    1
                ),
                resource("counter", "RWStructuredBuffer<int>", 1),
                resource(
                    "plain",
                    "RWStructuredBuffer<vector<float,4>, DefaultDataLayout>",
                    1
                ),
                resource("masks", "Texture2D<float>", 4),
                resource("pinned", "Texture2D<float>", 1),
            ]
        );
    });
}

#[test]
fn binding_spaces_and_binding_numbers_report_explicit_bindings() {
    with_bindings_layout(|layout| {
        let slot = ParameterCategory::DescriptorTableSlot;
        let pinned = scene_parameter(layout, "pinned");
        assert_eq!(pinned.binding_space(slot), Some(BindingSpace::new(5)));
        assert_eq!(pinned.offset(slot).amount, LayoutSize::Finite(2));
        assert_eq!(
            scene_parameter(layout, "masks").binding_space(slot),
            Some(BindingSpace::new(0))
        );
        assert_eq!(
            layout.global_constant_buffer_binding(),
            Some(BindingIndex::new(0))
        );
        let globals = layout
            .global_params_var_layout()
            .and_then(VariableLayout::type_layout)
            .expect("the global scope has a layout");
        let sets = globals
            .descriptor_sets()
            .expect("a count")
            .map(|set| {
                let offsets = set
                    .descriptor_ranges()
                    .expect("a count")
                    .map(|range| range.index_offset().expect("an offset"))
                    .collect::<Vec<_>>();
                (set.space_offset().expect("an offset"), offsets)
            })
            .collect::<Vec<_>>();
        let index = |number| Some(BindingIndex::new(number));
        assert_eq!(
            sets,
            [
                (
                    BindingSpace::new(0),
                    vec![index(0), index(1), index(2), index(3)]
                ),
                (BindingSpace::new(5), vec![index(2)]),
            ]
        );
    });
}
