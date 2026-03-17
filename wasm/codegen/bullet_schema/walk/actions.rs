//! Extract the five CallMessage action groups from the schema.

use sov_universal_wallet::schema::Link;
use sov_universal_wallet::ty::Ty;

use super::super::{FieldInfo, Types};
use super::field_info_from_link;

/// Raw action group before field mapping.
pub struct RawActionGroup {
    pub call_message_variant: String,
    pub action_enum: String,
    pub variants: Vec<RawVariantInfo>,
}

pub struct RawVariantInfo {
    pub variant_name: String,
    pub fields: Vec<FieldInfo>,
}

/// Find the `CallMessage` enum in the schema by name and extract all action groups.
pub fn extract_action_groups(types: &Types) -> Vec<RawActionGroup> {
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

            RawActionGroup {
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
) -> RawVariantInfo {
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

    RawVariantInfo {
        variant_name: enum_variant.name.clone(),
        fields,
    }
}
