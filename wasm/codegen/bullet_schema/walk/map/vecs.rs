//! Mapping for Vec<T> types.

use std::collections::HashSet;

use sov_universal_wallet::schema::{Link, Primitive as SchemaPrimitive};
use sov_universal_wallet::ty::{Tuple, Ty};

use super::super::super::Types;
use super::newtypes::{self, NewtypeKind};
use super::{ParamMapping, primitives};

/// Map a `Vec { value }` schema type.
pub fn map_vec(
    context_name: &str,
    value_link: &Link,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
) -> ParamMapping {
    match value_link {
        Link::ByIndex(inner_idx) => {
            map_vec_by_index(context_name, *inner_idx, types, wrapper_indices)
        }
        Link::Immediate(SchemaPrimitive::ByteArray { .. }) => json_fallback(),
        Link::Immediate(prim) => {
            let inner = primitives::map_immediate(prim);
            ParamMapping {
                param_type: format!("Vec<{}>", inner.param_type),
                conversion: "{v}".into(),
                jsdoc_type: None,
                is_optional: false,
            }
        }
        _ => panic!("Unexpected link in Vec"),
    }
}

fn map_vec_by_index(
    context_name: &str,
    inner_idx: usize,
    types: &Types,
    wrapper_indices: &HashSet<usize>,
) -> ParamMapping {
    if let Some(m) = newtypes::classify(inner_idx, types).and_then(NewtypeKind::vec_mapping) {
        return m;
    }

    let inner_ty = &types[inner_idx];
    match inner_ty {
        Ty::Struct(s) if !s.type_name.starts_with("__SovVirtualWallet_") => {
            if wrapper_indices.contains(&inner_idx) {
                // JS passes `[new NewOrderArgs(...), ...]`; each element is recovered via
                // TryFromJsValue.
                let wrapper_name = format!("Wasm{}", s.type_name);
                ParamMapping {
                    param_type: "js_sys::Array".into(),
                    conversion: format!(
                        "extract_array::<{wrapper_name}>({{v}})?.into_iter().map(|w| w.inner).collect()"
                    ),
                    jsdoc_type: Some(format!("Array<{}>", s.type_name)),
                    is_optional: false,
                }
            } else {
                json_fallback()
            }
        }
        // Vec<(multi-field tuple)> — currently used by admin cancel ops.
        Ty::Tuple(t) => map_admin_cancel_vec(context_name, t, types).unwrap_or_else(json_fallback),
        _ => json_fallback(),
    }
}

/// Map the schema tuple used by admin cancel ops.
fn map_admin_cancel_vec(
    context_name: &str,
    tuple: &Tuple<sov_universal_wallet::schema::IndexLinking>,
    types: &Types,
) -> Option<ParamMapping> {
    let id_kind = admin_cancel_id_kind(context_name)?;
    let expected = [NewtypeKind::MarketId, id_kind, NewtypeKind::Address];
    if tuple.fields.len() != expected.len() {
        return None;
    }

    let mut seen = HashSet::new();
    let mut components = Vec::with_capacity(tuple.fields.len());
    for field in &tuple.fields {
        let kind = expected.iter().copied().find(|kind| {
            !seen.contains(kind) && newtypes::classify_link(&field.value, types) == Some(*kind)
        })?;
        seen.insert(kind);
        components.push(admin_cancel_component(kind));
    }
    if seen.len() != expected.len() {
        return None;
    }

    let raw_types = components
        .iter()
        .map(|c| c.raw_type)
        .collect::<Vec<_>>()
        .join(", ");
    let bindings = components
        .iter()
        .map(|c| c.binding)
        .collect::<Vec<_>>()
        .join(", ");
    let conversions = components
        .iter()
        .map(|c| c.conversion)
        .collect::<Vec<_>>()
        .join(", ");

    let conversion = format!(
        "{{ let raw: Vec<({raw_types})> = from_json({{v}})?; \
         raw.into_iter().map(|({bindings})| Ok(({conversions})))\
         .collect::<Result<Vec<_>, String>>()? }}"
    );

    Some(ParamMapping {
        param_type: "&str".into(),
        conversion,
        jsdoc_type: None,
        is_optional: false,
    })
}

fn admin_cancel_id_kind(context_name: &str) -> Option<NewtypeKind> {
    match context_name {
        "CancelOrders" => Some(NewtypeKind::OrderId),
        "CancelTriggerOrders" => Some(NewtypeKind::TriggerOrderId),
        _ => None,
    }
}

struct AdminCancelComponent {
    raw_type: &'static str,
    binding: &'static str,
    conversion: &'static str,
}

fn admin_cancel_component(kind: NewtypeKind) -> AdminCancelComponent {
    match kind {
        NewtypeKind::MarketId => AdminCancelComponent {
            raw_type: "u16",
            binding: "market_id",
            conversion: "MarketId(market_id)",
        },
        NewtypeKind::OrderId => AdminCancelComponent {
            raw_type: "u64",
            binding: "order_id",
            conversion: "OrderId(order_id)",
        },
        NewtypeKind::TriggerOrderId => AdminCancelComponent {
            raw_type: "u64",
            binding: "trigger_order_id",
            conversion: "TriggerOrderId(trigger_order_id)",
        },
        NewtypeKind::Address => AdminCancelComponent {
            raw_type: "String",
            binding: "address",
            conversion: "parse_addr(&address)?",
        },
        _ => panic!("unsupported admin cancel tuple kind {kind:?}"),
    }
}

fn json_fallback() -> ParamMapping {
    ParamMapping {
        param_type: "&str".into(),
        conversion: "from_json({v})?".into(),
        jsdoc_type: None,
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
            type_name: None,
            template: None,
            peekable: false,
            fields: vec![
                immediate_u64_field(),
                immediate_u64_field(),
                immediate_u64_field(),
            ],
        })];

        let mapping = map_vec("FutureStruct", &Link::ByIndex(0), &types, &HashSet::new());

        assert_eq!(mapping.param_type, "&str");
        assert_eq!(mapping.conversion, "from_json({v})?");
    }

    fn immediate_u64_field() -> UnnamedField<sov_universal_wallet::schema::IndexLinking> {
        UnnamedField {
            value: Link::Immediate(Primitive::Integer(
                IntegerType::u64,
                IntegerDisplay::Decimal,
            )),
            silent: false,
            doc: String::new(),
        }
    }
}
