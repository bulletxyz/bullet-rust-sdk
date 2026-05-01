//! Mapping for known newtype wrapper indices.
//!
//! These are `Tuple` types in the schema that wrap a single inner value.
//! They must be identified by schema index (not structure) because multiple
//! `Tuple(u16)` or `Tuple(u64)` types exist with different semantics.

use super::ParamMapping;

/// Try to map a known newtype index. Returns `None` if the index is not a known newtype.
pub fn try_map_newtype(idx: usize) -> Option<ParamMapping> {
    let (param_type, conversion) = match idx {
        7 => ("&str", "parse_addr({v})?"),           // Address(Base58)
        15 => ("u16", "AssetId({v})"),               // AssetId(u16)
        16 => ("&str", "parse_dec({v})?"),           // PositiveDecimal
        17 => ("&str", "parse_surrogate_dec({v})?"), // SurrogateDecimal
        30 => ("u16", "MarketId({v})"),              // MarketId(u16)
        40 => ("i64", "UnixTimestampMicros::from_micros({v})"), // UnixTimestampMicros(i64)
        47 => ("u64", "ClientOrderId({v})"),         // ClientOrderId(u64)
        59 => ("u64", "OrderId({v})"),               // OrderId(u64)
        71 => ("u64", "TriggerOrderId({v})"),        // TriggerOrderId(u64)
        75 => ("u64", "TwapId({v})"),                // TwapId(u64)
        168 => ("&str", "TokenId::from_str({v}).unwrap()"), // TokenId(CustomString)
        _ => return None,
    };

    Some(ParamMapping {
        param_type: param_type.into(),
        conversion: conversion.into(),
        is_optional: false,
    })
}
