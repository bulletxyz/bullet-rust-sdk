//! Mapping for known newtypes (named single-value `Tuple`s in the schema).

use sov_universal_wallet::schema::{Link, Primitive as SchemaPrimitive};
use sov_universal_wallet::ty::{ByteDisplay, IntegerType, Ty};

use super::super::super::Types;
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
pub fn try_map_newtype(idx: usize, types: &Types) -> Option<ParamMapping> {
    classify(idx, types).map(NewtypeKind::scalar_mapping)
}

pub fn classify(idx: usize, types: &Types) -> Option<NewtypeKind> {
    let kind = match &types[idx] {
        Ty::Tuple(tuple) if tuple.fields.len() == 1 => kind_from_name(tuple.type_name.as_deref()?)?,
        Ty::Struct(s) if s.type_name == "SurrogateDecimal" => NewtypeKind::SurrogateDecimal,
        _ => return None,
    };
    assert!(
        kind_matches_index(kind, idx, types),
        "schema index {idx} is named {kind:?}, but its shape no longer matches the conversion"
    );
    Some(kind)
}

pub fn classify_link(link: &Link, types: &Types) -> Option<NewtypeKind> {
    match link {
        Link::ByIndex(idx) => classify(*idx, types),
        _ => None,
    }
}

fn kind_from_name(name: &str) -> Option<NewtypeKind> {
    match name {
        "Address" => Some(NewtypeKind::Address),
        "AssetId" => Some(NewtypeKind::AssetId),
        "PositiveDecimal" => Some(NewtypeKind::PositiveDecimal),
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
            matches!(
                tuple_inner(idx, types),
                Some(Link::Immediate(SchemaPrimitive::String))
            )
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

impl NewtypeKind {
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
            NewtypeKind::TokenId => (
                "&str",
                "TokenId::from_str({v}).map_err(|e| format!(\"{e:?}\"))?",
            ),
        };

        ParamMapping {
            param_type: param_type.into(),
            conversion: conversion.into(),
            jsdoc_type: None,
            is_optional: false,
        }
    }

    pub fn vec_mapping(self) -> Option<ParamMapping> {
        let (param_type, conversion) = match self {
            NewtypeKind::Address => (
                "Vec<String>",
                "{v}.iter().map(|s| parse_addr(s)).collect::<Result<Vec<_>, _>>()?",
            ),
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
            jsdoc_type: None,
            is_optional: false,
        })
    }
}
