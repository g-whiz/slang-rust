use std::collections::HashMap;

use crate::check::{Selection, Validator, unfixed_prim};
use crate::emit::{Slot, put};
use crate::error::{Error, ErrorKind, Result};
use crate::model::{CType, Enum, Item, ItemKind, Prim, Record, Ty, Underlying};
use crate::rules::RuleSet;

const SLOT_FUNCTION: &str = "slang_rhi_sys_abi_probe_slots";

const SLOT_HELPER: &str = r#"namespace {
template <typename Method>
std::size_t slang_rhi_sys_slot(Method method)
{
    struct Representation
    {
        std::uintptr_t pointer;
        std::ptrdiff_t adjustment;
    };
    static_assert(sizeof(Method) == sizeof(Representation), "a pointer to a member function has the size of two words");
    Representation representation;
    std::memcpy(&representation, &method, sizeof representation);
#if defined(__aarch64__) || defined(__arm__)
    if ((representation.adjustment & 1) == 0)
        return SIZE_MAX;
    return representation.pointer;
#else
    if ((representation.pointer & 1) == 0)
        return SIZE_MAX;
    return representation.pointer - 1;
#endif
}
}

"#;

pub(crate) fn probe(
    selection: &Selection,
    rules: &RuleSet,
    slots: Option<&[Slot]>,
) -> Result<String> {
    let records: HashMap<&str, &Record> = selection
        .items
        .iter()
        .filter_map(|item| match &item.kind {
            ItemKind::Record(record) => Some((item.name.cxx.as_str(), record)),
            _ => None,
        })
        .collect();
    let mut text = format!(
        "#include \"{}\"\n#include <cstddef>\n#include <cstdint>\n#include <cstring>\n#include <type_traits>\n\n",
        rules.main_header
    );
    let validator = Validator::new(selection, rules, "", None);
    for item in selection.items.iter().filter(|item| is_nameable(item)) {
        match &item.kind {
            ItemKind::Record(record) => {
                assert_record(&mut text, &records, &item.name.cxx, record, &item.name.cxx)?;
            }
            ItemKind::Enum(enumeration) if enumeration.constants_of.is_none() => {
                assert_enum(&mut text, &validator, &item.name.cxx, enumeration);
            }
            _ => {}
        }
    }
    if let Some(slots) = slots {
        text.push('\n');
        text.push_str(SLOT_HELPER);
        put(
            &mut text,
            format_args!(
                "extern \"C\" std::size_t {SLOT_FUNCTION}(std::size_t* out, std::size_t capacity)\n{{\n"
            ),
        );
        if slots.is_empty() {
            text.push_str("    (void)out;\n    (void)capacity;\n    return 0;\n}\n");
        } else {
            text.push_str("    const std::size_t slots[] = {\n");
            for slot in slots {
                let canonical = &slot.canonical;
                put(
                    &mut text,
                    format_args!(
                        "        slang_rhi_sys_slot(static_cast<{} ({}::*)({}){}>(&{}::{})),\n",
                        canonical.result,
                        slot.interface_cxx,
                        canonical.params.join(", "),
                        if canonical.is_const { " const" } else { "" },
                        slot.interface_cxx,
                        slot.method
                    ),
                );
            }
            text.push_str(
                "    };\n    const std::size_t count = sizeof(slots) / sizeof(slots[0]);\n    for (std::size_t index = 0; index < count && index < capacity; ++index)\n        out[index] = slots[index];\n    return count;\n}\n",
            );
        }
    }
    Ok(text)
}

fn assert_enum(text: &mut String, validator: &Validator<'_>, name: &str, enumeration: &Enum) {
    let (integer, fixed) = match &enumeration.underlying {
        Underlying::Fixed(ty) => (integer(validator.resolve(ty)), true),
        Underlying::Unfixed { .. } => (integer(&Ty::Prim(unfixed_prim(enumeration))), false),
    };
    let Some((size, signed)) = integer else {
        return;
    };
    put(
        text,
        format_args!(
            "static_assert(sizeof({name}) == {size}, \"the size of the enumeration {name}\");\n"
        ),
    );
    if !fixed {
        return;
    }
    put(
        text,
        format_args!(
            "static_assert(std::is_signed_v<std::underlying_type_t<{name}>> == {signed}, \"the signedness of the enumeration {name}\");\n"
        ),
    );
}

fn integer(ty: &Ty) -> Option<(usize, bool)> {
    match ty {
        Ty::Bool => Some((1, false)),
        Ty::Prim(prim) => Some(match prim {
            Prim::I8 => (1, true),
            Prim::U8 => (1, false),
            Prim::I16 => (2, true),
            Prim::U16 => (2, false),
            Prim::I32 => (4, true),
            Prim::U32 => (4, false),
            Prim::I64 | Prim::Isize => (8, true),
            Prim::U64 | Prim::Usize => (8, false),
        }),
        Ty::C(c) => match c {
            CType::SChar => Some((1, true)),
            CType::UChar => Some((1, false)),
            CType::Short => Some((2, true)),
            CType::UShort => Some((2, false)),
            CType::Int => Some((4, true)),
            CType::UInt => Some((4, false)),
            CType::LongLong => Some((8, true)),
            CType::ULongLong => Some((8, false)),
            CType::Char | CType::Long | CType::ULong | CType::Float | CType::Double => None,
        },
        _ => None,
    }
}

fn is_nameable(item: &Item) -> bool {
    !item.name.cxx.contains('(')
}

fn assert_record(
    text: &mut String,
    records: &HashMap<&str, &Record>,
    expression: &str,
    record: &Record,
    what: &str,
) -> Result<()> {
    put(
        text,
        format_args!(
            "static_assert(sizeof({expression}) == {}, \"the size of {what}\");\n",
            record.size
        ),
    );
    put(
        text,
        format_args!(
            "static_assert(alignof({expression}) == {}, \"the alignment of {what}\");\n",
            record.align
        ),
    );
    assert_fields(text, records, expression, record, 0, what)
}

fn assert_fields(
    text: &mut String,
    records: &HashMap<&str, &Record>,
    expression: &str,
    record: &Record,
    base: usize,
    what: &str,
) -> Result<()> {
    for field in &record.fields {
        let offset = base + field.offset_bits / 8;
        let anonymous = match &field.ty {
            Ty::Named(name) if name.contains('(') => Some(*records.get(name.as_str()).ok_or_else(
                || {
                    Error::at(
                        ErrorKind::Unsupported,
                        &field.location,
                        format!(
                            "internal error in the binding generator: the anonymous record {name} is not included"
                        ),
                    )
                },
            )?),
            _ => None,
        };
        match (&field.name, anonymous) {
            (Some(name), inner) => {
                put(
                    text,
                    format_args!(
                        "static_assert(offsetof({expression}, {name}) == {offset}, \"the offset of {what}::{name}\");\n"
                    ),
                );
                if let Some(inner) = inner {
                    let inner_expression = format!("decltype({expression}::{name})");
                    let inner_what = format!("{what}::{name}");
                    assert_record(text, records, &inner_expression, inner, &inner_what)?;
                }
            }
            (None, Some(inner)) => {
                assert_fields(text, records, expression, inner, offset, what)?;
            }
            (None, None) => {
                return Err(Error::at(
                    ErrorKind::Unsupported,
                    &field.location,
                    format!(
                        "internal error in the binding generator: a field of {what} has no name and no anonymous record type"
                    ),
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::Location;
    use crate::model::{CanonicalSignature, Enumerator, Field, Init, Name};
    use crate::rules::SLANG_RHI;

    fn field(name: Option<&str>, ty: Ty, offset: usize) -> Field {
        Field {
            name: name.map(str::to_owned),
            location: Location::new("slang-rhi.h", 1),
            ty,
            offset_bits: offset * 8,
            bit_width: None,
            init: Init::None,
            spelling: None,
        }
    }

    fn record(cxx: &str, owner: Option<&str>, size: usize, fields: Vec<Field>) -> Item {
        Item {
            name: Name {
                cxx: cxx.to_owned(),
                rust: None,
            },
            location: Location::new("slang-rhi.h", 1),
            owner: owner.map(str::to_owned),
            kind: ItemKind::Record(Record {
                union: false,
                size,
                align: 8,
                fields,
                members: Vec::new(),
                has_bases: false,
            }),
        }
    }

    fn selection() -> Selection {
        let inner = "rhi::Binding::(anonymous 0)";
        let named = "rhi::Binding::(anonymous 1)";
        let kind = Item {
            name: Name {
                cxx: "rhi::Kind".to_owned(),
                rust: Some("Kind".to_owned()),
            },
            location: Location::new("slang-rhi.h", 1),
            owner: None,
            kind: ItemKind::Enum(Enum {
                unnamed: false,
                constants_of: None,
                underlying: Underlying::Fixed(Ty::Prim(Prim::U8)),
                size: 1,
                enumerators: vec![Enumerator {
                    name: "First".to_owned(),
                    value: 0,
                }],
            }),
        };
        Selection {
            items: vec![
                kind,
                record(
                    inner,
                    Some("rhi::Binding"),
                    8,
                    vec![field(Some("offset"), Ty::Prim(Prim::U64), 0)],
                ),
                record(
                    named,
                    Some("rhi::Binding"),
                    8,
                    vec![field(Some("count"), Ty::Prim(Prim::U32), 4)],
                ),
                record(
                    "rhi::Binding",
                    None,
                    24,
                    vec![
                        field(Some("type"), Ty::Prim(Prim::I32), 0),
                        field(None, Ty::Named(inner.to_owned()), 8),
                        field(Some("extra"), Ty::Named(named.to_owned()), 16),
                    ],
                ),
            ],
            excluded: Vec::new(),
            external: Vec::new(),
            macros: Vec::new(),
            without_default: std::collections::BTreeMap::new(),
            microsoft_abi: false,
        }
    }

    #[test]
    fn probe_asserts_records_anonymous_members_and_enumerations() {
        let text = probe(&selection(), &SLANG_RHI, None).expect("the probe is written");
        let expected = [
            "#include \"slang-rhi.h\"",
            "static_assert(sizeof(rhi::Kind) == 1, \"the size of the enumeration rhi::Kind\");",
            "static_assert(std::is_signed_v<std::underlying_type_t<rhi::Kind>> == false, \"the signedness of the enumeration rhi::Kind\");",
            "static_assert(sizeof(rhi::Binding) == 24, \"the size of rhi::Binding\");",
            "static_assert(alignof(rhi::Binding) == 8, \"the alignment of rhi::Binding\");",
            "static_assert(offsetof(rhi::Binding, type) == 0, \"the offset of rhi::Binding::type\");",
            "static_assert(offsetof(rhi::Binding, offset) == 8, \"the offset of rhi::Binding::offset\");",
            "static_assert(offsetof(rhi::Binding, extra) == 16, \"the offset of rhi::Binding::extra\");",
            "static_assert(sizeof(decltype(rhi::Binding::extra)) == 8, \"the size of rhi::Binding::extra\");",
            "static_assert(offsetof(decltype(rhi::Binding::extra), count) == 4, \"the offset of rhi::Binding::extra::count\");",
        ];
        for line in expected {
            assert!(text.contains(line), "{line}\n{text}");
        }
        assert!(!text.contains("(anonymous"), "{text}");
        assert!(!text.contains(SLOT_FUNCTION), "{text}");
    }

    #[test]
    fn probe_reads_each_slot_through_a_cast_to_the_exact_member_function_type() {
        let slots = [Slot {
            interface_cxx: "rhi::IDevice".to_owned(),
            interface_rust: "IDevice".to_owned(),
            method: "readBuffer".to_owned(),
            field: "readBuffer_blob".to_owned(),
            canonical: CanonicalSignature {
                result: "int".to_owned(),
                params: vec!["rhi::IBuffer *".to_owned(), "ISlangBlob **".to_owned()],
                is_const: true,
            },
        }];
        let text = probe(&selection(), &SLANG_RHI, Some(&slots)).expect("the probe is written");
        assert!(text.contains("slang_rhi_sys_slot(static_cast<int (rhi::IDevice::*)(rhi::IBuffer *, ISlangBlob **) const>(&rhi::IDevice::readBuffer)),"), "{text}");
        assert!(text.contains("extern \"C\" std::size_t slang_rhi_sys_abi_probe_slots(std::size_t* out, std::size_t capacity)"), "{text}");
        let empty = probe(&selection(), &SLANG_RHI, Some(&[])).expect("the probe is written");
        assert!(empty.contains("    return 0;\n}\n"), "{empty}");
    }

    #[test]
    fn integer_gives_the_size_and_signedness_of_fixed_width_types_only() {
        assert_eq!(integer(&Ty::Prim(Prim::I16)), Some((2, true)));
        assert_eq!(integer(&Ty::Prim(Prim::Usize)), Some((8, false)));
        assert_eq!(integer(&Ty::C(CType::UInt)), Some((4, false)));
        assert_eq!(integer(&Ty::C(CType::Long)), None);
        assert_eq!(integer(&Ty::C(CType::Char)), None);
        assert_eq!(integer(&Ty::Void), None);
    }
}
