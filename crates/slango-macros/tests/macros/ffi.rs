use std::collections::HashMap;
use std::mem::offset_of;
use std::path::Path;

use slango::__private::{
    ElementType, FieldType, ParameterKind, Precompiled, StructElement, Structure, precompile_file,
};

use crate::structs::layouts::{Gap, Grid, Inner, Nested, Tail, Wide};

#[test]
fn the_macro_compiles_a_shader_and_reports_each_failure_at_the_path() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/macros/ui/saxpy.rs");
    cases.compile_fail("tests/macros/ui/slang_error.rs");
    cases.compile_fail("tests/macros/ui/unsupported_parameter.rs");
    cases.compile_fail("tests/macros/ui/unsupported_field.rs");
    cases.compile_fail("tests/macros/ui/name_conflict.rs");
    cases.compile_fail("tests/macros/ui/not_a_literal.rs");
    cases.compile_fail("tests/macros/ui/suffixed_literal.rs");
}

fn collect_structures<'a>(structure: &'a Structure, all: &mut HashMap<&'a str, &'a Structure>) {
    all.insert(&structure.name, structure);
    for field in &structure.fields {
        if let FieldType::Structure(inner) = &field.ty {
            collect_structures(inner, all);
        }
    }
}

/// Returns the structure that Slang reflects under `name` in a structured
/// buffer of the module, or `None` when the module has none.
fn reflected_structure<'a>(precompiled: &'a Precompiled, name: &str) -> Option<&'a Structure> {
    let mut all = HashMap::new();
    for kernel in &precompiled.interface.kernels {
        for parameter in &kernel.parameters {
            if let ParameterKind::Buffer {
                element: ElementType::Structure(structure),
                ..
            } = &parameter.kind
            {
                collect_structures(structure, &mut all);
            }
        }
    }
    all.get(name).copied()
}

/// Returns the size of `T`, which is also its stride, and the offsets of
/// its fields, and panics when the size of `T` differs from its stride.
fn rust_layout<T: StructElement>(offsets: &[(&str, usize)]) -> (usize, Vec<(String, usize)>) {
    assert_eq!(
        size_of::<T>(),
        T::STRIDE,
        "the size of the struct `{}` differs from its stride",
        T::SLANG_NAME
    );
    let fields = offsets
        .iter()
        .map(|(name, offset)| ((*name).to_owned(), *offset))
        .collect();
    (T::STRIDE, fields)
}

#[test]
fn each_generated_struct_has_the_size_and_offsets_that_slang_reflects() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/macros/shaders/layouts.slang");
    let precompiled = precompile_file(&path).expect("the module compiles");
    let reflected_layout = |name: &str| {
        let structure = reflected_structure(&precompiled, name)
            .expect("Slang reflects the structure in a structured buffer");
        let fits = |value| usize::try_from(value).expect("a layout value fits a usize");
        let fields: Vec<_> = structure
            .fields
            .iter()
            .map(|field| (field.name.clone(), fits(field.offset)))
            .collect();
        (fits(structure.size), fields)
    };
    assert_eq!(
        rust_layout::<Tail>(&[("direction", offset_of!(Tail, direction))]),
        reflected_layout(Tail::SLANG_NAME)
    );
    assert_eq!(
        rust_layout::<Gap>(&[
            ("weight", offset_of!(Gap, weight)),
            ("colour", offset_of!(Gap, colour)),
        ]),
        reflected_layout(Gap::SLANG_NAME)
    );
    assert_eq!(
        rust_layout::<Grid>(&[
            ("cells", offset_of!(Grid, cells)),
            ("mask", offset_of!(Grid, mask)),
        ]),
        reflected_layout(Grid::SLANG_NAME)
    );
    assert_eq!(
        rust_layout::<Inner>(&[
            ("offset", offset_of!(Inner, offset)),
            ("id", offset_of!(Inner, id)),
        ]),
        reflected_layout(Inner::SLANG_NAME)
    );
    assert_eq!(
        rust_layout::<Nested>(&[
            ("position", offset_of!(Nested, position)),
            ("inner", offset_of!(Nested, inner)),
            ("gap", offset_of!(Nested, gap)),
            ("weight", offset_of!(Nested, weight)),
        ]),
        reflected_layout(Nested::SLANG_NAME)
    );
    assert_eq!(
        rust_layout::<Wide>(&[
            ("a", offset_of!(Wide, a)),
            ("b", offset_of!(Wide, b)),
            ("c", offset_of!(Wide, c)),
            ("d", offset_of!(Wide, d)),
            ("e", offset_of!(Wide, e)),
            ("f", offset_of!(Wide, f)),
            ("g", offset_of!(Wide, g)),
            ("h", offset_of!(Wide, h)),
        ]),
        reflected_layout(Wide::SLANG_NAME)
    );
}
