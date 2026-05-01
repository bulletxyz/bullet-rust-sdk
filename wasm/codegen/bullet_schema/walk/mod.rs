//! Walk the Transaction schema and resolve all types for codegen.
//!
//! Extracts action groups, structs, and enums from the schema,
//! then maps all fields to wasm-bindgen-compatible param types.

mod actions;
mod enums;
pub(crate) mod map;
mod structs;

use std::collections::{HashSet, VecDeque};

use bullet_exchange_interface::schema::Schema;
use bullet_exchange_interface::transaction::Transaction;
use sov_universal_wallet::schema::Link;

use super::{
    ActionGroup, FieldInfo, MappedField, Primitive, SchemaInfo, SchemaStruct, VariantInfo,
};

/// Walk the Transaction schema and produce fully resolved `SchemaInfo`.
pub fn extract_schema_info() -> SchemaInfo {
    let schema = Schema::of_single_type::<Transaction>().expect("failed to build schema");
    let types = schema.types();

    // Phase 1: Extract raw action groups and discover structs/enums.
    let raw_groups = actions::extract_action_groups(types);
    let seeds: Vec<usize> = raw_groups
        .iter()
        .flat_map(|g| &g.variants)
        .flat_map(|v| &v.fields)
        .filter_map(|f| f.schema_index)
        .collect();

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let raw_structs = structs::discover_structs(&seeds, types, &mut visited, &mut queue);
    let enums = enums::discover_enums(&visited, types);

    // Which structs and enums will have wasm-bindgen wrappers generated?
    // The map phase needs to know this so it can decide per-field whether to
    // accept a typed wrapper (e.g. `WasmNewOrderArgs`) or fall back to a
    // JSON string. We build these sets once and pass them through.
    let wrapper_indices = raw_structs.iter().map(|s| s.schema_index).collect();

    let enum_indices = enums.iter().map(|e| e.schema_index).collect();

    // Phase 3: Resolve all raw fields from the schema to fields mapped to the WASM bindgen types.
    let action_groups = raw_groups
        .into_iter()
        .map(|g| ActionGroup {
            call_message_variant: g.call_message_variant,
            action_enum: g.action_enum,
            variants: g
                .variants
                .into_iter()
                .map(|v| VariantInfo {
                    variant_name: v.variant_name,
                    fields: resolve_fields(&v.fields, types, &wrapper_indices, &enum_indices),
                })
                .collect(),
        })
        .collect();

    let structs = raw_structs
        .into_iter()
        .map(|s| SchemaStruct {
            type_name: s.type_name,
            schema_index: s.schema_index,
            fields: resolve_fields(&s.fields, types, &wrapper_indices, &enum_indices),
        })
        .collect();

    SchemaInfo {
        action_groups,
        structs,
        enums,
    }
}

fn resolve_fields(
    fields: &[FieldInfo],
    types: &super::Types,
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> Vec<MappedField> {
    let mappings = map::map_fields(fields, types, wrapper_indices, enum_indices);
    fields
        .iter()
        .zip(mappings)
        .map(|(f, m)| MappedField {
            name: f.name.clone(),
            param_type: m.param_type,
            conversion: m.conversion,
            is_optional: m.is_optional,
        })
        .collect()
}

// ── Shared helpers ───────────────────────────────────────────────────────────

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
        P::ByteVec { .. } => Primitive::ByteVec,
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
