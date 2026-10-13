//! Mapping for enum types.

use std::collections::HashSet;

use super::ParamMapping;

pub fn map_enum(
    type_name: &str,
    all_unit: bool,
    idx: usize,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    if type_name == "DegenAction" {
        // DegenAction has a handwritten wrapper; other data-carrying enums use JSON.
        ParamMapping {
            param_type: "WasmDegenAction".into(),
            conversion: "{v}.inner".into(),
            jsdoc_type: None,
            is_optional: false,
        }
    } else if all_unit && enum_indices.contains(&idx) {
        let wrapper_name = format!("Wasm{type_name}");
        ParamMapping {
            param_type: wrapper_name,
            conversion: "{v}.into_domain()".into(),
            jsdoc_type: None,
            is_optional: false,
        }
    } else if all_unit {
        // Not expected for reachable types: accept a string like "Bid", quoted for serde.
        ParamMapping {
            param_type: "&str".into(),
            conversion: r#"from_json(&format!("\"{}\"", {v}))?"#.into(),
            jsdoc_type: None,
            is_optional: false,
        }
    } else {
        ParamMapping {
            param_type: "&str".into(),
            conversion: "from_json({v})?".into(),
            jsdoc_type: None,
            is_optional: false,
        }
    }
}
