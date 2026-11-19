use super::apply;
use crate::error::ErrorKind;
use crate::layout::{
    Attribute, CompiledRender, Rejection, RenderLayout, RenderStage, Scalar, UsableRender, Usage,
};

mod ffi;

fn attribute(path: &str, location: u32, usage: Usage) -> Attribute {
    Attribute {
        path: path.to_owned(),
        location,
        scalar: Scalar::Float32,
        components: 4,
        usage,
    }
}

fn record(name: &str, stage: RenderStage, attributes: Vec<Attribute>) -> CompiledRender {
    CompiledRender {
        name: name.to_owned(),
        stage,
        usable: Ok(UsableRender {
            layout: RenderLayout {
                bindings: Vec::new(),
                immediates: None,
                attributes,
            },
            spirv: Vec::new(),
        }),
    }
}

fn usages(record: &CompiledRender) -> Vec<(&str, Usage)> {
    let Ok(usable) = &record.usable else {
        return Vec::new();
    };
    usable
        .layout
        .attributes
        .iter()
        .map(|attribute| (attribute.path.as_str(), attribute.usage))
        .collect()
}

#[test]
fn the_usage_of_each_named_attribute_of_a_named_vertex_entry_point_is_replaced() {
    let mut records = [
        record(
            "vsMain",
            RenderStage::Vertex,
            vec![
                attribute("position", 0, Usage::Used),
                attribute("spare", 1, Usage::Used),
                attribute("unknown", 2, Usage::Used),
            ],
        ),
        record(
            "vsOther",
            RenderStage::Vertex,
            vec![attribute("spare", 0, Usage::Used)],
        ),
        record("vsMain", RenderStage::Fragment, Vec::new()),
    ];
    let usage = [(
        "vsMain".to_owned(),
        vec![
            ("position".to_owned(), Usage::Used),
            ("spare".to_owned(), Usage::Unused),
        ],
    )];
    apply(&mut records, &usage);
    assert_eq!(
        usages(&records[0]),
        [
            ("position", Usage::Used),
            ("spare", Usage::Unused),
            ("unknown", Usage::Used)
        ]
    );
    assert_eq!(usages(&records[1]), [("spare", Usage::Used)]);
    assert_eq!(usages(&records[2]), []);
}

#[test]
fn a_rejected_vertex_record_stays_rejected() {
    let rejection = || Rejection {
        kind: ErrorKind::UnsupportedParameter,
        detail: "refused".to_owned(),
        source: None,
    };
    let mut records = [CompiledRender {
        name: "vsMain".to_owned(),
        stage: RenderStage::Vertex,
        usable: Err(rejection()),
    }];
    apply(
        &mut records,
        &[(
            "vsMain".to_owned(),
            vec![("position".to_owned(), Usage::Unused)],
        )],
    );
    assert_eq!(records[0].usable, Err(rejection()));
}
