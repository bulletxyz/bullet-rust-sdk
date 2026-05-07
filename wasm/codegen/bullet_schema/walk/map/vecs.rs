//! Mapping for Vec<T> types.

use std::collections::HashSet;

use sov_universal_wallet::schema::Link;
use sov_universal_wallet::ty::Ty;

use super::super::super::Types;
use super::newtypes::{self, NewtypeKind};
use super::{ParamMapping, primitives};

/// Map a `Vec { value }` schema type.
pub fn map_vec(
    context_name: &str,
    field_name: &str,
    value_link: &Link,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
) -> ParamMapping {
    match value_link {
        Link::ByIndex(inner_idx) => {
            map_vec_by_index(context_name, field_name, *inner_idx, types, wrapper_indices)
        }
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
    context_name: &str,
    field_name: &str,
    inner_idx: usize,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
) -> ParamMapping {
    // Known newtype Vecs.
    if let Some(m) =
        newtypes::classify(field_name, inner_idx, types).and_then(NewtypeKind::vec_mapping)
    {
        return m;
    }

    // Dynamic dispatch based on inner type.
    let inner_ty = &types[inner_idx];
    match inner_ty {
        // Vec<NamedStruct> — accept js_sys::Array of wrapper objects.
        Ty::Struct(s) if !s.type_name.starts_with("__SovVirtualWallet_") => {
            if wrapper_indices.contains(&inner_idx) {
                // This struct has a generated wrapper — accept a JS Array
                // of wrapper objects. Each element is recovered from the
                // JsValue via TryFromJsValue, then .inner extracts the
                // domain type. JS usage: `[new NewOrderArgs(...), ...]`
                let wrapper_name = format!("Wasm{}", s.type_name);
                ParamMapping {
                    param_type: "js_sys::Array".into(),
                    conversion: format!(
                        "extract_array::<{wrapper_name}>({{v}})?.into_iter().map(|w| w.inner).collect()"
                    ),
                    is_optional: false,
                }
            } else {
                // No wrapper — fall back to JSON string.
                json_fallback()
            }
        }
        // Vec<(multi-field tuple)> — admin cancel ops use a specialized
        // compact JSON shape. Other tuple Vecs should stay on the generic
        // JSON fallback so future schema additions do not panic here.
        Ty::Tuple(t) if t.fields.len() == 3 && is_admin_cancel_context(context_name) => {
            map_admin_cancel_vec(context_name, t, types)
        }
        // Everything else — JSON fallback.
        _ => json_fallback(),
    }
}

fn is_admin_cancel_context(context_name: &str) -> bool {
    matches!(context_name, "CancelOrders" | "CancelTriggerOrders")
}

/// Map `Vec<(MarketId, OrderId|TriggerOrderId, Address)>` for admin cancel ops.
fn map_admin_cancel_vec(
    context_name: &str,
    tuple: &sov_universal_wallet::ty::Tuple<sov_universal_wallet::schema::IndexLinking>,
    types: &Types,
) -> ParamMapping {
    expect_tuple_newtype(&tuple.fields[0].value, "market_id", NewtypeKind::MarketId, types);
    expect_tuple_newtype(&tuple.fields[2].value, "address", NewtypeKind::Address, types);

    let id_wrapper = match context_name {
        "CancelOrders" => {
            expect_tuple_newtype(&tuple.fields[1].value, "order_id", NewtypeKind::OrderId, types);
            "OrderId"
        }
        "CancelTriggerOrders" => {
            expect_tuple_newtype(
                &tuple.fields[1].value,
                "trigger_order_id",
                NewtypeKind::TriggerOrderId,
                types,
            );
            "TriggerOrderId"
        }
        _ => unreachable!("admin cancel context checked before mapping"),
    };

    let conversion = format!(
        "{{ let raw: Vec<(u16, u64, String)> = from_json({{v}})?; \
         raw.into_iter().map(|(m, id, a)| Ok((MarketId(m), {id_wrapper}(id), parse_addr(&a)?)))\
         .collect::<Result<Vec<_>, String>>()? }}"
    );

    ParamMapping { param_type: "&str".into(), conversion, is_optional: false }
}

fn expect_tuple_newtype(link: &Link, field_name: &str, expected: NewtypeKind, types: &Types) {
    let Link::ByIndex(idx) = link else {
        panic!("Expected ByIndex for {field_name} in cancel tuple");
    };

    let actual = newtypes::classify(field_name, *idx, types);
    assert_eq!(actual, Some(expected), "unexpected type for {field_name} in cancel tuple");
}

fn json_fallback() -> ParamMapping {
    ParamMapping {
        param_type: "&str".into(),
        conversion: "from_json({v})?".into(),
        is_optional: false,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use sov_universal_wallet::schema::{Link, Primitive};
    use sov_universal_wallet::ty::{IntegerDisplay, IntegerType, Ty, UnnamedField};

    use super::map_vec;

    #[test]
    fn unknown_three_tuple_vec_context_falls_back_to_json() {
        let types = vec![Ty::Tuple(sov_universal_wallet::ty::Tuple {
            template: None,
            peekable: false,
            fields: vec![immediate_u64_field(), immediate_u64_field(), immediate_u64_field()],
        })];

        let mapping =
            map_vec("FutureStruct", "tuple_items", &Link::ByIndex(0), &types, &HashSet::new());

        assert_eq!(mapping.param_type, "&str");
        assert_eq!(mapping.conversion, "from_json({v})?");
    }

    fn immediate_u64_field() -> UnnamedField<sov_universal_wallet::schema::IndexLinking> {
        UnnamedField {
            value: Link::Immediate(Primitive::Integer(IntegerType::u64, IntegerDisplay::Decimal)),
            silent: false,
            doc: String::new(),
        }
    }
}
