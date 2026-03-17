use std::collections::{HashSet, VecDeque};

use bullet_exchange_interface::schema::Schema;
use bullet_exchange_interface::transaction::Transaction;
use sov_universal_wallet::schema::Link;
use sov_universal_wallet::ty::Ty;

use super::{ActionGroup, FieldInfo, Primitive, SchemaEnum, SchemaInfo, SchemaStruct, VariantInfo};

/// Walk the Transaction schema and extract all CallMessage action groups,
/// plus all reachable named structs and simple enums.
pub fn extract_schema_info() -> SchemaInfo {
    let schema = Schema::of_single_type::<Transaction>().expect("failed to build schema");
    let types = schema.types();

    let action_groups = extract_action_groups(types);

    // Collect all schema indices reachable from action variant fields,
    // then walk those to find named structs and simple enums.
    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();

    for group in &action_groups {
        for variant in &group.variants {
            for field in &variant.fields {
                if let Some(idx) = field.schema_index {
                    queue.push_back(idx);
                }
            }
        }
    }

    let mut structs = Vec::new();
    let mut enums = Vec::new();

    while let Some(idx) = queue.pop_front() {
        if !visited.insert(idx) {
            continue;
        }

        match &types[idx] {
            Ty::Struct(s) => {
                // Skip internal wrapper structs.
                if s.type_name.starts_with("__SovVirtualWallet_") {
                    continue;
                }
                // Skip SurrogateDecimal — it's handled as a string.
                if s.type_name == "SurrogateDecimal" {
                    continue;
                }

                let fields: Vec<FieldInfo> = s
                    .fields
                    .iter()
                    .map(|f| field_info_from_link(&f.display_name, &f.value))
                    .collect();

                // Enqueue child types for transitive discovery.
                for field in &fields {
                    if let Some(child_idx) = field.schema_index {
                        queue.push_back(child_idx);
                    }
                }

                structs.push(SchemaStruct {
                    type_name: s.type_name.clone(),
                    schema_index: idx,
                    fields,
                });
            }
            Ty::Enum(e) => {
                let all_unit = e.variants.iter().all(|v| v.value.is_none());
                if all_unit {
                    enums.push(SchemaEnum {
                        type_name: e.type_name.clone(),
                        schema_index: idx,
                        variants: e.variants.iter().map(|v| v.name.clone()).collect(),
                    });
                }
                // Complex enums: don't create wrappers, they stay as JSON.
            }
            Ty::Option { value } => {
                // Walk into the Option's inner type.
                if let Link::ByIndex(i) = value {
                    queue.push_back(*i);
                }
            }
            Ty::Vec { value } => {
                // Walk into the Vec's inner type.
                if let Link::ByIndex(i) = value {
                    queue.push_back(*i);
                }
            }
            Ty::Tuple(t) => {
                // Walk into tuple fields.
                for f in &t.fields {
                    if let Link::ByIndex(i) = &f.value {
                        queue.push_back(*i);
                    }
                }
            }
            Ty::Map { key, value } => {
                if let Link::ByIndex(i) = key {
                    queue.push_back(*i);
                }
                if let Link::ByIndex(i) = value {
                    queue.push_back(*i);
                }
            }
            _ => {}
        }
    }

    SchemaInfo {
        action_groups,
        structs,
        enums,
    }
}

fn extract_action_groups(
    types: &[Ty<sov_universal_wallet::schema::IndexLinking>],
) -> Vec<ActionGroup> {
    // Type 5 = CallMessage enum (hardcoded from schema analysis, but we verify).
    let call_message_enum = match &types[5] {
        Ty::Enum(e) => {
            assert_eq!(
                e.type_name, "CallMessage",
                "Expected CallMessage at index 5"
            );
            e
        }
        _ => panic!("Expected Enum at index 5"),
    };

    let mut groups = Vec::new();

    for variant in &call_message_enum.variants {
        let tuple_index = match variant
            .value
            .as_ref()
            .expect("CallMessage variant must have value")
        {
            Link::ByIndex(i) => *i,
            _ => panic!(
                "Expected ByIndex link for CallMessage variant {}",
                variant.name
            ),
        };

        let action_enum_index = match &types[tuple_index] {
            Ty::Tuple(t) => {
                assert_eq!(t.fields.len(), 1, "Expected single-field tuple wrapper");
                match &t.fields[0].value {
                    Link::ByIndex(i) => *i,
                    _ => panic!("Expected ByIndex in tuple wrapper"),
                }
            }
            _ => panic!("Expected Tuple at index {tuple_index}"),
        };

        let action_enum = match &types[action_enum_index] {
            Ty::Enum(e) => e,
            _ => panic!("Expected Enum at index {action_enum_index}"),
        };

        let mut variants = Vec::new();

        for action_variant in &action_enum.variants {
            let struct_index = match action_variant.value.as_ref() {
                Some(Link::ByIndex(i)) => *i,
                Some(_) => panic!(
                    "Expected ByIndex for action variant {}",
                    action_variant.name
                ),
                None => {
                    variants.push(VariantInfo {
                        variant_name: action_variant.name.clone(),
                        fields: vec![],
                    });
                    continue;
                }
            };

            let fields = match &types[struct_index] {
                Ty::Struct(s) => s
                    .fields
                    .iter()
                    .map(|f| field_info_from_link(&f.display_name, &f.value))
                    .collect(),
                _ => panic!(
                    "Expected Struct at index {struct_index} for variant {}",
                    action_variant.name
                ),
            };

            variants.push(VariantInfo {
                variant_name: action_variant.name.clone(),
                fields,
            });
        }

        groups.push(ActionGroup {
            call_message_variant: variant.name.clone(),
            action_enum: action_enum.type_name.clone(),
            variants,
        });
    }

    groups
}

fn field_info_from_link(name: &str, link: &Link) -> FieldInfo {
    match link {
        Link::ByIndex(i) => FieldInfo {
            name: name.to_string(),
            schema_index: Some(*i),
            primitive: None,
        },
        Link::Immediate(prim) => FieldInfo {
            name: name.to_string(),
            schema_index: None,
            primitive: Some(convert_primitive(prim)),
        },
        _ => panic!("Unexpected link type for field {name}"),
    }
}

fn convert_primitive(prim: &sov_universal_wallet::schema::Primitive) -> Primitive {
    use sov_universal_wallet::schema::Primitive as P;
    use sov_universal_wallet::ty::IntegerType;
    match prim {
        P::Boolean => Primitive::Bool,
        P::String => Primitive::String,
        P::Integer(IntegerType::u8, _) => Primitive::U8,
        P::Integer(IntegerType::u16, _) => Primitive::U16,
        P::Integer(IntegerType::u32, _) => Primitive::U32,
        P::Integer(IntegerType::u64, _) => Primitive::U64,
        P::Integer(IntegerType::i16, _) => Primitive::I16,
        P::Integer(IntegerType::i64, _) => Primitive::I64,
        P::Integer(IntegerType::u128, _) => Primitive::U128,
        other => panic!("Unsupported immediate primitive: {other:?}"),
    }
}
