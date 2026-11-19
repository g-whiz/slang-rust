use std::path::{Path, PathBuf};

use super::super::interface::describe;
use super::super::{
    Access, ElementType, KernelInterface, Parameter, ParameterKind, Precompiled, Scalar, Shape,
    Structure, ValueType, precompile_file,
};
use crate::encoding::decode;
use crate::error::ErrorKind;
use crate::layout;
use crate::module::SourceFile;

fn shaders() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/slango/shaders")
}

fn precompile(name: &str) -> Precompiled {
    precompile_file(&shaders().join(name)).expect("the shader precompiles")
}

fn file_names(paths: &[PathBuf]) -> Vec<&str> {
    let mut names: Vec<&str> = paths
        .iter()
        .filter_map(|path| path.file_name()?.to_str())
        .collect();
    names.sort_unstable();
    names
}

#[test]
fn every_test_shader_precompiles_to_the_snapshot_and_description_that_a_load_compiles() {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(shaders())
        .expect("the directory of the test shaders")
        .map(|entry| entry.expect("an entry of the directory").path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "slang")
        })
        .collect();
    paths.sort();
    assert!(paths.len() >= 5, "the test shaders {paths:?}");
    for path in paths {
        let file = SourceFile::read(&path).expect("the shader is read");
        let loaded = layout::compile(&file.source()).expect("the shader compiles");
        let packed =
            layout::packed_structures(&file.source()).expect("the packed structures are read");
        let precompiled = precompile_file(&path).expect("the shader precompiles");
        let decoded = decode(&precompiled.encoded).expect("the encoded snapshot decodes");
        assert_eq!(decoded, loaded.compiled, "{}", path.display());
        assert_eq!(
            precompiled.interface,
            describe(&loaded, &packed),
            "{}",
            path.display()
        );
    }
}

fn parameter(path: &str, kind: ParameterKind) -> Parameter {
    Parameter {
        path: path.to_owned(),
        kind,
    }
}

fn scalar(scalar: Scalar) -> ParameterKind {
    ParameterKind::Value(ValueType {
        scalar,
        shape: Shape::Scalar,
    })
}

#[test]
fn the_description_of_a_kernel_names_each_parameter_with_its_kind_and_type() {
    let interface = precompile("scale.slang").interface;
    assert_eq!(
        interface.kernels,
        [KernelInterface {
            name: "scale".to_owned(),
            thread_group_size: [64, 1, 1],
            parameters: vec![
                parameter("factor", scalar(Scalar::Float32)),
                parameter("count", scalar(Scalar::Uint32)),
                parameter(
                    "data",
                    ParameterKind::Buffer {
                        access: Access::ReadWrite,
                        element: ElementType::Plain {
                            scalar: Scalar::Float32,
                            components: 1,
                        },
                        packed: None,
                    },
                ),
            ],
            structures: Vec::new(),
        }]
    );
    assert_eq!(file_names(&interface.dependencies), ["scale.slang"]);
}

/// Returns the structure of the first buffer of structures of a test shader,
/// with the layout of the backend wgpu and the packed layout.
fn element_structures(name: &str) -> (Structure, Structure) {
    precompile(name)
        .interface
        .kernels
        .into_iter()
        .flat_map(|kernel| kernel.parameters)
        .find_map(|parameter| match parameter.kind {
            ParameterKind::Buffer {
                element: ElementType::Structure(structure),
                packed: Some(packed),
                ..
            } => Some((structure, packed)),
            ParameterKind::Value(_)
            | ParameterKind::Buffer { .. }
            | ParameterKind::Texture
            | ParameterKind::Sampler => None,
        })
        .expect("a buffer of structures")
}

fn fields(structure: &Structure) -> Vec<(&str, u64)> {
    structure
        .fields
        .iter()
        .map(|field| (field.name.as_str(), field.offset))
        .collect()
}

#[test]
fn the_description_of_a_buffer_of_structures_names_the_structure_and_its_fields() {
    let (element, _) = element_structures("particles.slang");
    assert_eq!((element.name.as_str(), element.size), ("Particle", 32));
    assert_eq!(
        fields(&element),
        [("position", 0), ("mass", 12), ("velocity", 16)]
    );
}

#[test]
fn the_packed_layout_of_a_buffer_of_structures_is_the_scalar_layout_of_spirv() {
    let (element, packed) = element_structures("particles.slang");
    assert_eq!((packed.name.as_str(), packed.size), ("Particle", 24));
    assert_eq!(fields(&packed), fields(&element));
}

#[test]
fn the_description_names_each_module_of_the_session_with_the_files_read_for_it() {
    let interface = precompile("two_kernels.slang").interface;
    let modules: Vec<(&str, Vec<&str>)> = interface
        .modules
        .iter()
        .map(|module| {
            assert!(module.path.is_absolute(), "{}", module.path.display());
            (module.name.as_str(), file_names(&module.files))
        })
        .collect();
    assert_eq!(
        modules,
        [
            ("helpers", vec!["helpers.slang"]),
            ("two_kernels", vec!["helpers.slang", "two_kernels.slang"]),
        ]
    );
}

#[test]
fn the_files_that_slang_read_include_every_imported_file() {
    let interface = precompile("two_kernels.slang").interface;
    assert_eq!(
        file_names(&interface.dependencies),
        ["helpers.slang", "two_kernels.slang"]
    );
    for path in &interface.dependencies {
        assert!(path.is_file(), "{} is a file", path.display());
    }
}

#[test]
fn a_missing_file_is_an_invalid_argument_and_broken_source_a_compile_error() {
    let missing = precompile_file(&shaders().join("missing.slang")).expect_err("no such file");
    assert_eq!(missing.kind(), ErrorKind::InvalidArgument);
    let broken = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/shaders/broken.slang");
    let error = precompile_file(&broken).expect_err("the source does not compile");
    assert_eq!(error.kind(), ErrorKind::Compile);
}
