//! Mapping for Option<T> types.
//!
//! Wraps an inner ParamMapping to produce the Option-ified version.
//! Handles the wasm-bindgen constraint that Option<&str> must be Option<String>,
//! and that fallible conversions need .map().transpose() patterns.

use super::ParamMapping;

/// Wrap an inner mapping as Option<T>.
pub fn map_option(inner: ParamMapping) -> ParamMapping {
    // Option<&str> → Option<String> because wasm-bindgen can't do Option<&str>.
    let param_type = if inner.param_type == "&str" {
        "Option<String>".into()
    } else {
        format!("Option<{}>", inner.param_type)
    };

    let conversion = build_option_conversion(&inner);

    ParamMapping { param_type, conversion, is_optional: true }
}

fn build_option_conversion(inner: &ParamMapping) -> String {
    if inner.param_type == "&str" {
        build_str_option_conversion(inner)
    } else if inner.conversion.ends_with(".inner") {
        // Option<WasmStruct> → .map(|w| w.inner)
        "{v}.map(|w| w.inner)".into()
    } else if inner.conversion == "{v}" {
        // Identity: Option<T> passes through.
        "{v}".into()
    } else if inner.param_type == "js_sys::Array" {
        // Option<Array> — inner has complex ? usage, wrap in Result closure.
        let inner_expr = inner.conversion.replace("{v}", "v");
        format!("{{v}}.map(|v| -> Result<_, String> {{ Ok({inner_expr}) }}).transpose()?")
    } else {
        let inner_expr = inner.conversion.replace("{v}", "v");
        if inner_expr.contains('?') {
            let expr_no_q = inner_expr.trim_end_matches('?');
            format!("{{v}}.map(|v| {expr_no_q}).transpose()?")
        } else if let Some(ctor) = simple_callable(&inner.conversion) {
            // e.g. "ClientOrderId({v})" → "{v}.map(ClientOrderId)"
            // e.g. "UnixTimestampMicros::from_micros({v})" →
            // "{v}.map(UnixTimestampMicros::from_micros)"
            format!("{{v}}.map({ctor})")
        } else {
            format!("{{v}}.map(|v| {inner_expr})")
        }
    }
}

/// Extract simple callable conversions like "TypeName({v})" or "Type::function({v})".
fn simple_callable(conversion: &str) -> Option<&str> {
    if let Some(rest) = conversion.strip_suffix("({v})") {
        let valid = !rest.is_empty()
            && rest.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == ':')
            && rest.split("::").all(|segment| {
                segment.chars().next().is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
            });
        valid.then_some(rest)
    } else {
        None
    }
}

fn build_str_option_conversion(inner: &ParamMapping) -> String {
    if inner.conversion == "{v}.into()" {
        // Option<String> → Option<CustomString>
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
        "{{v}}.as_deref().map(from_json).transpose()?".into()
    } else {
        let inner_expr = inner.conversion.replace("{v}", "s");
        if inner_expr.contains('?') {
            let expr_no_q = inner_expr.trim_end_matches('?');
            format!("{{v}}.as_deref().map(|s| {expr_no_q}).transpose()?")
        } else {
            format!("{{v}}.as_deref().map(|s| {inner_expr})")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ParamMapping, map_option};

    #[test]
    fn option_mapping_uses_associated_function_without_redundant_closure() {
        let mapping = map_option(ParamMapping {
            param_type: "i64".into(),
            conversion: "UnixTimestampMicros::from_micros({v})".into(),
            is_optional: false,
        });

        assert_eq!(mapping.conversion, "{v}.map(UnixTimestampMicros::from_micros)");
    }
}
