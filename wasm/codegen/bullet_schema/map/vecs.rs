//! Mapping for Vec<T> types.

use std::collections::HashSet;

use sov_universal_wallet::schema::Link;
use sov_universal_wallet::ty::Ty;

use super::super::Types;
use super::primitives;
use super::ParamMapping;

/// Map a `Vec { value }` schema type.
pub fn map_vec(value_link: &Link, types: &Types, wrapper_indices: &HashSet<usize>) -> ParamMapping {
    match value_link {
        Link::ByIndex(inner_idx) => map_vec_by_index(*inner_idx, types, wrapper_indices),
        Link::Immediate(prim) => {
            let inner = primitives::map_immediate(prim);
            ParamMapping {
                param_type: format!("Vec<{}>", inner.param_type),
                conversion: "{v}".into(),
                is_optional: false,
            }
        }
        _ => panic!("Unexpected link in Vec"),
    }
}

fn map_vec_by_index(
    inner_idx: usize,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
) -> ParamMapping {
    // Known newtype Vecs.
    if let Some(m) = try_map_known_vec(inner_idx) {
        return m;
    }

    // Dynamic dispatch based on inner type.
    let inner_ty = &types[inner_idx];
    match inner_ty {
        // Vec<NamedStruct> — accept js_sys::Array of wrapper objects.
        Ty::Struct(s) if !s.type_name.starts_with("__SovVirtualWallet_") => {
            if wrapper_indices.contains(&inner_idx) {
                let wrapper_name = format!("Wasm{}", s.type_name);
                ParamMapping {
                    param_type: "js_sys::Array".into(),
                    conversion: format!(
                        "extract_array::<{wrapper_name}>({{v}})?.into_iter().map(|w| w.inner).collect()"
                    ),
                    is_optional: false,
                }
            } else {
                json_fallback()
            }
        }
        // Vec<(multi-field tuple)> — admin cancel ops.
        Ty::Tuple(t) if t.fields.len() == 3 => map_admin_cancel_vec(t),
        // Everything else — JSON fallback.
        _ => json_fallback(),
    }
}

/// Known Vec<Newtype> patterns with direct primitive params.
fn try_map_known_vec(inner_idx: usize) -> Option<ParamMapping> {
    let (param_type, conversion) = match inner_idx {
        9 => ("Vec<u16>", "{v}.into_iter().map(AssetId).collect()"),
        25 => ("Vec<u16>", "{v}.into_iter().map(MarketId).collect()"),
        22 => (
            "Vec<String>",
            "{v}.iter().map(|s| parse_addr(s)).collect::<Result<Vec<_>, _>>()?",
        ),
        45 => ("Vec<u64>", "{v}.into_iter().map(OrderId).collect()"),
        57 => ("Vec<u64>", "{v}.into_iter().map(TriggerOrderId).collect()"),
        33 => ("Vec<u64>", "{v}.into_iter().map(ClientOrderId).collect()"),
        61 => ("Vec<u64>", "{v}.into_iter().map(TwapId).collect()"),
        _ => return None,
    };

    Some(ParamMapping {
        param_type: param_type.into(),
        conversion: conversion.into(),
        is_optional: false,
    })
}

/// Map `Vec<(MarketId, OrderId|TriggerOrderId, Address)>` for admin cancel ops.
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
        "{{ let raw: Vec<(u16, u64, String)> = from_json({{v}})?; \
         raw.into_iter().map(|(m, id, a)| Ok((MarketId(m), {id_wrapper}(id), parse_addr(&a)?)))\
         .collect::<Result<Vec<_>, String>>()? }}"
    );

    ParamMapping {
        param_type: "&str".into(),
        conversion,
        is_optional: false,
    }
}

fn json_fallback() -> ParamMapping {
    ParamMapping {
        param_type: "&str".into(),
        conversion: "from_json({v})?".into(),
        is_optional: false,
    }
}
