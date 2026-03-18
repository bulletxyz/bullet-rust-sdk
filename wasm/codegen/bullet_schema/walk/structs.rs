//! Discover named structs reachable from CallMessage fields.

use std::collections::{HashSet, VecDeque};

use sov_universal_wallet::schema::Link;
use sov_universal_wallet::ty::Ty;

use super::super::{FieldInfo, Types};
use super::field_info_from_link;

/// Raw struct discovered from the schema, before field mapping.
pub struct RawSchemaStruct {
    pub type_name: String,
    pub schema_index: usize,
    pub fields: Vec<FieldInfo>,
}

/// Walk the type graph from the given starting indices to find all named structs.
///
/// Skips `__SovVirtualWallet_*` internal wrappers and `SurrogateDecimal`
/// (handled as a string parse).
pub fn discover_structs(
    seed_indices: &[usize],
    types: &Types,
    visited: &mut HashSet<usize>,
    queue: &mut VecDeque<usize>,
) -> Vec<RawSchemaStruct> {
    for &idx in seed_indices {
        queue.push_back(idx);
    }

    let mut structs = Vec::new();

    while let Some(idx) = queue.pop_front() {
        if !visited.insert(idx) {
            continue;
        }

        match &types[idx] {
            Ty::Struct(s) => {
                if s.type_name.starts_with("__SovVirtualWallet_")
                    || s.type_name == "SurrogateDecimal"
                {
                    continue;
                }

                let fields: Vec<FieldInfo> = s
                    .fields
                    .iter()
                    .map(|f| field_info_from_link(&f.display_name, &f.value))
                    .collect();

                for field in &fields {
                    if let Some(child_idx) = field.schema_index {
                        queue.push_back(child_idx);
                    }
                }

                structs.push(RawSchemaStruct {
                    type_name: s.type_name.clone(),
                    schema_index: idx,
                    fields,
                });
            }
            Ty::Option { value } | Ty::Vec { value } => {
                if let Link::ByIndex(i) = value {
                    queue.push_back(*i);
                }
            }
            Ty::Tuple(t) => {
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

    structs
}
