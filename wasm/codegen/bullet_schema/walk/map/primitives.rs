//! Mapping for immediate primitive types (bool, u8, u16, etc.).

use super::super::super::Primitive;
use super::ParamMapping;

/// Map an immediate primitive to its wasm param type and conversion.
pub fn map_primitive(prim: &Primitive) -> ParamMapping {
    let (param_type, conversion) = match prim {
        Primitive::Bool => ("bool", "{v}"),
        Primitive::ByteVec => ("Vec<u8>", "{v}"),
        Primitive::U8 => ("u8", "{v}"),
        Primitive::U16 => ("u16", "{v}"),
        Primitive::U32 => ("u32", "{v}"),
        Primitive::U64 => ("u64", "{v}"),
        Primitive::I16 => ("i16", "{v}"),
        Primitive::I64 => ("i64", "{v}"),
        Primitive::U128 => ("u128", "{v}"),
        Primitive::String => ("&str", "{v}.into()"),
    };

    ParamMapping {
        param_type: param_type.into(),
        conversion: conversion.into(),
        is_optional: false,
    }
}

/// Map an immediate primitive from the raw schema `Primitive` type.
pub fn map_immediate(prim: &sov_universal_wallet::schema::Primitive) -> ParamMapping {
    use sov_universal_wallet::schema::Primitive as P;
    use sov_universal_wallet::ty::IntegerType;

    if let P::ByteArray { len, .. } = prim {
        return ParamMapping {
            param_type: "Vec<u8>".into(),
            conversion: format!(
                "{{v}}.try_into().map_err(|v: Vec<u8>| format!(\"expected {len}-byte array, got {{}}\", v.len()))?"
            ),
            is_optional: false,
        };
    }

    let (param_type, conversion) = match prim {
        P::Boolean => ("bool", "{v}"),
        P::ByteVec { .. } => ("Vec<u8>", "{v}"),
        P::String => ("&str", "{v}.into()"),
        P::Integer(IntegerType::u8, _) => ("u8", "{v}"),
        P::Integer(IntegerType::u16, _) => ("u16", "{v}"),
        P::Integer(IntegerType::u32, _) => ("u32", "{v}"),
        P::Integer(IntegerType::u64, _) => ("u64", "{v}"),
        P::Integer(IntegerType::i16, _) => ("i16", "{v}"),
        P::Integer(IntegerType::i64, _) => ("i64", "{v}"),
        P::Integer(IntegerType::u128, _) => ("u128", "{v}"),
        other => panic!("Unsupported immediate primitive: {other:?}"),
    };

    ParamMapping {
        param_type: param_type.into(),
        conversion: conversion.into(),
        is_optional: false,
    }
}
