//! Mapping for enum types.

use std::collections::HashSet;

use super::ParamMapping;

pub fn map_enum(
    type_name: &str,
    all_unit: bool,
    idx: usize,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    if all_unit && enum_indices.contains(&idx) {
        // Simple enum with a generated WasmX wrapper — accept the wrapper
        // and call .into_domain() to convert to the Rust domain type.
        let wrapper_name = format!("Wasm{type_name}");
        ParamMapping {
            param_type: wrapper_name,
            conversion: "{v}.into_domain()".into(),
            is_optional: false,
        }
    } else if all_unit {
        // Simple enum but no wrapper (shouldn't happen for reachable types) —
        // accept a string like "Bid" and wrap it in quotes for serde.
        ParamMapping {
            param_type: "&str".into(),
            conversion: r#"from_json(&format!("\"{}\"", {v}))?"#.into(),
            is_optional: false,
        }
    } else {
        // Complex enum with data variants — accept a full JSON string.
        ParamMapping {
            param_type: "&str".into(),
            conversion: "from_json({v})?".into(),
            is_optional: false,
        }
    }
}
