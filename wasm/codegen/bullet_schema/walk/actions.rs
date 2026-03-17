//! Extract the five CallMessage action groups from the schema.

use sov_universal_wallet::schema::Link;
use sov_universal_wallet::ty::Ty;

use super::super::{ActionGroup, FieldInfo, Types, VariantInfo};
use super::field_info_from_link;

/// Find the `CallMessage` enum in the schema by name and extract all action groups.
pub fn extract_action_groups(types: &Types) -> Vec<ActionGroup> {
    let call_message_enum = types
        .iter()
        .find_map(|ty| match ty {
            Ty::Enum(e) if e.type_name == "CallMessage" => Some(e),
            _ => None,
        })
        .expect("CallMessage enum not found in schema");

    call_message_enum
        .variants
        .iter()
        .map(|variant| {
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

            // Unwrap the Tuple wrapper to get the action enum index.
            let action_enum_index = match &types[tuple_index] {
                Ty::Tuple(t) => {
                    assert_eq!(t.fields.len(), 1);
                    match &t.fields[0].value {
                        Link::ByIndex(i) => *i,
                        _ => panic!("Expected ByIndex in tuple wrapper"),
                    }
                }
                _ => panic!("Expected Tuple at index {tuple_index}"),
            };

            // Action enums: User, Keeper, Admin, etc.
            let action_enum = match &types[action_enum_index] {
                Ty::Enum(e) => e,
                _ => panic!("Expected Enum at index {action_enum_index}"),
            };

            let variants = action_enum
                .variants
                .iter()
                .map(|av| extract_variant(av, types))
                .collect();

            ActionGroup {
                call_message_variant: variant.name.clone(),
                action_enum: action_enum.type_name.clone(),
                variants,
            }
        })
        .collect()
}

fn extract_variant(
    enum_variant: &sov_universal_wallet::ty::EnumVariant<
        sov_universal_wallet::schema::IndexLinking,
    >,
    types: &Types,
) -> VariantInfo {
    let fields = match enum_variant.value.as_ref() {
        Some(Link::ByIndex(i)) => match &types[*i] {
            Ty::Struct(s) => s
                .fields
                .iter()
                .map(|f| field_info_from_link(&f.display_name, &f.value))
                .collect(),
            _ => panic!(
                "Expected Struct at index {i} for variant {}",
                enum_variant.name
            ),
        },
        Some(_) => panic!("Expected ByIndex for action variant {}", enum_variant.name),
        None => vec![],
    };

    VariantInfo {
        variant_name: enum_variant.name.clone(),
        fields,
    }
}
