//! Mapping for known newtype wrappers.
//!
//! These are `Tuple` types in the schema that wrap a single inner value. We
//! classify them from their field name plus schema shape instead of relying on
//! schema indices, which shift whenever upstream adds or reorders types.

use sov_universal_wallet::schema::{Link, Primitive as SchemaPrimitive};
use sov_universal_wallet::ty::{ByteDisplay, IntegerType, Ty};

use super::super::super::Types;
use super::ParamMapping;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
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
pub fn try_map_newtype(field_name: &str, idx: usize, types: &Types) -> Option<ParamMapping> {
    classify(field_name, idx, types).map(NewtypeKind::scalar_mapping)
}

pub fn classify(field_name: &str, idx: usize, types: &Types) -> Option<NewtypeKind> {
    match &types[idx] {
        Ty::Tuple(tuple) if tuple.fields.len() == 1 => {
            classify_tuple_field(field_name, &tuple.fields[0].value, types)
        }
        Ty::Struct(s) if s.type_name == "SurrogateDecimal" => Some(NewtypeKind::SurrogateDecimal),
        _ => None,
    }
}

fn classify_tuple_field(field_name: &str, value: &Link, types: &Types) -> Option<NewtypeKind> {
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
