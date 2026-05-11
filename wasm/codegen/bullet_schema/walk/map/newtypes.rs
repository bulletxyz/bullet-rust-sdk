//! Mapping for known newtype wrappers.
//!
//! These are `Tuple` types in the schema that wrap a single inner value.

use sov_universal_wallet::schema::{Link, Primitive as SchemaPrimitive};
use sov_universal_wallet::ty::{ByteDisplay, IntegerType, Ty};

use super::super::super::{SerdeMetadata, Types};
use super::ParamMapping;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NewtypeKind {
    Address,
    AssetId,
    PositiveDecimal,
    SurrogateDecimal,
    MarketId,
    UnixTimestampMicros,
    ClientOrderId,
    OrderId,
    TriggerOrderId,
    TwapId,
    TokenId,
}

/// Try to map a known newtype. Returns `None` if the type is not a known newtype.
pub fn try_map_newtype(
    field_name: &str,
    idx: usize,
    types: &Types,
    serde_metadata: &SerdeMetadata,
) -> Option<ParamMapping> {
    classify(field_name, idx, types, serde_metadata).map(NewtypeKind::scalar_mapping)
}

pub fn classify(
    field_name: &str,
    idx: usize,
    types: &Types,
    serde_metadata: &SerdeMetadata,
) -> Option<NewtypeKind> {
    classify_index(idx, types, serde_metadata, NewtypeHint::Any)
        .or_else(|| classify_index_by_field_name(field_name, idx, types))
}

pub fn classify_link_as(
    link: &Link,
    expected: NewtypeKind,
    types: &Types,
    serde_metadata: &SerdeMetadata,
) -> Option<NewtypeKind> {
    classify_link(link, types, serde_metadata, NewtypeHint::Expected(expected))
}

#[derive(Clone, Copy)]
enum NewtypeHint {
    Any,
    Expected(NewtypeKind),
}

fn classify_index(
    idx: usize,
    types: &Types,
    serde_metadata: &SerdeMetadata,
    hint: NewtypeHint,
) -> Option<NewtypeKind> {
    if let Some(kind) = metadata_kind(idx, serde_metadata).or_else(|| explicit_index_kind(idx)) {
        return classify_known_index(idx, kind, types, hint);
    }

    match hint {
        NewtypeHint::Any => None,
        NewtypeHint::Expected(_) => {
            classify_index_by_expected_shape(idx, types, serde_metadata, hint)
        }
    }
}

fn classify_known_index(
    idx: usize,
    kind: NewtypeKind,
    types: &Types,
    hint: NewtypeHint,
) -> Option<NewtypeKind> {
    assert!(
        kind_matches_index(kind, idx, types),
        "newtype mapping for schema index {idx} classified it as {kind:?}, but the schema shape no longer matches"
    );
    hint.accept(kind)
}

fn classify_index_by_expected_shape(
    idx: usize,
    types: &Types,
    serde_metadata: &SerdeMetadata,
    hint: NewtypeHint,
) -> Option<NewtypeKind> {
    match &types[idx] {
        Ty::Tuple(tuple) if tuple.fields.len() == 1 => {
            classify_tuple_field(&tuple.fields[0].value, types, serde_metadata, hint)
        }
        Ty::Struct(s) if s.type_name == "SurrogateDecimal" => {
            hint.accept(NewtypeKind::SurrogateDecimal)
        }
        _ => None,
    }
}

fn classify_index_by_field_name(
    field_name: &str,
    idx: usize,
    types: &Types,
) -> Option<NewtypeKind> {
    match &types[idx] {
        Ty::Tuple(tuple) if tuple.fields.len() == 1 => {
            classify_tuple_field_by_name(field_name, &tuple.fields[0].value, types)
        }
        Ty::Struct(s) if s.type_name == "SurrogateDecimal" => Some(NewtypeKind::SurrogateDecimal),
        _ => None,
    }
}

fn classify_link(
    link: &Link,
    types: &Types,
    serde_metadata: &SerdeMetadata,
    hint: NewtypeHint,
) -> Option<NewtypeKind> {
    match link {
        Link::ByIndex(inner_idx) => classify_index(*inner_idx, types, serde_metadata, hint),
        Link::Immediate(prim) => classify_immediate(prim, hint),
        _ => None,
    }
}

fn classify_tuple_field(
    value: &Link,
    types: &Types,
    serde_metadata: &SerdeMetadata,
    hint: NewtypeHint,
) -> Option<NewtypeKind> {
    match (hint, value) {
        (NewtypeHint::Expected(NewtypeKind::PositiveDecimal), Link::ByIndex(inner_idx))
            if is_surrogate_decimal(*inner_idx, types) =>
        {
            Some(NewtypeKind::PositiveDecimal)
        }
        _ => classify_link(value, types, serde_metadata, hint),
    }
}

fn classify_immediate(prim: &SchemaPrimitive, hint: NewtypeHint) -> Option<NewtypeKind> {
    let NewtypeHint::Expected(kind) = hint else {
        return None;
    };
    kind.matches_immediate(prim).then_some(kind)
}

fn classify_tuple_field_by_name(
    field_name: &str,
    value: &Link,
    types: &Types,
) -> Option<NewtypeKind> {
    match value {
        Link::Immediate(SchemaPrimitive::ByteArray { len: 32, display }) => {
            matches!(display, ByteDisplay::Base58).then_some(NewtypeKind::Address)
        }
        Link::Immediate(SchemaPrimitive::Integer(IntegerType::u16, _)) => {
            classify_u16_field(field_name)
        }
        Link::Immediate(SchemaPrimitive::Integer(IntegerType::u64, _)) => {
            classify_u64_field(field_name)
        }
        Link::Immediate(SchemaPrimitive::Integer(IntegerType::i64, _))
            if is_timestamp_field(field_name) =>
        {
            Some(NewtypeKind::UnixTimestampMicros)
        }
        Link::Immediate(SchemaPrimitive::String) if is_token_id_field(field_name) => {
            Some(NewtypeKind::TokenId)
        }
        Link::ByIndex(inner_idx) if is_surrogate_decimal(*inner_idx, types) => {
            Some(NewtypeKind::PositiveDecimal)
        }
        _ => None,
    }
}

fn classify_u16_field(field_name: &str) -> Option<NewtypeKind> {
    if is_market_id_field(field_name) {
        Some(NewtypeKind::MarketId)
    } else if is_asset_id_field(field_name) {
        Some(NewtypeKind::AssetId)
    } else {
        None
    }
}

fn classify_u64_field(field_name: &str) -> Option<NewtypeKind> {
    if is_client_order_id_field(field_name) {
        Some(NewtypeKind::ClientOrderId)
    } else if is_trigger_order_id_field(field_name) {
        Some(NewtypeKind::TriggerOrderId)
    } else if is_twap_id_field(field_name) {
        Some(NewtypeKind::TwapId)
    } else if is_order_id_field(field_name) {
        Some(NewtypeKind::OrderId)
    } else {
        None
    }
}

fn is_client_order_id_field(field_name: &str) -> bool {
    field_name.ends_with("client_order_id") || field_name.ends_with("client_order_ids")
}

fn is_trigger_order_id_field(field_name: &str) -> bool {
    field_name.ends_with("trigger_order_id") || field_name.ends_with("trigger_order_ids")
}

fn is_twap_id_field(field_name: &str) -> bool {
    field_name.ends_with("twap_id") || field_name.ends_with("twap_ids")
}

fn is_order_id_field(field_name: &str) -> bool {
    field_name.ends_with("order_id") || field_name.ends_with("order_ids")
}

fn is_asset_id_field(field_name: &str) -> bool {
    field_name.ends_with("asset_id") || field_name.ends_with("asset_ids")
}

fn is_market_id_field(field_name: &str) -> bool {
    field_name.ends_with("market_id") || field_name.ends_with("market_ids")
}

fn is_token_id_field(field_name: &str) -> bool {
    field_name.ends_with("token_id")
}

fn is_timestamp_field(field_name: &str) -> bool {
    field_name == "timestamp" || field_name.ends_with("_timestamp") || field_name == "expires_at"
}

fn metadata_kind(idx: usize, serde_metadata: &SerdeMetadata) -> Option<NewtypeKind> {
    let name = serde_metadata.get(idx)?.name.as_str();
    kind_from_name(name)
}

fn kind_from_name(name: &str) -> Option<NewtypeKind> {
    let name = name.rsplit("::").next().unwrap_or(name);
    match name {
        "Address" => Some(NewtypeKind::Address),
        "AssetId" => Some(NewtypeKind::AssetId),
        "PositiveDecimal" => Some(NewtypeKind::PositiveDecimal),
        "SurrogateDecimal" => Some(NewtypeKind::SurrogateDecimal),
        "MarketId" => Some(NewtypeKind::MarketId),
        "UnixTimestampMicros" => Some(NewtypeKind::UnixTimestampMicros),
        "ClientOrderId" => Some(NewtypeKind::ClientOrderId),
        "OrderId" => Some(NewtypeKind::OrderId),
        "TriggerOrderId" => Some(NewtypeKind::TriggerOrderId),
        "TwapId" => Some(NewtypeKind::TwapId),
        "TokenId" => Some(NewtypeKind::TokenId),
        _ => None,
    }
}

fn explicit_index_kind(idx: usize) -> Option<NewtypeKind> {
    // `sov_universal_wallet` currently leaves serde metadata names empty for
    // tuple wrappers, so these anonymous schema indices are the only explicit
    // identifiers available for the current bullet-exchange-interface schema.
    // Keep this table narrow and shape-guarded; do not infer these wrappers
    // from field names.
    match idx {
        7 => Some(NewtypeKind::Address),
        15 => Some(NewtypeKind::AssetId),
        16 => Some(NewtypeKind::PositiveDecimal),
        17 => Some(NewtypeKind::SurrogateDecimal),
        30 => Some(NewtypeKind::MarketId),
        40 => Some(NewtypeKind::UnixTimestampMicros),
        47 => Some(NewtypeKind::ClientOrderId),
        59 => Some(NewtypeKind::OrderId),
        71 => Some(NewtypeKind::TriggerOrderId),
        75 => Some(NewtypeKind::TwapId),
        168 => Some(NewtypeKind::TokenId),
        _ => None,
    }
}

fn kind_matches_index(kind: NewtypeKind, idx: usize, types: &Types) -> bool {
    match kind {
        NewtypeKind::Address => tuple_inner(idx, types).is_some_and(matches_base58_address),
        NewtypeKind::AssetId | NewtypeKind::MarketId => {
            tuple_inner(idx, types).is_some_and(|link| matches_integer(link, IntegerType::u16))
        }
        NewtypeKind::PositiveDecimal => {
            matches!(tuple_inner(idx, types), Some(Link::ByIndex(inner_idx)) if is_surrogate_decimal(*inner_idx, types))
        }
        NewtypeKind::SurrogateDecimal => is_surrogate_decimal(idx, types),
        NewtypeKind::UnixTimestampMicros => {
            tuple_inner(idx, types).is_some_and(|link| matches_integer(link, IntegerType::i64))
        }
        NewtypeKind::ClientOrderId
        | NewtypeKind::OrderId
        | NewtypeKind::TriggerOrderId
        | NewtypeKind::TwapId => {
            tuple_inner(idx, types).is_some_and(|link| matches_integer(link, IntegerType::u64))
        }
        NewtypeKind::TokenId => {
            matches!(tuple_inner(idx, types), Some(Link::Immediate(SchemaPrimitive::String)))
        }
    }
}

fn tuple_inner(idx: usize, types: &Types) -> Option<&Link> {
    match &types[idx] {
        Ty::Tuple(tuple) if tuple.fields.len() == 1 => Some(&tuple.fields[0].value),
        _ => None,
    }
}

fn matches_base58_address(link: &Link) -> bool {
    matches!(
        link,
        Link::Immediate(SchemaPrimitive::ByteArray { len: 32, display })
            if matches!(display, ByteDisplay::Base58)
    )
}

fn matches_integer(link: &Link, expected: IntegerType) -> bool {
    matches!(
        link,
        Link::Immediate(SchemaPrimitive::Integer(kind, _)) if *kind == expected
    )
}

fn is_surrogate_decimal(idx: usize, types: &Types) -> bool {
    matches!(&types[idx], Ty::Struct(s) if s.type_name == "SurrogateDecimal")
}

impl NewtypeHint {
    fn accept(self, kind: NewtypeKind) -> Option<NewtypeKind> {
        match self {
            NewtypeHint::Any => Some(kind),
            NewtypeHint::Expected(expected) if expected == kind => Some(kind),
            NewtypeHint::Expected(_) => None,
        }
    }
}

impl NewtypeKind {
    fn matches_immediate(self, prim: &SchemaPrimitive) -> bool {
        match self {
            NewtypeKind::Address => matches!(
                prim,
                SchemaPrimitive::ByteArray { len: 32, display }
                    if matches!(display, ByteDisplay::Base58)
            ),
            NewtypeKind::AssetId | NewtypeKind::MarketId => {
                matches!(prim, SchemaPrimitive::Integer(IntegerType::u16, _))
            }
            NewtypeKind::UnixTimestampMicros => {
                matches!(prim, SchemaPrimitive::Integer(IntegerType::i64, _))
            }
            NewtypeKind::ClientOrderId
            | NewtypeKind::OrderId
            | NewtypeKind::TriggerOrderId
            | NewtypeKind::TwapId => {
                matches!(prim, SchemaPrimitive::Integer(IntegerType::u64, _))
            }
            NewtypeKind::TokenId => matches!(prim, SchemaPrimitive::String),
            NewtypeKind::PositiveDecimal | NewtypeKind::SurrogateDecimal => false,
        }
    }

    pub fn scalar_mapping(self) -> ParamMapping {
        let (param_type, conversion) = match self {
            NewtypeKind::Address => ("&str", "parse_addr({v})?"),
            NewtypeKind::AssetId => ("u16", "AssetId({v})"),
            NewtypeKind::PositiveDecimal => ("&str", "parse_dec({v})?"),
            NewtypeKind::SurrogateDecimal => ("&str", "parse_surrogate_dec({v})?"),
            NewtypeKind::MarketId => ("u16", "MarketId({v})"),
            NewtypeKind::UnixTimestampMicros => ("i64", "UnixTimestampMicros::from_micros({v})"),
            NewtypeKind::ClientOrderId => ("u64", "ClientOrderId({v})"),
            NewtypeKind::OrderId => ("u64", "OrderId({v})"),
            NewtypeKind::TriggerOrderId => ("u64", "TriggerOrderId({v})"),
            NewtypeKind::TwapId => ("u64", "TwapId({v})"),
            NewtypeKind::TokenId => {
                ("&str", "TokenId::from_str({v}).map_err(|e| format!(\"{e:?}\"))?")
            }
        };

        ParamMapping {
            param_type: param_type.into(),
            conversion: conversion.into(),
            is_optional: false,
        }
    }

    pub fn vec_mapping(self) -> Option<ParamMapping> {
        let (param_type, conversion) = match self {
            NewtypeKind::Address => {
                ("Vec<String>", "{v}.iter().map(|s| parse_addr(s)).collect::<Result<Vec<_>, _>>()?")
            }
            NewtypeKind::AssetId => ("Vec<u16>", "{v}.into_iter().map(AssetId).collect()"),
            NewtypeKind::MarketId => ("Vec<u16>", "{v}.into_iter().map(MarketId).collect()"),
            NewtypeKind::ClientOrderId => {
                ("Vec<u64>", "{v}.into_iter().map(ClientOrderId).collect()")
            }
            NewtypeKind::OrderId => ("Vec<u64>", "{v}.into_iter().map(OrderId).collect()"),
            NewtypeKind::TriggerOrderId => {
                ("Vec<u64>", "{v}.into_iter().map(TriggerOrderId).collect()")
            }
            NewtypeKind::TwapId => ("Vec<u64>", "{v}.into_iter().map(TwapId).collect()"),
            NewtypeKind::PositiveDecimal
            | NewtypeKind::SurrogateDecimal
            | NewtypeKind::UnixTimestampMicros
            | NewtypeKind::TokenId => return None,
        };

        Some(ParamMapping {
            param_type: param_type.into(),
            conversion: conversion.into(),
            is_optional: false,
        })
    }
}
