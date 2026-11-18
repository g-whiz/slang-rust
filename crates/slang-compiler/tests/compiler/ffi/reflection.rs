//! The tests of reflection through the public API: the usage flow of the
//! crate-level example, snapshots of the JSON text of a layout and of a
//! printed layout, the checks of positions, null returns, and objects of
//! another session, including those of `ProgramLayout::type_layout`, and the
//! lifetime of the handles.

use std::fmt::Write as _;

use slang_compiler::reflection::{
    BindingType, DeclKind, EntryPointLayout, Function, LayoutSize, ModifierId, ProgramLayout,
    ScalarType, Type, TypeKind, TypeLayout, Variable, VariableLayout,
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
        .load_module_from_source(name, format!("{name}.slang"), source)
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

#[test]
fn a_field_layout_is_found_by_its_position_as_the_iterator_yields_it() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let program = link(&session, "scene", SCENE, "process");
    let layout = program
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the program")
        .into_value();
    let light = layout
        .find_type_by_name("Light")
        .expect("no NUL byte")
        .expect("the module defines Light");
    let light_layout = layout
        .type_layout(light)
        .expect("the type belongs to the session")
        .expect("Slang lays out the type");
    let position = light_layout
        .find_field_index_by_name("intensity")
        .expect("no NUL byte")
        .expect("Light has the field");
    let intensity = light_layout
        .field_layout(position)
        .expect("in range")
        .expect("Slang lays out the field");
    assert_eq!(name(intensity), Some("intensity"));
    assert_eq!(
        intensity.offset(ParameterCategory::Uniform).amount,
        LayoutSize::Finite(12)
    );
    let by_position = (0..2)
        .map(|position| {
            light_layout
                .field_layout(position)
                .expect("in range")
                .and_then(name)
        })
        .collect::<Vec<_>>();
    let iterated = light_layout.field_layouts().map(name).collect::<Vec<_>>();
    assert_eq!(by_position, iterated);
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
        for result in [
            light_layout.field_binding_range_offset(field).map(|_| ()),
            light_layout.field_layout(field).map(|_| ()),
        ] {
            let error = result.expect_err("a field past the end fails");
            assert_eq!(error.kind(), ErrorKind::InvalidArgument);
            assert!(
                error
                    .to_string()
                    .contains(&format!("field {field} is past the end of the 2")),
                "{error}"
            );
        }
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
        assert_eq!(counter("appended"), Some(1));
        for name in ["plain", "masks", "pinned"] {
            assert_eq!(counter(name), None, "{name} has no counter");
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

/// A module with an overloaded function, generic functions with a type
/// parameter, a constraint, and a value parameter, a function marked
/// `[Differentiable]`, a function with a user attribute with arguments, and
/// generic structures with a method and a generic method.
const FUNCTIONS: &str = r#"
[__AttributeUsage(_AttributeTargets.Function)]
struct KernelAttribute
{
    string mode;
    int level;
};

float pick(float value) { return value; }
float pick(float value, float other) { return value + other; }
int pick(int value) { return value; }

T twice<T : IArithmetic>(T value) { return value + value; }

float sum<let N : int>(float values[N])
{
    float total = 0;
    for (int i = 0; i < N; i++)
        total += values[i];
    return total;
}

[Differentiable]
float square(float x) { return x * x; }

[Kernel("fast", 3)]
void tagged(inout float value, out float result)
{
    value += 1.0;
    result = value;
}

struct Box<T : IArithmetic>
{
    T inner;
    T get() { return inner; }
    U convert<U : IArithmetic>(U scale) { return scale; }
}

struct Grid<let N : int>
{
    float cells[N];
}

[shader("compute")]
[numthreads(1, 1, 1)]
void main() {}
"#;

/// Loads `FUNCTIONS` into `session` and returns the module.
fn functions_module(session: &Session) -> slang_compiler::Module {
    session
        .load_module_from_source("functions", "functions.slang", FUNCTIONS)
        .expect("the source compiles")
        .into_value()
}

/// Finds the function `name` in `layout`.
fn function<'a>(layout: ProgramLayout<'a>, name: &str) -> Function<'a> {
    layout
        .find_function_by_name(name)
        .expect("no NUL byte")
        .unwrap_or_else(|| panic!("the module defines {name}"))
}

/// Finds the type `name` in `layout`.
fn named_type<'a>(layout: ProgramLayout<'a>, name: &str) -> Type<'a> {
    layout
        .find_type_by_name(name)
        .expect("no NUL byte")
        .unwrap_or_else(|| panic!("the module defines {name}"))
}

fn full_names<'a>(types: impl Iterator<Item = Type<'a>>) -> Vec<String> {
    types
        .map(|ty| ty.full_name().expect("a full name"))
        .collect()
}

#[test]
fn functions_are_found_by_name_also_inside_a_type_and_as_an_expression() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let module = functions_module(&session);
    let layout = module
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the module")
        .into_value();
    let square = function(layout, "square");
    assert_eq!(square.name().expect("UTF-8"), Some("square"));
    assert!(!square.is_overloaded());
    assert_eq!(square.overloads().count(), 0);
    assert_eq!(square.parameters().count(), 1);
    assert!(
        layout
            .find_function_by_name("missing")
            .expect("no NUL byte")
            .is_none()
    );
    assert!(
        layout
            .find_function_by_name("1 + 2")
            .expect("no NUL byte")
            .is_none()
    );
    let specialized = function(layout, "twice<float>");
    let generic = specialized.generic_container().expect("a generic");
    let parameter = generic.type_parameters().next().expect("T");
    assert_eq!(
        generic
            .concrete_type(parameter)
            .expect("a parameter of the generic")
            .map(|ty| ty.full_name().expect("a full name")),
        Some(String::from("float"))
    );
    let boxed = named_type(layout, "Box<float>");
    let get = layout
        .find_function_by_name_in_type(boxed, "get")
        .expect("same session")
        .expect("Box has get");
    assert_eq!(get.name().expect("UTF-8"), Some("get"));
    assert_eq!(
        get.return_type()
            .map(|ty| ty.full_name().expect("a full name")),
        Some(String::from("float"))
    );
    assert!(
        layout
            .find_function_by_name_in_type(boxed, "missing")
            .expect("same session")
            .is_none()
    );
    let error = layout
        .find_function_by_name("bad\0name")
        .expect_err("a NUL byte fails");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
    let error = layout
        .find_function_by_name_in_type(boxed, "bad\0name")
        .expect_err("a NUL byte fails");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
}

#[test]
fn an_overloaded_name_lists_its_overloads_and_resolves_for_argument_types() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let module = functions_module(&session);
    let layout = module
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the module")
        .into_value();
    let pick = function(layout, "pick");
    assert!(pick.is_overloaded());
    let arities = pick
        .overloads()
        .map(|overload| overload.parameters().count())
        .collect::<Vec<_>>();
    assert_eq!(arities, [1, 2, 1]);
    let mut overloads = pick.overloads();
    assert_eq!(overloads.size_hint(), (0, Some(3)));
    assert!(overloads.next_back().is_some());
    assert_eq!(overloads.count(), 2);
    let float = named_type(layout, "float");
    let int = named_type(layout, "int");
    let boolean = named_type(layout, "bool");
    let signature = |argument_types: &[Type<'_>]| {
        pick.specialize_with_arg_types(argument_types)
            .expect("same session")
            .map(|chosen| {
                full_names(
                    chosen
                        .parameters()
                        .map(|parameter| parameter.ty().expect("a type")),
                )
            })
    };
    assert_eq!(signature(&[int]), Some(vec![String::from("int")]));
    assert_eq!(
        signature(&[float, float]),
        Some(vec![String::from("float"), String::from("float")])
    );
    assert_eq!(signature(&[boolean, boolean, boolean]), None);
    assert_eq!(signature(&[]), None);
}

#[test]
fn attributes_and_modifiers_of_functions_and_parameters_are_read() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let module = functions_module(&session);
    let layout = module
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the module")
        .into_value();
    let every = [
        ModifierId::Shared,
        ModifierId::NoDiff,
        ModifierId::Static,
        ModifierId::Const,
        ModifierId::Export,
        ModifierId::Extern,
        ModifierId::Differentiable,
        ModifierId::Mutating,
        ModifierId::In,
        ModifierId::Out,
        ModifierId::InOut,
    ];
    let modifiers_of = |has: &dyn Fn(ModifierId) -> bool| {
        every
            .into_iter()
            .filter(|modifier| has(*modifier))
            .collect::<Vec<_>>()
    };
    let square = function(layout, "square");
    assert_eq!(
        modifiers_of(&|modifier| square.has_modifier(modifier)),
        [ModifierId::Differentiable]
    );
    assert_eq!(square.user_attributes().count(), 0);
    let pick = function(layout, "pick");
    assert_eq!(modifiers_of(&|modifier| pick.has_modifier(modifier)), []);
    let tagged = function(layout, "tagged");
    assert_eq!(modifiers_of(&|modifier| tagged.has_modifier(modifier)), []);
    let parameters = tagged
        .parameters()
        .map(|parameter| modifiers_of(&|modifier| parameter.has_modifier(modifier)))
        .collect::<Vec<_>>();
    assert_eq!(
        parameters,
        [
            vec![ModifierId::Out, ModifierId::InOut],
            vec![ModifierId::Out]
        ]
    );
    let names = tagged
        .user_attributes()
        .map(|attribute| attribute.name().expect("UTF-8"))
        .collect::<Vec<_>>();
    assert_eq!(names, [Some("Kernel")]);
    let kernel = tagged
        .find_user_attribute_by_name("Kernel")
        .expect("no NUL byte")
        .expect("tagged has Kernel");
    assert_eq!(kernel.argument_count(), 2);
    assert_eq!(
        kernel.argument_value_string(0).expect("a position"),
        Some("fast")
    );
    assert_eq!(kernel.argument_value_int(1).expect("a position"), Some(3));
    assert!(
        tagged
            .find_user_attribute_by_name("Missing")
            .expect("no NUL byte")
            .is_none()
    );
    let error = tagged
        .find_user_attribute_by_name("bad\0name")
        .expect_err("a NUL byte fails");
    assert_eq!(error.kind(), ErrorKind::InvalidArgument);
}

#[test]
fn generic_containers_list_their_parameters_constraints_and_arguments() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let module = functions_module(&session);
    let layout = module
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the module")
        .into_value();
    assert!(function(layout, "square").generic_container().is_none());
    let twice = function(layout, "twice");
    let generic = twice.generic_container().expect("twice is generic");
    assert_eq!(generic.name().expect("UTF-8"), Some("twice"));
    assert_eq!(generic.inner_kind().expect("a known kind"), DeclKind::Func);
    assert!(generic.outer_generic_container().is_none());
    assert_eq!(generic.value_parameters().count(), 0);
    let parameters = generic.type_parameters().collect::<Vec<_>>();
    assert_eq!(parameters.len(), 1);
    let parameter = parameters[0];
    assert_eq!(parameter.name().expect("UTF-8"), Some("T"));
    assert_eq!(
        full_names(
            generic
                .type_parameter_constraints(parameter)
                .expect("a type parameter of the generic")
        ),
        ["IArithmetic"]
    );
    assert!(
        generic
            .concrete_type(parameter)
            .expect("a type parameter of the generic")
            .is_none()
    );

    let float = named_type(layout, "float");
    let specialized = twice
        .specialize_with_arg_types(&[float])
        .expect("same session")
        .expect("twice takes a float");
    assert_eq!(
        specialized
            .return_type()
            .map(|ty| ty.full_name().expect("a full name")),
        Some(String::from("float"))
    );
    let arguments = specialized.generic_container().expect("a generic");
    let applied = twice
        .apply_specializations(arguments)
        .expect("same session")
        .expect("Slang applies the arguments");
    assert_eq!(
        applied
            .return_type()
            .map(|ty| ty.full_name().expect("a full name")),
        Some(String::from("float"))
    );
    let open = twice.return_type().expect("T");
    assert_eq!(
        open.apply_specializations(arguments)
            .expect("same session")
            .map(|ty| ty.full_name().expect("a full name")),
        Some(String::from("float"))
    );
    let applied_generic = generic
        .apply_specializations(arguments)
        .expect("same session")
        .expect("Slang applies the arguments");
    let filled = applied_generic
        .type_parameters()
        .map(|parameter| {
            applied_generic
                .concrete_type(parameter)
                .expect("a type parameter of the generic")
                .map(|ty| ty.full_name().expect("a full name"))
        })
        .collect::<Vec<_>>();
    assert_eq!(filled, [Some(String::from("float"))]);
}

#[test]
fn value_parameters_and_outer_generics_report_their_arguments() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let module = functions_module(&session);
    let layout = module
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the module")
        .into_value();
    let sum = function(layout, "sum")
        .generic_container()
        .expect("sum is generic");
    assert_eq!(sum.type_parameters().count(), 0);
    let count = sum.value_parameters().next().expect("N");
    assert_eq!(count.name().expect("UTF-8"), Some("N"));
    assert_eq!(sum.concrete_int_val(count).expect("a value parameter"), 0);
    let grid = named_type(layout, "Grid<4>")
        .generic_container()
        .expect("Grid is generic");
    assert_eq!(grid.inner_kind().expect("a known kind"), DeclKind::Struct);
    let cells = grid.value_parameters().next().expect("N");
    assert_eq!(grid.concrete_int_val(cells).expect("a value parameter"), 4);

    let boxed = named_type(layout, "Box<float>");
    let convert = layout
        .find_function_by_name_in_type(boxed, "convert")
        .expect("same session")
        .expect("Box has convert");
    let inner = convert.generic_container().expect("convert is generic");
    assert_eq!(inner.name().expect("UTF-8"), Some("convert"));
    let outer = inner.outer_generic_container().expect("Box is generic");
    assert_eq!(outer.name().expect("UTF-8"), Some("Box"));
    let element = outer.type_parameters().next().expect("T of Box");
    assert_eq!(
        full_names(
            outer
                .type_parameter_constraints(element)
                .expect("T is a type parameter of Box")
        ),
        ["IArithmetic"]
    );
    let filled = outer
        .concrete_type(element)
        .expect("T is a type parameter of Box")
        .map(|ty| ty.full_name().expect("UTF-8"));
    assert_eq!(filled, Some(String::from("float")));
    assert_eq!(
        inner
            .concrete_type(element)
            .map(|_| ())
            .expect_err("T of Box is not a parameter of convert")
            .kind(),
        ErrorKind::InvalidArgument
    );
    assert_eq!(
        boxed.generic_container().map(|g| g.name().expect("UTF-8")),
        Some(Some("Box"))
    );
}

#[test]
fn a_parameter_of_another_generic_or_kind_fails_before_any_call() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let module = functions_module(&session);
    let layout = module
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the module")
        .into_value();
    let twice = function(layout, "twice")
        .generic_container()
        .expect("twice is generic");
    let sum = function(layout, "sum")
        .generic_container()
        .expect("sum is generic");
    let boxed = named_type(layout, "Box<float>")
        .generic_container()
        .expect("Box is generic");
    let theirs = boxed.type_parameters().next().expect("T of Box");
    let value = sum.value_parameters().next().expect("N");
    let mine = twice.type_parameters().next().expect("T of twice");
    let field = function(layout, "square").parameters().next().expect("x");
    for error in [
        twice
            .type_parameter_constraints(theirs)
            .map(|_| ())
            .expect_err("T of Box is not a parameter of twice"),
        twice
            .concrete_type(field)
            .map(|_| ())
            .expect_err("x is not a parameter of twice"),
        twice
            .concrete_type(value)
            .map(|_| ())
            .expect_err("N is a value parameter"),
        sum.concrete_int_val(mine)
            .map(|_| ())
            .expect_err("T is a type parameter"),
        sum.concrete_int_val(field)
            .map(|_| ())
            .expect_err("x is not a parameter of sum"),
    ] {
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::Reflect);
        assert_eq!(error.code(), None);
    }
    let error = sum
        .concrete_int_val(field)
        .expect_err("x is not a parameter of sum");
    assert_eq!(
        error.to_string(),
        "cannot read reflection: the variable is not a value parameter of this generic, and Slang reads it as one without a check"
    );
}

#[test]
fn functions_and_generics_reject_handles_of_another_session_before_any_call() {
    let global = GlobalSession::new().expect("Slang creates a global session");
    let session = spirv_session(&global);
    let other = spirv_session(&global);
    let module = functions_module(&session);
    let foreign_module = functions_module(&other);
    let layout = module
        .layout(session.target_index(0).expect("a target"))
        .expect("Slang lays out the module")
        .into_value();
    let foreign = foreign_module
        .layout(other.target_index(0).expect("a target"))
        .expect("Slang lays out the module")
        .into_value();
    let twice = function(layout, "twice");
    let generic = twice.generic_container().expect("twice is generic");
    let their_twice = function(foreign, "twice");
    let their_generic = their_twice.generic_container().expect("twice is generic");
    let their_parameter = their_generic.type_parameters().next().expect("T");
    let their_float = named_type(foreign, "float");
    let their_box = named_type(foreign, "Box<float>");
    let their_value = function(foreign, "sum")
        .generic_container()
        .expect("sum is generic")
        .value_parameters()
        .next()
        .expect("N");
    let sum = function(layout, "sum")
        .generic_container()
        .expect("sum is generic");
    let errors = [
        layout
            .find_function_by_name_in_type(their_box, "get")
            .map(|_| ())
            .expect_err("a type of another session"),
        twice
            .specialize_with_arg_types(&[their_float])
            .map(|_| ())
            .expect_err("a type of another session"),
        twice
            .apply_specializations(their_generic)
            .map(|_| ())
            .expect_err("a generic of another session"),
        twice
            .return_type()
            .expect("T")
            .apply_specializations(their_generic)
            .map(|_| ())
            .expect_err("a generic of another session"),
        generic
            .apply_specializations(their_generic)
            .map(|_| ())
            .expect_err("a generic of another session"),
        generic
            .type_parameter_constraints(their_parameter)
            .map(|_| ())
            .expect_err("a parameter of another session"),
        generic
            .concrete_type(their_parameter)
            .map(|_| ())
            .expect_err("a parameter of another session"),
        sum.concrete_int_val(their_value)
            .map(|_| ())
            .expect_err("a parameter of another session"),
    ];
    for error in errors {
        assert_eq!(error.kind(), ErrorKind::InvalidArgument);
        assert_eq!(error.operation(), Operation::Reflect);
        assert_eq!(error.code(), None);
        assert!(
            error.to_string().contains("belongs to another session"),
            "{error}"
        );
    }
    let error = twice
        .specialize_with_arg_types(&[named_type(layout, "float"), their_float])
        .expect_err("a type of another session");
    assert_eq!(
        error.to_string(),
        "cannot read reflection: argument type 1 belongs to another session, and Slang resolves it only in its own session"
    );
}
