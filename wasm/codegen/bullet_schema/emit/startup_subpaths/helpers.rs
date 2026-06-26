use super::super::super::MappedField;

pub(super) fn ordered_fields(fields: &[MappedField]) -> Vec<&MappedField> {
    let mut ordered = fields.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|field| field.is_optional as u8);
    ordered
}

pub(super) fn field_names_array(fields: &[&MappedField]) -> String {
    let values = fields.iter().map(|f| js_string(&f.name)).collect::<Vec<_>>().join(", ");
    format!("[{values}]")
}

pub(super) fn js_params(fields: &[&MappedField]) -> String {
    fields
        .iter()
        .map(|f| if f.is_optional { format!("{} = undefined", f.name) } else { f.name.clone() })
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn js_values(fields: &[&MappedField]) -> String {
    fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>().join(", ")
}

pub(super) fn ts_param(field: &&MappedField) -> String {
    let name = &field.name;
    let ty = ts_type(&field.param_type);
    if field.is_optional { format!("{name}?: {ty} | null") } else { format!("{name}: {ty}") }
}

pub(super) fn ts_type(param_type: &str) -> String {
    let ty =
        param_type.strip_prefix("Option<").and_then(|v| v.strip_suffix('>')).unwrap_or(param_type);

    if let Some(inner) = ty.strip_prefix("Vec<").and_then(|v| v.strip_suffix('>')) {
        return match inner {
            "u8" => "Uint8Array | number[]".to_string(),
            "u64" => "BigUint64Array | bigint[] | number[]".to_string(),
            _ => format!("Array<{}>", ts_type(inner)),
        };
    }

    if ty.starts_with("Wasm") {
        return ty.trim_start_matches("Wasm").to_string();
    }

    match ty {
        "&str" | "String" => "string".to_string(),
        "bool" => "boolean".to_string(),
        "u64" | "u128" | "i64" => "bigint | number".to_string(),
        "u8" | "u16" | "u32" | "i16" => "number".to_string(),
        "js_sys::Array" => "Array<unknown>".to_string(),
        other => other.to_string(),
    }
}

pub(super) fn js_string(value: &str) -> String {
    format!("{value:?}")
}

pub(super) fn ts_string_literal(value: &str) -> String {
    js_string(value)
}
