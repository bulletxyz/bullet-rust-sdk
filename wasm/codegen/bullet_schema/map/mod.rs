//! Map schema types to wasm-bindgen-compatible param types and conversion expressions.

pub mod enums;
pub mod newtypes;
pub mod options;
pub mod primitives;
pub mod structs;
pub mod vecs;

use std::collections::HashSet;

use sov_universal_wallet::schema::Link;
use sov_universal_wallet::ty::Ty;

use super::{FieldInfo, SchemaEnum, SchemaStruct, Types};

/// The resolved parameter type and conversion expression for a field.
#[derive(Debug, Clone)]
pub struct ParamMapping {
    /// The Rust type for the wasm-bindgen function parameter.
    pub param_type: String,
    /// The expression to convert the parameter into the domain type.
    /// Uses `{v}` as a placeholder for the parameter variable name.
    pub conversion: String,
    /// Whether this parameter is optional (affects ordering — must be trailing).
    pub is_optional: bool,
}

/// Build a set of schema indices that have wasm-bindgen struct wrappers.
pub fn wrapped_struct_indices(schema_structs: &[SchemaStruct]) -> HashSet<usize> {
    schema_structs.iter().map(|s| s.schema_index).collect()
}

/// Build a set of schema indices that have wasm-bindgen enum wrappers.
pub fn wrapped_enum_indices(schema_enums: &[SchemaEnum]) -> HashSet<usize> {
    schema_enums.iter().map(|e| e.schema_index).collect()
}

/// Resolve all field mappings. Returns a Vec parallel to the input fields.
pub fn map_fields(
    fields: &[FieldInfo],
    types: &Types,
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> Vec<ParamMapping> {
    fields
        .iter()
        .map(|f| map_field(f, types, wrapper_indices, enum_indices))
        .collect()
}

/// Map a single field to its wasm param type and conversion expression.
fn map_field(
    field: &FieldInfo,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    if let Some(prim) = &field.primitive {
        return primitives::map_primitive(prim);
    }

    let idx = field
        .schema_index
        .expect("field must have schema_index or primitive");
    map_by_index(idx, types, wrapper_indices, enum_indices)
}

/// Map a schema type by its index.
pub fn map_by_index(
    idx: usize,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    // Try known newtypes first.
    if let Some(m) = newtypes::try_map_newtype(idx) {
        return m;
    }

    // Dynamic dispatch on the schema type.
    let ty = &types[idx];
    match ty {
        Ty::Option { value } => {
            let inner = map_link(value, types, wrapper_indices, enum_indices);
            options::map_option(inner)
        }

        Ty::Vec { value } => vecs::map_vec(value, types, wrapper_indices),

        Ty::Struct(s) => structs::map_struct(&s.type_name, idx, wrapper_indices),

        Ty::Enum(e) => {
            let all_unit = e.variants.iter().all(|v| v.value.is_none());
            enums::map_enum(&e.type_name, all_unit, idx, enum_indices)
        }

        Ty::Tuple(t) => {
            if t.fields.len() == 1 {
                map_link(&t.fields[0].value, types, wrapper_indices, enum_indices)
            } else {
                ParamMapping {
                    param_type: "&str".into(),
                    conversion: "from_json({v})?".into(),
                    is_optional: false,
                }
            }
        }

        Ty::Map { .. } | Ty::Array { .. } => ParamMapping {
            param_type: "&str".into(),
            conversion: "from_json({v})?".into(),
            is_optional: false,
        },

        _ => panic!("Unsupported schema type at index {idx}: {ty:?}"),
    }
}

/// Map a `Link` (either ByIndex or Immediate).
fn map_link(
    link: &Link,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    match link {
        Link::ByIndex(i) => map_by_index(*i, types, wrapper_indices, enum_indices),
        Link::Immediate(prim) => primitives::map_immediate(prim),
        _ => panic!("Unexpected link type"),
    }
}
