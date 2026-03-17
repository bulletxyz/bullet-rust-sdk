//! Discover simple enums (all unit variants) reachable from CallMessage fields.

use std::collections::HashSet;

use sov_universal_wallet::ty::Ty;

use super::super::{SchemaEnum, Types};

/// Scan already-visited indices for simple enums.
///
/// Called after struct discovery so `visited` contains all reachable indices.
/// Any `Ty::Enum` where every variant has `value: None` is a simple enum.
pub fn discover_enums(visited: &HashSet<usize>, types: &Types) -> Vec<SchemaEnum> {
    visited
        .iter()
        .filter_map(|&idx| {
            if let Ty::Enum(e) = &types[idx] {
                let all_unit = e.variants.iter().all(|v| v.value.is_none());
                if all_unit {
                    return Some(SchemaEnum {
                        type_name: e.type_name.clone(),
                        schema_index: idx,
                        variants: e.variants.iter().map(|v| v.name.clone()).collect(),
                    });
                }
            }
            None
        })
        .collect()
}
