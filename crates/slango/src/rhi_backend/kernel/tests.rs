use super::{check_addressable, segments, top_level_fields};
use crate::error::ErrorKind;
use crate::layout::{
    Access, Binding, ElementType, KernelLayout, Place, PlainValue, Resource, Scalar, Shape,
    UniformBlock, Usage,
};

fn block(paths: &[&str]) -> UniformBlock {
    UniformBlock {
        size: 16,
        values: paths
            .iter()
            .map(|path| PlainValue {
                path: (*path).to_owned(),
                offset: 0,
                size: 4,
                scalar: Scalar::Float32,
                shape: Shape::Scalar,
            })
            .collect(),
    }
}

#[test]
fn the_empty_path_of_the_global_values_has_no_names_and_a_dotted_path_each_name() {
    assert_eq!(segments(""), Vec::<&str>::new());
    assert_eq!(segments("data"), ["data"]);
    assert_eq!(segments("outer.inner.values"), ["outer", "inner", "values"]);
}

#[test]
fn each_top_level_field_of_the_global_values_is_written_once_in_the_order_of_its_values() {
    let globals = block(&["params.scale", "factor", "params.inner.x", "count"]);
    assert_eq!(
        top_level_fields("", &globals),
        Ok(vec!["params", "factor", "count"])
    );
}

#[test]
fn the_fields_of_a_block_are_named_after_its_path() {
    let constants = block(&[
        "outer.inner.tint",
        "outer.inner.rows.first",
        "outer.inner.gain",
    ]);
    assert_eq!(
        top_level_fields("outer.inner", &constants),
        Ok(vec!["tint", "rows", "gain"])
    );
}

#[test]
fn a_value_outside_the_path_of_its_block_is_reported_by_its_path() {
    let constants = block(&["outer.scale", "outerscale"]);
    assert_eq!(
        top_level_fields("outer", &constants),
        Err("outerscale".to_owned())
    );
}

fn binding(path: &str, usage: Usage, resource: Resource) -> Binding {
    Binding {
        path: path.to_owned(),
        set: 0,
        index: 0,
        usage,
        resource,
    }
}

fn storage() -> Resource {
    Resource::StorageBuffer {
        access: Access::ReadWrite,
        stride: 4,
        element: ElementType::Plain {
            scalar: Scalar::Float32,
            components: 1,
        },
    }
}

fn kernel(bindings: Vec<Binding>) -> KernelLayout {
    KernelLayout {
        name: "main".to_owned(),
        thread_group_size: [1, 1, 1],
        bindings,
        immediates: None,
        structures: Vec::new(),
    }
}

const GLOBAL: Place = Place {
    in_entry_point: false,
    in_structure: false,
};

const IN_STRUCTURE: Place = Place {
    in_entry_point: true,
    in_structure: true,
};

#[test]
fn buffers_and_blocks_of_structures_at_either_scope_are_addressable() {
    let layout = kernel(vec![
        binding("values", Usage::Used, storage()),
        binding("", Usage::Used, Resource::UniformBuffer(block(&["scale"]))),
        binding(
            "constants",
            Usage::Used,
            Resource::UniformBuffer(block(&["constants.scale"])),
        ),
    ]);
    let entry = Place {
        in_entry_point: true,
        in_structure: false,
    };
    assert!(check_addressable(&layout, &[GLOBAL, GLOBAL, entry]).is_ok());
}

#[test]
fn a_used_resource_inside_a_plain_structure_is_an_unsupported_parameter() {
    let layout = kernel(vec![binding("params.values", Usage::Used, storage())]);
    let error = check_addressable(&layout, &[IN_STRUCTURE]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter);
    assert_eq!(
        error.to_string(),
        "cannot load a Slang module: the parameter `params.values` of the kernel `main` is a resource inside a structure that is not the element of a constant buffer or a parameter block, which slango with the feature `rhi` does not support"
    );
}

#[test]
fn a_used_constant_buffer_of_a_plain_value_is_an_unsupported_parameter() {
    let layout = kernel(vec![binding(
        "tint",
        Usage::Used,
        Resource::UniformBuffer(block(&["tint"])),
    )]);
    let error = check_addressable(&layout, &[GLOBAL]).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::UnsupportedParameter);
    assert_eq!(
        error.to_string(),
        "cannot load a Slang module: the parameter `tint` of the kernel `main` is a constant buffer whose element is not a structure, which slango with the feature `rhi` does not support"
    );
}

#[test]
fn an_unused_parameter_that_cannot_be_addressed_is_accepted() {
    let layout = kernel(vec![
        binding("params.values", Usage::Unused, storage()),
        binding(
            "tint",
            Usage::Unused,
            Resource::UniformBuffer(block(&["tint"])),
        ),
    ]);
    assert!(check_addressable(&layout, &[IN_STRUCTURE, GLOBAL]).is_ok());
}
