//! Mapping for named struct types.

use std::collections::HashSet;

use super::ParamMapping;

/// Map a named struct type. If it has a wasm wrapper, accept the wrapper.
/// Otherwise fall back to JSON string.
pub fn map_struct(type_name: &str, idx: usize, wrapper_indices: &HashSet<usize>) -> ParamMapping {
    assert!(
        !type_name.starts_with("__SovVirtualWallet_"),
        "Unexpected internal struct at index {idx}: {type_name}"
    );

    if wrapper_indices.contains(&idx) {
        let wrapper_name = format!("Wasm{type_name}");
        ParamMapping {
            param_type: wrapper_name,
            conversion: "{v}.inner".into(),
            is_optional: false,
        }
    } else {
        ParamMapping {
            param_type: "&str".into(),
            conversion: "from_json({v})?".into(),
            is_optional: false,
        }
    }
}
