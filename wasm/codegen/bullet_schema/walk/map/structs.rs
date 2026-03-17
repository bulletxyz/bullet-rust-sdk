//! Mapping for named struct types.

use std::collections::HashSet;

use super::ParamMapping;

pub fn map_struct(type_name: &str, idx: usize, wrapper_indices: &HashSet<usize>) -> ParamMapping {
    assert!(
        !type_name.starts_with("__SovVirtualWallet_"),
        "Unexpected internal struct at index {idx}: {type_name}"
    );

    if wrapper_indices.contains(&idx) {
        // This struct has a generated WasmX wrapper — accept it directly
        // and extract .inner to get the domain type.
        let wrapper_name = format!("Wasm{type_name}");
        ParamMapping {
            param_type: wrapper_name,
            conversion: "{v}.inner".into(),
            is_optional: false,
        }
    } else {
        // No wrapper generated (shouldn't happen for reachable types,
        // but as a safety fallback) — accept a JSON string.
        ParamMapping {
            param_type: "&str".into(),
            conversion: "from_json({v})?".into(),
            is_optional: false,
        }
    }
}
