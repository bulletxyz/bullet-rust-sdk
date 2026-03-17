//! Walk the Transaction schema to extract action groups, structs, and enums.

mod actions;
mod enums;
mod structs;

use std::collections::{HashSet, VecDeque};

use bullet_exchange_interface::schema::Schema;
use sov_universal_wallet::schema::Link;

use super::{FieldInfo, Primitive, SchemaInfo};

/// Walk the Transaction schema and extract everything needed for codegen.
pub fn extract_schema_info(schema: &Schema) -> SchemaInfo {
    let types = schema.types();

    let action_groups = actions::extract_action_groups(types);

    // Collect all field type indices as starting points for struct/enum discovery.
    let seeds: Vec<usize> = action_groups
        .iter()
        .flat_map(|g| &g.variants)
        .flat_map(|v| &v.fields)
        .filter_map(|f| f.schema_index)
        .collect();

    let mut visited = HashSet::new();
    let mut queue = VecDeque::new();
    let structs = structs::discover_structs(&seeds, types, &mut visited, &mut queue);
    let enums = enums::discover_enums(&visited, types);

    SchemaInfo {
        action_groups,
        structs,
        enums,
    }
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
