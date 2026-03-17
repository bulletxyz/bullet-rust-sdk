//! Mapping for enum types.

use std::collections::HashSet;

use super::ParamMapping;

/// Map an enum type. Simple enums with wrappers use the wrapper type.
/// Simple enums without wrappers fall back to string + from_json.
/// Complex enums always use JSON.
pub fn map_enum(
    type_name: &str,
    all_unit: bool,
    idx: usize,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    if all_unit && enum_indices.contains(&idx) {
        let wrapper_name = format!("Wasm{type_name}");
        ParamMapping {
            param_type: wrapper_name,
            conversion: "{v}.into_domain()".into(),
            is_optional: false,
        }
    } else if all_unit {
        ParamMapping {
            param_type: "&str".into(),
            conversion: r#"from_json(&format!("\"{}\"", {v}))?"#.into(),
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
