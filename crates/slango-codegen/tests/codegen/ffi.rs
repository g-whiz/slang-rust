use slango::__private::{Access, ElementType, ParameterKind, Scalar, precompile_file};
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
            "include_bytes ! ({:?})",
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
            }
    ));
    let text = bindings("imports.slang");
    assert!(
        text.contains("::core::include_bytes!(\"<shaders>/common.slang\");"),
        "{text}"
    );
    assert!(
        text.contains("::core::include_bytes!(\"<shaders>/imports.slang\");"),
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
fn a_buffer_of_a_structure_is_unsupported() {
    let (kind, text, _) = failure("particles.slang");
    assert_eq!(kind, ErrorKind::Unsupported);
    assert_eq!(
        text,
        "cannot generate the bindings: the parameter `particles` of the kernel `step` is not supported, because it is a structured buffer of the structure `Particle`, for which this version generates no struct"
    );
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
