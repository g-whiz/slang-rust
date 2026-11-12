use slang_compiler::reflection::ScalarType;

use super::{
    Attribute, CompiledRender, KernelLayout, LOAD, Offsets, Position, Rejection, RenderLayout,
    RenderStage, Role, Scalar, attribute_type, render_record,
};
use crate::error::{Error, ErrorKind};

mod ffi;

fn offsets(set: u64, binding: u64, byte: u64, register_space: u64) -> Offsets {
    Offsets {
        set,
        binding,
        byte,
        register_space,
        location: 0,
    }
}

#[test]
fn descend_adds_every_offset_of_the_variable_to_the_running_sums() {
    let parent = Position {
        set: 1,
        binding: 2,
        byte: 16,
        register_space: 1,
        location: 0,
    };
    assert_eq!(
        parent.descend(offsets(0, 3, 8, 1)),
        Position {
            set: 1,
            binding: 5,
            byte: 24,
            register_space: 2,
            location: 0,
        }
    );
}

#[test]
fn a_parameter_block_takes_the_register_space_sum_as_its_set_and_restarts_bindings() {
    let outside = Position::default().descend(offsets(0, 3, 0, 0));
    let block = outside.descend(offsets(0, 0, 0, 1)).parameter_block();
    assert_eq!(block.set, 1);
    assert_eq!(block.binding, 0);
    let resource = block.block_element().descend(offsets(0, 1, 0, 0));
    assert_eq!((resource.set, resource.binding), (1, 1));
}

#[test]
fn a_nested_parameter_block_takes_the_set_after_its_outer_block() {
    let outer = Position::default()
        .descend(offsets(0, 0, 0, 1))
        .parameter_block();
    let element = outer.block_element().descend(offsets(0, 0, 0, 1));
    let inner = element.descend(offsets(0, 0, 0, 0)).parameter_block();
    assert_eq!(outer.set, 1);
    assert_eq!(inner.set, outer.set + 1);
}

#[test]
fn the_element_of_a_block_restarts_byte_offsets_and_keeps_its_bindings() {
    let block = Position {
        set: 2,
        binding: 1,
        byte: 48,
        register_space: 2,
        location: 3,
    };
    assert_eq!(block.block_element(), Position { byte: 0, ..block });
}

#[test]
fn the_location_of_a_field_is_the_sum_of_the_varying_offsets_along_its_path() {
    let parameter = Position::default().descend(Offsets {
        location: 1,
        ..Offsets::default()
    });
    let field = parameter.descend(Offsets {
        location: 2,
        ..Offsets::default()
    });
    assert_eq!((parameter.location, field.location), (1, 3));
}

#[test]
fn an_attribute_takes_a_32_bit_scalar_type_and_1_to_4_components() {
    assert_eq!(
        attribute_type(ScalarType::Float32, 3),
        Some((Scalar::Float32, 3))
    );
    assert_eq!(
        attribute_type(ScalarType::Int32, 1),
        Some((Scalar::Int32, 1))
    );
    assert_eq!(
        attribute_type(ScalarType::Uint32, 4),
        Some((Scalar::Uint32, 4))
    );
    for scalar in [
        ScalarType::Bool,
        ScalarType::Float16,
        ScalarType::Float64,
        ScalarType::Int64,
    ] {
        assert_eq!(attribute_type(scalar, 2), None, "{scalar:?}");
    }
    for components in [0, 5, u64::MAX] {
        assert_eq!(
            attribute_type(ScalarType::Float32, components),
            None,
            "{components}"
        );
    }
}

fn assert_send_sync<T: Send + Sync + 'static>() {}

#[test]
fn the_layout_snapshot_is_send_and_sync() {
    assert_send_sync::<KernelLayout>();
    assert_send_sync::<RenderLayout>();
    assert_send_sync::<Attribute>();
    assert_send_sync::<CompiledRender>();
}

fn empty_layout() -> RenderLayout {
    RenderLayout {
        bindings: Vec::new(),
        immediates: None,
        attributes: Vec::new(),
    }
}

fn unsupported() -> Error {
    Error::new(ErrorKind::UnsupportedParameter, LOAD)
        .with_detail("the parameter `image` is a texture".to_owned())
}

fn slang_failure() -> Error {
    Error::new(ErrorKind::Compile, "compile a Slang module")
        .with_source(std::io::Error::other("Slang reports error 30019"))
}

fn not_spirv() -> Error {
    Error::new(ErrorKind::Compile, LOAD).with_detail("not a sequence of SPIR-V words".to_owned())
}

fn rejection_of(record: &CompiledRender) -> &Rejection {
    record
        .usable
        .as_ref()
        .expect_err("the record holds a rejection")
}

#[test]
fn a_render_entry_point_with_a_layout_and_spirv_is_usable() {
    let record = render_record(
        "vsMain".to_owned(),
        Role::Vertex,
        Ok(empty_layout()),
        Ok(vec![0x0723_0203]),
    );
    assert_eq!(record.name, "vsMain");
    assert_eq!(record.stage, RenderStage::Vertex);
    let usable = record.usable.as_ref().expect("the record is usable");
    assert_eq!(usable.layout, empty_layout());
    assert_eq!(usable.spirv, [0x0723_0203]);
}

#[test]
fn a_refused_parameter_of_a_render_entry_point_becomes_its_rejection() {
    let record = render_record(
        "fsMain".to_owned(),
        Role::Fragment,
        Err(unsupported()),
        Ok(vec![0x0723_0203]),
    );
    assert_eq!(record.stage, RenderStage::Fragment);
    assert_eq!(
        rejection_of(&record),
        &Rejection {
            kind: ErrorKind::UnsupportedParameter,
            detail: "the parameter `image` is a texture".to_owned(),
        }
    );
}

#[test]
fn a_failure_of_slang_in_the_walk_of_a_render_entry_point_becomes_its_rejection() {
    let record = render_record(
        "fsMain".to_owned(),
        Role::Fragment,
        Err(slang_failure()),
        Ok(vec![0x0723_0203]),
    );
    let rejection = rejection_of(&record);
    assert_eq!(rejection.kind, ErrorKind::Compile);
    assert_eq!(
        rejection.detail,
        "Slang failed on the fragment entry point `fsMain`: Slang reports error 30019"
    );
}

#[test]
fn a_failure_to_generate_the_spirv_of_a_render_entry_point_becomes_its_rejection() {
    let cases = [
        (
            slang_failure(),
            "Slang failed on the vertex entry point `vsMain`: Slang reports error 30019",
        ),
        (not_spirv(), "not a sequence of SPIR-V words"),
    ];
    for (error, detail) in cases {
        let record = render_record(
            "vsMain".to_owned(),
            Role::Vertex,
            Ok(empty_layout()),
            Err(error),
        );
        assert_eq!(
            rejection_of(&record),
            &Rejection {
                kind: ErrorKind::Compile,
                detail: detail.to_owned(),
            }
        );
    }
}

#[test]
fn a_rejection_keeps_the_text_of_the_source_after_the_detail() {
    let error = Error::new(ErrorKind::Compile, LOAD)
        .with_detail("Slang reports no layout for the entry point".to_owned())
        .with_source(std::io::Error::other("Slang reports error 30019"));
    let record = render_record(
        "fsMain".to_owned(),
        Role::Fragment,
        Err(error),
        Ok(vec![0x0723_0203]),
    );
    assert_eq!(
        rejection_of(&record),
        &Rejection {
            kind: ErrorKind::Compile,
            detail: "Slang reports no layout for the entry point: Slang reports error 30019"
                .to_owned(),
        }
    );
}

#[test]
fn the_failure_of_the_walk_comes_before_the_failure_of_the_spirv() {
    let record = render_record(
        "vsMain".to_owned(),
        Role::Vertex,
        Err(unsupported()),
        Err(not_spirv()),
    );
    assert_eq!(rejection_of(&record).kind, ErrorKind::UnsupportedParameter);
}
