use std::collections::HashSet;

use bullet_exchange_interface::schema::Schema;
use bullet_exchange_interface::transaction::Transaction;
use sov_universal_wallet::schema::Link;
use sov_universal_wallet::ty::Ty;

use super::{FieldInfo, Primitive, SchemaEnum, SchemaStruct};

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
pub fn wrapped_struct_indices(structs: &[SchemaStruct]) -> HashSet<usize> {
    structs.iter().map(|s| s.schema_index).collect()
}

/// Build a set of schema indices that have wasm-bindgen enum wrappers.
pub fn wrapped_enum_indices(enums: &[SchemaEnum]) -> HashSet<usize> {
    enums.iter().map(|e| e.schema_index).collect()
}

/// Resolve all field mappings using the schema. Returns a Vec parallel to the input fields.
pub fn map_fields(
    fields: &[FieldInfo],
    types: &[Ty<sov_universal_wallet::schema::IndexLinking>],
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> Vec<ParamMapping> {
    fields
        .iter()
        .map(|f| map_single_field(f, types, wrapper_indices, enum_indices))
        .collect()
}

fn map_single_field(
    field: &FieldInfo,
    types: &[Ty<sov_universal_wallet::schema::IndexLinking>],
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    if let Some(prim) = &field.primitive {
        return map_primitive(prim);
    }

    let idx = field
        .schema_index
        .expect("field must have schema_index or primitive");
    map_by_index(idx, types, wrapper_indices, enum_indices)
}

fn map_primitive(prim: &Primitive) -> ParamMapping {
    match prim {
        Primitive::Bool => ParamMapping {
            param_type: "bool".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        Primitive::U8 => ParamMapping {
            param_type: "u8".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        Primitive::U16 => ParamMapping {
            param_type: "u16".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        Primitive::U32 => ParamMapping {
            param_type: "u32".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        Primitive::U64 => ParamMapping {
            param_type: "u64".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        Primitive::I16 => ParamMapping {
            param_type: "i16".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        Primitive::I64 => ParamMapping {
            param_type: "i64".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        Primitive::U128 => ParamMapping {
            param_type: "u128".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        Primitive::String => ParamMapping {
            param_type: "&str".into(),
            conversion: "{v}.into()".into(),
            is_optional: false,
        },
    }
}

fn map_by_index(
    idx: usize,
    types: &[Ty<sov_universal_wallet::schema::IndexLinking>],
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    // Known newtype indices — identified by position in the schema, not structure.
    match idx {
        // AssetId(u16)
        9 => ParamMapping {
            param_type: "u16".into(),
            conversion: "AssetId({v})".into(),
            is_optional: false,
        },
        // PositiveDecimal(SurrogateDecimal)
        10 => ParamMapping {
            param_type: "&str".into(),
            conversion: "parse_dec({v})?".into(),
            is_optional: false,
        },
        // SurrogateDecimal — needs separate parser since it's not PositiveDecimal
        11 => ParamMapping {
            param_type: "&str".into(),
            conversion: "parse_surrogate_dec({v})?".into(),
            is_optional: false,
        },
        // Address(ByteArray 32 Base58)
        22 => ParamMapping {
            param_type: "&str".into(),
            conversion: "parse_addr({v})?".into(),
            is_optional: false,
        },
        // MarketId(u16)
        25 => ParamMapping {
            param_type: "u16".into(),
            conversion: "MarketId({v})".into(),
            is_optional: false,
        },
        // ClientOrderId(u64)
        33 => ParamMapping {
            param_type: "u64".into(),
            conversion: "ClientOrderId({v})".into(),
            is_optional: false,
        },
        // OrderId(u64)
        45 => ParamMapping {
            param_type: "u64".into(),
            conversion: "OrderId({v})".into(),
            is_optional: false,
        },
        // TriggerOrderId(u64)
        57 => ParamMapping {
            param_type: "u64".into(),
            conversion: "TriggerOrderId({v})".into(),
            is_optional: false,
        },
        // TwapId(u64)
        61 => ParamMapping {
            param_type: "u64".into(),
            conversion: "TwapId({v})".into(),
            is_optional: false,
        },
        // UnixTimestampMicros(i64)
        94 => ParamMapping {
            param_type: "i64".into(),
            conversion: "UnixTimestampMicros::from_micros({v})".into(),
            is_optional: false,
        },
        // TokenId(CustomString) — use FromStr
        147 => ParamMapping {
            param_type: "&str".into(),
            conversion: "TokenId::from_str({v}).unwrap()".into(),
            is_optional: false,
        },
        // Everything else: inspect the schema type.
        _ => map_dynamic(idx, types, wrapper_indices, enum_indices),
    }
}

fn map_dynamic(
    idx: usize,
    types: &[Ty<sov_universal_wallet::schema::IndexLinking>],
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    let ty = &types[idx];
    match ty {
        // Option { value } — recurse on inner, mark as optional.
        Ty::Option { value } => {
            let inner = map_link(value, types, wrapper_indices, enum_indices);
            let param_type = if inner.param_type == "&str" {
                "Option<String>".into()
            } else {
                format!("Option<{}>", inner.param_type)
            };
            let conversion = if inner.param_type == "&str" {
                if inner.conversion == "{v}.into()" {
                    "{v}.as_deref().map(|s| s.into())".into()
                } else if inner.conversion.contains("parse_") {
                    let fn_name = if inner.conversion.contains("parse_surrogate_dec") {
                        "parse_surrogate_dec"
                    } else if inner.conversion.contains("parse_dec") {
                        "parse_dec"
                    } else {
                        "parse_addr"
                    };
                    format!("{{v}}.as_deref().map({fn_name}).transpose()?")
                } else if inner.conversion.contains("from_json") {
                    format!("{{v}}.as_deref().map(from_json).transpose()?")
                } else {
                    let inner_expr = inner.conversion.replace("{v}", "s");
                    format!("{{v}}.as_deref().map(|s| {inner_expr})")
                }
            } else if inner.conversion.ends_with(".inner") {
                // Option<WasmStruct> -> .map(|w| w.inner)
                format!("{{v}}.map(|w| w.inner)")
            } else if inner.conversion == "{v}" {
                "{v}".into()
            } else if inner.param_type == "js_sys::Array" {
                // Option<js_sys::Array> — the inner conversion has complex ? usage.
                // Wrap in a closure returning Result for .map().transpose().
                let inner_expr = inner.conversion.replace("{v}", "v");
                format!("{{v}}.map(|v| -> Result<_, String> {{ Ok({inner_expr}) }}).transpose()?")
            } else {
                let inner_expr = inner.conversion.replace("{v}", "v");
                if inner_expr.contains('?') {
                    let expr_no_q = inner_expr.trim_end_matches('?');
                    format!("{{v}}.map(|v| {expr_no_q}).transpose()?")
                } else {
                    format!("{{v}}.map(|v| {inner_expr})")
                }
            };
            ParamMapping {
                param_type,
                conversion,
                is_optional: true,
            }
        }

        // Vec { value } — depends on what's inside.
        Ty::Vec { value } => map_vec(value, types, wrapper_indices, enum_indices),

        // Named struct — if it has a wrapper, accept the wrapper type.
        Ty::Struct(s) => {
            if s.type_name.starts_with("__SovVirtualWallet_") {
                panic!(
                    "Unexpected __SovVirtualWallet_ struct at index {idx}: {}",
                    s.type_name
                );
            }
            if wrapper_indices.contains(&idx) {
                let wrapper_name = format!("Wasm{}", s.type_name);
                ParamMapping {
                    param_type: wrapper_name,
                    conversion: "{v}.inner".into(),
                    is_optional: false,
                }
            } else {
                // Fallback to JSON for structs without wrappers.
                ParamMapping {
                    param_type: "&str".into(),
                    conversion: "from_json({v})?".into(),
                    is_optional: false,
                }
            }
        }

        // Enum types.
        Ty::Enum(e) => {
            let all_unit = e.variants.iter().all(|v| v.value.is_none());
            if all_unit && enum_indices.contains(&idx) {
                // Simple enum with a wasm-bindgen wrapper — accept the wrapper type.
                let wrapper_name = format!("Wasm{}", e.type_name);
                ParamMapping {
                    param_type: wrapper_name,
                    conversion: "{v}.into_domain()".into(),
                    is_optional: false,
                }
            } else if all_unit {
                // Simple enum without a wrapper (shouldn't happen) — fall back to string.
                ParamMapping {
                    param_type: "&str".into(),
                    conversion: r#"from_json(&format!("\"{}\"", {v}))?"#.into(),
                    is_optional: false,
                }
            } else {
                // Complex enum — pass as JSON.
                ParamMapping {
                    param_type: "&str".into(),
                    conversion: "from_json({v})?".into(),
                    is_optional: false,
                }
            }
        }

        // Tuple — could be a newtype we didn't catch, or a multi-field tuple.
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

        // Map — pass as JSON.
        Ty::Map { .. } => ParamMapping {
            param_type: "&str".into(),
            conversion: "from_json({v})?".into(),
            is_optional: false,
        },

        // Array — pass as JSON.
        Ty::Array { .. } => ParamMapping {
            param_type: "&str".into(),
            conversion: "from_json({v})?".into(),
            is_optional: false,
        },

        _ => panic!("Unsupported schema type at index {idx}: {ty:?}"),
    }
}

fn map_vec(
    value_link: &Link,
    types: &[Ty<sov_universal_wallet::schema::IndexLinking>],
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    match value_link {
        Link::ByIndex(inner_idx) => {
            match *inner_idx {
                // Vec<AssetId> -> Vec<u16>
                9 => ParamMapping {
                    param_type: "Vec<u16>".into(),
                    conversion: "{v}.into_iter().map(AssetId).collect()".into(),
                    is_optional: false,
                },
                // Vec<MarketId> -> Vec<u16>
                25 => ParamMapping {
                    param_type: "Vec<u16>".into(),
                    conversion: "{v}.into_iter().map(MarketId).collect()".into(),
                    is_optional: false,
                },
                // Vec<Address> -> Vec<String>
                22 => ParamMapping {
                    param_type: "Vec<String>".into(),
                    conversion: "{v}.iter().map(|s| parse_addr(s)).collect::<Result<Vec<_>, _>>()?"
                        .into(),
                    is_optional: false,
                },
                // Vec<OrderId> -> Vec<u64>
                45 => ParamMapping {
                    param_type: "Vec<u64>".into(),
                    conversion: "{v}.into_iter().map(OrderId).collect()".into(),
                    is_optional: false,
                },
                // Vec<TriggerOrderId> -> Vec<u64>
                57 => ParamMapping {
                    param_type: "Vec<u64>".into(),
                    conversion: "{v}.into_iter().map(TriggerOrderId).collect()".into(),
                    is_optional: false,
                },
                // Vec<ClientOrderId> -> Vec<u64>
                33 => ParamMapping {
                    param_type: "Vec<u64>".into(),
                    conversion: "{v}.into_iter().map(ClientOrderId).collect()".into(),
                    is_optional: false,
                },
                // Vec<TwapId> -> Vec<u64>
                61 => ParamMapping {
                    param_type: "Vec<u64>".into(),
                    conversion: "{v}.into_iter().map(TwapId).collect()".into(),
                    is_optional: false,
                },
                _ => {
                    let inner_ty = &types[*inner_idx];
                    match inner_ty {
                        // Vec<SomeStruct> — accept js_sys::Array of wrapper objects.
                        // JS passes [new Struct(...), new Struct(...)].
                        // Each element is recovered via TryFromJsValue.
                        Ty::Struct(s) if !s.type_name.starts_with("__SovVirtualWallet_") => {
                            if wrapper_indices.contains(inner_idx) {
                                let wrapper_name = format!("Wasm{}", s.type_name);
                                let conversion = format!(
                                    "extract_array::<{wrapper_name}>({{v}})?.into_iter().map(|w| w.inner).collect()"
                                );
                                ParamMapping {
                                    param_type: "js_sys::Array".into(),
                                    conversion,
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
                        // Vec<(multi-field tuple)> -> admin cancel ops
                        Ty::Tuple(t) if t.fields.len() > 1 => {
                            if t.fields.len() == 3 {
                                map_admin_cancel_vec(t)
                            } else {
                                ParamMapping {
                                    param_type: "&str".into(),
                                    conversion: "from_json({v})?".into(),
                                    is_optional: false,
                                }
                            }
                        }
                        Ty::Tuple(_) => ParamMapping {
                            param_type: "&str".into(),
                            conversion: "from_json({v})?".into(),
                            is_optional: false,
                        },
                        _ => ParamMapping {
                            param_type: "&str".into(),
                            conversion: "from_json({v})?".into(),
                            is_optional: false,
                        },
                    }
                }
            }
        }
        Link::Immediate(prim) => {
            let inner = map_immediate_primitive(prim);
            ParamMapping {
                param_type: format!("Vec<{}>", inner.param_type),
                conversion: "{v}".into(),
                is_optional: false,
            }
        }
        _ => panic!("Unexpected link in Vec"),
    }
}

fn map_admin_cancel_vec(
    tuple: &sov_universal_wallet::ty::Tuple<sov_universal_wallet::schema::IndexLinking>,
) -> ParamMapping {
    let id_index = match &tuple.fields[1].value {
        Link::ByIndex(i) => *i,
        _ => panic!("Expected ByIndex for cancel tuple field 1"),
    };

    let id_wrapper = match id_index {
        45 => "OrderId",
        57 => "TriggerOrderId",
        _ => panic!("Unknown ID type at index {id_index} in cancel tuple"),
    };

    let conversion = format!(
        "{{ let raw: Vec<(u16, u64, String)> = from_json({{v}})?; raw.into_iter().map(|(m, id, a)| Ok((MarketId(m), {id_wrapper}(id), parse_addr(&a)?))).collect::<Result<Vec<_>, String>>()? }}"
    );

    ParamMapping {
        param_type: "&str".into(),
        conversion,
        is_optional: false,
    }
}

fn map_link(
    link: &Link,
    types: &[Ty<sov_universal_wallet::schema::IndexLinking>],
    wrapper_indices: &HashSet<usize>,
    enum_indices: &HashSet<usize>,
) -> ParamMapping {
    match link {
        Link::ByIndex(i) => map_by_index(*i, types, wrapper_indices, enum_indices),
        Link::Immediate(prim) => map_immediate_primitive(prim),
        _ => panic!("Unexpected link type"),
    }
}

pub fn map_immediate_primitive(prim: &sov_universal_wallet::schema::Primitive) -> ParamMapping {
    use sov_universal_wallet::schema::Primitive as P;
    use sov_universal_wallet::ty::IntegerType;
    match prim {
        P::Boolean => ParamMapping {
            param_type: "bool".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        P::String => ParamMapping {
            param_type: "&str".into(),
            conversion: "{v}.into()".into(),
            is_optional: false,
        },
        P::Integer(IntegerType::u8, _) => ParamMapping {
            param_type: "u8".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        P::Integer(IntegerType::u16, _) => ParamMapping {
            param_type: "u16".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        P::Integer(IntegerType::u32, _) => ParamMapping {
            param_type: "u32".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        P::Integer(IntegerType::u64, _) => ParamMapping {
            param_type: "u64".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        P::Integer(IntegerType::i16, _) => ParamMapping {
            param_type: "i16".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        P::Integer(IntegerType::i64, _) => ParamMapping {
            param_type: "i64".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        P::Integer(IntegerType::u128, _) => ParamMapping {
            param_type: "u128".into(),
            conversion: "{v}".into(),
            is_optional: false,
        },
        other => panic!("Unsupported immediate primitive in link: {other:?}"),
    }
}
