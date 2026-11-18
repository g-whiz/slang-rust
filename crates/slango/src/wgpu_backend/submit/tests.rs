use super::group_entries;
use crate::call::{Entry, Plan, Source};
use crate::layout::{Binding, Resource, Usage};

fn sampler(set: u32, index: u32) -> Binding {
    Binding {
        path: format!("sampler_{set}_{index}"),
        set,
        index,
        usage: Usage::Used,
        resource: Resource::Sampler,
    }
}

#[test]
fn entries_are_grouped_by_set_with_an_empty_group_for_a_set_between_used_sets() {
    let bindings = [sampler(0, 0), sampler(0, 1), sampler(0, 2), sampler(2, 0)];
    let plan = Plan {
        entries: vec![
            Entry {
                binding: 0,
                source: Source::Uniform(vec![1, 2, 3, 4]),
            },
            Entry {
                binding: 1,
                source: Source::Argument(2),
            },
            Entry {
                binding: 3,
                source: Source::Argument(3),
            },
        ],
        immediates: Vec::new(),
    };
    assert_eq!(
        group_entries(&bindings, &plan),
        vec![
            vec![
                (0, &Source::Uniform(vec![1, 2, 3, 4])),
                (1, &Source::Argument(2)),
            ],
            vec![],
            vec![(0, &Source::Argument(3))],
        ]
    );
}

#[test]
fn a_plan_without_entries_has_no_groups() {
    let plan = Plan {
        entries: Vec::new(),
        immediates: Vec::new(),
    };
    assert_eq!(
        group_entries(&[sampler(0, 0)], &plan),
        Vec::<Vec<(u32, &Source)>>::new()
    );
}
