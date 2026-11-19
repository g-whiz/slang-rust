use std::path::{Path, PathBuf};

use slango::__private::{
    Access, ElementType, FieldType, ParameterKind, Scalar, Structure, precompile_file,
};
use slango_codegen::{ErrorKind, generate};

use crate::common::{bindings, failure, same_file, shader};

#[test]
fn generate_includes_each_file_by_its_absolute_path_and_embeds_the_encoded_snapshot() {
    let path = shader("saxpy.slang");
    let tokens = generate(&path)
        .expect("the bindings of the shader")
        .to_string();
    let precompiled = precompile_file(&path).expect("the module compiles");
    let dependencies = &precompiled.interface.dependencies;
    assert!(
        dependencies
            .iter()
            .any(|dependency| same_file(dependency, &path)),
        "{dependencies:?}"
    );
    for dependency in dependencies {
        assert!(dependency.is_absolute(), "{dependency:?}");
        let include = format!(
            "include_str ! ({:?})",
            dependency.to_str().expect("a UTF-8 path")
        );
        assert!(tokens.contains(&include), "{include}\n{tokens}");
    }
    let encoded = proc_macro2::Literal::byte_string(&precompiled.encoded).to_string();
    assert!(tokens.contains(&encoded), "{tokens}");
}

#[test]
fn saxpy_gives_typed_buffers_and_values() {
    insta::assert_snapshot!(bindings("saxpy.slang"));
}

#[test]
fn a_constant_buffer_a_texture_a_sampler_and_two_kernels_give_typed_fields() {
    insta::assert_snapshot!(bindings("resources.slang"));
}

#[test]
fn an_imported_file_is_among_the_files_read_and_included() {
    let precompiled = precompile_file(&shader("imports.slang")).expect("the module compiles");
    let dependencies = &precompiled.interface.dependencies;
    assert_eq!(dependencies.len(), 2, "{dependencies:?}");
    for expected in ["common.slang", "imports.slang"] {
        assert!(
            dependencies
                .iter()
                .any(|dependency| same_file(dependency, &shader(expected))),
            "{expected} in {dependencies:?}"
        );
    }
    let [kernel] = precompiled.interface.kernels.as_slice() else {
        panic!("one kernel: {:?}", precompiled.interface.kernels);
    };
    assert_eq!(kernel.name, "doubleValues");
    assert_eq!(kernel.thread_group_size, [32, 1, 1]);
    assert!(matches!(
        kernel.parameters.as_slice(),
        [parameter] if parameter.path == "data"
            && parameter.kind == ParameterKind::Buffer {
                access: Access::ReadWrite,
                element: ElementType::Plain { scalar: Scalar::Float32, components: 1 },
                packed: None,
            }
    ));
    let text = bindings("imports.slang");
    assert!(
        text.contains("::core::include_str!(\"<shaders>/common.slang\");"),
        "{text}"
    );
    assert!(
        text.contains("::core::include_str!(\"<shaders>/imports.slang\");"),
        "{text}"
    );
}

#[test]
fn a_shader_that_does_not_compile_is_a_compile_error_with_the_messages_of_slang() {
    let (kind, text, causes) = failure("broken.slang");
    assert_eq!(kind, ErrorKind::Compile);
    assert_eq!(text, "cannot generate the bindings of the Slang file");
    assert!(
        causes.iter().any(|cause| cause.contains("undefinedValue")),
        "{causes:?}"
    );
}

#[test]
fn a_missing_file_is_a_compile_error() {
    let (kind, text, causes) = failure("missing.slang");
    assert_eq!(kind, ErrorKind::Compile);
    assert_eq!(text, "cannot generate the bindings of the Slang file");
    assert_ne!(causes, Vec::<String>::new());
}

#[test]
fn a_buffer_of_float3_is_unsupported() {
    let (kind, text, causes) = failure("vector3.slang");
    assert_eq!(kind, ErrorKind::Unsupported);
    assert_eq!(
        text,
        "cannot generate the bindings: the parameter `points` of the kernel `move` is not supported, because it is a structured buffer of `float3`, which has no built-in element type in slango"
    );
    assert_eq!(causes, Vec::<String>::new());
}

#[test]
fn a_structure_element_with_padding_gives_a_struct_with_its_layout() {
    insta::assert_snapshot!(bindings("particles.slang"));
}

#[test]
fn a_nested_structure_and_two_kernels_of_one_structure_give_each_struct_once() {
    insta::assert_snapshot!(bindings("nested.slang"));
}

#[test]
fn a_uniform_structure_is_set_field_by_field_and_an_element_serves_as_both() {
    insta::assert_snapshot!(bindings("uniform.slang"));
}

#[test]
fn a_64_bit_scalar_is_unsupported() {
    let (kind, text, causes) = failure("double.slang");
    assert_eq!(kind, ErrorKind::Unsupported);
    assert_eq!(text, "cannot generate the bindings of the Slang file");
    assert!(
        causes.iter().any(|cause| cause.contains("factor")),
        "{causes:?}"
    );
}

#[test]
fn each_field_kind_that_a_generated_struct_does_not_support_is_unsupported() {
    let shaders = [
        ("field_bool.slang", "a `bool`"),
        ("field_matrix.slang", "a matrix"),
        (
            "field_array.slang",
            "an array whose stride of 16 bytes differs from the 12 bytes of its element",
        ),
        ("field_half.slang", "of the type `half`"),
        ("field_double.slang", "of the type `double`"),
    ];
    for (name, what) in shaders {
        let (kind, text, causes) = failure(name);
        assert_eq!(kind, ErrorKind::Unsupported, "{name}");
        assert_eq!(
            text,
            format!(
                "cannot generate the bindings: the parameter `cells` of the kernel `grow` is not supported, because the field `extra` of the structure `Cell` is {what}, which a generated struct does not support"
            ),
            "{name}"
        );
        assert_eq!(causes, Vec::<String>::new(), "{name}");
    }
}

#[test]
fn two_structures_of_one_name_in_two_namespaces_conflict() {
    let (kind, text, _) = failure("same_name.slang");
    assert_eq!(kind, ErrorKind::NameConflict);
    assert_eq!(
        text,
        "cannot name an item of the bindings: the structure `Item` of the parameter `weights` of the kernel `tally` and the structure `Item` of the parameter `counts` of the kernel `tally` are different structures that both become the Rust name `Item`"
    );
}

#[test]
fn generate_resolves_a_relative_path_and_includes_each_file_by_an_absolute_path() {
    let relative = Path::new("tests/codegen/shaders/imports.slang");
    let absolute = shader("imports.slang");
    assert!(
        same_file(
            &std::env::current_dir()
                .expect("a working directory")
                .join(relative),
            &absolute
        ),
        "the test runs in the directory of the package"
    );
    let tokens = generate(relative)
        .expect("the bindings of the shader")
        .to_string();
    let includes = tokens
        .split("include_str ! (")
        .skip(1)
        .map(|rest| {
            let literal = rest.split(')').next().expect("a closing parenthesis");
            PathBuf::from(literal.trim().trim_matches('"'))
        })
        .collect::<Vec<_>>();
    assert_eq!(includes.len(), 4, "{tokens}");
    for include in &includes {
        assert!(include.is_absolute(), "{include:?}");
    }
    assert!(
        includes.iter().any(|include| same_file(include, &absolute)),
        "{includes:?}"
    );
    assert_eq!(
        tokens,
        generate(&absolute)
            .expect("the bindings of the shader")
            .to_string()
    );
}

#[test]
fn generate_reports_a_path_that_cannot_be_resolved_as_a_compile_error() {
    let error = generate("").expect_err("an empty path names no file");
    assert_eq!(error.kind(), ErrorKind::Compile);
    assert!(
        error
            .to_string()
            .contains("cannot resolve `` against the working directory"),
        "{error}"
    );
}

/// Returns the name, the offset, and the nesting of each field of a
/// structure, each nested field after the field that holds it.
fn layout(structure: &Structure) -> Vec<(String, u64)> {
    let mut fields = Vec::new();
    for field in &structure.fields {
        fields.push((field.name.clone(), field.offset));
        if let FieldType::Structure(inner) = &field.ty {
            fields.extend(
                layout(inner)
                    .into_iter()
                    .map(|(name, offset)| (format!("{}.{name}", field.name), offset)),
            );
        }
    }
    fields
}

#[test]
fn the_packed_layout_of_each_element_structure_is_the_scalar_layout_of_spirv() {
    let precompiled = precompile_file(&shader("nested.slang")).expect("the module compiles");
    let mut elements = Vec::new();
    for kernel in &precompiled.interface.kernels {
        for parameter in &kernel.parameters {
            if let ParameterKind::Buffer {
                element: ElementType::Structure(element),
                packed: Some(packed),
                ..
            } = &parameter.kind
            {
                elements.push((parameter.path.clone(), element.clone(), packed.clone()));
            }
        }
    }
    let bodies = elements
        .iter()
        .find(|(path, ..)| path == "bodies")
        .expect("the buffer of bodies");
    let motions = elements
        .iter()
        .find(|(path, ..)| path == "motions")
        .expect("the buffer of motions");
    let named = |fields: &[(&str, u64)]| -> Vec<(String, u64)> {
        fields
            .iter()
            .map(|(name, offset)| ((*name).to_owned(), *offset))
            .collect()
    };
    assert_eq!(bodies.1.size, 48);
    assert_eq!(
        layout(&bodies.1),
        named(&[
            ("position", 0),
            ("motion", 16),
            ("motion.velocity", 0),
            ("motion.drag", 8),
            ("id", 32)
        ])
    );
    assert_eq!(bodies.2.size, 28);
    assert_eq!(
        layout(&bodies.2),
        named(&[
            ("position", 0),
            ("motion", 12),
            ("motion.velocity", 0),
            ("motion.drag", 8),
            ("id", 24)
        ])
    );
    assert_eq!((motions.1.size, motions.2.size), (16, 12));
}

#[test]
fn the_modules_of_an_import_are_recorded_and_embedded_with_the_shader_last() {
    let precompiled = precompile_file(&shader("imports.slang")).expect("the module compiles");
    let names: Vec<&str> = precompiled
        .interface
        .modules
        .iter()
        .map(|module| module.name.as_str())
        .collect();
    assert_eq!(names, ["common", "imports"]);
    for module in &precompiled.interface.modules {
        assert!(module.path.is_absolute(), "{}", module.path.display());
        assert!(
            module
                .files
                .iter()
                .any(|file| same_file(file, &module.path)),
            "{module:?}"
        );
    }
    let text = bindings("imports.slang");
    let common = text
        .find("name: \"common\",\n                path: \"<shaders>/common.slang\",")
        .expect("the embedded module common");
    let imports = text
        .find("name: \"imports\",\n                path: \"<shaders>/imports.slang\",")
        .expect("the embedded shader");
    assert!(common < imports, "{text}");
}
